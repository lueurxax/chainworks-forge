# Installed Headless MCP Schemas And Isolation Findings

Date: 2026-09-19. Source HEAD: `a096b8831f0db7f17d1ffe426851c49a65b8b000`.
Scope: the user's request to find and study the upstream contracts behind
P1-04/P1-05. Research only, not an admitted broker manifest or release receipt.

## Result

All 54 advertised tools' complete input/output schemas were obtained directly
from the already-running Xcode Service. Schema absence is no longer an external
blocker for P1-04. Closed provider schemas, error normalization and adapter
validation remain implementation/design work; they cannot be inferred from a
raw `tools/list` response.

The investigated Apple documentation establishes signed-agent/folder permission
and workspace-reference-based access. It does not establish the stronger
checkout-only, race-resistant confinement and non-reassignable workspace-ID
contract required by the proposal. P1-05 remains unresolved. This is absence of
the required guarantee in the inspected sources, not a discovered vulnerability
or proof that Apple's implementation lacks enforcement.

## Captures And Provenance

| Artifact | Contents |
| --- | --- |
| [mcp-2025-06-18.json](mcp-2025-06-18.json) | Exact initialize and tools/list exchanges; request offered 2025-11-25 and server selected 2025-06-18 |
| [mcp-2024-11-05.json](mcp-2024-11-05.json) | Exact metadata exchanges offering the previously used 2024-11-05 protocol |
| [schema-index.json](schema-index.json) | Derived per-tool fields, requirements, source pointers and inventory counts; not authorization policy |

Captured installation: Xcode 27.0, build `27A266a`; server `xcode-tools`,
version `25317`. Host PID 29224, UID 501, start identity
`Sat Sep 19 11:11:18 2026`. Independent process-path inspection matched:
`/Applications/Xcode.app/Contents/Developer/Library/Xcode/Agents/Xcode Service.app/Contents/MacOS/Xcode Service`.
The selected host identity was unchanged before/after each capture.

File SHA-256 values:

```text
e00d534d7dbbc705ecc2211da80759281d8f42f423c408f67529a535723ab020  mcp-2025-06-18.json
8f6c31fbccae1cac96e10f6018724bf57534b4cbae7226fcc9c7e0d5302981a0  mcp-2024-11-05.json
```

Each connection used a cleared host-user environment, explicit selected PID
and exactly `initialize`, `notifications/initialized`, `tools/list`. There were
**zero `tools/call` requests**, no workspace open/close, agent launch, build,
test, permission write or service restart. Each diagnostic bridge was closed
and reaped; both exited 1 on stdin close with empty stderr, as in prior probes.
The successful correlated responses are the evidence, not a bridge exit-0 claim.

Host-side status before/after: enabled, running, unsafe allow-all false,
`openWorkspaces=[]`. Xcode IDE was absent before/after. A sandboxed status query
misreported enabled/running as false while process enumeration was denied;
the host-side read corrected that result. It is not evidence of a service
shutdown or permission change. The broker must probe in its intended host context.

## Protocol And Schema Findings

The prior 2024-11-05 observation reflected negotiation with that client request,
not the service's only supported protocol. A newer offer negotiated 2025-06-18.
The 54 complete tool definitions are structurally identical across these two
captures after ignoring JSON object-member order. Do not infer support for
2025-11-25 merely because the client offered it.

