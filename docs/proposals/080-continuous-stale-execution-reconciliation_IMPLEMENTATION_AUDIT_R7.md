# P080 Implementation Audit R7 - Continuous Stale Execution Reconciliation

## Metadata

| Field | Value |
|---|---|
| Proposal | `docs/proposals/080-continuous-stale-execution-reconciliation.md` |
| Proposal ID | P080 |
| Proposal revision | `p080-refined-2026-06-02-r28` |
| Audit report | `docs/proposals/080-continuous-stale-execution-reconciliation_IMPLEMENTATION_AUDIT_R7.md` |
| Audit time | 2026-06-20T19:32:11Z |
| Repository | `/Users/user/Documents/Chainworks Forge` |
| HEAD | `0e6482c82b588b74a76294a225e68286bfe37fa4` |
| Working tree | Dirty; audit target is current same-tree working copy |
| Prior proposal-review reuse | Not reused; no proposal-review artifacts were discovered for P080 |
| Track 1 REQ conformance | Partial |
| Track 2 implementation readiness | Not Ready |
| Final verdict | Not Ready - material contract gaps and canonical gate failure |

## Target And Compare Base

This audit compares the P080 proposal contract against the current same-tree implementation at HEAD
`0e6482c82b588b74a76294a225e68286bfe37fa4` plus local working-tree changes. The working tree is
broadly dirty, so findings are scoped to the P080-specific implementation surfaces and the P080 gate
result observed in this tree.

The P080-specific implementation surfaces inspected were:

- `control-plane/crates/db/migrations/086_p080_stale_execution_reconciliation.sql`
- `control-plane/crates/domain/src/p080.rs`
- `control-plane/crates/db/src/repos/p080.rs`
- `control-plane/crates/db/src/repos/work_items.rs`
- `control-plane/crates/mcp-server/src/tools/p080.rs`
- `control-plane/crates/graphql-server/src/types/p080.rs`
- `control-plane/crates/graphql-server/src/schema.rs`
- `control-plane/crates/engine/src/executor.rs`
- `control-plane/crates/engine/src/release/receipt.rs`
- `control-plane/crates/mcp-server/src/tools/reports.rs`
- `scripts/test-gate.sh`
- `docs/reference/test-gates.md`
- `docs/runbooks/p080-stale-execution-repair.md`
- `docs/evidence/rollout-contract/**/p080*.json`
- `docs/evidence/rollout/p080/phase-scoped-same-tree-proof.json`

Prior implementation-audit reports were not used for reviewer selection or verdicting. They remain
historical context only.

## Selected Reviewers

| Reviewer | Why selected |
|---|---|
| `rust_arch_reviewer` | P080 changes Rust control-plane boundaries across DB, engine, MCP, GraphQL, work queue, and release receipt readback. |
| `rust_reliability_reviewer` | The proposal is primarily about stale execution truth, recurrence, idempotent repair, race aborts, and recovery safety. |
| `rust_security_reviewer` | The diff touches auth gates, public MCP/GraphQL ingress, parser limits, redaction, HMAC keys, and process/session ownership. |
| `api_contract_reviewer` | P080 defines closed MCP/GraphQL/readback schemas, cursor contracts, error vocabularies, and multi-lane parity. |
| `observability_rollout_reviewer` | Acceptance depends on rollout gates, fixtures, metrics, operator readback, runbooks, and phase evidence. |

Rejected close alternatives:

- `apple-ui-ux`: no P080 Swift/AppKit surface was found in this phase, and future diagnostics UI appears explicitly separated into P099.
- `rust_performance_reviewer`: resource limits and scan bounds were reviewed under reliability and rollout; no throughput benchmark is required to reach this verdict.
- Product/design reviewers: the proposal is backend control-plane safety infrastructure with no active product UI surface in the audited implementation.

## Proposal Contract

