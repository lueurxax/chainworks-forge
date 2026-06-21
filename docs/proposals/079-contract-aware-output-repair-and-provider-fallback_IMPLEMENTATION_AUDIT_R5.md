# P079 Implementation Audit R5: Contract-Aware Output Repair and Provider Fallback

| Field | Value |
|---|---|
| Proposal | `docs/proposals/079-contract-aware-output-repair-and-provider-fallback.md` |
| Report | `docs/proposals/079-contract-aware-output-repair-and-provider-fallback_IMPLEMENTATION_AUDIT_R5.md` |
| Generated | 2026-06-20T16:44:22+03:00 |
| Repo | `/Users/user/Documents/Chainworks Forge` |
| Audited revision | `0e6482c8` plus current dirty workspace |
| Proposal revision | `p079-contract-aware-output-repair-and-provider-fallback-r5` |
| Reviewer-selection input | `discover_prior_review.py docs/proposals/079-contract-aware-output-repair-and-provider-fallback.md` |
| Reviewer-selection reuse | Not reused. The helper returned no source-review artifacts. |
| Overall conformance | Partially Implemented |
| Readiness verdict | Not Ready |

## Executive Verdict

P079 has a real partial implementation: the SQLite schema, domain evidence model, repair-event and lease repositories, GraphQL/MCP/run-report readback, Swift DTO/presenter decode slice, ACP repair permission posture, plan-evidence hardening, deterministic fixture same-session repair, and core P079 metric declarations all exist and the local `proposal-079` gate exits successfully.

It is not implementation-complete against the proposal. The highest-risk issue is not merely a deferred lane: the current transcript-recovery path can promote transcript output to accepted recovery even though transport-attributed chunk ownership is documented as not implemented and `CHAINWORKS_P079_TRANSCRIPT_RECOVERY_ENABLED` is not enforced. Controlled provider fallback from frozen YAML policy is still missing. Production same-session repair remains fail-closed for all production providers. The macOS inspector UI and several rollout/reliability/docs requirements remain deferred.

This proposal should stay open. It is not closeout-ready and should not be retired into reference-only documentation as fully implemented.

## Scope And Method

Compared the proposal requirements, inline rollout contract, reference docs, gate definitions, and implementation surface in:

- `control-plane/crates/engine/src/executor.rs`
- `control-plane/crates/acp/src/transport.rs`
- `control-plane/crates/domain/src/output_contract_repair.rs`
- `control-plane/crates/db/migrations/095_p079_output_contract_repair.sql`
- `control-plane/crates/db/src/repos/output_contract_repair.rs`
- `control-plane/crates/db/src/metrics.rs`
- `control-plane/crates/graphql-server/src/types/stage.rs`
- `control-plane/crates/mcp-server/src/tools/reports.rs`
- `Chainworks Forge/Engine/Readback/OutputContractRepair/*`
- `Chainworks ForgeTests/Proposal079ContractRepairReadbackTests.swift`
- `docs/reference/output-contracts-failure-evidence-and-recovery.md`
- `docs/reference/test-gates.md`
- `docs/runbooks/orchestration/p079-output-repair.md`

The worktree was already dirty. I did not treat unrelated dirty files as audit defects, but the audit is against current source truth rather than a clean committed baseline.

## Reviewer And Lens Selection

Prior review reuse: none. The review-discovery helper returned:

```json
{
  "artifacts": [],
  "proposal_path": "/Users/user/Documents/Chainworks Forge/docs/proposals/079-contract-aware-output-repair-and-provider-fallback.md",
  "repo_root": "/Users/user/Documents/Chainworks Forge"
}
```

Selected audit lenses:

| Lens | Why selected |
|---|---|
| API contract | P079 defines closed schemas across SQLite, GraphQL, MCP, run report, and Swift DTOs. |
| Architecture | The proposal changes the output-settlement order and ownership boundaries. |
| Reliability | P079 has lease, restart, idempotency, sweep, and single-flight requirements. |
| Security | The proposal covers provider text, transcript parsing, filesystem writes, permission grants, redaction, and side-effect exclusions. |
| Observability/rollout | P079 requires metrics, gates, flags, runbook/readback, and rollback behavior. |
| Apple UI/UX | The canonical operator shell must decode and render P079 diagnostic state. |
| Performance/resource bounds | Transcript recovery and readback JSON parsing are explicitly bounded. |

