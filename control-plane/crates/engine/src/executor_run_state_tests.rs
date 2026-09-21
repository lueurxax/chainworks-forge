use super::*;
use serde_json::json;

async fn fixture() -> (
    tempfile::TempDir,
    SqlitePool,
    Arc<DbWriter>,
    domain::run::Run,
    Vec<DeclaredOutput>,
    Vec<ExpectedOutputSpec>,
) {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let meta = root.join(".chainworks/runs/current");
    std::fs::create_dir_all(&meta).unwrap();
    let pool = db::pool::create_pool("sqlite::memory:").await.unwrap();
    let writer = Arc::new(DbWriter::new(pool.clone()));
    db::writer::register_shared_writer(&pool, writer.clone())
        .await
        .unwrap();
    let run_id = RunId::new();
    let idea_id = domain::ids::IdeaId::new();
    sqlx::query("INSERT INTO ideas (id,title,body,status,created_at) VALUES (?,'run state fixture','','active',?)")
        .bind(idea_id.to_string()).bind(chrono::Utc::now().to_rfc3339()).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO runs (id,idea_id,status,workflow_id,workflow_title,workspace_root,artifact_root,started_at,current_state,chainworks_meta_root) VALUES (?,?,'running','fixture','Fixture',?,?,?,'state_7_implementation_started',?)")
        .bind(run_id.to_string()).bind(idea_id.to_string()).bind(root.to_str().unwrap())
        .bind(meta.to_str().unwrap()).bind(chrono::Utc::now().to_rfc3339()).bind(meta.to_str().unwrap())
        .execute(&pool).await.unwrap();
    let run = runs::find_by_id(&pool, run_id).await.unwrap().unwrap();
    // The live frozen catalog has no schema for run_state.
    let outputs = ["implementation_plan", "implementation_backlog", "run_state"]
        .into_iter()
        .map(|name| DeclaredOutput {
            output_name: name.into(),
            target_path: if name == "run_state" {
                meta.join("state/run-state.json")
            } else {
                meta.join(format!("implementation/{name}.json"))
            }
            .to_string_lossy()
            .into_owned(),
            schema: None,
            reuse_policy: None,
            companion_output_name: None,
            companion_path: None,
        })
        .collect::<Vec<_>>();
    let specs = build_expected_output_specs(
        &outputs,
        &run.workspace_root,
        None,
        run.chainworks_meta_root.as_deref(),
        false,
    );
    (dir, pool, writer, run, outputs, specs)
}

fn baseline(
    specs: &[ExpectedOutputSpec],
    bytes: &[u8],
    attempt: u32,
) -> Vec<PrePromptExpectedOutputMetadata> {
    let context = PrePromptExpectedOutputContext {
        agent_execution_id: format!("execution-{attempt}"),
        stage_execution_id: format!("stage-{attempt}"),
        attempt_number: attempt,
        session_generation_id: format!("session-{attempt}"),
        prompt_turn_id: format!("turn-{attempt}"),
        discovery_generation_id: format!("discovery-{attempt}"),
    };
    specs
        .iter()
        .map(|spec| {
            let mut value = PrePromptExpectedOutputMetadata::absent(spec, &context);
            if spec.output_name == "run_state" {
                value.baseline_status = ExpectedPathBaselineStatus::RegularContentCaptured;
                value.existed = true;
                value.file_type = "regular".into();
                value.size_bytes = Some(bytes.len() as u64);
                value.content_digest = Some(sha256_digest(bytes));
            }
            value
        })
        .collect()
}

fn provider_outputs(
    outputs: &[DeclaredOutput],
    state_bytes: Option<&[u8]>,
    mode: &str,
) -> Vec<acp::DiscoveredArtifact> {
    outputs.iter().filter_map(|output| {
        let value = if output.output_name == "run_state" {
            let bytes = state_bytes?;
            json!({"mode":mode,"path":output.target_path,"digest":sha256_digest(bytes),"size_bytes":bytes.len()})
        } else { json!({"status":"ready","items":[]}) };
        Some(acp::DiscoveredArtifact {
            name: output.target_path.clone(), content: serde_json::to_vec(&value).unwrap(),
            source_path: None, source_kind: acp::DiscoveredArtifactSourceKind::ChainworksOutput,
        })
    }).collect()
}

