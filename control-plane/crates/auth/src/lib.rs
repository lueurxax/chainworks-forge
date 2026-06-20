use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::path::Path;
use std::sync::{Arc, RwLock};

// P029: PrincipalClass is canonically defined in domain::commands.
// Re-export here so downstream crates that use auth::PrincipalClass keep working.
pub use domain::{CapabilityToolId, PrincipalClass, ResourceTemplateId};

/// P081 Phase 1: boundary matrix fixture loading and validation.
pub mod boundary;

// ── P081 Phase 2: CallerClass and CallerContext ──────────────────────────

/// P081 Phase 2: Request-scoped caller classification derived from principal,
/// token, transport, and surface policy. Not stored in principal table.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CallerClass {
    UiOperator,
    AgentOperator,
    Automation,
    Observer,
    DeveloperBreakGlass,
}

impl CallerClass {
    pub fn as_str(&self) -> &'static str {
        match self {
            CallerClass::UiOperator => "ui_operator",
            CallerClass::AgentOperator => "agent_operator",
            CallerClass::Automation => "automation",
            CallerClass::Observer => "observer",
            CallerClass::DeveloperBreakGlass => "developer_break_glass",
        }
    }
}

impl std::fmt::Display for CallerClass {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// P081 Phase 2: Request-scoped caller context derived at auth resolution time.
/// transport is a string matching the boundary matrix transport enum values.
#[derive(Debug, Clone)]
pub struct CallerContext {
    pub principal_id: String,
    pub principal_class: PrincipalClass,
    pub caller_class: CallerClass,
    pub transport: String,
    pub token_id: Option<String>,
    pub request_id: Option<String>,
}

/// P081 Phase 2/3: Derive the CallerClass from a resolved principal.
/// Checks caller_class_override first (v3 principals), then falls back to
/// PrincipalClass-based derivation. Caller-supplied provenance never overrides
/// the stored principal-table entry; PrincipalClass remains the persisted identity truth.
///
/// Derivation rules:
/// - v3 explicit override: automation, developer_break_glass, or any CallerClass
/// - Observer principal → observer
/// - Operator principal → ui_operator (GraphQL surface is the governed UI boundary)
/// - Agent principal → agent_operator
pub fn derive_caller_class(principal: &Principal) -> CallerClass {
    if let Some(ref cc) = principal.caller_class_override {
        return cc.clone();
    }
    derive_caller_class_from_principal_class(&principal.class)
}

/// P081 Phase 3: Derive CallerClass directly from PrincipalClass.
/// Used for GraphQL request context where only the class is available without
/// a full Principal object. Operator principals on GraphQL are ui_operator.
pub fn derive_caller_class_from_principal_class(class: &PrincipalClass) -> CallerClass {
    match class {
        PrincipalClass::Operator => CallerClass::UiOperator,
        PrincipalClass::Agent => CallerClass::AgentOperator,
        PrincipalClass::Observer => CallerClass::Observer,
        // read_only_operator: treated as Observer for routing; capability matrix controls access.
        PrincipalClass::ReadOnlyOperator => CallerClass::Observer,
    }
}

/// P081 Phase 3: Derive CallerClass for MCP transports from a resolved Principal.
///
/// Checks caller_class_override first so v3 automation and developer_break_glass
/// principals reach their matrix rows. Operator principals on MCP are classified
/// as agent_operator; ui_operator is reserved for the Swift UI connecting via GraphQL.
pub fn derive_caller_class_for_mcp(principal: &Principal) -> CallerClass {
    if let Some(ref cc) = principal.caller_class_override {
        return cc.clone();
    }
    match &principal.class {
        PrincipalClass::Operator => CallerClass::AgentOperator,
        PrincipalClass::Agent => CallerClass::AgentOperator,
        PrincipalClass::Observer => CallerClass::Observer,
        PrincipalClass::ReadOnlyOperator => CallerClass::Observer,
    }
}

/// Validate that a raw token (without "Bearer " prefix) meets the P081 length
/// and character-set requirements. Used by MCP stdio where no HTTP header is present.
pub fn validate_raw_token(token: &str) -> bool {
    token.len() >= 32 && token.len() <= 4096 && token.bytes().all(|b| (0x21..=0x7e).contains(&b))
}

/// P080 Phase 1: Check whether a principal is authorized to access a specific run_id.
///
/// For restricted principals (Agent, ReadOnlyOperator):
/// - Must have an explicit `run_scope` configured. Without one, returns
///   `Err("auth_scope_required: ...")` — fail-closed to prevent cross-run disclosure
///   via caller-supplied run_id (SEC-P080-001).
/// - With a non-empty `run_scope`, `run_id` must be in the scope set →
///   `Ok(())` or `Err(auth_scope_violation)`.
/// - With an empty `run_scope`, always `Err(auth_scope_violation)`.
///
/// For Operator/Observer principals: always `Ok(())` (no run-level restriction).
pub fn check_p080_run_scope(
    principal: &Principal,
    filter_run_id: Option<&str>,
) -> Result<(), &'static str> {
    let restricted = matches!(
        principal.class,
        PrincipalClass::Agent | PrincipalClass::ReadOnlyOperator
    );
    if !restricted {
        return Ok(());
    }
    if let Some(scope) = principal.run_scope.as_ref() {
        if scope.is_empty() {
            // Empty scope list → fail closed: no run is accessible.
            return Err(
                "auth_scope_violation: principal run_scope is empty; no run_id is authorized",
            );
        }
        match filter_run_id {
            Some(id) if scope.iter().any(|s| s == id) => Ok(()),
            _ => Err("auth_scope_violation: filter.run_id is not in the principal's authorized run_scope"),
        }
    } else {
        // Fail-closed: restricted principal with no run_scope cannot access any run's P080 data.
        // Operator must configure run-binding before granting P080 diagnostics access (SEC-P080-001).
        Err("auth_scope_required: restricted principal has no run_scope configured; contact operator to configure run-binding")
    }
}

// ── Principal types ─────────────────────────────────────────────────────

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Principal {
    pub id: String,
    pub class: PrincipalClass,
    #[serde(default)]
    pub tool_capabilities: BTreeSet<CapabilityToolId>,
    #[serde(default)]
    pub resource_capabilities: BTreeSet<ResourceTemplateId>,
    /// P081 v3: explicit caller class override from PrincipalEntry.
    /// None means the class is derived from PrincipalClass per the default rules.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub caller_class_override: Option<CallerClass>,
    #[serde(default, skip)]
    pub has_explicit_surface_policies: bool,
    #[serde(default, skip)]
    pub graphql_policy: Option<GraphqlPolicy>,
    /// P080 Phase 1: optional server-side run scope for Agent/ReadOnlyOperator principals.
    /// When non-empty, only the listed run_ids are accessible for P080 diagnostics.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run_scope: Option<Vec<String>>,
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
            caller_class_override: None,
            has_explicit_surface_policies: false,
            graphql_policy: None,
            run_scope: None,
        }
    }

    fn from_entry(entry: &PrincipalEntry) -> Self {
        let mut principal = Principal::new(entry.id.clone(), entry.class.clone());
        principal.caller_class_override = entry.caller_class_override.clone();
        principal.run_scope = entry.run_scope.clone();
        if let Some(policies) = entry.surface_policies.as_ref() {
            principal.has_explicit_surface_policies = true;
            principal.graphql_policy = policies.graphql.clone();
            // surface_policies present: the mcp stanza controls tool access.
            // No mcp stanza means zero MCP tools and zero MCP resources (fail-closed).
            if let Some(mcp) = policies.mcp.as_ref() {
                principal.tool_capabilities = mcp
                    .allowed_tools
                    .iter()
                    .filter_map(|tool| capability_tool_id_for_name(tool))
                    .filter(|id| tool_allowed_for_class(&principal.class, *id))
                    .collect();
                // SEC-HIGH-001: whenever an mcp stanza is present in surface_policies,
                // always zero resource_capabilities regardless of how many tools are
                // granted. Resources are NOT inherited from class defaults; they must be
                // explicitly granted in a separate stanza. This prevents a narrow
                // tool-only Operator principal (e.g. p080.diagnostics.get.v1 only) from
                // accidentally reading run://, artifact://, or report:// resources through
                // the class-default resource matrix.
                principal.resource_capabilities = BTreeSet::new();
            } else {
                // No mcp stanza: fail-closed for both tools and resources.
                principal.tool_capabilities = BTreeSet::new();
                principal.resource_capabilities = BTreeSet::new();
            }
        }
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
#[serde(deny_unknown_fields)]
struct PrincipalEntry {
    token: String,
    id: String,
    class: PrincipalClass,
    /// P072 v2: Per-principal surface policies. Required for app-owned
    /// GraphQL principals in schema_version 2.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    surface_policies: Option<SurfacePolicies>,
    /// P081 v3: Unix epoch milliseconds after which this principal is no longer valid.
    /// Absent means no expiry (v1/v2 compatibility). Present and in the past → rejected.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    expires_at_ms: Option<i64>,
    /// P081 v3: Unix epoch milliseconds before which this principal is not yet valid.
    /// Absent means no not-before constraint.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    not_before_ms: Option<i64>,
    /// P081 v3: Explicitly disabled principals are rejected like expired tokens,
    /// with no disclosure of the disable reason.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    disabled: Option<bool>,
    /// P081 v3: Explicit caller class for automation and developer_break_glass principals.
    /// Absent means the class is derived from PrincipalClass per the default derivation rules.
    /// Only meaningful for schema_version 3 entries; ignored for v1/v2 compatibility principals.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    caller_class_override: Option<CallerClass>,
    /// P080 Phase 1: optional server-side run scope for Agent/ReadOnlyOperator principals.
    /// When non-empty, the principal may only access P080 diagnostics for runs whose ID
    /// is in this set; the run_id check is server-derived rather than caller-supplied.
    /// Absent or empty means no explicit scope binding (for Operator class; restricted
    /// classes still require run_id in filter but without membership verification).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run_scope: Option<Vec<String>>,
}

/// P072: Per-principal surface policies for schema_version 2.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct SurfacePolicies {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub graphql: Option<GraphqlPolicy>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mcp: Option<McpPolicy>,
}

/// P072: GraphQL-specific principal policy.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct GraphqlPolicy {
    #[serde(default)]
    pub allow_queries: bool,
    #[serde(default)]
    pub allow_subscriptions: bool,
    #[serde(default)]
    pub allowed_mutations: Vec<String>,
}

/// P072: MCP-specific principal policy.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct McpPolicy {
    #[serde(default)]
    pub allowed_tools: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PrincipalTableFile {
    #[serde(default)]
    schema_version: Option<u32>,
    principals: Vec<PrincipalEntry>,
}

/// Maximum supported schema version for principal table files.
const MAX_PRINCIPAL_TABLE_SCHEMA_VERSION: u32 = 3;

#[derive(Clone, Debug)]
pub struct PrincipalTable {
    entries: Vec<PrincipalEntry>,
}

#[derive(Clone, Debug)]
pub struct LivePrincipalSource {
    table: Arc<RwLock<PrincipalTable>>,
}

pub type LivePrincipalTable = LivePrincipalSource;

impl LivePrincipalSource {
    pub fn new(table: PrincipalTable) -> Self {
        Self {
            table: Arc::new(RwLock::new(table)),
        }
    }

    pub fn replace(&self, table: PrincipalTable) {
        if let Ok(mut guard) = self.table.write() {
            *guard = table;
        }
    }

    pub fn update(&self, table: PrincipalTable) {
        self.replace(table);
    }

    pub fn resolve_bearer(&self, token: &str) -> Result<Principal, AuthError> {
        self.table
            .read()
            .map_err(|_| AuthError::TableLoadFailed("principal table lock poisoned".into()))
            .and_then(|guard| resolve_bearer(token, &guard))
    }
}