P080 requires the daemon to reconcile non-terminal run truth while the Rust control plane is live,
distinguish active work from stale running truth, safely repair retryable non-side-effect work, and
fail closed around side effects, helpers, authorization, rollout, and duplicate operator requests.

The explicit contract includes:

- continuous live reconciliation for non-terminal runs;
- durable execution/session/lease witnesses before stale classification or cleanup;
- no automatic side-effect retry unless P076 returns `retry_safe`;
- no interruption of active prompts with useful progress;
- stable `p080_readback_v1` across MCP, GraphQL, run reports, and release receipts;
- durable cooldown, recurrence, repair idempotency, and operator request dedup replay fences across restarts;
- closed MCP/GraphQL schemas, parser/resource limits, redaction, and authorization-before-durable-write ordering;
- rollout evidence, negative fixtures, metrics, and operator runbook coverage;
- no SwiftUI repair tools, no enabled `hold`, no permanent-hold clear, and no P080-owned `acp_prompt_stale` repair in this phase.

## Platform And Product Scope

The audited scope is the Rust control-plane parity implementation: SQLite migration and repos,
daemon reconciliation loop, MCP tools, GraphQL read models/subscription, run report/release receipt
readback, rollout fixtures, metrics, and runbooks.

The SwiftUI app remains the canonical operator shell, but no P080 Swift implementation surface was
found. That is acceptable for the current proposal phase because P080 explicitly excludes macOS
repair UI and a follow-up diagnostics window is tracked separately.

## Primary Flows

1. Live daemon tick checks `live_disable`, requires `detection_only`, classifies running executions,
   and writes diagnostic readback/events.
2. Operator calls `p080.diagnostics.get.v1` to page `p080_readback_v1` rows through closed MCP
   schemas with run-scope auth, rollout gates, cursor validation, and redaction.
3. Operator calls `p080.reconcile.request.v1` with `repair_if_safe` for `acp_startup_stale`; when
   Phase 2 class rollout is enabled, the handler requeues a running `InvokeAgent` work item and marks
   the prior execution failed.
4. Read-only GraphQL query/subscription exposes diagnostics rows and revalidates authorization and
   rollout state before emitting subscription data.
5. Run reports and release receipts include P080 reconciliation sections for operator readback.

## Fidelity And Divergence

Implemented with reasonable fidelity:

- The migration adds P080 tables for helper leases, events, recurrence, operator dedup, deferral,
  cursors, readback, rollout control, and projections.
- MCP schemas are closed and include P080 readback, diagnostic, reconcile, clear-hold, and rollout
  control shapes.
- The diagnostic MCP path performs resource, schema, closed-field, run-scope authorization, rollout,
  cursor, projection, and redaction checks.
- The GraphQL query/subscription surfaces are read-only and include authorization/run-scope checks,
  live rollout gates, pagination, and subscription revalidation.
- Run report and release receipt readback paths are present.
- A focused positive repair slice exists for `acp_startup_stale` running `InvokeAgent` work items.

Material divergences:

- The canonical P080 gate failed before tests ran.
- The mutating MCP repair path requires an `operator_request_dedup_key` but never reads or writes the
  durable dedup table; replay and stale-fingerprint conflict behavior is not enforced.
- The live daemon loop remains diagnostic-only for `acp_startup_stale`; it does not perform continuous
  automatic repair within the proposal's two-healthy-interval success criterion.
- Active repair is limited to manual MCP `repair_if_safe` for `acp_startup_stale`; helper/process
  cleanup, release side-effect reconciliation through P076, scheduler ownership drift repair,
  permanent hold, and P037 prompt-stale delegation are not fully implemented or proved.
- The rollout evidence file is a same-tree unit/gate manifest, not canary/soak/false-positive sample
  or operator sign-off evidence.
- The runbook still says Phase 1 performs no scheduler capacity repair, while current code and
  evidence claim a Phase 2 `acp_startup_stale` requeue path.
- GraphQL subscription behavior is implemented but not included in the named P080 gate test list.

