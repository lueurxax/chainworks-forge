# Bounded Tool Output and Safe Search Policy

**Status**: Implemented (P096; proposal retired into this reference doc — git history retains the proposal).
**Owner**: Control-plane runtime (ACP/provider tool boundary, failure classification, prompt guardrails).
**Gate**: `./scripts/test-gate.sh proposal-096` (alias `p096`).

## Why this exists

Reviewer and auditor agents can legitimately interpret "inspect relevant
code/tests/docs" as broad repository discovery. Repo-root `rg`/`find` without
generated-root excludes (over `control-plane/target/**`, DerivedData,
node_modules, provider session homes) can expand tool output to millions of
original tokens, poison the provider context, and kill the stream with
`turn_aborted` or a generic provider failure. The guard is owned by the runtime
tool boundary, not by prompt text.

## Runtime policy

The reusable tool policy module ships:

- `policyVersion = bounded-tool-output-safe-search.v1`
- `guardVersion = p096-safe-search-guard.v1`
- per-tool output budget defaults
- cumulative output budget defaults
- a generated/build root denylist shared by runtime preflight and health readback

Default generated/build roots:

`control-plane/target/**`, `**/target/**`, `**/.build/**`, `**/DerivedData/**`,
`**/node_modules/**`, `**/.git/**`, `**/.swiftpm/**`, `**/.forge-codex-acp/**`,
`**/.junie/**`, `**/.claude/**`, `**/.codex/**`, `**/*.xcresult/**`,
`**/*.dSYM/**`, `**/build/**`, `**/dist/**`

## Safe search preflight

Before granting provider permission for shell/search tool execution, the runtime
classifies command shape and rejects broad discovery commands that do not
include generated-root excludes. Dangerous shapes include `rg ... .`,
`rg ... control-plane docs`, `find . ...`, and any search over repo/worktree
roots without generated/build excludes.

Rejection is fail-safe, never a silent rewrite. A rejected command returns the
typed error `tool_output_budget_preflight_denied` with concise guidance: narrow
the query (e.g. `rg prompt_stream_failed control-plane/crates/acp/src`) or
include every exclude from
`runtime.health.toolOutputGuard.generatedRootDenylist` and cap output.

Narrow searches over specific source directories are allowed. Broad searches
with complete generated-root exclude coverage proceed.

## Output budgets during provider polling

After preflight, the runtime keeps monitoring provider-local activity:

- per-tool output bytes
- per-tool output lines (when provider metadata reports line counts)
- cumulative function/tool output bytes per prompt turn
- cumulative provider session-store bytes read during local-activity polling

Exceeding any budget records `tool_output_budget_exceeded` and classifies the
terminal path before generic provider/internal fallback.

## Failure classification and quarantine

Classification order puts bounded-output pathologies before generic fallback:

1. `tool_output_budget_preflight_denied`
2. `tool_output_budget_exceeded`
3. `codex_unbounded_tool_output`
4. provider/session control failures
5. generic provider/internal fallback

When unbounded output has already reached provider context, that provider
session is unsafe to reuse: retries may create a fresh session but never resume
the poisoned one. Runtime receipts preserve the typed classification, output
metrics, and guard/policy versions.

Preflight denial happens before output enters provider context: it stays a
distinct phase, guides the agent to narrow the command, and does **not** imply
session quarantine. `tool_output_budget_exceeded` and
`codex_unbounded_tool_output` are the fresh-session/quarantine signals.

## Runtime health readback

`runtime.health` includes a `toolOutputGuard` section: status, policy version,
guard version, generated-root denylist, max per-output bytes, max per-output
lines, max cumulative output bytes, static policy readback status, and the
configured provider wrapper enforcement status (including whether health ran an
active probe). Operators and tests use this to confirm a live daemon has the
preventive guard installed.

## Prompt guidance

Reviewer and auditor prompts direct agents to inspect changed files, implicated
paths, and evidence artifacts first, and may show safe search examples. Prompt
text is advisory only; the runtime/tool boundary stays authoritative.

## Regression coverage

The `proposal-096|p096` gate covers: denylist completeness, preflight denial of
broad `rg`/`find` without excludes, preflight allowance of narrow or properly
excluded searches, a typed `tool_output_budget_preflight_denied` JSON-RPC error
on broad-`rg` permission requests, `tool_output_budget_exceeded` recording for
excessive function output / session-store growth / wrapper truncation markers,
classifier mapping that never falls through to `provider_internal_error`, and
`runtime.health.toolOutputGuard` readback. See
[test-gates.md](test-gates.md) for the gate-by-gate statement.
