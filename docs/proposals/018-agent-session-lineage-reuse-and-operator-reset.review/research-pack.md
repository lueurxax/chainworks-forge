# Proposal Research Pack

## 0. Review Target and Local Context Consumed
- Proposal: `docs/proposals/018-agent-session-lineage-reuse-and-operator-reset.md`
- Research round: `R2`
- Proposal evidence pack used: `docs/reviews/018-agent-session-lineage-reuse-and-operator-reset-evidence-pack.md`
- Current-system baseline used: `.review-baselines/current-system-baseline.md`
- Proposal-specific integration context used: `none`
- Existing research pack reused: prior `R1` on the same proposal, refreshed rather than copied forward blindly
- Adjacent docs consumed:
  - `docs/reference/live-provider-execution-slice.md`
  - `docs/reference/runtime-contract.md`
  - `docs/reference/execution-truth-and-recovery.md`
  - `docs/reference/operator-experience.md`
  - `docs/proposals/015-skill-resolution-and-runtime-injection.md`
  - `examples/workflows/full-mvp-live.yaml`
- Current code / module mapping consumed:
  - `Chainworks Forge/Models/StageExecution.swift`
  - `Chainworks Forge/Models/AgentExecution.swift`
  - `Chainworks Forge/Models/Approval.swift`
  - `Chainworks Forge/Engine/RecoveryCoordinator.swift`
  - `Chainworks Forge/Engine/StageRetryCoordinator.swift`
  - `Chainworks Forge/Views/RecoverySheet.swift`
  - `Chainworks Forge/Views/BlockedRunRecoveryView.swift`
- Local evidence IDs that triggered this deeper round:
  - `DOC-01`
  - `DOC-06`
  - `DOC-07`
  - `DOC-08`
  - `DOC-11`
  - `MAP-01`
  - `MAP-02`
  - `MAP-03`
  - `REAL-01`
  - `REAL-04`
  - new research triggers recorded in section `O`
- Notes on baseline freshness or local contradictions:
  - baseline remained fresh for runtime ownership, recovery ownership, and execution-truth seams
  - this round was triggered by new proposal text about token-burn protection, compaction, checkpoint rehydration, and wider family reuse, not by baseline drift

## 1. Research Questions Derived from Local Evidence
| Question ID | Derived From (`Proposal gap | Baseline constraint | Host-system integration risk | Unresolved tradeoff`) | Local Evidence IDs | Research Question | Why Local Evidence Is Not Enough | Priority |
|---|---|---|---|---|---|
| RQ-01 | Host-system integration risk | `REAL-04`, `MAP-01`, `MAP-02`, `MAP-03` | In systems that support rerun, retry, or reset, how should a new branch/attempt identifier relate to existing persisted execution lineage so there is one durable authority rather than parallel lineage IDs? | Local repo evidence proves the collision risk, but not which external durable-execution pattern is strongest to adopt. | High |
| RQ-02 | Unresolved tradeoff | `DOC-01`, `DOC-06`, `DOC-07`, `REAL-01` | For long-running reused sessions, what external patterns determine when reuse stops paying off and should compact or invalidate instead of dragging raw history forward? | Local proposal names thresholds, but local repo evidence does not say which signals are strongest or which ones should be treated as canonical versus heuristic. | High |
| RQ-03 | Proposal gap | `DOC-01`, `DOC-07`, `DOC-08` | Before budget-driven refresh or operator reset, what should a durable checkpoint contain so a fresh generation can rehydrate from explicit truth instead of opaque transcript carry-over? | Local proposal names a checkpoint artifact, but local evidence alone does not define the strongest externally grounded minimum contents. | High |
| RQ-04 | Host-system integration risk | `DOC-01`, `DOC-11`, `REAL-01` | When widening from `same_invocation_owner` to opt-in `same_agent_family_within_run`, what compatibility constraints should hold so reuse does not silently outrank the current invocation contract and burn tokens on low-value hidden context? | The workflow examples prove repeated same-agent invocations exist, but not when wider reuse is actually safe. | High |
| RQ-05 | Proposal gap | `DOC-01`, `DOC-06` | Which provider-exposed counters are strong enough to support `cold_start_tokens_saved`, `session_growth_tokens`, and savings-versus-fresh measurement without guessing from transcript length alone? | The proposal names KPIs, but local evidence does not show whether providers expose the required counters. | High |

