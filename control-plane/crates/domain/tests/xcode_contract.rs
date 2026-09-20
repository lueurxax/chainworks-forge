use domain::xcode_contract::{
    canonical_bytes, canonical_digest, decode_open, decode_read, manifest_digest,
    open_schema_identity, parse_unique_json, read_tool_definition, trust_policy_digest,
    validate_initialize, validate_open_arguments, validate_trust_policy, validate_upstream_tools,
    ContractError, ReadArguments, TRUST_POLICY_ID,
};
use serde_json::{json, Value};

fn captured_tools() -> Vec<Value> {
    let capture: Value = serde_json::from_str(include_str!(
        "../../acp/tests/fixtures/xcode_headless_i1/apple-contract.json"
    ))
    .unwrap();
    capture["tools"].as_array().unwrap().clone()
}

fn open() -> Value {
    json!({"structuredContent": {
        "workspaceIdentifier": "workspace1", "workspacePath": "/tmp/Trusted.xcodeproj"
    }, "isError": false})
}

fn read() -> Value {
    json!({"structuredContent": {
        "content": "     1\tHello\n     2\t", "filePath": "Project/Source.swift",
        "fileSize": 6, "totalLines": 2, "linesRead": 2, "startLine": 1
    }, "isError": false})
}

fn args() -> ReadArguments {
    ReadArguments::parse(&json!({"filePath": "Project/Source.swift"}), "cw-alias").unwrap()
}

