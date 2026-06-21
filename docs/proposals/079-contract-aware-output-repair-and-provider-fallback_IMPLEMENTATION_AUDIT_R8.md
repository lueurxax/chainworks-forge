# Proposal 079 Implementation Audit R8

## Metadata

- Proposal: `docs/proposals/079-contract-aware-output-repair-and-provider-fallback.md`
- Proposal revision: `p079-contract-aware-output-repair-and-provider-fallback-r5`
- Audit report: `docs/proposals/079-contract-aware-output-repair-and-provider-fallback_IMPLEMENTATION_AUDIT_R8.md`
- Audit date: 2026-06-20
- Audited revision: `0e6482c82b588b74a76294a225e68286bfe37fa4`
- Worktree state: dirty; this audit evaluates the current working tree and cites uncommitted P079 implementation/reference/test-gate files where present.
- Overall conformance verdict: Not Implemented
- Readiness verdict: Not Ready
- Reviewer reuse: Not reused. Prior-review discovery returned no reusable reviewer artifacts for this proposal audit pass.
- Audit confidence: High for the current partial implementation boundary; medium for live production-provider behavior because the canonical P079 gate remains fixture/local only.

## Gate Result

Current canonical gate result:

- `./scripts/test-gate.sh proposal-079` passed on 2026-06-20.
- The gate includes Rust schema/repo/engine/ACP/GraphQL/MCP slices and `p079-swift-readback`.
- Swift readback executed 27 `Proposal079ContractRepairReadbackTests` and passed.

This is not a full proposal acceptance signal. The repository reference explicitly defines `proposal-079|p079` as a "partial-acceptance gate" and lists controlled provider fallback dispatch, full projection rebuild and recovery sweep, provider-fallback rollout metric readback, and required reference docs as deferred (`docs/reference/test-gates.md:2027`, `docs/reference/test-gates.md:2050`).

## Selected Reviewers

- `chainworks_execution_truth_reviewer` - mandatory for run/stage/agent/execution truth, output settlement, repair/fallback state, and operator readback ownership.
- `rust_reliability_reviewer` - leases, crash consistency, replay, sweep/reclamation, and fail-closed runtime behavior.
- `api_contract_reviewer` - SQLite, GraphQL, MCP, run-report, Swift DTO, enum/nullability, and schema evolution contracts.
- `rust_security_reviewer` - transcript parsing, permission posture, filesystem boundaries, redaction, principal binding, and fallback packet handling.
- `observability_rollout_reviewer` - gates, rollout metrics, operator evidence, deferred lanes, and operational closeout readiness.

Rejected due the hard cap:

- `rust_arch_reviewer` - relevant, but architecture coverage is included through Chainworks execution-truth plus reliability/API/security.
- `macos_ui_reviewer`, `apple_ux_reviewer`, and `apple_arch_reviewer` - relevant to the Swift inspector and DTO layer, but the backend proposal blockers dominate readiness. Swift evidence was still inspected directly.
- `rust_performance_reviewer` - resource bounds are relevant, but no independent performance acceptance claim is central to readiness; DoS/resource limits are covered under security and reliability.

## Proposal Contract Summary

P079 promises a bounded output-contract recovery lane after normal output settlement fails. The key in-scope promises are:

- at most one same-session corrective repair turn for eligible missing, empty, invalid, or mode-mismatched required outputs;
- recovery of attributable current-invocation transcript/provider-envelope output;
- at most one controlled provider fallback attempt from frozen fallback policy;
- declared output contracts, canonical paths, source-generation claims, and existing settlement as the only artifact truth;
- no release/publish/upload/distribution/git-push side-effect lanes;
- typed evidence through run reports, MCP, GraphQL, and Swift DTO/readback;
- deterministic local/fixture gates, no live provider dependency.

The proposal also requires published reference docs for the repair prompt template, recovery attribution, and adapter idempotency (`docs/proposals/079-contract-aware-output-repair-and-provider-fallback.md:8`).

## Implementation Summary

Implemented or substantially wired:

