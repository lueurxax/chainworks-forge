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
use domain::PrincipalClass;

tokio::task_local! {
    /// Id of the currently-handled inbound MCP HTTP request, if any.
    /// Populated by `http::handle_mcp_post` via
    /// [`scope_request_id`]; read by [`mcp_caller`] when a tool
    /// handler constructs its `CallerContext`.
    pub static MCP_REQUEST_ID: Option<String>;
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

/// Build a `CallerContext::mcp(...)` and attach the ambient request id
/// if one was installed by the HTTP handler. Use this in every MCP tool
/// handler instead of calling `CallerContext::mcp` directly.
pub fn mcp_caller(
    principal_id: &str,
    principal_class: &PrincipalClass,
    tool_name: &str,
) -> CallerContext {
    let caller = CallerContext::mcp(principal_id, principal_class, tool_name);
    match current_request_id() {
        Some(rid) => caller.with_request_id(rid),
        None => caller,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn mcp_caller_is_unscoped_outside_request_body() {
        let caller = mcp_caller("op", &PrincipalClass::Operator, "runs.start");
        assert!(
            caller.request_id.is_none(),
            "outside a `scope_request_id` span the request id must be None"
        );
    }

    #[tokio::test]
    async fn mcp_caller_picks_up_scoped_request_id() {
        let caller = scope_request_id(Some("req-42".into()), async {
            mcp_caller("op", &PrincipalClass::Operator, "runs.start")
        })
        .await;
        assert_eq!(caller.request_id.as_deref(), Some("req-42"));
    }

    #[tokio::test]
    async fn scope_request_id_none_is_transparent() {
        let caller = scope_request_id(None, async {
            mcp_caller("op", &PrincipalClass::Operator, "runs.cancel")
        })
        .await;
        assert!(caller.request_id.is_none());
    }
}
