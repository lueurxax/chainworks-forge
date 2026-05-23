# Proposal 086 Implementation Audit R5

## Metadata

| Field | Value |
|---|---|
| Proposal | `docs/proposals/086-agent-work-continuation-and-lead-directed-session-resumption.md` |
| Proposal status | Draft |
| Audit timestamp | 2026-05-23 17:46:56 EEST |
| Audit mode | proposal-implementation-audit |
| Implementation target | Worktree `/Users/user/Documents/Chainworks Forge/.chainworks/worktrees/cw-implement-proposal-086-agent-w-976f3d1b` |
| Branch | `cw/implement-proposal-086-agent-w/976f3d1b` |
| HEAD | `9b79b0667ed9ea0c67659fe4f47e47a60118feab` |
| Working tree | Dirty; audited current working tree, including uncommitted implementation files and untracked tests/migrations |
| Compare base | Implicit current worktree; no PR base supplied |
| Prior proposal-review reuse | Not reused |
| Prior implementation audits | R1-R4 present beside proposal; not used for reviewer selection or as proof |
| Overall conformance | Partial |
| Overall implementation readiness | Not Ready |
| Audit confidence | High for inspected Rust/MCP/GraphQL/Swift surfaces; Medium for runtime production behavior beyond fixture integration |

## Prior Review Reuse

`discover_prior_review.py` returned no proposal-review artifacts for this proposal. Reviewer selection was therefore routed from the current proposal and implementation evidence. Existing `IMPLEMENTATION_AUDIT_R1` through `R4` files were intentionally excluded from reviewer selection.

## Selected Reviewers

| Reviewer | Reason |
|---|---|
| `rust_arch_reviewer` | Rust control-plane crates, DB schema, worker, ACP manager, and orchestration boundaries are central. |
| `rust_reliability_reviewer` | Idempotency, duplicate prompt prevention, cancellation, replay, daemon recovery, and orphan process reaping are core P086 risks. |
| `api_contract_reviewer` | MCP schemas, GraphQL readback, artifact schemas, and Swift read-model contracts are proposal requirements. |
| `observability_rollout_reviewer` | Durable metrics, rollout fixtures, negative fixtures, and operator report gates are required. |
| `macos_ui_reviewer` | Proposal now includes SwiftUI read-only continuation status/history, with explicit no-command UI constraint. |

Rejected close alternatives:

| Reviewer | Reason rejected |
|---|---|
| `security_reviewer` | Auth and validation were inspected under API/Rust reliability; no new secret, public network, or privilege boundary beyond existing operator/agent principal checks was primary. |
| `performance_reviewer` | No p95/p99 or throughput target in P086; performance risk is limited to metric/event bounding and handled as rollout. |
| `product_reviewer` | Product value is explicit, but remaining risks are contract/reliability/readiness rather than product direction. |
| `apple_arch_reviewer` | Swift change is read-model/presentation only, not a new app architecture path. |

## Proposal Contract Summary

P086 adds server-owned agent work continuation via `agents.continue_work` so an implementation/code-writing agent can continue useful same-session work instead of starting a fresh retry. The proposal distinguishes retry, output repair, live-handle continuation, and provider-session resurrection. It requires strict eligibility checks, durable continuation truth, evidence artifacts on disk, idempotency and duplicate-send protection, lead-directed automatic continuation under policy, recovery/orphan reaping, GraphQL/SwiftUI readback without an in-app command surface, and rollout metrics.

Platform/product scope:

| Scope | Classification |
|---|---|
| Apple | macOS read-only SwiftUI status/history surface |
| Backend/service | Rust control-plane MCP, GraphQL, DB, worker, recovery, metrics |
| Data | SQLite migrations, side-effect ledger, artifact metadata, metric events |
| Rollout | Operator readback fixture, negative fixtures, proposal gates |
| Out of scope | P093 Phase 5 soak/expansion; provider-specific resurrection support for adapters that do not yet expose attach/resume |

## Primary Flows Audited

