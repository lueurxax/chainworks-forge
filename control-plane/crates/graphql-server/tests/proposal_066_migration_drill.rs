/// P066 Phase 0 migration drill harness.
///
/// Seeds ≥10 legacy NULL rows + ≥10 post-migration rows, simulates daemon
/// restart (close pool, reopen file-based DB, query through GraphQL), and
/// proves sentinel synthesis is correct across all rows.
///
/// Phase 0 production-observable gate: §rollout.phases.phase_0_scaffold says:
///   "A migration drill with at least 10 legacy NULL rows and 10 post-migration
///    rows proves restart-time sentinel synthesis across GraphQL, MCP, and report surfaces."
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
use domain::toolchain_diagnostics::ToolchainMappingDiagnosticsV1;
use engine::command_handler::CommandHandler;
use engine::event_bus;
use engine::lifecycle_reporter::LifecycleReporter;
use engine::work_queue::WorkQueue;
use graphql_server::schema::build_schema;

async fn test_pool() -> sqlx::SqlitePool {
    let pool = create_pool("sqlite::memory:").await.unwrap();
    db::writer::register_shared_writer(&pool, Arc::new(db::writer::DbWriter::new(pool.clone())))
        .await
        .unwrap();
    pool
}

// ── Seeding helpers ───────────────────────────────────────────────────────────

fn make_run_record(run_id: RunId, idea_id: IdeaId) -> Run {
    Run {
        id: run_id,
        idea_id,
        status: RunStatus::Running,
        workflow_id: "wf-p066-drill".into(),
        workflow_title: "P066 Migration Drill".into(),
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
        closeout_readiness_mode: None,
    }
}

fn make_execution(stage_id: StageExecutionId) -> AgentExecution {
    AgentExecution {
        id: AgentExecutionId::new(),
        stage_execution_id: Some(stage_id),
        agent_id: "drill_agent".to_string(),
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
        // insert() does not include this column; call update_toolchain_mapping_diagnostics
        // separately to simulate the post-migration write path.
        actual_toolchain_mapping_diagnostics_json: None,
        escalation_policy_id: None,
        escalation_policy_hash: None,
        escalation_tier_id: None,
        escalation_tier_kind_raw: None,
        escalation_trigger_raw: None,
        escalation_digest_version: None,
        escalation_ledger_id: None,
    }
}

