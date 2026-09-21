use domain::{ids::RunId, run::DeliveryConfiguration, run_carry_forward::EntryRole};
use engine::run_carry_forward::preview::{
    preview, InputReference, PreviewError, PreviewInput, PreviewOutcome, PreviewSelection,
    PreviewTarget,
};
use serde_json::{json, Value};
use sqlx::SqlitePool;
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

const PREPARE: &str = "state_7_implementation_started";

#[tokio::test]
async fn legacy_unbound_approval_remains_unproven_historical_context() {
    use domain::run_carry_forward_api::{CarryForwardEntryV1, HistoricalApprovalRelation};
    let f = Fixture::new().await;
    let before = tree(&f.root);
    let (_, prepared) = f.page().await;
    let seed = prepared
        .inputs()
        .iter()
        .find(|input| input.role == EntryRole::ExecutionSeed)
        .unwrap();
    assert_eq!(
        seed.historical_approval_relation,
        json!({"authority":"none","binding":"unproven","fresh_review_required":true})
    );
    let entry = prepared
        .plan()
        .entries
        .iter()
        .find(|entry| entry.role == EntryRole::ExecutionSeed)
        .unwrap();
    let wire = engine::run_carry_forward::preview::wire_manifest_entry(
        &serde_json::to_value(entry).unwrap(),
    )
    .unwrap();
    let CarryForwardEntryV1::ExecutionSeed(wire) = wire else {
        panic!("execution seed diagnostic missing");
    };
    assert_eq!(
        wire.historical_approval_relation,
        HistoricalApprovalRelation::HistoricalBindingUnproven
    );
    assert_eq!(
        prepared.target().plan.initial_state,
        "state_4_proposal_reviewed"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM approvals")
            .fetch_one(&f.pool)
            .await
            .unwrap(),
        1
    );
    assert_eq!(sqlx::query_scalar::<_,i64>("SELECT (SELECT COUNT(*) FROM run_continuation_approval_bindings)+(SELECT COUNT(*) FROM run_continuations)").fetch_one(&f.pool).await.unwrap(),0);
    assert_eq!(before, tree(&f.root));
}

#[tokio::test]
async fn reserved_source_current_target_check_is_read_only_and_rejects_drift() {
    use domain::run_carry_forward::canonical_digest;
    use engine::run_carry_forward::{manifest::ManifestV1, preview::verify_current_target};
    let f = Fixture::new().await;
    let (_, prepared) = f.page().await;
    let source = db::repos::runs::find_by_id(&f.pool, f.input.source_run_id)
        .await
        .unwrap()
        .unwrap();
    let plan = serde_json::to_value(prepared.plan()).unwrap();
    let operation = uuid::Uuid::new_v4();
    let successor = RunId::new();
    let journal = uuid::Uuid::new_v4();
    sqlx::query("INSERT INTO command_journal(id,command_type,payload_json,run_id,created_at) VALUES (?,'ContinueBlocked','{}',?,'2026-09-20')")
        .bind(journal.to_string()).bind(source.id.to_string()).execute(&f.pool).await.unwrap();
    sqlx::query("INSERT INTO run_continuations(operation_id,source_run_id,idea_id,reserved_successor_run_id,caller_fingerprint,caller_request_id,intent_sha256,profile,plan_ref,plan_sha256,target_ref,source_witness_sha256,journal_id,created_at,updated_at,deadline_at) VALUES (?,?,?,?,'fixture',?,?,'implementation_restart_v1','plan',?,'target',?,?,'2026-09-20','2026-09-20','2026-09-21')")
        .bind(operation.to_string()).bind(source.id.to_string()).bind(source.idea_id.to_string()).bind(successor.to_string()).bind(uuid::Uuid::new_v4().to_string())
        .bind("a".repeat(64)).bind("b".repeat(64)).bind("c".repeat(64)).bind(journal.to_string()).execute(&f.pool).await.unwrap();
    sqlx::query("INSERT INTO run_execution_fences(source_run_id,operation_id,generation,disposition,created_at) VALUES (?,?,1,'reserved','2026-09-20')")
        .bind(source.id.to_string()).bind(operation.to_string()).execute(&f.pool).await.unwrap();
    assert!(matches!(
        preview(&f.pool, f.input.clone()).await.unwrap(),
        PreviewOutcome::Held { .. }
    ));
    // Target-only fixture: publication/manifest authentication is separately owned.
    let manifest: ManifestV1 = serde_json::from_value(json!({
        "schema_version":"run_carry_forward_manifest_v1", "operation_id":operation,
        "source_run_id":source.id,"reserved_successor_run_id":successor,"idea_id":source.idea_id,
        "plan_sha256":prepared.plan().plan_sha256,"source_witness_sha256":canonical_digest(&plan["source_witness"]).unwrap(),
        "source_semantic_sha256":prepared.source_witness().semantic_sha256,"plan":plan,"target":prepared.target(),
        "target_sha256":canonical_digest(prepared.target()).unwrap(),"original_workflow_sha256":prepared.plan().summary.target.workflow_snapshot_hash,
        "original_catalog_sha256":prepared.plan().summary.target.catalog_snapshot_hash,"definition_files_sha256":prepared.plan().summary.target.definition_files_sha256,
        "capability_delta_sha256":canonical_digest(&prepared.plan().summary.capability_delta).unwrap(),"unresolved_findings_sha256":canonical_digest(&json!([])).unwrap(),"preserved_bytes":0,
        "workspace":{"operation_root":f.root.parent().unwrap().join("not-created"),"source_root":f.root,"checkout_root":f.root,"preservation_root":f.root,"metadata_root":f.root,
            "head":prepared.workspace().head(),"pin_ref":"unused","source_witness_sha256":prepared.workspace().digest(),"preservation_manifest_sha256":prepared.workspace().digest(),"directory_identities":{},
            "checkout_index_sha256":domain::run_carry_forward::ContentDigest::of(&fs::read(f.root.join(".git/index")).unwrap())},
        "files":[],"inputs":[],"input_provenance":[],"successor":source,"verified_at":"2026-09-20"
    })).unwrap();
    let before = tree(f.root.parent().unwrap());
    let changes: i64 = sqlx::query_scalar("SELECT total_changes()")
        .fetch_one(&f.pool)
        .await
        .unwrap();
    verify_current_target(&f.pool, &manifest).await.unwrap();
    assert_eq!(
        changes,
        sqlx::query_scalar::<_, i64>("SELECT total_changes()")
            .fetch_one(&f.pool)
            .await
            .unwrap()
    );
    assert_eq!(before, tree(f.root.parent().unwrap()));
    let mut corrupt = manifest.clone();
    corrupt.target_sha256 = domain::run_carry_forward::ContentDigest::of(b"not the derived target");
    assert!(verify_current_target(&f.pool, &corrupt).await.is_err());
    let path = f.root.join("examples/workflows/current.yaml");
    let mut bytes = fs::read(&path).unwrap();
    bytes.extend_from_slice(b"\n# byte drift only\n");
    fs::write(path, bytes).unwrap();
    assert!(verify_current_target(&f.pool, &manifest).await.is_err());
}

#[tokio::test]
async fn manifest_entry_converter_matches_preview_and_rejects_unknown_fields() {
    use engine::run_carry_forward::preview::{wire_manifest_entry, wire_page};
    let f = Fixture::new().await;
    let (page, prepared) = f.page().await;
    let wire = wire_page(&page, &prepared).unwrap();
    for (entry, expected) in page.entries.iter().zip(wire.entries.as_slice()) {
        let value = serde_json::to_value(entry).unwrap();
        assert_eq!(wire_manifest_entry(&value).unwrap(), *expected);
        let mut bad = value.clone();
        bad["destination"] = json!("/not-authorized");
        assert!(wire_manifest_entry(&bad).is_err());
        let mut bad = value;
        bad["key"]["relative_path"] = json!("../not-authorized");
        assert!(wire_manifest_entry(&bad).is_err());
    }
}

fn git(root: &Path, args: &[&str]) {
    let output = Command::new("/usr/bin/git")
        .current_dir(root)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .args(["-c", "core.hooksPath=/dev/null"])
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
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
    pool: SqlitePool,
    input: PreviewInput,
}

impl Fixture {
    async fn new() -> Self {
        Self::new_with_database(false).await
    }

