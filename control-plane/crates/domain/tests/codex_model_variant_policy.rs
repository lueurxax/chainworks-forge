use std::fs;
use std::path::PathBuf;

use domain::codex_model_variant_policy::{
    load_pinned_policy_v1, parse_policy_json_v1, AUTHORED_CODEX_PROVIDER, CANONICAL_CODEX_PROVIDER,
    CODEX_MODEL_VARIANT_POLICY_BYTES_V1, CODEX_MODEL_VARIANT_POLICY_SHA256_V1,
};

fn repository_root() -> PathBuf {
    std::env::current_dir()
        .expect("test process should have a working directory")
        .ancestors()
        .find(|candidate| {
            candidate
                .join("examples/agents/codex-model-variant-matrix.v1.json")
                .is_file()
                && candidate.join("control-plane/Cargo.toml").is_file()
        })
        .map(std::path::Path::to_path_buf)
        .expect("test working directory should be inside the Chainworks repository")
}

fn fixture_path() -> PathBuf {
    repository_root().join("examples/agents/codex-model-variant-matrix.v1.json")
}

fn valid_fixture() -> Vec<u8> {
    fs::read(fixture_path()).expect("pinned policy fixture must exist")
}

#[test]
fn pinned_policy_bytes_and_matrix_are_exact() {
    let bytes = valid_fixture();
    assert_eq!(bytes.len(), CODEX_MODEL_VARIANT_POLICY_BYTES_V1);
    assert_eq!(CODEX_MODEL_VARIANT_POLICY_BYTES_V1, 1_479);
    assert_eq!(
        CODEX_MODEL_VARIANT_POLICY_SHA256_V1,
        "b6ad3f2047466a34da42241eae6b790f60bb835d9e6826cb77b51eb3fc558911"
    );

    let policy = load_pinned_policy_v1(&bytes).expect("fixture must match the pinned contract");
    assert_eq!(policy.provider, AUTHORED_CODEX_PROVIDER);
    assert_eq!(policy.canonical_provider, CANONICAL_CODEX_PROVIDER);
    assert_eq!(policy.variants.len(), 3);
    assert_eq!(policy.production_profiles.len(), 7);
    assert_eq!(
        policy
            .production_profile("codex_orchestrator_high")
            .expect("orchestrator profile")
            .model_id,
        "gpt-5.6-sol"
    );
    assert_eq!(
        policy
            .production_profile("codex_audit_high")
            .expect("audit profile")
            .effort,
        "ultra"
    );
    assert!(!policy
        .variant("gpt-5.6-luna")
        .expect("Luna variant")
        .allowed_efforts
        .iter()
        .any(|effort| effort == "ultra"));
}

#[test]
fn parser_rejects_duplicate_keys_and_unknown_fields() {
    let duplicate = br#"{
      "schema_version": 1,
      "schema_version": 1,
      "policy_id": "codex_model_variant_matrix_v1",
      "provider": "codex_acp",
      "canonical_provider": "codex",
      "variants": [],
      "production_profiles": []
    }"#;
    let error = parse_policy_json_v1(duplicate).expect_err("duplicate root key must fail");
    assert_eq!(error.code(), "policy_schema_invalid");

    let unknown = br#"{
      "schema_version": 1,
      "policy_id": "codex_model_variant_matrix_v1",
      "provider": "codex_acp",
      "canonical_provider": "codex",
      "variants": [],
      "production_profiles": [],
      "unexpected": true
    }"#;
    let error = parse_policy_json_v1(unknown).expect_err("unknown root field must fail");
    assert_eq!(error.code(), "policy_schema_invalid");
}

#[test]
fn parser_rejects_duplicate_ids_and_invalid_production_rows() {
    let valid = valid_fixture();
    let text = String::from_utf8(valid).unwrap();

    let duplicate_model = text.replacen(
        "\"model_id\": \"gpt-5.6-terra\"",
        "\"model_id\": \"gpt-5.6-sol\"",
        1,
    );
    assert_eq!(
        parse_policy_json_v1(duplicate_model.as_bytes())
            .expect_err("duplicate model id must fail")
            .code(),
        "policy_schema_invalid"
    );

    let generic = text.replacen(
        "\"model_id\": \"gpt-5.6-sol\",\n      \"effort\": \"max\"",
        "\"model_id\": \"gpt-5.6\",\n      \"effort\": \"max\"",
        1,
    );
    assert_eq!(
        parse_policy_json_v1(generic.as_bytes())
            .expect_err("undeclared generic production model must fail")
            .code(),
        "policy_schema_invalid"
    );

    let luna_ultra = text.replacen(
        "\"model_id\": \"gpt-5.6-luna\",\n      \"effort\": \"high\"",
        "\"model_id\": \"gpt-5.6-luna\",\n      \"effort\": \"ultra\"",
        1,
    );
    assert_eq!(
        parse_policy_json_v1(luna_ultra.as_bytes())
            .expect_err("Luna ultra must fail")
            .code(),
        "policy_schema_invalid"
    );
}

#[test]
fn pinned_loader_distinguishes_byte_integrity_from_schema_errors() {
    let bytes = valid_fixture();
    for mutation in [
        bytes[..bytes.len() - 1].to_vec(),
        {
            let mut value = bytes.clone();
            value[0] = b'[';
            value
        },
        {
            let mut value = bytes.clone();
            value.push(b'\n');
            value
        },
    ] {
        let error = load_pinned_policy_v1(&mutation).expect_err("mutated bytes must fail");
        assert_eq!(error.code(), "policy_bytes_mismatch");
    }

    let malformed_utf8 = [0xff, b'\n'];
    let error = parse_policy_json_v1(&malformed_utf8).expect_err("invalid UTF-8 must fail");
    assert_eq!(error.code(), "policy_schema_invalid");
}
