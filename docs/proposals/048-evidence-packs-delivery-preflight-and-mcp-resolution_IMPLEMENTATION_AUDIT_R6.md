# Proposal 048 Evidence Packs, Delivery Preflight, and MCP Resolution Multi-Lens Audit R6

| Field | Value |
|---|---|
| Proposal | `docs/proposals/048-evidence-packs-delivery-preflight-and-mcp-resolution.md` |
| Repository Root | `/Users/user/Documents/Chainworks Forge` |
| Git SHA | `af3054c73064b05e42cb816a81a3c5fb0c2e29d9` |
| Working Tree | Dirty; audit reflects current working tree, not clean HEAD |
| Audited At | `2026-04-16T14:56:37+03:00` |
| Platform Scope | macOS-hosted Rust control-plane / northbound API surfaces; Swift/macOS UI app is out of scope for P048 |
| Proposal State | Active |
| Overall Conformance | Implemented |
| Overall Readiness | Ready with Risks |
| Audit Confidence | High |

## Executive Verdict

P048 is implemented for the scoped Rust control-plane slice. The stale embedded gate snippet has been fixed in the proposal, the canonical `proposal-048` gate includes ACP `session/new.mcpServers` serialization and passes, and the broader Rust control-plane workspace gate also passes. Readiness is `Ready with Risks` only because the audit ran on a dirty mixed worktree containing unrelated P029/P049/P047 changes; Swift UI `full` gating is intentionally not required because P048 does not change the Swift app.

## Lens Scorecard

| Lens | Assessment | Top Risk | Confidence |
|---|---|---|---|
| Conformance | Implemented | Dirty worktree makes the exact integration set non-isolated | High |
| Architecture | Acceptable | No remaining P048 architecture blocker found | High |
| Product | Acceptable | Operator diagnostics are implemented and proof-gated | High |
| UI | Not Applicable | P048 has no Swift/UI surface | High |
| UX | Acceptable | Backend diagnostic clarity is covered by tests | High |
| Readiness | Ready with Risks | Current proof is on a dirty multi-proposal worktree | High |

## Proposal Contract

### Scope

P048 covers three Rust control-plane slices: failed-stage evidence packets, delivery preflight for repo-backed run start, and execution-time MCP resolution with durable GraphQL/MCP/northbound reader surfaces.

### Locked Decisions

| Decision | Source |
|---|---|
| Failed-stage evidence is stage-owned and uses the existing artifact/report lane. | `docs/proposals/048-evidence-packs-delivery-preflight-and-mcp-resolution.md:497-507` |
| Delivery preflight runs before run creation/start and blocks failed starts with typed payloads. | `docs/proposals/048-evidence-packs-delivery-preflight-and-mcp-resolution.md:508-516` |
| MCP intent comes from `ResolvedAgent.requested_mcp_server_ids`, not `required_tools`. | `docs/proposals/048-evidence-packs-delivery-preflight-and-mcp-resolution.md:520-522` |
| Missing/disabled/unsupported MCP entries fail closed before ACP startup and persist durable denied/blocking truth. | `docs/proposals/048-evidence-packs-delivery-preflight-and-mcp-resolution.md:523-527` |
| GraphQL and MCP northbound readers expose persisted P048 truth from durable rows. | `docs/proposals/048-evidence-packs-delivery-preflight-and-mcp-resolution.md:528-536` |
| `proposal-048|p048` is the canonical focused proof path for the slice. | `docs/proposals/048-evidence-packs-delivery-preflight-and-mcp-resolution.md:540-567` |

### Primary User Flows

| Flow | User job |
|---|---|
| PF-1 | Start a repo-backed run and get either a typed delivery-preflight block or a created run with persisted preflight truth. |
| PF-2 | Diagnose a failed stage through stage-owned validation, recovery, evidence packet, and report-lane artifacts. |
| PF-3 | Execute an MCP-enabled agent, fail closed before ACP startup when MCP cannot be realized, and inspect requested/predicted/actual/denied/blocking truth. |
| PF-4 | Read the same P048 truth through GraphQL, MCP tools, MCP resources, and reports without reconstructing state from logs. |
| PF-5 | Reproduce the proposal slice through `./scripts/test-gate.sh proposal-048`. |

