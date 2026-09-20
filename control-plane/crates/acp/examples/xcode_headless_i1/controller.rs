use super::{
    evidence::Recorder,
    host::{validate_host, HostFacts, HostInspector, ServiceGeneration},
    project::{digest, seed_digests, FixturePair},
    protocol::ExitRecord,
    tools::{
        mapping_status, verify_sentinel, MappingStatus, OpenResult, Peer, PeerFactory, ReadResult,
    },
};
use anyhow::{ensure, Result};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::time::{timeout_at, Instant};
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CaseStatus {
    Pass,
    Fail,
    Blocked,
    NotRun,
    Unverified,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Outcome {
    Supported,
    Partial,
    Refuted,
    Blocked,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct ExperimentalWorkspaceObservation {
    pub label: String,
    pub requested_package: std::path::PathBuf,
    pub generation: ServiceGeneration,
    pub opened: OpenResult,
    pub mapping: MappingStatus,
    pub reads: Vec<ReadResult>,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct Report {
    pub cases: [CaseStatus; 8],
    pub observations: Vec<ExperimentalWorkspaceObservation>,
    pub unresolved_open: bool,
    pub exits: Vec<ExitRecord>,
    pub error: Option<String>,
    pub deviations: Vec<super::evidence::Deviation>,
}
pub fn classify(report: &Report) -> Outcome {
    if report.cases.contains(&CaseStatus::Fail) {
        return Outcome::Refuted;
    }
    if report.cases.iter().all(|s| *s == CaseStatus::Pass)
        && !report.unresolved_open
        && report.error.is_none()
    {
        return Outcome::Supported;
    }
    if report.cases[4] == CaseStatus::Pass {
        Outcome::Partial
    } else {
        Outcome::Blocked
    }
}
pub fn category(error: &anyhow::Error) -> &'static str {
    match error.to_string().as_str() {
        "i1_mapping_mismatch" => "i1_mapping_mismatch",
        "i1_read_mismatch" => "i1_read_mismatch",
        "i1_duplicate_workspace_id" => "i1_duplicate_workspace_id",
        "i1_host_changed" => "i1_host_changed",
        "i1_seed_changed" => "i1_seed_changed",
        "i1_cancelled" => "i1_cancelled",
        "i1_timeout" => "i1_timeout",
        "i1_tool_error" => "i1_tool_error",
        "i1_rpc_error" => "i1_rpc_error",
        "i1_tool_shape" => "i1_tool_shape",
        "i1_conflicting_encoding" => "i1_conflicting_encoding",
        "i1_mapping_shape" => "i1_mapping_shape",
        "i1_read_shape" => "i1_read_shape",
        "i1_pending_invalidation" => "i1_pending_invalidation",
        "i1_notification" => "i1_notification",
        "i1_json_shape" => "i1_json_shape",
        "i1_rpc_envelope" => "i1_rpc_envelope",
        "i1_response_id" => "i1_response_id",
        _ => "i1_stopped",
    }
}
pub fn public_report(report: &Report) -> serde_json::Value {
    let observations:Vec<_> = report.observations.iter().enumerate().map(|(i,o)| serde_json::json!({
        "fixture":if o.label=="A" {"A"} else {"B"},"workspace_alias":format!("workspace-{}",i+1),
        "mapping":o.mapping,"verified_reads":o.reads.len(),
        "generation_sha256":digest(&serde_json::to_vec(&o.generation).unwrap_or_default())
    })).collect();
    serde_json::json!({"version":1,"outcome":classify(report),"cases":report.cases,
        "observations":observations,"unresolved_open":report.unresolved_open,"bridge_exits":report.exits,
        "deviations":report.deviations,
        "error":report.error.as_ref().map(|e|category(&anyhow::anyhow!(e.clone()))),
        "limitation":"Fixture routing only; no service confinement or production admission proof."})
}
async fn check_host(host: &mut dyn HostInspector, approved: &HostFacts) -> Result<()> {
    let observed = host.inspect().await?;
    validate_host(&observed)?;
    ensure!(&observed == approved, "i1_host_changed");
    Ok(())
}
pub async fn run_sequence(
    pair: &FixturePair,
    approved_host: &HostFacts,
    host: &mut dyn HostInspector,
    peers: &mut dyn PeerFactory,
    record: &mut dyn Recorder,
) -> Result<Report> {
    let deadline = Instant::now() + Duration::from_secs(300);
    let mut report = Report {
        cases: [CaseStatus::NotRun; 8],
        observations: vec![],
        unresolved_open: false,
        exits: vec![],
        error: None,
        deviations: vec![],
    };
    let mut peer: Option<Box<dyn Peer>> = None;
    let mut stage = 1usize;
    let initial = match seed_digests(pair) {
        Ok(seeds) => seeds,
        Err(_) => {
            report.cases[7] = CaseStatus::Blocked;
            report.error = Some("i1_seed_changed".into());
            return Ok(report);
        }
    };
    let sequence = async {
        check_host(host, approved_host).await?;
        peer = Some(peers.connect(approved_host, pair, deadline).await?);
        peer.as_mut().unwrap().initialize().await?;
        report.cases[1] = CaseStatus::Pass;
        stage = 2;
        for project in &pair.projects {
            check_host(host, approved_host).await?;
            ensure!(seed_digests(pair)? == initial, "i1_seed_changed");
            record.begin_open(project, &approved_host.generation)?;
            report.unresolved_open = true;
            let opened = match peer.as_mut().unwrap().open(project).await {
                Ok(opened) => opened,
                Err(error) => {
                    let _ = record.unknown_open(&project.label);
                    return Err(error);
                }
            };
            let mapping = mapping_status(project, &opened).map_err(|e|super::evidence::retain(e,
                &serde_json::json!({"workspaceIdentifier":opened.id,"workspacePath":opened.path})))?;
            ensure!(
                !report.observations.iter().any(|o| o.opened.id == opened.id),
                "i1_duplicate_workspace_id"
            );
            record.finish_open(&project.label, &opened)?;
            report.unresolved_open = false;
            report.observations.push(ExperimentalWorkspaceObservation {
                label: project.label.clone(),
                requested_package: project.package.clone(),
                generation: approved_host.generation.clone(),
                opened,
                mapping,
                reads: vec![],
            });
        }
        report.cases[2] = CaseStatus::Pass;
        report.cases[3] = if report
            .observations
            .iter()
            .all(|o| o.mapping == MappingStatus::Observed)
        {
            CaseStatus::Pass
        } else {
            CaseStatus::Unverified
        };
        for connection in 0..2 {
            stage = if connection == 0 { 4 } else { 5 };
            if connection == 1 {
                check_host(host, approved_host).await?;
                report.exits.push(peer.as_mut().unwrap().close().await?);
                peer.take();
                check_host(host, approved_host).await?;
                peer = Some(peers.connect(approved_host, pair, deadline).await?);
                peer.as_mut().unwrap().initialize().await?;
            }
            for index in [0, 1, 0] {
                check_host(host, approved_host).await?;
                ensure!(seed_digests(pair)? == initial, "i1_seed_changed");
                let observation = &mut report.observations[index];
                let result = peer
                    .as_mut()
                    .unwrap()
                    .read(&pair.projects[index], &observation.opened.id)
                    .await?;
                verify_sentinel(&pair.projects[index], &result).map_err(|e| {
                    super::evidence::retain(e, &serde_json::to_value(&result).unwrap_or_default())
                })?;
                observation.reads.push(result);
            }
            report.cases[stage] = CaseStatus::Pass;
        }
        stage = 7;
        check_host(host, approved_host).await?;
        ensure!(seed_digests(pair)? == initial, "i1_seed_changed");
        report.cases[7] = CaseStatus::Pass;
        Ok::<(), anyhow::Error>(())
    };
    let result = tokio::select! {
        result = timeout_at(deadline,sequence) => result.unwrap_or_else(|_|Err(anyhow::anyhow!("i1_timeout"))),
        _ = tokio::signal::ctrl_c() => Err(anyhow::anyhow!("i1_cancelled")),
    };
    if let Err(error) = result {
        if let Some(deviation) = error.downcast_ref::<super::evidence::DeviationError>() {
            let _ = record.deviation(&deviation.0);
            report.deviations.push(deviation.0.clone());
        }
        report.cases[1] = CaseStatus::Unverified;
        let reason = category(&error);
        if reason == "i1_mapping_mismatch" {
            stage = 3;
        }
        report.cases[stage] = if matches!(
            reason,
            "i1_mapping_mismatch" | "i1_read_mismatch" | "i1_duplicate_workspace_id"
        ) {
            CaseStatus::Fail
        } else {
            CaseStatus::Blocked
        };
        report.error = Some(reason.into());
    }
    if let Some(mut owned) = peer {
        match owned.close().await {
            Ok(exit) => report.exits.push(exit),
            Err(_) => {
                report.cases[1] = CaseStatus::Unverified;
                report.cases[7] = CaseStatus::Blocked;
                report.error = Some("i1_cleanup_unknown".into());
            }
        }
    }
    if report.error.is_none()
        && !matches!(
            timeout_at(deadline, check_host(host, approved_host)).await,
            Ok(Ok(()))
        )
    {
        report.cases[1] = CaseStatus::Blocked;
        report.cases[7] = CaseStatus::Blocked;
        report.error = Some("i1_post_host_unknown".into());
    }
    if seed_digests(pair).ok().as_ref() != Some(&initial) {
        report.cases[7] = CaseStatus::Blocked;
        report.error = Some("i1_seed_changed".into());
    }
    // Offline proofs are composed at experiment closeout, never inferred from a live happy path.
    Ok(report)
}
#[cfg(test)]
mod tests {
    use super::super::{
        host::fixture_facts,
        project::{create_pair, FixtureProject},
        tools::Peer,
    };
    use super::*;
    use std::sync::{Arc, Mutex};
    type Log = Arc<Mutex<Vec<String>>>;
    #[tokio::test]
    async fn review_real_storage_sync_faults_preserve_intent_and_stop_bytes() {
        use super::super::evidence::LocalRecorder;
        for fault in ["intent_file", "intent_dir", "result_file", "result_dir"] {
            let t = tempfile::tempdir().unwrap();
            let pair = create_pair(&t.path().join("pair")).unwrap();
            let facts = fixture_facts();
            let log = Log::default();
            let dir = t.path().join("record");
            let mut record = LocalRecorder::create(&dir).unwrap();
            record.sync_fault = Some(fault);
            let report = run_sequence(
                &pair,
                &facts,
                &mut Host {
                    facts: facts.clone(),
                    inspections: 0,
                    change_at: usize::MAX,
                },
                &mut Factory {
                    log: log.clone(),
                    failure: "",
                    connections: 0,
                },
                &mut record,
            )
            .await
            .unwrap();
            assert!(report.error.is_some(), "{fault}");
            assert_eq!(
                log.lock()
                    .unwrap()
                    .iter()
                    .filter(|s| s.starts_with("open:"))
                    .count(),
                if fault.starts_with("intent") { 0 } else { 1 },
                "{fault}"
            );
            assert!(std::fs::metadata(dir.join("A.intent.json")).unwrap().len() > 0);
            assert!(LocalRecorder::unresolved(&dir).unwrap(), "{fault}");
            drop(record);
            assert!(LocalRecorder::create(&dir).is_err());
        }
    }
    struct Host {
        facts: HostFacts,
        inspections: usize,
        change_at: usize,
    }
    #[async_trait::async_trait]
    impl HostInspector for Host {
        async fn inspect(&mut self) -> Result<HostFacts> {
            self.inspections += 1;
            let mut f = self.facts.clone();
            if self.inspections >= self.change_at {
                f.generation.start_usec += 1;
            }
            Ok(f)
        }
    }
    struct Factory {
        log: Log,
        failure: &'static str,
        connections: usize,
    }
    struct FakePeer {
        log: Log,
        failure: &'static str,
        number: usize,
    }
    #[async_trait::async_trait]
    impl PeerFactory for Factory {
        async fn connect(
            &mut self,
            _: &HostFacts,
            _: &FixturePair,
            _: tokio::time::Instant,
        ) -> Result<Box<dyn Peer>> {
            self.connections += 1;
            self.log.lock().unwrap().push("connect".into());
            if self.failure == "reconnect" && self.connections == 2 {
                anyhow::bail!("foreign secret");
            }
            Ok(Box::new(FakePeer {
                log: self.log.clone(),
                failure: self.failure,
                number: self.connections,
            }))
        }
    }
    #[async_trait::async_trait]
    impl Peer for FakePeer {
        async fn initialize(&mut self) -> Result<()> {
            self.log.lock().unwrap().push("initialize".into());
            Ok(())
        }
        async fn open(&mut self, p: &FixtureProject) -> Result<OpenResult> {
            self.log.lock().unwrap().push(format!("open:{}", p.label));
            if matches!(self.failure, "lost" | "partial_write" | "malformed") {
                anyhow::bail!("foreign secret");
            }
            if self.failure == "pending" {
                std::future::pending::<()>().await;
            }
            Ok(OpenResult {
                id: if self.failure == "duplicate" {
                    "same".into()
                } else {
                    p.label.clone()
                },
                path: if self.failure == "wrong_mapping" {
                    Some(
                        p.root
                            .parent()
                            .unwrap()
                            .join("B/Fixture.xcodeproj")
                            .to_string_lossy()
                            .into(),
                    )
                } else if self.failure == "unmapped" {
                    None
                } else {
                    Some(p.package.to_string_lossy().into())
                },
            })
        }
        async fn read(&mut self, p: &FixtureProject, id: &str) -> Result<ReadResult> {
            self.log
                .lock()
                .unwrap()
                .push(format!("read:{}:{id}:{}", p.label, self.number));
            Ok(ReadResult {
                content: format!(
                    "     1\t{}\n     2\t",
                    if self.failure == "wrong_content" {
                        "foreign secret"
                    } else {
                        &p.marker
                    }
                ),
                file_path: p.read_path.clone(),
                file_size: 8,
                total_lines: 2,
                lines_read: 2,
                start_line: 1,
            })
        }
        async fn close(&mut self) -> Result<ExitRecord> {
            self.log.lock().unwrap().push("close".into());
            Ok(ExitRecord {
                code: Some(1),
                forced: false,
            })
        }
    }
    struct Record {
        log: Log,
        failure: &'static str,
    }
    impl Recorder for Record {
        fn begin_open(&mut self, p: &FixtureProject, _: &ServiceGeneration) -> Result<()> {
            self.log.lock().unwrap().push(format!("intent:{}", p.label));
            if self.failure == "intent" {
                anyhow::bail!("disk secret");
            }
            Ok(())
        }
        fn finish_open(&mut self, label: &str, _: &OpenResult) -> Result<()> {
            self.log.lock().unwrap().push(format!("terminal:{label}"));
            if self.failure == "terminal" {
                anyhow::bail!("disk secret");
            }
            Ok(())
        }
        fn unknown_open(&mut self, label: &str) -> Result<()> {
            self.log.lock().unwrap().push(format!("unknown:{label}"));
            Ok(())
        }
    }
    async fn scenario(failure: &'static str, change_at: usize) -> (Report, Vec<String>) {
        let t = tempfile::tempdir().unwrap();
        let pair = create_pair(&t.path().join("pair")).unwrap();
        let facts = fixture_facts();
        let log = Log::default();
        let report = run_sequence(
            &pair,
            &facts,
            &mut Host {
                facts: facts.clone(),
                inspections: 0,
                change_at,
            },
            &mut Factory {
                log: log.clone(),
                failure,
                connections: 0,
            },
            &mut Record {
                log: log.clone(),
                failure,
            },
        )
        .await
        .unwrap();
        let entries = log.lock().unwrap().clone();
        (report, entries)
    }
    #[tokio::test]
    async fn i1_two_opens_and_explicit_reads_survive_reconnect_without_reopen() {
        let (report, log) = scenario("", usize::MAX).await;
        assert_eq!(
            log,
            [
                "connect",
                "initialize",
                "intent:A",
                "open:A",
                "terminal:A",
                "intent:B",
                "open:B",
                "terminal:B",
                "read:A:A:1",
                "read:B:B:1",
                "read:A:A:1",
                "close",
                "connect",
                "initialize",
                "read:A:A:2",
                "read:B:B:2",
                "read:A:A:2",
                "close"
            ]
        );
        assert_eq!(report.cases[5], CaseStatus::Pass);
        assert!(!report.unresolved_open);
    }
    #[tokio::test]
    async fn i1_failures_stop_dispatch_and_preserve_uncertainty() {
        for failure in [
            "intent",
            "terminal",
            "lost",
            "partial_write",
            "malformed",
            "duplicate",
            "wrong_content",
            "reconnect",
        ] {
            let (report, log) = scenario(failure, usize::MAX).await;
            assert!(report.error.is_some(), "{failure}");
            assert_ne!(
                report.cases[1],
                CaseStatus::Pass,
                "unobserved ending host: {failure}"
            );
            let opens = log.iter().filter(|v| v.starts_with("open:")).count();
            assert_eq!(
                opens,
                if failure == "intent" {
                    0
                } else if matches!(failure, "terminal" | "lost" | "partial_write" | "malformed") {
                    1
                } else {
                    2
                },
                "{failure}"
            );
            assert!(matches!(
                log.last().map(String::as_str),
                Some("close" | "connect")
            ));
            assert_ne!(classify(&report), Outcome::Supported);
        }
        let (report, log) = scenario("", 1).await;
        assert!(report.error.is_some());
        assert!(!log.iter().any(|v| v.starts_with("open:")));
    }
    #[test]
    fn i1_no_default_supported_verdict() {
        let mut r = Report {
            cases: [CaseStatus::NotRun; 8],
            observations: vec![],
            unresolved_open: false,
            exits: vec![],
            error: None,
            deviations: vec![],
        };
        assert_eq!(classify(&r), Outcome::Blocked);
        r.cases = [CaseStatus::Pass; 8];
        assert_eq!(classify(&r), Outcome::Supported);
        r.unresolved_open = true;
        assert_ne!(classify(&r), Outcome::Supported);
    }
    #[tokio::test]
    async fn i1_missing_mapping_and_public_redaction() {
        let (mut report, log) = scenario("unmapped", usize::MAX).await;
        assert_eq!(report.cases[3], CaseStatus::Unverified);
        assert_eq!(classify(&report), Outcome::Partial);
        assert_eq!(log.iter().filter(|v| v.starts_with("open:")).count(), 2);
        report.error = Some("/foreign/project secret-token".into());
        report.observations[0].opened.id = "secret-token".into();
        report.observations[0].requested_package = "/foreign/project".into();
        let public = public_report(&report).to_string();
        assert!(!public.contains("foreign"));
        assert!(!public.contains("secret-token"));
    }
    #[tokio::test]
    async fn review_mapping_deviation_is_durable_without_foreign_paths() {
        use super::super::evidence::LocalRecorder;
        let t = tempfile::tempdir().unwrap();
        let pair = create_pair(&t.path().join("pair")).unwrap();
        let facts = fixture_facts();
        let log = Log::default();
        let dir = t.path().join("record");
        let mut record = LocalRecorder::create(&dir).unwrap();
        let report = run_sequence(
            &pair,
            &facts,
            &mut Host {
                facts: facts.clone(),
                inspections: 0,
                change_at: usize::MAX,
            },
            &mut Factory {
                log: log.clone(),
                failure: "wrong_mapping",
                connections: 0,
            },
            &mut record,
        )
        .await
        .unwrap();
        assert_eq!(classify(&report), Outcome::Refuted);
        assert_eq!(report.deviations.len(), 1);
        let retained = std::fs::read_to_string(dir.join("deviation.json")).unwrap();
        assert!(retained.contains(&report.deviations[0].payload_sha256));
        assert!(!retained.contains(&pair.projects[1].root.to_string_lossy().to_string()));
        assert_eq!(
            log.lock()
                .unwrap()
                .iter()
                .filter(|s| s.starts_with("open:"))
                .count(),
            1
        );
        assert!(LocalRecorder::unresolved(&dir).unwrap());
    }
    #[tokio::test]
    async fn i1_cancelled_open_leaves_durable_uncertainty_without_replay() {
        let t = tempfile::tempdir().unwrap();
        let pair = create_pair(&t.path().join("pair")).unwrap();
        let facts = fixture_facts();
        let log = Log::default();
        let dir = t.path().join("record");
        let mut record = super::super::evidence::LocalRecorder::create(&dir).unwrap();
        let mut host = Host {
            facts: facts.clone(),
            inspections: 0,
            change_at: usize::MAX,
        };
        let mut factory = Factory {
            log: log.clone(),
            failure: "pending",
            connections: 0,
        };
        assert!(tokio::time::timeout(
            Duration::from_millis(100),
            run_sequence(&pair, &facts, &mut host, &mut factory, &mut record)
        )
        .await
        .is_err());
        assert!(super::super::evidence::LocalRecorder::unresolved(&dir).unwrap());
        drop(record);
        assert!(super::super::evidence::LocalRecorder::create(&dir).is_err());
        assert_eq!(
            log.lock()
                .unwrap()
                .iter()
                .filter(|s| s.starts_with("open:"))
                .count(),
            1
        );
    }
    #[tokio::test]
    async fn i1_generation_change_before_each_dispatch_never_reopens() {
        for change in 2..=13 {
            let (report, log) = scenario("", change).await;
            assert!(report.error.is_some(), "{change}");
            assert!(log.iter().filter(|s| s.starts_with("open:")).count() <= 2);
            if change == 2 {
                assert!(!log.iter().any(|s| s.starts_with("open:")));
            }
        }
    }
}
