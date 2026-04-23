| Field | Value |
|---|---|
| Proposal | `docs/proposals/053-bounded-acp-artifact-discovery-and-startup-latency.md` |
| Repository Root | `.` |
| Git SHA | `1770a306c045a15a78c7e596c9a77acd6292a6ec` |
| Working Tree | clean |
| Audited At | `2026-04-23T18:35:08+03:00` |
| Platform Scope | Ambiguous |
| Proposal State | Active |
| Overall Conformance | Partial |
| Overall Readiness | Not Ready |
| Audit Confidence | High |

## Executive Verdict

R1 is stale. On the current `1770a306c045a15a78c7e596c9a77acd6292a6ec` tree, the core P053 control-plane implementation is materially aligned with the revised proposal: the operation-recorder boundary exists, the same-tree `proposal-053` gate evidence exists, the security artifact exists, and macOS UI is explicitly deferred to P069. P053 is still not ready for closeout because the current proposal keeps explicit Phase 0/Phase 1 evidence and observability contracts that are only partially satisfied on this tree: `cap-validation.json` does not match the declared schema, the Phase 1 evidence pack stops at gate/security and does not include the required manual spot-check / operator-clarity / retrospective proof, and only a subset of the required structured metrics fields is emitted.

## Lens Scorecard

| Lens | Assessment | Top Risk | Confidence |
|---|---|---|---|
| Conformance | Partial | Proposal-level evidence contract is only partially implemented | High |
| Architecture | Acceptable | Observability contract is incomplete relative to the proposal | High |
| Product | Acceptable | Branch is intentionally `gate_only_internal`, not production-exposed | High |
| UI | Acceptable | P053 UI is explicitly deferred to P069 | High |
| UX | Acceptable | Operator rendering is out of scope for P053 closeout after the P069 split | High |
| Readiness | Not Ready | Phase 0/1 exit evidence is incomplete on the current tree | High |

## Proposal Contract

### Scope
- Remove broad pre-`initialize` discovery from ACP startup.
- Move artifact truth to typed expected-output specs, bounded pre-prompt metadata, engine-owned discovery decisions, and P057/P058 settlement.
- Persist durable diagnostics and readback for reports, GraphQL, MCP, restart recovery, and future P069 UI readers.
- Keep direct macOS UI implementation out of P053 after the explicit P069 deferral.

### Locked Decisions
- Fresh sessions must send `initialize` before repository/workspace/worktree/generated-state traversal.
- `ExpectedOutputSpec`, `PrePromptExpectedOutputMetadata`, `OutputDiscoveryDecision`, `DiscoveryFilesystem`, `GitManifestRunner`, and `settle_agent_outputs_from_discovery_decisions` are the frozen core interfaces.
- Discovery settlement is engine-owned; ACP transport is protocol/timing/envelope capture only.
- Missing P053 macOS UI is not a P053 blocker after the P069 split.
- `gate_only_internal` is an allowed Phase 1 exposure mode for same-tree validation, but it does not waive the proposal's explicit Phase 0/1 evidence contracts.

### Primary User Flows
- A fresh ACP execution starts promptly and sends `initialize` before local broad discovery work.
- A fresh or reused ACP session captures bounded pre-prompt metadata and settles required outputs through typed discovery decisions.
- Support/operator readback can inspect durable discovery diagnostics, settlement truth, and reconciliation state through server-owned reads.
- Compatibility paths can use legacy broad discovery only via explicit, auditable policy/override paths.

### UI Commitments
- P053 itself does not implement the macOS UI; this is explicitly deferred to P069.
- P053 must still expose durable read models that P069 can consume through GraphQL only.

### UX Commitments
- Operators must be able to distinguish Forge overhead from provider latency.
- Operators must be able to see which output is missing/rejected and why.
- Before P069 lands, diagnostics must remain compact and server-owned rather than forcing raw local inference.

### Acceptance Criteria
- Fresh startup sends `initialize` before broad traversal.
- Bounded pre-prompt metadata and bounded meta-root discovery apply to fresh and reused sessions.
- Required outputs settle through typed discovery decisions rather than raw target-path rereads.
- Legacy broad discovery is disabled by default and auditable when used.
- The deterministic `proposal-053|p053` gate proves the no-pre-`initialize` traversal and bounded-discovery invariants.
- P069, not P053, owns the future macOS rendering.

