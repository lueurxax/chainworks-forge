use super::*;
use domain::artifact::Artifact;
use sqlx::Row;
use std::os::unix::fs::MetadataExt;

#[derive(Debug, Clone, Serialize)]
pub struct CanonicalSourceInput {
    pub reference: InputReference,
    pub immediate_source_run_id: RunId,
    pub original_artifact_id: Uuid,
    pub original_run_id: RunId,
    pub ancestor_manifest_sha256: Vec<ContentDigest>,
    pub source_relative_path: String,
    pub logical_name: String,
    pub source_contract: String,
    pub source_schema_version: String,
    pub target_logical_name: String,
    pub role: EntryRole,
    pub sha256: ContentDigest,
    pub bytes: u64,
    pub mode: u32,
    pub mandatory: bool,
    pub historical_approval_relation: Value,
    source_root: PathBuf,
}

impl CanonicalSourceInput {
    pub fn source_root(&self) -> &Path {
        &self.source_root
    }
    pub fn source_path(&self) -> PathBuf {
        self.source_root.join(&self.source_relative_path)
    }
    pub(super) fn entry(&self) -> PlanEntry {
        let (namespace, id) = match self.reference {
            InputReference::Artifact { id } => ("artifact", id),
            InputReference::CarriedInput { id } => ("carried_input", id),
        };
        PlanEntry {
            key: EntryKey {
                namespace: namespace.into(),
                relative_path: self.source_relative_path.clone(),
                stable_id: id.to_string(),
            },
            source_artifact_id: Some(self.original_artifact_id),
            logical_name: Some(self.logical_name.clone()),
            role: self.role,
            sha256: Some(self.sha256.clone()),
            bytes: self.bytes,
            mode: self.mode,
            source_contract: Some(self.source_contract.clone()),
            source_schema_version: Some(self.source_schema_version.clone()),
            target_logical_name: Some(self.target_logical_name.clone()),
            reason: if self.mandatory {
                "mandatory_reference"
            } else {
                "selected_input"
            }
            .into(),
            historical_approval_relation: Some(self.historical_approval_relation.clone()),
            workspace_entry: None,
        }
    }
}

// Exact schema IDs, not substrings, artifact labels or natural-language status heuristics.
// Preserve the whole contract, including resolved entries: only fresh reviewers may adjudicate it.
fn mandatory_contract(contract: &str) -> bool {
    matches!(
        contract,
        "score_lift_backlog_v1"
            | "proposal_feedback_coverage_v1"
            | "proposal_review_v1"
            | "proposal_review_summary_v2"
            | "implementation_backlog"
            | "implementation_backlog_v1"
            | "implementation_self_assessment_v1"
            | "implementation_self_assessment_v2"
            | "audit_report_v1"
            | "security_report_v1"
            | "prepush_review_v1"
            | "implementation_review_summary_v1"
            | "quality_gate_blocker_assessment_v1"
            | "blocker_boundary_status_v1"
            | "blocker_boundary_approval_request_v1"
            | "blocker_boundary_human_decision_v1"
            | "proposal_decomposition_plan_v1"
            | "followup_proposal_seed_v1"
    )
}

