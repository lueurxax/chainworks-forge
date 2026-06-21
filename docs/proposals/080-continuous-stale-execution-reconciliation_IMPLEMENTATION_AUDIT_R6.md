# P080 Implementation Audit R6 - Continuous Stale Execution Reconciliation

## Metadata

- Proposal: `docs/proposals/080-continuous-stale-execution-reconciliation.md`
- Proposal ID: `P080`
- Proposal revision: `p080-refined-2026-06-02-r28`
- Source review pass: `6a336c7e-f387-4840-a607-8adeb72e8fa4`
- Proposal status: `draft_refined_for_implementation_review`
- Audit date: 2026-06-20
- Audited HEAD: `0e6482c82b588b74a76294a225e68286bfe37fa4`
- Audit report: `docs/proposals/080-continuous-stale-execution-reconciliation_IMPLEMENTATION_AUDIT_R6.md`

## Implementation Target And Compare Base

The audit target is the current worktree implementation against the full P080 r28 proposal contract, not just the passing focused gate. The relevant audited files are currently dirty in the worktree:

- `control-plane/crates/db/src/repos/p080.rs`
- `control-plane/crates/engine/src/executor.rs`
- `control-plane/crates/mcp-server/src/tools/p080.rs`
- `scripts/test-gate.sh`
- P080 rollout evidence fixtures under `docs/evidence/rollout-contract/` and `docs/evidence/rollout/p080/`

Existing untracked prior P080 reports R2-R5 were not used as proposal-review input. This R6 report is a new audit artifact only.

## Prior Proposal-Review Reuse

- Prior proposal-review artifacts discovered: none.
- Reuse decision: not reused.
- Command result: `discover_prior_review.py` returned an empty `artifacts` list for this proposal.

## Selected Reviewers

The reviewer registry and security fingerprint required API contract, architecture, observability/rollout, reliability, and security lenses. The five-reviewer cap selected:

- `rust_arch_reviewer`
- `rust_reliability_reviewer`
- `rust_security_reviewer`
- `api_contract_reviewer`
- `observability_rollout_reviewer`

## Rejected Alternatives

- `rust_performance_reviewer`: resource limits and backpressure are present, but the cap was consumed by stronger blocking API/reliability/security surfaces. Performance remains a residual risk because this audit does not claim Ready.
- `macos_ui_reviewer` / `apple_ux_reviewer`: Phase 1 explicitly has no new SwiftUI diagnostic surface, and the implementation scan found no P080 Swift UI or repair wrapper. The future UI fixture remains out of scope for this implementation.
- `product_reviewer`: no independent product workflow was added beyond operational rollout metrics and readback.

## Proposal State And Contract

P080 r28 requires continuous live stale execution reconciliation with durable ownership truth, safe repair of retryable non-side-effect work, fail-closed behavior around side effects, shared `p080_readback_v1` readback across MCP/GraphQL/reports/receipts, persistent cooldown/recurrence/idempotency/dedup fences, strict MCP/GraphQL schema contracts, and rollout proof through the `proposal-080`/`p080` gate.

Key contract points used for this audit:

- Goals require continuous non-terminal reconciliation, safe repair, durable helper/session ownership, shared readback, and restart-stable replay fences.
- Non-goals prohibit GraphQL repair mutation, arbitrary process termination, Workflow/Agent YAML changes, SwiftUI/AppKit repair wrappers, enabled `hold`, and P080-owned `acp_prompt_stale` repair.
- Phase 1 is detection-only; `repair_if_safe`, `hold`, and `clear_permanent_hold` are disabled.
- Phase 2 enables `repair_if_safe` for `acp_startup_stale` when class flag and safety predicates pass.
- Phase 3 enables scheduler ownership drift repair only when P076 side-effect truth is `retry_safe`.
- Phase 4 enables owned helper reaping only with durable lease metadata and process identity evidence.
- Phase 5 enables auto-retry ledger and permanent-hold clear.
- The MCP schema appendix requires closed request, response, and error envelopes with `additionalProperties: false` and closed enum vocabularies.
- `repair_idempotency_key` must be HMAC-SHA-256 over `(recurrence_epoch, run_id, stage_id, work_item_id, stale_class, repair_action)`, keyed by a daemon-scoped key, truncated to 96 bits, and prefixed `p080-rik-`.

