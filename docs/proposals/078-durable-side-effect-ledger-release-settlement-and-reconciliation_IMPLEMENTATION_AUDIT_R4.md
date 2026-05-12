# Proposal 078 Implementation Audit R4

## Metadata

| Field | Value |
| --- | --- |
| Proposal | `docs/proposals/078-durable-side-effect-ledger-release-settlement-and-reconciliation.md` |
| Proposal revision | `p078-refined-2026-05-07-dfc2d583-r2` |
| Audit report | `docs/proposals/078-durable-side-effect-ledger-release-settlement-and-reconciliation_IMPLEMENTATION_AUDIT_R4.md` |
| Audit timestamp | 2026-05-12 22:46:29 EEST |
| Worktree | `.chainworks/worktrees/cw-implement-proposal-078-durable-e49976db` |
| Branch | `cw/implement-proposal-078-durable/e49976db` |
| Audited HEAD | `8fb2afa96f8fb2ce9c26ae612a46fa26308147c4` |
| Working tree | Dirty; audit covers the current working tree, not only committed HEAD |
| Overall conformance | **Partial** |
| Overall implementation readiness | **Not Ready** |
| Reviewer-selection reuse | **Not reused** |
| Audit confidence | **High** |

## Implementation Target / Compare Base

- Target: current dirty working tree in `.chainworks/worktrees/cw-implement-proposal-078-durable-e49976db`.
- Compare base: implicit current implementation branch state; no PR base, merge-base, or explicit diff range was provided.
- Prior implementation audit reports R1-R3 exist beside the proposal and were used only as historical context. They were not reused as proposal-review routing artifacts.
- The proposal file itself is modified in the target worktree. This audit treats its current contents as the requested source of truth and does not edit it.

## Prior Proposal-Review Reuse

- Discovery result: no prior proposal-review artifacts were found for reviewer selection by the skill helper.
- Reuse state: **Not reused**.
- Rationale: only prior `IMPLEMENTATION_AUDIT` files were present; the audit workflow explicitly ignores those for reviewer selection unless asked otherwise.

## Selected Reviewers

| Reviewer | Why selected |
| --- | --- |
| `rust_arch_reviewer` | P078 changes Rust domain/db/engine/MCP/GraphQL boundaries and migration-backed persistence. |
| `rust_reliability_reviewer` | The core contract is durable intent, leases, CAS, recovery, retry blocking, and fail-closed reconciliation. |
| `api_contract_reviewer` | MCP, GraphQL, fixture, schema, and Swift DTO readback surfaces must stay aligned and read-only where promised. |
| `observability_rollout_reviewer` | P078 explicitly requires P084 rollout fixtures, metrics, logs, feature flags, hold conditions, and rollback disposition. |
| `macos_ui_reviewer` | Governed SwiftUI/macOS readback must remain projection-only while presenting operator next actions and diagnostics. |

## Rejected Close Alternatives

- `apple_arch_reviewer`: relevant to the Swift read boundary, but the implementation risk is narrower than the Rust control-plane and API/rollout surface.
- `apple_ux_reviewer`: recovery clarity matters, but the concrete review questions are covered by `macos_ui_reviewer` and P078's read-only UI contract.
- `rust_security_reviewer`: no new credential, auth, unsafe, or public security-sensitive boundary was the dominant P078 change in this audit slice.
- `rust_performance_reviewer`: no proposal performance target or benchmark envelope was central to the implementation readiness decision.
- `product_reviewer`: rollout/adoption metrics are in scope, but the blocking issues are operational proof and conformance rather than product strategy.

## Proposal State and Contract Summary

Proposal state: **Active**, inferred from the file remaining under `docs/proposals/` with no explicit retired/superseded/deprecated status field.

P078 promises a durable side-effect ledger for externally visible release operations. The key commitments are:

- persist side-effect intent before the first external operation executes;
- wire `git_commit`, `git_push`, `build_archive`, and `connect_upload`, while keeping `tag_create` and `artifact_publish` schema-supported only;
- enforce deterministic idempotency keys, request fingerprints, derivation-version conflict blocking, at-most-one external write per side-effect row, and CAS-protected settlement/reaper races;
- preserve compact SQLite rows and file-spooled evidence under P075 discipline;
- fail closed for retry, targeted retry, cancellation, scheduler/startup recovery, and ledger readback errors with `requires_effect_reconciliation`;
- expose command/control only through MCP `effects.*`; GraphQL and governed SwiftUI remain read-only projections;
- provide rollout contract coverage, negative/readback fixtures, metrics/log definitions, hold conditions, rollback disposition, and no-live-side-effect gate validation.

