use anyhow::Result;
use async_trait::async_trait;
use db::repos::run_continuations as continuations;
use domain::{
    ids::RunId,
    run_carry_forward::{canonical_digest, ContentDigest, EntryRole},
};
use engine::run_carry_forward::{
    read_manifest, read_verified_manifest, PreparationLane, PreparationObserver,
};
use serde_json::{json, Value};
use sqlx::SqlitePool;
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};
use uuid::Uuid;

#[allow(dead_code)]
mod p039_fixture;
use p039_fixture::*;

#[tokio::test]
async fn preparation_metrics_publish_only_after_durable_prepared_and_include_reservation_age() {
    const CHILD: &str = "P039_PREPARATION_METRICS_CHILD";
    // Metrics are process-global. Keep exact deltas isolated from sibling tests.
    if std::env::var_os(CHILD).is_none() {
        let output = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "preparation_metrics_publish_only_after_durable_prepared_and_include_reservation_age", "--nocapture"])
            .env(CHILD, "1").output().unwrap();
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        return;
    }
    db::metrics::reset_for_tests();
    let f = Fixture::new().await;
    let plan = f.plan().await;
    let age = chrono::Utc::now() - chrono::Duration::seconds(90);
    // Seed a historical queued reservation at INSERT time, preserving immutable timestamps.
    let id = Uuid::new_v4().to_string();
    let source = f.input.source_run_id.to_string();
    let idea: String = sqlx::query_scalar("SELECT idea_id FROM runs WHERE id=?")
        .bind(&source)
        .fetch_one(&f.pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO command_journal(id,command_type,payload_json,created_at) VALUES (?,'ContinueBlocked','{}',?)")
        .bind(&id).bind(age.to_rfc3339()).execute(&f.pool).await.unwrap();
    sqlx::query("INSERT INTO run_continuations(operation_id,source_run_id,idea_id,reserved_successor_run_id,caller_fingerprint,caller_request_id,intent_sha256,profile,plan_ref,plan_sha256,target_ref,source_witness_sha256,journal_id,created_at,updated_at,deadline_at) VALUES (?,?,?,?,'metrics-fixture',?,?,'implementation_restart_v1',?,?,?,?,?,?,?,?)")
        .bind(&id).bind(&source).bind(idea).bind(Uuid::new_v4().to_string()).bind(&id).bind("a".repeat(64))
        .bind(f.owned.join(&id).join("plan.json").to_str().unwrap()).bind(hex(&plan.plan().plan_sha256))
        .bind(f.owned.join(&id).join("target.json").to_str().unwrap()).bind(hex(&plan.source_witness().semantic_sha256))
        .bind(&id).bind(age.to_rfc3339()).bind(age.to_rfc3339()).bind((age+chrono::Duration::minutes(15)).to_rfc3339()).execute(&f.pool).await.unwrap();
    sqlx::query("INSERT INTO run_execution_fences(source_run_id,operation_id,generation,disposition,created_at) VALUES (?,?,1,'reserved',?)")
        .bind(&source).bind(&id).bind(age.to_rfc3339()).execute(&f.pool).await.unwrap();
    sqlx::query("INSERT INTO work_items(id,kind,payload_json,created_at,scheduled_at) VALUES (?,'prepare_run_continuation',?,?,?)")
        .bind(Uuid::new_v4().to_string()).bind(json!({"operation_id":id,"generation":1}).to_string()).bind(age.to_rfc3339()).bind(age.to_rfc3339()).execute(&f.pool).await.unwrap();
    let op = continuations::find(&f.pool, &id).await.unwrap().unwrap();
    assert_eq!(
        db::metrics::get_counter("run_continuation_preserved_bytes_total"),
        0
    );
    let lane = PreparationLane::new(f.pool.clone());
    let prepared = lane
        .try_submit(f.job(Uuid::parse_str(&op.operation_id).unwrap(), plan))
        .unwrap()
        .wait()
        .await
        .unwrap();
    lane.shutdown().await.unwrap();
    let manifest = read_manifest(&prepared).await.unwrap();
    assert_eq!(
        continuations::find(&f.pool, &op.operation_id)
            .await
            .unwrap()
            .unwrap()
            .phase,
        "prepared"
    );
    assert_eq!(
        db::metrics::get_counter("run_continuation_preserved_bytes_total"),
        manifest.preserved_bytes
    );
    let metrics = db::metrics::p039_rollout_metric_values_json();
    assert_eq!(
        metrics["run_continuation_phase_duration_ms"]["preparing"]["sample_count"],
        1
    );
    assert!(
        metrics["run_continuation_phase_duration_ms"]["preparing"]["p95"]
            .as_u64()
            .unwrap()
            >= 90_000
    );
    assert!(!metrics.to_string().contains(&op.operation_id));
    assert!(!metrics.to_string().contains(f.owned.to_str().unwrap()));
    // A partial-copy failure must not count as a completed preparation.
    let failed = Fixture::new().await;
    let plan = failed.plan().await;
    let op = failed.reserve(&plan).await;
    let lane = PreparationLane::new(failed.pool.clone());
    let observer = Arc::new(FullDiskDuringInput {
        pool: failed.pool.clone(),
        operation: op.operation_id.clone(),
        chunks: AtomicUsize::new(0),
    });
    assert!(lane
        .try_submit(
            failed
                .job(Uuid::parse_str(&op.operation_id).unwrap(), plan)
                .with_observer(observer)
        )
        .unwrap()
        .wait()
        .await
        .is_err());
    lane.shutdown().await.unwrap();
    assert_eq!(db::metrics::p039_rollout_metric_values_json(), metrics);
}

