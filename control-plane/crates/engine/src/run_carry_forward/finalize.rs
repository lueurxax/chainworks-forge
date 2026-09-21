//! Trusted provider-free composition of an opaque, in-process preview.

use anyhow::{ensure, Context, Result};
use async_trait::async_trait;
use db::repos::{run_continuation_inputs::PreparedInput, run_continuations::Continuation};
use domain::{
    ids::{IdeaId, RunId},
    run::{DeliveryConfiguration, Run, RunStatus},
    run_carry_forward::{canonical_digest, ContentDigest, EntryRole},
};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    os::unix::fs::{MetadataExt, PermissionsExt},
    path::Path,
    sync::Arc,
};
use uuid::Uuid;

use super::{
    budget::Deadline,
    manifest::{self, FileBindingV1, InputProvenanceV1, ManifestV1, WorkspaceMaterialV1},
    materialize::copy_verified_input,
    preview::{CanonicalSourceInput, CarryForwardPlan, InputReference, PreparedPlan},
    safe_files::{validate_relative, SafeRoot},
    MaterializationReceiptV1, PreparationContext, PreparationFinalizer, PreparedManifest,
    WorkspacePreview,
};
use workflow::carry_forward::ContinuationTargetV1;

pub struct CarryForwardFinalizer {
    plan: CarryForwardPlan,
    workspace: Arc<WorkspacePreview>,
    target: ContinuationTargetV1,
    inputs: Vec<CanonicalSourceInput>,
}

impl CarryForwardFinalizer {
    /// Pure: no database or filesystem effects, and no caller-forged RunPlan constructor.
    pub fn new(prepared: PreparedPlan) -> Result<Self> {
        let (plan, workspace, target, inputs) = prepared.into_parts();
        ensure!(plan.summary.holds.is_empty(), "continuation_not_eligible");
        ensure!(
            canonical_digest(&target)? == plan.summary.target.compiled_plan_sha256,
            "source_changed: compiled target"
        );
        seed_path(&target)?;
        manifest::bounded_preservation_bytes(
            plan.entries.iter().map(|e| e.bytes),
            [
                workspace.index.len() as u64,
                workspace.staged.len() as u64,
                workspace.unstaged.len() as u64,
            ],
        )?;
        ensure!(
            plan.entries.len() <= 50_000
                && inputs.len() <= 129
                && inputs
                    .iter()
                    .filter(|i| i.role == EntryRole::ExecutionSeed)
                    .count()
                    == 1,
            "continuation_budget_exceeded: inventory"
        );
        Ok(Self {
            plan,
            workspace: Arc::new(workspace),
            target,
            inputs,
        })
    }

    pub fn workspace(&self) -> Arc<WorkspacePreview> {
        self.workspace.clone()
    }
}

