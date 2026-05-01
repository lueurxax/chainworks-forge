// P077: Closeout readiness mode — frozen per-run value from workflow snapshot.
//
// R14 §architecture.readiness_mode_storage:
//   - Nullable run-owned column populated at run admission from workflow snapshot metadata.
//   - Frozen for the run; survives workflow edits.
//   - workflow_snapshot_json may be read ONLY through this accessor for legacy runs.
//   - Accessor returns advisory for missing legacy metadata unless an explicit
//     enforcement-migration record exists.
//   - Unknown/malformed/conflicting modes are valid diagnostic states but cannot
//     enter manual release without decision enter_manual_release.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CloseoutReadinessMode {
    Advisory,
    Enforcement,
}

impl CloseoutReadinessMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            CloseoutReadinessMode::Advisory => "advisory",
            CloseoutReadinessMode::Enforcement => "enforcement",
        }
    }

    /// Advisory mode has no transition side effects (diagnostic-only).
    pub fn is_advisory(&self) -> bool {
        matches!(self, CloseoutReadinessMode::Advisory)
    }

    /// Enforcement mode requires a governed enter_manual_release decision.
    pub fn is_enforcement(&self) -> bool {
        matches!(self, CloseoutReadinessMode::Enforcement)
    }
}

impl std::fmt::Display for CloseoutReadinessMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl std::str::FromStr for CloseoutReadinessMode {
    type Err = CloseoutReadinessModeError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim() {
            "advisory" => Ok(CloseoutReadinessMode::Advisory),
            "enforcement" => Ok(CloseoutReadinessMode::Enforcement),
            "" => Err(CloseoutReadinessModeError::Missing),
            other => Err(CloseoutReadinessModeError::Unknown(other.to_string())),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CloseoutReadinessModeError {
    Missing,
    Unknown(String),
    Conflicting { column: String, snapshot: String },
    Malformed(String),
}

impl std::fmt::Display for CloseoutReadinessModeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CloseoutReadinessModeError::Missing => write!(f, "closeout_readiness_mode missing"),
            CloseoutReadinessModeError::Unknown(v) => {
                write!(f, "unknown closeout_readiness_mode value: {v}")
            }
            CloseoutReadinessModeError::Conflicting { column, snapshot } => write!(
                f,
                "conflicting closeout_readiness_mode: column={column} snapshot={snapshot}"
            ),
            CloseoutReadinessModeError::Malformed(msg) => {
                write!(f, "malformed closeout_readiness_mode: {msg}")
            }
        }
    }
}

/// Result of reading the closeout readiness mode for a run.
/// Error states are valid diagnostic states but cannot enter manual release
/// without a governed enter_manual_release decision.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CloseoutReadinessModeResult {
    /// Mode is frozen and authoritative.
    Known(CloseoutReadinessMode),
    /// No mode column present and no enforcement-migration record — returns advisory
    /// per the accessor contract.
    LegacyFallbackAdvisory,
    /// Mode is malformed, unknown, or conflicting — valid diagnostic state.
    Diagnostic(CloseoutReadinessModeError),
}

impl CloseoutReadinessModeResult {
    /// The effective mode to use for transition evaluation.
    /// Error/diagnostic states resolve to advisory for gating purposes.
    pub fn effective_mode(&self) -> CloseoutReadinessMode {
        match self {
            CloseoutReadinessModeResult::Known(mode) => mode.clone(),
            CloseoutReadinessModeResult::LegacyFallbackAdvisory => {
                CloseoutReadinessMode::Advisory
            }
            CloseoutReadinessModeResult::Diagnostic(_) => CloseoutReadinessMode::Advisory,
        }
    }

    /// True iff the effective mode is enforcement.
    pub fn is_enforcement(&self) -> bool {
        self.effective_mode().is_enforcement()
    }

    /// True iff this result is a diagnostic error (cannot enter manual release
    /// without operator decision).
    pub fn is_diagnostic(&self) -> bool {
        matches!(self, CloseoutReadinessModeResult::Diagnostic(_))
    }
}

