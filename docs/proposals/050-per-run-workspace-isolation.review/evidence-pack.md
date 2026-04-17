# Proposal Evidence Pack

Proposal: `docs/proposals/050-per-run-workspace-isolation.md`
Mode: `proposal-readiness`
Verified on: 2026-04-16
Git SHA: `bb3f0ef3ac562267e6cd5b5462aee5d7f01888a2`
Working tree: Dirty; P050 is untracked, broad P029 control-plane changes are present, and P048/P049 proposal/reference artifacts are being moved or deleted.

## A. Local / Repo Inputs
| Evidence ID | Source / Path / Artifact | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---:|---|---|---|---|
| DOC-01 | `docs/proposals/050-per-run-workspace-isolation.md` | 2026-04-16 | High | Draft P050 proposes per-run `.chainworks/runs/{run_id}` meta roots for daemon workflow artifacts and names ACs for stale artifact, concurrent run, transition condition, cancel, fallback, and gate proof. | Review could judge stale or different proposal text. | Primary review target. |
| DOC-02 | `docs/reference/workflow-execution-engine.md` | 2026-04-16 | High | Swift-side baseline says `RunWorkspace` owns the isolation boundary and `artifactRoot` is already run-scoped. | P050's comparison to Swift isolation could be wrong. | Baseline comparison. |
| DOC-03 | `docs/reference/rust-control-plane.md` | 2026-04-16 | High | Rust control-plane baseline documents workflow artifact paths, `exists('artifact_name')`, artifact field resolution, ACP transport, and workspace snapshot artifact discovery. | Proposal could miss a current daemon artifact seam. | Rust daemon baseline. |
| DOC-04 | `docs/reference/full-mvp-delivery.md` | 2026-04-16 | Medium | Repo-backed delivery uses per-run delivery configuration and artifact-backed review/release transitions. | Isolation defects can affect delivery/release truth, not only proposal drafting. | Adjacent workflow context. |

## B. External Sources
| Evidence ID | Source / Path / Artifact | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---:|---|---|---|---|
| WEB-01 | None | 2026-04-16 | High | No external research was needed; the proposal is local daemon architecture. | None. | N/A |

## C. Build and Run Log
| Evidence ID | Scheme / Target | Device / OS | Verified On | Build Result | Run Result | Blockers | Confidence | Relevance |
|---|---|---|---:|---|---|---|---|---|
| RUN-01 | `xcodebuild -list -project "Chainworks Forge.xcodeproj"` | macOS host | 2026-04-16 | Succeeded | Not run | None | High | Confirms available Xcode scheme for skill-required build evidence. |
| RUN-02 | `xcodebuild -project "Chainworks Forge.xcodeproj" -scheme "Chainworks Forge" -destination "platform=macOS" -derivedDataPath <tmp> build` | macOS 26.4 SDK, My Mac | 2026-04-16 | Succeeded | Not run | Build warnings unrelated to P050 exist | High | Required build attempt. P050 is daemon-only, so this is host app build evidence, not daemon proof. |

## D. Xcode Screenshot Log
| Evidence ID | Source / Path | Scheme / Target | Device / OS | Flow Step | State | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---:|---|---|---|---|
| SCR-01 | Not captured | N/A | N/A | N/A | N/A | 2026-04-16 | High | P050 explicitly scopes daemon-only Rust changes and no UI surface is reachable for screenshot validation. | Treating missing screenshots as product evidence would be misleading. | UI evidence gate marked not applicable. |