#[test]
fn unique_json_rejects_duplicate_keys_at_every_depth_and_invalid_encoding() {
    for bytes in [
        br#"{"a":1,"a":2}"#.as_slice(),
        br#"{"x":[{"a":1,"\u0061":2}]}"#,
        br#"{"s":"\ud800"}"#,
        br#"{"s":"\udc00"}"#,
        b"{\"s\":\"\xff\"}",
        b"NaN",
        b"1e999",
        b"{} {}",
    ] {
        assert!(parse_unique_json(bytes).is_err(), "accepted {bytes:?}");
    }
    assert_eq!(
        parse_unique_json(br#"{"x":[{"a":1},{"a":2}]}"#).unwrap(),
        json!({"x":[{"a":1},{"a":2}]})
    );
    assert!(parse_unique_json(&vec![b' '; 1024 * 1024 + 1]).is_err());
    let mut boundary = vec![b' '; 1024 * 1024];
    boundary[..2].copy_from_slice(b"{}");
    assert_eq!(parse_unique_json(&boundary).unwrap(), json!({}));
}

#[test]
fn jcs_digest_matches_published_vectors_and_preserves_authority_distinctions() {
    assert_eq!(
        canonical_bytes(&json!({"b":2,"a":1})).unwrap(),
        br#"{"a":1,"b":2}"#
    );
    assert_eq!(
        canonical_digest("cw.xcode.canonical.v1", &json!({"b":2,"a":1})).unwrap(),
        "8547eab8cbd047549dd060bebdd2b52f0ac3e942b0a3fe354daade156ec6d3f3"
    );
    assert_eq!(
        canonical_digest("cw.xcode.canonical.v1", &json!({"a":1,"b":3})).unwrap(),
        "880568bdd65867e6b927358e43dd3cbcad5eda9f5d58f1ce9adb13a685fa6854"
    );
    for (a, b) in [
        (json!({}), json!({"a":null})),
        (json!([1, 2]), json!([2, 1])),
        (json!("\u{e9}"), json!("e\u{301}")),
    ] {
        assert_ne!(
            canonical_digest("d", &a).unwrap(),
            canonical_digest("d", &b).unwrap()
        );
    }
    assert_ne!(
        canonical_digest("a", &json!({})).unwrap(),
        canonical_digest("b", &json!({})).unwrap()
    );
    assert!(canonical_digest("a\0b", &json!({})).is_err());
}

#[test]
fn jcs_uses_utf16_property_order_ecmascript_numbers_and_control_escaping() {
    let value = parse_unique_json(br#"{"\ue000":1,"\ud83d\ude00":2,"n":-0.0,"s":"\b\n\u0001","f":333333333.33333329,"e":1e-7}"#).unwrap();
    assert_eq!(String::from_utf8(canonical_bytes(&value).unwrap()).unwrap(),
        "{\"e\":1e-7,\"f\":333333333.3333333,\"n\":0,\"s\":\"\\b\\n\\u0001\",\"\u{1f600}\":2,\"\u{e000}\":1}");
    for value in [
        json!(9007199254740992_u64),
        json!(-9007199254740992_i64),
        json!({"deep":[9007199254740992.0]}),
        json!(1e30),
    ] {
        assert!(canonical_bytes(&value).is_err());
    }
    assert_eq!(
        canonical_bytes(&json!(9007199254740991_u64)).unwrap(),
        b"9007199254740991"
    );
    assert!(parse_unique_json(b"9007199254740993").is_err());
}

#[test]
fn initialization_requires_installed_protocol_server_and_tools_capability() {
    let good = json!({"protocolVersion":"2025-06-18", "serverInfo":{"name":"xcode-tools","version":"25317"},"capabilities":{"tools":{}}});
    validate_initialize(&good).unwrap();
    for (pointer, bad) in [
        ("/protocolVersion", json!("2024-11-05")),
        ("/serverInfo/name", json!("other")),
        ("/serverInfo/version", json!("25318")),
        ("/capabilities/tools", Value::Null),
    ] {
        let mut value = good.clone();
        *value.pointer_mut(pointer).unwrap() = bad;
        assert!(validate_initialize(&value).is_err());
    }
    assert!(validate_initialize(&json!({})).is_err());
}

#[test]
fn admission_pins_both_schemas_rejects_duplicates_and_ignores_unadmitted_tools() {
    let tools = captured_tools();
    validate_upstream_tools(&tools).unwrap();
    for pointer in [
        "/inputSchema/properties/path/type",
        "/outputSchema/properties/workspacePath/type",
    ] {
        let mut changed = tools.clone();
        *changed[0].pointer_mut(pointer).unwrap() = json!("integer");
        assert!(validate_upstream_tools(&changed).is_err());
    }
    for pointer in [
        "/inputSchema/properties/filePath/type",
        "/outputSchema/properties/content/type",
    ] {
        let mut changed = tools.clone();
        *changed[1].pointer_mut(pointer).unwrap() = json!("integer");
        assert!(validate_upstream_tools(&changed).is_err());
    }
    let mut duplicate = tools.clone();
    duplicate.push(tools[0].clone());
    assert!(validate_upstream_tools(&duplicate).is_err());
    assert!(validate_upstream_tools(&tools[..1]).is_err());
    let mut extra = tools.clone();
    extra.push(json!({"name":"BuildProject","inputSchema":{},"outputSchema":{}}));
    validate_upstream_tools(&extra).unwrap();
    extra.push(extra.last().unwrap().clone());
    assert!(validate_upstream_tools(&extra).is_err());
    let mut unknown = tools;
    unknown[0]["newField"] = json!(true);
    assert!(validate_upstream_tools(&unknown).is_err());
}

#[test]
fn open_requires_explicit_mapping_and_discards_validated_optional_metadata() {
    let mut value = open();
    value["structuredContent"]["message"] = json!("private Apple detail");
    value["structuredContent"]["activeScheme"] = json!("App");
    value["structuredContent"]["activeRunDestination"] = json!("Mac");
    let decoded = decode_open(&value).unwrap();
    assert_eq!(decoded.workspace_identifier, "workspace1");
    assert_eq!(decoded.workspace_path, "/tmp/Trusted.xcodeproj");
    for field in ["workspaceIdentifier", "workspacePath"] {
        let mut bad = open();
        bad["structuredContent"]
            .as_object_mut()
            .unwrap()
            .remove(field);
        assert!(decode_open(&bad).is_err());
    }
    for (field, bad) in [
        ("workspaceIdentifier", json!("x".repeat(257))),
        ("workspacePath", json!("/".to_owned() + &"x".repeat(2048))),
        ("workspacePath", json!("relative.xcodeproj")),
        ("workspaceIdentifier", json!("a\tb")),
        ("activeScheme", Value::Null),
        ("message", json!("x".repeat(4097))),
        ("extra", json!(1)),
    ] {
        let mut value = open();
        value["structuredContent"][field] = bad;
        assert!(decode_open(&value).is_err(), "accepted {field}");
    }
}

#[test]
fn envelopes_reject_malformed_errors_unsupported_content_and_disagreeing_text() {
    let good = open();
    let mut matching = good.clone();
    matching["content"] = json!([{"type":"text","text":good["structuredContent"].to_string()}]);
    decode_open(&matching).unwrap();
    for value in [
        json!({"structuredContent":good["structuredContent"], "isError":true}),
        json!({"structuredContent":good["structuredContent"], "isError":"false"}),
        json!({"structuredContent":good["structuredContent"], "unknown":true}),
        json!({"structuredContent":good["structuredContent"], "content":[]}),
        json!({"structuredContent":good["structuredContent"], "content":[{"type":"text","text":"{}"}]}),
        json!({"structuredContent":good["structuredContent"], "content":[{"type":"image","data":"x"}]}),
        json!({"content":[{"type":"text","text":good["structuredContent"].to_string()}]}),
    ] {
        assert!(decode_open(&value).is_err());
    }
    let error = decode_open(
        &json!({"isError":true,"content":[{"type":"text","text":"private token secret"}]}),
    )
    .unwrap_err();
    assert_eq!(error, ContractError::UpstreamError);
    assert!(!error.to_string().contains("private"));
}

#[test]
fn read_arguments_inject_alias_and_never_forward_it_to_apple() {
    let parsed = args();
    assert_eq!(parsed.file_path(), "Project/Source.swift");
    assert_eq!(parsed.workspace_identifier(), "cw-alias");
    assert_eq!(parsed.offset(), 1);
    assert_eq!(parsed.limit(), 600);
    assert_eq!(
        parsed.upstream_arguments("workspace7"),
        json!({"filePath":"Project/Source.swift", "workspaceIdentifier":"workspace7", "offset":1,"limit":600})
    );
    ReadArguments::parse(&json!({"filePath":"LinkedGroup/External.swift", "workspaceIdentifier":"cw-alias", "offset":4,"limit":1}), "cw-alias").unwrap();
    for path in [
        "",
        "/tmp/a",
        "../a",
        "Project/../a",
        "Project/./a",
        "Project//a",
        "Project/",
        "C:\\a",
        "a\tb",
        "a\0b",
    ] {
        assert!(
            ReadArguments::parse(&json!({"filePath":path}), "cw-alias").is_err(),
            "accepted {path:?}"
        );
    }
    for (field, bad) in [
        ("workspaceIdentifier", json!("workspace7")),
        ("workspaceIdentifier", Value::Null),
        ("offset", json!(0)),
        ("offset", json!(9007199254740992_u64)),
        ("offset", Value::Null),
        ("limit", json!(0)),
        ("limit", json!(601)),
        ("limit", json!(1.5)),
        ("tabIdentifier", json!("tab1")),
        ("path", json!("anything")),
    ] {
        let mut value = json!({"filePath":"Project/Source.swift"});
        value[field] = bad;
        assert!(
            ReadArguments::parse(&value, "cw-alias").is_err(),
            "accepted {field}"
        );
    }
}

#[test]
fn read_normalizes_closed_result_and_validates_cross_field_limits() {
    let mut value = read();
    value["structuredContent"]["message"] = json!("private Apple detail");
    assert_eq!(
        decode_read(&value, &args()).unwrap(),
        json!({"schema_version":1,
        "content":"     1\tHello\n     2\t", "filePath":"Project/Source.swift",
        "fileSize":6,"totalLines":2,"linesRead":2,"startLine":1})
    );
    for (field, bad) in [
        ("content", json!("x".repeat(256 * 1024 + 1))),
        ("content", json!("\u{e9}".repeat(128 * 1024 + 1))),
        ("filePath", json!("Project/Other.swift")),
        ("fileSize", json!(-1)),
        ("fileSize", json!(9007199254740992_u64)),
        ("totalLines", json!(1)),
        ("linesRead", json!(601)),
        ("startLine", json!(2)),
        ("message", Value::Null),
        ("other", json!(1)),
    ] {
        let mut value = read();
        value["structuredContent"][field] = bad;
        assert!(decode_read(&value, &args()).is_err(), "accepted {field}");
    }
    let one_line = ReadArguments::parse(
        &json!({"filePath":"Project/Source.swift","limit":1}),
        "cw-alias",
    )
    .unwrap();
    assert!(decode_read(&read(), &one_line).is_err());
    let offset = ReadArguments::parse(
        &json!({"filePath":"Project/Source.swift","offset":2}),
        "cw-alias",
    )
    .unwrap();
    let mut beyond_end = read();
    beyond_end["structuredContent"]["startLine"] = json!(2);
    assert!(decode_read(&beyond_end, &offset).is_err());
    let empty = json!({"structuredContent":{"content":"","filePath":"Project/Source.swift", "fileSize":0,"totalLines":0,"linesRead":0,"startLine":1}});
    assert_eq!(decode_read(&empty, &args()).unwrap()["content"], "");
}

#[test]
fn manifest_and_tool_definition_are_static_closed_and_do_not_admit_builds() {
    let definition = read_tool_definition();
    assert_eq!(definition["name"], "XcodeRead");
    assert_eq!(definition["inputSchema"]["additionalProperties"], false);
    assert_eq!(definition["outputSchema"]["additionalProperties"], false);
    assert_eq!(
        definition["inputSchema"]["properties"]["limit"]["maximum"],
        600
    );
    assert!(!definition.to_string().contains("workspace1"));
    assert_eq!(TRUST_POLICY_ID, "trusted_local_project_v1");
    let digest = manifest_digest().unwrap();
    assert_eq!(digest.len(), 64);
    assert!(digest
        .bytes()
        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)));
    let identity = open_schema_identity().unwrap();
    identity.validate().unwrap();
    assert_eq!(identity.contract_id, "workspace_open");
    assert_eq!(identity.version, 1);
    assert_eq!(identity.manifest_digest, digest);
}

