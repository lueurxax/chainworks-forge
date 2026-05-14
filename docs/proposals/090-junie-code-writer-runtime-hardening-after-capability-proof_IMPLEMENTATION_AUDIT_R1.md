# Implementation Audit R1: Proposal 090 - Junie Code Writer Runtime Hardening After Capability Proof

## Verdict

- Overall Conformance: Not Implemented
- Overall Readiness: Not Ready
- Audit Confidence: Medium-High
- Reviewer Selection Reuse: Reused exactly

This implementation contains useful P090 slices: additive receipt/readback fields, a P090 gate, evidence fixture inventory checks, settlement-row persistence, a narrow repair materialization safety test, and a first launch-root preflight path. It does not yet close the proposal contract. The core gaps are staged per-output settlement, complete public subtype classification, rollout flag behavior, and Junie runtime preflight depth.

The roll-up is "Not Implemented" under the audit skill because at least one in-scope requirement is Missing. The implementation should not be treated as merge-ready for P090.

## Audit Metadata

| Field | Value |
| --- | --- |
| Proposal | `docs/proposals/090-junie-code-writer-runtime-hardening-after-capability-proof.md` |
| Proposal state | Active draft proposal |
| Proposal md5 | `f98170c78ca39398e9aaed497180c057` |
| Repository | `/Users/user/Documents/Chainworks Forge` |
| Branch | `main` |
| Audited HEAD | `225bac4e47b135d92c4fe2de243dd13c4647be19` |
| Audit report | `docs/proposals/090-junie-code-writer-runtime-hardening-after-capability-proof_IMPLEMENTATION_AUDIT_R1.md` |
| Gate executed | `./scripts/test-gate.sh proposal-090` |
| Gate result | Passed |

## Worktree Scope

The worktree is dirty. The P090 implementation appears concentrated in Rust control-plane ACP, engine, DB, GraphQL, MCP/readback tests, evidence inventory, and the test gate. One unrelated existing Swift change was present in `Chainworks Forge/Support/DaemonLifecycleClient.swift` and was not considered part of this audit.

Primary touched implementation surfaces:

- `control-plane/crates/acp/src/adapters/mod.rs`
- `control-plane/crates/acp/src/transport.rs`
- `control-plane/crates/domain/src/code_writer_completion.rs`
- `control-plane/crates/db/migrations/053_p090_code_writer_runtime_hardening.sql`
- `control-plane/crates/db/src/repos/code_writer_completion_receipts.rs`
- `control-plane/crates/engine/src/executor.rs`
- `control-plane/crates/graphql-server/src/types/run.rs`
- GraphQL, MCP, DB, ACP, and engine tests
- `docs/evidence/090/junie-runtime-hardening/*`
- `scripts/test-gate.sh`

## Reviewer Routing

Prior review routing was found in `docs/proposals/090-junie-code-writer-runtime-hardening-after-capability-proof.review/evidence-pack.md`, but it was produced for an older proposal md5 (`e6f4a176751fffe415aeed362041a0bb`). The reviewer set still matches the current implementation surfaces, so it is reused exactly:

- `chainworks_execution_truth_reviewer`
- `rust_reliability_reviewer`
- `api_contract_reviewer`
- `observability_rollout_reviewer`

Rejected alternatives remain non-primary for this audit:

- `apple_arch_reviewer`: no SwiftUI/app architecture behavior is in the critical path.
- `rust_arch_reviewer`: architecture concerns are covered by execution-truth and reliability lenses.
- `rust_security_reviewer`: spoof-rejection is security-sensitive, but the proposal frames it as execution truth and API contract authority.
- `product_reviewer`: operator semantics matter, but the required public contract is already covered by API and rollout reviewers.

## Contract Summary

P090 promises runtime hardening after P089 proved Junie can return structured output. It is not a provider replacement proposal. It requires:

- strict final completion envelopes and no success from free-form prose alone;
- separate final payload capture independent of the forensic transcript;
- provider-neutral public `completion_boundary_subtype` readback with Junie-specific subtypes;
- staged, durable, per-output repair settlement before canonical artifact mutation;
- output-name keyed repair payloads;
- bounded final-response capture with truncation classification;
- progress-without-terminal-handoff classification;
- transcript absence not erasing completion truth;
- Junie runtime path preflight and remediation before provider launch;
- additive DB, GraphQL, MCP, and report readback;
- rollout controls for strict final payload, Junie preflight, and staged repair settlement;
- focused implementation and readiness gates backed by negative fixtures.

