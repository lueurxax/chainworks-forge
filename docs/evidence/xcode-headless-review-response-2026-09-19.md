# Headless Proposal Review Disposition

Current status: [I1 implementation and live evidence](xcode-headless-i1-live-2026-09-19.md)
close the approved experiment at R5: H1 **Supported for the tested ordinary-routing
scenario**, through explicit offline/live composition. The latest generated
report remains `Partial`; historical blocked/`Refuted` attempts remain unchanged.
This does not close production findings, approve I2-I4 or declare full readiness.
Production admission and the installed daemon are unchanged. Current production
source includes later offline root-selection changes under the
[separate dependency plan](../superpowers/plans/2026-09-19-xcode-headless-offline-foundations.md),
without enabling headless dispatch; the standalone DB journal is implemented
but not connected to production effects. Its verification is recorded in the
[offline checkpoint](xcode-headless-offline-2026-09-19.md).
This dependency work is not full I2 and is not certified by the I1 evidence.
R6 updates the parent's offline implementation status; R5 closeout identities
remain historical and the I1/runtime/wire/release children are unchanged.

Earlier status: the user's [repeat review](xcode-headless-i1-review-2026-09-19.md)
accepted R3 for I1 planning only and raised three nonblocking future P2 items.
The handoff and pre-review statements below remain a historical record of R3
preparation. R4 recorded the first blocked live attempt and private logging
adaptation. R3 and R4 pins below remain historical identities; neither is the
identity or an independent review of the R5 parent/I1 bytes.

Date: 2026-09-19. Review mode: proposal-readiness.
Reviewed source HEAD: `a096b8831f0db7f17d1ffe426851c49a65b8b000`.
Reviewed parent MD5: `de96080e5c2400f1eb23fa10811917d3`.
Both matched the working tree before revision. Input was the user's pasted
review with verdict **Not ready**, six P1 items and one P2 item.

Revision 2 updated the
[parent design](../superpowers/specs/2026-09-19-xcode-headless-mcp-design.md)
and added bounded normative runtime, wire and release contracts. Revision 3
records the completed schema study and the user's iterative approach, adds an
isolated first-iteration contract and scopes the earlier contracts to later
production admission/release. These are design responses, not an independent
re-review, implementation proof or a Ready verdict. No production source,
frozen input, live daemon or workspace changed during these document revisions.

## Revision 5 Experiment Closeout

Latest attempt `f39670ec-af8f-4032-bda7-73f04d419f54` exited 0 and records
`[NotRun, Pass, Pass, Pass, Pass, Pass, NotRun, Pass]` for I1-01 through I1-08.
The two NotRun cases are offline-only root/fault checks; the runtime harness
deliberately does not compose them. The latest saved logs show 37 harness
tests and 351 total selected ACP/example tests including the harness, with one
existing ignored test. The linked closeout composes these into scenario-limited
H1 support without changing the `Partial` report or inventing live fault probes.

Both connections completed explicit A/B/A reads with correct sentinels, distinct
A/B IDs stable across reconnect, and structured returned path mappings. There
were two opens on the first connection and none on the second. IDE absence and
stable service generation were observed at all prescribed checks. Owned bridges
exited 1 without forced termination, recorded separately from successful RPCs.
At the final I1 run, all 575 source and eight fixture-seed hashes were verified
unchanged post-run; I1 had made no production source changes at that point.
These are historical checks, not a claim that the current source tree matches.
Full private traces remain outside the repo with 0700 directories/0600 files;
only redacted summaries, shapes and hashes enter public evidence.

Historical `fea2590b` passed both opens/mappings but its first correct-sentinel
read failed the old one-line oracle. Apple's empty second editor line led to an
exact two-line RED/GREEN adaptation with extra/reordered/nonempty-line negative
cases retained. Its `Refuted` report is preserved as a representation-assumption
failure, not cross-routing evidence. Earlier `d43e6994`/`c2c417e2` uncertain A
records remain retained, without replay or cleanup; the latest pair's resolved
opens do not reconcile them.

The user authorized inspected fresh I1 pairs without repeated per-pair
questions, with source/identity checks and the same restricted scratch scope.
The latest instruction supersedes further live work: no live activity until
tomorrow (2026-09-20). This sidecar edits only the four authorized Markdown
documents and performs static evidence validation; it does not rerun tests,
probe or operate services/IDE/runs, act on permissions, commit or push. This
sidecar does not edit the ledger, original plan, source or generated attempts.
Parallel offline-foundations work and the owner's later ledger update have
a separate provenance and do not change the pinned I1 conclusion.

Next decision: review the I1 evidence, select the smallest useful next
hypothesis, and explicitly address mapping/confinement and lifecycle-effect
prerequisites before production broker admission. No full I2 specification or
I2 approval is created. ID non-reuse, atomic mapping, execution-time containment,
shared-state isolation, cold start, packaged consent and release proof remain
unestablished; the runtime/wire/release contracts are unchanged.

