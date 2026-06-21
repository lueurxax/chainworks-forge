# P083 Implementation Audit R4: Execution-Truth Ownership and Invariant Model

## Metadata

- Audit date: 2026-06-20
- Proposal: `docs/proposals/083-execution-truth-ownership-invariant-model.md`
- Proposal revision audited: `P083-r70-refined-r69-score-lift`
- Repository root: `/Users/user/Documents/Chainworks Forge`
- Implementation target: current working tree at `0e6482c8`
- Compare base: not supplied; this audit inspected the current dirty working tree.
- Report path: `docs/proposals/083-execution-truth-ownership-invariant-model_IMPLEMENTATION_AUDIT_R4.md`
- Final verdict: **Not Implemented / Not Ready**

The working tree contains broad unrelated dirty state across P079, P080, P082, P086, Swift app, Rust control-plane, docs, and untracked audit reports. P083-specific conclusions below are based on current files and local gate execution, not on an isolated PR diff.

## Prior Review Reuse

`discover_prior_review.py docs/proposals/083-execution-truth-ownership-invariant-model.md` returned no reusable prior review artifacts:

```json
{
  "artifacts": [],
  "proposal_path": "/Users/user/Documents/Chainworks Forge/docs/proposals/083-execution-truth-ownership-invariant-model.md",
  "repo_root": "/Users/user/Documents/Chainworks Forge"
}
```

Reviewer-selection reuse: **not reused**. Existing `IMPLEMENTATION_AUDIT_R1`, `R2`, and `R3` were treated as historical context only, not as reusable reviewer-routing evidence.

## Proposal State And Contract Summary

P083 defines SQLite-backed execution truth for runs, stages, agents, approvals, artifacts, side effects, provider sessions, command idempotency, shutdown receipts, rollout state, and operator readback. The proposal requires:

- durable caller-owned `CallerRequestId` command idempotency for eight lifecycle commands;
- rollback target parity across GraphQL, MCP, idempotency hashing, audit rows, and rollout readback;
- GraphQL and MCP lifecycle contracts with closed enums, shared denial vocabulary, strict schemas, and current revision fixtures;
- eight additive SQLite migrations with migration readback;
- durable monotonic clock baseline correlation for deadline-bearing rows;
- shutdown/cancellation/late-output recovery rules;
- bounded operational metrics and rollout-contract hold conditions;
- macOS command placement, toolbar parity, and manual identity-check UI behavior;
- `current_review_refresh_gate_v1` before Ready.

The proposal itself is still `Revise-required` and says implementation may start only after a human implementation approval gate plus a fresh aggregate review against this exact revision with `decision=approve`, `blocker_count=0`, and corpus-only-current-revision attestation (`docs/proposals/083-execution-truth-ownership-invariant-model.md:1`, `:7`, `:43-46`, `:598-606`, `:980`).

## Platform/Product Scope

- Rust control-plane: DB migrations/repos, engine command handler, shutdown recovery, daemon startup baseline, GraphQL, MCP, metrics, rollout readback.
- macOS SwiftUI app: read-only operator shell, Run command menu/toolbar affordances, manual identity-check banner and readback model.
- Documentation/evidence: reference docs, MCP schema paths, evidence fixtures, rollout contract negative fixtures, and `proposal-083` gate.

Primary flows audited:

1. Operator lifecycle commands: `runs.cancel`, `runs.retry`, `stages.retry`, `approvals.resolve`, `side_effects.force_reconcile`, `provider_session.shutdown`, `p083.rollback_execution`, `p083.set_enforcement_mode`.
2. Rollout-contract readback across GraphQL/MCP/run-report/release-receipt expectations.
3. Shutdown, cancellation, durable clock, and post-cancel late-output recovery.
4. Manual process identity-check UI and native macOS command placement.
5. Evidence corpus, rollout fixtures, current-review gate, and canonical proof gate.

## Reviewer Selection

Mandatory helper `implementation_surface_fingerprint.py --root . --json` required these lenses:

- `api-contract`
- `apple-ui-ux`
- `architecture`
- `observability-rollout`
- `performance`
- `reliability`
- `security`

