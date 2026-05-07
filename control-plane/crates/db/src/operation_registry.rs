//! P075 write operation registry loader and validator.
//!
//! Parses `write-operation-registry.toml` for the proposal-075 gate. Every
//! `WriteOperation.operation_name` submitted to DbWriter must have an entry here.
//! The Phase 7 gate (fail-closed) fails on observed names not in this registry
//! (LIFT-REL-07).

use serde::Deserialize;

/// A single operation registry entry.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct OperationEntry {
    /// Stable name matching `WriteOperation.operation_name`.
    pub operation_name: String,
    /// Write class: "A", "B", "C", or "D".
    pub class: String,
    /// Replay policy: "natural_key", "last_writer_wins", "checksum_idempotent",
    /// "caller_guarded", or "telemetry_merge".
    pub replay_policy: String,
    /// Description of the idempotency key format.
    pub idempotency_key_kind: String,
    /// Test path proving no double-application. Required for `caller_guarded` entries.
    pub duplicate_application_test_path: String,
}

impl OperationEntry {
    /// Returns true if all required fields are non-empty.
    pub fn is_complete(&self) -> bool {
        !self.operation_name.is_empty()
            && !self.class.is_empty()
            && !self.replay_policy.is_empty()
            && !self.idempotency_key_kind.is_empty()
    }

    /// Returns true if this entry uses a valid class token.
    pub fn has_valid_class(&self) -> bool {
        matches!(self.class.as_str(), "A" | "B" | "C" | "D")
    }

    /// Returns true if this entry uses a valid replay policy token.
    pub fn has_valid_replay_policy(&self) -> bool {
        matches!(
            self.replay_policy.as_str(),
            "natural_key"
                | "last_writer_wins"
                | "checksum_idempotent"
                | "caller_guarded"
                | "telemetry_merge"
        )
    }

    /// Returns true if caller_guarded entries have a non-empty test path.
    pub fn has_caller_guarded_test(&self) -> bool {
        if self.replay_policy == "caller_guarded" {
            !self.duplicate_application_test_path.is_empty()
        } else {
            true
        }
    }
}

#[derive(Debug, Deserialize)]
struct RegistryFile {
    #[serde(default)]
    operations: Vec<OperationEntry>,
}

/// Parsed operation registry.
#[derive(Debug, Clone, Default)]
pub struct OperationRegistry {
    pub entries: Vec<OperationEntry>,
}

impl OperationRegistry {
    /// Parse the registry from a TOML string.
    ///
    /// Returns an error if TOML is malformed, any entry is incomplete, has an invalid
    /// class/replay_policy, or is `caller_guarded` without a test path.
    pub fn parse(toml_str: &str) -> Result<Self, OperationRegistryError> {
        let file: RegistryFile =
            toml::from_str(toml_str).map_err(OperationRegistryError::ParseError)?;

        for entry in &file.operations {
            if !entry.is_complete() {
                return Err(OperationRegistryError::IncompleteEntry(
                    entry.operation_name.clone(),
                ));
            }
            if !entry.has_valid_class() {
                return Err(OperationRegistryError::InvalidClass {
                    operation_name: entry.operation_name.clone(),
                    class: entry.class.clone(),
                });
            }
            if !entry.has_valid_replay_policy() {
                return Err(OperationRegistryError::InvalidReplayPolicy {
                    operation_name: entry.operation_name.clone(),
                    replay_policy: entry.replay_policy.clone(),
                });
            }
            if !entry.has_caller_guarded_test() {
                return Err(OperationRegistryError::MissingCallerGuardedTest(
                    entry.operation_name.clone(),
                ));
            }
        }

        Ok(Self {
            entries: file.operations,
        })
    }

    /// Load and parse from a file path.
    pub fn load_file(path: &std::path::Path) -> Result<Self, OperationRegistryError> {
        let content = std::fs::read_to_string(path).map_err(OperationRegistryError::Io)?;
        Self::parse(&content)
    }

    /// Find an entry by operation name.
    pub fn find(&self, operation_name: &str) -> Option<&OperationEntry> {
        self.entries
            .iter()
            .find(|e| e.operation_name == operation_name)
    }

    /// Returns true if the given operation name is registered.
    pub fn contains(&self, operation_name: &str) -> bool {
        self.find(operation_name).is_some()
    }

    /// Returns operation names not found in the registry from a given list.
    /// Used by the Phase 7 gate to detect unregistered operation names.
    pub fn unregistered<'a>(&self, observed: &[&'a str]) -> Vec<&'a str> {
        observed
            .iter()
            .copied()
            .filter(|name| !self.contains(name))
            .collect()
    }
}

