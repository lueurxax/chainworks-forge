# Headless Release Evidence Contract

Revision 4 (R4), 2026-09-20. Normative production-release child of the
[headless design](2026-09-19-xcode-headless-mcp-design.md).
Owns P2-01 and acceptance of the runtime/wire contracts. Nothing in this
document authorizes a live probe, permission grant, installation or run retry.
R4 records R8's September 20 explicit approval of `trusted_local_project_v1`
and full implementation, not a release pass or completed migration.

## Iteration Applicability

This is the I4 full-migration release gate, not a prerequisite for implementing
or closing [I1](2026-09-19-xcode-headless-iteration-1.md). I1 produces an experiment
record with per-case outcomes and limitations, not this receipt or a release
pass. Cold start, packaged launch/consent, production recovery and full workflow
coverage remain later obligations. Their absence must not be misreported as
I1 failure, but it still prevents a full-migration Ready/release claim.

The original review findings remain tracked across iterations. Moving a check
later changes scheduling, not its result. The user explicitly approved the
trusted-project boundary on September 20: release now requires tested admission
and scope controls under that model, not an imaginary Apple checkout-sandbox
proof. Further changes to required capabilities or that boundary still require
explicit review. Historical I1 contracts and evidence remain unchanged.

## Required Artifact

Every proposed release has a redacted JSON receipt at
`docs/evidence/xcode-headless/<release-id>/release-receipt.v1.json` and a manifest
of referenced evidence files with SHA-256 digests. Release ID is a source/build
identity, not a mutable `latest` pointer. The normative receipt schema belongs
to the same `control-plane/contracts/xcode-headless/v1/` bundle as the wire DTOs.
No release receipt has been generated for this specification revision.

Receipt object has exactly these fields; all are required:

| Field | Type / contents |
| --- | --- |
| `schema_version` | integer, exactly 1 |
| `release_id` | nonempty bounded string |
| `source_commit` | exact source commit; dirty source requires separate content digest and cannot masquerade as that commit |
| `source_content_digest` | canonical source artifact digest |
| `generated_at` | UTC RFC3339 timestamp |
| `app` | version, build, bundle ID, signing team, designated-requirement digest, artifact digest |
| `daemon` | version/build SHA, artifact digest, signing requirement digest, DB schema version |
| `xcode` | version/build, selected toolchain identity digest, service bundle ID/build |
| `launch_identity` | packaged/development mode, UID pseudonym, daemon/bridge executable digests and signing requirement digests |
| `service_generation` | redacted boot/start identities, PID, bundle/executable identity digests |
| `contract` | manifest, schema bundle, effective policy and binding digests; `trust_policy_id=trusted_local_project_v1` and `trust_policy_digest` |
| `consent` | `approved\|denied\|unknown`, actual principal and folder-scope digests, evidence reference |
| `capabilities` | required workflow capability rows and their admitted route/evidence |
| `checks` | exactly the named check rows below |
| `unresolved_attempt_count` | nonnegative integer from the durable journal |
| `evidence_manifest` | relative evidence paths and content digests, no external mutable URLs |
| `verdict` | `pass\|hold`; computed, never a manual substitute for failed checks |
| `receipt_digest` | canonical receipt digest, excluding this field itself |

All nested fields in the table are required strings unless a type is stated.
Digests are 64 lowercase hex. Unknown facts use a check with `unknown`, not an
invented value such as `n/a` in a mandatory identity. Raw filesystem paths,
bearer tokens, signing private material and Apple payloads never enter the
release artifact. Use stable receipt-local project/host labels with digests of
the validated identities; privileged local readback retains the actual paths.

## Check Rows And Hold Rule

Each check row has exactly `status`, `observed_at`, `evidence_refs`, `details`
and `source_identity_digest`. Status is `pass|fail|unknown|not_run`.
Details are a bounded redacted explanation, not executable recovery instructions.
A check cannot pass without at least one matching evidence reference.

Required check names:

| Check | Required evidence |
| --- | --- |
| `ide_absent` | IDE absence before and after real broker acceptance; service presence is distinct |
| `packaged_launch_identity` | Actual packaged daemon/bridge signing and launch chain, not development-shell approval |
| `consent_scope` | Agent and exact project folder allowed, unsafe global permission disabled, no automatic grants; project trust is not Apple consent |
| `cold_start` | Controlled-host start with no foreign clients stopped; exact selected service acquired |
| `warm_attach` | Running service connection without IDE or service restart |
| `project_binding` | Initial structured open result supplies exact canonical requested path and ID; same pinned service generation and fresh structured status confirm exact path still open before explicit-ID scoped read through actual broker lease |
| `foreign_project_rejection` | Foreign explicit selectors/handles/paths are rejected through the first lease; no claim that Apple cannot follow trusted project's external references |
| `trusted_project_admission` | Explicit operator local-project trust tied to pinned root/project/UID, installation, service generation and initial mapping; matching trust-policy ID/digest and evaluated permission/tool allowlist; absent/revoked/mismatched authority denied |
| `scope_controls` | Closed tested adapters, explicit bound IDs, path/handle rules, fresh observable drift checks, denial of unknown tools/default/prose mapping and no close/reopen broker tools; documented blind interval is not atomic mapping or sandbox proof |
| `reuse_revalidation` | Stale project/service/policy blocked before reused prompt transmission |
| `broker_shim_exclusion` | Both Chainworks routes use one cooperative coordinator; concurrent effect dispatch rejected/queued, without claiming exclusion of external Apple clients |
| `restart_recovery` | Stale leases invalidated; dispatched uncertainty restored; zero automatic replay |
| `effect_dedupe` | Repeated nonce and new key under unresolved hold produce no second effect |
| `public_redaction` | Unauthenticated health and error paths contain no project/generation/attempt detail |
| `mixed_versions` | Old/new Swift/daemon and future enum/version cases match the wire contract |
| `canonical_gates` | Required repository gates and remote-only UI policy respected |
| `required_capabilities` | Every frozen workflow-required Xcode capability has an admitted, tested headless route |

