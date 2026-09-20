# Headless I1 Live Evidence And R5 Closeout

2026-09-19. R5: I1 experiment closed; H1 **Supported for the tested ordinary-routing scenario**.
This conclusion composes the latest live report with separate offline evidence;
the immutable runtime report intentionally remains `Partial`.
At the final I1 run, I1 had changed neither production admission nor the
production implementation. Later offline dependency work has a separate scope
and source identity; production admission and the installed daemon are unchanged.
No IDE launch, service restart, production deployment or provider run occurred.
This documentation-only closeout performs no new live checks or permission actions.

## Attempt History

| Attempt | Source content SHA-256 | Outcome |
| --- | --- | --- |
| `9ff20b4e-ba21-4f7d-ad18-12481b44d85d` | `7699d8f3866d3c36560fecdf697cf495ef4a2bf46cc3b59634208b1fbdf37966` | Inspection blocked before any MCP calls: incorrect flat status-field assumption |
| `d43e6994-9c85-4222-b73b-91d140ef4256` | `e3485682a0939b61f4354a89d1f58f76cad4fe99b048dbb24bf208cc56c8f80e` | Explicitly authorized live attempt stopped at the first open with MCP `isError: true` |
| `c2c417e2-be40-4366-81db-bc49dd26280a` | `0d18f34784eb862a1b883efe9367020974a0f00b7770e3c5560e7dfb239c5999` | Blocked at A: full response identifies Apple's 10-second initial-run-destination timeout |
| `fea2590b-e612-43b0-9735-008b15ac3532` | `50ed6bb2df9eada9f86876b25a3b1240cbfddc33bf3daca0be99b224df84756a` | Both opens and mappings passed; first read rejected by the old one-line oracle; historical report remains `Refuted` |
| `f39670ec-af8f-4032-bda7-73f04d419f54` | `05dce156a417de8e8b9b86c077611e2815f34a396aed6d771502676501fd8785` | Run exited 0; all live cases passed; runtime `Partial` awaits offline composition, completed below |

These are dirty-source identities at HEAD
`a096b8831f0db7f17d1ffe426851c49a65b8b000`, not committed revisions.
The first three source identity records contain 573 file hashes; the target
fixture revision adds two templates, for 575 files. The original offline
[evidence](xcode-headless-i1-offline-2026-09-19.md) remains the historical
33-test baseline, not verification of these adaptations.

## R5 Evidence Composition

Latest immutable [report](xcode-headless/iteration-1/f39670ec-af8f-4032-bda7-73f04d419f54/report.json)
and [manifest](xcode-headless/iteration-1/f39670ec-af8f-4032-bda7-73f04d419f54/manifest.json):
`cases = [NotRun, Pass, Pass, Pass, Pass, Pass, NotRun, Pass]`,
`outcome = Partial`, `error = null`, `deviations = []`,
`unresolved_open = false`. I1-01 and I1-07 are offline cases, deliberately not
composed by the runtime harness. `Partial` here does not mean a failed live case.

| Case | Latest runtime | Separate evidence / composed result |
| --- | --- | --- |
| I1-01 | NotRun | Pass offline: exact root/project selection, worktree strategy, missing/ambiguous/linked selection and overwrite refusal |
| I1-02 | Pass | Warm service identity/negotiation checked; IDE absent and the same service generation at all prescribed checks |
| I1-03 | Pass | Two opens on connection 1, one per fresh fixture; distinct returned workspace IDs |
| I1-04 | Pass | Structured returned path mapping observed for both requested canonical packages |
| I1-05 | Pass | Explicit A/B/A reads on connection 1 returned the correct sentinels and exact metadata |
| I1-06 | Pass | Connection 2 repeated explicit A/B/A with the same IDs and zero opens; six reads total, A four and B two |
| I1-07 | NotRun | Pass offline: denial/error/malformed/conflicting/timeout/generation and storage/transport fault cases stop dispatch and retain uncertainty without replay |
| I1-08 | Pass | Restricted calls, source/seed preservation, retained generated metadata and owned-child reaping; no production route or foreign cleanup |

