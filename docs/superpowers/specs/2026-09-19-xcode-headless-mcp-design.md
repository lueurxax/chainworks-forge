# Managed Headless Xcode MCP

Date: 2026-09-19. Source baseline: `a096b883`.
Status: revision 8 (R8), 2026-09-20, approved trusted-local-project migration.
H1 is Supported for the tested ordinary-routing scenario only.
Revision 3 was accepted for I1 planning; R3/R4 hashes remain historical.
This is not production admission or a full-migration/release Ready verdict.
Reviewed revision 1 MD5: `de96080e5c2400f1eb23fa10811917d3`.
The isolated I1 harness is implemented and tested. The production design below
remains normative; the later implementation is documented in the
[headless runtime reference](../../reference/xcode-headless-runtime.md).
The dated component checkpoints below describe their own source state, not the
final integration. This spec does not enable production admission or change the
installed daemon. Earlier offline dependency work follows a
[separate foundations plan](../plans/2026-09-19-xcode-headless-offline-foundations.md):
at that checkpoint, production source had shared root-selection changes and a
standalone durable effect journal; headless dispatch was not enabled. The
[offline checkpoint](../../evidence/xcode-headless-offline-2026-09-19.md)
distinguishes wired consumers from standalone components and records verification.
This is not full I2; its source identity and verification are separate from the
final I1 run. R8 explicitly revises prospective admission requirements together
with runtime/wire/release R4; historical I1 contracts and evidence stay unchanged.

The [September 20 continuation](../../evidence/xcode-headless-journal-integration-2026-09-20.md)
adds the fixed-scope engine journal adapter and pre-admission startup recovery.
Native Codex reads returned the correct A/B sentinels with explicit IDs after
operator consent, but rejected absolute-path selectors. This is a bounded
compatibility observation, not a production binding or isolation guarantee.
The new integration passed its focused tests and independent source review;
broader baseline failures are retained in the checkpoint rather than hidden.
R7 left the trust model pending. On 2026-09-20 the user explicitly approved the
trusted-local-project model and full migration implementation. R8 records that
decision, not an implementation-complete, live-acceptance or release verdict.

The [trusted-runtime checkpoint](../../evidence/xcode-headless-trusted-runtime-2026-09-20.md)
adds native host/project inspection, the closed open/read contract and immutable
trust-policy bundle, journal-driven workspace preparation, and a dedicated
deadline-bound stdio transport. At that checkpoint these components were not the
production broker/session/coordinator path. Its evidence is retained separately
from both the earlier journal checkpoint and the later integrated implementation.

The [completion plan](../plans/2026-09-20-xcode-headless-completion.md) joins the
headless controller, common coordinator, durable effects, preparation before
session policy, invocation-bound broker/shim routes, explicit operator controls
and one-shot cold startup. Offline proof and actual packaged-host acceptance
remain distinct. The old IDE-dependent backend is not a production fallback.
Trust mutation is project-global and therefore requires global administration;
diagnostics and reconciliation retain their explicit run scopes. Reads and
binding revalidation have a 60-second operation limit, preparation a 720-second
limit, and an invocation a fixed maximum lifetime of 24 hours without renewal.
Release validation is implemented separately from collecting real host evidence;
it cannot turn missing packaged consent or production cutover into a pass.
The [final implementation record](../../evidence/xcode-headless-completion-2026-09-20.md)
pins this integrated source and its verification, including baseline failures and
the separate installed-host acceptance boundary.

## Approved Trust Decision

The production target is `trusted_local_project_v1`, not an Apple-enforced
adversarial-checkout sandbox. Admission requires an explicit operator decision
to trust the selected local project, pinned canonical root/project identities,
UID, selected installation, service generation and initial structured workspace
ID/path mapping. Model approval alone is neither per-project trust admission
nor an Apple permission grant. Record `trust_policy_id` and its immutable
`trust_policy_digest` alongside evaluated permissions and the tool allowlist.

Before scoped dispatch, check observable drift: the pinned service generation,
root/project identities, fresh structured status showing the exact project path
still open, and current trust/permission/tool policy. Status need not supply an
ID-to-path re-query that Apple does not expose. Always send the explicit bound
workspace ID upstream; neither a server default nor workspace-list prose is
mapping authority. Unknown tools stay denied until a closed, tested adapter
exists. There are no automatic permission grants.

Apple workspace references and build scripts may access outside the checkout.
Ordinary path checks guard routing and accidental scope mistakes; they are not
security confinement. Malicious concurrent filesystem changes, external Apple
clients and close/reopen or hostile remap between observations are not sandboxed.
The coordinator lock is cooperative among Chainworks clients, not an external
Apple lock. The user accepted these limits; R8 does not claim atomic mapping,
non-reassignable IDs or race-resistant checkout confinement.

