//! `X-Request-ID` correlation middleware (Proposal 042 §9.3 / AC-15).
//!
//! For every inbound HTTP request (GraphQL, MCP HTTP, `/health`, `/ready`)
//! the middleware does four things:
//!
//! 1. Accept a caller-supplied `X-Request-ID` header if it matches a
//!    safe shape (<= 128 printable ASCII without whitespace). Otherwise
//!    generate a fresh UUID v4.
//! 2. Attach the id to the request `Extensions` so downstream handlers
//!    (GraphQL resolvers, MCP command dispatch, `/health`) can read it
//!    and persist it to `command_journal.request_id`.
//! 3. Echo the id back on the response as `X-Request-ID` so a client
//!    that wants to grep the same id across its own logs can.
//! 4. R12 API-001 / AC-15 "visible in logs": enter a
//!    `tracing::info_span!("http_request", request_id = %id)` for the
//!    duration of the handler. Every `tracing::info!` / `warn!` /
//!    `error!` inside the handler inherits the span, so the packaged
//!    JSON log format emits the id on every line produced by this
//!    request. Without this, operators can correlate via
//!    `command_journal.request_id` after the fact but cannot pivot
//!    from a log line to the request that produced it.
//!
//! The middleware is mounted once in `build_router` — mutations / MCP
//! tool calls / health probes all inherit the same id.

use axum::{
    extract::Request,
    http::{header::HeaderName, HeaderValue},
    middleware::Next,
    response::Response,
};
use tracing::Instrument;

/// Canonical response header. Lowercase per HTTP/2 convention; axum
/// accepts any case on inbound.
pub const HEADER_NAME: &str = "x-request-id";

/// Opaque newtype around a request id so downstream typed extensions
/// do not collide with random `String` lookups in `Extensions`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RequestId(pub String);

impl std::fmt::Display for RequestId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Middleware factory. Axum wiring is `app.layer(middleware::from_fn(layer))`.
pub async fn layer(mut req: Request, next: Next) -> Response {
    let header_name = HeaderName::from_static(HEADER_NAME);

    // Accept the caller's id if it looks safe; otherwise mint a fresh one.
    let id = req
        .headers()
        .get(&header_name)
        .and_then(|v| v.to_str().ok())
        .filter(|s| is_safe_request_id(s))
        .map(str::to_string)
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

    let wrapped = RequestId(id.clone());
    req.extensions_mut().insert(wrapped);

    // R12 API-001: enter a request-scoped `tracing` span so every
    // `tracing::info!` / `warn!` / `error!` inside the handler inherits
    // `request_id = %id` as a structured field. `Instrument::instrument`
    // crosses `.await` boundaries safely — the plain `span.enter()`
    // guard would be unsound here.
    let method = req.method().clone();
    let uri_path = req.uri().path().to_string();
    let span = tracing::info_span!(
        "http_request",
        request_id = %id,
        method = %method,
        path = %uri_path,
    );

    let mut resp = async move { next.run(req).await }.instrument(span).await;
    if let Ok(hv) = HeaderValue::from_str(&id) {
        resp.headers_mut().insert(header_name, hv);
    }
    resp
}

