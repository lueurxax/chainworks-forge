use serde::{Deserialize, Serialize};
use std::path::Path;

// ── Principal types ─────────────────────────────────────────────────────

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Principal {
    pub id: String,
    pub class: PrincipalClass,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum PrincipalClass {
    Operator,
    Agent,
    Observer,
}

impl std::fmt::Display for PrincipalClass {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PrincipalClass::Operator => write!(f, "operator"),
            PrincipalClass::Agent => write!(f, "agent"),
            PrincipalClass::Observer => write!(f, "observer"),
        }
    }
}

// ── Auth errors ─────────────────────────────────────────────────────────

#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    #[error("missing credential")]
    MissingCredential,
    #[error("unknown token")]
    UnknownToken,
    #[error("malformed authorization header")]
    MalformedHeader,
    #[error("principal table load failed: {0}")]
    TableLoadFailed(String),
}

// ── Principal table ─────────────────────────────────────────────────────

#[derive(Clone, Debug, Serialize, Deserialize)]
struct PrincipalEntry {
    token: String,
    id: String,
    class: PrincipalClass,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct PrincipalTableFile {
    principals: Vec<PrincipalEntry>,
}

#[derive(Clone, Debug)]
pub struct PrincipalTable {
    entries: Vec<PrincipalEntry>,
}

impl PrincipalTable {
    /// Test/fixture stand-in: single operator principal with a known token.
    /// Plain pub fn (not cfg(test)) because integration tests in other crates
    /// need to construct a table without touching the filesystem.
    pub fn test_fixture() -> Self {
        PrincipalTable {
            entries: vec![PrincipalEntry {
                token: "test-token".into(),
                id: "test-operator".into(),
                class: PrincipalClass::Operator,
            }],
        }
    }

    /// Load from a JSON file. If the file does not exist, bootstrap a default
    /// operator-class principal, write it to disk, and return the table.
    pub fn load_or_bootstrap(path: &Path) -> Result<Self, AuthError> {
        if path.exists() {
            let content = std::fs::read_to_string(path)
                .map_err(|e| AuthError::TableLoadFailed(format!("read {}: {e}", path.display())))?;
            let file: PrincipalTableFile = serde_json::from_str(&content).map_err(|e| {
                AuthError::TableLoadFailed(format!("parse {}: {e}", path.display()))
            })?;
            if file.principals.is_empty() {
                return Err(AuthError::TableLoadFailed(
                    "principal table contains zero entries".into(),
                ));
            }
            Ok(PrincipalTable {
                entries: file.principals,
            })
        } else {
            // Bootstrap a default operator token
            let token = uuid::Uuid::new_v4().to_string();
            let entry = PrincipalEntry {
                token: token.clone(),
                id: "default-operator".into(),
                class: PrincipalClass::Operator,
            };
            let file = PrincipalTableFile {
                principals: vec![entry.clone()],
            };
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).map_err(|e| {
                    AuthError::TableLoadFailed(format!("create dir {}: {e}", parent.display()))
                })?;
            }
            let json = serde_json::to_string_pretty(&file)
                .map_err(|e| AuthError::TableLoadFailed(format!("serialize: {e}")))?;
            std::fs::write(path, &json).map_err(|e| {
                AuthError::TableLoadFailed(format!("write {}: {e}", path.display()))
            })?;
            tracing::info!(
                path = %path.display(),
                token = %token,
                "Auto-bootstrapped default operator principal"
            );
            Ok(PrincipalTable {
                entries: vec![entry],
            })
        }
    }
}

// ── Token resolution ────────────────────────────────────────────────────

pub fn resolve_bearer(token: &str, table: &PrincipalTable) -> Result<Principal, AuthError> {
    table
        .entries
        .iter()
        .find(|e| e.token == token)
        .map(|e| Principal {
            id: e.id.clone(),
            class: e.class.clone(),
        })
        .ok_or(AuthError::UnknownToken)
}

/// Extract bearer token from an Authorization header value.
/// Expects format: "Bearer <token>"
pub fn extract_bearer_token(header_value: &str) -> Result<&str, AuthError> {
    let trimmed = header_value.trim();
    if let Some(token) = trimmed.strip_prefix("Bearer ") {
        let token = token.trim();
        if token.is_empty() {
            return Err(AuthError::MalformedHeader);
        }
        Ok(token)
    } else {
        Err(AuthError::MalformedHeader)
    }
}

// ── Capability filtering ────────────────────────────────────────────────

/// Tool spec for capability filtering. Just needs a name.
pub struct ToolSpec {
    pub name: String,
}

/// Filter tools based on principal class.
pub fn filter_tools(principal: &Principal, specs: &[ToolSpec]) -> Vec<String> {
    let allowed = allowed_tools_for_class(&principal.class);
    specs
        .iter()
        .filter(|s| allowed.contains(&s.name.as_str()))
        .map(|s| s.name.clone())
        .collect()
}

/// Check if a specific tool is allowed for a principal.
pub fn is_tool_allowed(principal: &Principal, tool_name: &str) -> bool {
    allowed_tools_for_class(&principal.class).contains(&tool_name)
}