## Platform / Product Scope

- Apple scope: **macOS** governed SwiftUI read-only projection surface.
- Backend/service scope: **cross-stack service, worker, API, data, and rollout**.
- Product scope: operator recovery and release safety, not a new end-user feature.

## Primary Implementation Flows

1. Release executor creates a durable ledger intent before a wired external release operation runs, then settles via a CAS-protected lifecycle path.
2. Retry/cancel/requeue/startup recovery checks unresolved effects and blocks canonical mutation with a typed reconciliation envelope.
3. Watchdog/recovery transitions stale prepared/executing/externally observed effects and validates settled evidence summaries.
4. MCP operator tools inspect, reconcile, mark conflict/unrecoverable, or clear after manual verification without triggering external writes from readback paths.
5. GraphQL and SwiftUI expose unresolved side-effect lifecycle and next action diagnostics without adding command/control affordances.

## Proposal Fidelity Inventory

### Matches

- Ledger/domain/db/engine/MCP surfaces exist for P078 side effects, statuses, attempts, settlements, idempotency, leases, and dispositions.
- First wired release paths call `write_p078_side_effect_evidence_manifest` from release execution code (`control-plane/crates/engine/src/executor.rs:6869`, `:7108`, `:7379`, `:7655`).
- P075-style spooling is now used for the release receipt copy and evidence manifest (`control-plane/crates/engine/src/executor.rs:9111`, `:9131`, `:9223`, `:9308`).
- Watchdog startup recovery now covers prepared/external-observed crash windows and settled evidence validation (`control-plane/crates/engine/src/side_effects.rs:263`, `:365`, tests at `:1095`, `:1167`).
- MCP `effects.mark_conflict` is public and tested (`control-plane/crates/mcp-server/src/tools/effects.rs:252`, tests at `control-plane/crates/mcp-server/tests/proposal_078_effects_tools.rs:303`, `:316`).
- `effects.reconcile` now returns a file-backed report under the evidence root in tests (`control-plane/crates/mcp-server/tests/proposal_078_effects_tools.rs:371`).
- GraphQL exposes read-only side-effect summary/readback JSON and tests absence of command mutations (`control-plane/crates/graphql-server/src/schema.rs:623`, `:900`, `:1075`, `:2108`; run DTO at `control-plane/crates/graphql-server/src/types/run.rs:71`).
- Swift read boundary and UI card now present P078 readback and copy-only diagnostics (`Chainworks Forge/Support/P031ThinGraphQLReadBoundary.swift:1555`, `:4808`, `:4993`; `Chainworks Forge/Views/RunsHomeView.swift:197`, `:1330`, `:1378`).
- P078 gate now checks non-empty operator readback lanes, metrics fixture cardinality, forbidden Swift command controls, GraphQL/Swift markers, P075 spool markers, and new recovery/evidence markers (`scripts/test-gate.sh:6229`, `:6242`, `:6283`, `:6302`, `:6311`).

### Divergences

- The proposal names migration `046_p078_side_effect_ledger.sql`; the implementation uses `052_p078_side_effect_ledger.sql`.
- P078 evidence spooling lists `stdout.log`, `stderr.log`, `git-ls-remote.json`, `upload-readback.json`, `archive-summary.json`, `reconciliation-report.json`, and `evidence-manifest.json`. Current settlement evidence spooling materially proves release receipt + manifest, not the full committed file set.
- The proposal says missing/partial/checksum-mismatched evidence transitions affected records to `needs_reconciliation`; current settled evidence validation calls `mark_settled_evidence_failed` and transitions to `unrecoverable`.
- Ledger readback error handling exists, but the explicit "3 errors in 5 minutes opens a 10 minute fail-closed circuit breaker per call_site" behavior is not implemented/proven.
- Metrics coverage is marker/fixture-heavy and incomplete versus the proposal's full metric list.

