# Proposal 086 Implementation Audit R4

## Metadata

| Field | Value |
| --- | --- |
| Proposal | `docs/proposals/086-agent-work-continuation-and-lead-directed-session-resumption.md` |
| Implementation target | Current worktree at `/Users/user/Documents/Chainworks Forge/.chainworks/worktrees/cw-implement-proposal-086-agent-w-976f3d1b` |
| Compare base | Implicit current worktree; no PR base or commit range supplied |
| Branch | `cw/implement-proposal-086-agent-w/976f3d1b` |
| HEAD | `9b79b0667ed9ea0c67659fe4f47e47a60118feab` |
| Audit timestamp | `2026-05-23 13:29:23 EEST` |
| Report path | `docs/proposals/086-agent-work-continuation-and-lead-directed-session-resumption_IMPLEMENTATION_AUDIT_R4.md` |
| Prior proposal-review reuse | Not reused |
| Overall Conformance | Partial |
| Overall Implementation Readiness | Not Ready |
| Audit Confidence | Medium-High |

## Implementation Target / Compare Base

This audit covers the dirty implementation worktree, not only committed `HEAD`. The worktree contains staged and unstaged P086 implementation changes across Rust control-plane crates, schemas, examples, docs, and gates, plus untracked prior implementation audit reports and new P086 files such as:

- `control-plane/crates/daemon/tests/proposal_086_mcp_continuation_live_reuse.rs`
- `control-plane/crates/db/migrations/066_p086_supervised_worker_provider_process.sql`
- `docs/proposals/086-agent-work-continuation-and-lead-directed-session-resumption_IMPLEMENTATION_AUDIT_R1.md`
- `docs/proposals/086-agent-work-continuation-and-lead-directed-session-resumption_IMPLEMENTATION_AUDIT_R2.md`
- `docs/proposals/086-agent-work-continuation-and-lead-directed-session-resumption_IMPLEMENTATION_AUDIT_R3.md`

Only this R4 report was written by this audit pass.

## Prior Proposal-Review Reuse Summary

The bundled discovery helper returned no proposal-review artifacts for this proposal:

```json
{
  "artifacts": [],
  "proposal_path": "/Users/user/Documents/Chainworks Forge/.chainworks/worktrees/cw-implement-proposal-086-agent-w-976f3d1b/docs/proposals/086-agent-work-continuation-and-lead-directed-session-resumption.md",
  "repo_root": "/Users/user/Documents/Chainworks Forge/.chainworks/worktrees/cw-implement-proposal-086-agent-w-976f3d1b"
}
```

Reuse classification: Not reused. Existing implementation audit reports were ignored for reviewer selection, per skill rules.

## Selected Reviewers

| Reviewer | Why selected |
| --- | --- |
| `rust_arch_reviewer` | P086 changes Rust crate boundaries across MCP, engine, ACP, DB, workflow catalog, daemon tests, and recovery. |
| `rust_reliability_reviewer` | The proposal is driven by live-session liveness, idempotency, duplicate-send prevention, cancellation, worker supervision, and daemon restart recovery. |
| `api_contract_reviewer` | MCP request/response schemas, GraphQL readback, artifact schemas, and canonical request/response evidence are explicit contracts. |
| `observability_rollout_reviewer` | P086 requires rollout gates, metrics, evidence bundles, fixtures, stale-worker proof, and operator-readable readiness evidence. |

Rejected close alternatives:

- `macos_ui_reviewer`: No SwiftUI P086 implementation was changed. The missing/proven UI readback surface is handled as a readiness/API gap.
- `security_reviewer`: Auth and validation risks are important but contained in operator/agent MCP admission, schema validation, and side-effect checks already covered by API/reliability.
- `performance_reviewer`: No proposal latency or throughput target is central to the implemented slice; performance is secondary to correctness and durable proof.
- `product_reviewer`: Product metrics are listed in the proposal, but implementation risk is primarily service/contract/rollout rather than product-market fit.

## Proposal State And Contract Summary

Proposal state: Active. The proposal file exists, and Phase 5 expansion has been split out to P093 in the proposal itself.

Core proposal sources:

- Core decision: proposal lines 105-127.
- Initial scope and exclusions: proposal lines 131-161.
- Eligibility and mode-specific checks: proposal lines 205-244.
- Daemon restart and orphan recovery: proposal lines 246-269.
- Triggers and lead-auto policy: proposal lines 273-331.
- MCP input/output: proposal lines 609-655.
- GraphQL and SwiftUI readback/no-mutation contract: proposal lines 675-717.
- Metrics: proposal lines 1120-1137.
- Required tests: proposal lines 1141-1185.
- Acceptance criteria: proposal lines 1189-1208.

Proposal contract summary:

- Add server-owned `agents.continue_work`.
- Support `live_handle_continuation` through an existing live ACP session generation.
- Support provider-session resurrection only when an adapter explicitly supports attach/resume by provider session id; otherwise fail closed with `provider_session_resurrection_unsupported`.
- Allow operator-triggered MCP continuation and lead-directed automatic continuation when a `lead_continuation_decision_v1` artifact passes server validation.
- Limit initial role support to `code_writer`; fail closed for lead/reviewer/security/release/push/upload/distribution lanes and unresolved side effects.
- Keep SwiftUI read-only: no in-app Continue control, no GraphQL mutation, but GraphQL/SwiftUI readback may show continuation state/evidence.
- Persist durable continuation metadata, request fingerprints, idempotency behavior, side-effect rows, evidence artifacts, worktree/test/readback data, and no-progress classification.
- Prevent duplicate provider sends after `prompt_sent` and reconcile from evidence instead.
- Reap or fail closed around stale/orphan ACP subprocesses after daemon restart.
- Track proposal metrics needed for rollout and future limit observability.

## Platform / Product Scope

Apple platform scope: macOS readback only. The proposal explicitly forbids an in-app Continue command but allows read-only UI evidence.

Backend/service scope: Rust control-plane service, worker, API, data, recovery, ACP runtime, schemas, and rollout evidence.

Product/rollout scope: Operator trust in continuation evidence, no-progress classification, lead-auto guardrails, and metrics for future policy limits.

## Primary Service Flows

1. Operator MCP live-handle continuation:
   `agents.continue_work` validates a completed `code_writer` agent execution, admits an idempotent row, queues `ProcessContinuation`, reuses the live ACP session, sends the mode-reset prompt, and materializes terminal evidence artifacts.

2. Lead-directed continuation:
   An Agent or Operator request with `trigger_kind=lead_auto` supplies a lead decision artifact, SHA, instruction hash, and server-validated target context before any admission row is created.

3. Provider-session resurrection / unsupported fail-closed:
   A resurrection request is rejected as unsupported unless a future adapter explicitly enables attach/resume by provider session id.

4. Readback:
   MCP and GraphQL expose continuation status, history, candidates, artifact ids, failure/no-progress reason, and redacted sensitive fields for non-operator principals.

5. Crash/replay/recovery:
   The worker records prompt-sent before provider I/O, refuses duplicate prompt sends after that point, reconciles stale prompt-sent rows, handles cancellation races, records supervised workers, and attempts stale-provider-process cleanup after daemon restart.

## Implementation Fingerprint

Stack tags:

- Rust
- SQLite
- MCP Streamable HTTP / JSON-RPC tool surface
- GraphQL readback
- ACP subprocess runtime
- macOS SwiftUI read-only client boundary

Surface tags:

- MCP tools: `control-plane/crates/mcp-server/src/tools/agents.rs`
- DB migrations/repos: `control-plane/crates/db/migrations/065_p086_agent_work_continuations.sql`, `066_p086_supervised_worker_provider_process.sql`, `control-plane/crates/db/src/repos/agent_work_continuations.rs`
- Domain model: `control-plane/crates/domain/src/continuation.rs`
- Engine worker/recovery/cancellation: `control-plane/crates/engine/src/executor.rs`, `control-plane/crates/engine/src/recovery.rs`, `control-plane/crates/engine/src/cancellation.rs`
- ACP runtime: `control-plane/crates/acp/src/manager.rs`, `control-plane/crates/acp/src/transport.rs`
- GraphQL readback: `control-plane/crates/graphql-server/src/schema.rs`, `control-plane/crates/graphql-server/src/types/continuation.rs`
- Catalog opt-in: `control-plane/crates/workflow/src/catalog.rs`, `examples/agents/agents.yaml`
- Schemas/evidence/gates: `docs/reference/p086/schemas/**`, `docs/evidence/rollout-contract/**`, `scripts/test-gate.sh`

Risk tags:

- Idempotency and duplicate provider-send prevention
- Runtime session liveness
- Recovery and orphan process cleanup
- MCP/GraphQL/schema contract drift
- Lead-auto authority and safety policy
- Evidence quality and rollout truth
- SwiftUI readback completeness

## Proposal Fidelity Inventory

Matches:

- `agents.continue_work`, `agents.continuation_status`, and `agents.continuation_candidates` are registered MCP tools.
- Operator-only MCP continuation and Agent-principal `lead_auto` exception are implemented.
- Core live-handle path reuses an existing ACP session through `reuse_existing_session=true`.
- Same-session continuation now has daemon-level integration coverage.
- Provider resurrection requests fail closed with `provider_session_resurrection_unsupported`.
- `code_writer` catalog opt-in is modeled in frozen catalog shape and example catalog.
- Side-effect, approval, forbidden stage, session/provider, role, and capability guards exist.
- Durable continuation rows, idempotency fingerprints, worker supervision, side-effect ledger rows, and artifact ids exist.
- Terminal evidence artifacts are materialized to files rather than high-volume SQLite payloads.
- GraphQL exposes read-only continuation queries and no continuation mutation.
- Duplicate prompt-sent requests do not resend provider I/O.
- Recovery now records durable provider process binding and has process-group signal logic.

Divergences:

- The MCP `agents.continue_work` response is async/minimal, not the proposal's terminal output shape with response hash/session/provider/artifact ids.
- Lead-auto admission and artifact validation exist, but no lead orchestration path was found that automatically emits the decision and invokes continuation.
- Canonical request artifacts still hardcode `caller_principal_id` as `"operator"` even for potential `lead_auto`.
- Provider-session resurrection has only unsupported fail-closed behavior; no adapter-supported attach/resume path exists.
- SwiftUI app readback is not implemented or tested.
- Proposal metrics are mostly absent beyond an active continuation count in runtime health/projections.
- Recovery evidence records reap policy/outcome but not explicit signal/deadline fields required by the proposal.
- Rollout readback fixture uses synthetic ids and hashes while the gate labels it as rollout evidence.

Ambiguities / Evidence Gaps:

- The proposal says SwiftUI "may show" readback but acceptance criterion 9 says "UI can read continuation status through GraphQL"; this audit treats app readback as required but partially implemented because GraphQL exists.
- No adapter currently declares provider-session resurrection support, so supported resurrection behavior is not verifiable in this implementation target.
- `agents.continuation_candidates` is optional first version in the proposal, but current candidate quality is thinner than the suggested live-compatible/useful-progress set.
- Recovery has process-group reap code, but no focused test proves a real orphan provider process is signaled and observed dead.
- GraphQL continuation code exists, but `cargo test -p graphql-server continuation` matched zero tests.

## Requirement Summary

| Status | Count |
| --- | ---: |
| Implemented | 8 |
| Partially Implemented | 10 |
| Missing | 0 |
| Not Verifiable | 0 |
| Out of Scope | 0 |

Because at least one in-scope requirement is partially implemented and none are classified Missing, Overall Conformance is Partial.

## Detailed REQ Audit

