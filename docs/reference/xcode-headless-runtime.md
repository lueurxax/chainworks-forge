# Headless Xcode Runtime

This guide describes the implemented daemon, invocation, shim, and operator CLI
paths as of 2026-09-21. Offline fixtures cover their admission and failure
behavior. **Live acceptance of this combined runtime, including native cold
startup, has not yet been executed.** Historical Apple schema captures are
contract inputs, not evidence that the current deployment has passed acceptance.

## Supported Host and Trust Boundary

The production route uses the dedicated **Xcode Service**, never the Xcode IDE.
Admission currently pins all of the following:

| Identity | Required value |
| --- | --- |
| Xcode product build | `27A266a` |
| Service bundle ID | `com.apple.dt.mcp-server` |
| Service bundle version | `1.0` |
| Service source version | `25317000000000000` |
| Upstream MCP server | `xcode-tools`, version `25317` |
| Upstream MCP protocol | `2025-06-18` |
| Trust policy | `trusted_local_project_v1` |

The selected developer directory comes from the host's `DEVELOPER_DIR` or
`xcode-select -p`. Native process metadata pins the effective UID, executable,
PID, start time, boot identity, installation, and service generation. Missing,
ambiguous, changed, or unsupported identities fail closed; another installation
or an open IDE is not a fallback. A version upgrade needs a reviewed contract
update, not an environment override that bypasses the pin.

Chainworks project trust and Apple's consent are separate prerequisites.
Runtime execution does not enable headless access, approve an agent or folder,
set unsafe allow-all, reset permissions, or repair insecure trust-store files.
Apple headless access must already be enabled with
`unsafeAlwaysAllowAllAgents=false`. If Apple consent is missing, stop at that
operator boundary; a model approval or Chainworks trust grant cannot replace it.

Trust binds an explicitly admitted local project, effective execution root,
directory identities, UID, operator grant, and policy digest. It is **not an
Apple sandbox or checkout-only filesystem boundary**. Xcode project organization
references may resolve outside the checkout. Project metadata and referenced
local content must be operator-trusted, and same-UID external clients are assumed
cooperative rather than isolated.

Projects must be valid `.xcodeproj` or `.xcworkspace` directories inside the
resolved execution root, with regular in-bundle metadata files. An explicit
selector is root-relative, for example `App.xcodeproj` or `workspace:App.xcworkspace`.
Exact supported legacy `workspace:` paths are remapped to the effective worktree;
they cannot fall back to the original checkout. Without a selector, discovery
requires exactly one immediate-child project/workspace and scans at most 4096
entries. It does not recursively search or guess workspace membership.

## Closed Provider Surface

The provider-facing MCP surface exposes only `XcodeRead` when the current frozen
permission policy grants read access. `tools/list` never advertises internal
workspace opening, native service startup, build/test tools, or arbitrary Apple
tools discovered upstream. The immutable Apple v1 open/read contract is not
expanded by canonical gate admission.

`XcodeRead` takes a project-organization `filePath`, optional `offset` (default 1),
and `limit` (1 through 600, default 600). An optional `workspaceIdentifier` must
equal the runtime's binding alias; omission uses that binding. Providers cannot
select an upstream workspace ID, process, checkout, or service generation.
Content is bounded to 256 KiB. JSON is bounded to 1 MiB and duplicate object keys
are rejected. Unexpected schemas or upstream envelopes do not gain admission.

Build and test execution uses the repository's existing canonical entry point:

```bash
./scripts/test-gate.sh build
./scripts/test-gate.sh fast
```

With authenticated shim environment present, the script routes immediately to
the installed `chainworks-test-gate` shim, before local bootstrap/cache work.
The Unix socket authenticates the actual peer and token and resolves the token's
invocation on the server. Client invocation ID, root, cwd, and environment are
not authority. The shim generates one operation key per invocation and does not
automatically retry a failed exchange.

The runtime checks the exact gate and argument vector, pins the canonical
`scripts/test-gate.sh` identity and SHA-256, commits the journal fence, and executes
that script in the prepared root with a pinned host environment. Shim credentials
are cleared for host execution, preventing recursion. There is no MCP gate
facade, shell-command-text dispatch, or arbitrary executable override. Raw
`xcodebuild`, `xcrun`, `simctl`, and `mcpbridge` shim requests are rejected on this
production path, including requests offered as harmless introspection.

