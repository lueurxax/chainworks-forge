# Proposal Evidence Pack

## A. Repo-Local Proposal / Document Inventory
| Evidence ID | Source / Path / Artifact | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|
| DOC-01 | `/Users/user/Documents/Chainworks Forge/docs/proposals/026-acp-first-runtime-transport-and-goose-decoupling.md` | 2026-04-07 | High | The main `P026` text still explicitly narrows `runtime_profile` to repo-owned runtime intent, splits requested/predicted/actual truth across concrete owners, scopes bridge degradations away from the default Goose path, and anchors report/recovery continuity to current shell-owned readers. | The review could judge stale proposal text. | Primary proposal source. |
| DOC-02 | `/Users/user/Documents/Chainworks Forge/docs/proposals/026-acp-runtime-plan-additive-profiles.md` | 2026-04-07 | High | The depended-on additive-profiles note remains aligned with the main proposal: `runtime_profiles` carry repo-owned identity/capability fields and explicitly exclude machine-local launch/bootstrap authority. | Cross-doc alignment could be judged stale. | Confirms the last prior blocker stays closed. |
| DOC-03 | `/Users/user/Documents/Chainworks Forge/docs/evidence/codex-acp-research.md` | 2026-04-07 | High | `codex-acp` is execution-proven and credible, but still lacks enough tool/permission/MCP proof to displace Claude Agent ACP or Gemini CLI ACP in the first additive wave. | First-wave candidate posture could be overstated or understated. | New depended-on evidence in this round. |
| DOC-04 | `/Users/user/Documents/Chainworks Forge/docs/evidence/acp-runtime-candidate-comparison.md` | 2026-04-07 | High | The refreshed candidate ranking remains: Claude Agent ACP first, Gemini CLI ACP second, Codex ACP third. | The proposal could be judged against stale candidate posture. | Validates that the new Codex evidence does not reopen rollout posture. |
| DOC-05 | `/Users/user/Documents/Chainworks Forge/docs/reference/runtime-contract.md` | 2026-04-07 | High | Current baseline still expects frozen run snapshots and stable runtime boundary truth above transport details. | Run-start ownership could be judged against the wrong baseline. | Stable runtime-boundary reference. |
| DOC-06 | `/Users/user/Documents/Chainworks Forge/.review-baselines/current-system-baseline.md` | 2026-04-07 | High | The reusable review baseline still treats provider/platform, operator shell, and Goose-backed live execution as stable current-system slices. | Baseline freshness could be overstated. | Review entry point. |
| DOC-07 | `/Users/user/Documents/Chainworks Forge/docs/reference/current-system-baseline.md` | 2026-04-07 | High | The promoted baseline still maps provider platform, Goose transport, execution truth, and operator shell as current stable references. | Stable reference map could be judged stale. | Confirms reference-doc posture. |
| DOC-08 | `/Users/user/Documents/Chainworks Forge/docs/reviews/026-acp-first-runtime-transport-and-goose-decoupling-review.md` | 2026-04-07 | High | The last green review already closed the owner-boundary blockers; this pass is validating whether the new dependency evidence reopens any of them. | The current pass could accidentally ignore prior closure history. | Prior round continuity. |
| DOC-09 | `/Users/user/Documents/Chainworks Forge/docs/reviews/026-acp-first-runtime-transport-and-goose-decoupling-evidence-pack.md` | 2026-04-07 | High | The previous evidence pack remains largely fresh for owner seams and shell readers, but needed targeted refresh for the new `codex-acp` evidence and refreshed comparison file. | Evidence reuse could become stale. | Prior round continuity. |

