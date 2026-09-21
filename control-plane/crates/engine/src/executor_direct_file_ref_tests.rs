use super::*;

const BASELINE_BYTES: &[u8] = br#"{"status":"ready","summary":"approved implementation plan"}"#;
const UPDATED_BYTES: &[u8] =
    br#"{"status":"ready","summary":"plan reconciled with the current attempt"}"#;

struct DirectFileFixture {
    _root: tempfile::TempDir,
    declared: DeclaredOutput,
    spec: ExpectedOutputSpec,
    baseline: Vec<PrePromptExpectedOutputMetadata>,
}

impl DirectFileFixture {
    fn new(reuse_policy: OutputReusePolicy) -> Self {
        let root = tempfile::tempdir().unwrap();
        let meta_root = root.path().join(".chainworks/runs/current-run");
        let target = meta_root.join("implementation/plan.json");
        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
        std::fs::write(&target, BASELINE_BYTES).unwrap();
        // This is an agent-owned output. The run_state projection has its own
        // control-plane ownership contract and must not relax this boundary.
        let declared = DeclaredOutput {
            output_name: "implementation_plan".to_string(),
            target_path: target.to_string_lossy().into_owned(),
            schema: Some(workflow::plan::OutputSchema {
                contract_id: "implementation_plan_v1".to_string(),
                format: "json".to_string(),
                human_format: None,
                machine_format: Some("json".to_string()),
                validation_mode: Some("strict_structured".to_string()),
                normalized_artifact_name: None,
                raw_artifact_name: None,
                required_fields: vec!["status".to_string(), "summary".to_string()],
            }),
            reuse_policy: Some(reuse_policy),
            companion_output_name: None,
            companion_path: None,
        };
        let spec = build_expected_output_specs(
            std::slice::from_ref(&declared),
            root.path().to_str().unwrap(),
            None,
            Some(meta_root.to_str().unwrap()),
            false,
        )
        .remove(0);
        assert_eq!(spec.source_generation_owner, SourceGenerationOwner::Agent);
        let context = PrePromptExpectedOutputContext {
            agent_execution_id: "current-execution".to_string(),
            stage_execution_id: "current-stage".to_string(),
            attempt_number: 2,
            session_generation_id: "fresh-session".to_string(),
            prompt_turn_id: "current-turn".to_string(),
            discovery_generation_id: "current-discovery".to_string(),
        };
        let baseline = vec![
            StdDiscoveryFilesystem::capture_pre_prompt_expected_output_metadata(&spec, &context),
        ];
        assert_eq!(
            baseline[0].baseline_status,
            ExpectedPathBaselineStatus::RegularContentCaptured
        );
        Self {
            _root: root,
            declared,
            spec,
            baseline,
        }
    }

    fn manifest(&self, bytes: &[u8]) -> serde_json::Value {
        // Both observed Codex and Claude envelopes used this exact object
        // shape, keyed by the absolute canonical output path.
        serde_json::json!({
            "mode": "direct_file",
            "output_name": self.spec.output_name,
            "path": self.spec.target_path,
            "digest": sha256_digest(bytes),
            "size_bytes": bytes.len(),
        })
    }

    fn settlement(&self, manifest: serde_json::Value) -> DeclaredOutputDiscoverySettlement {
        let artifact = acp::DiscoveredArtifact {
            name: self.spec.target_path.clone(),
            content: serde_json::to_vec(&manifest).unwrap(),
            source_path: None,
            source_kind: acp::DiscoveredArtifactSourceKind::ChainworksOutput,
        };
        build_declared_output_discovery_settlement(
            std::slice::from_ref(&self.spec),
            &[artifact],
            &self.baseline,
        )
    }

    fn assert_rejected(&self, manifest: serde_json::Value, reason: &str) {
        let settlement = self.settlement(manifest);
        assert_eq!(settlement.decisions.len(), 1);
        let decision = &settlement.decisions[0];
        assert_eq!(decision.status, OutputDiscoveryStatus::Rejected);
        assert_eq!(decision.reason, OutputDiscoveryReason::ContractInvalid);
        assert_eq!(
            decision
                .diagnostics
                .get("direct_file_ref_rejection")
                .map(String::as_str),
            Some(reason)
        );
        assert!(settlement.accepted_payloads.is_empty());
        let captured = build_captured_outputs_from_discovery_decisions(
            std::slice::from_ref(&self.declared),
            &settlement.decisions,
            &settlement.accepted_payloads,
        );
        assert!(captured[0].machine_bytes.is_none());
        assert_eq!(
            validate_task_outputs(&captured).failure_class,
            Some(domain::validation::ValidationFailureClass::NoOutputProduced)
        );
    }
}

#[test]
fn real_provider_direct_file_manifest_cannot_relabel_unchanged_bytes_as_fresh() {
    let fixture = DirectFileFixture::new(OutputReusePolicy::MustProduce);
    fixture.assert_rejected(
        fixture.manifest(BASELINE_BYTES),
        "canonical_file_unchanged_from_pre_prompt_baseline",
    );
}