### Ambiguities / Evidence Gaps

- No prior proposal-review routing artifacts were available, so reviewer selection was rerouted from proposal and implementation evidence.
- Swift presenter tests exist, but no executed macOS accessibility tree scan, snapshot, or UI runtime proof was found in this audit.
- The P078 gate is strong for targeted behavior, but rollout lint/readback proof still relies partly on source/fixture checks rather than a full end-to-end P084 validator.

## Requirement Summary

| ID | Requirement | Status |
| --- | --- | --- |
| REQ-001 | Add durable side-effect ledger data model, migration, repository validation, and lifecycle rows | Implemented |
| REQ-002 | Scope effect kinds and wire first release paths only | Implemented |
| REQ-003 | Deterministic idempotency, request fingerprint, derivation-version conflict blocking | Implemented |
| REQ-004 | Durable intent before execute, one external write per row, leases, and CAS settlement/reaper race control | Implemented |
| REQ-005 | Retry/cancel/recovery preflight fail-closed including ledger readback circuit breaker | Partially Implemented |
| REQ-006 | Watchdog/startup recovery for stale, crash-window, and settled-evidence states | Partially Implemented |
| REQ-007 | P075 file-spooled evidence contract with manifest-first recovery policy | Partially Implemented |
| REQ-008 | MCP-only reconciliation command/control with disposition idempotency and readback-only reconcile | Implemented |
| REQ-009 | GraphQL read-only projections and no side-effect command mutations | Implemented |
| REQ-010 | Governed SwiftUI read-only diagnostics, raw value fallback, copy/accessibility affordances | Partially Implemented |
| REQ-011 | P084 rollout contract, fixtures, metrics/logs, feature flags, rollback and hold conditions | Partially Implemented |
| REQ-012 | Proposal/p078 gates avoid live external side effects and credentials | Implemented |

## Detailed REQ Audit

### REQ-001 - Durable ledger data model

- Source: proposal `data_model`, `lifecycle_model`, `artifact_contracts` (`docs/proposals/078-...md:1`).
- Status: **Implemented**.
- Evidence: `control-plane/crates/domain/src/side_effect.rs`, `control-plane/crates/db/src/repos/side_effects.rs`, `control-plane/crates/db/migrations/052_p078_side_effect_ledger.sql`, db tests under `control-plane/crates/db/tests/proposal_078_side_effects.rs`.
- Notes: implementation uses migration `052`, while the proposal text still names `046`; this is a documentation/numbering divergence, not a functional absence.

### REQ-002 - Effect kinds and wired paths

- Source: proposal `scope.initial_effect_kinds`, `first_wired_paths`, `schema_supported_deferred_kinds`.
- Status: **Implemented**.
- Evidence: side-effect domain enums/raw preservation, release git/archive/connect paths, P078 release tests, `write_p078_side_effect_evidence_manifest` calls from executor release lanes.
- Notes: deferred `tag_create` and `artifact_publish` remain schema-supported, consistent with proposal scope.

### REQ-003 - Idempotency and fingerprint policy

- Source: proposal `idempotency_and_attempt_policy`.
- Status: **Implemented**.
- Evidence: db repository tests for idempotency uniqueness, request fingerprint conflicts, derivation-version conflict blocking, and disposition idempotency; `./scripts/test-gate.sh proposal-078` passed these focused suites.
- Notes: no contrary evidence found.

### REQ-004 - Intent-before-execute and CAS control

- Source: proposal `external_write_rule`, `reaper_and_cas_contract`, `acceptance_criteria`.
- Status: **Implemented**.
- Evidence: executor release paths create/settle through P078 coordinator, `run_with_lease_renewal` marker required by gate, `reaper_transition_cas` and settlement tests, `./scripts/test-gate.sh proposal-078` passing.
- Notes: this status covers the first wired paths under local/fake validation, not live external pushes/uploads.

### REQ-005 - Retry/cancel/recovery preflight and circuit breaker