### Test / Evidence Requirements
- `docs/proposals/053.review/cap-validation.json` with the declared Phase 0 schema.
- Durable Phase 1 security checklist or risk-acceptance artifact.
- Passing same-tree `./scripts/test-gate.sh proposal-053` evidence.
- Phase 1 manual latency spot-check evidence.
- Phase 1 qualitative operator-clarity evidence.
- Phase 1 retrospective decision evidence.

### Explicit Exclusions
- No P053 macOS operator UI implementation.
- No HTTP ACP transport switch.
- No second output-validation system outside P057/P058.
- No default broad legacy discovery.

## Proposal Fidelity / Divergence

### Matches
- Core ACP startup path now logs pre-`initialize`, `initialize`, and `session/new` timing around the real handshake boundary.
- `DiscoveryFilesystem` now exposes an explicit operation-recorder boundary in `domain::discovery` with gate-covered recorder tests.
- Same-tree `proposal-053` gate evidence exists on this branch and the canonical gate now checks the Phase 0/Phase 1 evidence artifacts.
- Phase 1 security artifact now exists.
- Current proposal explicitly defers macOS UI to P069, and reference docs were updated to match that decision.

### Divergences
- `cap-validation.json` exists, but it does not satisfy the full Phase 0 evidence schema described by the proposal and explicitly records incomplete readiness timing/sampling.
- The Phase 1 evidence pack does not include the proposal-required manual latency spot-check, operator-clarity proof, or retrospective record.
- Only part of the required structured metrics field set is emitted in source.

### Ambiguities / Evidence Gaps
- The committed gate evidence is same-tree branch evidence, but this audit did not rerun `proposal-053` itself.
- The branch intentionally remains `gate_only_internal`; no claim is made here about production-exposed P053 readiness.

## Requirement Summary

| Status | Count |
|---|---:|
| Implemented | 12 |
| Partially Implemented | 3 |
| Missing | 0 |
| Not Verifiable | 0 |

## Requirement Audit

### REQ-001 Fresh ACP startup sends `initialize` before traversal
- Proposal Source: Goals (`lines 43-44`), ACP Execution Sequence (`lines 102-105`), Behavioral Acceptance Criteria (`lines 817-819`)
- Status: Implemented
- Evidence Type: `code`, `tests-found`
- Evidence:
  - `control-plane/crates/acp/src/transport.rs:1061-1125`
  - `docs/proposals/053.review/proposal-053-gate-2026-04-23.md:13-22`
  - `docs/reference/test-gates.md:917-949`
- Gap / Note: Current source and gate registration match the revised no-pre-`initialize` contract.

### REQ-002 Typed expected-output contract and engine-owned settlement boundary exist
- Proposal Source: Expected Output Specs (`lines 152-178`), Output Discovery Decisions (`lines 233-259`), Implementation Contracts (`lines 833-838`)
- Status: Implemented
- Evidence Type: `code`
- Evidence:
  - `control-plane/crates/domain/src/discovery.rs:214-230`
  - `control-plane/crates/engine/src/contracts.rs:371-457`
  - `control-plane/crates/engine/src/executor.rs:3142-3180`
- Gap / Note: Typed specs and settlement are present in the current tree and remain engine-owned.

### REQ-003 Fresh and reused ACP sessions capture bounded pre-prompt metadata per prompt turn
- Proposal Source: ACP Execution Sequence (`lines 100-106`), Pre-Prompt Metadata Bounds (`lines 180-231`), R9 resolution (`lines 853-856`)
- Status: Implemented
- Evidence Type: `code`, `tests-found`
- Evidence:
  - `control-plane/crates/acp/src/transport.rs:1269-1327`
  - `control-plane/crates/domain/src/discovery.rs:255-303`
  - `docs/proposals/053.review/proposal-053-gate-2026-04-23.md:16-21`
- Gap / Note: The current transport constructs per-turn `PrePromptExpectedOutputContext` and measures bounded metadata capture.

### REQ-004 Generated-state denylist and discovery operation-recorder boundary are implemented in `domain::discovery`
- Proposal Source: Generated-State Exclusion (`lines 124-150`), Phase 0 freeze (`lines 610-613`), Implementation Contracts (`line 839`), R9 resolution (`line 855`)
- Status: Implemented
- Evidence Type: `code`, `tests-found`
- Evidence:
  - `control-plane/crates/domain/src/discovery.rs:24-115`
  - `control-plane/crates/domain/src/discovery.rs:540-600`
  - `control-plane/crates/domain/src/discovery.rs:1438-1522`
  - `docs/proposals/053.review/proposal-053-gate-2026-04-23.md:16-17`
