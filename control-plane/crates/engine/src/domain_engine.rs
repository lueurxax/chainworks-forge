use domain::approval::Approval;
use domain::run::{Run, RunStatus};
use domain::stage::{StageExecution, StageStatus};
use domain::ids::StageExecutionId;

pub struct DomainEngine;

pub enum RunEvaluation {
    /// Run can advance to next stage
    CanAdvance { next_stage_id: StageExecutionId },
    /// Run is waiting for approval
    WaitingApproval { stage_id: String },
    /// Run is complete
    Complete,
    /// Run is blocked
    Blocked { reason: String },
    /// Run is already in a terminal state
    Terminal,
}

impl DomainEngine {
    /// Evaluate what the next action should be for a run
    pub fn evaluate_run(run: &Run, stages: &[StageExecution], _approvals: &[Approval]) -> RunEvaluation {
        if run.status.is_terminal() {
            return RunEvaluation::Terminal;
        }

        if matches!(run.status, RunStatus::Cancelling) {
            return RunEvaluation::Blocked {
                reason: "Cancellation in progress".to_string(),
            };
        }

        if Self::is_run_complete(stages) {
            return RunEvaluation::Complete;
        }

        if Self::is_run_blocked(stages) {
            return RunEvaluation::Blocked {
                reason: "One or more stages are blocked".to_string(),
            };
        }

        // Check if any stage is waiting for approval
        let waiting = stages.iter().find(|s| s.status == StageStatus::WaitingApproval);
        if let Some(stage) = waiting {
            return RunEvaluation::WaitingApproval {
                stage_id: stage.stage_id.clone(),
            };
        }

        // Find the next pending stage
        if let Some(next) = Self::next_pending_stage(stages) {
            return RunEvaluation::CanAdvance {
                next_stage_id: next.id,
            };
        }

        // There are running stages but nothing to advance — still in progress
        let any_running = stages.iter().any(|s| s.status == StageStatus::Running);
        if any_running {
            return RunEvaluation::Blocked {
                reason: "Stages currently running".to_string(),
            };
        }

        RunEvaluation::Complete
    }

    /// Check if a stage transition is legal
    pub fn is_transition_legal(from: StageStatus, to: StageStatus) -> bool {
        use StageStatus::*;
        matches!(
            (from, to),
            (Pending, Ready)
                | (Pending, Running)
                | (Ready, Running)
                | (Running, WaitingApproval)
                | (Running, Completed)
                | (Running, Failed)
                | (Running, Blocked)
                | (WaitingApproval, Running)
                | (WaitingApproval, Blocked)
                | (Blocked, Running)
                | (Blocked, Skipped)
                | (Failed, Running) // retry
                | (_, Skipped)
        )
    }

    /// Determine if a run is complete (all stages terminal)
    pub fn is_run_complete(stages: &[StageExecution]) -> bool {
        if stages.is_empty() {
            return false;
        }
        stages.iter().all(|s| {
            matches!(
                s.status,
                StageStatus::Completed | StageStatus::Skipped | StageStatus::Failed
            )
        }) && stages.iter().any(|s| s.status == StageStatus::Completed)
    }

    /// Determine if a run is blocked (any stage blocked, no pending work)
    pub fn is_run_blocked(stages: &[StageExecution]) -> bool {
        stages.iter().any(|s| s.status == StageStatus::Blocked)
    }

    /// Get the next stage to activate (first Pending or Ready after any Completed)
    pub fn next_pending_stage(stages: &[StageExecution]) -> Option<&StageExecution> {
        stages
            .iter()
            .find(|s| matches!(s.status, StageStatus::Pending | StageStatus::Ready))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::run::{Run, RunStatus};
    use domain::stage::{StageExecution, StageStatus};
    use domain::ids::{IdeaId, RunId, StageExecutionId};
    use chrono::Utc;

    fn make_run(status: RunStatus) -> Run {
        Run {
            id: RunId::new(),
            idea_id: IdeaId::new(),
            status,
            workflow_id: "wf1".into(),
            workflow_title: "Test Workflow".into(),
            workspace_root: "/tmp".into(),
            artifact_root: "/tmp/artifacts".into(),
            started_at: Utc::now(),
            completed_at: None,
            cancellation_requested_at: None,
            cancellation_settled_at: None,
        }
    }

    fn make_stage(run_id: RunId, status: StageStatus) -> StageExecution {
        StageExecution {
            id: StageExecutionId::new(),
            run_id,
            stage_id: "stage-1".into(),
            label: "Stage 1".into(),
            status,
            iteration: 1,
            attempt_number: 1,
            settlement_kind: None,
            started_at: Utc::now(),
            completed_at: None,
        }
    }

    #[test]
    fn complete_when_all_stages_done() {
        let run = make_run(RunStatus::Running);
        let stage = make_stage(run.id, StageStatus::Completed);
        let eval = DomainEngine::evaluate_run(&run, &[stage], &[]);
        assert!(matches!(eval, RunEvaluation::Complete));
    }

    #[test]
    fn can_advance_when_pending_stage_exists() {
        let run = make_run(RunStatus::Running);
        let completed = make_stage(run.id, StageStatus::Completed);
        let mut pending = make_stage(run.id, StageStatus::Pending);
        pending.stage_id = "stage-2".into();
        let eval = DomainEngine::evaluate_run(&run, &[completed, pending], &[]);
        assert!(matches!(eval, RunEvaluation::CanAdvance { .. }));
    }

    #[test]
    fn waiting_approval_when_stage_needs_it() {
        let run = make_run(RunStatus::Running);
        let stage = make_stage(run.id, StageStatus::WaitingApproval);
        let eval = DomainEngine::evaluate_run(&run, &[stage.clone()], &[]);
        assert!(matches!(eval, RunEvaluation::WaitingApproval { .. }));
    }

    #[test]
    fn blocked_when_stage_is_blocked() {
        let run = make_run(RunStatus::Running);
        let stage = make_stage(run.id, StageStatus::Blocked);
        let eval = DomainEngine::evaluate_run(&run, &[stage], &[]);
        assert!(matches!(eval, RunEvaluation::Blocked { .. }));
    }
}
