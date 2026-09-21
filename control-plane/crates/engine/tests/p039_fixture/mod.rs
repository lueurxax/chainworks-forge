use db::repos::run_continuations::{self as continuations, Continuation, Reservation};
use domain::{ids::RunId, run::DeliveryConfiguration, run_carry_forward::ContentDigest};
use engine::run_carry_forward::{
    preview::{
        preview, InputReference, PreparedPlan, PreviewInput, PreviewOutcome, PreviewSelection,
        PreviewTarget,
    },
    CarryForwardFinalizer, CarryForwardService, PreparationJob, PreparationLane, ServiceConfig,
};
use serde_json::{json, Value};
use sqlx::SqlitePool;
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::Arc,
};
use uuid::Uuid;

pub(crate) fn git(root: &Path, args: &[&str]) {
    let out = Command::new("/usr/bin/git")
        .current_dir(root)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .args([
            "-c",
            "core.hooksPath=/dev/null",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

pub(crate) fn copy_tree(source: &Path, target: &Path) {
    fs::create_dir_all(target).unwrap();
    for item in fs::read_dir(source).unwrap() {
        let item = item.unwrap();
        if item.file_type().unwrap().is_dir() {
            copy_tree(&item.path(), &target.join(item.file_name()));
        } else {
            fs::copy(item.path(), target.join(item.file_name())).unwrap();
        }
    }
}

pub(crate) struct Fixture {
    pub(crate) _temp: tempfile::TempDir,
    pub(crate) root: PathBuf,
    pub(crate) owned: PathBuf,
    pub(crate) metadata: PathBuf,
    pub(crate) pool: SqlitePool,
    pub(crate) input: PreviewInput,
    pub(crate) seed: Vec<u8>,
}

impl Fixture {
    pub(crate) async fn with_rollout_seed(mut self) -> Self {
        let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let contract_path = "docs/evidence/rollout-contract/p039-rollout-contract.json";
        let contract: Value =
            serde_json::from_slice(&fs::read(repository.join(contract_path)).unwrap()).unwrap();
        let mut paths = vec![
            contract_path.to_owned(),
            contract["readback_fixture"].as_str().unwrap().to_owned(),
        ];
        paths.extend(
            contract["negative_fixtures"]
                .as_object()
                .unwrap()
                .values()
                .map(|p| p.as_str().unwrap().to_owned()),
        );
        for path in paths {
            let target = self.root.join(&path);
            fs::create_dir_all(target.parent().unwrap()).unwrap();
            fs::copy(repository.join(path), target).unwrap();
        }
        git(&self.root, &["add", "docs"]);
        git(
            &self.root,
            &[
                "-c",
                "user.name=Fixture",
                "-c",
                "user.email=fixture@example.invalid",
                "commit",
                "-qm",
                "canonical rollout evidence",
            ],
        );
        self.seed = serde_json::to_vec(&json!({"title":"Continue bounded implementation work", "rollout_contract_v1":contract})).unwrap();
        fs::write(
            self.metadata.join("proposals/current/proposal.md"),
            &self.seed,
        )
        .unwrap();
        let id = match self.input.selection.proposal_input {
            InputReference::Artifact { id } => id,
            _ => unreachable!(),
        };
        sqlx::query("UPDATE artifacts SET checksum_sha256=?,size_bytes=? WHERE id=?")
            .bind(hex(&ContentDigest::of(&self.seed)))
            .bind(self.seed.len() as i64)
            .bind(id.to_string())
            .execute(&self.pool)
            .await
            .unwrap();
        self
    }

    pub(crate) fn service(&self) -> (CarryForwardService, auth::Principal) {
        let service = CarryForwardService::new(
            self.pool.clone(),
            ServiceConfig {
                enabled: true,
                owned_parent: self.owned.clone(),
            },
        );
        let mut principal =
            auth::Principal::new("finalizer-service-fixture", auth::PrincipalClass::Operator);
        use domain::CapabilityToolId::*;
        principal.tool_capabilities.extend([
            RunsContinuationPreview,
            RunsContinueBlocked,
            RunsContinuationGet,
            RunsContinuationActivate,
            RunsContinuationAbort,
            RunsContinuationReconcile,
        ]);
        (service, principal)
    }

    pub(crate) async fn service_prepare(
        &self,
    ) -> (
        CarryForwardService,
        auth::Principal,
        Value,
        Value,
        Continuation,
    ) {
        let (service, principal) = self.service();
        let request = self.service_request(&service, &principal).await;
        let accepted = service
            .call("runs.continue_blocked", &principal, request.clone())
            .await
            .unwrap();
        assert_eq!(accepted["status"], "accepted", "{accepted:#}");
        let id = accepted["operation"]["operation_id"].as_str().unwrap();
        let op = tokio::time::timeout(std::time::Duration::from_secs(60), async {
            loop {
                let op = continuations::find(&self.pool, id).await.unwrap().unwrap();
                if op.phase != "preparing" {
                    break op;
                }
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("bounded preparation wait");
        assert_eq!(op.phase, "prepared", "{op:?}");
        (service, principal, request, accepted, op)
    }

    pub(crate) async fn service_request(
        &self,
        service: &CarryForwardService,
        principal: &auth::Principal,
    ) -> Value {
        let plan = self.plan().await;
        let mut request = json!({
            "run_id":self.input.source_run_id,
            "target":{
                "workflow_yaml_path":self.input.target.workflow_yaml_path,
                "agent_catalog_yaml_path":self.input.target.agent_catalog_yaml_path,
                "expected_workflow_snapshot_hash":plan.plan().summary.target.workflow_snapshot_hash,
                "expected_catalog_snapshot_hash":plan.plan().summary.target.catalog_snapshot_hash,
                "delivery_configuration_json":self.input.target.delivery_configuration_json,
            },
            "selection":{"proposal_input":self.input.selection.proposal_input,"reference_inputs":[],"include_dirty_work":true,"excluded_workspace_paths":[]},
            "profile":"implementation_restart_v1", "limit":2,
        });
        let page = service
            .call("runs.continuation_preview", &principal, request.clone())
            .await
            .unwrap();
        let _: domain::run_carry_forward_api::RunCarryForwardPreviewPageV1 =
            serde_json::from_value(page.clone()).unwrap();
        assert_eq!(page["entries"].as_array().unwrap().len(), 2);
        request["expected_plan_sha256"] = page["plan_sha256"].clone();
        request["caller_request_id"] = json!(Uuid::new_v4());
        request["reason"] = json!("Preserve source work for fresh review");
        request
    }

    pub(crate) async fn new() -> Self {
        Self::new_with_storage(None).await
    }

    pub(crate) async fn sharing_storage(other: &Self) -> Self {
        Self::new_with_storage(Some((other.pool.clone(), other.owned.clone()))).await
    }

    async fn new_with_storage(storage: Option<(SqlitePool, PathBuf)>) -> Self {
        let temp = tempfile::tempdir().unwrap();
        let base = fs::canonicalize(temp.path()).unwrap();
        let root = base.join("repo");
        let owned = storage
            .as_ref()
            .map(|(_, owned)| owned.clone())
            .unwrap_or_else(|| base.join("operations"));
        fs::create_dir(&root).unwrap();
        if storage.is_none() {
            fs::create_dir(&owned).unwrap();
        }
        let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
        copy_tree(&repo.join("examples/agents"), &root.join("examples/agents"));
        fs::create_dir_all(root.join("examples/workflows")).unwrap();
        let original = fs::read_to_string(repo.join("examples/workflows/workflow.yaml")).unwrap();
        let workflow = root.join("examples/workflows/current.yaml");
        fs::write(&workflow, &original).unwrap();
        let catalog = root.join("examples/agents/agents.yaml");
        let old = workflow::compiler::compile_for_new_run_v1(
            workflow.to_str().unwrap(),
            catalog.to_str().unwrap(),
        )
        .unwrap()
        .into_plan();
        let mut current: Value = serde_yaml::from_str(&original).unwrap();
        current["blocked_run_continuation"] = json!({
            "schema_version":"blocked_run_continuation_profile_v1", "profile":"implementation_restart_v1",
            "review_state":"state_4_proposal_reviewed", "approval_state":"state_6_implementation_approval",
            "preparation_state":"state_7_implementation_started", "proposal_input":"proposal_current",
            "preparation_task":"freeze_approved_proposal_and_prepare_worktree"
        });
        fs::write(&workflow, serde_yaml::to_string(&current).unwrap()).unwrap();
        fs::write(root.join(".gitignore"), ".chainworks/\n").unwrap();
        fs::write(root.join("product.txt"), "base\n").unwrap();
        git(&root, &["init", "-q"]);
        git(&root, &["add", "."]);
        git(
            &root,
            &[
                "-c",
                "user.name=Fixture",
                "-c",
                "user.email=fixture@example.invalid",
                "commit",
                "-qm",
                "base",
            ],
        );
        fs::write(root.join("product.txt"), "dirty source\n").unwrap();
        let pool = if let Some((pool, _)) = storage {
            pool
        } else {
            let pool = db::pool::create_pool(&format!(
                "sqlite://{}?mode=rwc",
                base.join("fixture.db").display()
            ))
            .await
            .unwrap();
            db::writer::register_shared_writer(
                &pool,
                Arc::new(db::writer::DbWriter::new(pool.clone())),
            )
            .await
            .unwrap();
            pool
        };
        let idea = Uuid::new_v4().to_string();
        let run = RunId::new();
        let meta = format!(".chainworks/runs/{run}");
        let metadata = root.join(&meta);
        fs::create_dir_all(metadata.join("proposals/current")).unwrap();
        let seed = vec![b'x'; 2 * 1024 * 1024 + 17];
        fs::write(metadata.join("proposals/current/proposal.md"), &seed).unwrap();
        let delivery = DeliveryConfiguration {
            repo_identifier: "fixture-repository".into(),
            repo_root: root.to_str().unwrap().into(),
            base_branch: "HEAD".into(),
            worktree_base_path: owned.to_str().unwrap().into(),
            target_branch: "fixture-delivery".into(),
            release_target_id: Some("fixture".into()),
            release_mode: Some("sandbox".into()),
        };
        sqlx::query("INSERT INTO ideas(id,title,body,workspace_root_path,created_at) VALUES (?,'Source','Body',?,'2026-09-20T00:00:00Z')")
            .bind(&idea).bind(root.to_str().unwrap()).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO runs(id,idea_id,status,workflow_id,workflow_title,workspace_root,artifact_root,started_at,current_state,worktree_root,chainworks_meta_root,workflow_snapshot_json,catalog_snapshot_json,workflow_snapshot_hash,catalog_snapshot_hash,delivery_configuration_json,agent_catalog_yaml_path) VALUES (?,?,'blocked','proposal_to_release','Workflow',?,?,'2026-09-20T00:00:00Z','state_7_implementation_started',?,?,?,?,?,?,?,?)")
            .bind(run.to_string()).bind(&idea).bind(root.to_str().unwrap()).bind(metadata.to_str().unwrap())
            .bind(root.to_str().unwrap()).bind(&meta).bind(old.workflow_snapshot_json).bind(old.catalog_snapshot_json)
            .bind(old.workflow_snapshot_hash).bind(old.catalog_snapshot_hash).bind(serde_json::to_string(&delivery).unwrap()).bind(catalog.to_str().unwrap())
            .execute(&pool).await.unwrap();
        let head = Command::new("/usr/bin/git")
            .current_dir(&root)
            .args(["rev-parse", "HEAD"])
            .output()
            .unwrap();
        sqlx::query("UPDATE runs SET base_branch='HEAD',base_revision=? WHERE id=?")
            .bind(String::from_utf8(head.stdout).unwrap().trim())
            .bind(run.to_string())
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO stage_executions(id,run_id,stage_id,label,status,iteration,attempt_number,started_at) VALUES (?,?,'state_7_implementation_started','Implementation','blocked',0,1,'2026-09-20T00:00:00Z')")
            .bind(Uuid::new_v4().to_string()).bind(run.to_string()).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO approvals(id,run_id,stage_id,decision,requested_at,decided_at) VALUES (?,?,'state_6_implementation_approval','granted','2026-09-19T00:00:00Z','2026-09-19T01:00:00Z')")
            .bind(Uuid::new_v4().to_string()).bind(run.to_string()).execute(&pool).await.unwrap();
        let seed_id = Uuid::new_v4();
        let input = PreviewInput {
            source_run_id: run,
            target: PreviewTarget {
                workflow_yaml_path: "examples/workflows/current.yaml".into(),
                agent_catalog_yaml_path: "examples/agents/agents.yaml".into(),
                expected_workflow_snapshot_hash: None,
                expected_catalog_snapshot_hash: None,
                delivery_configuration_json: serde_json::to_string(&delivery).unwrap(),
            },
            selection: PreviewSelection {
                proposal_input: InputReference::Artifact { id: seed_id },
                reference_inputs: vec![],
                include_dirty_work: true,
                excluded_workspace_paths: BTreeMap::new(),
            },
            profile: "implementation_restart_v1".into(),
            cursor: None,
            limit: Some(2),
        };
        let f = Self {
            _temp: temp,
            root,
            owned,
            metadata,
            pool,
            input,
            seed,
        };
        f.artifact(
            seed_id,
            "proposal_current",
            "proposal_current",
            "proposals/current/proposal.md",
            &f.seed,
        )
        .await;
        fs::create_dir(f.metadata.join("audit")).unwrap();
        let bytes = serde_json::to_vec(&json!({"status":"needs_code_fixes","matches_proposal":false,"missing_items":[],"extra_items":[],"defects":[{"owner":"human","description":"scope undecided"}],"required_fixes":["human decision"]})).unwrap();
        fs::write(
            f.metadata.join("audit/proposal-vs-implementation.json"),
            &bytes,
        )
        .unwrap();
        f.artifact(
            Uuid::new_v4(),
            "audit_report",
            "audit_report_v1",
            "audit/proposal-vs-implementation.json",
            &bytes,
        )
        .await;
        f
    }

    pub(crate) async fn artifact(
        &self,
        id: Uuid,
        name: &str,
        contract: &str,
        path: &str,
        bytes: &[u8],
    ) {
        sqlx::query("INSERT INTO artifacts(id,run_id,stage_id,agent_id,name,contract_id,format,file_path,checksum_sha256,size_bytes,provider,created_at) VALUES (?,?,'historical','fixture',?,?,'markdown',?,?,?,'fixture','2026-09-20T00:00:00Z')")
            .bind(id.to_string()).bind(self.input.source_run_id.to_string()).bind(name).bind(contract).bind(self.metadata.join(path).to_str().unwrap())
            .bind(hex(&ContentDigest::of(bytes))).bind(bytes.len() as i64).execute(&self.pool).await.unwrap();
    }

    pub(crate) async fn plan(&self) -> PreparedPlan {
        match preview(&self.pool, self.input.clone()).await.unwrap() {
            PreviewOutcome::Page { prepared, .. } => prepared,
            result => panic!("expected plan, got {result:?}"),
        }
    }

    pub(crate) async fn reserve(&self, plan: &PreparedPlan) -> Continuation {
        continuations::reserve_checked(
            &self.pool,
            &Reservation {
                source_run_id: self.input.source_run_id.to_string(),
                caller_fingerprint: "finalizer-test".into(),
                caller_request_id: Uuid::new_v4().to_string(),
                intent_sha256: "a".repeat(64),
                plan_ref: "plan.json".into(),
                plan_sha256: hex(&plan.plan().plan_sha256).into(),
                target_ref: "target.json".into(),
                source_witness_sha256: hex(&plan.source_witness().semantic_sha256).into(),
                reason: "fixture".into(),
            },
            &self.owned,
        )
        .await
        .unwrap()
    }

    pub(crate) fn job(&self, id: Uuid, plan: PreparedPlan) -> PreparationJob {
        let finalizer = Arc::new(CarryForwardFinalizer::new(plan).unwrap());
        PreparationJob::new(id, finalizer.workspace(), self.owned.clone(), finalizer)
    }

    pub(crate) async fn prepare(&self) -> Continuation {
        let plan = self.plan().await;
        let op = self.reserve(&plan).await;
        let lane = PreparationLane::new(self.pool.clone());
        let result = lane
            .try_submit(self.job(Uuid::parse_str(&op.operation_id).unwrap(), plan))
            .unwrap()
            .wait()
            .await
            .unwrap();
        lane.shutdown().await.unwrap();
        result
    }
}

pub(crate) fn hex(digest: &ContentDigest) -> &str {
    digest.as_str().strip_prefix("sha256:").unwrap()
}

pub(crate) fn private_file_digests(root: &Path) -> BTreeMap<PathBuf, ContentDigest> {
    fn walk(root: &Path, relative: &Path, files: &mut BTreeMap<PathBuf, ContentDigest>) {
        for item in fs::read_dir(root.join(relative)).unwrap() {
            let item = item.unwrap();
            let relative = relative.join(item.file_name());
            if item.file_type().unwrap().is_dir() {
                walk(root, &relative, files);
            } else {
                files.insert(relative, ContentDigest::of(&fs::read(item.path()).unwrap()));
            }
        }
    }
    let mut files = BTreeMap::new();
    walk(root, Path::new(""), &mut files);
    files
}
