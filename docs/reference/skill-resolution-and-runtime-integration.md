# Skill Resolution and Runtime Integration

Stable reference for how Chainworks Forge resolves agent skills, injects them into runtime execution, freezes skill truth into runs, and exposes that truth back to operator surfaces.

## Purpose

Skills are part of the execution contract, not prompt decoration.

The system must be able to answer, for any agent execution:

- which skill reference the catalog selected,
- which concrete content was resolved at run start,
- how role-specific specialization changed that content,
- what exact injected snapshot was sent to the runtime,
- and where the operator can inspect that frozen truth later.

## Scope

This reference covers:

- catalog-owned `skill_ref` and `skill_role`,
- builtin, inline, and external skill resolution,
- role-aware specialization,
- injected runtime content generation,
- frozen run-start skill truth,
- persisted execution-time skill provenance,
- and shell-owned readback in reports, comparison, and artifact inspection.

It does not define:

- the general provider/runtime transport layer,
- MCP policy and runtime extension settlement,
- or arbitrary Codex skill authoring outside the app's catalog/runtime contract.

## Related docs

- [workflow-execution-engine.md](workflow-execution-engine.md)
- [runtime-contract.md](runtime-contract.md)
- [operator-experience.md](operator-experience.md)
- [run-surface-information-architecture-and-artifact-hierarchy.md](run-surface-information-architecture-and-artifact-hierarchy.md)
- [per-agent-mcp-policy-and-runtime-validation.md](per-agent-mcp-policy-and-runtime-validation.md)

## Skill truth model

### Catalog truth

Agent definitions own the stable skill binding:

- `skill_ref` selects the catalog skill entry,
- `skill_role` selects an optional specialization mode,
- the catalog skill record defines whether the skill is builtin, inline, or external.

No runtime surface should invent a different skill owner.

### Resolved skill truth

`RunPlanCompiler` resolves every referenced skill before the run starts.

The resolved value is a `ResolvedSkill` that carries:

- skill type,
- source path or source description,
- resolved content,
- injected content,
- content hash,
- injected-content hash,
- specialization summary,
- optional bundle manifest metadata.

### Execution truth

The app persists both raw identity and injected snapshot truth:

- `Run.resolvedSkillsJSON`
- `Run.skillContentHashesJSON`
- `Run.skillInjectedContentHashesJSON`
- `AgentExecution.skillRef`
- `AgentExecution.skillRole`
- `AgentExecution.skillSnapshotHash`
- `AgentExecution.skillContentSummary`

This preserves the exact run-time skill explanation even if catalog files change later.

## Resolution pipeline

The implemented resolution path is:

```text
AgentCatalog
  -> RunPlanCompiler
    -> SkillResolver
      -> ExternalSkillLoader / builtin / inline source
      -> SkillRoleCustomizer
      -> SkillInjector
    -> ResolvedAgent
  -> RunStartSnapshot
  -> RuntimeSessionBridge / runtime execution packet
  -> AgentExecution + reports + inspector
```

### Builtin skills

Builtin skills resolve from app-owned stable skill content.
They do not depend on external repo paths.

### Inline skills

Inline skills resolve directly from catalog text.
They are useful for small agent-local directives that should stay in the YAML source of truth.

### External skills

External skills resolve from explicit external bundles.

Current rules:

- external resolution is rooted in explicit catalog base context,
- relative paths are resolved against the catalog source,
- the loader reads bundle content plus companion metadata,
- resolution fails closed when the bundle cannot be read or the skill contract is malformed.

The app does not silently substitute a different skill when external resolution fails.

### Role specialization

`skill_role` is part of the contract, not an ad hoc prompt suffix.

Role-aware specialization currently supports review-style shared skills such as one bundle serving:

- `product_owner`
- `ux`
- `ui`
- `architect`

The specialized resolved content is what gets frozen and injected.

## Runtime injection

`SkillInjector` produces the text that actually enters the runtime packet.

Important distinction:

- `resolvedContent` is the human-readable skill truth,
- `injectedContent` is the exact runtime packet contribution after specialization and injection policy are applied.

Operator surfaces should prefer frozen execution-time truth over re-resolving current disk content.

## Frozen run behavior

Skill resolution happens before the run becomes durable execution.

At run start the app freezes:

- resolved skill payloads,
- skill hashes,
- injected hashes,
- and agent bindings that reference those frozen skills.

Resume, comparison, and reports read from frozen run truth, not from live catalog files.

This is what prevents stale local edits from rewriting historical run explanation.

## Operator-visible surfaces

Skill truth is integrated into the existing operator shell rather than a separate inspector lane.

### Reports

Run reports surface:

- `skillRef`
- `skillRole`
- injected snapshot hash
- resolved skill content where the report format supports it

### Comparison

Run comparison can explain skill drift by comparing frozen skill provenance instead of only provider/model differences.

### Artifact inspector

Artifact inspection exposes the producing execution's skill identity and frozen skill truth so that a report or receipt can be understood without reopening catalog files.

### Agent catalog view

`AgentCatalogView` remains the static catalog-facing inspection surface:

- current skill ref,
- role,
- resolved type,
- source path/description,
- content hash,
- injected hash,
- specialization summary.

This is configuration truth, not historical run truth.

## Integration with the rest of the app

The skill system plugs into the existing app in four places:

1. catalog/preflight validation,
2. run compilation,
3. runtime execution,
4. operator readback.

That means skill functionality is not a sidecar feature. It is part of:

- run compilation,
- runtime provenance,
- and operator explainability.

## Current implementation owners

- `Chainworks Forge/Engine/Skills/ResolvedSkill.swift`
- `Chainworks Forge/Engine/Skills/SkillResolver.swift`
- `Chainworks Forge/Engine/Skills/ExternalSkillLoader.swift`
- `Chainworks Forge/Engine/Skills/SkillRoleCustomizer.swift`
- `Chainworks Forge/Engine/Skills/SkillInjector.swift`
- `Chainworks Forge/Engine/RunPlanCompiler.swift`
- `Chainworks Forge/Engine/RunStartSnapshot.swift`
- `Chainworks Forge/Models/AgentExecution.swift`
- `Chainworks Forge/Views/RunReportView.swift`
- `Chainworks Forge/Views/RunComparisonView.swift`
- `Chainworks Forge/Views/ArtifactInspectorView.swift`
- `Chainworks Forge/Views/AgentCatalogView.swift`

## Verification baseline

Current stable verification for this slice is:

- dedicated skill-resolution capability regression coverage on the current tree
- approved-host non-UI verification summary `15/15` passed
- canonical app-proof export on the approved host
- same-tree approved-host `full` green basis:
  - `full-20260408-101540.xcresult`
