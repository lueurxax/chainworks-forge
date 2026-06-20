# Proposal 080 Implementation Audit R1

## Metadata

| Field | Value |
| --- | --- |
| Proposal | `docs/proposals/080-continuous-stale-execution-reconciliation.md` |
| Proposal ID / revision | `P080` / `p080-refined-2026-06-02-r28` |
| Proposal status | `draft_refined_for_implementation_review` |
| Audit report | `docs/proposals/080-continuous-stale-execution-reconciliation_IMPLEMENTATION_AUDIT_R1.md` |
| Audit date | 2026-06-20 |
| Repo root | `/Users/user/Documents/Chainworks Forge` |
| Implementation target | Current worktree |
| Compare base | Implicit; no PR/range supplied |
| Audited HEAD | `97bea6d580ee9de8954071662ed528153f125afd` |
| Worktree status before report | Modified: `control-plane/crates/auth/src/lib.rs`, `control-plane/crates/mcp-server/src/server.rs`, `control-plane/crates/mcp-server/src/tools/reports.rs`, `control-plane/crates/mcp-server/src/tools/runs.rs`; unrelated untracked audit reports present |
| Overall conformance | **Not Implemented** |
| Overall readiness | **Not Ready** |
| Reviewer-selection reuse | **Not reused**; no prior P080 proposal-review artifacts found |
| Audit confidence | High for Phase 1/current-slice behavior and gate failure; medium for unimplemented Phase 2-5 internals because the active code intentionally blocks those paths |

## Implementation Target And Compare Base

The user supplied only a proposal path, so this audit evaluates the current worktree. No prior review artifacts were found by the helper or sibling-file scan. Existing `IMPLEMENTATION_AUDIT` reports were ignored for reviewer routing as required.

The worktree is not clean. The modified files are not primarily P080 files, but they are auth/MCP/reporting surfaces that overlap P080's security-sensitive public ingress. The audit did not modify implementation code, tests, docs, configs, or prior reports.

## Proposal State And Contract Summary

P080 is a Rust control-plane proposal for continuous stale execution reconciliation. The proposal says Chainworks should continuously reconcile non-terminal runs while the daemon is live, classify running truth from durable execution rows and live ownership witnesses, repair retryable non-side-effect stale work, keep release/publish/git/external side effects fail-closed unless P076 declares `retry_safe`, persist durable readback and idempotency state, and prove rollout safety through `proposal-080` / `p080` gates.

Platform and product scope:

- Apple: macOS read-only operator shell only. Phase 1 has no new SwiftUI diagnostic surface and no SwiftUI/AppKit repair wrapper.
- Backend/service: Rust daemon, SQLite persistence, MCP tools, read-only GraphQL projection/subscription, run-report and release-receipt readback, rollout controls, metrics, and operational evidence.
- Cross-stack API: MCP schema, GraphQL SDL, report/receipt JSON, metrics/dashboard/runbook artifacts.

Explicit exclusions:

- No automatic approval bypass.
- No blind retry of release, publish, git, or external side effects.
- No GraphQL repair mutation.
- No hard timeout for active prompts with useful progress.
- No arbitrary user process termination outside durable Chainworks lease/session metadata.
- No Workflow YAML or Agent Catalog YAML schema change.
- No SwiftUI/AppKit wrapper around MCP repair tools in Phase 1.
- `requested_action=hold` remains disabled and needs a later proposal.
- `acp_prompt_stale` is detected/delegated to P037, not repaired by P080.

## Primary Service Flows

1. Live daemon loop classifies running executions, writes P080 readback, and preserves active useful work.
2. Operator reads stale execution diagnostics through MCP `p080.diagnostics.get.v1`.
3. Swift/operator clients read the same projection through read-only GraphQL `p080Diagnostics` and `p080DiagnosticsUpdates`.
4. Operator requests `diagnose_only`, `repair_if_safe`, or permanent-hold clear through MCP, with auth, rollout, dedup, and predicate revalidation ordering.
5. Run reports and release receipts embed `p080_reconciliation` readback with safe redaction and projection integrity.

## Prior Proposal-Review Reuse

Reuse status: **Not reused**.

Discovery result:

```json
{"artifacts":[],"proposal_path":"/Users/user/Documents/Chainworks Forge/docs/proposals/080-continuous-stale-execution-reconciliation.md","repo_root":"/Users/user/Documents/Chainworks Forge"}
```

No `<proposal>.review/`, sibling review/evidence/research files, or clearly matching repo-local prior review artifacts were found.

## Selected Reviewers

