use super::*;
use crate::run_carry_forward::readback::{report_fields, ReadbackConfig, ReportFields};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

fn file_sha256(path: &std::path::Path) -> String {
    format!("{:x}", Sha256::digest(std::fs::read(path).unwrap()))
}

fn write_report(path: &std::path::Path, fields: &ReportFields) {
    let root = path.parent().unwrap();
    let declared = DeclaredOutput {
        output_name: "run_report".into(),
        target_path: path.to_str().unwrap().into(),
        schema: None,
        reuse_policy: None,
        companion_output_name: None,
        companion_path: None,
    };
    let discovered = vec![acp::DiscoveredArtifact {
        name: "run_report".into(),
        content: br#"{"summary":"fixture report","continued_from_run_id":"forged","carry_forward_input_provenance":[]}"#.to_vec(),
        source_path: None,
        source_kind: acp::DiscoveredArtifactSourceKind::ProviderEnvelope,
    }];
    let specs = build_expected_output_specs(
        &[declared.clone()],
        root.to_str().unwrap(),
        None,
        None,
        false,
    );
    let mut settlement = settle_agent_outputs_from_discovery_decisions(
        &[declared.clone()],
        &specs,
        &discovered,
        &[],
    )
    .unwrap();
    stamp_carry_forward_report(&mut settlement, fields, &specs).unwrap();
    let captured = build_captured_outputs_from_discovery_decisions(
        &[declared.clone()],
        &settlement.decisions,
        &settlement.accepted_payloads,
    );
    let validation = validate_task_outputs(&captured);
    materialize_validated_discovery_decisions(&[declared], &settlement, &validation, Some(root))
        .unwrap();
    assert_eq!(
        settlement.decisions[0].accepted_bytes_sha256.as_deref(),
        Some(format!("sha256:{}", file_sha256(path)).as_str())
    );
}

