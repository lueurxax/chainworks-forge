//! Pinned, fail-closed Xcode open/read contracts. No I/O, dispatch or authorization.
//!
//! Trusted project organization paths can reference files outside the checkout.
//! These adapters do not establish filesystem confinement or effect completion.

use crate::xcode_effect::SchemaIdentity;
use serde::de::{self, DeserializeSeed, MapAccess, SeqAccess, Visitor};
use serde::{Deserializer, Serialize};
use serde_json::{json, Map, Number, Value};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::fmt;

pub const TRUST_POLICY_ID: &str = "trusted_local_project_v1";
pub const MAX_JSON_BYTES: usize = 1024 * 1024;
pub const MAX_CONTENT_BYTES: usize = 256 * 1024;
pub const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;
const MAX_DEPTH: usize = 128;
const TRUST_POLICY_DIGEST_DOMAIN: &str = "cw.xcode.trust-policy.v1";
const SUPPORTED_TRUST_POLICY_DIGEST: &str =
    "d172d6beb3acfa3e063f28588a856dda4079ba7f841de3e3e47fb81182eaa907";
const MANIFEST: &[u8] = include_bytes!("../../../contracts/xcode-headless/v1/manifest.json");
const SCHEMAS: &[u8] = include_bytes!("../../../contracts/xcode-headless/v1/schemas.json");
const CAPTURE: &[u8] = include_bytes!("../../../contracts/xcode-headless/v1/upstream-capture.json");
const TRUST_POLICY: &[u8] = include_bytes!("../../../contracts/xcode-headless/v1/policy.json");

pub type Result<T> = std::result::Result<T, ContractError>;

/// Deliberately carries no upstream strings, paths, parser errors or payloads.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum ContractError {
    #[error("JSON input exceeds the contract bound.")]
    InputTooLarge,
    #[error("JSON input is invalid.")]
    InvalidJson,
    #[error("JSON contains a duplicate object key.")]
    DuplicateKey,
    #[error("JSON number is outside the safe integer range.")]
    UnsafeNumber,
    #[error("Digest domain is invalid.")]
    InvalidDomain,
    #[error("The upstream contract is not supported.")]
    SchemaDrift,
    #[error("Tool arguments do not match the contract.")]
    InvalidArguments,
    #[error("Tool response envelope does not match the contract.")]
    InvalidEnvelope,
    #[error("The upstream tool reported an error.")]
    UpstreamError,
    #[error("Tool result does not match the contract.")]
    InvalidResult,
    #[error("The embedded contract bundle is invalid.")]
    InvalidBundle,
}

impl ContractError {
    pub fn code(self) -> &'static str {
        match self {
            Self::InputTooLarge => "input_too_large",
            Self::InvalidJson => "invalid_json",
            Self::DuplicateKey => "duplicate_key",
            Self::UnsafeNumber => "unsafe_number",
            Self::InvalidDomain => "invalid_domain",
            Self::SchemaDrift => "schema_drift",
            Self::InvalidArguments => "invalid_arguments",
            Self::InvalidEnvelope => "invalid_envelope",
            Self::UpstreamError => "upstream_error",
            Self::InvalidResult => "invalid_result",
            Self::InvalidBundle => "invalid_bundle",
        }
    }

    pub fn to_value(self) -> Value {
        json!({"schema_version":1, "error":{
            "code":self.code(), "message":self.to_string(), "retry":"never"
        }})
    }
}

/// Use at the raw wire boundary, before serde_json::Value can discard duplicates.
pub fn parse_unique_json(bytes: &[u8]) -> Result<Value> {
    if bytes.len() > MAX_JSON_BYTES {
        return Err(ContractError::InputTooLarge);
    }
    let mut deserializer = serde_json::Deserializer::from_slice(bytes);
    let mut reason = None;
    let value = Unique {
        reason: &mut reason,
    }
    .deserialize(&mut deserializer)
    .map_err(|_| reason.unwrap_or(ContractError::InvalidJson))?;
    deserializer.end().map_err(|_| ContractError::InvalidJson)?;
    Ok(value)
}

struct Unique<'a> {
    reason: &'a mut Option<ContractError>,
}

impl<'de> DeserializeSeed<'de> for Unique<'_> {
    type Value = Value;

    fn deserialize<D: Deserializer<'de>>(
        self,
        deserializer: D,
    ) -> std::result::Result<Value, D::Error> {
        deserializer.deserialize_any(self)
    }
}

