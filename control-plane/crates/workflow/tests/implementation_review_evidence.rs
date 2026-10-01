use std::path::PathBuf;

use workflow::{compiler, plan::CompiledTask};

fn review_task(agent_id: &str) -> CompiledTask {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let plan = compiler::compile(
        root.join("examples/workflows/full-mvp-live.yaml")
            .to_str()
            .unwrap(),
        root.join("examples/agents/agents.yaml").to_str().unwrap(),
    )
    .expect("full MVP workflow should compile");
    plan.states["state_9_implementation_reviewed"]
        .tasks
        .iter()
        .find(|task| task.agent.agent_id == agent_id)
        .expect("implementation reviewer should have a compiled task")
        .clone()
}

fn assert_inputs(task: &CompiledTask, required: &[&str]) {
    for input in required {
        assert!(
            task.inputs.iter().any(|candidate| candidate == input),
            "{} invocation must receive {input}; catalog inputs alone do not reach the task",
            task.agent.agent_id,
        );
    }
}

#[test]
fn security_receives_evidence_for_documentation_only_implementation() {
    let task = review_task("security_checker");
    assert_inputs(
        &task,
        &[
            "approved_proposal",
            "implementation_progress",
            "changed_files_manifest",
            "tests_result",
        ],
    );
}

#[test]
fn auditor_receives_accompanying_proposal_revision_summary() {
    let task = review_task("proposal_implementation_auditor");
    assert_inputs(&task, &["approved_proposal", "proposal_revision_summary"]);
}

#[test]
fn prepush_receives_implementation_evidence_alongside_upstream_dispositions() {
    let task = review_task("prepush_code_reviewer");
    assert_inputs(
        &task,
        &[
            "approved_proposal",
            "implementation_progress",
            "changed_files_manifest",
            "tests_result",
            "audit_report",
            "security_report",
        ],
    );
}
