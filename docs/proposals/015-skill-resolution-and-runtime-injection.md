# Proposal 015: Skill Resolution and Runtime Injection

| Field | Value |
|---|---|
| Date | 2026-03-29 |
| Status | Draft |
| Author | Engineer (single-engineer project) |
| Depends on | [reference/workflow-execution-engine.md](../reference/workflow-execution-engine.md), [reference/runtime-contract.md](../reference/runtime-contract.md), [reference/goose-server-transport.md](../reference/goose-server-transport.md), [reference/current-system-baseline.md](../reference/current-system-baseline.md), [reference/output-contracts-failure-evidence-and-recovery.md](../reference/output-contracts-failure-evidence-and-recovery.md) |
| Scope | Skill resolution, skill type dispatch, runtime injection of skill content into agent execution context, skill-role customization, skill provenance, skill preflight validation, and operator skill visibility |
| Goal | Make every `skills.*` definition, every `agents.*.skill_ref`, and every `agents.*.skill_role` in the agent catalog a runtime-authoritative declaration that changes agent execution behavior — not metadata-only annotation. |

---

## 1. Context

### 1.1 The declarative-runtime gap

`examples/agents/agents.yaml` declares a skill system with three distinct types:

| Type | Count | Example | What it declares |
|---|---|---|---|
| `external_skill` | 2 | `proposal_review_triad` | Filesystem path to a skill package |
| `builtin_agent` | 1 | `docs_quality_guardian` | Name of a known built-in agent |
| `inline_skill` | 8 | `orchestrator_core` | Textual description of agent capability |

Every agent in the catalog references a skill through `skill_ref`, and some reference a role variant through `skill_role`.

The current runtime treats all of these identically: **the skill name is copied as a string into `ResolvedAgent`, hashed for Steward provenance, and then ignored**. No type dispatch, no content loading, no prompt injection, no role customization.

The implemented output-contract and failure-evidence reference explicitly documented this gap in its tiered coverage and deferred it:

> *SkillResolutionBridge — Resolving `skills.*`, `skill_ref`, and `skill_role` into live execution requires skill-system design beyond this bounded slice.*

Proposal 015 is that design and that implementation.

### 1.2 Why skills must be in MVP

Without functional skills, the Chainworks agent catalog is a lie: it declares rich role-skill bindings that never reach the agent. The agent receives only the hardcoded `prompt` field. This means:

1. **External skill packages are dead code.** Two filesystem paths in the catalog point to skill definitions that are never loaded. The product claims to support external skills but does not.
2. **Skill roles are cosmetic.** Four proposal reviewers share `proposal_review_triad` with different `skill_role` values (`product_owner`, `ux_designer`, `ui_designer`, `architect`). The role is never used to customize the agent. The reviewers are differentiated only by their hardcoded `prompt` field — which means the `skill_ref` / `skill_role` system serves no runtime purpose.
3. **Inline skill descriptions are invisible at execution time.** Eight inline skills describe agent capabilities (`Lead orchestration, fan-out/fan-in, gatekeeping...`). None of this reaches the agent.
4. **Provenance is hollow.** The `skillSnapshotHash` in `AgentExecution` hashes only the skill *name*, not its *content*. If an external skill changes on disk, provenance cannot detect it.
5. **Preflight cannot validate skills.** No check verifies that external skill paths exist or that builtin agent names are resolvable.

A product that declares a skill system in its configuration surface but ignores it at runtime is not an MVP — it is a prototype with aspirational YAML.

### 1.3 What this proposal is

Proposal 015 is the bounded slice that makes skills functional:

- resolve skill type and content at compilation time,
- inject resolved skill content into agent execution context,
- customize injection based on skill role,
- validate skill integrity at preflight,
- persist resolved skill content in provenance snapshots,
- show skill truth in operator surfaces.

### 1.4 What this proposal is not

Proposal 015 is **not**:

- a skill authoring tool or skill editor UI,
- a skill marketplace or package manager,
- a skill versioning or dependency system,
- a skill hot-reload or runtime update mechanism,
- a change to the workflow state machine,
- a new provider integration,
- or a redesign of the agent execution model.

