# Proposal 039: Blocked Run Fork and Canonical Carry-Forward

| Field | Value |
| --- | --- |
| Created / revised | 2026-04-12 / 2026-09-21 |
| Revision | `p039-r2` |
| Status | I1-I3 accepted; merged, published and deployed. Authorized P095 lifecycle correction completed; I4 preview now holds on the historical-reference budget. Migration and full closeout incomplete |
| Implementation | See [audit R2](039-blocked-run-fork-and-canonical-carry-forward_IMPLEMENTATION_AUDIT_R2.md), [offline evidence](../evidence/p039-durable-integration.md), [live rollout](../evidence/p039-live-rollout-2026-09-21.md) and [authorized correction](../evidence/p095-legacy-reconciliation-2026-09-21.md). No successor was reserved or activated; proposal retirement remains pending live acceptance |
| Owner | Rust control plane; SwiftUI is a read/approval client |
| First supported case | P095-like blocked implementation run, same repository and idea |
| Decision | New run with verified inputs, current compiled definitions, fresh review and approval |

## 1. Intent And Evidence

Continue useful work from a blocked run without losing source files or reusing
obsolete execution permissions, sessions, retries or approvals. Success is a
linked successor that safely proceeds under the current runtime, not merely a
second run ID or a copied directory.

The immediate case is P095, source run
`bd83a310-360f-4d45-b4c6-a94873de3733`. The 2026-09-20 preflight observed
`blocked / state_7_implementation_started`, no pending/running work items, no
unresolved release effects, and a clean worktree at
`82b1d72581e6503e5e19a40e5a1164b2b9f2ae3b`. Its frozen `CODE_WRITE` policy
has no `xcode_headless` grant although `code_writer` requires Xcode execution.
These are historical observations, not current eligibility authority.

The approved proposal was preserved with SHA-256
`04ecdd50e44cbcae1d0f1ce1dab6cc31280cce615551cda0dd61c25ba4523d44`.
The private preservation includes worktree/run/artifact files, historical
approvals and command records. It is not an importable execution snapshot.
See [integration context](039-blocked-run-fork-and-canonical-carry-forward.review/integration-context.md).

### Changes From The April Draft

| Old assumption | Current decision |
| --- | --- |
| UI button and GraphQL continuation mutation | MCP-only commands; GraphQL/SwiftUI readback and existing approval actions only |
| Carry the latest meaningful workflow/catalog | Freeze explicitly selected current compiler output; old snapshots are historical evidence |
| Resume at an unspecified frontier | Closed `implementation_restart_v1` entry: fresh proposal review, then fresh implementation approval |
| Blocked status is sufficient | Verify quiescence, output-repair ownership and ordinary/headless effects; acquire a durable execution fence |
| Source becomes immutable by declaration | Enforced source fence and lineage across command, queue, dispatch and settlement boundaries |
| P038 is a prerequisite | No dependency on compaction, deletion, UI cleanup or DB-size reduction |
| P064 barrier is assumed available | Schema/readback alone is not proof of an enforced execution fence |

## 2. Scope And Alternatives

1. In-place catalog rewrite is rejected: existing retrofit permits only
   escalation/backend changes; a permissions rewrite would alter frozen truth.
2. Normal StartRun plus manual output copies is rejected: it provides no typed
   lineage, preservation proof or race protection, and files can accidentally
   satisfy transitions.
3. Journalled preparation followed by atomic successor activation is selected:
   verified independent inputs, explicit ownership transfer and fresh authority.

V1 is same-repository, same-idea continuation using `implementation_restart_v1`.
A source must be blocked in the implementation preparation/loop/review region of
an explicitly supported `proposal_to_release` workflow, with a recorded
historical implementation approval and a selectable proposal. That old approval
is context, never new-run authority.

Excluded: proposal-only, active or terminal sources; cross-repository/idea forks;
release/publish/upload/push frontiers; arbitrary state jumps; multiple successors
per source; session resurrection; automatic approval; full Git-history copying;
merge/rebase; destructive cleanup; in-place permission upgrades.

Dirty work is preserved, not discarded for eligibility. Unmerged indexes,
submodules, unavailable LFS content, special files and unsupported external
source links hold executable materialization in v1. No automatic stash, commit,
reset, source edit or cleanup is allowed.

## 3. Document Ownership

This parent owns scope, invariants, workflow semantics and readiness. Normative
children share revision `p039-r2` and do not authorize deployment independently:

- [Runtime and wire](039/runtime-and-wire-contract.md): admission, APIs, schemas,
  persistence, materialization, fences, replay and recovery.
- [Verification and rollout](039/verification-and-rollout.md): implementation
  slices, acceptance matrix, migration, gates and separate live P095 proof.
- [Rollout declaration](../evidence/rollout-contract/p039-rollout-contract.json):
  machine-readable design intent; future gates are not existing proof.

Implemented references supersede older proposals:
[UI boundary](../reference/ui-action-boundary.md),
[API/auth](../reference/boundary-first-api-auth-contract.md),
[execution truth](../reference/execution-truth-and-recovery.md),
[per-run isolation](../reference/per-run-workspace-isolation.md),
[headless runtime](../reference/xcode-headless-runtime.md),
[rollout template](../reference/executable-rollout-gate-template.md).

## 4. Invariants

1. Source frozen snapshots, stages, agents, approval decisions and output
   provenance are never rewritten. Source remains `blocked`; lineage/fence
   records its historical role separately from RunStatus.
2. Preserve and verify source work before activation. During preparation retain
   an explicit hold. No P039 path calls CancelRun, worktree cleanup or reset.
3. Successor owns new run/stage/agent/session/generation/approval identities, a
   distinct worktree and metadata/output roots, with no source-path fallback.
4. Old results are input evidence. Tests, scores, accepted risks, rollout passes
   and approvals never become current transition authority. Future gates run.
5. Freeze current compiler output: permission profiles, provider bindings, skills
   and effective worktree strategy. Copied old task payloads cannot authorize work.
6. At most one successor activates per source, despite retry, crash, timeout,
   concurrent operators or expired transport-idempotency retention.
7. Filesystem/Git work occurs outside SQLite writer transactions. Activation is
   one bounded metadata transaction after verified materialization.
8. Unknown effects hold. Startup reconciles evidence but never automatically
   repeats an uncertain Git operation or activates a successor.
9. No implicit project-trust, Apple-consent, provider-access or capability grant.
10. Successor failure/cancellation, disabling P039 or rollback never automatically
    makes the historical source runnable again.

## 5. Inputs And Preservation

Preview partitions candidates into `execution_seed`, `reference_only`,
`preserve_only` or `excluded`, each with provenance and a reason. A filename or
pin alone is insufficient evidence of canonical identity or validity.

| Material | Successor treatment |
| --- | --- |
| Selected approved/current proposal verified against source metadata | New `proposal_current` input with original artifact identity/hash/role and observed historical approval relation |
| Idea brief | Input bound to source idea/run and idea digest |
| Reviews, unresolved findings, plan/backlog, handoff | Isolated read-only reference bundle, required reviewer/planner input when present, never current gate outputs |
| Committed and dirty/untracked source | New checkout at pinned source HEAD plus verified overlay; original index semantics preserved in archive only |
| Old tests/audits/self-assessment/release receipts | Historical evidence, no successful stage/active output/risk waiver |
| Old YAML/catalog/approval/journal records | Preserve-only audit evidence |
| Run-state, active index, counters, sessions/tokens, locks | Never execution seeds; runtime state remains source-local/private historical preservation only |
| Caches, builds and external machine configuration | Explicit exclusions; no dereference or executable carry-forward |

Metadata/checksum mismatch holds. If an old approval lacks a cryptographic
proposal binding, report `historical_binding_unproven`, not an invented binding.
A verified operator-selected proposal may still enter fresh review without any
exemption from new approval.

Mandatory unresolved findings include human decisions and scope/decomposition
blockers. Their presence reaches new reviewers/planner; a fork does not resolve
or waive them. New review explicitly dispositions them under the current target.

Preservation is a private checksummed copy on this host, not an offsite backup.
Its receipt identifies roots, retention pin, source Git ref/HEAD/tree, index and
patches, untracked/deleted inventory, exclusions and limits. Existing manual P095
preservation is supporting evidence; implementation must make current checks.

## 6. Fresh Workflow Entry

Add optional compiler metadata; workflows without it remain unchanged and are
not eligible targets. V1 accepts one closed profile, not an arbitrary state:

```yaml
blocked_run_continuation:
  schema_version: blocked_run_continuation_profile_v1
  profile: implementation_restart_v1
  review_state: state_4_proposal_reviewed
  approval_state: state_6_implementation_approval
  preparation_state: state_7_implementation_started
  proposal_input: proposal_current
  preparation_task: freeze_approved_proposal_and_prepare_worktree
```