pub(super) async fn resolve(
    pool: &SqlitePool,
    source: &Run,
    plan: &RunPlan,
    roots: &Roots,
    selection: &PreviewSelection,
    deadline: Deadline,
) -> anyhow::Result<Vec<CanonicalSourceInput>> {
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM artifacts WHERE run_id=?")
        .bind(source.id.to_string())
        .fetch_one(pool)
        .await?;
    ensure!(count <= 50_000, "continuation_budget_exceeded: artifacts");
    let artifacts = db::repos::artifacts::list_by_run(pool, source.id).await?;
    let mut selected: BTreeMap<InputReference, bool> = selection
        .reference_inputs
        .iter()
        .cloned()
        .map(|r| (r, false))
        .collect();
    for artifact in &artifacts {
        let id = artifact.id.0;
        if mandatory_contract(&artifact.contract_id)
            || ["implementation_backlog", "implementation_plan"].contains(&artifact.name.as_str())
        {
            selected.insert(InputReference::Artifact { id }, true);
        }
        // A known mandatory logical input with a removed/unknown contract must not silently vanish.
        if [
            "score_lift_backlog",
            "proposal_feedback_coverage",
            "implementation_backlog",
            "implementation_self_assessment",
            "audit_report",
            "security_report",
            "prepush_review_report",
            "implementation_review_summary",
            "quality_gate_blocker_assessment",
            "blocker_boundary_status",
            "blocker_boundary_human_decision",
        ]
        .contains(&artifact.name.as_str())
        {
            ensure!(
                mandatory_contract(&artifact.contract_id)
                    || artifact.name == "implementation_backlog",
                "artifact_provenance_invalid: unknown finding schema"
            );
        }
    }
    let carried = sqlx::query("SELECT input_id,source_schema,target_logical_name,role FROM run_continuation_inputs WHERE successor_run_id=? AND installed=1 LIMIT 130")
        .bind(source.id.to_string()).fetch_all(pool).await?;
    ensure!(
        carried.len() <= 129,
        "continuation_budget_exceeded: carried inputs"
    );
    for row in carried {
        let schema: String = row.try_get("source_schema")?;
        let role: String = row.try_get("role")?;
        if mandatory_contract(&schema) || role == "reference_only" {
            let id: String = row.try_get("input_id")?;
            selected.insert(
                InputReference::CarriedInput {
                    id: Uuid::parse_str(&id)?,
                },
                true,
            );
        }
    }
    selected.remove(&selection.proposal_input);
    ensure!(
        selected.len() <= 128,
        "continuation_budget_exceeded: mandatory references"
    );
    let ancestors = lineage(pool, source.id).await?;
    let generations =
        super::generations::resolve(pool, source, plan, roots, &artifacts, &selected).await?;
    let catalog: Value = serde_json::from_str(&plan.catalog_snapshot_json)?;
    let mut result = Vec::new();
    for (reference, role, mandatory) in std::iter::once((
        selection.proposal_input.clone(),
        EntryRole::ExecutionSeed,
        false,
    ))
    .chain(
        selected
            .into_iter()
            .map(|(r, mandatory)| (r, EntryRole::ReferenceOnly, mandatory)),
    ) {
        deadline.check()?;
        let descriptor = match &reference {
            InputReference::Artifact { id } => {
                let artifact = artifacts
                    .iter()
                    .find(|a| a.id.0 == *id)
                    .context("artifact_provenance_invalid: immediate source artifact")?;
                from_artifact(
                    pool,
                    source,
                    roots,
                    plan,
                    &catalog,
                    artifact,
                    generations.get(id),
                    reference.clone(),
                    role,
                    mandatory,
                    ancestors.clone(),
                    deadline,
                )
                .await?
            }
            InputReference::CarriedInput { id } => {
                from_carried(
                    pool,
                    source,
                    roots,
                    *id,
                    role,
                    mandatory,
                    ancestors.clone(),
                    deadline,
                )
                .await?
            }
        };
        if role == EntryRole::ExecutionSeed {
            ensure!(
                descriptor.logical_name == "proposal_current"
                    && (descriptor.source_contract == "proposal_current"
                        || descriptor.source_schema_version == "legacy_plain_proposal_v1"),
                "artifact_provenance_invalid: proposal seed"
            );
        }
        result.push(descriptor);
    }
    result.sort_by_key(|input| input.entry().key);
    Ok(result)
}

