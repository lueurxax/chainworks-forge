# Proposal 086 Implementation Audit R7

## Metadata

| Field | Value |
| --- | --- |
| Proposal | `docs/proposals/086-agent-work-continuation-and-lead-directed-session-resumption.md` |
| Proposal title | Proposal 086: Provider Session Resurrection Completion |
| Proposal state | Draft |
| Audit type | `proposal-implementation-audit` |
| Report version | R7 |
| Audit timestamp | 2026-06-20T06:30:15Z |
| Repository | `/Users/user/Documents/Chainworks Forge` |
| Audited target | Current working tree |
| Base revision | `4c3dce2c5f70cd7dc540887979d399f7d354c59d` |
| Worktree state | Dirty; staged and unstaged implementation changes present |
| Final conformance verdict | Not Implemented |
| Final implementation readiness | Not Ready |
| Audit confidence | High for primary blockers; medium for downstream behavior hidden behind the failing migration gate |

## Target And Base

This audit evaluates the same-tree implementation currently present in the working tree at base revision `4c3dce2c5f70cd7dc540887979d399f7d354c59d`. The tree is not clean. The audit intentionally treats staged and unstaged files as the target implementation because the requested task was an implementation audit of the proposal against the current repository state.

No implementation files, proposal files, or prior audit reports were modified by this audit. This report is the only intended output.

## Prior Review Reuse

Prior proposal-review artifacts were not found for Proposal 086 by the review-discovery helper. Existing implementation audit reports R1 through R6 were deliberately not reused for reviewer selection, per the audit skill rule that only prior proposal-review artifacts count for reviewer reuse.

Reviewer reuse status: Not reused.

## Proposal Contract

Proposal 086 is not a live-handle continuation cleanup proposal. Its active scope is provider session resurrection completion: starting a new Chainworks-managed ACP process and attaching/resuming a known provider session id for adapters that support it, while failing closed for adapters that do not.

The proposal explicitly says the already-shipped baseline is not the remaining work:

- `agents.continue_work`
- live-handle continuation
- continuation rows, artifacts, metrics, readback, and GraphQL
- lead-auto continuation limited to live handles
- unsupported `provider_session_resurrection` fail-closed behavior
- Swift read-only surfaces

The proposal also states that P093 owns soak only, not the implementation of provider-session resurrection. Therefore "still unsupported" or "future gated" cannot be treated as acceptable completion for P086 unless the proposal itself excludes that scope. It does not.

The required successful path is:

1. `agents.continue_work` accepts `provider_session_resurrection` only when the frozen run catalog opted into that mode and the adapter declares supported resurrection capability.
2. Claude is the first required production-relevant supported adapter.
3. Chainworks starts a new managed ACP process, asks the provider to resume the recorded provider session id, and proves requested session id equals actual provider session id before prompting.
4. The worker persists an attach receipt before prompt, persists a prompt-turn marker, sends the P086 mode-reset prompt through the resumed session, correlates completion by prompt-turn marker, and settles via continuation artifacts/readback rather than normal retry.
5. Unsupported, unsafe, mismatched, expired, side-effectful, approval-blocked, release/publish/security, or unverifiable cases fail closed with no prompt and no fallback to fresh retry.
6. Durable `resurrection_phase` state, receipt v2/readback, metrics, output-only recovery, session-store recovery, replay rules, and the proposal gate must prove the behavior.

## Platform And Scope

Primary implementation surface is the Rust control plane:

- ACP adapter capability and Claude adapter behavior in `control-plane/crates/acp`
- MCP `agents.continue_work` admission and readback in `control-plane/crates/mcp-server`
- engine continuation worker, replay, and settlement in `control-plane/crates/engine`
- continuation domain types in `control-plane/crates/domain`
- migrations and repositories in `control-plane/crates/db`
- GraphQL readback in `control-plane/crates/graphql-server`
- YAML frozen catalog examples in `examples/agents`
- proposal gate and evidence docs under `scripts` and `docs/reference`

