use db::repos::{run_continuation_witness as witness, run_continuations as c};
use domain::ids::RunId;

async fn admission_fixture() -> (sqlx::SqlitePool, c::Reservation, String) {
    let pool = db::pool::create_pool("sqlite::memory:").await.unwrap();
    db::writer::register_shared_writer(
        &pool,
        std::sync::Arc::new(db::writer::DbWriter::new(pool.clone())),
    )
    .await
    .unwrap();
    let source = RunId::new().to_string();
    let other = RunId::new().to_string();
    for run in [&source, &other] {
        sqlx::query("INSERT INTO ideas(id,title,body,created_at) VALUES (?,'I','B','now')")
            .bind(run)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO runs(id,idea_id,status,workflow_id,workflow_title,workspace_root,artifact_root,started_at) VALUES (?,?,'blocked','w','W',?,'/meta','now')")
            .bind(run).bind(run).bind(format!("/repo/{run}")).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO stage_executions(id,run_id,stage_id,label,status,started_at) VALUES (?,?,'s','S','completed','now')")
            .bind(format!("{run}-stage")).bind(run).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO agent_executions(id,stage_execution_id,owner_id,agent_id,provider,status,started_at) VALUES (?,?,?,'a','fixture','completed','now')")
            .bind(format!("{run}-agent")).bind(format!("{run}-stage")).bind(format!("{run}-stage")).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO lead_conflict_mediations(id,run_id,conflict_id,conflict_fingerprint,lead_agent_id,status,created_at,updated_at) VALUES (?,?,'conflict','fingerprint','lead','settled','now','now')")
            .bind(format!("{run}-mediation")).bind(run).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO agent_work_continuations(id,run_id,stage_execution_id,agent_execution_id,mode,trigger_kind,status,idempotency_scope,idempotency_key,request_fingerprint_sha256,created_at,updated_at) VALUES (?,?,?,?,'live_handle_continuation','operator_mcp','succeeded',?,'key',?,'now','now')")
            .bind(format!("{run}-continuation")).bind(run).bind(format!("{run}-stage")).bind(format!("{run}-agent")).bind(run).bind("a".repeat(64)).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO session_lineages(id,run_id,agent_id,lineage_id,session_reuse_scope,created_at,closed_at) VALUES (?,?,'a',?,'run','now','now')")
            .bind(format!("{run}-lineage")).bind(run).bind(run).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO session_generations(id,lineage_id,generation,invocation_owner_key,binding_fingerprint,working_directory,workspace_mode,runtime_provider,runtime_model,status,created_at,ended_at) VALUES (?,?,1,'owner','binding',?,'worktree','fixture','model','closed','now','now')")
            .bind(format!("{run}-generation")).bind(format!("{run}-lineage")).bind(format!("/repo/{run}")).execute(&pool).await.unwrap();
        sqlx::query("UPDATE agent_executions SET session_generation_id=? WHERE id=?")
            .bind(format!("{run}-generation"))
            .bind(format!("{run}-agent"))
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO provider_sessions(provider_session_id,run_id,agent_execution_id,provider,lifecycle_state,process_fate,process_fate_updated_at,created_at,updated_at) VALUES (?,?,?,'fixture','self_exit_observed','absent_verified','now','now','now')")
            .bind(format!("{run}-provider")).bind(run).bind(format!("{run}-agent")).execute(&pool).await.unwrap();
    }
    let request = c::Reservation {
        source_run_id: source,
        caller_fingerprint: "operator".into(),
        caller_request_id: uuid::Uuid::new_v4().to_string(),
        intent_sha256: "a".repeat(64),
        plan_ref: "plan".into(),
        plan_sha256: "b".repeat(64),
        target_ref: "target".into(),
        source_witness_sha256: "c".repeat(64),
        reason: "admission".into(),
    };
    (pool, request, other)
}

