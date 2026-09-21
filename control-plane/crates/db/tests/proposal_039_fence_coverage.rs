use db::repos::run_continuations::{self as c, Reservation};
use sqlx::{AssertSqlSafe, SqlitePool};

async fn execute(pool: &SqlitePool, sql: &str) {
    // Only literal fixtures and fixed identifier substitutions from this file.
    sqlx::raw_sql(AssertSqlSafe(sql))
        .execute(pool)
        .await
        .unwrap_or_else(|e| panic!("{sql}: {e}"));
}

async fn pool() -> SqlitePool {
    let pool = db::pool::create_pool("sqlite::memory:").await.unwrap();
    db::writer::register_shared_writer(
        &pool,
        std::sync::Arc::new(db::writer::DbWriter::new(pool.clone())),
    )
    .await
    .unwrap();
    for id in ["source", "other"] {
        sqlx::query("INSERT INTO ideas(id,title,body,created_at) VALUES (?, 'Idea','Body','now')")
            .bind(format!("idea-{id}"))
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO runs(id,idea_id,status,workflow_id,workflow_title,workspace_root,worktree_root,artifact_root,started_at) VALUES (?,?,'blocked','workflow','Workflow',?,?, '/meta','now')")
            .bind(id).bind(format!("idea-{id}")).bind(format!("/repo/{id}")).bind(format!("/repo/{id}")).execute(&pool).await.unwrap();
        let seed = "INSERT INTO stage_executions(id,run_id,stage_id,label,status,started_at) VALUES ('stage','source','s','S','completed','now');
            INSERT INTO agent_executions(id,stage_execution_id,owner_id,agent_id,provider,status,started_at) VALUES ('agent','stage','stage','a','codex','completed','now');
            INSERT INTO lead_conflict_mediations(id,run_id,conflict_id,conflict_fingerprint,lead_agent_id,status,created_at,updated_at) VALUES ('mediation','source','conflict','fingerprint','lead','settled','now','now');
            INSERT INTO session_lineages(id,run_id,agent_id,lineage_id,session_reuse_scope,created_at,closed_at) VALUES ('lineage','source','a','lineage-key','run','now','now');
            INSERT INTO session_generations(id,lineage_id,generation,invocation_owner_key,binding_fingerprint,working_directory,workspace_mode,runtime_provider,runtime_model,status,created_at,ended_at) VALUES ('generation','lineage',1,'owner','binding','/repo','worktree','codex','model','closed','now','now');
            INSERT INTO provider_sessions(provider_session_id,run_id,provider,lifecycle_state,process_fate,process_fate_updated_at,created_at,updated_at) VALUES ('provider','source','codex','self_exit_observed','absent_verified','now','now','now');";
        execute(
            &pool,
            &if id == "source" {
                seed.into()
            } else {
                other(seed)
            },
        )
        .await;
    }
    pool
}

fn other(sql: &str) -> String {
    [
        "source",
        "stage",
        "agent",
        "mediation",
        "lineage",
        "generation",
        "provider",
        "row",
    ]
    .iter()
    .fold(sql.into(), |s: String, id| {
        s.replace(
            &format!("'{id}'"),
            &format!("'other-{}'", if *id == "source" { "" } else { id }),
        )
    })
    .replace("'other-'", "'other'")
}

fn request() -> Reservation {
    Reservation {
        source_run_id: "source".into(),
        caller_fingerprint: "operator".into(),
        caller_request_id: uuid::Uuid::new_v4().to_string(),
        intent_sha256: "a".repeat(64),
        plan_ref: "plan.json".into(),
        plan_sha256: "b".repeat(64),
        target_ref: "target.json".into(),
        source_witness_sha256: "c".repeat(64),
        reason: "policy".into(),
    }
}

async fn idea_guard_fixture() -> (SqlitePool, domain::run::Run) {
    let pool = db::pool::create_pool("sqlite::memory:").await.unwrap();
    db::writer::register_shared_writer(
        &pool,
        std::sync::Arc::new(db::writer::DbWriter::new(pool.clone())),
    )
    .await
    .unwrap();
    let idea = domain::ids::IdeaId::new();
    sqlx::query("INSERT INTO ideas(id,title,body,created_at) VALUES (?,'Idea','Body','2026-09-20T00:00:00Z')")
        .bind(idea.to_string()).execute(&pool).await.unwrap();
    let run = serde_json::from_value(serde_json::json!({
        "id": domain::ids::RunId::new(), "idea_id": idea, "status": "blocked",
        "workflow_id": "workflow", "workflow_title": "Workflow",
        "workspace_root": "/source", "artifact_root": "/source/meta",
        "started_at": "2026-09-20T00:00:00Z"
    }))
    .unwrap();
    db::repos::runs::insert(&pool, &run).await.unwrap();
    (pool, run)
}

async fn idea_guard_request(pool: &SqlitePool, source: domain::ids::RunId) -> Reservation {
    let hash = db::repos::run_continuation_witness::observe(
        &mut pool.acquire().await.unwrap(),
        source,
        None,
    )
    .await
    .unwrap();
    Reservation {
        source_run_id: source.to_string(),
        source_witness_sha256: hash.as_str().strip_prefix("sha256:").unwrap().into(),
        ..request()
    }
}

#[tokio::test]
async fn idea_guard_preserves_ordinary_legacy_two_running_runs() {
    use db::repos::runs;
    use domain::{ids::RunId, run::RunStatus};
    let (pool, mut run) = idea_guard_fixture().await;
    runs::update_status(&pool, run.id, RunStatus::Running)
        .await
        .unwrap();
    let first = run.id;
    run.id = RunId::new();
    run.status = RunStatus::Running;
    runs::insert(&pool, &run)
        .await
        .expect("P039 must not impose new insert policy on an ordinary idea");
    let rows = runs::list_by_idea(&pool, run.idea_id).await.unwrap();
    assert_eq!(rows.len(), 2);
    assert!(rows.iter().all(|r| r.status == RunStatus::Running));
    assert!(rows.iter().any(|r| r.id == first));
    assert!(rows.iter().any(|r| r.id == run.id));
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM run_continuations")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    pool.close().await;
}