#[tokio::test]
async fn unchanged_run_state_is_db_owned_across_codex_claude_and_no_manifest() {
    let (_dir, pool, writer, run, outputs, specs) = fixture().await;
    let first = generate_declared_run_state_output(&pool, &run, &specs)
        .await
        .unwrap();
    let original_bytes = first.bytes.as_ref().unwrap().clone();
    for (attempt, mode) in ["direct_file", "direct_file_ref", "no_manifest"]
        .into_iter()
        .enumerate()
    {
        let generated = generate_declared_run_state_output(&pool, &run, &specs)
            .await
            .unwrap();
        assert_eq!(
            generated.bytes.as_ref().unwrap(),
            &original_bytes,
            "no cosmetic digest change"
        );
        let artifacts = provider_outputs(
            &outputs,
            (mode != "no_manifest").then_some(original_bytes.as_slice()),
            mode,
        );
        let settlement = build_declared_output_discovery_settlement_with_generated_run_state(
            &specs,
            &artifacts,
            &baseline(&specs, &original_bytes, attempt as u32 + 1),
            &StdDiscoveryFilesystem,
            Some(&generated),
        );
        let captured = build_captured_outputs_from_discovery_decisions(
            &outputs,
            &settlement.decisions,
            &settlement.accepted_payloads,
        );
        assert!(validate_task_outputs(&captured).failure_class.is_none());
        let decision = settlement
            .decisions
            .iter()
            .find(|d| d.output_name == "run_state")
            .unwrap();
        assert_eq!(
            decision.reason,
            OutputDiscoveryReason::ControlPlaneGenerated
        );
        assert_eq!(decision.generated_by.as_deref(), Some("control_plane"));
        assert_eq!(
            captured[2].machine_bytes.as_deref(),
            Some(original_bytes.as_slice())
        );
    }
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM artifact_contract_generations")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    writer.shutdown().await;
    pool.close().await;
}

#[tokio::test]
async fn run_state_rebuild_replaces_forged_or_missing_export_without_importing_it() {
    let (_dir, pool, writer, run, outputs, specs) = fixture().await;
    let path = Path::new(&outputs[2].target_path);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    for forged in [
        Some(
            br#"{"run_id":"other","current_state":"completed","active_index_owner":"agent"}"#
                .as_slice(),
        ),
        None,
    ] {
        if let Some(bytes) = forged {
            std::fs::write(path, bytes).unwrap();
        } else if path.exists() {
            std::fs::remove_file(path).unwrap();
        }
        let generated = generate_declared_run_state_output(&pool, &run, &specs)
            .await
            .unwrap();
        let bytes = generated.bytes.as_ref().unwrap();
        let value: serde_json::Value = serde_json::from_slice(bytes).unwrap();
        assert_eq!(value["run_id"], run.id.to_string());
        assert_eq!(value["current_state"], "state_7_implementation_started");
        assert_eq!(value["active_index_owner"], "sqlite");
        assert_eq!(std::fs::read(path).unwrap(), *bytes);
        // A provider cannot alter the accepted bytes after the DB snapshot/export.
        std::fs::write(path, b"forged again").unwrap();
        let settlement = build_declared_output_discovery_settlement_with_generated_run_state(
            &specs,
            &[],
            &[],
            &StdDiscoveryFilesystem,
            Some(&generated),
        );
        let captured = build_captured_outputs_from_discovery_decisions(
            &outputs,
            &settlement.decisions,
            &settlement.accepted_payloads,
        );
        assert_eq!(captured[2].machine_bytes.as_deref(), Some(bytes.as_slice()));
    }
    writer.shutdown().await;
    pool.close().await;
}

