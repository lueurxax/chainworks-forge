//! Explicit administrative enrollment, separate from zero-uncertainty bootstrap.
//! No journal rows, historical grants, or uncertainty holds are changed here.
use crate::xcode_admin::{
    process_inventory, verify_legacy_process_inventory, LegacyLock, XcodeAdminCommand,
};
use acp::xcode_coordinator::{CoordinatorError, JournalAuthority};
use anyhow::{bail, ensure, Result};
use auth::Principal;
use serde::Serialize;
use sha2::{Digest, Sha256};
use sqlx::{Column, Connection, Row, TypeInfo, ValueRef};
use std::{
    fs::{self, File, OpenOptions},
    io::Read,
    os::{
        fd::AsRawFd,
        unix::fs::{FileExt, MetadataExt, OpenOptionsExt, PermissionsExt},
    },
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

const MAX_ROWS: i64 = 100_000;
const MAX_BYTES: u64 = 32 * 1024 * 1024;
const MAX_TRUST_FILES: usize = 4096;

pub async fn execute(
    command: &XcodeAdminCommand,
    principal: &Principal,
    database_url: &str,
    trust_root: &Path,
) -> Result<serde_json::Value> {
    execute_at(
        command,
        principal,
        database_url,
        trust_root,
        &AuthorityLocation::Production,
        StopProof::Native,
    )
    .await
}

enum AuthorityLocation {
    Production,
    #[cfg(test)]
    Fixture(PathBuf),
    #[cfg(test)]
    FixtureWithMaintenanceProbe {
        root: PathBuf,
        probe: fn(&Path),
    },
}
impl AuthorityLocation {
    fn inspect(
        &self,
        database: &Path,
        inventory: &str,
    ) -> Result<acp::xcode_coordinator::AuthorityRecoveryPlan, CoordinatorError> {
        match self {
            Self::Production => JournalAuthority::inspect_recovery(database, inventory),
            #[cfg(test)]
            Self::Fixture(root) | Self::FixtureWithMaintenanceProbe { root, .. } => {
                acp::xcode_coordinator::FixtureJournalAuthority::inspect_recovery(
                    root, database, inventory,
                )
            }
        }
    }
    fn enroll(
        &self,
        database: &Path,
        approved: &str,
        operator: &str,
        check: impl FnMut(&Path) -> Result<String, CoordinatorError>,
    ) -> Result<acp::xcode_coordinator::AuthorityRecoveryReceipt, CoordinatorError> {
        match self {
            Self::Production => {
                JournalAuthority::enroll_current_database(database, approved, operator, check)
            }
            #[cfg(test)]
            Self::Fixture(root) => {
                acp::xcode_coordinator::FixtureJournalAuthority::enroll_current_database(
                    root, database, approved, operator, check,
                )
            }
            #[cfg(test)]
            Self::FixtureWithMaintenanceProbe { root, probe } => {
                let mut check = check;
                acp::xcode_coordinator::FixtureJournalAuthority::enroll_current_database(
                    root,
                    database,
                    approved,
                    operator,
                    |path| {
                        probe(path);
                        check(path)
                    },
                )
            }
        }
    }
}
enum StopProof {
    Native,
    #[cfg(test)]
    Fixture(String),
}
impl StopProof {
    async fn read(self) -> Result<String> {
        match self {
            Self::Native => process_inventory().await,
            #[cfg(test)]
            Self::Fixture(value) => Ok(value),
        }
    }
}

fn authorize(principal: &Principal) -> Result<()> {
    ensure!(
        principal.class == domain::PrincipalClass::Operator
            && principal.run_scope.is_none()
            && principal
                .tool_capabilities
                .contains(&domain::CapabilityToolId::XcodeGlobalAdmin)
            && principal
                .tool_capabilities
                .contains(&domain::CapabilityToolId::XcodeProjectTrust),
        "xcode_authority_global_operator_required"
    );
    auth::require_xcode_operator(principal, domain::CapabilityToolId::XcodeProjectTrust, None)
        .map_err(anyhow::Error::msg)
}

async fn execute_at(
    command: &XcodeAdminCommand,
    principal: &Principal,
    database_url: &str,
    trust_root: &Path,
    authority: &AuthorityLocation,
    stop: StopProof,
) -> Result<serde_json::Value> {
    authorize(principal)?;
    let approved = match command {
        XcodeAdminCommand::AuthorityInspect {} => None,
        XcodeAdminCommand::AuthorityEnroll {
            approved_plan_digest,
            confirm_stopped_writers: true,
            ..
        } => {
            ensure!(
                approved_plan_digest.len() == 64
                    && approved_plan_digest
                        .bytes()
                        .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c)),
                "xcode_authority_invalid_plan_digest"
            );
            Some(approved_plan_digest.as_str())
        }
        _ => bail!("xcode_authority_stopped_writers_required"),
    };
    let options: sqlx::sqlite::SqliteConnectOptions = database_url.parse()?;
    let database = options.get_filename().to_path_buf();
    // Anchor before SQLite opens its handle, then validate again after opening.
    // A pool opened before this pin could silently refer to a replaced database.
    let pinned_database = SecureNode::open_database(&database)?;
    let singleton = approved
        .map(|_| LegacyLock::acquire(&database))
        .transpose()?;
    let trust = TrustInventory::open(trust_root, approved.is_some())?;
    if approved.is_none() {
        return inspect_snapshot(&pinned_database, &trust, authority).await;
    }
    let mut connection = sqlx::SqliteConnection::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new()
            .filename(&database)
            .create_if_missing(false)
            .busy_timeout(Duration::from_millis(100)),
    )
    .await?;
    pinned_database.check()?;
    let mut snapshot = connection.begin_with("BEGIN IMMEDIATE").await?;
    pinned_database.check()?;
    let journal = journal_inventory(&mut snapshot).await?;
    let trust_files = trust.inventory()?;
    let inventory_digest = digest_json(&(&journal, &trust_files))?;
    let plan = authority.inspect(&database, &inventory_digest);
    ensure!(trust_files.available, "xcode_authority_trust_lock_required");
    let plan = plan?;
    ensure!(
        Some(plan.plan_digest.as_str()) == approved,
        "xcode_authority_stale_plan"
    );
    let proved_at = Instant::now();
    let inventory = stop.read().await?;
    verify_legacy_process_inventory(&inventory, unsafe { libc::geteuid() }, std::process::id())?;
    // BEGIN IMMEDIATE keeps schema and all journal rows stable. The synchronous
    // publication callback additionally checks every opened filesystem handle.
    let receipt = authority.enroll(&database, approved.unwrap(), &principal.id, |path| {
        let verify = || -> Result<String> {
            ensure!(
                proved_at.elapsed() < Duration::from_secs(2),
                "xcode_authority_stop_proof_expired"
            );
            ensure!(path == database, "xcode_authority_database_changed");
            pinned_database.check()?;
            singleton
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("xcode_authority_singleton_required"))?
                .check()?;
            let current_trust = trust.inventory()?;
            ensure!(
                current_trust == trust_files,
                "xcode_authority_trust_changed"
            );
            ensure!(
                proved_at.elapsed() < Duration::from_secs(2),
                "xcode_authority_stop_proof_expired"
            );
            Ok(inventory_digest.clone())
        };
        verify().map_err(|_| CoordinatorError::LegacyTransitionUnproven)
    })?;
    // The transaction was used only to exclude writers, and is always rolled back.
    snapshot.rollback().await?;
    Ok(
        serde_json::json!({"enrolled": true, "receipt": receipt, "journal_unchanged": true, "trust_unchanged": true, "fresh_project_trust_required": true}),
    )
}

