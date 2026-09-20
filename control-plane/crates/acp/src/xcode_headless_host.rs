use anyhow::{bail, ensure, Context, Result};
use async_trait::async_trait;
use domain::{execution_root::ResolvedExecutionRoot, xcode_effect::ProjectKey};
use serde::Serialize;
use std::{
    collections::BTreeSet,
    fs,
    path::{Component, Path, PathBuf},
};

const METADATA_LIMIT: usize = 1_048_576;
const DISCOVERY_LIMIT: usize = 4096;
const SERVICE_BUNDLE_ID: &str = "com.apple.dt.mcp-server";

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ServiceGeneration {
    pub uid: u32,
    pub pid: i32,
    pub start_sec: u64,
    pub start_usec: u64,
    pub boot_id: String,
    pub developer_dir: PathBuf,
    pub executable: PathBuf,
    pub bundle_id: String,
    pub build: String,
    pub xcode_build: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct HostSnapshot {
    pub generation: ServiceGeneration,
    pub operator_home: PathBuf,
    pub account_name: String,
    pub darwin_tmpdir: PathBuf,
    pub open_projects: Vec<PathBuf>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct HeadlessServiceStartupPlan {
    pub installation: ServiceGeneration,
    environment: CommandEnvironment,
}

impl HeadlessServiceStartupPlan {
    /// Fixture data only. Native startup accepts only its own exactly pinned plan.
    pub fn from_fixture_snapshot(snapshot: &HostSnapshot) -> Self {
        let mut installation = snapshot.generation.clone();
        installation.pid = 0;
        installation.start_sec = 0;
        installation.start_usec = 0;
        Self {
            environment: CommandEnvironment {
                home: snapshot.operator_home.clone(),
                account: snapshot.account_name.clone(),
                tmpdir: snapshot.darwin_tmpdir.clone(),
                developer: Some(installation.developer_dir.clone()),
            },
            installation,
        }
    }
}

#[async_trait]
pub trait HeadlessHostInspector: Send + Sync {
    async fn inspect(&self) -> Result<HostSnapshot>;

    /// Read-only preparation, called only after project trust admission.
    async fn prepare_service_startup(&self) -> Result<Option<HeadlessServiceStartupPlan>> {
        Ok(None)
    }

    /// The caller must hold the durable cold workspace-open fence. An error or
    /// cancellation is uncertain and must not authorize a fresh startup attempt.
    async fn start_service_after_fence(&self, _plan: HeadlessServiceStartupPlan) -> Result<()> {
        bail!("headless_service_startup_disabled")
    }
}

#[derive(Default)]
pub struct LocalHeadlessHostInspector {
    startup: tokio::sync::Mutex<ServiceStartupSlot>,
}

impl LocalHeadlessHostInspector {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_startup_on_absence() -> Self {
        Self {
            startup: tokio::sync::Mutex::new(ServiceStartupSlot::Available),
        }
    }
}

#[derive(Default)]
enum ServiceStartupSlot {
    #[default]
    Disabled,
    Available,
    Pinned(HeadlessServiceStartupPlan),
    Spent,
}

impl ServiceStartupSlot {
    fn pin(&mut self, plan: Option<HeadlessServiceStartupPlan>) -> Result<()> {
        ensure!(
            matches!(self, Self::Available),
            "headless_service_startup_consumed"
        );
        *self = plan.map(Self::Pinned).unwrap_or(Self::Spent);
        Ok(())
    }

    fn consume(&mut self, plan: &HeadlessServiceStartupPlan) -> Result<()> {
        ensure!(
            matches!(self, Self::Pinned(pinned) if pinned == plan),
            "headless_service_startup_plan_mismatch_or_consumed"
        );
        *self = Self::Spent;
        Ok(())
    }
}

fn validate_startup_status(bytes: &[u8]) -> Result<()> {
    ensure!(bytes.len() <= METADATA_LIMIT, "headless_metadata_size");
    let status =
        domain::xcode_contract::parse_unique_json(bytes).context("headless_status_shape")?;
    for (pointer, required) in [
        ("/permission/enabled", true),
        ("/permission/unsafeAlwaysAllowAllAgents", false),
        ("/running", false),
    ] {
        ensure!(
            status.pointer(pointer).and_then(serde_json::Value::as_bool) == Some(required),
            "headless_service_startup_action_required"
        );
    }
    Ok(())
}

struct ServiceStartupObservation {
    installation: ServiceGeneration,
    generation: Option<ServiceGeneration>,
    status: Vec<u8>,
}

fn startup_status_ready(bytes: &[u8]) -> Result<bool> {
    ensure!(bytes.len() <= METADATA_LIMIT, "headless_metadata_size");
    let status =
        domain::xcode_contract::parse_unique_json(bytes).context("headless_status_shape")?;
    for (pointer, required) in [
        ("/permission/enabled", true),
        ("/permission/unsafeAlwaysAllowAllAgents", false),
    ] {
        ensure!(
            status.pointer(pointer).and_then(serde_json::Value::as_bool) == Some(required),
            "headless_service_startup_action_required"
        );
    }
    let running = status
        .get("running")
        .and_then(serde_json::Value::as_bool)
        .context("headless_status_shape")?;
    if running {
        status_projects(bytes)?;
    }
    Ok(running)
}

async fn await_startup_readiness<F, Fut>(
    expected: &ServiceGeneration,
    deadline: tokio::time::Instant,
    mut observe: F,
) -> Result<()>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<ServiceStartupObservation>>,
{
    let mut observed_generation: Option<ServiceGeneration> = None;
    tokio::time::timeout_at(deadline, async {
        loop {
            ensure!(
                tokio::time::Instant::now() < deadline,
                "headless_service_startup_outcome_unknown"
            );
            let observed = observe().await?;
            ensure!(
                tokio::time::Instant::now() < deadline,
                "headless_service_startup_outcome_unknown"
            );
            confirm_generation(expected, &observed.installation)?;
            let ready = startup_status_ready(&observed.status)?;
            if let Some(generation) = observed.generation {
                let generation = select_service(expected, &[generation])?;
                if let Some(previous) = &observed_generation {
                    confirm_generation(previous, &generation)?;
                }
                observed_generation = Some(generation);
                if ready {
                    return Ok::<_, anyhow::Error>(());
                }
            } else {
                ensure!(
                    observed_generation.is_none(),
                    "headless_service_generation_changed"
                );
            }
            tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        }
    })
    .await
    .context("headless_service_startup_outcome_unknown")?
}

#[async_trait]
impl HeadlessHostInspector for LocalHeadlessHostInspector {
    async fn inspect(&self) -> Result<HostSnapshot> {
        #[cfg(target_os = "macos")]
        {
            let snapshot = native::inspect().await?;
            let mut slot = self.startup.lock().await;
            if matches!(
                *slot,
                ServiceStartupSlot::Available | ServiceStartupSlot::Pinned(_)
            ) {
                *slot = ServiceStartupSlot::Spent;
            }
            Ok(snapshot)
        }
        #[cfg(not(target_os = "macos"))]
        {
            bail!("headless_host_unsupported")
        }
    }

    async fn prepare_service_startup(&self) -> Result<Option<HeadlessServiceStartupPlan>> {
        let mut slot = self.startup.lock().await;
        match &*slot {
            ServiceStartupSlot::Disabled | ServiceStartupSlot::Spent => return Ok(None),
            ServiceStartupSlot::Pinned(plan) => return Ok(Some(plan.clone())),
            ServiceStartupSlot::Available => {}
        }
        // Cancellation during probing consumes admission too; it never arms a retry.
        let mut pending = std::mem::replace(&mut *slot, ServiceStartupSlot::Spent);
        #[cfg(target_os = "macos")]
        {
            let plan = native::prepare_startup().await?;
            pending.pin(plan.clone())?;
            *slot = pending;
            Ok(plan)
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = &mut pending;
            bail!("headless_host_unsupported")
        }
    }

    async fn start_service_after_fence(&self, plan: HeadlessServiceStartupPlan) -> Result<()> {
        self.startup.lock().await.consume(&plan)?;
        #[cfg(target_os = "macos")]
        {
            native::start_after_fence(&plan).await
        }
        #[cfg(not(target_os = "macos"))]
        {
            bail!("headless_host_unsupported")
        }
    }
}

/// An ordinary routing guard, not hostile-filesystem confinement or a grant.
#[derive(Clone, Debug)]
pub struct TrustedProject {
    root: ResolvedExecutionRoot,
    key: ProjectKey,
}

impl TrustedProject {
    pub fn resolve(root: ResolvedExecutionRoot, selector: Option<&str>, uid: u32) -> Result<Self> {
        crate::execution_root::revalidate_execution_root(&root)?;
        let effective = Path::new(&root.effective);
        let candidate = match selector {
            Some(selector) => {
                ensure!(
                    !selector.is_empty() && !selector.chars().any(char::is_control),
                    "headless_project_selector"
                );
                let relative = if let Some(workspace) = selector.strip_prefix("workspace:") {
                    let path = Path::new(workspace);
                    if path.is_absolute() {
                        // Map only the exact repository-relative legacy spelling. Never
                        // inspect/select the original checkout when a worktree is active.
                        path.strip_prefix(effective)
                            .or_else(|_| path.strip_prefix(Path::new(&root.repository)))
                            .context("headless_project_outside_root")?
                    } else {
                        ensure!(!workspace.contains(':'), "headless_project_selector");
                        path
                    }
                } else {
                    ensure!(
                        !selector.contains(':') && !Path::new(selector).is_absolute(),
                        "headless_project_selector"
                    );
                    Path::new(selector)
                };
                ensure!(
                    !relative.as_os_str().is_empty()
                        && relative
                            .components()
                            .all(|c| matches!(c, Component::Normal(_)))
                        && !selector.split('/').any(|part| part == "." || part == ".."),
                    "headless_project_selector"
                );
                effective.join(relative)
            }
            None => {
                let mut candidates = Vec::new();
                for (index, entry) in fs::read_dir(effective)
                    .context("headless_project_discovery")?
                    .enumerate()
                {
                    ensure!(index < DISCOVERY_LIMIT, "headless_project_discovery_limit");
                    let path = entry?.path();
                    if package_marker(&path).is_some() {
                        candidates.push(path);
                    }
                }
                // Nested bundles do not participate. Sibling workspace/project
                // pairs remain ambiguous: guessing membership would misroute work.
                ensure!(
                    candidates.len() == 1,
                    "headless_project_missing_or_ambiguous"
                );
                candidates.remove(0)
            }
        };
        let path = canonical_project(&candidate)?;
        ensure!(
            path != effective && path.starts_with(effective),
            "headless_project_outside_root"
        );
        let (device, inode) = file_identity(&path)?;
        let key = ProjectKey {
            version: 1,
            uid,
            canonical_path: path.to_str().context("headless_project_not_utf8")?.into(),
            device,
            inode,
        };
        key.validate()?;
        let project = Self { root, key };
        project.revalidate()?;
        Ok(project)
    }

    pub fn root(&self) -> &ResolvedExecutionRoot {
        &self.root
    }

    pub fn key(&self) -> &ProjectKey {
        &self.key
    }

    pub fn revalidate(&self) -> Result<()> {
        crate::execution_root::revalidate_execution_root(&self.root)?;
        self.key.validate()?;
        let path = Path::new(&self.key.canonical_path);
        ensure!(
            canonical_project(path)? == path && path.starts_with(&self.root.effective),
            "headless_project_changed"
        );
        let (device, inode) = file_identity(path)?;
        ensure!(
            device == self.key.device && inode == self.key.inode,
            "headless_project_changed"
        );
        Ok(())
    }
}

fn package_marker(path: &Path) -> Option<&'static str> {
    match path.extension()?.to_str()? {
        "xcodeproj" => Some("project.pbxproj"),
        "xcworkspace" => Some("contents.xcworkspacedata"),
        _ => None,
    }
}

fn canonical_project(path: &Path) -> Result<PathBuf> {
    let marker = package_marker(path).context("headless_project_kind")?;
    let canonical = path.canonicalize().context("headless_project_missing")?;
    ensure!(
        canonical.is_dir() && package_marker(&canonical) == Some(marker),
        "headless_project_malformed"
    );
    // Project configuration may be edited normally, but must remain a regular
    // file in this bundle, never a link to an unrelated checkout.
    ensure!(
        fs::symlink_metadata(canonical.join(marker))?
            .file_type()
            .is_file(),
        "headless_project_malformed"
    );
    Ok(canonical)
}

#[cfg(unix)]
fn file_identity(path: &Path) -> Result<(String, String)> {
    use std::os::unix::fs::MetadataExt;
    let metadata = fs::metadata(path)?;
    ensure!(metadata.is_dir(), "headless_project_malformed");
    Ok((metadata.dev().to_string(), metadata.ino().to_string()))
}

#[cfg(not(unix))]
fn file_identity(_path: &Path) -> Result<(String, String)> {
    bail!("headless_project_identity_unsupported")
}

fn status_projects(bytes: &[u8]) -> Result<Vec<PathBuf>> {
    use serde_json::Value;
    ensure!(bytes.len() <= METADATA_LIMIT, "headless_metadata_size");
    let status =
        domain::xcode_contract::parse_unique_json(bytes).context("headless_status_shape")?;
    for (pointer, required) in [
        ("/permission/enabled", true),
        ("/running", true),
        ("/permission/unsafeAlwaysAllowAllAgents", false),
    ] {
        let actual = status
            .pointer(pointer)
            .and_then(Value::as_bool)
            .context("headless_status_shape")?;
        ensure!(actual == required, "headless_host_action_required: service must be enabled/running with unsafe allow-all disabled");
    }
    let entries = status
        .get("openWorkspaces")
        .and_then(Value::as_array)
        .context("headless_status_shape")?;
    ensure!(
        entries.len() <= DISCOVERY_LIMIT,
        "headless_status_workspace_limit"
    );
    let mut projects = BTreeSet::new();
    for entry in entries {
        let object = entry
            .as_object()
            .context("headless_status_workspace_shape")?;
        ensure!(object.len() == 3, "headless_status_workspace_shape");
        let text = |key| {
            object
                .get(key)
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty() && !s.chars().any(char::is_control))
                .context("headless_status_workspace_shape")
        };
        let path = Path::new(text("path")?);
        text("displayName")?;
        // Only the observed string-valued schema is admitted; no invented ID or
        // nullable-scheme compatibility fallback.
        let scheme = object
            .get("activeSchemeName")
            .context("headless_status_workspace_shape")?;
        ensure!(
            scheme
                .as_str()
                .is_some_and(|s| !s.chars().any(char::is_control)),
            "headless_status_workspace_shape"
        );
        ensure!(path.is_absolute(), "headless_status_workspace_path");
        let canonical = canonical_project(path)?;
        ensure!(
            projects.insert(canonical),
            "headless_status_workspace_ambiguous"
        );
    }
    Ok(projects.into_iter().collect())
}

