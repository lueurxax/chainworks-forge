# Carry-Forward Integration Evidence

Status: **candidate-3 offline readiness independently accepted**. The final
canonical gate passed, all broad failures are classified and no scoped P039
finding remains open. This is not live admission,
a P095 migration receipt, or permission to retire the proposal.

Baseline: `8c40f71a03cfebff413fbad584131e0bcbe9859b`. At this offline cutoff,
changes were isolated and uncommitted in the `p039-carry-forward` worktree.
Subsequent [live rollout evidence](p039-live-rollout-2026-09-21.md) records the
published/deployed merge and P095 preview hold; it does not retroactively make
this an acceptance receipt for a completed transfer. Earlier results retain their cutoffs;
the final combined gate is recorded separately below.

## Final Candidate-3 Gate

The final clean-environment `proposal-039` gate completed with **exit 0**:

- **348 selected Rust tests passed**, zero failed/ignored, across 37 selections.
- Coverage-checker unit tests: **3/3**; rollout contract lint: **PASS**.
- Executed coverage: **29 cases, 164 distinct required exact test names**.
- All **1003 frozen file checksums** matched after execution.
- Workspace/all-targets compilation passed in **36.12s**, with existing warnings.
- Current Rust formatting and tracked diff whitespace checks passed.

The introduced producer-inventory regression also passed an independent exact
rerun. The current checker rejects the old candidate-2 log because it lacks that
new required event; the older green result cannot satisfy the expanded gate.

| Artifact | SHA-256 |
| --- | --- |
| Candidate-3 manifest, `/private/tmp/p039-final-tree-candidate-3.sha256` | `02cd077edeb7300b5ec856b728a4880b71fbd55aa2b9bf5df860ed82ab1a1d08` |
| Final gate, `/private/tmp/p039-final-gate-attempt-3.log` | `c023f6b428c07a8af7bbf7e599426c2da43c2ff6ab7522e82a9208385c34fe02` |
| All-targets check, `/private/tmp/p039-final-all-targets-check.log` | `3ae8cb27d1e94e30b70233a7b86df343c7f9c6698ddada197d6f0c3e83c0e499` |
| `scripts/test-gate.sh` | `0747e5651c964cecc9865536ad20872d3494b5de160a0183bbfb3a6f14f5b6a8` |
| `scripts/p039-coverage.json` | `d3c08c938536f2334dbf41d17906dd9ba075d4bb63516d943dfb4fc34e205bc4` |

These are scoped offline results, not a green full-workspace run or live P095
acceptance. The complete broad failure accounting and excluded lanes are below.
Earlier focused-check and review paragraphs retain their historical cutoffs;
their pending wording is not the current status of candidate 3.

## Historical Candidate-2 Gate

On 2026-09-21, `CHAINWORKS_AUTO_CACHE_CLEANUP=0 ./scripts/test-gate.sh proposal-039`
completed with **exit 0** on the candidate-2 snapshot:

- **347 selected Rust tests passed**, zero failed/ignored, across 36 selections.
- Coverage-checker unit tests: **3/3**.
- Executed coverage: **29 cases, 163 distinct mandatory exact test names**.
- Rollout contract lint: **PASS**.
- All **1003** source/test/gate/example/rollout/spec file checksums matched before
  and after the run; no in-run source changes. This is not a full workspace pass.

| Artifact | SHA-256 |
| --- | --- |
| Candidate-2 file manifest, `/private/tmp/p039-final-tree.sha256` | `88fd09d257b7fa28b2ec23983bdea4f3f5fcf5b31eeebed3b264bb51dd97f0f5` |
| Complete run log, `/private/tmp/p039-final-gate-attempt-2.log` | `52fc276be7f66ebe7b05c3dfc09fbeeddee6a20bf0507636df346dc76d1a1fbd` |
| `scripts/test-gate.sh` | `d80c80bb715a0ab1a671de6f682c4de51b8e1db742c9c815e46ef5ec01257177` |
| `scripts/p039-coverage.json` | `b3de7f99937f74328c67b439433a069076dcca29310c6bc14b6697436c28b730` |

The audit/evidence documents are updated after measurement and are not part of
that source manifest. The failed first attempt and its fixture correction remain
recorded below. Independent final synthesis and a clean-environment broader
workspace regression run are separate, not inferred from this successful gate.

