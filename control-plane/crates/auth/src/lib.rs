use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::path::Path;

// P029: PrincipalClass is canonically defined in domain::commands.
// Re-export here so downstream crates that use auth::PrincipalClass keep working.
pub use domain::{CapabilityToolId, PrincipalClass, ResourceTemplateId};

// ── Principal types ─────────────────────────────────────────────────────

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Principal {
    pub id: String,
    pub class: PrincipalClass,
    #[serde(default)]
    pub tool_capabilities: BTreeSet<CapabilityToolId>,
    #[serde(default)]
    pub resource_capabilities: BTreeSet<ResourceTemplateId>,
}

impl Principal {
    pub fn new(id: impl Into<String>, class: PrincipalClass) -> Self {
        let tool_capabilities = default_tool_capabilities(&class);
        let resource_capabilities = default_resource_capabilities(&class);
        Self {
            id: id.into(),
            class,
            tool_capabilities,
            resource_capabilities,
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
            #[cfg(unix)]
            {
                use std::io::Write;
                use std::os::unix::fs::OpenOptionsExt;
                let mut file = std::fs::OpenOptions::new()
                    .write(true)
                    .create(true)
                    .truncate(true)
                    .mode(0o600)
                    .open(path)
                    .map_err(|e| {
                        AuthError::TableLoadFailed(format!("create {}: {e}", path.display()))
                    })?;
                file.write_all(json.as_bytes()).map_err(|e| {
                    AuthError::TableLoadFailed(format!("write {}: {e}", path.display()))
                })?;
            }
            #[cfg(not(unix))]
            {
                std::fs::write(path, &json).map_err(|e| {
                    AuthError::TableLoadFailed(format!("write {}: {e}", path.display()))
                })?;
            }
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
        .map(|e| Principal::new(e.id.clone(), e.class.clone()))
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

pub fn filter_tools(principal: &Principal, ids: &[CapabilityToolId]) -> Vec<CapabilityToolId> {
    ids.iter()
        .copied()
        .filter(|id| principal.tool_capabilities.contains(id))
        .collect()
}

pub fn filter_resources(
    principal: &Principal,
    ids: &[ResourceTemplateId],
) -> Vec<ResourceTemplateId> {
    ids.iter()
        .copied()
        .filter(|id| principal.resource_capabilities.contains(id))
        .collect()
}

/// Check if a specific tool is allowed for a principal.
pub fn is_tool_allowed(principal: &Principal, tool_name: &str) -> bool {
    let Some(id) = capability_tool_id_for_name(tool_name) else {
        return false;
    };
    principal.tool_capabilities.contains(&id)
}

fn default_tool_capabilities(class: &PrincipalClass) -> BTreeSet<CapabilityToolId> {
    match class {
        PrincipalClass::Operator => [
            CapabilityToolId::IdeasCreate,
            CapabilityToolId::IdeasList,
            CapabilityToolId::RunsStart,
            CapabilityToolId::RunsList,
            CapabilityToolId::RunsGet,
            CapabilityToolId::RunsCancel,
            CapabilityToolId::ApprovalsList,
            CapabilityToolId::ApprovalsResolve,
            CapabilityToolId::StagesRetry,
            CapabilityToolId::ReportsGet,
            CapabilityToolId::StewardRunAnalysis,
            CapabilityToolId::StewardListAnalyses,
            CapabilityToolId::StewardGetAnalysis,
        ]
        .into_iter()
        .collect(),
        PrincipalClass::Agent => [
            CapabilityToolId::IdeasCreate,
            CapabilityToolId::IdeasList,
            CapabilityToolId::RunsStart,
            CapabilityToolId::RunsList,
            CapabilityToolId::RunsGet,
            CapabilityToolId::ReportsGet,
        ]
        .into_iter()
        .collect(),
        PrincipalClass::Observer => [
            CapabilityToolId::IdeasList,
            CapabilityToolId::RunsList,
            CapabilityToolId::RunsGet,
            CapabilityToolId::ApprovalsList,
            CapabilityToolId::ReportsGet,
            CapabilityToolId::StewardListAnalyses,
            CapabilityToolId::StewardGetAnalysis,
        ]
        .into_iter()
        .collect(),
    }
}

// ── Resource capability filtering ───────────────────────────────────────

fn default_resource_capabilities(class: &PrincipalClass) -> BTreeSet<ResourceTemplateId> {
    match class {
        PrincipalClass::Operator => all_resource_templates().into_iter().collect(),
        PrincipalClass::Agent => [
            ResourceTemplateId::RunEntity,
            ResourceTemplateId::IdeaEntity,
            ResourceTemplateId::ArtifactEntity,
            ResourceTemplateId::ReportEntity,
            ResourceTemplateId::ChainworksRuns,
            ResourceTemplateId::ChainworksIdeas,
        ]
        .into_iter()
        .collect(),
        PrincipalClass::Observer => all_resource_templates().into_iter().collect(),
    }
}

pub fn all_resource_templates() -> [ResourceTemplateId; 10] {
    [
        ResourceTemplateId::RunEntity,
        ResourceTemplateId::IdeaEntity,
        ResourceTemplateId::ArtifactEntity,
        ResourceTemplateId::ReportEntity,
        ResourceTemplateId::StewardAnalysisEntity,
        ResourceTemplateId::ChainworksRuns,
        ResourceTemplateId::ChainworksIdeas,
        ResourceTemplateId::ChainworksApprovalsInbox,
        ResourceTemplateId::ChainworksRunStages,
        ResourceTemplateId::ChainworksRunArtifacts,
    ]
}

pub fn is_resource_allowed(principal: &Principal, id: ResourceTemplateId) -> bool {
    principal.resource_capabilities.contains(&id)
}

pub fn match_resource_uri<F>(
    principal: &Principal,
    uri: &str,
    classify_uri: F,
) -> Option<ResourceTemplateId>
where
    F: FnOnce(&str) -> Option<ResourceTemplateId>,
{
    let id = classify_uri(uri)?;
    principal.resource_capabilities.contains(&id).then_some(id)
}

fn capability_tool_id_for_name(name: &str) -> Option<CapabilityToolId> {
    match name {
        "ideas.create" => Some(CapabilityToolId::IdeasCreate),
        "ideas.list" => Some(CapabilityToolId::IdeasList),
        "runs.start" => Some(CapabilityToolId::RunsStart),
        "runs.list" => Some(CapabilityToolId::RunsList),
        "runs.get" => Some(CapabilityToolId::RunsGet),
        "runs.cancel" => Some(CapabilityToolId::RunsCancel),
        "approvals.list" => Some(CapabilityToolId::ApprovalsList),
        "approvals.resolve" => Some(CapabilityToolId::ApprovalsResolve),
        "stages.retry" => Some(CapabilityToolId::StagesRetry),
        "reports.get" => Some(CapabilityToolId::ReportsGet),
        "steward.run_analysis" => Some(CapabilityToolId::StewardRunAnalysis),
        "steward.list_analyses" => Some(CapabilityToolId::StewardListAnalyses),
        "steward.get_analysis" => Some(CapabilityToolId::StewardGetAnalysis),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::{CapabilityToolId, ResourceTemplateId};

    #[test]
    fn principal_carries_typed_capability_sets() {
        let p = Principal::new("op", PrincipalClass::Operator);

        assert!(p.tool_capabilities.contains(&CapabilityToolId::RunsStart));
        assert!(p
            .tool_capabilities
            .contains(&CapabilityToolId::StewardRunAnalysis));
        assert!(p
            .resource_capabilities
            .contains(&ResourceTemplateId::StewardAnalysisEntity));
    }

    #[test]
    fn typed_filters_and_resource_match_share_principal_sets() {
        let observer = Principal::new("ob", PrincipalClass::Observer);
        let filtered = filter_tools(
            &observer,
            &[
                CapabilityToolId::RunsGet,
                CapabilityToolId::RunsStart,
                CapabilityToolId::StewardGetAnalysis,
            ],
        );

        assert_eq!(
            filtered,
            vec![
                CapabilityToolId::RunsGet,
                CapabilityToolId::StewardGetAnalysis
            ]
        );
        assert_eq!(
            match_resource_uri(
                &observer,
                "server-owned-artifacts-uri",
                test_resource_id_for_uri
            ),
            Some(ResourceTemplateId::ChainworksRunArtifacts)
        );
        assert_eq!(
            match_resource_uri(
                &Principal::new("ag", PrincipalClass::Agent),
                "server-owned-steward-uri",
                test_resource_id_for_uri
            ),
            None
        );
    }

    fn test_resource_id_for_uri(uri: &str) -> Option<ResourceTemplateId> {
        match uri {
            "server-owned-artifacts-uri" => Some(ResourceTemplateId::ChainworksRunArtifacts),
            "server-owned-steward-uri" => Some(ResourceTemplateId::StewardAnalysisEntity),
            _ => None,
        }
    }

    #[test]
    fn operator_has_all_tools() {
        let p = Principal::new("op", PrincipalClass::Operator);
        assert!(is_tool_allowed(&p, "runs.start"));
        assert!(is_tool_allowed(&p, "approvals.resolve"));
        assert!(is_tool_allowed(&p, "stages.retry"));
    }

    #[test]
    fn agent_cannot_approve() {
        let p = Principal::new("ag", PrincipalClass::Agent);
        assert!(is_tool_allowed(&p, "runs.start"));
        assert!(!is_tool_allowed(&p, "approvals.resolve"));
        assert!(!is_tool_allowed(&p, "stages.retry"));
        assert!(!is_tool_allowed(&p, "runs.cancel"));
    }

    #[test]
    fn observer_read_only() {
        let p = Principal::new("ob", PrincipalClass::Observer);
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

    #[test]
    fn operator_has_steward_tools() {
        let p = Principal::new("op", PrincipalClass::Operator);
        assert!(is_tool_allowed(&p, "steward.run_analysis"));
        assert!(is_tool_allowed(&p, "steward.list_analyses"));
        assert!(is_tool_allowed(&p, "steward.get_analysis"));
    }

    #[test]
    fn observer_has_read_only_steward_tools() {
        let p = Principal::new("ob", PrincipalClass::Observer);
        assert!(!is_tool_allowed(&p, "steward.run_analysis"));
        assert!(is_tool_allowed(&p, "steward.list_analyses"));
        assert!(is_tool_allowed(&p, "steward.get_analysis"));
    }

    #[test]
    fn steward_analysis_resource_policy() {
        let op = Principal::new("op", PrincipalClass::Operator);
        let ag = Principal::new("ag", PrincipalClass::Agent);
        let ob = Principal::new("ob", PrincipalClass::Observer);
        assert!(is_resource_allowed(
            &op,
            ResourceTemplateId::StewardAnalysisEntity
        ));
        assert!(!is_resource_allowed(
            &ag,
            ResourceTemplateId::StewardAnalysisEntity
        ));
        assert!(is_resource_allowed(
            &ob,
            ResourceTemplateId::StewardAnalysisEntity
        ));
    }

    #[test]
    fn agent_has_no_steward_tools() {
        let ag = Principal::new("ag", PrincipalClass::Agent);
        assert!(!is_tool_allowed(&ag, "steward.run_analysis"));
        assert!(!is_tool_allowed(&ag, "steward.list_analyses"));
        assert!(!is_tool_allowed(&ag, "steward.get_analysis"));
    }

    #[test]
    fn observer_has_all_read_resources() {
        let ob = Principal::new("ob", PrincipalClass::Observer);
        assert!(is_resource_allowed(&ob, ResourceTemplateId::RunEntity));
        assert!(is_resource_allowed(&ob, ResourceTemplateId::IdeaEntity));
        assert!(is_resource_allowed(&ob, ResourceTemplateId::ArtifactEntity));
        assert!(is_resource_allowed(&ob, ResourceTemplateId::ReportEntity));
        assert!(is_resource_allowed(
            &ob,
            ResourceTemplateId::StewardAnalysisEntity
        ));
        assert!(is_resource_allowed(&ob, ResourceTemplateId::ChainworksRuns));
        assert!(is_resource_allowed(
            &ob,
            ResourceTemplateId::ChainworksIdeas
        ));
        assert!(is_resource_allowed(
            &ob,
            ResourceTemplateId::ChainworksApprovalsInbox
        ));
        assert!(is_resource_allowed(
            &ob,
            ResourceTemplateId::ChainworksRunStages
        ));
        assert!(is_resource_allowed(
            &ob,
            ResourceTemplateId::ChainworksRunArtifacts
        ));
    }
}