## Platform And Product Scope

- Apple platform scope: N/A for this implementation audit. The affected surfaces are backend/control-plane runtime behavior, DB, GraphQL, MCP, and test gates.
- Product scope: operator trust in code-writer completion truth, repair safety, and explanation of why a long-running Junie session did or did not publish outputs.
- Primary service flows:
  1. Junie code-writer returns a terminal payload, and the engine records trusted final completion truth.
  2. Junie returns prose, a truncated payload, or no terminal handoff, and the engine records the correct public boundary subtype.
  3. A repair turn returns mixed valid and malformed outputs, and only accepted outputs are durably staged, settled, and then published.
  4. Runtime path failures are caught before provider launch, remediated once where allowed, and exposed as failed-no-launch receipt facts.
  5. GraphQL, MCP, and reports expose the same completion boundary and settlement fields while preserving old-client compatibility.

## Requirement Classification

| ID | Requirement | Status | Evidence |
| --- | --- | --- | --- |
| REQ-001 | Preserve P089 capability proof semantics and do not reframe Junie as incapable. | Not Verifiable | The implementation remains Junie-centered, but this audit only ran `proposal-090`; it did not rerun the P089 capability gate or a long-running Junie canary. |
| REQ-002 | Engine owns completion/failure authority; provider-authored engine envelopes are untrusted and spoof attempts fail closed. | Partially Implemented | `control-plane/crates/acp/src/transport.rs` rejects provider-authored envelope extraction in a P090 test, and domain subtype values include spoof/mismatch names. The gate does not prove end-to-end engine readback of `provider_claim_rejected` or all mismatch fixtures. |
| REQ-003 | Strict final completion envelope; free-form prose is not an acceptable terminal success shape. | Partially Implemented | `control-plane/crates/engine/src/executor.rs` records final capture metadata and classifies missing/truncated cases, but strictness is hardcoded to `provider == "junie"` and not controlled by the rollout flags. |
| REQ-004 | Separate final payload capture from forensic transcript. | Partially Implemented | Receipt fields for final payload capture exist, but the captured JSON is inline metadata rather than a distinct persisted final-response artifact with redacted text path, and transcript failure independence is not exercised. |
| REQ-005 | Expose provider-neutral `completion_boundary_subtype` through DB, GraphQL, MCP/report, preserving unknown raw values. | Partially Implemented | Domain, DB, and GraphQL fields exist. GraphQL/MCP tests primarily exercise compatibility with `none`, not non-`none` Junie subtype readback and unknown raw round-trip behavior. |
| REQ-006 | Implement all required Junie boundary subtypes and map them to the correct conditions/actions. | Partially Implemented | Known values exist, but `p090_completion_boundary_subtype` does not emit all required conditions and can collapse partial repair rejection to `provider_envelope_unrecognized`. |
| REQ-007 | Persist durable per-output settlement rows linked to receipts with idempotency and lineage. | Partially Implemented | Migration and repository support settlement rows and idempotent digest checks, but `session_generation_id` is nullable, lineage validation is incomplete, and rows are persisted after repair materialization rather than before canonical mutation. |
| REQ-008 | Validate-before-materialize repair outputs using staging paths and active-pointer publication. | Partially Implemented | The engine writes only validation-passing outputs and leaves malformed siblings untouched, but it writes directly to canonical paths. Settlement rows have `staging_path`, `canonical_before_sha256`, and `active_pointer_generation_id` set to `None`. |
| REQ-009 | Prefer output-name keyed repair payloads. | Not Verifiable | Existing discovery/repair logic appears compatible with declared outputs, but this audit found no focused P090 test proving the new preferred keyed repair shape. |
| REQ-010 | Enforce final response size budget and classify truncation. | Partially Implemented | Final capture metadata includes byte counts and truncation status, and the subtype helper can return `junie_final_response_truncated`. The gate does not exercise a large final response fixture end to end. |
| REQ-011 | Distinguish progress-without-terminal-handoff from no progress. | Partially Implemented | The engine has a `junie_progress_without_terminal_handoff` path, but `progress_before_handoff` values do not match the proposal's public values and the condition is not proven with a realistic ACP progress/no-final fixture. |
| REQ-012 | Transcript absence must not erase completion truth. | Not Verifiable | Receipt fields include transcript and final capture statuses, but no test proves final-payload truth survives transcript persistence absence or failure. |
| REQ-013 | Add Junie runtime preflight before launch with fact capture, fail-before-launch behavior, and one remediation attempt where allowed. | Partially Implemented | ACP launch now ensures the Chainworks meta root and provider execution root before spawning. It does not implement the full preflight matrix, proof fixture reads, changed-file manifest parent checks, runtime-home write checks, shell-read checks, or remediation retry evidence. |
| REQ-014 | Keep receipt/readback schema additive and old-client compatible. | Implemented | Migration adds nullable/defaulted fields, and existing GraphQL/MCP readback tests pass with P090 fields present. |
| REQ-015 | Operator readback answers provider start, progress, final handoff, truncation, repair, and per-output settlement consistently across APIs. | Partially Implemented | Fields are exposed, but value semantics are incomplete and agreement is only proven for a narrow compatibility slice. |
| REQ-016 | Add concrete negative fixtures and validate the evidence inventory in the proposal-090 gate. | Implemented | `docs/evidence/090/junie-runtime-hardening/evidence-index.json` lists concrete negative fixtures and `scripts/test-gate.sh proposal-090` validates their presence and hashes. |
| REQ-017 | Implement rollout controls `CHAINWORKS_P090_STRICT_FINAL_PAYLOAD`, `CHAINWORKS_P090_JUNIE_PREFLIGHT_ENFORCE`, `CHAINWORKS_P090_STAGED_REPAIR_SETTLEMENT`, and disable behavior. | Missing | Runtime grep found these flags only in proposal/gate text, while engine receipt booleans are derived from provider/repair state rather than environment-controlled rollout behavior. |
| REQ-018 | Provide a focused proposal-090 gate covering subtype mapping, spoof rejection, staged settlement, preflight lifecycle, and API compatibility. | Partially Implemented | The gate exists and passes, but it does not yet prove all subtype values, staged settlement transaction ordering, crash recovery, non-`none` GraphQL/MCP readback, rollout flags, or the unchanged P089 capability proof. |

