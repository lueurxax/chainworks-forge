# Executable Rollout Gate Template

Proposal authors declare rollout intent **before implementation starts**. This template
defines the three v1 contracts, required fields, enum domains, fixture paths, and
security/path/authorization guidance. The `lint-rollout-contract` validator enforces
the contract shape; the `proposal-084|p084` test gate proves all checks pass.

## When to fill this in

Every proposal that introduces a behavior change, schema migration, new operator
surface, or new metric **must** include an inline `rollout_contract_v1` object before
implementation work is scheduled. The control-plane run-start preflight checks this
declaration and blocks work enqueue under enforce mode when the contract is absent,
invalid, stale, or tamper-suspect.

Proposals with genuinely no applicable behavior change may set
`applicability: not_applicable` with a `not_applicable_justification` explaining why.

---

## Contract 1: `rollout_contract_v1`

Declared inline in the proposal JSON (preferred) or as a sidecar JSON file. Duplicate
inline+sidecar is rejected. The linter enforces `additionalProperties: false`.

### Required fields when `applicability: required`

| Field | Type | Description |
|---|---|---|
| `schema_version` | `"rollout_contract_v1"` | Must be this exact string. |
| `applicability` | `"required"` \| `"not_applicable"` | `"not_applicable"` requires a `not_applicable_justification`. |
| `gate_aliases` | non-empty `string[]` | All `scripts/test-gate.sh` aliases registered for this proposal. |
| `commands.allowlist` | object | Declarative expected commands (never executed by linter). Shell metacharacters rejected. |
| `migrations` | object | DB migration description. Use `not_applicable: true` plus `justification` if no migration. |
| `metrics` | object | At minimum one of: `adoption_metric` (string) or `operational_metrics` (string[]). |
| `readback_lanes` | non-empty `string[]` | Lanes where operators can read the rollout decision: `"run_report"`, `"mcp"`, `"release_receipt"`, `"graphql"`. |
| `readback_fields` | non-empty `string[]` | Specific field names exposed on each lane. |
| `readback_fixture` | string | Repo-relative path to parity fixture proving operator decision surface. |
| `operator_report_fields` | non-empty `string[]` | Full list of operator report fields. |
| `hold_conditions` | `string[]` | Conditions that trigger a hold. May be empty but must be present. |
| `rollback_disposition` | object | Required fields: `mode` (string) and `data_loss_risk` (`"none"` \| `"low"` \| `"medium"` \| `"high"`). |
| `decision_vocabulary` | non-empty `string[]` | Accepted decision strings for this proposal. |
| `negative_fixtures` | object | Map of named negative fixture paths (repo-relative, no traversal). |

### Optional fields

| Field | Description |
|---|---|
| `hold_conditions_detail` | Extended descriptions for each hold condition. |
| `not_applicable_justifications` | Per-section justifications when a section is not applicable. |
| `cutover_policy` | Machine-readable cutover declaration (see below). |

### Valid example

