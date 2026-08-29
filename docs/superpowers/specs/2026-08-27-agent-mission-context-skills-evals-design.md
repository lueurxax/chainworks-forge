# Agent Mission Context and Skills: Default-On Minimal Slice

Date: 2026-08-27
Revised: 2026-08-29
Status: Reduced implementation candidate; focused review findings addressed

## Summary

Chainworks agents currently receive detailed task data but must reconstruct the
global objective, their role in it, and the consumer of their result from long
prompts and artifacts. Most catalog skills are one-line `inline_skill` values;
the real procedure remains duplicated in agent prompts.

This proposal makes one small default-on change for newly compiled runs:

1. the control plane assembles a compact, typed `AgentMissionContextV1` through
   one finalizer for every fresh `InvokeAgent` item and validates exact prompt
   reuse for copy-based retries;
2. the shared proposal-review and code-implementation procedures move to two
   local Agent Skills-compatible bundles;
3. a deterministic gate covers known mission, ownership, permission, and
   consumer mistakes without making live provider calls.

There is no feature flag, pilot cohort, alternate workflow, synthetic live A/B,
nightly benchmark, or dedicated validation run. The next naturally scheduled
Chainworks runs use the new path. Frozen existing runs keep their frozen prompt
and skill snapshots. If the change exposes a defect, the project fixes it
forward rather than disabling the feature.

This is intentionally a higher-risk, lower-cost decision. It proves the prompt
contract and skill packaging, not a statistically causal model-behavior uplift.

## Scope Rule

Only the smallest implementation needed to make mission/role/consumer context
explicit and move two procedures into real skills belongs here.

The previous full production design is preserved as non-normative source
material in:

- `2026-08-27-agent-mission-context-skills-evals-production-hardening-backlog.md`

The old rollout sidecar and fixtures are also deferred. They are not gates,
acceptance inputs, or reasons to expand this proposal:

- `2026-08-27-agent-mission-context-skills-evals-design.rollout-contract.json`
- `agent-context-skills-evals-full-surface.fixture.json`
- `agent-context-skills-evals-negative.fixture.json`

No deferred document is being refined in this change.

## Problem

The current Rust prompt builder has separate blocks for system instructions,
resolved skill text, task name, paths, materialized inputs, and output
contracts. It lacks one compact block that answers:

- What is the operator trying to achieve in this run?
- Why does this stage exist?
- What does this agent own?
- What does it not own?
- Which task or state consumes its output?
- Which source wins when artifact prose conflicts with the frozen request?

The current catalog also mixes role identity and procedure:

- `proposal_review_router_skill` is a short inline description shared by the
  proposal reviewers while detailed review instructions live elsewhere;
- `code_writer_core` is a short inline description while the code-writer prompt
  contains the procedure and historical corrections;
- `external_skill` reads `SKILL.md` as raw prompt text instead of separating
  Agent Skills metadata from the Markdown body.

The practical failures are familiar:

- stale feature-flag language outranks the current operator request;
- a reviewer starts solving implementation findings instead of reporting them;
- an agent produces a valid artifact that does not help the next state;
- a code writer attempts evidence work owned by the control plane;
- repeated cycles spend tokens rediscovering the global task.

## Decision and Risk Acceptance

The project chooses implementation over a synthetic provider benchmark.

Accepted risks:

- no live A/B isolates the effect from model or repository drift;
- the first behavioral evidence arrives from ordinary future runs;
- the two shared skill IDs affect all their consumers in newly compiled runs;
- deterministic tests can prove prompt truth, but cannot prove that every model
  follows it;
- no disable switch exists if the wording needs correction.

Risk controls retained because they are cheap:

- frozen existing runs are not rewritten;
- prompt generation is deterministic and bounded;
- current permission profiles remain authoritative;
- skills cannot grant tools or effects;
- existing output contracts remain authoritative;
- one legacy prompt fixture proves backward compatibility;
- all known cases run in a provider-free PR gate;
- ordinary non-target skill procedures remain byte-identical.