async fn from_artifact(
    pool: &SqlitePool,
    source: &Run,
    roots: &Roots,
    plan: &RunPlan,
    catalog: &Value,
    artifact: &Artifact,
    generation: Option<&super::generations::GenerationRead>,
    reference: InputReference,
    role: EntryRole,
    mandatory: bool,
    ancestors: Vec<ContentDigest>,
    deadline: Deadline,
) -> anyhow::Result<CanonicalSourceInput> {
    ensure!(
        artifact.run_id == source.id,
        "artifact_provenance_invalid: run owner"
    );
    if artifact.checksum_sha256.is_none() && generation.is_none() {
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM artifacts WHERE run_id=? AND name=? AND file_path=?",
        )
        .bind(source.id.to_string())
        .bind(&artifact.name)
        .bind(&artifact.file_path)
        .fetch_one(pool)
        .await?;
        ensure!(
            count == 1,
            "artifact_provenance_invalid: checksumless canonical generation unproven"
        );
    }
    let mut proof = if let Some(generation) = generation {
        Some(generation.proof.clone())
    } else if artifact.checksum_sha256.is_none()
        || artifact.contract_id.ends_with(".output")
        || !plan.artifact_paths.contains_key(&artifact.name)
        || ["implementation_backlog", "implementation_plan"].contains(&artifact.name.as_str())
    {
        Some(super::provenance::declared_output(pool, source, plan, artifact, false).await?)
    } else {
        None
    };
    let relative = if let Some(expected) = plan.artifact_paths.get(&artifact.name) {
        expected
            .strip_prefix("${CHAINWORKS_META_ROOT:-.chainworks}/")
            .context("artifact_provenance_invalid: unsupported artifact path template")?
            .to_string()
    } else {
        let path = proof
            .as_ref()
            .and_then(|proof| proof["dynamic_canonical_path"].as_str())
            .context("artifact_provenance_invalid: unknown logical path")?;
        Path::new(path)
            .strip_prefix(&roots.metadata)
            .context("artifact_provenance_invalid: dynamic output outside metadata root")?
            .to_str()
            .context("unsafe_path: dynamic output path")?
            .to_string()
    };
    validate_relative(&relative)?;
    ensure!(
        Path::new(&artifact.file_path) == roots.metadata.join(&relative),
        "artifact_provenance_invalid: noncanonical artifact path"
    );
    let relative = generation.map(|g| g.relative.as_str()).unwrap_or(&relative);
    let root = SafeRoot::open(&roots.metadata)?;
    let bytes = read_regular(
        &root,
        relative,
        InventoryLimits::default().max_file_bytes,
        deadline,
    )?;
    let hash = ContentDigest::of(&bytes);
    ensure!(
        artifact
            .checksum_sha256
            .as_deref()
            .map_or(proof.is_some(), |h| snapshot_hash(h).ok().as_ref()
                == Some(&hash))
            && artifact.size_bytes == Some(bytes.len() as i64),
        "artifact_provenance_invalid: source artifact bytes"
    );
    let schema = source_schema(artifact, &bytes, catalog, proof.as_ref())?;
    let mut relation =
        json!({"authority":"none", "binding":"unproven", "fresh_review_required":true});
    if let Some(proof) = proof.as_mut() {
        proof
            .as_object_mut()
            .unwrap()
            .remove("declared_output_schema");
        proof["content_binding"] = json!(if artifact.checksum_sha256.is_some() {
            "historical_checksum"
        } else {
            "fresh_observation_only"
        });
        proof["reader"] = json!(schema);
        relation["provenance"] = proof.clone();
        relation["fresh_approval_required"] = json!(true);
    }
    let metadata = root.open_file(relative)?.metadata()?;
    let original = artifact.id.0;
    Ok(CanonicalSourceInput {
        reference,
        immediate_source_run_id: source.id,
        original_artifact_id: original,
        original_run_id: source.id,
        ancestor_manifest_sha256: ancestors,
        source_relative_path: relative.into(),
        logical_name: artifact.name.clone(),
        source_contract: artifact.contract_id.clone(),
        source_schema_version: schema.into(),
        target_logical_name: if role == EntryRole::ExecutionSeed {
            "proposal_current".into()
        } else {
            artifact.name.clone()
        },
        role,
        sha256: hash,
        bytes: bytes.len() as u64,
        mode: metadata.mode(),
        mandatory,
        historical_approval_relation: relation,
        source_root: roots.metadata.clone(),
    })
}