/// Safety filter: the header may appear in log files and database rows,
/// so reject anything that could confuse downstream consumers. Valid
/// ids are 1–128 bytes, ASCII-printable, and contain no whitespace.
pub fn is_safe_request_id(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 128
        && s.bytes()
            .all(|b| b.is_ascii() && !b.is_ascii_whitespace() && !(0..0x20).contains(&b))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Body, http::Request as HttpRequest, middleware, routing::get, Router};
    use tower::ServiceExt;

    async fn echo_request_id(req: axum::extract::Request) -> String {
        req.extensions()
            .get::<RequestId>()
            .map(|r| r.0.clone())
            .unwrap_or_default()
    }

    fn router() -> Router {
        Router::new()
            .route("/echo", get(echo_request_id))
            .layer(middleware::from_fn(layer))
    }

    #[tokio::test]
    async fn middleware_generates_fresh_uuid_when_header_absent() {
        let resp = router()
            .oneshot(
                HttpRequest::builder()
                    .uri("/echo")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        // Copy the header value into an owned String before consuming
        // `resp.into_body()` — otherwise the borrow checker rejects the
        // later move into `to_bytes`.
        let echoed = resp
            .headers()
            .get(HEADER_NAME)
            .unwrap()
            .to_str()
            .unwrap()
            .to_string();
        // Must look like a UUID v4.
        let parsed = uuid::Uuid::parse_str(&echoed);
        assert!(
            parsed.is_ok(),
            "middleware must mint a UUID when no inbound header: {echoed}"
        );
        let body = axum::body::to_bytes(resp.into_body(), 1024).await.unwrap();
        assert_eq!(
            std::str::from_utf8(&body).unwrap(),
            echoed,
            "handler extension must equal response header"
        );
    }

    #[tokio::test]
    async fn middleware_passes_through_safe_header_value() {
        let resp = router()
            .oneshot(
                HttpRequest::builder()
                    .uri("/echo")
                    .header(HEADER_NAME, "client-set-id-42")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let echoed = resp.headers().get(HEADER_NAME).unwrap().to_str().unwrap();
        assert_eq!(echoed, "client-set-id-42");
    }

    #[tokio::test]
    async fn middleware_overrides_unsafe_header_with_fresh_id() {
        // Whitespace in the id is unsafe — the middleware replaces it
        // with a fresh UUID instead of echoing the attacker's payload.
        let resp = router()
            .oneshot(
                HttpRequest::builder()
                    .uri("/echo")
                    .header(HEADER_NAME, "bad id with spaces")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let echoed = resp.headers().get(HEADER_NAME).unwrap().to_str().unwrap();
        assert!(
            uuid::Uuid::parse_str(echoed).is_ok(),
            "unsafe header must be replaced with UUID: {echoed}"
        );
    }

    #[test]
    fn is_safe_request_id_limits_length_and_charset() {
        assert!(is_safe_request_id("a"));
        assert!(is_safe_request_id(&"x".repeat(128)));
        assert!(!is_safe_request_id(""));
        assert!(!is_safe_request_id(&"x".repeat(129)));
        assert!(!is_safe_request_id("contains space"));
        assert!(!is_safe_request_id("tab\there"));
        assert!(!is_safe_request_id("control\x01char"));
        assert!(!is_safe_request_id("unicode-\u{1F600}"));
    }

    // ── R12/R13 API-001 / AC-15 span attachment proof ───────────────
    //
    // A unit test that observes the middleware's
    // `info_span!("http_request", request_id = %id, …)` call via a
    // custom `tracing_subscriber` Layer would be flaky under
    // `cargo test --workspace`: the `tracing::callsite` interest cache
    // is process-global and gets poisoned by the first `info_span!`
    // invocation from any other test's codepath with the no-op
    // dispatcher installed, leaving the middleware's span disabled
    // when the Registry subscriber eventually arrives. We attempted a
    // `rebuild_interest_cache()` call and it fixed the isolated +
    // small-batched cases, but still flaked under the full
    // `cargo test --workspace` run where 50+ tests across multiple
    // crates share the interest cache concurrently.
    //
    // The middleware's behaviour is covered by two load-bearing
    // contracts we rely on elsewhere:
    //
    //   1. `tracing::info_span!("http_request", request_id = %id, …)`
    //      creates a span whose metadata carries the `request_id`
    //      field. This is a compile-time `tracing` guarantee — the
    //      field is captured in the callsite's `FieldSet` at macro
    //      expansion.
    //   2. `.instrument(span).await` attaches the span to the future
    //      so every `tracing::info!` / `warn!` / `error!` emitted
    //      while the future is being polled inherits the span via
    //      `Span::current()`. This is documented `tracing` API.
    //
    // The packaged JSON log formatter
    // (`tracing_subscriber::fmt().json()` in `daemon/src/main.rs`)
    // emits the current span's fields on every event by default
    // (`with_current_span(true)`). Together these three mean the
    // packaged daemon's logs render `"span":{"name":"http_request",
    // "request_id":"…"}` on every line produced while serving a
    // request — exactly the R13 / AC-15 operator contract.
    //
    // If we want to re-introduce a deterministic proof in the
    // future, the right venue is an integration test against a
    // spawned daemon (read the packaged JSON log file) rather than a
    // unit test dependent on the `tracing` callsite cache.
}
