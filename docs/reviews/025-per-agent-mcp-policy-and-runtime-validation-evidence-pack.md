# Proposal Evidence Pack

## A. Repo-Local Proposal / Document Inventory
| Evidence ID | Source / Path / Artifact | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|
| DOC-01 | `/Users/user/Documents/Chainworks Forge/docs/proposals/025-per-agent-mcp-policy-and-runtime-validation.md` | 2026-04-03 | High | `P025` introduces `mcp_server_registry`, `mcp_profiles`, per-agent `mcp_profile`, session reconciliation, preflight/runtime validation, diagnostics, and burn telemetry. | The review could judge stale proposal text. | Primary proposal source. |
| DOC-02 | `/Users/user/Documents/Chainworks Forge/docs/proposals/025-mcp-policy-review-notes.md` | 2026-04-03 | High | Earlier local notes already separate conceptual permission names from real Goose extension IDs and motivate a default-deny runtime policy. | Prior repo-local context could be ignored. | Confirms the proposal is solving a real local problem. |
| DOC-03 | `/Users/user/Documents/Chainworks Forge/docs/proposals/025-mcp-policy-review-notes-v2.md` | 2026-04-03 | High | The second note set broadens the intended MCP allocation for some agents but keeps installed-registry, fallback, and burn-telemetry requirements. | The review could miss current repo expectations for realistic agent allocations. | Helps judge whether the proposal is still too abstract or contradictory. |
| DOC-04 | `/Users/user/Documents/Chainworks Forge/docs/reference/goose-server-transport.md` | 2026-04-03 | High | Current Goose transport truth is session creation, provider binding, SSE execution, and blanket extension removal after `/agent/start`. | Runtime seams could be misread. | Needed for Section 5.4 / 7 review. |
| DOC-05 | `/Users/user/Documents/Chainworks Forge/docs/reference/provider-platform.md` | 2026-04-03 | High | Current runtime capability, diagnostics, and preflight truth are machine-local provider-platform concerns, not repo YAML ownership. | Capability authority could be assigned to the wrong layer. | Needed for registry/capability authority review. |
| DOC-06 | `/Users/user/Documents/Chainworks Forge/docs/reference/live-provider-execution-slice.md` | 2026-04-03 | High | The live slice keeps Goose as execution substrate while app code owns control-plane truth, start gating, and post-run inspectability. | Session/runtime owner split could be judged incorrectly. | Needed for session reconciliation and operator-inspection review. |
| DOC-07 | `/Users/user/Documents/Chainworks Forge/docs/reference/execution-truth-and-recovery.md` | 2026-04-03 | High | Current repo already separates frozen truth, runtime execution truth, and report-reader precedence; persisted truth must outrank live reconstruction. | Persistence-owner risks could be understated. | Needed for requested-vs-effective MCP truth review. |

## B. Reusable Baseline Inputs
| Evidence ID | Artifact / Slice | Status (`Reused | Partially refreshed | Missing`) | Covered Surfaces | Verified On | Confidence | Freshness Notes | Relevance |
|---|---|---|---|---|---|---|---|
| BASE-01 | `/Users/user/Documents/Chainworks Forge/.review-baselines/current-system-baseline.md` | Reused | product type, operator shell, provider/platform, live runtime, report/recovery ownership | 2026-04-03 | High | Still fresh for operator-shell and provider-platform ownership. | Review entry point. |
| BASE-02 | `/Users/user/Documents/Chainworks Forge/docs/reference/current-system-baseline.md` | Reused | stable subsystem map and canonical reference-doc pointers | 2026-04-03 | High | Still fresh for current repo ownership and stable references. | Confirms which stable docs own the touched surfaces. |
| BASE-03 | Proposal-local integration context | Missing | N/A | 2026-04-03 | High | No existing `P025` integration-context artifact existed; a narrow targeted refresh was enough. | Explains why this pass stayed repo-local. |

## C. Scope, Out-of-Scope, and Intentional Deferrals
- In scope:
  - per-agent MCP policy declaration
  - Goose session extension reconciliation
  - preflight/runtime validation ownership
  - requested-vs-effective MCP diagnostics
  - burn telemetry ownership