#[tokio::test]
async fn reservation_matches_indirect_and_resource_owners_before_fencing() {
    for case in [
        "generation-agent",
        "generation-provider",
        "effect",
        "effect-attempt",
        "repair",
        "repair-lease",
        "helper",
    ] {
        for source_owned in [true, false] {
            let (pool, request, other) = admission_fixture().await;
            let owner = if source_owned {
                &request.source_run_id
            } else {
                &other
            };
            if case.starts_with("generation-") {
                sqlx::query("UPDATE session_generations SET working_directory=? WHERE id=?")
                    .bind(format!("/repo/{owner}"))
                    .bind(format!("{other}-generation"))
                    .execute(&pool)
                    .await
                    .unwrap();
                if case == "generation-agent" {
                    sqlx::query("UPDATE agent_executions SET status='running' WHERE id=?")
                        .bind(format!("{other}-agent"))
                        .execute(&pool)
                        .await
                        .unwrap();
                } else {
                    sqlx::query("UPDATE provider_sessions SET process_fate='identity_ambiguous' WHERE provider_session_id=?")
                        .bind(format!("{other}-provider")).execute(&pool).await.unwrap();
                }
            } else if case.starts_with("effect") {
                sqlx::query("INSERT INTO side_effects(id,run_id,stage_execution_id,agent_execution_id,effect_kind,target_key,idempotency_key,request_fingerprint,status,created_at,updated_at) VALUES ('effect',?,?,?,'git_commit','target','key','fingerprint',?,'now','now')")
                    .bind(&other).bind(format!("{other}-stage")).bind(format!("{owner}-agent"))
                    .bind(if case == "effect" { "needs_reconciliation" } else { "settled" }).execute(&pool).await.unwrap();
                if case == "effect-attempt" {
                    sqlx::query("INSERT INTO side_effect_attempts(id,side_effect_id,attempt_number,owner_instance_id,started_at) VALUES ('attempt','effect',1,'owner','now')").execute(&pool).await.unwrap();
                }
            } else if case.starts_with("repair") {
                sqlx::query("INSERT INTO output_contract_repair_events(repair_attempt_id,run_id,stage_execution_id,agent_execution_id,session_generation_id,role,provider_family,adapter_family,required_output_mode,initial_failure_class,status,recorded_at,created_at,updated_at) VALUES ('repair',?,?,?,?,'code_writer','fixture','fixture','required','missing_required_outputs',?,'now','now','now')")
                    .bind(&other).bind(format!("{other}-stage")).bind(format!("{owner}-agent")).bind(format!("{other}-generation"))
                    .bind(if case == "repair" { "in_progress" } else { "recovered" }).execute(&pool).await.unwrap();
                if case == "repair-lease" {
                    sqlx::query("INSERT INTO output_contract_repair_leases(lease_key,repair_event_id,run_id,stage_execution_id,parent_agent_execution_id,lease_kind,idempotency_token,lease_owner_principal_id,lease_acquired_at,lease_expires_at,lease_seconds,created_at,updated_at) VALUES ('lease','repair',?,?,?,'repair','token','operator','now','later',60,'now','now')")
                        .bind(&other).bind(format!("{other}-stage")).bind(format!("{other}-agent")).execute(&pool).await.unwrap();
                }
            } else {
                sqlx::query("INSERT INTO p080_helper_leases_v1(id,run_id,stage_id,work_item_id,helper_kind,process_group_id,owner_process_id,process_start_boottime_nanos,acquired_at,created_at,updated_at,agent_execution_id) VALUES ('helper',?,'s','work','fixture',42,42,1,'now','now','now',?)")
                    .bind(&other).bind(format!("{owner}-agent")).execute(&pool).await.unwrap();
            }
            let outcome = c::reserve(&pool, &request).await;
            if source_owned {
                let error = outcome.expect_err(&format!("source owner missed: {case}"));
                assert!(error.to_string().contains("source_busy"), "{case}: {error}");
                assert_eq!(sqlx::query_scalar::<_,i64>("SELECT (SELECT count(*) FROM run_continuations)+(SELECT count(*) FROM run_execution_fences)+(SELECT count(*) FROM command_journal)+(SELECT count(*) FROM work_items)").fetch_one(&pool).await.unwrap(), 0);
            } else {
                outcome.unwrap_or_else(|error| panic!("unrelated owner blocked: {case}: {error}"));
            }
            pool.close().await;
        }
    }
}

async fn queue_owner(
    pool: &sqlx::SqlitePool,
    run: Option<&str>,
    payload: serde_json::Value,
    status: &str,
) {
    sqlx::query("INSERT INTO work_items(id,kind,payload_json,run_id,status,created_at,scheduled_at) VALUES ('queued-owner','invoke_agent',?,?,?,'now','now')")
        .bind(payload.to_string()).bind(run).bind(status).execute(pool).await.unwrap();
}