- Source: proposal `retry_recovery_and_circuit_breakers`.
- Status: **Partially Implemented**.
- Evidence: unresolved preflight blocks and emits `side_effect_retry_block_total` (`control-plane/crates/engine/src/side_effects.rs:708`, `:744`); ledger readback errors emit `side_effect_ledger_readback_error_total` and return fail-closed errors (`:717`, `:722`, `:781`); heuristic guard flag constant exists (`control-plane/crates/domain/src/side_effect.rs:452`).
- Gap: no implementation or test evidence was found for the explicit call-site circuit breaker: repeated ledger readback errors 3 times within 5 minutes opening a 10 minute fail-closed breaker. Searches found no circuit state, expiry, per-call-site bucket, or test.

### REQ-006 - Watchdog/startup recovery

- Source: proposal `reaper_and_cas_contract`, `partial_evidence_policy`, acceptance criteria.
- Status: **Partially Implemented**.
- Evidence: watchdog pass handles stale/unresolved effects and settled evidence validation (`control-plane/crates/engine/src/side_effects.rs:263`, `:275`, `:365`); tests cover prepared/external crash windows and missing settled manifest fail-closed behavior (`:1095`, `:1167`).
- Gap: the missing settled evidence path transitions via `mark_settled_evidence_failed` to `unrecoverable` (`control-plane/crates/db/src/repos/side_effects.rs:1067`), while the proposal explicitly requires `needs_reconciliation` for missing/partial/checksum evidence.

### REQ-007 - P075 evidence spooling

- Source: proposal `evidence_spooling`, `non_goals`, acceptance criterion "P075 discipline is preserved".
- Status: **Partially Implemented**.
- Evidence: release receipt copy and evidence manifest use `db::evidence_spool::write_spool_file` and record spool refs (`control-plane/crates/engine/src/executor.rs:9131`, `:9223`, `:9308`); P078 gate checks `p075_write_spool_file` and `verify_p078_observed_evidence_summary` markers.
- Gap: implementation does not yet prove the complete proposal file contract for stdout/stderr/git-ls-remote/upload-readback/archive-summary/reconciliation evidence. It currently proves a narrower receipt-copy + manifest subset.

### REQ-008 - MCP-only reconciliation command/control

- Source: proposal `mcp_contract`, `graphql_contract.surface_policy`, `ux_ui_notes`.
- Status: **Implemented**.
- Evidence: public MCP tool mapping for `effects.reconcile`, `effects.mark_conflict`, `effects.mark_unrecoverable`, and `effects.clear_after_manual_verification` (`control-plane/crates/mcp-server/src/tools/mod.rs:99`, `:134`, `:224`); handlers in `tools/effects.rs:434`, `:486`, `:574`, `:667`; tests cover capability ids, public visibility, file-backed reconcile reports, and disposition behavior.
- Notes: the implementation goes beyond the proposal's `capability_tool_ids` list by making `effects.mark_conflict` public, which is consistent with the proposal acceptance criteria mentioning mark conflict.

### REQ-009 - GraphQL read-only projections

- Source: proposal `graphql_contract`.
- Status: **Implemented**.
- Evidence: `unresolved_side_effects` query and `GqlSideEffectSummary` (`control-plane/crates/graphql-server/src/schema.rs:623`, `:900`); run-level `side_effect_readback_json` (`:1075`, `control-plane/crates/graphql-server/src/types/run.rs:71`); tests assert forbidden side-effect mutations are absent (`control-plane/crates/graphql-server/src/schema.rs:2108`).
- Notes: `cargo check -p graphql-server -p mcp-server` passed on the audited tree.

### REQ-010 - Governed SwiftUI read-only diagnostics

- Source: proposal `ux_ui_notes.governed_macos_contract`, `swift_tests`.
- Status: **Partially Implemented**.
- Evidence: Swift presenter/card and tests exist (`Chainworks Forge/Support/P031ThinGraphQLReadBoundary.swift:1555`, `Chainworks Forge/Views/RunsHomeView.swift:1330`, `Chainworks ForgeTests/Proposal078OperatorReadbackTests.swift:8`, `:53`); P078 gate scans Swift sources for forbidden side-effect mutation controls (`scripts/test-gate.sh:6311`); `./scripts/test-gate.sh build` passed.
- Gap: proposal requires macOS accessibility/view hierarchy scans and snapshot-like coverage for absence of mutation affordances and status accessibility. The audit found source/unit coverage, not executed runtime UI/accessibility evidence.

### REQ-011 - Rollout contract, metrics, logs, flags, rollback