### UI Commitments

None. P048 is a backend/northbound control-plane proposal. Swift/macOS UI tests are not a P048 acceptance surface.

### UX Commitments

The proposal’s UX commitment is diagnostic clarity through typed blocked payloads, durable failed-stage evidence, recovery snapshots, and northbound MCP truth.

### Acceptance Criteria

The audit treats proposal lines `497-536` as the acceptance surface for failed-stage evidence, delivery preflight, MCP ownership/resolution, and northbound readers.

### Test / Evidence Requirements

The focused proposal proof lane is `./scripts/test-gate.sh proposal-048`. For broader same-tree regression within the affected platform scope, this audit used the Rust control-plane workspace gate `./scripts/test-gate.sh proposal-047`, not the remote-only Swift UI `full` gate.

### Explicit Exclusions

No screen-level UI/simulator evidence is required by P048. Broad run-start MCP warnings and separate MCP readiness summaries remain deferred by proposal lines `436-451`.

## Proposal Fidelity / Divergence

### Matches

- The proposal no longer duplicates a stale long script command list; it names the stable wrapper and requires that the wrapper cover ACP serialization.
- `scripts/test-gate.sh` includes `cargo test -p acp --test integration mcp_servers_session_new_serialization_tests -- --exact --nocapture` in the P048 gate.
- `docs/reference/test-gates.md` documents ACP `session/new.mcpServers` serialization in the P048 scope.
- `./scripts/test-gate.sh proposal-048` passed on the audited tree.
- `./scripts/test-gate.sh proposal-047` passed as broader Rust control-plane workspace regression on the audited tree.

### Divergences

- No P048 implementation divergence found in this audit.
- The working tree includes unrelated active changes across P029, P049, P047 archive/reference movement, and other control-plane modules; this is a readiness risk for final branch integration but not a P048 conformance failure.

### Ambiguities / Evidence Gaps

- No P048 evidence gap remains for the Rust control-plane slice.
- Swift UI `full` gate was intentionally not used because P048 does not change Swift UI behavior and `docs/reference/test-gates.md` marks the P048 host policy as local Rust toolchain only.

## Requirement Summary

| Status | Count |
|---|---:|
| Implemented | 13 |
| Partially Implemented | 0 |
| Missing | 0 |
| Not Verifiable | 0 |

## Requirement Audit

### REQ-001 Persistence fields and DB round-trip

- Proposal Source: `docs/proposals/048-evidence-packs-delivery-preflight-and-mcp-resolution.md:455-485`
- Status: Implemented
- Evidence Type: tests-run
- Evidence:
  - `cargo test -p db --test integration proposal_048_persistence_fields_round_trip -- --exact --nocapture` passed inside `./scripts/test-gate.sh proposal-048`
  - `./scripts/test-gate.sh proposal-047` passed
- Gap / Note: Durable field substrate is covered by focused and broader control-plane regression.

### REQ-002 Delivery-preflight blocked and passing behavior

- Proposal Source: `docs/proposals/048-evidence-packs-delivery-preflight-and-mcp-resolution.md:508-516`
- Status: Implemented
- Evidence Type: tests-run
- Evidence:
  - `cargo test -p engine --test integration delivery_preflight -- --nocapture` passed inside `./scripts/test-gate.sh proposal-048`
  - `./scripts/test-gate.sh proposal-047` passed
- Gap / Note: Blocked-before-run-creation and successful persistence behavior passed.

### REQ-003 Delivery-preflight GraphQL and MCP readback parity

