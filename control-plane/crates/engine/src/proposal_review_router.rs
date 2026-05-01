//! P060: Deterministic proposal review router.
//!
//! Implements the scoring and selection algorithm defined in Proposal 060 §5.
//! This is a system.routing task — no LLM/provider is invoked.

use chrono::Utc;
use domain::ids::{RoutingReceiptId, SystemExecutionId};
use domain::routing::{
    AgentSelectionPlanV1, CompiledDynamicAgentBinding, IneligibleCandidate, InputSnapshotHashes,
    RejectedAlternative, ReviewRoutingMode, ReviewRoutingOptions, RoutingEvidenceRef,
    RoutingFailureKind, RoutingReceipt, RoutingReceiptStatus, ScoreTerms, SelectedAgent,
    SystemExecution, SystemExecutionStatus,
};
use sha2::{Digest, Sha256};

/// Maximum number of selected reviewers (P060 §5: hard-cap at 5).
const MAX_SELECTED_REVIEWERS: usize = 5;

/// Minimum number of selected reviewers in normal operation.
const MIN_SELECTED_REVIEWERS: usize = 2;

/// Score threshold for optional reviewer selection (P060 §5).
const SCORE_THRESHOLD: i64 = 6;

/// Evidence extracted from a proposal for routing decisions.
#[derive(Clone, Debug, Default)]
pub struct ProposalFingerprint {
    pub proposal_md5: String,
    pub stacks: Vec<String>,
    pub surfaces: Vec<String>,
    pub risks: Vec<String>,
    pub strong_keywords: Vec<String>,
    pub repo_signals: Vec<String>,
    pub cross_stack_dependencies: Vec<String>,
    pub baseline_gaps: Vec<String>,
    pub evidence_refs: Vec<RoutingEvidenceRef>,
}

/// The outcome of the routing algorithm.
#[derive(Clone, Debug)]
pub enum RoutingOutcome {
    /// Routing succeeded: plan + receipt + system execution.
    Success {
        plan: AgentSelectionPlanV1,
        receipt: RoutingReceipt,
        system_execution: SystemExecution,
    },
    /// Routing failed: receipt + system execution (no plan).
    Failure {
        failure_kind: RoutingFailureKind,
        receipt: RoutingReceipt,
        system_execution: SystemExecution,
        validation_failure_json: String,
    },
}

/// Run the deterministic routing algorithm.
///
/// Takes compiled candidate bindings, proposal fingerprint, and
/// routing options. Returns the routing outcome.
pub fn route_proposal_reviewers(
    run_id: domain::ids::RunId,
    stage_id: &str,
    attempt_id: i64,
    bindings: &[CompiledDynamicAgentBinding],
    fingerprint: &ProposalFingerprint,
    options: &ReviewRoutingOptions,
    input_hashes: &InputSnapshotHashes,
) -> RoutingOutcome {
    let system_execution_id = SystemExecutionId::new();
    let receipt_id = RoutingReceiptId::new();
    let now = Utc::now();

    // Step 1: Partition candidates into eligible and ineligible.
    let (eligible, ineligible) = partition_candidates(bindings);

    // Step 2: Validate overrides.
    if let Err(failure_kind) = validate_overrides(options, &eligible) {
        return make_failure(
            failure_kind,
            run_id,
            stage_id,
            attempt_id,
            system_execution_id,
            receipt_id,
            input_hashes,
            now,
        );
    }

    // Step 3: Score eligible candidates.
    let mut scored: Vec<ScoredCandidate> = eligible
        .iter()
        .map(|b| score_candidate(b, fingerprint, options))
        .collect();
    apply_overlap_penalties(&mut scored);

    // Step 4: Apply selection rules.
    let selection_result = match select_reviewers(&mut scored, options) {
        Ok(result) => result,
        Err(failure_kind) => {
            return make_failure(
                failure_kind,
                run_id,
                stage_id,
                attempt_id,
                system_execution_id,
                receipt_id,
                input_hashes,
                now,
            );
        }
    };
    let selected = selection_result.agents;
    let under_specified_fallback = selection_result.under_specified_fallback;
    let mandatory_overflow_pruned_ids: std::collections::HashSet<&str> = selection_result
        .mandatory_overflow_pruned_agent_ids
        .iter()
        .map(|id| id.as_str())
        .collect();

    // Step 5: Build rejected alternatives from unselected scored candidates.
    let selected_ids: std::collections::HashSet<&str> =
        selected.iter().map(|s| s.agent_id.as_str()).collect();

    let rejected: Vec<RejectedAlternative> = scored
        .iter()
        .filter(|c| !selected_ids.contains(c.agent_id.as_str()))
        .map(|c| RejectedAlternative {
            agent_id: c.agent_id.clone(),
            routing_id: c.routing_id.clone(),
            score: c.total_score(),
            reason: if mandatory_overflow_pruned_ids.contains(c.agent_id.as_str()) {
                "mandatory_overflow_pruned".into()
            } else if c.override_source.is_some() && selected_ids.len() >= MAX_SELECTED_REVIEWERS {
                "force_include_over_capacity".into()
            } else if c.total_score() < SCORE_THRESHOLD {
                "below_threshold".into()
            } else {
                "not_selected".into()
            },
            score_terms: c.terms.clone(),
        })
        .collect();

    // Step 6: Build warnings.
    let mut warnings = Vec::new();
    if under_specified_fallback {
        warnings.push("under_specified_selection".to_string());
    }
    if !mandatory_overflow_pruned_ids.is_empty() {
        warnings.push("mandatory_overflow_pruned".to_string());
    }

    // Step 7: Compute plan hash.
    let plan_hash = compute_plan_hash(&selected, input_hashes);

    let plan = AgentSelectionPlanV1 {
        schema_version: "1".into(),
        routing_rules_version: "1".into(),
        proposal_md5: fingerprint.proposal_md5.clone(),
        plan_hash: plan_hash.clone(),
        mode: options.mode.clone(),
        fingerprint: {
            let mut tags = Vec::new();
            tags.extend(fingerprint.stacks.clone());
            tags.extend(fingerprint.surfaces.clone());
            tags.extend(fingerprint.risks.clone());
            tags.sort();
            tags.dedup();
            tags
        },
        selected_agents: selected,
        rejected_alternatives: rejected,
        ineligible_candidates: ineligible,
        warnings,
        input_snapshot_hashes: input_hashes.clone(),
    };

    let receipt = RoutingReceipt {
        receipt_id,
        run_id,
        stage_id: stage_id.into(),
        attempt_id,
        system_execution_id,
        status: RoutingReceiptStatus::Succeeded,
        failure_kind: None,
        plan_hash: Some(plan_hash.clone()),
        input_snapshot_hashes_json: serde_json::to_string(input_hashes).ok(),
        operator_actions_json: operator_actions_json(options),
        created_at: now,
    };

    let system_execution = SystemExecution {
        id: system_execution_id,
        run_id,
        stage_id: stage_id.into(),
        attempt_id,
        task_id: "proposal_review_router".into(),
        task_type: "proposal_review_router".into(),
        status: SystemExecutionStatus::Succeeded,
        started_at: now,
        completed_at: Some(now),
        receipt_id: Some(receipt_id),
        plan_hash: Some(plan_hash),
        failure_kind: None,
    };

    RoutingOutcome::Success {
        plan,
        receipt,
        system_execution,
    }
}

