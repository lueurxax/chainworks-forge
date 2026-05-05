# P041 Implementation Audit R1

Audit target: current worktree at `bafc04deed5cd199392908fb162c8851dd7b5e52`  
Proposal: `docs/proposals/041-server-parity-harness-golden-runs-and-behavioral-diff.md`  
Proposal revision: `P041-draft-2026-04-30-r11`  
Audit date: 2026-05-05

## Verdict

Overall proposal conformance: **Partially Implemented**.

Overall implementation readiness: **Not Ready for closeout**.

The core parity harness is real and the canonical P041 gate passes on the current clean tree. It validates all seven fixtures, all six surfaces, GraphQL/MCP owner readback, live-shadow evidence, runtime row/detail publication, and same-tree provenance. The closeout blockers are not in the happy-path fixture replay itself. They are in the proposal's operational and handoff guarantees:

1. The P041 gate default deadline is 7200 seconds and no per-fixture replay/readback/live-shadow deadlines enforce the proposal's 25 minute / 60 second / 30 second / 60 second limits.
2. The P031 handoff family is not internally ready: `scripts/p031-thin-ui-gate.py --repo-root .` fails because the tracked Phase 0 manifest still marks `p041_parity_evidence` as `not_ready_pending_gate_run`.
3. Interruption handling is incomplete: the gate has timeout handling, but no P041 SIGINT/SIGTERM path that publishes `blocked_interrupted`.
4. The Python supervisor's canonical control-file writes use plain `fsync` instead of preferring Darwin `F_FULLFSYNC`, even though other P041 writers do use the stronger contract.

## Reviewer Reuse

Reuse state: **Partially reused**.

Prior proposal-review artifacts were found under `.chainworks/runs/4459e17c-c5f0-48ee-9821-97ee937ec3dd/reviews/proposal/`. The selected proposal panel was UI, Apple architecture, API contract, reliability, and macOS. The implementation audit reused the same risk lenses, with API contract, reliability, and macOS carrying most of the runtime evidence review; Apple architecture covered the future-client boundary; UI was kept for the CLI/status contract. No new delta reviewer was added because the prior five-lane set already hit the hard cap and covered the implementation's primary surfaces.

## Requirement Conformance

| ID | Proposal commitment | Status | Evidence |
|---|---|---|---|
| REQ-001 | Preserve seven checked-in golden fixtures with fixture metadata, frozen inputs, capture records, and regeneration reports. | Implemented | `control-plane/crates/engine/src/parity_control.rs` defines the canonical seven fixtures; `proposal_041_fixture_inventory_and_schema_contract` passed inside `./scripts/test-gate.sh proposal-041`. |
| REQ-002 | Preserve six comparison surfaces per fixture: canonical state, projections, GraphQL readback, MCP report readback, artifact identity, operator summary. | Implemented | `REQUIRED_COMPARISON_SURFACES` lists all six; the P041 gate passed all 42 fixture/surface cells. |
| REQ-003 | Runtime row/detail artifacts must be versioned, agree on schema/status/state/generation, and carry provenance. | Implemented | Runtime row/detail at `control-plane/target/parity/publication/current/` published `ready_same_tree_verified` with matching generation `p041-2026-05-05T05:58:49Z-0cb49e28`. |
| REQ-004 | Ready publication must require clean tree, zero status lines, all fixtures/surfaces passing, and live checkout match. | Implemented | P041 gate captured commit `bafc04deed5cd199392908fb162c8851dd7b5e52`, tree `ee28dc02c8acdb46b7db1fcd4fed0b8eb6196402`, clean status line count `0`, and passed. |
| REQ-005 | CLI status projection, summary grid/fallback, and JSON-only footer must be deterministic. | Implemented | Gate output ended with all seven vertical fallback rows, `status=ready_same_tree_verified`, `JSON-only evidence`, row path, and detail path. |
| REQ-006 | Canonical `parity-control` metadata writes must use same-dir temp, durable flush, atomic rename, and Darwin `F_FULLFSYNC` preference. | Partially Implemented | Rust and later Python writers use this contract, but the supervisor helper at `scripts/test-gate.sh:626` writes `lease.json`, `current-step.json`, and `timeout-marker.json` with plain `os.fsync(handle.fileno())`. |
| REQ-007 | Full gate must fail above 25 minutes; replay, readback, live-shadow, and drain must have the proposal's bounded deadlines. | Missing | `scripts/test-gate.sh:2595` defaults `P041_GATE_DEADLINE_SECONDS` to `7200`; `p041_supervised_run` enforces only that global deadline. No 60s replay, 30s readback, or 60s live-shadow deadline was found. |
| REQ-008 | Timeout/SIGINT/SIGTERM lifecycle must publish blocked evidence and prove descendant absence before release/cleanup. | Partially Implemented | Timeout/process-group handling exists, but P041 has no SIGINT/SIGTERM handler that writes `interruption-marker.json`; `KeyboardInterrupt` in the supervisor is folded into timeout handling. |
| REQ-009 | Successor reclaim matrix A/A1/A2/B/C/D and stale-owner diagnostics must fail closed. | Implemented | Gate code and tests cover Case A/A2/B/C/D, lease heartbeat/control sequence, reclaim markers, and `blocked_manual_recovery` preservation. |
| REQ-010 | Retention/pruning must preserve manual-recovery generations, keep newest ready and blocked diagnostics, warn above 500 MB, and remove SQLite triples together. | Implemented | Gate retention block preserves manual recovery, prunes eligible older generations, reports storage budget, and removes `.sqlite`, `.sqlite-wal`, `.sqlite-shm` together. |
| REQ-011 | Phase C P031 handoff artifacts must agree across scripts, reference docs, manifest, schema record, and runtime/reference boundary. | Partially Implemented | Runtime row/detail and docs exist, but `python3 scripts/p031-thin-ui-gate.py --repo-root .` fails because `docs/reference/p031-phase-0-artifact-manifest.json:93` marks `p041_parity_evidence` as `not_ready_pending_gate_run`. |
| REQ-012 | No SwiftUI/AppKit surface, app-owned persistence, or direct filesystem readiness authority ships in P041. | Implemented | No new UI surface was found; P031 guard validates GraphQL-owned future boundary and runtime row/detail live-checkout comparison. |

