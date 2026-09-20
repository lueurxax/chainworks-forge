//! Explicit operator CLI. Authentication and daemon startup wiring stay in main.

use acp::{
    xcode_coordinator::{CoordinatorError, JournalAuthority},
    xcode_headless_host::TrustedProject,
    xcode_project_trust::ProjectTrustStore,
};
use anyhow::{bail, ensure, Result};
use auth::Principal;
use domain::{
    execution_root::ResolvedExecutionRoot,
    xcode_effect::{AttemptRevision, Reconciliation},
};
use engine::xcode_effect_admin::XcodeEffectAdmin;
use serde::Deserialize;
use std::{
    fs::{File, OpenOptions},
    io::Read,
    os::{
        fd::AsRawFd,
        unix::fs::{MetadataExt, OpenOptionsExt},
    },
    path::{Path, PathBuf},
    process::Stdio,
    time::{Duration, Instant},
};
use tokio::io::AsyncReadExt;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
#[serde(tag = "operation", rename_all = "kebab-case", deny_unknown_fields)]
pub enum XcodeAdminCommand {
    BootstrapAuthority {
        confirm_legacy_shutdown: bool,
    },
    TrustGrant {
        run_id: Uuid,
        root: ResolvedExecutionRoot,
        selector: Option<String>,
    },
    TrustRevoke {
        run_id: Uuid,
        root: ResolvedExecutionRoot,
        selector: Option<String>,
    },
    EffectsList {
        run_id: Uuid,
        owner: String,
        limit: u32,
        cursor: Option<String>,
    },
    EffectsGet {
        run_id: Uuid,
        owner: String,
        attempt_id: Uuid,
    },
    EffectsReconcile {
        run_id: Uuid,
        owner: String,
        #[serde(deserialize_with = "deserialize_revision")]
        attempt: AttemptRevision,
        evidence: Reconciliation,
    },
}

fn deserialize_revision<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> std::result::Result<AttemptRevision, D::Error> {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct WireRevision {
        attempt_id: Uuid,
        revision: i64,
    }
    let wire = WireRevision::deserialize(deserializer)?;
    Ok(AttemptRevision {
        attempt_id: wire.attempt_id,
        revision: wire.revision,
    })
}

/// `args` excludes argv[0]. No bearer token is accepted as a command argument.
pub fn parse_cli(args: &[String]) -> Result<Option<XcodeAdminCommand>> {
    if !args.iter().any(|arg| arg == "--xcode-admin") {
        return Ok(None);
    }
    ensure!(
        args.len() == 3 && args[0] == "--xcode-admin",
        "xcode_admin_invalid_arguments"
    );
    ensure!(
        matches!(
            args[1].as_str(),
            "bootstrap-authority"
                | "trust-grant"
                | "trust-revoke"
                | "effects-list"
                | "effects-get"
                | "effects-reconcile"
        ),
        "xcode_admin_invalid_action"
    );
    // Never include user arguments, request bytes or parse errors in CLI errors.
    let parse = || -> Result<XcodeAdminCommand> {
        let file = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC)
            .open(&args[2])?;
        let metadata = file.metadata()?;
        ensure!(
            metadata.is_file() && metadata.len() <= 65536,
            "invalid_request"
        );
        let mut bytes = Vec::new();
        file.take(65537).read_to_end(&mut bytes)?;
        ensure!(bytes.len() <= 65536, "invalid_request");
        let mut value = domain::xcode_contract::parse_unique_json(&bytes)?;
        let object = value
            .as_object_mut()
            .ok_or_else(|| anyhow::anyhow!("invalid_request"))?;
        ensure!(!object.contains_key("operation"), "invalid_request");
        object.insert("operation".into(), args[1].clone().into());
        Ok(serde_json::from_value(value)?)
    };
    parse()
        .map(Some)
        .map_err(|_| anyhow::anyhow!("xcode_admin_invalid_request"))
}

