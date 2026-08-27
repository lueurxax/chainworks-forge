# Agent Mission Context, Agent Skills, and Evaluation Design

Date: 2026-08-27
Status: Design approved

## Summary

Chainworks agents currently receive provider instructions, one resolved skill
fragment, task and artifact paths, materialized input artifacts, output
contracts, and runtime identifiers. They do not receive one compact,
control-plane-owned statement of the run mission, the purpose of the current
stage, their role in that mission, or the consumer of their result.

At the same time, most production skills in `examples/agents/agents.yaml` are
one-line `inline_skill` descriptions while detailed procedures and role rules
live in large per-agent prompts. Existing external skill bundles are split
between example and plugin trees, and their eval scenarios are declarative
documents rather than an executable quality system.

This design replaces that arrangement with three separate contracts:

1. typed, frozen mission and assignment context owned by the control plane;
2. Agent Skills-compatible procedural bundles with one canonical source;
3. deterministic and live evaluation lanes that compare candidate behavior
   with a frozen baseline.

The new production path is mandatory for every newly compiled run. It has no
feature flag, disable switch, cohort, or legacy single-path fallback. Existing
frozen runs remain replayable from their existing snapshots.

## Problem Evidence

The current implementation has several structural failure modes.

- A normal task sees the Idea only when the task explicitly declares an Idea
  input. Later implementation and review agents usually infer the global goal
  from large proposal and review artifacts.
- Workflow states have labels and owners, and tasks have names, but neither has
  a typed purpose or completion outcome.
- Agent role, reusable procedure, runtime restrictions, output-shape advice,
  and historical corrections are mixed inside long catalog prompts.
- `external_skill` resolution reads the complete `SKILL.md` as raw prompt text,
  including frontmatter, and does not expose bundle resources through a
  versioned run-local manifest.
- Production agents primarily use `inline_skill`, so the skill layer has little
  practical effect on behavior.
- Existing skill scenarios are not executed against a baseline, repeated
  across production providers, or scored by retained assertions.

The retained P095 history provides a concrete authority failure. An older run
brief required a default-off canary while a later explicit operator directive
required mandatory activation with no feature flag. Reviewers continued to
reason from the stale brief. The model did not have a typed authority chain
that made the supersession visible and enforceable.

## Current Skill Inventory

The production catalog currently declares twelve skill IDs. Eleven are
`inline_skill` entries and one, `docs_quality_guardian`, is a hardcoded
`builtin_agent`. None of the Agent Skills-style bundles under `examples/skills`
or `plugins/proposal-lifecycle-review/skills` is used as an external production
bundle by the current catalog.

The migration inventory is:

| Current skill ID | Current form | Canonical bundle disposition |
|---|---|---|
| `proposal_review_triad` | inline compatibility alias | retire for new catalogs; historical snapshot compatibility only |
| `proposal_review_router_skill` | inline | `proposal-review-router` |
| `proposal_implementation_audit` | inline | `proposal-implementation-audit` |
| `docs_quality_guardian` | builtin | `docs-quality-guardian` |
| `orchestrator_core` | inline | `lead-orchestration` |
| `proposal_writer_core` | inline | `proposal-authoring` |
| `code_writer_core` | inline | `code-implementation` |
| `security_checker_core` | inline | `security-review` |
| `prepush_review_core` | inline | `prepush-review` |
| `github_commit_push` | inline | `github-delivery` |
| `connect_publisher` | inline | `connect-publishing` |
| `steward_core` | inline | `forge-steward` |

The example and plugin copies of proposal review and implementation audit have
different digests today. The canonical registry becomes the authoring source;
plugin packaging is generated from it and exact parity is enforced by a gate.

## Goals

- Make the global run mission visible and authoritative in every provider
  invocation.
- Tell every agent why the current task exists, what the agent owns, what it
  must not own, and what valid completion means.
- Separate role identity from reusable procedures and from runtime policy.
- Adopt Agent Skills bundle structure and validation for production skills.
- Freeze exact context, role, skill, resource, and eval provenance with the run.
- Turn historical Chainworks failures into executable regression cases.
- Measure whether skill, context, workflow, model, or budget changes improve
  behavior relative to a frozen baseline.
