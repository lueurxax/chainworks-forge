#![cfg(unix)]
#[allow(dead_code)]
mod p039_fixture;

use acp::ExecutionRequest;
use db::repos::{run_continuations, runs};
use domain::{ids::RunId, run::Run, run_carry_forward::ContentDigest};
use engine::run_carry_forward::{launch::approve_metadata_root, read_manifest};
use p039_fixture::Fixture;
use serde_json::json;
use std::fs;
use uuid::Uuid;

async fn request(f: &Fixture, run: &Run) -> ExecutionRequest {
    let stage = Uuid::new_v4().to_string();
    let execution = Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO stage_executions(id,run_id,stage_id,label,iteration,attempt_number,status,started_at) VALUES (?,?,'review','Review',0,1,'running','2026-09-20T00:00:00Z')")
        .bind(&stage).bind(run.id.to_string()).execute(&f.pool).await.unwrap();
    sqlx::query("INSERT INTO agent_executions(id,stage_execution_id,agent_id,provider,status,started_at,owner_kind,owner_id) VALUES (?,?,'reviewer','fixture','running','2026-09-20T00:00:00Z','stage_execution',?)")
        .bind(&execution).bind(&stage).bind(&stage).execute(&f.pool).await.unwrap();
    serde_json::from_value(json!({
        "run_id":run.id, "agent_execution_id":execution,
        "stage_execution_id":stage, "stage_id":"review",
        "agent_id":"reviewer", "provider":"fixture", "prompt":"fixture",
        "workspace_root":run.workspace_root, "worktree_root":run.worktree_root,
        "worktree_strategy":"shared_implementation_worktree",
        "worktree_write_enabled":false, "chainworks_meta_root":run.chainworks_meta_root
    }))
    .unwrap()
}

#[tokio::test]
async fn p039_metadata_authority_requires_activated_manifest_and_owned_execution() {
    let f = Fixture::new().await.with_rollout_seed().await;
    let foreign = Fixture::sharing_storage(&f).await;
    let foreign_run = runs::find_by_id(&f.pool, foreign.input.source_run_id)
        .await
        .unwrap()
        .unwrap();
    let foreign_request = request(&f, &foreign_run).await;
    let (service, principal, _, _, prepared) = f.service_prepare().await;
    let manifest = read_manifest(&prepared).await.unwrap();
    let mut pending: ExecutionRequest = serde_json::from_value(json!({
        "run_id":prepared.reserved_successor_run_id,
        "agent_execution_id":Uuid::new_v4(), "stage_id":"review",
        "agent_id":"reviewer", "provider":"fixture", "prompt":"fixture",
        "workspace_root":manifest.successor.workspace_root,
        "worktree_root":manifest.workspace.checkout_root,
        "chainworks_meta_root":manifest.workspace.metadata_root
    }))
    .unwrap();
    approve_metadata_root(&f.pool, &mut pending).await.unwrap();
    assert!(
        pending.approved_metadata_root.is_none(),
        "prepared files are not activated authority"
    );
    let response = service.call("runs.continuation_activate", &principal, json!({
        "operation_id":prepared.operation_id, "expected_version":prepared.version,
        "expected_manifest_sha256":ContentDigest::from_sha256_hex(prepared.manifest_sha256.as_deref().unwrap()).unwrap(),
        "caller_request_id":Uuid::new_v4()
    })).await.unwrap();
    assert_eq!(response["status"], "accepted", "{response:#}");
    let operation = run_continuations::find(&f.pool, &prepared.operation_id)
        .await
        .unwrap()
        .unwrap();
    let run_id: RunId = operation
        .successor_run_id
        .as_deref()
        .unwrap()
        .parse()
        .unwrap();
    let run = runs::find_by_id(&f.pool, run_id).await.unwrap().unwrap();
    let mut valid = request(&f, &run).await;
    let before = fs::read_dir(&manifest.workspace.metadata_root)
        .unwrap()
        .count();
    approve_metadata_root(&f.pool, &mut valid).await.unwrap();
    assert!(valid.approved_metadata_root.is_some());
    assert_eq!(
        fs::read_dir(&manifest.workspace.metadata_root)
            .unwrap()
            .count(),
        before,
        "minting authority performs no metadata writes"
    );

    for variant in 0..7 {
        let mut invalid = valid.clone();
        match variant {
            0 => invalid.workspace_root = foreign_run.workspace_root.clone(),
            1 => invalid.worktree_root = foreign_run.worktree_root.clone(),
            2 => invalid.chainworks_meta_root = foreign_run.chainworks_meta_root.clone(),
            3 => invalid.agent_id = "other".into(),
            4 => invalid.provider = "other".into(),
            5 => invalid.agent_execution_id = None,
            _ => {
                invalid.agent_execution_id = foreign_request.agent_execution_id;
                invalid.stage_execution_id = foreign_request.stage_execution_id.clone();
            }
        }
        assert!(
            approve_metadata_root(&f.pool, &mut invalid).await.is_err(),
            "variant {variant}"
        );
        assert!(
            invalid.approved_metadata_root.is_none(),
            "old authority must be cleared"
        );
    }

    let mut ordinary = foreign_request.clone();
    ordinary.approved_metadata_root = valid.approved_metadata_root.clone();
    approve_metadata_root(&f.pool, &mut ordinary).await.unwrap();
    assert!(ordinary.approved_metadata_root.is_none());

    sqlx::query("UPDATE agent_executions SET status='completed',completed_at='2026-09-20T00:01:00Z' WHERE id=?")
        .bind(valid.agent_execution_id.unwrap().to_string()).execute(&f.pool).await.unwrap();
    let mut finished = valid.clone();
    assert!(approve_metadata_root(&f.pool, &mut finished).await.is_err());
    assert!(finished.approved_metadata_root.is_none());
    sqlx::query("UPDATE agent_executions SET status='running',completed_at=NULL WHERE id=?")
        .bind(valid.agent_execution_id.unwrap().to_string())
        .execute(&f.pool)
        .await
        .unwrap();

    let path = operation.manifest_ref.as_deref().unwrap();
    let bytes = fs::read(path).unwrap();
    let mut changed = bytes.clone();
    changed.push(b' ');
    fs::write(path, changed).unwrap();
    let mut tampered = valid.clone();
    assert!(approve_metadata_root(&f.pool, &mut tampered).await.is_err());
    assert!(tampered.approved_metadata_root.is_none());
    fs::write(path, bytes).unwrap();
    approve_metadata_root(&f.pool, &mut valid).await.unwrap();
    assert!(valid.approved_metadata_root.is_some());
}
