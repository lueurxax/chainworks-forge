# Proposal 053: Bounded ACP Artifact Discovery and Startup Latency Multi-Lens Audit R4

| Field | Value |
|---|---|
| Proposal | `docs/proposals/053-bounded-acp-artifact-discovery-and-startup-latency.md` |
| Repository Root | `.` |
| Git SHA | `1770a306c045a15a78c7e596c9a77acd6292a6ec` |
| Working Tree | dirty |
| Audited At | `2026-04-23T21:25:47+03:00` |
| Platform Scope | Ambiguous |
| Proposal State | Active |
| Overall Conformance | Partial |
| Overall Readiness | Not Ready |
| Audit Confidence | High |

## Executive Verdict

`R3` is stale. The current worktree closes the previous gate-freshness blocker and the previous metrics/schema gaps: the branch now has fresh same-tree `proposal-053` gate evidence, the full Phase 0 schema is enforced in the gate, the Phase 1 evidence pack exists, and the structured metrics/readback surface is implemented across ACP, executor diagnostics, DB, GraphQL, and MCP tests. P053 still does not reach proposal-closeout truth because the Phase 1 manual latency spot-check evidence does not match the proposal's explicit requirement: the proposal asks for a manual spot-check on the reference `8.9 GB / 126,643-file` workspace, but the recorded artifact cites an ACP fixture test with `acp_pre_initialize_local_latency_ms=0` instead.

## Lens Scorecard

| Lens | Assessment | Top Risk | Confidence |
|---|---|---|---|
| Conformance | Partial | Phase 1 manual latency evidence does not satisfy the proposal's stated proof path | High |
| Architecture | Acceptable | Core control-plane contract now aligns with the revised proposal | High |
| Product | Acceptable | Branch is correctly positioned as `gate_only_internal`, not production-exposed | High |
| UI | Acceptable | P053 UI remains explicitly deferred to P069 | High |
| UX | Acceptable | Operator evidence pack exists and the future UI dependency is clearly separated | Medium |
| Readiness | Not Ready | One explicit Phase 1 exit-evidence requirement is still not satisfied as written | High |

## Proposal Contract

### Scope
- Remove repository/workspace/worktree-wide discovery from the pre-`initialize` ACP path.
- Replace implicit artifact inference with typed expected outputs, bounded pre-prompt metadata, engine-owned discovery decisions, and P057/P058 settlement truth.
- Persist durable diagnostics and readback for reports, GraphQL, MCP, restart recovery, support workflows, and future P069 readers.
- Keep P053 macOS UI rendering out of scope after the explicit P069 deferral.

### Locked Decisions
- Fresh ACP startup must send `initialize` before repository/workspace/worktree/generated-state traversal.
- `ExpectedOutputSpec`, `PrePromptExpectedOutputMetadata`, `OutputDiscoveryDecision`, `DiscoveryFilesystem`, `GitManifestRunner`, and `settle_agent_outputs_from_discovery_decisions` are the core frozen control-plane contract.
- Discovery settlement is engine-owned; ACP transport is protocol/timing/envelope capture only.
- `gate_only_internal` is allowed for P053 same-tree control-plane validation.
- Missing macOS UI is not a P053 blocker after the P069 split.

### Primary User Flows
- A fresh ACP execution reaches `initialize` without hidden broad local traversal.
- A fresh or reused ACP session captures bounded pre-prompt metadata and settles outputs through typed discovery decisions.
- Operators and support tooling inspect durable discovery truth through server-owned readback.
- Compatibility paths use legacy broad discovery only through explicit, auditable policy.

### UI Commitments
- P053 does not implement the macOS operator UI; P069 owns that work after P031.
- P053 must still expose server-owned read models for future GraphQL-only UI consumption.

### UX Commitments
- Evidence must distinguish Forge overhead from provider latency.
- Missing/stale/rejected outputs must remain inspectable through durable readback.
- Before P069, control-plane readback must still be understandable and server-owned.