#[tokio::test]
async fn run_state_requires_trusted_projection_even_when_disk_and_envelope_look_valid() {
    let (_dir, pool, writer, run, outputs, specs) = fixture().await;
    let generated = generate_declared_run_state_output(&pool, &run, &specs)
        .await
        .unwrap();
    let bytes = generated.bytes.as_ref().unwrap();
    let settlement = build_declared_output_discovery_settlement(
        &specs,
        &provider_outputs(&outputs, Some(bytes), "direct_file"),
        &baseline(&specs, bytes, 1),
    );
    let decision = settlement
        .decisions
        .iter()
        .find(|d| d.output_name == "run_state")
        .unwrap();
    assert_eq!(decision.status, OutputDiscoveryStatus::Rejected);
    assert_eq!(
        decision.diagnostics["control_plane_rejection"],
        "run_state_projection_unavailable_or_invalid"
    );
    writer.shutdown().await;
    pool.close().await;
}

#[tokio::test]
async fn repair_other_output_keeps_run_state_owned_by_control_plane() {
    let (_dir, pool, writer, run, outputs, specs) = fixture().await;
    let generated = generate_declared_run_state_output(&pool, &run, &specs)
        .await
        .unwrap();
    let original = build_declared_output_discovery_settlement_with_generated_run_state(
        &specs,
        &[],
        &[],
        &StdDiscoveryFilesystem,
        Some(&generated),
    );
    let captured = build_captured_outputs_from_discovery_decisions(
        &outputs,
        &original.decisions,
        &original.accepted_payloads,
    );
    let mut validation = validate_task_outputs(&captured);
    enforce_generated_run_state_status(&mut validation, &original);
    let prompt = output_contract_repair_prompt(&validation, &outputs, "run", "stage", "execution");
    assert!(
        !prompt.contains(&outputs[2].target_path),
        "repair must not ask provider to mutate generated run_state"
    );
    let repaired = build_declared_output_discovery_settlement_with_generated_run_state(
        &specs,
        &provider_outputs(&outputs, None, "direct_file"),
        &[],
        &StdDiscoveryFilesystem,
        Some(&generated),
    );
    let merged = merge_repair_discovery_settlements(&original, &repaired);
    let captured = build_captured_outputs_from_discovery_decisions(
        &outputs,
        &merged.decisions,
        &merged.accepted_payloads,
    );
    assert!(validate_task_outputs(&captured).failure_class.is_none());
    assert_eq!(
        merged.decisions[2].reason,
        OutputDiscoveryReason::ControlPlaneGenerated
    );
    writer.shutdown().await;
    pool.close().await;
}

#[tokio::test]
async fn run_state_relative_meta_root_resolves_against_run_workspace() {
    let (_dir, pool, writer, mut run, outputs, specs) = fixture().await;
    let relative = ".chainworks/runs/current";
    sqlx::query("UPDATE runs SET chainworks_meta_root = ? WHERE id = ?")
        .bind(relative)
        .bind(run.id.to_string())
        .execute(&pool)
        .await
        .unwrap();
    run.chainworks_meta_root = Some(relative.into());
    let generated = generate_declared_run_state_output(&pool, &run, &specs)
        .await
        .unwrap();
    assert!(generated.bytes.is_ok(), "{:?}", generated.bytes);
    assert_eq!(
        std::fs::read(&outputs[2].target_path).unwrap(),
        *generated.bytes.as_ref().unwrap()
    );
    writer.shutdown().await;
    pool.close().await;
}

