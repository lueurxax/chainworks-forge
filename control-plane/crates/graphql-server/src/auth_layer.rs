use axum::{
    extract::Request,
    http::{Method, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};

/// Axum middleware that extracts and validates a Bearer token.
/// On success, inserts `auth::Principal` into request extensions so that
/// `async_graphql::Context::data::<auth::Principal>()` can retrieve it.
/// On failure, returns HTTP 401 with a GraphQL-shaped error body.
///
/// The `PrincipalTable` is passed as a parameter (not from request extensions)
/// to avoid relying on extension injection from an outer layer.
pub async fn require_auth(
    mut request: Request,
    next: Next,
    principal_table: auth::PrincipalTable,
) -> Response {
    // Playground exemption: allow only the HTML playground shell when
    // CHAINWORKS_PLAYGROUND_AUTH=skip. GET requests carrying a query string
    // still pass through bearer auth.
    let is_playground_get = is_playground_html_request(&request);
    let playground_skip = std::env::var("CHAINWORKS_PLAYGROUND_AUTH")
        .ok()
        .map(|v| v == "skip")
        .unwrap_or(false);

    if is_playground_get && playground_skip {
        return next.run(request).await;
    }

    let auth_header = request
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    match auth_header {
        Some(header_value) => match auth::extract_bearer_token(&header_value) {
            Ok(token) => match auth::resolve_bearer(token, &principal_table) {
                Ok(principal) => {
                    request.extensions_mut().insert(principal);
                    next.run(request).await
                }
                Err(_) => unauthorized_response(),
            },
            Err(_) => unauthorized_response(),
        },
        None => unauthorized_response(),
    }
}

fn is_playground_html_request(request: &Request) -> bool {
    request.method() == Method::GET
        && request.uri().path() == "/graphql"
        && request.uri().query().is_none()
}

fn unauthorized_response() -> Response {
    let body = serde_json::json!({
        "errors": [{
            "message": "unauthorized",
            "extensions": { "code": "UNAUTHORIZED" }
        }]
    });
    (
        StatusCode::UNAUTHORIZED,
        [("content-type", "application/json")],
        serde_json::to_string(&body).unwrap(),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;

    fn request(method: Method, uri: &str) -> Request {
        Request::builder()
            .method(method)
            .uri(uri)
            .body(Body::empty())
            .expect("request")
    }

    #[test]
    fn playground_skip_only_matches_plain_graphql_get() {
        assert!(is_playground_html_request(&request(
            Method::GET,
            "/graphql"
        )));
        assert!(!is_playground_html_request(&request(
            Method::GET,
            "/graphql?query=%7B__typename%7D"
        )));
        assert!(!is_playground_html_request(&request(
            Method::POST,
            "/graphql"
        )));
        assert!(!is_playground_html_request(&request(Method::GET, "/ready")));
    }
}
