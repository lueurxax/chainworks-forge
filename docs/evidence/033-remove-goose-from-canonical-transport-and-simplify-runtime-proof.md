# Proof: Canonical ACP Transport Simplification

## Goal
Demonstrate that canonical execution truth is ACP-shaped and transport-neutral, while provider-specific transport details remain isolated to adapters and migration compatibility layers.

## Evidence scope
- `proposal-033` gate.
- Runtime bridge/session/session-id continuity flow.
- Settings migration and transfer compatibility for legacy payloads.

## What is considered proven
1. Core orchestration and persistence no longer use Goose transport shape as canonical contract.
2. Legacy raw payload compatibility is preserved with explicit pre-decode migration seams.
3. Settings transfer/import preserves continuity and does not silently reinterpret runtime identity.
4. Operator-facing text and state surfaces use runtime-first terminology.

## Current verification commands
- `scripts/test-gate.sh proposal-033`
- transport boundary and settings migration tests in runtime-related suites

## Residual risk
- Legacy provider compatibility is intentionally adapter-local and should be validated again whenever external transport schema changes.