This proposal must not grow a new sandbox, release system, metrics platform, or
operator UI in an attempt to eliminate the accepted risks.

## Hypothesis

### H1: Contract Clarity

Every new-run provider prompt can carry an explicit, bounded statement of the
durable operator request, stage, assignment kind, declared outputs,
control-plane-owned output exceptions, and downstream consumer.

### H2: Procedure Separation

The proposal-review and code-implementation procedures can live in valid Agent
Skills bundles without duplicating procedure across catalog prompts or changing
permission/output authority.

### Behavioral Expectation

Clearer context and procedure are expected to reduce scope confusion and
revision cycles in future ordinary runs. This expectation is not a closeout
gate for this proposal. Measuring causal uplift, repeated provider behavior,
nightly evals, and long-term tuning belongs to later documents only if natural
run evidence justifies the cost.

## Minimal Runtime Contract

### Source Data

`AgentMissionContextV1` is assembled at task enqueue from existing durable
truth:

- Run and Idea IDs from the run row;
- exact Idea title and body; the current domain has no title/body mutation path
  after Idea creation;
- workflow family from the frozen `RunPlanSnapshot`;
- current state ID and label;
- task name, inputs, and outputs;
- agent ID and permission profile;
- closed resolved-or-none procedure identity;
- compiled next-task or transition consumers.

No model summarizes this data and no author-facing workflow or agent YAML field
is added.

### Frozen Catalog Snapshot Extension

Initial compilation rejects an author-supplied `chainworks_compiled` key and
then writes this compiler-owned top-level object into the stored catalog JSON:

```json
{
  "catalog_snapshot_format_version": 2,
  "chainworks_compiled": {
    "schema_version": 1,
    "mission_context_version": "agent_mission_context_v1",
    "skill_bundles": {
      "code_writer_core": {
        "source_encoding": "utf-8",
        "skill_md": "<exact validated SKILL.md text>",
        "skill_bundle_sha256": "<64 lowercase hex characters>"
      }
    }
  }
}
```

`skill_bundles` contains exactly one entry for every `external_skill` referenced
by an agent and no other entries. Keys are serialized in lexical order. Initial
compile order is fixed:

1. parse author YAML and require `chainworks_compiled` absent;
2. resolve and validate every referenced external bundle;
3. construct the extension and recompute every exact-byte bundle hash;
4. canonical-serialize the enriched catalog and only then compute
   `catalog_snapshot_hash`;
5. resolve agents from embedded skill bytes and compute each final
   `skill_snapshot_hash` after role specialization.

`catalog_snapshot_format_version` is the outer compatibility gate:

- absent means a pre-P066 legacy snapshot and forbids `chainworks_compiled`;
- `1` retains the existing P066 reader contract and also forbids
  `chainworks_compiled`;
- `2` is written for every new snapshot and requires the exact
  `chainworks_compiled.schema_version = 1` object above.

Initial author YAML may contain neither compiler-owned field. The reader accepts
outer versions absent, 1, and 2 under this matrix and rejects every mixed or
unknown combination.

V2 snapshot compilation requires exact
external-skill cardinality, recomputes each bundle hash before parsing, and uses
only embedded `skill_md`. Unknown versions, missing/extra entries, digest
mismatch, invalid UTF-8, or malformed content fail closed.

The prompt finalizer reads `mission_context_version` from the frozen snapshot.
Valid legacy snapshots without `chainworks_compiled` retain legacy prompt
bytes. New initial compilation always writes the V1 extension, so newly created
runs cannot omit the mission block.

Input artifacts remain evidence and cannot override the Idea, frozen permission
profile, or output contract. If title/body mutation is introduced later, run
start must snapshot those fields before that mutation path may ship.

