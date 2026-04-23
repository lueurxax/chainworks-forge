# Proposal 053: Bounded ACP Artifact Discovery and Startup Latency Multi-Lens Audit R3

| Field | Value |
|---|---|
| Proposal | `docs/proposals/053-bounded-acp-artifact-discovery-and-startup-latency.md` |
| Repository Root | `.` |
| Git SHA | `1770a306c045a15a78c7e596c9a77acd6292a6ec` |
| Working Tree | dirty |
| Audited At | `2026-04-23T20:33:10+03:00` |
| Platform Scope | Ambiguous |
| Proposal State | Active |
| Overall Conformance | Partial |
| Overall Readiness | Not Ready |
| Audit Confidence | High |

## Executive Verdict

`R2` is already stale relative to the current dirty tree. On the current worktree, the branch closes the substantive `R2` gaps: the full Phase 0 cap-validation schema is now present, the Phase 1 evidence pack exists in-tree, and the previously missing structured metrics surface is now implemented across ACP transport, executor diagnostics, and readback tests. P053 is still not ready for closeout because the current audited tree contains uncommitted code changes after the last recorded `proposal-053` gate evidence, so the available gate proof is no longer same-tree for this exact worktree. One additional proposal-contract gap also remains in the new manual spot-check note: it records a pass decision, but not the observed `acp_pre_initialize_local_latency_ms` value that the proposal explicitly says must be recorded.

## Lens Scorecard

| Lens | Assessment | Top Risk | Confidence |
|---|---|---|---|
| Conformance | Partial | Current worktree lacks fresh same-tree `proposal-053` proof | High |
| Architecture | Acceptable | Core control-plane boundary now matches the revised proposal | High |
| Product | Acceptable | Branch correctly remains `gate_only_internal`, not production-exposed | High |
| UI | Acceptable | P053 UI remains explicitly deferred to P069 | High |
| UX | Acceptable | Operator evidence artifacts exist, but runtime-proof freshness is stale | Medium |
| Readiness | Not Ready | Recorded gate evidence predates current dirty-tree code changes | High |

## Proposal Contract

### Scope
- Remove repository/workspace/worktree-wide discovery from the pre-`initialize` ACP path.
- Replace implicit artifact inference with typed expected outputs, bounded pre-prompt metadata, engine-owned discovery decisions, and P057/P058 settlement truth.
- Persist durable diagnostics and readback for reports, GraphQL, MCP, restart recovery, and the future P069 UI.
- Treat P053 macOS UI rendering as out of scope after the explicit P069 split.

### Locked Decisions
- Fresh ACP startup must send `initialize` before repository/workspace/worktree/generated-state traversal.
- `ExpectedOutputSpec`, `PrePromptExpectedOutputMetadata`, `OutputDiscoveryDecision`, `DiscoveryFilesystem`, `GitManifestRunner`, and `settle_agent_outputs_from_discovery_decisions` are the frozen control-plane contract.
- The engine, not ACP transport, owns discovery settlement.
- `gate_only_internal` is allowed for P053 same-tree control-plane closeout, but only with matching in-tree evidence.
- Missing macOS operator UI is not a P053 blocker after the P069 deferral.

### Primary User Flows
- A fresh ACP execution starts promptly and reaches `initialize` without hidden broad local traversal.
- A fresh or reused ACP session captures bounded pre-prompt metadata and settles outputs through typed discovery decisions.
- Operators and support tooling read durable discovery truth through DB/GraphQL/MCP/report surfaces.
- Compatibility paths use legacy broad discovery only through explicit, bounded, auditable policy.

### UI Commitments
- P053 itself does not ship the macOS UI surfaces; P069 owns that work after P031.
- P053 must expose server-owned read models that P069 can consume through GraphQL only.

### UX Commitments
- Operator evidence must distinguish Forge overhead from provider latency.
- Missing/stale/rejected outputs must remain inspectable through durable readback.
- Before P069, the control plane must still provide understandable readback rather than opaque raw local inference.

### Acceptance Criteria
- Fresh ACP startup sends `initialize` before broad traversal.
- Pre-prompt metadata is bounded and refreshed for both fresh and reused sessions.
- Required outputs settle through accepted discovery decisions rather than raw target-path rereads.
- Bounded meta-root discovery stays supplemental-only.
- Legacy broad discovery is disabled by default and auditable when used.
- `proposal-053|p053` is the deterministic proof path.
- P069, not P053, owns the future macOS rendering.