It is specifically about making the already-declared skill system operational.

---

## 2. Product questions this proposal must answer

After Proposal 015, the engineer must be able to answer all of these with code truth:

1. When an agent executes, does its resolved skill content reach the Goose session as part of the execution context?
2. When two agents share the same `skill_ref` but have different `skill_role` values, does the execution context differ according to role?
3. When an external skill path does not exist on disk, does preflight catch and report it before the run starts?
4. When an external skill changes on disk between two runs, does provenance detect the content drift?
5. Can the operator see, for any agent execution, the resolved skill type, content summary, and role that were active at execution time?
6. Does every skill type (`external_skill`, `inline_skill`, `builtin_agent`) have at least one tested code path from YAML through compilation, resolution, injection, execution, and provenance?

Proposal 015 is done only when all six answers are yes with test evidence.

---

## 3. What we build

Proposal 015 delivers four tightly coupled layers.

### Layer A: Skill Resolution

| Component | Responsibility |
|---|---|
| **ResolvedSkill** | Immutable value type carrying: `id`, `type` (enum: `external`, `inline`, `builtin`), `resolvedContent` (String), `contentHash` (SHA-256), `injectedContentHash` (SHA-256 of exactly what reaches execution), `sourcePath` (optional, for external), `sourceDescription` (optional, for inline), and `bundleManifest` (optional summary of companion files discovered but not injected) |
| **SkillResolver** | Pure function: `(AgentCatalog.SkillRef, SkillResolverContext) -> ResolvedSkill`. Dispatches on type, loads content, computes hash. |
| **ExternalSkillLoader** | Loads external skill content from filesystem path. Reads a Codex `SKILL.md` bundle and returns the normalized instruction string plus bundle metadata. |
| **BuiltinSkillRegistry** | Maps builtin agent names to known instruction sets. MVP set: `docs-quality-guardian`. |
| **SkillResolverContext** | Carries filesystem access, path resolution rules, and environment variable substitution context. |

### Layer B: Compilation and Snapshot Integration

| Component | Responsibility |
|---|---|
| **RunPlanCompiler extension** | Resolves every agent's `skill_ref` into a `ResolvedSkill` during compilation. Stores `ResolvedSkill` inside `ResolvedAgent`. |
| **ResolvedAgent extension** | Gains `resolvedSkill: ResolvedSkill?` and `skillRole: String?` fields. |
| **RunStartSnapshot extension** | Captures all resolved skill content hashes and injected-content hashes in the frozen start snapshot. The immutable owner is current `Run` frozen fields plus `RunStartSnapshot`, not a parallel `RunPlanSnapshot` substrate. |
| **DefinitionHasher extension** | Hashes resolved skill content (not just the name) for provenance. |

### Layer C: Runtime Injection

| Component | Responsibility |
|---|---|
| **SkillInjector** | Builds the skill-specific portion of the agent execution context. Combines resolved skill content with role customization. |
| **GooseSessionBridge extension** | Calls `SkillInjector` during `buildExecutionPacket()` to feed the canonical `ExecutionPacket`; it does not become a second packet owner. |
| **SkillRoleCustomizer** | For skills that support specialization (for example `proposal_review_triad`), applies skill-specific mode mapping or role-specific instructions that modify the base skill content. |
| **SkillInjectionPolicy** | Defines injection strategy per skill type: `prepend_to_system_prompt`, `append_to_system_prompt`, or `structured_context_block`. MVP uses `prepend_to_system_prompt` for all types. |

### Layer D: Validation, Provenance, and Visibility

| Component | Responsibility |
|---|---|
| **SkillPreflightCheck** | Added to `PreflightService`. Validates: external paths exist, builtin names are registered, inline descriptions are non-empty. |
| **AgentExecution extension** | `skillSnapshotHash` now stores hash of resolved content (not name). Gains `skillType: String?` and `skillContentSummary: String?` fields. |
| **AgentCatalogView extension** | Shows resolved skill type, content preview (first 200 chars), role, and content hash. |
| **Shell-owned report / comparison / artifact surfaces extension** | Extends existing `RunReportView`, `RunComparisonView`, and `ArtifactInspectorView` to show resolved skill truth for a specific execution. Proposal 015 does not create a parallel operator-inspection lane. |

