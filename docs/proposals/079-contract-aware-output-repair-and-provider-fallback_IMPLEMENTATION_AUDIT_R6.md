# P079 Implementation Audit R6 - Contract-Aware Output Repair and Provider Fallback

## Metadata

- Proposal: `docs/proposals/079-contract-aware-output-repair-and-provider-fallback.md`
- Proposal ID: `P079`
- Proposal revision: `p079-contract-aware-output-repair-and-provider-fallback-r5`
- Audit report: `docs/proposals/079-contract-aware-output-repair-and-provider-fallback_IMPLEMENTATION_AUDIT_R6.md`
- Audit date: 2026-06-20
- Auditor: Codex
- Repository: `/Users/user/Documents/Chainworks Forge`
- Head audited: `0e6482c82b588b74a76294a225e68286bfe37fa4`
- Compare base: proposal contract in current workspace versus current repository implementation
- Prior review reuse: Not reused. `discover_prior_review.py` returned no review artifacts for this proposal.
- Worktree note: the workspace was already dirty before this audit, including P079-relevant modified files and existing R2-R5 audit reports. This audit did not revert or normalize those changes.

## Verdict

- Overall Conformance: Not Implemented
- Overall Readiness: Not Ready
- Audit Confidence: High for the primary blockers; Medium for full gate status because the full P079 and Swift gates were killed before producing output in this environment.

P079 has a substantial schema, persistence, readback, metrics-inventory, and deterministic fixture repair substrate. It does not yet implement the proposal-level production behavior. Production same-session repair is deliberately fail-closed for all production provider families, transcript/provider-envelope recovery has no accepted recovery path, controlled provider fallback dispatch is not wired, and the macOS inspector UI commitments are not integrated into the app shell.

## Selected Reviewers

- `rust_arch_reviewer`: runtime orchestration, proposal architecture, fallback dispatch, frozen policy shape.
- `rust_reliability_reviewer`: lease ordering, restart/sweep semantics, atomic settlement, failure recovery.
- `rust_security_reviewer`: permission posture, filesystem boundary, redaction, non-operator readback privacy.
- `api_contract_reviewer`: GraphQL/MCP/Swift DTO/readback contract and compatibility defaults.
- `macos_ui_reviewer`: operator-facing read-only inspector and presentation requirements.

Rejected alternatives:

- `observability_rollout_reviewer`: not selected due reviewer cap; rollout and metrics were still reviewed as readiness blockers.
- `performance_reviewer`: bounded transcript and lease paths were checked, but no performance success claim is possible while the main behavior is incomplete.
- `apple_ux_reviewer`: covered through the macOS UI reviewer because the current issue is missing product integration, not fine-grained interaction polish.

## Proposal Contract

The proposal requires the output-failure lane to:

- Try at most one same-session repair turn for eligible missing, empty, invalid, or mode-mismatched required outputs.
- Recover valid output already present in the current invocation transcript or provider envelope with transport attribution.
- Allow at most one controlled provider fallback after repair or recovery is unavailable or unsuccessful.
- Preserve declared output contracts, canonical paths, source-generation checks, existing settlement semantics, and no-broad-scan boundaries.
- Expose typed repair, recovery, fallback, lease, budget, and final-settlement evidence through run reports, MCP, GraphQL, and Swift readback.
- Keep the macOS app a passive operator shell with read-only diagnostic status and stable presentation categories.
- Prove behavior with deterministic local fixture tests and the `proposal-079|p079` gate.

## Platform And Product Scope

Audited scope includes:

- Rust control-plane domain, DB migrations/repos, metrics, engine executor, ACP transport, GraphQL, and MCP report surfaces.
- Swift readback DTO, presenter, and readback tests.
- Reference docs and test-gate definitions.

Out of scope:

- Live provider execution, networked provider calls, or remote UI test hosts.
- Unrelated dirty-worktree changes for other proposals.

## Primary Flows

1. Eligible agent output contract failure
   - Expected: transcript/provider-envelope recovery first, then same-session repair, then fallback only if needed.
   - Actual: eligibility and fixture repair substrate exist; production same-session repair is fail-closed, transcript recovery cannot accept, fallback dispatch is absent.

2. Operator reads diagnostic evidence
   - Expected: GraphQL/MCP/run report/Swift/macOS inspector show typed repair status, lease, budget, permission, plan evidence, and fallback fields.
   - Actual: GraphQL/MCP/Swift DTO and presenter coverage exists; actual macOS inspector integration is missing.