## Platform And Product Scope

- Rust control-plane daemon: in scope.
- SQLite persistence and migrations: in scope.
- MCP tools: in scope.
- Read-only GraphQL query/subscription: in scope.
- Run-report and release-receipt sections: in scope.
- macOS SwiftUI diagnostics UI: out of scope for Phase 1, and no P080 Swift UI implementation was found.
- GraphQL mutation and SwiftUI repair wrapper: prohibited by the proposal and not found.

## Primary Flows Audited

1. Daemon startup seeds and validates the P080 rollout-control matrix.
2. Background executor runs the P080 stale-execution reconciliation loop.
3. Classifier writes `p080_readback_heartbeats_v1` for running executions.
4. MCP diagnostics and reconcile handlers parse, authorize, gate, read, and sometimes mutate.
5. GraphQL exposes read-only `p080Diagnostics` and `p080DiagnosticsUpdates`.
6. Run reports and release receipts embed P080 reconciliation sections.
7. Proposal gate checks migration presence, evidence inventory, builds DB/MCP/GraphQL crates, and runs focused unit tests.

## Fidelity And Divergence Summary

The implementation has meaningful P080 substrate: additive tables, rollout control seeding, classifier/readback rows, live loop scheduling, MCP and GraphQL readback surfaces, run-report/release-receipt integration, duplicate-key scanning, redaction boundaries, and a focused passing gate.

It does not yet satisfy the full r28 proposal contract. The implementation evidence is phase-scoped: `docs/evidence/rollout/p080/phase-scoped-same-tree-proof.json` lists `detection_only` and `acp_startup_stale` as implemented while `scheduler_ownership_drift`, `helper_orphan_drift`, `release_side_effect_drift`, and `permanent_hold_clear` remain disabled. Without a proposal revision or follow-up proposal owning that tail, those disabled classes are residual P080 scope, not closeout-ready implementation.

## Residual Scope And Follow-Up Ownership

Residual scope that must be implemented or explicitly moved to follow-up proposals before closeout:

- Publish full MCP input, output, and error schemas in the tool inventory for all P080 MCP tools.
- Align `repair_if_safe` phase gating with the proposal's Phase 2 ACP startup stale repair contract.
- Implement or explicitly retire the Phase 3 scheduler ownership drift repair path and P076 `retry_safe` integration.
- Implement or explicitly retire the Phase 4 owned helper reaping path with process identity checks.
- Implement or explicitly retire Phase 5 permanent-hold clear and auto-retry ledger behavior.
- Replace the current unkeyed repair key derivation with the daemon-keyed HMAC tuple required by the proposal.
- Expand gate evidence from phase-scoped unit coverage to full schema, SDL, versioning, timeout/crash, process-control, and phase-promotion fixture coverage.

## Specialist Coverage Matrix

| Lens | Reviewer | Coverage | Result |
|---|---|---|---|
| Architecture | `rust_arch_reviewer` | Daemon loop, DB migration shape, rollout-control ownership, shared readback lanes | Partial |
| Reliability | `rust_reliability_reviewer` | Live loop, phase gates, repair paths, crash/restart durability, idempotency | Not ready |
| Security | `rust_security_reviewer` | Auth order, parser boundary, redaction, repair key derivation, process boundary | Not ready |
| API contract | `api_contract_reviewer` | MCP schemas, GraphQL read-only shape, response envelopes, gate scope | Not ready |
| Observability/rollout | `observability_rollout_reviewer` | Metric vocabulary, dashboards, alerts, rollout evidence, gate | Partial |

## Requirement Summary