#[test]
fn manifest_digest_binds_schemas_metadata_and_unchanged_capture() {
    let manifest = parse_unique_json(include_bytes!(
        "../../../contracts/xcode-headless/v1/manifest.json"
    ))
    .unwrap();
    let schemas = parse_unique_json(include_bytes!(
        "../../../contracts/xcode-headless/v1/schemas.json"
    ))
    .unwrap();
    let capture = include_bytes!("../../../contracts/xcode-headless/v1/upstream-capture.json");
    assert_eq!(
        capture.as_slice(),
        include_bytes!("../../acp/tests/fixtures/xcode_headless_i1/apple-contract.json").as_slice()
    );
    assert_eq!(manifest["entries"].as_array().unwrap().len(), 2);
    assert_eq!(manifest["entries"][0]["name"], "workspace_open");
    assert_eq!(manifest["entries"][0]["visibility"], "internal");
    assert_eq!(manifest["entries"][1]["name"], "XcodeRead");
    assert_eq!(manifest["entries"][1]["visibility"], "provider");
    for entry in manifest["entries"].as_array().unwrap() {
        assert_eq!(entry["trust_policy_id"], TRUST_POLICY_ID);
        for field in [
            "request_schema",
            "result_schema",
            "error_schema",
            "upstream_result_schema",
        ] {
            let reference = entry[field]
                .as_str()
                .unwrap()
                .strip_prefix("schemas.json#")
                .unwrap();
            let schema = schemas.pointer(reference).unwrap();
            assert_eq!(schema["type"], "object");
            assert_eq!(schema["additionalProperties"], false);
        }
    }
    let bundle = json!({"manifest":manifest, "schemas":schemas, "upstream_capture":parse_unique_json(capture).unwrap(),
        "trust_policy":parse_unique_json(include_bytes!("../../../contracts/xcode-headless/v1/policy.json")).unwrap()});
    let expected = canonical_digest("cw.xcode.manifest.v1", &bundle).unwrap();
    assert_eq!(manifest_digest().unwrap(), expected);
    for pointer in [
        "/schemas/$defs/readResult/properties/content/maxLength",
        "/manifest/trust_policy_id",
        "/upstream_capture/tools/0/outputSchema",
    ] {
        let mut changed = bundle.clone();
        *changed.pointer_mut(pointer).unwrap() = json!("changed");
        assert_ne!(
            canonical_digest("cw.xcode.manifest.v1", &changed).unwrap(),
            expected
        );
    }
    assert!(!read_tool_definition().to_string().contains("\"$ref\""));
}

