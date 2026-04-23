use std::collections::BTreeMap;
use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderFamily {
    Claude,
    Gemini,
    Codex,
    Auggie,
    Junie,
}

impl ProviderFamily {
    pub const ALL: [ProviderFamily; 5] = [
        ProviderFamily::Claude,
        ProviderFamily::Gemini,
        ProviderFamily::Codex,
        ProviderFamily::Auggie,
        ProviderFamily::Junie,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            ProviderFamily::Claude => "claude",
            ProviderFamily::Gemini => "gemini",
            ProviderFamily::Codex => "codex",
            ProviderFamily::Auggie => "auggie",
            ProviderFamily::Junie => "junie",
        }
    }

    pub fn resolve(alias: impl AsRef<str>) -> Result<Self, UnknownProviderFamily> {
        let raw = alias.as_ref();
        match normalize_provider_alias(raw).as_str() {
            "claude" | "claude_acp" | "claude_agent" | "claude_agent_acp" => {
                Ok(ProviderFamily::Claude)
            }
            "gemini" | "gemini_acp" | "gemini_cli" | "gemini_cli_acp" => Ok(ProviderFamily::Gemini),
            "codex" | "codex_acp" | "codex_cli" | "codex_cli_acp" | "openai_codex" => {
                Ok(ProviderFamily::Codex)
            }
            "auggie" | "auggie_acp" => Ok(ProviderFamily::Auggie),
            "junie" | "junie_acp" => Ok(ProviderFamily::Junie),
            _ => Err(UnknownProviderFamily {
                alias: raw.to_string(),
            }),
        }
    }

    pub fn canonicalize_alias(alias: impl AsRef<str>) -> Result<String, UnknownProviderFamily> {
        Ok(Self::resolve(alias)?.as_str().to_string())
    }

    pub fn canonicalize_known_alias(alias: impl AsRef<str>) -> Option<String> {
        Self::resolve(alias)
            .ok()
            .map(|family| family.as_str().to_string())
    }
}

impl fmt::Display for ProviderFamily {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for ProviderFamily {
    type Err = UnknownProviderFamily;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::resolve(s)
    }
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
#[error("unknown provider family alias: {alias}")]
pub struct UnknownProviderFamily {
    alias: String,
}

impl UnknownProviderFamily {
    pub fn alias(&self) -> &str {
        &self.alias
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct InvokeAgentCapacityConfig {
    pub global_active_agent_executions: usize,
    pub per_run_active_agent_executions: usize,
    pub provider_caps: BTreeMap<ProviderFamily, usize>,
}

impl InvokeAgentCapacityConfig {
    pub fn default_provider_caps() -> BTreeMap<ProviderFamily, usize> {
        BTreeMap::from([
            (ProviderFamily::Claude, 8),
            (ProviderFamily::Gemini, 4),
            (ProviderFamily::Codex, 10),
            (ProviderFamily::Auggie, 1),
            (ProviderFamily::Junie, 1),
        ])
    }

    pub fn provider_cap(&self, family: ProviderFamily) -> usize {
        *self.provider_caps.get(&family).unwrap_or(&0)
    }
}

impl Default for InvokeAgentCapacityConfig {
    fn default() -> Self {
        Self {
            global_active_agent_executions: 20,
            per_run_active_agent_executions: 4,
            provider_caps: Self::default_provider_caps(),
        }
    }
}

fn normalize_provider_alias(alias: &str) -> String {
    alias.trim().to_ascii_lowercase().replace('-', "_")
}

#[cfg(test)]
mod tests {
    use super::{InvokeAgentCapacityConfig, ProviderFamily};

    #[test]
    fn provider_family_resolves_approved_aliases_to_canonical_values() {
        let cases = [
            ("claude", ProviderFamily::Claude),
            ("claude_acp", ProviderFamily::Claude),
            ("claude_agent", ProviderFamily::Claude),
            ("claude_agent_acp", ProviderFamily::Claude),
            ("gemini", ProviderFamily::Gemini),
            ("gemini_acp", ProviderFamily::Gemini),
            ("gemini_cli", ProviderFamily::Gemini),
            ("gemini_cli_acp", ProviderFamily::Gemini),
            ("codex", ProviderFamily::Codex),
            ("codex_acp", ProviderFamily::Codex),
            ("codex_cli", ProviderFamily::Codex),
            ("codex_cli_acp", ProviderFamily::Codex),
            ("openai_codex", ProviderFamily::Codex),
            ("auggie", ProviderFamily::Auggie),
            ("auggie_acp", ProviderFamily::Auggie),
            ("junie", ProviderFamily::Junie),
            ("junie_acp", ProviderFamily::Junie),
        ];

        for (alias, expected) in cases {
            let resolved = ProviderFamily::resolve(alias).unwrap();
            assert_eq!(resolved, expected);
            assert_eq!(
                ProviderFamily::canonicalize_alias(alias).unwrap(),
                expected.as_str()
            );
        }
    }

    #[test]
    fn provider_family_unknown_alias_fails_loudly() {
        let error = ProviderFamily::resolve("provider-that-would-bypass-caps").unwrap_err();

        assert_eq!(error.alias(), "provider-that-would-bypass-caps");
    }

    #[test]
    fn examples_agent_provider_strings_resolve_to_known_families() {
        let catalogs = [
            include_str!("../../../../examples/agents/agents.yaml"),
            include_str!("../../../../examples/agents/proposal-po-reviewer.yaml"),
        ];

        let mut checked = 0;
        for catalog in catalogs {
            for line in catalog.lines() {
                let Some((_, provider)) = line.trim().split_once("provider:") else {
                    continue;
                };
                let provider = provider.trim();
                if provider.is_empty() {
                    continue;
                }
                ProviderFamily::resolve(provider).unwrap_or_else(|error| {
                    panic!("examples/agents provider alias must resolve: {error}")
                });
                checked += 1;
            }
        }

        assert!(checked > 0, "expected examples/agents provider aliases");
    }

    #[test]
    fn invoke_agent_capacity_defaults_match_proposal_061() {
        let config = InvokeAgentCapacityConfig::default();

        assert_eq!(config.global_active_agent_executions, 20);
        assert_eq!(config.per_run_active_agent_executions, 4);
        assert_eq!(config.provider_cap(ProviderFamily::Claude), 8);
        assert_eq!(config.provider_cap(ProviderFamily::Gemini), 4);
        assert_eq!(config.provider_cap(ProviderFamily::Codex), 10);
        assert_eq!(config.provider_cap(ProviderFamily::Auggie), 1);
        assert_eq!(config.provider_cap(ProviderFamily::Junie), 1);
    }
}