| Requirement | Status | Notes |
|---|---|---|
| REQ-001 continuous live daemon reconciliation | Partial | Loop exists, but only diagnoses selected `acp_startup_stale` rows. |
| REQ-002 durable classification rows and ownership truth | Partial | Tables and classifier exist; class coverage is incomplete. |
| REQ-003 safe retryable repair | Missing | Full Phase 2/3/4 repair contract is not implemented. |
| REQ-004 side-effect fail-closed and P076 delegation | Partial | Fail-closed posture exists; Phase 3 P076 retry-safe repair path is absent. |
| REQ-005 durable provider/helper ownership before cleanup | Partial | Helper lease tables exist; helper reaping is disabled/not implemented. |
| REQ-006 shared readback across lanes | Partial | Lanes exist, but MCP schema inventory and phase coverage are incomplete. |
| REQ-007 auth/idempotency/dedup replay fences | Partial | Strong ordering exists, but repair key derivation is non-conformant. |
| REQ-008 MCP schema appendix | Missing | Tool inventory does not publish full closed input/output/error schemas. |
| REQ-009 read-only GraphQL projection | Partial | Query/subscription exist; full SDL contract proof is not in the gate. |
| REQ-010 parser and envelope resource limits | Partial | Duplicate-key/raw scan and handler checks exist; gate coverage is selective. |
| REQ-011 diagnostic redaction | Partial | Allow-list and format checks exist; HMAC-derived repair key is wrong. |
| REQ-012 timeout/crash recovery hierarchy | Partial | Loop deadline exists; crash-cutpoint fixture coverage is not proven by the gate. |
| REQ-013 `acp_prompt_stale` delegation to P037 | Partial | Proposal-owned repair is absent as required; full delegated readback path not fully proven. |
| REQ-014 migrations | Implemented with caveat | Additive P080 tables and rollout control exist. Gate only checks a narrow subset of shape. |
| REQ-015 run-report/release-receipt sections | Implemented with caveat | Sections are wired; parity is limited by partial readback behavior. |
| REQ-016 permanent hold/cooldown | Missing/Partial | Clear tool is registered but disabled; Phase 5 behavior missing. |
| REQ-017 process-control security/helper reaping | Missing | No enabled helper reaping path was found. |
| REQ-018 feature flags and rollout controls | Partial | Durable rollout control exists; operator mutation/tooling is future. |
| REQ-019 phase promotion/rollback evidence | Partial | Phase-scoped proof exists, not full proposal coverage. |
| REQ-020 metrics/dashboards/alerts | Partial | Vocabulary and evidence files exist; emissions for missing phases cannot be complete. |
| REQ-021 canonical `proposal-080` gate | Partial | Gate passes, but its checks do not cover the full r28 contract. |
| REQ-022 prohibited surfaces absent | Implemented | No P080 GraphQL mutation or SwiftUI repair wrapper found. |

## Detailed Requirement Audit

### REQ-001 Continuous live daemon reconciliation - Partial

Evidence:

- The executor spawns `run_p080_stale_execution_reconciliation_loop()` at `control-plane/crates/engine/src/executor.rs:5408`.
- The loop uses a 30 second interval and 20 second tick deadline at `control-plane/crates/engine/src/executor.rs:5491`.
- The tick checks `live_disable`, `detection_only`, runs `classify_and_upsert_running_executions`, and emits metrics at `control-plane/crates/engine/src/executor.rs:5550`.

Divergence:

- The loop explicitly says Phase 0/1/2 are `diagnose_only` and perform no actual ACP reset or scheduler capacity reclamation at `control-plane/crates/engine/src/executor.rs:5495` and `control-plane/crates/engine/src/executor.rs:5540`.
- The live loop fetches only `stale_class: Some("acp_startup_stale")` at `control-plane/crates/engine/src/executor.rs:5655`.

### REQ-002 Durable classification rows - Partial

Evidence:

- Migration creates P080 event, epoch, dedup, deferral, cursor, heartbeat, watchdog, helper lease, and rollout-control tables in `control-plane/crates/db/migrations/086_p080_stale_execution_reconciliation.sql:8`.
- Classifier writes `p080_readback_heartbeats_v1` rows and tracks warmup, ACP startup stale, scheduler drift, and useful classifications in `control-plane/crates/db/src/repos/p080.rs:169`.

