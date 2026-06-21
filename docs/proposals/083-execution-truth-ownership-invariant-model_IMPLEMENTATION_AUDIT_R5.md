# P083 Implementation Audit R5: Execution-Truth Ownership and Invariant Model

## 0. Metadata

| Field | Value |
|---|---|
| Proposal | `docs/proposals/083-execution-truth-ownership-invariant-model.md` |
| Proposal state | Active proposal, `status=Revise-required`; `implementation_may_start=false` |
| Proposal revision audited | `P083-r70-refined-r69-score-lift` |
| Implementation target | Current dirty worktree |
| Compare base | Not supplied; audit inspected same-tree working files at current checkout |
| Repository root | `/Users/user/Documents/Chainworks Forge` |
| Git SHA | `0e6482c8` |
| Working tree status | Dirty, 172 changed paths at audit start; includes unrelated P079/P080/P082/P086/P094 and Swift/Rust/doc state |
| Audit timestamp | 2026-06-20 17:20:39 EEST |
| Report path | `docs/proposals/083-execution-truth-ownership-invariant-model_IMPLEMENTATION_AUDIT_R5.md` |
| Platform/product scope | macOS SwiftUI operator shell, Rust control-plane service, GraphQL API, MCP API, SQLite persistence, rollout/evidence |

## 1. Verdict

- Overall Conformance: `Not Implemented`
- Overall Implementation Readiness: `Not Ready`
- Reviewer Selection Reuse: `Not reused`
- Audit Confidence: `High` for the blocking findings; `Medium` for exhaustive UI/runtime coverage because no remote UI or Swift full gate was executed.
- Same-tree full regression / canonical gate: `Failed` as a wrapper run. The canonical wrapper reached the first Cargo command and was killed with exit 137 in its dedicated gate target. The wrapper's constituent Rust/static/lint checks were rerun directly and passed, but that is not a canonical gate pass.
- Highest-risk blockers:
  1. The GraphQL SDL contract still diverges from the proposal: lifecycle mutations return `Result<ConcretePayload>` with GraphQL errors, not the required success-or-`DenialPayload` unions; caller IDs are `String`, not a `CallerRequestId` scalar; denial vocabulary is not byte-equal to the proposal.
  2. The proposal's own current-review refresh gate is unsatisfied. No fresh R70 aggregate approval with `blocker_count=0` and corpus-only-current-revision attestation was found.
  3. P083 rollout/release evidence is still incomplete for release use: the operator readback fixture self-identifies as a placeholder, several referenced negative fixtures self-identify as placeholders, and 14 active P083 rollout/evidence JSON files lack `proposal_revision_id`.

## 2. Prior Proposal-Review Reuse

- Prior artifacts found: `discover_prior_review.py` returned no proposal-review artifacts for P083.
- Prior selected reviewers: none reusable.
- Prior rejected close alternatives: none reusable.
- Prior stacks / surfaces / risks: inferred from current implementation surface, not reused from prior proposal review.
- Prior required changes before implementation: the proposal itself carries the R70 current-review refresh gate.
- Reuse decision: `Not reused`.
- Delta from prior selection: not applicable.
- Reasoning: Existing implementation audit files `R1` through `R4` were treated as historical context only, not as reviewer-selection evidence. R4's stale findings were rechecked against the current tree; several were resolved, including MCP schema files and native Run menu structure.

## 3. Current Reviewer Routing

| Reviewer | Discipline / Stack | Why Selected | Evidence IDs | Reused From Proposal Review? | Notes |
|---|---|---|---|---|---|
| API contract | GraphQL, MCP, schema parity | `implementation_surface_fingerprint.py` required `api-contract`; P083 is API-heavy | API-01, API-02, REQ-004, REQ-005, REQ-006 | No | Highest-risk finding is here. |
| Rust reliability | command idempotency, shutdown, late output, durable clock | Required `reliability`; P083 owns recovery invariants | REL-01, REQ-007 through REQ-010 | No | Direct Rust slices passed. |
| Rust architecture/persistence | SQLite authority, migrations, durable truth model | Required `architecture`; proposal is persistence-first | ARCH-01, REQ-008 | No | Additive migration shape is strong. |
| Observability/rollout | rollout contract, metrics, evidence corpus | Required `observability-rollout` | ROLL-01, ROLL-02, REQ-001 through REQ-003, REQ-011 | No | Release evidence remains blocked. |
| Rust security | auth boundary, public ingress, parser/DoS surfaces | `security_sensitive_diff.py` triggered security-sensitive categories | SEC-01, API-01 | No | No exploit validated, but readiness gates fail. |
| Apple UI/UX | macOS commands, manual identity check UI | Required `apple-ui-ux`; proposal has native UI commitments | UI-01, REQ-012, REQ-013, REQ-014 | No | Static source improved; runtime UI proof missing. |
| Performance/resource | bounded metrics, DoS/resource caps | Required `performance`; covered through bounded-label and resource checks | PERF-01, REQ-011 | No | No benchmark claims made. |

