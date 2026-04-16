# Proposal 048 Evidence Packs, Delivery Preflight, and MCP Resolution Multi-Lens Audit R3

| Field | Value |
|---|---|
| Proposal | `docs/proposals/048-evidence-packs-delivery-preflight-and-mcp-resolution.md` |
| Repository Root | `/Users/user/Documents/Chainworks Forge` |
| Git SHA | `af3054c73064b05e42cb816a81a3c5fb0c2e29d9` |
| Working Tree | Dirty; audit reflects current working tree, not clean HEAD |
| Audited At | `2026-04-16T11:41:37+03:00` |
| Platform Scope | macOS-hosted Rust control-plane / northbound API surfaces; no screen-level UI scope |
| Proposal State | Active |
| Overall Conformance | Partial |
| Overall Readiness | Not Ready |
| Audit Confidence | High |

## Executive Verdict

The current implementation closes the R2 functional gap for pre-session MCP blocks by persisting explicit empty actual MCP truth plus a blocked-before-session observation, and the canonical `proposal-048` gate has been expanded to include ACP, delivery-preflight readback, failed-stage evidence readback, and GraphQL execution truth. However, the current `./scripts/test-gate.sh proposal-048` run fails on `mcp_servers_session_new_serialization_tests` with an ACP initialize handshake timeout, so P048 cannot be signed off as implemented or ready on this tree.

## Lens Scorecard

| Lens | Assessment | Top Risk | Confidence |
|---|---|---|---|
| Conformance | Partial | Canonical P048 gate fails at ACP transport serialization proof | High |
| Architecture | At Risk | ACP `mcpServers` serialization code exists but its contract test is red | High |
| Product | At Risk | Operator-facing MCP diagnosis is mostly present, but the proof lane cannot produce a green handoff | Medium |
| UI | Acceptable | No UI requirements in this proposal | High |
| UX | Acceptable with backend risk | Northbound diagnostic clarity is improved, but failed gate blocks trust in the flow | Medium |
| Readiness | Not Ready | Focused gate is red; no full regression was run | High |

## Proposal Contract

### Scope

P048 covers three Rust control-plane slices: failed-stage evidence packets, delivery preflight for repo-backed run start, and execution-time MCP resolution through GraphQL/MCP/northbound reader surfaces.

### Locked Decisions

| Decision | Source |
|---|---|
| Failed-stage evidence is stage-owned and uses the existing artifact/report lane. | `docs/proposals/048-evidence-packs-delivery-preflight-and-mcp-resolution.md:497-507` |
| Delivery preflight runs before run creation/start and blocks failed starts with typed payloads. | `docs/proposals/048-evidence-packs-delivery-preflight-and-mcp-resolution.md:508-516` |
| MCP intent comes from `ResolvedAgent.requested_mcp_server_ids`, not `required_tools`. | `docs/proposals/048-evidence-packs-delivery-preflight-and-mcp-resolution.md:520-522` |
| Missing/disabled/unsupported MCP entries fail closed before ACP startup and persist durable denied/blocking truth. | `docs/proposals/048-evidence-packs-delivery-preflight-and-mcp-resolution.md:523-527` |
| GraphQL and MCP northbound readers must expose persisted P048 truth from durable rows. | `docs/proposals/048-evidence-packs-delivery-preflight-and-mcp-resolution.md:528-536` |
| `proposal-048|p048` is the canonical proof path for the slice. | `docs/proposals/048-evidence-packs-delivery-preflight-and-mcp-resolution.md:540-583` |

### Primary User Flows

| Flow | User job |
|---|---|
| PF-1 | Start a repo-backed run and get either a typed delivery-preflight block or a created run with persisted preflight truth. |
| PF-2 | Diagnose a failed stage through stage-owned validation, recovery, evidence packet, and report-lane artifacts. |
| PF-3 | Execute an MCP-enabled agent, fail closed before ACP startup when MCP cannot be realized, and inspect requested/predicted/actual/denied/blocking truth. |
| PF-4 | Read the same P048 truth through GraphQL, MCP tools, MCP resources, and reports without reconstructing state from logs. |
| PF-5 | Reproduce the proposal slice through `./scripts/test-gate.sh proposal-048`. |

### UI Commitments

None. P048 is a backend/northbound control-plane proposal.

### UX Commitments