- Proposal Source: `docs/proposals/048-evidence-packs-delivery-preflight-and-mcp-resolution.md:530-535`
- Status: Implemented
- Evidence Type: tests-run
- Evidence:
  - `cargo test -p graphql-server --lib delivery_preflight_graphql_readback_tests -- --nocapture` passed inside `./scripts/test-gate.sh proposal-048`
  - `cargo test -p mcp-server --lib delivery_preflight_mcp_readback_tests -- --nocapture` passed inside `./scripts/test-gate.sh proposal-048`
  - `./scripts/test-gate.sh proposal-047` passed
- Gap / Note: GraphQL, `runs.get`, and `run://{run_id}` delivery-preflight readback are covered.

### REQ-004 ACP `session/new.mcpServers` serialization

- Proposal Source: `docs/proposals/048-evidence-packs-delivery-preflight-and-mcp-resolution.md:359-394`, `docs/proposals/048-evidence-packs-delivery-preflight-and-mcp-resolution.md:548-567`
- Status: Implemented
- Evidence Type: tests-run
- Evidence:
  - `cargo test -p acp --test integration mcp_servers_session_new_serialization_tests -- --exact --nocapture` passed inside `./scripts/test-gate.sh proposal-048`
  - `./scripts/test-gate.sh proposal-047` passed and included the ACP integration tests
- Gap / Note: This closes the ACP proof finding.

### REQ-005 Fail-closed MCP persistence, including explicit actual truth

- Proposal Source: `docs/proposals/048-evidence-packs-delivery-preflight-and-mcp-resolution.md:523-527`
- Status: Implemented
- Evidence Type: tests-run
- Evidence:
  - `cargo test -p engine --test integration mcp_resolution_persistence_tests -- --nocapture` passed inside `./scripts/test-gate.sh proposal-048`
  - `./scripts/test-gate.sh proposal-047` passed
- Gap / Note: The blocked-before-session actual-truth lane is covered.

### REQ-006 Failed-stage evidence packet shape

- Proposal Source: `docs/proposals/048-evidence-packs-delivery-preflight-and-mcp-resolution.md:497-507`
- Status: Implemented
- Evidence Type: tests-run
- Evidence:
  - `cargo test -p engine failed_stage_evidence_packet_tests -- --nocapture` passed inside `./scripts/test-gate.sh proposal-048`
  - `./scripts/test-gate.sh proposal-047` passed
- Gap / Note: Packet V1 shape and recovery mirroring are covered.

### REQ-007 Failed-stage evidence report-lane readback

- Proposal Source: `docs/proposals/048-evidence-packs-delivery-preflight-and-mcp-resolution.md:504`
- Status: Implemented
- Evidence Type: tests-run
- Evidence:
  - `cargo test -p mcp-server --lib reports_failed_stage_evidence_contract_tests -- --nocapture` passed inside `./scripts/test-gate.sh proposal-048`
  - `cargo test -p mcp-server --lib report_resource_decodes_failed_stage_evidence_payload -- --nocapture` passed inside `./scripts/test-gate.sh proposal-048`
  - `./scripts/test-gate.sh proposal-047` passed
- Gap / Note: Both report tool and report resource evidence paths are covered.

### REQ-008 Typed GraphQL blocked-start payload

- Proposal Source: `docs/proposals/048-evidence-packs-delivery-preflight-and-mcp-resolution.md:511-513`
- Status: Implemented
- Evidence Type: tests-run
- Evidence:
  - `cargo test -p graphql-server --lib start_run_blocked_preflight_returns_typed_payload -- --nocapture` passed inside `./scripts/test-gate.sh proposal-048`
  - `./scripts/test-gate.sh proposal-047` passed
- Gap / Note: The GraphQL blocked-start transport contract is covered.

### REQ-009 GraphQL stage `executions` MCP truth

- Proposal Source: `docs/proposals/048-evidence-packs-delivery-preflight-and-mcp-resolution.md:530-532`
- Status: Implemented
- Evidence Type: tests-run
- Evidence:
  - `cargo test -p graphql-server --lib execution_mcp_truth_contract_tests -- --nocapture` passed inside `./scripts/test-gate.sh proposal-048`
  - `./scripts/test-gate.sh proposal-047` passed