Mutators still require durable journaling, no automatic resend and uncertainty
holds. Evaluated permission restrictions, canonical gates and remote-only UI
policy are unchanged. Implementation is authorized; this documentation task
does not authorize commits, deployment or live host operations.

## Intent And Boundaries

Chainworks must perform its Xcode MCP work with Xcode IDE closed. The daemon
manages connection to Xcode Service, opens the exact project for the execution's
checkout, and exposes only workspace-bound operations through existing leases.
An installed, selected Xcode toolchain and its SDKs remain prerequisites.

Success is a verified project binding and authorized useful operation with the
IDE absent, not merely process discovery or a successful MCP handshake.

The migration replaces the IDE-dependent production path completely. It does
not preserve IDE fallback, AppleScript discovery, window/tab selection, or the
single-unidentified-process fallback. Unsupported headless installations fail
with an actionable reason instead of opening the IDE.

This scope preserves provider fake-home isolation, the HTTP lease facade,
frozen workflow/catalog semantics, existing permission restrictions, and the
repository's test-gate and remote-only UI-test policies. It does not redesign
ACP, implement a general OS sandbox, or expose every new Apple tool by default.
No live deployment, restart, run retry, snapshot edit, commit or push is
authorized by this specification. Those remain separate Ops actions.

## Contract Scope And Precedence

This parent owns intent, iteration boundaries and dependency order. The first
iteration child governs the isolated I1 experiment. Runtime, wire and release
children govern the eventual production migration, not prerequisites for
implementing that experiment. Within those boundaries their specific rules
govern the corresponding summaries below. An experimental exception cannot
authorize a production lease or weaken a later admission requirement.

| Contract | Scope / review items |
| --- | --- |
| [First iteration](2026-09-19-xcode-headless-iteration-1.md) | Closed I1 experiment, scenario-limited H1 support, retained deviations and evidence composition |
| [Runtime and effects](2026-09-19-xcode-headless-runtime-contract.md) | Preparation before reuse, common coordinator, durable dispatch/outcomes, trusted-project admission and observable drift; P1-01/02/03/05 |
| [Wire and readback](2026-09-19-xcode-headless-wire-contract.md) | Sole manifest, digests, error matrix, effect API, handles, privilege and mixed versions; P1-04/06 |
| [Release evidence](2026-09-19-xcode-headless-release-contract.md) | Reproducible receipt, required capabilities and fail-closed hold; P2-01 |

[Review disposition](../../evidence/xcode-headless-review-response-2026-09-19.md)
separates addressed design gaps from unresolved installed-contract facts.
These children are one migration contract set, not permission to add unrelated
features. The advertised Apple schemas are captured. Missing required admission
or route evidence cannot be filled by prose or by declaring affected capabilities
optional.

## Evidence And Current Gaps

The [I1 closeout](../../evidence/xcode-headless-i1-live-2026-09-19.md) composes
37 passing harness tests (351 total selected ACP/example tests, one existing
ignored) with attempt `f39670ec-af8f-4032-bda7-73f04d419f54`: two opens,
structured A/B mappings and explicit A/B/A reads on each of two connections,
without reopening after reconnect. IDE absence and stable service generation
were observed at every prescribed check. The runtime report remains `Partial`
because its two offline cases are composed only at closeout; all live cases pass.

Earlier blocked opens and the historical `fea2590b` read-oracle rejection remain
unchanged. Apple's two-line representation refuted the one-line expectation,
not cross-project routing. Adaptations cover nested permission fields, complete
private transport logging, native-target fixtures and an exact two-line oracle.
Unknown effects from the earlier attempts are retained, never automatically
replayed or cleaned up. This is experimental routing evidence, not confinement.

[Contract research](../../evidence/xcode-headless-mcp-research-2026-09-19.md)
records the installed Xcode 27.0 build `27A266a`, selected developer directory,
official release history, discarded IDE prototype, and read-only probes.

With the IDE closed, the independently running `com.apple.dt.mcp-server` host
completed MCP initialization and exposed 54 tools. Its `XcodeListWorkspaces`
call returned `isError: true` because the diagnostic agent was not approved.
Those earlier research probes opened no project and granted no permission.

The later [metadata-only contract study](../../evidence/xcode-headless-contract-2026-09-19/README.md)
captured all 54 input/output schemas and negotiated MCP 2025-06-18 when offered
a newer protocol. The earlier 2024-11-05 result was negotiation with that older
offer. P1-04 exact upstream schema study is complete as `upstream54`. The closed
facade is now implemented; live trusted-project route acceptance remains separate
and unverified by the offline completion pass.