## Residual Scope And Ownership

| Residual | Blocking? | Owner / disposition |
|---|---:|---|
| Manual hold and clear-hold semantics | No for this phase | P098 appears to own this follow-up; P080 keeps actions disabled. |
| macOS diagnostics UI | No for this phase | P099 appears to own the future UI. |
| Durable operator dedup replay fence in mutating repair | Yes | No separate owner found; must be fixed in P080 before readiness. |
| Continuous automatic repair within two healthy intervals | Yes | No separate owner found; current live loop only diagnoses. |
| Helper/process lease cleanup and PID-reuse protection | Yes for full P080 | Tables exist, behavior is disabled/unproved. |
| P076 side-effect `retry_safe` integration | Yes for full P080 | Side-effect repair remains disabled/unproved. |
| P037 prompt-stale delegation readback | Yes for full P080 delegation claim | No complete classifier/delegation proof found. |
| GraphQL subscription contract tests | Yes for API confidence | Code exists, gate coverage does not. |
| Rollout soak/canary/readiness evidence | Yes for readiness | Current evidence is not phase-promotion evidence. |

## Specialist Coverage Matrix

| Surface | Reviewer lens | Coverage |
|---|---|---|
| SQLite migration and DB repos | Rust architecture, reliability, security | Reviewed for additive shape, readback/event writes, repair, recurrence, idempotency, dedup primitives. |
| Daemon live loop | Rust reliability, observability | Reviewed for live-disable gates, classifier execution, diagnostic writes, and repair absence. |
| MCP ingress and handlers | Security, API contract, reliability | Reviewed for auth, resource limits, closed schemas, dedup handling, rollout gates, repair path, redaction. |
| GraphQL query/subscription | API contract, security | Reviewed for read-only exposure, auth revalidation, pagination, cursor/event contract, and test coverage. |
| Run reports/release receipts | API contract, rollout | Reviewed for P080 lane presence and stable readback sections. |
| Evidence/runbooks/gates | Observability/rollout | Reviewed for canonical gate, fixture inventory, phase proof, and operator guidance alignment. |

## Requirement Summary

| REQ | Requirement | Status |
|---|---|---|
| REQ-001 | Continuous live daemon reconciliation of non-terminal runs | Partially Implemented |
| REQ-002 | Durable running-truth classification using execution/session/ownership witnesses | Partially Implemented |
| REQ-003 | Safe repair of retryable non-side-effect stale scheduler capacity | Partially Implemented |
| REQ-004 | Side-effect work fail-closed unless P076 says `retry_safe` | Partially Implemented |
| REQ-005 | Provider/helper cleanup only with durable ownership and PID-reuse protection | Partially Implemented |
| REQ-006 | Stable `p080_readback_v1` across MCP, GraphQL, run report, release receipt | Implemented with gate risk |
| REQ-007 | Durable recurrence and repair idempotency | Partially Implemented |
| REQ-008 | Durable operator request dedup replay fence | Missing |
| REQ-009 | Auth, parser, resource-limit, redaction, and closed-schema gates before durable writes | Partially Implemented |
| REQ-010 | Read-only GraphQL diagnostics, pagination, and subscription behavior | Partially Implemented |
| REQ-011 | Rollout controls, metrics, fixtures, runbook, and phase evidence | Partially Implemented |
| REQ-012 | Canonical proposal gate passes on same tree | Missing |
| REQ-013 | Manual hold/clear and macOS UI are not accidentally enabled in P080 | Implemented |

## Detailed Requirement Audit

### REQ-001 - Continuous live daemon reconciliation

Status: Partially Implemented.

Evidence:

- The engine starts a P080 reconciliation loop (`control-plane/crates/engine/src/executor.rs:5408`).
- Each tick checks `live_disable`, requires `detection_only`, calls the classifier, and writes
  diagnostic events/readback (`control-plane/crates/engine/src/executor.rs:5550`,
  `control-plane/crates/engine/src/executor.rs:5584`, `control-plane/crates/engine/src/executor.rs:5604`,
  `control-plane/crates/engine/src/executor.rs:5738`).