Capability row fields are `capability`, `required_by`, `route`, `status`,
`evidence_refs`. Route is `headless_mcp|canonical_gate|existing_filesystem`.
An unrelated filesystem tool is not a substitute for a required Xcode semantic
operation. Determine required capabilities from actual frozen intent and
workflow proof obligations, not from whatever tools happen to remain enabled.

The validator emits `pass` only when every required field is valid, all check
statuses are pass, all capabilities are covered, consent is approved, artifact
digests match, no unresolved effect exists and sources/identities match the
candidate release. Missing evidence, an unadmitted required tool, unknown,
not-run, stale identity, dev-only consent or a failed check produces `hold`.
There is no skip/waive flag that turns these into success.

Fixture checks and installed-host observations are different evidence types.
A fixture cannot stand in for closed-IDE project access or packaged consent;
a live handshake cannot stand in for the concurrency/fault-injection suite.

The parent records a user-supplied September 20 native status observation, not
a new release evidence file: `openWorkspaces` has `path`, `displayName` and
`activeSchemeName`, but no workspace ID. Acceptance must test the supported
initial-open mapping plus fresh path-presence checks, not demand an unsupported
complete ID/path re-query. Absolute-path selectors failed the earlier native
read checkpoint and are not an alternative to explicit IDs. Observed
disappearance/restart invalidates admission; external close/reopen between
probes or hostile remap can remain undetected under the accepted trust model.

Workspace references and authorized build scripts may access outside checkout.
Ordinary path checks are routing/accident guards, not security confinement;
malicious concurrent filesystem changes and external Apple clients are not
sandboxed. Tests must state these limits, not count them as confinement passes.
This does not relax journal/no-resend rules, evaluated permissions, canonical
gates or remote-only UI policy. Every required capability still needs actual
route acceptance; disabled tools cannot hide missing workflow coverage.

## Reproducibility And Fault Boundaries

Record exact source artifact, test gate selection, toolchain and start/end
times with exit/results. Keep ordinary commands and opaque values out of public
messages when they contain host details; store redacted structured evidence.
The receipt digest uses `cw.xcode.receipt.v1` from the wire contract. Validate
referenced file digests and prohibit evidence paths escaping the receipt folder.

Fault-injection evidence records effect counters and journal transitions at:
prepared commit, dispatch fence, host write, terminal response, outcome commit,
response delivery, cancellation, permit release and daemon restart. A pass
must establish zero unintended second dispatch, not merely an expected error.

Test the validator with a missing check, a removed field, modified evidence,
future schema version, dev approval substituted for packaged approval, one
unresolved attempt, forged manual pass, wrong source and redaction violations.
Also reject missing/mismatched trust-policy fields or admission evidence,
path-only status presented as initial ID/path proof, and scope-control claims
unsupported by tested adapters. Do not require a sandbox-proof field to pass
this trusted-project contract or relabel an old receipt as an R4 result.
No host mutation is needed to test receipt validation.

## Release And Recovery Ownership

Ops runs the authorized packaged-host lane and owns publication, installation
and any subsequent run retry. The local implementation lane supplies immutable
source/test artifacts and an incomplete receipt until packaged checks finish.
A successful deployment is recorded separately from a successful workflow retry.

Journal migration and authority compatibility run before accepting leases.
If upgrade fails, broker and effectful shim remain unavailable; other daemon
health is reported independently. Never open the IDE to recover headless setup.
Downgrade cannot erase attempt history or enable an older unfenced effect route;
unsupported DB/contract versions require a forward fix. Service/workspace
cleanup is separately authorized and never a receipt-completion shortcut.

## Review State

The contract defines required release evidence, not completed checks. R8's
trust-model decision is approved and P1-04's exact-schema study is complete as
`upstream54`; the admitted facade and actual production route acceptance remain
incomplete. I1 stays a separate historical experiment, not release evidence for
newly implemented routes. Full production/release readiness still requires
tested trusted-project admission, scope controls, packaged launch identity and
consent, cold start, recovery, canonical gates and required capability coverage.
No release pass is claimed until those actual checks and receipt validation pass.