### Rejected Close Alternatives

| Reviewer | Why Rejected | Evidence IDs |
|---|---|---|
| iOS UI | Product is macOS-only for these surfaces | UI-01 |
| Go service | No Go implementation surface in P083 | ARCH-01 |
| Data science/product analytics | P083 metrics are operational counters, not analytical KPI design | PERF-01 |

## 4. Proposal Contract Summary

- In scope: execution-truth ownership model, caller-owned idempotency for lifecycle commands, rollback target parity, GraphQL/MCP contracts, eight additive SQLite migrations, durable monotonic clock, shutdown/cancellation/late-output recovery, bounded metrics, rollout readback, and native macOS read-only operator surfaces.
- Out of scope: new auth/RBAC, workflow YAML semantic changes, destructive history rewrite, filesystem/SwiftUI/GraphQL/MCP becoming authoritative for execution truth.
- Platform/product scope: Rust control-plane is authoritative for execution truth; SwiftUI is a read-only operator shell except externally governed command copy/trigger affordances.
- Locked decisions: caller IDs are selectors/idempotency keys, not authority; non-null rollback target must be caller supplied; rollout evidence and current review must be current-revision-only before Ready.
- Primary flows:
  1. Lifecycle commands: `runs.cancel`, `runs.retry`, `stages.retry`, `approvals.resolve`, `side_effects.force_reconcile`, `provider_session.shutdown`, `p083.rollback_execution`, `p083.set_enforcement_mode`.
  2. Rollback to `permissive` or `disabled` through GraphQL, MCP, command hash, audit row, and readback.
  3. Shutdown recovery through receipts, signal side effects, provider cancellation intents, and durable clock baselines.
  4. Post-cancel late-output quarantine and overflow latching.
  5. Manual process identity check and native Run menu/toolbar operation.
- Acceptance criteria: lines 958-983 of the proposal require the gate, evidence corpus, GraphQL/MCP parity, migrations, idempotency, runtime recovery, metrics, UI, current review, and hardening proof before implementation-complete or release-ready status.

## 5. Implementation Evidence Summary

- Changed files / modules inspected: `control-plane/crates/domain/src/commands.rs`, `control-plane/crates/engine/src/command_handler.rs`, `control-plane/crates/db/migrations/087...094_p083_*.sql`, `control-plane/crates/db/src/metrics.rs`, `control-plane/crates/db/src/repos/rollout_contract_checks.rs`, `control-plane/crates/graphql-server/src/schema.rs`, `control-plane/crates/graphql-server/src/types/p083.rs`, `control-plane/crates/mcp-server/src/tools/runs.rs`, `docs/reference/mcp/p083/*.schema.json`, `Chainworks Forge/Chainworks_ForgeApp.swift`, `Chainworks Forge/Views/RunsHomeView.swift`, `Chainworks Forge/Views/ManualProcessIdentityCheckBanner.swift`, `Chainworks Forge/Views/P083IdentityAmbiguousInboxView.swift`, `Chainworks Forge/Models/P083IdentityHoldSessionsModel.swift`, and P083 evidence fixtures.
- Adjacent files inspected: `scripts/test-gate.sh`, `scripts/cargo-cache-env.sh`, `.chainworks/reviews/proposal/*`, prior P083 audit R4 for historical context.
- Tests found: DB migration integration tests, engine P083/shutdown unit tests, domain denial-code test, GraphQL approval mutation tests, MCP P083 validation tests.
- Tests run: see Verification Log.
- Runtime checks: no live daemon or remote UI runtime check was run.
- Benchmarks: none run; no performance pass claimed.
- API/schema/migration checks: MCP schema files parse; GraphQL/MCP source inspected; migration files and readback descriptors present; direct static gate snippets passed.
- Rollout/telemetry checks: `scripts/lint-rollout-contract docs/evidence/083/rollout-contract-v1.json` passed, but placeholder fixtures remain.
- Evidence gaps: no canonical wrapper pass, no fresh R70 aggregate review, GraphQL SDL union/scalar mismatch, no remote UI proof, placeholder rollout fixtures, and active negative fixtures missing `proposal_revision_id`.

## 6. Proposal Fidelity / Divergence

### Matches

- All eight P083 migration files exist and are referenced by rollout readback descriptors.
- DB migration tests passed: 57/57.
- Command handler uses canonical intent hashing and transactional `command_idempotency::acquire_tx` paths for P083 lifecycle commands.
- R70 rollback target is present in GraphQL input, MCP input, command intent hash, and `p083_rollback_audit.target_enforcement_mode`.
- MCP P083 schema files now exist under `docs/reference/mcp/p083/` and inline MCP schemas reject unknown properties/invalid rollback target before command journal mutation.
- Native macOS Run menu now has `Lifecycle` and `Recovery` submenus, key equivalents, and toolbar menu parity source wiring.
- Manual identity check UI exists with copy diagnostic, retry identity check, mark process absent confirmation, overflow evidence menu, accessibility labels, copy confirmation, and read-only backend refresh model.
- P083 metric recorders use bounded label domains and drop out-of-domain labels.

