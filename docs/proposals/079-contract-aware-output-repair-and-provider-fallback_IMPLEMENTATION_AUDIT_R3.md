# P079 Implementation Audit R3: Contract-Aware Output Repair and Provider Fallback

**Audit timestamp:** 2026-06-20 14:43 EEST
**Auditor:** Codex, proposal-implementation-audit skill
**Proposal:** `docs/proposals/079-contract-aware-output-repair-and-provider-fallback.md`
**Repo / HEAD:** `/Users/user/Documents/Chainworks Forge`, `0e6482c82b588b74a76294a225e68286bfe37fa4`
**Prior review reuse:** none found by `discover_prior_review.py`

## Verdict

**Overall conformance: Not Implemented / Not Ready for closeout.**

The tree contains substantial P079 infrastructure and the current `./scripts/test-gate.sh proposal-079` gate passes, but the full proposal is not implemented. Production same-session repair is deliberately fail-closed for all current production providers, transport-attributed recovery is incomplete, provider-envelope recovery is absent, controlled provider fallback dispatch from frozen YAML policy is absent, the macOS inspector UI is not integrated, and P079 metric emission remains deferred.

Do not retire this proposal or mark it implemented without either completing the missing behavior or explicitly narrowing/splitting the proposal with follow-up ownership.

## Scope Checked

I audited these implementation areas against the proposal's full behavior, not only the passing gate:

- Rust control-plane domain, DB migration/repos, engine executor, ACP transport, GraphQL, MCP reports.
- Swift readback DTO/presenter and targeted Swift tests.
- P079 reference documentation and rollout/test-gate wiring.
- Security-sensitive surfaces flagged by the audit helper: auth, filesystem/subprocess boundary, parser boundary, public ingress, resource limits, and secrets/redaction/privacy.

## Gate Result

**Canonical gate passed:** `./scripts/test-gate.sh proposal-079`

Observed passing slices:

- Static proposal-079 checks passed.
- Rust domain P079 tests: 9 passed.
- Rust DB P079 repo tests: 13 passed.
- Rust DB `proposal_079_output_contract_repair` integration tests: 6 passed.
- Rust engine P079 unit tests: 62 passed.
- Rust engine same-session repair fixture integration: 1 passed.
- ACP P079 posture tests: 18 unit + 13 integration passed.
- GraphQL P079 readback tests: 4 passed.
- MCP P079 readback/redaction tests: 5 unit + 2 integration passed.
- Swift `Proposal079ContractRepairReadbackTests`: 25 passed.

This proves the gated subset. It does not prove the full proposal because key commitments are explicitly outside that subset.

## Track 1: Requirement Conformance

| Requirement area | Status | Evidence |
|---|---:|---|
| Failure detection, eligibility gates, and ordering | Partial | Repair triggers after validation failure and records eligibility skips (`control-plane/crates/engine/src/executor.rs:10531`, `:10650`, `:11334`). The intended ordering includes fallback after recovery/repair, but fallback is not wired. |
| Same-session repair | Partial | Fixture same-session repair works and settles `ValidOutputsFromRepair` (`control-plane/crates/engine/tests/integration.rs:12349`, `:12531`). Production providers are blocked by design because only `fixture` supports enforced permissions (`control-plane/crates/engine/src/executor.rs:19463`, `:19475`) and advisory providers settle skipped before dispatch (`:11481`). |
| P079 permission posture and canonical-path boundary | Implemented for repair posture | ACP repair mode allows only exact canonical write tools/paths, denies shell/network/custom/read tools, uses `allow_once`, and fails closed (`control-plane/crates/acp/src/transport.rs:2917`, `:2974`, `:5307`). Tests cover canonical allowed/denied cases (`control-plane/crates/acp/tests/integration.rs:7328`). |
| Transcript/provider-envelope recovery | Partial / missing | Transcript bounded parsing exists, but the code states transport-derived attribution is not implemented (`control-plane/crates/engine/src/executor.rs:20392`). Provider-envelope recovery has schema/readback enum support but no active recovery implementation. |
| Controlled provider fallback | Missing | DB/domain/readback shapes exist, but executor inserts `provider_fallback_json: None` and comments that fallback policy hash is for "when implemented" (`control-plane/crates/engine/src/executor.rs:10885`, `:10920`). Reference truth says controlled fallback dispatch and YAML `output_repair_policies` parsing are not wired (`docs/reference/output-contracts-failure-evidence-and-recovery.md:793-794`). |
| Frozen fallback policy and packet contract | Missing behavior, partial schema | `OutputContractFallbackPacket` and fallback parent links exist (`control-plane/crates/domain/src/output_contract_repair.rs:568`, `control-plane/crates/db/src/repos/output_contract_repair.rs:888`), but no engine path compiles/fetches frozen YAML policy or dispatches a fallback child. |
| DB persistence, leases, crash-consistent settlement | Mostly implemented for repair rows/leases | Migration creates events, leases, and fallback parent links. Engine writes event+repair lease atomically before dispatch (`control-plane/crates/engine/src/executor.rs:10990`) and settles terminal event+lease together on failure/success paths (`:12137`). Fallback lease behavior remains DB-only without dispatch. |
| Junie plan evidence capture/redaction | Implemented for diagnostic lane | Junie `.junie/plans/*.md` capture is bounded, redacted, stored under P079 meta-root, and symlink/hard-link guarded (`control-plane/crates/engine/src/executor.rs:19521`). Tests cover redaction and symlink/hard-link rejection. |
| GraphQL/MCP/run report readback | Partial | Typed GraphQL objects and MCP snake_case readback are present (`control-plane/crates/graphql-server/src/types/stage.rs:686`, `:1551`; `control-plane/crates/mcp-server/src/tools/reports.rs:926`). They expose defaults for absent fallback/recovery behavior, but cannot substitute for missing behavior. |
| Swift readback | Partial | DTO/presenter and 25 fixture tests exist (`Chainworks Forge/Engine/Readback/OutputContractRepair/OutputContractRepairEvidence.swift:1`, `Chainworks ForgeTests/Proposal079ContractRepairReadbackTests.swift:25`). Search found no SwiftUI view/inspector integration beyond the presenter. |
| Metrics and rollout observability | Missing | Reference truth states "P079 metric emission remains deferred" (`docs/reference/output-contracts-failure-evidence-and-recovery.md:787`). I found no P079 metric helpers or gate assertions comparable to other proposals' metrics checks. |
| Full acceptance coverage | Partial | The current gate passes, but it is weaker than the proposal acceptance set: it proves schema/readback, fixture repair, and security posture tests, while fallback dispatch, provider-envelope recovery, production repair, metrics, and inspector UI remain absent/deferred. |

