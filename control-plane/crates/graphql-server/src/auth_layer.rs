use axum::{
    extract::Request,
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};

/// Axum middleware that extracts and validates a Bearer token.
/// On success, inserts `auth::Principal` into request extensions.
/// On failure, returns HTTP 401 with a GraphQL-shaped error body.
pub async fn require_auth(mut request: Request, next: Next) -> Response {
    // Playground exemption: allow unauthenticated GET (playground HTML)
    // when CHAINWORKS_PLAYGROUND_AUTH=skip.
    let is_playground_get = request.method() == axum::http::Method::GET;
    let playground_skip = std::env::var("CHAINWORKS_PLAYGROUND_AUTH")
        .ok()
        .map(|v| v == "skip")
        .unwrap_or(false);

    if is_playground_get && playground_skip {
        return next.run(request).await;
    }

    let table = request.extensions().get::<auth::PrincipalTable>().cloned();

    let auth_header = request
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    match auth_header {
        Some(header_value) => match auth::extract_bearer_token(&header_value) {
            Ok(token) => {
                if let Some(ref table) = table {
                    match auth::resolve_bearer(token, table) {
                        Ok(principal) => {
                            request.extensions_mut().insert(principal);
                            next.run(request).await
                        }
                        Err(_) => unauthorized_response(),
                    }
                } else {
                    unauthorized_response()
                }
            }
            Err(_) => unauthorized_response(),
        },
        None => unauthorized_response(),
    }
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