/// SQLite's read_only flag still creates WAL/SHM and may update SHM. Only the
/// disposable, private copy is opened by SQLite during inspection.
async fn inspect_snapshot(
    database: &SecureNode,
    trust: &TrustInventory,
    authority: &AuthorityLocation,
) -> Result<serde_json::Value> {
    let copy = InspectionCopy::capture(database)?;
    let mut connection = sqlx::SqliteConnection::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new()
            .filename(&copy.database)
            .create_if_missing(false)
            .busy_timeout(Duration::from_millis(100)),
    )
    .await?;
    let result = async {
        let mut snapshot = connection.begin().await?;
        let journal = journal_inventory(&mut snapshot).await?;
        let trust_files = trust.inventory()?;
        let inventory_digest = digest_json(&(&journal, &trust_files))?;
        let plan = authority.inspect(&database.path, &inventory_digest);
        trust.check()?;
        database.check()?;
        snapshot.rollback().await?;
        let mut blockers = Vec::new();
        if !trust_files.available { blockers.push("project_trust_store_missing".to_owned()); }
        let plan = match plan {
            Ok(plan) => Some(plan),
            Err(error) => { blockers.push(error.to_string()); None }
        };
        Ok(serde_json::json!({"plan": plan, "inventory": {"journal": journal, "trust": trust_files}, "blockers": blockers,
            "required_decision": "enroll-current-database-as-new-authority", "requires_stopped_writers": true,
            "fresh_project_trust_required": true}))
    }.await;
    // Always close the copied database before removing its private directory,
    // including failed queries or failed native authority inspection.
    connection.close().await?;
    result
}

struct InspectionCopy {
    _directory: tempfile::TempDir,
    database: PathBuf,
}
impl InspectionCopy {
    fn capture(source: &SecureNode) -> Result<Self> {
        let _lock = SqliteSnapshotLock::acquire(&source.file)?;
        source.check()?;
        let mut header = [0_u8; 100];
        source.file.read_exact_at(&mut header, 0)?;
        ensure!(
            &header[..16] == b"SQLite format 3\0" && header[18] == 2 && header[19] == 2,
            "xcode_authority_snapshot_requires_wal_database"
        );
        // A rollback journal can reference a super-journal outside this source.
        // Refuse it instead of allowing recovery of such references in a copy.
        let journal = sidecar_path(&source.path, "-journal");
        ensure!(
            matches!(fs::symlink_metadata(&journal), Err(error) if error.kind() == std::io::ErrorKind::NotFound),
            "xcode_authority_rollback_journal_present"
        );
        let wal_path = sidecar_path(&source.path, "-wal");
        let wal = match fs::symlink_metadata(&wal_path) {
            Ok(_) => Some(SecureNode::open(&wal_path, false)?),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(error) => return Err(error.into()),
        };
        let directory = tempfile::Builder::new()
            .prefix("chainworks-authority-inspect-")
            .permissions(fs::Permissions::from_mode(0o700))
            .tempdir()?;
        let database = directory.path().join("snapshot.sqlite");
        let mut remaining = 2_u64 * 1024 * 1024 * 1024;
        copy_private(source, &database, &mut remaining)?;
        if let Some(wal) = &wal {
            copy_private(wal, &sidecar_path(&database, "-wal"), &mut remaining)?;
        }
        source.check()?;
        if let Some(wal) = &wal {
            wal.check()?;
        } else {
            ensure!(
                matches!(fs::symlink_metadata(&wal_path), Err(error) if error.kind() == std::io::ErrorKind::NotFound),
                "xcode_authority_snapshot_changed"
            );
        }
        ensure!(
            matches!(fs::symlink_metadata(&journal), Err(error) if error.kind() == std::io::ErrorKind::NotFound),
            "xcode_authority_snapshot_changed"
        );
        // OFD guard remains alive until all source copies/checks finish. SQLite
        // only sees the copy after this returns and releases the source lock.
        Ok(Self {
            _directory: directory,
            database,
        })
    }
}

fn sidecar_path(database: &Path, suffix: &str) -> PathBuf {
    let mut path = database.as_os_str().to_os_string();
    path.push(suffix);
    path.into()
}