Divergence:

- Classifier coverage is broader than live-loop repair coverage. `scheduler_ownership_drift` can be classified, but the live reconciliation loop does not repair or diagnose it as an acted candidate.

### REQ-003 Safe repair of retryable non-side-effect work - Missing

Evidence:

- Proposal Phase 2 requires `repair_if_safe` for `acp_startup_stale`.
- Proposal Phase 3 requires scheduler ownership drift repair when P076 says `retry_safe`.
- Proposal Phase 4 requires owned helper reaping.

Divergence:

- MCP `repair_if_safe` only supports `acp_startup_stale`; all other classes return fail-closed at `control-plane/crates/mcp-server/src/tools/p080.rs:979`.
- The handler requires `phase_3`, `phase_4`, or `phase_5` before accepting ACP startup stale repair at `control-plane/crates/mcp-server/src/tools/p080.rs:1045`, while the proposal says ACP startup stale repair is Phase 2.
- The live daemon loop remains diagnosis-only for Phase 0/1/2 at `control-plane/crates/engine/src/executor.rs:5706`.
- No scheduler drift repair or helper reaping success path was found.

### REQ-004 Side-effect fail-closed / P076 delegation - Partial

Evidence:

- Rollout classes include `release_side_effect_drift` and the proposal evidence marks side-effect-adjacent repair not enabled without P076 retry-safe truth.

Divergence:

- No implemented Phase 3 path was found that consumes P076 retry-safe truth and performs scheduler ownership drift repair for retryable non-side-effect work.

### REQ-005 Durable provider/helper ownership before cleanup - Partial

Evidence:

- Helper lease tables and member tables exist at `control-plane/crates/db/migrations/086_p080_stale_execution_reconciliation.sql:8` and `:61`.
- Rollout classes include `helper_orphan_drift` at `control-plane/crates/db/src/repos/p080.rs:21`.

Divergence:

- The phase-scoped proof explicitly lists `helper_orphan_drift` disabled.
- No enabled helper reaping path, signal progression, or process identity verification flow was found.

### REQ-006 Shared readback object across lanes - Partial

Evidence:

- MCP readback redaction validates `p080_readback_v1` and closed enums at `control-plane/crates/mcp-server/src/tools/p080.rs:1428`.
- GraphQL query returns a diagnostics connection at `control-plane/crates/graphql-server/src/schema.rs:3406`.
- GraphQL subscription exists at `control-plane/crates/graphql-server/src/schema.rs:6822`.
- Run report embeds `p080_reconciliation` at `control-plane/crates/mcp-server/src/tools/reports.rs:276`.
- Release receipt passes a P080 reconciliation section at `control-plane/crates/engine/src/executor.rs:16916`.

Divergence:

- The shared object is not fully proven through the public MCP schema inventory because MCP tool output schemas are absent.
- Readback content is limited by missing repair classes.

### REQ-007 Authorization, idempotency, and replay fences - Partial

Evidence:

- Diagnostics handler enforces resource limits, schema version, closed fields, run-scope auth, then rollout gates at `control-plane/crates/mcp-server/src/tools/p080.rs:326`.
- Reconcile handler validates action, read-only class restrictions, closed fields, run-scope auth, then rollout gates at `control-plane/crates/mcp-server/src/tools/p080.rs:616`.
- GraphQL checks run-scope authorization before rollout gates for query and subscription at `control-plane/crates/graphql-server/src/schema.rs:3419` and `:6834`.
- Dedup table stores principal class, policy generation, secret generation, rollout phase, repair class hash, live-disable generation, fingerprint, and response at `control-plane/crates/db/migrations/086_p080_stale_execution_reconciliation.sql:176`.

Divergence:

- The repair idempotency key derivation is not proposal-conformant; see finding SEC-001.
- Full replay coverage for mutating repair phases cannot be complete while the phases are disabled or missing.

### REQ-008 MCP schema appendix - Missing

