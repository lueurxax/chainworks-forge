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

    fn from_entry(entry: &PrincipalEntry) -> Self {
        let mut principal = Principal::new(entry.id.clone(), entry.class.clone());
        if let Some(policies) = entry.surface_policies.as_ref() {
            // surface_policies present: override class defaults. Fail closed —
            // an absent mcp stanza means no MCP tool or resource access.
            if let Some(mcp) = policies.mcp.as_ref() {
                principal.tool_capabilities = mcp
                    .allowed_tools
                    .iter()
                    .filter_map(|tool| capability_tool_id_for_name(tool))
                    .filter(|id| tool_allowed_for_class(&principal.class, *id))
                    .collect();
            } else {
                principal.tool_capabilities = BTreeSet::new();
            }
            // SEC-HIGH-001: when surface_policies are present, deny MCP resource access
            // by default. Resources require an explicit allowlist in a future policy field;
            // absent mcp stanza or absent resource stanza both map to empty resource set.
            principal.resource_capabilities = BTreeSet::new();
        }
        // No surface_policies: v1 behavior — keep class-default capabilities.
        principal
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
                surface_policies: Some(SurfacePolicies {
                    graphql: Some(GraphqlPolicy {
                        allow_queries: true,
                        allow_subscriptions: true,
                        allowed_mutations: approval_mutations(),
                    }),
                    mcp: None,
                }),
            }],
        }
    }

    /// Load from a JSON file. If the file does not exist, bootstrap a default
    /// operator-class principal, write it to disk, and return the table.
    pub fn load_or_bootstrap(path: &Path) -> Result<Self, AuthError> {
        if path.exists() {
            // MEDIUM-001: Close the TOCTOU window between the symlink check and open.
            // lstat (symlink_metadata) rejects obvious symlink presence; the opened fd's
            // inode is then compared with the lstat inode so a symlink substitution that
            // races between the two checks is detected before any content is read.
            #[cfg(unix)]
            let content = {
                use std::io::Read;
                use std::os::unix::fs::MetadataExt;
                let sym_meta = std::fs::symlink_metadata(path).map_err(|e| {
                    AuthError::TableLoadFailed(format!("stat {}: {e}", path.display()))
                })?;
                if sym_meta.file_type().is_symlink() {
                    return Err(AuthError::TableLoadFailed(
                        "principals.json must not be a symlink".into(),
                    ));
                }
                let lstat_ino = sym_meta.ino();
                let mut file = std::fs::File::open(path).map_err(|e| {
                    AuthError::TableLoadFailed(format!("open {}: {e}", path.display()))
                })?;
                let fd_meta = file.metadata().map_err(|e| {
                    AuthError::TableLoadFailed(format!("fstat {}: {e}", path.display()))
                })?;
                // Inode mismatch means the file was replaced after lstat (symlink race).
                if fd_meta.ino() != lstat_ino {
                    return Err(AuthError::TableLoadFailed(
                        "principals.json was replaced between stat and open".into(),
                    ));
                }
                let mode = fd_meta.mode() & 0o777;
                if mode != 0o600 {
                    return Err(AuthError::TableLoadFailed(format!(
                        "principals.json has unsafe permissions {:o}; expected 0600",
                        mode
                    )));
                }
                let mut content = String::new();
                file.read_to_string(&mut content).map_err(|e| {
                    AuthError::TableLoadFailed(format!("read {}: {e}", path.display()))
                })?;
                content
            };
            #[cfg(not(unix))]
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
            let principals = normalize_principal_entries(file.schema_version, file.principals)?;
            if file.schema_version == Some(2) {
                validate_v2_principals(&principals)?;
            }
            Ok(PrincipalTable {
                entries: principals,
            })
        } else {
            // Bootstrap a default operator token
            let token = uuid::Uuid::new_v4().to_string();
            let entry = default_operator_entry(token.clone());
            let file = PrincipalTableFile {
                schema_version: Some(2),
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
                // create_new(true) maps to O_CREAT|O_EXCL — fails atomically if a file or
                // symlink was placed at `path` between the exists() check and this open,
                // preventing TOCTOU-based symlink substitution attacks.
                let mut file = std::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
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
                "Auto-bootstrapped default operator principal (token written to file, not logged)"
            );
            Ok(PrincipalTable {
                entries: vec![entry],
            })
        }
    }
}

fn default_operator_entry(token: String) -> PrincipalEntry {
    PrincipalEntry {
        token,
        id: "default-operator".into(),
        class: PrincipalClass::Operator,
        surface_policies: Some(SurfacePolicies {
            graphql: Some(GraphqlPolicy {
                allow_queries: true,
                allow_subscriptions: true,
                allowed_mutations: approval_mutations(),
            }),
            mcp: Some(McpPolicy {
                allowed_tools: vec![],
            }),
        }),
    }
}

fn approval_mutations() -> Vec<String> {
    vec!["approveApproval".into(), "rejectApproval".into()]
}

fn is_exact_approval_mutation_set(mutations: &[String]) -> bool {
    let mut sorted = mutations.to_vec();
    sorted.sort();
    sorted == approval_mutations()
}

fn normalize_principal_entries(
    schema_version: Option<u32>,
    mut entries: Vec<PrincipalEntry>,
) -> Result<Vec<PrincipalEntry>, AuthError> {
    if schema_version == Some(2) {
        return Ok(entries);
    }

    for entry in &mut entries {
        if entry.id == "default-operator"
            && entry.class == PrincipalClass::Operator
            && entry.surface_policies.is_none()
        {
            entry.surface_policies = default_operator_entry(entry.token.clone()).surface_policies;
        }
    }
    Ok(entries)
}

// ── Token resolution ────────────────────────────────────────────────────

/// Constant-time byte slice comparison to avoid timing side-channels during
/// bearer-token lookup. MEDIUM-001: bearer tokens are secrets; normal string
/// comparison allows timing oracle attacks. We scan all entries (no early exit
/// on match) and XOR-fold bytes without short-circuiting on length mismatch.
fn constant_time_bytes_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff: u8 = 0;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