- SQLite migration creates `output_contract_repair_events`, `output_contract_repair_leases`, and `output_contract_repair_fallback_parent_links`, with status/settlement enums, budget flags, projection fields, lease state, and fallback parent-link columns (`control-plane/crates/db/migrations/095_p079_output_contract_repair.sql:9`, `control-plane/crates/db/migrations/095_p079_output_contract_repair.sql:117`, `control-plane/crates/db/migrations/095_p079_output_contract_repair.sql:187`).
- Domain schema, presentation enums, transcript recovery, provider fallback DTO structs, evidence schema constants, and feature flag constants exist (`control-plane/crates/domain/src/output_contract_repair.rs:519`, `control-plane/crates/domain/src/output_contract_repair.rs:557`, `control-plane/crates/domain/src/output_contract_repair.rs:582`, `control-plane/crates/domain/src/output_contract_repair.rs:718`).
- Repair event plus reserved lease insertion is atomic; prompt-sent transition precedes dispatch; terminal event/lease settlement is transactional (`control-plane/crates/db/src/repos/output_contract_repair.rs:515`, `control-plane/crates/db/src/repos/output_contract_repair.rs:647`, `control-plane/crates/db/src/repos/output_contract_repair.rs:724`).
- Expired lease enumeration, reclamation, and fallback-link repository functions exist (`control-plane/crates/db/src/repos/output_contract_repair.rs:873`, `control-plane/crates/db/src/repos/output_contract_repair.rs:941`, `control-plane/crates/db/src/repos/output_contract_repair.rs:1052`).
- Engine inserts P079 evidence after validation failure, persists transcript recovery evidence, records required outputs, and creates a repair lease (`control-plane/crates/engine/src/executor.rs:11241`, `control-plane/crates/engine/src/executor.rs:11337`, `control-plane/crates/engine/src/executor.rs:11444`).
- Plan evidence collection/redaction is wired for Junie diagnostic evidence with meta-root containment and protected directory creation (`control-plane/crates/engine/src/executor.rs:11580`, `control-plane/crates/engine/src/executor.rs:11609`, `control-plane/crates/engine/src/executor.rs:11659`).
- Repair prompt assembly exists with template version, runtime IDs, failed outputs, caps, and narrow-output instructions (`control-plane/crates/engine/src/executor.rs:11710`, `control-plane/crates/engine/src/executor.rs:21085`).
- Transcript recovery is bounded, feature-gated, and only accepts transport-attributed provider-envelope artifacts (`control-plane/crates/engine/src/executor.rs:20938`, `control-plane/crates/engine/src/executor.rs:21006`, `control-plane/crates/engine/src/executor.rs:21053`).
- GraphQL typed readback exists with non-null nested defaults and redaction of operator-only details (`control-plane/crates/graphql-server/src/types/stage.rs:935`, `control-plane/crates/graphql-server/src/types/stage.rs:1288`, `control-plane/crates/graphql-server/src/types/stage.rs:1551`).
- MCP run-report readback exists with safe path handling, non-null defaults, and fallback principal redaction (`control-plane/crates/mcp-server/src/tools/reports.rs:143`, `control-plane/crates/mcp-server/src/tools/reports.rs:959`, `control-plane/crates/mcp-server/src/tools/reports.rs:1017`).
- Swift DTOs decode closed enums with `unknownDiagnostic`, presenter state is read-only, unsafe paths are rejected, and the inspector exposes status/evidence/authority groups with copy/reveal affordances (`Chainworks Forge/Engine/Readback/OutputContractRepair/OutputContractRepairEvidence.swift:3`, `Chainworks Forge/Engine/Readback/OutputContractRepair/OutputContractRepairPresenter.swift:34`, `Chainworks Forge/Engine/Readback/OutputContractRepair/OutputContractRepairPresenter.swift:96`, `Chainworks Forge/Views/RunInspectorView.swift:178`).
- The thin GraphQL read boundary requests `outputContractRepair` and surfaces the most severe current P079 evidence in run-detail presentation (`Chainworks Forge/Support/P031ThinGraphQLReadBoundary.swift:3286`, `Chainworks Forge/Support/P031ThinGraphQLReadBoundary.swift:6822`, `Chainworks Forge/Support/P031ThinGraphQLReadBoundary.swift:6874`).

Still missing or explicitly deferred:

- controlled provider fallback dispatch from frozen YAML policy;
- production same-session repair for current production providers;
- full projection artifact rebuild with bounded background sweep;
- release-lane and source-generation supersession eligibility exclusions;
- full provider-fallback rollout metric readback;
- required reference docs `p079-repair-prompt-template.md`, `p079-recovery-attribution.md`, and `p079-adapter-idempotency.md`;
- full, non-partial P079 acceptance gate.

These deferred lanes are stated in the canonical reference (`docs/reference/output-contracts-failure-evidence-and-recovery.md:759`, `docs/reference/output-contracts-failure-evidence-and-recovery.md:763`, `docs/reference/output-contracts-failure-evidence-and-recovery.md:765`) and the test-gate reference (`docs/reference/test-gates.md:2029`, `docs/reference/test-gates.md:2031`, `docs/reference/test-gates.md:2050`).