| Reviewer | Why selected |
| --- | --- |
| `chainworks_execution_truth_reviewer` | Repo-local reviewer for run/stage/agent/recovery/projection truth and MCP truth. P080 changes durable execution truth. |
| `rust_reliability_reviewer` | Required for live loop, retry, idempotency, cooldown, crash/restart, cancellation, helper lifecycle, and stale execution repair. |
| `api_contract_reviewer` | Required for MCP request/response schemas, GraphQL SDL/subscription, cursor semantics, report/receipt JSON, and enum/versioning contracts. |
| `observability_rollout_reviewer` | Required for rollout-control rows, phase promotion, metrics, alerts, dashboards, migrations, rollback, and evidence fixtures. |
| `rust_security_reviewer` | Required because P080 touches auth, MCP/GraphQL ingress, parser limits, redaction, subprocess/process signaling, filesystem/report boundaries, and DoS budgets. |

Rejected close alternatives:

- `rust_arch_reviewer`: displaced by the repo-local execution-truth reviewer under the five-reviewer cap.
- `rust_performance_reviewer`: resource-limit and timeout concerns were covered by reliability/security/rollout; a dedicated performance pass would be required before any successful readiness verdict that claims budget performance.
- `macos_ui_reviewer` / `apple_ux_reviewer`: Phase 1 explicitly has no new SwiftUI diagnostic visual surface; the future diagnostics window is residual future UI scope.
- `product_reviewer`: metrics are operational rollout metrics, not product experiment or customer-value metrics.

## Proposal Fidelity And Divergence

Matches:

- `scripts/test-gate.sh` has `proposal-080|p080` aliases and a P080 test list.
- P080 domain enums and DTO-like Rust types exist in `control-plane/crates/domain/src/p080.rs`.
- Additive SQLite tables exist in `control-plane/crates/db/migrations/086_p080_stale_execution_reconciliation.sql`.
- `db::repos::p080` contains rollout-control seed/validation, classifier/readback rows, dedup helpers, event insert helpers, redaction, and run-report/release-receipt section builders.
- A live engine loop exists in `control-plane/crates/engine/src/executor.rs` and runs a 30-second P080 tick.
- MCP `p080.diagnostics.get.v1`, `p080.reconcile.request.v1`, and `p080.clear_permanent_hold.v1` are registered and routed through `tools::p080`.
- Read-only GraphQL `p080Diagnostics` and `p080DiagnosticsUpdates` surfaces exist.
- Run reports and release receipts have P080 reconciliation section integration.
- No Swift P080 references were found in `Chainworks Forge/**/*.swift`, matching the Phase 1 no-Swift-wrapper constraint.

Divergences:

- The implementation and reference docs describe the current system as **Phase 1 detection/readback only**. The proposal's goals and rollout plan also require active repair phases, helper reaping, scheduler repair, P076-gated side-effect handling, permanent-hold clear, and phase promotion evidence.
- `repair_if_safe` is not implemented as a successful repair path. The MCP handler always returns `class_disabled` or `rollout_disabled`.
- The live loop writes `diagnosed` / `diagnose_only` events/readback for `acp_startup_stale` candidates but does not perform ACP reset, scheduler capacity repair, helper reap, or permanent-hold clear.
- The classifier currently covers only useful/warmup/acp-startup/scheduler-ownership basics; helper orphan drift, release side-effect drift, and P037/P076 ownership witnesses are not active classifier inputs.
- The required rollout-contract fixtures under `docs/evidence/rollout-contract/negative/p080-*` and operator readback fixture are placeholders.
- `docs/evidence/rollout/p080/` contains only a README, not phase soak/readiness artifacts.
- The canonical P080 gate failed before running the P080 tests because the current migration directory has duplicate SQLx migration version `079`.

Ambiguities / evidence gaps:

- The proposal status says no unresolved/deferred/disputed items remain for the score-lift pass, while current reference docs intentionally narrow the implementation to Phase 1. No concrete follow-up proposal was found for the unimplemented active-repair phases.
- The proposal text has an internal migration-count inconsistency: the rollout contract says `p080_001` through `p080_009`, while migration rollout evidence mentions `p080_001` through `p080_010`. The implementation has one SQL file with P080 tables plus rollout-control tables.
- The P080 gate contains a Phase 1 test list, not the full fixture matrix required by proposal lines 797-821.

## Residual Scope / Follow-Up Ownership

| Residual item | Follow-up owner | Blocks conformance/readiness? |
| --- | --- | --- |
| Active `repair_if_safe` ACP startup reset | No concrete follow-up proposal found | Yes |
| Scheduler ownership drift repair and capacity reclamation | No concrete follow-up proposal found | Yes |
| P076-gated release/publish/git/external side-effect reconciliation | P076 is dependency, but P080 active integration has no follow-up owner | Yes |
| Owned helper reaping with Darwin identity verification | No concrete follow-up proposal found | Yes |
| Permanent-hold five-repair cap and `clear_permanent_hold` active path | No concrete follow-up proposal found | Yes |
| Full phase promotion/rollback soak artifacts | No concrete follow-up proposal found | Yes |
| Non-placeholder rollout-contract fixtures | No concrete follow-up proposal found | Yes |
| Future Window > P080 Diagnostics UI | Explicit later UI slice proposal | No for Phase 1 |
| Enabled manual `requested_action=hold` | Explicit later proposal required by non-goal | No for P080 |

