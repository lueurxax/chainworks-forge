pub mod ids;
pub mod idea;
pub mod run;
pub mod stage;
pub mod agent;
pub mod approval;
pub mod artifact;
pub mod commands;
pub mod events;

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
            StageStatus::Pending, StageStatus::Running, StageStatus::Completed,
            StageStatus::Failed, StageStatus::Skipped,
        ] {
            let s2: StageStatus = s.to_string().parse().unwrap();
            assert_eq!(s, &s2);
        }
    }
}