Current catalog permissions are explicit data, not authority inferred from a
profile name or shell-command label:

```yaml
xcode_headless: {read: true, gates: [build, fast, full, guardrails, list]}
```

`RO_VERIFY` and `CODE_WRITE` carry those gates. Relevant broker-only profiles
`RO_REVIEW`, `PROPOSAL_WRITE`, and `DOC_WRITE` carry
`xcode_headless: {read: true, gates: []}`. Other current profiles do not receive
this field. Backend/extension selection still determines whether the Xcode route
is requested. The runtime hashes the full evaluated permission policy; labels,
prompt text, and an Apple tool allowlist cannot replace these keys. A gate must
also belong to the canonical script's closed gate schema; extra arguments are
not accepted by the current one-selector interface.

The canonical script retains its remote-only UI-test controls. Nothing in this
route authorizes local UI tests. See [Test Gates](test-gates.md) and
[Agent UI Test Execution](agent-ui-test-execution.md).

## Invocation Lifecycle

Admission resolves the persisted run's effective root and explicit permissions,
checks durable project trust, and acquires the shared coordinator permit before
provider session launch. The current runtime holds an exclusive project permit
for the invocation; nested gates borrow that owner rather than upgrading a read
lock. Idle provider sessions are not perpetual authority. Invocation/session
binding and an active prompt are required for provider reads and shim effects.

The invocation timeout must be positive and no greater than **24 hours**.
Preparation has a separate **720-second maximum**, bounded by the same invocation
deadline. Completing preparation does not reset or extend the invocation clock.
Production coordinator queue bounds are 16 entries and 45 seconds. Actual task
deadlines may be shorter than any of these ceilings.

Each provider read and prepared-binding revalidation has a **60-second maximum**,
capped by the remaining invocation lifetime. This includes waiting for controller
access and the entire host-check/read/host-check phase, not just the upstream
response. A long-lived invocation does not grant each read 24 hours or reset its
overall deadline. Timeout fails closed rather than triggering an automatic retry.

`LocalHeadlessHostInspector::new()` and ordinary `inspect()` are read-only. The
daemon explicitly selects `with_startup_on_absence()`. Its cold path is:

1. Prepare a server-pinned startup plan only after trust admission, verified
   service absence, safe existing Apple settings, and installation checks.
2. Commit the existing `workspace_open` journal dispatch fence with the planned
   installation identity. PID zero is a planning value, not a verified binding.
3. Consume the exact self-pinned startup plan once and launch only the dedicated
   service via native LaunchServices, with no project URLs or command arguments.
   Activation, prompts, recent-item updates, and running-application substitution
   are disabled; the service is hidden and no new duplicate instance is requested.
4. Wait for a stable service generation and read-only status with `running=true`,
   access enabled, and unsafe allow-all disabled. Recheck the pinned installation
   and process identity around each status probe; a PID alone is not readiness.
5. Inspect the actual service generation, initialize and validate the pinned MCP
   schema, issue one internal workspace open, validate its structured path/ID
   mapping, and durably settle the attempt before issuing a binding.

Ordinary revalidation never starts or restarts the service. A warm observation,
consumed startup plan, failed preparation, or cancelled preparation cannot arm an
automatic launch retry. Native launch/readiness has one five-second deadline;
repeated read-only probes and their delays do not reset it. A missing PID or safe
`running=false` status can remain pending. Unsafe or malformed status, installation
drift, and a changed or disappeared observed service generation fail closed.
There is no relaunch or workspace-open retry, nor a separate `service_start`
policy or operation. The installed `mcp-server` CLI has no startup-only
subcommand; its `open` command opens projects and is not used as a startup fallback.

Gate stdout and stderr retain at most 64 KiB each plus a truncation marker while
the pipes continue to drain. The invocation bounds the gate deadline. Known exit
status is distinguished from timeout, cancellation, signal, or unverified cleanup.
The process-group driver cleans descendants and reaps its leader; only verified
group settlement allows the runtime to record a terminal gate outcome.

