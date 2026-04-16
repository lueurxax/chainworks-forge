use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;

use acp::{AcpMcpServerPayload, ResolvedMcpServerTransport};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct McpResolutionReport {
    pub profile_id: String,
    pub requested_extensions: Vec<String>,
    pub predicted_effective_extensions: Vec<String>,
    pub predicted_effective_runtime_ids: Vec<String>,
    pub denied_extensions: Vec<String>,
    pub warnings: Vec<String>,
    pub blocking_issues: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct McpResolution {
    pub report: McpResolutionReport,
    pub payloads: Vec<AcpMcpServerPayload>,
}

#[derive(Debug, Default, Deserialize)]
struct MachineMcpRegistry {
    #[serde(default, alias = "mcpServers", alias = "mcp_servers")]
    mcp: HashMap<String, RegistryMcpServer>,
    #[serde(default)]
    servers: HashMap<String, RegistryMcpServer>,
}

#[derive(Clone, Debug, Default, Deserialize)]
struct RegistryMcpServer {
    #[serde(default, alias = "runtimeId", alias = "runtime_id")]
    runtime_id: Option<String>,
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    disabled: Option<bool>,
    #[serde(default)]
    enabled: Option<bool>,
    #[serde(default)]
    command: Option<String>,
    #[serde(default)]
    args: Vec<String>,
    #[serde(default)]
    env: BTreeMap<String, String>,
    #[serde(default)]
    provider: Option<String>,
    #[serde(default, rename = "type")]
    transport_type: Option<String>,
}

impl MachineMcpRegistry {
    fn into_servers(mut self) -> HashMap<String, RegistryMcpServer> {
        self.mcp.extend(self.servers);
        self.mcp
    }
}

pub fn resolve_mcp_servers(
    requested_extensions: &[String],
    backend_profile_id: Option<&str>,
    provider: &str,
) -> McpResolution {
    if requested_extensions.is_empty() {
        return McpResolution {
            report: empty_report(backend_profile_id, requested_extensions),
            payloads: Vec::new(),
        };
    }

    let mut warnings = Vec::new();
    let registry = match load_machine_registry() {
        Ok(registry) => registry,
        Err(issue) => {
            return McpResolution {
                report: McpResolutionReport {
                    profile_id: backend_profile_id.unwrap_or("unknown").to_string(),
                    requested_extensions: requested_extensions.to_vec(),
                    predicted_effective_extensions: Vec::new(),
                    predicted_effective_runtime_ids: Vec::new(),
                    denied_extensions: requested_extensions.to_vec(),
                    warnings,
                    blocking_issues: vec![issue],
                },
                payloads: Vec::new(),
            };
        }
    };

    let servers = registry.into_servers();
    let mut payloads = Vec::new();
    let mut predicted_effective_extensions = Vec::new();
    let mut predicted_effective_runtime_ids = Vec::new();
    let mut denied_extensions = Vec::new();
    let mut blocking_issues = Vec::new();

    for extension_id in requested_extensions {
        let Some(entry) = servers.get(extension_id) else {
            denied_extensions.push(extension_id.clone());
            blocking_issues.push(format!(
                "MCP extension '{extension_id}' is requested by backend profile '{}' but is missing from the machine-local registry.",
                backend_profile_id.unwrap_or("unknown")
            ));
            continue;
        };

        if entry.disabled.unwrap_or(false) || entry.enabled == Some(false) {
            denied_extensions.push(extension_id.clone());
            blocking_issues.push(format!(
                "MCP extension '{extension_id}' is disabled in the machine-local registry."
            ));
            continue;
        }

        let runtime_id = entry
            .runtime_id
            .as_deref()
            .or(entry.id.as_deref())
            .unwrap_or(extension_id)
            .to_string();

        let transport = if let Some(command) = entry
            .command
            .as_deref()
            .filter(|command| !command.is_empty())
        {
            ResolvedMcpServerTransport::Stdio {
                command: command.to_string(),
                args: entry.args.clone(),
                env: entry.env.clone(),
            }
        } else if let Some(platform_provider) = entry
            .provider
            .as_deref()
            .filter(|provider| !provider.is_empty())
        {
            if platform_provider != provider {
                denied_extensions.push(extension_id.clone());
                blocking_issues.push(format!(
                    "MCP extension '{extension_id}' is bound to provider '{platform_provider}' but agent execution uses provider '{provider}'."
                ));
                continue;
            }
            ResolvedMcpServerTransport::Platform {
                provider: platform_provider.to_string(),
            }
        } else {
            denied_extensions.push(extension_id.clone());
            blocking_issues.push(format!(
                "MCP extension '{extension_id}' has no supported executable transport in the machine-local registry."
            ));
            continue;
        };

        if let Some(transport_type) = entry.transport_type.as_deref() {
            if transport_type != "stdio" && transport_type != "platform" {
                warnings.push(format!(
                    "MCP extension '{extension_id}' declares transport type '{transport_type}', resolved from executable fields instead."
                ));
            }
        }

        predicted_effective_extensions.push(extension_id.clone());
        predicted_effective_runtime_ids.push(runtime_id.clone());
        payloads.push(AcpMcpServerPayload {
            id: runtime_id,
            extension_id: extension_id.clone(),
            transport,
        });
    }

    if !blocking_issues.is_empty() {
        payloads.clear();
    }

    McpResolution {
        report: McpResolutionReport {
            profile_id: backend_profile_id.unwrap_or("unknown").to_string(),
            requested_extensions: requested_extensions.to_vec(),
            predicted_effective_extensions,
            predicted_effective_runtime_ids,
            denied_extensions,
            warnings,
            blocking_issues,
        },
        payloads,
    }
}

fn empty_report(
    backend_profile_id: Option<&str>,
    requested_extensions: &[String],
) -> McpResolutionReport {
    McpResolutionReport {
        profile_id: backend_profile_id.unwrap_or("unknown").to_string(),
        requested_extensions: requested_extensions.to_vec(),
        predicted_effective_extensions: Vec::new(),
        predicted_effective_runtime_ids: Vec::new(),
        denied_extensions: Vec::new(),
        warnings: Vec::new(),
        blocking_issues: Vec::new(),
    }
}

fn load_machine_registry() -> Result<MachineMcpRegistry, String> {
    let registry_path = registry_path().ok_or_else(|| {
        "No MCP registry path is available; set CHAINWORKS_CODEX_CONFIG_PATH or create ~/.config/mcp/config.yaml."
            .to_string()
    })?;
    let content = std::fs::read_to_string(&registry_path).map_err(|err| {
        format!(
            "Failed to read MCP registry '{}': {err}",
            registry_path.display()
        )
    })?;
    serde_yaml::from_str(&content).map_err(|err| {
        format!(
            "Failed to parse MCP registry '{}': {err}",
            registry_path.display()
        )
    })
}

fn registry_path() -> Option<PathBuf> {
    if let Ok(path) = std::env::var("CHAINWORKS_CODEX_CONFIG_PATH") {
        let path = PathBuf::from(path);
        if path.is_file() {
            return Some(path);
        }
    }

    if let Some(home) = dirs::home_dir() {
        let canonical = home.join(".config/mcp/config.yaml");
        if canonical.is_file() {
            return Some(canonical);
        }
        let legacy = home.join(".config/goose/config.yaml");
        if legacy.is_file() {
            return Some(legacy);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_stdio_registry_entries() {
        let registry: MachineMcpRegistry = serde_yaml::from_str(
            r#"
mcp:
  filesystem:
    runtime_id: fs-runtime
    command: mcp-filesystem
    args: ["--root", "/tmp"]
    env:
      TOKEN: secret
"#,
        )
        .unwrap();

        let servers = registry.into_servers();
        let entry = servers.get("filesystem").unwrap();
        assert_eq!(entry.runtime_id.as_deref(), Some("fs-runtime"));
        assert_eq!(entry.command.as_deref(), Some("mcp-filesystem"));
    }
}
