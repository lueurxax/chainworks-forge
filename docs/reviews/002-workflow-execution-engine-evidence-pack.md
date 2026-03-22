# Evidence Pack

## A. Local / Repo Inputs
| Evidence ID | Source / Path / Artifact | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|
| DOC-01 | `docs/proposals/002-workflow-execution-engine.md` | 2026-03-22 | High | Proposal 002 is the active draft for execution engine, approval flow, resume, and execution UI; source version `mtime 2026-03-22 19:46:32 +0200`, `md5 7fabb7b7312733f1b606627fe84d0169`, unchanged since the prior review pass. | Review would be stale if anchored to an older draft. | Primary reviewed artifact. |
| DOC-02 | `docs/reviews/001-proposal-002-gate.md` | 2026-03-22 | High | Proposal 001 is gated `GO`, so Proposal 002 is being reviewed against a foundation slice that is present in code and green enough to hand off from scaffold to execution. | Could overstate readiness if the gate artifact were invalid. | Defines the handoff boundary and what is already solved. |
| DOC-03 | `docs/ps/chainworks-forge-mvp.md` | 2026-03-22 | High | MVP requires workflow execution, frozen run snapshots, run monitor UI, approval gates, artifact metadata on disk + SwiftData, and explicit resume policy. | Architecture findings could be mis-prioritized without the PS constraints. | Product/architecture baseline for Proposal 002. |
| DOC-04 | `docs/reference/workspace-isolation-risk.md` | 2026-03-22 | High | Run isolation requires explicit `workspace_root`, no implicit `cwd`, and runtime guardrails on filesystem paths. | Understates execution risk if ignored. | Critical runtime constraint adjacent to the execution-engine design. |

## B. External Sources
| Evidence ID | Source / Path / Artifact | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|
| WEB-01 | None used | 2026-03-22 | High | Local repo evidence was sufficient. | None. | No external research was required. |

## C. Build and Run Log
| Evidence ID | Scheme / Target | Device / OS | Verified On | Build Result | Run Result | Blockers | Confidence | Relevance |
|---|---|---|---|---|---|---|---|---|
| RUN-01 | `Chainworks Forge` | `My Mac` | 2026-03-22 19:53 local | `xcodebuild ... build` passed | N/A | Non-blocking AppIntents metadata warning only | High | Confirms the repo still builds while reviewing Proposal 002. |
| RUN-02 | `Chainworks Forge` | `My Mac` | 2026-03-22 19:53-19:55 local | `xcodebuild ... test` passed | App launched under UI tests; scaffold baseline states were exercised successfully; latest result bundle `/Users/user/Library/Developer/Xcode/DerivedData/Chainworks_Forge-ayratnusrqmfbievclfarmszyhkv/Logs/Test/Test-Chainworks Forge-2026.03.22_19-53-54-+0200.xcresult` | Reused in the current repeat round because the proposal source and relevant app baseline files remained unchanged after this run; the run still proves only scaffold-era UI states | High | Establishes the current runtime baseline and the evidence-gap boundary. |

## D. Xcode Screenshot Log
| Evidence ID | Source / Path / Artifact | Scheme / Target | Device / OS | Flow Step | State | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|---|---|---|---|
| SCR-01 | `docs/reviews/artifacts/proposal-002-baseline-2026-03-22/125B50E7-6151-459D-955C-B9D65C30303E.png` | `Chainworks ForgeUITests` | `My Mac` | Entry | Launch | 2026-03-22 | High | Reused screenshot. Current code inspection and refreshed UI checkpoint test still show the same three-tab shell as the reachable baseline. | Low. | Entry-state evidence for current baseline. |
| SCR-02 | `docs/reviews/artifacts/proposal-002-baseline-2026-03-22/013B3E01-856C-457A-B730-CFB2671F1735.png` | `Chainworks ForgeUITests` | `My Mac` | Main path | Ideas tab | 2026-03-22 | High | Reused screenshot. Ideas still shows the Proposal 001 scaffold: list/detail and idea creation only. | Low. | Confirms the current running app lacks Proposal 002 start-run affordances. |
| SCR-03 | `docs/reviews/artifacts/proposal-002-baseline-2026-03-22/2F1AF392-BFEE-4751-80CB-587FFD939F43.png` | `Chainworks ForgeUITests` | `My Mac` | Main path | Agent Catalog tab | 2026-03-22 | High | Reused screenshot. Agent catalog remains an inspection surface only. | Low. | Confirms current app is still scaffold-only. |
| SCR-04 | `docs/reviews/artifacts/proposal-002-baseline-2026-03-22/F8E68B9B-E83C-4A20-BA1D-61C78358E526.png` | `Chainworks ForgeUITests` | `My Mac` | Main path | Workflow Inspector tab | 2026-03-22 | High | Reused screenshot. Workflow inspector remains a read-only parser/validator surface. | Low. | Confirms Proposal 002 execution views are not reachable today. |
| SCR-05 | `docs/reviews/artifacts/proposal-002-baseline-2026-03-22/540DE837-B869-47AC-B748-CD8CF49C875F.png` | `Chainworks ForgeUITests` | `My Mac` | Confirmation | Idea created | 2026-03-22 | Medium | Reused screenshot. The highest confirmed happy-path state in the app is still "idea created", not "run compiled / executing". | Low. | Shows where observable runtime evidence stops today. |