## Revision 2 Response

| Item | Source-backed assessment | Revision 2 / remaining evidence |
| --- | --- | --- |
| P1-01 | Confirmed: engine evaluates session policy before broker attachment; ACP execute branches directly to prompt for reuse | Runtime contract defines typed root/generation, prepare-before-policy, commit tickets on both paths, endpoint compatibility and cancellation/crash cleanup |
| P1-02 | Confirmed: daemon wires broker and shim independently; shim dispatch executes its own host plan | One injected coordinator, reentrant ownership, fair bounded permits and durable holds. Existing per-DB singleton is real but insufficient across different databases; per-UID admission and DB-authority pin are required |
| P1-03 | Confirmed: current HTTP forwarding has no durable per-effect dedupe/outcome authority | Durable attempt state machine, dispatch fence, explicit prepare/commit nonce contract, shim participation and evidence-backed reconciliation. An unknown result holds the project against new nonces too |
| P1-04 | Confirmed: the parent did not define a closed wire contract or an enforceable build-gate choice | Domain-owned schema bundle, digests, broker envelopes, error matrix and handle registry specified. Raw Apple build/execution tools are unavailable for this repo. **Follow-up:** all 54 advertised input/output schemas are now captured; schema absence is resolved. Admitted closed adapters and error/response normalization remain unfinished |
| P1-05 | Confirmed: service liveness alone does not prove workspace mapping; checking a path before another process uses it cannot eliminate a race | Per-dispatch mapping validation plus upstream non-reassignability/atomic binding and executor-side containment are admission requirements. **Partial:** those Apple guarantees have not been established; affected tools remain unavailable, and missing required capabilities block release |
| P1-06 | Confirmed: health route has no auth; old Swift health enum is closed and strictly decoded | Additive versioned detail, unknown/absent/not-applicable semantics, explicit privilege projection, coarse public health, legacy enum preservation and negotiated mixed-version reads |
| P2-01 | Confirmed: the first draft listed checks without a receipt or completeness rule | Versioned redacted release receipt, source/evidence digests, named mandatory checks, required capability matrix and fail-closed validator contract |

## Evidence Anchors

These are source references at the reviewed HEAD, not runtime observations:

- [Engine fingerprint and session decision](../../control-plane/crates/engine/src/executor.rs): effective working directory is resolved before `SessionPolicyInput`; its Xcode input is an intent hash, not a prepared project-ready binding.
- [ACP session manager](../../control-plane/crates/acp/src/manager.rs): `execute` chooses `prompt_session` when reuse is requested; new-session attachment cannot protect that branch by itself.
- [Daemon wiring](../../control-plane/crates/daemon/src/main.rs): separate construction of `XcodeMcpBridgePool` and `XcodeShimGrantRegistry`; database lock admission is per database.
- [Singleton implementation](../../control-plane/crates/daemon/src/supervisor.rs): actual kernel locking exists. The review should not be read as saying there is no singleton mechanism at all.
- [Shim execution](../../control-plane/crates/acp/src/xcode_shim.rs): plan validation and host process execution are separate from broker request serialization.
- [Broker HTTP route](../../control-plane/crates/daemon/src/xcode_broker_http.rs): unauthenticated health and authorized request-by-request forwarding; no effect journal/dedupe at this boundary.
- [Domain observation model](../../control-plane/crates/domain/src/xcode_runtime.rs): bounded observation arrays can be truncated; they cannot serve as the durable effect ledger.
- [Swift health DTO](../../Chainworks%20Forge/Support/DaemonLifecycleClient.swift): existing raw-value Codable health enum has four cases and is decoded with `decode`, so new enum values are not automatically tolerated.

## Limits Of The Proposed Remedies

Nonce dedupe alone cannot infer whether a differently identified request is a
retry. The design therefore also blocks all conflicting work while outcome is
unknown. It does not claim exactly-once execution of arbitrary Apple commands.

A mapping precheck is necessary but not sufficient for atomic binding. A
`realpath` precheck is not a filesystem sandbox for a remote executor. Revision
2 does not replace these missing guarantees with more optimistic wording or a
successful happy-path test. No generic file-edit proxy was added as a substitute.

The [subsequent metadata-only study](xcode-headless-contract-2026-09-19/README.md)
resolved the missing upstream schema inventory: all 54 input/output schemas are
captured. P1-04 still needs the admitted facade and response/error decisions;
that is no longer blocked on finding the advertised schema. P1-05 still lacks
the stronger execution-time scope guarantee. Packaged launch/permission behavior
also remains unverified. Denying a required capability does not count as a
successful full migration.

## Revision 3 Disposition