fn copy_private(source: &SecureNode, destination: &Path, remaining: &mut u64) -> Result<()> {
    use std::io::Write;
    let before = source.file.metadata()?;
    ensure!(before.len() <= *remaining, "xcode_authority_snapshot_limit");
    let mut target = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(destination)?;
    let mut offset = 0;
    let mut buffer = [0_u8; 65536];
    while offset < before.len() {
        let size = (before.len() - offset).min(buffer.len() as u64) as usize;
        source.file.read_exact_at(&mut buffer[..size], offset)?;
        target.write_all(&buffer[..size])?;
        offset += size as u64;
    }
    let after = source.file.metadata()?;
    ensure!(
        before.len() == after.len()
            && before.mtime() == after.mtime()
            && before.mtime_nsec() == after.mtime_nsec()
            && before.ctime() == after.ctime()
            && before.ctime_nsec() == after.ctime_nsec(),
        "xcode_authority_snapshot_changed"
    );
    source.check()?;
    *remaining -= before.len();
    Ok(())
}

/// Native open-file-description locks conflict with SQLite's POSIX locks,
/// including the database SHARED lock required for WAL readers/writers. Unlike
/// F_SETLK, another fd closing in this process cannot silently release this lock.
/// No fallback to process-scoped locks or unlocked/immutable source reads exists.
struct SqliteSnapshotLock<'a> {
    file: &'a File,
}
impl<'a> SqliteSnapshotLock<'a> {
    fn acquire(file: &'a File) -> Result<Self> {
        sqlite_ofd_lock(file, libc::F_WRLCK as _)?;
        Ok(Self { file })
    }
}
impl Drop for SqliteSnapshotLock<'_> {
    fn drop(&mut self) {
        let _ = sqlite_ofd_lock(self.file, libc::F_UNLCK as _);
    }
}
fn sqlite_ofd_lock(file: &File, kind: libc::c_short) -> Result<()> {
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    {
        let mut lock: libc::flock = unsafe { std::mem::zeroed() };
        lock.l_type = kind;
        lock.l_whence = libc::SEEK_SET as _;
        lock.l_start = 0x40000000;
        lock.l_len = 512;
        ensure!(
            unsafe { libc::fcntl(file.as_raw_fd(), libc::F_OFD_SETLK, &lock) } == 0,
            "xcode_authority_snapshot_busy_or_unsupported"
        );
        Ok(())
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        let _ = (file, kind);
        bail!("xcode_authority_snapshot_lock_unsupported")
    }
}

fn digest_json(value: &impl Serialize) -> Result<String> {
    Ok(format!("{:x}", Sha256::digest(serde_json::to_vec(value)?)))
}