impl PrincipalTable {
    /// Test/fixture stand-in: single operator principal with a known token.
    /// Plain pub fn (not cfg(test)) because integration tests in other crates
    /// need to construct a table without touching the filesystem.
    pub fn test_fixture() -> Self {
        PrincipalTable {
            entries: vec![PrincipalEntry {
                token: "test-token-xxxxxxxxxxxxxxxxxxxxx".into(),
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
                ..Default::default()
            }],
        }
    }

    pub fn test_fixture_with_id(id: impl Into<String>) -> Self {
        let mut table = Self::test_fixture();
        if let Some(entry) = table.entries.first_mut() {
            entry.id = id.into();
        }
        table
    }

    pub fn test_fixture_with_token(token: impl Into<String>) -> Self {
        let mut table = Self::test_fixture();
        if let Some(entry) = table.entries.first_mut() {
            entry.token = token.into();
        }
        table
    }

    pub fn test_fixture_with_class(
        token: impl Into<String>,
        id: impl Into<String>,
        class: PrincipalClass,
    ) -> Self {
        PrincipalTable {
            entries: vec![PrincipalEntry {
                token: token.into(),
                id: id.into(),
                class,
                surface_policies: None,
                ..Default::default()
            }],
        }
    }

    pub fn test_fixture_graphql_query_only(
        token: impl Into<String>,
        id: impl Into<String>,
    ) -> Self {
        PrincipalTable {
            entries: vec![PrincipalEntry {
                token: token.into(),
                id: id.into(),
                class: PrincipalClass::Operator,
                surface_policies: Some(SurfacePolicies {
                    graphql: Some(GraphqlPolicy {
                        allow_queries: true,
                        allow_subscriptions: true,
                        allowed_mutations: vec![],
                    }),
                    mcp: None,
                }),
                ..Default::default()
            }],
        }
    }

    pub fn test_fixture_disabled_token(token: impl Into<String>, id: impl Into<String>) -> Self {
        PrincipalTable {
            entries: vec![PrincipalEntry {
                token: token.into(),
                id: id.into(),
                class: PrincipalClass::Operator,
                disabled: Some(true),
                surface_policies: None,
                ..Default::default()
            }],
        }
    }

    /// Observer-class fixture for cross-crate tests that need a non-operator token.
    /// Token length meets the 32-byte minimum required by extract_bearer_token.
    pub fn test_fixture_observer() -> Self {
        PrincipalTable {
            entries: vec![PrincipalEntry {
                token: "observer-token-xxxxxxxxxxxxxxxxxx".into(),
                id: "test-observer".into(),
                class: PrincipalClass::Observer,
                surface_policies: None,
                ..Default::default()
            }],
        }
    }

    /// P080: ReadOnlyOperator fixture with P080DiagnosticsGet capability and graphql
    /// subscription policy. Used by cross-crate tests for auth_ok_for_p080_subscription.
    pub fn test_fixture_p080_read_only_operator() -> Self {
        PrincipalTable {
            entries: vec![PrincipalEntry {
                token: "p080-ro-token-xxxxxxxxxxxxxxxxxxxxxxx".into(),
                id: "test-p080-read-only-operator".into(),
                class: PrincipalClass::ReadOnlyOperator,
                surface_policies: Some(SurfacePolicies {
                    graphql: Some(GraphqlPolicy {
                        allow_queries: true,
                        allow_subscriptions: true,
                        allowed_mutations: vec![],
                    }),
                    mcp: Some(McpPolicy {
                        allowed_tools: vec![
                            "p080.diagnostics.get.v1".into(),
                            "p080.reconcile.request.v1".into(),
                        ],
                    }),
                }),
                ..Default::default()
            }],
        }
    }

    /// P080: ReadOnlyOperator fixture with a specific run_scope binding.
    /// Used by cross-crate tests to verify run_scope-restricted P080 subscription auth.
    pub fn test_fixture_p080_read_only_operator_with_scope(run_ids: Vec<String>) -> Self {
        let mut table = Self::test_fixture_p080_read_only_operator();
        if let Some(entry) = table.entries.first_mut() {
            entry.run_scope = Some(run_ids);
        }
        table
    }

    /// Load from a JSON file. If the file does not exist, bootstrap a default
    /// operator-class principal, write it to disk, and return the table.
    ///
    /// SEC-M002: Relative paths are rejected so a config injection attack cannot
    /// redirect auth to a file resolved against an unpredictable working directory.
    /// Callers in packaged mode should additionally verify canonical containment
    /// against the expected auth root before calling this function.
    pub fn load_or_bootstrap(path: &Path) -> Result<Self, AuthError> {
        // Reject relative paths unconditionally: principals.json must be an absolute path
        // so the file cannot be redirected by controlling the process working directory.
        if path.is_relative() {
            return Err(AuthError::TableLoadFailed(format!(
                "principals.json path must be absolute; got relative path: {}",
                path.display()
            )));
        }
        if path.exists() {
            // SEC-001: Explicit symlink/mode checks first so error messages are clear,
            // then open with O_NOFOLLOW to atomically close the race window between the
            // metadata check and the read (prevents a symlink swap between the two steps).
            #[cfg(unix)]
            let content = {
                use std::io::Read;
                use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
                let meta = std::fs::symlink_metadata(path).map_err(|e| {
                    AuthError::TableLoadFailed(format!("stat {}: {e}", path.display()))
                })?;
                if meta.file_type().is_symlink() {
                    return Err(AuthError::TableLoadFailed(format!(
                        "principals.json must not be a symlink: {}",
                        path.display()
                    )));
                }
                if meta.nlink() != 1 {
                    return Err(AuthError::TableLoadFailed(format!(
                        "principals.json must not be hard-linked (nlink={}): {}",
                        meta.nlink(),
                        path.display()
                    )));
                }
                if let Some(parent) = path.parent() {
                    let parent_meta = std::fs::symlink_metadata(parent).map_err(|e| {
                        AuthError::TableLoadFailed(format!(
                            "stat auth dir {}: {e}",
                            parent.display()
                        ))
                    })?;
                    let parent_mode = parent_meta.mode() & 0o777;
                    if parent_mode != 0o700 {
                        return Err(AuthError::TableLoadFailed(format!(
                            "principals.json parent directory must have mode 0700, found 0{parent_mode:o}: {}",
                            parent.display()
                        )));
                    }
                }
                let mode = meta.mode() & 0o777;
                if mode != 0o600 {
                    return Err(AuthError::TableLoadFailed(format!(
                        "principals.json must have mode 0600, found 0{mode:o}: {}",
                        path.display()
                    )));
                }
                // O_NOFOLLOW: if a symlink was swapped in between the metadata check
                // above and this open, ELOOP fails the open and the read never happens.
                // O_NOFOLLOW raw values: macOS = 0x100, Linux = 0x20000.
                #[cfg(target_os = "macos")]
                const O_NOFOLLOW: i32 = 0x100;
                #[cfg(not(target_os = "macos"))]
                const O_NOFOLLOW: i32 = 0x20000;
                let mut f = std::fs::OpenOptions::new()
                    .read(true)
                    .custom_flags(O_NOFOLLOW)
                    .open(path)
                    .map_err(|e| {
                        AuthError::TableLoadFailed(format!(
                            "principals.json must not be a symlink (race detected at open): {}: {e}",
                            path.display()
                        ))
                    })?;
                let mut s = String::new();
                f.read_to_string(&mut s).map_err(|e| {
                    AuthError::TableLoadFailed(format!("read {}: {e}", path.display()))
                })?;
                s
            };
            #[cfg(not(unix))]
            let content = std::fs::read_to_string(path)
                .map_err(|e| AuthError::TableLoadFailed(format!("read {}: {e}", path.display())))?;
            let file: PrincipalTableFile = serde_json::from_str(&content).map_err(|e| {
                AuthError::TableLoadFailed(format!("parse {}: {e}", path.display()))
            })?;
            // Accept only documented schema versions fail-closed (Phase 2 contract).
            // Versions 1..=MAX_PRINCIPAL_TABLE_SCHEMA_VERSION are accepted; None is treated
            // as version 1 for backwards compatibility. Version 0 and any unknown value
            // are rejected.
            let effective_version = file.schema_version.unwrap_or(1);
            if effective_version == 0 || effective_version > MAX_PRINCIPAL_TABLE_SCHEMA_VERSION {
                return Err(AuthError::TableLoadFailed(format!(
                    "unsupported schema_version {} in {}: accepted versions are 1..={} (got {})",
                    effective_version,
                    path.display(),
                    MAX_PRINCIPAL_TABLE_SCHEMA_VERSION,
                    effective_version
                )));
            }
            if file.principals.is_empty() {
                return Err(AuthError::TableLoadFailed(
                    "principal table contains zero entries".into(),
                ));
            }
            let principals = normalize_principal_entries(Some(effective_version), file.principals)?;
            // SEC-M-001: validate uniqueness unconditionally for ALL schema versions to prevent
            // order-dependent class resolution in resolve_bearer (last-match-wins with non-short-
            // circuit scan). This check is independent of schema_version.
            validate_no_duplicates(&principals)?;
            if effective_version >= 2 {
                validate_v2_principals(&principals)?;
            }
            // SEC-H-002: schema_version 3 must provide explicit surface_policies for every
            // principal. Entries without surface_policies would fall back to class-default
            // MCP capabilities, bypassing the boundary-aware explicit surface policy model.
            if effective_version >= 3 {
                validate_v3_principals(&principals)?;
            }
            Ok(PrincipalTable {
                entries: principals,
            })
        } else {
            // Bootstrap a default operator token; boundary-aware writers emit schema_version 3.
            // SEC-M-002: use cryptographically random 256-bit token rather than UUID (which has
            // fixed bits and structured format unsuitable for bearer credentials).
            let token = generate_bootstrap_token();
            let entry = default_operator_entry(token.clone());
            let file = PrincipalTableFile {
                schema_version: Some(3),
                principals: vec![entry.clone()],
            };
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).map_err(|e| {
                    AuthError::TableLoadFailed(format!("create dir {}: {e}", parent.display()))
                })?;
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700))
                        .map_err(|e| {
                            AuthError::TableLoadFailed(format!(
                                "chmod auth dir {} to 0700: {e}",
                                parent.display()
                            ))
                        })?;
                }
            }
            let json = serde_json::to_string_pretty(&file)
                .map_err(|e| AuthError::TableLoadFailed(format!("serialize: {e}")))?;
            #[cfg(unix)]
            {
                use std::io::Write;
                use std::os::unix::fs::OpenOptionsExt;
                // O_CREAT|O_EXCL: TOCTOU-safe; fails if a symlink was placed between
                // the path.exists() check and here.
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
            // Token is written to file only; never emitted to logs or diagnostics.
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

impl Default for PrincipalEntry {
    fn default() -> Self {
        PrincipalEntry {
            token: String::new(),
            id: String::new(),
            class: PrincipalClass::Operator,
            surface_policies: None,
            expires_at_ms: None,
            not_before_ms: None,
            disabled: None,
            caller_class_override: None,
            run_scope: None,
        }
    }
}

/// Generate a cryptographically random bearer token for bootstrapping.
/// Returns 43-char base64url (without padding) encoding of 32 random bytes = 256 bits entropy.
/// Satisfies P081 token format: 32..4096 visible ASCII, no CTL characters.
fn generate_bootstrap_token() -> String {
    use rand::RngCore;
    let mut bytes = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    base64_url_no_pad(&bytes)
}

fn base64_url_no_pad(bytes: &[u8]) -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::with_capacity((bytes.len() * 4 + 2) / 3);
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0] as usize;
        let b1 = if chunk.len() > 1 {
            chunk[1] as usize
        } else {
            0
        };
        let b2 = if chunk.len() > 2 {
            chunk[2] as usize
        } else {
            0
        };
        out.push(ALPHABET[b0 >> 2] as char);
        out.push(ALPHABET[((b0 & 3) << 4) | (b1 >> 4)] as char);
        if chunk.len() > 1 {
            out.push(ALPHABET[((b1 & 0xf) << 2) | (b2 >> 6)] as char);
        }
        if chunk.len() > 2 {
            out.push(ALPHABET[b2 & 0x3f] as char);
        }
    }
    out
}

