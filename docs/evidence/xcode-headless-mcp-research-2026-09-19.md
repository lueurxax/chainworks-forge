# Xcode 27 Headless MCP Contract Research

Date: 2026-09-19. Source base: `a096b883`.
Status: research for a full headless migration. The user rejected the narrow IDE
fix and closed Xcode IDE. All prototype source/reference changes described below
were removed; they are historical evidence, not the migration implementation.
The managed headless-only direction is approved. The
[written design](../superpowers/specs/2026-09-19-xcode-headless-mcp-design.md)
is now revision 3 following a Not ready review, full schema capture and the
decision to proceed through hypothesis-driven implementation. See the
[review disposition](xcode-headless-review-response-2026-09-19.md) for addressed
contracts and remaining mapping/confinement questions. Advertised schemas are
captured; the first open/read iteration is specified but has not been run.
No commit, push, installation, restart, live retry, frozen snapshot/catalog edits
or run worktree changes. P070/P095 and the test-only Green main scope are unchanged.

## Incident And Read-Only Preflight

Green main run `108e6cdc-533b-45d6-84ed-459ea7d992e4`, stage
`55625510-0bbd-4971-b781-1b547edd4493`, attempt 3:

- `security_checker`, execution `8c16fe45-4581-4713-a3dc-92d563aa4ccd`, failed
  before provider launch with `xcode_target_not_found`.
- `docs_guardian`, execution `27e29d3b-f286-45cf-a09c-bf10a84c6737`, completed
  at `2026-09-19T12:24:54.575530+00:00` (fresh readback, not the earlier running
  observation).
- App-support SQLite was queried read-only after inspecting its schema. No
  pending/running agent executions or work items existed at preflight or the
  later live-probe preflight.
- `/ready`: ready, schema 99, build `a096b883`, PID `48695`, started
  `2026-09-19T12:22:50.100439Z`. Broker: healthy, zero active/queued leases,
  zero backend sessions. The same build/PID/counts remained after the probe.

The first sandboxed localhost request was unavailable; the authorized host-side
read succeeded. It was not evidence that the daemon was down.

## Installed Contract

| Observation | Verified result |
| --- | --- |
| Installed bundle | `/Applications/Xcode.app`; no other `Xcode*.app` in `/Applications` |
| Version metadata | Xcode 27.0, product build `27A266a` |
| `xcode-select -p` | `/Applications/Xcode.app/Contents/Developer` |
| Shell `DEVELOPER_DIR` | Unset; no override in the diagnostic shell |
| `xcrun --find mcpbridge` | `/Applications/Xcode.app/Contents/Developer/usr/bin/mcpbridge` |
| IDE identity | `com.apple.dt.Xcode`, executable `Xcode`, PID 27392, UID 501, PPID 1 |
| Headless identity | `com.apple.dt.mcp-server`, executable `Xcode Service`, PID 29224, UID 501, PPID 1 |
| Service bundle | `/Applications/Xcode.app/Contents/Developer/Library/Xcode/Agents/Xcode Service.app` |
| Headless status | enabled, running, `unsafeAlwaysAllowAllAgents=false`, no open workspaces |

Both processes are launchd children. Parent-child ancestry alone cannot identify
which one is the IDE. The nested service is a real, separate MCP host, not merely
an irrelevant process to kill or an alias for the IDE.

Installed `mcpbridge --help` still specifies stdio JSON-RPC without a subcommand,
optional `MCP_XCODE_PID` for explicit host selection, and optional UUID
`MCP_XCODE_SESSION_ID`. Its documented default follows xcode-select. `run-agent`
is an additional agent-launch facility, not a replacement for stdio bridging.
Installed `mcp-server --help` exposes status, workspace opening and explicit
permission administration. Only help/status were used; no enable/disable/open,
approval, permission reset, unsafe permission flag or stop operation was issued.

## Recent Apple Changes

Official sources were fetched on 2026-09-19, including their current Markdown
representations because search excerpts still described older beta revisions.