### Divergences

- GraphQL does not implement the required SDL shape: no `CallerRequestId` scalar, no success/`DenialPayload` unions, and no byte-equal `DenialReason` enum.
- The implementation's denial-code vocabulary differs materially from the proposal. Proposal-only values include `request_id_not_owned`, `lifecycle_not_actionable`, `provider_session_not_cancellable`, `late_output_overflow_latched`, and `unknown_command`; implementation-only values include `idempotency_in_flight`, `idempotency_replayed`, `operator_required`, `run_not_found`, and `internal`.
- The proposal remains revise-required and explicitly disallows Ready until a fresh R70 aggregate review approves with corpus-only-current-revision attestation.
- Rollout release evidence still has self-labeled placeholders and missing revision IDs.
- Canonical `proposal-083` wrapper did not pass in this environment.

### Ambiguities / Evidence Gaps

- Direct Rust/static gate slices passed, but the canonical wrapper's dedicated gate target was killed. The code may be functionally green, but readiness cannot claim canonical gate success.
- Swift/macOS UI was source-inspected only. No Swift build, UI test, screenshot, VoiceOver runtime check, or remote UI smoke was run.
- Several P083 fixtures are current-revision JSON contract markers rather than executable proof. This is acceptable as inventory evidence but not enough to prove the GraphQL SDL union contract or runtime UI behavior.

## 7. Requirement Summary

| Status | Count |
|---|---:|
| Implemented | 8 |
| Partially Implemented | 2 |
| Missing | 3 |
| Not Verifiable | 2 |
| Out of Scope | 0 |

## 8. Requirement Audit

### REQ-001 Fresh current-review gate
- Proposal Source: `active_readiness_narrative`, `current_review_refresh_gate_v1`, acceptance line 980.
- Status: Missing.
- Implementation Mapping: expected review artifacts outside implementation code.
- Evidence Type: repository artifact search.
- Evidence: `discover_prior_review.py` returned no artifacts; `.chainworks/reviews/proposal` contains an unrelated JWT Authentication Module review with blocker counts, not P083 R70.
- Gap / Note: Ready cannot be claimed until fresh R70 aggregate review returns approve, blocker_count=0, and corpus-only-current-revision attestation.

### REQ-002 Canonical `proposal-083` proof gate
- Proposal Source: acceptance line 959.
- Status: Not Verifiable.
- Implementation Mapping: `scripts/test-gate.sh:9474-9702`.
- Evidence Type: command execution.
- Evidence: `./scripts/test-gate.sh p083` exited 137. Trace showed the first Cargo command killed at `cargo test -p db --test proposal_083_migrations` in `/Users/user/Library/Caches/Chainworks Forge/cargo-target/gates/proposal-083-gate`.
- Gap / Note: Direct constituent commands passed, but this is not a canonical wrapper pass.

### REQ-003 Current revision rollout/evidence fixtures
- Proposal Source: acceptance lines 961, 963-979; rollout fixture readiness rule around line 758.
- Status: Missing.
- Implementation Mapping: `docs/evidence/083/**`, `docs/evidence/rollout-contract/**`.
- Evidence Type: fixture scan and JSON validation.
- Evidence: 112 declared evidence paths exist; 131 JSON files parse. However `docs/evidence/rollout-contract/operator-readback/p083-full-surface.fixture.json:2` says it is a placeholder to replace before release; lines 10, 15, 25, and 27 carry placeholder fail/disabled messages. Referenced negatives such as `p083-rollback-disposition-missing-schema-version.json:15-27`, `p083-stale-security-review.json:15-27`, and others self-identify as placeholder negative fixtures. Fourteen active P083 rollout/evidence JSON files lack `proposal_revision_id`.
- Gap / Note: Shape is present, release evidence is not.

### REQ-004 GraphQL SDL lifecycle contract
- Proposal Source: `graphql_sdl_contract_v1`; acceptance line 965.
- Status: Missing.
- Implementation Mapping: `control-plane/crates/graphql-server/src/schema.rs`, `control-plane/crates/graphql-server/src/types/p083.rs`.
- Evidence Type: source inspection and vocabulary comparison.
- Evidence: `schema.rs:5819-5824`, `5885-5890`, `5932-5937`, and `5979-5983` expose `caller_request_id: String`. `types/p083.rs:738-829` defines concrete `SimpleObject` payloads; `schema.rs:5412-5448` maps command failures to GraphQL errors. `rg` found no `DenialPayload`, `DenialReason`, or lifecycle payload union types in `control-plane/crates/graphql-server/src`.
- Gap / Note: This is a contract mismatch even though GraphQL compiles and bounded error codes exist.

