# Codex Model Variant Truth and UI Labels

Date: 2026-08-30
Status: Approved design; implementation pending

## Summary

Chainworks currently freezes the generic Codex model value `gpt-5.6` and
renders that value in Overview, Stages, and active-agent readback. The value
does not identify Sol, Terra, or Luna.

Fresh runs will use role-balanced, exact GPT-5.6 variant IDs. The Codex ACP
adapter will require the exact selected model through the provider's model
configuration option before a prompt is sent. Operator surfaces will render a
single human-readable model identity. Legacy frozen runs keep their original
bytes and are labeled as variant-unspecified instead of being guessed or
rewritten.

## Problem and Root Cause

- All current Codex backend profiles declare `model: gpt-5.6`.
- The isolated Codex runtime intentionally removes the host user's model and
  effort overrides so catalog truth controls provider execution.
- Current `codex-acp` creates a session with its provider default and exposes
  model selection through the `model` session config option.
- Chainworks currently applies the reasoning-effort config option after
  `session/new`, but does not apply the requested model config option.
- Stage topology projects the frozen task model and SwiftUI renders it without
  enough information to identify a variant.

The current UI therefore cannot honestly claim that a generic `gpt-5.6` run is
using Sol, Terra, or Luna.

## Goals

- Pin one exact GPT-5.6 variant for every fresh Codex backend profile.
- Preserve a balanced quality/cost policy by role.
- Fail session startup before prompt dispatch when an exact fresh-run model
  selection is rejected by Codex ACP.
- Use one model-label formatter across Overview, Stages, and active-agent
  readback.
- Make legacy generic model identity explicitly uncertain.
- Preserve frozen-run compatibility without mutating catalog snapshots.

## Non-Goals

- Add a runtime model-selection UI or feature flag.
- Change provider families, agent IDs, reasoning effort, temperature, turn
  budgets, escalation ordering, or permissions.
- Rewrite existing frozen run plans.
- Infer a variant from the host Codex configuration.
- Add a new GraphQL field when the existing model projection is sufficient.

## Approved Model Matrix

| Backend profile | Exact model | Existing effort | Rationale |
|---|---|---|---|
| `codex_orchestrator_high` | `gpt-5.6-sol` | `high` | Lead authority and cross-stage decisions |
| `codex_architect_high` | `gpt-5.6-sol` | `xhigh` | Architecture and API contract review |
| `codex_audit_high` | `gpt-5.6-sol` | `xhigh` | Final implementation conformance audit |
| `codex_writer_high` | `gpt-5.6-terra` | `high` | Iterative proposal authoring |
| `codex_builder_high` | `gpt-5.6-terra` | `high` | General implementation and fallback work |
| `codex_orchestrator_acp` | `gpt-5.6-terra` | `medium` | Routine orchestration |
| `codex_ops_low` | `gpt-5.6-luna` | `low` | Bounded low-cost operational work |

No other backend profile changes in this slice.

## Runtime Contract

For exact Codex model IDs in a fresh run:

1. The workflow compiler freezes the exact catalog model ID as it does today.
2. The Codex adapter sends the session request using that model identity.
3. After `session/new`, the adapter resolves the provider's `model` config
   option against the advertised option values.
4. It sends `session/set_config_option` with the exact model value as a required
   config operation.
5. Missing option support, an unknown exact model, send failure, or provider
   rejection fails session startup before prompt dispatch.
6. Runtime receipts and existing execution/session model fields retain the
   exact requested-and-accepted model ID.

The model resolver must require an exact case-insensitive value or display-name
match for this required operation. It must not use substring/token matching to
turn `gpt-5.6` into an arbitrary Sol, Terra, or Luna value.

### Legacy compatibility

Existing frozen runs that contain generic `gpt-5.6` remain readable and
resumable under their previous runtime behavior. The adapter does not invent an
exact variant for those snapshots. Their UI label is `GPT-5.6 (variant
unspecified)`.

This compatibility rule is intentionally limited to already frozen generic
snapshots. Fresh catalog compilation is covered by a guard that rejects generic
`gpt-5.6` in active Codex backend profiles.

## Readback and UI Contract

GraphQL continues to project the existing `model` field. No schema migration is
needed.

One Swift presentation formatter owns provider/model labels:

| Raw value | Display value |
|---|---|
| `gpt-5.6-sol` | `GPT-5.6 Sol` |
| `gpt-5.6-terra` | `GPT-5.6 Terra` |
| `gpt-5.6-luna` | `GPT-5.6 Luna` |
| `gpt-5.6` | `GPT-5.6 (variant unspecified)` |
| unknown non-empty model | Preserve the exact raw value |

The same formatter is used by:

- current/previous stage occurrence rows in Overview;
- the Stages topology surface;
- active-agent readback rows;
- accessibility labels derived from those rows.

For a pending task, the label describes the frozen planned model. Once an
execution exists, stage topology prefers the latest matching execution's
provider/model values so escalation or fallback cannot leave a stale planned
model on a running/completed row.

The compact row remains one line and keeps the existing order:

```text
task - status - attempts - provider - exact model - effort
```

## Failure Behavior

- Fresh generic Codex model ID: catalog/gate failure.
- Exact model unavailable in provider options: typed session-start failure,
  zero prompt dispatch.
- Provider rejects required model configuration: session-start failure, zero
  prompt dispatch.
- Legacy generic frozen run: allowed, visibly variant-unspecified.
- Missing model readback: omit the model segment rather than infer one.

## Verification

Provider-free tests must prove:

1. the approved seven-profile matrix is exact and efforts are unchanged;
2. no active fresh Codex backend profile uses generic `gpt-5.6`;
3. Codex adapter model config is required for exact variants;
4. unsupported or rejected exact model selection fails before prompt work;
5. exact matching cannot fuzzy-map generic `gpt-5.6` to a variant;
6. legacy generic snapshots preserve bytes and remain resumable;
7. formatter output covers Sol, Terra, Luna, generic legacy, missing, and
   unknown values;
8. stage topology prefers the latest execution model over stale planned model;
9. Overview/Stages and active-agent readback use the same formatter;
10. accessibility output includes the exact or variant-unspecified identity.

Run focused Swift and Rust tests through `scripts/test-gate.sh`. No remote UI or
live-provider run is required for this bounded change.

## Rollout

- The catalog change affects only newly compiled runs.
- The current run remains frozen and will display the legacy
  variant-unspecified label after the updated app is installed.
- A normal later run can provide operational observation; it is not a release
  prerequisite for this fix.
- There is no feature flag or disable path.
