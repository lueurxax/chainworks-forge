# Proposal 048 Evidence Packs, Delivery Preflight, and MCP Resolution Multi-Lens Audit R5

| Field | Value |
|---|---|
| Proposal | `docs/proposals/048-evidence-packs-delivery-preflight-and-mcp-resolution.md` |
| Repository Root | `/Users/user/Documents/Chainworks Forge` |
| Git SHA | `af3054c73064b05e42cb816a81a3c5fb0c2e29d9` |
| Working Tree | Dirty; audit reflects current working tree, not clean HEAD |
| Audited At | `2026-04-16T12:31:03+03:00` |
| Platform Scope | macOS-hosted Rust control-plane / northbound API surfaces; no screen-level UI scope |
| Proposal State | Active |
| Overall Conformance | Partial |
| Overall Readiness | Not Ready |
| Audit Confidence | High |

## Executive Verdict

The current implementation satisfies the explicit P048 focused proposal contract: `./scripts/test-gate.sh proposal-048` passed on this tree, including DB persistence, delivery-preflight blocked/pass paths, ACP `session/new.mcpServers`, fail-closed MCP persistence, failed-stage evidence, GraphQL execution truth, and MCP report/resource readback. The audit cannot roll up to `Implemented` or `Ready` because the mandatory same-tree full regression gate is unavailable on this host and exits before running.

## Lens Scorecard

| Lens | Assessment | Top Risk | Confidence |
|---|---|---|---|
| Conformance | Focused P048 implemented; roll-up partial | Full regression unavailable on this host | High |
| Architecture | Acceptable | One non-functional module-placement divergence from the proposal file table | High |
| Product | Acceptable with release caveat | Operator diagnostics are focused-gate proven but not full-regression signed off | Medium |
| UI | Acceptable | No UI scope in P048 | High |
| UX | Acceptable | Diagnostic truth is machine-readable in focused tests | High |
| Readiness | Not Ready | `./scripts/test-gate.sh full` is host-policy blocked | High |

## Proposal Contract

### Scope

P048 is a Rust control-plane delta covering three slices:

- stage-owned failed-stage evidence packets, including stage-owned `recovery_snapshot`, without creating a second export truth lane
- run-creation-time delivery preflight persistence and blocking semantics
- execution-time MCP resolution and northbound exposure from `backend_profile.mcp -> ResolvedAgent -> AgentExecution`

### Locked Decisions

| Decision | Source |
|---|---|
| Failed-stage evidence is stage-owned, persists to `stage_executions.evidence_packet_json`, mirrors stage-owned `recovery_snapshot_json`, and rides the existing report artifact lane. | `docs/proposals/048-evidence-packs-delivery-preflight-and-mcp-resolution.md:127-216`, `497-507` |
| Delivery preflight runs during `StartRun` only when `delivery_configuration_json` is present, blocks before run creation when failed, and persists `delivery_preflight_json` on successful created runs. | `docs/proposals/048-evidence-packs-delivery-preflight-and-mcp-resolution.md:218-297`, `508-516` |
| GraphQL blocked-start truth is a typed union/payload, not `errors[].extensions`; MCP `runs.start` returns the same typed delivery-preflight truth. | `docs/proposals/048-evidence-packs-delivery-preflight-and-mcp-resolution.md:272-291`, `510-513` |
| Requested MCP intent comes from `ResolvedAgent.requested_mcp_server_ids`, with profile identity from `ResolvedAgent.backend_profile_id`; `required_tools` is not MCP authority. | `docs/proposals/048-evidence-packs-delivery-preflight-and-mcp-resolution.md:77-99`, `299-358`, `520-522` |
| Missing, disabled, unsupported, or malformed MCP registry entries fail closed before ACP session startup and persist denied/blocking truth on `AgentExecution`. | `docs/proposals/048-evidence-packs-delivery-preflight-and-mcp-resolution.md:351-405`, `523-527` |
| GraphQL and MCP readers expose the same durable P048 truth from `runs`, `stage_executions`, artifacts, and `AgentExecution` rows. | `docs/proposals/048-evidence-packs-delivery-preflight-and-mcp-resolution.md:407-435`, `528-536` |
| `proposal-048|p048` is the canonical focused proof path for this slice. | `docs/proposals/048-evidence-packs-delivery-preflight-and-mcp-resolution.md:540-583` |

