# P096 — Bounded Tool Output and Safe Search Policy

Status: Draft
Owner: Control-plane runtime
Scope: ACP/provider runtime tool boundary, failure classification, reviewer/auditor prompt guardrails

## Problem

Reviewer and auditor agents can legitimately interpret instructions such as
"inspect relevant code/tests/docs" as broad repository discovery. In live Codex
sessions this has produced commands like repo-root `rg` or `find` without
excluding generated/build roots. When `control-plane/target/**`, DerivedData,
node_modules, or provider session homes are included, tool output can expand to
hundreds of thousands or millions of original tokens, poison the provider
context, and terminate the stream with `turn_aborted` or a generic provider
failure.

This is systemic, not a one-off prompt mistake:

1. Prompts invite repo-wide discovery.
2. Permission profiles allow `rg`/`find` but do not constrain command shape or
   output budget.
3. Runtime classification can still fall through to
   `prompt_stream_failed/provider_internal_error` after the provider stream is
   already damaged.

## Goals

- Prevent unsafe broad discovery before huge output enters provider/model
  context.
- Keep the generated/build denylist owned by runtime/tool policy, not only by
  prompts.
- Type any escaped pathology as `codex_unbounded_tool_output` or
  `tool_output_budget_exceeded` before generic provider/internal fallback.
- Quarantine sessions whose context may already contain unbounded tool output.
- Expose guard and policy versions in runtime health.
- Add reviewer/auditor prompt guidance that complements, but does not replace,
  runtime enforcement.

## Non-goals

- UI changes.
- Output artifact repair.
- Provider fallback or provider selection changes.
- Continuation, compaction, summarization, or context-rewrite features.
- Replacing `rg`/`find` with a new UI-level search product.

## Runtime policy

Introduce a reusable tool policy module with:

- `policyVersion = bounded-tool-output-safe-search.v1`
- `guardVersion = p096-safe-search-guard.v1`
- per-tool output budget defaults;
- cumulative output budget defaults;
- a generated/build root denylist used by runtime preflight and health readback.

Default generated/build roots:

- `control-plane/target/**`
- `**/target/**`
- `**/.build/**`
- `**/DerivedData/**`
- `**/node_modules/**`
- `**/.git/**`
- `**/.swiftpm/**`
- `**/.forge-codex-acp/**`
- `**/.junie/**`
- `**/.claude/**`
- `**/.codex/**`
- `**/*.xcresult/**`
- `**/*.dSYM/**`
- `**/build/**`
- `**/dist/**`

## Safe search preflight

Before granting provider permission for shell/search tool execution, the runtime
classifies command shape. The guard rejects broad discovery commands when they do
not include generated-root excludes.

Dangerous shapes include:

- `rg ... .`
- `rg ... control-plane docs`
- `rg ... "Chainworks Forge" control-plane docs scripts`
- `find . ...`
- `find control-plane ...`
- any search over repo/worktree roots without generated/build excludes.

The preferred behavior is fail-safe rejection, not silent rewriting. A rejected
command returns a concise typed error:

```text
tool_output_budget_preflight_denied:
Broad repository search must use bounded search and exclude generated/build roots.
Use a narrower query or the safe search tool.
Excluded roots include control-plane/target/**, **/target/**, **/.build/**, DerivedData, node_modules.
```

Narrow searches over specific source directories remain allowed. Broad searches
with explicit generated/build excludes may proceed.

## Output budgets during provider polling

The runtime must continue monitoring provider-local activity after preflight:

- per-tool output bytes;
- per-tool output lines when provider metadata reports line counts;
- cumulative function/tool output bytes per prompt turn;
- cumulative provider session-store bytes read during local-activity polling.

If any budget is exceeded, the monitor records `tool_output_budget_exceeded` and
must classify the terminal path before generic provider/internal fallback.

## Failure classification and quarantine

Failure classification order must put bounded-output pathologies before generic
provider/internal fallback:

1. `tool_output_budget_preflight_denied`
2. `tool_output_budget_exceeded`
3. `codex_unbounded_tool_output`
4. provider/session control failures
5. generic provider/internal fallback

When unbounded output has already reached provider context, the affected provider
session is unsafe to reuse. The execution should be treated as quarantined for
retry purposes: retry may create a fresh provider session, but must not resume or
continue the poisoned session. Runtime receipts should preserve the typed
classification, output metrics, and guard/policy versions.

## Runtime health

`runtime.health` should include a `toolOutputGuard` section with:

- status;
- policy version;
- guard version;
- generated-root denylist;
- max per-output bytes;
- max per-output lines;
- max cumulative output bytes.

This lets operators and tests confirm that the daemon running a live workflow has
the preventive guard installed.

## Prompt guidance

Reviewer and auditor prompts should say to inspect changed files, implicated
paths, and evidence artifacts first. They may mention safe search examples, but
prompt text is advisory only. The runtime/tool boundary remains authoritative.

## Tests and gates

Minimum regression coverage:

1. Unit test: generated-root denylist contains all required roots.
2. Unit test: preflight denies broad `rg` and `find` without excludes.
3. Unit test: preflight allows narrow searches and broad searches with generated
   excludes.
4. ACP transport test: permission request containing broad `rg` returns a typed
   `tool_output_budget_preflight_denied` JSON-RPC error.
5. ACP local-activity test: excessive function output or session-store growth
   records `tool_output_budget_exceeded` before prompt-stream fallback.
6. Engine classifier test: strings containing `tool_output_budget_exceeded`,
   `tool_output_budget_preflight_denied`, or `codex_unbounded_tool_output` map to
   `tool_output_budget_exceeded`, not `provider_internal_error`.
7. Runtime health test: `runtime.health.toolOutputGuard` reports policy version,
   guard version, denylist, and budgets.

Canonical validation should include targeted Rust tests for `domain`, `acp`,
`engine`, and `mcp-server`, plus the repository proposal gate once available.