Gap:

- The loop writes `repair_action=diagnose_only` and explicitly performs no actual ACP reset
  (`control-plane/crates/engine/src/executor.rs:5706`). It does not prove the success criterion that
  retryable stale non-side-effect rows are repaired within two healthy intervals after stale grace.

### REQ-002 - Durable running-truth classification

Status: Partially Implemented.

Evidence:

- The classifier reads running `agent_executions` with joined `session_generations` and work-item
  evidence, then writes `p080_readback_v1` rows (`control-plane/crates/db/src/repos/p080.rs:171`,
  `control-plane/crates/db/src/repos/p080.rs:179`, `control-plane/crates/db/src/repos/p080.rs:257`).
- The migration includes helper lease tables (`control-plane/crates/db/migrations/086_p080_stale_execution_reconciliation.sql:11`).

Gap:

- Classification is effectively limited to `acp_startup_stale`, `scheduler_ownership_drift`,
  warmup, and useful-running states. Full helper ownership, side-effect, prompt-stale delegation,
  and ambiguous-owner behavior is not fully implemented or proved.

### REQ-003 - Safe retryable non-side-effect repair

Status: Partially Implemented.

Evidence:

- MCP `repair_if_safe` requires Operator class, a target tuple, `stale_class=acp_startup_stale`,
  a dedup key, and Phase 2 rollout enablement before repair
  (`control-plane/crates/mcp-server/src/tools/p080.rs:1133`,
  `control-plane/crates/mcp-server/src/tools/p080.rs:1186`,
  `control-plane/crates/mcp-server/src/tools/p080.rs:1208`,
  `control-plane/crates/mcp-server/src/tools/p080.rs:1226`).
- The DB repair path validates stale readback, requeues the work item, advances recurrence, derives
  a repair idempotency key, records an event, and writes repaired readback
  (`control-plane/crates/db/src/repos/p080.rs:1747`).
- Work-item repair only updates a matching running `InvokeAgent` row and CASes status back to pending
  (`control-plane/crates/db/src/repos/work_items.rs:731`, `control-plane/crates/db/src/repos/work_items.rs:778`).
- MCP tests include a positive `p080_repair_if_safe_phase2_requeues_acp_startup_stale` case
  (`scripts/test-gate.sh:363`).

Gap:

- This is manual MCP repair, not continuous daemon repair.
- It covers only `acp_startup_stale`.
- It is unsafe to call this readiness-complete while the operator dedup replay fence is missing.

### REQ-004 - Side-effect fail-closed behavior

Status: Partially Implemented.

Evidence:

- Phase proof declares side-effect-adjacent repair disabled without P076 truth
  (`docs/evidence/rollout/p080/phase-scoped-same-tree-proof.json:44`).
- Runbook tells operators not to retry release, publish, git, upload, or distribution work unless
  P076 reports `retry_safe` (`docs/runbooks/p080-stale-execution-repair.md:11`).

Gap:

- No active P076 `retry_safe` integration or end-to-end side-effect readback path was proved.

### REQ-005 - Helper/process ownership before cleanup

Status: Partially Implemented.

Evidence:

- Helper lease and member tables exist in the migration
  (`control-plane/crates/db/migrations/086_p080_stale_execution_reconciliation.sql:11`).
- Phase evidence declares helper process signaling disabled
  (`docs/evidence/rollout/p080/phase-scoped-same-tree-proof.json:42`).

Gap:

- No implemented helper cleanup/reap path with durable parent-chain/PID-reuse proof was found.

### REQ-006 - Shared readback lanes

Status: Implemented with gate risk.

Evidence:

- MCP publishes P080 closed output schemas (`control-plane/crates/mcp-server/src/tools/p080.rs:72`).
- GraphQL defines P080 enums/readback and diagnostics types
  (`control-plane/crates/graphql-server/src/types/p080.rs:1`).