Swift UI command work is not selected as active implementation scope because P086 describes Swift read-only surfaces as shipped baseline and does not require new Swift mutation commands.

## Primary Flows Audited

1. Operator invokes `agents.continue_work` using `provider_session_resurrection` for `code_writer` after the original live ACP handle is gone.
2. Frozen catalog and adapter capability gates decide whether the request is admitted before queueing work.
3. Claude provider resurrection starts a new ACP subprocess, attaches to the requested provider session id, proves identity, persists receipt/marker, sends prompt, and settles output.
4. Output-only recovery and Claude session-store recovery avoid source edits unless explicitly allowed and reject ambiguous resurrection identity.
5. Crash/replay/readback/metrics/security surfaces expose the right state without duplicating prompts or leaking raw provider receipt data outside run scope.

## Specialist Coverage

| Reviewer lens | Status | Coverage |
| --- | --- | --- |
| Rust architecture reviewer | Covered | ACP adapter trait, capability shape, engine worker boundaries, DB/domain model |
| Rust reliability reviewer | Covered | continuation state, replay/idempotency, live-handle dependency, prompt correlation, orphan/attach behavior |
| API contract reviewer | Covered | MCP `agents.continue_work`, raw attach receipt readback, GraphQL receipt readback, schema/readback fields |
| Observability and rollout reviewer | Covered | migrations, metrics, test gate, docs/reference evidence |
| Rust security reviewer | Covered | raw receipt exposure, operator/run scope, provider session ids, process/runtime receipt data, public MCP/GraphQL ingress |

Rejected alternatives:

- Apple UI/UX reviewers: not selected because active P086 scope excludes new Swift command UX. Surface-fingerprint hits were caused by docs and test-gate references rather than material Swift implementation.
- Performance reviewer: not selected because P086 has no throughput or p95 performance acceptance claim. Resource and process concerns were covered under reliability/security.
- Product reviewer: not selected because the active proposal is a technical control-plane completion proposal, not a product outcome proposal.
- Go reviewers: not applicable.

Specialist coverage hard gate: satisfied for audit coverage, but the covered reviews found readiness blockers.

## Fidelity And Divergence

### What Matches The Proposal

- `agents.continue_work` has typed `provider_session_resurrection` admission plumbing and rejects unsupported adapters before enqueueing.
- Frozen catalog parsing checks for missing, disabled, malformed, and trigger-ineligible continuation capability.
- The example agent catalog contains a `provider_session_resurrection` capability block for `code_writer`, currently disabled.
- DB migration scaffolding exists for `resurrection_phase`, terminal idempotency, and raw attach receipt persistence.
- A provider-session attach receipt v2 schema exists in reference docs.
- Some metrics counters and readback fields exist for unsupported and attach receipt paths.
- Existing live-handle continuation remains present.

### Material Divergence

- No adapter currently supports provider-session resurrection. Claude does not declare or implement the required successful path.
- The adapter contract is still a boolean-style support check, not the proposal's versioned `ProviderSessionResurrectionCapability` contract with attach request/result, proof source, safe-write flag, and typed failure classes.
- The current example catalog leaves `provider_session_resurrection.enabled: false`, so new runs using this catalog do not opt in to the active P086 behavior.
- The engine continuation worker is still live-handle oriented. It requires an existing `session_generation_id`, checks `has_live_session`, writes a `live_handle_continuation` receipt, and calls ACP with `reuse_existing_session: true`.
- No implementation starts a new managed ACP process for resurrection, attaches/resumes a provider session id, verifies actual id before prompt, writes a prompt-turn marker, or correlates by that marker.
- No output-only recovery path with no-source-edit evidence was found.
- No Claude session-store recovery path with transcript digest, turn/tool evidence, and ownership proof was found.
- `resurrection_phase` is scaffolded in SQL but not represented as a typed domain enum, not populated at admission, and not generally exposed through MCP/GraphQL continuation readback.
- The canonical `proposal-086` gate is still described as a historical live-handle continuation gate and currently fails before the deeper slices run.
- Raw provider-session attach receipt readback has a security access-control gap.

