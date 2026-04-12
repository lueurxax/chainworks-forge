# Proposal Evidence Pack

## A. Repo-Local Proposal / Document Inventory
| Evidence ID | Source / Path / Artifact | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|
| DOC-01 | `docs/proposals/037-acp-execution-supervision-and-idle-watchdog.md` | 2026-04-11 | High | Current draft already chooses `ExecutionEventBridge` as taxonomy owner, already names a repo-owned `proposal-037` lane, and still defines the initial canonical mapping plus implementation slices in `§10.2` and `§11`. | Review could miss live proposal-first contradictions if it uses stale text assumptions. | Primary proposal source. |
| DOC-02 | `.review-baselines/current-system-baseline.md` | 2026-04-11 | High | Reusable intake baseline is stale for this slice: it still describes runtime as Goose-backed and still names MVP provider families as `codex`, `claude_code`, `gemini`. | Review could inherit stale runtime/provider assumptions without targeted refresh. | Intake baseline. |
| DOC-03 | `docs/reference/current-system-baseline.md` | 2026-04-11 | High | Stable current-system baseline is ACP-era and current enough for this slice; it names ACP-backed runtime transport and ACP-era provider vocabulary. | Using only the stale intake baseline would distort proposal dependency checks. | Stable baseline refresh anchor. |
| DOC-04 | `docs/reference/runtime-contract.md` | 2026-04-11 | High | Stable runtime boundary is ACP-era and still compatible with the proposal's watchdog scope. | Wrong runtime baseline would distort owner checks. | Stable runtime dependency reference. |
| DOC-05 | `docs/reference/execution-truth-and-recovery.md` | 2026-04-11 | High | Stable execution-truth doc still anchors readers on flattened persisted execution columns and stage-owned recovery truth, but it does not yet document `supervisionClassification` or watchdog-specific read-order. | Proposal can look complete while the stable dependency chain remains underspecified. | Primary stable truth reference. |
| DOC-06 | `docs/reference/test-gates.md` | 2026-04-11 | High | Repo-owned proof lanes are already part of stable test-gate policy, and `proposal-037` is already documented there on current `HEAD`. | A stale review could preserve a false blocker about missing proof-lane ownership. | Verification ownership reference. |
| DOC-07 | `scripts/test-gate.sh` | 2026-04-11 | High | Current repo already has `PROPOSAL_037_TESTS` and a `proposal-037|p037` case block. | A stale review could preserve a false blocker about missing gate implementation ownership. | Current proof-lane reality. |
| DOC-08 | current `037` review / evidence artifacts | 2026-04-11 | High | Prior local artifacts were useful only as stale-basis comparators; they were not current truth. | Review could accidentally re-emit stale blockers instead of reevaluating current `HEAD`. | Freshness comparator. |

## B. Reusable Baseline Inputs
| Evidence ID | Artifact / Slice | Status (`Reused | Partially refreshed | Missing`) | Covered Surfaces | Verified On | Confidence | Freshness Notes | Relevance |
|---|---|---|---|---|---|---|---|
| BASE-01 | `.review-baselines/current-system-baseline.md` | Partially refreshed | review intake assumptions | 2026-04-11 | High | Useful entry point, but runtime/provider wording is stale for ACP-era execution. | Intake baseline only. |
| BASE-02 | `docs/reference/current-system-baseline.md` | Reused | current product/runtime/provider baseline | 2026-04-11 | High | Fresh and directly relevant. | Stable baseline authority. |
| BASE-03 | `docs/reference/runtime-contract.md` | Reused | frozen runtime/provider boundary | 2026-04-11 | High | No blocking contradiction found here. | Stable runtime boundary. |
| BASE-04 | `docs/reference/execution-truth-and-recovery.md` | Reused | canonical execution truth, stage truth, recovery precedence | 2026-04-11 | High | Fresh enough for owner checks, but still missing watchdog-specific promotion. | Stable execution-truth authority. |
| BASE-05 | `docs/reference/test-gates.md` + `scripts/test-gate.sh` | Reused | repo-owned proof-lane model | 2026-04-11 | High | Fresh and directly relevant. | Proof-lane ownership. |
| BASE-06 | prior `P037` review/evidence artifacts | Partially refreshed | stale-basis comparator only | 2026-04-11 | High | Used only to verify which previous findings were already closed on current `HEAD`. | Freshness control. |
| BASE-07 | proposal-specific integration context | Missing | none | 2026-04-11 | High | No dedicated integration-context slice exists; local docs plus current code were sufficient. | Not blocking. |