Observed headless contracts:

| Surface | Observed behavior |
| --- | --- |
| `mcp-server` | Status, project opening, and separate permission administration |
| `mcpbridge` | Stdio JSON-RPC; explicit host selection through `MCP_XCODE_PID` |
| `XcodeOpenWorkspace` | Absolute project/workspace path; required returned identifier, optional returned path |
| `XcodeListWorkspaces` | Permission-gated, human-readable message output |
| `XcodeCloseWorkspace` | Workspace identifier argument |
| `XcodeRead`, `BuildProject` | Optional workspace identifier; server defaults must not reach provider calls |

Warm fixture opening, structured returned ID/path mapping, two-project routing
and reconnect reads were observed in I1. Cold start, packaged-daemon permission
identity and actual broker-route acceptance remain unverified. Atomic mapping,
non-reusable IDs and execution-time confinement are not established and are not
prerequisites under the approved trusted-project model.

The user supplied a fresh read-only native `mcp-server status` observation on
2026-09-20: `openWorkspaces` contains `{path, displayName, activeSchemeName}`,
with no `workspaceIdentifier`. This is design rationale from that observation,
not a new evidence file or a probe performed by this documentation revision.
Initial structured open must provide the exact canonical path and ID; fresh
status can subsequently confirm only that the exact path remains open under the
same pinned service generation. It cannot prove the ID still denotes that path
after an unobserved external close/reopen. Absolute-path selectors failed the
native read at the previous checkpoint; use the explicit returned ID, not a
path-selector fallback or prose-derived mapping.
Apple's [Xcode 27 release notes](https://developer.apple.com/documentation/xcode-release-notes/xcode-27-release-notes)
describe the headless preview and signed-agent/directory permissions; runtime
behavior must still be checked against the selected installed toolchain.

Current code gaps at the source baseline:

- `xcode_target.rs` discovers IDE candidates through command strings and open
  document hints, with an unidentified single-process fallback.
- `xcode_broker.rs` actually shares bridges by PID and developer directory,
  including across runs, and retains idle bridges after the last lease.
  The reference document's per-run/last-release claims are stale.
- Warmup ends after initialize and tools discovery. Policy filters tool names,
  not workspace arguments; a missing tool allowlist currently allows all tools.
- `executor.rs` derives an effective working directory using worktree strategy,
  but attaches `run.workspace_root` to Xcode MCP. The adapter's execution-root
  helper separately checks only `worktree_write_enabled`.
- Existing health cannot establish project access, and cached host probe data
  is insufficient to establish that a service generation is still alive.

## Research Conclusions And Working Hypotheses

The available primary documentation, installed help and schema captures have
answered the discovery questions; repeating that search is not a prerequisite
for I1. The minimal implementation has now supplied the bounded observations
above; R8 approves continuation under the explicit trusted-project contract.
This is not a claim that no other information exists: incorporate later Apple
documentation, new versions and experiment results when they change a decision.

| Evidence level | Conclusion / consequence |
| --- | --- |
| Observed | The installed headless service initialized and advertised 54 complete input/output schemas with the IDE absent; metadata availability is not project access |
| Observed | A newer protocol offer negotiated 2025-06-18; 2024-11-05 was also negotiated, with the same tool definitions |
| Observed in I1 | Open returned distinct IDs and matching structured paths; explicit A/B/A reads returned correct sentinels before and after bridge reconnect, with the same IDs and no reopen |
| Advertised beyond the exercised subset | Workspace listing returns prose; many tools default an omitted workspace identifier; I1 did not test that default or expose it to providers |
| Documented | Apple describes signed-agent/folder permissions and workspace-reference access; this does not establish Chainworks checkout-only confinement |
| Unverified live | Production broker/provider integration, cold start and packaged consent |
| Observed on 2026-09-20, user-supplied | Native structured status confirms open paths but supplies no workspace IDs; it cannot refresh an ID/path association |
| Accepted limitation in R8 | Workspace-ID non-reuse, atomic mapping validation, execution-time containment and isolation from external Apple clients are not guaranteed |

**H1:** with Xcode IDE closed, a minimal Chainworks headless controller can open
the exact project selected from an execution root, obtain an identifier and read
that project's known content using explicit scope. Repeat with a second project
with the same relative names but different content, then reconnect the bridge.

The loop is hypothesis and observable criteria -> minimal implementation ->
fixture and authorized live tests -> recorded expected/actual deviations ->
implementation/specification adjustment -> next iteration. Offline regression
tests accompany implementation; live contract checks follow the completed
minimal slice. Do not require the answer before implementing the experiment.

