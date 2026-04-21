//! Proposal 029 §8 command-journal caller-metadata audit tests.
//!
//! `CommandHandler::handle` inserts one row into `command_journal` for every
//! command invocation **before** executing the command. The row must carry
//! caller provenance so audit can tell whether the command arrived via MCP
//! or GraphQL, and which principal + surface-specific tool/mutation invoked
//! it. Each test below drives one `Command` variant through one caller
//! surface and reads the row back via raw SQL to prove:
//!
//! - `caller_surface` is set ("mcp" or "graphql")
//! - `caller_principal_id` matches what was passed in `CallerContext`
//! - `caller_principal_class` matches the principal class
//! - `caller_tool` matches the surface-specific tool/mutation name
//!
//! The commands themselves may fail (e.g. cancelling a non-existent run) —
//! the journal INSERT happens before execution so it's unaffected.

use chrono::Utc;
use db::pool::create_pool;
use domain::commands::{
    ApproveStageCmd, CallerContext, CallerSurface, CancelRunCmd, Command,
    OverrideArtifactContractCmd, PrincipalClass, RunStewardAnalysisCmd, StartRunCmd,
};
use domain::ids::{IdeaId, RunId};
use engine::command_handler::CommandHandler;
use engine::event_bus;
use engine::work_queue::WorkQueue;
use sqlx::{Row, SqlitePool};

async fn test_pool() -> SqlitePool {
    create_pool("sqlite::memory:")
        .await
        .expect("in-memory pool failed")
}

fn make_handler(pool: SqlitePool) -> CommandHandler {
    let events = event_bus::new_bus(64);
    let work_queue = WorkQueue::new(pool.clone());
    CommandHandler::new(pool, events, work_queue)
}

/// Fetch the most recent command_journal row and return its caller-metadata
/// columns as a tuple for easy assertion.
async fn latest_row(
    pool: &SqlitePool,
) -> (
    String, // command_type
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
) {
    let row = sqlx::query(
        "SELECT command_type, caller_surface, caller_principal_id, caller_principal_class, caller_tool
         FROM command_journal ORDER BY created_at DESC LIMIT 1",
    )
    .fetch_one(pool)
    .await
    .expect("read back latest journal row");
    (
        row.get::<String, _>(0),
        row.get::<Option<String>, _>(1),
        row.get::<Option<String>, _>(2),
        row.get::<Option<String>, _>(3),
        row.get::<Option<String>, _>(4),
    )
}

fn start_run_cmd() -> Command {
    // Use workflow paths from examples/ — CommandHandler compiles the plan
    // at handle time, so we need real YAML files. On compile failure the
    // handler still records the journal row first, so the audit assertion
    // works either way — but we give valid paths so the test doesn't
    // accidentally depend on specific error messaging.
    let manifest = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workflow = manifest.join("../../../examples/workflows/workflow.yaml");
    let catalog = manifest.join("../../../examples/agents/agents.yaml");
    Command::StartRun(StartRunCmd {
        idea_id: IdeaId::new(),
        workflow_id: "wf-audit".into(),
        workflow_title: "Audit Workflow".into(),
        workspace_root: "/tmp/ws".into(),
        artifact_root: "/tmp/art".into(),
        delivery_configuration_json: None,
        workflow_yaml_path: workflow.to_string_lossy().into_owned(),
        agent_catalog_yaml_path: catalog.to_string_lossy().into_owned(),
    })
}

// ── MCP-side audit rows ─────────────────────────────────────────────────

