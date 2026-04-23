use std::sync::Arc;

use async_graphql::{futures_util::StreamExt, Request};
use db::pool::create_pool;
use engine::command_handler::CommandHandler;
use engine::event_bus;
use engine::lifecycle_reporter::LifecycleReporter;
use engine::work_queue::WorkQueue;
use graphql_server::schema::{build_schema, AppSchema};

fn make_schema(pool: sqlx::SqlitePool) -> AppSchema {
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

async fn assert_observer_forbidden(schema: &AppSchema, query: &str) {
    let response = schema
        .execute(Request::new(query).data(auth::Principal::new(
            "observer",
            auth::PrincipalClass::Observer,
        )))
        .await;

    assert!(
        response
            .errors
            .iter()
            .any(|error| error.message == "forbidden"),
        "expected forbidden error, got {:?}",
        response.errors
    );
}

async fn assert_observer_subscription_forbidden(schema: &AppSchema, subscription: &str) {
    let mut stream = schema.execute_stream(Request::new(subscription).data(auth::Principal::new(
        "observer",
        auth::PrincipalClass::Observer,
    )));

    let response = stream
        .next()
        .await
        .expect("subscription should return an initial authorization response");

    assert!(
        response
            .errors
            .iter()
            .any(|error| error.message == "forbidden"),
        "expected forbidden subscription error, got {:?}",
        response.errors
    );
}

#[tokio::test]
async fn proposal_031_idea_queries_require_operator_read() {
    let pool = create_pool("sqlite::memory:").await.unwrap();
    let schema = make_schema(pool);

    assert_observer_forbidden(&schema, "{ ideas { id title body } }").await;
    assert_observer_forbidden(
        &schema,
        r#"{ idea(id: "00000000-0000-0000-0000-000000000000") { id title body } }"#,
    )
    .await;
}

#[tokio::test]
async fn proposal_031_steward_queries_require_operator_read() {
    let pool = create_pool("sqlite::memory:").await.unwrap();
    let schema = make_schema(pool);

    assert_observer_forbidden(&schema, "{ stewardAnalyses { id status } }").await;
    assert_observer_forbidden(
        &schema,
        r#"{ stewardAnalysis(id: "analysis-1") { id status } }"#,
    )
    .await;
    assert_observer_forbidden(
        &schema,
        r#"{ stewardAnalysisRunLinks(analysisId: "analysis-1") { analysisId runId } }"#,
    )
    .await;
    assert_observer_forbidden(
        &schema,
        r#"{ stewardRecommendations(analysisId: "analysis-1") { analysisId id } }"#,
    )
    .await;
}

#[tokio::test]
async fn proposal_031_stage_detail_queries_require_operator_read() {
    let pool = create_pool("sqlite::memory:").await.unwrap();
    let schema = make_schema(pool);
    let stage_execution_id = "00000000-0000-0000-0000-000000000000";

    assert_observer_forbidden(
        &schema,
        &format!(r#"{{ stage(id: "{stage_execution_id}") {{ id freshnessState }} }}"#),
    )
    .await;
    assert_observer_forbidden(
        &schema,
        &format!(
            r#"{{ agentExecutions(stageExecutionId: "{stage_execution_id}") {{ id provider }} }}"#
        ),
    )
    .await;
}

#[tokio::test]
async fn proposal_031_sensitive_subscriptions_require_operator_read() {
    let pool = create_pool("sqlite::memory:").await.unwrap();
    let schema = make_schema(pool);
    let run_id = "00000000-0000-0000-0000-000000000000";

    assert_observer_subscription_forbidden(
        &schema,
        &format!(r#"subscription {{ runStatusChanged(runId: "{run_id}") {{ id }} }}"#),
    )
    .await;
    assert_observer_subscription_forbidden(
        &schema,
        &format!(r#"subscription {{ stageStatusChanged(runId: "{run_id}") {{ id }} }}"#),
    )
    .await;
    assert_observer_subscription_forbidden(
        &schema,
        "subscription { approvalRequested { id diagnosticId serverDebugDetail } }",
    )
    .await;
    assert_observer_subscription_forbidden(
        &schema,
        "subscription { approvalResolved { id diagnosticId serverDebugDetail } }",
    )
    .await;
    assert_observer_subscription_forbidden(
        &schema,
        &format!(
            r#"subscription {{ runtimeStatusChanged(runId: "{run_id}") {{ runId stageId agentId provider eventKind }} }}"#
        ),
    )
    .await;
}