#[tokio::test]
async fn idea_guard_checked_reservation_serializes_both_start_orders() {
    use db::repos::runs;
    use domain::{ids::RunId, run::RunStatus};
    // Both possible write-lock orders, without timing sleeps or a mock guard.
    for start_first in [true, false] {
        let (pool, source) = idea_guard_fixture().await;
        let request = idea_guard_request(&pool, source.id).await;
        let parent = tempfile::tempdir().unwrap();
        let mut sibling = source.clone();
        sibling.id = RunId::new();
        sibling.status = RunStatus::Pending;
        runs::check_continuation_start_guard(&pool, source.idea_id)
            .await
            .unwrap();
        if start_first {
            runs::insert(&pool, &sibling).await.unwrap();
            let error = c::reserve_checked(&pool, &request, parent.path())
                .await
                .unwrap_err();
            assert!(
                error.to_string().contains("continuation_in_progress"),
                "{error:#}"
            );
            for table in [
                "run_continuations",
                "run_execution_fences",
                "run_continuation_commands",
                "command_journal",
                "work_items",
            ] {
                let count: i64 =
                    sqlx::query_scalar(AssertSqlSafe(format!("SELECT COUNT(*) FROM {table}")))
                        .fetch_one(&pool)
                        .await
                        .unwrap();
                assert_eq!(count, 0, "denied reservation wrote {table}");
            }
            assert_eq!(
                runs::list_by_idea(&pool, source.idea_id)
                    .await
                    .unwrap()
                    .len(),
                2
            );
        } else {
            let op = c::reserve_checked(&pool, &request, parent.path())
                .await
                .unwrap();
            // The preflight passed before reservation; the write must still fail.
            let error = runs::insert(&pool, &sibling).await.unwrap_err();
            assert!(
                format!("{error:#}").contains("continuation_in_progress"),
                "{error:#}"
            );
            assert_eq!(
                c::find(&pool, &op.operation_id)
                    .await
                    .unwrap()
                    .unwrap()
                    .phase,
                "preparing"
            );
            assert_eq!(
                runs::list_by_idea(&pool, source.idea_id)
                    .await
                    .unwrap()
                    .len(),
                1
            );
        }
        assert_eq!(
            runs::find_by_id(&pool, source.id)
                .await
                .unwrap()
                .unwrap()
                .status,
            RunStatus::Blocked
        );
        pool.close().await;
    }
}

#[tokio::test]
async fn idea_guard_activated_successor_is_the_only_runnable_head() {
    use db::repos::runs;
    use domain::{ids::RunId, run::RunStatus};
    let (pool, source) = idea_guard_fixture().await;
    let request = idea_guard_request(&pool, source.id).await;
    let parent = tempfile::tempdir().unwrap();
    let op = c::reserve_checked(&pool, &request, parent.path())
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
    let mut successor = source.clone();
    successor.id = RunId(uuid::Uuid::parse_str(&op.reserved_successor_run_id).unwrap());
    successor.status = RunStatus::Pending;
    successor.current_state = Some("fresh_review".into());
    successor.worktree_root = Some("/successor/checkout".into());
    c::activate(
        &pool,
        &op.operation_id,
        prepared.version,
        &"d".repeat(64),
        &successor,
    )
    .await
    .unwrap();
    let mut sibling = successor.clone();
    sibling.id = RunId::new();
    let error = runs::insert(&pool, &sibling).await.unwrap_err();
    assert!(
        format!("{error:#}").contains("continuation_in_progress"),
        "{error:#}"
    );
    runs::update_status(&pool, successor.id, RunStatus::Completed)
        .await
        .unwrap();
    runs::insert(&pool, &sibling)
        .await
        .expect("historical source is not a runnable head");
    assert_eq!(
        runs::find_by_id(&pool, source.id)
            .await
            .unwrap()
            .unwrap()
            .status,
        RunStatus::Blocked
    );
    assert_eq!(
        runs::list_recoverable(&pool)
            .await
            .unwrap()
            .iter()
            .map(|r| r.id)
            .collect::<Vec<_>>(),
        vec![sibling.id]
    );
    pool.close().await;
}

async fn fence(pool: &SqlitePool) {
    execute(pool, "INSERT INTO command_journal(id,command_type,payload_json,created_at) VALUES ('op','ContinueBlocked','{}','now')").await;
    sqlx::query("INSERT INTO run_continuations(operation_id,source_run_id,idea_id,reserved_successor_run_id,caller_fingerprint,caller_request_id,intent_sha256,profile,plan_ref,plan_sha256,target_ref,source_witness_sha256,journal_id,created_at,updated_at,deadline_at) VALUES ('op','source','idea-source','successor','operator','request',?,'implementation_restart_v1','plan',?,'target',?,'op','now','now','later')")
        .bind("a".repeat(64)).bind("b".repeat(64)).bind("c".repeat(64)).execute(pool).await.unwrap();
    execute(
        pool,
        "INSERT INTO run_execution_fences VALUES ('source','op',1,'reserved','now')",
    )
    .await;
}

const CONTINUATION: &str = "INSERT INTO agent_work_continuations(id,run_id,stage_execution_id,agent_execution_id,mode,trigger_kind,status,idempotency_scope,idempotency_key,request_fingerprint_sha256,created_at,updated_at) VALUES ('row','source','stage','agent','live_handle_continuation','operator_mcp','succeeded','scope','key','aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa','now','now')";
const REPAIR: &str = "INSERT INTO output_contract_repair_events(repair_attempt_id,run_id,stage_execution_id,agent_execution_id,session_generation_id,role,provider_family,adapter_family,required_output_mode,initial_failure_class,status,recorded_at,created_at,updated_at) VALUES ('row','source','stage','agent','generation','code_writer','fixture','fixture','required','missing_required_outputs','recovered','now','now','now')";
const MAIN_SYNC: &str = "INSERT INTO main_sync_attempts(id,run_id,idempotency_key,trigger_reason,status,created_at) VALUES ('row','source','key','operator','succeeded','now')";
const BARRIER: &str = "INSERT INTO worktree_mutation_barriers(id,run_id,worktree_resource_key,owner_id,owner_kind,status,reason,expires_at,created_at) VALUES ('row','source','/repo/source','owner','main_sync','released','sync','later','now')";
const SIDE_EFFECT: &str = "INSERT INTO side_effects(id,run_id,stage_execution_id,effect_kind,target_key,idempotency_key,request_fingerprint,status,created_at,updated_at) VALUES ('row','source','stage','git_commit','target','key','fingerprint','settled','now','now')";

