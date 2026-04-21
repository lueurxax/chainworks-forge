use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;

use acp::{AcpMcpServerPayload, BrokeredXcodeMcpIntent, ResolvedMcpServerTransport};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

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

pub fn attach_xcode_broker_execution_context(
    payloads: &mut [AcpMcpServerPayload],
    workspace_root: &str,
    permission_profile_id: Option<&str>,
) {
    for payload in payloads {
        let ResolvedMcpServerTransport::XcodeBrokerIntent { intent } = &mut payload.transport
        else {
            continue;
        };
        if intent.workspace_root.is_none() {
            intent.workspace_root = Some(workspace_root.to_string());
        }
        if intent.permission_profile_id.is_none() {
            intent.permission_profile_id = permission_profile_id.map(ToOwned::to_owned);
        }
    }
}

pub fn xcode_broker_contract_hash(payloads: &[AcpMcpServerPayload]) -> Option<String> {
    let mut intents: Vec<BrokeredXcodeMcpIntent> = payloads
        .iter()
        .filter_map(|payload| match &payload.transport {
            ResolvedMcpServerTransport::XcodeBrokerIntent { intent } => Some(intent.clone()),
            _ => None,
        })
        .collect();

    if intents.is_empty() {
        return None;
    }

    intents.sort_by(|a, b| {
        (
            &a.extension_id,
            &a.runtime_id,
            &a.server_id,
            &a.workspace_root,
            &a.xcode_pid_selector,
            &a.runtime_profile_id,
            &a.permission_profile_id,
            &a.resolved_tool_allowlist_hash,
            a.provider_http_required,
        )
            .cmp(&(
                &b.extension_id,
                &b.runtime_id,
                &b.server_id,
                &b.workspace_root,
                &b.xcode_pid_selector,
                &b.runtime_profile_id,
                &b.permission_profile_id,
                &b.resolved_tool_allowlist_hash,
                b.provider_http_required,
            ))
    });

    let raw = serde_json::to_vec(&serde_json::json!({
        "version": 1,
        "xcode_broker_intents": intents,
    }))
    .expect("Xcode broker contract payload should serialize");
    Some(format!("{:x}", Sha256::digest(raw)))
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
    #[serde(default)]
    transport: Option<String>,
    #[serde(default, alias = "xcodePidSelector", alias = "xcode_pid_selector")]
    xcode_pid_selector: Option<String>,
}