impl Unique<'_> {
    fn number<E: de::Error>(self, number: Number) -> std::result::Result<Value, E> {
        if let Err(error) = check_number(&number) {
            *self.reason = Some(error);
            return Err(E::custom("unsafe number"));
        }
        Ok(Value::Number(number))
    }
}

impl<'de> Visitor<'de> for Unique<'_> {
    type Value = Value;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("unique-key JSON")
    }

    fn visit_bool<E: de::Error>(self, value: bool) -> std::result::Result<Value, E> {
        Ok(Value::Bool(value))
    }
    fn visit_unit<E: de::Error>(self) -> std::result::Result<Value, E> {
        Ok(Value::Null)
    }
    fn visit_str<E: de::Error>(self, value: &str) -> std::result::Result<Value, E> {
        Ok(Value::String(value.to_owned()))
    }
    fn visit_string<E: de::Error>(self, value: String) -> std::result::Result<Value, E> {
        Ok(Value::String(value))
    }
    fn visit_i64<E: de::Error>(self, value: i64) -> std::result::Result<Value, E> {
        self.number(value.into())
    }
    fn visit_u64<E: de::Error>(self, value: u64) -> std::result::Result<Value, E> {
        self.number(value.into())
    }
    fn visit_f64<E: de::Error>(self, value: f64) -> std::result::Result<Value, E> {
        let number = Number::from_f64(value).ok_or_else(|| E::custom("non-finite number"))?;
        self.number(number)
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> std::result::Result<Value, A::Error> {
        let mut values = Vec::new();
        while let Some(value) = seq.next_element_seed(Unique {
            reason: &mut *self.reason,
        })? {
            values.push(value);
        }
        Ok(Value::Array(values))
    }
    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> std::result::Result<Value, A::Error> {
        let mut values = Map::new();
        while let Some(key) = map.next_key::<String>()? {
            if values.contains_key(&key) {
                *self.reason = Some(ContractError::DuplicateKey);
                return Err(de::Error::custom("duplicate key"));
            }
            let value = map.next_value_seed(Unique {
                reason: &mut *self.reason,
            })?;
            values.insert(key, value);
        }
        Ok(Value::Object(values))
    }
}

fn check_number(number: &Number) -> Result<()> {
    let value = number.as_f64().ok_or(ContractError::UnsafeNumber)?;
    if !value.is_finite() || value.abs() > MAX_SAFE_INTEGER as f64 {
        return Err(ContractError::UnsafeNumber);
    }
    Ok(())
}

fn check_numbers(value: &Value, depth: usize) -> Result<()> {
    if depth > MAX_DEPTH {
        return Err(ContractError::InvalidJson);
    }
    match value {
        Value::Number(number) => check_number(number),
        Value::Array(values) => values.iter().try_for_each(|v| check_numbers(v, depth + 1)),
        Value::Object(values) => values
            .values()
            .try_for_each(|v| check_numbers(v, depth + 1)),
        _ => Ok(()),
    }
}

pub fn canonical_bytes(value: &Value) -> Result<Vec<u8>> {
    check_numbers(value, 0)?;
    serde_json_canonicalizer::to_vec(value).map_err(|_| ContractError::InvalidJson)
}

pub fn canonical_digest(domain: &str, value: &Value) -> Result<String> {
    if domain.is_empty() || domain.len() > 256 || domain.chars().any(char::is_control) {
        return Err(ContractError::InvalidDomain);
    }
    let mut hash = Sha256::new();
    hash.update(domain.as_bytes());
    hash.update([0]);
    hash.update(canonical_bytes(value)?);
    Ok(format!("{:x}", hash.finalize()))
}

fn bundle() -> Result<Value> {
    let bundle = json!({
        "manifest":parse_unique_json(MANIFEST).map_err(|_| ContractError::InvalidBundle)?,
        "schemas":parse_unique_json(SCHEMAS).map_err(|_| ContractError::InvalidBundle)?,
        "upstream_capture":parse_unique_json(CAPTURE).map_err(|_| ContractError::InvalidBundle)?,
        "trust_policy":parse_unique_json(TRUST_POLICY).map_err(|_| ContractError::InvalidBundle)?
    });
    validate_trust_policy(&bundle["manifest"], &bundle["trust_policy"])?;
    Ok(bundle)
}

