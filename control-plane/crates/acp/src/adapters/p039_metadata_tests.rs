use super::ensure_chainworks_meta_root_launch_dir;
use crate::{ApprovedMetadataRoot, ExecutionRequest};
use domain::ids::{AgentExecutionId, RunId};
use std::{
    fs,
    os::unix::fs::{symlink, MetadataExt, PermissionsExt},
    path::Path,
};

fn identity(path: &Path) -> (u64, u64) {
    let metadata = fs::metadata(path).unwrap();
    (metadata.dev(), metadata.ino())
}

fn request(base: &Path) -> ExecutionRequest {
    for path in ["repo", "operation/checkout", "operation/metadata"] {
        fs::create_dir_all(base.join(path)).unwrap();
        fs::set_permissions(base.join(path), fs::Permissions::from_mode(0o700)).unwrap();
    }
    serde_json::from_value(serde_json::json!({
        "run_id": RunId::new(), "agent_execution_id": AgentExecutionId::new(),
        "stage_id": "review", "agent_id": "reviewer", "provider": "fixture",
        "workspace_root": base.join("repo"), "prompt": "fixture",
        "worktree_root": base.join("operation/checkout"),
        "worktree_strategy": "dedicated",
        "chainworks_meta_root": base.join("operation/metadata")
    }))
    .unwrap()
}

fn approve(req: &mut ExecutionRequest) {
    req.approved_metadata_root = Some(
        ApprovedMetadataRoot::from_engine_verified_roots(
            req.run_id,
            req.agent_execution_id.unwrap(),
            Path::new(&req.workspace_root),
            Path::new(req.worktree_root.as_deref().unwrap()),
            Path::new(req.chainworks_meta_root.as_deref().unwrap()),
            identity(Path::new(req.worktree_root.as_deref().unwrap())),
            identity(Path::new(req.chainworks_meta_root.as_deref().unwrap())),
        )
        .unwrap(),
    );
}

#[test]
fn p039_verified_metadata_capability_allows_only_exact_root() {
    let temp = tempfile::tempdir().unwrap();
    let base = temp.path().canonicalize().unwrap();
    let mut req = request(&base);
    approve(&mut req);
    let cwd = req.execution_root().unwrap().to_owned();
    ensure_chainworks_meta_root_launch_dir(&req).unwrap();
    let meta = Path::new(req.chainworks_meta_root.as_deref().unwrap());
    let mut names: Vec<_> = fs::read_dir(meta)
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    names.sort();
    assert_eq!(names, ["artifacts", "context", "state", "summaries"]);
    assert_eq!(req.execution_root().unwrap(), cwd);
    assert_eq!(Path::new(&cwd), base.join("operation/checkout"));
    assert_eq!(fs::read_dir(base.join("repo")).unwrap().count(), 0);
}

#[test]
fn p039_serialized_or_ordinary_external_root_never_grants_authority() {
    let temp = tempfile::tempdir().unwrap();
    let base = temp.path().canonicalize().unwrap();
    let mut req = request(&base);
    assert!(ensure_chainworks_meta_root_launch_dir(&req).is_err());
    let mut json = serde_json::to_value(&req).unwrap();
    json["approved_metadata_root"] = serde_json::json!({
        "run_id":req.run_id,"agent_execution_id":req.agent_execution_id,
        "workspace_root":req.workspace_root,"worktree_root":req.worktree_root,
        "metadata_root":req.chainworks_meta_root
    });
    let forged: ExecutionRequest = serde_json::from_value(json).unwrap();
    assert!(forged.approved_metadata_root.is_none());
    assert!(ensure_chainworks_meta_root_launch_dir(&forged).is_err());
    approve(&mut req);
    let encoded = serde_json::to_value(&req).unwrap();
    assert!(encoded.get("approved_metadata_root").is_none());
    let decoded: ExecutionRequest = serde_json::from_value(encoded).unwrap();
    assert!(ensure_chainworks_meta_root_launch_dir(&decoded).is_err());
    assert_eq!(
        fs::read_dir(base.join("operation/metadata"))
            .unwrap()
            .count(),
        0
    );
}