impl MachineMcpRegistry {
    fn server_entries(&self) -> Vec<(String, RegistryMcpServer)> {
        self.mcp
            .iter()
            .chain(self.servers.iter())
            .map(|(server_id, entry)| (server_id.clone(), entry.clone()))
            .collect()
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

    let warnings = Vec::new();
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

    resolve_mcp_servers_from_registry(
        requested_extensions,
        backend_profile_id,
        provider,
        &registry,
    )
}

fn resolve_mcp_servers_from_registry(
    requested_extensions: &[String],
    backend_profile_id: Option<&str>,
    provider: &str,
    registry: &MachineMcpRegistry,
) -> McpResolution {
    let server_entries = registry.server_entries();
    let servers: HashMap<String, RegistryMcpServer> = server_entries.iter().cloned().collect();
    let mut payloads = Vec::new();
    let mut predicted_effective_extensions = Vec::new();
    let mut predicted_effective_runtime_ids = Vec::new();
    let mut denied_extensions = Vec::new();
    let mut blocking_issues = Vec::new();
    let mut warnings = Vec::new();

    for extension_id in requested_extensions {
        if extension_id == "xcode" {
            match resolve_xcode_broker_intent(extension_id, backend_profile_id, &server_entries) {
                Ok((runtime_id, server_id, intent, warning)) => {
                    if let Some(warning) = warning {
                        warnings.push(warning);
                    }
                    predicted_effective_extensions.push(extension_id.clone());
                    predicted_effective_runtime_ids.push(runtime_id.clone());
                    payloads.push(AcpMcpServerPayload {
                        id: runtime_id.clone(),
                        extension_id: extension_id.clone(),
                        transport: ResolvedMcpServerTransport::XcodeBrokerIntent {
                            intent: BrokeredXcodeMcpIntent {
                                extension_id: extension_id.clone(),
                                runtime_id,
                                server_id,
                                workspace_root: None,
                                xcode_pid_selector: intent.xcode_pid_selector,
                                runtime_profile_id: backend_profile_id.map(ToOwned::to_owned),
                                permission_profile_id: None,
                                resolved_tool_allowlist_hash: None,
                                provider_http_required: true,
                            },
                        },
                    });
                }
                Err(issue) => {
                    denied_extensions.push(extension_id.clone());
                    blocking_issues.push(issue);
                }
            }
            continue;
        }

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

struct XcodeIntentRegistryFields {
    xcode_pid_selector: Option<String>,
}

fn resolve_xcode_broker_intent(
    extension_id: &str,
    backend_profile_id: Option<&str>,
    entries: &[(String, RegistryMcpServer)],
) -> Result<(String, String, XcodeIntentRegistryFields, Option<String>), String> {
    let enabled_entries: Vec<(String, RegistryMcpServer)> = entries
        .iter()
        .filter(|(_, entry)| !entry.disabled.unwrap_or(false) && entry.enabled != Some(false))
        .cloned()
        .collect();

    let canonical_entries: Vec<(String, RegistryMcpServer)> = enabled_entries
        .iter()
        .filter(|(server_id, entry)| is_canonical_xcode_broker_entry(server_id, entry))
        .cloned()
        .collect();

    if canonical_entries.len() > 1 {
        return Err("xcode_mcp_registry_ambiguous: multiple enabled canonical Xcode MCP broker entries found; keep one canonical 'xcode' broker entry.".to_string());
    }

    if let Some((server_id, entry)) = canonical_entries.into_iter().next() {
        let runtime_id = runtime_id_for(&server_id, &entry, extension_id);
        return Ok((
            runtime_id,
            server_id,
            XcodeIntentRegistryFields {
                xcode_pid_selector: entry.xcode_pid_selector,
            },
            None,
        ));
    }

    if let Some((server_id, entry)) = enabled_entries
        .iter()
        .find(|(server_id, _)| server_id == extension_id)
        .cloned()
    {
        if is_xcrun_mcpbridge_entry(&entry) {
            return Ok((
                runtime_id_for(&server_id, &entry, extension_id),
                server_id,
                XcodeIntentRegistryFields {
                    xcode_pid_selector: entry.xcode_pid_selector,
                },
                Some("xcode_mcp_registry_stdio_migrated".to_string()),
            ));
        }
    }

    if enabled_entries
        .iter()
        .any(|(server_id, entry)| server_id != extension_id && is_mcpbridge_stdio_entry(entry))
    {
        return Err("xcode_mcp_registry_stale_stdio: direct mcpbridge stdio entries are not valid for brokered Xcode MCP.".to_string());
    }

    if enabled_entries
        .iter()
        .any(|(_, entry)| is_mcpbridge_stdio_entry(entry))
    {
        return Err("xcode_mcp_registry_stale_stdio: direct mcpbridge stdio entries are not valid for brokered Xcode MCP.".to_string());
    }

    let profile = backend_profile_id.unwrap_or("unknown");
    Err(format!(
        "xcode_mcp_registry_missing_canonical_id: MCP extension '{extension_id}' is requested by backend profile '{profile}' but no canonical 'xcode' broker entry exists."
    ))
}

fn runtime_id_for(server_id: &str, entry: &RegistryMcpServer, extension_id: &str) -> String {
    entry
        .runtime_id
        .as_deref()
        .or(entry.id.as_deref())
        .unwrap_or_else(|| {
            if server_id == "xcode" {
                "xcode"
            } else {
                extension_id
            }
        })
        .to_string()
}

fn is_canonical_xcode_broker_entry(server_id: &str, entry: &RegistryMcpServer) -> bool {
    let has_canonical_id = server_id == "xcode"
        || entry.runtime_id.as_deref() == Some("xcode")
        || entry.id.as_deref() == Some("xcode");
    has_canonical_id
        && entry.command.as_deref().unwrap_or("").is_empty()
        && entry
            .transport_type
            .as_deref()
            .or(entry.transport.as_deref())
            == Some("xcode_broker")
}

fn is_mcpbridge_stdio_entry(entry: &RegistryMcpServer) -> bool {
    entry
        .command
        .as_deref()
        .map(|command| command_basename(command) == "mcpbridge" || is_xcrun_mcpbridge_entry(entry))
        .unwrap_or(false)
}

fn is_xcrun_mcpbridge_entry(entry: &RegistryMcpServer) -> bool {
    let Some(command) = entry.command.as_deref() else {
        return false;
    };
    command_basename(command) == "xcrun"
        && entry
            .args
            .iter()
            .find(|arg| !arg.starts_with('-'))
            .map(|arg| command_basename(arg) == "mcpbridge")
            .unwrap_or(false)
}

fn command_basename(command: &str) -> &str {
    command.rsplit('/').next().unwrap_or(command)
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

        let servers: HashMap<String, RegistryMcpServer> =
            registry.server_entries().into_iter().collect();
        let entry = servers.get("filesystem").unwrap();
        assert_eq!(entry.runtime_id.as_deref(), Some("fs-runtime"));
        assert_eq!(entry.command.as_deref(), Some("mcp-filesystem"));
    }

    #[test]
    fn resolves_canonical_xcode_broker_intent() {
        let registry: MachineMcpRegistry = serde_yaml::from_str(
            r#"
mcp:
  xcode:
    type: xcode_broker
    runtime_id: xcode
    xcode_pid_selector: workspace:Chainworks.xcworkspace
"#,
        )
        .unwrap();

        let resolution = resolve_mcp_servers_from_registry(
            &["xcode".into()],
            Some("profile-a"),
            "codex",
            &registry,
        );

        assert!(resolution.report.blocking_issues.is_empty());
        assert_eq!(resolution.payloads.len(), 1);
        match &resolution.payloads[0].transport {
            ResolvedMcpServerTransport::XcodeBrokerIntent { intent } => {
                assert_eq!(intent.server_id, "xcode");
                assert_eq!(intent.runtime_id, "xcode");
                assert_eq!(intent.runtime_profile_id.as_deref(), Some("profile-a"));
                assert_eq!(
                    intent.xcode_pid_selector.as_deref(),
                    Some("workspace:Chainworks.xcworkspace")
                );
                assert!(intent.provider_http_required);
            }
            other => panic!("expected Xcode broker intent, got {other:?}"),
        }
    }

    #[test]
    fn attaches_execution_context_and_hashes_xcode_broker_contract() {
        let registry: MachineMcpRegistry = serde_yaml::from_str(
            r#"
mcp:
  xcode:
    type: xcode_broker
    runtime_id: xcode
"#,
        )
        .unwrap();

        let mut resolution = resolve_mcp_servers_from_registry(
            &["xcode".into()],
            Some("profile-a"),
            "codex",
            &registry,
        );
        attach_xcode_broker_execution_context(
            &mut resolution.payloads,
            "/workspace/project",
            Some("workspace_write"),
        );
        let first_hash = xcode_broker_contract_hash(&resolution.payloads)
            .expect("xcode broker payload should produce a contract hash");

        let mut changed_resolution = resolution.clone();
        attach_xcode_broker_execution_context(
            &mut changed_resolution.payloads,
            "/workspace/other",
            Some("workspace_write"),
        );
        if let ResolvedMcpServerTransport::XcodeBrokerIntent { intent } =
            &mut changed_resolution.payloads[0].transport
        {
            intent.workspace_root = Some("/workspace/other".to_string());
        }
        let changed_hash = xcode_broker_contract_hash(&changed_resolution.payloads)
            .expect("xcode broker payload should produce a contract hash");

        match &resolution.payloads[0].transport {
            ResolvedMcpServerTransport::XcodeBrokerIntent { intent } => {
                assert_eq!(intent.workspace_root.as_deref(), Some("/workspace/project"));
                assert_eq!(
                    intent.permission_profile_id.as_deref(),
                    Some("workspace_write")
                );
            }
            other => panic!("expected Xcode broker intent, got {other:?}"),
        }
        assert_ne!(first_hash, changed_hash);
    }

    #[test]
    fn migrates_canonical_xcrun_mcpbridge_with_warning() {
        let registry: MachineMcpRegistry = serde_yaml::from_str(
            r#"
mcp:
  xcode:
    command: xcrun
    args: ["mcpbridge"]
"#,
        )
        .unwrap();

        let resolution = resolve_mcp_servers_from_registry(
            &["xcode".into()],
            Some("profile-a"),
            "codex",
            &registry,
        );

        assert!(resolution.report.blocking_issues.is_empty());
        assert_eq!(
            resolution.report.warnings,
            vec!["xcode_mcp_registry_stdio_migrated".to_string()]
        );
        assert!(matches!(
            resolution.payloads[0].transport,
            ResolvedMcpServerTransport::XcodeBrokerIntent { .. }
        ));
    }

    #[test]
    fn rejects_noncanonical_direct_mcpbridge_registry_entries() {
        let registry: MachineMcpRegistry = serde_yaml::from_str(
            r#"
mcp:
  xcode-direct:
    runtime_id: xcode
    command: /usr/bin/xcrun
    args: ["mcpbridge"]
"#,
        )
        .unwrap();

        let resolution = resolve_mcp_servers_from_registry(
            &["xcode".into()],
            Some("profile-a"),
            "codex",
            &registry,
        );

        assert!(resolution.payloads.is_empty());
        assert_eq!(
            resolution.report.denied_extensions,
            vec!["xcode".to_string()]
        );
        assert!(resolution.report.blocking_issues[0].contains("xcode_mcp_registry_stale_stdio"));
    }

    #[test]
    fn rejects_ambiguous_xcode_broker_entries() {
        let registry: MachineMcpRegistry = serde_yaml::from_str(
            r#"
mcp:
  xcode:
    type: xcode_broker
servers:
  xcode-alt:
    runtime_id: xcode
    transport: xcode_broker
"#,
        )
        .unwrap();

        let resolution = resolve_mcp_servers_from_registry(
            &["xcode".into()],
            Some("profile-a"),
            "codex",
            &registry,
        );

        assert!(resolution.payloads.is_empty());
        assert!(resolution.report.blocking_issues[0].contains("xcode_mcp_registry_ambiguous"));
    }

    #[test]
    fn rejects_xcode_request_without_canonical_broker_id() {
        let registry: MachineMcpRegistry = serde_yaml::from_str(
            r#"
mcp:
  filesystem:
    command: mcp-filesystem
"#,
        )
        .unwrap();

        let resolution = resolve_mcp_servers_from_registry(
            &["xcode".into()],
            Some("profile-a"),
            "codex",
            &registry,
        );

        assert!(resolution.payloads.is_empty());
        assert!(resolution.report.blocking_issues[0]
            .contains("xcode_mcp_registry_missing_canonical_id"));
    }
}
