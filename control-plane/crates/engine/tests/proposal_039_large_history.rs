#![allow(dead_code)]
mod p039_fixture;

use domain::{
    ids::RunId,
    run_carry_forward::{ContentDigest, EntryRole},
};
use engine::run_carry_forward::{
    preview::{preview, PreviewOutcome},
    read_verified_manifest,
};
use p039_fixture::Fixture;
use serde_json::{json, Value};
use std::collections::BTreeSet;
use uuid::Uuid;

#[tokio::test]
async fn mandatory_215_references_survive_preparation_activation_readback_and_agent_context() {
    roundtrip(215).await;
}

#[tokio::test]
async fn expanded_history_at_512_survives_full_roundtrip() {
    roundtrip(512).await;
}

async fn roundtrip(count: usize) {
    let f = Fixture::new()
        .await
        .with_rollout_seed()
        .await
        .with_reference_count(count)
        .await;
    let plan = f.plan().await;
    assert_eq!(
        plan.plan()
            .entries
            .iter()
            .filter(|e| e.role == EntryRole::ReferenceOnly)
            .count(),
        count
    );
    let op = f.prepare().await;
    let (service, principal) = f.service();
    let m = read_verified_manifest(&op).await.unwrap();
    assert_eq!(m.inputs.len(), count + 1);
    let intent: String = sqlx::query_scalar("SELECT intent_json FROM run_continuation_steps WHERE operation_id=? AND step_key='finalizer_metadata'")
        .bind(&op.operation_id).fetch_one(&f.pool).await.unwrap();
    assert!(intent.len() <= 65536);
    let intent: Value = serde_json::from_str(&intent).unwrap();
    assert_eq!(intent["source_input_count"], count + 1);
    assert_eq!(
        intent["source_inputs_sha256"],
        serde_json::to_value(domain::run_carry_forward::canonical_digest(&plan.inputs()).unwrap())
            .unwrap()
    );
    let activation = service.call("runs.continuation_activate", &principal, json!({
        "operation_id":op.operation_id,"expected_version":op.version,
        "expected_manifest_sha256":ContentDigest::from_sha256_hex(op.manifest_sha256.as_deref().unwrap()).unwrap(),
        "caller_request_id":Uuid::new_v4()
    })).await.unwrap();
    assert_eq!(activation["status"], "accepted", "{activation:#}");
    let successor = RunId(Uuid::parse_str(&op.reserved_successor_run_id).unwrap());
    let refs = db::repos::run_continuation_inputs::references(&f.pool, successor)
        .await
        .unwrap();
    assert_eq!(refs.len(), count);
    assert_eq!(
        refs.iter()
            .map(|r| &r.original_artifact_id)
            .collect::<BTreeSet<_>>()
            .len(),
        count
    );

    let mut cursor = Value::Null;
    let mut references = BTreeSet::new();
    loop {
        let page = service
            .call(
                "runs.continuation_get",
                &principal,
                json!({"operation_id":op.operation_id,"cursor":cursor,"limit":73}),
            )
            .await
            .unwrap();
        let _: domain::run_carry_forward_api::RunContinuationReadbackV1 =
            serde_json::from_value(page.clone()).unwrap();
        for entry in page["manifest_page"].as_array().unwrap() {
            if entry["role"] == "reference_only" {
                assert!(references.insert(entry["source_artifact_id"].clone().to_string()));
            }
        }
        cursor = page["next_cursor"].clone();
        if cursor.is_null() {
            break;
        }
    }
    assert_eq!(references.len(), count);
    let run = db::repos::runs::find_by_id(&f.pool, successor)
        .await
        .unwrap()
        .unwrap();
    let idea = db::repos::ideas::find_by_id(&f.pool, run.idea_id)
        .await
        .unwrap()
        .unwrap();
    let task = &m.target.plan.states["state_4_proposal_reviewed"].tasks[0];
    let context =
        engine::run_carry_forward::inputs::resolve_task_inputs(&f.pool, &run, &m.target.plan, task)
            .await
            .unwrap()
            .unwrap();
    assert_eq!(context.references.len(), count);
    let prompt = engine::orchestrator::build_task_prompt_for_runtime(
        &f.pool,
        task,
        &m.target.plan,
        &run,
        Some(&idea),
        None,
        None,
        false,
    )
    .await
    .unwrap();
    assert!(
        prompt.len() <= acp::input_context::MAX_PROMPT_BYTES,
        "prompt has {} bytes",
        prompt.len()
    );
    assert!(prompt.contains("not approval, waiver, gate evidence, or a current provider output"));
    let mut request: acp::ExecutionRequest = serde_json::from_value(json!({
        "run_id":successor, "stage_id":run.current_state, "agent_id":task.agent.agent_id,
        "provider":task.agent.provider, "workspace_root":run.workspace_root,
        "worktree_root":run.worktree_root, "worktree_write_enabled":task.agent.worktree_write_enabled,
        "worktree_strategy":task.agent.worktree_strategy, "prompt":prompt
    })).unwrap();
    acp::input_context::bind_request_manifest(&mut request).unwrap();
    let snapshots = serde_json::to_value(request.input_manifest.as_ref().unwrap()).unwrap();
    let mut observed = BTreeSet::new();
    for snapshot in snapshots["artifacts"].as_array().unwrap() {
        if !snapshot["name"]
            .as_str()
            .unwrap()
            .starts_with("carry_forward_references/")
        {
            continue;
        }
        let bytes = std::fs::read(snapshot["path"].as_str().unwrap()).unwrap();
        let bundle: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(bundle["authority"], "none");
        for record in bundle["references"].as_array().unwrap() {
            let expected = context
                .references
                .iter()
                .find(|r| r.name == record["name"])
                .unwrap();
            assert!(observed.insert(expected.name.clone()));
            assert_eq!(record["content"], expected.content);
            assert_eq!(record["source_path"], expected.display_path);
            assert_eq!(
                record["sha256"],
                ContentDigest::of(expected.content.as_bytes()).as_str()
            );
        }
    }
    assert_eq!(observed.len(), count);
    let report =
        engine::run_carry_forward::readback::report_fields(&f.pool, successor, Default::default())
            .await
            .unwrap();
    assert_eq!(
        serde_json::to_value(report).unwrap()["carry_forward_input_provenance"]
            .as_array()
            .unwrap()
            .len(),
        count + 1
    );

    // A disposable successor settled at a later blocked frontier must carry the
    // complete installed history again. This fixture does not dispatch providers.
    sqlx::query("UPDATE work_items SET status='completed' WHERE run_id=?")
        .bind(successor.to_string())
        .execute(&f.pool)
        .await
        .unwrap();
    db::repos::runs::update_current_state(&f.pool, successor, "state_7_implementation_started")
        .await
        .unwrap();
    db::repos::runs::update_status(&f.pool, successor, domain::run::RunStatus::Blocked)
        .await
        .unwrap();
    sqlx::query("INSERT INTO stage_executions(id,run_id,stage_id,label,status,iteration,attempt_number,started_at) VALUES (?,?,'state_7_implementation_started','Fixture frontier','blocked',0,1,'2026-09-27')")
        .bind(Uuid::new_v4().to_string()).bind(successor.to_string()).execute(&f.pool).await.unwrap();
    sqlx::query("INSERT INTO approvals(id,run_id,stage_id,decision,requested_at,decided_at) VALUES (?,?,'state_6_implementation_approval','granted','2026-09-26','2026-09-27')")
        .bind(Uuid::new_v4().to_string()).bind(successor.to_string()).execute(&f.pool).await.unwrap();
    let mut again = f.input.clone();
    again.source_run_id = successor;
    again.target.delivery_configuration_json = run.delivery_configuration_json.clone().unwrap();
    again.selection.proposal_input =
        engine::run_carry_forward::preview::InputReference::CarriedInput {
            id: m
                .inputs
                .iter()
                .find(|i| i.role == EntryRole::ExecutionSeed)
                .unwrap()
                .input_id,
        };
    let carried = match preview(&f.pool, again).await.unwrap() {
        PreviewOutcome::Page { prepared, .. } => prepared,
        other => panic!("expected complete second-fork preview: {other:?}"),
    };
    assert_eq!(carried.inputs().len(), count + 1);
    assert_eq!(
        carried
            .inputs()
            .iter()
            .filter(|i| i.role == EntryRole::ReferenceOnly)
            .map(|i| i.original_artifact_id.to_string())
            .collect::<BTreeSet<_>>(),
        refs.iter()
            .map(|r| r.original_artifact_id.clone())
            .collect()
    );

    if count == 512 {
        let seed = m
            .inputs
            .iter()
            .find(|i| i.role == EntryRole::ExecutionSeed)
            .unwrap();
        sqlx::query("UPDATE run_continuation_inputs SET role='reference_only' WHERE input_id=?")
            .bind(seed.input_id.to_string())
            .execute(&f.pool)
            .await
            .unwrap();
        assert!(
            db::repos::run_continuation_inputs::references(&f.pool, successor)
                .await
                .unwrap_err()
                .to_string()
                .contains("continuation_budget_exceeded")
        );
        sqlx::query("UPDATE run_continuation_inputs SET role='execution_seed' WHERE input_id=?")
            .bind(seed.input_id.to_string())
            .execute(&f.pool)
            .await
            .unwrap();
    }

    // Corruption beyond the old 128th row must still be observed, never truncated away.
    let last = m
        .inputs
        .iter()
        .rev()
        .find(|i| i.role == EntryRole::ReferenceOnly)
        .unwrap();
    std::fs::write(
        m.workspace.metadata_root.join(&last.target_relative_path),
        "tampered",
    )
    .unwrap();
    assert!(engine::run_carry_forward::inputs::resolve_task_inputs(
        &f.pool,
        &run,
        &m.target.plan,
        task
    )
    .await
    .is_err());
}

#[tokio::test]
async fn expanded_history_over_512_holds_without_reservation_or_truncation() {
    let f = Fixture::new().await.with_reference_count(513).await;
    assert!(matches!(preview(&f.pool, f.input.clone()).await.unwrap(),
        PreviewOutcome::Held { holds } if holds.iter().any(|h| h.code.as_str() == "continuation_budget_exceeded")));
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM run_continuations")
            .fetch_one(&f.pool)
            .await
            .unwrap(),
        0
    );
}
