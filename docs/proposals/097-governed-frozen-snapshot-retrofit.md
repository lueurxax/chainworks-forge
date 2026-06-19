# Proposal 097: Governed Frozen Snapshot Retrofit

## Status

Proposed / emergency implementation in progress.

## Context

Chainworks intentionally freezes workflow and agent catalog snapshots at run start. That invariant protects replay, auditability, drift review, and steward metrics. A live incident exposed a narrow counter-pressure: several blocked runs were created before a P058 escalation policy was added to `examples/agents/agents.yaml`. Cancelling and restarting those runs would preserve the model but discard expensive provider work and burn additional quota.

Direct SQLite mutation is not acceptable. If an operator must retrofit a frozen snapshot during an incident, the operation must be explicit, journaled, idempotent, bounded, and visible to steward analysis.

## Decision

Add an operator-only MCP repair command, `runs.retrofit_catalog_snapshot`, for emergency catalog snapshot replacement under strict guardrails.

The first supported scope is `escalation_policy_only`. It exists to backfill P058 escalation policy declarations and the backend profiles they reference into blocked runs that were admitted before the catalog policy was present.

This is not a normal workflow practice. New catalog/workflow changes still apply only to new runs by default.

## Contract

The MCP command requires:

- `run_id`
- `expected_catalog_snapshot_hash`
- `reason`
- `scope=escalation_policy_only`
- `idempotency_key`

The engine must fail closed unless all of these are true:

- caller is an operator;
- run exists and is `blocked`;
- run has no pending or running work items;
- frozen catalog hash equals `expected_catalog_snapshot_hash`;
- run has frozen workflow/catalog snapshot JSON and current YAML paths;
- current workflow YAML compiles and its hash still equals the frozen workflow hash;
- current catalog differs from the frozen catalog only in `escalation_policies` and `backend_profiles`;
- the current catalog compiles to at least one enabled escalation policy for the run's current state.

On success, the command:

- updates only `runs.catalog_snapshot_json` and `runs.catalog_snapshot_hash`;
- records `command_journal` and MCP idempotency evidence;
- appends an `audit_log` row with old/new hashes, scope, reason, and applied policy IDs;
- rebuilds run projections for readback parity.

## Steward Expectations

Steward should treat this command as an emergency repair event, not as ordinary run progress. Future steward analysis should:

- count retrofit frequency per window;
- surface runs whose progress depended on retrofitted snapshots;
- warn if retrofit rate becomes non-zero outside incident windows;
- avoid interpreting post-retrofit throughput as a clean baseline unless the analysis explicitly includes retrofit events.

## Non-Goals

- No general workflow snapshot rewrite.
- No arbitrary JSON patch support.
- No UI write surface.
- No raw DB repair scripts.
- No automatic retrofit during startup recovery.

## Acceptance Criteria

- MCP exposes `runs.retrofit_catalog_snapshot` and Codex alias `runs_retrofit_catalog_snapshot`.
- Non-operator callers cannot execute it.
- Active or non-blocked runs are rejected.
- Hash mismatch rejects before mutation.
- Workflow drift rejects before mutation.
- Non-escalation catalog drift rejects before mutation.
- Successful retrofit produces command journal, audit log, updated catalog hash, and projection rebuild.