3. Controlled provider fallback
   - Expected: compile frozen fallback policy, build redacted context packet, dispatch fallback child, link parent/child, settle parent after child validation.
   - Actual: schemas, constants, tables, and readback fields exist; runtime fallback policy parsing and dispatch are not wired.

4. Restart, sweep, lease, and budget recovery
   - Expected: crash-consistent repair/fallback leases, TTL sweeps, source-generation supersession, and single-flight behavior.
   - Actual: repair lease repository behavior and atomic settlement are tested; full projection rebuild/sweep and fallback lease execution remain deferred.

5. Security/privacy boundary
   - Expected: no release lanes, no durable side-effect fallback, no broad scanning, no provider self-declared identity trust, redacted non-operator readback.
   - Actual: the implemented path is conservative and fail-closed, but production repair is unavailable until a real provider sandbox exists.

## Fidelity Summary

Implemented substrate:

- `control-plane/crates/db/migrations/095_p079_output_contract_repair.sql` creates repair events, leases, and fallback parent-link tables.
- `control-plane/crates/domain/src/output_contract_repair.rs` defines schema constants, flags, enums, evidence types, lease states, fallback result types, and presentation categories.
- `control-plane/crates/db/src/repos/output_contract_repair.rs` persists repair events, leases, stale/fresh projection status, atomic terminal settlement, and fallback parent-link rows.
- `control-plane/crates/db/src/metrics.rs` declares P079 metric names and records repair lifecycle metrics in the DB repository path.
- `control-plane/crates/engine/src/executor.rs` has eligibility handling, repair prompt creation, plan evidence redaction, deterministic fixture same-session repair, atomic materialization ordering, and fail-closed production provider checks.
- `control-plane/crates/acp/src/transport.rs` implements P079 permission-decision capture, canonical-path checks, redaction, and unsafe-continuation classification for the ACP repair posture.
- GraphQL/MCP readback surfaces expose typed P079 evidence and sanitize sensitive fallback/provider details for non-operator callers.
- Swift DTO/presenter/readback tests decode old-run nil fields, unknown enum cases, stable identity, permission decisions, path sanitization, and presentation categories.

Major divergences:

- Production same-session repair is intentionally blocked for `codex`, `claude`, `gemini`, `junie`, and `auggie`.
- Transcript/provider-envelope recovery is intentionally fail-closed with `attribution_not_verified`.
- Controlled provider fallback is represented in schemas and docs but not implemented as a runtime path.
- The macOS inspector UI is not wired to the P079 presenter or evidence model.
- The reference gate itself documents P079 as partial acceptance, with accepted recovery, fallback dispatch, full projection sweep, macOS inspector UI, metric emission, and required reference docs deferred.

## Requirement Audit

| Req | Proposal requirement | Status | Evidence |
| --- | --- | --- | --- |
| REQ-001 | Trigger after eligible normal output contract failure and preserve settlement authority | Partially Implemented | Engine has output-failure branch and eligibility checks, but repair is production fail-closed. |
| REQ-002 | At most one same-session repair turn | Partially Implemented | Lease/budget and fixture repair exist; production providers are blocked by advisory-posture guard. |
| REQ-003 | Narrow repair prompt with canonical paths, previous output, failure summary, and no broad rerun | Implemented for fixture path | `output_contract_repair_prompt` and canonical path binding are present in executor tests and implementation. |
| REQ-004 | Server-enforced repair permission posture | Partially Implemented | ACP posture checks exist, but executor blocks production providers because current provider runtimes are advisory-only. |
| REQ-005 | Transcript/provider-envelope recovery with verified attribution | Partially Implemented | Bounds and evidence structure exist; recovery returns `attribution_not_verified` and no settlement. |
| REQ-006 | Plan evidence capture/redaction for Junie/mode mismatch | Partially Implemented | Executor copies and redacts plan evidence; retention/complete production path not fully proven here. |
| REQ-007 | Controlled provider fallback with frozen policy and packet | Missing | Docs state fallback dispatch and YAML `output_repair_policies` parsing remain deferred. Runtime search found schema/readback only. |
| REQ-008 | Canonical path binding and crash-consistent materialization before final settlement | Implemented for repair lane | Engine stages only valid repaired outputs and materializes before final recovered settlement. |
| REQ-009 | Typed GraphQL/MCP/run-report/Swift readback | Partially Implemented | GraphQL/MCP/Swift DTOs exist; actual product UI and deferred fallback/recovery states are incomplete. |
| REQ-010 | SQLite persistence, leases, fallback links, projection rebuild/sweep | Partially Implemented | Migration/repos/tests cover core rows and leases; full projection rebuild and recovery sweep are documented deferred. |
| REQ-011 | Reliability semantics for shutdown, TTL, cancellation, supersession, fallback child restart | Partially Implemented | Repair lease behavior is tested; fallback child lifecycle is absent. |
| REQ-012 | Metrics, flags, rollback, rollout docs | Partially Implemented | Flags and metric inventory exist; full operational metric emission and required docs are deferred. |
| REQ-013 | macOS read-only inspector UI with status chips, details, copy/reveal, accessibility | Missing | Swift DTO/presenter/tests exist, but `RunInspectorView` does not consume P079 evidence. |
| REQ-014 | Deterministic local acceptance gate | Partially Implemented | Domain/DB slices pass; `proposal-079` and `p079-swift-readback` were killed with exit 137 in this environment; docs define the gate as partial acceptance. |
| REQ-015 | Non-goals and exclusions: no approvals/conflicts inference, release fallback, live providers, broad scan | Partially Implemented | Implemented repair path is conservative; missing fallback/recovery means full exclusion behavior is not exercised end to end. |