The compiler verifies actual proposal routing/review, success through the required
manual gate, rejection/refinement through review, and gate dominance over every
write-enabled implementation/release task. Reject ambiguous topology, writable
review agents, dynamic bypasses and incompatible required artifact contracts.
Compile all target states and delivery/release preflight, not just the first task.

Freeze `shared_implementation_worktree` read strategy for continuation review and
refinement tasks in the compiled plan, including dynamically selected reviewers.
Their source context, read-path expansion, cwd and Xcode binding must resolve to
the successor checkout, not the repository/main checkout. They may write only
their new-run metadata outputs, not product code. Current definition/skill loading
is a separate trusted input. Persist this derived root mapping in the snapshot;
do not reconstruct it from an old task payload at dispatch.

Activation enters the declared review state and queues one AdvanceRun. There are
no fake successful earlier stages: the entry record explicitly says
`entry_reason=verified_carry_forward`. Omitting drafting is authorized by the
closed compiled profile, not by a supplied current_state or old approvals.

Fresh review may refine the proposal before the ordinary new implementation gate.
Its approval payload binds manifest digest, source/new IDs, proposal hash, code
snapshot digest, target workflow/catalog hashes, capability delta and unresolved
findings. Grant binds that tuple; changed material makes it non-actionable and
requires new review/approval. Existing `granted`/`rejected` vocabulary remains.
The runtime child defines the durable approval binding and supersession record;
an old grant for the same logical state is not sufficient.

After grant, ordinary preparation produces fresh approved proposal, plan and
backlog using the reference bundle. Existing code is preserved; previously
implemented items require new proof, not copied completed-task flags. No active
approved-proposal output is imported before preparation, so this path does not
need the P095 legacy duplicate-output workaround.

The checkout starts at the source HEAD, not silently at current main. Current
runtime/catalog and source revision are separate dimensions. Record both; no
automatic main-sync during preparation. Later sync remains workflow-owned.

## 7. Operator And Trust Boundary

Commands are MCP-only; no new GraphQL mutation or UI-controlled workflow. Existing
UI reads lineage, phase, verification/holds and ordinary approval via GraphQL.
Detailed manifests are privileged and paginated. Restricted/wrong-run callers
receive no private paths, contents, approval comments or hidden operation result.
Operator class alone is insufficient without the exact capability.

Target headless policy must be explicit for all required tasks. Compiler/policy
preflight does not open Apple workspaces. Preparation/activation call neither
Xcode nor providers. Later review/implementation can need native consent and must
hold at that boundary. Project trust is not cloned from the old checkout; missing
trust is visible before dispatch. No IDE fallback or unsafe allow-all.

## 8. Hypothesis And Acceptance

**H1:** two disposable repositories demonstrate that a blocked implementation
source with staged/unstaged/untracked work and an obsolete catalog produces one
independent successor with current definitions; source hashes remain unchanged;
fresh review and human approval precede implementation.

First proof is provider-free Rust integration against isolated temporary DBs and
repositories. No real P095, production DB, Apple service, provider or remote host.
The [verification contract](039/verification-and-rollout.md) owns the full matrix.

Minimum implementation acceptance:

- deterministic bounded preview and verified independent preservation;
- exactly-one activation and same-idea execution ownership;
- source guards across every command/dispatch/recovery/settlement path;
- new compiler/permissions/skills with no inherited execution authority;
- fresh review/approval and explicit artifact input origin;
- unknown effects hold both source and candidate;
- auth, stale-projection behavior and four-lane readback parity;
- additive migration, compatible rollback and no implicit cleanup;
- provider-free gate plus separately authorized P095 live acceptance.

A specification update/review is not implementation, deployment, Apple consent,
P095 implementation approval or authorization to delete the source.

## 9. Readiness

Five independent specialists reviewed the candidate; targeted rechecks closed
all their findings at specification level. See the
[readiness review](039-blocked-run-fork-and-canonical-carry-forward.review/proposal-readiness-review.md)
for dispositions, final fingerprints and the reviewer-cap coverage gap.

Proceed with an I1 task-level plan, then I2/I3 integration against its measured
results. I4 remains gated on offline implementation proof, compatible deployment
and exact live scope. The separate execution-truth review omitted by the
five-reviewer cap is required before production admission. This does not block
the provider-free first experiment. None of these gates is claimed completed.