struct SecureNode {
    path: PathBuf,
    file: File,
    directory: bool,
}
impl SecureNode {
    fn open(path: &Path, directory: bool) -> Result<Self> {
        Self::open_mode(path, directory, false)
    }
    fn open_database(path: &Path) -> Result<Self> {
        Self::open_mode(path, false, true)
    }
    fn open_mode(path: &Path, directory: bool, for_lock: bool) -> Result<Self> {
        ensure!(
            path.is_absolute() && path.canonicalize()? == path,
            "xcode_authority_insecure_path"
        );
        let file = OpenOptions::new()
            .read(true)
            .write(for_lock)
            .custom_flags(
                libc::O_NOFOLLOW
                    | libc::O_NONBLOCK
                    | libc::O_CLOEXEC
                    | if directory { libc::O_DIRECTORY } else { 0 },
            )
            .open(path)?;
        let node = Self {
            path: path.into(),
            file,
            directory,
        };
        node.check()?;
        Ok(node)
    }
    fn check(&self) -> Result<()> {
        ensure!(
            self.path.canonicalize()? == self.path,
            "xcode_authority_path_changed"
        );
        let named = fs::symlink_metadata(&self.path)?;
        let opened = self.file.metadata()?;
        for node in [&named, &opened] {
            ensure!(
                node.uid() == unsafe { libc::geteuid() }
                    && node.mode() & 0o7777 == if self.directory { 0o700 } else { 0o600 }
                    && if self.directory {
                        node.is_dir()
                    } else {
                        node.is_file() && node.nlink() == 1
                    },
                "xcode_authority_insecure_node"
            );
        }
        ensure!(
            named.dev() == opened.dev() && named.ino() == opened.ino(),
            "xcode_authority_path_changed"
        );
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct TrustFile {
    name: String,
    sha256: String,
    device: u64,
    inode: u64,
    length: u64,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct TrustFiles {
    available: bool,
    root: String,
    directory_device: Option<u64>,
    directory_inode: Option<u64>,
    files: Vec<TrustFile>,
}
struct TrustInventory {
    root: PathBuf,
    directory: Option<SecureNode>,
    lock: Option<SecureNode>,
}
impl TrustInventory {
    fn open(root: &Path, exclusive: bool) -> Result<Self> {
        if let Err(error) = fs::symlink_metadata(root) {
            if error.kind() == std::io::ErrorKind::NotFound {
                ensure!(!exclusive, "xcode_authority_trust_lock_required");
                return Ok(Self {
                    root: root.into(),
                    directory: None,
                    lock: None,
                });
            }
            return Err(error.into());
        }
        let directory = SecureNode::open(root, true)?;
        let lock = SecureNode::open(&root.join("trust.lock"), false)?;
        ensure!(
            unsafe {
                libc::flock(
                    lock.file.as_raw_fd(),
                    (if exclusive {
                        libc::LOCK_EX
                    } else {
                        libc::LOCK_SH
                    }) | libc::LOCK_NB,
                )
            } == 0,
            "xcode_authority_trust_busy"
        );
        let store = Self {
            root: root.into(),
            directory: Some(directory),
            lock: Some(lock),
        };
        store.check()?;
        Ok(store)
    }
    fn check(&self) -> Result<()> {
        if let (Some(directory), Some(lock)) = (&self.directory, &self.lock) {
            directory.check()?;
            lock.check()?;
        } else {
            ensure!(
                matches!(fs::symlink_metadata(&self.root), Err(error) if error.kind() == std::io::ErrorKind::NotFound),
                "xcode_authority_trust_changed"
            );
        }
        Ok(())
    }
    fn inventory(&self) -> Result<TrustFiles> {
        self.check()?;
        if self.directory.is_none() {
            return Ok(TrustFiles {
                available: false,
                root: self.root.display().to_string(),
                directory_device: None,
                directory_inode: None,
                files: Vec::new(),
            });
        }
        let mut paths = Vec::new();
        for entry in fs::read_dir(&self.root)? {
            let entry = entry?;
            ensure!(
                paths.len() < MAX_TRUST_FILES,
                "xcode_authority_inventory_limit"
            );
            paths.push(entry.path());
        }
        paths.sort();
        let mut files = Vec::new();
        let mut total = 0;
        for path in &paths {
            let node = SecureNode::open(path, false)?;
            let before = node.file.metadata()?;
            ensure!(before.len() <= 65536, "xcode_authority_inventory_limit");
            total += before.len();
            ensure!(total <= MAX_BYTES, "xcode_authority_inventory_limit");
            let mut bytes = Vec::new();
            (&node.file).take(65537).read_to_end(&mut bytes)?;
            let after = node.file.metadata()?;
            ensure!(
                bytes.len() as u64 == before.len()
                    && before.len() == after.len()
                    && before.mtime() == after.mtime()
                    && before.mtime_nsec() == after.mtime_nsec(),
                "xcode_authority_trust_changed"
            );
            node.check()?;
            let name = path
                .file_name()
                .and_then(|name| name.to_str())
                .ok_or_else(|| anyhow::anyhow!("xcode_authority_insecure_path"))?;
            files.push(TrustFile {
                name: name.into(),
                sha256: format!("{:x}", Sha256::digest(&bytes)),
                device: before.dev(),
                inode: before.ino(),
                length: before.len(),
            });
        }
        let mut after: Vec<_> = fs::read_dir(&self.root)?
            .map(|entry| entry.map(|entry| entry.path()))
            .collect::<std::io::Result<_>>()?;
        after.sort();
        ensure!(paths == after, "xcode_authority_trust_changed");
        self.check()?;
        let directory = self.directory.as_ref().unwrap().file.metadata()?;
        Ok(TrustFiles {
            available: true,
            root: self.root.display().to_string(),
            directory_device: Some(directory.dev()),
            directory_inode: Some(directory.ino()),
            files,
        })
    }
}

#[derive(Serialize)]
struct TableInventory {
    table: &'static str,
    rows: i64,
    sha256: String,
}
#[derive(Serialize)]
struct JournalInventory {
    journal_mode: String,
    schema_version: i64,
    user_version: i64,
    tables: Vec<TableInventory>,
}
async fn journal_inventory(connection: &mut sqlx::SqliteConnection) -> Result<JournalInventory> {
    // On apply this query is inside BEGIN IMMEDIATE. Native identity probes
    // open/close DB fds, so require the WAL writer lock on SHM (which those
    // probes never open), not a rollback reservation on the DB inode.
    let journal_mode: String = sqlx::query_scalar("PRAGMA journal_mode")
        .fetch_one(&mut *connection)
        .await?;
    ensure!(
        journal_mode == "wal",
        "xcode_authority_requires_wal_database"
    );
    let schema_version = sqlx::query_scalar("PRAGMA schema_version")
        .fetch_one(&mut *connection)
        .await?;
    let user_version = sqlx::query_scalar("PRAGMA user_version")
        .fetch_one(&mut *connection)
        .await?;
    let mut tables = Vec::new();
    for (table, order) in [
        ("sqlite_schema", "type,name"),
        ("_sqlx_migrations", "version"),
        ("xcode_effect_attempts", "sequence"),
        ("xcode_project_holds", "project_key"),
        ("side_effects", "id"),
    ] {
        // Identifiers come exclusively from this closed list or SQLite's column
        // metadata, never CLI input. Bound total bytes before materializing rows.
        let columns: Vec<String> =
            sqlx::query("SELECT name FROM pragma_table_info(?) ORDER BY cid")
                .bind(table)
                .fetch_all(&mut *connection)
                .await?
                .iter()
                .map(|row| row.get("name"))
                .collect();
        ensure!(
            !columns.is_empty() && columns.len() <= 256,
            "xcode_authority_schema_unavailable"
        );
        let lengths = columns
            .iter()
            .map(|name| {
                format!(
                    "COALESCE(length(CAST(\"{}\" AS BLOB)),0)",
                    name.replace('"', "\"\"")
                )
            })
            .collect::<Vec<_>>()
            .join("+");
        let sizes = sqlx::query(sqlx::AssertSqlSafe(format!(
            "SELECT count(*) AS rows, COALESCE(sum({lengths}),0) AS bytes FROM {table}"
        )))
        .fetch_one(&mut *connection)
        .await?;
        let count: i64 = sizes.try_get("rows")?;
        let bytes: i64 = sizes.try_get("bytes")?;
        ensure!(
            (0..=MAX_ROWS).contains(&count) && (0..=MAX_BYTES as i64).contains(&bytes),
            "xcode_authority_inventory_limit"
        );
        let rows = sqlx::query(sqlx::AssertSqlSafe(format!(
            "SELECT * FROM {table} ORDER BY {order}"
        )))
        .fetch_all(&mut *connection)
        .await?;
        let mut digest = Sha256::new();
        hash_field(&mut digest, table.as_bytes());
        for row in rows {
            digest.update([0xff]);
            for (index, column) in row.columns().iter().enumerate() {
                hash_field(&mut digest, column.name().as_bytes());
                let value = row.try_get_raw(index)?;
                if value.is_null() {
                    hash_field(&mut digest, b"NULL");
                    continue;
                }
                let kind = value.type_info();
                hash_field(&mut digest, kind.name().as_bytes());
                match kind.name() {
                    "INTEGER" => {
                        hash_field(&mut digest, &row.try_get::<i64, _>(index)?.to_le_bytes())
                    }
                    "REAL" => hash_field(
                        &mut digest,
                        &row.try_get::<f64, _>(index)?.to_bits().to_le_bytes(),
                    ),
                    "TEXT" => hash_field(&mut digest, row.try_get::<String, _>(index)?.as_bytes()),
                    "BLOB" => hash_field(&mut digest, &row.try_get::<Vec<u8>, _>(index)?),
                    _ => bail!("xcode_authority_inventory_type_unavailable"),
                }
            }
        }
        tables.push(TableInventory {
            table,
            rows: count,
            sha256: format!("{:x}", digest.finalize()),
        });
    }
    Ok(JournalInventory {
        journal_mode,
        schema_version,
        user_version,
        tables,
    })
}
fn hash_field(digest: &mut Sha256, bytes: &[u8]) {
    digest.update((bytes.len() as u64).to_le_bytes());
    digest.update(bytes);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::{symlink, PermissionsExt};
    struct Fixture {
        _dir: tempfile::TempDir,
        root: PathBuf,
        database: PathBuf,
        url: String,
        pool: sqlx::SqlitePool,
    }
    impl Fixture {
        async fn new() -> Self {
            let dir = tempfile::tempdir().unwrap();
            let root = dir.path().canonicalize().unwrap();
            let database = root.join("fixture.sqlite");
            let url = format!("sqlite://{}?mode=rwc", database.display());
            let pool = db::pool::create_pool(&url).await.unwrap();
            fs::set_permissions(&database, fs::Permissions::from_mode(0o600)).unwrap();
            let authority = acp::xcode_coordinator::FixtureJournalAuthority::open(
                &root.join("authority"),
                &database,
            )
            .unwrap();
            drop(authority);
            pool.close().await;
            Self {
                _dir: dir,
                root,
                database,
                url,
                pool,
            }
        }
        fn trust(&self) {
            fs::create_dir(self.root.join("trust")).unwrap();
            fs::set_permissions(self.root.join("trust"), fs::Permissions::from_mode(0o700))
                .unwrap();
            self.file("trust/trust.lock", b"");
            self.file(
                "trust/legacy-grant.json",
                br#"{"version":2,"revoked_at":"preserved"}"#,
            );
        }
        fn file(&self, name: &str, bytes: &[u8]) {
            fs::write(self.root.join(name), bytes).unwrap();
            fs::set_permissions(self.root.join(name), fs::Permissions::from_mode(0o600)).unwrap();
        }
        async fn run(&self, command: &XcodeAdminCommand) -> Result<serde_json::Value> {
            self.run_as(command, &operator(), stopped()).await
        }
        async fn run_as(
            &self,
            command: &XcodeAdminCommand,
            principal: &Principal,
            stop: String,
        ) -> Result<serde_json::Value> {
            execute_at(
                command,
                principal,
                &self.url,
                &self.root.join("trust"),
                &AuthorityLocation::Fixture(self.root.join("authority")),
                StopProof::Fixture(stop),
            )
            .await
        }
        async fn plan(&self) -> String {
            self.run(&XcodeAdminCommand::AuthorityInspect {})
                .await
                .unwrap()["plan"]["plan_digest"]
                .as_str()
                .unwrap()
                .into()
        }
        async fn connection(&self) -> sqlx::SqliteConnection {
            sqlx::SqliteConnection::connect_with(
                &sqlx::sqlite::SqliteConnectOptions::new()
                    .filename(&self.database)
                    .create_if_missing(false),
            )
            .await
            .unwrap()
        }
        async fn journal(&self) -> String {
            let mut conn = self.connection().await;
            let result = digest_json(&journal_inventory(&mut conn).await.unwrap()).unwrap();
            conn.close().await.unwrap();
            result
        }
        async fn unknown(&self) {
            let mut connection = self.connection().await;
            let id = uuid::Uuid::new_v4().to_string();
            let operation = uuid::Uuid::new_v4().to_string();
            let key = serde_json::json!({"version":1,"uid":unsafe {libc::geteuid()},"canonical_path":"/fixture/App.xcodeproj","device":"7","inode":"11"});
            let intent = serde_json::json!({"owner_lineage":"fixture", "project_key":key,"operation_key":operation,"origin":"lifecycle","effect_class":"mutate","binding_digest":null,"operation":"workspace_open"});
            sqlx::query("INSERT INTO xcode_effect_attempts(attempt_id,nonce,owner_lineage,project_key,operation_key,intent_json,state,revision,created_at,updated_at,dispatched_at,uncertainty_reason) VALUES (?,?,'fixture',?,?,?,'unknown',2,'before','before','before','restart')").bind(&id).bind(&id).bind(key.to_string()).bind(operation).bind(intent.to_string()).execute(&mut connection).await.unwrap();
            sqlx::query("INSERT INTO xcode_project_holds(project_key,attempt_id,uid,canonical_path,device,inode) VALUES (?,?,?,'/fixture/App.xcodeproj','7','11')").bind(key.to_string()).bind(id).bind(unsafe {libc::geteuid()} as i64).execute(&mut connection).await.unwrap();
            sqlx::query("INSERT INTO side_effects(id,run_id,stage_execution_id,effect_kind,target_key,idempotency_key,request_fingerprint,status,external_write_attempted,created_at,updated_at) VALUES ('unknown-effect','run','stage','build_archive','target','key','fingerprint','executing',1,'before','before')").execute(&mut connection).await.unwrap();
            connection.close().await.unwrap();
        }
    }
    fn operator() -> Principal {
        let mut principal = Principal::new("operator", domain::PrincipalClass::Operator);
        principal.has_explicit_surface_policies = true;
        principal.tool_capabilities = [
            domain::CapabilityToolId::XcodeGlobalAdmin,
            domain::CapabilityToolId::XcodeProjectTrust,
        ]
        .into();
        principal
    }
    fn stopped() -> String {
        format!("{} {} /fixture/maintenance\n", std::process::id(), unsafe {
            libc::geteuid()
        })
    }
    fn enroll(digest: String) -> XcodeAdminCommand {
        XcodeAdminCommand::AuthorityEnroll {
            approved_plan_digest: digest,
            decision:
                crate::xcode_admin::AuthorityEnrollmentDecision::EnrollCurrentDatabaseAsNewAuthority,
            confirm_stopped_writers: true,
        }
    }

    fn source_fingerprint(root: &Path) -> Vec<(std::ffi::OsString, String, u64, u64, i64, i64)> {
        let mut entries: Vec<_> = fs::read_dir(root)
            .unwrap()
            .map(|entry| {
                let entry = entry.unwrap();
                let metadata = entry.metadata().unwrap();
                (
                    entry.file_name(),
                    if metadata.is_file() {
                        format!("{:x}", Sha256::digest(fs::read(entry.path()).unwrap()))
                    } else {
                        String::new()
                    },
                    metadata.ino(),
                    metadata.len(),
                    metadata.mtime(),
                    metadata.mtime_nsec(),
                )
            })
            .collect();
        entries.sort_by(|a, b| a.0.cmp(&b.0));
        entries
    }

    fn sqlite_peer(database: &Path, action: &str) {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "xcode_authority_recovery::tests::sqlite_snapshot_child",
                "--nocapture",
            ])
            .env("CW_AUTHORITY_FIXTURE_DATABASE", database)
            .env("CW_AUTHORITY_FIXTURE_ACTION", action)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "SQLite fixture peer failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[test]
    fn sqlite_snapshot_child() {
        let Some(database) = std::env::var_os("CW_AUTHORITY_FIXTURE_DATABASE") else {
            return;
        };
        let database = PathBuf::from(database);
        assert!(database.starts_with(std::env::temp_dir().canonicalize().unwrap()));
        assert_eq!(database.file_name().unwrap(), "fixture.sqlite");
        let action = std::env::var("CW_AUTHORITY_FIXTURE_ACTION").unwrap();
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                let options = sqlx::sqlite::SqliteConnectOptions::new()
                    .filename(database)
                    .create_if_missing(false)
                    .busy_timeout(Duration::from_millis(20));
                let result: Result<()> = async {
                    let mut connection = sqlx::SqliteConnection::connect_with(&options).await?;
                    if action == "write-busy" {
                        sqlx::query(
                            "UPDATE side_effects SET last_error = 'fixture-write-must-not-land'",
                        )
                        .execute(&mut connection)
                        .await?;
                        connection.close().await?;
                        return Ok(());
                    }
                    if action == "hold" {
                        use std::io::Write;
                        let mut transaction = connection.begin().await?;
                        let _: i64 = sqlx::query_scalar("SELECT count(*) FROM side_effects")
                            .fetch_one(&mut *transaction)
                            .await?;
                        println!("PEER_READY");
                        std::io::stdout().flush()?;
                        let mut release = String::new();
                        std::io::stdin().read_line(&mut release)?;
                        ensure!(release.trim() == "release", "fixture_release_required");
                        transaction.rollback().await?;
                        connection.close().await?;
                        return Ok(());
                    }
                    if action == "crash-wal" {
                        sqlx::query("UPDATE side_effects SET last_error = 'fixture-wal-commit'")
                            .execute(&mut connection)
                            .await?;
                        // Deliberately leave committed WAL without close/checkpoint.
                        std::process::exit(0);
                    }
                    let _: i64 = sqlx::query_scalar("SELECT count(*) FROM side_effects")
                        .fetch_one(&mut connection)
                        .await?;
                    connection.close().await?;
                    Ok(())
                }
                .await;
                if matches!(action.as_str(), "busy" | "write-busy") {
                    assert!(result.unwrap_err().to_string().contains("locked"));
                } else {
                    assert!(matches!(action.as_str(), "read" | "hold"));
                    result.unwrap();
                }
            });
    }

