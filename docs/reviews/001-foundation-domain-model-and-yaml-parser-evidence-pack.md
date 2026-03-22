# Evidence Pack

## A. Local / Repo Inputs
| Evidence ID | Source / Path / Artifact | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|
| DOC-01 | `docs/proposals/001-foundation-domain-model-and-yaml-parser.md` | 2026-03-22 | High | Current draft adds canonical provenance hashing, snapshotted run provenance, atomic `RunGuard`, expanded validator coverage, and a narrowed verification scaffold; compact parsing/normalization is still in scope. | Review would over-credit or under-credit the draft if the file changed again. | Primary reviewed artifact. |
| DOC-02 | `docs/ps/chainworks-forge-mvp.md` | 2026-03-22 | High | MVP still requires YAML-driven workflows, one active run per idea, SwiftData persistence, approval gates, and resume-on-launch. | Completeness findings could weaken if product requirements changed. | Requirement baseline. |
| DOC-03 | `README.md` | 2026-03-22 | High | The repository still presents the app as largely template-level while docs/specs are ahead of implementation. | Current-state baseline could be misstated. | Confirms review is against a mostly unimplemented app. |
| DOC-04 | `docs/research/chainworks_core_idea.md` | 2026-03-22 | Medium | Product framing remains run-centric, artifact-first, approval-gated, and YAML-defined. | Architectural framing could be skewed if product direction changed. | Supports domain-model expectations. |
| DOC-05 | `docs/research/goose_swiftui_agent_architecture_research.md` | 2026-03-22 | Medium | Runtime direction still treats repo YAML as product truth and an orchestrator as the compilation boundary. | Parser/runtime boundary findings could be off. | Supports boundary judgments. |
| DOC-06 | `examples/agents/agents.yaml` | 2026-03-22 | High | Canonical catalog uses snake_case keys and canonical agent IDs with underscores, e.g. `proposal_writer`, `security_checker`, `commit_and_push_to_github`, `build_archive_and_push_connect`. | Parser/normalizer findings would be invalid if fixture changed. | Ground truth for catalog parsing and reference validation. |
| DOC-07 | `examples/workflows/workflow.yaml` | 2026-03-22 | High | Canonical full workflow uses `states`, `transitions`, `run`, `parallel`, `then`, `run_after_approval`, loops, variables, and scoring. | Full workflow parser scope could be misjudged. | Ground truth for full workflow parsing. |
| DOC-08 | `examples/workflows/proposal-to-release.yaml` | 2026-03-22 | High | Compact workflow uses `required_providers`, `stages`, `needs`, `gate`, and hyphenated agent IDs such as `proposal-writer`, `security-checker`, and `connect-publisher`. | Compact parser/normalizer findings would be invalid if fixture changed. | Ground truth for the compact path. |
| DOC-09 | `docs/proposals/001-foundation-domain-model-and-yaml-parser.md` review response log | 2026-03-22 | High | The proposal explicitly claims earlier findings were fixed, including `CodingKeys`, `RunGuard`, and hashing. | Could miss regressions or incomplete fixes. | Important because this is a repeat review. |

## B. External Sources
| Evidence ID | Source / Path / Artifact | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|
| WEB-01 | None used | 2026-03-22 | High | Local repo evidence was sufficient for this pass. | None. | Keeps review anchored to repo reality. |

## C. Build and Run Log
| Evidence ID | Scheme / Target | Device / OS | Verified On | Build Result | Run Result | Blockers | Confidence | Relevance |
|---|---|---|---|---|---|---|---|---|
| RUN-01 | `Chainworks Forge` | `My Mac / macOS 26.x` | 2026-03-22 | `xcodebuild -project "Chainworks Forge.xcodeproj" -scheme "Chainworks Forge" -destination "platform=macOS" build` succeeded. | Built app bundle is launchable. | None at build time. | High | Confirms repo remains buildable today. |
| RUN-02 | `Chainworks Forge` tests | `My Mac / macOS 26.x` | 2026-03-22 | `xcodebuild -project "Chainworks Forge.xcodeproj" -scheme "Chainworks Forge" -destination "platform=macOS" test` succeeded. Result bundle: `~/Library/Developer/Xcode/DerivedData/Chainworks_Forge-ayratnusrqmfbievclfarmszyhkv/Logs/Test/Test-Chainworks Forge-2026.03.22_10-47-58-+0200.xcresult` | 1 unit test and 4 UI tests passed, including launch screenshot attachments. | Tests still cover only template behavior. | High | Confirms baseline runtime health and preserves screenshot evidence. |

## D. Xcode Screenshot Log
| Evidence ID | Source / Path / Artifact | Scheme / Target | Device / OS | Flow Step | State | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|---|---|---|---|
| SCR-01 | `docs/reviews/artifacts/proposal-001-launch/00F45C92-67A3-4A53-9829-F6818D0D6113.png` | `Chainworks Forge` | `My Mac / macOS 26.x` | Entry | Launch baseline (light) | 2026-03-22 | High | Launch-test attachment shows the current app as the stock split-view template with an empty list and `Select an item`. | Screenshot captures the full desktop, not a cropped app-only view. | Confirms current UI baseline in light mode. |
| SCR-02 | `docs/reviews/artifacts/proposal-001-launch/D0068B54-F071-45EF-904C-07A0DB58AC73.png` | `Chainworks Forge` | `My Mac / macOS 26.x` | Entry | Launch baseline (dark) | 2026-03-22 | High | Launch-test attachment shows the same template baseline in dark mode. | Same capture caveat as above. | Confirms current UI baseline in dark mode. |