### Primary User Flows

| Flow | User job |
|---|---|
| PF-1 | Start a repo-backed run and receive either a typed delivery-preflight block or a created run with persisted delivery-preflight truth. |
| PF-2 | Diagnose a failed stage through stage-owned validation, recovery snapshot, evidence packet, and normal report artifacts. |
| PF-3 | Execute an MCP-enabled agent, fail closed before ACP startup when MCP cannot be realized, and inspect requested/predicted/actual/denied/blocking truth. |
| PF-4 | Read the same P048 truth through GraphQL, MCP tools, MCP resources, and reports without reconstructing state from logs. |
| PF-5 | Reproduce the proposal slice through `./scripts/test-gate.sh proposal-048`. |

### UI Commitments

None. P048 is a backend/northbound control-plane proposal and does not define visual screens or app navigation.

### UX Commitments

P048's UX commitment is diagnostic clarity through typed blocked-start payloads, durable failed-stage evidence, recovery snapshots, and explicit northbound MCP truth.

### Acceptance Criteria

The audit treats proposal lines `497-536` as the explicit acceptance surface for failed-stage evidence, delivery preflight, MCP ownership/resolution, and northbound readers.

### Test / Evidence Requirements

P048 requires `./scripts/test-gate.sh proposal-048` as the canonical focused proof lane. The audit skill additionally requires same-tree full regression evidence before reporting a successful `Implemented`, `Ready`, or `Ready with Risks` roll-up.

### Explicit Exclusions

P048 excludes run-export evidence pack design, cohort/sign-off evidence pack design, broad workflow `PreflightService`, start-time MCP warning UX beyond executor fail-closed behavior, and redesign of the machine-local MCP registry format or ownership.

## Proposal Fidelity / Divergence

### Matches

- `./scripts/test-gate.sh proposal-048` passed on the audited working tree.
- DB migration and repo/domain rows cover `stage_executions.validation_failure_json`, `stage_executions.evidence_packet_json`, `stage_executions.recovery_snapshot_json`, `runs.delivery_preflight_json`, and `AgentExecution` MCP provenance fields.
- Delivery preflight blocks before run creation on failure and persists run-owned preflight truth on success.
- GraphQL `startRun` returns a typed blocked-start payload, and MCP `runs.start` returns typed `delivery_preflight` truth for blocked starts.
- MCP requested/predicted/denied/blocking truth is persisted before ACP startup; actual truth is explicit for both blocked and successful MCP executions.
- ACP `session/new` receives `mcpServers` keyed by runtime ID and preserves extension provenance inside the internal payload.
- GraphQL stage reads expose execution-level MCP truth through `GqlStageExecution.executions`.
- MCP `reports.get` and `report://{run_id}` expose failed-stage evidence and execution-level MCP truth.

### Divergences

- The proposal file table names `graphql-server/src/types/agent_execution.rs` as a new file, but the implementation colocates `GqlAgentExecution` in `graphql-server/src/types/stage.rs`. The behavior is proven, but the module shape diverges from the proposal's file-level guidance.
- The implemented `proposal-048` gate is broader than the proposal's illustrative snippet: it includes DB persistence, ACP serialization, report resource proof, and explicit blocked-before-session actual truth. This is favorable for proof quality.
- The audit cannot produce a successful roll-up because `./scripts/test-gate.sh full` is remote-only on this host.

### Ambiguities / Evidence Gaps

- Full regression evidence is unavailable locally: `./scripts/test-gate.sh full` exits with host-policy error before running UI/full checks.
- The working tree is dirty and includes unrelated P029/P047/P049 changes, so this is a current-worktree audit rather than a clean branch sign-off.
- No live daemon/manual GraphQL/MCP runtime smoke was run; the audit relies on focused Rust tests and code inspection, which are appropriate for this backend proposal but not a substitute for full release sign-off.

