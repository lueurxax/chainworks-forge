# Proposal 048 Implementation Audit R2

## Verdict

| Field | Value |
|---|---|
| Proposal | `docs/proposals/048-evidence-packs-delivery-preflight-and-mcp-resolution.md` |
| Proposal state | Active draft; no superseding marker found in the proposal text or adjacent gate/reference artifacts inspected for this audit |
| Audit timestamp | `2026-04-16T10:25:16+03:00` |
| Repository root | `/Users/user/Documents/Chainworks Forge` |
| Git SHA | `af3054c73064b05e42cb816a81a3c5fb0c2e29d9` |
| Working tree | Dirty; audit reflects the current working tree, not clean HEAD |
| Platform scope | Rust control-plane / northbound API surfaces; no iOS/macOS screen-level UI commitments |
| Overall Conformance | Partial |
| Overall Readiness | Not Ready |
| Audit Confidence | High |

P048 is substantially implemented in the current working tree. The new `proposal-048` gate exists and passed on this tree, and the earlier R1 gaps around typed GraphQL blocked preflight, failed-stage evidence packet shape, MCP report/resource truth, ACP `mcpServers`, and failed-stage recovery/evidence ownership are materially improved.

The implementation is still not sign-off ready against the proposal text. MCP executions blocked before ACP startup persist denied/blocking truth but leave the promised actual MCP truth lane unset, and the canonical `proposal-048` gate is narrower than the focused proof scope explicitly listed in the proposal.

## Reproducibility

| Command | Result |
|---|---|
| `./scripts/test-gate.sh proposal-048` | Passed |

Observed passing sub-steps from the gate:

| Step | Result |
|---|---|
| `cargo test -p db --test integration proposal_048_persistence_fields_round_trip -- --exact --nocapture` | Passed: 1 test |
| `cargo test -p engine --test integration delivery_preflight -- --nocapture` | Passed: 2 tests |
| `cargo test -p acp --test integration mcp_servers_session_new_serialization_tests -- --exact --nocapture` | Passed: 1 test |
| `cargo test -p engine failed_stage_evidence_packet_tests -- --nocapture` | Passed: 1 test |
| `cargo test -p graphql-server --lib start_run_blocked_preflight_returns_typed_payload -- --nocapture` | Passed: 1 test |
| `cargo test -p mcp-server --lib reports_mcp_resolution_truth_tests -- --nocapture` | Passed: 1 test |
| `cargo test -p mcp-server --lib report_resource_exposes_mcp_execution_truth -- --nocapture` | Passed: 1 test |

Full repository regression was not run. Because this audit found proposal-level gaps, a successful full-regression roll-up would not change the readiness verdict.

## Proposal Contract Summary

Primary user flows:

| Flow | User job |
|---|---|
| PF-1 | Start a repo-backed run with delivery configuration and receive either a typed blocked-start preflight payload or a created run with persisted preflight truth. |
| PF-2 | Diagnose a failed stage from durable stage-owned validation, evidence, recovery, and report-lane artifacts. |
| PF-3 | Execute an agent with MCP intent from `backend_profile.mcp`, fail closed before ACP startup when MCP cannot be realized, and inspect requested/predicted/actual/denied/blocking truth after execution. |
| PF-4 | Read the same P048 truth through GraphQL, MCP tools, MCP resources, and report surfaces without separate truth lanes. |
| PF-5 | Reproduce the slice through the repo-owned `./scripts/test-gate.sh proposal-048` wrapper. |

Locked proposal decisions:

| Area | Proposal source |
|---|---|
| Failed-stage evidence is stage-owned and must ride the existing report artifact lane, not an export-pack namespace. | `docs/proposals/048-evidence-packs-delivery-preflight-and-mcp-resolution.md:127-216`, `:497-507` |
| Delivery preflight is run-creation validation; failed preflight blocks run creation/start. | `docs/proposals/048-evidence-packs-delivery-preflight-and-mcp-resolution.md:218-297`, `:508-516` |
| MCP intent authority is `backend_profile.mcp -> ResolvedAgent -> AgentExecution`; `required_tools` is not MCP authority. | `docs/proposals/048-evidence-packs-delivery-preflight-and-mcp-resolution.md:299-405`, `:518-527` |
| Northbound placement is explicit across GraphQL, MCP tools, MCP resources, and report surfaces. | `docs/proposals/048-evidence-packs-delivery-preflight-and-mcp-resolution.md:407-435`, `:528-536` |
| `proposal-048|p048` gate is required and has a listed focused proof scope. | `docs/proposals/048-evidence-packs-delivery-preflight-and-mcp-resolution.md:540-583` |