Unknown effects retain durable project holds. Transport loss, cancellation,
service/mapping drift, or a missing outcome acknowledgement never proves that
Apple stopped working. Recovery turns unfinished dispatched attempts into
unknown attempts before new Xcode admission. Repeating an operation key does not
send the operation again; a changed request under the same identity is rejected.

### Project Trust Admission Failures

A missing `xcode-project-trust` directory or absent project grant produces
`project_trust_required`. This is separate from the journal authority in
`xcode-runtime`: a valid authority record and coordinator lock do not grant
project trust. Do not bootstrap or replace journal authority to address a missing
project grant.

| Preparation reason | Operator disposition |
| --- | --- |
| `project_trust_required` | Review and explicitly admit the exact resolved project/root before retry. |
| `project_trust_revoked` | Review the revocation; any regrant is a new explicit operator decision. |
| `project_trust_unavailable` | Inspect the underlying store, identity, lock, or timeout failure before retry. |

`HeadlessRuntime::prepare` attaches a typed `ProjectTrustAdmissionFailure`
containing the resolved execution root, project key, and bounded reason code.
The underlying error is preserved. Unsafe links, ownership or mode violations,
damaged records, missing lock files, lock contention, and identity changes remain
fail-closed; they are not repaired by lookup or treated as an absent grant.

A trust-admission preparation failure records `failed_no_launch`,
`provider_launched: false`, and an operator disposition in execution runtime
facts. Operator readback includes
the exact root/project identity; callers without operator access receive the
bounded summary without the path-bearing details. Admission failure occurs
before provider/session launch, Xcode service startup, or workspace opening.
It does not consume provider retry/escalation as if a provider had failed.
Operator GraphQL/MCP diagnostics carry the existing CLI guidance. UI Retry
retains its MCP-command guidance; neither surface creates grants. Existing
historical failures without these facts are not
retroactively repaired or reclassified.

## Operator CLI Setup

The installed daemon binary accepts this exact one-shot interface:

```text
control-plane --xcode-admin ACTION REQUEST.json
```

Set `CONTROL_PLANE` below to the selected deployed binary. Use the same `MODE`,
database, host account, and principal table as the daemon being administered.
`MODE=packaged-app` and `MODE=packaged-helper` use
`~/Library/Application Support/Chainworks Forge/control-plane.db` and ignore
`DATABASE_URL`. Development mode accepts `DATABASE_URL`; use an explicit absolute
database URL rather than relying on its cwd-relative default. Do not point a
different mode at the same database and assume its trust-store path is identical.

Authentication requires an existing principal table (normally
`~/.chainworks/auth/principals.json`) and an existing valid token supplied through
`CHAINWORKS_MCP_TOKEN`. Do not put a bearer token in argv, request JSON, evidence,
or shell history. `CHAINWORKS_AUTH_PRINCIPALS_PATH`, when used, must obey the
daemon's existing absolute-path and packaged-mode auth-root restrictions.

The recommended authorization path uses explicit policies in the existing
`schema_version: 3` principal table. The table describes that path; the narrowly
defined legacy compatibility exception is described below.

| Action | Explicit-v3 capability | Scope |
| --- | --- | --- |
| `bootstrap-authority`, `authority-inspect`, `authority-enroll` | `xcode.global_admin` and `xcode.project_trust` | Global Operator; no `run_scope` |
| `trust-grant`, `trust-revoke` | `xcode.global_admin` and `xcode.project_trust` | Global Operator; no `run_scope` |
| `effects-list`, `effects-get` | `xcode.effects.list` or `xcode.effects.get` | Requested run and owner; run-limited Operator needs matching `run_scope` |
| `effects-reconcile` | `xcode.effects.reconcile` | Requested run and owner; run-limited Operator needs matching `run_scope` |

These names map to `XcodeGlobalAdmin`, `XcodeProjectTrust`,
`XcodeEffectsDiagnostics`, and `XcodeEffectsReconcile`; they are not newly
published MCP tools. Provider, observer, and read-only-operator principals cannot
administer this boundary. An Operator class label or matching run scope alone
does not grant a missing capability.

