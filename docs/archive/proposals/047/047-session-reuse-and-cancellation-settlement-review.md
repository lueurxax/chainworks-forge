# Consolidated Proposal Review

## 0. Review Mode and Proposal Evidence Summary
- Mode used: `proposal-readiness`
- Evidence completeness: `Complete`
- Reviewed on: `2026-04-15`
- Reviewed tree: working tree rooted at commit `ddc5c0d52aff` with local proposal/review drafts and modified Rust control-plane crates
- Proposal / docs reviewed:
  - `docs/proposals/047-session-reuse-and-cancellation-settlement.md`
  - `.review-baselines/current-system-baseline.md`
  - `docs/reference/session-lineage-reuse-and-operator-reset.md`
  - `docs/reference/run-control.md`
  - `docs/reference/execution-truth-and-recovery.md`
  - `docs/reference/acp-runtime-transport.md`
- Reusable baseline used:
  - `.review-baselines/current-system-baseline.md`
  - `docs/reference/session-lineage-reuse-and-operator-reset.md`
  - `docs/reference/run-control.md`
  - `docs/reference/execution-truth-and-recovery.md`
  - `docs/reference/acp-runtime-transport.md`
- Baseline refreshed:
  - targeted reread of the stable session-lineage / operator-reset reference
  - targeted reread of the stable run-control reference
  - targeted reread of the stable execution-truth reference
  - targeted reread of the stable ACP runtime transport reference
  - targeted code refresh for current Swift session-reuse transport ownership and current Rust ACP / DB / run-reader seams
- Baseline freshness: `Partially refreshed`
- Proposal-specific integration context: none
- External research used: `None`
- Code areas inspected:
  - `Chainworks Forge/Engine/ContextBudgetGuard.swift`
  - `Chainworks Forge/Models/AgentSessionLineage.swift`
  - `Chainworks Forge/Engine/SessionReusePolicy.swift`
  - `Chainworks Forge/Models/Run.swift`
  - `Chainworks Forge/Engine/RunCancellationCoordinator.swift`
  - `Chainworks Forge/Engine/RuntimeSessionBridge.swift`
  - `Chainworks Forge/Engine/ACPAdapters/ClaudeAgentACPTransport.swift`
  - `control-plane/crates/acp/src/lib.rs`
  - `control-plane/crates/acp/src/manager.rs`
  - `control-plane/crates/acp/src/transport.rs`
  - `control-plane/crates/acp/src/adapters/claude.rs`
  - `control-plane/crates/db/migrations/002_projections.sql`
  - `control-plane/crates/db/src/repos/projections.rs`
  - `control-plane/crates/db/src/repos/runs.rs`
  - `control-plane/crates/db/src/work_item.rs`
  - `control-plane/crates/domain/src/agent.rs`
  - `control-plane/crates/domain/src/run.rs`
  - `control-plane/crates/graphql-server/src/schema.rs`
  - `control-plane/crates/graphql-server/src/types/run.rs`
  - `control-plane/crates/mcp-server/src/tools/runs.rs`
  - `control-plane/crates/workflow/src/catalog.rs`
- Current repo contradictions found:
  - the old review basis is materially stale: the current draft now correctly restores the stable budget decision mapping, execution-first cancellation settlement, and generic `FreshAfterInvalidation` reuse taxonomy
  - the current Rust ACP path is still one-shot: adapters spawn a fresh subprocess per invoke, `run_acp_session` always performs `session/close` and subprocess shutdown, and `AcpRuntimeManager` owns no live session registry
  - the current DB already has a legacy `session_lineages` table from `002_projections.sql` with an incompatible shape, so the draft cannot treat `session_lineages` as a greenfield table without an explicit migration / rename / backfill strategy
  - the current GraphQL/MCP run-reader split is real, and the draft maps it in the right direction
- Remaining blockers:
  - live ACP session reuse still lacks a transport-lifetime owner in the proposal
  - migration strategy for the already-existing `session_lineages` table is still missing

## 1. Executive Summary
- Overall readiness: `Red`
- Confidence: `High`
- Proposal completeness signal: `The draft has closed the stale review findings, but it is still not implementable as written because the reuse transport owner and legacy schema migration are underspecified`
- Top residual implementation risks:
  1. The reuse section still assumes a `sessionId` can be resumed by transport logic alone, while the current Rust ACP stack destroys the subprocess and closes the session after every invocation.
  2. The new lineage schema collides with an already-shipped `session_lineages` table, and the proposal does not say how existing databases migrate safely.

