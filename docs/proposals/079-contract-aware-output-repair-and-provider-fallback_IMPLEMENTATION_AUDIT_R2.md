# P079 Implementation Audit R2: Contract-Aware Output Repair and Provider Fallback

## Metadata

| Field | Value |
| --- | --- |
| Proposal | `docs/proposals/079-contract-aware-output-repair-and-provider-fallback.md` |
| Proposal ID | P079 |
| Proposal revision | `p079-contract-aware-output-repair-and-provider-fallback-r5` |
| Audit timestamp | 2026-06-20 13:01:07 EEST |
| Implementation target | Current worktree in `/Users/user/Documents/Chainworks Forge` |
| HEAD | `3168e9d93d3c7ddcb1c578c9c72953a29efff844` |
| Compare base | Implicit current worktree; no PR/range supplied |
| Worktree status | Dirty before this audit. Multiple unrelated P083/P086/P058 files and previous untracked audit reports were present; P079 conclusions below are limited to the P079 proposal and directly relevant implementation/readback/gate evidence. |
| Prior proposal-review reuse | Not reused. `discover_prior_review.py` returned no prior proposal-review artifacts for P079. Existing `*_IMPLEMENTATION_AUDIT_*` files were ignored for reviewer selection per audit rules. |
| Overall conformance | Not Implemented |
| Overall implementation readiness | Not Ready |
| Audit confidence | High for the blocking gaps; medium for exhaustive inventory because unrelated dirty worktree changes add routing noise. |

## Proposal State and Contract Summary

Proposal state: Active / draft for implementation review. The proposal is stored as a JSON wrapper whose `document_markdown` plus inline schemas, migration appendix, rollout contract, and readback contract define the audited contract.

P079 commits the system to recover output-contract failures before blocking a run, while preserving artifact truth and avoiding unsafe continuation. The key promised behavior is:

- Start only after ordinary output collection/validation fails with closed eligible failure classes.
- Attempt at most one same-session corrective turn in the same live ACP session.
- Recover valid outputs from the current transcript/provider envelope when transport-attributed evidence proves current-execution ownership.
- Dispatch at most one controlled provider fallback from a frozen YAML policy when recovery/repair cannot satisfy the contract.
- Preserve exact frozen canonical paths, source-generation truth, and validation settlement authority.
- Persist typed evidence in SQLite and expose the same evidence through run reports, MCP, GraphQL, and Swift DTO/readback surfaces.
- Enforce permission posture: auto-grant only canonical `fs.write` output paths; deny all other tool/shell/network/write requests; fail closed when the boundary is not enforceable.
- Include bounded plan evidence, bounded transcript recovery, redaction, metrics, rollout flags, deterministic fixtures, and Swift readback gates.
- Exclude release/publish/upload/distribution/git-push lanes and never treat plan files, stale artifacts, prior attempts, or provider-self-declared attribution as output truth.

## Platform and Product Scope

Apple scope: macOS readback/UI. The implementation includes Swift DTO and presenter tests; the proposal also commits a macOS inspector/progress/readback UX that remains deferred.

Backend/service scope: Rust control-plane engine, ACP transport, SQLite persistence, GraphQL, MCP, run-report readback, feature flags, rollout fixtures, and security/reliability behavior.

## Selected Reviewers

| Reviewer | Why selected |
| --- | --- |
| `chainworks_execution_truth_reviewer` | P079 changes run/stage/agent-output truth, artifact settlement, repair/fallback authority, and evidence ownership. Repo-local reviewer is mandatory for these boundaries. |
| `rust_reliability_reviewer` | Lease state, idempotency, replay, cancellation, prompt-sent ordering, restart recovery, and fallback single-flight behavior are core proposal commitments. |
| `api_contract_reviewer` | P079 adds/changes GraphQL, MCP, run-report, Swift DTO, JSON schema, enum, and readback contracts. |
| `observability_rollout_reviewer` | P079 includes migrations, feature flags, rollout fixtures, metrics, partial gate semantics, rollback/readback, and operational evidence requirements. |
| `rust_security_reviewer` | P079 handles untrusted provider output, transcript parsing, filesystem writes, path traversal/symlinks, redaction, principal binding, ACP permission posture, and public/semi-public readback. |

Rejected close alternatives:

- `rust_arch_reviewer`: displaced by the repo-local execution-truth reviewer plus reliability/API/rollout/security under the hard cap.
- `macos_ui_reviewer` and `apple_ux_reviewer`: the proposal has UI commitments, but the current implementation has no full macOS inspector UI surface to inspect. The missing UI is captured as conformance/readiness scope and should receive a platform pass before a future Ready verdict.
- `rust_performance_reviewer`: resource caps were reviewed under security/reliability. No throughput/latency benchmark claim is ready to validate; add this lens before a future successful readiness audit if transcript/fallback parsing becomes hot-path production code.
- `product_reviewer`: product metrics exist in the proposal, but the current blocker is implementation/rollout completeness rather than metric strategy.

## Primary Implementation Flows

1. Normal agent output collection fails validation, then P079 decides whether recovery/repair/fallback is eligible.
2. Same-session repair inserts durable event and lease rows, enforces permission posture, sends one corrective ACP prompt, validates outputs, and settles the parent execution.
3. Transcript/provider-envelope recovery scans bounded current-execution evidence, accepts only transport-attributed valid payloads, and skips repair when successful.
4. Provider fallback dispatches a fresh child execution from frozen YAML policy, with sanitized context packet, principal binding, single-flight lease, and parent-child settlement.
5. Operators and clients read the same evidence through SQLite projections, run report, MCP, GraphQL, Swift DTO/presenter, and macOS inspector UI.

## Fidelity and Divergence Inventory

Matches:

- SQLite migration `095_p079_output_contract_repair.sql` creates evidence, lease, and fallback-link authority tables with closed enums, uniqueness, TTL, principal, and dispatch-commit fields.
- Domain and DB repository slices exist and pass focused tests for event rows, leases, status transitions, reclamation, and settlement enums.
- Engine P079 lane is feature-flagged and fail-closed by default.
- Deterministic fixture same-session repair path is wired and exercised by `invoke_agent_repairs_missing_required_output_in_same_live_session`.
- ACP permission-posture helper tests cover canonical path allow/deny and `allow_once` selection for voluntary permission requests.
- Swift DTO/presenter readback tests pass for old runs, nil evidence, recovered/skipped/blocked/cancelled/stale states, unknown enums, lease identity, progress chip state, permission decisions, and safe relative paths.
- GraphQL and MCP P079 readback/sanitization tests pass.
- Junie plan-evidence capture/redaction/path containment code exists with size caps and dirfd/openat protections.

Divergences:

- Production same-session repair is intentionally fail-closed for current production providers because permission enforcement is advisory only.
- Transcript/provider-envelope recovery never accepts recovered output; it returns `Unavailable`/bounded diagnostics until transport-attributed chunk scanning exists.
- Controlled provider fallback dispatch is not wired; the engine writes `provider_fallback_json: None` and comments that YAML `output_repair_policies` parsing is missing.
- Required P079 metrics are deferred.
- Required standalone reference docs `p079-repair-prompt-template.md`, `p079-recovery-attribution.md`, and `p079-adapter-idempotency.md` are absent.
- The proposal's macOS inspector UI remains deferred; current Swift evidence is DTO/presenter fixture coverage only.
- Most rollout-contract negative fixtures under `docs/evidence/rollout-contract/p079` are still placeholder fixtures.
- The canonical `proposal-079` gate passes, but repo documentation explicitly defines it as a partial-acceptance gate, not a full proposal acceptance gate.

Ambiguities / evidence gaps:

- The dirty worktree contains unrelated P083/P086/P058 changes, so helper-generated surface lists are noisy.
- No live-provider runtime validation was run or required; P079 acceptance requires deterministic fixture coverage only, but production-provider behavior remains absent by design.
- Some API readback surfaces can decode or expose states that the active backend cannot yet produce because recovery/fallback successful paths are missing.

## Residual Scope / Follow-up Ownership

