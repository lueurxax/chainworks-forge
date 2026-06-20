# Proposal 086 Implementation Audit R8: Provider Session Resurrection Completion

Audit date: 2026-06-20
Auditor: Codex
Target proposal: `docs/proposals/086-agent-work-continuation-and-lead-directed-session-resumption.md`
Target revision: `3168e9d93d3c7ddcb1c578c9c72953a29efff844`
Verdict: Not Implemented / Not Ready

## Scope

This audit evaluates whether Proposal 086's current text is implemented, with emphasis on the implementation tail added by the proposal: `provider_session_resurrection`, at least one supported production-relevant adapter, verified provider-session identity before prompt send, output-only repair safety, resurrection phase replay, raw attach receipt access, and a proposal gate that proves those behaviors.

The proposal itself is intentionally narrow: it explicitly excludes the already-shipped live-handle continuation path and says P093 owns soak/scale only, not provider-session resurrection implementation. Residual provider-session resurrection work is therefore in scope for P086 and cannot be deferred to P093 as closeout follow-up.

## Reviewer Selection

Prior review reuse: not reused. The review discovery helper found no reusable non-audit proposal-review artifacts for this proposal. Existing `IMPLEMENTATION_AUDIT_R*` files were treated as audit history, not reviewer-selection input.

Selected specialist lenses:

- `chainworks_execution_truth_reviewer`: run/stage/agent execution truth, MCP/ACP continuation ownership, artifact and replay boundaries.
- `rust_reliability_reviewer`: crash/replay, idempotency, worker lifecycle, fail-closed state-machine behavior.
- `api_contract_reviewer`: MCP/GraphQL/schema/readback compatibility and gate coverage.
- `observability_rollout_reviewer`: proposal gate, rollout fixtures, metrics, evidence sufficiency.
- `rust_security_reviewer`: raw provider session ids, attach receipt access control, MCP/GraphQL public surfaces, subprocess/runtime-home evidence.

Rejected lenses: `rust_arch_reviewer` was displaced by the repo-local Chainworks execution-truth reviewer under the hard cap; macOS/UI reviewers were not selected because the proposal keeps SwiftUI read-only and no Swift implementation files changed; performance/product reviewers were not selected because no performance or product-decision claim is being certified.

## Requirement Status

| Requirement | Status | Evidence |
| --- | --- | --- |
| Supported `provider_session_resurrection` flow for at least Claude | Missing | ACP adapter trait defaults to fail-closed support only (`control-plane/crates/acp/src/adapters/mod.rs:806`). MCP hard-codes all listed providers, including Claude, to unsupported (`control-plane/crates/mcp-server/src/tools/agents.rs:1114`). |
| Frozen catalog opt-in and fail-closed unsupported handling | Partially implemented | Admission checks catalog fields and rejects disabled/unsupported requests (`control-plane/crates/mcp-server/src/tools/agents.rs:1246`). The example catalog still sets `provider_session_resurrection.enabled: false` (`examples/agents/agents.yaml:2047`). |
| Versioned adapter capability and typed attach result/proof contract | Missing / partial scaffolding | There is only a boolean adapter capability hook; no typed provider-session attach request/result implementation or Claude capability version was found. |
| New managed ACP process attaches to requested provider session id before prompt | Missing | Current code rejects before adapter dispatch. No code path starts `claude-agent-acp` or proves `actual_provider_session_id == requested_provider_session_id` before prompt send. |
| Attach receipt v2 and raw access readback | Partially implemented | The v2 schema contains the required identity/process/phase/output-only fields (`docs/reference/p086/schemas/artifacts/provider_session_attach_receipt_v2.schema.json:8`). MCP/GraphQL raw access was hardened to require `run_id`, with focused tests passing, but no successful runtime receipt can be produced because resurrection never reaches attach. |
| Output-only repair over resurrected session with source-change proof | Missing | No successful resurrection path exists; required output-only repair fixtures remain placeholder evidence. |
| Resurrection phase replay and no-duplicate-prompt proof | Partially implemented | DB/readback reference docs describe `resurrection_phase` infrastructure (`docs/reference/agent-work-continuation.md:117`), but several crash/replay and timeout fixtures are still placeholders and no supported runtime path exercises them. |
| Operator-visible readback and metrics for resurrection | Partially implemented | Metrics/readback fields exist, including attach success/failure counters, but current docs say provider resurrection is unconditionally rejected until adapter enablement (`docs/reference/agent-work-continuation.md:101`). Unsupported-only counters do not satisfy the proposal's success/readback acceptance criteria. |
| Proposal gate proves new P086 resurrection requirements | Missing | `proposal-086` is documented as a retained historical live-handle continuation gate, with aliases "gate names only" (`docs/reference/test-gates.md:2457`). No `proposal-086-resurrection` gate was found. |

## Findings

### ARCH-001 Critical: The required provider-session resurrection success path is absent

Proposal 086 requires at least Claude to start a new Chainworks-managed ACP process, attach/resume the recorded provider session id, prove the new process attached to the requested id before prompt send, then settle through continuation artifacts rather than retry. The implementation still blocks the entire mode before adapter dispatch: `provider_session_resurrection_adapter_supported` returns `false` for `claude`, `claude_acp`, `claude_code`, Codex, Gemini, Auggie, and Junie (`control-plane/crates/mcp-server/src/tools/agents.rs:1114`), while the ACP adapter trait only exposes a default false boolean (`control-plane/crates/acp/src/adapters/mod.rs:806`).

Impact: the central user story cannot happen. An operator cannot continue code-writer work from a known provider session id after Chainworks has lost the live ACP handle. The current behavior is safe fail-closed scaffolding, not the implementation requested by P086.