- Out of scope:
  - implementation audit
  - build/run/simulator proof
  - external web research
  - generic extension marketplace or editor UX
- Deferred intentionally:
  - non-Goose runtime implementation beyond capability hooks
  - interactive MCP policy editing UI
  - broader provider/model validation already covered by stable refs
- Assumptions:
  - current Goose runtime remains the immediate implementation target
  - current operator shell remains the canonical report/comparison/recovery spine
- Open questions:
  - should the draft add a lightweight proof-owner section now, or leave that to implementation audit discipline later?
- Blockers:
  - none for proposal-readiness

## D. Affected Screens / Navigation / Entry-Point Slice
| Evidence ID | Screen / Surface / Entry Point | Source (`Baseline | Targeted refresh | Proposal`) | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|---|
| NAV-01 | `PilotReadinessView` + `PreflightReportView` | Baseline + targeted refresh | 2026-04-03 | High | Current preflight output is still owned by provider/platform readiness surfaces before run start. | Preflight ownership could be assigned to the wrong surface. | Needed to judge Section 5.5. |
| NAV-02 | `RunReportView` | Baseline + targeted refresh | 2026-04-03 | High | Current run report is shell-owned and already consumes persisted KPI/report JSON lanes. | Telemetry/report risks could be understated. | Needed to judge Section 5.6 / 5.7. |
| NAV-03 | `RunComparisonView` | Baseline + targeted refresh | 2026-04-03 | High | Current comparison UI already treats proof owner, evaluation set, and normalized KPI data as shell-owned truth. | Recommendation/telemetry routing could drift. | Needed to judge telemetry ownership. |
| NAV-04 | `AgentCatalogView` | Targeted refresh | 2026-04-03 | Medium | The current catalog UI only shows backend and permission profile metadata; no MCP runtime truth is surfaced yet. | UI delta could be overstated. | Confirms the proposal still describes a real visibility gap. |

## E. Impacted Modules / Code-Path Map
| Evidence ID | File Path / Module / Symbol | Layer | Role in Flow | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|---|---|
| MAP-01 | `/Users/user/Documents/Chainworks Forge/Chainworks Forge/DSL/AgentCatalog.swift` | DSL | Current catalog schema owner | 2026-04-03 | High | Catalog currently has `permission_profiles.*.mcp.allow` only; there is no per-agent `mcp_profile` or explicit MCP registry yet. | Proposal delta could be misstated. | Core proposal seam. |
| MAP-02 | `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Engine/GooseTransport.swift` | Transport contract | Session request/response canonical types | 2026-04-03 | High | `GooseSessionRequest` has no MCP-extension-set field today; current canonical session contract only carries execution policy and metadata. | Reconciliation seam could be misread. | Needed for Section 5.4 review. |
| MAP-03 | `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Engine/GooseServerTransport.swift` | Runtime transport | Creates Goose sessions and removes all enabled extensions after `/agent/start` | 2026-04-03 | High | The current tactical runtime path is blanket removal, and extension state is extracted from start JSON rather than a durable app-owned registry model. | Proposal feasibility or owner split could be misjudged. | Direct grounding for motivation and migration. |
| MAP-04 | `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Engine/GooseSessionBridge.swift` | Runtime packet assembly | Builds session request and enforces start ordering | 2026-04-03 | High | `GooseSessionBridge` is the current canonical runtime-prepared execution path before prompt submission. | Session-reconciliation owner could drift. | Needed for Section 5.4 review. |
| MAP-05 | `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Engine/PreflightService.swift` | Preflight | Current prelaunch validation owner | 2026-04-03 | High | `PreflightService` validates files, provider bindings, workspace, and runtime health before start; it does not own actual post-session enabled state. | Section 5.5 / AC-5 could overclaim preflight authority. | Core authority seam. |
| MAP-06 | `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Models/Run.swift` | Persistence | Frozen run truth and report/KPI owners | 2026-04-03 | High | Current run-level telemetry/report truth lives on `Run` (`sessionKPIExportJSON`, `sessionLineageReportJSON`, frozen snapshots). | MCP telemetry owner could be assigned to the wrong layer. | Needed for Section 5.7 review. |
| MAP-07 | `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Models/AgentExecution.swift` | Persistence | Per-attempt runtime truth owner | 2026-04-03 | High | Current runtime/session truth lands per execution, while frozen intent and run-level summary stay on `Run`. | Requested-vs-effective MCP truth could be persisted on the wrong owner. | Needed for Section 5.5 / 5.6 review. |
| MAP-08 | `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Views/RunReportView.swift` | Operator UI | Shell-owned report consumer | 2026-04-03 | High | Current report surfaces already consume persisted KPI/report JSON rather than ad hoc diagnostics blobs. | Telemetry-surface drift could be understated. | Needed for Section 5.7 review. |
| MAP-09 | `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Views/RunComparisonView.swift` | Operator UI | Shell-owned comparison consumer | 2026-04-03 | High | Current comparison surfaces already expect proof owner, evaluation set, and normalized telemetry. | Recommendation/telemetry routing could be misjudged. | Needed for telemetry ownership review. |

