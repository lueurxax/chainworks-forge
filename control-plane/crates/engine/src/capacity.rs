use std::collections::BTreeMap;

use domain::provider::{InvokeAgentCapacityConfig, ProviderFamily};

const GLOBAL_CAP_ENV: &str = "CHAINWORKS_INVOKE_AGENT_GLOBAL_CAP";
const PER_RUN_CAP_ENV: &str = "CHAINWORKS_INVOKE_AGENT_PER_RUN_CAP";
const MAX_GLOBAL_CAP: usize = 100;
const MAX_PER_RUN_CAP: usize = 20;
const MAX_PROVIDER_CAP: usize = 50;

pub fn load_invoke_agent_capacity_config_from_env() -> InvokeAgentCapacityConfig {
    invoke_agent_capacity_config_from_lookup(|key| std::env::var(key).ok())
}

pub fn invoke_agent_capacity_config_from_lookup(
    lookup: impl Fn(&str) -> Option<String>,
) -> InvokeAgentCapacityConfig {
    let defaults = InvokeAgentCapacityConfig::default();
    let global_active_agent_executions = lookup_usize(&lookup, GLOBAL_CAP_ENV)
        .map(|value| clamp_cap(GLOBAL_CAP_ENV, value, MAX_GLOBAL_CAP))
        .unwrap_or(defaults.global_active_agent_executions);
    let per_run_active_agent_executions = lookup_usize(&lookup, PER_RUN_CAP_ENV)
        .map(|value| clamp_cap(PER_RUN_CAP_ENV, value, MAX_PER_RUN_CAP))
        .unwrap_or(defaults.per_run_active_agent_executions);

    let mut provider_caps = BTreeMap::new();
    for family in ProviderFamily::ALL {
        let env_key = provider_cap_env_key(family);
        let cap = lookup_usize(&lookup, &env_key)
            .map(|value| clamp_cap(&env_key, value, MAX_PROVIDER_CAP))
            .unwrap_or_else(|| defaults.provider_cap(family));
        provider_caps.insert(family, cap);
    }

    InvokeAgentCapacityConfig {
        global_active_agent_executions,
        per_run_active_agent_executions,
        provider_caps,
    }
}

fn lookup_usize(lookup: &impl Fn(&str) -> Option<String>, key: &str) -> Option<usize> {
    lookup(key).and_then(|value| value.parse::<usize>().ok())
}

fn clamp_cap(key: &str, value: usize, max: usize) -> usize {
    if value > max {
        tracing::warn!(
            env_var = key,
            requested = value,
            clamped_to = max,
            "InvokeAgent capacity override exceeds local safety cap"
        );
        max
    } else {
        value
    }
}

fn provider_cap_env_key(family: ProviderFamily) -> String {
    format!(
        "CHAINWORKS_INVOKE_AGENT_PROVIDER_CAP_{}",
        family.as_str().to_ascii_uppercase()
    )
}

#[cfg(test)]
mod tests {
    use super::invoke_agent_capacity_config_from_lookup;
    use domain::provider::ProviderFamily;

    #[test]
    fn proposal_061_capacity_config_loads_defaults_without_env() {
        let config = invoke_agent_capacity_config_from_lookup(|_| None);

        assert_eq!(config.global_active_agent_executions, 20);
        assert_eq!(config.per_run_active_agent_executions, 4);
        assert_eq!(config.provider_cap(ProviderFamily::Claude), 8);
        assert_eq!(config.provider_cap(ProviderFamily::Gemini), 4);
        assert_eq!(config.provider_cap(ProviderFamily::Codex), 10);
        assert_eq!(config.provider_cap(ProviderFamily::Auggie), 1);
        assert_eq!(config.provider_cap(ProviderFamily::Junie), 1);
    }

    #[test]
    fn proposal_061_capacity_config_accepts_environment_compatible_overrides() {
        let config = invoke_agent_capacity_config_from_lookup(|key| match key {
            "CHAINWORKS_INVOKE_AGENT_GLOBAL_CAP" => Some("12".into()),
            "CHAINWORKS_INVOKE_AGENT_PER_RUN_CAP" => Some("3".into()),
            "CHAINWORKS_INVOKE_AGENT_PROVIDER_CAP_GEMINI" => Some("2".into()),
            "CHAINWORKS_INVOKE_AGENT_PROVIDER_CAP_CODEX" => Some("5".into()),
            _ => None,
        });

        assert_eq!(config.global_active_agent_executions, 12);
        assert_eq!(config.per_run_active_agent_executions, 3);
        assert_eq!(config.provider_cap(ProviderFamily::Claude), 8);
        assert_eq!(config.provider_cap(ProviderFamily::Gemini), 2);
        assert_eq!(config.provider_cap(ProviderFamily::Codex), 5);
    }

    #[test]
    fn proposal_061_capacity_config_clamps_unreasonable_overrides() {
        let config = invoke_agent_capacity_config_from_lookup(|key| match key {
            "CHAINWORKS_INVOKE_AGENT_GLOBAL_CAP" => Some("999999".into()),
            "CHAINWORKS_INVOKE_AGENT_PER_RUN_CAP" => Some("999999".into()),
            "CHAINWORKS_INVOKE_AGENT_PROVIDER_CAP_CLAUDE" => Some("999999".into()),
            _ => None,
        });

        assert_eq!(config.global_active_agent_executions, 100);
        assert_eq!(config.per_run_active_agent_executions, 20);
        assert_eq!(config.provider_cap(ProviderFamily::Claude), 50);
    }
}
