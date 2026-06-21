# P079 Implementation Audit R1

Proposal: `docs/proposals/079-contract-aware-output-repair-and-provider-fallback.md`
Report path: `docs/proposals/079-contract-aware-output-repair-and-provider-fallback_IMPLEMENTATION_AUDIT_R1.md`
Generated: 2026-06-20
Head audited: `4c3dce2c5f70cd7dc540887979d399f7d354c59d`

## Verdict

Track 1 conformance verdict: **Not Implemented**

Track 2 implementation readiness verdict: **Not Ready**

P079 has meaningful scaffolding in place: Rust domain/readback types, SQLite tables and repository helpers, GraphQL/MCP/readback surfaces, Swift DTO/presenter source, ACP permission-posture tests, fixture evidence files, and an engine branch for fixture-only same-session repair. It does not satisfy the proposal contract because the current tree is unmerged and unbuildable at the full surface, the canonical `proposal-079` / `p079` gates are absent, production same-session repair fails closed for advisory providers, transcript/provider-envelope recovery never accepts recovered output, controlled provider fallback dispatch is not wired, required reference docs are missing, P079 metrics are not implemented, and the macOS inspector/readback test gate remains absent.

The proposal cannot be closed or promoted to implemented reference docs from this tree.

## Audit Inputs

- Proposal contract: `docs/proposals/079-contract-aware-output-repair-and-provider-fallback.md`
- Prior-review discovery: no prior P079 review artifacts found by the audit helper.
- Current implementation surfaces reviewed:
  - `AGENTS.md`
  - `docs/reference/output-contracts-failure-evidence-and-recovery.md`
  - `docs/reference/test-gates.md`
  - `control-plane/crates/domain/src/output_contract_repair.rs`
  - `control-plane/crates/db/migrations/079_p079_output_contract_repair.sql`
  - `control-plane/crates/db/src/repos/output_contract_repair.rs`
  - `control-plane/crates/engine/src/executor.rs`
  - `control-plane/crates/acp/src/transport.rs`
  - `control-plane/crates/acp/tests/integration.rs`
  - `control-plane/crates/graphql-server/src/schema.rs`
  - `control-plane/crates/mcp-server/src/tools/reports.rs`
  - `Chainworks Forge/Engine/Readback/OutputContractRepair/OutputContractRepairEvidence.swift`
  - `Chainworks Forge/Engine/Readback/OutputContractRepair/OutputContractRepairPresenter.swift`
  - `docs/evidence/rollout-contract/p079/**`
  - `scripts/test-gate.sh`

## Prior Review Reuse

No prior reviewer findings were reused.

The helper returned:

```json
{"artifacts":[],"proposal_path":"/Users/user/Documents/Chainworks Forge/docs/proposals/079-contract-aware-output-repair-and-provider-fallback.md","repo_root":"/Users/user/Documents/Chainworks Forge"}
```

## Preflight State

The worktree already contained unresolved merge conflicts before this audit report was written. The audit did not modify implementation code, tests, proposal text, or prior reports.

Unmerged paths include Rust public ingress, auth, engine, GraphQL, MCP, docs, and the test gate script:

- `control-plane/crates/auth/src/lib.rs`
- `control-plane/crates/daemon/src/main.rs`
- `control-plane/crates/domain/src/capabilities.rs`
- `control-plane/crates/engine/src/executor.rs`
- `control-plane/crates/graphql-server/src/schema.rs`
- `control-plane/crates/graphql-server/src/types/mod.rs`
- `control-plane/crates/mcp-server/src/server.rs`
- `control-plane/crates/mcp-server/src/tools/mod.rs`
- `control-plane/crates/mcp-server/src/tools/reports.rs`
- `control-plane/crates/mcp-server/src/tools/runs.rs`
- `docs/reference/test-gates.md`
- `scripts/test-gate.sh`

`git diff --check` fails with conflict markers across these files. This alone blocks readiness for a proposal that changes execution, auth, public readback, MCP, GraphQL, reports, and gates.

## Track 1: Requirement Conformance

