# P079 Implementation Audit R4: Contract-Aware Output Repair and Provider Fallback

**Audit timestamp:** 2026-06-20 16:17 EEST
**Auditor:** Codex, proposal-implementation-audit skill
**Proposal:** `docs/proposals/079-contract-aware-output-repair-and-provider-fallback.md`
**Report path:** `docs/proposals/079-contract-aware-output-repair-and-provider-fallback_IMPLEMENTATION_AUDIT_R4.md`
**Repo / HEAD:** `/Users/user/Documents/Chainworks Forge`, `0e6482c82b588b74a76294a225e68286bfe37fa4`
**Target / compare base:** current dirty worktree compared to P079 proposal revision `p079-contract-aware-output-repair-and-provider-fallback-r5` in the proposal JSON.
**Prior review reuse:** `discover_prior_review.py` found no proposal-review artifacts. Existing R1-R3 implementation audits were treated only as historical regression context, not as specialist-review reuse.
**Dirty-tree caveat:** this workspace also contains unrelated P082/P083/P086/P080 edits and untracked audit files. The audit below is scoped to P079-owned behavior and P079-adjacent readback/gate files.

## Final Verdict

**Partial implementation / Not Ready for closeout.**

P079 has stronger implementation evidence than R3: core P079 DB lifecycle metrics are now declared, emitted from repair repository operations, and tested; Rust domain/DB/engine/ACP/GraphQL/MCP targeted P079 slices pass. However, the proposal still cannot be retired because major promised behavior remains absent or intentionally fail-closed:

- Production same-session repair is still disabled for all current production providers until an enforceable runtime sandbox exists.
- Transcript recovery lacks the required transport-attributed chunk ownership proof, and provider-envelope recovery remains deferred.
- Controlled provider fallback dispatch from frozen YAML policy is not wired.
- The macOS P079 inspector/status UI is not integrated beyond DTO/presenter helpers.
- The documented `proposal-079|p079` gate is explicitly partial, and the local full gate did not complete in this audit.

Keep P079 open unless the proposal is narrowed and residual work is moved into explicit follow-up ownership.

## Proposal State And Summary

The proposal is titled **Contract-Aware Output Repair and Provider Fallback** and is still marked `draft_for_implementation_review`. Its goals require at most one same-session repair turn, current-invocation transcript/provider-envelope recovery, at most one controlled fallback attempt from frozen policy, strict contract/canonical-path settlement, durable evidence through reports/MCP/GraphQL/Swift DTOs, and deterministic local gates.

The acceptance criteria require, among other items, transport-attributed recovery, fallback packet and policy behavior, repair permission posture, deterministic leases, fallback principal binding, readback through GraphQL/MCP/run reports/Swift, and passing local `proposal-079|p079` gates.

## Selected Reviewer Lenses

| Lens | Included | Reason |
|---|---:|---|
| Architecture | Yes | P079 changes execution ordering and settlement authority. |
| Reliability | Yes | Leases, restart behavior, prompt-sent durability, and sweeps are proposal-critical. |
| API contract | Yes | Evidence schema must match GraphQL/MCP/run-report/Swift. |
| Security | Yes | Helper triggered security-sensitive categories; permission posture and parser bounds are security-critical. |
| Observability / rollout | Yes | Metrics, rollout gate semantics, and reference truth changed since R3. |
| Apple UI/UX | Covered in Track 1, not selected as full specialist lens | Hard cap kept the routed specialist set to five; UI was still checked against proposal requirements. |
| Performance | Not selected | Covered only where resource bounds are part of security/reliability. |

## Requirement Conformance

