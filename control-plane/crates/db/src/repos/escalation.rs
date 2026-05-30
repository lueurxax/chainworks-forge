use anyhow::{anyhow, bail, Result};
use chrono::{DateTime, Utc};
use sqlx::{Row, Sqlite, SqlitePool, Transaction};

use domain::escalation::{EscalationEvent, EscalationExecutionMetadata, EscalationLedger};
use domain::ids::RunId;

/// SEC-004: Detect duplicate object keys at any depth using a custom serde Visitor.
/// serde_json's `Value` parser silently keeps the last occurrence; this pass runs
/// first on the raw string so duplicate-key payloads are rejected before canonicalization.
fn reject_duplicate_json_keys(json_str: &str) -> Result<()> {
    use serde::de::{Deserialize, Deserializer, MapAccess, SeqAccess, Visitor};
    use std::collections::HashSet;
    use std::fmt;

    struct DupKeyChecker;

    impl<'de> Deserialize<'de> for DupKeyChecker {
        fn deserialize<D: Deserializer<'de>>(de: D) -> std::result::Result<Self, D::Error> {
            de.deserialize_any(DupKeyVisitor)
        }
    }

    struct DupKeyVisitor;

    impl<'de> Visitor<'de> for DupKeyVisitor {
        type Value = DupKeyChecker;
        fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
            write!(f, "any JSON value")
        }
        fn visit_map<A: MapAccess<'de>>(
            self,
            mut map: A,
        ) -> std::result::Result<Self::Value, A::Error> {
            let mut seen: HashSet<String> = HashSet::new();
            while let Some(key) = map.next_key::<String>()? {
                if !seen.insert(key.clone()) {
                    return Err(serde::de::Error::custom(format!(
                        "duplicate JSON object key: {key}"
                    )));
                }
                map.next_value::<DupKeyChecker>()?;
            }
            Ok(DupKeyChecker)
        }
        fn visit_seq<A: SeqAccess<'de>>(
            self,
            mut seq: A,
        ) -> std::result::Result<Self::Value, A::Error> {
            while seq.next_element::<DupKeyChecker>()?.is_some() {}
            Ok(DupKeyChecker)
        }
        fn visit_bool<E: serde::de::Error>(self, _: bool) -> std::result::Result<Self::Value, E> {
            Ok(DupKeyChecker)
        }
        fn visit_i64<E: serde::de::Error>(self, _: i64) -> std::result::Result<Self::Value, E> {
            Ok(DupKeyChecker)
        }
        fn visit_u64<E: serde::de::Error>(self, _: u64) -> std::result::Result<Self::Value, E> {
            Ok(DupKeyChecker)
        }
        fn visit_f64<E: serde::de::Error>(self, _: f64) -> std::result::Result<Self::Value, E> {
            Ok(DupKeyChecker)
        }
        fn visit_str<E: serde::de::Error>(self, _: &str) -> std::result::Result<Self::Value, E> {
            Ok(DupKeyChecker)
        }
        fn visit_none<E: serde::de::Error>(self) -> std::result::Result<Self::Value, E> {
            Ok(DupKeyChecker)
        }
        fn visit_unit<E: serde::de::Error>(self) -> std::result::Result<Self::Value, E> {
            Ok(DupKeyChecker)
        }
    }

    let mut de = serde_json::Deserializer::from_str(json_str);
    DupKeyChecker::deserialize(&mut de).map_err(|e| anyhow!("payload_json rejected: {e}"))?;
    Ok(())
}

/// Validate that `value` is well-formed JSON if present.
/// The proposal requires repository-layer JSON rejection even without sqlite json1.
fn validate_json_field(field_name: &str, value: &Option<String>) -> Result<()> {
    if let Some(json_str) = value {
        serde_json::from_str::<serde_json::Value>(json_str)
            .map_err(|e| anyhow!("field {field_name} contains malformed JSON: {e}"))?;
    }
    Ok(())
}

/// Approved non-dereferenceable prefixes for `redacted_evidence_ref` and
/// `redacted_message_fragment_hash` payload fields (P058-SEC-02).
/// Rejects URL schemes (`https://`), absolute paths (`/...`), and bare credentials (`sk-...`)
/// by requiring a known hash algorithm or `ref/` artifact reference prefix.
const APPROVED_REF_PREFIXES: &[&str] = &[
    "sha256:",
    "sha3-256:",
    "sha3-384:",
    "sha3-512:",
    "hmac-sha256:",
    "blake2:",
    "blake3:",
    "ref/",
];

/// Approved prefixes for hash-only fields like `redacted_message_fragment_hash` (no `ref/`).
const APPROVED_HASH_PREFIXES: &[&str] = &[
    "sha256:",
    "sha3-256:",
    "sha3-384:",
    "sha3-512:",
    "hmac-sha256:",
    "blake2:",
    "blake3:",
];

/// Minimum hex-digest lengths by algorithm prefix. SEC-MED-001: prevents short hex-encoded
/// sensitive material from passing as a real digest reference. Each value is the exact number
/// of lowercase hex characters produced by the algorithm (e.g. SHA-256 → 32 bytes → 64 hex).
/// blake2 has variable output (min 32 bytes = 64 hex used here); blake3 outputs 32 bytes.
const HASH_MIN_HEX_LENGTHS: &[(&str, usize)] = &[
    ("sha256:", 64),
    ("sha3-256:", 64),
    ("sha3-384:", 96),
    ("sha3-512:", 128),
    ("hmac-sha256:", 64),
    ("blake2:", 64),
    ("blake3:", 64),
];

/// Returns true when `s` starts with an approved hash/ref prefix AND the suffix after
/// the prefix passes strict shape validation. This prevents bypass via credential-shaped
/// strings (`sha256:sk-...`), nested URL schemes (`sha256:https://...`), or absolute paths
/// (`sha256:/Users/...`) that would pass a naive prefix-only check. P058-SEC-02.
///
/// Suffix rules by prefix kind:
/// - Hash prefixes (ending in `:`): suffix must be pure hexadecimal with at least the
///   algorithm-appropriate minimum length (SEC-MED-001). This rejects short hex-encoded
///   secrets and any non-digest payload.
/// - `ref/` prefix: suffix must be non-empty, must not start with `/` (no absolute paths
///   like `ref//Users/...`), must not contain `://`, and must only contain alphanumeric
///   plus `-`, `_`, `.`, `/`.
fn is_safe_ref_value(s: &str, prefixes: &[&str]) -> bool {
    if s.is_empty() || s.contains("..") || s.contains('\\') {
        return false;
    }
    let Some(&matched_prefix) = prefixes.iter().find(|&&p| s.starts_with(p)) else {
        return false;
    };
    let suffix = &s[matched_prefix.len()..];
    if suffix.is_empty() {
        return false;
    }
    if matched_prefix.ends_with(':') {
        // Hash prefix — suffix must be pure hex and meet the algorithm minimum length.
        if !suffix.chars().all(|c| c.is_ascii_hexdigit()) {
            return false;
        }
        let min_len = HASH_MIN_HEX_LENGTHS
            .iter()
            .find(|(p, _)| *p == matched_prefix)
            .map(|(_, len)| *len)
            .unwrap_or(32);
        suffix.len() >= min_len
    } else {
        // ref/ prefix — suffix is a relative artifact path; no leading '/', no '://'.
        if suffix.starts_with('/') || suffix.contains("://") || suffix.contains(':') {
            return false;
        }
        suffix
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '/'))
    }
}

/// Returns true when `s` is a safe identifier with no whitespace or control characters.
/// Used to validate enum/identifier payload fields that should not carry raw prose or credentials.
/// P058-SEC-02.
fn is_safe_identifier_value(s: &str) -> bool {
    !s.is_empty() && !s.chars().any(|c| c.is_whitespace() || c.is_control())
}

/// Per-value byte caps for string fields inside payload_json (P058-SEC-01).
/// Tight caps prevent oversized raw evidence, prompts, or credentials from being persisted
/// under permitted key names and surfaced verbatim through GraphQL/MCP readback.
const PAYLOAD_STRING_VALUE_MAX: usize = 512;
const PAYLOAD_EVIDENCE_REF_MAX: usize = 256;
const PAYLOAD_FRAGMENT_HASH_MAX: usize = 128;

/// P058-SEC-L1: Validate payload_json against a strict allowlist schema.
/// Top level must be a JSON object. Only the permitted top-level keys below are accepted.
/// Unknown keys — including uppercase variants (e.g. "Message"), alternate spellings
/// (e.g. "msg", "secret", "api_key"), and any other key — are rejected.
/// Writers must bump redaction_version when this validator changes.
///
/// Permitted top-level keys and their value constraints:
///   digest_inputs          → object; permitted sub-keys only (see DIGEST_INPUT_KEYS)
///   redacted_evidence_ref  → string
///   tier_id                → string
///   tier_kind_raw          → string
///   trigger_raw            → string
///   pause_reason_raw       → string or null
///   event_kind_raw         → string
///   policy_id              → string
///   chain_attempt_index    → number
///   digest_version         → string
///   waiting_retry_after_until → string
///   external_acknowledgement_ref → string
/// Returns canonical JSON (re-serialized from parsed Value) so duplicate keys are collapsed
/// before the caller stores the string. HIGH-001: callers MUST bind the returned canonical
/// string, not the original raw input.
fn canonicalize_and_validate_payload_json(json_str: &str) -> Result<String> {
    /// Permitted top-level object keys for escalation event payload_json.
    const TOP_LEVEL_KEYS: &[&str] = &[
        "digest_inputs",
        "redacted_evidence_ref",
        "tier_id",
        "tier_kind_raw",
        "trigger_raw",
        "pause_reason_raw",
        "event_kind_raw",
        "policy_id",
        "chain_attempt_index",
        "digest_version",
        "waiting_retry_after_until",
        "external_acknowledgement_ref",
        "metric_sample_ms",
        "metric_numerator",
        "metric_denominator",
    ];
    /// Permitted keys inside the digest_inputs object (proposal blocker_digest section).
    const DIGEST_INPUT_KEYS: &[&str] = &[
        "failure_kind",
        "output_settlement_state",
        "validation_evidence_kind",
        "redacted_message_fragment_hash",
    ];

    // SEC-004: Reject duplicate JSON object keys before parsing.
    reject_duplicate_json_keys(json_str)?;

    let v: serde_json::Value = serde_json::from_str(json_str)
        .map_err(|e| anyhow!("payload_json is not valid JSON: {e}"))?;

    let obj = match &v {
        serde_json::Value::Object(o) => o,
        other => bail!(
            "payload_json top level must be a JSON object, got {} (P058-SEC-L1)",
            match other {
                serde_json::Value::Array(_) => "array",
                serde_json::Value::String(_) => "string",
                serde_json::Value::Number(_) => "number",
                serde_json::Value::Bool(_) => "bool",
                serde_json::Value::Null => "null",
                serde_json::Value::Object(_) => unreachable!(),
            }
        ),
    };

    for key in obj.keys() {
        if !TOP_LEVEL_KEYS.contains(&key.as_str()) {
            bail!(
                "payload_json contains unknown top-level key '{}'; only digest_inputs, \
                 redacted_evidence_ref, and tier-metadata keys are permitted (P058-SEC-L1)",
                key
            );
        }
    }

    // Value-type constraints for each permitted top-level key (P058-SEC-L1 + P058-SEC-01).
    // Prevents raw evidence, nested objects, or credential blobs from reaching readback
    // under allowed key names. Per-value byte caps and identifier-format checks for
    // hash/ref fields block raw transcripts even when the JSON type is correct.
    for (key, val) in obj.iter() {
        match key.as_str() {
            "digest_inputs" => {} // validated below as object with sub-key allowlist
            "pause_reason_raw" => {
                if !val.is_string() && !val.is_null() {
                    bail!(
                        "payload_json key 'pause_reason_raw' must be a string or null, \
                         got {kind} (P058-SEC-L1)",
                        kind = json_type_name(val)
                    );
                }
                if let Some(s) = val.as_str() {
                    if s.len() > PAYLOAD_STRING_VALUE_MAX {
                        bail!(
                            "payload_json key 'pause_reason_raw' value exceeds maximum \
                             {PAYLOAD_STRING_VALUE_MAX} bytes (P058-SEC-02)"
                        );
                    }
                    if !is_safe_identifier_value(s) {
                        bail!(
                            "payload_json key 'pause_reason_raw' must be a bounded identifier \
                             with no whitespace or control characters (P058-SEC-02)"
                        );
                    }
                }
            }
            "chain_attempt_index"
            | "metric_sample_ms"
            | "metric_numerator"
            | "metric_denominator" => {
                if !val.is_number() {
                    bail!(
                        "payload_json key '{key}' must be a number, \
                         got {kind} (P058-SEC-L1)",
                        kind = json_type_name(val)
                    );
                }
            }
            "redacted_evidence_ref" => {
                // Must be a string with an approved hash/ref prefix (e.g. sha256:abc, ref/path).
                // P058-SEC-02: rejects bare credentials, URL schemes, absolute paths, traversal.
                if !val.is_string() {
                    bail!(
                        "payload_json key 'redacted_evidence_ref' must be a string, \
                         got {kind} (P058-SEC-L1)",
                        kind = json_type_name(val)
                    );
                }
                let s = val.as_str().unwrap();
                if s.len() > PAYLOAD_EVIDENCE_REF_MAX {
                    bail!(
                        "payload_json key 'redacted_evidence_ref' exceeds maximum \
                         {PAYLOAD_EVIDENCE_REF_MAX} bytes; must be a hash or reference \
                         identifier, not raw evidence (P058-SEC-02)"
                    );
                }
                if !is_safe_ref_value(s, APPROVED_REF_PREFIXES) {
                    bail!(
                        "payload_json key 'redacted_evidence_ref' must be an approved hash/ref \
                         identifier starting with a known prefix (sha256:, hmac-sha256:, ref/, \
                         etc.); URL schemes, absolute paths, path traversal, and bare credentials \
                         are rejected (P058-SEC-02)"
                    );
                }
            }
            _ => {
                if !val.is_string() {
                    bail!(
                        "payload_json key '{key}' must be a string, \
                         got {kind} (P058-SEC-L1)",
                        kind = json_type_name(val)
                    );
                }
                let s = val.as_str().unwrap();
                // Per-value byte cap: prevents oversized strings from reaching readback.
                if s.len() > PAYLOAD_STRING_VALUE_MAX {
                    bail!(
                        "payload_json key '{key}' value exceeds maximum \
                         {PAYLOAD_STRING_VALUE_MAX} bytes; values must be bounded identifiers, \
                         not raw evidence or credential blobs (P058-SEC-02)"
                    );
                }
                // Identifier/enum fields must not contain whitespace or control characters
                // to prevent raw prose, transcripts, or credentials from persisting under
                // allowed key names. P058-SEC-02.
                if !is_safe_identifier_value(s) {
                    bail!(
                        "payload_json key '{key}' must be a bounded identifier with no \
                         whitespace or control characters; raw messages and credentials \
                         are rejected (P058-SEC-02)"
                    );
                }
                // SEC-P058-003: apply the same credential/path/URL pattern rejection used by
                // shadow decision validation to all identifier fields in payload_json (tier_id,
                // policy_id, event_kind_raw, trigger_raw, digest_version, etc.).
                if has_credential_pattern(s) {
                    bail!(
                        "payload_json key '{key}' contains a credential-shaped, path, or URL \
                         value; only bounded tier/policy identifier values are permitted \
                         (P058-SEC-03)"
                    );
                }
            }
        }
    }

    // Validate digest_inputs sub-object when present.
    if let Some(di) = obj.get("digest_inputs") {
        let di_obj = match di {
            serde_json::Value::Object(o) => o,
            _ => bail!("payload_json 'digest_inputs' must be a JSON object (P058-SEC-L1)"),
        };
        for (key, val) in di_obj.iter() {
            if !DIGEST_INPUT_KEYS.contains(&key.as_str()) {
                bail!(
                    "payload_json digest_inputs contains unknown key '{}'; permitted keys are \
                     failure_kind, output_settlement_state, validation_evidence_kind, \
                     redacted_message_fragment_hash (P058-SEC-L1)",
                    key
                );
            }
            if !val.is_string() {
                bail!(
                    "payload_json digest_inputs key '{key}' must be a string, \
                     got {kind} (P058-SEC-L1)",
                    kind = json_type_name(val)
                );
            }
            let s = val.as_str().unwrap();
            // P058-SEC-02: redacted_message_fragment_hash requires an approved hash prefix.
            // Rejects raw evidence, bare credentials, URL schemes, and absolute paths.
            if key == "redacted_message_fragment_hash" {
                if s.len() > PAYLOAD_FRAGMENT_HASH_MAX {
                    bail!(
                        "payload_json digest_inputs.redacted_message_fragment_hash exceeds \
                         maximum {PAYLOAD_FRAGMENT_HASH_MAX} bytes; must be a bounded hash \
                         identifier, not raw evidence (P058-SEC-02)"
                    );
                }
                if !is_safe_ref_value(s, APPROVED_HASH_PREFIXES) {
                    bail!(
                        "payload_json digest_inputs.redacted_message_fragment_hash must be an \
                         approved hash identifier starting with a known hash prefix (sha256:, \
                         hmac-sha256:, etc.); raw evidence, URL schemes, absolute paths, and \
                         bare credentials are rejected (P058-SEC-02)"
                    );
                }
            } else {
                if s.len() > PAYLOAD_STRING_VALUE_MAX {
                    bail!(
                        "payload_json digest_inputs key '{key}' value exceeds maximum \
                         {PAYLOAD_STRING_VALUE_MAX} bytes (P058-SEC-02)"
                    );
                }
                // digest_inputs identifier fields must not carry raw prose or credentials.
                if !is_safe_identifier_value(s) {
                    bail!(
                        "payload_json digest_inputs key '{key}' must be a bounded identifier \
                         with no whitespace or control characters (P058-SEC-02)"
                    );
                }
                // MEDIUM-001: apply credential/path/URL pattern rejection to the typed
                // digest_inputs identifier fields (failure_kind, output_settlement_state,
                // validation_evidence_kind). A no-whitespace identifier check alone is
                // insufficient — credential-shaped tokens like sk-..., /Users/..., https://...
                // pass the identifier check but must not reach readback.
                if has_credential_pattern(s) {
                    bail!(
                        "payload_json digest_inputs key '{key}' contains a credential-shaped, \
                         path, or URL value; only bounded typed identifier values are permitted \
                         (P058-SEC-M1/MEDIUM-001)"
                    );
                }
            }
        }
    }

    // Re-serialize from the parsed Value to produce a canonical string.
    // This collapses duplicate keys (serde_json keeps the last occurrence) so the raw
    // input — which may contain {"redacted_evidence_ref":"sk-secret","redacted_evidence_ref":"sha256:..."}
    // — cannot bypass the redaction boundary (HIGH-001). The canonical string is what
    // callers must store and surface through GraphQL/MCP readback.
    let canonical = serde_json::to_string(&v)
        .map_err(|e| anyhow!("failed to re-serialize payload_json to canonical form: {e}"))?;
    Ok(canonical)
}