## Routed Findings

### READY-P041-001 - P031 Phase 0 manifest still blocks P041 evidence

Severity: P1  
Reviewer lenses: API contract, Apple architecture, readiness/rollout

`docs/reference/p031-phase-0-artifact-manifest.json:90-96` adds `p041_parity_evidence`, but its `validation_status` remains `not_ready_pending_gate_run` while `blocking_phase` is `Phase 1`. `scripts/p031-thin-ui-gate.py:934-945` treats non-ready Phase 1 manifest entries as blockers, so the actual handoff check fails:

```text
python3 scripts/p031-thin-ui-gate.py --repo-root .
Phase 0 manifest entry p041_parity_evidence blocks Phase 1: not_ready_pending_gate_run (docs/reference/p031-p041-parity-evidence.json)
```

This violates the P041 Phase C agreement requirement in proposal Section 6.6/8 that `scripts/test-gate.sh`, `scripts/p031-thin-ui-gate.py`, `docs/reference/p031-phase-0-artifact-manifest.json`, and the reference artifacts agree on the runtime-versus-reference handoff.

### REL-P041-001 - Gate deadlines do not match the proposal contract

Severity: P1  
Reviewer lenses: reliability, readiness/rollout, performance guardrails

The proposal requires the full gate to be bounded by 25 minutes, replay by 60 seconds, readback by 30 seconds, live-shadow by 60 seconds, and drain by 30 seconds. The current gate sets `P041_GATE_DEADLINE_SECONDS` to `7200` by default at `scripts/test-gate.sh:2595` and `p041_supervised_run` at `scripts/test-gate.sh:697-731` enforces only one global deadline for each supervised command. The replay/readback/shadow tests each loop over all fixtures internally, so an individual fixture stall is not bounded by the proposal's per-fixture deadlines.

### REL-P041-002 - Interruption state is not implemented as a P041 runtime outcome

Severity: P1  
Reviewer lenses: reliability, macOS process lifecycle

The proposal requires SIGINT/SIGTERM handling before cleanup and a `blocked_interrupted` diagnostic path. The implementation defines an `InterruptionMarker` type and status mapping, but the gate has no P041 signal handler writing `interruption-marker.json`. The only supervisor interruption path catches `KeyboardInterrupt` and continues through the timeout branch, writing `blocked_timeout` instead. This leaves operator interruption behavior outside the runtime evidence contract.

### MACOS-P041-001 - Supervisor control writes skip Darwin `F_FULLFSYNC`

Severity: P2  
Reviewer lenses: macOS, reliability

Most P041 atomic writers now prefer `F_FULLFSYNC` on Darwin, but the supervisor helper's `_atomic_json` at `scripts/test-gate.sh:626-637` writes control files with `os.fsync(handle.fileno())` only. That helper updates `lease.json`, `current-step.json`, and `timeout-marker.json`, which are canonical `parity-control` files named by the proposal's durable-write contract.

## Verification Log

Passed:

```text
./scripts/test-gate.sh proposal-041
[PASS] Proposal 041 control-plane gate: ready_same_tree_verified
```

Important evidence from the passing run:

- Generation: `p041-2026-05-05T05:58:49Z-0cb49e28`
- Runtime row status: `ready_same_tree_verified`
- Runtime detail status: `ready_same_tree_verified`
- Required fixtures: 7
- Required surfaces: 6
- Live checkout: clean, status line count `0`

Failed adjacent handoff check:

```text
python3 scripts/p031-thin-ui-gate.py --repo-root .
Phase 0 manifest entry p041_parity_evidence blocks Phase 1: not_ready_pending_gate_run (docs/reference/p031-p041-parity-evidence.json)
```

Working tree after verification was clean.

## Closeout Readiness

Do not close out P041 yet. The server parity happy path is passing, but the proposal's closeout bar includes the operational lifecycle and P031 handoff contracts. Closeout should wait until the deadline/interruption/durability gaps are fixed and the P031 handoff gate passes on the same clean tree.
