# Proposal Evidence Pack

## A. Repo-Local Proposal / Document Inventory
| Evidence ID | Source / Path / Artifact | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|
| DOC-01 | `/Users/user/Documents/Chainworks Forge/docs/proposals/026-acp-first-runtime-transport-and-goose-decoupling.md` | 2026-04-05 | High | The main `P026` text now explicitly narrows `runtime_profile` to repo-owned runtime intent, splits requested/predicted/actual truth across concrete owners, scopes bridge degradations away from the default Goose path, and anchors report/recovery continuity to current shell-owned readers. | The review could judge stale proposal text. | Primary proposal source. |
| DOC-02 | `/Users/user/Documents/Chainworks Forge/docs/proposals/026-acp-runtime-plan-additive-profiles.md` | 2026-04-05 | High | The depended-on additive-profiles note now aligns with the main proposal: `runtime_profiles` carry repo-owned identity/capability fields and explicitly exclude machine-local launch/bootstrap authority. | Cross-doc alignment could be judged stale. | Confirms the last live blocker is closed. |
| DOC-03 | `/Users/user/Documents/Chainworks Forge/docs/reference/goose-server-transport.md` | 2026-04-05 | High | Current Goose runtime still owns concrete start / update / reply / extension mutation mechanics. | The proposal could be judged against the wrong current seam. | Grounds the transport migration section. |
| DOC-04 | `/Users/user/Documents/Chainworks Forge/docs/reference/live-provider-execution-slice.md` | 2026-04-05 | High | Current live execution keeps app-owned control-plane truth above Goose-backed runtime mechanics. | Core transport ownership could be overstated or understated. | Needed for transport-neutrality review. |
| DOC-05 | `/Users/user/Documents/Chainworks Forge/docs/reference/provider-platform.md` | 2026-04-05 | High | Machine-local runtime settings, configured providers, transport, secrets, and preflight live under provider-platform ownership, not repo YAML. | Runtime-profile authority could be assigned to the wrong layer. | Core owner-boundary evidence. |
| DOC-06 | `/Users/user/Documents/Chainworks Forge/docs/reference/provider-binding-truth.md` | 2026-04-05 | High | Frozen provider/runtime binding truth must come from run-start snapshots, not later reconstruction from mutable settings. | Runtime selection freezing could be misowned. | Needed for success-criteria and run-snapshot review. |
| DOC-07 | `/Users/user/Documents/Chainworks Forge/docs/reference/execution-truth-and-recovery.md` | 2026-04-05 | High | Current repo already separates frozen truth, per-attempt runtime truth, and report/recovery reader precedence. | Requested/predicted/actual truth split could be misjudged. | Needed for Section `8.1` / `11` review. |
| DOC-08 | `/Users/user/Documents/Chainworks Forge/docs/reference/operator-experience.md` | 2026-04-05 | High | Reports, comparison, and recovery already belong to the existing shell-owned operator spine. | Runtime-neutral reporting continuity could be left unanchored. | Needed for report / recovery continuity review. |

## B. Reusable Baseline Inputs
| Evidence ID | Artifact / Slice | Status (`Reused | Partially refreshed | Missing`) | Covered Surfaces | Verified On | Confidence | Freshness Notes | Relevance |
|---|---|---|---|---|---|---|---|
| BASE-01 | `/Users/user/Documents/Chainworks Forge/.review-baselines/current-system-baseline.md` | Reused | product type, operator shell, provider/platform, runtime truth, reporting ownership | 2026-04-05 | High | Still fresh for current owner boundaries. | Review entry point. |
| BASE-02 | `/Users/user/Documents/Chainworks Forge/docs/reference/current-system-baseline.md` | Reused | stable subsystem map and reference-doc pointers | 2026-04-05 | High | Still fresh for locating stable docs. | Confirms which stable refs govern touched surfaces. |
| BASE-03 | Proposal-local integration context | Partially refreshed | transport seam, provider-platform settings, frozen run truth, operator shell readers, depended-on companion doc | 2026-04-05 | High | Fresh reads were needed because the main proposal changed and the companion note then caught up. | Explains the targeted refresh in this pass. |

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
- Open questions:
  - should the draft add a lightweight proof-owner section, or leave that to implementation audit discipline later?
- Blockers:
  - none for proposal-readiness

