# Mission Context, Skill Resolution, and Runtime Integration

Stable reference for how Chainworks Forge gives every Rust-owned provider
invocation a bounded mission, resolves agent procedures, freezes both contracts
into a run, and exposes skill truth back to operator surfaces.

## Purpose

Mission context and skills are execution contracts, not prompt decoration.

The system must be able to answer, for any agent execution:

- which skill reference the catalog selected,
- which concrete content was resolved at run start,
- how role-specific specialization changed that content,
- what exact injected snapshot was sent to the runtime,
- which durable mission, stage, assignment, and completion contract authorized
  the invocation,
- and where the operator can inspect that frozen truth later.

## Scope

This reference covers:

- the default-on Rust `AgentMissionContextV1` contract,
- prompt finalization and persisted-copy validation,
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

## Default-on Rust invocation contract

New Rust-compiled runs use catalog snapshot format `2`. The compiler owns the
`chainworks_compiled` extension and freezes:

- extension schema version `1`,
- `mission_context_version = agent_mission_context_v1`,
- every referenced external `SKILL.md` bundle,
- bundle and injected-content hashes,
- and the resolved agent authority already present in `RunPlan`.

Authors cannot supply or override the compiler-owned extension. There is no
feature flag, cohort, disable switch, or fresh-run fallback that removes
mission context.

Legacy frozen catalog snapshots with absent or format `1` metadata remain
readable when they contain only legacy builtin/inline skills. They fail closed
if they reference external skills without authenticated embedded bytes. Format
`2` snapshots validate the complete extension and embedded bundle cardinality
before reuse; resume never reloads changed live bundle bytes.

### Input bounds

The control plane rejects oversized context before provider work is created:

- Idea title plus body: at most `16 KiB`;
- serialized mission JSON inside a persisted prompt: at most `24 KiB`, checked
  before JSON deserialization;
- external `SKILL.md`: at most `65,536` bytes and `500` body lines.

Preflight failure leaves no durable Run or provider work. Dynamic fan-out is
prepared completely before its materialization and work rows are committed in
one transaction. A finalizer failure settles its typed blocked evidence, Stage,
and Run atomically.

### `AgentMissionContextV1`

Every fresh V1 `InvokeAgent` prompt contains exactly one canonical JSON object
with these closed sections:

- `schema_version`, `run_id`, and `idea_id`;
- `mission`: operator request title/body and frozen workflow family;
- `stage`: frozen state ID and label;
- `assignment`: exactly one of `task`, `state_owner`, or `mediation`;
- `runtime`: permission profile, worktree-write disposition, and total
  procedure identity (`resolved` or `none`).

Task assignments include static/dynamic origin, task name, phase, parallel
shape, declared outputs, provider-owned outputs, engine-owned outputs, direct
consumers, and completion contract. State-owner assignments name transition
consumers. Mediation assignments name the P017/P058 origin, frozen system lead,
durable conflict or escalation ID, lead-resolution contract, transition
consumers, and completion contract.

Task inputs remain task-body/materialization data. They are intentionally not a
field in the closed mission object and cannot broaden the frozen assignment.

### Prompt order

The common finalizer emits sections in this order:

1. frozen agent system instructions, when present;
2. one `## Mission Context` block with canonical JSON;
3. frozen precedence rules;
4. resolved external procedure content, when present;
5. the task-specific body and materialized input guidance.

Procedure prose is injected once. Active external-skill agents do not retain a
second copy of the same reusable procedure in their catalog prompt.

### Output ownership

The mission projects existing output authority; it does not create new write
permission. Declared outputs are partitioned into provider-owned and
control-plane-owned outputs. `changed_files_manifest` is control-plane-owned:
provider agents do not need direct Git metadata access to produce it.

Runtime permission profiles, worktree strategy, provider/model binding, MCP
requirements, output contracts, and side-effect policy remain the enforcing
authorities. Mission text cannot override them.

### Persisted copy and retry validation

Retry and resume preserve the original prompt bytes. The control plane parses
and validates them without regenerating the prompt:

- exactly one bounded mission block must exist;
- Run and Idea IDs/title/body must match durable truth;
- stage, task/owner, phase, parallel shape, consumers, output partition, agent
  authority, permission, and procedure hashes must match the frozen plan;
- dynamic copies must retain a frozen binding identity;
- P017 mediation copies must match the unique frozen system lead and
  `lead_resolution_contract`, durable workflow conflict, mediation record, and
  execution owner relation;
- P058 mediation copies must match the unique frozen system lead and contract,
  durable escalation ledger, frozen policy hash, current `lead_mediation` tier,
  and stage.

A mediation copy without an unambiguous durable authority anchor fails closed.
All four current copy/retry paths perform this validation before payload
mutation or retry/work/state writes. P058 lead-tier construction performs a
second validation after replacing agent authority and before opening its write
transaction.

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

The Swift app keeps its existing canonicalized bundle-loader contract. The Rust
control plane uses a narrower production format for newly compiled runs:

- paths are descriptor-relative and confined to the canonical catalog root;
- path components and final files are opened without following symlinks;
- a production bundle contains exactly one regular `SKILL.md` and no auxiliary
  files;
- frontmatter is validated and `allowed-tools` is rejected because tools remain
  catalog/runtime authority;
- malformed UTF-8, oversized content, path escape, symlink/rename races, and
  unexpected entries fail compilation;
- the validated file bytes are embedded into the frozen format `2` snapshot.

The app does not silently substitute a different skill when external resolution fails.

### Active production bundles

The current catalog resolves these five procedures from strict local bundles:

| Catalog skill ID | Bundle |
|---|---|
| `proposal_review_router_skill` | `examples/agents/skills/proposal-review-router/SKILL.md` |
| `proposal_implementation_audit` | `examples/agents/skills/implementation-audit/SKILL.md` |
| `code_writer_core` | `examples/agents/skills/code-implementation/SKILL.md` |
| `security_checker_core` | `examples/agents/skills/security-review/SKILL.md` |
| `prepush_review_core` | `examples/agents/skills/prepush-review/SKILL.md` |

`docs_quality_guardian` remains builtin. `orchestrator_core`,
`proposal_writer_core`, `github_commit_push`, `connect_publisher`, and
`steward_core` remain inline. Moving those procedures, adding skill resources
or scripts, and introducing provider evaluation are future roadmap work and
require separately reviewed bounded proposals.

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

Skill and mission resolution happen before the run becomes durable execution.

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

Rust execution owners:

- `control-plane/crates/workflow/src/compiler.rs`
- `control-plane/crates/workflow/src/skill_bundle.rs`
- `control-plane/crates/workflow/src/plan.rs`
- `control-plane/crates/engine/src/agent_mission_context.rs`
- `control-plane/crates/engine/src/orchestrator.rs`
- `control-plane/crates/engine/src/command_handler.rs`
- `control-plane/crates/engine/src/p058_deadline_resume.rs`
- `control-plane/crates/engine/tests/agent_context_skills.rs`
- `control-plane/crates/engine/tests/fixtures/agent_context/`

Swift resolution and operator-readback owners:

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

Current stable verification is the provider-free canonical gate:

```bash
./scripts/test-gate.sh agent-context-skills
```

The gate executes the closed `CTX-001..008` corpus and twelve-clause proof
manifest, strict bundle and frozen-snapshot compatibility tests, recursive
InvokeAgent producer inventory, exact prompt/copy mutation negatives, dynamic
atomicity and zero-work failure proofs, P017/P058 durable mediation authority,
and P058 deadline/resume regressions. It requires no daemon, network, UI host,
or live provider.
