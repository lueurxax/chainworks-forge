use domain::{
    ids::RunId,
    run_carry_forward::{ContentDigest, EntryRole},
};
use engine::run_carry_forward::{
    preview::{
        preview, InputReference, PreviewInput, PreviewOutcome, PreviewSelection, PreviewTarget,
    },
    read_manifest,
};
use serde_json::{json, Value};
use sqlx::SqlitePool;
use std::{collections::BTreeMap, fs};
use uuid::Uuid;

#[allow(dead_code)]
mod p039_fixture;
use p039_fixture::{hex, private_file_digests, Fixture};

const PREPARE: &str = "state_7_implementation_started";
const HUMAN_SCHEMA: &str = "blocker_boundary_human_decision_v1";
const HUMAN_BYTES: &[u8] = br#"{"schema_version":"blocker_boundary_human_decision_v1","decision_label":"followup_proposal","notes":"Preserve the unresolved human boundary"}"#;

async fn witness(pool: &SqlitePool, run: RunId) -> ContentDigest {
    let mut connection = pool.acquire().await.unwrap();
    db::repos::run_continuation_witness::observe(&mut connection, run, None)
        .await
        .unwrap()
}

// Snapshot fixture table rows without excluding source authority or audit tables.
async fn database_rows(pool: &SqlitePool) -> BTreeMap<String, Vec<String>> {
    fn identifier(name: &str) -> String {
        format!("\"{}\"", name.replace('"', "\"\""))
    }
    let tables: Vec<String> = sqlx::query_scalar("SELECT name FROM sqlite_schema WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name")
        .fetch_all(pool).await.unwrap();
    let mut result = BTreeMap::new();
    for table in tables {
        let columns: Vec<String> =
            sqlx::query_scalar("SELECT name FROM pragma_table_info(?) ORDER BY cid")
                .bind(&table)
                .fetch_all(pool)
                .await
                .unwrap();
        let expressions: Vec<_> = columns.iter().map(|column| {
            let name = identifier(column);
            format!("json_array(typeof({name}),CASE WHEN typeof({name})='blob' THEN hex({name}) ELSE {name} END)")
        }).collect();
        let mut query = sqlx::QueryBuilder::<sqlx::Sqlite>::new("SELECT json_array(");
        query
            .push(expressions.join(","))
            .push(") FROM ")
            .push(identifier(&table));
        let mut rows: Vec<String> = query.build_query_scalar().fetch_all(pool).await.unwrap();
        rows.sort();
        result.insert(table, rows);
    }
    result
}

async fn set_source_catalog(f: &Fixture, catalog: &Value) {
    let raw = serde_json::to_string(catalog).unwrap();
    sqlx::query("UPDATE runs SET catalog_snapshot_json=?,catalog_snapshot_hash=? WHERE id=?")
        .bind(&raw)
        .bind(hex(&ContentDigest::of(raw.as_bytes())))
        .bind(f.input.source_run_id.to_string())
        .execute(&f.pool)
        .await
        .unwrap();
}

