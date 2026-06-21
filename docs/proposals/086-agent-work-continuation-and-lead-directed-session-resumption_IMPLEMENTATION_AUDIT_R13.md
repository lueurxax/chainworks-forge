# Proposal 086 Implementation Audit R13

Date: 2026-06-20

Proposal: `docs/proposals/086-agent-work-continuation-and-lead-directed-session-resumption.md`

Audited revision: `0e6482c82b588b74a76294a225e68286bfe37fa4` with the current dirty worktree.

Report path: `docs/proposals/086-agent-work-continuation-and-lead-directed-session-resumption_IMPLEMENTATION_AUDIT_R13.md`

## Verdict

Not ready for closeout.

The core resurrection path is substantially implemented: the Claude adapter declares a provider-session resurrection capability, admission is frozen-catalog gated, target identity and safety checks run before enqueue, the worker starts a fresh ACP child, verifies the provider-returned session id before prompt, persists attach evidence before provider I/O, and the canonical `proposal-086` gate exits successfully.

However, the implementation still fails closeout on externally visible contract evidence:

1. `agents.attach_receipt.get` does not return the response shape declared by the P086 MCP response schema or the reference doc.
2. The proposal gate's Swift readback slice is wired to a selector that currently executes zero tests.
3. The v2 attach receipt readback remains a pre-prompt snapshot after successful execution, so `prompt_sent_at` and final `resurrection_phase` are not reflected in the raw receipt/projection returned to operators and reviewers.

## Prior Reviewer Reuse

Reviewer-selection reuse: not reused.

`discover_prior_review.py` returned no prior review artifacts for this proposal. Existing `IMPLEMENTATION_AUDIT_R*` files were not used for reviewer selection.

## Reviewer Selection

Triggered lenses from the implementation fingerprint and manual pass:

- `api-contract`
- `architecture`
- `observability-rollout`
- `reliability`
- `security`
- `performance`
- `apple-ui-ux`

Selected reviewers, capped at five:

- `rust_arch_reviewer` - validates control-plane ownership, mode boundaries, state transitions, and daemon/app parity.
- `rust_reliability_reviewer` - validates crash/replay, idempotency, orphan reap, prompt duplication prevention, and fail-closed behavior.
- `rust_security_reviewer` - required by auth, public ingress, subprocess/filesystem boundary, parser/resource, redaction/privacy, and receipt-access surfaces.
- `api_contract_reviewer` - required by MCP/GraphQL request/response schemas, readback fields, and JSON artifact contracts.
- `observability_rollout_reviewer` - required by metrics, operator readback, evidence fixtures, gate coverage, and release readiness.

Displaced reviewers:

- `rust_performance_reviewer` - displaced by the hard cap; no readiness claim depends on throughput/latency beyond existing admission backpressure and timeout checks.
- `macos_ui_reviewer` / `apple_ux_reviewer` - displaced by the hard cap; P086 UI scope is passive readback and not the main residual blocker.
- `product_reviewer` - displaced because product semantics are already represented through operator readback and rollout evidence.

## Implementation Evidence

Strongly implemented areas:

- Adapter capability model exists with explicit `ProviderSessionResurrectionCapability` fields and closed failure-class vocabulary in `control-plane/crates/acp/src/adapters/mod.rs:30-79`.
- Claude declares `provider_session_resurrection_v1`, injects `resumeSessionId` into `session/new`, and marks `session_new_result.sessionId` as the identity proof source in `control-plane/crates/acp/src/adapters/claude.rs:57-70` and `control-plane/crates/acp/src/adapters/claude.rs:103-132`.
- The ACP manager starts a fresh ordered session, reads the returned provider session id, rejects identity mismatch before prompt, and releases the new live handle on mismatch in `control-plane/crates/acp/src/manager.rs:495-574`.
- Admission rejects missing/malformed frozen catalog opt-in, missing provider session id, unsupported adapters, forbidden lanes, unresolved side effects, pending approvals, and active continuations before enqueue in `control-plane/crates/mcp-server/src/tools/agents.rs:1146-1400` and `control-plane/crates/mcp-server/src/tools/agents.rs:2232-2496`.
- Admission is atomic for idempotency, active-continuation exclusion, saturation checks, command journaling, and continuation insertion in `control-plane/crates/db/src/repos/agent_work_continuations.rs:819-1150`.
- The worker claims before provider I/O, creates a resurrection session generation, attaches, persists the raw receipt and redacted artifact before prompt, inserts a `provider_send` side-effect ledger row before the prompt, then sends the canonical continuation prompt through the attached handle in `control-plane/crates/engine/src/executor.rs:7431-8111`.
- Claude session-store recovery is target-correlated by prompt marker, request fingerprint, stage execution id, and agent execution id in `control-plane/crates/acp/src/session.rs:865-918`.
- Recovery contains stale supervised-worker detection and process-group reap with UID/PGID guards in `control-plane/crates/db/src/repos/agent_work_continuations.rs:1672-1692` and `control-plane/crates/engine/src/recovery.rs:125-240`.

## Findings

### P1: MCP attach receipt readback does not match the declared P086 response contract

The reference MCP schema declares the Operator response as requiring `outcome`, `continuation_id`, `run_id`, `attach_receipt_artifact_id`, and `receipt` with `outcome = "ok"` in `docs/reference/p086/schemas/mcp/agents.attach_receipt.get.response.schema.json:15-28`. Reviewer projection requires `outcome = "reviewer_projection"` plus stable projection fields in `docs/reference/p086/schemas/mcp/agents.attach_receipt.get.response.schema.json:30-53`. Guest projection requires `outcome = "redacted"`, `attach_receipt_artifact_present`, and `resurrection_phase` in `docs/reference/p086/schemas/mcp/agents.attach_receipt.get.response.schema.json:55-63`.