## D. Affected Screens / Navigation / Entry-Point Slice
| Evidence ID | Screen / Surface / Entry Point | Source (`Baseline | Targeted refresh | Proposal`) | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|---|
| NAV-01 | `RunsHomeView` | Baseline | 2026-04-05 | High | `RunsHomeView` is the operator landing surface and exposes runtime provenance / contextual actions today. | Runtime-neutral reporting continuity could be routed to the wrong place. | Confirms shell ownership context. |
| NAV-02 | `RunReportView` | Baseline + targeted refresh | 2026-04-05 | High | Current report surfaces already read persisted run-level KPI/report JSON. | A second runtime diagnostics/report lane could be introduced accidentally. | Needed for report continuity review. |
| NAV-03 | `RunComparisonView` | Baseline + targeted refresh | 2026-04-05 | High | Comparison is already shell-owned and deterministic. | Runtime-neutral comparison truth could drift into a parallel surface. | Needed for operator-surface continuity review. |
| NAV-04 | `RecoverySheet` / `BlockedRunRecoveryView` | Baseline + targeted refresh | 2026-04-05 | High | Recovery remains an existing shell-owned surface over persisted truth. | Transport migration could mistakenly add a second recovery/debug lane. | Needed for continuity review. |
| NAV-05 | Provider readiness / platform surfaces | Baseline | 2026-04-05 | High | Preflight and provider diagnostics are already machine-local, prelaunch-owned surfaces. | Runtime capability truth could be misassigned to repo YAML. | Needed for runtime-profile authority review. |

## E. Impacted Modules / Code-Path Map
| Evidence ID | File Path / Module / Symbol | Layer | Role in Flow | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|---|---|
| MAP-01 | `/Users/user/Documents/Chainworks Forge/Chainworks Forge/DSL/AgentCatalog.swift` | DSL | Current catalog schema owner | 2026-04-05 | High | The current catalog already owns `mcp_server_registry`, `mcp_profiles`, `backend_profiles`, and `permission_profiles`, but it does not yet own `runtime_profiles`. | Proposed DSL delta could be misstated. | Grounds runtime-profile discussion. |
| MAP-02 | `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Providers/ConfiguredProvider.swift` | Provider platform | Machine-local configured runtime settings | 2026-04-05 | High | `ConfiguredProvider` already owns `transport`, `endpoint`, auth mode, capabilities, and adapter version as machine-local settings. | Runtime-profile authority could be assigned to the wrong owner. | Core owner-boundary evidence. |
| MAP-03 | `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Providers/BackendProfileResolverV2.swift` | Provider binding | Frozen resolved provider binding builder | 2026-04-05 | High | Current backend-profile resolution already freezes provider, model, effort, transport, and adapter version into a resolved binding. | Runtime-profile freeze location could be left ambiguous. | Needed for `RunStartSnapshot` / success-criteria review. |
| MAP-04 | `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Engine/GooseTransport.swift` | Transport contract | Current protocol and payload seam | 2026-04-05 | High | The protocol and payload types are still Goose-shaped: `GooseTransportProtocol`, `GooseSessionRequest`, `GoosePromptRequest`, `GooseSessionRuntimeState`. | Motivation or migration seam could be misread. | Grounds the transport-neutrality problem statement. |
| MAP-05 | `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Engine/GooseSessionBridge.swift` | Runtime bridge | Current canonical session bootstrap and prompt path | 2026-04-05 | High | Core execution still routes through a Goose-named session bridge using the Goose-shaped transport protocol. | Seam extraction scope could be misstated. | Needed for additive Goose bridge review. |
| MAP-06 | `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Engine/ExecutionService.swift` | Engine | Live runtime configuration owner and executor selection | 2026-04-05 | High | `LiveRuntimeConfiguration` still sits above transport execution and reflects machine-local runtime/bootstrap state. | Proposal could incorrectly move this truth into catalog YAML. | Supports machine-local authority review. |
| MAP-07 | `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Engine/RunStartSnapshot.swift` | Persistence | Frozen run-start snapshot owner | 2026-04-05 | High | `RunStartSnapshot` persists provider binding snapshot, provenance, resolved skills, resolved MCP policies, and other frozen run-start truth. | Runtime-profile freeze owner could be left ambiguous. | Core owner-boundary evidence. |
| MAP-08 | `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Models/Run.swift` | Persistence | Run-level frozen truth and KPI/report owners | 2026-04-05 | High | `Run` still owns `providerBindingSnapshotJSON`, `bindingProvenanceJSON`, `sessionKPIExportJSON`, and `sessionLineageReportJSON`. | Reports may be judged against the wrong persistence owner. | Needed for runtime selection and report continuity review. |
| MAP-09 | `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Models/AgentExecution.swift` | Persistence | Per-attempt runtime-settled truth owner | 2026-04-05 | High | `AgentExecution` already persists `runtimeProvider`, `runtimeModel`, and `adapterVersion`. | Actual runtime-settled truth could be misowned. | Needed for requested/predicted/actual truth split review. |
| MAP-10 | `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Views/RunReportView.swift` | Operator UI | Shell-owned report reader | 2026-04-05 | High | `RunReportView` already reads persisted KPI/report data from `Run`. | A second runtime report lane could be opened accidentally. | Grounds report continuity review. |
| MAP-11 | `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Views/RunComparisonView.swift` | Operator UI | Shell-owned comparison reader | 2026-04-05 | High | Current comparison remains a shell-owned consumer of persisted run truth. | Runtime-neutral comparison could drift into a side lane. | Grounds operator continuity review. |
| MAP-12 | `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Views/RecoverySheet.swift` | Operator UI | Shell-owned recovery surface | 2026-04-05 | High | Recovery already has a current shell-owned home. | Runtime migration could create an unanchored recovery/debug surface. | Grounds operator continuity review. |
| MAP-13 | `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Views/BlockedRunRecoveryView.swift` | Operator UI | Shell-owned blocked-run recovery surface | 2026-04-05 | High | Blocked-run recovery remains part of the existing operator shell. | Same risk as above. | Grounds operator continuity review. |