### Test / Evidence Requirements
- `docs/proposals/053.review/cap-validation.json` exists and matches the Phase 0 schema.
- `docs/proposals/053.review/security-checklist.md` (or risk acceptance) exists.
- `docs/proposals/053.review/manual-latency-spot-check.md` exists and records the observed `acp_pre_initialize_local_latency_ms`.
- `docs/proposals/053.review/operator-clarity-evidence.md` exists.
- `docs/proposals/053.review/phase-1-retrospective.md` exists.
- `./scripts/test-gate.sh proposal-053` passes on the audited tree.

### Explicit Exclusions
- No P053 macOS UI implementation.
- No ACP transport switch to HTTP.
- No default broad discovery fallback.
- No second output-validation system outside P057/P058.

## Proposal Fidelity / Divergence

### Matches
- The worktree now implements the full structured P053 metrics/readback surface that `R2` previously flagged as incomplete.
- The Phase 0 cap-validation artifact now includes the proposal-declared schema fields that were previously missing.
- The new Phase 1 evidence pack files now exist in-tree.
- The current tree still preserves the explicit P069 UI deferral and server-owned P053 readback contract.

### Divergences
- The current audited worktree is dirty, while the recorded `proposal-053` pass lives in `docs/proposals/053.review/proposal-053-gate-2026-04-23.md` and does not cover these uncommitted code changes.
- The new manual latency spot-check note records a pass conclusion, but not the observed `acp_pre_initialize_local_latency_ms` value explicitly required by the proposal.

### Ambiguities / Evidence Gaps
- The current tree may well pass `proposal-053`, but this audit did not rerun the gate and the existing gate artifact is stale relative to the dirty worktree.
- The new operator-clarity note is a qualitative artifact, not runtime evidence produced in this audit.

## Requirement Summary

| Status | Count |
|---|---:|
| Implemented | 13 |
| Partially Implemented | 1 |
| Missing | 0 |
| Not Verifiable | 1 |

## Requirement Audit

### REQ-001 Fresh ACP startup sends `initialize` before broad traversal
- Proposal Source: Goals (`lines 43-44`), ACP Execution Sequence (`lines 102-105`), Behavioral Acceptance Criteria (`lines 817-819`)
- Status: Implemented
- Evidence Type: `code`, `tests-found`
- Evidence:
  - `control-plane/crates/acp/src/transport.rs:1064`
  - `control-plane/crates/acp/src/transport.rs:1072`
  - `docs/proposals/053.review/proposal-053-gate-2026-04-23.md:13-22`
- Gap / Note: The startup timing boundary remains aligned with the proposal contract.

### REQ-002 Typed expected-output specs remain the engine-owned discovery contract
- Proposal Source: Expected Output Specs (`lines 152-178`), Implementation Contracts (`lines 833-839`)
- Status: Implemented
- Evidence Type: `code`
- Evidence:
  - `control-plane/crates/domain/src/discovery.rs:215`
  - `control-plane/crates/engine/src/contracts.rs:371`
  - `control-plane/crates/engine/src/contracts.rs:434`
- Gap / Note: The current tree still builds typed specs in the engine and uses them as the discovery contract.

### REQ-003 Fresh and reused sessions capture bounded pre-prompt metadata per prompt turn
- Proposal Source: ACP Execution Sequence (`lines 100-106`), Pre-Prompt Metadata Bounds (`lines 180-231`)
- Status: Implemented
- Evidence Type: `code`
- Evidence:
  - `control-plane/crates/acp/src/transport.rs:1298`
  - `control-plane/crates/acp/src/transport.rs:1315`
  - `control-plane/crates/acp/src/transport.rs:1346-1366`
- Gap / Note: The current transport now records both latency and timeout/digest budget details for this phase.

### REQ-004 Provider-envelope and `CHAINWORKS_OUTPUT` cap parity is enforced before acceptance
- Proposal Source: Executive Summary (`lines 18-20`), Phase 1 exit criteria (`lines 666-668`)
- Status: Implemented
- Evidence Type: `code`, `tests-found`
- Evidence:
  - `control-plane/crates/engine/src/executor.rs:3311-3355`
  - `docs/proposals/053.review/security-checklist.md:21-24`
  - `docs/proposals/053.review/proposal-053-gate-2026-04-23.md:20-21`
- Gap / Note: Cap parity remains explicitly covered by the security artifact and gate scope.

