# Proposal Evidence Pack

## A. Repo-Local Proposal / Document Inventory
| Evidence ID | Source / Path / Artifact | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|
| DOC-01 | `docs/proposals/016-transport-outcome-truth-stage-settlement-and-resume-idempotency.md` | 2026-03-29 | High | Updated draft keeps the earlier closed blockers fixed and now adds explicit provider/app limit-exhaustion and neutral-stop handling. | Review could miss whether the new delta reopens second-authority or grounding issues. | Primary document under review. |
| DOC-02 | `.review-baselines/current-system-baseline.md` | 2026-03-29 | High | Reusable baseline already positions run control, runtime contract, operator shell, provider truth, and delivery/sign-off as stable reference areas. | Review could reopen already-stabilized repo seams unnecessarily. | Primary review accelerator. |
| DOC-03 | `docs/reference/current-system-baseline.md` | 2026-03-29 | High | Current host system already has stable run-control, recovery, report, and provider-truth seams that Proposal 016 must extend rather than replace. | Could misjudge whether the proposal is additive or duplicative. | Cross-check for baseline reuse. |
| DOC-04 | `docs/reference/runtime-contract.md` | 2026-03-29 | High | Runtime contract still requires immutable artifacts per stage attempt and separate state machines, including `cancelled` agent executions. | Could miss whether cancellation and retry remain grounded in current truth. | Anchors execution-truth scope. |
| DOC-05 | `docs/reference/workflow-execution-engine.md` | 2026-03-29 | High | Current engine still creates one `StageExecution` per state and persists artifacts under stage-attempt-scoped paths. | Could misjudge whether the aggregate model remains additive instead of parallel. | Anchors stage/aggregate ownership. |
| DOC-06 | `docs/reference/operator-experience.md` | 2026-03-29 | High | Recovery and blocked-run explanation already live in shell-owned surfaces. | Could misread proposal wording as a new top-level UI surface. | Anchors recovery/report ownership. |
| DOC-07 | `docs/reference/provider-binding-truth.md` | 2026-03-29 | High | Frozen binding snapshot, frozen provenance, and `unverifiable` downgrade rules are already stable reference truth. | Could treat provider-truth migration as unconstrained. | Anchors Layer `U`. |
| DOC-08 | `docs/reference/run-control.md` | 2026-03-29 | High | Cancellation settlement is already stable repo truth, and active agent executions legitimately settle to `.cancelled`. | Could miss whether the updated taxonomy still honors that truth. | Anchors transport-outcome completeness. |
| DOC-09 | prior review: `docs/reviews/016-transport-outcome-truth-stage-settlement-and-resume-idempotency-review.md` | 2026-03-29 | High | Prior green round already closed cancellation, outcome-owner, and aggregate-authority blockers. | Could lose track of whether the new limit-exhaustion delta regressed the draft. | Supports delta analysis. |
| DOC-10 | prior evidence pack: `docs/reviews/016-transport-outcome-truth-stage-settlement-and-resume-idempotency-evidence-pack.md` | 2026-03-29 | High | Prior evidence already mapped the relevant runtime seams and contradictions. | Could duplicate work or drift from established repo truth. | Supports reuse-after-freshness-check. |
| DOC-11 | proposal-local research pack: `docs/proposals/016-transport-outcome-truth-stage-settlement-and-resume-idempotency.review/research-pack.md` | 2026-03-29 | High | Existing bounded research already confirmed neutral-stop semantics, non-auto-retryable limit/policy stop guidance, and flattened-columns-plus-diagnostic-envelope ownership. | Could miss whether the fresh proposal edits actually incorporated the last research-backed deltas. | Supports reuse of prior research without another web pass. |

## B. Reusable Baseline Inputs
| Evidence ID | Artifact / Slice | Status (`Reused | Partially refreshed | Missing`) | Covered Surfaces | Verified On | Confidence | Freshness Notes | Relevance |
|---|---|---|---|---|---|---|---|
| BASE-01 | `.review-baselines/current-system-baseline.md` | Reused | repo-level architecture, shell ownership, stable references | 2026-03-29 | High | Fresh for repo-level topology and proposal dependency orientation. | Replaces any need for a broad repo remap. |
| BASE-02 | targeted refresh over transport / settlement / repair seams | Partially refreshed | limit exhaustion, neutral-stop handling, cancellation taxonomy, outcome-owner model, aggregate authority, guard placement | 2026-03-29 | High | Needed because the repo-level baseline does not enumerate P016-specific seams in detail. | Supports a defensible proposal-readiness call. |
| BASE-03 | `<proposal>.review/integration-context.md` | Missing | proposal-local narrow context slice | 2026-03-29 | High | Still absent, but not blocking because the targeted refresh stayed narrow. | Optional future accelerator. |

