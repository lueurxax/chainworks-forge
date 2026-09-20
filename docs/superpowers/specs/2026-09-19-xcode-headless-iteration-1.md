# Headless Iteration 1: Open And Read

Revision 5, 2026-09-19. Normative I1 child of the
[headless design](2026-09-19-xcode-headless-mcp-design.md).
Status: experiment closed; H1 **Supported for the tested ordinary-routing scenario**.
See the [evidence composition and adaptations](../../evidence/xcode-headless-i1-live-2026-09-19.md).
The latest runtime report remains `Partial` because offline cases require
separate composition; R5 supplies it without altering any generated attempt. No
production admission, I2 approval or full-migration Ready verdict is granted.

## Hypothesis And Decision

H1: with Xcode IDE absent and a supported headless service already running,
Chainworks can select the exact project within an execution root, open it,
receive its workspace identifier, and read that project's known content by
explicit identifier. This remains true for two distinct roots with identical
relative project/file names and after reconnecting only the owned bridge.

H1 tests ordinary project routing and the usable request/result contract. It
does not test or establish adversarial filesystem isolation, ID non-reuse,
atomic mapping validation, a packaged identity or a production provider lease.
No successful example can be extrapolated to those guarantees.

The decision after I1 is whether to keep/adapt the minimal controller and how
to specify I2. The decision is not whether to deploy the complete migration.

R5 composes the latest `f39670ec-af8f-4032-bda7-73f04d419f54` live passes with
offline root-selection and fault checks. Both fixtures opened with distinct IDs
and matching structured paths; each connection completed explicit A/B/A reads,
the second with the same IDs and no open. The IDE remained absent and service
generation stable at all prescribed checks. Both owned bridges exited 1 without
forced termination, separately from successful RPCs and harness exit 0.
The saved regression records 37 harness tests, 351 total including the harness,
and one existing ignored test. All 575 source and eight seed hashes were
verified unchanged post-run. This is one warm-service development scenario,
not evidence of production integration or service-wide ID stability.

## Minimal Implementation Boundary

- Implement project selection and a small headless controller behind injected
  host/process/MCP interfaces, following existing ACP transport patterns.
- Keep the experiment opt-in in a development/test harness. Do not wire it into
  production daemon admission, provider sessions, HTTP leases or shim grants.
  No production flag may disable binding or confinement checks for this path.
- Use an explicit canonical execution root and project selection. Offline cases
  cover the existing repository/dedicated/shared-worktree resolution rules,
  including read-only worktree tasks and missing required worktrees. Live
  fixtures exercise two root slots; this is not engine integration proof.
- Pin the selected installation and independently verified service generation.
  Never select an IDE process or use command text as executable identity.
- Initialize and check the captured schemas for the two exercised tools:
  internal `XcodeOpenWorkspace` and `XcodeRead`. Parse bounded JSON-RPC and MCP
  results, distinguish `isError`, and retain unexpected shapes as deviations.
- Permit read-only CLI status for host readiness and optional mapping readback.
  Inspect populated `status --format json` without assuming an unobserved shape;
  only a structured ID/path association can supply mapping evidence. No extra
  MCP tool is added to obtain it, and unrelated workspace detail is not retained.
- Permit only the harness's fixed fixture paths and internal call sequence.
  There is no caller-supplied arbitrary tool/method/argument passthrough.
- Store an `ExperimentalWorkspaceObservation`: requested canonical project,
  observed service generation, returned ID, optional returned path, mapping
  evidence source/status, and fixture read result. It cannot construct or
  deserialize as a production `VerifiedWorkspaceBinding` or readiness ticket.
- Invoke `XcodeRead` with the explicit returned ID, known fixture project path
  and bounded range. Project-organization paths are not assumed OS paths;
  the fixture layout defines their relationship for this experiment only.

No source editing, builds, tests, preview, snippets, device actions, arbitrary
discovery/listing tools, workspace close/new-project or permission administration
are admitted through MCP. Repository implementation tests still use managed
Cargo; any later Swift work uses canonical gates. UI tests remain remote-only.

## Disposable Fixtures And Side Effects

Prepare two minimal local project fixtures, A and B, each with the same relative
project and file names but different fixed sentinel content. Use fresh dedicated
directories outside the real checkout and existing run worktrees. Fixture
creation writes only these files; thereafter compare source/project digests
before and after the experiment. Record service-generated metadata separately.
Fixtures contain no secrets, external references, packages, symlinks or build
scripts. Do not seed by copying a live application or its private settings.