## Track 1: REQ Conformance

| ID | Requirement | Proposal source | Status | Evidence | Gap / note |
|---|---|---|---|---|---|
| REQ-001 | Add durable DB/domain fields for stage validation failure, failed-stage evidence, recovery snapshot, run delivery preflight, and agent MCP provenance. | `:396-405`, `:455-485` | Implemented | `control-plane/crates/db/migrations/010_evidence_preflight_and_mcp.sql:1-16`; `control-plane/crates/domain/src/run.rs:120-122`; `control-plane/crates/domain/src/stage.rs:104-109`; `control-plane/crates/domain/src/agent.rs:59-68`; gate DB test passed | Persistence substrate exists and round-trips. |
| REQ-002 | Failed-stage evidence packet carries the V1 identity, timing, failure, output presence, validation failure, output envelope, and recovery snapshot fields. | `:127-193`, `:497-507` | Implemented | `control-plane/crates/engine/src/evidence.rs:89-129`; `control-plane/crates/engine/src/evidence.rs:204-370`; gate evidence test passed | R1 packet-shape gap is fixed; packet now includes the required high-value V1 fields and mirrors persisted recovery truth. |
| REQ-003 | Failed-stage evidence persists to `stage_executions.evidence_packet_json`, writes a collision-safe artifact path, and creates a normal `report_kind = "failed_stage_evidence"` artifact. | `:203-216`, `:499-504` | Implemented | `control-plane/crates/engine/src/evidence.rs:130-160`; `control-plane/crates/mcp-server/src/tools/reports.rs:145-155` | Artifact/report lane exists. |
| REQ-004 | `engine/src/recovery.rs` owns the recovery snapshot producer and persists `stage_executions.recovery_snapshot_json` before failed-stage evidence construction for newly failed stages. | `:186-193`, `:505-506`, `:460-463` | Implemented | `control-plane/crates/engine/src/recovery.rs:27-59`; `control-plane/crates/engine/src/executor.rs:523-543`; `control-plane/crates/engine/src/executor.rs:854-875`; `control-plane/crates/engine/src/orchestrator.rs:384-401` | Both executor failure branches and the multi-task orchestrator failed settlement path persist recovery before evidence. |
| REQ-005 | Delivery preflight validates repo root, git repository, base branch, writable worktree base, release target, and repo identifier. | `:218-258`, `:508-516` | Implemented | `control-plane/crates/engine/src/preflight.rs:25-49`; gate delivery preflight tests passed | Checks are implemented in the new preflight module. |
| REQ-006 | `StartRun` with delivery configuration runs preflight before run creation, blocks failed preflight, returns a typed blocked result, and persists passing `delivery_preflight_json`. | `:260-297`, `:510-515` | Implemented | `control-plane/crates/engine/src/command_handler.rs` references; gate engine delivery preflight tests passed | Behavior is covered by the focused engine tests. |
| REQ-007 | GraphQL `startRun` exposes blocked delivery preflight through an explicit typed union/payload, not `errors[].extensions`. | `:413`, `:428-429`, `:511-513` | Implemented | `control-plane/crates/graphql-server/src/schema.rs:129-181`; `control-plane/crates/graphql-server/src/schema.rs:921-980`; gate GraphQL typed payload test passed | R1 raw-JSON GraphQL gap is fixed. |
| REQ-008 | MCP `runs.start`, `runs.get`, and `run://{run_id}` expose delivery-preflight truth on the correct blocked-start and persisted-run surfaces. | `:415-417`, `:534-535` | Implemented | `control-plane/crates/mcp-server/src/tools/runs.rs:124-130`; `control-plane/crates/mcp-server/src/server.rs:522-525` | Code evidence exists, but the current canonical gate no longer proves all three delivery-preflight readback surfaces; see READY-001. |
| REQ-009 | Requested MCP intent is read only from `ResolvedAgent.requested_mcp_server_ids`, profile identity from `backend_profile_id`, and `required_tools` does not participate. | `:299-318`, `:520-522` | Implemented | `control-plane/crates/engine/src/executor.rs:297-314`; `control-plane/crates/engine/src/mcp.rs:61-190`; prior R1 code search found no `required_tools` resolver path | Owner chain remains aligned with the proposal. |
| REQ-010 | Executor-time MCP registry resolution supports canonical/override/legacy source lookup, fail-closed denied/blocking persistence, and no daemon restart for registry edits. | `:345-358`, `:438-451`, `:523-525` | Implemented | `control-plane/crates/engine/src/mcp.rs:61-190`; `control-plane/crates/engine/src/mcp.rs:207-245`; `control-plane/crates/engine/src/executor.rs:300-314` | Resolver reads the machine-local registry during executor handling. |
| REQ-011 | ACP request carries executable MCP payloads keyed by runtime ID and serializes them into ACP `session/new.mcpServers` without promoting command/args/env into operator-facing readers. | `:359-394`, `:526-527`, `:536` | Implemented | `control-plane/crates/acp/src/lib.rs:57-60`; `control-plane/crates/acp/src/lib.rs:96-117`; `control-plane/crates/acp/src/transport.rs:654`; gate ACP serialization test passed | ACP handoff is implemented and tested. |
| REQ-012 | Requested, predicted, actual, denied, blocking, and latency MCP truth persists on `AgentExecution`. | `:314-316`, `:396-405`, `:523-524` | Partially Implemented | Success-path actual update: `control-plane/crates/engine/src/executor.rs:654-672`; blocked-path insertion: `control-plane/crates/engine/src/executor.rs:501-510`; blocked-path return: `control-plane/crates/engine/src/executor.rs:514-590` | Pre-session MCP blocks persist requested/predicted/denied/blocking fields but leave `actual_mcp_extensions_json`, `actual_mcp_runtime_ids_json`, and `actual_mcp_observation_json` as `None`. The promised actual truth lane is therefore absent for the fail-closed path. |
| REQ-013 | GraphQL stage reads add explicit `executions: [GqlAgentExecution!]!`, and `GqlAgentExecution` exposes persisted MCP truth from `AgentExecution`. | `:418-420`, `:530-532` | Implemented | `control-plane/crates/graphql-server/src/types/stage.rs:28-90` | Functional resolver exists. The gate does not currently prove this parity; see READY-001. |
| REQ-014 | `reports.get` and `report://{run_id}` expose execution-level MCP truth from persisted `AgentExecution` rows. | `:422-423`, `:535` | Implemented | `control-plane/crates/mcp-server/src/tools/reports.rs:45-63`; `control-plane/crates/mcp-server/src/tools/reports.rs:72-102`; `control-plane/crates/mcp-server/src/server.rs:440-450`; gate MCP report/resource tests passed | Reader surfaces now exist and are tested for MCP truth. The blocked-path actual-null issue is covered under REQ-012. |
| REQ-015 | Failed-stage evidence and validation failure stay on the typed artifact/report lane. | `:421`, `:433-434`, `:504` | Implemented | `control-plane/crates/mcp-server/src/tools/reports.rs:115-157`; `control-plane/crates/engine/src/evidence.rs:142-160` | Existing report lane is used. Gate proof for failed-stage evidence report readback is incomplete; see READY-001. |
| REQ-016 | Add and document `proposal-048|p048` in `scripts/test-gate.sh` and `docs/reference/test-gates.md`. | `:540-546`, `:562-582` | Implemented | `scripts/test-gate.sh:1199`; `scripts/test-gate.sh:1499-1511`; `docs/reference/test-gates.md:570-603`; gate command passed | The gate exists and is runnable. |
| REQ-017 | The `proposal-048|p048` gate covers the focused proof scope listed in the proposal. | `:548-554`, `:562-582` | Partially Implemented | Actual gate: `scripts/test-gate.sh:1503-1509`; docs entry: `docs/reference/test-gates.md:574-582`; gate command passed | The gate omits several proposal-listed proof buckets, including GraphQL stage `executions` MCP parity, GraphQL/`runs.get`/`run://` delivery-preflight readback parity, failed-stage evidence `reports.get`/`report://` readback, and engine fail-closed denied/blocking MCP persistence. |

