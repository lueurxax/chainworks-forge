use domain::run_carry_forward_api::RunCarryForwardPreviewHoldV1;
use serde_json::{json, Value};

fn hold() -> Value {
    json!({"schema_version":"run_carry_forward_preview_hold_v1",
        "source_run_id":"00000000-0000-4000-8000-000000000039",
        "holds":[{"code":"unsupported_frontier","message":"Source frontier is not supported"}],
        "next_action":"inspect"})
}

#[test]
fn preview_hold_is_strict_nonempty_and_has_no_fabricated_witness() {
    for action in ["inspect", "resolve_hold", "refresh_preview"] {
        let mut value = hold();
        value["next_action"] = json!(action);
        let dto: RunCarryForwardPreviewHoldV1 = serde_json::from_value(value.clone()).unwrap();
        dto.validate().unwrap();
        assert_eq!(serde_json::to_value(dto).unwrap(), value);
    }
    for action in [
        "none",
        "await_worker",
        "read_operation",
        "explicit_reconcile",
        "unknown",
    ] {
        let mut value = hold();
        value["next_action"] = json!(action);
        assert!(serde_json::from_value::<RunCarryForwardPreviewHoldV1>(value).is_err());
    }
    for holds in [
        json!([]),
        json!(null),
        json!(vec![hold()["holds"][0].clone(); 129]),
    ] {
        let mut value = hold();
        value["holds"] = holds;
        assert!(serde_json::from_value::<RunCarryForwardPreviewHoldV1>(value).is_err());
    }
    for key in [
        "plan_sha256",
        "source_witness",
        "operation_id",
        "caller_class",
    ] {
        let mut value = hold();
        value[key] = json!("not accepted");
        assert!(serde_json::from_value::<RunCarryForwardPreviewHoldV1>(value).is_err());
    }
    for key in ["schema_version", "source_run_id", "holds", "next_action"] {
        let mut value = hold();
        value.as_object_mut().unwrap().remove(key);
        assert!(serde_json::from_value::<RunCarryForwardPreviewHoldV1>(value).is_err());
    }
    let mut unknown = hold();
    unknown["schema_version"] = json!("run_carry_forward_preview_hold_v2");
    assert!(serde_json::from_value::<RunCarryForwardPreviewHoldV1>(unknown).is_err());
}

#[test]
fn preview_hold_rejects_raw_duplicate_fields_and_noncanonical_source_ids() {
    for suffix in [
        r#""next_action":"inspect""#,
        r#""holds":[]"#,
        r#""source_run_id":"00000000-0000-4000-8000-000000000039""#,
    ] {
        let raw = format!(
            "{},{}{}",
            hold().to_string().trim_end_matches('}'),
            suffix,
            "}"
        );
        assert!(serde_json::from_str::<RunCarryForwardPreviewHoldV1>(&raw).is_err());
    }
    let mut value = hold();
    value["holds"][0]["unexpected"] = json!(true);
    assert!(serde_json::from_value::<RunCarryForwardPreviewHoldV1>(value).is_err());
    let mut value = hold();
    value["source_run_id"] = json!("00000000000040008000000000000039");
    assert!(serde_json::from_value::<RunCarryForwardPreviewHoldV1>(value).is_err());
}