    async fn new_with_database(file_backed: bool) -> Self {
        let temp = tempfile::tempdir().unwrap();
        let root = fs::canonicalize(temp.path()).unwrap().join("repo");
        fs::create_dir(&root).unwrap();
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
            "preparation_state":PREPARE, "proposal_input":"proposal_current",
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
        let database_url = if file_backed {
            format!(
                "sqlite://{}?mode=rwc",
                root.parent().unwrap().join("reopened.sqlite").display()
            )
        } else {
            "sqlite::memory:".into()
        };
        let pool = db::pool::create_pool(&database_url).await.unwrap();
        let idea = uuid::Uuid::new_v4().to_string();
        let run = RunId::new();
        let meta = format!(".chainworks/runs/{run}");
        fs::create_dir_all(root.join(&meta).join("proposals/current")).unwrap();
        let proposal = root.join(&meta).join("proposals/current/proposal.md");
        fs::write(&proposal, "# Proposal\nPreserve current work.\n").unwrap();
        let delivery = DeliveryConfiguration {
            repo_identifier: "fixture-repository".into(),
            repo_root: root.to_str().unwrap().into(),
            base_branch: "HEAD".into(),
            worktree_base_path: root
                .parent()
                .unwrap()
                .join("not-created")
                .to_str()
                .unwrap()
                .into(),
            target_branch: "fixture-delivery".into(),
            release_target_id: Some("fixture".into()),
            release_mode: Some("sandbox".into()),
        };
        sqlx::query("INSERT INTO ideas(id,title,body,workspace_root_path,created_at) VALUES (?,'Source','Body',?,'2026-09-20T00:00:00Z')")
            .bind(&idea).bind(root.to_str().unwrap()).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO runs(id,idea_id,status,workflow_id,workflow_title,workspace_root,artifact_root,started_at,current_state,worktree_root,chainworks_meta_root,workflow_snapshot_json,catalog_snapshot_json,workflow_snapshot_hash,catalog_snapshot_hash,delivery_configuration_json,agent_catalog_yaml_path) VALUES (?,?,'blocked','proposal_to_release','Workflow',?,?,'2026-09-20T00:00:00Z',?,?,?,?,?,?,?,?,?)")
            .bind(run.to_string()).bind(&idea).bind(root.to_str().unwrap()).bind(root.join(&meta).to_str().unwrap())
            .bind(PREPARE).bind(root.to_str().unwrap()).bind(&meta).bind(old.workflow_snapshot_json).bind(old.catalog_snapshot_json)
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
        sqlx::query("INSERT INTO stage_executions(id,run_id,stage_id,label,status,iteration,attempt_number,started_at) VALUES (?,?,?,'Implementation','blocked',0,1,'2026-09-20T00:00:00Z')")
            .bind(uuid::Uuid::new_v4().to_string()).bind(run.to_string()).bind(PREPARE).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO approvals(id,run_id,stage_id,decision,requested_at,decided_at) VALUES (?,?,'state_6_implementation_approval','granted','2026-09-19T00:00:00Z','2026-09-19T01:00:00Z')")
            .bind(uuid::Uuid::new_v4().to_string()).bind(run.to_string()).execute(&pool).await.unwrap();
        let proposal_id = uuid::Uuid::new_v4();
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
                proposal_input: InputReference::Artifact { id: proposal_id },
                reference_inputs: vec![],
                include_dirty_work: true,
                excluded_workspace_paths: BTreeMap::new(),
            },
            profile: "implementation_restart_v1".into(),
            cursor: None,
            limit: Some(2),
        };
        let fixture = Self {
            _temp: temp,
            root,
            pool,
            input,
        };
        fixture
            .artifact(
                proposal_id,
                "proposal_current",
                "proposal_current",
                &proposal,
                b"# Proposal\nPreserve current work.\n",
            )
            .await;
        fixture
    }

    async fn artifact(
        &self,
        id: uuid::Uuid,
        name: &str,
        contract: &str,
        path: &Path,
        bytes: &[u8],
    ) {
        use sha2::{Digest, Sha256};
        sqlx::query("INSERT INTO artifacts(id,run_id,stage_id,agent_id,name,contract_id,format,file_path,checksum_sha256,size_bytes,provider,created_at) VALUES (?,?,'state_3_proposal_created','proposal_writer',?,?,'markdown',?,?,?,'fixture','2026-09-20T00:00:00Z')")
            .bind(id.to_string()).bind(self.input.source_run_id.to_string()).bind(name).bind(contract).bind(path.to_str().unwrap())
            .bind(format!("{:x}", Sha256::digest(bytes))).bind(bytes.len() as i64).execute(&self.pool).await.unwrap();
    }

    async fn page(
        &self,
    ) -> (
        engine::run_carry_forward::preview::PreviewPage,
        engine::run_carry_forward::preview::PreparedPlan,
    ) {
        match preview(&self.pool, self.input.clone()).await.unwrap() {
            PreviewOutcome::Page { page, prepared } => (page, prepared),
            result => panic!("expected inventory page, got {result:?}"),
        }
    }

    async fn frozen_plan(&self) -> workflow::plan::RunPlan {
        let run = db::repos::runs::find_by_id(&self.pool, self.input.source_run_id)
            .await
            .unwrap()
            .unwrap();
        workflow::compiler::compile_from_snapshot_json(
            run.workflow_snapshot_json.as_deref().unwrap(),
            run.catalog_snapshot_json.as_deref().unwrap(),
            run.agent_catalog_yaml_path.as_deref().unwrap(),
        )
        .unwrap()
    }

    async fn produced(
        &self,
        name: &str,
        stage_id: &str,
        bytes: &[u8],
        checksum: bool,
    ) -> uuid::Uuid {
        let plan = self.frozen_plan().await;
        let task = plan.states[stage_id]
            .tasks
            .iter()
            .find(|t| t.outputs.iter().any(|o| o == name))
            .unwrap();
        let relative = plan.artifact_paths[name]
            .strip_prefix("${CHAINWORKS_META_ROOT:-.chainworks}/")
            .unwrap();
        let path = self
            .root
            .join(format!(".chainworks/runs/{}", self.input.source_run_id))
            .join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, bytes).unwrap();
        let id = uuid::Uuid::new_v4();
        let contract = task
            .output_schemas
            .get(name)
            .map(|s| s.contract_id.clone())
            .unwrap_or_else(|| format!("{}.output", task.agent.provider));
        self.artifact(id, name, &contract, &path, bytes).await;
        self.bind_producer(id, stage_id, &task.agent.agent_id, &task.agent.provider, &task.task_name,
            json!([{"output_name": name,"target_path":path,"schema":task.output_schemas.get(name),"companion_output_name":null,"companion_path":null}]), None).await;
        if task
            .outputs
            .iter()
            .any(|output| !task.output_schemas.contains_key(output))
        {
            sqlx::query("UPDATE agent_execution_runtime_facts SET valid_required_outputs=0 WHERE agent_execution_id=(SELECT agent_execution_id FROM artifacts WHERE id=?)")
                .bind(id.to_string()).execute(&self.pool).await.unwrap();
        }
        if !checksum {
            sqlx::query("UPDATE artifacts SET checksum_sha256=NULL WHERE id=?")
                .bind(id.to_string())
                .execute(&self.pool)
                .await
                .unwrap();
        }
        id
    }

    async fn bind_producer(
        &self,
        id: uuid::Uuid,
        stage_id: &str,
        agent: &str,
        provider: &str,
        task: &str,
        outputs: Value,
        dynamic: Option<Value>,
    ) {
        let stage = if stage_id == PREPARE {
            sqlx::query_scalar::<_, String>(
                "SELECT id FROM stage_executions WHERE run_id=? AND stage_id=?",
            )
            .bind(self.input.source_run_id.to_string())
            .bind(stage_id)
            .fetch_one(&self.pool)
            .await
            .unwrap()
        } else {
            uuid::Uuid::new_v4().to_string()
        };
        let execution = uuid::Uuid::new_v4().to_string();
        let work = uuid::Uuid::new_v4().to_string();
        sqlx::query("INSERT OR IGNORE INTO stage_executions(id,run_id,stage_id,label,status,iteration,attempt_number,started_at,completed_at) VALUES (?,?,?,'Producer','completed',0,1,'2026-09-18T00:00:00Z','2026-09-18T01:00:00Z')")
            .bind(&stage).bind(self.input.source_run_id.to_string()).bind(stage_id).execute(&self.pool).await.unwrap();
        sqlx::query("INSERT INTO agent_executions(id,stage_execution_id,agent_id,provider,status,started_at,completed_at,owner_kind,owner_id) VALUES (?,?,?,?,'completed','2026-09-18T00:00:00Z','2026-09-18T01:00:00Z','stage_execution',?)")
            .bind(&execution).bind(&stage).bind(agent).bind(provider).bind(&stage).execute(&self.pool).await.unwrap();
        sqlx::query("INSERT INTO agent_execution_runtime_facts(agent_execution_id,output_settlement,valid_required_outputs,created_at,updated_at) VALUES (?,'valid_outputs_from_completed_execution',1,'2026-09-18','2026-09-18')")
            .bind(&execution).execute(&self.pool).await.unwrap();
        let mut payload = json!({"run_id":self.input.source_run_id,"stage_id":stage_id,"stage_execution_id":stage,"agent_id":agent,"provider":provider,"task_name":task,"declared_outputs":outputs,"p058_claimed":{"agent_execution_id":execution}});
        if let Some(dynamic) = dynamic {
            payload
                .as_object_mut()
                .unwrap()
                .extend(dynamic.as_object().unwrap().clone());
        }
        sqlx::query("INSERT INTO work_items(id,kind,payload_json,status,run_id,stage_id,created_at,scheduled_at) VALUES (?,'invoke_agent',?,'completed',?,?,'2026-09-18','2026-09-18')")
            .bind(&work).bind(payload.to_string()).bind(self.input.source_run_id.to_string()).bind(stage_id).execute(&self.pool).await.unwrap();
        sqlx::query(
            "UPDATE artifacts SET stage_id=?,agent_id=?,provider=?,agent_execution_id=? WHERE id=?",
        )
        .bind(stage_id)
        .bind(agent)
        .bind(provider)
        .bind(execution)
        .bind(id.to_string())
        .execute(&self.pool)
        .await
        .unwrap();
    }
}

#[tokio::test]
async fn plain_proposal_uses_frozen_producer_identity_with_or_without_historical_checksum() {
    for checksum in [true, false] {
        let mut f = Fixture::new().await;
        sqlx::query("DELETE FROM artifacts")
            .execute(&f.pool)
            .await
            .unwrap();
        let id = f
            .produced(
                "proposal_current",
                "state_2_proposal_drafted",
                b"# Ordinary proposal\n",
                checksum,
            )
            .await;
        f.input.selection.proposal_input = InputReference::Artifact { id };
        let before = tree(&f.root);
        let (_, prepared) = f.page().await;
        let seed = &prepared.inputs()[0];
        assert_eq!(seed.original_artifact_id, id);
        assert!(seed.source_contract.ends_with(".output"));
        assert_eq!(seed.role, EntryRole::ExecutionSeed);
        assert_eq!(seed.historical_approval_relation["binding"], "unproven");
        assert_eq!(
            seed.historical_approval_relation["fresh_review_required"],
            true
        );
        assert_eq!(
            seed.historical_approval_relation["provenance"]["content_binding"],
            if checksum {
                "historical_checksum"
            } else {
                "fresh_observation_only"
            }
        );
        assert_eq!(
            sqlx::query_scalar::<_, Option<String>>(
                "SELECT checksum_sha256 FROM artifacts WHERE id=?"
            )
            .bind(id.to_string())
            .fetch_one(&f.pool)
            .await
            .unwrap()
            .is_some(),
            checksum
        );
        assert_eq!(before, tree(&f.root));
    }
}

#[tokio::test]
async fn plain_output_logical_alias_does_not_replace_execution_and_declared_output_proof() {
    for tamper in [
        "missing_execution",
        "wrong_task",
        "wrong_declared_path",
        "unsettled",
        "wrong_provider",
    ] {
        let mut f = Fixture::new().await;
        sqlx::query("DELETE FROM artifacts")
            .execute(&f.pool)
            .await
            .unwrap();
        let id = f
            .produced(
                "proposal_current",
                "state_2_proposal_drafted",
                b"# Ordinary proposal\n",
                false,
            )
            .await;
        f.input.selection.proposal_input = InputReference::Artifact { id };
        match tamper {
            "missing_execution" => {
                sqlx::query("UPDATE artifacts SET agent_execution_id=NULL")
                    .execute(&f.pool)
                    .await
                    .unwrap();
            }
            "wrong_task" => {
                sqlx::query("UPDATE work_items SET payload_json=json_set(payload_json,'$.task_name','invented')").execute(&f.pool).await.unwrap();
            }
            "wrong_declared_path" => {
                sqlx::query("UPDATE work_items SET payload_json=json_set(payload_json,'$.declared_outputs[0].target_path','/tmp/unbound.md')").execute(&f.pool).await.unwrap();
            }
            "unsettled" => {
                sqlx::query("UPDATE agent_execution_runtime_facts SET output_settlement='none'")
                    .execute(&f.pool)
                    .await
                    .unwrap();
            }
            _ => {
                sqlx::query("UPDATE artifacts SET provider='invented'")
                    .execute(&f.pool)
                    .await
                    .unwrap();
            }
        }
        assert!(
            matches!(
                preview(&f.pool, f.input.clone()).await.unwrap(),
                PreviewOutcome::Held { .. }
            ),
            "accepted {tamper}"
        );
    }
}

#[tokio::test]
async fn historical_implementation_grant_is_required() {
    for decision in [None, Some("rejected"), Some("expired")] {
        let f = Fixture::new().await;
        if let Some(decision) = decision {
            sqlx::query("UPDATE approvals SET decision=?")
                .bind(decision)
                .execute(&f.pool)
                .await
                .unwrap();
        } else {
            sqlx::query("DELETE FROM approvals")
                .execute(&f.pool)
                .await
                .unwrap();
        }
        assert!(
            matches!(
                preview(&f.pool, f.input.clone()).await.unwrap(),
                PreviewOutcome::Held { .. }
            ),
            "missing historical grant accepted: {decision:?}"
        );
    }
}

