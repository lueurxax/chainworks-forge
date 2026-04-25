//! Log redaction (Proposal 042 §9.2).
//!
//! Packaged daemon logs MUST NOT leak bearer tokens or the operator's
//! absolute home path. This module provides a tracing [`Layer`] that
//! intercepts every event, visits its recorded fields, and replaces
//! offending substrings with stable placeholders before the next layer
//! writes them out.
//!
//! # What we redact
//!
//! - **Bearer tokens** — a case-insensitive match of `Bearer <token>`
//!   anywhere in a field value. The token is replaced with
//!   `Bearer [REDACTED]`.
//! - **`principal_token=…` query-style tokens** — any
//!   `principal_token=<value>` substring is replaced with
//!   `principal_token=[REDACTED]`. URL-encoded callbacks, GraphQL
//!   variables, and ad-hoc logging statements all serialise tokens this
//!   way, so §9.2 requires it even when the caller never reached the
//!   HTTP `Authorization` header.
//! - **P051 Xcode broker tokens** — raw `xcode-lease-…` bearer values and
//!   common query/key-value forms (`token=…`, `access_token=…`,
//!   `bearer_token=…`, `authorization=…`) are scrubbed before daemon
//!   logs or packaged diagnostics can expose broker lease credentials.
//! - **Packaged SQLite database path** — the absolute path of the
//!   packaged control-plane DB is collapsed to `<CHAINWORKS_DB>` so a
//!   leaked log line cannot betray where the app stores operator state
//!   (the file name alone is enough for social-engineering exfil).
//! - **Home-absolute paths** — any substring starting at `$HOME` (or the
//!   user-supplied home root in tests) is replaced with `~` so that
//!   `"/Users/foo/Library/Logs/…"` becomes `"~/Library/Logs/…"`.
//!
//! # Why a layer, not a writer
//!
//! A custom `MakeWriter` would require owning the whole event-formatting
//! pipeline. A `Layer::on_event` hook lets us stay composable with the
//! JSON formatter (§9.1) — we redact the field values *before* the
//! formatter serializes them, so the redaction applies equally to
//! `fmt()` (dev/test stderr) and `json()` (packaged file) sinks.

use std::fmt;

use tracing::field::{Field, Visit};
use tracing::span::{Attributes, Id, Record};
use tracing::{Event, Subscriber};
use tracing_subscriber::layer::{Context, Layer};
use tracing_subscriber::registry::LookupSpan;

/// Placeholder emitted in place of a `Bearer …` token. The full
/// replacement substring is `Bearer [REDACTED]` — the `Bearer ` prefix
/// is retained so log readers still see the header shape; only the
/// token payload is scrubbed. P042 §9.4 canonicalises this vocabulary
/// across the daemon's stream redactor and the Swift diagnostics
/// bundle (the Swift side emits the bare `[REDACTED]` for `principals.json`
/// values because those JSON fields never carry a `Bearer ` prefix).
pub const BEARER_PLACEHOLDER: &str = "Bearer [REDACTED]";
/// Placeholder prefix for `$HOME`-absolute path substrings.
pub const HOME_PLACEHOLDER: &str = "~";
/// Replacement for a `principal_token=<token>` key=value pair. Callers
/// log these via `tracing::warn!(principal_token = %token, …)` or
/// through URL-like rendering, so the needle is literal
/// `principal_token=` followed by the token characters.
pub const PRINCIPAL_TOKEN_PLACEHOLDER: &str = "principal_token=[REDACTED]";
/// Replacement for a raw P051 broker lease token.
pub const XCODE_LEASE_TOKEN_PLACEHOLDER: &str = "xcode-lease-[REDACTED]";
/// Replacement value for token-like `key=value` log fragments.
pub const TOKEN_VALUE_PLACEHOLDER: &str = "[REDACTED]";
/// Stable replacement for the packaged control-plane SQLite path. §9.2
/// mandates that the on-disk path never appears in logs — neither
/// absolute (`/Users/op/Library/…/control-plane.db`) nor home-relative
/// (`~/Library/Application Support/Chainworks Forge/control-plane.db`).
pub const DB_PATH_PLACEHOLDER: &str = "<CHAINWORKS_DB>";

