use db::repos::run_continuations::Continuation;
use domain::run_carry_forward_api::{ContinuationNextAction, ProjectionFreshness};

fn operation() -> Continuation {
    Continuation {
        operation_id: uuid::Uuid::new_v4().to_string(),
        source_run_id: uuid::Uuid::new_v4().to_string(),
        idea_id: uuid::Uuid::new_v4().to_string(),
        reserved_successor_run_id: uuid::Uuid::new_v4().to_string(),
        successor_run_id: None,
        caller_fingerprint: "private principal".into(),
        caller_request_id: uuid::Uuid::new_v4().to_string(),
        intent_sha256: "a".repeat(64),
        phase: "needs_reconciliation".into(),
        version: 3,
        generation: 2,
        plan_ref: "/private/secret-plan".into(),
        plan_sha256: "b".repeat(64),
        target_ref: "/private/secret-target".into(),
        source_witness_sha256: "c".repeat(64),
        manifest_ref: None,
        manifest_sha256: None,
        journal_id: uuid::Uuid::new_v4().to_string(),
        holds_json: "[\"effect_outcome_unknown\"]".into(),
        worker_claimed: true,
        created_at: "2026-09-20T00:00:00Z".into(),
        updated_at: "2026-09-20T00:00:00Z".into(),
        deadline_at: "2026-09-20T00:15:00Z".into(),
    }
}

#[test]
fn compact_readback_retains_holds_without_private_paths_or_principals() {
    let value = engine::run_carry_forward::readback::compact(&operation(), false).unwrap();
    let json = serde_json::to_string(&value).unwrap();
    assert!(!json.contains("/private") && !json.contains("private principal"));
    assert!(json.contains("effect_outcome_unknown"));
    assert!(!value.admission.enabled);
    assert!(value.admission.fences_enforced);
    assert_eq!(value.projection_freshness, ProjectionFreshness::Fresh);
    assert!(value
        .next_actions
        .as_slice()
        .contains(&ContinuationNextAction::ExplicitReconcile));
    let decoded: domain::run_carry_forward_api::RunContinuationCompactReadbackV1 =
        serde_json::from_str(&json).unwrap();
    assert_eq!(value, decoded);
}

#[test]
fn unknown_phase_is_readable_but_never_actionable() {
    let mut op = operation();
    op.phase = "future_hold".into();
    let value = engine::run_carry_forward::readback::compact(&op, false).unwrap();
    assert_eq!(value.phase.as_str(), "future_hold");
    assert!(value.actionable_next_actions().is_empty());
}

#[test]
fn p039_readback_configuration_distinguishes_rollout_from_service_attachment() {
    use engine::run_carry_forward::readback::{compact_with_config, ReadbackConfig};
    let attached = compact_with_config(
        &operation(),
        ReadbackConfig {
            enabled: false,
            reconcile_available: true,
        },
    )
    .unwrap();
    assert!(!attached.admission.enabled);
    assert!(attached.admission.reconcile_available);
    let detached = compact_with_config(&operation(), ReadbackConfig::default()).unwrap();
    assert!(!detached.admission.reconcile_available);
}

#[test]
fn p039_public_links_keep_both_directions_and_only_activated_ids() {
    use domain::run_carry_forward_api::{LinksSchema, RunCarryForwardLinksV1};
    use engine::run_carry_forward::readback::{compact, public_fields};
    let mut incoming = operation();
    incoming.phase = "activated".into();
    incoming.successor_run_id = Some(incoming.reserved_successor_run_id.clone());
    incoming.manifest_sha256 = Some("d".repeat(64));
    let mut outgoing = operation();
    outgoing.source_run_id = incoming.successor_run_id.clone().unwrap();
    let links = RunCarryForwardLinksV1 {
        schema_version: LinksSchema::V1,
        incoming: Some(compact(&incoming, false).unwrap()),
        outgoing: Some(compact(&outgoing, false).unwrap()),
    };
    let value = public_fields(&links).unwrap();
    assert_eq!(value["continued_from_run_id"], incoming.source_run_id);
    assert!(value["continued_as_run_id"].is_null());
    assert_eq!(
        value["carry_forward_readback"]["outgoing"]["phase"],
        "needs_reconciliation"
    );
    assert_eq!(
        value["carry_forward_readback"]["incoming"]["manifest_sha256"],
        format!("sha256:{}", "d".repeat(64))
    );
    let encoded = value.to_string();
    for forbidden in [
        "manifest_page",
        "plan_ref",
        "target_ref",
        "caller_fingerprint",
        "/private",
    ] {
        assert!(!encoded.contains(forbidden));
    }
    let empty = RunCarryForwardLinksV1 {
        schema_version: LinksSchema::V1,
        incoming: None,
        outgoing: None,
    };
    assert_eq!(
        public_fields(&empty).unwrap(),
        serde_json::json!({"continued_from_run_id":null,"continued_as_run_id":null,"carry_forward_readback":null})
    );
}

