//! Trusted-project workspace preparation. The daemon does not admit this route
//! until the common coordinator and pre-prompt integration are installed.

use std::{
    collections::HashMap,
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};

use anyhow::{bail, ensure, Result};
use domain::{xcode_contract as contract, xcode_effect::*};
use serde_json::{json, Value};
use tokio::{
    sync::{oneshot, Mutex, Semaphore},
    time::{timeout, timeout_at, Instant},
};
use uuid::Uuid;

use crate::{
    xcode_headless_host::{
        HeadlessHostInspector, HeadlessServiceStartupPlan, HostSnapshot, TrustedProject,
    },
    XcodeDispatchDecision, XcodeEffectJournal,
};

#[async_trait::async_trait]
pub trait HeadlessPeer: Send {
    async fn initialize(&mut self) -> Result<()>;
    async fn open(&mut self, project_path: &str) -> Result<Value>;
    async fn read(&mut self, arguments: Value) -> Result<Value>;
}

#[async_trait::async_trait]
pub trait HeadlessPeerFactory: Send + Sync {
    async fn connect(
        &self,
        host: &HostSnapshot,
        deadline: Instant,
    ) -> Result<Box<dyn HeadlessPeer>>;
}

#[derive(Clone)]
pub struct WorkspaceOpenRequest {
    pub run_id: Uuid,
    pub owner_lineage: String,
    pub invocation_id: Uuid,
    pub operation_key: Uuid,
    pub permission_policy_digest: String,
    pub trust_policy_id: String,
    pub timeout: Duration,
}

const MAX_PREPARATIONS: usize = 8;
const SETTLEMENT_TIMEOUT: Duration = Duration::from_secs(10);
const READ_TIMEOUT: Duration = Duration::from_secs(60);

pub struct HeadlessWorkspaceController {
    host: Arc<dyn HeadlessHostInspector>,
    peers: Arc<dyn HeadlessPeerFactory>,
    slots: Arc<Semaphore>,
    closing: Arc<AtomicBool>,
    access: Arc<Mutex<()>>,
    mappings: Arc<Mutex<HashMap<(String, String), ObservedMapping>>>,
}

struct ObservedMapping {
    project: ProjectKey,
    valid: Arc<AtomicBool>,
}

/// Not a provider ticket or a serializable permission grant. The shared
/// coordinator/session commit layer must adopt it before exposing an endpoint.
pub struct PreparedWorkspace {
    host: Arc<dyn HeadlessHostInspector>,
    peer: Box<dyn HeadlessPeer>,
    project: Arc<TrustedProject>,
    snapshot: HostSnapshot,
    alias: String,
    upstream_id: String,
    binding_digest: String,
    deadline: Instant,
    closing: Arc<AtomicBool>,
    access: Arc<Mutex<()>>,
    revoked: bool,
    mapping_valid: Arc<AtomicBool>,
}

struct CancelOnDrop(Arc<AtomicBool>);

struct RevokeOnDrop(Option<Arc<AtomicBool>>);

impl Drop for RevokeOnDrop {
    fn drop(&mut self) {
        if let Some(valid) = &self.0 {
            valid.store(false, Ordering::Release);
        }
    }
}

impl Drop for CancelOnDrop {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Release);
    }
}

