# Headless Offline Implementation Checkpoint

2026-09-19, isolated worktree at source HEAD
`a096b8831f0db7f17d1ffe426851c49a65b8b000`; all changes are uncommitted.
The user requested implementation while unavailable and deferred interactive
testing. No Apple, provider, IDE, permission, service restart, live database,
deployment, commit or push operation is part of this checkpoint.

## Implemented Boundary

The [I1 closeout](xcode-headless-i1-live-2026-09-19.md) supports ordinary routing:
two disposable projects, six explicit reads and one bridge replacement with
the IDE absent. Its historical source identity is separate from this newer
code. Earlier uncertain opens remain retained, without replay or cleanup.

The [offline dependency plan](../superpowers/plans/2026-09-19-xcode-headless-offline-foundations.md)
does not enable production headless dispatch:

- Shared root selection is wired into engine policy/MCP context, provider
  process/session setup, Junie preflight, Claude transcript lookup and shim
  runtime scope. Read-only shared/dedicated worktree tasks no longer disagree
  across those consumers. Missing explicitly required worktrees stop before
  provider admission; legacy default fallback is preserved.
- A synthetic provider exercises actual process/session/shim roots through a
  successful prompt, live reuse, close, resurrection and another prompt. This
  verifies the existing manager path without contacting Apple or a real provider.
- Filesystem-pinned `ResolvedExecutionRoot` resolution/revalidation is tested
  independently. It is not yet a committed manager ticket, prepared binding or
  fingerprint input. Canonical-path/identity checks are not a filesystem sandbox.
- Standalone durable journal is implemented: prepare dedupe, dispatch CAS and
  hold in one committed transaction, terminal/unknown outcomes, restart recovery,
  evidence-bearing reconciliation, and historical schema identity. Holds block
  both canonical path replacement and device/inode aliases. Mutations consume
  existing writer-admitted transactions; no raw production writer bypass was
  added. No broker/shim effect is connected to this repository yet, and its
  evidence/digest inputs require validation by the future trusted adapter.

## Verification

| Check | Observed result |
| --- | --- |
| Root selection RED/GREEN | Missing API; then explicit read-only process cwd mismatch; then passing tests |
| Engine MCP context RED/GREEN | Replacing engine root failed to change the old context hash; corrected test passes |
| New/reuse/resurrection RED/GREEN | Missing required worktree initially reached downstream lookup; now rejected first |
| Final ACP library/integration/example suite | 356 passed, 1 existing manual latency test ignored; live-smoke variable unset |
| Domain complete suite | 341 passed, zero failed |
| Engine MCP module | 11 passed |
| Explicit shim worktree observation | 1 passed |
| Standalone journal | 26 passed in implementation and parent rerun |
| Full DB library suite | 441 passed, 2 failed; both failures reproduced on clean HEAD |
| End-to-end root/reuse/resurrection fixture | 1 passed; no real provider or Apple process |
| Engine complete unit suite | 568 passed, 4 failed |
| Clean exact-HEAD engine baseline | Same 568 passes and same 4 failures |
| Final engine in isolated managed cache | Same 568 passes and same 4 baseline failures |
| Managed workspace check | Passed; existing MCP report dead-code warnings retained |
| Workspace formatting | Passed |

[Independent offline-patch review](xcode-headless-offline-review-2026-09-19.md)
found no actionable blocker in these components. The reviewer did not certify
production integration or rerun the tests. Its suggested integration-coverage
improvement was subsequently added and passed in the parent run.
No merge or full-production-readiness claim is made.

The four engine failures were reproduced in a clean archive of the exact source
HEAD, not by reverting the working tree or skipping tests:

1. `proposal_053_settlement_boundary_records_idempotency_key`: expected materialized file absent.
2. `provider_quota_runtime_receipt_preserves_explicit_claude_reset_time`: current-date-derived reset differs from the fixed August expectation.
3. `targeted_p058_retry_refreshes_stale_tier_from_the_durable_ledger`: fixture idea ID is not a UUID.
4. `persisted_dynamic_health_fallback_payload_uses_frozen_target_profile_authority`: fixture model disagrees with frozen authority.

The two DB failures also reproduce on clean HEAD:

1. `p091_claim_next_quarantines_malformed_typed_advance_and_claims_next`: fixture unwraps a missing value.
2. `p091_claim_next_quarantines_source_work_item_only_retry_advance`: fixture unwraps a missing value.

After baseline compilation, a worktree compile in the default shared target
reported missing new domain exports despite their presence in source. The
final engine suite and workspace check succeeded in compiling the current code
in a separate managed gate cache with sccache disabled. No source workaround or
shared-cache deletion was used; the earlier compile failure remains recorded.
An ACP diagnostic run with `CHAINWORKS_CARGO_SCCACHE=0` then failed two existing
tests that expect enabled cache injection (198 passed, 2 failed; later suites
did not run). The final complete ACP run in the isolated target used normal
managed-cache settings and passed all 356 selected tests. The environment-caused
failure was not hidden or addressed by a source change.

Logs are retained under `.superpowers/sdd/2026-09-19-xcode-headless-offline/`.
Two initial mistyped test filters selected zero tests; they are not counted as
verification. Their exact tests were subsequently run and passed.
The [32-file source manifest](xcode-headless-offline-source-2026-09-19.json)
pins all changed/new implementation and fixture inputs relative to HEAD. It
includes historical I1 additions plus newer offline foundations. It is evidence,
not a production authority digest, a commit or a release receipt.
The [verification record](xcode-headless-offline-verification-2026-09-19.json)
contains result counts and hashes of the retained local logs, including failures.
Final static validation matched all 32 source files, 13 log hashes and six
current document pins; 82 local links in 19 headless Markdown documents resolve.
All five active specifications remain below 2,000 lines. `git diff --check`
passes. The original checkout's unrelated changes were not modified or reverted.

## Not A Full Migration

The installed daemon and old IDE-dependent production implementation have not
been replaced. Remaining work includes the production headless controller,
shared broker/shim coordinator, per-UID/DB authority, preparation before session
policy/reuse, committed tickets, admitted closed facade, privileged readback,
legacy-daemon cutover, packaged consent and release receipt.

The strict production contract also requires upstream non-reassignable/atomic
workspace identity and execution-time filesystem confinement. The investigated
schemas/docs and successful I1 do not establish them. Offline implementation
cannot manufacture those guarantees. Before production admission, the project
needs either evidence for that enforcement boundary or a reviewed change to a
trusted-workspace model. Neither decision is silently made while the operator
is unavailable. A live test tomorrow is not guaranteed to reduce this to a minor
fix, and denying every required capability would not count as replacement.