Selected routed reviewers/lenses for this audit:

- `rust_architecture_reviewer`
- `rust_reliability_reviewer`
- `api_contract_reviewer`
- `observability_rollout_reviewer`
- `rust_security_reviewer`

Rejected or scoped-down lenses:

- `apple-ui-ux`: triggered and manually inspected, but not selected as a full independent specialist because of the hard cap. This remains a readiness coverage gap for any Ready verdict.
- `performance`: triggered, but only scoped for resource/DoS risk and metric cardinality. No performance Ready clearance is claimed.

## Security-Sensitive Diff Summary

Mandatory helper `security_sensitive_diff.py --root . --json` triggered a security-sensitive diff. Categories:

- `auth`
- `dos_resource_limits`
- `filesystem_subprocess_boundary`
- `parser_boundary`
- `public_ingress`
- `secrets_redaction_privacy`
- `unsafe_crypto_dependency`

Independent security pass summary:

- Positive evidence: P083 commands derive caller identity from authenticated principal context, `command_idempotency` explicitly warns principal IDs must come from verified auth context, GraphQL/MCP handlers validate caller request IDs, P083 rollback target is now part of GraphQL/MCP/idempotency/audit wiring, process-start identity is hashed before UI readback, and pasteboard writes use current-host-only APIs.
- Blocking security/readiness issues: current-revision security review evidence is missing/stale by the proposal's own rollout hold rules; the evidence corpus has mixed revisions; the canonical proof gate fails before proving security-sensitive command paths; MCP schema files promised by the proposal are absent; and GraphQL denial semantics do not match the proposal's shared `DenialPayload` union contract.

No direct exploit was validated in this audit, but the security hard gate cannot be cleared.

## Track 1: Proposal Requirement Conformance

Overall Track 1 result: **Fail**.

### Blocking Findings

**P083-R4-001 - Current review gate is unsatisfied.**

The proposal declares `implementation_may_start=false` and requires a fresh aggregate review against `P083-r70-refined-r69-score-lift` before Ready. Local search found no current aggregate review artifact for R70. `discover_prior_review.py` found no reusable artifacts. This alone prevents implementation-complete, closeout-ready, or release-ready status.

**P083-R4-002 - Canonical proposal gate fails on the declared evidence corpus.**

`bash ./scripts/test-gate.sh proposal-083` exited `1` immediately:

```text
proposal-083: FAIL - declared evidence corpus is incomplete (49 missing of 112 declared paths)
```

The missing paths include GraphQL/MCP parity fixtures, rollback-target fixtures, durable clock fixtures, SwiftData boundary fixtures, idempotency fixtures, UI action-hierarchy fixtures, macOS menu/toolbar fixtures, and rollout negative fixtures. Because the gate stops here, it does not prove the Rust/Swift implementation.

**P083-R4-003 - Evidence corpus is mixed-revision and stale.**

A proposal-path scan found 112 declared evidence paths, 49 missing, and 52 existing JSON fixtures with stale `proposal_revision_id` values. The operator readback fixture is still `P083-r64-refined-3da64326`, has `rollout_contract_status: fail`, `rollout_contract_decision: hold`, `rollout_contract_projection_integrity: stale`, and states it is a placeholder, not release evidence.

**P083-R4-004 - GraphQL lifecycle contract does not match the proposal's shared denial union.**

The proposal requires lifecycle mutations to use a shared `DenialPayload` branch with `DenialReason` parity (`docs/proposals/083-execution-truth-ownership-invariant-model.md:965`). Current GraphQL code returns concrete success payloads inside `Result<...>` and maps failures into GraphQL errors (`control-plane/crates/graphql-server/src/schema.rs:5412-5468`, `:5819-6083`; `control-plane/crates/graphql-server/src/types/p083.rs:738-847`). `rg` found no `DenialPayload` or `DenialReason` GraphQL type in `control-plane/crates/graphql-server/src`.

This is a contract divergence, not just a missing fixture.

**P083-R4-005 - MCP inventory schema files promised by the proposal are absent.**