### Acceptance Criteria
- Fresh ACP startup sends `initialize` before broad traversal.
- Pre-prompt metadata is bounded and refreshed for both fresh and reused sessions.
- Required outputs settle through accepted discovery decisions instead of raw target-path rereads.
- Bounded meta-root discovery stays supplemental-only.
- Legacy broad discovery is disabled by default and auditable when used.
- `proposal-053|p053` is the deterministic proof path.
- P069, not P053, owns future macOS rendering.

### Test / Evidence Requirements
- `docs/proposals/053.review/cap-validation.json` exists and matches the Phase 0 schema.
- `docs/proposals/053.review/security-checklist.md` (or risk acceptance) exists.
- `docs/proposals/053.review/manual-latency-spot-check.md` records the observed `acp_pre_initialize_local_latency_ms` from the proposal-required manual spot-check.
- `docs/proposals/053.review/operator-clarity-evidence.md` exists.
- `docs/proposals/053.review/phase-1-retrospective.md` exists.
- `./scripts/test-gate.sh proposal-053` passes on the audited tree.

### Explicit Exclusions
- No P053 macOS UI implementation.
- No ACP transport switch to HTTP.
- No default broad legacy discovery.
- No second output-validation system outside P057/P058.

## Proposal Fidelity / Divergence

### Matches
- Fresh same-tree `proposal-053` gate evidence now exists on this worktree.
- The gate now enforces the expanded Phase 0 schema and the Phase 1 evidence-pack presence.
- The structured metrics/readback surface that was incomplete in `R2`/`R3` is now implemented across source and tests.
- The P069 UI deferral remains explicitly reflected in proposal/reference truth.

### Divergences
- The manual latency spot-check artifact still does not prove the proposal-required reference-workspace measurement.

### Ambiguities / Evidence Gaps
- This audit did not rerun the gate itself; it relies on the in-tree recorded rerun evidence.
- The branch remains `gate_only_internal`, so no production-exposure claim is made here.

## Requirement Summary

| Status | Count |
|---|---:|
| Implemented | 15 |
| Partially Implemented | 1 |
| Missing | 0 |
| Not Verifiable | 0 |

## Requirement Audit

### REQ-001 Fresh ACP startup sends `initialize` before broad traversal
- Proposal Source: Goals (`lines 43-44`), ACP Execution Sequence (`lines 102-105`), Behavioral Acceptance Criteria (`lines 817-819`)
- Status: Implemented
- Evidence Type: `code`, `tests-found`
- Evidence:
  - `control-plane/crates/acp/src/transport.rs:1064-1130`
  - `docs/proposals/053.review/proposal-053-gate-2026-04-23.md:5-9`
  - `docs/proposals/053.review/proposal-053-gate-2026-04-23.md:13-22`
- Gap / Note: Fresh startup timing remains aligned with the no-pre-`initialize` traversal contract.

### REQ-002 Typed expected-output specs remain the engine-owned discovery contract
- Proposal Source: Expected Output Specs (`lines 152-178`), Implementation Contracts (`lines 833-839`)
- Status: Implemented
- Evidence Type: `code`
- Evidence:
  - `control-plane/crates/domain/src/discovery.rs:215-230`
  - `control-plane/crates/engine/src/contracts.rs:371-457`
- Gap / Note: The current tree still builds typed specs in the engine and uses them as the discovery contract.

### REQ-003 Fresh and reused sessions capture bounded pre-prompt metadata per prompt turn
- Proposal Source: ACP Execution Sequence (`lines 100-106`), Pre-Prompt Metadata Bounds (`lines 180-231`)
- Status: Implemented
- Evidence Type: `code`
- Evidence:
  - `control-plane/crates/acp/src/transport.rs:1298-1367`
  - `control-plane/crates/acp/src/lib.rs:142-157`
  - `control-plane/crates/acp/src/session.rs:45-94`