### REQ-005 `DiscoveryFilesystem` operation recording exists under `domain::discovery`
- Proposal Source: Phase 0 freeze (`lines 610-613`), R9 resolution (`line 855`)
- Status: Implemented
- Evidence Type: `code`, `tests-found`
- Evidence:
  - `control-plane/crates/domain/src/discovery.rs:80-115`
  - `docs/proposals/053.review/cap-validation.json:99-107`
  - `docs/proposals/053.review/proposal-053-gate-2026-04-23.md:16-17`
- Gap / Note: This remains closed relative to stale `R1`.

### REQ-006 Supplemental discovery is bounded to the current run meta-root and remains supplemental-only
- Proposal Source: Goals (`line 48`), ACP Execution Sequence (`line 110`), Phase 1 exit criteria (`lines 670-671`)
- Status: Implemented
- Evidence Type: `code`
- Evidence:
  - `control-plane/crates/engine/src/executor.rs:3352-3355`
  - `control-plane/crates/engine/src/executor.rs:3513`
  - `docs/proposals/053.review/security-checklist.md:38-39`
- Gap / Note: Current executor code still treats bounded meta-root discovery as a separate downstream engine phase.

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
  - `control-plane/crates/db/tests/proposal_053_discovery_diagnostics.rs:320`
  - `control-plane/crates/graphql-server/tests/proposal_058_runtime_facts.rs:264`
  - `control-plane/crates/mcp-server/tests/proposal_058_runtime_facts.rs:247`
- Gap / Note: Current test coverage now asserts the richer diagnostics payload fields through DB/GraphQL/MCP readback.

### REQ-009 The full structured metrics surface declared by P053 exists in source
- Proposal Source: Metrics and Observability (`lines 726-777`), Phase 1 exit criteria (`line 672`)
- Status: Implemented
- Evidence Type: `code`
- Evidence:
  - `control-plane/crates/acp/src/lib.rs:142-157`
  - `control-plane/crates/acp/src/session.rs:45-59`
  - `control-plane/crates/acp/src/transport.rs:1064-1130`
  - `control-plane/crates/acp/src/transport.rs:1346-1366`
  - `control-plane/crates/engine/src/executor.rs:1601-1627`
  - `control-plane/crates/engine/src/executor.rs:3263-3355`
  - `control-plane/crates/engine/src/executor.rs:3513-3554`
- Gap / Note: Repo-wide search on this worktree now finds all 36 required structured fields. This closes the metrics blocker from `R2`.

### REQ-010 Phase 0 cap-validation artifact matches the declared schema surface
- Proposal Source: Dependencies and Readiness (`lines 79-88`), Evidence schema (`lines 615-655`)
- Status: Implemented
- Evidence Type: `code`, `tests-found`
- Evidence:
  - `docs/proposals/053.review/cap-validation.json:1-114`
  - `scripts/test-gate.sh:2414-2464`
  - `docs/reference/test-gates.md:917-955`
- Gap / Note: The current artifact now includes the schema fields that were missing in `R2`, including `generated_at`, envelope/aggregate cap selections, discovery owner, and non-empty `sampled_execution_ids`.

### REQ-011 Phase 1 security artifact exists
- Proposal Source: Security review exit criterion (`lines 679-683`)
- Status: Implemented
- Evidence Type: `tests-found`
- Evidence:
  - `docs/proposals/053.review/security-checklist.md:1-44`
  - `scripts/test-gate.sh:2399-2413`
- Gap / Note: This remained closed from `R2` onward.

### REQ-012 Phase 1 manual latency spot-check artifact exists and records the observed `acp_pre_initialize_local_latency_ms`
- Proposal Source: Phase 1 exit criteria (`lines 673-675`)
- Status: Partially Implemented
- Evidence Type: `code`
- Evidence:
  - `docs/proposals/053.review/manual-latency-spot-check.md:26-39`
  - `control-plane/crates/acp/src/transport.rs:1064-1070`
- Gap / Note: The new artifact exists and records a pass conclusion, but it does not record the observed `acp_pre_initialize_local_latency_ms` value itself. The proposal text asks for the observed metric to be recorded as evidence.

### REQ-013 Phase 1 operator-clarity evidence exists
- Proposal Source: Phase 1 exit criteria (`line 675`)
- Status: Implemented
- Evidence Type: `code`
- Evidence:
  - `docs/proposals/053.review/operator-clarity-evidence.md:11-24`
  - `docs/proposals/053.review/operator-clarity-evidence.md:43-51`
  - `control-plane/crates/acp/src/transport.rs:1064-1130`
  - `control-plane/crates/engine/src/executor.rs:3301-3355`
