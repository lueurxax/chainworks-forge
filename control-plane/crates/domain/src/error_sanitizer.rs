/// Truncate `s` to at most `max_bytes` bytes without splitting UTF-8.
pub fn truncate_utf8_safe(s: &str, max_bytes: usize) -> String {
    if s.len() <= max_bytes {
        return s.to_string();
    }

    let mut boundary = max_bytes;
    while boundary > 0 && !s.is_char_boundary(boundary) {
        boundary -= 1;
    }

    format!("{}...[truncated]", &s[..boundary])
}

/// Strip credential-like material from release/provider errors before storing
/// or exposing them through operator readback.
pub fn strip_credentials_from_error(s: &str) -> String {
    let redacted = redact_sensitive_assignments(s);
    let tokens: Vec<&str> = redacted.split_whitespace().collect();
    let mut out = Vec::with_capacity(tokens.len());
    let mut redact_next = false;

    let mut idx = 0;
    while idx < tokens.len() {
        let token = tokens[idx];
        let lower = token.to_ascii_lowercase();
        let normalized = lower.trim_matches(|c: char| matches!(c, '"' | '\'' | ',' | ';' | ':'));

        if redact_next {
            out.push("[redacted]".to_string());
            redact_next = normalized == "bearer" || normalized == "token";
            idx += 1;
            continue;
        }

        if normalized == "bearer" || normalized == "token" {
            out.push(token.to_string());
            redact_next = true;
            idx += 1;
            continue;
        }

        if is_sensitive_label(normalized) {
            out.push(token.to_string());
            if let Some(next) = tokens.get(idx + 1) {
                if matches!(
                    next.trim_matches(|c: char| matches!(c, '"' | '\'' | ',' | ';')),
                    ":" | "="
                ) {
                    out.push("[redacted]".to_string());
                    idx += 2;
                    redact_next = true;
                    continue;
                }
            }
            redact_next = true;
            idx += 1;
            continue;
        }

        if let Some(sep) = token.find('=').or_else(|| token.find(':')) {
            let key = token[..sep].to_ascii_lowercase();
            if is_sensitive_key(&key) {
                out.push(format!("{}=[redacted]", &token[..sep]));
                idx += 1;
                continue;
            }
        }

        if lower.starts_with("sk-")
            || lower.starts_with("ghp_")
            || lower.contains("/.ssh/")
            || lower.ends_with("/id_rsa")
            || lower.contains("/id_rsa")
        {
            out.push("[redacted]".to_string());
            idx += 1;
            continue;
        }

        out.push(token.to_string());
        idx += 1;
    }

    out.join(" ")
}

const SENSITIVE_ASSIGNMENT_KEYS: &[&str] = &[
    "access_token",
    "auth_token",
    "authorization",
    "api_key",
    "password",
    "apikey",
    "secret",
    "token",
];

fn is_sensitive_label(label: &str) -> bool {
    matches!(
        label,
        "access_token"
            | "auth_token"
            | "authorization"
            | "api_key"
            | "apikey"
            | "bearer"
            | "password"
            | "secret"
            | "token"
    )
}

fn is_sensitive_key(key: &str) -> bool {
    key.contains("token")
        || key.contains("api_key")
        || key.contains("apikey")
        || key.contains("secret")
        || key.contains("password")
        || key.contains("authorization")
        || key.contains("bearer")
}

fn is_assignment_key_boundary(byte: Option<u8>) -> bool {
    match byte {
        None => true,
        Some(byte) => {
            byte.is_ascii_whitespace()
                || matches!(
                    byte,
                    b'?' | b'&' | b'"' | b'\'' | b',' | b';' | b'(' | b'[' | b'{'
                )
        }
    }
}

fn is_assignment_value_boundary(byte: u8) -> bool {
    byte.is_ascii_whitespace()
        || matches!(byte, b'&' | b'"' | b'\'' | b',' | b';' | b')' | b']' | b'}')
}

fn skip_sensitive_assignment_value(s: &str, mut i: usize, key: &str) -> usize {
    let bytes = s.as_bytes();
    while i < bytes.len() && bytes[i].is_ascii_whitespace() {
        i += 1;
    }

    if i >= bytes.len() {
        return i;
    }

    if matches!(bytes[i], b'"' | b'\'') {
        let quote = bytes[i];
        i += 1;
        while i < bytes.len() && bytes[i] != quote {
            i += 1;
        }
        if i < bytes.len() {
            i += 1;
        }
        return i;
    }

    let first_start = i;
    while i < bytes.len() && !is_assignment_value_boundary(bytes[i]) {
        i += 1;
    }

    if key == "authorization" {
        let first = s[first_start..i]
            .to_ascii_lowercase()
            .trim_matches(|c: char| matches!(c, '"' | '\'' | ',' | ';' | ':'))
            .to_string();
        if first == "bearer" {
            while i < bytes.len() && bytes[i].is_ascii_whitespace() {
                i += 1;
            }
            while i < bytes.len() && !is_assignment_value_boundary(bytes[i]) {
                i += 1;
            }
        }
    }

    i
}

