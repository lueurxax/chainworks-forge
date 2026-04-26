# Proposal 047: YAML Validation and Definition Inspection API

| Field | Value |
|---|---|
| Date | 2026-04-17 |
| Status | Draft |
| Author | Andrey Khasanov |
| Depends on | [043-query-projections-and-client-consumption-contract.md](043-query-projections-and-client-consumption-contract.md) |
| Scope | Add standalone YAML validation, dry-run compilation, and definition inspection to MCP and GraphQL, including execution-order sorting, agent grouping, semantic diffing, and cost estimation. |
| Goal | The operator can validate, preview, and compare workflow and catalog definitions without starting a run, catching errors earlier and understanding impact before execution. |

---

## 1. Context and Motivation

The control-plane compiles workflow YAML and agent catalog YAML internally during `runs.start` (in `workflow/src/compiler.rs`). Validation happens as part of compilation. But there are **no standalone APIs** to validate or inspect these definitions without starting a run.

The Swift app has `YAMLParser`, `YAMLValidator`, `WorkflowInspectorView`, and `AgentCatalogView`. These are purely client-side — the Swift app parses the YAML files itself and renders the results. Moving this logic to the control-plane has three advantages:

1. **Single source of truth** — validation rules live in one place, not duplicated between Swift and Rust.
2. **Pre-commit verification** — an MCP client (e.g., an agent editing YAML) can validate changes before committing.
3. **New capabilities** — dry-run compilation, cost estimation, and semantic diffing are computationally intensive and better suited to the server.

---

## 2. Product Questions This Proposal Must Answer

1. Can the operator validate a workflow + catalog pair without starting a run?
2. Can the operator preview the full compiled run plan (state count, agent bindings, execution order)?
3. Can the operator see estimated cost ranges before execution?
4. Can the operator inspect workflow states in execution order (not alphabetical)?
5. Can the operator browse agents grouped by functional role?
6. Can the operator compare two definition versions and see a semantic diff?
7. Can validation return actionable fix suggestions, not just error messages?

---

## 3. Scope

This proposal includes:

- 2 new MCP tools: `definitions.validate`, `definitions.dry_run`.
- 3 new GraphQL queries: `workflowInspection`, `agentCatalogInspection`, `definitionDiff`.
- Execution-order sorting via topological walk from `initial_state`.
- Agent grouping by functional role derived from `mode` field.
- Cost estimation based on backend profile parameters.
- Semantic diffing between two definition versions.

This proposal does **not** include:

- Changes to YAML schema or workflow/catalog file formats.
- Live reloading of definitions during a running run.
- Visual rendering of workflow DAGs (client-side concern).
- Provider availability health checks (covered by provider management).

---

## 4. Problem Statement

### 4.1 Validation requires starting a run

Today, the only way to discover YAML errors is to call `runs.start`. If the workflow YAML has a typo in a state transition target, the operator discovers this after creating a run record, which then fails during compilation. The run is left in a failed state.

### 4.2 No execution-order view

The Swift app's `WorkflowInspectorView` sorts states alphabetically (`workflow.states.keys.sorted()`). The actual execution order requires walking the transition graph from `initial_state`. Neither the Swift app nor the control-plane exposes this order.

### 4.3 Agent catalog is a flat list

The Swift app's `AgentCatalogView` shows all agents in a flat `ForEach`. With 15+ agents spanning orchestration, review, implementation, release, quality, and steward roles, finding a specific agent requires scanning. No grouping exists in either system.

### 4.4 No impact preview before changes

When an operator edits workflow YAML (adds a state, changes an agent binding), there is no way to preview what changed without manually comparing files. A semantic diff would show: "state X added, agent Y's model changed from A to B, transition Z removed".

---

## 5. Core Product Behavior

### 5.1 MCP Tool: `definitions.validate`

Validate a workflow + catalog pair without starting a run.

```json
{
  "name": "definitions.validate",
  "description": "Validate workflow YAML and agent catalog YAML without starting a run",
  "input_schema": {
    "type": "object",
    "required": ["workflow_yaml_path", "agent_catalog_yaml_path"],
    "properties": {
      "workflow_yaml_path": { "type": "string" },
      "agent_catalog_yaml_path": { "type": "string" }
    }
  }
}
```

**Response on success:**

```json
{
  "valid": true,
  "issues": [],
  "summary": {
    "state_count": 12,
    "agent_count": 15,
    "gate_count": 2,
    "loop_count": 1,
    "estimated_agent_invocations": 18,
    "execution_order": ["state_1_idea", "state_2_proposal", "state_3_review_po", ...],
    "agents_by_state": {
      "state_2_proposal": { "agent_id": "proposal_writer", "provider": "codex", "model": "codex-1" },
      ...
    }
  }
}
```

**Response on failure:**

```json
{
  "valid": false,
  "issues": [
    {
      "severity": "error",
      "message": "State 'state_99_missing' referenced in transition from 'state_5' does not exist",
      "location": "workflow.yaml:state_5.transitions[0].target",
      "suggestion": "Did you mean 'state_9_release'? Available states: [...]"
    },
    {
      "severity": "warning",
      "message": "Agent 'unused_agent' is defined in catalog but not referenced by any workflow state",
      "location": "agents.yaml:agents.unused_agent"
    }
  ]
}
```