## Proposal Fidelity Inventory

Matches:

| Area | Evidence |
|---|---|
| Persistence substrate | Migration 010 and domain structs include the P048 field families. |
| Delivery preflight core | The focused gate proves blocked-start and passing-start behavior. |
| Failed-stage evidence | Packet V1 shape, recovery mirroring, artifact path, and report kind are implemented. |
| MCP ACP handoff | `ExecutionRequest.mcp_servers` reaches ACP `session/new.mcpServers` and the focused ACP test passes. |
| Northbound MCP report/resource readers | `reports.get` and `report://{run_id}` now load execution-level truth from `AgentExecution` rows. |
| GraphQL stage execution relation | `GqlStageExecution.executions` resolves persisted `AgentExecution` rows. |

Divergences:

| Area | Evidence |
|---|---|
| Blocked MCP actual truth | `executor.rs:501-510` inserts blocked executions with actual fields `None`, then `executor.rs:514-590` exits the blocked path before the success-path actual update at `executor.rs:654-672`. |
| Canonical proof scope | Proposal lines `548-554` list five proof buckets; `scripts/test-gate.sh:1503-1509` runs a smaller subset. |
| Gate docs drift | `docs/reference/test-gates.md:574-582` documents the narrower current gate rather than the proposal-listed focused proof scope. |