Roll-up: because REQ-007 and REQ-013 are Missing, and multiple core requirements are only partial, the proposal is Not Implemented under the audit rubric.

## Coverage Matrix

| Lens | Coverage | Result |
| --- | --- | --- |
| Architecture | Engine, domain, DB, fallback policy/fallback packet references, docs | Blocked by missing fallback dispatch and production repair. |
| Reliability | DB leases, atomic settlement, repair budget tests, restart/sweep docs | Partial; fallback lifecycle and full sweep remain deferred. |
| Security | Sensitive-diff scan, ACP permission posture, fail-closed provider support, redaction | Conservative posture is correct; production repair cannot be enabled without enforceable sandboxing. |
| API contract | GraphQL/MCP fields, Swift DTO, v1 compatibility defaults | Partial; readback exists but represents deferred paths rather than working behavior. |
| macOS UI | Swift presenter/tests and app view search | Missing product integration in `RunInspectorView`. |
| Observability/rollout | Metrics constants, DB repo metric tests, docs/test-gates | Partial; full operational metric emission and required P079 reference docs deferred. |
| Performance/resource limits | Transcript bound constants and fixture test path | Not a readiness blocker by itself, but full production path cannot be assessed. |

## Security Scan

`security_sensitive_diff.py` triggered because P079 touches auth, filesystem/subprocess boundaries, parser boundaries, public ingress, secrets/redaction/privacy, resource limits, and dependency-sensitive files. A focused pass over the P079-relevant surfaces found no reason to enable production repair today; the implementation correctly fails closed where the provider runtime cannot enforce filesystem/tool/network restrictions.

Residual security risk is readiness-related: if production same-session repair were enabled by bypassing `CHAINWORKS_P079_ACCEPT_ADVISORY_REPAIR_POSTURE` without a real sandbox, the ACP permission interceptor would not prevent direct provider file writes.

## Findings

### P079-ARCH-001 - Production same-session repair is intentionally fail-closed

- Severity: Major
- Expected: eligible output contract failures can receive at most one same-session repair turn in production.
- Actual: executor blocks repair dispatch when permission enforcement is advisory; `p079_provider_supports_enforced_permissions` returns true only for `fixture`.
- Evidence:
  - `control-plane/crates/engine/src/executor.rs:11703` documents the fail-closed guard and blocks advisory providers.
  - `control-plane/crates/engine/src/executor.rs:19691` states production providers use full-access or bypass permissions and are not enforceable.
  - `docs/reference/test-gates.md:2051` says production same-session repair is fail-closed for advisory-only providers.
- Impact: the central same-session repair promise is fixture-only, not production-ready.
- Required action: implement an enforceable provider sandbox or revise P079 to scope production repair out of this proposal.

### P079-REL-002 - Transcript/provider-envelope recovery cannot accept recovered output

- Severity: Major
- Expected: valid current-invocation output in transcript or provider envelope is recovered before repair/fallback.
- Actual: the transcript recovery function returns `attribution_not_verified` and no settlement after scanning; provider-envelope recovery is not wired as an accepted lane.
- Evidence:
  - `control-plane/crates/engine/src/executor.rs:20736` returns fail-closed because transport-derived attribution is not wired.
  - `docs/reference/output-contracts-failure-evidence-and-recovery.md:773` lists recovery as a goal but marks it deferred at line 774.
  - `docs/reference/test-gates.md:2050` lists accepted transcript/provider-envelope recovery as deferred.
