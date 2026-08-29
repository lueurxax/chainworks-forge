use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use super::json::canonical_hash;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StewardConfig {
    pub schema_version: Option<u32>,
    pub windows: StewardWindows,
    #[serde(default)]
    pub thresholds: BTreeMap<String, StewardThreshold>,
    #[serde(default)]
    pub context_strategy_profiles: BTreeMap<String, serde_json::Value>,
    pub triggers: StewardTriggers,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StewardWindows {
    pub observation_window_size: usize,
    pub baseline_window_size: usize,
    pub minimum_window_size: usize,
    pub maximum_window_age_days: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StewardThreshold {
    pub method: String,
    pub trigger: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StewardTriggers {
    pub post_run_hook: StewardPostRunHook,
    pub on_config_change: StewardToggle,
    pub schedule: StewardSchedule,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StewardPostRunHook {
    pub enabled: bool,
    pub run_interval: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StewardToggle {
    pub enabled: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StewardSchedule {
    pub enabled: bool,
    pub cron: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StewardConfigLoadStatus {
    LoadedAndValidated,
    LoadedWithDefaultFallback { validation_errors: Vec<String> },
}

#[derive(Clone, Debug)]
pub struct StewardRuntimeInputs {
    pub steward_config_path: PathBuf,
    pub steward_config: StewardConfig,
    pub steward_config_hash: String,
    pub steward_config_load_status: StewardConfigLoadStatus,
    pub agent_catalog_path: PathBuf,
    pub agent_catalog_json: serde_json::Value,
    pub agent_catalog_hash: String,
    pub previous_steward_config_hash: Option<String>,
    pub previous_agent_catalog_hash: Option<String>,
    pub config_change_analysis_scheduled: bool,
}

#[derive(Clone, Debug)]
pub struct EffectiveStewardConfig {
    pub config: StewardConfig,
    pub hash: String,
    pub load_status: StewardConfigLoadStatus,
    pub used_default: bool,
}

impl StewardConfig {
    pub fn default_config() -> Self {
        let thresholds = BTreeMap::from([
            (
                "timing".into(),
                StewardThreshold {
                    method: "median_percentage".into(),
                    trigger: 0.30,
                },
            ),
            (
                "rework".into(),
                StewardThreshold {
                    method: "mean_percentage".into(),
                    trigger: 0.50,
                },
            ),
            (
                "quality".into(),
                StewardThreshold {
                    method: "ratio".into(),
                    trigger: 2.0,
                },
            ),
            (
                "cost".into(),
                StewardThreshold {
                    method: "median_percentage".into(),
                    trigger: 0.25,
                },
            ),
            (
                "stability".into(),
                StewardThreshold {
                    method: "ratio".into(),
                    trigger: 2.0,
                },
            ),
        ]);
        Self {
            schema_version: Some(1),
            windows: StewardWindows {
                observation_window_size: 20,
                baseline_window_size: 20,
                minimum_window_size: 5,
                maximum_window_age_days: 90,
            },
            thresholds,
            context_strategy_profiles: BTreeMap::new(),
            triggers: StewardTriggers {
                post_run_hook: StewardPostRunHook {
                    enabled: true,
                    run_interval: 1,
                },
                on_config_change: StewardToggle { enabled: true },
                schedule: StewardSchedule {
                    enabled: false,
                    cron: "0 8 * * 1".into(),
                },
            },
        }
    }

    pub fn validate(&self) -> Result<()> {
        if self.schema_version != Some(1) {
            anyhow::bail!("schema_version must be 1");
        }
        if self.windows.minimum_window_size == 0 {
            anyhow::bail!("minimum_window_size must be > 0");
        }
        if self.windows.observation_window_size < self.windows.minimum_window_size {
            anyhow::bail!("observation_window_size must be >= minimum_window_size");
        }
        if self.windows.baseline_window_size < self.windows.minimum_window_size {
            anyhow::bail!("baseline_window_size must be >= minimum_window_size");
        }
        if self.triggers.post_run_hook.enabled && self.triggers.post_run_hook.run_interval == 0 {
            anyhow::bail!("post_run_hook.run_interval must be > 0 when enabled");
        }
        for required in ["timing", "rework", "quality", "cost", "stability"] {
            if !self.thresholds.contains_key(required) {
                anyhow::bail!("thresholds.{required} is required");
            }
        }
        for (name, threshold) in &self.thresholds {
            if !matches!(
                threshold.method.as_str(),
                "median_percentage" | "mean_percentage" | "ratio"
            ) {
                anyhow::bail!("thresholds.{name}.method is unsupported");
            }
            if !threshold.trigger.is_finite() || threshold.trigger <= 0.0 {
                anyhow::bail!("thresholds.{name}.trigger must be > 0");
            }
        }
        Ok(())
    }
}

pub fn default_config_path() -> PathBuf {
    first_existing_path(&[
        "examples/steward/steward_config.yaml",
        "../examples/steward/steward_config.yaml",
        "../../examples/steward/steward_config.yaml",
        "../../../examples/steward/steward_config.yaml",
    ])
}

pub fn default_agent_catalog_path() -> PathBuf {
    first_existing_path(&[
        "examples/agents/agents.yaml",
        "../examples/agents/agents.yaml",
        "../../examples/agents/agents.yaml",
        "../../../examples/agents/agents.yaml",
    ])
}

pub fn load_effective_config(path: Option<&Path>) -> EffectiveStewardConfig {
    let default_path;
    let path = match path {
        Some(path) => path,
        None => {
            default_path = default_config_path();
            default_path.as_path()
        }
    };
    let loaded = std::fs::read_to_string(path)
        .with_context(|| format!("read steward config {}", path.display()))
        .and_then(|content| {
            serde_yaml::from_str::<StewardConfig>(&content)
                .with_context(|| format!("parse steward config {}", path.display()))
        })
        .and_then(|config| {
            config
                .validate()
                .with_context(|| format!("validate steward config {}", path.display()))?;
            Ok(config)
        });

    let (config, load_status, used_default) = match loaded {
        Ok(config) => (config, StewardConfigLoadStatus::LoadedAndValidated, false),
        Err(err) => (
            StewardConfig::default_config(),
            StewardConfigLoadStatus::LoadedWithDefaultFallback {
                validation_errors: vec![err.to_string()],
            },
            true,
        ),
    };
    let hash = canonical_hash(&config).expect("default steward config must serialize");
    EffectiveStewardConfig {
        config,
        hash,
        load_status,
        used_default,
    }
}

pub fn load_agent_catalog_json(path: &Path) -> Result<(serde_json::Value, String)> {
    let catalog = workflow::catalog::load(path.to_string_lossy().as_ref())
        .with_context(|| format!("load agent catalog {}", path.display()))?;
    let value = serde_json::to_value(&catalog).context("convert agent catalog to json")?;
    let hash = canonical_hash(&value)?;
    Ok((value, hash))
}

pub fn synthetic_runtime_inputs(
    steward_config: StewardConfig,
    agent_catalog_json: serde_json::Value,
) -> Result<StewardRuntimeInputs> {
    let steward_config_hash = canonical_hash(&steward_config)?;
    let agent_catalog_hash = canonical_hash(&agent_catalog_json)?;
    Ok(StewardRuntimeInputs {
        steward_config_path: PathBuf::from("<synthetic>"),
        steward_config,
        steward_config_hash,
        steward_config_load_status: StewardConfigLoadStatus::LoadedAndValidated,
        agent_catalog_path: PathBuf::from("<synthetic>"),
        agent_catalog_json,
        agent_catalog_hash,
        previous_steward_config_hash: None,
        previous_agent_catalog_hash: None,
        config_change_analysis_scheduled: false,
    })
}

fn first_existing_path(candidates: &[&str]) -> PathBuf {
    candidates
        .iter()
        .map(PathBuf::from)
        .find(|path| path.exists())
        .unwrap_or_else(|| PathBuf::from(candidates[0]))
}

#[cfg(test)]
mod tests {
    use super::StewardConfig;

    #[test]
    fn canonical_config_enables_every_run_analysis() {
        let config: StewardConfig = serde_yaml::from_str(include_str!(
            "../../../../../examples/steward/steward_config.yaml"
        ))
        .expect("canonical Steward config must decode");

        assert!(config.triggers.post_run_hook.enabled);
        assert_eq!(config.triggers.post_run_hook.run_interval, 1);
    }
}
