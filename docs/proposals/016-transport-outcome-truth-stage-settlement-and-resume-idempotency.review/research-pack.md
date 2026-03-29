# Proposal Research Pack

## 0. Review Target and Local Context Consumed
- Proposal: `docs/proposals/016-transport-outcome-truth-stage-settlement-and-resume-idempotency.md`
- Research round: `R1`
- Proposal evidence pack used: `docs/reviews/016-transport-outcome-truth-stage-settlement-and-resume-idempotency-evidence-pack.md`
- Current-system baseline used: `.review-baselines/current-system-baseline.md`
- Proposal-specific integration context used: none present
- Existing research pack reused: none; this is the first `P016` research pack
- Adjacent docs consumed:
  - `docs/reference/current-system-baseline.md`
  - `docs/reference/runtime-contract.md`
  - `docs/reference/workflow-execution-engine.md`
  - `docs/reference/operator-experience.md`
  - `docs/reference/provider-binding-truth.md`
  - `docs/reference/run-control.md`
- Current code / module mapping consumed:
  - `Chainworks Forge/Models/Run.swift`
  - `Chainworks Forge/Models/StageExecution.swift`
  - `Chainworks Forge/Models/AgentExecution.swift`
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift`
  - `Chainworks Forge/Engine/ResumeManager.swift`
  - `Chainworks Forge/Engine/RecoveryCoordinator.swift`
  - `Chainworks Forge/Engine/RunReportBuilder.swift`
  - `Chainworks Forge/Engine/RunCancellationCoordinator.swift`
  - `Chainworks Forge/Engine/ExecutionReceiptBuilder.swift`
  - `Chainworks Forge/Providers/ProviderExecutionReceipt.swift`
  - `Chainworks Forge/Providers/UsageReceiptNormalizer.swift`
  - `Chainworks Forge/Engine/StageRetryCoordinator.swift`
- Local evidence IDs that triggered research: `DOC-01`, `MAP-05`, `MAP-06`, `MAP-09`, `MAP-10`, `REAL-02`, `REAL-04`, `RSH-01`, `RSH-02`, `RSH-03`
- Notes on baseline freshness or local contradictions:
  - baseline remained fresh for repo topology and run-control ownership
  - targeted local refresh already confirmed the new `limit_exhausted_*` delta is grounded in current receipt seams
  - research is confirmatory and scope-sharpening, not rescue work for a red draft

## 1. Research Questions Derived from Local Evidence
| Question ID | Derived From (`Proposal gap | Baseline constraint | Host-system integration risk | Unresolved tradeoff`) | Local Evidence IDs | Research Question | Why Local Evidence Is Not Enough | Priority |
|---|---|---|---|---|---|
| RQ-01 | Host-system integration risk | `DOC-01`, `MAP-10`, `REAL-04`, `RSH-01` | Across official provider docs, which finish / stop reasons and limit-exhaustion signals can terminate generation after partial output, and do neutral markers like `stop` or stream closure ever justify success on their own? | Local code shows where this truth should persist, but not the latest official provider semantics the proposal must normalize. | High |
| RQ-02 | Unresolved tradeoff | `DOC-01`, `MAP-05`, `MAP-06`, `MAP-09`, `RSH-02` | What do primary workflow-orchestration docs recommend for durable settlement and resume idempotency so restart / replay does not duplicate work or rewrite terminal truth after cancellation, timeout, or transport interruption? | Local seams show current ownership, but external durable-execution guidance is useful to validate the proposal’s prevention-vs-repair ordering. | High |
| RQ-03 | Unresolved tradeoff | `DOC-01`, `REAL-02`, `REAL-04`, `RSH-03` | In primary technical guidance, should canonical outcome fields stay separate from raw receipts / transport envelopes, and if so, what should the raw payload be used for? | Local review concluded the draft’s flattened-column model is coherent; external confirmation helps strengthen that decision and bound future drift. | Medium |

## 2. Source Ledger
| Source ID | Title | Publisher / Authority | URL or Reference | Published Date | Last Updated Date | Accessed / Verified Date | Why This Source Matters | Temporal Volatility / Freshness Risk | Confidence |
|---|---|---|---|---|---|---|---|---|---|
| SRC-01 | Handling stop reasons | Anthropic | https://docs.anthropic.com/en/api/handling-stop-reasons | Not stated | Not stated | 2026-03-29 | Official stop-reason taxonomy including truncation and neutral finish markers. | Medium: provider enums may expand. | High |
| SRC-02 | Rate limits | Anthropic | https://docs.anthropic.com/en/api/rate-limits | Not stated | Not stated | 2026-03-29 | Official distinction between rate-limit errors and successful responses with stop reasons. | Medium. | High |
| SRC-03 | GenerateContentResponse / FinishReason | Google AI for Developers | https://ai.google.dev/api/generate-content | Not stated | Not stated | 2026-03-29 | Official finish-reason taxonomy for Gemini candidates, including `STOP`, `MAX_TOKENS`, and safety/policy cases. | Medium: enums may evolve. | High |
| SRC-04 | Configure safety filters | Google Cloud Vertex AI | https://cloud.google.com/vertex-ai/generative-ai/docs/multimodal/configure-safety-filters | Not stated | Not stated | 2026-03-29 | Official explanation of blocked responses and when content is voided rather than treated as ordinary completion. | Medium. | High |
| SRC-05 | API Overview | OpenAI | https://developers.openai.com/api/reference/overview | Not stated | Not stated | 2026-03-29 | Official API reference explaining request IDs and rate-limit headers as diagnostic metadata separate from response semantics. | Medium. | High |
| SRC-06 | Temporal Docs | Temporal | https://docs.temporal.io/ | Not stated | Not stated | 2026-03-29 | Official durable-execution positioning: state recovery and resume from the last completed step. | Low. | High |
| SRC-07 | Crafting an error handling strategy | Temporal | https://learn.temporal.io/assets/files/crafting-an-error-handling-strategy-dotnet-replay2025-e633cef41481baac8b25a636533c4d37.pdf | 2025 | Not stated | 2026-03-29 | Official workshop material on retryable vs non-retryable failures and when manual intervention is required. | Low. | High |
| SRC-08 | Temporal 102 with Java | Temporal | https://learn.temporal.io/assets/files/temporal-102-with-java-replay2025-468b8109d5a9ce33b36c2910c48433d9.pdf | 2025 | Not stated | 2026-03-29 | Official workshop material on idempotent activities, default activity retries, and durable event history. | Low. | High |

## 3. Findings by Theme

### Architecture / State / Concurrency / Offline / Sync Patterns
- Finding ID: `FIND-ARC-01`
  Research question IDs: `RQ-02`
  Source IDs: `SRC-06`, `SRC-08`
  Source-backed finding: Temporal positions durable execution as automatic state recovery where execution resumes from the last completed step, and it explicitly says Activities should be idempotent because failed Activities may be retried, with the default retry policy continuing until success or cancellation.
  Model inference / host-system note: This supports Proposal 016’s current ordering: durable settlement truth and create-path guards must be the primary prevention boundary, while startup repair remains secondary cleanup. It also supports keeping stop / timeout / cancellation outcomes as persisted settlement truth rather than recomputing them from replay side effects.
  Host-system surface touched: `WorkflowOrchestrator`, `ResumeManager`, `RunCancellationCoordinator`, `StageExecution`
  Time-sensitive: `No`
  Confidence: `High`

- Finding ID: `FIND-ARC-02`
  Research question IDs: `RQ-02`
  Source IDs: `SRC-07`, `SRC-08`
  Source-backed finding: Temporal’s official error-handling guidance distinguishes retryable failures from non-retryable / permanent failures and recommends explicit non-retryable classification when repeating the same operation will not change the outcome. Its workshop materials also treat cancellation as a terminal boundary for automatic retries.
  Model inference / host-system note: Provider/app limit exhaustion, quota exhaustion, policy blocks, and ambiguous neutral-stop cases should not default to blind automatic retry. Proposal 016 is stronger if these classes are framed as explicit settlement plus operator-mediated or policy-mediated next action, not “just retry again.”
  Host-system surface touched: `AgentExecution`, `StageRetryCoordinator`, `RecoveryCoordinator`
  Time-sensitive: `No`
  Confidence: `High`

### Consumer-Finance Trust / Transparency / Recovery
- Finding ID: `FIND-TRUST-01`
  Research question IDs: `RQ-01`, `RQ-03`
  Source IDs: `SRC-01`, `SRC-02`, `SRC-03`, `SRC-04`
  Source-backed finding: Official provider docs do not treat neutral transport markers as universally equivalent to business success. Anthropic distinguishes neutral and truncation stop reasons such as `end_turn`, `stop_sequence`, `max_tokens`, and `model_context_window_exceeded`; Google’s Gemini docs distinguish `STOP`, `MAX_TOKENS`, `SAFETY`, `RECITATION`, `BLOCKLIST`, `PROHIBITED_CONTENT`, `SPII`, and other finish reasons, and explicitly document cases where blocked responses return no candidate content.
  Model inference / host-system note: Proposal 016’s choice to keep `providerStopReason` first-class and to forbid inferring success from `Finish: stop` or stream closure alone is aligned with modern provider behavior. The host system should preserve partial output plus the stop reason, then let canonical outcome determine whether the attempt is successful, exhausted, cancelled, or failed.
  Host-system surface touched: `AgentExecution`, `ProviderExecutionReceipt`, `ExecutionReceiptBuilder`, `RunReportBuilder`
  Time-sensitive: `Yes`
  Confidence: `High`

- Finding ID: `FIND-TRUST-02`
  Research question IDs: `RQ-03`
  Source IDs: `SRC-01`, `SRC-03`, `SRC-05`
  Source-backed finding: Official APIs expose structured terminal or finish semantics separately from broader diagnostic payloads. Anthropic returns `stop_reason` and `usage`; Gemini returns `finishReason`, `finishMessage`, `safetyRatings`, and `usageMetadata`; OpenAI documents request IDs and rate-limit headers as separate debugging/operational metadata.
  Model inference / host-system note: This matches Proposal 016’s flattened canonical columns plus diagnostic-envelope model. Raw provider receipts, headers, and transcripts should remain inspectable evidence, but they should not outrank persisted canonical outcome fields in report/recovery readers.
  Host-system surface touched: `AgentExecution`, `RunReportBuilder`, `RecoveryCoordinator`, export/evidence surfaces
  Time-sensitive: `Yes`
  Confidence: `High`

### Testing Strategy
- Finding ID: `FIND-TEST-01`
  Research question IDs: `RQ-01`, `RQ-02`
  Source IDs: `SRC-01`, `SRC-03`, `SRC-07`, `SRC-08`
  Source-backed finding: Provider semantics and durable-execution guidance both imply that the highest-risk regressions are classification regressions, not only transport failures. The critical proof cases are truncation-after-output, blocked/empty output, cancellation after durable output, and replay/resume after a terminal settlement write.
  Model inference / host-system note: Proposal 016’s existing verification sections are directionally right. If the draft changes again, the highest-value additions are proof assertions that neutral stop plus partial output does not silently become success, and that replay after a settled attempt does not emit a second conflicting settlement record.
  Host-system surface touched: `Proposal016` unit/integration suite, report/recovery readers
  Time-sensitive: `Medium`
  Confidence: `High`

## 4. Host-System Applicability Matrix
| Insight ID | Source IDs | Classification (`Adopt | Adapt | Watch | Reject`) | Proposal Area Affected | Host-System Surface Touched | Why It Applies or Does Not Apply | Concrete Recommended Change |
|---|---|---|---|---|---|---|
| APP-01 | `SRC-01`, `SRC-03` | Adopt | Section `4.2`, Section `7.5` | `AgentExecution`, receipt normalization, recovery/report readers | Official provider docs clearly distinguish neutral finish from truncation, safety, and policy stops. | Keep `providerStopReason` first-class and state explicitly that neutral markers alone never prove success. |
| APP-02 | `SRC-02`, `SRC-07` | Adopt | Section `4.2`, Section `5`, Section `7` | retry policy, recovery next actions | Rate-limit / quota conditions and permanent policy failures should not be treated like generic retryable transport blips. | State that limit exhaustion and policy blocks are non-auto-retryable by default unless a narrower policy override exists. |
| APP-03 | `SRC-06`, `SRC-08` | Adopt | Section `5`, Section `7.3`, Section `7.4` | `WorkflowOrchestrator`, `ResumeManager`, create-path guards | Durable execution guidance strongly favors preventing duplicate side effects through idempotent boundaries, not discovering them later via repair. | Preserve the current proposal ordering: settle durably first, guard create paths second, run startup repair only as cleanup. |
| APP-04 | `SRC-01`, `SRC-03`, `SRC-05` | Adopt | Section `4.3`, Section `6.3` | `AgentExecution`, export/report readers | Official APIs expose canonical outcome-like fields separately from diagnostic payloads. | Keep flattened persisted fields canonical and keep envelopes/headers/receipts diagnostic-only. |
| APP-05 | `SRC-05` | Watch | Section `4.3` | provider receipt/debug metadata | OpenAI’s docs surface diagnostic headers cleanly, but response-completion docs around `incomplete`/`incomplete_details` were not the most stable reference surface in this round. | Recheck OpenAI response-status docs during implementation if OpenAI-native classification becomes a first-class provider path in the runtime. |

## 5. Proposal Deltas / Recommended Updates
| Delta ID | Proposal Section / Decision | Recommended Update | Why It Helps | Supporting Source IDs | Supporting Local Evidence IDs | Priority |
|---|---|---|---|---|---|---|
| DELTA-01 | Section `4.2` outcome taxonomy notes | Add one explicit sentence that neutral finish markers describe transport termination only; success still requires explicit success criteria plus durable output semantics. | Removes any future temptation to treat `stop` or stream closure as implicit success during implementation drift. | `SRC-01`, `SRC-03` | `DOC-01`, `MAP-10`, `REAL-04` | Medium |
| DELTA-02 | Section `4.3` persisted storage contract | Add one explicit sentence that raw receipts, headers, transcripts, and diagnostic envelopes may explain an outcome but may never contradict or outrank the persisted canonical columns. | Strengthens the already-correct single-authority model and protects report/export surfaces from envelope-first regressions. | `SRC-01`, `SRC-03`, `SRC-05` | `REAL-02`, `REAL-04` | Medium |
| DELTA-03 | Section `7.5` reconciliation table / recovery guidance | Add one sentence that limit exhaustion, policy blocks, and other permanent/provider-defined terminal stops are non-auto-retryable by default and require explicit policy or operator intent to re-enter execution. | Aligns retry posture with official orchestration guidance for non-retryable failures. | `SRC-02`, `SRC-07` | `MAP-06`, `MAP-09`, `MAP-11` | Medium |

## 6. Freshness Risks / Recheck Triggers
| Trigger ID | Claim / Recommendation | Why It Is Time-Sensitive | What Must Be Rechecked | Recheck Trigger / Window | Source IDs |
|---|---|---|---|---|---|
| FRESH-01 | Provider finish / stop taxonomies support `limit_exhausted_*` and neutral-stop separation | Providers can add or rename finish reasons and policy-block enums. | Anthropic stop reasons, Gemini `FinishReason`, any provider-specific stop-reason mapping table used in implementation. | Recheck at implementation start and before shipping a new provider family. | `SRC-01`, `SRC-03`, `SRC-04` |
| FRESH-02 | OpenAI diagnostic separation remains consistent with a flattened canonical outcome model | OpenAI docs structure is evolving rapidly and page URLs moved during this round. | Response-status and incomplete-response docs if OpenAI-native classification is implemented directly. | Recheck during implementation if OpenAI path becomes first-class. | `SRC-05` |

## 7. Remaining Open Questions
- QUESTION-01: If the host system eventually supports provider-specific adaptive retry policies, should `limit_exhausted_after_output` remain strictly terminal, or should the runtime allow a constrained same-run continuation policy for providers that support quota recovery within the same logical attempt?