/// Checks imported policy pins against the immutable supported v1 definition.
/// This establishes contract identity, not operator trust or a permission grant.
pub fn validate_trust_policy(manifest: &Value, policy: &Value) -> Result<()> {
    let error = ContractError::InvalidBundle;
    let digest = canonical_digest(TRUST_POLICY_DIGEST_DOMAIN, policy).map_err(|_| error)?;
    if policy["schema_version"] != 1
        || policy["trust_policy_id"] != TRUST_POLICY_ID
        || digest != SUPPORTED_TRUST_POLICY_DIGEST
        || manifest["trust_policy"] != "policy.json"
        || manifest["trust_policy_digest_domain"] != TRUST_POLICY_DIGEST_DOMAIN
        || manifest["trust_policy_id"] != TRUST_POLICY_ID
        || manifest["trust_policy_digest"] != digest
    {
        return Err(error);
    }
    let entries = manifest["entries"]
        .as_array()
        .filter(|v| !v.is_empty())
        .ok_or(error)?;
    for entry in entries {
        if entry["trust_policy_id"] != TRUST_POLICY_ID || entry["trust_policy_digest"] != digest {
            return Err(error);
        }
    }
    Ok(())
}

pub fn trust_policy_digest() -> Result<String> {
    canonical_digest(TRUST_POLICY_DIGEST_DOMAIN, &bundle()?["trust_policy"])
}

pub fn manifest_digest() -> Result<String> {
    canonical_digest("cw.xcode.manifest.v1", &bundle()?)
}

/// Pins the complete open adapter entry, request/result/error schemas and capture.
pub fn open_schema_identity() -> Result<SchemaIdentity> {
    let bundle = bundle()?;
    let entry = bundle["manifest"]["entries"]
        .as_array()
        .and_then(|entries| entries.iter().find(|e| e["name"] == "workspace_open"))
        .ok_or(ContractError::InvalidBundle)?;
    let schema = json!({"entry":entry,
        "request":schema_from_bundle(&bundle, "openArguments")?,
        "result":schema_from_bundle(&bundle, "openResult")?,
        "error":schema_from_bundle(&bundle, "errorResult")?,
        "upstream_result":schema_from_bundle(&bundle, "openEnvelope")?,
        "upstream_capture":bundle["upstream_capture"]
    });
    Ok(SchemaIdentity {
        contract_id: "workspace_open".into(),
        version: 1,
        manifest_digest: canonical_digest("cw.xcode.manifest.v1", &bundle)?,
        schema_digest: canonical_digest("cw.xcode.schema.v1", &schema)?,
    })
}

fn schema_from_bundle(bundle: &Value, name: &str) -> Result<Value> {
    let definitions = bundle["schemas"]["$defs"]
        .as_object()
        .ok_or(ContractError::InvalidBundle)?;
    expand_schema(
        definitions.get(name).ok_or(ContractError::InvalidBundle)?,
        definitions,
        0,
    )
}

fn expand_schema(value: &Value, definitions: &Map<String, Value>, depth: usize) -> Result<Value> {
    if depth > MAX_DEPTH {
        return Err(ContractError::InvalidBundle);
    }
    match value {
        Value::Object(object) => {
            if let Some(reference) = object.get("$ref") {
                let name = reference
                    .as_str()
                    .and_then(|s| s.strip_prefix("#/$defs/"))
                    .ok_or(ContractError::InvalidBundle)?;
                if object.len() != 1 {
                    return Err(ContractError::InvalidBundle);
                }
                return expand_schema(
                    definitions.get(name).ok_or(ContractError::InvalidBundle)?,
                    definitions,
                    depth + 1,
                );
            }
            let mut result = Map::new();
            for (key, value) in object {
                result.insert(key.clone(), expand_schema(value, definitions, depth + 1)?);
            }
            Ok(Value::Object(result))
        }
        Value::Array(values) => values
            .iter()
            .map(|v| expand_schema(v, definitions, depth + 1))
            .collect::<Result<Vec<_>>>()
            .map(Value::Array),
        _ => Ok(value.clone()),
    }
}

/// Static provider schema; contains neither a live upstream ID nor its path.
pub fn read_tool_definition() -> Value {
    let bundle = bundle().expect("checked-in Xcode contract bundle must be valid");
    json!({
        "name":"XcodeRead",
        "description":"Read a file by its path in the trusted Xcode project organization.",
        "inputSchema":schema_from_bundle(&bundle, "readArguments").expect("checked-in read input schema"),
        "outputSchema":schema_from_bundle(&bundle, "readResult").expect("checked-in read output schema")
    })
}

fn object<'a>(
    value: &'a Value,
    allowed: &[&str],
    required: &[&str],
    error: ContractError,
) -> Result<&'a Map<String, Value>> {
    let object = value.as_object().ok_or(error)?;
    if object.keys().any(|key| !allowed.contains(&key.as_str()))
        || required.iter().any(|key| !object.contains_key(*key))
    {
        return Err(error);
    }
    Ok(object)
}