- GraphQL query maps DB rows into diagnostics connection items
  (`control-plane/crates/graphql-server/src/schema.rs:3412`).
- GraphQL subscription exposes snapshot/update/removal/projection events with auth revalidation
  (`control-plane/crates/graphql-server/src/schema.rs:6926`).
- Run reports include `p080_reconciliation`
  (`control-plane/crates/mcp-server/src/tools/reports.rs:250`).
- Release receipts include optional P080 reconciliation readback
  (`control-plane/crates/engine/src/release/receipt.rs:7`).

Risk:

- The canonical gate failed before exercising these tests in this tree.

### REQ-007 - Recurrence and repair idempotency

Status: Partially Implemented.

Evidence:

- Recurrence and repair idempotency tables exist.
- The repair path advances recurrence and derives a daemon-keyed HMAC repair idempotency key
  (`control-plane/crates/db/src/repos/p080.rs:1803`, `control-plane/crates/db/src/repos/p080.rs:1837`).
- HMAC key material is loaded from env/file with a generated local file fallback
  (`control-plane/crates/db/src/repos/p080.rs:1982`).

Gap:

- This does not cover operator request replay semantics. That is REQ-008 and is currently missing.

### REQ-008 - Operator request dedup replay fence

Status: Missing.

Evidence:

- The migration and DB repo define durable dedup storage and fence fields
  (`control-plane/crates/db/migrations/086_p080_stale_execution_reconciliation.sql:176`,
  `control-plane/crates/db/src/repos/p080.rs:1162`,
  `control-plane/crates/db/src/repos/p080.rs:1205`,
  `control-plane/crates/db/src/repos/p080.rs:1280`).
- The MCP repair handler extracts `_dedup_key` but never uses it
  (`control-plane/crates/mcp-server/src/tools/p080.rs:1208`).
- Searching the P080 MCP handler found no calls to `get_dedup_entry`, `get_dedup_response`, or
  `insert_dedup_entry`.

Impact:

- Repeated mutating requests with the same `operator_request_dedup_key` can execute the handler path
  again instead of replaying the first response or returning a conflict when auth, policy, secret,
  rollout phase, repair-class enablement, live-disable generation, or request fingerprint changes.
- This directly violates P080 success criteria for stale replay prevention and idempotent operator
  repair.

### REQ-009 - Auth/parser/resource/redaction gates

Status: Partially Implemented.

Evidence:

- MCP diagnostics enforces resource limits, schema version, closed nested fields, run-scope auth,
  rollout gates, cursor validation, and redaction before returning rows
  (`control-plane/crates/mcp-server/src/tools/p080.rs:536`).
- MCP reconcile validates schema/action, rejects read-only repair, checks run scope, validates dedup
  key shape, and disallows dedup on `diagnose_only`
  (`control-plane/crates/mcp-server/src/tools/p080.rs:823`,
  `control-plane/crates/mcp-server/src/tools/p080.rs:889`,
  `control-plane/crates/mcp-server/src/tools/p080.rs:923`,
  `control-plane/crates/mcp-server/src/tools/p080.rs:951`,
  `control-plane/crates/mcp-server/src/tools/p080.rs:980`).
- The server has P080 duplicate-key tests in the gate list (`scripts/test-gate.sh:341`).

Gap:

- The security-sensitive mutating path is still missing the durable dedup replay fence.
- The gate failed before these protections were re-proved in this tree.

### REQ-010 - GraphQL diagnostics and subscription

Status: Partially Implemented.

Evidence:

- The query requires P080 diagnostics read authorization and run-scope checks before rollout gates
  (`control-plane/crates/graphql-server/src/schema.rs:3412`,
  `control-plane/crates/graphql-server/src/schema.rs:3601`).
