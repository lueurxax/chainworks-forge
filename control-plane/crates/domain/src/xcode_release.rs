//! Bounded R4 release validation. No filesystem, host access or release authority.
//!
//! Evidence is a redacted structured assertion supplied by a trusted collector,
//! not an Apple payload or a cryptographic attestation. Hashes detect changed
//! bytes, not a dishonest collector. The caller independently pins candidate
//! identities, admitted workflow routes and the durable unresolved count.

use crate::xcode_contract::{
    canonical_digest, parse_unique_json, trust_policy_digest, MAX_SAFE_INTEGER, TRUST_POLICY_ID,
};
use chrono::{DateTime, FixedOffset};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

pub const MAX_RECEIPT_BYTES: usize = 256 * 1024;
pub const MAX_EVIDENCE_BYTES: usize = 256 * 1024;
pub const MAX_EVIDENCE_FILES: usize = 128;
pub const MAX_TOTAL_EVIDENCE_BYTES: usize = 4 * 1024 * 1024;
const MAX_CAPABILITIES: usize = 128;
const MAX_REFS: usize = 32;

/// Closed R4 proof vocabulary. A path-presence observation cannot provide the
/// initial ID/path proof; fixture observations cannot provide installed proof.
const CHECKS: &[(&str, EvidenceKind, &[&str])] = &[
    (
        "ide_absent",
        EvidenceKind::InstalledHost,
        &["ide_absent_before_and_after"],
    ),
    (
        "packaged_launch_identity",
        EvidenceKind::InstalledHost,
        &["packaged_signing_and_launch_chain"],
    ),
    (
        "consent_scope",
        EvidenceKind::InstalledHost,
        &[
            "exact_principal_and_folder",
            "global_permission_disabled",
            "no_automatic_grants",
        ],
    ),
    (
        "cold_start",
        EvidenceKind::InstalledHost,
        &["selected_service_acquired", "foreign_clients_untouched"],
    ),
    (
        "warm_attach",
        EvidenceKind::InstalledHost,
        &["no_ide_or_service_restart"],
    ),
    (
        "project_binding",
        EvidenceKind::InstalledHost,
        &[
            "initial_structured_id_and_exact_path",
            "same_service_generation",
            "fresh_exact_path_presence",
            "explicit_id_read_via_broker_lease",
        ],
    ),
    (
        "foreign_project_rejection",
        EvidenceKind::Fixture,
        &["foreign_selectors_handles_paths_denied_via_lease"],
    ),
    (
        "trusted_project_admission",
        EvidenceKind::Fixture,
        &[
            "operator_trust_pins_root_project_uid_installation_generation_mapping",
            "evaluated_permissions_and_tool_allowlist",
            "absent_revoked_mismatched_authority_denied",
        ],
    ),
    (
        "scope_controls",
        EvidenceKind::Fixture,
        &[
            "closed_tested_adapters",
            "explicit_bound_ids_and_path_handle_rules",
            "fresh_drift_checks",
            "unknown_default_prose_and_close_reopen_denied",
            "blind_interval_and_external_references_acknowledged",
        ],
    ),
    (
        "reuse_revalidation",
        EvidenceKind::Fixture,
        &["stale_project_service_policy_denied_before_prompt"],
    ),
    (
        "broker_shim_exclusion",
        EvidenceKind::Fixture,
        &["shared_coordinator", "concurrent_effect_dispatch_excluded"],
    ),
    (
        "restart_recovery",
        EvidenceKind::Fixture,
        &[
            "stale_leases_invalidated",
            "uncertainty_restored",
            "zero_automatic_replay",
        ],
    ),
    (
        "effect_dedupe",
        EvidenceKind::Fixture,
        &[
            "repeat_nonce_and_new_key_zero_second_dispatch",
            "fault_boundaries_and_journal_transitions",
        ],
    ),
    (
        "public_redaction",
        EvidenceKind::Fixture,
        &["health_and_errors_no_private_details"],
    ),
    (
        "mixed_versions",
        EvidenceKind::Fixture,
        &["old_new_clients_and_future_values"],
    ),
    (
        "canonical_gates",
        EvidenceKind::RepositoryGate,
        &["required_gates_passed", "remote_only_ui_policy"],
    ),
    (
        "required_capabilities",
        EvidenceKind::Fixture,
        &["frozen_workflow_inventory_and_admitted_routes"],
    ),
];