#[tokio::test]
async fn complete_manifest_survives_restart_without_installing_rows_or_historical_authority() {
    let f = Fixture::new().await;
    let op = f.prepare().await;
    assert_eq!(op.phase, "prepared");
    let m = read_verified_manifest(&op).await.unwrap();
    assert_eq!(m.inputs.len(), 2);
    assert_eq!(m.input_provenance.len(), 2);
    assert_eq!(m.successor.id.to_string(), op.reserved_successor_run_id);
    assert_eq!(m.successor.status, domain::run::RunStatus::Pending);
    assert_eq!(
        m.successor.current_state.as_deref(),
        Some("state_4_proposal_reviewed")
    );
    assert!(m.successor.delivery_preflight_json.is_none());
    assert!(m.successor.review_routing_json.is_none());
    assert!(m.plan["entries"].as_array().unwrap().len() > 2);
    assert_eq!(m.target_sha256, canonical_digest(&m.target).unwrap());
    assert_eq!(
        m.capability_delta_sha256,
        canonical_digest(&m.plan["capability_delta"]).unwrap()
    );
    let seed = m
        .inputs
        .iter()
        .find(|i| i.role == EntryRole::ExecutionSeed)
        .unwrap();
    assert_eq!(seed.target_logical_name, "proposal_current");
    assert_eq!(seed.target_relative_path, "proposals/current/proposal.md");
    assert_eq!(
        fs::read(m.workspace.metadata_root.join(&seed.target_relative_path)).unwrap(),
        f.seed
    );
    let reference = m
        .inputs
        .iter()
        .find(|i| i.role == EntryRole::ReferenceOnly)
        .unwrap();
    assert!(reference
        .target_relative_path
        .starts_with("carry-forward/references/"));
    assert!(!m
        .workspace
        .metadata_root
        .join("audit/proposal-vs-implementation.json")
        .exists());
    assert!(!m
        .workspace
        .metadata_root
        .join("proposals/approved")
        .exists());
    assert_eq!(
        fs::read(m.workspace.checkout_root.join("product.txt")).unwrap(),
        b"dirty source\n"
    );
    for (query, expected) in [
        ("SELECT COUNT(*) FROM runs", 1),
        ("SELECT COUNT(*) FROM artifacts", 2),
        ("SELECT COUNT(*) FROM run_continuation_inputs", 0),
        ("SELECT COUNT(*) FROM approvals", 1),
    ] {
        assert_eq!(
            sqlx::query_scalar::<_, i64>(query)
                .fetch_one(&f.pool)
                .await
                .unwrap(),
            expected,
            "{query}"
        );
    }
    assert_eq!(
        read_verified_manifest(&op).await.unwrap().plan_sha256,
        m.plan_sha256
    );
}

#[tokio::test]
async fn unreserved_job_and_pure_constructor_create_no_files() {
    let f = Fixture::new().await;
    let plan = f.plan().await;
    let lane = PreparationLane::new(f.pool.clone());
    assert!(lane
        .try_submit(f.job(Uuid::new_v4(), plan))
        .unwrap()
        .wait()
        .await
        .is_err());
    lane.shutdown().await.unwrap();
    assert_eq!(fs::read_dir(&f.owned).unwrap().count(), 0);
}