**Improvement over Swift**: Validation returns `suggestion` fields with fix recommendations. The Swift app only shows error messages.

### 5.2 MCP Tool: `definitions.dry_run`

Compile the full run plan without executing. A "what-if" tool.

```json
{
  "name": "definitions.dry_run",
  "description": "Compile a full run plan without executing. Returns execution order, agent bindings, cost estimates, and warnings.",
  "input_schema": {
    "type": "object",
    "required": ["workflow_yaml_path", "agent_catalog_yaml_path"],
    "properties": {
      "workflow_yaml_path": { "type": "string" },
      "agent_catalog_yaml_path": { "type": "string" },
      "delivery_configuration_json": { "type": "string", "description": "Optional delivery config for repo-backed validation" },
      "strategy_profile_id": { "type": "string", "description": "Optional context strategy for budget estimation" }
    }
  }
}
```

**Response:**

```json
{
  "compiled": true,
  "plan": {
    "initial_state": "state_1_idea",
    "execution_order": [
      {
        "ordinal": 1,
        "state_id": "state_1_idea",
        "label": "Idea Brief",
        "type": "start",
        "owner_agent": {
          "agent_id": "lead",
          "provider": "claude",
          "model": "claude-sonnet-4-20250514",
          "effort": "high",
          "max_turns": 5,
          "mcp_extensions": ["developer", "xcode"],
          "worktree_write_enabled": false
        },
        "approval_required": false,
        "loop_config": null,
        "transitions": [
          { "target": "state_2_proposal", "condition": "exists('idea_brief')" }
        ]
      },
      ...
    ],
    "unreachable_states": ["state_error_handler"],
    "cost_estimate": {
      "min_cents": 120,
      "max_cents": 480,
      "breakdown_by_agent": [
        { "agent_id": "lead", "invocations": 3, "max_turns_total": 15, "estimated_cost_cents": 45 },
        { "agent_id": "proposal_writer", "invocations": 2, "max_turns_total": 40, "estimated_cost_cents": 160 }
      ],
      "note": "Estimates based on max_turns per invocation. Actual cost depends on context size and early stopping."
    },
    "warnings": [
      { "kind": "mcp_extension_unavailable", "agent_id": "writer", "extension": "autovisualiser", "message": "Extension 'autovisualiser' is opt_in_high_burn and not available by default" }
    ]
  }
}
```

**Not in Swift**: The Swift app has no dry-run capability. This is entirely new.

### 5.3 GraphQL: `workflowInspection`

```graphql
type WorkflowInspection {
  valid: Boolean!
  issues: [ValidationIssue!]!
  stateCount: Int!
  gateCount: Int!
  loopCount: Int!
  states: [WorkflowStateInspection!]!
  unreachableStates: [String!]!
}

type WorkflowStateInspection {
  ordinal: Int!
  stateId: String!
  label: String!
  stateType: String
  ownerAgentId: String!
  approvalRequired: Boolean!
  loopConfig: LoopConfigInspection
  transitions: [TransitionInspection!]!
}

type TransitionInspection {
  targetState: String!
  condition: String!
  loopIncrement: String
  loopBreak: Boolean
}

type LoopConfigInspection {
  maxIterations: Int!
  counterVariable: String!
  breakCondition: String
}

type ValidationIssue {
  severity: IssueSeverity!
  message: String!
  location: String
  suggestion: String
}

enum IssueSeverity {
  ERROR
  WARNING
}

extend type Query {
  workflowInspection(
    workflowYamlPath: String!
    agentCatalogYamlPath: String!
  ): WorkflowInspection!
}
```

States are returned **in execution order** (topological sort from `initial_state`), not alphabetical.

### 5.4 GraphQL: `agentCatalogInspection`

```graphql
type AgentCatalogInspection {
  valid: Boolean!
  issues: [ValidationIssue!]!
  agentCount: Int!
  backendProfileCount: Int!
  permissionProfileCount: Int!
  groups: [AgentGroup!]!
}

type AgentGroup {
  name: String!
  agents: [AgentInspection!]!
}

type AgentInspection {
  agentId: String!
  title: String!
  mode: String!
  provider: String!
  model: String!
  effort: String!
  maxTurns: Int!
  permissionProfile: String!
  skillRef: String!
  skillRole: String
  mcpExtensions: [String!]!
  worktreePolicy: String
  requiresHumanApproval: Boolean!
}

extend type Query {
  agentCatalogInspection(
    agentCatalogYamlPath: String!
  ): AgentCatalogInspection!
}
```

**Agent grouping** derives from the `mode` field:

| Mode pattern | Group name |
|---|---|
| `orchestration` | Orchestration |
| `proposal_authoring` | Proposal |
| `proposal_review.*` | Review |
| `aggregation.*`, `summary.*` | Aggregation |
| `implementation`, `audit` | Implementation |
| `security`, `prepush_review`, `docs` | Quality |
| `release_git`, `release_publish` | Release |
| `steward` | Steward |
| _(other)_ | Other |