// ── Internal helpers ─────────────────────────────────────────────────

struct ScoredCandidate {
    agent_id: String,
    routing_id: String,
    binding_id: String,
    mandatory: bool,
    override_source: Option<String>,
    terms: ScoreTerms,
    weights: domain::routing::ScoreWeights,
    rationale: String,
    evidence_refs: Vec<RoutingEvidenceRef>,
}

fn partition_candidates(
    bindings: &[CompiledDynamicAgentBinding],
) -> (Vec<&CompiledDynamicAgentBinding>, Vec<IneligibleCandidate>) {
    let mut eligible = Vec::new();
    let mut ineligible = Vec::new();

    for b in bindings {
        if !b.enabled_for_proposal_review {
            ineligible.push(IneligibleCandidate {
                agent_id: b.agent_id.clone(),
                routing_id: b.routing_metadata.routing_id.clone(),
                reason: "disabled_by_rollout_wave".into(),
            });
        } else {
            eligible.push(b);
        }
    }

    (eligible, ineligible)
}

fn validate_overrides(
    options: &ReviewRoutingOptions,
    eligible: &[&CompiledDynamicAgentBinding],
) -> Result<(), RoutingFailureKind> {
    let eligible_ids: std::collections::HashSet<&str> =
        eligible.iter().map(|b| b.agent_id.as_str()).collect();

    // Check force_include references exist in the eligible pool.
    for agent_id in &options.force_include {
        if !eligible_ids.contains(agent_id.as_str()) {
            return Err(RoutingFailureKind::UnknownAgent);
        }
    }

    // Check force_exclude doesn't collide with force_include.
    for agent_id in &options.force_exclude {
        if options.force_include.contains(agent_id) {
            return Err(RoutingFailureKind::OverrideConflict);
        }
    }

    Ok(())
}

fn score_candidate(
    binding: &CompiledDynamicAgentBinding,
    fingerprint: &ProposalFingerprint,
    options: &ReviewRoutingOptions,
) -> ScoredCandidate {
    let meta = &binding.routing_metadata;
    let mut terms = ScoreTerms::default();
    let mut evidence_refs = Vec::new();
    let mut rationale_parts = Vec::new();

    // Force include bonus.
    if options.force_include.contains(&binding.agent_id) {
        terms.force_include = 1;
        rationale_parts.push("force_include override".to_string());
    }

    // Stack matches.
    let stack_count = meta
        .stacks
        .iter()
        .filter(|s| fingerprint.stacks.contains(s))
        .count() as i64;
    terms.stack_matches = stack_count;
    if stack_count > 0 {
        rationale_parts.push(format!("{stack_count} stack match(es)"));
    }

    // Surface matches.
    let surface_count = meta
        .surfaces
        .iter()
        .filter(|s| fingerprint.surfaces.contains(s))
        .count() as i64;
    terms.surface_matches = surface_count;
    if surface_count > 0 {
        rationale_parts.push(format!("{surface_count} surface match(es)"));
    }

    // Risk matches.
    let risk_count = meta
        .risks
        .iter()
        .filter(|r| fingerprint.risks.contains(r))
        .count() as i64;
    terms.risk_matches = risk_count;
    if risk_count > 0 {
        rationale_parts.push(format!("{risk_count} risk match(es)"));
    }

    // Strong keyword matches.
    let keyword_tags = if meta.strong_proposal_keywords.is_empty() {
        &meta.capabilities
    } else {
        &meta.strong_proposal_keywords
    };
    let keyword_count = keyword_tags
        .iter()
        .filter(|c| fingerprint.strong_keywords.contains(c))
        .count() as i64;
    terms.strong_keyword_matches = keyword_count;

    // Repo signal matches.
    let repo_signal_tags: Vec<&String> = meta
        .strong_repo_files
        .iter()
        .chain(meta.strong_repo_symbols.iter())
        .collect();
    let repo_count = fingerprint
        .repo_signals
        .iter()
        .filter(|s| {
            repo_signal_tags.contains(s)
                || (repo_signal_tags.is_empty()
                    && (meta.stacks.contains(s) || meta.surfaces.contains(s)))
        })
        .count() as i64;
    terms.repo_signal_matches = repo_count;

    // Cross-stack dependency matches.
    let cross_stack_count = fingerprint
        .cross_stack_dependencies
        .iter()
        .filter(|d| meta.stacks.contains(d))
        .count() as i64;
    terms.cross_stack_dependency_matches = cross_stack_count;

    // Baseline gap matches.
    let baseline_count = fingerprint
        .baseline_gaps
        .iter()
        .filter(|g| meta.capabilities.contains(g) || meta.risks.contains(g))
        .count() as i64;
    terms.baseline_gap_matches = baseline_count;

    // Build evidence refs for matched items.
    for stack in &meta.stacks {
        if fingerprint.stacks.contains(stack) {
            evidence_refs.push(RoutingEvidenceRef {
                evidence_id: format!("stack:{}", stack),
                evidence_type: "stack_match".into(),
                hash: hash_string(stack),
                normalized_value: Some(stack.clone()),
                path: None,
                symbol: None,
                span: None,
            });
        }
    }
    for surface in &meta.surfaces {
        if fingerprint.surfaces.contains(surface) {
            evidence_refs.push(RoutingEvidenceRef {
                evidence_id: format!("surface:{}", surface),
                evidence_type: "surface_match".into(),
                hash: hash_string(surface),
                normalized_value: Some(surface.clone()),
                path: None,
                symbol: None,
                span: None,
            });
        }
    }
    for risk in &meta.risks {
        if fingerprint.risks.contains(risk) {
            evidence_refs.push(RoutingEvidenceRef {
                evidence_id: format!("risk:{}", risk),
                evidence_type: "risk_match".into(),
                hash: hash_string(risk),
                normalized_value: Some(risk.clone()),
                path: None,
                symbol: None,
                span: None,
            });
        }
    }
    for keyword in keyword_tags {
        if fingerprint.strong_keywords.contains(keyword) {
            evidence_refs.push(RoutingEvidenceRef {
                evidence_id: format!("keyword:{}", keyword),
                evidence_type: "strong_keyword_match".into(),
                hash: hash_string(keyword),
                normalized_value: Some((*keyword).clone()),
                path: None,
                symbol: None,
                span: None,
            });
        }
    }
    for signal in &fingerprint.repo_signals {
        if repo_signal_tags.contains(&signal)
            || (repo_signal_tags.is_empty()
                && (meta.stacks.contains(signal) || meta.surfaces.contains(signal)))
        {
            evidence_refs.push(RoutingEvidenceRef {
                evidence_id: format!("repo:{}", signal),
                evidence_type: "repo_signal_match".into(),
                hash: hash_string(signal),
                normalized_value: Some(signal.clone()),
                path: None,
                symbol: None,
                span: None,
            });
        }
    }
    for dependency in &fingerprint.cross_stack_dependencies {
        if meta.stacks.contains(dependency) {
            evidence_refs.push(RoutingEvidenceRef {
                evidence_id: format!("cross_stack:{}", dependency),
                evidence_type: "cross_stack_dependency_match".into(),
                hash: hash_string(dependency),
                normalized_value: Some(dependency.clone()),
                path: None,
                symbol: None,
                span: None,
            });
        }
    }
    for gap in &fingerprint.baseline_gaps {
        if meta.capabilities.contains(gap) || meta.risks.contains(gap) {
            evidence_refs.push(RoutingEvidenceRef {
                evidence_id: format!("baseline_gap:{}", gap),
                evidence_type: "baseline_gap_match".into(),
                hash: hash_string(gap),
                normalized_value: Some(gap.clone()),
                path: None,
                symbol: None,
                span: None,
            });
        }
    }

    // Mandatory check.
    let mandatory = meta.mandatory_when.iter().any(|rule| {
        // Simple rule matching: mandatory_when rules are tag-match expressions.
        fingerprint.risks.contains(rule)
            || fingerprint.stacks.contains(rule)
            || fingerprint.surfaces.contains(rule)
    });

    if rationale_parts.is_empty() {
        rationale_parts.push("no direct matches".to_string());
    }

    ScoredCandidate {
        agent_id: binding.agent_id.clone(),
        routing_id: meta.routing_id.clone(),
        binding_id: binding.binding_id.clone(),
        mandatory,
        override_source: if options.force_include.contains(&binding.agent_id) {
            Some("force_include".into())
        } else {
            None
        },
        terms,
        weights: meta.score_weights.clone(),
        rationale: rationale_parts.join("; "),
        evidence_refs,
    }
}