## E. Code / Architecture Evidence
| Evidence ID | Source / Path / Artifact | File Path / Module | Layer | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|---|---|
| CODE-01 | File inventory | `Chainworks Forge/`, `Chainworks ForgeTests/`, `Chainworks ForgeUITests/` | Module map | 2026-03-22 | High | Current repo still has `DSL/`, `Models/`, and `Views/`, but no `Engine/`, no `RunPlan*`, no orchestrator, no transition evaluator, no artifact manager, and no execution UI views. | Low. | Confirms Proposal 002 target flow is not implemented in code yet. |
| CODE-02 | Current app shell | `Chainworks Forge/ContentView.swift:4-48` | UI entry | 2026-03-22 | High | The app still exposes only `Ideas`, `Agent Catalog`, and `Workflow Inspector` tabs. | Low. | Confirms runtime baseline and missing Proposal 002 surfaces. |
| CODE-03 | Current idea detail | `Chainworks Forge/Views/IdeaListView.swift:153-178` | UI detail | 2026-03-22 | High | `IdeaDetailView` shows idea fields and a read-only runs list; there is no `[Start New Run]`, no compilation sheet, and no progress navigation. | Low. | Direct comparison point for Proposal 002 UI additions. |
| CODE-04 | Current proposal contract reread | `docs/proposals/002-workflow-execution-engine.md:154-158`, `docs/proposals/002-workflow-execution-engine.md:854-872`, `docs/proposals/002-workflow-execution-engine.md:1532-1538` | Proposal architecture | 2026-03-22 | High | The previously reported artifact-path notation drift is fixed in the current draft; no new proposal-text inconsistency surfaced in the rechecked sections. | Low. | Basis for closing the last live proposal-design finding. |
| CODE-05 | Current runtime baseline | `Chainworks ForgeUITests/Chainworks_ForgeUITests.swift`, `RUN-02` | Test runtime | 2026-03-22 | High | The latest UI checkpoint test passed, but it still exercises only the Proposal 001 scaffold flow. | Low. | Explains why the full-review gate remains partial even after a green test run. |

## F. Current-State Baseline
| Evidence ID | Source / Path / Artifact | Verified On | Observed State | Verified in Simulator | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|---|---|
| BASE-01 | `ContentView.swift`, `IdeaListView.swift`, `SCR-02`, `SCR-04` | 2026-03-22 | Scaffold-only app | Yes | High | The running app is still Proposal 001's verification scaffold. No Proposal 002 entry, progress, approval, artifact, or resume UI state is observable. | Low. | Primary reason the full-review gate fails. |
| BASE-02 | File inventory + `CODE-01` | 2026-03-22 | No execution-engine module in repo | No | High | Proposal 002 components exist only in the draft, not in source files. | Low. | Confirms current repo reality diverges from the target proposal surface. |
| BASE-03 | `RUN-02` + screenshot artifacts | 2026-03-22 | Reachable runtime states stop at idea creation and YAML inspection | Yes | High | The latest rerun confirms the scaffold baseline is healthy, but it still does not reach any real Proposal 002 lifecycle state. | Low. | Limits what UI/UX claims can be made with confidence. |

## G. Product / Data / Ops Evidence (Optional)
| Evidence ID | Source / Path / Artifact | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|
| DATA-01 | `docs/ps/chainworks-forge-mvp.md` | 2026-03-22 | High | The product still targets a 50% reduction in manual orchestration time, which makes execution the next real product bottleneck once Proposal 001 is implemented. | Product conclusions weaken if the metric changes. | Relevant context for why Proposal 002 matters now. |

## H. Assumptions, Open Questions, and Blockers
- ASSUMP-01: Review mode is `full-review` without product overlay because the user requested a review of the proposal itself, not KPI prioritization or rollout planning.
- ASSUMP-02: Repeat-round freshness check found no material delta in the proposal text or relevant scaffold-era app baseline since the immediately prior report, so runtime and screenshot evidence were reused rather than rebuilt.
- BLOCKER-01: The primary Proposal 002 runtime states (`Start Run`, `Run Progress`, `Approval Gate`, `Artifact Inspector`, resume recovery`) are not implemented in the current app, so they cannot be validated in the simulator.
- BLOCKER-02: Screenshot coverage exists only for scaffold-era states; entry/main/confirmation/error/empty/recovery coverage for the actual Proposal 002 flow does not exist yet.
- BLOCKER-03: The evidence pack is therefore `Partial`, not `Complete`, and a defensible full triad review must still fall back to an `Evidence Gap Review`.
