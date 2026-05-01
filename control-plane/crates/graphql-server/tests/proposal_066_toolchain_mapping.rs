/// P066 Phase 0 GraphQL surface tests.
///
/// Covers:
/// - NULL actual_toolchain_mapping_diagnostics_json → legacy_row_unavailable sentinel
/// - Post-migration row with stored JSON → fields exposed correctly (active, disabled_by_policy)
/// - policy_source is always runplan_snapshot or synthesized_legacy (never agent_catalog)
/// - Absolute paths are not exposed on the northbound surface

use std::sync::Arc;

use async_graphql::Request;
use chrono::Utc;
use db::pool::create_pool;
use db::repos::{agent_executions, ideas, runs, stages};
use domain::agent::{AgentExecution, AgentStatus};
use domain::idea::{Idea, IdeaStatus};
use domain::ids::{AgentExecutionId, IdeaId, RunId, StageExecutionId};
use domain::run::{Run, RunStatus};
use domain::stage::{StageExecution, StageStatus};
use engine::command_handler::CommandHandler;
use engine::event_bus;
use engine::lifecycle_reporter::LifecycleReporter;
use engine::work_queue::WorkQueue;
use graphql_server::schema::build_schema;

fn make_run(run_id: RunId, idea_id: IdeaId) -> Run {
    Run {
        id: run_id,
        idea_id,
        status: RunStatus::Running,
        workflow_id: "wf-p066".into(),
        workflow_title: "P066 Toolchain Test".into(),
        workspace_root: "/tmp/ws".into(),
        artifact_root: "/tmp/artifacts".into(),
        started_at: Utc::now(),
        completed_at: None,
        cancellation_requested_at: None,
        cancellation_settled_at: None,
        cancellation_settlement_log: None,
        current_state: Some("state_1".into()),
        workflow_yaml_path: None,
        agent_catalog_yaml_path: None,
        worktree_root: None,
        base_branch: None,
        base_revision: None,
        target_branch: None,
        delivery_configuration_json: None,
        delivery_preflight_json: None,
        workflow_family: None,
        project_key: None,
        risk_class: None,
        stack: None,
        workflow_snapshot_hash: None,
        catalog_snapshot_hash: None,
        workflow_snapshot_json: None,
        catalog_snapshot_json: None,
        drift_detected_at: None,
        drift_details_json: None,
        chainworks_meta_root: None,
        review_routing_json: None,
    }
}

async fn seed_execution(
    pool: &sqlx::SqlitePool,
    toolchain_diagnostics_json: Option<&str>,
) -> (RunId, StageExecutionId, AgentExecutionId) {
    let idea_id = IdeaId::new();
    let run_id = RunId::new();
    let stage_execution_id = StageExecutionId::new();
    let agent_execution_id = AgentExecutionId::new();

    ideas::insert(
        pool,
        &Idea {
            id: idea_id,
            title: "P066 test idea".into(),
            body: "body".into(),
            workspace_root_path: None,
            project_key: None,
            status: IdeaStatus::Active,
            created_at: Utc::now(),
            archived_at: None,
        },
    )
    .await
    .unwrap();

    runs::insert(pool, &make_run(run_id, idea_id)).await.unwrap();

    stages::insert(
        pool,
        &StageExecution {
            id: stage_execution_id,
            run_id,
            stage_id: "state_1".into(),
            label: "State 1".into(),
            status: StageStatus::Running,
            iteration: 1,
            attempt_number: 1,
            settlement_kind: None,
            started_at: Utc::now(),
            completed_at: None,
            owner_agent: None,
            provider: None,
            model: None,
            stage_type: None,
            validation_failure_json: None,
            evidence_packet_json: None,
            recovery_snapshot_json: None,
            retry_reason: None,
        },
    )
    .await
    .unwrap();

    agent_executions::insert(
        pool,
        &AgentExecution {
            id: agent_execution_id,
            stage_execution_id: Some(stage_execution_id),
            agent_id: "code_writer".to_string(),
            provider: "claude".to_string(),
            model: Some("claude-sonnet-4-6".to_string()),
            started_at: Utc::now(),
            completed_at: None,
            status: AgentStatus::Running,
            owner_execution_lineage_id: None,
            session_lineage_id: None,
            session_generation_id: None,
            rehydrated_from_checkpoint_artifact_id: None,
            invocation_owner_key: None,
            session_reuse_scope: None,
            session_family_id: None,
            session_reuse_disposition: None,
            session_reset_reason: None,
            backend_profile_id: None,
            requested_mcp_extensions_json: None,
            predicted_mcp_extensions_json: None,
            predicted_mcp_runtime_ids_json: None,
            actual_mcp_extensions_json: None,
            actual_mcp_runtime_ids_json: None,
            denied_mcp_extensions_json: None,
            mcp_blocking_issues_json: None,
            actual_mcp_observation_json: None,
            actual_xcode_runtime_observation_json: None,
            mcp_session_startup_latency_ms: None,
            owner_kind: None,
            owner_id: None,
            lead_mediation_record_id: None,
            origin_stage_execution_id: None,
            total_cost_cents: None,
            input_tokens: None,
            output_tokens: None,
            cached_input_tokens: None,
            transcript_artifact_id: None,
            actual_toolchain_mapping_diagnostics_json: None,
        },
    )
    .await
    .unwrap();

    if let Some(json) = toolchain_diagnostics_json {
        agent_executions::update_toolchain_mapping_diagnostics(pool, agent_execution_id, json)
            .await
            .unwrap();
    }

    (run_id, stage_execution_id, agent_execution_id)
}