The proposal lists schema paths such as `docs/reference/mcp/p083/runs.cancel.input.schema.json` under `mcp_tool_inventory_contract_v1`, but `docs/reference/mcp` does not exist in the current tree. The MCP server does contain inline JSON schemas and handlers in `control-plane/crates/mcp-server/src/tools/runs.rs`, including `additionalProperties: false` and `target_enforcement_mode` for `p083.rollback_execution`, but the proposal requires durable schema files and parity fixtures.

**P083-R4-006 - Native macOS command contract is not implemented as specified.**

The proposal requires a top-level Run menu with `Lifecycle` and `Recovery` submenus, fixed ordering, key equivalents, and coverage for `Export Text` and `Copy Diagnostic` (`docs/proposals/083-execution-truth-ownership-invariant-model.md:1277-1310`). The current implementation has a flat `CommandMenu("Run")` with buttons for Cancel, Retry, Retry Stage, Resolve Approval, Shutdown Provider Session, and Retry Identity Check, with no submenus, no key equivalents, and no menu items for Copy Diagnostic or Export Text (`Chainworks Forge/Chainworks_ForgeApp.swift:493-516`).

**P083-R4-007 - Manual identity-check UI is partial and diverges from the action/feedback contract.**

The proposal requires primary/secondary/tertiary/overflow placement, confirmation dialog for Mark Process Absent, loading/success/error states, 1500 ms copy confirmation, reduce-motion-aware feedback, duplicate-session grouping, and no focus-stealing spinner (`docs/proposals/083-execution-truth-ownership-invariant-model.md:1166-1238`). The current banner exists and includes visible copy, copy diagnostic, retry, mark-absent, and open-evidence actions (`Chainworks Forge/Views/ManualProcessIdentityCheckBanner.swift:16-199`), but actions are rendered as a flat HStack, Mark Process Absent has no confirmation dialog, Retry has no loading/error feedback, Copy confirmation sleeps for 2000 ms, Open Evidence is a plain button rather than overflow, and duplicate sessions render as separate banners (`Chainworks Forge/Views/P083IdentityAmbiguousInboxView.swift:29-49`).

**P083-R4-008 - SwiftData, durable clock, hardening, and rollout proofs are not complete.**

Some code exists for durable monotonic clock baseline insertion, bounded metrics, command idempotency policies, shutdown recovery, and provider cancellation intent handling. However, the declared clock, SwiftData, idempotency, hardening, UI, macOS, and rollout fixtures are missing or stale. The proposal requires all implementation hardening items to be proven before P083 can be marked implementation-complete (`docs/proposals/083-execution-truth-ownership-invariant-model.md:1312-1348`, `:981-983`).

### Positive Implementation Evidence

The implementation is not empty. Current tree includes substantial P083-shaped work:

- All eight P083 physical SQLite migration files exist: `087_p083_001_artifact_lineage_report_kind.sql` through `094_p083_008_signal_dispatching_state.sql`.
- Migration DDL includes command idempotency tables, shutdown receipts/signal side effects, late-output overflow latches, enforcement/rollback audit, durable monotonic clock samples, provider cancellation intents, and signal dispatching state.
- GraphQL has P083 lifecycle mutations and R70 rollback target input: `providerSessionShutdown`, `p083RollbackExecution(targetEnforcementMode, callerRequestId)`, `p083SetEnforcementMode`, `runsRetry`, and `p083MarkProviderSessionProcessAbsent`.
- MCP inline schemas and handlers exist for P083 lifecycle tools and validate `caller_request_id`, `target_enforcement_mode`, and operator principal class.
- Command handler paths use command idempotency acquisition/commit/fail/replay patterns for P083 commands and include `target_enforcement_mode` in rollback intent hash/audit.
- Daemon startup runs P083 shutdown recovery, dispatches planned shutdown signals, and inserts a durable monotonic clock baseline.
- Metrics include P083 required metric names and bounded label domains.
- SwiftUI includes a readback model and manual identity-check banner surface backed by `p083IdentityHoldSessions`.
- Reference docs already describe P083 as implemented-system context in `docs/reference/execution-truth-and-recovery.md` and `docs/reference/rust-control-plane.md`.