The proposal’s UX commitment is diagnostic clarity through typed blocked payloads, durable failed-stage evidence, recovery snapshots, and northbound MCP truth.

### Acceptance Criteria

The audit treats proposal lines `497-536` as the acceptance surface for failed-stage evidence, delivery preflight, MCP ownership/resolution, and northbound readers.

### Test / Evidence Requirements

The audit treats proposal lines `540-583` and the current `scripts/test-gate.sh` implementation as the canonical focused proof lane. A successful implementation verdict would also require same-tree full regression evidence, which was not run because the focused P048 gate is already red.

### Explicit Exclusions

No runtime UI/simulator evidence was required. Broad run-start MCP warnings and separate MCP readiness summaries remain deferred by proposal lines `436-451`.

## Proposal Fidelity / Divergence

### Matches

- Migration/domain/repo fields for stage evidence/recovery, run delivery preflight, and agent MCP provenance exist.
- Delivery preflight has engine behavior and GraphQL/MCP readback tests in the expanded gate.
- Pre-session MCP block now persists explicit empty actual truth and a structured blocked-before-session observation.
- ACP transport serializes `ExecutionRequest.mcp_servers` into `session/new.mcpServers` in code.
- GraphQL stage `executions` and MCP report/resource execution truth surfaces exist.
- The `proposal-048|p048` script entry now includes ACP, MCP fail-closed, delivery-preflight readback, failed-stage evidence readback, and GraphQL execution truth commands.

### Divergences

- The current canonical gate fails on the ACP serialization test before later proof buckets can run.
- The proposal text still lists persisted execution truth without naming `actual_mcp_observation_json`, while the implementation added that field to resolve the blocked actual-truth ambiguity.
- `docs/reference/test-gates.md` has the P048 scope text, but the visible entry inspected in this audit does not include the command block before the P049 heading.

### Ambiguities / Evidence Gaps

- The failed ACP test times out during the initialize handshake, so this audit cannot prove whether the fixture, adapter, or transport path is the root cause without debugging beyond audit scope.
- Full repository regression was not run because the focused P048 gate is already red.

## Requirement Summary

| Status | Count |
|---|---:|
| Implemented | 14 |
| Partially Implemented | 3 |
| Missing | 0 |
| Not Verifiable | 0 |

## Requirement Audit

### REQ-001 Persistence fields

- Proposal Source: `docs/proposals/048-evidence-packs-delivery-preflight-and-mcp-resolution.md:455-485`
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `control-plane/crates/db/migrations/010_evidence_preflight_and_mcp.sql:1`
  - `control-plane/crates/domain/src/run.rs:120`
  - `control-plane/crates/domain/src/stage.rs:104`
  - `control-plane/crates/domain/src/agent.rs:59`
  - `cargo test -p db --test integration proposal_048_persistence_fields_round_trip -- --exact --nocapture` passed inside `./scripts/test-gate.sh proposal-048`
- Gap / Note: Durable field substrate exists and round-trips.

### REQ-002 Failed-stage evidence packet and artifact lane

- Proposal Source: `docs/proposals/048-evidence-packs-delivery-preflight-and-mcp-resolution.md:497-507`
- Status: Implemented
- Evidence Type: code, tests-found
- Evidence:
  - `control-plane/crates/engine/src/evidence.rs:89`
  - `control-plane/crates/engine/src/evidence.rs:130`
  - `control-plane/crates/mcp-server/src/tools/reports.rs:145`
  - `control-plane/crates/mcp-server/src/tools/reports.rs:703`
  - `control-plane/crates/mcp-server/src/server.rs:979`
- Gap / Note: Evidence packet, persisted stage JSON, artifact payload decoding, and report resource readback paths exist. The focused gate did not reach these tests because it failed earlier at ACP.

### REQ-003 Recovery snapshot producer before evidence construction

- Proposal Source: `docs/proposals/048-evidence-packs-delivery-preflight-and-mcp-resolution.md:505-506`
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `control-plane/crates/engine/src/recovery.rs:27`
  - `control-plane/crates/engine/src/executor.rs:573`
  - `control-plane/crates/engine/src/orchestrator.rs:384`
- Gap / Note: Failed executor and orchestrator settlement paths persist recovery before evidence construction.

### REQ-004 Delivery preflight start behavior