fn allowed_tools_for_class(class: &PrincipalClass) -> &'static [&'static str] {
    match class {
        PrincipalClass::Operator => &[
            "ideas.create",
            "ideas.list",
            "runs.start",
            "runs.list",
            "runs.get",
            "runs.cancel",
            "approvals.list",
            "approvals.resolve",
            "stages.retry",
            "reports.get",
        ],
        PrincipalClass::Agent => &[
            "ideas.create",
            "ideas.list",
            "runs.start",
            "runs.list",
            "runs.get",
            "reports.get",
        ],
        PrincipalClass::Observer => &[
            "ideas.list",
            "runs.list",
            "runs.get",
            "approvals.list",
            "reports.get",
        ],
    }
}

// ── Resource capability filtering ───────────────────────────────────────

/// Resource URI templates that each class is allowed to access.
fn allowed_resource_templates(class: &PrincipalClass) -> &'static [&'static str] {
    match class {
        PrincipalClass::Operator => &[
            "run://",
            "idea://",
            "artifact://",
            "report://",
            "chainworks://runs",
            "chainworks://ideas",
            "chainworks://approvals/inbox",
            "chainworks://runs/{run_id}/stages",
            "chainworks://runs/{run_id}/artifacts",
        ],
        PrincipalClass::Agent => &[
            "run://",
            "idea://",
            "artifact://",
            "report://",
            "chainworks://runs",
            "chainworks://ideas",
        ],
        PrincipalClass::Observer => &[
            "run://",
            "idea://",
            "report://",
            "chainworks://runs",
            "chainworks://ideas",
            "chainworks://approvals/inbox",
        ],
    }
}

/// Check if a concrete resource URI matches the principal's allowed templates.
/// Template matching: a concrete URI like "chainworks://runs/abc/artifacts"
/// matches template "chainworks://runs/{run_id}/artifacts".
pub fn is_resource_allowed(principal: &Principal, uri: &str) -> bool {
    let templates = allowed_resource_templates(&principal.class);
    for template in templates {
        if uri_matches_template(uri, template) {
            return true;
        }
    }
    false
}

fn uri_matches_template(uri: &str, template: &str) -> bool {
    // Entity URIs: "run://abc-123" matches "run://"
    if template.ends_with("://") {
        return uri.starts_with(template);
    }
    // Exact match for simple collection URIs
    if uri == template {
        return true;
    }
    // Template with {param} placeholders — split and match segments
    let t_parts: Vec<&str> = template.split('/').collect();
    let u_parts: Vec<&str> = uri.split('/').collect();
    if t_parts.len() != u_parts.len() {
        return false;
    }
    t_parts
        .iter()
        .zip(u_parts.iter())
        .all(|(t, u)| t.starts_with('{') && t.ends_with('}') || t == u)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entity_uri_matching() {
        assert!(uri_matches_template("run://abc-123", "run://"));
        assert!(uri_matches_template("idea://xyz", "idea://"));
        assert!(!uri_matches_template("run://abc", "idea://"));
    }

    #[test]
    fn collection_uri_matching() {
        assert!(uri_matches_template(
            "chainworks://runs",
            "chainworks://runs"
        ));
        assert!(!uri_matches_template(
            "chainworks://runs",
            "chainworks://ideas"
        ));
    }

    #[test]
    fn template_param_matching() {
        assert!(uri_matches_template(
            "chainworks://runs/abc-123/artifacts",
            "chainworks://runs/{run_id}/artifacts"
        ));
        assert!(uri_matches_template(
            "chainworks://runs/abc-123/stages",
            "chainworks://runs/{run_id}/stages"
        ));
        assert!(!uri_matches_template(
            "chainworks://runs/abc-123/artifacts",
            "chainworks://runs/{run_id}/stages"
        ));
    }

    #[test]
    fn operator_has_all_tools() {
        let p = Principal {
            id: "op".into(),
            class: PrincipalClass::Operator,
        };
        assert!(is_tool_allowed(&p, "runs.start"));
        assert!(is_tool_allowed(&p, "approvals.resolve"));
        assert!(is_tool_allowed(&p, "stages.retry"));
    }

    #[test]
    fn agent_cannot_approve() {
        let p = Principal {
            id: "ag".into(),
            class: PrincipalClass::Agent,
        };
        assert!(is_tool_allowed(&p, "runs.start"));
        assert!(!is_tool_allowed(&p, "approvals.resolve"));
        assert!(!is_tool_allowed(&p, "stages.retry"));
        assert!(!is_tool_allowed(&p, "runs.cancel"));
    }

    #[test]
    fn observer_read_only() {
        let p = Principal {
            id: "ob".into(),
            class: PrincipalClass::Observer,
        };
        assert!(is_tool_allowed(&p, "runs.list"));
        assert!(is_tool_allowed(&p, "reports.get"));
        assert!(!is_tool_allowed(&p, "ideas.create"));
        assert!(!is_tool_allowed(&p, "runs.start"));
    }

    #[test]
    fn resolve_bearer_works() {
        let table = PrincipalTable {
            entries: vec![PrincipalEntry {
                token: "tok-123".into(),
                id: "test-op".into(),
                class: PrincipalClass::Operator,
            }],
        };
        let p = resolve_bearer("tok-123", &table).unwrap();
        assert_eq!(p.id, "test-op");
        assert_eq!(p.class, PrincipalClass::Operator);
        assert!(resolve_bearer("bad-token", &table).is_err());
    }

    #[test]
    fn extract_bearer_token_works() {
        assert_eq!(extract_bearer_token("Bearer abc123").unwrap(), "abc123");
        assert!(extract_bearer_token("Basic abc123").is_err());
        assert!(extract_bearer_token("Bearer ").is_err());
    }
}