```json
{
  "schema_version": "rollout_contract_v1",
  "applicability": "required",
  "gate_aliases": ["proposal-NNN", "pNNN"],
  "commands": {
    "allowlist": [
      "./scripts/test-gate.sh proposal-NNN",
      "./scripts/test-gate.sh pNNN"
    ],
    "commentary": "Gate commands are declarative expectations; the linter does not execute them."
  },
  "migrations": {
    "not_applicable": true,
    "justification": "No schema migration required for this change."
  },
  "metrics": {
    "adoption_metric": "new_applicable_proposals_with_passing_rollout_contract_percent",
    "operational_metrics": [
      "rollout_contract_lint_total{proposal_id,status,failure_reason}",
      "rollout_contract_run_start_block_total{proposal_id,reason,enforcement_mode}"
    ]
  },
  "readback_lanes": ["run_report", "mcp", "release_receipt", "graphql"],
  "readback_fields": [
    "rollout_contract_status",
    "rollout_contract_decision",
    "rollout_contract_failure_reasons",
    "rollout_contract_waiver_state",
    "rollout_contract_waiver_expires_at",
    "rollout_contract_enforcement_mode",
    "rollout_contract_enforcement_mode_reason",
    "rollout_contract_hold_conditions",
    "rollout_contract_rollback_disposition",
    "rollout_contract_source_lane",
    "rollout_contract_enabled_state",
    "rollout_contract_disabled_reason_code",
    "rollout_contract_action_id",
    "rollout_contract_operator_message",
    "rollout_contract_projection_integrity",
    "rollout_contract_cutover_policy_revision",
    "rollout_contract_diagnostic_redaction",
    "rollout_contract_next_steps"
  ],
  "readback_fixture": "docs/evidence/rollout-contract/operator-readback/pNNN-full-surface.fixture.json",
  "operator_report_fields": [
    "rollout_contract_status",
    "rollout_contract_decision",
    "rollout_contract_failure_reasons",
    "rollout_contract_waiver_state",
    "rollout_contract_waiver_expires_at",
    "rollout_contract_enforcement_mode",
    "rollout_contract_enforcement_mode_reason",
    "rollout_contract_hold_conditions",
    "rollout_contract_rollback_disposition",
    "rollout_contract_source_lane",
    "rollout_contract_enabled_state",
    "rollout_contract_disabled_reason_code",
    "rollout_contract_action_id",
    "rollout_contract_operator_message",
    "rollout_contract_projection_integrity",
    "rollout_contract_cutover_policy_revision",
    "rollout_contract_diagnostic_redaction",
    "rollout_contract_next_steps"
  ],
  "hold_conditions": [
    "Unresolved projection-integrity violation detected at run start",
    "Metric emission verification failed post-deploy"
  ],
  "rollback_disposition": {
    "mode": "feature_flag_disable_or_enforcement_mode_permissive",
    "data_loss_risk": "none",
    "steps": [
      "Move enforcement mode to permissive via privileged audited control-plane mutation with TTL.",
      "Keep proposal gate runnable for diagnosis.",
      "Expose permissive/disabled state in operator_readback_v1.",
      "Repair and attach re-enable evidence before TTL expiry."
    ]
  },
  "decision_vocabulary": ["pass", "fail", "waived", "not_applicable", "timeout"],
  "negative_fixtures": {
    "missing_hold_and_rollback": "docs/evidence/rollout-contract/negative/missing-hold-and-rollback.json"
  }
}
```

### Not-applicable example

```json
{
  "schema_version": "rollout_contract_v1",
  "applicability": "not_applicable",
  "not_applicable_justification": "Documentation-only change with no behavior change, schema migration, operator surface, or new metric."
}
```

---

## Contract 2: `rollout_contract_check_v1`

Emitted by the control-plane run-start preflight to
`.chainworks/runs/<run-id>/readiness/rollout-contract-check.json`. This is a
**projection** derived from authoritative storage; it must never be used as the sole
scheduling authority. The scheduler trusts the typed store, not this file.

### Fields

| Field | Type | Description |
|---|---|---|
| `schema_version` | `"rollout_contract_check_v1"` | Version sentinel. |
| `proposal_id` | string | The proposal identifier. |
| `proposal_revision_id` | string | The revision of the proposal validated. |
| `proposal_content_hash` | string | SHA-256 of the proposal JSON at check time. |
| `contract_object_hash` | string | SHA-256 of the extracted rollout_contract_v1 object. |
| `content_snapshot_id` | string | Opaque snapshot reference for audit. |
| `checker_version` | string | Semantic version of the linter/checker binary. |
| `authoritative_record_id` | string | Primary key in the control-plane readiness store. |
| `status` | enum | `pass`, `fail`, `waived`, `not_applicable`, `timeout`, `cancelled`, `missing_contract`, `tamper_detected`, `stale`. |
| `decision` | enum | `release`, `hold`, `waive`, `not_applicable`. |
| `lifecycle_state` | enum | `running`, `terminal`, `partial`. |
| `enforcement_mode` | enum | `enforce`, `permissive`, `disabled`. |
| `waiver` | object \| null | Waiver record if status=waived. |
| `diagnostics` | string[] | Human-readable diagnostic lines. |
| `failure_reasons` | string[] | Machine-readable failure reason codes. |
| `timeouts` | object | Timeout configuration used for this check. |
| `retry_count` | integer | How many times this check was retried. |
| `projection_integrity` | enum | `valid`, `tamper_detected`, `stale`. |
| `cutover_policy_revision` | string \| null | Machine-readable cutover policy revision. |
| `redaction_state` | enum | `none`, `partial`, `full`. |

The control-plane store may attach additional audit fields (`run_id`, `created_at`,
`updated_at`) to the projection for traceability. These are not part of the contract
surface authors declare and are not used as scheduling authority.

