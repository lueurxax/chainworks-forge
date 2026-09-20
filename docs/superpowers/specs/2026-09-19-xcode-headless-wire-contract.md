# Headless Wire And Readback Contract

Revision 4 (R4), 2026-09-20. Normative production-target child of the
[headless design](2026-09-19-xcode-headless-mcp-design.md).
Owns P1-04, P1-06 and output-handle scope controls in P1-05.
This specifies new wire behavior; it is not a claim about current Apple schemas.
R4 records the parent's R8 explicit trusted-local-project approval, not completed
facade implementation or production acceptance. Historical I1 evidence is unchanged.

## Iteration Applicability

These provider/API/readback rules govern I2-I4. The
[I1 experiment](2026-09-19-xcode-headless-iteration-1.md) exposes no provider API,
changes no public health/GraphQL/Swift DTO and admits no arbitrary tool calls.
It implements only bounded internal open/read adapters from the captured input
schemas and records actual result/error shapes to inform this later bundle.
Do not build a 54-tool facade before testing those two operations.

The manifest, effect API and projection definitions below are the production
target, not a claim that a checked-in schema bundle already exists. Each later
slice must close the exact schemas/errors it exposes before admission. I1's
experimental mapping status cannot populate a current production `Binding` DTO.
R4 replaces the prospective confinement prerequisite with tested admission under
`trusted_local_project_v1`; it does not turn historical observations into grants.

## Sole Manifest Owner

`domain::xcode_contract` owns canonical encoding, manifest types, public reason
codes and readback DTOs. A checked-in contract bundle under
`control-plane/contracts/xcode-headless/v1/` is the sole schema authority.
ACP implements adapters for its admitted entries. Engine, daemon, GraphQL, MCP
reports and Swift cannot independently compute a different digest or broaden
the accepted tools. Generate consumer fixtures from this bundle.

Each manifest entry contains exactly: `name`, `version`, `effect`,
`request_schema`, `result_schema`, `error_schema`, `upstream_contract_id`,
`workspace_arguments`, `path_arguments`, `handle_inputs`, `handle_outputs`,
`completion_rule`, `trust_policy_id`, `trust_policy_digest`,
`required_permissions` and `repository_gate_policy`. Effect is `read`, `mutate`, `execute` or
`broker_control`. All schema references are local, immutable and digest-bound.

`trust_policy_id` is exactly `trusted_local_project_v1` in this revision.
`trust_policy_digest` identifies the immutable versioned policy definition in
the bundle, using the canonical trust-policy digest domain below. The same
ID/digest is bound into runtime bindings and release receipts. It declares the
trusted local project model and its limits, not an Apple sandbox proof. A policy
digest is not an operator trust decision: admission separately requires explicit
operator trust of the pinned project/root and UID plus evaluated permissions and
the effective tool allowlist. Unknown policy versions/digests fail closed.

An admitted tool has complete request/result/error JSON Schemas with closed
objects, explicit required/nullable fields, numeric/string/array bounds and
no unresolved references. Do not copy dynamically discovered schemas into the
provider surface. Apple version/build and captured schema fingerprints select
a tested adapter; version-number matching alone does not establish support.

The [installed-contract follow-up](../../evidence/xcode-headless-contract-2026-09-19/README.md)
now captures all 54 advertised input schemas and 54 output schemas for server
25317, including negotiation of MCP 2025-06-18 and the legacy protocol.
P1-04's exact-schema study is complete as `upstream54`; schema discovery is no
longer an external planning blocker. The admitted facade is incomplete. Raw
schemas are not the admitted manifest: their objects are not closed, one output shape
is effectively unspecified, and there is no uniform advertised Apple error
schema. The bundle must define explicit normalization, effect classification
and scoped response fixtures before admitting a tool. Missing/incompatible
entries remain denied and missing required capabilities keep release on hold.
The parent records the September 20 native status observation: structured
`openWorkspaces` entries carry `path`, `displayName`, `activeSchemeName`, but no
`workspaceIdentifier`. Adapters must not claim status revalidates an ID/path
association. R4 requires initial structured open ID/path evidence, pinned service
generation and fresh exact open-path status, not atomic workspace lifetime or
execution-time confinement guarantees.