1. Operator calls MCP `agents.continue_work` for a completed/failed stage-owned `code_writer`; admission validates scope, identity, side effects, catalog opt-in, idempotency, and enqueues a continuation.
2. Background worker claims the continuation row, verifies the live ACP handle/session, records attach and side-effect ledger rows, sends the canonical mode-reset prompt, materializes evidence artifacts, and settles terminal readback.
3. Lead agent emits `lead_continuation_decision_v1`; engine validates target/hash/safety/capability and admits a `lead_auto` continuation through the same durable transaction.
4. Daemon restart recovery detects stale supervised continuation workers, closes/reaps registered ACP provider process groups, records evidence, and moves affected rows to reconciliation.
5. GraphQL and SwiftUI read continuation history/metrics/evidence without exposing a GraphQL mutation or in-app Continue command.

## Fidelity Inventory

Matches:

- `agents.continue_work`, `agents.continuation_status`, and `agents.continuation_candidates` are registered in MCP (`control-plane/crates/mcp-server/src/tools/agents.rs:21-153`).
- Live-handle continuation uses the existing ACP session and does not create a fresh session in the integration test (`control-plane/crates/daemon/tests/proposal_086_mcp_continuation_live_reuse.rs:352-585`).
- Provider-session resurrection fails closed as unsupported for current adapters (`control-plane/crates/mcp-server/src/tools/agents.rs:1216-1244`).
- Lead-auto engine orchestration reads decision artifacts, validates target/capability/safety, writes a durable admission row, and enqueues `ProcessContinuation` (`control-plane/crates/engine/src/executor.rs:3992-4235`).
- Evidence artifacts and DB pointers are materialized (`control-plane/crates/engine/src/executor.rs:4708-5015`).
- Recovery persists provider pid/pgid/uid and reaps process groups with TERM/KILL evidence (`control-plane/crates/engine/src/recovery.rs:120-235`, `1070-1205`).
- GraphQL is read-only for P086 continuation and exposes history plus metric summary (`control-plane/crates/graphql-server/src/schema.rs:1205-1234`).
- SwiftUI decodes and renders continuation readback and its test asserts no continuation mutation query (`Chainworks ForgeTests/Proposal031ThinGraphQLReadBoundaryTests.swift:620-655`).

Divergences:

- Proposal input names `continuation_mode`, while the implemented MCP request schema/tool require `mode` and reject additional properties (`docs/proposals/086-agent-work-continuation-and-lead-directed-session-resumption.md:613-633`, `docs/reference/p086/schemas/mcp/agents.continue_work.request.schema.json:8-44`, `control-plane/crates/mcp-server/src/tools/agents.rs:93-150`).
- The response schema declares `error.data` with `additionalProperties=false` and no allowed properties, but implemented rejection responses include machine-readable fields such as `failure_reason`, `agent_execution_id`, and queue counts (`docs/reference/p086/schemas/mcp/agents.continue_work.response.schema.json:28-40`, `control-plane/crates/mcp-server/src/tools/agents.rs:1233-1241`, `1708-1717`, `1735-1750`).
- Lead-auto policy count limits from proposal section 7.3 are not enforced before admission. The atomic DB admission checks active rows and global saturation only, not max one lead-directed continuation per agent execution or max two per stage after terminal rows (`docs/proposals/086-agent-work-continuation-and-lead-directed-session-resumption.md:321-331`, `control-plane/crates/db/src/repos/agent_work_continuations.rs:878-979`, `control-plane/crates/engine/src/executor.rs:3992-4235`).
- Post-prompt reconciliation uses post-continuation worktree mtime plus a committed `provider_send` ledger row, but does not read persisted transcript evidence or explicit transcript absence before terminal settlement (`docs/proposals/086-agent-work-continuation-and-lead-directed-session-resumption.md:1208-1213`, `control-plane/crates/engine/src/executor.rs:5560-5607`).

Ambiguities / evidence gaps:

- Provider-session resurrection support for a real adapter remains deferred; this is acceptable under acceptance criterion 2 only because unsupported mode is explicit and fail-closed.
- Metrics are durable, but run-level summary currently reads only the newest 500 metric events (`control-plane/crates/db/src/repos/agent_work_continuations.rs:286-290`), which may undercount longer rollout histories.
- No UI runtime screenshot was captured; UI evidence is code review plus targeted Swift tests.

## Requirement Summary

| Status | Count |
|---|---:|
| Implemented | 14 |
| Partially Implemented | 4 |
| Missing | 0 |
| Not Verifiable | 0 |
| Out of Scope | 0 |

## Detailed Requirement Audit

| Req | Proposal source | Status | Evidence | Notes |
|---|---|---|---|---|
| REQ-001 Distinct work-continuation operation | Sections 2-3, lines 76-127 | Implemented | MCP/code/tests | Separate `agents.continue_work`; prompt says not retry/output repair/checkpoint rehydration. |
| REQ-002 Initial scope limited to code writer and non-side-effect lanes | Section 4, lines 131-161 | Implemented | code/tests/catalog | Catalog opt-in and forbidden stage/side-effect checks cover initial lane. |
| REQ-003 MCP request/response contract for bounded async admission | Section 12, lines 609-679 | Partially Implemented | schema/code/tests | Async accepted/replay/rejected response exists and gate passes, but request field name differs from proposal and rejected `error.data` does not validate against schema. See API-001/API-002. |
| REQ-004 Live-handle same-session continuation | Sections 2.4, 3, 21 | Implemented | code/tests-run | Worker checks live ACP session and daemon integration proves one session start plus two prompts. |
| REQ-005 Provider-session resurrection explicit fail-closed unsupported mode | Lines 231-242, 1192-1202, 1225-1227 | Implemented | code/tests-run/telemetry | Unsupported resurrection returns `provider_session_resurrection_unsupported` and records unsupported metric. |
| REQ-006 Eligibility and safety validation | Section 6, lines 205-244 | Partially Implemented | code/tests-run | Run/stage/agent/session/provider/worktree/capability/side-effect/release checks exist. Continuation count policy limits are incomplete. |
| REQ-007 Lead decision artifact can trigger server-owned continuation | Section 14, lines 745-754 | Implemented | code/tests-found | Engine scans materialized lead artifacts, validates hash/target/capability/safety, admits/enqueues. |
| REQ-008 Automatic continuation policy limits | Section 7.3, lines 321-331 | Partially Implemented | code/search | Side-effect/release restrictions exist; max 1 per agent and max 2 per stage are not enforced. |
| REQ-009 Durable data model, idempotency, and side-effect ledger | Section 8 and tests 22-24 | Implemented | migration/code/tests-run | Migrations 065-067, atomic `BEGIN IMMEDIATE` admission, conflict count, replay, active row, saturation, and ordered ledger implemented. |
| REQ-010 Duplicate prompt-sent replay never resends and reconciles from evidence | Tests 24-26, lines 1208-1213 | Partially Implemented | code/tests-run | Duplicate-send prevention is implemented. Reconciliation lacks transcript evidence readback before terminal settlement. |
| REQ-011 Canonical mode-reset prompt | Acceptance criterion 5 | Implemented | code/tests-run | Prompt includes P086 mode reset, identity, bounds, no side effects, no fresh restart. |
| REQ-012 Evidence artifacts on disk, not high-volume SQLite | Acceptance criteria 7-8 | Implemented | code/tests-run | Canonical request, attach receipt, worktree readback, evidence bundle, response snapshot, result/no-progress, and report artifacts are persisted as files and DB stores pointers. |
| REQ-013 GraphQL readback, no mutation | Section 13, lines 699-728 | Implemented | code/tests-run | Read fields and SDL mutation absence are tested. |
| REQ-014 SwiftUI readback/history only, no Continue command | Section 13, lines 730-741, tests 12a | Implemented | code/tests-run | Run detail query decodes continuation history/metrics and card renders passive status; test asserts no `continueWork`/`agentsContinueWork` in documents. |
| REQ-015 Metrics | Section 20, lines 1151-1168 | Implemented | migration/code/tests-run | Durable metric event table and run summary cover proposed categories. Bounded summary risk recorded as OPS-001. |
| REQ-016 Daemon restart/orphan ACP recovery | Section 7.1, lines 246-269, tests 19-21 | Implemented | code/tests-run | Durable provider process binding, UID/PGID guard, TERM/KILL, stale evidence, and reconciliation status are implemented and function-tested. |
| REQ-017 Catalog opt-in and unsupported snapshots fail closed | Eligibility, acceptance criteria | Implemented | code/tests-run | `continuation_capability` required in frozen catalog; missing/malformed support rejects rather than falling back to retry. |
| REQ-018 Required test matrix | Section 21, lines 1172-1217 | Partially Implemented | tests-run/search | Canonical gate is broad and passing, but no direct tests prove lead-auto count limits, rejected response schema instance validation, or transcript-backed reconciliation. |