## C. Scope, Out-of-Scope, and Intentional Deferrals

- Round classification: fresh delta round; the prior green verdict was revalidated against research-backed edits over the limit-exhaustion / policy-stop slice.
- In scope:
  - agent terminal-outcome taxonomy and persistence
  - stage-settlement ownership and aggregate-settlement truth
  - startup repair and resume idempotency ownership
  - provider/runtime binding truth migration over existing frozen truth
  - report/recovery alignment to canonical settlement records
  - provider/app limit-exhaustion truth and neutral-stop handling
- Out of scope:
  - build/run attempts
  - implementation audit
  - product/KPI overlay
  - broad UI polish
- Deferred intentionally:
  - proposal-local integration-context artifact
  - runtime replay proof
- Main result:
  - the fresh edits incorporate the previously recommended neutral-stop, canonical-owner, and non-auto-retryable policy-stop clarifications without reopening any closed blockers

## D. Impacted Modules / Code-Path Map
| Evidence ID | Module / Surface | Current Role | Verified On | Confidence | Why It Matters |
|---|---|---|---|---|---|
| MAP-01 | `Chainworks Forge/Models/Run.swift` | current run-level truth already includes `runtimeTrustLevel`, `providerBindingSnapshotJSON`, `bindingProvenanceJSON`, and cancellation settlement fields | 2026-03-29 | High | Proposal 016 continues to extend, not replace, these run-level truth seams. |
| MAP-02 | `Chainworks Forge/Models/StageExecution.swift` | current stage model already owns retry lineage, validation failure, evidence packet, and recovery snapshot | 2026-03-29 | High | The aggregate rule still keeps stage terminality canonical. |
| MAP-03 | `Chainworks Forge/Models/AgentExecution.swift` | current agent model already owns receipt JSON, validation failure, output envelopes, retry lineage, and resolved runtime-ish fields | 2026-03-29 | High | The new limit-exhaustion fields remain inside the same flattened outcome-owner model. |
| MAP-04 | `Chainworks Forge/Models/Approval.swift` | approval is currently minimal: `stageID`, timestamps, decision, comment, expiry | 2026-03-29 | High | Proposal 016 still correctly names a real approval-lineage persistence delta. |
| MAP-05 | `Chainworks Forge/Engine/WorkflowOrchestrator.swift` | current orchestrator owns state entry, approval creation, output persistence triggers, validation invocation, and stage status mutation | 2026-03-29 | High | The updated draft still makes create-path guard placement explicit around these mutation points. |
| MAP-06 | `Chainworks Forge/Engine/ResumeManager.swift` | current startup classification uses drift, compiler version, side-effect heuristics, and frozen workspace validation | 2026-03-29 | High | Proposal 016 still positions startup repair as secondary cleanup rather than the primary prevention mechanism. |
| MAP-07 | `Chainworks Forge/Engine/RecoveryCoordinator.swift` | recovery actions and context already derive from stage status, evidence packets, and recovery snapshots | 2026-03-29 | High | Proposal 016 still aligns with existing shell-owned recovery readers. |
| MAP-08 | `Chainworks Forge/Engine/RunReportBuilder.swift` | report builder already reads stage evidence packets, recovery snapshots, frozen bindings, frozen provenance, and runtime trust | 2026-03-29 | High | The updated read-order rules remain aligned with these existing consumers. |
| MAP-09 | `Chainworks Forge/Engine/RunCancellationCoordinator.swift` | cancellation already settles active agent executions to `.cancelled` through a stable two-phase contract | 2026-03-29 | High | Proposal 016 still explicitly bridges that truth into the new taxonomy. |
| MAP-10 | `Chainworks Forge/Engine/ExecutionReceiptBuilder.swift` + `Providers/ProviderExecutionReceipt.swift` + `Providers/UsageReceiptNormalizer.swift` | current repo already has two receipt/evidence seams: structured execution receipt artifacts and normalized provider receipt JSON | 2026-03-29 | High | The new limit-exhaustion slice is grounded in real current receipt/transport seams rather than invented abstractions. |
| MAP-11 | `Chainworks Forge/Engine/StageRetryCoordinator.swift` | same-run retry already distinguishes agent retry vs. stage retry using stage/agent lineage and recovery snapshots | 2026-03-29 | High | Proposal 016 remains compatible with current retry-in-place semantics. |