async fn assert_busy_without_reservation(pool: &sqlx::SqlitePool, request: &c::Reservation) {
    let error = c::reserve(pool, request)
        .await
        .expect_err("unknown or source-owned work must hold");
    assert!(error.to_string().contains("source_busy"), "{error}");
    assert_eq!(sqlx::query_scalar::<_,i64>("SELECT (SELECT count(*) FROM run_continuations)+(SELECT count(*) FROM run_execution_fences)+(SELECT count(*) FROM command_journal)").fetch_one(pool).await.unwrap(), 0);
}

#[tokio::test]
async fn reservation_holds_preexisting_unknown_queue_owners_without_fencing() {
    let (pool, request, other) = admission_fixture().await;
    for key in [
        "run_id",
        "stage_execution_id",
        "agent_execution_id",
        "mediation_record_id",
        "lead_mediation_record_id",
        "continuation_id",
    ] {
        queue_owner(&pool, None, serde_json::json!({key:"missing"}), "running").await;
        assert_busy_without_reservation(&pool, &request).await;
        sqlx::query("DELETE FROM work_items")
            .execute(&pool)
            .await
            .unwrap();
    }
    for run in [None, Some("missing")] {
        queue_owner(&pool, run, serde_json::json!({}), "running").await;
        assert_busy_without_reservation(&pool, &request).await;
        sqlx::query("DELETE FROM work_items")
            .execute(&pool)
            .await
            .unwrap();
    }
    queue_owner(
        &pool,
        None,
        serde_json::json!({"run_id":"missing"}),
        "completed",
    )
    .await;
    sqlx::query("UPDATE work_items SET id='historical-unknown'")
        .execute(&pool)
        .await
        .unwrap();
    queue_owner(
        &pool,
        Some(&other),
        serde_json::json!({"run_id":other}),
        "running",
    )
    .await;
    c::reserve(&pool, &request).await.unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM work_items WHERE id IN ('historical-unknown','queued-owner')"
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        2
    );
    pool.close().await;
}

#[tokio::test]
async fn reservation_resolves_preexisting_mediation_and_continuation_queue_owners() {
    let (pool, request, other) = admission_fixture().await;
    for (key, suffix) in [
        ("mediation_record_id", "mediation"),
        ("lead_mediation_record_id", "mediation"),
        ("continuation_id", "continuation"),
    ] {
        queue_owner(
            &pool,
            Some(&other),
            serde_json::json!({key:format!("{}-{suffix}", request.source_run_id)}),
            "pending",
        )
        .await;
        assert_busy_without_reservation(&pool, &request).await;
        sqlx::query("DELETE FROM work_items")
            .execute(&pool)
            .await
            .unwrap();
    }
    queue_owner(
        &pool,
        None,
        serde_json::json!({"continuation_id":format!("{other}-continuation")}),
        "running",
    )
    .await;
    c::reserve(&pool, &request).await.unwrap();
    pool.close().await;
}