- Gap / Note: The transport now returns the richer pre-prompt metrics and timeout/digest details through `ExecutionResult`.

### REQ-004 Provider-envelope and `CHAINWORKS_OUTPUT` cap parity is enforced before acceptance
- Proposal Source: Executive Summary (`lines 18-20`), Phase 1 exit criteria (`lines 666-668`)
- Status: Implemented
- Evidence Type: `code`, `tests-found`
- Evidence:
  - `control-plane/crates/engine/src/executor.rs:3311-3355`
  - `docs/proposals/053.review/security-checklist.md:21-24`
  - `docs/proposals/053.review/proposal-053-gate-2026-04-23.md:20-21`
- Gap / Note: Cap parity remains covered by both source and the recorded gate slices.

### REQ-005 `DiscoveryFilesystem` operation recording exists under `domain::discovery`
- Proposal Source: Phase 0 freeze (`lines 610-613`), R9 resolution (`line 855`)
- Status: Implemented
- Evidence Type: `code`, `tests-found`
- Evidence:
  - `control-plane/crates/domain/src/discovery.rs:80-115`
  - `docs/proposals/053.review/cap-validation.json:99-107`
  - `docs/proposals/053.review/proposal-053-gate-2026-04-23.md:15-17`
- Gap / Note: This remains closed relative to the old `R1`/`R2` findings.

### REQ-006 Supplemental discovery is bounded to the current run meta-root and remains supplemental-only
- Proposal Source: Goals (`line 48`), ACP Execution Sequence (`line 110`), Phase 1 exit criteria (`lines 670-671`)
- Status: Implemented
- Evidence Type: `code`
- Evidence:
  - `control-plane/crates/engine/src/executor.rs:3352-3355`
  - `control-plane/crates/engine/src/executor.rs:3513-3554`
  - `docs/proposals/053.review/security-checklist.md:38-39`
- Gap / Note: The current executor still models bounded meta-root discovery as a separate downstream phase.

### REQ-007 `changed_files_manifest` generation remains part of the declared control-plane path
- Proposal Source: Goals (`line 49`), Phase 2 exit criteria (`line 692`), Implementation Contracts (`line 840`)
- Status: Implemented
- Evidence Type: `code`, `tests-found`
- Evidence:
  - `control-plane/crates/engine/src/executor.rs:3263-3308`
  - `docs/proposals/053.review/proposal-053-gate-2026-04-23.md:21-22`
- Gap / Note: The current tree now also records structured manifest-status diagnostics.

### REQ-008 Durable discovery diagnostics persist and project through server-owned readback
- Proposal Source: Goals (`line 51`), Phase 2 exit criteria (`lines 693-699`), Deferred Phase 3 (`lines 710-714`)
- Status: Implemented
- Evidence Type: `code`, `tests-found`
- Evidence:
  - `control-plane/crates/db/src/repos/agent_execution_discovery_diagnostics.rs:9-18`
  - `control-plane/crates/db/src/repos/agent_execution_discovery_diagnostics.rs:196-239`
  - `control-plane/crates/db/tests/proposal_053_discovery_diagnostics.rs:320-371`
  - `control-plane/crates/graphql-server/tests/proposal_058_runtime_facts.rs:264-299`
  - `control-plane/crates/mcp-server/tests/proposal_058_runtime_facts.rs:247-282`
- Gap / Note: Current tests now assert the richer diagnostics payload through DB/GraphQL/MCP readback.

### REQ-009 The full structured metrics surface declared by P053 exists in source
- Proposal Source: Metrics and Observability (`lines 726-777`), Phase 1 exit criteria (`line 672`)
- Status: Implemented
- Evidence Type: `code`
- Evidence:
  - `control-plane/crates/acp/src/lib.rs:142-157`
  - `control-plane/crates/acp/src/session.rs:45-94`
  - `control-plane/crates/acp/src/transport.rs:1064-1130`
  - `control-plane/crates/acp/src/transport.rs:1346-1367`
  - `control-plane/crates/engine/src/executor.rs:1601-1627`
  - `control-plane/crates/engine/src/executor.rs:3263-3355`
  - `control-plane/crates/engine/src/executor.rs:3513-3554`
