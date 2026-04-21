//! P051 direct-command declaration scanner.
//!
//! The scanner is intentionally conservative in the scaffold phase: it detects
//! declared Xcode command surfaces and fails only on direct bypass forms that
//! cannot be routed through the later PATH shim boundary.

use crate::catalog;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};

#[derive(Debug, Clone, Default)]
pub struct DirectCommandScan {
    agent_signals: HashMap<String, AgentXcodeSignals>,
    permission_profile_signals: HashMap<String, AgentXcodeSignals>,
    pub declarations: Vec<DirectCommandDeclaration>,
    pub errors: Vec<String>,
}

impl DirectCommandScan {
    pub fn ensure_no_errors(&self) -> anyhow::Result<()> {
        if self.errors.is_empty() {
            return Ok(());
        }
        anyhow::bail!(
            "P051 direct-command catalog lint failed:\n{}",
            self.errors.join("\n")
        )
    }

    pub fn signals_for_agent(
        &self,
        agent_id: &str,
        permission_profile: Option<&str>,
    ) -> AgentXcodeSignals {
        let mut signals = AgentXcodeSignals::default();
        if let Some(profile) =
            permission_profile.and_then(|id| self.permission_profile_signals.get(id))
        {
            signals.merge(profile);
        }
        if let Some(agent) = self.agent_signals.get(agent_id) {
            signals.merge(agent);
        }
        signals
    }
}

#[derive(Debug, Clone, Default)]
pub struct AgentXcodeSignals {
    pub xcode_shim_injection_signal: bool,
    pub requires_xcode_host_execution: bool,
    pub xcode_prompt_lint_warnings: Vec<String>,
}

impl AgentXcodeSignals {
    fn merge(&mut self, other: &AgentXcodeSignals) {
        self.xcode_shim_injection_signal |= other.xcode_shim_injection_signal;
        self.requires_xcode_host_execution |= other.requires_xcode_host_execution;
        self.xcode_prompt_lint_warnings
            .extend(other.xcode_prompt_lint_warnings.iter().cloned());
        self.xcode_prompt_lint_warnings.sort();
        self.xcode_prompt_lint_warnings.dedup();
    }

