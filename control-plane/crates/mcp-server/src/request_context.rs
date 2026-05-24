//! MCP request context plumbing (Proposal 042 §9.3).
//!
//! The MCP HTTP transport lives behind the same axum router that the
//! GraphQL server uses, so every MCP request has an `X-Request-ID`
//! attached by the shared middleware. Tool handlers need that id when
//! they construct a `CallerContext` so the command journal picks it up.
//!
//! Rather than thread an extra parameter through every `dispatch_tool`
//! call-site, we stash the id in a tokio task-local for the lifetime of
//! the MCP request. `mcp_caller` reads the task-local and attaches the
//! id to `CallerContext::mcp` automatically. MCP stdio mode never
//! enters the scope, so the task-local resolves to `None` and the
//! caller context is unchanged.

use domain::commands::CallerContext;

tokio::task_local! {
    /// Id of the currently-handled inbound MCP HTTP request, if any.
    /// Populated by `http::handle_mcp_post` via
    /// [`scope_request_id`]; read by [`mcp_caller`] when a tool
    /// handler constructs its `CallerContext`.
    pub static MCP_REQUEST_ID: Option<String>;

    /// Diagnostic token_id for the current MCP HTTP request, if any.
    /// SEC-P081-M002: populated by `http::handle_mcp_post` with the derived
    /// sha256-based token_id (never the raw bearer token). Plumbed into audit_log
    /// writes for incident correlation without exposing the raw credential.
    pub static MCP_TOKEN_ID: Option<String>;

    /// P081 Phase 3: MCP idempotency key for the current state-changing tool call.
    /// Scoped by `server.rs` before `dispatch_tool` so `mcp_caller` can stamp it
    /// into `CallerContext.mcp_idempotency_key` and command_journal persists the linkage.
    pub static MCP_IDEMPOTENCY_KEY: Option<String>;

    /// P081 Phase 3: Boundary matrix row_id that allowed the current tool call.
    /// Scoped by `server.rs` after BoundaryPolicy::Allow so command_journal carries
    /// the row_id for audit linkage without threading it through every tool handler.
    pub static MCP_BOUNDARY_ROW_ID: Option<String>;
}

/// Wrap `future` so its inner tasks observe `request_id` on the
/// `MCP_REQUEST_ID` task-local. No-op semantics for `None` — call-sites
/// outside the HTTP handler can still invoke `mcp_caller` and it will
/// simply skip the `with_request_id` step.
pub async fn scope_request_id<F, T>(request_id: Option<String>, future: F) -> T
where
    F: std::future::Future<Output = T>,
{
    MCP_REQUEST_ID.scope(request_id, future).await
}

/// Read the ambient `MCP_REQUEST_ID` task-local, returning `None` when
/// called outside a `scope_request_id` span (e.g. MCP stdio).
pub fn current_request_id() -> Option<String> {
    MCP_REQUEST_ID.try_with(|rid| rid.clone()).ok().flatten()
}

/// Wrap `future` so its inner tasks observe `token_id` on the `MCP_TOKEN_ID`
/// task-local. Must be used together with `scope_request_id` in HTTP handlers.
pub async fn scope_token_id<F, T>(token_id: Option<String>, future: F) -> T
where
    F: std::future::Future<Output = T>,
{
    MCP_TOKEN_ID.scope(token_id, future).await
}

/// Read the ambient `MCP_TOKEN_ID` task-local, returning `None` when
/// called outside a `scope_token_id` span (e.g. MCP stdio or unauthenticated paths).
pub fn current_token_id() -> Option<String> {
    MCP_TOKEN_ID.try_with(|tid| tid.clone()).ok().flatten()
}

/// Wrap `future` so its inner tasks observe `key` on the `MCP_IDEMPOTENCY_KEY` task-local.
/// Called by server.rs immediately before `dispatch_tool` for state-changing MCP calls.
pub async fn scope_idempotency_key<F, T>(key: Option<String>, future: F) -> T
where
    F: std::future::Future<Output = T>,
{
    MCP_IDEMPOTENCY_KEY.scope(key, future).await
}

/// Read the ambient `MCP_IDEMPOTENCY_KEY` task-local.
pub fn current_idempotency_key() -> Option<String> {
    MCP_IDEMPOTENCY_KEY.try_with(|k| k.clone()).ok().flatten()
}