#[tokio::test]
async fn selected_source_digest_change_is_held_without_manifest() {
    let f = Fixture::new().await;
    let plan = f.plan().await;
    let op = f.reserve(&plan).await;
    fs::write(
        f.metadata.join("proposals/current/proposal.md"),
        vec![b'y'; f.seed.len()],
    )
    .unwrap();
    let lane = PreparationLane::new(f.pool.clone());
    let error = lane
        .try_submit(f.job(Uuid::parse_str(&op.operation_id).unwrap(), plan))
        .unwrap()
        .wait()
        .await
        .unwrap_err();
    assert!(error.to_string().contains("source_changed"), "{error:#}");
    lane.shutdown().await.unwrap();
    let now = continuations::find(&f.pool, &op.operation_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(now.phase, "needs_reconciliation");
    assert!(now.manifest_sha256.is_none());
    assert!(!f.owned.join(op.operation_id).join("manifest.json").exists());
}

#[tokio::test]
async fn restart_readback_rejects_manifest_input_and_checkout_tampering() {
    let f = Fixture::new().await;
    let op = f.prepare().await;
    let m = read_verified_manifest(&op).await.unwrap();
    let path = Path::new(op.manifest_ref.as_ref().unwrap());
    let bytes = fs::read(path).unwrap();
    fs::write(path, b"not json").unwrap();
    let error = read_verified_manifest(&op).await.unwrap_err();
    assert!(
        error.to_string().contains("manifest digest"),
        "must hash before decode: {error:#}"
    );
    fs::write(path, bytes).unwrap();
    let seed = m
        .inputs
        .iter()
        .find(|i| i.role == EntryRole::ExecutionSeed)
        .unwrap();
    let input_path = m.workspace.metadata_root.join(&seed.target_relative_path);
    fs::write(&input_path, b"changed").unwrap();
    assert!(read_verified_manifest(&op).await.is_err());
    fs::write(&input_path, &f.seed).unwrap();
    assert!(read_verified_manifest(&op).await.is_ok());
    fs::write(m.workspace.checkout_root.join("product.txt"), b"changed\n").unwrap();
    assert!(read_verified_manifest(&op).await.is_err());
}

#[tokio::test]
async fn service_rejects_changed_missing_or_corrupt_successor_index_without_repair() {
    for mutation in ["staged_only", "rewritten_clean", "missing", "corrupt"] {
        let f = Fixture::new().await.with_rollout_seed().await;
        let (service, principal, _, _, op) = f.service_prepare().await;
        let manifest = read_verified_manifest(&op).await.unwrap();
        let output = Command::new("/usr/bin/git")
            .current_dir(&manifest.workspace.checkout_root)
            .args(["rev-parse", "--path-format=absolute", "--git-path", "index"])
            .output()
            .unwrap();
        assert!(output.status.success());
        let index = PathBuf::from(String::from_utf8(output.stdout).unwrap().trim());
        let original_index = fs::read(&index).unwrap();
        let source_index = fs::read(f.root.join(".git/index")).unwrap();
        match mutation {
            "staged_only" => git(&manifest.workspace.checkout_root, &["add", "product.txt"]),
            "rewritten_clean" => {
                git(
                    &manifest.workspace.checkout_root,
                    &["update-index", "--index-version=4"],
                );
                git(
                    &manifest.workspace.checkout_root,
                    &["diff", "--cached", "--exit-code", "HEAD", "--"],
                );
            }
            "missing" => fs::remove_file(&index).unwrap(),
            "corrupt" => fs::write(&index, b"invalid index").unwrap(),
            _ => unreachable!(),
        }
        let changed = fs::read(&index).ok();
        assert_ne!(changed.as_ref(), Some(&original_index));
        let product = fs::read(manifest.workspace.checkout_root.join("product.txt")).unwrap();
        assert!(
            read_verified_manifest(&op).await.is_err(),
            "{mutation}: changed index accepted"
        );
        // Immutable historical evidence does not freeze legitimate successor staging forever.
        assert!(read_manifest(&op).await.is_ok());
        let denial = service.call("runs.continuation_activate", &principal, json!({
            "operation_id":op.operation_id, "expected_version":op.version,
            "expected_manifest_sha256":ContentDigest::from_sha256_hex(op.manifest_sha256.as_deref().unwrap()).unwrap(),
            "caller_request_id":Uuid::new_v4()
        })).await.unwrap();
        assert_eq!(denial["status"], "denied", "{mutation}: {denial:#}");
        assert_eq!(denial["denial"]["code"], "prepared_material_changed");
        let current = continuations::find(&f.pool, &op.operation_id)
            .await
            .unwrap()
            .unwrap();
        let request = Uuid::new_v4().to_string();
        continuations::reconcile_command(
            &f.pool,
            &op.operation_id,
            current.version,
            Some(domain::run_carry_forward_api::ContinuationReasonCode::PreparedMaterialChanged),
            &db::repos::run_continuation_commands::CommandIdentity {
                caller_fingerprint: "index-hold",
                caller_request_id: &request,
                command: "runs.continuation_reconcile",
                intent_sha256: &"a".repeat(64),
            },
        )
        .await
        .unwrap();
        let held = continuations::find(&f.pool, &op.operation_id)
            .await
            .unwrap()
            .unwrap();
        service.call("runs.continuation_reconcile", &principal, json!({
            "operation_id":op.operation_id, "expected_version":held.version,
            "caller_request_id":Uuid::new_v4(), "reason":"Verify prepared index without repair"
        })).await.unwrap();
        let held = continuations::find(&f.pool, &op.operation_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(held.phase, "needs_reconciliation", "{mutation}");
        assert!(held.holds_json.contains("prepared_material_changed"));
        assert!(held.successor_run_id.is_none());
        assert_eq!(
            fs::read(&index).ok(),
            changed,
            "index must not be repaired/refreshed"
        );
        assert_eq!(fs::read(f.root.join(".git/index")).unwrap(), source_index);
        assert_eq!(
            fs::read(manifest.workspace.checkout_root.join("product.txt")).unwrap(),
            product
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM runs")
                .fetch_one(&f.pool)
                .await
                .unwrap(),
            1
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM work_items WHERE kind='advance_run'"
            )
            .fetch_one(&f.pool)
            .await
            .unwrap(),
            0
        );
        assert!(
            continuations::check_source_guard(&f.pool, &op.source_run_id)
                .await
                .is_err()
        );
    }
}

struct AbortDuringInput {
    pool: SqlitePool,
    operation: String,
    chunks: AtomicUsize,
}

#[async_trait]
impl PreparationObserver for AbortDuringInput {
    async fn before_effect(&self, label: &str) -> Result<()> {
        if label == "input:copy_chunk" {
            let intents: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM run_continuation_steps WHERE operation_id=? AND step_key GLOB 'selected_input_[0-9]*' AND status='dispatching'")
                .bind(&self.operation).fetch_one(&self.pool).await?;
            assert_eq!(
                intents, 1,
                "selected input intent must precede every streamed write"
            );
        }
        if label == "input:copy_chunk" && self.chunks.fetch_add(1, Ordering::SeqCst) == 1 {
            let op = continuations::find(&self.pool, &self.operation)
                .await?
                .unwrap();
            continuations::begin_abort(&self.pool, &self.operation, op.version).await?;
        }
        Ok(())
    }
}

#[tokio::test]
async fn chunk_copy_observes_abort_before_next_write_or_publication() {
    let f = Fixture::new().await;
    let plan = f.plan().await;
    let op = f.reserve(&plan).await;
    let observer = Arc::new(AbortDuringInput {
        pool: f.pool.clone(),
        operation: op.operation_id.clone(),
        chunks: AtomicUsize::new(0),
    });
    let lane = PreparationLane::new(f.pool.clone());
    let error = lane
        .try_submit(
            f.job(Uuid::parse_str(&op.operation_id).unwrap(), plan)
                .with_observer(observer.clone()),
        )
        .unwrap()
        .wait()
        .await
        .unwrap_err();
    assert!(error.to_string().contains("stale_worker"), "{error:#}");
    lane.shutdown().await.unwrap();
    assert_eq!(observer.chunks.load(Ordering::SeqCst), 2);
    let root = f.owned.join(&op.operation_id);
    assert!(!root.join("metadata/proposals/current/proposal.md").exists());
    assert!(!root.join("manifest.json").exists());
}

struct ChangeAfterCopy {
    path: PathBuf,
}

struct FullDiskDuringInput {
    pool: SqlitePool,
    operation: String,
    chunks: AtomicUsize,
}

#[async_trait]
impl PreparationObserver for FullDiskDuringInput {
    async fn before_effect(&self, label: &str) -> Result<()> {
        if label == "input:copy_chunk" {
            let seed: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM run_continuation_steps WHERE operation_id=? AND step_key GLOB 'selected_input_[0-9]*' AND status='dispatching' AND json_extract(intent_json,'$.source.role')='execution_seed')")
                .bind(&self.operation).fetch_one(&self.pool).await?;
            if seed && self.chunks.fetch_add(1, Ordering::SeqCst) == 1 {
                return Err(std::io::Error::from_raw_os_error(libc::ENOSPC).into());
            }
        }
        Ok(())
    }
}