Both owned bridges exited with code 1, `forced = false`. These are transport
process completion observations, recorded separately from successful RPCs and
the harness's exit 0; they are not rewritten as exit 0 or classified as failed
reads. IDs were stable across these two connections, not proven non-reusable
or stable across service restarts. The recorded service-generation digest was
`b27b2b7abb9c88362af1315fb6f6d8ffcd3f76945a89c5a83e1ad0e678f963a7`.

The latest saved [harness log](../../.superpowers/sdd/2026-09-19-xcode-headless-i1/example-read-lines.log)
records 37 passed, zero failed. The saved serial
[ACP regression log](../../.superpowers/sdd/2026-09-19-xcode-headless-i1/regression-read-lines.log)
records 351 passed **including those 37**, zero failed, and one existing ignored
manual latency test (196 + 15 + 76 + 27 + 37 passed). These tests were not rerun
by this sidecar. The original 33-test offline record remains historical; the
latest logs cover the adapted implementation and the offline cases composed here.

| Retained artifact | SHA-256 |
| --- | --- |
| Latest report | `d8c0978a907991328ca45050630151a9eaa56acfe1b33389d9cd24e913dd002c` |
| Latest manifest | `de31fd1a804f22938756a18e1fcf100ac346957b771cb97040aca72f830492fb` |
| Latest inspection (manifest pin) | `58b529f37da4f574abd8c859e75d3358e77e606f50229fc258d10b82f76be4ec` |
| Harness read-lines log | `aa769517a2938c59f5096c0f92d4b77258c9ad95b7a859b74d1090041a5b05e0` |
| Serial regression read-lines log | `56c5114693360f6a6ac0710abccf400d82c8d019cac11d5d78620eb518e1c2e5` |

The execution owner's verified post-run handoff records all 575 source hashes
and all eight fixture-seed hashes unchanged, plus the connection-level trace
checks and run exit 0 above. This sidecar independently rehashed the 575 local
source inputs to the latest manifest identity at its initial checkpoint and
verified report/log digests;
it did not inspect the host or reopen external fixture directories. Both
fixtures' added workspace metadata is recorded separately in the report with
digest `7f3b00b5c3fdb45242d7b87e1e5c4e25d1fa8129a16c94295ecc4e8ea2235c5f`.
Full private traces remain outside the repo in 0700 directories and 0600 files,
as verified in the handoff. Public evidence retains only redacted summaries,
shapes and hashes, not raw traces, workspace IDs or private fixture paths.

During final static validation, parallel offline-foundations source changes
and an owner update to the progress ledger appeared. They were left untouched.
The I1 verdict and test counts apply to the pinned 575-input source identity,
not that newer dirty tree; this closeout does not certify the parallel work.

## Historical First-Open Observations

The installed status result has `/permission/enabled` and
`/permission/unsafeAlwaysAllowAllAgents`; `/running` is top-level. The flat
parser stopped with `i1_status_shape` (public fallback `i1_stopped`). Two tests
reproduced the mismatch before the parser was corrected. Missing/non-boolean
flags still block; no flat fallback was added.

The second inspection verified Xcode build `27A266a`, the selected Xcode
Service process, absent IDE, enabled/running status and unsafe allow-all false.
It pinned the development executable and actual signed parent/bridge chain.
Approved inspection digest:
`722084d44dbeadf11afb71851a9f7d178ae2a00f5000e39ae92bc0261b7b663e`.
The user explicitly authorized only the named A/B pair and fixed sequence.

The bridge initialized and validated the two captured schemas. It sent exactly
one tool call, `XcodeOpenWorkspace` for A. The Apple activity log independently
records one tool call and disconnection. Apple recorded user approval for this
agent and fixture A's folder. This is development consent, not packaged consent.
The tool returned `isError: true`; no ID/path binding was accepted. B was not
opened, no reads occurred and no reconnect occurred. The owned bridge was
reaped with exit code 1, without a forced kill.

Post-attempt CLI status showed `openWorkspaces: []`, service running and unsafe
allow-all false. A separate process inventory showed no Xcode IDE or harness.
Other clients' bridges were left untouched. No evidence claims that those
clients are isolated or exclusively controlled by this experiment.

All four project/source seed hashes still match. Apple added only A's embedded
`project.xcworkspace/contents.xcworkspacedata` within the inspected fixtures.
Its bytes have SHA-256
`7f3b00b5c3fdb45242d7b87e1e5c4e25d1fa8129a16c94295ecc4e8ea2235c5f`.
The created metadata establishes a side effect; an empty workspace inventory
does not erase the uncertain open record. Both fixture directories remain.