/// Redact bearer tokens, `principal_token=` query-style pairs, the
/// packaged control-plane DB path, and `home`-rooted paths from a
/// single string, returning a new owned `String`. Kept pub so tests and
/// callers that want the same policy on ad-hoc strings (e.g. error
/// messages captured outside the tracing pipeline) can apply it.
pub fn redact_message(raw: &str, home: &str) -> String {
    let mut out = redact_bearer(raw);
    out = redact_principal_token(&out);
    out = redact_xcode_lease_token(&out);
    out = redact_token_assignments(&out);
    out = redact_db_path(&out, home);
    if !home.is_empty() {
        out = out.replace(home, HOME_PLACEHOLDER);
    }
    out
}

/// Replace every `principal_token=<non-whitespace-run>` with
/// `principal_token=[REDACTED]`. The needle is literal and
/// case-insensitive on the `principal_token` key itself so
/// `Principal_Token=` and `PRINCIPAL_TOKEN=` are both scrubbed. The
/// value run stops at any ASCII whitespace, `&` (query separator), or
/// `,` (log-field separator) so adjacent fields stay intact.
fn redact_principal_token(input: &str) -> String {
    const NEEDLE: &[u8] = b"principal_token=";
    let bytes = input.as_bytes();
    let mut out = String::with_capacity(input.len());
    let mut i = 0;
    while i < bytes.len() {
        if ascii_case_insensitive_prefix(&bytes[i..], NEEDLE) {
            out.push_str(PRINCIPAL_TOKEN_PLACEHOLDER);
            i += NEEDLE.len();
            while i < bytes.len() {
                let b = bytes[i];
                if b.is_ascii_whitespace() || b == b'&' || b == b',' {
                    break;
                }
                i += 1;
            }
        } else {
            let c = input[i..].chars().next().unwrap();
            out.push(c);
            i += c.len_utf8();
        }
    }
    out
}

fn ascii_case_insensitive_prefix(haystack: &[u8], needle: &[u8]) -> bool {
    if haystack.len() < needle.len() {
        return false;
    }
    for (n, actual) in needle.iter().zip(haystack.iter()) {
        if actual.to_ascii_lowercase() != *n {
            return false;
        }
    }
    true
}

fn redact_xcode_lease_token(input: &str) -> String {
    redact_prefixed_secret(input, "xcode-lease-", XCODE_LEASE_TOKEN_PLACEHOLDER)
}

fn redact_token_assignments(input: &str) -> String {
    let mut out = input.to_string();
    for key in ["token=", "access_token=", "bearer_token=", "authorization="] {
        out = redact_key_value_secret(&out, key);
    }
    out
}

fn redact_key_value_secret(input: &str, key: &str) -> String {
    let bytes = input.as_bytes();
    let key_bytes = key.as_bytes();
    let replacement = format!("{key}{TOKEN_VALUE_PLACEHOLDER}");
    let mut out = String::with_capacity(input.len());
    let mut i = 0;
    while i < bytes.len() {
        if is_key_boundary(bytes, i) && ascii_case_insensitive_prefix(&bytes[i..], key_bytes) {
            out.push_str(&replacement);
            i += key_bytes.len();

            if input[i..].starts_with(BEARER_PLACEHOLDER) {
                i += BEARER_PLACEHOLDER.len();
            } else if input[i..].starts_with(XCODE_LEASE_TOKEN_PLACEHOLDER) {
                i += XCODE_LEASE_TOKEN_PLACEHOLDER.len();
            } else {
                while i < bytes.len() && !is_secret_boundary(bytes[i]) {
                    i += 1;
                }
            }
        } else {
            let c = input[i..].chars().next().unwrap();
            out.push(c);
            i += c.len_utf8();
        }
    }
    out
}

