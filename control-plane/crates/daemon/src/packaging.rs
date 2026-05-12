//! Packaging and per-mode configuration (Proposal 042 §7).
//!
//! Owns the `MODE` env-var dispatch, per-mode path resolution for DB /
//! logs / principals, the port-4000 → ephemeral fallback, and the
//! `build-sha.txt` writer the Swift diagnostics bundle reads.

use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use anyhow::{Context, Result};
use serde::Serialize;

/// P042 §7.1 mode dispatch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DaemonMode {
    /// Developer checkout. `cargo run`; DB and log live in cwd.
    Dev,
    /// App-bundled daemon launched by `SMAppService.Agent`.
    PackagedApp,
    /// Headless launchd LaunchAgent.
    PackagedHelper,
    /// Test/fixture; paths are per-test.
    Test,
    /// `MODE=mcp` — legacy stdio-only mode. Retained for P029 stdio tests.
    Mcp,
}

impl DaemonMode {
    pub fn from_env_var(raw: &str) -> Self {
        match raw {
            "packaged-app" => Self::PackagedApp,
            "packaged-helper" => Self::PackagedHelper,
            "test" => Self::Test,
            "mcp" => Self::Mcp,
            _ => Self::Dev,
        }
    }

    pub fn is_packaged(self) -> bool {
        matches!(self, Self::PackagedApp | Self::PackagedHelper)
    }
}

impl std::fmt::Display for DaemonMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Dev => f.write_str("dev"),
            Self::PackagedApp => f.write_str("packaged-app"),
            Self::PackagedHelper => f.write_str("packaged-helper"),
            Self::Test => f.write_str("test"),
            Self::Mcp => f.write_str("mcp"),
        }
    }
}

/// Resolved set of paths and bind address for a given mode.
#[derive(Debug, Clone)]
pub struct ModePaths {
    pub mode: DaemonMode,
    pub database_url: String,
    pub principals_path: PathBuf,
    /// Directory where daemon writes `daemon.pid`, `crash-budget.json`,
    /// `daemon.port`, and `build-sha.txt`.
    pub app_support_dir: PathBuf,
    pub log_path: Option<PathBuf>,
    pub bind_addr: String,
}

impl ModePaths {
    pub fn pid_path(&self) -> PathBuf {
        self.app_support_dir.join("daemon.pid")
    }
    pub fn crash_budget_path(&self) -> PathBuf {
        self.app_support_dir.join("crash-budget.json")
    }
    pub fn daemon_port_path(&self) -> PathBuf {
        self.app_support_dir.join("daemon.port")
    }
    pub fn daemon_endpoint_path(&self) -> PathBuf {
        self.app_support_dir.join("daemon-endpoint.json")
    }
    pub fn build_sha_path(&self) -> PathBuf {
        self.app_support_dir.join("build-sha.txt")
    }
}

/// Resolve all per-mode paths using environment overrides where
/// reasonable. Used once at startup in `main.rs`.
pub fn resolve_paths(mode: DaemonMode) -> Result<ModePaths> {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));

    let (app_support_dir, default_db, default_log, default_bind) = match mode {
        DaemonMode::PackagedApp | DaemonMode::PackagedHelper => {
            let app_support = home
                .join("Library")
                .join("Application Support")
                .join("Chainworks Forge");
            let log = home
                .join("Library")
                .join("Logs")
                .join("Chainworks Forge")
                .join("daemon.log");
            let db = app_support.join("control-plane.db");
            (
                app_support,
                format!("sqlite://{}?mode=rwc", db.display()),
                Some(log),
                "127.0.0.1:4000".to_string(),
            )
        }
        DaemonMode::Dev | DaemonMode::Mcp => {
            // Dev mode keeps cwd-relative defaults so `cargo run` from the
            // repo works without setup.
            let app_support = home.join(".chainworks").join("dev-app-support");
            (
                app_support,
                "sqlite://./chainworks-control-plane.db".to_string(),
                None,
                "127.0.0.1:4000".to_string(),
            )
        }
        DaemonMode::Test => (
            std::env::temp_dir().join("chainworks-test-app-support"),
            "sqlite::memory:".to_string(),
            None,
            "127.0.0.1:0".to_string(),
        ),
    };

    std::fs::create_dir_all(&app_support_dir)
        .with_context(|| format!("create app_support_dir {}", app_support_dir.display()))?;

    let database_url = std::env::var("DATABASE_URL").unwrap_or(default_db);
    let bind_addr = std::env::var("GRAPHQL_ADDR").unwrap_or(default_bind);

    let principals_path = match std::env::var("CHAINWORKS_AUTH_PRINCIPALS_PATH") {
        Ok(p) if !p.trim().is_empty() => PathBuf::from(p),
        _ => home
            .join(".chainworks")
            .join("auth")
            .join("principals.json"),
    };

    Ok(ModePaths {
        mode,
        database_url,
        principals_path,
        app_support_dir,
        log_path: default_log,
        bind_addr,
    })
}