Evidence:

- Proposal requires all P080 MCP request, response, and error envelopes to have JSON-schema-like contracts, `additionalProperties: false` everywhere, and closed enum vocabularies.

Divergence:

- `p080.diagnostics.get.v1` input schema does not require `schema_version`, and `stale_class`/`hold_reason` are generic strings rather than closed enums at `control-plane/crates/mcp-server/src/tools/p080.rs:28`.
- All three P080 tools publish `output_schema: None` at `control-plane/crates/mcp-server/src/tools/p080.rs:64`, `:113`, and `:151`.
- Reconcile and clear-permanent-hold target `stale_class` fields are generic strings at `control-plane/crates/mcp-server/src/tools/p080.rs:90` and `:138`.

Runtime validation catches some invalid calls, but the public tool inventory does not satisfy the schema appendix.

### REQ-009 GraphQL read-only projection - Partial

Evidence:

- `p080_diagnostics` query exists and is read-only at `control-plane/crates/graphql-server/src/schema.rs:3406`.
- `p080_diagnostics_updates` subscription exists at `control-plane/crates/graphql-server/src/schema.rs:6822`.
- Search found no P080 GraphQL mutation; mutation enum entries are P031/P083/retry-related, not P080.

Divergence:

- The canonical P080 gate runs one GraphQL authorization test, not a full SDL/introspection golden contract for the P080 query/subscription.

### REQ-010 Parser and envelope limits - Partial

Evidence:

- MCP HTTP raw duplicate-key scanner runs before auth and before typed extraction at `control-plane/crates/mcp-server/src/http.rs:62`.
- The scanner is bounded by `SCAN_KEY_BUDGET` at `control-plane/crates/mcp-server/src/http.rs:244`.
- P080 handlers call `check_p080_resource_limits` before schema or auth checks at `control-plane/crates/mcp-server/src/tools/p080.rs:331` and `:621`.

Divergence:

- The gate proves a useful subset of parser behavior but does not prove every parser/envelope limit fixture named in the proposal.

### REQ-011 Diagnostic redaction - Partial

Evidence:

- MCP egress rejects missing/wrong `schema_version`, non-scalar values, invalid `repair_idempotency_key` format, and out-of-vocabulary enums at `control-plane/crates/mcp-server/src/tools/p080.rs:1428`.
- DB write boundary validates readback JSON, allowed keys, secret-like values, and closed enums in `control-plane/crates/db/src/repos/p080.rs:1424`.

Divergence:

- The implementation validates `p080-rik-<24 hex>` format but does not derive the key using the required daemon-keyed HMAC tuple.

### REQ-012 Timeouts and crash recovery - Partial

Evidence:

- Live loop uses a 20 second tick deadline at `control-plane/crates/engine/src/executor.rs:5491`.
- Candidate deferral and iteration cursor tables exist at `control-plane/crates/db/migrations/086_p080_stale_execution_reconciliation.sql:206` and `:229`.

Divergence:

- The focused gate does not prove the full timeout/backpressure hierarchy or crash cutpoint matrix named in the proposal.

### REQ-013 `acp_prompt_stale` delegation - Partial

Evidence:

- Rollout classes exclude `acp_prompt_stale` with a comment that P080 delegates it to P037 at `control-plane/crates/db/src/repos/p080.rs:13`.

Divergence:

- The explicit delegated readback behavior for `acp_prompt_stale` was not fully proven through a runtime path in the focused gate.

### REQ-014 Migration contract - Implemented With Caveat

Evidence:

- Migration `086_p080_stale_execution_reconciliation.sql` creates the expected P080 additive tables and constraints.
- Daemon seeds and validates rollout control fail-closed at `control-plane/crates/daemon/src/main.rs:525`.
- Gate checks migration presence and a principal-class constraint subset at `scripts/test-gate.sh:7205`.

Caveat:

- The gate does not validate every named table/constraint/index shape from the proposal.

### REQ-015 Run report and release receipt - Implemented With Caveat

Evidence:

- DB builds `p080_run_report_section_v1` at `control-plane/crates/db/src/repos/p080.rs:892`.
- DB builds `p080_release_receipt_section_v1` at `control-plane/crates/db/src/repos/p080.rs:1050`.
- MCP run report includes `p080_reconciliation` at `control-plane/crates/mcp-server/src/tools/reports.rs:276`.
- Engine release receipt includes a P080 section at `control-plane/crates/engine/src/executor.rs:16916`.

Caveat:

- Report/receipt lane content reflects partial runtime behavior while repair phases remain missing.

### REQ-016 Permanent hold and cooldown - Missing/Partial

Evidence:

- Clear-permanent-hold tool is registered and described as Phase 5 only at `control-plane/crates/mcp-server/src/tools/p080.rs:115`.
- `permanent_hold_clear` rollout class exists.

Divergence:

- The tool remains disabled by phase and Phase 5 behavior was not implemented.

### REQ-017 Process-control security - Missing

Evidence:

- Helper lease tables constrain positive PIDs in migration.
- The phase-scoped proof says helper process signal is not enabled.

Divergence:

- No enabled process-control flow was found for owned helper reaping, SIGTERM/SIGKILL verification, or max-reap duration.

### REQ-018 Feature flags and rollout controls - Partial

Evidence:

- Rollout control table and audit table exist at `control-plane/crates/db/migrations/086_p080_stale_execution_reconciliation.sql:296`.
- Daemon seeds and validates the matrix at startup.

Divergence:

- The operator-facing rollout-control mutation/tool remains future per runbook references.

### REQ-019 Phase promotion and rollback - Partial

Evidence:

- Phase-scoped evidence exists in `docs/evidence/rollout/p080/phase-scoped-same-tree-proof.json`.
- Evidence says implemented classes are `detection_only` and `acp_startup_stale`, and disabled classes are scheduler/helper/side-effect/permanent-hold.

Divergence:

- The proof is phase-scoped and does not cover full P080 phase promotion criteria through Phase 5.

### REQ-020 Metrics, dashboards, and alerts - Partial

Evidence:

- Required P080 metric vocabulary is listed at `control-plane/crates/db/src/metrics.rs:236`.
- Dashboard and alerts exist in `docs/evidence/dashboards/p080-overview.json` and `docs/evidence/alerts/p080-alerts.yaml`.
- Live loop increments detection and readback projection counters.

Divergence:

- Metrics tied to missing repair/helper/permanent-hold phases cannot be fully emitted by this implementation.

### REQ-021 Canonical gate - Partial

Evidence:

- `./scripts/test-gate.sh proposal-080` passed in this audit run.
- Gate verifies fixture inventory, builds DB/MCP/GraphQL crates, and runs named Rust tests at `scripts/test-gate.sh:7201`.

Divergence:

- Gate test list is focused on DB/MCP/GraphQL unit tests and one GraphQL policy test at `scripts/test-gate.sh:322`.
- The proposal's gate list includes MCP schema contract, GraphQL SDL contract, version negotiation, timeout/backpressure, crash recovery, process-control security, phase promotion, migration rollout, and runbook matrix. The implemented gate does not prove that full list.

### REQ-022 Prohibited surfaces absent - Implemented

Evidence:

- Search found no P080 SwiftUI repair wrapper or P080 Swift app surface. The only Swift match for "reconciliation" was unrelated YAML validation text.
- Search found no P080 GraphQL mutation.

## Reviewer And Lens Scorecard

| Reviewer | Score | Rationale |
|---|---:|---|
| `rust_arch_reviewer` | 2/5 | Durable tables and lanes are present, but full phase architecture is incomplete. |
| `rust_reliability_reviewer` | 1/5 | Passing gate does not cover full repair lifecycle, crash cutpoints, or phase tail. |
| `rust_security_reviewer` | 2/5 | Strong auth/redaction work exists, but repair key derivation is non-conformant and helper process controls are missing. |
| `api_contract_reviewer` | 1/5 | Public MCP schema inventory is materially out of contract. |
| `observability_rollout_reviewer` | 2/5 | Vocabulary/evidence exists, but rollout proof is phase-scoped and metrics for missing phases cannot be complete. |