#[test]
fn real_provider_direct_file_manifest_accepts_changed_bytes_and_validates_captured_content() {
    let fixture = DirectFileFixture::new(OutputReusePolicy::MustProduce);
    std::fs::write(&fixture.spec.target_path, UPDATED_BYTES).unwrap();
    let settlement = fixture.settlement(fixture.manifest(UPDATED_BYTES));
    let decision = &settlement.decisions[0];
    assert_eq!(decision.status, OutputDiscoveryStatus::Accepted);
    assert_eq!(decision.reason, OutputDiscoveryReason::ExactPathChanged);
    assert_eq!(
        decision.provenance,
        Some(OutputDiscoveryProvenance::ExactPath)
    );
    let captured = build_captured_outputs_from_discovery_decisions(
        std::slice::from_ref(&fixture.declared),
        &settlement.decisions,
        &settlement.accepted_payloads,
    );
    assert_eq!(captured[0].machine_bytes.as_deref(), Some(UPDATED_BYTES));
    assert!(validate_task_outputs(&captured).failure_class.is_none());
}

#[test]
fn declared_reuse_accepts_unchanged_direct_file_bytes_with_explicit_provenance() {
    let fixture = DirectFileFixture::new(OutputReusePolicy::AllowUnchangedExisting);
    let settlement = fixture.settlement(fixture.manifest(BASELINE_BYTES));
    let decision = &settlement.decisions[0];
    assert_eq!(decision.status, OutputDiscoveryStatus::Accepted);
    assert_eq!(decision.reason, OutputDiscoveryReason::DeclaredReusePolicy);
    assert_eq!(
        decision.provenance,
        Some(OutputDiscoveryProvenance::DeclaredReusePolicy)
    );
    assert_eq!(
        decision.accepted_payload_ref.as_deref(),
        Some(declared_reuse_policy_payload_ref("implementation_plan").as_str())
    );
    let captured = build_captured_outputs_from_discovery_decisions(
        std::slice::from_ref(&fixture.declared),
        &settlement.decisions,
        &settlement.accepted_payloads,
    );
    assert_eq!(captured[0].machine_bytes.as_deref(), Some(BASELINE_BYTES));
    assert!(validate_task_outputs(&captured).failure_class.is_none());
    assert_eq!(
        std::fs::read(&fixture.spec.target_path).unwrap(),
        BASELINE_BYTES
    );
}

#[test]
fn declared_reuse_does_not_override_manifest_digest_size_path_or_output_identity() {
    let fixture = DirectFileFixture::new(OutputReusePolicy::AllowUnchangedExisting);
    for (field, value, reason) in [
        (
            "digest",
            serde_json::json!(sha256_digest(b"different bytes")),
            "manifest_digest_differs_from_canonical_file",
        ),
        (
            "size_bytes",
            serde_json::json!(BASELINE_BYTES.len() + 1),
            "manifest_size_differs_from_canonical_file",
        ),
        (
            "path",
            serde_json::json!(format!("{}.other", fixture.spec.target_path)),
            "manifest_path_differs_from_declared_target",
        ),
        (
            "output_name",
            serde_json::json!("another_output"),
            "invalid_manifest_or_output_identity",
        ),
    ] {
        let mut manifest = fixture.manifest(BASELINE_BYTES);
        manifest[field] = value;
        fixture.assert_rejected(manifest, reason);
    }
}

#[test]
fn direct_file_reference_without_pre_prompt_baseline_is_not_proof_of_freshness() {
    for policy in [
        OutputReusePolicy::MustProduce,
        OutputReusePolicy::AllowUnchangedExisting,
    ] {
        let mut fixture = DirectFileFixture::new(policy);
        fixture.baseline.clear();
        fixture.assert_rejected(
            fixture.manifest(BASELINE_BYTES),
            "pre_prompt_baseline_missing",
        );
    }
}

#[test]
fn declared_reuse_cannot_accept_another_runs_canonical_file() {
    let mut fixture = DirectFileFixture::new(OutputReusePolicy::AllowUnchangedExisting);
    let other_run = fixture
        ._root
        .path()
        .join(".chainworks/runs/other-run/plan.json");
    std::fs::create_dir_all(other_run.parent().unwrap()).unwrap();
    std::fs::write(&other_run, BASELINE_BYTES).unwrap();
    fixture.spec.target_path = other_run.to_string_lossy().into_owned();
    fixture.declared.target_path = fixture.spec.target_path.clone();
    fixture.baseline[0].target_path = fixture.spec.target_path.clone();
    fixture.assert_rejected(
        fixture.manifest(BASELINE_BYTES),
        "canonical_file_wrong_run_meta_root",
    );
}

#[cfg(unix)]
#[test]
fn declared_reuse_cannot_follow_canonical_output_symlink_outside_authorized_root() {
    let fixture = DirectFileFixture::new(OutputReusePolicy::AllowUnchangedExisting);
    let outside = tempfile::tempdir().unwrap();
    let outside_file = outside.path().join("plan.json");
    std::fs::write(&outside_file, BASELINE_BYTES).unwrap();
    std::fs::remove_file(&fixture.spec.target_path).unwrap();
    std::os::unix::fs::symlink(&outside_file, &fixture.spec.target_path).unwrap();
    fixture.assert_rejected(
        fixture.manifest(BASELINE_BYTES),
        "canonical_file_symlink_escape",
    );
    assert_eq!(std::fs::read(outside_file).unwrap(), BASELINE_BYTES);
}

#[test]
fn declared_reuse_does_not_bypass_output_byte_limit() {
    let mut fixture = DirectFileFixture::new(OutputReusePolicy::AllowUnchangedExisting);
    fixture.spec.max_bytes = BASELINE_BYTES.len() as u64 - 1;
    fixture.assert_rejected(
        fixture.manifest(BASELINE_BYTES),
        "canonical_file_exceeds_output_limit",
    );
}
