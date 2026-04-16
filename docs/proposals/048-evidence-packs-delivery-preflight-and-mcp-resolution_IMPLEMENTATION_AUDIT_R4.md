# Proposal 048 Evidence Packs, Delivery Preflight, and MCP Resolution Multi-Lens Audit R4

| Field | Value |
|---|---|
| Proposal | `docs/proposals/048-evidence-packs-delivery-preflight-and-mcp-resolution.md` |
| Repository Root | `/Users/user/Documents/Chainworks Forge` |
| Git SHA | `af3054c73064b05e42cb816a81a3c5fb0c2e29d9` |
| Working Tree | Dirty; audit reflects current working tree, not clean HEAD |
| Audited At | `2026-04-16T12:03:58+03:00` |
| Platform Scope | macOS-hosted Rust control-plane / northbound API surfaces; no screen-level UI scope |
| Proposal State | Active |
| Overall Conformance | Partial |
| Overall Readiness | Not Ready |
| Audit Confidence | High |

## Executive Verdict

The current implementation now passes the expanded `./scripts/test-gate.sh proposal-048` gate, including ACP `session/new.mcpServers` serialization, engine fail-closed MCP persistence, delivery-preflight readback, failed-stage evidence readback, GraphQL stage execution truth, and MCP report/resource truth. The audit still cannot report `Implemented` or `Ready` because the repository `full` gate is remote-only on this host and exited before running, so same-tree full-regression evidence is unavailable.

## Lens Scorecard

| Lens | Assessment | Top Risk | Confidence |
|---|---|---|---|
| Conformance | Focused P048 implemented; audit roll-up partial | Full regression unavailable on this host | High |
| Architecture | Acceptable | No remaining P048 architecture blocker found in the focused proof lane | High |
| Product | Acceptable with release caveat | P048 operator diagnostics are proven by focused gate but not by full sign-off | Medium |
| UI | Acceptable | No UI requirements in this proposal | High |
| UX | Acceptable | Diagnostic ambiguity around blocked MCP actual truth is resolved in the focused gate | High |
| Readiness | Not Ready | Successful audit verdict blocked by unavailable `full` gate | High |

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
| GraphQL and MCP northbound readers must expose persisted P048 truth from durable rows. | `docs/proposals/048-evidence-packs-delivery-preflight-and-mcp-resolution.md:528-536` |
| `proposal-048|p048` is the canonical focused proof path for the slice. | `docs/proposals/048-evidence-packs-delivery-preflight-and-mcp-resolution.md:540-583` |

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

The focused proposal proof lane is `./scripts/test-gate.sh proposal-048`. The audit skill additionally requires same-tree full regression for any successful `Implemented`, `Ready`, or `Ready with Risks` verdict.

### Explicit Exclusions

No screen-level UI/simulator evidence is required by P048. Broad run-start MCP warnings and separate MCP readiness summaries remain deferred by proposal lines `436-451`.

## Proposal Fidelity / Divergence

### Matches

- The expanded P048 gate passed on this tree.
- ACP `mcpServers` serialization proof is now included in the gate and passed.
- Engine fail-closed MCP persistence proof is included in the gate and passed.
- Delivery-preflight blocked/passing behavior and GraphQL/MCP readback proofs passed.
- Failed-stage evidence packet and `reports.get` / `report://` readback proofs passed.
- GraphQL blocked-start payload and stage `executions` MCP truth proofs passed.
- MCP `reports.get` and `report://` execution truth proofs passed.

### Divergences

- The proposal’s illustrative script snippet still lists only engine, GraphQL, and MCP-server commands, while the implementation script has expanded beyond the snippet to include the ACP transport test. This is favorable implementation behavior, but the proposal text may remain stale if it is intended as exact copy/paste guidance.
- The report cannot roll up to a successful audit verdict because `./scripts/test-gate.sh full` is unavailable on this host.

### Ambiguities / Evidence Gaps

- Same-tree full regression evidence is unavailable: `./scripts/test-gate.sh full` exited with host-policy error because UI tests are remote-only and this host is not in the approved list.
- The working tree is dirty and includes unrelated P029/P049/P047 changes; this audit is therefore a current-worktree audit, not a clean-branch sign-off.

## Requirement Summary