- [Xcode 26.4](https://developer.apple.com/documentation/xcode-release-notes/xcode-26_4-release-notes): fixes for external MCP configuration being overwritten during Codex initialization and repeated connection dialogs.
- [Xcode 26.5](https://developer.apple.com/documentation/xcode-release-notes/xcode-26_5-release-notes): correction to malformed `RunSomeTests` MCP data.
- [Xcode 26.6](https://developer.apple.com/documentation/xcode-release-notes/xcode-26_6-release-notes): ACP support and expanded preview capabilities.
- [Xcode 27](https://developer.apple.com/documentation/xcode-release-notes/xcode-27-release-notes): expanded MCP tools; dynamic build/test activity; a filesystem-access security layer; and, starting in beta 5, a preview headless MCP server with longer-lived signed-agent/directory permissions. Apple notes limitations in that preview's settings/permission administration. This is a new host model, not evidence that explicit IDE bridging was removed.
- [External-agent setup](https://developer.apple.com/documentation/xcode/giving-external-agents-access-to-xcode): the IDE path still uses `xcrun mcpbridge`, external-agent permission and an open project.

The precise release that introduced the observed service executable was not
established by an old installed binary. The beta-5 milestone above is Apple's
published headless feature history; the local host behavior is independently
verified below.

## Root Cause And Host Comparison

At the source base, `is_xcode_process_command` accepted any command containing
`/Contents/MacOS/Xcode`. It therefore admitted both `Xcode` and `Xcode Service`.
Their workspace identities were unresolved by command-line and `lsof` hints.
The false second IDE then suppressed the document-path query, which runs only
for a single candidate. No workspace matched and the single-live fallback was
unavailable. That reproduces the reported preflight failure.

The project metadata was not absent: a read-only AppleScript query to the
already-running IDE returned
`/Users/user/Documents/Chainworks Forge/Chainworks Forge.xcodeproj`.
`lsof` for these PIDs returned no matching project/workspace paths.

Bounded read-only bridge probes used the same cleared host-user environment as
the broker and explicit PIDs. No provider CLI, build, test, editor mutation or
workspace-open tool was invoked:

| Probe | Result |
| --- | --- |
| IDE PID 27392 | initialize succeeded; `xcode-tools` version `25317`, protocol `2024-11-05`; 52 tools |
| IDE `XcodeListWindows` | Chainworks project present, with an opaque window tab identifier |
| Service PID 29224 | initialize succeeded; same server version/protocol; 54 tools |
| Service tool shape | `XcodeListWorkspaces`, `XcodeOpenWorkspace`, `XcodeCloseWorkspace`, `XcodeNewProject`; no `XcodeListWindows` |
| Independent `mcp-server status --format json` | No service workspaces; permissions enabled, unsafe allow-all disabled |

This disproves treating the service as a second IDE, and also disproves treating
it as harmlessly interchangeable with the current selected workspace. No
service project was opened. A first disposable Python probe hit its own 64-KiB
line-reader limit on the larger tools response; increasing that diagnostic
reader limit yielded the results above. This was not a production Rust reader
failure. Closing diagnostic stdin produced bridge exit 1 with empty stderr;
the successful RPC responses are the claimed evidence, not process exit 0.

## Superseded IDE Prototype

This section records the discarded prototype. Its checks do not verify the
requested headless migration. Independent review also identified a regression:
removing command-argument workspace evidence could broaden the existing
single-unknown fallback. `ps comm` is argv[0], not independently verified process
identity. The prototype was removed rather than published with these limitations.

The prototype change was confined to `acp/src/xcode_target.rs`: use `ps comm`
executable paths and a positive exact standalone `.app/Contents/MacOS/Xcode`
identity. Reject nested app helpers and executable prefixes without a blacklist
of service names. Renamed/beta bundles and spaces remained supported. Resolver
branches and GUI UID checks were unchanged, but removing argument-derived
workspace hints could broaden the end-to-end single-unidentified-IDE fallback,
as the independent review above identified. Open paths and document metadata
remained the prototype's workspace discovery sources.

Provider-free tests cover the actual two-process incident, helper/lookalike
rejection, renamed/beta paths, several real IDEs, helper explicit-PID rejection,
and existing workspace/UID/host-environment failures. The old command-line
fixture was updated to model `comm` rather than pretend that executable paths
contain project arguments.

All commands below ran in `control-plane` with
`CARGO_TARGET_DIR='/Users/user/Library/Caches/Chainworks Forge/cargo-target/gates/xcode-discovery-20260919'`:

```bash
../scripts/cargo-managed test -p acp --lib xcode_target::tests:: -- --nocapture
../scripts/cargo-managed test -p acp --no-fail-fast -- --test-threads=1
```

- RED before the production edit: 9 passed, 3 failed; incident fixture failed
  with the same `xcode_target_not_found`, and helpers inflated candidate counts.
- GREEN: 12 discovery tests passed.
- Complete ACP suite: 318 passed, 2 ignored (200 unit, 15 input-pressure,
  76 integration, 27 mapper tests passed). The manual live probe and existing
  reference-workspace latency check are opt-in.
- An earlier full attempt stopped at 199 passed / 1 failed:
  `adapters::tests::register_xcode_shim_grant_for_child_captures_live_process_binding`
  saw a changing executable fingerprint across two inspections of its spawned
  `/bin/sh -c 'sleep 5'` child. The later unchanged test passed. A targeted clean
  `a096b883` archive check also passed, so a baseline failure was **not** confirmed.
  The transient failure remains recorded; no unrelated test was modified.

An opt-in macOS prototype test in `xcode_broker.rs` exercised patched discovery,
the production broker spawn/request pump, and read-only MCP window listing.
It requires the expected PID and workspace, rejects fallback selection, bounds
RPC time, and closes/reaps only its own diagnostic bridge:

```bash
CHAINWORKS_XCODE_PROBE_WORKSPACE='/Users/user/Documents/Chainworks Forge' \
CHAINWORKS_XCODE_PROBE_PID=27392 \
../scripts/cargo-managed test -p acp --lib \
  live_xcode_ide_discovery_and_readonly_handshake \
  -- --ignored --nocapture --test-threads=1
```

Result: 1 passed; PID 27392, `workspace_match`, correct developer directory,
initialize accepted, 52 tools and the Chainworks project in `XcodeListWindows`.
The test was removed with the prototype; the historical command is not available
in the current tree. Do not run or re-open the IDE to repeat it.
Existing IDE, service, external bridge PIDs and daemon remained alive afterward.
This is RPC/discovery validation, not a local UI test or full-run acceptance.

## Headless Readback With IDE Closed

After the user closed the IDE, PID 27392 disappeared; service PID 29224 and
daemon PID 48695 remained. Broker leases/queue/backend sessions remained zero.
A new read-only bridge probe to the service still completed initialize and
listed 54 tools. No project was opened and no permission was changed.

Important discovered permission boundary: `XcodeListWorkspaces` returned
`isError: true`, explaining that this diagnostic agent was not yet approved.
Opening or creating a project initiates approval of the agent and project
folder. Therefore initialize/tools-list success is not workspace access proof.

Observed schemas:

- `XcodeOpenWorkspace`: required absolute `path` to `.xcworkspace` or
  `.xcodeproj`; result requires `workspaceIdentifier`, with optional
  `workspacePath`, active scheme/destination and message.
- `XcodeListWorkspaces`: no arguments; result is a message, subject to agent
  approval.
- `XcodeCloseWorkspace`: required `workspaceIdentifier`.
- `BuildProject`: optional `workspaceIdentifier` (opaque ID or absolute path)
  and `buildForTesting`.
- `XcodeRead`: required `filePath`, optional `workspaceIdentifier`, offset/limit.

The optional workspace argument must not be allowed to choose a shared global
default across leases. A migration needs broker-owned workspace binding and
cross-workspace rejection, not only discovery of the service process.

## Migration Requirements

- Headless adoption needs an explicit host kind, workspace open/readback and
  ownership, signed-agent/folder permission handling, capability/schema checks,
  and lease/session identity appropriate to that host. Do not reuse IDE window
  identifiers, choose the first process, or enable unsafe allow-all.
- The existing broker shares backends by PID plus developer directory and keeps
  them warm after the last lease; it does not key by run ID. The reference's old
  per-run/last-release claims are stale. Their prototype correction was removed
  with the rejected narrow patch; the migration must update the canonical truth.
- The broker forwards tool schemas dynamically and retains per-lease policy.
  Successful discovery does not prove every new tool, permission flow, build/test
  activity notification, provider integration or headless workspace lifecycle.
- No Swift/UI or full Rust workspace gate was run. No success of Green main is
  claimed. Its installed daemon still uses `a096b883`. The new migration has not
  been implemented or validated.

## Ops Coordination

Ops was told not to apply the intermediate IDE diff or retry it. The user now
requires a full headless-only transition, with no IDE opening or fallback.
Design must cover host lifecycle, canonical worktree/project resolution,
workspace-bound leases and calls, permissions, restart/drift handling and
readback. The managed direction is approved; product implementation awaits
review of the written specification and subsequent implementation plan.
Any later publication/deployment/retry remains a separate authorized Ops action.

## Follow-Up: Complete Schema Discovery

The user subsequently requested the exact Apple contracts behind P1-04/P1-05.
[The follow-up study](xcode-headless-contract-2026-09-19/README.md) contains raw
metadata captures for all 54 tools, a derived schema index, protocol negotiation
comparison, and an evidence-backed distinction between Apple's workspace/folder
permission model and strict checkout confinement. No workspace was opened and
no tool action was called. Raw schema absence is resolved; real binding,
response normalization, packaged consent and confinement proof remain separate.