pub async fn execute(
    command: XcodeAdminCommand,
    principal: &Principal,
    trust: &ProjectTrustStore,
    effects: &XcodeEffectAdmin,
) -> Result<serde_json::Value> {
    match command {
        XcodeAdminCommand::BootstrapAuthority { .. } => {
            bail!("xcode_bootstrap_requires_stop_proof")
        }
        XcodeAdminCommand::TrustGrant {
            run_id,
            root,
            selector,
        } => {
            authorize(
                principal,
                domain::CapabilityToolId::XcodeProjectTrust,
                Some(run_id),
            )?;
            let project =
                TrustedProject::resolve(root, selector.as_deref(), unsafe { libc::geteuid() })?;
            effects
                .authorize_trust_target(principal, run_id, &project)
                .await?;
            let digest = trust.grant(&project, &principal.id)?;
            Ok(serde_json::json!({"granted": true, "trust_digest": digest}))
        }
        XcodeAdminCommand::TrustRevoke {
            run_id,
            root,
            selector,
        } => {
            authorize(
                principal,
                domain::CapabilityToolId::XcodeProjectTrust,
                Some(run_id),
            )?;
            let project =
                TrustedProject::resolve(root, selector.as_deref(), unsafe { libc::geteuid() })?;
            effects
                .authorize_trust_target(principal, run_id, &project)
                .await?;
            trust.revoke(&project, &principal.id)?;
            Ok(serde_json::json!({"revoked": true}))
        }
        XcodeAdminCommand::EffectsList {
            run_id,
            owner,
            limit,
            cursor,
        } => {
            let page = effects
                .list(principal, run_id, &owner, limit, cursor.as_deref())
                .await?;
            Ok(serde_json::json!({"items": page.items, "next_cursor": page.next_cursor}))
        }
        XcodeAdminCommand::EffectsGet {
            run_id,
            owner,
            attempt_id,
        } => Ok(serde_json::to_value(
            effects.get(principal, run_id, &owner, attempt_id).await?,
        )?),
        XcodeAdminCommand::EffectsReconcile {
            run_id,
            owner,
            attempt,
            evidence,
        } => Ok(serde_json::to_value(
            effects
                .reconcile(principal, run_id, &owner, attempt, &evidence)
                .await?,
        )?),
    }
}

/// Explicit stopped-daemon transition only. The caller opens an existing DB
/// without migration and authenticates against the existing principal table.
pub async fn bootstrap_authority(
    command: &XcodeAdminCommand,
    principal: &Principal,
    pool: &sqlx::SqlitePool,
) -> Result<serde_json::Value> {
    authorize(principal, domain::CapabilityToolId::XcodeProjectTrust, None)?;
    ensure!(
        matches!(
            command,
            XcodeAdminCommand::BootstrapAuthority {
                confirm_legacy_shutdown: true
            }
        ),
        "legacy_transition_unproven"
    );
    let path: String =
        sqlx::query_scalar("SELECT file FROM pragma_database_list WHERE name = 'main'")
            .fetch_one(pool)
            .await?;
    let database = PathBuf::from(path).canonicalize()?;
    let legacy_lock = LegacyLock::acquire(&database)?;
    // Keep one read snapshot and the legacy singleton lock alive until the
    // authority record is durable. No migration or journal mutation occurs.
    let mut snapshot = pool.begin().await?;
    require_settled_effects(&mut snapshot).await?;
    let database_identity = std::fs::metadata(&database)?;
    let inventory = process_inventory().await?;
    verify_legacy_process_inventory(&inventory, unsafe { libc::geteuid() }, std::process::id())?;
    let proved_at = Instant::now();
    let authority = JournalAuthority::open_with_bootstrap_check(&database, |pinned| {
        let checked = || -> Result<()> {
            ensure!(
                proved_at.elapsed() < Duration::from_secs(2),
                "legacy_transition_unproven"
            );
            ensure!(pinned == database, "legacy_transition_unproven");
            legacy_lock.check()?;
            let current = std::fs::metadata(pinned)?;
            ensure!(
                current.dev() == database_identity.dev()
                    && current.ino() == database_identity.ino(),
                "legacy_transition_unproven"
            );
            Ok(())
        };
        checked().map_err(|_| CoordinatorError::LegacyTransitionUnproven)
    })?;
    authority.check()?;
    snapshot.rollback().await?;
    Ok(serde_json::json!({"bootstrapped": true}))
}

fn authorize(
    principal: &Principal,
    capability: domain::CapabilityToolId,
    run: Option<Uuid>,
) -> Result<()> {
    auth::require_xcode_operator(principal, capability, run).map_err(anyhow::Error::msg)
}

async fn require_settled_effects(connection: &mut sqlx::SqliteConnection) -> Result<()> {
    let unresolved: i64 = sqlx::query_scalar(
        "SELECT (SELECT count(*) FROM xcode_project_holds) + \
         (SELECT count(*) FROM xcode_effect_attempts WHERE state IN ('dispatched','unknown')) + \
         (SELECT count(*) FROM side_effects WHERE status NOT IN ('settled','reconciled') \
          AND (status <> 'prepared' OR external_write_attempted <> 0))",
    )
    .fetch_one(connection)
    .await?;
    ensure!(unresolved == 0, "legacy_transition_unproven");
    Ok(())
}

