#![allow(dead_code, unused_imports)]
// Real Git/SQLite service activation fixture mirrors proposal_039_finalize.
use anyhow::Result;
use async_trait::async_trait;
use db::repos::run_continuations::{self as continuations, Continuation, Reservation};
use domain::{
    ids::RunId,
    run::DeliveryConfiguration,
    run_carry_forward::{canonical_digest, ContentDigest, EntryRole},
};
use engine::run_carry_forward::{
    preview::{
        preview, InputReference, PreparedPlan, PreviewInput, PreviewOutcome, PreviewSelection,
        PreviewTarget,
    },
    read_manifest, read_verified_manifest, CarryForwardFinalizer, CarryForwardService,
    PreparationJob, PreparationLane, PreparationObserver, ServiceConfig,
};
use serde_json::{json, Value};
use sqlx::SqlitePool;
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};
use uuid::Uuid;

fn git(root: &Path, args: &[&str]) {
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

fn copy_tree(source: &Path, target: &Path) {
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

struct Fixture {
    _temp: tempfile::TempDir,
    root: PathBuf,
    owned: PathBuf,
    metadata: PathBuf,
    pool: SqlitePool,
    input: PreviewInput,
    seed: Vec<u8>,
}

impl Fixture {
    async fn with_rollout_seed(mut self) -> Self {
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

    async fn service_prepare(
        &self,
    ) -> (
        CarryForwardService,
        auth::Principal,
        Value,
        Value,
        Continuation,
    ) {
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

    async fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let base = fs::canonicalize(temp.path()).unwrap();
        let root = base.join("repo");
        let owned = base.join("operations");
        fs::create_dir(&root).unwrap();
        fs::create_dir(&owned).unwrap();
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
        fs::rename(root.join("product.txt"), root.join("renamed.txt")).unwrap();
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

    async fn artifact(&self, id: Uuid, name: &str, contract: &str, path: &str, bytes: &[u8]) {
        sqlx::query("INSERT INTO artifacts(id,run_id,stage_id,agent_id,name,contract_id,format,file_path,checksum_sha256,size_bytes,provider,created_at) VALUES (?,?,'historical','fixture',?,?,'markdown',?,?,?,'fixture','2026-09-20T00:00:00Z')")
            .bind(id.to_string()).bind(self.input.source_run_id.to_string()).bind(name).bind(contract).bind(self.metadata.join(path).to_str().unwrap())
            .bind(hex(&ContentDigest::of(bytes))).bind(bytes.len() as i64).execute(&self.pool).await.unwrap();
    }

    async fn plan(&self) -> PreparedPlan {
        match preview(&self.pool, self.input.clone()).await.unwrap() {
            PreviewOutcome::Page { prepared, .. } => prepared,
            result => panic!("expected plan, got {result:?}"),
        }
    }

    async fn reserve(&self, plan: &PreparedPlan) -> Continuation {
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

    fn job(&self, id: Uuid, plan: PreparedPlan) -> PreparationJob {
        let finalizer = Arc::new(CarryForwardFinalizer::new(plan).unwrap());
        PreparationJob::new(id, finalizer.workspace(), self.owned.clone(), finalizer)
    }

    async fn prepare(&self) -> Continuation {
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

fn hex(digest: &ContentDigest) -> &str {
    digest.as_str().strip_prefix("sha256:").unwrap()
}

async fn activated() -> (
    Fixture,
    engine::run_carry_forward::ManifestV1,
    domain::run::Run,
) {
    let f = Fixture::new().await.with_rollout_seed().await;
    let (service, principal, _, _, op) = f.service_prepare().await;
    let manifest = read_manifest(&op).await.unwrap();
    let result = service.call("runs.continuation_activate", &principal, json!({
        "operation_id":op.operation_id,"expected_version":op.version,
        "expected_manifest_sha256":ContentDigest::from_sha256_hex(op.manifest_sha256.as_deref().unwrap()).unwrap(),
        "caller_request_id":Uuid::new_v4()
    })).await.unwrap();
    assert_eq!(result["status"], "accepted", "{result:#}");
    let run = db::repos::runs::find_by_id(&f.pool, manifest.successor.id)
        .await
        .unwrap()
        .unwrap();
    assert!(!manifest
        .workspace
        .checkout_root
        .join("product.txt")
        .exists());
    assert_eq!(
        fs::read(manifest.workspace.checkout_root.join("renamed.txt")).unwrap(),
        b"dirty source\n"
    );
    (f, manifest, run)
}

#[tokio::test]
async fn installed_seed_and_human_findings_reach_review_and_planner_without_output_authority() {
    use engine::run_carry_forward::inputs::{
        output_for_predicate, resolve_task_inputs, InputOrigin, OutputResolution,
    };
    let (f, m, run) = activated().await;
    let mut task = m.target.plan.states["state_4_proposal_reviewed"].tasks[0].clone();
    task.inputs = vec!["proposal_current".into(), "audit_report".into()];
    let before: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM artifacts")
        .fetch_one(&f.pool)
        .await
        .unwrap();
    let context = resolve_task_inputs(&f.pool, &run, &m.target.plan, &task)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        context.inputs["proposal_current"].as_ref().unwrap().origin,
        InputOrigin::ExecutionSeed
    );
    assert!(context.inputs["audit_report"].is_none());
    assert_eq!(context.references.len(), 1);
    assert!(context.references[0].content.contains("scope undecided"));
    assert!(context.references[0]
        .name
        .starts_with("carry_forward_reference/"));
    assert_eq!(context.references[0].origin, InputOrigin::ReferenceOnly);
    for name in ["proposal_current", "audit_report", "approved_proposal"] {
        assert!(matches!(
            output_for_predicate(&f.pool, &run, &m.target.plan, name)
                .await
                .unwrap(),
            OutputResolution::Missing
        ));
    }
    let idea = db::repos::ideas::find_by_id(&f.pool, run.idea_id)
        .await
        .unwrap()
        .unwrap();
    let prompt = engine::orchestrator::build_task_prompt_for_runtime(
        &f.pool,
        &task,
        &m.target.plan,
        &run,
        Some(&idea),
        None,
        None,
        false,
    )
    .await
    .unwrap();
    assert!(prompt.contains("carry_forward_reference/"));
    assert!(prompt.contains("scope undecided"));
    assert!(prompt.contains("not approval, waiver, gate evidence, or a current provider output"));
    let mut planner_run = run.clone();
    planner_run.current_state = Some("state_7_implementation_started".into());
    let planner = &m.target.plan.states["state_7_implementation_started"].tasks[0];
    let prompt = engine::orchestrator::build_task_prompt_for_runtime(
        &f.pool,
        planner,
        &m.target.plan,
        &planner_run,
        Some(&idea),
        None,
        None,
        false,
    )
    .await
    .unwrap();
    assert!(prompt.contains("scope undecided"));
    assert_eq!(
        before,
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM artifacts")
            .fetch_one(&f.pool)
            .await
            .unwrap()
    );
}

async fn provider_artifact(
    f: &Fixture,
    run: &domain::run::Run,
    path: &Path,
    bytes: &[u8],
    valid: bool,
) -> String {
    let stage = Uuid::new_v4().to_string();
    let agent = Uuid::new_v4().to_string();
    let id = Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO stage_executions(id,run_id,stage_id,label,status,iteration,attempt_number,started_at) VALUES (?,?,'state_5_proposal_refined','Refine','completed',0,1,'2026-09-20T02:00:00Z')")
        .bind(&stage).bind(run.id.to_string()).execute(&f.pool).await.unwrap();
    sqlx::query("INSERT INTO agent_executions(id,stage_execution_id,agent_id,status,provider,started_at,completed_at) VALUES (?,?,'proposal_writer','completed','fixture','2026-09-20T02:00:00Z','2026-09-20T02:01:00Z')")
        .bind(&agent).bind(&stage).execute(&f.pool).await.unwrap();
    sqlx::query("INSERT INTO agent_execution_runtime_facts(agent_execution_id,output_settlement,valid_required_outputs,created_at,updated_at) VALUES (?,?,?,'2026-09-20T02:01:00Z','2026-09-20T02:01:00Z')")
        .bind(&agent).bind(if valid {"valid_outputs_from_completed_execution"}else{"invalid_required_outputs"}).bind(valid).execute(&f.pool).await.unwrap();
    sqlx::query("INSERT INTO artifacts(id,run_id,stage_id,agent_id,name,contract_id,format,file_path,checksum_sha256,size_bytes,provider,agent_execution_id,created_at) VALUES (?,?,'state_5_proposal_refined','proposal_writer','proposal_current','proposal_current','markdown',?,?,?,'fixture',?,?)")
        .bind(&id).bind(run.id.to_string()).bind(path.to_str().unwrap()).bind(hex(&ContentDigest::of(bytes))).bind(bytes.len() as i64).bind(agent)
        .bind(if valid {"2026-09-20T02:01:00Z"}else{"2026-09-20T03:01:00Z"}).execute(&f.pool).await.unwrap();
    id
}

#[tokio::test]
async fn production_declared_output_persistence_feeds_successor_input_without_metadata_repair() {
    use domain::{
        agent::AgentStatus,
        artifact_contracts::ArtifactSourceGenerationClaimKey,
        ids::{AgentExecutionId, StageExecutionId},
        mediation::OwnerKind,
    };
    use engine::{
        contracts::{CapturedOutput, DeclaredOutput},
        executor::BackgroundExecutor,
        run_carry_forward::inputs::{self, InputOrigin, OutputResolution},
    };
    let (f, manifest, run) = activated().await;
    let provider = &manifest.target.plan.states["state_5_proposal_refined"].tasks[0]
        .agent
        .provider;
    let stage = StageExecutionId::new();
    let agent = AgentExecutionId::new();
    sqlx::query("INSERT INTO stage_executions(id,run_id,stage_id,label,status,iteration,attempt_number,started_at) VALUES (?,?,'state_5_proposal_refined','Refine','running',0,1,'2026-09-20T02:00:00Z')")
        .bind(stage.to_string()).bind(run.id.to_string()).execute(&f.pool).await.unwrap();
    sqlx::query("INSERT INTO agent_executions(id,stage_execution_id,agent_id,status,provider,started_at) VALUES (?,?,'proposal_writer','running',?,'2026-09-20T02:00:00Z')")
        .bind(agent.to_string()).bind(stage.to_string()).bind(provider).execute(&f.pool).await.unwrap();
    let path = manifest
        .workspace
        .metadata_root
        .join("proposals/current/proposal.md");
    let bytes = b"# Fresh ordinary refinement\nProduced after activation.\n".to_vec();
    fs::write(&path, &bytes).unwrap();
    let output = DeclaredOutput {
        output_name: "proposal_current".into(),
        target_path: path.to_str().unwrap().into(),
        schema: None,
        reuse_policy: None,
        companion_output_name: None,
        companion_path: None,
    };
    let captured = CapturedOutput {
        declared: output.clone(),
        machine_bytes: Some(bytes.clone()),
        companion_bytes: None,
    };
    let events = engine::event_bus::new_bus(16);
    let queue = engine::work_queue::WorkQueue::new(f.pool.clone());
    let orchestrator = Arc::new(engine::orchestrator::Orchestrator::new(
        f.pool.clone(),
        events.clone(),
        queue.clone(),
    ));
    let executor = BackgroundExecutor::new(
        f.pool.clone(),
        queue,
        orchestrator,
        Arc::new(acp::AcpRuntimeManager::new_with_adapters(vec![])),
        events,
    );
    let claim = ArtifactSourceGenerationClaimKey {
        run_id: run.id,
        owner_kind: OwnerKind::StageExecution,
        owner_id: stage.to_string(),
        stage_execution_id: Some(stage),
        agent_execution_id: agent,
        source_work_item_id: "p039-production-output".into(),
    };
    executor
        .p082_import_declared_contract_outputs_for_regression(
            &[output],
            &[captured],
            &run.workspace_root,
            run.id,
            "state_5_proposal_refined",
            "proposal_writer",
            provider,
            None,
            stage,
            agent,
            "p039-production-output",
            &claim,
            None,
            AgentStatus::Completed,
            chrono::Utc::now(),
        )
        .await
        .unwrap();
    let rows = db::repos::artifacts::list_by_run(&f.pool, run.id)
        .await
        .unwrap();
    let produced = rows.iter().find(|a| a.name == "proposal_current").unwrap();
    assert_eq!(
        produced.checksum_sha256.as_deref(),
        Some(hex(&ContentDigest::of(&bytes)))
    );
    assert_eq!(produced.contract_id, format!("{provider}.output"));
    assert_eq!(produced.agent_execution_id, Some(agent.to_string()));
    let facts = db::repos::agent_execution_runtime_facts::find_by_execution_id(&f.pool, agent)
        .await
        .unwrap()
        .unwrap();
    assert!(
        !facts.valid_required_outputs,
        "plain outputs do not invent a passed schema"
    );
    let resolved =
        inputs::resolve_execution_input(&f.pool, &run, &manifest.target.plan, "proposal_current")
            .await
            .unwrap()
            .unwrap();
    assert_eq!(resolved.content.as_bytes(), bytes);
    assert_eq!(resolved.origin, InputOrigin::ProviderOutput);
    assert_eq!(resolved.artifact_id, Some(produced.id));
    assert!(matches!(
        inputs::output_for_predicate(&f.pool, &run, &manifest.target.plan, "proposal_current")
            .await
            .unwrap(),
        OutputResolution::Output(_)
    ));
    for (change, restore) in [
        ("UPDATE agent_executions SET status='failed' WHERE id=?", "UPDATE agent_executions SET status='completed' WHERE id=?"),
        ("UPDATE agent_execution_runtime_facts SET failure_kind='provider_error' WHERE agent_execution_id=?", "UPDATE agent_execution_runtime_facts SET failure_kind=NULL WHERE agent_execution_id=?"),
        ("UPDATE agent_execution_runtime_facts SET output_settlement='invalid_required_outputs' WHERE agent_execution_id=?", "UPDATE agent_execution_runtime_facts SET output_settlement='valid_outputs_from_completed_execution' WHERE agent_execution_id=?"),
    ] {
        sqlx::query(sqlx::AssertSqlSafe(change)).bind(agent.to_string()).execute(&f.pool).await.unwrap();
        assert!(matches!(inputs::output_for_predicate(&f.pool, &run, &manifest.target.plan, "proposal_current").await.unwrap(), OutputResolution::Missing));
        sqlx::query(sqlx::AssertSqlSafe(restore)).bind(agent.to_string()).execute(&f.pool).await.unwrap();
    }
    sqlx::query("UPDATE stage_executions SET stage_id='state_7_implementation_started' WHERE id=?")
        .bind(stage.to_string())
        .execute(&f.pool)
        .await
        .unwrap();
    assert!(
        inputs::output_for_predicate(&f.pool, &run, &manifest.target.plan, "proposal_current")
            .await
            .unwrap_err()
            .to_string()
            .contains("undeclared plain output")
    );
    sqlx::query("UPDATE stage_executions SET stage_id='state_5_proposal_refined' WHERE id=?")
        .bind(stage.to_string())
        .execute(&f.pool)
        .await
        .unwrap();
    fs::write(&path, b"unregistered changed proposal").unwrap();
    assert!(
        inputs::output_for_predicate(&f.pool, &run, &manifest.target.plan, "proposal_current")
            .await
            .is_err()
    );
}

#[tokio::test]
async fn production_mixed_review_outputs_feed_the_gate_without_aggregate_fact_repair() {
    use domain::{
        agent::AgentStatus,
        artifact_contracts::ArtifactSourceGenerationClaimKey,
        ids::{AgentExecutionId, StageExecutionId},
        mediation::OwnerKind,
    };
    use engine::{
        contracts::{CapturedOutput, DeclaredOutput},
        executor::BackgroundExecutor,
        run_carry_forward::inputs::{self, InputOrigin, OutputResolution},
    };
    for bad_sibling in [None, Some("missing"), Some("invalid")] {
        let (f, manifest, run) = activated().await;
        let state = "state_4_proposal_reviewed";
        let task = manifest.target.plan.states[state]
            .tasks
            .iter()
            .find(|task| task.task_name == "aggregate_proposal_reviews")
            .unwrap();
        let stage = StageExecutionId::new();
        let agent = AgentExecutionId::new();
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query("INSERT INTO stage_executions(id,run_id,stage_id,label,status,iteration,attempt_number,started_at,completed_at) VALUES (?,?,?,'Review','completed',0,1,?,?)")
            .bind(stage.to_string()).bind(run.id.to_string()).bind(state).bind(&now).bind(&now).execute(&f.pool).await.unwrap();
        sqlx::query("INSERT INTO agent_executions(id,stage_execution_id,agent_id,status,provider,started_at) VALUES (?,?,?,'running',?,?)")
            .bind(agent.to_string()).bind(stage.to_string()).bind(&task.agent.agent_id).bind(&task.agent.provider).bind(&now).execute(&f.pool).await.unwrap();
        let summary = json!({"average_score":10,"aggregate_score":10,"min_individual_score":10,"blocker_count":0,"blocking_issues":[],"blocking_required_changes":[],"advisory_follow_ups":[],"recurring_themes":[],"summary":"Fresh review passed","decision":"approved","pass":true});
        let payloads = json!({
            "proposal_review_summary": summary,
            "review_corpus_bundle_v2": {"selected_review_artifacts":[],"selected_reviewer_ids":[],"reviewer_count":0,"selection_plan_hash":"fixture","selection_plan":{},"legacy_fixed_mode":false},
            "score_lift_backlog": {"review_pass_id":"fresh-review","source_proposal_artifact":"proposal_current","items":[]},
            "proposal_fact_digest": {"proposal_revision_id":"fresh","claims":[]},
            "reviewer_scope_plan": {"scopes":[]},
            "orchestrator_summary": {"summary":"Fresh complete review"},
            "run_state": {"advisory":true}
        });
        let mut outputs = Vec::new();
        let mut captured = Vec::new();
        for name in &task.outputs {
            assert!(
                payloads.get(name).is_some(),
                "new frozen output needs explicit fixture: {name}"
            );
            let path = engine::orchestrator::resolve_path_template(
                &manifest.target.plan.artifact_paths[name],
                &run.workspace_root,
                run.chainworks_meta_root.as_deref(),
            );
            fs::create_dir_all(Path::new(&path).parent().unwrap()).unwrap();
            let output = DeclaredOutput {
                output_name: name.clone(),
                target_path: path.clone(),
                schema: task.output_schemas.get(name).cloned(),
                reuse_policy: None,
                companion_output_name: None,
                companion_path: None,
            };
            let bytes = if name == "score_lift_backlog" && bad_sibling == Some("missing") {
                None
            } else if name == "score_lift_backlog" && bad_sibling == Some("invalid") {
                Some(b"{invalid".to_vec())
            } else {
                Some(serde_json::to_vec(&payloads[name]).unwrap())
            };
            if let Some(bytes) = &bytes {
                fs::write(&path, bytes).unwrap();
            }
            captured.push(CapturedOutput {
                declared: output.clone(),
                machine_bytes: bytes,
                companion_bytes: None,
            });
            outputs.push(output);
        }
        assert!(outputs.iter().any(|output| output.schema.is_none()));
        let events = engine::event_bus::new_bus(16);
        let queue = engine::work_queue::WorkQueue::new(f.pool.clone());
        let orchestrator = Arc::new(engine::orchestrator::Orchestrator::new(
            f.pool.clone(),
            events.clone(),
            queue.clone(),
        ));
        let executor = BackgroundExecutor::new(
            f.pool.clone(),
            queue,
            orchestrator.clone(),
            Arc::new(acp::AcpRuntimeManager::new_with_adapters(vec![])),
            events,
        );
        let claim = ArtifactSourceGenerationClaimKey {
            run_id: run.id,
            owner_kind: OwnerKind::StageExecution,
            owner_id: stage.to_string(),
            stage_execution_id: Some(stage),
            agent_execution_id: agent,
            source_work_item_id: "mixed-review".into(),
        };
        executor
            .p082_import_declared_contract_outputs_for_regression(
                &outputs,
                &captured,
                &run.workspace_root,
                run.id,
                state,
                &task.agent.agent_id,
                &task.agent.provider,
                None,
                stage,
                agent,
                "mixed-review",
                &claim,
                None,
                AgentStatus::Completed,
                chrono::Utc::now(),
            )
            .await
            .unwrap();
        let facts = db::repos::agent_execution_runtime_facts::find_by_execution_id(&f.pool, agent)
            .await
            .unwrap()
            .unwrap();
        assert!(
            !facts.valid_required_outputs,
            "do not rewrite execution-wide schema facts"
        );
        let resolved = inputs::output_for_predicate(
            &f.pool,
            &run,
            &manifest.target.plan,
            "proposal_review_summary",
        )
        .await
        .unwrap();
        if bad_sibling.is_some() {
            assert!(
                matches!(resolved, OutputResolution::Missing),
                "a required sibling failure cannot promote the summary"
            );
            continue;
        }
        let OutputResolution::Output(resolved) = resolved else {
            panic!("successful mixed task must expose its validated summary")
        };
        assert_eq!(resolved.origin, InputOrigin::ProviderOutput);
        assert_eq!(
            serde_json::from_str::<Value>(&resolved.content).unwrap(),
            summary
        );
        orchestrator.advance_run(run.id).await.unwrap();
        assert_eq!(
            db::repos::runs::find_by_id(&f.pool, run.id)
                .await
                .unwrap()
                .unwrap()
                .current_state
                .as_deref(),
            Some("state_6_implementation_approval")
        );
        let artifact = resolved.artifact_id.unwrap();
        sqlx::query("DELETE FROM active_artifact_contracts WHERE run_id=? AND contract_id='proposal_review_summary_v2'")
            .bind(run.id.to_string()).execute(&f.pool).await.unwrap();
        assert!(
            inputs::output_for_predicate(
                &f.pool,
                &run,
                &manifest.target.plan,
                "proposal_review_summary"
            )
            .await
            .is_err(),
            "bound schema bytes without active per-output generation are not authority: {artifact}"
        );
    }
}

#[tokio::test]
async fn ordinary_frozen_provider_fallback_output_is_readable_only_with_bound_dispatch() {
    use domain::{
        agent::{AgentExecutionRuntimeFacts, AgentFailureKind, AgentOutputSettlement, AgentStatus},
        ids::{AgentExecutionId, StageExecutionId},
    };
    use engine::{
        contracts::{CapturedOutput, DeclaredOutput},
        executor::{claim_next_invoke_agent_with_start, BackgroundExecutor},
        run_carry_forward::inputs::{self, InputOrigin},
    };
    let (f, manifest, run) = activated().await;
    let task = &manifest.target.plan.states["state_5_proposal_refined"].tasks[0];
    let failed_stage = StageExecutionId::new();
    let failed_agent = AgentExecutionId::new();
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query("INSERT INTO stage_executions(id,run_id,stage_id,label,status,started_at,completed_at) VALUES (?,?,'state_2_proposal_created','Prior proposal','failed',?,?)")
        .bind(failed_stage.to_string()).bind(run.id.to_string()).bind(&now).bind(&now).execute(&f.pool).await.unwrap();
    sqlx::query("INSERT INTO agent_executions(id,stage_execution_id,agent_id,provider,status,started_at,completed_at,owner_kind,owner_id) VALUES (?,?,?,?,'failed',?,?,'stage_execution',?)")
        .bind(failed_agent.to_string()).bind(failed_stage.to_string()).bind(&task.agent.agent_id).bind(&task.agent.provider).bind(&now).bind(&now).bind(failed_stage.to_string()).execute(&f.pool).await.unwrap();
    let mut facts = AgentExecutionRuntimeFacts::defaults_for(failed_agent, chrono::Utc::now());
    facts.failure_kind = Some(AgentFailureKind::MissingRequiredOutputs);
    facts.output_settlement = AgentOutputSettlement::MissingRequiredOutputs;
    db::repos::agent_execution_runtime_facts::upsert(&f.pool, &facts)
        .await
        .unwrap();
    sqlx::query("UPDATE runs SET current_state='state_5_proposal_refined' WHERE id=?")
        .bind(run.id.to_string())
        .execute(&f.pool)
        .await
        .unwrap();
    let run = db::repos::runs::find_by_id(&f.pool, run.id)
        .await
        .unwrap()
        .unwrap();
    let events = engine::event_bus::new_bus(32);
    let queue = engine::work_queue::WorkQueue::new(f.pool.clone());
    let orchestrator = Arc::new(engine::orchestrator::Orchestrator::new(
        f.pool.clone(),
        events.clone(),
        queue.clone(),
    ));
    orchestrator.advance_run(run.id).await.unwrap();
    let queued = db::repos::work_items::list_by_run(&f.pool, run.id)
        .await
        .unwrap();
    assert!(
        queued
            .iter()
            .any(|item| item.kind == db::work_item::WorkItemKind::InvokeAgent),
        "refinement not queued; stages={:#?}",
        db::repos::stages::list_by_run(&f.pool, run.id)
            .await
            .unwrap()
    );
    // Advance only the fixture's due timestamp past the scheduler's one-second grace.
    sqlx::query("UPDATE work_items SET scheduled_at=? WHERE run_id=? AND kind='invoke_agent' AND status='pending'")
        .bind((chrono::Utc::now() - chrono::Duration::seconds(2)).to_rfc3339())
        .bind(run.id.to_string()).execute(&f.pool).await.unwrap();
    let claimed = claim_next_invoke_agent_with_start(&f.pool)
        .await
        .unwrap()
        .unwrap_or_else(|| panic!("ordinary refinement claim absent after enqueue"));
    let item = db::repos::work_items::find_by_id(&f.pool, &claimed.work_item_id)
        .await
        .unwrap()
        .unwrap();
    let payload: Value = serde_json::from_str(&item.payload_json).unwrap();
    let provider = payload["provider"].as_str().unwrap();
    assert_ne!(provider, task.agent.provider);
    assert_eq!(
        payload["provider_health_fallback"]["failed_agent_execution_id"],
        failed_agent.to_string()
    );
    let outputs: Vec<DeclaredOutput> =
        serde_json::from_value(payload["declared_outputs"].clone()).unwrap();
    let bytes = b"# Proposal written by the authorized fallback\n".to_vec();
    let mut captured = Vec::new();
    for output in &outputs {
        let content = if output.output_name == "proposal_current" {
            bytes.clone()
        } else {
            let mut value = serde_json::Map::new();
            if let Some(schema) = &output.schema {
                for name in &schema.required_fields {
                    value.insert(name.clone(), json!([]));
                }
            }
            serde_json::to_vec(&value).unwrap()
        };
        fs::create_dir_all(Path::new(&output.target_path).parent().unwrap()).unwrap();
        fs::write(&output.target_path, &content).unwrap();
        captured.push(CapturedOutput {
            declared: output.clone(),
            machine_bytes: Some(content),
            companion_bytes: None,
        });
    }
    let executor = BackgroundExecutor::new(
        f.pool.clone(),
        queue,
        orchestrator,
        Arc::new(acp::AcpRuntimeManager::new_with_adapters(vec![])),
        events,
    );
    executor
        .p082_import_declared_contract_outputs_for_regression(
            &outputs,
            &captured,
            &run.workspace_root,
            run.id,
            "state_5_proposal_refined",
            &task.agent.agent_id,
            provider,
            payload["model"].as_str(),
            claimed.stage_execution_id,
            claimed.agent_execution_id,
            &claimed.work_item_id,
            &claimed.artifact_claim_key,
            claimed.session_generation_id.as_deref(),
            AgentStatus::Completed,
            chrono::Utc::now(),
        )
        .await
        .unwrap();
    let resolved =
        inputs::resolve_execution_input(&f.pool, &run, &manifest.target.plan, "proposal_current")
            .await
            .unwrap()
            .unwrap();
    assert_eq!(resolved.origin, InputOrigin::ProviderOutput);
    assert_eq!(resolved.content.as_bytes(), bytes);
    for pointer in [
        "/provider_health_fallback",
        "/backend_profile_id",
        "/p058_claimed/agent_execution_id",
    ] {
        let mut changed = payload.clone();
        *changed.pointer_mut(pointer).unwrap() = json!("unbound");
        sqlx::query("UPDATE work_items SET payload_json=? WHERE id=?")
            .bind(changed.to_string())
            .bind(&claimed.work_item_id)
            .execute(&f.pool)
            .await
            .unwrap();
        assert!(
            inputs::resolve_execution_input(
                &f.pool,
                &run,
                &manifest.target.plan,
                "proposal_current"
            )
            .await
            .is_err(),
            "missing dispatch proof must hold: {pointer}"
        );
        sqlx::query("UPDATE work_items SET payload_json=? WHERE id=?")
            .bind(&item.payload_json)
            .bind(&claimed.work_item_id)
            .execute(&f.pool)
            .await
            .unwrap();
    }
}

#[tokio::test]
async fn valid_successor_output_wins_but_newer_invalid_output_does_not_gain_authority() {
    use engine::run_carry_forward::inputs::{
        output_for_predicate, resolve_task_inputs, InputOrigin, OutputResolution,
    };
    let (f, m, run) = activated().await;
    let seed = m
        .inputs
        .iter()
        .find(|i| i.role == EntryRole::ExecutionSeed)
        .unwrap();
    let path = m.workspace.metadata_root.join(&seed.target_relative_path);
    let fresh = b"# Fresh proposal\nCurrent successor output.\n";
    fs::write(&path, fresh).unwrap();
    provider_artifact(&f, &run, &path, fresh, true).await;
    let task = &m.target.plan.states["state_4_proposal_reviewed"].tasks[0];
    let context = resolve_task_inputs(&f.pool, &run, &m.target.plan, task)
        .await
        .unwrap()
        .unwrap();
    let input = context.inputs["proposal_current"].as_ref().unwrap();
    assert_eq!(input.origin, InputOrigin::ProviderOutput);
    assert_eq!(input.content.as_bytes(), fresh);
    provider_artifact(
        &f,
        &run,
        &path,
        b"invalid candidate never materialized",
        false,
    )
    .await;
    let context = resolve_task_inputs(&f.pool, &run, &m.target.plan, task)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        context.inputs["proposal_current"]
            .as_ref()
            .unwrap()
            .content
            .as_bytes(),
        fresh
    );
    assert!(matches!(
        output_for_predicate(&f.pool, &run, &m.target.plan, "proposal_current")
            .await
            .unwrap(),
        OutputResolution::Output(_)
    ));
    fs::write(&path, "unvalidated replacement").unwrap();
    assert!(resolve_task_inputs(&f.pool, &run, &m.target.plan, task)
        .await
        .is_err());
    assert!(
        output_for_predicate(&f.pool, &run, &m.target.plan, "proposal_current")
            .await
            .is_err()
    );
}

#[tokio::test]
async fn installed_input_tamper_and_reference_alias_cannot_become_normal_input() {
    use engine::run_carry_forward::inputs::resolve_task_inputs;
    let (f, m, run) = activated().await;
    let task = &m.target.plan.states["state_4_proposal_reviewed"].tasks[0];
    let reference = m
        .inputs
        .iter()
        .find(|i| i.role == EntryRole::ReferenceOnly)
        .unwrap();
    sqlx::query("UPDATE run_continuation_inputs SET target_relative_path='proposals/current/proposal.md' WHERE input_id=?")
        .bind(reference.input_id.to_string()).execute(&f.pool).await.expect_err("unique operation path");
    sqlx::query("UPDATE run_continuation_inputs SET source_schema='unknown' WHERE input_id=?")
        .bind(reference.input_id.to_string())
        .execute(&f.pool)
        .await
        .unwrap();
    assert!(resolve_task_inputs(&f.pool, &run, &m.target.plan, task)
        .await
        .is_err());
}

#[tokio::test]
async fn missing_uninstalled_changed_and_symlinked_seed_fail_closed() {
    use engine::run_carry_forward::inputs::{resolve_execution_input, InputOrigin};
    let (f, m, run) = activated().await;
    let seed = m
        .inputs
        .iter()
        .find(|i| i.role == EntryRole::ExecutionSeed)
        .unwrap();
    let path = m.workspace.metadata_root.join(&seed.target_relative_path);
    let original = fs::read(&path).unwrap();
    let selected = resolve_execution_input(&f.pool, &run, &m.target.plan, "proposal_current")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(selected.origin, InputOrigin::ExecutionSeed);
    assert_eq!(selected.content.as_bytes(), original);
    assert!(
        resolve_execution_input(&f.pool, &run, &m.target.plan, "audit_report")
            .await
            .unwrap()
            .is_none()
    );
    let mut wrong_root = run.clone();
    wrong_root.chainworks_meta_root = Some(f.metadata.to_string_lossy().into_owned());
    assert!(
        resolve_execution_input(&f.pool, &wrong_root, &m.target.plan, "proposal_current")
            .await
            .is_err()
    );
    fs::write(&path, "changed without a validated new output").unwrap();
    assert!(
        resolve_execution_input(&f.pool, &run, &m.target.plan, "proposal_current")
            .await
            .is_err()
    );
    fs::remove_file(&path).unwrap();
    assert!(
        resolve_execution_input(&f.pool, &run, &m.target.plan, "proposal_current")
            .await
            .is_err()
    );
    let outside = f.owned.join("outside-seed.txt");
    fs::write(&outside, &original).unwrap();
    std::os::unix::fs::symlink(&outside, &path).unwrap();
    assert!(
        resolve_execution_input(&f.pool, &run, &m.target.plan, "proposal_current")
            .await
            .is_err()
    );
    fs::remove_file(&path).unwrap();
    fs::write(&path, &original).unwrap();
    sqlx::query(
        "UPDATE run_continuation_inputs SET installed=0,successor_run_id=NULL WHERE input_id=?",
    )
    .bind(seed.input_id.to_string())
    .execute(&f.pool)
    .await
    .unwrap();
    assert!(
        resolve_execution_input(&f.pool, &run, &m.target.plan, "proposal_current")
            .await
            .is_err()
    );
}

#[tokio::test]
async fn requested_valid_new_output_need_not_be_an_installed_logical_name() {
    use engine::run_carry_forward::inputs::{resolve_task_inputs, InputOrigin};
    let (f, m, run) = activated().await;
    assert!(!m
        .inputs
        .iter()
        .any(|i| i.target_logical_name == "implementation_plan"));
    let path = m.workspace.metadata_root.join("implementation/plan.md");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let bytes = b"# Current implementation plan\nValidated in the successor.\n";
    fs::write(&path, bytes).unwrap();
    let id = provider_artifact(&f, &run, &path, bytes, true).await;
    sqlx::query("UPDATE artifacts SET name='implementation_plan',contract_id='implementation_plan' WHERE id=?")
        .bind(id).execute(&f.pool).await.unwrap();
    let mut task = m.target.plan.states["state_7_implementation_started"].tasks[0].clone();
    task.inputs = vec!["implementation_plan".into(), "approved_proposal".into()];
    let context = resolve_task_inputs(&f.pool, &run, &m.target.plan, &task)
        .await
        .unwrap()
        .unwrap();
    let input = context.inputs["implementation_plan"].as_ref().unwrap();
    assert_eq!(input.origin, InputOrigin::ProviderOutput);
    assert_eq!(input.content.as_bytes(), bytes);
    assert!(context.inputs["approved_proposal"].is_none());
}

#[tokio::test]
async fn mandatory_reference_content_is_hash_bound_and_never_silently_omitted() {
    use engine::run_carry_forward::inputs::resolve_task_inputs;
    let (f, m, run) = activated().await;
    let task = &m.target.plan.states["state_4_proposal_reviewed"].tasks[0];
    let reference = m
        .inputs
        .iter()
        .find(|i| i.role == EntryRole::ReferenceOnly)
        .unwrap();
    fs::write(
        m.workspace
            .metadata_root
            .join(&reference.target_relative_path),
        "findings removed",
    )
    .unwrap();
    assert!(resolve_task_inputs(&f.pool, &run, &m.target.plan, task)
        .await
        .is_err());
}