For new bootstrap and trust administration setup, use an explicit Operator with
both global-admin and project-trust capabilities, and omit `run_scope`. A
run-scoped token cannot mutate global trust, even when the request names one of
its runs. The global marker alone is insufficient and is excluded from default
capability sets. The default UI Operator does not qualify for global trust
administration without the required explicit authorization.

Compatibility: a fully unrestricted legacy Operator is admitted before the
marker check and can bootstrap or grant/revoke trust without `xcode.global_admin`.
This requires no explicit surface policies, GraphQL policy, caller-class override,
or run scope, plus tool and resource capability sets exactly equal to the full
Operator defaults. An unrestricted label alone is not sufficient. This exception
does not admit the default UI principal or run-limited operators. Prefer the
explicit-v3 path; do not remove existing restrictions to force compatibility.

The relevant policy fragment is below. This is not a complete principal record
or a replacement principal table: retain the existing identity, privately
provisioned token, other required policy entries, and other principals. Updating
operator credentials/capabilities is a separate explicit provisioning decision;
the Xcode CLI never creates this authorization automatically.

```json
{
  "class": "operator",
  "surface_policies": {
    "mcp": {
      "allowed_tools": ["xcode.global_admin", "xcode.project_trust"]
    }
  }
}
```

Diagnostics and reconciliation remain run/owner-scoped, not global-trust writes.
A run-limited Operator needs its specific effects capability and explicit
membership in the requested `run_scope`; it cannot bootstrap or grant/revoke
project trust. An explicit global Operator still needs the specific effects
capability in addition to `xcode.global_admin`; the marker does not grant all
administrative actions by itself.

Request files must be regular, non-symlink files, at most 64 KiB, containing one
JSON object. Duplicate keys and unknown fields are rejected. **Do not include an
`operation` field**: the CLI injects it from `ACTION`. Use private request files
and substitute real persisted identities for every `<placeholder>` below.
These examples are templates, not permission or completion evidence.

The CLI opens an existing database without creating it or running migrations.
Reconciliation writes the effect journal. Authority enrollment opens a write-capable
connection only to retain a writer reservation; it rolls back without changing
database rows. Explicit trust operations write the separate trust store, and
bootstrap/enrollment publish authority records.

## Bootstrap and Migration

An existing initialized database with the Xcode journal migration is required.
Bootstrap does not migrate storage, generate a principal, or import trust from
old profile labels. Normal startup holds headless admission when authority is
missing or invalid; it does not silently enable a legacy execution route.

For the first authority transition, stop the old app and daemon through their
normal lifecycle controls and verify that their work is settled. Do not use a
forced kill or delete a lock/journal file to manufacture proof. The bootstrap
checks the legacy database lock, same-user app/daemon process inventory, database
identity, unsettled legacy effects, and Xcode holds/nonterminal dispatched effects.

`bootstrap.json`:

```json
{"confirm_legacy_shutdown": true}
```

```bash
"$CONTROL_PLANE" --xcode-admin bootstrap-authority ./bootstrap.json
```

Success returns `{"bootstrapped":true}`. The confirmation flag alone is not
sufficient. `legacy_transition_unproven`, `authority_busy`, or an insecure or
mismatched authority requires investigation, not a bypass. The authority is
host-user-global at
`~/Library/Application Support/Chainworks Forge/xcode-runtime`, pins one canonical
database identity, and uses private files plus a retained coordinator lock.
Do not remove it to switch databases or reset an unknown effect.

After successful bootstrap, start the new daemon normally so journal recovery
runs before admission. Keep old app/daemon versions from restarting concurrently.
Existing frozen catalogs retain their original contents: the new explicit
`xcode_headless` policy is captured by new snapshots. Do not relabel old bindings,
rewrite historical snapshots, or retry a held old attempt to bypass missing
permissions. This operator CLI does not migrate frozen permission policies.
A newly authorized run captures the updated catalog and still requires fresh
trust/admission checks, subject to existing project holds.

## Grant or Revoke Project Trust

Use the persisted run's repository/worktree selection and its resolved root,
not the operator shell's current directory. The CLI verifies the target against
that run. **The trust grant/revocation itself is global for that project/root
identity, not scoped to `run_id`.** It affects other invocations using the same
trusted project. The request's run ID validates the target; it cannot turn a
run-scoped token into global trust authority.

