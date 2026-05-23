# Auto-Retry Observation Ledger

P076 turns the auto-retry monitor from a free-form retry loop into an observe-only evidence surface.

The implemented contract is intentionally side-effect free: P076 records blocked-run observations, deduplicates blocker signatures, exposes latest readback, and produces rollup evidence. It does not call retry, recovery, cancellation, archive, continuation, or approval tools.

## Files

All files resolve under `CHAINWORKS_META_ROOT` when set, otherwise under the control-plane meta root:

- `.chainworks/automation/auto-retry-observations.jsonl`
- `.chainworks/automation/auto-retry-known-issues.json`
- `.chainworks/automation/auto-retry-known-issues.md`
- `.chainworks/automation/auto-retry-budget.json`
- `.chainworks/automation/auto-retry.lock`
- `.chainworks/automation/auto-retry-rollup.json`

The JSON catalog is canonical. Markdown is generated from JSON only and must not be parsed as authority.

## Ledger

Each completed poll appends one `auto-retry-observation.v1` JSON object to the ledger. Readers tolerate one partial trailing line and emit a `partial_trailing_record` diagnostic. Any earlier parse failure produces degraded readback instead of fabricated state.

`scripts/chainworks/auto_retry_observe.py` is the P076 observe-only writer. It accepts a normalized blocked-run input snapshot, acquires `.chainworks/automation/auto-retry.lock`, validates that every row remains side-effect free, appends exactly one newline-terminated JSONL observation, fsyncs the ledger, ensures the budget-state file exists, and refreshes the JSON catalog, markdown view, and rollup report. The writer intentionally contains no retry, recovery, approval, archive, cancellation, continuation, or provider dispatch hook.

P076-created observations remain observe-only:

- `retry_action` is `none` or `recommend_retry`
- `retry_result` is `not_attempted` or `not_allowed`
- `human_gate` observations never record a retry attempt
- budget-unavailable and lock/deadline failures fail closed to observation/readback

## MCP Readback

`automation.auto_retry.latest` exposes `auto_retry_readback.v1`.

The readback always echoes the six resolved path fields:

- `ledger_path`
- `budget_state_path`
- `known_issue_catalog_path`
- `generated_markdown_catalog_path`
- `lock_path`
- `rollup_report_path`

If the ledger has no history, the tool returns `no_observation_history`. If artifact reading is degraded, it returns a successful payload with diagnostics and empty arrays. Unsupported client versions fail as JSON-RPC application error `-32076` with message `unsupported_version`.

SwiftUI remains passive for this slice. Future app display must consume a compact read model derived from the same MCP/readback object; views must not parse JSONL directly or own retry-policy decisions.

## Rollup

`scripts/chainworks/auto_retry_rollup.py` reads valid ledger records and writes:

- `auto-retry-rollup.json`
- `auto-retry-known-issues.json`
- optionally `auto-retry-known-issues.md`

Rows are grouped by `blocker_signature_id`, preserving first/last seen time, occurrence count, affected runs, last stage, last observation, policy decision, retry result, evidence id, and proposed owner lane.

## Gate

`./scripts/test-gate.sh proposal-076` and alias `p076` validate:

- normative fixture and negative-fixture schema coverage
- observe-only retry invariants
- observe-only writer append behavior, newline termination, budget/catalog/rollup refresh, and human-gate no-retry behavior
- `automation.auto_retry.latest` source and focused Rust test
- rollup dedupe behavior against a real temporary JSONL ledger
- top-level readback path echo and degraded/no-history semantics