Ambiguities / evidence gaps:

| Area | Note |
|---|---|
| Full repository regression | Not run. The focused P048 gate passed, but release-level confidence still needs the broader project regression lane if this branch is being prepared for merge. |
| Legacy MCP registry wording | Implementation treats `~/.config/goose/config.yaml` as an ongoing fallback when canonical config is absent. The proposal says “one-time legacy migration source”; this audit treats fallback behavior as acceptable for V1 because it still respects the stated source order and executor-time resolution goal. |

## Track 2: Expert Findings

### ARCH-001: Pre-session MCP blocks leave the actual truth lane unset

| Field | Value |
|---|---|
| Severity | Major |
| Confidence | High |
| Related REQs | REQ-012, REQ-014 |
| Evidence types | code |
| Evidence references | `docs/proposals/048-evidence-packs-delivery-preflight-and-mcp-resolution.md:314-316`; `docs/proposals/048-evidence-packs-delivery-preflight-and-mcp-resolution.md:396-405`; `control-plane/crates/engine/src/executor.rs:501-510`; `control-plane/crates/engine/src/executor.rs:514-590`; `control-plane/crates/engine/src/executor.rs:654-672` |

Why it matters: P048 distinguishes predicted and actual MCP truth so operators can tell what was requested, what the resolver expected, and what actually happened. For the fail-closed pre-session path, the implementation creates an `AgentExecution`, persists denied/blocking truth, marks it failed, and returns before writing `actual_mcp_extensions_json`, `actual_mcp_runtime_ids_json`, or `actual_mcp_observation_json`. Readers therefore see `NULL` actual fields instead of explicit “no ACP session started; actual accepted MCP set is empty” truth.

Recommended action: On the blocked path, persist actual MCP fields as empty arrays and write an observation record such as `source = "mcp_resolution_blocked_before_session_new"`, `trust_level = "authoritative_no_session"`, and `actual_equals_predicted = false` when predicted was non-empty. Add an engine test that drives a missing/disabled MCP registry entry and asserts the failed `AgentExecution` has requested/predicted/actual/denied/blocking truth populated before any ACP call.

### PROD-001: Blocked MCP diagnosis is still ambiguous for northbound operators

| Field | Value |
|---|---|
| Severity | Major |
| Confidence | High |
| Related REQs | REQ-012, REQ-014 |
| Evidence types | code, inference |
| Evidence references | `control-plane/crates/mcp-server/src/tools/reports.rs:72-102`; `control-plane/crates/mcp-server/src/server.rs:440-450`; `control-plane/crates/engine/src/executor.rs:501-510` |

Why it matters: The report/resource surfaces now correctly expose persisted execution rows, but they can only expose what the executor persisted. For MCP-resolution blocks, operators get denied/blocking fields but cannot distinguish an unknown actual result from an explicit no-session/no-actual-runtime outcome without inferring from the blocking issue text.

Recommended action: Normalize blocked MCP executions into a complete persisted diagnostic record. Keep report/resource readers simple and truth-preserving by loading the stored actual-empty/observation fields rather than reconstructing blocked semantics at read time.

### UI-001: No screen-level UI commitments are in scope for P048