The user selected an evolutionary loop: form a hypothesis, implement a minimal
slice, test it, retain expected/actual deviations and adapt. Complete advertised
schemas are already available; repeating documentation discovery is not an I1
entry gate. New sources remain welcome, but additional prose cannot establish
runtime guarantees that have not been observed or documented.

R3 keeps the full managed headless-only destination and removes the accidental
dependency on completing the entire production architecture before the first
experiment. The new [I1 contract](../superpowers/specs/2026-09-19-xcode-headless-iteration-1.md)
allows only development-harness open/read on two known-content disposable
projects. It does not grant provider access or promise a checkout sandbox.
Workspace open is explicitly treated as a service-state effect, with local
intent/outcome evidence, bounded waits, no automatic replay and no foreign
cleanup. The production journal and coordinator remain required later.

| Item | I1 treatment | Remaining production/release obligation |
| --- | --- | --- |
| P1-01 | Exact root selection has offline cases; no provider/session reuse exists in I1 | I2 must wire one resolved root and prepare/commit before both new and reused prompts |
| P1-02 | One restricted harness, owned bridges and disposable targets; no broker/shim traffic | Common coordinator, cross-process authority, reentrancy and fairness before production access |
| P1-03 | No provider mutators; open has a local dispatch-intent record and stops on uncertain outcome | Durable shared attempts/holds, dedupe and reconciliation before any production effect, including lifecycle open |
| P1-04 | All 54 upstream schemas captured; implement only internal open/read adapters and observe their real result/error shapes | Closed admitted facade, canonical digests, exact error normalization and handle provenance; Apple build/execution tools remain unavailable |
| P1-05 | Separately report sentinel routing and structured mapping; absent mapping remains unverified, conflicting mapping stops reads | Atomic/non-reassignable authority and execution-time containment remain unestablished; no production admission or Ready claim by successful examples alone |
| P1-06 | No public DTO/API change; redacted experiment evidence only | Versioned projection, privilege enforcement and mixed-version fixtures before production readback |
| P2-01 | Per-attempt experiment record and evidence digests, not a release receipt | Full required checks and reproducible release receipt at I4; Ops owns authorized rollout/retry |

No review item is marked implemented or independently closed by this table.
P1-04's upstream inventory question is resolved; its facade remains work.
P1-05 is explicitly open. Assigning a requirement to a later slice is not
waiving it, and I1 cannot reach production via a bypass flag.

## Historical R3 Repeat Review Request

Review the R3 parent and all four children together, then use the schema study
as supporting evidence. The original pasted review remains the findings baseline;
its old line references and MD5 identify R1, not this revision.

Requested decisions:

1. Is I1 sufficiently bounded and observable to proceed to its implementation
   plan, despite the explicitly unestablished production guarantees?
2. Are routing, structured mapping, permission and security-confinement evidence
   kept distinct, including partial/blocked outcomes and reconnect behavior?
3. Are lifecycle effects and uncertainty constrained without requiring the full
   production journal before a disposable experiment?
4. Do I2-I4 still require the original cross-route, reuse, wire, compatibility
   and release safeguards before corresponding authority is enabled?

Return an I1 planning-readiness verdict separately from full-migration/release
readiness. Give concrete blocking findings against the applicable iteration;
do not require future release-host checks to have passed before implementing
I1. Conversely, do not label the full migration Ready because I1 can begin.
No repeat review has been performed or sent to Ops by this document update.

## Revision 3 Review Identity

Prepared against source HEAD `a096b8831f0db7f17d1ffe426851c49a65b8b000`.
The documents are local, uncommitted artifacts, not part of that source commit.
R3 parent MD5: `e9d6a8dad4bbc2d9ab38cbc4dc88142b`.
MD5 preserves the original review's locator convention; SHA-256 below pins the
whole normative set. Preserve these historical entries after later edits;
record each changed revision in its own identity table instead.
This disposition is not included in its own digest table.

| Artifact | SHA-256 |
| --- | --- |
| [Parent design](../superpowers/specs/2026-09-19-xcode-headless-mcp-design.md) | `7657816a074c7278566325a9286bf52d1f7801b18c9c3eeef8ded8f75cd7115c` |
| [Iteration 1](../superpowers/specs/2026-09-19-xcode-headless-iteration-1.md) | `509ca0ba4d2f97e5b71505092537f15cf1ab67d9af29962371feab59523c4045` |
| [Runtime](../superpowers/specs/2026-09-19-xcode-headless-runtime-contract.md) | `8f2e5a0d915db7e64d3ac364ef8764de086b9d552e15503c0ce5507fba749a5f` |
| [Wire](../superpowers/specs/2026-09-19-xcode-headless-wire-contract.md) | `f0bd17b9a3c86da0e208b0ce7c2b38b534b0b586ea2ad91a9bf779664f1ecd48` |
| [Release](../superpowers/specs/2026-09-19-xcode-headless-release-contract.md) | `90ede76bf7e2a7a513780ef65f0461f210f78cc21ea8b807be7c2ea248dad81a` |