## Residual Scope

P086 still has implementation scope left inside the active proposal. This cannot be deferred to P093 because P093 is only the soak/validation owner. Future support for additional providers may be deferred, but P086 requires the generic contract plus at least one production-relevant adapter, and Claude is explicitly required.

The minimum residual implementation needed before readiness:

1. Replace the boolean adapter support shape with the versioned capability contract required by P086.
2. Implement Claude resurrection through a new managed ACP process, with requested-vs-actual identity proof before prompt.
3. Enable frozen catalog opt-in for supported new runs while preserving fail-closed behavior for old or unsupported snapshots.
4. Add resurrection-specific worker phases, prompt-turn marker persistence, attach receipt v2 writing, no-fresh-retry settlement, and replay/idempotency rules.
5. Add output-only recovery and Claude session-store recovery with the proposal's source-edit and ambiguity protections.
6. Fix raw receipt authorization and rerun an updated P086 gate that proves the active resurrection success path.

## Requirement Summary

| Requirement | Status | Notes |
| --- | --- | --- |
| REQ-001 explicit continuation modes and surfaces | Partial | `provider_session_resurrection` exists, but required modes such as `normal_fresh_execution`, `normal_live_reuse`, and `output_only_recovery` are not modeled as proposed. |
| REQ-002 versioned adapter capability contract | Partial | Only a support boolean/default was found; no versioned capability struct with typed attach request/result/proof contract. |
| REQ-003 frozen catalog gate | Partial | Gate exists and rejects unsupported/disabled/missing fields, but current catalog disables resurrection and no supported adapter can pass in production. |
| REQ-004 Claude supported resurrection adapter | Missing | Claude has no provider-session resurrection implementation. |
| REQ-005 new managed ACP attach/resume flow | Missing | Worker still uses live-handle reuse and rejects missing live handle. |
| REQ-006 output-only recovery | Missing | No no-source-edit output-only recovery flow was found. |
| REQ-007 durable resurrection phase and replay | Partial | SQL scaffolding exists, but domain/readback/admission/worker replay behavior is incomplete. |
| REQ-008 receipt v2 and evidence | Partial | Schema/storage/readback pieces exist, but no successful path writes the required receipt before prompt. |
| REQ-009 metrics and reports | Partial | Unsupported counters exist; successful attach, prompt, useful/no-progress, and fresh-retry-avoided metrics are incomplete. |
| REQ-010 fail-closed safety rules | Partial | Some admission checks exist; identity, orphan reap, quota/runtime health, and successful-path mismatch rules are not implemented end to end. |
| REQ-011 required tests and canonical gate | Missing | `proposal-086` fails due duplicate migration version and does not yet prove active resurrection completion. |
| REQ-012 P095 prompt minimalism relationship | Partial | Existing live prompt avoids output paths, but the required resurrection/output-only prompts are missing. |
| REQ-013 Swift read-only baseline | Implemented | No new Swift mutation command was required or found as active P086 scope. |

## Detailed Requirement Audit

### REQ-001: Continuation Modes

Status: Partial.

Evidence:

- `control-plane/crates/domain/src/continuation.rs` defines only `LiveHandleContinuation` and `ProviderSessionResurrection`.
- The proposal requires explicit separation of `normal_fresh_execution`, `normal_live_reuse`, `provider_session_resurrection`, and `output_only_recovery`.
- Mode surfaces partially exist through continuation records and MCP admission, but they do not implement the full classification contract.

Impact:

The implementation can distinguish live-handle continuation from provider-session resurrection at admission, but it cannot express the full proposal state model or prevent all ambiguous retry/reuse classification problems the proposal was written to eliminate.