| ID | Requirement | Proposal source | Status | Evidence types | Implementation mapping / evidence | Gap / note |
| --- | --- | --- | --- | --- | --- | --- |
| REQ-001 | Initial support is `code_writer`; excluded roles and side-effect lanes fail closed. | Lines 138-161 | Implemented | proposal, code, config, tests-run | `examples/agents/agents.yaml:1667`; `control-plane/crates/mcp-server/src/tools/agents.rs:746`; `proposal-086` gate passed. | Focused first-role scope is respected. |
| REQ-002 | Server-owned `agents.continue_work` MCP operation exists. | Lines 107-127, 277-295 | Implemented | proposal, code, schema, tests-run | `control-plane/crates/mcp-server/src/tools/agents.rs:57`, `:993`; MCP tests in `proposal-086` passed 33 tests. | Operation is server-owned and not SwiftUI-owned. |
| REQ-003 | Live-handle continuation uses same live ACP provider session and does not create a new session generation. | Lines 113-116, 226-230, 1145-1146 | Implemented | code, runtime-test, tests-run | Worker calls ACP with `reuse_existing_session=true` at `control-plane/crates/engine/src/executor.rs:5329`; ACP dispatches reuse at `control-plane/crates/acp/src/manager.rs:766`; daemon integration test `proposal_086_mcp_continuation_live_reuse.rs:469` asserts one session and two prompts. | Core primary flow now has same-tree daemon-level proof. |
| REQ-004 | Provider-session resurrection either attaches/resumes by provider session id for supported adapters or fails closed unsupported. | Lines 231-244, 1161-1163, 1193-1195 | Partially Implemented | proposal, code, tests-run | Unsupported mode rejection in `control-plane/crates/mcp-server/src/tools/agents.rs:1177`; example catalog disables resurrection with fail-closed config at `examples/agents/agents.yaml:1684`. | No supported adapter attach/resume path or supported-resurrection test exists. Current implementation satisfies unsupported fail-closed but not the future supported mode. |
| REQ-005 | Lead-directed automatic continuation through `lead_continuation_decision_v1`. | Lines 297-319, 721+, 1195 | Partially Implemented | proposal, code, schema, tests-run | Lead artifact validation in `control-plane/crates/mcp-server/src/tools/agents.rs:452`, `:866`, `:1398`; MCP tests cover target/safety/budget validation. | No lead orchestrator/closeout path was found that automatically emits the decision and invokes continuation; canonical request evidence remains operator-shaped. |
| REQ-006 | Eligibility validates run/stage/agent/session/provider/mode/family/worktree/runtime/side-effects/stage/policy/prompt guard. | Lines 205-244 | Partially Implemented | proposal, code, tests-run | MCP validates role/capability/session/provider/forbidden stage/side effects/pending approvals and live-session preconditions in `agents.rs`; DB side-effect check in `agent_work_continuations.rs:239`. | Count limits, worktree/runtime compatibility depth, candidates, and policy accounting remain incomplete. |
| REQ-007 | Durable continuation data model, idempotency, fingerprints, worker supervision, and side-effect ledger. | Lines 335-547 | Implemented | migration, code, tests-run | Migration `065_p086...sql:11`, `:118`, `:162`; atomic admission in `agent_work_continuations.rs:436`; DB lifecycle tests passed 9 tests. | Status vocabulary differs from proposal examples but model is durable and tested. |
| REQ-008 | Duplicate prompt-sent replay never resends and reconciles instead. | Lines 1172-1181, 1205-1208 | Partially Implemented | code, tests-run | Prompt-sent guard in `agent_work_continuations.rs:627`; worker reconciliation path in `executor.rs:4861`, `:4765`; DB and engine tests passed. | Reconciliation still uses post-created worktree changes rather than full transcript/tool evidence. |
| REQ-009 | Evidence is spooled to files and includes request, response, worktree, tests, result/no-progress, and report readback. | Lines 551-609, 1199-1200 | Partially Implemented | code, schema, tests-run | Artifact schemas under `docs/reference/p086/schemas/artifacts`; terminal artifact materialization in `executor.rs:4182`; daemon integration test verifies response/result/evidence artifacts. | Evidence is present, but tool-trace/transcript depth and diff ownership remain thin. |
| REQ-010 | Continuation prompt uses canonical mode-reset template and is not retry/output repair/checkpoint rehydration. | Lines 766-940, 1197 | Partially Implemented | code, tests-run | Prompt header at `executor.rs:4088`; test at `executor.rs:15097`; daemon test asserts header at `proposal_086_mcp_continuation_live_reuse.rs:472`. | Full proposal prompt wording, lead-specific prompt, closeout requirements, and explicit anti-`CHAINWORKS_OUTPUT` guard are not fully represented. |
| REQ-011 | Continuation remains separate from retry, output repair, and checkpoint recovery. | Lines 35-42, 1193-1204 | Implemented | code, tests-run | Distinct `ProcessContinuation` work item; live path uses `reuse_existing_session`; provider resurrection unsupported path does not fall back to fresh retry. | No ordinary retry fallback was found. |
| REQ-012 | Daemon restart recovery terminates/reaps stale ACP subprocesses and fails closed if unverified. | Lines 246-269, 1165-1171 | Partially Implemented | code, migration, tests-found | Migration `066_p086...sql`; process binding in `executor.rs:5103`; process-group signal code in `recovery.rs:110`; recovery loop in `recovery.rs:966`. | No runtime test proves real orphan process reap; evidence omits explicit signal/deadline fields. |
| REQ-013 | GraphQL exposes read-only continuation status/history/candidates and no continuation mutation. | Lines 675-704, 1156 | Implemented | code, tests-run | Query fields in `graphql-server/src/schema.rs:1149`, `:1180`; `MutationRoot` has no continuation mutation; proposal gate checks no mutation. | `cargo test -p graphql-server continuation` matched zero tests, so direct GraphQL test coverage is absent. |
| REQ-014 | SwiftUI can read continuation status but cannot invoke continuation. | Lines 123-127, 706-717, 1157-1158, 1201 | Partially Implemented | code-search, proposal | No SwiftUI invocation surface found; searches under `Chainworks Forge/` found no P086 continuation UI usage. | The app also does not display P086 readback/history/evidence, so the read side is not implemented/proven. |
| REQ-015 | Agent catalog capability opt-in and fail-closed disabled defaults. | Lines 1038-1075 | Implemented | config, code, tests-run | `ContinuationCapabilityYaml` in `workflow/src/catalog.rs:161`; example `code_writer` opt-in in `examples/agents/agents.yaml:1667`; MCP capability guard tests passed. | Broader frozen-snapshot fixture coverage could be expanded. |
| REQ-016 | Metrics track counts, fresh sessions avoided, progress/no-progress, tests, changed files, trigger split, budget, orphan/reap, resurrection attach. | Lines 1120-1137 | Partially Implemented | telemetry, code-search | `continuation_active_count` exists in projections/storage health. | The proposal metric set is otherwise absent. |
| REQ-017 | Required tests cover same-session, no fresh generation, eligibility rejects, side effects, lead policy, evidence, GraphQL, no-progress, worktree readback, resurrection, orphan recovery, idempotency, prompt-sent, reconciliation. | Lines 1141-1185 | Partially Implemented | tests-found, tests-run | `proposal-086` gate passes and now includes daemon live-reuse integration; readback/negative/operator gates pass. | No supported resurrection test, real orphan reap test, SwiftUI readback test, or GraphQL continuation test matched. |
| REQ-018 | MCP input/output contract matches proposal. | Lines 609-655 | Partially Implemented | schema, code | Request schema exists and validates strict properties; accepted response is documented in `agents.continue_work.response.schema.json`. | Wire response is accepted/replay/rejected async shape, not the proposal's terminal output with response/session/provider/artifact ids. |