| Residual item | Proposal owner found? | Blocks conformance/readiness? | Notes |
| --- | --- | --- | --- |
| Production same-session repair with enforceable permission boundary | No | Yes | Current providers are advisory-only and dispatch fails closed. |
| Accepted transcript/provider-envelope recovery with transport attribution | No | Yes | Current function records bounded unavailable evidence only. |
| Controlled provider fallback from frozen YAML policy | No | Yes | Fallback policy parsing and child dispatch are absent. |
| Fallback context packet used by active dispatch path | No | Yes | Schema exists, but successful fallback lane is absent and fixtures are placeholders. |
| Full projection rebuild and bounded recovery sweep for P079 evidence artifacts | No | Yes | Repo reference marks this deferred. |
| Release-lane and source-generation supersession exclusions | No | Yes | Repo reference marks these deferred. |
| P079 operational metric emission and metric label linting | No | Yes | Metrics are proposed and documented as deferred. |
| macOS inspector UI and related UX/accessibility/readback states | No | Yes | Swift DTO/presenter exists; inspector UI remains deferred. |
| Required standalone P079 reference docs | No | Yes | `find docs/reference -name 'p079-*'` returned no files. |
| Non-placeholder rollout fixtures for reliability/security/negative cases | No | Yes | 56 fixture files exist, but many negative/Swift files still contain `placeholder_fixture_kind`. |

## Specialist Coverage Matrix

| Surface | Trigger | Required lens | Audit coverage |
| --- | --- | --- | --- |
| Execution truth / artifact settlement | P079 repair/fallback changes output settlement authority | `chainworks_execution_truth_reviewer` | Covered; blocking gaps recorded in READY/API/REL findings. |
| Reliability / lifecycle | Leases, idempotency, cancellation, restart, fallback single-flight | `rust_reliability_reviewer` | Covered; incomplete runtime/recovery lanes recorded. |
| API/readback | GraphQL, MCP, run report, Swift DTO, schemas/enums | `api_contract_reviewer` | Covered; synthetic-vs-producible state gap recorded. |
| Rollout/ops | Migration, flags, metrics, fixtures, gate, rollback | `observability_rollout_reviewer` | Covered; metrics/docs/fixtures/gate partiality recorded. |
| Security | Auth/principal, public ingress, redaction, path/symlink, ACP permission, parser/resource bounds | `rust_security_reviewer` | Covered; no Ready verdict because successful production path lacks enforceable boundary. |
| macOS UI/UX | Proposal UI commitments | macOS UI/UX reviewer | Not selected under hard cap because no implemented inspector UI surface exists; required before any future Ready verdict. |
| Performance/resource limits | Transcript/packet bounds and parser caps | performance reviewer | Not selected under hard cap; resource-exhaustion aspects reviewed by security/reliability. Required before a future Ready verdict if production parsing/dispatch is implemented. |

## Requirement Summary

| Status | Count |
| --- | ---: |
| Implemented | 2 |
| Partially Implemented | 9 |
| Missing | 4 |
| Not Verifiable | 0 |
| Out of Scope | 0 |

Because in-scope successful transcript recovery, provider fallback, metrics, and required docs/fixtures are missing without a concrete follow-up proposal owner, overall conformance is `Not Implemented`.

## Detailed Requirement Audit