The MCP [lifecycle specification](https://modelcontextprotocol.io/specification/2025-06-18/basic/lifecycle)
defines protocol negotiation separately from tool discovery. The
[tools specification](https://modelcontextprotocol.io/specification/2025-06-18/server/tools)
defines output schemas and separates protocol errors from `isError` tool failures.
That is a protocol-level contract, not an Apple-specific stable error catalogue
or filesystem sandbox guarantee.

| Fact from the capture | Consequence |
| --- | --- |
| 54 unique names, 54 input schemas, 54 output schemas | Complete advertised input/result metadata is available locally |
| 46 tools have `workspaceIdentifier`: 45 optional, close-workspace required | Broker scope injection/rejection is still necessary |
| 8 tools have no workspace argument | A single workspace-argument rule cannot safely cover the entire service |
| No tool has `annotations` | Read/write/idempotency classifications cannot be imported from advertised hints |
| No root schema has `additionalProperties:false` | Raw advertised schemas are not closed broker schemas; this alone does not prove runtime accepts unknown fields |
| No advertised `errorSchema` or uniform error-code catalogue | Do not fabricate Apple error enums from success schemas |
| `UpdateTargetBuildSetting` output is an object with no declared properties or required fields | A present output schema need not describe meaningful result fields |
| Some descriptions mention null while their property schema says string | Nullability needs an explicit adapter decision and response fixtures |

Static checks found no duplicate tool names, unresolved `$ref` entries or
required-property names missing from their object definitions. This checks
metadata structure only, not full JSON Schema conformance or actual tool results.

### Workspace Lifecycle

- `XcodeOpenWorkspace` requires absolute `path`; its result requires
  `workspaceIdentifier`, but `workspacePath` is optional.
- `XcodeListWorkspaces` has no declared input properties; its only required
  output field is `message`, not a structured workspace array.
- `XcodeCloseWorkspace` requires `workspaceIdentifier`; output is `message`.
- Workspace-scoped descriptions accept an identifier, with `workspace1` as an
  example, or an absolute workspace path. They do not specify ID lifetime,
  non-reuse, mapping revision, compare-and-use semantics or an expected-generation
  argument.
- CLI `status --format json` reports structured status, but the empty workspace
  list captured here cannot establish the shape/identity guarantees of a
  populated list. No real workspace mapping was acquired in this research.

### Project Paths Are Not Checkout Paths

`XcodeRead`, `XcodeWrite`, `XcodeLS`, `XcodeGlob` and `XcodeGrep` explicitly use
Xcode project organization. `GetTargetBuildSettings.projectPath` and
`XcodeListTargets.targets[].containingProjectPath` also refer to project
organization, despite the latter description calling it canonical.

Therefore `effective_root.join(filePath)` is not a demonstrated translation to
the file Apple will touch. A project can contain references and dependencies;
LS/Glob/Grep output schemas explicitly include `packageDependencies`.
`XcodeWrite.absolutePath` is optional and appears only after the write, so it
cannot be used as a pre-effect authorization check.

One concrete nullability discrepancy:
`XcodeListTargets.targets[].containingProjectPath` is declared `type:string`,
while its description says null may be returned when canonicalization fails.
No actual null response was tested, and no schema was silently rewritten.

### Handles, State And Effects

The eight tools without a workspace argument are `DeviceInteractionEndSession`,
`DeviceInteractionStartSession`, `DeviceInteractionSynthesize`,
`DocumentationSearch`, `XcodeListTemplates`, `XcodeListWorkspaces`,
`XcodeNewProject`, and `XcodeOpenWorkspace`. Some operate on service/global or
device state; they are not all harmless workspace-independent queries.

Device creation returns `interactionSessionKey`, while synthesis consumes
`interactSessionKey`. Preserve this actual spelling difference in an adapter;
do not guess field names. Provenance binding is still needed for such handles.
`GetConsoleOutput.launchSessionReference` can default to the current/recent
launch. Scheme, destination and test-plan tools use active shared state.

Several listing tools describe writing full listings to files and returning
their paths. A query-shaped operation may have auxiliary filesystem effects.
Those paths are not automatically authorized for a provider to read, and
query-like naming is not proof of a pure operation.

## What Apple Establishes

### Headless Permission Model

The current official [Xcode 27 release notes](https://developer.apple.com/documentation/xcode-release-notes/xcode-27-release-notes),
retrieved through their Markdown representation to avoid stale beta search
excerpts, describe the headless preview and persistent permission for signed
agents to use projects within a directory tree. They warn about preview
permission-management limitations and do not recommend global unsafe grants
for ordinary desktop use.

Installed CLI help narrows the observable administrative contract:

- `allow-folder` permits serving workspaces from a folder and its descendants.
- `approve --always` grants durable trust to signed agents/folders;
  `--for-24-hours` is time-bounded.
- Neither help text specifies per-RPC checkout-root containment, link-resolution
  rules, workspace-ID non-reuse or atomic authorization with file use.

Only help/status was read. No mutating administration command was executed without
`--help`; no settings were enabled or reset.

### Workspace References And Agent Security

In the official [WWDC26 Coding Intelligence group lab, around 12:08](https://developer.apple.com/videos/play/wwdc2026/8007/?time=728),
Apple engineers describe managed filesystem security and access to items
referenced by an Xcode workspace, including multiple projects. Their brief
statement is: "If it's referenced in your Xcode workspace, the agent can get to it".
This is a reference-aware access model, not a documented lexical checkout-root
boundary for every external headless MCP call.

[Extending and customizing agents](https://developer.apple.com/documentation/xcode/extending-and-customizing-agents)
describes command/tool permissions in Xcode's Intelligence settings. Together
with the release-note security-layer description, this establishes that Apple
has agent access controls. Neither source proves that an independently launched
Chainworks provider inherits an execution-time sandbox from the MCP service.

The service's signed entitlements were also inspected read-only. No
`com.apple.security.app-sandbox` entitlement appeared. This observation does
**not** prove absence of private or runtime access controls and is not used as
proof of an escape. It only prevents treating standard App Sandbox confinement
as already established by this service's identity.

### Unestablished Guarantees

The inspected official docs, local CLI help and all 54 schemas do not establish:

1. A workspace ID cannot be rebound/reused while the service process lives.
2. An RPC atomically verifies an expected workspace mapping/version before use.
3. All file operations enforce the Chainworks effective-root policy at actual
   use, including project references, dependencies and links.
4. Separate bridge processes isolate Apple's active scheme, destination,
   debugger/device handles or other workspace-global state.
5. A uniform Apple mutation nonce, dedupe guarantee or durable outcome API.

These are specifically the guarantees being sought, not a claim that the
service is unsafe. No adversarial path, symlink-race or cross-project operation
was executed. Ordinary successful reads would not prove these properties.

## Implications For The Proposal

P1-04: replace "upstream schemas unavailable" with this exact captured input.
Derive a closed allowlisted facade, classify effects and output artifacts, fix
nullability decisions explicitly, and validate real permitted responses later.
Keep protocol negotiation, Apple schemas and Chainworks policy separate.

P1-05: do not relabel signed-agent/folder consent as a checkout sandbox. The
proposal currently demands stronger authority than these sources establish.
Closing that finding requires either a documented/execution-proven enforcement
boundary outside the shared service, or an explicit reviewed decision to trust
the approved workspace and its references instead of promising strict root
confinement. That changes the security contract and is not selected here.

No product code, frozen configuration, implementation plan or release verdict
was changed. The first metadata/schema research blocker is resolved; real
workspace access, packaged consent and strict confinement remain unproven.