## Reviewer / Lens Scorecard

| Lens | Result | Top risk | Confidence |
| --- | --- | --- | --- |
| Proposal conformance | Partial | Lead-auto orchestration, UI readback, metrics, resurrection/recovery proof remain partial. | Medium-High |
| Rust architecture | Mostly aligned | Canonical request evidence still hardcodes operator identity and lead-auto authority is MCP-local rather than orchestrated. | Medium |
| Rust reliability | Not Ready | Orphan reap lacks runtime proof and reconciliation evidence remains weak. | Medium-High |
| API contract | Not Ready | `agents.continue_work` response diverges from proposal terminal output; GraphQL has no matching continuation tests. | Medium-High |
| Observability/rollout | Not Ready | Metrics mostly absent and rollout readback fixture is synthetic. | High |
| Release readiness | Not Ready | Canonical P086 gate passes, but major conformance/readiness gaps remain. | Medium-High |

## Routed Specialist Findings

### API-001: `agents.continue_work` response does not match the proposal terminal output contract

Reviewer: `api_contract_reviewer`
Severity: Major
Confidence: Medium-High
Related requirements: REQ-018, REQ-009
Evidence types: proposal, schema, code

Evidence references:

- Proposal lines 637-654 require output fields including `status`, `continuation_mode`, `response_fingerprint_sha256`, `session_generation_id`, `provider_session_id`, and artifact ids.
- `docs/reference/p086/schemas/mcp/agents.continue_work.response.schema.json` requires only `outcome`.
- `control-plane/crates/mcp-server/src/tools/agents.rs:1580` returns accepted/replay payloads with `outcome`, `continuation_id`, `status`, and `request_fingerprint_sha256`.

Why it matters:

The implementation may be intentionally async, but the proposal's public command contract reads as terminal output. Clients following the proposal cannot obtain the promised terminal fields from the command response.

Recommended action:

Either update the command to return the proposal terminal response after worker settlement, or explicitly split the contract into accepted response plus readback response in the proposal/reference docs and prove the readback path covers all proposed fields.

Acceptance criteria:

- Contract docs and schemas agree with the implemented wire response.
- Tests prove that every proposal output field is available either directly from `agents.continue_work` or from the documented readback sequence.

### ARCH-001: Lead-auto is admission-capable but not an automatic lead orchestration flow

Reviewer: `rust_arch_reviewer`
Severity: Major
Confidence: Medium
Related requirements: REQ-005, REQ-010
Evidence types: proposal, code, tests-found

Evidence references:

- Proposal lines 118-122 and 297-319 describe lead-directed automatic continuation after a valid decision artifact.
- Lead artifact validation exists in `control-plane/crates/mcp-server/src/tools/agents.rs:452` and `:866`.
- `control-plane/crates/engine/src/executor.rs:4153` writes canonical request `caller_principal_id` as `"operator"`.
- Searches found no lead orchestrator path outside MCP that emits `lead_continuation_decision_v1` and invokes continuation automatically.

Why it matters:

The server can validate a supplied lead-auto request, but the proposal's automatic lead-directed flow is not complete. The hardcoded operator identity also weakens audit evidence for actual lead-triggered continuations.

Recommended action:

Wire the lead result path to emit the decision artifact and request continuation through the server, carry the actual principal/trigger identity into canonical request evidence, and use/test the lead-specific prompt template.

Acceptance criteria:

- A lead-generated decision artifact can trigger continuation without manual operator MCP assembly.
- Canonical request evidence records the actual caller/trigger identity.
- Tests cover successful lead-auto and safety-policy rejection paths.

### REL-001: Orphan ACP reap exists in code but is not fully proven or fully evidenced

Reviewer: `rust_reliability_reviewer`
Severity: Major
Confidence: Medium-High
Related requirements: REQ-012, REQ-017
Evidence types: proposal, migration, code, tests-found

Evidence references:

- Proposal lines 254-262 require durable old pid/session/provider id/signal/deadline/outcome evidence and fail-closed behavior.
- `control-plane/crates/db/migrations/066_p086_supervised_worker_provider_process.sql` adds provider process binding columns.
- `control-plane/crates/engine/src/recovery.rs:110` sends SIGTERM/SIGKILL to the registered process group.
- `control-plane/crates/engine/src/recovery.rs:1018` records pid/group/uid and attempted/verified fields.
- No focused test was found that spawns a stale provider process, restarts/repairs, and verifies the process is reaped.

Why it matters:

The implementation is directionally correct, but the hardest recovery behavior is still only code-inspected. The recorded evidence also does not include explicit signal/deadline fields named in the proposal.

Recommended action:

Add a recovery test with a real child process group and stale supervised-worker row, assert signal/reap/dead outcome, and include explicit signal/deadline fields in the stale-worker recovery evidence.

Acceptance criteria:

- Test proves successful reap and failed/unverified reap outcomes.
- Recovery evidence includes old pid, session generation, provider session id, signal(s), deadline/timing, outcome, timestamp, and error reason.

### REL-002: Prompt-sent reconciliation still relies on thin worktree evidence

Reviewer: `rust_reliability_reviewer`
Severity: Major
Confidence: Medium
Related requirements: REQ-008, REQ-009
Evidence types: proposal, code, tests-run

Evidence references:

- Proposal lines 1176-1181 require reconciliation to read worktree/transcript evidence and settle without sending another continuation turn.
- `control-plane/crates/engine/src/executor.rs:4646` checks whether changed files have mtime after continuation creation.
- `control-plane/crates/engine/src/executor.rs:4765` reconciles with no provider result/transcript.
- Engine test `p086_reconciliation_requires_post_continuation_worktree_change` passed, but covers only worktree mtime classification.

Why it matters:

This prevents the old "any dirty file" false positive, but it still cannot prove that the changed file came from the continuation turn. An unrelated post-created file can classify the continuation as succeeded.

Recommended action:

Record a pre-continuation worktree baseline, provider prompt/send receipt, transcript/tool evidence when available, and changed-file ownership before classifying reconciliation as succeeded.

Acceptance criteria:

- Reconciliation succeeds only when continuation-scoped evidence exists.
- Tests cover unrelated post-created dirty files, transcript-only no-progress, and changed-file-without-provider-evidence cases.

### OPS-001: Rollout readback fixture is still synthetic despite passing the gate