fn make_schema(pool: sqlx::SqlitePool) -> graphql_server::schema::AppSchema {
    let events = event_bus::new_bus(16);
    let handler = Arc::new(CommandHandler::new(
        pool.clone(),
        events.clone(),
        WorkQueue::new(pool.clone()),
    ));
    build_schema(
        pool,
        handler,
        events.clone(),
        auth::PrincipalTable::test_fixture(),
        LifecycleReporter::new(15, "test-build", events),
    )
}

static TOOLCHAIN_DIAGNOSTICS_QUERY: &str = r#"
    stage(id: "%s") {
        id
        executions {
            id
            actualToolchainMappingDiagnostics {
                mappingState
                mappingEnabled
                inactiveReason
                policySource
                policyVersion
                providerFamily
                version
            }
        }
    }
"#;

/// P066: NULL diagnostics column → legacy_row_unavailable sentinel is synthesized.
/// Matches normative example graphql_legacy from proposal §normative_examples.
#[tokio::test]
async fn p066_null_toolchain_diagnostics_synthesizes_legacy_row_unavailable() {
    let pool = create_pool("sqlite::memory:").await.unwrap();
    let (_, stage_id, _) = seed_execution(&pool, None::<&str>).await;
    let schema = make_schema(pool);

    let query = format!(
        r#"{{ stage(id: "{}") {{ id executions {{ actualToolchainMappingDiagnostics {{ mappingState mappingEnabled inactiveReason policySource policyVersion providerFamily version }} }} }} }}"#,
        stage_id
    );

    let response = schema
        .execute(
            Request::new(query).data(auth::Principal::new("operator", auth::PrincipalClass::Operator)),
        )
        .await;

    assert!(response.errors.is_empty(), "graphql errors: {:?}", response.errors);

    let data = response.data.into_json().unwrap();
    let diag = &data["stage"]["executions"][0]["actualToolchainMappingDiagnostics"];

    assert_eq!(diag["mappingState"], "legacy_row_unavailable",
        "NULL column must synthesize legacy_row_unavailable");
    assert_eq!(diag["mappingEnabled"], false,
        "legacy sentinel must have mappingEnabled=false");
    assert_eq!(diag["inactiveReason"], "legacy_row",
        "legacy sentinel must have inactiveReason=legacy_row");
    assert_eq!(diag["policySource"], "synthesized_legacy",
        "legacy sentinel must have policySource=synthesized_legacy");
    assert!(diag["policyVersion"].is_null(),
        "legacy sentinel policyVersion must be null");
    assert_eq!(diag["providerFamily"], "unknown",
        "legacy sentinel providerFamily must be unknown");
    assert_eq!(diag["version"], 1,
        "diagnostics doc version must be 1");
}

