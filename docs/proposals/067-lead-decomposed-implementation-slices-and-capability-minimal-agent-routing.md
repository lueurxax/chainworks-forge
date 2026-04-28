# Proposal 067: Lead-Decomposed Implementation Slices and Capability-Minimal Agent Routing

| Field | Value |
|---|---|
| Date | 2026-04-22 |
| Status | Draft |
| Author | Andrey Khasanov |
| Depends on | [workflow-execution-engine.md](../reference/workflow-execution-engine.md), [060-lead-driven-reviewer-routing-and-expanded-reviewer-catalog.md](060-lead-driven-reviewer-routing-and-expanded-reviewer-catalog.md), [xcode-mcp-bridge-pool.md](../reference/xcode-mcp-bridge-pool.md), [rust-control-plane.md#capacity-aware-scheduling-and-backpressure](../reference/rust-control-plane.md#capacity-aware-scheduling-and-backpressure) |
| Scope | Split implementation work into lead-owned slices and route each slice to the minimal agent profile and MCP capability set needed for that slice. |
| Goal | Avoid one-size-fits-all implementation agents by letting the lead describe multiple concrete work slices while the orchestrator selects safe, capability-minimal agent profiles per slice. |

**Gate naming note:** this proposal owns the future canonical gate alias `proposal-067|p067`. It must be added to `scripts/test-gate.sh` and `docs/reference/test-gates.md` when implementation starts.

---

## 1. Context and Motivation

Parallel implementation runs now regularly touch mixed surfaces:

- Rust control-plane engine, DB, GraphQL, MCP, and workflow code;
- macOS SwiftUI operator UI;
- proposal and reference documentation;
- security, rollout, and test-gate evidence;
- agent catalog and runtime configuration.

Today an implementation stage can select a broad `code_writer` style agent profile that carries more MCP capability than the current task needs. For example, a Rust/control-plane slice may inherit `xcode` MCP because the selected agent supports macOS work in general. That is not wrong when the catalog says the agent has that capability, but it is inefficient and fragile when the task does not need it.

The real missing abstraction is not "disable Xcode everywhere". The missing abstraction is:

1. the lead decomposes implementation into concrete work slices;
2. each slice declares intent, scope, dependencies, and expected outputs;
3. the orchestrator maps each slice to the least-capable suitable agent profile;
4. the run aggregates slice results into normal review and approval.

---

## 2. Problem Statement

### 2.1 A proposal may require several different implementation profiles

One proposal can require:

- Rust engine/database work;
- Swift UI readback;
- GraphQL schema updates;
- documentation and acceptance gate updates;
- security or rollout evidence.

Selecting one broad agent profile for the whole implementation forces every task to inherit the union of tools, MCP servers, plugins, skills, and runtime assumptions.

### 2.2 Extra MCP capability creates avoidable operational risk

Unneeded MCP startup increases:

- provider startup latency;
- auth/keychain failure probability;
- laptop sleep/wake recovery noise;
- stale MCP process cleanup burden;
- unrelated prompt/approval interruptions;
- debugging complexity in ACP runtime facts.

The issue is not that an agent with `xcode` starts `xcode` when asked to use that profile. The issue is that the workflow lacks a first-class way to choose a smaller profile for non-Xcode work.

### 2.3 Agents.yaml should remain a capability catalog

`agents.yaml` should describe available agents, skills, profiles, and capabilities. It should not become an operator-authored per-run imperative script. The lead and orchestrator should choose from the catalog based on work intent.

---

## 3. Scope

P067 includes:

- a lead-authored `implementation_slices` output contract;
- per-slice intent, scope, dependency, write-scope, and acceptance metadata;
- orchestrator selection of agent profile per slice;
- minimal MCP capability routing per selected profile;
- conflict detection for overlapping write scopes;
- parallel execution for disjoint slices;
- sequential or lead-mediated execution for dependent/overlapping slices;
- durable run artifacts explaining agent/profile selection decisions;
- aggregation of slice results into implementation review, refinement, and approval.

P067 does not include:

- manual operator selection of every low-level MCP server;
- UI use of MCP tools;
- new GraphQL write paths beyond existing approval/retry/cancel semantics;
- removing Xcode MCP from agents that legitimately need it;
- hardcoding language-specific behavior into the scheduler;
- replacing P060 reviewer routing or current lead validation.

---

## 4. Proposed Model

### 4.1 Lead output: implementation slices

The lead produces a structured plan before implementation work starts or before a retry that changes work shape.

Example:

```yaml
implementation_slices:
  - id: rust_control_plane
    intent: rust_control_plane
    summary: Implement scheduler/read-model changes in the Rust daemon.
    scope:
      include:
        - control-plane/crates/engine/**
        - control-plane/crates/db/**
        - control-plane/crates/graphql-server/**
      exclude:
        - Chainworks Forge/**
    write_scope_class: rust_control_plane
    dependencies: []
    agent_role: code_writer
    required_capabilities:
      - filesystem
      - shell
      - rust_toolchain
      - sqlite
    excluded_capabilities:
      - xcode
    acceptance:
      - Rust code compiles under the proposal gate.
      - New projections are exposed through GraphQL.

  - id: macos_operator_ui
    intent: macos_swift_ui
    summary: Add operator readback for the new projection.
    scope:
      include:
        - Chainworks Forge/**
        - Chainworks ForgeTests/**
    write_scope_class: macos_ui
    dependencies:
      - rust_control_plane
    agent_role: code_writer
    required_capabilities:
      - filesystem
      - shell
      - xcode
    excluded_capabilities: []
    acceptance:
      - UI reads through GraphQL.
      - No MCP usage is added to the UI.

  - id: docs_and_gate
    intent: docs_and_gate
    summary: Update reference docs and proposal gate wiring.
    scope:
      include:
        - docs/reference/**
        - scripts/test-gate.sh
    write_scope_class: docs_gate
    dependencies:
      - rust_control_plane
    agent_role: docs_guardian
    required_capabilities:
      - filesystem
    excluded_capabilities:
      - xcode
    acceptance:
      - Gate alias is documented.
      - Reference docs match implemented behavior.
```

The exact wire format can be JSON or YAML, but the persisted run artifact must be machine-validated.

### 4.2 Orchestrator responsibility

The orchestrator maps each slice to an agent profile by applying:

1. role compatibility: `agent_role`;
2. intent compatibility: `rust_control_plane`, `macos_swift_ui`, `docs_and_gate`, `security_review`, etc.;
3. required capability coverage;
4. excluded capability filtering;
5. write-scope conflict checks;
6. run-level concurrency and provider backpressure limits.

The orchestrator must record:

- selected agent id;
- selected profile id;
- selected provider;
- requested MCP extensions;
- actual MCP extensions;
- selection reason;
- rejected candidate profiles and concise rejection reasons.

### 4.3 Agents.yaml remains agnostic

The agent catalog should describe what agents can do, not why the lead wants them for one run.

Example catalog shape:

```yaml
agents:
  - id: code_writer
    profiles:
      - id: control_plane
        intents: [rust_control_plane, graphql_api, mcp_server]
        capabilities: [filesystem, shell, rust_toolchain, sqlite]
        mcp_extensions: [chainworks-control-plane]

      - id: macos_ui
        intents: [macos_swift_ui]
        capabilities: [filesystem, shell, xcode]
        mcp_extensions: [chainworks-control-plane, xcode]

      - id: docs
        intents: [docs_and_gate, proposal_refinement]
        capabilities: [filesystem]
        mcp_extensions: []
```

The catalog is not reviewer-specific. It is a general list of agents/profiles from which the lead/orchestrator can select.

### 4.4 Parallelism and dependencies

Slices with disjoint write scopes and no dependencies may run concurrently.

Slices with overlapping write scopes must either:

- run sequentially;
- be merged by a lead/refine pass;
- be rejected as an invalid decomposition and sent back to lead planning.

The orchestrator must not launch two write-capable slices against the same file ownership area unless the slice plan explicitly declares a merge strategy.

### 4.5 Retry behavior

Retry operates at slice granularity:

- retrying one failed slice should not re-run unrelated completed slices;
- if a slice repeatedly fails because the selected profile lacks capability, the orchestrator asks the lead for a revised slice/profile plan;
- if a task is misclassified, the lead can produce a new slice intent without discarding durable work from completed slices.

---

## 5. Acceptance Criteria

1. Implementation stages can persist a machine-validated `implementation_slices` artifact.
2. A mixed Rust + macOS UI + docs proposal routes each slice to a different profile when appropriate.
3. A Rust/control-plane slice does not start `xcode` MCP unless the slice explicitly requires Xcode.
4. A macOS SwiftUI slice can start `xcode` MCP when its selected profile requires it.
5. A docs/proposal/gate slice does not inherit tool-heavy MCP extensions by default.
6. The run artifact index records `selected_agent_profile`, `selection_reason`, `requested_mcp_extensions`, and `actual_mcp_extensions` per slice.
7. Disjoint write scopes can run concurrently; overlapping write scopes are serialized or routed back through lead mediation.
8. Slice-level retry can restart one failed slice without discarding completed slice artifacts.
9. Existing P060 reviewer routing and current lead validation remain compatible.
10. UI read paths remain GraphQL-only, and this proposal adds no UI MCP usage.

---

## 6. Implementation Outline

1. Define the `implementation_slices` contract in the domain/workflow layer.
2. Extend lead prompts and output validation to produce slice plans for implementation stages.
3. Extend `agents.yaml` profile metadata with supported intents, capabilities, and MCP extension sets.
4. Add an agent-profile selector in the orchestrator.
5. Add write-scope conflict detection and dependency ordering.
6. Change implementation scheduling to create slice-scoped work items.
7. Persist slice selection and MCP runtime facts into run artifacts.
8. Add retry support for one failed slice.
9. Aggregate completed slice artifacts into existing implementation review and approval stages.
10. Add the `proposal-067|p067` gate.

---

## 7. Test Plan

Add `./scripts/test-gate.sh proposal-067`.

The gate should cover:

- schema validation for `implementation_slices`;
- selection of a control-plane profile without `xcode`;
- selection of a macOS UI profile with `xcode`;
- docs-only slice selection with no tool-heavy MCP extensions;
- parallel scheduling for disjoint scopes;
- serialization or lead mediation for overlapping scopes;
- retry of one failed slice while preserving completed slice artifacts;
- run artifact readback for selection reason and actual MCP extensions.

The gate does not require real Xcode builds, simulator runs, or external MCP approvals. Profile selection can be tested with fake agents and fake MCP extension ids.

---

## 8. Rollout

1. Introduce slice planning behind a feature flag or workflow-family capability.
2. Start with proposal implementation workflows only.
3. Keep the existing single-agent implementation path as fallback while slice planning is dogfooded.
4. Enable slice planning for mixed Rust/Swift/docs proposals first.
5. After stable dogfood evidence, make slice planning the default for implementation stages.

---

## 9. Risks and Tradeoffs

**Risk: Lead output becomes too verbose.**
Mitigation: allow a compact one-slice plan for simple proposals and only require detailed decomposition when multiple scopes are detected.

**Risk: Misclassified slices choose too-small profiles.**
Mitigation: capability failures should return to lead planning with concrete missing-capability evidence.

**Risk: More orchestration state increases DB pressure.**
Mitigation: persist compact slice metadata and reuse the Rust control-plane scheduler/backpressure controls for concurrency.

**Risk: Agents.yaml grows into a policy engine.**
Mitigation: keep policy in orchestrator selection rules. The catalog only advertises available profiles and capabilities.

---

## 10. Open Questions

1. Should the lead always produce slices, or only when the proposal touches multiple ownership classes?
2. Should profile selection be deterministic by default, or may it use score/history to choose among equivalent profiles?
3. Should slice intent names be fixed enums or catalog-defined strings with validation?
4. How much candidate rejection detail should be exposed to the operator UI versus kept in debug artifacts?
5. Should completed slices be allowed to feed knowledge capsules into later slices in the same run, or should that remain a P064 responsibility?

---

## 11. Non-Goals Reaffirmed

P067 does not remove capabilities from capable agents. It ensures the orchestrator chooses the smallest suitable profile for the work slice at hand.

P067 does not make operators micromanage MCP servers. Operators approve meaningful workflow gates; lead and orchestrator handle decomposition and profile selection.
