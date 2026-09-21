use domain::CapabilityToolId;
use mcp_server::tools;
use mcp_server::{protocol::JsonRpcRequest, server::McpServer};
use serde_json::{json, Value};
use std::sync::Arc;
use std::sync::Mutex;
use tools::run_continuations::{ContinuationService, ContinuationServiceError};
use tower::ServiceExt;

const SOURCE: &str = "00000000-0000-4000-8000-000000000039";
const OPERATION: &str = "00000000-0000-4000-8000-000000000040";
const REQUEST: &str = "00000000-0000-4000-8000-000000000041";
const SUCCESSOR: &str = "00000000-0000-4000-8000-000000000042";

async fn server() -> (McpServer, sqlx::SqlitePool) {
    let pool = db::pool::create_pool("sqlite::memory:").await.unwrap();
    db::writer::register_shared_writer(&pool, Arc::new(db::writer::DbWriter::new(pool.clone())))
        .await
        .unwrap();
    let (events, _) = tokio::sync::broadcast::channel(16);
    let handler = Arc::new(engine::command_handler::CommandHandler::new(
        pool.clone(),
        events,
        engine::work_queue::WorkQueue::new(pool.clone()),
    ));
    (
        McpServer::new(pool.clone(), handler, auth::PrincipalTable::test_fixture()),
        pool,
    )
}

fn operator() -> auth::Principal {
    let mut p = auth::Principal::new("p039-test", auth::PrincipalClass::Operator);
    p.tool_capabilities = TOOLS.into_iter().map(|(_, id)| id).collect();
    p
}

fn call(name: &str, arguments: Value) -> JsonRpcRequest {
    serde_json::from_value(json!({"jsonrpc":"2.0","id":39,"method":"tools/call",
        "params":{"name":name,"arguments":arguments}}))
    .unwrap()
}

fn request(name: &str) -> Value {
    let mut value = match name {
        "runs.continuation_preview" | "runs.continue_blocked" => json!({
            "run_id":SOURCE,"target":{
                "workflow_yaml_path":"examples/workflows/target.yaml",
                "agent_catalog_yaml_path":"examples/agents/agents.yaml",
                "expected_workflow_snapshot_hash":format!("sha256:{}", "a".repeat(64)),
                "expected_catalog_snapshot_hash":format!("sha256:{}", "b".repeat(64)),
                "delivery_configuration_json":json!({"repo_identifier":"fixture", "repo_root":"/fixture",
                    "base_branch":"main", "worktree_base_path":"/fixture/worktrees", "target_branch":"successor"}).to_string()
            },"selection":{"proposal_input":{"kind":"artifact","id":SOURCE},
                "reference_inputs":[],"include_dirty_work":true,"excluded_workspace_paths":[]},
            "profile":"implementation_restart_v1"
        }),
        _ => json!({"operation_id":OPERATION}),
    };
    if name != "runs.continuation_preview" && name != "runs.continuation_get" {
        value["caller_request_id"] = json!(REQUEST);
        if name == "runs.continue_blocked" {
            value["expected_plan_sha256"] = json!(format!("sha256:{}", "c".repeat(64)));
        } else {
            value["expected_version"] = json!(1);
        }
        if name == "runs.continuation_activate" {
            value["expected_manifest_sha256"] = json!(format!("sha256:{}", "d".repeat(64)));
        } else {
            value["reason"] = json!("explicit fixture intent");
        }
    }
    value
}

async fn http(mcp: Arc<McpServer>, body: String) -> (axum::http::StatusCode, Value) {
    let response = mcp_server::http::routes(mcp)
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/mcp")
                .header("authorization", "Bearer test-token-xxxxxxxxxxxxxxxxxxxxx")
                .extension(graphql_server::request_id::RequestId(
                    "p039-correlation".into(),
                ))
                .body(axum::body::Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), 2 * 1024 * 1024)
        .await
        .unwrap();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

const TOOLS: [(&str, CapabilityToolId); 6] = [
    (
        "runs.continuation_preview",
        CapabilityToolId::RunsContinuationPreview,
    ),
    (
        "runs.continue_blocked",
        CapabilityToolId::RunsContinueBlocked,
    ),
    (
        "runs.continuation_get",
        CapabilityToolId::RunsContinuationGet,
    ),
    (
        "runs.continuation_activate",
        CapabilityToolId::RunsContinuationActivate,
    ),
    (
        "runs.continuation_reconcile",
        CapabilityToolId::RunsContinuationReconcile,
    ),
    (
        "runs.continuation_abort",
        CapabilityToolId::RunsContinuationAbort,
    ),
];

#[test]
fn p039_registration_has_six_exact_capabilities_aliases_and_closed_schemas() {
    for (name, id) in TOOLS {
        assert_eq!(tools::capability_id_for(name), Some(id));
        assert!(tools::all_capability_tool_ids().contains(&id));
        assert_eq!(tools::capability_id_for(&name.replace('.', "_")), Some(id));
        let spec = tools::mcp_tool_for(id);
        assert_eq!(spec.name, name);
        assert_eq!(spec.input_schema["additionalProperties"], false);
        assert!(spec.output_schema.is_some());
        assert!(spec.input_schema["properties"]
            .get("idempotency_key")
            .is_none());
    }
}

#[tokio::test]
async fn p039_runtime_health_exposes_bounded_live_metrics_alongside_p094() {
    let (_, pool) = server().await;
    let before = tools::runtime::execute(json!({}), &pool, None)
        .await
        .unwrap();
    let expected_names = [
        "run_continuation_admission_total",
        "run_continuation_phase_duration_ms",
        "run_continuation_hold_total",
        "run_continuation_source_guard_denied_total",
        "run_continuation_preserved_bytes_total",
        "run_continuation_activation_total",
    ];
    assert_eq!(
        before["runCarryForward"]["metricNames"],
        json!(expected_names)
    );
    let previous_bytes = before["runCarryForward"]["metricValues"]
        ["run_continuation_preserved_bytes_total"]
        .as_u64()
        .unwrap();
    db::metrics::record_continuation_preserved_bytes(17);
    db::metrics::record_continuation_phase_duration(
        domain::run_carry_forward_api::ContinuationPhase::Preparing,
        std::time::Duration::from_millis(23),
    );

    let after = tools::runtime::execute(json!({}), &pool, None)
        .await
        .unwrap();
    assert_eq!(after["schemaVersion"], "runtime_health.v1");
    assert_eq!(
        after["qualityGateBoundary"]["schemaVersion"],
        "quality_gate_boundary_runtime_v1"
    );
    let readback = after["runCarryForward"].as_object().unwrap();
    assert_eq!(readback.len(), 2);
    assert_eq!(readback["metricNames"], json!(expected_names));
    let values = readback["metricValues"].as_object().unwrap();
    assert_eq!(values.len(), expected_names.len());
    for name in expected_names {
        if name != "run_continuation_phase_duration_ms" {
            assert!(values[name].is_u64(), "{name}: {}", values[name]);
        }
    }
    assert!(
        values["run_continuation_preserved_bytes_total"]
            .as_u64()
            .unwrap()
            >= previous_bytes + 17
    );
    let phases = values["run_continuation_phase_duration_ms"]
        .as_object()
        .unwrap();
    assert!(phases.contains_key("preparing"));
    for (phase, samples) in phases {
        assert!([
            "preparing",
            "prepared",
            "aborting",
            "needs_reconciliation",
            "activated",
            "aborted"
        ]
        .contains(&phase.as_str()));
        assert_eq!(samples.as_object().unwrap().len(), 2);
        assert!((1..=1024).contains(&samples["sample_count"].as_u64().unwrap()));
        assert!(samples["p95"].is_u64());
    }
}