#[tokio::test]
async fn run_state_directory_cannot_bypass_failed_projection_export() {
    let (_dir, pool, writer, run, outputs, specs) = fixture().await;
    let target = Path::new(&outputs[2].target_path);
    std::fs::create_dir_all(target).unwrap();
    std::fs::write(target.join("forged.json"), b"{}").unwrap();
    let generated = generate_declared_run_state_output(&pool, &run, &specs)
        .await
        .unwrap();
    assert!(generated.bytes.is_err());
    let settlement = build_declared_output_discovery_settlement_with_generated_run_state(
        &specs,
        &[],
        &[],
        &StdDiscoveryFilesystem,
        Some(&generated),
    );
    let decision = &settlement.decisions[2];
    assert_eq!(decision.status, OutputDiscoveryStatus::Rejected);
    assert_eq!(decision.generated_by.as_deref(), Some("control_plane"));
    assert!(settlement.accepted_payloads.is_empty());
    writer.shutdown().await;
    pool.close().await;
}

#[tokio::test]
async fn rejected_generated_run_state_is_not_imported_as_agent_evidence() {
    let (_dir, pool, writer, run, outputs, mut specs) = fixture().await;
    let generated = generate_declared_run_state_output(&pool, &run, &specs)
        .await
        .unwrap();
    specs[2].aggregate_acceptance_cap_bytes = 1;
    let settlement = build_declared_output_discovery_settlement_with_generated_run_state(
        &specs,
        &[],
        &[],
        &StdDiscoveryFilesystem,
        Some(&generated),
    );
    assert_eq!(
        settlement.decisions[2].reason,
        OutputDiscoveryReason::AggregateExactOutputCap
    );
    assert_eq!(
        settlement.decisions[2].generated_by.as_deref(),
        Some("control_plane")
    );
    let events = crate::event_bus::new_bus(8);
    let queue = crate::work_queue::WorkQueue::new(pool.clone());
    let orchestrator = Arc::new(crate::orchestrator::Orchestrator::new(
        pool.clone(),
        events.clone(),
        queue.clone(),
    ));
    let executor = BackgroundExecutor::new(
        pool.clone(),
        queue,
        orchestrator,
        Arc::new(AcpRuntimeManager::new_with_adapters(vec![])),
        events,
    );
    let mut persisted_paths = HashSet::new();
    let artifacts = executor
        .prepare_declared_output_artifacts(
            &outputs,
            Some(&settlement.decisions),
            &run.workspace_root,
            run.id,
            "state_7_implementation_started",
            "lead_orchestrator",
            "codex",
            None,
            chrono::Utc::now(),
            &mut persisted_paths,
        )
        .unwrap();
    assert!(artifacts.is_empty());
    assert!(
        persisted_paths.contains(&outputs[2].target_path),
        "supplemental scans must also exclude the generated projection"
    );
    writer.shutdown().await;
    pool.close().await;
}