#[async_trait]
impl PreparationFinalizer for CarryForwardFinalizer {
    async fn finalize(
        &self,
        context: &PreparationContext,
        receipt: &MaterializationReceiptV1,
    ) -> Result<PreparedManifest> {
        context.check_current().await?;
        let op = context.operation();
        let root_path = &receipt.operation_root;
        ensure!(
            receipt.operation_id.to_string() == op.operation_id
                && self.plan.summary.source_run_id.to_string() == op.source_run_id
                && manifest::db_digest(&op.plan_sha256)? == self.plan.plan_sha256
                && manifest::db_digest(&op.source_witness_sha256)?
                    == self.plan.summary.source_witness.semantic_sha256
                && receipt.source_witness == *self.workspace.digest()
                && receipt.checkout_root == root_path.join("checkout")
                && receipt.preservation_root == root_path.join("preservation"),
            "source_changed: reserved plan binding"
        );
        for (actual, name) in [(&op.plan_ref, "plan.json"), (&op.target_ref, "target.json")] {
            ensure!(
                actual == name || Path::new(actual) == root_path.join(name),
                "unsafe_path: reserved file reference"
            );
        }
        let root = SafeRoot::open(root_path)?;
        let metadata_path = root_path.join("metadata");
        context.begin_step("finalizer_metadata", &json!({"schema_version":"run_carry_forward_finalization_intent_v1",
            "plan_sha256":self.plan.plan_sha256, "target_sha256":self.plan.summary.target.compiled_plan_sha256,
            "source_inputs":self.inputs, "metadata_root":metadata_path, "files":["plan.json","target.json","workflow.snapshot.json","catalog.snapshot.json","manifest.json"]}), root_path).await?;
        context.checkpoint("finalizer:checkout_privacy").await?;
        root.verify_path(root_path)?;
        SafeRoot::open(&receipt.checkout_root)?
            .file
            .set_permissions(std::fs::Permissions::from_mode(0o700))?;
        context.checkpoint("finalizer:create_metadata").await?;
        root.verify_path(root_path)?;
        root.create_directory("metadata")?;
        let mut installed = Vec::new();
        let mut provenance = Vec::new();
        for (ordinal, input) in self.inputs.iter().enumerate() {
            context.check_current().await?;
            let source_directory = SafeRoot::open(input.source_root())?.file.metadata()?;
            ensure!(
                self.plan
                    .summary
                    .source_witness
                    .directory_identities
                    .get("metadata")
                    == Some(&(source_directory.dev(), source_directory.ino())),
                "source_changed: selected input root"
            );
            let input_id = Uuid::new_v4();
            let target_relative_path = match input.role {
                EntryRole::ExecutionSeed => {
                    ensure!(
                        input.target_logical_name == "proposal_current"
                            && input.source_contract == "proposal_current",
                        "artifact_provenance_invalid: seed authority"
                    );
                    seed_path(&self.target)?.to_owned()
                }
                EntryRole::ReferenceOnly => format!("carry-forward/references/{input_id}/content"),
                _ => anyhow::bail!("artifact_provenance_invalid: input role"),
            };
            let key = format!("selected_input_{ordinal}");
            context.begin_step(&key, &json!({"input_id":input_id,"source":input,"target_relative_path":target_relative_path}), &metadata_path.join(&target_relative_path)).await?;
            copy_verified_input(
                input.source_root(),
                &input.source_relative_path,
                &metadata_path,
                &target_relative_path,
                &input.sha256,
                input.bytes,
                input.mode,
                context,
            )
            .await?;
            let (kind, source_id) = match input.reference {
                InputReference::Artifact { id } => ("artifact", id),
                InputReference::CarriedInput { id } => ("carried_input", id),
            };
            installed.push(PreparedInput {
                input_id,
                ordinal,
                role: input.role,
                source_kind: kind.into(),
                source_id,
                source_run_id: input.immediate_source_run_id,
                original_artifact_id: input.original_artifact_id.into(),
                source_schema: input.source_schema_version.clone(),
                content_sha256: input.sha256.clone(),
                target_logical_name: input.target_logical_name.clone(),
                target_relative_path,
                historical_relation: input.historical_approval_relation.clone(),
            });
            provenance.push(InputProvenanceV1 {
                input_id,
                source_root: input.source_root().to_path_buf(),
                source_relative_path: input.source_relative_path.clone(),
                logical_name: input.logical_name.clone(),
                source_contract: input.source_contract.clone(),
                original_run_id: input.original_run_id,
                ancestor_manifest_sha256: input.ancestor_manifest_sha256.clone(),
                content_sha256: input.sha256.clone(),
                bytes: input.bytes,
                source_mode: input.mode,
                installed_mode: 0o600,
                mandatory: input.mandatory,
            });
            context.finish_step(&key, &input.sha256).await?;
        }
        let plan = serde_json::to_value(&self.plan)?;
        let published = [
            ("plan.json", serde_json::to_vec(&plan)?),
            (
                "target.json",
                serde_json::to_vec(&serde_json::to_value(&self.target)?)?,
            ),
            (
                "workflow.snapshot.json",
                self.target.plan.workflow_snapshot_json.as_bytes().to_vec(),
            ),
            (
                "catalog.snapshot.json",
                self.target.plan.catalog_snapshot_json.as_bytes().to_vec(),
            ),
        ];
        let mut files = Vec::new();
        for (name, bytes) in published {
            context.publish_file(root_path, name, &bytes).await?;
            files.push(FileBindingV1::new(name, &bytes));
        }
        for (name, bytes) in [
            ("preservation/index", self.workspace.index.clone()),
            ("preservation/staged.patch", self.workspace.staged.clone()),
            (
                "preservation/unstaged.patch",
                self.workspace.unstaged.clone(),
            ),
            (
                "preservation/manifest.json",
                serde_json::to_vec(self.workspace.as_ref())?,
            ),
            ("verified-receipt.json", serde_json::to_vec(receipt)?),
            (
                "owner.json",
                serde_json::to_vec(&json!({"schema_version":"run_carry_forward_owner_v1",
                "operation_id":receipt.operation_id,"source_witness":self.workspace.digest()}))?,
            ),
        ] {
            files.push(FileBindingV1::new(name, &bytes));
        }
        let mut directories = BTreeMap::new();
        for relative in [
            "",
            "checkout",
            "preservation",
            "preservation/workspace",
            "metadata",
        ] {
            context.check_current().await?;
            let dir = SafeRoot::open(&root_path.join(relative))?.file.metadata()?;
            directories.insert(relative.to_owned(), (dir.dev(), dir.ino()));
        }
        let workspace = WorkspaceMaterialV1 {
            operation_root: root_path.clone(),
            source_root: self.workspace.source().to_path_buf(),
            checkout_root: receipt.checkout_root.clone(),
            preservation_root: receipt.preservation_root.clone(),
            metadata_root: metadata_path,
            head: self.workspace.head().into(),
            pin_ref: receipt.pin_ref.clone(),
            checkout_index_sha256: manifest::clean_checkout_index_digest(
                &receipt.checkout_root,
                Deadline::after(context.remaining()?),
                Some(context),
            )
            .await?,
            source_witness_sha256: receipt.source_witness.clone(),
            preservation_manifest_sha256: receipt.manifest_sha256.clone(),
            directory_identities: directories,
        };
        let successor = build_successor(op, &workspace, &self.target, &plan)?;
        let manifest = ManifestV1 {
            schema_version: "run_carry_forward_manifest_v1".into(),
            operation_id: receipt.operation_id,
            source_run_id: self.plan.summary.source_run_id,
            reserved_successor_run_id: successor.id,
            idea_id: successor.idea_id,
            plan_sha256: self.plan.plan_sha256.clone(),
            source_witness_sha256: canonical_digest(&self.plan.summary.source_witness)?,
            source_semantic_sha256: self.plan.summary.source_witness.semantic_sha256.clone(),
            target_sha256: canonical_digest(&self.target)?,
            original_workflow_sha256: self.plan.summary.target.workflow_snapshot_hash.clone(),
            original_catalog_sha256: self.plan.summary.target.catalog_snapshot_hash.clone(),
            definition_files_sha256: self.plan.summary.target.definition_files_sha256.clone(),
            capability_delta_sha256: canonical_digest(&self.plan.summary.capability_delta)?,
            unresolved_findings_sha256: manifest::findings_digest(&installed, &provenance)?,
            preserved_bytes: manifest::bounded_preservation_bytes(
                self.plan.entries.iter().map(|e| e.bytes),
                [
                    self.workspace.index.len() as u64,
                    self.workspace.staged.len() as u64,
                    self.workspace.unstaged.len() as u64,
                ],
            )?,
            plan,
            target: self.target.clone(),
            workspace,
            files,
            inputs: installed,
            input_provenance: provenance,
            successor,
            verified_at: chrono::Utc::now().to_rfc3339(),
        };
        let deadline = Deadline::after(context.remaining()?);
        manifest::verify_material(&manifest, op, deadline, Some(context)).await?;
        context.checkpoint("finalizer:manifest").await?;
        // Observer waits may invalidate copied material, not just the generation.
        manifest::verify_material(&manifest, op, deadline, Some(context)).await?;
        let bytes = serde_json::to_vec(&manifest)?;
        context
            .publish_file(root_path, "manifest.json", &bytes)
            .await?;
        manifest::verify_material(&manifest, op, deadline, Some(context)).await?;
        context
            .finish_step("finalizer_metadata", &ContentDigest::of(&bytes))
            .await?;
        Ok(PreparedManifest {
            manifest_ref: root_path.join("manifest.json"),
            manifest_sha256: ContentDigest::of(&bytes),
        })
    }
}

