# Proposal 086 Implementation Audit R9

## Metadata

- Proposal: `docs/proposals/086-agent-work-continuation-and-lead-directed-session-resumption.md`
- Proposal title: `Proposal 086: Provider Session Resurrection Completion`
- Proposal state at audit time: Draft
- Audit report: `docs/proposals/086-agent-work-continuation-and-lead-directed-session-resumption_IMPLEMENTATION_AUDIT_R9.md`
- Audit timestamp: 2026-06-20 14:28:52 EEST
- Audited HEAD: `0e6482c82b588b74a76294a225e68286bfe37fa4`
- Working tree: dirty before this report was created; unrelated/concurrent edits were audited as found and not reverted.
- Skill: `proposal-implementation-audit`
- Overall conformance: Partial
- Implementation readiness: Not Ready

## Target And Audit Boundary

P086 now scopes the remaining implementation to `provider_session_resurrection`: start a new Chainworks-managed ACP process, attach/resume a recorded provider session id for supported adapters, prove that identity before prompt, preserve fail-closed behavior for unsupported adapters, and support output-only and session-store recovery cases. The proposal explicitly says P093 does not own this implementation tail, so missing provider-session resurrection behavior cannot be retired as a later soak/scale follow-up.

I audited the current repository state against the proposal text, not against earlier P086 review variants. I did not edit code, tests, configs, or the proposal.

## Prior Review Reuse

- Prior review discovery: no reusable prior proposal-review artifact was discovered for this run.
- Reuse decision: not reused.
- Consequence: this audit uses current proposal text plus current repo evidence.

## Proposal Contract Summary

The proposal requires all of the following before it can be called implemented:

1. Explicit continuation modes, including `provider_session_resurrection` and `output_only_recovery`, with no silent fresh retry fallback.
2. A versioned ACP adapter capability contract and at least one production-relevant adapter, Claude, enabled.
3. Frozen-catalog admission gating before enqueue, with old/malformed snapshots failing closed.
4. Claude resurrection through a new ACP child, identity proof before prompt, and mismatch/unsupported cases failing before prompt.
5. Claude provider session-store recovery when ACP loses a terminal response, with machine-checkable ownership evidence.
6. Durable attach/prompt/settlement receipt fields, including prompt-turn correlation, request fingerprint, target ids, provider request ids when exposed, and session-store recovery fields.
7. Output-only repair mode that can recover malformed/incomplete outputs without source edits unless explicitly allowed.
8. Crash/replay rules around `resurrection_phase`, including no duplicate prompt after prompt-received ambiguity.
9. MCP/GraphQL/report readback and metrics that expose the above without raw JSON inspection.
10. The same-tree P086 gate proving the behavior.

## Platform And Product Scope

- Rust control plane: ACP manager, daemon worker, SQLite repos/migrations, MCP tools, GraphQL readback.
- ACP adapter boundary: Claude capability and attach/resume identity proof.
- Operator surface: MCP commands and passive SwiftUI/GraphQL readback. The proposal does not require a Swift mutation UI.
- Security-sensitive boundary: provider session ids, ACP subprocess lifecycle, DB-backed raw receipts, operator-scoped raw readback.

## Selected And Rejected Reviewers

Selected:

- `chainworks_execution_truth_reviewer`: ownership, mode truth, run/stage/agent boundary, no silent retry.
- `rust_reliability_reviewer`: crash/replay, phase transitions, idempotency, duplicate prompt prevention.
- `api_contract_reviewer`: ACP capability, MCP/GraphQL schemas, attach receipt shape.
- `observability_rollout_reviewer`: metrics, report/readback fields, rollout fixtures and gates.
- `rust_security_reviewer`: auth, session-id secrecy, subprocess/file boundary, receipt exposure.

Rejected or scoped:

- `apple_ui_ux_reviewer`: scoped out for this conformance decision because P086 keeps SwiftUI read-only/passive. The helper did detect Swift files, so this lens should be re-run before any Ready claim if UI changes remain part of the P086 closeout.
- `performance_reviewer`: scoped into reliability/security for this pass because the proposal's concrete performance-like obligations are timeout/deadline/resource-bound behavior. The helper did detect performance surfaces, so a Ready claim should either add this reviewer or explicitly scope the lens with evidence.

## Specialist Coverage Matrix