## 2. Source Ledger
| Source ID | Title | Publisher / Authority | URL or Reference | Published Date | Last Updated Date | Accessed / Verified Date | Why This Source Matters | Temporal Volatility / Freshness Risk | Confidence |
|---|---|---|---|---|---|---|---|---|---|
| SRC-01 | Variables reference | GitHub Docs | https://docs.github.com/en/enterprise-server%403.15/actions/reference/workflows-and-actions/variables | not stated | not stated | 2026-03-30 | Official definition of `GITHUB_RUN_ID` and `GITHUB_RUN_ATTEMPT`, which directly encodes stable run identity plus subordinate rerun attempt identity. | Medium: doc version is GitHub Enterprise Server 3.15, but the identity semantics are simple and stable. | High |
| SRC-02 | REST API endpoints for workflow runs | GitHub Docs | https://docs.github.com/en/enterprise-cloud@latest/rest/actions/workflow-runs | not stated | API version shown on page | 2026-03-30 | Official run-attempt API still shows attempts nested under one stable run identifier. | Medium: API version may move, but the run/attempt nesting is explicit on the current page. | High |
| SRC-03 | workflow package - go.temporal.io/api/workflow/v1 | Temporal API Docs | https://pkg.go.dev/go.temporal.io/api/workflow/v1 | not stated | current as served | 2026-03-30 | Official Temporal API docs state that `FirstRunId` remains the first run in a chain across continue-as-new, retry, reset, and cron, and `ResetRunId` points to the newer run. | Medium: field names should be rechecked if adopted verbatim later. | High |
| SRC-04 | Temporal 102 workshop PDF | Temporal / Learn Temporal | https://learn.temporal.io/assets/files/temporal-102-with-dotnet-replay2025-f13b3a1ccdf33c0be7ae515f1a41e821.pdf | 2025 workshop edition | n/a | 2026-03-30 | Official workshop material explains event history as the durable append-only execution truth and gives concrete event-history warning/termination limits. | Medium: workshop PDF is less stable than API docs but still primary Temporal material. | High |
| SRC-05 | Crafting an error handling strategy | Temporal / Learn Temporal | https://learn.temporal.io/assets/files/crafting-an-error-handling-strategy-dotnet-replay2025-e633cef41481baac8b25a636533c4d37.pdf | 2025 workshop edition | n/a | 2026-03-30 | Official workshop material explains replay-after-failure and idempotent recovery from durable event history rather than opaque in-memory state. | Medium. | High |
| SRC-06 | Prompt caching | Anthropic Claude API Docs | https://docs.anthropic.com/en/docs/build-with-claude/prompt-caching | not stated | current as served | 2026-03-30 | Official Anthropic caching rules show that cacheability depends on exact shared prefix, ordering of `tools` / `system` / `messages`, stable breakpoints, and measurable cache economics. | Medium: pricing/model support may change, but the compatibility rules are core. | High |
| SRC-07 | Compaction | Anthropic Claude API Docs | https://platform.claude.com/docs/en/build-with-claude/compaction | beta | current as served | 2026-03-30 | Official Anthropic compaction guidance gives the clearest primary-source pattern for summary-before-refresh, explicit compaction iterations, and using summary state when raw history is no longer accessible. | High: beta feature and wording may evolve; recheck before quoting exact beta names. | High |
| SRC-08 | Prompt caching | OpenAI API Docs | https://developers.openai.com/api/docs/guides/prompt-caching | not stated | current as served | 2026-03-30 | Official OpenAI caching rules show exact-prefix matching, `cached_tokens` telemetry, retention windows, overflow behavior, and best-practice prompt structuring. | Medium: retention/model support can change, but prefix/cache telemetry semantics are stable enough for design guidance. | High |
| SRC-09 | Managing costs | OpenAI API Docs | https://developers.openai.com/api/docs/guides/realtime-costs | not stated | current as served | 2026-03-30 | Official OpenAI guidance explicitly ties token cost control to truncation windows, cache preservation, and avoiding repeated cache busting in long-running sessions. | Medium. | High |
| SRC-10 | Generating content / UsageMetadata | Google AI for Developers | https://ai.google.dev/api/generate-content | not stated | current as served | 2026-03-30 | Official Gemini API docs expose `promptTokenCount`, `cachedContentTokenCount`, and `totalTokenCount`, which directly supports measurable burn/savings telemetry. | Medium: fields may expand, but current token counters are explicit. | High |
| SRC-11 | Context caching overview | Vertex AI Docs | https://cloud.google.com/vertex-ai/generative-ai/docs/context-cache/context-cache-overview | not stated | 2026-03-27 | 2026-03-30 | Official Vertex docs show implicit/explicit caching discounts, TTL behavior, `cachedContentTokenCount`, and the need to keep common prefixes stable. | Medium: supported models and discounts can shift, but the structural guidance is current. | High |
| SRC-12 | REST Resource: CachedContent | Vertex AI Docs | https://cloud.google.com/vertex-ai/docs/reference/rest/v1/projects.locations.cachedContents | not stated | 2026-03-19 | 2026-03-30 | Official API reference shows cached content is immutable, time-bounded, separately addressable, and exposes usage metadata like total cached token count. | Medium. | High |
| SRC-13 | Use a context cache | Vertex AI Docs | https://cloud.google.com/vertex-ai/generative-ai/docs/context-cache/context-cache-use | not stated | current as served | 2026-03-30 | Official usage restrictions make system instructions, tools, and tool config part of the cache contract instead of something safely redefined later. | Medium. | High |
| SRC-14 | Prompt caching for Claude on Vertex AI | Vertex AI Docs | https://cloud.google.com/vertex-ai/generative-ai/docs/partner-models/claude/prompt-caching | not stated | 2026-03-16 | 2026-03-30 | Official Google wrapper docs independently confirm Anthropic-style identical-prefix requirements, TTL refresh, and cached-read price reduction on a second platform. | Medium. | High |