| ID | Requirement | Source | Status | Evidence and gap |
| --- | --- | --- | --- | --- |
| REQ-001 | Enter P079 only after normal output-contract failure with closed eligible failure classes. | Architecture / Eligible Failure Classes / Acceptance Criteria | Implemented | Engine invokes P079 after validation summary requires repair (`executor.rs:10052-10065`) and records skip classes for provider failures before assistant output (`executor.rs:10073-10103`). |
| REQ-002 | Same-session repair: at most one corrective turn in the same live ACP session, no rerun work, durable atomic settlement. | Same-Session Repair / Repair Prompt / Acceptance Criteria | Partially Implemented | Fixture path passes; event+lease atomic insert exists (`executor.rs:10396-10446`), prompt-sent transition precedes dispatch (`executor.rs:11028-11060`), and `proposal-079` gate passed. Production providers fail closed because permission posture is advisory (`executor.rs:10924-10983`, `executor.rs:18889-18904`). |
| REQ-003 | Permission posture: only canonical output-path writes are auto-granted; all other tool/shell/network/write requests denied; applies to repair and fallback. | Permission Posture / Security hold conditions | Partially Implemented | ACP posture tests pass for voluntary permission requests, and production dispatch fails closed when unenforceable. The promised successful production permission boundary does not exist yet (`executor.rs:10924-10983`, `executor.rs:18889-18904`), and fallback is not active. |
| REQ-004 | Transcript/provider-envelope recovery accepts valid current-execution outputs only with transport-allocated attribution and bounded parsing. | Transcript and Provider-Envelope Recovery / Recovery Bounds | Missing | `p079_attempt_transcript_recovery` documents that transport-derived attribution is not implemented and always returns unavailable/unattributable for declared outputs (`executor.rs:19818-19893`). |
| REQ-005 | Controlled provider fallback dispatch from frozen YAML policy, fresh child execution, same output contract, no failed-execution truth mutation, single-flight policy hash. | Provider Fallback / Fallback Policy YAML / Acceptance Criteria | Missing | Engine persists `provider_fallback_json: None` and comments that YAML `output_repair_policies` parsing is missing (`executor.rs:10369-10380`). Migration has fallback-link tables, but active fallback dispatch is absent. |
| REQ-006 | Fallback context packet schema, redaction, size caps, packet hash, principal binding, negative fixtures before enabling fallback. | Fallback Context Packet / Fallback Policy YAML | Partially Implemented | Domain/migration fields exist (`095_p079_output_contract_repair.sql:187-208`), but active fallback dispatch is absent and rollout negative fixtures remain placeholders. |
| REQ-007 | Exact canonical artifact path binding and source-generation truth; no normalization, stale output, wrong path, or provider-declared attribution acceptance. | Canonical Artifact Path Binding / Goals / Non-Goals | Partially Implemented | Base validation and repair prompt include canonical targets, and path containment tests pass. The full P079 recovery/fallback success paths that must enforce the invariant are missing, and release/source-generation exclusions are documented deferred. |
| REQ-008 | Provider plan evidence is bounded, copied into P079-owned protected storage, redacted, meta-root-relative, and never output truth. | Plan Evidence / Junie | Implemented | Plan evidence collection caps/redacts and writes 0600 under protected storage (`executor.rs:18947-19007`); gate and engine tests cover path/redaction hardening. |
| REQ-009 | Persistence authority: events, leases, fallback links, projection integrity, rebuild attempts, rollback-readable rows. | Persistence / SQLite Migration Appendix | Partially Implemented | Migration creates the authority schema (`095_p079_output_contract_repair.sql:9-214`) and DB tests pass. Full projection artifact rebuild plus bounded background sweep remain deferred in repo reference docs. |
| REQ-010 | Reliability state machine: prompt-sent no re-prompt, TTL/CAS lease reclamation, restart, cancellation, lost ACK, source supersession, fallback child restart. | Reliability / Acceptance Criteria | Partially Implemented | DB/engine/ACP tests cover selected lease and fixture repair behavior. Full runtime restart/sweep, fallback child restart, release-lane exclusion, and source-generation supersession remain deferred or unproven. |
| REQ-011 | Readback parity through GraphQL, MCP, run report, and Swift DTO/gate, including null old runs and unknown enum fallback. | Evidence and Readback / Swift Client Migration | Partially Implemented | GraphQL/MCP/Swift slices pass and typed GraphQL objects exist (`stage.rs:685-720`, `reports.rs:986-1030`). However readback can decode recovered transcript/fallback states that active backend cannot produce yet. |
| REQ-012 | macOS inspector/progress/readback UX: chip, grouping, copy/reveal, stale/unknown diagnostics, accessibility, notifications. | UX/UI Notes / Presentation Polish / Swift Client Migration | Partially Implemented | Swift presenter tests cover progress chip, unknown enum, stale projection, identity, and safe paths. The macOS inspector UI and notifications are documented deferred (`test-gates.md:2050`). |
| REQ-013 | P079 metrics and auto-retry observe-only rollups with safe labels. | Metrics / Rollout / Acceptance Criteria | Missing | Repo reference says P079 metric emission remains deferred (`output-contracts-failure-evidence-and-recovery.md:786-787`, `test-gates.md:2050`). Placeholder metric-label fixtures remain. |
| REQ-014 | Rollout gates, fixtures, flags, reference docs, local deterministic acceptance gate. | Rollout / Test Plan / Acceptance Criteria | Partially Implemented | Feature flags and `proposal-079` gate exist and pass (`scripts/test-gate.sh:8088-8160`), but the gate is explicitly partial and required reference docs are absent (`test-gates.md:2027-2052`). Many rollout fixtures are placeholders. |
| REQ-015 | Release/publish/upload/distribution/git-push lanes excluded; no live providers/network; no legacy/stale output scanning. | Non-Goals / Acceptance Criteria | Missing | Deterministic no-live-provider gate exists, but release-lane and source-generation supersession eligibility exclusions are documented deferred (`output-contracts-failure-evidence-and-recovery.md:765`). |