| Requirement | Status | Evidence |
| --- | --- | --- |
| P079 canonical acceptance gate `./scripts/test-gate.sh proposal-079` and alias `p079` pass locally | **Missing** | The gate returns `Unknown gate: proposal-079`; `rg` finds no `proposal-079`, `p079`, `p079-swift-readback`, or `Proposal079` entry in `scripts/test-gate.sh`. |
| Normal output collection remains first, then P079 can run only for eligible output-contract failures | **Partial** | `executor.rs` contains a P079 branch after validation failure, but the same file has unresolved conflict markers and the full gate cannot run. |
| At most one same-session repair turn in the existing live ACP session | **Partial** | Fixture/domain support exists, and the engine attempts a lease-backed repair prompt. Production providers are treated as advisory-only and fail closed unless an override is enabled. |
| Repair prompt `p079_repair_v1` with bounded reflected content and canonical output-only instruction | **Partial** | Prompt-building code and placeholder fixtures exist, but required reference documentation is absent and the gate is missing. |
| Server-side permission posture only allows writes to frozen canonical output paths and denies other capabilities | **Partial** | ACP posture tests and permission decision types exist. The engine still blocks production repair because current providers do not prove enforceable permission posture. |
| Recover valid current-invocation output from transcript/provider envelope using transport-allocated attribution only | **Missing** | Engine recovery code explicitly fails closed as `unattributable_envelope`; docs state accepted transcript/provider-envelope recovery remains deferred. |
| Controlled provider fallback from frozen workflow policy, with packet schema and principal binding | **Partial / Missing active behavior** | Domain/DB/schema fields exist, but engine code sets `provider_fallback_json: None`, comments that fallback policy is not wired, and no fallback dispatch path was found. |
| Preserve declared validator, canonical path, source-generation, and existing settlement truth | **Partial** | DB lease/event helpers and materialization helpers exist, but the engine settlement area is conflicted and source-generation/release-lane exclusions remain documented as deferred. |
| Exclude release/publish/upload/distribution/git push and durable side-effect lanes | **Partial** | The proposal and reference docs preserve the rule; implementation evidence says release-lane and source-generation supersession exclusions remain deferred. |
| Durable DB authority for events, leases, fallback links, projection rebuild, and rollback readability | **Partial** | Migration and repo helpers exist. Full projection rebuild, restart sweep, and acceptance-gate coverage remain deferred. |
| Lease/restart/idempotency semantics for `reserved`, `prompt_sent`, terminal settlement, lost ACK, cancellation, supersession | **Partial** | Tables and repository helpers exist; many corresponding fixture files are placeholders, the engine is conflicted, and fallback-specific restart behavior is not wired. |
| Run report, MCP, GraphQL, and Swift readback parity for `output_contract_repair.v1` | **Partial** | Additive readback types and Swift DTO source exist. Public ingress files are conflicted, the Swift readback gate is absent, and no decode tests were found under `Chainworks Forge/Tests/Engine/Readback/`. |
| macOS operator shell shows read-only P079 diagnostics without mutating repair/fallback state | **Partial / Missing UI completion** | Swift DTO and presenter exist, but no macOS inspector integration or UI verification evidence was found. Reference docs state the macOS inspector UI remains deferred. |
| P079 metrics with bounded enum labels and no sensitive/high-cardinality labels | **Missing** | Metrics are listed in the proposal, but no active P079 metric emission or metric-label gate was found; placeholder metric negative fixtures exist. |
| Auto-retry ledger remains observe-only for P079 | **Partial** | Placeholder fixtures exist for observe-only/debounce behavior; no full acceptance proof was available because the P079 gate is absent. |
| Required docs and deterministic fixtures land before feature enablement | **Partial** | Many fixture files exist, but `docs/reference/p079-repair-prompt-template.md`, `docs/reference/p079-recovery-attribution.md`, and `docs/reference/p079-adapter-idempotency.md` are absent. Many rollout-contract fixtures contain `placeholder_fixture_kind`. |

## Track 2: Reviewer Routing

Selected reviewers:

- `rust_reliability_reviewer` for leases, restart, repair settlement, idempotency, cancellation, and fallback execution.
- `api_contract_reviewer` for closed schema, GraphQL/MCP/run-report parity, DTO identity, and decode behavior.
- `observability_rollout_reviewer` for migrations, gates, metrics, rollout fixtures, and documentation closure.
- `rust_security_reviewer` for permission posture, canonical path binding, fallback packet redaction, plan evidence boundaries, auth/principal revocation, and public ingress.
- `macos_ui_reviewer` for operator readback presentation, Swift DTO/presenter behavior, inspector integration, accessibility, and stale/unknown diagnostic states.

Rejected due to the five-reviewer cap:

- `rust_architecture_reviewer`; architecture concerns are covered here through reliability, API contract, rollout, and security findings because the proposal is not ready for a successful architecture sign-off.

Specialist coverage hard gate:

- Security-sensitive scope: **Triggered and failed for readiness** because P079 touches auth, public MCP/GraphQL ingress, filesystem writes, provider subprocess execution, permission grants, redaction, and untrusted transcript/plan parsing. The current conflicted tree prevents a conclusive security pass.
- UI/UX scope: **Triggered and failed for readiness** because the proposal requires operator-visible macOS diagnostics and inspector polish, but UI integration and tests are absent.
- Persistence/reliability scope: **Triggered and failed for readiness** because the proposal depends on crash-consistent leases and recovery sweeps, while full recovery/fallback coverage is missing.

