# Proposal Research Pack

## 0. Review Target and Local Context Consumed

- Target proposal: `docs/proposals/013-output-contract-alignment-retry-truth-and-failure-evidence-hardening.md`
- Proposal hash at research time: `c0fc6d7f9cac165a751ea7de7df0507a`
- Local evidence pack used to derive research scope: `docs/reviews/013-output-contract-alignment-retry-truth-and-failure-evidence-hardening-evidence-pack.md`
- Local seams already established before browsing:
  - `DOC-02`, `DOC-03`, `DOC-05`: current repo truth is still immutable stage-attempt artifact storage
  - `DATA-02`, `DATA-03`, `DATA-04`: validation/persistence and retry lineage seams are already mapped
  - `INT-03`, `REAL-01`: Proposal 013 now defines a disjoint same-stage agent-retry namespace
- This research pack does not reopen local architecture review. It only tests whether authoritative external patterns strengthen, narrow, or contradict the current draft.

## 1. Research Questions Derived from Local Evidence

| RQ ID | Question | Local Evidence IDs | Why Local Evidence Was Not Enough |
|---|---|---|---|
| `RQ-01` | Do authoritative workflow systems preserve previous attempt history and evidence when rerunning failed work from the same logical snapshot? | `DOC-02`, `DOC-03`, `DOC-05`, `DATA-03`, `DATA-04`, `INT-03` | Local evidence showed Proposal 013 is coherent internally, but not whether its immutable-history pattern matches stronger host-system practice. |
| `RQ-02` | Should validation failure evidence be persisted as a first-class result object distinct from metrics or summary state? | `DOC-01`, `DATA-02`, `DATA-03`, `TEST-01` | Local evidence justified `ValidationFailureRecord`, but not whether external validation systems treat failure results as durable operator-facing records. |
| `RQ-03` | Should output-contract and validation mismatches be treated more like permanent or operator-actionable failures than transient auto-retry cases? | `DOC-01`, `DATA-02`, `DATA-04`, `TEST-01` | Local draft prefers narrow recovery, but external guidance helps decide whether blind automatic retry is the wrong default for this failure class. |

## 2. Source Ledger