#[tokio::test]
async fn target_without_explicit_headless_is_denied_without_rewriting_legacy_source() {
    let f = Fixture::new().await;
    let source = db::repos::runs::find_by_id(&f.pool, f.input.source_run_id)
        .await
        .unwrap()
        .unwrap();
    let mut historical: Value =
        serde_json::from_str(source.catalog_snapshot_json.as_deref().unwrap()).unwrap();
    let profile = &mut historical["permission_profiles"]["CODE_WRITE"];
    profile.as_object_mut().unwrap().remove("xcode_headless");
    profile["shell"]["allow"]
        .as_array_mut()
        .unwrap()
        .push(json!(
            "xcodebuild -workspace Legacy.xcworkspace -scheme Legacy build"
        ));
    profile["mcp"]["legacy_allow"] = json!(["xcode.build", "xcode.test"]);
    set_source_catalog(&f, &historical).await;

    let (service, principal) = f.service();
    // A real eligible preview establishes that the legacy source itself is supported.
    let mut request = f.service_request(&service, &principal).await;
    let catalog_path = f.root.join("examples/agents/agents.yaml");
    let mut target: Value = serde_yaml::from_slice(&fs::read(&catalog_path).unwrap()).unwrap();
    target["permission_profiles"]["CODE_WRITE"] =
        historical["permission_profiles"]["CODE_WRITE"].clone();
    fs::write(&catalog_path, serde_yaml::to_string(&target).unwrap()).unwrap();
    let current = workflow::compiler::compile_for_new_run_v1(
        f.root
            .join("examples/workflows/current.yaml")
            .to_str()
            .unwrap(),
        catalog_path.to_str().unwrap(),
    )
    .unwrap()
    .into_plan();
    request["target"]["expected_workflow_snapshot_hash"] =
        json!(format!("sha256:{}", current.workflow_snapshot_hash));
    request["target"]["expected_catalog_snapshot_hash"] =
        json!(format!("sha256:{}", current.catalog_snapshot_hash));
    let policy_error = workflow::carry_forward::freeze_continuation_target_v1(
        current,
        catalog_path.to_str().unwrap(),
    )
    .unwrap_err();
    assert!(format!("{policy_error:#}")
        .contains("target_policy_incompatible: explicit headless capability required"));
    let before_files = private_file_digests(&f.root);
    let before_index = fs::read(f.root.join(".git/index")).unwrap();
    let before_db = database_rows(&f.pool).await;
    let before_witness = witness(&f.pool, f.input.source_run_id).await;
    let mut preview_request = request.clone();
    for key in ["caller_request_id", "expected_plan_sha256", "reason"] {
        preview_request.as_object_mut().unwrap().remove(key);
    }
    let held = service
        .call("runs.continuation_preview", &principal, preview_request)
        .await
        .unwrap();
    assert_eq!(held["schema_version"], "run_carry_forward_preview_hold_v1");
    // Preview classifies the compiler's outer target-profile context first.
    assert_eq!(
        held["holds"],
        json!([{"code":"target_profile_invalid","message":"target_profile_invalid"}])
    );
    assert_eq!(
        witness(&f.pool, f.input.source_run_id).await,
        before_witness
    );
    assert_eq!(database_rows(&f.pool).await, before_db);
    assert_eq!(private_file_digests(&f.root), before_files);
    assert_eq!(fs::read(f.root.join(".git/index")).unwrap(), before_index);

    let denied = service
        .call("runs.continue_blocked", &principal, request.clone())
        .await
        .unwrap();
    assert_eq!(denied["status"], "denied");
    assert_eq!(denied["denial"]["code"], "target_profile_invalid");
    assert!(denied["operation"].is_null());
    assert!(denied["journal_id"].is_null());
    let mut after_db = database_rows(&f.pool).await;
    let receipts = after_db.get_mut("run_continuation_commands").unwrap();
    assert!(before_db["run_continuation_commands"].is_empty());
    assert_eq!(
        receipts.len(),
        1,
        "exactly one durable denial receipt is permitted"
    );
    let receipt: String = sqlx::query_scalar("SELECT result_json FROM run_continuation_commands WHERE source_run_id=? AND caller_request_id=? AND command='runs.continue_blocked'")
        .bind(f.input.source_run_id.to_string()).bind(request["caller_request_id"].as_str().unwrap())
        .fetch_one(&f.pool).await.unwrap();
    assert_eq!(serde_json::from_str::<Value>(&receipt).unwrap(), denied);
    receipts.clear();
    assert_eq!(
        after_db, before_db,
        "no source row, reservation, fence, job, artifact or approval mutation"
    );
    // The raw canonical witness includes that audit receipt; do not call it unchanged.
    let denied_witness = witness(&f.pool, f.input.source_run_id).await;
    assert_ne!(denied_witness, before_witness);
    let replay = service
        .call("runs.continue_blocked", &principal, request)
        .await
        .unwrap();
    assert_eq!(replay["status"], "replayed");
    assert_eq!(replay["replay_of"], "denied");
    assert_eq!(replay["denial"], denied["denial"]);
    assert_eq!(
        witness(&f.pool, f.input.source_run_id).await,
        denied_witness
    );
    assert_eq!(private_file_digests(&f.root), before_files);
    assert_eq!(fs::read(f.root.join(".git/index")).unwrap(), before_index);
    assert_eq!(fs::read_dir(&f.owned).unwrap().count(), 0);
    let retained = db::repos::runs::find_by_id(&f.pool, f.input.source_run_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(retained.catalog_snapshot_json.as_deref().unwrap()).unwrap(),
        historical
    );
}