#[tokio::test]
async fn p039_raw_duplicates_rejected_before_value_even_with_escaped_names() {
    let (mcp, _) = server().await;
    let mcp = Arc::new(mcp);
    for arguments in [
        format!(r#"{{"operation_id":"{OPERATION}","operation_id":"{OPERATION}"}}"#),
        format!(r#"{{"operation_id":"{OPERATION}","\u006fperation_id":"{OPERATION}"}}"#),
        r#"{"nested":{"é":1,"\u00e9":2}}"#.into(),
        r#"{"selection":{"proposal_input":{"kind":"artifact","kind":"carried_input"}}}"#.into(),
    ] {
        let body = format!(
            r#"{{"jsonrpc":"2.0","id":39,"method":"tools/call","params":{{"name":"runs\u002econtinuation_get","arguments":{arguments}}}}}"#
        );
        let (_, result) = http(mcp.clone(), body).await;
        assert_eq!(result["error"]["code"], -32602, "{result}");
        assert!(result.get("result").is_none());
    }
}

#[test]
fn p039_transport_budget_is_one_mib() {
    assert_eq!(mcp_server::http::MCP_HTTP_BODY_LIMIT_BYTES, 1024 * 1024);
}

/// A denial-only adapter: never pretends admission, activation or engine success.
#[derive(Default)]
struct DenyService {
    calls: Mutex<Vec<(String, String, Value)>>,
    error: Option<ContinuationServiceError>,
    malformed: bool,
}

#[async_trait::async_trait]
impl ContinuationService for DenyService {
    async fn call(
        &self,
        tool: &str,
        principal: &auth::Principal,
        arguments: Value,
    ) -> anyhow::Result<Value> {
        self.calls
            .lock()
            .unwrap()
            .push((tool.into(), principal.id.clone(), arguments.clone()));
        if let Some(error) = self.error {
            return Err(error.into());
        }
        if self.malformed {
            return Ok(json!({"status":"accepted","private_root":"/must/not/leak"}));
        }
        if !tools::run_continuations::is_mutation(tool) {
            return Err(ContinuationServiceError::AdmissionUnavailable.into());
        }
        Ok(
            json!({"schema_version":"run_continuation_command_result_v1","status":"denied",
            "caller_request_id":arguments["caller_request_id"],"journal_id":null,"operation":null,
            "denial":{"code":"rollout_hold","message":"Fixture denies admission"},"next_action":"resolve_hold","replay_of":null}),
        )
    }
}

async fn seed(pool: &sqlx::SqlitePool, activated: bool) {
    sqlx::query(
        "INSERT INTO ideas(id,title,body,created_at) VALUES (?,'Idea','Body','2026-09-20')",
    )
    .bind(SOURCE)
    .execute(pool)
    .await
    .unwrap();
    for run in [SOURCE, SUCCESSOR] {
        sqlx::query("INSERT INTO runs(id,idea_id,status,workflow_id,workflow_title,workspace_root,artifact_root,started_at) VALUES (?,?,?,'w','Workflow','/fixture','/fixture/meta','2026-09-20T00:00:00Z')")
            .bind(run).bind(SOURCE).bind(if run == SOURCE { "blocked" } else { "completed" }).execute(pool).await.unwrap();
    }
    sqlx::query("INSERT INTO command_journal(id,command_type,payload_json,created_at) VALUES (?,'ContinueBlocked','{}','2026-09-20')")
        .bind(OPERATION).execute(pool).await.unwrap();
    sqlx::query("INSERT INTO run_continuations(operation_id,source_run_id,idea_id,reserved_successor_run_id,successor_run_id,caller_fingerprint,caller_request_id,intent_sha256,profile,phase,plan_ref,plan_sha256,target_ref,source_witness_sha256,manifest_ref,manifest_sha256,journal_id,created_at,updated_at,deadline_at) VALUES (?,?,?,?,?,'fixture',?,?,'implementation_restart_v1',?,'plan.json',?,'target.json',?,'manifest.json',?,?,'2026-09-20T00:00:00Z','2026-09-20T00:00:00Z','2026-09-21T00:00:00Z')")
        .bind(OPERATION).bind(SOURCE).bind(SOURCE).bind(SUCCESSOR).bind(activated.then_some(SUCCESSOR))
        .bind(REQUEST).bind("a".repeat(64)).bind(if activated {"activated"} else {"preparing"})
        .bind("b".repeat(64)).bind("c".repeat(64)).bind("d".repeat(64)).bind(OPERATION)
        .execute(pool).await.unwrap();
}

#[tokio::test]
async fn p039_existing_run_and_report_reads_receive_compact_scoped_links() {
    let (mcp, pool) = server().await;
    seed(&pool, true).await;
    let mut p = auth::Principal::new("existing-read", auth::PrincipalClass::Operator);
    p.tool_capabilities = [
        CapabilityToolId::RunsGet,
        CapabilityToolId::RunsList,
        CapabilityToolId::ReportsGet,
    ]
    .into_iter()
    .collect();
    // Existing run reads require no new P039 capability.
    for name in ["runs.get", "reports.get"] {
        let response = mcp
            .handle_request(call(name, json!({"run_id":SOURCE})), &p)
            .await;
        assert!(response.error.is_none(), "{name}: {:?}", response.error);
        let value: Value = serde_json::from_str(
            response.result.unwrap()["content"][0]["text"]
                .as_str()
                .unwrap(),
        )
        .unwrap();
        assert_eq!(value["continued_as_run_id"], SUCCESSOR, "{name}: {value}");
        assert_eq!(
            value["carry_forward_readback"]["outgoing"]["operation_id"],
            OPERATION
        );
        assert_eq!(
            value["carry_forward_readback"]["outgoing"]["admission"]["reconcile_available"],
            false
        );
        assert!(value["carry_forward_readback"]
            .get("manifest_page")
            .is_none());
        if let Some(reports) = value["reports"].as_array() {
            for report in reports {
                assert_eq!(report["continued_as_run_id"], SUCCESSOR);
            }
        }
    }
    db::repos::projections::rebuild_all_for_run(&pool, SOURCE.parse().unwrap())
        .await
        .unwrap();
    let response = mcp.handle_request(call("runs.list", json!({})), &p).await;
    assert!(response.error.is_none(), "{:?}", response.error);
    let rows: Value = serde_json::from_str(
        response.result.unwrap()["content"][0]["text"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(rows[0]["continued_as_run_id"], SUCCESSOR);
    let compact = rows[0]["carry_forward_readback"].to_string();
    for forbidden in ["manifest_page", "plan_ref", "target_ref", "/fixture"] {
        assert!(!compact.contains(forbidden));
    }
    let mcp = mcp.with_continuation_service(Arc::new(DenyService::default()));
    let response = mcp
        .handle_request(call("runs.get", json!({"run_id":SOURCE})), &p)
        .await;
    let value: Value = serde_json::from_str(
        response.result.unwrap()["content"][0]["text"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        value["carry_forward_readback"]["outgoing"]["admission"]["reconcile_available"],
        true
    );
    p.run_scope = Some(vec![SOURCE.into()]);
    let response = mcp
        .handle_request(call("runs.get", json!({"run_id":SOURCE})), &p)
        .await;
    let value: Value = serde_json::from_str(
        response.result.unwrap()["content"][0]["text"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    assert!(value["carry_forward_readback"].is_null());
    assert!(value["continued_as_run_id"].is_null());
}

#[tokio::test]
async fn p039_resources_preserve_middle_chain_parity_scope_and_abort() {
    async fn resource(mcp: &McpServer, uri: &str, p: &auth::Principal) -> Value {
        let response = mcp
            .handle_request(
                serde_json::from_value(json!({
                    "jsonrpc":"2.0","id":39,"method":"resources/read","params":{"uri":uri}
                }))
                .unwrap(),
                p,
            )
            .await;
        assert!(response.error.is_none(), "{uri}: {:?}", response.error);
        serde_json::from_str(
            response.result.unwrap()["contents"][0]["text"]
                .as_str()
                .unwrap(),
        )
        .unwrap()
    }
    let (mcp, pool) = server().await;
    let p = auth::Principal::new("resource-reader", auth::PrincipalClass::Operator);
    let outgoing = uuid::Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO ideas(id,title,body,created_at) VALUES (?,'Idea','Body','2026-09-20')",
    )
    .bind(SOURCE)
    .execute(&pool)
    .await
    .unwrap();
    for (id, status) in [(SOURCE, "completed"), (SUCCESSOR, "blocked")] {
        sqlx::query("INSERT INTO runs(id,idea_id,status,workflow_id,workflow_title,workspace_root,artifact_root,started_at) VALUES (?,?,?,'w','W','/fixture','/fixture/meta','2026-09-20T00:00:00Z')")
            .bind(id).bind(SOURCE).bind(status).execute(&pool).await.unwrap();
        db::repos::projections::rebuild_all_for_run(&pool, id.parse().unwrap())
            .await
            .unwrap();
    }
    for uri in [
        format!("run://{SOURCE}"),
        format!("chainworks://runs/{SOURCE}"),
        format!("report://{SOURCE}"),
    ] {
        let old = resource(&mcp, &uri, &p).await;
        for name in [
            "continued_from_run_id",
            "continued_as_run_id",
            "carry_forward_readback",
        ] {
            assert_eq!(old.get(name), Some(&Value::Null), "{uri}: {name}");
        }
    }
    // Existing report metadata, not a fabricated provider execution or release.
    sqlx::query("INSERT INTO artifacts(id,run_id,stage_id,agent_id,name,contract_id,format,file_path,provider,created_at,report_kind) VALUES (?,?, 'fixture','fixture','run_report','run_report','json','/fixture/meta/report.json','fixture','2026-09-20T00:00:00Z','run')")
        .bind(uuid::Uuid::new_v4().to_string()).bind(SUCCESSOR).execute(&pool).await.unwrap();
    for (op, source, successor, phase) in [
        (OPERATION, SOURCE, Some(SUCCESSOR), "activated"),
        (outgoing.as_str(), SUCCESSOR, None, "needs_reconciliation"),
    ] {
        sqlx::query("INSERT INTO command_journal(id,command_type,payload_json,created_at) VALUES (?,'ContinueBlocked','{}','2026-09-20T00:00:00Z')")
            .bind(op).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO run_continuations(operation_id,source_run_id,idea_id,reserved_successor_run_id,successor_run_id,caller_fingerprint,caller_request_id,intent_sha256,profile,phase,plan_ref,plan_sha256,target_ref,source_witness_sha256,manifest_ref,manifest_sha256,journal_id,holds_json,created_at,updated_at,deadline_at) VALUES (?,?,?,?,?,'fixture',?,?,'implementation_restart_v1',?,'private-plan',?,'private-target',?,'private-manifest',?,?,?,'2026-09-20T00:00:00Z','2026-09-20T00:00:00Z','2026-09-21T00:00:00Z')")
            .bind(op).bind(source).bind(SOURCE).bind(successor.map(str::to_owned).unwrap_or_else(||uuid::Uuid::new_v4().to_string())).bind(successor)
            .bind(op).bind("a".repeat(64)).bind(phase).bind("b".repeat(64)).bind("c".repeat(64)).bind("d".repeat(64)).bind(op)
            .bind(if successor.is_some() {"[]"} else {"[\"effect_outcome_unknown\"]"}).execute(&pool).await.unwrap();
    }
    let mcp = mcp.with_continuation_service(Arc::new(
        engine::run_carry_forward::CarryForwardService::new(
            pool.clone(),
            engine::run_carry_forward::ServiceConfig {
                enabled: false,
                owned_parent: "/unprovisioned".into(),
            },
        ),
    ));
    for aborted in [false, true] {
        if aborted {
            let op = db::repos::run_continuations::begin_abort(&pool, &outgoing, 1)
                .await
                .unwrap();
            db::repos::run_continuations::finish_abort(&pool, &outgoing, op.version)
                .await
                .unwrap();
        }
        let tool = mcp
            .handle_request(call("runs.get", json!({"run_id":SUCCESSOR})), &p)
            .await;
        assert!(tool.error.is_none(), "{:?}", tool.error);
        let tool: Value =
            serde_json::from_str(tool.result.unwrap()["content"][0]["text"].as_str().unwrap())
                .unwrap();
        for uri in [
            format!("run://{SUCCESSOR}"),
            format!("chainworks://runs/{SUCCESSOR}"),
            format!("report://{SUCCESSOR}"),
            "chainworks://runs".into(),
        ] {
            let value = resource(&mcp, &uri, &p).await;
            let value = if uri == "chainworks://runs" {
                value
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|v| v["id"] == SUCCESSOR)
                    .unwrap()
            } else {
                &value
            };
            assert_eq!(value["continued_from_run_id"], SOURCE, "{uri}");
            assert!(value["continued_as_run_id"].is_null());
            assert_eq!(
                value["carry_forward_readback"]["incoming"]["operation_id"],
                OPERATION
            );
            assert_eq!(
                value["carry_forward_readback"]["incoming"]["admission"]["enabled"],
                false
            );
            assert_eq!(
                value["carry_forward_readback"]["incoming"]["admission"]["reconcile_available"],
                true
            );
            if aborted {
                assert!(value["carry_forward_readback"]["outgoing"].is_null());
            } else {
                assert_eq!(
                    value["carry_forward_readback"]["outgoing"]["operation_id"],
                    outgoing
                );
                assert_eq!(
                    value["carry_forward_readback"]["outgoing"]["holds"][0]["code"],
                    "effect_outcome_unknown"
                );
            }
            for name in [
                "continued_from_run_id",
                "continued_as_run_id",
                "carry_forward_readback",
            ] {
                assert_eq!(value[name], tool[name], "{uri}: {name}");
            }
            let compact = value["carry_forward_readback"].to_string();
            for forbidden in ["private-", "manifest_page", "caller_fingerprint"] {
                assert!(!compact.contains(forbidden));
            }
            if uri.starts_with("report://") {
                let artifacts = value["artifacts"].as_array().unwrap();
                assert_eq!(artifacts.len(), 1);
                assert_eq!(
                    artifacts[0]["carry_forward_readback"],
                    value["carry_forward_readback"]
                );
            }
            let mut scoped = p.clone();
            scoped.run_scope = Some(vec![SUCCESSOR.into()]);
            let scoped = resource(&mcp, &uri, &scoped).await;
            let scoped = if uri == "chainworks://runs" {
                scoped
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|v| v["id"] == SUCCESSOR)
                    .unwrap()
            } else {
                &scoped
            };
            assert!(scoped["continued_from_run_id"].is_null());
            assert!(scoped["carry_forward_readback"]["incoming"].is_null());
            if !aborted {
                assert_eq!(
                    scoped["carry_forward_readback"]["outgoing"]["operation_id"],
                    outgoing
                );
            }
        }
    }
}

#[tokio::test]
async fn p039_missing_service_denies_all_six_without_read_or_fake_success() {
    let (mcp, pool) = server().await;
    pool.close().await; // proves no canonical read/replay is needed to refuse admission.
    for (name, _) in TOOLS {
        let response = mcp
            .handle_request(call(name, request(name)), &operator())
            .await;
        assert!(response.result.is_none());
        let error = response.error.unwrap();
        assert_eq!(error.code, -32004, "{name}: {error:?}");
        assert_eq!(
            error.data.unwrap()["reason_code"],
            "P039_ADMISSION_UNAVAILABLE"
        );
    }
}

#[tokio::test]
async fn p039_all_dtos_validate_before_service_or_storage() {
    let service = Arc::new(DenyService::default());
    let (mcp, pool) = server().await;
    let mcp = mcp.with_continuation_service(service.clone());
    pool.close().await;
    for (name, _) in TOOLS {
        let mut bad = request(name);
        bad["source_status"] = json!("blocked");
        assert_eq!(
            mcp.handle_request(call(name, bad), &operator())
                .await
                .error
                .unwrap()
                .code,
            -32602
        );
        let mut bad = request(name);
        if tools::run_continuations::is_mutation(name) {
            bad["caller_request_id"] = json!("00000000-0000-7000-8000-000000000041");
        } else {
            bad["limit"] = json!(501);
        }
        assert_eq!(
            mcp.handle_request(call(name, bad), &operator())
                .await
                .error
                .unwrap()
                .code,
            -32602
        );
        let mut bad = request(name);
        bad["idempotency_key"] = json!("00000000-0000-7000-8000-000000000041");
        assert_eq!(
            mcp.handle_request(call(name, bad), &operator())
                .await
                .error
                .unwrap()
                .code,
            -32602
        );
    }
    for bad in [Value::Null, json!([]), json!({})] {
        assert_eq!(
            mcp.handle_request(call(TOOLS[0].0, bad), &operator())
                .await
                .error
                .unwrap()
                .code,
            -32602
        );
    }
    assert!(service.calls.lock().unwrap().is_empty());
}

#[tokio::test]
async fn p039_explicit_v4_mutations_delegate_canonical_name_but_never_fake_acceptance() {
    let service = Arc::new(DenyService::default());
    let (mcp, pool) = server().await;
    seed(&pool, false).await;
    let mcp = mcp.with_continuation_service(service.clone());
    for (name, _) in TOOLS
        .into_iter()
        .filter(|(name, _)| tools::run_continuations::is_mutation(name))
    {
        let response = mcp
            .handle_request(call(&name.replace('.', "_"), request(name)), &operator())
            .await;
        assert!(response.error.is_none(), "{name}: {response:?}");
        let result: Value = serde_json::from_str(
            response.result.unwrap()["content"][0]["text"]
                .as_str()
                .unwrap(),
        )
        .unwrap();
        assert_eq!(result["status"], "denied");
        assert_eq!(result["denial"]["code"], "rollout_hold");
        assert_eq!(result["caller_request_id"], REQUEST);
    }
    let calls = service.calls.lock().unwrap();
    assert_eq!(calls.len(), 4);
    for (name, principal, arguments) in calls.iter() {
        assert!(name.starts_with("runs."));
        assert_eq!(principal, "p039-test");
        assert_eq!(arguments, &request(name));
    }
    drop(calls);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM command_journal")
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM run_continuation_commands")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM work_items")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
}

#[tokio::test]
async fn p039_auth_before_lookup_and_replay_for_every_negative_identity() {
    let service = Arc::new(DenyService::default());
    let (mcp, pool) = server().await;
    let mcp = mcp.with_continuation_service(service.clone());
    pool.close().await; // any premature canonical lookup would produce INTERNAL, not a denial.
    for (name, id) in TOOLS {
        let mut missing = operator();
        missing.tool_capabilities.remove(&id);
        let mut narrow = operator();
        narrow.tool_capabilities = [CapabilityToolId::RunsStart].into();
        let mut empty = operator();
        empty.run_scope = Some(vec![]);
        for principal in [
            auth::Principal::new("default", auth::PrincipalClass::Operator),
            missing,
            narrow,
            empty,
        ] {
            let response = mcp
                .handle_request(call(name, request(name)), &principal)
                .await;
            assert_eq!(response.error.unwrap().code, -32004, "{name}");
        }
        if !tools::run_continuations::is_mutation(name) {
            continue;
        }
        let mut negatives = vec![];
        for class in [
            auth::PrincipalClass::Agent,
            auth::PrincipalClass::Observer,
            auth::PrincipalClass::ReadOnlyOperator,
        ] {
            let mut principal = operator();
            principal.class = class;
            negatives.push(principal);
        }
        for caller in [
            auth::CallerClass::Automation,
            auth::CallerClass::Observer,
            auth::CallerClass::DeveloperBreakGlass,
        ] {
            let mut principal = operator();
            principal.caller_class_override = Some(caller);
            negatives.push(principal);
        }
        let mut scoped = operator();
        scoped.run_scope = Some(vec![SOURCE.into()]);
        negatives.push(scoped);
        for principal in negatives {
            for _ in 0..2 {
                // same request cannot reveal or invoke a replay after loss of authority.
                assert_eq!(
                    mcp.handle_request(call(name, request(name)), &principal)
                        .await
                        .error
                        .unwrap()
                        .code,
                    -32004
                );
            }
        }
    }
    assert!(service.calls.lock().unwrap().is_empty());
}

#[tokio::test]
async fn p039_missing_and_foreign_operations_have_identical_denials_and_successor_is_scoped() {
    let service = Arc::new(DenyService::default());
    let (mcp, pool) = server().await;
    seed(&pool, true).await;
    let mcp = mcp.with_continuation_service(service.clone());
    for class in [
        auth::PrincipalClass::Operator,
        auth::PrincipalClass::ReadOnlyOperator,
        auth::PrincipalClass::Observer,
        auth::PrincipalClass::Agent,
    ] {
        let mut principal = operator();
        principal.class = class;
        principal.run_scope = Some(vec![REQUEST.into()]);
        let denied = mcp
            .handle_request(call(TOOLS[2].0, request(TOOLS[2].0)), &principal)
            .await;
        let missing = mcp
            .handle_request(
                call(TOOLS[2].0, json!({"operation_id":REQUEST})),
                &principal,
            )
            .await;
        assert_eq!(
            serde_json::to_value(&denied).unwrap(),
            serde_json::to_value(missing).unwrap()
        );
        assert_eq!(denied.error.unwrap().code, -32004);
        principal.run_scope = Some(vec![SOURCE.into()]);
        assert_eq!(
            mcp.handle_request(call(TOOLS[2].0, request(TOOLS[2].0)), &principal)
                .await
                .error
                .unwrap()
                .code,
            -32004
        );
    }
    assert!(service.calls.lock().unwrap().is_empty());
    let mut principal = operator();
    principal.run_scope = Some(vec![SOURCE.into(), SUCCESSOR.into()]);
    for class in [
        auth::PrincipalClass::Operator,
        auth::PrincipalClass::ReadOnlyOperator,
        auth::PrincipalClass::Observer,
        auth::PrincipalClass::Agent,
    ] {
        principal.class = class;
        let response = mcp
            .handle_request(call(TOOLS[2].0, request(TOOLS[2].0)), &principal)
            .await;
        assert_eq!(
            response.error.unwrap().data.unwrap()["reason_code"],
            "P039_ADMISSION_UNAVAILABLE"
        );
    }
    assert_eq!(service.calls.lock().unwrap().len(), 4);
}

#[tokio::test]
async fn p039_boundary_shadow_and_legacy_modes_do_not_create_authority() {
    let (mcp, _) = server().await;
    let mcp = mcp.with_boundary_policy(Arc::new(
        auth::boundary::BoundaryPolicy::from_embedded_with_mode(
            auth::boundary::PolicyMode::LegacyCompat,
        )
        .unwrap(),
    ));
    assert_eq!(
        mcp.handle_request(call(TOOLS[1].0, request(TOOLS[1].0)), &operator())
            .await
            .error
            .unwrap()
            .code,
        -32004
    );
    let (mcp, _) = server().await;
    let mut principal = operator();
    principal.caller_class_override = Some(auth::CallerClass::DeveloperBreakGlass);
    assert_eq!(
        mcp.handle_request(call(TOOLS[0].0, request(TOOLS[0].0)), &principal)
            .await
            .error
            .unwrap()
            .code,
        -32004
    );
}

#[tokio::test]
async fn p039_tools_list_has_full_input_and_output_schemas_without_default_grants() {
    let (mcp, _) = server().await;
    let list =
        || serde_json::from_value(json!({"jsonrpc":"2.0","id":1,"method":"tools/list"})).unwrap();
    let result = mcp
        .handle_request(list(), &operator())
        .await
        .result
        .unwrap();
    for (name, id) in TOOLS {
        let spec = tools::mcp_tool_for(id);
        let listed = result["tools"]
            .as_array()
            .unwrap()
            .iter()
            .find(|t| t["name"] == name.replace('.', "_"))
            .unwrap();
        assert_eq!(listed["inputSchema"], spec.input_schema);
        assert_eq!(listed["outputSchema"], spec.output_schema.unwrap());
    }
    let result = mcp
        .handle_request(
            list(),
            &auth::Principal::new("default", auth::PrincipalClass::Operator),
        )
        .await
        .result
        .unwrap();
    assert!(!result["tools"]
        .as_array()
        .unwrap()
        .iter()
        .any(|t| tools::run_continuations::is_tool(t["name"].as_str().unwrap())));
}

fn http_grants(mcp: &McpServer) {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    let path = dir.path().join("principals.json");
    std::fs::write(&path,json!({"schema_version":3,"principals":[{"id":"p039-test","class":"operator",
        "token":"test-token-xxxxxxxxxxxxxxxxxxxxx","surface_policies":{"mcp":{"allowed_tools":TOOLS.map(|(name,_)|name)}}}]}).to_string()).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
    mcp.live_principal_source()
        .replace(auth::PrincipalTable::load_or_bootstrap(&path).unwrap());
}

#[tokio::test]
async fn p039_uncertain_commit_data_is_exact_even_with_http_request_id() {
    let service = Arc::new(DenyService {
        error: Some(ContinuationServiceError::CommandOutcomeUnknown),
        ..Default::default()
    });
    let (mcp, pool) = server().await;
    seed(&pool, false).await;
    let mcp = Arc::new(mcp.with_continuation_service(service));
    http_grants(&mcp);
    let (_, result) = http(
        mcp,
        serde_json::to_string(&call(TOOLS[1].0, request(TOOLS[1].0))).unwrap(),
    )
    .await;
    assert_eq!(result["error"]["code"], -32603);
    assert_eq!(
        result["error"]["data"],
        json!({"schema_version":"p039_transport_error_v1","code":"command_outcome_unknown","next_action":"replay_same_request"})
    );
}

#[tokio::test]
async fn p039_revoked_http_bearer_cannot_replay() {
    let service = Arc::new(DenyService::default());
    let (mcp, pool) = server().await;
    seed(&pool, false).await;
    let mcp = Arc::new(mcp.with_continuation_service(service.clone()));
    http_grants(&mcp);
    let body = serde_json::to_string(&call(TOOLS[1].0, request(TOOLS[1].0))).unwrap();
    let (_, first) = http(mcp.clone(), body.clone()).await;
    assert!(first.get("error").is_none(), "{first}");
    mcp.live_principal_source()
        .replace(auth::PrincipalTable::test_fixture_disabled_token(
            "test-token-xxxxxxxxxxxxxxxxxxxxx",
            "p039-test",
        ));
    let (_, denied) = http(mcp, body).await;
    assert_eq!(denied["error"]["code"], -32004);
    assert!(denied.get("result").is_none());
    assert_eq!(service.calls.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn p039_real_service_http_durable_denial_replay_stale_cas_and_queued_abort() {
    use db::repos::run_continuations as operations;
    use engine::run_carry_forward::{CarryForwardService, ServiceConfig};

    async fn outcome(mcp: &Arc<McpServer>, name: &str, args: &Value) -> Value {
        let (status, response) = http(
            mcp.clone(),
            serde_json::to_string(&call(name, args.clone())).unwrap(),
        )
        .await;
        assert_eq!(status, axum::http::StatusCode::OK, "{response}");
        assert!(response.get("error").is_none(), "{response}");
        let value: Value =
            serde_json::from_str(response["result"]["content"][0]["text"].as_str().unwrap())
                .unwrap();
        serde_json::from_value::<domain::run_carry_forward_api::RunContinuationCommandResultV1>(
            value.clone(),
        )
        .unwrap()
        .validate()
        .unwrap();
        value
    }

    async fn counts(pool: &sqlx::SqlitePool) -> (i64, i64, i64) {
        sqlx::query_as("SELECT (SELECT COUNT(*) FROM run_continuation_commands), (SELECT COUNT(*) FROM command_journal), (SELECT COUNT(*) FROM work_items)")
            .fetch_one(pool).await.unwrap()
    }

    for prepared in [false, true] {
        let (mcp, pool) = server().await;
        sqlx::query(
            "INSERT INTO ideas(id,title,body,created_at) VALUES (?,'Idea','Body','2026-09-20')",
        )
        .bind(SOURCE)
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query("INSERT INTO runs(id,idea_id,status,workflow_id,workflow_title,workspace_root,artifact_root,started_at) VALUES (?,?,'blocked','w','Workflow','/fixture','/fixture/meta','2026-09-20T00:00:00Z')")
            .bind(SOURCE).bind(SOURCE).execute(&pool).await.unwrap();
        let mut op = operations::reserve(
            &pool,
            &operations::Reservation {
                source_run_id: SOURCE.into(),
                caller_fingerprint: "bookkeeping-fixture".into(),
                caller_request_id: uuid::Uuid::new_v4().to_string(),
                intent_sha256: "a".repeat(64),
                plan_ref: "plan.json".into(),
                plan_sha256: "b".repeat(64),
                target_ref: "target.json".into(),
                source_witness_sha256: "c".repeat(64),
                reason: "CF-28 database-only boundary fixture".into(),
            },
        )
        .await
        .unwrap();
        if prepared {
            // Bookkeeping only: no materialization is claimed or activation enabled.
            let claim = operations::claim_preparation_for(&pool, &op.operation_id)
                .await
                .unwrap()
                .unwrap();
            operations::begin_step(
                &pool,
                &op.operation_id,
                claim.generation,
                "fixture_bookkeeping",
                "{}",
                "fixture",
            )
            .await
            .unwrap();
            operations::finish_step(
                &pool,
                &op.operation_id,
                claim.generation,
                "fixture_bookkeeping",
                &"d".repeat(64),
            )
            .await
            .unwrap();
            op = operations::mark_prepared(
                &pool,
                &op.operation_id,
                claim.generation,
                "manifest.json",
                &"d".repeat(64),
            )
            .await
            .unwrap();
        }
        let temporary = tempfile::tempdir().unwrap();
        let unprovisioned = temporary.path().join("must-not-be-created");
        let service = Arc::new(CarryForwardService::new(
            pool.clone(),
            ServiceConfig {
                enabled: false,
                owned_parent: unprovisioned.clone(),
            },
        ));
        let mcp = Arc::new(mcp.with_continuation_service(service));
        http_grants(&mcp);
        let baseline = counts(&pool).await;
        assert_eq!(baseline, (1, 1, 1));

        for (index, (offset, code)) in [(0, "rollout_hold"), (1, "stale_operation_version")]
            .into_iter()
            .enumerate()
        {
            let args = json!({"operation_id":op.operation_id,
                "expected_version":op.version + offset,
                "expected_manifest_sha256":format!("sha256:{}", "d".repeat(64)),
                "caller_request_id":uuid::Uuid::new_v4().to_string()});
            let denied = outcome(&mcp, "runs.continuation_activate", &args).await;
            assert_eq!(denied["status"], "denied");
            assert_eq!(denied["denial"]["code"], code);
            assert_eq!(denied["operation"]["phase"], op.phase);
            assert_eq!(denied["operation"]["version"], op.version);
            let stored: String = sqlx::query_scalar(
                "SELECT result_json FROM run_continuation_commands WHERE caller_request_id=?",
            )
            .bind(args["caller_request_id"].as_str().unwrap())
            .fetch_one(&pool)
            .await
            .unwrap();
            assert_eq!(serde_json::from_str::<Value>(&stored).unwrap(), denied);
            let mut expected_replay = denied;
            expected_replay["status"] = json!("replayed");
            expected_replay["replay_of"] = json!("denied");
            assert_eq!(
                outcome(&mcp, "runs.continuation_activate", &args).await,
                expected_replay
            );
            assert_eq!(
                counts(&pool).await,
                (baseline.0 + index as i64 + 1, baseline.1, baseline.2)
            );
            let unchanged = operations::find(&pool, &op.operation_id)
                .await
                .unwrap()
                .unwrap();
            assert_eq!(
                (unchanged.phase, unchanged.version),
                (op.phase.clone(), op.version)
            );
        }

        let args = json!({"operation_id":op.operation_id,"expected_version":op.version,
            "caller_request_id":uuid::Uuid::new_v4().to_string(),"reason":"Cancel fixture reservation"});
        let accepted = outcome(&mcp, "runs.continuation_abort", &args).await;
        assert_eq!(accepted["status"], "accepted");
        assert_eq!(accepted["operation"]["phase"], "aborted");
        assert_eq!(accepted["operation"]["version"], op.version + 2);
        assert!(accepted["operation"]["successor_run_id"].is_null());
        assert!(accepted["journal_id"].is_string());
        let stored: String = sqlx::query_scalar(
            "SELECT result_json FROM run_continuation_commands WHERE caller_request_id=?",
        )
        .bind(args["caller_request_id"].as_str().unwrap())
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(serde_json::from_str::<Value>(&stored).unwrap(), accepted);
        let mut expected_replay = accepted;
        expected_replay["status"] = json!("replayed");
        expected_replay["replay_of"] = json!("accepted");
        assert_eq!(
            outcome(&mcp, "runs.continuation_abort", &args).await,
            expected_replay
        );
        let final_counts = (baseline.0 + 3, baseline.1 + 1, baseline.2);
        assert_eq!(counts(&pool).await, final_counts);
        let work_status: String = sqlx::query_scalar(
            "SELECT status FROM work_items WHERE kind='prepare_run_continuation'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(
            work_status,
            if prepared { "completed" } else { "cancelled" }
        );
        let durable: (i64, i64, String) = sqlx::query_as("SELECT (SELECT COUNT(*) FROM runs), (SELECT COUNT(*) FROM run_execution_fences), (SELECT status FROM runs WHERE id=?)")
            .bind(SOURCE).fetch_one(&pool).await.unwrap();
        assert_eq!(durable, (1, 0, "blocked".into()));

        mcp.live_principal_source()
            .replace(auth::PrincipalTable::test_fixture_disabled_token(
                "test-token-xxxxxxxxxxxxxxxxxxxxx",
                "p039-test",
            ));
        let (_, revoked) = http(
            mcp,
            serde_json::to_string(&call("runs.continuation_abort", args)).unwrap(),
        )
        .await;
        assert_eq!(revoked["error"]["code"], -32004);
        assert!(revoked.get("result").is_none());
        assert_eq!(counts(&pool).await, final_counts);
        assert!(!unprovisioned.exists());
    }
}

#[tokio::test]
async fn p039_malformed_service_output_is_redacted_internal_never_accepted() {
    let (mcp, pool) = server().await;
    seed(&pool, false).await;
    let mcp = mcp.with_continuation_service(Arc::new(DenyService {
        malformed: true,
        ..Default::default()
    }));
    let response = mcp
        .handle_request(call(TOOLS[1].0, request(TOOLS[1].0)), &operator())
        .await;
    assert!(response.result.is_none());
    assert_eq!(response.error.as_ref().unwrap().code, -32603);
    assert!(!serde_json::to_string(&response)
        .unwrap()
        .contains("/must/not/leak"));
}

mod activation_lost_ack {
    use super::*;
    use domain::run_carry_forward::ContentDigest;
    use engine::run_carry_forward::{CarryForwardService, ServiceConfig};
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicBool, Ordering};

    struct LoseActivationAck {
        real: CarryForwardService,
        lose_once: AtomicBool,
        committed: Mutex<Option<Value>>,
    }

    #[async_trait::async_trait]
    impl ContinuationService for LoseActivationAck {
        fn admission_enabled(&self) -> bool {
            self.real.enabled()
        }

        async fn call(
            &self,
            tool: &str,
            principal: &auth::Principal,
            args: Value,
        ) -> anyhow::Result<Value> {
            let result = ContinuationService::call(&self.real, tool, principal, args).await?;
            if tool == "runs.continuation_activate" && self.lose_once.swap(false, Ordering::SeqCst)
            {
                // Inject only acknowledgement loss, after the actual service commits.
                assert_eq!(result["status"], "accepted", "{result:#}");
                assert_eq!(result["operation"]["phase"], "activated", "{result:#}");
                *self.committed.lock().unwrap() = Some(result);
                return Err(ContinuationServiceError::CommandOutcomeUnknown.into());
            }
            Ok(result)
        }
    }

    fn git(root: &Path, args: &[&str]) -> String {
        let output = std::process::Command::new("/usr/bin/git")
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
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap().trim().to_owned()
    }

    fn copy_tree(from: &Path, to: &Path) {
        fs::create_dir_all(to).unwrap();
        for entry in fs::read_dir(from).unwrap() {
            let entry = entry.unwrap();
            if entry.file_type().unwrap().is_dir() {
                copy_tree(&entry.path(), &to.join(entry.file_name()));
            } else {
                fs::copy(entry.path(), to.join(entry.file_name())).unwrap();
            }
        }
    }

    struct Fixture {
        _temp: tempfile::TempDir,
        pool: sqlx::SqlitePool,
        db_url: String,
        owned: PathBuf,
        preview: Value,
    }

    impl Fixture {
        async fn new() -> Self {
            use std::os::unix::fs::PermissionsExt;
            let temp = tempfile::tempdir().unwrap();
            let base = fs::canonicalize(temp.path()).unwrap();
            let root = base.join("repo");
            let owned = base.join("operations");
            fs::create_dir(&root).unwrap();
            fs::create_dir(&owned).unwrap();
            fs::set_permissions(&owned, fs::Permissions::from_mode(0o700)).unwrap();
            let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
            copy_tree(
                &repository.join("examples/agents"),
                &root.join("examples/agents"),
            );
            fs::create_dir_all(root.join("examples/workflows")).unwrap();
            let workflow = root.join("examples/workflows/current.yaml");
            let catalog = root.join("examples/agents/agents.yaml");
            fs::copy(
                repository.join("examples/workflows/workflow.yaml"),
                &workflow,
            )
            .unwrap();
            let contract_path = "docs/evidence/rollout-contract/p039-rollout-contract.json";
            let contract: Value =
                serde_json::from_slice(&fs::read(repository.join(contract_path)).unwrap()).unwrap();
            let mut evidence = vec![
                contract_path.to_owned(),
                contract["readback_fixture"].as_str().unwrap().to_owned(),
            ];
            evidence.extend(
                contract["negative_fixtures"]
                    .as_object()
                    .unwrap()
                    .values()
                    .map(|p| p.as_str().unwrap().to_owned()),
            );
            for path in evidence {
                let destination = root.join(&path);
                fs::create_dir_all(destination.parent().unwrap()).unwrap();
                fs::copy(repository.join(path), destination).unwrap();
            }
            let metadata_relative = format!(".chainworks/runs/{SOURCE}");
            let metadata = root.join(&metadata_relative);
            fs::create_dir_all(metadata.join("proposals/current")).unwrap();
            let seed = serde_json::to_vec(&json!({"title":"Bounded activation acknowledgement proof","rollout_contract_v1":contract})).unwrap();
            let seed_path = metadata.join("proposals/current/proposal.md");
            fs::write(&seed_path, &seed).unwrap();
            let delivery = json!({"repo_identifier":"fixture-repository","repo_root":root,
                "base_branch":"HEAD","worktree_base_path":owned,"target_branch":"fixture-delivery",
                "release_target_id":"fixture","release_mode":"sandbox"})
            .to_string();
            let db_url = format!("sqlite://{}?mode=rwc", base.join("fixture.db").display());
            let pool = db::pool::create_pool(&db_url).await.unwrap();
            db::writer::register_shared_writer(
                &pool,
                Arc::new(db::writer::DbWriter::new(pool.clone())),
            )
            .await
            .unwrap();
            // Historical blocked source is a fixture. Reservation, preparation,
            // activation, command receipts and successor rows below are all real.
            sqlx::query("INSERT INTO ideas(id,title,body,workspace_root_path,created_at) VALUES (?,'Source','Body',?,'2026-09-20T00:00:00Z')")
                .bind(SOURCE).bind(root.to_str().unwrap()).execute(&pool).await.unwrap();
            sqlx::query("INSERT INTO runs(id,idea_id,status,workflow_id,workflow_title,workspace_root,artifact_root,started_at,current_state,worktree_root,chainworks_meta_root,workflow_yaml_path,agent_catalog_yaml_path,delivery_configuration_json) VALUES (?,?,'blocked','proposal_to_release','Workflow',?,?,'2026-09-20T00:00:00Z','state_7_implementation_started',?,?,?,?,?)")
                .bind(SOURCE).bind(SOURCE).bind(root.to_str().unwrap()).bind(metadata.to_str().unwrap())
                .bind(root.to_str().unwrap()).bind(&metadata_relative).bind(workflow.to_str().unwrap())
                .bind(catalog.to_str().unwrap()).bind(&delivery).execute(&pool).await.unwrap();
            let source = db::repos::runs::find_by_id(&pool, SOURCE.parse().unwrap())
                .await
                .unwrap()
                .unwrap();
            let frozen = engine::command_handler::compile_run_plan_for_run(&source)
                .unwrap()
                .unwrap();
            sqlx::query("UPDATE runs SET workflow_snapshot_json=?,catalog_snapshot_json=?,workflow_snapshot_hash=?,catalog_snapshot_hash=? WHERE id=?")
                .bind(&frozen.workflow_snapshot_json).bind(&frozen.catalog_snapshot_json)
                .bind(&frozen.workflow_snapshot_hash).bind(&frozen.catalog_snapshot_hash).bind(SOURCE)
                .execute(&pool).await.unwrap();
            // JSON is valid YAML; preserve the compiled legacy topology and add
            // only the explicit current-target continuation profile.
            let mut target: Value = serde_json::from_str(&frozen.workflow_snapshot_json).unwrap();
            target["blocked_run_continuation"] = json!({
                "schema_version":"blocked_run_continuation_profile_v1","profile":"implementation_restart_v1",
                "review_state":"state_4_proposal_reviewed","approval_state":"state_6_implementation_approval",
                "preparation_state":"state_7_implementation_started","proposal_input":"proposal_current",
                "preparation_task":"freeze_approved_proposal_and_prepare_worktree"});
            fs::write(&workflow, serde_json::to_vec(&target).unwrap()).unwrap();
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
                    "source and rollout evidence",
                ],
            );
            let head = git(&root, &["rev-parse", "HEAD"]);
            sqlx::query("UPDATE runs SET base_branch='HEAD',base_revision=? WHERE id=?")
                .bind(head)
                .bind(SOURCE)
                .execute(&pool)
                .await
                .unwrap();
            fs::write(root.join("product.txt"), "dirty source\n").unwrap();
            sqlx::query("INSERT INTO stage_executions(id,run_id,stage_id,label,status,iteration,attempt_number,started_at) VALUES (?,?,'state_7_implementation_started','Implementation','blocked',0,1,'2026-09-20T00:00:00Z')")
                .bind(uuid::Uuid::new_v4().to_string()).bind(SOURCE).execute(&pool).await.unwrap();
            sqlx::query("INSERT INTO approvals(id,run_id,stage_id,decision,requested_at,decided_at) VALUES (?,?,'state_6_implementation_approval','granted','2026-09-19T00:00:00Z','2026-09-19T01:00:00Z')")
                .bind(uuid::Uuid::new_v4().to_string()).bind(SOURCE).execute(&pool).await.unwrap();
            let seed_id = uuid::Uuid::new_v4().to_string();
            sqlx::query("INSERT INTO artifacts(id,run_id,stage_id,agent_id,name,contract_id,format,file_path,checksum_sha256,size_bytes,provider,created_at) VALUES (?,?,'historical','fixture','proposal_current','proposal_current','markdown',?,?,?,'fixture','2026-09-20T00:00:00Z')")
                .bind(&seed_id).bind(SOURCE).bind(seed_path.to_str().unwrap())
                .bind(ContentDigest::of(&seed).as_str().strip_prefix("sha256:").unwrap()).bind(seed.len() as i64)
                .execute(&pool).await.unwrap();
            Self {
                _temp: temp,
                pool,
                db_url,
                owned,
                preview: json!({"run_id":SOURCE,
                "target":{"workflow_yaml_path":"examples/workflows/current.yaml","agent_catalog_yaml_path":"examples/agents/agents.yaml","delivery_configuration_json":delivery},
                "selection":{"proposal_input":{"kind":"artifact","id":seed_id},"reference_inputs":[],"include_dirty_work":true,"excluded_workspace_paths":[]},
                "profile":"implementation_restart_v1","limit":2}),
            }
        }
    }

    fn mcp(pool: &sqlx::SqlitePool, service: Arc<dyn ContinuationService>) -> Arc<McpServer> {
        let (events, _) = tokio::sync::broadcast::channel(16);
        let handler = Arc::new(engine::command_handler::CommandHandler::new(
            pool.clone(),
            events,
            engine::work_queue::WorkQueue::new(pool.clone()),
        ));
        let mcp = Arc::new(
            McpServer::new(pool.clone(), handler, auth::PrincipalTable::test_fixture())
                .with_continuation_service(service),
        );
        http_grants(&mcp);
        mcp
    }

    async fn outcome(mcp: &Arc<McpServer>, tool: &str, args: Value) -> Value {
        let (status, response) = http(
            mcp.clone(),
            serde_json::to_string(&call(tool, args)).unwrap(),
        )
        .await;
        assert_eq!(status, axum::http::StatusCode::OK);
        assert!(response.get("error").is_none(), "{response:#}");
        serde_json::from_str(response["result"]["content"][0]["text"].as_str().unwrap()).unwrap()
    }

    async fn durable_counts(pool: &sqlx::SqlitePool) -> (i64, i64, i64, i64, i64) {
        sqlx::query_as("SELECT (SELECT count(*) FROM runs WHERE id<>?1), (SELECT count(*) FROM work_items WHERE kind='advance_run'), (SELECT count(*) FROM run_continuation_commands WHERE command='runs.continuation_activate'), (SELECT count(*) FROM command_journal), (SELECT count(*) FROM agent_executions)")
            .bind(SOURCE).fetch_one(pool).await.unwrap()
    }

    #[tokio::test]
    async fn p039_real_service_http_activation_lost_ack_reopens_and_replays_once() {
        let fixture = Fixture::new().await;
        let service = Arc::new(LoseActivationAck {
            real: CarryForwardService::new(
                fixture.pool.clone(),
                ServiceConfig {
                    enabled: true,
                    owned_parent: fixture.owned.clone(),
                },
            ),
            lose_once: AtomicBool::new(true),
            committed: Mutex::new(None),
        });
        let server = mcp(&fixture.pool, service.clone());
        let preview = outcome(
            &server,
            "runs.continuation_preview",
            fixture.preview.clone(),
        )
        .await;
        assert_eq!(
            preview["schema_version"], "run_carry_forward_preview_page_v1",
            "{preview:#}"
        );
        assert_eq!(preview["plan_summary"]["holds"], json!([]));
        let mut prepare = fixture.preview.clone();
        prepare["target"] = preview["plan_summary"]["target"].clone();
        prepare["expected_plan_sha256"] = preview["plan_sha256"].clone();
        prepare["caller_request_id"] = json!(uuid::Uuid::new_v4());
        prepare["reason"] = json!("Bounded lost acknowledgement proof");
        let accepted = outcome(&server, "runs.continue_blocked", prepare).await;
        assert_eq!(accepted["status"], "accepted", "{accepted:#}");
        let operation = accepted["operation"]["operation_id"].as_str().unwrap();
        let prepared = tokio::time::timeout(std::time::Duration::from_secs(60), async {
            loop {
                let current = db::repos::run_continuations::find(&fixture.pool, operation)
                    .await
                    .unwrap()
                    .unwrap();
                if current.phase != "preparing" {
                    break current;
                }
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("bounded real preparation wait");
        assert_eq!(prepared.phase, "prepared", "{prepared:?}");
        assert_eq!(durable_counts(&fixture.pool).await, (0, 0, 0, 1, 0));
        let activation = json!({"operation_id":operation,"expected_version":prepared.version,
            "expected_manifest_sha256":format!("sha256:{}", prepared.manifest_sha256.as_ref().unwrap()),
            "caller_request_id":uuid::Uuid::new_v4()});
        let body =
            serde_json::to_string(&call("runs.continuation_activate", activation.clone())).unwrap();
        let (status, uncertain) = http(server.clone(), body.clone()).await;
        assert_eq!(status, axum::http::StatusCode::OK);
        assert_eq!(uncertain["error"]["code"], -32603);
        assert!(uncertain.get("result").is_none());
        assert_eq!(
            uncertain["error"]["data"],
            json!({"schema_version":"p039_transport_error_v1",
            "code":"command_outcome_unknown","next_action":"replay_same_request"})
        );
        let committed = service.committed.lock().unwrap().clone().unwrap();
        let stored: String = sqlx::query_scalar(
            "SELECT result_json FROM run_continuation_commands WHERE caller_request_id=?",
        )
        .bind(activation["caller_request_id"].as_str().unwrap())
        .fetch_one(&fixture.pool)
        .await
        .unwrap();
        assert_eq!(serde_json::from_str::<Value>(&stored).unwrap(), committed);
        assert_eq!(
            committed["operation"]["successor_run_id"],
            prepared.reserved_successor_run_id
        );
        assert_eq!(durable_counts(&fixture.pool).await, (1, 1, 1, 2, 0));
        let manifest_path = PathBuf::from(prepared.manifest_ref.unwrap());
        let manifest_bytes = fs::read(&manifest_path).unwrap();
        drop(server);
        drop(service);
        tokio::time::timeout(std::time::Duration::from_secs(20), fixture.pool.close())
            .await
            .unwrap();

        let pool = db::pool::create_pool(&fixture.db_url).await.unwrap();
        db::writer::register_shared_writer(
            &pool,
            Arc::new(db::writer::DbWriter::new(pool.clone())),
        )
        .await
        .unwrap();
        db::repos::run_continuations::recover_preparations(&pool)
            .await
            .unwrap();
        let server = mcp(
            &pool,
            Arc::new(CarryForwardService::new(
                pool.clone(),
                ServiceConfig {
                    enabled: true,
                    owned_parent: fixture.owned.clone(),
                },
            )),
        );
        let mut expected = committed.clone();
        expected["status"] = json!("replayed");
        expected["replay_of"] = json!("accepted");
        for _ in 0..2 {
            let (status, replay) = http(server.clone(), body.clone()).await;
            assert_eq!(status, axum::http::StatusCode::OK);
            assert!(replay.get("error").is_none(), "{replay:#}");
            let replay: Value =
                serde_json::from_str(replay["result"]["content"][0]["text"].as_str().unwrap())
                    .unwrap();
            assert_eq!(replay, expected);
            assert_eq!(durable_counts(&pool).await, (1, 1, 1, 2, 0));
        }
        let reopened: String = sqlx::query_scalar(
            "SELECT result_json FROM run_continuation_commands WHERE caller_request_id=?",
        )
        .bind(activation["caller_request_id"].as_str().unwrap())
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(
            reopened, stored,
            "original receipt bytes must not be rewritten for replay"
        );
        let advanced: (String, String) =
            sqlx::query_as("SELECT run_id,status FROM work_items WHERE kind='advance_run'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(
            advanced,
            (prepared.reserved_successor_run_id.clone(), "pending".into())
        );
        let operation = db::repos::run_continuations::find(&pool, operation)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(operation.phase, "activated");
        assert_eq!(
            operation.successor_run_id.as_deref(),
            Some(prepared.reserved_successor_run_id.as_str())
        );
        assert_eq!(fs::read(manifest_path).unwrap(), manifest_bytes);
        drop(server);
        pool.close().await;
    }
}

#[tokio::test]
async fn p039_http_one_mib_boundary_includes_whitespace_and_checks_before_service() {
    let (mcp, _) = server().await;
    let mcp = Arc::new(mcp);
    http_grants(&mcp);
    let mut body = serde_json::to_string(&call(TOOLS[2].0, request(TOOLS[2].0))).unwrap();
    body.push_str(&" ".repeat(1024 * 1024 - body.len()));
    let (status, result) = http(mcp.clone(), body.clone()).await;
    assert_eq!(status, axum::http::StatusCode::OK);
    assert_eq!(
        result["error"]["data"]["reason_code"],
        "P039_ADMISSION_UNAVAILABLE"
    );
    body.push(' ');
    assert_eq!(
        http(mcp, body).await.0,
        axum::http::StatusCode::PAYLOAD_TOO_LARGE
    );
}

#[test]
fn p039_schemas_match_exact_dto_field_names_nullability_and_bounds() {
    for (name, id) in TOOLS {
        let schema = tools::mcp_tool_for(id).input_schema;
        let mut expected = request(name)
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect::<Vec<_>>();
        if matches!(
            name,
            "runs.continuation_preview" | "runs.continue_blocked" | "runs.continuation_get"
        ) {
            expected.extend(["cursor".into(), "limit".into()]);
            assert_eq!(schema["properties"]["limit"]["maximum"], 500);
            assert_eq!(schema["properties"]["limit"]["default"], 100);
            assert_eq!(schema["properties"]["cursor"]["anyOf"][1]["type"], "null");
        }
        expected.sort();
        assert_eq!(
            schema["properties"]
                .as_object()
                .unwrap()
                .keys()
                .cloned()
                .collect::<Vec<_>>(),
            expected
        );
        for key in schema["required"].as_array().unwrap() {
            assert!(
                request(name).get(key.as_str().unwrap()).is_some(),
                "{name}: {key}"
            );
        }
        if name == "runs.continuation_preview" || name == "runs.continue_blocked" {
            let target = &schema["properties"]["target"];
            assert_eq!(target["additionalProperties"], false);
            assert_eq!(
                target["properties"]["delivery_configuration_json"]["type"],
                "string"
            );
            assert_eq!(
                target["required"].as_array().unwrap().len(),
                if name == "runs.continuation_preview" {
                    3
                } else {
                    5
                }
            );
            let selection = &schema["properties"]["selection"];
            assert_eq!(selection["additionalProperties"], false);
            assert_eq!(selection["properties"].as_object().unwrap().len(), 4);
            assert_eq!(selection["properties"]["reference_inputs"]["maxItems"], 128);
            assert_eq!(
                selection["properties"]["reference_inputs"]["uniqueItems"],
                true
            );
            assert_eq!(selection["properties"]["include_dirty_work"]["const"], true);
            assert_eq!(
                selection["properties"]["excluded_workspace_paths"]["maxItems"],
                128
            );
            assert_eq!(
                selection["properties"]["excluded_workspace_paths"]["uniqueItems"],
                true
            );
        }
    }
    let command = tools::mcp_tool_for(TOOLS[1].1).output_schema.unwrap();
    assert_eq!(command["required"].as_array().unwrap().len(), 8);
    assert_eq!(
        command["properties"]["operation"]["anyOf"][0]["additionalProperties"],
        false
    );
    assert_eq!(
        command["properties"]["denial"]["anyOf"][0]["properties"]["code"]["enum"]
            .as_array()
            .unwrap()
            .len(),
        22
    );
    for (_, id) in [TOOLS[3], TOOLS[4], TOOLS[5]] {
        assert_eq!(tools::mcp_tool_for(id).output_schema.unwrap(), command);
    }
    let get = tools::mcp_tool_for(TOOLS[2].1).output_schema.unwrap();
    assert_eq!(get["additionalProperties"], false);
    assert_eq!(get["properties"]["manifest_page"]["maxItems"], 500);
    assert_eq!(
        get["properties"]["admission"]["additionalProperties"],
        false
    );
    let preview = tools::mcp_tool_for(TOOLS[0].1).output_schema.unwrap();
    assert_eq!(
        preview["oneOf"][0]["properties"]["entries"]["maxItems"],
        500
    );
    assert_eq!(
        preview["oneOf"][0]["properties"]["plan_summary"]["additionalProperties"],
        false
    );
    assert_eq!(
        preview["oneOf"][1]["properties"]["code"]["const"],
        "preview_stale"
    );
}

#[test]
fn p039_boundary_uses_existing_runs_wildcard_without_default_capability_grant() {
    let policy = auth::boundary::BoundaryPolicy::from_embedded_with_mode(
        auth::boundary::PolicyMode::Enforce,
    )
    .unwrap();
    for (name, _) in TOOLS {
        match policy.evaluate("agent_operator", "mcp_tools_call", Some(name)) {
            auth::boundary::PolicyDecision::Allow { row_id } => assert_eq!(
                row_id.as_deref(),
                Some("p081.agent_operator.mcp_tools_call.command")
            ),
            other => panic!("{other:?}"),
        }
        assert!(!auth::is_tool_allowed(
            &auth::Principal::new("default", auth::PrincipalClass::Operator),
            name
        ));
        assert_eq!(
            matches!(
                policy.evaluate("observer", "mcp_tools_call", Some(name)),
                auth::boundary::PolicyDecision::Allow { .. }
            ),
            !tools::run_continuations::is_mutation(name)
        );
    }
}

struct InvalidReadService {
    wrong_source: bool,
}
#[async_trait::async_trait]
impl ContinuationService for InvalidReadService {
    async fn call(&self, _: &str, _: &auth::Principal, _: Value) -> anyhow::Result<Value> {
        let excluded = json!({"role":"excluded","entry_id":"e","source_namespace":"workspace",
            "source_artifact_id":null,"logical_name":"generated","source_relative_path":"target/",
            "reason":"generated data"});
        // Structurally valid DTO, deliberately inconsistent with canonical identity
        // or the requested page size. This is never admission/dispatch evidence.
        Ok(
            json!({"schema_version":"run_continuation_readback_v1","operation_id":OPERATION,
            "source_run_id":if self.wrong_source {REQUEST} else {SOURCE},"successor_run_id":null,
            "phase":"preparing","version":1,"plan_sha256":format!("sha256:{}","b".repeat(64)),
            "manifest_sha256":null,"execution_disposition":"source_reserved","verification_status":"pending",
            "holds":[],"next_actions":[],"journal_id":OPERATION,"updated_at":"2026-09-20T00:00:00Z",
            "projection_freshness":"fresh","admission":{"schema_version":"run_continuation_admission_v1",
                "enabled":false,"reason":"fixture","fences_enforced":true,"reconcile_available":false},
            "manifest_page":if self.wrong_source {json!([])} else {json!([excluded.clone(),excluded])},"next_cursor":null}),
        )
    }
}

#[tokio::test]
async fn p039_service_readback_must_match_canonical_source_and_requested_page_bound() {
    for wrong_source in [true, false] {
        let (mcp, pool) = server().await;
        seed(&pool, false).await;
        let mcp = mcp.with_continuation_service(Arc::new(InvalidReadService { wrong_source }));
        let response = mcp
            .handle_request(
                call(TOOLS[2].0, json!({"operation_id":OPERATION,"limit":1})),
                &operator(),
            )
            .await;
        assert!(
            response.result.is_none(),
            "invalid service readback escaped: {response:?}"
        );
        assert_eq!(response.error.unwrap().code, -32603);
    }
}

struct PreviewHoldService {
    source: &'static str,
}
#[async_trait::async_trait]
impl ContinuationService for PreviewHoldService {
    async fn call(&self, _: &str, _: &auth::Principal, _: Value) -> anyhow::Result<Value> {
        Ok(
            json!({"schema_version":"run_carry_forward_preview_hold_v1","source_run_id":self.source,
            "holds":[{"code":"unsupported_frontier","message":"Source frontier is not supported"}],
            "next_action":"inspect"}),
        )
    }
}

#[tokio::test]
async fn p039_expected_preview_hold_is_typed_success_without_dummy_witness_or_hash() {
    let (mcp, pool) = server().await;
    seed(&pool, false).await;
    let mcp = mcp.with_continuation_service(Arc::new(PreviewHoldService { source: SOURCE }));
    let response = mcp
        .handle_request(call(TOOLS[0].0, request(TOOLS[0].0)), &operator())
        .await;
    assert!(response.error.is_none(), "{response:?}");
    let value: Value = serde_json::from_str(
        response.result.unwrap()["content"][0]["text"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    let _: domain::run_carry_forward_api::RunCarryForwardPreviewHoldV1 =
        serde_json::from_value(value.clone()).unwrap();
    assert_eq!(value.as_object().unwrap().len(), 4);
    assert!(value.get("plan_sha256").is_none());
    assert!(value.get("source_witness").is_none());
    let schema = tools::mcp_tool_for(TOOLS[0].1).output_schema.unwrap();
    let hold = schema["oneOf"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["properties"]["schema_version"]["const"] == "run_carry_forward_preview_hold_v1")
        .unwrap();
    assert_eq!(hold["additionalProperties"], false);
    assert_eq!(hold["properties"]["holds"]["minItems"], 1);
    assert_eq!(hold["properties"]["holds"]["maxItems"], 128);
    assert_eq!(
        hold["properties"]["next_action"]["enum"],
        json!(["inspect", "resolve_hold", "refresh_preview"])
    );
}

#[tokio::test]
async fn p039_preview_hold_cannot_substitute_another_source() {
    let (mcp, pool) = server().await;
    seed(&pool, false).await;
    let mcp = mcp.with_continuation_service(Arc::new(PreviewHoldService { source: REQUEST }));
    let response = mcp
        .handle_request(call(TOOLS[0].0, request(TOOLS[0].0)), &operator())
        .await;
    assert!(response.result.is_none());
    assert_eq!(response.error.unwrap().code, -32603);
}