---

## 4. Skill resolution rules

### 4.1 External skill resolution

An `external_skill` declares a filesystem path to a Codex skill bundle.

Resolution rules:

1. The path must exist on the local filesystem at compilation time.
2. The skill package is a directory rooted by a required `SKILL.md`.
3. `SKILL.md` is the executable entrypoint. Companion files under `assets/`, `references/`, `evals/`, or `agents/` are not implicitly concatenated into execution content.
4. The loader records companion bundle metadata for provenance and operator visibility, but the MVP injected content comes from `SKILL.md` plus explicit role/mode customization only.
5. Environment variables in the path are substituted using the same rules as `paths.*` in the catalog.
6. If the path does not exist or `SKILL.md` is missing, resolution fails with a typed error. Preflight reports this as a blocking failure.

This rule intentionally matches the repo's actual external skills under `/Users/user/.codex/skills/*`, which are Codex bundles rooted at `SKILL.md` rather than generic markdown directories.

### 4.1.1 External bundle companions

The resolver may inspect companion markdown files for metadata and specialized routing, but they do not become executable prompt content unless Proposal 015 explicitly names them as such.

MVP companion handling:

- `SKILL.md`: executable root content
- `references/*.md`: provenance-visible companion context only
- `assets/*.md`: authoring/review templates only, not execution content
- `agents/*.yaml`: bundle-local helper config only, not execution content
- `evals/*`: proof assets only, not execution content

This keeps raw skill truth and executable injected truth aligned.

### 4.2 Inline skill resolution

An `inline_skill` declares a description string directly in the YAML.

Resolution rules:

1. The `description` field must be non-empty.
2. The resolved content is the description string itself.
3. No filesystem access is needed.
4. The content hash is computed from the description text.

### 4.3 Builtin agent resolution

A `builtin_agent` declares the name of a known agent type.

Resolution rules:

1. The `name` field must match an entry in `BuiltinSkillRegistry`.
2. The registry returns a canonical instruction set for the named agent.
3. MVP set contains exactly one entry: `docs-quality-guardian`.
4. Unknown names fail resolution with a typed error.

### 4.4 Resolution failure policy

If any agent's skill resolution fails during compilation:

1. The `RunPlanCompiler` reports the failure with agent ID, skill ref, and error type.
2. The run does not start.
3. The failure is visible in the preflight readiness surface.
4. The operator must fix the catalog or filesystem before retrying.

---

## 5. Skill injection into agent execution context

### 5.1 Injection model

The resolved skill content is injected into the agent's execution context via the system prompt. The injection does not replace the agent's existing `prompt` field — it augments it.

Prompt assembly order:

1. **Skill preamble** (from `SkillInjector`)
2. **Role specialization** (from `SkillRoleCustomizer`, when `skill_role` is present)
3. **Agent prompt** (from `agents.*.prompt` in catalog)

This means the agent always receives its skill context before its specific task prompt.

### 5.2 Skill preamble format

```
## Skill: {skill_id}
Type: {external|inline|builtin}

{resolved_content}
```

### 5.3 Role specialization format

When `skill_role` is present:

```
## Active Role: {skill_role}

You are operating in the "{skill_role}" role for this skill.
Apply all skill instructions through the lens of this role.
```

For `external_skill` types, generic `roles/{skill_role}.md` discovery is not sufficient for MVP because current repo reality already has a shared mode-based specialist contract.

Proposal 015 therefore requires two specialization paths:

1. **Skill-specific specialization registry**
   - A skill can declare that `skill_role` maps to a concrete runtime mode instead of a role file.
   - Canonical MVP case: `proposal_review_triad`
     - `product_owner` -> `product-only`
     - `ux_designer` -> `ux-only`
     - `ui_designer` -> `ui-only`
     - `architect` -> `architecture-only`
   - The injected execution content must reflect the selected mode contract, not just a generic "act through this lens" paragraph.

2. **Optional bundle-local role file**
   - Only when a skill explicitly opts into file-based specialization may `roles/{skill_role}.md` augment the base skill content.

If neither specialization path is declared, the runtime may fall back to the generic role block.

