//! P075 bypass allowlist loader and validator.
//!
//! Parses `write-bypass-allowlist.toml` for the proposal-075 gate. In Phase 1
//! (inventory mode) the gate reports unlisted owners and expired entries. In Phase 7
//! (fail-closed) the gate fails on any violation.

use serde::Deserialize;

/// A single bypass allowlist entry.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct BypassEntry {
    /// Stable identifier, never reused.
    pub id: String,
    /// Qualified path of the code owner (crate::module::fn or glob pattern).
    pub owner: String,
    /// Short rationale for the bypass.
    pub reason: String,
    /// Bypass category: "migrations", "tests", "startup_repair", or "temporary_rollout".
    pub scope: String,
    /// Glob-style path fragment matching bypass call sites.
    pub path_pattern: String,
    /// Condition under which this bypass is permitted.
    pub allowed_context: String,
    /// Observable condition that proves the bypass can be removed.
    pub retirement_criteria: String,
    /// Phase number (1–8). The gate fails when the current phase exceeds this value
    /// for temporary_rollout scopes.
    pub expires_after_phase: u32,
}

impl BypassEntry {
    /// Returns true if this entry is expired for the given current phase.
    /// Permanent bypass categories (migrations, tests, startup_repair) never expire.
    pub fn is_expired_at_phase(&self, current_phase: u32) -> bool {
        match self.scope.as_str() {
            "migrations" | "tests" | "startup_repair" => false,
            _ => current_phase > self.expires_after_phase,
        }
    }

    /// Returns true if all required fields are non-empty.
    pub fn is_complete(&self) -> bool {
        !self.id.is_empty()
            && !self.owner.is_empty()
            && !self.reason.is_empty()
            && !self.scope.is_empty()
            && !self.path_pattern.is_empty()
            && !self.allowed_context.is_empty()
            && !self.retirement_criteria.is_empty()
    }
}

#[derive(Debug, Deserialize)]
struct AllowlistFile {
    #[serde(default)]
    bypasses: Vec<BypassEntry>,
}

/// Parsed allowlist.
#[derive(Debug, Clone, Default)]
pub struct BypassAllowlist {
    pub entries: Vec<BypassEntry>,
}

impl BypassAllowlist {
    /// Parse the allowlist from a TOML string.
    ///
    /// Returns an error if the TOML is malformed or any entry is missing required fields.
    pub fn parse(toml_str: &str) -> Result<Self, BypassAllowlistError> {
        let file: AllowlistFile =
            toml::from_str(toml_str).map_err(BypassAllowlistError::ParseError)?;

        for entry in &file.bypasses {
            if !entry.is_complete() {
                return Err(BypassAllowlistError::IncompleteEntry(entry.id.clone()));
            }
        }

        Ok(Self { entries: file.bypasses })
    }

    /// Load and parse from a file path.
    pub fn load_file(path: &std::path::Path) -> Result<Self, BypassAllowlistError> {
        let content =
            std::fs::read_to_string(path).map_err(BypassAllowlistError::Io)?;
        Self::parse(&content)
    }

    /// Find an entry by id.
    pub fn find_by_id(&self, id: &str) -> Option<&BypassEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    /// Returns entries expired at the given phase (temporary_rollout scopes only).
    pub fn expired_at_phase(&self, current_phase: u32) -> Vec<&BypassEntry> {
        self.entries
            .iter()
            .filter(|e| e.is_expired_at_phase(current_phase))
            .collect()
    }

    /// Returns entries with incomplete required fields.
    pub fn incomplete_entries(&self) -> Vec<&BypassEntry> {
        self.entries.iter().filter(|e| !e.is_complete()).collect()
    }
}

