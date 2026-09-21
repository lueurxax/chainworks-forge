use async_graphql::Request;
use serde_json::{json, Value};
use std::sync::Arc;

const SOURCE: &str = "00000000-0000-4000-8000-000000000039";
const SUCCESSOR: &str = "00000000-0000-4000-8000-000000000040";
const OPERATION: &str = "00000000-0000-4000-8000-000000000041";

async fn fixture() -> (graphql_server::schema::AppSchema, sqlx::SqlitePool) {
    let pool = db::pool::create_pool("sqlite::memory:").await.unwrap();
    db::writer::register_shared_writer(&pool, Arc::new(db::writer::DbWriter::new(pool.clone())))
        .await
        .unwrap();
    sqlx::query("INSERT INTO ideas(id,title,body,created_at) VALUES (?,'P039','Body','2026-09-20T00:00:00Z')").bind(SOURCE).execute(&pool).await.unwrap();
    for id in [SOURCE, SUCCESSOR] {
        sqlx::query("INSERT INTO runs(id,idea_id,status,workflow_id,workflow_title,workspace_root,artifact_root,started_at) VALUES (?,?,'completed','w','Workflow','/private/workspace','/private/artifacts','2026-09-20T00:00:00Z')")
            .bind(id).bind(SOURCE).execute(&pool).await.unwrap();
    }
    sqlx::query("INSERT INTO command_journal(id,command_type,payload_json,created_at) VALUES (?,'ContinueBlocked','{}','2026-09-20T00:00:00Z')").bind(OPERATION).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO run_continuations(operation_id,source_run_id,idea_id,reserved_successor_run_id,successor_run_id,caller_fingerprint,caller_request_id,intent_sha256,profile,phase,plan_ref,plan_sha256,target_ref,source_witness_sha256,manifest_ref,manifest_sha256,journal_id,created_at,updated_at,deadline_at) VALUES (?,?,?,?,?,'private caller',?,?,'implementation_restart_v1','activated','/private/plan',?,'/private/target',?,'/private/manifest',?,?,'2026-09-20T00:00:00Z','2026-09-20T00:00:00Z','2026-09-21T00:00:00Z')")
        .bind(OPERATION).bind(SOURCE).bind(SOURCE).bind(SUCCESSOR).bind(SUCCESSOR).bind(OPERATION).bind("a".repeat(64)).bind("b".repeat(64)).bind("c".repeat(64)).bind("d".repeat(64)).bind(OPERATION).execute(&pool).await.unwrap();
    let events = engine::event_bus::new_bus(16);
    let handler = Arc::new(engine::command_handler::CommandHandler::new(
        pool.clone(),
        events.clone(),
        engine::work_queue::WorkQueue::new(pool.clone()),
    ));
    let reporter =
        engine::lifecycle_reporter::LifecycleReporter::new(101, "p039-test", events.clone());
    let schema = graphql_server::schema::build_schema(
        pool.clone(),
        handler,
        events,
        auth::PrincipalTable::test_fixture(),
        reporter,
    );
    (schema, pool)
}

fn principal() -> auth::Principal {
    auth::Principal::new("reader", auth::PrincipalClass::Operator)
}

#[tokio::test]
async fn p039_graphql_additive_compact_fields_use_canonical_links_not_manifest_files() {
    let (schema, _pool) = fixture().await;
    let response = schema.execute(Request::new(format!("{{ source: run(id:\"{SOURCE}\") {{ continuedFromRunId continuedAsRunId carryForwardReadback }} successor: run(id:\"{SUCCESSOR}\") {{ continuedFromRunId continuedAsRunId carryForwardReadback }} }}")).data(principal())).await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let data = response.data.into_json().unwrap();
    assert_eq!(data["source"]["continuedAsRunId"], SUCCESSOR);
    assert!(data["source"]["continuedFromRunId"].is_null());
    assert_eq!(data["successor"]["continuedFromRunId"], SOURCE);
    assert!(data["successor"]["continuedAsRunId"].is_null());
    let readback = &data["source"]["carryForwardReadback"];
    assert_eq!(readback["schema_version"], "run_carry_forward_links_v1");
    assert_eq!(
        readback["outgoing"]["manifest_sha256"],
        format!("sha256:{}", "d".repeat(64))
    );
    for forbidden in ["manifest_page", "/private", "private caller", "plan_ref"] {
        assert!(!data.to_string().contains(forbidden));
    }
}

