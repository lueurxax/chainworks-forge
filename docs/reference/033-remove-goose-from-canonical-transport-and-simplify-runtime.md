# Canonical Transport and Runtime Simplification (former P033)

## Status
- **Implemented and stabilized**: 2026-04-11
- **Primary contract owners**: transport boundary, runtime session bridge, settings migration path
- **Evidence source**: runtime transport + settings transfer implementation and `proposal-033` proof lane

## Purpose
This document captures the shipped contract after completing the canonical-transport simplification: the runtime engine and persisted run state now treat ACP as the canonical transport envelope, while provider-specific transport details remain adapter-local.

## Canonical transport contract
1. **ACP is the persistence and orchestration boundary**
   - Execution decisions are made against stable ACP-shaped payloads.
   - Canonical identifiers (`runtimeSessionID`, profile identity, resume/transition metadata) are preserved independent of provider transport internals.

2. **Goose-specific transport is removed from canonical truth**
   - Engine orchestration should not depend on Goose transport-specific object shapes for normal run ownership.
   - Goose-specific behavior remains in adapter compatibility seams where needed, not as the canonical contract.

3. **Schema-neutral settings migration and transfer compatibility**
   - Raw pre-decode migration seam exists to support legacy local and transfer payloads.
   - `SettingsTransferService` is the canonical path for persisted settings import/export and placeholder migration.
   - Provider-specific transfer payload shapes are handled by schema-aware adapters, not by spreading transport assumptions into execution core.

4. **Operator-language and surface clarity**
   - UI and logs use runtime-first terms.
   - Legacy Goose-era wording is removed from operator-facing contract and surfaced only in compatibility adapters.

## Implementation surface
- `RuntimeSessionBridge` and transport runtime entrypoints.
- `RuntimeTransport` and runtime session lifecycle wrappers.
- `SettingsTransferService` migration helpers and compatibility rewrite paths.
- Adapter adapters for provider-specific payload normalization.

## Data and safety invariants
- `runtimeSessionID` continuity is preserved across import/export and re-auth handoffs.
- Transport evolution must never silently reinterpret persisted runtime truth into another namespace.
- Backward-compatibility migrations are explicit and bounded; no typed-first migration without raw preservation.

## Evidence
- Canonical proof lane: `scripts/test-gate.sh proposal-033`
- Compatibility migration tests and transport boundary tests in runtime/session bridge suites.
- Settings transfer/rewrite tests around legacy payload formats and namespace continuity.

## Related stable docs
- [acp-runtime-transport.md](acp-runtime-transport.md)
- [provider-platform.md](provider-platform.md)
- [runtime-contract.md](runtime-contract.md)