#[test]
fn every_static_error_matches_its_closed_error_schema() {
    let schemas = parse_unique_json(include_bytes!(
        "../../../contracts/xcode-headless/v1/schemas.json"
    ))
    .unwrap();
    let variants = schemas["$defs"]["errorResult"]["properties"]["error"]["oneOf"]
        .as_array()
        .unwrap();
    let errors = [
        ContractError::InputTooLarge,
        ContractError::InvalidJson,
        ContractError::DuplicateKey,
        ContractError::UnsafeNumber,
        ContractError::InvalidDomain,
        ContractError::SchemaDrift,
        ContractError::InvalidArguments,
        ContractError::InvalidEnvelope,
        ContractError::UpstreamError,
        ContractError::InvalidResult,
        ContractError::InvalidBundle,
    ];
    assert_eq!(variants.len(), errors.len());
    for error in errors {
        let expected = variants
            .iter()
            .find(|v| v["properties"]["code"]["const"] == error.code())
            .unwrap();
        let value = error.to_value();
        assert_eq!(value["schema_version"], 1);
        assert_eq!(value.as_object().unwrap().len(), 2);
        assert_eq!(value["error"].as_object().unwrap().len(), 3);
        for field in ["code", "message", "retry"] {
            assert_eq!(
                value["error"][field],
                expected["properties"][field]["const"]
            );
        }
    }
}