Reviewer: `observability_rollout_reviewer`
Severity: Major
Confidence: High
Related requirements: REQ-016, REQ-017
Evidence types: config, tests-run

Evidence references:

- `docs/evidence/rollout-contract/operator-readback/p086-continuation-full-surface.fixture.json` contains `p086-fixture-continuation-0001`, repeated placeholder-like UUIDs, and `aaaaaaaa...` / `bbbbbbbb...` hashes.
- `scripts/test-gate.sh` readback gate checks field coverage and `rollout_contract_status='pass'`, not runtime provenance.
- `./scripts/test-gate.sh p086-continuation-readback` passed.

Why it matters:

The gate can pass on synthetic readback data. That is acceptable for schema coverage but not for rollout truth or implementation closeout.

Recommended action:

Generate the operator readback fixture from the daemon integration or a captured local run, and have the gate assert non-synthetic ids/hashes and provenance.

Acceptance criteria:

- Fixture references actual continuation rows/artifacts or explicitly declares itself schema-only.
- Gate fails on synthetic ids/hashes when claiming rollout readiness.

### OPS-002: Proposal metrics are mostly absent

Reviewer: `observability_rollout_reviewer`
Severity: Major
Confidence: High
Related requirements: REQ-016
Evidence types: proposal, code-search, telemetry

Evidence references:

- Proposal lines 1120-1137 list required metrics.
- Searches found `continuation_active_count` in projections/storage health.
- No implementation evidence was found for fresh-session avoided count, useful/no-progress rates, tests passed after continuation, changed files after continuation, trigger success split, follow-up validation success, provider/session budget impact, or resurrection attach success/failure.

Why it matters:

Without these metrics, the system cannot evaluate whether continuation is safer or more useful than retry, or tune future automatic continuation limits.

Recommended action:

Add the proposed metric set or explicitly revise the proposal to defer metrics to P093/another rollout proposal.

Acceptance criteria:

- Metrics cover operator vs lead trigger split, success/no-progress, changed files/tests, fresh-session avoidance, budget impact, orphan reap, and resurrection attach outcomes.
- A focused gate or test proves metric writes/readback.

### READY-001: SwiftUI readback is not implemented or proven

Reviewer: `api_contract_reviewer`
Severity: Major
Confidence: Medium-High
Related requirements: REQ-014, REQ-017
Evidence types: proposal, code-search

Evidence references:

- Proposal lines 675-717 allow UI readback and forbid UI invocation.
- Searches under `Chainworks Forge/` found no P086 continuation readback types, queries, views, or `agents.continue_work` UI invocation.
- Existing Swift matches are unrelated workflow/resume continuations or Swift async continuations.

Why it matters:

The app correctly has no Continue command, but operators also cannot inspect P086 continuation history/evidence inside the canonical SwiftUI shell.

Recommended action:

Either implement read-only SwiftUI continuation history/evidence consumption through GraphQL or explicitly defer app readback in proposal/reference docs.

Acceptance criteria:

- SwiftUI shows continuation status/history/evidence without any executable Continue affordance, or docs narrow P086 to API-only readback.
- UI or snapshot tests prove no invocation control is rendered.

### READY-002: GraphQL continuation behavior lacks direct test coverage

Reviewer: `api_contract_reviewer`
Severity: Minor
Confidence: High
Related requirements: REQ-013, REQ-017
Evidence types: code, tests-run

Evidence references:

- GraphQL query implementation exists in `control-plane/crates/graphql-server/src/schema.rs:1149` and `:1180`.
- `CARGO_TARGET_DIR=target/proposal-086-gate cargo test -p graphql-server continuation` passed with zero matched tests.
- The canonical proposal gate checks for no mutation but does not execute a continuation GraphQL query.

Why it matters:

The read-only surface is code-inspected but not directly proven through GraphQL execution.

Recommended action:

Add a GraphQL test that seeds continuation records/artifact ids and queries status/history/candidates, plus a schema assertion that no continuation mutation exists.

Acceptance criteria:

- `cargo test -p graphql-server continuation` runs at least one test.
- Test verifies status/history fields and absence of mutation.

## Readiness Checklist