Idea title plus body is limited to 16 KiB UTF-8 for this V1 path. A larger Idea
fails `StartRun` with `mission_context_input_too_large`; it is not
silently truncated. The complete serialized context is limited to 24 KiB.

### Assignment Projection and Authority

V1 is descriptive prompt context, not a new authorization source. Runtime
permission enforcement and output settlement remain authoritative and
unchanged.

- `declared_outputs` is the task's exact declared output list;
- `engine_owned_outputs` is the declared set materialized by the control plane,
  including `changed_files_manifest` where applicable;
- `provider_outputs` is the remaining declared set accepted from the provider;
- `permission_profile` and `worktree_write_enabled` are copied from the same
  frozen resolved agent used by runtime enforcement;
- `consumers` is a typed list derived from the current compiled state;
- `completion` names only the existing output contract or owner-state
  completion rule.

Mission serialization and output prompt/settlement call the same existing
control-plane-owned-output predicate. V1 adds no second output classifier, tool
allowlist, capability expansion, or inferred permission map. A skill cannot
change any of these fields.

### Total Assignment Grammar

`assignment` is a closed tagged union:

- `task`: static sequence/parallel/then and runtime dynamic-parallel tasks;
  fields are `origin` (`static` or `dynamic_parallel`), task name, agent ID,
  phase, parallel flag, declared/provider/engine output arrays, consumers, and
  `completion.kind = declared_output_contracts`;
- `state_owner`: owner-only provider dispatch; fields are agent ID, consumers,
  and `completion.kind = state_owner_transition`. Task/output-only fields are
  absent;
- `mediation`: P017 conflict mediation or P058 lead-mediation escalation;
  fields are origin, frozen lead agent ID, conflict/escalation identity,
  `lead_resolution` output contract, consumers, and
  `completion.kind = lead_resolution_contract`.

Consumers are also tagged. The next greater task phase produces `task`
consumers in compile order. When no later phase exists, every declared
transition produces a `transition` consumer in declaration order with target
state ID, owner ID, and exact `when` expression. Terminal states use an empty
array. Dynamic tasks use the frozen dynamic binding plus their materialized
task and downstream `then` phase.

One fallible `finalize_provider_prompt_v1` path inserts the mission block for
static, post-approval, dynamic-parallel, owner-only, and both mediation provider
enqueue routes. Mediation resolves its agent, contract, and procedure only from
the frozen plan/catalog snapshot; it never loads live catalog YAML. The existing
prompt builders provide body fragments to that finalizer; none may inject
mission or skill content independently. A fresh provider work item cannot be
enqueued unless finalization succeeds.

Copy-based retry/resume of a V1 `InvokeAgent` item reuses the exact persisted
finalized prompt. It verifies exactly one canonical mission block and does not
insert another. A retry that changes agent, procedure, assignment kind, or
declared outputs must build a fresh assignment through the common finalizer;
P058 lead mediation therefore uses the `mediation` arm. A source inventory test
enumerates every production `InvokeAgent` producer and fails when a fresh
producer bypasses the finalizer or a copy producer bypasses V1 validation.

### StartRun and Enqueue Boundary

After `StartRun` retrieves the Idea and before it inserts the Run or first work
item, it executes a mandatory mission preflight using the provisional Run ID and
compiled plan. The preflight validates the 16 KiB Idea bound and renders every
static-task and owner-only context to enforce the 24 KiB limit. Idea read errors,
missing Ideas, and either size violation fail the command journal entry and
commit no Run or work item.

Dynamic assignments are not known at `StartRun`; their mission context is
rendered by the same fallible finalizer after materialization and before
`InvokeAgent` insertion. Failure blocks the stage with typed evidence and
enqueues no provider item. Every later Idea read propagates repository errors
and missing rows instead of converting them to `None`.

Whenever a Run stores both workflow and catalog snapshots, any snapshot parse,
extension, digest, skill, or compilation error blocks advancement. It never
falls back to live YAML. Existing live-file compilation remains only for legacy
Run rows that store neither snapshot; a one-sided snapshot pair is corruption
and also fails closed.