- Gap / Note: The artifact is qualitative rather than runtime-generated in this audit, but it satisfies the proposal's requirement to record clarity evidence in-tree.

### REQ-014 Phase 1 retrospective decision artifact exists
- Proposal Source: Phase 1 exit criteria (`line 676`)
- Status: Implemented
- Evidence Type: `code`
- Evidence:
  - `docs/proposals/053.review/phase-1-retrospective.md:19-29`
  - `scripts/test-gate.sh:2480-2484`
- Gap / Note: The new retrospective doc closes the missing-retrospective gap from `R2`.

### REQ-015 P053 closeout does not depend on macOS UI implementation
- Proposal Source: Status (`line 6`), UI Deferral to P069 (`lines 90-94`), Deferred Phase 3 (`lines 708-714`)
- Status: Implemented
- Evidence Type: `code`
- Evidence:
  - `docs/proposals/069-p053-discovery-diagnostics-operator-ui.md:1-9`
  - `docs/reference/artifact-discovery-and-settlement-optimization.md:32-38`
- Gap / Note: Current proposal/reference truth remains aligned that P069 owns the macOS UI.

### REQ-016 Canonical `proposal-053|p053` proof exists for the current audited tree
- Proposal Source: Behavioral Acceptance Criteria (`line 818`), Phase 1 exit criteria (`line 677`), Implementation Contracts (`line 845`)
- Status: Not Verifiable
- Evidence Type: `tests-found`, `inference`
- Evidence:
  - `docs/proposals/053.review/proposal-053-gate-2026-04-23.md:1-26`
  - `git status --short --branch && git rev-parse HEAD`
  - `scripts/test-gate.sh:2391-2475`
- Gap / Note: The branch has recorded same-branch gate evidence, but the current audited worktree is dirty with additional code changes after that recorded pass. Because this audit did not rerun `./scripts/test-gate.sh proposal-053`, the available proof is stale relative to the audited tree and cannot back a successful closeout verdict.

## Architecture Review

**Summary:** Acceptable

No new architecture finding. The current dirty tree closes the substantive control-plane design gaps from `R2`: metrics, diagnostics projection, cap-validation schema, and evidence-pack structure now line up with the revised P053 contract.

## Product Review

**Summary:** Acceptable

### PROD-001 Current value remains internal closeout value, not production exposure value
- Severity: Note
- Confidence: High
- Related Proposal Items / Requirements: `REQ-010`, `REQ-013`
- Evidence Type: `code`
- Evidence:
  - `docs/proposals/053.review/cap-validation.json:7-27`
  - `docs/proposals/053.review/security-checklist.md:12-26`
- Why It Matters: This branch is still correctly positioned as `gate_only_internal`. That is acceptable for P053 closeout, but it should not be misread as production readiness.
- Recommended Action: Keep production-exposure claims blocked on refreshed sampling/signoff, exactly as the current artifacts already say.

## UI Review

**Summary:** Acceptable

No current UI finding. P053 UI rendering is explicitly deferred to P069 and should not be reintroduced as a P053 blocker.

## UX Review

**Summary:** Acceptable

No current UX blocker beyond stale gate freshness. The new operator-clarity note is sufficient as an in-tree artifact for this proposal slice, even though it is not runtime evidence captured by this audit.

## Delivery / Readiness Review

**Summary:** Not Ready

### READY-001 Recorded `proposal-053` pass is stale relative to the current dirty worktree
- Severity: Major
- Confidence: High
- Related Proposal Items / Requirements: `REQ-016`
- Evidence Type: `tests-found`, `inference`
- Evidence:
  - `docs/proposals/053.review/proposal-053-gate-2026-04-23.md:1-26`
  - `git status --short --branch && git rev-parse HEAD`
- Why It Matters: The proposal requires same-tree proof for a successful closeout verdict. The current worktree contains uncommitted code changes in ACP, domain, DB, engine, tests, gate docs, and evidence artifacts. The recorded gate pass therefore does not prove the current audited tree.
- Recommended Action: Rerun `./scripts/test-gate.sh proposal-053` on the current worktree and record fresh same-tree evidence.

### READY-002 Manual latency spot-check artifact still omits the observed latency value required by the proposal
- Severity: Minor
- Confidence: High
- Related Proposal Items / Requirements: `REQ-012`
- Evidence Type: `code`
- Evidence:
  - `docs/proposals/053.review/manual-latency-spot-check.md:26-39`
  - `control-plane/crates/acp/src/transport.rs:1064-1070`
