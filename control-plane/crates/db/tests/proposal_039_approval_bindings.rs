use chrono::Utc;
use db::repos::run_continuations as c;
use domain::{
    approval::{Approval, ApprovalDecision},
    ids::{ApprovalId, RunId, StageExecutionId},
    run::Run,
    run_carry_forward::{ApprovalEvidenceV1, ContentDigest},
};
use sqlx::SqlitePool;

async fn successor() -> (SqlitePool, Run, ApprovalEvidenceV1) {
    let pool = db::pool::create_pool("sqlite::memory:").await.unwrap();
    db::writer::register_shared_writer(
        &pool,
        std::sync::Arc::new(db::writer::DbWriter::new(pool.clone())),
    )
    .await
    .unwrap();
    let source = RunId::new();
    let idea = uuid::Uuid::new_v4();
    sqlx::query("INSERT INTO ideas(id,title,body,created_at) VALUES (?,'Idea','Body','2026-09-20T00:00:00Z')").bind(idea.to_string()).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO runs(id,idea_id,status,workflow_id,workflow_title,workspace_root,artifact_root,started_at) VALUES (?,?,'blocked','workflow','Workflow','/source','/source/meta','2026-09-20T00:00:00Z')")
        .bind(source.to_string()).bind(idea.to_string()).execute(&pool).await.unwrap();
    let op = c::reserve(
        &pool,
        &c::Reservation {
            source_run_id: source.to_string(),
            caller_fingerprint: "operator".into(),
            caller_request_id: uuid::Uuid::new_v4().to_string(),
            intent_sha256: "a".repeat(64),
            plan_ref: "plan.json".into(),
            plan_sha256: "b".repeat(64),
            target_ref: "target.json".into(),
            source_witness_sha256: "c".repeat(64),
            reason: "fixture".into(),
        },
    )
    .await
    .unwrap();
    let claim = c::claim_preparation(&pool).await.unwrap().unwrap();
    c::begin_step(
        &pool,
        &op.operation_id,
        claim.generation,
        "materialize",
        "{}",
        "owned",
    )
    .await
    .unwrap();
    c::finish_step(
        &pool,
        &op.operation_id,
        claim.generation,
        "materialize",
        &"d".repeat(64),
    )
    .await
    .unwrap();
    let prepared = c::mark_prepared(
        &pool,
        &op.operation_id,
        claim.generation,
        "manifest.json",
        &"d".repeat(64),
    )
    .await
    .unwrap();
    let run:Run=serde_json::from_value(serde_json::json!({
        "id":op.reserved_successor_run_id,"idea_id":idea,"status":"pending","workflow_id":"proposal_to_release",
        "workflow_title":"Current","workspace_root":"/repo","artifact_root":"/new/meta","chainworks_meta_root":"/new/meta","worktree_root":"/new/checkout",
        "started_at":"2026-09-20T00:00:00Z","current_state":"review","workflow_snapshot_hash":"e".repeat(64),"catalog_snapshot_hash":"f".repeat(64)
    })).unwrap();
    c::activate(
        &pool,
        &op.operation_id,
        prepared.version,
        &"d".repeat(64),
        &run,
    )
    .await
    .unwrap();
    let evidence = ApprovalEvidenceV1 {
        source_run_id: source,
        successor_run_id: run.id,
        manifest_sha256: ContentDigest::from_sha256_hex(&"d".repeat(64)).unwrap(),
        proposal_sha256: ContentDigest::of(b"proposal"),
        code_sha256: ContentDigest::of(b"code"),
        workflow_snapshot_hash: ContentDigest::from_sha256_hex(&"e".repeat(64)).unwrap(),
        catalog_snapshot_hash: ContentDigest::from_sha256_hex(&"f".repeat(64)).unwrap(),
        capability_delta_sha256: ContentDigest::of(b"delta"),
        unresolved_findings_sha256: ContentDigest::of(b"findings"),
    };
    (pool, run, evidence)
}