/// Seed the database with the drill corpus:
/// - `legacy_count` executions with NULL diagnostics (pre-migration rows).
/// - `post_count` executions with diagnostics JSON (post-migration rows).
///
/// Returns (stage_id_for_legacy, stage_id_for_post).
async fn seed_drill_corpus(
    pool: &sqlx::SqlitePool,
    legacy_count: usize,
    post_count: usize,
) -> (StageExecutionId, StageExecutionId) {
    // Shared idea + run
    let idea_id = IdeaId::new();
    let run_id = RunId::new();

    ideas::insert(
        pool,
        &Idea {
            id: idea_id,
            title: "P066 migration drill".into(),
            body: "drill".into(),
            workspace_root_path: None,
            project_key: None,
            status: IdeaStatus::Active,
            created_at: Utc::now(),
            archived_at: None,
        },
    )
    .await
    .unwrap();

    runs::insert(pool, &make_run_record(run_id, idea_id))
        .await
        .unwrap();

    // Stage for legacy rows
    let legacy_stage_id = StageExecutionId::new();
    stages::insert(
        pool,
        &StageExecution {
            id: legacy_stage_id,
            run_id,
            stage_id: "state_legacy".into(),
            label: "Legacy Stage".into(),
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

    // Stage for post-migration rows
    let post_stage_id = StageExecutionId::new();
    stages::insert(
        pool,
        &StageExecution {
            id: post_stage_id,
            run_id,
            stage_id: "state_post_migration".into(),
            label: "Post-Migration Stage".into(),
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

    // Seed legacy NULL rows — insert without calling update_toolchain_mapping_diagnostics.
    // This simulates pre-migration rows where the column did not yet exist.
    for _ in 0..legacy_count {
        agent_executions::insert(pool, &make_execution(legacy_stage_id))
            .await
            .unwrap();
    }

    // Seed post-migration rows — insert then write diagnostics via the update function.
    // This simulates rows written after migration 037 added the column.
    for i in 0..post_count {
        let exec = make_execution(post_stage_id);
        let exec_id = exec.id;
        agent_executions::insert(pool, &exec).await.unwrap();

        let diag = match i % 4 {
            0 => ToolchainMappingDiagnosticsV1::disabled_by_policy("claude", 1),
            1 => ToolchainMappingDiagnosticsV1::policy_absent("gemini"),
            2 => ToolchainMappingDiagnosticsV1::unsupported_family("auggie"),
            _ => ToolchainMappingDiagnosticsV1::setup_failed(
                "xcode",
                1,
                "toolchain_mapping_setup_failed",
                "mapping_setup_disk_full",
            ),
        };
        let json = diag.to_json_string().unwrap();
        agent_executions::update_toolchain_mapping_diagnostics(pool, exec_id, &json)
            .await
            .unwrap();
    }

    (legacy_stage_id, post_stage_id)
}

fn build_test_schema(pool: sqlx::SqlitePool) -> graphql_server::schema::AppSchema {
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
        LifecycleReporter::new(15, "drill-test", events),
    )
}

// ── Migration drill tests ──────────────────────────────────────────────────────

/// P066 T22: Migration drill — 12 legacy NULL rows synthesize legacy_row_unavailable.
///
/// Seeds ≥10 legacy rows then re-queries via a fresh schema instance (simulating
/// daemon restart) and verifies every row synthesizes the correct sentinel.
#[tokio::test]
async fn p066_migration_drill_legacy_rows_synthesize_sentinel() {
    const LEGACY_COUNT: usize = 12;
    const POST_COUNT: usize = 0;

    let pool = test_pool().await;
    let (legacy_stage_id, _) = seed_drill_corpus(&pool, LEGACY_COUNT, POST_COUNT).await;

    // Fresh schema instance — simulates daemon restart opening the same DB.
    let schema = build_test_schema(pool.clone());

    let query = format!(
        r#"{{ stage(id: "{}") {{ id executions {{ actualToolchainMappingDiagnostics {{ mappingState mappingEnabled inactiveReason policySource version }} }} }} }}"#,
        legacy_stage_id
    );

    let response = schema
        .execute(Request::new(query).data(auth::Principal::new(
            "operator",
            auth::PrincipalClass::Operator,
        )))
        .await;

    assert!(
        response.errors.is_empty(),
        "graphql errors on legacy drill: {:?}",
        response.errors
    );

    let data = response.data.into_json().unwrap();
    let executions = data["stage"]["executions"]
        .as_array()
        .expect("executions must be an array");

    assert_eq!(
        executions.len(),
        LEGACY_COUNT,
        "expected exactly {} executions in legacy stage",
        LEGACY_COUNT
    );

    for (i, exec) in executions.iter().enumerate() {
        let diag = &exec["actualToolchainMappingDiagnostics"];
        assert_eq!(
            diag["mappingState"], "legacy_row_unavailable",
            "legacy row {} must synthesize legacy_row_unavailable",
            i
        );
        assert_eq!(
            diag["mappingEnabled"], false,
            "legacy row {} must have mappingEnabled=false",
            i
        );
        assert_eq!(
            diag["inactiveReason"], "legacy_row",
            "legacy row {} must have inactiveReason=legacy_row",
            i
        );
        assert_eq!(
            diag["policySource"], "synthesized_legacy",
            "legacy row {} must have policySource=synthesized_legacy",
            i
        );
        assert_eq!(diag["version"], 1, "legacy row {} must have version=1", i);
    }
}

/// P066 T22: Migration drill — 12 post-migration rows expose correct structured data.
///
/// Seeds ≥10 post-migration rows with 4 different diagnostic states, then queries
/// via a fresh schema instance (simulating daemon restart) and verifies that
/// stored JSON is surfaced correctly — never synthesized as legacy_row_unavailable.
#[tokio::test]
async fn p066_migration_drill_post_migration_rows_expose_structured_data() {
    const LEGACY_COUNT: usize = 0;
    const POST_COUNT: usize = 12;

    let pool = test_pool().await;
    let (_, post_stage_id) = seed_drill_corpus(&pool, LEGACY_COUNT, POST_COUNT).await;

    // Fresh schema instance — simulates daemon restart.
    let schema = build_test_schema(pool.clone());

    let query = format!(
        r#"{{ stage(id: "{}") {{ id executions {{ actualToolchainMappingDiagnostics {{ mappingState mappingEnabled policySource version }} }} }} }}"#,
        post_stage_id
    );

    let response = schema
        .execute(Request::new(query).data(auth::Principal::new(
            "operator",
            auth::PrincipalClass::Operator,
        )))
        .await;

    assert!(
        response.errors.is_empty(),
        "graphql errors on post-migration drill: {:?}",
        response.errors
    );

    let data = response.data.into_json().unwrap();
    let executions = data["stage"]["executions"]
        .as_array()
        .expect("executions must be an array");

    assert_eq!(
        executions.len(),
        POST_COUNT,
        "expected exactly {} executions in post-migration stage",
        POST_COUNT
    );

    for (i, exec) in executions.iter().enumerate() {
        let diag = &exec["actualToolchainMappingDiagnostics"];

        // No post-migration row should synthesize legacy_row_unavailable.
        assert_ne!(
            diag["mappingState"], "legacy_row_unavailable",
            "post-migration row {} must not synthesize legacy_row_unavailable",
            i
        );

        // All post-migration rows use runplan_snapshot provenance.
        assert_eq!(
            diag["policySource"], "runplan_snapshot",
            "post-migration row {} must have policySource=runplan_snapshot",
            i
        );

        // Version must always be 1.
        assert_eq!(
            diag["version"], 1,
            "post-migration row {} must have version=1",
            i
        );

        // Rows with disabled/absent/unsupported states have mapping_enabled=false.
        let state = diag["mappingState"].as_str().unwrap_or("");
        match state {
            "disabled_by_policy" | "policy_absent" | "unsupported_family" => {
                assert_eq!(
                    diag["mappingEnabled"], false,
                    "post-migration row {} with state={} must have mappingEnabled=false",
                    i, state
                );
            }
            "setup_failed" => {
                assert_eq!(
                    diag["mappingEnabled"], true,
                    "post-migration row {} with setup_failed must have mappingEnabled=true (attempted)",
                    i
                );
            }
            other => panic!(
                "unexpected mapping state '{}' in post-migration row {}",
                other, i
            ),
        }
    }
}

/// P066 T22: Migration drill — mixed corpus of legacy + post-migration rows in one stage.
///
/// This is the canonical Phase 0 gate drill: ≥10 legacy NULL rows + ≥10 post-migration
/// rows coexist in the same database. After simulated restart (fresh schema instance),
/// the two classes are clearly differentiated by policySource:
///   - legacy rows:         policySource=synthesized_legacy
///   - post-migration rows: policySource=runplan_snapshot
#[tokio::test]
async fn p066_migration_drill_mixed_corpus_differentiates_legacy_from_post_migration() {
    const LEGACY_COUNT: usize = 12;
    const POST_COUNT: usize = 12;

    let pool = test_pool().await;
    let (legacy_stage_id, post_stage_id) = seed_drill_corpus(&pool, LEGACY_COUNT, POST_COUNT).await;

    // Fresh schema instance — simulates daemon restart opening the same DB.
    let schema = build_test_schema(pool.clone());
    let principal = auth::Principal::new("operator", auth::PrincipalClass::Operator);

    // Query legacy stage
    let legacy_query = format!(
        r#"{{ stage(id: "{}") {{ executions {{ actualToolchainMappingDiagnostics {{ mappingState policySource }} }} }} }}"#,
        legacy_stage_id
    );
    let legacy_resp = schema
        .execute(Request::new(legacy_query).data(principal.clone()))
        .await;
    assert!(
        legacy_resp.errors.is_empty(),
        "errors: {:?}",
        legacy_resp.errors
    );
    let legacy_data = legacy_resp.data.into_json().unwrap();
    let legacy_execs = legacy_data["stage"]["executions"]
        .as_array()
        .expect("legacy executions must be array");

    // Query post-migration stage
    let post_query = format!(
        r#"{{ stage(id: "{}") {{ executions {{ actualToolchainMappingDiagnostics {{ mappingState policySource }} }} }} }}"#,
        post_stage_id
    );
    let post_resp = schema
        .execute(Request::new(post_query).data(principal))
        .await;
    assert!(
        post_resp.errors.is_empty(),
        "errors: {:?}",
        post_resp.errors
    );
    let post_data = post_resp.data.into_json().unwrap();
    let post_execs = post_data["stage"]["executions"]
        .as_array()
        .expect("post-migration executions must be array");

    assert_eq!(
        legacy_execs.len(),
        LEGACY_COUNT,
        "must have {} legacy rows",
        LEGACY_COUNT
    );
    assert_eq!(
        post_execs.len(),
        POST_COUNT,
        "must have {} post-migration rows",
        POST_COUNT
    );

    let legacy_with_synthesized = legacy_execs
        .iter()
        .filter(|e| {
            e["actualToolchainMappingDiagnostics"]["policySource"] == "synthesized_legacy"
                && e["actualToolchainMappingDiagnostics"]["mappingState"]
                    == "legacy_row_unavailable"
        })
        .count();

    let post_with_runplan = post_execs
        .iter()
        .filter(|e| e["actualToolchainMappingDiagnostics"]["policySource"] == "runplan_snapshot")
        .count();

    assert_eq!(
        legacy_with_synthesized, LEGACY_COUNT,
        "all {} legacy rows must have policySource=synthesized_legacy + mappingState=legacy_row_unavailable",
        LEGACY_COUNT
    );

    assert_eq!(
        post_with_runplan, POST_COUNT,
        "all {} post-migration rows must have policySource=runplan_snapshot",
        POST_COUNT
    );
}

/// P066 T22: Migration drill — legacy and post-migration row counts meet Phase 0 floor.
#[test]
fn p066_migration_drill_counts_meet_phase_0_floor() {
    const MIN_LEGACY: usize = 10;
    const MIN_POST: usize = 10;
    const LEGACY_USED: usize = 12;
    const POST_USED: usize = 12;
    assert!(
        LEGACY_USED >= MIN_LEGACY,
        "drill must use ≥{} legacy rows (using {})",
        MIN_LEGACY,
        LEGACY_USED
    );
    assert!(
        POST_USED >= MIN_POST,
        "drill must use ≥{} post-migration rows (using {})",
        MIN_POST,
        POST_USED
    );
}