fn select_service(
    expected: &ServiceGeneration,
    candidates: &[ServiceGeneration],
) -> Result<ServiceGeneration> {
    let matching: Vec<_> = candidates
        .iter()
        .filter(|candidate| {
            candidate.uid == expected.uid && candidate.executable == expected.executable
        })
        .collect();
    ensure!(!matching.is_empty(), "headless_service_action_required: exact installed Xcode Service is not running for this user; automatic startup is disabled");
    ensure!(matching.len() == 1, "headless_service_ambiguous");
    let candidate = matching[0];
    ensure!(
        candidate.developer_dir == expected.developer_dir
            && candidate.bundle_id == expected.bundle_id
            && candidate.bundle_id == SERVICE_BUNDLE_ID
            && candidate.build == expected.build
            && candidate.xcode_build == expected.xcode_build
            && candidate.boot_id == expected.boot_id
            && uuid::Uuid::parse_str(&candidate.boot_id).is_ok()
            && candidate.pid > 0
            && candidate.start_sec > 0
            && candidate.start_usec < 1_000_000,
        "headless_service_identity_unknown"
    );
    Ok(candidate.clone())
}

fn confirm_generation(before: &ServiceGeneration, after: &ServiceGeneration) -> Result<()> {
    ensure!(before == after, "headless_service_generation_changed");
    Ok(())
}