## C. Scope, Out-of-Scope, and Intentional Deferrals
- In scope:
  - ACP execution supervision based on normalized event progress
  - fixed watchdog thresholds
  - one automatic fresh retry
  - supervision truth in receipts, reports, recovery, and timeline surfaces
  - repo-owned proof expectations for the watchdog slice
- Out of scope:
  - provider-specific watchdog policy matrices
  - CPU/process/socket heuristics as canonical truth
  - workflow-YAML redesign
  - broader ACP transport redesign outside supervision/retry behavior
- Deferred intentionally:
  - none blocking at proposal-readiness level
- Assumptions:
  - review mode is `proposal-readiness`
  - no external research is required; local proposal/docs/code evidence is sufficient
- Open questions:
  - whether `search`, `execute`, and `permission:execute` are intentionally being migrated from weak/discovery to strong progress
- Blockers:
  - `§10.2` taxonomy mismatch against the current shared classifier and proof surfaces
  - `§11` still mixes landed owners with future work and does not fully own stable-doc promotion

## D. Affected Screens / Navigation / Entry-Point Slice
| Evidence ID | Screen / Surface / Entry Point | Source (`Baseline | Targeted refresh | Proposal`) | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|---|
| NAV-01 | run-detail focused timeline / live history | Proposal + stable baseline | 2026-04-11 | High | Current draft keeps watchdog history inside the current run-detail / focused-timeline shell path. | Review could invent a UI blocker that the current text no longer has. | User-facing supervision surface. |
| NAV-02 | immutable run report / latest summary | Proposal + stable truth refs | 2026-04-11 | High | Proposal keeps watchdog truth subordinate to persisted execution truth and stage-owned recovery truth, not a new diagnostics surface. | Wrong shell ownership would distort user-facing scope. | Report surface. |
| NAV-03 | recovery UI / next action narrative | Proposal + stable truth refs | 2026-04-11 | High | Proposal keeps recovery guidance stage-owned and secondary to attempt truth. | Review could overstate a recovery-owner conflict that the text already closed. | Recovery surface. |