/// P066: Stored disabled_by_policy JSON → fields exposed correctly.
/// policy_source must be runplan_snapshot (never agent_catalog).
#[tokio::test]
async fn p066_disabled_by_policy_diagnostics_exposed_correctly() {
    let diagnostics_json = serde_json::json!({
        "version": 1,
        "mapping_state": "disabled_by_policy",
        "mapping_enabled": false,
        "inactive_reason": "policy_disabled",
        "policy_source": "runplan_snapshot",
        "policy_version": 1,
        "provider_family": "claude"
    })
    .to_string();

    let pool = create_pool("sqlite::memory:").await.unwrap();
    let (_, stage_id, exec_id) = seed_execution(&pool, Some(diagnostics_json.as_str())).await;
    let schema = make_schema(pool.clone());

    let query = format!(
        r#"{{ stage(id: "{}") {{ id executions {{ actualToolchainMappingDiagnostics {{ mappingState mappingEnabled inactiveReason policySource policyVersion providerFamily version }} }} }} }}"#,
        stage_id
    );

    let response = schema
        .execute(
            Request::new(query).data(auth::Principal::new("operator", auth::PrincipalClass::Operator)),
        )
        .await;

    assert!(response.errors.is_empty(), "graphql errors: {:?}", response.errors);

    let data = response.data.into_json().unwrap();
    let diag = &data["stage"]["executions"][0]["actualToolchainMappingDiagnostics"];

    assert_eq!(diag["mappingState"], "disabled_by_policy");
    assert_eq!(diag["mappingEnabled"], false);
    assert_eq!(diag["inactiveReason"], "policy_disabled");
    assert_eq!(diag["policySource"], "runplan_snapshot",
        "authoritative policy_source must be runplan_snapshot, not agent_catalog");
    assert_eq!(diag["policyVersion"], 1);
    assert_eq!(diag["providerFamily"], "claude");
    assert_eq!(diag["version"], 1);

    // policy_source must never be "agent_catalog" per DEC-003.
    assert_ne!(diag["policySource"], "agent_catalog",
        "agent_catalog must never be an authoritative policy_source");

    let _ = exec_id; // used in seeding
}

/// P066: Stored active mapping JSON → fields exposed correctly.
/// Matches normative example graphql_active from proposal §normative_examples (key fields only).
#[tokio::test]
async fn p066_active_mapping_diagnostics_exposed_correctly() {
    let diagnostics_json = serde_json::json!({
        "version": 1,
        "mapping_state": "active",
        "mapping_enabled": true,
        "inactive_reason": null,
        "policy_source": "runplan_snapshot",
        "policy_version": 1,
        "provider_family": "xcode"
    })
    .to_string();

    let pool = create_pool("sqlite::memory:").await.unwrap();
    let (_, stage_id, _) = seed_execution(&pool, Some(diagnostics_json.as_str())).await;
    let schema = make_schema(pool);

    let query = format!(
        r#"{{ stage(id: "{}") {{ id executions {{ actualToolchainMappingDiagnostics {{ mappingState mappingEnabled inactiveReason policySource policyVersion providerFamily version }} }} }} }}"#,
        stage_id
    );

    let response = schema
        .execute(
            Request::new(query).data(auth::Principal::new("operator", auth::PrincipalClass::Operator)),
        )
        .await;

    assert!(response.errors.is_empty(), "graphql errors: {:?}", response.errors);

    let data = response.data.into_json().unwrap();
    let diag = &data["stage"]["executions"][0]["actualToolchainMappingDiagnostics"];

    assert_eq!(diag["mappingState"], "active");
    assert_eq!(diag["mappingEnabled"], true);
    assert!(diag["inactiveReason"].is_null(), "active state must have null inactiveReason");
    assert_eq!(diag["policySource"], "runplan_snapshot");
    assert_eq!(diag["policyVersion"], 1);
    assert_eq!(diag["providerFamily"], "xcode");
    assert_eq!(diag["version"], 1);
}