async fn stage(pool: &SqlitePool, run: RunId) -> StageExecutionId {
    let id = StageExecutionId::new();
    sqlx::query("INSERT INTO stage_executions(id,run_id,stage_id,label,status,started_at) VALUES (?,?,'implementation_gate','Gate','waiting_approval',?)")
        .bind(id.to_string()).bind(run.to_string()).bind(Utc::now().to_rfc3339()).execute(pool).await.unwrap();
    id
}

fn approval(run: RunId) -> Approval {
    Approval {
        id: ApprovalId::new(),
        run_id: run,
        stage_id: "implementation_gate".into(),
        decision: ApprovalDecision::Requested,
        requested_at: Utc::now(),
        decided_at: None,
        comment: None,
        expires_at: None,
    }
}

#[tokio::test]
async fn exact_binding_resolution_and_single_dispatch_consumption() {
    use db::repos::run_continuation_approvals as b;
    let (pool, run, evidence) = successor().await;
    let stage = stage(&pool, run.id).await;
    let approval = approval(run.id);
    b::create(&pool, &approval, stage, &evidence).await.unwrap();
    let mut changed = evidence.clone();
    changed.code_sha256 = ContentDigest::of(b"changed");
    let mut tx = db::writer::begin_repository_transaction(&pool, "approvals.resolve")
        .await
        .unwrap();
    assert!(
        b::validate_resolution_tx(&mut tx, approval.id, stage, &changed)
            .await
            .is_err()
    );
    tx.rollback().await.unwrap();
    let mut tx = db::writer::begin_repository_transaction(&pool, "approvals.resolve")
        .await
        .unwrap();
    b::validate_resolution_tx(&mut tx, approval.id, stage, &evidence)
        .await
        .unwrap();
    db::repos::approvals::resolve_tx(
        &mut tx,
        approval.id,
        ApprovalDecision::Granted,
        Utc::now(),
        None,
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    assert!(b::authorizes(
        &pool,
        run.id,
        "implementation_gate",
        stage,
        &evidence,
        ApprovalDecision::Granted
    )
    .await
    .unwrap());
    assert!(!b::authorizes(
        &pool,
        run.id,
        "implementation_gate",
        StageExecutionId::new(),
        &evidence,
        ApprovalDecision::Granted
    )
    .await
    .unwrap());
    b::consume(&pool, approval.id, stage, &evidence, "transition-1")
        .await
        .unwrap();
    assert!(
        b::consume(&pool, approval.id, stage, &evidence, "transition-2")
            .await
            .is_err()
    );
    b::consume(&pool, approval.id, stage, &evidence, "transition-1")
        .await
        .unwrap();
}

#[tokio::test]
async fn later_gate_supersedes_binding_without_rewriting_historical_decision() {
    use db::repos::run_continuation_approvals as b;
    let (pool, run, evidence) = successor().await;
    let old_stage = stage(&pool, run.id).await;
    let old = approval(run.id);
    b::create(&pool, &old, old_stage, &evidence).await.unwrap();
    db::repos::approvals::resolve(&pool, old.id, ApprovalDecision::Granted, Utc::now(), None)
        .await
        .unwrap();
    let new_stage = stage(&pool, run.id).await;
    let new = approval(run.id);
    b::create(&pool, &new, new_stage, &evidence).await.unwrap();
    assert_eq!(
        db::repos::approvals::find_by_id(&pool, old.id)
            .await
            .unwrap()
            .unwrap()
            .decision,
        ApprovalDecision::Granted
    );
    assert!(!b::authorizes(
        &pool,
        run.id,
        "implementation_gate",
        old_stage,
        &evidence,
        ApprovalDecision::Granted
    )
    .await
    .unwrap());
    let mut tx = db::writer::begin_repository_transaction(&pool, "approvals.resolve")
        .await
        .unwrap();
    assert!(
        b::validate_resolution_tx(&mut tx, old.id, old_stage, &evidence)
            .await
            .is_err()
    );
    tx.rollback().await.unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM run_continuation_approval_bindings WHERE status='current'"
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        1
    );
}

