# Evidence Pack

## A. Local / Repo Inputs
| Evidence ID | Source / Path / Artifact | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|
| DOC-01 | `/Users/user/Documents/Chainworks Forge/docs/ps/chainworks-forge-mvp.md` | 2026-03-24 | High | The PS still defines the MVP success metric, DoD, unresolved open questions, and a `[TBD]` output-retrieval SLO. | The post-007 scope could be aimed at the wrong target. | Primary source of truth for “fully ready MVP”. |
| DOC-02 | `/Users/user/Documents/Chainworks Forge/docs/proposals/006-provider-expansion-settings-diagnostics-and-mvp-pilot-readiness.md` | 2026-03-24 | High | Proposal 006 expands to Codex, Claude, and Gemini plus settings/diagnostics/pilot surfaces. | MVP provider boundary could be misread. | Needed to see whether provider/platform work already closes MVP. |
| DOC-03 | `/Users/user/Documents/Chainworks Forge/docs/proposals/007-full-mvp-delivery-slice-worktrees-implementation-loop-manual-release-and-dogfooding.md` | 2026-03-24 | High | Proposal 007 explicitly frames itself as the first believable full-loop dogfood path and says the likely next step is dogfood hardening, not Steward or backend extraction. | The recommendation for the next proposal could overreach. | Primary roadmap owner for “what comes after 007”. |
| DOC-04 | `/Users/user/Documents/Chainworks Forge/docs/reference/runtime-contract.md` | 2026-03-24 | High | The runtime contract still defines MVP provider scope as `codex` + `claude_code`, with Gemini post-MVP. | Provider-boundary conclusions could be incorrect. | Reveals a live contract mismatch with Proposal 006’s wider provider scope. |

## B. External Sources
| Evidence ID | Source / Path / Artifact | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|
| WEB-01 | None required | 2026-03-24 | High | The reviewed question is answerable from repo-local materials and current code. | Low. | Avoids inventing external product assumptions. |

## C. Build and Run Log
| Evidence ID | Scheme / Target | Device / OS | Verified On | Build Result | Run Result | Blockers | Confidence | Relevance |
|---|---|---|---|---|---|---|---|---|
| RUN-01 | `Chainworks Forge` / app build | macOS host | 2026-03-24 | `passed` via `xcodebuild -quiet -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -derivedDataPath /tmp/codex-dd-mvp-next-build-quiet build` | N/A | Swift 6 migration warnings remain in YAML parsing/validation and `SimulatedAgentExecutor`, but build is green. | High | Confirms the current baseline is buildable enough to trust repo and UI evidence. |
| RUN-02 | `Chainworks ForgeUITests` focused current-shell slice | macOS host | 2026-03-24 | `passed` | `passed` via `xcodebuild test -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -derivedDataPath /tmp/codex-dd-mvp-next-ui -only-testing:'Chainworks ForgeUITests/Chainworks_ForgeUITests/testApprovalInboxReachable' -only-testing:'Chainworks ForgeUITests/Chainworks_ForgeUITests/testRunProgressViewSurface' -only-testing:'Chainworks ForgeUITests/Chainworks_ForgeUITests/testLiveRuntimeUnavailableShowsRecoveryGuidance'` | None in this focused slice. | High | Gives fresh simulator-backed evidence for the current operator shell baseline used as the pre-007 platform. |

## D. Xcode Screenshot Log
| Evidence ID | Source / Path / Artifact | Scheme / Target | Device / OS | Flow Step | State | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|---|---|---|---|
| SCR-01 | `/tmp/codex-dd-mvp-next-ui/Logs/Test/Test-Chainworks Forge-2026.03.24_20-41-53-+0200.xcresult` attachment `REQ011_Approvals` | `Chainworks ForgeUITests` | macOS host | Operator navigation | Approval inbox reachable from current shell | 2026-03-24 | High | Current app shell still surfaces approvals as a first-class reachable view. | The current operator baseline could be overstated. | Relevant to whether post-007 work should add new shell complexity or harden the current one. |
| SCR-02 | `/tmp/codex-dd-mvp-next-ui/Logs/Test/Test-Chainworks Forge-2026.03.24_20-41-53-+0200.xcresult` attachment `P004_NonHappy_MissingRuntime` | `Chainworks ForgeUITests` | macOS host | Start Run non-happy path | Missing live-runtime guidance | 2026-03-24 | High | The current app already exposes operator guidance for runtime-preflight failure. | The baseline operator UX could be mis-scoped. | Relevant because post-007 work should close MVP readiness rather than redoing already-surfaced guidance. |
| SCR-03 | `/tmp/codex-dd-mvp-next-ui/Logs/Test/Test-Chainworks Forge-2026.03.24_20-41-53-+0200.xcresult` attachments `REQ011_RunProgress_Entry`, `REQ011_RunProgress_Overview`, `REQ011_RunProgress_Sections` | `Chainworks ForgeUITests` | macOS host | Live run surface | Run Progress view reachable and instrumented | 2026-03-24 | High | The current shell already has a usable run-progress baseline that 007 extends rather than replaces. | The roadmap could add redundant UI scope. | Helps bound what a post-007 proposal should and should not rebuild. |

