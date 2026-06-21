# Proposal 079 Implementation Audit R7

## Verdict

**Not Ready - Partial implementation.**

The implementation has meaningful durable pieces: SQLite tables, domain/readback DTOs, GraphQL/MCP report projection, ACP repair-turn permission posture helpers, deterministic fixture same-session repair, plan-evidence copy/redaction, and Swift decode/presenter coverage. It does not yet satisfy the proposal as an implemented feature because production same-session repair is fail-closed for all real providers, controlled provider fallback from frozen YAML policy is not wired, required reference docs and rollout lanes are still deferred, and the local `proposal-079` gate failed in the Swift readback tail.

Closeout is rejected. This proposal should not be retired into reference-only documentation.

## Audit Metadata

| Field | Value |
| --- | --- |
| Proposal | `docs/proposals/079-contract-aware-output-repair-and-provider-fallback.md` |
| Audit report | `docs/proposals/079-contract-aware-output-repair-and-provider-fallback_IMPLEMENTATION_AUDIT_R7.md` |
| Repo | `/Users/user/Documents/Chainworks Forge` |
| HEAD | `0e6482c82b588b74a76294a225e68286bfe37fa4` |
| Worktree | Dirty; unrelated P080/P083/P086 changes present, P079 changes also present |
| Audit time | 2026-06-20 22:15:00 EEST |
| Prior-review reuse | Not reused. `discover_prior_review.py` returned no reusable artifacts for this proposal. |
| Report-path helper | `report_path.py` selected this R7 path. |

## Reviewer Selection

Selected reviewers, capped at five:

| Reviewer | Why selected |
| --- | --- |
| `rust_arch_reviewer` | P079 changes orchestration order, output settlement, fallback policy, and cross-crate Rust boundaries. |
| `rust_reliability_reviewer` | Lease ordering, retry/reclamation, crash windows, and restart semantics are central acceptance criteria. |
| `rust_security_reviewer` | The proposal touches provider prompts, filesystem writes, permission grants, redaction, and cross-provider fallback packet flow. |
| `api_contract_reviewer` | GraphQL, MCP, run-report, SQLite, and Swift DTO compatibility are part of the declared contract. |
| `macos_ui_reviewer` | The proposal has explicit macOS inspector and readback-presentation requirements. |

Displaced by cap: `observability_rollout_reviewer` and `rust_performance_reviewer`. I still checked rollout metrics/gates enough to classify readiness; a future Ready audit should include observability/rollout explicitly.

## Implementation Summary

Landed:

- Domain schema/enums for `output_contract_repair.v1`, including status/presentation enums, fallback structs, budget, leases, and fallback packet constants: `control-plane/crates/domain/src/output_contract_repair.rs:8`, `control-plane/crates/domain/src/output_contract_repair.rs:382`, `control-plane/crates/domain/src/output_contract_repair.rs:518`, `control-plane/crates/domain/src/output_contract_repair.rs:567`.
- SQLite migration for repair events, leases, and fallback parent links: `control-plane/crates/db/migrations/095_p079_output_contract_repair.sql:9`, `control-plane/crates/db/migrations/095_p079_output_contract_repair.sql:117`, `control-plane/crates/db/migrations/095_p079_output_contract_repair.sql:187`.
- Repair event and lease repo operations, including atomic event+lease insert and TTL reclamation: `control-plane/crates/db/src/repos/output_contract_repair.rs:1048`.
- ACP repair permission posture helpers allowing only exact canonical-path writes and denying non-allowlisted operations: `control-plane/crates/acp/src/transport.rs:2917`, `control-plane/crates/acp/src/transport.rs:2974`, `control-plane/crates/acp/src/transport.rs:3179`.
- Bounded transcript/provider-envelope recovery and fail-closed unavailable states: `control-plane/crates/engine/src/executor.rs:20820`.
- Plan-evidence collection/redaction for Junie with path containment and 0600/0700 intent: `control-plane/crates/engine/src/executor.rs:19949`, `control-plane/crates/engine/src/executor.rs:20241`.
- GraphQL/MCP readback redaction for sensitive fallback principal data: `control-plane/crates/graphql-server/src/types/stage.rs:1288`, `control-plane/crates/mcp-server/src/tools/reports.rs:143`.
- Swift DTO/presenter/inspector slice exists: `Chainworks Forge/Engine/Readback/OutputContractRepair/OutputContractRepairEvidence.swift:657`, `Chainworks Forge/Engine/Readback/OutputContractRepair/OutputContractRepairPresenter.swift:34`, `Chainworks Forge/Views/RunInspectorView.swift:178`.

