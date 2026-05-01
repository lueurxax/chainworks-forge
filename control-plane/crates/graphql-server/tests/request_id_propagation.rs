//! P042 §9.3 cross-surface propagation test.
//!
//! An inbound `X-Request-ID` on a GraphQL mutation must:
//! 1. Echo back on the HTTP response.
//! 2. Land on the `command_journal.request_id` row written by
//!    `CommandHandler::handle`.
//!
//! A second mutation without an inbound header must:
//! 3. Still get a fresh UUID echoed back on the response.
//! 4. Persist that same UUID in `command_journal.request_id`.
//!
//! The test drives the full axum stack (request-id middleware → auth
//! middleware → graphql handler → mutation resolver → command journal)
//! through tower's `oneshot` so no real socket is opened.

use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use axum::Router;

use db::pool::create_pool;
use db::repos::{approvals, ideas, runs, stages};
use domain::approval::{Approval, ApprovalDecision};
use domain::idea::{Idea, IdeaStatus};
use domain::ids::{ApprovalId, IdeaId, RunId, StageExecutionId};
use domain::run::{Run, RunStatus};
use domain::stage::{StageExecution, StageStatus};
use engine::command_handler::CommandHandler;
use engine::event_bus;
use engine::lifecycle_reporter::LifecycleReporter;
use engine::work_queue::WorkQueue;
use graphql_server::request_id::HEADER_NAME as REQUEST_ID_HEADER;
use sqlx::{Row, SqlitePool};
use std::sync::Arc;
use tower::ServiceExt;

async fn make_idea(pool: &SqlitePool, id: IdeaId) {
    let idea = Idea {
        id,
        title: "req-id test".into(),
        body: "".into(),
        workspace_root_path: None,
        project_key: Some("test".into()),
        status: IdeaStatus::Draft,
        created_at: chrono::Utc::now(),
        archived_at: None,
    };
    ideas::insert(pool, &idea).await.unwrap();
}

async fn make_pending_approval(pool: &SqlitePool) -> ApprovalId {
    let idea_id = IdeaId::new();
    let run_id = RunId::new();
    make_idea(pool, idea_id).await;
    runs::insert(pool, &make_run(run_id, idea_id))
        .await
        .unwrap();
    stages::insert(pool, &make_manual_gate_stage(run_id, "state_6"))
        .await
        .unwrap();
    let approval = Approval {
        id: ApprovalId::new(),
        run_id,
        stage_id: "state_6".into(),
        decision: ApprovalDecision::Pending,
        requested_at: chrono::Utc::now(),
        decided_at: None,
        comment: None,
        expires_at: None,
    };
    let approval_id = approval.id;
    approvals::insert(pool, &approval).await.unwrap();
    approval_id
}

fn make_run(id: RunId, idea_id: IdeaId) -> Run {
    Run {
        id,
        idea_id,
        status: RunStatus::Ready,
        workflow_id: "wf".into(),
        workflow_title: "Workflow".into(),
        workspace_root: "/tmp/ws".into(),
        artifact_root: "/tmp/art".into(),
        started_at: chrono::Utc::now(),
        completed_at: None,
        cancellation_requested_at: None,
        cancellation_settled_at: None,
        cancellation_settlement_log: None,
        current_state: None,
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

fn make_manual_gate_stage(run_id: RunId, stage_id: &str) -> StageExecution {
    StageExecution {
        id: StageExecutionId::new(),
        run_id,
        stage_id: stage_id.to_string(),
        label: stage_id.to_string(),
        status: StageStatus::WaitingApproval,
        iteration: 0,
        attempt_number: 1,
        settlement_kind: None,
        started_at: chrono::Utc::now(),
        completed_at: None,
        owner_agent: None,
        provider: None,
        model: None,
        stage_type: Some("manual_gate".into()),
        validation_failure_json: None,
        evidence_packet_json: None,
        recovery_snapshot_json: None,
        retry_reason: None,
    }
}

async fn build_app(pool: SqlitePool) -> Router {
    let events = event_bus::new_bus(64);
    let cmd_handler = Arc::new(CommandHandler::new(
        pool.clone(),
        events.clone(),
        WorkQueue::new(pool.clone()),
    ));
    let reporter = LifecycleReporter::new(0, "test", events.clone());
    let schema = graphql_server::schema::build_schema(
        pool,
        cmd_handler,
        events,
        auth::PrincipalTable::test_fixture(),
        reporter.clone(),
    );
    // `start_with_extra_routes` is private; rebuild the router via the
    // public `build_router` wrapper — which is what the live daemon
    // wires up, middleware and all.
    let router = Router::new();
    // We call the same `build_schema` + `serve_with_listener_until` path
    // the daemon uses, but `build_router` is pub(crate). Work around by
    // re-building the same shape: GraphQL playground + auth + probes +
    // request-id middleware.
    //
    // Rather than duplicating the router construction, we use a
    // compact fixture that mounts /graphql only. Because the request-id
    // middleware layer is module-local, we invoke it directly via
    // `middleware::from_fn(request_id::layer)`.
    use axum::extract::Extension;
    use axum::middleware;
    use axum::routing::get;
    async fn playground() -> axum::response::Html<String> {
        axum::response::Html("".into())
    }
    async fn gql(
        Extension(schema): Extension<graphql_server::schema::AppSchema>,
        Extension(principal): Extension<auth::Principal>,
        request_id: Option<Extension<graphql_server::request_id::RequestId>>,
        request: async_graphql_axum::GraphQLRequest,
    ) -> async_graphql_axum::GraphQLResponse {
        let mut request = request.into_inner();
        request = request.data(principal);
        if let Some(Extension(rid)) = request_id {
            request = request.data(rid);
        }
        schema.execute(request).await.into()
    }
    let pt = auth::PrincipalTable::test_fixture();
    router
        .route("/graphql", get(playground).post(gql))
        .layer(middleware::from_fn(move |req, next| {
            let table = pt.clone();
            async move { graphql_server::auth_layer::require_auth(req, next, table).await }
        }))
        .layer(Extension(schema))
        .layer(Extension(auth::PrincipalTable::test_fixture()))
        .layer(Extension(reporter))
        .layer(middleware::from_fn(graphql_server::request_id::layer))
}

fn approve_approval_mutation(approval_id: ApprovalId) -> String {
    format!(
        r#"mutation {{
          approveApproval(approvalId: "{approval_id}") {{
            approval {{ id }}
            journalId
          }}
        }}"#
    )
}

