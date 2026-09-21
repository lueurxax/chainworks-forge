use auth::{Principal, PrincipalClass};
use domain::{ids::RunId, CapabilityToolId};
use engine::run_carry_forward::{CarryForwardService, ServiceConfig, ServiceError};
use serde_json::{json, Value};
use std::sync::Arc;

async fn fixture() -> (sqlx::SqlitePool, RunId, Principal, tempfile::TempDir) {
    let pool = db::pool::create_pool("sqlite::memory:").await.unwrap();
    db::writer::register_shared_writer(&pool, Arc::new(db::writer::DbWriter::new(pool.clone())))
        .await
        .unwrap();
    let source = RunId::new();
    let idea = uuid::Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO ideas(id,title,body,created_at) VALUES (?,'I','B','2026-09-20T00:00:00Z')",
    )
    .bind(&idea)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO runs(id,idea_id,status,workflow_id,workflow_title,workspace_root,artifact_root,started_at) VALUES (?,?,'blocked','w','W','/repo','/meta','2026-09-20T00:00:00Z')").bind(source.to_string()).bind(&idea).execute(&pool).await.unwrap();
    let mut operator = Principal::new("p039-operator", PrincipalClass::Operator);
    operator.tool_capabilities.extend([
        CapabilityToolId::RunsContinuationPreview,
        CapabilityToolId::RunsContinueBlocked,
        CapabilityToolId::RunsContinuationGet,
        CapabilityToolId::RunsContinuationAbort,
        CapabilityToolId::RunsContinuationReconcile,
        CapabilityToolId::RunsContinuationActivate,
    ]);
    (pool, source, operator, tempfile::tempdir().unwrap())
}

fn request(source: RunId) -> Value {
    json!({"run_id":source,"target":{
    "workflow_yaml_path":"/repo/workflow.yaml","agent_catalog_yaml_path":"/repo/agents.yaml",
    "expected_workflow_snapshot_hash":format!("sha256:{}","a".repeat(64)),"expected_catalog_snapshot_hash":format!("sha256:{}","b".repeat(64)),
    "delivery_configuration_json":json!({"repo_identifier":"repo","repo_root":"/repo","base_branch":"main","worktree_base_path":"/worktrees","target_branch":"main"}).to_string()},
    "selection":{"proposal_input":{"kind":"artifact","id":uuid::Uuid::new_v4()},"reference_inputs":[],"include_dirty_work":true,"excluded_workspace_paths":[]},
    "profile":"implementation_restart_v1","expected_plan_sha256":format!("sha256:{}","c".repeat(64)),"caller_request_id":uuid::Uuid::new_v4(),"reason":"Continue reviewed work"})
}

#[tokio::test]
async fn disabled_admission_has_durable_original_denial_and_no_fence_or_files() {
    let (pool, source, operator, dir) = fixture().await;
    let service = CarryForwardService::new(
        pool.clone(),
        ServiceConfig {
            enabled: false,
            owned_parent: dir.path().into(),
        },
    );
    let request = request(source);
    let denied = service
        .call("runs.continue_blocked", &operator, request.clone())
        .await
        .unwrap();
    assert_eq!(denied["status"], "denied");
    assert_eq!(denied["denial"]["code"], "rollout_hold");
    assert!(
        db::metrics::get_counter_with_label(
            "run_continuation_admission_total",
            "result=denied,reason=rollout_hold"
        ) > 0
    );
    let replay = service
        .call("runs.continue_blocked", &operator, request.clone())
        .await
        .unwrap();
    assert_eq!(replay["status"], "replayed");
    assert_eq!(replay["replay_of"], "denied");
    assert_eq!(replay["denial"], denied["denial"]);
    assert!(
        db::metrics::get_counter_with_label(
            "run_continuation_admission_total",
            "result=replayed,reason=rollout_hold"
        ) > 0
    );
    let mut changed = request.clone();
    changed["reason"] = json!("Different intent");
    let conflict = service
        .call("runs.continue_blocked", &operator, changed)
        .await
        .unwrap();
    assert_eq!(conflict["denial"]["code"], "idempotency_conflict");
    let mut revoked = operator;
    revoked.tool_capabilities.clear();
    let error = service
        .call("runs.continue_blocked", &revoked, request)
        .await
        .unwrap_err();
    assert_eq!(
        error.downcast_ref::<ServiceError>(),
        Some(&ServiceError::Unauthorized)
    );
    let rows:i64=sqlx::query_scalar("SELECT (SELECT COUNT(*) FROM run_continuations)+(SELECT COUNT(*) FROM run_execution_fences)+(SELECT COUNT(*) FROM work_items)+(SELECT COUNT(*) FROM provider_sessions)").fetch_one(&pool).await.unwrap();
    assert_eq!(rows, 0);
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
}