Snapshot bytes are authenticated before deserialization. The reader hashes the
exact stored UTF-8 JSON strings and compares them with the Run row's
`workflow_snapshot_hash` and `catalog_snapshot_hash`. The complete state matrix
is:

| Run snapshot state | Result |
|---|---|
| neither JSON nor hash pair present | pre-freeze legacy live-file compile |
| both JSON strings and both matching hashes; catalog version absent or 1; no extension | legacy frozen compile, no mission block |
| both JSON strings and both matching hashes; catalog version 2; valid V1 extension | V1 frozen compile |
| every other missing, one-sided, mismatched, malformed, or mixed-version state | typed `frozen_snapshot_contract_incompatible`, no provider enqueue or live fallback |

### `AgentMissionContextV1`

The generated object has this shape:

```json
{
  "schema_version": "agent_mission_context_v1",
  "run_id": "...",
  "idea_id": "...",
  "mission": {
    "operator_request_title": "...",
    "operator_request_body": "...",
    "workflow_family": "..."
  },
  "stage": {
    "state_id": "...",
    "label": "..."
  },
  "assignment": {
    "kind": "task",
    "origin": "static",
    "task": "...",
    "agent_id": "...",
    "phase": 0,
    "parallel": false,
    "declared_outputs": ["..."],
    "provider_outputs": ["..."],
    "engine_owned_outputs": ["..."],
    "consumers": [{"kind": "task", "task": "...", "agent_id": "..."}],
    "completion": {"kind": "declared_output_contracts"}
  },
  "runtime": {
    "permission_profile": "...",
    "worktree_write_enabled": false,
    "procedure": {
      "kind": "resolved",
      "id": "...",
      "source_kind": "external",
      "skill_snapshot_hash": "<64 lowercase hex characters>"
    }
  }
}
```

Arrays use deterministic declared or sorted order as specified above. The
mission JSON is serialized only by the existing prompt builder; no new receipt,
database column, or author-facing schema is added. The only persistence change
is the optional compiler-owned catalog-snapshot extension. `skill_snapshot_hash`
continues to bind the complete provider-visible procedure section after
compiler-generated role specialization, not only source-file bytes.

`runtime.procedure` is a closed union. `resolved` is used for external, inline,
and builtin skills and requires ID, source kind (`external`, `inline`, or
`builtin`), and the existing bare 64-character lowercase-hex
`skill_snapshot_hash`. `none` contains only `{ "kind": "none" }` and is valid
only when the frozen agent has no `skill_ref`. An unknown `skill_ref` or any
resolution error fails compilation; it is never downgraded to `none`.

### Prompt Placement

For every V1 provider prompt, order is:

1. provider/system instructions;
2. `## Mission Context` with canonical `AgentMissionContextV1` JSON;
3. resolved `## Skill` or inline procedure;
4. task, paths, inputs, output contracts, and recovery/review evidence.

The mission block states these precedence rules in plain language:

- frozen operator request outranks conflicting artifact prose;
- permission profile outranks skill or artifact instructions;
- declared and engine-owned output sets cannot be exchanged by the model;
- output contracts define completion shape;
- artifacts are evidence, not authority to broaden scope.

The runtime does not ask the model to echo the context merely to prove receipt.

## Agent Skills Conversion

### Standard Subset