## Specialist Coverage Matrix

| Surface | Trigger | Required lens | Selected reviewer/pass | Coverage result |
| --- | --- | --- | --- | --- |
| Durable execution truth/projection truth | Live loop, readback, recurrence, events | Architecture / execution truth | `chainworks_execution_truth_reviewer` | Completed manually; blockers found |
| Reliability/lifecycle | Live loop, crash/restart, cooldown, idempotency, repair | Reliability | `rust_reliability_reviewer` | Completed manually; blockers found |
| MCP/GraphQL/report contracts | MCP schemas, GraphQL SDL, cursor, report/receipt JSON | API contract | `api_contract_reviewer` | Completed manually; blockers found |
| Migrations/flags/metrics/rollout | SQLite, rollout-control, phase gates, alerts | Observability/rollout | `observability_rollout_reviewer` | Completed manually; blockers found |
| Auth/parser/redaction/process signaling | Security hard gate triggers | Rust security | `rust_security_reviewer` | Completed manually; readiness still blocked |
| Performance/resource limits | Parser/canonicalization budgets, count budget, tick deadline | Performance | Covered by reliability/security for this non-success verdict | Dedicated pass required before any future Ready verdict |
| macOS UI/UX | Future UI notes only | UI/UX | Not selected | Not blocking Phase 1 because proposal explicitly says no Phase 1 Swift UI surface |

## Security-Sensitive Diff Scan Summary

The bundled helper triggered security review for:

- authN/authZ and principal handling
- public MCP/GraphQL ingress
- parser/deserialization boundary
- secrets/redaction/privacy
- filesystem/subprocess/process boundary
- DoS/resource limits
- dependency/crypto-related matches

Reviewed surfaces:

- `control-plane/crates/auth/src/lib.rs`
- `control-plane/crates/mcp-server/src/server.rs`
- `control-plane/crates/mcp-server/src/http.rs`
- `control-plane/crates/mcp-server/src/tools/p080.rs`
- `control-plane/crates/graphql-server/src/schema.rs`
- `control-plane/crates/graphql-server/src/types/p080.rs`
- `control-plane/crates/db/src/repos/p080.rs`
- `control-plane/crates/db/migrations/086_p080_stale_execution_reconciliation.sql`
- P080 runbook, alert, dashboard, and fixture files

Security reviewer verdict: **Not Ready**.

The active implementation is mostly fail-closed for repair, which avoids unsafe process signaling or side-effect retry in the current slice. However, P080 cannot receive a successful security sign-off because the canonical gate fails, the declared security fixtures are placeholders, and the promised process-control security implementation is not present as active helper-reap behavior.

## Requirement Summary

| ID | Requirement | Status |
| --- | --- | --- |
| REQ-001 | Canonical `proposal-080` / `p080` gates prove rollout contract | Missing |
| REQ-002 | Continuous live reconciliation loop for non-terminal runs | Partially Implemented |
| REQ-003 | Ownership registry joins durable execution rows and live ownership witnesses | Partially Implemented |
| REQ-004 | Classifier covers all approved stale classes | Partially Implemented |
| REQ-005 | Shared typed repair transition is the only mutation entry point | Missing |
| REQ-006 | Retryable non-side-effect stale work is repaired safely | Missing |
| REQ-007 | Release/publish/git/external side effects fail closed unless P076 says `retry_safe` | Partially Implemented |
| REQ-008 | Provider sessions and helpers are owned by durable session/lease records before cleanup | Partially Implemented |
| REQ-009 | `p080_readback_v1` is shared across MCP, GraphQL, run reports, and release receipts | Partially Implemented |
| REQ-010 | Auth, rollout, and dedup ordering prevents stale/unauthorized replay | Partially Implemented |
| REQ-011 | Parser/envelope limits and closed MCP schemas are enforced | Partially Implemented |
| REQ-012 | Diagnostic redaction and `repair_idempotency_key` semantics are enforced | Partially Implemented |
| REQ-013 | GraphQL read-only projection/subscription matches the SDL contract | Partially Implemented |
| REQ-014 | Additive migrations and rollout/downgrade evidence are proven | Partially Implemented |
| REQ-015 | Cooldown, recurrence, permanent-hold, and clear behavior persist across restarts | Partially Implemented |
| REQ-016 | Darwin process-control security for helper reaping is implemented | Missing |
| REQ-017 | Feature flags, live-disable, and phase promotion/rollback controls are executable | Partially Implemented |
| REQ-018 | Rollout metrics, alerts, dashboard, and success criteria are implemented | Partially Implemented |
| REQ-019 | Required negative fixtures and operator readback fixture are concrete evidence | Missing |
| REQ-020 | Explicit non-goals are preserved | Partially Implemented |