impl HeadlessWorkspaceController {
    pub async fn prepare_reusing_verified_mapping(
        &self,
        project: Arc<TrustedProject>,
        request: WorkspaceOpenRequest,
        journal: Arc<dyn XcodeEffectJournal>,
    ) -> Result<PreparedWorkspace> {
        self.prepare_inner(project, request, journal, true, None)
            .await
    }
    pub async fn prepare_for_invocation(
        &self,
        project: Arc<TrustedProject>,
        request: WorkspaceOpenRequest,
        journal: Arc<dyn XcodeEffectJournal>,
        binding_deadline: Instant,
    ) -> Result<PreparedWorkspace> {
        self.prepare_inner(project, request, journal, true, Some(binding_deadline))
            .await
    }
    pub fn new(host: Arc<dyn HeadlessHostInspector>, peers: Arc<dyn HeadlessPeerFactory>) -> Self {
        Self {
            host,
            peers,
            slots: Arc::new(Semaphore::new(MAX_PREPARATIONS)),
            closing: Arc::new(AtomicBool::new(false)),
            access: Arc::new(Mutex::new(())),
            mappings: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn prepare(
        &self,
        project: Arc<TrustedProject>,
        request: WorkspaceOpenRequest,
        journal: Arc<dyn XcodeEffectJournal>,
    ) -> Result<PreparedWorkspace> {
        self.prepare_inner(project, request, journal, false, None)
            .await
    }

    async fn prepare_inner(
        &self,
        project: Arc<TrustedProject>,
        request: WorkspaceOpenRequest,
        journal: Arc<dyn XcodeEffectJournal>,
        reuse_mapping: bool,
        binding_deadline: Option<Instant>,
    ) -> Result<PreparedWorkspace> {
        ensure!(
            !request.timeout.is_zero() && request.timeout <= Duration::from_secs(12 * 60),
            "headless_deadline_invalid"
        );
        ensure!(
            request.trust_policy_id == contract::TRUST_POLICY_ID,
            "headless_project_trust_required"
        );
        validate_digest(
            "permission_policy_digest",
            &request.permission_policy_digest,
        )?;
        ensure!(
            !self.closing.load(Ordering::Acquire),
            "headless_runtime_closed"
        );
        let preparation_deadline = Instant::now() + request.timeout;
        let binding_deadline = binding_deadline.unwrap_or(preparation_deadline);
        ensure!(
            binding_deadline > Instant::now()
                && binding_deadline <= Instant::now() + Duration::from_secs(24 * 60 * 60),
            "headless_deadline_invalid"
        );
        let deadline = preparation_deadline.min(binding_deadline);
        let slot = self
            .slots
            .clone()
            .try_acquire_owned()
            .map_err(|_| anyhow::anyhow!("headless_capacity_exhausted"))?;
        ensure!(
            !self.closing.load(Ordering::Acquire),
            "headless_runtime_closed"
        );
        let cancelled = Arc::new(AtomicBool::new(false));
        let _cancellation = CancelOnDrop(cancelled.clone());
        let (sender, receiver) = oneshot::channel();
        let driver = Driver {
            reuse_mapping,
            host: self.host.clone(),
            peers: self.peers.clone(),
            project,
            request,
            journal,
            deadline,
            binding_deadline,
            cancelled,
            closing: self.closing.clone(),
            access: self.access.clone(),
            mappings: self.mappings.clone(),
        };
        // The bounded driver owns the effect through outcome persistence even if
        // its caller drops the receiver after sending bytes to Apple.
        tokio::spawn(async move {
            let result = driver.run().await;
            let _ = sender.send(result);
            drop(slot);
        });
        receiver
            .await
            .map_err(|_| anyhow::anyhow!("headless_preparation_interrupted"))?
    }

    pub async fn shutdown(&self) {
        self.closing.store(true, Ordering::Release);
        // Acquiring every slot joins all admitted drivers, not idle bindings.
        let _ = self.slots.acquire_many(MAX_PREPARATIONS as u32).await;
    }
}

impl PreparedWorkspace {
    pub async fn revalidate(&mut self) -> Result<()> {
        ensure!(
            !self.revoked && self.mapping_valid.load(Ordering::Acquire),
            "headless_binding_revoked"
        );
        self.revoked = true;
        let result = timeout_at(self.deadline.min(Instant::now() + READ_TIMEOUT), async {
            let _access = self.access.lock().await;
            let mut revocation = RevokeOnDrop(Some(self.mapping_valid.clone()));
            ensure!(
                self.mapping_valid.load(Ordering::Acquire) && !self.closing.load(Ordering::Acquire),
                "headless_binding_revoked"
            );
            validate_current(&*self.host, &self.project, &self.snapshot, true).await?;
            ensure!(
                self.mapping_valid.load(Ordering::Acquire) && !self.closing.load(Ordering::Acquire),
                "headless_binding_revoked"
            );
            revocation.0 = None;
            Ok::<_, anyhow::Error>(())
        })
        .await
        .map_err(|_| anyhow::anyhow!("headless_binding_timeout"))?;
        if result.is_ok() {
            self.revoked = false;
        }
        result
    }

    pub fn host_snapshot(&self) -> &HostSnapshot {
        &self.snapshot
    }
    pub fn alias(&self) -> &str {
        &self.alias
    }

    pub fn binding_digest(&self) -> &str {
        &self.binding_digest
    }

    pub async fn read(&mut self, arguments: Value) -> Result<Value> {
        ensure!(
            !self.revoked
                && self.mapping_valid.load(Ordering::Acquire)
                && !self.closing.load(Ordering::Acquire),
            "headless_binding_revoked"
        );
        let arguments = contract::ReadArguments::parse(&arguments, &self.alias)?;
        // Cancellation or an invalid response must not leave a reusable peer.
        self.revoked = true;
        let result = timeout_at(self.deadline.min(Instant::now() + READ_TIMEOUT), async {
            let _access = self.access.lock().await;
            // Queued abandonment is local. Once inspection starts, publish any
            // loss of continuity before releasing access to another binding.
            let mut revocation = RevokeOnDrop(Some(self.mapping_valid.clone()));
            ensure!(
                self.mapping_valid.load(Ordering::Acquire) && !self.closing.load(Ordering::Acquire),
                "headless_binding_revoked"
            );
            validate_current(&*self.host, &self.project, &self.snapshot, true).await?;
            ensure!(
                self.mapping_valid.load(Ordering::Acquire) && !self.closing.load(Ordering::Acquire),
                "headless_binding_revoked"
            );
            let response = self
                .peer
                .read(arguments.upstream_arguments(&self.upstream_id))
                .await?;
            validate_current(&*self.host, &self.project, &self.snapshot, true).await?;
            ensure!(
                self.mapping_valid.load(Ordering::Acquire) && !self.closing.load(Ordering::Acquire),
                "headless_binding_revoked"
            );
            let result = contract::decode_read(&response, &arguments)?;
            revocation.0 = None;
            Ok::<_, anyhow::Error>(result)
        })
        .await
        .map_err(|_| anyhow::anyhow!("headless_read_timeout"))?;
        if result.is_ok() {
            self.revoked = false;
        }
        result
    }
}

struct Driver {
    reuse_mapping: bool,
    host: Arc<dyn HeadlessHostInspector>,
    peers: Arc<dyn HeadlessPeerFactory>,
    project: Arc<TrustedProject>,
    request: WorkspaceOpenRequest,
    journal: Arc<dyn XcodeEffectJournal>,
    deadline: Instant,
    binding_deadline: Instant,
    cancelled: Arc<AtomicBool>,
    closing: Arc<AtomicBool>,
    access: Arc<Mutex<()>>,
    mappings: Arc<Mutex<HashMap<(String, String), ObservedMapping>>>,
}

enum ServicePreparation {
    Warm(HostSnapshot, Box<dyn HeadlessPeer>),
    Cold(HeadlessServiceStartupPlan),
}

impl Driver {
    fn require_active(&self) -> Result<()> {
        ensure!(
            !self.cancelled.load(Ordering::Acquire) && !self.closing.load(Ordering::Acquire),
            "headless_preparation_cancelled"
        );
        ensure!(
            Instant::now() < self.deadline,
            "headless_preparation_timeout"
        );
        Ok(())
    }

    async fn run(self) -> Result<PreparedWorkspace> {
        let _access = timeout_at(self.deadline, self.access.lock())
            .await
            .map_err(|_| anyhow::anyhow!("headless_preparation_timeout"))?;
        self.require_active()?;
        self.project.revalidate()?;
        let service = timeout_at(self.deadline, async {
            if let Some(plan) = self.host.prepare_service_startup().await? {
                ensure!(
                    plan.installation.uid == self.project.key().uid,
                    "headless_host_uid_changed"
                );
                for mapping in self.mappings.lock().await.values() {
                    mapping.valid.store(false, Ordering::Release);
                }
                return Ok(ServicePreparation::Cold(plan));
            }
            let snapshot = self.host.inspect().await?;
            ensure!(
                snapshot.generation.uid == self.project.key().uid,
                "headless_host_uid_changed"
            );
            let mut peer = self.peers.connect(&snapshot, self.binding_deadline).await?;
            peer.initialize().await?;
            validate_current(&*self.host, &self.project, &snapshot, false).await?;
            Ok::<_, anyhow::Error>(ServicePreparation::Warm(snapshot, peer))
        })
        .await
        .map_err(|_| anyhow::anyhow!("headless_preparation_timeout"))??;
        self.require_active()?;
        let installation = match &service {
            ServicePreparation::Warm(snapshot, _) => &snapshot.generation,
            ServicePreparation::Cold(plan) => &plan.installation,
        };
        let cold = matches!(&service, ServicePreparation::Cold(_));
        let mut target = json!({
            "root": self.project.root(), "project": self.project.key(),
            "generation": installation, "trust_policy_id": self.request.trust_policy_id,
            "trust_policy_digest": contract::trust_policy_digest()?,
            "permission_policy_digest": self.request.permission_policy_digest,
            "manifest_digest": contract::manifest_digest()?,
        });
        let generation = contract::canonical_digest(
            "cw.xcode.binding.v1",
            &serde_json::to_value(installation)?,
        )?;
        if self.reuse_mapping && !cold {
            let cached = self
                .mappings
                .lock()
                .await
                .iter()
                .find(|((observed, _), mapping)| {
                    observed == &generation && &mapping.project == self.project.key()
                })
                .map(|((_, id), mapping)| (id.clone(), mapping.valid.clone()));
            if let Some((upstream_id, mapping_valid)) = cached {
                let ServicePreparation::Warm(snapshot, peer) = service else {
                    unreachable!()
                };
                let mut revocation = RevokeOnDrop(Some(mapping_valid.clone()));
                ensure!(
                    mapping_valid.load(Ordering::Acquire),
                    "headless_binding_revoked"
                );
                timeout_at(
                    self.deadline,
                    validate_current(&*self.host, &self.project, &snapshot, true),
                )
                .await
                .map_err(|_| anyhow::anyhow!("headless_preparation_timeout"))??;
                self.require_active()?;
                let binding_digest = contract::canonical_digest(
                    "cw.xcode.binding.v1",
                    &json!({"target":target,"workspace_identifier":upstream_id}),
                )?;
                revocation.0 = None;
                return Ok(PreparedWorkspace {
                    host: self.host.clone(),
                    peer,
                    project: self.project.clone(),
                    snapshot,
                    alias: format!("cw-workspace-{binding_digest}"),
                    upstream_id,
                    binding_digest,
                    deadline: self.binding_deadline,
                    closing: self.closing.clone(),
                    access: self.access.clone(),
                    revoked: false,
                    mapping_valid,
                });
            }
        }
        let intent = NormalizedIntent {
            run_id: self.request.run_id,
            owner_lineage: self.request.owner_lineage.clone(),
            invocation_id: self.request.invocation_id,
            project_key: self.project.key().clone(),
            operation_key: self.request.operation_key,
            operation: "workspace_open".into(),
            target_digest: contract::canonical_digest("cw.xcode.binding.v1", &target)?,
            binding_digest: None,
            request_digest: contract::canonical_digest(
                "cw.xcode.operation.v1",
                &json!({"operation":"workspace_open", "path":self.project.key().canonical_path}),
            )?,
            origin: EffectOrigin::Lifecycle,
            effect_class: EffectClass::Mutate,
            result_schema: contract::open_schema_identity()?,
        };
        let prepared = timeout_at(self.deadline, self.journal.prepare(&intent))
            .await
            .map_err(|_| anyhow::anyhow!("headless_journal_timeout"))??;
        ensure!(
            prepared.state == AttemptState::Prepared,
            "headless_open_already_recorded"
        );
        if self.require_active().is_err() {
            let payload = HistoricalPayload::Error {
                code: EffectErrorCode::CancelledBeforeDispatch,
                message: RedactedText::new("Workspace preparation cancelled before dispatch")?,
            };
            let outcome = HistoricalResult {
                schema_version: 1,
                schema: intent.result_schema.clone(),
                result_digest: contract::canonical_digest(
                    "cw.xcode.operation.v1",
                    &serde_json::to_value(&payload)?,
                )?,
                payload,
            };
            timeout(
                SETTLEMENT_TIMEOUT,
                self.journal.cancel(revision(&prepared), &outcome),
            )
            .await
            .map_err(|_| anyhow::anyhow!("headless_journal_timeout"))??;
            bail!("headless_preparation_cancelled");
        }
        // A lost journal acknowledgement may conceal a committed hold. Keep
        // earlier bindings unusable unless dispatch and settlement both finish.
        let mut dispatch_revocations: Vec<_> = self
            .mappings
            .lock()
            .await
            .iter()
            .filter(|((existing, _), _)| cold || existing == &generation)
            .map(|(_, mapping)| RevokeOnDrop(Some(mapping.valid.clone())))
            .collect();
        let dispatched = match timeout_at(
            self.deadline,
            self.journal
                .dispatch(prepared.nonce, &intent.request_digest, prepared.revision),
        )
        .await
        .map_err(|_| anyhow::anyhow!("headless_journal_timeout"))??
        {
            XcodeDispatchDecision::Dispatch(attempt) => attempt,
            XcodeDispatchDecision::Existing(_) => bail!("headless_open_already_recorded"),
        };
        let opened = timeout_at(self.deadline, async {
            self.require_active()?;
            let (snapshot, mut peer) = match service {
                ServicePreparation::Warm(snapshot, peer) => (snapshot, peer),
                ServicePreparation::Cold(plan) => {
                    let installation = plan.installation.clone();
                    self.host.start_service_after_fence(plan).await?;
                    self.require_active()?;
                    let snapshot = self.host.inspect().await?;
                    ensure!(
                        snapshot.generation.uid == self.project.key().uid,
                        "headless_host_uid_changed"
                    );
                    let mut observed_installation = snapshot.generation.clone();
                    observed_installation.pid = 0;
                    observed_installation.start_sec = 0;
                    observed_installation.start_usec = 0;
                    ensure!(
                        observed_installation == installation,
                        "headless_startup_installation_changed"
                    );
                    let mut peer = self.peers.connect(&snapshot, self.binding_deadline).await?;
                    peer.initialize().await?;
                    (snapshot, peer)
                }
            };
            let generation = contract::canonical_digest(
                "cw.xcode.binding.v1",
                &serde_json::to_value(&snapshot.generation)?,
            )?;
            validate_current(&*self.host, &self.project, &snapshot, false).await?;
            self.require_active()?;
            let response = peer.open(&self.project.key().canonical_path).await?;
            let opened = contract::decode_open(&response)?;
            self.project.revalidate()?;
            ensure!(
                Path::new(&opened.workspace_path).is_absolute(),
                "headless_workspace_mapping_mismatch"
            );
            ensure!(
                Path::new(&opened.workspace_path).canonicalize()?
                    == Path::new(&self.project.key().canonical_path),
                "headless_workspace_mapping_mismatch"
            );
            validate_current(&*self.host, &self.project, &snapshot, true).await?;
            let mut mappings = self.mappings.lock().await;
            mappings.retain(|(existing, _), mapping| {
                if existing != &generation {
                    mapping.valid.store(false, Ordering::Release);
                }
                existing == &generation
            });
            let key = (generation.clone(), opened.workspace_identifier.clone());
            let mapping_valid = if let Some(mapping) = mappings.get(&key) {
                if &mapping.project != self.project.key() {
                    mapping.valid.store(false, Ordering::Release);
                    bail!("headless_workspace_identifier_reassigned");
                }
                ensure!(
                    mapping.valid.load(Ordering::Acquire),
                    "headless_workspace_identifier_reassigned"
                );
                mapping.valid.clone()
            } else {
                ensure!(mappings.len() < 4096, "headless_mapping_capacity");
                let valid = Arc::new(AtomicBool::new(true));
                mappings.insert(
                    key,
                    ObservedMapping {
                        project: self.project.key().clone(),
                        valid: valid.clone(),
                    },
                );
                valid
            };
            Ok::<_, anyhow::Error>((opened, mapping_valid, snapshot, peer))
        })
        .await
        .unwrap_or_else(|_| Err(anyhow::anyhow!("headless_open_timeout")));
        let (opened, mapping_valid, snapshot, peer) = match opened {
            Ok(opened) => opened,
            Err(_) => {
                // Once an open may have reached Apple, an uncertain response can
                // conceal reassignment of any observed identifier in this service.
                for ((existing, _), mapping) in self.mappings.lock().await.iter() {
                    if cold || existing == &generation {
                        mapping.valid.store(false, Ordering::Release);
                    }
                }
                let reason = if self.cancelled.load(Ordering::Acquire)
                    || self.closing.load(Ordering::Acquire)
                {
                    UncertaintyReason::CancelledAfterDispatch
                } else {
                    UncertaintyReason::TransportLost
                };
                timeout(
                    SETTLEMENT_TIMEOUT,
                    self.journal
                        .complete(revision(&dispatched), &Completion::Unknown { reason }),
                )
                .await
                .map_err(|_| anyhow::anyhow!("headless_journal_timeout"))??;
                bail!("headless_open_outcome_unknown");
            }
        };
        dispatch_revocations.push(RevokeOnDrop(Some(mapping_valid.clone())));
        target["generation"] = serde_json::to_value(&snapshot.generation)?;
        let result_digest = contract::canonical_digest(
            "cw.xcode.operation.v1",
            &json!({
                "workspaceIdentifier":opened.workspace_identifier, "workspacePath": self.project.key().canonical_path,
            }),
        )?;
        let completion = Completion::Succeeded {
            outcome: HistoricalResult {
                schema_version: 1,
                schema: intent.result_schema.clone(),
                result_digest: result_digest.clone(),
                payload: HistoricalPayload::Result {
                    summary: RedactedText::new(
                        "Workspace open returned the exact selected project mapping",
                    )?,
                },
            },
            // This adapter settles synchronous workspace opening only, not the
            // indexing/build activity that a trusted Apple project may trigger.
            proof: SettlementProof {
                evidence_ref: format!("xcode/open-response/{result_digest}"),
                active_operation_check_ref: format!(
                    "xcode/synchronous-open/{}",
                    dispatched.attempt_id
                ),
                no_operation_in_flight: true,
            },
        };
        timeout(
            SETTLEMENT_TIMEOUT,
            self.journal.complete(revision(&dispatched), &completion),
        )
        .await
        .map_err(|_| anyhow::anyhow!("headless_journal_timeout"))??;
        let binding_digest = contract::canonical_digest(
            "cw.xcode.binding.v1",
            &json!({
                "target":target, "workspace_identifier":opened.workspace_identifier,
            }),
        )?;
        for revocation in &mut dispatch_revocations {
            revocation.0 = None;
        }
        Ok(PreparedWorkspace {
            host: self.host.clone(),
            peer,
            project: self.project.clone(),
            snapshot,
            alias: format!("cw-workspace-{binding_digest}"),
            upstream_id: opened.workspace_identifier,
            binding_digest,
            deadline: self.binding_deadline,
            closing: self.closing.clone(),
            access: self.access.clone(),
            revoked: false,
            mapping_valid,
        })
    }
}

fn revision(attempt: &StoredAttempt) -> AttemptRevision {
    AttemptRevision {
        attempt_id: attempt.attempt_id,
        revision: attempt.revision,
    }
}

async fn validate_current(
    host: &dyn HeadlessHostInspector,
    project: &TrustedProject,
    expected: &HostSnapshot,
    require_open: bool,
) -> Result<()> {
    project.revalidate()?;
    let current = host.inspect().await?;
    ensure!(
        current.generation == expected.generation && current.generation.uid == project.key().uid,
        "headless_service_generation_changed"
    );
    if require_open {
        ensure!(
            current
                .open_projects
                .iter()
                .filter(|path| path.as_path() == Path::new(&project.key().canonical_path))
                .count()
                == 1,
            "headless_workspace_stale"
        );
    }
    Ok(())
}
