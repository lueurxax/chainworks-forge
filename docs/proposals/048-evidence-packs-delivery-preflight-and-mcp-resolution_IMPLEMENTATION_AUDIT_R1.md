# Proposal 048 Implementation Audit R1

## Verdict

| Field | Value |
|---|---|
| Proposal | `docs/proposals/048-evidence-packs-delivery-preflight-and-mcp-resolution.md` |
| Proposal state | Draft, not found superseded/replaced in nearby proposal/review/reference search |
| Audit timestamp | 2026-04-16T08:58:27+03:00 |
| Repository root | `/Users/user/Documents/Chainworks Forge` |
| Git SHA | `af3054c73064b05e42cb816a81a3c5fb0c2e29d9` |
| Working tree | Dirty; audit reflects current working tree, not clean HEAD |
| Platform scope | Rust control-plane / macOS host app backend surfaces; no screen-level UI commitments |
| Overall Conformance | Not Implemented |
| Overall Readiness | Not Ready |
| Audit Confidence | High |

P048 has substantial implementation in the working tree: DB/domain fields exist, delivery preflight core semantics are implemented and tested, MCP resolution is wired from `backend_profile.mcp` through executor-time resolution into ACP `session/new`, and GraphQL exposes execution rows through a stage relation.

The proposal is not closed. The canonical `proposal-048` gate is missing, `reports.get` and `report://{run_id}` still do not expose execution-level MCP truth, failed-stage evidence packets are thinner than the required V1 contract and are not built from the generic orchestrator failed-settlement owner boundary, and GraphQL blocked preflight exposes a JSON string rather than the typed `GqlDeliveryPreflight` payload promised by the proposal.

## Reproducibility

| Command | Result |
|---|---|
| `./scripts/test-gate.sh proposal-048` | Failed: `error: Unknown gate: proposal-048` |
| `cargo test -p engine --test integration delivery_preflight -- --nocapture` | Passed: 2 tests |
| `cargo test -p db --test integration proposal_048_persistence_fields_round_trip -- --exact --nocapture` | Passed: 1 test |
| `cargo test -p mcp-server --lib reports_get -- --nocapture` | Passed: 2 tests |
| `cargo test -p mcp-server --lib report_resource -- --nocapture` | Passed: 1 test |
| `cargo test -p graphql-server --lib delivery_configuration_json -- --nocapture` | Passed: 2 tests |

Full regression was not run because the proposal cannot receive a successful roll-up while the required canonical gate is absent and in-scope requirements are missing. Per the audit skill, successful readiness would require same-tree full regression evidence after all blocking gaps are closed.

## Proposal Contract

Primary user flows:

| Flow | User Job |
|---|---|
| PF-1 | Start a repo-backed run with delivery configuration and either receive a typed blocked-start preflight payload or a created run with persisted preflight truth. |
| PF-2 | Diagnose a failed stage from durable stage-owned validation, evidence, recovery, and report-lane artifacts. |
| PF-3 | Run an agent with MCP intent from `backend_profile.mcp`, fail closed before ACP startup when MCP cannot be realized, and inspect requested/predicted/actual/denied/blocking truth after execution. |
| PF-4 | Read the same P048 truth through GraphQL, MCP tools, and MCP resources without separate truth lanes. |
| PF-5 | Reproduce the slice through the repo-owned `./scripts/test-gate.sh proposal-048` wrapper. |

Locked proposal decisions:

| Area | Proposal Source |
|---|---|
| Failed-stage evidence is stage-owned and must ride the existing report artifact lane, not an export-pack namespace. | Lines 35-58, 127-216, 497-507 |
| Delivery preflight is run-creation validation; failed preflight blocks run creation/start. | Lines 59-75, 218-297, 508-516 |
| MCP intent authority is `backend_profile.mcp -> ResolvedAgent -> AgentExecution`; `required_tools` is not MCP authority. | Lines 77-99, 299-406, 518-527 |
| Northbound placement is explicit across GraphQL, MCP tools, MCP resources, and report surfaces. | Lines 407-435, 528-536 |
| `proposal-048|p048` gate is required and is the canonical proof path. | Lines 540-560 |

## Track 1: REQ Conformance