fn source_schema(
    artifact: &Artifact,
    bytes: &[u8],
    catalog: &Value,
    proof: Option<&Value>,
) -> anyhow::Result<String> {
    if let Some(schema) = proof
        .and_then(|p| p.get("declared_output_schema"))
        .filter(|v| !v.is_null())
    {
        let schema: workflow::plan::OutputSchema = serde_json::from_value(schema.clone())?;
        ensure!(
            crate::contracts::validate_output(&artifact.name, bytes, Some(&schema)).status
                == domain::validation::ValidationStatus::Passed,
            "artifact_provenance_invalid: historical per-output validation"
        );
    }
    let reader = if proof.is_some() && artifact.name == "implementation_backlog" {
        super::historical::backlog(bytes)?;
        "legacy_implementation_backlog_v1"
    } else if proof.is_some() && artifact.name == "implementation_plan" {
        let text = std::str::from_utf8(bytes)
            .context("artifact_provenance_invalid: historical plan UTF8")?;
        ensure!(
            !text.trim().is_empty(),
            "artifact_provenance_invalid: empty historical plan"
        );
        "legacy_implementation_plan_v1"
    } else if proof.is_some()
        && artifact.name == "proposal_current"
        && artifact.contract_id == format!("{}.output", artifact.provider)
    {
        validate_schema("proposal_current", &artifact.name, bytes, Some(catalog))?;
        "legacy_plain_proposal_v1"
    } else {
        validate_schema(&artifact.contract_id, &artifact.name, bytes, Some(catalog))?;
        &artifact.contract_id
    };
    Ok(reader.into())
}