/// P058 Phase 2 readback projection derived only from durable, redacted escalation_events.
/// This keeps GraphQL and MCP parity without trusting client-side reconstruction.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct EscalationLedgerRuntimeReadback {
    pub waiting_retry_after_until: Option<String>,
    pub trace_unavailable_reason_raw: Option<String>,
    pub escalation_trace_json_redacted: Option<String>,
    pub policy_drift_state: Option<String>,
    pub external_acknowledgement_ref: Option<String>,
    pub feature_flag_state: Option<String>,
}

pub fn runtime_readback_from_events(
    ledger: &EscalationLedger,
    events: &[EscalationEvent],
    events_truncated: bool,
) -> EscalationLedgerRuntimeReadback {
    let mut waiting_retry_after_until = None;
    let mut external_acknowledgement_ref = None;
    let mut trace_events = Vec::with_capacity(events.len());

    for event in events {
        let payload = event
            .payload_json
            .as_deref()
            .and_then(|raw| serde_json::from_str::<serde_json::Value>(raw).ok());
        if let Some(obj) = payload.as_ref().and_then(|v| v.as_object()) {
            if let Some(value) = obj
                .get("waiting_retry_after_until")
                .and_then(|v| v.as_str())
            {
                waiting_retry_after_until = Some(value.to_string());
            }
            if let Some(value) = obj
                .get("external_acknowledgement_ref")
                .and_then(|v| v.as_str())
            {
                external_acknowledgement_ref = Some(value.to_string());
            }
        }

        trace_events.push(serde_json::json!({
            "event_kind_raw": event.event_kind_raw,
            "tier_id": event.tier_id,
            "tier_kind_raw": event.tier_kind_raw,
            "trigger_raw": event.trigger_raw,
            "pause_reason_raw": event.pause_reason_raw,
            "payload_json": event.payload_json,
            "redaction_version": event.redaction_version,
            "created_at": event.created_at.to_rfc3339(),
        }));
    }

    let escalation_trace_json_redacted = if trace_events.is_empty() {
        None
    } else {
        Some(
            serde_json::json!({
                "schema_version": "p058_escalation_trace_redacted_v1",
                "redaction_version": "redaction_v1",
                "events": trace_events,
                "events_truncated": events_truncated,
            })
            .to_string(),
        )
    };

    let policy_drift_state =
        if ledger.pause_reason_raw.as_deref() == Some("escalation_policy_drift") {
            Some("pending_ack".to_string())
        } else {
            None
        };

    let feature_flag_state = match ledger.pause_reason_raw.as_deref() {
        Some("escalation_kill_switch_engaged") => Some("kill_switch_engaged".to_string()),
        Some("escalation_policy_disabled") => Some("policy_disabled".to_string()),
        Some("escalation_policy_drift") => Some("policy_drift_pending_ack".to_string()),
        _ if ledger.trigger_raw.is_some() => Some("in_flight_continue".to_string()),
        _ => None,
    };

    EscalationLedgerRuntimeReadback {
        waiting_retry_after_until,
        trace_unavailable_reason_raw: events_truncated.then(|| "event_cap_exceeded".to_string()),
        escalation_trace_json_redacted,
        policy_drift_state,
        external_acknowledgement_ref,
        feature_flag_state,
    }
}

pub fn digest_inputs_for_meta_from_events(
    events: &[EscalationEvent],
    tier_id: &str,
    trigger_raw: Option<&str>,
) -> Option<String> {
    events.iter().rev().find_map(|event| {
        if event.tier_id.as_deref() != Some(tier_id) {
            return None;
        }
        if let Some(trigger) = trigger_raw {
            if event.trigger_raw.as_deref() != Some(trigger) {
                return None;
            }
        }
        let payload = event.payload_json.as_deref()?;
        let value = serde_json::from_str::<serde_json::Value>(payload).ok()?;
        let digest_inputs = value.get("digest_inputs")?;
        serde_json::to_string(digest_inputs).ok()
    })
}

/// Thin wrapper for callers that only need pass/fail validation (unit tests, etc.).
fn validate_payload_json_shape(json_str: &str) -> Result<()> {
    canonicalize_and_validate_payload_json(json_str).map(|_| ())
}

/// P058-SEC-L2: Per-field byte caps for escalation_ledger and execution_metadata.
/// Internal writers are trusted but these bounds prevent oversized strings from
/// reaching MCP/GraphQL readback via a misbehaving or future code path.
const FIELD_ID_MAX: usize = 256;
const FIELD_HINT_ANCHOR_MAX: usize = 1024; // 1 KiB for human-readable strings
const FIELD_ENUM_RAW_MAX: usize = 256; // tier_kind_raw, trigger_raw, pause_reason_raw, etc.

fn check_field_len(field: &str, value: &str, max: usize) -> Result<()> {
    if value.len() > max {
        bail!(
            "field '{field}' exceeds maximum {max} bytes (got {})",
            value.len()
        );
    }
    Ok(())
}

fn check_opt_field_len(field: &str, value: &Option<String>, max: usize) -> Result<()> {
    if let Some(v) = value {
        check_field_len(field, v, max)?;
    }
    Ok(())
}

/// MEDIUM-002: Checks that `value` is a safe identifier (no whitespace/control characters)
/// and does not contain credential-shaped, path, or URL patterns. Applied to ID/enum columns
/// exposed through GraphQL/MCP readback (policy_id, tier_id, trigger_raw, event_kind_raw, etc.).
fn check_identifier_field(field: &str, value: &str) -> Result<()> {
    check_field_len(field, value, FIELD_ENUM_RAW_MAX)?;
    if !is_safe_identifier_value(value) {
        bail!(
            "field '{field}' must be a safe identifier with no whitespace or control \
             characters; raw messages and credentials are not permitted (P058-SEC-MEDIUM-002)"
        );
    }
    if has_credential_pattern(value) {
        bail!(
            "field '{field}' contains a credential-shaped, path, or URL value; only bounded \
             identifier values are permitted (P058-SEC-MEDIUM-002)"
        );
    }
    Ok(())
}

fn check_opt_identifier_field(field: &str, value: &Option<String>) -> Result<()> {
    if let Some(v) = value {
        check_identifier_field(field, v)?;
    }
    Ok(())
}

/// Checks byte caps on all mutable fields of EscalationLedger (shared by insert and update paths).
/// MEDIUM-002: enum/ID fields also get identifier + credential-pattern validation.
fn check_ledger_mutable_field_lengths(ledger: &EscalationLedger) -> Result<()> {
    check_identifier_field("status_raw", &ledger.status_raw)?;
    check_opt_identifier_field("current_tier_id", &ledger.current_tier_id)?;
    check_opt_identifier_field("current_tier_kind_raw", &ledger.current_tier_kind_raw)?;
    check_opt_identifier_field("trigger_raw", &ledger.trigger_raw)?;
    check_opt_identifier_field("pause_reason_raw", &ledger.pause_reason_raw)?;
    // operator_action_hint: human-readable prose; allow spaces but reject control chars and credentials.
    // P058-SEC-M2: reject control characters to prevent log injection and UI rendering exploits.
    check_opt_field_len(
        "operator_action_hint",
        &ledger.operator_action_hint,
        FIELD_HINT_ANCHOR_MAX,
    )?;
    if let Some(ref s) = ledger.operator_action_hint {
        if has_credential_pattern(s) {
            bail!("field 'operator_action_hint' contains a credential-shaped value (P058-SEC-MEDIUM-002)");
        }
        if s.chars().any(|c| c.is_control() && c != '\t') {
            bail!("field 'operator_action_hint' contains control characters; only printable Unicode and tab are permitted (P058-SEC-M2)");
        }
    }
    // runbook_anchor: slug-style path; validate as alphanumeric plus _./-  (no spaces, no control chars).
    // P058-SEC-M2: constrain to safe slug characters to prevent injection into URLs and log lines.
    check_opt_field_len(
        "runbook_anchor",
        &ledger.runbook_anchor,
        FIELD_HINT_ANCHOR_MAX,
    )?;
    if let Some(ref s) = ledger.runbook_anchor {
        if has_credential_pattern(s) {
            bail!(
                "field 'runbook_anchor' contains a credential-shaped value (P058-SEC-MEDIUM-002)"
            );
        }
        if !s
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '/' | '-'))
        {
            bail!("field 'runbook_anchor' must contain only ASCII alphanumeric, _, ., /, - characters; got non-slug character (P058-SEC-M2)");
        }
    }
    Ok(())
}

