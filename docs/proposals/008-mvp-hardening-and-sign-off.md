# Proposal 008: MVP Hardening and Sign-Off — Validation Loop, Boundary Freeze, Recovery UX, and Launch Gate

| Field | Value |
|---|---|
| Date | 2026-03-24 |
| Status | Draft |
| Author | Engineer (single-engineer project) |
| Depends on | [ps/chainworks-forge-mvp.md](../ps/chainworks-forge-mvp.md), [reference/runtime-contract.md](../reference/runtime-contract.md), [006-provider-expansion-settings-diagnostics-and-mvp-pilot-readiness.md](006-provider-expansion-settings-diagnostics-and-mvp-pilot-readiness.md), [007-full-mvp-delivery-slice-worktrees-implementation-loop-manual-release-and-dogfooding.md](007-full-mvp-delivery-slice-worktrees-implementation-loop-manual-release-and-dogfooding.md) |
| Adjacent work | Proposal 003 (Forge Steward) and backend extraction remain explicitly post-MVP |
| Goal | Convert the post-007 product from a believable dogfood slice into a sign-off-ready MVP by freezing the final boundary, proving the PS success metric, hardening repo-backed operator UX, and defining an explicit go/no-go launch gate. |

---

## 1. Context

Proposal 007 is the first full repo-backed delivery slice.
That is necessary, but it is not yet the same thing as MVP sign-off.

The PS still defines success as:

> 50% reduction in manual orchestration time per idea.

That metric is stronger than:

- “the app can execute the full loop once,”
- “the dogfood flow looks believable,”
- or “the runtime architecture is now complete enough.”

After Proposal 007, the next step should therefore be narrow and pragmatic:

- no new capability family,
- no new platform extraction,
- no new orchestration ambition,
- just MVP closure.

Proposal 008 is that closure slice.

### 1.1 Hard prerequisite from Proposal 007

Proposal 008 does not start from an abstract roadmap dependency.
It starts only after Proposal 007 is both:

- implemented,
- and review-proven on current `HEAD` with repo-backed evidence.

That means:

- Proposal 007 must no longer be `Evidence Gap Review`;
- the repo-backed workflow, worktree path, release gate, and delivery receipts must already be demonstrated on current `HEAD`;
- no `GO/HOLD` evaluation may run against a future-state-only 007 runtime.

If Proposal 007 is still red or only partially evidenced, Proposal 008 remains blocked.

### 1.2 What this proposal is not

Proposal 008 is **not**:

- Forge Steward activation,
- Temporal/backend extraction,
- a new provider-expansion proposal,
- autonomous optimization,
- or a general “polish everything” bucket.

It is the contract for:

- validating the MVP against the PS,
- freezing the final MVP boundary,
- hardening the last operator surfaces required for sign-off,
- and deciding whether the product is actually ready to call “MVP achieved.”

---

## 2. Product question this proposal must answer

After Proposal 008, the engineer must be able to answer all of these with evidence rather than instinct:

1. Does Chainworks reduce manual orchestration time by at least 50% on a fixed benchmark cohort?
2. Is the MVP boundary frozen and internally consistent across PS, runtime contract, proposals, UI copy, and acceptance tests?
3. Can blocked repo-backed runs be recovered without reconstructing context from raw logs?
4. Does every completed sign-off run produce one trustworthy operator packet:
   - report,
   - receipts,
   - elapsed time,
   - cost,
   - and evidence-pack status?

If any of these answers is still “not really,” then the product is not at MVP sign-off yet, even if the underlying delivery runtime is impressive.

### Definition of done

Proposal 008 is done only when all of the following are true at once:

1. the benchmark cohort and manual-vs-app protocol are fixed and repeatable;
2. the product proves the PS success metric or fails it honestly with a hold decision;
3. the MVP provider boundary is frozen to one canonical list everywhere;
4. all remaining PS open questions are resolved into explicit contracts;
5. blocked review/release re-entry and completed-run export are operator-complete;
6. the app can produce a final go/no-go sign-off packet for MVP review.

---

## 3. What we build

Two tightly scoped layers.

### Layer K: MVP Validation and Launch Gate

| Component | Responsibility |
|---|---|
| **BenchmarkCohortDefinition** | Defines the fixed idea/repository set used for manual-vs-app comparison |
| **ManualBaselineImport** | Records the manual-orchestration half of each benchmark pair into persisted benchmark state |
| **BenchmarkRunRecorder** | Captures proposal approval, implementation approval, release decision, elapsed time, outcome metadata, and artifact links for app-driven benchmark runs |
| **MVPSignOffEvaluator** | Computes pass/fail against the PS success metric and guardrails strictly from persisted benchmark records |
| **SignOffEvidencePackBuilder** | Exports the final review packet for a benchmark run or cohort |
| **MVPBoundaryPolicy** | Freezes the final provider set and other MVP scope boundaries |