async fn process_inventory() -> Result<String> {
    let read = async {
        let mut child = tokio::process::Command::new("/bin/ps")
            .args(["-axo", "pid=,uid=,comm="])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()?;
        let mut bytes = Vec::new();
        child
            .stdout
            .take()
            .ok_or_else(|| anyhow::anyhow!("legacy_transition_unproven"))?
            .take(4 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .await?;
        ensure!(bytes.len() <= 4 * 1024 * 1024, "legacy_transition_unproven");
        ensure!(child.wait().await?.success(), "legacy_transition_unproven");
        Ok::<_, anyhow::Error>(String::from_utf8(bytes)?)
    };
    tokio::time::timeout(Duration::from_secs(3), read)
        .await
        .map_err(|_| anyhow::anyhow!("legacy_transition_unproven"))?
        .map_err(|_| anyhow::anyhow!("legacy_transition_unproven"))
}

fn verify_legacy_process_inventory(inventory: &str, uid: u32, self_pid: u32) -> Result<()> {
    let mut saw_self = false;
    let mut seen = std::collections::HashSet::new();
    for line in inventory.lines() {
        let (pid, rest) = line
            .trim()
            .split_once(char::is_whitespace)
            .ok_or_else(|| anyhow::anyhow!("legacy_transition_unproven"))?;
        let (owner, executable) = rest
            .trim_start()
            .split_once(char::is_whitespace)
            .ok_or_else(|| anyhow::anyhow!("legacy_transition_unproven"))?;
        let pid: u32 = pid
            .parse()
            .map_err(|_| anyhow::anyhow!("legacy_transition_unproven"))?;
        let owner: u32 = owner
            .parse::<u32>()
            // macOS ps can print system uid_t values as signed integers (e.g. -2).
            .or_else(|_| owner.parse::<i32>().map(|value| value as u32))
            .map_err(|_| anyhow::anyhow!("legacy_transition_unproven"))?;
        ensure!(
            pid != 0 && seen.insert(pid) && !executable.trim().is_empty(),
            "legacy_transition_unproven"
        );
        if pid == self_pid {
            ensure!(owner == uid, "legacy_transition_unproven");
            saw_self = true;
            continue;
        }
        if owner == uid {
            let name = Path::new(executable.trim())
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("");
            ensure!(
                !matches!(
                    name,
                    "control-plane" | "chainworks-forge-daemon" | "Chainworks Forge"
                ),
                "legacy_transition_unproven"
            );
        }
    }
    ensure!(saw_self, "legacy_transition_unproven");
    Ok(())
}

struct LegacyLock {
    path: PathBuf,
    file: File,
}

impl LegacyLock {
    fn acquire(database: &Path) -> Result<Self> {
        let name = database
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| anyhow::anyhow!("legacy_transition_unproven"))?;
        let path = database.with_file_name(format!("{name}.lock"));
        let options = || {
            let mut options = OpenOptions::new();
            options
                .read(true)
                .write(true)
                .mode(0o600)
                .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK);
            options
        };
        let file = match options().create_new(true).open(&path) {
            Ok(file) => file,
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => options().open(&path)?,
            Err(e) => return Err(e.into()),
        };
        let guard = Self { path, file };
        guard.check()?;
        ensure!(
            unsafe { libc::flock(guard.file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } == 0,
            "legacy_transition_unproven"
        );
        guard.check()?;
        Ok(guard)
    }

    fn check(&self) -> Result<()> {
        let named = std::fs::symlink_metadata(&self.path)?;
        let opened = self.file.metadata()?;
        for metadata in [&named, &opened] {
            ensure!(
                metadata.is_file()
                    && metadata.nlink() == 1
                    && metadata.uid() == unsafe { libc::geteuid() }
                    && metadata.mode() & 0o022 == 0,
                "legacy_transition_unproven"
            );
        }
        ensure!(
            named.dev() == opened.dev() && named.ino() == opened.ino(),
            "legacy_transition_unproven"
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn assert_run_scoped_trust_change_denied(grant: bool) {
        use acp::xcode_headless_runtime::HeadlessProjectTrust;
        let dir = tempfile::tempdir().unwrap();
        let directory = dir.path().canonicalize().unwrap();
        let root = directory.join("shared-checkout");
        std::fs::create_dir(&root).unwrap();
        std::fs::create_dir(root.join("App.xcodeproj")).unwrap();
        std::fs::write(root.join("App.xcodeproj/project.pbxproj"), b"fixture").unwrap();
        let pool = db::pool::create_pool(&format!(
            "sqlite://{}?mode=rwc",
            directory.join("fixture.sqlite").display()
        ))
        .await
        .unwrap();
        let run_a = Uuid::new_v4();
        let run_b = Uuid::new_v4();
        let idea = Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO ideas(id,title,body,created_at) VALUES (?, 'fixture', '', ?)")
            .bind(&idea)
            .bind(chrono::Utc::now().to_rfc3339())
            .execute(&pool)
            .await
            .unwrap();
        for run in [run_a, run_b] {
            sqlx::query("INSERT INTO runs(id,idea_id,workflow_id,workflow_title,workspace_root,artifact_root,started_at) VALUES (?, ?, 'fixture', 'fixture', ?, ?, ?)")
                .bind(run.to_string()).bind(&idea).bind(root.to_str().unwrap()).bind(root.to_str().unwrap())
                .bind(chrono::Utc::now().to_rfc3339()).execute(&pool).await.unwrap();
        }
        let root =
            acp::execution_root::resolve_execution_root(root.to_str().unwrap(), None, false, None)
                .unwrap();
        let project = TrustedProject::resolve(root.clone(), Some("App.xcodeproj"), unsafe {
            libc::geteuid()
        })
        .unwrap();
        let trust = ProjectTrustStore::new(directory.join("trust"));
        let prior_digest = trust.grant(&project, "global-operator").unwrap();
        let writer = std::sync::Arc::new(db::writer::DbWriter::new(pool.clone()));
        let effects = XcodeEffectAdmin::new(pool.clone(), writer.clone());
        let mut principal = Principal::new("run-a-operator", domain::PrincipalClass::Operator);
        principal.has_explicit_surface_policies = true;
        principal.tool_capabilities =
            std::collections::BTreeSet::from([domain::CapabilityToolId::XcodeProjectTrust]);
        principal.run_scope = Some(vec![run_a.to_string()]);
        for global_marker in [false, true] {
            if global_marker {
                principal
                    .tool_capabilities
                    .insert(domain::CapabilityToolId::XcodeGlobalAdmin);
            }
            let command = if grant {
                XcodeAdminCommand::TrustGrant {
                    run_id: run_a,
                    root: root.clone(),
                    selector: Some("App.xcodeproj".into()),
                }
            } else {
                XcodeAdminCommand::TrustRevoke {
                    run_id: run_a,
                    root: root.clone(),
                    selector: Some("App.xcodeproj".into()),
                }
            };
            assert!(execute(command, &principal, &trust, &effects)
                .await
                .is_err());
            assert_eq!(trust.check(&project).await.unwrap(), prior_digest);
        }
        writer.shutdown().await;
        pool.close().await;
    }

    #[tokio::test]
    async fn run_scoped_operator_cannot_grant_shared_project_trust() {
        assert_run_scoped_trust_change_denied(true).await;
    }

    #[tokio::test]
    async fn run_scoped_operator_cannot_revoke_shared_project_trust() {
        assert_run_scoped_trust_change_denied(false).await;
    }

    #[tokio::test]
    async fn bootstrap_effect_proof_rejects_uncertainty_and_missing_schema() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().canonicalize().unwrap().join("fixture.sqlite");
        let pool = db::pool::create_pool(&format!("sqlite://{}?mode=rwc", path.display()))
            .await
            .unwrap();
        let mut connection = pool.acquire().await.unwrap();
        require_settled_effects(&mut connection).await.unwrap();
        sqlx::query("INSERT INTO side_effects(id,run_id,stage_execution_id,effect_kind,target_key,idempotency_key,request_fingerprint,status,external_write_attempted,created_at,updated_at) VALUES ('effect','run','stage','build_archive','target','key','fingerprint','executing',1,'fixture','fixture')")
            .execute(&mut *connection).await.unwrap();
        assert!(require_settled_effects(&mut connection).await.is_err());
        sqlx::query("UPDATE side_effects SET status = 'needs_reconciliation'")
            .execute(&mut *connection)
            .await
            .unwrap();
        assert!(require_settled_effects(&mut connection).await.is_err());
        sqlx::query("UPDATE side_effects SET status = 'reconciled'")
            .execute(&mut *connection)
            .await
            .unwrap();
        require_settled_effects(&mut connection).await.unwrap();
        sqlx::query("DROP TABLE xcode_project_holds")
            .execute(&mut *connection)
            .await
            .unwrap();
        assert!(require_settled_effects(&mut connection).await.is_err());
        drop(connection);
        pool.close().await;
    }

    #[test]
    fn legacy_lock_is_exclusive_persistent_and_rejects_insecure_aliases() {
        use std::os::unix::fs::{symlink, PermissionsExt};
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let database = root.join("fixture.sqlite");
        std::fs::write(&database, b"fixture").unwrap();
        let path = root.join("fixture.sqlite.lock");
        let guard = LegacyLock::acquire(&database).unwrap();
        let inode = std::fs::metadata(&path).unwrap().ino();
        assert!(LegacyLock::acquire(&database).is_err());
        drop(guard);
        assert_eq!(std::fs::metadata(&path).unwrap().ino(), inode);
        LegacyLock::acquire(&database).unwrap();
        let saved = root.join("saved-lock");
        std::fs::rename(&path, &saved).unwrap();
        symlink(&saved, &path).unwrap();
        assert!(LegacyLock::acquire(&database).is_err());
        std::fs::remove_file(&path).unwrap();
        std::fs::hard_link(&saved, &path).unwrap();
        assert!(LegacyLock::acquire(&database).is_err());
        std::fs::remove_file(&saved).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o666)).unwrap();
        assert!(LegacyLock::acquire(&database).is_err());
    }

    #[test]
    fn parser_requires_explicit_action_and_rejects_credentials_without_echoing_them() {
        assert!(parse_cli(&[]).unwrap().is_none());
        let secret = "fixture-do-not-echo-secret";
        let error =
            parse_cli(&["--xcode-admin".into(), "--token".into(), secret.into()]).unwrap_err();
        assert!(!format!("{error:#}").contains(secret));
    }

    #[test]
    fn legacy_stop_proof_accepts_signed_system_uid_without_weakening_owner_checks() {
        let inventory =
            "1 0 /sbin/launchd\n42942 -2 /usr/libexec/dhcp6d\n42 501 /fixture/control-plane\n";
        verify_legacy_process_inventory(inventory, 501, 42).unwrap();
        let legacy = format!("{inventory}43 501 /other-db/chainworks-forge-daemon\n");
        assert!(verify_legacy_process_inventory(&legacy, 501, 42).is_err());
        for invalid in ["-4294966795", "4294967296", "unknown"] {
            let invalid = format!("42 501 /fixture/control-plane\n43 {invalid} /system/helper\n");
            assert!(verify_legacy_process_inventory(&invalid, 501, 42).is_err());
        }
        assert!(
            verify_legacy_process_inventory("42 -2 /fixture/control-plane\n", 501, 42).is_err()
        );
    }

    #[test]
    fn legacy_stop_proof_requires_parseable_complete_inventory_without_another_same_uid_daemon() {
        verify_legacy_process_inventory(
            "1 0 /sbin/launchd\n42 501 /fixture/control-plane\n",
            501,
            42,
        )
        .unwrap();
        assert!(verify_legacy_process_inventory(
            "42 501 /fixture/control-plane\n43 501 /other-db/chainworks-forge-daemon\n",
            501,
            42
        )
        .is_err());
        assert!(verify_legacy_process_inventory(
            "43 501 /Applications/Chainworks Forge.app/Contents/MacOS/Chainworks Forge\n",
            501,
            42
        )
        .is_err());
        assert!(verify_legacy_process_inventory("", 501, 42).is_err());
        assert!(verify_legacy_process_inventory("unparseable output", 501, 42).is_err());
    }

    #[test]
    fn parser_reads_bounded_typed_request_and_does_not_accept_body_operation_override() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("request.json");
        let run = Uuid::new_v4();
        std::fs::write(
            &path,
            serde_json::to_vec(&serde_json::json!({
                "run_id": run, "owner": "fixture-owner", "limit": 5
            }))
            .unwrap(),
        )
        .unwrap();
        let args = vec![
            "--xcode-admin".into(),
            "effects-list".into(),
            path.to_str().unwrap().into(),
        ];
        assert!(
            matches!(parse_cli(&args).unwrap(), Some(XcodeAdminCommand::EffectsList { run_id, limit: 5, .. }) if run_id == run)
        );
        std::fs::write(&path, br#"{"operation":"trust-grant"}"#).unwrap();
        assert!(parse_cli(&args).is_err());
        std::fs::write(&path, vec![b' '; 65537]).unwrap();
        assert!(parse_cli(&args).is_err());
    }
}
