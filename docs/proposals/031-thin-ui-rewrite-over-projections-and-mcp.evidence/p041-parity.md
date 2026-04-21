# P041 Parity Evidence for P031

| Field | Value |
|---|---|
| Status | Ready |
| Gate | `./scripts/test-gate.sh proposal-041` |
| Alias | `./scripts/test-gate.sh p041` |
| Relevant regression boundary | `cd control-plane && cargo test --workspace` |
| Scope | P041 Rust control-plane parity evidence consumed by P031 |

## Fixture verdicts

| Fixture | Behavioral diff | Server replay | Verdict | Blocking divergences | Waiver |
|---|---|---|---|---:|---|
| `proposal-loop-basic` | `control-plane/target/parity/reports/proposal-loop-basic/behavioral-diff-report.json` | `control-plane/target/parity/proposal-loop-basic/server-replay.json` | ready | 0 | none |
| `implementation-refine-review` | `control-plane/target/parity/reports/implementation-refine-review/behavioral-diff-report.json` | `control-plane/target/parity/implementation-refine-review/server-replay.json` | ready | 0 | none |
| `approval-pause-resume` | `control-plane/target/parity/reports/approval-pause-resume/behavioral-diff-report.json` | `control-plane/target/parity/approval-pause-resume/server-replay.json` | ready | 0 | none |
| `retry-recovery-flow` | `control-plane/target/parity/reports/retry-recovery-flow/behavioral-diff-report.json` | `control-plane/target/parity/retry-recovery-flow/server-replay.json` | ready | 0 | none |
| `cancelled-or-blocked-run` | `control-plane/target/parity/reports/cancelled-or-blocked-run/behavioral-diff-report.json` | `control-plane/target/parity/cancelled-or-blocked-run/server-replay.json` | ready | 0 | none |
| `terminal-report-evidence` | `control-plane/target/parity/reports/terminal-report-evidence/behavioral-diff-report.json` | `control-plane/target/parity/terminal-report-evidence/server-replay.json` | ready | 0 | none |
| `projection-readback-surface` | `control-plane/target/parity/reports/projection-readback-surface/behavioral-diff-report.json` | `control-plane/target/parity/projection-readback-surface/server-replay.json` | ready | 0 | none |

## Cutover interpretation

P031 may consume this artifact only when `./scripts/test-gate.sh proposal-041` passes on the same tree. The gate validates every required P041 fixture, generated behavioral diff report, server replay snapshot, retained fixture DB reference, GraphQL readback collector, MCP readback collector, and live-shadow side-effect contract.

The P041 evidence currently has no waivers and no blocking divergences. If any fixture becomes `red`, P031 cutover is blocked. If a future fixture is marked `ready_with_risks`, this document must name the owner, date, affected surface, hold/rollback criteria, and why P031 can proceed safely.

## Evidence owners

| Evidence | Owner |
|---|---|
| Fixture schema and replay | `control-plane/crates/engine/tests/proposal_041_parity.rs` |
| GraphQL readback | `control-plane/crates/graphql-server/src/schema.rs` |
| MCP report readback | `control-plane/crates/mcp-server/src/server.rs` and `control-plane/crates/mcp-server/src/tools/reports.rs` |
| Gate orchestration | `scripts/test-gate.sh` |
| Gate reference | `docs/reference/test-gates.md` |

## P031 hold rule

P031 must hold thin-client cutover when any of these conditions are true:

- `./scripts/test-gate.sh proposal-041` fails,
- a required fixture report is missing,
- any fixture has blocking divergences,
- GraphQL or MCP readback actuals are not collected by their owning northbound surface,
- live-shadow evidence lacks source/shadow/idempotency correlation,
- this handoff artifact is stale relative to the implementation tree.