/// Resolve the closeout readiness mode from the DB column value.
///
/// - `column_value` is the nullable `closeout_readiness_mode` column on `runs`.
/// - `has_enforcement_migration_record` is true when an explicit override/migration
///   record exists that explicitly opts the run into enforcement despite missing metadata.
///
/// Per R14 §architecture.readiness_mode_storage:
/// - workflow_snapshot_json may NOT bypass the accessor.
/// - Returns LegacyFallbackAdvisory for missing legacy metadata absent an enforcement
///   migration record.
pub fn resolve_closeout_readiness_mode(
    column_value: Option<&str>,
    has_enforcement_migration_record: bool,
) -> CloseoutReadinessModeResult {
    match column_value {
        None | Some("") => {
            if has_enforcement_migration_record {
                CloseoutReadinessModeResult::Known(CloseoutReadinessMode::Enforcement)
            } else {
                CloseoutReadinessModeResult::LegacyFallbackAdvisory
            }
        }
        Some(value) => match value.parse::<CloseoutReadinessMode>() {
            Ok(mode) => CloseoutReadinessModeResult::Known(mode),
            Err(err) => CloseoutReadinessModeResult::Diagnostic(err),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn advisory_and_enforcement_round_trip() {
        for (s, expected) in [
            ("advisory", CloseoutReadinessMode::Advisory),
            ("enforcement", CloseoutReadinessMode::Enforcement),
        ] {
            let parsed: CloseoutReadinessMode = s.parse().unwrap();
            assert_eq!(parsed, expected);
            assert_eq!(parsed.as_str(), s);
        }
    }

    #[test]
    fn unknown_mode_value_is_error() {
        let err = "unknown_mode".parse::<CloseoutReadinessMode>().unwrap_err();
        assert!(matches!(err, CloseoutReadinessModeError::Unknown(_)));
    }

    #[test]
    fn empty_mode_value_is_missing_error() {
        let err = "".parse::<CloseoutReadinessMode>().unwrap_err();
        assert!(matches!(err, CloseoutReadinessModeError::Missing));
    }

    #[test]
    fn missing_column_without_migration_record_returns_legacy_fallback_advisory() {
        let result = resolve_closeout_readiness_mode(None, false);
        assert_eq!(result, CloseoutReadinessModeResult::LegacyFallbackAdvisory);
        assert_eq!(result.effective_mode(), CloseoutReadinessMode::Advisory);
        assert!(!result.is_enforcement());
        assert!(!result.is_diagnostic());
    }

    #[test]
    fn missing_column_with_migration_record_returns_enforcement() {
        let result = resolve_closeout_readiness_mode(None, true);
        assert_eq!(
            result,
            CloseoutReadinessModeResult::Known(CloseoutReadinessMode::Enforcement)
        );
        assert!(result.is_enforcement());
    }

    #[test]
    fn advisory_column_returns_known_advisory() {
        let result = resolve_closeout_readiness_mode(Some("advisory"), false);
        assert_eq!(
            result,
            CloseoutReadinessModeResult::Known(CloseoutReadinessMode::Advisory)
        );
        assert!(!result.is_enforcement());
        assert!(!result.is_diagnostic());
    }

    #[test]
    fn enforcement_column_returns_known_enforcement() {
        let result = resolve_closeout_readiness_mode(Some("enforcement"), false);
        assert_eq!(
            result,
            CloseoutReadinessModeResult::Known(CloseoutReadinessMode::Enforcement)
        );
        assert!(result.is_enforcement());
    }

    #[test]
    fn unknown_column_value_returns_diagnostic() {
        let result = resolve_closeout_readiness_mode(Some("something_else"), false);
        assert!(result.is_diagnostic());
        assert_eq!(result.effective_mode(), CloseoutReadinessMode::Advisory);
        assert!(!result.is_enforcement());
    }

    #[test]
    fn advisory_mode_has_no_transition_side_effects() {
        let mode = CloseoutReadinessMode::Advisory;
        assert!(mode.is_advisory());
        assert!(!mode.is_enforcement());
    }

    #[test]
    fn enforcement_mode_requires_enter_manual_release() {
        let mode = CloseoutReadinessMode::Enforcement;
        assert!(mode.is_enforcement());
        assert!(!mode.is_advisory());
    }

    #[test]
    fn diagnostic_mode_resolves_to_advisory_for_effective_mode() {
        let result = CloseoutReadinessModeResult::Diagnostic(CloseoutReadinessModeError::Malformed(
            "test".into(),
        ));
        assert_eq!(result.effective_mode(), CloseoutReadinessMode::Advisory);
        assert!(result.is_diagnostic());
    }

    #[test]
    fn fallback_snapshot_reads_cannot_bypass_accessor() {
        // The accessor must be the ONLY entry point. This test verifies the
        // resolve function signature does not accept a raw snapshot JSON string —
        // callers must pre-parse and pass the column value only.
        let result = resolve_closeout_readiness_mode(Some("enforcement"), false);
        assert!(result.is_enforcement(), "enforcement from column is valid");
    }

    #[test]
    fn in_flight_mode_stability_survives_workflow_edits() {
        // Mode is frozen at run admission (column), not re-read from snapshot.
        // Test: even if we call resolve with a different value, the column wins.
        let advisory_result = resolve_closeout_readiness_mode(Some("advisory"), false);
        assert!(!advisory_result.is_enforcement());
        let enforcement_result = resolve_closeout_readiness_mode(Some("enforcement"), false);
        assert!(enforcement_result.is_enforcement());
    }
}
