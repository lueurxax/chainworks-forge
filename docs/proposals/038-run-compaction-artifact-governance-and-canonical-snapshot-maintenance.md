# Proposal 038: Run Compaction, Artifact Governance, and Canonical Snapshot Maintenance

| Field | Value |
|---|---|
| Date | 2026-04-01 |
| Status | Draft |
| Author | Engineer (single-engineer project) |
| Depends on | Current execution truth, recovery truth, proposal-loop fidelity, session-lineage truth, artifact hierarchy, and target-state direction toward a Rust local control plane |
| Goal | Introduce a server-owned `Compact Run` operation that aggressively reduces artifact noise in completed, failed, and blocked runs by archiving superseded artifacts, deduplicating duplicates, repairing broken linkages, rebuilding projections, and emitting compaction maintenance artifacts that feed the existing shell-owned inspection and reporting surfaces. |

---

## 1. Why this proposal exists

Runs are accumulating too much operational noise.

In practice this currently means:

- stage retries and repeated review/refine loops create many generations of artifacts,
- duplicate or near-duplicate receipts and summaries accumulate,
- projections become harder to inspect and slower to reason about,
- report and comparison surfaces become harder to navigate,
- old stage attempts and superseded artifacts keep crowding the active run surface,
- operator effort shifts from “understand what happened” to “find the one thing that still matters.”

This is no longer a cosmetic problem.

It is now a product problem because artifact noise degrades:

- run inspectability,
- operator trust,
- comparison quality,
- report readability,
- and eventually overall system usability.

Proposal 038 introduces a **radical, server-owned compaction operation**.
This is not a view-only trick.
It is a real maintenance command that shrinks the active artifact surface of a run while preserving canonical truth.

---

## 2. Product outcome

After Proposal 038:

- a completed / failed / blocked run can be compacted,
- superseded and low-signal artifacts leave the active run surface,
- exact duplicates are collapsed,
- broken pointers and stale projection inconsistencies are repaired,
- new compaction maintenance artifacts are emitted for shell-owned readers,
- the operator sees a dramatically smaller, clearer active view of the run,
- archival truth remains recoverable when needed.

This proposal is successful when compaction makes the run easier to inspect **without** destroying evidence required for recovery, reports, comparison, or auditability.

---

## 3. Scope

## 3.1 In scope

- a single server-owned `Compact Run` command
- artifact classification
- archive-eligible detection
- exact duplicate detection
- link repair
- projection rebuild
- `run_compaction_plan`
- `run_compaction_report`
- `run_compaction_snapshot`
- optional model-assisted semantic summary / clustering
- GraphQL mutation for UI
- MCP tool for external operators
- compacted active run surface

## 3.2 Out of scope

- compaction for running runs
- workflow-state mutation
- changing stage outcomes
- deleting immutable reports
- deleting recovery-critical evidence
- deleting active session lineage truth
- rewriting run history to hide what happened
- using the model as the owner of destructive maintenance

---

## 4. Eligibility

Compaction is allowed only for runs in these states:

- `completed`
- `failed`
- `blocked`

Compaction is **not** allowed for:

- `running`
- `ready`
- `waitingApproval`
- `pending`

This is an explicit product rule for Proposal 038.

### Why this restriction exists

Compaction is allowed to be radical.
That means it should only operate once the active live frontier of the run is no longer changing.

The current goal is to reduce operator pain and artifact noise without risking live execution integrity.

If future evidence shows a need for partial compaction on running runs, that should be a separate proposal.

---

## 5. Core command

The system introduces one canonical maintenance command:

## `Compact Run`

This command performs all of the following in one coordinated server-side operation:

1. freezes the run’s canonical frontier,
2. classifies all artifacts and evidence,
3. archives superseded and low-signal artifacts,
4. collapses exact duplicates,
5. repairs stale pointers and broken references,
6. rebuilds run-facing projections,
7. emits compaction maintenance artifacts,
8. verifies that report/recovery/compare readers still function.

There is no lightweight “preview-only” mode in this proposal.
The command is intentionally operational and meaningful.

---

## 6. Server ownership and model role

## 6.1 Server-owned maintenance

The server is the only owner of compaction truth.

The server decides:

- what is canonical,
- what is archive-eligible,
- what can be deduplicated,
- what link repairs are legal,
- what projections must be rebuilt,
- and whether compaction succeeded.

## 6.2 Model-assisted semantic layer

A model may assist in one bounded role:

- clustering semantically similar summaries,
- identifying candidate promoted artifacts,
- producing a human-readable compaction summary,
- suggesting semantic duplicate groups.

But the model may **not**:

- decide destructive deletion,
- decide canonical history,
- repair links based only on “looks similar” reasoning,
- rewrite session lineage,
- or become the source of truth for archive policy.

This keeps semantic help available without turning compaction into opaque magic.

---

## 7. Canonical preservation rules

Compaction may be aggressive, but some classes of data are always preserved.

## 7.1 Must preserve

The following must remain intact and queryable after compaction:

- immutable reports
- latest run summary
- canonical run status and stage truth
- approval history
- recovery-critical evidence
- unresolved score-lift backlog / equivalent unresolved issue state
- latest valid proposal artifact
- latest valid review corpus for the terminal meaningful state
- pinned/promoted artifacts
- canonical runtime receipts/provenance
- session lineage truth
- compaction plan/report/snapshot artifacts themselves

## 7.2 May archive

The following may leave the active run surface and move to archive:

- superseded proposal drafts
- superseded revision summaries
- stale raw transcripts not needed by canonical readers
- exact duplicate artifacts
- superseded stage-attempt artifacts
- orphaned low-signal artifacts
- noisy intermediate outputs with no current evidence value

## 7.3 Must not do hard delete by default

Proposal 038 prefers:

- archive
- tombstone
- pointer-to-archive
- compact bundle

over immediate irreversible deletion.

The active run surface should shrink radically.
But forensic recovery should still be possible.

---

## 8. Artifact classification model

Every artifact touched by compaction must be assigned to one of these classes:

- `canonical`
- `promoted`
- `latest_summary`
- `immutable_history`
- `recovery_critical`
- `report_critical`
- `session_lineage_related`
- `superseded`
- `duplicate_exact`
- `orphaned`
- `broken_link_candidate`
- `archive_eligible`
- `manual_review_required`

These classes are server-derived, not manually tagged.

### 8.1 Canonical persistence owners

Compaction must **not** introduce a second artifact-truth lane. The ownership split is explicit:

- **Durable artifact truth** remains on canonical persistence:
  - `Artifact` continues to own identity, lineage, and durable supersedence/immutability.
  - `Run` continues to own latest pointers (`latest_summary`, `latest_report`, latest comparison anchors).
  - Any class that changes long-lived truth (`canonical`, `promoted`, `latest_summary`, `immutable_history`, `recovery_critical`, `report_critical`, `session_lineage_related`) is persisted only via those canonical owners.
- **Compaction-only classifications** live in compaction artifacts:
  - `run_compaction_plan`, `run_compaction_report`, and `run_compaction_snapshot` carry `archive_eligible`, `duplicate_exact`, `orphaned`, `broken_link_candidate`, `manual_review_required`, and the per-artifact action outcome.
  - These classifications are **advisory / maintenance-only**, and must never be treated as canonical artifact truth by readers.
- **Archive/tombstone state** is owned by a single, explicit store:
  - if compaction archives an artifact, the archive pointer/tombstone is persisted on the canonical `Artifact` record (or a single run-owned maintenance table), and **all readers** consult that one owner.
  - compaction artifacts may reference the archive/tombstone decision, but they are not the source of truth.

If this split is violated, compaction is considered invalid and must not be applied.

---

## 9. New first-class artifacts

Proposal 038 introduces these new artifacts and contracts.

## 9.1 `run_compaction_plan`
Created before destructive/archive actions are applied.

Purpose:
- say what will be preserved,
- what will be archived,
- what duplicates were found,
- what repairs are intended,
- what projections will be rebuilt.

## 9.2 `run_compaction_report`
Created after compaction completes.

Purpose:
- counts before/after,
- archived artifacts,
- duplicate collapse results,
- repaired inconsistencies,
- unresolved repair issues,
- projection rebuild result,
- links to compact bundle/snapshot.

## 9.3 `run_compaction_snapshot`
A compacted maintenance snapshot of the run after maintenance.

Purpose:
- summarize the post-compaction active run shape,
- preserve latest meaningful state,
- expose compacted report/recovery inputs for shell-owned readers,
- support deterministic compare/recovery/report rendering without introducing a new reader authority.

## 9.4 `semantic_compaction_summary` (optional)
Produced only if model-assisted semantic summarization is enabled.

Purpose:
- explain what changed in human terms,
- cluster similar attempts/summaries,
- highlight promoted artifacts,
- summarize what noise was removed.

This artifact is explanatory only, never authoritative.

---

## 10. Algorithm

## Phase 1 — freeze canonical frontier

The server determines the run’s preserved frontier:

- canonical run record
- canonical stage state
- terminal/meaningful latest proposal
- terminal/meaningful latest review bundle
- unresolved backlog/recovery state
- reports
- approvals
- session lineage metadata
- runtime provenance

This frontier becomes the protection boundary.

## Phase 2 — graph scan

The server scans:

- run
- stages
- agent executions
- approvals
- artifacts
- reports
- runtime receipts
- session lineage references
- projections

## Phase 3 — classify

Everything outside the protected frontier is classified into:
- keep
- archive
- deduplicate
- repair
- unresolved/manual-review-required

## Phase 4 — optional semantic assist

If enabled, the model may:

- cluster semantically similar revision summaries,
- cluster semantically similar reviewer summaries,
- suggest promoted artifacts,
- produce `semantic_compaction_summary`.

This phase cannot make destructive decisions.

## Phase 5 — deterministic apply

The server applies compaction:

- archive archive-eligible artifacts
- collapse exact duplicates
- write tombstones/archive pointers
- repair legal links
- rebuild projections
- emit plan/report/snapshot artifacts

## Phase 6 — verification

The server verifies that all required **shell-owned** consumers still work:

- run summary readers
- `RunArtifactHierarchyView`
- `RunReportView`
- `RunComparisonView`
- `RecoverySheet`
- `BlockedRunRecoveryView`

Verification is against the existing shell-owned readers rendering post-compaction truth, not against any snapshot-first reader lane.

If verification fails, compaction **must not** create a competing reader authority.

Failure semantics:
- The server **rolls back** to the pre-compaction canonical truth:
  - canonical `Artifact` + `Run` pointers are restored,
  - archive/tombstone writes are reverted or left quarantined and ignored,
  - `run_compaction_snapshot` is marked invalid and never used by readers.
- The compaction report must explicitly describe:
  - the rollback reason,
  - which actions were attempted,
  - which actions were reverted,
  - any artifacts left in quarantine.

Readers always follow canonical owners, even after a failed compaction.

---

## 11. GraphQL and MCP exposure

## 11.1 GraphQL

Add a mutation:

- `compactRun(runId: ID!): CompactRunResult!`

Suggested result shape:
- `compactionId`
- `status`
- `planArtifactId`
- `reportArtifactId`
- `snapshotArtifactId`
- `warnings`
- `archivedArtifactCount`
- `deduplicatedArtifactCount`

GraphQL should also expose:
- compacted run status
- compact snapshot artifact
- archive summary
- compaction reports

## 11.2 MCP

Add a northbound MCP tool:

- `runs.compact`

This tool should:
- validate run eligibility,
- execute compaction,
- return the resulting compaction report identifiers and summary.

MCP clients should not need special knowledge of internal storage details.

---

## 12. Interaction with UI and active run surfaces

After compaction, the UI should show:

- one clear latest proposal path
- one clear latest review path
- one clear unresolved backlog / report path
- a compacted artifact hierarchy
- a visible note that the run has been compacted
- access to the compaction report
- optional access to archived artifacts in a separate secondary view

The default run view after compaction should be dramatically quieter.

### 12.1 Shell-owned reader binding

Compaction **must not** introduce a parallel snapshot-first reader surface.

Post-compaction inspection remains anchored to the existing shell-owned readers:
- `RunArtifactHierarchyView` continues as the canonical artifact browser, now rendering compaction metadata.
- `RunReportView` and `RunComparisonView` remain the only report/comparison readers.
- `RecoverySheet` and `BlockedRunRecoveryView` remain the only recovery readers.

`run_compaction_snapshot` and `run_compaction_report` are **inputs** to those readers, not alternative entry points.

---

## 13. Risks

## 13.1 Over-aggressive archiving
Risk:
useful evidence disappears from active view too early.

Mitigation:
- explicit preservation rules
- archive instead of delete
- compaction report with counts and links
- shell-owned reader validation against compaction artifacts

## 13.2 Broken readers after compaction
Risk:
reports, comparisons, or recovery surfaces still point to archived or removed artifacts.

Mitigation:
- deterministic verification phase
- projection rebuild
- shell-owned readers remain the only post-compaction inspection surfaces
- `run_compaction_snapshot` and `run_compaction_report` are validated as inputs to those readers (no snapshot-first reader lane)

## 13.3 Model semantic overreach
Risk:
semantic assistant starts deciding what should be removed.

Mitigation:
- model remains advisory only
- destructive apply stays deterministic and server-owned

## 13.4 False duplicate collapse
Risk:
two artifacts with similar meaning but different canonical role get merged incorrectly.

Mitigation:
- exact duplicates first by checksum/content
- semantic clustering never auto-collapses canonical evidence

---

## 14. Acceptance criteria

Proposal 038 is complete only when all of the following are true:

1. a completed, failed, or blocked run can be compacted by one server-owned command;
2. superseded and noisy artifacts leave the active run surface;
3. exact duplicates are collapsed or archived;
4. broken links and stale projection issues are repaired where deterministically possible;
5. immutable reports, latest summaries, approvals, session lineage truth, and recovery-critical evidence remain intact;
6. a `run_compaction_snapshot` is emitted as a maintenance input to the shell-owned readers;
7. the compacted run is materially easier to inspect than before;
8. compaction is available via GraphQL and MCP;
9. compaction is impossible for running runs.

---

## 15. Final recommendation

Proposal 038 should be treated as a necessary operational maintenance feature, not a cosmetic enhancement.

The system now produces enough runs, retries, repeated stages, and artifact generations that compaction is required to keep the product inspectable.

The right design is:

- aggressive enough to reduce real noise,
- conservative enough to preserve canonical truth,
- server-owned,
- optionally model-assisted,
- and explicitly limited to `completed`, `failed`, and `blocked` runs.

That will give the system a much cleaner active artifact surface without sacrificing the ability to investigate or explain what happened.