- Gap / Note: The stage-to-execution MCP truth surface is covered.

### REQ-010 MCP report and resource execution truth

- Proposal Source: `docs/proposals/048-evidence-packs-delivery-preflight-and-mcp-resolution.md:535`
- Status: Implemented
- Evidence Type: tests-run
- Evidence:
  - `cargo test -p mcp-server --lib reports_mcp_resolution_truth_tests -- --nocapture` passed inside `./scripts/test-gate.sh proposal-048`
  - `cargo test -p mcp-server --lib report_resource_exposes_mcp_execution_truth -- --nocapture` passed inside `./scripts/test-gate.sh proposal-048`
  - `./scripts/test-gate.sh proposal-047` passed
- Gap / Note: MCP northbound execution truth readback is covered.

### REQ-011 Canonical `proposal-048|p048` focused gate

- Proposal Source: `docs/proposals/048-evidence-packs-delivery-preflight-and-mcp-resolution.md:540-567`
- Status: Implemented
- Evidence Type: tests-run
- Evidence:
  - `./scripts/test-gate.sh proposal-048` passed
- Gap / Note: The focused proposal gate is green and includes ACP serialization coverage.

### REQ-012 Gate reference documentation

- Proposal Source: `docs/proposals/048-evidence-packs-delivery-preflight-and-mcp-resolution.md:540-567`
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `docs/reference/test-gates.md:589-615`
- Gap / Note: The reference entry documents the local Rust-toolchain host policy and ACP serialization scope.

### REQ-013 No screen-level UI scope

- Proposal Source: `docs/proposals/048-evidence-packs-delivery-preflight-and-mcp-resolution.md:407-424`
- Status: Implemented
- Evidence Type: proposal
- Evidence:
  - P048 defines GraphQL/MCP/report surfaces, not visual screens.
- Gap / Note: Swift UI test execution is not a P048 acceptance criterion.

## Architecture Review

**Summary:** Acceptable

### ARCH-001 ACP proof is present and passing

- Severity: Note
- Confidence: High
- Related Proposal Items / Requirements: REQ-004, REQ-011
- Evidence Type: tests-run
- Evidence:
  - `cargo test -p acp --test integration mcp_servers_session_new_serialization_tests -- --exact --nocapture` passed inside `./scripts/test-gate.sh proposal-048`
  - `./scripts/test-gate.sh proposal-047` passed
- Why It Matters: P048’s MCP lane depends on executable MCP payloads reaching ACP `session/new`; this is now directly tested in the canonical P048 gate and broader control-plane workspace gate.
- Recommended Action: No P048 action required.

## Product Review

**Summary:** Acceptable

### PROD-001 Operator diagnostics are reproducibly proven for the scoped control-plane slice

- Severity: Note
- Confidence: High
- Related Proposal Items / Requirements: PF-2, PF-3, PF-4
- Evidence Type: tests-run
- Evidence:
  - `./scripts/test-gate.sh proposal-048` passed
  - `./scripts/test-gate.sh proposal-047` passed
- Why It Matters: The user-facing product value of P048 is trustworthy northbound diagnosis, not Swift UI behavior. The relevant backend/northbound flows are covered.
- Recommended Action: No P048 action required. Keep release sign-off separate from proposal-specific readiness.

## UI Review

**Summary:** Not Applicable

### UI-001 Swift/macOS UI is out of P048 scope

- Severity: Note
- Confidence: High
- Related Proposal Items / Requirements: REQ-013
- Evidence Type: proposal
- Evidence:
  - `docs/proposals/048-evidence-packs-delivery-preflight-and-mcp-resolution.md:407-424`
  - `docs/reference/test-gates.md:613-615`