Summary: 2 Implemented, 11 Partially Implemented, 1 Missing, 4 Not Verifiable.

## Routed Findings

### [Critical] REL-001: Repair settlement is not staged before canonical artifact mutation

P090's central repair contract requires validate-before-materialize with durable settlement rows written before canonical mutation, staging paths, canonical before/after hashes, active pointer generation, and publication only from accepted rows in a transaction boundary.

The implementation still writes accepted repair outputs directly to canonical paths in `control-plane/crates/engine/src/executor.rs` before receipt persistence. `materialize_validated_discovery_decisions` performs direct filesystem writes, then `persist_p088_code_writer_completion_receipt` later stores settlement rows. The P090 row helper sets `staging_path`, `canonical_before_sha256`, and `active_pointer_generation_id` to `None`.

This means the repository can advertise `repair_materialization_mode = staged_per_output` without a staged settlement protocol. It reduces the blast radius for malformed siblings, but it does not deliver the durable settlement and crash-recovery semantics P090 requires.

Affected requirements: REQ-007, REQ-008, REQ-018.

### [Major] API-001: Boundary subtype mapping is incomplete and can misclassify partial repair outcomes

The proposal requires public Junie subtype values including `junie_repair_returned_narrative`, `junie_repair_returned_malformed_json`, and `junie_repair_outputs_partially_materialized`. The domain enum includes the values, but `p090_completion_boundary_subtype` in `control-plane/crates/engine/src/executor.rs` does not emit all of them from the relevant runtime conditions.

Notably, when required outputs are missing and any settlement decision is rejected, the helper can return `provider_envelope_unrecognized` instead of the Junie repair partial-materialization subtype. The gate has a mixed valid/malformed repair test, but it checks materialization behavior and rows rather than asserting the public boundary subtype.

Affected requirements: REQ-005, REQ-006, REQ-011, REQ-015, REQ-018.

### [Major] OPS-001: Rollout controls are documented but not implemented in runtime behavior

P090 defines rollout controls for strict final payload, Junie preflight enforcement, staged repair settlement, and an emergency disable path. Runtime search found the `CHAINWORKS_P090_*` names only in proposal/gate material, not in engine/ACP runtime configuration.