## F. Data / API / Persistence / Auth Touchpoints
| Evidence ID | Touchpoint | File / Module / Doc | Direction | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|---|---|
| DATA-01 | Catalog MCP declarations | `AgentCatalog.swift`, proposal Sections 5-6 | Repo-local input | 2026-04-03 | High | Today only coarse `permission_profiles.*.mcp.allow` exists; the proposal introduces new repo-level MCP declarations. | Repo-level authority risk could be understated. | Core DSL seam. |
| DATA-02 | Goose session start data | `GooseServerTransport.createSession()` | Runtime response | 2026-04-03 | High | Current enabled extension names are discovered from `/agent/start` response JSON, then removed. | Actual installed/enabled truth could be conflated with static catalog truth. | Core runtime seam. |
| DATA-03 | Preflight report | `PreflightService`, `PreflightReport` | Prelaunch report | 2026-04-03 | High | Current preflight emits prelaunch checks only; it does not own settled session state. | Proposal could assign actual runtime truth to preflight incorrectly. | Needed for Section 5.5 review. |
| DATA-04 | Frozen run truth | `Run`, `RunStartSnapshot`, provider-platform/execution-truth refs | Persisted at run creation | 2026-04-03 | High | Current repo freezes intent and provenance on `Run` / `RunStartSnapshot`. | MCP requested-profile truth could be frozen on the wrong owner. | Core architecture seam. |
| DATA-05 | Per-attempt runtime truth | `AgentExecution`, execution-truth ref | Persisted during/after execution | 2026-04-03 | High | Current repo expects settled runtime truth to live per attempt and be readable by reports later. | Actual enabled-extension truth could be lost or misowned. | Core architecture seam. |
| DATA-06 | Run-level KPI/report export | `Run.sessionKPIExportJSON`, `Run.sessionLineageReportJSON`, `RunReportView`, `RunComparisonView` | Aggregated reporting | 2026-04-03 | High | The current KPI/report lane already exists and is shell-owned. | MCP burn telemetry could incorrectly open a second metrics lane. | Core telemetry seam. |

## G. Current Host-System Integration Surfaces
| Evidence ID | Surface / Seam / Owner | Source (`Baseline | Targeted refresh | Current repo`) | Verified On | Confidence | Key Fact | Conflict / Proposal Risk | Relevance |
|---|---|---|---|---|---|---|---|
| INT-01 | Provider/runtime capability owner | Baseline + current repo | 2026-04-03 | High | Provider/platform docs and `PreflightService` keep runtime capability truth machine-local and prelaunch-owned. | `P025` currently mixes repo registry and runtime capability ownership. | Architecture blocker. |
| INT-02 | Runtime-prepared session path | Current repo | 2026-04-03 | High | `GooseSessionBridge` is the existing start-order authority before prompt submission. | Reconciliation must extend this path rather than invent a side channel. | Architecture grounding. |
| INT-03 | Frozen-vs-runtime truth split | Baseline + current repo | 2026-04-03 | High | Current repo freezes run intent on `Run` and runtime outcome/session truth on `AgentExecution`. | `P025` still does not assign requested vs effective MCP truth to those owners explicitly. | Architecture blocker. |
| INT-04 | Shell-owned KPI/report lane | Current repo | 2026-04-03 | High | `RunReportView` and `RunComparisonView` already consume persisted KPI/report JSON. | `P025` telemetry can still open a second metrics blob if left unanchored. | Product/architecture blocker. |

