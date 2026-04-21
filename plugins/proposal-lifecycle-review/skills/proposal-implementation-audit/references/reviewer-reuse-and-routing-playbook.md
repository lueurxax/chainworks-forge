# Reviewer Reuse and Routing Playbook

Implementation audit routing has two jobs:

1. Preserve the proposal-review reviewer choices when they still match the implementation.
2. Add or remove reviewers when current implementation evidence proves the old route is stale.

## Reuse First, But Verify

A prior proposal review is useful because it already fingerprinted the proposal. Start there. Then check current implementation evidence.

Good reuse examples:

- Proposal review selected `rust_arch_reviewer` and `api_contract_reviewer`; implementation changes only Rust crate boundaries and proto-generated code. Reuse exactly.
- Proposal review selected `ios_ui_reviewer`, `apple_ux_reviewer`, and `apple_arch_reviewer`; implementation changes SwiftUI view state and navigation only. Reuse exactly.

Good delta examples:

- Prior reviewers: `go_service_arch_reviewer`, `api_contract_reviewer`. Implementation adds JWT middleware. Add `go_security_reviewer`.
- Prior reviewers: `rust_arch_reviewer`. Implementation adds queue retries and shutdown handling. Add `rust_reliability_reviewer`.
- Prior reviewers: `api_contract_reviewer`. Implementation adds DB migration and feature flag. Add `observability_rollout_reviewer`.

Good removal examples:

- Prior selected `product_reviewer` for proposal completeness, but implementation audit target is a mechanical schema compatibility fix and no metrics/decision gate is touched. Drop product and explain.
- Prior selected `ios_ui_reviewer`, but implementation landed only backend contract code and the client work is out of scope for this audit. Drop UI and explain.

## Hard Cap Handling

The hard cap is 5 reviewers. If reuse plus delta exceeds 5:

1. Keep reviewers tied to explicit proposal requirements.
2. Keep reviewers tied to security, data migration, public API, auth, or reliability-critical implementation risks.
3. Drop reviewers that only cover optional polish or unchanged surfaces.
4. Record rejected close alternatives.

## Under-Routing Signals

Add a reviewer or reroute when evidence shows:

- changed files are in a stack not present in prior routing
- auth/security/PII/secret handling appears without a security reviewer
- retry/idempotency/shutdown/backpressure appears without a reliability reviewer
- `.proto`, OpenAPI, schema, generated client/server code appears without API contract review
- migrations/flags/telemetry/deploy config appears without rollout/ops review
- UI runtime behavior appears without platform UI/UX review
- benchmark/performance claims appear without performance review

## Over-Routing Signals

Drop or reject reviewers when:

- they were selected only because a keyword appears in docs but no implementation surface changed
- their surface is explicitly out of scope for this implementation target
- their concerns are fully covered by a more specific reviewer
- product review is not tied to metrics, decision gates, or core user value
