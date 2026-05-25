# Proposal 081 Implementation Audit R3

## Metadata

| Field | Value |
|---|---|
| Proposal | `docs/proposals/081-boundary-first-api-auth-contract-matrix.md` |
| Proposal state | Active (`Status: revised_for_review_blocker_closure`) |
| Audit timestamp | 2026-05-24T19:33:39+0300 |
| Repo root | `/Users/user/Documents/Chainworks Forge/.chainworks/worktrees/cw-implement-proposal-081-boundar-4dd7c886` |
| Implementation target | Current worktree |
| Current SHA | `45bc91a8b226ce34965ab1f1dc62eedc66dfcc1f` |
| Compare base | Implicit current worktree audit; no PR/base supplied |
| Working tree status at audit start | Clean |
| Overall Conformance | Partial |
| Overall Implementation Readiness | Not Ready |
| Reviewer Selection Reuse | Not reused |
| Audit Confidence | High for contract/code/test evidence; Medium for live runtime/macOS delivery claims |

## Prior Proposal-Review Reuse

`discover_prior_review.py` returned no adjacent or repo-local proposal-review artifacts for P081. Prior implementation audits were intentionally ignored for reviewer selection per the skill rules. Reviewer selection is therefore `Not reused`.

## Selected Reviewers

| Reviewer | Reason |
|---|---|
| `rust_arch_reviewer` | Shared BoundaryPolicy ownership, daemon injection, crate boundaries, request-path file I/O boundary |
| `rust_reliability_reviewer` | SQLite write units, idempotency, committed-unack recovery, safe-mode and shutdown behavior |
| `api_contract_reviewer` | GraphQL/MCP error shapes, WebSocket close codes, redaction extensions, readback schema |
| `observability_rollout_reviewer` | Metrics, canary/shadow coverage, operator readback, rollout hold conditions |
| `macos_ui_reviewer` | macOS native alert delivery, status item, disabled actionability, accessibility parity |

Rejected close alternatives:

| Reviewer | Rejection reason |
|---|---|
| `rust_security_reviewer` | Security evidence is relevant, but P081 remaining blockers are mainly contract/readiness/observability; security-specific checks were sampled under Rust/API lenses. |
| `apple_arch_reviewer` | Swift work is mostly service/model support; macOS UI/accessibility evidence is the more relevant Apple lens. |
| `product_reviewer` | No new product decisioning or experiment surface is needed beyond rollout/readiness. |
| `performance_reviewer` | No explicit p99/throughput benchmark target beyond operational histograms; gaps are tracked under observability/readiness. |

## Proposal Contract Summary

P081 defines a boundary-first auth/API contract across the Rust control plane and macOS operator shell. Explicit commitments include a machine-readable boundary matrix, one immutable daemon-injected `BoundaryPolicy`, caller classification, GraphQL/MCP authorization behavior, audit-log durability, idempotency, bounded readback, operator alerts, macOS redaction/actionability/accessibility behavior, rollout metrics, canaries, safe-mode recovery, and reliability proof.

Platform/product scope:

| Scope | Classification |
|---|---|
| Apple | macOS |
| Backend/service | Rust control-plane service, GraphQL, MCP, SQLite persistence, daemon lifecycle |
| Cross-stack | macOS Swift client reads GraphQL/operator alert/redaction contracts from Rust daemon |

Primary implementation flows audited:

1. Operator/agent/observer request is classified into `CallerClass`, evaluated by the shared `BoundaryPolicy`, and allowed, denied, or redacted consistently.
2. GraphQL HTTP/WebSocket calls return deterministic errors, close codes, and `extensions.redactions`; observer read-only opt-in receives redacted data without response-level errors.
3. State-changing MCP/approval calls use idempotency and command-journal linkage without duplicate durable side effects.
4. Audit rows/checkpoints and bounded runtime/operator readback expose health without raw audit table browsing.
5. Swift decodes redaction/operator-alert readback and drives macOS native attention, disabled actionability, and accessibility metadata.

## Fidelity Inventory

### Matches