### Layer L: Operator Closure UX

| Component | Responsibility |
|---|---|
| **RunsHomeSignOffRoute** | Shell-owned route from `RunsHomeView` into recovery, export, and sign-off states for one selected run |
| **BlockedRunRecoverySurface** | Recovery subroute owned by the current shell for blocked implementation/review/release re-entry |
| **CompletedRunExportHub** | Terminal export subroute owned by `RunReportView` for report, receipts, evidence-pack status, elapsed time, and cost |
| **MVPSignOffSummarySurface** | Sign-off summary subroute rendered from the current shell/report context, not as a parallel top-level destination |
| **ApprovalResumeRouter** | Defines relaunch behavior for runs waiting at approval gates |
| **OutputRetrievalSLOProbe** | Measures active output/report open latency against the MVP SLO |

---

## 4. Frozen MVP boundary

Proposal 008 must freeze one canonical MVP boundary and remove late-stage ambiguity.

### 4.1 Canonical MVP provider set

The MVP provider set is:

- `codex`
- `claude_code`
- `gemini`

This proposal intentionally treats those three as the required provider families for MVP sign-off.
Anything beyond them is post-MVP.

### 4.2 Consequences

- PS, runtime contract, proposal text, UI copy, and acceptance tests must all use the same provider boundary.
- No MVP sign-off path may depend on a fourth provider family.
- Provider diagnostics, Start Run copy, and benchmark runs must treat Codex, Claude Code, and Gemini as first-class supported providers.
- Future provider expansion remains possible, but it is explicitly outside MVP sign-off.

### 4.3 Explicitly out of MVP after this freeze

- provider families beyond Codex, Claude Code, and Gemini,
- automated provider benchmarking,
- provider auto-routing based on cost/performance heuristics,
- live pricing sync,
- policy engines that mutate provider choice dynamically during sign-off runs.

---

## 5. Final MVP validation loop

### 5.1 Benchmark cohort

Proposal 008 defines one fixed benchmark cohort:

- **Repository A**: the controlled sample repo profile from Proposal 007
- **Repository B**: one messier real-world repo used by the engineer for actual work
- **Idea set**: 6 total ideas
  - 3 ideas executed on Repository A
  - 3 ideas executed on Repository B

Each idea in the benchmark cohort is executed twice:

1. manual orchestration baseline
2. Chainworks app-driven orchestration

### 5.2 Persisted benchmark and sign-off model

Proposal 008 does **not** extend the operational `Run` aggregate with launch-governance state.
Instead it introduces a separate persisted sign-off model linked to run IDs.

Required persisted records:

- `BenchmarkCohort`
  - frozen cohort ID
  - repository profile
  - idea identifier
  - cohort membership metadata
- `BenchmarkExecutionRecord`
  - execution mode: `manual_baseline` or `app_driven`
  - linked `runID` for app-driven executions, nullable for manual baselines
  - checkpoint timestamps
  - terminal outcome
  - elapsed time
  - artifact links
- `BenchmarkPair`
  - one manual execution record
  - one app-driven execution record
  - immutable pair ID
- `MVPSignOffDecisionSnapshot`
  - evaluator version
  - cohort ID
  - computed medians
  - failing gate reasons
  - exported decision payload checksum

Rules:

- `BenchmarkRunRecorder` and `ManualBaselineImport` write only these persisted benchmark records;
- `MVPSignOffEvaluator` reads only these persisted benchmark records;
- rerunning the evaluator on the same stored records must yield the same `GO/HOLD` result;
- the exported sign-off packet must contain enough data to replay the decision without notebook notes or raw-log archaeology.

### 5.3 Required measurements

For every benchmark execution, the system must capture:

- `time_to_proposal_approval`
- `time_to_implementation_approval`
- `time_to_final_release_decision`
- `total_orchestration_time`
- `run_outcome`
  - `happy_path_completed`
  - `recovered_non_happy_path_completed`
  - `failed_unrecovered`

### 5.4 Timing semantics

Proposal 008 freezes the timing contract:

- `time_to_proposal_approval`: from run start to explicit proposal approval decision
- `time_to_implementation_approval`: from run start to explicit approval to leave implementation review/refinement and proceed to release
- `time_to_final_release_decision`: from run start to explicit human release approval or rejection decision
- `total_orchestration_time`: from run start to terminal run completion or terminal hold state