Required fix: implement a versioned adapter capability and at least the Claude attach/resume path, including managed process launch, typed attach request/result, identity proof, mismatch/expired/missing-session failures, persisted v2 attach receipt before prompt, and terminal settlement tied to prompt-turn correlation.

### REL-001 Major: Resurrection replay, output-only repair, and identity-proof evidence are still placeholder or unreachable

The proposal requires proof for crash/replay phases, `attached_unprompted` no-prompt behavior, no duplicate prompt, output-only repair with `changed_source_files == 0`, source-edit violation handling, identity mismatch before prompt, timeout settlement, and orphan-reap verification. Current evidence still describes the mode as Phase 0/pre-4 universally blocked, or carries placeholder fixture markers for these required cases (`docs/evidence/rollout-contract/p086/negative/resurrection-unsupported-adapter.json:6`, `docs/evidence/rollout-contract/p086/negative/resurrection-before-attach-receipt.json:6`, `docs/evidence/rollout-contract/p086/negative/output-only-repair-violation-changed-source-files.fixture.json:28`, `docs/evidence/rollout-contract/p086/negative/identity-mismatch-before-prompt-fail-closed.fixture.json:28`).

Impact: the state machine and safety claims cannot be audited under a supported runtime path. The DB/readback schema may exist, but the safety-critical transitions are not proven by execution.

Required fix: replace placeholder and Phase 0-only fixtures with runtime or deterministic fake-adapter evidence that reaches each required phase without provider I/O duplication, and prove output-only repair separately from code-editing continuation.

### OPS-001 Major: The canonical P086 gate is historical, misses the new proposal scope, and is currently red

The current `proposal-086` gate validates the earlier live-handle continuation surface. Its own documentation says `proposal-086`, `p086`, and `p086-continuation-*` are retained gate names only for the stable `agent-work-continuation.md` contract (`docs/reference/test-gates.md:2457`). The gate script still checks live-session reuse and v1 attach receipt prompts (`scripts/test-gate.sh:9450`, `scripts/test-gate.sh:9604`). It does not prove a successful provider-session resurrection adapter, v2 attach receipt from a real attach, prompt-turn marker correlation, output-only repair, or the required failure classes for a supported adapter.

The gate also failed in this checkout:

```text
./scripts/test-gate.sh proposal-086
failed in control-plane/crates/daemon/tests/proposal_086_mcp_continuation_live_reuse.rs:418
called Result::unwrap() on an Err value: workspace_root contains a symlink component
```

Impact: even the retained historical gate is not green, and it would still be insufficient if fixed because it does not cover the proposal's resurrection-completion acceptance criteria.

Required fix: add or repurpose a focused resurrection gate, for example `proposal-086-resurrection`, that proves the full supported adapter path and required negative cases. Keep the historical live-handle gate if needed, but do not use it as evidence that this proposal is implemented.

### API-001 Minor: Reference documentation for attach receipt access is slightly behind the hardened schema

The dirty implementation correctly tightens MCP/GraphQL raw receipt access to require `run_id`, and focused tests passed. The reference text still lists `providerSessionAttachReceipt(continuationId: ID!, runId: ID)` and says `agents.attach_receipt.get` requires `continuation_id` while Operators also supply `run_id` (`docs/reference/agent-work-continuation.md:23`, `docs/reference/agent-work-continuation.md:48`). The GraphQL test expects `runId: ID!` (`control-plane/crates/graphql-server/tests/proposal_086_continuation_readback.rs:619`).

Impact: not a readiness blocker by itself, but closeout docs would misstate the hardened contract.

Required fix: update the reference documentation after the implementation behavior is settled.

## Security Gate

Security review was required because the touched surface includes MCP/GraphQL raw receipt access, raw provider session ids, principal-scoped authorization, subprocess/runtime-home metadata, and receipt disclosure policy. I found no new high-severity security blocker in the `run_id` hardening itself; the focused MCP tests for schema and missing-run rejection passed. However, security sign-off for the proposal cannot be granted while the primary success path is absent, because the attach identity proof, raw receipt production, child-process evidence, and output-only repair restrictions are not exercised under supported resurrection.

## Residual Scope and Follow-Up Ownership

This is not a case where small follow-up cleanup remains after a complete implementation. The unimplemented work is the proposal's core acceptance surface:

- a production-relevant supported adapter, specifically Claude;
- managed ACP process launch and provider-session attach/resume;
- identity proof before prompt;
- v2 receipt production from a real attach;
- output-only repair evidence;
- resurrection crash/replay proof;
- a resurrection-specific gate.

P093 cannot own this residual work because P086 explicitly says P093 owns soak/scale only and does not own provider-session resurrection implementation.

## Verification Performed

- `CARGO_TARGET_DIR=target/proposal-086-gate cargo test -p mcp-server provider_session_resurrection_adapter_support_fails_closed_until_attach_is_proven -- --nocapture` passed.
- `CARGO_TARGET_DIR=target/proposal-086-gate cargo test -p mcp-server attach_receipt_get_ -- --nocapture` passed.
- `./scripts/test-gate.sh proposal-086` failed in the daemon live ACP reuse regression with `workspace_root contains a symlink component`.
- Static inspection found no `proposal-086-resurrection` gate and no adapter override for `supports_provider_session_resurrection`.

## Closeout Decision

Do not close out or retire Proposal 086. The repository currently contains useful fail-closed scaffolding, persistence/readback schema, raw receipt access hardening, and historical live-handle continuation coverage, but it does not implement the provider-session resurrection completion requested by the proposal.