#[tokio::test]
async fn exact_human_and_decomposition_schemas_remain_mandatory_historical_references() {
    let f = Fixture::new().await;
    let plan = f.frozen_plan().await;
    let cases = [
        (
            "blocker_boundary_human_decision",
            json!({"schema_version":"blocker_boundary_human_decision_v1","decision_label":"followup_proposal","notes":"unresolved human boundary"}),
        ),
        (
            "proposal_decomposition_plan",
            json!({"schema_version":"proposal_decomposition_plan_v1","requires_split":true,"implementation_start_decision":"split_before_implementation","children":[{"title":"Preserve this scope"}]}),
        ),
        (
            "quality_gate_blocker_assessment",
            json!({"schema_version":"quality_gate_blocker_assessment_v1","blockers":[]}),
        ),
        (
            "blocker_boundary_status",
            json!({"schema_version":"blocker_boundary_status_v1","status":"blocked","assessment_generation_id":"generation-1","projection_integrity":"valid","followup_proposal_required":true,"has_release_blocking_external_blockers":true,"has_no_release_blocking_external_blockers":false,"workflow_route_hint":"human_boundary"}),
        ),
        (
            "blocker_boundary_approval_request",
            json!({"schema_version":"blocker_boundary_approval_request_v1","question":"How should this blocker be handled?","allowed_decisions":["followup_proposal"],"label_to_approval_state":{"followup_proposal":"granted"}}),
        ),
        (
            "followup_proposal_seed",
            json!({"schema_version":"followup_proposal_seed_v1","title":"Follow-up","problem":"Unresolved boundary","proposed_scope":"Separate change"}),
        ),
    ];
    for (name, value) in cases {
        let path = f
            .root
            .join(format!(".chainworks/runs/{}", f.input.source_run_id))
            .join(
                plan.artifact_paths[name]
                    .strip_prefix("${CHAINWORKS_META_ROOT:-.chainworks}/")
                    .unwrap(),
            );
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let bytes = serde_json::to_vec(&value).unwrap();
        fs::write(&path, &bytes).unwrap();
        f.artifact(
            uuid::Uuid::new_v4(),
            name,
            value["schema_version"].as_str().unwrap(),
            &path,
            &bytes,
        )
        .await;
    }
    let before = tree(&f.root);
    let (_, prepared) = f.page().await;
    assert_eq!(prepared.inputs().len(), 7);
    assert_eq!(
        prepared
            .inputs()
            .iter()
            .filter(|i| i.mandatory && i.role == EntryRole::ReferenceOnly)
            .count(),
        6
    );
    assert_eq!(before, tree(&f.root));
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM active_artifact_contracts")
            .fetch_one(&f.pool)
            .await
            .unwrap(),
        0
    );
}

// Literal persisted P095 shape; no test reads the operator's live run directory.
const HISTORICAL_BACKLOG: &[u8] = b"schema_version: implementation_backlog_v1\nproposal_revision_id: p095-refined-v16\nstatus: ready_for_implementation\nbacklog_items:\n  - id: P095-IMPL-001\n    title: Durable causal ordinals\n    owner_class: reliability\n    description: Preserve event ordering across restarts.\n    state: open\n    extra_context: do not drop unresolved findings\n";

#[tokio::test]
async fn reopened_reader_uses_original_frozen_human_schema_not_current_target_catalog() {
    let mut f = Fixture::new_with_database(true).await;
    let path = f.root.join(format!(
        ".chainworks/runs/{}/quality-gate/blocker-boundary-human-decision.json",
        f.input.source_run_id
    ));
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let bytes = br#"{"schema_version":"blocker_boundary_human_decision_v1","decision_label":"followup_proposal","notes":"Keep unresolved boundary"}"#;
    fs::write(&path, bytes).unwrap();
    let id = uuid::Uuid::new_v4();
    f.artifact(
        id,
        "blocker_boundary_human_decision",
        "blocker_boundary_human_decision_v1",
        &path,
        bytes,
    )
    .await;
    let (_, original) = f.page().await;
    let old = original
        .inputs()
        .iter()
        .find(|input| input.original_artifact_id == id)
        .unwrap();
    let catalog = f.root.join("examples/agents/agents.yaml");
    let mut current: Value = serde_yaml::from_slice(&fs::read(&catalog).unwrap()).unwrap();
    current["contracts"]
        .as_object_mut()
        .unwrap()
        .remove("blocker_boundary_human_decision_v1");
    fs::write(&catalog, serde_yaml::to_string(&current).unwrap()).unwrap();
    let database = f.root.parent().unwrap().join("reopened.sqlite");
    f.pool.close().await;
    f.pool = db::pool::create_pool(&format!("sqlite://{}?mode=rwc", database.display()))
        .await
        .unwrap();
    let (_, reopened) = f.page().await;
    let input = reopened
        .inputs()
        .iter()
        .find(|input| input.original_artifact_id == id)
        .unwrap();
    assert_eq!(input.sha256, old.sha256);
    assert_eq!(input.source_contract, old.source_contract);
    assert_eq!(fs::read(input.source_path()).unwrap(), bytes);
    assert_eq!(input.role, EntryRole::ReferenceOnly);
    let frozen = f.frozen_plan().await;
    let frozen: Value = serde_json::from_str(&frozen.catalog_snapshot_json).unwrap();
    assert_eq!(
        frozen["contracts"]["blocker_boundary_human_decision_v1"]["required_fields"],
        json!(["schema_version", "decision_label"])
    );
}

#[tokio::test]
async fn p095_yaml_backlog_and_plan_are_reference_only_without_invented_frozen_contracts() {
    for identity in [
        None,
        Some("implementation_backlog"),
        Some("implementation_backlog_v1"),
    ] {
        let f = Fixture::new().await;
        let id = f
            .produced("implementation_backlog", PREPARE, HISTORICAL_BACKLOG, false)
            .await;
        if let Some(identity) = identity {
            sqlx::query("UPDATE artifacts SET contract_id=? WHERE id=?")
                .bind(identity)
                .bind(id.to_string())
                .execute(&f.pool)
                .await
                .unwrap();
        }
        f.produced(
            "implementation_plan",
            PREPARE,
            b"# Historical plan\n",
            false,
        )
        .await;
        let before = tree(&f.root);
        let (_, prepared) = f.page().await;
        let backlog = prepared
            .inputs()
            .iter()
            .find(|i| i.original_artifact_id == id)
            .unwrap();
        assert_eq!(backlog.role, EntryRole::ReferenceOnly);
        assert!(backlog.mandatory);
        assert_eq!(
            backlog.source_schema_version,
            "legacy_implementation_backlog_v1"
        );
        assert_eq!(fs::read(backlog.source_path()).unwrap(), HISTORICAL_BACKLOG);
        assert!(
            prepared
                .inputs()
                .iter()
                .any(|i| i.logical_name == "implementation_plan"
                    && i.role == EntryRole::ReferenceOnly)
        );
        assert_eq!(
            sqlx::query_scalar::<_, Option<String>>(
                "SELECT checksum_sha256 FROM artifacts WHERE id=?"
            )
            .bind(id.to_string())
            .fetch_one(&f.pool)
            .await
            .unwrap(),
            None
        );
        assert_eq!(before, tree(&f.root));
    }
}

#[tokio::test]
async fn unknown_or_malformed_backlog_does_not_gain_reference_eligibility() {
    for bytes in [
        b"schema_version: implementation_backlog_v2\nbacklog_items: []\n".as_slice(),
        b"schema_version: implementation_backlog_v1\nproposal_revision_id: v1\nstatus: ready_for_implementation\nbacklog_items: [{id: 1}]\n".as_slice(),
    ] {
        let f = Fixture::new().await;
        f.produced("implementation_backlog", PREPARE, bytes, true).await;
        assert!(matches!(preview(&f.pool, f.input.clone()).await.unwrap(), PreviewOutcome::Held { .. }));
    }
}

fn review_summary(finding: &str) -> Vec<u8> {
    serde_json::to_vec(&json!({"schema_version":"proposal_review_summary_v2","pass":false,
        "average_score":7.0,"aggregate_score":7.0,"min_individual_score":7.0,"blocker_count":1,
        "blocking_issues":[finding],"summary":"Unresolved finding","blocking_required_changes":[finding],
        "advisory_follow_ups":[],"recurring_themes":[],"decision":"revise"})).unwrap()
}

async fn review_generation(
    f: &Fixture,
    id: uuid::Uuid,
    raw_path: &Path,
    supersedes: Option<&str>,
    active: bool,
) -> String {
    let generation = uuid::Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO artifact_contract_generations(generation_id,run_id,artifact_id,contract_id,canonical_path,raw_path,raw_status,canonical_status,source_agent_execution_id,source_stage_execution_id,source_work_item_id,valid,source_generation_verified,output_settlement,supersedes_generation_id,created_at) SELECT ?,a.run_id,a.id,a.contract_id,a.name,?,'revise','revise',e.id,e.stage_execution_id,w.id,1,1,'valid_outputs_from_completed_execution',?,'2026-09-18' FROM artifacts a JOIN agent_executions e ON e.id=a.agent_execution_id JOIN work_items w ON json_extract(w.payload_json,'$.p058_claimed.agent_execution_id')=e.id WHERE a.id=?")
        .bind(&generation).bind(raw_path.to_str().unwrap()).bind(supersedes).bind(id.to_string()).execute(&f.pool).await.unwrap();
    if active {
        sqlx::query("INSERT INTO active_artifact_contracts(run_id,contract_id,generation_id,updated_at) SELECT run_id,contract_id,?,'2026-09-18' FROM artifacts WHERE id=?")
            .bind(&generation).bind(id.to_string()).execute(&f.pool).await.unwrap();
    }
    generation
}

#[tokio::test]
async fn repeated_review_generations_preserve_old_bytes_instead_of_relabeling_current_file() {
    let f = Fixture::new().await;
    let old_bytes = review_summary("OLD-UNRESOLVED");
    let current_bytes = review_summary("NEW-UNRESOLVED");
    let old = f
        .produced(
            "proposal_review_summary",
            "state_4_proposal_reviewed",
            &old_bytes,
            true,
        )
        .await;
    let metadata = f
        .root
        .join(format!(".chainworks/runs/{}", f.input.source_run_id));
    let archive = metadata.join("reviews/proposal/generations/first.json");
    fs::create_dir_all(archive.parent().unwrap()).unwrap();
    fs::write(&archive, &old_bytes).unwrap();
    let old_generation = review_generation(&f, old, &archive, None, false).await;
    let current = f
        .produced(
            "proposal_review_summary",
            "state_4_proposal_reviewed",
            &current_bytes,
            true,
        )
        .await;
    sqlx::query("UPDATE stage_executions SET iteration=1 WHERE id=(SELECT stage_execution_id FROM agent_executions WHERE id=(SELECT agent_execution_id FROM artifacts WHERE id=?))")
        .bind(current.to_string()).execute(&f.pool).await.unwrap();
    let canonical = metadata.join("reviews/proposal/summary.json");
    review_generation(&f, current, &canonical, Some(&old_generation), true).await;
    let before = tree(&f.root);
    let (_, prepared) = f.page().await;
    let old_input = prepared
        .inputs()
        .iter()
        .find(|input| input.original_artifact_id == old)
        .unwrap();
    let current_input = prepared
        .inputs()
        .iter()
        .find(|input| input.original_artifact_id == current)
        .unwrap();
    assert_eq!(fs::read(old_input.source_path()).unwrap(), old_bytes);
    assert_eq!(
        fs::read(current_input.source_path()).unwrap(),
        current_bytes
    );
    assert_eq!(
        old_input.historical_approval_relation["provenance"]["generation_selection"],
        "preserved_historical_generation"
    );
    assert_eq!(
        current_input.historical_approval_relation["provenance"]["generation_selection"],
        "verified_active_canonical_generation"
    );
    assert!(old_input.mandatory && current_input.mandatory);
    assert_eq!(before, tree(&f.root));
    fs::remove_file(&archive).unwrap();
    assert!(
        matches!(
            preview(&f.pool, f.input.clone()).await.unwrap(),
            PreviewOutcome::Held { .. }
        ),
        "missing historical findings must hold"
    );
}

#[tokio::test]
async fn repeated_review_without_immutable_history_remains_held() {
    let f = Fixture::new().await;
    let metadata = f
        .root
        .join(format!(".chainworks/runs/{}", f.input.source_run_id));
    let canonical = metadata.join("reviews/proposal/summary.json");
    let old = f
        .produced(
            "proposal_review_summary",
            "state_4_proposal_reviewed",
            &review_summary("OLD"),
            true,
        )
        .await;
    let generation = review_generation(&f, old, &canonical, None, false).await;
    let current = f
        .produced(
            "proposal_review_summary",
            "state_4_proposal_reviewed",
            &review_summary("NEW"),
            true,
        )
        .await;
    review_generation(&f, current, &canonical, Some(&generation), true).await;
    assert!(matches!(
        preview(&f.pool, f.input.clone()).await.unwrap(),
        PreviewOutcome::Held { .. }
    ));
}