    fn record_command_declaration(&mut self) {
        self.xcode_shim_injection_signal = true;
        self.requires_xcode_host_execution = true;
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DirectCommandDeclaration {
    pub source_document: String,
    pub source_path: String,
    pub declaration_kind: String,
    pub tokenization_mode: String,
    pub raw_value: String,
    pub argv_tokens: Vec<String>,
    pub matched_xcode_tool: Option<String>,
    pub policy_decision: DirectCommandPolicyDecision,
    pub contributes_to_xcode_shim_injection_signal: bool,
    pub warning_or_error_code: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DirectCommandPolicyDecision {
    Allow,
    HardFail,
    SoftWarning,
}

pub fn scan_catalog(
    catalog: &catalog::AgentCatalogFile,
    workflow_raw: &serde_yaml::Value,
    catalog_raw: &serde_yaml::Value,
) -> DirectCommandScan {
    let mut scan = DirectCommandScan::default();
    scan_permission_profile_shell_allow(catalog, &mut scan);
    scan_agent_required_tools(catalog, &mut scan);
    scan_agent_prompt_text(catalog, &mut scan);
    scan_raw_command_like_values("workflow", workflow_raw, &mut scan);
    scan_raw_command_like_values("catalog", catalog_raw, &mut scan);
    scan
}

fn scan_permission_profile_shell_allow(
    catalog: &catalog::AgentCatalogFile,
    scan: &mut DirectCommandScan,
) {
    let Some(profiles) = catalog.permission_profiles.as_ref() else {
        return;
    };
    let Some(profile_map) = profiles.as_mapping() else {
        return;
    };

    for (profile_id, profile_value) in profile_map {
        let Some(profile_id) = profile_id.as_str() else {
            continue;
        };
        let Some(shell) = mapping_get(profile_value, "shell") else {
            continue;
        };
        let Some(allow) = mapping_get(shell, "allow").and_then(|v| v.as_sequence()) else {
            continue;
        };
        for (idx, command) in allow.iter().enumerate() {
            let Some(raw) = command.as_str() else {
                continue;
            };
            record_command(
                scan,
                None,
                Some(profile_id),
                "catalog",
                format!("permission_profiles.{profile_id}.shell.allow[{idx}]"),
                "shell_allowlist",
                raw,
            );
        }
    }
}

fn scan_agent_required_tools(catalog: &catalog::AgentCatalogFile, scan: &mut DirectCommandScan) {
    let Some(agents) = catalog.agents.as_ref() else {
        return;
    };
    for agent in agents {
        let Some(required_tools) = agent.required_tools.as_ref() else {
            continue;
        };
        for (idx, command) in required_tools.iter().enumerate() {
            record_command(
                scan,
                Some(&agent.id),
                None,
                "catalog",
                format!("agents.{}.required_tools[{idx}]", agent.id),
                "required_tool",
                command,
            );
        }
    }
}

fn scan_agent_prompt_text(catalog: &catalog::AgentCatalogFile, scan: &mut DirectCommandScan) {
    let Some(agents) = catalog.agents.as_ref() else {
        return;
    };
    for agent in agents {
        for (field, value) in [
            ("prompt", agent.prompt.as_deref()),
            ("notes", agent.notes.as_deref()),
            ("title", agent.title.as_deref()),
        ] {
            let Some(raw) = value else {
                continue;
            };
            if !mentions_xcode_tool_or_path(raw) {
                continue;
            }
            let warning = format!(
                "p051_prompt_mentions_xcode_command:{}:{}",
                field,
                compact_value(raw)
            );
            let signals = scan.agent_signals.entry(agent.id.clone()).or_default();
            signals.xcode_prompt_lint_warnings.push(warning.clone());
            signals.xcode_prompt_lint_warnings.sort();
            signals.xcode_prompt_lint_warnings.dedup();
            scan.declarations.push(DirectCommandDeclaration {
                source_document: "catalog".to_string(),
                source_path: format!("agents.{}.{}", agent.id, field),
                declaration_kind: "prompt_text".to_string(),
                tokenization_mode: "plain_text".to_string(),
                raw_value: raw.to_string(),
                argv_tokens: Vec::new(),
                matched_xcode_tool: matched_xcode_tool(raw),
                policy_decision: DirectCommandPolicyDecision::SoftWarning,
                contributes_to_xcode_shim_injection_signal: false,
                warning_or_error_code: Some("p051_prompt_mentions_xcode_command".to_string()),
            });
        }
    }
}

fn scan_raw_command_like_values(
    source_document: &str,
    raw: &serde_yaml::Value,
    scan: &mut DirectCommandScan,
) {
    let mut strings = BTreeMap::new();
    collect_yaml_strings(raw, source_document.to_string(), &mut strings);
    for (source_path, value) in strings {
        if !is_command_like_path(&source_path) || !mentions_xcode_tool_or_path(&value) {
            continue;
        }
        record_command(
            scan,
            None,
            None,
            source_document,
            source_path,
            "raw_yaml_command",
            &value,
        );
    }
}

fn record_command(
    scan: &mut DirectCommandScan,
    agent_id: Option<&str>,
    permission_profile_id: Option<&str>,
    source_document: &str,
    source_path: String,
    declaration_kind: &str,
    raw: &str,
) {
    let classification = classify_command(raw);
    let Some(matched_xcode_tool) = classification.matched_xcode_tool.clone() else {
        return;
    };

    if classification.contributes_to_xcode_shim_injection_signal {
        if let Some(agent_id) = agent_id {
            scan.agent_signals
                .entry(agent_id.to_string())
                .or_default()
                .record_command_declaration();
        }
        if let Some(profile_id) = permission_profile_id {
            scan.permission_profile_signals
                .entry(profile_id.to_string())
                .or_default()
                .record_command_declaration();
        }
    }

    if let Some(error_code) = classification.error_code.as_ref() {
        scan.errors.push(format!(
            "{source_document}:{source_path}: {error_code}: {}",
            compact_value(raw)
        ));
    }

    scan.declarations.push(DirectCommandDeclaration {
        source_document: source_document.to_string(),
        source_path,
        declaration_kind: declaration_kind.to_string(),
        tokenization_mode: "shell_words_scaffold".to_string(),
        raw_value: raw.to_string(),
        argv_tokens: classification.argv_tokens,
        matched_xcode_tool: Some(matched_xcode_tool),
        policy_decision: if classification.error_code.is_some() {
            DirectCommandPolicyDecision::HardFail
        } else {
            DirectCommandPolicyDecision::Allow
        },
        contributes_to_xcode_shim_injection_signal: classification
            .contributes_to_xcode_shim_injection_signal,
        warning_or_error_code: classification.error_code,
    });
}

#[derive(Debug, Clone)]
struct CommandClassification {
    argv_tokens: Vec<String>,
    matched_xcode_tool: Option<String>,
    contributes_to_xcode_shim_injection_signal: bool,
    error_code: Option<String>,
}

fn classify_command(raw: &str) -> CommandClassification {
    let argv_tokens = tokenize_shell_words(raw);
    let matched_xcode_tool =
        matched_xcode_tool_from_tokens(&argv_tokens).or_else(|| matched_xcode_tool(raw));
    let contributes_to_xcode_shim_injection_signal = matches!(
        matched_xcode_tool.as_deref(),
        Some("xcodebuild" | "simctl" | "xcrun")
    );
    let error_code = hard_fail_code(raw, &argv_tokens);

    CommandClassification {
        argv_tokens,
        matched_xcode_tool,
        contributes_to_xcode_shim_injection_signal,
        error_code,
    }
}

fn hard_fail_code(raw: &str, argv_tokens: &[String]) -> Option<String> {
    if raw.contains("/usr/bin/xcrun") || raw.contains("/usr/bin/xcodebuild") {
        return Some("p051_absolute_xcode_tool_path".to_string());
    }
    if raw.contains("/Applications/Xcode") && raw.contains("/Contents/Developer/") {
        return Some("p051_absolute_xcode_developer_path".to_string());
    }
    if raw.contains("DEVELOPER_DIR=")
        && (contains_word(raw, "xcodebuild") || contains_word(raw, "simctl"))
    {
        return Some("p051_developer_dir_direct_xcode_command".to_string());
    }
    if argv_tokens
        .iter()
        .any(|token| basename(token).as_deref() == Some("mcpbridge"))
    {
        return Some("p051_direct_mcpbridge_command".to_string());
    }
    None
}

fn matched_xcode_tool(raw: &str) -> Option<String> {
    ["mcpbridge", "xcodebuild", "simctl", "xcrun"]
        .iter()
        .find(|tool| contains_word(raw, tool))
        .map(|tool| (*tool).to_string())
}

fn matched_xcode_tool_from_tokens(argv_tokens: &[String]) -> Option<String> {
    argv_tokens.iter().find_map(|token| {
        let base = basename(token)?;
        matches!(
            base.as_str(),
            "xcodebuild" | "simctl" | "mcpbridge" | "xcrun"
        )
        .then_some(base)
    })
}

fn mentions_xcode_tool_or_path(raw: &str) -> bool {
    matched_xcode_tool(raw).is_some()
        || raw.contains("/Applications/Xcode")
        || raw.contains("/Contents/Developer/")
        || raw.contains("DEVELOPER_DIR=")
}

fn is_command_like_path(source_path: &str) -> bool {
    let lower = source_path.to_ascii_lowercase();
    lower.contains(".command")
        || lower.contains(".commands")
        || lower.contains(".cmd")
        || lower.contains(".argv")
        || lower.contains(".args")
        || lower.contains(".shell.")
        || lower.contains(".required_tools")
}

fn collect_yaml_strings(
    value: &serde_yaml::Value,
    source_path: String,
    strings: &mut BTreeMap<String, String>,
) {
    match value {
        serde_yaml::Value::String(s) => {
            strings.insert(source_path, s.clone());
        }
        serde_yaml::Value::Sequence(items) => {
            for (idx, item) in items.iter().enumerate() {
                collect_yaml_strings(item, format!("{source_path}[{idx}]"), strings);
            }
        }
        serde_yaml::Value::Mapping(map) => {
            for (key, value) in map {
                let key = key
                    .as_str()
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| format!("{key:?}"));
                collect_yaml_strings(value, format!("{source_path}.{key}"), strings);
            }
        }
        serde_yaml::Value::Tagged(tagged) => {
            collect_yaml_strings(&tagged.value, source_path, strings);
        }
        serde_yaml::Value::Null | serde_yaml::Value::Bool(_) | serde_yaml::Value::Number(_) => {}
    }
}

fn mapping_get<'a>(value: &'a serde_yaml::Value, key: &str) -> Option<&'a serde_yaml::Value> {
    value
        .as_mapping()?
        .get(serde_yaml::Value::String(key.to_string()))
}