- Gap / Note: This closes the stale R1 blocker around the promised recorder boundary.

### REQ-005 Provider-envelope and `CHAINWORKS_OUTPUT` cap parity is enforced before acceptance
- Proposal Source: Executive Summary (`lines 18-20`), Phase 1 exit criteria (`lines 666-668`), R9 resolution (`line 854`)
- Status: Implemented
- Evidence Type: `code`, `tests-found`
- Evidence:
  - `docs/proposals/053.review/proposal-053-gate-2026-04-23.md:20-21`
  - `docs/proposals/053.review/security-checklist.md:21-24`
  - `docs/proposals/053.review/security-checklist.md:35-39`
- Gap / Note: The gate and security artifact now explicitly cover envelope-cap behavior.

### REQ-006 Supplemental discovery is bounded to the current run meta-root and remains supplemental-only
- Proposal Source: Goals (`line 48`), ACP Execution Sequence (`line 110`), Phase 1 exit criteria (`lines 670-671`)
- Status: Implemented
- Evidence Type: `code`, `tests-found`
- Evidence:
  - `control-plane/crates/engine/src/executor.rs:1407-1423`
  - `docs/proposals/053.review/security-checklist.md:38-39`
  - `docs/proposals/053.review/proposal-053-gate-2026-04-23.md:17-18`
- Gap / Note: Current source measures bounded meta-root discovery and treats it as a separate engine phase.

### REQ-007 `changed_files_manifest` is generated through `GitManifestRunner`
- Proposal Source: Goals (`line 49`), Phase 2 exit criteria (`line 692`), Implementation Contracts (`line 840`)
- Status: Implemented
- Evidence Type: `code`, `tests-found`
- Evidence:
  - `control-plane/crates/engine/src/git_manifest.rs:82-120`
  - `control-plane/crates/engine/src/executor.rs:3116-3140`
  - `docs/proposals/053.review/proposal-053-gate-2026-04-23.md:21-22`
- Gap / Note: Manifest generation is present and explicitly covered by the branch gate evidence.

### REQ-008 Durable discovery diagnostics persist and project through server-owned readback
- Proposal Source: Goals (`line 51`), Phase 2 exit criteria (`lines 693-699`), Deferred Phase 3 boundary (`lines 710-714`)
- Status: Implemented
- Evidence Type: `code`, `tests-found`
- Evidence:
  - `control-plane/crates/db/src/repos/agent_execution_discovery_diagnostics.rs:8-18`
  - `control-plane/crates/db/src/repos/agent_execution_discovery_diagnostics.rs:192-235`
  - `control-plane/crates/graphql-server/src/types/stage.rs:255-286`
  - `control-plane/crates/mcp-server/src/tools/reports.rs:165-214`
  - `docs/proposals/053.review/proposal-053-gate-2026-04-23.md:18-22`
- Gap / Note: Current tree provides durable diagnostics plus GraphQL/MCP projection and reconciliation marking.

### REQ-009 Legacy broad discovery is disabled by default and remains auditable when used
- Proposal Source: Goals (`line 52`), ACP Execution Sequence (`line 111`), Phase 2 exit criteria (`lines 695-699`)
- Status: Implemented
- Evidence Type: `code`, `tests-found`
- Evidence:
  - `docs/proposals/053.review/security-checklist.md:37-39`
  - `docs/proposals/053.review/proposal-053-gate-2026-04-23.md:17-18`
  - `docs/proposals/053.review/proposal-053-gate-2026-04-23.md:21-22`
- Gap / Note: Legacy override persistence/readback exists and the canonical gate now covers it.

### REQ-010 P053 closeout is not blocked by missing macOS UI, but it must expose durable readback for future P069 readers
- Proposal Source: Status (`line 6`), UI Deferral to P069 (`lines 90-94`), Deferred Phase 3 (`lines 708-714`)
- Status: Implemented
- Evidence Type: `code`, `tests-found`
- Evidence:
  - `docs/proposals/069-p053-discovery-diagnostics-operator-ui.md:1-9`
  - `docs/reference/artifact-discovery-and-settlement-optimization.md:32-38`
  - `docs/reference/artifact-discovery-and-settlement-optimization.md:143-146`