## E. Impacted Modules / Code-Path Map
| Evidence ID | File Path / Module / Symbol | Layer | Role in Flow | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|---|---|
| MAP-01 | `Chainworks Forge/Engine/ExecutionEventBridge.swift` | event normalization | shared ACP progress classifier | 2026-04-11 | High | Current shared classifier treats `search`, `read`, `read_file`, `read_workspace`, `execute`, `permission:read`, and `permission:execute` as weak/discovery progress. | `§10.2` can silently redefine core watchdog behavior if this owner mapping is ignored. | Main owner for finding `ARCH-001`. |
| MAP-02 | `Chainworks Forge/Engine/RuntimeAgentExecutor.swift` | runtime execution | executor-side coarse classifier and watchdog behavior | 2026-04-11 | High | Current executor still mirrors the same weak/discovery mapping for `search` and `execute`. | Proposal taxonomy mismatch is live against current executor behavior, not only against one helper. | Confirms `ARCH-001` is real. |
| MAP-03 | `Chainworks Forge/Models/AgentExecution.swift` | persistence | attempt-level durable truth | 2026-04-11 | High | `AgentExecution` already persists `supervisionClassification` on current `HEAD`. | `§11` becomes stale if it still frames this as future work. | Main owner for finding `ARCH-002`. |
| MAP-04 | `scripts/test-gate.sh` | proof lane | repo-owned proposal gate runner | 2026-04-11 | High | Current repo already defines `PROPOSAL_037_TESTS` and the `proposal-037|p037` lane. | `§11` becomes stale if it still frames the gate as not yet present. | Main owner for finding `ARCH-002`. |
| MAP-05 | `docs/reference/test-gates.md` | proof documentation | stable proof-lane documentation | 2026-04-11 | High | Stable gate docs already describe `proposal-037` and its intended suite mix. | A stale review could keep a false proof-lane blocker alive. | Confirms proof-lane ownership already exists. |
| MAP-06 | `docs/reference/execution-truth-and-recovery.md` | stable truth reference | canonical execution-truth / recovery precedence | 2026-04-11 | High | Stable doc still documents flattened execution-truth columns and stage truth precedence, but does not yet promote watchdog-specific truth or `supervisionClassification`. | Proposal depends on a stable reference layer that is still partially stale for this slice. | Main owner for finding `ARCH-002`. |
| MAP-07 | `Chainworks ForgeTests/RuntimeAgentExecutorTests.swift` | proof surface | current read-loop watchdog coverage | 2026-04-11 | High | Current proof surface uses `search`-only churn as the read-loop / weak-progress watchdog shape. | Proposal taxonomy mismatch is already visible in existing tests. | Confirms `ARCH-001` at proof level. |

## F. Data / API / Persistence / Auth Touchpoints
| Evidence ID | Touchpoint | File / Module / Doc | Direction | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|---|---|
| DATA-01 | shared progress taxonomy | `ExecutionEventBridge.swift`, `RuntimeAgentExecutor.swift`, `P037 §10.2` | normalized tool names -> watchdog classification | 2026-04-11 | High | Current code says `search`/`execute` are weak; proposal says they are strong. | This is the main live proposal-first contradiction. | Core architecture finding. |
| DATA-02 | durable attempt truth | `AgentExecution.swift`, `ExecutionTruth.swift` | persisted attempt columns -> reports/recovery | 2026-04-11 | High | `supervisionClassification` is already in the durable attempt model. | Proposal must stop framing that field as future work. | Current persistence baseline. |
| DATA-03 | stable execution-truth reference | `docs/reference/execution-truth-and-recovery.md` | stable docs -> implementer/reviewer dependency chain | 2026-04-11 | High | Stable doc still omits watchdog-specific truth and `supervisionClassification` despite proposal depending on them. | Stable dependency chain remains underspecified. | Stable-doc promotion gap. |
| DATA-04 | proof-lane ownership | `scripts/test-gate.sh`, `docs/reference/test-gates.md` | repo-owned lane -> proof execution | 2026-04-11 | High | `proposal-037` already exists on current `HEAD`. | Proposal should treat this as landed substrate, not future work. | Implementation framing check. |

## G. Current Host-System Integration Surfaces
| Evidence ID | Surface / Seam / Owner | Source (`Baseline | Targeted refresh | Current repo`) | Verified On | Confidence | Key Fact | Conflict / Proposal Risk | Relevance |
|---|---|---|---|---|---|---|---|
| INT-01 | ACP event normalization | Current repo | 2026-04-11 | High | Shared classifier lives in `ExecutionEventBridge`; runtime executor mirrors the same discovery-tool set. | Proposal must either match or explicitly migrate this owner contract. | Main taxonomy seam. |
| INT-02 | durable attempt truth | Current repo | 2026-04-11 | High | `AgentExecution` already carries the proposed supervision column. | Proposal text becomes stale if it still presents this as not landed. | Persistence seam. |
| INT-03 | repo-owned proof-lane model | Stable refs + current repo | 2026-04-11 | High | `proposal-037` is already real in both stable gate docs and the shell script. | Proposal text becomes stale if it still frames this as future gate creation. | Verification seam. |
| INT-04 | stable truth reference layer | Stable refs | 2026-04-11 | High | `execution-truth-and-recovery.md` still has not absorbed the watchdog-specific truth the proposal depends on. | Even a good proposal can leave the stable dependency chain stale if promotion is not owned. | Stable-doc seam. |