| Lens | Helper required | Coverage decision | Result |
| --- | --- | --- | --- |
| API contract | Yes | Selected | Blocking receipt/schema and adapter-boundary gaps found. |
| Apple UI/UX | Yes | Scoped out for current Not Ready verdict | Passive readback only; re-run before Ready if Swift changes are retained. |
| Architecture | Yes | Selected through execution-truth review | Partial: MCP admission duplicates adapter support instead of querying adapter capability. |
| Observability/rollout | Yes | Selected | Partial: metrics/readback exist, but output-only/session-store fields and fixtures are incomplete. |
| Performance | Yes | Scoped into reliability/security for this pass | No Ready claim made; timeout/deadline behavior still needs complete replay proof. |
| Reliability | Yes | Selected | Blocking replay/session-store/output-only gaps found. |
| Security | Yes | Selected | Security helper triggered; independent pass found a session-id logging issue. |

## Audited Flows

### Flow 1: Claude Provider Session Resurrection Happy Path

Status: Partially Implemented.

Evidence:

- `control-plane/crates/acp/src/adapters/mod.rs:29-78` defines a versioned `ProviderSessionResurrectionCapability` and failure classes.
- `control-plane/crates/acp/src/adapters/claude.rs:123-139` declares Claude support and `resumeSessionId` identity proof shape.
- `control-plane/crates/acp/src/manager.rs:492-548` requires a requested provider session id, opens a new ordered session, compares the actual provider session id, and returns attach metadata.
- `control-plane/crates/daemon/tests/proposal_086_mcp_continuation_live_reuse.rs:724-842` proves a fixture happy path with one `session/new`, one prompt, v2 raw receipt storage, and final DB phase `completed`.

Divergence:

- The implemented path proves the basic attach-before-prompt shape, but the stored receipt omits several proposal-required correlation and session-store fields.

### Flow 2: Mismatch, Unsupported, And Frozen Catalog Admission

Status: Partially Implemented.

Evidence:

- `control-plane/crates/mcp-server/src/tools/agents.rs:1128-1312` checks the frozen catalog, trigger, mode, provider session id, and a provider support flag.
- `control-plane/crates/daemon/tests/proposal_086_mcp_continuation_live_reuse.rs:846-930` proves actual-session mismatch fails before prompt and leaves no raw receipt.

Divergence:

- `control-plane/crates/mcp-server/src/tools/agents.rs:1114-1126` hardcodes provider support by provider string. The proposal requires adapter-declared support to be the boundary truth before enqueue. The worker re-checks through ACP later, but admission is not solely driven by the adapter capability contract.

### Flow 3: Output-Only Recovery

Status: Missing.

Evidence:

- `control-plane/crates/domain/src/continuation.rs:6-9` models only `live_handle_continuation` and `provider_session_resurrection`; there is no `output_only_recovery` mode.
- `control-plane/crates/engine/src/executor.rs:7529-7561` hardcodes `"output_only": false`, `"source_edit_allowance": true`, and `"changed_source_files_count": 0` in the attach receipt.
- The only P086 output-only negative fixture, `docs/evidence/rollout-contract/p086/negative/output-only-repair-violation-changed-source-files.fixture.json:1-40`, is explicitly a placeholder.

Impact:

The P079/P088 output-repair use case promised by proposal section 3.5 is not implemented or proven.

### Flow 4: Claude Session-Store Recovery

Status: Partially Implemented infrastructure, Missing P086 contract integration.

Evidence:

- ACP has general Claude session-store recovery on prompt error: `control-plane/crates/acp/src/session.rs:425-443`.
- ACP tests prove finding the latest Claude transcript output for a provider session id: `control-plane/crates/acp/src/session.rs:1472-1527`.
- P086 settlement reads only `provider_result.transcript_text` for response/evidence artifacts: `control-plane/crates/engine/src/executor.rs:6117-6125`.
- P086 reconciliation reads the previously persisted response artifact or records explicit absence: `control-plane/crates/engine/src/executor.rs:6813-6871`.
- The v2 receipt schema required list does not include session-store transcript path, digest, recovery result, read timestamp, ownership source, or latest turn/tool activity: `docs/reference/p086/schemas/artifacts/provider_session_attach_receipt_v2.schema.json:8-40`.

Impact:

General ACP recovery exists, but P086 does not yet satisfy the proposal's first-class resurrection/session-store evidence contract.