| Requirement area | Status | Evidence |
|---|---:|---|
| P079 starts after output settlement failure | Implemented for the repair lane | The engine invokes the lane after declared-output settlement and validation failure in `control-plane/crates/engine/src/executor.rs:10528-10664`. |
| Eligibility gates for cancellation, approval, and workflow conflict | Partial | Cancellation/approval/conflict gates exist at `control-plane/crates/engine/src/executor.rs:10800-10827` and dispatch skip handling at `:11462-11547`. Reference docs still list release-lane and source-generation supersession exclusions as deferred at `docs/reference/output-contracts-failure-evidence-and-recovery.md:765`. |
| Same-session repair | Partial | Fixture/enforced repair path exists and engine tests pass. Production providers remain fail-closed because only `fixture` returns enforced support in `control-plane/crates/engine/src/executor.rs:19596-19611`; advisory providers settle skipped before dispatch at `:11608-11668`. |
| Repair permission posture | Implemented for ACP repair mode | The ACP P079 unit and integration slices pass; the posture allows only single canonical write requests and denies shell/network/custom/non-canonical requests. |
| Transcript recovery | Partial | Bounded scanning exists and can validate a `CHAINWORKS_OUTPUT` candidate, but the function states transport-derived attribution is not implemented and starts fail-closed without that proof in `control-plane/crates/engine/src/executor.rs:20525-20533`. |
| Provider-envelope recovery | Missing | Reference truth still says accepted transcript/provider-envelope recovery is deferred at `docs/reference/output-contracts-failure-evidence-and-recovery.md:765` and `:773-775`. I found schema/readback support but no active provider-envelope recovery lane. |
| Controlled provider fallback | Missing | Event rows still initialize `provider_fallback_json: None` in `control-plane/crates/engine/src/executor.rs:11050-11054`, repair lease key comments say fallback policy hash is for "when implemented" at `:11010-11020`, and reference docs state fallback dispatch and YAML `output_repair_policies` parsing are not wired at `docs/reference/output-contracts-failure-evidence-and-recovery.md:793-794`. |
| Fallback packet and parent-link persistence | Schema-only / DB partial | Migration/domain/repo shapes exist, including fallback parent links, but no engine path constructs a frozen packet and dispatches a fallback child. |
| DB events, leases, and repair-row lifecycle | Mostly implemented for repair | Event+lease insertion is atomic at `control-plane/crates/engine/src/executor.rs:11123-11130`; prompt-sent is durably committed before dispatch at `:11736-11755`; repo tests passed. Fallback lease behavior remains unproven because fallback dispatch is absent. |
| Junie plan evidence | Implemented for diagnostic lane | Plan-evidence directory creation, redaction, and DB persistence are wired at `control-plane/crates/engine/src/executor.rs:11166-11296`; engine redaction/path tests passed. |
| Metrics | Partial but improved since R3 | `P079_REQUIRED_METRICS` is declared in `control-plane/crates/db/src/metrics.rs:132-155`; repair repo emits attempt/terminal/transcript/budget metrics at `control-plane/crates/db/src/repos/output_contract_repair.rs:109-113`, `:153-157`, and `:197-204`; the metric declaration and repo metric tests passed. Full provider-fallback rollout readback remains deferred per `docs/reference/output-contracts-failure-evidence-and-recovery.md:787`. |
| GraphQL/MCP/run-report readback | Partial | Typed GraphQL and MCP readback are present, and targeted readback safety tests passed. These surfaces expose schema/defaults for fallback states that the engine cannot yet produce through controlled fallback dispatch. |
| Swift DTO/presenter | Partial | DTO identity and optional-parent decoding exist in `Chainworks Forge/Engine/Readback/OutputContractRepair/OutputContractRepairEvidence.swift:650-770`; presentation helpers exist in `Chainworks Forge/Engine/Readback/OutputContractRepair/OutputContractRepairPresenter.swift:34-112`. The standalone Swift gate failed locally before tests could run due to an app-host code-signing/system-policy crash. |
| Swift macOS inspector/status UI | Missing / not integrated | Search found P079 DTO/presenter/tests but no P079 references in app views. Reference truth still lists the Swift macOS inspector UI as deferred at `docs/reference/output-contracts-failure-evidence-and-recovery.md:765`. |
| Full proposal gate | Not satisfied | `docs/reference/test-gates.md:2029-2051` explicitly calls the P079 gate partial and lists accepted recovery, fallback dispatch, projection rebuild/sweep, macOS inspector UI, and required reference docs as deferred. |

## Specialist Scorecard