- The subscription emits initial snapshot/update/projection events and revalidates auth and rollout
  before initial rows and each poll tick (`control-plane/crates/graphql-server/src/schema.rs:6926`,
  `control-plane/crates/graphql-server/src/schema.rs:7029`,
  `control-plane/crates/graphql-server/src/schema.rs:7147`,
  `control-plane/crates/graphql-server/src/schema.rs:7266`).

Gap:

- The P080 gate list includes only one GraphQL test, a read-only operator policy denial
  (`scripts/test-gate.sh:372`). It does not prove the subscription event contract, auth-loss
  termination, projection rebuild, or rate-shed behavior.

### REQ-011 - Rollout controls, metrics, fixtures, runbook, phase evidence

Status: Partially Implemented.

Evidence:

- The gate verifies migration presence, negative fixture inventory, operator readback fixture lanes,
  and rollout evidence JSON before building/tests (`scripts/test-gate.sh:7210`).
- The phase proof records implemented classes `detection_only` and `acp_startup_stale`
  (`docs/evidence/rollout/p080/phase-scoped-same-tree-proof.json:10`).

Gaps:

- The phase proof is a unit/gate manifest, not canary, soak, false-positive sampling, blast-radius,
  or operator sign-off evidence.
- The runbook says Phase 1 performs no ACP reset or scheduler capacity repair
  (`docs/runbooks/p080-stale-execution-repair.md:3`) while the same tree claims and tests a Phase 2
  `acp_startup_stale` requeue path.

### REQ-012 - Canonical gate

Status: Missing.

Evidence:

- Ran `./scripts/test-gate.sh proposal-080`.
- Result: fixture/evidence inventory passed, then `cargo build -p db -p mcp-server -p graphql-server`
  was killed with exit code 137 before any P080 Rust tests ran.
- The gate command builds the relevant crates before executing the P080 test list
  (`scripts/test-gate.sh:7303`).

Impact:

- Same-tree proof is absent. Under the proposal audit rules, this alone blocks any Ready or Ready
  with Risks verdict.

### REQ-013 - Non-goal actions remain disabled

Status: Implemented.

Evidence:

- `hold` returns `action_disabled_in_phase`; `clear_permanent_hold` is listed as disabled in this
  phase and covered in tool/gate naming (`control-plane/crates/mcp-server/src/tools/p080.rs:1118`,
  `scripts/test-gate.sh:360`).
- Phase evidence declares permanent hold clear disabled
  (`docs/evidence/rollout/p080/phase-scoped-same-tree-proof.json:6`).

## Reviewer Scorecard

| Lens | Score | Notes |
|---|---:|---|
| Rust architecture | 6/10 | Additive DB/API surfaces are coherent, but behavior is split between diagnostic loop and manual repair with stale phase comments/docs. |
| Rust reliability | 4/10 | ACP startup requeue slice exists, but continuous repair, dedup replay, full class coverage, and same-tree proof are missing. |
| Rust security | 5/10 | Good auth/parser/redaction work is visible, but the mutating dedup replay fence is absent on the security-sensitive repair path. |
| API contract | 6/10 | MCP/GraphQL/readback schemas are extensive; GraphQL subscription and dedup conflict contracts are not adequately proved. |
| Observability/rollout | 4/10 | Fixture inventory and metrics exist, but rollout evidence is not real phase evidence and the runbook is inconsistent with code. |

## Security-Sensitive Diff Summary

The security-sensitive scan triggered. Relevant categories were auth, public ingress, parser/resource
limits, secrets/redaction/privacy, filesystem key material, subprocess/process boundaries, and
dependency/crypto-adjacent handling.

Manual security pass covered:

- MCP `p080.diagnostics.get.v1`, `p080.reconcile.request.v1`, and clear/rollout tool boundaries.
- GraphQL query/subscription authorization and live revalidation.
- Redaction and closed-schema tamper handling.
- HMAC repair-idempotency key generation/storage.
- Helper/process lease scaffolding and disabled process signaling.
- Duplicate-key and parser limit gate coverage.