The first iteration exposes no provider tools and uses only disposable,
known-content projects. It tests routing, not a security sandbox. A successful
read cannot close P1-05 or establish universal isolation. R8 makes the explicit
scope/trust decision: production must prove the trusted-project admission and
scope controls below, not claim the stronger guarantees that I1 never tested.

Low change cost applies to this isolated development slice, not automatically
to source writes, database migration, permissions, deployment or existing runs.
The evidence register originally recorded H1 `not_run`; that historical baseline
is retained. R5 composes offline root/fault checks with the latest live evidence
as H1 Supported for this ordinary-routing scenario. Metadata alone did not
establish H1, and this experiment does not implement the production migration.

## Selected Approach

Use a daemon-owned headless controller behind the existing broker. An
attach-only controller would leave project startup and recovery to the user;
dual IDE/headless support would retain the ambiguity the user wants removed.
Managed headless-only operation is the approved direction.

The controller owns host discovery, readiness, and project binding. The broker
owns leases, protocol forwarding, scope enforcement, queueing and observations.
The engine owns execution intent and the effective checkout. Providers receive
neither service administration authority nor unbound project selection.

## Effective Checkout And Project Selection

This section through Ownership Map specifies the production target. I1 applies
only the explicitly selected subset in its child contract; it does not construct
a production `VerifiedWorkspaceBinding` or turn on an experimental bypass.

One typed `ResolvedExecutionRoot` must feed provider cwd, shim context and
headless binding. Resolve it from the existing frozen effective worktree
strategy, including read-only tasks assigned to a shared implementation
worktree. Do not equate read-only permission with use of the repository root.

Preserve the existing legacy/default strategy behavior. An execution explicitly
requiring a dedicated/shared worktree must not silently bind the repository
project when that worktree is missing. Report the missing checkout before
starting a provider or opening a project.

Project selection is deterministic:

1. Canonicalize and validate the effective execution root.
2. Resolve an explicit project/workspace selection relative to that root.
3. Without a selection, consider only immediate `.xcodeproj`/`.xcworkspace`
   children. Exactly one candidate is required. Do not recursively search
   generated directories, other run worktrees, or the operator's home.
4. Zero candidates means project-not-found; multiple candidates require an
   explicit selection. There is no first-result or workspace-over-project guess.
5. Canonicalize the chosen package and require containment in the effective
   root. Recheck identity when acquiring/rebinding a lease.

Legacy `workspace:<relative-path>` selection may resolve under that root.
An absolute repository project hint may be mapped to a worktree only by its
exact relative path under the known original repository root, with existence
and containment checks at the destination. Record both paths. Other outside-root
hints are rejected. A registry hint is never authority to change the execution
root or permission profile.

Explicit tool path arguments must satisfy the execution's evaluated filesystem
rules; unsupported external arguments are denied. This is a routing/accident
guard, not confinement of Apple's resolver. Linked workspace references,
dependencies and authorized build scripts may reach outside the checkout.
Operator project trust accepts that behavior without granting new tool or
filesystem permissions or admitting otherwise forbidden build/test tools.

## Host Lifecycle And Permissions

Resolve the selected developer installation once for an acquisition attempt,
honoring the daemon's explicit developer-directory configuration or the selected
toolchain. Use its utilities and SDK context consistently for the whole binding.

Identify Xcode Service by the expected UID, canonical installation, bundle
identity, independently observed executable path, and process start identity.
Do not trust `ps` command text or `argv[0]` as executable identity. Ambiguous,
wrong-user, wrong-installation, or unsupported hosts fail closed.

Acquisition proceeds through observable states:

`resolving -> service_ready -> bridge_ready -> workspace_binding -> project_ready`

Any stage may transition to `action_required`, `failed`, or `cancelled`.

- Read structured service status first. Disabled access requires operator action.
  An unsafe global allow-all configuration is not an acceptable readiness state.
- If the service is absent and supported access is enabled, use the selected
  toolchain's documented project-open utility to start it with the exact project
  path. Verify the resulting service identity before bridging to it.
- If the service is already running, connect directly and use broker-owned
  `XcodeOpenWorkspace` with the same exact path. Neither path opens Xcode IDE.
- Spawn `mcpbridge` in the existing cleared host-user environment, preserving
  required HOME/TMPDIR/developer context and pinning the verified service PID.
  Omit `MCP_XCODE_SESSION_ID` initially; it is not an isolation guarantee.
- Initialize, discover and validate capabilities, open/bind the project, verify
  its identity, and perform an allowed scoped read before admitting the provider.