- Matrix docs/JSON are present and linked from `docs/reference/README.md:41-42` and `docs/README.md:76`.
- `scripts/test-gate.sh` wires `scripts/check-boundary-coverage.sh` into `guardrails` at `scripts/test-gate.sh:2311-2322` and into `proposal-081` at `scripts/test-gate.sh:6191-6196`.
- Boundary fixture validation rejects unknown fields/enums, required-row mismatches, wildcard misuse, and deny-side-effect conflicts in `control-plane/crates/auth/src/boundary/mod.rs`.
- The daemon constructs one shared `BoundaryPolicy` and injects it into MCP and GraphQL at `control-plane/crates/daemon/src/main.rs:652-685`.
- GraphQL observer field redaction now exists via `P081GraphqlRedactionCollector` at `control-plane/crates/graphql-server/src/schema.rs:39-123` and the observer alert redaction test at `control-plane/crates/graphql-server/src/schema.rs:7710-7773`.
- MCP idempotency now has pending-sentinel preclaim, command-journal linkage, replay, and committed-unack recovery tests at `control-plane/crates/mcp-server/src/server.rs:710-780` and `:4064-4263`.
- Swift unit tests cover typed redaction decoding, redaction accessibility metadata, operator alert lifecycle/native delivery model, actionability false, and silence behavior at `Chainworks ForgeTests/Proposal031ThinGraphQLReadBoundaryTests.swift:6-244`.

### Divergences

- The proposal metrics list is broader than the implementation. `control-plane/crates/db/src/metrics.rs:95-107` declares only a subset of P081 metrics; searches found no implementation references for `auth_ambiguous_caller_warn_total`, `boundary_no_op_label_total`, `audit_log_rate_limited_total`, `approval_idempotency_duplicate_total`, `boundary_commit_transaction_latency_ms`, `audit_budget_cleanup_duration_ms`, or `operator_alert_clear_latency_ms`.
- `boundaryRuntime.auditLogHealth` exposes row/checkpoint/integrity fields, but not all proposal-required fields: audit writability, retention/cleanup state, budget/used bytes, and shadow coverage report refs are not present in `control-plane/crates/graphql-server/src/schema.rs:394-408`.
- The canary artifact exists, but the gate validator is a lightweight text/subset check in `scripts/test-gate.sh:6241-6307`, not a schema-equivalent YAML validator.
- Reliability proof for several proposal-named cases is partly token/inventory based in `scripts/test-gate.sh:6309-6338`, not direct scenario tests for all cases.
- macOS hidden/inactive alert delivery and Full Keyboard Access / Increase Contrast / Reduce Motion coverage are represented by model/service unit tests, not by live hidden-window or accessibility UI execution.

### Ambiguities / Evidence Gaps

- The audit did not run a live daemon restart/reconnect exercise or remote UI smoke. P081 gate explicitly says it must not require live daemon startup or UI smoke hosts (`docs/proposals/081...md:760-761`), so this is a readiness gap for closeout, not a gate failure.
- Shadow coverage report rows are canary-covered with zero live observations (`docs/evidence/boundary-policy-shadow-coverage/report.json:15-27`). The proposal permits canary coverage before enforce, but live observation quality remains unproven.

## Requirement Summary

| Status | Count |
|---|---:|
| Implemented | 15 |
| Partially Implemented | 10 |
| Missing | 0 |
| Not Verifiable | 0 |
| Out of Scope | 0 |

## Detailed REQ Audit