## Requirement Summary

| Status | Count |
|---|---:|
| Implemented | 12 |
| Partially Implemented | 0 |
| Missing | 0 |
| Not Verifiable | 0 |

All explicit P048 proposal requirements below are implemented by direct code evidence and the passing focused gate. The overall conformance roll-up remains `Partial` only because the audit rules require same-tree full regression evidence for an `Implemented` verdict.

## Requirement Audit

### REQ-001 Persistence fields and round-trip storage

- Proposal Source: `docs/proposals/048-evidence-packs-delivery-preflight-and-mcp-resolution.md:455-485`, `487-491`
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `control-plane/crates/db/migrations/010_evidence_preflight_and_mcp.sql:1`
  - `control-plane/crates/domain/src/stage.rs:105`
  - `control-plane/crates/domain/src/run.rs:122`
  - `control-plane/crates/domain/src/agent.rs:59`
  - `control-plane/crates/db/tests/integration.rs:556`
  - `./scripts/test-gate.sh proposal-048` passed, including `cargo test -p db --test integration proposal_048_persistence_fields_round_trip -- --exact --nocapture`
- Gap / Note: Historical rows remain nullable as proposed.

### REQ-002 Failed-stage evidence packet, recovery mirroring, and artifact lane

- Proposal Source: `docs/proposals/048-evidence-packs-delivery-preflight-and-mcp-resolution.md:127-216`, `497-507`
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `control-plane/crates/engine/src/recovery.rs:27`
  - `control-plane/crates/engine/src/evidence.rs:21`
  - `control-plane/crates/engine/src/evidence.rs:92`
  - `control-plane/crates/engine/src/evidence.rs:128`
  - `control-plane/crates/engine/src/executor.rs:767`
  - `control-plane/crates/engine/src/orchestrator.rs:418`
  - `./scripts/test-gate.sh proposal-048` passed, including `cargo test -p engine failed_stage_evidence_packet_tests -- --nocapture`
- Gap / Note: The packet embeds `recovery_snapshot` from the stage-owned record and persists a normal `failed_stage_evidence` report artifact.

### REQ-003 Delivery-preflight checks and StartRun blocking semantics

- Proposal Source: `docs/proposals/048-evidence-packs-delivery-preflight-and-mcp-resolution.md:218-271`, `508-516`
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `control-plane/crates/engine/src/preflight.rs:25`
  - `control-plane/crates/engine/src/command_handler.rs:150`
  - `control-plane/crates/engine/src/command_handler.rs:156`
  - `control-plane/crates/engine/src/command_handler.rs:200`
  - `control-plane/crates/engine/tests/integration.rs:1545`
  - `control-plane/crates/engine/tests/integration.rs:1603`
  - `./scripts/test-gate.sh proposal-048` passed, including `cargo test -p engine --test integration delivery_preflight -- --nocapture`
- Gap / Note: The implementation checks repo root existence, git repository validity, base branch, worktree writability, release target ID, and repo identifier.

### REQ-004 Typed blocked-start transport across GraphQL and MCP

- Proposal Source: `docs/proposals/048-evidence-packs-delivery-preflight-and-mcp-resolution.md:272-291`, `510-513`
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `control-plane/crates/graphql-server/src/schema.rs:202`
  - `control-plane/crates/graphql-server/src/schema.rs:247`
  - `control-plane/crates/graphql-server/src/schema.rs:340`
  - `control-plane/crates/graphql-server/src/schema.rs:998`
  - `control-plane/crates/mcp-server/src/tools/runs.rs:121`
  - `control-plane/crates/mcp-server/src/tools/runs.rs:126`
  - `./scripts/test-gate.sh proposal-048` passed, including `cargo test -p graphql-server --lib start_run_blocked_preflight_returns_typed_payload -- --nocapture`
- Gap / Note: GraphQL uses typed result payloads rather than transport-level GraphQL errors for this domain outcome.