### REQ-002: Versioned Adapter Capability Contract

Status: Partial.

Evidence:

- `control-plane/crates/acp/src/adapters/mod.rs` exposes a default `supports_provider_session_resurrection() -> bool { false }`.
- No versioned `ProviderSessionResurrectionCapability` object was found with provider/adapter id, schema version, launch args/session fields/env values, typed attach request/result, proof source, safe-write flag, or typed failure classes.
- `control-plane/crates/acp/src/adapters/claude.rs` does not override support or implement attach/resume proof behavior.

Impact:

The implementation preserves fail-closed unsupported behavior, but it does not provide the adapter contract P086 requires for any supported adapter.

### REQ-003: Frozen Catalog Gate

Status: Partial.

Evidence:

- `control-plane/crates/mcp-server/src/tools/agents.rs` implements frozen catalog rejection for missing snapshot, malformed JSON, missing `code_writer`, disabled capability, disallowed triggers, missing provider session id, and unsupported adapter.
- `examples/agents/agents.yaml` includes `code_writer.continuation_capability.provider_session_resurrection.enabled: false`.
- `provider_session_resurrection_adapter_supported` returns false for Claude, Codex, Gemini, Auggie, and Junie.

Impact:

The fail-closed gate is useful, but it only proves the unsupported path. It does not admit a real supported P086 run.

### REQ-004: Claude Provider Session Resurrection

Status: Missing.

Evidence:

- Claude adapter code declares normal launch/session specs only.
- MCP adapter support logic explicitly returns false for Claude aliases.
- No code path asks Claude to resume the recorded provider session id, observes the actual provider session id, compares requested versus actual, or fails with `identity_unverifiable` before prompt.
- No Claude session-store recovery evidence path was found.

Impact:

This is the core active P086 deliverable. Without it, P086 cannot be considered implemented even if the unsupported path is correct.

### REQ-005: New Managed ACP Process, Prompt Marker, And Settlement

Status: Missing.

Evidence:

- `control-plane/crates/engine/src/executor.rs` continuation worker requires `session_generation_id`.
- It checks for an existing live ACP handle and settles `no_progress` when the live handle is missing.
- It writes a receipt with `attach_kind: live_handle_continuation` and `managed_process_reused: true`.
- It invokes ACP with `reuse_existing_session: true`.
- No resurrection-specific attach-before-prompt, prompt-turn marker, or marker-based terminal correlation was found.

Impact:

The implementation cannot continue useful code-writer work from a provider session id after Chainworks no longer owns the live ACP handle, which is the central user outcome of P086.

### REQ-006: Output-Only Recovery

Status: Missing.

Evidence:

- No separate `output_only_recovery` mode was found in the domain enum.
- No prompt path was found that forbids code edits unless explicitly allowed and asks only for missing or invalid outputs.
- No pre/post source snapshot or `changed_source_files == 0` evidence was found for this mode.

Impact:

The P079/P088 malformed-output recovery contract remains unimplemented for P086.

### REQ-007: Durable Resurrection Phase And Replay

Status: Partial.

Evidence:

- `control-plane/crates/db/migrations/079_p086_resurrection_state_and_idempotency.sql` adds a `resurrection_phase` CHECK for the proposal's phases.
- Domain continuation records do not expose a typed `resurrection_phase` enum.
- The continuation repository select/row mapping omits `resurrection_phase`.
- Admission inserts do not populate `resurrection_phase = 'admitted'`.
- The engine worker does not transition through resurrection phases.

Impact:

The schema has a good start, but crash/replay behavior cannot be considered implemented without domain representation, writes, readback, and replay rules.

### REQ-008: Attach Receipt V2 And Evidence

Status: Partial.

Evidence:

- `docs/reference/p086/schemas/artifacts/provider_session_attach_receipt_v2.schema.json` requires many of the proposal fields.
- DB/repository support for raw P086 attach receipts exists.
- The active worker writes a v1-style live-handle receipt, not a successful provider-session resurrection receipt before prompt.
- The v2 schema description says raw fields are stored in a private filesystem sidecar, while implementation stores raw receipt JSON in DB-backed readback.

Impact:

The contract is partially documented, but there is no production success path that can satisfy it.

### REQ-009: Metrics And Reports

Status: Partial.

Evidence:

- Metrics include unsupported/attach success/attach failure summary fields.
- MCP admission emits an unsupported resurrection metric when the adapter is unsupported.
- No successful resurrection path emits requested, attach success, prompt sent, no-progress/useful-progress, or fresh-retry-avoided metrics for the completed behavior.
- The failing gate prevented downstream report/readback validation.

Impact:

Observability proves the unsupported path, not the active successful path.

### REQ-010: Fail-Closed Safety Rules

Status: Partial.

Evidence:

- Admission rejects disabled catalog capability, disallowed trigger, missing provider session id, unsupported adapter, side-effect/approval issues, and forbidden release/security-like lanes in existing continuation logic.
- No successful path exists to enforce requested-vs-actual mismatch, expired/missing provider session, orphan reap failure, quota/auth/runtime health, or identity-unverifiable failures before prompt.
- Because successful resurrection never starts, the safety contract is not fully exercised.

Impact:

The safe unsupported behavior is present, but P086 also requires safe success-path failure classes.

### REQ-011: Tests And Canonical Gate

Status: Missing.

Evidence:

- `./scripts/test-gate.sh proposal-086` was run.
- Phase 0 preflight passed.
- `cargo test -p domain "continuation"` passed.
- `cargo test -p db --test proposal_086_continuation_lifecycle` failed all 11 tests.
- The shared failure is SQLx migration application failing with `UNIQUE constraint failed: _sqlx_migrations.version`.
- Duplicate migration version `079` exists for `079_p079_output_contract_repair.sql` and `079_p086_resurrection_state_and_idempotency.sql`.
- `docs/reference/test-gates.md` describes `proposal-086|p086` as retained historical coverage for live-handle continuation rather than the active resurrection completion contract.

Impact:

The required same-tree proposal gate does not pass, and the gate content is not yet a sufficient active P086 proof.

### REQ-012: P095 Prompt Minimalism

Status: Partial.

Evidence:

- The existing live-handle continuation prompt does not include output artifact paths or `CHAINWORKS_OUTPUT`.
- Required resurrection and output-only prompts were not found, so their compliance cannot be proven.

Impact:

Prompt-minimalism conformance remains incomplete because the active P086 prompts do not exist.

### REQ-013: Swift Read-Only Baseline

Status: Implemented for active scope.

Evidence:

- P086 does not require new Swift command UI.
- No material Swift mutation path was selected as active P086 implementation scope.

Impact:

No readiness blocker.

## Security Scan

The security-sensitive diff hard gate triggered and required manual security review. Trigger categories included auth, public ingress, filesystem/subprocess boundary, secrets/redaction/privacy, parser boundary, and resource limits.

Security review result: Not Ready due SEC-001.

Dependency scan note:

- `cargo-audit` was not installed.
- `cargo-deny` was not installed.
- Dependency vulnerability policy therefore was not fully verified in this audit.

### SEC-001: Raw attach receipt readback is not run-scoped when `run_id` is omitted

Severity: Major.

Evidence:

- MCP `agents.attach_receipt.get` parses `continuation_id` and optional `run_id`.
- The MCP path only compares the actual run id to caller-supplied `run_id` when `run_id` is provided. If omitted, an operator-class caller can retrieve the raw receipt by continuation id.
- GraphQL `providerSessionAttachReceipt(continuationId, runId: Option<ID>)` has the same optional-run shape and authorizes operator callers when `actual_run.is_some()` if no run id is provided.
- Raw attach receipt data includes provider session ids, process/runtime fields, and attach evidence that the docs describe as run-scoped/private.