## Reviewer Findings

### READY-001: Unresolved conflicts make the audited tree unshippable

Severity: Critical
Track: Readiness
Files: `control-plane/crates/engine/src/executor.rs`, `control-plane/crates/graphql-server/src/schema.rs`, `control-plane/crates/mcp-server/src/tools/reports.rs`, `control-plane/crates/auth/src/lib.rs`, `scripts/test-gate.sh`, `docs/reference/test-gates.md`

The current tree contains unresolved merge conflict markers in implementation, auth, GraphQL, MCP, docs, and the gate script. `git diff --check` fails, and several public ingress surfaces are among the conflicted files. No P079 readiness verdict can pass until the branch is merged cleanly and the canonical gate can run.

### OPS-001: The canonical P079 acceptance gate is absent

Severity: Critical
Track: Readiness
Files: `scripts/test-gate.sh`, `docs/reference/test-gates.md`

The proposal makes `./scripts/test-gate.sh proposal-079` and `./scripts/test-gate.sh p079` the primary acceptance gates. The current script reports `Unknown gate: proposal-079`, and text search finds no P079 gate alias. The conflicted `docs/reference/test-gates.md` also describes P079 as a partial-acceptance gate with multiple deferred acceptance items. This blocks both conformance and closeout.

### REL-001: Production same-session repair is not implemented beyond fail-closed posture

Severity: Major
Track: Conformance / Readiness
Files: `control-plane/crates/engine/src/executor.rs`, `docs/reference/output-contracts-failure-evidence-and-recovery.md`

The proposal requires at most one same-session repair turn for eligible output-contract failures. The implementation has fixture/domain support and lease scaffolding, but production providers are classified as advisory-only and the engine fails closed rather than dispatching repair. This is a safer partial state, but it is not the accepted P079 behavior.

### REL-002: Transcript/provider-envelope recovery never accepts recovered output

Severity: Major
Track: Conformance
Files: `control-plane/crates/engine/src/executor.rs`, `docs/reference/output-contracts-failure-evidence-and-recovery.md`

P079 requires recovery of contract-valid output already present in the current invocation transcript/provider envelope when transport-allocated attribution proves ownership. The current implementation records bounded unavailable evidence and returns `unattributable_envelope`; docs state accepted recovery remains deferred. This is a direct missing goal.

### REL-003: Controlled provider fallback dispatch and frozen policy binding are missing

Severity: Major
Track: Conformance
Files: `control-plane/crates/domain/src/output_contract_repair.rs`, `control-plane/crates/db/src/repos/output_contract_repair.rs`, `control-plane/crates/engine/src/executor.rs`, `docs/reference/output-contracts-failure-evidence-and-recovery.md`

P079 requires one controlled fallback attempt from a frozen fallback policy. Domain types, DB fields, and fallback packet structures exist, but the engine does not parse frozen `output_repair_policies`, does not dispatch fallback, and persists no active `provider_fallback_json` for the attempted repair path. This is schema scaffolding without the required behavior.

### API-001: Readback parity is incomplete and unverified

Severity: Major
Track: Readiness
Files: `control-plane/crates/graphql-server/src/schema.rs`, `control-plane/crates/mcp-server/src/tools/reports.rs`, `Chainworks Forge/Engine/Readback/OutputContractRepair/OutputContractRepairEvidence.swift`, `Chainworks Forge/Engine/Readback/OutputContractRepair/OutputContractRepairPresenter.swift`

The Swift DTO and presenter exist and use stable identity, closed enums, and optional parent decode helpers. However, no `p079-swift-readback` gate exists, no decode tests were found under `Chainworks Forge/Tests/Engine/Readback/`, and the GraphQL/MCP report files are conflicted. The proposal's compiled DTO/decode-test requirement is therefore not met.

### UI-001: Required macOS operator diagnostic surface is deferred

Severity: Major
Track: Conformance / Readiness
Files: `Chainworks Forge/Engine/Readback/OutputContractRepair/OutputContractRepairPresenter.swift`, `docs/reference/output-contracts-failure-evidence-and-recovery.md`

The proposal requires passive read-only operator diagnostics in the macOS shell, including progress chips, grouping, unknown diagnostic states, accessibility labels, copy behavior, stale projection handling, and notification affordances. Only DTO/presenter infrastructure was found; reference docs still list the macOS inspector UI as deferred.

### OPS-002: Rollout evidence is placeholder-heavy and required reference docs are missing

Severity: Major
Track: Readiness
Files: `docs/evidence/rollout-contract/p079/**`, `docs/reference/`