### REQ-005 Persisted delivery-preflight readback through GraphQL, MCP `runs.get`, and `run://`

- Proposal Source: `docs/proposals/048-evidence-packs-delivery-preflight-and-mcp-resolution.md:293-297`, `530`, `534`
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `control-plane/crates/graphql-server/src/types/run.rs:34`
  - `control-plane/crates/graphql-server/src/types/run.rs:54`
  - `control-plane/crates/graphql-server/src/schema.rs:1141`
  - `control-plane/crates/mcp-server/src/tools/runs.rs:373`
  - `control-plane/crates/mcp-server/src/server.rs:384`
  - `control-plane/crates/mcp-server/src/server.rs:848`
  - `./scripts/test-gate.sh proposal-048` passed, including GraphQL and MCP delivery-preflight readback tests
- Gap / Note: Successful run truth is persisted on the run; blocked-start truth remains a transport result and does not create a run resource.

### REQ-006 MCP intent owner is `backend_profile.mcp -> ResolvedAgent`, not `required_tools`

- Proposal Source: `docs/proposals/048-evidence-packs-delivery-preflight-and-mcp-resolution.md:77-99`, `299-317`, `520-522`
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `control-plane/crates/workflow/src/compiler.rs:275`
  - `control-plane/crates/workflow/src/compiler.rs:345`
  - `control-plane/crates/workflow/src/plan.rs:57`
  - `control-plane/crates/workflow/src/plan.rs:72`
  - `control-plane/crates/engine/src/executor.rs:512`
  - `control-plane/crates/engine/src/executor.rs:515`
  - `./scripts/test-gate.sh proposal-048` passed, including MCP resolution persistence tests
- Gap / Note: `required_tools` still exists in catalog data, but it is not used by the audited MCP resolver path.

### REQ-007 Executor-time registry resolution and fail-closed MCP behavior

- Proposal Source: `docs/proposals/048-evidence-packs-delivery-preflight-and-mcp-resolution.md:345-358`, `436-451`, `523-525`
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `control-plane/crates/engine/src/mcp.rs:61`
  - `control-plane/crates/engine/src/mcp.rs:74`
  - `control-plane/crates/engine/src/mcp.rs:99`
  - `control-plane/crates/engine/src/mcp.rs:109`
  - `control-plane/crates/engine/src/mcp.rs:174`
  - `control-plane/crates/engine/src/mcp.rs:226`
  - `control-plane/crates/engine/src/executor.rs:729`
  - `./scripts/test-gate.sh proposal-048` passed, including `cargo test -p engine --test integration mcp_resolution_persistence_tests -- --nocapture`
- Gap / Note: Registry path resolution honors `CHAINWORKS_CODEX_CONFIG_PATH`, `~/.config/mcp/config.yaml`, and legacy `~/.config/goose/config.yaml`.

### REQ-008 ACP `mcpServers` payload serialization and runtime-ID ownership

- Proposal Source: `docs/proposals/048-evidence-packs-delivery-preflight-and-mcp-resolution.md:359-394`, `526`
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `control-plane/crates/acp/src/lib.rs:57`
  - `control-plane/crates/acp/src/lib.rs:97`
  - `control-plane/crates/acp/src/transport.rs:113`
  - `control-plane/crates/acp/src/transport.rs:127`
  - `control-plane/crates/acp/tests/integration.rs:446`
  - `./scripts/test-gate.sh proposal-048` passed, including `cargo test -p acp --test integration mcp_servers_session_new_serialization_tests -- --exact --nocapture`
- Gap / Note: The payload uses runtime ID as `mcpServers[].id` and preserves extension ID as provenance.

### REQ-009 AgentExecution MCP truth persistence, including blocked-before-session actual truth