### 5.5 Required evidence

The benchmark loop is not complete without both:

- at least one happy-path evidence pack
- at least one recovered non-happy-path evidence pack

Recovered non-happy-path means:

- a meaningful block occurred,
- the operator used the app’s intended recovery path,
- the run or cloned recovery path still produced a trustworthy terminal packet.

### 5.6 Sign-off gate

Proposal 008 defines one explicit launch gate:

`GO` only if all of the following hold:

1. median `total_orchestration_time` improves by at least 50% versus the manual baseline;
2. all three checkpoint timings are present for every benchmark run;
3. at least one happy-path and one recovered non-happy-path evidence pack are complete;
4. no benchmark run requires raw-log archaeology for operator recovery;
5. evidence packs, reports, and receipts are exportable from the app for all sign-off runs.

Otherwise:

`HOLD`

The purpose of Proposal 008 is not to force a green result.
It is to force an honest one.

### 5.7 Required sign-off summary payload

`MVPSignOffSummarySurface` and the exported sign-off packet must both show:

- cohort members,
- pair IDs,
- manual baseline records,
- app-driven records,
- per-run checkpoint timings,
- median calculation inputs,
- computed median outputs,
- explicit failing gate reasons when the result is `HOLD`,
- evaluator version and exported payload checksum.

The operator must be able to reconstruct the final decision from the app or the exported packet alone.

---

## 6. Remaining PS contracts closed here

Proposal 008 closes the unresolved PS details that still make MVP validation fuzzy.

### 6.1 Attachment policy

MVP idea attachments are:

- optional,
- single-file,
- local-path-based references,
- read-only.

In Proposal 008, attachments are explicitly **reference-only**.
They are stored, displayed, exported, and validated as local references for operator context, but they are **not** automatically injected into agent execution context.

That means:

- an attachment can inform the operator;
- an attachment can appear in reports and sign-off packets;
- an attachment can be validated as an allowed local reference;
- but the current MVP runtime does not treat `attachmentPath` as an agent-ingested execution input.

Supported reference attachment types in MVP:

- `.md`
- `.txt`
- `.pdf`
- `.png`
- `.jpg`
- `.jpeg`
- `.json`
- `.yaml`
- `.yml`
- `.swift`
- `.diff`
- `.patch`

Anything else is out of MVP unless explicitly added later.

UI and runtime consequences:

- every attachment must show one of two states:
  - `reference_only`
  - `rejected`
- unsupported paths or extensions produce a deterministic rejection record before run start;
- product copy must never imply that a reference attachment was consumed by agents unless a later proposal adds real attachment ingestion.

### 6.2 Cost granularity

MVP cost policy is:

- **completed-run overview** shows total run cost;
- **completed-run export hub** also exposes per-stage and per-agent receipt breakdown when available.

This keeps the top-level operator surface simple without throwing away auditability.

### 6.3 Relaunch behavior at approval gate

If the app relaunches while a run is at an approval gate:

- the run is restored to `waiting_approval`;
- the operator is routed to a visible approval/recovery context;
- the app does **not** auto-open a destructive modal action;
- the app does **not** continue execution silently.

The system should foreground the pending approval through the operator shell, not through surprise execution.

### 6.4 Active output/report SLO

Proposal 008 replaces the PS `[TBD]` with:

- `p95 <= 2.0 seconds` to open an active output or completed report surface on a typical local machine

Measurement rule:

- measured from operator action to first rendered output/report content
- verified on local benchmark hardware using artifact sizes representative of MVP runs
- benchmarked on the primary MVP dogfood machine class named in the evidence pack
- reported with `p50`, `p95`, and `p99`

Surface-state rule:

- report/export surfaces must define loading, empty, timeout, and retry states;
- no report/export transition may render a blank-content shell while data is still pending.

---

## 7. Final repo-backed operator UX for sign-off

Proposal 007 gets the repo-backed runtime into place.
Proposal 008 hardens the last mile so the operator can actually sign it off.

### 7.1 Shell ownership is explicit

Proposal 008 does not introduce a parallel operator shell.
All new recovery/export/sign-off flows remain owned by the current shell hierarchy:

- `RunsHomeView` remains the canonical selected-run owner;
- `RecoverySheet` remains the canonical entry to blocked-run recovery;
- `RunReportView` remains the canonical entry to completed-run export and sign-off summary;
- any new subviews are subordinate routes or embedded sections under those owners.