- Source: proposal `rollout_contract`, `metrics_and_logs`, `validation_plan`.
- Status: **Partially Implemented**.
- Evidence: readback fixture now carries non-empty side-effect readback and metric cardinality (`docs/evidence/rollout-contract/operator-readback/p078-full-surface.fixture.json:57`, `:61`); P078 gate validates readback lanes and selected metric markers (`scripts/test-gate.sh:6229`, `:6242`, `:6266`); runtime metric markers exist for intent/transition/retry block/settlement latency/unresolved/readback error (`control-plane/crates/engine/src/side_effects.rs:22`, `:168`, `:484`, `:622`, `:717`, `:744`).
- Gap: no implementation evidence found for several proposal metrics: `p078_release_side_effects_with_durable_intent_percent`, `side_effect_unresolved_age_seconds`, `startup_side_effect_recovery_total`, `startup_side_effect_recovery_duration_seconds`, `side_effect_evidence_spooled_bytes_total`, `side_effect_evidence_disk_bytes`, and `side_effect_prepare_denied_total`. Metric emission is structured tracing-style, not clearly integrated with a metrics backend.

### REQ-012 - No live external side effects in gate

- Source: proposal `validation_plan`, `rollout_contract.commands.commentary`.
- Status: **Implemented**.
- Evidence: `./scripts/test-gate.sh proposal-078` passed using local/fake tests; gate checks no live credentials/external side effects by policy and source markers; no live git push, App Store Connect upload, notarization, simulator run, or UI smoke run was observed.
- Notes: this audit also ran a build gate; the Xcode device passcode warning was non-fatal and the build succeeded.

## Reviewer / Lens Scorecard

| Lens | Score | Top risk | Confidence |
| --- | --- | --- | --- |
| Proposal conformance | Partial | Evidence/circuit/rollout commitments remain partial | High |
| Rust architecture | Mostly aligned | Migration number drift and broad modified tree need closeout cleanup | Medium |
| Rust reliability | Partial | Missing explicit readback circuit breaker and evidence status mismatch | High |
| API contract | Mostly aligned | GraphQL/MCP/Swift now align, but runtime UI proof is incomplete | High |
| Observability/rollout | Partial | Metrics and P084 proof remain narrower than proposal | High |
| macOS UI | Partial | Read-only UI exists, but runtime accessibility/view hierarchy proof is missing | Medium |
| Readiness | Not Ready | Major conformance gaps remain despite passing targeted gates | High |

## Routed Specialist Findings

### REL-001 - Settled evidence integrity failure transitions to the wrong unresolved terminal state

- Reviewer: `rust_reliability_reviewer`
- Severity: **Major**
- Confidence: **High**
- Related requirements: REQ-006, REQ-007
- Evidence: proposal `evidence_spooling.partial_evidence_policy`; `control-plane/crates/engine/src/side_effects.rs:275`, `:365`; `control-plane/crates/db/src/repos/side_effects.rs:1067`; test `control-plane/crates/engine/src/side_effects.rs:1167`.
- Why it matters: P078 says missing manifest/file/checksum/size evidence must transition affected records to `needs_reconciliation` with typed evidence error. The implementation intentionally fails closed, but records `unrecoverable`. That changes the operator recovery path and next-action semantics promised by the proposal.
- Recommended action: change settled evidence integrity failure to the proposal's `needs_reconciliation` state, or revise P078 before closeout if `unrecoverable` is now the intended lifecycle.
- Acceptance criteria: missing/partial/checksum evidence tests assert `NeedsReconciliation`, readback next action remains operator reconciliation, and P078 gate covers this exact state.

### REL-002 - Ledger readback circuit breaker is not implemented/proven

- Reviewer: `rust_reliability_reviewer`
- Severity: **Major**
- Confidence: **High**
- Related requirements: REQ-005
- Evidence: proposal `retry_recovery_and_circuit_breakers.preflight_behavior`; `control-plane/crates/engine/src/side_effects.rs:708`, `:717`, `:722`, `:781`; `control-plane/crates/domain/src/side_effect.rs:452`.
- Why it matters: fail-closed on one ledger readback error is useful, but P078 explicitly adds an operational circuit breaker after repeated readback failures for the same call site. Without the circuit state and expiry, the implementation does not meet the retry/recovery contract under persistent ledger failure.
- Recommended action: add per-call-site readback-error buckets, 5 minute/3 error threshold, 10 minute fail-closed open state, read-only heuristic fallback enforcement, metrics/logs, and tests for blocked canonical mutation while open.
- Acceptance criteria: tests demonstrate the breaker opens, blocks mutation, expires, and never permits heuristic fallback to authorize mutation.