pub(super) fn seed_path(target: &ContinuationTargetV1) -> Result<&str> {
    ensure!(
        target.profile.proposal_input == "proposal_current"
            && target.plan.initial_state == target.profile.review_state,
        "target_policy_incompatible: fresh review required"
    );
    let template = target
        .plan
        .artifact_paths
        .get("proposal_current")
        .context("target_policy_incompatible: proposal_current")?;
    let path = template
        .strip_prefix("${CHAINWORKS_META_ROOT:-.chainworks}/")
        .context("unsafe_path: seed template")?;
    validate_relative(path)?;
    ensure!(
        path.len() <= 4096
            && !path.chars().any(char::is_control)
            && !path.contains(['$', '\\'])
            && !path
                .split('/')
                .any(|p| matches!(p, ".git" | "carry-forward" | ".chainworks")),
        "unsafe_path: seed destination"
    );
    for (name, other) in &target.plan.artifact_paths {
        if name == "proposal_current" {
            continue;
        }
        if let Some(other) = other.strip_prefix("${CHAINWORKS_META_ROOT:-.chainworks}/") {
            ensure!(
                !path_overlap(path, other) && !path_overlap("carry-forward", other),
                "target_policy_incompatible: input/output namespace overlap"
            );
        }
    }
    Ok(path)
}