fn busy_cases() -> Vec<(&'static str, String)> {
    vec![
        ("stage", "UPDATE stage_executions SET status='running' WHERE id='stage'".into()),
        ("mediation agent", "INSERT INTO agent_executions(id,owner_kind,owner_id,agent_id,provider,status,started_at) VALUES ('live','lead_conflict_mediation','mediation','lead','codex','running','now')".into()),
        ("unknown mediation owner", "INSERT INTO agent_executions(id,owner_kind,owner_id,agent_id,provider,status,started_at) VALUES ('live','lead_conflict_mediation','missing','lead','codex','running','now')".into()),
        ("mediation", "UPDATE lead_conflict_mediations SET status='queued' WHERE id='mediation'".into()),
        ("agent continuation", format!("{CONTINUATION}; UPDATE agent_work_continuations SET status='queued'")),
        ("continuation effect", format!("{CONTINUATION}; INSERT INTO agent_external_side_effect_ledger(continuation_id,idempotency_key,side_effect_kind,sequence_number,status) VALUES ('row','effect','provider_send',1,'started')")),
        ("continuation worker", format!("{CONTINUATION}; INSERT INTO supervised_workers_continuation(continuation_id,worker_pid,daemon_generation_id,session_generation_id,registered_at) VALUES ('row',42,'daemon','generation','now')")),
        ("repair event without lease", format!("{REPAIR}; UPDATE output_contract_repair_events SET status='in_progress'")),
        ("unknown main sync", format!("{MAIN_SYNC}; UPDATE main_sync_attempts SET status='future_status'")),
        ("recovery-required main sync", format!("{MAIN_SYNC}; UPDATE main_sync_attempts SET status='recovery_required'")),
        ("shared barrier", format!("{BARRIER}; UPDATE worktree_mutation_barriers SET run_id='other',status='active'")),
        ("shared background lease", "INSERT INTO background_leases(id,resource_key,owner_id,acquired_at,expires_at,run_id,worktree_resource_key,worktree_access_mode) VALUES ('row','resource','owner','now','later','other','/repo/source','write')".into()),
        ("provider process", "UPDATE provider_sessions SET process_fate='identity_ambiguous',lifecycle_state='spawn_error_no_child' WHERE provider_session_id='provider'".into()),
        ("cancellation", "INSERT INTO provider_cancellation_intents(provider_session_id,cancellation_epoch,intent_state,reason,requested_at_monotonic_ms,requested_at_wall_clock) VALUES ('provider',1,'held','operator_cancel',1,'now')".into()),
        ("signal dispatch", "INSERT INTO shutdown_signal_side_effects(signal_effect_id,provider_session_id,shutdown_epoch,process_id,process_start_identity,signal_kind,generation,intent_state) VALUES ('row','provider',1,42,'identity','kill',1,'dispatching')".into()),
        ("unsettled side effect", format!("{SIDE_EFFECT}; UPDATE side_effects SET status='needs_reconciliation'")),
        ("shared queue resource", "INSERT INTO work_items(id,kind,payload_json,run_id,worktree_resource_key,worktree_access_mode,created_at,scheduled_at) VALUES ('row','invoke_agent','{}','other','/repo/source','write','now','now')".into()),
        ("unknown stage", "UPDATE stage_executions SET status='future_status' WHERE id='stage'".into()),
        ("shared effective worktree", "UPDATE runs SET worktree_root='/repo/source' WHERE id='other'; UPDATE agent_executions SET status='running' WHERE id='other-agent'".into()),
        ("unknown barrier", format!("{BARRIER}; UPDATE worktree_mutation_barriers SET status='future_status'")),
        ("side effect attempt", format!("{SIDE_EFFECT}; INSERT INTO side_effect_attempts(id,side_effect_id,attempt_number,owner_instance_id,started_at) VALUES ('attempt','row',1,'owner','now')")),
        ("repair lease", format!("{REPAIR}; INSERT INTO output_contract_repair_leases(lease_key,repair_event_id,run_id,stage_execution_id,parent_agent_execution_id,lease_kind,idempotency_token,lease_owner_principal_id,lease_acquired_at,lease_expires_at,lease_seconds,created_at,updated_at) VALUES ('lease','row','source','stage','agent','repair','token','operator','now','later',60,'now','now')")),
        ("orphan signal", "INSERT INTO shutdown_signal_side_effects(signal_effect_id,provider_session_id,shutdown_epoch,process_id,process_start_identity,signal_kind,generation,intent_state) VALUES ('row','unmapped-provider',1,42,'identity','kill',1,'identity_mismatch')".into()),
        ("unknown cancellation owner", "INSERT INTO provider_cancellation_intents(provider_session_id,cancellation_epoch,intent_state,reason,requested_at_monotonic_ms,requested_at_wall_clock) VALUES ('unmapped-provider',1,'held','operator_cancel',1,'now')".into()),
        ("unreleased continuation lease", format!("{CONTINUATION}; INSERT INTO agent_external_side_effect_ledger(continuation_id,idempotency_key,side_effect_kind,sequence_number,status) VALUES ('row','effect','worktree_lease',1,'committed')")),
        ("unverified continuation worker release", format!("{CONTINUATION}; INSERT INTO supervised_workers_continuation(continuation_id,worker_pid,daemon_generation_id,session_generation_id,registered_at,released_at,release_reason) VALUES ('row',42,'daemon','generation','now','now','stale_generation_reap_unverified')")),
        ("continuation reconciliation", format!("{CONTINUATION}; UPDATE agent_work_continuations SET reconciliation_status='unknown'")),
    ]
}

#[tokio::test]
async fn reservation_denies_canonical_active_and_unknown_owners_atomically() {
    let mut missed = vec![];
    for (name, seed) in busy_cases() {
        let pool = pool().await;
        execute(&pool, &seed).await;
        let result = c::reserve(&pool, &request()).await;
        if !matches!(&result, Err(e) if e.to_string().contains("source_busy")) {
            missed.push(format!(
                "{name}: {:?}",
                result.map(|_| "admitted").map_err(|e| format!("{e:#}"))
            ));
        } else {
            for table in [
                "run_continuations",
                "run_execution_fences",
                "run_continuation_commands",
                "command_journal",
            ] {
                assert_eq!(
                    sqlx::query_scalar::<_, i64>(AssertSqlSafe(format!(
                        "SELECT COUNT(*) FROM {table}"
                    )))
                    .fetch_one(&pool)
                    .await
                    .unwrap(),
                    0,
                    "{name}: {table}"
                );
            }
        }
        pool.close().await;
    }
    assert!(
        missed.is_empty(),
        "reservation admitted owners: {missed:#?}"
    );
}