#[tokio::test]
async fn multiple_plain_generations_cannot_rebind_old_checksumless_id_to_current_bytes() {
    let mut f = Fixture::new().await;
    sqlx::query("DELETE FROM artifacts")
        .execute(&f.pool)
        .await
        .unwrap();
    let old = f
        .produced(
            "proposal_current",
            "state_2_proposal_drafted",
            b"# Old revision\n",
            false,
        )
        .await;
    f.produced(
        "proposal_current",
        "state_2_proposal_drafted",
        b"# New revision\n",
        false,
    )
    .await;
    f.input.selection.proposal_input = InputReference::Artifact { id: old };
    assert!(matches!(
        preview(&f.pool, f.input.clone()).await.unwrap(),
        PreviewOutcome::Held { .. }
    ));
}

#[tokio::test]
async fn mixed_task_structured_reference_uses_per_output_generation_without_global_fact_repair() {
    let f = Fixture::new().await;
    let bytes = review_summary("KEEP-MIXED-FINDING");
    let id = f
        .produced(
            "proposal_review_summary",
            "state_4_proposal_reviewed",
            &bytes,
            false,
        )
        .await;
    let plan = f.frozen_plan().await;
    let task = plan.states["state_4_proposal_reviewed"]
        .tasks
        .iter()
        .find(|t| t.task_name == "aggregate_proposal_reviews")
        .unwrap();
    let metadata = f
        .root
        .join(format!(".chainworks/runs/{}", f.input.source_run_id));
    let mut outputs = Vec::new();
    for name in &task.outputs {
        let path = metadata.join(
            plan.artifact_paths[name]
                .strip_prefix("${CHAINWORKS_META_ROOT:-.chainworks}/")
                .unwrap(),
        );
        outputs.push(json!({"output_name":name,"target_path":path,"schema":task.output_schemas.get(name),"companion_output_name":null,"companion_path":null}));
        if name != "proposal_review_summary" {
            let sibling = uuid::Uuid::new_v4();
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            let sibling_bytes = match name.as_str() {
                "score_lift_backlog" => serde_json::to_vec(&json!({"review_pass_id":"pass-1","source_proposal_artifact":"proposal_current","items":[]})).unwrap(),
                "review_corpus_bundle_v2" => serde_json::to_vec(&json!({"selected_review_artifacts":[],"selected_reviewer_ids":[],"reviewer_count":0,"selection_plan_hash":"a".repeat(64),"selection_plan":{},"legacy_fixed_mode":false})).unwrap(),
                "proposal_fact_digest" => serde_json::to_vec(&json!({"proposal_revision_id":"revision-1","claims":[]})).unwrap(),
                _ => b"Historical sibling\n".to_vec(),
            };
            fs::write(&path, &sibling_bytes).unwrap();
            let contract = task
                .output_schemas
                .get(name)
                .map(|s| s.contract_id.clone())
                .unwrap_or_else(|| format!("{}.output", task.agent.provider));
            f.artifact(sibling, name, &contract, &path, &sibling_bytes)
                .await;
            sqlx::query("UPDATE artifacts SET stage_id='state_4_proposal_reviewed',agent_id=?,provider=?,agent_execution_id=(SELECT agent_execution_id FROM artifacts WHERE id=?) WHERE id=?")
                .bind(&task.agent.agent_id).bind(&task.agent.provider).bind(id.to_string()).bind(sibling.to_string()).execute(&f.pool).await.unwrap();
            sqlx::query("UPDATE artifacts SET checksum_sha256=NULL WHERE id=?")
                .bind(sibling.to_string())
                .execute(&f.pool)
                .await
                .unwrap();
            if domain::artifact_contracts::known_contract_id(&contract) {
                review_generation(&f, sibling, &path, None, true).await;
            }
        }
    }
    sqlx::query(
        "UPDATE work_items SET payload_json=json_set(payload_json,'$.declared_outputs',json(?))",
    )
    .bind(serde_json::to_string(&outputs).unwrap())
    .execute(&f.pool)
    .await
    .unwrap();
    sqlx::query("UPDATE agent_execution_runtime_facts SET valid_required_outputs=0")
        .execute(&f.pool)
        .await
        .unwrap();
    let canonical = metadata.join("reviews/proposal/summary.json");
    review_generation(&f, id, &canonical, None, true).await;
    let (_, prepared) = f.page().await;
    assert_eq!(
        fs::read(
            prepared
                .inputs()
                .iter()
                .find(|i| i.original_artifact_id == id)
                .unwrap()
                .source_path()
        )
        .unwrap(),
        bytes
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT valid_required_outputs FROM agent_execution_runtime_facts"
        )
        .fetch_one(&f.pool)
        .await
        .unwrap(),
        0
    );
    sqlx::query("UPDATE artifact_contract_generations SET valid=0")
        .execute(&f.pool)
        .await
        .unwrap();
    assert!(matches!(
        preview(&f.pool, f.input.clone()).await.unwrap(),
        PreviewOutcome::Held { .. }
    ));
    sqlx::query("UPDATE artifact_contract_generations SET valid=1")
        .execute(&f.pool)
        .await
        .unwrap();
    sqlx::query("UPDATE agent_execution_runtime_facts SET failure_kind='output_validation_failed',output_settlement='none'").execute(&f.pool).await.unwrap();
    sqlx::query("UPDATE agent_executions SET status='failed'")
        .execute(&f.pool)
        .await
        .unwrap();
    assert!(
        matches!(
            preview(&f.pool, f.input.clone()).await.unwrap(),
            PreviewOutcome::Held { .. }
        ),
        "required sibling failure accepted"
    );
}

#[tokio::test]
async fn historical_fallback_requires_exact_frozen_backend_and_claimed_dispatch() {
    use domain::agent::{AgentExecutionRuntimeFacts, AgentFailureKind, AgentOutputSettlement};
    use domain::ids::AgentExecutionId;
    let mut f = Fixture::new().await;
    sqlx::query("DELETE FROM artifacts")
        .execute(&f.pool)
        .await
        .unwrap();
    let plan = f.frozen_plan().await;
    let task = &plan.states["state_5_proposal_refined"].tasks[0];
    let catalog: Value = serde_json::from_str(&plan.catalog_snapshot_json).unwrap();
    let target = if matches!(task.agent.provider.as_str(), "codex" | "codex_acp") {
        "claude_writer_high"
    } else {
        "codex_writer_high"
    };
    let profile = &catalog["backend_profiles"][target];
    let provider = profile["provider"].as_str().unwrap();
    assert_ne!(provider, task.agent.provider);
    let id = f
        .produced(
            "proposal_current",
            "state_5_proposal_refined",
            b"# Historical fallback proposal\n",
            false,
        )
        .await;
    f.input.selection.proposal_input = InputReference::Artifact { id };
    let (execution, stage): (String, String) = sqlx::query_as("SELECT e.id,e.stage_execution_id FROM artifacts a JOIN agent_executions e ON e.id=a.agent_execution_id WHERE a.id=?")
        .bind(id.to_string()).fetch_one(&f.pool).await.unwrap();
    let (work, raw): (String, String) = sqlx::query_as("SELECT id,payload_json FROM work_items WHERE json_extract(payload_json,'$.p058_claimed.agent_execution_id')=?")
        .bind(&execution).fetch_one(&f.pool).await.unwrap();
    let failed = AgentExecutionId::new();
    sqlx::query("INSERT INTO agent_executions(id,stage_execution_id,agent_id,provider,status,started_at,completed_at,owner_kind,owner_id) VALUES (?,?,?,?,'failed','2026-09-17T00:00:00Z','2026-09-17T01:00:00Z','stage_execution',?)")
        .bind(failed.to_string()).bind(&stage).bind(&task.agent.agent_id).bind(&task.agent.provider).bind(&stage).execute(&f.pool).await.unwrap();
    let mut facts = AgentExecutionRuntimeFacts::defaults_for(failed, chrono::Utc::now());
    facts.failure_kind = Some(AgentFailureKind::MissingRequiredOutputs);
    facts.output_settlement = AgentOutputSettlement::MissingRequiredOutputs;
    db::repos::agent_execution_runtime_facts::upsert(&f.pool, &facts)
        .await
        .unwrap();
    let mut payload: Value = serde_json::from_str(&raw).unwrap();
    payload["provider"] = json!(provider);
    payload["task_outputs"] = json!(task.outputs);
    payload["backend_profile_id"] = json!(target);
    for key in ["model", "effort", "max_turns"] {
        payload[key] = profile[key].clone();
    }
    payload["provider_health_fallback"] = json!({
        "from_provider":task.agent.provider,"from_backend_profile_id":task.agent.backend_profile_id,
        "to_provider":provider,"to_backend_profile_id":target,
        "reason":"prior_run_local_provider_output_failure","failed_agent_execution_id":failed});
    sqlx::query("UPDATE work_items SET payload_json=? WHERE id=?")
        .bind(payload.to_string())
        .bind(&work)
        .execute(&f.pool)
        .await
        .unwrap();
    sqlx::query("UPDATE agent_executions SET provider=?,model=? WHERE id=?")
        .bind(provider)
        .bind(profile["model"].as_str())
        .bind(&execution)
        .execute(&f.pool)
        .await
        .unwrap();
    sqlx::query("UPDATE artifacts SET provider=?,contract_id=? WHERE id=?")
        .bind(provider)
        .bind(format!("{provider}.output"))
        .bind(id.to_string())
        .execute(&f.pool)
        .await
        .unwrap();
    let (_, prepared) = f.page().await;
    let seed = prepared
        .inputs()
        .iter()
        .find(|input| input.original_artifact_id == id)
        .unwrap();
    assert_eq!(seed.source_schema_version, "legacy_plain_proposal_v1");
    assert_eq!(
        seed.historical_approval_relation["provenance"]["content_binding"],
        "fresh_observation_only"
    );
    for pointer in [
        "/provider_health_fallback",
        "/backend_profile_id",
        "/p058_claimed/agent_execution_id",
    ] {
        let mut changed = payload.clone();
        *changed.pointer_mut(pointer).unwrap() = json!("unbound");
        sqlx::query("UPDATE work_items SET payload_json=? WHERE id=?")
            .bind(changed.to_string())
            .bind(&work)
            .execute(&f.pool)
            .await
            .unwrap();
        assert!(
            matches!(
                preview(&f.pool, f.input.clone()).await.unwrap(),
                PreviewOutcome::Held { .. }
            ),
            "unbound historical fallback accepted: {pointer}"
        );
        sqlx::query("UPDATE work_items SET payload_json=? WHERE id=?")
            .bind(payload.to_string())
            .bind(&work)
            .execute(&f.pool)
            .await
            .unwrap();
    }
    let checksum: Option<String> =
        sqlx::query_scalar("SELECT checksum_sha256 FROM artifacts WHERE id=?")
            .bind(id.to_string())
            .fetch_one(&f.pool)
            .await
            .unwrap();
    assert!(checksum.is_none());
}

