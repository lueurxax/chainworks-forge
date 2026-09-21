//! In-process, provider-free preparation ownership. Startup never reconstructs jobs.

use anyhow::{ensure, Context, Result};
use async_trait::async_trait;
use db::repos::run_continuations::{self as continuations, Continuation};
use domain::run_carry_forward::ContentDigest;
use sqlx::SqlitePool;
use std::{
    collections::HashSet,
    io::Read,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};
use tokio::{
    sync::{mpsc, oneshot, OwnedSemaphorePermit, Semaphore},
    task::JoinHandle,
};
use uuid::Uuid;

use super::{
    budget::Deadline,
    materialize::{materialize_with_context, publish_checked},
    safe_files::{identity, SafeRoot},
    MaterializationReceiptV1, WorkspacePreview,
};

pub struct PreparedManifest {
    pub manifest_ref: PathBuf,
    pub manifest_sha256: ContentDigest,
}

/// Trusted server composition only. Implementations must copy/verify selected inputs,
/// freeze the compiled target and publish the COMPLETE manifest, never an I1 receipt.
/// Runs on the blocking lane. Check context before each effect and after each wait;
/// do not spawn untracked tasks, subprocesses or provider work. Errors retain evidence.
#[async_trait]
pub trait PreparationFinalizer: Send + Sync {
    async fn finalize(
        &self,
        context: &PreparationContext,
        workspace: &MaterializationReceiptV1,
    ) -> Result<PreparedManifest>;
}

/// Optional progress/backpressure observer. Returning or failing never grants authority:
/// the worker rechecks generation and deadline after every observer wait.
#[async_trait]
pub trait PreparationObserver: Send + Sync {
    async fn before_effect(&self, label: &str) -> Result<()>;
}

pub struct PreparationJob {
    operation_id: Uuid,
    preview: Arc<WorkspacePreview>,
    owned_parent: PathBuf,
    finalizer: Arc<dyn PreparationFinalizer>,
    observer: Option<Arc<dyn PreparationObserver>>,
}

impl PreparationJob {
    pub fn new(
        operation_id: Uuid,
        preview: Arc<WorkspacePreview>,
        owned_parent: PathBuf,
        finalizer: Arc<dyn PreparationFinalizer>,
    ) -> Self {
        Self {
            operation_id,
            preview,
            owned_parent,
            finalizer,
            observer: None,
        }
    }

    pub fn with_observer(mut self, observer: Arc<dyn PreparationObserver>) -> Self {
        self.observer = Some(observer);
        self
    }
}

pub struct PreparationContext {
    pool: SqlitePool,
    operation: Continuation,
    deadline: Deadline,
    cancelled: Arc<AtomicBool>,
    lane_stopped: Arc<AtomicBool>,
    operation_root: PathBuf,
    observer: Option<Arc<dyn PreparationObserver>>,
}

impl PreparationContext {
    pub fn operation(&self) -> &Continuation {
        &self.operation
    }

    pub fn remaining(&self) -> Result<Duration> {
        self.deadline.remaining()
    }

    pub async fn check_current(&self) -> Result<()> {
        self.deadline.check()?;
        self.check_cancelled()?;
        let current = continuations::find(&self.pool, &self.operation.operation_id)
            .await?
            .context("continuation_stale_worker")?;
        ensure!(
            current.phase == "preparing"
                && current.generation == self.operation.generation
                && current.worker_claimed,
            "continuation_stale_worker"
        );
        ensure!(
            chrono::DateTime::parse_from_rfc3339(&current.deadline_at)? > chrono::Utc::now(),
            "continuation_budget_exceeded"
        );
        self.deadline.check()?;
        self.check_cancelled()?;
        Ok(())
    }

    fn check_cancelled(&self) -> Result<()> {
        ensure!(
            !self.cancelled.load(Ordering::Acquire) && !self.lane_stopped.load(Ordering::Acquire),
            "continuation_worker_cancelled"
        );
        Ok(())
    }

    pub async fn checkpoint(&self, label: &str) -> Result<()> {
        self.check_current().await?;
        if let Some(observer) = &self.observer {
            observer.before_effect(label).await?;
        }
        self.check_current().await
    }

    pub async fn begin_step(
        &self,
        key: &str,
        intent: &serde_json::Value,
        destination: &Path,
    ) -> Result<()> {
        self.check_current().await?;
        continuations::begin_step(
            &self.pool,
            &self.operation.operation_id,
            self.operation.generation,
            key,
            &serde_json::to_string(intent)?,
            destination
                .to_str()
                .context("unsafe_path: non-UTF8 destination")?,
        )
        .await?;
        self.check_current().await
    }

