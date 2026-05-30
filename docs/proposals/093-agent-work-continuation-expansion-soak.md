# Proposal 093: Agent Work Continuation Expansion Soak

| Field | Value |
|---|---|
| Date | 2026-05-19 |
| Status | Draft |
| Author | Operator-directed split from agent work continuation implementation |
| Depends on | Implemented live-handle continuation contract in [`docs/reference/agent-work-continuation.md`](../reference/agent-work-continuation.md), plus completion of [`Proposal 086: Provider Session Resurrection Completion`](086-agent-work-continuation-and-lead-directed-session-resumption.md) before resurrection-specific soak evidence is counted |
| Related | Agent work continuation readback in [`docs/reference/agent-work-continuation.md`](../reference/agent-work-continuation.md), P086 provider-session resurrection completion |
| Scope | Own expansion after the implemented continuation contract: 14-day no-hold soak, SLO-budget validation, and at least 100 successful continuations across 30 runs. |
| Non-goal | Do not change the implemented phases 1-4 continuation contract. |

---

## 1. Problem

Expansion and soak is an operational milestone, not an implementation slice.
Keeping it inside the already implemented continuation contract would make
closeout depend on calendar time and production-style evidence that cannot be
produced by more refinement cycles alone.

## 2. Decision

Track expansion and soak as a separate follow-up proposal.

The stable continuation baseline already owns:

1. read-only continuation status/candidates and parity evidence;
2. operator MCP live-handle continuation command path;
3. lead-auto continuation hardening and enablement gates;

Provider-session resurrection per adapter is not implemented by the baseline.
It is active P086 scope. P093 may only count resurrection-specific soak or scale
evidence after P086 implements a supported adapter path and its attach/orphan
evidence.

Proposal 093 owns only post-implementation expansion.

## 3. Acceptance Criteria

1. The agent work continuation contract is implemented, validated, and enabled for internal use.
2. No continuation rollout hold condition trips for 14 consecutive days.
3. SLO budgets remain within the continuation rollout contract thresholds.
4. At least 100 successful continuations are observed across at least 30 runs.
5. Duplicate provider sends remain zero.
6. Unresolved side-effect ledger rows older than 10 minutes remain zero.
7. Operator report and readback lanes show stable parity across MCP, GraphQL, run reports, and release receipts.

## 4. Rollback

If any hold condition trips during expansion, pause expansion, keep continuation
contract evidence intact, and return the affected surface to the narrowest safe
enabled phase.