Device and inode are decimal strings from the effective directory;
for an already selected canonical root, this macOS command is read-only:

```bash
/usr/bin/stat -f '%d %i' "$EFFECTIVE_ROOT"
```

Dedicated-worktree example `trust.json`. Use the canonical repository and exact
persisted worktree; a repository grant does not cover a dedicated worktree.
For a repository-root request, `effective` instead equals `repository`, `kind`
is `repository`, and `strategy` is `null`.

```json
{
  "run_id": "<persisted-run-uuid>",
  "root": {
    "version": 1,
    "repository": "/canonical/path/to/repository",
    "effective": "/canonical/path/to/exact-dedicated-worktree",
    "device": "<effective-directory-device>",
    "inode": "<effective-directory-inode>",
    "strategy": "dedicated",
    "kind": "worktree"
  },
  "selector": "App.xcodeproj"
}
```

```bash
"$CONTROL_PLANE" --xcode-admin trust-grant ./trust.json
"$CONTROL_PLANE" --xcode-admin trust-revoke ./trust.json
```

These are separate operator decisions, not a sequence to run automatically.
Grant returns `granted: true` and a `trust_digest`; revoke returns `revoked: true`.
Grant creates or replaces a valid explicit grant but never repairs malformed or
insecure records. Regranting creates a new identity and invalidates old bindings.
The store is `xcode-project-trust` under the selected mode's app-support directory
(`~/.chainworks/dev-app-support` in development; packaged app support as above).
Directories/files are private, owner-checked, and reject inappropriate links or
modes. Runtime lookup never creates a missing store or grants trust implicitly.
Root/project replacement, revocation, and policy drift require fresh admission.

New explicit grants use trust record v3. Root and project each pin an APFS volume
UUID and a persistent 64-bit object ID, with creation metadata, UID and canonical
path. Device numbers and boot UUIDs are observations. Cross-boot device drift is
accepted only when the persisted identity matches; same-boot device drift,
foreign volumes and replaced objects fail closed. Runtime checks additionally pin
the observations accepted during that store instance's lifetime.

For a `worktree` root, `dedicated` (writer) and `shared_implementation_worktree`
(reviewer) share a durable trust slot when the remaining identity matches.
Other strategies and repository identities remain distinct. The original root,
project key and strategy remain in the grant for audit; frozen invocation inputs
are not rewritten.

All v1/v2 grants require a separate explicit v3 grant, including records whose
current device numbers still match. Runtime never falls back to legacy grants,
migrates them, or revives revoked decisions. Explicit grant checks discoverable
exact legacy slots for corruption and leaves their bytes unchanged. Revocation
publishes a v3 tombstone and also revokes discoverable exact legacy slots; it does
not infer or scan old-device identities. An explicit regrant is a new operator
trust decision. Authority enrollment alone grants no project trust.

Deploy runtime and trust-administration code together. Older administrators and
runtimes cannot consume or enforce v3 decisions and must remain stopped after
upgrade. Unsupported or unavailable persistent filesystem identity fails closed.

Headless preparation failures persist bounded `headless_preparation_failure`
diagnostics in the existing operator runtime-preflight JSON: phase, reason,
allowlisted source code, typed I/O kind/OS error, and an effect attempt ID after
dispatch. Phases distinguish startup planning/startup, host inspection,
connection, initialization, pre-open validation, opening, response decoding,
workspace mapping and post-open validation. Raw upstream messages, payloads,
paths and credentials are excluded. The phase identifies the failing boundary,
not the underlying cause of a historical failure.

After the dispatch fence, uncertainty retains the existing unknown effect and
project hold. Recovery says `reconcile_xcode_effect_before_retry`; provider
launch, automatic retry and provider fallback remain forbidden. Historical
unknown/held error codes receive the same guidance, without backfilling missing
phase evidence. If a caller disappears before error delivery, journal safety
still holds but execution diagnostics may remain absent; this is not a crash
diagnostic storage migration.

### Review, Grant, Then Retry

