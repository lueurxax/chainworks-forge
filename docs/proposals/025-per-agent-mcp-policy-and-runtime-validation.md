# Proposal 025: Per-Agent MCP Policy And Runtime Validation

| Field | Value |
|---|---|
| Date | 2026-04-03 |
| Status | Draft |
| Author | Codex |
| Depends on | [015-skill-resolution-and-runtime-injection.md](015-skill-resolution-and-runtime-injection.md), [../reference/goose-server-transport.md](../reference/goose-server-transport.md), [../reference/provider-platform.md](../reference/provider-platform.md), [../reference/live-provider-execution-slice.md](../reference/live-provider-execution-slice.md), [../reference/execution-truth-and-recovery.md](../reference/execution-truth-and-recovery.md) |
| Scope | Add canonical per-agent MCP policy to the catalog, reconcile session extensions through Goose session APIs, and add preflight/runtime validation that the requested MCP contract can actually be honored. |
| Goal | Make MCP access explicit, default-deny, measurable, and runtime-enforced so agents only carry the extension surface they really need. |

---

## 1. Context and Motivation

`Chainworks Forge` currently has two incompatible MCP truths:

1. conceptual permission-profile names such as `github.read`, `scanner.readonly`, and `connect.upload`
2. actual Goose session extension IDs such as `developer`, `xcode`, `context7`, and `summon`

Those are not the same namespace, and today the runtime authority is still too fuzzy.

Current gaps:

1. Goose can start sessions with globally enabled extensions even when an agent should run with zero MCP.
2. Prompt-level “do not use MCP” instructions do not prevent extension startup overhead.
3. `permission_profiles.*.mcp.allow` is too coarse to be the runtime truth because many agents share a permission profile but should not share the same session extension set.
4. Preflight can validate provider/model compatibility, but it cannot yet validate whether the selected runtime can honor the requested MCP contract.
5. The system cannot currently explain repo-owned server mapping truth, machine-local runtime capability truth, requested MCP policy, and effective enabled session extensions as separate layers.

The immediate workaround is already clear: create the Goose session and remove all extensions before prompt submission. Proposal 025 defines the durable model that should replace this blanket cleanup with an explicit per-agent runtime contract.

Proposal 025 also intersects directly with [Proposal 015](./015-skill-resolution-and-runtime-injection.md). Proposal 015 makes `skill_ref` and `skill_role` runtime-authoritative; Proposal 025 must do the same for `mcp_profile`. Together they define the full execution contract for an agent:

- resolved skill content and role-specific injection
- resolved MCP profile and session extension reconciliation
- preflight/runtime validation that both can actually be honored

---

## 2. Product Questions This Proposal Must Answer

After implementation, the system must be able to answer:

1. Which MCP extensions is a given agent explicitly requesting?
2. Which of those extensions are required vs optional?
3. Which installed runtime/server registry entries correspond to those requested names?
4. Can two agents using the same provider have different MCP extension sets?
5. Can preflight stop a run before launch when the selected runtime cannot honor the requested MCP contract?
6. Can diagnostics explain repo-owned server mapping, machine-local runtime capability, requested policy, and effective enabled session state separately?

---

## 3. Scope

This proposal includes:

- a canonical repo-owned MCP server registry
- a canonical per-agent MCP profile model in the catalog
- runtime/session application of that MCP policy for Goose-backed sessions
- machine-local runtime capability validation in preflight/diagnostics
- preflight validation of requested MCP against both registry mapping truth and runtime capability truth
- operator-visible diagnostics and telemetry for requested vs effective MCP state

This proposal does **not** include:

- a generic plugin marketplace or arbitrary extension authoring flow
- UI for interactive editing of agent MCP policy
- non-Goose runtime implementation beyond capability declaration hooks
- replacing provider/model validation logic already in place

---

## 4. Design Principles

1. Default deny. The default MCP profile is `none`.
2. Fail closed. If required MCP cannot be honored, preflight blocks launch.
3. Session truth, not wishful prompting. Runtime must reconcile session extensions before prompt submission.
4. Agent-level authority. `agents.*.mcp_profile` is the runtime authority for session extension selection.
5. Permission-profile MCP is legacy metadata or a coarse ceiling, never a widening source.
6. Installed-registry truth, requested profile truth, and effective enabled-session truth must remain separately inspectable.