## Historical Focused Checks

| Check | Result | Scope |
| --- | --- | --- |
| `db/proposal_039_atomicity` | 5 passed | Checked activation rolls back six durable boundaries; lost-response replay after DB reopen creates no extra run/queue/input; two operators produce one reservation; verified reconciliation and projection-failure recovery preserve canonical authority |
| `engine/proposal_039_fences` | 5 passed | Fourteen command variants, early StartRun, orchestration and actual startup/P092 recovery keep the source fenced |
| `db/proposal_039_inputs` | 1 passed | Distinct historical findings may share a logical name, but retain unique IDs and physical paths; no fabricated outputs |
| `engine/proposal_039_service` | 5 passed | Disabled admission, permanent denial/replay, reauthorization, typed missing-material hold, abort without implicit resume |
| `db/proposal_039_metrics` | 1 passed | Closed labels, bounded samples and no private sentinel in diagnostic values |
| `engine --lib p039_` | 4 passed | Pinned read-only root, all-provider source fence, output predicate exclusion, actual report writer lineage |
| `workflow/proposal_039_profile` | 13 passed | Closed target profile, current workflow opt-in and frozen permission/topology checks |
| `scripts/test-p039-coverage.py` | 3 passed | Missing cases, ignored/failed/filter-only tests and duplicate events fail coverage verification |

Subsequent focused checks (not yet a single frozen-tree gate):

- `engine/proposal_039_runtime_inputs`: 7 passed. The real declared-output
  producer writes checksum, size and execution ownership; the reader prefers
  that new output over the installed seed. Plain outputs retain
  `NoContractDeclared`, not fabricated schema-validation success. Their reader
  checks the exact frozen stage/agent/provider/output and completed settlement.
  Failed attempts, failure facts, wrong frozen provenance and tampered bytes do
  not acquire authority. The fixture does not repair produced artifact metadata.
- `engine --lib p039_declared_output_digest_streams_regular_bytes_and_rejects_links`:
  1 passed for streaming a multi-buffer file, empty bytes, and rejecting links or
  directories. Metadata stores the existing plain hexadecimal SHA-256 format.
- Existing `p082_r17_cancelled_late_output_import_path_quarantines_without_active_mutation`:
  1 passed with the updated production importer.
- `projection_failure_after_activation_keeps_canonical_links_and_rebuild_is_not_dispatch`:
  1 passed. An injected projection failure leaves committed canonical lineage
  intact; rebuilding and replaying create neither another successor nor dispatch.
- Approval sidecar: 11 DB binding tests, 10 ordinary runtime composition tests
  and 4 compatibility tests passed. Independent review of consumption ordering
  and the bounded rollout-preflight exception remains in progress.

Review-fix handbacks, still preceding the final integrated gate:

- Complete parent runtime-input target: **9 passed, 0 failed, 0 ignored**. Adds
  the full mixed-review task/gate proof and ordinary provider fallback through
  real enqueue, claim and output import. The alternate provider must match its
  exact persisted fallback decision, allowed frozen backend, catalog hash and
  failure lineage. Missing or altered decision/backend/claim evidence holds.
  Independent code/regression inspection accepts F01/F02; it did not rerun
  these tests or accept historical preview or approval ordering.
- API sidecar: **8 MCP unit + 23 MCP integration; 6 engine unit + 4 selected
  readback integration passed**. Independent source/regression recheck accepts
  API-039-01/02/03 within that scope. Historical-declaration resolution remains
  separate from that verdict.
- Reliability sidecar: **87 passed** across its library, service, materializer,
  worker, manifest, finalizer and storage targets. Successor-index binding,
  pre-reservation capacity ownership and settled-abort reconciliation were each
  RED/GREEN. Independent recheck accepts R1/R2/R3 after **43 focused tests
  passed, zero ignored**, with all 19 scoped hashes unchanged.
- Backup relative-filename regression: **7 integration + 17 migration unit
  passed** after reproducing the empty-parent directory-sync error. Independent
  source/regression inspection accepts the bounded fix with unchanged hashes;
  it did not rerun tests. No production database was used.

