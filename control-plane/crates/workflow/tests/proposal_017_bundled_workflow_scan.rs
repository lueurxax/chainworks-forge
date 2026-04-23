use std::path::PathBuf;

use workflow::transition_lint::scan_workflow_file_for_simultaneous_transitions;

#[test]
fn proposal_017_bundled_workflows_have_no_static_simultaneous_transition_matches() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("..");
    let workflow_dir = root.join("examples").join("workflows");
    let workflow_paths = [
        workflow_dir.join("workflow.yaml"),
        workflow_dir.join("full-mvp-live.yaml"),
        workflow_dir.join("proposal-loop-live.yaml"),
        workflow_dir.join("proposal-to-release.yaml"),
    ];

    let mut findings = Vec::new();
    for workflow_path in workflow_paths {
        findings.extend(
            scan_workflow_file_for_simultaneous_transitions(&workflow_path)
                .unwrap_or_else(|err| panic!("scan {}: {err:#}", workflow_path.display())),
        );
    }

    assert!(
        findings.is_empty(),
        "P017 bundled workflow simultaneous transition findings: {findings:#?}"
    );
}