There must be one selected-run source of truth for:

- recovery,
- export,
- sign-off summary,
- and return navigation.

No duplicate top-level destinations are allowed for the same operator task.

### 7.2 Blocked review / release re-entry

The app must provide one visible recovery surface, entered from `RunsHomeView` and hosted by `RecoverySheet`, for:

- blocked implementation review,
- blocked release,
- partial release completion,
- approval-returned recovery paths.

That surface must show:

- blocker reason,
- current stage and stage history,
- preserved receipts,
- diff/test context,
- next valid operator actions,
- whether the path is resume, retry, clone, or cancel.

Return behavior:

- leaving recovery returns to the same selected run in the shell;
- no rediscovery of the run is required after recovery actions;
- blocked and resumed paths preserve the same run context unless the operator explicitly clones.

### 7.3 Completed-run export hub

Terminal repo-backed runs must expose one export hub, entered from `RunReportView`, containing:

- final report,
- provider and delivery receipts,
- evidence-pack status,
- elapsed time,
- total cost,
- per-stage/per-agent receipt breakdown,
- export actions for the sign-off packet.

Visual hierarchy rule:

- the top region is one dominant summary block with run result, elapsed time, total cost, and evidence-pack status;
- the next region is the sign-off summary block;
- receipt breakdowns and detailed per-stage/per-agent records live behind disclosure groups or subordinate sections;
- export actions stay grouped in one footer/action region.

### 7.4 Sign-off summary surface

`MVPSignOffSummarySurface` is not a separate app destination.
It is a shell-owned subroute or embedded section under `RunReportView` / export flow.

It must show:

- cohort identity,
- pair identity,
- manual-vs-app comparison,
- checkpoint timings,
- median calculation,
- `GO/HOLD` result,
- explicit failing gate reasons,
- linkouts to the exported sign-off packet and evidence pack.

### 7.5 Evidence-pack status is first-class

The operator should not have to guess whether a run is sign-off-ready.

Each completed benchmark run must show:

- `evidence_pack_missing`
- `evidence_pack_in_progress`
- `evidence_pack_ready`
- `evidence_pack_exported`

### 7.6 Recovery stays explicit

Proposal 008 does not introduce clever autonomous recovery.

Recovery remains:

- explicit,
- visible,
- receipt-backed,
- and operator-approved.

---

## 8. File and component additions

```text
Chainworks Forge/
  Support/
    MVPBoundaryPolicy.swift              ← NEW
    BenchmarkCohortDefinition.swift      ← NEW

  Models/
    BenchmarkCohort.swift                ← NEW
    BenchmarkExecutionRecord.swift       ← NEW
    BenchmarkPair.swift                  ← NEW
    MVPSignOffDecisionSnapshot.swift     ← NEW

  Engine/
    ManualBaselineImport.swift           ← NEW
    BenchmarkRunRecorder.swift           ← NEW
    MVPSignOffEvaluator.swift            ← NEW
    SignOffEvidencePackBuilder.swift     ← NEW
    OutputRetrievalSLOProbe.swift        ← NEW

  Views/
    RunsHomeView.swift                   ← CHANGED: shell-owned routing into recovery/export/sign-off
    RecoverySheet.swift                  ← CHANGED: blocked-run recovery surface
    RunReportView.swift                  ← CHANGED: completed-run export hub + sign-off summary
    BlockedRunRecoveryView.swift         ← NEW: shell-owned subview, not top-level destination
    CompletedRunExportHub.swift          ← NEW: shell-owned subview/section
    MVPSignOffSummaryView.swift          ← NEW: shell-owned subview/section
```

---

## 9. Acceptance criteria

### Boundary freeze

- [ ] The canonical MVP provider set is fixed to `codex`, `claude_code`, and `gemini`
- [ ] PS, runtime contract, proposals, and UI copy use the same provider boundary
- [ ] No MVP acceptance path depends on any provider family beyond that set
- [ ] Proposal 008 cannot begin implementation or `GO/HOLD` evaluation until Proposal 007 has current-head green evidence for repo-backed completion

### Benchmark and sign-off

- [ ] The benchmark cohort is fixed and documented
- [ ] Manual-vs-app comparison is repeatable
- [ ] Every benchmark run captures proposal approval, implementation approval, release decision, and total elapsed time
- [ ] Manual baselines and app-driven runs are stored as persisted benchmark records with immutable pair IDs
- [ ] The app computes a final MVP `GO` or `HOLD` result only from persisted benchmark records
- [ ] The exported sign-off packet is sufficient to replay the final decision without external notes