fn redact_sensitive_assignments(s: &str) -> String {
    let lower = s.to_ascii_lowercase();
    let bytes = s.as_bytes();
    let lower_bytes = lower.as_bytes();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;

    while i < bytes.len() {
        let matched_key = SENSITIVE_ASSIGNMENT_KEYS.iter().find(|key| {
            let key_bytes = key.as_bytes();
            lower_bytes[i..].starts_with(key_bytes)
                && is_assignment_key_boundary(i.checked_sub(1).map(|idx| bytes[idx]))
                && matches!(bytes.get(i + key_bytes.len()), Some(b'=') | Some(b':'))
        });

        if let Some(key) = matched_key {
            let sep_idx = i + key.len();
            out.push_str(&s[i..=sep_idx]);
            out.push_str("[redacted]");
            i = skip_sensitive_assignment_value(s, sep_idx + 1, key);
            continue;
        }

        let ch = s[i..].chars().next().expect("valid char boundary");
        out.push(ch);
        i += ch.len_utf8();
    }

    out
}

pub fn sanitize_error_for_storage(s: &str, max_bytes: usize) -> String {
    truncate_utf8_safe(&strip_credentials_from_error(s), max_bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncates_on_utf8_boundary() {
        let value = "abc😀def";
        let truncated = truncate_utf8_safe(value, 5);
        assert_eq!(truncated, "abc...[truncated]");
    }

    #[test]
    fn strips_credentials_before_truncating() {
        let value = "failed Authorization:Bearer token sk-test api_key=abc ghp_secret";
        let sanitized = sanitize_error_for_storage(value, 512);
        assert!(!sanitized.contains("sk-test"));
        assert!(!sanitized.contains("api_key=abc"));
        assert!(!sanitized.contains("ghp_secret"));
        assert!(sanitized.contains("[redacted]"));
    }

    #[test]
    fn strips_token_like_url_query_and_key_value_forms() {
        let value = "failed url=https://example.test/callback?token=secret-token&ok=1 password=hunter2 authorization=Bearer";
        let sanitized = sanitize_error_for_storage(value, 512);

        assert!(!sanitized.contains("secret-token"));
        assert!(!sanitized.contains("hunter2"));
        assert!(!sanitized.contains("Bearer"));
        assert!(sanitized.contains("token=[redacted]"));
        assert!(sanitized.contains("password=[redacted]"));
        assert!(sanitized.contains("authorization=[redacted]"));
        assert!(sanitized.contains("ok=1"));
    }

    #[test]
    fn strips_separated_secret_value_forms() {
        let value = concat!(
            "password: hunter2 ",
            "secret abc123 ",
            "api_key: sk_api ",
            "apikey = quoted ",
            "access_token abc ",
            "auth_token = xyz ",
            "authorization: Bearer auth123 ",
            "token bare-token"
        );
        let sanitized = sanitize_error_for_storage(value, 512);

        for leaked in [
            "hunter2",
            "abc123",
            "sk_api",
            "quoted",
            "abc",
            "xyz",
            "auth123",
            "bare-token",
        ] {
            assert!(
                !sanitized.contains(leaked),
                "sanitized output leaked {leaked}: {sanitized}"
            );
        }
        assert!(sanitized.contains("[redacted]"));
    }

    #[test]
    fn strips_quoted_and_comma_terminated_secret_values() {
        let value = r#"password: "hunter2", secret='abc123'; api_key: sk_live, authorization: "Bearer auth123""#;
        let sanitized = sanitize_error_for_storage(value, 512);

        for leaked in ["hunter2", "abc123", "sk_live", "auth123"] {
            assert!(
                !sanitized.contains(leaked),
                "sanitized output leaked {leaked}: {sanitized}"
            );
        }
        assert!(sanitized.contains("password=[redacted]"));
        assert!(sanitized.contains("secret=[redacted]"));
        assert!(sanitized.contains("api_key=[redacted]"));
        assert!(sanitized.contains("authorization=[redacted]"));
    }
}