## Detailed REQ Audit

### REQ-001: Canonical gates prove rollout contract

- Proposal source: Goals line 40; rollout contract gate aliases; Tests and Gates lines 797-821.
- Status: **Missing**
- Evidence: `scripts/test-gate.sh` defines `proposal-080|p080`, but `./scripts/test-gate.sh proposal-080` failed.
- Implementation mapping: `scripts/test-gate.sh` lines 319-363 and 7193-7224.
- Gap: The gate fails on the first DB test with `_sqlx_migrations.version` uniqueness failure because migration version `079` is duplicated by `079_p079_output_contract_repair.sql` and `079_p086_resurrection_state_and_idempotency.sql`. The gate also lists Phase 1 unit tests rather than the full fixture matrix from the proposal.

### REQ-002: Continuous live reconciliation loop for non-terminal runs

- Proposal source: Goals lines 33-35; Architecture lines 122-127; Rollout Plan lines 768-779.
- Status: **Partially Implemented**
- Evidence: `RuntimeAgentExecutor::run_p080_stale_execution_reconciliation_loop` runs a 30-second loop with a 20-second tick deadline; `p080_reconciliation_tick` consults `live_disable` and `detection_only`.
- Implementation mapping: `control-plane/crates/engine/src/executor.rs` lines 5485-5798.
- Gap: The loop diagnoses `acp_startup_stale` rows only. It does not execute active repair for scheduler ownership drift, helper orphan drift, release side-effect drift, or permanent-hold clear.

### REQ-003: Ownership registry joins durable execution rows and live ownership witnesses

- Proposal source: Architecture lines 122-129.
- Status: **Partially Implemented**
- Evidence: The classifier joins `agent_executions`, `stage_executions`, `session_generations`, and `work_items`.
- Implementation mapping: `control-plane/crates/db/src/repos/p080.rs` lines 176-320.
- Gap: No active join to ACP provider session state, helper leases, runtime invocation rows, P037 readback, or P076 side-effect ledger was found in the classifier.

### REQ-004: Classifier covers all approved stale classes

- Proposal source: Architecture lines 123-124; MCP enum lines 191-203.
- Status: **Partially Implemented**
- Evidence: Domain enums contain all classes; the live classifier writes useful, warmup, acp-startup-stale, and scheduler-ownership-drift classifications.
- Implementation mapping: `control-plane/crates/domain/src/p080.rs`; `control-plane/crates/db/src/repos/p080.rs` lines 236-252.
- Gap: `acp_prompt_stale`, `helper_orphan_drift`, `release_side_effect_drift`, `ambiguous_owner`, and process/side-effect witness paths are not actively classified by the live loop.

### REQ-005: Shared typed repair transition is the only mutation entry point

- Proposal source: Architecture line 125; Authorization lines 131-141.
- Status: **Missing**
- Evidence: DB helpers can atomically insert an event and upsert readback; the live loop uses this for diagnose-only events.
- Implementation mapping: `control-plane/crates/db/src/repos/p080.rs` atomic helper; `control-plane/crates/engine/src/executor.rs` lines 5731-5756.
- Gap: No active repair transition that mutates scheduler/session/helper leases under predicate revalidation was found. MCP `repair_if_safe` does not invoke a repair transition.

### REQ-006: Retryable non-side-effect stale work is repaired safely

- Proposal source: Goals lines 35-36; Rollout Plan lines 773-777; Success Criteria lines 787-790.
- Status: **Missing**
- Evidence: `repair_if_safe` always returns disabled/phase-gated responses; live loop logs explicitly say no ACP reset or scheduler repair occurs.
- Implementation mapping: `control-plane/crates/mcp-server/src/tools/p080.rs` lines 926-969; `control-plane/crates/engine/src/executor.rs` lines 5495-5498 and 5706-5709.
- Gap: No successful ACP reset, scheduler lease reclaim, or helper reap behavior is implemented.

### REQ-007: Side effects fail closed unless P076 says `retry_safe`

- Proposal source: Goals line 36; Non-goals line 44; Rollout Plan lines 775-776; Success Criteria line 787.
- Status: **Partially Implemented**
- Evidence: Current active code performs no side-effect repair, so it is fail-closed by absence.
- Implementation mapping: Reference docs explicitly state current slice does not perform repair.
- Gap: P080 does not actively consult P076 ledger state for `release_side_effect_drift` classification/repair, so the promised delegated retry-safe path is missing.