## B. Reusable Baseline Inputs
| Evidence ID | Artifact / Slice | Status (`Reused | Partially refreshed | Missing`) | Covered Surfaces | Verified On | Confidence | Freshness Notes | Relevance |
|---|---|---|---|---|---|---|---|
| BASE-01 | `/Users/user/Documents/Chainworks Forge/.review-baselines/current-system-baseline.md` | Reused | product type, operator shell, provider/platform, runtime truth, reporting ownership | 2026-04-07 | High | Still fresh for current owner boundaries. | Review entry point. |
| BASE-02 | `/Users/user/Documents/Chainworks Forge/docs/reference/current-system-baseline.md` | Reused | stable subsystem map and reference-doc pointers | 2026-04-07 | High | Still fresh for locating stable docs. | Confirms which stable refs govern touched surfaces. |
| BASE-03 | Prior `P026` review/evidence artifacts | Partially refreshed | owner split, shell reader continuity, default-Goose preservation | 2026-04-07 | High | Reused for closure history, but refreshed around candidate evidence and depended-on docs. | Explains why this reread stays local and bounded. |

## C. Scope, Out-of-Scope, and Intentional Deferrals
- In scope:
  - ACP-shaped runtime vocabulary in core
  - runtime-profile selection through catalog and backend profiles
  - Goose bridge posture as default runtime path
  - transport-neutral MCP intent and runtime realization split
  - run-start freezing and report/recovery continuity
- Out of scope:
  - implementation audit
  - external web research
  - broader proof execution
  - destructive Goose removal in the first wave
- Deferred intentionally:
  - second-wave runtimes after first-wave seam proof
  - long-tail parity for weaker ACP candidates
- Assumptions:
  - current provider-platform split remains authoritative
  - the existing operator shell remains the canonical report / comparison / recovery spine
  - `codex-acp` remains a second-tier candidate on current evidence
- Open questions:
  - should the draft add a lightweight proof-owner section, or leave that to implementation audit discipline later?
- Blockers:
  - none for proposal-readiness

## D. Affected Screens / Navigation / Entry-Point Slice
| Evidence ID | Screen / Surface / Entry Point | Source (`Baseline | Targeted refresh | Proposal`) | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|---|
| NAV-01 | `RunReportView` | Baseline + targeted refresh | 2026-04-07 | High | Current report surfaces already read persisted run-level KPI/report JSON. | A second runtime diagnostics/report lane could be introduced accidentally. | Needed for report continuity review. |
| NAV-02 | `RunComparisonView` | Baseline + targeted refresh | 2026-04-07 | High | Comparison is already shell-owned and deterministic. | Runtime-neutral comparison truth could drift into a parallel surface. | Needed for operator-surface continuity review. |
| NAV-03 | `RecoverySheet` | Baseline + targeted refresh | 2026-04-07 | High | Recovery remains an existing shell-owned surface over persisted truth. | Transport migration could mistakenly add a second recovery/debug lane. | Needed for continuity review. |
| NAV-04 | `BlockedRunRecoveryView` | Baseline + targeted refresh | 2026-04-07 | High | Blocked-run recovery remains part of the shell hierarchy, not a parallel tool surface. | Same risk as above. | Needed for continuity review. |
| NAV-05 | Provider readiness / platform surfaces | Baseline | 2026-04-07 | High | Preflight and provider diagnostics remain machine-local, prelaunch-owned surfaces. | Runtime capability truth could be misassigned to repo YAML. | Needed for runtime-profile authority review. |