- Gap / Note: Repo-wide search on this worktree now finds all proposal-required structured fields.

### REQ-010 Phase 0 cap-validation artifact matches the declared schema surface
- Proposal Source: Dependencies and Readiness (`lines 79-88`), Evidence schema (`lines 615-655`)
- Status: Implemented
- Evidence Type: `code`, `tests-found`
- Evidence:
  - `docs/proposals/053.review/cap-validation.json:1-114`
  - `scripts/test-gate.sh:2414-2464`
  - `docs/reference/test-gates.md:917-955`
- Gap / Note: The current artifact and gate now enforce the schema fields that were missing in earlier audits.

### REQ-011 Phase 1 security artifact exists
- Proposal Source: Security review exit criterion (`lines 679-683`)
- Status: Implemented
- Evidence Type: `tests-found`
- Evidence:
  - `docs/proposals/053.review/security-checklist.md:1-44`
  - `scripts/test-gate.sh:2399-2413`
- Gap / Note: This remained closed from `R2` onward.

### REQ-012 Phase 1 manual latency spot-check artifact proves the proposal-required reference-workspace check
- Proposal Source: Phase 1 exit criteria (`lines 673-675`)
- Status: Partially Implemented
- Evidence Type: `code`, `tests-found`
- Evidence:
  - `docs/proposals/053.review/manual-latency-spot-check.md:26-33`
  - `docs/proposals/053.review/manual-latency-spot-check.md:39-48`
  - `docs/proposals/053.review/proposal-053-gate-2026-04-23.md:28`
  - `docs/proposals/053.review/cap-validation.json:14-23`
- Gap / Note: The artifact now records an observed `acp_pre_initialize_local_latency_ms` value (`0`), but the recorded command is `cargo test -p acp test_claude_adapter_executes_subprocess_and_returns_artifacts --test integration -- --nocapture`, i.e. an ACP fixture test, not the manual spot-check on the reference `8.9 GB / 126,643-file` workspace explicitly named by the proposal. The cap-validation artifact also describes this as a `manual reference-workspace spot-check`, which the spot-check doc itself does not substantiate.

### REQ-013 Phase 1 operator-clarity evidence exists
- Proposal Source: Phase 1 exit criteria (`line 675`)
- Status: Implemented
- Evidence Type: `code`
- Evidence:
  - `docs/proposals/053.review/operator-clarity-evidence.md:11-24`
  - `docs/proposals/053.review/operator-clarity-evidence.md:43-51`
  - `control-plane/crates/engine/src/executor.rs:3263-3355`
- Gap / Note: The artifact is qualitative, but it satisfies the proposal's requirement to record in-tree clarity evidence.

### REQ-014 Phase 1 retrospective decision artifact exists
- Proposal Source: Phase 1 exit criteria (`line 676`)
- Status: Implemented
- Evidence Type: `code`
- Evidence:
  - `docs/proposals/053.review/phase-1-retrospective.md:19-29`
  - `scripts/test-gate.sh:2480-2484`
- Gap / Note: This closes the missing-retrospective gap from earlier audits.

### REQ-015 P053 closeout does not depend on macOS UI implementation
- Proposal Source: Status (`line 6`), UI Deferral to P069 (`lines 90-94`), Deferred Phase 3 (`lines 708-714`)
- Status: Implemented
- Evidence Type: `code`
- Evidence:
  - `docs/proposals/069-p053-discovery-diagnostics-operator-ui.md:1-9`
  - `docs/reference/artifact-discovery-and-settlement-optimization.md:32-38`