### Coverage Matrix

| Requirement area | Status | Evidence |
| --- | --- | --- |
| Current-review refresh gate | Fail | Proposal requires fresh R70 approve/blocker_count=0; no artifact found. |
| Canonical `proposal-083` gate | Fail | Exit 1, 49 missing declared evidence paths. |
| Evidence corpus current revision | Fail | 49 missing, 52 stale JSON fixtures, R64 placeholder readback. |
| Eight additive migrations | Partial | Files present; gate did not reach migration tests. |
| Command idempotency for eight commands | Partial | Repo/handler code present; missing/stale fixtures and gate failure. |
| Rollback target parity | Partial | GraphQL/MCP/handler/audit code present; required fixtures missing/stale. |
| GraphQL SDL contract | Fail | Closed enum pieces present; shared denial union/payload contract absent. |
| MCP tool inventory contract | Fail | Inline schemas present; promised `docs/reference/mcp/p083` schema files absent. |
| Durable monotonic clock | Partial | Daemon baseline and migration present; all declared clock fixtures missing. |
| SwiftData lifecycle boundary | Not proven | Required SwiftData concurrency fixtures missing. |
| Manual process identity UI | Partial/Fail | Banner exists; action hierarchy and feedback contract not met/proven. |
| Native command validation | Fail | Flat Run menu; no submenus/key equivalents/Copy Diagnostic/Export Text. |
| Metrics/rollout labels | Partial | Recorders/domains present; metric fixtures stale or not fully proven by gate. |
| Shutdown/cancellation/late output | Partial | Migrations/repos/services exist; fixtures stale/missing and gate not green. |
| Hardening requirements | Not proven | Some code present; mandatory proof incomplete and gate fails. |

## Track 2: Routed Specialist Review

Overall Track 2 result: **Fail / Not Ready**.

### Reviewer/Lens Scorecard

| Lens | Result | Notes |
| --- | --- | --- |
| Security | Fail for readiness | Security-sensitive diff triggered; no exploit proven, but current review/evidence/gate hard gates are not satisfied. |
| Reliability | Fail | Durable recovery code exists, but clock/shutdown/idempotency evidence is missing/stale and the canonical gate fails. |
| API contract | Fail | GraphQL denial union and MCP schema-file contracts diverge from proposal; parity fixtures missing. |
| Architecture | Not Ready | SQLite authority direction is aligned, but active proposal gates and proof artifacts do not allow implementation-complete status. |
| Observability/rollout | Fail | Rollout readback fixture is stale placeholder; hold/negative fixtures missing; current security/rollout reviews missing or stale. |
| Apple UI/UX | Fail, scoped manual pass | Native menu and manual identity banner do not satisfy specified structure/feedback. |
| Performance | Not cleared | No full performance pass; DoS/resource surfaces remain under security/reliability until gate evidence exists. |

### Routed Findings

- API: Convert GraphQL lifecycle failure branches to the proposal's shared `DenialPayload`/`DenialReason` union contract, or revise the proposal to accept GraphQL error extensions as the authoritative denial shape and regenerate parity fixtures.
- API: Materialize the promised MCP Draft 2020-12 schema files under `docs/reference/mcp/p083/` and prove byte parity with GraphQL denial/enum vocabularies.
- Reliability: Regenerate all idempotency, clock, shutdown, cancellation, late-output, and SwiftData fixtures for `P083-r70-refined-r69-score-lift`.
- macOS/UI: Implement the specified Run > Lifecycle and Run > Recovery menu hierarchy, keyboard equivalents, toolbar/menu parity, disabled reason exposure, and manual identity banner action hierarchy/feedback states.
- Observability/rollout: Replace the stale R64 operator readback placeholder with current revision readback across all required lanes and negative fixtures.
- Security: Run a fresh current-revision security review after the contract/evidence fixes and include corpus-only-current-revision attestation.

## Residual Scope And Follow-Up Ownership