/// Exact historical structured identities are read against the frozen declaration.
fn validate_schema(
    contract: &str,
    logical: &str,
    bytes: &[u8],
    catalog: Option<&Value>,
) -> anyhow::Result<()> {
    if contract == "proposal_current" && logical == "proposal_current" {
        let text =
            std::str::from_utf8(bytes).context("artifact_provenance_invalid: proposal UTF8")?;
        ensure!(
            !text.trim().is_empty(),
            "artifact_provenance_invalid: empty proposal"
        );
        return Ok(());
    }
    ensure!(
        mandatory_contract(contract),
        "artifact_provenance_invalid: unknown historical schema"
    );
    // No checked-in machine schema for implementation_backlog exists. Do not infer its fields from prose.
    ensure!(
        contract != "implementation_backlog",
        "artifact_provenance_invalid: backlog schema unproven"
    );
    let value: Value = serde_json::from_slice(bytes)
        .context("artifact_provenance_invalid: structured historical input")?;
    let object = value
        .as_object()
        .context("artifact_provenance_invalid: object required")?;
    if let Some(version) = object.get("schema_version") {
        ensure!(
            version.as_str() == Some(contract),
            "artifact_provenance_invalid: schema version"
        );
    }
    let required: &[&str] = match contract {
        "score_lift_backlog_v1" => &["review_pass_id", "source_proposal_artifact", "items"],
        "proposal_feedback_coverage_v1" => &[
            "proposal_revision_id",
            "source_review_pass_id",
            "backlog_items_addressed",
            "backlog_items_unresolved",
            "backlog_items_deferred",
            "backlog_items_disputed",
            "sections_changed",
            "factual_claims_added_or_corrected",
            "notes",
        ],
        "proposal_review_v1" => &[
            "agent_id",
            "role",
            "score",
            "decision",
            "verdict",
            "summary",
            "issues",
            "blocking_issues",
            "non_blocking_issues",
            "suggestions",
            "assumptions",
        ],
        "proposal_review_summary_v2" => &[
            "pass",
            "average_score",
            "aggregate_score",
            "min_individual_score",
            "blocker_count",
            "blocking_issues",
            "summary",
            "blocking_required_changes",
            "advisory_follow_ups",
            "recurring_themes",
            "decision",
        ],
        "implementation_self_assessment_v1" => &[
            "seemingly_complete",
            "remaining_tasks",
            "known_risks",
            "tests_run",
            "docs_impacted",
        ],
        "implementation_self_assessment_v2" => &[
            "implementation_complete",
            "verification_green",
            "remaining_code_tasks",
            "handoff_tasks",
            "known_risks",
            "tests_run",
            "docs_impacted",
        ],
        "audit_report_v1" => &[
            "status",
            "matches_proposal",
            "missing_items",
            "extra_items",
            "defects",
            "required_fixes",
        ],
        "security_report_v1" => &[
            "status",
            "critical",
            "high",
            "medium",
            "low",
            "findings",
            "required_fixes",
        ],
        "prepush_review_v1" => &[
            "status",
            "major_concerns",
            "cleanup_items",
            "test_coverage_notes",
            "release_note",
        ],
        "implementation_review_summary_v1" => &[
            "status",
            "open_blockers",
            "must_fix",
            "recommended_next_step",
        ],
        "proposal_decomposition_plan_v1" => &[
            "schema_version",
            "requires_split",
            "implementation_start_decision",
        ],
        "quality_gate_blocker_assessment_v1" => &["schema_version", "blockers"],
        "blocker_boundary_status_v1" => &[
            "schema_version",
            "status",
            "assessment_generation_id",
            "projection_integrity",
            "followup_proposal_required",
            "has_release_blocking_external_blockers",
            "has_no_release_blocking_external_blockers",
            "workflow_route_hint",
        ],
        "blocker_boundary_approval_request_v1" => &[
            "schema_version",
            "question",
            "allowed_decisions",
            "label_to_approval_state",
        ],
        "blocker_boundary_human_decision_v1" => &["schema_version", "decision_label"],
        "followup_proposal_seed_v1" => &["schema_version", "title", "problem", "proposed_scope"],
        _ => anyhow::bail!("artifact_provenance_invalid: historical schema adapter unavailable"),
    };
    ensure!(
        required
            .iter()
            .all(|key| object.get(*key).is_some_and(|v| !v.is_null())),
        "artifact_provenance_invalid: missing historical fields"
    );
    super::historical::boundary(contract, &value)?;
    if let Some(catalog) = catalog {
        let declaration = catalog["contracts"]
            .get(contract)
            .context("artifact_provenance_invalid: absent frozen schema")?;
        let declared = declaration["required_fields"]
            .as_array()
            .context("artifact_provenance_invalid: absent frozen required fields")?;
        let expected: BTreeSet<_> = required.iter().copied().collect();
        let actual: BTreeSet<_> = declared.iter().filter_map(Value::as_str).collect();
        ensure!(
            expected == actual && declared.len() == actual.len(),
            "artifact_provenance_invalid: changed historical schema"
        );
    }
    Ok(())
}

async fn lineage(pool: &SqlitePool, source: RunId) -> anyhow::Result<Vec<ContentDigest>> {
    let mut current = source.to_string();
    let mut seen = BTreeSet::new();
    let mut digests = Vec::new();
    loop {
        ensure!(
            seen.insert(current.clone()),
            "artifact_provenance_invalid: ancestry cycle"
        );
        let row: Option<(String, String)> = sqlx::query_as("SELECT source_run_id,manifest_sha256 FROM run_continuations WHERE successor_run_id=? AND phase='activated'")
            .bind(&current).fetch_optional(pool).await?;
        let Some((parent, digest)) = row else {
            break;
        };
        ensure!(
            digests.len() < 32,
            "artifact_provenance_invalid: ancestry exceeds 32 hops"
        );
        digests.push(snapshot_hash(&digest)?);
        current = parent;
    }
    Ok(digests)
}