### REQ-008: Durable session/lease ownership before cleanup

- Proposal source: Goals line 37; Architecture line 129; Migration lines 508-517.
- Status: **Partially Implemented**
- Evidence: Helper lease and member tables exist with positive PID/start-time constraints.
- Implementation mapping: `control-plane/crates/db/migrations/086_p080_stale_execution_reconciliation.sql` lines 8-90.
- Gap: No active helper lease producer, lease membership verifier, or cleanup/reap transition was found.

### REQ-009: Shared `p080_readback_v1` across MCP/GraphQL/reports/receipts

- Proposal source: Goals line 38; MCP object lines 205-226; GraphQL contract lines 323-429; artifact contracts lines 521-563.
- Status: **Partially Implemented**
- Evidence: MCP diagnostics returns readback items; GraphQL query/subscription types exist; run-report and release-receipt builders add P080 sections.
- Implementation mapping: `tools/p080.rs`, `graphql-server/src/schema.rs`, `db/src/repos/p080.rs` lines 891-1155, `engine/src/release/receipt.rs` lines 17-21.
- Gap: The canonical gate failed, operator readback fixture is a placeholder, and the implementation only proves/serves the Phase 1 detection-readback slice.

### REQ-010: Auth, rollout, and dedup ordering prevents stale replay

- Proposal source: Authorization lines 131-155; Feature Flags lines 588-612.
- Status: **Partially Implemented**
- Evidence: MCP handlers validate schema/action/auth before rollout/dedup for active paths; dedup table/repository helpers exist; recent auth changes add a live principal source.
- Implementation mapping: `tools/p080.rs` lines 156-224 and 616-969; `db/src/repos/p080.rs` lines 1157-1320; modified `auth/src/lib.rs` and `mcp-server/src/server.rs`.
- Gap: Mutating `repair_if_safe` and `clear_permanent_hold` paths are disabled, so strict replay across successful repair/clear mutations is not proven by the gate.

### REQ-011: Parser/envelope limits and closed MCP schemas

- Proposal source: Parser lines 170-186; MCP schema lines 188-321.
- Status: **Partially Implemented**
- Evidence: `tools/p080.rs` rejects unknown fields and enforces nested schema and size/depth/array limits; HTTP preflight has duplicate-key scanning before JSON-RPC parse.
- Implementation mapping: `tools/p080.rs` lines 263-324 and 364-386; `mcp-server/src/http.rs`.
- Gap: The declared parser/resource-limit fixtures are placeholders and the P080 gate fails before executing its unit tests.

### REQ-012: Diagnostic redaction and `repair_idempotency_key`

- Proposal source: Diagnostic Redaction lines 157-168; Secret-Like Redaction Matrix lines 669-681.
- Status: **Partially Implemented**
- Evidence: Redaction helpers validate allowed keys, reject secret-like strings, and validate `p080-rik-<24hex>` values.
- Implementation mapping: `db/src/repos/p080.rs` lines 1377-1486 and 1739-2168.
- Gap: No active repaired event path derives the HMAC-backed `repair_idempotency_key`; redaction fixtures remain placeholders.

### REQ-013: GraphQL read-only projection/subscription matches SDL

- Proposal source: GraphQL lines 323-429; Versioning lines 431-453.
- Status: **Partially Implemented**
- Evidence: `p080Diagnostics` and `p080DiagnosticsUpdates` exist, with run-scope auth, live rollout gate checks, cursor encoding/decoding, subscription polling, and `AUTHORIZATION_LOST` handling.
- Implementation mapping: `graphql-server/src/schema.rs` lines 3368-3498 and 6685-7041.
- Gap: The P080 gate fails before GraphQL tests; SDL fixture is a placeholder; no successful same-tree schema introspection proof was produced.

### REQ-014: Additive migrations and migration rollout evidence

- Proposal source: Migration Contracts lines 503-519; Migration Rollout Evidence lines 716-727.
- Status: **Partially Implemented**
- Evidence: P080 migration file creates the requested P080 tables, constraints, indexes, and rollout-control/audit tables.
- Implementation mapping: `086_p080_stale_execution_reconciliation.sql`.
- Gap: Same-tree migration preflight fails because an unrelated duplicate `079` migration version exists. Migration rollout fixture is a placeholder; previous-binary downgrade, projection rebuild after disablement, and failed-migration forward-fix scenarios were not proven.

### REQ-015: Cooldown, recurrence, permanent-hold, and clear behavior

- Proposal source: Timeouts lines 468-498; Permanent Hold lines 565-566; Rollout Plan line 779.
- Status: **Partially Implemented**
- Evidence: Recurrence, events, cooldown columns, and permanent-hold/clear enum values exist; `clear_permanent_hold` tool exists.
- Implementation mapping: migration tables and `tools/p080.rs` clear handler.
- Gap: Permanent hold cap, five-repair threshold, clear active path, recurrence advancement for repairs, and cooldown persistence through real repair cycles are not active.

