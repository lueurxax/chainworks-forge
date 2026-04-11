# Runtime Profiles and Provider-Family Rollout (former P030)

## Status
- **Implemented and stabilized**: 2026-04-11
- **Primary contract owners**: runtime profile compiler, provider adapter factory, preflight/recovery services
- **Evidence source**: execution engine code and `proposal-030` proof lane

## Purpose
This document replaces the old proposal text for second-wave runtime profile support. It defines the current contract for provider families that are part of the Codex/Auggie/Junie ACP lane and how they are represented in the catalog + runtime plumbing.

## Canonical truth
- Provider profiles in catalog are authoritative for execution profile behavior.
- The transport layer must dispatch by `ProfileFamily`/runtime family identifier before any launch-time behavior.
- Unknown families must fail closed with a dedicated adapter-family failure path.
- `RuntimeProfile.requires` is a normative capability map: it gates launch, startup preflight, and MCP-policy reconciliation checks.

## Execution contract
1. **Family-aware profile model**
   - ACP profile families are represented in `runtime_profiles` and interpreted in one canonical path.
   - Profiles include second-wave families (`codex_acp`, `auggie_cli_acp`, `junie_cli_acp`) with distinct preflight and provider-readiness constraints.

2. **Dispatch and adapter selection**
   - Dispatch selects an adapter for the profile family.
   - Registry mismatch or unsupported provider family is treated as an execution failure before launch.
   - Adapter behavior and runtime contract are shared across families through normalized capability checks.

3. **Run-time requirements and rollout states**
   - Required capabilities are evaluated before run execution and reflected in run readiness.
   - Disabled, configured, and unavailable states are distinct in operator-facing behavior (not collapsed into a single message path).
   - Reconciliation with MCP/approval/state metadata is driven by profile capabilities and is no longer treated as an ad-hoc path.

4. **Readiness and recovery continuity**
   - If a profile family is known but lacks runtime requirements, run execution is blocked with the deterministic readiness contract.
   - If a profile is disabled, it remains discoverable with explicit disabled state.

## Implementation surface
- `RuntimeProfile` and registry-backed profile definitions in runtime setup flows.
- `ProviderAdapterFactory` adapter selection and fallback behavior.
- `MCPPolicyRuntime` and preflight checks that enforce `requires`.
- Provider settings and catalog surfaces consumed by `PreflightService` and startup gates.

## API / config expectations
- Catalog entries should define second-wave families explicitly and consistently.
- Capability tokens in catalog must map to executable checks in the readiness and runtime policy layers.
- Family-specific configuration fields are normalized at the runtime boundary; transport-specific schema noise is not propagated into run-level persistence.

## Backward compatibility
- Existing first-wave families remain supported where present.
- Unsupported-family failures are explicit and reproducible, preserving fail-closed behavior.

## Evidence
- Canonical proof lane: `scripts/test-gate.sh proposal-030`
- Runtime-family/factory coverage: `Chainworks Forge/Engine` runtime-adapter and preflight pathways.
- Profile and readiness contracts exercised in `Chainworks ForgeTests` runtime plan/preflight flows.
- UI-facing provider state surfaced via runs/profiles and readiness indicators rather than ad-hoc transport assumptions.

## Related stable docs
- [acp-runtime-transport.md](acp-runtime-transport.md)
- [provider-platform.md](provider-platform.md)
- [runtime-contract.md](runtime-contract.md)