/// Returns the JSON type name as a static string for use in error messages.
fn json_type_name(v: &serde_json::Value) -> &'static str {
    match v {
        serde_json::Value::Object(_) => "object",
        serde_json::Value::Array(_) => "array",
        serde_json::Value::String(_) => "string",
        serde_json::Value::Number(_) => "number",
        serde_json::Value::Bool(_) => "bool",
        serde_json::Value::Null => "null",
    }
}

pub async fn insert_ledger(pool: &SqlitePool, ledger: &EscalationLedger) -> Result<()> {
    let mut tx = pool.begin().await?;
    insert_ledger_tx(&mut tx, ledger).await?;
    tx.commit().await?;
    Ok(())
}

pub async fn insert_ledger_tx(
    tx: &mut Transaction<'_, Sqlite>,
    ledger: &EscalationLedger,
) -> Result<()> {
    // MEDIUM-002: ID fields exposed through readback get identifier + credential-pattern check.
    check_identifier_field("id", &ledger.id)?;
    check_identifier_field("stage_id", &ledger.stage_id)?;
    check_identifier_field("agent_id", &ledger.agent_id)?;
    check_identifier_field("policy_id", &ledger.policy_id)?;
    // policy_hash may contain hex digits and prefix; length cap only (not identifier format check).
    check_field_len("policy_hash", &ledger.policy_hash, FIELD_ID_MAX)?;
    if has_credential_pattern(&ledger.policy_hash) {
        bail!("field 'policy_hash' contains a credential-shaped value (P058-SEC-MEDIUM-002)");
    }
    check_ledger_mutable_field_lengths(ledger)?;
    sqlx::query(
        r#"INSERT INTO escalation_ledger
           (id, run_id, stage_id, agent_id, policy_id, policy_hash,
            status_raw, current_tier_id, current_tier_kind_raw,
            chain_attempt_index, trigger_raw, pause_reason_raw,
            operator_action_hint, runbook_anchor, created_at, updated_at)
           VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)"#,
    )
    .bind(&ledger.id)
    .bind(ledger.run_id.to_string())
    .bind(&ledger.stage_id)
    .bind(&ledger.agent_id)
    .bind(&ledger.policy_id)
    .bind(&ledger.policy_hash)
    .bind(&ledger.status_raw)
    .bind(&ledger.current_tier_id)
    .bind(&ledger.current_tier_kind_raw)
    .bind(ledger.chain_attempt_index)
    .bind(&ledger.trigger_raw)
    .bind(&ledger.pause_reason_raw)
    .bind(&ledger.operator_action_hint)
    .bind(&ledger.runbook_anchor)
    .bind(ledger.created_at.to_rfc3339())
    .bind(ledger.updated_at.to_rfc3339())
    .execute(&mut **tx)
    .await?;
    crate::metrics::record_escalation_chain_started(
        &ledger.policy_id,
        ledger.current_tier_kind_raw.as_deref(),
    );
    Ok(())
}

pub async fn update_ledger_tx(
    tx: &mut Transaction<'_, Sqlite>,
    ledger: &EscalationLedger,
) -> Result<()> {
    check_ledger_mutable_field_lengths(ledger)?;
    sqlx::query(
        r#"UPDATE escalation_ledger SET
           status_raw = ?1,
           current_tier_id = ?2,
           current_tier_kind_raw = ?3,
           chain_attempt_index = ?4,
           trigger_raw = ?5,
           pause_reason_raw = ?6,
           operator_action_hint = ?7,
           runbook_anchor = ?8,
           updated_at = ?9
           WHERE id = ?10"#,
    )
    .bind(&ledger.status_raw)
    .bind(&ledger.current_tier_id)
    .bind(&ledger.current_tier_kind_raw)
    .bind(ledger.chain_attempt_index)
    .bind(&ledger.trigger_raw)
    .bind(&ledger.pause_reason_raw)
    .bind(&ledger.operator_action_hint)
    .bind(&ledger.runbook_anchor)
    .bind(ledger.updated_at.to_rfc3339())
    .bind(&ledger.id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub async fn find_ledgers_by_run(
    pool: &SqlitePool,
    run_id: RunId,
) -> Result<Vec<EscalationLedger>> {
    // P058-SEC-02: SQL-level row cap (cap+1) prevents unbounded fetch_all.
    // Application-layer cap is 50; fetching 51 lets callers detect truncation (len > 50).
    let rows = sqlx::query(
        r#"SELECT id, run_id, stage_id, agent_id, policy_id, policy_hash,
                  status_raw, current_tier_id, current_tier_kind_raw,
                  chain_attempt_index, trigger_raw, pause_reason_raw,
                  operator_action_hint, runbook_anchor, created_at, updated_at
           FROM escalation_ledger
           WHERE run_id = ?
           ORDER BY created_at ASC, id ASC
           LIMIT 51"#,
    )
    .bind(run_id.to_string())
    .fetch_all(pool)
    .await?;

    rows.into_iter()
        .map(|row| {
            let run_id_str: String = row.try_get("run_id")?;
            let created_at_str: String = row.try_get("created_at")?;
            let updated_at_str: String = row.try_get("updated_at")?;
            Ok(EscalationLedger {
                id: row.try_get("id")?,
                run_id: run_id_str.parse().map_err(|e| anyhow!("bad run_id: {e}"))?,
                stage_id: row.try_get("stage_id")?,
                agent_id: row.try_get("agent_id")?,
                policy_id: row.try_get("policy_id")?,
                policy_hash: row.try_get("policy_hash")?,
                status_raw: row.try_get("status_raw")?,
                current_tier_id: row.try_get("current_tier_id")?,
                current_tier_kind_raw: row.try_get("current_tier_kind_raw")?,
                chain_attempt_index: row.try_get("chain_attempt_index")?,
                trigger_raw: row.try_get("trigger_raw")?,
                pause_reason_raw: row.try_get("pause_reason_raw")?,
                operator_action_hint: row.try_get("operator_action_hint")?,
                runbook_anchor: row.try_get("runbook_anchor")?,
                created_at: created_at_str
                    .parse()
                    .map_err(|e| anyhow!("bad created_at: {e}"))?,
                updated_at: updated_at_str
                    .parse()
                    .map_err(|e| anyhow!("bad updated_at: {e}"))?,
            })
        })
        .collect()
}

/// Return the exact count of escalation_ledger rows for a run.
/// Used by readback to report chains_total accurately when the fetch is capped.
pub async fn count_ledgers_by_run(pool: &SqlitePool, run_id: RunId) -> Result<i64> {
    let row = sqlx::query("SELECT COUNT(*) as c FROM escalation_ledger WHERE run_id = ?")
        .bind(run_id.to_string())
        .fetch_one(pool)
        .await?;
    Ok(row.try_get::<i64, _>("c")?)
}

/// Return the count of triggered or non-idle escalation chains for a run.
/// A chain is "triggered" when its trigger_raw IS NOT NULL (a failure has fired an escalation
/// event) OR its status_raw is not 'active' (it has advanced, paused, or been exhausted).
/// Used by has_active_escalation readback: a chain created at claim time (trigger_raw NULL,
/// status_raw = 'active') is not yet an active escalation — it is just the first attempt with
/// a configured policy. BLOCK-2: prevents has_active_escalation from flipping true on every
/// claim start just because a policy is configured.
pub async fn count_triggered_ledgers_by_run(pool: &SqlitePool, run_id: RunId) -> Result<i64> {
    let row = sqlx::query(
        "SELECT COUNT(*) as c FROM escalation_ledger \
         WHERE run_id = ? AND (trigger_raw IS NOT NULL OR status_raw != 'active')",
    )
    .bind(run_id.to_string())
    .fetch_one(pool)
    .await?;
    Ok(row.try_get::<i64, _>("c")?)
}

/// Return the count of paused or exhausted chains for a run (unbounded aggregate).
/// Used by readback for paused_chain_count when there may be more than ESCALATION_MAX_LEDGERS rows.
pub async fn count_paused_ledgers_by_run(pool: &SqlitePool, run_id: RunId) -> Result<i64> {
    let row = sqlx::query(
        "SELECT COUNT(*) as c FROM escalation_ledger \
         WHERE run_id = ? AND (status_raw = 'paused' OR status_raw = 'exhausted')",
    )
    .bind(run_id.to_string())
    .fetch_one(pool)
    .await?;
    Ok(row.try_get::<i64, _>("c")?)
}

/// Return the pause_reason_raw of the highest-severity paused/exhausted chain for a run.
/// Severity follows the banner-precedence ordering from the proposal EscalationBannerStack spec:
///   kill_switch > policy_drift > policy_disabled > recovery_inconsistent >
///   capacity_probe_failed > shadow_mode > (any other value, by created_at ASC)
/// This ensures the operator surface surfaces the most actionable pause first when multiple
/// paused chains coexist, rather than returning an older but lower-severity pause (P058-SEC-LOW-03).
pub async fn dominant_pause_reason_for_run(
    pool: &SqlitePool,
    run_id: RunId,
) -> Result<Option<String>> {
    let rows = sqlx::query(
        "SELECT pause_reason_raw FROM escalation_ledger \
         WHERE run_id = ? AND (status_raw = 'paused' OR status_raw = 'exhausted') \
         ORDER BY created_at ASC \
         LIMIT 1000",
    )
    .bind(run_id.to_string())
    .fetch_all(pool)
    .await?;

    if rows.is_empty() {
        return Ok(None);
    }

    // Banner-precedence ordering (highest severity first). Any unknown pause_reason_raw
    // values are ranked after all known codes.
    const PRECEDENCE: &[&str] = &[
        "escalation_kill_switch_engaged",
        "escalation_policy_drift",
        "escalation_policy_disabled",
        "escalation_recovery_inconsistent",
        "capacity_probe_failed",
        "shadow_mode",
    ];

    let reasons: Vec<Option<String>> = rows
        .into_iter()
        .map(|r| {
            r.try_get::<Option<String>, _>("pause_reason_raw")
                .ok()
                .flatten()
        })
        .collect();

    // Return the first reason that matches the highest-precedence position, falling back
    // to the earliest row's reason when no known code is present.
    for &code in PRECEDENCE {
        if reasons.iter().any(|r| r.as_deref() == Some(code)) {
            return Ok(Some(code.to_string()));
        }
    }

    // No known high-severity code found — return the earliest chain's pause reason.
    Ok(reasons.into_iter().flatten().next())
}

/// Return the exact count of events for a ledger.
/// Used by readback to report events_total accurately when the fetch is capped.
pub async fn count_events_by_ledger(pool: &SqlitePool, escalation_ledger_id: &str) -> Result<i64> {
    let row =
        sqlx::query("SELECT COUNT(*) as c FROM escalation_events WHERE escalation_ledger_id = ?")
            .bind(escalation_ledger_id)
            .fetch_one(pool)
            .await?;
    Ok(row.try_get::<i64, _>("c")?)
}

/// Return the exact count of execution_metadata rows for a ledger.
/// Used by readback to report execution_metas_total accurately when the fetch is capped.
pub async fn count_metas_by_ledger(pool: &SqlitePool, escalation_ledger_id: &str) -> Result<i64> {
    let row = sqlx::query(
        "SELECT COUNT(*) as c FROM escalation_execution_metadata WHERE escalation_ledger_id = ?",
    )
    .bind(escalation_ledger_id)
    .fetch_one(pool)
    .await?;
    Ok(row.try_get::<i64, _>("c")?)
}

/// Count execution metadata rows for a ledger in a recent time window.
///
/// P058 uses this as the durable launch-recycle storm detector: a retry chain
/// that repeatedly reaches provider launch in a short window is paused instead
/// of spawning another retry that is likely to fail the same way.
pub async fn count_recent_metas_by_ledger(
    pool: &SqlitePool,
    escalation_ledger_id: &str,
    since: DateTime<Utc>,
) -> Result<i64> {
    let row = sqlx::query(
        "SELECT COUNT(*) as c FROM escalation_execution_metadata
         WHERE escalation_ledger_id = ? AND created_at >= ?",
    )
    .bind(escalation_ledger_id)
    .bind(since.to_rfc3339())
    .fetch_one(pool)
    .await?;
    Ok(row.try_get::<i64, _>("c")?)
}

pub async fn find_ledger_by_id(
    pool: &SqlitePool,
    ledger_id: &str,
) -> Result<Option<EscalationLedger>> {
    let row = sqlx::query(
        r#"SELECT id, run_id, stage_id, agent_id, policy_id, policy_hash,
                  status_raw, current_tier_id, current_tier_kind_raw,
                  chain_attempt_index, trigger_raw, pause_reason_raw,
                  operator_action_hint, runbook_anchor, created_at, updated_at
           FROM escalation_ledger
           WHERE id = ?"#,
    )
    .bind(ledger_id)
    .fetch_optional(pool)
    .await?;

    row.map(|row| {
        let run_id_str: String = row.try_get("run_id")?;
        let created_at_str: String = row.try_get("created_at")?;
        let updated_at_str: String = row.try_get("updated_at")?;
        Ok(EscalationLedger {
            id: row.try_get("id")?,
            run_id: run_id_str.parse().map_err(|e| anyhow!("bad run_id: {e}"))?,
            stage_id: row.try_get("stage_id")?,
            agent_id: row.try_get("agent_id")?,
            policy_id: row.try_get("policy_id")?,
            policy_hash: row.try_get("policy_hash")?,
            status_raw: row.try_get("status_raw")?,
            current_tier_id: row.try_get("current_tier_id")?,
            current_tier_kind_raw: row.try_get("current_tier_kind_raw")?,
            chain_attempt_index: row.try_get("chain_attempt_index")?,
            trigger_raw: row.try_get("trigger_raw")?,
            pause_reason_raw: row.try_get("pause_reason_raw")?,
            operator_action_hint: row.try_get("operator_action_hint")?,
            runbook_anchor: row.try_get("runbook_anchor")?,
            created_at: created_at_str
                .parse()
                .map_err(|e| anyhow!("bad created_at: {e}"))?,
            updated_at: updated_at_str
                .parse()
                .map_err(|e| anyhow!("bad updated_at: {e}"))?,
        })
    })
    .transpose()
}

/// Look up an escalation ledger by its chain key: (run_id, stage_id, agent_id, policy_id).
/// Returns None when no matching chain exists yet.
pub async fn find_ledger_by_chain_key(
    pool: &SqlitePool,
    run_id: RunId,
    stage_id: &str,
    agent_id: &str,
    policy_id: &str,
) -> Result<Option<EscalationLedger>> {
    let row = sqlx::query(
        r#"SELECT id, run_id, stage_id, agent_id, policy_id, policy_hash,
                  status_raw, current_tier_id, current_tier_kind_raw,
                  chain_attempt_index, trigger_raw, pause_reason_raw,
                  operator_action_hint, runbook_anchor, created_at, updated_at
           FROM escalation_ledger
           WHERE run_id = ? AND stage_id = ? AND agent_id = ? AND policy_id = ?
           ORDER BY created_at ASC, id ASC
           LIMIT 1"#,
    )
    .bind(run_id.to_string())
    .bind(stage_id)
    .bind(agent_id)
    .bind(policy_id)
    .fetch_optional(pool)
    .await?;

    row.map(|row| {
        let run_id_str: String = row.try_get("run_id")?;
        let created_at_str: String = row.try_get("created_at")?;
        let updated_at_str: String = row.try_get("updated_at")?;
        Ok(EscalationLedger {
            id: row.try_get("id")?,
            run_id: run_id_str.parse().map_err(|e| anyhow!("bad run_id: {e}"))?,
            stage_id: row.try_get("stage_id")?,
            agent_id: row.try_get("agent_id")?,
            policy_id: row.try_get("policy_id")?,
            policy_hash: row.try_get("policy_hash")?,
            status_raw: row.try_get("status_raw")?,
            current_tier_id: row.try_get("current_tier_id")?,
            current_tier_kind_raw: row.try_get("current_tier_kind_raw")?,
            chain_attempt_index: row.try_get("chain_attempt_index")?,
            trigger_raw: row.try_get("trigger_raw")?,
            pause_reason_raw: row.try_get("pause_reason_raw")?,
            operator_action_hint: row.try_get("operator_action_hint")?,
            runbook_anchor: row.try_get("runbook_anchor")?,
            created_at: created_at_str
                .parse()
                .map_err(|e| anyhow!("bad created_at: {e}"))?,
            updated_at: updated_at_str
                .parse()
                .map_err(|e| anyhow!("bad updated_at: {e}"))?,
        })
    })
    .transpose()
}

/// Insert a new escalation ledger row if none exists for this chain key, or return the existing
/// ledger_id. Uses INSERT OR IGNORE + SELECT inside a single transaction so no interleaved
/// mutator can change policy_hash between the insert and the readback (P058-SEC-LOW-01).
///
/// Returns the ledger id (whether newly inserted or pre-existing).
pub async fn insert_or_ignore_ledger(
    pool: &SqlitePool,
    ledger: &EscalationLedger,
) -> Result<String> {
    check_identifier_field("id", &ledger.id)?;
    check_identifier_field("stage_id", &ledger.stage_id)?;
    check_identifier_field("agent_id", &ledger.agent_id)?;
    check_identifier_field("policy_id", &ledger.policy_id)?;
    check_field_len("policy_hash", &ledger.policy_hash, FIELD_ID_MAX)?;
    if has_credential_pattern(&ledger.policy_hash) {
        bail!("field 'policy_hash' contains a credential-shaped value (P058-SEC-MEDIUM-002)");
    }
    check_ledger_mutable_field_lengths(ledger)?;

    let mut tx = pool.begin().await?;

    let insert_result = sqlx::query(
        r#"INSERT OR IGNORE INTO escalation_ledger
           (id, run_id, stage_id, agent_id, policy_id, policy_hash,
            status_raw, current_tier_id, current_tier_kind_raw,
            chain_attempt_index, trigger_raw, pause_reason_raw,
            operator_action_hint, runbook_anchor, created_at, updated_at)
           VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)"#,
    )
    .bind(&ledger.id)
    .bind(ledger.run_id.to_string())
    .bind(&ledger.stage_id)
    .bind(&ledger.agent_id)
    .bind(&ledger.policy_id)
    .bind(&ledger.policy_hash)
    .bind(&ledger.status_raw)
    .bind(&ledger.current_tier_id)
    .bind(&ledger.current_tier_kind_raw)
    .bind(ledger.chain_attempt_index)
    .bind(&ledger.trigger_raw)
    .bind(&ledger.pause_reason_raw)
    .bind(&ledger.operator_action_hint)
    .bind(&ledger.runbook_anchor)
    .bind(ledger.created_at.to_rfc3339())
    .bind(ledger.updated_at.to_rfc3339())
    .execute(&mut *tx)
    .await?;
    if insert_result.rows_affected() > 0 {
        crate::metrics::record_escalation_chain_started(
            &ledger.policy_id,
            ledger.current_tier_kind_raw.as_deref(),
        );
    }

    // Fetch the canonical ledger row inside the same transaction so the INSERT and the
    // readback are atomic. Compare policy_hash to detect silent reuse of a stale chain
    // (MEDIUM-002): if the existing row's frozen policy_hash differs from the candidate,
    // the run plan has drifted and we must refuse silent reuse rather than silently
    // attaching new executions to a chain whose frozen hash no longer matches.
    let existing: (String, String) = sqlx::query_as(
        r#"SELECT id, policy_hash FROM escalation_ledger
           WHERE run_id = ? AND stage_id = ? AND agent_id = ? AND policy_id = ?
           LIMIT 1"#,
    )
    .bind(ledger.run_id.to_string())
    .bind(&ledger.stage_id)
    .bind(&ledger.agent_id)
    .bind(&ledger.policy_id)
    .fetch_one(&mut *tx)
    .await?;

    let (existing_id, existing_policy_hash) = existing;
    if existing_policy_hash != ledger.policy_hash {
        // Roll back the INSERT OR IGNORE (which was a no-op anyway since the existing row
        // was already present). Then open a durable drift pause on the existing ledger row
        // using the pool directly so the pause is committed to the DB even though the
        // caller's in-flight tx was rolled back (P058 MEDIUM-002).
        let _ = tx.rollback().await;
        open_drift_pause(pool, &existing_id).await?;
        return Err(anyhow::Error::from(EscalationPolicyDrift {
            ledger_id: existing_id.clone(),
        })
        .context(format!(
            "escalation_policy_drift: durable pause opened on ledger_id={existing_id} for \
             run_id={} stage_id={} agent_id={}; frozen policy_hash={existing_policy_hash} \
             differs from candidate policy_hash={}; operator must acknowledge drift via \
             external MCP/operator workflow before new executions can be attributed to this \
             chain (P058 MEDIUM-002)",
            ledger.run_id, ledger.stage_id, ledger.agent_id, ledger.policy_hash,
        )));
    }

    tx.commit().await?;
    Ok(existing_id)
}

/// Transaction-scoped variant of `insert_or_ignore_ledger`.
///
/// Performs INSERT OR IGNORE and the policy_hash drift check within the caller's transaction.
/// Used by the atomic scheduler init path that combines ledger + agent_execution + metadata
/// in a single SQLite transaction (proposal §architecture.scheduler_transaction).
pub async fn insert_or_ignore_ledger_tx(
    tx: &mut Transaction<'_, Sqlite>,
    ledger: &EscalationLedger,
) -> Result<String> {
    check_identifier_field("id", &ledger.id)?;
    check_identifier_field("stage_id", &ledger.stage_id)?;
    check_identifier_field("agent_id", &ledger.agent_id)?;
    check_identifier_field("policy_id", &ledger.policy_id)?;
    check_field_len("policy_hash", &ledger.policy_hash, FIELD_ID_MAX)?;
    if has_credential_pattern(&ledger.policy_hash) {
        bail!("field 'policy_hash' contains a credential-shaped value (P058-SEC-MEDIUM-002)");
    }
    check_ledger_mutable_field_lengths(ledger)?;

    let insert_result = sqlx::query(
        r#"INSERT OR IGNORE INTO escalation_ledger
           (id, run_id, stage_id, agent_id, policy_id, policy_hash,
            status_raw, current_tier_id, current_tier_kind_raw,
            chain_attempt_index, trigger_raw, pause_reason_raw,
            operator_action_hint, runbook_anchor, created_at, updated_at)
           VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)"#,
    )
    .bind(&ledger.id)
    .bind(ledger.run_id.to_string())
    .bind(&ledger.stage_id)
    .bind(&ledger.agent_id)
    .bind(&ledger.policy_id)
    .bind(&ledger.policy_hash)
    .bind(&ledger.status_raw)
    .bind(&ledger.current_tier_id)
    .bind(&ledger.current_tier_kind_raw)
    .bind(ledger.chain_attempt_index)
    .bind(&ledger.trigger_raw)
    .bind(&ledger.pause_reason_raw)
    .bind(&ledger.operator_action_hint)
    .bind(&ledger.runbook_anchor)
    .bind(ledger.created_at.to_rfc3339())
    .bind(ledger.updated_at.to_rfc3339())
    .execute(&mut **tx)
    .await?;
    if insert_result.rows_affected() > 0 {
        crate::metrics::record_escalation_chain_started(
            &ledger.policy_id,
            ledger.current_tier_kind_raw.as_deref(),
        );
    }

    let (existing_id, existing_policy_hash): (String, String) = sqlx::query_as(
        r#"SELECT id, policy_hash FROM escalation_ledger
           WHERE run_id = ? AND stage_id = ? AND agent_id = ? AND policy_id = ?
           LIMIT 1"#,
    )
    .bind(ledger.run_id.to_string())
    .bind(&ledger.stage_id)
    .bind(&ledger.agent_id)
    .bind(&ledger.policy_id)
    .fetch_one(&mut **tx)
    .await?;

    if existing_policy_hash != ledger.policy_hash {
        // Return a typed EscalationPolicyDrift error containing the existing ledger_id.
        // The caller's transaction will be rolled back on drop. The caller MUST then call
        // `open_drift_pause(pool, ledger_id)` to commit the durable pause state separately
        // (proposal P058 MEDIUM-002: drift must open a durable escalation_policy_drift pause).
        return Err(anyhow::Error::from(EscalationPolicyDrift {
            ledger_id: existing_id.clone(),
        })
        .context(format!(
            "escalation_policy_drift: ledger_id={existing_id} for run_id={} stage_id={} \
             agent_id={}; frozen policy_hash={existing_policy_hash} differs from \
             candidate policy_hash={}; caller must call open_drift_pause before proceeding \
             (P058 MEDIUM-002)",
            ledger.run_id, ledger.stage_id, ledger.agent_id, ledger.policy_hash,
        )));
    }

    Ok(existing_id)
}