### REQ-005 MCP tool inventory and schema files
- Proposal Source: `mcp_tool_inventory_contract_v1`; acceptance line 966.
- Status: Implemented.
- Implementation Mapping: `docs/reference/mcp/p083/*.schema.json`, `control-plane/crates/mcp-server/src/tools/runs.rs`.
- Evidence Type: source inspection, schema inventory, tests.
- Evidence: 16 P083 MCP schema files exist. `runs.rs:374-448` exposes `p083.rollback_execution` and `p083.set_enforcement_mode` with `additionalProperties=false`, required `caller_request_id`, and rollback target enum. `cargo test -p mcp-server p083_mcp_` passed 3/3.
- Gap / Note: Shared denial vocabulary parity is blocked by REQ-004.

### REQ-006 R70 rollback target end-to-end
- Proposal Source: acceptance line 964.
- Status: Implemented.
- Implementation Mapping: GraphQL input, MCP input, command handler, migration.
- Evidence Type: source inspection and tests.
- Evidence: `schema.rs:5885-5907` accepts `target_enforcement_mode`; `runs.rs:382-390` requires `target_enforcement_mode`; `command_handler.rs:8109-8117` includes `target_enforcement_mode` in the canonical intent hash; `command_handler.rs:8354-8360` persists it into rollback audit; `091_p083_005_enforcement_and_rollback.sql:49` declares `target_enforcement_mode TEXT NOT NULL CHECK(...)`.
- Gap / Note: The old R69 target contradiction appears addressed.

### REQ-007 Command idempotency for lifecycle commands
- Proposal Source: acceptance line 963 and `command_idempotency_contract_v1`.
- Status: Implemented.
- Implementation Mapping: domain commands, command handler, command idempotency repo.
- Evidence Type: source inspection and tests.
- Evidence: `domain/src/commands.rs:52-90`, `344-360`, `432-445`, and `503-551` carry CallerRequestId-bearing command shapes. `command_handler.rs` contains `acquire_tx` calls for P083 paths including lines 3718, 4831, 5606, 7658, 8298, 8653, 8975, and 9309. DB migration tests passed idempotency lease, alias, and mismatch cases.
- Gap / Note: GraphQL scalar naming remains blocked under REQ-004.

### REQ-008 Eight additive SQLite migrations
- Proposal Source: acceptance line 962.
- Status: Implemented.
- Implementation Mapping: `control-plane/crates/db/migrations/087...094_p083_*.sql`.
- Evidence Type: source inspection and DB integration tests.
- Evidence: all eight physical migration files exist and are referenced by `rollout_contract_checks.rs`; `cargo test -p db --test proposal_083_migrations` passed 57/57.
- Gap / Note: none found in audited slices.

### REQ-009 Durable monotonic clock and shutdown baseline correlation
- Proposal Source: acceptance lines 968, 972, 975-978.
- Status: Implemented.
- Implementation Mapping: daemon startup, migrations 089/092/093/094, command handler.
- Evidence Type: static gate checks and Rust tests.
- Evidence: static gate snippets confirmed `monotonic_clock_ms()` use, `baseline_generation`, `wall_clock_iso8601`, `baseline_sample_id` columns, and provider-session shutdown baseline persistence. Engine shutdown filter passed 6/6; DB tests covered baseline samples and shutdown receipts/signals.
- Gap / Note: no live daemon restart drill was run in this audit.

### REQ-010 Cancellation, late output, and provider identity holds
- Proposal Source: acceptance lines 971, 976-978.
- Status: Implemented.
- Implementation Mapping: migrations 090 and 093, shutdown service, command handler.
- Evidence Type: source inspection and DB/engine tests.
- Evidence: migration 090 has generated normalized latch key columns and unique index; migration 093 has provider cancellation intents and `identity_ambiguous` process fate; DB tests passed overflow latch, provider cancellation, process fate, and null-epoch held-state cases.
- Gap / Note: runtime restart fixtures remain evidence markers rather than live restart proof in this audit.

### REQ-011 Bounded metrics and rollout labels
- Proposal Source: acceptance line 970.
- Status: Implemented.
- Implementation Mapping: `control-plane/crates/db/src/metrics.rs`.
- Evidence Type: source inspection and DB tests.
- Evidence: `metrics.rs:1334-1558` defines P083 bounded label domains and drops out-of-domain labels; DB tests passed required P083 metric names and valid label conformance.
- Gap / Note: rollout negative fixture for unbounded metric still self-identifies as placeholder under REQ-003.

