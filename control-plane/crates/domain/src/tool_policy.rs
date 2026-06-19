//! Runtime-owned tool shape policy for provider/ACP shell discovery tools.
//!
//! This module intentionally lives in `domain` so all runtime surfaces can share
//! the same generated-root denylist, policy/guard versions, and command
//! preflight classifier instead of copying prompt text.

pub const TOOL_POLICY_VERSION: &str = "bounded-tool-output-safe-search.v1";
pub const TOOL_GUARD_VERSION: &str = "p096-safe-search-guard.v1";

pub const DEFAULT_TOOL_OUTPUT_MAX_BYTES: u64 = 4 * 1024 * 1024;
pub const DEFAULT_TOOL_OUTPUT_MAX_LINES: u64 = 2_000;
pub const DEFAULT_CUMULATIVE_TOOL_OUTPUT_MAX_BYTES: u64 = 8 * 1024 * 1024;

pub const GENERATED_ROOT_DENYLIST: &[&str] = &[
    "control-plane/target/**",
    "**/target/**",
    "**/.build/**",
    "**/DerivedData/**",
    "**/node_modules/**",
    "**/.git/**",
    "**/.swiftpm/**",
    "**/.forge-codex-acp/**",
    "**/.junie/**",
    "**/.claude/**",
    "**/.codex/**",
    "**/*.xcresult/**",
    "**/*.dSYM/**",
    "**/build/**",
    "**/dist/**",
];

