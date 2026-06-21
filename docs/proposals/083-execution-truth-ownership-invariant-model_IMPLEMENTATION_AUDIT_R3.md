# P083 Implementation Audit R3 - Execution Truth Ownership Invariant Model

Audit timestamp: 2026-06-20T12:33:23Z
Audited proposal: `docs/proposals/083-execution-truth-ownership-invariant-model.md`
Audited tree: `0e6482c8` with a dirty working tree
Report path: `docs/proposals/083-execution-truth-ownership-invariant-model_IMPLEMENTATION_AUDIT_R3.md`

## Verdict

Overall conformance: Not Implemented for closeout.

Overall readiness: Not Ready.

The implementation contains substantial P083 backend work: additive SQLite migrations, command idempotency plumbing, rollback/set-enforcement handlers, GraphQL and MCP surfaces, shutdown/cancellation recovery, metrics/readback, and macOS readback UI surfaces. It is not closeout-conformant because P083 makes current-revision review, declared fixture evidence, gate behavior, migration inventory, API parity, and macOS action hierarchy part of the implementation contract. Those requirements are not fully satisfied in the audited tree.

## Routing and Review Coverage

Prior-review routing:

- `discover_prior_review.py` returned no prior artifacts for this proposal.
- Reviewer reuse: not reused.

Mandatory helper routing:

- `security_sensitive_diff.py --json` triggered. Categories included auth, public ingress, parser boundary, filesystem/subprocess boundary, DoS/resource limits, secrets/redaction/privacy, and unsafe crypto/dependency. A security-focused manual pass was performed over the P083 lifecycle/auth/API/readback surfaces.
- `implementation_surface_fingerprint.py --json` required these lenses: api-contract, apple-ui-ux, architecture, observability-rollout, performance, reliability, and security.
- Selected primary reviewers under the five-reviewer cap: `rust_arch_reviewer`, `rust_reliability_reviewer`, `api_contract_reviewer`, `observability_rollout_reviewer`, and `rust_security_reviewer`.
- Explicit macOS UI/UX inspection was also performed because P083 contains UI acceptance criteria, but `apple-ui-ux` was not independently covered by a selected specialist under the hard cap. That remains a readiness coverage gap.

Primary audited flows:

1. Lifecycle mutation path through GraphQL/MCP, auth classification, command handler idempotency, SQLite writes, and readback.
2. Provider shutdown/cancellation path through durable signal side effects, restart recovery, and manual identity holds.
3. P083 rollback and enforcement-mode mutation path through target mode validation, intent hashing, audit persistence, and readback.
4. macOS operator shell path through Run menu/toolbar commands, identity banner, copy/export, and read-only lifecycle guidance.
5. Rollout and evidence path through migration descriptors, rollout contract linting, metrics, negative fixtures, and `proposal-083` gate behavior.

## Canonical Gate Result

Command: `./scripts/test-gate.sh proposal-083`

Result: failed with exit code 127.

The P083 body of the gate printed `==> Proposal 083 gate passed` after running these checks:

- DB migration suite: 57 tests passed.
- `cargo check -p db`: passed.
- Engine P083 tests: 21 tests passed.
- Engine shutdown tests: 6 tests passed.
- `cargo check` for daemon, GraphQL server, and MCP server: passed with warnings.
- Domain denial-code round-trip test: passed.
- GraphQL approval mutation tests: 5 tests passed.
- MCP P083 tests: 3 tests passed.
- Rollout contract lint: PASS.
- Static proofs found eight physical P083 migrations, eight readback descriptors, rollback disposition validation, atomic idempotency acquire path, monotonic clock source, baseline correlation, rollback/set-enforcement API and idempotency, and macOS Run menu/toolbar strings.

The overall command still exited nonzero after the P083 body completed:

```text
./scripts/test-gate.sh: line 11888: h_templates_do_not_double_nest_run_meta_root: command not found
```

Because the repository policy requires canonical gates to be green, this alone blocks Ready.

## Blocking Findings

### P0 - Current-revision approval and review gate are not satisfied

P083 still says implementation may start only after human implementation approval and a fresh aggregate review against the current revision with `blocker_count=0` (`docs/proposals/083-execution-truth-ownership-invariant-model.md:7`, `docs/proposals/083-execution-truth-ownership-invariant-model.md:26`, `docs/proposals/083-execution-truth-ownership-invariant-model.md:584`). The acceptance criteria also require `current_review_refresh_gate_v1` before Ready (`docs/proposals/083-execution-truth-ownership-invariant-model.md:944`).

No current-revision approval/aggregate-review artifact was found by the prior-review helper, and no concrete follow-up proposal owns this omission. Under the implementation tail gate, this is in scope for P083 and blocks both conformance and readiness.

### P0 - Canonical `proposal-083` gate does not exit successfully

The same-tree canonical gate failed with exit code 127 after the internal P083 checks printed pass. The gate is the declared proving path for the proposal and is the only acceptable closeout proof. A body-level pass is not sufficient when the command process exits nonzero.

### P0 - Declared evidence corpus is incomplete and the gate misses it

