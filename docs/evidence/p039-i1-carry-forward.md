# P039 I1 Carry-Forward Core

Date: 2026-09-20. Baseline: `8c40f71a03cfebff413fbad584131e0bcbe9859b`.
Scope: provider-free core in an isolated worktree, not production continuation.

## Hypothesis Result

**Partial.** The current compiler, workspace preservation and fresh approval
contracts compose successfully against disposable repositories and a temporary
SQLite database. The source run, old approval, catalog and worktree contents stay
unchanged; no successor Run or queue item is fabricated by the experiment.

The full H1 claim includes durable successor activation and observed fresh
review/approval before implementation. Those remain unproven until I2/I3 wire the
core into canonical storage, dispatch and approval ownership. A local checksum
or a domain approval unit test is not that runtime proof.

## Implemented Core

- `workflow::carry_forward` compiles explicit current definitions, validates a
  closed review/approval/preparation profile and gate dominance, rejects product
  write authority before approval, and freezes successor read roots for static
  and dynamic reviewers. No target profile is enabled in bundled live workflows.
- `domain::run_carry_forward` defines strict content digests, input roles and
  approval evidence bound to exact source/successor, stage execution and content.
  It does not change existing production approval predicates.
- `engine::run_carry_forward` inventories tracked and nonignored untracked
  material, captures HEAD/index/staged/unstaged evidence, streams file hashes,
  rejects unsafe paths/active filters, and creates a separate pinned checkout
  plus private preservation. Existing or partial destinations are never retried
  or overwritten. No production caller is registered.
- `proposal-039-i1|p039-i1` is a provider-free Rust proof gate, not the future full
  `proposal-039|p039` gate.

## Observed Corrections

1. The current proposal writer sets `write_enabled=true` for `meta_only`, not
   product code. The continuation compiler validates those metadata permissions
   and derives a shared read-only source strategy rather than rejecting valid
   proposal refinement or copying the old write flag.
2. The first graph check allowed a refinement-to-approval shortcut after rejection.
   A failing regression now requires review to dominate every return to the gate.
3. Tokio stdin shutdown alone did not close the Git input pipe. Explicit ownership
   drop fixed the `check-attr` EOF wait; the filter-negative test now checks its
   actual denial reason so a timeout cannot masquerade as a successful guard.
4. The real new-run compiler normalized unknown workflow fields away. The SQLite
   composition test caught the missing profile after snapshot-only tests passed.
   The optional metadata is now preserved in `WorkflowFile`, omitted when absent
   so ordinary historical/default workflow hashes are unchanged.
5. Independent review found eight issues. Regression fixes reject symlink chains,
   partial clones, split indexes and sparse/assume-unchanged entries; inventory
   includes HEAD-only staged deletions and rename origins, including file/directory
   replacements. Destination directories are descriptor-relative, held identities
   are checked around phases, and common Git metadata cannot be a destination.
6. Review success/refinement predicates are now exact closed-profile conditions.
   Aggregate size is checked before each file hash. Preview (120 s) and preparation
   (15 min) deadlines include Git, hashing, copying, verification and receipt
   publication. A deterministic expiry test covers the post-fsync/pre-linkat boundary.

## Proof Boundary

The local materializer is not a production-safe dispatch entry. I2 must add the
durable effect intent, source/idea fence, bounded worker lane, generation-safe
abort, reconciliation and atomic activation before wiring it to a live command.
I3 must connect installed input origins, exact-stage approval binding, capability
checks and directional readback. The current domain binding is not a DB grant.

External same-UID writers must be quiescent over source, operation-owned destination
and their ancestors throughout I1. A trusted caller alone does not establish this.
Git remains pathname-based: identity checks around a call are not atomic containment
against an active concurrent path replacement during Git. Before production admission,
I2 must enforce the relevant ownership/quiescence or strengthen Git effect containment;
the static review did not waive this requirement. The core has no production caller.

Deadlines are cooperative between local syscalls; cancellation of stuck OS I/O is
not claimed. I2 owns blocking-worker isolation. A failure after linkat can leave a
receipt on disk; neither its presence nor a timeout authorizes retry or activation.

I1 additionally uses a bounded 16 MiB Git/index capture buffer and supports only
file-level untracked machine exclusions. Symlink chains, sparse/hidden indexes,
split indexes and partial clones are explicit unsupported holds, not hydrated or
silently flattened. Tracked machine exclusions, whole-tree
exclusion policy, full provenance selection/pagination and complete CF-01..29
coverage remain integration work; they are not claimed by the I1 gate.

No production DB, P095 worktree, provider, Apple service or Xcode IDE was used.
No migration, live API registration, source cancellation, merge, push or daemon
update is part of this evidence.

## Validation

Initial I1 gate: 24 passed. Post-review gate: 37 passed (4 domain,
11 workflow, 4 engine unit, 12 inventory, 5 materialization,
1 SQLite composition), zero failures. The materialization positive test exercises
two independent disposable repositories, staged and unstaged deletions, a staged
rename, binary edits, modes, untracked inputs and a relative symlink.

Independent code review: all eight findings statically closed after fixes; the
destination finding is closed only under the quiescent-writer assumption above.
No live acceptance or complete H1 verdict was inferred from review.

Full domain regression passed. Domain/workflow combined regression: 559 passed, one failure:
`implementation_agents_consume_control_plane_git_evidence_without_direct_git_tools`;
the identical prompt assertion was reproduced in an unchanged HEAD archive.
Likewise, the auth `p083_bootstrap_uses_approval_only_and_full_set_still_loads` and
two `p091_claim_next_quarantines_*` DB failures reproduced at the unchanged baseline.
ACP cache-environment tests passed when rerun with the ordinary managed sccache
environment; the broad diagnostic run disabled it. Five DB metric tests passed
on a serial baseline rerun. These reruns do not turn the broad suite green.
The broad diagnostic workspace run completed with exit 101 and nine failing targets:
`acp --lib`, `auth --lib`, `db --lib`, `engine --lib`, `engine --test integration`,
`engine --test proposal_041_parity`, `engine --test release`, `mcp-server --lib`,
and `workflow --test integration`. It ran during implementation with sccache disabled,
not as an immutable final-tree sign-off. Remaining engine/MCP/parity/release failures
were not fully classified here; they are not asserted to be harmless or all pre-existing.
There is no green workspace or merge-readiness claim.

Both final focused aliases completed with exit 0 after the review fixes. Scoped
Rust formatting, `git diff --check`, shell syntax and gate-list registration passed.

Local command logs (not committed, no live payloads):
- `/private/tmp/p039-i1-final-gate.log`, `/private/tmp/p039-i1-final-alias-gate.log`
- `/private/tmp/p039-domain-workflow-regression.log`
- `/private/tmp/p039-workspace-isolated.log`
- `/private/tmp/p039-baseline-auth.log`, `/private/tmp/p039-baseline-db.log`,
  `/private/tmp/p039-baseline-workflow.log`, `/private/tmp/p039-baseline-metrics.log`
- `/private/tmp/p039-acp-env.log`, `/private/tmp/p039-acp-fingerprint.log`