## F. Data / API / Persistence / Auth Touchpoints
| Evidence ID | Touchpoint | File / Module / Doc | Direction | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|---|---|
| DATA-01 | Machine-local runtime configuration | `ConfiguredProvider`, `ExecutionService.LiveRuntimeConfiguration`, `Chainworks_ForgeApp.loadLiveRuntimeConfiguration` | Settings -> runtime bootstrap | 2026-04-05 | High | Startup transport, endpoints, auth, and live runtime bootstrap are already machine-local concerns. | Proposal could incorrectly move installed-runtime truth into repo YAML. | Core owner-boundary evidence. |
| DATA-02 | Frozen provider/runtime binding | `BackendProfileResolverV2`, `RunStartSnapshot`, `Run.providerBindingSnapshotJSON`, `Run.bindingProvenanceJSON` | Run start -> persisted truth | 2026-04-05 | High | Current repo already freezes binding selection and provenance at run start. | Runtime-profile selection could be frozen nowhere or in multiple places. | Core owner-boundary evidence. |
| DATA-03 | Actual runtime-settled truth | `AgentExecution.runtimeProvider`, `runtimeModel`, `adapterVersion` | Runtime -> per-attempt persistence | 2026-04-05 | High | Actual runtime truth already persists per attempt, not on preflight or catalog records. | Proposal could overclaim preflight or report authority. | Core owner-boundary evidence. |
| DATA-04 | Run-level report/KPI export | `Run.sessionKPIExportJSON`, `Run.sessionLineageReportJSON`, `RunReportView` | Persisted truth -> shell readers | 2026-04-05 | High | Current run-owned KPI/report lane already exists and is shell-owned. | Runtime-neutral reporting continuity could fragment. | Supports operator-surface continuity review. |
| DATA-05 | `runtime_profiles` example contract in depended-on note | `docs/proposals/026-acp-runtime-plan-additive-profiles.md` | Proposal guidance -> implementation interpretation | 2026-04-05 | High | The companion note now uses owner-safe `adapter_family` / capability examples and omits machine-local launch fields from `runtime_profiles`. | Cross-doc alignment could be overstated if the note later drifts again. | Confirms the main contract is now reinforced rather than contradicted. |