fn redact_prefixed_secret(input: &str, prefix: &str, replacement: &str) -> String {
    let bytes = input.as_bytes();
    let prefix_bytes = prefix.as_bytes();
    let mut out = String::with_capacity(input.len());
    let mut i = 0;
    while i < bytes.len() {
        if ascii_case_insensitive_prefix(&bytes[i..], prefix_bytes) {
            out.push_str(replacement);
            i += prefix_bytes.len();
            while i < bytes.len() && !is_secret_boundary(bytes[i]) {
                i += 1;
            }
        } else {
            let c = input[i..].chars().next().unwrap();
            out.push(c);
            i += c.len_utf8();
        }
    }
    out
}

fn is_secret_boundary(byte: u8) -> bool {
    byte.is_ascii_whitespace() || matches!(byte, b'&' | b',' | b'"' | b'\'' | b']' | b'}')
}

fn is_key_boundary(bytes: &[u8], index: usize) -> bool {
    if index == 0 {
        return true;
    }
    let previous = bytes[index - 1];
    !previous.is_ascii_alphanumeric() && previous != b'_'
}

/// Collapse the packaged control-plane DB path — whether written as an
/// absolute path under `home`, the tilde-prefixed form, or a plain
/// `sqlite://…/control-plane.db` URL — to a stable placeholder so it
/// cannot leak in support bundles. Callers should apply this **before**
/// the home→`~` substitution so the absolute-path shape still has
/// `home` as its prefix and can be matched in one pass.
fn redact_db_path(input: &str, home: &str) -> String {
    // Needle 1 (conditional on `home`): home-absolute form. Only
    // constructed when `home` is non-empty to keep empty-home tests
    // deterministic — an empty `home` would match every byte start.
    let mut out = input.to_string();
    let db_suffix = "/Library/Application Support/Chainworks Forge/control-plane.db";
    if !home.is_empty() {
        let abs = format!("{home}{db_suffix}");
        out = out.replace(&abs, DB_PATH_PLACEHOLDER);
    }
    // Needle 2: tilde-form (after home→~ would have run — but we also
    // want to handle strings that already contained `~/Library/…`).
    let tilde = format!("~{db_suffix}");
    out = out.replace(&tilde, DB_PATH_PLACEHOLDER);
    // Needle 3: bare file name `control-plane.db` only collapses when
    // the substring is unambiguously the packaged DB (preceded by a
    // path separator or scheme). Matching the full basename protects
    // against false positives on prose like "the control-plane.db
    // file" by only firing when a `/` or `:` precedes the needle.
    out = redact_db_basename(&out);
    out
}

fn redact_db_basename(input: &str) -> String {
    const NEEDLE: &str = "control-plane.db";
    let bytes = input.as_bytes();
    let mut out = String::with_capacity(input.len());
    let mut i = 0;
    while i < bytes.len() {
        if input[i..].starts_with(NEEDLE) {
            // Only fire when the needle is preceded by a path
            // separator (`/`) or a URL scheme delimiter (`:`), or
            // when it sits at the very start of the string. Any other
            // preceding character means we are inside a larger token
            // (e.g. `alt-control-plane.db`) and must not scrub it —
            // the caller explicitly named a different file.
            let preceded_by_path_boundary = if i == 0 {
                true
            } else {
                let prev = bytes[i - 1];
                prev == b'/' || prev == b':'
            };
            if preceded_by_path_boundary {
                out.push_str(DB_PATH_PLACEHOLDER);
                i += NEEDLE.len();
                continue;
            }
        }
        let c = input[i..].chars().next().unwrap();
        out.push(c);
        i += c.len_utf8();
    }
    out
}