- Let Steward detect and propose tuning work without allowing autonomous
  mutation of canonical production skills.

## Non-Goals

- Add new agent IDs or reviewer disciplines.
- Add a runtime feature-management system.
- Let a provider choose its own role, authority order, permissions, or required
  outputs.
- Make untrusted artifacts part of system instructions.
- Automatically merge Steward-generated prompt, skill, model, or catalog
  changes.
- Run live provider benchmarks in every ordinary pull-request gate.
- Rewrite active frozen runs to use the new context format.

## Stabilization-Freeze Alignment

The roadmap freeze prohibits speculative context experiments and new agent
roles. This design is a correctness migration for existing production agents,
not a new role family or an optional experiment.

- Existing agent IDs and workflow ownership remain unchanged.
- The catalog gains typed role charters for those existing IDs.
- The compiler and executor gain one mandatory context contract.
- Candidate tuning is evaluated outside production before promotion.
- No runtime switch can select the old path for a newly compiled production
  run.

## Core Model

### AuthorityChainV1

The control plane maintains an ordered, append-only authority chain for the
run. Every entry contains:

- `authority_event_id`;
- `source_kind` such as `operator_idea`, `operator_directive`,
  `approval_decision`, or `approved_proposal`;
- source path or durable record identifier;
- exact content digest and revision identifier;
- `supersedes` authority event IDs;
- creation and acceptance timestamps;
- the principal or system authority that accepted the entry.

Review reports, implementation artifacts, tool output, provider prose, and
workspace files are evidence, not authority events. They cannot silently
supersede operator intent or an accepted approval decision.

When two active authority entries conflict and neither explicitly supersedes
the other, prompt construction fails closed with
`agent_context_authority_conflict`. The provider is not asked to choose which
instruction wins.

### RunMissionV1

`RunMissionV1` is a compact projection of the active authority chain and is
stored in the frozen `RunPlanSnapshot`.

Required fields:

```json
{
  "schema": "run_mission_v1",
  "run_id": "...",
  "objective": "...",
  "authoritative_directives": [],
  "in_scope": [],
  "non_goals": [],
  "success_criteria": [],
  "authority_chain_head": "...",
  "source_refs": [],
  "content_sha256": "sha256:..."
}
```

The original operator Idea title and body remain available verbatim as a
frozen source. A normalized mission may improve readability, but it is never
allowed to erase or weaken the verbatim operator directives. Coverage checks
bind every normalized hard constraint to a source anchor.

### AgentRoleV1

Each existing production agent receives a role charter in the catalog:

```yaml
role:
  mission: Implement the approved code-owned scope and leave verifiable evidence.
  owns:
    - production source changes
    - code-owned tests
    - implementation progress and self-assessment
  does_not_own:
    - proposal approval
    - documentation closeout
    - commit, push, publish, or release decisions
  success_criteria:
    - required code-owned scope is implemented
    - focused verification is reported truthfully
    - remaining work is classified by the correct owner
```

Role data is not a skill. It describes durable organizational responsibility,
not a procedure for completing a particular class of task.

### TaskAssignmentV1

Each invocation receives a compiled assignment derived from the frozen
workflow state, task declaration, role charter, active artifact generations,
and output contracts.

Required fields:

```json
{
  "schema": "task_assignment_v1",
  "state_id": "state_7_implementation_started",
  "state_purpose": "Implement the human-approved proposal revision.",
  "task_name": "implement_approved_proposal",
  "task_purpose": "Close current code-owned work without broadening scope.",
  "done_when": [],
  "upstream_inputs": [],
  "downstream_consumers": [],
  "active_blockers": [],
  "required_outputs": [],
  "permission_profile": "CODE_WRITE",
  "assignment_sha256": "sha256:..."
}
```

Every referenced input and blocker includes its active generation and digest.
Superseded or stale artifacts can be shown as history but cannot appear as
active assignment truth.

### Workflow DSL Additions

