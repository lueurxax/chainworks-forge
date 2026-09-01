use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

pub const AUTHORED_CODEX_PROVIDER: &str = "codex_acp";
pub const CANONICAL_CODEX_PROVIDER: &str = "codex";
pub const CODEX_MODEL_VARIANT_POLICY_FILE_V1: &str = "codex-model-variant-matrix.v1.json";
pub const CODEX_MODEL_VARIANT_POLICY_BYTES_V1: usize = 1_479;
pub const CODEX_MODEL_VARIANT_POLICY_SHA256_V1: &str =
    "b6ad3f2047466a34da42241eae6b790f60bb835d9e6826cb77b51eb3fc558911";

const POLICY_ID_V1: &str = "codex_model_variant_matrix_v1";
const EFFORT_VOCABULARY: &[&str] = &["low", "medium", "high", "xhigh", "max", "ultra"];

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum CodexModelVariantPolicyError {
    #[error("policy_schema_invalid: {0}")]
    SchemaInvalid(String),
    #[error("policy_bytes_mismatch: {0}")]
    BytesMismatch(String),
}

impl CodexModelVariantPolicyError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::SchemaInvalid(_) => "policy_schema_invalid",
            Self::BytesMismatch(_) => "policy_bytes_mismatch",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CodexModelVariantPolicyV1 {
    pub schema_version: u32,
    pub policy_id: String,
    pub provider: String,
    pub canonical_provider: String,
    pub variants: Vec<CodexModelVariantV1>,
    pub production_profiles: Vec<CodexProductionProfileV1>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CodexModelVariantV1 {
    pub model_id: String,
    pub display_name: String,
    pub allowed_efforts: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CodexProductionProfileV1 {
    pub backend_profile_id: String,
    pub model_id: String,
    pub effort: String,
}

impl CodexModelVariantPolicyV1 {
    pub fn variant(&self, model_id: &str) -> Option<&CodexModelVariantV1> {
        self.variants
            .iter()
            .find(|variant| variant.model_id == model_id)
    }

    pub fn production_profile(
        &self,
        backend_profile_id: &str,
    ) -> Option<&CodexProductionProfileV1> {
        self.production_profiles
            .iter()
            .find(|profile| profile.backend_profile_id == backend_profile_id)
    }
}

pub fn parse_policy_json_v1(
    bytes: &[u8],
) -> Result<CodexModelVariantPolicyV1, CodexModelVariantPolicyError> {
    let policy: CodexModelVariantPolicyV1 = serde_json::from_slice(bytes)
        .map_err(|error| CodexModelVariantPolicyError::SchemaInvalid(error.to_string()))?;
    validate_policy(&policy)?;
    Ok(policy)
}

pub fn load_pinned_policy_v1(
    bytes: &[u8],
) -> Result<CodexModelVariantPolicyV1, CodexModelVariantPolicyError> {
    if !bytes.ends_with(b"\n") {
        return Err(CodexModelVariantPolicyError::BytesMismatch(
            "final LF is required".to_string(),
        ));
    }
    if bytes.len() != CODEX_MODEL_VARIANT_POLICY_BYTES_V1 {
        return Err(CodexModelVariantPolicyError::BytesMismatch(format!(
            "expected {} bytes, got {}",
            CODEX_MODEL_VARIANT_POLICY_BYTES_V1,
            bytes.len()
        )));
    }
    let digest = format!("{:x}", Sha256::digest(bytes));
    if digest != CODEX_MODEL_VARIANT_POLICY_SHA256_V1 {
        return Err(CodexModelVariantPolicyError::BytesMismatch(format!(
            "expected SHA-256 {CODEX_MODEL_VARIANT_POLICY_SHA256_V1}, got {digest}"
        )));
    }
    parse_policy_json_v1(bytes)
}

fn validate_policy(policy: &CodexModelVariantPolicyV1) -> Result<(), CodexModelVariantPolicyError> {
    if policy.schema_version != 1 {
        return schema_error("schema_version must be 1");
    }
    if policy.policy_id != POLICY_ID_V1 {
        return schema_error("policy_id is not codex_model_variant_matrix_v1");
    }
    if policy.provider != AUTHORED_CODEX_PROVIDER
        || policy.canonical_provider != CANONICAL_CODEX_PROVIDER
    {
        return schema_error("provider tokens do not match the V1 contract");
    }
    if policy.variants.is_empty() || policy.production_profiles.is_empty() {
        return schema_error("variants and production_profiles must be nonempty");
    }

    let mut model_ids = HashSet::new();
    let mut display_names = HashSet::new();
    for variant in &policy.variants {
        if variant.model_id.trim() != variant.model_id
            || variant.model_id.is_empty()
            || variant.model_id == "gpt-5.6"
        {
            return schema_error("variant model_id is blank, padded, or generic");
        }
        if !model_ids.insert(variant.model_id.as_str()) {
            return schema_error("duplicate variant model_id");
        }
        if variant.display_name.trim() != variant.display_name || variant.display_name.is_empty() {
            return schema_error("variant display_name is blank or padded");
        }
        if !display_names.insert(variant.display_name.as_str()) {
            return schema_error("duplicate variant display_name");
        }
        if variant.allowed_efforts.is_empty() {
            return schema_error("variant allowed_efforts must be nonempty");
        }
        let mut efforts = HashSet::new();
        for effort in &variant.allowed_efforts {
            if !EFFORT_VOCABULARY.contains(&effort.as_str()) {
                return schema_error("variant declares an unsupported effort");
            }
            if !efforts.insert(effort.as_str()) {
                return schema_error("variant declares a duplicate effort");
            }
        }
        if variant.model_id == "gpt-5.6-luna"
            && variant
                .allowed_efforts
                .iter()
                .any(|effort| effort == "ultra")
        {
            return schema_error("Luna does not support ultra effort");
        }
    }

    let mut profile_ids = HashSet::new();
    for profile in &policy.production_profiles {
        if profile.backend_profile_id.trim() != profile.backend_profile_id
            || profile.backend_profile_id.is_empty()
            || !profile_ids.insert(profile.backend_profile_id.as_str())
        {
            return schema_error("production backend_profile_id is blank, padded, or duplicate");
        }
        let Some(variant) = policy.variant(&profile.model_id) else {
            return schema_error("production profile references an undeclared model");
        };
        if !variant
            .allowed_efforts
            .iter()
            .any(|effort| effort == &profile.effort)
        {
            return schema_error("production profile uses an unsupported effort");
        }
    }
    Ok(())
}

fn schema_error<T>(message: impl Into<String>) -> Result<T, CodexModelVariantPolicyError> {
    Err(CodexModelVariantPolicyError::SchemaInvalid(message.into()))
}