- Gap / Note: The current proposal revision explicitly removes the old P053 UI blocker, and the current tree exposes the durable readback needed for P069.

### REQ-011 Phase 0 cap-validation artifact matches the declared schema and readiness record
- Proposal Source: Dependencies and Readiness (`lines 79-88`), Phase 0 exit criteria (`lines 596-613`), Evidence schema (`lines 615-655`)
- Status: Partially Implemented
- Evidence Type: `code`, `tests-found`
- Evidence:
  - `docs/proposals/053.review/cap-validation.json:1-92`
  - `docs/proposals/053.review/proposal-053-gate-2026-04-23.md:13-14`
  - `scripts/test-gate.sh:2391-2444`
- Gap / Note: The artifact exists and the gate validates a reduced subset, but the current file still diverges from the proposal schema and readiness claims. `sampled_execution_ids` is empty (`line 13`), `dependency_readiness_recorded_within_two_working_days` is `false` (`line 46`), and schema keys such as `chosen_max_provider_envelope_bytes`, `chosen_max_aggregate_declared_output_bytes`, `chosen_provider_envelope_buffer_policy`, `workflow_output_size_policy_required`, `fresh_and_reused_session_metadata_semantics_frozen`, `discovery_filesystem_owner`, and `generated_at` are absent.

### REQ-012 Phase 1 security artifact exists for the current gate-only/internal tree
- Proposal Source: Security review exit criterion (`lines 679-683`)
- Status: Implemented
- Evidence Type: `tests-found`
- Evidence:
  - `docs/proposals/053.review/security-checklist.md:1-44`
  - `docs/proposals/053.review/proposal-053-gate-2026-04-23.md:13-14`
  - `scripts/test-gate.sh:2395-2444`
- Gap / Note: This closes the stale R1 blocker about the missing security artifact. The current checklist is explicitly limited to `gate_only_internal` and not production signoff.

### REQ-013 Canonical `proposal-053|p053` gate is registered and passing same-tree evidence exists on this branch
- Proposal Source: Behavioral Acceptance Criteria (`line 818`), Phase 1 exit criteria (`line 677`), Implementation Contracts (`line 845`)
- Status: Implemented
- Evidence Type: `tests-found`
- Evidence:
  - `docs/proposals/053.review/proposal-053-gate-2026-04-23.md:5-9`
  - `docs/proposals/053.review/proposal-053-gate-2026-04-23.md:24-26`
  - `docs/reference/test-gates.md:911-949`
- Gap / Note: The current branch now has explicit same-tree gate evidence; the old R1 gate blocker is no longer current truth.

### REQ-014 Phase 1 exit evidence includes manual spot-check, operator-clarity proof, and retrospective decision record
- Proposal Source: Phase 1 exit criteria (`lines 672-677`)
- Status: Partially Implemented
- Evidence Type: `tests-found`
- Evidence:
  - `docs/proposals/053.review/proposal-053-gate-2026-04-23.md:1-26`
  - `docs/proposals/053.review/security-checklist.md:1-44`
  - `docs/proposals/053.review/cap-validation.json:7-19`
- Gap / Note: The branch contains gate evidence and a security checklist, but the evidence pack stops there. `docs/proposals/053.review/` currently contains only `cap-validation.json`, `proposal-053-gate-2026-04-23.md`, and `security-checklist.md`; this audit found no same-tree artifact for the required manual latency spot-check, qualitative operator-clarity proof, or Phase 1 retrospective record.

### REQ-015 Required structured metrics and observability fields are emitted
- Proposal Source: Metrics and Observability (`lines 726-777`), Phase 1 exit criteria (`line 672`)
- Status: Partially Implemented
- Evidence Type: `code`
- Evidence:
  - `control-plane/crates/acp/src/transport.rs:1061-1125`
  - `control-plane/crates/acp/src/transport.rs:1317-1326`
  - `control-plane/crates/engine/src/executor.rs:1414-1422`
  - `control-plane/crates/engine/src/executor.rs:3133-3179`