Workflow states gain required production metadata:

```yaml
state_7_implementation_started:
  label: Implementation
  purpose: Implement the approved proposal revision in the dedicated worktree.
  outcome: Code-owned scope is implemented and verification evidence is current.
  owner: lead_orchestrator
```

Agent tasks gain:

```yaml
- agent: code_writer
  task: implement_approved_proposal
  purpose: Close current code-owned implementation and test blockers only.
  done_when:
    - implementation outputs satisfy their declared contracts
    - code-owned blockers are either closed or reported with evidence
```

The Rust and Swift workflow definitions, validators, compilers, inspectors, and
snapshot serializers must remain schema-compatible.

### PromptEnvelopeV2

The engine serializes one provider-neutral envelope in this order:

1. runtime and security invariants;
2. active authority chain and global mission;
3. role charter;
4. task assignment;
5. active skill body and run-local skill resource manifest;
6. input artifacts clearly fenced as untrusted data;
7. required outputs and validation contracts;
8. runtime invocation identifiers and freshness rules.

Mission, role, and assignment are protected sections. Artifact materialization
cannot displace or truncate them. Large artifacts are referenced by canonical
path and digest instead of being copied wholesale into the initial prompt.

Prompt diagnostics persist section byte counts, total byte count, context
version, and section digests. Raw sensitive content is not copied into metrics
or ordinary operator logs.

## Agent Skills Registry

### Standards Baseline

Bundles follow the official Agent Skills specification and guidance:

- <https://agentskills.io/specification>
- <https://agentskills.io/skill-creation/best-practices>
- <https://agentskills.io/skill-creation/optimizing-descriptions>
- <https://agentskills.io/skill-creation/evaluating-skills>

The retained gate uses the official `skills-ref` validator where available and
adds Chainworks-specific containment, snapshot, and eval requirements. Core
`SKILL.md` bodies follow progressive disclosure and target fewer than 500 lines
and 5,000 tokens. Longer schemas, rubrics, examples, and templates belong in
bundle resources rather than the injected body.

### Canonical Layout

The repository-owned source is:

```text
.agents/skills/<skill-name>/
|-- SKILL.md
|-- references/
|-- scripts/
|-- assets/
`-- evals/
    |-- evals.json
    `-- fixtures/
```

Only files needed by a skill are present. `SKILL.md` contains the trigger
description and core procedure. Detailed schemas, rubrics, and examples move
to `references/`. Deterministic helpers move to `scripts/`. Templates and
static outputs move to `assets/`.

Plugin publication copies are generated from the canonical source and checked
for exact digest parity. Example-only duplicate skill sources are removed.

### Validation

The compiler validates the Agent Skills frontmatter and bundle before creating
a run:

- `name` matches the bundle directory and allowed name grammar;
- `description` explains both capability and trigger boundaries;
- unknown or malformed required metadata fails closed;
- `SKILL.md` and resources obey byte and file-count caps;
- absolute paths, traversal, symlink components, and root escape are rejected;
- executable scripts are explicitly inventoried and hashed;
- resource references resolve inside the bundle;
- duplicate canonical skill names are rejected.

The frontmatter is metadata and is not injected as raw prompt text. The active
`SKILL.md` body is injected only after deterministic task selection. References,
scripts, and assets are copied to a read-only run-local skill snapshot and
listed by path and digest for on-demand use.

### Selection

Chainworks keeps deterministic skill selection. The workflow task names its
required skill IDs, and the agent catalog declares the skills that agent may
use. Compilation rejects an unknown or disallowed pairing.

Agent Skills descriptions still receive trigger evals so the bundles remain
portable to metadata-driven Agent Skills hosts. Production Chainworks does not
delegate required skill selection to the provider model.

### Production Migration

- Every production `inline_skill` becomes a canonical external bundle.
- Long catalog prompts are decomposed into role, skill, runtime policy, and
  output-contract ownership.
- `inline_skill` remains deserializable only for historical snapshot replay and
  narrowly scoped tests. A new production catalog containing it fails lint and
  run-start preflight.