#[tokio::test]
async fn admission_uses_canonical_approval_decisions_and_holds_unknowns() {
    for (decision, allowed) in [
        ("granted", true),
        ("rejected", true),
        ("expired", true),
        ("pending", false),
        ("requested", false),
        ("approved", false),
        ("cancelled", false),
        ("superseded", false),
    ] {
        let pool = pool().await;
        sqlx::query("INSERT INTO approvals(id,run_id,stage_id,decision,requested_at) VALUES ('approval','source','s',?,'now')")
            .bind(decision).execute(&pool).await.unwrap();
        let result = c::reserve(&pool, &request()).await;
        if allowed {
            result.unwrap_or_else(|error| panic!("{decision}: {error:#}"));
        } else {
            assert!(
                result.unwrap_err().to_string().contains("source_busy"),
                "{decision}"
            );
        }
    }
}

#[tokio::test]
async fn reservation_must_also_deny_late_owner_dispatch() {
    let mut missed = vec![];
    for (name, sql) in busy_cases() {
        let pool = pool().await;
        c::reserve(&pool, &request()).await.unwrap();
        let result = sqlx::raw_sql(AssertSqlSafe(sql)).execute(&pool).await;
        if !matches!(&result, Err(e) if e.to_string().contains("continuation_in_progress")) {
            missed.push(format!("{name}: {result:?}"));
        }
        pool.close().await;
    }
    assert!(
        missed.is_empty(),
        "late ownership was not fenced: {missed:#?}"
    );
}

#[tokio::test]
async fn headless_run_and_effective_project_ownership_are_not_owner_lineage() {
    for (run, path, busy) in [
        ("source", "/unrelated/App.xcodeproj", true),
        ("other", "/repo/source/App.xcodeproj", true),
        ("other", "/repo/source-neighbor/App.xcodeproj", false),
    ] {
        let pool = pool().await;
        let id = uuid::Uuid::new_v4().to_string();
        let operation = uuid::Uuid::new_v4().to_string();
        let project = serde_json::json!({"uid":501,"canonical_path":path,"device":"1","inode":"2"});
        let intent = serde_json::json!({"run_id":run,"owner_lineage":"opaque-lineage","operation_key":operation,"project_key":project,"origin":"lifecycle","effect_class":"mutate","binding_digest":null,"operation":"workspace_open"});
        sqlx::query("INSERT INTO xcode_effect_attempts(attempt_id,nonce,owner_lineage,project_key,operation_key,intent_json,state,revision,created_at,updated_at) VALUES (?,?,'opaque-lineage',?,?,?,'prepared',0,'now','now')")
            .bind(&id).bind(&id).bind(project.to_string()).bind(operation).bind(intent.to_string()).execute(&pool).await.unwrap();
        let result = c::reserve(&pool, &request()).await;
        if busy {
            assert!(
                result.unwrap_err().to_string().contains("source_busy"),
                "{run} {path}"
            );
        } else {
            result.unwrap();
        }
    }
}

#[tokio::test]
async fn fence_promotion_requires_current_prepared_generation() {
    let pool = pool().await;
    fence(&pool).await;
    sqlx::query("UPDATE run_continuations SET phase='prepared',version=2,generation=2,manifest_ref='manifest',manifest_sha256=?").bind("d".repeat(64)).execute(&pool).await.unwrap();
    assert!(
        sqlx::query("UPDATE run_execution_fences SET disposition='historical'")
            .execute(&pool)
            .await
            .is_err()
    );
    execute(
        &pool,
        "UPDATE run_execution_fences SET disposition='historical',generation=2",
    )
    .await;
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT generation FROM run_execution_fences")
            .fetch_one(&pool)
            .await
            .unwrap(),
        2
    );
}

#[tokio::test]
async fn replacement_cannot_reset_fence_identity_without_recursive_triggers() {
    let pool = pool().await;
    fence(&pool).await;
    let mut connection = pool.acquire().await.unwrap();
    sqlx::query("PRAGMA recursive_triggers=OFF")
        .execute(&mut *connection)
        .await
        .unwrap();
    let error = sqlx::query("INSERT OR REPLACE INTO run_execution_fences(source_run_id,operation_id,generation,disposition,created_at) VALUES ('source','op',1,'reserved','rewritten')")
        .execute(&mut *connection).await.expect_err("REPLACE must not erase an existing fence");
    assert!(error.to_string().contains("continuation_"));
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT created_at FROM run_execution_fences WHERE source_run_id='source'"
        )
        .fetch_one(&mut *connection)
        .await
        .unwrap(),
        "now"
    );
}

#[tokio::test]
async fn replacement_cannot_reset_approval_consumption_without_recursive_triggers() {
    let pool = pool().await;
    execute(&pool, "INSERT INTO approvals(id,run_id,stage_id,decision,requested_at) VALUES ('approval','other','s','granted','now')").await;
    sqlx::query("INSERT INTO run_continuation_approval_bindings(approval_id,successor_run_id,stage_execution_id,logical_stage_id,evidence_json,evidence_sha256,generation,status,consumed_transition_id) VALUES ('approval','other','other-stage','s','{}',?,1,'current','consumed')")
        .bind("e".repeat(64)).execute(&pool).await.unwrap();
    let mut connection = pool.acquire().await.unwrap();
    sqlx::query("PRAGMA recursive_triggers=OFF")
        .execute(&mut *connection)
        .await
        .unwrap();
    let error = sqlx::query("INSERT OR REPLACE INTO run_continuation_approval_bindings(approval_id,successor_run_id,stage_execution_id,logical_stage_id,evidence_json,evidence_sha256,generation,status,consumed_transition_id) SELECT approval_id,successor_run_id,stage_execution_id,logical_stage_id,evidence_json,evidence_sha256,generation,'current',NULL FROM run_continuation_approval_bindings")
        .execute(&mut *connection).await.expect_err("REPLACE must not unconsume an approval");
    assert!(error.to_string().contains("continuation_binding"));
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT consumed_transition_id FROM run_continuation_approval_bindings"
        )
        .fetch_one(&mut *connection)
        .await
        .unwrap(),
        "consumed"
    );
}