impl ScoredCandidate {
    fn total_score(&self) -> i64 {
        self.terms.total_with_weights(&self.weights)
    }

    fn threshold_score(&self) -> i64 {
        let mut terms = self.terms.clone();
        terms.overlap_penalty = 0;
        terms.total_with_weights(&self.weights)
    }
}

fn apply_overlap_penalties(scored: &mut [ScoredCandidate]) {
    let mut order: Vec<usize> = (0..scored.len()).collect();
    order.sort_by(|a, b| {
        scored[*b]
            .total_score()
            .cmp(&scored[*a].total_score())
            .then_with(|| scored[*a].agent_id.cmp(&scored[*b].agent_id))
    });

    for position in 0..order.len() {
        let idx = order[position];
        let mut covered = std::collections::HashSet::new();
        for earlier_idx in order.iter().take(position) {
            for evidence in &scored[*earlier_idx].evidence_refs {
                if matches!(
                    evidence.evidence_type.as_str(),
                    "stack_match" | "surface_match" | "risk_match"
                ) {
                    covered.insert(evidence.evidence_id.clone());
                }
            }
        }
        let overlap = scored[idx]
            .evidence_refs
            .iter()
            .filter(|evidence| covered.contains(&evidence.evidence_id))
            .count() as i64;
        scored[idx].terms.overlap_penalty = overlap;
        if overlap > 0 {
            scored[idx]
                .rationale
                .push_str(&format!("; {overlap} overlap penalty term(s)"));
        }
    }
}

/// Selection result with metadata about how selection was achieved.
struct SelectionResult {
    agents: Vec<SelectedAgent>,
    under_specified_fallback: bool,
    mandatory_overflow_pruned_agent_ids: Vec<String>,
}

fn select_reviewers(
    scored: &mut [ScoredCandidate],
    options: &ReviewRoutingOptions,
) -> Result<SelectionResult, RoutingFailureKind> {
    // Collect mandatory reviewers.
    let mut selected: Vec<&ScoredCandidate> = scored.iter().filter(|c| c.mandatory).collect();
    let mut mandatory_overflow_pruned_agent_ids = Vec::new();
    if selected.len() > MAX_SELECTED_REVIEWERS {
        selected.sort_by(|a, b| {
            b.total_score()
                .cmp(&a.total_score())
                .then_with(|| a.agent_id.cmp(&b.agent_id))
        });
        mandatory_overflow_pruned_agent_ids = selected
            .iter()
            .skip(MAX_SELECTED_REVIEWERS)
            .map(|c| c.agent_id.clone())
            .collect();
        selected.truncate(MAX_SELECTED_REVIEWERS);
    }

    // Add force_include candidates that aren't already mandatory.
    for c in scored.iter() {
        if selected.len() >= MAX_SELECTED_REVIEWERS {
            break;
        }
        if c.override_source.is_some()
            && !selected.iter().any(|s| s.agent_id == c.agent_id)
            && !options.force_exclude.contains(&c.agent_id)
        {
            selected.push(c);
        }
    }

    // Apply force_exclude to non-mandatory reviewers.
    selected.retain(|c| !options.force_exclude.contains(&c.agent_id) || c.mandatory);

    // Fill remaining slots by score (descending), threshold >= SCORE_THRESHOLD.
    let mut optional_candidates: Vec<&ScoredCandidate> = scored
        .iter()
        .filter(|c| {
            !c.mandatory
                && c.override_source.is_none()
                && !options.force_exclude.contains(&c.agent_id)
                && c.threshold_score() >= SCORE_THRESHOLD
                && !selected.iter().any(|s| s.agent_id == c.agent_id)
        })
        .collect();

    // Sort by score descending, then by agent_id alphabetically for ties.
    optional_candidates.sort_by(|a, b| {
        b.total_score()
            .cmp(&a.total_score())
            .then_with(|| a.agent_id.cmp(&b.agent_id))
    });

    for c in optional_candidates {
        if selected.len() >= MAX_SELECTED_REVIEWERS {
            break;
        }
        selected.push(c);
    }

    // Under-specified fallback: if fewer than MIN_SELECTED_REVIEWERS,
    // select product_owner + architect.
    let mut under_specified_fallback = false;
    if selected.len() < MIN_SELECTED_REVIEWERS {
        under_specified_fallback = true;
        let fallback_ids = [
            "proposal_reviewer_product_owner",
            "proposal_reviewer_architect",
        ];
        for fallback_id in &fallback_ids {
            if !selected.iter().any(|s| s.agent_id == *fallback_id) {
                if let Some(c) = scored.iter().find(|c| c.agent_id == *fallback_id) {
                    selected.push(c);
                }
            }
        }
    }

    // Sort selected: mandatory first, then by score descending, then alphabetically.
    selected.sort_by(|a, b| {
        b.mandatory
            .cmp(&a.mandatory)
            .then_with(|| b.total_score().cmp(&a.total_score()))
            .then_with(|| a.agent_id.cmp(&b.agent_id))
    });

    Ok(SelectionResult {
        agents: selected
            .into_iter()
            .map(|c| SelectedAgent {
                agent_id: c.agent_id.clone(),
                routing_id: c.routing_id.clone(),
                score: c.total_score(),
                mandatory: c.mandatory,
                override_source: c.override_source.clone(),
                score_terms: c.terms.clone(),
                rationale: c.rationale.clone(),
                evidence_refs: c.evidence_refs.clone(),
                materialization_binding_id: c.binding_id.clone(),
            })
            .collect(),
        under_specified_fallback,
        mandatory_overflow_pruned_agent_ids,
    })
}

