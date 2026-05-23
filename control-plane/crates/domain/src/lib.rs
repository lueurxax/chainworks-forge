pub mod agent;
pub mod approval;
pub mod artifact;
pub mod artifact_contracts;
pub mod capabilities;
pub mod closeout_readiness;
pub mod closeout_readiness_mode;
pub mod closeout_readiness_summary_accessor;
pub mod code_writer_completion;
pub mod commands;
pub mod discovery;
pub mod error_sanitizer;
pub mod events;
pub mod idea;
pub mod ids;
pub mod lifecycle;
pub mod main_sync;
pub mod mediation;
pub mod operator_action_routing;
pub mod proposal_gate_result;
pub mod provider;
pub mod recovery_matrix;
pub mod retry_authority;
pub mod retry_instruction;
pub mod risk_lineage;
pub mod routing;
pub mod run;
pub mod session;
pub mod side_effect;
pub mod stage;
pub mod steward;
pub mod toolchain;
pub mod toolchain_diagnostics;
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
