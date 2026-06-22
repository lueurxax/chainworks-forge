# Execution-Truth Ownership Evidence

Stable evidence corpus for the retained `proposal-083|p083` proof gate.

The proposal document is retired. The implemented behavior is owned by
[../../reference/execution-truth-and-recovery.md](../../reference/execution-truth-and-recovery.md),
with public northbound surfaces documented in
[../../reference/mcp-northbound-control-plane-server.md](../../reference/mcp-northbound-control-plane-server.md)
and thin-client readback in
[../../reference/query-projections-and-client-consumption-contract.md](../../reference/query-projections-and-client-consumption-contract.md).

This folder retains source-controlled fixtures for:

- GraphQL and MCP lifecycle contract parity under `api/`
- command idempotency and rollback replay under `idempotency/`
- shutdown, signal side effects, cancellation, and provider identity holds under
  `shutdown/`, `shutdown-signal/`, and `cancellation/`
- durable monotonic clock behavior under `clock/`
- post-cancel late output latches under `late-output/`
- bounded operational metrics under `metrics/`
- SwiftData, macOS menu/toolbar, and manual identity-check UI proof under
  `swift/`, `macos/`, and `ui/`
- rollout contract lint/readback under `rollout-contract-v1.json` plus retained
  `p083` fixtures in `docs/evidence/rollout-contract/`

Run `./scripts/test-gate.sh proposal-083` or `./scripts/test-gate.sh p083` to
verify this corpus together with the Rust DB, engine, GraphQL, MCP, and domain
tests.