1. Read the failed execution and persisted run through the existing operator
   diagnostics. Match its `run_id`, repository, effective worktree, strategy,
   project selector, UID, device, and inode. Recheck the current directory
   identities; an old failure record alone is not current trust authority.
2. Review the private request file against those identities and obtain an
   explicit operator decision for that exact project/root. The selector is
   relative to the effective root. Use the same deployed binary, `MODE`, database,
   host account, and principal table as the daemon. Packaged modes ignore
   `DATABASE_URL`; development mode needs the daemon's explicit database URL
   and retains its separate development trust store.
3. With the existing authorized token supplied privately in the environment,
   run this template once after approval. `DAEMON_MODE` and `TRUST_REQUEST`
   denote the verified mode and private request-file path; no token belongs in
   either value or the request:

   ```bash
   MODE="$DAEMON_MODE" "$CONTROL_PLANE" --xcode-admin trust-grant "$TRUST_REQUEST"
   ```

4. Check the successful JSON result: `granted: true` and a 64-character
   `trust_digest`. The store rereads the written record and verifies its digest
   before returning success. There is no separate `trust-inspect` CLI action or
   MCP grant-write API. A successful grant is not proof of provider execution.
5. Separately request the supported `stages.retry` operation for the exact failed
   `stage_execution_id`, using a fresh lowercase UUIDv4 `caller_request_id`.
   Reuse that ID if resubmitting the same command after an uncertain response.
   Include `agent_execution_id` when a single-execution retry is intended. Read
   back the new execution's admission facts and actual provider/session start;
   command acceptance alone does not prove recovery.

The grant is shared by all invocations of that exact identity, not run-local.
Retry still enforces frozen permissions, provider selection, project holds, and
all other admission requirements. The separate A/B/D quota-escalation versus
frozen-profile validation defect recorded on 2026-09-21 can still prevent
recovery. A grant does not repair that contract, settle historical running
states, or authorize changes to frozen snapshots. See the
[local fix evidence](../evidence/project-trust-admission-fix-2026-09-21.md).

## Inspect and Reconcile Effects

List within one persisted run and exact owner lineage. Do not substitute an
agent label for the owner execution lineage. `effects-list.json`:

```json
{"run_id":"<run-uuid>","owner":"<owner-execution-lineage>","limit":25,"cursor":null}
```

```bash
"$CONTROL_PLANE" --xcode-admin effects-list ./effects-list.json
```

The response has `items` and `next_cursor`. Limits are 1 through 100. Feed back
only the returned cursor in the same run/owner scope; it is an attempt UUID, not
an offset or authority token. For `effects-get.json`:

```json
{"run_id":"<run-uuid>","owner":"<owner-execution-lineage>","attempt_id":"<attempt-uuid>"}
```

```bash
"$CONTROL_PLANE" --xcode-admin effects-get ./effects-get.json
```

Reconciliation is accepted only for an `unknown` attempt with its project hold
and current revision. First inspect actual project state and establish that no
operation remains in flight. A process disappearing, path-only status, a timeout,
or a human acknowledgement without effect evidence is insufficient. If that
cannot be established, retain the hold. The CLI does not execute a verification
script, fetch evidence references, or attest to the operator's assertions.

`effects-reconcile` must acquire the same exclusive host-user authority as the
daemon. Stop the daemon through its normal lifecycle before this one-shot write;
preserve the database, authority files, and unresolved holds. Stopping the daemon
does not prove that Apple has no operation in flight. If the command reports
`authority_busy`, investigate the remaining authority owner rather than removing
its lock. Read-only `effects-list` and `effects-get` do not take this writer lock.

The following `effects-reconcile.json` shape is for a **proven not-applied**
outcome. Replace revision `3` with the current readback revision, copy the exact
`intent.result_schema`, and supply the verified adapter/evidence result digest.
Do not manufacture a digest or set `no_operation_in_flight` merely to clear a hold.