Security verdict: No Critical issue was identified in the inspected redaction/auth/read-only GraphQL
paths, but one Major security/reliability blocker remains: mutating operator repair does not enforce
the durable dedup replay fence required by the proposal.

Because this scan triggered and the canonical gate failed, P080 cannot be marked Ready or Ready with
Risks in this audit.

## Routed Specialist Findings

### SEC-001 / REL-001 - Major - Mutating repair does not enforce operator-request dedup replay

The P080 repair handler requires `operator_request_dedup_key`, but only binds it to `_dedup_key` and
never reads or writes `p080_operator_request_dedup_v1`. The DB repo already defines `get_dedup_entry`,
`get_dedup_response`, and `insert_dedup_entry`, including the intended fence fields, but those
functions are not called by the mutating MCP path.

Evidence:

- `control-plane/crates/mcp-server/src/tools/p080.rs:1208`
- `control-plane/crates/db/src/repos/p080.rs:1205`
- `control-plane/crates/db/src/repos/p080.rs:1280`

Required action:

- Before performing `repair_if_safe`, compute the request fingerprint and current fence tuple
  (principal class, auth policy generation, secret generation, rollout phase, repair-class enablement
  hash, live-disable generation).
- If the dedup row exists and all fences match, replay the stored response.
- If the row exists and any fence differs, return the closed P080 idempotency conflict error without
  mutating state.
- If no row exists, perform the mutation atomically with first-writer-wins dedup insertion, then cover
  replay and conflict behavior in Phase 2 tests.

### REL-002 - Major - Continuous repair guarantee is not implemented

The daemon loop classifies and writes diagnostic readback but does not repair stale rows. The actual
repair path is a manual MCP call and only supports `acp_startup_stale`.

Evidence:

- `control-plane/crates/engine/src/executor.rs:5706`
- `control-plane/crates/mcp-server/src/tools/p080.rs:1186`
- `control-plane/crates/db/src/repos/work_items.rs:731`

Required action:

- Either implement daemon-owned repair scheduling for the accepted retryable classes or narrow the
  proposal/reference/readiness claim to manual Phase 2 `acp_startup_stale` repair only.

### OPS-001 - Major - Canonical P080 gate failed before tests ran

`./scripts/test-gate.sh proposal-080` passed fixture inventory, then the build command was killed
with exit code 137 before executing the P080 Rust tests.

Evidence:

- `scripts/test-gate.sh:7303`

Required action:

- Make the same-tree proposal gate complete successfully. Until then, no readiness verdict above Not
  Ready is defensible.

### OPS-002 - Major - Rollout evidence and runbook are inconsistent with the implementation phase

The rollout proof says `acp_startup_stale` repair is implemented, but the operator runbook still says
P080 performs no scheduler capacity repair. The proof file is also not canary/soak/operator sign-off
evidence.

Evidence:

- `docs/evidence/rollout/p080/phase-scoped-same-tree-proof.json:10`
- `docs/runbooks/p080-stale-execution-repair.md:3`

Required action:

- Update runbooks/reference docs to match the actual promoted phase.
- Add real phase-promotion evidence: canary metrics, soak interval, false-positive sample review,
  blast-radius review, and operator sign-off.

### API-001 - Major - GraphQL subscription contract is not in the P080 gate

GraphQL subscription code exists and includes initial snapshot, row update/removal, projection
rebuild, rate-shed, and authorization-lost behavior, but the named P080 gate list includes only one
GraphQL test: read-only operator query denial.

Evidence:

- `control-plane/crates/graphql-server/src/schema.rs:6926`
- `scripts/test-gate.sh:372`

Required action:

- Add P080 gate tests for initial snapshot, row update/removal, projection rebuild/rate shed, and
  authorization lost on token/capability/run-scope/rollout changes.

### REL-003 - Major - Full stale-class matrix is scaffolded but not implemented/proved