| Lens | Score | Assessment |
|---|---:|---|
| Architecture | Not Ready | The repair lane has a durable skeleton, but the promised recovery -> repair -> fallback pipeline is incomplete. |
| Reliability | Partial | Repair event/lease ordering and atomic settlement have useful tests. Fallback restart/idempotency/deadline behavior cannot be proven without fallback dispatch. |
| API contract | Partial | Evidence shapes span Rust, GraphQL, MCP, and Swift DTOs. Some exposed fallback/recovery states are still schema/readback-only. |
| Security | Partial / fail-closed | Implemented repair posture is strict and tested. Production repair remains disabled because current providers are advisory-only; missing recovery/fallback paths cannot receive a Ready security assessment. |
| Observability / rollout | Partial | Core repair lifecycle metrics are now implemented and tested. Full fallback metric readback and the full rollout gate remain deferred. |
| Apple UI/UX | Not Ready | DTO and presenter exist, but operator-facing inspector/status integration remains absent. |

## Security-Sensitive Diff Summary

The security helper triggered and required an independent security pass. Triggered categories included auth, filesystem/subprocess boundary, parser boundary, public ingress, DoS/resource limits, secrets/redaction/privacy, and dependency/crypto-adjacent surfaces.

Security assessment:

- No enabled critical vulnerability was found in the implemented P079 repair posture.
- The ACP repair posture tests are strong: canonical write allow, non-canonical write deny, shell/network/custom tool deny, ambiguous multi-path deny, no safe `allow_once` deny, and sanitized decision fields.
- The production path is intentionally fail-closed for advisory providers, which is safer than enabling prompt-only repair permissions.
- Security conformance is still incomplete because transport-attributed recovery, provider-envelope recovery, and controlled fallback dispatch are not implemented. Their security controls cannot be validated as production behavior.

## Findings

### P1: Controlled provider fallback is still schema-only

P079 requires at most one controlled fallback attempt after repair/recovery is unavailable or unsuccessful, using frozen YAML fallback policy. The current implementation has migration/domain/readback scaffolding, but the engine never constructs a fallback packet or dispatches a child execution from frozen policy. Evidence rows still start with `provider_fallback_json: None`, repair lease key derivation leaves the fallback policy hash empty with a "when implemented" comment, and reference docs state fallback dispatch and YAML policy parsing are not wired.

Closeout impact: this directly blocks the "provider fallback" half of the proposal and invalidates the full acceptance criteria around fallback packets, principal binding, idempotency, deadlines, policy drift, and fallback final settlement.

### P1: Production same-session repair remains intentionally fail-closed

The proposal goal says eligible roles receive at most one same-session repair turn. Current code only treats the deterministic fixture provider as enforced. All production provider families are advisory-only and are skipped before dispatch until a real runtime permission boundary exists.

Closeout impact: fixture repair is useful proof, but production P079 repair behavior is not implemented for Codex/Claude/Gemini/Junie/Auggie lanes.

### P1: Recovery is not yet transport-attributed and provider-envelope recovery is absent

The transcript recovery function has byte/depth/chunk bounds and can settle a valid transcript candidate, but the code explicitly documents that transport-derived attribution is not implemented. The required provider-envelope recovery lane is still deferred in reference truth.

Closeout impact: P079 cannot claim the "recover current invocation output already present in transcript/provider envelope" goal because the security-critical attribution rule is the core acceptance condition.

### P1: P079 still lacks the promised macOS operator inspector/status integration

Swift DTO and presentation helpers exist, but app views do not reference P079 evidence, and the current reference doc lists the macOS inspector UI as deferred. The standalone Swift gate did not reach DTO assertions in this audit because the test host app crashed before bootstrapping due `Chainworks Forge.debug.dylib` being denied by system policy in DerivedData.

Closeout impact: the canonical operator shell is not verified as the passive P079 readback surface described by the proposal.

### P2: The canonical P079 gate is not a full acceptance gate

`docs/reference/test-gates.md` labels `proposal-079|p079` as a partial-acceptance gate and lists the remaining deferred lanes. In this audit, `./scripts/test-gate.sh proposal-079` also failed locally before completing because the first Cargo slice was killed by the OS after static checks.

Closeout impact: passing the current gate would still not prove the full proposal. The local run did not even produce a full-gate pass in this audit.

## Residual Scope And Ownership

Before closeout, P079 needs either implementation or explicit follow-up proposals for:

1. Transport-attributed transcript chunk recovery and provider-envelope recovery.
2. Frozen YAML `output_repair_policies` compilation, fallback packet construction, fallback child dispatch, fallback parent linkage, and fallback settlement.
3. The enforceable provider sandbox/permission boundary required before same-session repair can run for production providers.
4. Release-lane and source-generation supersession exclusions that reference truth still lists as deferred.
5. Full projection artifact rebuild and bounded background sweep.
6. macOS inspector/status UI integration for P079 evidence.
7. Full fallback rollout metric readback and dashboards.
8. Required reference docs: `p079-repair-prompt-template.md`, `p079-recovery-attribution.md`, and `p079-adapter-idempotency.md`.
9. A `proposal-079|p079` gate whose passing result means full proposal acceptance, not partial acceptance.

## Verification Log

| Check | Result | Notes |
|---|---:|---|
| `report_path.py` | Pass | Next report path resolved to this R4 file. |
| `discover_prior_review.py` | Pass | Returned no prior proposal-review artifacts. |
| `security_sensitive_diff.py --root ... --json` | Triggered | Required security pass; categories listed above. |
| `implementation_surface_fingerprint.py --root ... --json` | Pass | Required lenses included API, Apple UI, architecture, observability/rollout, performance, reliability, and security; routed set capped to five plus Track 1 UI check. |
| `./scripts/test-gate.sh proposal-079` | Failed to complete | Static checks passed, then `cargo test -p domain output_contract_repair` was killed by the OS (`Killed: 9`, exit 137). |
| `CARGO_BUILD_JOBS=1 CARGO_TARGET_DIR=target/p079-audit-r4 cargo test -p domain output_contract_repair -- --nocapture` | Pass | 9 tests passed. |
| `CARGO_BUILD_JOBS=1 CARGO_TARGET_DIR=target/p079-audit-r4 cargo test -p db proposal_079_required_metric_names_are_declared_and_recordable -- --nocapture` | Pass | 1 test passed. |
| `CARGO_BUILD_JOBS=1 CARGO_TARGET_DIR=target/p079-audit-r4 cargo test -p db output_contract_repair -- --nocapture` | Pass | 14 tests passed. |
| `CARGO_BUILD_JOBS=1 CARGO_TARGET_DIR=target/p079-audit-r4 cargo test -p engine --lib p079 -- --nocapture` | Pass | 62 tests passed. |
| `CARGO_BUILD_JOBS=1 CARGO_TARGET_DIR=target/p079-audit-r4 cargo test -p acp --lib p079 -- --nocapture` | Pass | 18 tests passed. |
| `CARGO_BUILD_JOBS=1 CARGO_TARGET_DIR=target/p079-audit-r4 cargo test -p acp --test integration p079 -- --nocapture` | Pass | 13 tests passed. |
| `CARGO_BUILD_JOBS=1 CARGO_TARGET_DIR=target/p079-audit-r4 cargo test -p graphql-server --lib p079 -- --nocapture` | Pass | 4 tests passed. |
| `CARGO_BUILD_JOBS=1 CARGO_TARGET_DIR=target/p079-audit-r4 cargo test -p mcp-server --lib p079 -- --nocapture` | Pass | 5 tests passed. |
| `./scripts/test-gate.sh p079-swift-readback` | Failed before DTO assertions | Exit 65. App test host crashed before bootstrapping: `Chainworks Forge.debug.dylib` was denied by system policy from the temporary DerivedData bundle. |

## Readiness Checklist

- [x] P079 report written beside the proposal as a new audit round.
- [x] Track 1 proposal conformance separated from Track 2 specialist review.
- [x] Security-sensitive helper result included.
- [x] Current targeted Rust evidence captured.
- [x] Dirty worktree caveat documented.
- [ ] Full `proposal-079|p079` gate passed.
- [ ] Swift `p079-swift-readback` gate passed in this audit environment.
- [ ] Production same-session repair implemented for production providers.
- [ ] Transport-attributed transcript/provider-envelope recovery implemented.
- [ ] Controlled provider fallback dispatch implemented.
- [ ] macOS P079 inspector/status UI integrated.

## Final Action

Do not close or retire P079. Treat the current implementation as a useful partial foundation with tested repair evidence, lease, metric, security-posture, and readback slices. The residual work above must be completed or explicitly split into owned follow-up proposals before the reference docs can become stable implemented-system truth.
