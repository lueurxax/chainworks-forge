# Xcode MCP Bridge Pool

Stable reference for the implemented Proposal 051 Xcode MCP bridge pool behavior.

The bridge pool is the Chainworks-owned boundary for Xcode-capable ACP agents. It keeps provider fake-home isolation intact while moving Xcode MCP and selected Xcode shell execution through daemon-owned host-user services.

## Scope

This reference covers:

- brokered Xcode MCP intent and HTTP MCP lease attachment,
- provider capability preflight and launch/session spec separation,
- daemon-mounted Xcode MCP HTTP routes,
- broker health readback,
- Xcode runtime observations,
- shim dispatch for direct Xcode commands,
- minimum operator readback surfaces,
- and P051 verification gates.

It does not claim live dogfood sign-off. Fixture and static evidence can prove broker mechanics; live modal behavior and release-owner approval remain separate readiness evidence.

## Brokered MCP Path

Xcode MCP is represented as a broker intent until the pool reserves a lease for the execution. The intent carries the extension id, runtime id, server id, workspace/target selection hints, runtime profile, permission profile, resolved tool allowlist hash, and the requirement that the provider support HTTP MCP.

Before provider launch, the runtime attaches brokered leases and replaces broker intents with provider-facing HTTP MCP entries. The ACP `session/new` wire payload uses canonical HTTP server shape:

```json
{
  "name": "xcode",
  "type": "http",
  "url": "http://127.0.0.1:<port>/xcode-mcp/<lease_id>",
  "headers": [
    {
      "name": "Authorization",
      "value": "Bearer <lease-token>"
    }
  ]
}
```

The lease token is stored as a hash in the broker state. Readback surfaces redact endpoint tokens.

## Daemon Routes

The daemon creates one `XcodeMcpBridgePool` after SQLite preflight and listener binding, then mounts:

- `GET /xcode-mcp/health`
- `POST /xcode-mcp/{lease_id}`

The pool base URL uses the same bound daemon port as GraphQL/MCP. `CHAINWORKS_XCODE_BROKER_DISABLED=1` disables lease acquisition and records a fail-closed observation instead of falling back to direct stdio `mcpbridge`.

Broker health is subsystem health, not global daemon readiness. The snapshot includes reason code, lease-acquisition availability, active and queued lease counts, capacity, last transition time, operator message, rollback disabled state, backend availability, and observation persistence failure count. Missing broker backend is `Failed`; queue pressure, capacity saturation, or any observation append failure is `Degraded`.

## Lease And Backend Semantics

The pool owns lease state, capacity, queueing, authorization, target snapshot resolution, and observation emission.

Implemented behavior:

- reserved leases are attached before provider `session/new`,
- active lease and queue counts feed broker health,
- requests over capacity fail or wait within the configured queue timeout,
- backend initialization is serialized for a target Xcode process,
- sibling leases remain isolated by lease token and broker MCP policy,
- shutdown drains broker lease cleanup before waiting on provider session close,
- backend failures, first-connect timeouts, target ambiguity, capacity exhaustion, disabled broker state, and policy denials emit typed observations.

Provider sessions do not receive host-home access for their ordinary runtime state. Host Xcode work is routed through the broker/shim boundary.

## Shim Dispatch

Agents that declare `xcode_shim_injection_signal` receive a shim runtime. The shim grant is scoped to the launched provider process binding and active prompt state. Dispatch accepts only authorized grant use and evaluates command policy for `xcodebuild`, `simctl`, and `xcrun`.

Direct `mcpbridge` execution is rejected through the shim path. Accepted Xcode command routes are recorded as Xcode runtime observation events.

## Runtime Observation

The durable owner is `AgentExecution.actual_xcode_runtime_observation_json`.

Observation data includes:

- broker source and lease state,
- backend PID/disposition when available,
- broker endpoint with token redaction on readback,
- backend failure class and friendly status,
- target Xcode PID/workspace information when resolved,
- shim invocation and warning events,
- storage truncation/drop counters.

The domain model owns typed observation shape and redaction. The DB repository owns transactional append/update. The engine injects the observation sink into ACP so ACP does not depend on DB. After a successful late append, the sink publishes the existing stage-status invalidation event with the current stage status so GraphQL subscribers re-read the stage and agent execution rows from DB; MCP report readback sees the same persisted observation on the next pull.

If observation persistence fails, the broker increments its subsystem failure count, emits an error-level trace with metric marker `xcode_observation_persist_failed_total` and warning marker `observation_persistence_degraded`, and degrades broker health. It does not recursively try to append a warning through the same failed observation sink.

## Readback Surfaces

Current implemented readback surfaces are:

- GraphQL execution/stage readback for typed Xcode runtime observations,
- MCP `reports.get` / `report://{run_id}` readback with redacted broker endpoint,
- daemon status `xcode_broker_health`,
- Swift operator surfaces for broker health, run-timeline Xcode Runtime rows, policy warnings, and friendly failure labels.

## Catalog Signals

Agent catalog entries can declare:

- `xcode_broker_required`
- `xcode_shim_injection_signal`
- `requires_xcode_host_execution`

Xcode-capable catalog entries must make host-execution intent explicit rather than relying on direct prompt text or inherited shell behavior.

## Gates

Use the staged gate names in [test-gates.md](test-gates.md):

```bash
./scripts/test-gate.sh p051-scaffold
./scripts/test-gate.sh proposal-051
./scripts/test-gate.sh p051
```

`p051-scaffold` is the fixture/static substrate gate. `proposal-051|p051` composes the scaffold gate with the broader fixture/readback lane.

Live dogfood is intentionally outside these fixture gates. P051 pre-ship readiness still requires a parallel Xcode-capable dogfood run, modal-count evidence, observation completeness evidence, token-leakage review, and operator or release-owner sign-off.