### Flow 5: Crash/Replay And Phase Boundaries

Status: Partially Implemented.

Evidence:

- Migrations add `resurrection_phase` values and deadline/timeout fields: `control-plane/crates/db/migrations/079_p086_resurrection_state_and_idempotency.sql:1-25` and `control-plane/crates/db/migrations/081_p086_resurrection_phase_cancelling.sql:78-95`.
- The provider-session path sets `launching`, `attached_unprompted`, `prompting`, `settling`, `completed`, and `failed_closed`: `control-plane/crates/engine/src/executor.rs:7396-7824`.
- DB tests prove a post-`prompt_sent` claim does not rewind or resend before provider I/O: `control-plane/crates/db/tests/proposal_086_continuation_lifecycle.rs:235-274`.

Divergence:

- The domain layer has no typed `ResurrectionPhase` enum; `control-plane/crates/domain/src/continuation.rs:61-80` only models continuation status.
- I found no proposal-specific crash/replay tests for the proposal's `launching`, `launched`, `attaching`, `attached_unprompted`, and ambiguous `prompting` replay rules.
- The worker never appears to set `launched` or `attaching`, despite those being required phase boundaries in the proposal.

## Requirement Status

| ID | Requirement | Status | Evidence |
| --- | --- | --- | --- |
| REQ-001 | Four explicit continuation/recovery modes and no silent retry | Partial | Domain has only two continuation modes at `domain/src/continuation.rs:6-9`; no output-only mode. |
| REQ-002 | Versioned ACP adapter capability | Partial | Capability and Claude support exist, but MCP admission hardcodes provider support at `agents.rs:1114-1126`. |
| REQ-003 | Frozen catalog admission gate | Partial | Frozen catalog checks exist at `agents.rs:1128-1312`; adapter truth before enqueue is duplicated/hardcoded. |
| REQ-004 | Claude attach/resume and identity proof before prompt | Implemented for basic path | Manager compares actual/requested ids before prompt at `manager.rs:508-520`; daemon tests cover happy/mismatch. |
| REQ-005 | Session-store recovery as P086 evidence source | Missing | ACP has infrastructure, but P086 receipt/readback lacks required session-store fields and settlement binding. |
| REQ-006 | Attach receipt before prompt and prompt-turn correlation | Partial | Receipt is persisted before prompt, but schema/raw receipt omit prompt-turn marker and several target/correlation fields. |
| REQ-007 | Output-only recovery with no source edits unless allowed | Missing | Receipt hardcodes `output_only=false` and `source_edit_allowance=true`; fixture is placeholder. |
| REQ-008 | Durable `resurrection_phase` state and replay rules | Partial | DB phase exists; no typed enum and no crash matrix proof for required phase boundaries. |
| REQ-009 | MCP/GraphQL/report readback for receipt fields | Partial | Readback exists, but it can only expose the fields that are currently stored. |
| REQ-010 | Metrics for resurrection request/unsupported/attach/prompt/progress/fresh retry avoided | Partial | Metrics summary exists and tests cover some counters at `db/tests/proposal_086_continuation_lifecycle.rs:485-525`; not all promised output-only/session-store outcomes are represented. |
| REQ-011 | Safety fail-closed behavior | Partial | Unsupported/mismatch fail closed; output-only and replay ambiguity paths are incomplete. |
| REQ-012 | Same-tree P086 gate | Failed | `./scripts/test-gate.sh proposal-086` fails compiling `mcp-server` due missing `P083LifecycleDenialCode` variants. |
| REQ-013 | P095 prompt minimalism | Partial | Prompt contract exists, but P086 output-only recovery is absent and the prompt text is not proof for that mode. |

## Fidelity And Divergence Summary

High-fidelity portions:

- Claude adapter declares an explicit resurrection capability and passes `resumeSessionId`.
- ACP manager proves requested and actual provider session id equality before prompt.
- Unsupported providers are intended to fail closed.
- DB-backed raw receipts, receipt access audit, passive readback, and basic metrics exist.
- Duplicate prompt after `prompt_sent` has at least one DB-level guard test.

Material divergences:

- Output-only recovery is absent.
- P086-specific session-store recovery evidence is absent from receipt/readback and not proven in daemon tests.
- Attach receipt schema v2 is narrower than the proposal-required field set.
- Prompt-turn correlation is represented by a side-effect ledger row, not the proposal's named durable prompt-turn marker with provider request/turn id fields.
- Resurrection phase/replay behavior is not fully typed or tested across the required crash windows.
- The canonical gate fails before completion.

