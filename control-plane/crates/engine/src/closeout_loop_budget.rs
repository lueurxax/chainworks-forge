use domain::stage::StageExecution;

pub fn closeout_loop_budget_remaining(
    plan: &workflow::plan::RunPlan,
    stages: &[StageExecution],
    refine_state_id: &str,
) -> bool {
    closeout_loop_budget_exhaustion(plan, stages, refine_state_id).is_none()
}

pub(crate) struct CloseoutLoopBudgetExhaustion {
    pub counter: String,
    pub iterations: u64,
    pub max: u64,
}

pub(crate) fn closeout_loop_budget_exhaustion(
    plan: &workflow::plan::RunPlan,
    stages: &[StageExecution],
    refine_state_id: &str,
) -> Option<CloseoutLoopBudgetExhaustion> {
    let target_state = plan.states.get(refine_state_id)?;
    let loop_config = target_state.loop_config.as_ref()?;
    let iterations = loop_iterations_for_state(stages, refine_state_id);
    (iterations >= loop_config.max).then(|| CloseoutLoopBudgetExhaustion {
        counter: loop_config.counter.clone(),
        iterations,
        max: loop_config.max,
    })
}

fn loop_iterations_for_state(stages: &[StageExecution], state_id: &str) -> u64 {
    stages
        .iter()
        .filter(|stage| stage.stage_id == state_id)
        .filter_map(|stage| u64::try_from(stage.iteration).ok())
        .max()
        .unwrap_or(0)
}