#[tokio::test]
async fn repair_holds_same_stage_terminal_or_nonterminal_prior_binding() {
    use db::repos::run_continuation_approvals as b;
    let (pool, run, evidence) = successor().await;
    let old_stage = stage(&pool, run.id).await;
    let old = approval(run.id);
    b::create(&pool, &old, old_stage, &evidence).await.unwrap();
    db::repos::approvals::resolve(&pool, old.id, ApprovalDecision::Rejected, Utc::now(), None)
        .await
        .unwrap();
    let fresh = approval(run.id);
    assert!(
        b::repair_missing(&pool, &fresh, old_stage, &evidence)
            .await
            .is_err(),
        "same-stage terminal binding must not be replaced"
    );
    let new_stage = stage(&pool, run.id).await;
    assert!(
        b::repair_missing(&pool, &fresh, new_stage, &evidence)
            .await
            .is_err(),
        "prior stage must be durably terminal"
    );
    sqlx::query("UPDATE stage_executions SET status='completed' WHERE id=?")
        .bind(old_stage.to_string())
        .execute(&pool)
        .await
        .unwrap();
    assert!(b::repair_missing(&pool, &fresh, new_stage, &evidence)
        .await
        .unwrap());
    assert!(
        !b::repair_missing(&pool, &approval(run.id), new_stage, &evidence)
            .await
            .unwrap()
    );
    assert_eq!(
        b::current(&pool, run.id, "implementation_gate")
            .await
            .unwrap()
            .unwrap()
            .approval_id,
        fresh.id.to_string()
    );
    assert_eq!(
        db::repos::approvals::find_by_id(&pool, old.id)
            .await
            .unwrap()
            .unwrap()
            .decision,
        ApprovalDecision::Rejected
    );
}

#[tokio::test]
async fn resolution_rechecks_frozen_policy_in_the_decision_transaction() {
    use db::repos::run_continuation_approvals as b;
    let (pool, run, evidence) = successor().await;
    let stage = stage(&pool, run.id).await;
    let approval = approval(run.id);
    b::create(&pool, &approval, stage, &evidence).await.unwrap();
    sqlx::query("UPDATE runs SET catalog_snapshot_hash=? WHERE id=?")
        .bind("9".repeat(64))
        .bind(run.id.to_string())
        .execute(&pool)
        .await
        .unwrap();
    let mut tx = db::writer::begin_repository_transaction(&pool, "approvals.resolve")
        .await
        .unwrap();
    assert!(
        b::validate_resolution_tx(&mut tx, approval.id, stage, &evidence)
            .await
            .is_err(),
        "a fresh preflight must not authorize a changed frozen tuple at commit"
    );
    tx.rollback().await.unwrap();
}

#[tokio::test]
async fn consumed_entry_without_started_execution_rejects_code_drift() {
    use db::repos::run_continuation_approvals as b;
    let (pool, run, evidence) = successor().await;
    let stage = stage(&pool, run.id).await;
    let approval = approval(run.id);
    b::create(&pool, &approval, stage, &evidence).await.unwrap();
    db::repos::approvals::resolve(
        &pool,
        approval.id,
        ApprovalDecision::Granted,
        Utc::now(),
        None,
    )
    .await
    .unwrap();
    b::consume(&pool, approval.id, stage, &evidence, "entry-1")
        .await
        .unwrap();
    assert!(
        !b::authorizes(
            &pool,
            run.id,
            "implementation_gate",
            stage,
            &evidence,
            ApprovalDecision::Granted
        )
        .await
        .unwrap(),
        "a consumed entry cannot authorize a later gate transition"
    );
    let mut changed = evidence.clone();
    changed.code_sha256 = ContentDigest::of(b"implementation after authorized entry");
    assert!(
        b::consume(&pool, approval.id, stage, &changed, "entry-1")
            .await
            .is_err(),
        "snapshot authorization is not evidence of started execution"
    );
    assert!(b::consume(&pool, approval.id, stage, &changed, "entry-2")
        .await
        .is_err());
    changed.proposal_sha256 = ContentDigest::of(b"unreviewed scope");
    assert!(b::consume(&pool, approval.id, stage, &changed, "entry-1")
        .await
        .is_err());
}