#[tokio::test]
async fn dynamic_reviewer_uses_exact_frozen_binding_and_generated_confined_path() {
    let f = Fixture::new().await;
    let plan = f.frozen_plan().await;
    let binding = plan
        .dynamic_candidate_bindings
        .iter()
        .find(|b| b.agent_id == "proposal_reviewer_rust_architect")
        .unwrap();
    let agent: workflow::plan::ResolvedAgent =
        serde_json::from_str(&binding.resolved_agent_snapshot_json).unwrap();
    let name = "proposal_review_rust_architect";
    assert!(!plan.artifact_paths.contains_key(name));
    let path = f.root.join(format!(
        ".chainworks/runs/{}/reviews/proposal/rust-architect.json",
        f.input.source_run_id
    ));
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let bytes = serde_json::to_vec(&json!({"agent_id":agent.agent_id,"role":"rust_architect","score":7,"decision":"revise","verdict":"revise","summary":"Keep unresolved lifetime finding","issues":["OLD-RUST-1"],"blocking_issues":["OLD-RUST-1"],"non_blocking_issues":[],"suggestions":[],"assumptions":[]})).unwrap();
    fs::write(&path, &bytes).unwrap();
    let id = uuid::Uuid::new_v4();
    f.artifact(id, name, "proposal_review_v1", &path, &bytes)
        .await;
    let schema = json!({"contract_id":"proposal_review_v1","format":"json","machine_format":"json","validation_mode":"strict_structured","required_fields":["agent_id","role","score","decision","verdict","summary","issues","blocking_issues","non_blocking_issues","suggestions","assumptions"]});
    f.bind_producer(id, "state_4_proposal_reviewed", &agent.agent_id, &agent.provider, "dynamic_review_proposal_reviewer_rust_architect",
        json!([{"output_name":name,"target_path":path,"schema":schema,"companion_output_name":null,"companion_path":null}]),
        Some(json!({"p060_binding_id":binding.binding_id,"p060_plan_hash":"a".repeat(64),"p060_dynamic_phase":0,
            "task_inputs":plan.states["state_4_proposal_reviewed"].dynamic_parallel.as_ref().unwrap().inputs,
            "task_outputs":[name]}))).await;
    let (work, raw): (String, String) = sqlx::query_as("SELECT id,payload_json FROM work_items WHERE json_extract(payload_json,'$.p060_binding_id')=?")
        .bind(&binding.binding_id).fetch_one(&f.pool).await.unwrap();
    let (_, prepared) = f.page().await;
    let review = prepared
        .inputs()
        .iter()
        .find(|i| i.original_artifact_id == id)
        .unwrap();
    assert!(review.mandatory);
    assert_eq!(review.role, EntryRole::ReferenceOnly);
    assert_eq!(
        review.source_relative_path,
        "reviews/proposal/rust-architect.json"
    );
    assert_eq!(fs::read(review.source_path()).unwrap(), bytes);
    assert_eq!(
        review.historical_approval_relation["provenance"]["dynamic_binding_id"],
        binding.binding_id
    );
    sqlx::query("UPDATE work_items SET payload_json=json_set(payload_json,'$.p060_binding_id','invented-binding') WHERE id=?").bind(&work).execute(&f.pool).await.unwrap();
    assert!(matches!(
        preview(&f.pool, f.input.clone()).await.unwrap(),
        PreviewOutcome::Held { .. }
    ));
    sqlx::query("UPDATE work_items SET payload_json=? WHERE id=?")
        .bind(&raw)
        .bind(&work)
        .execute(&f.pool)
        .await
        .unwrap();
    let arbitrary = path.with_file_name("arbitrary.json");
    fs::write(&arbitrary, &bytes).unwrap();
    let mut changed: Value = serde_json::from_str(&raw).unwrap();
    changed["declared_outputs"][0]["target_path"] = json!(arbitrary);
    sqlx::query("UPDATE work_items SET payload_json=? WHERE id=?")
        .bind(changed.to_string())
        .bind(&work)
        .execute(&f.pool)
        .await
        .unwrap();
    sqlx::query("UPDATE artifacts SET file_path=? WHERE id=?")
        .bind(arbitrary.to_str().unwrap())
        .bind(id.to_string())
        .execute(&f.pool)
        .await
        .unwrap();
    assert!(
        matches!(
            preview(&f.pool, f.input.clone()).await.unwrap(),
            PreviewOutcome::Held { .. }
        ),
        "matching persisted paths do not authorize an arbitrary dynamic output path"
    );
}

#[tokio::test]
async fn strict_wire_page_uses_observed_witness_and_omits_excluded_hashes() {
    use domain::run_carry_forward_api::RunCarryForwardPreviewPageV1;
    let mut f = Fixture::new().await;
    fs::write(f.root.join(".DS_Store"), b"machine metadata").unwrap();
    f.input
        .selection
        .excluded_workspace_paths
        .insert(".DS_Store".into(), "machine metadata".into());
    f.input.limit = Some(500);
    let (page, prepared) = f.page().await;
    let wire = engine::run_carry_forward::preview::wire_page(&page, &prepared).unwrap();
    let stage: String = sqlx::query_scalar("SELECT id FROM stage_executions WHERE run_id=?")
        .bind(f.input.source_run_id.to_string())
        .fetch_one(&f.pool)
        .await
        .unwrap();
    assert_eq!(
        wire.plan_summary
            .source_witness
            .stage_execution_id
            .as_uuid()
            .to_string(),
        stage
    );
    assert_eq!(
        wire.plan_summary.source_witness.head.as_str(),
        prepared.workspace().head()
    );
    assert!(wire
        .plan_summary
        .source_witness
        .cursor
        .as_str()
        .contains(PREPARE));
    assert!(wire
        .plan_summary
        .source_witness
        .journal_ids
        .as_slice()
        .is_empty());
    assert_eq!(
        wire.entry_count.get() as usize,
        prepared.plan().entries.len()
    );
    assert_eq!(wire.plan_sha256, page.plan_sha256);
    let encoded = serde_json::to_value(&wire).unwrap();
    let _: RunCarryForwardPreviewPageV1 = serde_json::from_value(encoded.clone()).unwrap();
    assert!(encoded["plan_summary"]["source_witness"]
        .to_string()
        .find(f.root.to_str().unwrap())
        .is_none());
    let excluded = encoded["entries"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["role"] == "excluded")
        .unwrap();
    assert!(excluded.get("sha256").is_none());
    assert_eq!(excluded["reason"], "excluded_machine_metadata");
    assert!(encoded["plan_summary"]["capability_delta"]["removed"]
        .as_array()
        .is_some_and(|v| !v.is_empty()));
}

#[tokio::test]
async fn missing_or_ambiguous_canonical_stage_is_not_replaced_with_an_invented_id() {
    let f = Fixture::new().await;
    sqlx::query("DELETE FROM stage_executions")
        .execute(&f.pool)
        .await
        .unwrap();
    assert!(
        matches!(preview(&f.pool, f.input).await.unwrap(), PreviewOutcome::Held { holds } if holds.iter().any(|h| h.code.as_str() == "unsupported_frontier"))
    );
}

#[tokio::test]
async fn mediation_owned_agents_are_in_witness_and_block_preview() {
    use engine::run_carry_forward::preview::source_semantic_witness;
    let f = Fixture::new().await;
    let mediation = uuid::Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO lead_conflict_mediations(id,run_id,conflict_id,conflict_fingerprint,lead_agent_id,status,created_at,updated_at) VALUES (?,?,'conflict','fingerprint','lead_orchestrator','settled','2026-09-20','2026-09-20')")
        .bind(&mediation).bind(f.input.source_run_id.to_string()).execute(&f.pool).await.unwrap();
    let before = source_semantic_witness(&f.pool, f.input.source_run_id)
        .await
        .unwrap();
    sqlx::query("INSERT INTO agent_executions(id,agent_id,provider,status,started_at,owner_kind,owner_id,lead_mediation_record_id) VALUES (?,'lead_orchestrator','fixture','running','2026-09-20','lead_conflict_mediation',?,?)")
        .bind(uuid::Uuid::new_v4().to_string()).bind(&mediation).bind(&mediation).execute(&f.pool).await.unwrap();
    assert_ne!(
        before,
        source_semantic_witness(&f.pool, f.input.source_run_id)
            .await
            .unwrap()
    );
    assert!(
        matches!(preview(&f.pool, f.input).await.unwrap(), PreviewOutcome::Held { holds } if holds.iter().any(|h| h.code.as_str() == "source_busy"))
    );
}

#[tokio::test]
async fn runtime_invocation_under_indirect_stage_owner_changes_semantic_witness() {
    use engine::run_carry_forward::preview::source_semantic_witness;
    let f = Fixture::new().await;
    let execution = uuid::Uuid::new_v4().to_string();
    let stage: String = sqlx::query_scalar("SELECT id FROM stage_executions")
        .fetch_one(&f.pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO agent_executions(id,stage_execution_id,agent_id,provider,status,started_at,owner_kind,owner_id) VALUES (?,?,'code_writer','fixture','completed','2026-09-20','stage_execution',?)")
        .bind(&execution).bind(&stage).bind(&stage).execute(&f.pool).await.unwrap();
    let before = source_semantic_witness(&f.pool, f.input.source_run_id)
        .await
        .unwrap();
    sqlx::query("INSERT INTO runtime_invocations(id,agent_execution_id,provider,status,started_at) VALUES (?,?,'fixture','running','2026-09-20')")
        .bind(uuid::Uuid::new_v4().to_string()).bind(execution).execute(&f.pool).await.unwrap();
    assert_ne!(
        before,
        source_semantic_witness(&f.pool, f.input.source_run_id)
            .await
            .unwrap()
    );
    assert!(
        matches!(preview(&f.pool, f.input).await.unwrap(), PreviewOutcome::Held { holds } if holds.iter().any(|h| h.code.as_str() == "source_busy"))
    );
}

#[tokio::test]
async fn deletion_preimages_stream_large_blobs_and_hash_genuinely_empty_files() {
    use domain::run_carry_forward::ContentDigest;
    let f = Fixture::new().await;
    let large = vec![0u8; 17 * 1024 * 1024];
    fs::write(f.root.join("large.bin"), &large).unwrap();
    fs::write(f.root.join("empty.txt"), []).unwrap();
    git(&f.root, &["add", "large.bin", "empty.txt"]);
    git(
        &f.root,
        &[
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=fixture@example.invalid",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "-qm",
            "historical preimages",
        ],
    );
    fs::remove_file(f.root.join("large.bin")).unwrap();
    fs::remove_file(f.root.join("empty.txt")).unwrap();
    let before = tree(&f.root);
    let (_, prepared) = f.page().await;
    for (name, bytes) in [("large.bin", large.as_slice()), ("empty.txt", &[][..])] {
        let entry = prepared
            .plan()
            .entries
            .iter()
            .find(|entry| entry.key.relative_path == name)
            .unwrap();
        assert_eq!(entry.sha256, Some(ContentDigest::of(bytes)));
        assert_eq!(entry.bytes, bytes.len() as u64);
        assert_eq!(entry.reason, "preserve_workspace_deletion");
        assert!(entry.workspace_entry.as_ref().unwrap().deleted);
        engine::run_carry_forward::preview::wire_manifest_entry(
            &serde_json::to_value(entry).unwrap(),
        )
        .unwrap();
    }
    assert_eq!(before, tree(&f.root));
}

#[tokio::test]
async fn deleted_path_wire_entry_hashes_real_head_preimage_without_restoring_file() {
    use domain::run_carry_forward::ContentDigest;
    use engine::run_carry_forward::preview::wire_manifest_entry;
    let f = Fixture::new().await;
    fs::remove_file(f.root.join("product.txt")).unwrap();
    let before = tree(f.root.parent().unwrap());
    let (_, prepared) = f.page().await;
    let entry = prepared
        .plan()
        .entries
        .iter()
        .find(|entry| entry.key.relative_path == "product.txt")
        .unwrap();
    assert!(entry.workspace_entry.as_ref().unwrap().deleted);
    assert_eq!(entry.role, EntryRole::PreserveOnly);
    assert_eq!(entry.sha256.as_ref(), Some(&ContentDigest::of(b"base\n")));
    assert_eq!(entry.bytes, 5);
    assert_eq!(entry.source_contract.as_deref(), Some("git_head_blob"));
    assert_eq!(
        entry.source_schema_version.as_deref(),
        Some("workspace_deletion_preimage_v1")
    );
    let wire =
        serde_json::to_value(wire_manifest_entry(&serde_json::to_value(entry).unwrap()).unwrap())
            .unwrap();
    assert_eq!(wire["reason"], "preserve_workspace_deletion");
    assert_eq!(before, tree(f.root.parent().unwrap()));
}

#[tokio::test]
async fn renamed_paths_and_staged_only_deletions_keep_every_entry_and_index_preimage() {
    use domain::run_carry_forward::ContentDigest;
    use engine::run_carry_forward::preview::wire_manifest_entry;
    let f = Fixture::new().await;
    fs::rename(f.root.join("product.txt"), f.root.join("renamed.txt")).unwrap();
    git(&f.root, &["add", "--", "product.txt", "renamed.txt"]);
    fs::write(f.root.join("staged-only.txt"), b"only in preserved index\n").unwrap();
    git(&f.root, &["add", "--", "staged-only.txt"]);
    fs::remove_file(f.root.join("staged-only.txt")).unwrap();
    let before = tree(f.root.parent().unwrap());
    let (_, prepared) = f.page().await;
    for name in ["product.txt", "renamed.txt", "staged-only.txt"] {
        let entry = prepared
            .plan()
            .entries
            .iter()
            .find(|entry| entry.key.relative_path == name)
            .unwrap();
        wire_manifest_entry(&serde_json::to_value(entry).unwrap()).unwrap();
    }
    let index = prepared
        .plan()
        .entries
        .iter()
        .find(|entry| entry.key.relative_path == "staged-only.txt")
        .unwrap();
    assert_eq!(index.source_contract.as_deref(), Some("git_index_blob"));
    assert_eq!(
        index.sha256.as_ref(),
        Some(&ContentDigest::of(b"only in preserved index\n"))
    );
    assert_eq!(before, tree(f.root.parent().unwrap()));
}

#[tokio::test]
async fn more_than_128_observed_journals_hold_instead_of_truncating_wire_witness() {
    let f = Fixture::new().await;
    for _ in 0..129 {
        sqlx::query("INSERT INTO command_journal(id,command_type,payload_json,run_id,created_at) VALUES (?,'fixture','{}',?,'2026-09-20')")
            .bind(uuid::Uuid::new_v4().to_string()).bind(f.input.source_run_id.to_string()).execute(&f.pool).await.unwrap();
    }
    assert!(
        matches!(preview(&f.pool, f.input).await.unwrap(), PreviewOutcome::Held { holds } if holds.iter().any(|h| h.code.as_str() == "continuation_budget_exceeded"))
    );
}

fn tree(root: &Path) -> BTreeMap<PathBuf, (u32, Vec<u8>)> {
    use std::os::unix::fs::PermissionsExt;
    let mut result = BTreeMap::new();
    fn walk(root: &Path, at: &Path, out: &mut BTreeMap<PathBuf, (u32, Vec<u8>)>) {
        for item in fs::read_dir(at).unwrap() {
            let item = item.unwrap();
            let metadata = item.path().symlink_metadata().unwrap();
            let bytes = if metadata.is_dir() {
                vec![]
            } else if metadata.is_symlink() {
                fs::read_link(item.path())
                    .unwrap()
                    .as_os_str()
                    .as_encoded_bytes()
                    .to_vec()
            } else {
                fs::read(item.path()).unwrap()
            };
            out.insert(
                item.path().strip_prefix(root).unwrap().to_owned(),
                (metadata.permissions().mode(), bytes),
            );
            if metadata.is_dir() {
                walk(root, &item.path(), out);
            }
        }
    }
    walk(root, root, &mut result);
    result
}

#[tokio::test]
async fn preview_compiles_current_target_without_filesystem_git_or_database_writes() {
    let f = Fixture::new().await;
    let before = tree(f.root.parent().unwrap());
    let changes: i64 = sqlx::query_scalar("SELECT total_changes()")
        .fetch_one(&f.pool)
        .await
        .unwrap();
    let (page, prepared) = f.page().await;
    assert_eq!(page.schema_version, "run_carry_forward_preview_page_v1");
    assert_eq!(
        prepared.target().plan.initial_state,
        "state_4_proposal_reviewed"
    );
    assert_eq!(prepared.inputs().len(), 1);
    assert_eq!(prepared.inputs()[0].role, EntryRole::ExecutionSeed);
    assert_eq!(prepared.workspace().source(), f.root);
    assert!(page
        .plan_summary
        .pending_checks
        .iter()
        .any(|c| c == "worktree_base_writable"));
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT total_changes()")
            .fetch_one(&f.pool)
            .await
            .unwrap(),
        changes
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM run_continuations")
            .fetch_one(&f.pool)
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM command_journal")
            .fetch_one(&f.pool)
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM work_items")
            .fetch_one(&f.pool)
            .await
            .unwrap(),
        0
    );
    assert_eq!(tree(f.root.parent().unwrap()), before);
}

