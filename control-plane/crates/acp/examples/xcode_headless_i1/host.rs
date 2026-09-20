use anyhow::{ensure, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ServiceGeneration {
    pub uid: u32,
    pub pid: i32,
    pub start_sec: u64,
    pub start_usec: u64,
    pub developer_dir: PathBuf,
    pub executable: PathBuf,
    pub bundle_id: String,
    pub build: String,
}
pub struct HostExpectation {
    pub uid: u32,
    pub developer_dir: PathBuf,
    pub executable: PathBuf,
    pub bundle_id: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HostFacts {
    pub generation: ServiceGeneration,
    pub xcode_build: String,
    pub ide_absent: bool,
    pub enabled: bool,
    pub running: bool,
    pub unsafe_allow_all: bool,
    pub launch_identity: Value,
}
#[async_trait::async_trait]
pub trait HostInspector: Send {
    async fn inspect(&mut self) -> Result<HostFacts>;
}
pub fn select_service(
    expected: &HostExpectation,
    candidates: &[ServiceGeneration],
) -> Result<ServiceGeneration> {
    let matching: Vec<_> = candidates
        .iter()
        .filter(|c| {
            c.uid == expected.uid
                && c.developer_dir == expected.developer_dir
                && c.executable == expected.executable
                && c.bundle_id == expected.bundle_id
                && c.pid > 0
                && c.start_sec > 0
                && c.start_usec < 1_000_000
                && !c.build.is_empty()
        })
        .collect();
    ensure!(matching.len() == 1, "i1_service_identity");
    Ok(matching[0].clone())
}
pub fn validate_host(facts: &HostFacts) -> Result<()> {
    ensure!(
        facts.ide_absent && facts.enabled && facts.running && !facts.unsafe_allow_all,
        "i1_host_unavailable"
    );
    ensure!(facts.xcode_build == "27A266a", "i1_xcode_version");
    ensure!(
        facts.generation.pid > 0 && facts.generation.start_sec > 0,
        "i1_process_unknown"
    );
    Ok(())
}

pub fn command(path: &Path) -> Result<tokio::process::Command> {
    let mut cmd = tokio::process::Command::new(path);
    cmd.env_clear().env("PATH", "/usr/bin:/bin:/usr/sbin:/sbin");
    #[cfg(target_os = "macos")]
    {
        let (home, account) = native::account()?;
        cmd.env("HOME", home)
            .env("USER", &account)
            .env("LOGNAME", account)
            .env("TMPDIR", std::env::temp_dir().canonicalize()?);
    }
    cmd.kill_on_drop(true);
    acp::transport::isolate_process_group(&mut cmd);
    Ok(cmd)
}

// Both pipes are drained concurrently under one absolute deadline.
pub async fn metadata(path: &Path, args: &[&std::ffi::OsStr]) -> Result<(bool, Vec<u8>, Vec<u8>)> {
    use std::process::Stdio;
    use tokio::io::AsyncReadExt;
    let mut child = command(path)?
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let mut stdout = child.stdout.take().unwrap().take(1_048_577);
    let mut stderr = child.stderr.take().unwrap().take(1_048_577);
    let mut out = Vec::new();
    let mut err = Vec::new();
    let result = tokio::time::timeout(std::time::Duration::from_secs(5), async {
        let (_, _, status) = tokio::try_join!(
            stdout.read_to_end(&mut out),
            stderr.read_to_end(&mut err),
            child.wait()
        )?;
        ensure!(
            out.len() <= 1_048_576 && err.len() <= 1_048_576,
            "i1_metadata_size"
        );
        Ok::<_, anyhow::Error>((status.success(), out, err))
    })
    .await;
    match result {
        Ok(Ok(v)) => Ok(v),
        _ => {
            let _ = child.start_kill();
            let _ = tokio::time::timeout(std::time::Duration::from_secs(5), child.wait()).await;
            anyhow::bail!("i1_metadata_unavailable")
        }
    }
}

pub struct LiveHost;

fn status_flags(status: &Value) -> Result<(bool, bool, bool)> {
    // The installed CLI nests access flags; missing values remain unknown.
    let boolean = |pointer| {
        status
            .pointer(pointer)
            .and_then(Value::as_bool)
            .ok_or_else(|| anyhow::anyhow!("i1_status_shape"))
    };
    Ok((
        boolean("/permission/enabled")?,
        boolean("/running")?,
        boolean("/permission/unsafeAlwaysAllowAllAgents")?,
    ))
}

impl LiveHost {
    pub fn new() -> Result<Self> {
        ensure!(cfg!(target_os = "macos"), "i1_unsupported_host");
        Ok(Self)
    }
}
#[async_trait::async_trait]
impl HostInspector for LiveHost {
    async fn inspect(&mut self) -> Result<HostFacts> {
        #[cfg(target_os = "macos")]
        {
            native::inspect().await
        }
        #[cfg(not(target_os = "macos"))]
        {
            anyhow::bail!("i1_unsupported_host")
        }
    }
}

#[cfg(target_os = "macos")]
mod native {
    use super::*;
    use sha2::{Digest, Sha256};
    use std::{
        ffi::{CStr, OsStr},
        fs::File,
        io::Read,
        mem,
    };
    pub fn account() -> Result<(PathBuf, String)> {
        let mut record = mem::MaybeUninit::<libc::passwd>::zeroed();
        let mut buffer = vec![0i8; 65_536];
        let mut found = std::ptr::null_mut();
        // libc writes pointers into the live buffer; copy strings before it is dropped.
        unsafe {
            ensure!(
                libc::getpwuid_r(
                    libc::geteuid(),
                    record.as_mut_ptr(),
                    buffer.as_mut_ptr(),
                    buffer.len(),
                    &mut found
                ) == 0
                    && !found.is_null(),
                "i1_account_unknown"
            );
            let p = record.assume_init();
            Ok((
                PathBuf::from(CStr::from_ptr(p.pw_dir).to_str()?).canonicalize()?,
                CStr::from_ptr(p.pw_name).to_str()?.into(),
            ))
        }
    }
    fn process(pid: i32) -> Result<(PathBuf, libc::proc_bsdinfo)> {
        let mut path = vec![0u8; 4096];
        let mut info = mem::MaybeUninit::<libc::proc_bsdinfo>::zeroed();
        unsafe {
            let size = libc::proc_pidpath(pid, path.as_mut_ptr().cast(), path.len() as u32);
            ensure!(size > 0, "i1_process_unreadable");
            ensure!(
                libc::proc_pidinfo(
                    pid,
                    libc::PROC_PIDTBSDINFO,
                    0,
                    info.as_mut_ptr().cast(),
                    mem::size_of::<libc::proc_bsdinfo>() as i32
                ) == mem::size_of::<libc::proc_bsdinfo>() as i32,
                "i1_process_unreadable"
            );
            let executable =
                PathBuf::from(CStr::from_ptr(path.as_ptr().cast()).to_str()?).canonicalize()?;
            Ok((executable, info.assume_init()))
        }
    }
    async fn plist(file: &Path, key: &str) -> Result<String> {
        let (ok, out, _) = metadata(
            Path::new("/usr/bin/plutil"),
            &[
                OsStr::new("-extract"),
                OsStr::new(key),
                OsStr::new("raw"),
                file.as_os_str(),
            ],
        )
        .await?;
        ensure!(ok, "i1_bundle_unknown");
        Ok(std::str::from_utf8(&out)?.trim().into())
    }
    async fn executable_identity(path: &Path) -> Result<Value> {
        let mut file = File::open(path)?;
        let mut hash = Sha256::new();
        let mut block = [0u8; 65536];
        loop {
            let n = file.read(&mut block)?;
            if n == 0 {
                break;
            }
            hash.update(&block[..n]);
        }
        let (ok, out, err) = metadata(
            Path::new("/usr/bin/codesign"),
            &[
                OsStr::new("-dv"),
                OsStr::new("--verbose=2"),
                path.as_os_str(),
            ],
        )
        .await?;
        let signature = String::from_utf8_lossy(&err);
        let category = if ok {
            if signature.contains("Signature=adhoc") {
                "ad_hoc"
            } else {
                "signed"
            }
        } else if signature.contains("code object is not signed at all") {
            "unsigned"
        } else {
            anyhow::bail!("i1_signing_unknown")
        };
        Ok(
            serde_json::json!({"path":path,"sha256":format!("{:x}",hash.finalize()),
            "signing":category,"signature_sha256":super::super::project::digest(&[out,err].concat())}),
        )
    }
    pub async fn inspect() -> Result<HostFacts> {
        let developer = if let Some(value) = std::env::var_os("DEVELOPER_DIR") {
            PathBuf::from(value)
        } else {
            let (ok, out, _) =
                metadata(Path::new("/usr/bin/xcode-select"), &[OsStr::new("-p")]).await?;
            ensure!(ok, "i1_installation_unknown");
            PathBuf::from(std::str::from_utf8(&out)?.trim())
        }
        .canonicalize()?;
        let service_bundle = developer.join("Library/Xcode/Agents/Xcode Service.app/Contents");
        let executable = service_bundle.join("MacOS/Xcode Service").canonicalize()?;
        let bundle_id = plist(&service_bundle.join("Info.plist"), "CFBundleIdentifier").await?;
        ensure!(bundle_id == "com.apple.dt.mcp-server", "i1_bundle_identity");
        let build = plist(&service_bundle.join("Info.plist"), "CFBundleVersion").await?;
        let xcode_build = plist(
            &developer
                .parent()
                .ok_or_else(|| anyhow::anyhow!("i1_installation_unknown"))?
                .join("version.plist"),
            "ProductBuildVersion",
        )
        .await?;
        let uid = unsafe { libc::geteuid() };
        let expected = HostExpectation {
            uid,
            developer_dir: developer.clone(),
            executable: executable.clone(),
            bundle_id: bundle_id.clone(),
        };
        let (ok, out, _) = metadata(
            Path::new("/bin/ps"),
            &[OsStr::new("-axo"), OsStr::new("pid=,comm=")],
        )
        .await?;
        ensure!(ok, "i1_process_inventory_unknown");
        let mut candidates = Vec::new();
        let mut ide_absent = true;
        for line in std::str::from_utf8(&out)?.lines() {
            let line = line.trim();
            let Some((pid, name)) = line.split_once(char::is_whitespace) else {
                continue;
            };
            let name = name.trim();
            if !name.ends_with("/Xcode") && !name.ends_with("/Xcode Service") {
                continue;
            }
            let pid: i32 = pid.parse()?;
            let (path, info) = process(pid)?;
            if path.file_name() == Some(OsStr::new("Xcode")) {
                ide_absent = false;
            }
            if path == executable {
                candidates.push(ServiceGeneration {
                    uid: info.pbi_uid,
                    pid,
                    start_sec: info.pbi_start_tvsec,
                    start_usec: info.pbi_start_tvusec,
                    developer_dir: developer.clone(),
                    executable: path,
                    bundle_id: bundle_id.clone(),
                    build: build.clone(),
                });
            }
        }
        let generation = select_service(&expected, &candidates)?;
        let (ok, out, _) = metadata(
            &developer.join("usr/bin/mcp-server"),
            &[
                OsStr::new("status"),
                OsStr::new("--format"),
                OsStr::new("json"),
            ],
        )
        .await?;
        ensure!(ok, "i1_status_unknown");
        let status: Value = serde_json::from_slice(&out)?;
        let (enabled, running, unsafe_allow_all) = status_flags(&status)?;
        let mut chain = Vec::new();
        let mut pid = unsafe { libc::getppid() };
        for _ in 0..16 {
            if pid <= 1 {
                break;
            }
            let (path, info) = process(pid)?;
            chain.push(executable_identity(&path).await?);
            ensure!(info.pbi_ppid != pid as u32, "i1_launcher_cycle");
            pid = info.pbi_ppid as i32;
        }
        ensure!(pid <= 1, "i1_launcher_depth");
        let launch_identity = serde_json::json!({"uid":uid,
            "harness":executable_identity(&std::env::current_exe()?.canonicalize()?).await?,
            "bridge":executable_identity(&developer.join("usr/bin/mcpbridge").canonicalize()?).await?,
            "launchers":chain});
        let facts = HostFacts {
            generation,
            xcode_build,
            ide_absent,
            enabled,
            running,
            unsafe_allow_all,
            launch_identity,
        };
        validate_host(&facts)?;
        Ok(facts)
    }
}

#[cfg(test)]
pub fn fixture_facts() -> HostFacts {
    HostFacts {
        generation: ServiceGeneration {
            uid: 1001,
            pid: 42,
            start_sec: 1,
            start_usec: 0,
            developer_dir: "/fixture/Developer".into(),
            executable: "/fixture/Developer/Xcode Service".into(),
            bundle_id: "com.apple.dt.mcp-server".into(),
            build: "fixture".into(),
        },
        xcode_build: "27A266a".into(),
        ide_absent: true,
        enabled: true,
        running: true,
        unsafe_allow_all: false,
        launch_identity: serde_json::json!({"fixture":true}),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn i1_status_uses_observed_nested_permission_flags() {
        let status = serde_json::json!({
            "openWorkspaces": [],
            "permission": {"enabled": true, "unsafeAlwaysAllowAllAgents": false},
            "running": true
        });
        assert_eq!(status_flags(&status).unwrap(), (true, true, false));
        for (pointer, expected) in [
            ("/permission/enabled", (false, true, false)),
            ("/running", (true, false, false)),
            ("/permission/unsafeAlwaysAllowAllAgents", (true, true, true)),
        ] {
            let mut changed = status.clone();
            let value = changed.pointer_mut(pointer).unwrap();
            *value = Value::Bool(!value.as_bool().unwrap());
            assert_eq!(status_flags(&changed).unwrap(), expected);
            let mut facts = fixture_facts();
            (facts.enabled, facts.running, facts.unsafe_allow_all) = expected;
            assert!(validate_host(&facts).is_err());
        }
    }
    #[test]
    fn i1_status_rejects_missing_or_malformed_flags_without_flat_fallback() {
        let status = serde_json::json!({
            "permission": {"enabled": true, "unsafeAlwaysAllowAllAgents": false},
            "running": true
        });
        for pointer in [
            "/permission",
            "/permission/enabled",
            "/permission/unsafeAlwaysAllowAllAgents",
            "/running",
        ] {
            for value in [Value::Null, Value::String("true".into()), Value::from(1)] {
                let mut changed = status.clone();
                *changed.pointer_mut(pointer).unwrap() = value;
                assert!(status_flags(&changed).is_err(), "{pointer}");
            }
        }
        assert!(status_flags(&serde_json::json!({
            "enabled": true, "running": true, "unsafeAlwaysAllowAllAgents": false
        }))
        .is_err());
        assert!(status_flags(&serde_json::json!({"running": true})).is_err());
    }
    #[test]
    fn i1_host_rejects_unavailable_or_changed_facts() {
        let good = fixture_facts();
        validate_host(&good).unwrap();
        for i in 0..5 {
            let mut f = good.clone();
            match i {
                0 => f.ide_absent = false,
                1 => f.enabled = false,
                2 => f.running = false,
                3 => f.unsafe_allow_all = true,
                _ => f.xcode_build = "changed".into(),
            }
            assert!(validate_host(&f).is_err(), "{i}");
        }
        let mut reused = good.clone();
        reused.generation.start_usec += 1;
        assert_ne!(reused, good);
    }
    #[test]
    fn i1_service_requires_independent_exact_identity() {
        let good = fixture_facts().generation;
        let expected = HostExpectation {
            uid: good.uid,
            developer_dir: good.developer_dir.clone(),
            executable: good.executable.clone(),
            bundle_id: good.bundle_id.clone(),
        };
        assert_eq!(
            select_service(&expected, std::slice::from_ref(&good)).unwrap(),
            good
        );
        for i in 0..5 {
            let mut bad = good.clone();
            match i {
                0 => bad.uid += 1,
                1 => bad.executable = "/wrong".into(),
                2 => bad.developer_dir = "/wrong".into(),
                3 => bad.start_sec = 0,
                _ => bad.bundle_id = "wrong".into(),
            }
            assert!(select_service(&expected, &[bad]).is_err(), "{i}");
        }
        assert!(select_service(&expected, &[good.clone(), good]).is_err());
        assert!(select_service(&expected, &[]).is_err());
    }
}