    pub async fn finish_step(&self, key: &str, receipt: &ContentDigest) -> Result<()> {
        self.check_current().await?;
        continuations::finish_step(
            &self.pool,
            &self.operation.operation_id,
            self.operation.generation,
            key,
            hex_digest(receipt),
        )
        .await?;
        self.check_current().await
    }

    /// Exclusive, no-follow publication under an already journalled finalizer intent.
    /// The root must be an existing operation-owned directory, not a caller-supplied root.
    pub async fn publish_file(&self, root: &Path, relative: &str, bytes: &[u8]) -> Result<()> {
        ensure!(
            bytes.len() <= 256 * 1024 * 1024,
            "continuation_budget_exceeded: file"
        );
        ensure!(
            root.starts_with(&self.operation_root),
            "unsafe_path: finalizer destination"
        );
        let directory = SafeRoot::open(root)?;
        publish_checked(&directory, root, relative, bytes, self.deadline, Some(self)).await
    }
}

fn hex_digest(digest: &ContentDigest) -> &str {
    // ContentDigest validates the prefix; SQLite's v1 digest columns store bare hex.
    digest
        .as_str()
        .strip_prefix("sha256:")
        .expect("ContentDigest invariant")
}

struct Queued {
    job: PreparationJob,
    reply: oneshot::Sender<Result<Continuation>>,
    cancelled: Arc<AtomicBool>,
    _slot: OwnedSemaphorePermit,
}

/// In-process admission reserved before creating a durable operation/fence.
/// Dropping an unused permit releases capacity; only its originating lane can consume it.
pub struct PreparationPermit {
    slot: OwnedSemaphorePermit,
    lane: Arc<Semaphore>,
}

/// One worker plus three waiting slots. DB reservation independently enforces global
/// admission before fencing. Dropping a ticket does NOT cancel accepted durable work.
pub struct PreparationLane {
    sender: Option<mpsc::Sender<Queued>>,
    slots: Arc<Semaphore>,
    submitted: Arc<Mutex<HashSet<Uuid>>>,
    stop: Arc<AtomicBool>,
    task: Option<JoinHandle<()>>,
}