### REQ-016: Darwin process-control security for helper reaping

- Proposal source: Process-Control Security lines 568-586.
- Status: **Missing**
- Evidence: Helper lease tables exist.
- Implementation mapping: `086_p080_stale_execution_reconciliation.sql` lines 8-90.
- Gap: No active libproc/sysctl identity verification, signal phase handling, SIGTERM/SIGKILL verification, PID reuse rejection, or helper reap transition was found.

### REQ-017: Feature flags, live-disable, and phase promotion/rollback controls

- Proposal source: Feature Flags lines 588-612; Phase Promotion lines 614-667.
- Status: **Partially Implemented**
- Evidence: Rollout-control rows are seeded default-disabled, live-disable is checked in the live loop and MCP/GraphQL gates, and audit rows are written for seed/set helper functions.
- Implementation mapping: `db/src/repos/p080.rs` lines 30-149 and 693-824; `daemon/src/main.rs` lines 83-129 and 525-549.
- Gap: No authorized MCP rollout-control mutation is registered, and no phase soak/readiness artifacts exist beyond the README. Phase promotion is not executable evidence.

### REQ-018: Metrics, alerts, dashboard, and success criteria

- Proposal source: Metrics lines 683-714; Success Criteria lines 783-795.
- Status: **Partially Implemented**
- Evidence: `P080_REQUIRED_METRICS` lists the requested metrics, alert/dashboard files exist, and selected active paths emit some counters.
- Implementation mapping: `db/src/metrics.rs` lines 211-238; `docs/evidence/alerts/p080-alerts.yaml`; `docs/evidence/dashboards/p080-overview.json`.
- Gap: Many metrics are only listed, not emitted from active code paths because repair, helper reap, permanent hold, and migration validation paths are not active.

### REQ-019: Negative fixtures and operator readback fixture are concrete evidence

- Proposal source: Tests and Gates lines 797-821; rollout_contract_v1 negative fixture map.
- Status: **Missing**
- Evidence: `rg placeholder_fixture_kind docs/evidence/rollout-contract/negative/p080-*` finds placeholders across the declared fixture set; operator readback fixture also declares itself a placeholder.
- Implementation mapping: `docs/evidence/rollout-contract/negative/p080-*`; `docs/evidence/rollout-contract/operator-readback/p080-full-surface.fixture.json`.
- Gap: Placeholder fixtures are not executable proof of the contracts they claim to validate.

### REQ-020: Explicit non-goals are preserved

- Proposal source: Non-goals lines 42-51; UX/UI lines 53-56.
- Status: **Partially Implemented**
- Evidence: No Swift P080 references were found; GraphQL is read-only; `hold` and `clear_permanent_hold` return disabled/phase-gated errors; active code does not retry release/git side effects.
- Implementation mapping: Swift search returned no matches; `tools/p080.rs` disables hold/clear/repair; GraphQL exposes query/subscription only.
- Gap: `acp_prompt_stale` delegation is documented, but active classifier behavior for that class was not found. Non-goal preservation is mostly due to fail-closed disabled behavior rather than a complete delegated implementation.

## Reviewer / Lens Scorecard

| Lens | Conformance | Readiness | Top risk | Confidence |
| --- | --- | --- | --- | --- |
| Execution truth | Partial | Not Ready | Current implementation is Phase 1 diagnose-only while proposal requires active repair phases. | High |
| Rust reliability | Not Implemented | Not Ready | Repair, cooldown, permanent-hold, helper-reap, and P076/P037 integration are missing as active behavior. | High |
| API contract | Partial | Not Ready | MCP/GraphQL/report surfaces exist but are not proven by the required gate/fixtures. | Medium |
| Observability/rollout | Partial | Not Ready | Gate fails; fixture and soak evidence are placeholders; metrics are incomplete. | High |
| Security | Partial | Not Ready | Active paths fail closed, but security-sensitive fixtures and process-control implementation are absent. | High |

## Routed Specialist Findings

### READY-001: Canonical P080 gate fails before P080 tests run

- Reviewer: `observability_rollout_reviewer`
- Severity: Critical
- Confidence: High
- Related requirements: REQ-001, REQ-014, REQ-019
- Evidence types: tests-run, config, migration
- Evidence references: `./scripts/test-gate.sh proposal-080`; `control-plane/crates/db/migrations/079_p079_output_contract_repair.sql`; `control-plane/crates/db/migrations/079_p086_resurrection_state_and_idempotency.sql`
- Why it matters: P080 cannot be accepted without the proposal gate passing on the same tree. The gate failed on migration preflight with `_sqlx_migrations.version` uniqueness failure before the P080 DB tests ran.
- Recommended action: Resolve duplicate migration version `079`, then rerun `./scripts/test-gate.sh proposal-080`.
- Acceptance criteria: P080 gate reaches and passes every listed DB/MCP/GraphQL test on the audited tree.

