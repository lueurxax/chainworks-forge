//! P082-SEC-H1: GraphQL WebSocket bearer-token auth uses the same strict parser
//! as HTTP GraphQL and MCP.
//!
//! Tests prove that `connection_init_data` (the WebSocket on_connection_init handler)
//! rejects tokens that the HTTP path rejects: short, whitespace-normalized,
//! control-character, non-visible-ASCII, and over-4096-byte tokens. Before the fix,
//! the WebSocket path used strip_prefix("Bearer ") + trim(), which accepted tokens
//! that would be rejected by auth::extract_bearer_token on the HTTP path.

use auth::PrincipalTable;
use graphql_server::server::connection_init_data;

fn make_table_with_token(token: &str) -> PrincipalTable {
    PrincipalTable::test_fixture_with_token(token)
}

/// Builds a connection_init JSON value with the given Authorization header string.
fn init_value(authorization: &str) -> serde_json::Value {
    serde_json::json!({ "Authorization": authorization })
}

// ── Valid token is accepted ────────────────────────────────────────────────────

#[tokio::test]
async fn ws_valid_bearer_token_is_accepted() {
    let token = "a".repeat(32);
    let table = make_table_with_token(&token);
    let value = init_value(&format!("Bearer {token}"));
    let result = connection_init_data(value, table).await;
    assert!(
        result.is_ok(),
        "P082-SEC-H1: a valid 32-byte visible-ASCII bearer token must be accepted over WS"
    );
}

// ── Short token is rejected ───────────────────────────────────────────────────

#[tokio::test]
async fn ws_short_bearer_token_is_rejected() {
    let token = "short"; // < 32 bytes
    let table = make_table_with_token(token);
    let value = init_value(&format!("Bearer {token}"));
    let result = connection_init_data(value, table).await;
    assert!(
        result.is_err(),
        "P082-SEC-H1: a token shorter than 32 bytes must be rejected over WS"
    );
}

// ── Whitespace-normalized token is rejected ───────────────────────────────────

#[tokio::test]
async fn ws_whitespace_padded_bearer_token_is_rejected() {
    // "Bearer <spaces> token" — old code would trim leading spaces and accept;
    // extract_bearer_token enforces strict "Bearer " prefix with no surrounding space.
    let token = "a".repeat(32);
    let table = make_table_with_token(&token);
    let value = init_value(&format!("  Bearer {token}")); // leading space
    let result = connection_init_data(value, table).await;
    assert!(
        result.is_err(),
        "P082-SEC-H1: a token with leading whitespace before 'Bearer' must be rejected over WS"
    );
}

#[tokio::test]
async fn ws_trailing_space_in_token_is_rejected() {
    // Token value contains an embedded space (not visible ASCII).
    let token = format!("{} {}", "a".repeat(16), "a".repeat(15)); // 32 chars with space
    let table = make_table_with_token(&token);
    let value = init_value(&format!("Bearer {token}"));
    let result = connection_init_data(value, table).await;
    assert!(
        result.is_err(),
        "P082-SEC-H1: a token containing a space must be rejected over WS"
    );
}

// ── Control-character token is rejected ──────────────────────────────────────

#[tokio::test]
async fn ws_control_character_bearer_token_is_rejected() {
    let mut token = "a".repeat(31);
    token.push('\x01'); // CTL byte
    let table = make_table_with_token(&token);
    let value = init_value(&format!("Bearer {token}"));
    let result = connection_init_data(value, table).await;
    assert!(
        result.is_err(),
        "P082-SEC-H1: a token containing a control character must be rejected over WS"
    );
}

// ── Non-visible-ASCII token is rejected ──────────────────────────────────────

#[tokio::test]
async fn ws_non_visible_ascii_bearer_token_is_rejected() {
    let mut token = "a".repeat(32);
    token.push('\u{00A9}'); // © — non-ASCII
    let table = make_table_with_token(&token);
    let value = init_value(&format!("Bearer {token}"));
    let result = connection_init_data(value, table).await;
    assert!(
        result.is_err(),
        "P082-SEC-H1: a token with non-visible-ASCII bytes must be rejected over WS"
    );
}

// ── Over-4096-byte token is rejected ─────────────────────────────────────────

#[tokio::test]
async fn ws_over_4096_byte_bearer_token_is_rejected() {
    let token = "a".repeat(4097);
    let table = make_table_with_token(&token);
    let value = init_value(&format!("Bearer {token}"));
    let result = connection_init_data(value, table).await;
    assert!(
        result.is_err(),
        "P082-SEC-H1: a token longer than 4096 bytes must be rejected over WS"
    );
}

// ── Missing Authorization field is rejected ───────────────────────────────────

#[tokio::test]
async fn ws_missing_authorization_field_is_rejected() {
    let table = make_table_with_token(&"a".repeat(32));
    let value = serde_json::json!({ "other_field": "irrelevant" });
    let result = connection_init_data(value, table).await;
    assert!(
        result.is_err(),
        "P082-SEC-H1: a connection_init payload without Authorization must be rejected over WS"
    );
}

// ── Wrong scheme is rejected ──────────────────────────────────────────────────

#[tokio::test]
async fn ws_basic_auth_scheme_is_rejected() {
    let table = make_table_with_token(&"a".repeat(32));
    let value = init_value(&format!("Basic {}", "a".repeat(32)));
    let result = connection_init_data(value, table).await;
    assert!(
        result.is_err(),
        "P082-SEC-H1: a Basic auth scheme must be rejected over WS (only Bearer is accepted)"
    );
}