### 5.4 Injection for each skill type

| Skill Type | Preamble Source | Role Support | Notes |
|---|---|---|---|
| `external_skill` | Loaded `SKILL.md` content | Yes, via skill-specific mode mapping or explicitly declared role file; otherwise generic | Full filesystem-backed instructions rooted in Codex bundle contract |
| `inline_skill` | Description string from YAML | Generic role block only | Lightweight, catalog-embedded |
| `builtin_agent` | Registry instruction set | Generic role block only | Known agent types with canonical instructions |

### 5.5 No-skill fallback

If an agent has no `skill_ref` (currently all agents have one, but the model must be safe):

1. No skill preamble is injected.
2. The agent receives only its `prompt` field.
3. This is not an error — it is a valid configuration for simple agents.

---

## 6. Provenance and drift detection

### 6.1 Snapshot integration

Current immutable run-start authority is:

- `Run.workflowSnapshotJSON`
- `Run.catalogSnapshotJSON`
- `Run` frozen provenance fields
- `RunStartSnapshot`

Proposal 015 adds:

- `resolvedSkillsJSON`: serialized map of `skillRef -> ResolvedSkill` for all skills used in the run
- `skillContentHashes`: map of `skillRef -> SHA-256` for all resolved raw skill content
- `skillInjectedContentHashes`: map of `skillRef -> SHA-256` for the exact injected execution content after role/mode customization

`RunStartSnapshot` is the proposal-owned extension point for this data. The proposal must not introduce a second frozen snapshot authority parallel to current immutable `Run` fields.

### 6.2 Drift detection

When a new run is created using the same catalog:

1. The compiler resolves all skills fresh.
2. Content hashes are compared against the previous run's snapshot.
3. If any external skill content has changed, the snapshot records the drift.
4. Steward can use this to detect configuration changes between runs.

### 6.3 Per-execution provenance

`AgentExecution.skillSnapshotHash` changes from hashing the skill name to hashing the exact injected execution content. Proposal 015 also preserves the raw resolved-content hash separately in frozen snapshot truth.

This means:

- Same skill name + different injected content = different execution hash.
- Same raw bundle + different role/mode specialization = different execution hash.
- Provenance can detect when an external skill was modified between executions.
- Reports can show whether two executions used the same raw skill content and whether they injected the same executable specialization.

### 6.4 Size-limit and truncation policy

Proposal 015 must not hash one thing and execute another.

Therefore MVP uses a fail-closed rule:

1. If size limits would require truncating executable skill content before injection, the run does not start.
2. Preflight reports an oversized-skill failure with the affected `skill_ref`.
3. The raw resolved-content hash and injected-content hash remain equal for every successful execution in MVP.

Future lazy or partial injection is allowed only if the runtime persists both:

- raw resolved skill hash
- exact injected content hash

and operator surfaces make the distinction visible.

---

## 7. Preflight validation

`PreflightService` gains a new check: `SkillPreflightCheck`.

| Check | Scope | Blocking? |
|---|---|---|
| External skill path exists | All agents with `external_skill` type | Yes |
| External skill instruction file found | All agents with `external_skill` type | Yes |
| External skill content is non-empty | All agents with `external_skill` type | Yes |
| Builtin agent name is registered | All agents with `builtin_agent` type | Yes |
| Inline skill description is non-empty | All agents with `inline_skill` type | Yes |
| Declared specialization path exists (mode mapping or explicit role file, when required by the skill) | Agents with both `skill_ref` -> `external_skill` and `skill_role` | No for optional specialization; Yes when the skill contract requires specialist mapping |

All blocking failures prevent run start and are reported in `PilotReadinessView`.

---

## 8. Operator visibility

### 8.1 AgentCatalogView changes

Current state: shows `skillRef` (string) and `skillRole` (string).

After Proposal 015:

| Field | Source | Always shown |
|---|---|---|
| Skill Ref | `agent.skillRef` | Yes |
| Skill Type | `resolvedSkill.type` (badge: External / Inline / Builtin) | Yes |
| Skill Role | `agent.skillRole` | When present |
| Content Preview | First 200 characters of `resolvedSkill.resolvedContent` | Yes |
| Content Hash | `resolvedSkill.contentHash` (truncated) | Disclosure toggle |
| Source Path | `resolvedSkill.sourcePath` | External only |