- Proposal Source: `docs/proposals/048-evidence-packs-delivery-preflight-and-mcp-resolution.md:508-516`
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `control-plane/crates/engine/src/preflight.rs:25`
  - `control-plane/crates/engine/src/command_handler.rs:147`
  - `cargo test -p engine --test integration delivery_preflight -- --nocapture` passed inside `./scripts/test-gate.sh proposal-048`
- Gap / Note: Passing and blocked start behavior passed in the current gate before the ACP failure.

### REQ-005 Delivery-preflight northbound readback

- Proposal Source: `docs/proposals/048-evidence-packs-delivery-preflight-and-mcp-resolution.md:530-535`
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `control-plane/crates/graphql-server/src/schema.rs:1141`
  - `control-plane/crates/mcp-server/src/tools/runs.rs:373`
  - `control-plane/crates/mcp-server/src/server.rs:545`
  - `cargo test -p graphql-server --lib delivery_preflight_graphql_readback_tests -- --nocapture` passed inside `./scripts/test-gate.sh proposal-048`
  - `cargo test -p mcp-server --lib delivery_preflight_mcp_readback_tests -- --nocapture` passed inside `./scripts/test-gate.sh proposal-048`
- Gap / Note: GraphQL and MCP readback proof passed before the ACP failure.

### REQ-006 Typed GraphQL blocked-start payload

- Proposal Source: `docs/proposals/048-evidence-packs-delivery-preflight-and-mcp-resolution.md:511-513`
- Status: Implemented
- Evidence Type: code, tests-found
- Evidence:
  - `control-plane/crates/graphql-server/src/schema.rs:247`
  - `control-plane/crates/graphql-server/src/schema.rs:342`
  - `control-plane/crates/graphql-server/src/schema.rs:921`
- Gap / Note: Typed union/payload exists. The current gate did not reach this test because ACP failed earlier.

### REQ-007 MCP intent owner and resolver source

- Proposal Source: `docs/proposals/048-evidence-packs-delivery-preflight-and-mcp-resolution.md:520-522`
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `control-plane/crates/engine/src/executor.rs:297`
  - `control-plane/crates/engine/src/mcp.rs:61`
- Gap / Note: Requested MCP server IDs flow through executor-time resolution from the resolved agent payload.

### REQ-008 Fail-closed MCP denied/blocking persistence

- Proposal Source: `docs/proposals/048-evidence-packs-delivery-preflight-and-mcp-resolution.md:523-527`
- Status: Implemented
- Evidence Type: code, tests-found
- Evidence:
  - `control-plane/crates/engine/src/executor.rs:523`
  - `control-plane/crates/engine/src/executor.rs:535`
  - `control-plane/crates/engine/tests/integration.rs:1653`
  - `control-plane/crates/engine/tests/integration.rs:1724`
- Gap / Note: The implementation writes denied/blocking fields, fails the agent/stage, and generates failed-stage evidence. The current gate did not reach this test because ACP failed earlier.

### REQ-009 Blocked-before-session actual MCP truth

- Proposal Source: `docs/proposals/048-evidence-packs-delivery-preflight-and-mcp-resolution.md:314-316`, `docs/proposals/048-evidence-packs-delivery-preflight-and-mcp-resolution.md:396-405`
- Status: Implemented
- Evidence Type: code, tests-found
- Evidence:
  - `control-plane/crates/engine/src/executor.rs:544`
  - `control-plane/crates/engine/src/executor.rs:546`
  - `control-plane/crates/engine/src/executor.rs:564`
  - `control-plane/crates/engine/tests/integration.rs:1738`
  - `control-plane/crates/engine/tests/integration.rs:1741`
- Gap / Note: R2’s pre-session actual-truth gap is fixed in implementation. The proposal text still does not name `actual_mcp_observation_json`, but the implementation uses it as trust metadata to disambiguate no-session actual truth.

### REQ-010 ACP `session/new.mcpServers` payload serialization

- Proposal Source: `docs/proposals/048-evidence-packs-delivery-preflight-and-mcp-resolution.md:359-394`, `docs/proposals/048-evidence-packs-delivery-preflight-and-mcp-resolution.md:553`
- Status: Partially Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `control-plane/crates/acp/src/lib.rs:96`
  - `control-plane/crates/acp/src/transport.rs:653`
  - `control-plane/crates/acp/tests/integration.rs:495`
  - `./scripts/test-gate.sh proposal-048` failed on `mcp_servers_session_new_serialization_tests`