### Valid example

```json
{
  "schema_version": "rollout_contract_check_v1",
  "proposal_id": "proposal-NNN",
  "proposal_revision_id": "pNNN-r1",
  "proposal_content_hash": "sha256:abc123...",
  "contract_object_hash": "sha256:def456...",
  "content_snapshot_id": "snap-20260502-abc",
  "checker_version": "1.0.0",
  "authoritative_record_id": "rc-check-00000000-0000-0000-0000-000000000001",
  "status": "pass",
  "decision": "release",
  "lifecycle_state": "terminal",
  "enforcement_mode": "enforce",
  "waiver": null,
  "diagnostics": [],
  "failure_reasons": [],
  "timeouts": {
    "preflight_timeout_seconds": 45,
    "infrastructure_retry_max": 3
  },
  "retry_count": 0,
  "projection_integrity": "valid",
  "cutover_policy_revision": "p084-cutover-v1",
  "redaction_state": "none"
}
```

---

## Contract 3: `operator_readback_v1`

The canonical operator decision surface. Emitted across run report, MCP, release
receipt, and GraphQL projection. The control-plane store derives the readback from the
authoritative `rollout_contract_check` record per lane via a single
`operator_readback_json_for_lane(...)` accessor, so every lane sees the same decision
fields. The `run_report`, `mcp`, and `release_receipt` lanes use the canonical
snake_case keys below; the `graphql` lane returns a lossless camelCase projection of
the same payload (e.g. `backend_decision` → `backendDecision`,
`rollback_disposition.data_loss_risk` → `rollbackDisposition.dataLossRisk`). Field
values remain canonical strings on every lane.

### Fields

| Field | Type | Description |
|---|---|---|
| `schema_version` | `"operator_readback_v1"` | Version sentinel. |
| `status` | string | Human-readable status. |
| `backend_decision` | enum | `release`, `hold`, `waive`, `not_applicable`. |
| `failure_reasons` | string[] | Machine-readable failure reason codes. |
| `waiver_state` | enum | `none`, `active`, `expired`, `revoked`. |
| `waiver_expires_at` | ISO-8601 string \| null | When the waiver expires. |
| `enforcement_mode` | enum | `enforce`, `permissive`, `disabled`. |
| `enforcement_mode_reason` | string \| null | Reason code for permissive/disabled mode (e.g. waiver reason or operator override). |
| `hold_conditions` | string[] | Active hold conditions. |
| `rollback_disposition` | object | `mode`, `data_loss_risk`, and optional `steps` carried from the contract. |
| `enabled_state` | enum | `enabled`, `disabled`. |
| `disabled_reason_code` | string \| null | Why the rollout gate is disabled. |
| `action_id` | string | Idempotency key for the operator decision. |
| `operator_message` | escaped plain text \| null | Operator-authored guidance. Never HTML or raw diagnostics. |
| `source_lane` | enum | `run_report`, `mcp`, `release_receipt`, `graphql`. |
| `projection_integrity` | enum | `valid`, `tamper_detected`, `stale`. |
| `cutover_policy_revision` | string \| null | Machine-readable cutover policy revision tied to this decision. |
| `diagnostic_redaction` | enum | `none`, `partial`, `full`. |
| `next_steps` | string[] | Operator-actionable recovery guidance. |

Lane payloads also carry traceability fields derived from the authoritative record
(`authoritative_record_id`, `run_id`, `proposal_id`, `proposal_revision_id`,
`waiver`, `adoption_metric`, `updated_at`). These are projection metadata, not
operator decision inputs; the 18 fields above are the canonical decision surface.

### Valid example

```json
{
  "schema_version": "operator_readback_v1",
  "status": "pass",
  "backend_decision": "release",
  "failure_reasons": [],
  "waiver_state": "none",
  "waiver_expires_at": null,
  "enforcement_mode": "enforce",
  "enforcement_mode_reason": null,
  "hold_conditions": [],
  "rollback_disposition": {
    "mode": "feature_flag_disable_or_enforcement_mode_permissive",
    "data_loss_risk": "none",
    "steps": [
      "Move enforcement mode to permissive via privileged audited control-plane mutation with TTL."
    ]
  },
  "enabled_state": "enabled",
  "disabled_reason_code": null,
  "action_id": "rollout_contract_check:00000000-0000-0000-0000-000000000001",
  "operator_message": "Rollout contract preflight passed; implementation scheduling may continue.",
  "source_lane": "run_report",
  "projection_integrity": "valid",
  "cutover_policy_revision": "p084-cutover-v1",
  "diagnostic_redaction": "none",
  "next_steps": ["continue_implementation_scheduling"]
}
```