#[derive(sqlx::FromRow)]
struct Carried {
    input_id: String,
    operation_id: String,
    successor_run_id: Option<String>,
    source_run_id: String,
    role: String,
    source_kind: String,
    source_id: String,
    original_artifact_id: String,
    source_schema: String,
    content_sha256: String,
    target_logical_name: String,
    target_relative_path: String,
    installed: bool,
    historical_relation_json: String,
}

async fn carried(pool: &SqlitePool, id: &str, owner: &str) -> anyhow::Result<Carried> {
    let input: Carried = sqlx::query_as("SELECT * FROM run_continuation_inputs WHERE input_id=?")
        .bind(id)
        .fetch_optional(pool)
        .await?
        .context("artifact_provenance_invalid: missing carried input")?;
    ensure!(
        input.installed && input.successor_run_id.as_deref() == Some(owner),
        "artifact_provenance_invalid: immediate carried owner"
    );
    let op: Option<(String, String, String)> = sqlx::query_as("SELECT source_run_id,successor_run_id,manifest_sha256 FROM run_continuations WHERE operation_id=? AND phase='activated'")
        .bind(&input.operation_id).fetch_optional(pool).await?;
    let (source, successor, digest) =
        op.context("artifact_provenance_invalid: inactive input operation")?;
    ensure!(
        source == input.source_run_id && successor == owner,
        "artifact_provenance_invalid: operation lineage"
    );
    snapshot_hash(&digest)?;
    Ok(input)
}