## H. State Coverage Matrix
| State | Proposal Status (`Specified | Partial | Missing | Contradicted by repo | Deferred intentionally`) | Evidence IDs | Repo Touchpoints | Notes / Risks |
|---|---|---|---|---|
| Entry | Specified | DOC-01, MAP-01 | event classifier before watchdog logic | Entry path is clear. |
| Happy path | Specified | DOC-01, DATA-02 | attempt truth, same-stage retry lineage | Happy path is clear enough for proposal-readiness. |
| Loading | Contradicted by repo | DOC-01, MAP-01, MAP-02, MAP-07 | shared classifier and runtime proof surfaces | `§10.2` says `search` / `execute` are strong, while current repo treats them as weak/discovery. |
| Empty | Deferred intentionally | DOC-01 | not a primary watchdog state | Acceptable. |
| Validation error | Deferred intentionally | DOC-01 | outside watchdog focus | Acceptable. |
| Backend error | Specified | DOC-01, DATA-02, DATA-03 | persisted execution truth and stage truth | Backend-error handling is clear enough. |
| Offline / degraded | Deferred intentionally | DOC-01 | outside scope | Acceptable. |
| Retry / recovery | Specified | DOC-01, DATA-02, DATA-04 | durable retry lineage and proof-lane ownership | Owner chain is clear enough. |
| Auth / permission expiry | Deferred intentionally | DOC-01 | outside slice focus | Acceptable. |
| Rollback / cancellation | Deferred intentionally | DOC-01 | adjacent to broader recovery slices | Acceptable. |

## I. Feature Flags / Rollout / Rollback
| Evidence ID | Mechanism / Flag | Scope | Rollout Plan | Rollback Path | Verified On | Confidence | Notes |
|---|---|---|---|---|---|---|---|
| FLAG-01 | none specified | supervision rollout | no feature flag is defined | same-tree proof plus existing outer timeout remain the only guardrail | 2026-04-11 | Medium | Not a blocker for proposal-readiness. |

## J. Analytics / Instrumentation
| Evidence ID | Event / Signal | Purpose | Trigger Point | Verified On | Confidence | Gap / Risk |
|---|---|---|---|---|---|---|
| METRIC-01 | normalized ACP tool names -> `ACPProgressClass` | drive watchdog classification | `ExecutionEventBridge.progressClass(for:)` | 2026-04-11 | High | Proposal's initial mapping currently disagrees with the live classifier. |
| METRIC-02 | watchdog/read-loop runtime tests | prove weak-progress stall behavior | `RuntimeAgentExecutorTests` read-loop fixtures | 2026-04-11 | High | Existing proof surfaces already embody the weak/discovery taxonomy. |

## K. Testing Strategy
| Evidence ID | Layer | Covered Surface | Current Coverage | Proposed Additions | Verified On | Confidence | Gap / Risk |
|---|---|---|---|---|---|---|---|
| TEST-01 | runtime unit tests | read-loop weak-progress watchdog | existing tests already use repeated `search` churn as the failing weak-progress shape | retain as proposal proof, unless the proposal explicitly migrates the taxonomy | 2026-04-11 | High | Current proof currently contradicts the draft mapping. |
| TEST-02 | runtime/unit + orchestrator gate | proposal-owned proof lane | `proposal-037` already exists in script and stable gate docs | keep and evolve coverage; do not frame lane creation as future work | 2026-04-11 | High | Proof-lane ownership is already landed. |
| TEST-03 | stable truth documentation | execution-truth stable refs | no stable-doc proof currently exists for watchdog-specific truth promotion | add explicit doc-promotion ownership in proposal | 2026-04-11 | High | Stable dependency chain remains incomplete without this. |

