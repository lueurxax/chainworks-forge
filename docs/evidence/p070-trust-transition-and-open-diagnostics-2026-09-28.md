# P070 project-trust transition and headless-open diagnostics

## Scope and source identity

Branch: `codex/p070-trust-transition-and-open-diagnostics`, based on
`90e6616f`. The operator requested fixes and committed evidence, then explicitly
authorized merge to `main`, push to `origin` and daemon update. Project-trust
grants, live retries and hold clearance remain outside this rollout.
The original dirty checkout and all run-owned worktrees are preserved.

The originating read-only triage is
`/Users/user/Documents/Chainworks Forge/docs/evidence/blocked-runs-triage-2026-09-28.md`.
This patch addresses two distinct failures:

| Run | Failed execution | Immediate boundary |
| --- | --- | --- |
| P070 B `2478fbf5-3eb8-4e9d-b805-8b463edf5345` | `dcc27bd5-ee91-48e3-b81e-e91ba0760c8a` | `security_checker` project trust before launch |
| P070 D `4520fcf0-373b-4a4a-9654-9aefffea8630` | `147228ca-919c-4f22-b7d7-3bbbb6bd323b` | `security_checker` project trust before launch |
| P070 A `b0e74a3e-877e-4b6a-8b46-d0c0b587d094` | `1fbd077d-dbeb-4350-9558-16de2222476c` | workspace-open outcome unknown |
| P070 C `0d3908d4-b551-47f3-bbb4-eb881ffebac4` | `4bcdbdd5-58f4-4599-9a2e-d33d23b87e5f` | workspace-open outcome unknown |

## Trust contract decision

The immutable `trusted_local_project_v1` policy admits an exact pinned execution
root, project and UID. It separately requires current evaluated invocation
permissions. `ResolvedExecutionRoot` is a filesystem observation, not a permission
grant. `dedicated` and `shared_implementation_worktree` select the same persisted
worktree for writer and implementation reader respectively; frozen mission
validation still owns the strategy and evaluated capabilities.

The store had included the entire root, including routing strategy, in its record
key and comparison. The B/D writer grant and reviewer request match repository,
effective worktree, project, UID, device and inode; only strategy differs. This is
an identity-model defect, not an intentional new permission granted to reviewers.
The correction is limited to this exact two-strategy worktree pair. It does not
change frozen mission validation, permission evaluation or Xcode tool admission.

The v2 store key removes only the strategy from this finite pair. Legacy lookup
checks both exact v1 aliases under one lock and returns the original digest
without modifying files. Any revoked or corrupt alias blocks; two active grants
are ambiguous. Existing v2 is authoritative and never falls back to legacy.
An explicit upgraded revoke updates every existing legacy slot before publishing
the v2 tombstone. Runtime and administration must be upgraded together; old
administrators cannot update v2 authority.

## Historical unknown effects

P070 A effect `9b47616f-28d5-4219-8049-110d96ecd34f` and P070 C effect
`4c040f90-73c7-4b78-85e9-f24f5ed044d3` retain unknown outcomes and project holds.
The existing catch-all error branch discarded the phase and original diagnostic.
An outer `headless_open_outcome_unknown` message, `transport_lost` journal reason
and elapsed time alone cannot distinguish startup, inspection, connection,
initialization, opening, response decoding or mapping failure. Late Apple consent
does not prove either historical operation's outcome.

Read-only evidence at `2026-09-28T17:44:08Z` confirmed both effects at revision 2
with no outcome, settlement proof or reconciliation. The live DB was identified
from PID 20634's open descriptors, then opened with `mode=ro` and `query_only`.
Neither execution retained a provider receipt, transcript or lower-level phase.
Daemon logs contain the same outer error at `2026-09-27T15:49:11.017069Z` (A)
and `2026-09-22T05:30:14.223225Z` (C). Bounded system-log windows did not prove
the cause. The private evidence summary is
`/private/tmp/cw-sep28-headless-evidence.json`.

New errors preserve a typed phase, reason, allowlisted source code, optional
typed I/O kind/errno and dispatch attempt ID. Unknown settlement errors retain
that safe context too. No raw upstream error chain is persisted. The executor
stores the typed fields in its existing operator-only preflight JSON and the
recovery snapshot requires evidence-backed reconciliation before explicit retry.
If the caller disappears before error delivery, the existing detached journal
settlement still protects the project but cannot deliver execution diagnostics.

