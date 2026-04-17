# Consolidated Proposal Review

## 0. Review Mode and Evidence Summary
- Mode used: `proposal-readiness` via `proposal-review-triad`, adapted for a daemon-only proposal.
- Evidence completeness: `Complete` for proposal/code readiness; Xcode screenshot evidence is not applicable because P050 has no UI surface.
- Documents / repo inputs reviewed: `docs/proposals/050-per-run-workspace-isolation.md`, `docs/reference/workflow-execution-engine.md`, `docs/reference/rust-control-plane.md`, `docs/reference/full-mvp-delivery.md`, current `examples/agents/agents.yaml`, current Rust control-plane code.
- External sources reviewed: none.
- Build/run attempts: `xcodebuild -list` succeeded; `xcodebuild ... build` succeeded for the macOS app.
- Screenshots captured: none; daemon-only proposal.
- Code areas inspected: Rust `Run`, DB run repository/migrations, orchestrator path resolution, executor artifact handling, ACP `ExecutionRequest`, ACP `session/new` construction, agent catalog meta-root references, test gate registry.
- Remaining assumptions: incident details are accepted from proposal text; actual CryptoSavingsTracker run directory was not independently re-opened.
- Remaining blockers: migration filename collision, ACP env handoff assumption, and worktree/meta-root path-normalization ambiguity.

## 1. Executive Summary
- Overall readiness: `Red`
- Confidence: `High`
- Release blockers: 3
- Top risks:
  1. P050 proposes `012_per_run_meta_root.sql`, but migration `012_steward_analysis.sql` already exists on this tree.
  2. P050's core env-injection plan assumes ACP per-session env support that current `ExecutionRequest` and `session/new` params do not expose.
  3. P050 does not specify how `.chainworks/runs/{run_id}` paths avoid being rewritten into `worktree_root` for write-enabled agents.
- Top opportunities:
  1. The problem statement is strong and tied to a real, severe contamination incident.
  2. Reusing `${CHAINWORKS_META_ROOT:-.chainworks}` is the right low-blast-radius design direction.
  3. A narrow implementation can be made robust if it explicitly owns all path-resolution call sites and proof cases.

## 2. Discipline Scorecard
| Discipline | Readiness | Confidence | Evidence Completeness | Critical | High | Medium | Low |
|---|---|---|---|---:|---:|---:|---:|
| UI | Green | High | Complete / N/A | 0 | 0 | 0 | 0 |
| UX | Amber | Medium | Complete | 0 | 0 | 1 | 0 |
| iOS Architecture | Red | High | Complete | 1 | 3 | 1 | 0 |

## 3. Findings by Discipline

### 3.1 UI Findings
None. P050 explicitly scopes daemon-only Rust changes and does not introduce or modify an operator UI surface.

### 3.2 UX Findings
- Finding ID: `UX-050-01`
  Severity: `Medium`
  Evidence IDs: `DOC-01`, `DATA-01`, `BASE-01`, `BLOCKER-02`, `BLOCKER-03`
  Why it matters:
  The proposal correctly identifies a high-trust failure mode: a new run silently inherited stale proposal/review state and the agent continued implementation instead of drafting. However, the current draft's proof plan focuses on isolated paths and does not require an end-to-end "agent prompt sees fresh inputs" assertion across ACP env propagation and worktree normalization. If those seams are wrong, the operator sees the same confusing behavior even though the DB field and path resolver appear implemented.
  Recommended fix:
  Add an acceptance test that starts a new run with stale root `.chainworks/state/run-state.json`, executes or simulates the proposal-writer ACP request, and asserts the prompt/input paths and expected output paths point only at `.chainworks/runs/{new_run_id}/...`.
  Acceptance criteria:
  - The prompt for `state_2_proposal_drafted` includes per-run `idea_brief`, `run_state`, proposal, and review paths.
  - A stale root `.chainworks/proposals/current/proposal.md` is not included as an input or lazy path for the new run.
  - The test covers at least one write-enabled agent path or explicitly proves proposal drafting is never worktree-normalized.
  Confidence: `Medium`

### 3.3 iOS Architecture Findings
- Finding ID: `ARCH-050-01`
  Severity: `Critical`
  Evidence IDs: `DOC-01`, `CODE-11`, `BLOCKER-01`
  Why it matters:
  P050 names `db/migrations/012_per_run_meta_root.sql`, but the current control-plane migrations already include `012_steward_analysis.sql`. SQL migration numbering is a repo-ordering contract; colliding migration numbers make the implementation ambiguous and can break fresh database setup or migration tracking.
  Recommended fix:
  Rename the proposed migration to the next available number on this tree, currently `013_per_run_meta_root.sql`, and make the proposal say "next migration number" rather than hard-coding a stale value.
  Acceptance criteria:
  - P050's files-to-modify table names a non-conflicting migration filename.
  - Fresh DB setup applies migrations in deterministic order.
  - A migration test or DB integration test proves `runs.chainworks_meta_root` exists after migration.
  Confidence: `High`

