# Proposal 078 Implementation Audit R3

Proposal: `docs/proposals/078-durable-side-effect-ledger-release-settlement-and-reconciliation.md`
Implementation target: `.chainworks/worktrees/cw-implement-proposal-078-durable-e49976db`
Branch: `cw/implement-proposal-078-durable/e49976db`
HEAD: `8fb2afa96f8fb2ce9c26ae612a46fa26308147c4`
Audit timestamp: `2026-05-12 16:45:45 EEST`

## Verdict

Overall Conformance: **Partially Implemented**
Overall Implementation Readiness: **Not Ready**
Confidence: **High**

R3 shows material progress since the earlier audit state: the focused P078 gate passes, Swift builds, public operator conflict disposition exists, lease renewal/watchdog/preflight paths are wired, and GraphQL/MCP/report readback surfaces now expose side-effect state. This is no longer a mostly unimplemented slice.

It is still not ready to close P078. The remaining gaps are in the parts that make the proposal durable: P075-grade evidence spooling and recovery verification are not implemented, startup recovery only handles stale executing rows, metrics/rollout validation are mostly marker-level, and the Swift read model is only a DTO with no presenter or operator UI integration.

## Skill And Routing

Skill: `proposal-implementation-audit`
Mode: auto
Prior proposal-review reuse: **Not reused**

No prior proposal-review artifacts were discovered for P078. Prior implementation audit reports were ignored for reviewer selection per the skill rules and used only as historical context.

Selected reviewer lenses:

- `rust_reliability_reviewer`: side-effect ledger, CAS, lease renewal, crash recovery, idempotency, evidence durability.
- `api_contract_reviewer`: GraphQL/MCP/run-report readback and public operator tool contract.
- `observability_rollout_reviewer`: metrics, rollout-contract fixtures, gate coverage, operator handoff.
- `macos_product_reviewer`: Swift read model and operator affordance completeness.

## Target State

The target worktree is dirty and contains many implementation changes. The relevant P078 surfaces include:

- `control-plane/crates/db/migrations/052_p078_side_effect_ledger.sql`
- `control-plane/crates/domain/src/side_effect.rs`
- `control-plane/crates/db/src/repos/side_effects.rs`
- `control-plane/crates/engine/src/side_effects.rs`
- `control-plane/crates/engine/src/executor.rs`
- `control-plane/crates/mcp-server/src/tools/effects.rs`
- `control-plane/crates/mcp-server/src/tools/runs.rs`
- `control-plane/crates/graphql-server/src/schema.rs`
- `Chainworks Forge/Models/ExecutionTruth.swift`
- `docs/evidence/rollout-contract/operator-readback/p078-full-surface.fixture.json`
- `scripts/test-gate.sh`
- `docs/reference/test-gates.md`

The proposal file is structured JSON despite the `.md` extension. It has `schema_version=proposal_document_v1`, `proposal_revision_id=p078-refined-2026-05-07-dfc2d583-r2`, no explicit status field, and includes a rollout contract.

## Proposal Contract Summary

P078 requires a durable release side-effect ledger for irreversible or externally visible release actions. The core contract is:

- record intent before any external side effect;
- enforce idempotency, target keys, request fingerprints, and CAS-based single executor ownership;
- use explicit side-effect states, lease TTLs, deadlines, retry windows, and operator reconciliation;
- fail closed when unresolved effects exist or ledger readback fails;
- expose safe operator MCP tools and read-only GraphQL/report surfaces;
- spool expected and observed evidence using P075 durability discipline;
- preserve local validation safety without live git push/archive/upload effects;
- surface operator next actions in the app/read model;
- provide rollout evidence, metrics, and gates proving the behavior.

## Requirement Conformance