## Security Scan Summary

Security-sensitive diff helper result:

- Triggered: true.
- Categories: `auth`, `dos_resource_limits`, `filesystem_subprocess_boundary`, `parser_boundary`, `public_ingress`, `secrets_redaction_privacy`, `unsafe_crypto_dependency`.
- Required action from helper: independent security pass before Ready/Ready with Risks.

Manual security review findings:

- Positive: raw duplicate-key rejection runs before auth and typed extraction.
- Positive: MCP and GraphQL run-scope authorization checks precede rollout-state disclosure.
- Positive: DB and MCP readback boundaries enforce allow-listed keys, scalar-only values, and closed enum vocabularies.
- Blocking: `repair_idempotency_key` derivation uses an unkeyed SHA-256 predicate hash, not the proposal-required daemon-keyed HMAC tuple.
- Blocking/major API-security boundary: MCP tool inventory does not publish closed output/error schemas, so clients cannot rely on the declared public contract.

## Routed Specialist Findings

### API-001 - P080 MCP tool inventory does not publish the required closed schemas

- Severity: Blocker
- Confidence: High
- Reviewer: `api_contract_reviewer`
- Evidence:
  - Proposal MCP schema appendix requires closed request/response/error contracts.
  - `p080.diagnostics.get.v1` lacks required `schema_version` in the inventory schema and exposes `stale_class`/`hold_reason` as generic strings at `control-plane/crates/mcp-server/src/tools/p080.rs:28`.
  - All P080 MCP tools set `output_schema: None` at `control-plane/crates/mcp-server/src/tools/p080.rs:64`, `:113`, and `:151`.
- Impact: Clients introspecting MCP tools do not see the P080 public response/error contract. This violates the proposal even though runtime validation catches some bad calls.
- Required fix: Publish full input, output, and error schemas with `additionalProperties: false`, required `schema_version`, closed enum values, and tests that inspect `tools/list`.

### REL-001 - Full repair phase contract is not implemented

- Severity: Blocker
- Confidence: High
- Reviewer: `rust_reliability_reviewer`
- Evidence:
  - Live loop comments state Phase 0/1/2 are `diagnose_only`, with no ACP reset or scheduler reclamation at `control-plane/crates/engine/src/executor.rs:5495`.
  - Live loop only fetches `acp_startup_stale` candidates at `control-plane/crates/engine/src/executor.rs:5655`.
  - MCP repair path rejects every class except `acp_startup_stale` at `control-plane/crates/mcp-server/src/tools/p080.rs:979`.
  - Phase-scoped proof lists scheduler/helper/side-effect/permanent-hold classes disabled.
- Impact: The implementation cannot satisfy goals for retryable non-side-effect repair, scheduler ownership drift repair, helper cleanup, or permanent-hold clear.
- Required fix: Implement Phases 2-5 or revise the proposal so those phases are explicitly owned by later proposal IDs.

### REL-002 - ACP startup stale repair is gated to Phase 3+, not proposal Phase 2

- Severity: Major
- Confidence: High
- Reviewer: `rust_reliability_reviewer`
- Evidence:
  - Proposal rollout plan says Phase 2 enables `repair_if_safe` for `acp_startup_stale`.
  - Code requires rollout phase `phase_3`, `phase_4`, or `phase_5` at `control-plane/crates/mcp-server/src/tools/p080.rs:1045`.
- Impact: Even the implemented `acp_startup_stale` repair class is not phase-aligned with the proposal.
- Required fix: Align phase gating and tests with Phase 2, or revise the proposal and evidence to rename the phase boundary.

### SEC-001 - `repair_idempotency_key` derivation is not the required daemon-keyed HMAC