The headless sidecar separately executed four tests against a real carried
checkout, trust store, effect journal and headless preparation/controller, with
fixture peer/host I/O. Missing successor trust and injected native-consent denial
send no open. Exact successor trust permits reading its carried bytes; source
and main roots cannot substitute. This is not a real Apple consent or provider
execution observation.

## Regression Accounting

The existing `test_post_approval_tasks_enqueued_after_approval` passed on the
baseline and initially failed on this branch. An unconditional prompt-time
worktree requirement had changed the legacy manual-release path. Narrowing the
new prompt check to continuation context and explicit read-only shared-worktree
review restores the exact test; the current and baseline runs both pass. Actual
provider-launch root validation is unchanged. This regression was fixed, not
classified as an unrelated baseline failure.

Historical broad-suite failures are not blanket waivers. The final affected
workspace run must distinguish reproduced baseline failures from new regressions.

The subsequent full `db --lib` run completed with **443 passed / 3 failed**.
The two `p091_claim_next_quarantines_*` failures are also present in the untouched
baseline archive (`/private/tmp/p039-baseline-db.log`); its work-items source was
compared byte-for-byte with the stated baseline commit. The existing
`proposal_058_required_metric_names_are_declared` assertion failed in the parallel
suite but passed in isolation. Running the baseline's full metrics test group
also reproduced that exact assertion, alongside two other shared-metrics test
failures. The baseline metrics source likewise matches the commit byte-for-byte.
These are recorded baseline test failures, not a full-suite pass or a blanket
waiver for future failures. No shared metrics-test or P091 behavior was changed.

`production_mixed_review_outputs_feed_the_gate_without_aggregate_fact_repair`
was RED on a successful real mixed-output aggregate task. It now passes for all
seven frozen task outputs and an ordinary orchestrator transition to the approval
gate. Missing and invalid required sibling outputs both prevent the otherwise
valid summary from acquiring authority; removing its active contract generation
also fails closed. No aggregate runtime fact or produced artifact metadata is
rewritten by the fixture.

## Required Final Evidence

The first combined diagnostic `proposal-039` invocation exited 101 at the DB
fence target: domain 20, workflow 13, approval bindings 13, atomicity five and
checked reservation one passed; fences were 11 passed / four failed. Two failures
were fixture INSERT arity errors after the approved three-column approval change.
Explicit column lists corrected those fixtures without changing assertions;
the owner reran all three approval-filtered fence tests successfully. The other
two failures are the unresolved SQL ownership fences. This invocation did not
reach engine, API, daemon or final coverage checks and was not a frozen-tree signoff.
The subsequent complete fence target finished with **13 passed / two failed**,
zero ignored or filtered, in 136.35s. Only
`reservation_must_also_deny_late_owner_dispatch` and
`source_owner_mutations_check_old_and_new_and_preserve_history` remain failing.
On 2026-09-21 the user explicitly approved implementing the previously rejected
SQL expansion only in the isolated worktree and disposable test databases, with
independent review. Its owner resumed implementation; this is not yet a new
passing result or production authorization.

After E03 reused the existing dynamic-review name/schema/path helpers, the full
historical preview target passed **48/48, zero ignored or filtered**, in 89.97s.
Its independently scoped recheck also passed 48/48, zero ignored or filtered,
in 82.66s with unchanged scoped hashes. The owner also ran the approval
runtime subset with **10 passed and four explicitly filtered executor-dependent
cases**, not a full runtime pass. DB approval bindings passed 13/13 and operation
registry checks 14/14. The actual later-gate publication-failure/reopen regression
passed separately. No manually assigned started marker substitutes for the four
remaining actual executor/PromptSent proofs.

Ordinary approval composition now reaches the actual ACP fixture launch and
exposed a separate integration gap: operation-owned metadata is outside the
original repository workspace, whereas ACP's existing launch preflight requires
metadata within it. The implemented correction is an engine-validated,
non-serialized metadata-root capability; ordinary request confinement and the
execution cwd remain unchanged. On 2026-09-21 the six ACP boundary cases passed
as part of the full ACP unit target: **254 passed, zero failures/ignored/filtered**,
5.23s. The engine activated-manifest/owned-execution test passed **1/1**, 7.93s,
after correcting its fixture's missing mandatory stage label. All seven cases
are now required by CF-18. Independent review and full approval-runtime proof
remain required; these results do not prove a live provider launch.