| Req | Proposal commitment | Status | Evidence |
| --- | --- | --- | --- |
| REQ-001 | Durable side-effect ledger schema/domain model with effect kinds, statuses, owner, target, request fingerprint, evidence, and settlement fields. | Implemented | Migration is present as `052_p078_side_effect_ledger.sql`; domain/repo tests pass under the P078 gate. |
| REQ-002 | Stable idempotency and request-fingerprint based dedupe for release effects. | Implemented | Release executor derives target/idempotency/fingerprint values before preparing intents, e.g. `control-plane/crates/engine/src/executor.rs:6798`. |
| REQ-003 | Intent must be recorded before external git/archive/upload operations. | Implemented | Git commit prepares and leases the effect before `mark_write_started` and before invoking `git_service.commit_changes` at `control-plane/crates/engine/src/executor.rs:6817` and `control-plane/crates/engine/src/executor.rs:6842`. Equivalent paths exist for push, archive, and Connect upload. |
| REQ-004 | CAS single-writer settlement plus leases/deadlines/renewal for long-running operations. | Implemented | `run_with_lease_renewal` renews during the external future at `control-plane/crates/engine/src/side_effects.rs:416`; settlement observes `lease_renewed_at` at `control-plane/crates/engine/src/side_effects.rs:479`. |
| REQ-005 | Fail closed for retry, cancellation, and advancement when unresolved effects or ledger readback errors exist. | Partially Implemented | Run-level preflight blocks unresolved effects at `control-plane/crates/engine/src/side_effects.rs:586`, and executor paths call it for advance/trigger/settle. The implementation does not yet prove the full proposal-level circuit-breaker/readback-error behavior across all operator flows. |
| REQ-006 | Startup and watchdog recovery reconcile crash windows and stale effects. | Partially Implemented | Startup repair calls `watchdog_pass` and tests cover stale executing effects. The watchdog only queries `list_expired_executing` at `control-plane/crates/engine/src/side_effects.rs:260`; prepared and externally-observed recovery, evidence validation, and partial-evidence classification remain incomplete. |
| REQ-007 | Operator MCP tools include safe public disposition of conflicts and protected manual-clear behavior. | Implemented | Public `effects.mark_conflict` is exposed and mapped at `control-plane/crates/mcp-server/src/tools/effects.rs:252`, `control-plane/crates/mcp-server/src/tools/effects.rs:486`, and `control-plane/crates/mcp-server/src/tools/mod.rs:100`; P078 MCP tests pass. |
| REQ-008 | GraphQL exposes read-only side-effect summaries with evidence and next-action affordances. | Implemented | `GqlSideEffectSummary` includes evidence, readback source, blocked reason, next action, recommended MCP tool, and retry-forbidden fields at `control-plane/crates/graphql-server/src/schema.rs:899`. |
| REQ-009 | Run reports, release receipts, MCP readback, GraphQL readback, and Swift read models expose unresolved side-effect state to operators. | Partially Implemented | MCP run reports include `side_effect_readback` at `control-plane/crates/mcp-server/src/tools/runs.rs:844`; Swift DTOs exist at `Chainworks Forge/Models/ExecutionTruth.swift:74`. The Swift DTOs are not used by a presenter/view and no macOS UI/accessibility tests prove operator affordance. |
| REQ-010 | Expected and observed side-effect evidence must be spooled with P075 durability, manifest-last ordering, checksum/size validation, and recovery classification. | Partially Implemented | Evidence roots and manifest JSON are written at `control-plane/crates/engine/src/executor.rs:9037`. However, the manifest is written via direct `std::fs::write` at `control-plane/crates/engine/src/executor.rs:9104`, only references the release receipt, and does not show temp-write, fsync, atomic rename, directory fsync, required file set, or startup checksum verification. |
| REQ-011 | Rollout contract and observability prove metrics/logs/readback parity and hold release until unresolved effects are visible. | Partially Implemented | `scripts/test-gate.sh` checks readback fixtures and metric string literals. `emit_p078_metric` only logs `metric_name` via tracing at `control-plane/crates/engine/src/side_effects.rs:19`, so operational metrics and full P084-style cardinality/readback validation are not proven. |
| REQ-012 | Local validation remains safe and does not perform live release side effects. | Implemented | The P078 gate passes using fixtures/source checks and focused unit tests; no live git push, archive upload, or Connect upload is performed. |

## Routed Findings

### REL-001 - P075-grade evidence spooling is still not implemented

Severity: High

The implementation now creates an evidence root, expected evidence JSON, and a manifest, but it does not satisfy the durable evidence contract that P078 depends on. `write_p078_side_effect_evidence_manifest` reads the release receipt, builds a manifest with a single `release_receipt` entry, and writes the manifest directly with `std::fs::write` (`control-plane/crates/engine/src/executor.rs:9074` through `control-plane/crates/engine/src/executor.rs:9104`).

That is not P075-grade spooling. The proposal requires manifest-last evidence with durable ordering and recovery semantics: temp path, file fsync, atomic rename, directory fsync, checksum/size validation, and startup classification of missing/partial/checksum-mismatched evidence. The current implementation can mark an effect settled with an observed evidence summary even though the evidence manifest itself was not durably written using the required protocol.

### REL-002 - Recovery is improved, but it covers only one crash class

Severity: High

`watchdog_pass` transitions stale `executing` effects to `needs_reconciliation` and the startup repair path invokes it, which is meaningful progress. The implementation still queries only `list_expired_executing` (`control-plane/crates/engine/src/side_effects.rs:260`), so it does not prove the proposal's broader recovery model for prepared-without-execution, externally-observed-but-unsettled effects, or settled records with missing/partial evidence.