fn path_overlap(a: &str, b: &str) -> bool {
    Path::new(a).starts_with(b) || Path::new(b).starts_with(a)
}

pub(super) fn build_successor(
    op: &Continuation,
    workspace: &WorkspaceMaterialV1,
    target: &ContinuationTargetV1,
    plan: &Value,
) -> Result<Run> {
    let snapshot: Value = serde_json::from_str(&target.plan.workflow_snapshot_json)?;
    let workflow = &snapshot["workflow"];
    let workflow_id = workflow["id"]
        .as_str()
        .context("target_policy_incompatible: workflow id")?
        .to_owned();
    let workflow_title = workflow["name"].as_str().unwrap_or(&workflow_id).to_owned();
    let delivery: DeliveryConfiguration =
        serde_json::from_value(plan["target"]["delivery_configuration"].clone())?;
    let utf8 = |p: &Path| -> Result<String> { Ok(p.to_str().context("unsafe_path: UTF8")?.into()) };
    let definition_path = |key: &str| -> Result<String> {
        let relative = plan["target"][key]
            .as_str()
            .context("target_policy_incompatible: definition path")?;
        validate_relative(relative)?;
        utf8(&Path::new(&delivery.repo_root).join(relative))
    };
    Ok(Run {
        id: RunId(Uuid::parse_str(&op.reserved_successor_run_id)?),
        idea_id: IdeaId(Uuid::parse_str(&op.idea_id)?),
        status: RunStatus::Pending,
        workflow_id,
        workflow_title,
        workspace_root: delivery.repo_root.clone(),
        artifact_root: utf8(&workspace.metadata_root)?,
        started_at: chrono::DateTime::parse_from_rfc3339(&op.created_at)?
            .with_timezone(&chrono::Utc),
        completed_at: None,
        cancellation_requested_at: None,
        cancellation_settled_at: None,
        cancellation_settlement_log: None,
        current_state: Some(target.profile.review_state.clone()),
        workflow_yaml_path: Some(definition_path("workflow_yaml_path")?),
        agent_catalog_yaml_path: Some(definition_path("agent_catalog_yaml_path")?),
        worktree_root: Some(utf8(&workspace.checkout_root)?),
        base_branch: Some(
            plan["workspace_snapshot"]["base_branch"]
                .as_str()
                .context("source_changed: original base branch")?
                .into(),
        ),
        base_revision: Some(
            plan["workspace_snapshot"]["base_revision"]
                .as_str()
                .context("source_changed: original base revision")?
                .into(),
        ),
        target_branch: Some(
            workspace
                .pin_ref
                .strip_prefix("refs/heads/")
                .context("unsafe_path: pin ref")?
                .into(),
        ),
        delivery_configuration_json: Some(serde_json::to_string(&delivery)?),
        delivery_preflight_json: None,
        workflow_family: target.plan.workflow_family.clone(),
        project_key: None,
        risk_class: target.plan.risk_class.clone(),
        stack: target.plan.stack.clone(),
        workflow_snapshot_hash: Some(target.plan.workflow_snapshot_hash.clone()),
        catalog_snapshot_hash: Some(target.plan.catalog_snapshot_hash.clone()),
        workflow_snapshot_json: Some(target.plan.workflow_snapshot_json.clone()),
        catalog_snapshot_json: Some(target.plan.catalog_snapshot_json.clone()),
        drift_detected_at: None,
        drift_details_json: None,
        chainworks_meta_root: Some(utf8(&workspace.metadata_root)?),
        review_routing_json: None,
        closeout_readiness_mode: target.plan.closeout_readiness_mode.clone(),
    })
}