The same day's `cargo-managed check --locked --offline --workspace --all-targets`
initially exposed one missing `DeliveryReceipt.carry_forward` initializer in an
existing release test. Adding the ordinary-run `Default::default()` field fixed
compilation; the complete all-target check then passed, with existing warnings.
The exact runtime test
`background_executor_preserves_existing_delivery_receipt_without_overwrite`
fails before receipt construction at its immediate queue-claim assertion, both
on this worktree (0.65s) and unchanged baseline archive (0.64s). Baseline release
test and work-items sources were byte-compared against HEAD. This is a reproduced
baseline runtime failure, not a passing release regression or waiver. The
receipt fixture correction changes no assertion or production queue behavior.
Coverage checker tests (3/3), shell syntax, rollout lint and tracked diff checks
also passed. A combined frozen-tree gate has not yet completed.

The independent metadata review found R5-META-01: the constructor re-opened
verified paths without comparing handles to the manifest-recorded device/inode.
The deterministic local stale-identity handoff test was RED before the check.
Mandatory expected checkout/metadata identities now come from the immutable
manifest; opened handles must match those identities and private permissions
before capability installation. All **7 ACP metadata tests passed** (0.03s), and
the engine launch test passed **1/1** (7.04s). CF-18 requires the added regression;
the independent remediation recheck also passed all eight cases with stable
scoped hashes and accepted R5-META-01. This corrects the earlier
six-case proof gap rather than treating the first green run as sufficient.

The complete ordinary approval-runtime target then passed **14/14**, zero
filtered or ignored, in 125.37s; DB bindings passed 13/13 and registry checks
14/14. Actual ACP fixture PromptSent and later-gate publication/reopen behavior
are exercised. R01/R02 and the exact activated-profile snapshot suppression were
independently accepted by source and assertion inspection, not a reviewer rerun.
The fixture no longer compensates for the production snapshot ownership gap.

The approved SQL expansion subsequently passed **21/21 fence tests**, 40 related
P039 DB tests and two selected P082 malformed-JSON tests. Those results supersede
the older 13/15 diagnostic result above, but independent composition review
found two remaining defects: F1 admission used narrower indirect/resource owners
than the fences, and F2 an unresolved pending queue head could prevent unrelated
claims. Both are local implementation issues, not live rollout prerequisites.

Parent F1 correction first reproduced accepted reservations for unresolved queue
owners and for an active other-run agent using the source generation directory.
The corrected writer-locked owner query passed **4/4 checked-admission tests**
(11.86s) and **13/13 storage tests** (18.47s), zero filtered or ignored. The new
matrix pairs seven source-owned indirect/resource cases with distinct-resource
positive controls and proves denied reservations leave no operation, fence,
journal or work-item rows. F1 independently reran all 17 tests and accepted the
correction with unchanged hashes. F2 added transaction-local FIFO progress past
exact source-fence denials and conservative unresolved-owner quarantine; its
12 tests passed in both owner and independent reviewer runs. Protected history
stays unchanged, unrelated due work progresses and other SQL errors roll back.
The integrated gate has not yet passed at this evidence cutoff.

Final bounded API/policy proof additions passed **24 MCP, three GraphQL and
three engine policy tests** in complete owner targets. Independent source and
assertion inspection accepts ET01-ET04, with unchanged scoped hashes; no reviewer
rerun is claimed. The HTTP test delegates actual service activation, loses one
post-commit acknowledgement, reopens the database/service and replays the same
request twice with one successor, one AdvanceRun and the identical receipt.
GraphQL queries after outgoing abort preserve incoming history. Neither proof
claims a real COMMIT I/O failure, process kill or provider execution.

The combined historical test installs a structured carried reference by real
activation and reads it after DB reopen and removal of its declaration from the
current catalog, retaining original ancestry/schema/bytes. Unauthenticated
legacy external skills remain an explicit unsupported-frontier hold; arbitrary
removed-original-tool compatibility is unproved. Missing explicit headless
permission returns the existing public `target_profile_invalid` denial, while
the underlying missing-headless cause is asserted. Preview preserves the full
canonical witness. Denied prepare adds exactly one durable audit receipt and
therefore changes that witness, but preserves source rows, files and index;
same-key replay changes neither the receipt nor the post-denial witness.