Impact:

Any operator principal with a known or guessable continuation id can retrieve raw provider-session attach receipt data without proving authorization for that run through the readback API. That is a privacy and boundary-control regression on a security-sensitive readback surface.

Required fix:

Require run-scoped authorization for raw receipt readback. The API should either require `run_id` and verify it against the continuation record and principal scope, or derive the run id from the continuation and enforce principal authorization for that derived run before returning the raw receipt. GraphQL and MCP must match.

## Findings

### READY-001: No implemented provider-session resurrection success path

Severity: Critical.

Affected areas:

- `control-plane/crates/acp/src/adapters`
- `control-plane/crates/mcp-server/src/tools/agents.rs`
- `control-plane/crates/engine/src/executor.rs`
- `examples/agents/agents.yaml`

The current implementation proves unsupported fail-closed behavior but not the active P086 outcome. Claude does not support resurrection, all adapter aliases return unsupported, the catalog disables the mode, and the worker still depends on live-handle reuse.

This blocks conformance to the proposal goal: continuing useful code-writer work from a provider session id after the live handle is gone.

### READY-002: Canonical proposal gate fails on duplicate migration version

Severity: Critical.

Affected areas:

- `control-plane/crates/db/migrations/079_p079_output_contract_repair.sql`
- `control-plane/crates/db/migrations/079_p086_resurrection_state_and_idempotency.sql`
- `./scripts/test-gate.sh proposal-086`

`proposal-086` fails during DB tests because SQLx migration application hits `UNIQUE constraint failed: _sqlx_migrations.version`. The duplicate migration version `079` prevents the same-tree gate from reaching deeper engine/MCP/GraphQL coverage.

This alone makes implementation readiness Not Ready.

### ARCH-001: Adapter capability contract does not match P086

Severity: Major.

Affected areas:

- `control-plane/crates/acp/src/adapters/mod.rs`
- provider adapter implementations

P086 requires a versioned `ProviderSessionResurrectionCapability` declaration through `AcpAdapter`. The implementation only exposes a support boolean defaulting false and does not model the required attach request/result/proof contract.

This blocks both generic correctness and provider-specific reviewability.

### REL-001: Resurrection phase/replay model is schema-only scaffolding

Severity: Major.

Affected areas:

- `control-plane/crates/db/migrations/079_p086_resurrection_state_and_idempotency.sql`
- `control-plane/crates/domain/src/continuation.rs`
- `control-plane/crates/db/src/repos/agent_work_continuations.rs`
- `control-plane/crates/engine/src/executor.rs`

The DB migration introduces phases, but the domain model, repository mapping, admission insert, worker transitions, and replay behavior do not implement the phase state machine. Crash/replay guarantees such as no duplicate prompt and no fresh retry fallback are therefore not proven.

### API-001: Active P086 readback contract is incomplete

Severity: Major.

Affected areas:

- MCP continuation/readback APIs
- GraphQL continuation/readback APIs
- report/readback docs

Raw receipt and summary pieces exist, but general continuation readback does not expose the full `resurrection_phase` and active success-path evidence. The schema also describes private filesystem sidecar storage while the implementation exposes DB-backed raw JSON readback.

### OPS-001: P086 gate appears historical and insufficient for active completion

Severity: Major.

Affected areas:

- `docs/reference/test-gates.md`
- `scripts/test-gate.sh`

The reference docs describe `proposal-086|p086` as retained historical coverage for live-handle continuation. Active P086 requires provider-session resurrection completion. Even after the duplicate migration is fixed, the gate must be updated to prove a supported adapter success path, identity mismatch rejection before prompt, prompt-turn correlation, phase replay, output-only recovery, and receipt/readback.

## Scorecard