| Status | Count |
|---|---:|
| Implemented | 12 |
| Partially Implemented | 1 |
| Missing | 0 |
| Not Verifiable | 0 |

## Requirement Audit

### REQ-001 Persistence fields and DB round-trip

- Proposal Source: `docs/proposals/048-evidence-packs-delivery-preflight-and-mcp-resolution.md:455-485`
- Status: Implemented
- Evidence Type: tests-run
- Evidence:
  - `cargo test -p db --test integration proposal_048_persistence_fields_round_trip -- --exact --nocapture` passed inside `./scripts/test-gate.sh proposal-048`
- Gap / Note: Durable field substrate is covered by the focused gate.

### REQ-002 Delivery-preflight blocked and passing behavior

- Proposal Source: `docs/proposals/048-evidence-packs-delivery-preflight-and-mcp-resolution.md:508-516`
- Status: Implemented
- Evidence Type: tests-run
- Evidence:
  - `cargo test -p engine --test integration delivery_preflight -- --nocapture` passed inside `./scripts/test-gate.sh proposal-048`
- Gap / Note: Both blocked-before-run-creation and successful persistence behavior passed.

### REQ-003 Delivery-preflight GraphQL and MCP readback parity

- Proposal Source: `docs/proposals/048-evidence-packs-delivery-preflight-and-mcp-resolution.md:530-535`
- Status: Implemented
- Evidence Type: tests-run
- Evidence:
  - `cargo test -p graphql-server --lib delivery_preflight_graphql_readback_tests -- --nocapture` passed inside `./scripts/test-gate.sh proposal-048`
  - `cargo test -p mcp-server --lib delivery_preflight_mcp_readback_tests -- --nocapture` passed inside `./scripts/test-gate.sh proposal-048`
- Gap / Note: The readback parity promised by the focused gate is now proven.

### REQ-004 ACP `session/new.mcpServers` serialization

- Proposal Source: `docs/proposals/048-evidence-packs-delivery-preflight-and-mcp-resolution.md:359-394`, `docs/proposals/048-evidence-packs-delivery-preflight-and-mcp-resolution.md:548-554`
- Status: Implemented
- Evidence Type: tests-run
- Evidence:
  - `cargo test -p acp --test integration mcp_servers_session_new_serialization_tests -- --exact --nocapture` passed inside `./scripts/test-gate.sh proposal-048`
- Gap / Note: This closes the R3 blocker and the review finding about missing ACP proof in the implemented gate.

### REQ-005 Fail-closed MCP persistence, including explicit actual truth

- Proposal Source: `docs/proposals/048-evidence-packs-delivery-preflight-and-mcp-resolution.md:523-527`
- Status: Implemented
- Evidence Type: tests-run
- Evidence:
  - `cargo test -p engine --test integration mcp_resolution_persistence_tests -- --nocapture` passed inside `./scripts/test-gate.sh proposal-048`
- Gap / Note: The previously ambiguous blocked-before-session actual lane is now covered by the focused gate.

### REQ-006 Failed-stage evidence packet shape

- Proposal Source: `docs/proposals/048-evidence-packs-delivery-preflight-and-mcp-resolution.md:497-507`
- Status: Implemented
- Evidence Type: tests-run
- Evidence:
  - `cargo test -p engine failed_stage_evidence_packet_tests -- --nocapture` passed inside `./scripts/test-gate.sh proposal-048`
- Gap / Note: Packet V1 shape and recovery mirroring are covered.

### REQ-007 Failed-stage evidence report-lane readback

- Proposal Source: `docs/proposals/048-evidence-packs-delivery-preflight-and-mcp-resolution.md:504`
- Status: Implemented
- Evidence Type: tests-run
- Evidence:
  - `cargo test -p mcp-server --lib reports_failed_stage_evidence_contract_tests -- --nocapture` passed inside `./scripts/test-gate.sh proposal-048`
  - `cargo test -p mcp-server --lib report_resource_decodes_failed_stage_evidence_payload -- --nocapture` passed inside `./scripts/test-gate.sh proposal-048`
- Gap / Note: Both report tool and report resource evidence paths are proven.

### REQ-008 Typed GraphQL blocked-start payload