- Gap / Note: The current tree emits the core startup/metadata/manifest/acceptance counts, but repo-wide search still found only 14 of the 36 required fields. Missing examples include `acp_prompt_duration_ms`, `acp_pre_prompt_metadata_timeout`, `acp_pre_prompt_metadata_digest_bytes`, `acp_git_manifest_status`, `acp_discovery_schema_version`, `acp_exact_output_aggregate_bytes`, `acp_cap_validation_p90_output_bytes`, and `acp_legacy_broad_discovery_truncation_reason`.

## Architecture Review

**Summary:** Acceptable

### ARCH-001 Observability contract is only partially implemented
- Severity: Major
- Confidence: High
- Related Proposal Items / Requirements: `REQ-015`
- Evidence Type: `code`
- Evidence:
  - `control-plane/crates/acp/src/transport.rs:1061-1125`
  - `control-plane/crates/acp/src/transport.rs:1317-1326`
  - `control-plane/crates/engine/src/executor.rs:1414-1422`
  - `control-plane/crates/engine/src/executor.rs:3133-3179`
- Why It Matters: The proposal explicitly turns timing attribution and diagnostic counters into durable observability contract, not optional logging polish. A partial metric set weakens the proposal's stated ability to distinguish Forge overhead, provider behavior, cap reasons, and fallback/reconciliation conditions in a stable way.
- Recommended Action: Either finish emitting the remaining required fields or narrow the proposal/reference truth to the smaller metric set that is actually implemented.

## Product Review

**Summary:** Acceptable

### PROD-001 Current branch is valid only for gate-only/internal value, not production-exposed value
- Severity: Minor
- Confidence: High
- Related Proposal Items / Requirements: `REQ-011`, `REQ-014`
- Evidence Type: `tests-found`
- Evidence:
  - `docs/proposals/053.review/cap-validation.json:6-19`
  - `docs/proposals/053.review/security-checklist.md:12-26`
  - `docs/reference/test-gates.md:944-949`
- Why It Matters: This branch proves the control-plane slice, but the proposal still frames some evidence in terms of operator understanding and representative workload validation. Without that, the branch is a strong internal merge candidate, not a clean proposal closeout.
- Recommended Action: Keep the branch positioned as `gate_only_internal` until the missing Phase 1 evidence pack is written or the proposal is explicitly narrowed.

## UI Review

**Summary:** Acceptable

No current P053 UI finding. The revised proposal explicitly defers macOS operator rendering to P069, and the current tree updates proposal/reference truth accordingly.

## UX Review

**Summary:** Acceptable

No direct P053 UX finding beyond the missing Phase 1 operator-clarity evidence already captured under readiness. UX rendering/clarity implementation is intentionally deferred to P069.

## Delivery / Readiness Review

**Summary:** Not Ready

### READY-001 Phase 0 artifact and gate check are out of sync with the proposal's own evidence schema
- Severity: Major
- Confidence: High
- Related Proposal Items / Requirements: `REQ-011`
- Evidence Type: `code`, `tests-found`
- Evidence:
  - `docs/proposals/053-bounded-acp-artifact-discovery-and-startup-latency.md:620-655`
  - `docs/proposals/053.review/cap-validation.json:13-20`
  - `docs/proposals/053.review/cap-validation.json:46-68`
  - `scripts/test-gate.sh:2405-2444`
- Why It Matters: The branch now has a cap-validation artifact, but the gate only enforces a reduced subset of the schema while the proposal still declares a larger Phase 0 contract. That leaves proposal closeout truth weaker than the proposal currently claims.
- Recommended Action: Align all three surfaces: proposal schema, `cap-validation.json`, and `proposal-053` gate validation.

### READY-002 Phase 1 evidence pack is incomplete even after the gate/security blockers were closed
- Severity: Major
- Confidence: High
- Related Proposal Items / Requirements: `REQ-014`
- Evidence Type: `tests-found`
- Evidence:
  - `docs/proposals/053.review/proposal-053-gate-2026-04-23.md:1-26`
  - `docs/proposals/053.review/security-checklist.md:1-44`
  - `docs/proposals/053.review/cap-validation.json:7-19`
- Why It Matters: The revised tree fixes the stale R1 blockers, but the proposal still requires three more Phase 1 evidence items before closeout: manual latency spot-check, qualitative operator-clarity evidence, and retrospective decision record. Without them, the branch is not at proposal-closeout quality.
- Recommended Action: Add the missing evidence artifacts under `docs/proposals/053.review/`, or narrow the proposal so these are no longer part of the committed exit criteria.