The rollout-contract directory contains many P079 fixture files, but numerous files still include `placeholder_fixture_kind`, including Swift DTO fixtures, fallback packet fixtures, metric-label fixtures, repair prompt fixtures, permission fixtures, and recovery/idempotency fixtures. The required reference docs `p079-repair-prompt-template.md`, `p079-recovery-attribution.md`, and `p079-adapter-idempotency.md` are not present.

### OPS-003: P079 metrics are not implemented

Severity: Major
Track: Conformance / Readiness
Files: `docs/evidence/rollout-contract/p079/negative/metric-label-cardinality-violation.json`, `docs/evidence/rollout-contract/p079/negative/metric-label-enum-drift-rejected.json`

The proposal requires a concrete metric inventory with bounded labels and no sensitive/high-cardinality values. I found placeholder metric fixtures but no active P079 metric emission or gate evidence. This leaves rollout monitoring and production safety unproven.

### SEC-001: Security pass cannot be conclusive on a conflicted public-ingress/auth tree

Severity: Major
Track: Readiness
Files: `control-plane/crates/auth/src/lib.rs`, `control-plane/crates/graphql-server/src/schema.rs`, `control-plane/crates/mcp-server/src/server.rs`, `control-plane/crates/mcp-server/src/tools/*.rs`, `control-plane/crates/engine/src/executor.rs`

P079 is security-sensitive: it handles untrusted provider content, permission requests, filesystem materialization, redaction, principal binding, public MCP/GraphQL readback, and subprocess recovery/fallback behavior. The production fail-closed posture is the correct safe default for advisory providers, but unresolved conflicts in auth and public ingress prevent a conclusive security review or readiness approval.

## Positive Evidence

- `control-plane/crates/domain/src/output_contract_repair.rs` defines a substantial closed P079 domain surface, including status, presentation category, adapter/provider family, same-session repair, transcript recovery, provider fallback, plan evidence, permissions, leases, budget, fallback packets, and constants.
- `control-plane/crates/db/migrations/079_p079_output_contract_repair.sql` creates P079 events, leases, and fallback links with useful constraints.
- `control-plane/crates/db/src/repos/output_contract_repair.rs` has event, subobject, lease, atomic settlement, stale projection, fallback link, and reclamation helpers.
- `control-plane/crates/acp/tests/integration.rs` includes P079 permission posture coverage.
- `Chainworks Forge/Engine/Readback/OutputContractRepair/OutputContractRepairEvidence.swift` has stable row identity based on `repairAttemptId:agentExecutionId`, excluding `evidenceVersion`.
- `cargo test -p domain output_contract_repair -- --nocapture` passed: 9 tests passed.

These are necessary building blocks, not sufficient implementation of the proposal.

## Verification Commands

| Command | Result |
| --- | --- |
| Prior review helper | Passed; no prior artifacts found. |
| `git ls-files -u` | Failed readiness; unmerged entries are present. |
| `git diff --check` | Failed; conflict markers remain in implementation/docs/gate files. |
| `./scripts/test-gate.sh proposal-079` | Failed; `Unknown gate: proposal-079`. |
| `rg -n "proposal-079\|p079\|p079-swift-readback\|Proposal079\|output_contract_repair" scripts/test-gate.sh` | No matches. |
| `cargo test -p domain output_contract_repair -- --nocapture` | Passed; 9 domain tests passed. |
| `rg -n "placeholder_fixture_kind" docs/evidence/rollout-contract/p079` | Found placeholder fixtures across Swift, fallback, recovery, permission, lease, metric, prompt, and auto-retry evidence. |
| `ls docs/reference/p079-*` | No required P079 reference docs found. |
| `rg -n "Proposal079\|OutputContractRepairReadback\|p079-swift-readback\|OutputContractRepair" "Chainworks Forge" --glob "*.swift"` | Found DTO/presenter source only; no tests or gate entry. |

## Closeout Recommendation

Do not close P079. The next implementation pass should first cleanly resolve the merge state, restore/add the canonical P079 gates, and then complete the missing active behaviors in this order:

1. Same-session repair acceptance under enforceable provider permission posture.
2. Transport-attributed transcript/provider-envelope recovery that can accept valid current-invocation output.
3. Frozen policy parsing and single-flight controlled provider fallback dispatch.
4. Crash/restart/reclamation coverage for repair and fallback leases.
5. GraphQL/MCP/run-report/Swift decode parity under `p079-swift-readback`.
6. macOS read-only inspector diagnostics and UI/accessibility verification.
7. P079 metrics and non-placeholder rollout fixtures/reference docs.

After that, rerun `./scripts/test-gate.sh proposal-079` from a clean worktree and only then consider implementation closeout.