### ARCH-001: Current implementation is a Phase 1 slice, not full P080

- Reviewer: `chainworks_execution_truth_reviewer`
- Severity: Major
- Confidence: High
- Related requirements: REQ-002, REQ-005, REQ-006, REQ-007, REQ-016
- Evidence types: code, docs
- Evidence references: `control-plane/crates/engine/src/executor.rs` lines 5485-5798; `docs/reference/current-system-baseline.md` P080 bullet; `docs/reference/rust-control-plane.md` P080 section
- Why it matters: The proposal goals require active stale repair, side-effect delegation, helper ownership cleanup, and durable mutation semantics. Current docs and code explicitly say active repair remains disabled.
- Recommended action: Either complete the missing active repair phases under P080 or formally split the current Phase 1 detection slice into a narrower proposal with concrete follow-up ownership.
- Acceptance criteria: The audited proposal no longer has unowned active-repair scope, or active repair paths exist and pass proposal gates.

### REL-001: `repair_if_safe` never performs safe repair

- Reviewer: `rust_reliability_reviewer`
- Severity: Major
- Confidence: High
- Related requirements: REQ-005, REQ-006, REQ-010, REQ-015
- Evidence types: code
- Evidence references: `control-plane/crates/mcp-server/src/tools/p080.rs` lines 926-969
- Why it matters: Operator-triggered repair is the service path that should reclaim stale scheduler capacity when predicates are safe. The handler always returns disabled/rollout errors.
- Recommended action: Implement predicate-revalidated typed repair transitions for approved repair classes and preserve current fail-closed behavior for disabled classes.
- Acceptance criteria: `repair_if_safe` can repair an eligible stale tuple in a focused integration test, writes the required event/readback/dedup atomically, and still rejects unsafe/disabled cases.

### REL-002: Live loop diagnoses only `acp_startup_stale`

- Reviewer: `rust_reliability_reviewer`
- Severity: Major
- Confidence: High
- Related requirements: REQ-002, REQ-003, REQ-004, REQ-007, REQ-016
- Evidence types: code
- Evidence references: `control-plane/crates/engine/src/executor.rs` lines 5655-5783; `control-plane/crates/db/src/repos/p080.rs` lines 236-252
- Why it matters: Continuous reconciliation must classify and act on multiple stale classes while preserving active prompts and side-effect safety. The current loop is a narrow diagnose-only path.
- Recommended action: Add P037/P076/helper/runtime ownership witnesses and class-specific handling for the remaining stale classes.
- Acceptance criteria: Focused tests prove each P080 stale class reaches its required readback and repair/delegation/hold outcome.

### SEC-001: Process-control security is specified but not implemented

- Reviewer: `rust_security_reviewer`
- Severity: Major
- Confidence: High
- Related requirements: REQ-008, REQ-016
- Evidence types: code, migration
- Evidence references: `control-plane/crates/db/migrations/086_p080_stale_execution_reconciliation.sql` lines 8-90; no active libproc/sysctl helper-reap implementation found
- Why it matters: P080's helper cleanup promise is only safe if PID/start-time/parent-chain verification happens before every signal phase. The current implementation avoids unsafe signaling by not reaping at all, but that leaves the promised behavior missing.
- Recommended action: Implement helper lease acquisition, Darwin identity verification, signal sequencing, and fail-closed evidence before enabling helper reaping.
- Acceptance criteria: Tests cover PID reuse, partial enumeration, permission failure, parent-chain mismatch, SIGTERM grace, SIGKILL verify, and max reap duration.

### API-001: Required schema/readback fixtures are placeholders

- Reviewer: `api_contract_reviewer`
- Severity: Major
- Confidence: High
- Related requirements: REQ-009, REQ-011, REQ-013, REQ-019
- Evidence types: tests-found, config
- Evidence references: `docs/evidence/rollout-contract/negative/p080-*`; `docs/evidence/rollout-contract/operator-readback/p080-full-surface.fixture.json`
- Why it matters: The proposal relies on fixtures to prove closed schemas and cross-lane parity. Placeholder fixtures do not prevent drift or prove MCP/GraphQL/report compatibility.
- Recommended action: Replace every P080 placeholder fixture with concrete failing/positive evidence and wire the gate to validate them.
- Acceptance criteria: `rg placeholder docs/evidence/rollout-contract/*p080*` returns no placeholder evidence, and the P080 gate validates each declared fixture.

### OPS-001: Phase promotion evidence is absent