/// P066: Absolute filesystem paths must not be exposed via GraphQL.
/// The northbound surface only exposes structured fields.
#[tokio::test]
async fn p066_absolute_paths_not_exposed_on_graphql_surface() {
    // Include absolute paths in the stored JSON to verify they are not surfaced.
    let diagnostics_json = serde_json::json!({
        "version": 1,
        "mapping_state": "active",
        "mapping_enabled": true,
        "inactive_reason": null,
        "policy_source": "runplan_snapshot",
        "policy_version": 1,
        "provider_family": "xcode",
        "absolute_derived_data_path": "/Users/testuser/toolchain_home/providers/xcode/run-123/xcode/DerivedData",
        "absolute_tmpdir": "/Users/testuser/toolchain_home/providers/xcode/run-123/xcode/tmp"
    })
    .to_string();

    let pool = create_pool("sqlite::memory:").await.unwrap();
    let (_, stage_id, _) = seed_execution(&pool, Some(diagnostics_json.as_str())).await;
    let schema = make_schema(pool);

    // Only query structured fields — absolute path fields are not on the GQL type.
    let query = format!(
        r#"{{ stage(id: "{}") {{ id executions {{ actualToolchainMappingDiagnostics {{ mappingState mappingEnabled policySource providerFamily version }} }} }} }}"#,
        stage_id
    );

    let response = schema
        .execute(
            Request::new(query).data(auth::Principal::new("operator", auth::PrincipalClass::Operator)),
        )
        .await;

    assert!(response.errors.is_empty(), "graphql errors: {:?}", response.errors);

    let data = response.data.into_json().unwrap();
    let diag_str = data["stage"]["executions"][0]["actualToolchainMappingDiagnostics"].to_string();

    assert!(
        !diag_str.contains("/Users/testuser"),
        "absolute paths must not appear in GraphQL response"
    );
    assert_eq!(
        data["stage"]["executions"][0]["actualToolchainMappingDiagnostics"]["mappingState"],
        "active"
    );
}

/// P066: policy_absent state — agent without toolchain_cache_policy block.
/// inactive_reason=policy_absent, policy_source=runplan_snapshot, policy_version=null.
#[tokio::test]
async fn p066_policy_absent_diagnostics_exposed_correctly() {
    let diagnostics_json = serde_json::json!({
        "version": 1,
        "mapping_state": "policy_absent",
        "mapping_enabled": false,
        "inactive_reason": "policy_absent",
        "policy_source": "runplan_snapshot",
        "policy_version": null,
        "provider_family": "gemini"
    })
    .to_string();

    let pool = create_pool("sqlite::memory:").await.unwrap();
    let (_, stage_id, _) = seed_execution(&pool, Some(diagnostics_json.as_str())).await;
    let schema = make_schema(pool);

    let query = format!(
        r#"{{ stage(id: "{}") {{ id executions {{ actualToolchainMappingDiagnostics {{ mappingState mappingEnabled inactiveReason policySource policyVersion providerFamily version }} }} }} }}"#,
        stage_id
    );

    let response = schema
        .execute(
            Request::new(query).data(auth::Principal::new("operator", auth::PrincipalClass::Operator)),
        )
        .await;

    assert!(response.errors.is_empty(), "graphql errors: {:?}", response.errors);
    let data = response.data.into_json().unwrap();
    let diag = &data["stage"]["executions"][0]["actualToolchainMappingDiagnostics"];

    assert_eq!(diag["mappingState"], "policy_absent");
    assert_eq!(diag["mappingEnabled"], false);
    assert_eq!(diag["inactiveReason"], "policy_absent");
    assert_eq!(diag["policySource"], "runplan_snapshot");
    assert!(diag["policyVersion"].is_null(), "policy_absent has no policy_version");
    assert_eq!(diag["providerFamily"], "gemini");
    assert_ne!(diag["policySource"], "agent_catalog", "DEC-003: agent_catalog must never be authoritative");
}

/// P066: unsupported_family state — provider not covered by P066 mapping.
/// inactive_reason=unsupported_family, mapping_enabled=false.
#[tokio::test]
async fn p066_unsupported_family_diagnostics_exposed_correctly() {
    let diagnostics_json = serde_json::json!({
        "version": 1,
        "mapping_state": "unsupported_family",
        "mapping_enabled": false,
        "inactive_reason": "unsupported_family",
        "policy_source": "runplan_snapshot",
        "policy_version": null,
        "provider_family": "auggie"
    })
    .to_string();

    let pool = create_pool("sqlite::memory:").await.unwrap();
    let (_, stage_id, _) = seed_execution(&pool, Some(diagnostics_json.as_str())).await;
    let schema = make_schema(pool);

    let query = format!(
        r#"{{ stage(id: "{}") {{ id executions {{ actualToolchainMappingDiagnostics {{ mappingState mappingEnabled inactiveReason policySource version }} }} }} }}"#,
        stage_id
    );

    let response = schema
        .execute(
            Request::new(query).data(auth::Principal::new("operator", auth::PrincipalClass::Operator)),
        )
        .await;

    assert!(response.errors.is_empty(), "graphql errors: {:?}", response.errors);
    let data = response.data.into_json().unwrap();
    let diag = &data["stage"]["executions"][0]["actualToolchainMappingDiagnostics"];

    assert_eq!(diag["mappingState"], "unsupported_family");
    assert_eq!(diag["mappingEnabled"], false);
    assert_eq!(diag["inactiveReason"], "unsupported_family");
    assert_eq!(diag["policySource"], "runplan_snapshot");
}

