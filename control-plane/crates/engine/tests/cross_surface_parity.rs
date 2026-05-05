//! Proposal 029 §11.3 / §9.1 — cross-surface parity test.
//!
//! During Stage A of the MCP rollout, GraphQL mutations and MCP tool calls
//! both construct `Command` values from the same `domain::commands` enum
//! and dispatch them through the same `CommandHandler`. The *shape* of
//! the command is compile-time identical because both surfaces use the
//! same Rust types.
//!
//! This test is the first-wave canary for semantic parity — i.e. the
//! invariant that "same `Command` variant + same inputs ⇒ same observable
//! run outcome". If the two surfaces ever diverge (for instance, one
//! pre-processes the command differently before dispatching), this test
//! will catch it as a difference in persisted run state.
//!
//! The test drives `CommandHandler::handle` twice with identical
//! `StartRunCmd` payloads — once with `CallerContext::mcp` (representing
//! the MCP path) and once with `CallerContext::graphql` (representing the
//! GraphQL path) — and asserts both produce a `RunStarted` result whose
//! persisted `Run` row has the same workflow identity and delivery-config
//! semantics. The journal rows differ by `caller_surface` (that's the
//! point of P029), but everything else must match.

use chrono::Utc;
use db::pool::create_pool;
use db::repos::{ideas, runs};
use domain::commands::{CallerContext, Command, PrincipalClass, StartRunCmd};
use domain::idea::{Idea, IdeaStatus};
use domain::ids::IdeaId;
use engine::command_handler::{CommandHandler, CommandResult};
use engine::event_bus;
use engine::work_queue::WorkQueue;
use sqlx::SqlitePool;
use std::process::Command as ProcessCommand;

async fn test_pool() -> SqlitePool {
    create_pool("sqlite::memory:")
        .await
        .expect("in-memory pool failed")
}

fn make_idea(id: IdeaId) -> Idea {
    Idea {
        id,
        title: "Parity test idea".into(),
        body: "body".into(),
        workspace_root_path: None,
        project_key: None,
        status: IdeaStatus::Active,
        created_at: Utc::now(),
        archived_at: None,
    }
}

fn make_handler(pool: SqlitePool) -> CommandHandler {
    let events = event_bus::new_bus(64);
    let work_queue = WorkQueue::new(pool.clone());
    CommandHandler::new(pool, events, work_queue)
}

fn test_workflow_yaml_path() -> String {
    format!(
        "{}/../../../examples/workflows/workflow.yaml",
        env!("CARGO_MANIFEST_DIR")
    )
}

fn test_agent_catalog_yaml_path() -> String {
    format!(
        "{}/../../../examples/agents/agents.yaml",
        env!("CARGO_MANIFEST_DIR")
    )
}

fn make_start_cmd(idea_id: IdeaId, delivery_configuration_json: String) -> Command {
    Command::StartRun(StartRunCmd {
        idea_id,
        workflow_id: "wf-parity".into(),
        workflow_title: "Parity Workflow".into(),
        workspace_root: "/tmp/parity-ws".into(),
        artifact_root: "/tmp/parity-art".into(),
        delivery_configuration_json: Some(delivery_configuration_json),
        workflow_yaml_path: test_workflow_yaml_path(),
        agent_catalog_yaml_path: test_agent_catalog_yaml_path(),
        review_routing_json: None,
        rollout_contract_preflight_policy_json: None,
    })
}