### PS closure

- [ ] Supported attachment file types are explicitly fixed as reference-only attachments for MVP
- [ ] Cost granularity is fixed across report and export surfaces
- [ ] Approval-gate relaunch behavior is fixed
- [ ] Active output/report retrieval has a numeric SLO and a verification method

### Operator closure UX

- [ ] Blocked implementation/release recovery is available from one shell-owned visible surface
- [ ] Terminal repo-backed runs expose a completed-run export hub through `RunReportView`
- [ ] Evidence-pack status is visible on completed benchmark runs
- [ ] Sign-off summary is reachable from the current operator shell with preserved selected-run context
- [ ] Recovery, re-entry, and export states are screenshot-tested

### MVP sign-off evidence

- [ ] At least one happy-path evidence pack exists
- [ ] At least one recovered non-happy-path evidence pack exists
- [ ] One benchmark repo is a messier real-world target, not only the sample repo
- [ ] MVP sign-off cannot pass without complete exported review packets

---

## 10. Out of scope

| Exclusion | Reason |
|---|---|
| Forge Steward activation | Valuable later, but not part of MVP sign-off |
| Backend extraction / Temporal migration | Architectural scaling work, not MVP closure |
| Provider families beyond Codex, Claude Code, and Gemini | Scope freeze matters more than breadth at sign-off time |
| “Smart” autonomous optimization beyond explicit recovery | Hardening should reduce ambiguity, not add it |
| New workflow families unrelated to MVP sign-off | Proposal 008 validates and closes, it does not expand |
| Automatic attachment ingestion into agent context | MVP keeps attachments reference-only unless a later proposal expands runtime input handling |

---

## 11. Locked decisions

| ID | Decision | Rationale |
|---|---|---|
| ARCH-080 | The canonical MVP provider set is `codex`, `claude_code`, and `gemini` | Remove contract drift and honor the intended MVP provider breadth |
| ARCH-081 | Proposal 008 is a hardening/sign-off slice, not a capability-expansion slice | Keep the roadmap convergent |
| ARCH-082 | MVP sign-off requires a benchmark cohort and an explicit `GO/HOLD` gate computed only from persisted benchmark records | The PS success metric must be proven, not assumed |
| ARCH-083 | Proposal 008 is blocked until Proposal 007 has current-head green repo-backed evidence | Sign-off cannot depend on future-state runtime |
| ARCH-084 | Benchmark/sign-off state lives outside the operational `Run` aggregate | Keep launch-governance data replayable without polluting runtime lifecycle |
| ARCH-085 | Recovery/export/sign-off flows remain shell-owned extensions of `RunsHomeView`, `RecoverySheet`, and `RunReportView` | Preserve operator coherence and one selected-run source of truth |
| ARCH-086 | MVP attachments are reference-only local-path artifacts, not agent-ingested inputs | Keep product language aligned with runtime reality |
| UX-080 | Relaunch at approval gate returns to visible approval context without silent execution | Preserve operator control |
| UX-081 | Completed-run overview shows total cost; export hub adds breakdown | Keep top-level UI calm while preserving auditability |
| PERF-080 | Active output/report retrieval target is `p95 <= 2.0s` | Replace the PS `[TBD]` with a measurable contract |

---

## 12. Execution plan

| Day | Deliverable |
|---|---|
| Day 0 | Reconfirm Proposal 007 current-head green prerequisite before any 008 implementation starts |
| Day 1 | Freeze provider boundary, attachment policy, and PS contract decisions |
| Day 2 | Define persisted benchmark cohort, pair model, manual-baseline import, and timing instrumentation |
| Day 3 | Define shell-owned recovery/export/sign-off routing and UI hierarchy |
| Day 4 | Add evidence-pack status and sign-off packet contract |
| Day 5 | Add replayable `GO/HOLD` evaluator and decision snapshot model |
| Day 6 | Run first happy-path sign-off rehearsal |
| Day 7 | Run first recovered non-happy-path sign-off rehearsal |
| Day 8 | Final MVP review packet and `GO/HOLD` decision |

---

## 13. What this proposal enables

Proposal 008 is the point where the product stops asking for trust and starts earning it.

It enables:

- a frozen MVP boundary,
- a real validation loop tied to the PS,
- a final operator experience that is good enough to sign off,
- and a credible handoff from MVP work into post-MVP evolution.

After Proposal 008, the next roadmap step can be chosen honestly:

- if the sign-off gate is green, move into post-MVP work;
- if it is red, the repo now says exactly why.