impl PreparationLane {
    pub fn new(pool: SqlitePool) -> Self {
        let (sender, mut receiver) = mpsc::channel::<Queued>(4);
        let slots = Arc::new(Semaphore::new(4));
        let submitted = Arc::new(Mutex::new(HashSet::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let stopped = stop.clone();
        let submitted_worker = submitted.clone();
        let task = tokio::spawn(async move {
            while let Some(queued) = receiver.recv().await {
                let id = queued.job.operation_id;
                let result = if stopped.load(Ordering::Acquire) {
                    hold_unclaimed_job(&pool, id)
                        .await
                        .and_then(|()| Err(anyhow::anyhow!("continuation_lane_stopped")))
                } else {
                    run_job(&pool, queued.job, queued.cancelled, stopped.clone()).await
                };
                submitted_worker.lock().expect("submitted lock").remove(&id);
                drop(queued._slot);
                let _ = queued.reply.send(result);
            }
        });
        Self {
            sender: Some(sender),
            slots,
            submitted,
            stop,
            task: Some(task),
        }
    }

    pub fn try_submit(&self, job: PreparationJob) -> Result<PreparationTicket> {
        self.try_submit_reserved(job, self.try_reserve()?)
    }

    pub fn try_reserve(&self) -> Result<PreparationPermit> {
        ensure!(
            !self.stop.load(Ordering::Acquire)
                && self
                    .sender
                    .as_ref()
                    .is_some_and(|sender| !sender.is_closed()),
            "continuation_capacity: lane stopped"
        );
        let slot = self
            .slots
            .clone()
            .try_acquire_owned()
            .map_err(|_| anyhow::anyhow!("continuation_capacity"))?;
        Ok(PreparationPermit {
            slot,
            lane: self.slots.clone(),
        })
    }

    pub fn try_submit_reserved(
        &self,
        job: PreparationJob,
        permit: PreparationPermit,
    ) -> Result<PreparationTicket> {
        ensure!(
            Arc::ptr_eq(&self.slots, &permit.lane),
            "invalid preparation lane permit"
        );
        ensure!(
            !self.stop.load(Ordering::Acquire),
            "continuation_lane_stopped"
        );
        let mut submitted = self.submitted.lock().expect("submitted lock");
        ensure!(
            submitted.insert(job.operation_id),
            "continuation_in_progress"
        );
        let id = job.operation_id;
        let (reply, result) = oneshot::channel();
        let cancelled = Arc::new(AtomicBool::new(false));
        let queued = Queued {
            job,
            reply,
            cancelled: cancelled.clone(),
            _slot: permit.slot,
        };
        if self
            .sender
            .as_ref()
            .context("continuation_lane_stopped")?
            .try_send(queued)
            .is_err()
        {
            submitted.remove(&id);
            anyhow::bail!("continuation_capacity");
        }
        Ok(PreparationTicket { result, cancelled })
    }

    /// Drain accepted work and join the owner. Never abort a spawn_blocking task or
    /// treat a timeout as proof that filesystem/Git children stopped.
    pub async fn shutdown(mut self) -> Result<()> {
        self.sender.take();
        if let Some(task) = self.task.take() {
            task.await.context("effect_outcome_unknown: worker task")?;
        }
        Ok(())
    }
}

impl Drop for PreparationLane {
    fn drop(&mut self) {
        // The actor retains and awaits its blocking handle. No abort-on-drop; DB
        // ownership survives a runtime/process crash for explicit reconciliation.
        self.stop.store(true, Ordering::Release);
    }
}

#[cfg(test)]
pub(super) async fn stop_lane_for_test(lane: &mut PreparationLane) {
    lane.sender.take();
    if let Some(task) = lane.task.take() {
        task.await.expect("test lane must settle");
    }
}

pub struct PreparationTicket {
    result: oneshot::Receiver<Result<Continuation>>,
    cancelled: Arc<AtomicBool>,
}

impl PreparationTicket {
    /// Cooperative stop only, NOT a durable abort or permission to release a fence.
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }

    pub async fn wait(self) -> Result<Continuation> {
        self.result
            .await
            .context("effect_outcome_unknown: preparation owner lost")?
    }
}

async fn hold_unclaimed_job(pool: &SqlitePool, id: Uuid) -> Result<()> {
    async {
        let id = id.to_string();
        let Some(operation) = continuations::find(pool, &id).await? else {
            return Ok(());
        };
        // Claim commit may have succeeded despite an acknowledgement error. Never
        // turn that uncertainty, another owner or a revoked generation into settlement.
        if operation.phase != "preparing" || operation.worker_claimed {
            return Ok(());
        }
        let reason = if chrono::DateTime::parse_from_rfc3339(&operation.deadline_at)?
            <= chrono::Utc::now()
        {
            "continuation_budget_exceeded"
        } else {
            "effect_outcome_unknown"
        };
        continuations::hold_queued_preparation(pool, &id, operation.generation, reason).await
    }
    .await
    .context("effect_outcome_unknown: could not record unclaimed preparation hold")
}

async fn run_job(
    pool: &SqlitePool,
    job: PreparationJob,
    cancelled: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
) -> Result<Continuation> {
    let operation =
        match continuations::claim_preparation_for(pool, &job.operation_id.to_string()).await {
            Ok(Some(operation)) => operation,
            result => {
                hold_unclaimed_job(pool, job.operation_id).await?;
                return Err(result.err().unwrap_or_else(|| {
                    anyhow::anyhow!("continuation_not_claimable: inspect operation; no retry")
                }));
            }
        };
    let claimed = operation.clone();
    let worker_pool = pool.clone();
    let runtime = tokio::runtime::Handle::current();
    // The entire synchronous file/hash path lives on this tracked blocking task,
    // not merely the Git awaits. Never timeout/drop its join handle.
    let result = tokio::task::spawn_blocking(move || {
        runtime.block_on(async move {
            let deadline = Deadline::from_stored(&operation.deadline_at)?;
            let reserved_at = chrono::DateTime::parse_from_rfc3339(&operation.created_at)?;
            let parent = std::fs::canonicalize(&job.owned_parent)?;
            let context = PreparationContext {
                pool: worker_pool,
                operation,
                deadline,
                cancelled,
                lane_stopped: stop,
                operation_root: parent.join(job.operation_id.to_string()),
                observer: job.observer,
            };
            let workspace = materialize_with_context(
                &job.preview,
                &parent,
                job.operation_id,
                deadline,
                Some(&context),
            )
            .await?;
            context.begin_step("selected_inputs_and_target", &serde_json::json!({
            "plan_ref":context.operation.plan_ref, "plan_sha256":context.operation.plan_sha256,
            "target_ref":context.operation.target_ref, "workspace_receipt":workspace,
            "requires":"verified selected inputs, compiled target, complete manifest",
        }), &workspace.operation_root).await?;
            context.checkpoint("finalizer:begin").await?;
            let manifest = job.finalizer.finalize(&context, &workspace).await?;
            context.checkpoint("finalizer:returned").await?;
            verify_manifest(&context, &workspace, &manifest).await?;
            context
                .finish_step("selected_inputs_and_target", &manifest.manifest_sha256)
                .await?;
            context.checkpoint("prepared:publish").await?;
            let preserved_bytes = verify_manifest(&context, &workspace, &manifest).await?;
            let prepared = continuations::mark_prepared(
                &context.pool,
                &context.operation.operation_id,
                context.operation.generation,
                manifest
                    .manifest_ref
                    .to_str()
                    .context("unsafe_path: manifest ref")?,
                hex_digest(&manifest.manifest_sha256),
            )
            .await?;
            db::metrics::record_continuation_phase_duration(
                domain::run_carry_forward_api::ContinuationPhase::Preparing,
                chrono::Utc::now()
                    .signed_duration_since(reserved_at)
                    .to_std()
                    .unwrap_or_default(),
            );
            db::metrics::record_continuation_preserved_bytes(preserved_bytes);
            Ok::<_, anyhow::Error>(prepared)
        })
    })
    .await;
    match result {
        Ok(Ok(prepared)) => Ok(prepared),
        Ok(Err(error)) => {
            // A returned file error proves synchronous I/O returned, but an unknown
            // Git result does not prove child/process-group settlement. Keep its claim.
            let unknown = error
                .chain()
                .any(|cause| cause.to_string().contains("effect_outcome_unknown"));
            if unknown {
                continuations::hold_preparation(
                    pool,
                    &claimed.operation_id,
                    claimed.generation,
                    "effect_outcome_unknown",
                )
                .await
                .context("effect_outcome_unknown: could not record preparation hold")?;
            } else {
                // CAS refuses old generations after startup and cannot overwrite prepared.
                if let Err(settlement) =
                    continuations::settle_worker(pool, &claimed.operation_id, claimed.generation)
                        .await
                {
                    tracing::warn!(error = %settlement, "preparation worker settlement held");
                }
            }
            Err(error)
        }
        Err(error) => {
            continuations::hold_preparation(
                pool,
                &claimed.operation_id,
                claimed.generation,
                "effect_outcome_unknown",
            )
            .await
            .context("effect_outcome_unknown: could not record failed-task hold")?;
            Err(anyhow::anyhow!(
                "effect_outcome_unknown: preparation task did not return: {error}"
            ))
        }
    }
}

async fn verify_manifest(
    context: &PreparationContext,
    workspace: &MaterializationReceiptV1,
    manifest: &PreparedManifest,
) -> Result<u64> {
    let relative = manifest
        .manifest_ref
        .strip_prefix(&workspace.operation_root)
        .context("unsafe_path: manifest outside operation")?
        .to_str()
        .context("unsafe_path: manifest path")?;
    let root = SafeRoot::open(&workspace.operation_root)?;
    let mut file = root.open_file(relative)?;
    let before = file.metadata()?;
    ensure!(
        before.len() <= 256 * 1024 * 1024,
        "continuation_budget_exceeded: manifest"
    );
    let mut bytes = Vec::new();
    let mut buffer = vec![0; 1024 * 1024];
    loop {
        context.check_current().await?;
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        ensure!(
            bytes.len() + n <= 256 * 1024 * 1024,
            "continuation_budget_exceeded: manifest"
        );
        bytes.extend_from_slice(&buffer[..n]);
    }
    context.check_current().await?;
    root.verify_path(&workspace.operation_root)?;
    ensure!(
        identity(&before) == identity(&file.metadata()?)
            && ContentDigest::of(&bytes) == manifest.manifest_sha256,
        "prepared_material_changed: final manifest"
    );
    let value: serde_json::Value = serde_json::from_slice(&bytes)?;
    ensure!(
        value.get("schema_version").and_then(|v| v.as_str())
            == Some("run_carry_forward_manifest_v1")
            && value.get("operation_id").and_then(|v| v.as_str())
                == Some(context.operation.operation_id.as_str()),
        "complete_manifest_required"
    );
    let preserved_bytes = value
        .get("preserved_bytes")
        .and_then(|v| v.as_u64())
        .context("complete_manifest_required: preserved_bytes")?;
    ensure!(
        preserved_bytes <= 2 * 1024 * 1024 * 1024,
        "continuation_budget_exceeded: preserved bytes"
    );
    context.check_current().await?;
    Ok(preserved_bytes)
}