## 2. Proposal Scope and Completeness
- In scope:
  - durable ACP session lineage in the Rust daemon
  - generation-scoped context budget
  - two-phase cancellation settlement with durable evidence
  - northbound run-reader surfacing for cancellation settlement
- Out of scope:
  - implementation audit or gate execution
  - thin-client UI work
  - unrelated delivery / approval proposals
- Most important baseline refreshes performed:
  - stable session-lineage / operator-reset contract
  - stable run-control contract
  - stable execution-truth contract
  - stable ACP transport ownership contract
  - current Rust ACP session lifecycle, DB migration, and GraphQL/MCP run-reader code paths
- Most important confirmations against current repo:
  - the current draft now correctly ports execution-side session provenance onto `AgentExecution`
  - the current draft now correctly restores execution-first cancellation settlement and the single-run vs list-reader split
  - the current draft now correctly restores `FreshAfterInvalidation` and the compact-vs-invalidate budget mapping
  - the remaining gaps are no longer the old stale findings; they are transport-lifetime ownership and migration strategy

## 3. Proposal Readiness Verdict
- `Readiness = Red`
- `Confidence = High`
- `Evidence Completeness = Complete`

This is not an Evidence Gap Review. Local proposal, baseline, and current-code evidence are sufficient.

## 4. Discipline Scorecard
| Discipline | Readiness | Confidence | Evidence Completeness | Critical | High | Medium | Low |
|---|---|---|---|---:|---:|---:|---:|
| UI | Green | High | Complete | 0 | 0 | 0 | 0 |
| UX | Green | High | Complete | 0 | 0 | 0 | 0 |
| Architecture | Red | High | Complete | 0 | 2 | 0 | 0 |

## 5. Findings by Discipline

### 5.1 UI Findings
- No live UI `proposal-text` finding.

### 5.2 UX Findings
- No live UX `proposal-text` finding.

### 5.3 Architecture Findings

#### ARCH-001 - Live ACP session reuse still lacks a transport-lifetime owner
- Severity: `High`
- Confidence: `High`
- Evidence Completeness: `Complete`
- Evidence IDs: `DOC-01`, `DOC-06`, `MAP-01`, `MAP-02`, `INT-01`, `REAL-01`
- Proposal refs:
  - `docs/proposals/047-session-reuse-and-cancellation-settlement.md:194`
  - `docs/proposals/047-session-reuse-and-cancellation-settlement.md:196`
  - `docs/proposals/047-session-reuse-and-cancellation-settlement.md:359`
  - `docs/proposals/047-session-reuse-and-cancellation-settlement.md:369`
- Current repo refs:
  - `control-plane/crates/acp/src/manager.rs:17`
  - `control-plane/crates/acp/src/manager.rs:53`
  - `control-plane/crates/acp/src/adapters/claude.rs:77`
  - `control-plane/crates/acp/src/transport.rs:433`
  - `control-plane/crates/acp/src/transport.rs:663`
  - `Chainworks Forge/Engine/ACPAdapters/ClaudeAgentACPTransport.swift:41`
  - `Chainworks Forge/Engine/RuntimeSessionBridge.swift:263`
- Why it matters:
  - The draft correctly identifies that reuse must eventually land on `session/prompt` against an existing provider session. But the current Rust ACP topology is not a long-lived transport with active-session ownership. Today each adapter spawns a fresh subprocess per execution, `run_acp_session` always performs `session/new`, then `session/close`, then process shutdown, and `AcpRuntimeManager` keeps no live session registry. The stable Swift path works because transport adapters hold `activeSessions` and `RuntimeSessionBridge.executeInExistingSession(...)` submits prompts onto that still-live transport-owned session. `P047` only names `acp/src/transport.rs` and says the transport should skip `session/new` + `initialize`. That is not enough against the current Rust topology because there is no owner for a persistent subprocess/session handle across invocations.
- Required fix:
  - Add an explicit Rust owner path for live ACP sessions across invocations.
  - Name the file/module owners that keep live session handles and subprocesses resident across reuse, for example:
    - `acp/src/manager.rs`
    - `acp/src/session.rs` or an equivalent live-session registry module
    - provider adapters that currently spawn one-shot subprocesses
  - Clarify whether active sessions are adapter-owned, manager-owned, or registry-owned, and how the executor resolves `generation.provider_session_id` back to a live transport handle before calling `session/prompt`.
  - Make cancellation settlement close those same live handles, not just persisted session IDs.