fn default_operator_entry(token: String) -> PrincipalEntry {
    // P083 UI action boundary: the bootstrapped app bearer is limited to approval mutations.
    // P083 lifecycle commands (providerSessionShutdown, p083MarkProviderSessionProcessAbsent,
    // p083RollbackExecution, p083SetEnforcementMode) require an explicitly-configured principal.
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
        ..Default::default()
    }
}

fn approval_mutations() -> Vec<String> {
    vec!["approveApproval".into(), "rejectApproval".into()]
}

fn p083_lifecycle_mutations() -> Vec<String> {
    vec![
        "providerSessionShutdown".into(),
        "p083MarkProviderSessionProcessAbsent".into(),
        "p083RollbackExecution".into(),
        "p083SetEnforcementMode".into(),
    ]
}

fn operator_full_mutations() -> Vec<String> {
    let mut m = approval_mutations();
    m.extend(p083_lifecycle_mutations());
    m
}

fn is_exact_approval_mutation_set(mutations: &[String]) -> bool {
    let mut sorted = mutations.to_vec();
    sorted.sort();
    sorted == approval_mutations()
}

fn is_full_operator_mutation_set(mutations: &[String]) -> bool {
    let mut sorted = mutations.to_vec();
    sorted.sort();
    let mut expected = operator_full_mutations();
    expected.sort();
    sorted == expected
}