Opening can initiate Apple's agent/folder consent. Report that requirement with
the selected project and actual launch identity. Preserve the existing bounded
long warmup budget and early action-required visibility; do not spend the
provider's startup timeout waiting for consent. Explicit denial ends acquisition.
An unresolved consent wait expires without automatic reopen/retry loops.

Never invoke sudo, enable/approve/allow-folder, unsafe allow-all, permission
reset, service stop, or IDE launch automatically. Operator consent remains a
separate OS authorization. A development probe's approval does not establish
approval for the signed packaged daemon and its actual bridge launch chain.

## Verified Workspace Binding

The immutable lease binding contains:

- effective root and canonical project/workspace path;
- service identity and generation, selected developer directory;
- returned workspace identifier and initial structured open ID/path mapping;
- run/execution provenance, operator project-trust admission, permission and
  tool-policy digests, `trust_policy_id=trusted_local_project_v1` and
  `trust_policy_digest`;
- headless contract epoch and validated capability/schema digest.

Check both JSON-RPC errors and MCP `isError`. A successful outer response is not
evidence of a successful open/read operation.

Require the initial structured open result to contain a nonempty workspace ID
and a `workspacePath` canonically equal to the requested path. A missing or
mismatched value returns `workspace_identity_unverifiable`. Human-readable list
prose, an opaque ID alone, an echoed request or path-only status cannot establish
that initial association. I1 observations remain experimental, not live tickets.

Retain that initial association while pinning the service generation. Before
each scoped dispatch, fresh structured status must confirm the exact canonical
path is still open, and root/project identity and policy checks must still pass.
Missing/unreadable status, observed disappearance, restart or other observable
drift invalidates the binding. Do not demand a complete current ID/path re-query:
the observed status has no ID. This checks continuity only to the extent
observable; external close/reopen between probes or hostile remap can escape
detection and is an accepted trust-model risk, not an atomic mapping proof.
Do not expose close/reopen broker tools or use close/reopen to validate mapping.

Preparation precedes engine session policy and fingerprint decisions on every
invocation, including reuse. The manager requires a committed, revalidated
ticket before either `session/new` or `session/prompt`. The runtime child defines
prepare/decide/commit ownership, borrowing existing endpoints and crash cleanup.

## Lease, Backend And Shared State

Keep the existing bearer-protected HTTP endpoint and bounded lease queue.
Authentication may establish provider connection, but tool forwarding requires
an immutable project-ready binding in addition to the active lease.

The new backend key is run ID, canonical project, service generation, selected
developer directory, and effective policy/contract digest. Do not reuse an
initialized backend across runs or incompatible permissions. Sibling leases
with the same key may share initialization and the ordered request pump.

Bounded idle reuse remains allowed for that exact key. Idle expiry releases
only the broker-owned bridge. Do not claim one consent prompt per run; Apple
owns permission reuse and the packaged identity determines its behavior.

One `WorkspaceAccessCoordinator`, shared by broker and shim, owns access.
A per-UID singleton plus durable DB-authority binding covers different-DB
daemons; the existing per-DB lock alone does not. Service startup is
single-flight per installation and UID; opening/binding is single-flight for a
service/project pair. Coordinate operations across all broker backends:

- Read-only leases may coexist for the same project.
- A lease authorized for state-changing operations takes exclusive access to
  that project for its active prompt and outstanding operations. Idle sessions
  hold no permit. Other incompatible leases use the bounded fair queue and
  report `workspace_busy` on expiry.
- The same coordination covers accepted shim commands targeting that project.
  A command that cannot be associated with an authorized root is not granted
  unscoped access as a workaround. A command belonging to a lease reuses its
  project permit rather than waiting for a second conflicting permit.

Validate scheme/destination/test-plan context for tools that depend on it.
Do not infer per-run isolation of Apple's global workspace state from separate
bridge PIDs. This lock coordinates Chainworks clients only; it neither locks out
external Apple clients nor prevents their unobserved workspace changes. Tools
without a closed, tested scoped adapter remain unavailable.
Cancellation or disconnect does not release a project into reuse until an
in-flight effect is finished or protected by a durable uncertainty hold.

## Provider-Facing Tool Contract

Introduce the domain-owned, versioned headless contract bundle. It classifies
each supported tool by effect, workspace/path arguments, output identifiers and
required schema. Unclassified tools and incompatible schemas are unavailable,
even if Apple advertises them. Empty registry allowlists no longer mean access
to arbitrary future Apple tools.