## L. Current Repo Reality / Contradictions
| Evidence ID | Repo Surface | Proposal Claim | Current Repo Reality | Verified On | Confidence | Implication |
|---|---|---|---|---|---|---|
| REAL-01 | shared progress classifier | `§10.2` says `search`, `execute`, `permission:execute` are strong non-mutating progress | both `ExecutionEventBridge` and `RuntimeAgentExecutor` still classify those tool names as weak/discovery | 2026-04-11 | High | Main live proposal-first blocker. |
| REAL-02 | runtime proof surfaces | proposal still implies the strong/non-mutating mapping is canonical | current watchdog proof uses repeated `search` churn as the weak/read-loop failure shape | 2026-04-11 | High | The contradiction is already visible in current tests, not just in helper code. |
| REAL-03 | implementation slice framing | `§11` still says "add `supervisionClassification` on `AgentExecution`" | current durable model already includes `supervisionClassification` | 2026-04-11 | High | Proposal implementation plan is partially stale. |
| REAL-04 | proof-lane framing | `§11` still says "add `proposal-037` gate" | current repo already has `PROPOSAL_037_TESTS` and a `proposal-037|p037` case | 2026-04-11 | High | Proposal implementation plan is partially stale. |
| REAL-05 | stable dependency chain | proposal depends on watchdog-specific truth and read order | `execution-truth-and-recovery.md` still documents generic execution-truth precedence only | 2026-04-11 | High | Proposal does not yet fully own stable-doc promotion. |

## M. Proposal Completeness Matrix
| Dimension | Status (`Specified | Partial | Missing | Contradicted by repo | Deferred intentionally`) | Evidence IDs | Notes |
|---|---|---|---|
| Problem / user intent | Specified | DOC-01 | Problem statement is clear. |
| Scope boundaries | Specified | DOC-01 | Scope/out-of-scope lines are clear. |
| Reusable baseline coverage | Partial | DOC-02, DOC-03, DOC-05 | Intake baseline is stale, so targeted ACP refresh was required. |
| Screen / surface definition | Specified | NAV-01, NAV-02, NAV-03 | Surface ownership is no longer a blocker. |
| Navigation / entry points | Specified | NAV-01 | Run-detail timeline path is explicit. |
| State handling | Partial | H matrix | Main remaining gap is the taxonomy contradiction in loading/read-loop semantics. |
| Data / API contract | Partial | DATA-01, DATA-02, DATA-03 | Persistence choice is good, but stable-doc promotion is still incomplete. |
| Persistence / caching | Specified | DATA-02 | Durable attempt storage is clear. |
| Permissions / auth expiry | Deferred intentionally | DOC-01 | Not needed for this slice. |
| Feature flags / rollout / rollback | Partial | FLAG-01 | No staged rollout plan. |
| Analytics / instrumentation | Partial | METRIC-01, METRIC-02 | Current telemetry owner is clear, but proposal mapping still conflicts with it. |
| Testing strategy | Specified | TEST-01, TEST-02 | Proof-lane ownership is explicit and already landed. |
| Dependencies / integration points | Partial | DOC-05, INT-04 | Stable execution-truth docs still need promotion. |

## N. Assumptions, Open Questions, and Blockers
- ASSUMP-01: `P037` is a delta/migration proposal over an already-landed ACP supervision substrate, not a greenfield executor slice.
- ASSUMP-02: `ExecutionEventBridge` remains the shared classifier owner unless the proposal explicitly chooses and proves a migration away from current semantics.
- QUESTION-01: Should `search`, `execute`, and `permission:execute` remain weak/discovery progress, or is `P037` intentionally migrating them to strong progress?
- BLOCKER-01: `§10.2` still contradicts the current shared classifier and read-loop proof surfaces.
- BLOCKER-02: `§11` still mixes already-landed work with future slices and does not yet fully own stable-doc promotion into `docs/reference/execution-truth-and-recovery.md`.

## O. Research Triggers / External Questions
No external research trigger was required for this pass. Local proposal/docs/code/baseline evidence was sufficient for a proposal-first verdict.