### OPS-001 - Metrics and rollout proof remain narrower than P078's rollout contract

- Reviewer: `observability_rollout_reviewer`
- Severity: **Major**
- Confidence: **High**
- Related requirements: REQ-011
- Evidence: proposal `metrics_and_logs.metric_definitions`; metric markers in `control-plane/crates/engine/src/side_effects.rs:22`, `:168`, `:484`, `:622`, `:717`, `:744`; fixture/gate checks at `docs/evidence/rollout-contract/operator-readback/p078-full-surface.fixture.json:57`, `scripts/test-gate.sh:6242`, `:6266`.
- Why it matters: P078 makes rollout contract coverage part of acceptance. Current proof validates a useful subset, but several required metrics are absent and emission appears to be structured tracing, not a complete metrics/readback path. This weakens rollout hold/rollback decisions.
- Recommended action: implement or explicitly retire the missing metrics, wire them into the same observable surface used by rollout fixtures, and extend the gate to fail on missing definitions/readback for every P078 metric.
- Acceptance criteria: all proposal metric names are emitted or deliberately removed from the proposal; the P078/P084 gate validates names, labels/cardinality, and readback lane presence.

### OPS-002 - Evidence spooling proves P075 mechanics only for a narrow subset of the committed evidence files

- Reviewer: `observability_rollout_reviewer`
- Severity: **Major**
- Confidence: **High**
- Related requirements: REQ-007, REQ-011
- Evidence: proposal `evidence_spooling.files`; implementation `control-plane/crates/engine/src/executor.rs:9111`, `:9131`, `:9223`, `:9308`; gate marker `scripts/test-gate.sh:6283`.
- Why it matters: P078 explicitly names the side-effect evidence file set and manifest-first verification policy. The new P075 `write_spool_file` usage is a real improvement, but it does not yet prove stdout/stderr/readback/archive/reconciliation evidence paths, size/checksum coverage, or recovery behavior for those files.
- Recommended action: either spool and verify the full P078 evidence file set, or revise the proposal to narrow the first implementation's evidence contract to release receipt copy + manifest and add a follow-up proposal for the rest.
- Acceptance criteria: tests cover every committed evidence file kind and manifest reference, including missing, partial, checksum, and size mismatch behavior.

### UI-001 - Swift read-only diagnostics exist, but runtime accessibility/view hierarchy proof is still missing

- Reviewer: `macos_ui_reviewer`
- Severity: **Minor**
- Confidence: **Medium**
- Related requirements: REQ-010
- Evidence: Swift source/tests at `Chainworks Forge/Support/P031ThinGraphQLReadBoundary.swift:1555`, `Chainworks Forge/Views/RunsHomeView.swift:1330`, `Chainworks ForgeTests/Proposal078OperatorReadbackTests.swift:8`, `:53`; P078 forbidden-control scan at `scripts/test-gate.sh:6311`.
- Why it matters: P078 specifically calls for accessibility tree/view hierarchy scans to ensure no governed SwiftUI mutation affordances exist and unresolved rows expose the required combined accessibility information. Source scans and presenter tests are not equivalent to runtime accessibility proof.
- Recommended action: add a focused macOS UI/accessibility test or recorded view hierarchy proof for the P078 card/row states.
- Acceptance criteria: test proof shows the card/row exposes copy-only diagnostics, combined accessibility content, stable identifiers, and no reconcile/retry/clear/push/upload controls.

### API-001 - Proposal and implementation disagree on migration numbering

