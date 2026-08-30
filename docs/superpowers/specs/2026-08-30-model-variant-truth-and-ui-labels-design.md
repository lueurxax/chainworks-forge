# Codex Model Variant Truth and UI Labels

Date: 2026-08-30
Status: Approved design; implementation pending

## Summary

Chainworks currently freezes the generic Codex model value `gpt-5.6` and
renders that value in Overview, Stages, and active-agent readback. The value
does not identify Sol, Terra, or Luna.

Fresh runs will use role-balanced, exact GPT-5.6 variant IDs and explicit
reasoning effort. The Codex ACP adapter will require both values through the
provider's configuration options before a prompt is sent. Operator surfaces
will render one human-readable model-and-effort identity. Legacy frozen runs
keep their original bytes and are labeled as variant-unspecified instead of
being guessed or rewritten.

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
- Pin one explicit, role-appropriate effort for every fresh Codex backend
  profile.
- Preserve a balanced quality/cost policy by role.
- Fail session startup before prompt dispatch when an exact fresh-run model or
  effort selection is rejected by Codex ACP.
- Use one model-and-effort formatter across Overview, Stages, and active-agent
  readback.
- Make legacy generic model identity explicitly uncertain.
- Preserve frozen-run compatibility without mutating catalog snapshots.

## Non-Goals

- Add a runtime model-selection UI or feature flag.
- Change provider families, agent IDs, temperature, turn budgets, escalation
  ordering, or permissions.
- Rewrite existing frozen run plans.
- Infer a variant from the host Codex configuration.
- Add a new GraphQL field when the existing model projection is sufficient.

## Approved Model and Effort Matrix

| Backend profile | Exact model | Approved effort | Rationale |
|---|---|---|---|
| `codex_orchestrator_high` | `gpt-5.6-sol` | `max` | Single lead authority and cross-stage decisions |
| `codex_architect_high` | `gpt-5.6-sol` | `xhigh` | Parallel architecture and API contract review |
| `codex_audit_high` | `gpt-5.6-sol` | `ultra` | Read-only final audit with independent evidence workstreams |
| `codex_writer_high` | `gpt-5.6-terra` | `high` | Iterative proposal authoring |
| `codex_builder_high` | `gpt-5.6-terra` | `high` | General implementation and fallback work |
| `codex_orchestrator_acp` | `gpt-5.6-terra` | `high` | Routine orchestration still requiring reliable tool judgment |
| `codex_ops_low` | `gpt-5.6-luna` | `medium` | Bounded operational work with a reasoning floor |

No other backend profile changes in this slice.

The current Codex ACP effort vocabulary is `low`, `medium`, `high`, `xhigh`,
`max`, and `ultra`. All six values remain recognized and tested. The approved
profile matrix intentionally starts at `medium`: no current Chainworks role is
safe to weaken merely to make every supported value appear in active catalog
assignments. The stable `codex_ops_low` profile ID is retained to avoid
reference churn; its frozen `effort` field, not the historical ID suffix, is
authoritative.

`ultra` is allowed only for `codex_audit_high`. It enables provider-internal
automatic task delegation, but does not create additional Chainworks stage or
agent authority and does not widen the audit agent's read-only permissions.