### REQ-012 Native macOS Run commands
- Proposal Source: `native_command_validation_contract_v1`; acceptance line 974.
- Status: Implemented.
- Implementation Mapping: `Chainworks Forge/Chainworks_ForgeApp.swift`, `Chainworks Forge/Views/RunsHomeView.swift`.
- Evidence Type: source inspection and static gate snippet.
- Evidence: `Chainworks_ForgeApp.swift:501-538` defines top-level `Run` menu with `Lifecycle` and `Recovery` submenus, all eight commands, and key equivalents. `RunsHomeView.swift:217-246` mirrors the same command grouping in toolbar menus. Static gate snippet passed menu and focused-value source checks.
- Gap / Note: no remote UI or accessibility runtime proof was run.

### REQ-013 Manual process identity check UI
- Proposal Source: acceptance line 969 and `manual_process_identity_check_ui_v1`.
- Status: Partially Implemented.
- Implementation Mapping: `ManualProcessIdentityCheckBanner.swift`, `P083IdentityAmbiguousInboxView.swift`, `P083IdentityHoldSessionsModel.swift`.
- Evidence Type: source inspection.
- Evidence: banner renders visible denial copy, retry, copy diagnostic, mark-process-absent confirmation, overflow evidence action, VoiceOver labels, automatic-retry-paused text, and 1500 ms copy confirmation (`ManualProcessIdentityCheckBanner.swift:96-230`). Backend readback model is read-only and GraphQL-backed (`P083IdentityHoldSessionsModel.swift:6-13`, `43-66`).
- Gap / Note: retry error feedback is not modeled because `onRetryIdentityCheck` is a void action and read errors are collapsed to empty sessions. No Swift/UI tests prove loading/success/error or VoiceOver behavior.

### REQ-014 SwiftData lifecycle boundary fixtures
- Proposal Source: acceptance lines 967 and 973.
- Status: Not Verifiable.
- Implementation Mapping: `docs/evidence/083/swift/*.fixture.json`, Swift app root.
- Evidence Type: fixture/source inspection.
- Evidence: Swift fixture files exist and parse, but this audit did not find P083-specific Swift tests under `Chainworks ForgeTests`, and no Swift build/UI gate was executed.
- Gap / Note: fixture presence alone does not prove representative copied pre-P083 stores or actor boundary behavior.

### REQ-015 Implementation hardening requirements
- Proposal Source: acceptance lines 981-983 and `implementation_hardening_requirements_v1`.
- Status: Partially Implemented.
- Implementation Mapping: report kind, schema validation, retry/TTL policies, late-output latches, side-effect command hashing, lease TTL policy.
- Evidence Type: source inspection and Rust tests.
- Evidence: many hardening items have code evidence: report kind migration, rollback disposition validator, failed terminal and TTL policy tables, overflow unique latch, command intent hashing, and bounded metrics.
- Gap / Note: hardening cannot be marked fully proven while REQ-002, REQ-003, REQ-004, and REQ-014 remain open.

## 9. Prior Review Finding Follow-Through

| Prior Finding / Required Change | Status | Evidence | Notes |
|---|---|---|---|
| R69 blocker: rollback target used in intent hash but not accepted by GraphQL/MCP | Addressed | `schema.rs:5885-5907`, `runs.rs:382-390`, `command_handler.rs:8109-8117`, migration 091 | Target is now caller supplied and durable. |
| R70 current-review refresh gate | Not Addressed | Proposal lines 43-44, 598-606, 980; no P083 R70 review artifact found | Blocks Ready. |
| R4 missing evidence paths | Addressed in inventory | declared=112, missing=0 | Release proof still blocked by placeholders/revision gaps. |
| R4 missing MCP schema files | Addressed | 16 schema files under `docs/reference/mcp/p083` | Denial parity still blocked by GraphQL contract mismatch. |
| R4 native Run menu structure | Addressed | `Chainworks_ForgeApp.swift:501-538` | Runtime UI not proven. |
| R4 GraphQL shared denial union mismatch | Not Addressed | no `DenialPayload` or `DenialReason` types found; concrete payloads/errors used | Critical API blocker remains. |

## 10. Reviewer Scorecard

| Reviewer | Result | Confidence | Evidence Completeness | Critical | Major | Minor | Notes |
|---|---|---|---|---:|---:|---:|---|
| API contract | Fail | High | Medium | 1 | 0 | 0 | GraphQL SDL mismatch is direct. |
| Observability/rollout | Fail | High | Medium | 2 | 1 | 1 | Review/evidence gates block release. |
| Rust reliability | Pass with Issues | Medium | High for unit/DB, low for live restart | 0 | 1 | 0 | Direct slices pass; canonical wrapper killed. |
| Rust architecture/persistence | Pass with Issues | Medium | High | 0 | 0 | 0 | SQLite shape aligns. |
| Rust security | Fail for readiness | Medium | Medium | 0 | 1 | 0 | Security-sensitive diff triggered; review evidence missing. |
| Apple UI/UX | Pass with Issues | Medium | Low runtime proof | 0 | 1 | 0 | Static UI improved; no runtime proof or error-state test. |
| Performance/resource | Pass with Issues | Medium | Medium | 0 | 0 | 0 | Bounded labels implemented; no benchmark claim. |