| ID | Requirement | Source | Status | Evidence | Gap / Note |
|---|---|---|---|---|---|
| REQ-001 | Add durable stage/run/agent DB and domain fields for validation failure, failed-stage evidence, recovery snapshot, delivery preflight, and MCP provenance. | Lines 195-208, 396-405, 455-485 | Implemented | `control-plane/crates/db/migrations/010_evidence_preflight_and_mcp.sql:1-16`; `domain/src/run.rs:120-122`; `domain/src/stage.rs:104-109`; `domain/src/agent.rs:59-68`; DB round-trip test passed | The persistence substrate exists and round-trips in `proposal_048_persistence_fields_round_trip`. |
| REQ-002 | Failed-stage evidence packet V1 carries required identity, timing, failure, raw/receipt/transcript existence, typed validation failure, typed output envelopes, and stage-owned recovery snapshot. | Lines 127-172 | Partially Implemented | `engine/src/evidence.rs:53-77` | Builder writes a packet, but it lacks required V1 fields such as `id`, `timestamp`, `stage_label`, `stage_attempt_number`, `failed_agent_title`, `supervision_classification`, `canonical_outcome`, `transport_error_kind`, `output_presence`, `raw_outputs_exist`, `receipt_exists`, and `transcript_exists`. `output_envelopes` is always `[]`, not extracted typed envelope truth. |
| REQ-003 | Recovery snapshot is computed by `engine/src/recovery.rs`, persisted to `stage_executions.recovery_snapshot_json`, and embedded by evidence without becoming a second recovery authority. | Lines 181-193, 505-507 | Partially Implemented | `engine/src/recovery.rs` implementation found; `engine/src/evidence.rs:36-40,76`; `engine/src/executor.rs:523-543,854-875` | The executor calls recovery before evidence on MCP-blocked and failed-agent paths, but the generic orchestrator settlement path only calls `stages::settle` and does not own the recovery/evidence sequence. |
| REQ-004 | Failed-stage evidence persists `evidence_packet_json`, writes a collision-safe canonical artifact path, and creates a normal `report_kind = "failed_stage_evidence"` artifact. | Lines 203-216, 499-504 | Implemented | `engine/src/evidence.rs:79-108`; `mcp-server/src/tools/reports.rs:94-104`; `mcp-server/src/server.rs:325-342` | Artifact path and report-kind lane exist. Packet completeness is covered separately by REQ-002. |
| REQ-005 | `stage_executions.validation_failure_json` is the canonical stage-owned copy of typed validation failure. | Lines 197, 205, 499 | Implemented | `engine/src/executor.rs:822-829`; `db/tests/integration.rs:359-410` | Validation failure JSON is persisted before failed-stage evidence construction on validation failure paths. |
| REQ-006 | Delivery preflight implements repo root, git repo, base branch, writable worktree base, release target, and repo identifier checks. | Lines 218-258 | Implemented | `engine/src/preflight.rs`; engine delivery preflight tests passed | The validator exists and focused engine tests pass. |
| REQ-007 | `StartRun` with `delivery_configuration_json` runs preflight before run DB creation, blocks failed starts, returns typed engine result, and persists passing `delivery_preflight_json`. | Lines 260-270, 510-515 | Implemented | `engine/src/command_handler.rs:125-174`; engine delivery preflight tests passed | The plan is compiled before preflight, but run insertion happens only after passing preflight. |
| REQ-008 | GraphQL `startRun` uses explicit union/payload for blocked preflight, not `errors[].extensions`. | Lines 272-290, 511-513 | Partially Implemented | `graphql-server/src/schema.rs:127-193`; GraphQL delivery-configuration tests passed | The union exists, but `GqlStartRunBlocked` exposes `delivery_preflight_json: String`, not the typed `delivery_preflight: GqlDeliveryPreflight` object promised by the proposal. Existing GraphQL tests cover delivery configuration success/read, not typed blocked-start shape. |
| REQ-009 | MCP `runs.start`, `runs.get`, and `run://{run_id}` expose delivery-preflight truth on the correct blocked-start and persisted run surfaces. | Lines 289-297, 415-417, 534 | Implemented | `mcp-server/src/tools/runs.rs:118-125,135-141`; `mcp-server/src/server.rs:397-421` | `runs.start` returns a `delivery_preflight` object on blocked starts; `runs.get` and `run://` serialize run-owned truth for created runs. |
| REQ-010 | Requested MCP intent comes only from `backend_profile.mcp` through `ResolvedAgent.requested_mcp_server_ids`; `required_tools` does not participate. | Lines 77-99, 353-355, 520-522 | Implemented | `workflow/src/compiler.rs:227,286-297`; `workflow/src/plan.rs:41-58`; `engine/src/executor.rs:292-304`; `rg` found `required_tools` only in catalog/schema definitions, not resolver flow | The canonical owner chain is implemented. |
| REQ-011 | Executor-time MCP registry resolution supports canonical/override/legacy paths, fail-closed denied/blocking persistence, and no daemon restart for registry edits. | Lines 345-358, 438-451, 523-525 | Implemented | `engine/src/mcp.rs:61-190,226-243`; `engine/src/executor.rs:300-314,501-514` | Registry is loaded at resolver call time and blocking issues are persisted before ACP startup. The legacy Goose file is used as a fallback source when canonical config is absent. |
| REQ-012 | `ExecutionRequest.mcp_servers` carries executable payloads keyed by runtime ID and ACP transport serializes them into `session/new.mcpServers`. | Lines 359-394, 526-527 | Implemented | `acp/src/lib.rs:57-60,96-117`; `engine/src/executor.rs:638-640`; `acp/src/transport.rs:653-655`; ACP tests present for `mcpServers` serialization | Runtime ID is `AcpMcpServerPayload.id`; raw command/args/env remain inside ACP payload structures, not northbound report reads. |
| REQ-013 | Requested/predicted/actual/denied/blocking MCP truth persists on `AgentExecution`. | Lines 396-405, 523-524 | Implemented | `domain/src/agent.rs:59-68`; `db/src/repos/agent_executions.rs`; `engine/src/executor.rs:501-510,654-672`; DB round-trip test passed | Persistence and actual observation update path exist. |
| REQ-014 | GraphQL stage reads add explicit `executions: [GqlAgentExecution!]!`, and `GqlAgentExecution` exposes persisted MCP truth. | Lines 418-420, 530-532 | Implemented | `graphql-server/src/types/stage.rs:28-90`; `graphql-server/src/schema.rs` stage query path inspected | The relation and fields exist. The implementation keeps `GqlAgentExecution` in `types/stage.rs` rather than a separate `types/agent_execution.rs`; that is a file-placement divergence, not a functional contract gap. |
| REQ-015 | `reports.get` and `report://{run_id}` expose the same execution-level MCP truth sourced from persisted `AgentExecution` rows. | Lines 422-423, 535 | Missing | `mcp-server/src/tools/reports.rs:32-47`; `mcp-server/src/server.rs:314-343`; MCP report tests passed only existing artifact payload behavior | `reports.get` returns an array of report artifacts only. `report://` returns run projection, stages, artifact index, and artifact payloads. Neither reads `agent_executions` nor emits an execution-level MCP truth array. |
| REQ-016 | Raw registry command/args/env are not promoted into operator-facing GraphQL, MCP reports, or resource reads. | Lines 392-394, 536 | Implemented | `graphql-server/src/types/stage.rs:38-47`; `mcp-server/src/tools/reports.rs:32-47`; `mcp-server/src/server.rs:314-343,397-421` | Operator-facing GraphQL exposes IDs/issues only. `run://` serializes `AgentExecution` rows, which do not include raw registry command/args/env fields. |
| REQ-017 | Add repo-owned `proposal-048|p048` gate in `scripts/test-gate.sh` and `docs/reference/test-gates.md`; canonical wrapper is `./scripts/test-gate.sh proposal-048`. | Lines 540-560 | Missing | `scripts/test-gate.sh:1170-1199,1438-1498`; `docs/reference/test-gates.md` search; command run failed unknown gate | The gate is absent from usage, dispatch, and docs. This alone blocks readiness. |
| REQ-018 | Focused proof scope covers delivery-preflight parity, failed-stage evidence readback, MCP realization/fail-closed persistence, and GraphQL stage execution parity. | Lines 548-554 | Partially Implemented | Tests found/run for delivery preflight, DB persistence, GraphQL delivery configuration, MCP report artifacts | Some focused tests exist, but the canonical P048 gate is absent and there is no focused proof that `reports.get`/`report://` expose execution-level MCP truth because that feature is missing. |

