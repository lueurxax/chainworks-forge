use db::repos::{
    run_continuation_inputs::{self as inputs, PreparedInput},
    run_continuations as c,
};
use domain::{
    ids::{ArtifactId, RunId},
    run_carry_forward::{ContentDigest, EntryRole},
};

#[tokio::test]
async fn prepared_inputs_validate_immediate_source_without_fabricating_outputs() {
    let pool = db::pool::create_pool("sqlite::memory:").await.unwrap();
    db::writer::register_shared_writer(
        &pool,
        std::sync::Arc::new(db::writer::DbWriter::new(pool.clone())),
    )
    .await
    .unwrap();
    let source = RunId::new();
    let idea = uuid::Uuid::new_v4();
    let artifact = ArtifactId::new();
    sqlx::query("INSERT INTO ideas(id,title,body,created_at) VALUES (?,'Idea','Body','now')")
        .bind(idea.to_string())
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO runs(id,idea_id,status,workflow_id,workflow_title,workspace_root,artifact_root,started_at) VALUES (?,?,'blocked','w','W','/repo','/meta','2026-09-20T00:00:00Z')")
        .bind(source.to_string()).bind(idea.to_string()).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO artifacts(id,run_id,stage_id,agent_id,name,contract_id,format,file_path,provider,created_at) VALUES (?,?,'s','writer','proposal_current','proposal_current','markdown','/old/proposal.md','codex','2026-09-20T00:00:00Z')")
        .bind(artifact.to_string()).bind(source.to_string()).execute(&pool).await.unwrap();
    let references = [ArtifactId::new(), ArtifactId::new()];
    for id in references {
        sqlx::query("INSERT INTO artifacts(id,run_id,stage_id,agent_id,name,contract_id,format,file_path,provider,created_at) VALUES (?,?,'s','writer','score_lift_backlog','score_lift_backlog_v1','json','/old/backlog.json','codex','2026-09-20T00:00:00Z')")
            .bind(id.to_string()).bind(source.to_string()).execute(&pool).await.unwrap();
    }
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
            reason: "test".into(),
        },
    )
    .await
    .unwrap();
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
    let op = c::mark_prepared(
        &pool,
        &op.operation_id,
        claim.generation,
        "manifest.json",
        &"d".repeat(64),
    )
    .await
    .unwrap();
    let input = PreparedInput {
        input_id: uuid::Uuid::new_v4(),
        ordinal: 0,
        role: EntryRole::ExecutionSeed,
        source_kind: "artifact".into(),
        source_id: artifact.0,
        source_run_id: source,
        original_artifact_id: artifact,
        source_schema: "proposal_current".into(),
        content_sha256: ContentDigest::of(b"proposal"),
        target_logical_name: "proposal_current".into(),
        target_relative_path: "proposals/current/proposal.md".into(),
        historical_relation: serde_json::json!({"status":"unproven"}),
    };
    let mut tx = pool.begin().await.unwrap();
    let mut foreign = input.clone();
    foreign.source_run_id = RunId::new();
    assert!(inputs::insert_tx(&mut tx, &op, &[foreign]).await.is_err());
    let mut escape = input.clone();
    escape.target_relative_path = "../proposal.md".into();
    assert!(inputs::insert_tx(&mut tx, &op, &[escape]).await.is_err());
    let mut reference = input.clone();
    reference.role = EntryRole::ReferenceOnly;
    assert!(inputs::insert_tx(&mut tx, &op, &[reference]).await.is_err());
    let mut all = vec![input.clone()];
    for (index, artifact) in references.into_iter().enumerate() {
        let mut reference = input.clone();
        reference.input_id = uuid::Uuid::new_v4();
        reference.ordinal = index + 1;
        reference.role = EntryRole::ReferenceOnly;
        reference.source_id = artifact.0;
        reference.original_artifact_id = artifact;
        reference.source_schema = "score_lift_backlog_v1".into();
        reference.target_logical_name = "score_lift_backlog".into();
        reference.target_relative_path =
            format!("carry-forward/references/{}/content", reference.input_id);
        all.push(reference);
    }
    inputs::insert_tx(&mut tx, &op, &all).await.unwrap();
    tx.commit().await.unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM run_continuation_inputs WHERE installed=0"
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        3
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM artifacts")
            .fetch_one(&pool)
            .await
            .unwrap(),
        3
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM active_artifact_contracts")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    assert!(inputs::execution_seed(&pool, source, "proposal_current")
        .await
        .unwrap()
        .is_none());
}