| ID | Requirement | Proposal source | Status | Evidence and notes |
|---|---|---|---|---|
| REQ-001 | Matrix reference doc and JSON fixture exist and are linked. | Acceptance `:994` | Implemented | `docs/reference/boundary-first-api-auth-contract.{md,json}` exist and are linked from `docs/reference/README.md:41-42` and `docs/README.md:76`. |
| REQ-002 | Fixture validator rejects missing fields, duplicate rows, unknown enums/fields, invalid schema, wildcard misuse, required-row transport mismatch, and missing required rows. | Acceptance `:995` | Implemented | Typed fixture structs use `deny_unknown_fields`; validation covers required rows, enum registries, wildcard misuse, and deny-side-effect conflicts in `auth/src/boundary/mod.rs`; `cargo test -p auth boundary::` passed 39 tests. |
| REQ-003 | Required rows validate against row-id grammar/transport enum and have executable coverage. | Acceptance `:996` | Partially Implemented | Required rows are validated and `proposal-081` checks all row ids. Coverage exists through canary rows, but example/coverage validation is not a full schema-equivalent validator. |
| REQ-004 | Validated embedded fixture and malformed deployed fixture enter read-only safe mode. | Acceptance `:997` | Implemented | Embedded fixture is `include_str!`; daemon malformed/oversize/path-rejected fixture paths fall back to `ReadOnlySafeMode` at `daemon/src/main.rs:300-360`. Auth boundary tests cover safe-mode denial and diagnostics. |
| REQ-005 | `audit_log` and `audit_log_checkpoints` migrations/repo/checkpoint/readback/retention/fail-closed behavior. | Acceptance `:998`, `:1019-1020` | Partially Implemented | Migrations `064`/`065` exist; `audit_log::append_tx`, standalone `append`, checkpoints, tamper verification, truncation, and 90-day cleanup are implemented. Readback/fail-closed/DoS evidence is not complete across all proposal fields and deny seams. |
| REQ-006 | Principal table v1/v2/v3 compatibility, strict versions, exact defaults, token redaction. | Acceptance `:999`, `:1021` | Implemented | `PrincipalTable::load_or_bootstrap` accepts known versions, rejects unknown versions, bootstraps v3, enforces v3 surface policies, 0600 file/0700 parent, no symlink/hard-link; focused auth tests passed. |
| REQ-007 | `CallerClass` and `CallerContext.caller_class`; all principal fixtures classify or explicitly deny. | Acceptance `:1000` | Implemented | CallerClass tests passed (`cargo test -p auth caller_class`); command journal caller-class migration exists. |
| REQ-008 | GraphQL, MCP, and approval actionability use the same daemon-injected `BoundaryPolicy`. | Acceptance `:1001`, `:1016`, rollout `:918` | Implemented | Daemon passes the same `Arc<BoundaryPolicy>` into MCP and GraphQL. GraphQL approval list applies policy actionability at `schema.rs:1074-1100`. Gate checks constructors at `scripts/test-gate.sh:6409-6423`. |
| REQ-009 | GraphQL deterministic HTTP/WebSocket extensions contract with camelCase fields. | Acceptance `:1002` | Implemented | HTTP redactions and WS close-code tests are in `graphql-server/src/server.rs`; `proposal-081` ran WebSocket 4401/4403/4408 and redaction tests successfully. |
| REQ-010 | MCP known-denied tools use `-32004`, unknown tools `-32601`, and initialize exposes capability signal. | Acceptance `:1003` | Implemented | MCP HTTP/server tests passed through `proposal-081`; operator readback fixture records denied-known and unknown codes. |
| REQ-011 | State-changing allowed calls use durable MCP idempotency preclaim, command-journal linkage, and committed-unack recovery. | Acceptance `:1004` | Implemented | `mcp_command_idempotency` pending sentinel is written before dispatch, command-journal id/key/row linkage is tested, and committed-unack recovery test passed. |
| REQ-012 | `approveApproval` / `rejectApproval` require idempotency, check terminal state under settlement transaction, and do not double-settle. | Acceptance `:1005` | Implemented | Approval idempotency table and transactional repo exist; `ResolveApproval` records command journal, settlement, idempotency, audit row, and completion in one transaction at `command_handler.rs:4262-4599`; Swift attempt-store tests passed. |
| REQ-013 | State-changing MCP commands require idempotency; read-only tools reject idempotency keys. | Acceptance `:1006` | Implemented | MCP server precheck rejects missing/invalid keys for state-changing calls and rejects idempotency keys on read-only calls at `mcp-server/src/server.rs:710-747`. |
| REQ-014 | Denial-side-effect tests prove zero command journal, approval settlement, and projection writes except declared audit rows. | Acceptance `:1007` | Partially Implemented | Fixture validator forbids command journal/approval settlement deny side effects and MCP denial returns before dispatch. A complete cross-surface side-effect sweep was not found in the gate output. |
| REQ-015 | Boundary coverage guardrail is wired into test-gate guardrails. | Acceptance `:1008` | Implemented | `scripts/test-gate.sh:2311-2322` invokes `check-boundary-coverage.sh`; `proposal-081` also invokes it and it passed. |
| REQ-016 | Security hardening tests cover principals, strict JSON, constant-time token comparison, expiry, token_id redaction, error non-disclosure, disabled break-glass, audit DoS, tamper evidence. | Acceptance `:1009` | Partially Implemented | Principal file hardening, strict schema, token_id derivation, payload size, and tamper evidence are present. The audited gate does not prove every named security case in one P081 acceptance sweep. |
| REQ-017 | `boundary-policy-canaries.yaml` has validator and contributes canary rows to shadow coverage schema. | Acceptance `:1010` | Partially Implemented | YAML exists at `docs/evidence/boundary-policy-shadow-coverage/boundary-policy-canaries.yaml` and report covers all 11 rows. The validator is a text/subset gate check rather than schema-equivalent YAML validation. |
| REQ-018 | SQLite contention, audit outage, subscription gap, safe-mode exit, SIGTERM drain, committed-unack recovery, and denial-audit backpressure are covered by reliability tests. | Acceptance `:1011` | Partially Implemented | SIGTERM drain and committed-unack tests are executed. Other items are partly represented by tokens/inventory in the gate, not direct scenario tests for every case. |
| REQ-019 | Operator alert contract has GraphQL/MCP readback, payload schema, severity/dedupe/silence/clear lifecycle, thresholds/windows, native delivery, hidden/inactive fires-and-clears tests. | Acceptance `:1012` | Partially Implemented | GraphQL/MCP readback and Swift model/service tests exist; hidden/inactive window and full native delivery execution were not proven by runtime/UI tests. |
| REQ-020 | `boundaryRuntime` and `audit_log_health` readback expose policy mode, safe mode, fixture digests, audit writability, integrity, retention/cleanup state, and shadow coverage refs without raw audit browser. | Acceptance `:1013` | Partially Implemented | Bounded readback exists and raw rows are absent. Missing/undetected fields include writability, retention/cleanup state, budget/used bytes, and shadow coverage refs. |
| REQ-021 | Swift approval mutations use `ApprovalActionAttemptStore`; typed GraphQL decoding preserves `extensions.redactions`. | Acceptance `:1014` | Implemented | `proposal-081` Swift tests passed for attempt-store reuse and redaction decoding. |
| REQ-022 | Accessibility parity tests cover redacted nil, ordinary nil, drop_resource, actionability_false, Full Keyboard Access, Increase Contrast, Reduce Motion. | Acceptance `:1015` | Partially Implemented | Unit tests cover redacted/ordinary/drop/actionability metadata. Full Keyboard Access, Increase Contrast, and Reduce Motion were not found as executed UI/accessibility tests. |
| REQ-023 | Baseline framing preserved: P081 changes auth/audit without changing GraphQL read/subscription plus approval-only UI boundary. | Acceptance `:1017` | Implemented | Proposal reference docs and guardrails preserve the P031/P072 boundary; no non-approval Swift mutation expansion found during audit. |
| REQ-024 | Request paths never read the fixture/docs directly; only startup validation/restart rebuilds policy inputs. | Acceptance `:1018` | Implemented | Boundary module states request paths never read artifacts directly; daemon startup reads deployed/embedded fixture and injects immutable policy. Request handlers receive policy by data/Arc. |
| REQ-025 | P081 metrics/counters/histograms are implemented and usable for rollout. | Metrics `:765-780`, `:960-976` | Partially Implemented | Some metrics are declared/recordable (`boundary_policy_decisions_total`, `boundary_policy_shadow_disagreement_total`, `operator_alert_native_delivery_total`, etc.). Several explicitly promised metrics are absent from code search: `auth_ambiguous_caller_warn_total`, `boundary_no_op_label_total`, `audit_log_rate_limited_total`, `approval_idempotency_duplicate_total`, `boundary_commit_transaction_latency_ms`, `audit_budget_cleanup_duration_ms`, `operator_alert_clear_latency_ms`. |