- Gap / Note: The proposal/reference truth remains aligned that P069 owns the macOS UI.

### REQ-016 Canonical `proposal-053|p053` proof exists for the current audited tree
- Proposal Source: Behavioral Acceptance Criteria (`line 818`), Phase 1 exit criteria (`line 677`), Implementation Contracts (`line 845`)
- Status: Implemented
- Evidence Type: `tests-found`
- Evidence:
  - `docs/proposals/053.review/proposal-053-gate-2026-04-23.md:5-9`
  - `docs/proposals/053.review/proposal-053-gate-2026-04-23.md:24-28`
  - `scripts/test-gate.sh:2391-2524`
- Gap / Note: The branch now has fresh same-tree recorded gate evidence; the stale-gate blocker from `R3` is closed.

## Architecture Review

**Summary:** Acceptable

No new architecture finding. The current worktree closes the substantive control-plane design gaps from `R2`/`R3`: metrics, diagnostics projection, cap-validation schema, and evidence-pack structure now line up with the revised P053 contract.

## Product Review

**Summary:** Acceptable

### PROD-001 Branch value remains internal closeout value, not production exposure value
- Severity: Note
- Confidence: High
- Related Proposal Items / Requirements: `REQ-010`, `REQ-011`, `REQ-013`
- Evidence Type: `code`
- Evidence:
  - `docs/proposals/053.review/cap-validation.json:7-27`
  - `docs/proposals/053.review/security-checklist.md:12-26`
- Why It Matters: This branch is still correctly positioned as `gate_only_internal`. That is acceptable for P053 control-plane closeout, but it is not production rollout proof.
- Recommended Action: Keep production-exposure claims blocked on refreshed production sampling/signoff exactly as the current artifacts already state.

## UI Review

**Summary:** Acceptable

No current UI finding. P053 UI rendering is explicitly deferred to P069 and should not be reintroduced as a P053 blocker.

## UX Review

**Summary:** Acceptable

No current UX blocker beyond the explicit spot-check mismatch already captured as a requirement gap. The operator evidence pack now exists and is clearly separated from the future P069 UI work.

## Delivery / Readiness Review

**Summary:** Not Ready

### READY-001 Manual latency spot-check evidence does not satisfy the proposal's named proof path
- Severity: Major
- Confidence: High
- Related Proposal Items / Requirements: `REQ-012`
- Evidence Type: `code`, `tests-found`
- Evidence:
  - `docs/proposals/053.review/manual-latency-spot-check.md:26-33`
  - `docs/proposals/053.review/manual-latency-spot-check.md:39-48`
  - `docs/proposals/053.review/cap-validation.json:14-23`
  - `docs/proposals/053.review/proposal-053-gate-2026-04-23.md:28`
- Why It Matters: The proposal explicitly asks for a manual latency spot-check on the reference large workspace and for the observed `acp_pre_initialize_local_latency_ms` to be recorded. The current artifact records a fixture-test value instead. That is an evidence-contract mismatch, not a backlog nice-to-have.
- Recommended Action: Replace or supplement the current note with the actual reference-workspace measurement and update `cap-validation.json` wording so it matches the real evidence source.

### READY-002 Fresh same-tree `proposal-053` gate evidence now exists and replaces the stale gate blocker from `R3`
- Severity: Note
- Confidence: High
- Related Proposal Items / Requirements: `REQ-016`
- Evidence Type: `tests-found`
- Evidence:
  - `docs/proposals/053.review/proposal-053-gate-2026-04-23.md:5-9`
  - `docs/proposals/053.review/proposal-053-gate-2026-04-23.md:24-28`
- Why It Matters: Future audits should not repeat the stale claim that this worktree lacks same-tree gate evidence.
- Recommended Action: Keep any next audit focused on the remaining spot-check proof mismatch rather than reraising the gate-freshness issue.

## Readiness Checklist

