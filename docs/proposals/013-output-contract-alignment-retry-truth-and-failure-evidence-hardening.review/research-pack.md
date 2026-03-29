# Proposal Research Pack

## 0. Review Target and Local Context Consumed

- Proposal: `docs/proposals/013-output-contract-alignment-retry-truth-and-failure-evidence-hardening.md`
- Research round: `refresh` on `2026-03-29`
- Proposal hash: `e879a47b9d5e44d000bb8adf3e7c7cec62334ca1f14681ec66d7b3dfb0ce80e7`
- Proposal evidence pack used: `docs/reviews/013-output-contract-alignment-retry-truth-and-failure-evidence-hardening-evidence-pack.md`
- Current-system baseline used: `.review-baselines/current-system-baseline.md`
- Proposal-specific integration context used: none
- Existing research pack reused: yes; prior question set reused, source set refreshed and strengthened toward primary docs
- Adjacent docs consumed:
  - `docs/reference/runtime-contract.md`
  - `docs/reference/workflow-execution-engine.md`
  - `docs/reference/operator-experience.md`
  - `docs/reference/full-mvp-delivery.md`
  - `docs/reference/mvp-sign-off.md`
- Current code / module mapping consumed:
  - `Chainworks Forge/Engine/AgentExecutor.swift`
  - `Chainworks Forge/Engine/ArtifactManager.swift`
  - `Chainworks Forge/Engine/ArtifactStorage.swift`
  - `Chainworks Forge/Engine/GooseSessionBridge.swift`
  - `Chainworks Forge/Engine/GooseTransport.swift`
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift`
  - `Chainworks Forge/Models/Artifact.swift`
  - `Chainworks Forge/Models/AgentExecution.swift`
  - `Chainworks Forge/Models/StageExecution.swift`
  - `examples/agents/agents.yaml`
  - `examples/workflows/workflow.yaml`
- Local evidence IDs that triggered research:
  - `REAL-07`: same-stage retry needs disjoint agent-attempt lineage
  - `REAL-08`: failure evidence must survive validation failure and stage settlement
  - `REAL-05`: `backend_profiles.*.structured_output` remains Tier `1` while live transport support is uneven
  - `REAL-10`: Appendix `B` is now correctly tiered, so research can focus on the bounded Tier `1` slice
  - `REAL-11`: top-level wording is slightly broader than the new tiered boundary
- Notes on baseline freshness or local contradictions:
  - repo-level baseline is fresh for topology and dependency-chain context
  - targeted local seam refresh was still required for declarative coverage, retry lineage, and failure-evidence ordering
  - no proposal-blocking contradiction surfaced between proposal, baseline, and current code

## 1. Research Questions Derived from Local Evidence

| Question ID | Derived From (`Proposal gap | Baseline constraint | Host-system integration risk | Unresolved tradeoff`) | Local Evidence IDs | Research Question | Why Local Evidence Is Not Enough | Priority |
|---|---|---|---|---|---|
| `RQ-01` | Host-system integration risk | `DOC-03`, `MAP-07`, `MAP-08`, `REAL-07` | Do stronger workflow systems preserve prior attempt history and evidence when rerunning failed work from the same logical snapshot? | Local evidence proves Proposal 013 is internally coherent, but external workflow history practice is useful to validate the same-snapshot retry model. | High |
| `RQ-02` | Unresolved tradeoff | `DOC-04`, `MAP-07`, `REAL-08` | Should validation failure evidence be persisted as a first-class result object rather than only a summary state or derived metric? | Local evidence shows where persistence happens, but external validation-system practice helps justify what should remain canonical and durable. | High |
| `RQ-03` | Unresolved tradeoff | `DOC-01`, `REAL-08` | Should output-contract and post-generation validation mismatch default to operator-mediated or non-auto-retryable handling rather than blind transient retry? | Local draft prefers narrow recovery, but external failure semantics help calibrate the right default classification. | Medium |
| `RQ-04` | Proposal gap | `MAP-04`, `REAL-05`, `REAL-10` | For `backend_profiles.*.structured_output`, should unsupported schema/capability combinations fail closed before execution, and should post-generation validation still remain mandatory even when transport support exists? | Local evidence proves the current transport gap, but external provider docs are needed to avoid overclaiming what "structured output support" actually guarantees. | High |

## 2. Source Ledger

| Source ID | Title | Publisher / Authority | URL or Reference | Published Date | Last Updated Date | Accessed / Verified Date | Why This Source Matters | Temporal Volatility / Freshness Risk | Confidence |
|---|---|---|---|---|---|---|---|---|---|
| `SRC-01` | Re-running workflows and jobs | GitHub Docs | [https://docs.github.com/en/actions/how-tos/manage-workflow-runs/re-run-workflows-and-jobs](https://docs.github.com/en/actions/how-tos/manage-workflow-runs/re-run-workflows-and-jobs) | Not stated | Not stated | 2026-03-29 | Gives a current mainstream rerun model with preserved run history and same source revision semantics. | Low; product docs may evolve, but rerun semantics are usually stable. | High |
| `SRC-02` | Event History | Temporal Platform Documentation | [https://docs.temporal.io/encyclopedia/event-history](https://docs.temporal.io/encyclopedia/event-history) | Not stated | Not stated | 2026-03-29 | Primary durable-execution reference for immutable event history, replay, and state reconstruction after failure. | Low; core platform concept. | High |
| `SRC-03` | Temporal Failures reference | Temporal Platform Documentation | [https://docs.temporal.io/references/failures](https://docs.temporal.io/references/failures) | Not stated | Not stated | 2026-03-29 | Primary reference for retryable vs non-retryable failures, attempt tracking, and terminal failed-state semantics. | Medium; exact retry semantics can evolve across features, but the failure model is core reference material. | High |
| `SRC-04` | Configure Validation Result Stores | Great Expectations OSS Docs 0.18.21 | [https://docs.greatexpectations.io/docs/0.18/oss/guides/setup/configuring_metadata_stores/configure_result_stores/](https://docs.greatexpectations.io/docs/0.18/oss/guides/setup/configuring_metadata_stores/configure_result_stores/) | Not stated | Not stated | 2026-03-29 | Shows a durable validation-result store pattern and explicitly warns that validation results can contain sensitive or regulated data. | Medium-High; official but legacy-versioned and no longer actively maintained. | Medium |
| `SRC-05` | Structured model outputs | OpenAI API Docs | [https://developers.openai.com/api/docs/guides/structured-outputs](https://developers.openai.com/api/docs/guides/structured-outputs) | Not stated | Not stated | 2026-03-29 | Official guidance that structured outputs use explicit strict schema contracts, support only part of JSON Schema, and should be backed by evals. | Medium-High; provider docs evolve with models and schema support. | High |
| `SRC-06` | Structured outputs | Google AI for Developers | [https://ai.google.dev/gemini-api/docs/structured-output](https://ai.google.dev/gemini-api/docs/structured-output) | Not stated | 2026-02-26 | 2026-03-29 | Official provider guidance that unsupported schema features may be ignored, large schemas may be rejected, and app-side validation remains necessary. | High; capability details change with model/platform releases. | High |

## 3. Findings by Theme

### Architecture / State / Concurrency / Offline / Sync Patterns

- Finding ID: `FIND-ARCH-01`
  - Research question IDs: `RQ-01`
  - Source IDs: `SRC-01`, `SRC-02`, `SRC-03`
  - Source-backed finding:
    - GitHub reruns keep the original source revision while preserving previous run attempts as inspectable history.
    - Temporal models recovery through durable event history rather than overwriting prior truth.
    - Temporal failure tracking also keeps attempt count and last-attempt failure details alongside retry semantics.
  - Model inference / host-system note:
    - Proposal 013 is aligned with stronger host-system practice when same-run `Retry Failed Agent` keeps the same frozen logical snapshot and appends inspectable lineage instead of mutating earlier attempt evidence.
    - The existing proposal wording already adopts the core pattern correctly.
  - Host-system surface touched: `StageRetryCoordinator`, `StageAttemptHistoryRecord`, `AgentAttemptHistoryRecord`, `ArtifactStorage`, regression proof sections
  - Time-sensitive: `No`
  - Confidence: `High`

- Finding ID: `FIND-ARCH-02`
  - Research question IDs: `RQ-04`
  - Source IDs: `SRC-05`, `SRC-06`
  - Source-backed finding:
    - Structured-output providers do not expose a uniform guarantee surface. OpenAI documents explicit strict schema mode but also notes only part of JSON Schema is supported; Gemini says unsupported properties may be ignored and large schemas may be rejected.
    - Gemini also says syntactically valid structured output still needs application-side validation before use.
  - Model inference / host-system note:
    - Proposal 013 should not define `StructuredOutputSchemaGate` as a simple boolean "provider supports structured output."
    - The useful pattern is provider-aware preflight compatibility checking plus continued post-generation validation inside `WorkflowOrchestrator.validateStructuredOutputs(...)`.
  - Host-system surface touched: `StructuredOutputSchemaGate`, `GooseSessionBridge`, `GooseTransport`, `WorkflowOrchestrator.validateStructuredOutputs(...)`, Appendix `B` Tier `1`
  - Time-sensitive: `Yes`
  - Confidence: `High`

### Testing Strategy

- Finding ID: `FIND-TEST-01`
  - Research question IDs: `RQ-04`
  - Source IDs: `SRC-05`, `SRC-06`
  - Source-backed finding:
    - OpenAI explicitly recommends using evals to determine whether a schema works well for the use case.
    - Gemini explicitly recommends final application-side validation even when the transport returns schema-shaped JSON.
  - Model inference / host-system note:
    - Proposal 013’s `10.1` proof should keep provider-subset and semantic-validation cases separate:
      - preflight rejection when the active backend or schema subset is unsupported
      - post-generation validation failure when transport accepted the request but the content still violates business or contract rules
  - Host-system surface touched: `Section 10.1`, contract-validation tests, structured-output schema gate tests
  - Time-sensitive: `Yes`
  - Confidence: `High`

### Consumer-Finance Trust / Transparency / Recovery

- Finding ID: `FIND-TRUST-01`
  - Research question IDs: `RQ-02`
  - Source IDs: `SRC-03`, `SRC-04`
  - Source-backed finding:
    - Great Expectations treats validation results as durable stored objects, not just ephemeral summary flags, and warns that those results can include sensitive or regulated data.
    - Temporal failure records also preserve failure details as inspectable structured state rather than collapsing everything into a generic status summary.
  - Model inference / host-system note:
    - Proposal 013 is strongest when `ValidationFailureRecord` or the failed-stage evidence packet remains the canonical durable record, while report/export/recovery surfaces reference it by stable identifier and present redacted or summarized views by default.
    - This is especially relevant for Chainworks because raw outputs, transcripts, and receipts may later include sensitive user or provider material.
  - Host-system surface touched: `ValidationFailureRecord`, `FailedStageEvidencePanel`, `RunReportBuilder`, export surfaces
  - Time-sensitive: `Yes`, because the source pattern is stable but the cited Great Expectations doc is legacy-versioned
  - Confidence: `Medium`

- Finding ID: `FIND-TRUST-02`
  - Research question IDs: `RQ-03`
  - Source IDs: `SRC-03`, `SRC-04`
  - Source-backed finding:
    - Temporal distinguishes retryable from non-retryable failures and treats non-retryable failures as terminal until explicit intervention.
    - Durable validation-result systems preserve the failed result rather than acting as if no meaningful output exists.
  - Model inference / host-system note:
    - Proposal 013’s default of treating output-contract mismatch and post-generation validation failure as non-auto-retryable is externally well grounded.
    - Chainworks should keep narrow retry available, but as an explicit recovery action or policy override after the operator can inspect the evidence.
  - Host-system surface touched: `RunRecoveryPolicy`, `RecoveryActionSnapshot`, `RecoverySheet`, `BlockedRunRecoveryView`
  - Time-sensitive: `No`
  - Confidence: `High`

## 4. Host-System Applicability Matrix

| Insight ID | Source IDs | Classification (`Adopt | Adapt | Watch | Reject`) | Proposal Area Affected | Host-System Surface Touched | Why It Applies or Does Not Apply | Concrete Recommended Change |
|---|---|---|---|---|---|---|
| `APP-01` | `SRC-01`, `SRC-02`, `SRC-03` | `Adopt` | `Sections 5.2`, `5.4`, `10.2`, `10.3`, `11` | retry lineage, stage/agent attempt history, artifact storage | External systems preserve prior attempts while keeping the same logical work context. This matches the motivating failure class directly. | Keep the current same-snapshot lineage model and verify inspectability of prior attempt artifacts in implementation proof. |
| `APP-02` | `SRC-03`, `SRC-04` | `Adopt` | `Sections 6.2`, `6.3`, `7.3`, `11.7` | validation-failure storage, report/export/reference truth | Durable validation or failure records are more trustworthy than summary-only status. This maps directly onto Proposal 013’s operator-trust goal. | Keep `ValidationFailureRecord` or failed-stage packet as canonical persisted evidence and reference it directly from recovery/report/export surfaces. |
| `APP-03` | `SRC-03`, `SRC-04` | `Adapt` | `Sections 5.4`, `7.2`, `11` | recovery policy and retry defaults | The permanent-vs-transient distinction is useful, but Chainworks still needs explicit operator-driven retry as a recovery action for some content failures. | Keep non-auto-retryable-by-default semantics, but allow explicit narrow retry or policy override after evidence inspection. |
| `APP-04` | `SRC-05`, `SRC-06` | `Adopt` | `Section 4.2.2`, Layer `Q`, `10.1`, `11.9`, Appendix `B` Tier `1` | `StructuredOutputSchemaGate`, transport bridge, post-generation validation | Provider docs show schema support is partial and non-uniform. Silent no-op or partial ignore is possible if the app does not gate capability per backend/schema combination. | Define `StructuredOutputSchemaGate` as provider-aware preflight compatibility plus continued post-generation validation, not as a generic yes/no flag. |
| `APP-05` | `SRC-04` | `Adapt` | `Sections 6.3`, `7.3`, export/report decisions | failure evidence panel, reports, exports | Durable validation results are useful, but the cited source warns they may contain sensitive or regulated data. Chainworks should not equate canonical storage with broad inline display. | Add a note that reports/exports reference canonical failure evidence by ID and default to redacted summaries unless explicit full-detail inspection is requested. |
| `APP-06` | `SRC-01`, `SRC-04` | `Reject` | recovery UX wording only | operator shell and failure UI | The truth model is reusable, but GitHub run-history UI and Great Expectations store taxonomy are not direct UX templates for Chainworks. | Borrow semantics, not literal controls or terminology. |

## 5. Proposal Deltas / Recommended Updates

| Delta ID | Proposal Section / Decision | Recommended Update | Why It Helps | Supporting Source IDs | Supporting Local Evidence IDs | Priority |
|---|---|---|---|---|---|---|
| `DELTA-01` | `Sections 5.2`, `5.4`, `10.2`, `10.3`, `11` | No new wording change required. Keep the current same-snapshot retry lineage language and verify it in implementation proof exactly as written. | External rerun/history systems still support the current immutable-lineage direction. | `SRC-01`, `SRC-02`, `SRC-03` | `REAL-07` | Low |
| `DELTA-02` | `Sections 5.4`, `7.2`, `11` | No new wording change required. Keep non-auto-retryable-by-default handling for contract mismatch and post-generation validation failure. | External failure semantics still support explicit operator-mediated recovery for this failure class. | `SRC-03`, `SRC-04` | `REAL-08` | Low |
| `DELTA-03` | `Section 4.2.2`, Layer `Q`, `10.1`, `11.9`, Appendix `B` Tier `1` | Add one explicit sentence that `StructuredOutputSchemaGate` is provider-aware: unsupported backend/schema subset combinations must fail in preflight, and successful transport-level structured output does not remove the need for post-generation contract validation. | Prevents Proposal 013 from overclaiming what provider-level structured-output support guarantees and keeps Tier `1` honest. | `SRC-05`, `SRC-06` | `MAP-04`, `REAL-05`, `REAL-10` | Medium |
| `DELTA-04` | `Sections 6.3`, `7.3`, export/report guidance | Add one explicit rule that canonical failure evidence may contain sensitive data, so reports and exports should reference the canonical record and default to summarized or redacted presentation. | Makes the failure-evidence hardening safer for consumer-finance workflows without changing the core proposal scope. | `SRC-04` | `REAL-08` | Medium |

## 6. Freshness Risks / Recheck Triggers

| Trigger ID | Claim / Recommendation | Why It Is Time-Sensitive | What Must Be Rechecked | Recheck Trigger / Window | Source IDs |
|---|---|---|---|---|---|
| `FRESH-01` | Provider-aware structured-output gate should fail unsupported combinations before execution | Provider schema support evolves quickly and can differ by model family | OpenAI and Gemini structured-output docs, plus active backend capabilities in the repo | Recheck before implementation and again if provider families or model bindings change | `SRC-05`, `SRC-06` |
| `FRESH-02` | App-side semantic validation remains mandatory even when provider transport accepts structured output | Providers may change what they validate server-side versus what they leave to clients | Structured-output docs and any new transport-layer guarantees | Recheck before implementing `StructuredOutputSchemaGate` and validation tests | `SRC-05`, `SRC-06` |
| `FRESH-03` | Great Expectations-style validation-result storage is a useful pattern, but not a naming contract | The cited GX docs are legacy-versioned and explicitly no longer actively maintained | Whether the team wants terminology, storage taxonomy, or only the durability pattern | Recheck only if later proposals borrow exact class/store names | `SRC-04` |
| `FRESH-04` | Same-snapshot retry lineage and canonical failure evidence remain the right external framing for Proposal 013 | Low volatility, but future scope expansion could change applicability | Whether Proposal 013 remains a bounded retry/evidence hardening slice rather than a broader history/analytics redesign | Recheck if Appendix `B` or reporting scope expands materially | `SRC-01`, `SRC-02`, `SRC-03`, `SRC-04` |

## 7. Remaining Open Questions

- `QUESTION-01`: Should Proposal 013 make the redacted-summary-versus-full-evidence display rule explicit now, or leave that as an implementation-detail decision inside the later audit?
- `QUESTION-02`: Does the repo eventually need a provider capability matrix artifact for `structured_output` support, so Appendix `B` does not become the long-term home for backend-specific schema compatibility rules?