### 5.5 GraphQL: `definitionDiff`

```graphql
type DefinitionDiff {
  statesAdded: [String!]!
  statesRemoved: [String!]!
  statesModified: [StateDiff!]!
  agentsAdded: [String!]!
  agentsRemoved: [String!]!
  agentsModified: [AgentDiff!]!
  transitionsChanged: [TransitionDiffEntry!]!
  breakingChanges: [BreakingChange!]!
}

type StateDiff {
  stateId: String!
  changes: [FieldChange!]!
}

type AgentDiff {
  agentId: String!
  changes: [FieldChange!]!
}

type FieldChange {
  field: String!
  oldValue: String
  newValue: String
}

type TransitionDiffEntry {
  fromState: String!
  description: String!
}

type BreakingChange {
  kind: String!
  message: String!
  affectedStates: [String!]!
}

extend type Query {
  definitionDiff(
    oldWorkflowPath: String!
    newWorkflowPath: String!
    oldCatalogPath: String!
    newCatalogPath: String!
  ): DefinitionDiff!
}
```

**Not in Swift**: Semantic definition diffing is entirely new. The Swift app has no comparable feature.

### 5.6 Execution-order algorithm

```
fn execution_order(plan: &RunPlan) -> Vec<(usize, String)> {
    let mut order = Vec::new();
    let mut visited = HashSet::new();
    let mut queue = VecDeque::from([plan.initial_state.clone()]);
    let mut ordinal = 1;

    while let Some(state_id) = queue.pop_front() {
        if !visited.insert(state_id.clone()) { continue; }
        order.push((ordinal, state_id.clone()));
        ordinal += 1;

        if let Some(state) = plan.states.get(&state_id) {
            for transition in &state.transitions {
                if !visited.contains(&transition.target_state) {
                    queue.push_back(transition.target_state.clone());
                }
            }
        }
    }
    order
}
```

States not reachable from the primary path appear in the `unreachable_states` list.

### 5.7 Cost estimation logic

Per agent invocation:
- `estimated_cost = max_turns * avg_tokens_per_turn * price_per_token`
- `avg_tokens_per_turn` defaults: high effort = 4000, medium = 2000, low = 1000
- Price per token from a static lookup table keyed by (provider, model)
- Min estimate assumes 30% of max_turns used; max assumes 100%

---

## 6. Migration

### 6.1 New module

Create `workflow/src/inspector.rs`:
- `validate(workflow_path, catalog_path) -> ValidationResult`
- `dry_run(workflow_path, catalog_path, delivery_config?, strategy?) -> DryRunResult`
- `diff(old_workflow, new_workflow, old_catalog, new_catalog) -> DefinitionDiff`

Reuses existing `compiler::compile()` internally.

### 6.2 MCP tools

Add `definitions.rs` to `mcp-server/src/tools/`:
- Register `definitions.validate` and `definitions.dry_run`

### 6.3 GraphQL schema

Add to `graphql-server/src/schema.rs`:
- Types: `GqlWorkflowInspection`, `GqlAgentCatalogInspection`, `GqlDefinitionDiff` and subtypes
- Resolvers for 3 new queries

### 6.4 No database changes

All tools are stateless computations over YAML files. No new tables or columns.

---

## 7. Verification

- `definitions.validate` with valid YAML returns `valid: true` with summary counts.
- `definitions.validate` with invalid YAML returns `valid: false` with specific issues and suggestions.
- `definitions.dry_run` returns execution order matching the actual order that `runs.start` would use.
- `definitions.dry_run` cost estimates are within 2x of actual costs on completed runs.
- `workflowInspection` returns states in BFS execution order, not alphabetical.
- `agentCatalogInspection` groups agents correctly (all `proposal_review.*` modes in Review group).
- `definitionDiff` detects: state added, state removed, agent model changed, transition target changed.
- `definitionDiff` flags breaking changes (removed agent still referenced by a state).
- All tools work with the canonical `examples/workflows/workflow.yaml` and `examples/agents/agents.yaml`.

---

## 8. Risks

| Risk | Severity | Mitigation |
|------|----------|------------|
| Compilation is slow for complex workflows | Low | Compilation is already fast (<100ms). Dry-run adds cost estimation which is O(agents). |
| Cost estimation is inaccurate | Medium | Clearly labeled as estimates. Note in response: "Actual cost depends on context size and early stopping." |
| Execution-order walk diverges from actual runtime order | Low | Both use the same `initial_state` + transitions. Runtime may skip states via conditions, but the walk shows the full reachable graph. |
| `definitionDiff` with large catalogs produces noisy output | Low | Breaking changes are separated from non-breaking. Client can filter by change type. |
| File path access for YAML requires permissions | Medium | Same file access model as `runs.start`. The daemon already reads these paths during compilation. |
| Suggestion generation for validation errors is brittle | Low | Suggestions are best-effort. The `suggestion` field is optional — validation works without it. |