Security helper result: triggered. Categories: `auth`, `dos_resource_limits`, `filesystem_subprocess_boundary`, `parser_boundary`, `public_ingress`, `secrets_redaction_privacy`, `unsafe_crypto_dependency`.

Implementation surface helper result: required lenses were `api-contract`, `apple-ui-ux`, `architecture`, `observability-rollout`, `performance`, `reliability`, and `security`.

## Implemented Surface

| Area | Status | Evidence |
|---|---|---|
| SQLite persistence | Implemented | `095_p079_output_contract_repair.sql` creates `output_contract_repair_events`, `output_contract_repair_leases`, and `output_contract_repair_fallback_parent_links` with closed CHECK values and uniqueness constraints. |
| Domain model | Implemented | `output_contract_repair.rs` defines v1 status, failure, repair, transcript recovery, fallback, lease, budget, permission, fallback packet, and feature-flag constants. |
| DB repository | Implemented/Partial | Repair event, lease, terminal settlement, TTL reclamation, metric emission, and fallback parent-link helpers exist. Fallback helpers are not called by an execution path. |
| Same-session repair | Partial | A fixture-capable repair path exists behind `CHAINWORKS_P079_OUTPUT_REPAIR_ENABLED`, with atomic event/lease insertion and `reserved -> prompt_sent` before ACP prompt dispatch. Production providers fail closed. |
| ACP permission posture | Partial | `p079_repair_canonical_paths` switches ACP permission handling to P079 posture; tests cover canonical write allow and unsafe continuation denial. Production provider runtimes are still advisory-only. |
| Transcript recovery | Failing/Partial | Bounded parsing and evidence types exist, but accepted recovery is not gated by `CHAINWORKS_P079_TRANSCRIPT_RECOVERY_ENABLED` and is not transport-attributed. |
| Provider fallback | Missing | Schema, constants, metrics names, fallback packet struct, lease table, and parent-link repo exist. No frozen YAML policy compile, fallback packet assembly/dispatch, fallback child execution, or principal-revocation path is wired. |
| Plan evidence | Implemented/Partial | Junie plan evidence collection/redaction and safe meta-root-relative readback exist. Retention and full proposal fixtures are still not fully proven by the gate. |
| GraphQL/MCP/run report readback | Implemented/Partial | Typed GraphQL `outputContractRepair` and MCP/report JSON surfaces exist with redaction. Some deferred lanes can only be represented by fixtures or schema defaults, not production events. |
| Swift DTO/presenter | Implemented/Partial | `OutputContractRepairEvidence.swift`, presenter, and 25 decode/presentation tests exist. The actual macOS inspector UI is not wired. |
| Metrics | Partial | Metric names and some repair lifecycle counters are declared/emitted. Full provider-fallback rollout metrics and dashboards/readback are deferred. |
| Reference docs/runbook | Partial | Current reference docs and runbook correctly describe partial implementation. Required P079 appendix docs are missing. |
| Canonical gate | Partial pass | `./scripts/test-gate.sh proposal-079` exits 0, but docs define it as a partial-acceptance gate. |

## Findings

### P0-SEC-001: Transcript recovery can be accepted without proposal-required transport attribution

Status: Open.

Requirement:

- Proposal goal: recover contract-valid output in the current transcript/provider envelope only when attributable to the current execution by transport-allocated identifiers.
- Rollout hold condition: any transcript or provider-envelope recovery accepted without transport-allocated attribution blocks rollout.
- Acceptance criteria: recovery has numeric bounds, fail-closed truncation, transport-derived attribution, parser version pin, and negative fixtures.

Evidence:

- `control-plane/crates/domain/src/output_contract_repair.rs` declares `FLAG_TRANSCRIPT_RECOVERY_ENABLED`, but source search found no runtime use of `CHAINWORKS_P079_TRANSCRIPT_RECOVERY_ENABLED` outside docs/constants.
- `control-plane/crates/engine/src/executor.rs` calls `p079_attempt_transcript_recovery(...)`; if validation succeeds, the caller sets `TranscriptRecoveryResult::Accepted`, stores `valid_outputs_from_transcript_recovery`, materializes the settlement, and later settles the event as recovered.
- The helper comment states transport-derived attribution is not implemented and says the function should conservatively return `Unavailable` with `attribution_not_verified` until chunk scanning proves ownership.
- The accepted branch overwrites the richer bounded recovery object with a minimal JSON object containing only `result`, `recovery_source`, and `recovery_parser_version`; GraphQL then relies on defaults for bound fields.
- `docs/reference/output-contracts-failure-evidence-and-recovery.md` says the transcript-recovery lane still fails closed until transport-attributed chunk scanning can prove current-execution ownership, which conflicts with the accepting caller path.

Risk:

The implementation can classify transcript text as recovered active output without the proposal's attribution proof or feature flag gate. This is exactly the failure mode P079's security constraints were meant to prevent.

Required fix:

Fail closed for transcript/provider-envelope recovery unless `CHAINWORKS_P079_TRANSCRIPT_RECOVERY_ENABLED` is enabled and transport-allocated chunk/session attribution is verified. Preserve explicit bound fields in the stored evidence. Add negative fixtures for forged attribution, missing flag, oversized payload, and provider-envelope source spoofing before accepting recovered output.

### P0-REQ-002: Controlled provider fallback from frozen YAML policy is not implemented

Status: Open.

Requirement:

- Proposal goal: allow at most one controlled provider fallback attempt after repair/recovery is unavailable or unsuccessful, only from frozen fallback policy.
- Acceptance criteria require YAML-declared, snapshot-frozen, drift-aware fallback policy, feature flags, fallback packet contract, single-flight lease keyed by frozen policy hash, principal binding, revocation abort, and child execution linkage.

Evidence:

- Source search for `output_repair_policies` found no workflow parser/compiler support. The only implementation hit is a comment in `executor.rs` noting "Without YAML output_repair_policies parsing".
- `executor.rs` writes `provider_fallback_json: None` for P079 events and has no production path that creates a fallback child execution from a frozen policy.
- `output_contract_repair.rs` and the migration define fallback packet/linkage/lease shapes, but `insert_fallback_parent_link` and related repo helpers are not called outside their own module.
- `docs/reference/test-gates.md` and the P079 runbook explicitly state controlled provider fallback dispatch from frozen YAML policy remains deferred.

Risk:

One of P079's core recovery mechanisms is absent. Runs that should recover via governed provider fallback still block or require manual/operator retry. The schema can present fallback success in fixtures, but the engine cannot produce it through the proposed controlled path.

Required fix:

Implement `output_repair_policies` parsing and snapshot freezing, fallback packet assembly/redaction/hash validation, single-flight fallback lease insertion, fallback child agent execution creation, principal binding/revocation checks, deadline combination, restart recovery, and final parent settlement. Add proposal fixtures for no-policy blocked, policy drift, duplicate fallback, release-lane rejection, principal revoked, packet oversized, and fallback success.

### P1-REQ-003: Production same-session repair is intentionally fail-closed for all production providers

Status: Open, safe but incomplete.

Requirement:

- Proposal goal: eligible roles receive at most one same-session corrective output repair turn.
- Same-session repair permission posture must be enforced server-side, not merely by prompt.

Evidence:

- `p079_provider_supports_enforced_permissions(provider_family)` returns true only for `fixture`.
- The executor sets `p079_permission_enforcement_advisory` for all production provider families and settles the repair event as `skipped` / `manual_investigation` before dispatch.
- Unit tests assert Codex, Claude, Gemini, Junie, and Auggie are advisory-only and that the legacy advisory opt-in env var is ignored.
- The integration test `invoke_agent_repairs_missing_required_output_in_same_live_session` uses the fixture provider and enables `CHAINWORKS_P079_OUTPUT_REPAIR_ENABLED`.

Risk:

This is the right security posture until provider runtimes expose enforceable restrictions, but it means the P079 production behavior is not implemented for the roles/providers the proposal targets.

Required fix:

Either add real runtime-enforced filesystem/tool/network restrictions for the production ACP providers and then enable same-session repair for the approved roles, or revise the proposal/status to explicitly scope production repair out of P079.

### P1-UX-004: The Swift DTO exists, but the macOS inspector UI is not wired

Status: Open.

Requirement:

- Proposal UI notes require compact operator status, badges, progress state, inspector groups, evidence path interactivity, accessibility labels, stale projection UX, pasteboard surfaces, and optional notification behavior.

Evidence:

- Source search for `OutputContractRepair` / `outputContractRepair` under `Chainworks Forge/Views` found no view integration.
- The Swift implementation is limited to `Chainworks Forge/Engine/Readback/OutputContractRepair/*` and `Chainworks ForgeTests/Proposal079ContractRepairReadbackTests.swift`.
- `docs/reference/test-gates.md` explicitly lists the macOS inspector UI as deferred.

Risk:

Operators get DTO/test coverage but not the promised diagnostic workflow in the canonical macOS shell. P079 evidence remains more accessible through MCP/GraphQL/reports than through the app.

Required fix:

Wire the presenter into run rows and `RunInspectorView` or the current inspector equivalent, including grouped diagnostics, progress/stale chips, safe path actions, accessibility identifiers, and snapshot identity behavior.

### P1-REL-005: Reliability, side-effect exclusion, and rollout observability remain partial

Status: Open.

Requirement:

- Acceptance criteria require release/durable side-effect exclusion, source-generation supersession eligibility exclusion, full projection rebuild/sweep behavior, stale-lease recovery, deterministic restart behavior, metric label contract, and required reference docs.

Evidence:

- `docs/reference/output-contracts-failure-evidence-and-recovery.md` lists as deferred: full projection artifact rebuild with bounded background sweep, release-lane and source-generation supersession eligibility exclusions, full rollout metric readback for provider fallback lanes, and required reference docs.
- `docs/reference/p079-repair-prompt-template.md`, `docs/reference/p079-recovery-attribution.md`, and `docs/reference/p079-adapter-idempotency.md` are absent.
- `P079_REQUIRED_METRICS` declares provider-fallback and release-lane metrics, but source search found no production provider-fallback dispatch path and no broad emission coverage for the full operational metric set.
- The executor checks cancellation, waiting approval, and blocking workflow conflict before repair, but the reference doc itself says release-lane and source-generation supersession eligibility exclusions remain deferred.

Risk:

The implementation can prove several repair/readback slices, but not the full operational invariants required to safely roll out recovery and fallback in production. Observability is also incomplete for the missing lanes.

Required fix:

Complete the deferred reliability slices or explicitly move them to a follow-up proposal: projection artifact rebuild/sweep, source-generation and release-lane eligibility checks before recovery/repair/fallback, full metric emission/readback with bounded labels, and the three required reference docs.

## Requirement Coverage Matrix