- Severity: Major
- Confidence: High
- Reviewer: `rust_security_reviewer`
- Evidence:
  - Proposal requires HMAC-SHA-256 over `(recurrence_epoch, run_id, stage_id, work_item_id, stale_class, repair_action)`, keyed by daemon-scoped secret and truncated to 96 bits.
  - Code computes `predicate_hash = lower_hex_sha256("{run_id}:{stage_id}:{work_item_id}:acp_startup_stale")`, then uses `p080-rik-{predicate_hash[..24]}` at `control-plane/crates/db/src/repos/p080.rs:1755`.
- Impact: Public repair keys are predictable from tuple values and omit recurrence epoch and repair action. This is not the promised replay/correlation boundary.
- Required fix: Derive `repair_idempotency_key` with daemon-scoped HMAC using the exact proposal tuple, include secret-generation rotation behavior, and add tests proving stability and non-predictability.

### OBS-001 - The P080 gate proves a useful subset, not the full proposal gate list

- Severity: Major
- Confidence: High
- Reviewer: `observability_rollout_reviewer`
- Evidence:
  - Gate runs migration presence checks, fixture inventory checks, builds, and a named DB/MCP/GraphQL test list at `scripts/test-gate.sh:7201`.
  - Test list includes focused tests but does not include full SDL, tool inventory output schema, version negotiation, crash cutpoint, process-control, phase-promotion, or runbook matrix proofs from the proposal's Tests and Gates section.
- Impact: A passing `proposal-080` gate cannot be used as closeout evidence for full r28 conformance.
- Required fix: Add explicit gate checks for every named P080 fixture category or split phase-scoped gates from full proposal closeout gates.

## Readiness Checklist

- [x] Proposal metadata extracted.
- [x] Prior proposal-review reuse checked; none found.
- [x] Reviewer registry selection applied.
- [x] Security-sensitive diff helper run and routed.
- [x] P080 implementation surfaces inspected in DB, daemon, engine, MCP, GraphQL, report, receipt, evidence, and gate.
- [x] Canonical `proposal-080` gate run.
- [ ] Full MCP schema appendix satisfied.
- [ ] Phase 2 `acp_startup_stale` repair aligned with proposal.
- [ ] Phase 3 scheduler ownership drift repair implemented.
- [ ] Phase 4 owned helper reaping implemented.
- [ ] Phase 5 permanent-hold clear/auto-retry ledger implemented.
- [ ] Repair idempotency key uses daemon-keyed HMAC tuple.
- [ ] Gate proves every named proposal fixture.

## Verification Log

- `git rev-parse HEAD`
  - Result: `0e6482c82b588b74a76294a225e68286bfe37fa4`
- `git status --short -- ...P080 paths...`
  - Result: P080 implementation files and `scripts/test-gate.sh` are dirty; prior R2-R5 audit reports are untracked.
- `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/discover_prior_review.py ...`
  - Result: no prior proposal-review artifacts discovered.
- `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/security_sensitive_diff.py --root ... --json`
  - Result: triggered security-sensitive categories listed above.
- `CHAINWORKS_ALLOW_LOCAL_CARGO_TARGET_DIR=1 CHAINWORKS_CARGO_WRAPPER=0 ./scripts/test-gate.sh proposal-080`
  - Result: passed. Build/test emitted existing warnings only.
- `rg -n "P080|p080|stale execution|reconcile|clear_permanent_hold" 'Chainworks Forge' 'Chainworks ForgeTests'`
  - Result: no P080 Swift UI/repair surface; only unrelated YAML validator wording matched.
- `rg -n "p080.*mutation|async fn p080|Mutation|p080_diagnostics|p080Diagnostics" control-plane/crates/graphql-server/src`
  - Result: P080 query/subscription found; no P080 mutation found.

## Final Verdict

- Conformance verdict: Not Implemented.
- Readiness verdict: Not Ready.
- Confidence: High.

The implementation is a useful phase-scoped P080 substrate and its focused gate passes, but it does not satisfy the full P080 r28 contract. The blockers are the missing public MCP schema inventory, incomplete repair phases, Phase 2/Phase 3 gate mismatch for ACP startup stale repair, and non-conformant repair idempotency key derivation.