- Impact: P079 may still block or attempt repair/fallback even when valid current-turn output exists, contrary to the invocation order.
- Required action: wire transport-owned chunk attribution and provider-envelope parsing, then accept only current-invocation, current-agent, contract-valid payloads.

### P079-ARCH-003 - Controlled provider fallback dispatch is missing

- Severity: Major
- Expected: after repair/recovery fails or is unavailable, the system can perform one controlled fallback attempt using frozen policy and redacted context packet.
- Actual: fallback schema, DB rows, GraphQL/MCP readback fields, and parent-link repository helpers exist, but runtime policy parsing, packet assembly, child dispatch, and parent settlement are absent.
- Evidence:
  - `docs/reference/output-contracts-failure-evidence-and-recovery.md:793` says controlled fallback dispatch remains deferred.
  - `docs/reference/output-contracts-failure-evidence-and-recovery.md:794` says YAML `output_repair_policies` parsing is not wired.
  - `docs/reference/test-gates.md:2050` lists controlled provider fallback dispatch as deferred.
  - `control-plane/crates/engine/src/executor.rs:11148` records repair events with `provider_fallback_json: None` in the P079 repair path.
- Impact: the proposal's third core behavior is absent; fallback-specific reliability, security, metric, and readback requirements cannot be proven.
- Required action: compile frozen fallback policy into the run snapshot, create a bounded/redacted packet, dispatch a linked child execution under a fallback lease, validate child outputs, and atomically settle parent/child evidence.

### P079-UI-004 - macOS inspector UI is not integrated

- Severity: Major
- Expected: the macOS operator shell presents P079 read-only diagnostics with compact status, details, copy/reveal affordances, accessibility identifiers, and stale-state handling.
- Actual: `OutputContractRepairEvidence` and `OutputContractRepairPresenter` exist, with decode/presenter tests, but app view search only finds the presenter and tests; `RunInspectorView` is not wired to P079 evidence.
- Evidence:
  - `Chainworks Forge/Engine/Readback/OutputContractRepair/OutputContractRepairPresenter.swift:34` defines the presenter.
  - `Chainworks ForgeTests/Proposal079ContractRepairReadbackTests.swift:34` exercises DTO/presenter decoding.
  - `Chainworks Forge/Views/RunInspectorView.swift:9` is the current inspector entry point, and search found no P079 presenter consumption in app views.
  - `docs/reference/test-gates.md:2050` lists the macOS inspector UI as deferred.
- Impact: operator-facing acceptance criteria are not met even though readback DTOs compile independently.
- Required action: add the read-only P079 section to the inspector, bind it to the DTO/presenter, and cover status chips, stale state, accessibility identifiers, and copy/reveal actions.

### P079-READY-005 - The canonical gate is partial and was not fully verifiable here

- Severity: Major
- Expected: `./scripts/test-gate.sh proposal-079` proves the proposal acceptance criteria.
- Actual: docs define the gate as partial acceptance, and both `proposal-079` and `p079-swift-readback` exited 137 before output in this environment. Rust domain and DB slices passed, but engine/ACP and Swift gate completion were not verified.
- Evidence:
  - `docs/reference/test-gates.md:2029` names the gate a partial-acceptance proof.
  - `docs/reference/test-gates.md:2050` lists accepted recovery, fallback dispatch, full projection sweep, macOS inspector UI, operational metric emission, and required reference docs as deferred.
  - Local verification log below records the exit 137 and targeted passing slices.
- Impact: the gate cannot be used as a full readiness proof for this proposal.
- Required action: either redefine P079 as a partial substrate proposal with follow-up IDs, or complete the missing behaviors and expand the gate to prove them.

## Scorecard

| Area | Score | Rationale |
| --- | ---: | --- |
| Contract fidelity | 2/5 | Core substrate exists, but production repair, accepted recovery, fallback, and UI are missing. |
| Runtime architecture | 2/5 | Repair path structure is present; fallback architecture remains schema-only. |
| Reliability | 3/5 | DB leases and atomic settlement are strong for repair; fallback and full sweep are missing. |
| Security posture | 3/5 | Fail-closed posture is correct; it also means production behavior is not delivered. |
| API/readback | 3/5 | Typed GraphQL/MCP/Swift DTOs exist; deferred lanes are not live. |
| macOS product UI | 1/5 | Presenter/tests exist but no integrated inspector surface. |
| Test evidence | 2/5 | Domain/DB slices pass; full gate/Swift gate not verified and documented partial. |
| Rollout readiness | 1/5 | Feature is not production-ready; docs identify major deferred scope. |