#[tokio::test]
async fn p039_nonempty_report_and_receipt_persist_middle_chain_provenance() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let db_url = format!("sqlite://{}?mode=rwc", root.join("readback.db").display());
    let pool = db::pool::create_pool(&db_url).await.unwrap();
    db::writer::register_shared_writer(&pool, Arc::new(DbWriter::new(pool.clone())))
        .await
        .unwrap();
    let a = RunId::new();
    let b = RunId::new();
    let c = RunId::new();
    let incoming = uuid::Uuid::new_v4().to_string();
    let outgoing = uuid::Uuid::new_v4().to_string();
    let workspace = root.join("workspace");
    let source_root = root.join("source");
    let target_root = root.join("successor");
    for path in [&workspace, &source_root, &target_root] {
        std::fs::create_dir_all(path).unwrap();
    }
    sqlx::query("INSERT INTO ideas(id,title,body,created_at) VALUES (?,'P039 fixture','Body','2026-09-20T00:00:00Z')")
        .bind(a.to_string()).execute(&pool).await.unwrap();
    for (run, status, artifacts_root) in
        [(a, "completed", &source_root), (b, "blocked", &target_root)]
    {
        sqlx::query("INSERT INTO runs(id,idea_id,status,workflow_id,workflow_title,workspace_root,artifact_root,worktree_root,started_at) VALUES (?,?,?,'fixture','Fixture',?,?,?,'2026-09-20T00:00:00Z')")
            .bind(run.to_string()).bind(a.to_string()).bind(status)
            .bind(workspace.to_str().unwrap()).bind(artifacts_root.to_str().unwrap())
            .bind(workspace.to_str().unwrap()).execute(&pool).await.unwrap();
    }
    let events = crate::event_bus::new_bus(16);
    let queue = WorkQueue::new(pool.clone());
    let orchestrator = Arc::new(Orchestrator::new(
        pool.clone(),
        events.clone(),
        queue.clone(),
    ));
    let readback_config = ReadbackConfig {
        enabled: false,
        reconcile_available: true,
    };
    let executor = BackgroundExecutor::new(
        pool.clone(),
        queue,
        orchestrator,
        Arc::new(AcpRuntimeManager::new_with_adapters(vec![])),
        events,
    )
    .with_continuation_readback(readback_config);

    let manifest_path = root.join("fixture-manifest.json");
    std::fs::write(&manifest_path, br#"{"fixture":"installed input inventory","inputs":["proposal_current","proposal_review"]}"#).unwrap();
    let manifest_hash = file_sha256(&manifest_path);
    // These are metadata fixtures, not preparation/activation or release-admission
    // proof. No execution fence is removed or bypassed to exercise the writers.
    for (operation, source, successor, phase) in [
        (&incoming, a, b, "activated"),
        (&outgoing, b, c, "needs_reconciliation"),
    ] {
        sqlx::query("INSERT INTO command_journal(id,command_type,payload_json,created_at) VALUES (?,'ContinueBlocked','{}','2026-09-20T00:00:00Z')")
            .bind(operation).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO run_continuations(operation_id,source_run_id,idea_id,reserved_successor_run_id,successor_run_id,caller_fingerprint,caller_request_id,intent_sha256,profile,phase,plan_ref,plan_sha256,target_ref,source_witness_sha256,manifest_ref,manifest_sha256,journal_id,holds_json,created_at,updated_at,deadline_at) VALUES (?,?,?,?,?,'fixture',?,?,'implementation_restart_v1',?,'fixture-plan',?,'fixture-target',?,?,?,?,?,'2026-09-20T00:00:00Z','2026-09-20T00:00:00Z','2026-09-21T00:00:00Z')")
            .bind(operation).bind(source.to_string()).bind(a.to_string()).bind(successor.to_string())
            .bind((phase == "activated").then(|| successor.to_string())).bind(operation)
            .bind("a".repeat(64)).bind(phase).bind("b".repeat(64)).bind("c".repeat(64))
            .bind(manifest_path.to_str().unwrap()).bind(&manifest_hash).bind(operation)
            .bind(if phase == "activated" { "[]" } else { "[\"effect_outcome_unknown\"]" })
            .execute(&pool).await.unwrap();
    }
    let mut expected_inputs = Vec::new();
    for (ordinal, name, role, schema, content) in [
        (
            0,
            "proposal_current",
            "execution_seed",
            "approved_proposal_v1",
            br#"{"proposal":"nonempty fixture seed"}"#.as_slice(),
        ),
        (
            1,
            "proposal_review",
            "reference_only",
            "proposal_review_v1",
            br#"{"review":"nonempty historical reference"}"#.as_slice(),
        ),
    ] {
        let source_path = source_root.join(format!("{name}.json"));
        let installed_path = target_root.join(format!("{name}.json"));
        std::fs::write(&source_path, content).unwrap();
        std::fs::copy(&source_path, &installed_path).unwrap();
        let artifact = executor
            .prepare_artifact_if_present(
                source_path.to_str().unwrap(),
                a,
                "fixture",
                "fixture",
                name,
                schema,
                ArtifactFormat::Json,
                "fixture",
                None,
                None,
                workspace.to_str().unwrap(),
                chrono::Utc::now(),
            )
            .unwrap()
            .unwrap();
        artifacts::insert(&pool, &artifact).await.unwrap();
        let input_id = uuid::Uuid::new_v4().to_string();
        let digest = file_sha256(&installed_path);
        sqlx::query("INSERT INTO run_continuation_inputs(input_id,operation_id,ordinal,successor_run_id,role,source_kind,source_id,source_run_id,original_artifact_id,source_schema,content_sha256,target_logical_name,target_relative_path,installed,historical_relation_json) VALUES (?,?,?,?,?,'artifact',?,?,?,?,?,?,?,1,'{}')")
            .bind(&input_id).bind(&incoming).bind(ordinal).bind(b.to_string()).bind(role)
            .bind(artifact.id.to_string()).bind(a.to_string()).bind(artifact.id.to_string())
            .bind(schema).bind(&digest).bind(name).bind(format!("{name}.json"))
            .execute(&pool).await.unwrap();
        expected_inputs.push((input_id, schema, digest, source_path, installed_path));
    }
    let fields = report_fields(&pool, b, readback_config).await.unwrap();
    assert_eq!(fields.carry_forward_input_provenance.len(), 2);
    let report_path = target_root.join("run-report.json");
    write_report(&report_path, &fields);
    let artifact = executor
        .prepare_artifact_if_present(
            report_path.to_str().unwrap(),
            b,
            "fixture",
            "fixture",
            "run_report",
            "run_report_v1",
            ArtifactFormat::Json,
            "fixture",
            None,
            Some("run_report"),
            workspace.to_str().unwrap(),
            chrono::Utc::now(),
        )
        .unwrap()
        .unwrap();
    artifacts::insert(&pool, &artifact).await.unwrap();
    let run = db::repos::runs::find_by_id(&pool, b)
        .await
        .unwrap()
        .unwrap();
    let delivery = DeliveryConfiguration {
        repo_identifier: "fixture".into(),
        repo_root: workspace.to_str().unwrap().into(),
        base_branch: "main".into(),
        worktree_base_path: workspace.to_str().unwrap().into(),
        target_branch: "fixture/target".into(),
        release_target_id: None,
        release_mode: None,
    };
    // Explicit failed fixture result: no provider, git, bundle, upload or release runs.
    let failed_release = ReleaseResult {
        git_manifest: None,
        git_receipt: None,
        bundle_manifest: None,
        upload_receipt: None,
        succeeded: false,
        failure_stage: Some("git".into()),
        failure_reason: Some("fixture failure".into()),
    };
    let receipt_path = std::path::PathBuf::from(
        executor
            .persist_delivery_receipt_if_absent(
                &run,
                &delivery,
                &failed_release,
                "Fixture",
                None,
                "fixture",
                "fixture",
                None,
            )
            .await
            .unwrap()
            .unwrap(),
    );
    assert!(executor
        .persist_delivery_receipt_if_absent(
            &run,
            &delivery,
            &failed_release,
            "Fixture",
            None,
            "fixture",
            "fixture",
            None
        )
        .await
        .unwrap()
        .is_none());
    let before_report = std::fs::read(&report_path).unwrap();
    let before_receipt = std::fs::read(&receipt_path).unwrap();
    drop(executor);
    pool.close().await;

    let pool = db::pool::create_pool(&db_url).await.unwrap();
    db::writer::register_shared_writer(&pool, Arc::new(DbWriter::new(pool.clone())))
        .await
        .unwrap();
    let stored = artifacts::list_by_run(&pool, b).await.unwrap();
    assert_eq!(stored.len(), 2);
    for (name, path) in [
        ("run_report", &report_path),
        ("delivery_receipt", &receipt_path),
    ] {
        let record = stored
            .iter()
            .find(|artifact| artifact.name == name)
            .unwrap();
        assert_eq!(
            record.checksum_sha256.as_deref(),
            Some(file_sha256(path).as_str()),
            "{name} checksum"
        );
        assert_eq!(
            record.size_bytes,
            Some(std::fs::metadata(path).unwrap().len() as i64)
        );
        let value: Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        assert_eq!(value["continued_from_run_id"], a.to_string());
        assert!(value["continued_as_run_id"].is_null());
        assert_eq!(
            value["carry_forward_readback"]["incoming"]["operation_id"],
            incoming
        );
        assert_eq!(
            value["carry_forward_readback"]["outgoing"]["operation_id"],
            outgoing
        );
        assert_eq!(
            value["carry_forward_readback"]["outgoing"]["phase"],
            "needs_reconciliation"
        );
        let bindings = value["carry_forward_input_provenance"].as_array().unwrap();
        assert_eq!(bindings.len(), 2);
        for (binding, (input, schema, digest, source_path, installed_path)) in
            bindings.iter().zip(&expected_inputs)
        {
            assert_eq!(binding["input_id"], *input);
            assert_eq!(binding["operation_id"], incoming);
            assert_eq!(binding["source_schema"], *schema);
            assert_eq!(binding.get("source_schema_sha256"), Some(&Value::Null));
            assert_eq!(binding["content_sha256"], format!("sha256:{digest}"));
            assert_eq!(
                binding["manifest_sha256"],
                format!("sha256:{manifest_hash}")
            );
            assert_eq!(file_sha256(source_path), *digest);
            assert_eq!(file_sha256(installed_path), *digest);
        }
        if name == "delivery_receipt" {
            assert_eq!(value["release_result"]["succeeded"], false);
            assert_eq!(value["release_result"]["failure_stage"], "git");
        }
    }
    assert_eq!(file_sha256(&manifest_path), manifest_hash);
    let aborting = db::repos::run_continuations::begin_abort(&pool, &outgoing, 1)
        .await
        .unwrap();
    db::repos::run_continuations::finish_abort(&pool, &outgoing, aborting.version)
        .await
        .unwrap();
    let after = report_fields(&pool, b, readback_config).await.unwrap();
    let after_json = serde_json::to_value(&after).unwrap();
    assert_eq!(after_json["continued_from_run_id"], a.to_string());
    assert_eq!(
        after_json["carry_forward_readback"]["incoming"]["operation_id"],
        incoming
    );
    assert!(after_json["carry_forward_readback"]["outgoing"].is_null());
    assert_eq!(
        json!(after.carry_forward_input_provenance),
        json!(fields.carry_forward_input_provenance)
    );
    let post_abort_report = target_root.join("run-report-after-abort.json");
    write_report(&post_abort_report, &after);
    let post_abort: Value =
        serde_json::from_slice(&std::fs::read(post_abort_report).unwrap()).unwrap();
    assert_eq!(
        post_abort["carry_forward_input_provenance"],
        json!(fields.carry_forward_input_provenance)
    );
    assert!(post_abort["carry_forward_readback"]["outgoing"].is_null());
    assert_eq!(std::fs::read(report_path).unwrap(), before_report);
    assert_eq!(std::fs::read(receipt_path).unwrap(), before_receipt);
    assert_eq!(artifacts::list_by_run(&pool, b).await.unwrap().len(), 2);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM agent_executions")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    pool.close().await;
}