- Proposal Source: `docs/proposals/048-evidence-packs-delivery-preflight-and-mcp-resolution.md:511-513`
- Status: Implemented
- Evidence Type: tests-run
- Evidence:
  - `cargo test -p graphql-server --lib start_run_blocked_preflight_returns_typed_payload -- --nocapture` passed inside `./scripts/test-gate.sh proposal-048`
- Gap / Note: The GraphQL blocked-start transport contract is proven.

### REQ-009 GraphQL stage `executions` MCP truth

- Proposal Source: `docs/proposals/048-evidence-packs-delivery-preflight-and-mcp-resolution.md:530-532`
- Status: Implemented
- Evidence Type: tests-run
- Evidence:
  - `cargo test -p graphql-server --lib execution_mcp_truth_contract_tests -- --nocapture` passed inside `./scripts/test-gate.sh proposal-048`
- Gap / Note: The stage-to-execution MCP truth surface is proven.

### REQ-010 MCP report and resource execution truth

- Proposal Source: `docs/proposals/048-evidence-packs-delivery-preflight-and-mcp-resolution.md:535`
- Status: Implemented
- Evidence Type: tests-run
- Evidence:
  - `cargo test -p mcp-server --lib reports_mcp_resolution_truth_tests -- --nocapture` passed inside `./scripts/test-gate.sh proposal-048`
  - `cargo test -p mcp-server --lib report_resource_exposes_mcp_execution_truth -- --nocapture` passed inside `./scripts/test-gate.sh proposal-048`
- Gap / Note: MCP northbound execution truth readback is proven.

### REQ-011 Canonical `proposal-048|p048` focused gate

- Proposal Source: `docs/proposals/048-evidence-packs-delivery-preflight-and-mcp-resolution.md:540-583`
- Status: Implemented
- Evidence Type: tests-run
- Evidence:
  - `./scripts/test-gate.sh proposal-048` passed
- Gap / Note: The focused proposal gate is now green.

### REQ-012 No screen-level UI scope

- Proposal Source: `docs/proposals/048-evidence-packs-delivery-preflight-and-mcp-resolution.md:407-424`
- Status: Implemented
- Evidence Type: proposal
- Evidence:
  - P048 defines GraphQL/MCP/report surfaces, not visual screens.
- Gap / Note: No UI verification is required for P048.

### REQ-013 Same-tree full regression evidence for successful audit verdict

- Proposal Source: audit skill roll-up rule
- Status: Partially Implemented
- Evidence Type: tests-run
- Evidence:
  - `./scripts/test-gate.sh proposal-048` passed
  - `./scripts/test-gate.sh full` exited with `error: UI tests are remote-only and may not run on this host.`
  - Approved remote hosts: `smacbook.local,smacbook`; observed host names: `0000659.localdomain,0000659`
- Gap / Note: This is not a P048 implementation gap, but it blocks a successful audit roll-up under the audit rules.

## Architecture Review

**Summary:** Acceptable

### ARCH-001 ACP proof blocker is resolved in the focused gate

- Severity: Note
- Confidence: High
- Related Proposal Items / Requirements: REQ-004, REQ-011
- Evidence Type: tests-run
- Evidence:
  - `cargo test -p acp --test integration mcp_servers_session_new_serialization_tests -- --exact --nocapture` passed inside `./scripts/test-gate.sh proposal-048`
- Why It Matters: P048’s MCP lane depends on executable MCP payloads reaching ACP `session/new`; the previously red proof is now green.
- Recommended Action: Keep this ACP test in the P048 gate and update the proposal snippet if exact textual parity is still expected.

## Product Review

**Summary:** Acceptable with release caveat

### PROD-001 P048 operator diagnostics are focused-gate proven but not full-regression signed off

- Severity: Minor
- Confidence: Medium
- Related Proposal Items / Requirements: PF-2, PF-3, PF-4, REQ-013
- Evidence Type: tests-run
- Evidence:
  - `./scripts/test-gate.sh proposal-048` passed
  - `./scripts/test-gate.sh full` did not run on this host
- Why It Matters: The operator-facing value is now covered by the proposal-specific gate, but release handoff still needs an approved host or CI path for full sign-off.
- Recommended Action: Run the full gate on an approved remote host or in CI before treating the branch as globally ready.

## UI Review

**Summary:** Acceptable

### UI-001 No screen-level UI requirements