## 11. Routed Specialist Findings

### 11.1 Critical

#### Finding ID: P083-R5-C01
- Reviewer: API contract
- Severity: Critical
- Confidence: High
- Related Proposal Items / REQs: REQ-004; proposal acceptance line 965.
- Evidence Type: source inspection and vocabulary comparison.
- Evidence: Proposal requires `CallerRequestId!`, lifecycle payload unions, shared `DenialPayload`, and byte-equal `DenialReason`. Implementation exposes `caller_request_id: String` (`schema.rs:5819-5824`, `5885-5890`, `5932-5937`), concrete `SimpleObject` payloads (`types/p083.rs:738-829`), and GraphQL errors (`schema.rs:5412-5448`). `rg` found no `DenialPayload`/`DenialReason`/union types. Vocabulary comparison found 10 proposal-only and 14 implementation-only denial names.
- Why It Matters: GraphQL clients cannot rely on the proposal's closed union contract or parity with MCP. This is a public API compatibility defect, not a missing test only.
- Recommended Action: Either implement the exact GraphQL SDL contract, including `CallerRequestId` scalar, `DenialReason`, shared `DenialPayload`, and success/failure unions, or revise P083 and regenerate review/evidence to make GraphQL error extensions the accepted contract.
- Acceptance Criteria: Generated/checked GraphQL SDL contains the proposal's required scalar, enums, payload unions, and byte-equal MCP denial vocabulary; fixtures become executable and fail if this drifts.

#### Finding ID: P083-R5-C02
- Reviewer: Observability/rollout
- Severity: Critical
- Confidence: High
- Related Proposal Items / REQs: REQ-001; proposal lines 43-44, 598-606, 980.
- Evidence Type: artifact search.
- Evidence: `discover_prior_review.py` returned no artifacts. `.chainworks/reviews/proposal` contains an unrelated JWT Authentication Module aggregate review with blockers, not P083 R70 approval.
- Why It Matters: The proposal declares implementation may start only after the human implementation approval gate and fresh aggregate R70 approval. Ready is explicitly forbidden until this exists.
- Recommended Action: Run a fresh aggregate proposal review against `P083-r70-refined-r69-score-lift` after code/evidence corrections, and store selected reviewer artifacts plus corpus-only-current-revision attestation.
- Acceptance Criteria: Review summary names the exact proposal revision, `decision=approve`, `blocker_count=0`, and includes required corpus attestation fields.

#### Finding ID: P083-R5-C03
- Reviewer: Observability/rollout
- Severity: Critical
- Confidence: High
- Related Proposal Items / REQs: REQ-003.
- Evidence Type: fixture inspection.
- Evidence: Operator readback fixture says it is a placeholder and "not release evidence" (`docs/evidence/rollout-contract/operator-readback/p083-full-surface.fixture.json:2`, `:27`). Referenced negative fixtures such as rollback-disposition-missing-schema-version, stale-security-review, stale-observability-rollout-review, final-readback-rank-stored, migration-sha256-missing, unbounded-metric-label, and force-quit-host-budget-claim self-identify as placeholders at lines 15-27. Fourteen active P083 rollout/evidence JSON files lack `proposal_revision_id`.
- Why It Matters: The linter can pass while release evidence is explicitly non-release placeholder evidence. This defeats the proposal's current-revision corpus integrity and rollout hold guarantees.
- Recommended Action: Replace placeholders with concrete same-tree run/readback/negative evidence, add `proposal_revision_id` where required, and make the linter fail self-labeled placeholders.
- Acceptance Criteria: No active P083 fixture contains placeholder markers; every P083 fixture carries `proposal_id=P083` and active revision where the proposal requires it; `scripts/lint-rollout-contract` fails if placeholder text reappears.

### 11.2 Major

#### Finding ID: P083-R5-M01
- Reviewer: Rust reliability
- Severity: Major
- Confidence: High
- Related Proposal Items / REQs: REQ-002.
- Evidence Type: command execution.
- Evidence: `./scripts/test-gate.sh p083` and `./scripts/test-gate.sh proposal-083` exited 137. `bash -x` showed the wrapper killed at the first Cargo command in the dedicated gate target. Running `CHAINWORKS_CARGO_SCCACHE=off ./scripts/test-gate.sh p083` also exited 137.
- Why It Matters: Conformance cannot claim canonical same-tree gate success, even though direct slices passed.
- Recommended Action: Fix the dedicated gate target/cache resource failure or adjust the gate cache policy, then rerun the canonical wrapper.
- Acceptance Criteria: `./scripts/test-gate.sh proposal-083` completes with "Proposal 083 gate passed" on the audited tree.