#[tokio::test]
async fn snapshot_admission_and_prompt_sent_cas_replay_keep_exact_identity() {
    use db::repos::run_continuation_approvals as b;
    let (pool, run, evidence) = successor().await;
    let gate = stage(&pool, run.id).await;
    let approval = approval(run.id);
    b::create(&pool, &approval, gate, &evidence).await.unwrap();
    db::repos::approvals::resolve(
        &pool,
        approval.id,
        ApprovalDecision::Granted,
        Utc::now(),
        None,
    )
    .await
    .unwrap();
    b::consume(&pool, approval.id, gate, &evidence, "entry")
        .await
        .unwrap();
    let preparation = StageExecutionId::new();
    let execution = domain::ids::AgentExecutionId::new();
    let snapshot = uuid::Uuid::new_v4().to_string();
    sqlx::query("UPDATE runs SET current_state='prepare' WHERE id=?")
        .bind(run.id.to_string())
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO stage_executions(id,run_id,stage_id,label,status,started_at) VALUES (?,?,'prepare','Prepare','running',?)")
        .bind(preparation.to_string()).bind(run.id.to_string()).bind(Utc::now().to_rfc3339()).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO agent_executions(id,stage_execution_id,agent_id,status,provider,started_at) VALUES (?,?,'fixture','running','fixture',?)")
        .bind(execution.to_string()).bind(preparation.to_string()).bind(Utc::now().to_rfc3339()).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO artifacts(id,run_id,stage_id,agent_id,name,contract_id,format,file_path,checksum_sha256,provider,is_pinned,created_at) VALUES (?,?,'prepare','engine','approved_proposal','approved_proposal','markdown','/fixture/snapshot',?,'engine',1,?)")
        .bind(&snapshot).bind(run.id.to_string()).bind(evidence.proposal_sha256.as_str().trim_start_matches("sha256:")).bind(Utc::now().to_rfc3339()).execute(&pool).await.unwrap();
    b::pin_snapshot(&pool, approval.id, "entry", &snapshot, "prepare")
        .await
        .unwrap();
    b::pin_snapshot(&pool, approval.id, "entry", &snapshot, "prepare")
        .await
        .unwrap();
    assert!(b::pin_snapshot(
        &pool,
        approval.id,
        "entry",
        &uuid::Uuid::new_v4().to_string(),
        "prepare"
    )
    .await
    .is_err());
    assert!(
        b::record_prompt_sent(
            &pool,
            approval.id,
            "entry",
            &snapshot,
            preparation,
            execution
        )
        .await
        .is_err(),
        "PromptSent requires exact prior admission"
    );
    sqlx::query("UPDATE artifacts SET is_pinned=0 WHERE id=?")
        .bind(&snapshot)
        .execute(&pool)
        .await
        .unwrap();
    assert!(
        b::admit_execution(
            &pool,
            approval.id,
            "entry",
            &snapshot,
            preparation,
            execution,
            &evidence
        )
        .await
        .is_err(),
        "snapshot metadata changed after preflight must fail at admission commit"
    );
    sqlx::query("UPDATE artifacts SET is_pinned=1 WHERE id=?")
        .bind(&snapshot)
        .execute(&pool)
        .await
        .unwrap();
    b::admit_execution(
        &pool,
        approval.id,
        "entry",
        &snapshot,
        preparation,
        execution,
        &evidence,
    )
    .await
    .unwrap();
    b::admit_execution(
        &pool,
        approval.id,
        "entry",
        &snapshot,
        preparation,
        execution,
        &evidence,
    )
    .await
    .unwrap();
    let admitted = b::current(&pool, run.id, "implementation_gate")
        .await
        .unwrap()
        .unwrap();
    assert!(admitted.first_execution_started_at.is_none());
    assert!(b::record_prompt_sent(
        &pool,
        approval.id,
        "wrong-entry",
        &snapshot,
        preparation,
        execution
    )
    .await
    .is_err());
    assert!(b::record_prompt_sent(
        &pool,
        approval.id,
        "entry",
        &snapshot,
        preparation,
        domain::ids::AgentExecutionId::new()
    )
    .await
    .is_err());
    b::record_prompt_sent(
        &pool,
        approval.id,
        "entry",
        &snapshot,
        preparation,
        execution,
    )
    .await
    .unwrap();
    let started = b::current(&pool, run.id, "implementation_gate")
        .await
        .unwrap()
        .unwrap();
    b::record_prompt_sent(
        &pool,
        approval.id,
        "entry",
        &snapshot,
        preparation,
        execution,
    )
    .await
    .unwrap();
    assert_eq!(
        started.first_execution_started_at,
        b::current(&pool, run.id, "implementation_gate")
            .await
            .unwrap()
            .unwrap()
            .first_execution_started_at
    );
    assert!(b::record_prompt_sent(
        &pool,
        approval.id,
        "entry",
        &snapshot,
        preparation,
        domain::ids::AgentExecutionId::new()
    )
    .await
    .is_err());
    let mut changed = evidence.clone();
    changed.code_sha256 = ContentDigest::of(b"legitimate edits after PromptSent");
    assert!(
        b::record_prompt_sent(
            &pool,
            approval.id,
            "entry",
            &snapshot,
            StageExecutionId::new(),
            execution
        )
        .await
        .is_err(),
        "replay must retain the exact admitted stage owner"
    );
    b::consume(&pool, approval.id, gate, &changed, "entry")
        .await
        .unwrap();
    changed.proposal_sha256 = ContentDigest::of(b"unreviewed proposal");
    assert!(b::consume(&pool, approval.id, gate, &changed, "entry")
        .await
        .is_err());
}