const IDENTITY_KEYS: &[&str] = &[
    "release_id",
    "source_commit",
    "source_content_digest",
    "app",
    "daemon",
    "xcode",
    "launch_identity",
    "service_generation",
    "contract",
    "consent_identity",
];

// Each identity object is closed. All fields are required strings under R4.
const IDENTITY_FIELDS: &[(&str, &[&str])] = &[
    (
        "app",
        &[
            "version",
            "build",
            "bundle_id",
            "signing_team",
            "designated_requirement_digest",
            "artifact_digest",
        ],
    ),
    (
        "daemon",
        &[
            "version",
            "build_sha",
            "artifact_digest",
            "signing_requirement_digest",
            "db_schema_version",
        ],
    ),
    (
        "xcode",
        &[
            "version",
            "build",
            "toolchain_identity_digest",
            "service_bundle_id",
            "service_build",
        ],
    ),
    (
        "launch_identity",
        &[
            "mode",
            "uid_pseudonym",
            "daemon_executable_digest",
            "bridge_executable_digest",
            "daemon_signing_requirement_digest",
            "bridge_signing_requirement_digest",
        ],
    ),
    (
        "service_generation",
        &[
            "boot_identity",
            "start_identity",
            "pid",
            "bundle_identity_digest",
            "executable_identity_digest",
        ],
    ),
    (
        "contract",
        &[
            "manifest_digest",
            "schema_bundle_digest",
            "effective_policy_digest",
            "binding_digest",
            "trust_policy_id",
            "trust_policy_digest",
        ],
    ),
];

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReleaseVerdict {
    Pass,
    Hold,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum HoldReason {
    IdentityMismatch,
    MissingEvidence,
    EvidenceMismatch,
    IncompleteChecks,
    CapabilityCoverage,
    Consent,
    DevelopmentLaunch,
    UnresolvedAttempts,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReleaseValidation {
    pub verdict: ReleaseVerdict,
    pub holds: BTreeSet<HoldReason>,
}

/// Errors deliberately contain no caller-controlled text or paths.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum ReleaseError {
    #[error("Release validation input exceeds its bound.")]
    InputTooLarge,
    #[error("Release receipt does not match the R4 schema.")]
    InvalidReceipt,
    #[error("Candidate release context is invalid.")]
    InvalidCandidate,
    #[error("Release evidence does not match the R4 schema.")]
    InvalidEvidence,
    #[error("Release input contains forbidden unredacted content.")]
    RedactionViolation,
    #[error("Release receipt digest does not match.")]
    ReceiptDigestMismatch,
    #[error("Declared release verdict does not match validation.")]
    VerdictMismatch,
}

/// Independently assembled candidate identity and frozen workflow requirements.
/// Callers must derive artifact digests and journal counts from trusted readback,
/// never by copying the receipt being validated.
#[derive(Clone, Debug)]
pub struct ReleaseCandidate {
    pub identity: Value,
    pub required_capabilities: Value,
    pub unresolved_attempt_count: u64,
}

impl ReleaseCandidate {
    pub fn source_identity_digest(&self) -> Result<String, ReleaseError> {
        validate_candidate(self)?;
        canonical_digest(
            "cw.xcode.release-source.v1",
            &serde_json::json!({"identity": self.identity,
                "required_capabilities": self.required_capabilities}),
        )
        .map_err(|_| ReleaseError::InvalidCandidate)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckStatus {
    Pass,
    Fail,
    Unknown,
    NotRun,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityRoute {
    HeadlessMcp,
    CanonicalGate,
    ExistingFilesystem,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceKind {
    Fixture,
    InstalledHost,
    RepositoryGate,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReleaseCheck {
    pub status: CheckStatus,
    pub observed_at: String,
    pub evidence_refs: Vec<String>,
    pub details: String,
    pub source_identity_digest: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReleaseCapability {
    pub capability: String,
    pub required_by: String,
    pub route: CapabilityRoute,
    pub status: CheckStatus,
    pub evidence_refs: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RequiredCapability {
    pub capability: String,
    pub required_by: String,
    pub admitted_routes: Vec<CapabilityRoute>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceCheck {
    pub status: CheckStatus,
    pub assertions: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceCapability {
    pub capability: String,
    pub required_by: String,
    pub route: CapabilityRoute,
    pub status: CheckStatus,
}

/// Redacted collector envelope. Its kind is provenance supplied by the caller,
/// not something this filesystem-free validator can independently establish.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReleaseEvidence {
    pub schema_version: u32,
    pub contract_revision: u32,
    pub source_identity_digest: String,
    pub observed_at: String,
    pub kind: EvidenceKind,
    pub checks: BTreeMap<String, EvidenceCheck>,
    pub capabilities: Vec<EvidenceCapability>,
}

/// Validates only bounded bytes and independently supplied expected identities.
/// No receipt or evidence files are created, repaired, relabelled or published.
pub fn validate_release_receipt(
    receipt: &[u8],
    candidate: &ReleaseCandidate,
    evidence: &BTreeMap<String, Vec<u8>>,
) -> Result<ReleaseValidation, ReleaseError> {
    if receipt.len() > MAX_RECEIPT_BYTES || evidence.len() > MAX_EVIDENCE_FILES {
        return Err(ReleaseError::InputTooLarge);
    }
    let mut total = 0usize;
    for bytes in evidence.values() {
        total = total
            .checked_add(bytes.len())
            .ok_or(ReleaseError::InputTooLarge)?;
        if bytes.len() > MAX_EVIDENCE_BYTES || total > MAX_TOTAL_EVIDENCE_BYTES {
            return Err(ReleaseError::InputTooLarge);
        }
    }
    let error = ReleaseError::InvalidReceipt;
    let required = validate_candidate(candidate)?;
    let source = candidate.source_identity_digest()?;
    let value = parse_unique_json(receipt).map_err(|_| error)?;
    let root = object(
        &value,
        &[
            "schema_version",
            "release_id",
            "source_commit",
            "source_content_digest",
            "generated_at",
            "app",
            "daemon",
            "xcode",
            "launch_identity",
            "service_generation",
            "contract",
            "consent",
            "capabilities",
            "checks",
            "unresolved_attempt_count",
            "evidence_manifest",
            "verdict",
            "receipt_digest",
        ],
        error,
    )?;
    if root["schema_version"].as_u64() != Some(1) {
        return Err(error);
    }
    let receipt_hash = digest_value(&root["receipt_digest"], error)?;
    let mut unhashed = root.clone();
    unhashed.remove("receipt_digest");
    if canonical_digest("cw.xcode.receipt.v1", &Value::Object(unhashed)).map_err(|_| error)?
        != receipt_hash
    {
        return Err(ReleaseError::ReceiptDigestMismatch);
    }
    let generated_at = timestamp(string(&root["generated_at"], error)?, error)?;
    validate_identity_fields(root, error)?;
    let consent = object(
        &root["consent"],
        &[
            "status",
            "principal_digest",
            "folder_scope_digest",
            "evidence_ref",
        ],
        error,
    )?;
    digest_value(&consent["principal_digest"], error)?;
    digest_value(&consent["folder_scope_digest"], error)?;
    let consent_status = string(&consent["status"], error)?;
    if !["approved", "denied", "unknown"].contains(&consent_status) {
        return Err(error);
    }
    let consent_ref = string(&consent["evidence_ref"], error)?;
    evidence_path(consent_ref, error)?;
    let checks: BTreeMap<String, ReleaseCheck> = decode(&root["checks"], error)?;
    if checks.len() != CHECKS.len()
        || CHECKS
            .iter()
            .any(|(name, _, _)| !checks.contains_key(*name))
    {
        return Err(error);
    }
    for check in checks.values() {
        timestamp(&check.observed_at, error)?;
        digest_text(&check.source_identity_digest, error)?;
        redacted_details(&check.details)?;
        validate_refs(&check.evidence_refs, error)?;
    }
    let capabilities: Vec<ReleaseCapability> = decode(&root["capabilities"], error)?;
    validate_capabilities(
        capabilities.iter().map(|c| (&c.capability, &c.required_by)),
        error,
    )?;
    for row in &capabilities {
        validate_refs(&row.evidence_refs, error)?;
    }
    let unresolved = root["unresolved_attempt_count"]
        .as_u64()
        .filter(|n| *n <= MAX_SAFE_INTEGER)
        .ok_or(error)?;
    let declared: ReleaseVerdict = decode(&root["verdict"], error)?;
    let manifest = root["evidence_manifest"]
        .as_object()
        .filter(|m| m.len() <= MAX_EVIDENCE_FILES)
        .ok_or(error)?;
    for (path, hash) in manifest {
        evidence_path(path, error)?;
        digest_value(hash, error)?;
    }

    let mut holds = BTreeSet::new();
    for key in IDENTITY_KEYS
        .iter()
        .filter(|key| **key != "consent_identity")
    {
        if root[*key] != candidate.identity[*key] {
            holds.insert(HoldReason::IdentityMismatch);
        }
    }
    for key in ["principal_digest", "folder_scope_digest"] {
        if consent[key] != candidate.identity["consent_identity"][key] {
            holds.insert(HoldReason::IdentityMismatch);
        }
    }
    if consent_status != "approved" {
        holds.insert(HoldReason::Consent);
    }
    if root["launch_identity"]["mode"] != "packaged" {
        holds.insert(HoldReason::DevelopmentLaunch);
    }
    if unresolved != 0
        || candidate.unresolved_attempt_count != 0
        || unresolved != candidate.unresolved_attempt_count
    {
        holds.insert(HoldReason::UnresolvedAttempts);
    }

    // Validate every supplied envelope, including extras, before retaining only
    // digest-matching manifest entries. Extra bytes cannot silently provide proof.
    let mut admitted = BTreeMap::new();
    for (path, bytes) in evidence {
        evidence_path(path, ReleaseError::InvalidEvidence)?;
        let observation = validate_evidence(bytes)?;
        if !manifest.contains_key(path) || manifest[path] != format!("{:x}", Sha256::digest(bytes))
        {
            holds.insert(HoldReason::EvidenceMismatch);
            continue;
        }
        if observation.source_identity_digest != source
            || timestamp(&observation.observed_at, ReleaseError::InvalidEvidence)? > generated_at
        {
            holds.insert(HoldReason::EvidenceMismatch);
            continue;
        }
        admitted.insert(path.as_str(), observation);
    }
    if manifest
        .keys()
        .any(|path| !admitted.contains_key(path.as_str()))
    {
        holds.insert(HoldReason::MissingEvidence);
    }
    let mut referenced = BTreeSet::from([consent_ref]);
    for (name, kind, assertions) in CHECKS {
        let check = &checks[*name];
        referenced.extend(check.evidence_refs.iter().map(String::as_str));
        let check_time = timestamp(&check.observed_at, error)?;
        let mut covered = BTreeSet::new();
        let mut matched = !check.evidence_refs.is_empty();
        for path in &check.evidence_refs {
            match admitted.get(path.as_str()) {
                Some(observation)
                    if observation.kind == *kind
                        && timestamp(&observation.observed_at, error)? <= check_time =>
                {
                    if let Some(proof) = observation
                        .checks
                        .get(*name)
                        .filter(|p| p.status == CheckStatus::Pass)
                    {
                        covered.extend(proof.assertions.iter().map(String::as_str));
                    } else {
                        matched = false;
                    }
                }
                _ => matched = false,
            }
        }
        if check.status != CheckStatus::Pass
            || check.source_identity_digest != source
            || check_time > generated_at
            || !matched
            || assertions.iter().any(|a| !covered.contains(a))
        {
            holds.insert(HoldReason::IncompleteChecks);
        }
        if check.source_identity_digest != source {
            holds.insert(HoldReason::IdentityMismatch);
        }
    }
    if !checks["consent_scope"]
        .evidence_refs
        .iter()
        .any(|r| r == consent_ref)
        || !admitted.get(consent_ref).is_some_and(|e| {
            e.kind == EvidenceKind::InstalledHost
                && e.checks
                    .get("consent_scope")
                    .is_some_and(|c| c.status == CheckStatus::Pass)
        })
    {
        holds.insert(HoldReason::Consent);
    }
    if capabilities.len() != required.len() {
        holds.insert(HoldReason::CapabilityCoverage);
    }
    for row in &capabilities {
        referenced.extend(row.evidence_refs.iter().map(String::as_str));
        let requirement = required
            .iter()
            .find(|r| r.capability == row.capability && r.required_by == row.required_by);
        let route_kind = match row.route {
            CapabilityRoute::HeadlessMcp | CapabilityRoute::ExistingFilesystem => {
                EvidenceKind::InstalledHost
            }
            CapabilityRoute::CanonicalGate => EvidenceKind::RepositoryGate,
        };
        let covered = !row.evidence_refs.is_empty()
            && row.evidence_refs.iter().all(|path| {
                admitted.get(path.as_str()).is_some_and(|e| {
                    e.kind == route_kind
                        && e.capabilities.iter().any(|c| {
                            c.capability == row.capability
                                && c.required_by == row.required_by
                                && c.route == row.route
                                && c.status == CheckStatus::Pass
                        })
                })
            });
        if row.status != CheckStatus::Pass
            || !requirement.is_some_and(|r| r.admitted_routes.contains(&row.route))
            || !covered
        {
            holds.insert(HoldReason::CapabilityCoverage);
        }
    }
    if referenced.iter().any(|path| !admitted.contains_key(path))
        || manifest
            .keys()
            .any(|path| !referenced.contains(path.as_str()))
    {
        holds.insert(HoldReason::MissingEvidence);
    }
    let verdict = if holds.is_empty() {
        ReleaseVerdict::Pass
    } else {
        ReleaseVerdict::Hold
    };
    if declared != verdict {
        return Err(ReleaseError::VerdictMismatch);
    }
    Ok(ReleaseValidation { verdict, holds })
}

fn validate_candidate(
    candidate: &ReleaseCandidate,
) -> Result<Vec<RequiredCapability>, ReleaseError> {
    let error = ReleaseError::InvalidCandidate;
    bound_value(&candidate.identity, 0)?;
    bound_value(&candidate.required_capabilities, 0)?;
    let identity = object(&candidate.identity, IDENTITY_KEYS, error)?;
    validate_identity_fields(identity, error)?;
    let consent = object(
        &identity["consent_identity"],
        &["principal_digest", "folder_scope_digest"],
        error,
    )?;
    for value in consent.values() {
        digest_value(value, error)?;
    }
    let required: Vec<RequiredCapability> = decode(&candidate.required_capabilities, error)?;
    validate_capabilities(
        required.iter().map(|r| (&r.capability, &r.required_by)),
        error,
    )?;
    for row in &required {
        if row.admitted_routes.len() > 3
            || row.admitted_routes.iter().collect::<BTreeSet<_>>().len()
                != row.admitted_routes.len()
        {
            return Err(error);
        }
    }
    if candidate.unresolved_attempt_count > MAX_SAFE_INTEGER {
        return Err(error);
    }
    Ok(required)
}

fn validate_identity_fields(
    root: &Map<String, Value>,
    error: ReleaseError,
) -> Result<(), ReleaseError> {
    label(string(&root["release_id"], error)?, error)?;
    hex(string(&root["source_commit"], error)?, &[40, 64], error)?;
    digest_value(&root["source_content_digest"], error)?;
    for (name, keys) in IDENTITY_FIELDS {
        let fields = object(&root[*name], keys, error)?;
        for (key, value) in fields {
            let text = string(value, error)?;
            if key.ends_with("_digest") {
                digest_text(text, error)?;
            } else if key == "build_sha" {
                hex(text, &[40, 64], error)?;
            } else if key == "pid" || key == "db_schema_version" {
                if text.is_empty()
                    || text.len() > 10
                    || text.starts_with('0')
                    || !text.bytes().all(|c| c.is_ascii_digit())
                    || text.parse::<u32>().is_err()
                {
                    return Err(error);
                }
            } else {
                label(text, error)?;
            }
        }
    }
    if !["packaged", "development"].contains(&string(&root["launch_identity"]["mode"], error)?)
        || root["contract"]["trust_policy_id"] != TRUST_POLICY_ID
        || root["contract"]["trust_policy_digest"] != trust_policy_digest().map_err(|_| error)?
    {
        return Err(error);
    }
    Ok(())
}

fn validate_evidence(bytes: &[u8]) -> Result<ReleaseEvidence, ReleaseError> {
    let error = ReleaseError::InvalidEvidence;
    let value = parse_unique_json(bytes).map_err(|_| error)?;
    let observation: ReleaseEvidence = decode(&value, error)?;
    if observation.schema_version != 1
        || observation.contract_revision != 4
        || observation.checks.len() > CHECKS.len()
    {
        return Err(error);
    }
    digest_text(&observation.source_identity_digest, error)?;
    timestamp(&observation.observed_at, error)?;
    for (name, check) in &observation.checks {
        let (_, _, allowed) = CHECKS.iter().find(|(n, _, _)| n == name).ok_or(error)?;
        if check.assertions.len() > allowed.len()
            || check.assertions.iter().collect::<BTreeSet<_>>().len() != check.assertions.len()
            || check
                .assertions
                .iter()
                .any(|a| !allowed.contains(&a.as_str()))
        {
            return Err(error);
        }
    }
    validate_capabilities(
        observation
            .capabilities
            .iter()
            .map(|c| (&c.capability, &c.required_by)),
        error,
    )?;
    Ok(observation)
}

fn validate_capabilities<'a>(
    rows: impl Iterator<Item = (&'a String, &'a String)>,
    error: ReleaseError,
) -> Result<(), ReleaseError> {
    let mut seen = BTreeSet::new();
    for (capability, required_by) in rows {
        label(capability, error)?;
        label(required_by, error)?;
        if !seen.insert((capability, required_by)) || seen.len() > MAX_CAPABILITIES {
            return Err(error);
        }
    }
    Ok(())
}

fn object<'a>(
    value: &'a Value,
    keys: &[&str],
    error: ReleaseError,
) -> Result<&'a Map<String, Value>, ReleaseError> {
    let object = value.as_object().ok_or(error)?;
    if object.len() != keys.len() || keys.iter().any(|key| !object.contains_key(*key)) {
        return Err(error);
    }
    Ok(object)
}

fn decode<T: serde::de::DeserializeOwned>(
    value: &Value,
    error: ReleaseError,
) -> Result<T, ReleaseError> {
    serde_json::from_value(value.clone()).map_err(|_| error)
}

fn string(value: &Value, error: ReleaseError) -> Result<&str, ReleaseError> {
    value.as_str().ok_or(error)
}

fn hex(value: &str, lengths: &[usize], error: ReleaseError) -> Result<(), ReleaseError> {
    if !lengths.contains(&value.len())
        || !value
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
    {
        return Err(error);
    }
    Ok(())
}

fn digest_text(value: &str, error: ReleaseError) -> Result<(), ReleaseError> {
    hex(value, &[64], error)
}

fn digest_value(value: &Value, error: ReleaseError) -> Result<&str, ReleaseError> {
    let text = string(value, error)?;
    digest_text(text, error)?;
    Ok(text)
}

fn timestamp(value: &str, error: ReleaseError) -> Result<DateTime<FixedOffset>, ReleaseError> {
    if value.len() > 35
        || (!value.ends_with('Z') && !value.ends_with("+00:00"))
        || !value.contains('T')
    {
        return Err(error);
    }
    let time = DateTime::parse_from_rfc3339(value).map_err(|_| error)?;
    if time.offset().local_minus_utc() != 0 {
        return Err(error);
    }
    Ok(time)
}

fn label(value: &str, error: ReleaseError) -> Result<(), ReleaseError> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"._-".contains(&c))
        || !value.as_bytes()[0].is_ascii_alphanumeric()
        || ["latest", "unknown", "none", "null", "na", "n-a", "pending"]
            .contains(&value.to_ascii_lowercase().as_str())
    {
        return Err(error);
    }
    reject_secret(value)
}

fn evidence_path(value: &str, error: ReleaseError) -> Result<(), ReleaseError> {
    if value.is_empty() || value.len() > 256 {
        return Err(error);
    }
    for part in value.split('/') {
        label(part, error)?;
    }
    Ok(())
}

fn validate_refs(refs: &[String], error: ReleaseError) -> Result<(), ReleaseError> {
    if refs.len() > MAX_REFS || refs.iter().collect::<BTreeSet<_>>().len() != refs.len() {
        return Err(error);
    }
    refs.iter().try_for_each(|path| evidence_path(path, error))
}

fn reject_secret(value: &str) -> Result<(), ReleaseError> {
    let lower = value.to_ascii_lowercase();
    if [
        "bearer",
        "token",
        "password",
        "secret",
        "private key",
        "private_key",
        "-----begin",
        "sk-",
        "ghp_",
        "github_pat_",
        "eyj",
    ]
    .iter()
    .any(|fragment| lower.contains(fragment))
    {
        return Err(ReleaseError::RedactionViolation);
    }
    Ok(())
}

fn redacted_details(value: &str) -> Result<(), ReleaseError> {
    if value.is_empty()
        || value.len() > 512
        || !value
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b" .,_-()'".contains(&c))
        || value.split_whitespace().any(|word| word.len() > 64)
    {
        return Err(ReleaseError::RedactionViolation);
    }
    reject_secret(value)
}

fn bound_value(value: &Value, depth: usize) -> Result<usize, ReleaseError> {
    if depth > 16 {
        return Err(ReleaseError::InputTooLarge);
    }
    let mut size = 1usize;
    match value {
        Value::String(s) => size = s.len(),
        Value::Array(values) => {
            if values.len() > MAX_CAPABILITIES {
                return Err(ReleaseError::InputTooLarge);
            }
            for value in values {
                size = size.saturating_add(bound_value(value, depth + 1)?);
            }
        }
        Value::Object(values) => {
            if values.len() > MAX_CAPABILITIES {
                return Err(ReleaseError::InputTooLarge);
            }
            for (key, value) in values {
                size = size
                    .saturating_add(key.len())
                    .saturating_add(bound_value(value, depth + 1)?);
            }
        }
        _ => {}
    }
    if size > MAX_RECEIPT_BYTES {
        return Err(ReleaseError::InputTooLarge);
    }
    Ok(size)
}