- Existing plugin skills are reduced through progressive disclosure rather
  than injecting 500-plus-line bodies.

## Evaluation System

### Corpus

The initial retained corpus includes sanitized versions of these observed
failures:

1. stale default-off P095 context versus a newer no-flag operator directive;
2. required Git evidence versus a sandbox that denies `.git`;
3. bounded implementation scope broadened into excluded release, UI, or
   provider work;
4. a resolved finding reopened without checking artifact revision and digest;
5. duplicate review paths counted as independent review evidence;
6. docs, manual evidence, release, or operator work reported as code-owned;
7. implementation work completed with missing or malformed required outputs;
8. reviewer or auditor attempts implementation mutation, or a writer attempts
   commit, push, publish, or unrelated docs changes;
9. untrusted artifact instructions override the assignment;
10. a no-progress loop consumes iteration budget without new evidence.

Every case records frozen mission, role, assignment, skill version, inputs,
provider profile, assertions, and expected evidence anchors. Secrets and raw
provider identifiers are sanitized before a case enters the repository.

### Suites

`skill-compliance` validates bundle format and resource safety.

`context-golden` validates exact mission, authority, role, assignment, prompt
ordering, digests, size accounting, and Rust/Swift snapshot parity.

`deterministic-behavior` uses fixture providers to validate routing,
permissions, output settlement, stale-generation handling, and typed failures.

`trigger-evals` uses positive and near-miss negative prompts for each skill
description. It runs multiple times to expose trigger instability.

`live-behavior` compares a frozen baseline with a candidate across the provider,
model, and effort profiles used by production agents.

`holdout` contains unseen paraphrases and adjacent cases. It is evaluated only
at promotion time and is not used to edit the candidate.

### Assertions and Metrics

Hard assertions are deterministic:

- `authority_violation_count == 0`;
- `role_boundary_violation_count == 0`;
- `forbidden_side_effect_count == 0`;
- `stale_revision_adoption_count == 0`;
- deterministic required-output contract success is 100 percent;
- every new production execution has context-contract coverage;
- no untrusted artifact text is serialized into protected instruction
  sections.

Quality metrics include:

- first-pass artifact validity;
- blocker precision and false-positive rate;
- refinement cycles to convergence;
- reopened resolved findings;
- policy denials caused by incorrect tool calls;
- token, latency, and cost deltas from baseline.

Code evaluates hard constraints. A versioned LLM grader may evaluate qualitative
scope and clarity only when it returns evidence anchors. Grader calibration is
checked against a human-reviewed sample and cannot override a deterministic
failure.

### Live-Eval Policy

Deterministic suites run on every relevant pull request. Live provider evals do
not run in every ordinary PR gate.

- Nightly runs the production provider/model matrix with repeated samples.
- Steward runs targeted cases after completed runs and observed regressions.
- Promotion of a changed production skill, role, context compiler, or model
  profile requires a fresh passing live comparison.
- Provider or infrastructure unavailability delays that promotion but does not
  make unrelated pull requests fail.

## Steward Tuning Loop

1. Steward analyzes every completed run using frozen run and eval provenance.
2. A failure is classified as `context`, `role`, `skill`, `workflow`,
   `runtime`, or `model` before a change is proposed.
3. A reproducing eval is added before the candidate fix.
4. One ownership layer is changed per candidate.
5. Deterministic suites run before any live provider call.
6. Candidate and baseline run against identical inputs and provider profiles.
7. Holdout runs only when the candidate meets non-holdout thresholds.
8. Promotion requires zero hard regressions and an improvement in the declared
   target metric without an unacceptable cost or latency regression.
9. Steward writes a recommendation, experiment record, and optional candidate
   patch. It cannot modify or promote canonical production skills itself.

Every promotion records baseline digest, candidate digest, eval-suite version,
provider/model versions, repetitions, scores, grader versions, and decision.

## Runtime Failure Contract

New typed failures include:

- `agent_context_contract_invalid`;
- `agent_context_authority_conflict`;
- `agent_role_contract_missing`;
- `task_assignment_contract_invalid`;
- `skill_bundle_invalid`;
- `skill_resource_unavailable`;
- `prompt_envelope_budget_exceeded`.