## 3. Findings by Theme

### Stable owner, subordinate generations, and durable replay
- Finding ID: `FIND-ARCH-01`
  Research question IDs: `RQ-01`
  Source IDs: `SRC-01`, `SRC-02`, `SRC-03`
  Source-backed finding: GitHub and Temporal still converge on one stable root plus subordinate attempts/runs. GitHub says `GITHUB_RUN_ID` does not change across reruns while `GITHUB_RUN_ATTEMPT` increments, and the REST API addresses attempts as `{run_id}` plus `{attempt_number}`. Temporal's API docs similarly treat `FirstRunId` as the first run in an execution chain and use `ResetRunId` to point to newer runs created by reset.
  Model inference / host-system note: The earlier `R1` conclusion remains intact after the proposal's budget additions. Session generations, resets, and checkpoints should remain subordinate to existing execution truth rather than creating a second branch authority just because token-burn logic is now richer.
  Host-system surface touched: `ownerExecutionLineageID`, `invocationOwnerKey`, generation/reset history
  Time-sensitive: `yes, low`
  Confidence: `High`

### Budget guard should be driven by measured cache value, not transcript length alone
- Finding ID: `FIND-BUDGET-01`
  Research question IDs: `RQ-02`, `RQ-05`
  Source IDs: `SRC-08`, `SRC-09`, `SRC-10`, `SRC-11`, `SRC-12`, `SRC-14`
  Source-backed finding: The researched provider docs do not treat "long conversation" as a sufficient budget signal by itself. OpenAI says cache hits require exact shared prefixes, exposes `cached_tokens`, and warns that changing history or repeated truncation degrades cache effectiveness. Gemini/Vertex expose `cachedContentTokenCount` and cache usage metadata, and Vertex explicitly discounts cached tokens versus standard input tokens. Vertex's cached-content resource also exposes durable metadata like total cached token count and expiration.
  Model inference / host-system note: `ContextBudgetGuard` should read measured signals such as cached-token share, effective prompt size, cumulative prompt tokens, and repeated truncation/compaction churn. A raw turn-count or transcript-size threshold is too weak on its own because a short but constantly cache-busting session can be economically worse than a longer, stable-prefix session.
  Host-system surface touched: `ContextBudgetGuard`, `cumulativePromptTokens`, `cumulativeCostCents`, KPI interpretation
  Time-sensitive: `yes, medium`
  Confidence: `High`

- Finding ID: `FIND-BUDGET-02`
  Research question IDs: `RQ-02`
  Source IDs: `SRC-07`, `SRC-09`
  Source-backed finding: Both Anthropic and OpenAI treat context-growth control as an active policy decision. Anthropic's compaction docs recommend server-side compaction for long-running agentic workflows and expose a separate `iterations` array so compaction cost is measurable. OpenAI's cost guide says clients can intentionally use a smaller token window to control usage, and that repeated truncation can bust cache effectiveness unless the system leaves more headroom.
  Model inference / host-system note: `fresh_after_budget` and `fresh_after_compaction` should be treated as economically motivated recovery outcomes, not just failure outcomes. The proposal is strongest when budget invalidation is tied to measured "continuing reuse is now lower value than checkpoint + fresh" rather than to a hard cap alone.
  Host-system surface touched: invalidation table, disposition semantics, budget metrics
  Time-sensitive: `yes, medium`
  Confidence: `High`