- Gap / Note: Code serializes `mcpServers`, and the gate now includes the ACP test. The actual test is red with `ACP: initialize handshake` / `ACP handshake read timeout`, so the ACP realization proof is not successful on this tree.

### REQ-011 ACP actual observation on successful session startup

- Proposal Source: `docs/proposals/048-evidence-packs-delivery-preflight-and-mcp-resolution.md:336-342`, `docs/proposals/048-evidence-packs-delivery-preflight-and-mcp-resolution.md:396-405`
- Status: Partially Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `control-plane/crates/acp/src/transport.rs:255`
  - `control-plane/crates/acp/src/lib.rs:119`
  - `control-plane/crates/acp/tests/integration.rs:537`
  - `./scripts/test-gate.sh proposal-048` failed before asserting the observation path
- Gap / Note: The code has a provider-reported or predicted-after-success observation path, but the focused ACP test currently fails before proving it.

### REQ-012 GraphQL stage `executions` MCP truth

- Proposal Source: `docs/proposals/048-evidence-packs-delivery-preflight-and-mcp-resolution.md:530-532`
- Status: Implemented
- Evidence Type: code, tests-found
- Evidence:
  - `control-plane/crates/graphql-server/src/types/stage.rs:28`
  - `control-plane/crates/graphql-server/src/types/stage.rs:75`
  - `control-plane/crates/graphql-server/src/schema.rs:1224`
- Gap / Note: Relation and contract test exist. The current gate did not reach this test because ACP failed earlier.

### REQ-013 MCP reports and `report://` execution truth

- Proposal Source: `docs/proposals/048-evidence-packs-delivery-preflight-and-mcp-resolution.md:535`
- Status: Implemented
- Evidence Type: code, tests-found
- Evidence:
  - `control-plane/crates/mcp-server/src/tools/reports.rs:45`
  - `control-plane/crates/mcp-server/src/tools/reports.rs:72`
  - `control-plane/crates/mcp-server/src/server.rs:454`
  - `control-plane/crates/mcp-server/src/tools/reports.rs:661`
  - `control-plane/crates/mcp-server/src/server.rs:887`
- Gap / Note: Report reader code exists. The current gate did not reach these tests because ACP failed earlier.

### REQ-014 Raw registry command/args/env stays internal

- Proposal Source: `docs/proposals/048-evidence-packs-delivery-preflight-and-mcp-resolution.md:392-394`, `docs/proposals/048-evidence-packs-delivery-preflight-and-mcp-resolution.md:536`
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `control-plane/crates/acp/src/lib.rs:104`
  - `control-plane/crates/graphql-server/src/types/stage.rs:38`
  - `control-plane/crates/mcp-server/src/tools/reports.rs:81`
- Gap / Note: Operator-facing GraphQL/MCP report rows expose IDs/issues/latency/observation, not raw registry executable command payloads.

### REQ-015 Canonical `proposal-048|p048` script gate

- Proposal Source: `docs/proposals/048-evidence-packs-delivery-preflight-and-mcp-resolution.md:540-583`
- Status: Partially Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `scripts/test-gate.sh:1500`
  - `scripts/test-gate.sh:1508`
  - `scripts/test-gate.sh:1516`
  - `./scripts/test-gate.sh proposal-048` failed on the ACP test
- Gap / Note: The gate exists and now includes the right broad proof buckets, but it is not a successful reproducible proof lane yet.

### REQ-016 Gate documentation entry

- Proposal Source: `docs/proposals/048-evidence-packs-delivery-preflight-and-mcp-resolution.md:544`
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `docs/reference/test-gates.md:570`
  - `docs/reference/test-gates.md:576`
  - `docs/reference/test-gates.md:587`
- Gap / Note: The scope text is present. The inspected entry appears to stop after host policy without a visible command block before the P049 heading, which is recorded as a readiness/documentation issue rather than a missing gate entry.

### REQ-017 Full successful same-tree proof for implementation sign-off

- Proposal Source: proposal audit skill output contract and proposal proof-lane requirement
- Status: Partially Implemented
- Evidence Type: tests-run
- Evidence:
  - `./scripts/test-gate.sh proposal-048` failed
- Gap / Note: No full regression was run because the focused proof lane failed first.

## Architecture Review

**Summary:** At Risk

### ARCH-001 ACP serialization implementation has a red contract test