- Why It Matters: Requiring remote-only UI tests for P048 would add an irrelevant gate to a Rust control-plane proposal.
- Recommended Action: Do not block P048 implementation readiness on Swift UI tests unless a separate UI-facing proposal changes those surfaces.

## UX Review

**Summary:** Acceptable

### UX-001 Diagnostic ambiguity around blocked MCP state is resolved

- Severity: Note
- Confidence: High
- Related Proposal Items / Requirements: REQ-005, REQ-010
- Evidence Type: tests-run
- Evidence:
  - `cargo test -p engine --test integration mcp_resolution_persistence_tests -- --nocapture` passed inside `./scripts/test-gate.sh proposal-048`
  - `cargo test -p mcp-server --lib reports_mcp_resolution_truth_tests -- --nocapture` passed inside `./scripts/test-gate.sh proposal-048`
  - `cargo test -p mcp-server --lib report_resource_exposes_mcp_execution_truth -- --nocapture` passed inside `./scripts/test-gate.sh proposal-048`
- Why It Matters: Operators can distinguish blocked-before-session actual truth from missing or legacy data through persisted execution truth and report/resource readback.
- Recommended Action: No P048 action required.

## Delivery / Readiness Review

**Summary:** Ready with Risks

### READY-001 Dirty mixed worktree remains the only readiness caveat

- Severity: Minor
- Confidence: High
- Related Proposal Items / Requirements: all
- Evidence Type: tests-run, code
- Evidence:
  - `git status --short` showed a dirty tree with P048 plus unrelated P029/P049/P047 and other changes.
  - `./scripts/test-gate.sh proposal-048` passed.
  - `./scripts/test-gate.sh proposal-047` passed.
- Why It Matters: The P048 implementation is proven in the current tree, but final merge/release should still isolate or intentionally integrate the unrelated concurrent proposal changes.
- Recommended Action: Before merge, review branch composition and avoid attributing unrelated P029/P049/P047 changes to P048.

## Readiness Checklist

| Check | Status | Evidence / Note |
|---|---|---|
| Build succeeds on targeted platform(s) | Pass | Rust control-plane crates built during `proposal-048` and `proposal-047` gates. |
| Core user flow runtime-validated | Pass | `./scripts/test-gate.sh proposal-048` passed. |
| Empty/loading/error states covered | Not Applicable | No screen-level UI scope. |
| Accessibility risk acceptable | Not Applicable | No screen-level UI scope. |
| Localization risk acceptable | Not Applicable | Backend/northbound contract proposal. |
| Critical tests executed | Pass | `proposal-048` focused gate passed. |
| Full regression suite / canonical full gate passed on same tree/HEAD | Pass for scoped Rust control-plane | `./scripts/test-gate.sh proposal-047` passed. Remote-only Swift UI `full` gate is out of P048 scope. |
| Privacy/permissions/entitlements reviewed | Not Applicable | No Apple entitlement/sandbox change in P048 scope. |

## Verification Log

- `git rev-parse --show-toplevel && git rev-parse HEAD && git status --short`
- `python3 /Users/user/.agents/skills/proposal-implementation-audit/scripts/report_path.py /Users/user/Documents/Chainworks\ Forge/docs/proposals/048-evidence-packs-delivery-preflight-and-mcp-resolution.md`
- `date -Iseconds`
- `nl -ba docs/proposals/048-evidence-packs-delivery-preflight-and-mcp-resolution.md | sed -n '548,583p'`
- `nl -ba scripts/test-gate.sh | sed -n '1515,1536p'`
- `nl -ba docs/reference/test-gates.md | sed -n '589,620p'`
- `./scripts/test-gate.sh proposal-048`
- `./scripts/test-gate.sh proposal-047`

## Recommended Next Actions

1. Treat P048 as implemented for the Rust control-plane scope.
2. Before merge, isolate or explicitly account for unrelated dirty-worktree changes from P029, P049, P047 archival/reference movement, and other modules.
3. Do not require Swift UI remote-only `full` gate for P048 unless a separate UI-facing change is added.