## Requirement Matrix

| Requirement | Status | Evidence |
|---|---:|---|
| P079 starts after normal output collection failure and before durable output-contract block | Partially Implemented | Engine recovery lane is wired after validation failure, but release/source-generation supersession exclusions remain deferred (`control-plane/crates/engine/src/executor.rs:11241`, `docs/reference/output-contracts-failure-evidence-and-recovery.md:765`). |
| Same-session repair at most once in same live ACP session | Partially Implemented | Lease/prompt dispatch and fixture path are wired, but production providers fail closed (`control-plane/crates/engine/src/executor.rs:12148`, `control-plane/crates/engine/src/executor.rs:12176`, `docs/reference/output-contracts-failure-evidence-and-recovery.md:763`). |
| Transcript/provider-envelope recovery with bounds and attribution | Partially Implemented | Bounded recovery and transport-attributed provider-envelope acceptance exist; required attribution reference doc remains missing (`control-plane/crates/engine/src/executor.rs:20938`, `control-plane/crates/engine/src/executor.rs:21053`, `docs/reference/output-contracts-failure-evidence-and-recovery.md:765`). |
| Controlled provider fallback from frozen fallback policy | Missing | Domain/DB/readback shells exist, but P079 dispatch remains deferred and engine repair events initialize `provider_fallback_json` as `None` (`control-plane/crates/engine/src/executor.rs:11464`, `docs/reference/output-contracts-failure-evidence-and-recovery.md:765`). |
| Fallback packet schema, redaction, principal binding, parent-child link | Partially Implemented | Schema/repo/readback fields exist, but no controlled fallback child dispatch exercises them (`control-plane/crates/db/migrations/095_p079_output_contract_repair.sql:187`, `control-plane/crates/db/src/repos/output_contract_repair.rs:941`, `docs/reference/test-gates.md:2050`). |
| Permission posture grants only canonical output writes | Partially Implemented | Transport/receipt tests pass and repair request carries canonical failed paths, but production repair is blocked because posture is advisory for all production providers (`control-plane/crates/engine/src/executor.rs:11734`, `control-plane/crates/engine/src/executor.rs:20010`, `docs/reference/test-gates.md:2051`). |
| Atomic settlement and canonical artifact truth | Partially Implemented | Materialization-before-recovered and transactional lease/event settlement are wired; release/source-generation supersession exclusions remain deferred (`control-plane/crates/engine/src/executor.rs:12529`, `control-plane/crates/db/src/repos/output_contract_repair.rs:724`, `docs/reference/output-contracts-failure-evidence-and-recovery.md:765`). |
| Plan evidence is diagnostic only, redacted, size-capped, protected | Implemented for current Junie lane | Plan evidence is collected as diagnostic evidence under protected meta-root paths; reference lists the hardening as current (`control-plane/crates/engine/src/executor.rs:20066`, `docs/reference/output-contracts-failure-evidence-and-recovery.md:761`). |
| Durable leases and replay/reclamation | Partially Implemented | Repair leases and TTL reclamation repo paths exist, including fallback lease enum support, but full projection rebuild/background sweep and fallback dispatch remain deferred (`control-plane/crates/db/src/repos/output_contract_repair.rs:647`, `control-plane/crates/db/src/repos/output_contract_repair.rs:1052`, `docs/reference/output-contracts-failure-evidence-and-recovery.md:765`). |
| GraphQL/MCP/run-report/Swift typed readback | Partially Implemented | Typed readback is present for current evidence and fallback defaults, but full provider-fallback metrics/readback cannot be complete without fallback dispatch (`control-plane/crates/graphql-server/src/types/stage.rs:1551`, `control-plane/crates/mcp-server/src/tools/reports.rs:986`, `Chainworks ForgeTests/Proposal079ContractRepairReadbackTests.swift:25`). |
| Metrics and rollout gate | Partially Implemented | Metric names are declared and repair metrics are emitted; full provider-fallback rollout metric readback and the full acceptance gate remain deferred (`control-plane/crates/db/src/metrics.rs:132`, `control-plane/crates/db/src/metrics.rs:963`, `docs/reference/test-gates.md:2050`). |
| Required docs and acceptance evidence | Missing / Partial | Evidence fixtures exist under `docs/evidence/rollout-contract/p079/`, but the required reference docs are absent and the gate checks only the two status docs (`scripts/test-gate.sh:8194`, `scripts/test-gate.sh:8202`, `docs/reference/output-contracts-failure-evidence-and-recovery.md:765`). |