fn normalize_principal_entries(
    schema_version: Option<u32>,
    mut entries: Vec<PrincipalEntry>,
) -> Result<Vec<PrincipalEntry>, AuthError> {
    // v2 and v3 tables are already fully specified; no defaulting is applied.
    // v3 explicitly requires surface_policies on every entry (validated separately),
    // so injecting defaults here would bypass that fail-closed check.
    if schema_version == Some(2) || schema_version == Some(3) {
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

/// SEC-H-001: Check whether a principal entry is valid at the given Unix-ms timestamp.
/// Returns false (→ treat as UnknownToken) for expired, not-yet-valid, or disabled entries.
/// Non-disclosing: callers must return UnknownToken regardless of the failure reason.
fn is_entry_valid_at(entry: &PrincipalEntry, now_ms: i64) -> bool {
    if entry.disabled == Some(true) {
        return false;
    }
    if let Some(exp) = entry.expires_at_ms {
        if now_ms >= exp {
            return false;
        }
    }
    if let Some(nbf) = entry.not_before_ms {
        if now_ms < nbf {
            return false;
        }
    }
    true
}

pub fn resolve_bearer(token: &str, table: &PrincipalTable) -> Result<Principal, AuthError> {
    use sha2::{Digest, Sha256};
    use subtle::ConstantTimeEq;

    let now_ms = chrono::Utc::now().timestamp_millis();

    // SEC-P081-M004: scan ALL entries without short-circuiting to avoid leaking
    // the position of a matching token through timing. The constant-time comparison
    // prevents leaking whether a match occurred on iteration N vs iteration M.
    // SEC-H-001: only accept entries that are valid at the current time.
    let candidate_hash = Sha256::digest(token.as_bytes());
    let mut found: Option<Principal> = None;
    for entry in &table.entries {
        let stored_hash = Sha256::digest(entry.token.as_bytes());
        if bool::from(stored_hash.ct_eq(&candidate_hash)) && is_entry_valid_at(entry, now_ms) {
            found = Some(Principal::from_entry(entry));
        }
    }
    found.ok_or(AuthError::UnknownToken)
}

/// Non-secret stable fingerprint for comparing a connection credential against
/// a later-reloaded principal table without retaining or exposing the bearer.
pub fn token_fingerprint(token: &str) -> String {
    use sha2::{Digest, Sha256};

    let digest = Sha256::digest(token.as_bytes());
    let mut out = String::with_capacity("sha256:".len() + digest.len() * 2);
    out.push_str("sha256:");
    for byte in digest {
        use std::fmt::Write as _;
        let _ = write!(&mut out, "{byte:02x}");
    }
    out
}

pub fn principal_token_fingerprint_by_id(
    table: &PrincipalTable,
    principal_id: &str,
) -> Option<String> {
    table
        .entries
        .iter()
        .find(|entry| entry.id == principal_id)
        .map(|entry| token_fingerprint(&entry.token))
}

/// Extract bearer token from an Authorization header value.
/// Strict grammar: exactly "Bearer <token>" with one SP; no surrounding whitespace.
/// SEC-P081: token length 32..4096 bytes, visible ASCII (0x21-0x7e) only.
pub fn extract_bearer_token(header_value: &str) -> Result<&str, AuthError> {
    let token = header_value
        .strip_prefix("Bearer ")
        .ok_or(AuthError::MalformedHeader)?;
    // SEC-P081: length must be 32..=4096 bytes.
    if token.len() < 32 || token.len() > 4096 {
        return Err(AuthError::MalformedHeader);
    }
    // SEC-P081: all bytes must be visible ASCII (0x21-0x7e, no CTL, no space, no DEL).
    if !token.bytes().all(|b| (0x21..=0x7e).contains(&b)) {
        return Err(AuthError::MalformedHeader);
    }
    Ok(token)
}

/// ── P081 M-001: unconditional duplicate validation ───────────────────────────

/// Validate that all principal entries have unique ids and unique token values.
/// Applied unconditionally regardless of schema_version to prevent order-dependent
/// class resolution in resolve_bearer (last-match-wins with non-short-circuit scan).
fn validate_no_duplicates(entries: &[PrincipalEntry]) -> Result<(), AuthError> {
    use std::collections::HashSet;
    let mut ids = HashSet::new();
    for entry in entries {
        if !ids.insert(&entry.id) {
            return Err(AuthError::TableLoadFailed(format!(
                "duplicate principal id: {}",
                entry.id
            )));
        }
    }
    let mut tokens = HashSet::new();
    for entry in entries {
        if !tokens.insert(&entry.token) {
            return Err(AuthError::TableLoadFailed(
                "duplicate token in principal table".into(),
            ));
        }
    }
    Ok(())
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
            // SEC-P081: validate MCP tool names for ALL principals with MCP surface policies,
            // not just default-operator. A typo in any principal's allowed_tools must fail at
            // table load rather than silently producing zero capability for that principal.
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
            if !is_exact_approval_mutation_set(&graphql.allowed_mutations)
                && !is_full_operator_mutation_set(&graphql.allowed_mutations)
            {
                return Err(AuthError::TableLoadFailed(
                    "default-operator must allow either the approval mutation set or the full operator lifecycle mutation set".into(),
                ));
            }
            // MCP tool name validity is already checked in the surface_policies block above.
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

/// SEC-H-002: Validate that every schema_version 3 principal has explicit surface_policies.
/// A v3 principal without surface_policies would silently fall back to broad class-default
/// capabilities, bypassing the boundary-aware policy model.
fn validate_v3_principals(entries: &[PrincipalEntry]) -> Result<(), AuthError> {
    for entry in entries {
        if entry.surface_policies.is_none() {
            return Err(AuthError::TableLoadFailed(format!(
                "schema_version 3 principal '{}' (class {:?}) must have explicit surface_policies; \
                 principals without explicit transport policies are rejected in v3 to prevent \
                 unintended class-default capability inheritance",
                entry.id, entry.class
            )));
        }
    }
    Ok(())
}

/// P081: Derive a diagnostic-only token identifier for audit log correlation.
///
/// Computes `base32(sha256("p081-v1-token-id" || principal_id || token))[0..26]`
/// per the P081 security hardening contract — a 26-character RFC 4648 base32 string.
/// The salt and principal_id prevent precomputation and ensure different principals
/// with the same token produce distinct token_ids.
/// This is v1 compatibility only; the raw token is never stored or returned.
/// SEC-P081-M002: derived token_id stays in-process; never written to logs or wire.
pub fn derive_token_id(token: &str, principal_id: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(b"p081-v1-token-id");
    h.update(principal_id.as_bytes());
    h.update(token.as_bytes());
    let hash = h.finalize();
    base32_encode_truncated(&hash, 26)
}

/// RFC 4648 base32 encoding (uppercase A-Z, 2-7), truncated to `len` characters.
fn base32_encode_truncated(bytes: &[u8], len: usize) -> String {
    const ALPHA: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";
    let mut out = String::with_capacity(len);
    let mut bits: u32 = 0;
    let mut bit_count: u32 = 0;
    for &byte in bytes {
        bits = (bits << 8) | (byte as u32);
        bit_count += 8;
        while bit_count >= 5 {
            bit_count -= 5;
            out.push(ALPHA[((bits >> bit_count) & 0x1F) as usize] as char);
            if out.len() == len {
                return out;
            }
        }
    }
    if out.len() < len && bit_count > 0 {
        out.push(ALPHA[((bits << (5 - bit_count)) & 0x1F) as usize] as char);
    }
    out
}

/// P081 Phase 2: Resolve the CallerClass for a bearer token.
/// Returns None if the token is not found or the entry is expired/disabled.
pub fn resolve_caller_class_for_token(table: &PrincipalTable, token: &str) -> Option<CallerClass> {
    use sha2::{Digest, Sha256};
    use subtle::ConstantTimeEq;

    let now_ms = chrono::Utc::now().timestamp_millis();

    // SEC-P081-M004: scan ALL entries without short-circuiting (same as resolve_bearer).
    // SEC-H-001: only return a class for valid (non-expired, non-disabled) entries.
    let candidate_hash = Sha256::digest(token.as_bytes());
    let mut found: Option<CallerClass> = None;
    for entry in &table.entries {
        let stored_hash = Sha256::digest(entry.token.as_bytes());
        if bool::from(stored_hash.ct_eq(&candidate_hash)) && is_entry_valid_at(entry, now_ms) {
            let principal = Principal::from_entry(entry);
            found = Some(derive_caller_class(&principal));
        }
    }
    found
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
pub fn is_mutation_allowed_by_principal_surface_policy(
    principal: &Principal,
    mutation_name: &str,
) -> Option<bool> {
    if !principal.has_explicit_surface_policies {
        return None;
    }
    Some(
        principal
            .graphql_policy
            .as_ref()
            .is_some_and(|graphql| graphql.allowed_mutations.iter().any(|m| m == mutation_name)),
    )
}

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
pub fn is_query_allowed_by_principal_surface_policy(principal: &Principal) -> Option<bool> {
    if !principal.has_explicit_surface_policies {
        return None;
    }
    Some(
        principal
            .graphql_policy
            .as_ref()
            .is_some_and(|graphql| graphql.allow_queries),
    )
}

pub fn is_query_allowed_by_surface_policy(
    table: &PrincipalTable,
    principal_id: &str,
) -> Option<bool> {
    table
        .entries
        .iter()
        .find(|e| e.id == principal_id)
        .and_then(|e| e.surface_policies.as_ref())
        .and_then(|sp| sp.graphql.as_ref())
        .map(|graphql| graphql.allow_queries)
}

/// P072: Check if GraphQL subscriptions are allowed for a principal based on v2 surface_policies.
/// Returns None if the principal has no surface_policies (v1 behavior applies).
pub fn is_subscription_allowed_by_principal_surface_policy(principal: &Principal) -> Option<bool> {
    if !principal.has_explicit_surface_policies {
        return None;
    }
    Some(
        principal
            .graphql_policy
            .as_ref()
            .is_some_and(|graphql| graphql.allow_subscriptions),
    )
}

pub fn is_subscription_allowed_by_surface_policy(
    table: &PrincipalTable,
    principal_id: &str,
) -> Option<bool> {
    table
        .entries
        .iter()
        .find(|e| e.id == principal_id)
        .and_then(|e| e.surface_policies.as_ref())
        .and_then(|sp| sp.graphql.as_ref())
        .map(|graphql| graphql.allow_subscriptions)
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

fn all_tool_capabilities() -> [CapabilityToolId; 54] {
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
        CapabilityToolId::RunsRetrofitCatalogSnapshot,
        CapabilityToolId::RunsCancel,
        CapabilityToolId::ApprovalsList,
        CapabilityToolId::ApprovalsResolve,
        CapabilityToolId::StagesRetry,
        CapabilityToolId::StagesConsumeProviderQuotaHold,
        CapabilityToolId::WorkflowConflictsResolve,
        CapabilityToolId::WorkflowLoopBudgetExtend,
        CapabilityToolId::LegacyDiscoveryOverrideCreate,
        CapabilityToolId::ReportsGet,
        CapabilityToolId::ArtifactsOverrideContract,
        CapabilityToolId::StewardRunAnalysis,
        CapabilityToolId::StewardListAnalyses,
        CapabilityToolId::StewardGetAnalysis,
        CapabilityToolId::StorageHealth,
        CapabilityToolId::RuntimeHealth,
        CapabilityToolId::OperatorAlertsList,
        CapabilityToolId::StorageWritePressure,
        CapabilityToolId::StorageEvidenceSpoolSummary,
        CapabilityToolId::StorageReconcileEvidenceOrphans,
        CapabilityToolId::ProposalGateSettle,
        // P078: durable side-effect ledger tools (Operator-only).
        CapabilityToolId::EffectsList,
        CapabilityToolId::EffectsInspect,
        CapabilityToolId::EffectsReconcile,
        CapabilityToolId::EffectsMarkConflict,
        CapabilityToolId::EffectsMarkUnrecoverable,
        CapabilityToolId::EffectsClearAfterManualVerification,
        // P087: storage admin tools (Operator-only).
        CapabilityToolId::StorageMaintenanceRepairSlot,
        CapabilityToolId::StorageProjectionsClearBacklog,
        CapabilityToolId::StorageProjectionsClearPoison,
        CapabilityToolId::AgentsContinuationStatus,
        CapabilityToolId::AgentsContinuationCandidates,
        CapabilityToolId::AgentsContinueWork,
        CapabilityToolId::AutomationAutoRetryLatest,
        CapabilityToolId::P080DiagnosticsGet,
        CapabilityToolId::P080ReconcileRequest,
        CapabilityToolId::P080ClearPermanentHold,
        // P083: provider session and enforcement mode lifecycle tools (Operator-only).
        CapabilityToolId::ProviderSessionShutdown,
        CapabilityToolId::ProviderSessionMarkProcessAbsent,
        CapabilityToolId::P083RollbackExecution,
        CapabilityToolId::P083SetEnforcementMode,
        CapabilityToolId::RetryRun,
        CapabilityToolId::SideEffectsForceReconcile,
    ]
}

fn tool_allowed_for_class(class: &PrincipalClass, id: CapabilityToolId) -> bool {
    match id {
        CapabilityToolId::IdeasCreate => {
            matches!(class, PrincipalClass::Operator | PrincipalClass::Agent)
        }
        // SEC-P080-001: ReadOnlyOperator is scoped to P080 diagnostic tools only.
        // General read tools (ideas.list, runs.list, etc.) remain Operator/Agent/Observer only.
        CapabilityToolId::IdeasList => {
            matches!(
                class,
                PrincipalClass::Operator | PrincipalClass::Agent | PrincipalClass::Observer
            )
        }
        // SEC-001: runs.start supplies daemon-side filesystem paths — Operator-only.
        CapabilityToolId::RunsStart => {
            // SEC-001: runs.start supplies daemon-side filesystem paths — Operator-only.
            matches!(class, PrincipalClass::Operator)
        }
        CapabilityToolId::RunsList => {
            matches!(
                class,
                PrincipalClass::Operator | PrincipalClass::Agent | PrincipalClass::Observer
            )
        }
        CapabilityToolId::RunsGet => {
            matches!(
                class,
                PrincipalClass::Operator | PrincipalClass::Agent | PrincipalClass::Observer
            )
        }
        CapabilityToolId::RunsMainSyncRequest => matches!(class, PrincipalClass::Operator),
        CapabilityToolId::RunsMainSyncRetry => matches!(class, PrincipalClass::Operator),
        CapabilityToolId::RunsMainSyncSetOverride => matches!(class, PrincipalClass::Operator),
        CapabilityToolId::RunsMainSyncRepairState => matches!(class, PrincipalClass::Operator),
        CapabilityToolId::RunsMainSyncRecordRecoveryDecision => {
            matches!(class, PrincipalClass::Operator)
        }
        CapabilityToolId::RunsKnowledgeCapsuleIgnore => matches!(class, PrincipalClass::Operator),
        CapabilityToolId::RunsRetrofitCatalogSnapshot => matches!(class, PrincipalClass::Operator),
        CapabilityToolId::RunsCancel => matches!(class, PrincipalClass::Operator),
        CapabilityToolId::ApprovalsList => {
            matches!(class, PrincipalClass::Operator | PrincipalClass::Observer)
        }
        CapabilityToolId::ApprovalsResolve => matches!(class, PrincipalClass::Operator),
        CapabilityToolId::StagesRetry => matches!(class, PrincipalClass::Operator),
        CapabilityToolId::StagesConsumeProviderQuotaHold => {
            matches!(class, PrincipalClass::Operator)
        }
        CapabilityToolId::WorkflowConflictsResolve => matches!(class, PrincipalClass::Operator),
        CapabilityToolId::WorkflowLoopBudgetExtend => matches!(class, PrincipalClass::Operator),
        CapabilityToolId::LegacyDiscoveryOverrideCreate => {
            matches!(class, PrincipalClass::Operator)
        }
        // SEC-HIGH-001: reports.get returns operator-sensitive payloads (rollout readback,
        // retry authority history, implementation summaries, canonical artifact contracts).
        // Restrict to Operator only; Agent and Observer must not see these.
        CapabilityToolId::ReportsGet => {
            matches!(class, PrincipalClass::Operator)
        }
        CapabilityToolId::ArtifactsOverrideContract => matches!(class, PrincipalClass::Operator),
        CapabilityToolId::StewardRunAnalysis => matches!(class, PrincipalClass::Operator),
        CapabilityToolId::StewardListAnalyses => {
            matches!(class, PrincipalClass::Operator | PrincipalClass::Observer)
        }
        CapabilityToolId::StewardGetAnalysis => {
            matches!(class, PrincipalClass::Operator | PrincipalClass::Observer)
        }
        // SEC-004: storage diagnostics expose WAL, queue pressure, orphan counts, and
        // kill-switch state — restrict to Operator to match the GraphQL storageHealth boundary.
        CapabilityToolId::StorageHealth => {
            matches!(class, PrincipalClass::Operator)
        }
        CapabilityToolId::RuntimeHealth => matches!(class, PrincipalClass::Operator),
        CapabilityToolId::OperatorAlertsList => matches!(class, PrincipalClass::Operator),
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
        // P078: durable side-effect ledger tools (Operator-only).
        CapabilityToolId::EffectsList => matches!(class, PrincipalClass::Operator),
        CapabilityToolId::EffectsInspect => matches!(class, PrincipalClass::Operator),
        CapabilityToolId::EffectsReconcile => matches!(class, PrincipalClass::Operator),
        CapabilityToolId::EffectsMarkConflict => matches!(class, PrincipalClass::Operator),
        CapabilityToolId::EffectsMarkUnrecoverable => matches!(class, PrincipalClass::Operator),
        CapabilityToolId::EffectsClearAfterManualVerification => {
            matches!(class, PrincipalClass::Operator)
        }
        // P087: storage admin tools (Operator-only).
        CapabilityToolId::StorageMaintenanceRepairSlot => matches!(class, PrincipalClass::Operator),
        CapabilityToolId::StorageProjectionsClearBacklog => {
            matches!(class, PrincipalClass::Operator)
        }
        CapabilityToolId::StorageProjectionsClearPoison => {
            matches!(class, PrincipalClass::Operator)
        }
        // P086: read-only continuation queries are Operator+Observer.
        // continue_work is Operator for manual requests and Agent only for
        // lead_auto requests; the handler enforces trigger-specific authority.
        CapabilityToolId::AgentsContinuationStatus => {
            matches!(class, PrincipalClass::Operator | PrincipalClass::Observer)
        }
        CapabilityToolId::AgentsContinuationCandidates => {
            matches!(class, PrincipalClass::Operator | PrincipalClass::Observer)
        }
        CapabilityToolId::AgentsContinueWork => {
            matches!(class, PrincipalClass::Operator | PrincipalClass::Agent)
        }
        CapabilityToolId::AutomationAutoRetryLatest => {
            matches!(class, PrincipalClass::Operator | PrincipalClass::Observer)
        }
        // P080: diagnostics.get and reconcile.request (diagnose_only) are allowed for
        // ReadOnlyOperator per approved proposal §3.1 auth matrix (lines 145-153).
        // repair_if_safe is gated at the handler level for Operator class only.
        // clear_permanent_hold is Phase 5+ and Operator-only.
        CapabilityToolId::P080DiagnosticsGet => {
            matches!(
                class,
                PrincipalClass::Operator | PrincipalClass::ReadOnlyOperator
            )
        }
        CapabilityToolId::P080ReconcileRequest => {
            matches!(
                class,
                PrincipalClass::Operator | PrincipalClass::ReadOnlyOperator
            )
        }
        CapabilityToolId::P080ClearPermanentHold => matches!(class, PrincipalClass::Operator),
        // P083: lifecycle mutations require Operator principal.
        CapabilityToolId::ProviderSessionShutdown => matches!(class, PrincipalClass::Operator),
        CapabilityToolId::ProviderSessionMarkProcessAbsent => {
            matches!(class, PrincipalClass::Operator)
        }
        CapabilityToolId::P083RollbackExecution => matches!(class, PrincipalClass::Operator),
        CapabilityToolId::P083SetEnforcementMode => matches!(class, PrincipalClass::Operator),
        CapabilityToolId::RetryRun => matches!(class, PrincipalClass::Operator),
        CapabilityToolId::SideEffectsForceReconcile => matches!(class, PrincipalClass::Operator),
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
    // SEC-P080-001: ReadOnlyOperator has no default MCP resource capabilities.
    // P080 diagnostic tools (diagnostics.get, reconcile.request) do not expose
    // MCP resources — they are tool-only. A ReadOnlyOperator bearer must not
    // inherit run://, idea://, artifact://, report://, or any index resource
    // from the generic default matrix, or it becomes a privacy boundary bypass.
    if matches!(class, PrincipalClass::ReadOnlyOperator) {
        return false;
    }
    match id {
        ResourceTemplateId::RunEntity => true,
        ResourceTemplateId::IdeaEntity => true,
        // SEC-HIGH-001: artifact:// and report:// expose sensitive operator payloads
        // (file_path, evidence, rollout readback). Restrict to Operator only so
        // Agent and Observer principals cannot read artifact content or reports.
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
        "runs.retrofit_catalog_snapshot" => Some(CapabilityToolId::RunsRetrofitCatalogSnapshot),
        "runs.cancel" => Some(CapabilityToolId::RunsCancel),
        "approvals.list" => Some(CapabilityToolId::ApprovalsList),
        "approvals.resolve" => Some(CapabilityToolId::ApprovalsResolve),
        "stages.retry" => Some(CapabilityToolId::StagesRetry),
        "stages.consume_provider_quota_hold" => {
            Some(CapabilityToolId::StagesConsumeProviderQuotaHold)
        }
        "workflow_conflicts.resolve" => Some(CapabilityToolId::WorkflowConflictsResolve),
        "legacy_discovery_override_create" => Some(CapabilityToolId::LegacyDiscoveryOverrideCreate),
        "reports.get" => Some(CapabilityToolId::ReportsGet),
        "artifacts.override_contract" => Some(CapabilityToolId::ArtifactsOverrideContract),
        "steward.run_analysis" => Some(CapabilityToolId::StewardRunAnalysis),
        "steward.list_analyses" => Some(CapabilityToolId::StewardListAnalyses),
        "steward.get_analysis" => Some(CapabilityToolId::StewardGetAnalysis),
        "storage.health" => Some(CapabilityToolId::StorageHealth),
        "runtime.health" | "boundary.runtime.get" => Some(CapabilityToolId::RuntimeHealth),
        "storage.write_pressure" => Some(CapabilityToolId::StorageWritePressure),
        "storage.evidence_spool_summary" => Some(CapabilityToolId::StorageEvidenceSpoolSummary),
        "storage.reconcile_evidence_orphans" => {
            Some(CapabilityToolId::StorageReconcileEvidenceOrphans)
        }
        "runs.settle_proposal_gate" => Some(CapabilityToolId::ProposalGateSettle),
        // P078: durable side-effect ledger tools.
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
        "agents.continuation_status" => Some(CapabilityToolId::AgentsContinuationStatus),
        "agents.continuation_candidates" => Some(CapabilityToolId::AgentsContinuationCandidates),
        "agents.continue_work" => Some(CapabilityToolId::AgentsContinueWork),
        "automation.auto_retry.latest" => Some(CapabilityToolId::AutomationAutoRetryLatest),
        "p080.diagnostics.get.v1" => Some(CapabilityToolId::P080DiagnosticsGet),
        "p080.reconcile.request.v1" => Some(CapabilityToolId::P080ReconcileRequest),
        "p080.clear_permanent_hold.v1" => Some(CapabilityToolId::P080ClearPermanentHold),
        // P083 lifecycle tools.
        "provider_session.shutdown" => Some(CapabilityToolId::ProviderSessionShutdown),
        "provider_session.mark_process_absent" => {
            Some(CapabilityToolId::ProviderSessionMarkProcessAbsent)
        }
        "p083.rollback_execution" => Some(CapabilityToolId::P083RollbackExecution),
        "p083.set_enforcement_mode" => Some(CapabilityToolId::P083SetEnforcementMode),
        "runs.retry" => Some(CapabilityToolId::RetryRun),
        "side_effects.force_reconcile" => Some(CapabilityToolId::SideEffectsForceReconcile),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::{CapabilityToolId, ResourceTemplateId};

    fn secure_principal_table_file(contents: &str) -> (tempfile::TempDir, std::path::PathBuf) {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().unwrap();
        std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
        let path = dir.path().join("principals.json");
        std::fs::write(&path, contents).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        (dir, path)
    }

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
        // SEC-001: runs.start supplies daemon-side filesystem paths; Operator-only.
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
        // SEC-HIGH-001: reports.get contains operator-sensitive payloads; Observer must be denied.
        assert!(!is_tool_allowed(&p, "reports.get"));
        assert!(!is_tool_allowed(&p, "ideas.create"));
        assert!(!is_tool_allowed(&p, "runs.start"));
    }

    #[test]
    fn reports_get_operator_only() {
        // SEC-HIGH-001: reports.get restricted to Operator only.
        let op = Principal::new("op", PrincipalClass::Operator);
        let ag = Principal::new("ag", PrincipalClass::Agent);
        let ob = Principal::new("ob", PrincipalClass::Observer);
        assert!(is_tool_allowed(&op, "reports.get"));
        assert!(!is_tool_allowed(&ag, "reports.get"));
        assert!(!is_tool_allowed(&ob, "reports.get"));
    }

    #[test]
    fn resolve_bearer_works() {
        let table = PrincipalTable {
            entries: vec![PrincipalEntry {
                token: "tok-123".into(),
                id: "test-op".into(),
                class: PrincipalClass::Operator,
                surface_policies: None,
                ..Default::default()
            }],
        };
        let p = resolve_bearer("tok-123", &table).unwrap();
        assert_eq!(p.id, "test-op");
        assert_eq!(p.class, PrincipalClass::Operator);
        assert!(resolve_bearer("bad-token", &table).is_err());
    }

    #[test]
    fn extract_bearer_token_works() {
        // SEC-P081: token must be 32..=4096 bytes of visible ASCII (0x21-0x7e).
        let valid_token = "a".repeat(32);
        let header = format!("Bearer {valid_token}");
        assert_eq!(extract_bearer_token(&header).unwrap(), valid_token);

        // Wrong scheme.
        assert!(extract_bearer_token("Basic abc123").is_err());
        // Empty token.
        assert!(extract_bearer_token("Bearer ").is_err());
        // Too short (< 32 bytes).
        assert!(extract_bearer_token("Bearer abc123").is_err());
        // Too long (> 4096 bytes).
        let long_token = format!("Bearer {}", "a".repeat(4097));
        assert!(extract_bearer_token(&long_token).is_err());
        // Contains space (0x20, not visible ASCII).
        let space_token = format!("Bearer {}a b{}", "a".repeat(15), "a".repeat(14));
        assert!(extract_bearer_token(&space_token).is_err());
        // Contains CTL character (0x01).
        let mut ctl_token = "a".repeat(32);
        ctl_token.push('\x01');
        assert!(extract_bearer_token(&format!("Bearer {ctl_token}")).is_err());
        // Strict grammar: leading whitespace rejected.
        assert!(extract_bearer_token(&format!("  Bearer {valid_token}")).is_err());
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
    fn p086_agents_can_reach_continue_work_handler_for_lead_auto_validation() {
        let agent = Principal::new("agent-p086", PrincipalClass::Agent);
        let observer = Principal::new("observer-p086", PrincipalClass::Observer);
        assert!(is_tool_allowed(&agent, "agents.continue_work"));
        assert!(!is_tool_allowed(&observer, "agents.continue_work"));
    }

    #[test]
    fn observer_has_all_read_resources() {
        let ob = Principal::new("ob", PrincipalClass::Observer);
        assert!(is_resource_allowed(&ob, ResourceTemplateId::RunEntity));
        assert!(is_resource_allowed(&ob, ResourceTemplateId::IdeaEntity));
        // SEC-HIGH-001: artifact:// and report:// expose operator-sensitive payloads
        // (file_path, evidence, rollout readback). Restricted to Operator only.
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
        assert!(is_resource_allowed(
            &ob,
            ResourceTemplateId::ChainworksRunArtifacts
        ));
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
                ..Default::default()
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
                ..Default::default()
            },
        ];
        assert!(validate_v2_principals(&entries).is_ok());
    }

    #[test]
    fn legacy_default_operator_file_is_normalized_to_p072_ui_policy() {
        let (_dir, path) = secure_principal_table_file(
            r#"{
              "principals": [
                {
                  "token": "legacy-token",
                  "id": "default-operator",
                  "class": "operator"
                }
              ]
            }"#,
        );

        let table = PrincipalTable::load_or_bootstrap(&path).unwrap();

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
                ..Default::default()
            },
            PrincipalEntry {
                token: "tok-2".into(),
                id: "same-id".into(),
                class: PrincipalClass::Operator,
                surface_policies: None,
                ..Default::default()
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
                ..Default::default()
            },
            PrincipalEntry {
                token: "same-tok".into(),
                id: "id-2".into(),
                class: PrincipalClass::Operator,
                surface_policies: None,
                ..Default::default()
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
            ..Default::default()
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
            ..Default::default()
        }];
        let err = validate_v2_principals(&entries).unwrap_err();
        assert!(err
            .to_string()
            .contains("default-operator must allow either the approval mutation set"));
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
            ..Default::default()
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
            ..Default::default()
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
                    ..Default::default()
                },
                PrincipalEntry {
                    token: "tok-2".into(),
                    id: "ui_operator".into(),
                    class: PrincipalClass::Operator,
                    surface_policies: None,
                    ..Default::default()
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

                    ..Default::default()
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

                    ..Default::default()
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

                    ..Default::default()
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

                    ..Default::default()
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
            ..Default::default()
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

                    ..Default::default()
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

                    ..Default::default()
                },
                PrincipalEntry {
                    token: "tok-v1".into(),
                    id: "v1-operator".into(),
                    class: PrincipalClass::Operator,
                    surface_policies: None,
                    ..Default::default()
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

    // ── P081 Phase 2: CallerClass derivation tests ──────────────────────

    #[test]
    fn caller_class_derives_ui_operator_from_operator_principal() {
        let p = Principal::new("op", PrincipalClass::Operator);
        assert_eq!(derive_caller_class(&p), CallerClass::UiOperator);
        assert_eq!(CallerClass::UiOperator.as_str(), "ui_operator");
    }

    #[test]
    fn caller_class_derives_agent_operator_from_agent_principal() {
        let p = Principal::new("ag", PrincipalClass::Agent);
        assert_eq!(derive_caller_class(&p), CallerClass::AgentOperator);
        assert_eq!(CallerClass::AgentOperator.as_str(), "agent_operator");
    }

    #[test]
    fn caller_class_derives_observer_from_observer_principal() {
        let p = Principal::new("ob", PrincipalClass::Observer);
        assert_eq!(derive_caller_class(&p), CallerClass::Observer);
        assert_eq!(CallerClass::Observer.as_str(), "observer");
    }

    #[test]
    fn caller_class_resolve_for_token_derives_correctly() {
        // NOTE: This test constructs PrincipalTable directly, bypassing
        // load_or_bootstrap and the SEC-H-002 validate_v3_principals check.
        // That is intentional: this is a unit test for resolve_caller_class_for_token
        // (token resolution), not for file-level validation. Production code always
        // goes through load_or_bootstrap which enforces the surface_policies requirement
        // for schema_version 3 principals.
        let table = PrincipalTable {
            entries: vec![
                PrincipalEntry {
                    token: "tok-op".into(),
                    id: "default-operator".into(),
                    class: PrincipalClass::Operator,
                    surface_policies: None,
                    ..Default::default()
                },
                PrincipalEntry {
                    token: "tok-ag".into(),
                    id: "default-agent".into(),
                    class: PrincipalClass::Agent,
                    surface_policies: None,
                    ..Default::default()
                },
                PrincipalEntry {
                    token: "tok-ob".into(),
                    id: "default-observer".into(),
                    class: PrincipalClass::Observer,
                    surface_policies: None,
                    ..Default::default()
                },
            ],
        };
        assert_eq!(
            resolve_caller_class_for_token(&table, "tok-op"),
            Some(CallerClass::UiOperator)
        );
        assert_eq!(
            resolve_caller_class_for_token(&table, "tok-ag"),
            Some(CallerClass::AgentOperator)
        );
        assert_eq!(
            resolve_caller_class_for_token(&table, "tok-ob"),
            Some(CallerClass::Observer)
        );
        assert_eq!(
            resolve_caller_class_for_token(&table, "unknown-token"),
            None
        );
    }

    #[test]
    fn v3_principal_table_rejects_unknown_schema_version() {
        // Verify both an out-of-range high version and version 0 are rejected.
        for bad_version in [0u32, 99u32] {
            let (_dir, path) = secure_principal_table_file(&format!(
                r#"{{"schema_version": {bad_version}, "principals": [{{"token": "t", "id": "i", "class": "operator"}}]}}"#
            ));
            let err = PrincipalTable::load_or_bootstrap(&path).unwrap_err();
            assert!(
                err.to_string().contains("unsupported schema_version")
                    || err.to_string().contains("unknown schema_version"),
                "schema_version {bad_version} should be rejected; got: {err}"
            );
        }
    }

    #[test]
    fn v3_principal_table_derives_caller_class_not_stored() {
        // CallerClass is server-derived; principal entries must not store an
        // explicit caller_class field (deny_unknown_fields enforces this).
        let (_dir, path) = secure_principal_table_file(
            r#"{
              "schema_version": 3,
              "principals": [
                {
                  "token": "tok-auto-v3",
                  "id": "auto-agent",
                  "class": "agent",
                  "caller_class": "automation"
                }
              ]
            }"#,
        );
        let err = PrincipalTable::load_or_bootstrap(&path).unwrap_err();
        assert!(
            err.to_string().contains("caller_class") || err.to_string().contains("unknown field"),
            "principal entry with caller_class must be rejected as unknown field, got: {err}"
        );
    }

    #[test]
    fn v3_principal_without_surface_policies_is_rejected() {
        // SEC-H-002: schema_version 3 must provide explicit surface_policies.
        // A v3 entry without surface_policies is rejected to prevent class-default
        // capability inheritance (fail-closed by design).
        let (_dir, path) = secure_principal_table_file(
            r#"{
              "schema_version": 3,
              "principals": [
                {
                  "token": "tok-agent-v3",
                  "id": "agent-v3",
                  "class": "agent"
                }
              ]
            }"#,
        );
        let err = PrincipalTable::load_or_bootstrap(&path).unwrap_err();
        assert!(
            err.to_string().contains("surface_policies"),
            "v3 principal without surface_policies must be rejected (SEC-H-002); got: {err}"
        );
    }

    #[test]
    fn v3_principal_with_explicit_surface_policies_loads_as_agent_operator() {
        // SEC-H-002: v3 principal WITH explicit surface_policies succeeds and derives CallerClass.
        let (_dir, path) = secure_principal_table_file(
            r#"{
              "schema_version": 3,
              "principals": [
                {
                  "token": "tok-agent-v3",
                  "id": "agent-v3",
                  "class": "agent",
                  "surface_policies": {
                    "mcp": { "allowed_tools": [] }
                  }
                }
              ]
            }"#,
        );
        let table = PrincipalTable::load_or_bootstrap(&path).unwrap();
        assert_eq!(
            resolve_caller_class_for_token(&table, "tok-agent-v3"),
            Some(CallerClass::AgentOperator),
            "agent principal with explicit surface_policies derives to agent_operator"
        );
    }

    #[test]
    fn bootstrap_emits_schema_version_3() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("principals.json");
        let _ = PrincipalTable::load_or_bootstrap(&path).unwrap();
        let content = std::fs::read_to_string(&path).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert_eq!(
            parsed["schema_version"].as_u64(),
            Some(3),
            "bootstrap writer must emit schema_version 3 (boundary-aware writers emit v3 only)"
        );
    }

    /// SEC-P083-LOW-001: bootstrapped default-operator uses approval-only mutations (UI boundary).
    /// P083 lifecycle mutations require an explicitly-configured principal, not the app bearer.
    /// Also verifies that a principals.json with the full operator mutation set still loads (compat).
    #[test]
    fn p083_bootstrap_uses_approval_only_and_full_set_still_loads() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("principals.json");
        // Bootstrap writes a schema_version 3 file.
        let table1 = PrincipalTable::load_or_bootstrap(&path).unwrap();
        assert_eq!(
            table1.entries.len(),
            1,
            "bootstrap must create one default-operator entry"
        );
        // Reload must succeed.
        let table2 = PrincipalTable::load_or_bootstrap(&path).unwrap();
        assert_eq!(
            table2.entries.len(),
            1,
            "reload must succeed and return the same entry"
        );
        let entry = &table2.entries[0];
        let mutations = entry
            .surface_policies
            .as_ref()
            .and_then(|p| p.graphql.as_ref())
            .map(|g| g.allowed_mutations.clone())
            .unwrap_or_default();
        // P083 UI action boundary: bootstrap produces approval-only mutations.
        let mut sorted = mutations.clone();
        sorted.sort();
        assert_eq!(
            sorted,
            vec!["approveApproval", "rejectApproval"],
            "bootstrapped default-operator must have approval-only mutations; got {mutations:?}"
        );
        assert!(
            !mutations.contains(&"providerSessionShutdown".to_string()),
            "bootstrapped default-operator must NOT include p083 lifecycle mutations; got {mutations:?}"
        );

        // Backward-compat: a manually-crafted v3 file with full operator mutations must still load.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let dir2 = tempfile::tempdir().unwrap();
            std::fs::set_permissions(dir2.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
            let path2 = dir2.path().join("principals.json");
            // Use a raw string to avoid format brace escaping issues.
            let full_set_json = r#"{"schema_version":3,"principals":[{"token":"tok-operator-compat-xxxxxxxxxxxxxxxx","id":"default-operator","class":"operator","surface_policies":{"graphql":{"allow_queries":true,"allow_subscriptions":true,"allowed_mutations":["approveApproval","rejectApproval","providerSessionShutdown","p083MarkProviderSessionProcessAbsent","p083RollbackExecution","p083SetEnforcementMode"]},"mcp":{"allowed_tools":[]}}}]}"#;
            std::fs::write(&path2, full_set_json).unwrap();
            std::fs::set_permissions(&path2, std::fs::Permissions::from_mode(0o600)).unwrap();
            let table3 = PrincipalTable::load_or_bootstrap(&path2).unwrap();
            assert_eq!(
                table3.entries.len(),
                1,
                "full-operator-set principals.json must load"
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn p081_principals_file_rejects_hard_links_and_non_private_parent_dir() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().unwrap();
        std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
        let path = dir.path().join("principals.json");
        std::fs::write(
            &path,
            r#"{
              "schema_version": 3,
              "principals": [
                {
                  "token": "tok-operator-v3-xxxxxxxxxxxxxxxx",
                  "id": "operator-v3",
                  "class": "operator",
                  "surface_policies": {
                    "graphql": { "allowed_operations": ["query", "subscription"] },
                    "mcp": { "allowed_tools": ["runtime.health"] }
                  }
                }
              ]
            }"#,
        )
        .unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        let hard_link = dir.path().join("principals-hardlink.json");
        std::fs::hard_link(&path, &hard_link).unwrap();

        let err = PrincipalTable::load_or_bootstrap(&path).unwrap_err();
        assert!(
            err.to_string().contains("hard-linked"),
            "hard-linked principals.json must fail closed, got {err}"
        );

        std::fs::remove_file(hard_link).unwrap();
        let public_dir = tempfile::tempdir().unwrap();
        std::fs::set_permissions(public_dir.path(), std::fs::Permissions::from_mode(0o755))
            .unwrap();
        let public_path = public_dir.path().join("principals.json");
        std::fs::copy(&path, &public_path).unwrap();
        std::fs::set_permissions(&public_path, std::fs::Permissions::from_mode(0o600)).unwrap();

        let err = PrincipalTable::load_or_bootstrap(&public_path).unwrap_err();
        assert!(
            err.to_string()
                .contains("parent directory must have mode 0700"),
            "public auth dir must fail closed, got {err}"
        );
    }

    #[test]
    fn caller_class_display_matches_as_str() {
        for cc in [
            CallerClass::UiOperator,
            CallerClass::AgentOperator,
            CallerClass::Automation,
            CallerClass::Observer,
            CallerClass::DeveloperBreakGlass,
        ] {
            assert_eq!(cc.to_string(), cc.as_str());
        }
    }

    // ── P081 Phase 3: MCP transport caller class derivation ─────────────

    /// Operator principals connecting via MCP must derive as agent_operator.
    /// The ui_operator class is reserved for the Swift app via GraphQL.
    #[test]
    fn derive_caller_class_for_mcp_maps_operator_to_agent_operator() {
        assert_eq!(
            derive_caller_class_for_mcp(&Principal::new("op", PrincipalClass::Operator)),
            CallerClass::AgentOperator,
            "Operator on MCP must be agent_operator (MCP is the agent control plane)"
        );
        assert_eq!(
            derive_caller_class_for_mcp(&Principal::new("ag", PrincipalClass::Agent)),
            CallerClass::AgentOperator
        );
        assert_eq!(
            derive_caller_class_for_mcp(&Principal::new("ob", PrincipalClass::Observer)),
            CallerClass::Observer
        );
    }

    #[test]
    fn derive_caller_class_for_mcp_respects_override() {
        let mut automation = Principal::new("auto-agent", PrincipalClass::Agent);
        automation.caller_class_override = Some(CallerClass::Automation);
        assert_eq!(
            derive_caller_class_for_mcp(&automation),
            CallerClass::Automation,
            "explicit override must take precedence over PrincipalClass derivation"
        );

        let mut bg = Principal::new("bg-op", PrincipalClass::Operator);
        bg.caller_class_override = Some(CallerClass::DeveloperBreakGlass);
        assert_eq!(
            derive_caller_class_for_mcp(&bg),
            CallerClass::DeveloperBreakGlass
        );
    }

    #[test]
    fn derive_caller_class_respects_override() {
        let mut automation = Principal::new("auto-agent", PrincipalClass::Agent);
        automation.caller_class_override = Some(CallerClass::Automation);
        assert_eq!(
            derive_caller_class(&automation),
            CallerClass::Automation,
            "explicit override must take precedence in GraphQL caller derivation"
        );
    }

    // ── P078 Effects* capability regression ─────────────────────────────

    /// P078 effects.* tools must remain Operator-only and recognized by name lookup.
    #[test]
    fn p078_effects_tools_are_operator_only() {
        let op = Principal::new("op", PrincipalClass::Operator);
        let ag = Principal::new("ag", PrincipalClass::Agent);
        let ob = Principal::new("ob", PrincipalClass::Observer);

        for tool in [
            "effects.list",
            "effects.inspect",
            "effects.reconcile",
            "effects.mark_conflict",
            "effects.mark_unrecoverable",
            "effects.clear_after_manual_verification",
        ] {
            assert!(
                is_tool_allowed(&op, tool),
                "Operator must have {tool} (P078 effects ledger)"
            );
            assert!(
                !is_tool_allowed(&ag, tool),
                "Agent must NOT have {tool} (P078 Operator-only)"
            );
            assert!(
                !is_tool_allowed(&ob, tool),
                "Observer must NOT have {tool} (P078 Operator-only)"
            );
        }
    }

    // ── HIGH-002 bootstrap log redaction ────────────────────────────────

    /// HIGH-002: the bootstrap bearer token must never appear in log output.
    /// We capture the tracing subscriber output on the current thread and
    /// confirm the randomly-generated token value is absent.
    #[test]
    fn high_002_bootstrap_log_does_not_contain_token() {
        use std::sync::{Arc, Mutex};

        let log_buf: Arc<Mutex<Vec<u8>>> = Arc::new(Mutex::new(Vec::new()));
        let log_buf_writer = log_buf.clone();

        struct BufWriter(Arc<Mutex<Vec<u8>>>);
        impl std::io::Write for BufWriter {
            fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
                self.0.lock().unwrap().extend_from_slice(buf);
                Ok(buf.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }

        let subscriber = tracing_subscriber::fmt::fmt()
            .with_writer(move || BufWriter(log_buf_writer.clone()))
            .finish();
        let _guard = tracing::subscriber::set_default(subscriber);

        let dir = tempfile::tempdir().expect("create tmp dir");
        let path = dir.path().join("principals.json");
        let _ = PrincipalTable::load_or_bootstrap(&path).expect("bootstrap should succeed");

        let content = std::fs::read_to_string(&path).expect("file written by bootstrap");
        let parsed: serde_json::Value =
            serde_json::from_str(&content).expect("bootstrap file is valid JSON");
        let token = parsed["principals"][0]["token"]
            .as_str()
            .expect("token field in bootstrap file")
            .to_string();

        let log_output = String::from_utf8_lossy(&log_buf.lock().unwrap()).to_string();
        assert!(
            !log_output.contains(&token),
            "bootstrap bearer token must not appear in log output (HIGH-002 redaction)"
        );
    }

    // ── HIGH-001 regression: surface_policies without mcp must yield zero MCP tools ──

    /// HIGH-001: A principal that has surface_policies (GraphQL policy) but no mcp
    /// stanza must have zero MCP tool capabilities. Previously, from_entry fell
    /// through to the class-default set, which could expose all Operator tools to a
    /// GraphQL-only principal through MCP.
    #[test]
    fn high_001_graphql_only_principal_has_no_mcp_tools() {
        let table = PrincipalTable {
            entries: vec![PrincipalEntry {
                token: "tok-graphql-only".into(),
                id: "graphql-only-operator".into(),
                class: PrincipalClass::Operator,
                surface_policies: Some(SurfacePolicies {
                    graphql: Some(GraphqlPolicy {
                        allow_queries: true,
                        allow_subscriptions: true,
                        allowed_mutations: approval_mutations(),
                    }),
                    mcp: None,
                }),
                ..Default::default()
            }],
        };
        let principal = resolve_bearer("tok-graphql-only", &table).unwrap();
        assert!(
            principal.tool_capabilities.is_empty(),
            "GraphQL-only principal (surface_policies present, mcp absent) must have zero MCP tools, \
             got: {:?}",
            principal.tool_capabilities
        );
    }

    // ── SEC-001 regression: surface_policies tool capabilities ──

    /// SEC-001 (tools only): A principal with surface_policies and an mcp stanza gets
    /// exactly the tools listed in allowed_tools; class-default tools do NOT leak.
    ///
    /// SEC-HIGH-001 (corrected): resource_capabilities are ALWAYS zeroed when an mcp stanza
    /// is present, regardless of how many tools are granted. Resources must be separately
    /// granted; they do not inherit from class defaults via the tool list.
    #[test]
    fn sec001_surface_policies_with_mcp_tools_controls_tool_capabilities() {
        let table = PrincipalTable {
            entries: vec![PrincipalEntry {
                token: "tok-agent-mcp".into(),
                id: "agent-mcp".into(),
                class: PrincipalClass::Agent,
                surface_policies: Some(SurfacePolicies {
                    graphql: None,
                    mcp: Some(McpPolicy {
                        allowed_tools: vec!["runs.list".into()],
                    }),
                }),
                ..Default::default()
            }],
        };
        let principal = resolve_bearer("tok-agent-mcp", &table).unwrap();
        // Tools are controlled by the mcp stanza.
        assert!(
            is_tool_allowed(&principal, "runs.list"),
            "allowed tool must be present"
        );
        // SEC-HIGH-001 fix: mcp stanza present → resource_capabilities always zeroed.
        // Resources do not inherit from class defaults; they must be explicitly granted.
        assert!(
            principal.resource_capabilities.is_empty(),
            "principal with mcp stanza must have zero resource capabilities (SEC-HIGH-001 fix)"
        );
    }

    /// SEC-001 (tools only): A principal with surface_policies but no mcp stanza must have
    /// zero tool capabilities (fail-closed for tools).
    #[test]
    fn sec001_surface_policies_without_mcp_zeros_tool_capabilities() {
        let table = PrincipalTable {
            entries: vec![PrincipalEntry {
                token: "tok-graphql-only-res".into(),
                id: "graphql-only-res".into(),
                class: PrincipalClass::Operator,
                surface_policies: Some(SurfacePolicies {
                    graphql: Some(GraphqlPolicy {
                        allow_queries: true,
                        allow_subscriptions: true,
                        allowed_mutations: approval_mutations(),
                    }),
                    mcp: None,
                }),
                ..Default::default()
            }],
        };
        let principal = resolve_bearer("tok-graphql-only-res", &table).unwrap();
        assert!(
            principal.tool_capabilities.is_empty(),
            "principal with surface_policies but no mcp stanza must have zero tool capabilities; \
             got: {:?}",
            principal.tool_capabilities
        );
        // HIGH-001: surface_policies with no mcp stanza → resource capabilities are also zeroed.
        assert!(principal.resource_capabilities.is_empty(),
            "operator principal with surface_policies but no mcp stanza must have zero resource capabilities; \
             got: {:?}",
            principal.resource_capabilities);
    }

    /// HIGH-001: The test_fixture() principal has surface_policies.mcp=None and must
    /// therefore have zero MCP tool capabilities.
    #[test]
    fn high_001_test_fixture_has_no_mcp_tools() {
        let table = PrincipalTable::test_fixture();
        let principal = resolve_bearer("test-token-xxxxxxxxxxxxxxxxxxxxx", &table).unwrap();
        assert!(
            principal.tool_capabilities.is_empty(),
            "test_fixture principal has surface_policies but no mcp stanza — must have zero tools, \
             got: {:?}",
            principal.tool_capabilities
        );
    }

    /// HIGH-001 fix: test_fixture() has surface_policies with no mcp stanza, so both tool and
    /// resource capabilities must be zeroed (fail-closed), not kept at class-default values.
    #[test]
    fn high_001_surface_policies_no_mcp_stanza_zeros_resources() {
        let table = PrincipalTable::test_fixture();
        let principal = resolve_bearer("test-token-xxxxxxxxxxxxxxxxxxxxx", &table).unwrap();
        assert!(
            principal.resource_capabilities.is_empty(),
            "principal with surface_policies but no mcp stanza must have zero resource capabilities; \
             got: {:?}",
            principal.resource_capabilities
        );
    }

    /// HIGH-001 fix: default-operator has mcp.allowed_tools=[] which yields zero tool_capabilities,
    /// so resource_capabilities must also be zeroed (not kept at class-default Operator values).
    #[test]
    fn high_001_default_operator_empty_tools_zeros_resources() {
        // Simulate the default-operator entry constructed by default_operator_entry().
        let table = PrincipalTable {
            entries: vec![PrincipalEntry {
                token: "tok-default-op".into(),
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
                ..Default::default()
            }],
        };
        let principal = resolve_bearer("tok-default-op", &table).unwrap();
        assert!(
            principal.tool_capabilities.is_empty(),
            "default-operator with empty allowed_tools must have zero tool capabilities"
        );
        assert!(
            principal.resource_capabilities.is_empty(),
            "default-operator with empty allowed_tools must have zero resource capabilities; \
             got: {:?}",
            principal.resource_capabilities
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

    // ── SEC-H-001 expiry enforcement ────────────────────────────────────

    fn make_table_with_entry(entry: PrincipalEntry) -> PrincipalTable {
        PrincipalTable {
            entries: vec![entry],
        }
    }

    fn base_entry() -> PrincipalEntry {
        PrincipalEntry {
            token: "tok-expiry-test".into(),
            id: "expiry-test".into(),
            class: PrincipalClass::Operator,
            surface_policies: None,
            expires_at_ms: None,
            not_before_ms: None,
            disabled: None,
            caller_class_override: None,
            run_scope: None,
        }
    }

    #[test]
    fn sec_h001_expired_token_returns_unknown() {
        let past_ms = chrono::Utc::now().timestamp_millis() - 60_000;
        let entry = PrincipalEntry {
            expires_at_ms: Some(past_ms),
            ..base_entry()
        };
        let table = make_table_with_entry(entry);
        assert!(
            matches!(
                resolve_bearer("tok-expiry-test", &table),
                Err(AuthError::UnknownToken)
            ),
            "expired principal must return UnknownToken (non-disclosing)"
        );
    }

    #[test]
    fn sec_h001_disabled_token_returns_unknown() {
        let entry = PrincipalEntry {
            disabled: Some(true),
            ..base_entry()
        };
        let table = make_table_with_entry(entry);
        assert!(
            matches!(
                resolve_bearer("tok-expiry-test", &table),
                Err(AuthError::UnknownToken)
            ),
            "disabled principal must return UnknownToken (non-disclosing)"
        );
    }

    #[test]
    fn sec_h001_not_yet_valid_token_returns_unknown() {
        let future_ms = chrono::Utc::now().timestamp_millis() + 3_600_000;
        let entry = PrincipalEntry {
            not_before_ms: Some(future_ms),
            ..base_entry()
        };
        let table = make_table_with_entry(entry);
        assert!(
            matches!(
                resolve_bearer("tok-expiry-test", &table),
                Err(AuthError::UnknownToken)
            ),
            "not-yet-valid principal must return UnknownToken (non-disclosing)"
        );
    }

    #[test]
    fn sec_h001_valid_future_expiry_resolves() {
        let future_ms = chrono::Utc::now().timestamp_millis() + 3_600_000;
        let entry = PrincipalEntry {
            expires_at_ms: Some(future_ms),
            ..base_entry()
        };
        let table = make_table_with_entry(entry);
        assert!(
            resolve_bearer("tok-expiry-test", &table).is_ok(),
            "principal with future expiry must resolve successfully"
        );
    }

    // ── SEC-P080-HIGH-001 / SEC-P080-HIGH-002 regression tests ────────────
    // ReadOnlyOperator must have ZERO MCP tool and resource capabilities.
    // HIGH-001: it must not inherit general read tools (ideas.list, runs.list,
    //           runs.get, reports.get) via the class=true grant.
    // HIGH-002: P080 tools are denied until per-run scope is implemented;
    //           a self-declared run_id filter does not constitute authorization.

    #[test]
    fn sec_p080_001_read_only_operator_has_no_resource_capabilities() {
        let ro = Principal::new("p080-ro", PrincipalClass::ReadOnlyOperator);
        // All ten resource templates must be denied.
        for id in all_resource_templates() {
            assert!(
                !is_resource_allowed(&ro, id),
                "ReadOnlyOperator must not have resource capability {id:?}"
            );
        }
        assert!(
            ro.resource_capabilities.is_empty(),
            "ReadOnlyOperator.resource_capabilities must be empty at construction"
        );
    }

    #[test]
    fn sec_p080_read_only_operator_tool_capabilities() {
        let ro = Principal::new("p080-ro", PrincipalClass::ReadOnlyOperator);
        // P080: ReadOnlyOperator has exactly diagnostics.get and reconcile.request
        // (diagnose_only gated at handler level) per proposal §3.1 auth matrix.
        assert!(
            is_tool_allowed(&ro, "p080.diagnostics.get.v1"),
            "ReadOnlyOperator must have p080.diagnostics.get.v1"
        );
        assert!(
            is_tool_allowed(&ro, "p080.reconcile.request.v1"),
            "ReadOnlyOperator must have p080.reconcile.request.v1"
        );
        assert!(
            !is_tool_allowed(&ro, "p080.clear_permanent_hold.v1"),
            "ReadOnlyOperator must not have p080.clear_permanent_hold.v1 (Phase 5+ Operator-only)"
        );
        // ReadOnlyOperator must NOT have general read/write tools.
        assert!(
            !is_tool_allowed(&ro, "ideas.list"),
            "ReadOnlyOperator must not have ideas.list"
        );
        assert!(
            !is_tool_allowed(&ro, "runs.list"),
            "ReadOnlyOperator must not have runs.list"
        );
        assert!(
            !is_tool_allowed(&ro, "runs.get"),
            "ReadOnlyOperator must not have runs.get"
        );
        assert!(
            !is_tool_allowed(&ro, "reports.get"),
            "ReadOnlyOperator must not have reports.get"
        );
        assert!(!is_tool_allowed(&ro, "runs.start"));
        assert!(!is_tool_allowed(&ro, "ideas.create"));
        assert!(!is_tool_allowed(&ro, "approvals.resolve"));
        assert!(!is_tool_allowed(&ro, "runs.cancel"));
        // ReadOnlyOperator has exactly the two P080 read tools.
        assert_eq!(
            ro.tool_capabilities.len(),
            2,
            "ReadOnlyOperator must have exactly 2 tool capabilities (P080DiagnosticsGet, P080ReconcileRequest)"
        );
    }

    #[test]
    fn sec_p080_001_read_only_operator_resource_isolation_from_operator() {
        let op = Principal::new("op", PrincipalClass::Operator);
        let ro = Principal::new("p080-ro", PrincipalClass::ReadOnlyOperator);
        // Operator has full read resources; ReadOnlyOperator has none.
        assert!(is_resource_allowed(&op, ResourceTemplateId::RunEntity));
        assert!(is_resource_allowed(&op, ResourceTemplateId::ArtifactEntity));
        assert!(is_resource_allowed(&op, ResourceTemplateId::ReportEntity));
        assert!(!is_resource_allowed(&ro, ResourceTemplateId::RunEntity));
        assert!(!is_resource_allowed(&ro, ResourceTemplateId::IdeaEntity));
        assert!(!is_resource_allowed(
            &ro,
            ResourceTemplateId::ArtifactEntity
        ));
        assert!(!is_resource_allowed(&ro, ResourceTemplateId::ReportEntity));
        assert!(!is_resource_allowed(
            &ro,
            ResourceTemplateId::ChainworksRuns
        ));
        assert!(!is_resource_allowed(
            &ro,
            ResourceTemplateId::ChainworksIdeas
        ));
        assert!(!is_resource_allowed(
            &ro,
            ResourceTemplateId::ChainworksApprovalsInbox
        ));
        assert!(!is_resource_allowed(
            &ro,
            ResourceTemplateId::ChainworksRunStages
        ));
        assert!(!is_resource_allowed(
            &ro,
            ResourceTemplateId::ChainworksRunArtifacts
        ));
        assert!(!is_resource_allowed(
            &ro,
            ResourceTemplateId::StewardAnalysisEntity
        ));
    }

    #[test]
    fn sec_h001_caller_class_expired_token_returns_none() {
        let past_ms = chrono::Utc::now().timestamp_millis() - 60_000;
        let entry = PrincipalEntry {
            expires_at_ms: Some(past_ms),
            ..base_entry()
        };
        let table = make_table_with_entry(entry);
        assert!(
            resolve_caller_class_for_token(&table, "tok-expiry-test").is_none(),
            "expired principal must return None from resolve_caller_class_for_token"
        );
    }
    /// SEC-HIGH-001 regression: an Operator principal configured with only P080 diagnostic
    /// tools must receive NO MCP resource access. Previously the code kept class-default
    /// resources whenever tool_capabilities was non-empty, which allowed narrow-tool
    /// principals to read run://, artifact://, and report:// resources.
    #[test]
    fn sec_high_001_narrow_tool_operator_gets_no_resource_access() {
        let entry = PrincipalEntry {
            token: "tok-p080-only-xxxxxxxxxxxxxxxxxxx".into(),
            id: "p080-diagnostic-only".into(),
            class: PrincipalClass::Operator,
            surface_policies: Some(SurfacePolicies {
                graphql: None,
                mcp: Some(McpPolicy {
                    allowed_tools: vec![
                        "p080.diagnostics.get.v1".into(),
                        "p080.reconcile.request.v1".into(),
                    ],
                }),
            }),
            ..Default::default()
        };
        let table = PrincipalTable {
            entries: vec![entry],
        };
        let p = resolve_bearer("tok-p080-only-xxxxxxxxxxxxxxxxxxx", &table).unwrap();

        // Narrow-tool principal must have the two P080 tools.
        assert!(
            is_tool_allowed(&p, "p080.diagnostics.get.v1"),
            "p080 diagnostic tool must be allowed"
        );
        assert!(
            is_tool_allowed(&p, "p080.reconcile.request.v1"),
            "p080 reconcile tool must be allowed"
        );

        // But NO resource access — not run://, not artifact://, not report://, not any index.
        for id in all_resource_templates() {
            assert!(
                !is_resource_allowed(&p, id),
                "narrow-tool Operator must have no resource access; got access to {id:?}"
            );
        }

        // And no other tools that were not in the allow-list.
        assert!(
            !is_tool_allowed(&p, "runs.start"),
            "runs.start must be blocked"
        );
        assert!(
            !is_tool_allowed(&p, "runs.list"),
            "runs.list must be blocked"
        );
        assert!(
            !is_tool_allowed(&p, "approvals.resolve"),
            "approvals.resolve must be blocked"
        );
    }

    /// SEC-HIGH-001 regression (empty tool list): an Operator with an mcp stanza but
    /// allowed_tools=[] must also have no resource access (preserves previous behaviour).
    #[test]
    fn sec_high_001_empty_tool_list_operator_gets_no_resource_access() {
        let entry = PrincipalEntry {
            token: "tok-empty-tools-xxxxxxxxxxxxxxxxxxx".into(),
            id: "empty-tools-operator".into(),
            class: PrincipalClass::Operator,
            surface_policies: Some(SurfacePolicies {
                graphql: None,
                mcp: Some(McpPolicy {
                    allowed_tools: vec![],
                }),
            }),
            ..Default::default()
        };
        let table = PrincipalTable {
            entries: vec![entry],
        };
        let p = resolve_bearer("tok-empty-tools-xxxxxxxxxxxxxxxxxxx", &table).unwrap();
        for id in all_resource_templates() {
            assert!(
                !is_resource_allowed(&p, id),
                "empty-tool Operator must have no resource access; got access to {id:?}"
            );
        }
    }

    // ── P080 SEC-HIGH-001: run_scope enforcement ─────────────────────────────

    #[test]
    fn p080_run_scope_operator_always_allowed() {
        let mut p = Principal::new("op", PrincipalClass::Operator);
        p.run_scope = Some(vec!["run-a".into()]);
        // Operators are not restricted by run_scope.
        assert!(check_p080_run_scope(&p, Some("run-z")).is_ok());
        assert!(check_p080_run_scope(&p, None).is_ok());
    }

    #[test]
    fn p080_run_scope_scoped_principal_allows_authorized_run() {
        let mut p = Principal::new("ro", PrincipalClass::ReadOnlyOperator);
        p.run_scope = Some(vec!["run-allowed".into(), "run-also-ok".into()]);
        assert!(
            check_p080_run_scope(&p, Some("run-allowed")).is_ok(),
            "authorized run_id must be allowed"
        );
        assert!(
            check_p080_run_scope(&p, Some("run-also-ok")).is_ok(),
            "second authorized run_id must be allowed"
        );
    }

    #[test]
    fn p080_run_scope_scoped_principal_rejects_unauthorized_run() {
        let mut p = Principal::new("ro", PrincipalClass::ReadOnlyOperator);
        p.run_scope = Some(vec!["run-allowed".into()]);
        let result = check_p080_run_scope(&p, Some("run-other"));
        assert!(
            result.is_err(),
            "out-of-scope run_id must be rejected by server-side check"
        );
    }

    #[test]
    fn p080_run_scope_scoped_principal_rejects_no_run_id() {
        let mut p = Principal::new("ro", PrincipalClass::ReadOnlyOperator);
        p.run_scope = Some(vec!["run-allowed".into()]);
        let result = check_p080_run_scope(&p, None);
        assert!(
            result.is_err(),
            "missing run_id must be rejected when scope is set"
        );
    }

    #[test]
    fn p080_run_scope_empty_scope_rejects_all() {
        let mut p = Principal::new("agent", PrincipalClass::Agent);
        p.run_scope = Some(vec![]); // empty scope = no access
        assert!(check_p080_run_scope(&p, Some("any-run")).is_err());
        assert!(check_p080_run_scope(&p, None).is_err());
    }

    #[test]
    fn p080_run_scope_unscoped_restricted_principal_rejects() {
        // SEC-P080-001: fail-closed — restricted principal with no run_scope must be rejected
        // regardless of whether a run_id is supplied, to prevent cross-run disclosure.
        let p = Principal::new("ro", PrincipalClass::ReadOnlyOperator);
        let err_none = check_p080_run_scope(&p, None);
        let err_some = check_p080_run_scope(&p, Some("any-run"));
        assert!(
            err_none.is_err(),
            "no run_scope must reject None filter_run_id"
        );
        assert!(
            err_some.is_err(),
            "no run_scope must reject caller-supplied run_id"
        );
        // Error must mention auth_scope_required so callers can distinguish from scope violations.
        assert!(
            err_none.unwrap_err().contains("auth_scope_required"),
            "error must contain auth_scope_required"
        );
        // Agent principals must also be rejected (not just ReadOnlyOperator).
        let agent = Principal::new("agent1", PrincipalClass::Agent);
        assert!(check_p080_run_scope(&agent, Some("run-x")).is_err());
    }

    #[test]
    fn p080_run_scope_round_trips_through_principal_table() {
        use std::os::unix::fs::PermissionsExt;
        // v3 principals require explicit surface_policies; use an empty mcp stanza.
        let json = r#"{
            "schema_version": 3,
            "principals": [{
                "token": "ro-scoped-token-xxxxxxxxxxxxxxxxxx",
                "id": "ro-scoped",
                "class": "read_only_operator",
                "surface_policies": { "mcp": { "allowed_tools": ["p080.diagnostics.get.v1"] } },
                "run_scope": ["run-abc-123"]
            }]
        }"#;
        let dir = tempfile::tempdir().unwrap();
        std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
        let path = dir.path().join("principals.json");
        std::fs::write(&path, json).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        let table = PrincipalTable::load_or_bootstrap(&path).unwrap();
        let p = resolve_bearer("ro-scoped-token-xxxxxxxxxxxxxxxxxx", &table).unwrap();
        assert_eq!(
            p.run_scope.as_deref(),
            Some(["run-abc-123".to_string()].as_slice())
        );
        assert!(check_p080_run_scope(&p, Some("run-abc-123")).is_ok());
        assert!(check_p080_run_scope(&p, Some("run-other")).is_err());
    }

    #[test]
    fn live_principal_source_revalidates_revoked_disabled_and_rescoped_credentials() {
        let token = "live-reload-token-xxxxxxxxxxxxxxx";
        let source = LivePrincipalSource::new(PrincipalTable::test_fixture_with_class(
            token,
            "live-operator",
            PrincipalClass::Operator,
        ));

        let initial = source.resolve_bearer(token).expect("initial token");
        assert_eq!(initial.class, PrincipalClass::Operator);

        source.update(PrincipalTable { entries: vec![] });
        assert!(
            matches!(source.resolve_bearer(token), Err(AuthError::UnknownToken)),
            "revoked token must be rejected after live reload"
        );

        source.update(PrincipalTable::test_fixture_disabled_token(
            token,
            "disabled-operator",
        ));
        assert!(
            matches!(source.resolve_bearer(token), Err(AuthError::UnknownToken)),
            "disabled token must be rejected after live reload"
        );

        source.update(PrincipalTable::test_fixture_with_class(
            token,
            "rescoped-agent",
            PrincipalClass::Agent,
        ));
        let rescoped = source.resolve_bearer(token).expect("rescoped token");
        assert_eq!(rescoped.class, PrincipalClass::Agent);
        assert!(
            !is_tool_allowed(&rescoped, "approvals.resolve"),
            "re-scoped bearer must not retain stale Operator privileges after live reload"
        );
    }
}