The reference doc says the same thing: Operator returns the raw JSON receipt with `outcome=ok`, Observer returns `outcome=reviewer_projection`, and Agent/Guest returns `outcome=redacted` in `docs/reference/agent-work-continuation.md:48`.

The actual handler returns different shapes:

- Operator: `principal_class`, `access_level`, `continuation_id`, and `receipt`, without `outcome`, `run_id`, or `attach_receipt_artifact_id` in `control-plane/crates/mcp-server/src/tools/agents.rs:530-535`.
- Observer: `principal_class`, `access_level`, `continuation_id`, and `receipt`, without the declared `outcome` or stable projection top-level fields in `control-plane/crates/mcp-server/src/tools/agents.rs:583-592`.
- Agent: `principal_class`, `continuation_id`, `resurrection_phase`, and `access_level`, without the declared `outcome` or `attach_receipt_artifact_present` in `control-plane/crates/mcp-server/src/tools/agents.rs:459-464`.

Impact: clients generated from the shipped MCP response schema cannot consume the server response. This also means the claimed "readback without raw artifact inspection" contract is not stable for MCP clients.

Required fix: align `agents.attach_receipt.get` runtime responses with the reference schema, or update the schema and reference doc together if the implementation shape is intentionally different. Add response-schema validation tests for Operator, Observer, Agent, not-found, and auth-failure cases.

### P1: The P086 Swift readback gate passes while executing zero Swift tests

`PROPOSAL_086_SWIFT_TESTS` selects only `Chainworks ForgeTests/Proposal031ThinGraphQLReadBoundaryTests` in `scripts/test-gate.sh:249-251`, and `proposal-086` invokes that list in `scripts/test-gate.sh:10154-10168`.

The audited gate run completed with `** TEST SUCCEEDED **`, but the Swift section reported:

```text
Test Suite 'Chainworks ForgeTests.xctest' passed
Executed 0 tests, with 0 failures
```

A workspace search found no `Proposal031ThinGraphQLReadBoundaryTests` test class under `Chainworks ForgeTests`; the selector is only referenced by gate configuration and production support text.

Impact: the acceptance proof does not currently validate Swift/passive readback behavior for P086. The gate can pass after the selected Swift tests disappear or are renamed.

Required fix: add a real P086 Swift readback test target or point the selector at existing tests, and make the gate fail when a selected test set executes zero tests.

### P2: The v2 attach receipt remains a pre-prompt snapshot after successful resurrection

The worker builds the raw `provider_session_attach_receipt_v2` before prompt with `prompt_sent_at = null` and `resurrection_phase = "attached_unprompted"` in `control-plane/crates/engine/src/executor.rs:7793-7797`. It then stores the raw DB receipt in `control-plane/crates/engine/src/executor.rs:7867-7873` and persists the public redacted artifact from that same pre-prompt JSON in `control-plane/crates/engine/src/executor.rs:7900-7912`.

Later, the worker advances the DB phase to `prompting` and `completed` in `control-plane/crates/engine/src/executor.rs:8051-8052` and `control-plane/crates/engine/src/executor.rs:8224-8225`, but the receipt body is not updated with `prompt_sent_at`, the later phase, or final settlement fields. The only post-prompt raw receipt update path I found is the Claude session-store recovery branch, and it updates only `session_store_*` fields in `control-plane/crates/engine/src/executor.rs:8115-8155`.

Impact: Operator/Reviewer attach-receipt readback can report an already-completed resurrection as `attached_unprompted` with no `prompt_sent_at`. That undercuts P086's requirement that successful resurrection evidence and readback expose prompt correlation and phase progress without requiring artifact spelunking.

Required fix: either update the raw DB receipt and redacted artifact/projection as the lifecycle advances, or split the contract into an immutable pre-prompt attach receipt plus a separate final resurrection lifecycle receipt. In either case, MCP/GraphQL/report readback should expose current phase, prompt timestamp, failure class, session-store recovery result, and output-only source-change evidence consistently.

## Verification

Command run:

```bash
CHAINWORKS_ALLOW_LOCAL_CARGO_TARGET_DIR=1 CHAINWORKS_CARGO_WRAPPER=0 ./scripts/test-gate.sh proposal-086
```

Result: passed.

Covered by the gate:

- P086 static preflight for migration/schema/source needles.
- Domain continuation tests: 4 passed.
- ACP Claude resurrection capability tests: 2 passed.
- ACP Claude session-store recovery tests: 5 passed.
- DB continuation lifecycle tests: 11 passed.
- Engine P086 tests: 7 passed.
- MCP `tools::agents` tests: 40 passed.
- GraphQL continuation readback tests: 2 passed.
- Daemon live/resurrection integration tests: 6 passed.
- Swift readback xcodebuild step: exited successfully but executed 0 tests.

Not covered enough by the gate:

- Runtime response validation against `agents.attach_receipt.get.response.schema.json`.
- Swift P086 readback behavior, because the selected test class executed zero tests.
- Post-completion attach receipt freshness (`prompt_sent_at`, final `resurrection_phase`, and final readback projection).

## Closeout Readiness

Closeout readiness: blocked.

The implementation is much closer than previous partial states and the central daemon/ACP path is credible, but P086 cannot be retired while its public MCP schema disagrees with the server and the Swift readback proof is a zero-test pass. Fix those contracts and add tests that prove the response schemas and post-completion receipt readback, then rerun `./scripts/test-gate.sh proposal-086`.
