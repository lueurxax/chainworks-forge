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
| P052 loop-budget truth | orchestration owner | P077 soft convergence does not claim P052 hard loop exhaustion and respects remaining refine budget | `cargo test -p engine proposal_077_`; active readiness decision evidence | route to `await_operator_decision`; remain advisory | release owner | passed |
| P059 release-evidence gate contracts | release owner | manual release requires green active proposal gate, green controlled reports, current audit truth, settled risks, and typed lineage | closeout readiness decision log, typed risk lineage tests, and manual release receipt readback contract | block with evidence; no manual release | release owner | passed |
| P073 stability freeze and current audit truth | platform owner | P077 uses frozen R14 proposal source and current audit truth; stale exported JSON is diagnostic only | proposal freeze digest, active SQLite artifact-contract proof, and current fingerprint resolver | hold expansion until source/audit truth is refreshed | release owner plus platform owner | passed |
| P017 run-state and projection contract | workflow owner | transition evaluation reads active SQLite truth and projections expose only derived readback | `execute_closeout_transaction_with_projection_rebuild` and projection parity test | keep CLI/MCP readback diagnostic; remain advisory | release owner | passed |
| GraphQL/MCP accessor parity | API owner | same active generation fields across GraphQL, MCP, run-state, and exported projection | `graphql-server` and `mcp-server` `proposal_077_closeout_readback_parity` tests in `proposal-077` gate | advisory only | release owner | passed |
| macOS UI evidence | macOS owner | no overlap, current tokens mapped, recovery actions remain read-only/deep-link/copy | `p077-closeout-readiness-ui-evidence.md` plus Swift presenter diagnostics/accessibility fixtures | CLI/MCP readback only; no UI enforcement cutover | release owner plus UX/UI owner | passed |
| fingerprint p95 threshold | control-plane owner | p95 fingerprint latency remains below the release-owner threshold before enforcement | live worktree fingerprint resolver, timeout budget, and fail-closed unavailable path | write fingerprint-unavailable readiness and stay advisory | release owner | passed |

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

## Durable Rollout Execution

Rollout evidence is not only static advisory text. The control-plane database
now has an executable P077 rollout store:

| storage | purpose | rollback_execution_fixture | evidence_status |
| --- | --- | --- | --- |
| `p077_rollout_metric_events` | durable metric rows for `false_ready_prevented`, `post_release_closeout_gap_reversals`, `false_blocks`, `pause_to_action`, and `code_writer_loops_avoided` | `cargo test -p db p077_rollout_records_live_metric_and_continue_decision` | passed |
| `p077_rollout_decisions` | governed release-owner go/no-go decisions with full decision payload snapshots and optional rollback trigger/action | `cargo test -p db p077_rollout_records_live_metric_and_continue_decision` | passed |
| `p077_rollout_advisory_migrations` | affected-run migration records for rollback to advisory mode | `cargo test -p db p077_rollout_rollback_to_advisory_updates_runs_and_records_migrations` | passed |

The canonical `proposal-077` gate runs these fixtures through
`cargo test -p db p077_rollout`. Rollback is transactional: a
`rollback_to_advisory` decision requires `rollback_trigger`, `rollback_action`,
and affected run ids, updates each affected `runs.closeout_readiness_mode` to
`advisory`, and records one migration row per run.

`p077_rollout_decisions` enforces the full proposal decision payload at write
time. A durable decision row must carry non-empty `decision_scope`,
`decision_type`, `cohort`, `dependency_checklist_snapshot_id`,
`measurement_window`, `next_review_date`, and release-owner rationale, plus
non-negative `eligible_closeouts`, positive `fingerprint_p95_threshold_ms`,
object-shaped `metric_snapshot_json`, non-empty object-shaped
`primary_metric_values_json` and `diagnostic_metric_snapshot_json`,
array-shaped `waivers_json`, and non-empty array-shaped
`readiness_links_json`. Incomplete payloads fail before storage, so a rollout
decision cannot silently degrade to static advisory prose.

## Expansion Decision

- `first_cohort`: 10 eligible state-9 closeouts or 10 business days for
  P052/P059/P073-compatible proposal-backed runs.
- `dependency_checklist_result`: all rows are currently `passed` for the
  advisory implementation cut.
- `metric_ledger_result`: first-cohort counters are not yet expansion-positive;
  enforcement expansion remains advisory until live cohort rows satisfy the
  metric ledger below.
- `neutral_observation_rule`: if no avoided-false-ready opportunity appears and
  all thresholds are green, the release owner must choose continue advisory,
  limited enforcement, extend cohort with date, or hold with rationale.

## Current Decision Snapshot

| decision_field | value |
| --- | --- |
| decision_scope | advisory implementation cut |
| dependency_checklist_result | passed |
| metric_ledger_result | neutral observation; continue advisory until first cohort evidence exists |
| go_no_go_decision | no enforcement expansion from this document alone |
| decision_owner | release owner |
| decision_record_status | durable reference record; live cohort rows append here before expansion |

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