## Reviewer / Lens Scorecard

| Lens | Conformance result | Readiness result | Top risk | Confidence |
| --- | --- | --- | --- | --- |
| Execution truth | Not implemented | Not ready | Primary successful recovery/fallback truth paths absent. | High |
| Reliability | Partial | Not ready | Lease/restart/fallback reliability not complete beyond fixture and DB slices. | High |
| API contract | Partial | Not ready | Readback states are partly synthetic until backend can produce them. | High |
| Rollout/ops | Partial | Not ready | Partial gate passes but metrics/docs/fixtures/full acceptance are deferred. | High |
| Security | Partial | Not ready | Fail-closed posture avoids unsafe dispatch, but promised production repair cannot run without enforceable boundary. | High |
| macOS UI/UX | Partial | Not ready | DTO/presenter exists; inspector UI remains deferred and not platform-reviewed. | Medium |

## Security-Sensitive Diff Scan Summary

The security hard gate triggered. Helper categories included `auth`, `dos_resource_limits`, `filesystem_subprocess_boundary`, `parser_boundary`, `public_ingress`, `secrets_redaction_privacy`, and `unsafe_crypto_dependency`. The helper file list was noisy because unrelated dirty worktree files were present, but P079 itself clearly touches security-sensitive surfaces:

- ACP permission handling and subprocess/provider boundaries.
- Transcript/provider-envelope parsing and size limits.
- SQLite and public readback of repair/fallback evidence.
- MCP/GraphQL/run-report redaction and path exposure.
- Plan-evidence filesystem containment, symlink/hard-link rejection, and openat/dirfd materialization.
- Principal binding for repair/fallback leases.

Manual security pass result: the implemented slice is conservative and fail-closed for production providers, and the focused P079 security tests in engine/ACP/GraphQL/MCP pass. No additional Critical exploitable issue was found in the active implemented slice during this audit. Readiness remains blocked because the proposal requires a successful production repair/fallback path with enforceable permission posture; the current implementation avoids the unsafe path by not implementing it.

## Routed Specialist Findings

### READY-001: Primary P079 success paths remain unimplemented

- Reviewer: `chainworks_execution_truth_reviewer`
- Severity: Critical
- Confidence: High
- Related requirements: REQ-002, REQ-004, REQ-005, REQ-007, REQ-015
- Evidence: `docs/reference/test-gates.md:2027-2052`, `docs/reference/output-contracts-failure-evidence-and-recovery.md:759-765`, `control-plane/crates/engine/src/executor.rs:10924-10983`, `control-plane/crates/engine/src/executor.rs:19818-19893`
- Why it matters: P079's primary service value is to recover output-contract failures before blocking a run. The implementation currently has fixture repair and fail-closed diagnostics, but production same-session repair, accepted transcript/provider-envelope recovery, and controlled provider fallback are not active.
- Recommended action: Implement the successful production paths or move each omitted path into a concrete follow-up proposal with acceptance gates.
- Acceptance criteria: Production-capable repair runs under enforceable permissions; transcript/provider-envelope recovery can accept transport-attributed valid payloads; fallback dispatch uses frozen YAML policy and settles parent/child truth; all pass deterministic P079 tests and reference docs are updated.

### SEC-001: Permission posture is safe only because production repair is blocked

- Reviewer: `rust_security_reviewer`
- Severity: Major
- Confidence: High
- Related requirements: REQ-002, REQ-003, REQ-005
- Evidence: `control-plane/crates/engine/src/executor.rs:10924-10983`, `control-plane/crates/engine/src/executor.rs:18889-18904`
- Why it matters: The proposal requires successful repair/fallback under restricted permissions. Current production providers run with full-access/bypass-style behavior where the transport can intercept only voluntary permission requests. The implementation correctly fails closed, but that means the required successful production path is absent.
- Recommended action: Add a real provider/runtime permission boundary or keep production repair disabled and explicitly re-scope the proposal.
- Acceptance criteria: Production providers cannot write outside frozen output paths during repair/fallback, non-canonical writes are denied server-side, and tests prove bypass attempts cannot mutate outputs.