Current reference documentation also states the same partial status and enumerates deferred lanes: `docs/reference/output-contracts-failure-evidence-and-recovery.md:759`, `docs/reference/output-contracts-failure-evidence-and-recovery.md:765`.

## Blocking Findings

### P1 - Production same-session repair is intentionally not available

The proposal requires eligible roles to get at most one same-session corrective repair turn after output settlement fails. The implementation only permits this for the deterministic `fixture` provider. The executor classifies all current production providers as advisory-only because their runtime permissions are not enforceable, then skips the repair dispatch and settles the evidence as skipped/manual investigation.

Evidence:

- Provider families `claude`, `codex`, `gemini`, `junie`, and `auggie` are mapped, then marked advisory when `p079_provider_supports_enforced_permissions` returns false: `control-plane/crates/engine/src/executor.rs:11277`.
- The production advisory guard terminates dispatch before the ACP repair prompt: `control-plane/crates/engine/src/executor.rs:11903`.
- The provider support function returns true only for `fixture`: `control-plane/crates/engine/src/executor.rs:19891`.
- The sole end-to-end repair integration test enables P079 only for provider `"fixture"`: `control-plane/crates/engine/tests/integration.rs:12475`.

Impact: real Codex/Claude/Gemini/Junie/Auggie runs still block instead of attempting the promised repair lane. This is a safe fail-closed posture, but it is not full implementation of the proposal.

### P1 - Controlled provider fallback is schema-only, not executable

The proposal requires one controlled provider fallback attempt after repair/recovery is unavailable or unsuccessful, sourced from frozen YAML policy. The implementation contains fallback DTOs, migration tables, metric names, and readback placeholders, but I did not find fallback policy parsing in the workflow crate or fallback child dispatch in the executor. The executor evidence row is initialized with `provider_fallback_json: None`, repair lease policy hash is explicitly empty, and a code comment says YAML `output_repair_policies` parsing is missing.

Evidence:

- Search for `FLAG_PROVIDER_FALLBACK`, `CHAINWORKS_P079_PROVIDER_FALLBACK_ENABLED`, and `output_repair_policies` found only domain constants and the executor missing-policy comment, not workflow compilation or engine dispatch.
- Executor comment: "Without YAML output_repair_policies parsing (MISSING-003) the only active flag is the env-var gate": `control-plane/crates/engine/src/executor.rs:11356`.
- Fallback evidence starts as `None`: `control-plane/crates/engine/src/executor.rs:11348`.
- Repair lease uses an empty fallback policy hash and notes fallback leases must supply the real hash when implemented: `control-plane/crates/engine/src/executor.rs:11313`.
- Domain fallback packet exists, but that alone does not dispatch fallback: `control-plane/crates/domain/src/output_contract_repair.rs:567`.
- Current reference docs list "controlled provider fallback dispatch from frozen YAML policy" as deferred: `docs/reference/output-contracts-failure-evidence-and-recovery.md:765`.

Impact: P079 cannot satisfy the fallback half of the proposal or the YAML frozen-policy/drift-aware acceptance criteria.

### P1 - The required local proposal gate failed

I ran:

```bash
CHAINWORKS_ALLOW_LOCAL_CARGO_TARGET_DIR=1 CHAINWORKS_CARGO_WRAPPER=0 ./scripts/test-gate.sh proposal-079
```