- Severity: Major
- Confidence: High
- Related Proposal Items / Requirements: REQ-010, REQ-011, REQ-015
- Evidence Type: code, tests-run
- Evidence:
  - `control-plane/crates/acp/src/transport.rs:653`
  - `control-plane/crates/acp/tests/integration.rs:495`
  - `./scripts/test-gate.sh proposal-048` failed on `mcp_servers_session_new_serialization_tests`
- Why It Matters: P048’s MCP lane depends on resolved MCP payloads reaching ACP `session/new`. The code path exists, but the test that should prove runtime-ID keys and command/args/env serialization does not complete the initialize handshake. That leaves the critical ACP realization seam unproven in the canonical gate.
- Recommended Action: Debug the ACP fixture or transport handshake until `cargo test -p acp --test integration mcp_servers_session_new_serialization_tests -- --exact --nocapture` passes consistently. Keep it in `proposal-048|p048`.

### ARCH-002 Blocked MCP actual truth is now explicit

- Severity: Note
- Confidence: High
- Related Proposal Items / Requirements: REQ-009
- Evidence Type: code, tests-found
- Evidence:
  - `control-plane/crates/engine/src/executor.rs:544`
  - `control-plane/crates/engine/src/executor.rs:546`
  - `control-plane/crates/engine/tests/integration.rs:1738`
  - `control-plane/crates/engine/tests/integration.rs:1741`
- Why It Matters: This closes the prior R2 ambiguity where blocked MCP executions could persist denied/blocking truth while leaving actual fields `NULL`.
- Recommended Action: Keep the blocked-before-session observation shape stable and consider adding `actual_mcp_observation_json` to the proposal text if the proposal is still being edited.

## Product Review

**Summary:** At Risk

### PROD-001 Operator value is implemented but not handoff-proven

- Severity: Major
- Confidence: Medium
- Related Proposal Items / Requirements: PF-3, PF-4, REQ-010, REQ-015
- Evidence Type: code, tests-run
- Evidence:
  - `control-plane/crates/engine/src/executor.rs:544`
  - `control-plane/crates/mcp-server/src/tools/reports.rs:72`
  - `./scripts/test-gate.sh proposal-048` failed
- Why It Matters: The operator-facing diagnostic model is now mostly present in code, but the product promise is reproducible auditability. A red canonical gate means the team cannot hand off the feature as reliably proven.
- Recommended Action: Treat the ACP serialization test failure as a release blocker for P048 sign-off, even if the failure is fixture-level.

## UI Review

**Summary:** Acceptable

### UI-001 No screen-level UI requirements

- Severity: Note
- Confidence: High
- Related Proposal Items / Requirements: none
- Evidence Type: proposal
- Evidence:
  - `docs/proposals/048-evidence-packs-delivery-preflight-and-mcp-resolution.md:407`
  - `docs/proposals/048-evidence-packs-delivery-preflight-and-mcp-resolution.md:528`
- Why It Matters: P048 is a control-plane/northbound contract proposal. No UI implementation should be blocked by this audit.
- Recommended Action: None for P048. Review any future visual surfaces under a separate UI proposal.

## UX Review

**Summary:** Acceptable with backend risk

### UX-001 Blocked MCP state is now machine-readable, but users still lack a green proof lane

- Severity: Minor
- Confidence: Medium
- Related Proposal Items / Requirements: REQ-009, REQ-015
- Evidence Type: code, tests-run
- Evidence:
  - `control-plane/crates/engine/src/executor.rs:546`
  - `control-plane/crates/engine/tests/integration.rs:1746`
  - `./scripts/test-gate.sh proposal-048` failed
- Why It Matters: The previous ambiguity between unknown actual truth and explicit no-session truth is fixed in storage. The remaining UX risk is operational trust: consumers of these diagnostics cannot rely on a passing canonical proof lane yet.
- Recommended Action: Fix the red gate, then rerun the full P048 gate before claiming the diagnostic UX is ready.

## Delivery / Readiness Review

**Summary:** Not Ready

### READY-001 Canonical P048 gate is red

- Severity: Critical
- Confidence: High
- Related Proposal Items / Requirements: REQ-010, REQ-015, REQ-017
- Evidence Type: tests-run
- Evidence:
  - `./scripts/test-gate.sh proposal-048`
  - Failure: `mcp_servers_session_new_serialization_tests` panicked at `control-plane/crates/acp/tests/integration.rs:537:45`
  - Error: `ACP: initialize handshake`, caused by `ACP handshake read timeout`, `deadline has elapsed`