The current implementation proves a narrow `acp_startup_stale` slice. Helper orphan drift, release
side-effect drift, scheduler ownership drift repair, P037 prompt-stale delegation, and permanent hold
semantics are either disabled, follow-up-owned, or unproved.

Evidence:

- `docs/evidence/rollout/p080/phase-scoped-same-tree-proof.json:2`
- `control-plane/crates/db/src/repos/p080.rs:239`
- `docs/runbooks/p080-stale-execution-repair.md:19`

Required action:

- Explicitly retire these from P080's readiness claim with follow-up owners, or implement/prove the
  promised detection/readback/delegation behavior for each class.

## Readiness Checklist

| Check | Result |
|---|---|
| Proposal contract extracted and mapped to REQs | Pass |
| Prior proposal-review artifacts discovered and reused | Not applicable; none discovered |
| Security-sensitive diff reviewed | Pass with unresolved Major finding |
| Same-tree `proposal-080` gate passed | Fail |
| Mutating operator repair idempotent across duplicate requests and fence changes | Fail |
| Continuous daemon repair guarantee proved | Fail |
| Side-effect retry remains fail-closed | Partial |
| Helper/process cleanup protected by durable ownership proof | Partial |
| MCP closed schemas/auth/parser/redaction boundaries present | Partial |
| GraphQL query/subscription read-only API present | Partial |
| GraphQL subscription contract covered by gate tests | Fail |
| Readback lanes present across MCP/GraphQL/report/receipt | Pass with gate risk |
| Rollout evidence sufficient for promotion | Fail |
| Operator runbook matches implementation phase | Fail |

## Verification Log

| Command | Result |
|---|---|
| `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/report_path.py docs/proposals/080-continuous-stale-execution-reconciliation.md` | Report path resolved to R7. |
| `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/discover_prior_review.py docs/proposals/080-continuous-stale-execution-reconciliation.md` | No prior proposal-review artifacts discovered. |
| `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/implementation_surface_fingerprint.py --json` | Required lenses included API contract, Apple UI/UX, architecture, observability/rollout, performance, reliability, and security; selected relevant Rust/API/security/rollout reviewers. |
| `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/security_sensitive_diff.py --json` | Security-sensitive diff triggered; manual pass performed. |
| `./scripts/test-gate.sh proposal-080` | Failed. Fixture/evidence inventory passed; build was killed with exit code 137 before Rust tests ran. |
| `rg -n "get_dedup_response\|insert_dedup_entry\|get_dedup_entry\|_dedup_key" control-plane/crates/mcp-server/src/tools/p080.rs` | Only `_dedup_key` binding found in MCP handler; no dedup repo calls found. |

## Final Verdict

Track 1 conformance is Partial. The implementation contains substantial P080 scaffolding and a
working-looking narrow Phase 2 `acp_startup_stale` manual repair slice, plus broad readback/API
surfaces. It does not yet satisfy the full P080 contract because durable operator-request dedup
replay is missing, continuous daemon repair is diagnostic-only, full stale-class behavior is not
implemented/proved, GraphQL subscription coverage is incomplete, and rollout evidence/runbooks are
not aligned.

Track 2 readiness is Not Ready. The canonical same-tree P080 gate failed before tests ran, and the
security-sensitive mutating repair path has a Major idempotency/replay-fence gap.

Required next actions before a higher verdict:

1. Wire and test the durable operator-request dedup replay/conflict fence for `repair_if_safe`.
2. Decide whether P080 readiness means continuous automatic repair or only manual Phase 2
   `acp_startup_stale` repair; then align code, proposal status, docs, runbook, and evidence.
3. Add gate coverage for GraphQL subscription behavior and Phase 2 dedup replay/conflict semantics.
4. Produce real rollout promotion evidence, not just fixture/unit-test inventory.
5. Make `./scripts/test-gate.sh proposal-080` pass on the same tree.
