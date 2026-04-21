# Consolidated Proposal Review

## 0. Review Mode and Proposal Evidence Summary
- Mode used: `proposal-readiness`
- Evidence completeness: Complete
- Proposal / docs reviewed: P044 R2, current system baseline, review baseline, idea lifecycle, project workspace contract, domain model, test gates, P029 northbound capability proposal
- Reusable baseline used: `.review-baselines/current-system-baseline.md` and `docs/reference/current-system-baseline.md`
- Baseline reused: idea lifecycle, workspace ownership, test-gate ownership, northbound auth/capability model
- Baseline refreshed: targeted Rust code map for idea repo, run start, MCP, GraphQL, command journal, auth, and gates
- Baseline freshness: Partially refreshed
- Proposal-specific integration context: none existed before this review
- Targeted context refresh performed: yes, code/docs only
- External research used: None
- Code areas inspected: `domain`, `db`, `engine`, `mcp-server`, `graphql-server`, `auth`, `scripts/test-gate.sh`, `docs/reference/test-gates.md`
- Current repo contradictions found: yes, around `runs.start` lifecycle transition and `update_status` caller assumptions
- Runtime evidence used: None
- Provenance of key evidence: see `evidence-pack.md`
- Remaining assumptions: dirty tree is the intended review target; no runtime gate required in this mode
- Remaining blockers: StartRun lifecycle contract and atomic archive guard

## 1. Executive Summary
Overall readiness: Red  
Confidence: High  
Proposal completeness signal: Mixed

P044 R2 is substantially stronger than a scaffold: it specifies tool names, patch semantics, command journaling, auth capabilities, GraphQL parity, and the distinct `proposal-044-ideas` gate. It is not ready to implement because the proposal's lifecycle matrix omits the existing `runs.start` path that already mutates idea status. That omission creates a direct bypass around the new Draft/Archived guards and makes the proposed `update_status` change unsafe.

Top risks:
1. `runs.start` can currently activate any found idea; P044's matrix says Draft->Active is only via `ideas.update`, and Archived updates require `ideas.unarchive`.
2. The archive active-run guard is specified as check-then-update, but run creation is a separate command path, so the guard is not atomic.
3. The proposal's only dependency is a stale `domain-model.md` slice, while canonical lifecycle/workspace truth lives in `idea-lifecycle.md` and `project-workspace-contract.md`.

## 2. Proposal Scope and Completeness
In scope is well defined: MCP and GraphQL idea lifecycle tools, list filters, command-journaled lifecycle writes, auth capabilities, and a new proof gate. Out of scope is also explicit: Swift UI, deletion, bulk operations, and external auth changes.

The proposal is strongest on protocol and audit shape. The weakest area is cross-command lifecycle ownership: idea state is not only changed through the new idea tools. Existing `StartRun` already changes ideas, so P044 must either make `runs.start` part of the same lifecycle state machine or explicitly forbid it from transitioning Draft/Archived ideas.

## 3. Discipline Scorecard
| Discipline | Readiness | Confidence | Evidence Completeness | Critical | High | Medium | Low |
|---|---|---|---|---:|---:|---:|---:|
| Architecture | Red | High | Complete | 1 | 0 | 1 | 0 |
| Reliability | Red | High | Complete | 0 | 1 | 0 | 0 |
| Performance | Green | Medium | Complete | 0 | 0 | 0 | 0 |
| Security | Amber | High | Complete | 0 | 0 | 1 | 0 |
| Product (optional) | Not scored | Medium | Complete | 0 | 0 | 0 | 0 |

## 4. Findings

### 4.1 Critical

Finding ID: P044-CRIT-01  
Discipline: Architecture / Reliability  
Severity: Critical  
Confidence: High  
Evidence IDs: DOC-02, DOC-03, DOC-05, MAP-03, MAP-05, REAL-01, REAL-02, REAL-04  
Summary: P044's lifecycle matrix omits `runs.start`, but current `CommandHandler::StartRun` inserts a run and then unconditionally sets the idea to `Active`. After P044 changes `ideas::update_status` from `COALESCE(?2, archived_at)` to unconditional `archived_at = ?2`, that same StartRun path can also clear `archived_at` when it marks an archived idea Active. This contradicts the proposal's own claims that Draft->Active is guarded by non-empty title/body, Archived ideas must be restored via unarchive, and duplicated Draft ideas are inert.

