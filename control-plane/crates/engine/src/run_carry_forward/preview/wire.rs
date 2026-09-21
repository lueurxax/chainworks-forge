use super::*;
use domain::run_carry_forward_api as api;

/// Strict presentation conversion for an already authenticated manifest entry.
/// This does not validate a manifest, authorize a source, or reopen any file.
pub fn wire_manifest_entry(value: &Value) -> anyhow::Result<api::CarryForwardEntryV1> {
    const FIELDS: [&str; 13] = [
        "key",
        "source_artifact_id",
        "logical_name",
        "role",
        "sha256",
        "bytes",
        "mode",
        "source_contract",
        "source_schema_version",
        "target_logical_name",
        "reason",
        "historical_approval_relation",
        "workspace_entry",
    ];
    ensure!(
        value
            .as_object()
            .is_some_and(|object| object.len() == FIELDS.len()
                && FIELDS.iter().all(|field| object.contains_key(*field))),
        "prepared_material_changed: entry fields"
    );
    ensure!(
        serde_json::to_vec(value)?.len() <= SUMMARY_LIMIT,
        "continuation_budget_exceeded: manifest entry"
    );
    let item: PlanEntry =
        serde_json::from_value(value.clone()).context("prepared_material_changed: entry schema")?;
    match item.key.namespace.as_str() {
        "workspace" => {
            let workspace = item
                .workspace_entry
                .as_ref()
                .context("prepared_material_changed: workspace entry")?;
            ensure!(
                workspace.path == item.key.relative_path
                    && item.key.stable_id == workspace.path
                    && item.source_artifact_id.is_none()
                    && item.historical_approval_relation.is_none(),
                "prepared_material_changed: workspace binding"
            );
            if workspace.deleted && !workspace.excluded {
                ensure!(
                    workspace.sha256.is_none()
                        && workspace.size == 0
                        && workspace.mode == 0
                        && item.role == EntryRole::PreserveOnly
                        && item.sha256.is_some()
                        && item.source_contract.as_deref().is_some_and(|contract| [
                            "git_head_blob",
                            "git_index_blob"
                        ]
                        .contains(&contract))
                        && item.source_schema_version.as_deref()
                            == Some("workspace_deletion_preimage_v1")
                        && item.logical_name.as_deref() == Some("workspace_deleted_path")
                        && item.target_logical_name.as_deref() == Some("workspace_absent_file")
                        && [0o100644, 0o100755, 0o120000].contains(&item.mode)
                        && item.reason == "preserve_workspace_deletion",
                    "prepared_material_changed: deletion preimage binding"
                );
                return entry(&item);
            }
            ensure!(
                workspace.sha256 == item.sha256
                    && workspace.size == item.bytes
                    && workspace.mode == item.mode
                    && item.source_contract.is_none()
                    && item.source_schema_version.is_none()
                    && item.logical_name.is_none()
                    && item.target_logical_name.is_none(),
                "prepared_material_changed: current file binding"
            );
            ensure!(
                if workspace.excluded {
                    item.role == EntryRole::Excluded
                        && item.sha256.is_none()
                        && item.reason == "excluded_machine_metadata"
                } else {
                    item.role == EntryRole::PreserveOnly && item.reason == "preserve_workspace"
                },
                "prepared_material_changed: workspace role"
            );
        }
        "artifact" | "carried_input" => {
            let id = Uuid::parse_str(&item.key.stable_id)
                .context("prepared_material_changed: input ID")?;
            ensure!(
                id.to_string() == item.key.stable_id
                    && item.source_artifact_id.is_some()
                    && item.logical_name.is_some()
                    && item.source_contract.is_some()
                    && item.source_schema_version.is_some()
                    && item.target_logical_name.is_some()
                    && item.workspace_entry.is_none()
                    && matches!(
                        item.role,
                        EntryRole::ExecutionSeed | EntryRole::ReferenceOnly
                    )
                    && ["selected_input", "mandatory_reference"].contains(&item.reason.as_str()),
                "prepared_material_changed: input entry"
            );
        }
        _ => anyhow::bail!("prepared_material_changed: entry namespace"),
    }
    entry(&item)
}