## Track 1: Proposal Conformance Findings

### P086-CONF-001 - Critical - Output-only recovery is not implemented

The proposal requires output-only recovery for malformed/incomplete output contracts, including a no-source-edit proof unless edits are explicitly allowed. Current code models only live-handle and provider-session modes (`domain/src/continuation.rs:6-9`), hardcodes the v2 receipt to `output_only=false` and `source_edit_allowance=true` (`executor.rs:7529-7561`), and has only a placeholder output-only negative fixture (`output-only-repair-violation-changed-source-files.fixture.json:1-40`).

### P086-CONF-002 - Critical - P086 session-store recovery proof is not integrated

ACP has general Claude session-store recovery (`acp/src/session.rs:425-443`) and unit tests for transcript recovery (`acp/src/session.rs:1472-1527`), but P086 settlement/readback does not persist the proposal-required session-store root/path/read timestamp/digest/latest-turn/recovery-result evidence. The P086 receipt schema required fields at `provider_session_attach_receipt_v2.schema.json:8-40` omit those fields.

### P086-CONF-003 - Major - Attach receipt v2 is missing required correlation fields

The proposal requires target stage/agent execution ids, request fingerprint, prompt-turn marker id, provider request/turn ids when exposed, and session-store recovery fields. The current schema requires only `continuation_id`, `agent_execution_id`, `run_id`, provider session ids, adapter/process fields, phase, orphan flags, output flags, and timeout fields (`provider_session_attach_receipt_v2.schema.json:8-40`). The raw receipt builder mirrors that smaller shape (`executor.rs:7529-7561`).

### P086-CONF-004 - Major - Adapter support is duplicated in MCP admission instead of owned solely by the adapter capability

`agents.continue_work` checks provider-session resurrection support with a local provider-name match (`agents.rs:1114-1126`). The worker later uses ACP manager capability, but proposal section 3.2 requires adapter support to be part of the admission gate before enqueue. This is a drift-prone duplicate authority.

### P086-CONF-005 - Major - Resurrection crash/replay matrix is incomplete

The DB schema has `resurrection_phase`, but the domain layer has no typed phase enum and the worker does not use every required phase boundary. The visible tests cover duplicate `prompt_sent` claim prevention, but not the proposal's `launching`, `launched`, `attaching`, `attached_unprompted`, and ambiguous `prompting` crash/replay rules.

### P086-CONF-006 - Major - Canonical proposal gate fails

`./scripts/test-gate.sh proposal-086` failed while compiling `mcp-server`: `control-plane/crates/mcp-server/src/tools/runs.rs` references missing `P083LifecycleDenialCode` variants including `AdditionalPropertiesRejected`, `SchemaInvalid`, `MissingCallerRequestId`, `RollbackTargetInvalid`, `RollbackTargetRequired`, and `LifecycleStateInvalid`. A failing same-tree gate blocks any Ready verdict regardless of P086-specific partial progress.

## Track 2: Specialist Findings

### SEC-001 - Major - Provider session ids are logged in plaintext

The ACP manager logs `requested_provider_session_id` during resurrection attach (`control-plane/crates/acp/src/manager.rs:499-505`) and includes both expected and actual provider session ids in an identity-mismatch error string (`manager.rs:516-520`). Provider session ids are strong continuation handles and are intentionally redacted from public receipt artifacts. They should not be emitted to normal logs or propagated error text in raw form; use a stable hash/ref instead.

### REL-001 - Major - Output-only and replay fixtures do not match the proposal's acceptance matrix

The `proposal-086` gate checks broad source needles and basic Rust tests, but the output-only negative fixture is marked as a placeholder and there is no observed crash/replay matrix for each resurrection phase. This lets the current gate miss proposal-required behavior even aside from the compile failure.

### API-001 - Major - Receipt/readback contract is narrower than proposal text

The v2 receipt schema has `additionalProperties=false`, which is good, but the required field set is under-scoped. Because the schema rejects undeclared fields, adding prompt-turn/session-store/target fields later is a contract migration rather than a compatible readback extension.

## Security Diff Summary

Security helper result: triggered.

Triggered categories:

- `auth`
- `dos_resource_limits`
- `filesystem_subprocess_boundary`
- `parser_boundary`
- `public_ingress`
- `secrets_redaction_privacy`
- `unsafe_crypto_dependency`

Independent security pass:

- Raw receipt storage is DB-backed and operator-scoped, which is directionally consistent with the proposal.
- Observer/reviewer projections redact sensitive fields.
- The main actionable security issue found in this pass is `SEC-001`: plaintext provider session id logging/error text.

## Verification Log

| Command | Result | Notes |
| --- | --- | --- |
| `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/security_sensitive_diff.py --root "/Users/user/Documents/Chainworks Forge" --json` | Passed helper, triggered | Required independent security pass completed. |
| `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/implementation_surface_fingerprint.py --root "/Users/user/Documents/Chainworks Forge" --json` | Passed helper | Required lenses: api-contract, apple-ui-ux, architecture, observability-rollout, performance, reliability, security. |
| `./scripts/test-gate.sh proposal-086` | Failed | Rust domain, ACP, DB, engine, MCP agents, and GraphQL readback portions ran before a later `mcp-server` compile failure in `tools/runs.rs`. |
| Evidence reads with `rg`, `nl`, and schema inspection | Completed | Focused on proposal, adapter capability, admission, engine worker, receipts, session-store recovery, tests, fixtures, and docs. |

## Scorecard

| Area | Score | Rationale |
| --- | --- | --- |
| Contract fidelity | Partial | Basic Claude resurrection exists; output-only/session-store/prompt-turn/replay tails missing. |
| Behavioral proof | Partial | Happy path and mismatch tests exist; acceptance matrix incomplete and gate fails. |
| API/schema quality | Partial | Schemas are strict, but attach receipt v2 omits required fields. |
| Reliability | Not Ready | Crash/replay phase proof is incomplete. |
| Security | Not Ready | Session ids leak into logs/error text. |
| Observability/readback | Partial | Metrics/readback exist but do not expose all required behaviors. |
| Documentation closeout | Not Ready | Reference docs describe implemented behavior more broadly than the current evidence proves. |

## Residual And Follow-Up Work Required

1. Fix the current `mcp-server` compile break so `./scripts/test-gate.sh proposal-086` can complete.
2. Add an explicit `output_only_recovery` contract, request mode, receipt fields, no-source-change enforcement, and tests replacing the placeholder fixture.
3. Extend P086 resurrection receipt/readback to include the proposal-required target ids, request fingerprint, prompt-turn marker id, provider request/turn ids when available, and session-store recovery fields.
4. Bind Claude session-store recovery into the P086 resurrection settlement/readback path with durable ownership proof and daemon-level lost-terminal-response tests.
5. Replace MCP provider-name hardcoding with adapter capability truth for admission, or persist adapter capability truth into admission inputs without duplicating support rules.
6. Add a typed `ResurrectionPhase` domain enum and crash/replay tests for each proposal phase.
7. Redact provider session ids from logs and propagated error strings.
8. Re-run the specialist helper and add Apple UI/UX and performance reviewers if those surfaces remain in the final P086 diff before any Ready claim.

## Readiness Checklist

- [x] Proposal read and scoped to current text.
- [x] Prior review discovery performed.
- [x] Security helper run and independent security pass completed.
- [x] Specialist surface helper run and coverage recorded.
- [x] Core implementation surfaces inspected.
- [x] Same-tree canonical P086 gate attempted.
- [ ] Same-tree canonical P086 gate passes.
- [ ] All proposal requirements are implemented or explicitly out of scope in proposal text.
- [ ] Output-only recovery is implemented and proven.
- [ ] Session-store recovery is implemented and proven as P086 evidence.
- [ ] Crash/replay acceptance matrix is implemented and proven.
- [ ] Security finding SEC-001 is resolved.

## Final Verdict

Not Ready.

P086 has meaningful partial implementation: Claude can be attached by provider session id in a new ACP child, identity mismatch fails before prompt, and DB/readback infrastructure exists. It still cannot be closed as implemented because several proposal-required tails are missing or only partially represented: output-only recovery, P086-specific session-store recovery evidence, full receipt correlation fields, phase/replay proof, adapter-owned admission truth, and a passing canonical gate. The full-implementation tail gate blocks Ready/Ready with Risks because these are promised P086 behaviors, not optional follow-ups.