### 8.2 Shell-owned execution visibility changes

For a completed or running agent execution, existing shell-owned surfaces gain:

| Field | Source |
|---|---|
| Resolved Skill Content | Full text of the skill content that was injected |
| Injection Strategy | `prepend_to_system_prompt` |
| Role Specialization | Full role block if applied |
| Content Hash at Execution | `agentExecution.skillSnapshotHash` |

Canonical owners:

- run-centric inspection in existing report surfaces
- comparison in `RunComparisonView`
- artifact-level drilldown in `ArtifactInspectorView`

Proposal 015 does not create a standalone `AgentInspectorView` product lane with separate truth semantics.

### 8.3 PilotReadinessView changes

Skill preflight results are shown as a dedicated section:

- Green: all skills resolved successfully
- Yellow: warnings (e.g., missing role-specific file, falling back to generic)
- Red: blocking failures (missing path, unknown builtin, empty content)

---

## 9. Implementation plan

### Phase 1: Data model and resolution (core)

1. Define `ResolvedSkill` value type.
2. Define `SkillType` enum (`external`, `inline`, `builtin`).
3. Implement `ExternalSkillLoader` with `SKILL.md` bundle discovery.
4. Implement `BuiltinSkillRegistry` with `docs-quality-guardian` entry.
5. Implement `SkillResolver` with type dispatch.
6. Add `resolvedSkill` to `ResolvedAgent`.
7. Extend `RunPlanCompiler` to resolve skills during compilation.

**Verification:** Unit tests proving each skill type resolves correctly. Unit test for resolution failure when path missing or name unknown.

### Phase 2: Runtime injection

8. Implement `SkillInjector` with preamble generation.
9. Implement `SkillRoleCustomizer` for generic specialization plus skill-specific mode mapping.
10. Extend `GooseSessionBridge.buildExecutionPacket()` to call `SkillInjector`.
11. Extend `SimulatedAgentExecutor` to verify skill content reaches the execution context.

**Verification:** Integration test proving skill content appears in the system prompt delivered to the executor. Test that two agents with the same `skill_ref` but different `skill_role` receive different prompts.

### Phase 3: Provenance and validation

12. Extend `RunStartSnapshot` plus immutable `Run` frozen fields with `resolvedSkillsJSON`, `skillContentHashes`, and `skillInjectedContentHashes`.
13. Change `AgentExecution.skillSnapshotHash` to hash injected execution content.
14. Implement `SkillPreflightCheck` in `PreflightService`.
15. Extend `DefinitionHasher` for skill content hashing.

**Verification:** Test that skill content drift between runs is detectable. Test that missing external path blocks preflight. Test that unknown builtin name blocks preflight.

### Phase 4: Operator visibility

16. Extend `AgentCatalogView` with skill type badge, content preview, and metadata.
17. Extend existing shell-owned report / comparison / artifact surfaces with full resolved skill content display.
18. Extend `PilotReadinessView` with skill preflight section.

**Verification:** UI smoke test showing skill information in all three surfaces.

---

## 10. Acceptance criteria

| # | Criterion | Verification |
|---|---|---|
| A1 | Every `external_skill` in the catalog is loaded from its filesystem path and its content appears in the agent's execution prompt | Integration test with fixture skill package |
| A2 | Every `inline_skill` description is injected into the agent's execution prompt | Integration test comparing prompt with and without skill |
| A3 | Every `builtin_agent` skill resolves to a known instruction set | Unit test with `docs-quality-guardian` |
| A4 | Agents sharing the same `skill_ref` with different `skill_role` receive different execution prompts | Integration test with `proposal_review_triad` agents proving current mode mapping (`product-only`, `ux-only`, `ui-only`, `architecture-only`) |
| A5 | Missing external skill path blocks run start via preflight | Preflight test |
| A6 | Unknown builtin name blocks run start via preflight | Preflight test |
| A7 | `RunStartSnapshot` plus immutable `Run` frozen fields capture resolved raw and injected skill content hashes | Snapshot round-trip test |
| A8 | `AgentExecution.skillSnapshotHash` reflects injected execution content, not just skill name | Provenance test comparing hash before and after skill content or specialization change |
| A9 | Operator can see skill type, content preview, and role in `AgentCatalogView` | UI smoke test |
| A10 | Operator can see full resolved skill content for a completed execution in current shell-owned report / comparison / artifact surfaces | UI smoke test |
| A11 | Skill preflight results are visible in `PilotReadinessView` | UI smoke test |
| A12 | All three skill types have end-to-end test coverage: YAML -> resolution -> injection -> execution -> provenance | Three integration tests (one per type) |