- Proposal Source: `docs/proposals/048-evidence-packs-delivery-preflight-and-mcp-resolution.md:396-405`, `523-524`
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `control-plane/crates/domain/src/agent.rs:59`
  - `control-plane/crates/db/src/repos/agent_executions.rs:17`
  - `control-plane/crates/db/src/repos/agent_executions.rs:87`
  - `control-plane/crates/db/src/repos/agent_executions.rs:119`
  - `control-plane/crates/engine/src/executor.rs:679`
  - `control-plane/crates/engine/src/executor.rs:738`
  - `control-plane/crates/engine/src/executor.rs:898`
  - `./scripts/test-gate.sh proposal-048` passed, including MCP resolution persistence tests
- Gap / Note: Blocked executions persist explicit empty actual arrays and an authoritative no-session observation.

### REQ-010 GraphQL stage execution relation and execution-level MCP truth

- Proposal Source: `docs/proposals/048-evidence-packs-delivery-preflight-and-mcp-resolution.md:418-420`, `531-532`
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `control-plane/crates/graphql-server/src/types/stage.rs:28`
  - `control-plane/crates/graphql-server/src/types/stage.rs:75`
  - `control-plane/crates/graphql-server/src/types/stage.rs:84`
  - `control-plane/crates/graphql-server/src/schema.rs:1224`
  - `./scripts/test-gate.sh proposal-048` passed, including `cargo test -p graphql-server --lib execution_mcp_truth_contract_tests -- --nocapture`
- Gap / Note: The type is implemented in `types/stage.rs` rather than a dedicated `types/agent_execution.rs` file.

### REQ-011 MCP `reports.get` and `report://` failed-stage evidence and execution truth

- Proposal Source: `docs/proposals/048-evidence-packs-delivery-preflight-and-mcp-resolution.md:421-424`, `504`, `535`
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `control-plane/crates/mcp-server/src/tools/reports.rs:72`
  - `control-plane/crates/mcp-server/src/tools/reports.rs:145`
  - `control-plane/crates/mcp-server/src/tools/reports.rs:661`
  - `control-plane/crates/mcp-server/src/tools/reports.rs:703`
  - `control-plane/crates/mcp-server/src/server.rs:940`
  - `control-plane/crates/mcp-server/src/server.rs:979`
  - `./scripts/test-gate.sh proposal-048` passed, including MCP report/resource failed-stage evidence and MCP truth tests
- Gap / Note: Both report tool and report resource paths source execution truth from persisted rows.

### REQ-012 Canonical focused `proposal-048|p048` gate and reference docs

- Proposal Source: `docs/proposals/048-evidence-packs-delivery-preflight-and-mcp-resolution.md:540-583`
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `docs/reference/test-gates.md:591`
  - `docs/reference/test-gates.md:595`
  - `scripts/test-gate.sh:1516`
  - `scripts/test-gate.sh:1519`
  - `./scripts/test-gate.sh proposal-048` passed
- Gap / Note: The implemented gate is stronger than the proposal snippet because it includes ACP serialization and report-resource assertions.

## Architecture Review

**Summary:** Acceptable

### ARCH-001 Focused control-plane architecture is implemented

- Severity: Note
- Confidence: High
- Related Proposal Items / Requirements: REQ-001 through REQ-012
- Evidence Type: code, tests-run
- Evidence:
  - `./scripts/test-gate.sh proposal-048` passed
  - `control-plane/crates/engine/src/evidence.rs:21`
  - `control-plane/crates/engine/src/preflight.rs:25`
  - `control-plane/crates/engine/src/mcp.rs:61`
  - `control-plane/crates/engine/src/executor.rs:515`
  - `control-plane/crates/acp/src/transport.rs:127`
- Why It Matters: P048's three ownership boundaries are all represented in the current implementation and covered by the focused gate.
- Recommended Action: Preserve the focused gate as the proposal-owned regression lane.

### ARCH-002 GraphQL execution type is colocated rather than split into the proposal-named new file

- Severity: Minor
- Confidence: High
- Related Proposal Items / Requirements: REQ-010
- Evidence Type: code
- Evidence:
  - `docs/proposals/048-evidence-packs-delivery-preflight-and-mcp-resolution.md:478`
  - `control-plane/crates/graphql-server/src/types/stage.rs:28`
  - `control-plane/crates/graphql-server/src/types/mod.rs:1`
  - `rg --files control-plane/crates/graphql-server/src/types` shows no `agent_execution.rs`