- Code owner: finish API contract parity, MCP schema artifacts, UI/native command behavior, and any missing hardening code.
- Evidence owner: rebuild the declared evidence corpus for `P083-r70-refined-r69-score-lift`; remove or archive stale R64/R68 fixtures from active P083 proof paths.
- Gate owner: keep `proposal-083` failing on missing evidence, and add/retain current-revision-only checks so stale fixtures cannot satisfy proof accidentally.
- Review owner: run fresh aggregate proposal review against R70 with `decision=approve`, `blocker_count=0`, and corpus-only-current-revision attestation before any Ready claim.
- Release owner: rerun `./scripts/test-gate.sh proposal-083`, then the same-tree canonical full gate required by repo policy.

## Readiness Checklist

- [ ] Fresh R70 aggregate review approves with `blocker_count=0`.
- [ ] Human implementation approval gate is granted.
- [ ] All 112 declared evidence paths exist or the proposal is updated to remove inactive paths.
- [ ] All P083 fixtures carry `proposal_id=P083` and `proposal_revision_id=P083-r70-refined-r69-score-lift`.
- [ ] `./scripts/test-gate.sh proposal-083` passes in the same tree.
- [ ] GraphQL lifecycle denial shape matches proposal or proposal is explicitly revised.
- [ ] MCP schema files exist and match inline/server behavior.
- [ ] Native Run menu, toolbar parity, keyboard equivalents, and accessibility evidence match `native_command_validation_contract_v1`.
- [ ] Manual identity banner action hierarchy, confirmation, feedback states, duplicate handling, and accessibility evidence match `manual_process_identity_check_ui_v1`.
- [ ] SwiftData, durable clock, idempotency, shutdown, cancellation, late-output, metrics, and hardening proofs are current and passing.
- [ ] Fresh security and observability/rollout review artifacts are current-revision only.
- [ ] Same-tree canonical full gate passes before closeout.

## Verification Log

- Read proposal JSON from `docs/proposals/083-execution-truth-ownership-invariant-model.md`.
- Ran `report_path.py`; next report path is this R4 file.
- Ran `discover_prior_review.py`; no reusable prior review artifacts found.
- Ran `implementation_surface_fingerprint.py`; required lenses: API contract, Apple UI/UX, architecture, observability/rollout, performance, reliability, security.
- Ran `security_sensitive_diff.py`; triggered security-sensitive categories listed above.
- Scanned declared evidence paths from proposal JSON: 112 declared, 49 missing, 52 stale JSON fixtures, 0 invalid JSON fixtures.
- Ran `bash ./scripts/test-gate.sh proposal-083`; failed exit 1 on incomplete declared evidence corpus.
- Inspected P083 migrations, GraphQL types/mutations, MCP inline tool schemas/handlers, command idempotency repo/handler references, daemon durable clock startup, metrics, SwiftUI command/menu code, and manual identity-check UI code.
- Did not run `./scripts/test-gate.sh full` because the focused proposal gate already fails and a successful Ready verdict is impossible.

## Final Verdict And Recommended Next Actions

Verdict: **Not Implemented / Not Ready / Not Closeout-Ready**.

The backend contains meaningful P083 implementation work, especially migrations, command idempotency, rollback target wiring, shutdown recovery, monotonic baseline insertion, metrics, MCP handlers, and GraphQL mutations. However, P083 cannot be marked implemented because the proposal's own review gate is unsatisfied, the canonical P083 gate fails, the evidence corpus is missing/stale, GraphQL and MCP contracts diverge from proposal text, and the macOS/manual identity UI contracts are only partially implemented.

Recommended next actions:

1. Regenerate and normalize the full P083 evidence corpus for `P083-r70-refined-r69-score-lift`.
2. Resolve GraphQL/MCP contract drift before adding more fixture placeholders.
3. Finish native command and manual identity UI behavior to match the proposal.
4. Re-run `./scripts/test-gate.sh proposal-083`.
5. Only after the focused gate is green, obtain the fresh R70 aggregate review and run the same-tree full/canonical gate.