## Proposal Fidelity Inventory

Matches:

| Area | Evidence |
|---|---|
| Persistence substrate | Migration 010 and domain/repo fields match the proposal's stage/run/agent storage families. |
| Delivery preflight core | Engine blocked-start and passing-start tests pass; run creation is blocked before DB insertion on failed preflight. |
| MCP owner chain | Compiler uses `profile.mcp`; executor uses `requested_mcp_server_ids` and `backend_profile_id`; `required_tools` is not in the resolver path. |
| ACP handoff | `ExecutionRequest.mcp_servers` reaches ACP transport `session/new.mcpServers`. |
| GraphQL execution relation | `GqlStageExecution.executions` resolves persisted `AgentExecution` rows with MCP truth fields. |

Divergences:

| Area | Evidence |
|---|---|
| Canonical gate | `./scripts/test-gate.sh proposal-048` fails with `Unknown gate`. |
| MCP report/resource parity | `reports.get` and `report://` do not include execution-level MCP truth arrays. |
| Failed-stage evidence packet shape | Current packet omits several required V1 fields and leaves `output_envelopes` empty. |
| Failed-settlement ownership | Evidence/recovery calls live in executor failure branches, while orchestrator's generic failed settlement does not call the producer/builder sequence. |
| GraphQL blocked-start typing | Blocked GraphQL payload is `delivery_preflight_json: String`, not typed `GqlDeliveryPreflight`. |

