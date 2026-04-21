pub mod agent;
pub mod approval;
pub mod artifact;
pub mod capabilities;
pub mod commands;
pub mod events;
pub mod idea;
pub mod ids;
pub mod provider;
pub mod run;
pub mod session;
pub mod stage;
pub mod steward;
pub mod validation;

// P029: PrincipalClass is canonically defined in domain::commands.
pub use capabilities::{CapabilityToolId, ResourceTemplateId};
pub use commands::PrincipalClass;

#[cfg(test)]
mod tests {
    use super::agent::{AgentFailureKind, OperatorActionHint};
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

    #[test]
    fn proposal_061_host_interruption_labels_roundtrip() {
        let failure: AgentFailureKind = "host_interruption".parse().unwrap();
        assert_eq!(failure, AgentFailureKind::HostInterruption);
        assert_eq!(failure.to_string(), "host_interruption");

        let sleep_hint: OperatorActionHint = "recovering_from_system_sleep".parse().unwrap();
        assert_eq!(sleep_hint, OperatorActionHint::RecoveringFromSystemSleep);
        assert_eq!(sleep_hint.to_string(), "recovering_from_system_sleep");

        let network_hint: OperatorActionHint = "resuming_after_network_change".parse().unwrap();
        assert_eq!(network_hint, OperatorActionHint::ResumingAfterNetworkChange);
        assert_eq!(network_hint.to_string(), "resuming_after_network_change");
    }
}
