use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};

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

pub fn load_xcode_broker_tool_allowlists() -> Result<BTreeMap<String, BTreeSet<String>>, String> {
    let registry = load_machine_registry()?;
    Ok(xcode_broker_tool_allowlists_from_registry(&registry))
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
    #[serde(default, alias = "envs")]
    env: BTreeMap<String, String>,
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    headers: BTreeMap<String, String>,
    #[serde(default)]
    provider: Option<String>,
    #[serde(default, rename = "type")]
    transport_type: Option<String>,
    #[serde(default)]
    transport: Option<String>,
    #[serde(default, alias = "xcodePidSelector", alias = "xcode_pid_selector")]
    xcode_pid_selector: Option<String>,
    #[serde(
        default,
        alias = "toolAllowlist",
        alias = "tool_allowlist",
        alias = "allowedTools",
        alias = "allowed_tools"
    )]
    tool_allowlist: Vec<String>,
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
                                resolved_tool_allowlist_hash: intent.resolved_tool_allowlist_hash,
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
        } else if let Some(url) = entry.url.as_deref().filter(|url| !url.is_empty()) {
            if matches!(entry.transport_type.as_deref(), Some("sse")) {
                denied_extensions.push(extension_id.clone());
                blocking_issues.push(format!(
                    "MCP extension '{extension_id}' declares SSE transport, which is not supported by ACP session/new."
                ));
                continue;
            }
            ResolvedMcpServerTransport::Http {
                url: url.to_string(),
                headers: entry.headers.clone(),
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
            if transport_type != "stdio" && transport_type != "platform" && transport_type != "http"
            {
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
    resolved_tool_allowlist_hash: Option<String>,
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
        let resolved_tool_allowlist_hash = resolved_tool_allowlist_hash(&entry);
        return Ok((
            runtime_id,
            server_id,
            XcodeIntentRegistryFields {
                xcode_pid_selector: entry.xcode_pid_selector,
                resolved_tool_allowlist_hash,
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
            let resolved_tool_allowlist_hash = resolved_tool_allowlist_hash(&entry);
            return Ok((
                runtime_id_for(&server_id, &entry, extension_id),
                server_id,
                XcodeIntentRegistryFields {
                    xcode_pid_selector: entry.xcode_pid_selector,
                    resolved_tool_allowlist_hash,
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

fn resolved_tool_allowlist_hash(entry: &RegistryMcpServer) -> Option<String> {
    resolved_tool_allowlist(entry).map(|(hash, _tools)| hash)
}

fn xcode_broker_tool_allowlists_from_registry(
    registry: &MachineMcpRegistry,
) -> BTreeMap<String, BTreeSet<String>> {
    registry
        .server_entries()
        .into_iter()
        .filter(|(server_id, entry)| is_canonical_xcode_broker_entry(server_id, entry))
        .filter_map(|(_, entry)| resolved_tool_allowlist(&entry))
        .collect()
}

fn resolved_tool_allowlist(entry: &RegistryMcpServer) -> Option<(String, BTreeSet<String>)> {
    let mut tools = entry
        .tool_allowlist
        .iter()
        .filter(|tool| !tool.trim().is_empty())
        .map(|tool| tool.trim().to_string())
        .collect::<Vec<_>>();
    if tools.is_empty() {
        return None;
    }
    tools.sort();
    tools.dedup();
    let raw = serde_json::to_vec(&serde_json::json!({
        "version": 1,
        "tools": tools,
    }))
    .expect("Xcode tool allowlist payload should serialize");
    Some((
        format!("{:x}", Sha256::digest(raw)),
        tools.into_iter().collect(),
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

const MCP_REGISTRY_YAML_MAX_BYTES: u64 = 256 * 1024;

fn read_bounded_mcp_registry_yaml(path: &Path) -> Result<String, String> {
    use std::io::Read;

    let file = std::fs::File::open(path)
        .map_err(|err| format!("Failed to open MCP registry '{}': {err}", path.display()))?;
    let mut reader = file.take(MCP_REGISTRY_YAML_MAX_BYTES + 1);
    let mut content = String::new();
    reader
        .read_to_string(&mut content)
        .map_err(|err| format!("Failed to read MCP registry '{}': {err}", path.display()))?;
    if content.len() as u64 > MCP_REGISTRY_YAML_MAX_BYTES {
        return Err(format!(
            "MCP registry '{}' exceeds maximum size of {} bytes",
            path.display(),
            MCP_REGISTRY_YAML_MAX_BYTES
        ));
    }
    Ok(content)
}

fn load_machine_registry() -> Result<MachineMcpRegistry, String> {
    let registry_path = registry_path().ok_or_else(|| {
        "No MCP registry path is available; set CHAINWORKS_CODEX_CONFIG_PATH or create ~/.config/mcp/config.yaml."
            .to_string()
    })?;
    let content = read_bounded_mcp_registry_yaml(&registry_path)?;
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
    fn resolves_http_registry_entries() {
        let registry: MachineMcpRegistry = serde_yaml::from_str(
            r#"
mcp:
  backstage:
    runtime_id: backstage
    type: http
    url: https://backstage.example.test/api/mcp
    headers:
      Authorization: Bearer secret
"#,
        )
        .unwrap();

        let resolution = resolve_mcp_servers_from_registry(
            &["backstage".into()],
            Some("profile-a"),
            "claude",
            &registry,
        );

        assert!(resolution.report.blocking_issues.is_empty());
        assert_eq!(
            resolution.report.predicted_effective_extensions,
            ["backstage"]
        );
        assert_eq!(
            resolution.report.predicted_effective_runtime_ids,
            ["backstage"]
        );
        assert_eq!(resolution.payloads.len(), 1);
        match &resolution.payloads[0].transport {
            ResolvedMcpServerTransport::Http { url, headers } => {
                assert_eq!(url, "https://backstage.example.test/api/mcp");
                assert_eq!(
                    headers.get("Authorization").map(String::as_str),
                    Some("Bearer secret")
                );
            }
            other => panic!("expected HTTP MCP transport, got {other:?}"),
        }
    }

    #[test]
    fn rejects_sse_registry_entries_before_session_new() {
        let registry: MachineMcpRegistry = serde_yaml::from_str(
            r#"
mcp:
  goland:
    runtime_id: goland
    type: sse
    url: http://localhost:64343/sse
"#,
        )
        .unwrap();

        let resolution = resolve_mcp_servers_from_registry(
            &["goland".into()],
            Some("profile-a"),
            "claude",
            &registry,
        );

        assert!(resolution.payloads.is_empty());
        assert_eq!(resolution.report.denied_extensions, ["goland"]);
        assert!(resolution.report.blocking_issues[0].contains("SSE transport"));
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
    fn xcode_broker_contract_hash_changes_when_tool_allowlist_content_changes() {
        let build_registry: MachineMcpRegistry = serde_yaml::from_str(
            r#"
mcp:
  xcode:
    type: xcode_broker
    runtime_id: xcode
    tool_allowlist: ["xcode.build"]
"#,
        )
        .unwrap();
        let test_registry: MachineMcpRegistry = serde_yaml::from_str(
            r#"
mcp:
  xcode:
    type: xcode_broker
    runtime_id: xcode
    tool_allowlist: ["xcode.test"]
"#,
        )
        .unwrap();

        let mut build_resolution = resolve_mcp_servers_from_registry(
            &["xcode".into()],
            Some("profile-a"),
            "codex",
            &build_registry,
        );
        let mut test_resolution = resolve_mcp_servers_from_registry(
            &["xcode".into()],
            Some("profile-a"),
            "codex",
            &test_registry,
        );
        attach_xcode_broker_execution_context(
            &mut build_resolution.payloads,
            "/workspace/project",
            Some("workspace_write"),
        );
        attach_xcode_broker_execution_context(
            &mut test_resolution.payloads,
            "/workspace/project",
            Some("workspace_write"),
        );

        let build_hash = xcode_broker_contract_hash(&build_resolution.payloads).unwrap();
        let test_hash = xcode_broker_contract_hash(&test_resolution.payloads).unwrap();

        let build_allowlist_hash = match &build_resolution.payloads[0].transport {
            ResolvedMcpServerTransport::XcodeBrokerIntent { intent } => {
                intent.resolved_tool_allowlist_hash.as_deref()
            }
            other => panic!("expected Xcode broker intent, got {other:?}"),
        };
        let test_allowlist_hash = match &test_resolution.payloads[0].transport {
            ResolvedMcpServerTransport::XcodeBrokerIntent { intent } => {
                intent.resolved_tool_allowlist_hash.as_deref()
            }
            other => panic!("expected Xcode broker intent, got {other:?}"),
        };

        assert_ne!(build_allowlist_hash, test_allowlist_hash);
        assert_ne!(build_hash, test_hash);
    }

    #[test]
    fn xcode_broker_tool_allowlists_from_registry_resolves_hash_table() {
        let registry: MachineMcpRegistry = serde_yaml::from_str(
            r#"
mcp:
  xcode:
    type: xcode_broker
    runtime_id: xcode
    tool_allowlist: ["xcode.test", "xcode.build", "xcode.build"]
"#,
        )
        .unwrap();

        let allowlists = xcode_broker_tool_allowlists_from_registry(&registry);
        let tools = allowlists
            .values()
            .next()
            .expect("fixture registry should produce one allowlist");

        assert_eq!(allowlists.len(), 1);
        assert!(tools.contains("xcode.build"));
        assert!(tools.contains("xcode.test"));
        assert_eq!(tools.len(), 2);
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