#[tokio::test]
async fn all_pages_share_full_digest_and_source_change_cannot_mix_generations() {
    let mut f = Fixture::new().await;
    let (first, prepared) = f.page().await;
    assert!(first.entry_count > first.entries.len());
    let digest = first.plan_sha256.clone();
    let mut count = first.entries.len();
    let mut cursor = first.next_cursor.clone();
    while let Some(next) = cursor {
        f.input.cursor = Some(next);
        f.input.limit = Some(500);
        let (page, _) = f.page().await;
        assert_eq!(page.plan_sha256, digest);
        count += page.entries.len();
        cursor = page.next_cursor;
    }
    assert_eq!(count, prepared.plan().entries.len());
    fs::write(f.root.join("product.txt"), "changed since first page\n").unwrap();
    f.input.cursor = first.next_cursor;
    assert!(matches!(
        preview(&f.pool, f.input).await.unwrap(),
        PreviewOutcome::Stale(_)
    ));
}

#[tokio::test]
async fn database_semantic_changes_invalidate_cursor_but_informational_timestamps_do_not() {
    let mut f = Fixture::new().await;
    let (page, _) = f.page().await;
    f.input.cursor = page.next_cursor;
    sqlx::query("UPDATE artifacts SET created_at='2026-09-21T00:00:00Z'")
        .execute(&f.pool)
        .await
        .unwrap();
    assert!(matches!(
        preview(&f.pool, f.input.clone()).await.unwrap(),
        PreviewOutcome::Page { .. }
    ));
    sqlx::query("UPDATE ideas SET body='changed scope'")
        .execute(&f.pool)
        .await
        .unwrap();
    assert!(matches!(
        preview(&f.pool, f.input).await.unwrap(),
        PreviewOutcome::Stale(_)
    ));
}

#[tokio::test]
async fn artifact_must_be_owned_by_immediate_source_and_current_bytes_must_match() {
    let f = Fixture::new().await;
    let id = match f.input.selection.proposal_input {
        InputReference::Artifact { id } => id,
        _ => unreachable!(),
    };
    sqlx::query("UPDATE artifacts SET checksum_sha256=? WHERE id=?")
        .bind("a".repeat(64))
        .bind(id.to_string())
        .execute(&f.pool)
        .await
        .unwrap();
    assert!(
        matches!(preview(&f.pool, f.input).await.unwrap(), PreviewOutcome::Held { holds } if holds.iter().any(|h| h.code.as_str() == "artifact_provenance_invalid"))
    );
}

#[tokio::test]
async fn exact_finding_contract_is_automatically_reference_only() {
    let f = Fixture::new().await;
    let id = uuid::Uuid::new_v4();
    let path = f
        .root
        .join(format!(".chainworks/runs/{}/audit", f.input.source_run_id));
    fs::create_dir_all(&path).unwrap();
    let path = path.join("proposal-vs-implementation.json");
    let bytes = serde_json::to_vec(&json!({"status":"needs_code_fixes","matches_proposal":false,"missing_items":[],"extra_items":[],"defects":[{"owner":"human","description":"scope undecided"}],"required_fixes":["human decision"]})).unwrap();
    fs::write(&path, &bytes).unwrap();
    f.artifact(id, "audit_report", "audit_report_v1", &path, &bytes)
        .await;
    let (_, prepared) = f.page().await;
    let reference = prepared
        .inputs()
        .iter()
        .find(|i| i.original_artifact_id == id)
        .unwrap();
    assert_eq!(reference.role, EntryRole::ReferenceOnly);
    assert!(reference.mandatory);
    assert_eq!(reference.immediate_source_run_id, f.input.source_run_id);
}

#[tokio::test]
async fn familiar_legacy_ids_do_not_authorize_changed_gate_topology() {
    let f = Fixture::new().await;
    let text: String = sqlx::query_scalar("SELECT workflow_snapshot_json FROM runs")
        .fetch_one(&f.pool)
        .await
        .unwrap();
    let mut value: Value = serde_json::from_str(&text).unwrap();
    value["states"]["state_6_implementation_approval"]["transitions"][0]["when"] = json!("true");
    let text = serde_json::to_string(&value).unwrap();
    use sha2::{Digest, Sha256};
    sqlx::query("UPDATE runs SET workflow_snapshot_json=?,workflow_snapshot_hash=?")
        .bind(&text)
        .bind(format!("{:x}", Sha256::digest(text.as_bytes())))
        .execute(&f.pool)
        .await
        .unwrap();
    assert!(
        matches!(preview(&f.pool, f.input).await.unwrap(), PreviewOutcome::Held { holds } if holds.iter().any(|h| h.code.as_str() == "unsupported_frontier"))
    );
}

#[tokio::test]
async fn stale_expected_target_hash_holds_and_malformed_cursor_is_invalid_input() {
    let mut f = Fixture::new().await;
    f.input.target.expected_workflow_snapshot_hash =
        Some(domain::run_carry_forward::ContentDigest::of(b"old target"));
    assert!(
        matches!(preview(&f.pool, f.input.clone()).await.unwrap(), PreviewOutcome::Held { holds } if holds.iter().any(|h| h.code.as_str() == "source_changed"))
    );
    f.input.target.expected_workflow_snapshot_hash = None;
    for cursor in ["not a cursor".to_string(), "x".repeat(4097), "{}".into()] {
        f.input.cursor = Some(cursor);
        assert!(matches!(
            preview(&f.pool, f.input.clone()).await,
            Err(PreviewError::InvalidInput(_))
        ));
    }
}

#[tokio::test]
async fn target_symlinks_and_definition_root_escapes_are_held() {
    use std::os::unix::fs::symlink;
    let mut f = Fixture::new().await;
    symlink("current.yaml", f.root.join("examples/workflows/link.yaml")).unwrap();
    for path in [
        "examples/workflows/link.yaml",
        "examples/workflows/../../product.txt",
    ] {
        f.input.target.workflow_yaml_path = path.into();
        assert!(
            matches!(preview(&f.pool, f.input.clone()).await.unwrap(), PreviewOutcome::Held { holds } if holds.iter().any(|h| h.code.as_str() == "unsafe_path"))
        );
    }
}

#[tokio::test]
async fn foreign_artifact_id_and_unknown_finding_schema_cannot_be_used() {
    let mut f = Fixture::new().await;
    let original = f.input.selection.proposal_input.clone();
    f.input.selection.proposal_input = InputReference::Artifact {
        id: uuid::Uuid::new_v4(),
    };
    assert!(
        matches!(preview(&f.pool, f.input.clone()).await.unwrap(), PreviewOutcome::Held { holds } if holds[0].code.as_str() == "artifact_provenance_invalid")
    );
    f.input.selection.proposal_input = original;
    let path = f.root.join(".chainworks/unknown.json");
    fs::write(&path, b"{}").unwrap();
    f.artifact(
        uuid::Uuid::new_v4(),
        "audit_report",
        "audit_report_v99",
        &path,
        b"{}",
    )
    .await;
    assert!(
        matches!(preview(&f.pool, f.input).await.unwrap(), PreviewOutcome::Held { holds } if holds[0].code.as_str() == "artifact_provenance_invalid")
    );
}

#[tokio::test]
async fn generated_prefix_cannot_authorize_discarding_unclassified_product_code() {
    let mut f = Fixture::new().await;
    fs::create_dir(f.root.join("target")).unwrap();
    fs::write(f.root.join("target/feature.rs"), "pub fn product() {}\n").unwrap();
    f.input
        .selection
        .excluded_workspace_paths
        .insert("target/feature.rs".into(), "generated prefix".into());
    assert!(
        matches!(preview(&f.pool, f.input).await.unwrap(), PreviewOutcome::Held { holds } if holds[0].code.as_str() == "unsafe_path")
    );
}