- Complete ordinary approval/input/snapshot composition and owner-fence checks.
- Run `./scripts/test-gate.sh proposal-039` against a stable integrated tree.
  `scripts/p039-coverage.json` requires exact executed successes for every offline
  CF row; the map itself does not prove semantic completeness.
- Independent immutable implementation/execution-truth review and fixes.
- Explicit compatible deployment/P095 canary scope, then live readback and fresh
  review/approval/headless acceptance as separate observations.
- Only after acceptance, promote stable contracts and retire proposal artifacts.

## Historical Gate Reconciliation

The first source-frozen combined gate attempt completed all DB targets and the
ordinary approval/finalizer/headless/launch/policy targets, then failed preview
with **44 passed / four failed**. Every failure was the same historical-fixture
UPDATE rejected as `source_continued`: its helper assigned ancestor and successor
the identical worktree resource. This is an invalid carry-forward fixture under
the accepted source-resource guard, not grounds to weaken that guard. The helper
now creates a real separate local detached checkout for the ancestor and also
asserts that changing the historical ancestor is still denied. All original
input/provenance assertions remain. Production code and SQL were unchanged.

The corrected full preview target passed **48/48**, zero filtered/ignored,
87.83s, and independent source/assertion review accepted the fixture correction.
The remaining engine diagnostic targets passed **38/38**: readback six, runtime
inputs nine, service five and worker 18. The second canonical gate subsequently
passed against the new 1003-file checksum manifest, as recorded above; the
preview test was the only changed file relative to attempt one. The separate
partial reruns are not substitutes for that full gate result.

The optional repository `guardrails` gate refused startup because the installed
Forge app was already running. It exited before checks; the app was not closed
and the guard was not bypassed. Source/diff/coverage/lint checks reported above
are distinct from that unexecuted gate.

At the earlier scoped-review cutoff, no integrated-feature verdict was claimed.
Historical plain
outputs, version-pinned finding readers, dynamic reviewer provenance, historical
approval eligibility and retained generations have scoped independent acceptance
after the 48-case rerun above. Approval review found premature consumption before the first
real dispatch and stale terminal bindings blocking publication of a later gate;
both corrections have scoped independent acceptance as recorded above. Bounded API,
reliability, fresh-output-reader and backup fixes were independently accepted as
described above. Candidate-2 canonical offline acceptance is now recorded, but
does not certify a later edited candidate, broad workspace success or I4.

The proposed expansion to 115 SQL triggers across 39 operational tables was
initially rejected by the execution approval reviewer because unverified ownership and
exception predicates could block reconciliation or ordinary writes. The patch
is now explicitly approved for implementation and tests only in this isolated
worktree, as recorded above. The expanded fence target now passes 21/21; F1/F2
composition corrections also have scoped independent acceptance. The combined
gate and whole-feature verdict remain separate. No production database permission
follows from the approval.

All executed parent checks use disposable repositories/databases and managed
Cargo. No production DB, daemon restart, provider, Apple service, Xcode IDE,
remote Git mutation, source cleanup, commit or push is represented by this file.

## Broad Regression Accounting

The completed candidate-2 workspace run used a clean private HOME/TMPDIR,
managed offline Cargo, four test threads and no cache cleanup. It excluded the
daemon package because actual daemon startup resolves the host-global headless
authority through the OS UID, independently of HOME. It completed with **4011
passed, 81 failed and three ignored** across 131 reported test selections, exit
101. This is not a green workspace result.

Daemon library and binary unit targets were then run separately after inspecting
their fixture isolation: **117 + 20 passed**, zero failed/ignored. Actual daemon
stdio/startup integration, installed providers, Apple and UI lanes remain
unexecuted. The guardrails refusal above also remains an explicit exclusion.

Independent baseline runs verify the unchanged archive against Git
`8c40f71a03cfebff413fbad584131e0bcbe9859b`, including 651 committed build,
control-plane and example files. They reproduce the ACP lease failure, auth
compatibility failure, four engine-library failures, all 48 engine integration
failures, three P041 parity failures and eight release failures. The 213 engine
integration test statuses and all 48 named failure signatures match exactly,
apart from generated identifiers and runtime paths. The DB P091 queue-due
failure also has an earlier exact baseline reproduction. These failures remain
red; reproducing them is not fixing or waiving them. The final 13 MCP-library and
one workflow-integration failures also reproduce on verified baseline source.
All shared named statuses and each named failure body match. Independent review
reproduced 79 failures; the earlier parent/Avicenna DB P091 proof accounts for
the remaining one. Thus **80 pre-existing failures and one introduced inventory
regression account for all 81**, with no unclassified failure in this comparison.