pub async fn insert_execution_metadata_tx(
    tx: &mut Transaction<'_, Sqlite>,
    meta: &EscalationExecutionMetadata,
) -> Result<()> {
    // MEDIUM-002: identifier + credential-pattern check for metadata columns in readback.
    check_identifier_field("escalation_ledger_id", &meta.escalation_ledger_id)?;
    check_identifier_field("tier_id", &meta.tier_id)?;
    check_identifier_field("tier_kind_raw", &meta.tier_kind_raw)?;
    check_opt_identifier_field("trigger_raw", &meta.trigger_raw)?;
    check_opt_identifier_field("digest_version", &meta.digest_version)?;
    sqlx::query(
        r#"INSERT INTO escalation_execution_metadata
           (agent_execution_id, escalation_ledger_id, tier_id, tier_kind_raw,
            tier_attempt_index, trigger_raw, digest_version, capacity_probe_counter,
            created_at, updated_at)
           VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)"#,
    )
    .bind(meta.agent_execution_id.to_string())
    .bind(&meta.escalation_ledger_id)
    .bind(&meta.tier_id)
    .bind(&meta.tier_kind_raw)
    .bind(meta.tier_attempt_index)
    .bind(&meta.trigger_raw)
    .bind(&meta.digest_version)
    .bind(meta.capacity_probe_counter)
    .bind(meta.created_at.to_rfc3339())
    .bind(meta.updated_at.to_rfc3339())
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// Pool-based wrapper for runtime callers that do not already hold a transaction.
pub async fn insert_execution_metadata(
    pool: &SqlitePool,
    meta: &EscalationExecutionMetadata,
) -> Result<()> {
    let mut tx = pool.begin().await?;
    insert_execution_metadata_tx(&mut tx, meta).await?;
    tx.commit().await?;
    Ok(())
}

/// Returns the next `tier_attempt_index` for the given `(escalation_ledger_id, tier_id)` pair.
///
/// Queries MAX(tier_attempt_index) from committed rows visible inside the current transaction
/// and returns max + 1, or 0 if no prior rows exist. Must be called inside the scheduler
/// transaction before inserting a new execution-metadata row to satisfy the idempotency unique
/// index on (escalation_ledger_id, tier_id, tier_attempt_index).
pub async fn next_tier_attempt_index_tx(
    tx: &mut Transaction<'_, Sqlite>,
    escalation_ledger_id: &str,
    tier_id: &str,
) -> Result<i64> {
    let row = sqlx::query(
        "SELECT COALESCE(MAX(tier_attempt_index), -1) AS max_idx
         FROM escalation_execution_metadata
         WHERE escalation_ledger_id = ? AND tier_id = ?",
    )
    .bind(escalation_ledger_id)
    .bind(tier_id)
    .fetch_one(&mut **tx)
    .await?;
    let max_idx: i64 = row.try_get("max_idx")?;
    Ok(max_idx + 1)
}