---

## 11. Out of scope

| Surface | Reason |
|---|---|
| Skill authoring UI | MVP resolves and injects existing skills; authoring is editor / CLI work |
| Skill versioning or dependency management | Skills are identified by content hash, not by version number |
| Skill hot-reload during a running execution | Immutable `Run` frozen fields plus `RunStartSnapshot` freeze skills at run creation time |
| Skill marketplace or sharing mechanism | Local-first single-engineer tool; sharing is filesystem copy |
| Permission profile enforcement at transport level | Remains Tier 3 per the output-contract tier classification; separate from skill resolution |
| `required_tools` enforcement at transport level | Remains Tier 3 per the output-contract tier classification; separate from skill resolution |
| Backend runtime settings propagation (`max_turns`, `temperature`, `effort`) | Remains Tier 3 per the output-contract tier classification; separate from skill resolution |

---

## 12. Risk assessment

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| External skill packages have inconsistent structure | Medium | Medium | Strict instruction file discovery order with clear error messages |
| Skill content is large and inflates system prompts | Low | Medium | Content size limit with fail-closed preflight; no truncation of executable content in MVP |
| Role-specific files in external skill packages are missing | Medium | Low | Graceful fallback to generic role block with preflight warning |
| Injection order conflicts with existing prompt engineering | Low | High | Skill preamble is always first; agent prompt always last; no interleaving |
| Resolved skill hashes change frequently for external skills | Medium | Low | This is correct behavior — provenance should detect external changes |

---

## 13. Migration from the output-contract tier classification

The output-contract and failure-evidence reference classified skills as Tier 3 (later proposal):

> | `skills.*` | Partial | Skill definitions exist in YAML, but skill content is not resolved into live execution. |
> | `agents.*.skill_ref` / `skill_role` | Partial | Parsed, validated, displayed, and hashed into provenance; NOT injected into Goose prompts or tool/session policy. |

After Proposal 015:

| Surface | New Status | New Truth |
|---|---|---|
| `skills.*` | **Resolved** | Skill definitions are resolved at compilation time. Content is loaded, hashed, and frozen in snapshot. |
| `agents.*.skill_ref` | **Runtime-authoritative** | Resolved skill content is injected into agent execution context via system prompt. |
| `agents.*.skill_role` | **Runtime-authoritative** | Role customizes the injected skill content. Affects the execution prompt. |

This moves skills from Tier 3 to **Tier 1 (mandatory, runtime-enforced)**.

---

## 14. Affected files (estimated)

### New files

| File | Purpose |
|---|---|
| `Engine/Skills/ResolvedSkill.swift` | Value type |
| `Engine/Skills/SkillType.swift` | Enum |
| `Engine/Skills/SkillResolver.swift` | Type-dispatched resolution |
| `Engine/Skills/ExternalSkillLoader.swift` | Filesystem loader |
| `Engine/Skills/BuiltinSkillRegistry.swift` | Known builtin agent instructions |
| `Engine/Skills/SkillInjector.swift` | Prompt injection builder |
| `Engine/Skills/SkillRoleCustomizer.swift` | Role specialization |
| `Engine/Skills/SkillPreflightCheck.swift` | Preflight validation |
| `Tests/SkillResolverTests.swift` | Resolution unit tests |
| `Tests/SkillInjectorTests.swift` | Injection unit tests |
| `Tests/SkillPreflightTests.swift` | Preflight tests |
| `Tests/SkillIntegrationTests.swift` | End-to-end tests per type |

### Modified files