The effective surface is the intersection of requested Xcode capability,
resolved execution permissions, registry tool restrictions, and this manifest.
Do not repurpose legacy catalog MCP flags as a new permission grant. Unknown
permission profiles or unresolved policy hashes fail closed. Read-only profiles
must not gain source writes, code execution, build/test or device control just
because a tool can be addressed to their workspace.

| Tool category | Required handling |
| --- | --- |
| `XcodeRead`, search, file/project metadata | Inject bound workspace; enforce permitted paths and bounded discovery |
| File/project edits | Explicit write authority; validate all source/destination paths and effects |
| Build/test/run/preview/snippet tools | Unavailable for this repo as specified in the wire contract; use canonical headless gate routes |
| Scheme/destination/test-plan changes | Exclusive workspace lease and validated scoped schema |
| Documentation/template discovery | Allow only explicitly classified workspace-independent tools |
| Debug/device/session tools | Require verified chain of origin from the bound workspace; otherwise deny |
| Workspace open | Broker lifecycle only; never directly forward provider calls |
| Workspace close/new-project | Unavailable to providers; no automatic cleanup or project creation |
| Workspace listing | Return only the verified lease workspace through a tested scoped facade |

For every workspace-scoped call, inject the bound workspace alias if omitted.
Accept a supplied alias only if it denotes the exact binding, then translate
internally to an explicit upstream identifier on every call. Reject another
project, obsolete identifier, IDE-only tab argument, null/default selection,
and unsupported argument forms.
Never strip an invalid selector and fall through to Apple's global default.

Check path traversal, containment and deny rules for every path argument, plus
the trusted-project admission and observable continuity checks before dispatch.
These checks cannot close a race with another process. R8 does not require
executor-side checkout confinement, non-reassignable IDs or atomic binding
validation; it requires tested adapters, explicit scope, evaluated authorization
and honest recording of the trusted-project limits defined by the runtime child.

Enforce authorization and normalization in the common forwarding boundary;
HTTP routing, cached tool lists and warmup helpers cannot bypass it. Broker
lifecycle calls use a separate internal interface that providers cannot invoke.
Reject unclassified MCP methods instead of transparently forwarding new resource
or administration surfaces. Advertised capabilities must match the facade.

Preserve JSON-RPC ID remapping and bounded response handling. Correlate activity
notifications to the originating request without mixing sibling observations.
On tool-list/schema change, invalidate the capability digest and revalidate
before the next call. A changed accepted digest requires a fresh binding, not
in-place mutation of a live lease. Do not silently skip relevant notifications
or interpret them as responses. This scope does not require a general HTTP
SSE redesign.

Keep direct `mcpbridge` denied through the shim and deny service-administration
commands there. Existing authorized `xcodebuild`/`simctl` routes remain headless
routes, not IDE fallback. For this repository, builds/tests still funnel through
`scripts/test-gate.sh`; raw Apple build/test tools are unavailable, not
transparently forwarded. The wire child fixes the exact scope and error rules.

## Failure, Restart And Cleanup

Validate live service generation, exact open-path status and pinned root/project
identity before every scoped dispatch; cached discovery is not sufficient. PID
reuse, observed same-generation remapping/close, toolchain drift, workspace
disappearance, trust/permission revocation or incompatible schema revokes
affected bindings and leases. Path-only status cannot detect every same-generation
remap or external close/reopen between observations.
Reacquisition must repeat readiness and binding checks before provider reuse.

Every effect requires a durable prepared attempt and dispatch fence shared by
MCP and shim. Repeated operation keys/nonces dedupe; uncertain effects create
restart-safe project holds that also reject new nonces. Only evidence-backed
reconciliation permits further effects; this is not an exactly-once guarantee.
The runtime and wire children define states, APIs and fault boundaries.
Never replay a mutating RPC with an unknown outcome. Single-flight startup,
bounded waits and cancellation must not produce consent or open retry storms.

Lease release closes only owned transport resources when their reuse policy
expires. It does not close Xcode workspaces or stop the shared service. Being the
client that opened a workspace does not prove continued exclusive ownership;
another application may have attached to it. Leave workspaces open by default
and report that fact. Explicit operator cleanup is outside automatic release.

After daemon restart, treat existing service workspaces as borrowed. Do not
reconstruct ownership or permission from durable observations. Revoke old lease
tokens and establish new bindings. Durable attempt/hold tables and host
coordinator authority are required; they record effects and admission, not
permission to close or claim ownership of shared Apple workspaces.

## Compatibility And Readback

Keep frozen snapshots and semantic intent unchanged. Add a runtime headless
contract epoch and binding/policy digest to provider-session compatibility;
incompatible IDE-era sessions must take the existing generation/reset path,
never reuse stale leases. Historical observations remain historical evidence.