---

## 5. Proposed Design

### 5.1 Installed server registry becomes explicit

The catalog needs one explicit repo-owned server registry, for example `mcp_server_registry`.

That registry should answer:

- what the canonical server ID is
- which runtime families know how to map to it
- what the runtime-local server ID is for each supported family
- whether the server is normal, high-burn, or high-risk
- whether the server is opt-in only

This is necessary because the current permission-profile namespace and the real installed Goose extension namespace do not match.

Without this registry, preflight would only validate names syntactically and still not know what runtime/server truth it is validating against.

Important split:

- `mcp_server_registry` is repo-owned mapping truth and policy metadata,
- machine-local runtime capability belongs to provider/runtime diagnostics and preflight state,
- the registry may declare that a runtime family has a mapping for a server, but it must not claim that the current machine/runtime can actually honor session-scoped reconciliation.

Examples of capability that do **not** belong in repo YAML:

- whether the current Goose installation supports session-scoped enable/disable,
- whether the current machine can enumerate enabled extensions,
- whether the currently configured runtime can distinguish unavailable vs installed-but-disabled,
- whether the current runtime build is temporarily degraded or disconnected.

Those are machine-local runtime facts and must remain in the same provider-platform capability/preflight lane already described by [../reference/provider-platform.md](../reference/provider-platform.md).

### 5.2 Per-agent `mcp_profile` becomes runtime authority

Runtime authority must be:

1. `agents.*.mcp_profile`
2. referenced `mcp_profiles.*`
3. runtime reconciliation against the installed registry and selected runtime capability model

`permission_profiles.*.mcp.allow` should be explicitly demoted to one of:

- legacy conceptual metadata, or
- a coarse ceiling that can only narrow access, never widen it

The runtime must never widen beyond `mcp_profile` because a permission profile happened to allow more.

This should mirror Proposal 015 in intent: once 015 lands, `skill_ref` is no longer metadata. Proposal 025 applies the same bar to `mcp_profile`.

The ownership split must stay explicit:

- catalog YAML owns declared agent intent (`agents.*.mcp_profile` and referenced `mcp_profiles.*`),
- `RunStartSnapshot` / frozen run-start state owns the normalized requested MCP contract selected for this run,
- preflight may add a predicted effective set and blocking/warning verdict for start-time decision making,
- `AgentExecution` owns the actual runtime-reconciled enabled/denied set for the session that really ran,
- reports and diagnostics must prefer persisted run/execution truth over heuristic reconstruction from receipts.

### 5.3 MCP profiles need required/optional/fallback semantics

Each MCP profile should declare:

- `required_extensions`
- `optional_extensions`
- `fallback_policy`

Recommended minimum fallback semantics:

- `fail_if_required_missing`
- `allow_without_extensions`

This avoids the false choice between:

- blocking a run because one nice-to-have server is missing
- or silently softening the contract with no explicit policy

Example:

```yaml
mcp_profiles:
  review_architecture:
    required_extensions:
      - xcode
    optional_extensions:
      - context7
    fallback_policy: fail_if_required_missing
```

The effective requested set is:

- `required_extensions`
- union `optional_extensions` that are available and supported

The catalog should not duplicate a separate `extensions` field unless it is explicitly treated as derived output rather than source-of-truth input.

### 5.4 Session extension state must be reconciled before prompt submission

For Goose-backed sessions, `Chainworks Forge` should apply the requested extension set through Goose session APIs.

Required behavior:

1. create the session
2. bind provider/model
3. reconcile session extensions to the requested MCP profile
4. only then submit the task prompt

This must support:

- empty extension sets
- agent-specific extension sets
- different extension sets across concurrent sessions

The immediate tactical version already exists:

- create session
- remove all extensions

Proposal 025 replaces that blanket removal with precise reconciliation to the requested profile.

Ordering relative to Proposal 015 matters. The runtime-prepared execution flow should be:

1. resolve skills and role-specific injected content
2. resolve MCP profile against registry and runtime capability
3. create the session and reconcile extensions
4. submit the final prompt/context payload

That keeps skill injection and MCP policy as coordinated parts of one execution contract rather than two unrelated side channels.

### 5.5 Preflight must validate runtime capability, installed registry truth, and requested profile separately