Why it matters: Implementing P044 as written can leave the new idea tools correct while the existing run-start surface still violates the lifecycle contract. An empty Draft could become Active by starting a run, and an Archived idea could be silently restored by `runs.start` instead of `ideas.unarchive`.

Recommended fix: Add `runs.start` to the lifecycle contract before implementation. The proposal must choose one rule and make it explicit: either StartRun may transition Draft->Active only after validating non-empty title/body and rejecting Archived ideas, or StartRun must reject all Draft/Archived ideas and require prior `ideas.update` / `ideas.unarchive`. Also replace the generic unsafe `update_status` call with lifecycle-specific helpers or a status-transition service so StartRun cannot accidentally clear `archived_at`.

Acceptance criteria:
- `runs.start` against an Archived idea fails, records a failed command journal row, and inserts no run.
- `runs.start` against a Draft idea with empty title or body fails, records a failed command journal row, and leaves status unchanged.
- The proposal's status matrix lists the chosen StartRun behavior explicitly.
- Tests cover MCP and GraphQL run-start attempts against archived, empty-draft, valid-draft, and duplicated-draft ideas.
- `archived_at` can only be cleared by the unarchive path, unless the proposal explicitly defines another legal transition.

### 4.2 High

Finding ID: P044-HIGH-02  
Discipline: Reliability  
Severity: High  
Confidence: High  
Evidence IDs: DOC-08, MAP-04, MAP-05, INT-02, TEST-05  
Summary: The active-run archive guard is specified as `runs::list_by_idea` followed by `ideas::update_status`, but current run creation is a separate StartRun command that can insert a run independently. Without a transaction, lock, or conditional write, archive can pass its check and then race with a new run insert for the same idea.

Why it matters: `idea-lifecycle.md` says an idea may not be archived while active, waiting on approval, or live in-flight. A check-then-update guard does not enforce that invariant under concurrent command handling.

Recommended fix: Specify an atomic lifecycle write. Prefer one command-handler transaction using SQLite transaction/IMMEDIATE semantics for archive eligibility plus status update, and make StartRun's idea eligibility check plus run insert part of a compatible transaction. Alternatively, implement archive as a single conditional update guarded by `NOT EXISTS` non-terminal runs, with reliable affected-row checks.

Acceptance criteria:
- Archive fails if a non-terminal run exists at the moment the archive status write commits.
- StartRun cannot insert a new run for an idea whose archive transition has committed.
- Tests prove archive/start interleaving cannot produce an archived idea with a non-terminal run.
- Failed archive attempts keep command-journal failure evidence.

### 4.3 Medium

Finding ID: P044-MED-03  
Discipline: Architecture / Security  
Severity: Medium  
Confidence: High  
Evidence IDs: DOC-06, DOC-08, DOC-09, DOC-10, DOC-12, REAL-03, DATA-06  
Summary: P044's dependency row cites only `../reference/domain-model.md`, but that doc is a SwiftData-oriented model reference and currently still lists `IdeaStatus` as `draft`, `active`, `completed`, `failed`. The active Rust code and canonical lifecycle doc use `draft`, `active`, `archived`; the workspace root edit contract is in `project-workspace-contract.md`; the northbound capability model is in P029 and test-gate references.

Why it matters: The proposal body mostly follows current Rust reality, but its declared source chain points reviewers and implementers at a stale status vocabulary and omits the docs that define archive eligibility, restore semantics, and workspace-root freeze behavior.

Recommended fix: Update the `Depends on` row and relevant text to include `idea-lifecycle.md`, `project-workspace-contract.md`, `docs/reference/current-system-baseline.md`, `docs/reference/test-gates.md`, and P029/northbound capability references. Add a short note that Rust `domain::idea::IdeaStatus` plus `idea-lifecycle.md` are controlling for this slice, not the stale SwiftData status list in `domain-model.md`.