/// Errors from allowlist parsing.
#[derive(Debug, thiserror::Error)]
pub enum BypassAllowlistError {
    #[error("TOML parse error: {0}")]
    ParseError(#[from] toml::de::Error),
    #[error("Bypass entry '{0}' is missing required fields")]
    IncompleteEntry(String),
    #[error("I/O error reading allowlist: {0}")]
    Io(#[from] std::io::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID_TOML: &str = r#"
[[bypasses]]
id = "bypass-001"
owner = "db::migrate"
reason = "Migrations are infrastructure."
scope = "migrations"
path_pattern = "crates/db/src/migrate.rs"
allowed_context = "Schema migration only."
retirement_criteria = "Never retire."
expires_after_phase = 8

[[bypasses]]
id = "bypass-002"
owner = "engine::recovery"
reason = "Temporary rollout path."
scope = "temporary_rollout"
path_pattern = "crates/engine/src/recovery.rs"
allowed_context = "Pre-DbWriter routing."
retirement_criteria = "Route through DbWriter."
expires_after_phase = 3
"#;

    #[test]
    fn parse_valid_allowlist() {
        let list = BypassAllowlist::parse(VALID_TOML).unwrap();
        assert_eq!(list.entries.len(), 2);
        assert_eq!(list.entries[0].id, "bypass-001");
        assert_eq!(list.entries[1].id, "bypass-002");
    }

    #[test]
    fn find_by_id() {
        let list = BypassAllowlist::parse(VALID_TOML).unwrap();
        assert!(list.find_by_id("bypass-001").is_some());
        assert!(list.find_by_id("nonexistent").is_none());
    }

    #[test]
    fn migrations_scope_never_expires() {
        let list = BypassAllowlist::parse(VALID_TOML).unwrap();
        let migrations = list.find_by_id("bypass-001").unwrap();
        assert!(!migrations.is_expired_at_phase(8));
        assert!(!migrations.is_expired_at_phase(100));
    }

    #[test]
    fn temporary_rollout_expires_after_phase() {
        let list = BypassAllowlist::parse(VALID_TOML).unwrap();
        let rollout = list.find_by_id("bypass-002").unwrap();
        assert!(!rollout.is_expired_at_phase(3)); // at phase 3: not yet expired
        assert!(rollout.is_expired_at_phase(4));  // phase 4 > expires_after_phase=3
    }

    #[test]
    fn expired_at_phase_filters_correctly() {
        let list = BypassAllowlist::parse(VALID_TOML).unwrap();
        let expired = list.expired_at_phase(4);
        assert_eq!(expired.len(), 1);
        assert_eq!(expired[0].id, "bypass-002");
    }

    #[test]
    fn no_entries_expired_at_phase_1() {
        let list = BypassAllowlist::parse(VALID_TOML).unwrap();
        assert!(list.expired_at_phase(1).is_empty());
    }

    #[test]
    fn parse_rejects_missing_required_field() {
        let bad = r#"
[[bypasses]]
id = "bypass-bad"
owner = "some::module"
reason = "reason"
scope = "migrations"
path_pattern = ""
allowed_context = "ctx"
retirement_criteria = "retire"
expires_after_phase = 8
"#;
        // path_pattern is empty, which is_complete treats as incomplete.
        let result = BypassAllowlist::parse(bad);
        assert!(result.is_err());
        if let Err(BypassAllowlistError::IncompleteEntry(id)) = result {
            assert_eq!(id, "bypass-bad");
        } else {
            panic!("expected IncompleteEntry error");
        }
    }

    #[test]
    fn parse_empty_allowlist() {
        let list = BypassAllowlist::parse("").unwrap();
        assert!(list.entries.is_empty());
    }

    #[test]
    fn canonical_allowlist_file_parses() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("write-bypass-allowlist.toml");
        if path.exists() {
            let list = BypassAllowlist::load_file(&path).expect("canonical allowlist should parse");
            assert!(!list.entries.is_empty(), "canonical allowlist should have at least one entry");
            // No entry should be incomplete.
            assert!(
                list.incomplete_entries().is_empty(),
                "canonical allowlist has incomplete entries: {:?}",
                list.incomplete_entries()
            );
        }
    }
}