## Reviewer / Lens Scorecard

| Lens | Conformance | Top risk | Confidence |
|---|---|---|---|
| Proposal contract | Partial | Explicit policy and API-schema gaps remain. | High |
| Rust architecture | Mostly sound | Large worker method has many responsibilities but follows existing repo style. | Medium |
| Rust reliability | Partial | Count-limit enforcement and transcript-backed reconciliation are incomplete. | High |
| API contract | Partial | Request field drift and invalid rejected-response schema. | High |
| Observability/rollout | Ready with minor risk | Metrics summary cap can undercount longer histories. | Medium |
| macOS UI | Implemented | No runtime screenshot, but read-only contract is tested. | Medium |
| Readiness | Not Ready | Major API/reliability findings remain despite passing gates. | High |

## Routed Specialist Findings

### API-001: MCP request contract uses `mode` while proposal callers are told to send `continuation_mode`

- Reviewer: `api_contract_reviewer`
- Severity: Major
- Confidence: High
- Related REQs: REQ-003, REQ-006
- Evidence types: proposal, schema, code
- Evidence:
  - Proposal request sample and text use `continuation_mode` (`docs/proposals/086-agent-work-continuation-and-lead-directed-session-resumption.md:613-633`).
  - Implemented request schema requires `mode` and has `additionalProperties=false` (`docs/reference/p086/schemas/mcp/agents.continue_work.request.schema.json:8-44`).
  - MCP tool schema mirrors `mode`, not `continuation_mode` (`control-plane/crates/mcp-server/src/tools/agents.rs:93-150`).
- Why it matters: A client following the proposal contract sends `continuation_mode` and gets rejected as an unknown field instead of admitted. That is a proposal/API contract break even though the current internal tests use the implementation-specific field.
- Recommended action: Either update proposal/reference contract to explicitly rename the field and provide compatibility/alias rationale, or accept both `continuation_mode` and `mode` while normalizing to the canonical internal field.
- Acceptance criteria: Schema, MCP tool spec, proposal text, and at least one request-instance test agree on the same field name; the stale field is either accepted as an alias with tests or explicitly documented as superseded.

### API-002: Rejected `agents.continue_work` responses do not validate against the reference response schema

- Reviewer: `api_contract_reviewer`
- Severity: Major
- Confidence: High
- Related REQs: REQ-003, REQ-018
- Evidence types: schema, code
- Evidence:
  - Response schema declares `error.data` as an object with `additionalProperties=false` and no declared properties (`docs/reference/p086/schemas/mcp/agents.continue_work.response.schema.json:28-40`).
  - Rejection paths return `error.data.failure_reason` and other fields, e.g. unsupported resurrection (`control-plane/crates/mcp-server/src/tools/agents.rs:1233-1241`), idempotency conflict (`control-plane/crates/mcp-server/src/tools/agents.rs:1708-1717`), and saturation queue counts (`control-plane/crates/mcp-server/src/tools/agents.rs:1735-1750`).