In `control-plane/crates/engine/src/executor.rs`, receipt booleans are hardcoded from local conditions such as `provider == "junie"` and `completion_turn_attempted`. That makes canary/staged rollout, downgrade behavior, and emergency disable unprovable.

Affected requirements: REQ-003, REQ-013, REQ-017, REQ-018.

### [Major] REL-002: Junie runtime preflight is only a shallow launch-root check

ACP launch now ensures the Chainworks meta root and provider execution root before provider spawn, and engine code maps some launch/open-session errors to P090 preflight JSON. That is a useful start, but it is not the P090 runtime preflight contract.

The proposal requires engine-owned decision facts plus adapter-owned launch checks for proof fixture reads, changed-file manifest parent paths, runtime-home/temp writeability, tool path readability, shell-level reads where relevant, fail-before-launch semantics, one remediation for cwd/runtime-home issues, and clear capacity accounting only after preflight passes. Those cases are not covered by implementation or tests.

Affected requirements: REQ-013, REQ-018.

### [Major] DB-001: Settlement row lineage is weaker than the proposal's minimum row contract

P090's minimum settlement row includes a non-null session generation id and lineage fields that tie a row to the exact execution generation. The migration declares `session_generation_id TEXT` as nullable, the domain model exposes it as `Option<String>`, and repository validation does not require stage id or session generation alignment when inserting rows.

That weakens idempotency, replay safety, and recovery diagnostics for a feature whose purpose is durable repair settlement.

Affected requirements: REQ-007, REQ-015.

### [Major] READY-001: Passing `proposal-090` is not yet sufficient readiness evidence

The focused gate passed and is valuable, but it does not cover the complete P090 acceptance contract. Missing proof includes the unchanged P089 capability gate, a long-running Junie canary, crash recovery around settlement rows, rollout flag on/off behavior, all public Junie subtype cases, and GraphQL/MCP readback for non-`none` P090 subtypes and unknown raw values.

Affected requirements: REQ-001, REQ-005, REQ-006, REQ-010, REQ-012, REQ-017, REQ-018.

## Reviewer Scorecards

### chainworks_execution_truth_reviewer

- Score: Not Ready
- Main concern: completion truth is not yet fully engine-owned in the public readback contract. Spoof extraction is guarded in ACP, but engine synthesized failure/readback authority and subtype truth are not proven across the required negative cases.

### rust_reliability_reviewer

- Score: Not Ready
- Main concern: the settlement path is not crash-safe staged settlement. Direct canonical writes before durable row publication leave the core reliability contract incomplete.

### api_contract_reviewer

- Score: Not Ready
- Main concern: public API fields exist, but their value semantics are incomplete. Required Junie subtypes and non-`none` API readback are under-tested.

### observability_rollout_reviewer

- Score: Not Ready
- Main concern: rollout controls are missing from runtime behavior, and operator-facing facts cannot yet support staged rollout, emergency disable, or full incident explanation.

## Verification Log

Executed:

```bash
./scripts/test-gate.sh proposal-090
```

Result: Passed.

Observed coverage:

- Evidence inventory validation passed.
- `cargo test -p db proposal_090_` passed.
- `cargo test -p acp proposal_090_` passed.
- `cargo test -p engine proposal_090_` passed.
- GraphQL and MCP P088 readback tests with P090 additive fields passed.

Not executed:

- Full regression gate.
- P089 capability proof gate.
- Real long-running Junie ACP canary.

Because the audit verdict is Not Ready, the full sign-off gate was not required to support a Ready verdict.

## Next Required Fixes

1. Implement true staged per-output repair settlement: staging path, validation, durable settlement rows before canonical mutation, canonical before/after hashes, active pointer generation, and crash recovery tests.
2. Complete subtype classification and add fixtures/tests for every required Junie subtype, including partial repair materialization and narrative repair responses.
3. Wire the `CHAINWORKS_P090_*` rollout controls into runtime behavior and test enabled, disabled, and downgrade modes.
4. Expand Junie preflight to the required launch-time checks and remediation lifecycle, with fail-before-launch tests.
5. Add GraphQL/MCP/report tests for non-`none` subtype readback, unknown raw values, and settlement rows.
6. Run P089 capability proof and a real long-running Junie canary before any Ready/Implemented closeout.