Ambiguities / evidence gaps:

| Area | Note |
|---|---|
| Legacy MCP migration wording | Implementation uses `~/.config/goose/config.yaml` as fallback when canonical config is absent. The proposal says "one-time legacy migration source"; if literal file migration is required, that is not implemented. If fallback-source semantics are intended, current behavior is adequate. |
| Full failed-stage runtime proof | Code paths exist for executor-blocked and failed-agent paths, but the audit did not execute a live failed stage and did not find a P048 gate covering failed-stage evidence readback. |

## Track 2: Expert Findings

### ARCH-001: Failed-stage evidence is not owned by the generic failed-stage settlement boundary

| Field | Value |
|---|---|
| Severity | Major |
| Confidence | High |
| Related REQs | REQ-002, REQ-003 |
| Evidence types | code |
| Evidence references | `engine/src/executor.rs:523-543,854-875`; `engine/src/orchestrator.rs:363-384,436-445`; `engine/src/evidence.rs:53-77` |

Why it matters: P048 explicitly assigns recovery snapshot computation and failed-stage evidence construction to failed stage settlement. Current implementation covers executor-known MCP-blocked and failed-agent paths, but the orchestrator still settles any aggregate failed stage by calling `stages::settle` directly. That makes evidence completeness dependent on which branch produced the failure rather than the stage-settlement invariant.

Recommended action: Move or wrap failed-stage settlement in a single owner path that persists recovery snapshot first, then builds evidence, then settles/surfaces events. Keep executor branch-specific failure detail as input data, not as the owner of the invariant.

### ARCH-002: Failed-stage evidence packet is structurally incomplete for V1

| Field | Value |
|---|---|
| Severity | Major |
| Confidence | High |
| Related REQs | REQ-002, REQ-004 |
| Evidence types | code |
| Evidence references | `docs/proposals/048-evidence-packs-delivery-preflight-and-mcp-resolution.md:127-172`; `engine/src/evidence.rs:53-77` |

Why it matters: The current JSON packet is useful, but it is not the proposed V1 contract. Missing required fields make downstream recovery/report clients guess at output presence, transcript/receipt availability, stage attempt identity, and transport/supervision classification. That recreates the evidence ambiguity P048 is meant to remove.

Recommended action: Define the packet struct explicitly and fill every required field. If a nullable/deferred field has no Rust owner yet, emit it as `null`; if a required existence field cannot be computed, emit `false` with a bounded reason field or add a proposal amendment before implementation sign-off.

### ARCH-003: MCP report surfaces do not read persisted execution truth

| Field | Value |
|---|---|
| Severity | Major |
| Confidence | High |
| Related REQs | REQ-015 |
| Evidence types | code, tests-run |
| Evidence references | `mcp-server/src/tools/reports.rs:32-47`; `mcp-server/src/server.rs:314-343`; `cargo test -p mcp-server --lib reports_get -- --nocapture`; `cargo test -p mcp-server --lib report_resource -- --nocapture` |

Why it matters: The implementation persists MCP truth on `AgentExecution`, but the report tool/resource path still only returns report artifacts and projection data. Operators using MCP report surfaces cannot see denied/blocking MCP reasons even though the proposal requires report parity.

Recommended action: Add a shared report projection helper that loads `agent_executions::list_by_run` and emits an `agent_executions` or `mcp_executions` truth array from persisted rows. Use it from both `reports.get` and `report://{run_id}` tests to prove parity.

### PROD-001: Operator-facing report diagnosis remains incomplete for MCP failures