- Why it matters: The accepted path is schema-compatible, but the rejected path is part of the bounded admission contract. Current machine-readable errors are useful, but the published schema rejects them.
- Recommended action: Add explicit `error.data` properties or a bounded `oneOf`/`$defs` per failure class, then add response-instance schema validation tests for accepted, replay, and representative rejected outcomes.
- Acceptance criteria: Every implemented `agents.continue_work` outcome validates against `agents.continue_work.response.schema.json`; tests fail if a rejection includes an undeclared field.

### REL-001: Lead-auto count policy limits are not enforced after terminal continuations

- Reviewer: `rust_reliability_reviewer`
- Severity: Major
- Confidence: High
- Related REQs: REQ-006, REQ-008, REQ-018
- Evidence types: proposal, code, tests-found
- Evidence:
  - Proposal default policy limits require max 1 lead-directed continuation per agent execution and max 2 per stage execution (`docs/proposals/086-agent-work-continuation-and-lead-directed-session-resumption.md:321-331`).
  - Engine lead-auto admission validates artifact, target, capability, stage kind, side effects, approvals, and then admits/enqueues (`control-plane/crates/engine/src/executor.rs:3992-4235`), but no terminal count check is present.
  - DB atomic admission checks only duplicate idempotency, active continuation rows, and global saturation (`control-plane/crates/db/src/repos/agent_work_continuations.rs:809-979`). Its active-row query excludes terminal rows, so a second lead decision after a terminal row can pass if idempotency differs.
- Why it matters: A lead can emit multiple distinct valid decision artifacts and exceed the proposal's bounded automatic continuation policy. This is exactly the automated path that needs tighter limits than operator-triggered continuation.
- Recommended action: Enforce `trigger_kind='lead_auto'` count limits inside the same admission transaction, counting terminal and non-terminal rows per agent and per stage. Return a typed fail-closed reason, for example `lead_auto_policy_limit_exceeded`.
- Acceptance criteria: Tests prove the second lead-auto continuation for one agent is rejected, the third lead-auto continuation for one stage is rejected, operator-MCP policy remains separate, and concurrent admissions cannot race past the count.

### REL-002: Post-prompt reconciliation does not read transcript evidence before terminal settlement

- Reviewer: `rust_reliability_reviewer`
- Severity: Major
- Confidence: Medium
- Related REQs: REQ-010, REQ-012, REQ-018
- Evidence types: proposal, code, tests-run
- Evidence:
  - Proposal test 26 requires continuation reconciliation to read worktree/transcript evidence and settle without another provider turn (`docs/proposals/086-agent-work-continuation-and-lead-directed-session-resumption.md:1208-1213`).
  - Reconciliation currently reads worktree status and checks for a committed `provider_send` ledger row, then marks success from post-continuation worktree evidence (`control-plane/crates/engine/src/executor.rs:5560-5607`).
  - Transcript data is captured when a provider result is available during normal settlement (`control-plane/crates/engine/src/executor.rs:4708-4812`), but the reconciliation path passes `provider_result=None` and does not read a persisted transcript artifact before deciding success/no-progress.
- Why it matters: This avoids duplicate provider sends, which is good, but it cannot distinguish provider-authored work from unrelated post-continuation worktree changes as strongly as the proposal requires. The current `provider_send` ledger proves a prompt was sent, not that the changed files came from that prompt.
- Recommended action: Persist a transcript/tool-call receipt or explicit transcript absence reason at provider-send/response boundary, and have reconciliation read that evidence together with worktree diff before terminal settlement.
- Acceptance criteria: A crash-after-prompt integration test proves reconciliation reads transcript/tool evidence when present, records an explicit absence reason when not present, and never marks success from worktree mtime plus `provider_send` alone.

### OPS-001: Run-level metrics summary can undercount longer histories