#[tokio::test]
async fn reservation_checks_source_witness_in_its_writer_transaction() {
    let pool = db::pool::create_pool("sqlite::memory:").await.unwrap();
    db::writer::register_shared_writer(
        &pool,
        std::sync::Arc::new(db::writer::DbWriter::new(pool.clone())),
    )
    .await
    .unwrap();
    let run = RunId::new();
    let idea = uuid::Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO ideas(id,title,body,created_at) VALUES (?,'Idea','Original','2026-09-20T00:00:00Z')").bind(&idea).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO runs(id,idea_id,status,workflow_id,workflow_title,workspace_root,artifact_root,started_at) VALUES (?,?,'blocked','w','W','/repo','/meta','2026-09-20T00:00:00Z')")
        .bind(run.to_string()).bind(&idea).execute(&pool).await.unwrap();
    let hash = witness::observe(&mut pool.acquire().await.unwrap(), run, None)
        .await
        .unwrap();
    let request = c::Reservation {
        source_run_id: run.to_string(),
        caller_fingerprint: "operator".into(),
        caller_request_id: uuid::Uuid::new_v4().to_string(),
        intent_sha256: "a".repeat(64),
        plan_ref: "ignored".into(),
        plan_sha256: "b".repeat(64),
        target_ref: "ignored".into(),
        source_witness_sha256: hash.as_str().strip_prefix("sha256:").unwrap().into(),
        reason: "checked".into(),
    };
    sqlx::query("UPDATE ideas SET body='Changed' WHERE id=?")
        .bind(&idea)
        .execute(&pool)
        .await
        .unwrap();
    let error = c::reserve_checked(&pool, &request, std::path::Path::new("/owned"))
        .await
        .unwrap_err();
    assert!(error.to_string().contains("source_changed"), "{error}");
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM run_continuations")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM command_journal")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    let denial_request = uuid::Uuid::new_v4().to_string();
    let denial_identity = db::repos::run_continuation_commands::CommandIdentity {
        caller_fingerprint: "operator",
        caller_request_id: &denial_request,
        command: "runs.continue_blocked",
        intent_sha256: &"f".repeat(64),
    };
    let denied = db::repos::run_continuation_commands::record_denial(
        &pool,
        &denial_identity,
        &run.to_string(),
        None,
        domain::run_carry_forward_api::ContinuationReasonCode::SourceChanged,
    )
    .await
    .unwrap();
    let replay = db::repos::run_continuation_commands::record_denial(
        &pool,
        &denial_identity,
        &run.to_string(),
        None,
        domain::run_carry_forward_api::ContinuationReasonCode::RolloutHold,
    )
    .await
    .unwrap();
    assert_eq!(denied.denial, replay.denial);
    assert_eq!(
        replay.replay_of,
        Some(domain::run_carry_forward_api::ReplayOf::Denied)
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM run_continuations")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    let hash = witness::observe(&mut pool.acquire().await.unwrap(), run, None)
        .await
        .unwrap();
    let mut request = request;
    request.source_witness_sha256 = hash.as_str()[7..].into();
    let op = c::reserve_checked(&pool, &request, std::path::Path::new("/owned"))
        .await
        .unwrap();
    assert_eq!(
        db::repos::runs::list_active(&pool).await.unwrap().len(),
        1,
        "historical source remains operator-visible"
    );
    assert!(
        db::repos::runs::list_recoverable(&pool)
            .await
            .unwrap()
            .is_empty(),
        "recovery cannot resume a fenced source"
    );
    assert_eq!(op.plan_ref, format!("/owned/{}/plan.json", op.operation_id));
    let after = witness::observe(
        &mut pool.acquire().await.unwrap(),
        run,
        Some(&op.operation_id),
    )
    .await
    .unwrap();
    assert_eq!(hash, after, "only this operation's bookkeeping is excluded");
    let claim = c::claim_preparation_for(&pool, &op.operation_id)
        .await
        .unwrap()
        .unwrap();
    c::begin_step(
        &pool,
        &op.operation_id,
        claim.generation,
        "manifest",
        "{}",
        "owned",
    )
    .await
    .unwrap();
    c::finish_step(
        &pool,
        &op.operation_id,
        claim.generation,
        "manifest",
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
    let mut successor = db::repos::runs::find_by_id(&pool, run)
        .await
        .unwrap()
        .unwrap();
    successor.id = RunId(uuid::Uuid::parse_str(&op.reserved_successor_run_id).unwrap());
    successor.status = domain::run::RunStatus::Pending;
    successor.current_state = Some("new_review".into());
    sqlx::query("UPDATE ideas SET body='Changed after preparation' WHERE id=?")
        .bind(&idea)
        .execute(&pool)
        .await
        .unwrap();
    let id = db::repos::run_continuation_commands::CommandIdentity {
        caller_fingerprint: "operator",
        caller_request_id: &uuid::Uuid::new_v4().to_string(),
        command: "runs.continuation_activate",
        intent_sha256: &"e".repeat(64),
    };
    let error = c::activate_checked_command(
        &pool,
        &op.operation_id,
        prepared.version,
        &"d".repeat(64),
        &successor,
        &[],
        &id,
    )
    .await
    .unwrap_err();
    assert!(error.to_string().contains("source_changed"), "{error}");
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM runs")
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        c::find(&pool, &op.operation_id)
            .await
            .unwrap()
            .unwrap()
            .phase,
        "prepared"
    );
}