Recorded adaptation after the first diagnostic attempts: the original
target-free fixture reached Apple's 10-second wait for
`hasResolvedInitialRunDestination`. New fixtures add one macOS command-line
target, a shared scheme and a local C source returning zero, with no packages,
dependencies or shell-script phases. Hash/validate these additional seeds too.
This tests a destination-readiness hypothesis; it does not authorize building
or executing that target, and preserves the original failed-attempt evidence.
The adapted fixtures opened successfully in `fea2590b` and `f39670ec`; this
observes their usability without proving the cause of the earlier timeout.

Opening a workspace changes shared service state and can trigger consent or
indexing; this is not a pure read-only experiment. Before live execution,
obtain explicit authorization for these fixture paths and the actual launch
identity. Approval of this specification alone does not grant OS permissions.
Do not approve folders/agents, reset permissions or use unsafe allow-all
automatically. If consent is unavailable, record `blocked` and stop.

Authorization history: the user subsequently approved the inspected pair and
fresh I1 pairs within the restricted scratch prefix without repeated per-pair
questions. Source/identity inspection and all no-replay/no-production limits
still applied; specification approval never substitutes for OS consent.
The latest instruction permits documentation-only closeout and **no further
live activity until tomorrow (2026-09-20)**: no probes, permission actions,
service/IDE/run operations, commits or pushes. Nothing here schedules a resume.

Use one harness at a time for its dedicated fixtures and a bounded request
budget. Record durable local dispatch intent before each open and terminal
result afterward. Missing/unwritable evidence storage means no open dispatch.
After timeout, cancellation, lost response or crash, mark/retain uncertainty:
never automatically replay, reopen, close/reopen or resume that fixture target.
A later attempt on the same target requires explicit operator authorization
after inspecting status and the prior record; inability to reconcile keeps it
blocked. This small experimental record is not the production effect journal.

Leave opened fixtures/workspaces in place and report their paths for later
authorized cleanup; deleting a directory is not a substitute for closing it.
Close/reap only owned bridge processes. Never stop the shared service, close
foreign workspaces, change live daemon state or open Xcode IDE. Abort on service
generation change or observed interference rather than attempting recovery.

## Test Sequence And Observable Criteria

Write focused offline regression tests with the implementation, then complete
the live sequence only after the minimal slice passes them. This does not
require live answers in advance of implementation.

| Case | Expected observation | Evidence boundary |
| --- | --- | --- |
| I1-01: root selection | Exact selected project; missing/ambiguous/foreign selection rejected before open | Offline; covers worktree strategy without changing engine production wiring |
| I1-02: warm attach | Correct service identity, supported negotiation, IDE absent before and after | Live; absent/disabled/unsafe service is blocked, not cold-started |
| I1-03: open A and B | One bounded open per fixture yields valid explicit IDs and records actual result shape | Live; open is an effect, not permission to retry it |
| I1-04: structured mapping | Returned path equals requested canonical package, or a supported structured readback links the same ID to that package | Live; prose/echoed request/ID alone is not authoritative mapping |
| I1-05: scoped reads | Read A, B, then A again by explicit IDs; each returns its own expected sentinel and bounded file/range metadata | Live; fixtures share relative names so a global-current-project default is observable |
| I1-06: reconnect | Replace only the owned bridge, reinitialize and repeat the explicit reads without another open | Live; retain resulting ID behavior, do not assume cross-bridge stability |
| I1-07: denials and uncertainty | Denial, malformed/conflicting results, `isError`, timeout and changed service stop dispatch; an absent optional mapping remains unverified; no open replay | Offline fault fixtures; a naturally occurring live denial is recorded, never manufactured by changing host permissions |
| I1-08: retained boundaries | No provider route, source mutation RPC, build, IDE launch, service stop or foreign cleanup; source/project digests unchanged | Offline dispatch allowlist and live before/after evidence |

I1-04 is reported independently of routing. If optional `workspacePath` is
absent and no supported structured mapping is available, record mapping as
`unverified`; controlled sentinel reads may continue only in this fixture
harness. Never parse prose into authority, fabricate a mapping revision, or
mark a production lease ready. A returned path that contradicts the request
stops that target before its read; it is a failed check, not optional evidence.

No direct service probes of identifier races, path escapes or third-party
projects are in scope. Offline negative cases check our own validation logic.
Neither two sentinels nor rejected synthetic selectors prove service confinement.

### Observed Read Representation

The newline-terminated eight-byte sentinel is returned as two editor lines:
`totalLines = 2`, `linesRead = 2`, `startLine = 1`, `fileSize = 8`, content
`"     1\tCW_I1_A\n     2\t"` for A (B substitutes `CW_I1_B`). The adapted oracle
requires the exact marker and empty second line, in order, with exact metadata.
The retained RED/GREEN adaptation includes rejection of extra/reordered lines,
nonempty/missing second lines and the other marker. It does not loosen the
oracle to a substring match or relabel a failed check as optional.