#[tokio::test]
async fn source_owner_mutations_check_old_and_new_and_preserve_history() {
    let cases = [
        ("stage_executions", "id='stage'", "", "run_id"),
        ("approvals", "id='row'", "INSERT INTO approvals(id,run_id,stage_id,decision,requested_at) VALUES ('row','source','s','granted','now')", "run_id"),
        ("artifacts", "id='row'", "INSERT INTO artifacts(id,run_id,stage_id,agent_id,name,contract_id,format,file_path,provider,created_at) VALUES ('row','source','s','a','name','contract','md','/old','codex','now')", "run_id"),
        ("artifact_contract_generations", "generation_id='row'", "INSERT INTO artifact_contract_generations(generation_id,run_id,artifact_id,contract_id,canonical_path,raw_path,raw_status,canonical_status,valid,created_at) VALUES ('row','source','artifact','contract','/old','/raw','valid','valid',1,'now')", "run_id"),
        ("active_artifact_contracts", "run_id='source'", "INSERT INTO artifact_contract_generations(generation_id,run_id,artifact_id,contract_id,canonical_path,raw_path,raw_status,canonical_status,valid,created_at) VALUES ('row','source','artifact','contract','/old','/raw','valid','valid',1,'now'); INSERT INTO active_artifact_contracts(run_id,contract_id,generation_id,updated_at) VALUES ('source','contract','row','now')", "run_id"),
        ("artifact_contract_overrides", "override_id='row'", "INSERT INTO artifact_contract_overrides(override_id,run_id,contract_id,override_type,from_status,to_status,reason,owner,expires_at_stage,journal_id,created_at) VALUES ('row','source','contract','override','invalid','valid','reason','operator','s','journal','now')", "run_id"),
        ("session_lineages", "id='lineage'", "", "run_id"),
        ("session_generations", "id='generation'", "", "lineage_id"),
        ("agent_executions", "id='agent'", "", "stage_execution_id"),
        ("workflow_transition_cursors", "run_id='source'", "INSERT INTO workflow_transition_cursors(run_id,current_state_id,cursor_status,resume_policy,updated_at,record_json) VALUES ('source','state','blocked','manual','now','{}')", "run_id"),
        ("workflow_conflicts", "conflict_id='row'", "INSERT INTO workflow_conflicts(conflict_id,conflict_fingerprint,run_id,current_state_id,reason,operator_label,status,candidate_transitions_json,candidate_transition_hash,created_at,updated_at,diagnostic_redaction_tier,record_json) VALUES ('row','fingerprint','source','s','workflow_conflict_unverifiable','Conflict','resolved','[]','hash','now','now','operator_safe','{}')", "run_id"),
        ("main_sync_attempts", "id='row'", MAIN_SYNC, "run_id"),
        ("worktree_mutation_barriers", "id='row'", BARRIER, "run_id"),
        ("lead_conflict_mediations", "id='mediation'", "", "run_id"),
        ("escalation_ledger", "id='row'", "INSERT INTO escalation_ledger(id,run_id,stage_id,agent_id,policy_id,policy_hash,status_raw,created_at,updated_at) VALUES ('row','source','s','a','policy','hash','paused','now','now')", "run_id"),
        ("agent_work_continuations", "id='row'", CONTINUATION, "run_id"),
        ("output_contract_repair_events", "repair_attempt_id='row'", REPAIR, "run_id"),
        ("work_items", "id='row'", "INSERT INTO work_items(id,kind,payload_json,run_id,status,created_at,scheduled_at) VALUES ('row','advance_run','{}','source','completed','now','now')", "run_id"),
    ];
    let mut missed = vec![];
    for (table, selector, seed, owner) in cases {
        let pool = pool().await;
        if !seed.is_empty() {
            execute(&pool, seed).await;
            execute(&pool, &other(seed)).await;
        }
        fence(&pool).await;
        for (direction, sql) in [
            (
                "same owner",
                format!("UPDATE {table} SET {owner}={owner} WHERE {selector}"),
            ),
            (
                "old owner",
                format!(
                    "UPDATE {table} SET {owner}={} WHERE {selector}",
                    match owner {
                        "lineage_id" => "'other-lineage'",
                        "stage_execution_id" => "'other-stage'",
                        _ => "'other'",
                    }
                ),
            ),
            (
                "new owner",
                format!(
                    "UPDATE {table} SET {owner}={} WHERE {}",
                    match owner {
                        "lineage_id" => "'lineage'",
                        "stage_execution_id" => "'stage'",
                        _ => "'source'",
                    },
                    other(selector)
                ),
            ),
            ("delete", format!("DELETE FROM {table} WHERE {selector}")),
        ] {
            let mut tx = pool.begin().await.unwrap();
            let result = sqlx::query(AssertSqlSafe(sql)).execute(&mut *tx).await;
            if !matches!(&result, Err(e) if e.to_string().contains("continuation_in_progress")) {
                missed.push(format!("{table} {direction}: {result:?}"));
            }
            tx.rollback().await.unwrap();
        }
        pool.close().await;
    }
    assert!(
        missed.is_empty(),
        "unfenced source mutation paths: {missed:#?}"
    );
}

#[tokio::test]
async fn fence_identity_and_promotion_are_guarded_but_abort_release_is_allowed() {
    let pool = pool().await;
    fence(&pool).await;
    for sql in [
        "UPDATE run_execution_fences SET source_run_id='other'",
        "UPDATE run_execution_fences SET generation=2",
        "UPDATE run_execution_fences SET created_at='rewritten'",
        "UPDATE run_execution_fences SET disposition='historical'",
        "DELETE FROM run_execution_fences",
    ] {
        let mut tx = pool.begin().await.unwrap();
        assert!(sqlx::query(sql).execute(&mut *tx).await.is_err(), "{sql}");
        tx.rollback().await.unwrap();
    }
    sqlx::query("UPDATE run_continuations SET phase='prepared',version=2,manifest_ref='manifest',manifest_sha256=?").bind("d".repeat(64)).execute(&pool).await.unwrap();
    execute(
        &pool,
        "UPDATE run_execution_fences SET disposition='historical'",
    )
    .await;
    assert!(
        sqlx::query("UPDATE run_execution_fences SET disposition='reserved'")
            .execute(&pool)
            .await
            .is_err()
    );
    let error = sqlx::query("UPDATE stage_executions SET status='running' WHERE id='stage'")
        .execute(&pool)
        .await
        .unwrap_err();
    assert!(error.to_string().contains("source_continued"));
}