#[tokio::test]
async fn test_command_journal_row_has_caller_mcp_for_runs_start() {
    let pool = test_pool().await;
    let handler = make_handler(pool.clone());

    let caller = CallerContext::mcp("op-1", &PrincipalClass::Operator, "runs.start");
    // The command itself may fail (fixture idea row doesn't exist), but
    // the journal insert happens first — that's what we're asserting.
    let _ = handler.handle(start_run_cmd(), caller).await;

    let (cmd_type, surface, pid, class, tool) = latest_row(&pool).await;
    assert_eq!(cmd_type, "StartRun");
    assert_eq!(surface.as_deref(), Some("mcp"));
    assert_eq!(pid.as_deref(), Some("op-1"));
    assert_eq!(class.as_deref(), Some("operator"));
    assert_eq!(tool.as_deref(), Some("runs.start"));
}

#[tokio::test]
async fn test_command_journal_row_has_caller_mcp_for_approvals_resolve() {
    let pool = test_pool().await;
    let handler = make_handler(pool.clone());

    let caller = CallerContext::mcp("op-2", &PrincipalClass::Operator, "approvals.resolve");
    let cmd = Command::ApproveStage(ApproveStageCmd {
        run_id: RunId::new(),
        stage_id: "state_6".into(),
        comment: None,
    });
    let _ = handler.handle(cmd, caller).await;

    let (cmd_type, surface, pid, class, tool) = latest_row(&pool).await;
    assert_eq!(cmd_type, "ApproveStage");
    assert_eq!(surface.as_deref(), Some("mcp"));
    assert_eq!(pid.as_deref(), Some("op-2"));
    assert_eq!(class.as_deref(), Some("operator"));
    assert_eq!(tool.as_deref(), Some("approvals.resolve"));
}

#[tokio::test]
async fn test_command_journal_row_has_caller_mcp_for_steward_run_analysis() {
    let pool = test_pool().await;
    let handler = make_handler(pool.clone());

    let caller = CallerContext::mcp("op-3", &PrincipalClass::Operator, "steward.run_analysis");
    let cmd = Command::RunStewardAnalysis(RunStewardAnalysisCmd {
        reason: "manual".into(),
        artifact_base: None,
    });
    // This command needs steward runtime infra that the minimal fixture
    // doesn't set up — it will fail during execute. That's fine: the
    // journal insert happens BEFORE execute and is what we're auditing.
    let _ = handler.handle(cmd, caller).await;

    let (cmd_type, surface, pid, class, tool) = latest_row(&pool).await;
    assert_eq!(cmd_type, "RunStewardAnalysis");
    assert_eq!(surface.as_deref(), Some("mcp"));
    assert_eq!(pid.as_deref(), Some("op-3"));
    assert_eq!(class.as_deref(), Some("operator"));
    assert_eq!(tool.as_deref(), Some("steward.run_analysis"));
}

#[tokio::test]
async fn override_artifact_contract_rejects_non_operator_at_command_boundary() {
    let pool = test_pool().await;
    let handler = make_handler(pool);
    let caller = CallerContext::mcp(
        "observer-1",
        &PrincipalClass::Observer,
        "artifacts.override_contract",
    );

    let result = handler
        .handle(
            Command::OverrideArtifactContract(OverrideArtifactContractCmd {
                run_id: RunId::new(),
                contract_id: "audit_report_v1".into(),
                override_type: "implementation_status".into(),
                from_status: "needs_code_fixes".into(),
                to_status: "implemented".into(),
                reason: "observer must not create canonical override truth".into(),
                source_artifacts: vec![],
                expires_at_stage: "state_11_manual_release".into(),
            }),
            caller,
        )
        .await;

    let error = match result {
        Ok(_) => panic!("shared CommandHandler path must reject non-operator overrides"),
        Err(error) => error,
    };
    assert!(
        error
            .to_string()
            .contains("OverrideArtifactContract requires operator principal"),
        "shared CommandHandler path must enforce override authorization, not only MCP edge code"
    );
}

// ── GraphQL-side audit rows ─────────────────────────────────────────────