#[tokio::test]
async fn enospc_after_first_real_chunk_retains_partial_evidence_without_publication_or_retry() {
    let f = Fixture::new().await;
    let plan = f.plan().await;
    let op = f.reserve(&plan).await;
    let observer = Arc::new(FullDiskDuringInput {
        pool: f.pool.clone(),
        operation: op.operation_id.clone(),
        chunks: AtomicUsize::new(0),
    });
    let lane = PreparationLane::new(f.pool.clone());
    let error = lane
        .try_submit(
            f.job(Uuid::parse_str(&op.operation_id).unwrap(), plan)
                .with_observer(observer.clone()),
        )
        .unwrap()
        .wait()
        .await
        .unwrap_err();
    lane.shutdown().await.unwrap();
    assert!(
        error.chain().any(|e| e
            .downcast_ref::<std::io::Error>()
            .is_some_and(|e| e.raw_os_error() == Some(libc::ENOSPC))),
        "{error:#}"
    );
    assert_eq!(observer.chunks.load(Ordering::SeqCst), 2);
    let root = f.owned.join(&op.operation_id);
    let destination = root.join("metadata/proposals/current");
    assert!(!destination.join("proposal.md").exists());
    assert!(!root.join("manifest.json").exists());
    let partial: Vec<_> = fs::read_dir(&destination)
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    assert_eq!(partial.len(), 1);
    assert!(partial[0]
        .file_name()
        .unwrap()
        .to_str()
        .unwrap()
        .starts_with(".pending-"));
    let retained = fs::read(&partial[0]).unwrap();
    assert_eq!(retained.len(), 1024 * 1024);
    assert_eq!(retained, f.seed[..1024 * 1024]);
    assert_eq!(
        fs::read(f.metadata.join("proposals/current/proposal.md")).unwrap(),
        f.seed
    );
    let held = continuations::find(&f.pool, &op.operation_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(held.phase, "needs_reconciliation");
    assert!(
        !held.worker_claimed,
        "returned synchronous I/O is settled, not replay permission"
    );
    assert!(held.manifest_ref.is_none());
    assert!(
        continuations::check_source_guard(&f.pool, &op.source_run_id)
            .await
            .is_err()
    );
    assert_eq!(sqlx::query_scalar::<_,String>("SELECT status FROM run_continuation_steps WHERE operation_id=? AND step_key GLOB 'selected_input_[0-9]*' AND json_extract(intent_json,'$.source.role')='execution_seed'").bind(&op.operation_id).fetch_one(&f.pool).await.unwrap(), "unknown");
    continuations::recover_preparations(&f.pool).await.unwrap();
    assert!(
        continuations::claim_preparation_for(&f.pool, &op.operation_id)
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(fs::read(&partial[0]).unwrap(), retained);
    assert_eq!(sqlx::query_scalar::<_,i64>("SELECT attempt_count FROM work_items WHERE kind='prepare_run_continuation' AND json_extract(payload_json,'$.operation_id')=?").bind(&op.operation_id).fetch_one(&f.pool).await.unwrap(), 1);
}

struct FinalizerCrash {
    pool: SqlitePool,
    operation: String,
    label: &'static str,
    role: Option<&'static str>,
    hits: AtomicUsize,
}

#[async_trait]
impl PreparationObserver for FinalizerCrash {
    async fn before_effect(&self, label: &str) -> Result<()> {
        if label != self.label {
            return Ok(());
        }
        let intent: Option<(String, String)> = if let Some(role) = self.role {
            sqlx::query_as("SELECT status,intent_json FROM run_continuation_steps WHERE operation_id=? AND step_key GLOB 'selected_input_[0-9]*' AND status='dispatching' AND json_extract(intent_json,'$.source.role')=?")
                .bind(&self.operation).bind(role).fetch_optional(&self.pool).await?
        } else {
            sqlx::query_as("SELECT status,intent_json FROM run_continuation_steps WHERE operation_id=? AND step_key='finalizer_metadata'")
                .bind(&self.operation).fetch_optional(&self.pool).await?
        };
        // Preservation has its own manifest.json before the finalizer is entered.
        let Some((status, intent)) = intent else {
            return Ok(());
        };
        assert_eq!(status, "dispatching", "journal must precede {label}");
        assert!(serde_json::from_str::<Value>(&intent)?.is_object());
        self.hits.fetch_add(1, Ordering::SeqCst);
        continuations::recover_preparations(&self.pool).await?;
        Ok(())
    }
}

#[tokio::test]
async fn input_and_complete_manifest_checkpoints_hold_on_restart_without_recopying() {
    for (label, role) in [
        ("input:published", Some("execution_seed")),
        ("input:open", Some("execution_seed")),
        ("input:create_pending", Some("execution_seed")),
        ("input:copy_chunk", Some("execution_seed")),
        ("input:publish_ready", Some("execution_seed")),
        ("input:open", Some("reference_only")),
        ("input:copy_chunk", Some("reference_only")),
        ("input:publish_ready", Some("reference_only")),
        ("input:published", Some("reference_only")),
        ("finalizer:manifest", None),
        ("publish:manifest.json", None),
        ("publish_ready:manifest.json", None),
        ("published:manifest.json", None),
    ] {
        let f = Fixture::new().await;
        let plan = f.plan().await;
        let op = f.reserve(&plan).await;
        let observer = Arc::new(FinalizerCrash {
            pool: f.pool.clone(),
            operation: op.operation_id.clone(),
            label,
            role,
            hits: AtomicUsize::new(0),
        });
        let lane = PreparationLane::new(f.pool.clone());
        let result = lane
            .try_submit(
                f.job(Uuid::parse_str(&op.operation_id).unwrap(), plan)
                    .with_observer(observer.clone()),
            )
            .unwrap()
            .wait()
            .await;
        lane.shutdown().await.unwrap();
        assert_eq!(
            observer.hits.load(Ordering::SeqCst),
            1,
            "missing checkpoint {label} {role:?}"
        );
        assert!(result.is_err(), "stale worker published after {label}");
        let evidence = private_file_digests(&f.owned.join(&op.operation_id));
        assert_eq!(
            fs::read(f.metadata.join("proposals/current/proposal.md")).unwrap(),
            f.seed
        );
        f.pool.close().await;
        let reopened = db::pool::create_pool(&format!(
            "sqlite://{}?mode=rwc",
            f._temp.path().join("fixture.db").display()
        ))
        .await
        .unwrap();
        db::writer::register_shared_writer(
            &reopened,
            Arc::new(db::writer::DbWriter::new(reopened.clone())),
        )
        .await
        .unwrap();
        let held = continuations::find(&reopened, &op.operation_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(held.phase, "needs_reconciliation", "{label}");
        assert_eq!(held.generation, op.generation + 1);
        assert!(held.worker_claimed);
        assert!(held.manifest_ref.is_none());
        assert!(
            continuations::claim_preparation_for(&reopened, &op.operation_id)
                .await
                .unwrap()
                .is_none()
        );
        assert!(
            continuations::check_source_guard(&reopened, &op.source_run_id)
                .await
                .is_err()
        );
        assert_eq!(
            private_file_digests(&f.owned.join(&op.operation_id)),
            evidence
        );
        assert_eq!(sqlx::query_scalar::<_,i64>("SELECT attempt_count FROM work_items WHERE kind='prepare_run_continuation' AND json_extract(payload_json,'$.operation_id')=?").bind(&op.operation_id).fetch_one(&reopened).await.unwrap(), 1);
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM runs")
                .fetch_one(&reopened)
                .await
                .unwrap(),
            1
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM run_continuation_inputs")
                .fetch_one(&reopened)
                .await
                .unwrap(),
            0
        );
        reopened.close().await;
    }
}

#[async_trait]
impl PreparationObserver for ChangeAfterCopy {
    async fn before_effect(&self, label: &str) -> Result<()> {
        if label == "finalizer:manifest" {
            fs::write(&self.path, b"changed after successful input copy")?;
        }
        Ok(())
    }
}

#[tokio::test]
async fn selected_source_is_rechecked_after_finalizer_wait() {
    let f = Fixture::new().await;
    let plan = f.plan().await;
    let op = f.reserve(&plan).await;
    let observer = Arc::new(ChangeAfterCopy {
        path: f.metadata.join("proposals/current/proposal.md"),
    });
    let lane = PreparationLane::new(f.pool.clone());
    let result = lane
        .try_submit(
            f.job(Uuid::parse_str(&op.operation_id).unwrap(), plan)
                .with_observer(observer),
        )
        .unwrap()
        .wait()
        .await;
    lane.shutdown().await.unwrap();
    let error = result.expect_err("source changed after copy but preparation was promoted");
    assert!(error.to_string().contains("source_changed"), "{error:#}");
    assert!(!f.owned.join(op.operation_id).join("manifest.json").exists());
}

#[tokio::test]
async fn selected_source_symlink_replacement_never_copies_external_bytes() {
    let f = Fixture::new().await;
    let plan = f.plan().await;
    let op = f.reserve(&plan).await;
    let source = f.metadata.join("proposals/current/proposal.md");
    fs::remove_file(&source).unwrap();
    std::os::unix::fs::symlink(f.root.join("product.txt"), &source).unwrap();
    let lane = PreparationLane::new(f.pool.clone());
    assert!(lane
        .try_submit(f.job(Uuid::parse_str(&op.operation_id).unwrap(), plan))
        .unwrap()
        .wait()
        .await
        .is_err());
    lane.shutdown().await.unwrap();
    assert!(!f.owned.join(op.operation_id).join("manifest.json").exists());
}

#[tokio::test]
async fn restart_readback_rejects_new_active_outputs_and_linked_input_or_manifest() {
    let f = Fixture::new().await;
    let op = f.prepare().await;
    let m = read_verified_manifest(&op).await.unwrap();
    let unexpected = m.workspace.metadata_root.join("approved_proposal.md");
    fs::write(&unexpected, "not approval authority").unwrap();
    assert!(read_verified_manifest(&op).await.is_err());
    fs::remove_file(unexpected).unwrap();
    let seed = m
        .inputs
        .iter()
        .find(|i| i.role == EntryRole::ExecutionSeed)
        .unwrap();
    let path = m.workspace.metadata_root.join(&seed.target_relative_path);
    fs::remove_file(&path).unwrap();
    std::os::unix::fs::symlink(f.metadata.join("proposals/current/proposal.md"), &path).unwrap();
    assert!(read_verified_manifest(&op).await.is_err());
    fs::remove_file(&path).unwrap();
    fs::write(&path, &f.seed).unwrap();
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    assert!(read_verified_manifest(&op).await.is_ok());
    let manifest = Path::new(op.manifest_ref.as_ref().unwrap());
    let other = f.owned.join("elsewhere.json");
    fs::rename(manifest, &other).unwrap();
    std::os::unix::fs::symlink(&other, manifest).unwrap();
    assert!(read_verified_manifest(&op).await.is_err());
}

#[tokio::test]
async fn immutable_manifest_allows_successor_work_but_preservation_and_references_stay_pinned() {
    let f = Fixture::new().await;
    let op = f.prepare().await;
    let m = read_manifest(&op).await.unwrap();
    let seed = m
        .inputs
        .iter()
        .find(|i| i.role == EntryRole::ExecutionSeed)
        .unwrap();
    fs::write(
        m.workspace.metadata_root.join(&seed.target_relative_path),
        b"Fresh reviewed revision",
    )
    .unwrap();
    fs::write(
        m.workspace.checkout_root.join("product.txt"),
        b"Legitimate successor implementation",
    )
    .unwrap();
    fs::write(m.workspace.metadata_root.join("fresh-review.json"), b"{}").unwrap();
    fs::write(
        f.metadata.join("proposals/current/proposal.md"),
        b"Source no longer active",
    )
    .unwrap();
    assert!(read_manifest(&op).await.is_ok());
    assert!(read_verified_manifest(&op).await.is_err());
    let reference = m
        .inputs
        .iter()
        .find(|i| i.role == EntryRole::ReferenceOnly)
        .unwrap();
    let path = m
        .workspace
        .metadata_root
        .join(&reference.target_relative_path);
    let bytes = fs::read(&path).unwrap();
    fs::write(&path, b"Historical reference mutation").unwrap();
    assert!(read_manifest(&op).await.is_err());
    fs::write(&path, bytes).unwrap();
    assert!(read_manifest(&op).await.is_ok());
    fs::write(
        m.workspace.preservation_root.join("workspace/product.txt"),
        b"Preservation mutation",
    )
    .unwrap();
    assert!(read_manifest(&op).await.is_err());
}

#[tokio::test]
async fn successor_keeps_original_delivery_base_while_checkout_is_pinned_at_source_head() {
    let f = Fixture::new().await;
    let original: String = sqlx::query_scalar("SELECT base_revision FROM runs WHERE id=?")
        .bind(f.input.source_run_id.to_string())
        .fetch_one(&f.pool)
        .await
        .unwrap();
    git(&f.root, &["add", "product.txt"]);
    git(
        &f.root,
        &[
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=fixture@example.invalid",
            "commit",
            "-qm",
            "source progress",
        ],
    );
    fs::write(
        f.root.join("product.txt"),
        b"Still dirty after source progress",
    )
    .unwrap();
    let op = f.prepare().await;
    let m = read_verified_manifest(&op).await.unwrap();
    assert_ne!(m.workspace.head, original);
    assert_eq!(
        m.successor.base_revision.as_deref(),
        Some(original.as_str())
    );
    assert_eq!(
        Path::new(&op.plan_ref),
        m.workspace.operation_root.join("plan.json")
    );
    assert_eq!(
        Path::new(&op.target_ref),
        m.workspace.operation_root.join("target.json")
    );
}

#[tokio::test]
async fn service_roundtrip_prepares_pages_activates_and_replays_without_second_advance() {
    let f = Fixture::new().await.with_rollout_seed().await;
    let (service, principal, prepare_request, accepted, op) = f.service_prepare().await;
    let manifest = read_manifest(&op).await.unwrap();
    let expected_entries = manifest.plan["entries"].as_array().unwrap().len();
    let mut cursor = Value::Null;
    let mut entries = Vec::new();
    loop {
        let page = service
            .call(
                "runs.continuation_get",
                &principal,
                json!({"operation_id":op.operation_id,"cursor":cursor,"limit":7}),
            )
            .await
            .unwrap();
        let _: domain::run_carry_forward_api::RunContinuationReadbackV1 =
            serde_json::from_value(page.clone()).unwrap();
        let slice = page["manifest_page"].as_array().unwrap();
        assert!(!slice.is_empty() && slice.len() <= 7);
        entries.extend(slice.iter().cloned());
        assert!(entries.len() <= expected_entries);
        assert!(
            !page.to_string().contains(f.owned.to_str().unwrap()),
            "private operation paths must not leak through get"
        );
        cursor = page["next_cursor"].clone();
        if cursor.is_null() {
            break;
        }
    }
    assert_eq!(entries.len(), expected_entries);
    let activate_request = json!({"operation_id":op.operation_id,"expected_version":op.version,
        "expected_manifest_sha256":ContentDigest::from_sha256_hex(op.manifest_sha256.as_deref().unwrap()).unwrap(),"caller_request_id":Uuid::new_v4()});
    let activation = service
        .call(
            "runs.continuation_activate",
            &principal,
            activate_request.clone(),
        )
        .await
        .unwrap();
    assert_eq!(
        activation["status"], "accepted",
        "valid real rollout evidence must activate: {activation:#}"
    );
    let active = continuations::find(&f.pool, &op.operation_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(active.phase, "activated");
    let successor = RunId(Uuid::parse_str(active.successor_run_id.as_deref().unwrap()).unwrap());
    assert_eq!(successor.to_string(), op.reserved_successor_run_id);
    assert!(
        continuations::check_source_guard(&f.pool, &op.source_run_id)
            .await
            .unwrap_err()
            .to_string()
            .contains("source_continued")
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT disposition FROM run_execution_fences WHERE source_run_id=?"
        )
        .bind(&op.source_run_id)
        .fetch_one(&f.pool)
        .await
        .unwrap(),
        "historical"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM runs")
            .fetch_one(&f.pool)
            .await
            .unwrap(),
        2
    );
    let seed =
        db::repos::run_continuation_inputs::execution_seed(&f.pool, successor, "proposal_current")
            .await
            .unwrap()
            .unwrap();
    assert_eq!(seed.content_sha256, hex(&ContentDigest::of(&f.seed)));
    assert!(
        db::repos::run_continuation_inputs::execution_seed(&f.pool, successor, "audit_report")
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(
        db::repos::run_continuation_inputs::references(&f.pool, successor)
            .await
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM artifacts")
            .fetch_one(&f.pool)
            .await
            .unwrap(),
        2
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM approvals")
            .fetch_one(&f.pool)
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM provider_sessions")
            .fetch_one(&f.pool)
            .await
            .unwrap(),
        0
    );
    // Simulate later canonical progress without dispatching providers or inventing review outputs.
    db::repos::runs::update_status(&f.pool, successor, domain::run::RunStatus::Running)
        .await
        .unwrap();
    db::repos::runs::update_current_state(
        &f.pool,
        successor,
        &manifest.target.profile.approval_state,
    )
    .await
    .unwrap();
    let replay = service
        .call("runs.continue_blocked", &principal, prepare_request)
        .await
        .unwrap();
    assert_eq!(replay["status"], "replayed");
    assert_eq!(replay["replay_of"], "accepted");
    for field in ["journal_id", "operation", "next_action"] {
        assert_eq!(
            replay[field], accepted[field],
            "original preparation receipt {field}"
        );
    }
    let replay = service
        .call("runs.continuation_activate", &principal, activate_request)
        .await
        .unwrap();
    assert_eq!(replay["status"], "replayed");
    for field in ["journal_id", "operation", "next_action"] {
        assert_eq!(
            replay[field], activation[field],
            "original activation receipt {field}"
        );
    }
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM work_items WHERE kind='advance_run' AND run_id=?"
        )
        .bind(successor.to_string())
        .fetch_one(&f.pool)
        .await
        .unwrap(),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM work_items WHERE kind='advance_run' AND run_id=?"
        )
        .bind(&op.source_run_id)
        .fetch_one(&f.pool)
        .await
        .unwrap(),
        0
    );
}