## E. Impacted Modules / Code-Path Map
| Evidence ID | File Path / Module / Symbol | Layer | Role in Flow | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|---|---|
| MAP-01 | `/Users/user/Documents/Chainworks Forge/Chainworks Forge/DSL/AgentCatalog.swift` | DSL | Current catalog schema owner | 2026-04-07 | High | The current catalog still has no `runtime_profiles` owner, so the proposal remains a real DSL delta rather than stale prose. | Proposed catalog change could be misstated. | Grounds runtime-profile discussion. |
| MAP-02 | `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Providers/ConfiguredProvider.swift` | Provider platform | Machine-local configured runtime settings | 2026-04-07 | High | `ConfiguredProvider` still owns `transport`, `endpoint`, auth mode, capabilities, and adapter version as machine-local settings. | Runtime-profile authority could be assigned to the wrong owner. | Core owner-boundary evidence. |
| MAP-03 | `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Providers/BackendProfileResolverV2.swift` | Provider binding | Frozen resolved provider binding builder | 2026-04-07 | High | Current backend-profile resolution still freezes provider, model, effort, transport, and adapter version into a resolved binding. | Runtime-profile freeze location could be left ambiguous. | Needed for run-start freezing review. |
| MAP-04 | `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Engine/GooseTransport.swift` | Transport contract | Current protocol and payload seam | 2026-04-07 | High | The transport seam is still Goose-shaped: `GooseTransportProtocol`, `GooseSessionRequest`, `GoosePromptRequest`, and Goose endpoint semantics remain first-class in core runtime code. | Motivation or migration seam could be misread. | Grounds the transport-neutrality problem statement. |
| MAP-05 | `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Engine/GooseSessionBridge.swift` | Runtime bridge | Current canonical session bootstrap and prompt path | 2026-04-07 | High | Core execution still routes through a Goose-named session bridge using the Goose-shaped transport protocol. | Seam extraction scope could be misstated. | Needed for additive Goose bridge review. |
| MAP-06 | `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Engine/ExecutionService.swift` | Engine | Live runtime configuration owner and executor selection | 2026-04-07 | High | `LiveRuntimeConfiguration` still sits above transport execution and reflects machine-local runtime/bootstrap state. | Proposal could incorrectly move this truth into catalog YAML. | Supports machine-local authority review. |
| MAP-07 | `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Engine/RunStartSnapshot.swift` | Persistence | Frozen run-start snapshot owner | 2026-04-07 | High | `RunStartSnapshot` still persists provider binding snapshot, provenance, resolved MCP policies, and other frozen run-start truth. | Runtime-profile freeze owner could be left ambiguous. | Core owner-boundary evidence. |
| MAP-08 | `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Models/Run.swift` | Persistence | Run-level frozen truth and KPI/report owners | 2026-04-07 | High | `Run` still owns `providerBindingSnapshotJSON`, `bindingProvenanceJSON`, `sessionKPIExportJSON`, and `sessionLineageReportJSON`. | Reports may be judged against the wrong persistence owner. | Needed for runtime selection and report continuity review. |
| MAP-09 | `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Models/AgentExecution.swift` | Persistence | Per-attempt runtime-settled truth owner | 2026-04-07 | High | `AgentExecution` still persists `runtimeProvider`, `runtimeModel`, `adapterVersion`, MCP settlement, and startup-latency fields. | Actual runtime-settled truth could be misowned. | Needed for requested/predicted/actual truth split review. |
| MAP-10 | `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Views/RunReportView.swift` | Operator UI | Shell-owned report reader | 2026-04-07 | High | `RunReportView` still reads persisted KPI/report data from `Run`. | A second runtime report lane could be opened accidentally. | Grounds report continuity review. |
| MAP-11 | `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Views/RunComparisonView.swift` | Operator UI | Shell-owned comparison reader | 2026-04-07 | High | Current comparison remains a shell-owned consumer of persisted run truth. | Runtime-neutral comparison could drift into a side lane. | Grounds operator continuity review. |
| MAP-12 | `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Views/RecoverySheet.swift` | Operator UI | Shell-owned recovery surface | 2026-04-07 | High | Recovery still has a current shell-owned home. | Runtime migration could create an unanchored recovery/debug surface. | Grounds operator continuity review. |
| MAP-13 | `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Views/BlockedRunRecoveryView.swift` | Operator UI | Shell-owned blocked-run recovery surface | 2026-04-07 | High | Blocked-run recovery remains hosted inside the shell hierarchy. | Same risk as above. | Grounds operator continuity review. |