#[tokio::test]
async fn approval_binding_identity_and_consumption_are_one_way() {
    let pool = pool().await;
    execute(&pool, "INSERT INTO approvals(id,run_id,stage_id,decision,requested_at) VALUES ('approval','other','s','granted','now')").await;
    sqlx::query("INSERT INTO run_continuation_approval_bindings(approval_id,successor_run_id,stage_execution_id,logical_stage_id,evidence_json,evidence_sha256,generation,status,consumed_transition_id) VALUES ('approval','other','other-stage','s','{}',?,1,'current',NULL)").bind("e".repeat(64)).execute(&pool).await.unwrap();
    for change in [
        "successor_run_id='source'",
        "stage_execution_id='stage'",
        "logical_stage_id='changed'",
        "evidence_json='[]'",
        "generation=2",
    ] {
        let mut tx = pool.begin().await.unwrap();
        assert!(
            sqlx::query(AssertSqlSafe(format!(
                "UPDATE run_continuation_approval_bindings SET {change}"
            )))
            .execute(&mut *tx)
            .await
            .is_err(),
            "{change}"
        );
        tx.rollback().await.unwrap();
    }
    execute(
        &pool,
        "UPDATE run_continuation_approval_bindings SET consumed_transition_id='transition'",
    )
    .await;
    for change in [
        "consumed_transition_id='other'",
        "consumed_transition_id=NULL",
    ] {
        assert!(
            sqlx::query(AssertSqlSafe(format!(
                "UPDATE run_continuation_approval_bindings SET {change}"
            )))
            .execute(&pool)
            .await
            .is_err(),
            "{change}"
        );
    }
    execute(
        &pool,
        "UPDATE run_continuation_approval_bindings SET status='superseded'",
    )
    .await;
    assert!(
        sqlx::query("UPDATE run_continuation_approval_bindings SET status='current'")
            .execute(&pool)
            .await
            .is_err()
    );
    assert!(
        sqlx::query("DELETE FROM run_continuation_approval_bindings")
            .execute(&pool)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn malformed_unowned_queue_json_does_not_break_historical_readback() {
    let pool = pool().await;
    fence(&pool).await;
    execute(&pool, "INSERT INTO work_items(id,kind,payload_json,status,created_at,scheduled_at) VALUES ('legacy','advance_run','{malformed','pending','now','now')").await;
    execute(&pool, "INSERT INTO session_events(id,lineage_id,generation_id,event_type,recorded_at) VALUES ('readback','lineage','generation','historical_readback','now')").await;
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM session_events")
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
}

#[tokio::test]
async fn settled_owners_and_operation_reconciliation_remain_available() {
    let pool = pool().await;
    for seed in [CONTINUATION, REPAIR, MAIN_SYNC, BARRIER, SIDE_EFFECT] {
        execute(&pool, seed).await;
    }
    execute(&pool, "INSERT INTO agent_external_side_effect_ledger(continuation_id,idempotency_key,side_effect_kind,sequence_number,status) VALUES ('row','effect','worktree_lease',1,'released');
        INSERT INTO supervised_workers_continuation(continuation_id,worker_pid,daemon_generation_id,session_generation_id,registered_at,released_at,release_reason) VALUES ('row',42,'daemon','generation','now','now','completed')").await;
    let op = c::reserve(&pool, &request()).await.unwrap();
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
    c::recover_preparations(&pool).await.unwrap();
    let recovered = c::find(&pool, &op.operation_id).await.unwrap().unwrap();
    assert_eq!(recovered.phase, "needs_reconciliation");
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT status FROM run_continuation_steps")
            .fetch_one(&pool)
            .await
            .unwrap(),
        "unknown"
    );
    execute(&pool, "UPDATE output_contract_repair_events SET projection_integrity='stale',projection_rebuild_attempts=1,updated_at='later'").await;
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM side_effects")
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
}

async fn assert_fenced(pool: &SqlitePool, sql: &str, reason: &str) {
    let mut tx = pool.begin().await.unwrap();
    let error = sqlx::raw_sql(AssertSqlSafe(sql))
        .execute(&mut *tx)
        .await
        .expect_err(sql);
    assert!(error.to_string().contains(reason), "{sql}: {error}");
    tx.rollback().await.unwrap();
}

#[tokio::test]
async fn fence_surface_has_115_guards_and_preserves_approval_columns() {
    let pool = pool().await;
    let tables = [
        "runs",
        "approvals",
        "artifacts",
        "workflow_transition_cursors",
        "implementation_handoff_statuses",
        "artifact_contract_overrides",
        "stage_executions",
        "session_lineages",
        "lead_conflict_mediations",
        "workflow_conflicts",
        "escalation_ledger",
        "agent_work_continuations",
        "agent_retry_budget_ledger",
        "artifact_source_generation_claims",
        "artifact_contract_generations",
        "active_artifact_contracts",
        "session_generations",
        "agent_executions",
        "lead_mediation_confirmations",
        "work_items",
        "main_sync_attempts",
        "worktree_mutation_barriers",
        "background_leases",
        "provider_sessions",
        "provider_cancellation_intents",
        "shutdown_signal_side_effects",
        "agent_external_side_effect_ledger",
        "supervised_workers_continuation",
        "side_effects",
        "side_effect_attempts",
        "output_contract_repair_events",
        "output_contract_repair_leases",
        "output_contract_repair_fallback_parent_links",
        "escalation_execution_metadata",
        "escalation_deadline_windows",
        "p080_helper_leases_v1",
        "p080_helper_lease_members_v1",
        "runtime_invocations",
        "xcode_effect_attempts",
    ];
    let actual: Vec<String> = sqlx::query_scalar(
        "SELECT name FROM sqlite_schema WHERE type='trigger' AND name GLOB 'p039_guard_*' ORDER BY name",
    ).fetch_all(&pool).await.unwrap();
    let mut expected = vec![];
    for table in tables {
        let columns: Vec<String> = sqlx::query_scalar("SELECT name FROM pragma_table_info(?)")
            .bind(table)
            .fetch_all(&pool)
            .await
            .unwrap();
        let assignments = columns
            .iter()
            .map(|column| format!("{column}={column}"))
            .collect::<Vec<_>>()
            .join(",");
        // Compile every trigger against the real migrated schema, including
        // guarded tables not exercised by the behavioral owner matrix below.
        for statement in [
            format!("EXPLAIN INSERT INTO {table} DEFAULT VALUES"),
            format!("EXPLAIN UPDATE {table} SET {assignments} WHERE 0"),
            format!("EXPLAIN DELETE FROM {table} WHERE 0"),
        ] {
            execute(&pool, &statement).await;
        }
        for event in ["insert", "update", "delete"] {
            if (table == "runs" && event == "insert")
                || (table == "xcode_effect_attempts" && event == "delete")
            {
                continue;
            }
            expected.push(format!("p039_guard_{table}_{event}"));
        }
    }
    expected.sort();
    assert_eq!(expected.len(), 115);
    assert_eq!(actual, expected);
    let columns: Vec<String> = sqlx::query_scalar(
        "SELECT name FROM pragma_table_info('run_continuation_approval_bindings')",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    for column in [
        "snapshot_artifact_id",
        "first_execution_id",
        "first_execution_started_at",
    ] {
        assert!(columns.iter().any(|value| value == column), "{column}");
    }
    pool.close().await;
}

#[tokio::test]
async fn fence_readback_exceptions_do_not_grant_execution_or_replace_history() {
    let pool = pool().await;
    execute(&pool, REPAIR).await;
    execute(&pool, SIDE_EFFECT).await;
    execute(&pool, "INSERT INTO side_effect_attempts(id,side_effect_id,attempt_number,owner_instance_id,started_at,completed_at,exit_status) VALUES ('original','row',1,'owner','now','now','succeeded')").await;
    fence(&pool).await;
    execute(
        &pool,
        "UPDATE provider_sessions SET updated_at='observed' WHERE provider_session_id='provider'",
    )
    .await;
    execute(&pool, "UPDATE output_contract_repair_events SET projection_integrity='stale',projection_stale_since='now',projection_schema_version='v2',projection_rebuild_attempts=1,updated_at='observed'").await;
    execute(&pool, "INSERT INTO side_effect_attempts(id,side_effect_id,attempt_number,attempt_kind,owner_instance_id,started_at) VALUES ('readback','row',2,'readback','observer','now')").await;
    execute(&pool, "UPDATE side_effect_attempts SET completed_at='observed',exit_status='succeeded',observed_evidence_summary_json='{}' WHERE id='readback'").await;
    for sql in [
        "UPDATE provider_sessions SET updated_at='observed',process_fate='running' WHERE provider_session_id='provider'",
        "UPDATE output_contract_repair_events SET projection_integrity='fresh',status='in_progress'",
        "UPDATE side_effect_attempts SET attempt_kind='retry' WHERE id='readback'",
        "UPDATE side_effect_attempts SET attempt_number=3 WHERE id='readback'",
        "DELETE FROM side_effect_attempts WHERE id='readback'",
        "INSERT OR REPLACE INTO side_effect_attempts(id,side_effect_id,attempt_number,attempt_kind,owner_instance_id,started_at) VALUES ('original','row',1,'readback','observer','later')",
    ] {
        assert_fenced(&pool, sql, "continuation_in_progress").await;
    }
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT attempt_kind FROM side_effect_attempts WHERE id='original'"
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        "initial"
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT status FROM output_contract_repair_events")
            .fetch_one(&pool)
            .await
            .unwrap(),
        "recovered"
    );
    pool.close().await;
}

#[tokio::test]
async fn fence_indirect_provider_continuation_and_effect_owners_are_checked() {
    let pool = pool().await;
    execute(&pool, "UPDATE provider_sessions SET agent_execution_id='agent' WHERE provider_session_id='other-provider'").await;
    execute(
        &pool,
        &CONTINUATION
            .replace("'source'", "'other'")
            .replace("'stage'", "'other-stage'"),
    )
    .await;
    execute(
        &pool,
        &SIDE_EFFECT
            .replace("'source'", "'other'")
            .replace("'stage'", "'other-stage'"),
    )
    .await;
    execute(&pool, "UPDATE side_effects SET agent_execution_id='agent'").await;
    execute(&pool, "INSERT INTO p080_helper_leases_v1(id,run_id,stage_id,work_item_id,helper_kind,process_group_id,owner_process_id,process_start_boottime_nanos,acquired_at,created_at,updated_at,agent_execution_id,terminated_at,termination_reason) VALUES ('helper','other','s','work','fixture',42,42,1,'now','now','now','agent','now','verified_terminated')").await;
    fence(&pool).await;
    for sql in [
        "INSERT INTO provider_cancellation_intents(provider_session_id,cancellation_epoch,intent_state,reason,requested_at_monotonic_ms,requested_at_wall_clock) VALUES ('other-provider',1,'requested','operator_cancel',1,'now')",
        "INSERT INTO shutdown_signal_side_effects(signal_effect_id,provider_session_id,shutdown_epoch,process_id,process_start_identity,signal_kind,generation,intent_state) VALUES ('signal','other-provider',1,42,'identity','kill',1,'planned')",
        "INSERT INTO agent_external_side_effect_ledger(continuation_id,idempotency_key,side_effect_kind,sequence_number,status) VALUES ('row','effect','provider_send',1,'started')",
        "INSERT INTO supervised_workers_continuation(continuation_id,worker_pid,daemon_generation_id,session_generation_id,registered_at) VALUES ('row',42,'daemon','other-generation','now')",
        "INSERT INTO side_effect_attempts(id,side_effect_id,attempt_number,owner_instance_id,started_at) VALUES ('attempt','row',1,'owner','now')",
        "INSERT INTO p080_helper_lease_members_v1(lease_id,member_pid,member_process_start_boottime_nanos,recorded_parent_pid,acquired_at,created_at,updated_at) VALUES ('helper',43,1,42,'now','now','now')",
        "INSERT INTO runtime_invocations(id,agent_execution_id,provider,status,started_at) VALUES ('invocation','agent','fixture','running','now')",
        "UPDATE runs SET worktree_root='/repo/source' WHERE id='other'",
        "INSERT INTO work_items(id,kind,payload_json,run_id,status,created_at,scheduled_at) VALUES ('unknown','invoke_agent','{}','missing','running','now','now')",
    ] {
        assert_fenced(&pool, sql, "continuation_in_progress").await;
    }
    execute(
        &pool,
        "UPDATE runs SET workflow_title='Unrelated readback' WHERE id='other'",
    )
    .await;
    pool.close().await;
}

#[tokio::test]
async fn fence_replacement_cannot_rebind_source_or_child_identity() {
    let pool = pool().await;
    fence(&pool).await;
    let mut conn = pool.acquire().await.unwrap();
    sqlx::query("PRAGMA recursive_triggers=OFF")
        .execute(&mut *conn)
        .await
        .unwrap();
    for sql in [
        "INSERT OR REPLACE INTO runs(id,idea_id,status,workflow_id,workflow_title,workspace_root,artifact_root,started_at) VALUES ('source','idea-other','completed','replacement','Replacement','/other','/other/meta','later')",
        "INSERT OR REPLACE INTO stage_executions(id,run_id,stage_id,label,status,started_at) VALUES ('stage','other','s','Replacement','completed','later')",
        "INSERT OR REPLACE INTO agent_executions(id,stage_execution_id,owner_id,agent_id,provider,status,started_at) VALUES ('agent','other-stage','other-stage','a','codex','completed','later')",
    ] {
        let error = sqlx::query(sql).execute(&mut *conn).await.expect_err(sql);
        assert!(error.to_string().contains("continuation_in_progress"), "{sql}: {error}");
    }
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT idea_id FROM runs WHERE id='source'")
            .fetch_one(&mut *conn)
            .await
            .unwrap(),
        "idea-source"
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT run_id FROM stage_executions WHERE id='stage'")
            .fetch_one(&mut *conn)
            .await
            .unwrap(),
        "source"
    );
    drop(conn);
    pool.close().await;
}

#[tokio::test]
async fn fence_unknown_typed_queue_owners_cannot_be_claimed() {
    let pool = pool().await;
    fence(&pool).await;
    for key in [
        "run_id",
        "stage_execution_id",
        "agent_execution_id",
        "mediation_record_id",
        "lead_mediation_record_id",
        "continuation_id",
    ] {
        let payload = serde_json::json!({key:"missing"}).to_string();
        let error = sqlx::query("INSERT INTO work_items(id,kind,payload_json,status,created_at,scheduled_at) VALUES (?,'invoke_agent',?,'running','now','now')")
            .bind(key).bind(payload).execute(&pool).await.expect_err(key);
        assert!(
            error.to_string().contains("continuation_in_progress"),
            "{key}: {error}"
        );
    }
    pool.close().await;
}

async fn headless_insert(pool: &SqlitePool, run: &str, path: &str) -> Result<String, sqlx::Error> {
    let id = uuid::Uuid::new_v4().to_string();
    let operation = uuid::Uuid::new_v4().to_string();
    let project = serde_json::json!({"uid":501,"canonical_path":path,"device":"1","inode":"2"});
    let intent = serde_json::json!({"run_id":run,"owner_lineage":"opaque-lineage","operation_key":operation,"project_key":project,"origin":"lifecycle","effect_class":"mutate","binding_digest":null,"operation":"workspace_open"});
    sqlx::query("INSERT INTO xcode_effect_attempts(attempt_id,nonce,owner_lineage,project_key,operation_key,intent_json,state,revision,created_at,updated_at) VALUES (?,?,'opaque-lineage',?,?,?,'prepared',0,'now','now')")
        .bind(&id).bind(&id).bind(project.to_string()).bind(operation).bind(intent.to_string()).execute(pool).await?;
    Ok(id)
}

#[tokio::test]
async fn fence_headless_admission_dispatch_and_settlement_are_distinct() {
    let pool = pool().await;
    let prepared = headless_insert(&pool, "source", "/repo/source/App.xcodeproj")
        .await
        .unwrap();
    let dispatched = headless_insert(&pool, "source", "/repo/source/App.xcodeproj")
        .await
        .unwrap();
    sqlx::query("UPDATE xcode_effect_attempts SET state='dispatched',dispatched_at='now',revision=1 WHERE attempt_id=?")
        .bind(&dispatched).execute(&pool).await.unwrap();
    // A manual fixture fence models an already committed stale dispatch: it may
    // settle uncertainty, but cannot authorize another dispatch or identity.
    fence(&pool).await;
    for (run, path) in [
        ("source", "/elsewhere/App.xcodeproj"),
        ("other", "/repo/source/App.xcodeproj"),
        ("other", "/alias/App.xcodeproj"),
        ("missing", "/unknown/App.xcodeproj"),
    ] {
        let error = headless_insert(&pool, run, path).await.unwrap_err();
        assert!(
            error.to_string().contains("continuation_in_progress"),
            "{run}: {path}: {error}"
        );
    }
    let error = sqlx::query("UPDATE xcode_effect_attempts SET state='dispatched',dispatched_at='now',revision=1 WHERE attempt_id=?")
        .bind(&prepared).execute(&pool).await.unwrap_err();
    assert!(
        error.to_string().contains("continuation_in_progress"),
        "{error}"
    );
    sqlx::query("UPDATE xcode_effect_attempts SET state='unknown',uncertainty_reason='restart',revision=2 WHERE attempt_id=?")
        .bind(&dispatched).execute(&pool).await.unwrap();
    let digest = "d".repeat(64);
    let outcome =
        serde_json::json!({"schema_version":1,"result_digest":digest,"payload":{"kind":"result"}})
            .to_string();
    sqlx::query("UPDATE xcode_effect_attempts SET state='reconciled',revision=3,completed_at='later',outcome_json=?,result_digest=?,settlement_proof_json='{}',reconciliation_json='{}',reconciliation_evidence_ref='receipt' WHERE attempt_id=?")
        .bind(outcome).bind(digest).bind(&dispatched).execute(&pool).await.unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT state FROM xcode_effect_attempts WHERE attempt_id=?"
        )
        .bind(&dispatched)
        .fetch_one(&pool)
        .await
        .unwrap(),
        "reconciled"
    );
    sqlx::query("UPDATE run_continuations SET phase='prepared',version=2,manifest_ref='manifest',manifest_sha256=?")
        .bind("d".repeat(64)).execute(&pool).await.unwrap();
    execute(
        &pool,
        "UPDATE run_execution_fences SET disposition='historical'",
    )
    .await;
    let error = headless_insert(&pool, "source", "/elsewhere/App.xcodeproj")
        .await
        .unwrap_err();
    assert!(error.to_string().contains("source_continued"), "{error}");
    pool.close().await;
}
