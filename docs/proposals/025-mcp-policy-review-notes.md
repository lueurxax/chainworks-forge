# Review notes for Proposal 025: Per-Agent MCP Policy and Runtime Validation

## Overall verdict

Direction is correct.

Per-agent MCP policy is a good next step for reducing accidental extension overhead and narrowing the execution surface. The proposal is especially strong on:

- making requested vs effective MCP state explicit,
- moving away from prompt-level "don't use tools" wishful thinking,
- requiring preflight to fail closed when the runtime cannot honor the requested set,
- and treating empty MCP as a real, supported session mode.

I would approve the proposal direction, but I would tighten a few things before calling the design complete.

## Main remarks

### 1. Add a real installed-server registry / capability map

Right now the proposal talks about "known MCP namespace" and "requested extensions", but the current world actually has two different namespaces:

- conceptual / legacy names already present in `permission_profiles.*.mcp.allow` such as:
  - `github.read`
  - `connect.read`
  - `scanner.readonly`
  - `dependency_scan.readonly`
  - `secrets_scan.readonly`
- actual installed server IDs such as:
  - `summon`
  - `tom`
  - `developer`
  - `analyze`
  - `todo`
  - `extensionmanager`
  - `apps`
  - `summarize`
  - `chatrecall`
  - `code_execution`
  - `computercontroller`
  - `autovisualiser`
  - `memory`
  - `tutorial`
  - `chromedevtools`
  - `context7`
  - `xcode`
  - `orchestrator`

These are not the same thing.

The proposal needs one explicit runtime registry concept, for example:

- `mcp_server_registry`
- `server_id`
- optional capability tags
- install state / health state
- provider/runtime compatibility

Without this, preflight will validate names syntactically but still not know how to reconcile the legacy conceptual names with the actual installed server list.

### 2. Make per-agent `mcp_profile` the runtime authority and demote permission-profile MCP to a ceiling / legacy lane

Today `permission_profiles.*.mcp.allow` is too coarse because several agents share a permission profile but should not share the same MCP set.

The proposal already points in the right direction with:

- `mcp_profiles`
- `agents.*.mcp_profile`

I would make that explicit:

- `mcp_profile` is the runtime authority for session extension selection
- `permission_profiles.*.mcp.allow` becomes either:
  - a coarse ceiling, or
  - transitional legacy metadata
- runtime must never widen beyond `mcp_profile` because a permission profile happened to allow more

Otherwise the implementation will drift back to profile-wide unions and you will keep paying for extensions that only one agent needed.

### 3. Add `required_extensions` vs `optional_extensions` (or equivalent fallback policy)

Some MCP servers are truly mandatory for an agent.
Others are just useful when present.

Without this split, you get an awkward choice:

- either fail the whole run because one nice-to-have server is unavailable,
- or silently widen / soften policy and lose trust.

Suggested minimal shape:

```yaml
mcp_profiles:
  swift_code_reference:
    required_extensions:
      - xcode
    optional_extensions:
      - context7
    fallback_policy: fail_if_required_missing
```

This gives preflight and diagnostics a more honest contract.

### 4. Add burn telemetry to the proposal itself

If the whole motivation is reducing burn, Proposal 025 should not stop at policy correctness.
It should also require measurement.

Minimum additional telemetry:

- enabled MCP count per session
- session startup latency with and without MCP
- tool-call count by MCP server
- bytes returned by each MCP server
- prompt/context delta caused by MCP output
- requested vs effective vs denied extension set
- runs blocked by MCP preflight
- runs saved by zero-MCP sessions

Without this, you can land a perfect policy system and still not know whether it actually improved burn.

### 5. Default-deny should be explicit and visible in docs

This proposal should explicitly say:

- the default MCP profile is `none`
- zero-MCP sessions are the preferred baseline
- an agent must justify every enabled MCP server

That is the only safe way to keep the system from drifting back toward "everything globally available, just don't use it."

### 6. Mark high-burn / high-risk servers as opt-in only

With the current server inventory, these should stay unassigned by default and require an explicit future justification:

- `memory`
- `chatrecall`
- `computercontroller`
- `apps`
- `extensionmanager`
- `tutorial`
- `summon`
- `orchestrator`
- `autovisualiser`
- `chromedevtools`

Reasons vary:

- broad hidden-context inflation,
- side-effect risk,
- duplicated authority with the app runtime,
- or unclear burn-to-value ratio.

Proposal 025 should probably mention this "dangerous/broad server" class explicitly so the policy system is not immediately weakened by convenience.

## Recommended per-agent MCP policy

I recommend a very conservative first rollout.

### Assigned profiles

- `lead_orchestrator` -> `orchestrator_minimal`
- `code_writer` -> `swift_code_reference`
- `system_steward` -> `steward_analysis`
- all other agents -> `none`

### Why

#### `lead_orchestrator`
Recommended server:
- `todo`

Reason:
- gives the orchestrator one bounded task-structuring aid without opening broad hidden-memory or browser/desktop surfaces

#### `code_writer`
Recommended servers:
- `xcode`
- `context7`

Reason:
- `xcode` is the one clearly high-value server for a Swift/macOS code-writing lane
- `context7` is a bounded documentation/reference lane
- I intentionally did **not** add `code_execution` because deterministic shell/test/build commands already exist and duplicate execution channels often increase chatter rather than reduce it

#### `system_steward`
Recommended server:
- `analyze`

Reason:
- if any agent should have a bounded analytical tool lane, it is the system-level steward
- I intentionally did **not** add `summarize` or `memory` because those are exactly the kind of "sounds helpful, silently widens context" servers that tend to increase burn

#### Everyone else
Recommended profile:
- `none`

Reason:
- most proposal reviewers, proposal writer, auditors, security, docs, release, and prepush agents already have enough filesystem / artifact / shell context
- giving them extra MCP servers now is much more likely to increase tool chatter than to reduce burn

## Important note about current release/security MCP names

The current catalog still contains conceptual names like:

- `github.read`
- `connect.read`
- `git.commit`
- `git.push`
- `connect.upload`
- `scanner.readonly`
- `dependency_scan.readonly`
- `secrets_scan.readonly`

Those do not appear in the installed server list you provided.

That means Proposal 025 should explicitly call out a migration step:

1. either map these conceptual names to real installed MCP server IDs,
2. or remove them from the MCP runtime path and treat them as non-MCP transport/tool capabilities owned elsewhere.

If you do not fix this naming split, diagnostics and preflight will look "correct" on paper while still validating the wrong namespace.

## Suggested acceptance-criteria additions

I would add these two acceptance bullets:

1. Preflight distinguishes between:
   - installed server registry truth,
   - requested agent MCP profile,
   - and effective enabled session extension set.

2. The proposal includes burn telemetry proving that:
   - moving an agent from "global MCP" to "per-agent MCP" actually reduced session overhead or tool chatter.

## Bottom line

I would keep Proposal 025.

But I would tighten it around these ideas:

- explicit installed server registry,
- `mcp_profile` as runtime authority,
- required vs optional extensions,
- burn telemetry,
- default-deny / zero-MCP baseline,
- and an explicit migration away from the old conceptual MCP names in permission profiles.