Acceptance criteria:
- The dependency row names the canonical lifecycle and workspace docs.
- P044 includes tests that updating `workspace_root_path` does not mutate existing run workspace truth.
- Any status vocabulary conflict with `domain-model.md` is resolved or explicitly scoped out.

### 4.4 Medium

Finding ID: P044-MED-04  
Discipline: Security / Migration  
Severity: Medium  
Confidence: Medium  
Evidence IDs: DOC-12, MAP-08, MAP-09, MAP-12, DATA-05  
Summary: P044 renames the read capability from `IdeasList` to `IdeasRead`, but it does not spell out compatibility expectations. Current principal table rows persist only token/id/class and derive capabilities at runtime, which lowers DB migration risk, but `Principal` and `CapabilityToolId` are serializable Rust API types and P029 treats the capability enum as a stable northbound contract.

Why it matters: The rename is semantically correct because `ideas.get` and `ideas.list` share read access, but implementation could break serialized fixtures, external config, or tests that mention `IdeasList` without a planned alias/removal posture.

Recommended fix: Add a short migration note: either `IdeasList` is intentionally replaced with no serde alias because no persisted principal capabilities exist, or support `#[serde(alias = "IdeasList")]` / explicit fixture migration for one release window. Also update P029/test-gate references that enumerate current capability variants.

Acceptance criteria:
- Tests prove existing principal table files still load after the rename.
- Converter tests prove both `ideas.list` and `ideas.get` map to the final read capability.
- If no alias is provided, the proposal states why no serialized compatibility path is needed.

## 5. Proposal Completeness Gaps
- Missing failure-state coverage: StartRun lifecycle rejection for Archived ideas, empty Draft ideas, and duplicated Draft ideas.
- Missing reliability detail: archive guard atomicity with concurrent run creation.
- Missing dependency detail: canonical lifecycle/workspace/northbound references are not in the dependency row.
- Missing migration detail: read-capability rename compatibility posture.
- Missing telemetry detail: lifecycle-specific rejection messages/signals for StartRun are not specified.

## 6. Current Repo Contradictions
- CONTRA-01: P044 says all status transitions are governed by its matrix, but current StartRun changes idea status to Active.
- CONTRA-02: P044 says the `update_status` change is backward-compatible because only archive/unarchive paths matter, but current StartRun is an existing caller.
- CONTRA-03: P044 says duplicate Draft ideas are inert until Active, but current StartRun can activate Draft ideas directly.
- CONTRA-04: P044 depends only on `domain-model.md`, whose status vocabulary conflicts with current Rust/archive truth.

## 7. Required Changes Before Implementation
- MUST-01: Add StartRun to the idea lifecycle transition contract and tests.
- MUST-02: Specify an atomic archive eligibility/status-write mechanism and compatible StartRun guard.
- MUST-03: Replace or constrain `update_status` so unarchive is the only path that clears `archived_at`.
- MUST-04: Update dependency/source-of-truth references to include lifecycle, workspace, northbound auth, and test-gate docs.
- MUST-05: Document the `IdeasList -> IdeasRead` compatibility posture.

## 8. Follow-Up Questions
- Q-01: Should StartRun be allowed to activate a valid Draft idea, or should run start require the idea to already be Active?
- Q-02: Should `ideas.unarchive` always restore to Active, or should it restore to Draft when the archived idea has empty title/body?
- Q-03: Should `IdeasList` remain as a serde alias for `IdeasRead`, or is an immediate enum rename acceptable because principal tables do not persist capability sets?

## 9. Suggested Next Review Path
Next mode to run: `proposal-readiness` again after the five required changes.  
Baseline refresh needed: no broad refresh; only P044 adjacent docs/dependency row need correction.  
Research needed: no.  
Can move to implementation after fixes: yes, if StartRun lifecycle and archive atomicity are specified and tested.