The proposal declares 112 `docs/evidence/*` paths. A direct evidence-path check found 49 missing paths. Missing examples include:

- GraphQL/MCP parity fixtures for lifecycle mutation SDL, denial vocabulary, enum vocabulary, additionalProperties rejection, rollback target required/invalid enum, approval resolution enum constraints, and output schema parity.
- Durable clock fixtures for baseline startup insertion, shutdown-signal/cancellation baseline correlation, wall/monotonic ordering, nonzero delta enforcement, and missing-baseline startup failure.
- SwiftData concurrency fixtures for main-actor model contexts, model-actor adapters, Sendable projection snapshots, and rejected non-Sendable leakage.
- Rollout negative fixtures for mixed revision corpus, missing durable monotonic baseline, and invalid rollback target contract.
- Idempotency fixtures for canonical serialization, caller-request-id exclusion, uppercase enum rejection, rollback same-request/new-request/mismatch behavior, and set-enforcement enum constraints.
- macOS fixtures for action hierarchy, feedback states, copy confirmation, duplicate rollup, byte-stable Run menu structure, toolbar/menu accessibility parity, and focused-value command wiring.

The gate script verifies selected P083 static properties (`scripts/test-gate.sh:9369`) but does not fail on the missing declared evidence corpus. Since P083 requires the proof gate to fail when hardening items lack evidence, the gate itself is incomplete.

### P1 - Migration inventory drift: proposal says seven, implementation proves eight

P083 states that it owns seven additive migrations and names `p083_001` through `p083_007` (`docs/proposals/083-execution-truth-ownership-invariant-model.md:353`, `docs/proposals/083-execution-truth-ownership-invariant-model.md:756`). The implementation readback descriptor contains eight P083 migrations, including `p083_008_signal_dispatching_state` (`control-plane/crates/db/src/repos/rollout_contract_checks.rs:1403`, `control-plane/crates/db/src/repos/rollout_contract_checks.rs:1483`), and the gate proves all eight physical migration files.

The implementation may be functionally valid, but the rollout contract and proposal contract disagree. That drift must be resolved before closeout because migration count, file inventory, and SHA readback are explicit P083 contract surfaces.

### P1 - GraphQL/MCP lifecycle API parity is not fully proven and likely diverges for some commands

The rollback and set-enforcement mutations carry the R70 target mode and caller request id in the GraphQL schema implementation (`control-plane/crates/graphql-server/src/schema.rs:5907`, `control-plane/crates/graphql-server/src/schema.rs:5954`). However, other lifecycle mutations still expose `request_id` naming and, for provider shutdown, an extra required `reason` argument (`control-plane/crates/graphql-server/src/schema.rs:5838`, `control-plane/crates/graphql-server/src/schema.rs:6001`). P083's declared lifecycle SDL requires non-null `CallerRequestId`-named inputs and fixed mutation signatures (`docs/proposals/083-execution-truth-ownership-invariant-model.md:151`).

The missing SDL and GraphQL/MCP parity fixtures make it impossible to prove the public contract is compatible with the proposal. This is a contract blocker, not only an evidence gap.

### P1 - macOS action hierarchy and feedback contract are incomplete

The macOS surfaces exist, but the observed implementation does not fully match P083's UI contract:

- `ManualProcessIdentityCheckBanner` renders actions in copy, retry, mark, evidence order (`Chainworks Forge/Views/ManualProcessIdentityCheckBanner.swift:81`) instead of the proposal's primary retry, secondary mark, tertiary copy/footer, and evidence-overflow hierarchy.
- The banner provides copy confirmation but no full loading/success/error feedback model for retry and mark actions (`Chainworks Forge/Views/ManualProcessIdentityCheckBanner.swift:28`, `Chainworks Forge/Views/ManualProcessIdentityCheckBanner.swift:81`).
- The Run menu exists (`Chainworks Forge/Chainworks_ForgeApp.swift:493`) but the audited code does not show the required `Run > Lifecycle` and `Run > Recovery` submenu structure, fixed order, and key-equivalent parity from the acceptance criteria.
- The static gate proves menu/toolbar strings, not the declared byte-stable menu structure, accessibility parity, focused-value wiring, or remote UI behavior.

Because P083 explicitly includes macOS copy/export/menu/banner requirements, this blocks closeout.

### P1 - Hardening requirements are not fully proven

The proposal marks P083-HARDEN-003, 004, 007, 008, 009, and 011 as mandatory before closeout or release (`docs/proposals/083-execution-truth-ownership-invariant-model.md:1298`). The implementation includes several relevant surfaces, including command idempotency, late-output overflow latching, shutdown recovery, and side-effect handling. The audited evidence does not prove all required hardening behavior end to end:

- Failed-terminal retry policy per lifecycle command is not covered by the declared fixture set.
- Atomic late-output concurrent-writer behavior is not covered by the gate evidence.
- External side-effect composition across crash/replay boundaries is not proven for each idempotent command.
- Minimum command lease TTL policy is not surfaced as a complete contract proof.
- Schema-version evolution policy and artifact-lineage backfill posture are not fully tied to rollout evidence.