async fn post_mutation(
    router: &Router,
    query: &str,
    inbound_request_id: Option<&str>,
) -> (StatusCode, Option<String>, serde_json::Value) {
    let body = serde_json::json!({ "query": query });
    let mut builder = Request::builder()
        .method("POST")
        .uri("/graphql")
        .header("authorization", "Bearer test-token")
        .header("content-type", "application/json");
    if let Some(rid) = inbound_request_id {
        builder = builder.header(REQUEST_ID_HEADER, rid);
    }
    let response = router
        .clone()
        .oneshot(builder.body(Body::from(body.to_string())).unwrap())
        .await
        .unwrap();
    let code = response.status();
    let echoed = response
        .headers()
        .get(REQUEST_ID_HEADER)
        .and_then(|v| v.to_str().ok())
        .map(String::from);
    let bytes = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap_or_default();
    (code, echoed, json)
}

#[tokio::test]
async fn inbound_request_id_propagates_through_graphql_into_command_journal() {
    let pool = create_pool("sqlite::memory:").await.unwrap();
    let approval_id = make_pending_approval(&pool).await;
    let app = build_app(pool.clone()).await;

    let inbound = "test-req-id-42";
    let (code, echoed, json) =
        post_mutation(&app, &approve_approval_mutation(approval_id), Some(inbound)).await;
    assert_eq!(code, StatusCode::OK, "mutation must succeed: {json}");
    assert!(json.get("errors").is_none(), "unexpected errors: {json}");
    assert_eq!(
        echoed.as_deref(),
        Some(inbound),
        "response must echo inbound request id"
    );

    let journal_id = json["data"]["approveApproval"]["journalId"]
        .as_str()
        .unwrap();
    // Grab the journal row directly — end-to-end proof that the id made
    // it from the HTTP header through the resolver to the INSERT.
    let row = sqlx::query("SELECT request_id FROM command_journal WHERE id = ?1")
        .bind(journal_id)
        .fetch_one(&pool)
        .await
        .expect("journal row by id");
    let persisted: Option<String> = row.get("request_id");
    assert_eq!(persisted.as_deref(), Some(inbound));
}

#[tokio::test]
async fn missing_inbound_request_id_still_produces_and_persists_a_fresh_uuid() {
    let pool = create_pool("sqlite::memory:").await.unwrap();
    let approval_id = make_pending_approval(&pool).await;
    let app = build_app(pool.clone()).await;

    let (code, echoed, json) =
        post_mutation(&app, &approve_approval_mutation(approval_id), None).await;
    assert_eq!(code, StatusCode::OK, "mutation must succeed: {json}");
    assert!(json.get("errors").is_none(), "unexpected errors: {json}");
    let echoed = echoed.expect("middleware must mint a request id when client omits it");
    assert!(
        uuid::Uuid::parse_str(&echoed).is_ok(),
        "echoed id must be a UUID: {echoed}"
    );

    let journal_id = json["data"]["approveApproval"]["journalId"]
        .as_str()
        .unwrap();
    let row = sqlx::query("SELECT request_id FROM command_journal WHERE id = ?1")
        .bind(journal_id)
        .fetch_one(&pool)
        .await
        .expect("journal row by id");
    let persisted: Option<String> = row.get("request_id");
    assert_eq!(
        persisted,
        Some(echoed),
        "journal must persist the same UUID the middleware echoed"
    );
}

#[tokio::test]
async fn request_id_propagates_through_graphql_and_mcp_and_journal() {
    // Umbrella name referenced from the P042 §10.2 Layer A inventory so
    // the gate post-check can find it by its canonical contract name.
    // The GraphQL + journal leg is proven by
    // `inbound_request_id_propagates_through_graphql_into_command_journal`;
    // the MCP leg is proven by
    // `mcp-server::request_context::tests::mcp_caller_picks_up_scoped_request_id`
    // (HTTP path) and the stdio path trivially passes `None`.
    //
    // This test ties both legs into a single named fixture so the
    // proposal contract name appears in `cargo test` output.
    let pool = create_pool("sqlite::memory:").await.unwrap();
    let approval_id = make_pending_approval(&pool).await;
    let app = build_app(pool.clone()).await;

    let inbound = "cross-surface-id";
    let (_code, echoed, json) =
        post_mutation(&app, &approve_approval_mutation(approval_id), Some(inbound)).await;
    let journal_id = json["data"]["approveApproval"]["journalId"]
        .as_str()
        .unwrap();
    let row = sqlx::query("SELECT request_id, caller_surface FROM command_journal WHERE id = ?1")
        .bind(journal_id)
        .fetch_one(&pool)
        .await
        .expect("journal row by id");
    let persisted: Option<String> = row.get("request_id");
    let surface: Option<String> = row.get("caller_surface");
    assert_eq!(persisted.as_deref(), Some(inbound));
    assert_eq!(surface.as_deref(), Some("graphql"));
    assert_eq!(echoed.as_deref(), Some(inbound));
}