/// P058 Phase 1b: Look up escalation execution metadata for a single agent execution.
/// Returns None when no escalation coverage exists for this execution (the agent ran without
/// an active escalation policy, or the execution pre-dates escalation schema rollout).
pub async fn find_execution_metadata_for_agent(
    pool: &SqlitePool,
    agent_execution_id_str: &str,
) -> Result<Option<EscalationExecutionMetadata>> {
    use domain::ids::AgentExecutionId;
    let row = sqlx::query(
        r#"SELECT em.agent_execution_id, em.escalation_ledger_id, em.tier_id, em.tier_kind_raw,
                  em.tier_attempt_index, em.trigger_raw, em.digest_version,
                  em.capacity_probe_counter, em.created_at, em.updated_at,
                  rf.would_select_tier_id, rf.would_select_trigger_raw,
                  rf.would_select_decision_json
           FROM escalation_execution_metadata em
           LEFT JOIN agent_execution_runtime_facts rf
                  ON rf.agent_execution_id = em.agent_execution_id
           WHERE em.agent_execution_id = ?
           LIMIT 1"#,
    )
    .bind(agent_execution_id_str)
    .fetch_optional(pool)
    .await?;

    row.map(|row| {
        let exec_id_str: String = row.try_get("agent_execution_id")?;
        let created_at_str: String = row.try_get("created_at")?;
        let updated_at_str: String = row.try_get("updated_at")?;
        Ok(EscalationExecutionMetadata {
            agent_execution_id: exec_id_str
                .parse::<AgentExecutionId>()
                .map_err(|e| anyhow!("bad agent_execution_id: {e}"))?,
            escalation_ledger_id: row.try_get("escalation_ledger_id")?,
            tier_id: row.try_get("tier_id")?,
            tier_kind_raw: row.try_get("tier_kind_raw")?,
            tier_attempt_index: row.try_get("tier_attempt_index")?,
            trigger_raw: row.try_get("trigger_raw")?,
            digest_version: row.try_get("digest_version")?,
            capacity_probe_counter: row.try_get("capacity_probe_counter")?,
            created_at: created_at_str
                .parse()
                .map_err(|e| anyhow!("bad created_at: {e}"))?,
            updated_at: updated_at_str
                .parse()
                .map_err(|e| anyhow!("bad updated_at: {e}"))?,
            would_select_tier_id: row.try_get("would_select_tier_id")?,
            would_select_trigger_raw: row.try_get("would_select_trigger_raw")?,
            would_select_decision_json: row.try_get("would_select_decision_json")?,
        })
    })
    .transpose()
}

pub async fn find_execution_metadata_for_agent_tx(
    tx: &mut Transaction<'_, Sqlite>,
    agent_execution_id_str: &str,
) -> Result<Option<EscalationExecutionMetadata>> {
    use domain::ids::AgentExecutionId;
    let row = sqlx::query(
        r#"SELECT em.agent_execution_id, em.escalation_ledger_id, em.tier_id, em.tier_kind_raw,
                  em.tier_attempt_index, em.trigger_raw, em.digest_version,
                  em.capacity_probe_counter, em.created_at, em.updated_at,
                  rf.would_select_tier_id, rf.would_select_trigger_raw,
                  rf.would_select_decision_json
           FROM escalation_execution_metadata em
           LEFT JOIN agent_execution_runtime_facts rf
                  ON rf.agent_execution_id = em.agent_execution_id
           WHERE em.agent_execution_id = ?
           LIMIT 1"#,
    )
    .bind(agent_execution_id_str)
    .fetch_optional(&mut **tx)
    .await?;

    row.map(|row| {
        let exec_id_str: String = row.try_get("agent_execution_id")?;
        let created_at_str: String = row.try_get("created_at")?;
        let updated_at_str: String = row.try_get("updated_at")?;
        Ok(EscalationExecutionMetadata {
            agent_execution_id: exec_id_str
                .parse::<AgentExecutionId>()
                .map_err(|e| anyhow!("bad agent_execution_id: {e}"))?,
            escalation_ledger_id: row.try_get("escalation_ledger_id")?,
            tier_id: row.try_get("tier_id")?,
            tier_kind_raw: row.try_get("tier_kind_raw")?,
            tier_attempt_index: row.try_get("tier_attempt_index")?,
            trigger_raw: row.try_get("trigger_raw")?,
            digest_version: row.try_get("digest_version")?,
            capacity_probe_counter: row.try_get("capacity_probe_counter")?,
            created_at: created_at_str
                .parse()
                .map_err(|e| anyhow!("bad created_at: {e}"))?,
            updated_at: updated_at_str
                .parse()
                .map_err(|e| anyhow!("bad updated_at: {e}"))?,
            would_select_tier_id: row.try_get("would_select_tier_id")?,
            would_select_trigger_raw: row.try_get("would_select_trigger_raw")?,
            would_select_decision_json: row.try_get("would_select_decision_json")?,
        })
    })
    .transpose()
}

pub async fn find_execution_metadata_by_ledger(
    pool: &SqlitePool,
    escalation_ledger_id: &str,
) -> Result<Vec<EscalationExecutionMetadata>> {
    use domain::ids::AgentExecutionId;
    // P058-SEC-02: SQL-level row cap (cap+1) prevents unbounded fetch_all.
    // Application-layer cap is 100; fetching 101 lets callers detect truncation (len > 100).
    // Phase 1b: LEFT JOIN agent_execution_runtime_facts to include shadow selection columns.
    let rows = sqlx::query(
        r#"SELECT em.agent_execution_id, em.escalation_ledger_id, em.tier_id, em.tier_kind_raw,
                  em.tier_attempt_index, em.trigger_raw, em.digest_version,
                  em.capacity_probe_counter, em.created_at, em.updated_at,
                  rf.would_select_tier_id, rf.would_select_trigger_raw,
                  rf.would_select_decision_json
           FROM escalation_execution_metadata em
           LEFT JOIN agent_execution_runtime_facts rf
                  ON rf.agent_execution_id = em.agent_execution_id
           WHERE em.escalation_ledger_id = ?
           ORDER BY em.created_at ASC, em.agent_execution_id ASC
           LIMIT 101"#,
    )
    .bind(escalation_ledger_id)
    .fetch_all(pool)
    .await?;

    rows.into_iter()
        .map(|row| {
            let exec_id_str: String = row.try_get("agent_execution_id")?;
            let created_at_str: String = row.try_get("created_at")?;
            let updated_at_str: String = row.try_get("updated_at")?;
            Ok(EscalationExecutionMetadata {
                agent_execution_id: exec_id_str
                    .parse::<AgentExecutionId>()
                    .map_err(|e| anyhow!("bad agent_execution_id: {e}"))?,
                escalation_ledger_id: row.try_get("escalation_ledger_id")?,
                tier_id: row.try_get("tier_id")?,
                tier_kind_raw: row.try_get("tier_kind_raw")?,
                tier_attempt_index: row.try_get("tier_attempt_index")?,
                trigger_raw: row.try_get("trigger_raw")?,
                digest_version: row.try_get("digest_version")?,
                capacity_probe_counter: row.try_get("capacity_probe_counter")?,
                created_at: created_at_str
                    .parse()
                    .map_err(|e| anyhow!("bad created_at: {e}"))?,
                updated_at: updated_at_str
                    .parse()
                    .map_err(|e| anyhow!("bad updated_at: {e}"))?,
                would_select_tier_id: row.try_get("would_select_tier_id")?,
                would_select_trigger_raw: row.try_get("would_select_trigger_raw")?,
                would_select_decision_json: row.try_get("would_select_decision_json")?,
            })
        })
        .collect()
}

/// Maximum size for payload_json in escalation_events (P058-SEC-L2).
/// Enforced before any JSON validation or DB write to prevent unbounded SQLite growth.
const PAYLOAD_JSON_MAX_BYTES: usize = 64 * 1024; // 64 KiB

pub async fn insert_event(pool: &SqlitePool, event: &EscalationEvent) -> Result<()> {
    let mut tx = pool.begin().await?;
    insert_event_tx(&mut tx, event).await?;
    tx.commit().await?;
    Ok(())
}

pub async fn insert_event_tx(
    tx: &mut Transaction<'_, Sqlite>,
    event: &EscalationEvent,
) -> Result<()> {
    // P058-SEC-03 + MEDIUM-002: identifier + credential-pattern validation for event columns
    // exposed through GraphQL/MCP readback.
    check_identifier_field("id", &event.id)?;
    check_identifier_field("escalation_ledger_id", &event.escalation_ledger_id)?;
    check_identifier_field("event_kind_raw", &event.event_kind_raw)?;
    check_opt_identifier_field("tier_id", &event.tier_id)?;
    check_opt_identifier_field("tier_kind_raw", &event.tier_kind_raw)?;
    check_opt_identifier_field("trigger_raw", &event.trigger_raw)?;
    check_opt_identifier_field("pause_reason_raw", &event.pause_reason_raw)?;

    // Enforce payload_json size cap before JSON validation — prevents unbounded SQLite growth.
    if let Some(ref json_str) = event.payload_json {
        if json_str.len() > PAYLOAD_JSON_MAX_BYTES {
            bail!(
                "payload_json exceeds maximum allowed size of {} bytes (got {}); Phase 2+ writers must redact and trim before insert",
                PAYLOAD_JSON_MAX_BYTES,
                json_str.len()
            );
        }
    }
    // HIGH-001: canonicalize payload_json before storage. Parsing to serde_json::Value and
    // re-serializing collapses duplicate keys (serde_json keeps the last value), so a
    // duplicate-key smuggling attempt like {"redacted_evidence_ref":"sk-secret",
    // "redacted_evidence_ref":"sha256:abc"} cannot persist the credential under the first key
    // while the validator only sees the deduplicated safe value.
    // The canonical string (not the original raw input) is what gets stored and surfaced
    // through GraphQL/MCP readback. validate_json_field is redundant after this but kept as
    // an explicit guard for the Option<None> case.
    let canonical_payload_json: Option<String> = match &event.payload_json {
        None => None,
        Some(json_str) => Some(canonicalize_and_validate_payload_json(json_str)?),
    };

    // Reject missing or unrecognized redaction_version — proposal mandates a known stamp on every event write.
    // Allowlist prevents arbitrary strings from satisfying the not-null contract.
    const KNOWN_REDACTION_VERSIONS: &[&str] = &["redaction_v1"];
    match event.redaction_version.as_deref() {
        None => bail!(
            "escalation_events.redaction_version is required; caller must supply a redaction stamp"
        ),
        Some(v) if !KNOWN_REDACTION_VERSIONS.contains(&v) => bail!(
            "escalation_events.redaction_version '{}' is not in the known allowlist {:?}",
            v,
            KNOWN_REDACTION_VERSIONS
        ),
        _ => {}
    }

    sqlx::query(
        r#"INSERT INTO escalation_events
           (id, escalation_ledger_id, event_kind_raw, tier_id, tier_kind_raw,
            trigger_raw, pause_reason_raw, payload_json, redaction_version, created_at)
           VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)"#,
    )
    .bind(&event.id)
    .bind(&event.escalation_ledger_id)
    .bind(&event.event_kind_raw)
    .bind(&event.tier_id)
    .bind(&event.tier_kind_raw)
    .bind(&event.trigger_raw)
    .bind(&event.pause_reason_raw)
    .bind(&canonical_payload_json) // HIGH-001: bind canonical, not raw original
    .bind(&event.redaction_version)
    .bind(event.created_at.to_rfc3339())
    .execute(&mut **tx)
    .await?;
    crate::metrics::record_escalation_event(
        &event.event_kind_raw,
        event.pause_reason_raw.as_deref(),
        event.tier_kind_raw.as_deref(),
        canonical_payload_json.as_deref(),
    );
    Ok(())
}

pub async fn find_events_by_ledger(
    pool: &SqlitePool,
    escalation_ledger_id: &str,
) -> Result<Vec<EscalationEvent>> {
    // P058-SEC-02: SQL-level row cap (cap+1) prevents unbounded fetch_all.
    // Application-layer cap is 200; fetching 201 lets callers detect truncation (len > 200).
    let rows = sqlx::query(
        r#"SELECT id, escalation_ledger_id, event_kind_raw, tier_id, tier_kind_raw,
                  trigger_raw, pause_reason_raw, payload_json, redaction_version, created_at
           FROM escalation_events
           WHERE escalation_ledger_id = ?
           ORDER BY created_at ASC, id ASC
           LIMIT 201"#,
    )
    .bind(escalation_ledger_id)
    .fetch_all(pool)
    .await?;

    rows.into_iter()
        .map(|row| {
            let created_at_str: String = row.try_get("created_at")?;
            Ok(EscalationEvent {
                id: row.try_get("id")?,
                escalation_ledger_id: row.try_get("escalation_ledger_id")?,
                event_kind_raw: row.try_get("event_kind_raw")?,
                tier_id: row.try_get("tier_id")?,
                tier_kind_raw: row.try_get("tier_kind_raw")?,
                trigger_raw: row.try_get("trigger_raw")?,
                pause_reason_raw: row.try_get("pause_reason_raw")?,
                payload_json: row.try_get("payload_json")?,
                redaction_version: row.try_get("redaction_version")?,
                created_at: created_at_str
                    .parse()
                    .map_err(|e| anyhow!("bad created_at: {e}"))?,
            })
        })
        .collect()
}

/// Validate and persist the shadow escalation columns on agent_execution_runtime_facts.
/// `would_select_decision_json` must be well-formed JSON if present — proposal mandate.
/// Permitted top-level keys for `would_select_decision_json`.
/// The shadow decision JSON must not contain raw evidence, prompts, transcripts,
/// or credential-shaped values (P058-SEC-M1). Only tier-selection metadata is allowed.
const SHADOW_DECISION_JSON_KEYS: &[&str] = &[
    "tier_id",
    "trigger_raw",
    "tier_kind_raw",
    "policy_id",
    "policy_hash",
    "chain_attempt_index",
    "digest_version",
    "redaction_version",
    "decision_reason",
    "timestamp_utc",
];

/// Maximum byte size for would_select_decision_json (P058-SEC-M1).
const SHADOW_DECISION_JSON_MAX_BYTES: usize = 4 * 1024; // 4 KiB

