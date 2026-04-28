pub mod agent;
pub mod approval;
pub mod artifact;
pub mod artifact_contracts;
pub mod capabilities;
pub mod commands;
pub mod discovery;
pub mod events;
pub mod idea;
pub mod ids;
pub mod lifecycle;
pub mod main_sync;
pub mod mediation;
pub mod provider;
pub mod retry_instruction;
pub mod run;
pub mod session;
pub mod stage;
pub mod steward;
pub mod validation;
pub mod workflow_conflict;
pub mod xcode_runtime;

// P029: PrincipalClass is canonically defined in domain::commands.
pub use capabilities::{CapabilityToolId, ResourceTemplateId};
pub use commands::PrincipalClass;

// P042: daemon lifecycle types are the canonical readback contract.
pub use lifecycle::{
    DaemonLifecycleState, DaemonStatus, DegradedKind, DegradedReason, FailureKind, FailureReason,
    XcodeBrokerHealthSnapshot, XcodeBrokerHealthState,
};

#[cfg(test)]
mod tests {
    use super::run::RunStatus;
    use super::stage::StageStatus;

    #[test]
    fn run_status_terminal() {
        assert!(RunStatus::Completed.is_terminal());
        assert!(RunStatus::Failed.is_terminal());
        assert!(RunStatus::Cancelled.is_terminal());
        assert!(!RunStatus::Running.is_terminal());
        assert!(!RunStatus::Pending.is_terminal());
    }

    #[test]
    fn stage_status_roundtrip() {
        for s in &[
            StageStatus::Pending,
            StageStatus::Running,
            StageStatus::Completed,
            StageStatus::Failed,
            StageStatus::Skipped,
        ] {
            let s2: StageStatus = s.to_string().parse().unwrap();
            assert_eq!(s, &s2);
        }
    }
}