/// Errors from registry parsing.
#[derive(Debug, thiserror::Error)]
pub enum OperationRegistryError {
    #[error("TOML parse error: {0}")]
    ParseError(#[from] toml::de::Error),
    #[error("Operation '{0}' is missing required fields")]
    IncompleteEntry(String),
    #[error("Operation '{operation_name}' has invalid class '{class}'; must be A, B, C, or D")]
    InvalidClass {
        operation_name: String,
        class: String,
    },
    #[error("Operation '{operation_name}' has invalid replay_policy '{replay_policy}'")]
    InvalidReplayPolicy {
        operation_name: String,
        replay_policy: String,
    },
    #[error("Operation '{0}' is caller_guarded but missing duplicate_application_test_path")]
    MissingCallerGuardedTest(String),
    #[error("I/O error reading registry: {0}")]
    Io(#[from] std::io::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID_TOML: &str = r#"
[[operations]]
operation_name = "canonical_stage_transition"
class = "A"
replay_policy = "natural_key"
idempotency_key_kind = "stage_execution_id"
duplicate_application_test_path = ""

[[operations]]
operation_name = "projection_invalidation_coalesced"
class = "B"
replay_policy = "last_writer_wins"
idempotency_key_kind = "(run_id, surface, projection_kind)"
duplicate_application_test_path = ""

[[operations]]
operation_name = "transcript_spool_metadata"
class = "C"
replay_policy = "checksum_idempotent"
idempotency_key_kind = "(run_id, relative_path)"
duplicate_application_test_path = ""

[[operations]]
operation_name = "write_lock_wait_rollup"
class = "D"
replay_policy = "telemetry_merge"
idempotency_key_kind = "window bucket key"
duplicate_application_test_path = ""
"#;

    #[test]
    fn parse_valid_registry() {
        let reg = OperationRegistry::parse(VALID_TOML).unwrap();
        assert_eq!(reg.entries.len(), 4);
    }

    #[test]
    fn find_by_operation_name() {
        let reg = OperationRegistry::parse(VALID_TOML).unwrap();
        assert!(reg.find("canonical_stage_transition").is_some());
        assert!(reg.find("nonexistent").is_none());
    }

    #[test]
    fn contains_returns_true_for_registered() {
        let reg = OperationRegistry::parse(VALID_TOML).unwrap();
        assert!(reg.contains("canonical_stage_transition"));
        assert!(!reg.contains("unregistered_op"));
    }

    #[test]
    fn unregistered_returns_unknown_names() {
        let reg = OperationRegistry::parse(VALID_TOML).unwrap();
        let observed = [
            "canonical_stage_transition",
            "unknown_op",
            "another_unknown",
        ];
        let unknown = reg.unregistered(&observed);
        assert_eq!(unknown.len(), 2);
        assert!(unknown.contains(&"unknown_op"));
        assert!(unknown.contains(&"another_unknown"));
    }

    #[test]
    fn parse_rejects_invalid_class() {
        let bad = r#"
[[operations]]
operation_name = "bad_class_op"
class = "X"
replay_policy = "natural_key"
idempotency_key_kind = "some key"
duplicate_application_test_path = ""
"#;
        let result = OperationRegistry::parse(bad);
        assert!(matches!(
            result,
            Err(OperationRegistryError::InvalidClass { .. })
        ));
    }

    #[test]
    fn parse_rejects_invalid_replay_policy() {
        let bad = r#"
[[operations]]
operation_name = "bad_policy_op"
class = "A"
replay_policy = "bogus_policy"
idempotency_key_kind = "some key"
duplicate_application_test_path = ""
"#;
        let result = OperationRegistry::parse(bad);
        assert!(matches!(
            result,
            Err(OperationRegistryError::InvalidReplayPolicy { .. })
        ));
    }

    #[test]
    fn parse_rejects_caller_guarded_without_test_path() {
        let bad = r#"
[[operations]]
operation_name = "caller_guarded_op"
class = "A"
replay_policy = "caller_guarded"
idempotency_key_kind = "some key"
duplicate_application_test_path = ""
"#;
        let result = OperationRegistry::parse(bad);
        assert!(matches!(
            result,
            Err(OperationRegistryError::MissingCallerGuardedTest(_))
        ));
    }

    #[test]
    fn parse_accepts_caller_guarded_with_test_path() {
        let good = r#"
[[operations]]
operation_name = "caller_guarded_with_test"
class = "A"
replay_policy = "caller_guarded"
idempotency_key_kind = "some key"
duplicate_application_test_path = "crates/engine/tests/no_double_apply.rs"
"#;
        let reg = OperationRegistry::parse(good).unwrap();
        assert!(reg.contains("caller_guarded_with_test"));
    }

    #[test]
    fn parse_empty_registry() {
        let reg = OperationRegistry::parse("").unwrap();
        assert!(reg.entries.is_empty());
    }

    #[test]
    fn canonical_registry_file_parses() {
        let path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("write-operation-registry.toml");
        if path.exists() {
            let reg = OperationRegistry::load_file(&path).expect("canonical registry should parse");
            // All entries in the canonical file must be valid.
            for entry in &reg.entries {
                assert!(
                    entry.is_complete(),
                    "entry {:?} is incomplete",
                    entry.operation_name
                );
                assert!(
                    entry.has_valid_class(),
                    "entry {:?} has invalid class",
                    entry.operation_name
                );
                assert!(
                    entry.has_valid_replay_policy(),
                    "entry {:?} has invalid replay_policy",
                    entry.operation_name
                );
                assert!(
                    entry.has_caller_guarded_test(),
                    "entry {:?} is caller_guarded without test path",
                    entry.operation_name
                );
            }
        }
    }
}