#[tokio::test]
async fn generated_run_state_failure_is_engine_failure_and_cannot_request_provider_repair() {
    let (_dir, pool, writer, run, outputs, mut specs) = fixture().await;
    let generated = generate_declared_run_state_output(&pool, &run, &specs)
        .await
        .unwrap();
    specs[2].aggregate_acceptance_cap_bytes = 1;
    let settlement = build_declared_output_discovery_settlement_with_generated_run_state(
        &specs,
        &provider_outputs(&outputs, None, "direct_file"),
        &[],
        &StdDiscoveryFilesystem,
        Some(&generated),
    );
    let captured = build_captured_outputs_from_discovery_decisions(
        &outputs,
        &settlement.decisions,
        &settlement.accepted_payloads,
    );
    let mut validation = validate_task_outputs(&captured);
    assert!(
        generated_run_state_failed(&settlement),
        "production repair gate must skip engine-owned failures"
    );
    enforce_generated_run_state_status(&mut validation, &settlement);
    assert_eq!(
        validation.failure_class,
        Some(domain::validation::ValidationFailureClass::PersistenceFailure)
    );
    assert!(validation
        .failure_summary
        .as_deref()
        .unwrap()
        .contains("provider repair cannot regenerate"));
    let close_diagnostic = acp::AcpCloseDiagnostic {
        transport_error_code: Some("EPIPE".into()),
        provider_exit_status: Some(141),
        message: "write EPIPE after session/close".into(),
    };
    let receipt = acp::AcpRuntimeReceipt {
        schema_version: 1,
        transport_family: "acp_stdio".into(),
        provider: "codex".into(),
        model: None,
        provider_session_id: Some("provider-session".into()),
        session_generation_id: Some("generation".into()),
        status: "completed".into(),
        failure_phase: None,
        jsonrpc_error_code: None,
        provider_error_message_redacted: None,
        started_at: chrono::Utc::now().to_rfc3339(),
        completed_at: Some(chrono::Utc::now().to_rfc3339()),
        xcode_shim_injected: false,
        requires_xcode_host_execution: false,
        handshake: acp::AcpRuntimeReceiptHandshake::default(),
        counters: acp::AcpRuntimeReceiptCounters {
            agent_message_chunk_count: 1,
            ..Default::default()
        },
        permission_roundtrips: Vec::new(),
        first_events: Vec::new(),
        last_events: Vec::new(),
        claude_diagnostics: None,
        p079_unsafe_continuation: false,
    };
    let facts = runtime_facts_for_execution_result(
        domain::ids::AgentExecutionId::new(),
        AgentStatus::Completed,
        Some(&validation),
        None,
        chrono::Utc::now(),
        Some(&close_diagnostic),
        Some(&receipt),
        None,
    );
    assert!(is_control_plane_run_state_failure(&facts));
    assert_eq!(facts.failure_kind, Some(AgentFailureKind::Unknown));
    assert_eq!(
        facts.operator_action_hint,
        Some(OperatorActionHint::InspectLogs)
    );
    assert_eq!(
        facts.output_settlement,
        AgentOutputSettlement::InvalidRequiredOutputs
    );
    assert!(!facts.valid_required_outputs);
    assert_eq!(facts.transport_error_code, None);
    assert_eq!(facts.retry_after, None);
    assert_eq!(
        facts.provider_exit_status,
        Some(141),
        "exit evidence remains available"
    );
    assert_eq!(
        crate::shadow_escalation::classify_trigger_from_runtime_facts(&facts),
        None
    );
    assert!(!crate::orchestrator::provider_health_fallback_failure(
        &facts
    ));
    writer.shutdown().await;
    pool.close().await;
}

#[test]
fn unrelated_persistence_failures_keep_provider_contract_classification() {
    for (output_name, summary) in [
        ("run_state", "ordinary artifact persistence failure"),
        (
            "implementation_plan",
            "control_plane_run_state_unavailable: unrelated output",
        ),
    ] {
        let validation = TaskValidationSummary {
            output_results: vec![domain::validation::OutputValidationResult {
                output_name: output_name.into(),
                contract_id: None,
                status: domain::validation::ValidationStatus::Failed,
                missing_fields: Vec::new(),
                validation_error: Some(summary.into()),
                raw_payload_size: 0,
            }],
            contract_metadata: Vec::new(),
            raw_output_exists: false,
            failure_class: Some(domain::validation::ValidationFailureClass::PersistenceFailure),
            failure_summary: Some(summary.into()),
        };
        let facts = runtime_facts_for_execution_result(
            domain::ids::AgentExecutionId::new(),
            AgentStatus::Completed,
            Some(&validation),
            None,
            chrono::Utc::now(),
            None,
            None,
            None,
        );
        assert!(!is_control_plane_run_state_failure(&facts));
        assert_eq!(
            facts.failure_kind,
            Some(AgentFailureKind::InvalidOutputContract)
        );
        assert_eq!(facts.operator_action_hint, Some(OperatorActionHint::Retry));
        assert_eq!(
            crate::shadow_escalation::classify_trigger_from_runtime_facts(&facts),
            Some("contract_output_failure")
        );
    }
}