```json
{
  "run_id": "<run-uuid>",
  "owner": "<owner-execution-lineage>",
  "attempt": {"attempt_id":"<attempt-uuid>","revision":3},
  "evidence": {
    "operator_id": "replaced-by-authenticated-principal",
    "inspected_state": "<bounded redacted statement of verified state>",
    "proof": {
      "evidence_ref": "operator-checks/<attempt-uuid>/state",
      "active_operation_check_ref": "operator-checks/<attempt-uuid>/activity",
      "no_operation_in_flight": true
    },
    "disposition": "not_applied",
    "outcome": {
      "schema_version": 1,
      "schema": {
        "contract_id": "<intent.result_schema.contract_id>",
        "version": 1,
        "manifest_digest": "<intent.result_schema.manifest_digest>",
        "schema_digest": "<intent.result_schema.schema_digest>"
      },
      "payload": {
        "kind": "error",
        "code": "reconciled_not_applied",
        "message": "<bounded redacted verified outcome>"
      },
      "result_digest": "<verified-result-digest>"
    }
  }
}
```

```bash
"$CONTROL_PLANE" --xcode-admin effects-reconcile ./effects-reconcile.json
"$CONTROL_PLANE" --xcode-admin effects-get ./effects-get.json
```

The server replaces `operator_id` with the authenticated principal and uses a
revision-checked transaction to record reconciliation and release the hold.
Read back the resulting state/revision. A conflict requires fresh inspection;
do not blindly resubmit with a higher revision. `applied` requires a result
payload with a bounded summary; `partial` requires an error payload with code
`reconciled_partial`. The same schema, evidence, and no-in-flight requirements
apply. Digests are lowercase 64-character SHA-256 strings. Evidence references
are bounded relative opaque IDs, not absolute paths, URLs, credentials, or raw
provider content. Reconciliation records history; it does not replay an operation
or automatically issue a new provider binding.

## Reboot identity and explicit authority recovery

A mount's `st_dev` can change after reboot. Authority v2 pins the supported
persistent filesystem identity separately from device/boot observations. The
native reader requires local APFS, nonempty volume UUID, valid persistent-object
and 64-bit-object capability bits, and the opened object's 64-bit file ID. Missing
or unsupported identity information denies admission. The reader does not call
allocating legacy object-ID APIs. Apple's
[filesystem attribute contract](https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man2/getattrlist.2.html)
describes the persistent-object capability; the macOS SDK specifies `ATTR_CMN_FILEID`
for volumes advertising 64-bit object IDs.

A legacy authority v1 record contains no historical volume or boot witness.
Unchanged path/inode and a newly observed volume UUID cannot prove continuity.
Normal startup does not rewrite the record. `bootstrap-authority` keeps its
original requirement for settled effects and no holds; it is not a rebind command.

For reviewed administrative enrollment, the separate operator protocol is:

1. Stop the app/daemon and close other SQLite connections before
   `authority-inspect`. Inspection acquires a nonblocking macOS open-file-description
   lock over SQLite's database lock range and copies the database and any WAL into
   a private temporary directory. It opens only that copy with SQLite, leaving the
   source database, WAL and SHM unchanged. A busy source, unsupported lock or
   rollback journal blocks inspection. No migration or grant occurs. The plan
   binds the original authority digest, current filesystem identity, schema and
   complete bounded journal/trust inventory; enrollment rechecks the live inventory.
2. Review its blockers and exact plan digest. Legacy v1 continuity remains
   `continuity_unproven`; enrollment is a **new trust decision** for the current
   database, not a claim that historical identity has been proved.
3. `authority-enroll` requires the exact approved plan, global Operator authority,
   stopped app/daemon writers, retained singleton/coordinator/trust locks and a
   SQLite writer reservation. Both inspection and enrollment require WAL mode;
   the plan binds that mode, and enrollment validates it inside its transaction.
   Changed records, identity, inventory or plans fail closed. A missing or insecure
   existing trust lock blocks enrollment.
4. Publication preserves the old authority and a durable audit receipt before
   atomically activating the new record. Database attempts, outcomes, revisions,
   holds and project grants are not migrated or reconciled by enrollment.
5. Review and explicitly grant each intended project separately using trust v3.
   Recovered authority alone does not enable a held or untrusted project.

Request templates (normal `--xcode-admin ACTION FILE` invocation):

```json
{}
```

Use that object with `authority-inspect`. Enrollment requires:

```json
{
  "approved_plan_digest": "<digest returned by inspection>",
  "decision": "enroll-current-database-as-new-authority",
  "confirm_stopped_writers": true
}
```

Confirmation alone is insufficient. No command in this protocol starts the daemon,
retries a run, clears a hold, invokes a provider, or calls Xcode.

Legacy holds retain their exact historical keys. Both coordinator readback and
transactional prepare/dispatch deny by UID plus either canonical path or inode.
The inode comparison deliberately overblocks possible aliases on other volumes;
it never establishes equivalence or grants trust. This preserves a held project's
fence across simultaneous rename and device drift. Reconciliation still requires
the exact original key and historical outcome evidence.

Transport loss after a dispatched `workspace_open` is a different state from
startup authority rejection: the operation may have reached Xcode even when no
provider session exists. The Xcode effect ledger and project holds are the source
for that uncertainty; a zero count in the general side-effect ledger does not
prove that no Xcode effects remain. A daemon restart, new run/owner, new operation
key, renamed checkout or new trust grant does not settle such an effect.

Use `effects-get` with the original run, owner and attempt to inspect its frozen
contract. Reconciliation requires an authenticated operator, the exact revision,
compatible historical result schema/digest, evidence references and proof that no
operation remains in flight. The CLI validates that contract; the operator must
independently verify the referenced evidence. Current project absence or a
successful new open cannot establish the historical result. If that evidence is
unavailable, retain UNKNOWN and its shared resource hold. No safe automatic
replay, hold-clear or successor exemption is provided.

An authority startup failure leaves the broker disabled. Health reports
`xcode_journal_authority_unavailable` with the bounded cause such as
`authority_mismatch`. The pre-provider error retains `xcode_mcp_broker_disabled`
for compatibility and includes that cause; it does not invent an environment
setting. Explicit environment disablement remains a separate reason. Overall
`/ready` success alone is not proof of Xcode admission.

## Verification and Source Map

All build/test work still goes through the canonical gate script. Focused offline
checks for these paths are:

```bash
./scripts/test-gate.sh xcode-headless-shim
./scripts/test-gate.sh xcode-headless-catalog
./scripts/test-gate.sh xcode-headless-project-trust
./scripts/test-gate.sh xcode-authority-recovery
./scripts/test-gate.sh xcode-headless-host-startup
```

These use fixtures, temporary processes, and sockets; they do not launch Apple
services or the IDE. Native cold-start acceptance, operator bootstrap/trust against
the intended deployed database, and end-to-end read/gate acceptance require a
separately authorized live session. Do not treat compile/offline GREEN or earlier
historical evidence as that sign-off. UI acceptance remains remote-only.

The separate release validator consumes bounded receipt/evidence bytes plus an
independently trusted candidate identity, frozen required capabilities and actual
journal count. Missing or mismatched proof yields hold; a caller-supplied pass
cannot override it. This pure validation API does not collect native evidence,
attest a collector, publish a receipt or grant release authority. The
[integration evidence](../evidence/xcode-headless-completion-2026-09-20.md) is an
offline implementation record, not a production release receipt.

Implementation references:

- [Runtime authority and gate journal wrapper](../../control-plane/crates/acp/src/xcode_headless_runtime.rs)
- [Workspace lifecycle and durable cold/warm fence](../../control-plane/crates/acp/src/xcode_headless.rs)
- [Native host inspection and one-shot startup](../../control-plane/crates/acp/src/xcode_headless_host.rs)
- [Pinned project trust store](../../control-plane/crates/acp/src/xcode_project_trust.rs)
- [Operator CLI](../../control-plane/crates/daemon/src/xcode_admin.rs)
- [Operator-scoped diagnostics and reconciliation](../../control-plane/crates/engine/src/xcode_effect_admin.rs)
- [Immutable Apple v1 contract and trust policy](../../control-plane/contracts/xcode-headless/v1/manifest.json)
- [Release receipt validator](../../control-plane/crates/domain/src/xcode_release.rs)
- [Separate release-only schemas](../../control-plane/contracts/xcode-headless/release-v1/schemas.json)
- [Canonical gate entry point](../../scripts/test-gate.sh)