- Reviewer: `api_contract_reviewer`
- Severity: **Minor**
- Confidence: **High**
- Related requirements: REQ-001
- Evidence: proposal `data_model.migration` names `046_p078_side_effect_ledger.sql`; worktree status shows rename to `control-plane/crates/db/migrations/052_p078_side_effect_ledger.sql`.
- Why it matters: the implementation can be functionally correct, but closeout cannot promote contradictory reference truth. Migration numbering drift creates avoidable confusion for operators and future audits.
- Recommended action: update the proposal/reference closeout trail to record the final migration number, or add a compatibility note explaining the renumbering.
- Acceptance criteria: proposal closeout/reference docs and test-gate references use the final migration number consistently.

## Readiness Checklist

| Check | Status | Evidence |
| --- | --- | --- |
| Same-tree P078 canonical gate | Pass | `./scripts/test-gate.sh proposal-078` passed |
| Same-tree Swift/build gate | Pass | `./scripts/test-gate.sh build` passed |
| GraphQL/MCP compile path | Pass | `CARGO_TARGET_DIR=target/proposal-078-audit-r4-check cargo check -p graphql-server -p mcp-server` passed |
| Whitespace/diff sanity | Pass | `git diff --check` on audited paths returned clean |
| Core durable ledger flow | Pass for targeted fake/local tests | P078 gate Rust domain/db/engine/release/MCP tests passed |
| Recovery/evidence flow | Partial | Crash-window tests pass; evidence state mismatch remains |
| GraphQL read-only contract | Pass | Read-only projections compile and mutation absence tests exist |
| Swift read-only UI | Partial | Presenter/card/tests exist; runtime accessibility scan not run/found |
| Rollout/metrics | Partial | Fixture/gate markers exist; full metric contract incomplete |
| Live external side-effect exclusion | Pass | P078 gate uses local/fake validation; no live side effects observed |
| Closeout readiness | Fail | Major REL/OPS findings remain |

## Verification Log

- `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/report_path.py ...` -> selected R4 path.
- `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/discover_prior_review.py ...` -> no prior proposal-review artifacts found.
- `git branch --show-current` -> `cw/implement-proposal-078-durable/e49976db`.
- `git rev-parse HEAD` -> `8fb2afa96f8fb2ce9c26ae612a46fa26308147c4`.
- `git status --short` -> dirty worktree with modified Rust/Swift/docs/gate files and untracked P078 Swift tests plus prior audit reports.
- `./scripts/test-gate.sh proposal-078` -> passed; included domain, db, engine, release, MCP, fixture, marker, and forbidden-control checks.
- `./scripts/test-gate.sh build` -> passed; Xcode emitted a non-fatal device passcode warning.
- `CARGO_TARGET_DIR=target/proposal-078-audit-r4-check cargo check -p graphql-server -p mcp-server` -> passed with existing dead-code warnings.
- `git diff --check -- control-plane "Chainworks Forge" "Chainworks ForgeTests" scripts/test-gate.sh docs/evidence/rollout-contract/operator-readback/p078-full-surface.fixture.json` -> passed.
- Focused searches inspected P078 circuit breaker, metrics, evidence spooling, GraphQL readback, MCP tools, Swift presenter/card, and tests.

## Final Verdict

**Overall Conformance: Partial.**

The R4 implementation materially improves over the previous state: Swift readback is no longer DTO-only, GraphQL/MCP compile, P078 targeted gate passes, P075-style spool writes are now used for the receipt/manifest path, watchdog recovery covers more crash windows, and readback fixtures now carry non-empty operator action data.

**Overall Implementation Readiness: Not Ready.**

The implementation should not be closed out as P078 complete until the remaining proposal-level gaps are resolved: evidence integrity must use the promised reconciliation state or the proposal must change, the ledger readback circuit breaker must be implemented/proven, the committed evidence file set must be narrowed or fully spooled/verified, rollout metrics must match the proposal, and the macOS read-only UI contract needs runtime accessibility/view hierarchy proof.

## Recommended Next Actions

1. Fix the settled evidence integrity state mismatch (`unrecoverable` vs `needs_reconciliation`) and add tests/gate coverage.
2. Implement the ledger readback circuit breaker with call-site thresholding, expiry, read-only heuristic fallback, metrics, and tests.
3. Decide whether P078's evidence file list is still the contract; either implement the full list or revise the proposal before closeout.
4. Complete rollout metric/readback coverage for every metric named in P078 or explicitly retire missing metrics from the proposal.
5. Add focused macOS accessibility/view hierarchy proof for the P078 read-only card/row state.