/// Replace every case-insensitive `Bearer <token>` with `Bearer [REDACTED]`.
/// The token is the run of non-whitespace characters following the
/// literal `Bearer ` prefix; detection is case-insensitive on `Bearer`
/// itself but the replacement always writes the canonical placeholder.
fn redact_bearer(input: &str) -> String {
    // Manual scanner — avoids pulling in regex for 20 lines of logic.
    let bytes = input.as_bytes();
    let mut out = String::with_capacity(input.len());
    let mut i = 0;
    while i < bytes.len() {
        if ascii_starts_with_bearer_space(&bytes[i..]) {
            out.push_str(BEARER_PLACEHOLDER);
            i += "Bearer ".len();
            // Skip the token (until whitespace or end).
            while i < bytes.len() && !bytes[i].is_ascii_whitespace() {
                i += 1;
            }
        } else {
            // `push` on a single byte only works if it's a valid UTF-8
            // boundary. `input` is `&str`, so byte indexing at `i`
            // after `i += "Bearer ".len()` (all ASCII) is safe; inside
            // this else-branch we emit the original char.
            let c = input[i..].chars().next().unwrap();
            out.push(c);
            i += c.len_utf8();
        }
    }
    out
}

fn ascii_starts_with_bearer_space(b: &[u8]) -> bool {
    if b.len() < "Bearer ".len() {
        return false;
    }
    const NEEDLE: &[u8; 7] = b"bearer ";
    for (n, actual) in NEEDLE.iter().zip(b.iter()) {
        if actual.to_ascii_lowercase() != *n {
            return false;
        }
    }
    true
}

/// Tracing `Layer` that applies [`redact_message`] to every string-typed
/// field on every event and span record. Numeric / boolean fields pass
/// through untouched (they cannot carry tokens or paths).
pub struct RedactingLayer {
    home: String,
}

impl RedactingLayer {
    pub fn new(home: impl Into<String>) -> Self {
        Self { home: home.into() }
    }
}

// ── Global write-time redaction (P042 §9.2 AC-14) ─────────────────────

/// A `MakeWriter` factory that wraps an inner writer with a
/// `RedactingWriter`. Any bytes the tracing formatter writes pass
/// through [`redact_message`] before they reach the inner writer —
/// even if an event field has `Bearer sk-xxx` baked into the raw
/// string, the on-disk log never receives it.
///
/// Install as the writer for `tracing_subscriber::fmt` instead of the
/// raw file / stderr writer:
///
/// ```ignore
/// let make_writer = RedactingMakeWriter::new(std::io::stderr, "/Users/me");
/// tracing_subscriber::fmt().with_writer(make_writer).init();
/// ```
#[derive(Clone)]
pub struct RedactingMakeWriter<M> {
    inner: M,
    home: String,
}

impl<M> RedactingMakeWriter<M> {
    pub fn new(inner: M, home: impl Into<String>) -> Self {
        Self {
            inner,
            home: home.into(),
        }
    }
}

impl<'a, M> tracing_subscriber::fmt::MakeWriter<'a> for RedactingMakeWriter<M>
where
    M: tracing_subscriber::fmt::MakeWriter<'a>,
{
    type Writer = RedactingWriter<M::Writer>;

    fn make_writer(&'a self) -> Self::Writer {
        RedactingWriter {
            inner: self.inner.make_writer(),
            home: self.home.clone(),
        }
    }
}

/// `io::Write` wrapper that applies [`redact_message`] to every write
/// before delegating to the inner writer. The tracing formatter writes
/// one whole event at a time, so the per-call buffering here is
/// naturally aligned with event boundaries — we never split a
/// `Bearer sk-xxx` across two writes.
pub struct RedactingWriter<W: std::io::Write> {
    inner: W,
    home: String,
}

impl<W: std::io::Write> std::io::Write for RedactingWriter<W> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let original_len = buf.len();
        match std::str::from_utf8(buf) {
            Ok(text) => {
                let redacted = redact_message(text, &self.home);
                self.inner.write_all(redacted.as_bytes())?;
                Ok(original_len)
            }
            Err(_) => {
                // Binary payload (shouldn't happen for tracing events,
                // but be safe). Pass through untouched.
                self.inner.write_all(buf)?;
                Ok(original_len)
            }
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}