#[test]
fn p039_capability_rejects_changed_request_identity_and_roots() {
    let temp = tempfile::tempdir().unwrap();
    let base = temp.path().canonicalize().unwrap();
    let mut req = request(&base);
    approve(&mut req);
    for variant in 0..6 {
        let mut changed = req.clone();
        match variant {
            0 => changed.run_id = RunId::new(),
            1 => changed.agent_execution_id = Some(AgentExecutionId::new()),
            2 => changed.agent_execution_id = None,
            3 => changed.workspace_root = base.to_str().unwrap().into(),
            4 => changed.worktree_root = Some(base.to_str().unwrap().into()),
            _ => changed.chainworks_meta_root = Some(base.to_str().unwrap().into()),
        }
        assert!(
            ensure_chainworks_meta_root_launch_dir(&changed).is_err(),
            "variant {variant}"
        );
    }
    assert_eq!(
        fs::read_dir(base.join("operation/metadata"))
            .unwrap()
            .count(),
        0
    );
}

#[test]
fn p039_capability_rejects_replaced_and_symlinked_directories() {
    for variant in ["repo", "operation/checkout", "operation/metadata"] {
        for replace_with_link in [false, true] {
            let temp = tempfile::tempdir().unwrap();
            let base = temp.path().canonicalize().unwrap();
            let mut req = request(&base);
            approve(&mut req);
            let path = base.join(variant);
            let saved = base.join("saved");
            fs::rename(&path, &saved).unwrap();
            if replace_with_link {
                symlink(&saved, &path).unwrap();
            } else {
                fs::create_dir(&path).unwrap();
            }
            assert!(
                ensure_chainworks_meta_root_launch_dir(&req).is_err(),
                "{variant}"
            );
            assert_eq!(fs::read_dir(&path).unwrap().count(), 0);
        }
    }
}

#[test]
fn p039_capability_constructor_requires_existing_canonical_directories() {
    let temp = tempfile::tempdir().unwrap();
    let base = temp.path().canonicalize().unwrap();
    let req = request(&base);
    let outside = base.join("outside");
    fs::create_dir(&outside).unwrap();
    symlink(&outside, base.join("link")).unwrap();
    for path in [
        base.join("missing"),
        base.join("link"),
        base.join("operation/../outside"),
    ] {
        assert!(ApprovedMetadataRoot::from_engine_verified_roots(
            req.run_id,
            req.agent_execution_id.unwrap(),
            Path::new(&req.workspace_root),
            Path::new(req.worktree_root.as_deref().unwrap()),
            &path,
            identity(Path::new(req.worktree_root.as_deref().unwrap())),
            identity(Path::new(req.chainworks_meta_root.as_deref().unwrap())),
        )
        .is_err());
    }
    assert!(!base.join("missing").exists());
}

#[test]
fn p039_approved_root_never_follows_required_child_symlink() {
    let temp = tempfile::tempdir().unwrap();
    let base = temp.path().canonicalize().unwrap();
    let mut req = request(&base);
    approve(&mut req);
    let outside = base.join("outside");
    fs::create_dir(&outside).unwrap();
    symlink(&outside, base.join("operation/metadata/context")).unwrap();
    assert!(ensure_chainworks_meta_root_launch_dir(&req).is_err());
    assert_eq!(fs::read_dir(&outside).unwrap().count(), 0);
    assert!(!base.join("operation/metadata/artifacts").exists());
}

#[test]
fn p039_capability_rejects_stale_manifest_directory_identity_before_mint() {
    for changed in ["operation/checkout", "operation/metadata"] {
        let temp = tempfile::tempdir().unwrap();
        let base = temp.path().canonicalize().unwrap();
        let req = request(&base);
        let checkout = Path::new(req.worktree_root.as_deref().unwrap());
        let metadata = Path::new(req.chainworks_meta_root.as_deref().unwrap());
        let checkout_identity = identity(checkout);
        let metadata_identity = identity(metadata);
        // The handoff carries the previously verified identities, not fresh observations.
        fs::rename(base.join(changed), base.join("verified-directory")).unwrap();
        fs::create_dir(base.join(changed)).unwrap();
        fs::set_permissions(base.join(changed), fs::Permissions::from_mode(0o700)).unwrap();
        let result = ApprovedMetadataRoot::from_engine_verified_roots(
            req.run_id,
            req.agent_execution_id.unwrap(),
            Path::new(&req.workspace_root),
            checkout,
            metadata,
            checkout_identity,
            metadata_identity,
        );
        assert!(result.is_err(), "stale manifest identity: {changed}");
        assert_eq!(fs::read_dir(metadata).unwrap().count(), 0);
        assert_eq!(
            fs::read_dir(base.join("verified-directory"))
                .unwrap()
                .count(),
            0
        );
    }
}