## G. Current Host-System Integration Surfaces
| Evidence ID | Surface / Seam / Owner | Source (`Baseline | Targeted refresh | Current repo`) | Verified On | Confidence | Key Fact | Conflict / Proposal Risk | Relevance |
|---|---|---|---|---|---|---|---|
| INT-01 | Provider-platform machine-local settings | Baseline + current repo | 2026-04-05 | High | Runtime/bootstrap truth is already machine-local and secret-safe. | The companion note now preserves that same split rather than pulling launch command into `runtime_profiles`. | Closed cross-doc blocker. |
| INT-02 | Frozen run-start ownership | Baseline + current repo | 2026-04-05 | High | `RunStartSnapshot` and `Run` already freeze provider/runtime-adjacent truth at run start. | Main `P026` now matches this split well enough. | Closed main-text blocker. |
| INT-03 | Per-attempt runtime settlement | Baseline + current repo | 2026-04-05 | High | `AgentExecution` already holds runtime-settled truth that reports later read. | Main `P026` now names this owner explicitly. | Closed main-text blocker. |
| INT-04 | Shell-owned report / recovery spine | Baseline + current repo | 2026-04-05 | High | Existing operator shell already owns run reports, comparison, and recovery surfaces. | Main `P026` now anchors continuity to those readers explicitly. | Closed main-text blocker. |
| INT-05 | Default Goose runtime bridge | Current repo | 2026-04-05 | High | Goose is still the current default runtime path and current live execution substrate. | Main `P026` now scopes degradation away from that preserved default path. | Closed main-text blocker. |

## H. State Coverage Matrix
| State | Proposal Status (`Specified | Partial | Missing | Contradicted by repo | Deferred intentionally`) | Evidence IDs | Repo Touchpoints | Notes / Risks |
|---|---|---|---|---|
| Entry | Specified | DOC-01, MAP-04, MAP-05 | Goose-shaped transport seam | Problem statement remains concrete and grounded. |
| Happy path | Specified | DOC-01, MAP-03, MAP-07, MAP-09 | binding freeze + execution truth | Main proposal now states the owner split clearly enough. |
| Loading | Deferred intentionally | DOC-01 | N/A | No UI loading-state design is in scope. |
| Empty | Deferred intentionally | DOC-01 | N/A | No separate empty-state slice is central here. |
| Validation error | Specified | DOC-01, DOC-05, DOC-07, DATA-02, DATA-03 | preflight + runtime truth | Requested/predicted/actual split is now explicit in the main proposal. |
| Backend error | Specified | DOC-01, MAP-04, MAP-05, MAP-09 | Goose bridge / runtime attempts | Main proposal now scopes bridge degradations away from the default Goose path. |
| Offline / degraded | Specified | DOC-01, DOC-02, DOC-05, INT-01, INT-05 | machine-local runtime + default Goose | Main proposal and companion note now preserve the same machine-local/bootstrap boundary. |
| Retry / recovery | Specified | DOC-01, DOC-07, DOC-08, INT-04 | persisted truth readers | Main proposal now anchors these readers explicitly. |
| Auth / permission expiry | Deferred intentionally | DOC-05 | provider-platform | Auth lifecycle remains provider-platform scope. |
| Rollback / cancellation | Specified | DOC-01, DOC-07, INT-05 | recovery / Goose bridge | Main proposal’s default-Goose guardrails now read coherently. |

## I. Feature Flags / Rollout / Rollback
| Evidence ID | Mechanism / Flag | Scope | Rollout Plan | Rollback Path | Verified On | Confidence | Notes |
|---|---|---|---|---|---|---|---|
| FLAG-01 | Additive phased rollout | whole runtime slice | Partial | Partial | 2026-04-05 | Medium | Rollout remains additive; no live proposal blocker remains here. |

## J. Analytics / Instrumentation
| Evidence ID | Event / Signal | Purpose | Trigger Point | Verified On | Confidence | Gap / Risk |
|---|---|---|---|---|---|---|
| METRIC-01 | Existing run-owned KPI/report lane | Persisted report/comparison truth | `Run.sessionKPIExportJSON`, `Run.sessionLineageReportJSON` | 2026-04-05 | High | Main proposal now anchors continuity to the current shell-owned readers. No live metrics-lane blocker remains. |

## K. Testing Strategy
| Evidence ID | Layer | Covered Surface | Current Coverage | Proposed Additions | Verified On | Confidence | Gap / Risk |
|---|---|---|---|---|---|---|---|
| TEST-01 | Proposal-defined | additive Goose bridge, first-wave ACP runtimes, frozen runtime selection, report/recovery continuity | Directional only | Partial | 2026-04-05 | Medium | Main proposal proof-focus is now materially better; no separate test-owner blocker is live in this pass. |