## Verification

- Trust regressions first reproduced seven expected behavioral failures;
  the complete trust-store suite then passed 23 tests.
- Phase regressions first reproduced discarded phase evidence, including a
  failed unknown settlement; the preparation suite then passed 34 tests.
- The executor integration test first reproduced the generic
  `headless_preparation_failed` instead of the typed unknown outcome. All three
  admission integration tests then passed, including persisted phase/effect ID,
  no raw secret, no provider launch/retry, retained hold, reconciliation-first
  recovery and writer-grant/read-only-reviewer admission.
- Independent read-only review found no blocking trust or diagnostics regression.
  A secondary review identified typed held-error aliases; the final verification
  also covers those through error context.
- `xcode-headless-project-trust`: exit 0, 68 tests passed across trust store,
  executor admission, preparation, prelaunch recovery and GraphQL/MCP readback.
  Final typed-hold coverage is verified by an additional focused prelaunch run:
  7 passed, zero failed (69 distinct tests across these selections).
- Changed Rust files pass rustfmt, shell syntax and `git diff --check`.
  No full-workspace or live Apple-operation acceptance is claimed.

| Private test log | SHA-256 |
| --- | --- |
| `/private/tmp/cw-sep28-project-trust-gate.log` | `e1362ea8f67eee6967e343ee3f546f57c3f95aee38507dea1fca6ebc1fce4f3e` |
| `/private/tmp/cw-sep28-admission-red.log` | `ecfbe62b6694b5d0c659ab1c07ab5b9fe2e35f8fd7cc764f69e80b5bf57adeba` |
| `/private/tmp/cw-sep28-typed-holds-red.log` | `7c71533d141ff0b2ddc3103dcbb690e10e3c227784e8196954b7c0b4262dfd7a` |
| `/private/tmp/cw-sep28-prelaunch-final.log` | `4ac4675db8f8ec06c87e5c9f6dbdc89907ffa44cc4b6060466ac475f697302cc` |

## Authorized rollout preflight

The installed app-owned daemon was PID 20634, build `dc4ea294`, schema 102.
`/health` and `/ready` returned 200; Xcode broker leases and backend sessions
were zero. MCP storage health was fresh/HEALTHY with an idle writer queue.
The existing `run_summaries` projection poison remains a separate condition.
`runtime.health` was denied by both existing principals with
`CAPABILITY_OUT_OF_SCOPE`; no authorization was changed to work around it.

At `2026-09-28T17:55:20Z`, supplemental read-only DB diagnostics confirmed zero
pending/running work items or agent executions. Two retained Codex ACP processes
belonged to completed reusable sessions, not executing stages. MCP reported one
run waiting for approval and eight blocked runs. No approval or retry is part of
this update.

The old bundle is preserved under
`/private/tmp/cw-p070-sep28-deployment/previous-Chainworks Forge.app`; its strict
signature verification passed. A SQLite backup via the backup API passed full
`integrity_check` at schema 102, SHA-256
`65e607c773caee93f61ba55975175676d04bb9275f4dab3a7830d509cf965137`.
The private preservation snapshot captures hashes of all seven trust files and
the exact A/C effect/hold rows. The staged bundle and post-install receipts live
in `/private/tmp/cw-p070-sep28-deployment/`; deployment must verify these records
and the exact compiled source SHA before claiming success.

## Deployment and recovery order

1. Integration, push and daemon update are authorized. Check current active run
   and invocation state before restarting the daemon.
2. Install a build containing both fixes and verify build SHA, readiness, MCP and
   storage readback. Use matching upgraded runtime and trust-administration code.
3. Re-read B/D current physical identities and trust evidence. A valid compatible
   legacy grant can cover the same writer/reviewer worktree; revoked, ambiguous,
   damaged or changed identities need a separate explicit operator decision.
4. For A/C, inspect each exact effect through the scoped operator diagnostics.
   Reconcile only with proof accepted by the existing effect contract. This patch
   does not establish such proof or permit replay of an unknown effect.
5. After any necessary evidence-backed reconciliation and explicit authorization,
   retry one eligible failed stage and read back actual admission/provider
   progress. New phase diagnostics improve future failure evidence; they do not
   reconstruct missing historical facts.