Historical `fea2590b` opened and mapped both fixtures but stopped at the first
correct-sentinel response because its old oracle expected one line. Its
`Refuted` report remains unchanged: the representation assumption failed, not
an observed cross-routing check. Only the fresh adapted run completes H1.

## Evidence And Adaptation

For each attempt create an evidence directory under
`docs/evidence/xcode-headless/iteration-1/<attempt-id>/` containing a short
`experiment.md`, bounded redacted exchanges and a file-digest manifest. Record:

- exact source commit and dirty-source content digest, fixture content digests;
- Xcode/server versions, negotiated protocol, schema capture digests and actual
  development launch/signing identity (never substituted for packaged consent);
- service generation and IDE absence before/after, permission outcome;
- requested versus returned project mapping, ID relationships, scoped read and
  reconnect results, local dispatch-intent/outcome evidence for each open;
- per-case expected/actual result, status, evidence reference and limitation;
- deviations, adaptation made, regression test added and next decision.

Do not persist tokens, environment secrets, private project content or unrelated
workspace listings. Public/review evidence uses fixture/host labels and digests;
local privileged paths needed for cleanup are kept separate and referenced by
label. A missing live check remains `not_run` or `blocked`, not a fixture pass.

Revision 4 diagnostic decision: at the user's explicit request, retain complete
requests and responses for the fixed disposable-fixture sequence in the private
attempt directory, with 0700 directories and 0600 files. Do not truncate error
text to a separate 4-KiB excerpt; retain the existing transport resource bounds.
Record before decoding so a rejected Apple response remains diagnosable. Public
evidence still receives only the redacted result/shape/hash, never these raw
transcripts. These files are development diagnostics, not a production logger
or proof that every recorded request was completely transmitted.

Use these outcome rules:

| Outcome | Meaning / next action |
| --- | --- |
| H1 supported for tested scenario | All I1 cases pass after explicit offline/live composition, including routing/reconnect and observed structured mapping; confinement remains unestablished; propose the next slice for review |
| Partial | Evidence is incomplete: runtime intentionally leaves offline I1-01/I1-07 NotRun pending composition; alternatively reconnect/mapping differs or is unavailable. State which reason applies and never infer missing evidence |
| Refuted | Returned data/path contradicts the selected fixture, or the proposed sequence cannot work on the tested contract; add a regression case and revise implementation/hypothesis |
| Blocked | Consent, host availability or another prerequisite prevents observation; preserve unknowns and request only the specific missing action |

Do not relabel a failed case as optional after the fact. An adapted hypothesis
receives a revision and retains the previous expected/actual record. Finish the
iteration by updating the parent and review disposition with the evidence;
do not convert a development experiment into release acceptance.

For `f39670ec`, the unchanged runtime cases are
`[NotRun, Pass, Pass, Pass, Pass, Pass, NotRun, Pass]`, with no error/deviation
or unresolved open. The latest saved offline logs supply Pass for I1-01/I1-07;
the closeout table links the evidence for each case and supports H1 only for
the tested scenario. These logs supersede the historical 33-test baseline for
the adapted source; 351 includes 37, not 351 plus 37. No tests or live checks
were rerun by the documentation sidecar.

## Exit And Review Criteria

I1 is ready to close only when its minimal implementation has focused offline
verification, the authorized live cases have recorded outcomes (including
blocked/partial outcomes), uncertainty/cleanup state is explicit, and the
hypothesis plus next decision are updated. Closing an experiment is not the
same as confirming H1. The first live attempt is retained as blocked; further
diagnostic attempts do not overwrite its outcome or unresolved-open record.

R5 meets these experimental exit criteria through the linked evidence, not a
new live run. Earlier `d43e6994`/`c2c417e2` uncertain A records, fixtures and
metadata remain retained without replay or cleanup. The latest report's
`unresolved_open = false` does not reconcile those earlier attempts. The
progress ledger and original plan are read-only inputs to this sidecar. Later
parallel offline work has a separate source identity, outside this I1 verdict.

The repeat review should check whether the fixture restriction and absence of
production exposure make this implementation scope acceptable, whether the
observable criteria distinguish routing from authority, and whether every
failure has a bounded stop/adapt path. Remaining production guarantees stay
open in the runtime/wire children. Written approval and a reviewed I1 plan
preceded implementation; the recorded live authorizations preceded opening.
Next, review the evidence and choose a bounded hypothesis with its production
authority/effect prerequisites explicit. No full I2 specification, new live
authorization or production readiness follows from closing I1.