### REL-001: Reliability matrix is incomplete for restart/replay/fallback paths

- Reviewer: `rust_reliability_reviewer`
- Severity: Major
- Confidence: High
- Related requirements: REQ-005, REQ-009, REQ-010
- Evidence: `docs/reference/test-gates.md:2050`, `docs/evidence/rollout-contract/p079/negative/*` placeholder fixtures, `control-plane/crates/db/migrations/095_p079_output_contract_repair.sql:117-208`
- Why it matters: The DB lease schema is present, but the runtime paths that need the most replay protection, especially fallback child dispatch, recovery sweep, lost ACK, supersession, and cancellation interactions, are not implemented or fully proven.
- Recommended action: Wire the recovery sweeps and fallback child lifecycle before enabling P079 beyond fixtures.
- Acceptance criteria: Non-placeholder tests cover reserved and prompt-sent restart, lost ACK, fallback duplicate lease, fallback child restart settlement, cancellation, source supersession, and lease reclamation without double dispatch.

### API-001: Readback surfaces expose/decode states the backend cannot yet produce

- Reviewer: `api_contract_reviewer`
- Severity: Major
- Confidence: High
- Related requirements: REQ-004, REQ-005, REQ-011
- Evidence: `Chainworks ForgeTests/Proposal079ContractRepairReadbackTests.swift`, `control-plane/crates/engine/src/executor.rs:10369-10380`, `control-plane/crates/engine/src/executor.rs:19838-19893`, `control-plane/crates/mcp-server/src/tools/reports.rs:986-1030`
- Why it matters: DTO/readback compatibility is useful, but parity requires the same values to be producible by the authoritative backend. Today recovered transcript and provider fallback fixtures are client/schema exercises, not end-to-end runtime evidence.
- Recommended action: Add integration tests that create each recovered/fallback state through the engine, then verify GraphQL, MCP, run-report, and Swift fixture parity against those rows.
- Acceptance criteria: Backend-generated evidence rows cover same-session repair, transcript recovery, provider-envelope recovery, fallback accepted, fallback rejected, blocked, cancelled, stale, and orphan cases, and all readback channels expose equivalent values.

### OPS-001: Rollout acceptance remains partial despite a passing gate

- Reviewer: `observability_rollout_reviewer`
- Severity: Major
- Confidence: High
- Related requirements: REQ-013, REQ-014
- Evidence: `docs/reference/test-gates.md:2027-2052`, `docs/reference/output-contracts-failure-evidence-and-recovery.md:759-765`, `find docs/reference -name 'p079-*'`, `rg placeholder_fixture_kind docs/evidence/rollout-contract/p079`
- Why it matters: The canonical gate now passes, but it is explicitly a partial-acceptance proof. Metrics, reference docs, production lanes, projection rebuild, UI, and many negative fixtures remain deferred, so the proposal cannot be closed or considered ready.
- Recommended action: Convert the gate from partial proof to full acceptance only after removing placeholder fixtures, adding metrics/docs, and wiring the missing lanes.
- Acceptance criteria: `proposal-079` documentation no longer labels the gate partial, all rollout fixtures are executable/non-placeholder, required reference docs exist, P079 metrics emit with safe labels, and the gate covers every acceptance criterion.

### READY-002: Successful future readiness needs macOS UI/UX and performance coverage

- Reviewer: audit coverage gate
- Severity: Major
- Confidence: Medium
- Related requirements: REQ-012, REQ-013, REQ-014
- Evidence: Proposal UI commitments, helper fingerprint output, and deferred UI/performance-related rollout scope.
- Why it matters: This audit is already Not Ready, so the hard cap prioritized execution truth, reliability, API, rollout, and security. Before any future Ready or Ready-with-Risks verdict, the missing inspector UI and production parser/packet bounds should receive dedicated platform/performance review if implemented.
- Recommended action: Add `macos_ui_reviewer` or `apple_ux_reviewer` once the inspector UI exists, and add `rust_performance_reviewer` if production transcript/fallback parsing is enabled on hot paths.
- Acceptance criteria: Future successful audit records those passes or explicitly explains why the proposal has been re-scoped to remove those surfaces.