## Routed Findings

### READY-001 - P079 is still a partial implementation, not proposal-complete

Severity: Critical
Reviewers: chainworks_execution_truth_reviewer, observability_rollout_reviewer, rust_reliability_reviewer

The canonical reference states P079 is partially implemented and lists deferred lanes that are in-scope for the proposal: controlled provider fallback dispatch, full projection rebuild/sweep, release/source-generation eligibility exclusions, provider-fallback metric readback, required reference docs, and full acceptance gate (`docs/reference/output-contracts-failure-evidence-and-recovery.md:759`, `docs/reference/output-contracts-failure-evidence-and-recovery.md:765`). The test-gate reference repeats that `proposal-079|p079` is a partial-acceptance proof, not the full proposal gate (`docs/reference/test-gates.md:2029`, `docs/reference/test-gates.md:2050`).

Impact: This blocks Ready/Implemented status regardless of the passing current gate.

Required closeout action: Either implement the deferred lanes and expand the gate to full acceptance, or revise P079 to explicitly remove those promises before closeout.

### CHAINWORKS-TRUTH-001 - Controlled P079 provider fallback dispatch is not implemented

Severity: Critical
Reviewers: chainworks_execution_truth_reviewer, api_contract_reviewer, rust_reliability_reviewer

The proposal requires at most one controlled fallback attempt from frozen fallback policy. The current implementation has schema, migration, domain DTOs, readback defaults, and repository support, but the P079 engine path still creates repair events with `provider_fallback_json: None` (`control-plane/crates/engine/src/executor.rs:11464`) and the reference explicitly says controlled provider fallback dispatch remains deferred (`docs/reference/output-contracts-failure-evidence-and-recovery.md:765`). The DB can store fallback links (`control-plane/crates/db/src/repos/output_contract_repair.rs:941`), but there is no corresponding P079 dispatcher from frozen YAML policy that assembles a closed fallback packet, binds the parent principal, creates the fallback child execution, and settles the child back to parent truth.

The older targeted retry provider fallback code in `command_handler.rs` is not a substitute: it is keyed off targeted retry payload/catalog binding behavior (`control-plane/crates/engine/src/command_handler.rs:1880`) and does not satisfy P079's frozen `output_repair_policies` contract, fallback packet schema, P079 lease single-flight, or parent-child settlement requirements.

Impact: The proposal's fallback recovery lane cannot succeed. Rows may display fallback defaults, but no P079-compliant fallback can be dispatched.

Required closeout action: Implement P079 fallback dispatch from `RunPlanSnapshot` fallback policy hash, feature flags, role allowlist, packet redaction/hash, principal binding, fallback lease, child execution, and parent settlement. Add positive and negative tests for the existing fallback fixture set.

### SEC-REL-001 - Production same-session repair is intentionally fail-closed

Severity: Major
Reviewers: rust_security_reviewer, rust_reliability_reviewer, chainworks_execution_truth_reviewer

The current security posture is safe but not proposal-complete. The implementation only treats the deterministic fixture provider as enforcement-capable (`control-plane/crates/engine/src/executor.rs:20010`), and all production providers are documented as full-access or bypassPermissions providers whose permission requests are advisory (`control-plane/crates/engine/src/executor.rs:20013`). The dispatch path then skips repair whenever `p079_permission_enforcement_advisory` is true (`control-plane/crates/engine/src/executor.rs:12020`). The reference states production same-session repair remains fail-closed and `CHAINWORKS_P079_ACCEPT_ADVISORY_REPAIR_POSTURE` is fixture-only (`docs/reference/output-contracts-failure-evidence-and-recovery.md:763`, `docs/reference/test-gates.md:2051`).

Impact: P079's same-session repair promise is only proven for deterministic fixtures. For Codex, Claude, Gemini, Junie, and Auggie, the repair path records diagnostic/blocked state instead of performing a production repair turn.

Required closeout action: Add enforceable server-side provider sandbox/tool restrictions and enable production repair only after the posture can actually constrain writes/tools/network, or revise P079 to declare production same-session repair out of scope.

### DOCS-API-001 - Required P079 reference docs are absent

Severity: Major
Reviewers: api_contract_reviewer, observability_rollout_reviewer