| Field | Value |
|---|---|
| Severity | Note |
| Confidence | High |
| Related REQs | None |
| Evidence types | proposal |
| Evidence references | `docs/proposals/048-evidence-packs-delivery-preflight-and-mcp-resolution.md:407-424`; `docs/proposals/048-evidence-packs-delivery-preflight-and-mcp-resolution.md:528-536` |

Why it matters: This audit should not invent visual requirements. P048 is a control-plane and northbound truth proposal, not a macOS or iOS screen redesign.

Recommended action: No UI implementation action is required for P048. If app screens later surface delivery-preflight or MCP-blocked state, audit those under a separate UI proposal or explicit P048 amendment.

### UX-001: Machine-readable blocked MCP state needs an explicit terminal actual outcome

| Field | Value |
|---|---|
| Severity | Major |
| Confidence | High |
| Related REQs | REQ-012 |
| Evidence types | code, inference |
| Evidence references | `control-plane/crates/engine/src/executor.rs:501-510`; `control-plane/crates/engine/src/executor.rs:514-590` |

Why it matters: The proposal’s UX for operators is northbound clarity rather than a screen. A `NULL` actual lane forces clients to add their own interpretation rules: `NULL` could mean not observed yet, not applicable, legacy row, or blocked before session. That is exactly the kind of multi-reader ambiguity P048 is trying to remove.

Recommended action: Make blocked execution actual truth explicit in storage and expose it unchanged. Use `actual_mcp_* = []` plus a structured observation source to distinguish blocked-before-start from legacy missing data.

### READY-001: The canonical P048 gate is narrower than the proposal’s own proof scope

| Field | Value |
|---|---|
| Severity | Major |
| Confidence | High |
| Related REQs | REQ-017 |
| Evidence types | code, tests-run |
| Evidence references | `docs/proposals/048-evidence-packs-delivery-preflight-and-mcp-resolution.md:548-554`; `docs/proposals/048-evidence-packs-delivery-preflight-and-mcp-resolution.md:562-582`; `scripts/test-gate.sh:1503-1509`; `docs/reference/test-gates.md:574-582`; `./scripts/test-gate.sh proposal-048` passed |

Why it matters: The gate exists and passes, but it is not the proof lane described by the proposal. It does not prove GraphQL stage `executions` MCP parity, failed-stage evidence `reports.get` / `report://` readback, GraphQL / `runs.get` / `run://` delivery-preflight readback parity, or engine fail-closed MCP persistence. Passing this narrower gate can give reviewers a false green signal.

Recommended action: Expand `proposal-048|p048` to cover the proposal-listed buckets or amend the proposal to match the intentionally narrower gate. At minimum, add an engine missing/disabled MCP resolution persistence test, a GraphQL stage `executions` MCP truth test, delivery-preflight readback parity tests, and failed-stage evidence report/resource readback tests.

## Readiness Notes

What is already usable:

| Area | Status |
|---|---|
| Delivery preflight blocked/passing behavior | Usable; focused engine and GraphQL tests pass. |
| Failed-stage evidence packet and artifact lane | Usable; packet shape test passes and report artifact payload code exists. |
| MCP resolver and ACP payload handoff | Usable for successful ACP startup; serialization test passes. |
| MCP report/resource reader parity | Usable for persisted execution rows; focused MCP tests pass. |
| GraphQL stage execution resolver | Implemented in code. |

What blocks sign-off:

| Blocker | Required fix |
|---|---|
| Blocked MCP actual truth is unset | Persist explicit empty actual arrays and a blocked-before-session observation on the fail-closed path. |
| Canonical gate proof scope is incomplete | Expand `proposal-048|p048` and `docs/reference/test-gates.md` to match proposal lines `548-554` or amend the proposal. |

## Final Roll-Up

| Dimension | Result |
|---|---|
| Track 1 conformance | Partial because REQ-012 and REQ-017 are only partially implemented. |
| Track 2 architecture/product/UX readiness | Not Ready because blocked MCP diagnostics still have an ambiguous actual lane. |
| Delivery readiness | Not Ready because the repo-owned proof gate passes but does not cover the full proposal-listed proof scope. |
| Test confidence | Medium-high for the covered focused slices; medium overall because important proof buckets are absent from the canonical gate. |
| Audit confidence | High because the remaining gaps are directly evidenced by proposal text, code paths, and the executed gate output. |