#[tokio::test]
async fn p039_receipt_retains_canonical_links_but_requires_actual_release_result() {
    use engine::release::{coordinator::ReleaseResult, receipt::DeliveryReceiptBuilder};
    use engine::run_carry_forward::readback::{report_fields, ReadbackConfig};
    let pool = db::pool::create_pool("sqlite::memory:").await.unwrap();
    let id = uuid::Uuid::new_v4().to_string();
    let successor = uuid::Uuid::new_v4().to_string();
    let op = uuid::Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO ideas(id,title,body,created_at) VALUES (?,'P039','Body','2026-09-20T00:00:00Z')").bind(&id).execute(&pool).await.unwrap();
    for run in [&id, &successor] {
        sqlx::query("INSERT INTO runs(id,idea_id,status,workflow_id,workflow_title,workspace_root,artifact_root,worktree_root,started_at) VALUES (?,?,'completed','wf','Workflow','/private/ws','/private/meta','/private/tree','2026-09-20T00:00:00Z')").bind(run).bind(&id).execute(&pool).await.unwrap();
    }
    sqlx::query("INSERT INTO command_journal(id,command_type,payload_json,created_at) VALUES (?,'ContinueBlocked','{}','2026-09-20T00:00:00Z')").bind(&op).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO run_continuations(operation_id,source_run_id,idea_id,reserved_successor_run_id,successor_run_id,caller_fingerprint,caller_request_id,intent_sha256,profile,phase,plan_ref,plan_sha256,target_ref,source_witness_sha256,manifest_ref,manifest_sha256,journal_id,created_at,updated_at,deadline_at) VALUES (?,?,?,?,?,'private caller',?,?,'implementation_restart_v1','activated','/private/plan',?,'/private/target',?,'/private/missing-manifest',?,?,'2026-09-20T00:00:00Z','2026-09-20T00:00:00Z','2026-09-21T00:00:00Z')")
        .bind(&op).bind(&id).bind(&id).bind(&successor).bind(&successor).bind(&op).bind("a".repeat(64)).bind("b".repeat(64)).bind("c".repeat(64)).bind("d".repeat(64)).bind(&op).execute(&pool).await.unwrap();
    let run = db::repos::runs::find_by_id(&pool, successor.parse().unwrap())
        .await
        .unwrap()
        .unwrap();
    let fields = report_fields(
        &pool,
        run.id,
        ReadbackConfig {
            enabled: false,
            reconcile_available: true,
        },
    )
    .await
    .unwrap();
    let config = domain::run::DeliveryConfiguration {
        repo_identifier: "fixture".into(),
        repo_root: "/private/ws".into(),
        base_branch: "main".into(),
        worktree_base_path: "/private/tree".into(),
        target_branch: "release/fixture".into(),
        release_target_id: None,
        release_mode: None,
    };
    assert!(DeliveryReceiptBuilder::build_receipt_with_carry_forward(
        &run,
        &config,
        None,
        None,
        None,
        "Idea",
        None,
        fields.clone()
    )
    .is_none());
    // A failed result is still a real outcome, never represented as success.
    let failed = ReleaseResult {
        git_manifest: None,
        git_receipt: None,
        bundle_manifest: None,
        upload_receipt: None,
        succeeded: false,
        failure_stage: Some("git".into()),
        failure_reason: Some("fixture failure".into()),
    };
    let receipt = DeliveryReceiptBuilder::build_receipt_with_carry_forward(
        &run,
        &config,
        Some(&failed),
        None,
        None,
        "Idea",
        None,
        fields,
    )
    .unwrap();
    let value = serde_json::to_value(receipt).unwrap();
    assert_eq!(value["continued_from_run_id"], id);
    assert_eq!(
        value["carry_forward_readback"]["incoming"]["manifest_sha256"],
        format!("sha256:{}", "d".repeat(64))
    );
    assert_eq!(
        value["carry_forward_readback"]["incoming"]["admission"]["reconcile_available"],
        true
    );
    assert_eq!(value["release_result"]["succeeded"], false);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM artifacts")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
}

#[tokio::test]
async fn p039_compact_scope_check_precedes_lookup_for_every_scoped_class() {
    use engine::run_carry_forward::readback::{authorized_links, ReadbackConfig};
    let pool = db::pool::create_pool("sqlite::memory:").await.unwrap();
    pool.close().await;
    for class in [
        auth::PrincipalClass::Operator,
        auth::PrincipalClass::ReadOnlyOperator,
        auth::PrincipalClass::Observer,
        auth::PrincipalClass::Agent,
    ] {
        let mut principal = auth::Principal::new("scoped", class);
        principal.run_scope = Some(Vec::new());
        let error = authorized_links(
            &pool,
            domain::ids::RunId::new(),
            &principal,
            ReadbackConfig::default(),
        )
        .await
        .unwrap_err();
        assert_eq!(error.to_string(), "continuation access denied");
    }
}
