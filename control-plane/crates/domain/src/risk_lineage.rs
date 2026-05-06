// P077: Typed risk lineage model.
//
// R14 §architecture.risk_lineage:
//   Accepted sources: typed controlled risk rows, release-owner decision record,
//   governed waiver or settlement command.
//   Required fields: risk_id, title, classification, authority,
//   journal_or_decision_id, source_generation_ids, settled_at.
//   Free-form known_risks text NEVER satisfies enter_manual_release.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RiskClassification {
    Low,
    Medium,
    High,
    Critical,
}

impl RiskClassification {
    pub fn as_str(&self) -> &'static str {
        match self {
            RiskClassification::Low => "low",
            RiskClassification::Medium => "medium",
            RiskClassification::High => "high",
            RiskClassification::Critical => "critical",
        }
    }
}

impl std::fmt::Display for RiskClassification {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl std::str::FromStr for RiskClassification {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "low" => Ok(RiskClassification::Low),
            "medium" => Ok(RiskClassification::Medium),
            "high" => Ok(RiskClassification::High),
            "critical" => Ok(RiskClassification::Critical),
            other => Err(format!("unknown RiskClassification: {other}")),
        }
    }
}

/// Source of a risk acceptance — only governed sources satisfy enter_manual_release.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RiskAcceptanceSource {
    /// Typed controlled risk row from the canonical risk registry.
    TypedControlledRiskRow,
    /// Release-owner decision record with full lineage.
    ReleaseOwnerDecision,
    /// Governed waiver or settlement command through the command journal.
    GovernedWaiverOrSettlement,
}

/// Fully typed risk acceptance lineage record.
/// All required fields must be present; free-form text is insufficient.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RiskAcceptanceLineage {
    pub risk_id: String,
    pub title: String,
    pub classification: RiskClassification,
    pub authority: String,
    pub journal_or_decision_id: String,
    pub source_generation_ids: Vec<String>,
    pub settled_at: DateTime<Utc>,
    pub acceptance_source: RiskAcceptanceSource,
    pub rationale: Option<String>,
}

impl RiskAcceptanceLineage {
    /// Validate that all required fields are present and non-empty.
    /// Free-form text alone (e.g. from known_risks[]) is never sufficient.
    pub fn validate(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();
        if self.risk_id.trim().is_empty() {
            errors.push("risk_id is required and must not be empty".into());
        }
        if self.title.trim().is_empty() {
            errors.push("title is required and must not be empty".into());
        }
        if self.authority.trim().is_empty() {
            errors.push("authority is required and must not be empty".into());
        }
        if self.journal_or_decision_id.trim().is_empty() {
            errors.push("journal_or_decision_id is required and must not be empty".into());
        }
        if self.source_generation_ids.is_empty() {
            errors.push("source_generation_ids must contain at least one entry".into());
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

/// Check whether a set of risk lineage records satisfies the enter_manual_release
/// criterion. Free-form known_risks strings never satisfy this.
///
/// Per R14: "Free-form known_risks text never satisfies enter_manual_release."
pub fn risks_satisfy_enter_manual_release(lineage: &[RiskAcceptanceLineage]) -> bool {
    lineage.iter().all(|l| l.validate().is_ok())
}

/// Reject free-form risk text as an acceptance source.
/// Returns an error explaining that free-form text is not a valid settlement.
pub fn reject_freeform_risk_text(text: &str) -> Result<(), String> {
    if text.trim().is_empty() {
        return Ok(());
    }
    Err(format!(
        "free-form risk text '{text}' does not satisfy enter_manual_release; \
         use a typed RiskAcceptanceLineage with governed authority"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_lineage() -> RiskAcceptanceLineage {
        RiskAcceptanceLineage {
            risk_id: "RISK-001".into(),
            title: "Parallel release policy".into(),
            classification: RiskClassification::Medium,
            authority: "release-owner-alice".into(),
            journal_or_decision_id: "journal-abc-123".into(),
            source_generation_ids: vec!["gen-1".into()],
            settled_at: Utc::now(),
            acceptance_source: RiskAcceptanceSource::TypedControlledRiskRow,
            rationale: Some("Accepted with mitigation plan".into()),
        }
    }

    #[test]
    fn valid_lineage_validates_ok() {
        assert!(valid_lineage().validate().is_ok());
    }

    #[test]
    fn missing_risk_id_fails_validation() {
        let mut l = valid_lineage();
        l.risk_id = "".into();
        assert!(l.validate().is_err());
    }

    #[test]
    fn missing_authority_fails_validation() {
        let mut l = valid_lineage();
        l.authority = "".into();
        assert!(l.validate().is_err());
    }

    #[test]
    fn empty_source_generation_ids_fails_validation() {
        let mut l = valid_lineage();
        l.source_generation_ids.clear();
        assert!(l.validate().is_err());
    }

    #[test]
    fn risks_satisfy_enter_manual_release_requires_all_valid() {
        let valid = valid_lineage();
        let mut invalid = valid_lineage();
        invalid.risk_id = "".into();
        assert!(risks_satisfy_enter_manual_release(&[valid.clone()]));
        assert!(!risks_satisfy_enter_manual_release(&[valid, invalid]));
    }

    #[test]
    fn freeform_risk_text_is_rejected() {
        assert!(reject_freeform_risk_text("known risk: advisory mode confusion").is_err());
    }

    #[test]
    fn empty_freeform_text_is_ok_since_theres_nothing_to_reject() {
        assert!(reject_freeform_risk_text("").is_ok());
        assert!(reject_freeform_risk_text("   ").is_ok());
    }

    #[test]
    fn risk_classification_round_trips() {
        for (s, expected) in [
            ("low", RiskClassification::Low),
            ("medium", RiskClassification::Medium),
            ("high", RiskClassification::High),
            ("critical", RiskClassification::Critical),
        ] {
            let parsed: RiskClassification = s.parse().unwrap();
            assert_eq!(parsed, expected);
            assert_eq!(parsed.as_str(), s);
        }
    }
}