- Reviewer: `observability_rollout_reviewer`
- Severity: Minor
- Confidence: Medium
- Related REQs: REQ-015
- Evidence types: code, migration, tests-run
- Evidence:
  - Durable event rows include run/stage/agent/continuation ids (`control-plane/crates/db/migrations/067_p086_continuation_metric_events.sql:7-26`).
  - Run summary reads only 500 events (`control-plane/crates/db/src/repos/agent_work_continuations.rs:286-290`).
- Why it matters: P086 metrics are meant to feed future limit observability. A 500-event cap can undercount multi-continuation runs because one terminal continuation can emit many metric events.
- Recommended action: Aggregate in SQL by metric/label for the run, expose a `truncated` flag, or document and test the cap as a UI readback limit rather than a metrics truth limit.
- Acceptance criteria: A test with more than 500 metric rows either returns exact totals or explicitly reports truncation so operators do not treat partial totals as complete.

## Readiness Checklist

| Check | Status | Evidence |
|---|---|---|
| Canonical P086 gate on audited tree | Pass | `./scripts/test-gate.sh proposal-086` |
| Rust DB lifecycle/metrics tests | Pass | 10 tests passed in `proposal_086_continuation_lifecycle` |
| Rust engine P086 tests | Pass | 6 tests passed, including mode-reset, reconciliation mtime guard, and process-group reaping |
| MCP agents tests | Pass | 33 tests passed |
| GraphQL continuation readback test | Pass | Included in canonical P086 gate |
| Daemon live ACP reuse integration | Pass | Included in canonical P086 gate; proves one provider session and two prompts |
| Swift readback test | Pass | 85 selected P031 tests passed; P086 readback test passed |
| Operator readback fixture gate | Pass | `./scripts/test-gate.sh p086-continuation-readback` |
| Negative fixture gate | Pass | `./scripts/test-gate.sh p086-continuation-negative-fixtures` |
| Operator report gate | Pass | `./scripts/test-gate.sh p086-continuation-operator-report` |
| UI runtime/screenshot evidence | Not run | Not required for conformance, but no screenshot/runtime UI session captured |
| Blocking proposal gaps | Present | API-001, API-002, REL-001, REL-002 |

## Verification Log

| Command | Result | Notes |
|---|---|---|
| `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/discover_prior_review.py <proposal>` | Pass | No prior proposal-review artifacts returned. |
| `./scripts/test-gate.sh proposal-086` | Pass | Rust unit/integration gates passed; Swift readback test passed; result bundle `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T//chainworks-test-gates/proposal-086-swift-readback-20260523-174920.xcresult`. |
| `./scripts/test-gate.sh p086-continuation-readback` | Pass | Operator readback fixture coverage passed. |
| `./scripts/test-gate.sh p086-continuation-negative-fixtures` | Pass | All 16 negative fixtures present and valid. |
| `./scripts/test-gate.sh p086-continuation-operator-report` | Pass | Operator report field coverage passed. |

Warnings observed:

- Rust emitted existing dead-code/unused-variable warnings.
- Swift emitted existing actor isolation/deprecation warnings.
- No warning caused a gate failure.

## Final Verdict

Overall conformance is Partial. The implementation now proves the major live-handle continuation, worker, evidence, recovery, GraphQL, SwiftUI readback, and rollout fixture paths on the audited tree. However, P086 is not ready for closeout because four proposal-contract issues remain:

1. MCP request field naming diverges from the proposal.
2. Rejected MCP responses do not validate against the published response schema.
3. Lead-auto count limits are not enforced.
4. Post-prompt reconciliation lacks transcript-backed evidence before terminal settlement.

Recommended next actions:

1. Align MCP request/response contracts and add schema instance tests for accepted/replay/rejected outputs.
2. Enforce lead-auto per-agent and per-stage count limits transactionally, with concurrency tests.
3. Add transcript/tool-evidence-backed reconciliation and crash-after-prompt integration coverage.
4. Decide whether the 500-event metrics summary cap is a UI limit or replace it with exact SQL aggregation/truncation reporting.