#[tokio::test]
async fn p039_graphql_scoped_links_and_old_rows_are_null_without_disclosure() {
    let (schema, pool) = fixture().await;
    let mut scoped = principal();
    scoped.run_scope = Some(vec![SOURCE.into()]);
    let query = format!(
        "{{ run(id:\"{SOURCE}\") {{ continuedFromRunId continuedAsRunId carryForwardReadback }} }}"
    );
    let response = schema.execute(Request::new(&query).data(scoped)).await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(
        response.data.into_json().unwrap()["run"],
        json!({"continuedFromRunId":null,"continuedAsRunId":null,"carryForwardReadback":null})
    );
    sqlx::query("DELETE FROM run_continuations")
        .execute(&pool)
        .await
        .unwrap();
    let response = schema.execute(Request::new(query).data(principal())).await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(
        response.data.into_json().unwrap()["run"]["carryForwardReadback"],
        Value::Null
    );
}

#[tokio::test]
async fn p039_graphql_middle_run_keeps_incoming_and_held_outgoing_independent() {
    let (schema, pool) = fixture().await;
    let outgoing = uuid::Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO command_journal(id,command_type,payload_json,created_at) VALUES (?,'ContinueBlocked','{}','2026-09-20T00:00:00Z')").bind(&outgoing).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO run_continuations(operation_id,source_run_id,idea_id,reserved_successor_run_id,caller_fingerprint,caller_request_id,intent_sha256,profile,phase,plan_ref,plan_sha256,target_ref,source_witness_sha256,journal_id,holds_json,created_at,updated_at,deadline_at) VALUES (?,?,?,?,'fixture',?,?,'implementation_restart_v1','needs_reconciliation','missing-plan',?,'missing-target',?,?, '[\"effect_outcome_unknown\"]','2026-09-20T00:00:00Z','2026-09-20T00:00:00Z','2026-09-21T00:00:00Z')")
        .bind(&outgoing).bind(SUCCESSOR).bind(SOURCE).bind(uuid::Uuid::new_v4().to_string()).bind(&outgoing).bind("a".repeat(64)).bind("b".repeat(64)).bind("c".repeat(64)).bind(&outgoing).execute(&pool).await.unwrap();
    let response = schema.execute(Request::new(format!("{{ run(id:\"{SUCCESSOR}\") {{ continuedFromRunId continuedAsRunId carryForwardReadback }} }}"))
        .data(principal()).data(engine::run_carry_forward::readback::ReadbackConfig {enabled:false,reconcile_available:true})).await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let data = response.data.into_json().unwrap();
    assert_eq!(data["run"]["continuedFromRunId"], SOURCE);
    assert!(data["run"]["continuedAsRunId"].is_null());
    let links = &data["run"]["carryForwardReadback"];
    assert_eq!(links["incoming"]["operation_id"], OPERATION);
    assert_eq!(links["outgoing"]["operation_id"], outgoing);
    assert_eq!(
        links["outgoing"]["holds"][0]["code"],
        "effect_outcome_unknown"
    );
    assert_eq!(links["outgoing"]["admission"]["enabled"], false);
    assert_eq!(links["outgoing"]["admission"]["reconcile_available"], true);

    let aborting = db::repos::run_continuations::begin_abort(&pool, &outgoing, 1)
        .await
        .unwrap();
    db::repos::run_continuations::finish_abort(&pool, &outgoing, aborting.version)
        .await
        .unwrap();
    let response = schema.execute(Request::new(format!(
        "{{ source: run(id:\"{SOURCE}\") {{ continuedFromRunId continuedAsRunId carryForwardReadback }} middle: run(id:\"{SUCCESSOR}\") {{ continuedFromRunId continuedAsRunId carryForwardReadback }} }}"
    )).data(principal()).data(engine::run_carry_forward::readback::ReadbackConfig {
        enabled: false, reconcile_available: true,
    })).await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let after = response.data.into_json().unwrap();
    assert_eq!(after["middle"]["continuedFromRunId"], SOURCE);
    assert!(after["middle"]["continuedAsRunId"].is_null());
    assert_eq!(
        after["middle"]["carryForwardReadback"]["incoming"],
        links["incoming"]
    );
    assert_eq!(
        after["middle"]["carryForwardReadback"].get("outgoing"),
        Some(&Value::Null)
    );
    assert_eq!(after["source"]["continuedAsRunId"], SUCCESSOR);
    assert_eq!(
        after["source"]["carryForwardReadback"]["outgoing"],
        links["incoming"]
    );
    assert_eq!(
        after["source"]["carryForwardReadback"]["outgoing"]["phase"],
        "activated"
    );
    assert_eq!(
        after["source"]["carryForwardReadback"]["outgoing"]["manifest_sha256"],
        format!("sha256:{}", "d".repeat(64))
    );
    let historical = db::repos::run_continuations::find(&pool, &outgoing)
        .await
        .unwrap()
        .expect("aborted operation remains historically readable");
    assert_eq!(historical.phase, "aborted");
    assert_eq!(historical.source_run_id, SUCCESSOR);
    assert!(historical.successor_run_id.is_none());
}
