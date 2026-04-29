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
    /// P072 v2: Per-principal surface policies. Required for app-owned
    /// GraphQL principals in schema_version 2.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    surface_policies: Option<SurfacePolicies>,
}

/// P072: Per-principal surface policies for schema_version 2.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SurfacePolicies {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub graphql: Option<GraphqlPolicy>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mcp: Option<McpPolicy>,
}

/// P072: GraphQL-specific principal policy.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GraphqlPolicy {
    #[serde(default)]
    pub allow_queries: bool,
    #[serde(default)]
    pub allow_subscriptions: bool,
    #[serde(default)]
    pub allowed_mutations: Vec<String>,
}

/// P072: MCP-specific principal policy.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct McpPolicy {
    #[serde(default)]
    pub allowed_tools: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct PrincipalTableFile {
    #[serde(default)]
    schema_version: Option<u32>,
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
                surface_policies: None,
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
            // P072: Validate v2 schema constraints if present.
            if file.schema_version == Some(2) {
                validate_v2_principals(&file.principals)?;
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
                surface_policies: None,
            };
            let file = PrincipalTableFile {
                schema_version: None,
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

// ── P072: v2 principal table validation ────────────────────────────────

/// Validate P072 schema_version 2 constraints on app-owned principals.
fn validate_v2_principals(entries: &[PrincipalEntry]) -> Result<(), AuthError> {
    use std::collections::HashSet;

    // Duplicate id check.
    let mut ids = HashSet::new();
    for entry in entries {
        if !ids.insert(&entry.id) {
            return Err(AuthError::TableLoadFailed(format!(
                "duplicate principal id: {}",
                entry.id
            )));
        }
    }

    // Duplicate token check.
    let mut tokens = HashSet::new();
    for entry in entries {
        if !tokens.insert(&entry.token) {
            return Err(AuthError::TableLoadFailed(
                "duplicate token in principal table".into(),
            ));
        }
    }

    // Validate known app-owned principals.
    for entry in entries {
        if let Some(ref policies) = entry.surface_policies {
            if let Some(ref graphql) = policies.graphql {
                // Validate known mutation names.
                let known_mutations = [
                    "startRun",
                    "approveStage",
                    "rejectStage",
                    "retryStage",
                    "overrideLegacyDiscoveryPolicy",
                    "cancelRun",
                    "approveApproval",
                    "rejectApproval",
                ];
                for mutation in &graphql.allowed_mutations {
                    if !known_mutations.contains(&mutation.as_str()) {
                        return Err(AuthError::TableLoadFailed(format!(
                            "unknown mutation '{}' in surface_policies for principal '{}'",
                            mutation, entry.id
                        )));
                    }
                }
            }
        }

        // P072: default-operator must have empty allowed_mutations.
        if entry.id == "default-operator" {
            if let Some(ref policies) = entry.surface_policies {
                if let Some(ref graphql) = policies.graphql {
                    if !graphql.allowed_mutations.is_empty() {
                        return Err(AuthError::TableLoadFailed(
                            "default-operator must have empty GraphQL allowed_mutations".into(),
                        ));
                    }
                }
            }
        }

        // P072: ui_operator must allow exactly approveApproval and rejectApproval.
        if entry.id == "ui_operator" {
            let policies = entry.surface_policies.as_ref().ok_or_else(|| {
                AuthError::TableLoadFailed(
                    "ui_operator must have surface_policies in schema_version 2".into(),
                )
            })?;
            let graphql = policies.graphql.as_ref().ok_or_else(|| {
                AuthError::TableLoadFailed(
                    "ui_operator must have graphql surface_policies".into(),
                )
            })?;
            let mut sorted = graphql.allowed_mutations.clone();
            sorted.sort();
            if sorted != vec!["approveApproval", "rejectApproval"] {
                return Err(AuthError::TableLoadFailed(
                    "ui_operator must allow exactly approveApproval and rejectApproval".into(),
                ));
            }
            // ui_operator must not have MCP tools.
            if let Some(ref mcp) = policies.mcp {
                if !mcp.allowed_tools.is_empty() {
                    return Err(AuthError::TableLoadFailed(
                        "ui_operator must not allow MCP tools".into(),
                    ));
                }
            }
        }
    }

    Ok(())
}

/// P072: Look up a principal by exact id.
pub fn find_principal_by_id(table: &PrincipalTable, id: &str) -> Option<Principal> {
    table
        .entries
        .iter()
        .find(|e| e.id == id)
        .map(|e| Principal::new(e.id.clone(), e.class.clone()))
}

/// P072: Check if a mutation is allowed for a principal based on v2 surface_policies.
/// Returns None if the principal has no surface_policies (v1 behavior applies).
pub fn is_mutation_allowed_by_surface_policy(
    table: &PrincipalTable,
    principal_id: &str,
    mutation_name: &str,
) -> Option<bool> {
    table
        .entries
        .iter()
        .find(|e| e.id == principal_id)
        .and_then(|e| e.surface_policies.as_ref())
        .and_then(|sp| sp.graphql.as_ref())
        .map(|graphql| graphql.allowed_mutations.iter().any(|m| m == mutation_name))
}

// ── Capability filtering ────────────────────────────────────────────────

pub fn filter_tools(principal: &Principal, ids: &[CapabilityToolId]) -> Vec<CapabilityToolId> {
    ids.iter()
        .copied()
        .filter(|id| {
            tool_allowed_for_class(&principal.class, *id)
                && principal.tool_capabilities.contains(id)
        })
        .collect()
}

pub fn filter_resources(
    principal: &Principal,
    ids: &[ResourceTemplateId],
) -> Vec<ResourceTemplateId> {
    ids.iter()
        .copied()
        .filter(|id| {
            resource_allowed_for_class(&principal.class, *id)
                && principal.resource_capabilities.contains(id)
        })
        .collect()
}

/// Check if a specific tool is allowed for a principal.
pub fn is_tool_allowed(principal: &Principal, tool_name: &str) -> bool {
    let Some(id) = capability_tool_id_for_name(tool_name) else {
        return false;
    };
    filter_tools(principal, &[id]).len() == 1
}

fn default_tool_capabilities(class: &PrincipalClass) -> BTreeSet<CapabilityToolId> {
    all_tool_capabilities()
        .into_iter()
        .filter(|id| tool_allowed_for_class(class, *id))
        .collect()
}

fn all_tool_capabilities() -> [CapabilityToolId; 22] {
    [
        CapabilityToolId::IdeasCreate,
        CapabilityToolId::IdeasList,
        CapabilityToolId::RunsStart,
        CapabilityToolId::RunsList,
        CapabilityToolId::RunsGet,
        CapabilityToolId::RunsMainSyncRequest,
        CapabilityToolId::RunsMainSyncRetry,
        CapabilityToolId::RunsMainSyncSetOverride,
        CapabilityToolId::RunsMainSyncRepairState,
        CapabilityToolId::RunsMainSyncRecordRecoveryDecision,
        CapabilityToolId::RunsKnowledgeCapsuleIgnore,
        CapabilityToolId::RunsCancel,
        CapabilityToolId::ApprovalsList,
        CapabilityToolId::ApprovalsResolve,
        CapabilityToolId::StagesRetry,
        CapabilityToolId::WorkflowConflictsResolve,
        CapabilityToolId::LegacyDiscoveryOverrideCreate,
        CapabilityToolId::ReportsGet,
        CapabilityToolId::ArtifactsOverrideContract,
        CapabilityToolId::StewardRunAnalysis,
        CapabilityToolId::StewardListAnalyses,
        CapabilityToolId::StewardGetAnalysis,
    ]
}

fn tool_allowed_for_class(class: &PrincipalClass, id: CapabilityToolId) -> bool {
    match id {
        CapabilityToolId::IdeasCreate => {
            matches!(class, PrincipalClass::Operator | PrincipalClass::Agent)
        }
        CapabilityToolId::IdeasList => true,
        CapabilityToolId::RunsStart => {
            matches!(class, PrincipalClass::Operator | PrincipalClass::Agent)
        }
        CapabilityToolId::RunsList => true,
        CapabilityToolId::RunsGet => true,
        CapabilityToolId::RunsMainSyncRequest => matches!(class, PrincipalClass::Operator),
        CapabilityToolId::RunsMainSyncRetry => matches!(class, PrincipalClass::Operator),
        CapabilityToolId::RunsMainSyncSetOverride => matches!(class, PrincipalClass::Operator),
        CapabilityToolId::RunsMainSyncRepairState => matches!(class, PrincipalClass::Operator),
        CapabilityToolId::RunsMainSyncRecordRecoveryDecision => {
            matches!(class, PrincipalClass::Operator)
        }
        CapabilityToolId::RunsKnowledgeCapsuleIgnore => matches!(class, PrincipalClass::Operator),
        CapabilityToolId::RunsCancel => matches!(class, PrincipalClass::Operator),
        CapabilityToolId::ApprovalsList => {
            matches!(class, PrincipalClass::Operator | PrincipalClass::Observer)
        }
        CapabilityToolId::ApprovalsResolve => matches!(class, PrincipalClass::Operator),
        CapabilityToolId::StagesRetry => matches!(class, PrincipalClass::Operator),
        CapabilityToolId::WorkflowConflictsResolve => matches!(class, PrincipalClass::Operator),
        CapabilityToolId::LegacyDiscoveryOverrideCreate => {
            matches!(class, PrincipalClass::Operator)
        }
        CapabilityToolId::ReportsGet => true,
        CapabilityToolId::ArtifactsOverrideContract => matches!(class, PrincipalClass::Operator),
        CapabilityToolId::StewardRunAnalysis => matches!(class, PrincipalClass::Operator),
        CapabilityToolId::StewardListAnalyses => {
            matches!(class, PrincipalClass::Operator | PrincipalClass::Observer)
        }
        CapabilityToolId::StewardGetAnalysis => {
            matches!(class, PrincipalClass::Operator | PrincipalClass::Observer)
        }
    }
}

// ── Resource capability filtering ───────────────────────────────────────

fn default_resource_capabilities(class: &PrincipalClass) -> BTreeSet<ResourceTemplateId> {
    all_resource_templates()
        .into_iter()
        .filter(|id| resource_allowed_for_class(class, *id))
        .collect()
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
    resource_allowed_for_class(&principal.class, id)
        && principal.resource_capabilities.contains(&id)
}

fn resource_allowed_for_class(class: &PrincipalClass, id: ResourceTemplateId) -> bool {
    match id {
        ResourceTemplateId::RunEntity => true,
        ResourceTemplateId::IdeaEntity => true,
        ResourceTemplateId::ArtifactEntity => true,
        ResourceTemplateId::ReportEntity => true,
        ResourceTemplateId::StewardAnalysisEntity => {
            matches!(class, PrincipalClass::Operator | PrincipalClass::Observer)
        }
        ResourceTemplateId::ChainworksRuns => true,
        ResourceTemplateId::ChainworksIdeas => true,
        ResourceTemplateId::ChainworksApprovalsInbox => {
            matches!(class, PrincipalClass::Operator | PrincipalClass::Observer)
        }
        ResourceTemplateId::ChainworksRunStages => {
            matches!(class, PrincipalClass::Operator | PrincipalClass::Observer)
        }
        ResourceTemplateId::ChainworksRunArtifacts => {
            matches!(class, PrincipalClass::Operator | PrincipalClass::Observer)
        }
    }
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
    is_resource_allowed(principal, id).then_some(id)
}

fn capability_tool_id_for_name(name: &str) -> Option<CapabilityToolId> {
    match name {
        "ideas.create" => Some(CapabilityToolId::IdeasCreate),
        "ideas.list" => Some(CapabilityToolId::IdeasList),
        "runs.start" => Some(CapabilityToolId::RunsStart),
        "runs.list" => Some(CapabilityToolId::RunsList),
        "runs.get" => Some(CapabilityToolId::RunsGet),
        "runs.main_sync.request" => Some(CapabilityToolId::RunsMainSyncRequest),
        "runs.main_sync.retry" => Some(CapabilityToolId::RunsMainSyncRetry),
        "runs.main_sync.set_override" => Some(CapabilityToolId::RunsMainSyncSetOverride),
        "runs.main_sync.repair_state" => Some(CapabilityToolId::RunsMainSyncRepairState),
        "runs.main_sync.record_recovery_decision" => {
            Some(CapabilityToolId::RunsMainSyncRecordRecoveryDecision)
        }
        "runs.knowledge_capsule.ignore" => Some(CapabilityToolId::RunsKnowledgeCapsuleIgnore),
        "runs.cancel" => Some(CapabilityToolId::RunsCancel),
        "approvals.list" => Some(CapabilityToolId::ApprovalsList),
        "approvals.resolve" => Some(CapabilityToolId::ApprovalsResolve),
        "stages.retry" => Some(CapabilityToolId::StagesRetry),
        "workflow_conflicts.resolve" => Some(CapabilityToolId::WorkflowConflictsResolve),
        "legacy_discovery_override_create" => Some(CapabilityToolId::LegacyDiscoveryOverrideCreate),
        "reports.get" => Some(CapabilityToolId::ReportsGet),
        "artifacts.override_contract" => Some(CapabilityToolId::ArtifactsOverrideContract),
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
        assert!(is_tool_allowed(&p, "runs.main_sync.request"));
        assert!(is_tool_allowed(&p, "approvals.resolve"));
        assert!(is_tool_allowed(&p, "stages.retry"));
    }

    #[test]
    fn agent_cannot_approve() {
        let p = Principal::new("ag", PrincipalClass::Agent);
        assert!(is_tool_allowed(&p, "runs.start"));
        assert!(!is_tool_allowed(&p, "approvals.resolve"));
        assert!(!is_tool_allowed(&p, "stages.retry"));
        assert!(!is_tool_allowed(&p, "runs.main_sync.request"));
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
                surface_policies: None,
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

    // ── P072 v2 schema tests ───────────────────────────────────────────

    #[test]
    fn v2_validates_valid_dual_principal_table() {
        let entries = vec![
            PrincipalEntry {
                token: "tok-read".into(),
                id: "default-operator".into(),
                class: PrincipalClass::Operator,
                surface_policies: Some(SurfacePolicies {
                    graphql: Some(GraphqlPolicy {
                        allow_queries: true,
                        allow_subscriptions: true,
                        allowed_mutations: vec![],
                    }),
                    mcp: Some(McpPolicy {
                        allowed_tools: vec![],
                    }),
                }),
            },
            PrincipalEntry {
                token: "tok-write".into(),
                id: "ui_operator".into(),
                class: PrincipalClass::Operator,
                surface_policies: Some(SurfacePolicies {
                    graphql: Some(GraphqlPolicy {
                        allow_queries: false,
                        allow_subscriptions: false,
                        allowed_mutations: vec![
                            "approveApproval".into(),
                            "rejectApproval".into(),
                        ],
                    }),
                    mcp: Some(McpPolicy {
                        allowed_tools: vec![],
                    }),
                }),
            },
        ];
        assert!(validate_v2_principals(&entries).is_ok());
    }

    #[test]
    fn v2_rejects_duplicate_ids() {
        let entries = vec![
            PrincipalEntry {
                token: "tok-1".into(),
                id: "same-id".into(),
                class: PrincipalClass::Operator,
                surface_policies: None,
            },
            PrincipalEntry {
                token: "tok-2".into(),
                id: "same-id".into(),
                class: PrincipalClass::Operator,
                surface_policies: None,
            },
        ];
        let err = validate_v2_principals(&entries).unwrap_err();
        assert!(err.to_string().contains("duplicate principal id"));
    }

    #[test]
    fn v2_rejects_duplicate_tokens() {
        let entries = vec![
            PrincipalEntry {
                token: "same-tok".into(),
                id: "id-1".into(),
                class: PrincipalClass::Operator,
                surface_policies: None,
            },
            PrincipalEntry {
                token: "same-tok".into(),
                id: "id-2".into(),
                class: PrincipalClass::Operator,
                surface_policies: None,
            },
        ];
        let err = validate_v2_principals(&entries).unwrap_err();
        assert!(err.to_string().contains("duplicate token"));
    }

    #[test]
    fn v2_rejects_unknown_mutation_name() {
        let entries = vec![PrincipalEntry {
            token: "tok".into(),
            id: "ui_operator".into(),
            class: PrincipalClass::Operator,
            surface_policies: Some(SurfacePolicies {
                graphql: Some(GraphqlPolicy {
                    allow_queries: false,
                    allow_subscriptions: false,
                    allowed_mutations: vec![
                        "approveApproval".into(),
                        "rejectApproval".into(),
                        "deleteEverything".into(),
                    ],
                }),
                mcp: None,
            }),
        }];
        let err = validate_v2_principals(&entries).unwrap_err();
        assert!(err.to_string().contains("unknown mutation"));
    }

    #[test]
    fn v2_rejects_default_operator_with_mutations() {
        let entries = vec![PrincipalEntry {
            token: "tok".into(),
            id: "default-operator".into(),
            class: PrincipalClass::Operator,
            surface_policies: Some(SurfacePolicies {
                graphql: Some(GraphqlPolicy {
                    allow_queries: true,
                    allow_subscriptions: true,
                    allowed_mutations: vec!["approveApproval".into()],
                }),
                mcp: None,
            }),
        }];
        let err = validate_v2_principals(&entries).unwrap_err();
        assert!(err
            .to_string()
            .contains("default-operator must have empty"));
    }

    #[test]
    fn v2_rejects_ui_operator_with_wrong_mutations() {
        let entries = vec![PrincipalEntry {
            token: "tok".into(),
            id: "ui_operator".into(),
            class: PrincipalClass::Operator,
            surface_policies: Some(SurfacePolicies {
                graphql: Some(GraphqlPolicy {
                    allow_queries: false,
                    allow_subscriptions: false,
                    allowed_mutations: vec!["approveApproval".into(), "startRun".into()],
                }),
                mcp: None,
            }),
        }];
        let err = validate_v2_principals(&entries).unwrap_err();
        assert!(err
            .to_string()
            .contains("ui_operator must allow exactly"));
    }

    #[test]
    fn v2_rejects_ui_operator_with_mcp_tools() {
        let entries = vec![PrincipalEntry {
            token: "tok".into(),
            id: "ui_operator".into(),
            class: PrincipalClass::Operator,
            surface_policies: Some(SurfacePolicies {
                graphql: Some(GraphqlPolicy {
                    allow_queries: false,
                    allow_subscriptions: false,
                    allowed_mutations: vec![
                        "approveApproval".into(),
                        "rejectApproval".into(),
                    ],
                }),
                mcp: Some(McpPolicy {
                    allowed_tools: vec!["runs.start".into()],
                }),
            }),
        }];
        let err = validate_v2_principals(&entries).unwrap_err();
        assert!(err.to_string().contains("must not allow MCP tools"));
    }

    #[test]
    fn find_principal_by_id_returns_principal() {
        let table = PrincipalTable {
            entries: vec![
                PrincipalEntry {
                    token: "tok-1".into(),
                    id: "default-operator".into(),
                    class: PrincipalClass::Operator,
                    surface_policies: None,
                },
                PrincipalEntry {
                    token: "tok-2".into(),
                    id: "ui_operator".into(),
                    class: PrincipalClass::Operator,
                    surface_policies: None,
                },
            ],
        };
        assert!(find_principal_by_id(&table, "default-operator").is_some());
        assert!(find_principal_by_id(&table, "ui_operator").is_some());
        assert!(find_principal_by_id(&table, "nonexistent").is_none());
    }

    #[test]
    fn is_mutation_allowed_by_surface_policy_checks() {
        let table = PrincipalTable {
            entries: vec![
                PrincipalEntry {
                    token: "tok-1".into(),
                    id: "default-operator".into(),
                    class: PrincipalClass::Operator,
                    surface_policies: Some(SurfacePolicies {
                        graphql: Some(GraphqlPolicy {
                            allow_queries: true,
                            allow_subscriptions: true,
                            allowed_mutations: vec![],
                        }),
                        mcp: None,
                    }),
                },
                PrincipalEntry {
                    token: "tok-2".into(),
                    id: "ui_operator".into(),
                    class: PrincipalClass::Operator,
                    surface_policies: Some(SurfacePolicies {
                        graphql: Some(GraphqlPolicy {
                            allow_queries: false,
                            allow_subscriptions: false,
                            allowed_mutations: vec![
                                "approveApproval".into(),
                                "rejectApproval".into(),
                            ],
                        }),
                        mcp: None,
                    }),
                },
            ],
        };
        // default-operator: no mutations allowed
        assert_eq!(
            is_mutation_allowed_by_surface_policy(&table, "default-operator", "approveApproval"),
            Some(false)
        );
        // ui_operator: approval mutations allowed
        assert_eq!(
            is_mutation_allowed_by_surface_policy(&table, "ui_operator", "approveApproval"),
            Some(true)
        );
        assert_eq!(
            is_mutation_allowed_by_surface_policy(&table, "ui_operator", "rejectApproval"),
            Some(true)
        );
        assert_eq!(
            is_mutation_allowed_by_surface_policy(&table, "ui_operator", "startRun"),
            Some(false)
        );
        // v1 principal without surface_policies
        assert_eq!(
            is_mutation_allowed_by_surface_policy(&table, "nonexistent", "approveApproval"),
            None
        );
    }
}