#### ARCH-002 - The proposal still has no migration strategy for the already-existing `session_lineages` table
- Severity: `High`
- Confidence: `High`
- Evidence Completeness: `Complete`
- Evidence IDs: `DOC-01`, `MAP-03`, `INT-02`, `REAL-02`
- Proposal refs:
  - `docs/proposals/047-session-reuse-and-cancellation-settlement.md:78`
  - `docs/proposals/047-session-reuse-and-cancellation-settlement.md:359`
  - `docs/proposals/047-session-reuse-and-cancellation-settlement.md:394`
- Current repo refs:
  - `control-plane/crates/db/migrations/002_projections.sql:2`
- Why it matters:
  - The draft treats `session_lineages` as a new table with a completely different shape from the one already shipped in `002_projections.sql`. The existing table has columns like `stage_id`, `lineage_kind`, and `previous_session_id`; the draft’s target schema has `agent_id`, `lineage_id`, `session_reuse_scope`, `session_family_id`, and `active_generation_id`. A migration file cannot simply `CREATE TABLE session_lineages (...)` again on a database that already contains that table. Without an explicit alter/rename/backfill/drop strategy, implementation is blocked at the schema boundary and existing installs have no defined upgrade path.
- Required fix:
  - Replace the greenfield wording with an explicit migration strategy for legacy databases.
  - State one of:
    - rename the old `session_lineages` table and create the new canonical table,
    - transform the old table in place with `ALTER TABLE` + backfill,
    - or intentionally discard/replace the old rows with a documented compatibility policy.
  - Update acceptance so migration is proved against a database already migrated through `002_projections.sql`, not only against an empty database.

## 6. Cross-Discipline Conflicts and Decisions
- Locked decision:
  - the old budget, cancellation-owner, and invalidation-taxonomy findings are stale on the current draft and should not be carried forward
- Conflict:
  - the draft describes reuse as if `sessionId` alone were enough, but the current Rust ACP path has no transport-owned live session lifetime
  - decision needed: choose and name the owner for persistent ACP session handles across invocations
- Conflict:
  - the draft describes `session_lineages` as a new table even though current DB migration `002` already created one with incompatible columns
  - decision needed: choose and specify the legacy migration path before implementation starts

## 7. Prioritized Action Backlog
| Priority | Item | Discipline | Owner | Horizon | Dependencies | Success Metric | Source Findings |
|---|---|---|---|---|---|---|---|
| P1 | Specify the transport-lifetime owner for live ACP session reuse | Architecture | proposal author | Before next review | current Rust ACP manager/adapter topology | proposal names the module(s) that keep provider sessions/subprocesses alive across invocations and cancellation closes those same handles | `ARCH-001` |
| P1 | Add an explicit migration strategy for the legacy `session_lineages` table | Architecture | proposal author | Before next review | current DB schema from `002_projections.sql` | proposal proves how existing DBs upgrade without table-name collision or silent row loss | `ARCH-002` |

## 8. Validation and Measurement Plan
| Area | What Will Be Measured | Leading Indicators | Guardrails | Review Checkpoint | Rollback / Hold Criteria |
|---|---|---|---|---|---|
| Reuse transport ownership | whether reused generations can bind back to a live ACP session instead of a dead historical `sessionId` | proposal names manager/registry/adapter owners for active session handles | no "transport just skips session/new" shorthand without lifetime owner remains | next proposal review | hold if the proposal still assumes session reuse without a live-session registry |
| Legacy schema migration | whether the lineage migration applies on already-initialized databases | migration section names rename/alter/backfill/discard policy for `session_lineages` from `002` | no greenfield `CREATE TABLE session_lineages` ambiguity remains | next proposal review | hold if migration still reads as create-new-table-only |

## 9. Evidence Gaps and Open Questions

### Evidence Gaps
- GAP-01: No blocking evidence gap remains. Local proposal, baseline, and current-code evidence are sufficient.

### Open Questions
- QUESTION-01: Should Rust ACP live-session ownership sit in `AcpRuntimeManager`, inside each adapter, or in a dedicated session-registry layer under `acp/src/session.rs`?
- QUESTION-02: Does the author want to preserve, transform, or intentionally discard rows from the legacy `002_projections.sql` `session_lineages` table?

## 10. Evidence Gap Review Fallback

Not used. Evidence completeness is `Complete`.