Rust/static portions passed, including:

- static proposal checks,
- domain P079 tests,
- DB metric declaration and repo tests,
- DB migration test,
- engine P079 unit tests,
- fixture same-session repair integration,
- ACP posture unit/integration tests,
- GraphQL/MCP P079 readback and redaction tests.

The gate failed in the Swift readback tail:

```text
Testing failed:
Chainworks Forge (...) encountered an error (Early unexpected exit, operation never finished bootstrapping - no restart will be attempted. (Underlying Error: The test runner exited with code 0 before establishing connection.))
** TEST FAILED **
```

Exit code: `65`.

Impact: the proposal acceptance criterion "the P079 gate passes locally" is not met, even before considering the runtime/fallback gaps.

### P2 - macOS inspector presentation is partial against the proposal polish contract

The Swift readback DTO/presenter exists, but the inspector surface does not implement several specific presentation requirements. The proposal asks for three `GroupBox` sections, `Copy Path` plus `Reveal in Finder`, relative and ISO date formatting, and background notifications for stale projection recovery. The implementation renders a single `InspectorSection` with `VStack`, a generic `Copy` context-menu item, and raw string labels from the presenter.

Evidence:

- Inspector uses `InspectorSection`/`VStack` and no `GroupBox` sections: `Chainworks Forge/Views/RunInspectorView.swift:181`.
- Plan evidence paths use `outputRepairCopyRow`: `Chainworks Forge/Views/RunInspectorView.swift:222`.
- Context menu exposes only `Button("Copy")`, not `Copy Path` or `Reveal in Finder`: `Chainworks Forge/Views/RunInspectorView.swift:267`.
- Search for `GroupBox`, `Reveal in Finder`, `Copy Path`, `Date.RelativeFormatStyle`, and `UNUserNotification` in the P079 Swift readback/inspector files found no implementation hits beyond comments.

Impact: this does not block backend safety, but it does block full APPLE-001/ui-001..004/macos-r3 acceptance.

### P2 - Rollout contract remains partial by current docs

The reference docs explicitly classify the P079 gate as partial and list deferred projection rebuild/sweep, provider-fallback metric readback, release-lane/source-generation exclusions, and required reference docs (`p079-repair-prompt-template.md`, `p079-recovery-attribution.md`, `p079-adapter-idempotency.md`): `docs/reference/test-gates.md:2050`, `docs/reference/output-contracts-failure-evidence-and-recovery.md:765`.

Impact: the current documentation is honest, but a proposal closeout would be inaccurate. The implementation is not in the "Implemented/Ready" state the closeout skill requires.

## Security Review

Security-sensitive diff scan was triggered by auth, filesystem/subprocess boundary, parser boundary, public ingress, DoS/resource limits, secrets/redaction/privacy, and unsafe dependency categories.

What looks sound:

- Production repair is fail-closed for advisory-only providers, preventing a false sense of permission enforcement: `control-plane/crates/engine/src/executor.rs:11903`.
- ACP repair posture denies shell/network/custom tools and only allows known write tools to byte-exact canonical paths: `control-plane/crates/acp/src/transport.rs:2941`, `control-plane/crates/acp/src/transport.rs:2951`, `control-plane/crates/acp/src/transport.rs:2974`.
- Permission denial sends an explicit JSON-RPC denial rather than silently continuing: `control-plane/crates/acp/src/transport.rs:3179`.
- Plan evidence collection rejects symlinks/hardlinks and writes copied evidence with a hardened path flow: `control-plane/crates/engine/src/executor.rs:20005`, `control-plane/crates/engine/src/executor.rs:20040`, `control-plane/crates/engine/src/executor.rs:20180`.
- GraphQL/MCP hide sensitive fallback principal values from non-operator callers: `control-plane/crates/graphql-server/src/types/stage.rs:1303`, `control-plane/crates/mcp-server/src/tools/reports.rs:143`.