## L. Current Repo Reality / Contradictions
| Evidence ID | Repo Surface | Proposal Claim | Current Repo Reality | Verified On | Confidence | Implication |
|---|---|---|---|---|---|---|
| REAL-01 | Main `P026` runtime-profile ownership | `runtime_profile` is repo-owned runtime intent, not machine-local launch authority | This now matches current provider-platform ownership | 2026-04-05 | High | Closed. |
| REAL-02 | Main `P026` requested/predicted/actual truth split | Catalog, preflight, `RunStartSnapshot`, `AgentExecution`, and shell readers now have explicit roles | This now matches current frozen-versus-runtime owner boundaries closely enough for readiness | 2026-04-05 | High | Closed. |
| REAL-03 | Main `P026` additive Goose bridge posture | Bridge degradations are explicitly scoped away from the preserved default Goose path | This now matches the default-Goose guardrail the repo requires | 2026-04-05 | High | Closed. |
| REAL-04 | Main `P026` report / recovery continuity | Current shell-owned readers are now named explicitly | This now matches the operator shell spine | 2026-04-05 | High | Closed. |
| REAL-05 | Depended-on additive runtime-profile note | `runtime_profiles` now carry identity/capability examples and explicitly exclude machine-local launch/bootstrap authority | Current repo still keeps launch/bootstrap truth machine-local | 2026-04-05 | High | Closed. |

## M. Proposal Completeness Matrix
| Dimension | Status (`Specified | Partial | Missing | Contradicted by repo | Deferred intentionally`) | Evidence IDs | Notes |
|---|---|---|---|
| Problem / user intent | Specified | DOC-01, DOC-03, MAP-04, MAP-05 | Motivation is strong and grounded in current code. |
| Scope boundaries | Specified | DOC-01 | In/out-of-scope boundaries are explicit. |
| Reusable baseline coverage | Specified | DOC-03..08, BASE-01..03, INT-01..05 | Main proposal now reuses the right baselines well. |
| Screen / surface definition | Specified | DOC-08, NAV-01..04, INT-04 | Main proposal now anchors shell-owned continuity explicitly. |
| Navigation / entry points | Specified | NAV-01..05 | Main proposal now places continuity on the current shell-owned surfaces. |
| State handling | Specified | H matrix, REAL-01..05 | Main proposal and its depended-on companion now preserve the same owner split. |
| Data / API contract | Specified | MAP-01..09, DATA-01..05, REAL-01..05 | Core runtime transport direction and owner boundaries are now explicit enough for implementation. |
| Persistence / caching | Specified | MAP-07..09, DATA-02..04, REAL-02 | Main proposal now states the frozen-versus-runtime split clearly enough. |
| Permissions / auth expiry | Deferred intentionally | DOC-05 | Remains provider-platform scope. |
| Feature flags / rollout / rollback | Partial | FLAG-01 | Migration steps exist, but guarded rollout/rollback remains less explicit than the core owner contract. |
| Analytics / instrumentation | Specified | METRIC-01 | No live blocker remains here. |
| Testing strategy | Partial | TEST-01 | Directional but adequate for proposal-readiness. |
| Dependencies / integration points | Specified | DOC-02, MAP-01..13, INT-01..05 | The depended-on document now aligns with the main owner split closely enough for readiness. |

## N. Assumptions, Open Questions, and Blockers
- ASSUMP-01: Goose remains the default runtime path through the first additive wave.
- ASSUMP-02: Provider-platform settings remain the machine-local owner for transport/bootstrap details and secrets.
- ASSUMP-03: Current report / comparison / recovery readers remain the operator shell spine for post-run runtime truth.
- QUESTION-01: Should the draft add a lightweight proof-owner section now, or leave that to implementation audit discipline later?
- BLOCKER-01: None

## O. Research Triggers / External Questions
| Trigger ID | Trigger Type (`Proposal gap | Baseline constraint | Host-system integration risk | Unresolved tradeoff`) | Local Evidence IDs | Question to Research | Why Local Evidence Is Not Enough | Time Sensitivity / Freshness Risk |
|---|---|---|---|---|---|
| RSH-01 | Unresolved tradeoff | TEST-01 | None required yet. The remaining open point is optional proof-owner hygiene, not a blocking architecture gap. | Repo-local baselines and code are sufficient for this proposal-readiness pass. | Low |