## F. Data / API / Persistence / Auth Touchpoints
| Evidence ID | Touchpoint | File / Module / Doc | Direction | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|---|---|
| DATA-01 | Machine-local runtime configuration | `ConfiguredProvider`, `ExecutionService.LiveRuntimeConfiguration`, baseline refs | Settings -> runtime bootstrap | 2026-04-07 | High | Startup transport, endpoints, auth, and live runtime bootstrap are still machine-local concerns. | Proposal could incorrectly move installed-runtime truth into repo YAML. | Core owner-boundary evidence. |
| DATA-02 | Frozen provider/runtime binding | `BackendProfileResolverV2`, `RunStartSnapshot`, `Run.providerBindingSnapshotJSON`, `Run.bindingProvenanceJSON` | Run start -> persisted truth | 2026-04-07 | High | Current repo still freezes binding selection and provenance at run start. | Runtime-profile selection could be frozen nowhere or in multiple places. | Core owner-boundary evidence. |
| DATA-03 | Actual runtime-settled truth | `AgentExecution.runtimeProvider`, `runtimeModel`, `adapterVersion`, MCP settlement fields | Runtime -> per-attempt persistence | 2026-04-07 | High | Actual runtime truth still persists per attempt, not on preflight or catalog records. | Proposal could overclaim preflight or report authority. | Core owner-boundary evidence. |
| DATA-04 | Run-level report/KPI export | `Run.sessionKPIExportJSON`, `Run.sessionLineageReportJSON`, `RunReportView` | Persisted truth -> shell readers | 2026-04-07 | High | Current run-owned KPI/report lane already exists and remains shell-owned. | Runtime-neutral reporting continuity could fragment. | Supports operator-surface continuity review. |
| DATA-05 | Runtime-candidate posture | `codex-acp-research.md`, `acp-runtime-candidate-comparison.md` | Evidence -> first-wave target choice | 2026-04-07 | High | The new Codex evidence improves the field but still leaves Claude/Gemini as the strongest first-wave pair. | Proposal scope could be judged against stale candidate posture. | Supports Section `5.6` / `9.3` review. |

## G. Current Host-System Integration Surfaces
| Evidence ID | Surface / Seam / Owner | Source (`Baseline | Targeted refresh | Current repo`) | Verified On | Confidence | Key Fact | Conflict / Proposal Risk | Relevance |
|---|---|---|---|---|---|---|---|
| INT-01 | Provider-platform machine-local settings | Baseline + current repo | 2026-04-07 | High | Runtime/bootstrap truth remains machine-local and secret-safe. | No live conflict with the current main text or companion note. | Closed owner-boundary blocker stays closed. |
| INT-02 | Frozen run-start ownership | Baseline + current repo | 2026-04-07 | High | `RunStartSnapshot` and `Run` still freeze provider/runtime-adjacent truth at run start. | Main `P026` still matches this split. | Closed main-text blocker stays closed. |
| INT-03 | Per-attempt runtime settlement | Baseline + current repo | 2026-04-07 | High | `AgentExecution` still holds runtime-settled truth that reports later read. | Main `P026` still names this owner explicitly. | Closed main-text blocker stays closed. |
| INT-04 | Shell-owned report / recovery spine | Baseline + current repo | 2026-04-07 | High | Existing operator shell still owns run reports, comparison, and recovery surfaces. | Main `P026` still anchors continuity to those readers explicitly. | Closed main-text blocker stays closed. |
| INT-05 | Default Goose runtime bridge | Current repo | 2026-04-07 | High | Goose remains the current default runtime path and live execution substrate. | Main `P026` still scopes degradation away from that preserved default path. | Closed main-text blocker stays closed. |
| INT-06 | Expanded ACP candidate field | Refreshed evidence docs | 2026-04-07 | High | `codex-acp` is now a credible active candidate, but not yet a first-wave replacement. | Proposal could have gone stale if the field ranking changed materially. | Confirms the proposal still matches the best available local evidence. |