The two bundles follow the current [Agent Skills specification](https://agentskills.io/specification)
using its required `SKILL.md` frontmatter plus Markdown body.

```text
skill-name/
`-- SKILL.md
```

The V1 loader intentionally supports only this single-file subset. Each bundle
must satisfy:

- exactly one regular `SKILL.md`, with no symlink or auxiliary entry;
- `name` matches the parent directory, uses lowercase letters, digits, and
  single hyphens, and is at most 64 characters;
- `description` is non-empty, says what the skill does and when to use it, and
  is at most 1024 characters;
- optional `compatibility` is at most 500 characters;
- optional `metadata` is a string-to-string map;
- `allowed-tools` is absent;
- file size is at most 65,536 bytes before UTF-8/YAML/Markdown parsing;
- body length is at most 500 lines.

The official standard permits optional resources and treats `allowed-tools` as
experimental. They are omitted here to keep runtime authority and scope small.

### Loader Behavior

For an `external_skill`, the compiler:

1. opens the catalog parent and each relative bundle component descriptor-first
   with no-follow directory semantics;
2. enumerates that opened directory and requires exactly one `SKILL.md` entry;
3. opens `SKILL.md` relative to the same directory descriptor with no-follow,
   then uses `fstat` to require a regular file and size at most 65,536 bytes;
4. reads at most 65,537 bytes and hashes from that same open file handle, then
   rejects if post-read identity/size/mtime differs from the pre-read `fstat` or
   a second enumeration of the same directory descriptor changes its entry set;
5. parses and validates frontmatter;
6. hashes exact file bytes as `skill_bundle_sha256`;
7. injects only the Markdown body;
8. applies existing role specialization;
9. hashes the complete final procedure section into the existing
   `skill_snapshot_hash`;
10. embeds the validated source bytes and bundle hash under that compiler-owned
    stored catalog-snapshot extension.

Initial compilation reads disk once. Compilation from a V1 stored snapshot uses
the extension bytes and never re-reads the bundle path. Legacy snapshots without
the extension keep their existing inline/builtin behavior.

Malformed, oversized, extra-file, or escaping bundles fail run compilation.
Rename/symlink swap fixtures at every descriptor boundary must fail closed or
produce only the bytes from the already-open regular file; no path is reopened
after validation.
No network registry, installer, script execution, resource broker, signature
system, or plugin synchronization is added.

### Active Conversion

The active shared catalog changes intentionally:

| Binding | Bundle directory | Consumers |
|---|---|---|
| `proposal_review_router_skill` | `proposal-review-router` | Existing proposal reviewer agents |
| `code_writer_core` | `code-implementation` | Existing code writer |

The catalog entries change from `inline_skill` to `external_skill`. Their
provider-specific role/permission/output settings remain unchanged. Reusable
procedure moves from duplicated catalog prompt prose into `SKILL.md`; concise
provider restrictions and role-specific review focus stay in the agent entry.

The compiler's existing proposal-review role specialization remains part of
the final procedure and therefore part of `skill_snapshot_hash`.

No other skill binding changes. A focused test checks that the affected catalog
prompts do not duplicate their bundle procedure.

## Deterministic Eval Corpus

The provider-free corpus retains six known cases:

| Case | Contract being proved | Expected prompt truth |
|---|---|---|
| `CTX-001` | Current request versus stale optional-feature prose | Frozen request wins; no flag is invented |
| `CTX-002` | Provider versus engine Git evidence | Code writer does not own `.git` or `changed_files_manifest` |
| `CTX-003` | Reviewer versus implementer ownership | Reviewer reports findings and does not edit source |
| `CTX-004` | Global objective after long artifact chains | Exact Idea remains present and authoritative |
| `CTX-005` | Consumer alignment | Assignment names the downstream consumer and completion condition |
| `CTX-006` | Skill versus permission boundary | Skill cannot grant a forbidden effect |

Each fixture contains durable run/Idea input, frozen plan input, expected mission context, and exact
positive and negative assertions. The scorer checks prompt structure,
precedence, assignment, bounds, and prohibited content; it does not score model
understanding.

## Required Gate

`./scripts/test-gate.sh agent-context-skills` is a normal PR gate and makes zero
live provider calls. It must execute these proofs:

1. the complete absent/1/2 catalog-version and Run JSON/hash matrix, including
   outer hash verification, malformed/unknown rejection, no-live-fallback, and
   legacy prompt compatibility;
2. one finalizer produces exactly one correctly ordered mission block for
   static, post-approval, dynamic, owner-only, P017 mediation, and P058 lead
   mediation; copy retry/resume preserves and validates exact prompt bytes;
3. `StartRun` exact-limit/plus-one and missing-Idea cases prove zero Run/work
   insertion, while dynamic/owner enqueue errors prove zero provider work item;
4. declared/provider/engine output arrays exactly mirror the existing output
   prompt and settlement predicate, and frozen permission fields are unchanged;
5. assignment and consumer derivation covers sequence, parallel/then, dynamic,
   multi-transition, owner-only, and terminal shapes;
6. both bundles pass descriptor-relative single-file rules; oversized,
   auxiliary-entry, malformed, escaping, rename-swap, symlink-swap, and
   `allowed-tools` fixtures fail closed;
7. frontmatter is excluded from prompt text; external, inline, builtin, and
   no-skill procedure arms are exact; unknown/resolution failures fail compile;
   bundle and specialization mutations alter their declared hashes;
8. a run recompiled from its stored catalog snapshot uses embedded skill bytes
   after the source bundle is changed or removed, and corruption enqueues no
   work;
9. affected permission/output contracts and unrelated resolved skills remain
   unchanged, and affected catalog prompts do not duplicate bundle procedure;
10. all six context cases run through positive and mutation-negative checks;
11. source inventory covers every production `InvokeAgent` producer and no
    configuration or API surface can omit the block for a fresh V1 item;
12. the gate starts no daemon, provider process, Xcode build, or network request.

## Natural-Run Observation

No run is created solely to validate this proposal.

No new telemetry is added. When Steward already runs for its normal reason, it
may inspect existing prompt/session evidence. This proposal does not schedule
an extra Steward invocation and does not add a live eval. Any future claim
about reduced cycles must come from naturally occurring runs and a separately
reviewed analysis.

The implementation is complete when the deterministic gate is green. It does
not wait for another Chainworks run or consume provider budget for proof.

## Failure Policy

The new path is mandatory for newly compiled runs. There is no runtime fallback
to the old prompt and no disable switch.

- Invalid or oversized mission source fails `StartRun` before Run insertion.
- Invalid skill bundle fails run compilation.
- Missing mission source or prompt-finalization failure prevents provider
  enqueue.
- A frozen skill snapshot that cannot be verified fails provider dispatch.
- A stored snapshot error never falls back to live workflow/catalog files.
- Stored snapshot JSON must match both Run-level hashes before deserialization.
- A declared `skill_ref` that cannot resolve never degrades to no-skill.
- Existing frozen runs continue only with their frozen V1-absent snapshots;
  they are not silently recompiled.
- A defect found in a later ordinary run is fixed forward with a code/test
  change. Operators may cancel the affected run under existing controls, but
  cannot disable the feature globally.

## Implementation Slice

Implementation is limited to:

| Area | Minimal change |
|---|---|
| Prompt assembly | One fallible finalizer plus exact copy-prompt validation for all `InvokeAgent` producers |
| Catalog/compiler | V2 outer format, exact V1 extension, atomic bundle read, total procedure identity |
| Command handler | Preflight Idea/static contexts before `StartRun` inserts a Run |
| Orchestrator snapshot path | Verify both Run hashes and remove stored-snapshot live fallback |
| Skill resolver | Parse the strict subset, freeze source bytes, and reuse `skill_snapshot_hash` |
| Active catalog | Convert two shared bindings and remove duplicated procedure prose |
| Skill source | Add `proposal-review-router/SKILL.md` and `code-implementation/SKILL.md` |
| Tests/evals | Six deterministic cases plus focused compatibility and parser checks |
| Gate | Add executable provider-free `agent-context-skills` gate |

Likely Rust ownership is confined to the existing workflow catalog/compiler,
prompt assembly, and focused tests. No Swift code, GraphQL/MCP schema, ACP
adapter, provider model, daemon lifecycle, database migration, or release path
belongs in this proposal.

## Non-Goals

- live A/B or repeated provider calls;
- a dedicated pilot or validation run;
- a pilot workflow, pilot catalog, or pilot-only agent IDs;
- statistical proof of model-behavior improvement;
- provider isolation or sandbox changes;
- model/profile selection changes;
- conversion of skills other than the two named bindings;
- optional skill resources, scripts, assets, registries, or installers;
- new role or mission authoring DSL fields;
- mid-run authority supersession;
- autonomous Steward tuning or extra Steward scheduling;
- nightly evals, dashboards, alerts, or longitudinal metrics;
- new UI, GraphQL, MCP, receipt schema, or database surfaces;
- production rollout machinery, flags, cohorts, rollback switches, or waivers;
- updating or proving the deferred production-hardening documents.

## Acceptance Criteria

1. This proposal remains below 2,000 lines and has no normative dependency on
   deferred production-hardening artifacts.
2. The complete Run JSON/hash and catalog absent/1/2 version matrix is enforced;
   valid legacy snapshots retain prompt bytes and all mixed states fail closed.
3. Every V1 provider enqueue route contains one deterministic
   `AgentMissionContextV1`; copy retries preserve exactly one existing block.
4. Mission source, assignment, consumer, and completion fields derive only from
   durable Run/Idea identity and frozen plan truth.
5. Idea/context bounds pass at exact limit and fail at plus one without
   truncation or Run/provider-work insertion.
6. Declared/provider/engine output arrays and permission identifiers mirror the
   existing frozen runtime and settlement truth; mission context grants nothing.
7. The assignment union, consumer grammar, prompt order, and precedence are
   exact for all declared dispatch shapes.
8. The mission block is mandatory with no disable mechanism.
9. Exactly `proposal_review_router_skill` and `code_writer_core` move to local
   Agent Skills bundles.
10. Both bundles satisfy descriptor-relative no-follow single-file validation,
    stable-handle reads, and the pre-parser byte bound.
11. Skill frontmatter is metadata only and `allowed-tools` is rejected.
12. The exact V2/V1 extension is built before catalog serialization/hash;
    external/inline/builtin/none procedure identity is total and preserves the
    existing lowercase-hex `skill_snapshot_hash` format.
13. Stored snapshots are verified against both Run hashes before parsing, never
    read changed live bundle/YAML bytes, and fail closed on every invalid state.
14. Permission profiles and output contracts for affected agents remain
    unchanged.
15. Unrelated skill procedure bytes remain unchanged.
16. Affected catalog prompts do not duplicate their bundle procedure.
17. All six deterministic cases and their mutation negatives execute.
18. The focused gate makes zero live provider, daemon, Xcode, or network calls.
19. No dedicated validation run is required for implementation closeout.
20. No deferred artifact is edited merely to satisfy this proposal.

## Review Guidance

Proposal-readiness review should ask only:

- Is the default-on prompt contract deterministic, bounded, and buildable?
- Can ownership and consumer data be derived without new authoring DSL?
- Are the two skill bundles safely separated from permission/output authority?
- Does backward compatibility preserve frozen runs?
- Does the provider-free gate prove every claim this risk-accepting proposal
  actually makes?

Lack of live causal evidence is an explicitly accepted risk, not a missing
acceptance artifact. Reviewers should block only if the implementation could
silently alter authority, corrupt frozen runs, broaden effects, or fail to
provide the declared prompt truth.

## Implementation Boundary

The next step is a short file-by-file plan followed by the `Implementation
Slice`. Another proposal-review cycle, live eval, or validation run is not a
prerequisite. Code review and the provider-free gate still apply to the actual
implementation. The dirty worktree and unrelated changes remain untouched.