#### Finding ID: P083-R5-M02
- Reviewer: Apple UI/UX
- Severity: Major
- Confidence: Medium
- Related Proposal Items / REQs: REQ-013, REQ-014.
- Evidence Type: source inspection.
- Evidence: No P083-specific Swift tests were found under `Chainworks ForgeTests`. `P083IdentityHoldSessionsModel.swift:64-66` collapses read errors to an empty session list, and `ManualProcessIdentityCheckBanner.swift:215-229` always reports "Identity refresh requested" after the retry action because the action cannot report failure.
- Why It Matters: The proposal commits loading/success/error feedback and Swift boundary proof; source presence does not prove UI behavior, VoiceOver, or copied pre-P083 store migration behavior.
- Recommended Action: Add Swift unit/UI coverage for manual identity check loading/success/error, VoiceOver labels, and SwiftData boundary fixtures; run the appropriate repo gate or remote UI proof.
- Acceptance Criteria: A same-tree Swift or remote UI proof demonstrates the committed UI states and SwiftData boundary behavior.

### 11.3 Minor

#### Finding ID: P083-R5-N01
- Reviewer: Observability/rollout
- Severity: Minor
- Confidence: Medium
- Related Proposal Items / REQs: REQ-003.
- Evidence Type: fixture inventory.
- Evidence: `docs/evidence/083/rollout-contract-v1.json` references 11 negative fixtures, while `docs/evidence/rollout-contract/negative` contains 16 `p083-*.json` files.
- Why It Matters: Unwired negative fixtures can create false confidence or drift from the active rollout contract.
- Recommended Action: Either wire the extra negatives into the rollout contract or move them out of the active P083 evidence path.
- Acceptance Criteria: Active negative fixture inventory and rollout contract references are byte-aligned.

#### Finding ID: P083-R5-N02
- Reviewer: Observability/rollout
- Severity: Minor
- Confidence: High
- Related Proposal Items / REQs: REQ-002.
- Evidence Type: source inspection.
- Evidence: `scripts/test-gate.sh:2583` describes `proposal-083|p083` as "focused code-fix regression gate (main-sync request id)", which is stale relative to the actual P083 gate at lines 9474-9702.
- Why It Matters: Operator gate lists can misroute future verification.
- Recommended Action: Update the gate list description to match execution-truth ownership.
- Acceptance Criteria: `./scripts/test-gate.sh list` describes P083 accurately.

### 11.4 Notes

- Direct Rust and static checks are substantially stronger than R4: DB, engine, domain, GraphQL compile/tests, MCP tests, linter, JSON parse, migrations, and schema inventory all passed outside the wrapper.
- The main remaining implementation defect is API contract fidelity, not absence of code.

## 12. Readiness Checklist

| Check | Status | Evidence / Note |
|---|---|---|
| Build or canonical gate status | Failed | Canonical wrapper exit 137; direct slices passed. |
| Proposal contract satisfied | Failed | GraphQL SDL, review gate, release evidence unresolved. |
| Prior review blockers addressed | Partial | R69 rollback target fixed; current-review gate unresolved. |
| Tests cover committed behavior | Partial | Rust coverage good; Swift/UI and GraphQL SDL contract proof missing. |
| Critical tests executed | Partial | Rust slices executed; canonical wrapper and UI proof missing. |
| Core user/service flow runtime or integration validated where needed | Partial | Rust unit/integration slices passed; no live daemon/UI. |
| Empty/loading/error/offline/permission states covered where relevant | Partial | P083 UI error/offline not proven. |
| Accessibility and localization risk acceptable where relevant | Not Verifiable | Source labels exist; no runtime VoiceOver proof. |
| API/schema compatibility acceptable | Failed | GraphQL SDL contract mismatch. |
| Migration/rollback path acceptable | Pass with Risks | Migrations and rollback target pass direct checks; canonical wrapper missing. |
| Telemetry/observability sufficient | Failed | Placeholder rollout evidence remains. |
| Security/privacy risk acceptable | Not Ready | Security-sensitive diff triggered; current security review/rollout evidence missing. |
| Privacy/permissions/entitlements reviewed where relevant | Partial | Pasteboard uses current-host-only; no full UI/security release review. |
| Performance risk acceptable | Pass with Risks | Bounded metrics present; no benchmark. |
| Full regression suite or canonical full/proposal gate passed on audited tree/HEAD | Failed | No canonical pass. |
| Release/handoff evidence sufficient | Failed | Placeholders and missing review gate. |

## 13. Product / Metrics Overlay

Product review was not selected as a primary routed specialist for R5. Operational metric notes:

- Leading metric: P083 rollout/check pass rate remains blocked by placeholder evidence.
- Guardrail metric: bounded label enforcement is implemented in `metrics.rs`.
- Decision checkpoint: no Ready/closeout until GraphQL contract and current-review gate are resolved.
- Rollout recommendation: stay disabled/hold.
- Instrumentation gaps: replace placeholder rollout/readback evidence with concrete same-tree metrics/readback output.