## Readiness Checklist

| Check | Result |
| --- | --- |
| Canonical P079 gate | Passed on current tree: `./scripts/test-gate.sh proposal-079`. The gate itself is documented as partial acceptance. |
| Full repository gate | Not run. A full repo sign-off was not required for this Not Ready verdict; same-tree P079 gate passed. |
| Core runtime flow validation | Partial. Deterministic fixture same-session repair passes; production repair, accepted transcript/provider-envelope recovery, and provider fallback are absent. |
| API/readback validation | Partial. GraphQL/MCP/Swift tests pass, but some states are synthetic/decode-only until backend lanes exist. |
| UI/UX validation | Partial. Swift DTO/presenter tests pass; macOS inspector UI not implemented or runtime-validated. |
| Accessibility/localization | Not fully validated. Proposal UI commitments remain unimplemented. |
| Privacy/permissions/security | Partial. Implemented slice reviewed and tested; successful production repair/fallback permission boundary missing. |
| Rollout/metrics/docs | Not ready. Metrics, required P079 reference docs, and many non-placeholder fixtures are missing. |

## Verification Log

Commands run during this audit:

- `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/discover_prior_review.py .../079-contract-aware-output-repair-and-provider-fallback.md` -> no artifacts.
- `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/report_path.py .../079-contract-aware-output-repair-and-provider-fallback.md` -> `docs/proposals/079-contract-aware-output-repair-and-provider-fallback_IMPLEMENTATION_AUDIT_R2.md`.
- `git status --short`, `git diff --stat`, `git rev-parse HEAD` -> dirty worktree recorded, HEAD `3168e9d93d3c7ddcb1c578c9c72953a29efff844`.
- `find docs/reference -maxdepth 1 -name 'p079-*' -print` -> no required standalone P079 reference docs found.
- `find docs/evidence/rollout-contract/p079 -type f | wc -l` -> 56 files.
- `rg -n "placeholder_fixture_kind|PLACEHOLDER|TODO|deferred|advisory|fixture-only" docs/evidence/rollout-contract/p079 -g '*.json'` -> many placeholder fixture markers.
- `python3 .../security_sensitive_diff.py --root ... --json` -> triggered security-sensitive categories; manual security pass completed for P079 surfaces.
- `python3 .../implementation_surface_fingerprint.py --root ... --json` -> triggered API, Apple UI/UX, architecture, observability, performance, reliability, and security surfaces; results treated as noisy due unrelated dirty worktree.
- `./scripts/test-gate.sh proposal-079` -> passed on retried current-tree run. Key executed P079 slices included domain 9 tests, DB 13 tests, DB integration 6 tests, engine lib 61 P079 tests, engine fixture integration 1 test, ACP 18 lib + 13 integration tests, GraphQL 4 tests, MCP 5 lib + 2 runtime-facts tests, and Swift DTO/presenter 25 tests. Non-fatal Rust warnings, duplicate Yams class warnings, and macOS `linkd` warnings were observed.

## Final Verdict

Overall conformance: Not Implemented.

Overall readiness: Not Ready.

P079 has a meaningful partial implementation: schema, DB authority, readback surfaces, fail-closed guards, deterministic fixture repair, plan-evidence hardening, and a passing partial gate. It does not satisfy the proposal because the proposal's core successful paths remain absent: production same-session repair with enforceable permissions, accepted transcript/provider-envelope recovery, controlled provider fallback from frozen YAML policy, operational metrics, full projection/recovery sweep, macOS inspector UI, required reference docs, and non-placeholder rollout fixtures.

Recommended next actions:

1. Decide whether P079 still owns all missing successful paths. If yes, implement them before closeout. If no, create concrete follow-up proposal files and revise P079 scope before treating residual work as non-blocking.
2. Implement enforceable production repair permissions before enabling same-session repair beyond deterministic fixtures.
3. Wire transcript/provider-envelope recovery with transport attribution and provider fallback from frozen YAML policy, then prove each through backend-generated rows and cross-channel readback parity.
4. Replace placeholder fixtures, add P079 metrics and required docs, and update the `proposal-079` gate from partial acceptance to full acceptance only when every acceptance criterion is covered.