#[tokio::test]
async fn structured_carried_reference_reopens_against_original_catalog_after_current_declaration_removal(
) {
    let mut f = Fixture::new().await.with_rollout_seed().await;
    let path = "quality-gate/blocker-boundary-human-decision.json";
    fs::create_dir_all(f.metadata.join("quality-gate")).unwrap();
    fs::write(f.metadata.join(path), HUMAN_BYTES).unwrap();
    let original = Uuid::new_v4();
    f.artifact(
        original,
        "blocker_boundary_human_decision",
        HUMAN_SCHEMA,
        path,
        HUMAN_BYTES,
    )
    .await;
    let original_run = db::repos::runs::find_by_id(&f.pool, f.input.source_run_id)
        .await
        .unwrap()
        .unwrap();
    let (service, principal, _, _, op) = f.service_prepare().await;
    let manifest = read_manifest(&op).await.unwrap();
    let activated = service.call("runs.continuation_activate", &principal, json!({
        "operation_id":op.operation_id,"expected_version":op.version,
        "expected_manifest_sha256":ContentDigest::from_sha256_hex(op.manifest_sha256.as_deref().unwrap()).unwrap(),
        "caller_request_id":Uuid::new_v4()
    })).await.unwrap();
    assert_eq!(activated["status"], "accepted", "{activated:#}");
    let successor = manifest.successor.id;
    let installed = db::repos::run_continuation_inputs::references(&f.pool, successor)
        .await
        .unwrap()
        .into_iter()
        .find(|input| input.original_artifact_id == original.to_string())
        .unwrap();
    assert_eq!(installed.role, "reference_only");
    assert_eq!(installed.source_schema, HUMAN_SCHEMA);
    let seed =
        db::repos::run_continuation_inputs::execution_seed(&f.pool, successor, "proposal_current")
            .await
            .unwrap()
            .unwrap();

    // Historical successor frontier setup only, not a claim of executed review/approval.
    sqlx::query("UPDATE work_items SET status='completed' WHERE run_id=? AND kind='advance_run'")
        .bind(successor.to_string())
        .execute(&f.pool)
        .await
        .unwrap();
    sqlx::query("UPDATE runs SET status='blocked',current_state=? WHERE id=?")
        .bind(PREPARE)
        .bind(successor.to_string())
        .execute(&f.pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO stage_executions(id,run_id,stage_id,label,status,iteration,attempt_number,started_at) VALUES (?,?,?,'Implementation','blocked',0,1,'2026-09-20T00:00:00Z')")
        .bind(Uuid::new_v4().to_string()).bind(successor.to_string()).bind(PREPARE).execute(&f.pool).await.unwrap();
    sqlx::query("INSERT INTO approvals(id,run_id,stage_id,decision,requested_at,decided_at) VALUES (?,?,'state_6_implementation_approval','granted','2026-09-20T00:00:00Z','2026-09-20T01:00:00Z')")
        .bind(Uuid::new_v4().to_string()).bind(successor.to_string()).execute(&f.pool).await.unwrap();
    let run = db::repos::runs::find_by_id(&f.pool, successor)
        .await
        .unwrap()
        .unwrap();
    let input = PreviewInput {
        source_run_id: successor,
        target: PreviewTarget {
            workflow_yaml_path: f.input.target.workflow_yaml_path.clone(),
            agent_catalog_yaml_path: f.input.target.agent_catalog_yaml_path.clone(),
            expected_workflow_snapshot_hash: None,
            expected_catalog_snapshot_hash: None,
            delivery_configuration_json: run.delivery_configuration_json.clone().unwrap(),
        },
        selection: PreviewSelection {
            proposal_input: InputReference::CarriedInput {
                id: seed.input_id.parse().unwrap(),
            },
            reference_inputs: vec![],
            include_dirty_work: true,
            excluded_workspace_paths: BTreeMap::new(),
        },
        profile: "implementation_restart_v1".into(),
        cursor: None,
        limit: None,
    };
    let PreviewOutcome::Page {
        prepared: before, ..
    } = preview(&f.pool, input.clone()).await.unwrap()
    else {
        panic!("installed structured history must be eligible before target declaration removal");
    };
    let catalog = f.root.join("examples/agents/agents.yaml");
    let mut current: Value = serde_yaml::from_slice(&fs::read(&catalog).unwrap()).unwrap();
    assert!(current["contracts"]
        .as_object_mut()
        .unwrap()
        .remove(HUMAN_SCHEMA)
        .is_some());
    fs::write(&catalog, serde_yaml::to_string(&current).unwrap()).unwrap();
    drop(service);
    f.pool.close().await;
    f.pool = db::pool::create_pool(&format!(
        "sqlite://{}?mode=rwc",
        f.root.parent().unwrap().join("fixture.db").display()
    ))
    .await
    .unwrap();
    let before_witness = witness(&f.pool, successor).await;
    let before_files = private_file_digests(&f.root);
    let before_owned = private_file_digests(&f.owned);
    let outcome = preview(&f.pool, input).await.unwrap();
    let PreviewOutcome::Page { prepared, .. } = outcome else {
        panic!("structured carried reopen held: {outcome:?}")
    };
    let reference = prepared
        .inputs()
        .iter()
        .find(|input| input.original_artifact_id == original)
        .unwrap();
    assert_eq!(
        reference.reference,
        InputReference::CarriedInput {
            id: installed.input_id.parse().unwrap()
        }
    );
    assert_eq!(reference.original_run_id, f.input.source_run_id);
    assert_eq!(reference.immediate_source_run_id, successor);
    assert_eq!(reference.role, EntryRole::ReferenceOnly);
    assert!(reference.mandatory);
    assert_eq!(reference.source_contract, HUMAN_SCHEMA);
    assert_eq!(reference.sha256, ContentDigest::of(HUMAN_BYTES));
    assert_eq!(
        reference.sha256,
        before
            .inputs()
            .iter()
            .find(|input| input.original_artifact_id == original)
            .unwrap()
            .sha256
    );
    assert_eq!(reference.historical_approval_relation["authority"], "none");
    assert_eq!(
        reference.historical_approval_relation["fresh_review_required"],
        true
    );
    assert_eq!(fs::read(reference.source_path()).unwrap(), HUMAN_BYTES);
    let target: Value =
        serde_json::from_str(&prepared.target().plan.catalog_snapshot_json).unwrap();
    assert!(target["contracts"].get(HUMAN_SCHEMA).is_none());
    let retained = db::repos::runs::find_by_id(&f.pool, f.input.source_run_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        retained.catalog_snapshot_json,
        original_run.catalog_snapshot_json
    );
    assert_eq!(
        retained.catalog_snapshot_hash,
        original_run.catalog_snapshot_hash
    );
    let frozen: Value =
        serde_json::from_str(retained.catalog_snapshot_json.as_deref().unwrap()).unwrap();
    assert_eq!(
        frozen["contracts"][HUMAN_SCHEMA]["required_fields"],
        json!(["schema_version", "decision_label"])
    );
    assert_eq!(witness(&f.pool, successor).await, before_witness);
    assert_eq!(private_file_digests(&f.root), before_files);
    assert_eq!(private_file_digests(&f.owned), before_owned);
}

#[tokio::test]
async fn legacy_external_skill_without_authenticated_bytes_holds_without_source_rewrite() {
    let f = Fixture::new().await;
    f.plan().await;
    let source = db::repos::runs::find_by_id(&f.pool, f.input.source_run_id)
        .await
        .unwrap()
        .unwrap();
    let mut historical: Value =
        serde_json::from_str(source.catalog_snapshot_json.as_deref().unwrap()).unwrap();
    let external = historical["skills"]
        .as_object()
        .unwrap()
        .values()
        .find(|skill| skill["type"] == "external_skill")
        .unwrap();
    assert!(f
        .root
        .join("examples/agents")
        .join(external["path"].as_str().unwrap())
        .exists());
    historical
        .as_object_mut()
        .unwrap()
        .remove("chainworks_compiled");
    historical["catalog_snapshot_format_version"] = json!(1);
    set_source_catalog(&f, &historical).await;
    let error = workflow::compiler::compile_from_snapshot_json(
        source.workflow_snapshot_json.as_deref().unwrap(),
        &serde_json::to_string(&historical).unwrap(),
        source.agent_catalog_yaml_path.as_deref().unwrap(),
    )
    .unwrap_err();
    assert!(format!("{error:#}").contains("legacy frozen catalog cannot reference external skill"));
    let before_witness = witness(&f.pool, f.input.source_run_id).await;
    let before_db = database_rows(&f.pool).await;
    let before_files = private_file_digests(&f.root);
    let outcome = preview(&f.pool, f.input.clone()).await.unwrap();
    let PreviewOutcome::Held { holds } = outcome else {
        panic!("expected legacy compatibility hold")
    };
    assert_eq!(holds.len(), 1);
    assert_eq!(holds[0].code.as_str(), "unsupported_frontier");
    assert_eq!(
        witness(&f.pool, f.input.source_run_id).await,
        before_witness
    );
    assert_eq!(database_rows(&f.pool).await, before_db);
    assert_eq!(private_file_digests(&f.root), before_files);
    assert_eq!(fs::read_dir(&f.owned).unwrap().count(), 0);
}