## Reviewer Scorecard

| Lens | Score | Top risk | Confidence |
|---|---|---|---|
| Proposal conformance | Partial | Metrics/readback/reliability/macOS evidence still incomplete | High |
| Rust architecture | Mostly aligned | Need keep request-path policy immutable and no direct fixture I/O as surfaces grow | High |
| Rust reliability | Partial | Reliability proof remains thinner than proposal acceptance text for several failure modes | Medium |
| API contract | Mostly aligned | `audit_log_health` / `boundaryRuntime` readback schema is narrower than proposal | High |
| Observability/rollout | Partial | Metrics contract and canary validation are incomplete | High |
| macOS UI/accessibility | Partial | Native delivery/accessibility proof is model-level, not hidden-window/FKA/contrast/motion runtime proof | Medium |
| Readiness | Not Ready | Major findings remain despite passing `proposal-081` gate | High |

## Routed Specialist Findings

### OPS-001: P081 rollout metrics are only partially implemented

- Reviewer: `observability_rollout_reviewer`
- Severity: Major
- Confidence: High
- Related REQs: REQ-025
- Evidence types: proposal, code, tests-found, tests-run
- Evidence references: proposal metrics at `docs/proposals/081...md:765-780` and `:960-976`; implementation subset at `control-plane/crates/db/src/metrics.rs:95-107`; exact-name search found absent metrics listed in REQ-025; `proposal-081` gate passed.
- Why it matters: Phase 3/4 rollout and enforce cutover depend on counters/histograms for disagreement, ambiguous callers, audit pressure, alert clearing, and commit latency. A partial metric set makes readiness claims hard to operate.
- Recommended action: Add the missing metric declarations and record points or explicitly revise P081 to defer them with a named follow-up.
- Acceptance criteria: Exact proposal metric names are declared, recorded on the relevant paths, exposed in bounded diagnostics when applicable, and covered by focused tests.