Preflight should validate three independent layers:

1. Installed registry truth:
   - are the requested server IDs present in the runtime registry?
2. Requested profile truth:
   - does the selected `mcp_profile` reference valid server IDs and a valid fallback policy?
3. Runtime capability truth:
   - can the selected runtime reconcile session-level MCP state for this server set?

Examples:

- Goose-backed provider with requested `[]`: valid.
- Goose-backed provider with requested `xcode` and server installed: valid.
- Goose-backed provider with requested `context7` but server unavailable:
  - fail if `context7` is required
  - continue only if it is optional and the fallback policy allows it
- Future runtime without session-level MCP control:
  - fail preflight for any non-empty requested set unless it can prove equivalent support

Preflight must stay within prediction authority.

The explicit owner split is:

1. requested profile truth:
   - owned by catalog selection and frozen run-start snapshot
2. predicted effective set:
   - owned by preflight/start-time diagnostics
   - may say which required/optional entries are expected to reconcile on this machine before launch
3. actual enabled set:
   - owned by `AgentExecution` after session creation and reconciliation
   - may differ from prediction only when runtime truth changed between preflight and actual launch, and that divergence must stay visible rather than silently repaired in reports

This prevents preflight from overclaiming runtime truth and prevents report readers from reconstructing MCP state from receipts alone when the execution row already persisted the settled answer.

### 5.6 Diagnostics must show requested vs effective MCP

Operator diagnostics should expose:

- selected `mcp_profile`
- required extensions
- optional extensions
- fallback policy
- installed registry verdict for each requested extension
- runtime capability verdict
- predicted effective enabled set at preflight/run start
- actual enabled session extension set after reconciliation
- denied / missing / dropped optional extensions

This is necessary because:

- globally installed
- requested by the agent
- predicted to be enabled on this machine at launch time
- and enabled in this session

are different truths.

This should appear next to Proposal 015 skill truth in the same inspection surface:

- resolved skill type/content/role
- selected MCP profile
- effective enabled session extensions

The operator should be able to inspect both as one agent-execution contract.

### 5.7 Burn telemetry is a first-class acceptance requirement

Proposal 025 should not stop at policy correctness. It should also require measurement.

Minimum telemetry:

- requested MCP count per session
- effective enabled MCP count per session
- session startup latency by extension set
- tool-call count by extension/server
- bytes returned by each extension/server
- prompt/context delta attributable to extension output
- requested vs effective vs denied extension set
- runs blocked by MCP preflight
- runs completed with zero-MCP sessions

Without this, the system can land a correct policy model and still fail to prove that burn actually went down.

This telemetry must extend the existing run-owned KPI/report lane rather than creating a free-floating MCP metrics blob.

Recommended ownership split:

- `AgentExecution` persists per-session MCP telemetry and settled reconciliation truth,
- `Run` persists the normalized MCP KPI/export summary alongside the existing session/strategy KPI lane,
- `RunReportView`, `RunComparisonView`, and other shell-owned report consumers read that persisted KPI/report lane first,
- raw receipts or ad hoc diagnostics may explain the numbers, but they must not become the canonical post-run metrics authority.

Suggested normalized MCP KPI fields:

- requested extension count,
- predicted effective extension count,
- actual enabled extension count,
- denied/missing count,
- startup latency attributable to reconciliation,
- tool-call count and bytes by server,
- prompt/context delta attributable to MCP output,
- zero-MCP execution count,
- preflight-blocked run count due to MCP incompatibility.

### 5.8 High-burn / high-risk servers remain opt-in only

The following class should stay unassigned by default and require explicit future justification:

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

- hidden-context inflation
- wide side-effect surface
- duplicated authority with the app runtime
- or weak burn-to-value ratio in the default workflow

This does **not** ban them forever. It means the catalog must opt them in explicitly and diagnostics must make that visible.

---

## 6. Catalog Shape

Recommended direction:

- `mcp_server_registry`
- `mcp_profiles`
- `agents.*.mcp_profile`

The catalog example below intentionally keeps only repo-owned mapping and assignment metadata.
Machine-local runtime capability such as session-scoped reconciliation support belongs to runtime/preflight diagnostics, not to this YAML block.

Example:

```yaml
mcp_server_registry:
  xcode:
    runtime_ids:
      goose: xcode
    risk_class: normal
    assignment_policy: explicit_opt_in

mcp_profiles:
  none:
    required_extensions: []
    optional_extensions: []
    fallback_policy: allow_without_extensions

  review_architecture:
    required_extensions:
      - xcode
    optional_extensions:
      - context7
    fallback_policy: fail_if_required_missing

agents:
  - id: proposal_reviewer_architect
    mcp_profile: review_architecture
```

---

## 7. Runtime Capability Model

The provider/runtime layer should advertise MCP capability in a structured way.

Capability questions:

- does this runtime support session-scoped extension reconciliation?
- does it support zero-extension sessions?
- can it add/remove extensions after session creation?
- can it enumerate currently enabled extensions?
- can it distinguish installed-but-disabled vs unavailable?

This should not be inferred from model name or provider family alone. It belongs in runtime capability metadata.

That metadata is machine-local and belongs in the provider/runtime diagnostics layer, not in `mcp_server_registry`.

Proposal 025 therefore needs two separate capability answers:

1. repo-owned mapping answer:
   - does this server have a runtime-family mapping at all?
2. machine-local runtime answer:
   - can the currently configured runtime on this machine actually honor session-scoped MCP reconciliation for this run?

Both are required for truthful preflight.

---

## 8. Migration Strategy

### 8.1 Immediate safe baseline

Until per-agent MCP policy is fully implemented, the safe baseline is:

- start Goose sessions
- remove all session extensions before prompt submission

This reduces accidental extension overhead and keeps zero-MCP as the default runtime baseline.

### 8.2 Promotion path

1. add `mcp_server_registry`
2. add `mcp_profiles`
3. migrate agents to `mcp_profile`
4. freeze `mcp_profile` in the same execution snapshot lineage that Proposal 015 uses for resolved skills
5. freeze predicted effective MCP set and validation verdict into the run-start/preflight lane without claiming actual runtime settlement
6. persist actual reconciled enabled/denied MCP state on `AgentExecution`
7. demote `permission_profiles.*.mcp.allow` to legacy or ceiling-only metadata
8. implement preflight validation against installed registry + runtime capability
9. ship requested-vs-predicted-vs-actual MCP diagnostics and burn telemetry through the existing KPI/report lane
8. remove blanket-removal assumptions

### 8.3 Conceptual-name cleanup

The current catalog still contains conceptual names such as:

- `github.read`
- `connect.read`
- `git.commit`
- `git.push`
- `scanner.readonly`
- `dependency_scan.readonly`
- `secrets_scan.readonly`

Proposal 025 must treat these as a separate migration lane:

1. either map them to real installed MCP server IDs
2. or remove them from the MCP runtime path entirely and treat them as non-MCP transport/tool capabilities owned elsewhere

Diagnostics and preflight must not pretend these names are already the same namespace as real Goose session extensions.

---

## 9. Acceptance Criteria

This proposal is satisfied when:

1. Agents can declare an explicit `mcp_profile` in the catalog.
2. The catalog includes an installed-server registry that stays separate from machine-local runtime capability truth.
3. `mcp_profile` is the runtime authority for session extension selection.
4. Goose-backed sessions honor that policy before prompt submission.
5. Preflight distinguishes:
   - installed registry truth
   - requested agent MCP profile
   - predicted effective enabled set for this machine/runtime
6. Actual reconciled enabled MCP state is persisted on the execution truth path rather than reconstructed from receipts.
7. Diagnostics clearly show requested, predicted, and actual MCP state plus dropped or denied extensions.
8. MCP telemetry extends the existing run-owned KPI/report lane rather than a second metrics blob.
9. Preflight fails when required MCP cannot be honored by the selected runtime.
10. Empty MCP policy results in genuinely MCP-free sessions, not just prompt-level discouragement.
11. Burn telemetry shows whether tightening MCP policy reduced session overhead or tool chatter.

---

## 10. Risks

- Overfitting to Goose-specific extension semantics if capability abstraction is weak.
- Treating globally installed extensions as equivalent to session-enabled extensions.
- Silent fallback from requested MCP policy to a broader effective set.
- Letting permission-profile metadata re-widen access after agent-level policy is defined.

The design must fail closed. If the runtime cannot honor the requested MCP set, preflight should block launch instead of silently widening access.
