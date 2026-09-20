use super::{
    controller::{category, public_report, run_sequence, CaseStatus},
    evidence::LocalRecorder,
    host::{metadata, HostFacts, HostInspector, LiveHost},
    project::{
        create_pair, digest, no_links_below, private_dir, seed_digests, validate_fixed_pair,
        write_new, FixturePair,
    },
    tools::LivePeerFactory,
};
use anyhow::{ensure, Result};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    ffi::OsString,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SourceIdentity {
    pub head: String,
    pub files: BTreeMap<String, String>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PreparedAttempt {
    pub version: u32,
    pub id: String,
    pub root: PathBuf,
    pub pair: FixturePair,
    pub seeds: BTreeMap<String, String>,
    pub source: SourceIdentity,
    pub evidence: PathBuf,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Inspection {
    pub attempt: PreparedAttempt,
    pub host: HostFacts,
}

enum Command {
    Usage,
    Prepare { scratch: PathBuf, evidence: PathBuf },
    Inspect { attempt: PathBuf },
    Run { attempt: PathBuf, approval: String },
}
fn parse(args: Vec<OsString>) -> Result<Command> {
    if args.is_empty() {
        return Ok(Command::Usage);
    }
    let name = args[0]
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("i1_cli_arguments"))?;
    ensure!(
        matches!(name, "prepare" | "inspect" | "run") && args.len() % 2 == 1,
        "i1_cli_arguments"
    );
    let mut flags = BTreeMap::new();
    for chunk in args[1..].chunks_exact(2) {
        let key = chunk[0]
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("i1_cli_arguments"))?;
        ensure!(
            flags.insert(key, chunk[1].clone()).is_none(),
            "i1_cli_arguments"
        );
    }
    let take = |flags: &mut BTreeMap<&str, OsString>, key| {
        flags
            .remove(key)
            .ok_or_else(|| anyhow::anyhow!("i1_cli_arguments"))
    };
    let cmd = match name {
        "prepare" => Command::Prepare {
            scratch: take(&mut flags, "--scratch-parent")?.into(),
            evidence: take(&mut flags, "--evidence-root")?.into(),
        },
        "inspect" => Command::Inspect {
            attempt: take(&mut flags, "--attempt-file")?.into(),
        },
        "run" => {
            let attempt = take(&mut flags, "--attempt-file")?.into();
            let approval = take(&mut flags, "--approved-plan-digest")?
                .into_string()
                .map_err(|_| anyhow::anyhow!("i1_cli_arguments"))?;
            ensure!(
                approval.len() == 64 && approval.bytes().all(|c| c.is_ascii_hexdigit()),
                "i1_approval_digest"
            );
            Command::Run { attempt, approval }
        }
        _ => unreachable!(),
    };
    ensure!(flags.is_empty(), "i1_cli_arguments");
    Ok(cmd)
}
pub async fn cli(args: Vec<OsString>) -> Result<()> {
    // Do not let io/serde errors disclose host paths or unexpected server text.
    execute(parse(args)?)
        .await
        .map_err(|e| anyhow::anyhow!(category(&e)))
}
async fn execute(command: Command) -> Result<()> {
    match command {
        Command::Usage => println!("I1 development experiment: prepare | inspect | run. Live run requires approval of exact inspection bytes."),
        Command::Prepare{scratch,evidence} => {
            let file = prepare(&scratch,&evidence,source_identity().await?)?;
            eprintln!("Private attempt file: {}",file.display());
            println!("Prepared; no host or MCP calls.");
        }
        Command::Inspect{attempt} => {
            let prepared = read_attempt(&attempt)?;
            ensure!(prepared.source==source_identity().await?,"i1_source_changed");
            ensure!(!prepared.root.join("control/record").exists(),"i1_already_attempted");
            let mut host = LiveHost::new()?;
            let facts = match host.inspect().await {
                Ok(facts) => facts,
                Err(error) => {
                    let blocked = serde_json::json!({"version":1,"status":"Blocked","reason":category(&error),
                        "error_sha256":digest(error.to_string().as_bytes()),"tools_calls":0});
                    write_new(&prepared.evidence.join("inspection-blocked.json"),&serde_json::to_vec_pretty(&blocked)?)?;
                    println!("{blocked}");
                    return Err(error);
                }
            };
            let inspection = Inspection{attempt:prepared.clone(),host:facts};
            let bytes = serde_json::to_vec_pretty(&inspection)?;
            let path = prepared.root.join("control/inspection.json");
            write_new(&path,&bytes)?;
            eprintln!("Private inspection: {}",path.display());
            for p in &prepared.pair.projects { eprintln!("Fixture {}: {}",p.label,p.package.display()); }
            println!("{}",serde_json::json!({"status":"inspection_ready","inspection_sha256":digest(&bytes),
                "launch_identity_sha256":digest(&serde_json::to_vec(&inspection.host.launch_identity)?),
                "xcode_build":inspection.host.xcode_build,"opens":2,"reads":6,"reconnects":1,
                "cleanup":"Fixtures remain open; no automatic retry or cleanup."}));
        }
        Command::Run{attempt,approval} => {
            let prepared = read_attempt(&attempt)?;
            ensure!(!prepared.root.join("control/record").exists(),"i1_already_attempted");
            let bytes = read_small(&prepared.root.join("control/inspection.json"))?;
            ensure!(digest(&bytes)==approval,"i1_approval_digest");
            let current = source_identity().await?;
            let mut host = LiveHost::new()?;
            let facts = host.inspect().await?;
            let inspection = validate_approval(&prepared,&bytes,&approval,&current,&facts)?;
            let record_dir = prepared.root.join("control/record");
            let mut record = LocalRecorder::create(&record_dir)?;
            let mut peers = LivePeerFactory::new(record_dir.clone());
            let mut report = run_sequence(&prepared.pair,&inspection.host,&mut host,&mut peers,&mut record).await?;
            report.unresolved_open |= LocalRecorder::unresolved(&record_dir).unwrap_or(true);
            if source_identity().await.ok().as_ref()!=Some(&prepared.source) {
                report.cases[7]=CaseStatus::Blocked; report.error=Some("i1_source_changed".into());
            }
            let added = super::project::added_metadata(&prepared.pair);
            if added.is_err() {
                report.cases[7]=CaseStatus::Blocked; report.error=Some("i1_metadata_unknown".into());
            }
            write_new(&prepared.root.join("control/report.json"),&serde_json::to_vec_pretty(&report)?)?;
            let mut public = public_report(&report);
            public["added_metadata"] = serde_json::to_value(added.ok())?;
            let result_bytes = serde_json::to_vec_pretty(&public)?;
            write_new(&prepared.evidence.join("report.json"),&result_bytes)?;
            let manifest = serde_json::json!({"version":1,"report.json":digest(&result_bytes),
                "source_sha256":digest(&serde_json::to_vec(&prepared.source)?),
                "inspection_sha256":approval,"fixture_seeds":prepared.seeds});
            write_new(&prepared.evidence.join("manifest.json"),&serde_json::to_vec_pretty(&manifest)?)?;
            let summary = format!("# I1 Experiment\n\nLive outcome: {:?}.\n\nOffline I1-01/I1-07 require separate evidence composition.\nNo confinement or production admission claim.\n\nSee report.json and manifest.json; private cleanup records retained.\n",super::controller::classify(&report));
            write_new(&prepared.evidence.join("experiment.md"),summary.as_bytes())?;
            println!("{public}");
            ensure!(report.error.is_none() && !report.unresolved_open,"i1_experiment_stopped");
        }
    }
    Ok(())
}
fn validate_approval(
    a: &PreparedAttempt,
    bytes: &[u8],
    approved_digest: &str,
    current: &SourceIdentity,
    host: &HostFacts,
) -> Result<Inspection> {
    ensure!(digest(bytes) == approved_digest, "i1_approval_digest");
    let inspection: Inspection = serde_json::from_slice(bytes)?;
    ensure!(
        inspection.attempt == *a && *current == a.source && inspection.host == *host,
        "i1_approval_changed"
    );
    ensure!(seed_digests(&a.pair)? == a.seeds, "i1_seed_changed");
    ensure!(
        !a.root.join("control/record").exists(),
        "i1_already_attempted"
    );
    Ok(inspection)
}
fn scratch_parent(scratch: &Path) -> Result<PathBuf> {
    let absolute = if scratch.is_absolute() {
        scratch.to_owned()
    } else {
        std::env::current_dir()?.join(scratch)
    };
    let mut component_path = PathBuf::new();
    for component in absolute.components() {
        ensure!(
            !matches!(component, std::path::Component::ParentDir),
            "i1_scratch_parent_component"
        );
        component_path.push(component);
        if matches!(component_path.to_str(), Some("/tmp" | "/var")) {
            continue;
        }
        ensure!(
            !std::fs::symlink_metadata(&component_path)?
                .file_type()
                .is_symlink(),
            "i1_scratch_symlink"
        );
    }
    let scratch = scratch.canonicalize()?;
    ensure!(scratch.is_dir(), "i1_scratch_missing");
    let source = source_root().canonicalize()?;
    ensure!(!scratch.starts_with(source), "i1_checkout_scratch");
    for parent in scratch.ancestors() {
        ensure!(
            !parent.join(".git").exists()
                && !matches!(
                    parent.file_name().and_then(|s| s.to_str()),
                    Some(".chainworks" | "worktrees")
                ),
            "i1_repository_scratch"
        );
    }
    Ok(scratch)
}
fn prepare(scratch: &Path, evidence: &Path, source: SourceIdentity) -> Result<PathBuf> {
    let scratch = scratch_parent(scratch)?;
    let evidence = create_evidence_parent(evidence)?;
    let id = uuid::Uuid::new_v4().to_string();
    let root = scratch.join(format!("cw-i1-{id}"));
    let pair = create_pair(&root)?;
    private_dir(&root.join("control"))?;
    let evidence = evidence.join(&id);
    private_dir(&evidence)?;
    let prepared = PreparedAttempt {
        version: 1,
        id,
        root: root.clone(),
        seeds: seed_digests(&pair)?,
        pair,
        source,
        evidence,
    };
    let path = root.join("control/attempt.json");
    write_new(&path, &serde_json::to_vec_pretty(&prepared)?)?;
    Ok(path)
}
fn read_small(path: &Path) -> Result<Vec<u8>> {
    use std::io::Read;
    let mut bytes = Vec::new();
    std::fs::File::open(path)?
        .take(1_048_577)
        .read_to_end(&mut bytes)?;
    ensure!(bytes.len() <= 1_048_576, "i1_record_size");
    Ok(bytes)
}
fn create_evidence_parent(path: &Path) -> Result<PathBuf> {
    let absolute = if path.is_absolute() {
        path.to_owned()
    } else {
        std::env::current_dir()?.join(path)
    };
    let mut current = PathBuf::new();
    for component in absolute.components() {
        if matches!(component, std::path::Component::ParentDir) {
            ensure!(current.pop(), "i1_evidence_parent_component");
            continue;
        }
        current.push(component);
        match std::fs::symlink_metadata(&current) {
            Ok(meta) => {
                ensure!(
                    meta.is_dir() || matches!(current.to_str(), Some("/tmp" | "/var")),
                    "i1_evidence_symlink"
                );
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => private_dir(&current)?,
            Err(e) => return Err(e.into()),
        }
    }
    Ok(absolute.canonicalize()?)
}
fn read_attempt(path: &Path) -> Result<PreparedAttempt> {
    let a: PreparedAttempt = serde_json::from_slice(&read_small(path)?)?;
    ensure!(
        a.version == 1 && uuid::Uuid::parse_str(&a.id).is_ok(),
        "i1_attempt_version"
    );
    ensure!(
        a.root.file_name().and_then(|s| s.to_str()) == Some(&format!("cw-i1-{}", a.id)),
        "i1_attempt_layout"
    );
    ensure!(
        a.root.canonicalize()? == a.root && a.root.parent().is_some(),
        "i1_attempt_layout"
    );
    scratch_parent(a.root.parent().unwrap())?;
    no_links_below(&a.root, &a.root.join("control/attempt.json"))?;
    ensure!(
        path.canonicalize()? == a.root.join("control/attempt.json"),
        "i1_attempt_copy"
    );
    ensure!(seed_digests(&a.pair)? == a.seeds, "i1_seed_changed");
    validate_fixed_pair(&a.root, &a.pair)?;
    for project in &a.pair.projects {
        let mut files = BTreeMap::new();
        collect_files(&project.root, &project.root, &mut files, false)?;
        ensure!(
            files.len() == super::project::SEED_PATHS.len(),
            "i1_unexpected_fixture_file"
        );
    }
    ensure!(
        a.evidence.is_absolute()
            && a.evidence.is_dir()
            && a.evidence.file_name().and_then(|s| s.to_str()) == Some(a.id.as_str()),
        "i1_evidence_layout"
    );
    Ok(a)
}
fn source_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(3)
        .unwrap()
}
fn collect_files(
    root: &Path,
    path: &Path,
    files: &mut BTreeMap<String, String>,
    source: bool,
) -> Result<()> {
    let meta = std::fs::symlink_metadata(path)?;
    ensure!(!meta.file_type().is_symlink(), "i1_source_symlink");
    if meta.is_dir() {
        for entry in std::fs::read_dir(path)? {
            let entry = entry?;
            if source && matches!(entry.file_name().to_str(), Some("target" | ".git")) {
                continue;
            }
            collect_files(root, &entry.path(), files, source)?;
        }
    } else {
        ensure!(
            meta.is_file() && meta.len() <= 16 * 1_048_576,
            "i1_source_size"
        );
        files.insert(
            path.strip_prefix(root)?.to_string_lossy().into(),
            digest(&std::fs::read(path)?),
        );
    }
    Ok(())
}
async fn source_identity() -> Result<SourceIdentity> {
    let root = source_root();
    let (ok, out, _) = metadata(
        Path::new("/usr/bin/git"),
        &[
            "-C".as_ref(),
            root.as_os_str(),
            "rev-parse".as_ref(),
            "HEAD".as_ref(),
        ],
    )
    .await?;
    ensure!(ok, "i1_source_head");
    let mut files = BTreeMap::new();
    collect_files(root, &root.join("control-plane"), &mut files, true)?;
    for relative in [
        "scripts/cargo-managed",
        "scripts/cargo-cache-env.sh",
        "docs/evidence/xcode-headless-contract-2026-09-19/mcp-2025-06-18.json",
        "docs/evidence/xcode-headless-contract-2026-09-19/mcp-2024-11-05.json",
    ] {
        collect_files(root, &root.join(relative), &mut files, true)?;
    }
    Ok(SourceIdentity {
        head: std::str::from_utf8(&out)?.trim().into(),
        files,
    })
}
#[cfg(test)]
mod tests {
    use super::super::{host::fixture_facts, project::digest};
    use super::*;
    fn source() -> SourceIdentity {
        SourceIdentity {
            head: "fixture".into(),
            files: BTreeMap::from([("a".into(), "b".into())]),
        }
    }
    #[tokio::test]
    async fn i1_cli_does_not_accept_arbitrary_tool_calls() {
        cli(vec![]).await.unwrap();
        for args in [
            vec!["run", "--tool", "XcodeWrite"],
            vec!["run", "--attempt-file", "/unused"],
            vec!["inspect", "--attempt-file", "/unused", "--pid", "42"],
            vec!["prepare", "--scratch-parent", "/tmp", "--resume", "true"],
        ] {
            assert!(cli(args.into_iter().map(OsString::from).collect())
                .await
                .is_err());
        }
    }
    #[test]
    fn i1_approval_binds_bytes_source_fixtures_and_stable_host() {
        let t = tempfile::tempdir().unwrap();
        let root = t.path().join("pair");
        let pair = create_pair(&root).unwrap();
        let a = PreparedAttempt {
            version: 1,
            id: "fixture".into(),
            root: root.canonicalize().unwrap(),
            seeds: seed_digests(&pair).unwrap(),
            pair,
            source: source(),
            evidence: t.path().join("evidence"),
        };
        let host = fixture_facts();
        let bytes = serde_json::to_vec(&Inspection {
            attempt: a.clone(),
            host: host.clone(),
        })
        .unwrap();
        validate_approval(&a, &bytes, &digest(&bytes), &source(), &host).unwrap();
        assert!(validate_approval(&a, &bytes, "incorrect", &source(), &host).is_err());
        let mut changed = source();
        changed.files.insert("new".into(), "changed".into());
        assert!(validate_approval(&a, &bytes, &digest(&bytes), &changed, &host).is_err());
        let mut changed = host.clone();
        changed.generation.start_usec += 1;
        assert!(validate_approval(&a, &bytes, &digest(&bytes), &source(), &changed).is_err());
        let mut changed = host.clone();
        changed.launch_identity = serde_json::json!({"changed":true});
        assert!(validate_approval(&a, &bytes, &digest(&bytes), &source(), &changed).is_err());
        std::fs::write(
            a.pair.projects[0].root.join("Sources/Sentinel.txt"),
            "edited",
        )
        .unwrap();
        assert!(validate_approval(&a, &bytes, &digest(&bytes), &source(), &host).is_err());
    }
    #[test]
    fn i1_prepare_is_local_and_record_path_cannot_be_redirected() {
        let t = tempfile::tempdir().unwrap();
        let evidence = t.path().join("evidence");
        std::fs::create_dir(&evidence).unwrap();
        let file = prepare(t.path(), &evidence, source()).unwrap();
        let a: PreparedAttempt = serde_json::from_slice(&std::fs::read(&file).unwrap()).unwrap();
        assert_eq!(a.pair.projects.len(), 2);
        assert!(file.starts_with(&a.root));
        assert!(!a.root.join("control/record").exists());
        assert_eq!(read_attempt(&file).unwrap(), a);
        let copied = t.path().join("copied.json");
        std::fs::copy(&file, &copied).unwrap();
        assert!(read_attempt(&copied).is_err());
        std::fs::create_dir(a.pair.projects[0].root.join("target")).unwrap();
        std::fs::write(a.pair.projects[0].root.join("target/extra"), "unexpected").unwrap();
        assert!(read_attempt(&file).is_err());
    }
    #[test]
    fn review_first_prepare_creates_missing_evidence_hierarchy() {
        let t = tempfile::tempdir().unwrap();
        let evidence = t.path().join("new/iteration-1");
        let file = prepare(t.path(), &evidence, source()).unwrap();
        let a = read_attempt(&file).unwrap();
        assert!(a.evidence.is_dir());
    }
}