### Parity-lane fixture

`readback_fixture` references a single fixture that proves the same decision surface
across every lane. The fixture **must** include a `parity_lanes` object whose
`run_report`, `mcp`, and `release_receipt` payloads carry the canonical snake_case
fields and whose `graphql` payload carries the matching camelCase projection. The
`proposal-084|p084` gate validates parity-lane shape; the canonical fixture for P084
itself lives at
`docs/evidence/rollout-contract/operator-readback/p084-full-surface.fixture.json`.

The Swift app decodes operator readback into a read-only `RolloutDecisionSummary`
projection: `PreflightReportView` consumes the snake_case payload from run reports,
and the GraphQL run-row read model decodes the camelCase projection. Swift never
recomputes a decision; it presents the backend authority.

---

## Cutover Policy Template

Include a `cutover_policy` in `rollout_contract_v1` when the proposal needs a machine-readable
effective cutover timestamp and enforcement mode transition:

```json
"cutover_policy": {
  "revision": "p084-cutover-v1",
  "enforcement_mode_at_cutover": "enforce",
  "applicable_to": "post_cutover_implementation_starts",
  "grandfathered_rendering": "not_applicable",
  "effective_timestamp_iso8601": "2026-06-01T00:00:00Z"
}
```

Cutover is a privileged, audited control-plane mutation. The `effective_timestamp_iso8601` is
informational; actual enforcement is controlled by the stored enforcement mode, not this field.

---

## Security and Path/Authorization Guidance

### Safe paths

- All paths in rollout contracts **must** be repo-relative (no leading `/`).
- Parent-traversal (`..`) is rejected by the linter.
- Symlink escapes outside the repository root are rejected at scheduling time.
- Paths that resolve outside `docs/`, `scripts/`, or declared `CHAINWORKS_META_ROOT`
  targets fail the unsafe-path check.
- `readback_fixture` and every value under `negative_fixtures` must resolve under
  `docs/evidence/rollout-contract/`. The linter rejects anything outside that root,
  including symlinks whose canonical target escapes it.

### Safe commands

- Commands in `commands.allowlist` are **declarative expectations only** — the linter
  never executes them and neither does the run-start preflight.
- Shell metacharacters (`|`, `&`, `;`, `$`, `` ` ``, `>`, `<`, `(`, `)`, `{`, `}`)
  are rejected in command strings.
- Absolute paths in commands are rejected.
- Commands must begin with `./scripts/` or be a well-known repo-relative entry point.

### Authorization

- Waivers, enforcement-mode changes (enforce → permissive → disabled), and re-enable
  decisions are **privileged, audited control-plane mutations** requiring operator
  principal identity.
- Waivers are evaluated at scheduling time only. A waiver that expires after scheduling
  remains valid historical evidence and does not unschedule in-flight work.
- Redaction of sensitive diagnostic detail requires explicit `diagnostic_redaction`
  state and authorized caller identity.

### Parser/display limits

- Rollout contract JSON files must not exceed 1 MiB.
- Control characters (code points below U+0020 except `\t`, `\n`, `\r`) in string
  values are rejected.
- Principal identifiers and raw diagnostic detail in `operator_message` must be
  redacted via the authorized detail surface; plain `operator_message` renders as
  escaped plain text only.
- Secret-like values (API keys, tokens, passwords, connection strings) must not appear
  in any rollout contract field.

---

## Linting and Gate Registration

Run the pure validator:

```bash
./scripts/lint-rollout-contract <path-to-rollout-contract.json>
```

Run the full P084 gate (checks template, fixtures, linter, self-contract):

```bash
./scripts/test-gate.sh proposal-084
./scripts/test-gate.sh p084
```

The gate is registered in `docs/reference/test-gates.md` under `proposal-084|p084`.

---

## P084 Self-Contract

P084 uses `p084_self_contract` as the inline rollout contract field in the proposal JSON.
The full operator decision surface fixture is at
`docs/evidence/rollout-contract/operator-readback/p084-full-surface.fixture.json`.