#[tokio::test]
async fn test_command_journal_row_has_caller_graphql_for_start_run() {
    let pool = test_pool().await;
    let handler = make_handler(pool.clone());

    let caller = CallerContext::graphql("gql-op-1", &PrincipalClass::Operator, "startRun");
    let _ = handler.handle(start_run_cmd(), caller).await;

    let (cmd_type, surface, pid, class, tool) = latest_row(&pool).await;
    assert_eq!(cmd_type, "StartRun");
    assert_eq!(surface.as_deref(), Some("graphql"));
    assert_eq!(pid.as_deref(), Some("gql-op-1"));
    assert_eq!(class.as_deref(), Some("operator"));
    assert_eq!(tool.as_deref(), Some("startRun"));
}

#[tokio::test]
async fn test_command_journal_row_has_caller_graphql_for_approve_stage() {
    let pool = test_pool().await;
    let handler = make_handler(pool.clone());

    let caller = CallerContext::graphql("gql-op-2", &PrincipalClass::Operator, "approveStage");
    let cmd = Command::ApproveStage(ApproveStageCmd {
        run_id: RunId::new(),
        stage_id: "state_3".into(),
        comment: None,
    });
    let _ = handler.handle(cmd, caller).await;

    let (cmd_type, surface, pid, class, tool) = latest_row(&pool).await;
    assert_eq!(cmd_type, "ApproveStage");
    assert_eq!(surface.as_deref(), Some("graphql"));
    assert_eq!(pid.as_deref(), Some("gql-op-2"));
    assert_eq!(class.as_deref(), Some("operator"));
    assert_eq!(tool.as_deref(), Some("approveStage"));
}

// ── Schema-compatibility invariant ──────────────────────────────────────
//
// Pre-P029 rows (written before migration 011 added caller_* columns) show
// up as NULL in every caller_* column. The schema must allow NULL and
// downstream readers must handle the absent case without panicking.

#[tokio::test]
async fn test_command_journal_caller_columns_nullable_for_pre_p029_rows() {
    let pool = test_pool().await;

    // Write a row directly, leaving every P029 caller_* column NULL — this
    // mirrors the shape a row inserted by a pre-P029 daemon would have.
    let id = uuid::Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO command_journal
         (id, command_type, payload_json, result_status, created_at,
          caller_surface, caller_principal_id, caller_principal_class, caller_tool)
         VALUES (?1, 'CancelRun', '{}', 'pending', ?2, NULL, NULL, NULL, NULL)",
    )
    .bind(&id)
    .bind(&now)
    .execute(&pool)
    .await
    .expect("INSERT with NULL caller_* columns must succeed (schema nullable)");

    // Also drive one real P029-shaped row so we can read both back.
    let handler = make_handler(pool.clone());
    let caller = CallerContext::mcp("op-post", &PrincipalClass::Operator, "runs.cancel");
    let _ = handler
        .handle(
            Command::CancelRun(CancelRunCmd {
                run_id: RunId::new(),
            }),
            caller,
        )
        .await;

    // Count pre-P029 rows (all caller_* NULL) — must be exactly one.
    let pre_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM command_journal
         WHERE caller_surface IS NULL
           AND caller_principal_id IS NULL
           AND caller_principal_class IS NULL
           AND caller_tool IS NULL",
    )
    .fetch_one(&pool)
    .await
    .expect("count pre-P029 rows");
    assert_eq!(
        pre_count, 1,
        "the synthetic pre-P029 row must still be present and queryable"
    );

    // Count P029-shaped rows (every caller_* populated) — must be exactly one.
    let post_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM command_journal
         WHERE caller_surface IS NOT NULL
           AND caller_principal_id IS NOT NULL
           AND caller_principal_class IS NOT NULL
           AND caller_tool IS NOT NULL",
    )
    .fetch_one(&pool)
    .await
    .expect("count post-P029 rows");
    assert_eq!(post_count, 1);

    // And CallerSurface::Mcp round-trips through the display impl as "mcp".
    assert_eq!(CallerSurface::Mcp.to_string(), "mcp");
    assert_eq!(CallerSurface::Graphql.to_string(), "graphql");
}