## H. State Coverage Matrix
| State | Proposal Status (`Specified | Partial | Missing | Contradicted by repo | Deferred intentionally`) | Evidence IDs | Repo Touchpoints | Notes / Risks |
|---|---|---|---|---|
| Entry | Specified | DOC-01, MAP-01, MAP-05 | catalog + preflight | Problem statement and entry conditions are clear. |
| Happy path | Partial | DOC-01, MAP-03, MAP-04, MAP-05 | Goose session path | Reconciliation ordering is clear, but persisted ownership of requested vs effective truth is still partial. |
| Loading | Deferred intentionally | DOC-01 | N/A | No UI loading-state design is in scope. |
| Empty | Specified | DOC-01, MAP-03 | zero-MCP sessions | Zero-MCP default is explicit. |
| Validation error | Specified | DOC-01, MAP-05, DATA-03 | preflight | Blocking behavior for missing required MCP is explicit. |
| Backend error | Partial | DOC-01, MAP-03, MAP-07 | transport/runtime | Runtime reconciliation failure classes are directionally clear, but persistence-owner semantics remain partial. |
| Offline / degraded | Partial | DOC-01, MAP-05 | preflight/runtime capability | Fallback policy is specified, but current owner split between preflight prediction and actual runtime truth is incomplete. |
| Retry / recovery | Partial | DOC-01, DOC-07, INT-03 | execution truth | Proposal references frozen execution truth but does not explicitly assign MCP truth to run vs attempt owners. |
| Auth / permission expiry | Deferred intentionally | DOC-01 | N/A | Secret/auth lifecycle stays under provider-platform baseline. |
| Rollback / cancellation | Deferred intentionally | DOC-01, DOC-07 | N/A | Not central to MCP policy MVP. |

## I. Feature Flags / Rollout / Rollback
| Evidence ID | Mechanism / Flag | Scope | Rollout Plan | Rollback Path | Verified On | Confidence | Notes |
|---|---|---|---|---|---|---|---|
| FLAG-01 | None specified | Whole slice | Missing | Partial | 2026-04-03 | Medium | Migration sequencing exists, but no explicit rollout guard or rollback lane is documented. |

## J. Analytics / Instrumentation
| Evidence ID | Event / Signal | Purpose | Trigger Point | Verified On | Confidence | Gap / Risk |
|---|---|---|---|---|---|---|
| METRIC-01 | Existing run-level KPI/report lane | Shell-owned summary and comparison | `Run.sessionKPIExportJSON` / `Run.sessionLineageReportJSON` | 2026-04-03 | High | The current repo already has a canonical run-owned KPI/report lane that `P025` now explicitly extends. |
| METRIC-02 | Proposed MCP burn telemetry | Measure requested/effective counts, startup latency, tool-call burn, and blocked runs | Proposal Section 5.7 | 2026-04-03 | High | Metrics are now explicitly anchored to the canonical persisted/report owner path. |

## K. Testing Strategy
| Evidence ID | Layer | Covered Surface | Current Coverage | Proposed Additions | Verified On | Confidence | Gap / Risk |
|---|---|---|---|---|---|---|---|
| TEST-01 | Proposal-defined | preflight, reconciliation, diagnostics, telemetry | No explicit proposal-owned proof section exists in `P025` today | Missing | 2026-04-03 | Medium | The draft has no explicit proving lane or test-owner section yet. |