/// Enforce loopback-only binding for packaged modes (§7.4). If the
/// configured `bind_addr` specifies a non-loopback host in a packaged
/// mode, rewrite it to `127.0.0.1` preserving the port. Dev/test modes
/// are left unchanged.
pub fn enforce_loopback(paths: &mut ModePaths) {
    if !paths.mode.is_packaged() {
        return;
    }
    if let Ok(sock) = SocketAddr::from_str(&paths.bind_addr) {
        if !sock.ip().is_loopback() {
            let new = SocketAddr::new("127.0.0.1".parse().unwrap(), sock.port());
            paths.bind_addr = new.to_string();
        }
    }
}

/// §7.3: try `bind_addr` first; if port is in use (and we are in a
/// packaged mode), fall back to an OS-allocated ephemeral port on
/// loopback and write the chosen port to `daemon.port`. Returns the
/// bound [`tokio::net::TcpListener`] + resolved port.
pub async fn bind_with_fallback(paths: &ModePaths) -> Result<(tokio::net::TcpListener, u16)> {
    // First attempt.
    match tokio::net::TcpListener::bind(&paths.bind_addr).await {
        Ok(l) => {
            let port = l.local_addr()?.port();
            write_daemon_port_file(paths, port)?;
            Ok((l, port))
        }
        Err(e) if paths.mode.is_packaged() => {
            // Fall back to ephemeral on loopback.
            tracing::warn!(
                preferred = %paths.bind_addr,
                err = %e,
                "preferred bind failed; falling back to ephemeral loopback"
            );
            let fallback = "127.0.0.1:0";
            let listener = tokio::net::TcpListener::bind(fallback).await?;
            let port = listener.local_addr()?.port();
            write_daemon_port_file(paths, port)?;
            Ok((listener, port))
        }
        Err(e) => Err(anyhow::anyhow!("bind {}: {e}", paths.bind_addr)),
    }
}

fn write_daemon_port_file(paths: &ModePaths, port: u16) -> Result<()> {
    let path = paths.daemon_port_path();
    std::fs::write(&path, port.to_string()).with_context(|| format!("write {}", path.display()))
}

#[derive(Serialize)]
struct DaemonEndpointSnapshot<'a> {
    pid: u32,
    port: u16,
    build_sha: &'a str,
}

pub fn write_daemon_endpoint_snapshot(
    paths: &ModePaths,
    pid: u32,
    port: u16,
    build_sha: &str,
) -> Result<()> {
    let payload = serde_json::to_vec_pretty(&DaemonEndpointSnapshot {
        pid,
        port,
        build_sha,
    })?;
    let path = paths.daemon_endpoint_path();
    std::fs::write(&path, payload).with_context(|| format!("write {}", path.display()))
}

/// The literal build SHA baked into the binary at compile time by the
/// Xcode embed phase (`scripts/embed-control-plane-daemon.sh` exports
/// `GIT_SHA` before `cargo build`). Empty if the embed phase didn't
/// export the variable — that's the signal for `resolved_build_sha`
/// to fall back to `"dev"`.
pub const COMPILE_TIME_BUILD_SHA: Option<&str> = option_env!("GIT_SHA");

/// R13 OPS-002: packaged modes must run from `$HOME` so relative
/// defaults (catalog YAMLs, workflow YAMLs, test fixtures) resolve
/// under a stable operator-owned root rather than the launcher's cwd
/// (source checkout or `/`). This helper returns the target cwd for
/// a given mode:
///
/// * `PackagedApp` / `PackagedHelper` → `Some(home)` so `main.rs` can
///   `set_current_dir(home)`.
/// * Dev / Test / Mcp → `None` so `cargo run` / `cargo test` keep
///   their original cwd semantics.
///
/// Takes `home` explicitly so unit tests can exercise both arms
/// without touching process state or the live `dirs::home_dir()`.
pub fn packaged_cwd_target(mode: DaemonMode, home: Option<PathBuf>) -> Option<PathBuf> {
    if mode.is_packaged() {
        home
    } else {
        None
    }
}