Legacy explicit IDE PID selectors return an actionable unsupported-selector
reason, not silent rebinding. Recognized workspace selectors follow the exact
mapping rules above. Headless-only means disabling the broker never activates
direct IDE or stdio fallback.

Replace the IDE-specific review suppression with capability/effect evaluation
only where the frozen invocation already requested Xcode or carries the existing
host-execution requirement. Do not add Xcode access to unrelated reviewers.
Preserve P079 repair restrictions and fail-closed provider enforcement limits.

Keep public health coarse and old health/failure enum values compatible. Add
version-negotiated, privileged `headless_runtime` detail with explicit
absent/unknown/not-applicable semantics and tolerant future-value decoding.
Project paths, identifiers, attempts and service generations never enter the
unauthenticated health response, including legacy free-text fields. The wire
child defines exact DTO fields, privilege projections and mixed-version rules.
GraphQL, MCP and Swift consume one projection contract. Swift remains read-only;
the engine owns authorized reconciliation and durable attempt truth.

## Ownership Map

| Owner | Scoped responsibility |
| --- | --- |
| `acp/src/xcode_headless.rs` (new) | Selected toolchain, verified service identity, lifecycle and structured probes |
| `acp/src/xcode_workspace.rs` (new) | Project resolution, verified binding and classified request scope |
| `acp/src/xcode_broker.rs` | Leases, keys, warmup and pump; consumes coordinator permits, never owns separate project locks |
| Shared ACP coordinator and manager | Prepare/commit tickets, broker/shim permits, cancellation and effect-journal trait |
| `acp/src/xcode_target.rs` | Retire IDE discovery from production; retain only necessary historical decoding compatibility |
| `acp/src/adapters/`, `acp/src/lib.rs`, shim boundary | Shared resolved root, HTTP attachment and scoped command routing |
| `engine/src/executor.rs`, `mcp.rs`, `session/fingerprint.rs` | Frozen intent, effective-root authority, permission propagation and session invalidation |
| `daemon/src/xcode_broker_http.rs`, `main.rs` | Ready-lease admission and health wiring |
| Domain contract/effect modules and `control-plane/contracts/xcode-headless/v1/` | Sole types, schemas, canonical digests and projection definitions |
| DB attempt/hold repositories, engine journal adapter | Durable effect states, dedupe, recovery and authorized reconciliation |
| `domain/src/xcode_runtime.rs`, DB observation fixtures | Additive evidence distinct from durable effect authority |
| GraphQL stage/health types and MCP report readback | Consistent redacted observations |
| Swift `DaemonLifecycleClient`, `P031ThinGraphQLReadBoundary`, existing views/tests | Tolerant decoding and operator-visible action required |
| Test gates and canonical reference docs | Executable acceptance and implemented truth after migration |

Paths above are relative to `control-plane/crates/` unless identified as Swift
or explicit repository-root paths/documentation. Extract focused responsibilities only; do not
rewrite the complete broker or unrelated executor paths.

## Acceptance And Dependency Order

### I1: Implement, Test And Adapt The First Hypothesis

The [first-iteration contract](2026-09-19-xcode-headless-iteration-1.md) and plan
were approved and the bounded harness implemented. R5 closes that experiment
with the per-case offline/live composition and retained deviations in the
linked evidence. No daemon deployment or provider run was needed or performed.
That I1 learning step did not itself approve the next slice; the September 20
approval recorded in R8 now authorizes full implementation under this contract.

Full mutation journaling, broker/shim coordination, public readback migration,
cold service startup, packaged consent and release receipts are not I1 entry
requirements. Lifecycle open is still an effect: the experiment records dispatch
intent, stops on uncertainty and never automatically reopens or resumes it.
The child defines the bounded recovery rule, not production-grade dedupe.

### I2-I4: Approved Implementation, Evidence-Gated Admission

| Iteration | Scope | Entry / exit decision |
| --- | --- | --- |
| I2 | Production root authority, prepare-before-reuse, closed read facade, binding and scoped readback | Implement approved trusted-project admission and observable continuity checks; then verify the actual broker lease with IDE absent under separate live authorization |
| I3 | Additional capabilities, coordinated lifecycle/shim effects, durable outcomes and recovery | Admit each capability only after its closed request/result/error adapter, trusted-project scope controls and effect tests; complete coordinator/journal before production lifecycle open, including if I2 needs it |
| I4 | Packaged identity, controlled cold start, complete workflow coverage and release | Validate runtime/wire contracts, canonical gates and the release receipt before Ops rollout/retry |