## H. State Coverage Matrix
| State | Proposal Status (`Specified | Partial | Missing | Contradicted by repo | Deferred intentionally`) | Evidence IDs | Repo Touchpoints | Notes / Risks |
|---|---|---|---|---|
| Entry | Specified | DOC-01, MAP-04, MAP-05 | Goose-shaped transport seam | Problem statement remains concrete and grounded. |
| Happy path | Specified | DOC-01, MAP-03, MAP-07, MAP-09 | binding freeze + execution truth | Main proposal still states the owner split clearly enough. |
| Loading | Deferred intentionally | DOC-01 | N/A | No UI loading-state design is in scope. |
| Empty | Deferred intentionally | DOC-01 | N/A | No separate empty-state slice is central here. |
| Validation error | Specified | DOC-01, DATA-01, DATA-02, DATA-03 | preflight + runtime truth | Requested/predicted/actual split remains explicit. |
| Backend error | Specified | DOC-01, MAP-04, MAP-05, MAP-09 | Goose bridge / runtime attempts | Bridge degradations remain scoped away from the default Goose path. |
| Offline / degraded | Specified | DOC-01, DOC-02, DATA-01, INT-01, INT-05 | machine-local runtime + default Goose | Main proposal and companion note still preserve the same machine-local/bootstrap boundary. |
| Retry / recovery | Specified | DOC-01, INT-04 | persisted truth readers | Main proposal still anchors these readers explicitly. |
| Auth / permission expiry | Deferred intentionally | DATA-01 | provider-platform | Auth lifecycle remains provider-platform scope. |
| Rollback / cancellation | Specified | DOC-01, INT-05 | recovery / Goose bridge | Default-Goose guardrails remain coherent. |

## I. Feature Flags / Rollout / Rollback
| Evidence ID | Mechanism / Flag | Scope | Rollout Plan | Rollback Path | Verified On | Confidence | Notes |
|---|---|---|---|---|---|---|---|
| FLAG-01 | Additive phased rollout | whole runtime slice | Partial | Partial | 2026-04-07 | Medium | Rollout remains additive; no live proposal blocker remains here. |

## J. Analytics / Instrumentation
| Evidence ID | Event / Signal | Purpose | Trigger Point | Verified On | Confidence | Gap / Risk |
|---|---|---|---|---|---|---|
| METRIC-01 | Existing run-owned KPI/report lane | Persisted report/comparison truth | `Run.sessionKPIExportJSON`, `Run.sessionLineageReportJSON` | 2026-04-07 | High | Main proposal still anchors continuity to the current shell-owned readers. No live metrics-lane blocker remains. |

## K. Testing Strategy
| Evidence ID | Layer | Covered Surface | Current Coverage | Proposed Additions | Verified On | Confidence | Gap / Risk |
|---|---|---|---|---|---|---|---|
| TEST-01 | Proposal-defined | additive Goose bridge, first-wave ACP runtimes, frozen runtime selection, report/recovery continuity | Directional only | Partial | 2026-04-07 | Medium | Main proposal proof-focus remains materially adequate for readiness. No separate test-owner blocker is live in this pass. |