## Historical First-Open Evidence And Per-Case Result

Immutable generated attempt files:
[report](xcode-headless/iteration-1/d43e6994-9c85-4222-b73b-91d140ef4256/report.json),
[manifest](xcode-headless/iteration-1/d43e6994-9c85-4222-b73b-91d140ef4256/manifest.json).
Report SHA-256: `c7cc9998868bcc691f0b39099b0f1964a058592e2e669312f9e8025074639884`.
Private intent, uncertainty and inspection records remain outside the repo.

| Case | Result |
| --- | --- |
| I1-01 | Pass offline; original root-selection tests unchanged |
| I1-02 | Initial host/handshake observed; runtime report conservatively Unverified because its complete ending observation did not run |
| I1-03 | Blocked on A's tool error; open outcome retained as uncertain |
| I1-04 | Not run: no successful open result |
| I1-05 | Not run: no reads |
| I1-06 | Not run: no reconnect |
| I1-07 | Pass offline; live failure stopped subsequent dispatch and retained intent |
| I1-08 | Source/seed preservation and owned-child completion observed; no complete successful live sequence |

The reported error shape and payload digest are retained, but the original
error text is not recoverable from that digest. Host logs also contain plugin
load warnings; their causal relationship to this tool error is **unproven**.
Neither a permission failure nor a malformed fixture is declared the cause.

## Historical Diagnostic Adaptation

The user requested complete requests and responses instead of the proposed
4-KiB private error excerpt. The development harness now records complete
transport messages in `control/record/bridge-N/requests.ndjson` and
`responses.ndjson`, in the private attempt directory. Directories are 0700,
files 0600. Raw transcripts are not copied to repository/public evidence.
The existing 1-MiB message and 8-MiB received-stream bounds remain; there is no
additional text truncation. Requests are recorded before writing to the pipe,
so a log entry alone does not prove complete transmission or successful effect.

A real synthetic-process test first failed for absent logs, then passed with
byte-for-byte requests (including initialization notification), the complete
7-KiB error text, restrictive modes and no transcript overwrite. All 36 harness
tests passed after this change. The preceding status-only revision passed the
serial ACP regression with 349 tests and one existing ignored test.
The full transcript revision then passed 350 ACP/example tests, with the same
one existing manual latency test ignored, and the workspace formatting check.
Local regression-log SHA-256:
`41dec30d47903bff28e0cab29dda5fb3c18f498e136f621193fbebc8c589d2aa`.

The diagnostic pair was explicitly authorized after inspection, digest
`23efce004a4739476c29aacc3c07907daa6dee2399dc1f7a9b8fb58f1ea1c050`.
Its outcome is recorded separately from the first live attempt below.

Subsequent experiments used fresh fixtures and preserved the prior uncertain
target without replay or cleanup, with per-attempt path/source/identity
inspection. Structured fixture mapping was later observed; confinement, durable
production effects and release acceptance remain unestablished. I2-I4 require
separate review and authorization.

## Full-Transcript Attempt And Fixture Adaptation

Attempt `c2c417e2-be40-4366-81db-bc49dd26280a` sent one open A and received
`isError: true`. The nested JSON text response was `type: error`, with `data`
stating a 10.0-second wait for `hasResolvedInitialRunDestination` on fixture A.
This is Apple's internal timeout, not the harness's 30-second request deadline.
No B open, reads or reconnect followed. The owned bridge exited 1 without a
forced kill. A's uncertain-open record and generated workspace metadata remain.

Private transcript digests, not their raw contents:

| File | SHA-256 |
| --- | --- |
| requests.ndjson | `d2d7e9e8959257fa4466ebee58decacbbc61e33a91a6316e9ed4b21de78285aa` |
| responses.ndjson | `ca105503985f26bad5177f0a3ad9d7a2a2e13f8d2b15ec2d7d7e0493d954f890` |
| generated public report | `e67c3dde51872df7a945c6e7ef344d2f65976f35188b6a64e6e8e0017d2ad302` |

The original target-free fixture has no scheme/run destination. The adaptation
hypothesis was that a minimal native macOS command-line target plus a shared
scheme supplies the destination expected by open. Later attempts below opened
both adapted fixtures successfully. This supports their usability, not isolated
proof of the original timeout's cause; unrelated plugin warnings are not
treated as its cause.