These cannot be deferred without a concrete successor proposal. No such owner was found.

## Track 1 - Proposal Conformance Inventory

| Requirement area | Status | Evidence and gap |
| --- | --- | --- |
| SQLite as execution-truth authority | Partial | Additive DB surfaces and readback descriptors exist, but current-review/evidence/gate blockers prevent closeout. |
| Caller classification and lifecycle idempotency | Partial | Command handler uses idempotency across lifecycle commands, but declared idempotency fixtures are missing and full failed-terminal/replay policy is not proven. |
| R70 rollback target mode through API, idempotency, audit, and readback | Partial | Backend handlers and gate static proof cover the main path; declared GraphQL/MCP/idempotency negative fixtures are missing. |
| GraphQL and MCP closed enums, denial parity, and schema strictness | Partial | P083-specific tools and mutations exist; public parity fixtures are missing and some GraphQL lifecycle signatures appear out of contract. |
| Additive migrations and SHA readback | Partial | Eight migrations and SHA readback are implemented; the proposal and rollout contract text still say seven. |
| Shutdown, cancellation, recovery, and monotonic clock | Partial | Runtime and daemon recovery code exist and tests pass; declared durable-clock and baseline-correlation fixtures are missing. |
| Late-output overflow and signal side-effect handling | Partial | Repositories and recovery paths exist; concurrent-writer and crash-composition hardening proofs are incomplete. |
| SwiftData lifecycle boundary and projection concurrency | Partial / not verifiable | Some pre-P083 store evidence exists; declared Swift concurrency fixtures are missing. |
| macOS read-only lifecycle shell | Partial | UI surfaces exist; action hierarchy, feedback states, submenu/key-equivalent parity, and UI proof fixtures are incomplete. |
| Metrics, rollout contract, and proof gates | Partial | Rollout lint passes and metrics are wired; current-review gate, declared fixture completeness, and canonical gate exit status fail. |
| Current-revision reviewer corpus before Ready | Missing | The proposal's own current-review gate is not satisfied, and no owner/follow-up was found. |

## Track 2 - Specialist Review Summary

| Reviewer | Verdict | Notes |
| --- | --- | --- |
| `rust_arch_reviewer` | Not Ready | Main architecture surfaces are present, but the migration inventory drift and evidence/gate gaps make the ownership invariant incomplete as a release contract. |
| `rust_reliability_reviewer` | Not Ready | Shutdown, recovery, and idempotency implementations are substantial, but canonical gate failure and missing hardening proofs block readiness. |
| `api_contract_reviewer` | Not Ready | GraphQL/MCP parity is not proven, required fixtures are absent, and some lifecycle signatures appear inconsistent with the proposal SDL. |
| `observability_rollout_reviewer` | Not Ready | Rollout contract lint passes, but current-review, evidence completeness, and gate process behavior are failing. |
| `rust_security_reviewer` | Not Ready | No new standalone critical security defect was found in the manual pass, but security-sensitive diff triggered and current-revision security/review evidence is missing. |
| macOS UI/UX scoped inspection | Not Ready | UI surfaces exist but do not satisfy the declared action hierarchy, feedback, menu, toolbar, and accessibility proof contract. |

## Positive Implementation Evidence

- P083 migration and rollout readback code exists in `control-plane/crates/db/src/repos/rollout_contract_checks.rs`.
- P083 lifecycle idempotency is integrated into command handlers for retry, approvals, shutdown, rollback, set-enforcement, and force-reconcile paths.
- Rollback execution normalizes target enforcement mode before intent hashing and writes audit/readback outcome data.
- GraphQL and MCP P083 rollback/set-enforcement surfaces exist.
- Shutdown recovery, planned signal dispatch, cancellation intent, identity-ambiguous holds, and monotonic baseline startup logic are implemented.
- macOS P083 readback surfaces, Run command actions, and the manual process identity banner exist.
- The P083 gate body exercises a meaningful Rust test set and static proof set before the wrapper failure.

## Residual Scope Without Follow-up Owner

The following are still owned by P083 because no concrete successor proposal or follow-up owner was found:

- Current-revision human approval and fresh aggregate review with `blocker_count=0`.
- Completion of all declared evidence paths and a gate assertion that fails when any required fixture is absent.
- Resolution of the seven-vs-eight migration contract drift.
- API contract reconciliation for all lifecycle mutations and parity fixtures across GraphQL and MCP.
- macOS action hierarchy, feedback states, Run submenu/key-equivalent parity, accessibility parity, and remote UI proof.
- Hardening proof for failed-terminal retry policy, late-output concurrent counters, side-effect crash composition, schema version evolution, artifact-lineage backfill posture, and minimum lease TTL.
- Canonical `proposal-083` gate process exit 0.

## Closeout Recommendation

Do not close out P083.

Before another closeout attempt, complete the current-review gate, reconcile the proposal/implementation migration inventory, add or regenerate the missing declared fixtures, strengthen the `proposal-083` gate so it checks fixture completeness, align the GraphQL/MCP lifecycle contract with the proposal, complete the macOS UI action/menu proof surface, and rerun the canonical gate until the command exits 0.