### Checkpoint before refresh should capture explicit durable continuity, not hidden chat state
- Finding ID: `FIND-CHECKPOINT-01`
  Research question IDs: `RQ-03`
  Source IDs: `SRC-07`
  Source-backed finding: Anthropic's default compaction prompt is unusually explicit about what a summary must preserve for a future context where raw history is no longer accessible: state, next steps, learnings, and anything else needed to continue making progress. The same page also says subsequent requests continue from the compaction summary rather than from the dropped raw history.
  Model inference / host-system note: This strongly supports `AgentSessionCheckpointBuilder` as a first-class durable artifact, not a cosmetic summary. The checkpoint should preserve continuation-relevant state explicitly enough that the next generation can continue from artifacts plus checkpoint without needing opaque session memory.
  Host-system surface touched: `AgentSessionCheckpointBuilder`, checkpoint artifact schema, reset/compaction handoff
  Time-sensitive: `yes, medium because Anthropic compaction is beta`
  Confidence: `High`

- Finding ID: `FIND-CHECKPOINT-02`
  Research question IDs: `RQ-03`
  Source IDs: `SRC-04`, `SRC-05`
  Source-backed finding: Temporal's official materials still make event history the durable append-only source of truth for reconstruction after crashes and retries. Workers replay from persisted history rather than trusting mutable in-memory execution state, and Temporal explicitly recommends keeping history manageable because replay cost rises as history grows.
  Model inference / host-system note: In Chainworks, a checkpoint should not try to become a parallel opaque session truth. It should be a compact continuation artifact that points back to canonical run artifacts, last validated aggregate state, and unresolved constraints. That keeps rehydration faithful to the repo's existing durable-truth model.
  Host-system surface touched: checkpoint contents, report/evidence readers, rehydration path
  Time-sensitive: `yes, low`
  Confidence: `High`

### Wider family reuse is safe only when the static prefix and binding contract truly stay the same
- Finding ID: `FIND-SCOPE-01`
  Research question IDs: `RQ-04`
  Source IDs: `SRC-06`, `SRC-08`, `SRC-13`, `SRC-14`
  Source-backed finding: Every provider-side caching surface reviewed is strict about compatibility. Anthropic says prompt caching references the full prefix of `tools`, `system`, then `messages`, and moving a breakpoint onto changing content can destroy cache usefulness. OpenAI says cache hits are possible only for exact shared prefixes and identical tools. Vertex says system instructions, tool config, and tools belong to the cache contract and should not be respecified when using that cache.
  Model inference / host-system note: `same_agent_family_within_run` should fail closed unless the family-wide work keeps the same static binding contract in practice, not just the same `agentID`. If task framing, tools, system instructions, skill injection, or workspace policy drift, then family reuse is more likely to leak stale hidden context and burn tokens than to save them.
  Host-system surface touched: `sessionFamilyID`, binding fingerprint, family reuse admission rules
  Time-sensitive: `yes, medium`
  Confidence: `High`

- Finding ID: `FIND-SCOPE-02`
  Research question IDs: `RQ-04`, `RQ-05`
  Source IDs: `SRC-08`, `SRC-11`
  Source-backed finding: OpenAI and Vertex both say the best caching outcomes come from placing static/common content at the beginning and dynamic content at the end, plus keeping prefixes similar in a short time window. Vertex's explicit/implicit caching docs frame caching as especially well suited to repeated reuse of a substantial initial context with request-specific tails.
  Model inference / host-system note: For `P018`, this means opt-in family reuse should be justified primarily for bounded same-agent flows where the initial scaffolding is truly shared and the delta work is in the tail. It is a poor fit for adjacent tasks that merely happen to use the same agent but carry materially different task contracts or tool surfaces.
  Host-system surface touched: workflow/catalog opt-in policy, `sessionFamilyID` authoring, KPI interpretation
  Time-sensitive: `yes, medium`
  Confidence: `High`

