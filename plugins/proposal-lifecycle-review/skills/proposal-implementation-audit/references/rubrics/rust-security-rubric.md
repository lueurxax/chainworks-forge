# Implementation Audit Addendum

Use this rubric inside `$proposal-implementation-audit`. Compare the proposal obligations, prior proposal-review findings, and actual implementation evidence. Do not fail a `REQ-*` item unless the proposal explicitly committed to that behavior; record other risks as routed specialist findings. Prefer direct code, diff, test-run, runtime, schema, migration, telemetry, benchmark, or log evidence over inference.

---

# Security Review Rubric: Rust Service Perspective

Evaluate trust boundaries and abuse resistance for a Rust service or backend proposal.

## Focus Areas

1. Authn/authz and public boundary shape
2. Secret handling and logging hygiene
3. Parsing, serialization, validation, and input hardening
4. Unsafe, FFI, and memory-adjacent risk
5. Dependency and supply-chain implications

## Output Requirements

For each finding, include:
- Finding ID
- Severity
- Evidence IDs
- Why it matters
- Recommended fix
- Acceptance criteria
- Confidence