/// P066: setup_failed state — directory preparation failed before any toolchain work.
/// mapping_enabled=true (mapping was attempted), inactive_reason=null, policy_source=runplan_snapshot.
#[tokio::test]
async fn p066_setup_failed_diagnostics_exposed_correctly() {
    let diagnostics_json = serde_json::json!({
        "version": 1,
        "mapping_state": "setup_failed",
        "mapping_enabled": true,
        "inactive_reason": null,
        "policy_source": "runplan_snapshot",
        "policy_version": 1,
        "provider_family": "xcode"
    })
    .to_string();

    let pool = create_pool("sqlite::memory:").await.unwrap();
    let (_, stage_id, _) = seed_execution(&pool, Some(diagnostics_json.as_str())).await;
    let schema = make_schema(pool);

    let query = format!(
        r#"{{ stage(id: "{}") {{ id executions {{ actualToolchainMappingDiagnostics {{ mappingState mappingEnabled inactiveReason policySource policyVersion version }} }} }} }}"#,
        stage_id
    );

    let response = schema
        .execute(
            Request::new(query).data(auth::Principal::new("operator", auth::PrincipalClass::Operator)),
        )
        .await;

    assert!(response.errors.is_empty(), "graphql errors: {:?}", response.errors);
    let data = response.data.into_json().unwrap();
    let diag = &data["stage"]["executions"][0]["actualToolchainMappingDiagnostics"];

    assert_eq!(diag["mappingState"], "setup_failed");
    assert_eq!(
        diag["mappingEnabled"], true,
        "setup_failed must have mappingEnabled=true (mapping was attempted before failing)"
    );
    assert!(
        diag["inactiveReason"].is_null(),
        "setup_failed has no inactive_reason"
    );
    assert_eq!(diag["policySource"], "runplan_snapshot");
    assert_eq!(diag["policyVersion"], 1);
    // setup_failed must not be misidentified as queue_timeout
    assert_ne!(diag["mappingState"], "queue_timeout");
}

/// P066: queue_timeout state — per-run Xcode lease wait exceeded deadline.
/// mapping_enabled=true, inactive_reason=null. Must NOT be setup_failed.
#[tokio::test]
async fn p066_queue_timeout_diagnostics_exposed_correctly() {
    let diagnostics_json = serde_json::json!({
        "version": 1,
        "mapping_state": "queue_timeout",
        "mapping_enabled": true,
        "inactive_reason": null,
        "policy_source": "runplan_snapshot",
        "policy_version": 1,
        "provider_family": "xcode"
    })
    .to_string();

    let pool = create_pool("sqlite::memory:").await.unwrap();
    let (_, stage_id, _) = seed_execution(&pool, Some(diagnostics_json.as_str())).await;
    let schema = make_schema(pool);

    let query = format!(
        r#"{{ stage(id: "{}") {{ id executions {{ actualToolchainMappingDiagnostics {{ mappingState mappingEnabled inactiveReason policySource policyVersion version }} }} }} }}"#,
        stage_id
    );

    let response = schema
        .execute(
            Request::new(query).data(auth::Principal::new("operator", auth::PrincipalClass::Operator)),
        )
        .await;

    assert!(response.errors.is_empty(), "graphql errors: {:?}", response.errors);
    let data = response.data.into_json().unwrap();
    let diag = &data["stage"]["executions"][0]["actualToolchainMappingDiagnostics"];

    assert_eq!(diag["mappingState"], "queue_timeout");
    assert_eq!(diag["mappingEnabled"], true, "queue_timeout implies mapping was enabled");
    assert!(diag["inactiveReason"].is_null(), "queue_timeout has no inactive_reason");
    assert_eq!(diag["policySource"], "runplan_snapshot");
    // queue_timeout must not be misidentified as setup_failed (DEC-007)
    assert_ne!(diag["mappingState"], "setup_failed",
        "DEC-007: queue_timeout is not a setup failure");
}