## E. Code / Architecture Evidence
| Evidence ID | Source / Path / Artifact | File Path / Module | Layer | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|---|---|
| CODE-01 | `ContentView.swift`, `RunsHomeView.swift`, `IdeaListView.swift` | `/Users/user/Documents/Chainworks Forge/Chainworks Forge/ContentView.swift`, `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Views/RunsHomeView.swift`, `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Views/IdeaListView.swift` | UI shell | 2026-03-24 | High | The app already has `Runs Home`, idea CRUD with optional attachment path, approvals, and run-progress/report surfaces in the main shell. | Post-007 scope could duplicate existing operator work. | Shows that the next proposal should harden/finish MVP rather than introduce a brand-new shell. |
| CODE-02 | `Run.swift`, `AgentExecution.swift`, `RunPlan.swift` | `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Models/Run.swift`, `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Models/AgentExecution.swift`, `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Engine/RunPlan.swift` | Data/runtime | 2026-03-24 | High | Current models carry report/trust data and frozen workspace state, but do not yet carry Proposal 007 repo/worktree/release metadata. | 007 readiness could be overstated. | Confirms repo-backed delivery is still future scope and should not be silently assumed closed. |
| CODE-03 | `rg --files 'Chainworks Forge'` absence proof for provider/settings surfaces | source tree | Provider/settings | 2026-03-24 | High | No `ProviderRegistry`, `ProviderSettingsStore`, `AppConfigurationStore`, `ProviderDiagnosticService`, adapter files, or Proposal 006 UI surfaces are present in current source. | Proposal 006 implementation maturity could be overstated. | Important because a post-007 proposal must not assume 006 became irrelevant; it still needs final MVP closure. |
| CODE-04 | `rg --files 'Chainworks Forge'` absence proof for repo-backed delivery services | source tree | Delivery runtime | 2026-03-24 | High | No `WorktreeProvisioner`, `RepoSafetyGuard`, `ReleaseOpsCoordinator`, `GitReleaseService`, `ConnectPublishService`, `ReleaseGateView`, or `DeliveryReceiptBuilder` files exist yet. | Proposal 007 maturity could be overstated. | Keeps the recommendation honest: post-007 scope must start only after 007 actually lands. |
| CODE-05 | `test -f examples/workflows/full-mvp-live.yaml` | `/Users/user/Documents/Chainworks Forge/examples/workflows/full-mvp-live.yaml` | Workflow fixtures | 2026-03-24 | High | `full-mvp-live.yaml` is still missing, while `proposal-loop-live.yaml` and `proposal-to-release.yaml` exist. | Repo-backed workflow readiness could be overstated. | Confirms that 007 still owns the first real repo-backed preset. |

## F. Current-State Baseline
| Evidence ID | Source / Path / Artifact | Verified On | Observed State | Verified in Simulator | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|---|---|
| BASE-01 | Current app shell + focused UI slice | 2026-03-24 | Proposal-loop operator shell is live and testable; provider/settings platform and repo-backed delivery layers are still absent | Yes | High | The repo today is between “operator shell baseline” and “full repo-backed MVP dogfood path”. | The roadmap recommendation could pick the wrong next-owner proposal. | Directly informs what remains after 007 to make the PS-ready MVP defensible. |

## G. Product / Data / Ops Evidence (Optional)
| Evidence ID | Source / Path / Artifact | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|
| DATA-01 | `docs/ps/chainworks-forge-mvp.md`, `docs/proposals/007-full-mvp-delivery-slice-worktrees-implementation-loop-manual-release-and-dogfooding.md` | 2026-03-24 | High | The PS success metric is a 50% reduction in manual orchestration time, but Proposal 007’s checkpoint only proves a believable full-loop dogfood session under 25 minutes on a sample repo. | MVP sign-off could stop short of the PS business outcome. | Strong signal that one post-007 proposal should own MVP validation and hardening, not new platform scope. |

## H. Assumptions, Open Questions, and Blockers
- ASSUMP-01: The user is asking about the proposal sequence after Proposal 007 lands substantially as written, not asking to audit current code as if Proposal 007 were already implemented.
- QUESTION-01: Should the post-007 MVP sign-off still freeze provider scope to Codex + Claude, or should Proposal 006 be revised to make Gemini explicitly post-MVP instead of “in MVP pilot readiness”?
- BLOCKER-01: Proposal 006 and Proposal 007 are still largely future-state in current code, so the recommendation can be evidence-based about ownership and scope, but not proven by a live end-to-end 007 runtime yet.