/// Maximum byte size for individual would_select_* column inputs (P058-SEC-M1).
const SHADOW_COLUMN_VALUE_MAX: usize = 256;

/// Known redaction version stamps for shadow decision JSON (P058-SEC-M1).
const SHADOW_KNOWN_REDACTION_VERSIONS: &[&str] = &["redaction_v1"];

/// Returns true when `s` is a safe ISO 8601 timestamp: bounded length, only alphanumeric
/// characters plus '-', '+', ':', '.', 'Z', 'T'. Rejects whitespace, control chars,
/// credential tokens, paths, and prose (P058-SEC-M1).
fn is_safe_timestamp_value(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 64
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | ':' | '.' | 'Z' | 'T'))
}

/// Returns true when `s` contains a known credential-shaped or path prefix that must be
/// rejected from shadow tier metadata fields even when it passes the identifier check.
/// Catches common no-whitespace token shapes not caught by whitespace-only filtering.
/// MEDIUM-001: case-insensitive matching; handles '=' and ':' assignment forms; covers
/// GitHub, Slack, AWS, and common API key prefix families to align with the project-wide
/// credential sanitizer in domain/src/error_sanitizer.rs.
fn has_credential_pattern(s: &str) -> bool {
    // Normalize to lowercase for case-insensitive prefix matching.
    let lower = s.to_ascii_lowercase();

    // Prefixes checked against the lowercase form. Covers both bare and assignment variants
    // (e.g. "authorization=", "authorization:", "bearer ").
    const CREDENTIAL_PREFIXES_LOWER: &[&str] = &[
        // Generic key/secret/token shapes (bare and assignment)
        "sk-",
        "sk_",
        "pk-",
        "pk_",
        "key-",
        "secret-",
        "token-",
        "api_key=",
        "api_key:",
        "apikey=",
        "apikey:",
        "authorization=",
        "authorization:",
        // HTTP auth scheme headers (space-separated value follows)
        "bearer ",
        "basic ",
        // GitHub personal access tokens
        "ghp_",
        "gho_",
        "ghu_",
        "ghs_",
        "ghr_",
        "github_pat_",
        // Slack bot/user/app tokens
        "xoxb-",
        "xoxp-",
        "xoxa-",
        "xoxr-",
        // AWS access key prefixes (originally uppercase; checked via lowercase form)
        "akia",
        "asia",
        // Common env-var assignment forms
        "token=",
        "secret=",
        "password=",
        "passwd=",
        "pwd=",
        "token:",
        "secret:",
        "password:",
        "passwd:",
    ];
    const PATH_PATTERNS: &[&str] = &["/users/", "/home/", "/etc/", "/var/", "c:\\", ":\\", "://"];
    // Reject any identifier that begins with an absolute path separator.
    // This catches /tmp/token, /private/tmp/key, /opt/secret, and similar
    // absolute Unix paths that are not covered by the prefix list above.
    if lower.starts_with('/') {
        return true;
    }
    CREDENTIAL_PREFIXES_LOWER
        .iter()
        .any(|p| lower.starts_with(p))
        || PATH_PATTERNS.iter().any(|p| lower.contains(p))
}

/// Validate per-key constraints for a shadow decision JSON string value.
/// Applies the strictest applicable check for each allowed key (P058-SEC-M1).
fn validate_shadow_json_string_value(key: &str, s: &str) -> Result<()> {
    if s.len() > PAYLOAD_STRING_VALUE_MAX {
        bail!(
            "would_select_decision_json key '{key}' value exceeds maximum \
             {PAYLOAD_STRING_VALUE_MAX} bytes (P058-SEC-M1)"
        );
    }
    match key {
        "policy_hash" => {
            if !is_safe_ref_value(s, APPROVED_HASH_PREFIXES) {
                bail!(
                    "would_select_decision_json policy_hash must be a valid hash identifier \
                     (e.g. sha256:<hex>); raw prose, credentials, and paths are not permitted \
                     (P058-SEC-M1)"
                );
            }
        }
        "redaction_version" => {
            if !SHADOW_KNOWN_REDACTION_VERSIONS.contains(&s) {
                bail!(
                    "would_select_decision_json redaction_version '{}' is not in the known \
                     allowlist {:?} (P058-SEC-M1)",
                    s,
                    SHADOW_KNOWN_REDACTION_VERSIONS
                );
            }
        }
        "timestamp_utc" => {
            if !is_safe_timestamp_value(s) {
                bail!(
                    "would_select_decision_json timestamp_utc must be a bounded ISO 8601 \
                     timestamp (alphanumeric + '-+:. ZT only, max 64 chars); raw prose, \
                     credentials, and paths are not permitted (P058-SEC-M1)"
                );
            }
        }
        // tier_id, trigger_raw, tier_kind_raw, digest_version, decision_reason, policy_id:
        // safe identifier + credential-pattern rejection.
        _ => {
            if !is_safe_identifier_value(s) {
                bail!(
                    "would_select_decision_json '{key}' must be a safe identifier with no \
                     whitespace or control characters; raw messages and credentials are \
                     rejected (P058-SEC-M1)"
                );
            }
            if has_credential_pattern(s) {
                bail!(
                    "would_select_decision_json '{key}' contains a credential-shaped or \
                     path value; only tier-selection identifier values are permitted \
                     (P058-SEC-M1)"
                );
            }
        }
    }
    Ok(())
}

/// Validate and canonicalize would_select_decision_json. Returns the canonical serialized
/// string (duplicate keys de-duplicated) so the caller can bind the canonical form.
/// LOW-001: reject duplicate JSON keys before serde_json::Value parsing (which silently
/// keeps the last value), matching the reject_duplicate_json_keys guard on payload_json.
fn canonicalize_and_validate_shadow_decision_json(json_str: &str) -> Result<String> {
    if json_str.len() > SHADOW_DECISION_JSON_MAX_BYTES {
        bail!(
            "would_select_decision_json exceeds maximum {} bytes (P058-SEC-M1)",
            SHADOW_DECISION_JSON_MAX_BYTES
        );
    }
    // LOW-001: reject duplicate object keys before serde_json canonicalization.
    // Note: reject_duplicate_json_keys also catches malformed JSON during its visitor pass,
    // so the error message covers both cases to satisfy test assertions.
    reject_duplicate_json_keys(json_str).map_err(|e| {
        anyhow!("would_select_decision_json contains malformed JSON or duplicate keys: {e}")
    })?;
    let v: serde_json::Value = serde_json::from_str(json_str)
        .map_err(|e| anyhow!("would_select_decision_json contains malformed JSON: {e}"))?;
    let obj = match &v {
        serde_json::Value::Object(o) => o,
        _ => bail!("would_select_decision_json top level must be a JSON object (P058-SEC-M1)"),
    };
    for key in obj.keys() {
        if !SHADOW_DECISION_JSON_KEYS.contains(&key.as_str()) {
            bail!(
                "would_select_decision_json contains unknown key '{}'; \
                 only tier-selection metadata keys are permitted (P058-SEC-M1)",
                key
            );
        }
    }
    for (key, val) in obj.iter() {
        match key.as_str() {
            "chain_attempt_index" => {
                if !val.is_number() {
                    bail!(
                        "would_select_decision_json chain_attempt_index must be a number, \
                         got {} (P058-SEC-M1)",
                        match val {
                            serde_json::Value::String(_) => "string",
                            serde_json::Value::Bool(_) => "bool",
                            serde_json::Value::Array(_) => "array",
                            serde_json::Value::Object(_) => "object",
                            serde_json::Value::Null => "null",
                            serde_json::Value::Number(_) => unreachable!(),
                        }
                    );
                }
            }
            _ => {
                let s = val.as_str().ok_or_else(|| {
                    anyhow!(
                    "would_select_decision_json key '{key}' must be a string value (P058-SEC-M1)"
                )
                })?;
                validate_shadow_json_string_value(key, s)?;
            }
        }
    }
    // Re-serialize canonical form to collapse duplicate keys (HIGH-001).
    let canonical = serde_json::to_string(&v)
        .map_err(|e| anyhow!("failed to re-serialize would_select_decision_json: {e}"))?;
    Ok(canonical)
}

/// Validate a would_select_* column input (not inside JSON): must be a bounded safe
/// identifier with no credential patterns (P058-SEC-M1).
fn validate_shadow_column_input(field: &str, value: &str) -> Result<()> {
    if value.len() > SHADOW_COLUMN_VALUE_MAX {
        bail!("would_select_{field} exceeds maximum {SHADOW_COLUMN_VALUE_MAX} bytes (P058-SEC-M1)");
    }
    if !is_safe_identifier_value(value) {
        bail!(
            "would_select_{field} must be a safe identifier with no whitespace or control \
             characters; raw messages and credentials are rejected (P058-SEC-M1)"
        );
    }
    if has_credential_pattern(value) {
        bail!(
            "would_select_{field} contains a credential-shaped or path value; only \
             tier-selection identifier values are permitted (P058-SEC-M1)"
        );
    }
    Ok(())
}

pub async fn update_shadow_escalation_columns_tx(
    tx: &mut Transaction<'_, Sqlite>,
    agent_execution_id: &str,
    would_select_tier_id: Option<&str>,
    would_select_trigger_raw: Option<&str>,
    would_select_decision_json: Option<&str>,
) -> Result<()> {
    if let Some(v) = would_select_tier_id {
        validate_shadow_column_input("tier_id", v)?;
    }
    if let Some(v) = would_select_trigger_raw {
        validate_shadow_column_input("trigger_raw", v)?;
    }
    // HIGH-001: canonicalize before storage so duplicate-key smuggling in shadow JSON is
    // collapsed before the canonical form reaches GraphQL/MCP readback.
    let canonical_decision_json: Option<String> = match would_select_decision_json {
        None => None,
        Some(json_str) => Some(canonicalize_and_validate_shadow_decision_json(json_str)?),
    };

    let rows_affected = sqlx::query(
        r#"UPDATE agent_execution_runtime_facts SET
           would_select_tier_id     = ?1,
           would_select_trigger_raw = ?2,
           would_select_decision_json = ?3
           WHERE agent_execution_id = ?4"#,
    )
    .bind(would_select_tier_id)
    .bind(would_select_trigger_raw)
    .bind(canonical_decision_json.as_deref())
    .bind(agent_execution_id)
    .execute(&mut **tx)
    .await?
    .rows_affected();

    if rows_affected == 0 {
        bail!("no agent_execution_runtime_facts row found for agent_execution_id={agent_execution_id}");
    }
    Ok(())
}

/// Typed error returned when policy drift is detected during ledger insertion.
/// Contains the existing ledger ID so callers can open a durable drift pause
/// via `open_drift_pause` after rolling back any in-flight transaction.
#[derive(Debug, Clone)]
pub struct EscalationPolicyDrift {
    pub ledger_id: String,
}

impl std::fmt::Display for EscalationPolicyDrift {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "escalation_policy_drift: ledger_id={}", self.ledger_id)
    }
}

impl std::error::Error for EscalationPolicyDrift {}