## Validation Boundary

### Historical Revision 4 Identity

R4 parent MD5: `1e40d095607cf9ab8e2bd06485172f2c`.
R3's review verdict does not apply to these changed bytes. Runtime/wire/release
remain unchanged at their R3 hashes; neither new production authority nor
production Ready is claimed.

| Changed artifact | R4 SHA-256 |
| --- | --- |
| Parent | `613438f2d270db3b7526aedd63fb3d491028953b39ed42382e7b7a2f183f4f19` |
| I1 | `45f81e2c8bd6883c13633d66eb0528dd02c73016a11e3524dbe0d80cc5bafcbc` |

### Historical Revision 5 Identity And Validation

R5 parent MD5: `96d35d5f8ed2de41519890e94c3b4731`.
These digests were computed after the R5 parent/I1 and live-evidence edits.
R3/R4 pins above are preserved, not recomputed as current identities. The
runtime/wire/release children remain byte-identical to their R3 versions.
This disposition is excluded from its own digest table; R5 is a documentation
closeout, not an independent production review or new authority grant.

| Artifact | R5 SHA-256 |
| --- | --- |
| [Parent design](../superpowers/specs/2026-09-19-xcode-headless-mcp-design.md) | `c4eae56d7f0e8e088c8116f2bd20c91b58bedd3ddd02d8e3f931d0eb8e732699` |
| [Iteration 1](../superpowers/specs/2026-09-19-xcode-headless-iteration-1.md) | `97acc94ed56bd2e4542013cc0d20879a89821d96830bac2ed3da8d4603870e44` |
| [Runtime (unchanged)](../superpowers/specs/2026-09-19-xcode-headless-runtime-contract.md) | `8f2e5a0d915db7e64d3ac364ef8764de086b9d552e15503c0ce5507fba749a5f` |
| [Wire (unchanged)](../superpowers/specs/2026-09-19-xcode-headless-wire-contract.md) | `f0bd17b9a3c86da0e208b0ce7c2b38b534b0b586ea2ad91a9bf779664f1ecd48` |
| [Release (unchanged)](../superpowers/specs/2026-09-19-xcode-headless-release-contract.md) | `90ede76bf7e2a7a513780ef65f0461f210f78cc21ea8b807be7c2ea248dad81a` |
| [I1 live closeout](xcode-headless-i1-live-2026-09-19.md) | `594177cf0627c51712bd14127311d41d18c0df5eab201e9ef9cd75478323d3c8` |

Static checks cover local link targets, the under-2,000-line budget for each
of the five specifications, both retained report/manifest pairs, saved test
counts/digests, and the I1 575-input source identity at the initial checkpoint.
Final validation observed subsequent parallel source/ledger changes; those
were not edited or reverted here, and the newer tree is not certified by I1.
The original I1 plan, generated attempts and older evidence remain unchanged.
External web links and the live host were not probed. No product tests were
rerun for this closeout; prior executor observations are not independent reruns.

### Revision 6 Offline Status Identity

R6 updates only the parent's implementation-status paragraph and links the
separate offline checkpoint. It does not change production admission contracts,
reclassify I1 evidence or claim that production integration is complete.
The standalone journal is now implemented rather than pending. Its independent
component review and regression results are recorded in the checkpoint.

R6 parent MD5: `638af5a8029853bc3e35d938d6d838d5`.
R6 parent SHA-256: `57a06875b1593535eabbfd1638e1f08d6f50aea5f07d4779fc67df8beb7a42bb`.
I1 and its live closeout retain their R5 hashes; runtime/wire/release retain
their R3 hashes. Current implementation inputs are separately pinned in the
[source manifest](xcode-headless-offline-source-2026-09-19.json).
The [component review](xcode-headless-offline-review-2026-09-19.md) is not a
full proposal-readiness review of R6 or production release approval.

### Historical Preparation

The initial revision-2 response used file/hash reads and static source
inspection. Only Markdown was edited then; product tests and runtime probes
were not run. The later user-authorized schema study used metadata-only bridge
connections and saved their raw JSON evidence as documented separately.
Xcode IDE was not opened, and no service/workspace/permission mutation occurred.

R3 preparation is also documentation-only: source baseline and dirty state are
checked, local links/scope budgets and captured-evidence integrity are validated,
and internal iteration/requirement consistency is self-reviewed. No product
tests, new runtime probes, workspace opens or permission changes are part of
this preparation. A documentation self-check is not independent proposal review.

The revised contract set needs review before implementation planning. Any
probe that opens a disposable project or initiates permission approval requires
its own explicit authorization; no deployment or run retry follows from this
document update.