const SAFE_SEARCH_ERROR: &str = "Broad repository search must use bounded search and exclude generated/build roots. Use a narrower query, for example `rg prompt_stream_failed control-plane/crates/acp/src`, or include every generated-root exclude from runtime.health.toolOutputGuard.generatedRootDenylist and cap output. Excluded roots include control-plane/target/**, **/target/**, **/.build/**, DerivedData, node_modules.";

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ToolPreflightDecision {
    Allow,
    Deny(ToolPreflightDenial),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolPreflightDenial {
    pub code: &'static str,
    pub matched_tool: String,
    pub command: String,
    pub message: &'static str,
}

impl ToolPreflightDenial {
    pub fn agent_error_text(&self) -> String {
        format!("{}:\n{}", self.code, self.message)
    }
}

pub fn preflight_shell_command(command: &str) -> ToolPreflightDecision {
    for segment in command_segments(command) {
        let tokens = tokenize_shell_segment(segment);
        if tokens.is_empty() {
            continue;
        }
        for (idx, token) in tokens.iter().enumerate() {
            let executable = executable_name(token);
            if executable == "rg" {
                if rg_is_broad_without_generated_excludes(&tokens[idx + 1..]) {
                    return deny("rg", command);
                }
            } else if executable == "find" {
                if find_is_broad_without_generated_excludes(&tokens[idx + 1..]) {
                    return deny("find", command);
                }
            }
        }
    }
    ToolPreflightDecision::Allow
}

pub fn default_safe_search_guidance() -> String {
    format!(
        "Runtime safe-search policy ({TOOL_POLICY_VERSION}/{TOOL_GUARD_VERSION}): reviewer and auditor discovery must be bounded. Do not run broad `rg` or `find` over repo/worktree roots unless generated/build roots are excluded and output is capped. Default generated-root denylist: {}. Prefer narrow paths, `rg --glob '!target/**' --glob '!**/.build/**' --glob '!**/DerivedData/**' --glob '!**/node_modules/**' --glob '!**/.git/**'`, and commands that cap output lines.",
        GENERATED_ROOT_DENYLIST.join(", ")
    )
}

fn deny(tool: &str, command: &str) -> ToolPreflightDecision {
    ToolPreflightDecision::Deny(ToolPreflightDenial {
        code: "tool_output_budget_preflight_denied",
        matched_tool: tool.to_string(),
        command: command.trim().to_string(),
        message: SAFE_SEARCH_ERROR,
    })
}

fn command_segments(command: &str) -> Vec<&str> {
    command
        .split(&[';', '|'][..])
        .flat_map(|part| part.split("&&"))
        .flat_map(|part| part.split("||"))
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect()
}

fn executable_name(token: &str) -> String {
    token
        .rsplit('/')
        .next()
        .unwrap_or(token)
        .trim_matches(|c: char| c == '"' || c == '\'')
        .to_ascii_lowercase()
}

fn rg_is_broad_without_generated_excludes(tokens: &[String]) -> bool {
    let roots = rg_positional_roots(tokens);
    let broad = roots.is_empty()
        || roots.iter().any(|root| is_repo_or_worktree_root(root))
        || roots
            .iter()
            .filter(|root| is_top_level_repo_area(root))
            .count()
            >= 2;
    broad && !has_generated_root_excludes(tokens)
}

fn find_is_broad_without_generated_excludes(tokens: &[String]) -> bool {
    let roots = find_roots(tokens);
    let broad = roots.is_empty()
        || roots.iter().any(|root| is_repo_or_worktree_root(root))
        || roots.iter().any(|root| root == "control-plane");
    broad && !has_generated_root_excludes(tokens)
}

fn rg_positional_roots(tokens: &[String]) -> Vec<String> {
    let mut roots = Vec::new();
    let mut skip_next = false;
    let mut saw_pattern = tokens
        .iter()
        .any(|token| matches!(token.as_str(), "--files" | "--type-list"));
    for token in tokens {
        if skip_next {
            skip_next = false;
            continue;
        }
        if token == "--" {
            continue;
        }
        if option_takes_value(token) {
            skip_next = !token.contains('=');
            continue;
        }
        if token.starts_with('-') {
            continue;
        }
        if !saw_pattern {
            saw_pattern = true;
            continue;
        }
        roots.push(normalize_path_token(token));
    }
    roots
}

fn find_roots(tokens: &[String]) -> Vec<String> {
    let mut roots = Vec::new();
    for token in tokens {
        if token == "--" {
            continue;
        }
        if token.starts_with('-') || token == "(" || token == ")" || token == "!" {
            break;
        }
        roots.push(normalize_path_token(token));
    }
    roots
}

fn option_takes_value(token: &str) -> bool {
    matches!(
        token,
        "-g" | "--glob"
            | "--iglob"
            | "--type"
            | "-t"
            | "--type-not"
            | "-T"
            | "-e"
            | "--regexp"
            | "-f"
            | "--file"
            | "--sort"
            | "--max-count"
            | "-m"
            | "--max-filesize"
    ) || token.starts_with("--glob=")
        || token.starts_with("--iglob=")
        || token.starts_with("--type=")
        || token.starts_with("--type-not=")
        || token.starts_with("--regexp=")
        || token.starts_with("--file=")
        || token.starts_with("--max-count=")
        || token.starts_with("--max-filesize=")
}

fn normalize_path_token(token: &str) -> String {
    token
        .trim_matches(|c: char| c == '"' || c == '\'')
        .trim_end_matches('/')
        .strip_prefix("./")
        .unwrap_or_else(|| token.trim_end_matches('/'))
        .to_string()
}

fn is_repo_or_worktree_root(path: &str) -> bool {
    matches!(path, "" | "." | "./" | "$PWD" | "${PWD}")
}

fn is_top_level_repo_area(path: &str) -> bool {
    matches!(
        path,
        "control-plane"
            | "docs"
            | "scripts"
            | "examples"
            | "Chainworks Forge"
            | "Chainworks ForgeTests"
            | "Chainworks ForgeUITests"
    )
}

fn has_generated_root_excludes(tokens: &[String]) -> bool {
    let joined = tokens.join(" ").to_ascii_lowercase();
    let has_exclude_syntax = joined.contains("--glob")
        || joined.contains(" -g ")
        || joined.contains("-g!")
        || joined.contains("-g'!")
        || joined.contains("-g\"!")
        || joined.contains("-path")
        || joined.contains("-prune")
        || joined.contains("--exclude");
    has_exclude_syntax
        && [
            &["control-plane/target"][..],
            &["**/target"][..],
            &[".build"][..],
            &["deriveddata"][..],
            &["node_modules"][..],
            &[".git"][..],
            &[".swiftpm"][..],
            &[".forge-codex-acp"][..],
            &[".junie"][..],
            &[".claude"][..],
            &[".codex"][..],
            &[".xcresult"][..],
            &[".dsym"][..],
            &["**/build"][..],
            &["**/dist"][..],
        ]
        .iter()
        .all(|needles| needles.iter().any(|needle| joined.contains(needle)))
}

fn tokenize_shell_segment(segment: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;
    let mut escaped = false;

    for ch in segment.chars() {
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
            Some(q) if ch == q => quote = None,
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

#[cfg(test)]
mod tests {
    use super::*;

    fn denied(command: &str) -> bool {
        matches!(
            preflight_shell_command(command),
            ToolPreflightDecision::Deny(_)
        )
    }

    #[test]
    fn generated_root_denylist_contains_required_roots() {
        for required in [
            "control-plane/target/**",
            "**/target/**",
            "**/.build/**",
            "**/DerivedData/**",
            "**/node_modules/**",
            "**/.git/**",
            "**/.swiftpm/**",
            "**/.forge-codex-acp/**",
            "**/.junie/**",
            "**/.claude/**",
            "**/.codex/**",
            "**/*.xcresult/**",
            "**/*.dSYM/**",
            "**/build/**",
            "**/dist/**",
        ] {
            assert!(GENERATED_ROOT_DENYLIST.contains(&required));
        }
    }

    #[test]
    fn preflight_denies_broad_rg_without_generated_excludes() {
        assert!(denied("rg foo ."));
        assert!(denied("rg foo control-plane docs"));
        assert!(denied(
            "rg foo 'Chainworks Forge' control-plane docs scripts"
        ));
        assert!(denied("cd /workspace/repo && rg foo ."));
    }

    #[test]
    fn preflight_denies_broad_find_without_generated_excludes() {
        assert!(denied("find . -type f"));
        assert!(denied("find control-plane -type f"));
    }

    #[test]
    fn preflight_allows_narrow_or_excluded_searches() {
        assert!(!denied("rg foo control-plane/crates/acp/src"));
        assert!(!denied("rg --files control-plane/crates/acp/src"));
        assert!(denied("rg --glob '!control-plane/target/**' foo ."));
        assert!(denied(
            "find . -path './control-plane/target' -prune -o -type f -print"
        ));
        assert!(!denied(&format!(
            "rg {} foo .",
            GENERATED_ROOT_DENYLIST
                .iter()
                .map(|root| format!("--glob '!{root}'"))
                .collect::<Vec<_>>()
                .join(" ")
        )));
        assert!(!denied(&format!(
            "find . {} -type f -print",
            GENERATED_ROOT_DENYLIST
                .iter()
                .map(|root| format!("-path './{}' -prune -o", root.trim_end_matches("/**")))
                .collect::<Vec<_>>()
                .join(" ")
        )));
    }
}