## Residual Scope And Ownership

| Residual scope | Suggested owner | Blocking status |
| --- | --- | --- |
| Enforceable same-session repair sandbox for production providers | Runtime/provider adapter owner | Blocks production repair. |
| Transcript/provider-envelope attribution and accepted recovery settlement | Engine plus ACP transport owner | Blocks recovery promise. |
| Frozen fallback policy, packet assembly, child dispatch, and parent-child settlement | Engine/workflow owner | Blocks controlled fallback. |
| MacOS inspector integration for P079 evidence | SwiftUI operator shell owner | Blocks product acceptance. |
| Full projection rebuild/sweep and fallback lease recovery | DB/reliability owner | Blocks reliability acceptance. |
| Required reference docs: repair prompt template, recovery attribution, adapter idempotency | Docs/runtime owners | Blocks closeout readiness. |
| Full P079 gate expansion and stable Swift gate execution | Test infrastructure owner | Blocks final proof. |

## Readiness Checklist

- [x] Proposal contract parsed from current proposal file.
- [x] Versioned report path selected as R6.
- [x] Static P079 implementation surfaces inspected.
- [x] Security-sensitive diff scan considered.
- [x] Rust domain P079 tests passed.
- [x] Rust DB P079 metric, repo, and integration tests passed.
- [ ] Full `proposal-079|p079` gate passed.
- [ ] Swift `p079-swift-readback` gate passed in this environment.
- [ ] Production same-session repair implemented.
- [ ] Accepted transcript/provider-envelope recovery implemented.
- [ ] Controlled provider fallback implemented.
- [ ] MacOS inspector UI implemented.
- [ ] Required P079 reference docs complete.

## Verification Log

- `./scripts/test-gate.sh proposal-079`
  - Result: Inconclusive/failed in this environment. Process exited 137 before producing output.
- `./scripts/test-gate.sh p079-swift-readback`
  - Result: Inconclusive/failed in this environment. Process exited 137 before producing output.
- `cd control-plane && CARGO_TARGET_DIR=target/p079-audit cargo test -p domain output_contract_repair -- --nocapture`
  - Result: Pass. 9 P079 domain tests passed.
- `cd control-plane && CARGO_TARGET_DIR=target/p079-audit cargo test -p db proposal_079_required_metric_names_are_declared_and_recordable -- --nocapture`
  - Result: Pass. 1 P079 metrics inventory test passed.
- `cd control-plane && CARGO_TARGET_DIR=target/p079-audit cargo test -p db output_contract_repair -- --nocapture`
  - Result: Pass. 14 DB repository tests passed.
- `cd control-plane && CARGO_TARGET_DIR=target/p079-audit cargo test -p db --test proposal_079_output_contract_repair -- --nocapture`
  - Result: Pass. 6 DB integration tests passed.
- `cd control-plane && CARGO_TARGET_DIR=target/p079-audit cargo test -p engine --lib p079 -- --nocapture && ... acp ...`
  - Result: Inconclusive. The command was interrupted after extended silent compile time; no test result was produced.
- Static path presence check for P079 migration, refs, domain, engine, metrics, repo, Swift DTO/presenter/tests
  - Result: Pass.
- Filesystem preflight
  - Result: about 7.7 GiB free on the workspace volume, which may have contributed to gate instability.
- `git diff --check -- docs/proposals/079-contract-aware-output-repair-and-provider-fallback_IMPLEMENTATION_AUDIT_R6.md`
  - Result: Pass.
- `LC_ALL=C grep -n '[^ -~]' docs/proposals/079-contract-aware-output-repair-and-provider-fallback_IMPLEMENTATION_AUDIT_R6.md`
  - Result: Pass. No non-ASCII characters found.

## Final Recommendation

Do not close out P079 as implemented.

Recommended next action is to split the current state explicitly:

1. Mark the existing implementation as a partial P079 substrate covering schema, persistence, repair evidence, fail-closed posture, deterministic fixture repair, and Swift readback DTOs.
2. File or retain blocking follow-ups for production repair sandboxing, accepted transcript/provider-envelope recovery, controlled provider fallback, macOS inspector UI, full sweep/rebuild, required reference docs, and full gate expansion.
3. Re-run the audit only after the gate proves the proposal-level user flows rather than the current partial substrate.