This matters because P078 is about release settlement after irreversible writes. The crash window after an external write but before complete evidence settlement is exactly where the operator needs deterministic reconciliation.

### OPS-001 - Metrics and rollout proof are marker-level, not operational proof

Severity: Medium

The focused gate checks useful strings and fixtures, and it now catches more P078 regressions than before. However, the observability implementation is currently a tracing helper that logs `metric_name` (`control-plane/crates/engine/src/side_effects.rs:19`), not a verified metrics surface with counters/histograms/cardinality constraints.

The rollout fixture proves field shape and parity lanes, but the effects arrays are empty. That validates readback presence, not the operator experience for a real unresolved release side effect. P078's rollout contract asks for metrics/logs/readback evidence strong enough to hold release when unresolved effects exist; the current proof does not reach that bar.

### UI-001 - Swift readback is DTO-only

Severity: Medium

`SideEffectReadbackSummary` and `SideEffectReadbackItem` are defined in `Chainworks Forge/Models/ExecutionTruth.swift:74`, and the Swift build passes. A search found no usage outside that model file. There is no presenter, view, operator affordance, or accessibility test proving that the canonical Swift app actually exposes the blocked state and next action to operators.

For P078, a backend-only readback field is not enough because the Swift app is the canonical operator shell during parity.

### READY-001 - The implementation is not closeout-ready yet

Severity: High

The focused P078 gate and build gate are green, which is necessary but insufficient. The remaining gaps are in proposal-critical durability and operator-readback behavior, not small cleanup. This proposal should not be marked Implemented/Ready until evidence spooling/recovery and Swift operator surfacing are proven by behavior-level tests.

## Positive Evidence

- `./scripts/test-gate.sh proposal-078` passed.
- `cd control-plane && CARGO_TARGET_DIR=target/proposal-078-audit-check cargo check -p graphql-server -p mcp-server` passed with warnings only.
- `./scripts/test-gate.sh build` passed.
- The P078 gate now runs domain, DB, engine, release, MCP, fixture, marker, and Swift-forbidden-tool checks.
- Lease renewal and deadline fields are now present for release side effects.
- Startup repair invokes the watchdog.
- Public `effects.mark_conflict` exists and is covered by P078 MCP tests.
- GraphQL and MCP run-report readback now expose side-effect evidence, blocked reason, next action, recommended MCP tool, and retry-forbidden state.

## Verification Log

| Command | Result | Notes |
| --- | --- | --- |
| `git diff --check -- ...` | Passed | No whitespace/conflict-marker failures in audited paths. |
| `./scripts/test-gate.sh proposal-078` | Passed | Gate ended with `Proposal 078 durable side-effect ledger gate passed`. |
| `cd control-plane && CARGO_TARGET_DIR=target/proposal-078-audit-check cargo check -p graphql-server -p mcp-server` | Passed | Warnings only. |
| `./scripts/test-gate.sh build` | Passed | Xcode build succeeded and embedded the control-plane daemon. |

Not run:

- `./scripts/test-gate.sh fast`
- `./scripts/test-gate.sh full`
- Remote UI smoke tests
- Live release side-effect execution against real git/archive/upload targets

The omitted live checks are acceptable for audit safety, but not for an Implemented/Ready closeout claim.

## Required Before Ready

1. Implement evidence spooling through the same durability discipline as P075: temp files, fsync, atomic rename, directory fsync, manifest last, and manifest verification during recovery.
2. Record the complete expected/observed evidence set per effect kind, not only a release receipt pointer.
3. Extend recovery to prepared, externally observed, and settled-with-bad-evidence states; add tests for each crash window.
4. Replace marker-only metrics proof with verified operational counters/histograms/log fields and cardinality constraints.
5. Surface `side_effect_readback` in the Swift operator flow with clear blocked state, next action, recommended MCP tool, and retry-forbidden behavior.
6. Add behavior-level tests for non-empty unresolved side-effect readback across MCP, GraphQL, run report, release receipt, and Swift presentation.

## Final Assessment

P078 has advanced from "not implemented" to a credible partial implementation. The implementation now has the central ledger path, release operation wrapping, lease renewal, stale-executing watchdog, MCP conflict disposition, and readback surfaces.

It should still remain open. The proposal is specifically about durable release settlement after irreversible side effects, and the current implementation does not yet prove durable evidence spooling, full crash-window recovery, or operator-facing Swift affordance. The correct closeout state is **Partially Implemented / Not Ready**.