/// Resolve a resource bundled next to the packaged daemon.
///
/// Xcode copies YAML resources into `Chainworks Forge.app/Contents/Resources`
/// while the LaunchAgent runs `Contents/MacOS/chainworks-forge-daemon`.
/// Packaged mode intentionally chdirs to `$HOME`, so daemon-owned defaults
/// must be rooted at the app bundle rather than at the current directory.
pub fn bundled_resource_path(resource_name: &str) -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    bundled_resource_path_from_exe(&exe, resource_name)
}

pub fn bundled_resource_path_from_exe(exe_path: &Path, resource_name: &str) -> Option<PathBuf> {
    let contents_dir = exe_path.parent()?.parent()?;
    let resource_path = contents_dir.join("Resources").join(resource_name);
    resource_path.exists().then_some(resource_path)
}

/// Resolve the build SHA that should be stamped into `build-sha.txt`.
/// Separated from the file-writing call so tests can exercise the
/// fallback logic without shelling out to a fresh `rustc`.
///
/// Resolution order:
/// 1. Runtime override — the `GIT_SHA` env var read *at call time* (not
///    compile time). Useful in tests and in a packaged release that
///    wants to re-stamp the SHA without a rebuild.
/// 2. `COMPILE_TIME_BUILD_SHA` — the value baked in by `cargo build`.
/// 3. `"dev"` — dev-workstation fallback. R12 OPS-001: this path is
///    what shipped before the embed script moved `GIT_SHA` above the
///    `cargo build` call, so seeing `"dev"` in a release bundle is
///    the signature of that regression.
pub fn resolved_build_sha() -> &'static str {
    if let Ok(val) = std::env::var("GIT_SHA") {
        if !val.trim().is_empty() {
            // Leak so the caller can store a `&'static str`. Called
            // at most once per daemon process at startup, so the
            // one-time leak is irrelevant.
            return Box::leak(val.into_boxed_str());
        }
    }
    COMPILE_TIME_BUILD_SHA
        .filter(|s| !s.trim().is_empty())
        .unwrap_or("dev")
}

/// Write the build SHA to `build-sha.txt` so the Swift diagnostics
/// bundle can read it without hitting GraphQL (§9.4). Value is
/// whatever [`resolved_build_sha`] returns.
pub fn write_build_sha(paths: &ModePaths) -> Result<()> {
    write_build_sha_value(paths, resolved_build_sha())
}