The fixture now contains `Sources/main.c` (returns zero), one macOS target and
`xcshareddata/xcschemes/Fixture.xcscheme`, in addition to the existing project
and sentinel. It has no dependencies, packages or shell-script build phases.
No compilation, execution, simulator/device operation or new MCP tool is added.
All eight seeds are hashed and fixed-content validated; old attempts are not
edited. The original plan's target-free assumption is superseded only for new
fixtures by this recorded experimental adaptation.

A parser-based regression first failed on zero targets, then passed with the
native target, macOS settings and tamper detection for scheme/source seeds.
All 37 harness tests and standalone XML validation passed. The full serial ACP
regression passed 351 tests with one existing manual test ignored; its local log
SHA-256 is `abdb431fb12e9239fb2b12cc886e78ebea1f9f4446f470e4b1dd771c700c541b`.
Those were pre-live results for the target adaptation. The subsequent live
observations and the newer read-lines logs above supersede their pending status.

## Historical Read-Representation Mismatch And Adaptation

Attempt `fea2590b-e612-43b0-9735-008b15ac3532` opened A and B successfully and
observed both structured mappings. Its first A read contained the correct
sentinel, but the old oracle expected one line. Apple represented the final
newline as an empty second editor line: `totalLines = 2`, `linesRead = 2`,
`startLine = 1`, `fileSize = 8`, and content
`"     1\tCW_I1_A\n     2\t"`. No B read or reconnect followed that rejection.

The immutable [report](xcode-headless/iteration-1/fea2590b-e612-43b0-9735-008b15ac3532/report.json)
remains `Refuted`, with `i1_read_mismatch`, I1-05 `Fail`, and zero verified
reads. Its SHA-256 is
`22157a6bf2d8d87dc579c06d71b9d7bab8b26613508375ced0d24ac458a71ac8`;
the [manifest](xcode-headless/iteration-1/fea2590b-e612-43b0-9735-008b15ac3532/manifest.json)
has SHA-256 `b76a394172700f21bbd533b5e7c1d5f3e678b3eefb132c2aef5415c57a930b76`.
The refuted assumption is the harness's read representation, not evidence of
cross-project routing. This clarification does not retroactively pass that run.

The execution handoff records a RED/GREEN adaptation of the exact read oracle.
The retained test and validator require two ordered numbered lines, the exact
fixture marker followed by an empty line, and exact path/size/range metadata.
Negative cases still reject an extra line, reordered numbering, a nonempty
second line, a missing second line and the other fixture's marker. The saved
37-test and 351-test logs verify GREEN; this closeout does not replay RED/GREEN.
The fresh `f39670ec` attempt then passed the complete authorized sequence.

## Authorization, Retained State And Next Decision

The [progress ledger](../../.superpowers/sdd/2026-09-19-xcode-headless-i1/progress.md), read-only to this sidecar,
records the user's authorization of inspected `fea2590b` and subsequent fresh
I1 pairs under the restricted scratch prefix, without repeat per-pair questions.
Each attempt still required actual source/identity inspection and all existing
no-replay/no-production/no-administration limits. That approval was not I2
approval, packaged OS consent or permission to replay uncertain opens.

The latest instruction supersedes further live work: the user is sleeping;
**no further live activity until tomorrow (2026-09-20)**. This sidecar performs
documentation/static validation only: no probes, permission actions, service or
IDE operations, provider/run operations, commits or pushes. No automatic resume
or scheduled action is created by this closeout.

The uncertain A records from `d43e6994` and `c2c417e2` remain unresolved and
retained, without replay or cleanup. `unresolved_open = false` in the latest
report applies only to its own pair. All fixture directories and observed
workspace metadata remain for later authorized reconciliation/cleanup; no
foreign workspace or bridge was touched.

Next decision: review the composed I1 evidence and select the smallest next
hypothesis, including the production mapping/confinement and lifecycle-effect
prerequisites before any broker lease is admitted. No full I2 specification is
written or approved here. Ordinary routing support does not establish atomic
mapping, ID non-reuse, confinement, cross-client isolation, cold start, packaged
consent, durable production effects or full-migration/release readiness.