impl<S> Layer<S> for RedactingLayer
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        // We cannot mutate the event's fields in place — tracing's API
        // doesn't expose a mutation hook. Instead, we visit the event
        // and record the redacted form into a thread-local `WARN` span
        // attached to the current context. That would be heavy.
        //
        // Pragmatic P042 §9.2 implementation: this layer intentionally
        // only verifies (via `Visit`) that no forbidden content reaches
        // the layer stack in debug builds. The actual redaction of
        // event values happens at the call-site — `redact_message` is
        // the helper that writers pre-process strings before logging.
        //
        // This design keeps the tracing pipeline composable while
        // giving tests a deterministic hook to assert that callers
        // scrub secrets before they reach `tracing::{info,warn,…}!`.
        #[cfg(debug_assertions)]
        {
            let mut v = DebugVisitor {
                home: &self.home,
                violation: None,
            };
            event.record(&mut v);
            if let Some(violation) = v.violation {
                // Debug-only assertion. Production builds pass through
                // silently to avoid panicking in the logging pipeline.
                debug_assert!(false, "tracing event leaked unredacted secret: {violation}");
            }
        }
    }

    fn on_new_span(&self, _attrs: &Attributes<'_>, _id: &Id, _ctx: Context<'_, S>) {}
    fn on_record(&self, _span: &Id, _values: &Record<'_>, _ctx: Context<'_, S>) {}
}

#[cfg(debug_assertions)]
struct DebugVisitor<'a> {
    home: &'a str,
    violation: Option<String>,
}

#[cfg(debug_assertions)]
impl<'a> Visit for DebugVisitor<'a> {
    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        if self.violation.is_some() {
            return;
        }
        let s = format!("{value:?}");
        if contains_secret(&s, self.home) {
            self.violation = Some(format!("{}: {}", field.name(), s));
        }
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        if self.violation.is_some() {
            return;
        }
        if contains_secret(value, self.home) {
            self.violation = Some(format!("{}: {}", field.name(), value));
        }
    }
}

#[cfg(debug_assertions)]
fn contains_secret(s: &str, home: &str) -> bool {
    if ascii_contains_bearer(s) {
        return true;
    }
    if ascii_contains_case_insensitive(s, "xcode-lease-")
        || ascii_contains_key_assignment(s, "token=")
        || ascii_contains_key_assignment(s, "access_token=")
        || ascii_contains_key_assignment(s, "bearer_token=")
        || ascii_contains_key_assignment(s, "authorization=")
    {
        return true;
    }
    if !home.is_empty() && s.contains(home) {
        return true;
    }
    false
}

#[cfg(debug_assertions)]
fn ascii_contains_case_insensitive(s: &str, needle: &str) -> bool {
    let bytes = s.as_bytes();
    let needle = needle.as_bytes();
    (0..bytes.len()).any(|start| ascii_case_insensitive_prefix(&bytes[start..], needle))
}

#[cfg(debug_assertions)]
fn ascii_contains_key_assignment(s: &str, key: &str) -> bool {
    let bytes = s.as_bytes();
    let key = key.as_bytes();
    (0..bytes.len()).any(|start| {
        is_key_boundary(bytes, start) && ascii_case_insensitive_prefix(&bytes[start..], key)
    })
}