#[tokio::test]
async fn repair_staging_never_publishes_generated_run_state_as_agent_output() {
    let (_dir, pool, writer, run, outputs, specs) = fixture().await;
    let generated = generate_declared_run_state_output(&pool, &run, &specs)
        .await
        .unwrap();
    let settlement = build_declared_output_discovery_settlement_with_generated_run_state(
        &specs,
        &provider_outputs(&outputs, None, "direct_file"),
        &[],
        &StdDiscoveryFilesystem,
        Some(&generated),
    );
    let captured = build_captured_outputs_from_discovery_decisions(
        &outputs,
        &settlement.decisions,
        &settlement.accepted_payloads,
    );
    let validation = validate_task_outputs(&captured);
    let staged = stage_p090_repair_materialization(
        &run.artifact_root,
        run.id,
        "state_7_implementation_started",
        domain::ids::StageExecutionId::new(),
        domain::ids::AgentExecutionId::new(),
        Some("session"),
        1,
        "receipt",
        &outputs,
        &settlement,
        &validation,
        chrono::Utc::now(),
    )
    .unwrap();
    assert!(staged
        .commits
        .iter()
        .all(|commit| commit.output_name != "run_state"));
    let row = staged
        .rows
        .iter()
        .find(|row| row.output_name == "run_state")
        .unwrap();
    assert_eq!(row.source_generation_owner, "control_plane");
    assert_eq!(row.materialization_state, "not_materialized");
    assert!(row.staging_path.is_none());
    assert!(row.active_pointer_generation_id.is_none());
    writer.shutdown().await;
    pool.close().await;
}

#[cfg(target_os = "macos")]
#[tokio::test]
async fn generated_run_state_exclusion_covers_macos_canonical_path_alias() {
    let (_dir, pool, writer, mut run, mut outputs, _specs) = fixture().await;
    let canonical_meta = run.chainworks_meta_root.clone().unwrap();
    assert!(canonical_meta.starts_with("/private/"));
    let aliased_meta = canonical_meta.replacen("/private/", "/", 1);
    sqlx::query("UPDATE runs SET chainworks_meta_root = ? WHERE id = ?")
        .bind(&aliased_meta)
        .bind(run.id.to_string())
        .execute(&pool)
        .await
        .unwrap();
    run.chainworks_meta_root = Some(aliased_meta.clone());
    for output in &mut outputs {
        output.target_path = output.target_path.replace(&canonical_meta, &aliased_meta);
    }
    let specs = build_expected_output_specs(
        &outputs,
        &run.workspace_root,
        None,
        run.chainworks_meta_root.as_deref(),
        false,
    );
    let generated = generate_declared_run_state_output(&pool, &run, &specs)
        .await
        .unwrap();
    assert!(generated.bytes.is_ok(), "{:?}", generated.bytes);
    let settlement = build_declared_output_discovery_settlement_with_generated_run_state(
        &specs,
        &[],
        &[],
        &StdDiscoveryFilesystem,
        Some(&generated),
    );
    let events = crate::event_bus::new_bus(8);
    let queue = crate::work_queue::WorkQueue::new(pool.clone());
    let orchestrator = Arc::new(crate::orchestrator::Orchestrator::new(
        pool.clone(),
        events.clone(),
        queue.clone(),
    ));
    let executor = BackgroundExecutor::new(
        pool.clone(),
        queue,
        orchestrator,
        Arc::new(AcpRuntimeManager::new_with_adapters(vec![])),
        events,
    );
    let mut persisted_paths = HashSet::new();
    assert!(executor
        .prepare_declared_output_artifacts(
            &outputs,
            Some(&settlement.decisions),
            &run.workspace_root,
            run.id,
            "state_7_implementation_started",
            "lead_orchestrator",
            "codex",
            None,
            chrono::Utc::now(),
            &mut persisted_paths
        )
        .unwrap()
        .is_empty());
    assert!(persisted_paths.contains(&outputs[2].target_path));
    let canonical = std::fs::canonicalize(&outputs[2].target_path).unwrap();
    assert!(persisted_paths.contains(canonical.to_str().unwrap()));
    writer.shutdown().await;
    pool.close().await;
}