pub(super) fn deserialize_workspace_entry<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<super::super::WorkspaceEntry>, D::Error> {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Workspace {
        path: String,
        sha256: Option<ContentDigest>,
        size: u64,
        mode: u32,
        deleted: bool,
        replacement: Option<Replacement>,
        excluded: bool,
        exclusion_reason: Option<String>,
        link_target: Option<String>,
    }
    #[derive(Deserialize)]
    #[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
    enum Replacement {
        Directory,
        Ancestor { path: String },
    }
    let value = Option::<Workspace>::deserialize(deserializer)?;
    Ok(value.map(|workspace| super::super::WorkspaceEntry {
        path: workspace.path,
        sha256: workspace.sha256,
        size: workspace.size,
        mode: workspace.mode,
        deleted: workspace.deleted,
        replacement: workspace.replacement.map(|replacement| match replacement {
            Replacement::Directory => super::super::WorkspaceReplacement::Directory,
            Replacement::Ancestor { path } => super::super::WorkspaceReplacement::Ancestor { path },
        }),
        excluded: workspace.excluded,
        exclusion_reason: workspace.exclusion_reason,
        link_target: workspace.link_target,
    }))
}

/// Convert only a page belonging to this opaque prepared plan. No I/O or authority
/// is introduced here; every field comes from the plan's original observations.
pub fn wire_page(
    page: &PreviewPage,
    prepared: &PreparedPlan,
) -> anyhow::Result<api::RunCarryForwardPreviewPageV1> {
    ensure!(
        page.plan_sha256 == prepared.plan.plan_sha256
            && page.entry_count == prepared.plan.entries.len()
            && canonical_digest(&page.plan_summary)? == canonical_digest(&prepared.plan.summary)?
            && !page.entries.is_empty()
            && page.entries.len() <= 500,
        "source_changed: page binding"
    );
    let start = prepared
        .plan
        .entries
        .binary_search_by(|entry| entry.key.cmp(&page.entries[0].key))
        .map_err(|_| anyhow::anyhow!("source_changed: page entry"))?;
    let end = start
        .checked_add(page.entries.len())
        .context("source_changed")?;
    ensure!(
        end <= prepared.plan.entries.len()
            && canonical_digest(&page.entries)?
                == canonical_digest(&&prepared.plan.entries[start..end])?,
        "source_changed: page contents"
    );
    if let Some(text) = &page.next_cursor {
        let cursor: Cursor = serde_json::from_str(text).context("source_changed: page cursor")?;
        ensure!(
            end < prepared.plan.entries.len()
                && cursor.version == 1
                && cursor.plan == page.plan_sha256
                && cursor.last == page.entries.last().context("source_changed")?.key,
            "source_changed: page cursor"
        );
    } else {
        ensure!(
            end == prepared.plan.entries.len(),
            "source_changed: missing page cursor"
        );
    }
    Ok(api::RunCarryForwardPreviewPageV1 {
        schema_version: api::PreviewPageSchema::V1,
        plan_summary: summary(prepared)?,
        entries: page
            .entries
            .iter()
            .map(entry)
            .collect::<anyhow::Result<Vec<_>>>()?
            .try_into()?,
        entry_count: (page.entry_count as u64).try_into()?,
        plan_sha256: page.plan_sha256.clone(),
        next_cursor: page
            .next_cursor
            .clone()
            .map(TryInto::try_into)
            .transpose()?,
    })
}

pub(super) fn validate_plan(prepared: &PreparedPlan) -> anyhow::Result<()> {
    let summary = summary(prepared)?;
    ensure!(
        serde_json::to_vec(&summary)?.len() <= SUMMARY_LIMIT,
        "continuation_budget_exceeded: wire summary"
    );
    for item in &prepared.plan.entries {
        entry(item)?;
    }
    Ok(())
}