- Why It Matters: The proposal explicitly makes `proposal-048|p048` the canonical proof path. A failing gate blocks `Implemented`, `Ready`, and `Ready with Risks` verdicts under the audit rules.
- Recommended Action: Fix or stabilize `mcp_servers_session_new_serialization_tests`, rerun `./scripts/test-gate.sh proposal-048`, and only then consider full regression for a successful audit verdict.

### READY-002 Gate docs entry is less complete than the script entry

- Severity: Minor
- Confidence: Medium
- Related Proposal Items / Requirements: REQ-016
- Evidence Type: code
- Evidence:
  - `docs/reference/test-gates.md:570`
  - `docs/reference/test-gates.md:594`
  - `docs/reference/test-gates.md:597`
- Why It Matters: The script entry now contains the expanded proof commands, but the reference-doc entry inspected here does not show the command block before the P049 section begins. That makes the operator-facing gate documentation less self-contained than earlier rounds.
- Recommended Action: Restore a small command block for `./scripts/test-gate.sh proposal-048` under the P048 docs entry after the host policy bullets.

## Readiness Checklist

| Check | Status | Evidence / Note |
|---|---|---|
| Build succeeds on targeted platform(s) | Partial | Crates compiled through several gate steps, but ACP test failed during execution. |
| Core user flow runtime-validated | Partial | Delivery-preflight flow passed; ACP MCP serialization proof failed. |
| Empty/loading/error states covered | Not Applicable | No screen-level UI scope. |
| Accessibility risk acceptable | Not Applicable | No screen-level UI scope. |
| Localization risk acceptable | Not Applicable | Backend/northbound strings only; not audited as product copy. |
| Critical tests executed | Fail | `./scripts/test-gate.sh proposal-048` failed. |
| Full regression suite / canonical full gate passed on same tree/HEAD | Not Checked | Not run because focused P048 gate is red. |
| Privacy/permissions/entitlements reviewed | Not Applicable | No Apple entitlement/sandbox change in P048 scope. |

## Verification Log

- `git rev-parse --show-toplevel && git rev-parse HEAD && git status --short`
- `python3 /Users/user/.agents/skills/proposal-implementation-audit/scripts/report_path.py /Users/user/Documents/Chainworks\ Forge/docs/proposals/048-evidence-packs-delivery-preflight-and-mcp-resolution.md`
- `nl -ba docs/proposals/048-evidence-packs-delivery-preflight-and-mcp-resolution.md | sed -n '299,406p;540,583p'`
- `nl -ba control-plane/crates/engine/src/executor.rs | sed -n '490,675p'`
- `nl -ba scripts/test-gate.sh | sed -n '1499,1515p'`
- `nl -ba docs/reference/test-gates.md | sed -n '570,610p'`
- `rg -n "mcp_resolution|mcp_servers_session_new|blocked_before|actual_mcp|mcp_observation|execution_mcp_truth|stage.*executions|delivery_preflight|failed_stage_evidence" control-plane/crates/engine/tests control-plane/crates/acp/tests control-plane/crates/graphql-server control-plane/crates/mcp-server control-plane/crates/db/tests/integration.rs`
- `./scripts/test-gate.sh proposal-048` failed at `cargo test -p acp --test integration mcp_servers_session_new_serialization_tests -- --exact --nocapture`
- `nl -ba control-plane/crates/acp/tests/integration.rs | sed -n '80,125p;495,545p'`
- `nl -ba control-plane/crates/acp/src/transport.rs | sed -n '250,340p;650,690p'`
- `nl -ba control-plane/crates/engine/tests/integration.rs | sed -n '1653,1778p'`

## Recommended Next Actions

1. Fix `mcp_servers_session_new_serialization_tests` so the ACP initialize handshake completes and the test can prove `session/new.mcpServers` serialization.
2. Rerun `./scripts/test-gate.sh proposal-048` on the same tree.
3. Restore a command block in `docs/reference/test-gates.md` for the P048 entry if that doc is intended as the operator-facing gate reference.
4. If the P048 gate passes and a successful audit verdict is desired, run the repository’s broader regression/full gate on the same tree before claiming `Implemented` or `Ready`.