#[tokio::test]
async fn service_reconcile_verified_material_restores_prepared_without_effects_then_activates() {
    let f = Fixture::new().await.with_rollout_seed().await;
    let (service, principal, _, _, op) = f.service_prepare().await;
    let manifest = read_verified_manifest(&op).await.unwrap();
    let before = private_file_digests(&manifest.workspace.operation_root);
    let steps: Vec<(String, String, Option<String>, String)> = sqlx::query_as(
        "SELECT step_key,status,receipt_sha256,updated_at FROM run_continuation_steps WHERE operation_id=? ORDER BY step_key"
    ).bind(&op.operation_id).fetch_all(&f.pool).await.unwrap();
    let request_id = Uuid::new_v4().to_string();
    let identity = db::repos::run_continuation_commands::CommandIdentity {
        caller_fingerprint: "finalizer-test-hold",
        caller_request_id: &request_id,
        command: "runs.continuation_reconcile",
        intent_sha256: &"a".repeat(64),
    };
    continuations::reconcile_command(
        &f.pool,
        &op.operation_id,
        op.version,
        Some(domain::run_carry_forward_api::ContinuationReasonCode::PreparedMaterialChanged),
        &identity,
    )
    .await
    .unwrap();
    let held = continuations::find(&f.pool, &op.operation_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(held.phase, "needs_reconciliation");
    assert!(!held.worker_claimed);
    let result = service
        .call(
            "runs.continuation_reconcile",
            &principal,
            json!({
                "operation_id":held.operation_id, "expected_version":held.version,
                "caller_request_id":Uuid::new_v4(), "reason":"Reverify complete owned material"
            }),
        )
        .await
        .unwrap();
    assert_eq!(result["status"], "accepted", "{result:#}");
    let restored = continuations::find(&f.pool, &op.operation_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(restored.phase, "prepared");
    assert_eq!(restored.version, held.version + 1);
    assert_eq!(restored.generation, op.generation);
    assert_eq!(restored.manifest_ref, op.manifest_ref);
    assert_eq!(restored.manifest_sha256, op.manifest_sha256);
    assert_eq!(restored.holds_json, "[]");
    assert!(!restored.worker_claimed);
    assert!(restored.successor_run_id.is_none());
    assert_eq!(
        private_file_digests(&manifest.workspace.operation_root),
        before
    );
    let after: Vec<(String, String, Option<String>, String)> = sqlx::query_as(
        "SELECT step_key,status,receipt_sha256,updated_at FROM run_continuation_steps WHERE operation_id=? ORDER BY step_key"
    ).bind(&op.operation_id).fetch_all(&f.pool).await.unwrap();
    assert_eq!(
        after, steps,
        "reconciliation must not replay or adopt effects"
    );
    for (query, expected) in [
        ("SELECT COUNT(*) FROM runs", 1),
        ("SELECT COUNT(*) FROM run_continuation_inputs", 0),
        (
            "SELECT COUNT(*) FROM work_items WHERE kind='advance_run'",
            0,
        ),
        (
            "SELECT SUM(attempt_count) FROM work_items WHERE kind='prepare_run_continuation'",
            1,
        ),
        ("SELECT COUNT(*) FROM provider_sessions", 0),
    ] {
        assert_eq!(
            sqlx::query_scalar::<_, i64>(query)
                .fetch_one(&f.pool)
                .await
                .unwrap(),
            expected,
            "{query}"
        );
    }
    assert!(
        continuations::check_source_guard(&f.pool, &op.source_run_id)
            .await
            .is_err()
    );
    let activation = service.call("runs.continuation_activate", &principal, json!({
        "operation_id":restored.operation_id, "expected_version":restored.version,
        "expected_manifest_sha256":ContentDigest::from_sha256_hex(restored.manifest_sha256.as_deref().unwrap()).unwrap(),
        "caller_request_id":Uuid::new_v4()
    })).await.unwrap();
    assert_eq!(activation["status"], "accepted", "{activation:#}");
    let active = continuations::find(&f.pool, &op.operation_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(active.phase, "activated");
    let successor = RunId(Uuid::parse_str(active.successor_run_id.as_deref().unwrap()).unwrap());
    assert!(db::repos::run_continuation_inputs::execution_seed(
        &f.pool,
        successor,
        "proposal_current"
    )
    .await
    .unwrap()
    .is_some());
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM runs")
            .fetch_one(&f.pool)
            .await
            .unwrap(),
        2
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM work_items WHERE kind='advance_run' AND run_id=?"
        )
        .bind(successor.to_string())
        .fetch_one(&f.pool)
        .await
        .unwrap(),
        1
    );
}

struct FailAfterCompleteManifest;

#[async_trait]
impl PreparationObserver for FailAfterCompleteManifest {
    async fn before_effect(&self, label: &str) -> Result<()> {
        anyhow::ensure!(
            label != "finalizer:returned",
            "fixture finalizer acknowledgement failed"
        );
        Ok(())
    }
}

#[tokio::test]
async fn service_reconcile_complete_manifest_with_unknown_step_stays_held_without_replay() {
    let f = Fixture::new().await.with_rollout_seed().await;
    let plan = f.plan().await;
    let op = f.reserve(&plan).await;
    let lane = PreparationLane::new(f.pool.clone());
    let error = lane
        .try_submit(
            f.job(Uuid::parse_str(&op.operation_id).unwrap(), plan)
                .with_observer(Arc::new(FailAfterCompleteManifest)),
        )
        .unwrap()
        .wait()
        .await
        .unwrap_err();
    assert!(error
        .to_string()
        .contains("fixture finalizer acknowledgement failed"));
    lane.shutdown().await.unwrap();
    let held = continuations::find(&f.pool, &op.operation_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(held.phase, "needs_reconciliation");
    assert!(
        !held.worker_claimed,
        "tracked synchronous failure is settled, its effect is not verified"
    );
    assert!(held.manifest_ref.is_none());
    let root = f.owned.join(&op.operation_id);
    assert!(root.join("manifest.json").is_file());
    let before = private_file_digests(&root);
    let (service, principal) = f.service();
    let result = service.call("runs.continuation_reconcile", &principal, json!({
        "operation_id":held.operation_id, "expected_version":held.version,
        "caller_request_id":Uuid::new_v4(), "reason":"Inspect complete file with unknown intent outcome"
    })).await.unwrap();
    assert_eq!(result["status"], "accepted", "{result:#}");
    let still_held = continuations::find(&f.pool, &op.operation_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(still_held.phase, "needs_reconciliation");
    assert!(still_held.holds_json.contains("effect_outcome_unknown"));
    assert!(!still_held.worker_claimed);
    assert!(still_held.manifest_ref.is_none());
    assert!(still_held.successor_run_id.is_none());
    assert_eq!(private_file_digests(&root), before);
    assert_eq!(sqlx::query_scalar::<_, String>("SELECT status FROM run_continuation_steps WHERE operation_id=? AND step_key='selected_inputs_and_target'")
        .bind(&op.operation_id).fetch_one(&f.pool).await.unwrap(), "unknown");
    for (query, expected) in [
        ("SELECT COUNT(*) FROM runs", 1),
        ("SELECT COUNT(*) FROM run_continuation_inputs", 0),
        (
            "SELECT COUNT(*) FROM work_items WHERE kind='advance_run'",
            0,
        ),
        (
            "SELECT SUM(attempt_count) FROM work_items WHERE kind='prepare_run_continuation'",
            1,
        ),
    ] {
        assert_eq!(
            sqlx::query_scalar::<_, i64>(query)
                .fetch_one(&f.pool)
                .await
                .unwrap(),
            expected,
            "{query}"
        );
    }
    assert!(
        continuations::check_source_guard(&f.pool, &op.source_run_id)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn service_tamper_denies_activation_and_reconciles_to_hold_without_successor() {
    let f = Fixture::new().await.with_rollout_seed().await;
    let (service, principal, _, _, op) = f.service_prepare().await;
    let manifest = read_manifest(&op).await.unwrap();
    fs::write(
        manifest.workspace.checkout_root.join("product.txt"),
        "changed after prepared",
    )
    .unwrap();
    let denial = service.call("runs.continuation_activate", &principal, json!({"operation_id":op.operation_id,"expected_version":op.version,
        "expected_manifest_sha256":ContentDigest::from_sha256_hex(op.manifest_sha256.as_deref().unwrap()).unwrap(),"caller_request_id":Uuid::new_v4()})).await.unwrap();
    assert_eq!(denial["status"], "denied", "{denial:#}");
    assert_eq!(denial["denial"]["code"], "prepared_material_changed");
    let current = continuations::find(&f.pool, &op.operation_id)
        .await
        .unwrap()
        .unwrap();
    service.call("runs.continuation_reconcile", &principal, json!({"operation_id":op.operation_id,"expected_version":current.version,"caller_request_id":Uuid::new_v4(),"reason":"Inspect changed prepared material"})).await.unwrap();
    let held = continuations::find(&f.pool, &op.operation_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(held.phase, "needs_reconciliation");
    assert!(held.holds_json.contains("prepared_material_changed"));
    assert!(held.successor_run_id.is_none());
    assert!(
        continuations::check_source_guard(&f.pool, &op.source_run_id)
            .await
            .is_err()
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM runs")
            .fetch_one(&f.pool)
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM run_continuation_inputs")
            .fetch_one(&f.pool)
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM work_items WHERE kind='advance_run'")
            .fetch_one(&f.pool)
            .await
            .unwrap(),
        0
    );
}