### READY-003 Same-tree `proposal-053` gate evidence is present and should replace the stale R1 gate blocker
- Severity: Note
- Confidence: High
- Related Proposal Items / Requirements: `REQ-013`
- Evidence Type: `tests-found`
- Evidence:
  - `docs/proposals/053.review/proposal-053-gate-2026-04-23.md:5-9`
  - `docs/proposals/053.review/proposal-053-gate-2026-04-23.md:24-26`
  - `docs/reference/test-gates.md:911-949`
- Why It Matters: Any future audit/addendum must treat the old "gate not run" blocker as stale. The current tree already records same-tree passing control-plane gate evidence.
- Recommended Action: Keep future findings focused on the remaining explicit proposal divergences, not the already-closed gate blocker.

## Readiness Checklist

| Check | Status | Evidence / Note |
|---|---|---|
| Build succeeds on targeted platform(s) | Not Checked | No build/test command was run in this audit. |
| Core user flow runtime-validated | Partial | Same-tree gate evidence exists, but this audit did not rerun runtime validation locally. |
| Empty/loading/error states covered | Not Checked | P053 UI rendering is deferred to P069. |
| Accessibility risk acceptable | Not Checked | P053 UI rendering is deferred to P069. |
| Localization risk acceptable | Not Checked | P053 UI rendering is deferred to P069. |
| Critical tests executed | Not Checked | Audit did not execute tests; it inspected committed same-tree gate evidence only. |
| Full regression suite / canonical full gate passed on same tree/HEAD | Pass | `docs/proposals/053.review/proposal-053-gate-2026-04-23.md` records same-tree `proposal-053` pass for this branch/worktree, but it was not rerun by this audit. |
| Privacy/permissions/entitlements reviewed | Partial | `docs/proposals/053.review/security-checklist.md` exists for gate-only/internal validation, but it is explicitly not a production signoff. |

## Verification Log

- `git status --short --branch && git rev-parse HEAD`
- `python3 /Users/user/.agents/skills/proposal-implementation-audit/scripts/report_path.py docs/proposals/053-bounded-acp-artifact-discovery-and-startup-latency.md`
- `git diff --name-only 190bc8f186bb788cc2efe884d9cff0f271adde15..1770a306c045a15a78c7e596c9a77acd6292a6ec --`
- `git diff --unified=20 190bc8f186bb788cc2efe884d9cff0f271adde15..1770a306c045a15a78c7e596c9a77acd6292a6ec -- docs/proposals/053-bounded-acp-artifact-discovery-and-startup-latency.md`
- `git diff --unified=20 190bc8f186bb788cc2efe884d9cff0f271adde15..1770a306c045a15a78c7e596c9a77acd6292a6ec -- control-plane/crates/domain/src/discovery.rs control-plane/crates/acp/src/transport.rs control-plane/crates/engine/src/executor.rs scripts/test-gate.sh docs/reference/test-gates.md docs/reference/artifact-discovery-and-settlement-optimization.md`
- `rg -n "RecordingDiscoveryOperationRecorder|trait DiscoveryOperationRecorder|phase_1_exposure_mode|proposal-053|P069" control-plane scripts docs/reference docs/proposals/053-bounded-acp-artifact-discovery-and-startup-latency.md`
- `rg -n "acp_pre_initialize_local_latency_ms|...|acp_reconciliation_pending" control-plane 'Chainworks Forge' docs/reference scripts -g '!**/target/**'`
- `rg -n "spot-check|operator-clarity|retrospective" docs/proposals/053.review docs/reference control-plane 'Chainworks Forge' -g '!**/target/**'`
- `nl -ba docs/proposals/053.review/cap-validation.json`
- `nl -ba docs/proposals/053.review/proposal-053-gate-2026-04-23.md`
- `nl -ba docs/proposals/053.review/security-checklist.md`
- targeted file reads for `domain::discovery`, `engine::contracts`, `engine::executor`, `engine::git_manifest`, GraphQL/MCP readback, and diagnostics repo code

## Recommended Next Actions

1. Align `docs/proposals/053.review/cap-validation.json`, the proposal's declared Phase 0 schema, and the `proposal-053` gate field checks.
2. Add the missing Phase 1 evidence artifacts: manual latency spot-check, qualitative operator-clarity proof, and retrospective decision record.
3. Either finish the remaining required metrics fields or narrow the proposal/reference truth to the subset actually implemented.