    #[tokio::test]
    async fn snapshot_lock_excludes_sqlite_peers_and_survives_other_handle_close() {
        let f = Fixture::new().await;
        let source = SecureNode::open_database(&f.database).unwrap();
        let guard = SqliteSnapshotLock::acquire(&source.file).unwrap();
        // OFD locks must survive native identity probing that opens/closes a
        // second descriptor for the same source inode.
        drop(File::open(&f.database).unwrap());
        sqlite_peer(&f.database, "busy");
        let options = sqlx::sqlite::SqliteConnectOptions::new()
            .filename(&f.database)
            .create_if_missing(false)
            .busy_timeout(Duration::from_millis(20));
        let mut connection = sqlx::SqliteConnection::connect_with(&options)
            .await
            .unwrap();
        assert!(sqlx::query("SELECT * FROM side_effects")
            .fetch_all(&mut connection)
            .await
            .is_err());
        connection.close().await.unwrap();
        sqlite_peer(&f.database, "busy");
        drop(guard);
        sqlite_peer(&f.database, "read");
    }

    #[tokio::test]
    async fn inspection_rejects_open_sqlite_peer_without_source_changes() {
        use std::io::{BufRead, Write};
        let f = Fixture::new().await;
        f.trust();
        let mut child = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "xcode_authority_recovery::tests::sqlite_snapshot_child",
                "--nocapture",
            ])
            .env("CW_AUTHORITY_FIXTURE_DATABASE", &f.database)
            .env("CW_AUTHORITY_FIXTURE_ACTION", "hold")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        let mut output = std::io::BufReader::new(child.stdout.take().unwrap());
        let mut line = String::new();
        loop {
            line.clear();
            assert!(
                output.read_line(&mut line).unwrap() > 0,
                "fixture peer exited before holding transaction"
            );
            if line.trim() == "PEER_READY" {
                break;
            }
        }
        // Fingerprinting closes parent DB fds; it cannot release the separate
        // child's POSIX SQLite lock (unlike an in-process peer fixture).
        let before = source_fingerprint(&f.root);
        let result = f.run(&XcodeAdminCommand::AuthorityInspect {}).await;
        let after = source_fingerprint(&f.root);
        child.stdin.take().unwrap().write_all(b"release\n").unwrap();
        assert!(child.wait().unwrap().success());
        let error = result
            .err()
            .expect("active SQLite transaction must block inspection");
        assert!(error.to_string().contains("snapshot_busy_or_unsupported"));
        assert_eq!(after, before);
        assert!(f.run(&XcodeAdminCommand::AuthorityInspect {}).await.is_ok());
    }

    #[tokio::test]
    async fn inspection_includes_committed_orphan_wal_without_touching_source_sidecars() {
        let f = Fixture::new().await;
        f.trust();
        f.unknown().await;
        let original = f.plan().await;
        let main = format!("{:x}", Sha256::digest(fs::read(&f.database).unwrap()));
        sqlite_peer(&f.database, "crash-wal");
        assert_eq!(
            format!("{:x}", Sha256::digest(fs::read(&f.database).unwrap())),
            main
        );
        assert!(sidecar_path(&f.database, "-wal").is_file());
        let before = source_fingerprint(&f.root);
        let reviewed = f.plan().await;
        assert_ne!(
            reviewed, original,
            "the committed WAL row must be part of the plan"
        );
        assert_eq!(source_fingerprint(&f.root), before);
        assert_eq!(f.plan().await, reviewed);
        assert_eq!(source_fingerprint(&f.root), before);
        // Applying the plan to source SQLite sees the same committed WAL state.
        assert_eq!(f.run(&enroll(reviewed)).await.unwrap()["enrolled"], true);
    }

    #[tokio::test]
    async fn inspection_copy_is_private_and_rollback_journal_is_refused() {
        let f = Fixture::new().await;
        let source = SecureNode::open_database(&f.database).unwrap();
        let copy = InspectionCopy::capture(&source).unwrap();
        assert_eq!(
            fs::metadata(copy._directory.path()).unwrap().mode() & 0o7777,
            0o700
        );
        assert_eq!(fs::metadata(&copy.database).unwrap().mode() & 0o7777, 0o600);
        let temporary = copy._directory.path().to_path_buf();
        drop(copy);
        assert!(!temporary.exists());
        let journal = sidecar_path(&f.database, "-journal");
        fs::write(&journal, b"unreviewed rollback journal").unwrap();
        let before = source_fingerprint(&f.root);
        assert!(InspectionCopy::capture(&source).is_err());
        assert_eq!(source_fingerprint(&f.root), before);
    }

    #[tokio::test]
    async fn stopped_inspection_preserves_every_source_file() {
        let f = Fixture::new().await;
        f.trust();
        f.unknown().await;
        f.pool.close().await;
        let before = source_fingerprint(&f.root);
        let result = f
            .run(&XcodeAdminCommand::AuthorityInspect {})
            .await
            .unwrap();
        assert!(result["plan"]["plan_digest"].is_string());
        assert_eq!(
            source_fingerprint(&f.root),
            before,
            "inspection must not create WAL/SHM or alter source bytes/metadata"
        );
    }

    #[tokio::test]
    async fn inspection_missing_trust_does_not_create_a_store() {
        let f = Fixture::new().await;
        let before = f.journal().await;
        let db_before = fs::read(&f.database).unwrap();
        let names_before: Vec<_> = fs::read_dir(&f.root)
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        let result = f
            .run(&XcodeAdminCommand::AuthorityInspect {})
            .await
            .unwrap();
        assert_eq!(result["inventory"]["trust"]["files"], serde_json::json!([]));
        assert_eq!(
            result["blockers"],
            serde_json::json!(["project_trust_store_missing"])
        );
        assert!(!f.root.join("trust").exists());
        assert!(!f.root.join("fixture.sqlite.lock").exists());
        assert_eq!(fs::read(&f.database).unwrap(), db_before);
        let names_after: Vec<_> = fs::read_dir(&f.root)
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        assert_eq!(names_after, names_before);
        assert!(f.run(&enroll(f.plan().await)).await.is_err());
        assert!(!f.root.join("trust").exists());
        assert_eq!(f.journal().await, before);
    }
    #[tokio::test]
    async fn enrollment_keeps_wal_writer_excluded_through_native_authority_probes() {
        fn writer_probe(path: &Path) {
            // Native observation also opens/closes this inode. The SQLite WAL
            // write lock must continue excluding another process afterward.
            drop(File::open(path).unwrap());
            sqlite_peer(path, "write-busy");
        }
        let f = Fixture::new().await;
        f.trust();
        f.unknown().await;
        let before = f.journal().await;
        let command = enroll(f.plan().await);
        let result = execute_at(
            &command,
            &operator(),
            &f.url,
            &f.root.join("trust"),
            &AuthorityLocation::FixtureWithMaintenanceProbe {
                root: f.root.join("authority"),
                probe: writer_probe,
            },
            StopProof::Fixture(stopped()),
        )
        .await
        .unwrap();
        assert_eq!(result["enrolled"], true);
        assert_eq!(f.journal().await, before);
    }

    #[tokio::test]
    async fn enrollment_rejects_journal_mode_drift_after_inspection() {
        let f = Fixture::new().await;
        f.trust();
        let plan = f.plan().await;
        let original = fs::read(f.root.join("authority/authority.json")).unwrap();
        let mut connection = f.connection().await;
        let mode: String = sqlx::query_scalar("PRAGMA journal_mode = DELETE")
            .fetch_one(&mut connection)
            .await
            .unwrap();
        assert_eq!(mode, "delete");
        connection.close().await.unwrap();
        assert!(
            f.run(&enroll(plan)).await.is_err(),
            "changing WAL to rollback mode must invalidate enrollment"
        );
        assert_eq!(
            fs::read(f.root.join("authority/authority.json")).unwrap(),
            original
        );
    }

    #[tokio::test]
    async fn enrollment_preserves_unknown_holds_all_journal_bytes_and_legacy_grants() {
        let f = Fixture::new().await;
        f.trust();
        f.unknown().await;
        let journal = f.journal().await;
        let grant = fs::read(f.root.join("trust/legacy-grant.json")).unwrap();
        let original = fs::read(f.root.join("authority/authority.json")).unwrap();
        let result = f.run(&enroll(f.plan().await)).await.unwrap();
        assert_eq!(result["enrolled"], true);
        let backup = result["receipt"]["backup_file"].as_str().unwrap();
        assert_eq!(
            fs::read(f.root.join("authority").join(backup)).unwrap(),
            original
        );
        assert_eq!(f.journal().await, journal);
        assert_eq!(
            fs::read(f.root.join("trust/legacy-grant.json")).unwrap(),
            grant
        );
        assert_ne!(
            fs::read(f.root.join("authority/authority.json")).unwrap(),
            original
        );
        assert!(
            acp::xcode_coordinator::FixtureJournalAuthority::open_existing(
                &f.root.join("authority"),
                &f.database
            )
            .unwrap()
            .check()
            .is_ok()
        );
    }
    #[tokio::test]
    async fn stale_journal_trust_and_database_replacement_reject_without_publication() {
        let f = Fixture::new().await;
        f.trust();
        let original = fs::read(f.root.join("authority/authority.json")).unwrap();
        let plan = f.plan().await;
        f.unknown().await;
        assert!(f.run(&enroll(plan)).await.is_err());
        let plan = f.plan().await;
        f.file("trust/legacy-grant.json", b"changed");
        assert!(f.run(&enroll(plan)).await.is_err());
        let plan = f.plan().await;
        let authority = f.root.join("authority/authority.json");
        let mut record: serde_json::Value = serde_json::from_slice(&original).unwrap();
        record["database"]["device"] = serde_json::json!(999);
        f.file(
            "authority/authority.json",
            &serde_json::to_vec(&record).unwrap(),
        );
        assert!(f.run(&enroll(plan)).await.is_err());
        fs::write(&authority, &original).unwrap();
        let plan = f.plan().await;
        // SQLite's own backup produces a consistent replacement, not raw WAL copying.
        let mut connection = f.connection().await;
        sqlx::query("VACUUM INTO ?")
            .bind(f.root.join("replacement.sqlite").to_str().unwrap())
            .execute(&mut connection)
            .await
            .unwrap();
        connection.close().await.unwrap();
        fs::set_permissions(
            f.root.join("replacement.sqlite"),
            fs::Permissions::from_mode(0o600),
        )
        .unwrap();
        f.pool.close().await;
        fs::rename(f.root.join("replacement.sqlite"), &f.database).unwrap();
        assert!(f.run(&enroll(plan)).await.is_err());
        assert_eq!(
            fs::read(f.root.join("authority/authority.json")).unwrap(),
            original
        );
    }
    #[tokio::test]
    async fn recovery_requires_both_global_capabilities_and_no_run_scope() {
        let f = Fixture::new().await;
        f.trust();
        let plan = f.plan().await;
        for capability in [
            domain::CapabilityToolId::XcodeGlobalAdmin,
            domain::CapabilityToolId::XcodeProjectTrust,
        ] {
            let mut principal = operator();
            principal.tool_capabilities.remove(&capability);
            assert!(f
                .run_as(&enroll(plan.clone()), &principal, stopped())
                .await
                .is_err());
            assert!(f
                .run_as(
                    &XcodeAdminCommand::AuthorityInspect {},
                    &principal,
                    stopped()
                )
                .await
                .is_err());
        }
        let mut principal = operator();
        principal.run_scope = Some(vec![]);
        assert!(f
            .run_as(&enroll(plan), &principal, stopped())
            .await
            .is_err());
    }
    #[tokio::test]
    async fn invalid_confirmation_digest_and_nonoperator_fail_before_creating_locks() {
        let f = Fixture::new().await;
        f.trust();
        let mut command = enroll(f.plan().await);
        if let XcodeAdminCommand::AuthorityEnroll {
            confirm_stopped_writers,
            ..
        } = &mut command
        {
            *confirm_stopped_writers = false;
        }
        assert!(f.run(&command).await.is_err());
        assert!(f.run(&enroll("a".repeat(63))).await.is_err());
        let mut principal = operator();
        principal.class = domain::PrincipalClass::Agent;
        assert!(f
            .run_as(
                &XcodeAdminCommand::AuthorityInspect {},
                &principal,
                stopped()
            )
            .await
            .is_err());
        assert!(!f.root.join("fixture.sqlite.lock").exists());
    }
    #[tokio::test]
    async fn enrollment_refuses_live_writer_processes_and_lock_contention() {
        let f = Fixture::new().await;
        f.trust();
        let plan = f.plan().await;
        let process = format!(
            "{}99 {} /Applications/Chainworks Forge.app/Contents/MacOS/Chainworks Forge\n",
            stopped(),
            unsafe { libc::geteuid() }
        );
        assert!(f
            .run_as(&enroll(plan.clone()), &operator(), process)
            .await
            .is_err());
        let singleton = LegacyLock::acquire(&f.database).unwrap();
        assert!(f.run(&enroll(plan.clone())).await.is_err());
        drop(singleton);
        let trust = TrustInventory::open(&f.root.join("trust"), true).unwrap();
        assert!(f.run(&enroll(plan.clone())).await.is_err());
        drop(trust);
        let coordinator = acp::xcode_coordinator::FixtureJournalAuthority::open_existing(
            &f.root.join("authority"),
            &f.database,
        )
        .unwrap();
        assert!(f.run(&enroll(plan.clone())).await.is_err());
        drop(coordinator);
        let mut connection = f.connection().await;
        let tx = connection.begin_with("BEGIN IMMEDIATE").await.unwrap();
        assert!(f.run(&enroll(plan)).await.is_err());
        tx.rollback().await.unwrap();
        connection.close().await.unwrap();
    }
    #[tokio::test]
    async fn insecure_trust_inventory_and_missing_schema_fail_closed() {
        let f = Fixture::new().await;
        f.trust();
        symlink(&f.database, f.root.join("trust/alias")).unwrap();
        assert!(f
            .run(&XcodeAdminCommand::AuthorityInspect {})
            .await
            .is_err());
        fs::remove_file(f.root.join("trust/alias")).unwrap();
        let mut connection = f.connection().await;
        sqlx::query("DROP TABLE xcode_project_holds")
            .execute(&mut connection)
            .await
            .unwrap();
        assert!(f
            .run(&XcodeAdminCommand::AuthorityInspect {})
            .await
            .is_err());
    }
    #[tokio::test]
    async fn inspection_of_missing_database_creates_no_database_or_locks() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let url = format!(
            "sqlite://{}?mode=rwc",
            root.join("missing.sqlite").display()
        );
        let result = execute_at(
            &XcodeAdminCommand::AuthorityInspect {},
            &operator(),
            &url,
            &root.join("trust"),
            &AuthorityLocation::Fixture(root.join("authority")),
            StopProof::Fixture(stopped()),
        )
        .await;
        assert!(result.is_err());
        assert_eq!(fs::read_dir(root).unwrap().count(), 0);
    }
}