fn make_failure(
    failure_kind: RoutingFailureKind,
    run_id: domain::ids::RunId,
    stage_id: &str,
    attempt_id: i64,
    system_execution_id: SystemExecutionId,
    receipt_id: RoutingReceiptId,
    input_hashes: &InputSnapshotHashes,
    now: chrono::DateTime<chrono::Utc>,
) -> RoutingOutcome {
    let failure_json = serde_json::json!({
        "receipt_id": receipt_id.to_string(),
        "failure_kind": failure_kind.to_string(),
    })
    .to_string();

    let receipt = RoutingReceipt {
        receipt_id,
        run_id,
        stage_id: stage_id.into(),
        attempt_id,
        system_execution_id,
        status: RoutingReceiptStatus::Failed,
        failure_kind: Some(failure_kind.to_string()),
        plan_hash: None,
        input_snapshot_hashes_json: serde_json::to_string(input_hashes).ok(),
        operator_actions_json: None,
        created_at: now,
    };

    let system_execution = SystemExecution {
        id: system_execution_id,
        run_id,
        stage_id: stage_id.into(),
        attempt_id,
        task_id: "proposal_review_router".into(),
        task_type: "proposal_review_router".into(),
        status: SystemExecutionStatus::Failed,
        started_at: now,
        completed_at: Some(now),
        receipt_id: Some(receipt_id),
        plan_hash: None,
        failure_kind: Some(failure_kind.to_string()),
    };

    RoutingOutcome::Failure {
        failure_kind,
        receipt,
        system_execution,
        validation_failure_json: failure_json,
    }
}

pub fn build_legacy_fixed_routing_outcome(
    run_id: domain::ids::RunId,
    stage_id: &str,
    attempt_id: i64,
    bindings: &[CompiledDynamicAgentBinding],
    fingerprint: &ProposalFingerprint,
    input_hashes: &InputSnapshotHashes,
) -> RoutingOutcome {
    let system_execution_id = SystemExecutionId::new();
    let receipt_id = RoutingReceiptId::new();
    let now = Utc::now();
    let (eligible, ineligible) = partition_candidates(bindings);
    let fixed_ids = [
        "proposal_reviewer_product_owner",
        "proposal_reviewer_ux",
        "proposal_reviewer_ui",
        "proposal_reviewer_architect",
    ];

    let mut selected = Vec::new();
    for fixed_id in fixed_ids {
        let Some(binding) = eligible.iter().find(|binding| binding.agent_id == fixed_id) else {
            return make_failure(
                RoutingFailureKind::UnknownAgent,
                run_id,
                stage_id,
                attempt_id,
                system_execution_id,
                receipt_id,
                input_hashes,
                now,
            );
        };
        selected.push(SelectedAgent {
            agent_id: binding.agent_id.clone(),
            routing_id: binding.routing_metadata.routing_id.clone(),
            score: 0,
            mandatory: true,
            override_source: Some("legacy_fixed".into()),
            score_terms: ScoreTerms::default(),
            rationale: "legacy fixed reviewer quartet".into(),
            evidence_refs: vec![],
            materialization_binding_id: binding.binding_id.clone(),
        });
    }

    let selected_ids: std::collections::HashSet<&str> = selected
        .iter()
        .map(|agent| agent.agent_id.as_str())
        .collect();
    let rejected_alternatives = eligible
        .iter()
        .filter(|binding| !selected_ids.contains(binding.agent_id.as_str()))
        .map(|binding| RejectedAlternative {
            agent_id: binding.agent_id.clone(),
            routing_id: binding.routing_metadata.routing_id.clone(),
            score: 0,
            reason: "legacy_fixed_not_in_fixed_quartet".into(),
            score_terms: ScoreTerms::default(),
        })
        .collect();

    let plan_hash = compute_plan_hash(&selected, input_hashes);
    let plan = AgentSelectionPlanV1 {
        schema_version: "1".into(),
        routing_rules_version: "1".into(),
        proposal_md5: fingerprint.proposal_md5.clone(),
        plan_hash: plan_hash.clone(),
        mode: ReviewRoutingMode::LegacyFixed,
        fingerprint: vec!["legacy_fixed".into()],
        selected_agents: selected,
        rejected_alternatives,
        ineligible_candidates: ineligible,
        warnings: vec!["legacy_fixed_mode".into()],
        input_snapshot_hashes: input_hashes.clone(),
    };

    let receipt = RoutingReceipt {
        receipt_id,
        run_id,
        stage_id: stage_id.into(),
        attempt_id,
        system_execution_id,
        status: RoutingReceiptStatus::Succeeded,
        failure_kind: None,
        plan_hash: Some(plan_hash.clone()),
        input_snapshot_hashes_json: serde_json::to_string(input_hashes).ok(),
        operator_actions_json: None,
        created_at: now,
    };
    let system_execution = SystemExecution {
        id: system_execution_id,
        run_id,
        stage_id: stage_id.into(),
        attempt_id,
        task_id: "proposal_review_router".into(),
        task_type: "proposal_review_router".into(),
        status: SystemExecutionStatus::Succeeded,
        started_at: now,
        completed_at: Some(now),
        receipt_id: Some(receipt_id),
        plan_hash: Some(plan_hash),
        failure_kind: None,
    };

    RoutingOutcome::Success {
        plan,
        receipt,
        system_execution,
    }
}

fn operator_actions_json(options: &ReviewRoutingOptions) -> Option<String> {
    if options.force_include.is_empty()
        && options.force_exclude.is_empty()
        && options.override_reason.is_none()
        && options.operator_id.is_none()
        && options.created_at.is_none()
    {
        return None;
    }
    serde_json::to_string(&serde_json::json!({
        "force_include": options.force_include,
        "force_exclude": options.force_exclude,
        "override_reason": options.override_reason,
        "operator_id": options.operator_id,
        "created_at": options.created_at.map(|dt| dt.to_rfc3339()),
    }))
    .ok()
}

fn compute_plan_hash(selected: &[SelectedAgent], input_hashes: &InputSnapshotHashes) -> String {
    let mut hasher = Sha256::new();
    for agent in selected {
        hasher.update(agent.agent_id.as_bytes());
        hasher.update(agent.score.to_le_bytes());
    }
    hasher.update(input_hashes.workflow_snapshot_hash.as_bytes());
    hasher.update(input_hashes.catalog_snapshot_hash.as_bytes());
    hasher.update(input_hashes.routing_metadata_hash.as_bytes());
    hasher.update(input_hashes.candidate_binding_hash.as_bytes());
    hasher.update(input_hashes.evidence_hash.as_bytes());
    if let Some(oh) = &input_hashes.override_hash {
        hasher.update(oh.as_bytes());
    }
    format!("{:x}", hasher.finalize())
}

