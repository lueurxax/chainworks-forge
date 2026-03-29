# MVP Sign-Off

Stable reference for the implemented MVP hardening and sign-off layer that was previously tracked by Proposal 008.

## Purpose

Chainworks Forge does not treat "the workflow ran once" as equivalent to MVP closure.

The product needs one explicit, replayable sign-off contract that answers:

- whether benchmark runs are persisted in a form that can be re-evaluated,
- whether recovery and export states remain shell-owned and operator-readable,
- whether the MVP provider boundary is frozen consistently,
- and whether one final `GO` or `HOLD` decision can be justified from persisted records rather than notebook notes.

This document defines that implemented contract.

## Scope

This reference covers:

- persisted benchmark and sign-off state that lives outside the operational `Run`,
- the canonical MVP provider boundary,
- sign-off measurements and `GO/HOLD` evaluation rules,
- shell-owned recovery, export, and summary surfaces,
- approval relaunch and export-state truth,
- current-head proof expectations for MVP sign-off.

It does not redefine the repo-backed delivery runtime itself.
That remains owned by:

- [full-mvp-delivery.md](full-mvp-delivery.md),
- [runtime-contract.md](runtime-contract.md),
- [operator-experience.md](operator-experience.md),
- [provider-platform.md](provider-platform.md),
- [run-control.md](run-control.md).

## Core rules

The MVP sign-off layer is built around five rules:

1. benchmark and sign-off state must persist outside the operational `Run` aggregate,
2. `GO/HOLD` decisions must be replayable from persisted benchmark records only,
3. blocked recovery, export, and sign-off remain subordinate shell routes, not parallel top-level destinations,
4. the MVP provider boundary is frozen and consistent across UI, runtime, and evaluation,
5. MVP sign-off is current-head evidence, not inherited proof from an older accepted SHA.

## Frozen MVP boundary

The implemented MVP provider set is:

- `codex`
- `claude_code`
- `gemini`

Consequences:

- preflight, provider diagnostics, run-start copy, and benchmark/sign-off evaluation all assume that these three provider families are first-class MVP participants,
- no sign-off packet may imply a wider default provider surface,
- future provider expansion remains possible, but it is not part of the MVP sign-off contract.

## Persisted benchmark and sign-off model

The sign-off layer adds a separate persisted model linked to runs by ID.

Required persisted entities:

- `BenchmarkCohort`
- `BenchmarkExecutionRecord`
- `BenchmarkPair`
- `MVPSignOffDecisionSnapshot`

Rules:

- `BenchmarkExecutionRecord` links to the operational run by `runID` for app-driven sessions and stays nullable for manual baselines,
- rerunning the evaluator on the same stored benchmark records must produce the same decision payload,
- exported sign-off packets must include enough information to replay the decision without raw-log archaeology,
- sign-off data never mutates historical run artifacts to "improve" an older benchmark result.

## Measurement contract

For every benchmark execution the system persists:

- `time_to_proposal_approval`
- `time_to_implementation_approval`
- `time_to_final_release_decision`
- `total_orchestration_time`
- `run_outcome`

Accepted `run_outcome` values:

- `happy_path_completed`
- `recovered_non_happy_path_completed`
- `failed_unrecovered`

Timing semantics:

- `time_to_proposal_approval` runs from run start to the explicit proposal approval decision,
- `time_to_implementation_approval` runs from run start to the explicit decision to leave implementation review/refinement and proceed toward release,
- `time_to_final_release_decision` runs from run start to the human release approval or rejection,
- `total_orchestration_time` runs from run start to terminal completion or terminal hold.

## Sign-off gate

The final decision is explicit:

- `GO`
- `HOLD`

`GO` requires all of the following:

1. the benchmark cohort is complete,
2. median `total_orchestration_time` shows at least a 50% improvement over the manual baseline,
3. all checkpoint timings are present for every benchmark pair,
4. at least one happy-path and one recovered non-happy-path evidence packet are complete,
5. no reviewed benchmark run still depends on raw-log archaeology for recovery or explanation,
6. the current-head proof set is green on the approved host for the canonical non-UI and UI gates required by the sign-off flow.

Otherwise the decision is `HOLD`.

## Shell ownership

The sign-off layer does not create a second shell.

Canonical owner path:

1. `RunsHomeView`
2. `RecoverySheet` / `BlockedRunRecoveryView`
3. `RunReportView`
4. `CompletedRunExportHub`
5. `MVPSignOffSummaryView`

Rules:

- blocked recovery remains operator-visible from the existing shell,
- terminal repo-backed runs expose export and sign-off summary as subordinate report routes,
- waiting-approval relaunch must restore the pending approval state instead of silently rerunning the stage,
- export state is driven by persisted truth such as `evidencePackExportedAt`, not by optimistic artifact presence alone.

## Export and replayability

Two packet types matter for sign-off:

1. the completed-run export packet,
2. the benchmark/sign-off decision packet.

Both must remain replayable and operator-readable.

Required sign-off packet contents:

- benchmark cohort identity,
- pair membership and execution mode,
- linked run identifiers,
- recorded timings,
- terminal outcome,
- exported evidence-pack status,
- computed decision payload,
- evaluator version and payload checksum.

## Proof expectations

The sign-off layer is only considered credible when the current head can show:

- a green non-UI proving path on the approved remote host,
- a screenshot-bearing UI smoke path on that same host,
- one current-head happy-path repo-backed app-launched evidence packet,
- one current-head non-happy-path repo-backed app-launched evidence packet,
- a replayable sign-off decision snapshot derived from persisted benchmark records.

This reference defines the contract.
The current proof state lives in [../evidence/mvp-sign-off-proof.md](../evidence/mvp-sign-off-proof.md).

## Adjacent references

Use:

- [full-mvp-delivery.md](full-mvp-delivery.md) for the repo-backed execution and release slice,
- [operator-experience.md](operator-experience.md) for shell-owned recovery/report behavior,
- [provider-platform.md](provider-platform.md) for diagnostics and provider boundary surfaces,
- [run-control.md](run-control.md) for stop/cancel truth,
- [../evidence/full-mvp-delivery-proof.md](../evidence/full-mvp-delivery-proof.md) for delivery-slice proof status,
- [../evidence/mvp-sign-off-proof.md](../evidence/mvp-sign-off-proof.md) for current sign-off proof status.