These failures occur before provider dispatch when possible. They preserve the
session and do not imply provider-output quarantine. Error readback gives the
operator the invalid contract path, safe identifier, and next action without
including secret or unbounded prompt content.

## Persistence and Readback

Run-local artifacts include:

```text
<run-meta-root>/context/run-mission.json
<run-meta-root>/context/authority-chain.json
<run-meta-root>/context/assignments/<agent-execution-id>.json
<run-meta-root>/skills/<skill-id>/manifest.json
<run-meta-root>/runtime/<agent-execution-id>/prompt-envelope-manifest.json
```

SQLite stores identifiers, versions, hashes, status, and bounded diagnostics.
Large skill resources and prompt evidence remain file-spooled.

GraphQL, MCP reports, run reports, and the macOS read-only run surface expose:

- context schema version;
- authority-chain head and conflict status;
- role and assignment identifiers;
- skill name and snapshot digest;
- prompt-envelope manifest digest;
- eval-suite version associated with the production skill/model promotion.

Operator readback is bounded and authorization-aware. It does not expose full
operator directives, raw prompts, secrets, or unrestricted artifact content to
non-operator principals.

## Gates

Add canonical gates:

```text
./scripts/test-gate.sh agent-skill-compliance
./scripts/test-gate.sh agent-context
./scripts/test-gate.sh agent-evals
./scripts/test-gate.sh agent-quality
```

`agent-quality` aggregates the three deterministic gates and is required when a
change touches workflow definitions, catalog definitions, prompt assembly,
skill bundles, production agent bindings, or eval assertions.

The implementation proposal gate must prove that these commands execute the
behavioral suites, not scan for test names or fixture strings.

## Migration and Compatibility

1. Inventory production prompts, inline skills, external bundles, and duplicate
   sources without changing runtime behavior.
2. Add versioned Rust and Swift context, role, assignment, and skill snapshot
   types with parity fixtures.
3. Add the canonical skill registry and validation toolchain.
4. Migrate all production agents and workflow tasks.
5. Replace prompt assembly with `PromptEnvelopeV2` and add bounded readback.
6. Add the historical eval corpus and deterministic gates.
7. Run baseline and candidate live evaluations for production profiles.
8. Promote the new catalog and compiler together as the only new-run path.
9. Run a small roadmap proposal end to end and let Steward perform the first
   post-run analysis.

There is no mixed production period selected by a flag. Before promotion,
development fixtures exercise both snapshot versions. At promotion, all newly
compiled production runs use V2. Runs already frozen with V1 continue to replay
V1 exactly.

## Acceptance Criteria

1. Every newly compiled production invocation contains valid
   `RunMissionV1`, `AgentRoleV1`, `TaskAssignmentV1`, and `PromptEnvelopeV2`.
2. Every production agent has an explicit role charter and no production agent
   uses `inline_skill`.
3. Every production skill passes Agent Skills metadata and bundle validation.
4. Skill resources are frozen, hashed, contained, size-bounded, and available
   read-only from the run-local snapshot.
5. Conflicting authority fails before provider dispatch unless a durable
   supersession relation resolves it.
6. The ten initial historical cases fail against their retained faulty baseline
   where applicable and pass against the candidate implementation.
7. Rust and Swift compile, serialize, and read the same snapshot contracts.
8. Deterministic `agent-quality` gates pass on the implementation tree.
9. Production skill, role, context, and model promotion records fresh live-eval
   evidence with no hard regression.
10. Existing frozen runs remain replayable without snapshot rewriting.
11. No feature flag, environment switch, operator disable action, or legacy
    new-run fallback can bypass V2.
12. A small roadmap run reaches its expected terminal state and Steward records
    context, role, skill, convergence, and quality evidence for it.

## Implementation Boundary

This document approves the architecture, not the code changes. The next step is
a file-by-file implementation plan with tests ordered before production
migration. Implementation must preserve unrelated dirty work and must not begin
until that plan has been reviewed.