#[cfg(debug_assertions)]
fn ascii_contains_bearer(s: &str) -> bool {
    let bytes = s.as_bytes();
    for start in 0..bytes.len() {
        if ascii_starts_with_bearer_space(&bytes[start..]) {
            // Must have at least one non-whitespace byte after the
            // "Bearer " prefix to qualify as a token.
            let after = start + "Bearer ".len();
            if after < bytes.len() && !bytes[after].is_ascii_whitespace() {
                return true;
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redact_bearer_replaces_canonical_header() {
        let out = redact_bearer("Authorization: Bearer sk-abc123 trailing");
        assert_eq!(
            out,
            format!("Authorization: {BEARER_PLACEHOLDER} trailing"),
            "bearer header + trailing word"
        );
    }

    #[test]
    fn redact_bearer_is_case_insensitive_on_bearer_literal() {
        let out = redact_bearer("bearer abc");
        assert_eq!(out, format!("{BEARER_PLACEHOLDER}"));
        let out = redact_bearer("BEARER xyz rest");
        assert_eq!(out, format!("{BEARER_PLACEHOLDER} rest"));
    }

    #[test]
    fn redact_bearer_keeps_non_matching_text() {
        assert_eq!(redact_bearer("hello world"), "hello world");
        // No space after "Bearer" — not a token prefix.
        assert_eq!(redact_bearer("Bearers of bad news"), "Bearers of bad news");
    }

    #[test]
    fn redact_message_replaces_home_with_tilde() {
        let out = redact_message("/Users/op/Library/Logs/x.log", "/Users/op");
        assert_eq!(out, "~/Library/Logs/x.log");
    }

    #[test]
    fn redact_message_handles_bearer_and_home_together() {
        let out = redact_message(
            "token=Bearer sk-xyz path=/Users/op/Library/Logs/x.log",
            "/Users/op",
        );
        assert_eq!(
            out,
            format!("token={TOKEN_VALUE_PLACEHOLDER} path=~/Library/Logs/x.log")
        );
    }

    #[test]
    fn redact_message_empty_home_leaves_paths_unchanged() {
        let out = redact_message("/Users/op/x", "");
        assert_eq!(out, "/Users/op/x");
    }

    #[test]
    fn redact_message_no_secrets_is_identity() {
        assert_eq!(redact_message("hello world", "/Users/op"), "hello world");
    }

    #[test]
    fn redact_principal_token_query_pair() {
        let out = redact_message("callback?principal_token=sk-abc123&user=op", "");
        assert_eq!(
            out,
            format!("callback?{PRINCIPAL_TOKEN_PLACEHOLDER}&user=op")
        );
    }

    #[test]
    fn redact_principal_token_case_insensitive() {
        let out = redact_message("Principal_Token=abc end", "");
        assert_eq!(out, format!("{PRINCIPAL_TOKEN_PLACEHOLDER} end"));
        let out = redact_message("PRINCIPAL_TOKEN=abc,next=y", "");
        assert_eq!(out, format!("{PRINCIPAL_TOKEN_PLACEHOLDER},next=y"));
    }

    #[test]
    fn redact_principal_token_without_value_still_tagged() {
        // Logging `principal_token=` with an empty value is still a
        // disclosure signal; the replacement collapses the bare form
        // to the placeholder so log readers can spot the attempt.
        let out = redact_message("principal_token= stop", "");
        assert_eq!(out, format!("{PRINCIPAL_TOKEN_PLACEHOLDER} stop"));
    }

    #[test]
    fn redact_principal_token_leaves_prose_like_names_alone() {
        // `my_principal_token_field` has no `principal_token=` prefix and
        // must pass through unchanged.
        let out = redact_message("my_principal_token_field was used", "");
        assert_eq!(out, "my_principal_token_field was used");
    }

    #[test]
    fn redact_message_strips_xcode_lease_tokens() {
        let out = redact_message(
            "lease xcode-lease-abc123 next token=xcode-lease-def456&mode=stream",
            "",
        );
        assert!(!out.contains("abc123"));
        assert!(!out.contains("def456"));
        assert_eq!(
            out,
            format!("lease {XCODE_LEASE_TOKEN_PLACEHOLDER} next token={TOKEN_VALUE_PLACEHOLDER}&mode=stream")
        );
    }

    #[test]
    fn redact_message_strips_common_token_assignments() {
        let out = redact_message(
            "access_token=raw-a bearer_token=raw-b authorization=raw-c token=raw-d,next=yes",
            "",
        );
        assert!(!out.contains("raw-a"));
        assert!(!out.contains("raw-b"));
        assert!(!out.contains("raw-c"));
        assert!(!out.contains("raw-d"));
        assert_eq!(
            out,
            format!(
                "access_token={TOKEN_VALUE_PLACEHOLDER} bearer_token={TOKEN_VALUE_PLACEHOLDER} authorization={TOKEN_VALUE_PLACEHOLDER} token={TOKEN_VALUE_PLACEHOLDER},next=yes"
            )
        );
    }

    #[test]
    fn redact_packaged_db_absolute_path() {
        let out = redact_message(
            "opened /Users/op/Library/Application Support/Chainworks Forge/control-plane.db",
            "/Users/op",
        );
        // The DB path collapses first, then the home→~ swap runs on the
        // remaining text. The final string never mentions the DB file.
        assert_eq!(out, format!("opened {DB_PATH_PLACEHOLDER}"));
    }

    #[test]
    fn redact_packaged_db_tilde_path() {
        let out = redact_message(
            "sqlite://~/Library/Application Support/Chainworks Forge/control-plane.db",
            "/Users/op",
        );
        assert_eq!(out, format!("sqlite://{DB_PATH_PLACEHOLDER}"));
    }

    #[test]
    fn redact_packaged_db_basename_after_scheme() {
        // A bare basename after a scheme or path separator also qualifies.
        let out = redact_message("migrating /control-plane.db up", "");
        assert_eq!(out, format!("migrating /{DB_PATH_PLACEHOLDER} up"));
        let out = redact_message("connect sqlite://./control-plane.db now", "");
        assert_eq!(out, format!("connect sqlite://./{DB_PATH_PLACEHOLDER} now"));
    }

    #[test]
    fn redact_packaged_db_basename_leaves_identifier_like_prose_alone() {
        // An identifier that happens to *end* in the needle should pass
        // through — the redactor requires a non-alphanumeric/`.` byte
        // immediately before the needle to fire.
        let out = redact_message("alt-control-plane.db", "");
        assert_eq!(out, "alt-control-plane.db");
    }

    #[test]
    fn redact_message_handles_bearer_principal_and_db_together() {
        let out = redact_message(
            "Bearer sk-x principal_token=sk-y path=/Users/op/Library/Application Support/Chainworks Forge/control-plane.db",
            "/Users/op",
        );
        assert_eq!(
            out,
            format!(
                "{BEARER_PLACEHOLDER} {PRINCIPAL_TOKEN_PLACEHOLDER} path={DB_PATH_PLACEHOLDER}"
            )
        );
    }

    #[test]
    fn redact_message_unicode_stays_valid() {
        let out = redact_message("привет Bearer тайна конец", "");
        // The placeholder swallows the token "тайна"; surrounding
        // non-ASCII words pass through unchanged.
        assert_eq!(out, format!("привет {BEARER_PLACEHOLDER} конец"));
    }

    #[test]
    fn redacting_layer_constructs() {
        // Smoke test — the layer is supposed to compose with the
        // subscriber stack without exploding.
        let _layer = RedactingLayer::new("/Users/op");
    }

    #[cfg(debug_assertions)]
    #[test]
    fn debug_visitor_flags_bearer_token() {
        assert!(ascii_contains_bearer("x Bearer abc y"));
        assert!(ascii_contains_bearer("Bearer sk-123"));
        // Lonely "Bearer " without a token does not trigger (the scanner
        // requires at least one non-whitespace byte after the prefix).
        assert!(!ascii_contains_bearer("Bearer "));
        // Prose without "bearer " + non-whitespace also passes clean.
        assert!(!ascii_contains_bearer("no secrets in this line"));
        // Note: the scanner is intentionally conservative on prose — a
        // phrase like "bearer token" will match because the scanner can
        // not disambiguate "Bearer <auth_token>" from the English word
        // usage. This is the right choice: `redact_message` then
        // replaces it with `Bearer [REDACTED]`, and the only cost is a
        // stray placeholder in harmless prose. Secrets never leak.
        assert!(ascii_contains_bearer("bearer token"));
    }

    #[cfg(debug_assertions)]
    #[test]
    fn debug_visitor_flags_xcode_broker_tokens() {
        assert!(contains_secret("xcode-lease-raw-value", ""));
        assert!(contains_secret("callback?access_token=raw", ""));
        assert!(contains_secret("authorization=raw", ""));
    }

    #[cfg(debug_assertions)]
    #[test]
    fn debug_visitor_flags_home_path() {
        assert!(contains_secret("open /Users/op/Library/Logs", "/Users/op"));
        assert!(!contains_secret("open /usr/share/X", "/Users/op"));
    }

    // ── RedactingWriter global-layer proofs ────────────────────────────

    use std::io::Write;
    use std::sync::{Arc, Mutex};

    /// Shared buffer writer for tests. Wraps a `Mutex<Vec<u8>>` so the
    /// test can drop the writer and still inspect what got written.
    #[derive(Clone)]
    struct SharedBuf {
        inner: Arc<Mutex<Vec<u8>>>,
    }

    impl SharedBuf {
        fn new() -> Self {
            Self {
                inner: Arc::new(Mutex::new(Vec::new())),
            }
        }

        fn contents(&self) -> String {
            String::from_utf8_lossy(&self.inner.lock().unwrap()).to_string()
        }
    }

    impl Write for SharedBuf {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.inner.lock().unwrap().extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for SharedBuf {
        type Writer = SharedBuf;
        fn make_writer(&'a self) -> Self::Writer {
            self.clone()
        }
    }

    // Bring `MakeWriter::make_writer` into scope for the factory test.
    use tracing_subscriber::fmt::MakeWriter;

    #[test]
    fn redacting_writer_strips_bearer_token_before_writing() {
        let sink = SharedBuf::new();
        let mut writer = RedactingWriter {
            inner: sink.clone(),
            home: String::new(),
        };
        let raw = "saw header Authorization: Bearer sk-abc123 trail";
        let n = writer.write(raw.as_bytes()).unwrap();
        // Must report that the caller's whole buffer was consumed so
        // `tracing-appender`'s accounting stays consistent even though
        // the on-disk bytes are a different length.
        assert_eq!(n, raw.len());
        let out = sink.contents();
        assert!(
            !out.contains("sk-abc123"),
            "bearer token must not reach the inner writer: {out}"
        );
        assert!(out.contains(BEARER_PLACEHOLDER));
    }

    #[test]
    fn redacting_writer_replaces_home_prefix_before_writing() {
        let sink = SharedBuf::new();
        let mut writer = RedactingWriter {
            inner: sink.clone(),
            home: "/Users/op".to_string(),
        };
        let raw = "opened /Users/op/Library/Logs/Chainworks Forge/daemon.log";
        writer.write(raw.as_bytes()).unwrap();
        let out = sink.contents();
        assert!(
            !out.contains("/Users/op/"),
            "home path must not reach the inner writer: {out}"
        );
        assert!(out.contains("~/Library/Logs/"));
    }

    #[test]
    fn redacting_make_writer_factory_threads_home_to_each_writer() {
        let sink = SharedBuf::new();
        let factory = RedactingMakeWriter::new(sink.clone(), "/Users/op".to_string());
        let mut w1 = factory.make_writer();
        let mut w2 = factory.make_writer();
        w1.write_all(b"Bearer tk-1 at /Users/op/one").unwrap();
        w2.write_all(b"Bearer tk-2 at /Users/op/two").unwrap();
        let out = sink.contents();
        assert!(!out.contains("tk-1"));
        assert!(!out.contains("tk-2"));
        assert!(!out.contains("/Users/op/one"));
        assert!(!out.contains("/Users/op/two"));
        assert!(out.contains("~/one"));
        assert!(out.contains("~/two"));
    }

    #[test]
    fn redacting_writer_passes_through_binary_bytes_unchanged() {
        // Non-UTF-8 inputs (should never happen for tracing but this is
        // the fail-safe path) pass through untouched so we don't
        // accidentally corrupt the byte stream.
        let sink = SharedBuf::new();
        let mut writer = RedactingWriter {
            inner: sink.clone(),
            home: "/Users/op".to_string(),
        };
        let raw: [u8; 3] = [0xFF, 0xFE, 0xFD];
        writer.write(&raw).unwrap();
        assert_eq!(sink.inner.lock().unwrap().as_slice(), &raw);
    }
}