These are dependency boundaries, not an instruction to finish all I3 code before
learning from I1. I2 cannot silently enable production workspace open while its
effect prerequisites are deferred to I3. Later child contracts and plans are
revised from recorded evidence and reviewed before expanding authority. R8
authorizes implementation, not automatic capability admission, live operations
or release. Neither that approval nor an I1 pass restores IDE fallback.

For the production slices, use injected process/CLI/MCP fixtures and RED/GREEN
tests for each invariant:

| Area | Required cases |
| --- | --- |
| Host | Absent IDE, cold/warm service, wrong UID/install, ambiguous hosts, PID reuse |
| Permission/trust | Missing or revoked operator project trust, mismatched policy digest, enabled but unapproved, pending, denied, revoked, unsafe global grants; zero auto-grants |
| Project | Repo versus implementation worktree, missing root, multiple candidates, symlink escape, exact legacy remap |
| Binding | Missing/mismatched initial open ID/path, MCP `isError`, path-only fresh status, missing status, observed disappearance/restart; no prose or default mapping |
| Facade | Missing/foreign/null/IDE selectors, disallowed paths, read-only effects, lifecycle and unknown-method denial |
| Protocol | ID isolation, notifications, list changes, schema incompatibility, bounded errors/output |
| Concurrency | Single-flight open, read sharing, exclusive mutation, queue timeout, cancellation, no cross-run backend reuse |
| Recovery | Service/daemon restart, stale tokens, unknown mutation outcome, no replay or foreign workspace cleanup |
| Integration | Provider HTTP payload, same cwd/shim/MCP root, repair restrictions, fingerprint reset, old observation decoding |

The child contracts add required fault injection, observed same-generation drift,
honest limits for undetectable external changes, two-daemon/different-DB
exclusion, public redaction and mixed-version fixtures.

Exercise the complete ACP suite and focused engine/domain/DB/GraphQL/MCP tests
through managed Cargo. Update the existing P051 gate family without renaming its
historical aliases; add a focused headless fixture lane and verify the actual
current Swift test selectors instead of retaining deleted inspector targets.
Swift checks use `scripts/test-gate.sh`. UI testing remains remote-only.

### Full Migration: Live And Release Proof

With IDE still closed, prove project-ready warmup, authorized read access and
scope rejection through the actual broker HTTP lease, not just a standalone
bridge. Repository builds/tests use canonical gates. Separate runtime transport
proof, repository test results, and provider-run success in the evidence.

Ops owns packaged-daemon installation/consent validation and an explicitly
authorized run retry. No current incident is considered resolved merely because
fixtures pass. Do not claim a completed full migration until the real workspace
binding, closed-IDE broker path and required release-host checks have passed.
The versioned release receipt and validator in the release child are mandatory;
missing/unknown checks or missing required capabilities mean `hold`.

Update `docs/reference/xcode-mcp-bridge-pool.md`, related gate/runtime references
and obsolete IDE-launch guidance only when they describe implemented behavior.
Keep the research record's discarded prototype evidence clearly historical.

## Implementation Handoff

R8 records the user's explicit September 20 trusted-local-project and full
implementation approval; runtime/wire/release children are R4. The trust-model
decision is no longer pending. Continue implementation and verify the actual
admitted routes against these contracts; do not repeat upstream schema discovery
as a prerequisite. P1-04's `upstream54` study is complete, but its closed facade
and production acceptance are incomplete. No implementation-complete or release
verdict follows from this spec edit.

R3 planning acceptance, R4's blocked attempts, R5's composed I1 closeout and R7's
then-pending trust decision remain historical. Preserve the I1 spec, plans and
all evidence files/identities; R8 does not retroactively revise their contracts
or results. Lifecycle-effect prerequisites remain required for production open.

Keep the I1 experimental verdict separate from full-migration/release
readiness. Do not call P1-01 through P2-01 implemented or P1-05 resolved because
they have been specified or assigned to a later iteration. The
[review disposition](../../evidence/xcode-headless-review-response-2026-09-19.md)
tracks that distinction, historical identities and the R5 identity.

Historical I1 authorization covered inspected fresh pairs with per-attempt
identity checks and no replay; the then-current pause was until September 20.
It is not today's next-decision gate or authorization for new live work. The
R8 documentation-only revision edited only these four specs: no probes,
permission actions, service/IDE/run operations, plans, evidence, source edits,
commits or pushes in that revision. Later implementation has its own record above.
At the final I1 run, I1 had changed no production code or frozen inputs.
Later offline dependency work has a separate source identity and is outside
this experimental verdict; no current-tree 575-input match is claimed.
Development consent does not establish packaged consent; no release or
independent production Ready verdict is claimed.