## E. Current Repo Reality / Contradictions
| Evidence ID | Repo Surface | Proposal Claim | Current Repo Reality | Verified On | Confidence | Implication |
|---|---|---|---|---|---|---|
| REAL-01 | cancellation truth | proposal still includes `cancelled_before_output` / `cancelled_after_output` and explicit cancellation-bridge tests | current repo already treats `.cancelled` as first-class settled agent truth through stable run-control | 2026-03-29 | High | Earlier cancellation blocker remains closed. |
| REAL-02 | outcome-owner model | proposal still states flattened columns are canonical and `outcomeEnvelopeJSON` is diagnostic only | current repo already carries supporting evidence in `providerReceiptJSON`, `validationFailureJSON`, and `outputEnvelopesJSON` | 2026-03-29 | High | Earlier schema-ownership blocker remains closed. |
| REAL-03 | aggregate-step ownership | proposal still states `StageExecution` remains canonical for stage terminality and `AggregateSettlementRecord` is subordinate detail | current repo already hangs report/recovery evidence off `StageExecution` | 2026-03-29 | High | Earlier aggregate-authority blocker remains closed. |
| REAL-04 | limit-exhaustion delta | proposal now adds `limit_exhausted_before_output` / `limit_exhausted_after_output`, `providerStopReason`, and neutral-stop handling | current repo already has normalized provider receipt JSON plus structured execution receipts, but does not yet surface stop reason as canonical truth | 2026-03-29 | High | This is a real implementation delta, not a proposal incompleteness issue. |
| REAL-05 | provider-truth migration | proposal still reuses frozen/provider truth rather than replacing it | current repo already has stable frozen binding snapshot, frozen provenance, and `runtimeTrustLevel` seams | 2026-03-29 | High | No live contradiction surfaced here. |

## F. State Coverage Matrix
| State | Proposal Status (`Specified | Partial | Missing | Contradicted by repo | Deferred intentionally`) | Evidence IDs | Repo Touchpoints | Notes / Risks |
|---|---|---|---|---|
| Entry | Specified | `DOC-01`, `BASE-01` | baseline + proposal overview | Scope and motivating incident are explicit. |
| Happy path | Specified | `DOC-01`, `MAP-05` | agent outcome + stage settlement + aggregate settlement | Happy-path transport truth is bounded. |
| Loading | Deferred intentionally | `DOC-01` | none material | Proposal is runtime-truth-focused, not loading-state-focused. |
| Empty | Deferred intentionally | `DOC-01` | none material | Not central to proposal readiness. |
| Validation error | Specified | `DOC-01`, `MAP-03`, `MAP-08` | validation failure JSON, evidence packet, report builder | Validation failure after output remains explicit and grounded. |
| Backend error | Specified | `DOC-01`, `MAP-03`, `MAP-10` | receipt / transcript / timeout evidence | Transport-error and timeout classes remain central. |
| Offline / degraded | Specified | `DOC-01`, `DOC-07` | runtime truth downgrade rules | `unverifiable` degradation stays grounded in existing provider-truth seams. |
| Retry / recovery | Specified | `DOC-01`, `MAP-06`, `MAP-07`, `MAP-11` | startup repair, recovery snapshot, retry coordinator | Startup repair and canonical next-action intent remain explicit. |
| Auth / permission expiry | Deferred intentionally | `DOC-01` | none material | Out of slice. |
| Rollback / cancellation | Specified | `DOC-01`, `DOC-08`, `MAP-09`, `REAL-01` | run-control + RunCancellationCoordinator | Cancellation remains explicitly inside the canonical outcome story. |
| Limit exhaustion | Specified | `DOC-01`, `MAP-10`, `REAL-04` | provider receipt + execution receipt seams | New delta is explicit and grounded. |