/// Test seam — same as [`write_build_sha`] but takes an explicit SHA
/// so unit tests can assert the on-disk content without fighting the
/// compile-time literal.
pub fn write_build_sha_value(paths: &ModePaths, sha: &str) -> Result<()> {
    let path = paths.build_sha_path();
    std::fs::write(&path, sha).with_context(|| format!("write {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mode_from_env_var_variants() {
        assert_eq!(
            DaemonMode::from_env_var("packaged-app"),
            DaemonMode::PackagedApp
        );
        assert_eq!(
            DaemonMode::from_env_var("packaged-helper"),
            DaemonMode::PackagedHelper
        );
        assert_eq!(DaemonMode::from_env_var("test"), DaemonMode::Test);
        assert_eq!(DaemonMode::from_env_var("mcp"), DaemonMode::Mcp);
        assert_eq!(DaemonMode::from_env_var("daemon"), DaemonMode::Dev);
        assert_eq!(DaemonMode::from_env_var("unknown"), DaemonMode::Dev);
    }

    #[test]
    fn mode_is_packaged_predicate() {
        assert!(DaemonMode::PackagedApp.is_packaged());
        assert!(DaemonMode::PackagedHelper.is_packaged());
        assert!(!DaemonMode::Dev.is_packaged());
        assert!(!DaemonMode::Test.is_packaged());
    }

    #[test]
    fn resolve_paths_packaged_app_uses_application_support() {
        // Clear env overrides that would mask the default.
        let prev_db = std::env::var("DATABASE_URL").ok();
        let prev_auth = std::env::var("CHAINWORKS_AUTH_PRINCIPALS_PATH").ok();
        std::env::remove_var("DATABASE_URL");
        std::env::remove_var("CHAINWORKS_AUTH_PRINCIPALS_PATH");
        let paths = resolve_paths(DaemonMode::PackagedApp).unwrap();
        // Restore.
        if let Some(v) = prev_db {
            std::env::set_var("DATABASE_URL", v);
        }
        if let Some(v) = prev_auth {
            std::env::set_var("CHAINWORKS_AUTH_PRINCIPALS_PATH", v);
        }
        assert!(paths
            .app_support_dir
            .to_string_lossy()
            .contains("Application Support/Chainworks Forge"));
        assert!(paths
            .database_url
            .contains("Application Support/Chainworks Forge"));
        assert!(paths.log_path.is_some());
    }

    #[test]
    fn enforce_loopback_rewrites_packaged_non_loopback() {
        let mut paths = ModePaths {
            mode: DaemonMode::PackagedApp,
            database_url: "sqlite::memory:".into(),
            principals_path: PathBuf::from("/tmp/p.json"),
            app_support_dir: PathBuf::from("/tmp"),
            log_path: None,
            bind_addr: "0.0.0.0:4000".into(),
        };
        enforce_loopback(&mut paths);
        assert_eq!(paths.bind_addr, "127.0.0.1:4000");
    }

    #[test]
    fn enforce_loopback_leaves_dev_mode_alone() {
        let mut paths = ModePaths {
            mode: DaemonMode::Dev,
            database_url: "sqlite::memory:".into(),
            principals_path: PathBuf::from("/tmp/p.json"),
            app_support_dir: PathBuf::from("/tmp"),
            log_path: None,
            bind_addr: "0.0.0.0:4000".into(),
        };
        enforce_loopback(&mut paths);
        assert_eq!(paths.bind_addr, "0.0.0.0:4000");
    }

    #[tokio::test]
    async fn bind_with_fallback_writes_daemon_port_file() {
        let dir = tempfile::tempdir().unwrap();
        let paths = ModePaths {
            mode: DaemonMode::Test,
            database_url: "sqlite::memory:".into(),
            principals_path: PathBuf::from("/tmp/p.json"),
            app_support_dir: dir.path().to_path_buf(),
            log_path: None,
            bind_addr: "127.0.0.1:0".into(),
        };
        let (listener, port) = bind_with_fallback(&paths).await.unwrap();
        assert!(port > 0);
        let contents = std::fs::read_to_string(paths.daemon_port_path()).unwrap();
        assert_eq!(contents, port.to_string());
        drop(listener);
    }

    #[test]
    fn write_daemon_endpoint_snapshot_persists_pid_port_and_sha() {
        let dir = tempfile::tempdir().unwrap();
        let paths = ModePaths {
            mode: DaemonMode::Test,
            database_url: "sqlite::memory:".into(),
            principals_path: PathBuf::from("/tmp/p.json"),
            app_support_dir: dir.path().to_path_buf(),
            log_path: None,
            bind_addr: "127.0.0.1:0".into(),
        };

        write_daemon_endpoint_snapshot(&paths, 4242, 64446, "65a2f5d3").unwrap();

        let value: serde_json::Value =
            serde_json::from_slice(&std::fs::read(paths.daemon_endpoint_path()).unwrap()).unwrap();
        assert_eq!(value["pid"], 4242);
        assert_eq!(value["port"], 64446);
        assert_eq!(value["build_sha"], "65a2f5d3");
    }

    #[test]
    fn write_build_sha_creates_file_with_content() {
        let dir = tempfile::tempdir().unwrap();
        let paths = ModePaths {
            mode: DaemonMode::Test,
            database_url: "sqlite::memory:".into(),
            principals_path: PathBuf::from("/tmp/p.json"),
            app_support_dir: dir.path().to_path_buf(),
            log_path: None,
            bind_addr: "127.0.0.1:0".into(),
        };
        write_build_sha(&paths).unwrap();
        let contents = std::fs::read_to_string(paths.build_sha_path()).unwrap();
        assert!(!contents.is_empty());
    }

    #[test]
    fn write_build_sha_value_roundtrips_caller_supplied_sha() {
        // R12 OPS-001: the embed-daemon script now exports GIT_SHA
        // BEFORE cargo build, so packaged daemons carry a real SHA.
        // Tests can't exercise the compile-time literal, but they can
        // pin the on-disk write contract via the explicit-value seam.
        let dir = tempfile::tempdir().unwrap();
        let paths = ModePaths {
            mode: DaemonMode::Test,
            database_url: "sqlite::memory:".into(),
            principals_path: PathBuf::from("/tmp/p.json"),
            app_support_dir: dir.path().to_path_buf(),
            log_path: None,
            bind_addr: "127.0.0.1:0".into(),
        };
        write_build_sha_value(&paths, "abcdef1").unwrap();
        let contents = std::fs::read_to_string(paths.build_sha_path()).unwrap();
        assert_eq!(contents, "abcdef1");
    }

    /// R13 OPS-002: packaged modes pick `$HOME` as the chdir target.
    #[test]
    fn packaged_cwd_target_returns_home_for_packaged_modes() {
        let home = PathBuf::from("/Users/op");
        assert_eq!(
            packaged_cwd_target(DaemonMode::PackagedApp, Some(home.clone())),
            Some(home.clone()),
            "PackagedApp must chdir to $HOME to enforce the zero-source-checkout-reads contract"
        );
        assert_eq!(
            packaged_cwd_target(DaemonMode::PackagedHelper, Some(home.clone())),
            Some(home.clone()),
            "PackagedHelper inherits the same contract as the main packaged app"
        );
    }

    /// Dev / Test / MCP modes keep their launcher cwd so `cargo run`
    /// / `cargo test` / MCP stdio adapters can resolve relative paths
    /// against the repo root.
    #[test]
    fn packaged_cwd_target_is_none_for_non_packaged_modes() {
        let home = Some(PathBuf::from("/Users/op"));
        assert!(packaged_cwd_target(DaemonMode::Dev, home.clone()).is_none());
        assert!(packaged_cwd_target(DaemonMode::Test, home.clone()).is_none());
        assert!(packaged_cwd_target(DaemonMode::Mcp, home).is_none());
    }

    /// If the OS couldn't resolve `$HOME`, packaged mode still gets
    /// `None` — `main.rs` logs a warning and continues from the
    /// launcher cwd rather than crashing the daemon at startup.
    #[test]
    fn packaged_cwd_target_is_none_when_home_is_unknown() {
        assert!(packaged_cwd_target(DaemonMode::PackagedApp, None).is_none());
        assert!(packaged_cwd_target(DaemonMode::PackagedHelper, None).is_none());
    }

    #[test]
    fn bundled_resource_path_from_exe_resolves_contents_resources() {
        let dir = tempfile::tempdir().unwrap();
        let macos_dir = dir.path().join("Chainworks Forge.app/Contents/MacOS");
        let resources_dir = dir.path().join("Chainworks Forge.app/Contents/Resources");
        std::fs::create_dir_all(&macos_dir).unwrap();
        std::fs::create_dir_all(&resources_dir).unwrap();
        let exe = macos_dir.join("chainworks-forge-daemon");
        let catalog = resources_dir.join("agents.yaml");
        std::fs::write(&exe, "").unwrap();
        std::fs::write(&catalog, "agents: []").unwrap();

        assert_eq!(
            bundled_resource_path_from_exe(&exe, "agents.yaml"),
            Some(catalog)
        );
    }

    /// Both runtime-override and fallback paths exercised in a single
    /// serialized test. `cargo test` runs tests in parallel by
    /// default, and both cases mutate the process-global `GIT_SHA`
    /// env var; splitting them into two tests lets one race ahead and
    /// observe the other's in-flight state. Keeping them in one
    /// function guarantees ordering.
    #[test]
    fn resolved_build_sha_honors_runtime_override_and_dev_fallback() {
        let prev = std::env::var("GIT_SHA").ok();

        // ── 1. Runtime override wins over compile-time literal ──────
        std::env::set_var("GIT_SHA", "runtime-override-value");
        assert_eq!(
            resolved_build_sha(),
            "runtime-override-value",
            "runtime GIT_SHA must take precedence over the compile-time literal"
        );

        // ── 2. Empty runtime env falls back to compile-time / dev ───
        std::env::remove_var("GIT_SHA");
        let resolved = resolved_build_sha();
        assert!(
            resolved == "dev" || resolved == COMPILE_TIME_BUILD_SHA.unwrap_or(""),
            "fallback must be `dev` (when COMPILE_TIME_BUILD_SHA is None) \
             or the compile-time literal; got {resolved:?}"
        );

        // Restore original env so neighbours in this process's test
        // harness see the same environment they started with.
        match prev {
            Some(v) => std::env::set_var("GIT_SHA", v),
            None => std::env::remove_var("GIT_SHA"),
        }
    }
}
