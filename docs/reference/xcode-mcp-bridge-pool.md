# Xcode MCP Bridge Pool

The daemon's production Xcode route is headless. It retains HTTP leases,
provider fake-home isolation and existing observation readback, but does not
resolve IDE windows, attach to an IDE process, or fall back to the old bridge.
See [headless runtime and operator procedures](xcode-headless-runtime.md) for
project trust, service prerequisites, journal authority and transition steps.
Implementation and offline verification do not establish packaged-host consent
or completed production cutover.

## Preparation And Leases

An Xcode intent remains internal until the engine prepares the exact effective
checkout and selected project, evaluates the frozen `xcode_headless` permission
profile and checks separately persisted operator trust. Preparation precedes
session-policy evaluation. Its binding digest participates in session identity.
The exact invocation and provider generation are committed before provider
startup, and an active-prompt guard is required for calls. Idle leases are not
authority to use Xcode.

The provider-facing shape is unchanged:

```json
{
  "name": "xcode",
  "type": "http",
  "url": "http://127.0.0.1:<port>/xcode-mcp/<lease_id>",
  "headers": [{"name": "Authorization", "value": "Bearer <lease-token>"}]
}
```

Tokens are hashed in broker state. Reuse retains its real endpoint/token only
after one-to-one runtime matching, current binding revalidation and exact
generation/policy agreement. Otherwise the provider session must reset. Shim
sessions are fresh-only. Broker warmup revalidates the prepared binding; it does
not perform an additional open. Old PID-keyed backend code remains fixture and
historical compatibility machinery, not the daemon's production selection.

## Routes And Tools

- `GET /xcode-mcp/health` returns coarse subsystem state, not private diagnostics.
- `POST /xcode-mcp/{lease_id}` authenticates before disclosing route state,
  enforces bounded unique-key JSON-RPC and a closed request shape.
- The admitted provider tool is `XcodeRead`, subject to frozen permissions and
  the lease's policy. The runtime supplies the explicit workspace ID.
- Lifecycle `XcodeOpenWorkspace` is internal, durable and never provider-callable.
  Other Apple tools and implicit workspace selection are not admitted.
- `CHAINWORKS_XCODE_BROKER_DISABLED=1` is fail-closed. It does not activate a
  direct stdio or IDE fallback.

Native service identity includes UID, installation, build, boot, PID and process
start time. Live mapping observations can be reused only while continuity is
preserved; persisted successful results do not reconstruct dispatch authority.
Observed close, replacement, restart, cancellation during inspection or an
uncertain operation revokes the binding. External changes between observations
remain outside the accepted trusted-local-project guarantees.

## Canonical Gates

Provider build/test work uses the authenticated `chainworks-test-gate` shim.
It accepts only the canonical checkout's script and an explicitly allowed gate,
pins server-owned root/environment/invocation, and borrows the invocation's
coordinator permit. Raw `xcodebuild`, `simctl`, `xcrun` and `mcpbridge` are denied
through this production shim route. Filesystem editing retains its existing
provider permission boundary. UI tests remain remote-only.

Workspace opening, optional one-shot service startup and canonical gates use the
same durable effect authority. Dispatch is fenced before effects; uncertainty
retains a project hold. Stable operation keys deduplicate calls without resending
effects, including after a daemon restart. Process-group settlement is required
before a gate result clears its hold. No Apple permission is granted automatically.

## Observations And Compatibility

`AgentExecution.actual_xcode_runtime_observation_json` remains the owner of
ordinary broker/shim observations. GraphQL, MCP reports, daemon status and the
Swift operator shell retain their existing typed/redacted readback. Observation
persistence failures degrade subsystem health; they do not replace the effect
journal or authorize replay.

Detailed attempt diagnostics and compare-and-swap reconciliation use dedicated
operator capabilities and run scopes via the local admin entry point. Historical
results carry their own schema identity, independently of tools admitted for new
dispatch. Restricted operators cannot acquire diagnostics or reconciliation
merely by having the Operator class.

The P051 dogfood records under `docs/evidence/051-shared-xcode-mcp-bridge-pool/`
describe the historical backend, not acceptance evidence for headless dispatch.
Historical `p051-scaffold`, `proposal-051` and `p051` gate aliases remain available.
New offline proof gates are `xcode-headless-shim`, `xcode-headless-catalog`,
`xcode-headless-project-trust` and `xcode-headless-host-startup`; see
[test gates](test-gates.md). Packaged signing, actual Apple consent and production
workflow acceptance remain separately recorded release-host obligations.