## L. Current Repo Reality / Contradictions
| Evidence ID | Repo Surface | Proposal Claim | Current Repo Reality | Verified On | Confidence | Implication |
|---|---|---|---|---|---|---|
| REAL-01 | Main `P026` runtime-profile ownership | `runtime_profile` is repo-owned runtime intent, not machine-local launch authority | This still matches current provider-platform ownership | 2026-04-07 | High | Closed. |
| REAL-02 | Main `P026` requested/predicted/actual truth split | Catalog, preflight, `RunStartSnapshot`, `AgentExecution`, and shell readers have explicit roles | This still matches current frozen-versus-runtime owner boundaries closely enough for readiness | 2026-04-07 | High | Closed. |
| REAL-03 | Main `P026` additive Goose bridge posture | Bridge degradations are explicitly scoped away from the preserved default Goose path | This still matches the default-Goose guardrail the repo requires | 2026-04-07 | High | Closed. |
| REAL-04 | Main `P026` report / recovery continuity | Current shell-owned readers are named explicitly | This still matches the operator shell spine | 2026-04-07 | High | Closed. |
| REAL-05 | Depended-on additive runtime-profile note | `runtime_profiles` carry identity/capability examples and exclude machine-local launch/bootstrap authority | Current repo still keeps launch/bootstrap truth machine-local | 2026-04-07 | High | Closed. |
| REAL-06 | New Codex ACP dependency | The proposal can expand its evidence field without changing first-wave scope | Current comparison still ranks Codex behind Claude and Gemini for first-wave operator-grade fit | 2026-04-07 | High | No new contradiction. |

## M. Proposal Completeness Matrix
| Dimension | Status (`Specified | Partial | Missing | Contradicted by repo | Deferred intentionally`) | Evidence IDs | Notes |
|---|---|---|---|
| Problem / user intent | Specified | DOC-01, MAP-04, MAP-05 | Motivation remains strong and grounded in current code. |
| Scope boundaries | Specified | DOC-01 | In/out-of-scope boundaries remain explicit. |
| Reusable baseline coverage | Specified | DOC-05..09, BASE-01..03, INT-01..06 | Main proposal still reuses the right baselines well. |
| Screen / surface definition | Specified | NAV-01..05, INT-04 | Main proposal still anchors shell-owned continuity explicitly. |
| Navigation / entry points | Specified | NAV-01..05 | Continuity still sits on the current shell-owned surfaces. |
| State handling | Specified | H matrix, REAL-01..06 | Main proposal and its depended-on companion still preserve the same owner split. |
| Data / API contract | Specified | MAP-01..13, DATA-01..05, REAL-01..06 | Core runtime transport direction and owner boundaries remain explicit enough for implementation. |
| Persistence / caching | Specified | MAP-07..09, DATA-02..04, REAL-02 | Frozen-versus-runtime split remains clear enough. |
| Permissions / auth expiry | Deferred intentionally | DATA-01 | Remains provider-platform scope. |
| Feature flags / rollout / rollback | Partial | FLAG-01 | Migration steps exist, but guarded rollout/rollback remains less explicit than the core owner contract. |
| Analytics / instrumentation | Specified | METRIC-01 | No live blocker remains here. |
| Testing strategy | Partial | TEST-01 | Directional but adequate for proposal-readiness. |
| Dependencies / integration points | Specified | DOC-02..04, MAP-01..13, INT-01..06 | The new Codex evidence broadens dependencies without breaking the main contract. |

## N. Assumptions, Open Questions, and Blockers
- ASSUMP-01: Goose remains the default runtime path through the first additive wave.
- ASSUMP-02: Provider-platform settings remain the machine-local owner for transport/bootstrap details and secrets.
- ASSUMP-03: Current report / comparison / recovery readers remain the operator shell spine for post-run runtime truth.
- ASSUMP-04: `codex-acp` stays a second-tier candidate until tool/permission/MCP proof closes further.
- QUESTION-01: Should the draft add a lightweight proof-owner section now, or leave that to implementation audit discipline later?
- BLOCKER-01: None

## O. Research Triggers / External Questions
| Trigger ID | Trigger Type (`Proposal gap | Baseline constraint | Host-system integration risk | Unresolved tradeoff`) | Local Evidence IDs | Question to Research | Why Local Evidence Is Not Enough | Time Sensitivity / Freshness Risk |
|---|---|---|---|---|---|
| RSH-01 | Unresolved tradeoff | DOC-03, DOC-04, DATA-05, REAL-06 | None required yet. The new Codex evidence changes the comparison field, but not enough to force a new first-wave decision. | Repo-local evidence is currently sufficient for proposal-readiness. | Medium |