#[tokio::test]
async fn cancelled_session_with_unsettled_signal_remains_a_hold() {
    let f = Fixture::new().await;
    let session = uuid::Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO provider_sessions(provider_session_id,run_id,provider,lifecycle_state,process_fate,created_at,updated_at) VALUES (?,?,'fixture','orphan_settled','absent_verified','2026-09-20','2026-09-20')")
        .bind(&session).bind(f.input.source_run_id.to_string()).execute(&f.pool).await.unwrap();
    sqlx::query("INSERT INTO shutdown_signal_side_effects(signal_effect_id,provider_session_id,shutdown_epoch,process_id,process_start_identity,signal_kind,generation,intent_state) VALUES (?,?,1,123,'fixture','kill',1,'dispatching')")
        .bind(uuid::Uuid::new_v4().to_string()).bind(session).execute(&f.pool).await.unwrap();
    assert!(
        matches!(preview(&f.pool, f.input).await.unwrap(), PreviewOutcome::Held { holds } if holds[0].code.as_str() == "source_busy")
    );
}

#[tokio::test]
async fn historical_release_admission_is_not_erased_by_current_implementation_label() {
    let f = Fixture::new().await;
    sqlx::query("INSERT INTO stage_executions(id,run_id,stage_id,label,status,started_at) VALUES (?,?,'state_11_manual_release','Release','completed','2026-09-20')")
        .bind(uuid::Uuid::new_v4().to_string()).bind(f.input.source_run_id.to_string()).execute(&f.pool).await.unwrap();
    assert!(
        matches!(preview(&f.pool, f.input).await.unwrap(), PreviewOutcome::Held { holds } if holds[0].code.as_str() == "unsupported_frontier")
    );
}

#[tokio::test]
async fn current_target_hash_and_request_selection_changes_make_paging_stale() {
    let mut f = Fixture::new().await;
    let (page, prepared) = f.page().await;
    f.input.target.expected_workflow_snapshot_hash = Some(
        prepared
            .plan()
            .summary
            .target
            .workflow_snapshot_hash
            .clone(),
    );
    let (page_with_expected, _) = f.page().await;
    f.input.cursor = page_with_expected.next_cursor;
    let path = f.root.join("examples/workflows/current.yaml");
    let mut yaml: Value = serde_yaml::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    yaml["workflow"]["description"] = json!("changed current definition");
    fs::write(path, serde_yaml::to_string(&yaml).unwrap()).unwrap();
    assert!(matches!(
        preview(&f.pool, f.input.clone()).await.unwrap(),
        PreviewOutcome::Stale(_)
    ));
    f.input.target.expected_workflow_snapshot_hash = None;
    f.input.cursor = page.next_cursor;
    f.input.selection.include_dirty_work = false;
    assert!(matches!(
        preview(&f.pool, f.input).await.unwrap(),
        PreviewOutcome::Stale(_)
    ));
}

#[tokio::test]
async fn strict_page_reference_and_request_bounds_are_checked_before_inventory() {
    let mut f = Fixture::new().await;
    for limit in [0, 501] {
        f.input.limit = Some(limit);
        assert!(matches!(
            preview(&f.pool, f.input.clone()).await,
            Err(PreviewError::InvalidInput(_))
        ));
    }
    f.input.limit = None;
    f.input.selection.reference_inputs = (0..129)
        .map(|_| InputReference::Artifact {
            id: uuid::Uuid::new_v4(),
        })
        .collect();
    assert!(matches!(
        preview(&f.pool, f.input.clone()).await,
        Err(PreviewError::InvalidInput(_))
    ));
    f.input.selection.reference_inputs.clear();
    f.input.profile = "x".repeat(1024 * 1024);
    assert!(matches!(
        preview(&f.pool, f.input).await,
        Err(PreviewError::InvalidInput(_))
    ));
}

async fn add_incoming(f: &mut Fixture, role: &str) -> uuid::Uuid {
    add_incoming_with_relation(f, role, json!({})).await
}