## G. Proposal Completeness Matrix
| Dimension | Status (`Specified | Partial | Missing | Contradicted by repo | Deferred intentionally`) | Evidence IDs | Notes |
|---|---|---|---|
| Problem / user intent | Specified | `DOC-01` | Motivating incident and intended repair slice remain concrete. |
| Scope boundaries | Specified | `DOC-01`, `DOC-03` | Scope and out-of-scope sections remain bounded. |
| Reusable baseline coverage | Specified | `DOC-02`, `BASE-01`, `BASE-02` | Baseline intake remains sufficient. |
| Screen / surface definition | Specified | `DOC-06`, `MAP-07`, `MAP-08` | Operator-facing repair/report consumers remain grounded in existing shell surfaces. |
| Navigation / entry points | Specified | `DOC-06` | Entry points are already clear from current shell baseline. |
| State handling | Specified | `F`, `REAL-01`..`REAL-04` | Prior state-handling blockers remain closed, and the new limit-exhaustion state is explicit. |
| Data / API contract | Specified | `MAP-01`..`MAP-11`, `REAL-01`..`REAL-05` | Outcome, settlement, aggregate, provider truth, and limit-exhaustion handling are bounded clearly enough to implement. |
| Persistence / caching | Specified | `MAP-01`..`MAP-11`, `REAL-02`, `REAL-03`, `REAL-04` | Singular owner model remains explicit where it needed to be. |
| Permissions / auth expiry | Deferred intentionally | `DOC-01` | Out of slice. |
| Feature flags / rollout / rollback | Missing | `DOC-01` | Still omitted, but non-blocking for proposal-readiness in this bounded slice. |
| Analytics / instrumentation | Deferred intentionally | `DOC-01` | Product overlay not requested. |
| Testing strategy | Specified | `DOC-01`, `REAL-01`..`REAL-04` | Verification now also covers the new limit-exhaustion slice. |
| Dependencies / integration points | Specified | `DOC-02`..`DOC-08`, `MAP-01`..`MAP-11` | Dependency chain remains explicit and locally mappable. |

## H. Assumptions, Gaps, and Open Questions

- ASSUMP-01: readiness can be judged from proposal/doc/code/baseline evidence without a fresh runtime replay.
- ASSUMP-02: `Approval.lineageID` in the field table and `Approval.lineageKey` in the preferred contract are semantically equivalent and intentionally leave naming flexibility to implementation.
- ASSUMP-03: `ExecutionReceiptV2` names the proposal’s normalized receipt/evidence layer over existing receipt artifacts and provider receipt, not a parallel canonical truth source.
- GAP-01: no proposal-local `integration-context.md` exists yet; optional future review hygiene only.
- OPEN-01: none proposal-blocking in the updated draft.

## I. Research Reuse Note

- No fresh web research was needed for this proposal-readiness pass.
- Existing proposal-local research at `docs/proposals/016-transport-outcome-truth-stage-settlement-and-resume-idempotency.review/research-pack.md` was reused after freshness check.
- This was a `reuse-after-freshness-check` round: repo baseline and code seams were rechecked locally, the research-backed recommendations were compared against the new proposal text, and the verdict stayed green because the edits closed those deltas without reopening runtime-truth contradictions.

## O. Research Triggers / External Questions

| Trigger ID | Trigger Type (`Proposal gap | Baseline constraint | Host-system integration risk | Unresolved tradeoff`) | Local Evidence IDs | Question to Research | Why Local Evidence Is Not Enough | Time Sensitivity / Freshness Risk |
|---|---|---|---|---|---|
| RSH-01 | Host-system integration risk | `DOC-01`, `MAP-10`, `REAL-04` | Across official provider docs, which finish / stop reasons and limit-exhaustion signals can end a response after partial output, and do neutral markers like `stop` or ordinary stream closure ever justify treating the attempt as success on their own? | Current repo seams show where this truth should live, but not the latest external provider semantics that Proposal 016 must normalize. | High: provider/API semantics can change. |
| RSH-02 | Unresolved tradeoff | `DOC-01`, `MAP-05`, `MAP-06`, `MAP-09` | In primary workflow-orchestration guidance, what durable-settlement / idempotency pattern is recommended so restart or resume does not duplicate work or create contradictory terminal truth after cancellation, timeout, or limit exhaustion? | Local code shows current ownership, but not external best-practice guidance for exact-once settlement and resume safety. | Medium: platform guidance evolves more slowly, but still benefits from refresh. |
| RSH-03 | Unresolved tradeoff | `DOC-01`, `REAL-02`, `REAL-04` | In primary technical guidance, how should systems separate canonical terminal outcome fields from diagnostic/raw envelopes so reports and recovery surfaces do not read competing authorities? | Local review established repo truth, but research can validate whether Proposal 016's flattened-columns-plus-diagnostic-envelope model matches broader modern practice. | Medium. |