fn text(value: &Value, min: usize, max: usize, error: ContractError) -> Result<&str> {
    let text = value.as_str().ok_or(error)?;
    if text.len() < min || text.len() > max {
        return Err(error);
    }
    Ok(text)
}

fn identifier(value: &Value, error: ContractError) -> Result<&str> {
    let value = text(value, 1, 256, error)?;
    if value.chars().any(char::is_control) {
        return Err(error);
    }
    Ok(value)
}

fn absolute_path(value: &Value, error: ContractError) -> Result<&str> {
    let value = text(value, 1, 2048, error)?;
    if !value.starts_with('/') || value.chars().any(char::is_control) {
        return Err(error);
    }
    Ok(value)
}

fn organization_path(value: &Value, error: ContractError) -> Result<&str> {
    let value = text(value, 1, 2048, error)?;
    if value.starts_with('/')
        || value
            .chars()
            .any(|c| c.is_control() || c == '\\' || c == ':')
        || value
            .split('/')
            .any(|p| p.is_empty() || p == "." || p == "..")
    {
        return Err(error);
    }
    Ok(value)
}

fn integer(value: &Value, min: u64, max: u64, error: ContractError) -> Result<u64> {
    let number = value.as_number().ok_or(error)?;
    check_number(number).map_err(|_| error)?;
    let number = number.as_f64().ok_or(error)?;
    if number.fract() != 0.0 || number < min as f64 || number > max as f64 {
        return Err(error);
    }
    Ok(number as u64)
}

pub fn validate_initialize(value: &Value) -> Result<()> {
    let error = ContractError::SchemaDrift;
    let root = object(
        value,
        &[
            "protocolVersion",
            "serverInfo",
            "capabilities",
            "instructions",
        ],
        &["protocolVersion", "serverInfo", "capabilities"],
        error,
    )?;
    if root["protocolVersion"] != "2025-06-18" {
        return Err(error);
    }
    let server = object(
        &root["serverInfo"],
        &["name", "version"],
        &["name", "version"],
        error,
    )?;
    if server["name"] != "xcode-tools" || server["version"] != "25317" {
        return Err(error);
    }
    let caps = object(&root["capabilities"], &["tools"], &["tools"], error)?;
    let tools = object(&caps["tools"], &["listChanged"], &[], error)?;
    if tools
        .get("listChanged")
        .is_some_and(|value| !value.is_boolean())
    {
        return Err(error);
    }
    if let Some(instructions) = root.get("instructions") {
        text(instructions, 0, 4096, error)?;
    }
    Ok(())
}

/// Extra tools are observed only for duplicate names; they never gain admission.
pub fn validate_upstream_tools(tools: &[Value]) -> Result<()> {
    let error = ContractError::SchemaDrift;
    if tools.len() > 256 {
        return Err(error);
    }
    let bundle = bundle()?;
    let expected = bundle["upstream_capture"]["tools"]
        .as_array()
        .ok_or(ContractError::InvalidBundle)?;
    let mut names = HashSet::new();
    let mut admitted = 0;
    for tool in tools {
        let name = identifier(tool.get("name").ok_or(error)?, error)?;
        if !names.insert(name) {
            return Err(error);
        }
        if let Some(pinned) = expected.iter().find(|pinned| pinned["name"] == name) {
            if tool != pinned {
                return Err(error);
            }
            admitted += 1;
        }
    }
    if admitted != expected.len() {
        return Err(error);
    }
    Ok(())
}

fn payload(value: &Value) -> Result<&Value> {
    let error = ContractError::InvalidEnvelope;
    let root = object(
        value,
        &["structuredContent", "content", "isError"],
        &[],
        error,
    )?;
    match root.get("isError") {
        None | Some(Value::Bool(false)) => {}
        Some(Value::Bool(true)) => return Err(ContractError::UpstreamError),
        _ => return Err(error),
    }
    let structured = root
        .get("structuredContent")
        .filter(|value| value.is_object())
        .ok_or(error)?;
    if let Some(content) = root.get("content") {
        let items = content
            .as_array()
            .filter(|items| items.len() == 1)
            .ok_or(error)?;
        let item = object(&items[0], &["type", "text"], &["type", "text"], error)?;
        if item["type"] != "text" {
            return Err(error);
        }
        let text = text(&item["text"], 0, MAX_JSON_BYTES, error)?;
        let decoded = parse_unique_json(text.as_bytes()).map_err(|_| error)?;
        if canonical_bytes(&decoded).map_err(|_| error)?
            != canonical_bytes(structured).map_err(|_| error)?
        {
            return Err(error);
        }
    }
    Ok(structured)
}