- Reviewer: `observability_rollout_reviewer`
- Severity: Major
- Confidence: High
- Related requirements: REQ-017, REQ-018, REQ-019
- Evidence types: docs, telemetry
- Evidence references: `docs/evidence/rollout/p080/README.md`; `docs/evidence/alerts/p080-alerts.yaml`; `docs/evidence/dashboards/p080-overview.json`
- Why it matters: The proposal gates active repair on soak reports, false-positive review, canary scope, event-volume criteria, and rollback triggers. The evidence directory contains only a README.
- Recommended action: Produce the per-phase readiness artifacts and make the gate check their schema and thresholds before enabling later classes.
- Acceptance criteria: Phase reports exist under `docs/evidence/rollout/p080/`, contain the required metrics windows and acknowledgements, and are validated by the P080 gate.

### OPS-002: Metrics are listed but not fully emitted

- Reviewer: `observability_rollout_reviewer`
- Severity: Major
- Confidence: Medium
- Related requirements: REQ-018
- Evidence types: code, telemetry
- Evidence references: `control-plane/crates/db/src/metrics.rs` lines 211-238; metric emission search across `control-plane/crates`
- Why it matters: Rollout thresholds depend on metrics like repair failures, race aborts, helper escalation, permanent holds, and migration validation. Several are only present in the registry because the active behavior that would emit them is missing.
- Recommended action: Emit the full metric vocabulary from active code paths or narrow the proposal/gate to a Phase 1 detection-only metric subset with follow-up ownership.
- Acceptance criteria: Every rollout-critical metric is emitted by a tested path or explicitly excluded by a concrete follow-up proposal.

## Readiness Checklist

| Check | Result |
| --- | --- |
| `git diff --check` | Passed |
| Canonical P080 gate | Failed on duplicate SQLx migration version `079` |
| Build inside P080 gate | `cargo build -p db -p mcp-server -p graphql-server` completed before test failure |
| Core live loop validation | Code inspected only; gate failed before tests |
| MCP diagnostics validation | Tests found in gate list, but not executed because gate stopped at first DB test |
| GraphQL readback validation | Tests found in gate list, but not executed because gate stopped early |
| Security-sensitive pass | Performed manually; readiness blocked by missing active process-control behavior and placeholder fixtures |
| UI/UX states | Out of Phase 1 scope; static scan found no Swift P080 usage |
| Accessibility/localization/privacy | Future UI out of scope; privacy/redaction backend path partially implemented but not fully fixture-proven |
| Full regression / canonical proposal gate on audited HEAD | Not passed |

## Verification Log

| Command | Result |
| --- | --- |
| `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/report_path.py .../080-continuous-stale-execution-reconciliation.md` | Report path resolved to this R1 file. |
| `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/discover_prior_review.py .../080-continuous-stale-execution-reconciliation.md` | Passed; returned no artifacts. |
| `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/security_sensitive_diff.py --root ... --json` | Triggered security review. |
| `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/implementation_surface_fingerprint.py --root ... --json` | Triggered API, reliability, rollout, architecture, performance, security, and UI/UX categories; UI/UX manually scoped to future slice. |
| `git diff --check` | Passed. |
| `./scripts/test-gate.sh proposal-080` | Failed. First DB test panicked during migration preflight: `_sqlx_migrations.version` unique constraint failure. |
| Duplicate migration scan | Found duplicate version `079`: `079_p079_output_contract_repair.sql` and `079_p086_resurrection_state_and_idempotency.sql`. |
| `cd control-plane && cargo test -p domain p080 -- --nocapture` | Passed trivially with 0 matching tests. |
| Swift P080 static scan | No matches in `Chainworks Forge/**/*.swift`. |
| Placeholder fixture scan | Found placeholders across P080 negative fixtures and operator readback fixture. |

## Final Verdict

Overall conformance: **Not Implemented**.

Overall implementation readiness: **Not Ready**.

P080 has a meaningful Phase 1 detection/readback slice in the Rust control plane, including rollout-control seeding, a live diagnose-only loop, MCP/GraphQL diagnostics, P080 tables, and report/receipt readback scaffolding. It does not satisfy the full proposal: active repair, scheduler reclamation, helper reaping, P076/P037 ownership integration, permanent-hold clear, phase promotion evidence, concrete fixtures, and the canonical gate are not complete.

Recommended next actions:

1. Resolve the duplicate migration version so the P080 gate can run.
2. Decide whether P080 is intended to close only Phase 1 detection/readback or the full Phase 0-5 proposal.
3. If Phase 1 only, split or supersede the proposal with explicit follow-up ownership for active repair phases.
4. If full P080, implement the missing repair/delegation/helper/permanent-hold paths and replace placeholder fixtures.
5. Rerun `./scripts/test-gate.sh proposal-080` from a clean tree before any closeout.