- Finding ID: `ARCH-050-02`
  Severity: `High`
  Evidence IDs: `DOC-01`, `CODE-07`, `CODE-08`, `CODE-09`, `BLOCKER-02`
  Why it matters:
  The proposal's core runtime plan says the executor sets `CHAINWORKS_META_ROOT` in the ACP subprocess environment and that "ACP transport already supports per-session env vars via `session/new.params`." Current code does not support that claim: `ExecutionRequest` has no env field, `build_session_new_params` emits only `mcpServers`, `cwd`, `model`, and `mode`, and the only `env` support is nested inside stdio MCP server payloads. Implementing P050 exactly as written will not make provider agents inherit the per-run meta root.
  Recommended fix:
  Specify the concrete ACP handoff shape. Either add `chainworks_meta_root` / `environment` to `ExecutionRequest` and have each adapter spawn the provider process with that env, or add a provider-supported `session/new` field and prove every adapter honors it. Do not rely on the existing MCP-server env payload.
  Acceptance criteria:
  - `ExecutionRequest` or adapter config has an explicit per-run env carrier.
  - `CHAINWORKS_META_ROOT` is visible to Claude/Codex/Gemini/Auggie/Junie ACP sessions or the design stops depending on provider env expansion.
  - A unit test proves `build_session_new_params` or adapter process env contains the per-run meta root.
  - An integration test proves an ACP agent writes to `.chainworks/runs/{run_id}/...`.
  Confidence: `High`

- Finding ID: `ARCH-050-03`
  Severity: `High`
  Evidence IDs: `DOC-01`, `CODE-01`, `CODE-04`, `QUESTION-01`, `BLOCKER-03`
  Why it matters:
  Current prompt/output path building resolves catalog artifact paths and then calls `normalize_path_for_worktree` for write-enabled agents. If P050 resolves `${CHAINWORKS_META_ROOT}` to `{workspace_root}/.chainworks/runs/{run_id}`, current normalization can rewrite those meta artifact paths into `{worktree_root}/.chainworks/runs/{run_id}`. The proposal says worktree provisioning is unchanged and implies per-run meta lives under the project workspace, but it does not define an exemption or alternate readback model for write-enabled agents.
  Recommended fix:
  Add a design rule for meta artifacts versus source worktree artifacts. Recommended rule: paths under `chainworks_meta_root` are control-plane meta artifacts and must not be worktree-normalized; source-code paths remain worktree-normalized for write-enabled agents.
  Acceptance criteria:
  - `normalize_path_for_worktree` skips canonical meta-root paths, or all transition/readback paths are taught to look in the worktree meta root.
  - Focused tests cover a write-enabled implementation/review agent that reads an approved proposal and writes implementation artifacts without falling back to root `.chainworks`.
  - Transition checks after write-enabled stages resolve to the same physical meta root used by the agent.
  Confidence: `High`

- Finding ID: `ARCH-050-04`
  Severity: `High`
  Evidence IDs: `DOC-01`, `CODE-01`, `CODE-03`, `CODE-05`, `CODE-06`
  Why it matters:
  P050 says `CHAINWORKS_META_ROOT` is resolved in "exactly two places" and that transition evaluation needs no logic change. The current code has many semantic call sites: base branch resolution, `exists`, `artifact.field`, prompt input paths, required output paths, companion output paths, release artifact paths, and post-ACP artifact normalization. Changing the resolver signature will force compilation fixes, but the proposal's acceptance tests only cover `exists` and stale state, not all semantic paths where stale artifacts can leak.
  Recommended fix:
  Replace the "exactly two places" claim with a call-site inventory and add tests for all user-visible path consumers: prompt inputs, output targets, `exists`, `artifact.field`, release artifact path resolution, and artifact normalization.
  Acceptance criteria:
  - Every `resolve_path_template` call site passes the run's meta root or proves it is non-artifact config.
  - `artifact.field` transition tests read from the current run's meta root.
  - `normalize_artifacts` writes/copies to the same per-run canonical paths that transition checks inspect.
  - Release artifact path resolution uses the per-run meta root for `git_push_receipt`, `delivery_receipt`, and related artifacts.
  Confidence: `High`

- Finding ID: `ARCH-050-05`
  Severity: `Medium`
  Evidence IDs: `DOC-01`, `CODE-02`, `QUESTION-02`
  Why it matters:
  Backward compatibility says old runs with `chainworks_meta_root = NULL` fall back to process `CHAINWORKS_META_ROOT` before `.chainworks`. That preserves current behavior, but it also preserves the shared mutable env failure mode for resumed legacy runs. A global daemon env var can still make multiple NULL runs read the same root.
  Recommended fix:
  Narrow legacy fallback. Prefer `run.chainworks_meta_root`, then template default `.chainworks`; allow process `CHAINWORKS_META_ROOT` only for explicit test/dev override with a warning, or persist a meta root when resuming a legacy run.
  Acceptance criteria:
  - Legacy NULL fallback behavior is explicitly documented as safe or intentionally dev-only.
  - A test proves two NULL legacy runs cannot accidentally share a non-default env root in production mode.
  - New runs always persist a non-null meta root.
  Confidence: `Medium`