#[tokio::test]
async fn service_preview_returns_typed_hold_without_a_write_or_invented_plan() {
    let (pool, source, operator, dir) = fixture().await;
    let service = CarryForwardService::new(
        pool.clone(),
        ServiceConfig {
            enabled: false,
            owned_parent: dir.path().into(),
        },
    );
    let mut request = request(source);
    let map = request.as_object_mut().unwrap();
    for key in ["expected_plan_sha256", "caller_request_id", "reason"] {
        map.remove(key);
    }
    let result = service
        .call("runs.continuation_preview", &operator, request)
        .await
        .unwrap();
    assert_eq!(
        result["schema_version"],
        "run_carry_forward_preview_hold_v1"
    );
    assert_eq!(result["source_run_id"], source.to_string());
    assert!(result.get("plan_sha256").is_none());
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM run_continuation_commands")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
}

#[tokio::test]
async fn missing_prepared_material_is_a_readback_hold_not_internal_or_repair() {
    use db::repos::run_continuations as c;
    let (pool, source, operator, dir) = fixture().await;
    let op = c::reserve(
        &pool,
        &c::Reservation {
            source_run_id: source.to_string(),
            caller_fingerprint: "fixture".into(),
            caller_request_id: uuid::Uuid::new_v4().to_string(),
            intent_sha256: "a".repeat(64),
            plan_ref: "plan".into(),
            plan_sha256: "b".repeat(64),
            target_ref: "target".into(),
            source_witness_sha256: "c".repeat(64),
            reason: "fixture".into(),
        },
    )
    .await
    .unwrap();
    let claimed = c::claim_preparation_for(&pool, &op.operation_id)
        .await
        .unwrap()
        .unwrap();
    c::begin_step(
        &pool,
        &op.operation_id,
        claimed.generation,
        "manifest",
        "{}",
        "owned",
    )
    .await
    .unwrap();
    c::finish_step(
        &pool,
        &op.operation_id,
        claimed.generation,
        "manifest",
        &"d".repeat(64),
    )
    .await
    .unwrap();
    let prepared = c::mark_prepared(
        &pool,
        &op.operation_id,
        claimed.generation,
        dir.path()
            .join(&op.operation_id)
            .join("manifest.json")
            .to_str()
            .unwrap(),
        &"d".repeat(64),
    )
    .await
    .unwrap();
    let service = CarryForwardService::new(
        pool.clone(),
        ServiceConfig {
            enabled: false,
            owned_parent: dir.path().into(),
        },
    );
    let reply = service
        .call(
            "runs.continuation_get",
            &operator,
            json!({"operation_id":op.operation_id}),
        )
        .await
        .unwrap();
    assert_eq!(reply["phase"], "prepared");
    assert_eq!(reply["verification_status"], "failed");
    assert!(!reply["holds"].as_array().unwrap().is_empty());
    assert_eq!(reply["manifest_page"], json!([]));
    assert_eq!(
        c::find(&pool, &op.operation_id)
            .await
            .unwrap()
            .unwrap()
            .version,
        prepared.version
    );
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
    let activation = json!({"operation_id":op.operation_id,"expected_version":prepared.version,
        "expected_manifest_sha256":format!("sha256:{}", "d".repeat(64)),"caller_request_id":uuid::Uuid::new_v4()});
    let denied = service
        .call("runs.continuation_activate", &operator, activation.clone())
        .await
        .unwrap();
    assert_eq!(denied["denial"]["code"], "rollout_hold");
    let replay = service
        .call("runs.continuation_activate", &operator, activation)
        .await
        .unwrap();
    assert_eq!(replay["replay_of"], "denied");
    assert_eq!(replay["journal_id"], denied["journal_id"]);
    assert_eq!(
        c::find(&pool, &op.operation_id)
            .await
            .unwrap()
            .unwrap()
            .version,
        prepared.version
    );
    let reconciled = service
        .call(
            "runs.continuation_reconcile",
            &operator,
            json!({"operation_id":op.operation_id,"expected_version":prepared.version,
            "caller_request_id":uuid::Uuid::new_v4(),"reason":"Inspect missing prepared evidence"}),
        )
        .await
        .unwrap();
    assert_eq!(reconciled["status"], "accepted");
    assert_eq!(reconciled["operation"]["phase"], "needs_reconciliation");
    assert!(c::check_source_guard(&pool, &source.to_string())
        .await
        .is_err());
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM runs")
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
}