## Track 2: Specialist Review

**Architecture:** Not ready. The execution pipeline has a strong repair-row/lease skeleton, but the full proposal requires recovery -> repair -> fallback ordering. The fallback leg is not present, and production repair is intentionally fail-closed.

**API/contract:** Partial. `output_contract_repair.v1` is modeled in Rust, GraphQL, MCP, and Swift. However, readback schemas include fallback/recovery states that the engine cannot actually produce for provider-envelope recovery or controlled fallback.

**Reliability:** Partial. Repair lease ordering, prompt_sent transition, atomic event+lease settlement, and materialize-before-recovered logic are good. The fallback restart/idempotency/deadline story cannot be verified because fallback dispatch is missing.

**Security:** Partial but safely fail-closed. The implemented ACP posture is strict and well tested. The main security-adjacent blocker is not an exploitable enabled path; it is that production behavior remains disabled until an enforceable provider sandbox exists. Transcript recovery also lacks the proposal's required transport-ID attribution.

**Apple UI/UX:** Partial. Swift DTOs, presentation mapping, identity, stale projection, safe path filtering, and fixture tests exist. The operator-facing macOS inspector/status integration required by the proposal is not present in app views.

**Observability/rollout:** Not ready. P079 metrics are explicitly deferred, and the passing gate does not enforce the full rollout contract.

**Performance/resource bounds:** Mostly implemented for the paths that exist. Transcript recovery has byte/depth/chunk caps, reflected prompt fragments are capped, plan evidence is capped, and MCP/GraphQL JSON parsing has caps. Missing fallback means fallback packet caps are schema-only for production behavior.

## Security Pass

The security helper triggered for P079-relevant categories: auth, filesystem/subprocess boundary, parser boundary, public ingress, resource limits, secrets/redaction/privacy, and dependency/crypto-adjacent surfaces.

Findings:

- No new enabled critical vulnerability found in the implemented P079 repair posture; the production path is blocked before same-session repair dispatch for advisory-only providers.
- The strict ACP posture is a positive control: exact write-tool allowlist, no title fallback, byte-exact canonical path checks, symlink-parent denial, no `allow_always`, and sanitized runtime receipts.
- Security-sensitive conformance gap remains: transcript recovery does not implement transport-derived attribution, and provider-envelope recovery/fallback security controls cannot be assessed because those behaviors are not implemented.

## Rejected Close Alternatives

- **Close based on passing `proposal-079` gate:** rejected. The gate passes but does not cover all proposal commitments.
- **Treat schema/readback support as fallback implementation:** rejected. `provider_fallback_json` readback defaults are not provider fallback dispatch.
- **Enable production repair behind the existing flag:** rejected. The code intentionally blocks all current production providers until a real enforceable sandbox exists.
- **Retire proposal into reference docs:** rejected. Current reference docs still state deferred P079 lanes.

## Follow-Up Ownership

Before closeout, decide whether P079 remains the owning proposal or whether a new explicit follow-up proposal owns the residual scope. Required residual work:

1. Implement transport-attributed transcript recovery and provider-envelope recovery, or explicitly remove them from P079.
2. Implement frozen YAML `output_repair_policies`, fallback packet construction, fallback child dispatch, fallback parent links, and final settlement.
3. Define/enforce the production provider sandbox boundary needed to enable same-session repair beyond the fixture provider.
4. Wire the macOS operator inspector/status UI, not only DTO/presenter code.
5. Add P079 operational metrics and gate-owned metric assertions.
6. Expand the gate so passing `proposal-079` means the full proposal is implemented, not only the current subset.

## Final Recommendation

Keep P079 open. The implementation is valuable and much of the hard infrastructure is present, but the proposal is not complete enough for closeout or retirement.