| File | Change |
|---|---|
| `Engine/RunPlan.swift` | Add `resolvedSkill` to `ResolvedAgent` |
| `Engine/RunPlanCompiler.swift` | Call `SkillResolver` during compilation |
| `Engine/GooseSessionBridge.swift` | Feed `SkillInjector` output into the canonical `ExecutionPacket` path |
| `Engine/GooseAgentExecutor.swift` | Pass skill-augmented prompt to transport |
| `Engine/SimulatedAgentExecutor.swift` | Verify skill content in execution context |
| `Engine/WorkflowOrchestrator.swift` | Pass `ResolvedSkill` through to execution |
| `Engine/PreflightService.swift` | Add `SkillPreflightCheck` to preflight chain |
| `Models/AgentExecution.swift` | Change `skillSnapshotHash` semantics; add `skillType`, `skillContentSummary` |
| `DSL/AgentCatalog.swift` | No model changes needed (SkillRef already parsed) |
| `DSL/YAMLValidator.swift` | Extend validation for skill type-specific rules |
| `Views/AgentCatalogView.swift` | Skill type badge, content preview, metadata |
| `Views/RunReportView.swift` | Show skill truth in shell-owned execution reporting |
| `Views/RunComparisonView.swift` | Show skill truth drift across runs |
| `Views/ArtifactInspectorView.swift` | Show skill truth for specific execution artifacts |
| `Views/PilotReadinessView.swift` | Skill preflight section |
| `Support/MVPBoundaryPolicy.swift` | Add skill resolution to MVP boundary contract |

---

## Appendix A: External skill package structure (MVP)

MVP external skill package is a Codex bundle with this minimal structure:

```
proposal-review-triad/
  SKILL.md              # Required executable entrypoint
  references/           # Optional non-executable companion docs
  assets/               # Optional non-executable templates and media
  evals/                # Optional proof assets
  agents/               # Optional bundle-local helper config
```

The `SKILL.md` file contains the executable base instructions.

For current repo reality, `proposal_review_triad` specialization is mode-based, not role-file-based:

- `product_owner` -> `product-only`
- `ux_designer` -> `ux-only`
- `ui_designer` -> `ui-only`
- `architect` -> `architecture-only`

If a future skill explicitly opts into file-based role specialization, that must be declared as an extension to the base bundle contract. It is not the generic MVP default.

---

## Appendix B: Prompt assembly example

Given agent `proposal_reviewer_product_owner` with:
- `skill_ref: proposal_review_triad` (type: `external_skill`)
- `skill_role: product_owner`
- `prompt: "Review the proposal as a product owner..."`

And external skill package at `/Users/user/.codex/skills/proposal-review-triad/`:
- `SKILL.md` contains: "You are a proposal reviewer. Evaluate proposals across dimensions..."
- the skill-specific specialization registry maps `product_owner` -> `product-only`

Assembled system prompt:

```
## Skill: proposal_review_triad
Type: external

You are a proposal reviewer. Evaluate proposals across dimensions...

## Active Role: product_owner

Mode: product-only

As the product owner lens, focus on business value, user problem clarity...

---

Review the proposal as a product owner.
Focus on user problem clarity, business value, scope discipline, acceptance criteria, rollout risk, metrics, and dependency realism.
Be strict about missing assumptions, hidden scope, and ambiguous success criteria.
Output only the structured review contract with a numeric score from 0 to 10.
Mark blocking issues only when they would materially reduce the chance of shipping the right thing.
```

---

## Appendix C: ResolvedSkill data model

```swift
struct ResolvedSkill: Codable, Sendable, Hashable {
    let id: String                      // Skill key from catalog (e.g., "proposal_review_triad")
    let type: SkillType                 // .external, .inline, .builtin
    let resolvedContent: String         // Full instruction text
    let contentHash: String             // SHA-256 of resolvedContent
    let sourcePath: String?             // Filesystem path (external only)
    let sourceDescription: String?      // Original YAML description (inline only)
    let roleContent: [String: String]?  // Role -> role-specific content (external with roles/ dir)
}

enum SkillType: String, Codable, Sendable {
    case external
    case inline
    case builtin
}
```