#[test]
fn open_arguments_and_response_accept_exact_byte_limits_but_reject_nulls() {
    validate_open_arguments(&json!({"path":"/tmp/Trusted.xcodeproj"})).unwrap();
    for value in [
        json!({}),
        json!({"path":null}),
        json!({"path":"relative"}),
        json!({"path":"/tmp/a", "extra":true}),
        json!({"path":"/a\n"}),
    ] {
        assert!(validate_open_arguments(&value).is_err());
    }
    let path = "/".to_owned() + &"a".repeat(2047);
    validate_open_arguments(&json!({"path":path})).unwrap();
    let mut value = open();
    value["structuredContent"]["workspaceIdentifier"] = json!("\u{e9}".repeat(128));
    value["structuredContent"]["workspacePath"] = json!(path);
    decode_open(&value).unwrap();
    value["structuredContent"]["workspaceIdentifier"] = json!("\u{e9}".repeat(129));
    assert!(decode_open(&value).is_err());
    for key in [
        "workspaceIdentifier",
        "workspacePath",
        "activeScheme",
        "activeRunDestination",
        "message",
    ] {
        let mut value = open();
        value["structuredContent"][key] = Value::Null;
        assert!(decode_open(&value).is_err());
    }
}

#[test]
fn read_accepts_safe_integral_float_numbers_empty_past_eof_and_exact_content_bound() {
    let argument = ReadArguments::parse(
        &json!({"filePath":"Project/Source.swift","offset":1.0,"limit":600.0}),
        "cw-alias",
    )
    .unwrap();
    let mut value = read();
    value["structuredContent"]["content"] = json!("\u{e9}".repeat(128 * 1024));
    value["structuredContent"]["fileSize"] = json!(9007199254740991_u64);
    assert_eq!(
        decode_read(&value, &argument).unwrap()["fileSize"],
        9007199254740991_u64
    );
    for key in [
        "content",
        "filePath",
        "fileSize",
        "totalLines",
        "linesRead",
        "startLine",
    ] {
        let mut value = read();
        value["structuredContent"]
            .as_object_mut()
            .unwrap()
            .remove(key);
        assert!(decode_read(&value, &args()).is_err());
        value["structuredContent"][key] = Value::Null;
        assert!(decode_read(&value, &args()).is_err());
    }
    let past_eof = ReadArguments::parse(
        &json!({"filePath":"Project/Source.swift", "offset":9007199254740991_u64}),
        "cw-alias",
    )
    .unwrap();
    let value = json!({"structuredContent":{"filePath":"Project/Source.swift", "content":"",
        "fileSize":0,"totalLines":0,"linesRead":0,"startLine":9007199254740991_u64}});
    decode_read(&value, &past_eof).unwrap();
}