fn reference(reference: &InputReference) -> api::InputReference {
    match reference {
        InputReference::Artifact { id } => api::InputReference::Artifact { id: (*id).into() },
        InputReference::CarriedInput { id } => {
            api::InputReference::CarriedInput { id: (*id).into() }
        }
    }
}

fn summary(prepared: &PreparedPlan) -> anyhow::Result<api::RunCarryForwardPlanSummaryV1> {
    let plan = &prepared.plan;
    let target = &plan.summary.target;
    let exclusions = plan
        .selection
        .excluded_workspace_paths
        .iter()
        .map(|(path, reason)| {
            Ok(api::ExcludedWorkspacePath {
                path: path.clone().try_into()?,
                reason: reason.clone().try_into()?,
            })
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    Ok(api::RunCarryForwardPlanSummaryV1 {
        source_run_id: plan.summary.source_run_id.0.into(),
        source_witness: plan.summary.source_witness.wire.clone(),
        target: api::CarryForwardTarget {
            workflow_yaml_path: target.workflow_yaml_path.clone().try_into()?,
            agent_catalog_yaml_path: target.agent_catalog_yaml_path.clone().try_into()?,
            expected_workflow_snapshot_hash: target.workflow_snapshot_hash.clone(),
            expected_catalog_snapshot_hash: target.catalog_snapshot_hash.clone(),
            delivery_configuration_json: serde_json::to_string(&target.delivery_configuration)?
                .try_into()?,
        },
        profile: api::CarryForwardProfile::ImplementationRestartV1,
        selection: api::CarryForwardSelection {
            proposal_input: reference(&plan.selection.proposal_input),
            reference_inputs: plan
                .selection
                .reference_inputs
                .iter()
                .map(reference)
                .collect::<Vec<_>>()
                .try_into()?,
            include_dirty_work: plan.selection.include_dirty_work.try_into()?,
            excluded_workspace_paths: exclusions.try_into()?,
        },
        workspace_snapshot: serde_json::from_value(plan.summary.workspace_snapshot.clone())?,
        capability_delta: serde_json::from_value(plan.summary.capability_delta.clone())?,
        holds: Vec::new().try_into()?,
        limits: api::CarryForwardLimitsV1::default(),
    })
}

fn entry(item: &PlanEntry) -> anyhow::Result<api::CarryForwardEntryV1> {
    let entry_id = if item.key.namespace == "workspace" {
        ContentDigest::of(item.key.relative_path.as_bytes())
            .as_str()
            .to_owned()
    } else {
        item.key.stable_id.clone()
    };
    let logical = item
        .logical_name
        .clone()
        .unwrap_or_else(|| "workspace_file".into());
    if item.role == EntryRole::Excluded {
        return Ok(api::CarryForwardEntryV1::Excluded(
            api::CarryForwardExcludedEntryV1 {
                entry_id: entry_id.try_into()?,
                source_namespace: item.key.namespace.clone().try_into()?,
                source_artifact_id: item.source_artifact_id.map(Into::into),
                logical_name: logical.try_into()?,
                source_relative_path: item.key.relative_path.clone().try_into()?,
                reason: "excluded_machine_metadata".to_string().try_into()?,
            },
        ));
    }
    // Deletions bind a historical preimage; they never claim current file bytes.
    let hash = item
        .sha256
        .clone()
        .context("source_changed: missing observable content or preimage")?;
    let reason = match item.role {
        EntryRole::ExecutionSeed => "selected_proposal_seed",
        EntryRole::ReferenceOnly => {
            if item.reason == "mandatory_reference" {
                "mandatory_reference"
            } else {
                "selected_reference"
            }
        }
        EntryRole::PreserveOnly => {
            if item.reason == "preserve_workspace_deletion" {
                "preserve_workspace_deletion"
            } else {
                "preserve_workspace"
            }
        }
        EntryRole::Excluded => unreachable!(),
    };
    let relation = match item.historical_approval_relation.as_ref() {
        Some(value)
            if value["authority"] == "none"
                && value["binding"] == "unproven"
                && value["fresh_review_required"] == true
                && value
                    .get("fresh_approval_required")
                    .is_none_or(|v| v == true)
                && value.get("provenance").is_none_or(Value::is_object)
                && value.as_object().is_some_and(|object| {
                    object.keys().all(|key| {
                        [
                            "authority",
                            "binding",
                            "fresh_review_required",
                            "fresh_approval_required",
                            "provenance",
                        ]
                        .contains(&key.as_str())
                    })
                }) =>
        {
            api::HistoricalApprovalRelation::HistoricalBindingUnproven
        }
        None => api::HistoricalApprovalRelation::NotApplicable,
        _ => anyhow::bail!("artifact_provenance_invalid: unknown historical binding"),
    };
    let entry = api::CarryForwardHashedEntryV1 {
        entry_id: entry_id.try_into()?,
        source_namespace: item.key.namespace.clone().try_into()?,
        source_artifact_id: item.source_artifact_id.map(Into::into),
        logical_name: logical.clone().try_into()?,
        source_relative_path: item.key.relative_path.clone().try_into()?,
        sha256: hash,
        bytes: item.bytes,
        mode: item.mode,
        source_contract: item
            .source_contract
            .clone()
            .map(TryInto::try_into)
            .transpose()?,
        source_schema_version: item
            .source_schema_version
            .clone()
            .map(TryInto::try_into)
            .transpose()?,
        target_logical_name: item
            .target_logical_name
            .clone()
            .unwrap_or(logical)
            .try_into()?,
        reason: reason.to_string().try_into()?,
        historical_approval_relation: relation,
    };
    Ok(match item.role {
        EntryRole::ExecutionSeed => api::CarryForwardEntryV1::ExecutionSeed(entry),
        EntryRole::ReferenceOnly => api::CarryForwardEntryV1::ReferenceOnly(entry),
        EntryRole::PreserveOnly => api::CarryForwardEntryV1::PreserveOnly(entry),
        EntryRole::Excluded => unreachable!(),
    })
}

pub(super) async fn source_witness(
    source: &Run,
    observation: &db::repos::run_continuation_witness::Observation,
    stage: Uuid,
    cursor: String,
    workspace: &WorkspacePreview,
    roots: &Roots,
    deadline: Deadline,
) -> anyhow::Result<api::SourceWitnessV1> {
    let ids = |values: &[String]| -> anyhow::Result<api::BoundedList<api::CanonicalUuid, 128>> {
        ensure!(
            values.len() <= 128,
            "continuation_budget_exceeded: witness IDs"
        );
        values
            .iter()
            .cloned()
            .map(TryInto::try_into)
            .collect::<Result<Vec<_>, _>>()?
            .try_into()
            .map_err(Into::into)
    };
    let tree = super::super::git::text(
        &roots.checkout,
        &[
            "rev-parse",
            "--verify",
            &format!("{}^{{tree}}", workspace.head),
        ],
        deadline,
    )
    .await?;
    let directories = roots
        .identities
        .iter()
        .map(|(name, (device, inode))| {
            Ok(api::DirectoryIdentityV1 {
                root: name.clone().try_into()?,
                device: *device,
                inode: *inode,
            })
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    Ok(api::SourceWitnessV1 {
        workflow_snapshot_hash: snapshot_hash(
            source
                .workflow_snapshot_hash
                .as_deref()
                .context("unsupported_frontier")?,
        )?,
        catalog_snapshot_hash: snapshot_hash(
            source
                .catalog_snapshot_hash
                .as_deref()
                .context("unsupported_frontier")?,
        )?,
        stage_execution_id: stage.into(),
        cursor: cursor.try_into()?,
        journal_ids: ids(&observation.journal_ids)?,
        approval_ids: ids(&observation.approval_ids)?,
        artifact_generation_ids: ids(&observation.artifact_generation_ids)?,
        idea_sha256: observation.idea_sha256.clone(),
        head: workspace.head.clone().try_into()?,
        tree: tree.try_into()?,
        index_sha256: workspace.index_sha256.clone(),
        content_sha256: canonical_digest(&workspace.entries)?,
        directory_identities: directories.try_into()?,
        settled_ownership_effect_sha256: observation.semantic_sha256.clone(),
    })
}

pub(super) async fn workspace_summary(
    source: &Run,
    workspace: &WorkspacePreview,
    deadline: Deadline,
) -> anyhow::Result<api::WorkspaceSnapshotSummaryV1> {
    let untracked = super::super::git::run(
        &workspace.source,
        &["ls-files", "--others", "--exclude-standard", "-z"],
        None,
        deadline,
    )
    .await?;
    let count = untracked
        .split(|byte| *byte == 0)
        .filter(|path| !path.is_empty())
        .count();
    Ok(api::WorkspaceSnapshotSummaryV1 {
        base_branch: source
            .base_branch
            .clone()
            .context("unsupported_frontier: missing original base branch")?
            .try_into()?,
        base_revision: source
            .base_revision
            .clone()
            .context("unsupported_frontier: missing original base revision")?
            .try_into()?,
        head: workspace.head.clone().try_into()?,
        staged_patch_sha256: workspace.staged_sha256.clone(),
        unstaged_patch_sha256: workspace.unstaged_sha256.clone(),
        inventory_sha256: canonical_digest(&workspace.entries)?,
        untracked_count: (count as u64).try_into()?,
        deleted_count: (workspace.entries.iter().filter(|e| e.deleted).count() as u64)
            .try_into()?,
        link_count: (workspace
            .entries
            .iter()
            .filter(|e| e.link_target.is_some())
            .count() as u64)
            .try_into()?,
        target_content_sha256: canonical_digest(
            &workspace
                .entries
                .iter()
                .filter(|e| !e.excluded)
                .collect::<Vec<_>>(),
        )?,
    })
}

pub(super) fn capability_delta(
    source: &RunPlan,
    target: &RunPlan,
) -> anyhow::Result<api::CapabilityDeltaV1> {
    fn capabilities(plan: &RunPlan) -> BTreeSet<String> {
        let mut values = BTreeSet::new();
        for state in plan.states.values() {
            for task in state.tasks.iter().chain(&state.post_approval_tasks) {
                let prefix = format!("{}:{}:{}", state.id, task.task_name, task.agent.agent_id);
                let agent = &task.agent;
                if let Some(permission) = &agent.permission_profile {
                    values.insert(format!("{prefix}:permission:{permission}"));
                }
                if let Some(strategy) = &agent.worktree_strategy {
                    values.insert(format!("{prefix}:worktree:{strategy}"));
                }
                if agent.worktree_write_enabled {
                    values.insert(format!("{prefix}:worktree_write"));
                }
                for mcp in &agent.requested_mcp_server_ids {
                    values.insert(format!("{prefix}:mcp:{mcp}"));
                }
                if agent.requires_xcode_host_execution {
                    values.insert(format!("{prefix}:xcode_host_execution"));
                }
                if agent.xcode_broker_required {
                    values.insert(format!("{prefix}:xcode_broker_required"));
                }
                if agent.xcode_shim_injection_signal {
                    values.insert(format!("{prefix}:xcode_shim"));
                }
            }
        }
        values
    }
    let old = capabilities(source);
    let new = capabilities(target);
    let added = new.difference(&old).cloned().collect::<Vec<_>>();
    let removed = old.difference(&new).cloned().collect::<Vec<_>>();
    ensure!(
        added.len() <= 128 && removed.len() <= 128,
        "continuation_budget_exceeded: capabilities"
    );
    let sha256 = canonical_digest(
        &json!({"schema_version":"resolved_capability_delta_v1","source":old,"target":new,"added":added,"removed":removed}),
    )?;
    Ok(api::CapabilityDeltaV1 {
        added: added
            .into_iter()
            .map(TryInto::try_into)
            .collect::<Result<Vec<_>, _>>()?
            .try_into()?,
        removed: removed
            .into_iter()
            .map(TryInto::try_into)
            .collect::<Result<Vec<_>, _>>()?
            .try_into()?,
        sha256,
    })
}