P079 requires `docs/reference/p079-repair-prompt-template.md`, `docs/reference/p079-recovery-attribution.md`, and `docs/reference/p079-adapter-idempotency.md`. The reference doc itself lists all three as deferred (`docs/reference/output-contracts-failure-evidence-and-recovery.md:765`). A file search under `docs/reference` found no matching files. The current gate only requires `output-contracts-failure-evidence-and-recovery.md` and `test-gates.md` (`scripts/test-gate.sh:8194`), so the gate can pass while these proposal-required docs are still missing.

Impact: Operators and implementers do not have the pinned template, adapter attribution rules, or idempotency behavior that the proposal treats as part of acceptance.

Required closeout action: Publish the three reference docs, link them from the canonical P079 reference section, and add them to the full P079 gate.

### OPS-001 - Full projection rebuild/sweep and provider-fallback metric readback remain deferred

Severity: Major
Reviewers: observability_rollout_reviewer, rust_reliability_reviewer

The DB repository exposes projection stale/fresh/permanently-stale updates and expired lease enumeration, and metrics are declared (`control-plane/crates/db/src/repos/output_contract_repair.rs:234`, `control-plane/crates/db/src/repos/output_contract_repair.rs:873`, `control-plane/crates/db/src/metrics.rs:132`). However, the canonical docs still list the full projection artifact rebuild with bounded background sweep and full provider-fallback rollout metric readback as deferred (`docs/reference/output-contracts-failure-evidence-and-recovery.md:765`, `docs/reference/test-gates.md:2050`).

Impact: Current observability is enough for the partial gate and current repair readback, but not enough for the full P079 rollout/rollback and fallback-operation contract.

Required closeout action: Wire the background projection rebuild/sweep, exercise permanently-stale abandonment behavior, and add provider-fallback metric readback tests once fallback dispatch exists.

## Security-Sensitive Diff Scan Summary

Sensitive surfaces reviewed:

- filesystem writes/materialization and plan-evidence copy boundaries;
- ACP repair-turn permission posture and runtime receipts;
- transcript/provider-envelope parsing and attribution;
- GraphQL/MCP redaction of canonical paths, principal IDs, and permission decisions;
- fallback packet/principal fields;
- metric label cardinality and privacy.

Current partial-state security posture is defensible because risky production repair fails closed. The security blocker is not an immediate unsafe dispatch; it is that the proposal promises a production same-session repair/fallback recovery lane that is not enabled until enforceable runtime restrictions and controlled fallback dispatch exist.

## Reviewer Scorecard

| Reviewer | Verdict | Notes |
|---|---:|---|
| chainworks_execution_truth_reviewer | Not Ready | Output repair evidence and current settlement paths exist, but fallback truth and full eligibility exclusions are missing. |
| rust_reliability_reviewer | Not Ready | Repair lease ordering and reclamation exist; fallback dispatch/replay and full sweep remain deferred. |
| api_contract_reviewer | Not Ready | Typed readback is strong for current rows; required reference docs and full fallback contract remain missing. |
| rust_security_reviewer | Not Ready for full P079 | Current production behavior is fail-closed; enabling the promised repair/fallback lanes requires enforceable sandbox and fallback packet/principal controls. |
| observability_rollout_reviewer | Not Ready | Gate passes but is explicitly partial; provider-fallback metrics/readback and full sweep are not in the acceptance path. |

## Verification Log

- Read proposal contract and structured rollout schema from `docs/proposals/079-contract-aware-output-repair-and-provider-fallback.md`.
- Read proposal-audit skill instructions, reviewer registry, router config, and Chainworks execution-truth reviewer guidance.
- Inspected Rust domain, migration, DB repo, engine executor, GraphQL, MCP, metrics, and command-handler fallback-adjacent code.
- Inspected Swift DTO, presenter, GraphQL read boundary, run inspector, and Swift readback tests.
- Searched for required P079 reference docs under `docs/reference`; none were present.
- Ran `./scripts/test-gate.sh proposal-079`: passed.

Gate caveats:

- The gate produced Rust/Swift compiler warnings and Xcode runtime warnings, but no P079 gate failure.
- The gate remains explicitly partial by repository documentation.

## Final Verdict

P079 has a meaningful partial implementation: schema, leases, repair evidence, bounded transcript/provider-envelope recovery, fixture same-session repair, readback, Swift presentation, and hardening are wired and covered by the current gate. It is not proposal-complete.

Do not close P079 as Implemented/Ready until the controlled fallback dispatcher, production repair posture decision, full projection sweep, release/source supersession exclusions, provider-fallback metrics/readback, required reference docs, and full acceptance gate are complete or the proposal is narrowed to match the implemented partial scope.
