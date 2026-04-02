# Proposal-Loop Feedback Fidelity and Rereview Proof

Current implementation and proof status for the proposal-loop feedback-fidelity, score-lift backlog, and targeted-rereview slice consolidated from Proposal 022.

## Status

| Field | Value |
|---|---|
| Slice | Proposal-Loop Feedback Fidelity and Rereview |
| Source contract | [../reference/proposal-loop-feedback-fidelity-and-rereview.md](../reference/proposal-loop-feedback-fidelity-and-rereview.md) |
| Current implementation status | Implemented |
| Current readiness | Ready |
| Primary proof owners | `Proposal022Tests`, `Proposal022ScaffoldingTests`, canonical remote `proposal-022` gate |
| Last consolidated documentation refresh | `2026-04-02` |

## What is considered proven

The accepted proof story for this slice supports these claims:

- aggregate proposal review persists a real `review_corpus_bundle` artifact instead of relying on summary-only refine truth,
- the writer consumes the full raw review quartet, aggregate summary, persisted bundle, backlog, and fact digest,
- `score_lift_backlog` persists normalized issue carry-forward with merge provenance,
- writer output persists `proposal_feedback_coverage` as a structured coverage record,
- targeted-rerun inputs and rationale remain visible through canonical artifacts and shell-owned surfaces,
- proposal-loop report/comparison/artifact surfaces expose backlog, coverage, merge-provenance, growth, and bounded-next-action truth,
- and the app-level proof path is satisfied by the remote built-app export consumed by `proposal-022`.

## Accepted current-head proof owners

The strongest current-head proof owners are:

- `Proposal022Tests`
- `Proposal022ScaffoldingTests`
- `scripts/test-gate.sh proposal-022`

The accepted current-head gate for this slice is:

```bash
ssh test@SMacBook.local "cd '/Users/test/chainworks-remote' && ./scripts/test-gate.sh proposal-022"
```

Fresh same-head proof recorded during final documentation closure:

- remote canonical `proposal-022` gate passed on `2026-04-02`
- focused non-UI slice passed `13/13`
- result bundle: `/var/folders/hh/ztmrr5z96xnbxvlcxyf1vxsc0000gp/T/chainworks-test-gates/proposal-022-non-ui-20260402-110940.xcresult`
- app-proof export: `/var/folders/hh/ztmrr5z96xnbxvlcxyf1vxsc0000gp/T/chainworks-test-gates/proposal-022-app-proof-20260402-111013.json`

## Requirement coverage

The final implementation closure established proof for:

- persisted `ReviewCorpusBundle` ownership for refine-handoff fidelity,
- raw quartet plus aggregate summary consumption by the writer,
- normalized score-lift backlog with merge provenance,
- structured writer coverage and unresolved/deferred/disputed truth,
- targeted rereview inputs and rationale,
- proposal growth discipline tied to score movement,
- shell-owned report/comparison/artifact visibility,
- and a canonical remote app-level proof export for the proposal loop.

## Canonical proof lane

The canonical proof owner for this slice is singular:

- focused current-head proposal-loop test suites,
- plus the named remote `proposal-022` gate in `scripts/test-gate.sh`

The gate is canonical because it is the repository-owned proof path for this slice and because it performs the app-level proof through the built application on the approved remote host rather than through an ad-hoc local UI run.

## Consolidation note

The old Proposal 022 draft and implementation audit were implementation-trail artifacts.

They have been superseded by:

- [../reference/proposal-loop-feedback-fidelity-and-rereview.md](../reference/proposal-loop-feedback-fidelity-and-rereview.md)
- this proof document

Historical reviews and evidence packs may remain useful as decision history, but they are no longer the canonical source of truth for current behavior.

## Remaining caution

There is no blocking conformance gap in the current proof story.

The practical caution is ordinary current-head drift in:

- proposal-loop prompt shapes,
- reviewer scope heuristics,
- and report wording around bounded next action.

Those should be rechecked through the same canonical remote `proposal-022` gate rather than assumed stable forever.

## Recommended usage

Use:

- [../reference/proposal-loop-feedback-fidelity-and-rereview.md](../reference/proposal-loop-feedback-fidelity-and-rereview.md) for the stable contract,
- [../reference/live-provider-execution-slice.md](../reference/live-provider-execution-slice.md) for the broader live proposal-loop runtime,
- [../reference/context-strategy-and-experiment-framework.md](../reference/context-strategy-and-experiment-framework.md) for strategy-owned handoff policy,
- [../reference/test-gates.md](../reference/test-gates.md) for the canonical verification lane,
- [../reference/agent-ui-test-execution.md](../reference/agent-ui-test-execution.md) for the remote-only app-proof policy.