## 14. Verification Log

- Commands run:
  - `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/report_path.py docs/proposals/083-execution-truth-ownership-invariant-model.md` -> R5 report path.
  - `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/discover_prior_review.py docs/proposals/083-execution-truth-ownership-invariant-model.md` -> no artifacts.
  - `python3 .../security_sensitive_diff.py --root ... --json` -> triggered `auth`, `dos_resource_limits`, `filesystem_subprocess_boundary`, `parser_boundary`, `public_ingress`, `secrets_redaction_privacy`, `unsafe_crypto_dependency`; 193 files.
  - `python3 .../implementation_surface_fingerprint.py --root ... --json` -> required lenses `api-contract`, `apple-ui-ux`, `architecture`, `observability-rollout`, `performance`, `reliability`, `security`; 193 files.
  - `./scripts/test-gate.sh proposal-083` -> exit 137, empty log.
  - `./scripts/test-gate.sh p083` -> exit 137.
  - `bash -x ./scripts/test-gate.sh p083` -> exit 137; killed at first dedicated-target Cargo command after evidence corpus verified.
  - `CHAINWORKS_CARGO_SCCACHE=off ./scripts/test-gate.sh p083` -> exit 137.
  - Declared evidence path scan -> 112 declared, 0 missing.
  - Placeholder scan -> operator readback and multiple referenced negative fixtures still self-label as placeholders.
  - `scripts/lint-rollout-contract docs/evidence/083/rollout-contract-v1.json` -> PASS.
  - Static checks mirroring P083 gate snippets -> migrations/readback descriptors/rollback validation/acquire_tx/monotonic clock/MCP rollback/GraphQL lifecycle terms/macOS menu source all passed.
  - `cargo test -p db --test proposal_083_migrations -- --test-threads=1 --nocapture` -> 57 passed.
  - `cargo check -p db` -> passed.
  - `cargo test -p engine p083 -- --test-threads=1 --nocapture` -> 21 passed.
  - `cargo test -p engine shutdown -- --test-threads=1 --nocapture` -> 6 passed.
  - `cargo check -p daemon` -> passed.
  - `cargo check -p graphql-server` -> passed.
  - `cargo check -p mcp-server` -> passed.
  - `cargo test -p domain p083_lifecycle_denial_code_all_round_trip_as_str -- --nocapture` -> 1 passed.
  - `cargo test -p graphql-server approval_mutations -- --nocapture` -> 5 passed.
  - `cargo test -p mcp-server p083_mcp_ -- --nocapture` -> 3 passed.
  - JSON parse over P083 evidence, rollout fixtures, negative fixtures, and MCP schema files -> 131 files, 0 parse errors.
  - Denial vocabulary comparison -> 10 proposal-only values, 14 implementation-only values.
  - `.chainworks/reviews/proposal` scan -> unrelated JWT review, no P083 R70 approval.
- Files inspected:
  - Proposal JSON, P083 evidence fixtures, rollout contract, negative fixtures, MCP schemas.
  - Rust domain, DB migrations/repos/metrics, engine command handler, daemon, GraphQL schema/types, MCP runs tool.
  - Swift app entry, RunsHome, ManualProcessIdentityCheckBanner, P083 identity inbox/model, tests directory.
  - `scripts/test-gate.sh`, `scripts/cargo-cache-env.sh`, prior R4 audit as historical context.
- Artifacts inspected:
  - `docs/evidence/083/**`
  - `docs/evidence/rollout-contract/operator-readback/p083-full-surface.fixture.json`
  - `docs/evidence/rollout-contract/negative/p083-*.json`
  - `.chainworks/reviews/proposal/*`
- Commands not run and why:
  - Swift full build/remote UI tests: not run because the canonical P083 wrapper and release readiness were already blocked; UI conclusions are source/evidence based.
  - Full regression gate: not run because P083 canonical gate did not pass and the worktree contains broad unrelated dirty state.
  - Live daemon restart drills: not run; Rust unit/integration slices were used instead.

## 15. Recommended Next Actions

- MUST-01: Resolve the GraphQL SDL contract mismatch. Implement the required `CallerRequestId` scalar, `DenialReason`, `DenialPayload`, and lifecycle payload unions, or revise P083 and rerun proposal review/evidence against the revised contract.
- MUST-02: Replace placeholder rollout/readback/negative fixtures with concrete same-tree evidence, add missing `proposal_revision_id` fields where required, and make rollout lint reject active placeholders.
- MUST-03: Fix the dedicated gate cache/resource kill and rerun `./scripts/test-gate.sh proposal-083` until the canonical wrapper passes.
- MUST-04: Run the fresh R70 aggregate proposal review and human implementation approval gate before any Ready or closeout claim.
- SHOULD-01: Add Swift/UI proof for manual identity check error/loading/success states, VoiceOver behavior, and SwiftData boundary fixtures against representative copied pre-P083 stores.