| Check | Result | Notes |
| --- | --- | --- |
| Canonical same-tree P086 gate | Passed | `./scripts/test-gate.sh proposal-086` passed on audited worktree and includes daemon live-reuse integration. |
| Core operator MCP live-handle flow | Passed | Daemon test submits MCP request, runs worker, proves one ACP session and terminal artifacts. |
| Provider-session resurrection unsupported fail-closed | Passed | MCP rejects with `provider_session_resurrection_unsupported`. |
| Supported provider-session resurrection | Not implemented | No adapter support or supported attach/resume test. |
| Lead-auto validation | Partial | Artifact validation exists; automatic orchestration path not found. |
| Duplicate prompt-sent no resend | Passed, with risk | Guard exists; reconciliation evidence remains thin. |
| Orphan process recovery | Partial | Process-group signal code exists; no real reap test and evidence lacks signal/deadline fields. |
| GraphQL readback | Partial | Code exists; direct GraphQL continuation tests matched zero. |
| SwiftUI readback | Not Ready | No app readback implementation/proof; no Continue control found. |
| Accessibility/localization/permissions/entitlements | Not in implemented UI scope | No P086 UI was added; risk becomes relevant if readback UI is implemented. |
| Privacy/auth risk | Partially covered | Operator and Agent-principal guardrails exist; non-operator redaction tests exist. |
| Metrics | Not Ready | Only active count found; proposal metric set missing. |
| Rollout evidence | Not Ready | Focused gates pass, but readback fixture remains synthetic. |
| Full regression suite | Not run | Canonical P086 proposal gate was run and passed; `fast`/`full` app gates were not run. |

## Verification Log

Commands run from `/Users/user/Documents/Chainworks Forge/.chainworks/worktrees/cw-implement-proposal-086-agent-w-976f3d1b` unless otherwise noted:

- `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/report_path.py ...` -> selected R4 report path.
- `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/discover_prior_review.py ...` -> no prior review artifacts.
- `git rev-parse HEAD` -> `9b79b0667ed9ea0c67659fe4f47e47a60118feab`.
- `git branch --show-current` -> `cw/implement-proposal-086-agent-w/976f3d1b`.
- `git status --short --branch` -> dirty worktree with staged/unstaged P086 files and untracked R1/R2/R3 reports, daemon test, and migration 066.
- `./scripts/test-gate.sh proposal-086` -> Passed.
  - Domain continuation tests: 4 passed.
  - DB `proposal_086_continuation_lifecycle`: 9 passed.
  - Engine P086 tests: 3 passed.
  - MCP `tools::agents`: 33 passed.
  - Daemon `proposal_086_mcp_continuation_live_reuse`: 1 passed.
- `./scripts/test-gate.sh p086-continuation-readback` -> Passed.
- `./scripts/test-gate.sh p086-continuation-negative-fixtures` -> Passed, 16 fixtures present and valid.
- `./scripts/test-gate.sh p086-continuation-operator-report` -> Passed.
- `CARGO_TARGET_DIR=target/proposal-086-gate cargo test -p daemon --test proposal_086_mcp_continuation_live_reuse` from `control-plane/` -> Passed, 1 test.
- `CARGO_TARGET_DIR=target/proposal-086-gate cargo test -p graphql-server continuation` from `control-plane/` -> Passed with zero matched tests; not counted as behavioral GraphQL coverage.
- Focused searches inspected P086 implementation, schemas, metrics, lead-auto, recovery, and SwiftUI readback surfaces.

## Final Verdict

Overall Conformance: Partial.

Overall Implementation Readiness: Not Ready.

The main improvement since the previous audit pass is substantial: the primary operator MCP live-handle continuation path now has daemon-level integration proof, and terminal artifact schema mismatches appear corrected in both code and gate checks. The implementation is still not closeout-ready because lead-auto is not an automatic orchestration flow, SwiftUI readback is absent, metrics are mostly missing, provider-session resurrection remains unsupported-only, orphan recovery lacks runtime proof and full evidence fields, GraphQL readback lacks direct tests, and rollout readback evidence remains synthetic.

Recommended next actions:

1. Decide whether `agents.continue_work` is async accepted-response by design; if yes, amend proposal/reference docs and prove readback returns the terminal output fields.
2. Wire real lead-auto orchestration and fix canonical request caller identity for lead-triggered continuations.
3. Add a real orphan-process reap test and include signal/deadline fields in recovery evidence.
4. Add GraphQL continuation tests and either implement or explicitly defer SwiftUI readback.
5. Replace synthetic rollout readback with generated runtime evidence.
6. Add or explicitly defer the proposal metric set before closeout.