| Source ID | Source | Type | Why It Was Chosen | Freshness / Scope Note |
|---|---|---|---|---|
| `SRC-01` | [GitHub Docs: Re-running workflows and jobs](https://docs.github.com/en/actions/how-tos/manage-workflow-runs/re-run-workflows-and-jobs) | official product docs | Clear, current reference for rerun semantics, same-source reruns, and preserved attempt history UI. | Stable operational docs; low freshness risk. |
| `SRC-02` | [Temporal Docs](https://docs.temporal.io/) | official product docs | Establishes Temporal as a durable execution system with persisted workflow state and replay semantics. | High-level source; useful for framing, not exact retry-policy wording. |
| `SRC-03` | [Temporal workshop PDF: Crafting an error handling strategy](https://learn.temporal.io/assets/files/crafting-an-error-handling-strategy-dotnet-replay2025-e633cef41481baac8b25a636533c4d37.pdf) | official educational material | Concrete reference for persisted failure history, retry attempts, and non-retryable/manual-intervention guidance. | Official but lower authority than core reference docs. Recheck before borrowing exact terminology. |
| `SRC-04` | [Great Expectations 0.18: Validation Result Store](https://docs.greatexpectations.io/docs/0.18/reference/learn/terms/validation_result_store/) | official product docs | Strong pattern for persisting validation results and associated metadata as a first-class store, separate from raw data flow. | Versioned legacy docs; use for pattern guidance, not current API naming. |

## 3. Findings by Theme

### 3.1 Immutable retry lineage and same-snapshot reruns

Research question: `RQ-01`

External signal:

- `SRC-01` says reruns reuse the same original `GITHUB_SHA` and `GITHUB_REF`, while preserving prior run attempts as selectable history.
- `SRC-03` shows failure details and retry attempts are recorded in durable event history rather than overwritten by later attempts.
- `SRC-02` reinforces the broader durable-execution model: workflow progress and recovery are expected to survive faults without losing prior truth.

Inference for Chainworks:

- The new Proposal 013 direction is consistent with stronger host-system practice: same logical work should keep its original frozen source context while later attempts append history instead of mutating it.
- The proposal’s `agent-retry-{agentAttemptNumber}` namespace and lineage metadata are the right shape.
- The highest-value extra clarification is not a new architecture layer; it is a sharper statement that same-run `Retry Failed Agent` reuses the same frozen logical snapshot and leaves prior agent-attempt evidence inspectable even after a later successful retry.

Why this matters locally:

- This reinforces `Section 5.4` and makes `Sections 10.2`, `10.3`, and `11` easier to verify against real runtime evidence.

### 3.2 Validation failure evidence should be a first-class persisted result

Research question: `RQ-02`

External signal:

- `SRC-04` describes a dedicated Validation Result Store that persists validation results and associated metadata automatically.
- `SRC-04` also distinguishes stored validation results from evaluation-parameter and metric usage; metrics may be derived from validation results, but they are not the same object.
- The same source says stored validation results exist for review and rendering into operator-facing Data Docs.

Inference for Chainworks:

- Proposal 013 is strongest when `ValidationFailureRecord` and the failed-stage evidence packet are treated as canonical persisted evidence, not just transient validation diagnostics or summary flags.
- Recovery UI, reports, and exports should reference that canonical persisted failure object or packet directly.
- A report summary alone is not enough; the durable failure record should be the inspectable source of truth behind the summary.

Why this matters locally:

- This strengthens `Sections 6.2`, `6.3`, and `7.3` and supports the motivating requirement that validation failure must not make downstream evidence disappear.

### 3.3 Contract mismatch is closer to a permanent or operator-actionable failure than a transient retry

Research question: `RQ-03`

External signal:

- `SRC-03` explicitly distinguishes permanent failures from transient ones and recommends marking permanent failures non-retryable so they can be fixed manually.
- `SRC-03` also shows that retry history should still preserve failure details even when the failure is considered non-retryable.
- `SRC-04` implicitly supports this separation: a failed validation still yields a durable validation result, not an absence of result.

Inference for Chainworks:

- Output-contract and validation mismatches should default to operator-mediated recovery, not blind auto-retry.
- Narrow retry can still be the preferred recovery action, but it should happen after an operator-visible explanation or an explicit policy decision, because the failure class usually reflects prompt/schema/content mismatch rather than flaky transport.
- Proposal 013 does not need to ban all automatic retries forever, but its default truth should classify these failures as non-transient unless policy explicitly says otherwise.

Why this matters locally:

- This sharpens recovery-policy language in `Sections 5`, `6`, and `7` and reduces the chance that a later implementation quietly reclassifies contract failures as generic transient noise.

## 4. Host-System Applicability Matrix

| Applicability ID | External Pattern | Source IDs | Classification | Chainworks Fit |
|---|---|---|---|---|
| `APP-01` | Preserve prior attempts while rerunning from the same logical source snapshot | `SRC-01`, `SRC-02`, `SRC-03` | `Adopt` | Directly supports Proposal 013’s immutable lineage model and same-run retry semantics. |
| `APP-02` | Persist validation outcomes as first-class reviewable records, not just metrics or summaries | `SRC-04` | `Adopt` | Directly strengthens `ValidationFailureRecord` plus failed-stage evidence packet design. |
| `APP-03` | Treat schema/contract mismatch closer to non-retryable/manual-intervention than to blind transient retry | `SRC-03`, `SRC-04` | `Adapt` | Good default for Chainworks, but should remain policy-shaped because some contract failures may become safely retryable after explicit prompt/contract repair. |
| `APP-04` | Mirror external UI attempt-history controls literally | `SRC-01` | `Reject` | Chainworks should borrow the truth model, not the exact GitHub attempt-switcher UX. Existing recovery/report owners are already correct. |
| `APP-05` | Reuse Great Expectations terminology and store taxonomy verbatim | `SRC-04` | `Watch` | The pattern is useful, but the cited docs are legacy-versioned; adopt the concept, not the names, without a later freshness recheck. |

## 5. Proposal Deltas / Recommended Updates

### `DELTA-01` Adopt

Target areas:

- `Section 5.4`
- `Section 10.2`
- `Section 10.3`
- `Section 11`

Recommended update:

- Add one explicit sentence that same-run `Retry Failed Agent` reuses the same frozen logical snapshot as the failed attempt.
- Add one proof expectation that earlier agent-attempt artifacts, receipts, and transcripts remain inspectable after a later retry succeeds.

Why:

- This is the highest-signal lesson from `SRC-01` plus `SRC-03`, and it makes Proposal 013’s lineage claim more testable.

### `DELTA-02` Adopt

Target areas:

- `Section 6.2`
- `Section 6.3`
- `Section 7.3`

Recommended update:

- Make the `ValidationFailureRecord` or failed-stage evidence packet the canonical reference target for recovery UI, report rendering, and export surfaces.
- State explicitly that summary fields may derive from that object, but may not replace it as the durable source of truth.

Why:

- `SRC-04` strongly supports durable validation-result storage as a first-class operator-visible object rather than only a derived summary.

### `DELTA-03` Adapt

Target areas:

- `Section 5.2`
- `Section 7.2`
- `Section 11`

Recommended update:

- Add a default policy statement that output-contract mismatch and post-generation validation failure are non-auto-retryable by default.
- Narrow retry remains allowed, but as an explicit recovery action or policy override, not as silent blind retry.

Why:

- `SRC-03` supports manual-intervention treatment for persistent failure classes, and this fits Proposal 013’s operator-trust goal.

## 6. Freshness Risks / Recheck Triggers

- `SRC-04` is official but versioned legacy documentation. Recheck current Great Expectations docs before borrowing any class names, store names, or config-shape terminology.
- `SRC-03` is official Temporal educational material, not the primary reference spec. Recheck Temporal reference docs if Chainworks later imports exact retry terminology or policy names.
- Re-run external research if Proposal 013 later grows from retry/evidence hardening into a broader workflow-history or operator-analytics redesign.

## 7. Remaining Open Questions

- Does Chainworks need an explicit persisted frozen-input reference at agent-retry scope so reports can prove “same logical snapshot” without inference?
- Should there be a bounded allowlist of contract-failure cases that may auto-retry safely, or is operator-mediated recovery the only acceptable default for Proposal 013?