- Why It Matters: Behavior is implemented and tested, but the module shape diverges from the proposal's file-level guidance. This can confuse future maintainers looking for execution-owned GraphQL types.
- Recommended Action: Either split `GqlAgentExecution` into `types/agent_execution.rs` before clean sign-off, or explicitly accept the colocation as an implementation decision in a follow-up doc/proposal note.

## Product Review

**Summary:** Acceptable with release caveat

### PROD-001 Operator diagnostics are focused-gate proven but not full-regression signed off

- Severity: Minor
- Confidence: Medium
- Related Proposal Items / Requirements: PF-1, PF-2, PF-3, PF-4, REQ-001 through REQ-012
- Evidence Type: tests-run
- Evidence:
  - `./scripts/test-gate.sh proposal-048` passed
  - `./scripts/test-gate.sh full` exited before running on this host
- Why It Matters: The P048 operator value is present in focused tests, but release handoff still needs the broader project gate on an approved host or CI.
- Recommended Action: Run the full gate on `smacbook.local`, `smacbook`, or CI before treating this branch as globally ready.

## UI Review

**Summary:** Acceptable

### UI-001 No screen-level UI requirements

- Severity: Note
- Confidence: High
- Related Proposal Items / Requirements: none
- Evidence Type: proposal
- Evidence:
  - `docs/proposals/048-evidence-packs-delivery-preflight-and-mcp-resolution.md:407-424`
  - `docs/proposals/048-evidence-packs-delivery-preflight-and-mcp-resolution.md:587-593`
- Why It Matters: P048 is a backend/northbound API proposal. No app visual implementation should be blocked by this audit.
- Recommended Action: None for P048.

## UX Review

**Summary:** Acceptable

### UX-001 Diagnostic ambiguity is resolved in focused proof

- Severity: Note
- Confidence: High
- Related Proposal Items / Requirements: PF-2, PF-3, PF-4, REQ-002, REQ-004, REQ-009, REQ-011
- Evidence Type: tests-run, code
- Evidence:
  - `./scripts/test-gate.sh proposal-048` passed
  - `control-plane/crates/engine/src/executor.rs:738`
  - `control-plane/crates/engine/src/executor.rs:740`
  - `control-plane/crates/mcp-server/src/tools/reports.rs:72`
  - `control-plane/crates/mcp-server/src/server.rs:940`
- Why It Matters: Operators can distinguish blocked-before-session MCP outcomes from absent data, and failed-stage evidence includes recovery context without log reconstruction.
- Recommended Action: Keep blocked-before-session actual truth and failed-stage evidence readback in the canonical gate.

## Delivery / Readiness Review

**Summary:** Not Ready

### READY-001 Full gate is unavailable on this host

- Severity: Critical
- Confidence: High
- Related Proposal Items / Requirements: audit roll-up rule, REQ-012
- Evidence Type: tests-run
- Evidence:
  - `./scripts/test-gate.sh full`
  - Exit code: `3`
  - Error: `UI tests are remote-only and may not run on this host.`
  - Approved remote hosts: `smacbook.local,smacbook`
  - Observed host names: `0000659.localdomain,0000659`
- Why It Matters: The audit skill requires same-tree full regression evidence before a successful `Implemented`, `Ready`, or `Ready with Risks` verdict. The focused P048 gate is green, but full sign-off cannot be collected locally.
- Recommended Action: Run `./scripts/test-gate.sh full` on an approved remote host or CI, then rerun this audit if a successful roll-up is needed.

### READY-002 Dirty worktree makes this a current-tree audit, not clean release sign-off

- Severity: Minor
- Confidence: High
- Related Proposal Items / Requirements: release/handoff readiness
- Evidence Type: code
- Evidence:
  - `git status --short` shows broad modified and untracked files across P029/P047/P048/P049/control-plane surfaces