### API-001: `boundaryRuntime.auditLogHealth` readback is narrower than the proposal contract

- Reviewer: `api_contract_reviewer`
- Severity: Major
- Confidence: High
- Related REQs: REQ-020
- Evidence types: proposal, code, tests-run
- Evidence references: proposal `:1013`; implementation `control-plane/crates/graphql-server/src/schema.rs:394-408`; P081 GraphQL readback test asserts schema/version/integrity/no raw rows but not writability/retention/cleanup/shadow refs.
- Why it matters: Operators need bounded readback to decide whether audit storage is writable, cleanup is healthy, and shadow coverage is sufficient without raw table access.
- Recommended action: Extend GraphQL/MCP boundary runtime payloads with audit writability, retention/cleanup state, budget/used fields, and shadow coverage report references, or narrow the proposal.
- Acceptance criteria: GraphQL and MCP tests assert every promised readback field and still prove no raw audit rows are exposed.

### REL-001: Reliability proof inventory does not exercise every accepted failure mode

- Reviewer: `rust_reliability_reviewer`
- Severity: Major
- Confidence: Medium
- Related REQs: REQ-018
- Evidence types: proposal, tests-found, tests-run
- Evidence references: proposal `:1011`; gate token inventory at `scripts/test-gate.sh:6309-6338`; executed SIGTERM and committed-unack tests in `proposal-081` output.
- Why it matters: The proposal names contention, audit outage, subscription gap, safe-mode exit, SIGTERM drain, committed-unack recovery, and denial-audit backpressure as covered reliability behavior. Token presence is weaker than scenario proof for several of those.
- Recommended action: Replace token inventory with focused tests for each named failure mode, or split unimplemented cases into a follow-up proposal.
- Acceptance criteria: `proposal-081` executes or directly delegates to named tests for each reliability item, with failing assertions on the promised behavior.

### OPS-002: Canary validation is present but not schema-equivalent

- Reviewer: `observability_rollout_reviewer`
- Severity: Major
- Confidence: Medium
- Related REQs: REQ-017
- Evidence types: proposal, config, tests-run
- Evidence references: `docs/evidence/boundary-policy-shadow-coverage/boundary-policy-canaries.yaml`; `scripts/test-gate.sh:6241-6307`; `docs/evidence/boundary-policy-shadow-coverage/report.json:15-27`.
- Why it matters: The canary file becomes part of enforce readiness. A substring validator can miss malformed YAML, duplicate fields, unknown fields, or row/test binding drift.
- Recommended action: Add a structured YAML validator for `boundary_policy_canaries_v1`, verify duplicate/unknown/missing fields, and assert the shadow coverage report consumes parsed canary rows.
- Acceptance criteria: Malformed canary fixtures fail the gate with typed errors; report generation consumes parsed rows, not text matching.

### UI-001: macOS alert/accessibility proof remains model-level