#[tokio::test]
async fn binding_insert_failure_rolls_back_approval_and_supersession() {
    use db::repos::run_continuation_approvals as b;
    let (pool, run, evidence) = successor().await;
    let first_stage = stage(&pool, run.id).await;
    let first = approval(run.id);
    b::create(&pool, &first, first_stage, &evidence)
        .await
        .unwrap();
    let next_stage = stage(&pool, run.id).await;
    let next = approval(run.id);
    sqlx::query("CREATE TRIGGER fail_binding_insert BEFORE INSERT ON run_continuation_approval_bindings BEGIN SELECT RAISE(ABORT,'injected binding write failure'); END")
        .execute(&pool).await.unwrap();
    assert!(b::create(&pool, &next, next_stage, &evidence)
        .await
        .is_err());
    assert!(db::repos::approvals::find_by_id(&pool, next.id)
        .await
        .unwrap()
        .is_none());
    assert_eq!(
        b::current(&pool, run.id, "implementation_gate")
            .await
            .unwrap()
            .unwrap()
            .approval_id,
        first.id.to_string()
    );
}

#[tokio::test]
async fn resolution_rejects_every_changed_evidence_dimension() {
    use db::repos::run_continuation_approvals as b;
    let (pool, run, evidence) = successor().await;
    let stage = stage(&pool, run.id).await;
    let approval = approval(run.id);
    b::create(&pool, &approval, stage, &evidence).await.unwrap();
    for dimension in 0..9 {
        let mut changed = evidence.clone();
        let digest = ContentDigest::of(b"different evidence");
        match dimension {
            0 => changed.source_run_id = RunId::new(),
            1 => changed.successor_run_id = RunId::new(),
            2 => changed.manifest_sha256 = digest,
            3 => changed.proposal_sha256 = digest,
            4 => changed.code_sha256 = digest,
            5 => changed.workflow_snapshot_hash = digest,
            6 => changed.catalog_snapshot_hash = digest,
            7 => changed.capability_delta_sha256 = digest,
            _ => changed.unresolved_findings_sha256 = digest,
        }
        let mut tx = db::writer::begin_repository_transaction(&pool, "approvals.resolve")
            .await
            .unwrap();
        assert!(
            b::validate_resolution_tx(&mut tx, approval.id, stage, &changed)
                .await
                .is_err(),
            "dimension {dimension}"
        );
        tx.rollback().await.unwrap();
    }
    assert_eq!(
        db::repos::approvals::find_by_id(&pool, approval.id)
            .await
            .unwrap()
            .unwrap()
            .decision,
        ApprovalDecision::Requested
    );
}