- Why It Matters: The focused P048 behavior is proven on the current tree, but the result is not isolated from unrelated proposal work. That is acceptable for an implementation audit, but not for final release sign-off.
- Recommended Action: Before final handoff, run the P048 and full gates on the intended integration branch or clean worktree snapshot.

## Readiness Checklist

| Check | Status | Evidence / Note |
|---|---|---|
| Build succeeds on targeted platform(s) | Partial | Rust crates in the focused P048 gate built and tested; full Xcode/UI/full gate did not run locally. |
| Core user flow runtime-validated | Pass for focused P048 scope | `./scripts/test-gate.sh proposal-048` passed. |
| Empty/loading/error states covered | Not Applicable | No screen-level UI scope. |
| Accessibility risk acceptable | Not Applicable | No UI scope. |
| Localization risk acceptable | Not Applicable | Backend/northbound contract proposal. |
| Critical tests executed | Pass for focused P048; full sign-off unavailable | Focused gate passed; full gate host-policy blocked. |
| Full regression suite / canonical full gate passed on same tree/HEAD | Fail / unavailable | `./scripts/test-gate.sh full` exited before running because this host is not approved for remote-only UI tests. |
| Privacy/permissions/entitlements reviewed | Not Applicable | No Apple entitlement/sandbox change in P048 scope. |

## Verification Log

- `pwd`
- `git rev-parse --show-toplevel && git rev-parse HEAD && git status --short`
- `python3 /Users/user/.agents/skills/proposal-implementation-audit/scripts/report_path.py /Users/user/Documents/Chainworks\ Forge/docs/proposals/048-evidence-packs-delivery-preflight-and-mcp-resolution.md`
- `date -Iseconds`
- `nl -ba docs/proposals/048-evidence-packs-delivery-preflight-and-mcp-resolution.md | sed -n '1,260p'`
- `nl -ba docs/proposals/048-evidence-packs-delivery-preflight-and-mcp-resolution.md | sed -n '260,620p'`
- `rg -n "superseded|deprecated|replaced|obsolete|Proposal 048|P048|evidence pack|preflight|MCP" docs/proposals docs/reference scripts control-plane/crates -g '*.md' -g '*.sh' -g '*.rs'`
- `sed -n '560,620p' docs/reference/test-gates.md && sed -n '1500,1545p' scripts/test-gate.sh`
- `./scripts/test-gate.sh proposal-048`
- `./scripts/test-gate.sh full`
- `rg -n "evidence_packet_json|recovery_snapshot_json|validation_failure_json|failed_stage_evidence|FailedStageEvidencePacket|build_failed|recovery_snapshot" control-plane/crates/engine/src control-plane/crates/db/src control-plane/crates/domain/src control-plane/crates/mcp-server/src control-plane/crates/graphql-server/src -g '*.rs'`
- `rg -n "DeliveryPreflight|delivery_preflight|StartRunBlockedByDeliveryPreflight|start_run_blocked|run://|runs.get|runs.start" control-plane/crates/engine/src control-plane/crates/graphql-server/src control-plane/crates/mcp-server/src control-plane/crates/db/src control-plane/crates/domain/src -g '*.rs'`
- `rg -n "McpResolution|McpActual|requested_mcp|predicted_mcp|actual_mcp|denied_mcp|mcp_blocking|mcpServers|AcpMcpServerPayload|ResolvedMcpServer|mcp_servers" control-plane/crates/engine/src control-plane/crates/acp/src control-plane/crates/graphql-server/src control-plane/crates/mcp-server/src control-plane/crates/db/src control-plane/crates/domain/src control-plane/crates/workflow/src -g '*.rs'`
- `rg --files control-plane/crates/graphql-server/src/types | sort`

## Recommended Next Actions

1. Run `./scripts/test-gate.sh full` on `smacbook.local`, `smacbook`, or CI for same-tree full-regression sign-off.
2. Decide whether to split `GqlAgentExecution` into `graphql-server/src/types/agent_execution.rs` or explicitly accept the current `types/stage.rs` colocation.
3. Rerun this audit after full gate evidence is available if the desired roll-up is `Implemented` / `Ready`.