#[tokio::test]
async fn disabled_service_can_abort_queued_work_and_replay_after_fence_release() {
    use db::repos::run_continuations as c;
    let (pool, source, operator, dir) = fixture().await;
    let op = c::reserve(
        &pool,
        &c::Reservation {
            source_run_id: source.to_string(),
            caller_fingerprint: "fixture".into(),
            caller_request_id: uuid::Uuid::new_v4().to_string(),
            intent_sha256: "a".repeat(64),
            plan_ref: "plan".into(),
            plan_sha256: "b".repeat(64),
            target_ref: "target".into(),
            source_witness_sha256: "c".repeat(64),
            reason: "fixture".into(),
        },
    )
    .await
    .unwrap();
    let service = CarryForwardService::new(
        pool.clone(),
        ServiceConfig {
            enabled: false,
            owned_parent: dir.path().into(),
        },
    );
    let request = json!({"operation_id":op.operation_id,"expected_version":op.version,"caller_request_id":uuid::Uuid::new_v4(),"reason":"Stop queued preparation"});
    let accepted = service
        .call("runs.continuation_abort", &operator, request.clone())
        .await
        .unwrap();
    assert_eq!(accepted["status"], "accepted");
    assert_eq!(accepted["operation"]["phase"], "aborted");
    let replay = service
        .call("runs.continuation_abort", &operator, request.clone())
        .await
        .unwrap();
    assert_eq!(replay["status"], "replayed");
    assert_eq!(replay["operation"], accepted["operation"]);
    assert_eq!(replay["journal_id"], accepted["journal_id"]);
    let mut stale = request;
    stale["caller_request_id"] = json!(uuid::Uuid::new_v4());
    let denial = service
        .call("runs.continuation_abort", &operator, stale)
        .await
        .unwrap();
    assert_eq!(denial["denial"]["code"], "stale_operation_version");
    c::check_source_guard(&pool, &source.to_string())
        .await
        .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT status FROM runs WHERE id=?")
            .bind(source.to_string())
            .fetch_one(&pool)
            .await
            .unwrap(),
        "blocked"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM work_items WHERE status IN ('pending','running')"
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        0
    );
}

#[tokio::test]
async fn scoped_identity_cannot_read_foreign_operations_or_mutate_even_with_exact_capability() {
    let (pool, source, operator, dir) = fixture().await;
    let service = CarryForwardService::new(
        pool.clone(),
        ServiceConfig {
            enabled: false,
            owned_parent: dir.path().into(),
        },
    );
    for scope in [
        vec![],
        vec![source.to_string()],
        vec![uuid::Uuid::new_v4().to_string()],
    ] {
        let mut scoped = operator.clone();
        scoped.run_scope = Some(scope);
        let error = service
            .call("runs.continue_blocked", &scoped, request(source))
            .await
            .unwrap_err();
        assert_eq!(
            error.downcast_ref::<ServiceError>(),
            Some(&ServiceError::Unauthorized)
        );
        let error = service
            .call(
                "runs.continuation_get",
                &scoped,
                json!({"operation_id":uuid::Uuid::new_v4()}),
            )
            .await
            .unwrap_err();
        assert_eq!(
            error.downcast_ref::<ServiceError>(),
            Some(&ServiceError::Unauthorized)
        );
    }
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM run_continuation_commands")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
}