#[tokio::test]
async fn test_graphql_and_mcp_produce_identical_run_for_start_run() {
    // Two separate ideas so each invocation has its own run row. We're
    // asserting structural parity between the two persisted runs, NOT
    // deduplication (which would be a separate proposal).
    let pool = test_pool().await;
    let idea_mcp = IdeaId::new();
    let idea_gql = IdeaId::new();
    ideas::insert(&pool, &make_idea(idea_mcp)).await.unwrap();
    ideas::insert(&pool, &make_idea(idea_gql)).await.unwrap();

    let handler = make_handler(pool.clone());
    let repo = tempfile::tempdir().unwrap();
    ProcessCommand::new("git")
        .args(["init", "--initial-branch", "main"])
        .current_dir(repo.path())
        .output()
        .expect("git init should run");
    ProcessCommand::new("git")
        .args(["config", "user.email", "test@example.com"])
        .current_dir(repo.path())
        .output()
        .expect("git config user.email should run");
    ProcessCommand::new("git")
        .args(["config", "user.name", "Test User"])
        .current_dir(repo.path())
        .output()
        .expect("git config user.name should run");
    std::fs::write(repo.path().join("README.md"), "initial\n").unwrap();
    ProcessCommand::new("git")
        .args(["add", "README.md"])
        .current_dir(repo.path())
        .output()
        .expect("git add should run");
    ProcessCommand::new("git")
        .args(["commit", "-m", "initial"])
        .current_dir(repo.path())
        .output()
        .expect("git commit should run");
    let worktrees = tempfile::tempdir().unwrap();
    let delivery_configuration_json = format!(
        r#"{{
            "repo_identifier":"repo-parity",
            "repo_root":"{}",
            "base_branch":"main",
            "worktree_base_path":"{}",
            "target_branch":"cw/parity",
            "release_target_id":"sandbox-target",
            "release_mode":"sandbox"
        }}"#,
        repo.path().display(),
        worktrees.path().display()
    );

    // ── MCP path ────────────────────────────────────────────────────
    let mcp_caller = CallerContext::mcp("op-mcp", &PrincipalClass::Operator, "runs.start");
    let mcp_result = handler
        .handle(
            make_start_cmd(idea_mcp, delivery_configuration_json.clone()),
            mcp_caller,
        )
        .await
        .expect("MCP start_run must succeed");
    let mcp_run_id = match mcp_result.result {
        CommandResult::RunStarted { run_id } => run_id,
        _ => panic!("MCP path: expected RunStarted from start_run"),
    };

    // ── GraphQL path ────────────────────────────────────────────────
    let gql_caller = CallerContext::graphql("op-gql", &PrincipalClass::Operator, "startRun");
    let gql_result = handler
        .handle(
            make_start_cmd(idea_gql, delivery_configuration_json),
            gql_caller,
        )
        .await
        .expect("GraphQL start_run must succeed");
    let gql_run_id = match gql_result.result {
        CommandResult::RunStarted { run_id } => run_id,
        _ => panic!("GraphQL path: expected RunStarted from start_run"),
    };

    // Both paths must have minted journal_ids (non-empty uuids).
    assert!(!mcp_result.journal_id.is_empty());
    assert!(!gql_result.journal_id.is_empty());
    assert_ne!(
        mcp_result.journal_id, gql_result.journal_id,
        "journal_ids must be unique per invocation"
    );

    // Fetch both runs and compare structural fields (everything except
    // identity and timestamps, which are trivially unique).
    let mcp_run = runs::find_by_id(&pool, mcp_run_id)
        .await
        .unwrap()
        .expect("MCP run row");
    let gql_run = runs::find_by_id(&pool, gql_run_id)
        .await
        .unwrap()
        .expect("GraphQL run row");

    assert_eq!(mcp_run.workflow_id, gql_run.workflow_id);
    assert_eq!(mcp_run.workflow_title, gql_run.workflow_title);
    assert_eq!(mcp_run.workspace_root, gql_run.workspace_root);
    assert_eq!(mcp_run.artifact_root, gql_run.artifact_root);
    assert_eq!(mcp_run.status, gql_run.status);
    assert_eq!(
        mcp_run.delivery_configuration_json,
        gql_run.delivery_configuration_json
    );
    assert_eq!(
        mcp_run.workflow_yaml_path, gql_run.workflow_yaml_path,
        "both surfaces must freeze the same workflow YAML path"
    );
    assert_eq!(
        mcp_run.agent_catalog_yaml_path, gql_run.agent_catalog_yaml_path,
        "both surfaces must freeze the same agent catalog path"
    );

    // The journal must record *different* caller_surface values for the
    // two invocations — that's the audit invariant P029 is adding.
    let surfaces: Vec<String> = sqlx::query_scalar(
        "SELECT caller_surface FROM command_journal
         WHERE command_type = 'StartRun' ORDER BY created_at ASC",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(surfaces, vec!["mcp".to_string(), "graphql".to_string()]);
}