| Check | Status | Evidence / Note |
|---|---|---|
| Build succeeds on targeted platform(s) | Not Checked | No build command was run in this audit. |
| Core user flow runtime-validated | Partial | Recorded same-tree `proposal-053` pass exists, but the manual reference-workspace spot-check proof is still not satisfied as written. |
| Empty/loading/error states covered | Not Checked | P053 UI rendering is deferred to P069. |
| Accessibility risk acceptable | Not Checked | P053 UI rendering is deferred to P069. |
| Localization risk acceptable | Not Checked | P053 UI rendering is deferred to P069. |
| Critical tests executed | Pass | `docs/proposals/053.review/proposal-053-gate-2026-04-23.md` records a passing same-tree `proposal-053` gate rerun. |
| Full regression suite / canonical full gate passed on same tree/HEAD | Pass | Recorded in `docs/proposals/053.review/proposal-053-gate-2026-04-23.md`. |
| Privacy/permissions/entitlements reviewed | Partial | Security checklist exists for `gate_only_internal`; it is not a production signoff. |

## Verification Log

- `git status --short --branch && git rev-parse HEAD`
- `python3 /Users/user/.agents/skills/proposal-implementation-audit/scripts/report_path.py /Users/user/Documents/'Chainworks Forge'/.chainworks/worktrees/codex-p053-manual-merge-1833dd16/docs/proposals/053-bounded-acp-artifact-discovery-and-startup-latency.md`
- `date -Iseconds`
- `nl -ba docs/proposals/053.review/proposal-053-gate-2026-04-23.md | sed -n '1,80p'`
- `nl -ba docs/proposals/053.review/manual-latency-spot-check.md | sed -n '1,120p'`
- `nl -ba docs/proposals/053.review/operator-clarity-evidence.md | sed -n '1,120p'`
- `nl -ba docs/proposals/053.review/phase-1-retrospective.md | sed -n '1,120p'`
- `git diff --unified=10 -- docs/proposals/053.review/proposal-053-gate-2026-04-23.md docs/proposals/053.review/manual-latency-spot-check.md docs/proposals/053.review/operator-clarity-evidence.md docs/proposals/053.review/phase-1-retrospective.md`
- `git diff --name-only -- control-plane/crates/acp/src/lib.rs control-plane/crates/acp/src/session.rs control-plane/crates/acp/src/transport.rs control-plane/crates/acp/tests/integration.rs control-plane/crates/db/src/repos/agent_execution_discovery_diagnostics.rs control-plane/crates/db/tests/proposal_053_discovery_diagnostics.rs control-plane/crates/domain/src/discovery.rs control-plane/crates/engine/src/executor.rs control-plane/crates/engine/tests/integration.rs control-plane/crates/engine/tests/proposal_041_parity.rs control-plane/crates/graphql-server/tests/proposal_058_runtime_facts.rs control-plane/crates/mcp-server/tests/proposal_058_runtime_facts.rs docs/proposals/053.review/cap-validation.json docs/proposals/053.review/proposal-053-gate-2026-04-23.md docs/reference/test-gates.md scripts/test-gate.sh`
- repo-wide metrics coverage script against the proposal's `Required structured fields` list
- cap-validation schema presence script against the proposal's `Evidence schema` list
- targeted line reads for ACP transport/session/lib, executor diagnostics wiring, DB diagnostics readback, gate script, gate reference docs, cap-validation artifact, and phase-1 evidence docs

## Recommended Next Actions

1. Replace or supplement `docs/proposals/053.review/manual-latency-spot-check.md` with the actual manual reference-workspace measurement required by the proposal.
2. Update `docs/proposals/053.review/cap-validation.json` so its `source_query_or_extraction_method` no longer overstates the current spot-check basis if the evidence remains fixture-based.
3. After that, rerun the audit once more; the other substantive `R2`/`R3` gaps are already closed on this tree.