fn tokenize_shell_words(raw: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;
    let mut escaped = false;

    for ch in raw.chars() {
        if escaped {
            current.push(ch);
            escaped = false;
            continue;
        }
        if ch == '\\' {
            escaped = true;
            continue;
        }
        match quote {
            Some(active) if ch == active => quote = None,
            Some(_) => current.push(ch),
            None if ch == '\'' || ch == '"' => quote = Some(ch),
            None if ch.is_whitespace() => {
                if !current.is_empty() {
                    tokens.push(std::mem::take(&mut current));
                }
            }
            None => current.push(ch),
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }

    tokens
}

fn basename(token: &str) -> Option<String> {
    let token = token.trim_matches('"').trim_matches('\'');
    let token = token.split('=').next_back().unwrap_or(token);
    let basename = token.rsplit('/').next().unwrap_or(token);
    (!basename.is_empty()).then_some(basename.to_string())
}

fn contains_word(raw: &str, needle: &str) -> bool {
    raw.match_indices(needle).any(|(idx, _)| {
        let before = raw[..idx].chars().next_back();
        let after = raw[idx + needle.len()..].chars().next();
        !before.is_some_and(is_ident_char) && !after.is_some_and(is_ident_char)
    })
}

fn is_ident_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-')
}

fn compact_value(raw: &str) -> String {
    let compact = raw.split_whitespace().collect::<Vec<_>>().join(" ");
    const MAX_LEN: usize = 160;
    if compact.len() <= MAX_LEN {
        compact
    } else {
        format!("{}...", &compact[..MAX_LEN])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_path_based_xcodebuild_as_shimmed_command() {
        let classified = classify_command(
            r#"xcodebuild -project "Chainworks Forge.xcodeproj" -scheme "Chainworks Forge" build"#,
        );

        assert_eq!(classified.matched_xcode_tool.as_deref(), Some("xcodebuild"));
        assert!(classified.contributes_to_xcode_shim_injection_signal);
        assert!(classified.error_code.is_none());
        assert!(classified
            .argv_tokens
            .contains(&"Chainworks Forge.xcodeproj".to_string()));
    }

    #[test]
    fn rejects_absolute_xcrun_and_direct_mcpbridge() {
        let xcrun = classify_command("/usr/bin/xcrun simctl list");
        assert_eq!(
            xcrun.error_code.as_deref(),
            Some("p051_absolute_xcode_tool_path")
        );

        let mcpbridge = classify_command("xcrun mcpbridge");
        assert_eq!(
            mcpbridge.error_code.as_deref(),
            Some("p051_direct_mcpbridge_command")
        );
    }
}