pub fn validate_open_arguments(value: &Value) -> Result<()> {
    let error = ContractError::InvalidArguments;
    let value = object(value, &["path"], &["path"], error)?;
    absolute_path(&value["path"], error)?;
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenResult {
    pub workspace_identifier: String,
    pub workspace_path: String,
}

pub fn decode_open(value: &Value) -> Result<OpenResult> {
    let error = ContractError::InvalidResult;
    let value = object(
        payload(value)?,
        &[
            "workspaceIdentifier",
            "workspacePath",
            "activeScheme",
            "activeRunDestination",
            "message",
        ],
        &["workspaceIdentifier", "workspacePath"],
        error,
    )?;
    let workspace_identifier = identifier(&value["workspaceIdentifier"], error)?.to_owned();
    let workspace_path = absolute_path(&value["workspacePath"], error)?.to_owned();
    for (key, max) in [
        ("activeScheme", 256),
        ("activeRunDestination", 2048),
        ("message", 4096),
    ] {
        if let Some(value) = value.get(key) {
            text(value, 0, max, error)?;
        }
    }
    Ok(OpenResult {
        workspace_identifier,
        workspace_path,
    })
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReadArguments {
    file_path: String,
    workspace_identifier: String,
    offset: u64,
    limit: u64,
}

impl ReadArguments {
    pub fn parse(value: &Value, alias: &str) -> Result<Self> {
        let error = ContractError::InvalidArguments;
        identifier(&Value::String(alias.to_owned()), error)?;
        let value = object(
            value,
            &["filePath", "workspaceIdentifier", "offset", "limit"],
            &["filePath"],
            error,
        )?;
        let file_path = organization_path(&value["filePath"], error)?.to_owned();
        if let Some(value) = value.get("workspaceIdentifier") {
            if identifier(value, error)? != alias {
                return Err(error);
            }
        }
        let offset = value
            .get("offset")
            .map(|v| integer(v, 1, MAX_SAFE_INTEGER, error))
            .transpose()?
            .unwrap_or(1);
        let limit = value
            .get("limit")
            .map(|v| integer(v, 1, 600, error))
            .transpose()?
            .unwrap_or(600);
        Ok(Self {
            file_path,
            workspace_identifier: alias.to_owned(),
            offset,
            limit,
        })
    }

    pub fn file_path(&self) -> &str {
        &self.file_path
    }
    pub fn workspace_identifier(&self) -> &str {
        &self.workspace_identifier
    }
    pub fn offset(&self) -> u64 {
        self.offset
    }
    pub fn limit(&self) -> u64 {
        self.limit
    }

    /// The controller supplies the ID from its current verified private binding.
    pub fn upstream_arguments(&self, upstream_id: &str) -> Value {
        json!({"filePath":self.file_path, "workspaceIdentifier":upstream_id,
            "offset":self.offset, "limit":self.limit})
    }
}

pub fn decode_read(value: &Value, arguments: &ReadArguments) -> Result<Value> {
    let error = ContractError::InvalidResult;
    let value = object(
        payload(value)?,
        &[
            "content",
            "filePath",
            "fileSize",
            "totalLines",
            "linesRead",
            "startLine",
            "message",
        ],
        &[
            "content",
            "filePath",
            "fileSize",
            "totalLines",
            "linesRead",
            "startLine",
        ],
        error,
    )?;
    let content = text(&value["content"], 0, MAX_CONTENT_BYTES, error)?;
    let path = organization_path(&value["filePath"], error)?;
    let file_size = integer(&value["fileSize"], 0, MAX_SAFE_INTEGER, error)?;
    let total_lines = integer(&value["totalLines"], 0, MAX_SAFE_INTEGER, error)?;
    let lines_read = integer(&value["linesRead"], 0, arguments.limit, error)?;
    let start_line = integer(&value["startLine"], 1, MAX_SAFE_INTEGER, error)?;
    if path != arguments.file_path
        || start_line != arguments.offset
        || lines_read > total_lines
        || (lines_read > 0
            && (start_line > total_lines || lines_read > total_lines - start_line + 1))
    {
        return Err(error);
    }
    if let Some(message) = value.get("message") {
        text(message, 0, 4096, error)?;
    }
    Ok(
        json!({"schema_version":1,"content":content,"filePath":path,"fileSize":file_size,
        "totalLines":total_lines,"linesRead":lines_read,"startLine":start_line}),
    )
}