fn tested_build(xcode: &str, bundle: &str, source: &str) -> Result<String> {
    ensure!(
        xcode == "27A266a" && bundle == "1.0" && source == "25317000000000000",
        "headless_unsupported_installation: update the tested contract before admission"
    );
    Ok(format!("{bundle}/{source}"))
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
struct CommandEnvironment {
    home: PathBuf,
    account: String,
    tmpdir: PathBuf,
    developer: Option<PathBuf>,
}

async fn metadata(
    environment: &CommandEnvironment,
    path: &Path,
    args: &[&std::ffi::OsStr],
) -> Result<(bool, Vec<u8>, Vec<u8>)> {
    use std::{
        process::Stdio,
        sync::atomic::{AtomicUsize, Ordering},
        time::Duration,
    };
    use tokio::io::{AsyncRead, AsyncReadExt};

    async fn drain(mut pipe: impl AsyncRead + Unpin, size: &AtomicUsize) -> Result<Vec<u8>> {
        let mut output = Vec::new();
        let mut block = [0; 8192];
        loop {
            let n = pipe.read(&mut block).await?;
            if n == 0 {
                return Ok(output);
            }
            ensure!(
                size.fetch_add(n, Ordering::Relaxed) + n <= METADATA_LIMIT,
                "headless_metadata_size"
            );
            output.extend_from_slice(&block[..n]);
        }
    }

    struct ProcessGroup(Option<u32>);
    impl Drop for ProcessGroup {
        fn drop(&mut self) {
            #[cfg(unix)]
            if let Some(pid) = self.0 {
                crate::transport::signal_process_group(pid, libc::SIGKILL);
            }
        }
    }

    ensure!(path.is_absolute(), "headless_metadata_command_path");
    let mut command = tokio::process::Command::new(path);
    command
        .env_clear()
        .env("PATH", "/usr/bin:/bin:/usr/sbin:/sbin")
        .env("HOME", &environment.home)
        .env("USER", &environment.account)
        .env("LOGNAME", &environment.account)
        .env("TMPDIR", &environment.tmpdir)
        .current_dir("/")
        .kill_on_drop(true)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(developer) = &environment.developer {
        command.env("DEVELOPER_DIR", developer);
    }
    crate::transport::isolate_process_group(&mut command);
    let mut child = command.spawn().context("headless_metadata_spawn")?;
    let mut group = ProcessGroup(Some(child.id().context("headless_metadata_pid")?));
    let stdout = child.stdout.take().context("headless_metadata_stdout")?;
    let stderr = child.stderr.take().context("headless_metadata_stderr")?;
    let size = AtomicUsize::new(0);
    // One absolute deadline covers both drains and exit; the shared byte budget
    // also bounds a noisy stderr independently of whether stdout makes progress.
    let result = tokio::time::timeout(Duration::from_secs(5), async {
        let (out, err) = tokio::try_join!(drain(stdout, &size), drain(stderr, &size))?;
        // Retain the unreaped leader while descendants hold pipes. Once wait
        // completes, immediately disarm so a recycled PGID cannot be signaled.
        let status = child.wait().await?;
        group.0 = None;
        Ok::<_, anyhow::Error>((status.success(), out, err))
    })
    .await;
    drop(group);
    match result {
        Ok(Ok(output)) => Ok(output),
        error => {
            let _ = child.start_kill();
            let _ = tokio::time::timeout(Duration::from_secs(1), child.wait()).await;
            match error {
                Ok(Err(error)) => Err(error),
                _ => bail!("headless_metadata_timeout"),
            }
        }
    }
}

#[cfg(target_os = "macos")]
mod native {
    use super::*;
    use std::{
        ffi::{CStr, OsStr},
        mem,
    };

    fn account_environment() -> Result<CommandEnvironment> {
        let mut record = mem::MaybeUninit::<libc::passwd>::zeroed();
        let mut buffer = vec![0i8; 65_536];
        let mut found = std::ptr::null_mut();
        // passwd string pointers are valid only while this scratch buffer lives.
        let (home, account) = unsafe {
            ensure!(
                libc::getuid() == libc::geteuid(),
                "headless_elevated_identity"
            );
            ensure!(
                libc::getpwuid_r(
                    libc::geteuid(),
                    record.as_mut_ptr(),
                    buffer.as_mut_ptr(),
                    buffer.len(),
                    &mut found
                ) == 0
                    && !found.is_null(),
                "headless_account_unknown"
            );
            let record = record.assume_init();
            ensure!(
                !record.pw_dir.is_null() && !record.pw_name.is_null(),
                "headless_account_unknown"
            );
            (
                PathBuf::from(CStr::from_ptr(record.pw_dir).to_str()?),
                CStr::from_ptr(record.pw_name).to_str()?.to_owned(),
            )
        };
        ensure!(
            home.is_absolute() && !account.is_empty() && !account.chars().any(char::is_control),
            "headless_account_unknown"
        );
        let home = home.canonicalize().context("headless_account_home")?;
        ensure!(home.is_dir(), "headless_account_home");
        let mut tmp = vec![0u8; 4096];
        let length = unsafe {
            libc::confstr(
                libc::_CS_DARWIN_USER_TEMP_DIR,
                tmp.as_mut_ptr().cast(),
                tmp.len(),
            )
        };
        ensure!(length > 1 && length <= tmp.len(), "headless_darwin_tmpdir");
        let tmpdir = PathBuf::from(CStr::from_bytes_with_nul(&tmp[..length])?.to_str()?);
        ensure!(tmpdir.is_absolute(), "headless_darwin_tmpdir");
        let tmpdir = tmpdir.canonicalize().context("headless_darwin_tmpdir")?;
        ensure!(tmpdir.is_dir(), "headless_darwin_tmpdir");
        Ok(CommandEnvironment {
            home,
            account,
            tmpdir,
            developer: None,
        })
    }

    fn boot_identity() -> Result<String> {
        let mut bytes = [0u8; 128];
        let mut length = bytes.len();
        let result = unsafe {
            libc::sysctlbyname(
                c"kern.bootsessionuuid".as_ptr(),
                bytes.as_mut_ptr().cast(),
                &mut length,
                std::ptr::null_mut(),
                0,
            )
        };
        ensure!(
            result == 0 && length > 1 && length <= bytes.len(),
            "headless_boot_identity_unknown"
        );
        let value = CStr::from_bytes_with_nul(&bytes[..length])?.to_str()?;
        ensure!(
            uuid::Uuid::parse_str(value).is_ok(),
            "headless_boot_identity_unknown"
        );
        Ok(value.to_owned())
    }

    fn process_info(pid: i32) -> Result<libc::proc_bsdinfo> {
        let mut info = mem::MaybeUninit::<libc::proc_bsdinfo>::zeroed();
        let size = mem::size_of::<libc::proc_bsdinfo>();
        unsafe {
            ensure!(
                libc::proc_pidinfo(
                    pid,
                    libc::PROC_PIDTBSDINFO,
                    0,
                    info.as_mut_ptr().cast(),
                    size as i32
                ) == size as i32,
                "headless_process_unreadable"
            );
            Ok(info.assume_init())
        }
    }

    fn process_executable(pid: i32) -> Result<PathBuf> {
        let mut bytes = [0u8; 4096];
        let length =
            unsafe { libc::proc_pidpath(pid, bytes.as_mut_ptr().cast(), bytes.len() as u32) };
        ensure!(
            length > 0 && (length as usize) < bytes.len(),
            "headless_process_unreadable"
        );
        Ok(PathBuf::from(CStr::from_bytes_until_nul(&bytes)?.to_str()?).canonicalize()?)
    }

    fn process(pid: i32, installation: &ServiceGeneration) -> Result<ServiceGeneration> {
        let before = process_info(pid)?;
        let executable = process_executable(pid)?;
        let after = process_info(pid)?;
        ensure!(
            before.pbi_pid == pid as u32
                && after.pbi_pid == before.pbi_pid
                && before.pbi_uid == after.pbi_uid
                && before.pbi_start_tvsec == after.pbi_start_tvsec
                && before.pbi_start_tvusec == after.pbi_start_tvusec,
            "headless_process_changed"
        );
        Ok(ServiceGeneration {
            uid: after.pbi_uid,
            pid,
            start_sec: after.pbi_start_tvsec,
            start_usec: after.pbi_start_tvusec,
            executable,
            ..installation.clone()
        })
    }

    fn services(installation: &ServiceGeneration) -> Result<Vec<ServiceGeneration>> {
        let mut pids = vec![0i32; 65_536];
        let count = unsafe {
            libc::proc_listallpids(
                pids.as_mut_ptr().cast(),
                (pids.len() * mem::size_of::<i32>()) as i32,
            )
        };
        ensure!(
            count > 0 && (count as usize) < pids.len(),
            "headless_process_inventory_unknown"
        );
        // No argv/comm filtering or IDE lookup. Kernel executable/start identity
        // decides whether a process belongs to this installed headless service.
        let mut candidates = Vec::new();
        for &pid in pids[..count as usize].iter().filter(|&&pid| pid > 0) {
            if process_executable(pid).ok().as_ref() != Some(&installation.executable) {
                continue;
            }
            // Once a process is known to be this service, missing/changed start
            // evidence must not hide a second candidate from ambiguity checks.
            let candidate = process(pid, installation)?;
            ensure!(
                candidate.executable == installation.executable,
                "headless_process_changed"
            );
            candidates.push(candidate);
        }
        Ok(candidates)
    }

    async fn plist(environment: &CommandEnvironment, path: &Path, key: &str) -> Result<String> {
        let (ok, out, _) = metadata(
            environment,
            Path::new("/usr/bin/plutil"),
            &[
                OsStr::new("-extract"),
                OsStr::new(key),
                OsStr::new("raw"),
                path.as_os_str(),
            ],
        )
        .await?;
        ensure!(ok, "headless_installation_metadata_unknown");
        let value = std::str::from_utf8(&out)?.trim();
        ensure!(
            !value.is_empty() && !value.chars().any(char::is_control),
            "headless_installation_metadata_unknown"
        );
        Ok(value.to_owned())
    }

    async fn installation(environment: &CommandEnvironment) -> Result<ServiceGeneration> {
        let developer = environment
            .developer
            .as_ref()
            .context("headless_developer_unknown")?;
        ensure!(
            developer.canonicalize()? == *developer,
            "headless_installation_changed"
        );
        let service = developer.join("Library/Xcode/Agents/Xcode Service.app/Contents");
        let info = service.join("Info.plist");
        let bundle_id = plist(environment, &info, "CFBundleIdentifier").await?;
        let bundle_version = plist(environment, &info, "CFBundleVersion").await?;
        let executable_name = plist(environment, &info, "CFBundleExecutable").await?;
        ensure!(
            bundle_id == SERVICE_BUNDLE_ID && executable_name == "Xcode Service",
            "headless_service_bundle_identity"
        );
        let source = plist(environment, &service.join("version.plist"), "SourceVersion").await?;
        let xcode_build = plist(
            environment,
            &developer
                .parent()
                .context("headless_developer_unknown")?
                .join("version.plist"),
            "ProductBuildVersion",
        )
        .await?;
        let build = tested_build(&xcode_build, &bundle_version, &source)?;
        let executable = service.join("MacOS/Xcode Service").canonicalize()?;
        ensure!(
            executable.starts_with(developer) && executable.is_file(),
            "headless_service_executable"
        );
        Ok(ServiceGeneration {
            uid: unsafe { libc::geteuid() },
            pid: 0,
            start_sec: 0,
            start_usec: 0,
            boot_id: boot_identity()?,
            developer_dir: developer.clone(),
            executable,
            bundle_id,
            build,
            xcode_build,
        })
    }

    async fn selected_environment() -> Result<CommandEnvironment> {
        let mut environment = account_environment()?;
        let developer = if let Some(path) = std::env::var_os("DEVELOPER_DIR") {
            PathBuf::from(path)
        } else {
            let (ok, out, _) = metadata(
                &environment,
                Path::new("/usr/bin/xcode-select"),
                &[OsStr::new("-p")],
            )
            .await?;
            ensure!(ok, "headless_developer_unknown");
            PathBuf::from(std::str::from_utf8(&out)?.trim())
        };
        ensure!(developer.is_absolute(), "headless_developer_unknown");
        let developer = developer
            .canonicalize()
            .context("headless_developer_unknown")?;
        ensure!(developer.is_dir(), "headless_developer_unknown");
        environment.developer = Some(developer);
        Ok(environment)
    }

    async fn status(environment: &CommandEnvironment) -> Result<Vec<u8>> {
        let developer = environment
            .developer
            .as_ref()
            .context("headless_developer_unknown")?;
        let cli = developer.join("usr/bin/mcp-server").canonicalize()?;
        ensure!(
            cli.starts_with(developer) && cli.is_file(),
            "headless_status_command_identity"
        );
        let (ok, out, _) = metadata(
            environment,
            &cli,
            &[
                OsStr::new("status"),
                OsStr::new("--format"),
                OsStr::new("json"),
            ],
        )
        .await?;
        ensure!(ok, "headless_status_unknown");
        Ok(out)
    }

    pub(super) async fn prepare_startup() -> Result<Option<HeadlessServiceStartupPlan>> {
        let environment = selected_environment().await?;
        let expected = installation(&environment).await?;
        let candidates = services(&expected)?;
        if candidates
            .iter()
            .any(|candidate| candidate.uid == expected.uid)
        {
            select_service(&expected, &candidates)?;
            return Ok(None);
        }
        validate_startup_status(&status(&environment).await?)?;
        let bundle = expected
            .developer_dir
            .join("Library/Xcode/Agents/Xcode Service.app");
        ensure!(
            bundle.canonicalize()? == bundle,
            "headless_service_bundle_redirected"
        );
        ensure!(
            plist(
                &environment,
                &bundle.join("Contents/Info.plist"),
                "LSUIElement"
            )
            .await?
                == "true",
            "headless_service_not_background_only"
        );
        confirm_generation(&expected, &installation(&environment).await?)?;
        let candidates = services(&expected)?;
        if candidates
            .iter()
            .any(|candidate| candidate.uid == expected.uid)
        {
            select_service(&expected, &candidates)?;
            return Ok(None);
        }
        Ok(Some(HeadlessServiceStartupPlan {
            installation: expected,
            environment,
        }))
    }

    pub(super) async fn start_after_fence(plan: &HeadlessServiceStartupPlan) -> Result<()> {
        let current = prepare_startup()
            .await?
            .context("headless_service_startup_no_longer_absent")?;
        ensure!(current == *plan, "headless_service_startup_plan_changed");
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(5);
        launch::dispatch(plan)?;
        // Only observe readiness after the single launch request. Never relaunch,
        // stop a service, enable access, approve a folder, or open a project here.
        await_startup_readiness(&plan.installation, deadline, || async {
            let observed_installation = installation(&plan.environment).await?;
            confirm_generation(&plan.installation, &observed_installation)?;
            let current_generation = || -> Result<Option<ServiceGeneration>> {
                let candidates = services(&plan.installation)?;
                if candidates
                    .iter()
                    .any(|candidate| candidate.uid == plan.installation.uid)
                {
                    Ok(Some(select_service(&plan.installation, &candidates)?))
                } else {
                    Ok(None)
                }
            };
            let generation = current_generation()?;
            let observed_status = status(&plan.environment).await?;
            confirm_generation(&plan.installation, &installation(&plan.environment).await?)?;
            let after = current_generation()?;
            if let Some(before) = &generation {
                confirm_generation(
                    before,
                    after
                        .as_ref()
                        .context("headless_service_generation_changed")?,
                )?;
            }
            // A PID first appearing during the status probe needs a later stable
            // observation; status and identity must describe the same generation.
            Ok(ServiceStartupObservation {
                installation: observed_installation,
                generation,
                status: observed_status,
            })
        })
        .await
    }

    pub(super) async fn inspect() -> Result<HostSnapshot> {
        let environment = selected_environment().await?;
        let expected = installation(&environment).await?;
        let generation = select_service(&expected, &services(&expected)?)?;
        // Ordinary inspection rejects absence before invoking even the status CLI.
        let open_projects = status_projects(&status(&environment).await?)?;
        let after_installation = installation(&environment).await?;
        confirm_generation(&expected, &after_installation)?;
        let after = select_service(&after_installation, &services(&after_installation)?)?;
        confirm_generation(&generation, &after)?;
        Ok(HostSnapshot {
            generation,
            operator_home: environment.home,
            account_name: environment.account,
            darwin_tmpdir: environment.tmpdir,
            open_projects,
        })
    }

    mod launch {
        use super::*;
        use std::ffi::CString;
        type Object = *mut libc::c_void;
        type Selector = *mut libc::c_void;

        #[link(name = "AppKit", kind = "framework")]
        unsafe extern "C" {}
        #[link(name = "Foundation", kind = "framework")]
        unsafe extern "C" {}
        #[link(name = "objc")]
        unsafe extern "C" {
            fn objc_getClass(name: *const libc::c_char) -> Object;
            fn sel_registerName(name: *const libc::c_char) -> Selector;
            fn objc_msgSend();
            fn objc_autoreleasePoolPush() -> Object;
            fn objc_autoreleasePoolPop(pool: Object);
        }

        struct Pool(Object);
        impl Drop for Pool {
            fn drop(&mut self) {
                unsafe {
                    objc_autoreleasePoolPop(self.0);
                }
            }
        }

        unsafe fn class(name: &CStr) -> Result<Object> {
            let object = objc_getClass(name.as_ptr());
            ensure!(!object.is_null(), "headless_service_launch_api_unavailable");
            Ok(object)
        }

        unsafe fn object(receiver: Object, name: &CStr) -> Result<Object> {
            let send: unsafe extern "C" fn(Object, Selector) -> Object =
                mem::transmute(objc_msgSend as unsafe extern "C" fn());
            let result = send(receiver, sel_registerName(name.as_ptr()));
            ensure!(!result.is_null(), "headless_service_launch_api_unavailable");
            Ok(result)
        }

        unsafe fn string(value: &str) -> Result<Object> {
            let value = CString::new(value)?;
            let send: unsafe extern "C" fn(Object, Selector, *const libc::c_char) -> Object =
                mem::transmute(objc_msgSend as unsafe extern "C" fn());
            let result = send(
                class(c"NSString")?,
                sel_registerName(c"stringWithUTF8String:".as_ptr()),
                value.as_ptr(),
            );
            ensure!(!result.is_null(), "headless_service_launch_string");
            Ok(result)
        }

        pub(super) fn dispatch(plan: &HeadlessServiceStartupPlan) -> Result<()> {
            let bundle = plan
                .installation
                .developer_dir
                .join("Library/Xcode/Agents/Xcode Service.app");
            ensure!(
                bundle.canonicalize()? == bundle,
                "headless_service_bundle_redirected"
            );
            ensure!(
                bundle.join("Contents/MacOS/Xcode Service").canonicalize()?
                    == plan.installation.executable,
                "headless_service_executable_changed"
            );
            // SDK NSWorkspace.h declares these exact Objective-C signatures. All
            // objects are confined to this synchronous autorelease scope; the
            // asynchronous API retains its URL/configuration. A nil completion
            // block is documented; native process inspection determines outcome.
            unsafe {
                let _pool = Pool(objc_autoreleasePoolPush());
                let workspace = object(class(c"NSWorkspace")?, c"sharedWorkspace")?;
                let configuration =
                    object(class(c"NSWorkspaceOpenConfiguration")?, c"configuration")?;
                let send_bool: unsafe extern "C" fn(Object, Selector, bool) =
                    mem::transmute(objc_msgSend as unsafe extern "C" fn());
                for (selector, value) in [
                    (c"setPromptsUserIfNeeded:", false),
                    (c"setAddsToRecentItems:", false),
                    (c"setActivates:", false),
                    (c"setHides:", true),
                    (c"setHidesOthers:", false),
                    (c"setCreatesNewApplicationInstance:", false),
                    (c"setAllowsRunningApplicationSubstitution:", false),
                ] {
                    send_bool(configuration, sel_registerName(selector.as_ptr()), value);
                }
                let environment = object(class(c"NSMutableDictionary")?, c"dictionary")?;
                let put: unsafe extern "C" fn(Object, Selector, Object, Object) =
                    mem::transmute(objc_msgSend as unsafe extern "C" fn());
                for (key, value) in [
                    (
                        "HOME",
                        plan.environment
                            .home
                            .to_str()
                            .context("headless_account_home")?,
                    ),
                    ("USER", plan.environment.account.as_str()),
                    ("LOGNAME", plan.environment.account.as_str()),
                    (
                        "TMPDIR",
                        plan.environment
                            .tmpdir
                            .to_str()
                            .context("headless_darwin_tmpdir")?,
                    ),
                    (
                        "DEVELOPER_DIR",
                        plan.installation
                            .developer_dir
                            .to_str()
                            .context("headless_developer_unknown")?,
                    ),
                    ("PATH", "/usr/bin:/bin:/usr/sbin:/sbin"),
                ] {
                    put(
                        environment,
                        sel_registerName(c"setObject:forKey:".as_ptr()),
                        string(value)?,
                        string(key)?,
                    );
                }
                let set_object: unsafe extern "C" fn(Object, Selector, Object) =
                    mem::transmute(objc_msgSend as unsafe extern "C" fn());
                set_object(
                    configuration,
                    sel_registerName(c"setEnvironment:".as_ptr()),
                    environment,
                );
                set_object(
                    configuration,
                    sel_registerName(c"setArguments:".as_ptr()),
                    object(class(c"NSArray")?, c"array")?,
                );
                let url: unsafe extern "C" fn(Object, Selector, Object) -> Object =
                    mem::transmute(objc_msgSend as unsafe extern "C" fn());
                let url = url(
                    class(c"NSURL")?,
                    sel_registerName(c"fileURLWithPath:".as_ptr()),
                    string(bundle.to_str().context("headless_service_bundle_path")?)?,
                );
                ensure!(!url.is_null(), "headless_service_launch_url");
                let launch: unsafe extern "C" fn(Object, Selector, Object, Object, Object) =
                    mem::transmute(objc_msgSend as unsafe extern "C" fn());
                launch(
                    workspace,
                    sel_registerName(
                        c"openApplicationAtURL:configuration:completionHandler:".as_ptr(),
                    ),
                    url,
                    configuration,
                    std::ptr::null_mut(),
                );
            }
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};

    fn generation() -> ServiceGeneration {
        ServiceGeneration {
            uid: 501, pid: 123, start_sec: 1000, start_usec: 4321,
            boot_id: "A78A118B-D725-4835-87A1-1BC539E8BB18".into(),
            developer_dir: "/Applications/Xcode.app/Contents/Developer".into(),
            executable: "/Applications/Xcode.app/Contents/Developer/Library/Xcode/Agents/Xcode Service.app/Contents/MacOS/Xcode Service".into(),
            bundle_id: "com.apple.dt.mcp-server".into(),
            build: "1.0/25317000000000000".into(), xcode_build: "27A266a".into(),
        }
    }

    fn status() -> Value {
        json!({"permission":{"enabled":true,"unsafeAlwaysAllowAllAgents":false},
               "running":true,"openWorkspaces":[]})
    }

    fn startup_plan() -> HeadlessServiceStartupPlan {
        let mut installation = generation();
        installation.pid = 0;
        installation.start_sec = 0;
        installation.start_usec = 0;
        HeadlessServiceStartupPlan {
            environment: CommandEnvironment {
                home: "/operator/home".into(),
                account: "operator".into(),
                tmpdir: "/operator/tmp".into(),
                developer: Some(installation.developer_dir.clone()),
            },
            installation,
        }
    }

    #[test]
    fn startup_default_inspector_is_read_only_and_opt_in_is_explicit() {
        assert!(matches!(
            LocalHeadlessHostInspector::new().startup.into_inner(),
            ServiceStartupSlot::Disabled
        ));
        assert!(matches!(
            LocalHeadlessHostInspector::with_startup_on_absence()
                .startup
                .into_inner(),
            ServiceStartupSlot::Available
        ));
        let mut disabled = ServiceStartupSlot::Disabled;
        assert!(disabled.pin(Some(startup_plan())).is_err());
        assert!(disabled.consume(&startup_plan()).is_err());
        let snapshot = HostSnapshot {
            generation: generation(),
            operator_home: "/operator/home".into(),
            account_name: "operator".into(),
            darwin_tmpdir: "/operator/tmp".into(),
            open_projects: vec!["/never-open-this.xcodeproj".into()],
        };
        let fixture = HeadlessServiceStartupPlan::from_fixture_snapshot(&snapshot);
        assert_eq!(fixture, startup_plan());
        assert!(disabled.consume(&fixture).is_err());
    }

    #[test]
    fn startup_plan_is_one_shot_and_never_restarts_a_warm_service() {
        let plan = startup_plan();
        let mut slot = ServiceStartupSlot::Available;
        slot.pin(Some(plan.clone())).unwrap();
        let mut changed = plan.clone();
        changed.installation.uid += 1;
        assert!(slot.consume(&changed).is_err());
        slot.consume(&plan).unwrap();
        assert!(slot.consume(&plan).is_err());
        assert!(slot.pin(Some(plan.clone())).is_err());
        let mut warm = ServiceStartupSlot::Available;
        warm.pin(None).unwrap();
        assert!(warm.pin(Some(plan)).is_err());
    }

    #[test]
    fn startup_requires_explicit_enabled_safe_and_absent_status() {
        let mut stopped = status();
        stopped["running"] = json!(false);
        validate_startup_status(&serde_json::to_vec(&stopped).unwrap()).unwrap();
        for pointer in [
            "/permission/enabled",
            "/permission/unsafeAlwaysAllowAllAgents",
            "/running",
        ] {
            for invalid in [
                Value::Null,
                json!("false"),
                json!(1),
                json!(!stopped.pointer(pointer).unwrap().as_bool().unwrap()),
            ] {
                let mut changed = stopped.clone();
                *changed.pointer_mut(pointer).unwrap() = invalid;
                assert!(
                    validate_startup_status(&serde_json::to_vec(&changed).unwrap()).is_err(),
                    "{pointer}"
                );
            }
        }
        assert!(validate_startup_status(br#"{"permission":{"enabled":false,"enabled":true,"unsafeAlwaysAllowAllAgents":false},"running":false}"#).is_err());
    }

    fn startup_observation(running: bool) -> ServiceStartupObservation {
        let mut value = status();
        value["running"] = json!(running);
        ServiceStartupObservation {
            installation: startup_plan().installation,
            generation: Some(generation()),
            status: serde_json::to_vec(&value).unwrap(),
        }
    }

    #[tokio::test(start_paused = true)]
    async fn startup_readiness_waits_for_safe_status_after_pid_appears() {
        let mut absent = startup_observation(false);
        absent.generation = None;
        let mut observations = std::collections::VecDeque::from([
            absent,
            startup_observation(false),
            startup_observation(false),
            startup_observation(true),
        ]);
        let mut calls = 0;
        await_startup_readiness(
            &startup_plan().installation,
            tokio::time::Instant::now() + std::time::Duration::from_secs(1),
            || {
                calls += 1;
                std::future::ready(Ok(observations
                    .pop_front()
                    .expect("unexpected extra probe")))
            },
        )
        .await
        .unwrap();
        assert_eq!(calls, 4, "a visible PID is not ready status");
        assert!(observations.is_empty());
    }

    #[tokio::test(start_paused = true)]
    async fn startup_readiness_rejects_unsafe_or_malformed_status() {
        let mut invalid = Vec::new();
        for (pointer, value) in [
            ("/permission/enabled", json!(false)),
            ("/permission/enabled", json!("true")),
            ("/permission/unsafeAlwaysAllowAllAgents", json!(true)),
            ("/permission/unsafeAlwaysAllowAllAgents", Value::Null),
            ("/running", Value::Null),
            ("/openWorkspaces", Value::Null),
        ] {
            let mut changed = status();
            *changed.pointer_mut(pointer).unwrap() = value;
            invalid.push(serde_json::to_vec(&changed).unwrap());
        }
        invalid.push(br#"{"permission":{"enabled":false,"enabled":true,"unsafeAlwaysAllowAllAgents":false},"running":true,"openWorkspaces":[]}"#.to_vec());
        for bytes in invalid {
            let mut observed = startup_observation(true);
            observed.status = bytes;
            let mut observed = Some(observed);
            let mut calls = 0;
            let result = await_startup_readiness(
                &startup_plan().installation,
                tokio::time::Instant::now() + std::time::Duration::from_secs(1),
                || {
                    calls += 1;
                    std::future::ready(Ok(observed.take().expect("unsafe status must not retry")))
                },
            )
            .await;
            assert!(
                result.is_err(),
                "unsafe or malformed status must not admit readiness"
            );
            assert_eq!(calls, 1);
        }
    }

    #[tokio::test(start_paused = true)]
    async fn startup_readiness_rejects_installation_and_generation_drift() {
        for change in 0..3 {
            let mut changed = startup_observation(true);
            match change {
                0 => changed.installation.xcode_build = "different".into(),
                1 => changed.generation.as_mut().unwrap().start_usec += 1,
                _ => changed.generation = None,
            }
            let mut observations =
                std::collections::VecDeque::from([startup_observation(false), changed]);
            let mut calls = 0;
            let result = await_startup_readiness(
                &startup_plan().installation,
                tokio::time::Instant::now() + std::time::Duration::from_secs(1),
                || {
                    calls += 1;
                    std::future::ready(Ok(observations.pop_front().expect("drift must not retry")))
                },
            )
            .await;
            assert!(
                result.is_err(),
                "installation or observed service generation changed"
            );
            assert_eq!(calls, 2);
        }
    }

    #[tokio::test(start_paused = true)]
    async fn startup_readiness_has_one_deadline_for_pending_status() {
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_millis(100);
        let mut calls = 0;
        let error = await_startup_readiness(&startup_plan().installation, deadline, || {
            calls += 1;
            std::future::ready(Ok(startup_observation(false)))
        })
        .await
        .unwrap_err();
        assert_eq!(
            error.to_string(),
            "headless_service_startup_outcome_unknown"
        );
        assert_eq!(tokio::time::Instant::now(), deadline);
        assert!(
            calls >= 2,
            "read-only observations may repeat inside the same deadline"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn startup_readiness_bounds_a_stalled_status_probe() {
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_millis(100);
        let mut calls = 0;
        let error = await_startup_readiness(&startup_plan().installation, deadline, || {
            calls += 1;
            std::future::pending::<Result<ServiceStartupObservation>>()
        })
        .await
        .unwrap_err();
        assert_eq!(
            error.to_string(),
            "headless_service_startup_outcome_unknown"
        );
        assert_eq!(tokio::time::Instant::now(), deadline);
        assert_eq!(calls, 1);
    }

    #[test]
    fn observed_bundle_version_is_not_mcp_server_version() {
        // Installed plists: CFBundleVersion=1.0; SourceVersion=25317000000000000.
        // Checked-in 2026-09-19 MCP initialize capture: serverInfo.version=25317.
        assert_eq!(
            tested_build("27A266a", "1.0", "25317000000000000").unwrap(),
            "1.0/25317000000000000"
        );
        for (xcode, bundle, source) in [
            ("future", "1.0", "25317000000000000"),
            ("27A266a", "25317", "25317000000000000"),
            ("27A266a", "1.0", "25318000000000000"),
        ] {
            assert!(tested_build(xcode, bundle, source).is_err());
        }
    }

    #[test]
    fn observed_status_uses_nested_flags_and_paths_not_workspace_ids() {
        assert!(status_projects(&serde_json::to_vec(&status()).unwrap())
            .unwrap()
            .is_empty());
        let temp = tempfile::tempdir().unwrap();
        let project = temp.path().join("Fixture.xcodeproj");
        std::fs::create_dir(&project).unwrap();
        std::fs::write(project.join("project.pbxproj"), b"fixture").unwrap();
        let mut value = status();
        value["openWorkspaces"] =
            json!([{"path":project,"displayName":"Fixture","activeSchemeName":"Fixture"}]);
        assert_eq!(
            status_projects(&serde_json::to_vec(&value).unwrap()).unwrap(),
            vec![project.canonicalize().unwrap()]
        );
        value["openWorkspaces"][0]["path"] = json!("relative.xcodeproj");
        assert!(status_projects(&serde_json::to_vec(&value).unwrap()).is_err());
    }

    #[test]
    fn status_unknown_unsafe_stopped_and_duplicate_paths_fail_closed() {
        for pointer in [
            "/permission/enabled",
            "/permission/unsafeAlwaysAllowAllAgents",
            "/running",
        ] {
            for value in [Value::Null, json!("true"), json!(1)] {
                let mut changed = status();
                *changed.pointer_mut(pointer).unwrap() = value;
                assert!(status_projects(&serde_json::to_vec(&changed).unwrap()).is_err());
            }
            let mut changed = status();
            let flag = changed.pointer_mut(pointer).unwrap();
            *flag = json!(!flag.as_bool().unwrap());
            assert!(status_projects(&serde_json::to_vec(&changed).unwrap()).is_err());
        }
        assert!(status_projects(br#"{"enabled":true,"running":true,"unsafeAlwaysAllowAllAgents":false,"openWorkspaces":[]}"#).is_err());
        let mut missing = status();
        missing.as_object_mut().unwrap().remove("openWorkspaces");
        assert!(status_projects(&serde_json::to_vec(&missing).unwrap()).is_err());
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("A.xcworkspace");
        std::fs::create_dir(&path).unwrap();
        std::fs::write(path.join("contents.xcworkspacedata"), b"fixture").unwrap();
        let entry = json!({"path":path,"displayName":"A","activeSchemeName":"A"});
        let mut duplicate = status();
        duplicate["openWorkspaces"] = json!([entry.clone(), entry]);
        assert!(status_projects(&serde_json::to_vec(&duplicate).unwrap()).is_err());
    }

    #[test]
    fn service_requires_exact_uid_install_and_unambiguous_native_identity() {
        let expected = generation();
        assert_eq!(
            select_service(&expected, &[expected.clone()]).unwrap(),
            expected
        );
        let mut ide = expected.clone();
        ide.executable = "/Applications/Xcode.app/Contents/MacOS/Xcode".into();
        assert_eq!(
            select_service(&expected, &[ide, expected.clone()]).unwrap(),
            expected
        );
        assert!(select_service(&expected, &[])
            .unwrap_err()
            .to_string()
            .contains("action_required"));
        assert!(select_service(&expected, &[expected.clone(), expected.clone()]).is_err());
        for field in 0..8 {
            let mut bad = expected.clone();
            match field {
                0 => bad.uid += 1,
                1 => bad.executable = "/wrong".into(),
                2 => bad.developer_dir = "/wrong".into(),
                3 => bad.start_sec = 0,
                4 => bad.pid = 0,
                5 => bad.start_usec = 1_000_000,
                6 => bad.boot_id.clear(),
                _ => bad.bundle_id = "wrong".into(),
            }
            assert!(select_service(&expected, &[bad]).is_err(), "{field}");
        }
    }

    #[test]
    fn malformed_same_install_candidate_cannot_be_silently_discarded() {
        let good = generation();
        let mut bad = good.clone();
        bad.pid += 1;
        bad.start_sec = 0;
        assert!(select_service(&good, &[good.clone(), bad]).is_err());
        let mut foreign_user = good.clone();
        foreign_user.uid += 1;
        assert_eq!(
            select_service(&good, &[foreign_user, good.clone()]).unwrap(),
            good
        );
    }

    #[test]
    fn malformed_status_entries_never_become_project_evidence() {
        let temp = tempfile::tempdir().unwrap();
        let project = temp.path().join("Fixture.xcodeproj");
        fs::create_dir(&project).unwrap();
        fs::write(project.join("project.pbxproj"), b"fixture").unwrap();
        let entry = json!({"path":project,"displayName":"Fixture","activeSchemeName":"Fixture"});
        for key in ["path", "displayName", "activeSchemeName"] {
            for invalid in [Value::Null, json!(1), json!({})] {
                let mut changed = entry.clone();
                changed[key] = invalid;
                let mut value = status();
                value["openWorkspaces"] = json!([changed]);
                assert!(
                    status_projects(&serde_json::to_vec(&value).unwrap()).is_err(),
                    "{key}"
                );
            }
        }
        let mut value = status();
        let mut with_id = entry;
        with_id["identifier"] = json!("invented-id");
        value["openWorkspaces"] = json!([with_id]);
        assert!(status_projects(&serde_json::to_vec(&value).unwrap()).is_err());
    }

    #[test]
    fn duplicate_nested_permission_flag_is_unknown_not_last_value_wins() {
        let duplicate = br#"{"permission":{"enabled":false,"enabled":true,"unsafeAlwaysAllowAllAgents":false},"running":true,"openWorkspaces":[]}"#;
        assert!(status_projects(duplicate).is_err());
    }

    #[test]
    fn same_pid_with_reused_start_or_new_boot_is_not_same_generation() {
        let before = generation();
        confirm_generation(&before, &before).unwrap();
        let mut after = before.clone();
        after.start_usec += 1;
        assert!(confirm_generation(&before, &after).is_err());
        after = before.clone();
        after.boot_id = "BD21212A-8027-4BF5-9186-8C05ACBC982E".into();
        assert!(confirm_generation(&before, &after).is_err());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn metadata_drains_both_streams_and_pins_environment() {
        use std::{ffi::OsStr, path::Path};
        let environment = CommandEnvironment {
            home: "/operator/home".into(),
            account: "operator".into(),
            tmpdir: "/operator/tmp".into(),
            developer: Some("/installed/developer".into()),
        };
        let (ok, out, err) = metadata(&environment, Path::new("/bin/sh"), &[OsStr::new("-c"), OsStr::new("printf '%s|%s|%s|%s|%s|%s' \"$HOME\" \"$USER\" \"$LOGNAME\" \"$TMPDIR\" \"$DEVELOPER_DIR\" \"$PATH\"; printf 'diagnostic' >&2")]).await.unwrap();
        assert!(ok);
        assert_eq!(out, b"/operator/home|operator|operator|/operator/tmp|/installed/developer|/usr/bin:/bin:/usr/sbin:/sbin");
        assert_eq!(err, b"diagnostic");
        let (_, out, _) = metadata(&environment, Path::new("/usr/bin/env"), &[])
            .await
            .unwrap();
        let actual = std::str::from_utf8(&out).unwrap();
        assert_eq!(actual.lines().count(), 6, "{actual}");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn metadata_drains_pipe_capacity_and_combines_stream_budgets() {
        use std::ffi::OsStr;
        let temp = tempfile::tempdir().unwrap();
        let environment = CommandEnvironment {
            home: temp.path().into(),
            account: "fixture".into(),
            tmpdir: temp.path().into(),
            developer: None,
        };
        let (ok, out, err) = metadata(&environment, Path::new("/bin/sh"), &[OsStr::new("-c"), OsStr::new("/usr/bin/head -c 524288 /dev/zero & /usr/bin/head -c 524288 /dev/zero >&2 & wait")]).await.unwrap();
        assert!(ok);
        assert_eq!(out.len(), 524288);
        assert_eq!(err.len(), 524288);
        let error = metadata(&environment, Path::new("/bin/sh"), &[OsStr::new("-c"), OsStr::new("/usr/bin/head -c 524289 /dev/zero & /usr/bin/head -c 524288 /dev/zero >&2 & wait")]).await.unwrap_err();
        assert!(error.to_string().contains("metadata_size"), "{error:#}");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn metadata_normal_exit_does_not_signal_reaped_process_group() {
        use std::ffi::OsStr;
        let temp = tempfile::tempdir().unwrap();
        let marker = temp.path().join("normal-exit");
        let environment = CommandEnvironment {
            home: temp.path().into(),
            account: "fixture".into(),
            tmpdir: temp.path().into(),
            developer: None,
        };
        let (ok, _, _) = metadata(&environment, Path::new("/bin/sh"), &[OsStr::new("-c"), OsStr::new("(/bin/sleep 0.2; printf 'survived' > \"$1\") </dev/null >/dev/null 2>&1 & exit 0"), OsStr::new("fixture"), marker.as_os_str()]).await.unwrap();
        assert!(ok);
        tokio::time::timeout(std::time::Duration::from_secs(2), async {
            while !marker.exists() {
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("normal metadata completion must disarm the process-group guard");
        assert_eq!(fs::read(marker).unwrap(), b"survived");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn metadata_descendant_held_pipes_still_time_out_and_are_killed() {
        use std::ffi::OsStr;
        let temp = tempfile::tempdir().unwrap();
        let pidfile = temp.path().join("descendant-pid");
        let environment = CommandEnvironment {
            home: temp.path().into(),
            account: "fixture".into(),
            tmpdir: temp.path().into(),
            developer: None,
        };
        let error = metadata(
            &environment,
            Path::new("/bin/sh"),
            &[
                OsStr::new("-c"),
                OsStr::new("/bin/sleep 30 & printf '%s' $! > \"$1\"; exit 0"),
                OsStr::new("fixture"),
                pidfile.as_os_str(),
            ],
        )
        .await
        .unwrap_err();
        assert!(error.to_string().contains("metadata_timeout"), "{error:#}");
        let pid: i32 = fs::read_to_string(pidfile).unwrap().parse().unwrap();
        tokio::time::timeout(std::time::Duration::from_secs(2), async {
            while unsafe { libc::kill(pid, 0) } == 0 {
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("timed-out descendant must be killed");
        assert_eq!(unsafe { libc::kill(pid, 0) }, -1);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn metadata_bounds_output_timeout_and_reaps_process_group() {
        use std::{ffi::OsStr, path::Path};
        let temp = tempfile::tempdir().unwrap();
        let pidfile = temp.path().join("pid");
        let environment = CommandEnvironment {
            home: temp.path().into(),
            account: "fixture".into(),
            tmpdir: temp.path().into(),
            developer: None,
        };
        let over = metadata(
            &environment,
            Path::new("/bin/sh"),
            &[
                OsStr::new("-c"),
                OsStr::new("/usr/bin/head -c 1048577 /dev/zero >&2"),
            ],
        )
        .await
        .unwrap_err();
        assert!(over.to_string().contains("metadata_size"), "{over:#}");
        let started = std::time::Instant::now();
        let timed = metadata(
            &environment,
            Path::new("/bin/sh"),
            &[
                OsStr::new("-c"),
                OsStr::new("printf '%s' $$ > \"$1\"; exec /bin/sleep 30"),
                OsStr::new("fixture"),
                pidfile.as_os_str(),
            ],
        )
        .await
        .unwrap_err();
        assert!(timed.to_string().contains("metadata_timeout"), "{timed:#}");
        assert!(started.elapsed() < std::time::Duration::from_secs(7));
        let pid: i32 = std::fs::read_to_string(pidfile).unwrap().parse().unwrap();
        assert_eq!(unsafe { libc::kill(pid, 0) }, -1);
        assert_eq!(
            std::io::Error::last_os_error().raw_os_error(),
            Some(libc::ESRCH)
        );
    }
}
