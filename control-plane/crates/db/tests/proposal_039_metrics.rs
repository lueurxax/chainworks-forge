#[test]
fn operational_metrics_have_closed_labels_and_bounded_samples() {
    use db::metrics as m;
    use domain::run_carry_forward_api::{
        ContinuationPhase as Phase, ContinuationReasonCode as Code,
    };
    let before = m::get_counter("run_continuation_admission_total");
    m::record_continuation_command(false, "denied", Some(Code::RolloutHold));
    m::record_continuation_command(true, "accepted", None);
    m::record_continuation_command(true, "/private/credential-canary", None);
    m::record_continuation_guard_denied();
    m::record_continuation_preserved_bytes(1234);
    for _ in 0..1500 {
        m::record_continuation_phase_duration(
            Phase::Preparing,
            std::time::Duration::from_millis(123),
        );
    }
    assert!(m::get_counter("run_continuation_admission_total") >= before + 3);
    assert!(
        m::get_counter_with_label(
            "run_continuation_admission_total",
            "result=denied,reason=rollout_hold"
        ) > 0
    );
    assert_eq!(
        m::get_counter_with_label(
            "run_continuation_activation_total",
            "result=/private/credential-canary"
        ),
        0
    );
    assert!(
        m::get_counter_with_label("run_continuation_activation_total", "result=internal_error") > 0
    );
    let values = m::p039_rollout_metric_values_json();
    assert_eq!(
        values["run_continuation_phase_duration_ms"]["preparing"]["sample_count"],
        1024
    );
    assert_eq!(
        values["run_continuation_phase_duration_ms"]["preparing"]["p95"],
        123
    );
    assert_eq!(
        values.as_object().unwrap().len(),
        m::P039_REQUIRED_METRICS.len()
    );
    assert!(!values.to_string().contains("credential-canary"));
}
