use std::path::Path;

use anyhow::{Context, Result};
use serde::Serialize;
use sha2::{Digest, Sha256};

pub fn canonical_json<T: Serialize>(value: &T) -> Result<String> {
    let value = serde_json::to_value(value).context("convert steward value to json")?;
    canonical_json_value(value)
}

pub fn canonical_json_value(value: serde_json::Value) -> Result<String> {
    let sorted = sort_json_value(value);
    serde_json::to_string_pretty(&sorted).context("serialize steward canonical json")
}

pub fn canonical_hash<T: Serialize>(value: &T) -> Result<String> {
    let json = canonical_json(value)?;
    let digest = Sha256::digest(json.as_bytes());
    Ok(format!("{digest:x}"))
}

pub fn write_canonical_json<T: Serialize>(path: &Path, value: &T) -> Result<String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("create steward artifact dir {}", parent.display()))?;
    }
    let json = canonical_json(value)?;
    std::fs::write(path, json.as_bytes())
        .with_context(|| format!("write steward artifact {}", path.display()))?;
    Ok(json)
}

fn sort_json_value(value: serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Array(items) => {
            serde_json::Value::Array(items.into_iter().map(sort_json_value).collect())
        }
        serde_json::Value::Object(map) => {
            let sorted = map
                .into_iter()
                .map(|(key, value)| (key, sort_json_value(value)))
                .collect();
            serde_json::Value::Object(sorted)
        }
        scalar => scalar,
    }
}