- Severity: Note
- Confidence: High
- Related Proposal Items / Requirements: none
- Evidence Type: proposal
- Evidence:
  - `docs/proposals/048-evidence-packs-delivery-preflight-and-mcp-resolution.md:407-424`
- Why It Matters: P048 is a control-plane/northbound contract proposal. No UI implementation should be blocked by this audit.
- Recommended Action: None for P048.

## UX Review

**Summary:** Acceptable

### UX-001 Diagnostic ambiguity around blocked MCP state is resolved in focused proof

- Severity: Note
- Confidence: High
- Related Proposal Items / Requirements: REQ-005, REQ-010
- Evidence Type: tests-run
- Evidence:
  - `cargo test -p engine --test integration mcp_resolution_persistence_tests -- --nocapture` passed inside `./scripts/test-gate.sh proposal-048`
  - `cargo test -p mcp-server --lib reports_mcp_resolution_truth_tests -- --nocapture` passed inside `./scripts/test-gate.sh proposal-048`
  - `cargo test -p mcp-server --lib report_resource_exposes_mcp_execution_truth -- --nocapture` passed inside `./scripts/test-gate.sh proposal-048`
- Why It Matters: Operators no longer need to infer blocked-before-session actual truth from absent fields or logs in the focused P048 flow.
- Recommended Action: Preserve this behavior and keep it in the gate.

## Delivery / Readiness Review

**Summary:** Not Ready

### READY-001 Full gate is unavailable on this host

- Severity: Critical
- Confidence: High
- Related Proposal Items / Requirements: REQ-013
- Evidence Type: tests-run
- Evidence:
  - `./scripts/test-gate.sh full`
  - Error: `UI tests are remote-only and may not run on this host.`
  - Approved remote hosts: `smacbook.local,smacbook`; observed host names: `0000659.localdomain,0000659`
- Why It Matters: The audit skill requires same-tree full regression evidence before a successful `Implemented`, `Ready`, or `Ready with Risks` verdict. The focused P048 gate is green, but full sign-off cannot be collected from this host.
- Recommended Action: Run `./scripts/test-gate.sh full` on an approved remote host or CI. If it passes on the same tree/worktree contents, rerun this audit for a successful roll-up.

## Readiness Checklist

| Check | Status | Evidence / Note |
|---|---|---|
| Build succeeds on targeted platform(s) | Partial | Rust crates in P048 focused gate built and tested; full Xcode/UI gate did not run on this host. |
| Core user flow runtime-validated | Pass for P048 focused scope | `./scripts/test-gate.sh proposal-048` passed. |
| Empty/loading/error states covered | Not Applicable | No screen-level UI scope. |
| Accessibility risk acceptable | Not Applicable | No screen-level UI scope. |
| Localization risk acceptable | Not Applicable | Backend/northbound contract proposal. |
| Critical tests executed | Pass for focused P048, fail for full sign-off availability | Focused P048 gate passed; full gate host-policy blocked. |
| Full regression suite / canonical full gate passed on same tree/HEAD | Fail / unavailable | `./scripts/test-gate.sh full` exited before running due remote-only UI host policy. |
| Privacy/permissions/entitlements reviewed | Not Applicable | No Apple entitlement/sandbox change in P048 scope. |

## Verification Log

- `git rev-parse --show-toplevel && git rev-parse HEAD && git status --short`
- `python3 /Users/user/.agents/skills/proposal-implementation-audit/scripts/report_path.py /Users/user/Documents/Chainworks\ Forge/docs/proposals/048-evidence-packs-delivery-preflight-and-mcp-resolution.md`
- `date -Iseconds`
- `find docs/proposals -maxdepth 1 -name '048-evidence-packs-delivery-preflight-and-mcp-resolution_IMPLEMENTATION_AUDIT_R*.md' -print | sort`
- `./scripts/test-gate.sh proposal-048`
- `./scripts/test-gate.sh`
- `./scripts/test-gate.sh full`

## Recommended Next Actions

1. Run `./scripts/test-gate.sh full` on `smacbook.local`, `smacbook`, or the approved CI/remote host for full sign-off.
2. If full gate passes on the same tree contents, rerun the P048 implementation audit and mark the conformance/readiness roll-up accordingly.
3. If proposal text is still being maintained as exact implementation guidance, update the P048 gate snippet to include the ACP transport test already present in `scripts/test-gate.sh`.