#[tokio::test]
async fn newer_waiting_execution_invalidates_old_preflight_before_binding_creation() {
    use db::repos::run_continuation_approvals as b;
    let (pool, run, evidence) = successor().await;
    let first_stage = stage(&pool, run.id).await;
    let approval = approval(run.id);
    b::create(&pool, &approval, first_stage, &evidence)
        .await
        .unwrap();
    let _next_stage = stage(&pool, run.id).await;
    let mut tx = db::writer::begin_repository_transaction(&pool, "approvals.resolve")
        .await
        .unwrap();
    assert!(
        b::validate_resolution_tx(&mut tx, approval.id, first_stage, &evidence)
            .await
            .is_err(),
        "latest exact gate execution must be rechecked inside the decision transaction"
    );
    tx.rollback().await.unwrap();
}

#[tokio::test]
async fn newer_gate_prevents_old_binding_creation() {
    let (pool, run, evidence) = successor().await;
    let first_stage = stage(&pool, run.id).await;
    let _next_stage = stage(&pool, run.id).await;
    let old = approval(run.id);
    assert!(
        db::repos::run_continuation_approvals::create(&pool, &old, first_stage, &evidence)
            .await
            .is_err(),
        "a delayed creator must not bind an older gate execution"
    );
    assert!(db::repos::approvals::find_by_id(&pool, old.id)
        .await
        .unwrap()
        .is_none());
}

#[tokio::test]
async fn newer_gate_prevents_old_entry_consumption() {
    use db::repos::run_continuation_approvals as b;
    let (pool, run, evidence) = successor().await;
    let first_stage = stage(&pool, run.id).await;
    let old = approval(run.id);
    b::create(&pool, &old, first_stage, &evidence)
        .await
        .unwrap();
    db::repos::approvals::resolve(&pool, old.id, ApprovalDecision::Granted, Utc::now(), None)
        .await
        .unwrap();
    let _next_stage = stage(&pool, run.id).await;
    assert!(
        b::consume(&pool, old.id, first_stage, &evidence, "old-entry")
            .await
            .is_err(),
        "a newer gate must invalidate an older dispatch preflight"
    );
    assert!(b::current(&pool, run.id, "implementation_gate")
        .await
        .unwrap()
        .unwrap()
        .consumed_transition_id
        .is_none());
}

#[tokio::test]
async fn concurrent_missing_request_repairs_publish_one_pending_generation() {
    use db::repos::run_continuation_approvals as b;
    let (pool, run, evidence) = successor().await;
    let stage = stage(&pool, run.id).await;
    let first = approval(run.id);
    let second = approval(run.id);
    let (left, right) = tokio::join!(
        b::repair_missing(&pool, &first, stage, &evidence),
        b::repair_missing(&pool, &second, stage, &evidence),
    );
    assert_ne!(left.unwrap(), right.unwrap());
    let approvals = db::repos::approvals::list_by_run(&pool, run.id)
        .await
        .unwrap();
    assert_eq!(approvals.len(), 1);
    assert_eq!(approvals[0].decision, ApprovalDecision::Requested);
    let binding = b::current(&pool, run.id, "implementation_gate")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(binding.generation, 1);
    assert_eq!(binding.approval_id, approvals[0].id.to_string());
}

#[tokio::test]
async fn missing_binding_with_existing_request_holds_without_replacement() {
    use db::repos::run_continuation_approvals as b;
    let (pool, run, evidence) = successor().await;
    let stage = stage(&pool, run.id).await;
    let orphan = approval(run.id);
    db::repos::approvals::insert(&pool, &orphan).await.unwrap();
    let replacement = approval(run.id);
    assert!(b::repair_missing(&pool, &replacement, stage, &evidence)
        .await
        .is_err());
    assert!(b::current(&pool, run.id, "implementation_gate")
        .await
        .unwrap()
        .is_none());
    let approvals = db::repos::approvals::list_by_run(&pool, run.id)
        .await
        .unwrap();
    assert_eq!(approvals.len(), 1);
    assert_eq!(approvals[0].id, orphan.id);
    assert_eq!(approvals[0].decision, ApprovalDecision::Requested);
}