| Field | Value |
|---|---|
| Severity | Major |
| Confidence | High |
| Related REQs | REQ-011, REQ-013, REQ-015 |
| Evidence types | code, inference |
| Evidence references | `engine/src/executor.rs:501-514`; `mcp-server/src/tools/reports.rs:32-47`; `mcp-server/src/server.rs:314-343` |

Why it matters: The product goal is not just to fail closed; it is to make the failure explainable through northbound readers. A run blocked by missing MCP can persist the truth but still hide it from the report APIs most likely to be used for audit/export workflows.

Recommended action: Treat execution-level MCP truth as first-class report data. Include requested/predicted/actual/denied/blocking/startup latency fields in the report resource and `reports.get`, sourced from `AgentExecution`.

### UI-001: No screen-level UI commitments are in scope for P048

| Field | Value |
|---|---|
| Severity | Note |
| Confidence | High |
| Related REQs | None |
| Evidence types | proposal |
| Evidence references | Proposal lines 9, 407-424 |

Why it matters: This audit should not invent visual requirements. P048 is a control-plane/northbound truth proposal, not a macOS screen redesign.

Recommended action: No UI implementation action for P048. If the app later adds visual surfaces for delivery preflight or MCP blocking, audit those under a separate UI proposal or explicit P048 amendment.

### UX-001: GraphQL blocked-start payload is schema-hostile

| Field | Value |
|---|---|
| Severity | Major |
| Confidence | High |
| Related REQs | REQ-008 |
| Evidence types | code |
| Evidence references | `graphql-server/src/schema.rs:127-180`; Proposal lines 272-290 |

Why it matters: The union avoids GraphQL transport errors, which is good. But returning a JSON string forces every client to parse an opaque blob and prevents schema introspection from revealing checks, timestamps, and failure details. That is weaker than the typed `GqlDeliveryPreflight` payload P048 promised.

Recommended action: Add `GqlDeliveryPreflight` and `GqlPreflightCheck` objects. Keep `deliveryPreflightJson` only as backward-compatible raw access if needed, but make the typed field canonical for the blocked-start contract.

### READY-001: Canonical P048 proof gate is missing

| Field | Value |
|---|---|
| Severity | Critical |
| Confidence | High |
| Related REQs | REQ-017, REQ-018 |
| Evidence types | tests-run, code, docs |
| Evidence references | `./scripts/test-gate.sh proposal-048` failed unknown gate; `scripts/test-gate.sh:1170-1199,1438-1498`; `docs/reference/test-gates.md` search |

Why it matters: The proposal explicitly defines `./scripts/test-gate.sh proposal-048` as the canonical proof path and says later audits should not treat a generic workspace run as the only proof contract. Without that gate, no operator or reviewer can reproduce the promised slice with a single repo-owned command.

Recommended action: Add `proposal-048|p048` to `scripts/test-gate.sh` and `docs/reference/test-gates.md`. The gate should run delivery preflight, GraphQL/MCP/read-resource parity, failed-stage evidence report readback, ACP MCP realization/fail-closed tests, and GraphQL stage execution MCP parity.

## Readiness Notes

What is already usable:

| Area | Status |
|---|---|
| Delivery preflight engine behavior | Usable and focused tests pass. |
| DB persistence fields | Usable and round-trip test passes. |
| MCP resolver/executor/ACP handoff | Strong code evidence; needs full gate and end-to-end failure/readback proof. |
| GraphQL stage execution relation | Implemented and inspectable; needs parity tests for P048. |

What blocks sign-off:

| Blocker | Required fix |
|---|---|
| Missing `proposal-048` gate | Add script dispatch, docs entry, and focused proof suite. |
| Missing MCP truth in `reports.get` and `report://` | Add execution-level MCP truth arrays sourced from persisted `AgentExecution` rows. |
| Incomplete failed-stage evidence packet | Fill required V1 fields and output envelope/existence truth. |
| Failed settlement not centralized | Ensure recovery/evidence is generated for every newly failed P048-era stage at the settlement boundary. |
| GraphQL blocked preflight is raw JSON string | Add typed `GqlDeliveryPreflight` payload for blocked-start union. |

## Final Roll-Up

| Dimension | Result |
|---|---|
| Track 1 conformance | Not Implemented because REQ-015 and REQ-017 are Missing, and several other requirements are Partial. |
| Track 2 architecture/product readiness | Not Ready because report truth and settlement ownership still violate the proposal's central invariants. |
| Test confidence | Medium for implemented slices, low for full P048 because the canonical gate is absent. |
| Audit confidence | High because the blocking gaps are directly evidenced by code inspection and focused command output. |