- Reviewer: `macos_ui_reviewer`
- Severity: Major
- Confidence: Medium
- Related REQs: REQ-019, REQ-022
- Evidence types: proposal, code, tests-run
- Evidence references: proposal `:1012-1015`; `NotificationService.applyP081OperatorAlerts` at `Chainworks Forge/Engine/NotificationService.swift:117-141`; Swift tests at `Chainworks ForgeTests/Proposal031ThinGraphQLReadBoundaryTests.swift:6-244`; `proposal-081` Swift test bundle passed.
- Why it matters: The proposal specifically calls out hidden/inactive window delivery, Full Keyboard Access, Increase Contrast, and Reduce Motion. Unit tests prove data/model behavior, but not those platform interaction conditions.
- Recommended action: Add remote UI/accessibility evidence or focused app-level tests for hidden/inactive alerts, temporary status item behavior, disabled approval keyboard reachability, contrast, and motion adaptation.
- Acceptance criteria: P081 evidence includes hidden/inactive alert fires-and-clears proof plus accessibility parity checks for the named macOS modes.

### READY-001: Do not close out P081 as fully implemented yet

- Reviewer: `observability_rollout_reviewer`
- Severity: Critical
- Confidence: High
- Related REQs: REQ-017 through REQ-025
- Evidence types: tests-run, code, proposal
- Evidence references: `./scripts/test-gate.sh proposal-081` passed; remaining partial requirements above.
- Why it matters: The same-tree proposal gate is green, but the proposal text still promises broader metrics, readback, reliability, and macOS accessibility evidence than the gate currently proves.
- Recommended action: Keep P081 active until partial REQs are either implemented and tested or explicitly scoped out into a successor proposal.
- Acceptance criteria: All in-scope REQs are `Implemented`, or the proposal is revised/superseded with clear deferrals before closeout.

## Readiness Checklist

| Item | Status | Notes |
|---|---|---|
| Canonical proposal gate | Passed | `./scripts/test-gate.sh proposal-081` passed on SHA `45bc91a8b226ce34965ab1f1dc62eedc66dfcc1f`. |
| Full regression suite | Not run | Not required for a failing/not-ready verdict; no successful readiness verdict claimed. |
| Remote UI smoke | Not run | P081 gate explicitly does not require UI smoke; hidden/inactive alert proof remains a gap. |
| Core backend service flows | Partially validated | Boundary fixture, GraphQL/MCP readback, idempotency, safe-mode, SIGTERM, and Swift unit tests passed. |
| Empty/loading/error/offline/permission states | Partial | Denial/redaction states are covered; macOS hidden/inactive delivery and platform accessibility modes are not. |
| Accessibility | Partial | Unit accessibility metadata exists; FKA/Increase Contrast/Reduce Motion runtime coverage not found. |
| Privacy/security | Partial | Principal/audit hardening improved; full named security acceptance sweep is not proven. |
| Telemetry/rollout | Partial | Metrics and canary validator are incomplete. |

## Verification Log

| Command / check | Result |
|---|---|
| `git rev-parse HEAD` | `45bc91a8b226ce34965ab1f1dc62eedc66dfcc1f` |
| `git status --short` | Clean |
| `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/discover_prior_review.py ...` | No prior proposal-review artifacts found |
| `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/report_path.py ...` | Selected this R3 report path |
| `rg boundary_policy_decisions_total ...` | Found partial metrics implementation; several exact proposal metric names absent |
| `find . -iname '*boundary*canar*'` | Found `docs/evidence/boundary-policy-shadow-coverage/boundary-policy-canaries.yaml` |
| `./scripts/test-gate.sh proposal-081` | Passed |
| Swift result bundle | `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/chainworks-test-gates/proposal-081-swift-20260524-193324.xcresult` |

## Final Verdict

P081 is materially closer than R2 and the same-tree `proposal-081` gate passes. The previous blocker around server-side observer field redaction is addressed, MCP idempotency has stronger committed-unack coverage, and canary/metric surfaces now exist.

The implementation still does not fully satisfy the proposal text. Overall conformance is `Partial`, not `Implemented`, because the rollout metrics list, audit health/readback fields, reliability scenario proof, canary validation strength, and macOS hidden/accessibility evidence remain incomplete. Overall readiness is `Not Ready`.

Recommended next actions:

1. Complete or explicitly defer the missing P081 metrics and histogram names.
2. Expand `boundaryRuntime`/MCP readback to include audit writability, retention/cleanup state, budget/used fields, and shadow coverage refs.
3. Replace reliability token inventory with direct tests for each named failure mode.
4. Add structured canary YAML validation and parsed report consumption.
5. Add macOS hidden/inactive alert and accessibility-mode evidence, or scope it into a follow-up before closeout.