| Requirement | Conformance | Notes |
|---|---|---|
| Starts only after normal output collection fails | Implemented | P079 logic is entered after declared-output settlement validation detects repair-relevant failure. |
| Eligible failure classes | Partial | Core validation classes are checked. Release-lane and source-generation supersession exclusions remain deferred. |
| At most one same-session repair | Partial | Counter/path exists. Production providers are fail-closed; fixture proves the path. |
| Repair prompt template pin and caps | Partial | Prompt exists with `p079_repair_v1`, caps, and redaction logic. Required reference doc is missing. |
| Server-side repair permission posture | Partial | ACP posture exists for permission requests and tests cover denial/allow cases. Production runtimes remain advisory-only. |
| Transcript recovery with attribution | Not implemented correctly | Bounds parser exists, but accepted recovery lacks transport attribution and feature flag gating. |
| Provider-envelope recovery | Missing | Enum/schema vocabulary exists, but no P079 provider-envelope recovery path with transport attribution was found. |
| Controlled provider fallback | Missing | Data/schema scaffolding exists; no frozen YAML policy compile or fallback child dispatch path. |
| Fallback packet v1 | Partial | Struct and constants exist; packet assembly, redaction fixtures, hash binding, and dispatch use are missing. |
| Frozen fallback policy | Missing | No `output_repair_policies` parser/compiler/snapshot binding found. |
| Canonical path binding | Partial | Repair materialization and ACP posture include canonical paths; fallback and transcript recovery are not complete. |
| Source-generation settlement | Partial | Post-import CAS guards recovered settlement, but supersession eligibility exclusion remains deferred. |
| Lease commit before dispatch | Implemented for repair | Repair lease transitions to `prompt_sent` before ACP prompt. Fallback lease dispatch is not implemented. |
| Lease TTL and reclamation | Partial | DB/recovery code handles expired repair/fallback lease rows; full projection/sweep behavior remains deferred. |
| Lost-ACK/idempotency | Partial | Repair lease idempotency tokens exist. Adapter idempotency reference doc and fallback dispatch behavior are missing. |
| Principal binding/revocation for fallback | Missing | DB fields exist; no fallback dispatch/revocation execution path found. |
| Human approval/workflow conflict non-resolution | Partial | Repair skips waiting approval and active conflict. Fallback path missing. |
| Release/durable side-effect exclusion | Partial/Missing | Reference doc says release-lane exclusions remain deferred. |
| Plan evidence protection | Partial | Junie plan evidence copy/redaction/path hardening exists. Retention and full fixture coverage are not complete. |
| Readback through GraphQL/MCP/run report | Partial | Surfaces exist and are typed/redacted; missing lanes appear as defaults/schema-only. |
| Swift DTO and decode gate | Implemented | DTO/presenter and 25 Swift tests pass. |
| macOS inspector presentation | Missing | No view integration found. |
| Metrics | Partial | Names and some counters exist; full provider-fallback and rollout readback are deferred. |
| Canonical proposal gate | Partial pass | The gate passes, but is documented as partial-acceptance. |

## Gate And Verification Log

| Check | Result |
|---|---|
| `report_path.py` | Produced this R5 report path. |
| `discover_prior_review.py` | No artifacts discovered; reviewer-selection reuse not applied. |
| `git rev-parse --short HEAD` | `0e6482c8` |
| `security_sensitive_diff.py --root . --json` | Triggered; categories listed above. |
| `implementation_surface_fingerprint.py --root . --json` | Required lenses listed above. |
| `./scripts/test-gate.sh proposal-079` | Exited 0. Static checks passed; Rust/domain/db/engine/acp/graphql/mcp slices passed; Swift `Proposal079ContractRepairReadbackTests` ran 25 tests and passed. |
| Gate log caveat | The log includes a trailing `unexpected EOF while looking for matching '"'` after the P079 pass banner, but the process exit was 0 and `bash -n scripts/test-gate.sh` passed. |
| `bash -n scripts/test-gate.sh` | Passed. |

## Readiness Checklist

| Item | Ready? | Notes |
|---|---|---|
| Full proposal behavior implemented | No | Major recovery/fallback lanes missing or inconsistent. |
| Security hold conditions satisfied | No | Transcript recovery can be accepted without transport attribution. |
| Production same-session repair enabled safely | No | Production providers fail closed. |
| Controlled provider fallback implemented | No | Missing dispatch/policy/packet execution path. |
| Operator UI complete | No | DTO exists; inspector wiring missing. |
| Canonical gates pass | Partial | Current gate passes but is explicitly a partial-acceptance gate. |
| Suitable for closeout | No | Keep proposal open. |

## Recommended Next Actions

1. Treat `P0-SEC-001` as the immediate blocker: either make transcript recovery fail closed or complete transport-attributed recovery behind the transcript flag.
2. Implement the controlled fallback lane end to end or split it into a follow-up proposal and update P079 status accordingly.
3. Keep production same-session repair fail-closed until a real enforcement boundary exists, but do not mark P079 implemented while that remains true for all production providers.
4. Add the missing macOS inspector UI and required reference docs before closeout.
5. Update `docs/reference/test-gates.md` only after the gate proves full acceptance rather than the current partial acceptance slice.