pub fn resolve_bearer(token: &str, table: &PrincipalTable) -> Result<Principal, AuthError> {
    // MEDIUM-001: scan all entries without early exit so the number of iterations
    // does not leak how many entries are in the table, and use constant-time
    // byte comparison so match position does not leak token prefix information.
    let mut found: Option<&PrincipalEntry> = None;
    for entry in &table.entries {
        if constant_time_bytes_eq(entry.token.as_bytes(), token.as_bytes()) {
            found = Some(entry);
        }
    }
    found
        .map(Principal::from_entry)
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

    // HIGH-002: ALL principals in schema_version 2 must declare surface_policies explicitly.
    // Omitting surface_policies keeps class-default tool_capabilities (fail-open) for every
    // class, not just Operator.  Reject at load time so no misconfigured entry reaches
    // resolve_bearer regardless of class.
    for entry in entries {
        if entry.surface_policies.is_none() {
            return Err(AuthError::TableLoadFailed(format!(
                "schema_version 2 principal '{}' (class {:?}) must declare surface_policies; \
                 omitting surface_policies grants class-default MCP/resource access (fail-closed)",
                entry.id, entry.class
            )));
        }
    }

    // Validate known app-owned principals.
    for entry in entries {
        if let Some(ref policies) = entry.surface_policies {
            if let Some(ref graphql) = policies.graphql {
                // Validate known mutation names.
                let known_mutations = ["approveApproval", "rejectApproval"];
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

        // P072: default-operator is the app bearer. It must support reads and
        // exactly the two approval mutations, while all other commands remain
        // MCP-only.
        if entry.id == "default-operator" {
            let policies = entry.surface_policies.as_ref().ok_or_else(|| {
                AuthError::TableLoadFailed(
                    "default-operator must have surface_policies in schema_version 2".into(),
                )
            })?;
            let graphql = policies.graphql.as_ref().ok_or_else(|| {
                AuthError::TableLoadFailed(
                    "default-operator must have graphql surface_policies".into(),
                )
            })?;
            if !graphql.allow_queries || !graphql.allow_subscriptions {
                return Err(AuthError::TableLoadFailed(
                    "default-operator must allow GraphQL queries and subscriptions".into(),
                ));
            }
            if !is_exact_approval_mutation_set(&graphql.allowed_mutations) {
                return Err(AuthError::TableLoadFailed(
                    "default-operator must allow only approveApproval and rejectApproval".into(),
                ));
            }
            if let Some(ref mcp) = policies.mcp {
                for tool in &mcp.allowed_tools {
                    if capability_tool_id_for_name(tool).is_none() {
                        return Err(AuthError::TableLoadFailed(format!(
                            "unknown MCP tool '{}' in surface_policies for principal '{}'",
                            tool, entry.id
                        )));
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
                AuthError::TableLoadFailed("ui_operator must have graphql surface_policies".into())
            })?;
            let mut sorted = graphql.allowed_mutations.clone();
            sorted.sort();
            if sorted != vec!["approveApproval", "rejectApproval"] {
                return Err(AuthError::TableLoadFailed(
                    "ui_operator must allow exactly approveApproval and rejectApproval".into(),
                ));
            }
            if !graphql.allow_queries || !graphql.allow_subscriptions {
                return Err(AuthError::TableLoadFailed(
                    "ui_operator must allow GraphQL queries and subscriptions".into(),
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
        .map(Principal::from_entry)
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

/// P072: Check if GraphQL queries are allowed for a principal based on v2 surface_policies.
/// Returns None if the principal has no surface_policies (v1 behavior applies).
/// When surface_policies is present but graphql stanza is absent, returns Some(false) (fail closed).
pub fn is_query_allowed_by_surface_policy(
    table: &PrincipalTable,
    principal_id: &str,
) -> Option<bool> {
    let entry = table.entries.iter().find(|e| e.id == principal_id)?;
    let sp = entry.surface_policies.as_ref()?;
    Some(sp.graphql.as_ref().map_or(false, |g| g.allow_queries))
}

/// P072: Check if GraphQL subscriptions are allowed for a principal based on v2 surface_policies.
/// Returns None if the principal has no surface_policies (v1 behavior applies).
/// When surface_policies is present but graphql stanza is absent, returns Some(false) (fail closed).
pub fn is_subscription_allowed_by_surface_policy(
    table: &PrincipalTable,
    principal_id: &str,
) -> Option<bool> {
    let entry = table.entries.iter().find(|e| e.id == principal_id)?;
    let sp = entry.surface_policies.as_ref()?;
    Some(sp.graphql.as_ref().map_or(false, |g| g.allow_subscriptions))
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

fn all_tool_capabilities() -> [CapabilityToolId; 38] {
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
        CapabilityToolId::WorkflowLoopBudgetExtend,
        CapabilityToolId::LegacyDiscoveryOverrideCreate,
        CapabilityToolId::ReportsGet,
        CapabilityToolId::ArtifactsOverrideContract,
        CapabilityToolId::StewardRunAnalysis,
        CapabilityToolId::StewardListAnalyses,
        CapabilityToolId::StewardGetAnalysis,
        CapabilityToolId::RuntimeHealth,
        CapabilityToolId::StorageHealth,
        CapabilityToolId::StorageWritePressure,
        CapabilityToolId::StorageEvidenceSpoolSummary,
        CapabilityToolId::StorageReconcileEvidenceOrphans,
        CapabilityToolId::ProposalGateSettle,
        CapabilityToolId::EffectsList,
        CapabilityToolId::EffectsInspect,
        CapabilityToolId::EffectsReconcile,
        CapabilityToolId::EffectsMarkConflict,
        CapabilityToolId::EffectsMarkUnrecoverable,
        CapabilityToolId::EffectsClearAfterManualVerification,
        CapabilityToolId::StorageMaintenanceRepairSlot,
        CapabilityToolId::StorageProjectionsClearBacklog,
        CapabilityToolId::StorageProjectionsClearPoison,
    ]
}

fn tool_allowed_for_class(class: &PrincipalClass, id: CapabilityToolId) -> bool {
    match id {
        CapabilityToolId::IdeasCreate => {
            matches!(class, PrincipalClass::Operator | PrincipalClass::Agent)
        }
        CapabilityToolId::IdeasList => true,
        // SEC-001: runs.start supplies caller-controlled filesystem paths to the daemon.
        // Restrict to Operator to prevent Agent principals from directing arbitrary reads.
        CapabilityToolId::RunsStart => {
            matches!(class, PrincipalClass::Operator)
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
        CapabilityToolId::WorkflowLoopBudgetExtend => matches!(class, PrincipalClass::Operator),
        CapabilityToolId::LegacyDiscoveryOverrideCreate => {
            matches!(class, PrincipalClass::Operator)
        }
        // SEC-HIGH-001: reports.get returns operator-grade report/evidence payloads,
        // local file_path values, rollout readback, and failed-stage evidence. Restrict
        // to Operator principals to match the report:// resource boundary.
        CapabilityToolId::ReportsGet => matches!(class, PrincipalClass::Operator),
        CapabilityToolId::ArtifactsOverrideContract => matches!(class, PrincipalClass::Operator),
        CapabilityToolId::StewardRunAnalysis => matches!(class, PrincipalClass::Operator),
        CapabilityToolId::StewardListAnalyses => {
            matches!(class, PrincipalClass::Operator | PrincipalClass::Observer)
        }
        CapabilityToolId::StewardGetAnalysis => {
            matches!(class, PrincipalClass::Operator | PrincipalClass::Observer)
        }
        CapabilityToolId::RuntimeHealth => {
            matches!(class, PrincipalClass::Operator | PrincipalClass::Observer)
        }
        // SEC-004: storage diagnostics expose WAL, queue pressure, orphan counts, and
        // kill-switch state — restrict to Operator to match the GraphQL storageHealth boundary.
        CapabilityToolId::StorageHealth => {
            matches!(class, PrincipalClass::Operator)
        }
        CapabilityToolId::StorageWritePressure => {
            matches!(class, PrincipalClass::Operator)
        }
        CapabilityToolId::StorageEvidenceSpoolSummary => {
            matches!(class, PrincipalClass::Operator)
        }
        CapabilityToolId::StorageReconcileEvidenceOrphans => {
            matches!(class, PrincipalClass::Operator)
        }
        CapabilityToolId::ProposalGateSettle => matches!(class, PrincipalClass::Operator),
        // P078: All effects.* tools are Operator-only. The MCP surface is the
        // only reconciliation command/control path; last_error and evidence_root
        // may contain sensitive adapter output so Observer access is not granted.
        CapabilityToolId::EffectsList => matches!(class, PrincipalClass::Operator),
        CapabilityToolId::EffectsInspect => matches!(class, PrincipalClass::Operator),
        CapabilityToolId::EffectsReconcile => matches!(class, PrincipalClass::Operator),
        CapabilityToolId::EffectsMarkConflict => matches!(class, PrincipalClass::Operator),
        CapabilityToolId::EffectsMarkUnrecoverable => matches!(class, PrincipalClass::Operator),
        CapabilityToolId::EffectsClearAfterManualVerification => {
            matches!(class, PrincipalClass::Operator)
        }
        CapabilityToolId::StorageMaintenanceRepairSlot => matches!(class, PrincipalClass::Operator),
        CapabilityToolId::StorageProjectionsClearBacklog => {
            matches!(class, PrincipalClass::Operator)
        }
        CapabilityToolId::StorageProjectionsClearPoison => {
            matches!(class, PrincipalClass::Operator)
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
        // HIGH-001: artifact:// exposes local filesystem file_path; report:// exposes
        // execution evidence and artifact payloads. Both are Operator-only to prevent
        // Agent/Observer principals from reading sensitive path or evidence material.
        ResourceTemplateId::ArtifactEntity => matches!(class, PrincipalClass::Operator),
        ResourceTemplateId::ReportEntity => matches!(class, PrincipalClass::Operator),
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
        // SEC-HIGH-002: chainworks://runs/{run_id}/artifacts handler returns
        // ArtifactIndexRow including file_path and source generation metadata.
        // Restrict to Operator to prevent Observer/Agent callers from leaking
        // filesystem paths and sensitive source/session/work-item identifiers.
        ResourceTemplateId::ChainworksRunArtifacts => {
            matches!(class, PrincipalClass::Operator)
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
        "workflow_loop_budget.extend" => Some(CapabilityToolId::WorkflowLoopBudgetExtend),
        "legacy_discovery_override_create" => Some(CapabilityToolId::LegacyDiscoveryOverrideCreate),
        "reports.get" => Some(CapabilityToolId::ReportsGet),
        "artifacts.override_contract" => Some(CapabilityToolId::ArtifactsOverrideContract),
        "steward.run_analysis" => Some(CapabilityToolId::StewardRunAnalysis),
        "steward.list_analyses" => Some(CapabilityToolId::StewardListAnalyses),
        "steward.get_analysis" => Some(CapabilityToolId::StewardGetAnalysis),
        "runtime.health" => Some(CapabilityToolId::RuntimeHealth),
        "storage.health" => Some(CapabilityToolId::StorageHealth),
        "storage.write_pressure" => Some(CapabilityToolId::StorageWritePressure),
        "storage.evidence_spool_summary" => Some(CapabilityToolId::StorageEvidenceSpoolSummary),
        "storage.reconcile_evidence_orphans" => {
            Some(CapabilityToolId::StorageReconcileEvidenceOrphans)
        }
        "effects.list" => Some(CapabilityToolId::EffectsList),
        "effects.inspect" => Some(CapabilityToolId::EffectsInspect),
        "effects.reconcile" => Some(CapabilityToolId::EffectsReconcile),
        "effects.mark_conflict" => Some(CapabilityToolId::EffectsMarkConflict),
        "effects.mark_unrecoverable" => Some(CapabilityToolId::EffectsMarkUnrecoverable),
        "effects.clear_after_manual_verification" => {
            Some(CapabilityToolId::EffectsClearAfterManualVerification)
        }
        "storage.maintenance.repair_slot" => Some(CapabilityToolId::StorageMaintenanceRepairSlot),
        "storage.projections.clear_backlog" => {
            Some(CapabilityToolId::StorageProjectionsClearBacklog)
        }
        "storage.projections.clear_poison" => Some(CapabilityToolId::StorageProjectionsClearPoison),
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
        // SEC-HIGH-002: chainworks://runs/{id}/artifacts is now Operator-only;
        // Observer match must return None.
        assert_eq!(
            match_resource_uri(
                &observer,
                "server-owned-artifacts-uri",
                test_resource_id_for_uri
            ),
            None
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
        // SEC-001: runs.start is now Operator-only (supplies daemon-side filesystem paths).
        assert!(!is_tool_allowed(&p, "runs.start"));
        assert!(!is_tool_allowed(&p, "approvals.resolve"));
        assert!(!is_tool_allowed(&p, "stages.retry"));
        assert!(!is_tool_allowed(&p, "runs.main_sync.request"));
        assert!(!is_tool_allowed(&p, "runs.cancel"));
    }

    #[test]
    fn observer_read_only() {
        let p = Principal::new("ob", PrincipalClass::Observer);
        assert!(is_tool_allowed(&p, "runs.list"));
        // SEC-HIGH-001: reports.get is Operator-only; Observer must be denied.
        assert!(!is_tool_allowed(&p, "reports.get"));
        assert!(!is_tool_allowed(&p, "ideas.create"));
        assert!(!is_tool_allowed(&p, "runs.start"));
    }

    #[test]
    fn proposal_087_storage_tools_are_operator_only() {
        let op = Principal::new("op", PrincipalClass::Operator);
        let ag = Principal::new("ag", PrincipalClass::Agent);
        let ob = Principal::new("ob", PrincipalClass::Observer);

        let p087_tools = [
            "storage.maintenance.repair_slot",
            "storage.projections.clear_backlog",
            "storage.projections.clear_poison",
            "storage.health",
        ];

        for tool in p087_tools {
            assert!(is_tool_allowed(&op, tool), "Operator should allow {}", tool);
            assert!(!is_tool_allowed(&ag, tool), "Agent should deny {}", tool);
            assert!(!is_tool_allowed(&ob, tool), "Observer should deny {}", tool);
        }
    }

    #[test]
    fn reports_get_is_operator_only() {
        // SEC-HIGH-001: reports.get exposes operator-grade report payloads,
        // local file_path values, rollout readback, and failed-stage evidence.
        let op = Principal::new("op", PrincipalClass::Operator);
        let ag = Principal::new("ag", PrincipalClass::Agent);
        let ob = Principal::new("ob", PrincipalClass::Observer);
        assert!(
            is_tool_allowed(&op, "reports.get"),
            "Operator must have reports.get"
        );
        assert!(
            !is_tool_allowed(&ag, "reports.get"),
            "Agent must not have reports.get"
        );
        assert!(
            !is_tool_allowed(&ob, "reports.get"),
            "Observer must not have reports.get"
        );
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
        // HIGH-001: ArtifactEntity and ReportEntity are Operator-only; Observer no longer has access.
        assert!(!is_resource_allowed(
            &ob,
            ResourceTemplateId::ArtifactEntity
        ));
        assert!(!is_resource_allowed(&ob, ResourceTemplateId::ReportEntity));
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
        // SEC-HIGH-002: chainworks://runs/{id}/artifacts returns file_path and
        // source generation metadata; Observer must be denied.
        assert!(!is_resource_allowed(
            &ob,
            ResourceTemplateId::ChainworksRunArtifacts
        ));
    }

    #[test]
    fn chainworks_run_artifacts_resource_is_operator_only() {
        // SEC-HIGH-002: artifact list returns unredacted file_path and sensitive
        // source/session/work-item identifiers; must be Operator-only.
        let op = Principal::new("op", PrincipalClass::Operator);
        let ag = Principal::new("ag", PrincipalClass::Agent);
        let ob = Principal::new("ob", PrincipalClass::Observer);
        assert!(
            is_resource_allowed(&op, ResourceTemplateId::ChainworksRunArtifacts),
            "Operator must have ChainworksRunArtifacts"
        );
        assert!(
            !is_resource_allowed(&ag, ResourceTemplateId::ChainworksRunArtifacts),
            "Agent must not have ChainworksRunArtifacts"
        );
        assert!(
            !is_resource_allowed(&ob, ResourceTemplateId::ChainworksRunArtifacts),
            "Observer must not have ChainworksRunArtifacts"
        );
    }

    #[test]
    fn artifact_and_report_resources_are_operator_only() {
        let op = Principal::new("op", PrincipalClass::Operator);
        let ag = Principal::new("ag", PrincipalClass::Agent);
        let ob = Principal::new("ob", PrincipalClass::Observer);
        // HIGH-001: artifact:// exposes local file_path; report:// exposes execution evidence.
        assert!(is_resource_allowed(&op, ResourceTemplateId::ArtifactEntity));
        assert!(is_resource_allowed(&op, ResourceTemplateId::ReportEntity));
        assert!(!is_resource_allowed(
            &ag,
            ResourceTemplateId::ArtifactEntity
        ));
        assert!(!is_resource_allowed(&ag, ResourceTemplateId::ReportEntity));
        assert!(!is_resource_allowed(
            &ob,
            ResourceTemplateId::ArtifactEntity
        ));
        assert!(!is_resource_allowed(&ob, ResourceTemplateId::ReportEntity));
    }

    #[test]
    fn v2_custom_operator_without_surface_policies_fails_closed() {
        // HIGH-002: any Operator principal in schema_version 2 that omits surface_policies
        // must be rejected at load time rather than granted class-default access.
        let entries = vec![
            PrincipalEntry {
                token: "tok-op".into(),
                id: "default-operator".into(),
                class: PrincipalClass::Operator,
                surface_policies: Some(SurfacePolicies {
                    graphql: Some(GraphqlPolicy {
                        allow_queries: true,
                        allow_subscriptions: true,
                        allowed_mutations: vec!["approveApproval".into(), "rejectApproval".into()],
                    }),
                    mcp: None,
                }),
            },
            PrincipalEntry {
                token: "tok-custom".into(),
                id: "custom-agent-operator".into(),
                class: PrincipalClass::Operator,
                surface_policies: None,
            },
        ];
        let err = validate_v2_principals(&entries).unwrap_err();
        assert!(
            err.to_string().contains("must declare surface_policies"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn v2_agent_without_surface_policies_fails_closed() {
        // HIGH-002 extended: Agent and Observer principals in schema_version 2 must also
        // declare surface_policies; omitting keeps class-default MCP/resource capabilities.
        for (class, id, token) in [
            (PrincipalClass::Agent, "my-agent", "tok-ag"),
            (PrincipalClass::Observer, "my-observer", "tok-ob"),
        ] {
            let entries = vec![PrincipalEntry {
                token: token.into(),
                id: id.into(),
                class,
                surface_policies: None,
            }];
            let err = validate_v2_principals(&entries).unwrap_err();
            assert!(
                err.to_string().contains("must declare surface_policies"),
                "class {:?} without surface_policies must be rejected: {err}",
                entries[0].class
            );
        }
    }

    #[test]
    fn bearer_constant_time_eq_rejects_on_diff_and_wrong_length() {
        // MEDIUM-001: constant_time_bytes_eq must reject wrong values and lengths.
        assert!(constant_time_bytes_eq(b"abcdef", b"abcdef"));
        assert!(!constant_time_bytes_eq(b"abcdef", b"abcdeX"));
        assert!(!constant_time_bytes_eq(b"abcdef", b"abcde"));
        assert!(!constant_time_bytes_eq(b"abcdef", b"abcdefg"));
    }

    #[test]
    fn resolve_bearer_uses_constant_time_comparison() {
        let table = PrincipalTable {
            entries: vec![PrincipalEntry {
                token: "aaaa-bbbb-cccc-dddd".into(),
                id: "test-op".into(),
                class: PrincipalClass::Operator,
                surface_policies: None,
            }],
        };
        // Correct token resolves.
        assert!(resolve_bearer("aaaa-bbbb-cccc-dddd", &table).is_ok());
        // Off-by-one character (same length): must fail.
        assert!(resolve_bearer("aaaa-bbbb-cccc-dddX", &table).is_err());
        // Wrong length: must fail.
        assert!(resolve_bearer("aaaa-bbbb-cccc-dddd-extra", &table).is_err());
        assert!(resolve_bearer("short", &table).is_err());
        // Empty: must fail.
        assert!(resolve_bearer("", &table).is_err());
    }

    // ── P072 v2 schema tests ───────────────────────────────────────────

    #[test]
    fn v2_validates_unified_ui_operator_table() {
        let entries = vec![
            PrincipalEntry {
                token: "tok-read".into(),
                id: "default-operator".into(),
                class: PrincipalClass::Operator,
                surface_policies: Some(SurfacePolicies {
                    graphql: Some(GraphqlPolicy {
                        allow_queries: true,
                        allow_subscriptions: true,
                        allowed_mutations: vec!["approveApproval".into(), "rejectApproval".into()],
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
                        allow_queries: true,
                        allow_subscriptions: true,
                        allowed_mutations: vec!["approveApproval".into(), "rejectApproval".into()],
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
    fn legacy_default_operator_file_is_normalized_to_p072_ui_policy() {
        let file = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(
            file.path(),
            r#"{
              "principals": [
                {
                  "token": "legacy-token",
                  "id": "default-operator",
                  "class": "operator"
                }
              ]
            }"#,
        )
        .unwrap();

        let table = PrincipalTable::load_or_bootstrap(file.path()).unwrap();

        assert_eq!(
            is_mutation_allowed_by_surface_policy(&table, "default-operator", "approveApproval"),
            Some(true)
        );
        assert_eq!(
            is_mutation_allowed_by_surface_policy(&table, "default-operator", "startRun"),
            Some(false)
        );
        assert_eq!(
            is_query_allowed_by_surface_policy(&table, "default-operator"),
            Some(true)
        );
        assert_eq!(
            is_subscription_allowed_by_surface_policy(&table, "default-operator"),
            Some(true)
        );
    }

    #[test]
    fn test_fixture_uses_approval_only_graphql_policy() {
        let table = PrincipalTable::test_fixture();

        assert_eq!(
            is_mutation_allowed_by_surface_policy(&table, "test-operator", "approveApproval"),
            Some(true)
        );
        assert_eq!(
            is_mutation_allowed_by_surface_policy(&table, "test-operator", "rejectApproval"),
            Some(true)
        );
        for mutation in [
            "startRun",
            "approveStage",
            "rejectStage",
            "retryStage",
            "overrideLegacyDiscoveryPolicy",
            "cancelRun",
        ] {
            assert_eq!(
                is_mutation_allowed_by_surface_policy(&table, "test-operator", mutation),
                Some(false),
                "{mutation} must not be exposed by the default GraphQL test fixture"
            );
        }
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
                    allow_queries: true,
                    allow_subscriptions: true,
                    allowed_mutations: vec![
                        "approveApproval".into(),
                        "rejectApproval".into(),
                        "startRun".into(),
                    ],
                }),
                mcp: None,
            }),
        }];
        let err = validate_v2_principals(&entries).unwrap_err();
        assert!(err.to_string().contains("unknown mutation"));
    }

    #[test]
    fn v2_rejects_default_operator_with_non_approval_mutations() {
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
            .contains("default-operator must allow only approveApproval and rejectApproval"));
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
                    allowed_mutations: vec!["approveApproval".into()],
                }),
                mcp: None,
            }),
        }];
        let err = validate_v2_principals(&entries).unwrap_err();
        assert!(err.to_string().contains("ui_operator must allow exactly"));
    }

    #[test]
    fn v2_rejects_ui_operator_with_mcp_tools() {
        let entries = vec![PrincipalEntry {
            token: "tok".into(),
            id: "ui_operator".into(),
            class: PrincipalClass::Operator,
            surface_policies: Some(SurfacePolicies {
                graphql: Some(GraphqlPolicy {
                    allow_queries: true,
                    allow_subscriptions: true,
                    allowed_mutations: vec!["approveApproval".into(), "rejectApproval".into()],
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
                            allowed_mutations: approval_mutations(),
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
                            allow_queries: true,
                            allow_subscriptions: true,
                            allowed_mutations: approval_mutations(),
                        }),
                        mcp: None,
                    }),
                },
            ],
        };
        // default-operator: unified app bearer allows only approval mutations
        assert_eq!(
            is_mutation_allowed_by_surface_policy(&table, "default-operator", "approveApproval"),
            Some(true)
        );
        assert_eq!(
            is_mutation_allowed_by_surface_policy(&table, "default-operator", "startRun"),
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

    #[test]
    fn resolve_bearer_applies_v2_mcp_surface_policy() {
        let table = PrincipalTable {
            entries: vec![
                PrincipalEntry {
                    token: "tok-read".into(),
                    id: "default-operator".into(),
                    class: PrincipalClass::Operator,
                    surface_policies: Some(SurfacePolicies {
                        graphql: Some(GraphqlPolicy {
                            allow_queries: true,
                            allow_subscriptions: true,
                            allowed_mutations: approval_mutations(),
                        }),
                        mcp: Some(McpPolicy {
                            allowed_tools: vec!["runs.list".into(), "runs.get".into()],
                        }),
                    }),
                },
                PrincipalEntry {
                    token: "tok-ui".into(),
                    id: "ui_operator".into(),
                    class: PrincipalClass::Operator,
                    surface_policies: Some(SurfacePolicies {
                        graphql: Some(GraphqlPolicy {
                            allow_queries: true,
                            allow_subscriptions: true,
                            allowed_mutations: approval_mutations(),
                        }),
                        mcp: Some(McpPolicy {
                            allowed_tools: vec![],
                        }),
                    }),
                },
            ],
        };

        let read_principal = resolve_bearer("tok-read", &table).unwrap();
        assert!(is_tool_allowed(&read_principal, "runs.list"));
        assert!(is_tool_allowed(&read_principal, "runs.get"));
        assert!(!is_tool_allowed(&read_principal, "runs.start"));

        let ui_principal = resolve_bearer("tok-ui", &table).unwrap();
        assert!(!is_tool_allowed(&ui_principal, "approvals.resolve"));
        assert!(!is_tool_allowed(&ui_principal, "runs.list"));
    }

    #[test]
    fn v2_rejects_unknown_mcp_tool_name() {
        let entries = vec![PrincipalEntry {
            token: "tok".into(),
            id: "default-operator".into(),
            class: PrincipalClass::Operator,
            surface_policies: Some(SurfacePolicies {
                graphql: Some(GraphqlPolicy {
                    allow_queries: true,
                    allow_subscriptions: true,
                    allowed_mutations: approval_mutations(),
                }),
                mcp: Some(McpPolicy {
                    allowed_tools: vec!["runs.lsit".into()],
                }),
            }),
        }];
        let err = validate_v2_principals(&entries).unwrap_err();
        assert!(err.to_string().contains("unknown MCP tool"));
    }

    #[test]
    fn is_query_allowed_by_surface_policy_checks() {
        let table = PrincipalTable {
            entries: vec![
                PrincipalEntry {
                    token: "tok-read".into(),
                    id: "default-operator".into(),
                    class: PrincipalClass::Operator,
                    surface_policies: Some(SurfacePolicies {
                        graphql: Some(GraphqlPolicy {
                            allow_queries: true,
                            allow_subscriptions: true,
                            allowed_mutations: approval_mutations(),
                        }),
                        mcp: None,
                    }),
                },
                PrincipalEntry {
                    token: "tok-ui".into(),
                    id: "ui_operator".into(),
                    class: PrincipalClass::Operator,
                    surface_policies: Some(SurfacePolicies {
                        graphql: Some(GraphqlPolicy {
                            allow_queries: true,
                            allow_subscriptions: true,
                            allowed_mutations: approval_mutations(),
                        }),
                        mcp: None,
                    }),
                },
                PrincipalEntry {
                    token: "tok-v1".into(),
                    id: "v1-operator".into(),
                    class: PrincipalClass::Operator,
                    surface_policies: None,
                },
            ],
        };
        // default-operator: queries allowed
        assert_eq!(
            is_query_allowed_by_surface_policy(&table, "default-operator"),
            Some(true)
        );
        // default-operator: subscriptions allowed
        assert_eq!(
            is_subscription_allowed_by_surface_policy(&table, "default-operator"),
            Some(true)
        );
        // ui_operator: queries allowed for the unified app flow
        assert_eq!(
            is_query_allowed_by_surface_policy(&table, "ui_operator"),
            Some(true)
        );
        // ui_operator: subscriptions allowed for the unified app flow
        assert_eq!(
            is_subscription_allowed_by_surface_policy(&table, "ui_operator"),
            Some(true)
        );
        // v1 principal without surface_policies: returns None (no restriction)
        assert_eq!(
            is_query_allowed_by_surface_policy(&table, "v1-operator"),
            None
        );
        assert_eq!(
            is_subscription_allowed_by_surface_policy(&table, "v1-operator"),
            None
        );
    }

    #[test]
    fn surface_policies_without_graphql_stanza_fails_closed() {
        // A principal that has surface_policies but NO graphql stanza must not be
        // allowed to bypass the check by returning None (which callers treat as
        // "v1, allow through"). It must return Some(false) — fail closed.
        let table = PrincipalTable {
            entries: vec![PrincipalEntry {
                token: "tok-mcp-only".into(),
                id: "mcp-only".into(),
                class: PrincipalClass::Operator,
                surface_policies: Some(SurfacePolicies {
                    graphql: None,
                    mcp: Some(McpPolicy {
                        allowed_tools: vec!["runs.list".into()],
                    }),
                }),
            }],
        };
        assert_eq!(
            is_query_allowed_by_surface_policy(&table, "mcp-only"),
            Some(false),
            "surface_policies present but no graphql stanza must deny queries"
        );
        assert_eq!(
            is_subscription_allowed_by_surface_policy(&table, "mcp-only"),
            Some(false),
            "surface_policies present but no graphql stanza must deny subscriptions"
        );
    }

    // ── P078 effects.* auth tests ─────────────────────────────────────────

    #[test]
    fn proposal_078_effects_tool_names_are_recognized() {
        // All P078 effects.* tool names must resolve to their CapabilityToolIds.
        // Without these arms, v2 principal tables listing effects.* tools would
        // crash the principal-table loader with "unknown MCP tool".
        let cases = [
            ("effects.list", CapabilityToolId::EffectsList),
            ("effects.inspect", CapabilityToolId::EffectsInspect),
            ("effects.reconcile", CapabilityToolId::EffectsReconcile),
            (
                "effects.mark_conflict",
                CapabilityToolId::EffectsMarkConflict,
            ),
            (
                "effects.mark_unrecoverable",
                CapabilityToolId::EffectsMarkUnrecoverable,
            ),
            (
                "effects.clear_after_manual_verification",
                CapabilityToolId::EffectsClearAfterManualVerification,
            ),
        ];
        for (name, expected) in cases {
            assert_eq!(
                capability_tool_id_for_name(name),
                Some(expected),
                "capability_tool_id_for_name({name}) must return {expected:?}"
            );
        }
    }

    #[test]
    fn proposal_078_effects_tools_operator_only() {
        // All effects.* tools must be granted to Operator and denied to Observer/Agent.
        let op = Principal::new("op", PrincipalClass::Operator);
        let ob = Principal::new("ob", PrincipalClass::Observer);
        let ag = Principal::new("ag", PrincipalClass::Agent);
        for tool in [
            "effects.list",
            "effects.inspect",
            "effects.reconcile",
            "effects.mark_conflict",
            "effects.mark_unrecoverable",
            "effects.clear_after_manual_verification",
        ] {
            assert!(is_tool_allowed(&op, tool), "Operator must have {tool}");
            assert!(!is_tool_allowed(&ob, tool), "Observer must not have {tool}");
            assert!(!is_tool_allowed(&ag, tool), "Agent must not have {tool}");
        }
    }

    #[test]
    fn proposal_087_repair_slot_is_operator_only() {
        let op = Principal::new("op-p087", PrincipalClass::Operator);
        let ob = Principal::new("ob-p087", PrincipalClass::Observer);
        let ag = Principal::new("ag-p087", PrincipalClass::Agent);

        assert!(is_tool_allowed(&op, "storage.maintenance.repair_slot"));
        assert!(!is_tool_allowed(&ob, "storage.maintenance.repair_slot"));
        assert!(!is_tool_allowed(&ag, "storage.maintenance.repair_slot"));
    }

    #[test]
    fn proposal_078_v2_principal_with_effects_tools_is_accepted() {
        // A v2 principal table listing effects.* tools must not be rejected.
        let entries = vec![PrincipalEntry {
            token: "tok".into(),
            id: "default-operator".into(),
            class: PrincipalClass::Operator,
            surface_policies: Some(SurfacePolicies {
                graphql: Some(GraphqlPolicy {
                    allow_queries: true,
                    allow_subscriptions: true,
                    allowed_mutations: approval_mutations(),
                }),
                mcp: Some(McpPolicy {
                    allowed_tools: vec![
                        "effects.list".into(),
                        "effects.inspect".into(),
                        "effects.reconcile".into(),
                    ],
                }),
            }),
        }];
        assert!(
            validate_v2_principals(&entries).is_ok(),
            "v2 principal table with effects.* tools must be accepted"
        );
    }

    #[test]
    fn proposal_078_resolve_bearer_grants_effects_tools_from_surface_policy() {
        let table = PrincipalTable {
            entries: vec![PrincipalEntry {
                token: "tok-effects".into(),
                id: "default-operator".into(),
                class: PrincipalClass::Operator,
                surface_policies: Some(SurfacePolicies {
                    graphql: Some(GraphqlPolicy {
                        allow_queries: true,
                        allow_subscriptions: true,
                        allowed_mutations: approval_mutations(),
                    }),
                    mcp: Some(McpPolicy {
                        allowed_tools: vec![
                            "effects.list".into(),
                            "effects.inspect".into(),
                            "effects.reconcile".into(),
                        ],
                    }),
                }),
            }],
        };
        let p = resolve_bearer("tok-effects", &table).unwrap();
        assert!(p.tool_capabilities.contains(&CapabilityToolId::EffectsList));
        assert!(p
            .tool_capabilities
            .contains(&CapabilityToolId::EffectsInspect));
        assert!(p
            .tool_capabilities
            .contains(&CapabilityToolId::EffectsReconcile));
        assert!(!p
            .tool_capabilities
            .contains(&CapabilityToolId::EffectsMarkUnrecoverable));
    }

    // ── HIGH-001 regression: v2 surface_policies present but mcp absent → empty tools ──

    #[test]
    fn high_001_v2_surface_policies_with_no_mcp_stanza_fails_closed() {
        // A v2 principal that has surface_policies but omits the mcp stanza
        // must resolve to ZERO tool_capabilities — NOT the class-default set.
        let table = PrincipalTable {
            entries: vec![PrincipalEntry {
                token: "tok-no-mcp".into(),
                id: "default-operator".into(),
                class: PrincipalClass::Operator,
                surface_policies: Some(SurfacePolicies {
                    graphql: Some(GraphqlPolicy {
                        allow_queries: true,
                        allow_subscriptions: true,
                        allowed_mutations: approval_mutations(),
                    }),
                    mcp: None, // deliberately absent
                }),
            }],
        };

        let p = resolve_bearer("tok-no-mcp", &table).unwrap();

        // Must have zero MCP tools — fail closed, not class defaults.
        assert!(
            p.tool_capabilities.is_empty(),
            "A v2 principal with surface_policies but no mcp stanza must have \
             empty tool_capabilities, got {:?}",
            p.tool_capabilities
        );

        // Crucially, the dangerous effects.* tools must NOT be granted.
        assert!(
            !p.tool_capabilities
                .contains(&CapabilityToolId::EffectsMarkUnrecoverable),
            "effects.mark_unrecoverable must not be granted when mcp stanza is absent"
        );
        assert!(
            !p.tool_capabilities
                .contains(&CapabilityToolId::EffectsClearAfterManualVerification),
            "effects.clear_after_manual_verification must not be granted when mcp stanza is absent"
        );
    }

    #[test]
    fn high_001_v1_principal_without_surface_policies_keeps_class_defaults() {
        // A v1 principal (no surface_policies at all) must still get class-default tools.
        let table = PrincipalTable {
            entries: vec![PrincipalEntry {
                token: "tok-v1".into(),
                id: "v1-op".into(),
                class: PrincipalClass::Operator,
                surface_policies: None,
            }],
        };

        let p = resolve_bearer("tok-v1", &table).unwrap();
        assert!(
            !p.tool_capabilities.is_empty(),
            "v1 principal without surface_policies must keep class-default tool_capabilities"
        );
        assert!(
            p.tool_capabilities.contains(&CapabilityToolId::RunsList),
            "v1 operator must have runs.list by default"
        );
    }

    // ── HIGH-002 regression: bootstrap log must not contain the bearer token ──

    #[test]
    fn high_002_bootstrap_log_does_not_contain_token() {
        use std::sync::{Arc, Mutex};

        let log_output: Arc<Mutex<Vec<u8>>> = Arc::new(Mutex::new(Vec::new()));

        struct CaptureWriter(Arc<Mutex<Vec<u8>>>);
        impl std::io::Write for CaptureWriter {
            fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
                self.0.lock().unwrap().extend_from_slice(buf);
                Ok(buf.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }

        let log_output_clone = log_output.clone();
        let make_writer = move || CaptureWriter(log_output_clone.clone());

        let subscriber = tracing_subscriber::fmt()
            .with_writer(make_writer)
            .with_max_level(tracing::Level::INFO)
            .finish();

        let dir = tempfile::tempdir().expect("create tmp dir");
        let path = dir.path().join("principals.json");

        tracing::subscriber::with_default(subscriber, || {
            let _table =
                PrincipalTable::load_or_bootstrap(&path).expect("bootstrap should succeed");
        });

        // Read the actual token that was written to the file
        let content = std::fs::read_to_string(&path).expect("principals file exists");
        let parsed: serde_json::Value = serde_json::from_str(&content).expect("valid JSON");
        let token = parsed["principals"][0]["token"]
            .as_str()
            .expect("token field present");

        let log_str = String::from_utf8_lossy(&log_output.lock().unwrap()).to_string();

        assert!(
            !log_str.contains(token),
            "Bootstrap log must not contain the bearer token. \
             Token was found in log output. This is a security regression (HIGH-002)."
        );
    }

    // ── SEC-004 regression ──────────────────────────────────────────────

    /// SEC-004: Observer must not have storage diagnostic capabilities.
    /// GraphQL storageHealth requires Operator; MCP storage tools must match.
    #[test]
    fn sec004_observer_cannot_access_mcp_storage_diagnostics() {
        let observer = Principal::new("observer-sec004", PrincipalClass::Observer);
        let operator = Principal::new("operator-sec004", PrincipalClass::Operator);

        let operator_only_tools = [
            CapabilityToolId::StorageHealth,
            CapabilityToolId::StorageWritePressure,
            CapabilityToolId::StorageEvidenceSpoolSummary,
            CapabilityToolId::StorageReconcileEvidenceOrphans,
        ];
        for tool in operator_only_tools {
            assert!(
                !observer.tool_capabilities.contains(&tool),
                "Observer must not have {tool:?} (SEC-004): \
                 storage diagnostics must be Operator-only to match GraphQL policy"
            );
            assert!(
                operator.tool_capabilities.contains(&tool),
                "Operator must have {tool:?} (SEC-004)"
            );
        }
    }

    #[test]
    fn sec_high_001_v2_surface_policies_deny_mcp_resources_by_default() {
        // SEC-HIGH-001: when surface_policies is present, resource_capabilities must be
        // empty by default regardless of class. An Operator with surface_policies but no
        // explicit resource allowlist must NOT be able to read artifact://, report://, or
        // chainworks://runs/{id}/artifacts resources.
        let entry = PrincipalEntry {
            token: "tok".into(),
            id: "graphql-scoped-operator".into(),
            class: PrincipalClass::Operator,
            surface_policies: Some(SurfacePolicies {
                graphql: Some(GraphqlPolicy {
                    allow_queries: true,
                    allow_subscriptions: true,
                    allowed_mutations: vec!["approveApproval".into(), "rejectApproval".into()],
                }),
                mcp: None,
            }),
        };
        let principal = Principal::from_entry(&entry);
        assert!(
            principal.resource_capabilities.is_empty(),
            "v2 Operator with surface_policies must have empty resource_capabilities by default"
        );
        assert!(
            !is_resource_allowed(&principal, ResourceTemplateId::ArtifactEntity),
            "ArtifactEntity must be denied when surface_policies present and no resource allowlist"
        );
        assert!(
            !is_resource_allowed(&principal, ResourceTemplateId::ReportEntity),
            "ReportEntity must be denied when surface_policies present and no resource allowlist"
        );
        assert!(
            !is_resource_allowed(&principal, ResourceTemplateId::ChainworksRunArtifacts),
            "ChainworksRunArtifacts must be denied when surface_policies present"
        );
        assert!(
            !is_resource_allowed(&principal, ResourceTemplateId::RunEntity),
            "RunEntity must be denied when surface_policies present and no resource allowlist"
        );
    }

    #[test]
    fn sec_high_001_v2_surface_policies_with_empty_mcp_also_denies_resources() {
        // SEC-HIGH-001: an explicit empty McpPolicy must also result in no resource access.
        let entry = PrincipalEntry {
            token: "tok".into(),
            id: "mcp-empty-operator".into(),
            class: PrincipalClass::Operator,
            surface_policies: Some(SurfacePolicies {
                graphql: Some(GraphqlPolicy {
                    allow_queries: true,
                    allow_subscriptions: true,
                    allowed_mutations: vec!["approveApproval".into(), "rejectApproval".into()],
                }),
                mcp: Some(McpPolicy {
                    allowed_tools: vec![],
                }),
            }),
        };
        let principal = Principal::from_entry(&entry);
        assert!(
            principal.resource_capabilities.is_empty(),
            "v2 Operator with empty McpPolicy must have empty resource_capabilities"
        );
        assert!(
            !is_resource_allowed(&principal, ResourceTemplateId::ArtifactEntity),
            "ArtifactEntity must be denied for empty-tool v2 principal"
        );
    }

    #[test]
    fn sec_high_001_v1_principal_without_surface_policies_retains_default_resources() {
        // SEC-HIGH-001: v1 principals (no surface_policies) must NOT be affected —
        // they keep class-default resource_capabilities.
        let entry = PrincipalEntry {
            token: "tok".into(),
            id: "legacy-operator".into(),
            class: PrincipalClass::Operator,
            surface_policies: None,
        };
        let principal = Principal::from_entry(&entry);
        assert!(
            !principal.resource_capabilities.is_empty(),
            "v1 Operator without surface_policies must retain default resource_capabilities"
        );
        assert!(
            is_resource_allowed(&principal, ResourceTemplateId::ArtifactEntity),
            "ArtifactEntity must be allowed for v1 Operator"
        );
    }
}