async fn from_carried(
    pool: &SqlitePool,
    source: &Run,
    roots: &Roots,
    id: Uuid,
    role: EntryRole,
    mandatory: bool,
    ancestors: Vec<ContentDigest>,
    deadline: Deadline,
) -> anyhow::Result<CanonicalSourceInput> {
    let first = carried(pool, &id.to_string(), &source.id.to_string()).await?;
    ensure!(
        role != EntryRole::ExecutionSeed || first.role == "execution_seed",
        "artifact_provenance_invalid: reference cannot seed"
    );
    validate_relative(&first.target_relative_path)?;
    let hash = snapshot_hash(&first.content_sha256)?;
    let root = SafeRoot::open(&roots.metadata)?;
    let bytes = read_regular(
        &root,
        &first.target_relative_path,
        InventoryLimits::default().max_file_bytes,
        deadline,
    )?;
    ensure!(
        ContentDigest::of(&bytes) == hash,
        "artifact_provenance_invalid: carried bytes"
    );
    let metadata = root.open_file(&first.target_relative_path)?.metadata()?;
    let mut cursor = carried(pool, &id.to_string(), &source.id.to_string()).await?;
    let mut seen = BTreeSet::new();
    let original;
    let schema;
    let mut relations = Vec::new();
    loop {
        deadline.check()?;
        ensure!(
            seen.len() < 32 && seen.insert(cursor.input_id.clone()),
            "artifact_provenance_invalid: carried ancestry cycle or budget"
        );
        ensure!(
            cursor.original_artifact_id == first.original_artifact_id
                && cursor.content_sha256 == first.content_sha256
                && cursor.source_schema == first.source_schema
                && cursor.target_logical_name == first.target_logical_name
                && (role != EntryRole::ExecutionSeed || cursor.role == "execution_seed"),
            "artifact_provenance_invalid: carried ancestry mismatch"
        );
        relations.push(serde_json::from_str::<Value>(
            &cursor.historical_relation_json,
        )?);
        match cursor.source_kind.as_str() {
            "carried_input" => {
                cursor = carried(pool, &cursor.source_id, &cursor.source_run_id).await?;
            }
            "artifact" => {
                let artifact_id =
                    Uuid::parse_str(&cursor.source_id).context("artifact_provenance_invalid")?;
                let artifact = db::repos::artifacts::find_by_id(pool, artifact_id.into())
                    .await?
                    .context("artifact_provenance_invalid: origin missing")?;
                ensure!(
                    cursor.source_id == first.original_artifact_id
                        && artifact.run_id.to_string() == cursor.source_run_id
                        && artifact.contract_id == first.source_schema
                        && artifact.name == first.target_logical_name
                        && artifact
                            .checksum_sha256
                            .as_deref()
                            .is_none_or(|h| h == first.content_sha256),
                    "artifact_provenance_invalid: origin mismatch"
                );
                let origin_run = db::repos::runs::find_by_id(pool, artifact.run_id)
                    .await?
                    .context("artifact_provenance_invalid")?;
                let origin_plan = super::legacy::compile_source(&origin_run)?;
                let catalog: Value = serde_json::from_str(&origin_plan.catalog_snapshot_json)?;
                let proof = if artifact.checksum_sha256.is_none()
                    || artifact.contract_id.ends_with(".output")
                    || !origin_plan.artifact_paths.contains_key(&artifact.name)
                    || ["implementation_backlog", "implementation_plan"]
                        .contains(&artifact.name.as_str())
                {
                    Some(
                        super::provenance::declared_output(
                            pool,
                            &origin_run,
                            &origin_plan,
                            &artifact,
                            true,
                        )
                        .await?,
                    )
                } else {
                    None
                };
                if artifact.checksum_sha256.is_none() {
                    ensure!(
                        proof.is_some()
                            && relations
                                .iter()
                                .all(|relation| relation["authority"] == "none"
                                    && relation["binding"] == "unproven"
                                    && relation["fresh_review_required"] == true
                                    && relation["provenance"]["content_binding"]
                                        == "fresh_observation_only"
                                    && relation["provenance"]["agent_execution_id"].as_str()
                                        == artifact.agent_execution_id.as_deref()),
                        "artifact_provenance_invalid: unproven original content binding"
                    );
                }
                schema = source_schema(&artifact, &bytes, &catalog, proof.as_ref())?;
                original = artifact;
                break;
            }
            _ => anyhow::bail!("artifact_provenance_invalid: unknown origin"),
        }
    }
    Ok(CanonicalSourceInput {
        reference: InputReference::CarriedInput { id },
        immediate_source_run_id: source.id,
        original_artifact_id: original.id.0,
        original_run_id: original.run_id,
        ancestor_manifest_sha256: ancestors,
        source_relative_path: first.target_relative_path,
        logical_name: first.target_logical_name.clone(),
        source_contract: first.source_schema.clone(),
        source_schema_version: schema,
        target_logical_name: first.target_logical_name,
        role,
        sha256: hash,
        bytes: bytes.len() as u64,
        mode: metadata.mode(),
        mandatory,
        historical_approval_relation: if relations[0].get("provenance").is_some() {
            relations[0].clone()
        } else {
            json!({"authority":"none", "binding":"unproven", "fresh_review_required":true})
        },
        source_root: roots.metadata.clone(),
    })
}

pub(super) fn verify_inputs(
    inputs: &[CanonicalSourceInput],
    deadline: Deadline,
) -> anyhow::Result<()> {
    for input in inputs {
        let root = SafeRoot::open(&input.source_root)?;
        let bytes = read_regular(
            &root,
            &input.source_relative_path,
            InventoryLimits::default().max_file_bytes,
            deadline,
        )?;
        let metadata = root.open_file(&input.source_relative_path)?.metadata()?;
        ensure!(
            ContentDigest::of(&bytes) == input.sha256
                && bytes.len() as u64 == input.bytes
                && metadata.mode() == input.mode,
            "source_changed: selected input"
        );
        root.verify_path(&input.source_root)?;
    }
    Ok(())
}