| Baseline failure target | Count |
| --- | ---: |
| ACP integration | 1 |
| Auth library | 1 |
| DB library, exact P091 queue-due case | 1 |
| Engine library | 4 |
| Engine integration | 48 |
| Engine P041 parity | 3 |
| Engine release | 8 |
| MCP library | 13 |
| Workflow integration | 1 |

The baseline/current MCP comparison covers all 377 shared tests; its eight
additional P039 tests pass. All 52 workflow integration statuses match. Baseline
source/dependency identity was expanded to 654 files and verified before and
after those runs. The final candidate fixes the introduced inventory regression;
the entire broad workspace was not rerun after its one-string fix and formatting.

One introduced regression was identified: the closed InvokeAgent producer
inventory still named `build_task_prompt(` after the dynamic producer moved to
`build_task_prompt_for_runtime(`. The correction changes only that manifest
string. The existing test retains its nine-producer inventory, preceding guard,
individual guard-removal negatives and unknown-producer negatives. The exact
test is now required by the P039 gate and CF-01 coverage map.

The baseline passes `cargo fmt --all -- --check`. Thirteen changed Rust files
needed formatting and were mechanically formatted; the current check now also
passes. Together with the inventory string and the gate/map addition, exactly
16 of the 1003 frozen files changed. No additional production behavior was
changed during this final regression correction.

Candidate 3 remained frozen throughout its successful canonical rerun. Recorded identities:

```text
02cd077edeb7300b5ec856b728a4880b71fbd55aa2b9bf5df860ed82ab1a1d08  /private/tmp/p039-final-tree-candidate-3.sha256
62bb2ccd635135388c5c48aceb84814de1cce0a228a82700cf5f543a407dbd20  /private/tmp/p039-workspace-regressions.log
bd1fe65dff3b8dcc06fd9ef62c5b0c751fa0fc12f9ecde2a86e68a8a57ba90e9  /private/tmp/p039-daemon-unit-regressions.log
c3f75cc4e6aaad4570172cbdaeb8aeef78e84940c401df28d30269c2879a7897  control-plane/crates/engine/tests/fixtures/agent_context/invoke_agent_producers.json
c7bfddc24a5fb1d9910199f4230821c9e914f611ae1685e4c8b8a2e7bd239c3f  /private/tmp/p039-baseline-db.log
2b0ada374ac5b0450fb92ad92ce6a864400be9b8aad23b7b862f0ab3106ab2a3  /private/tmp/p039-review-baseline-engine-combined.log
7fb39149dcd9de57405c4d7443bef0dea6f122413f9031988f839d2658c86a38  /private/tmp/p039-review-baseline-parity-release.log
33ac9b15f1f6abe7a4b6a9cb9b39baf522dd4487ce64e89dfdb7e221608e071b  /private/tmp/p039-review-baseline-mcp-lib.log
7b44c059dc171c3600a08b5e7b70de72aeb6beae37e059d415967b7d39db7e71  /private/tmp/p039-review-baseline-workflow-integration.log
69c054531ec500369a458da73c0fca77095b8ce7f67d87ee3da9b4e84c5cc352  /private/tmp/p039-review-candidate3-inventory.log
```

## Documentation Preparation

The implemented contract is consolidated in
[blocked-run-carry-forward.md](../reference/blocked-run-carry-forward.md).
README/index, Rust, execution-truth, MCP and gate references point to that owner;
the active P070 dependency is also rewired. Independent consistency review found
no substantive discrepancy in the new reference. All changed-document local
links pass. The global helper has the same 40 baseline findings outside this
change; three additional local ignored review-report locations use a file-line
link syntax the helper does not support. They are not new canonical-doc links.

This is documentation preparation, not proposal retirement. I4 permission was
requested separately and remained unanswered at this historical cutoff. No production
admission, merge, push, daemon update or P095 transfer follows from the isolated
SQL-expansion approval. No old source is resumed, cleaned or cancelled.