### Provider metrics are strong enough to support real burn accounting
- Finding ID: `FIND-METRICS-01`
  Research question IDs: `RQ-05`
  Source IDs: `SRC-07`, `SRC-08`, `SRC-10`, `SRC-11`, `SRC-12`
  Source-backed finding: The reviewed providers expose counters that are directly useful for a burn-accounting layer. OpenAI exposes `usage.prompt_tokens_details.cached_tokens`; Gemini exposes `promptTokenCount`, `cachedContentTokenCount`, and `totalTokenCount`; Vertex explicit cache resources expose `usageMetadata.totalTokenCount`; Anthropic compaction exposes per-iteration usage so compaction cost can be included instead of hidden.
  Model inference / host-system note: `cold_start_tokens_saved`, `session_growth_tokens`, and savings-versus-fresh do not need to be guessed from transcript length. The proposal can ground them in provider-returned counters plus local estimated-cost mapping. The remaining judgment call is how to normalize them across providers, not whether the raw signals exist.
  Host-system surface touched: KPI layer, receipts, session generation accounting, compaction accounting
  Time-sensitive: `yes, medium`
  Confidence: `High`

## 4. Host-System Applicability Matrix
| Insight ID | Source IDs | Classification (`Adopt | Adapt | Watch | Reject`) | Proposal Area Affected | Host-System Surface Touched | Why It Applies or Does Not Apply | Concrete Recommended Change |
|---|---|---|---|---|---|---|
| APP-01 | `SRC-01`, `SRC-02`, `SRC-03` | Adopt | authority model | `ownerExecutionLineageID`, generation/reset structure | The earlier `R1` result still holds after the token-burn expansion: stable execution owner first, subordinate attempts/generations beneath it. | Keep `ownerExecutionLineageID` read-only and upstream of session reuse. Do not let checkpoint or budget logic create a new branch authority. |
| APP-02 | `SRC-07`, `SRC-08`, `SRC-09`, `SRC-10`, `SRC-11` | Adopt | budget guard | `ContextBudgetGuard`, KPI semantics | Providers expose enough measured signals to judge when reuse stopped paying off. | Add one explicit sentence that budget decisions are based on measured usage/cached-token signals and compaction/truncation churn, not transcript length alone. |
| APP-03 | `SRC-07`, `SRC-04`, `SRC-05` | Adopt | checkpoint artifact | `AgentSessionCheckpointBuilder`, reset/compaction rehydration | Durable systems keep resumable state in explicit persisted truth; Anthropic compaction shows the minimum useful summary shape. | Keep checkpoint artifacts provider-agnostic and continuation-oriented: state, next steps, learnings, selected artifact refs, last validated aggregate state, and enough owner/binding context to rehydrate safely. |
| APP-04 | `SRC-06`, `SRC-08`, `SRC-13`, `SRC-14` | Adapt | family reuse | `same_agent_family_within_run`, binding fingerprint, `sessionFamilyID` | The wider reuse mode is viable only when the static prefix, tool surface, and binding contract stay compatible in practice. | Make family reuse fail closed if task framing, system/tools, skill hash, workspace policy, or other binding inputs no longer match the reusable static prefix contract. |
| APP-05 | `SRC-08`, `SRC-09`, `SRC-11` | Reject | naive reuse-as-savings assumption | success metrics, reuse-rate interpretation | External docs repeatedly show that longer sessions can lose caching value via prefix drift or truncation. | Reject any interpretation where higher reuse rate alone counts as success. Require savings evidence from provider counters or normalized cost estimates. |
| APP-06 | `SRC-07`, `SRC-11`, `SRC-12` | Watch | provider-specific cache semantics | persistence schema, receipts | Providers differ on TTL, storage, explicit versus implicit caching, and beta behavior. | Keep the canonical Chainworks schema provider-agnostic and treat provider cache handles/TTL/storage facts as diagnostic metadata, not as the portable ownership model. |