#[test]
fn read_text_is_checked_before_dropping_private_messages() {
    let mut value = read();
    value["content"] = json!([{"type":"text","text":value["structuredContent"].to_string()}]);
    decode_read(&value, &args()).unwrap();
    value["structuredContent"]["message"] = json!("different private metadata");
    assert!(decode_read(&value, &args()).is_err());
    value["content"][0]["text"] = json!(r#"{"content":"","content":"other"}"#);
    assert!(decode_read(&value, &args()).is_err());
    value["isError"] = json!(true);
    assert_eq!(
        decode_read(&value, &args()).unwrap_err(),
        ContractError::UpstreamError
    );
}

#[test]
fn installed_full_capture_is_accepted_without_admitting_extra_tools() {
    let capture: Value = serde_json::from_str(include_str!(
        "../../../../docs/evidence/xcode-headless-contract-2026-09-19/mcp-2025-06-18.json"
    ))
    .unwrap();
    validate_initialize(&capture["initialize_response"]["result"]).unwrap();
    let tools: Vec<Value> = capture["tools_list_responses"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|page| page["result"]["tools"].as_array().unwrap().iter().cloned())
        .collect();
    assert!(tools.len() > 2);
    validate_upstream_tools(&tools).unwrap();
    assert_eq!(read_tool_definition()["name"], "XcodeRead");
}

fn trusted_policy_definition() -> Value {
    json!({
        "schema_version": 1,
        "trust_policy_id": "trusted_local_project_v1",
        "scope": {
            "project_model": "explicitly_operator_trusted_local_project",
            "operator_admission": "exact_pinned_execution_root_project_and_uid",
            "operations": ["workspace_open", "XcodeRead"],
            "file_references": "xcode_project_organization_may_resolve_outside_checkout"
        },
        "assumptions": [
            "project_metadata_and_referenced_local_content_are_operator_trusted",
            "same_uid_external_clients_are_cooperative_not_isolated"
        ],
        "required_controls": [
            "explicit_operator_admission_reference_separate_from_policy_digest",
            "pinned_execution_root_project_uid_installation_and_service_generation",
            "current_evaluated_invocation_permissions_and_effective_tool_allowlist",
            "initial_structured_workspace_id_and_exact_canonical_requested_path",
            "fresh_structured_exact_open_path_status_before_each_scoped_dispatch",
            "explicit_bound_workspace_id_for_every_scoped_upstream_call",
            "current_trust_permission_tool_manifest_and_trust_policy_digest_checks",
            "durable_dispatch_fence_and_outcome_for_workspace_open",
            "unknown_effects_remain_held_without_automatic_replay",
            "invalidate_and_readmit_after_identity_mapping_trust_or_policy_drift"
        ],
        "excluded_guarantees": [
            "apple_sandbox_or_checkout_only_filesystem_confinement",
            "atomic_workspace_lifetime_or_execution_time_isolation",
            "status_revalidation_of_workspace_id_to_path_mapping",
            "operator_trust_decision_or_permission_grant_from_policy_digest"
        ],
        "prohibitions": [
            "automatic_apple_permission_grants_or_permission_allowlist_bypass",
            "admitting_build_test_execution_or_other_tools_from_project_trust_alone",
            "close_reopen_as_validation_or_recovery",
            "relabeling_existing_bindings_or_historical_observations_after_policy_change"
        ]
    })
}

#[test]
fn manifest_identity_binds_the_trusted_policy_definition_and_entry_pins() {
    let manifest = parse_unique_json(include_bytes!(
        "../../../contracts/xcode-headless/v1/manifest.json"
    ))
    .unwrap();
    let policy = trusted_policy_definition();
    let expected = json!({
        "manifest":manifest,
        "schemas":parse_unique_json(include_bytes!("../../../contracts/xcode-headless/v1/schemas.json")).unwrap(),
        "upstream_capture":parse_unique_json(include_bytes!("../../../contracts/xcode-headless/v1/upstream-capture.json")).unwrap(),
        "trust_policy":policy
    });
    assert_eq!(
        manifest_digest().unwrap(),
        canonical_digest("cw.xcode.manifest.v1", &expected).unwrap(),
        "manifest identity must bind the policy definition, not only its label"
    );
    let digest = canonical_digest("cw.xcode.trust-policy.v1", &policy).unwrap();
    assert_eq!(trust_policy_digest().unwrap(), digest);
    assert_ne!(
        canonical_digest("cw.xcode.manifest.v1", &policy).unwrap(),
        digest
    );
    assert_eq!(
        parse_unique_json(include_bytes!(
            "../../../contracts/xcode-headless/v1/policy.json"
        ))
        .unwrap(),
        policy
    );
    assert_eq!(manifest["trust_policy_digest"], digest);
    for entry in manifest["entries"].as_array().unwrap() {
        assert_eq!(entry["trust_policy_digest"], digest);
    }
}

#[test]
fn policy_drift_and_same_version_relabeling_fail_closed() {
    let manifest = parse_unique_json(include_bytes!(
        "../../../contracts/xcode-headless/v1/manifest.json"
    ))
    .unwrap();
    let policy = trusted_policy_definition();
    validate_trust_policy(&manifest, &policy).unwrap();
    for pointer in [
        "/schema_version",
        "/trust_policy_id",
        "/scope/file_references",
        "/required_controls/0",
    ] {
        let mut changed = policy.clone();
        *changed.pointer_mut(pointer).unwrap() = json!("changed");
        assert_eq!(
            validate_trust_policy(&manifest, &changed),
            Err(ContractError::InvalidBundle)
        );
    }
    for pointer in [
        "/trust_policy",
        "/trust_policy_digest_domain",
        "/trust_policy_id",
        "/trust_policy_digest",
        "/entries/0/trust_policy_id",
        "/entries/0/trust_policy_digest",
        "/entries/1/trust_policy_digest",
    ] {
        let mut changed = manifest.clone();
        *changed.pointer_mut(pointer).unwrap() = json!("changed");
        assert_eq!(
            validate_trust_policy(&changed, &policy),
            Err(ContractError::InvalidBundle)
        );
    }
    let mut changed = manifest.clone();
    changed["entries"][0]
        .as_object_mut()
        .unwrap()
        .remove("trust_policy_digest");
    assert_eq!(
        validate_trust_policy(&changed, &policy),
        Err(ContractError::InvalidBundle)
    );
    let mut changed_policy = policy.clone();
    changed_policy["scope"]["file_references"] = json!("different_policy_under_old_id");
    let digest = canonical_digest("cw.xcode.trust-policy.v1", &changed_policy).unwrap();
    let mut repinned = manifest.clone();
    repinned["trust_policy_digest"] = json!(digest);
    for entry in repinned["entries"].as_array_mut().unwrap() {
        entry["trust_policy_digest"] = json!(digest);
    }
    assert_eq!(
        validate_trust_policy(&repinned, &changed_policy),
        Err(ContractError::InvalidBundle)
    );
}

#[test]
fn organization_path_ecmascript_schema_matches_runtime_for_unicode_separators() {
    let definition = read_tool_definition();
    let schema = &definition["inputSchema"]["properties"]["filePath"];
    let mut cases = vec![
        ("Project/Source.swift".to_owned(), true),
        ("Shared Group/Source.swift".to_owned(), true),
        ("../Source.swift".to_owned(), false),
        ("Project/../Source.swift".to_owned(), false),
        ("Project//Source.swift".to_owned(), false),
        ("Project/".to_owned(), false),
        ("Project\n/Source.swift".to_owned(), false),
    ];
    for separator in ['\u{2028}', '\u{2029}'] {
        cases.extend([
            (format!("Group{separator}/Source.swift"), true),
            (format!("Group{separator}/../Source.swift"), false),
            (format!("Group{separator}/./Source.swift"), false),
            (format!("Group{separator}//Source.swift"), false),
            (format!("Group{separator}/.."), false),
            (format!("Group{separator}/."), false),
            (format!("Group{separator}/"), false),
        ]);
    }
    let input =
        json!({"schema":schema, "paths":cases.iter().map(|(path, _)| path).collect::<Vec<_>>()});
    // JSON Schema patterns have ECMAScript semantics, so use the real engine.
    let output = std::process::Command::new("node")
        .args([
            "-e",
            r#"
            const {schema, paths} = JSON.parse(process.argv[1]);
            const pattern = new RegExp(schema.pattern);
            process.stdout.write(JSON.stringify(paths.map(path =>
                Array.from(path).length >= schema.minLength &&
                Array.from(path).length <= schema.maxLength &&
                Buffer.byteLength(path, 'utf8') <= schema['x-maxUtf8Bytes'] &&
                pattern.test(path))));
        "#,
        ])
        .arg(input.to_string())
        .output()
        .expect("Node.js is required for the ECMAScript schema conformance test");
    assert!(
        output.status.success(),
        "ECMAScript schema check failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let schema_accepts: Vec<bool> = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(schema_accepts.len(), cases.len());
    for ((path, expected), schema_accepts) in cases.iter().zip(schema_accepts) {
        let runtime_accepts = ReadArguments::parse(&json!({"filePath":path}), "cw-alias").is_ok();
        assert_eq!(runtime_accepts, *expected, "runtime path {path:?}");
        assert_eq!(
            schema_accepts, runtime_accepts,
            "schema/runtime disagreement for {path:?}"
        );
    }
}