/// Open a durable `escalation_policy_drift` pause on an existing escalation ledger row.
/// Called after drift is detected (via `EscalationPolicyDrift` error) to ensure the
/// pause state is committed to the database outside the rolled-back caller transaction.
/// Idempotent: safe to call multiple times on the same ledger_id.
pub async fn open_drift_pause(pool: &SqlitePool, ledger_id: &str) -> Result<()> {
    sqlx::query(
        r#"UPDATE escalation_ledger
           SET status_raw           = 'paused',
               pause_reason_raw     = 'escalation_policy_drift',
               operator_action_hint = 'Acknowledge escalation policy drift through the external MCP/operator workflow or restart with the new policy.',
               runbook_anchor       = 'escalation/policy-drift',
               updated_at           = ?1
           WHERE id = ?2"#,
    )
    .bind(chrono::Utc::now().to_rfc3339())
    .bind(ledger_id)
    .execute(pool)
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn payload_json_shape_rejects_unknown_top_level_keys() {
        // Allowlist schema: any key not in the permitted set is rejected, including
        // lowercase secret names, uppercase variants, alternate spellings, and arbitrary keys.
        let rejected_cases = [
            // Former denylist keys — still rejected
            (r#"{"message": "some text"}"#, "message"),
            (r#"{"output": "text"}"#, "output"),
            (r#"{"transcript": "text"}"#, "transcript"),
            (r#"{"prompt": "text"}"#, "prompt"),
            (r#"{"body": "text"}"#, "body"),
            (r#"{"content": "text"}"#, "content"),
            (r#"{"text": "text"}"#, "text"),
            (r#"{"digest_inputs": {}, "message": "leak"}"#, "message"),
            // Uppercase variants — previously accepted by denylist, now rejected
            (r#"{"Message": "text"}"#, "Message"),
            (r#"{"Output": "text"}"#, "Output"),
            (r#"{"CONTENT": "text"}"#, "CONTENT"),
            // Alternate secret spellings — previously accepted, now rejected
            (r#"{"msg": "text"}"#, "msg"),
            (r#"{"secret": "value"}"#, "secret"),
            (r#"{"api_key": "sk-..."}"#, "api_key"),
            (r#"{"token": "abc"}"#, "token"),
            (r#"{"password": "x"}"#, "password"),
            (r#"{"stack_trace": "x"}"#, "stack_trace"),
            (r#"{"raw": "x"}"#, "raw"),
            (r#"{"err": "x"}"#, "err"),
            (r#"{"reasoning": "x"}"#, "reasoning"),
            (r#"{"headers": {}}"#, "headers"),
            // Arbitrary unknown key
            (r#"{"data": "x"}"#, "data"),
            (r#"{"extra_field": 1}"#, "extra_field"),
        ];
        for (case, key_hint) in &rejected_cases {
            let err = validate_payload_json_shape(case).unwrap_err();
            assert!(
                err.to_string().contains("unknown top-level key")
                    || err.to_string().contains("unknown key"),
                "expected unknown-key rejection for key '{key_hint}' in: {case}, got: {err}"
            );
        }
    }

    #[test]
    fn payload_json_shape_allows_permitted_keys() {
        // All hash values use proper algorithm-length hex strings (SEC-MED-001).
        let sha256_full = "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789"; // 64 hex
        let allowed_cases = [
            format!(r#"{{"digest_inputs": {{"failure_kind": "contract_output_failure"}}, "redacted_evidence_ref": "sha256:{sha256_full}"}}"#),
            r#"{"tier_id": "primary_retry", "tier_kind_raw": "same_backend_retry", "chain_attempt_index": 1}"#.to_string(),
            format!(r#"{{"redacted_evidence_ref": "sha256:{sha256_full}"}}"#),
            r#"{}"#.to_string(),
            r#"{"event_kind_raw": "escalation.tier_selected", "policy_id": "p1"}"#.to_string(),
            format!(r#"{{"digest_inputs": {{"failure_kind": "x", "output_settlement_state": "missing", "validation_evidence_kind": "hash", "redacted_message_fragment_hash": "sha256:{sha256_full}"}}}}"#),
            r#"{"trigger_raw": "stale_no_output", "pause_reason_raw": null, "digest_version": "redaction_v1"}"#.to_string(),
        ];
        for case in &allowed_cases {
            validate_payload_json_shape(case).expect("should accept permitted keys");
        }
    }

    #[test]
    fn payload_json_shape_rejects_malformed_json() {
        let err = validate_payload_json_shape("not json").unwrap_err();
        let msg = err.to_string();
        // SEC-004 duplicate-key scan runs first and produces "payload_json rejected: ..." for
        // malformed input; the downstream parser produces "not valid JSON" if the scan passes.
        assert!(
            msg.contains("not valid JSON") || msg.contains("payload_json rejected"),
            "expected JSON parse rejection; got: {msg}"
        );
    }

    #[test]
    fn payload_json_shape_rejects_non_object_top_level() {
        // Top level must be a JSON object — arrays, strings, numbers, booleans, and null are rejected.
        let non_object_cases = [
            (r#"[1,2,3]"#, "array"),
            (r#"null"#, "null"),
            (r#""some string""#, "string"),
            (r#"42"#, "number"),
            (r#"true"#, "bool"),
        ];
        for (case, kind) in &non_object_cases {
            let err = validate_payload_json_shape(case).unwrap_err();
            assert!(
                err.to_string().contains("must be a JSON object"),
                "expected object-required error for {kind}: {case}, got: {err}"
            );
        }
    }

    #[test]
    fn payload_json_shape_rejects_unknown_digest_input_keys() {
        // digest_inputs sub-keys also follow a strict allowlist.
        let bad_digest_cases = [
            r#"{"digest_inputs": {"message": "raw stack trace"}}"#,
            r#"{"digest_inputs": {"failure_kind": "x", "output": "leak"}}"#,
            r#"{"digest_inputs": {"unknown_key": "x"}}"#,
            r#"{"digest_inputs": {"FAILURE_KIND": "x"}}"#,
        ];
        for case in &bad_digest_cases {
            let err = validate_payload_json_shape(case).unwrap_err();
            assert!(
                err.to_string().contains("unknown key")
                    || err.to_string().contains("unknown top-level key"),
                "expected unknown-key error for digest_inputs case: {case}, got: {err}"
            );
        }
    }

    #[test]
    fn payload_json_shape_rejects_non_object_data_at_top_level() {
        // A key like "data" is not in the allowlist — rejected for the unknown key, not nested content.
        let err =
            validate_payload_json_shape(r#"{"data": [{"content": "raw text"}]}"#).unwrap_err();
        assert!(
            err.to_string().contains("unknown top-level key")
                || err.to_string().contains("unknown key"),
            "got: {err}"
        );
    }

    #[test]
    fn payload_json_shape_rejects_non_object_digest_inputs() {
        // digest_inputs must be an object, not a string or array.
        let bad_cases = [
            r#"{"digest_inputs": "raw string"}"#,
            r#"{"digest_inputs": [1,2]}"#,
            r#"{"digest_inputs": 42}"#,
        ];
        for case in &bad_cases {
            let err = validate_payload_json_shape(case).unwrap_err();
            assert!(
                err.to_string().contains("must be a JSON object"),
                "expected object-required error for digest_inputs: {case}, got: {err}"
            );
        }
    }

    #[test]
    fn payload_json_shape_rejects_wrong_value_types() {
        // Value-type enforcement: each permitted key has a required type.
        let cases = [
            // chain_attempt_index must be a number
            (
                r#"{"chain_attempt_index": "not_a_number"}"#,
                "chain_attempt_index",
            ),
            (r#"{"chain_attempt_index": null}"#, "chain_attempt_index"),
            (
                r#"{"chain_attempt_index": {"nested": "value"}}"#,
                "chain_attempt_index",
            ),
            (r#"{"chain_attempt_index": [1, 2]}"#, "chain_attempt_index"),
            // pause_reason_raw must be string or null — not number, bool, object, or array
            (r#"{"pause_reason_raw": 42}"#, "pause_reason_raw"),
            (r#"{"pause_reason_raw": true}"#, "pause_reason_raw"),
            (r#"{"pause_reason_raw": {}}"#, "pause_reason_raw"),
            (r#"{"pause_reason_raw": [1]}"#, "pause_reason_raw"),
            // String-typed keys must not accept nested objects, arrays, numbers, or booleans
            (r#"{"tier_id": {"nested": "object"}}"#, "tier_id"),
            (r#"{"tier_id": [1, 2, 3]}"#, "tier_id"),
            (r#"{"tier_id": 42}"#, "tier_id"),
            (r#"{"tier_id": true}"#, "tier_id"),
            (r#"{"tier_id": null}"#, "tier_id"),
            (
                r#"{"redacted_evidence_ref": {"raw": "evidence transcript here"}}"#,
                "redacted_evidence_ref",
            ),
            (r#"{"policy_id": null}"#, "policy_id"),
            (r#"{"digest_version": [1, 2]}"#, "digest_version"),
            (r#"{"event_kind_raw": 0}"#, "event_kind_raw"),
            (r#"{"trigger_raw": false}"#, "trigger_raw"),
            // digest_inputs sub-keys must be strings
            (r#"{"digest_inputs": {"failure_kind": 42}}"#, "failure_kind"),
            (
                r#"{"digest_inputs": {"failure_kind": null}}"#,
                "failure_kind",
            ),
            (
                r#"{"digest_inputs": {"failure_kind": {"nested": "object"}}}"#,
                "failure_kind",
            ),
            (
                r#"{"digest_inputs": {"redacted_message_fragment_hash": [1, 2]}}"#,
                "redacted_message_fragment_hash",
            ),
            (
                r#"{"digest_inputs": {"output_settlement_state": true}}"#,
                "output_settlement_state",
            ),
        ];
        for (case, field_hint) in &cases {
            let err = validate_payload_json_shape(case).unwrap_err();
            assert!(
                err.to_string().contains("must be"),
                "expected type error for field '{field_hint}' in: {case}, got: {err}"
            );
        }
    }

    // --- SEC-01 negative tests: redacted_evidence_ref and redacted_message_fragment_hash ---

    #[test]
    fn payload_json_shape_rejects_raw_text_in_redacted_evidence_ref() {
        // Raw transcripts, sentences with spaces, and prompt text must be rejected.
        let cases = [
            r#"{"redacted_evidence_ref": "This is a raw transcript from the agent session."}"#,
            r#"{"redacted_evidence_ref": "Bearer sk-secret-token-value"}"#,
            r#"{"redacted_evidence_ref": "error: connection timed out\nretrying..."}"#,
            r#"{"redacted_evidence_ref": "some text with spaces"}"#,
            r#"{"redacted_evidence_ref": "key=value&other=param"}"#,
        ];
        for case in &cases {
            let err = validate_payload_json_shape(case).unwrap_err();
            assert!(
                err.to_string().contains("hash/ref identifier")
                    || err.to_string().contains("safe identifier")
                    || err.to_string().contains("no whitespace"),
                "expected safe-identifier rejection for redacted_evidence_ref in: {case}, got: {err}"
            );
        }
    }

    #[test]
    fn payload_json_shape_rejects_oversized_redacted_evidence_ref() {
        // Values exceeding PAYLOAD_EVIDENCE_REF_MAX (256 bytes) must be rejected.
        let long_ref = format!(
            r#"{{"redacted_evidence_ref": "sha256:{}" }}"#,
            "a".repeat(300)
        );
        let err = validate_payload_json_shape(&long_ref).unwrap_err();
        assert!(
            err.to_string().contains("exceeds maximum") || err.to_string().contains("256"),
            "expected byte-cap rejection for long redacted_evidence_ref; got: {err}"
        );
    }

    #[test]
    fn payload_json_shape_rejects_raw_text_in_redacted_message_fragment_hash() {
        // Raw evidence, prompts, or whitespace must be rejected in this hash field.
        let cases = [
            r#"{"digest_inputs": {"redacted_message_fragment_hash": "raw transcript content here"}}"#,
            r#"{"digest_inputs": {"redacted_message_fragment_hash": "api key: sk-abc123"}}"#,
            r#"{"digest_inputs": {"redacted_message_fragment_hash": "multi\nline\nvalue"}}"#,
        ];
        for case in &cases {
            let err = validate_payload_json_shape(case).unwrap_err();
            assert!(
                err.to_string().contains("hash identifier")
                    || err.to_string().contains("safe identifier")
                    || err.to_string().contains("no whitespace"),
                "expected identifier-format rejection for redacted_message_fragment_hash in: {case}, got: {err}"
            );
        }
    }

    #[test]
    fn payload_json_shape_rejects_oversized_redacted_message_fragment_hash() {
        // Values exceeding PAYLOAD_FRAGMENT_HASH_MAX (128 bytes) must be rejected.
        let long_hash = format!(
            r#"{{"digest_inputs": {{"redacted_message_fragment_hash": "sha256:{}"}} }}"#,
            "a".repeat(200)
        );
        let err = validate_payload_json_shape(&long_hash).unwrap_err();
        assert!(
            err.to_string().contains("exceeds maximum") || err.to_string().contains("128"),
            "expected byte-cap rejection for long redacted_message_fragment_hash; got: {err}"
        );
    }

    #[test]
    fn payload_json_shape_rejects_oversized_string_value_under_permitted_key() {
        // A value exceeding PAYLOAD_STRING_VALUE_MAX (512 bytes) under any permitted string key
        // must be rejected — even if the key is in the allowlist.
        let long_val = "x".repeat(600);
        let case = format!(r#"{{"tier_id": "{long_val}"}}"#);
        let err = validate_payload_json_shape(&case).unwrap_err();
        assert!(
            err.to_string().contains("exceeds maximum") || err.to_string().contains("512"),
            "expected byte-cap rejection for oversized tier_id; got: {err}"
        );
    }

    #[test]
    fn payload_json_shape_accepts_valid_hash_ref_formats() {
        // SEC-MED-001: valid hash/ref values must use algorithm-appropriate minimum hex lengths.
        // sha256/sha3-256/hmac-sha256: 64 hex, sha3-384: 96 hex, sha3-512: 128 hex, blake2/3: 64 hex.
        let sha256_full = "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789"; // 64 hex
        let sha3_256_full = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"; // 64 hex
        let hmac_sha256_full = "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"; // 64 hex
        let valid_cases = [
            format!(r#"{{"redacted_evidence_ref": "sha256:{sha256_full}"}}"#),
            format!(r#"{{"redacted_evidence_ref": "sha3-256:{sha3_256_full}"}}"#),
            r#"{"redacted_evidence_ref": "ref/artifact-path.1"}"#.to_string(),
            format!(
                r#"{{"digest_inputs": {{"redacted_message_fragment_hash": "sha256:{sha256_full}"}}}}"#
            ),
            format!(
                r#"{{"digest_inputs": {{"redacted_message_fragment_hash": "hmac-sha256:{hmac_sha256_full}"}}}}"#
            ),
        ];
        for case in &valid_cases {
            validate_payload_json_shape(case)
                .unwrap_or_else(|e| panic!("should accept valid hash/ref '{case}', got: {e}"));
        }
    }

    #[test]
    fn payload_json_shape_rejects_short_hex_digests() {
        // SEC-MED-001: short hex values must be rejected even if they start with a valid prefix.
        // Real SHA-256 digests are always 64 hex chars; shorter values cannot be real digests.
        let short_cases = [
            (
                r#"{"redacted_evidence_ref": "sha256:abcdef01"}"#,
                "8-char sha256 hex",
            ),
            (
                r#"{"redacted_evidence_ref": "sha256:abc123def456"}"#,
                "12-char sha256 hex",
            ),
            (
                r#"{"redacted_evidence_ref": "sha256:abcdef0123456789"}"#,
                "16-char sha256 hex",
            ),
            (
                r#"{"redacted_evidence_ref": "sha3-256:abc"}"#,
                "3-char sha3-256 hex",
            ),
            (
                r#"{"digest_inputs": {"redacted_message_fragment_hash": "sha256:abcdef0123"}}"#,
                "10-char sha256 hex in digest_inputs",
            ),
            (
                r#"{"digest_inputs": {"redacted_message_fragment_hash": "hmac-sha256:abc123"}}"#,
                "6-char hmac-sha256 hex in digest_inputs",
            ),
        ];
        for (case, description) in &short_cases {
            let result = validate_payload_json_shape(case);
            assert!(
                result.is_err(),
                "expected rejection of short hex digest ({description}) in: {case}"
            );
        }
    }

    #[test]
    fn payload_json_shape_accepts_null_pause_reason_raw() {
        validate_payload_json_shape(r#"{"pause_reason_raw": null}"#)
            .expect("null is permitted for pause_reason_raw");
        validate_payload_json_shape(r#"{"pause_reason_raw": "escalation_chain_exhausted"}"#)
            .expect("string is permitted for pause_reason_raw");
    }

    #[test]
    fn payload_json_shape_accepts_numeric_chain_attempt_index() {
        validate_payload_json_shape(r#"{"chain_attempt_index": 0}"#)
            .expect("integer 0 is a valid chain_attempt_index");
        validate_payload_json_shape(r#"{"chain_attempt_index": 999}"#)
            .expect("integer 999 is a valid chain_attempt_index");
    }

    // --- SEC-02 negative tests: credential/path shapes rejected by prefix validation ---

    #[test]
    fn payload_json_shape_rejects_credential_and_path_shapes_in_redacted_evidence_ref() {
        // SEC-003: bare API-key patterns, URL schemes, and absolute paths must be rejected
        // because they do not start with an approved hash/ref prefix.
        let cases = [
            (
                r#"{"redacted_evidence_ref": "sk-abc123def456"}"#,
                "bare API key",
            ),
            (
                r#"{"redacted_evidence_ref": "https://example.com/path"}"#,
                "URL scheme",
            ),
            (
                r#"{"redacted_evidence_ref": "/Users/user/file.txt"}"#,
                "absolute path",
            ),
            (
                r#"{"redacted_evidence_ref": "plain-identifier"}"#,
                "plain identifier without prefix",
            ),
            (r#"{"redacted_evidence_ref": "abc123"}"#, "no prefix at all"),
        ];
        for (case, description) in &cases {
            let err = validate_payload_json_shape(case).unwrap_err();
            assert!(
                err.to_string().contains("hash/ref") || err.to_string().contains("approved"),
                "expected prefix rejection for {description} in: {case}, got: {err}"
            );
        }
    }

    #[test]
    fn payload_json_shape_rejects_credential_prefixed_bypass_patterns_in_redacted_evidence_ref() {
        // SEC-003: values that start with an approved prefix but have non-hex suffixes must be
        // rejected. This covers the bypass patterns where a credential, URL, or path is disguised
        // behind a valid prefix (e.g. sha256:sk-..., sha256:https://..., sha256:/Users/...).
        let cases = [
            (
                r#"{"redacted_evidence_ref": "sha256:sk-abc123"}"#,
                "sha256-prefixed API key",
            ),
            (
                r#"{"redacted_evidence_ref": "sha256:https://host/path"}"#,
                "sha256-prefixed URL scheme",
            ),
            (
                r#"{"redacted_evidence_ref": "sha256:/Users/user/file"}"#,
                "sha256-prefixed absolute path",
            ),
            (
                r#"{"redacted_evidence_ref": "sha3-256:sk-secret"}"#,
                "sha3-256-prefixed API key",
            ),
            (
                r#"{"redacted_evidence_ref": "hmac-sha256:Bearer token"}"#,
                "hmac-sha256-prefixed bearer token",
            ),
        ];
        for (case, description) in &cases {
            let err = validate_payload_json_shape(case).unwrap_err();
            assert!(
                err.to_string().contains("hash/ref") || err.to_string().contains("approved"),
                "expected prefix-bypass rejection for {description} in: {case}, got: {err}"
            );
        }
    }

    #[test]
    fn payload_json_shape_rejects_credential_shapes_in_redacted_message_fragment_hash() {
        // SEC-003: non-hash values in redacted_message_fragment_hash must be rejected.
        let cases = [
            (
                r#"{"digest_inputs": {"redacted_message_fragment_hash": "sk-abc123"}}"#,
                "bare API key",
            ),
            (
                r#"{"digest_inputs": {"redacted_message_fragment_hash": "https://x.com"}}"#,
                "URL scheme",
            ),
            (
                r#"{"digest_inputs": {"redacted_message_fragment_hash": "/absolute/path"}}"#,
                "absolute path",
            ),
            (
                r#"{"digest_inputs": {"redacted_message_fragment_hash": "ref/artifact.1"}}"#,
                "ref prefix not approved for hash field",
            ),
            (
                r#"{"digest_inputs": {"redacted_message_fragment_hash": "plain-value"}}"#,
                "plain value",
            ),
        ];
        for (case, description) in &cases {
            let err = validate_payload_json_shape(case).unwrap_err();
            assert!(
                err.to_string().contains("hash identifier") || err.to_string().contains("approved"),
                "expected hash-prefix rejection for {description} in: {case}, got: {err}"
            );
        }
    }

    #[test]
    fn payload_json_shape_rejects_credential_prefixed_bypass_patterns_in_hash_field() {
        // SEC-003: credential-prefixed bypass patterns for redacted_message_fragment_hash.
        // sha256: prefix is approved, but suffix must be pure hex — rejecting sk-..., URLs, paths.
        let cases = [
            (
                r#"{"digest_inputs": {"redacted_message_fragment_hash": "sha256:sk-abc123"}}"#,
                "sha256-prefixed API key",
            ),
            (
                r#"{"digest_inputs": {"redacted_message_fragment_hash": "sha256:https://host/path"}}"#,
                "sha256-prefixed URL",
            ),
            (
                r#"{"digest_inputs": {"redacted_message_fragment_hash": "sha256:/Users/secret"}}"#,
                "sha256-prefixed absolute path",
            ),
        ];
        for (case, description) in &cases {
            let err = validate_payload_json_shape(case).unwrap_err();
            assert!(
                err.to_string().contains("hash identifier") || err.to_string().contains("approved"),
                "expected hash-prefix bypass rejection for {description} in: {case}, got: {err}"
            );
        }
    }

    #[test]
    fn payload_json_shape_rejects_whitespace_in_identifier_fields() {
        // SEC-002: identifier/enum fields must not carry whitespace (prevents raw prose/credentials).
        let cases = [
            (r#"{"tier_id": "tier id with spaces"}"#, "tier_id"),
            (
                r#"{"trigger_raw": "trigger raw with spaces"}"#,
                "trigger_raw",
            ),
            (r#"{"policy_id": "policy id"}"#, "policy_id"),
            (r#"{"event_kind_raw": "event kind raw"}"#, "event_kind_raw"),
            (r#"{"tier_kind_raw": "kind with space"}"#, "tier_kind_raw"),
            (
                r#"{"digest_version": "version with spaces"}"#,
                "digest_version",
            ),
            (
                r#"{"pause_reason_raw": "pause reason with spaces"}"#,
                "pause_reason_raw",
            ),
        ];
        for (case, field_hint) in &cases {
            let err = validate_payload_json_shape(case).unwrap_err();
            assert!(
                err.to_string().contains("identifier") || err.to_string().contains("whitespace"),
                "expected whitespace rejection for field '{field_hint}' in: {case}, got: {err}"
            );
        }
    }

    #[test]
    fn payload_json_shape_rejects_whitespace_in_digest_input_identifier_fields() {
        // SEC-002: digest_inputs identifier fields must not carry whitespace.
        let cases = [
            r#"{"digest_inputs": {"failure_kind": "raw failure description here"}}"#,
            r#"{"digest_inputs": {"output_settlement_state": "state with spaces"}}"#,
            r#"{"digest_inputs": {"validation_evidence_kind": "kind with space"}}"#,
        ];
        for case in &cases {
            let err = validate_payload_json_shape(case).unwrap_err();
            assert!(
                err.to_string().contains("identifier") || err.to_string().contains("whitespace"),
                "expected whitespace rejection for digest_inputs identifier field in: {case}, got: {err}"
            );
        }
    }

    #[test]
    fn payload_json_shape_rejects_credential_shaped_identifier_values() {
        // SEC-P058-003: has_credential_pattern() is applied to the _ => branch that covers
        // tier_id, tier_kind_raw, policy_id, event_kind_raw, trigger_raw, and digest_version.
        // sk-*, token-, key-, secret-, absolute paths, and URL schemes must be rejected even
        // when the value passes the no-whitespace identifier check.
        let cases = [
            (r#"{"tier_id": "sk-abc123def456"}"#, "tier_id sk- API key"),
            (r#"{"tier_id": "token-xyzabcdef"}"#, "tier_id token- prefix"),
            (r#"{"tier_id": "key-some-value"}"#, "tier_id key- prefix"),
            (r#"{"tier_id": "secret-value"}"#, "tier_id secret- prefix"),
            (r#"{"policy_id": "sk-prod-secret"}"#, "policy_id sk- prefix"),
            (
                r#"{"policy_id": "token-abc123"}"#,
                "policy_id token- prefix",
            ),
            (
                r#"{"event_kind_raw": "sk-testkey"}"#,
                "event_kind_raw sk- prefix",
            ),
            (
                r#"{"trigger_raw": "key-trigger"}"#,
                "trigger_raw key- prefix",
            ),
            (
                r#"{"digest_version": "token-v1"}"#,
                "digest_version token- prefix",
            ),
            (
                r#"{"tier_kind_raw": "sk-tier"}"#,
                "tier_kind_raw sk- prefix",
            ),
            // Absolute Unix paths must be rejected from identifier fields.
            (
                r#"{"tier_id": "/Users/user/secret"}"#,
                "tier_id absolute path",
            ),
            (r#"{"policy_id": "/home/user/key"}"#, "policy_id /home path"),
            (r#"{"trigger_raw": "/etc/passwd"}"#, "trigger_raw /etc path"),
            // URL schemes must be rejected from identifier fields.
            (
                r#"{"tier_id": "https://example.com/tier"}"#,
                "tier_id URL scheme",
            ),
            (
                r#"{"policy_id": "http://internal/policy"}"#,
                "policy_id http URL",
            ),
        ];
        for (case, description) in &cases {
            let err = validate_payload_json_shape(case).unwrap_err();
            assert!(
                err.to_string().contains("credential") || err.to_string().contains("path")
                    || err.to_string().contains("identifier") || err.to_string().contains("URL"),
                "SEC-P058-003: expected credential/path rejection for {description}: {case}, got: {err}"
            );
        }
    }

    // SEC-004 + HIGH-001 regression: duplicate-key smuggling must be rejected at the
    // canonicalization stage. Previously we collapsed duplicate keys (keeping last); now we
    // reject them outright so neither the credential-first nor the credential-last ordering
    // can bypass the boundary.
    #[test]
    fn canonicalize_rejects_duplicate_keys_credential_first() {
        // Duplicate key: first value is a bare credential, second is a safe hash.
        // SEC-004 must reject this before reaching the value validation.
        let smuggled = r#"{"redacted_evidence_ref":"sk-secret-token","redacted_evidence_ref":"sha256:abcdef01"}"#;
        let err = canonicalize_and_validate_payload_json(smuggled).unwrap_err();
        assert!(
            err.to_string().contains("duplicate"),
            "expected duplicate-key rejection; got: {err}"
        );
    }

    #[test]
    fn canonicalize_rejects_when_last_duplicate_value_is_unsafe() {
        // If either duplicate value is unsafe, the whole payload is rejected (now at the
        // duplicate-key detection stage rather than the value-validation stage).
        let unsafe_last = r#"{"redacted_evidence_ref":"sha256:abcdef01","redacted_evidence_ref":"sk-unsafe-credential"}"#;
        let err = canonicalize_and_validate_payload_json(unsafe_last).unwrap_err();
        assert!(
            err.to_string().contains("duplicate")
                || err.to_string().contains("hash/ref")
                || err.to_string().contains("approved"),
            "expected rejection when last duplicate value is unsafe; got: {err}"
        );
    }

    // MEDIUM-001 regression: has_credential_pattern must apply to digest_inputs typed fields.
    #[test]
    fn payload_json_shape_rejects_credential_shapes_in_digest_inputs_typed_fields() {
        let cases = [
            (
                r#"{"digest_inputs": {"failure_kind": "sk-api-key"}}"#,
                "failure_kind sk- prefix",
            ),
            (
                r#"{"digest_inputs": {"output_settlement_state": "token-abc123"}}"#,
                "output_settlement_state token- prefix",
            ),
            (
                r#"{"digest_inputs": {"validation_evidence_kind": "/Users/user/secret"}}"#,
                "validation_evidence_kind absolute path",
            ),
            (
                r#"{"digest_inputs": {"failure_kind": "https://leak.example.com"}}"#,
                "failure_kind URL scheme",
            ),
        ];
        for (case, description) in &cases {
            let err = validate_payload_json_shape(case).unwrap_err();
            assert!(
                err.to_string().contains("credential")
                    || err.to_string().contains("path")
                    || err.to_string().contains("MEDIUM-001"),
                "MEDIUM-001: expected credential/path rejection for {description}: {case}, got: {err}"
            );
        }
    }

    // MEDIUM-002 regression: check_identifier_field rejects credential/path/URL shapes.
    #[test]
    fn check_identifier_field_rejects_credential_and_path_shapes() {
        let reject_cases = [
            ("sk-api-key-value", "sk- prefix"),
            ("token-abc123", "token- prefix"),
            ("/Users/user/secret", "absolute path"),
            ("https://example.com/tier", "URL scheme"),
            ("AKIA1234ABCD", "AWS key prefix"),
            ("ghp_abcdef12345", "GitHub PAT prefix"),
        ];
        for (value, description) in &reject_cases {
            let err = check_identifier_field("test_field", value).unwrap_err();
            assert!(
                err.to_string().contains("credential")
                    || err.to_string().contains("path")
                    || err.to_string().contains("MEDIUM-002"),
                "MEDIUM-002: expected rejection for {description}: {value}, got: {err}"
            );
        }
        // Safe identifiers must still be accepted.
        let accept_cases = [
            "same_backend_retry",
            "contract_output_failure",
            "code_writer_default_escalation",
            "escalation.tier_selected",
            "primary_retry",
        ];
        for value in &accept_cases {
            check_identifier_field("test_field", value).unwrap_or_else(|e| {
                panic!("MEDIUM-002: should accept safe identifier '{value}', got: {e}")
            });
        }
    }
}
