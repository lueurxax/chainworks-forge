# P077 Rollout and Dependency Evidence

This document is the durable evidence checklist for Proposal 077 rollout.
It covers the dependency checklist, rollout metrics, rollback rule, and
acceptance evidence that must exist before P077 moves beyond advisory mode.

Proposal source: `docs/proposals/077-bounded-implementation-closeout-readiness-gates.md`

## Dependency Checklist

Each dependency row must have an owner, pass rule, proof source, fallback, waiver
authority, and current evidence status. A row is expansion-ready only when
`evidence_status` is `passed` or `waived` with the listed authority.

| dependency | owner | pass_rule | proof | fallback | waiver_authority | evidence_status |
| --- | --- | --- | --- | --- | --- | --- |
| P052 loop-budget truth | orchestration owner | P077 soft convergence does not claim P052 hard loop exhaustion and respects remaining refine budget | bounded loop/refine tests plus active readiness decision evidence | route to `await_operator_decision`; remain advisory | release owner | pending |
| P059 release-evidence gate contracts | release owner | manual release requires green active proposal gate, green controlled reports, current audit truth, settled risks, and typed lineage | closeout readiness decision log plus manual release receipt readback | block with evidence; no manual release | release owner | pending |
| P073 stability freeze and current audit truth | platform owner | P077 uses frozen R14 proposal source and current audit truth; stale exported JSON is diagnostic only | freeze digest and active SQLite artifact-contract proof | hold expansion until source/audit truth is refreshed | release owner plus platform owner | pending |
| P017 run-state and projection contract | workflow owner | transition evaluation reads active SQLite truth and projections expose only derived readback | active run-state projection and accessor parity proof | keep CLI/MCP readback diagnostic; remain advisory | release owner | pending |
| GraphQL/MCP accessor parity | API owner | same active generation fields across GraphQL, MCP, run-state, and exported projection | `CloseoutReadinessSummaryAccessor` fixtures for `runs.get` and `runs.list` | advisory only | release owner | pending |
| macOS UI evidence | macOS owner | no overlap, current tokens mapped, recovery actions remain read-only/deep-link/copy | state matrix, transient, compact, accessibility, focus, copy, and token fixtures | CLI/MCP readback only; no UI enforcement cutover | release owner plus UX/UI owner | pending |
| fingerprint p95 threshold | control-plane owner | p95 fingerprint latency remains below the release-owner threshold before enforcement | Phase-1 latency snapshot | write `closeout_fingerprint_unavailable` and stay advisory | release owner | pending |

## Metric Ledger

Rollout expansion must be decided from the following metric rows. Each row must
have a metric name, numerator, denominator, threshold, owner, source, and
go_no_go_action. Empty cohorts do not silently expand.

| metric | numerator | denominator | threshold | owner | source | go_no_go_action |
| --- | --- | --- | --- | --- | --- | --- |
| false_ready_prevented | eligible closeouts blocked by P077 where the legacy self-assessment path would have allowed manual release | eligible closeouts | at least one confirmed prevention in the cohort, or a neutral-observation decision is required | release owner | closeout readiness decision log plus legacy comparison | continue advisory, limited enforcement, extend cohort, or hold with written rationale |
| post_release_closeout_gap_reversals | releases reversed because proposal proof, audit truth, gate freshness, risk settlement, or handoff was incomplete | P077-governed manual releases | zero for expansion | release owner | manual release receipts, closeout readiness generations, and post-release incident records | any reversal pauses enforcement expansion and requires corrective action |
| false_blocks | closeouts blocked by P077 that the release owner classifies as incorrect | eligible closeouts | `<= 5%` or `<= 2` in first cohort | control-plane owner | operator override records, release-owner decisions, and readiness diagnostic reasons | breach reverts new runs to advisory within one business day |
| pause_to_action | elapsed business time per paused closeout | paused closeouts | median less than one business day unless release owner waives with reason | operator experience owner | first blocking readiness generation timestamp to acknowledgement, settlement, rerun, or operator decision timestamp | breach requires copy, routing, or ownership fix before expansion |
| code_writer_loops_avoided | non-code handoff or operator-decision cases that did not invoke `code_writer` | non-code handoff or operator-decision cases | `100%`; any regression blocks expansion | orchestration owner | decision route, blocker classification, and code_writer invocation records | fix routing before enforcement expansion |

## Expansion Decision

- `first_cohort`: 10 eligible state-9 closeouts or 10 business days for
  P052/P059/P073-compatible proposal-backed runs.
- `dependency_checklist_result`: all rows passed, or waived with authority and
  rationale.
- `metric_ledger_result`: all thresholds met, or advisory continuation/hold is
  recorded by the release owner.
- `neutral_observation_rule`: if no avoided-false-ready opportunity appears and
  all thresholds are green, the release owner must choose continue advisory,
  limited enforcement, extend cohort with date, or hold with rationale.

## Rollback Rule

Rollback is intentionally narrow and checkable:

- `rollback_trigger_false_blocks`: false-block threshold breach.
- `rollback_trigger_closeout_gap_reversal`: any post-release closeout-gap
  reversal.
- `rollback_action`: new runs revert to advisory within one business day.
- `in_flight_policy`: in-flight modes stay frozen unless migrated by governed
  release-owner decision.
- `durability_requirement`: rollback decisions must reference the metric row,
  release-owner decision, and affected cohort/run list.

## Evidence Capture

For each cohort review, attach or reference:

- closeout readiness decision log snapshot
- manual release receipt readback
- active gate/readiness generation identifiers
- dependency row statuses and waiver records
- metric numerators, denominators, and thresholds
- release-owner go/no-go decision
- rollback decision record when a threshold breach occurs