## E. Code / Architecture Evidence
| Evidence ID | Source / Path | File Path / Module | Layer | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---:|---|---|---|---|
| CODE-01 | `rg "resolve_path_template\\("` | `control-plane/crates/engine/src/orchestrator.rs`, `control-plane/crates/engine/src/executor.rs` | Engine path resolution | 2026-04-16 | High | `resolve_path_template` is used at base-branch resolution, `exists`, `artifact.field`, companion/output prompt paths, release path resolution, and artifact normalization. | P050 could under-specify call-site migration and tests. | Central implementation blast radius. |
| CODE-02 | `orchestrator.rs:2054-2082` | `resolve_path_template` | Engine path resolution | 2026-04-16 | High | Current resolver reads `${VAR:-default}` from process env and falls back to template default, then makes relative paths workspace-root-relative. | Confirms root defect source and proposed seam. | P050 core seam. |
| CODE-03 | `orchestrator.rs:1216-1235`, `1308-1322` | `check_artifact_exists`, `read_artifact_field` | Transition evaluation | 2026-04-16 | High | Transition truth includes both `exists('artifact')` and JSON field reads from artifact paths. | P050 ACs can miss field-based stale artifact reads. | Test completeness. |
| CODE-04 | `orchestrator.rs:1633-1650`, `1710-1754` | prompt input/output path building | Agent IO contract | 2026-04-16 | High | Required output and input paths are resolved from artifact templates and then may be normalized to worktree paths for write-enabled agents. | Per-run meta root can be redirected into worktree unless specified. | Worktree isolation risk. |
| CODE-05 | `executor.rs:1128-1141`, `2290-2302` | artifact normalization | Executor artifact persistence | 2026-04-16 | High | Executor normalizes artifacts to canonical workflow paths after ACP completion and uses `resolve_path_template` again. | Agent-written outputs may be copied or detected against the wrong root. | End-to-end artifact truth. |
| CODE-06 | `executor.rs:2132-2136` | release artifact path resolution | Release artifacts | 2026-04-16 | High | Release artifact paths resolve through workflow/catalog templates. | P050 may affect release transition truth and receipts. | Adjacent release scope. |
| CODE-07 | `control-plane/crates/acp/src/lib.rs:15-61` | `ExecutionRequest` | ACP request model | 2026-04-16 | High | `ExecutionRequest` has no per-run env or `chainworks_meta_root` field. | P050's env injection cannot be implemented by only setting an existing request field. | ACP env handoff blocker. |
| CODE-08 | `control-plane/crates/acp/src/transport.rs:116-145`, `686-724` | `build_session_new_params`, `session/new` | ACP transport | 2026-04-16 | High | `session/new` params include `mcpServers`, `cwd`, `model`, and `mode`; no top-level environment payload is generated. | Proposal's "ACP already supports per-session env vars" claim is not supported by current code. | ACP env handoff blocker. |
| CODE-09 | `control-plane/crates/acp/src/transport.rs:147-166` | `mcp_servers_wire_value` | MCP server transport payload | 2026-04-16 | High | The only `env` handling in ACP transport is for stdio MCP server payloads nested under `mcpServers`, not the agent session process. | Env support can be confused with session env support. | Clarifies false assumption. |
| CODE-10 | `control-plane/crates/domain/src/run.rs:86-120` | `Run` | Domain model | 2026-04-16 | High | `Run` currently has `workspace_root`, `artifact_root`, and worktree fields but no `chainworks_meta_root`. | Confirms P050 model addition is needed. | Domain persistence. |
| CODE-11 | `control-plane/crates/db/migrations` | DB migrations | Persistence | 2026-04-16 | High | Current highest migration is already `012_steward_analysis.sql`. | P050's proposed `012_per_run_meta_root.sql` filename collides on this tree. | Migration blocker. |
| CODE-12 | `examples/agents/agents.yaml:16-71`, `689-996`, `1219` | agent catalog | Workflow/artifact catalog | 2026-04-16 | High | Many artifacts and permission surfaces use `${CHAINWORKS_META_ROOT:-.chainworks}`; one worktree policy path also uses `${CHAINWORKS_META_ROOT:-.chainworks}/proposals`. | Zero-YAML-change claim is plausible, but worktree policy interaction needs explicit proof. | Catalog scope. |

## F. Current-State Baseline
| Evidence ID | Source / Path | Verified On | Observed State | Verified in Simulator | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---:|---|---|---|---|---|---|
| BASE-01 | Current Rust daemon code | 2026-04-16 | Shared `.chainworks` default through env/template resolver | No | High | Current path resolver and executor can see stale workspace-local `.chainworks` artifacts. | Incident root cause could be misdiagnosed. | Confirms proposal problem. |
| BASE-02 | Current Swift app code/reference | 2026-04-16 | Swift `RunWorkspace` uses run-scoped roots | No | High | Swift app already has a distinct isolation model from Rust daemon. | Proposal could over-scope Swift changes. | Confirms daemon-only scope. |
| BASE-03 | Current test gate registry | 2026-04-16 | `proposal-050` is not registered in `scripts/test-gate.sh` or `docs/reference/test-gates.md` | No | High | Draft must add the gate as part of implementation. | Readiness could be overstated. | Proof lane. |

## G. Product / Data / Ops Evidence
| Evidence ID | Source / Path | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---:|---|---|---|---|
| DATA-01 | P050 incident section | 2026-04-16 | Medium | Proposal reports a live incident where stale `.chainworks` state made a proposal writer start implementation. | If incident details are wrong, severity may be overstated. | Product/user trust context. |
| DATA-02 | `git status --short` | 2026-04-16 | High | Current review occurred on a dirty tree with unrelated proposal/document movement. | Review could confuse proposal text with landed implementation. | Reproducibility note. |

## H. Assumptions, Open Questions, and Blockers
- ASSUMP-01: This is a proposal-readiness review, not an implementation audit.
- ASSUMP-02: The incident narrative is accepted as user-provided/local operational evidence, but the actual CryptoSavingsTracker run directory was not re-opened in this review.
- ASSUMP-03: Because P050 is daemon-only, UI screenshots are not applicable and are not evidence gaps.
- QUESTION-01: Should run meta artifacts always live under the original `workspace_root`, or should write-enabled worktree agents read/write meta artifacts under the worktree root? The proposal implies the former, but current path normalization pushes many artifact paths to `worktree_root`.
- QUESTION-02: Should existing NULL `chainworks_meta_root` runs still honor process `CHAINWORKS_META_ROOT`, or should they fall back only to `.chainworks` to avoid cross-run env leakage?
- BLOCKER-01: Proposed migration number `012_per_run_meta_root.sql` collides with existing `012_steward_analysis.sql`.
- BLOCKER-02: The proposal relies on ACP per-session env support that current `ExecutionRequest` and `session/new` construction do not provide.
- BLOCKER-03: The proposal does not specify how per-run meta paths interact with `normalize_path_for_worktree` for write-enabled agents.