/// Wrap `future` so its inner tasks observe `row_id` on the `MCP_BOUNDARY_ROW_ID` task-local.
pub async fn scope_boundary_row_id<F, T>(row_id: Option<String>, future: F) -> T
where
    F: std::future::Future<Output = T>,
{
    MCP_BOUNDARY_ROW_ID.scope(row_id, future).await
}

/// Read the ambient `MCP_BOUNDARY_ROW_ID` task-local.
pub fn current_boundary_row_id() -> Option<String> {
    MCP_BOUNDARY_ROW_ID.try_with(|r| r.clone()).ok().flatten()
}

/// Build a `CallerContext::mcp(...)` and attach the ambient request id
/// if one was installed by the HTTP handler, plus the derived caller_class.
/// Use this in every MCP tool handler instead of calling
/// `CallerContext::mcp` directly.
///
/// Passes the full `Principal` so that v3 caller_class_override (automation,
/// developer_break_glass) is stamped correctly into the command journal row.
pub fn mcp_caller(principal: &auth::Principal, tool_name: &str) -> CallerContext {
    // P081 Phase 3: stamp caller_class for MCP transport.
    // Respects caller_class_override for v3 automation/developer_break_glass principals.
    // Operator principals on MCP default to agent_operator; ui_operator is GraphQL-only.
    // matrix_row: p081.agent_operator.mcp_tools_call.command
    let caller_class = auth::derive_caller_class_for_mcp(principal);
    let mut caller = CallerContext::mcp(&principal.id, &principal.class, tool_name)
        .with_caller_class(caller_class.as_str());
    if let Some(rid) = current_request_id() {
        caller = caller.with_request_id(rid);
    }
    // P081 Phase 3: attach idempotency key and boundary row_id from task-locals
    // so command_journal carries the linkage without threading through every tool handler.
    if let Some(key) = current_idempotency_key() {
        caller = caller.with_mcp_idempotency_key(key);
    }
    if let Some(row_id) = current_boundary_row_id() {
        caller = caller.with_boundary_row_id(row_id);
    }
    caller
}

#[cfg(test)]
mod tests {
    use super::*;
    use auth::{CallerClass, Principal, PrincipalClass};

    #[tokio::test]
    async fn mcp_caller_is_unscoped_outside_request_body() {
        let p = Principal::new("op", PrincipalClass::Operator);
        let caller = mcp_caller(&p, "runs.start");
        assert!(
            caller.request_id.is_none(),
            "outside a `scope_request_id` span the request id must be None"
        );
    }

    #[tokio::test]
    async fn mcp_caller_picks_up_scoped_request_id() {
        let p = Principal::new("op", PrincipalClass::Operator);
        let caller = scope_request_id(Some("req-42".into()), async {
            mcp_caller(&p, "runs.start")
        })
        .await;
        assert_eq!(caller.request_id.as_deref(), Some("req-42"));
    }

    #[tokio::test]
    async fn scope_request_id_none_is_transparent() {
        let p = Principal::new("op", PrincipalClass::Operator);
        let caller = scope_request_id(None, async { mcp_caller(&p, "runs.cancel") }).await;
        assert!(caller.request_id.is_none());
    }

    // P081 Phase 3: caller_class must be stamped by mcp_caller.
    // Operator on MCP → agent_operator (MCP is the agent control plane).
    // matrix_row: p081.agent_operator.mcp_tools_call.command
    #[tokio::test]
    async fn mcp_caller_stamps_caller_class_from_principal_class() {
        let op = mcp_caller(
            &Principal::new("op", PrincipalClass::Operator),
            "runs.start",
        );
        assert_eq!(
            op.caller_class.as_deref(),
            Some("agent_operator"),
            "Operator on MCP must be agent_operator; ui_operator is GraphQL-only"
        );

        let ag = mcp_caller(&Principal::new("ag", PrincipalClass::Agent), "runs.start");
        assert_eq!(ag.caller_class.as_deref(), Some("agent_operator"));

        let ob = mcp_caller(&Principal::new("ob", PrincipalClass::Observer), "runs.list");
        assert_eq!(ob.caller_class.as_deref(), Some("observer"));
    }

    #[tokio::test]
    async fn mcp_caller_respects_caller_class_override() {
        let mut automation = Principal::new("auto-agent", PrincipalClass::Agent);
        automation.caller_class_override = Some(CallerClass::Automation);
        let caller = mcp_caller(&automation, "runs.list");
        assert_eq!(
            caller.caller_class.as_deref(),
            Some("automation"),
            "v3 caller_class_override must propagate into CallerContext"
        );
    }
}