## Canonical Digests

Canonical encoding is JCS as specified in
[RFC 8785](https://www.rfc-editor.org/rfc/rfc8785), implemented by a conformant
library rather than a new handwritten serializer. Reject duplicate keys,
invalid Unicode and non-finite values before authority comparison. Do not
normalize Unicode. Values requiring integer precision beyond the JCS numeric
domain, including large filesystem identities, use schema-defined decimal
strings, never lossy numeric coercion. Preserve array order and JCS property
ordering/number serialization.

Fields designated sets by the manifest are deduplicated and sorted before
canonical encoding. Path canonicalization and filesystem identity resolution
happen before encoding. A serialization order from a hash map is not a contract.

Digest is lowercase hex SHA-256 over `domain + NUL + canonical_json_utf8`.
Domains are `cw.xcode.manifest.v1`, `cw.xcode.binding.v1`,
`cw.xcode.operation.v1`, `cw.xcode.receipt.v1`, `cw.xcode.trust-policy.v1`;
do not reuse digests across domains. The trust-policy digest covers the immutable
policy definition, while the binding digest also covers the operator admission
reference and pinned execution identities. Changing policy requires re-admission,
not in-place relabeling of a lease or historical observation.

Canonical test vectors for domain `cw.xcode.canonical.v1`:

| Input | Canonical bytes after NUL | SHA-256 |
| --- | --- | --- |
| `{"b":2,"a":1}` | `{"a":1,"b":2}` | `8547eab8cbd047549dd060bebdd2b52f0ac3e942b0a3fe354daade156ec6d3f3` |
| `{"a":1,"b":2}` | `{"a":1,"b":2}` | `8547eab8cbd047549dd060bebdd2b52f0ac3e942b0a3fe354daade156ec6d3f3` |
| `{"a":1,"b":3}` | `{"a":1,"b":3}` | `880568bdd65867e6b927358e43dd3cbcad5eda9f5d58f1ce9adb13a685fa6854` |

Conformance also tests non-ASCII ordering, control escapes, duplicate rejection,
null versus omission, array order, and real normalized binding/request fixtures.
Only domain code produces authority digests; UI displays received values.

## Effectful Calls

Raw mutating Apple tools are not directly exposed. Admitted mutators use
broker-owned prepare/commit tools so providers have a persistent operation key
before an effect. Tool schemas and errors below are broker schemas, not claims
that Apple accepts additional nonce fields.

Notation: `uuid` is lowercase hyphenated UUID; `digest` is 64 lowercase hex;
strings are UTF-8. Every object below rejects undeclared fields. Nullable fields
are present with null; optional fields are explicitly identified as optional.

| Tool | Exact input fields | Exact successful structured output |
| --- | --- | --- |
| `ChainworksXcodePrepareMutation` | `operation_key: uuid`, `operation: admitted mutate/execute tool name`, `arguments: that entry's exact request schema` | `schema_version: 1`, `attempt_id: uuid`, `nonce: uuid`, `request_digest: digest`, `state: AttemptState`, `binding_digest: digest` |
| `ChainworksXcodeCommitMutation` | `nonce: uuid`, `request_digest: digest` | `schema_version: 1`, `attempt_id: uuid`, `state: AttemptState`, `result: admitted result or null`, `error: ToolError or null` |
| `ChainworksXcodeAttemptGet` | `attempt_id: uuid` | `schema_version: 1`, `attempt_id: uuid`, `state: AttemptState`, `request_digest: digest`, `result: admitted result or null`, `error: ToolError or null` |
| `ChainworksXcodeAttemptList` | `limit: integer[1..100]`, `cursor: string[1..256] or null` | `schema_version: 1`, `items: PrepareMutation outputs[0..limit] plus their operation_key`, `next_cursor: string[1..256] or null` |

`arguments` and `result` are a closed discriminated union generated from the
admitted manifest, never unrestricted JSON. No admitted mutators means prepare
and commit are absent from `tools/list`. AttemptGet/List remain available only to
an authorized owner with retained attempts. Nonce is not a credential; normal
bearer, owner, project and current-permission checks apply on every call.
Control-tool status reads are journal-only and need no live service. An expired
or revoked lease never gains recovery access; privileged operator readback
remains available when a provider no longer has an authorized endpoint.

`ToolError` has exactly `code: ReasonCode`, `message: string[1..512]`,
`retry: RetryDirective`, `attempt_id: uuid|null`, and
`binding_state: BindingState`. There are no raw subprocess errors or Apple
payloads in messages. Operation keys/nonces are never forwarded to Apple.

`AttemptState` is `prepared|dispatched|succeeded|failed|unknown|reconciled`.
`RetryDirective` is `never|after_operator_action|after_rebind|same_operation_only`.
No error tells a client to resubmit an unknown effect under a new key.
Get/commit encode terminal success with non-null result and null error; terminal
failure with null result and non-null error; prepared/dispatched with both null.
Unknown has null result and `outcome_unknown`. Reconciled returns the recorded
result if proven applied; otherwise a typed non-retryable error plus the
reconciliation outcome in privileged attempt readback. List is owner-lineage
scoped, ordered by durable creation sequence; its cursor cannot widen that scope.

Successful MCP tool results have `isError: false`, `structuredContent` matching
the admitted result schema and one text content item containing its canonical
JSON. Tool-level failures have `isError: true` and the same encoding of exactly
`{"schema_version":1,"error":ToolError}`. No images, resource links or embedded
files are passed through unless their exact entry explicitly admits them.
Uncorrelated/unknown fields and handles are not silently projected as trusted.

## Build And Execution Decision

For this repository, the following Apple tools are **unavailable**, not forwarded
and not hidden behind a nominal gate label: `BuildProject`, `RunAllTests`,
`RunSomeTests`, `RunProject`, `RenderPreview`, `RunCodeSnippet`,
`DeviceInteractionInstallAndRun`, and `DeviceInteractionStartWorkspaceSession`.
Their effects can bypass canonical gates or the remote-only UI policy.
Other tools with equivalent effects are denied by classification, not name alone.

Build/test access stays on the existing authorized shim path through
`scripts/test-gate.sh`. The coordinator and journal apply there as well. A gate
invocation is identified by a resolved approved gate entry, source revision and
argument schema, not arbitrary shell text containing the script name. Direct
commands without a proven active gate invocation are not upgraded to gate
authority. Existing remote UI gates remain remote.

There is no new MCP gate facade in this revision. A future admitted execution
tool needs separate effect, completion and repository-policy proof. Disabling
the raw Apple tools is not a loss of build/test workflow coverage when the
canonical headless gate route passes its required acceptance checks.

## Handles And Per-Dispatch Validation

`workspaceIdentifier` exposed to providers is a broker alias, not a global
Apple identifier. Missing scope is filled from the current lease. A supplied
alias must match that lease. Foreign, null/default, stale or IDE tab selectors
are rejected. Internal adapters always send the explicit upstream ID from the
initial exact structured open ID/path result; they never omit it for Apple's
default or substitute a path selector. Absolute-path selectors failed native
read in the previous checkpoint. Neither workspace-list prose nor path-only
status establishes an initial mapping or supplies a replacement ID.

Each output handle is registered before response delivery with type, opaque
upstream value, binding digest/revision, owning lineage, producing attempt or
read request and expiry. Provider aliases are scoped to that record. Input
handles must match all fields and current authority, not merely have a valid
format. Observed service restart, workspace remap or binding invalidation revokes
them. Mapping revision describes observed continuity, not proof that external
clients cannot reassign an ID between probes.

Do not infer a handle from free text, a caller-provided path or an unrelated
client's session. Unknown provenance means the tool is unavailable. Completed
attempt results containing expired handles remain historical results and do
not reactivate handles when served from dedupe storage.

The runtime child's trusted-project admission and observable pre-dispatch checks
apply to both reads and effects: pinned generation/root/project, current policy,
fresh structured exact open-path status and scoped arguments/handles. Do not
require an unsupported complete ID/path re-query or expose close/reopen broker
tools as a substitute. Unknown tools need closed tested adapters before admission;
mutators additionally need journalled completion/reconciliation and no resend.

Explicit path checks remain routing/accident guards under evaluated permissions,
not an Apple sandbox. Workspace references and authorized build scripts may go
outside the checkout. Malicious concurrent filesystem changes, external Apple
clients and unobserved close/reopen or remap remain accepted trust-model risks;
the cooperative lock cannot exclude them. Post-response filtering cannot undo
such access. No trust decision or facade response auto-grants Apple permissions.

## HTTP, JSON-RPC And MCP Errors

Preserve JSON-RPC IDs on correlated responses. Reject batches, null request
IDs and duplicate JSON object keys. Permit only bounded string/integer IDs.
Only known initialization/cancellation/progress notifications may lack an ID;
`tools/call` without an ID is rejected without dispatch. Unknown methods never
fall through to the backend.

| Condition | HTTP | JSON-RPC / MCP response |
| --- | --- | --- |
| Body exceeds 1 MiB | 413 | No body parsing or dispatch; response may be empty |
| Missing/invalid bearer or unowned lease | 401 | `-32000`, id null, generic unauthorized; no existence detail |
| Broker disabled/authority unavailable | 503 | `-32000`, authenticated detail only; no retry of an effect |
| Invalid JSON | 400 | `-32700`, id null |
| Invalid envelope/batch/notification call | 400 | `-32600`, valid recovered ID or null |
| Unknown method | 200 | `-32601`, caller ID |
| Invalid arguments, nonce digest conflict | 200 | `-32602`, caller ID; no dispatch |
| Denied tool/path/handle or foreign workspace | 403 | `-32004`, caller ID; sanitized reason data |
| Stale binding, missing/incompatible trust, project hold, capacity, permission or backend failure | 200 | `-32003`, caller ID; typed reason, retry directive and scoped attempt reference |
| Valid tool call with known content-level failure | 200 | ToolError MCP result with `isError: true` |
| Accepted known notification | 202 | Empty body; never a tool-effect dispatch |

JSON-RPC broker error `data` is exactly `schema_version:1`, `code:ReasonCode`,
`retry:RetryDirective`, `attempt_id:uuid|null`, `binding_state:BindingState`.
JSON-RPC `message` is a bounded static summary. A lost response is not permission
for automatic HTTP retry; dedupe/AttemptGet governs effect recovery.

Stable reasons: `unauthorized`, `broker_disabled`, `authority_mismatch`,
`service_unavailable`, `service_generation_changed`, `permission_required`,
`permission_denied`, `project_not_found`, `project_ambiguous`, `workspace_busy`,
`workspace_stale`, `workspace_identity_unverifiable`, `scope_denied`,
`trust_required`, `trust_policy_incompatible`, `contract_incompatible`,
`operation_conflict`, `operation_in_progress`, `outcome_unknown`, `journal_unavailable`,
`capacity_exhausted`, `request_invalid`, `tool_failed`, `unknown`.
New reasons require bundle revision; consumers decode unknown strings safely.
These are prospective R4 reasons; no stored I1 result or historical reason is
rewritten to fit the new trust contract.

## Versioned Readback

Do not add new values to legacy health/failure enums consumed by old clients.
Preserve the old observation envelope and its bounded arrays; add an optional
`headless_runtime` object. Its `schema_version` is 1. Legacy failure fields map
new causes to existing broad categories, while detail uses the new object.

`HeadlessRuntimeV1` has exactly these required fields:

| Field | Type / meaning |
| --- | --- |
| `schema_version` | integer, exactly 1 |
| `availability` | `known\|unknown\|not_applicable` |
| `observed_at` | nullable UTC RFC3339 timestamp |
| `freshness` | `current\|historical\|unknown`; historical detail cannot authorize dispatch |
| `host_kind` | `headless\|unknown`, null when not applicable |
| `binding_state` | BindingState |
| `reason_code` | ReasonCode or null when no failure/action is present |
| `service_generation` | nullable privileged ServiceGeneration DTO |
| `binding` | nullable privileged Binding DTO |
| `attempts` | at most 100 projected AttemptSummary entries, ordered by durable sequence |
| `attempts_truncated` | boolean; bounded projection is not the journal |
| `has_unresolved_effects` | boolean; computed from durable holds, not the bounded array |
| `manifest_digest`, `binding_digest` | nullable digest strings |
| `projection` | `operator\|run_scoped\|coarse` |

`BindingState` is `unprepared|preparing|project_ready|action_required|revoked|failed|unknown|not_applicable`.
`ServiceGeneration` fields: `uid`, `pid` (positive integers), `boot_id`,
`process_start_id`, `executable_identity`, `developer_directory`, `build_identity`
(bounded strings). `Binding` fields: `project_path`, `effective_root` (absolute
strings), `workspace_identifier` (broker alias), `mapping_revision` (positive
integer), `trust_policy_id` (exactly `trusted_local_project_v1`),
`trust_policy_digest` (digest). These objects are null outside the operator
projection; run-scoped tools use their separately authorized context. These
policy fields do not claim atomic mapping or executor-side confinement.

`AttemptSummary` fields: `attempt_id`, `state`, `effect`, `recorded_at`,
`reason_code` (nullable), `reconciliation` (nullable
`applied|not_applied|partial`), `evidence_ref` (nullable scoped reference).
All fields are required; their nullability is explicit. No raw request/result,
nonce, global upstream handle or credential appears in these summaries.

An absent `headless_runtime` means a legacy/unreported contract, not false,
ready, or not-applicable. `unknown` means expected information is unavailable
or an enum/version is unsupported. `not_applicable` means this invocation did
not request the capability. `known` means the observation was established at
`observed_at`; only `freshness=current` may describe current admission. Persisted
historical readiness is never promoted to current readiness after restart.

GraphQL uses string-backed tolerant enum wrappers for this additive surface,
nullable detail objects and non-null bounded lists. Swift maps unfamiliar values
to `unknown`, not a decode failure or healthy default. Existing fields retain
their previous wire names/nullability; new fragments are queried only after
contract-version negotiation. MCP reports use the same projected DTO, not a
second serialization of the privileged journal row.

## Privilege And Mixed Versions

| Surface / caller | Allowed data |
| --- | --- |
| Unauthenticated `/xcode-mcp/health` | Existing coarse counts/state, static reason/message, supported schema versions; no project paths, IDs, generations, attempts or raw errors |
| Authenticated operator diagnostics | Full redacted V1 DTO and bounded scoped attempt inspection |
| Authorized run/stage report reader | Run-scoped projection only, subject to existing report authorization; no unrelated run/project data |
| Provider lease | Only its own binding aliases and attempts; no public enumeration or reconciliation authority |
| Observer without detail capability | Coarse projection or existing denial; no newly inferred privileges |

Server-side authorization occurs before projection. Redaction cannot rely on
Swift hiding fields. Legacy `operator_message` and error text are also sanitized
so path leakage does not move into an old field. No new route gains authority
because it is bound to loopback.

The public health response may add `headless_runtime_schema_versions:[1]` while
retaining all legacy fields and the four existing health-state values. Absence
of that field makes a new client use its legacy query/read-only display path;
it must not send a GraphQL fragment an old daemon cannot validate. An unknown
version produces unsupported detail and blocks automation depending on it.

Required combinations: old Swift/new daemon, new Swift/old daemon, new/new,
future enum, future schema version, absent object, explicit unknown and explicit
not-applicable, restart with unresolved effects, and authenticated versus public
projection. Old UI cannot issue the new reconciliation command.

Upgrade migrates the journal before headless admission and invalidates IDE-era
session generations. Do not rewrite frozen runs or old observations. Downgrade
to a binary lacking the migration/contract must fail closed under the existing
DB-version policy. No destructive rollback or table drop; use a forward fix.