Capability basis for this design is a local `session/new` probe against
`codex-acp 1.1.7` on 2026-08-30. It advertised all six effort values for Sol
and Terra, and all values through `max` for Luna. [OpenAI's GPT-5.6
guidance](https://developers.openai.com/api/docs/guides/latest-model) positions
Sol for flagship work, Terra for balanced work, Luna for efficient high-volume
work, and recommends reserving `max` for the hardest quality-first workloads.
Because provider capabilities can change independently of this catalog, live
option validation remains mandatory.

## Runtime Contract

For exact Codex model IDs in a fresh run:

1. The workflow compiler freezes the exact catalog model ID and effort.
2. The Codex adapter sends the session request using that model identity.
3. After `session/new`, the adapter resolves the provider's `model` config
   option against the advertised option values.
4. It sends `session/set_config_option` with the exact model value as a required
   config operation.
5. It then sends the exact `reasoning_effort` as a second required config
   operation. Model must be applied first because changing model can change the
   supported effort set and provider default.
6. Missing option support, an unknown or incompatible value, send failure, or
   provider rejection fails session startup before prompt dispatch.
7. Runtime receipts and existing execution/session fields retain the exact
   requested-and-accepted model ID and effort.

The resolver must require an exact case-insensitive value or display-name match
for both required operations. It must not use substring/token matching to turn
`gpt-5.6` into an arbitrary Sol, Terra, or Luna value or to map an unknown
effort to the nearest supported level.

Fresh catalog validation enforces the approved pair matrix. It also rejects
`ultra` for Luna and rejects `ultra` on every profile except
`codex_audit_high`. Provider capability discovery remains the final authority:
if a future Codex ACP version stops advertising an approved value, startup
fails closed instead of silently downgrading.

### Legacy compatibility

Existing frozen runs that contain generic `gpt-5.6` remain readable and
resumable under their previous runtime behavior. The adapter does not invent an
exact variant or rewrite their frozen effort. Their UI label identifies the
model as `GPT-5.6 (variant unspecified)` and formats any existing effort
without changing it.

This compatibility rule is intentionally limited to already frozen generic
snapshots. Fresh catalog compilation is covered by a guard that rejects generic
`gpt-5.6` in active Codex backend profiles.

## Readback and UI Contract

GraphQL continues to project the existing `model` and `effort` fields. No
schema migration is needed.

One Swift presentation formatter owns provider/model/effort labels:

| Raw value | Display value |
|---|---|
| `gpt-5.6-sol` | `GPT-5.6 Sol` |
| `gpt-5.6-terra` | `GPT-5.6 Terra` |
| `gpt-5.6-luna` | `GPT-5.6 Luna` |
| `gpt-5.6` | `GPT-5.6 (variant unspecified)` |
| unknown non-empty model | Preserve the exact raw value |

Effort labels use this mapping:

| Raw value | Display value |
|---|---|
| `low` | `Low` |
| `medium` | `Medium` |
| `high` | `High` |
| `xhigh` | `Extra High` |
| `max` | `Max` |
| `ultra` | `Ultra` |
| unknown non-empty effort | Preserve the exact raw value |

The same formatter is used by:

- current/previous stage occurrence rows in Overview;
- the Stages topology surface;
- active-agent readback rows;
- accessibility labels derived from those rows.

For a pending task, the label describes the frozen planned model. Once an
execution exists, stage topology prefers the latest matching execution's
provider/model/effort values so escalation or fallback cannot leave stale
planned identity on a running/completed row.

The compact row remains one line and keeps the existing order:

```text
task - status - attempts - provider - exact model - effort
```

## Failure Behavior

- Fresh generic Codex model ID: catalog/gate failure.
- Unapproved model/effort pair, including Luna + Ultra or Ultra outside the
  audit profile: catalog/gate failure.
- Exact model or effort unavailable in provider options: typed session-start
  failure, zero prompt dispatch.
- Provider rejects either required configuration: session-start failure, zero
  prompt dispatch.
- Legacy generic frozen run: allowed, visibly variant-unspecified.
- Missing model readback: omit the model segment rather than infer one.

## Verification

Provider-free tests must prove:

1. the approved seven-profile model/effort matrix is exact;
2. no active fresh Codex backend profile uses generic `gpt-5.6`;
3. all six recognized effort values have parser/formatter coverage;
4. Codex adapter model and effort configs are required for fresh exact
   variants and are applied in that order;
5. unsupported or rejected exact model/effort selection fails before prompt
   work;
6. exact matching cannot fuzzy-map a generic model or unknown effort;
7. Luna + Ultra and Ultra outside `codex_audit_high` fail catalog validation;
8. legacy generic snapshots preserve bytes and remain resumable;
9. formatter output covers all model and effort labels, missing values, and
   unknown values;
10. stage topology prefers the latest execution model and effort over stale
    planned values;
11. Overview/Stages and active-agent readback use the same formatter;
12. accessibility output includes exact model and effort identity.

Run focused Swift and Rust tests through `scripts/test-gate.sh`. No remote UI or
live-provider run is required for this bounded change.

## Rollout

- The catalog change affects only newly compiled runs.
- The current run remains frozen and will display the legacy
  variant-unspecified label after the updated app is installed.
- A normal later run can provide operational observation; it is not a release
  prerequisite for this fix.
- There is no feature flag or disable path.