| Dimension | Score | Rationale |
| --- | --- | --- |
| Proposal conformance | Not Implemented | The core Claude resurrection success path is absent. |
| Implementation readiness | Not Ready | Canonical gate fails; security issue present. |
| Architecture | Not Ready | Adapter capability contract is not the proposal contract. |
| Reliability | Not Ready | Worker still depends on live handles; replay model incomplete. |
| API contract | Not Ready | Readback and receipt semantics are incomplete and have a security gap. |
| Observability/rollout | Not Ready | Metrics/gate prove unsupported path only; gate fails early. |
| Security | Not Ready | Raw receipt readback is not run-scoped when `run_id` is omitted. |

## Verification Log

| Command | Result |
| --- | --- |
| `git rev-parse HEAD` | `4c3dce2c5f70cd7dc540887979d399f7d354c59d` |
| `git status --short` | Dirty worktree with staged and unstaged implementation changes; no unmerged files observed. |
| `git diff --check` | Passed. |
| `./scripts/test-gate.sh proposal-086` | Failed. Phase 0 preflight passed; domain continuation tests passed; DB lifecycle test failed all 11 tests due duplicate SQLx migration version `079`. |
| `cargo-audit` availability check | Not installed. |
| `cargo-deny` availability check | Not installed. |

Failure detail for `proposal-086`:

- Passing slice: `cargo test -p domain "continuation"` -> 4 passed.
- Failing slice: `cargo test -p db --test proposal_086_continuation_lifecycle` -> 11 failed.
- Shared cause: `UNIQUE constraint failed: _sqlx_migrations.version`.
- Duplicate versions found: `079_p079_output_contract_repair.sql` and `079_p086_resurrection_state_and_idempotency.sql`.

## Readiness Checklist

| Gate | Status | Notes |
| --- | --- | --- |
| Proposal contract extracted | Passed | Active scope is provider-session resurrection completion, not baseline live-handle continuation. |
| Prior proposal-review reuse checked | Passed | No proposal-review artifacts found; implementation audits ignored for reviewer selection. |
| Specialist coverage hard gate | Passed for audit coverage | Five material reviewer lenses covered. |
| Security-sensitive diff hard gate | Failed readiness | Manual security review found SEC-001. |
| Same-tree canonical proposal gate | Failed | `./scripts/test-gate.sh proposal-086` fails on duplicate migration version. |
| Acceptance criteria | Failed | Core Claude resurrection, identity proof, prompt marker, output-only recovery, replay, and readback are missing or partial. |
| Documentation/reference alignment | Failed | Gate docs still describe historical live-handle coverage; schema storage description conflicts with implementation. |
| Residual scope owner | Failed | P093 cannot own the missing implementation because it is soak-only. |

## Final Verdict

Conformance: Not Implemented.

Readiness: Not Ready.

The implementation contains useful fail-closed scaffolding for `provider_session_resurrection`, but it does not implement the active P086 success path. No adapter, including Claude, supports resurrection; the worker remains live-handle based; the catalog disables the mode; phase/replay/readback are incomplete; output-only recovery is missing; and the proposal gate currently fails due duplicate migration version. A security issue in raw attach receipt readback also blocks readiness.

Required actions before a Ready verdict:

1. Implement the versioned adapter capability contract and the Claude resurrection adapter with requested-vs-actual provider session proof before prompt.
2. Enable frozen catalog opt-in for supported new runs while preserving fail-closed rejection for old, disabled, malformed, or unsupported snapshots.
3. Add the resurrection worker path: new managed ACP process, attach receipt before prompt, prompt-turn marker, marker-based settlement, no fresh retry fallback, and phase/replay idempotency.
4. Implement output-only recovery and Claude session-store recovery with source-edit evidence and ambiguity rejection.
5. Expose durable `resurrection_phase`, attach receipt v2, metrics, and report/MCP/GraphQL readback consistently.
6. Fix raw attach receipt authorization so raw data is always scoped to the authorized run.
7. Resolve duplicate migration version `079`, update `proposal-086` gate to active P086 coverage, and rerun the full canonical gate successfully.