- Why It Matters: The proposal did not ask only for a narrative pass/fail note; it asked to record the observed `acp_pre_initialize_local_latency_ms`. Without that number, the artifact is weaker than the committed exit criterion.
- Recommended Action: Amend the spot-check note to include the observed metric value from the run that justified the pass decision.

### READY-003 `R2` blockers around schema, metrics, and missing evidence pack are now closed on the current tree
- Severity: Note
- Confidence: High
- Related Proposal Items / Requirements: `REQ-009`, `REQ-010`, `REQ-013`, `REQ-014`
- Evidence Type: `code`
- Evidence:
  - `docs/proposals/053.review/cap-validation.json:14-112`
  - `docs/proposals/053.review/manual-latency-spot-check.md:1-39`
  - `docs/proposals/053.review/operator-clarity-evidence.md:1-51`
  - `docs/proposals/053.review/phase-1-retrospective.md:1-29`
  - `scripts/test-gate.sh:2399-2464`
- Why It Matters: Future audits should not repeat the stale `R2` findings about missing schema fields, missing Phase 1 evidence docs, or incomplete metrics coverage. Those are now closed on the current tree.
- Recommended Action: Keep any next audit focused only on fresh gate proof and the remaining spot-check content gap.

## Readiness Checklist

| Check | Status | Evidence / Note |
|---|---|---|
| Build succeeds on targeted platform(s) | Not Checked | No build/test command was run in this audit. |
| Core user flow runtime-validated | Partial | Existing branch evidence says `proposal-053` passed, but that proof is stale relative to the current dirty worktree. |
| Empty/loading/error states covered | Not Checked | P053 UI rendering is deferred to P069. |
| Accessibility risk acceptable | Not Checked | P053 UI rendering is deferred to P069. |
| Localization risk acceptable | Not Checked | P053 UI rendering is deferred to P069. |
| Critical tests executed | Not Checked | This audit inspected `tests-found` evidence only and did not rerun the gate. |
| Full regression suite / canonical full gate passed on same tree/HEAD | Partial | Branch evidence exists, but it is not same-tree for the current dirty worktree. |
| Privacy/permissions/entitlements reviewed | Partial | Security checklist exists for `gate_only_internal`; it is not a production signoff. |

## Verification Log

- `git status --short --branch && git rev-parse HEAD`
- `python3 /Users/user/.agents/skills/proposal-implementation-audit/scripts/report_path.py /Users/user/Documents/'Chainworks Forge'/.chainworks/worktrees/codex-p053-manual-merge-1833dd16/docs/proposals/053-bounded-acp-artifact-discovery-and-startup-latency.md`
- `git diff --unified=20 -- control-plane/crates/acp/src/lib.rs control-plane/crates/acp/src/session.rs control-plane/crates/acp/src/transport.rs control-plane/crates/domain/src/discovery.rs control-plane/crates/db/src/repos/agent_execution_discovery_diagnostics.rs control-plane/crates/engine/src/executor.rs`
- `git diff --unified=20 -- control-plane/crates/db/tests/proposal_053_discovery_diagnostics.rs control-plane/crates/engine/tests/integration.rs control-plane/crates/engine/tests/proposal_041_parity.rs control-plane/crates/graphql-server/tests/proposal_058_runtime_facts.rs control-plane/crates/mcp-server/tests/proposal_058_runtime_facts.rs scripts/test-gate.sh docs/reference/test-gates.md docs/proposals/053.review/cap-validation.json`
- `nl -ba docs/proposals/053.review/manual-latency-spot-check.md | sed -n '1,220p'`
- `nl -ba docs/proposals/053.review/operator-clarity-evidence.md | sed -n '1,220p'`
- `nl -ba docs/proposals/053.review/phase-1-retrospective.md | sed -n '1,220p'`
- repo-wide metrics coverage script against the proposal's `Required structured fields` list
- cap-validation schema presence script against the proposal's `Evidence schema` list
- targeted line reads for ACP transport/session/lib, executor diagnostics wiring, DB diagnostics readback, gate script, gate reference docs, and cap-validation artifact

## Recommended Next Actions

1. Rerun `./scripts/test-gate.sh proposal-053` on the current worktree and record fresh same-tree evidence.
2. Amend `docs/proposals/053.review/manual-latency-spot-check.md` to include the observed `acp_pre_initialize_local_latency_ms` value.
3. After those two steps, refresh the audit once more; the remaining substantive `R2` blockers are already closed on this tree.