Security residuals:

- The fallback packet redaction contract is not yet exercised by real fallback dispatch, so cross-provider data-flow risk is deferred, not proven.
- Production repair will remain correctly blocked until the runtime has enforceable provider sandbox/permission restrictions.
- The Swift inspector currently exposes copied path strings through a generic copy action; presenter filtering rejects absolute/traversal paths, which is good, but the promised "Reveal in Finder" workflow is absent rather than hardened.

## Requirement Scorecard

| Requirement area | Status | Notes |
| --- | --- | --- |
| SQLite migration and repos | Mostly implemented | Events, leases, fallback parent links, uniqueness, and reclamation paths exist. |
| Domain/readback schema | Mostly implemented | Closed enums and DTOs exist; fallback packet is a type, not an executed packet flow. |
| Same-session repair | Partial | Fixture provider only; production providers fail closed. |
| Transcript/provider-envelope recovery | Partial | Bounded, flag-gated recovery exists; broader attribution docs remain deferred by current reference docs. |
| Controlled provider fallback | Not implemented | No frozen YAML policy parsing or executor fallback child dispatch found. |
| Permission posture | Partially implemented | ACP voluntary posture exists; production enforcement boundary absent, so dispatch is blocked. |
| Plan evidence protection | Mostly implemented | Junie plan evidence copy/redaction/path hardening exists. Retention lifecycle still not proven in this audit. |
| GraphQL/MCP/run-report readback | Mostly implemented | Nested readback and redaction surfaces exist. |
| Swift DTO/readback | Partial | DTO/presenter exists; gate failed; inspector polish incomplete. |
| Metrics/rollout | Partial | Metric names and some repo emissions exist; fallback-lane readback remains deferred. |
| Acceptance gate | Failing | `proposal-079` exits 65 in Swift readback tail. |

Overall implementation conformance: **about 55-60%**. The durable readback/schema foundation is substantial, but the core runtime fallback/production repair behavior is not Ready.

## Verification Log

| Command/check | Result |
| --- | --- |
| `report_path.py docs/proposals/079-contract-aware-output-repair-and-provider-fallback.md` | Selected `docs/proposals/079-contract-aware-output-repair-and-provider-fallback_IMPLEMENTATION_AUDIT_R7.md`. |
| `discover_prior_review.py ...079...md` | No reusable prior review artifacts returned. |
| `implementation_surface_fingerprint.py --json` | Required surfaces included Rust architecture/reliability/security, API contract, observability/rollout, performance, and Apple UI/UX. |
| `security_sensitive_diff.py --json` | Triggered; categories included auth, parser, public ingress, filesystem/subprocess, DoS, redaction/privacy, dependency risk. |
| Targeted source inspection | Found implemented schema/readback/security slices and missing fallback/policy/production repair lanes. |
| `CHAINWORKS_ALLOW_LOCAL_CARGO_TARGET_DIR=1 CHAINWORKS_CARGO_WRAPPER=0 ./scripts/test-gate.sh proposal-079` | Failed with Xcode test runner early-exit in `p079-swift-readback`; Rust/static slices passed before failure. |

## Closeout Decision

Do not close out P079. Required before Ready:

1. Decide whether production same-session repair remains intentionally deferred; if yes, revise proposal scope/status instead of closing as implemented.
2. Implement frozen YAML `output_repair_policies` compilation into `RunPlanSnapshot`, provider fallback lease/child dispatch, fallback packet assembly/redaction/hash binding, and settlement back to the parent evidence row.
3. Add executable tests for fallback policy drift, fallback principal revocation, oversized packet blocking, fallback deadline, release-lane exclusion, and source-generation supersession.
4. Complete or explicitly descope the macOS inspector polish requirements.
5. Add the missing reference docs or remove them from acceptance scope.
6. Fix the `p079-swift-readback` test-runner failure and rerun `./scripts/test-gate.sh proposal-079`.