async fn add_incoming_with_relation(f: &mut Fixture, role: &str, relation: Value) -> uuid::Uuid {
    let source = db::repos::runs::find_by_id(&f.pool, f.input.source_run_id)
        .await
        .unwrap()
        .unwrap();
    let ancestor = RunId::new();
    let ancestor_root = f
        .root
        .parent()
        .unwrap()
        .join(format!("ancestor-{ancestor}"));
    // A carried successor has its own checkout, not the fenced ancestor's resource.
    git(
        &f.root,
        &[
            "worktree",
            "add",
            "--detach",
            ancestor_root.to_str().unwrap(),
            "HEAD",
        ],
    );
    let operation = uuid::Uuid::new_v4().to_string();
    let journal = uuid::Uuid::new_v4().to_string();
    let input = uuid::Uuid::new_v4();
    let artifact = match f.input.selection.proposal_input {
        InputReference::Artifact { id } => id,
        _ => unreachable!(),
    };
    sqlx::query("UPDATE runs SET status='completed' WHERE id=?")
        .bind(source.id.to_string())
        .execute(&f.pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO runs(id,idea_id,status,workflow_id,workflow_title,workspace_root,artifact_root,started_at,current_state,worktree_root,chainworks_meta_root,workflow_snapshot_json,catalog_snapshot_json,workflow_snapshot_hash,catalog_snapshot_hash,delivery_configuration_json,agent_catalog_yaml_path) VALUES (?,?,'blocked',?,?,?,?,?,?,?,?,?,?,?,?,?,?)")
        .bind(ancestor.to_string()).bind(source.idea_id.to_string()).bind(&source.workflow_id).bind(&source.workflow_title)
        .bind(&source.workspace_root).bind(&source.artifact_root).bind("2026-09-20T00:00:00Z").bind(&source.current_state)
        .bind(ancestor_root.to_str().unwrap()).bind(&source.chainworks_meta_root).bind(&source.workflow_snapshot_json).bind(&source.catalog_snapshot_json)
        .bind(&source.workflow_snapshot_hash).bind(&source.catalog_snapshot_hash).bind(&source.delivery_configuration_json).bind(&source.agent_catalog_yaml_path)
        .execute(&f.pool).await.unwrap();
    sqlx::query("UPDATE artifacts SET run_id=? WHERE id=?")
        .bind(ancestor.to_string())
        .bind(artifact.to_string())
        .execute(&f.pool)
        .await
        .unwrap();
    sqlx::query("UPDATE stage_executions SET run_id=? WHERE id=(SELECT stage_execution_id FROM agent_executions WHERE id=(SELECT agent_execution_id FROM artifacts WHERE id=?))")
        .bind(ancestor.to_string()).bind(artifact.to_string()).execute(&f.pool).await.unwrap();
    sqlx::query("UPDATE work_items SET run_id=?,payload_json=json_set(payload_json,'$.run_id',?) WHERE json_extract(payload_json,'$.p058_claimed.agent_execution_id')=(SELECT agent_execution_id FROM artifacts WHERE id=?)")
        .bind(ancestor.to_string()).bind(ancestor.to_string()).bind(artifact.to_string()).execute(&f.pool).await.unwrap();
    sqlx::query("INSERT INTO command_journal(id,command_type,payload_json,run_id,created_at) VALUES (?,'ContinueBlocked','{}',?,'2026-09-20')")
        .bind(&journal).bind(ancestor.to_string()).execute(&f.pool).await.unwrap();
    sqlx::query("INSERT INTO run_continuations(operation_id,source_run_id,idea_id,reserved_successor_run_id,caller_fingerprint,caller_request_id,intent_sha256,profile,plan_ref,plan_sha256,target_ref,source_witness_sha256,journal_id,created_at,updated_at,deadline_at) VALUES (?,?,?,?,'fixture',?,?,'implementation_restart_v1','plan',?,'target',?,?,'2026-09-20','2026-09-20','2026-09-21')")
        .bind(&operation).bind(ancestor.to_string()).bind(source.idea_id.to_string()).bind(source.id.to_string()).bind(uuid::Uuid::new_v4().to_string())
        .bind("a".repeat(64)).bind("b".repeat(64)).bind("c".repeat(64)).bind(journal).execute(&f.pool).await.unwrap();
    sqlx::query("INSERT INTO run_execution_fences(source_run_id,operation_id,generation,disposition,created_at) VALUES (?,?,1,'reserved','2026-09-20')")
        .bind(ancestor.to_string()).bind(&operation).execute(&f.pool).await.unwrap();
    sqlx::query("UPDATE run_continuations SET phase='prepared',version=2,manifest_ref='manifest',manifest_sha256=? WHERE operation_id=?")
        .bind("d".repeat(64)).bind(&operation).execute(&f.pool).await.unwrap();
    sqlx::query("UPDATE run_execution_fences SET disposition='historical' WHERE operation_id=?")
        .bind(&operation)
        .execute(&f.pool)
        .await
        .unwrap();
    sqlx::query("UPDATE run_continuations SET phase='activated',version=3,successor_run_id=reserved_successor_run_id WHERE operation_id=?").bind(&operation).execute(&f.pool).await.unwrap();
    let denied = sqlx::query("UPDATE runs SET status='running' WHERE id=?")
        .bind(ancestor.to_string())
        .execute(&f.pool)
        .await
        .unwrap_err();
    assert!(denied.to_string().contains("source_continued"));
    sqlx::query("UPDATE runs SET status='blocked' WHERE id=?")
        .bind(source.id.to_string())
        .execute(&f.pool)
        .await
        .unwrap();
    let hash: Option<String> =
        sqlx::query_scalar("SELECT checksum_sha256 FROM artifacts WHERE id=?")
            .bind(artifact.to_string())
            .fetch_one(&f.pool)
            .await
            .unwrap();
    let hash = hash.unwrap_or_else(|| {
        use sha2::{Digest, Sha256};
        format!(
            "{:x}",
            Sha256::digest(
                fs::read(f.root.join(format!(
                    ".chainworks/runs/{}/proposals/current/proposal.md",
                    source.id
                )))
                .unwrap()
            )
        )
    });
    let contract: String = sqlx::query_scalar("SELECT contract_id FROM artifacts WHERE id=?")
        .bind(artifact.to_string())
        .fetch_one(&f.pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO run_continuation_inputs(input_id,operation_id,ordinal,successor_run_id,role,source_kind,source_id,source_run_id,original_artifact_id,source_schema,content_sha256,target_logical_name,target_relative_path,installed,historical_relation_json) VALUES (?,?,0,?,?,'artifact',?,?,?,?,?,'proposal_current','proposals/current/proposal.md',1,?)")
        .bind(input.to_string()).bind(operation).bind(source.id.to_string()).bind(role).bind(artifact.to_string()).bind(ancestor.to_string())
        .bind(artifact.to_string()).bind(contract).bind(hash).bind(relation.to_string()).execute(&f.pool).await.unwrap();
    f.input.selection.proposal_input = InputReference::CarriedInput { id: input };
    input
}

#[tokio::test]
async fn installed_immediate_carried_seed_keeps_original_provenance_without_fabricated_artifacts() {
    let mut f = Fixture::new().await;
    let (_, target) = f.page().await;
    let plan = target.target().plan.clone();
    let id = add_incoming(&mut f, "execution_seed").await;
    sqlx::query("UPDATE runs SET workflow_snapshot_json=?,catalog_snapshot_json=?,workflow_snapshot_hash=?,catalog_snapshot_hash=? WHERE id=?")
        .bind(&plan.workflow_snapshot_json).bind(&plan.catalog_snapshot_json).bind(&plan.workflow_snapshot_hash).bind(&plan.catalog_snapshot_hash)
        .bind(f.input.source_run_id.to_string()).execute(&f.pool).await.unwrap();
    let (page, prepared) = f.page().await;
    assert_eq!(prepared.inputs().len(), 1);
    assert_eq!(
        prepared.inputs()[0].reference,
        InputReference::CarriedInput { id }
    );
    assert_ne!(prepared.inputs()[0].original_run_id, f.input.source_run_id);
    assert_eq!(prepared.inputs()[0].ancestor_manifest_sha256.len(), 1);
    assert_eq!(page.plan_summary.source_run_id, f.input.source_run_id);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM artifacts")
            .fetch_one(&f.pool)
            .await
            .unwrap(),
        1
    );
}

#[tokio::test]
async fn reference_only_input_cannot_be_presented_as_proposal_seed() {
    let mut f = Fixture::new().await;
    let (_, target) = f.page().await;
    let plan = target.target().plan.clone();
    add_incoming(&mut f, "reference_only").await;
    sqlx::query("UPDATE runs SET workflow_snapshot_json=?,catalog_snapshot_json=?,workflow_snapshot_hash=?,catalog_snapshot_hash=? WHERE id=?")
        .bind(&plan.workflow_snapshot_json).bind(&plan.catalog_snapshot_json).bind(&plan.workflow_snapshot_hash).bind(&plan.catalog_snapshot_hash)
        .bind(f.input.source_run_id.to_string()).execute(&f.pool).await.unwrap();
    assert!(
        matches!(preview(&f.pool, f.input).await.unwrap(), PreviewOutcome::Held { holds } if holds[0].code.as_str() == "artifact_provenance_invalid")
    );
}

#[tokio::test]
async fn installed_plain_seed_retains_fresh_binding_without_backfilling_historical_checksum() {
    let mut f = Fixture::new().await;
    sqlx::query("DELETE FROM artifacts")
        .execute(&f.pool)
        .await
        .unwrap();
    let artifact = f
        .produced(
            "proposal_current",
            "state_2_proposal_drafted",
            b"# Original plain seed\n",
            false,
        )
        .await;
    f.input.selection.proposal_input = InputReference::Artifact { id: artifact };
    let (_, original) = f.page().await;
    let seed = original.inputs()[0].clone();
    let plan = original.target().plan.clone();
    add_incoming_with_relation(
        &mut f,
        "execution_seed",
        seed.historical_approval_relation.clone(),
    )
    .await;
    sqlx::query("UPDATE runs SET workflow_snapshot_json=?,catalog_snapshot_json=?,workflow_snapshot_hash=?,catalog_snapshot_hash=? WHERE id=?")
        .bind(&plan.workflow_snapshot_json).bind(&plan.catalog_snapshot_json).bind(&plan.workflow_snapshot_hash).bind(&plan.catalog_snapshot_hash)
        .bind(f.input.source_run_id.to_string()).execute(&f.pool).await.unwrap();
    let (_, prepared) = f.page().await;
    assert_eq!(prepared.inputs()[0].original_artifact_id, artifact);
    assert_eq!(prepared.inputs()[0].sha256, seed.sha256);
    assert_eq!(
        prepared.inputs()[0].source_schema_version,
        "legacy_plain_proposal_v1"
    );
    assert_eq!(
        prepared.inputs()[0].historical_approval_relation["provenance"]["content_binding"],
        "fresh_observation_only"
    );
    assert_eq!(
        sqlx::query_scalar::<_, Option<String>>("SELECT checksum_sha256 FROM artifacts WHERE id=?")
            .bind(artifact.to_string())
            .fetch_one(&f.pool)
            .await
            .unwrap(),
        None
    );
    fs::write(
        prepared.inputs()[0].source_path(),
        b"# Tampered copied seed\n",
    )
    .unwrap();
    assert!(matches!(
        preview(&f.pool, f.input.clone()).await.unwrap(),
        PreviewOutcome::Held { .. }
    ));
}

#[tokio::test]
async fn canonical_successor_frozen_snapshots_remain_eligible_for_a_linear_second_fork() {
    let mut f = Fixture::new().await;
    let (_, prepared) = f.page().await;
    let plan = prepared.target().plan.clone();
    add_incoming(&mut f, "execution_seed").await;
    sqlx::query("UPDATE runs SET workflow_snapshot_json=?,catalog_snapshot_json=?,workflow_snapshot_hash=?,catalog_snapshot_hash=? WHERE id=?")
        .bind(&plan.workflow_snapshot_json).bind(&plan.catalog_snapshot_json).bind(&plan.workflow_snapshot_hash).bind(&plan.catalog_snapshot_hash)
        .bind(f.input.source_run_id.to_string()).execute(&f.pool).await.unwrap();
    assert!(matches!(
        preview(&f.pool, f.input).await.unwrap(),
        PreviewOutcome::Page { .. }
    ));
}

#[tokio::test]
async fn full_plan_digest_is_the_canonical_plan_without_its_self_hash() {
    let f = Fixture::new().await;
    let (_, prepared) = f.page().await;
    let mut body = serde_json::to_value(prepared.plan()).unwrap();
    body.as_object_mut().unwrap().remove("plan_sha256");
    assert_eq!(
        domain::run_carry_forward::canonical_digest(&body).unwrap(),
        prepared.plan().plan_sha256
    );
}

#[tokio::test]
async fn catalog_skill_bytes_and_original_target_hashes_are_part_of_plan_identity() {
    let mut f = Fixture::new().await;
    let (first, prepared) = f.page().await;
    assert_ne!(
        prepared.plan().summary.target.catalog_snapshot_hash,
        prepared
            .plan()
            .summary
            .target
            .successor_catalog_snapshot_hash
    );
    f.input.cursor = first.next_cursor;
    let skill = f
        .root
        .join("examples/agents/skills/code-implementation/SKILL.md");
    let mut text = fs::read_to_string(&skill).unwrap();
    text.push_str("\nAdditional current instruction.\n");
    fs::write(skill, text).unwrap();
    assert!(matches!(
        preview(&f.pool, f.input).await.unwrap(),
        PreviewOutcome::Stale(_)
    ));
}

#[tokio::test]
async fn oversized_artifact_is_a_budget_hold_without_reading_its_sparse_contents() {
    let f = Fixture::new().await;
    let path = f.root.join(format!(
        ".chainworks/runs/{}/proposals/current/proposal.md",
        f.input.source_run_id
    ));
    fs::OpenOptions::new()
        .write(true)
        .open(path)
        .unwrap()
        .set_len(256 * 1024 * 1024 + 1)
        .unwrap();
    assert!(
        matches!(preview(&f.pool, f.input).await.unwrap(), PreviewOutcome::Held { holds } if holds[0].code.as_str() == "continuation_budget_exceeded")
    );
}

#[tokio::test]
async fn new_artifact_contract_generation_invalidates_the_database_witness() {
    let mut f = Fixture::new().await;
    let (page, _) = f.page().await;
    f.input.cursor = page.next_cursor;
    let artifact = match f.input.selection.proposal_input {
        InputReference::Artifact { id } => id,
        _ => unreachable!(),
    };
    sqlx::query("INSERT INTO artifact_contract_generations(generation_id,run_id,artifact_id,contract_id,canonical_path,raw_path,raw_status,canonical_status,valid,created_at) VALUES (?,?,?,'proposal_current','fixture','fixture','valid','valid',1,'2026-09-20')")
        .bind(uuid::Uuid::new_v4().to_string()).bind(f.input.source_run_id.to_string()).bind(artifact.to_string()).execute(&f.pool).await.unwrap();
    assert!(matches!(
        preview(&f.pool, f.input).await.unwrap(),
        PreviewOutcome::Stale(_)
    ));
}

#[tokio::test]
async fn canonical_worktree_from_another_repository_cannot_be_carried_into_current_target() {
    let f = Fixture::new().await;
    let other = f.root.parent().unwrap().join("foreign");
    fs::create_dir(&other).unwrap();
    fs::write(other.join("foreign.txt"), "foreign\n").unwrap();
    git(&other, &["init", "-q"]);
    git(&other, &["add", "."]);
    git(
        &other,
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
    sqlx::query("UPDATE runs SET worktree_root=? WHERE id=?")
        .bind(other.to_str().unwrap())
        .bind(f.input.source_run_id.to_string())
        .execute(&f.pool)
        .await
        .unwrap();
    assert!(
        matches!(preview(&f.pool, f.input).await.unwrap(), PreviewOutcome::Held { holds } if holds[0].code.as_str() == "unsafe_path")
    );
}

#[tokio::test]
async fn transaction_witness_excludes_only_exact_owned_operation_and_its_journals() {
    let f = Fixture::new().await;
    let mut connection = f.pool.acquire().await.unwrap();
    let before =
        db::repos::run_continuation_witness::observe(&mut connection, f.input.source_run_id, None)
            .await
            .unwrap();
    let source = f.input.source_run_id.to_string();
    let idea: String = sqlx::query_scalar("SELECT idea_id FROM runs WHERE id=?")
        .bind(&source)
        .fetch_one(&mut *connection)
        .await
        .unwrap();
    let operation = uuid::Uuid::new_v4().to_string();
    let journal = uuid::Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO command_journal(id,command_type,payload_json,run_id,created_at) VALUES (?,'ContinueBlocked','{}',?,'2026-09-20')")
        .bind(&journal).bind(&source).execute(&mut *connection).await.unwrap();
    sqlx::query("INSERT INTO run_continuations(operation_id,source_run_id,idea_id,reserved_successor_run_id,caller_fingerprint,caller_request_id,intent_sha256,profile,plan_ref,plan_sha256,target_ref,source_witness_sha256,journal_id,created_at,updated_at,deadline_at) VALUES (?,?,?,?,'fixture',?,?,'implementation_restart_v1','plan',?,'target',?,?,'2026-09-20','2026-09-20','2026-09-21')")
        .bind(&operation).bind(&source).bind(idea).bind(uuid::Uuid::new_v4().to_string()).bind(uuid::Uuid::new_v4().to_string())
        .bind("a".repeat(64)).bind("b".repeat(64)).bind("c".repeat(64)).bind(&journal).execute(&mut *connection).await.unwrap();
    sqlx::query("INSERT INTO run_execution_fences(source_run_id,operation_id,generation,disposition,created_at) VALUES (?,?,1,'reserved','2026-09-20')")
        .bind(&source).bind(&operation).execute(&mut *connection).await.unwrap();
    let command_journal = uuid::Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO command_journal(id,command_type,payload_json,run_id,created_at) VALUES (?,'InspectContinuation','{}',?,'2026-09-20')")
        .bind(&command_journal).bind(&source).execute(&mut *connection).await.unwrap();
    sqlx::query("INSERT INTO run_continuation_commands(caller_fingerprint,caller_request_id,command,intent_sha256,source_run_id,operation_id,result_json,created_at) VALUES ('fixture',?,'runs.continuation_activate',?,?,?,?,'2026-09-20')")
        .bind(uuid::Uuid::new_v4().to_string()).bind("e".repeat(64)).bind(&source).bind(&operation)
        .bind(json!({"journal_id":command_journal}).to_string()).execute(&mut *connection).await.unwrap();
    assert_ne!(
        before,
        db::repos::run_continuation_witness::observe(&mut connection, f.input.source_run_id, None)
            .await
            .unwrap()
    );
    assert_eq!(
        before,
        db::repos::run_continuation_witness::observe(
            &mut connection,
            f.input.source_run_id,
            Some(&operation)
        )
        .await
        .unwrap()
    );
    assert!(db::repos::run_continuation_witness::observe(
        &mut connection,
        f.input.source_run_id,
        Some(&uuid::Uuid::new_v4().to_string())
    )
    .await
    .is_err());
    sqlx::query("INSERT INTO command_journal(id,command_type,payload_json,run_id,created_at) VALUES (?,'Unrelated','{}',?,'2026-09-20')")
        .bind(uuid::Uuid::new_v4().to_string()).bind(&source).execute(&mut *connection).await.unwrap();
    assert_ne!(
        before,
        db::repos::run_continuation_witness::observe(
            &mut connection,
            f.input.source_run_id,
            Some(&operation)
        )
        .await
        .unwrap()
    );
}