fn hash_string(s: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(s.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// P060: Resolve selected reviewer artifacts for aggregation.
///
/// Given an AgentSelectionPlanV1, filters the available artifacts to only
/// those produced by selected reviewers with the specified output_contract.
/// Ignores stale artifacts from unselected or previous reviewers.
pub fn resolve_selected_outputs(
    plan: &AgentSelectionPlanV1,
    available_artifacts: &[AvailableArtifact],
    output_contract: &str,
) -> SelectedOutputsResult {
    let selected_ids: std::collections::HashSet<&str> = plan
        .selected_agents
        .iter()
        .map(|a| a.agent_id.as_str())
        .collect();

    let mut selected_artifacts = Vec::new();
    let mut ignored_artifacts = Vec::new();

    for artifact in available_artifacts {
        if artifact.contract_id == output_contract
            && selected_ids.contains(artifact.agent_id.as_str())
        {
            selected_artifacts.push(artifact.clone());
        } else if artifact.contract_id == output_contract {
            ignored_artifacts.push(artifact.clone());
        }
    }

    SelectedOutputsResult {
        selected_review_artifacts: selected_artifacts,
        selected_reviewer_ids: plan
            .selected_agents
            .iter()
            .map(|a| a.agent_id.clone())
            .collect(),
        reviewer_count: plan.selected_agents.len(),
        selection_plan_hash: plan.plan_hash.clone(),
        legacy_fixed_mode: plan.mode == ReviewRoutingMode::LegacyFixed,
        ignored_artifacts,
    }
}

/// An artifact available for selected_outputs_from resolution.
#[derive(Clone, Debug)]
pub struct AvailableArtifact {
    pub artifact_id: String,
    pub agent_id: String,
    pub contract_id: String,
    pub file_path: String,
}

/// Result of selected_outputs_from resolution.
#[derive(Clone, Debug)]
pub struct SelectedOutputsResult {
    pub selected_review_artifacts: Vec<AvailableArtifact>,
    pub selected_reviewer_ids: Vec<String>,
    pub reviewer_count: usize,
    pub selection_plan_hash: String,
    pub legacy_fixed_mode: bool,
    pub ignored_artifacts: Vec<AvailableArtifact>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::ids::RunId;
    use domain::routing::RoutingMetadata;

    fn make_binding(
        agent_id: &str,
        routing_id: &str,
        stacks: &[&str],
        surfaces: &[&str],
        risks: &[&str],
        enabled: bool,
        rollout_wave: &str,
    ) -> CompiledDynamicAgentBinding {
        CompiledDynamicAgentBinding {
            binding_id: format!("bind-{agent_id}"),
            agent_id: agent_id.into(),
            resolved_agent_snapshot_json: "{}".into(),
            output_contracts: vec!["proposal_review_v1".into()],
            permission_hash: "ph".into(),
            worktree_hash: "wh".into(),
            mcp_profile_hash: "mh".into(),
            skill_hash: "sh".into(),
            routing_metadata_hash: "rmh".into(),
            enabled_for_proposal_review: enabled,
            rollout_wave: rollout_wave.into(),
            catalog_snapshot_hash: "csh".into(),
            routing_metadata: RoutingMetadata {
                routing_id: routing_id.into(),
                family: "proposal_reviewer".into(),
                capabilities: vec![],
                stacks: stacks.iter().map(|s| s.to_string()).collect(),
                surfaces: surfaces.iter().map(|s| s.to_string()).collect(),
                risks: risks.iter().map(|r| r.to_string()).collect(),
                enabled_for_proposal_review: enabled,
                rollout_wave: rollout_wave.into(),
                mandatory_when: vec![],
                usually_pair_with: vec![],
                close_alternatives: vec![],
                strong_proposal_keywords: vec![],
                strong_repo_files: vec![],
                strong_repo_symbols: vec![],
                score_weights: Default::default(),
            },
        }
    }

    fn default_input_hashes() -> InputSnapshotHashes {
        InputSnapshotHashes {
            workflow_snapshot_hash: "wsh".into(),
            catalog_snapshot_hash: "csh".into(),
            routing_metadata_hash: "rmh".into(),
            candidate_binding_hash: "cbh".into(),
            evidence_hash: "eh".into(),
            override_hash: None,
        }
    }

    #[test]
    fn ui_macos_proposal_selects_ui_and_macos() {
        let bindings = vec![
            make_binding(
                "proposal_reviewer_ui",
                "ui",
                &[],
                &["macos", "swiftui"],
                &[],
                true,
                "existing",
            ),
            make_binding(
                "proposal_reviewer_macos",
                "macos",
                &["swift"],
                &["macos"],
                &[],
                true,
                "phase_3_core",
            ),
            make_binding(
                "proposal_reviewer_rust_architect",
                "rust_arch",
                &["rust"],
                &[],
                &[],
                true,
                "phase_3_core",
            ),
            make_binding(
                "proposal_reviewer_product_owner",
                "po",
                &[],
                &[],
                &[],
                true,
                "existing",
            ),
            make_binding(
                "proposal_reviewer_architect",
                "arch",
                &[],
                &[],
                &[],
                true,
                "existing",
            ),
        ];

        let fingerprint = ProposalFingerprint {
            proposal_md5: "abc".into(),
            stacks: vec!["swift".into()],
            surfaces: vec!["macos".into(), "swiftui".into()],
            risks: vec![],
            strong_keywords: vec![],
            repo_signals: vec![],
            cross_stack_dependencies: vec![],
            baseline_gaps: vec![],
            evidence_refs: vec![],
        };

        let options = ReviewRoutingOptions {
            mode: ReviewRoutingMode::Dynamic,
            ..Default::default()
        };

        let outcome = route_proposal_reviewers(
            RunId::new(),
            "proposal_reviewed",
            1,
            &bindings,
            &fingerprint,
            &options,
            &default_input_hashes(),
        );

        match outcome {
            RoutingOutcome::Success { plan, .. } => {
                let ids: Vec<&str> = plan
                    .selected_agents
                    .iter()
                    .map(|a| a.agent_id.as_str())
                    .collect();
                assert!(
                    ids.contains(&"proposal_reviewer_ui"),
                    "expected UI reviewer, got {:?}",
                    ids
                );
                assert!(
                    ids.contains(&"proposal_reviewer_macos"),
                    "expected macOS reviewer, got {:?}",
                    ids
                );
                assert!(
                    !ids.contains(&"proposal_reviewer_rust_architect"),
                    "rust_architect should not be selected for UI proposal"
                );
                assert!(plan.selected_agents.len() >= 2);
                assert!(plan.selected_agents.len() <= 5);
            }
            RoutingOutcome::Failure { failure_kind, .. } => {
                panic!("Expected success, got failure: {failure_kind}");
            }
        }
    }

    #[test]
    fn rust_proposal_selects_rust_architect() {
        let bindings = vec![
            make_binding(
                "proposal_reviewer_rust_architect",
                "rust_arch",
                &["rust"],
                &["control-plane"],
                &["retry", "resume"],
                true,
                "phase_3_core",
            ),
            make_binding(
                "proposal_reviewer_reliability",
                "reliability",
                &[],
                &[],
                &["retry", "resume", "recovery"],
                true,
                "phase_3_core",
            ),
            make_binding(
                "proposal_reviewer_ui",
                "ui",
                &[],
                &["macos", "swiftui"],
                &[],
                true,
                "existing",
            ),
            make_binding(
                "proposal_reviewer_product_owner",
                "po",
                &[],
                &[],
                &[],
                true,
                "existing",
            ),
            make_binding(
                "proposal_reviewer_architect",
                "arch",
                &[],
                &[],
                &[],
                true,
                "existing",
            ),
        ];

        let fingerprint = ProposalFingerprint {
            proposal_md5: "def".into(),
            stacks: vec!["rust".into()],
            surfaces: vec!["control-plane".into()],
            risks: vec!["retry".into(), "resume".into()],
            ..Default::default()
        };

        let options = ReviewRoutingOptions {
            mode: ReviewRoutingMode::Dynamic,
            ..Default::default()
        };

        let outcome = route_proposal_reviewers(
            RunId::new(),
            "proposal_reviewed",
            1,
            &bindings,
            &fingerprint,
            &options,
            &default_input_hashes(),
        );

        match outcome {
            RoutingOutcome::Success { plan, .. } => {
                let ids: Vec<&str> = plan
                    .selected_agents
                    .iter()
                    .map(|a| a.agent_id.as_str())
                    .collect();
                assert!(ids.contains(&"proposal_reviewer_rust_architect"));
                assert!(ids.contains(&"proposal_reviewer_reliability"));
                assert!(!ids.contains(&"proposal_reviewer_ui"));
            }
            RoutingOutcome::Failure { failure_kind, .. } => {
                panic!("Expected success, got failure: {failure_kind}");
            }
        }
    }

    #[test]
    fn selected_agent_contains_evidence_refs_for_each_score_term() {
        let mut binding = make_binding(
            "proposal_reviewer_reliability",
            "reliability",
            &["rust-backend"],
            &["background-work"],
            &["availability-sensitive"],
            true,
            "phase_3_core",
        );
        binding.routing_metadata.capabilities = vec!["retry".into(), "recovery".into()];

        let fingerprint = ProposalFingerprint {
            proposal_md5: "evidence".into(),
            stacks: vec!["rust-backend".into()],
            surfaces: vec!["background-work".into()],
            risks: vec!["availability-sensitive".into()],
            strong_keywords: vec!["retry".into()],
            repo_signals: vec!["rust-backend".into()],
            cross_stack_dependencies: vec!["rust-backend".into()],
            baseline_gaps: vec!["recovery".into()],
            evidence_refs: vec![],
        };

        let outcome = route_proposal_reviewers(
            RunId::new(),
            "proposal_reviewed",
            1,
            &[
                binding,
                make_binding(
                    "proposal_reviewer_product_owner",
                    "po",
                    &[],
                    &[],
                    &[],
                    true,
                    "existing",
                ),
                make_binding(
                    "proposal_reviewer_architect",
                    "arch",
                    &[],
                    &[],
                    &[],
                    true,
                    "existing",
                ),
            ],
            &fingerprint,
            &ReviewRoutingOptions::default(),
            &default_input_hashes(),
        );

        let RoutingOutcome::Success { plan, .. } = outcome else {
            panic!("expected successful routing");
        };
        let reliability = plan
            .selected_agents
            .iter()
            .find(|agent| agent.agent_id == "proposal_reviewer_reliability")
            .expect("reliability reviewer should be selected");
        let evidence_types: std::collections::HashSet<&str> = reliability
            .evidence_refs
            .iter()
            .map(|evidence| evidence.evidence_type.as_str())
            .collect();

        for expected in [
            "stack_match",
            "surface_match",
            "risk_match",
            "strong_keyword_match",
            "repo_signal_match",
            "cross_stack_dependency_match",
            "baseline_gap_match",
        ] {
            assert!(
                evidence_types.contains(expected),
                "missing evidence ref for {expected}: {:?}",
                reliability.evidence_refs
            );
        }
    }

    #[test]
    fn disabled_reviewer_is_ineligible() {
        let bindings = vec![
            make_binding(
                "proposal_reviewer_ios",
                "ios",
                &["swift"],
                &["ios"],
                &[],
                false,
                "later_wave",
            ),
            make_binding(
                "proposal_reviewer_product_owner",
                "po",
                &[],
                &[],
                &[],
                true,
                "existing",
            ),
            make_binding(
                "proposal_reviewer_architect",
                "arch",
                &[],
                &[],
                &[],
                true,
                "existing",
            ),
        ];

        let fingerprint = ProposalFingerprint {
            proposal_md5: "ghi".into(),
            stacks: vec!["swift".into()],
            surfaces: vec!["ios".into()],
            ..Default::default()
        };

        let options = ReviewRoutingOptions {
            mode: ReviewRoutingMode::Dynamic,
            ..Default::default()
        };

        let outcome = route_proposal_reviewers(
            RunId::new(),
            "proposal_reviewed",
            1,
            &bindings,
            &fingerprint,
            &options,
            &default_input_hashes(),
        );

        match outcome {
            RoutingOutcome::Success { plan, .. } => {
                assert_eq!(plan.ineligible_candidates.len(), 1);
                assert_eq!(
                    plan.ineligible_candidates[0].agent_id,
                    "proposal_reviewer_ios"
                );
                assert_eq!(
                    plan.ineligible_candidates[0].reason,
                    "disabled_by_rollout_wave"
                );
                // Falls back to product_owner + architect.
                let ids: Vec<&str> = plan
                    .selected_agents
                    .iter()
                    .map(|a| a.agent_id.as_str())
                    .collect();
                assert!(ids.contains(&"proposal_reviewer_product_owner"));
                assert!(ids.contains(&"proposal_reviewer_architect"));
            }
            RoutingOutcome::Failure { .. } => panic!("Expected success with fallback"),
        }
    }

    #[test]
    fn force_exclude_on_mandatory_is_retained() {
        let mut binding = make_binding(
            "proposal_reviewer_security",
            "security",
            &[],
            &[],
            &["security"],
            true,
            "phase_3_core",
        );
        binding.routing_metadata.mandatory_when = vec!["security".into()];

        let bindings = vec![
            binding,
            make_binding(
                "proposal_reviewer_product_owner",
                "po",
                &[],
                &[],
                &[],
                true,
                "existing",
            ),
            make_binding(
                "proposal_reviewer_architect",
                "arch",
                &[],
                &[],
                &[],
                true,
                "existing",
            ),
        ];

        let fingerprint = ProposalFingerprint {
            proposal_md5: "jkl".into(),
            risks: vec!["security".into()],
            ..Default::default()
        };

        let options = ReviewRoutingOptions {
            mode: ReviewRoutingMode::Dynamic,
            force_exclude: vec!["proposal_reviewer_security".into()],
            ..Default::default()
        };

        let outcome = route_proposal_reviewers(
            RunId::new(),
            "proposal_reviewed",
            1,
            &bindings,
            &fingerprint,
            &options,
            &default_input_hashes(),
        );

        // Per P060 §5: force_exclude may not remove mandatory reviewers.
        // The mandatory reviewer should still be selected.
        match outcome {
            RoutingOutcome::Success { plan, .. } => {
                let ids: Vec<&str> = plan
                    .selected_agents
                    .iter()
                    .map(|a| a.agent_id.as_str())
                    .collect();
                assert!(
                    ids.contains(&"proposal_reviewer_security"),
                    "mandatory reviewer must survive force_exclude"
                );
            }
            RoutingOutcome::Failure { .. } => panic!("Expected success"),
        }
    }

    #[test]
    fn overlap_penalty_is_applied_to_lower_ranked_overlapping_candidates() {
        let bindings = vec![
            make_binding(
                "proposal_reviewer_rust_architect",
                "rust_arch",
                &["rust-backend"],
                &["persistence"],
                &["availability-sensitive", "backward-compatibility"],
                true,
                "phase_3_core",
            ),
            make_binding(
                "proposal_reviewer_reliability",
                "reliability",
                &["rust-backend"],
                &["persistence"],
                &["availability-sensitive", "idempotency"],
                true,
                "phase_3_core",
            ),
            make_binding(
                "proposal_reviewer_product_owner",
                "po",
                &[],
                &[],
                &[],
                true,
                "existing",
            ),
        ];
        let fingerprint = ProposalFingerprint {
            proposal_md5: "overlap".into(),
            stacks: vec!["rust-backend".into()],
            surfaces: vec!["persistence".into()],
            risks: vec![
                "availability-sensitive".into(),
                "backward-compatibility".into(),
                "idempotency".into(),
            ],
            ..Default::default()
        };

        let outcome = route_proposal_reviewers(
            RunId::new(),
            "proposal_reviewed",
            1,
            &bindings,
            &fingerprint,
            &ReviewRoutingOptions::default(),
            &default_input_hashes(),
        );

        let RoutingOutcome::Success { plan, .. } = outcome else {
            panic!("expected successful routing");
        };
        let overlapping = plan
            .selected_agents
            .iter()
            .find(|agent| agent.score_terms.overlap_penalty > 0)
            .expect("one selected lower-ranked reviewer should carry overlap penalty");
        assert!(
            overlapping.score_terms.overlap_penalty > 0,
            "lower-ranked overlapping candidate must carry an explicit overlap penalty"
        );
        let mut unpenalized_terms = overlapping.score_terms.clone();
        unpenalized_terms.overlap_penalty = 0;
        assert!(
            overlapping.score < unpenalized_terms.total(),
            "selected score must reflect the overlap penalty"
        );
    }

    #[test]
    fn override_reason_is_recorded_in_routing_receipt_operator_actions() {
        let bindings = vec![
            make_binding(
                "proposal_reviewer_security",
                "security",
                &[],
                &[],
                &["security-sensitive"],
                true,
                "phase_3_core",
            ),
            make_binding(
                "proposal_reviewer_product_owner",
                "po",
                &[],
                &[],
                &[],
                true,
                "existing",
            ),
            make_binding(
                "proposal_reviewer_architect",
                "arch",
                &[],
                &[],
                &[],
                true,
                "existing",
            ),
        ];
        let options = ReviewRoutingOptions {
            force_include: vec!["proposal_reviewer_security".into()],
            override_reason: Some("Security-sensitive API".into()),
            operator_id: Some("operator-1".into()),
            ..Default::default()
        };

        let outcome = route_proposal_reviewers(
            RunId::new(),
            "proposal_reviewed",
            1,
            &bindings,
            &ProposalFingerprint::default(),
            &options,
            &default_input_hashes(),
        );

        let RoutingOutcome::Success { receipt, .. } = outcome else {
            panic!("expected successful routing");
        };
        let actions: serde_json::Value = serde_json::from_str(
            receipt
                .operator_actions_json
                .as_deref()
                .expect("operator override reason must be persisted"),
        )
        .expect("operator actions JSON should parse");
        assert_eq!(actions["override_reason"], "Security-sensitive API");
        assert_eq!(actions["operator_id"], "operator-1");
        assert_eq!(actions["force_include"][0], "proposal_reviewer_security");
    }

    #[test]
    fn override_conflict_force_include_exclude_same_agent() {
        let bindings = vec![make_binding(
            "proposal_reviewer_security",
            "security",
            &[],
            &[],
            &["security"],
            true,
            "phase_3_core",
        )];

        let fingerprint = ProposalFingerprint {
            proposal_md5: "mno".into(),
            ..Default::default()
        };

        let options = ReviewRoutingOptions {
            mode: ReviewRoutingMode::Dynamic,
            force_include: vec!["proposal_reviewer_security".into()],
            force_exclude: vec!["proposal_reviewer_security".into()],
            ..Default::default()
        };

        let outcome = route_proposal_reviewers(
            RunId::new(),
            "proposal_reviewed",
            1,
            &bindings,
            &fingerprint,
            &options,
            &default_input_hashes(),
        );

        match outcome {
            RoutingOutcome::Failure { failure_kind, .. } => {
                assert_eq!(failure_kind, RoutingFailureKind::OverrideConflict);
            }
            RoutingOutcome::Success { .. } => panic!("Expected override conflict failure"),
        }
    }

    #[test]
    fn under_specified_falls_back_to_product_owner_architect() {
        let bindings = vec![
            make_binding(
                "proposal_reviewer_product_owner",
                "po",
                &[],
                &[],
                &[],
                true,
                "existing",
            ),
            make_binding(
                "proposal_reviewer_architect",
                "arch",
                &[],
                &[],
                &[],
                true,
                "existing",
            ),
        ];

        let fingerprint = ProposalFingerprint {
            proposal_md5: "pqr".into(),
            // No matching stacks/surfaces/risks.
            ..Default::default()
        };

        let options = ReviewRoutingOptions {
            mode: ReviewRoutingMode::Dynamic,
            ..Default::default()
        };

        let outcome = route_proposal_reviewers(
            RunId::new(),
            "proposal_reviewed",
            1,
            &bindings,
            &fingerprint,
            &options,
            &default_input_hashes(),
        );

        match outcome {
            RoutingOutcome::Success { plan, .. } => {
                let ids: Vec<&str> = plan
                    .selected_agents
                    .iter()
                    .map(|a| a.agent_id.as_str())
                    .collect();
                assert!(ids.contains(&"proposal_reviewer_product_owner"));
                assert!(ids.contains(&"proposal_reviewer_architect"));
                assert!(plan
                    .warnings
                    .contains(&"under_specified_selection".to_string()));
            }
            RoutingOutcome::Failure { .. } => panic!("Expected under_specified success"),
        }
    }

    #[test]
    fn mandatory_overflow_prunes_to_reviewer_cap() {
        // Create 6 mandatory bindings (> MAX_SELECTED_REVIEWERS).
        let mut bindings = Vec::new();
        for i in 0..6 {
            let mut b = make_binding(
                &format!("reviewer_{i}"),
                &format!("r{i}"),
                &["rust"],
                &[],
                &[],
                true,
                "phase_3_core",
            );
            b.routing_metadata.mandatory_when = vec!["rust".into()];
            bindings.push(b);
        }

        let fingerprint = ProposalFingerprint {
            proposal_md5: "overflow".into(),
            stacks: vec!["rust".into()],
            ..Default::default()
        };

        let options = ReviewRoutingOptions {
            mode: ReviewRoutingMode::Dynamic,
            ..Default::default()
        };

        let outcome = route_proposal_reviewers(
            RunId::new(),
            "proposal_reviewed",
            1,
            &bindings,
            &fingerprint,
            &options,
            &default_input_hashes(),
        );

        match outcome {
            RoutingOutcome::Success { plan, .. } => {
                assert_eq!(plan.selected_agents.len(), MAX_SELECTED_REVIEWERS);
                assert!(plan
                    .warnings
                    .contains(&"mandatory_overflow_pruned".to_string()));
                assert_eq!(
                    plan.rejected_alternatives
                        .iter()
                        .filter(|alt| alt.reason == "mandatory_overflow_pruned")
                        .count(),
                    1
                );
            }
            RoutingOutcome::Failure { .. } => panic!("Expected mandatory overflow pruning"),
        }
    }

    #[test]
    fn selected_outputs_resolves_only_selected_reviewers() {
        let plan = AgentSelectionPlanV1 {
            schema_version: "1".into(),
            routing_rules_version: "1".into(),
            proposal_md5: "abc".into(),
            plan_hash: "test_hash".into(),
            mode: ReviewRoutingMode::Dynamic,
            fingerprint: vec![],
            selected_agents: vec![
                SelectedAgent {
                    agent_id: "proposal_reviewer_security".into(),
                    routing_id: "security".into(),
                    score: 10,
                    mandatory: false,
                    override_source: None,
                    score_terms: ScoreTerms::default(),
                    rationale: "test".into(),
                    evidence_refs: vec![],
                    materialization_binding_id: "b1".into(),
                },
                SelectedAgent {
                    agent_id: "proposal_reviewer_rust_architect".into(),
                    routing_id: "rust_arch".into(),
                    score: 8,
                    mandatory: false,
                    override_source: None,
                    score_terms: ScoreTerms::default(),
                    rationale: "test".into(),
                    evidence_refs: vec![],
                    materialization_binding_id: "b2".into(),
                },
            ],
            rejected_alternatives: vec![],
            ineligible_candidates: vec![],
            warnings: vec![],
            input_snapshot_hashes: InputSnapshotHashes::default(),
        };

        let available = vec![
            AvailableArtifact {
                artifact_id: "a1".into(),
                agent_id: "proposal_reviewer_security".into(),
                contract_id: "proposal_review_v1".into(),
                file_path: "/path/to/security_review.json".into(),
            },
            AvailableArtifact {
                artifact_id: "a2".into(),
                agent_id: "proposal_reviewer_rust_architect".into(),
                contract_id: "proposal_review_v1".into(),
                file_path: "/path/to/rust_review.json".into(),
            },
            // Stale artifact from unselected reviewer
            AvailableArtifact {
                artifact_id: "a3".into(),
                agent_id: "proposal_reviewer_ui".into(),
                contract_id: "proposal_review_v1".into(),
                file_path: "/path/to/ui_review.json".into(),
            },
            // Different contract
            AvailableArtifact {
                artifact_id: "a4".into(),
                agent_id: "proposal_reviewer_security".into(),
                contract_id: "other_contract".into(),
                file_path: "/path/to/other.json".into(),
            },
        ];

        let result = resolve_selected_outputs(&plan, &available, "proposal_review_v1");
        assert_eq!(result.reviewer_count, 2);
        assert_eq!(result.selected_review_artifacts.len(), 2);
        assert_eq!(result.ignored_artifacts.len(), 1);
        assert_eq!(result.ignored_artifacts[0].agent_id, "proposal_reviewer_ui");
        assert!(!result.legacy_fixed_mode);
        assert_eq!(result.selection_plan_hash, "test_hash");
    }

    #[test]
    fn dynamic_mode_is_default() {
        let opts = ReviewRoutingOptions::default();
        assert_eq!(opts.mode, ReviewRoutingMode::Dynamic);
    }

    #[test]
    fn deterministic_plan_hash() {
        let bindings = vec![
            make_binding(
                "proposal_reviewer_security",
                "security",
                &[],
                &[],
                &["security"],
                true,
                "phase_3_core",
            ),
            make_binding(
                "proposal_reviewer_api_contract",
                "api",
                &[],
                &[],
                &["api"],
                true,
                "phase_3_core",
            ),
            make_binding(
                "proposal_reviewer_product_owner",
                "po",
                &[],
                &[],
                &[],
                true,
                "existing",
            ),
            make_binding(
                "proposal_reviewer_architect",
                "arch",
                &[],
                &[],
                &[],
                true,
                "existing",
            ),
        ];

        let fingerprint = ProposalFingerprint {
            proposal_md5: "det".into(),
            risks: vec!["security".into(), "api".into()],
            ..Default::default()
        };

        let options = ReviewRoutingOptions {
            mode: ReviewRoutingMode::Dynamic,
            ..Default::default()
        };

        let hashes: InputSnapshotHashes = default_input_hashes();

        // Run twice with the same inputs.
        let outcome1 = route_proposal_reviewers(
            RunId::new(),
            "s",
            1,
            &bindings,
            &fingerprint,
            &options,
            &hashes,
        );
        let outcome2 = route_proposal_reviewers(
            RunId::new(),
            "s",
            1,
            &bindings,
            &fingerprint,
            &options,
            &hashes,
        );

        let (plan1, plan2) = match (outcome1, outcome2) {
            (
                RoutingOutcome::Success { plan: p1, .. },
                RoutingOutcome::Success { plan: p2, .. },
            ) => (p1, p2),
            _ => panic!("Both should succeed"),
        };

        assert_eq!(
            plan1.plan_hash, plan2.plan_hash,
            "plan_hash must be deterministic"
        );
        assert_eq!(
            plan1
                .selected_agents
                .iter()
                .map(|a| &a.agent_id)
                .collect::<Vec<_>>(),
            plan2
                .selected_agents
                .iter()
                .map(|a| &a.agent_id)
                .collect::<Vec<_>>(),
            "selected order must be deterministic"
        );
    }

    /// P060 §17: selected_outputs_from aggregation works for 2, 3, 4, and 5 reviewers.
    #[test]
    fn selected_outputs_from_handles_2_to_5_reviewer_counts() {
        let reviewer_ids = [
            "proposal_reviewer_security",
            "proposal_reviewer_rust_architect",
            "proposal_reviewer_reliability",
            "proposal_reviewer_api_contract",
            "proposal_reviewer_observability_rollout",
        ];

        for count in 2..=5 {
            let selected_agents: Vec<SelectedAgent> = reviewer_ids[..count]
                .iter()
                .enumerate()
                .map(|(i, id)| SelectedAgent {
                    agent_id: id.to_string(),
                    routing_id: format!("r{}", i),
                    score: 10 - i as i64,
                    mandatory: false,
                    override_source: None,
                    score_terms: ScoreTerms::default(),
                    rationale: "test".into(),
                    evidence_refs: vec![],
                    materialization_binding_id: format!("b{}", i),
                })
                .collect();

            let plan = AgentSelectionPlanV1 {
                schema_version: "1".into(),
                routing_rules_version: "1".into(),
                proposal_md5: "test".into(),
                plan_hash: format!("hash_{}", count),
                mode: ReviewRoutingMode::Dynamic,
                fingerprint: vec![],
                selected_agents,
                rejected_alternatives: vec![],
                ineligible_candidates: vec![],
                warnings: vec![],
                input_snapshot_hashes: InputSnapshotHashes::default(),
            };

            // Create artifacts: one for each selected + one stale unselected.
            let mut available: Vec<AvailableArtifact> = reviewer_ids[..count]
                .iter()
                .enumerate()
                .map(|(i, id)| AvailableArtifact {
                    artifact_id: format!("art_{}", i),
                    agent_id: id.to_string(),
                    contract_id: "proposal_review_v1".into(),
                    file_path: format!("/path/{}.json", id),
                })
                .collect();
            // Stale artifact from an unselected reviewer.
            available.push(AvailableArtifact {
                artifact_id: "stale".into(),
                agent_id: "proposal_reviewer_ux".into(),
                contract_id: "proposal_review_v1".into(),
                file_path: "/path/stale_ux.json".into(),
            });

            let result = resolve_selected_outputs(&plan, &available, "proposal_review_v1");
            assert_eq!(
                result.reviewer_count, count,
                "reviewer_count for {count} selected"
            );
            assert_eq!(
                result.selected_review_artifacts.len(),
                count,
                "selected artifacts for {count} selected"
            );
            assert_eq!(
                result.ignored_artifacts.len(),
                1,
                "stale artifact ignored for {count} selected"
            );
            assert_eq!(result.selection_plan_hash, format!("hash_{}", count));
            assert!(!result.legacy_fixed_mode);
        }
    }
}