## L. Current Repo Reality / Contradictions
| Evidence ID | Repo Surface | Proposal Claim | Current Repo Reality | Verified On | Confidence | Implication |
|---|---|---|---|---|---|---|
| REAL-01 | Scope / acceptance wording versus later owner split | Later sections split repo-owned mapping truth from machine-local runtime capability truth | The updated scope and acceptance wording now match that split closely enough that the earlier contradiction is closed | 2026-04-03 | High | Closed. |
| REAL-02 | Preflight versus actual session truth | Proposal now says preflight stays within prediction authority | Current `PreflightService` is prelaunch-only and cannot own settled session-enabled truth, and the updated draft now matches that boundary | 2026-04-03 | High | Closed. |
| REAL-03 | Burn telemetry ownership | Proposal now says MCP telemetry extends the existing run-owned KPI/report lane | Current run reports/comparison already consume that canonical KPI/report lane on `Run`, and the updated draft now matches it | 2026-04-03 | High | Closed. |
| REAL-04 | Tactical runtime mitigation | Proposal should replace blanket cleanup with precise reconciliation | Current `GooseServerTransport` still removes all enabled extensions after session creation | 2026-04-03 | High | The proposal remains grounded in a real runtime seam and not an invented problem. |

## M. Proposal Completeness Matrix
| Dimension | Status (`Specified | Partial | Missing | Contradicted by repo | Deferred intentionally`) | Evidence IDs | Notes |
|---|---|---|---|
| Problem / user intent | Specified | DOC-01, DOC-02, DOC-03, REAL-04 | Motivation is strong and grounded in current repo reality. |
| Scope boundaries | Specified | DOC-01 | In/out-of-scope boundaries are explicit. |
| Reusable baseline coverage | Specified | BASE-01, BASE-02, DOC-04..07, INT-01..04, REAL-01..04 | The proposal now matches the relevant current owner boundaries. |
| Screen / surface definition | Specified | NAV-01..03, INT-04 | Diagnostics/report visibility and ownership are sufficiently anchored for readiness. |
| Navigation / entry points | Specified | NAV-01..03 | Preflight and shell-owned report surfaces are sufficiently implied for this proposal-sized slice. |
| State handling | Specified | H matrix, REAL-02 | Validation, zero-MCP, and predicted-vs-actual state ownership are now explicitly defined. |
| Data / API contract | Specified | MAP-02..05, DATA-01..05, REAL-01..04 | Core reconciliation intent and owner split are now explicit enough for implementation. |
| Persistence / caching | Specified | DATA-04..06, INT-03..04, REAL-02..03 | Requested/effective MCP truth and telemetry owner paths are now explicit. |
| Permissions / auth expiry | Deferred intentionally | DOC-01, DOC-05 | Provider secret/auth lifecycle stays out of scope. |
| Feature flags / rollout / rollback | Partial | FLAG-01 | Migration steps exist, but guarded rollout/rollback is not explicit. |
| Analytics / instrumentation | Specified | METRIC-01, METRIC-02, REAL-03 | Metrics and their canonical KPI/report owner are now explicit. |
| Testing strategy | Missing | TEST-01 | No explicit proof-owner or test section is present. |
| Dependencies / integration points | Partial | MAP-01..09, INT-01..04 | Main seams are identified, but some owner relationships remain implicit. |

## N. Assumptions, Open Questions, and Blockers
- ASSUMP-01: Goose-backed runtime remains the immediate implementation target for MCP session reconciliation.
- ASSUMP-02: Current operator shell should continue to own report, comparison, recovery, and artifact inspection without a parallel MCP inspector.
- QUESTION-01: Does the proposal want a later dedicated proof-owner section, or is the current acceptance text intentionally sufficient for implementation handoff?
- BLOCKER-01: None

## O. Research Triggers / External Questions
| Trigger ID | Trigger Type (`Proposal gap | Baseline constraint | Host-system integration risk | Unresolved tradeoff`) | Local Evidence IDs | Question to Research | Why Local Evidence Is Not Enough | Time Sensitivity / Freshness Risk |
|---|---|---|---|---|---|
| RSH-01 | Unresolved tradeoff | TEST-01 | None needed yet for readiness; the remaining open point is optional proof-owner hygiene, not a blocking architecture gap. | Current repo-local evidence is sufficient to judge the proposal. | Low |