## 4. Cross-Discipline Conflicts and Decisions
- Conflict: The proposal wants zero YAML changes and unchanged worktree provisioning, while current write-enabled path normalization rewrites workspace-root artifact paths into worktree-root paths.
  Tradeoff: Keeping normalization unchanged is lower implementation effort but risks splitting meta truth between workspace and worktree. Exempting meta-root paths adds logic but preserves one canonical per-run state root.
  Decision: Treat `.chainworks/runs/{run_id}` as control-plane meta, not source worktree content, unless the proposal explicitly chooses a worktree-local meta root and updates every readback/transition path.
  Owner: Proposal author / implementation owner.

## 5. Prioritized Action Backlog
| Priority | Item | Discipline | Owner | Horizon | Dependencies | Success Metric | Source Findings |
|---|---|---|---|---|---|---|---|
| P0 | Rename migration to the next available number and update the files table | Architecture | Proposal author | Before implementation | current migration list | No migration-number collision on fresh DB setup | `ARCH-050-01` |
| P0 | Specify and prove the ACP env handoff mechanism | Architecture | Proposal author / implementation owner | Before implementation | ACP adapter capabilities | Agents receive per-run `CHAINWORKS_META_ROOT` or no longer depend on env expansion | `ARCH-050-02` |
| P0 | Define meta-root behavior for write-enabled worktree agents | Architecture | Proposal author / implementation owner | Before implementation | `normalize_path_for_worktree` decision | Agent prompt paths and transition paths point at the same physical per-run root | `ARCH-050-03` |
| P1 | Expand call-site inventory and focused tests beyond `exists()` | Architecture | Implementation owner | During implementation | resolver signature update | Tests cover prompt inputs, outputs, `artifact.field`, release artifacts, and normalization | `ARCH-050-04` |
| P1 | Add end-to-end stale-root prompt proof | UX | Implementation owner | During implementation | ACP env + worktree decision | New run proposal writer cannot see stale prior proposal/review/run-state artifacts | `UX-050-01` |
| P2 | Clarify legacy NULL fallback semantics | Architecture | Proposal author | Before sign-off | migration/backcompat decision | Legacy runs cannot accidentally share a process env meta root in production | `ARCH-050-05` |

## 6. Validation and Measurement Plan
| Area | What Will Be Measured | Leading Indicators | Guardrails | Review Checkpoint | Rollback / Hold Criteria |
|---|---|---|---|---|---|
| Fresh run isolation | New run with stale root `.chainworks` does not read prior state/proposal/reviews | prompt/input paths contain `.chainworks/runs/{new_run_id}` only | no root `.chainworks` reads for new runs | `proposal-050` gate | hold if stale root artifact changes run behavior |
| Parallel run isolation | Two runs on same workspace write disjoint state/proposal/review artifacts | distinct meta roots and artifact paths per run | no shared `run-state.json`, proposal, review outputs | concurrency test | hold if paths collide |
| Worktree interaction | Write-enabled agents use source worktree and canonical run meta root consistently | prompt paths and transition paths match | no meta-root split between workspace and worktree | implementation review | hold if transition checks cannot see agent outputs |
| ACP env handoff | Provider sessions receive correct `CHAINWORKS_META_ROOT` or use absolute prompt paths without env reliance | unit + integration proof per adapter path | no reliance on unsupported `session/new` field | `proposal-050` gate | hold if provider writes to default `.chainworks` |
| Backward compatibility | NULL meta-root runs keep expected old behavior without poisoning new runs | legacy fallback test passes | new runs always non-null | migration test | hold if NULL runs read another run's meta root |

## 7. Evidence Gaps and Open Questions

### Evidence Gaps
- GAP-01: The actual CryptoSavingsTracker incident directory was not re-opened; incident facts were reviewed from proposal text.
- GAP-02: No simulator screenshots were captured because P050 has no UI surface.
- GAP-03: No `proposal-050` gate could be run because it is not registered yet.

### Open Questions
- QUESTION-01: Should meta artifacts live under the original project workspace root even for write-enabled agents, or under each write-enabled worktree?
- QUESTION-02: Should legacy NULL runs honor process `CHAINWORKS_META_ROOT`, or should production fallback ignore env and use the template default?
- QUESTION-03: Does every supported ACP provider accept a session-level environment payload, or must env be injected at adapter process spawn time?

## 8. Final Readiness Call
Readiness is `Red` on the current draft. The proposal targets the right defect and has the right high-level design direction, but it is not implementation-ready until it fixes the migration collision, replaces the unsupported ACP env assumption with a concrete handoff design, and defines how per-run meta paths interact with write-enabled worktrees and all artifact-resolution call sites.