## E. Code / Architecture Evidence
| Evidence ID | Source / Path / Artifact | File Path / Module | Layer | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|---|---|
| CODE-01 | App entrypoint | `Chainworks Forge/Chainworks_ForgeApp.swift` | App / persistence bootstrap | 2026-03-22 | High | The app still registers only `Item.self` in the SwiftData schema. | Could understate hidden implementation if other files existed, but repo scan showed none. | Confirms Proposal 001 is still unimplemented. |
| CODE-02 | Root view | `Chainworks Forge/ContentView.swift` | UI | 2026-03-22 | High | Root UI remains the default split-view template over `Item`. | Visual-flow review would be invalid if richer UI existed elsewhere. | Current-state UI baseline. |
| CODE-03 | Model | `Chainworks Forge/Item.swift` | Data model | 2026-03-22 | High | Only persisted model is the template `Item`. | Same as above. | Establishes current persistence gap. |
| CODE-04 | Test baseline | `Chainworks ForgeTests/Chainworks_ForgeTests.swift` | Testing | 2026-03-22 | High | Unit test target still contains only the empty template test. | Test-readiness findings would soften if hidden tests existed elsewhere. | Relevant to proposal test strategy. |
| CODE-05 | UI test baseline | `Chainworks ForgeUITests/*.swift` | UI testing | 2026-03-22 | High | UI tests still validate only example launch behavior. | Could misstate UI evidence if richer tests existed elsewhere. | Explains why review remains evidence-gap based. |
| CODE-06 | `AgentTask` contract in proposal | `docs/proposals/001-foundation-domain-model-and-yaml-parser.md:592-596` | DSL contract | 2026-03-22 | High | Normalized full workflows require `agent`, `task`, `inputs`, and `outputs` for each task. | Compact-normalization findings would weaken if those fields became optional. | Important for judging compact normalization completeness. |
| CODE-07 | Compact workflow meta in proposal | `docs/proposals/001-foundation-domain-model-and-yaml-parser.md:732-738` | DSL contract | 2026-03-22 | High | `CompactWorkflowMeta` currently declares `requiredProviders` without `CodingKeys`. | Decoding findings would be wrong if hidden decoding rules existed, but proposal explicitly relies on plain Yams decoding plus explicit `CodingKeys`. | Direct evidence for the remaining compact decoding gap. |
| CODE-08 | Normalizer contract in proposal | `docs/proposals/001-foundation-domain-model-and-yaml-parser.md:754-779` | DSL contract | 2026-03-22 | High | Normalizer rules describe stage→state, needs→transitions, approval/fanout conversion, and some defaults, but do not define compact agent alias mapping or full task derivation. | Review could overstate risk if those rules were documented elsewhere, but no such section exists. | Direct evidence for compact normalization gaps. |

## F. Current-State Baseline
| Evidence ID | Source / Path / Artifact | Verified On | Observed State | Verified in Simulator | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|---|---|
| BASE-01 | Repo + screenshots | 2026-03-22 | macOS SwiftUI template app builds and launches; UI remains the stock empty split-view shell. | No iOS simulator; verified via macOS UI test attachments. | High | Proposal is still being reviewed against a template baseline, not a partially implemented feature slice. | Review may appear harsher if a reader assumes hidden implementation. | Frames delivery risk accurately. |
| BASE-02 | Test result bundles | 2026-03-22 | Launch automation works reliably, but only against the template app. | No feature-specific states reached. | High | There is current UI evidence, but not for any proposal-implemented state because the proposal remains unimplemented. | Could overstate evidence completeness if treated as target-flow coverage. | Explains why this remains an evidence-gap review. |

## G. Product / Data / Ops Evidence (Optional)
| Evidence ID | Source / Path / Artifact | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|
| DATA-01 | Not collected | 2026-03-22 | High | No KPI, rollout, or analytics artifacts were requested for this pass. | None. | Product overlay was not triggered. |

## H. Assumptions, Open Questions, and Blockers
- ASSUMP-01: Review scope is Proposal 001 against current repo reality and the canonical fixtures in `examples/`, not against an unpublished branch.
- ASSUMP-02: Product overlay is out of scope because the request did not ask for prioritization, KPI, rollout, or business-value critique.
- QUESTION-01: Is compact workflow intended to be a first-class executable input, or only an inspection/authoring convenience?
- QUESTION-02: What canonical mapping should compact agent aliases use to resolve to catalog IDs?
- QUESTION-03: How should the normalizer derive `AgentTask.task`, task IO bindings, and `Transition.when` expressions from compact YAML?
- BLOCKER-01: Proposal 001 is not implemented, so the target flow cannot be exercised in the app.
- BLOCKER-02: Screenshot evidence covers only the stock template baseline, not the proposed scaffold or its failure states.
