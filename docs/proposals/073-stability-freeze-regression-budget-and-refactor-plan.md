# Proposal 073: Stability Freeze, Regression Budget, and Refactor Plan

| Field | Value |
|---|---|
| Date | 2026-04-25 |
| Status | Draft |
| Author | Engineer (single-engineer project) |
| Depends on | Current implemented baseline, Proposal 031, Proposal 038, [Xcode MCP bridge pool](../reference/xcode-mcp-bridge-pool.md), Proposal 070, [UI action boundary](../reference/ui-action-boundary.md) |
| Goal | Stop feature sprawl temporarily and introduce a stabilization protocol that reduces active architectural seams, artifact noise, projection drift, provider/runtime churn, and UI/control-plane ambiguity. |

---

## 1. Why this proposal exists

The system is no longer feature-poor.

It has:

- ACP runtimes,
- Rust server parity,
- GraphQL projections,
- MCP control plane,
- session lineage reuse,
- output contracts,
- run surfaces,
- proposal-loop fidelity,
- Xcode MCP bridge work,
- provider and runtime policy layers.

The current risk is no longer “we need more features.”
The current risk is:

> too many active seams, too many transitional states, and too many places that can drift.

Proposal 073 introduces a short stabilization freeze and regression budget.

---

## 2. Stabilization freeze

For the next stabilization window, allow only work that reduces complexity or closes active transitions.

Allowed:

- P031 GraphQL thin UI boundary stabilization
- P051 Xcode MCP bridge pool core only
- P038 run compaction
- P070 boundary/typed contract refactor
- UI action boundary
- bug fixes and regression fixes

Frozen:

- new ACP provider families
- new MCP tools not needed by the current boundary
- new UI surfaces
- new context strategy experiments
- new agent roles
- new model-routing experiments
- speculative runtime features

---

## 3. Regression budget

The system should track a small set of stability metrics and block further expansion when they regress.

Minimum metrics:

- failed/blocked run rate
- stale active execution count
- projection lag count
- GraphQL subscription reconnect failures
- artifact count per run
- compacted artifact count per run
- Xcode bridge pool leaks
- ACP session startup latency
- MCP tool failure rate
- UI degraded-state frequency
- run detail query latency
- approval settlement latency

---

## 4. Required gates

Introduce or update gates:

- `proposal-031-boundary`
- `proposal-038-compaction`
- `proposal-051|p051`
- `proposal-068-boundary`
- `proposal-072|p072`
- `proposal-073-stability`

The stability gate should fail when:

- forbidden UI mutations appear,
- agents require GraphQL/SQLite for ordinary operations,
- run artifact count exceeds configured thresholds without compaction,
- projection lag exceeds threshold,
- ACP adapter conformance drops below required capability,
- Xcode bridge pool leaks processes or stale sessions.

---

## 5. Refactor priorities

During the freeze, prioritize:

1. remove duplicate truth lanes;
2. remove client-owned workflow logic where server truth exists;
3. remove ad hoc artifact scans where projections exist;
4. remove GraphQL mutations that are not approval decisions;
5. remove MCP tools that bypass domain invariants;
6. collapse duplicated runtime/provider selection code;
7. make degraded states explicit rather than silent fallback.

---

## 6. What not to do

Do not use this proposal to:

- add new product features,
- add new runtimes,
- widen MCP access,
- widen UI write access,
- add another dashboard,
- start a broad rewrite without a gate.

---

## 7. Exit criteria

The stabilization window can end when:

1. P031 boundary is implemented and enforced;
2. P038 compaction reduces artifact noise on real runs;
3. P051 bridge pool is either stable or feature-flag disabled;
4. the UI action boundary is implemented in docs/gates;
5. projection lag and stale active execution counts are below threshold;
6. no agent-facing docs require GraphQL or SQLite fallbacks;
7. the system feels less noisy and easier to inspect.

---

## 8. Final recommendation

The project does not need a pause.

It needs a controlled tightening window.

During that window, the system should become more boring:

- fewer paths,
- fewer owners,
- fewer fallbacks,
- fewer artifacts,
- fewer hidden recovery modes.

That is the next step toward stability.