## 5. Proposal Deltas / Recommended Updates
| Delta ID | Proposal Section / Decision | Recommended Update | Why It Helps | Supporting Source IDs | Supporting Local Evidence IDs | Priority |
|---|---|---|---|---|---|---|
| DELTA-01 | Sections `6.3` and `7` | Add one explicit sentence that `ContextBudgetGuard` is driven by measured provider signals such as cached-token share, effective prompt size, compaction/truncation churn, and cumulative cost, not by transcript size alone. | Prevents the budget layer from becoming an arbitrary heuristic disconnected from provider reality. | `SRC-07`, `SRC-08`, `SRC-09`, `SRC-10`, `SRC-11` | `DOC-01`, `DOC-06` | High |
| DELTA-02 | Section `6.4` | Tighten the checkpoint artifact to say it must preserve continuation-relevant state in provider-agnostic form: state, next steps, learnings, unresolved constraints, selected artifact refs, last validated aggregate state, and binding/owner context needed for safe fresh rehydration. | Makes `AgentSessionCheckpointBuilder` durable and replay-safe rather than summary-like but underspecified. | `SRC-07`, `SRC-04`, `SRC-05` | `DOC-01`, `DOC-07`, `DOC-08` | High |
| DELTA-03 | Sections `6.2`, `6.3`, `6.6` | Add an explicit fail-closed rule for `same_agent_family_within_run`: if the static reusable prefix implied by task/tool/system/binding inputs no longer matches, the decision must degrade to fresh generation even if `sessionFamilyID` matches. | Prevents family reuse from becoming a hidden-context loophole that burns tokens and outranks the current invocation contract. | `SRC-06`, `SRC-08`, `SRC-13`, `SRC-14` | `DOC-11`, `REAL-01` | High |
| DELTA-04 | Section `7` success interpretation | Add one sentence that a reused session counts as a savings win only when measured cached-token share or normalized cost versus fresh baseline remains favorable after compaction/truncation effects are included. | Aligns the KPI layer with provider-measurable reality and blocks vanity reuse metrics. | `SRC-07`, `SRC-08`, `SRC-10`, `SRC-11`, `SRC-12` | `DOC-01`, `DOC-06` | Medium |
| DELTA-05 | Section `6.3` / invalidation table | Clarify that budget-driven refresh is a normal economic control path, not only a failure mode, and that `fresh_after_compaction` can be the preferred path when continued reuse is no longer efficient. | Matches the external pattern that compaction/continue-as-new is often proactive, not purely exceptional. | `SRC-07`, `SRC-09`, `SRC-04` | `DOC-01` | Medium |

## 6. Freshness Risks / Recheck Triggers
| Trigger ID | Claim / Recommendation | Why It Is Time-Sensitive | What Must Be Rechecked | Recheck Trigger / Window | Source IDs |
|---|---|---|---|---|---|
| FRESH-01 | Stable root plus subordinate attempt/run pattern | GitHub and Temporal docs evolve versioning and field names | confirm exact field names if the proposal later quotes them verbatim | next research reuse round or before implementation spec locks names | `SRC-01`, `SRC-02`, `SRC-03` |
| FRESH-02 | Anthropic compaction guidance | Anthropic compaction is a beta feature | confirm prompt shape, billing semantics, and beta header names | any later round that quotes beta-specific API details | `SRC-07` |
| FRESH-03 | OpenAI prompt caching retention and overflow behavior | retention windows and supported models can change | recheck retention defaults, overflow guidance, and telemetry field names | future research reuse or implementation planning | `SRC-08` |
| FRESH-04 | Vertex/Gemini caching discounts and supported models | cache discounts, TTL support, and model coverage may change | recheck numeric discounts and model availability | future research reuse or implementation planning | `SRC-10`, `SRC-11`, `SRC-12`, `SRC-14` |

## 7. Remaining Open Questions
- QUESTION-01: Should `P018` persist one normalized "fresh baseline estimate" alongside measured provider usage so savings-vs-fresh can be computed deterministically across providers, or is provider-specific raw telemetry plus off-line normalization enough?
- QUESTION-02: Should `same_agent_family_within_run` require explicit author-supplied proof that the task family shares the same static prompt/tool prefix, or is matching `sessionFamilyID` plus binding fingerprint sufficient?
- QUESTION-03: Does the proposal want checkpoint artifacts to be human-readable first, machine-readable first, or a companion pair? The external research strongly supports explicit durable continuity, but it does not force one exact representation.

## 8. Net Research Outcome
- `Adopt`: keep one stable execution/recovery owner and make session generations, resets, checkpoints, and budget refreshes subordinate to that owner.
- `Adopt`: drive `ContextBudgetGuard` from measured provider telemetry such as cached-token counts, effective prompt size, truncation/compaction churn, and normalized cost, not transcript length alone.
- `Adopt`: keep `AgentSessionCheckpointBuilder` as a durable continuity artifact that preserves explicit state, next steps, learnings, and artifact refs so a fresh generation can continue without opaque raw history.
- `Adapt`: allow `same_agent_family_within_run` only when the real reusable static prefix stays compatible across task/tool/system/binding inputs; otherwise fail closed to fresh generation.
- `Reject`: do not count reuse itself as success. If cache compatibility erodes or truncation/compaction churn dominates, checkpoint-plus-fresh is the correct economic outcome rather than a regression.
