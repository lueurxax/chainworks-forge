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
    let tokens: Vec<&str> = s.split_whitespace().collect();
    let mut out = Vec::with_capacity(tokens.len());
    let mut redact_next = false;

    for token in &tokens {
        let lower = token.to_ascii_lowercase();
        let normalized = lower.trim_matches(|c: char| matches!(c, '"' | '\'' | ',' | ';' | ':'));

        if redact_next {
            out.push("[redacted]".to_string());
            redact_next = false;
            continue;
        }

        if normalized == "bearer" || normalized == "token" {
            out.push(token.to_string());
            redact_next = true;
            continue;
        }

        if let Some(sep) = token.find('=').or_else(|| token.find(':')) {
            let key = token[..sep].to_ascii_lowercase();
            if key.contains("token")
                || key.contains("api_key")
                || key.contains("apikey")
                || key.contains("secret")
                || key.contains("password")
                || key.contains("authorization")
            {
                out.push(format!("{}=[redacted]", &token[..sep]));
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
            continue;
        }

        out.push(token.to_string());
    }

    out.join(" ")
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
}
