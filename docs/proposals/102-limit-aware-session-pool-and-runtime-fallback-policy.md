# Proposal 102: Limit-Aware Session Pool and Runtime Fallback Policy

| Field | Value |
|---|---|
| Date | 2026-06-09 |
| Status | Draft / Parked until P073 freeze lifts; depends on P101 observability landing first |
| Author | Roadmap triage 2026-06-09 |
| Depends on | P101 agent limit observatory, implemented [P079 output repair/fallback contract](../reference/output-contracts-failure-evidence-and-recovery.md#p079-output-contract-repair-and-fallback-details), implemented session lineage/reuse-scope machinery, durable side-effect ledger (P078) |
| Related | `docs/reference/session-lineage-reuse-and-operator-reset.md`, `docs/reference/acp-runtime-transport.md`, `docs/reference/provider-binding-truth.md`, `docs/reference/executable-rollout-gate-template.md` |
| Scope | Use P101 limit signals to drive session pooling/reuse decisions and bounded provider/model fallback when a binding hits limits, inside the existing session lineage and P079 fallback rails, with durable typed decisions and full readback. |
| Non-goal | Never applies to release/publish/git/upload side-effect lanes. No silent provider substitution. No changes to output contracts or approval semantics. Not an autoscaler. |

---

## 1. Problem

When a provider binding hits a rate limit or quota wall mid-run, Chainworks
today either stalls, burns retries against the same exhausted binding, or
requires manual operator intervention. Session reuse decisions ignore limit
pressure entirely. P101 gives the system limit truth; this proposal makes the
runtime act on it in a narrow, auditable way.

## 2. Goals

- G-1: Session pool admission consults P101 limit pressure: a binding in
  observed exhaustion is not handed new work turns until its pressure clears
  or a configured cool-down elapses.
- G-2: Bounded fallback: when a binding is exhausted and the agent catalog
  declares an eligible fallback profile, the runtime may reroute the next
  attempt through the P079 fallback path — same contract, same prompts,
  recorded as a typed durable decision (`limit_fallback_decision`).
- G-3: Every pooling/fallback decision is durable before it takes effect and
  readable back over GraphQL (read-only) and MCP, including the limit evidence
  that justified it.
- G-4: Kill switch defaults: the policy ships disabled; enabling is an
  explicit MCP-side configuration action, never a UI mutation.

## 3. Non-Goals

- No fallback for release/publish/git-push/upload stages; side-effect lanes
  stay fail-closed behind the durable side-effect ledger regardless of limit
  pressure.
- No cross-run global scheduling or fairness work.
- No new provider families; fallback targets must already exist in the agent
  catalog as resolved bindings.
- No weakening of P088 output freshness or P095 settlement semantics.

## 4. Design Sketch

- A `limit_policy` module in the engine consumes the P101 projection, not raw
  events, so policy stays decoupled from observation volume.
- Pool admission check runs where session reuse scope is resolved today; an
  exhausted binding yields a typed `binding_cooling_down` work-queue outcome
  rather than a launch attempt.
- Fallback reuses the P079 decision path with a new typed reason
  (`limit_exhaustion`), so evidence, retry lineage, and contract validation
  remain identical to contract-repair fallback.
- Decisions journal through the command/event path before execution
  (durable-intent-first, same shape as side-effect intent discipline).

## 5. Rollout Gates and Observability Contract

- Gate: `./scripts/test-gate.sh proposal-102` — fixture-driven exhaustion →
  cool-down → fallback sequence, side-effect-lane refusal fixtures, decision
  durability/readback tests.
- Metrics: `limit_pool_deferrals_total`, `limit_fallback_decisions_total`,
  `limit_fallback_success_ratio`.
- Readback: typed decision records with justifying evidence in run readback;
  `operator_readback_v1` lane for active cool-downs.
- Hold conditions: any fallback decision observed on a side-effect lane
  (automatic kill-switch), fallback success ratio below threshold, or decision
  records missing durable intent before execution.
- Rollback disposition: feature-flagged policy module; disabling reverts to
  current behavior with decision history retained as inert records.

## 6. Acceptance

- Gate fixtures prove: exhausted binding defers new turns; eligible fallback
  reroutes with full typed evidence; release-lane stages never consult the
  policy; disabled flag means zero behavior change.
- Operator can answer "why did this stage run on a different model?" entirely
  from readback.

## 7. Open Questions

- Cool-down sizing: static per-provider config vs. derived from observed
  retry-after signals.
- Whether fallback eligibility belongs in the agent catalog YAML
  (`backend_profiles`) or in a separate policy document.
- Interaction with P086 same-session continuation: a continued session on an
  exhausted binding probably must pause rather than fall back mid-lineage.
