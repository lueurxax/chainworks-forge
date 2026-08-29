# Security and Pre-Push Review Skills: Default-On Slice

Date: 2026-08-29
Status: Revised after proposal-readiness review; implementation prohibited until Ready

## Summary

Chainworks already injects a frozen `AgentMissionContextV1` and resolves three
production procedures from local Agent Skills-compatible bundles. The security
and pre-push reviewers still keep their reusable procedure inside catalog
prompts while their `skill_ref` values resolve to one-line `inline_skill`
descriptions.

This slice converts exactly two additional read-only review procedures:

1. `security_checker_core` becomes the external bundle `security-review`;
2. `prepush_review_core` becomes the external bundle `prepush-review`.

The change is mandatory for newly compiled runs. Existing frozen runs keep
their stored catalog and prompt bytes. No feature flag, fallback path, cohort,
or disable switch is added.

Two provider-free cases, `CTX-007` and `CTX-008`, extend the existing mission
context gate. After implementation is merged and the local runtime is refreshed,
one new Chainworks run will provide the first live end-to-end observation.

## Scope Boundary

In scope:

- two single-file Agent Skills bundles;
- two catalog binding changes from `inline_skill` to `external_skill`;
- removal of procedure duplication from the two agent prompts;
- parity, prompt-order, frozen-snapshot, and mutation-negative tests;
- `CTX-007` and `CTX-008` in the existing deterministic corpus;
- extension of `./scripts/test-gate.sh agent-context-skills`;
- one new-run observation after implementation.

Out of scope:

- any other skill ID or agent prompt;
- permission, tool, model, effort, input, output, or output-contract changes;
- workflow topology or transition changes;
- database, GraphQL, MCP, ACP, SwiftUI, or daemon protocol changes;
- scanner implementation or new scanner tools;
- live-provider benchmarks, repeated A/B evaluation, or nightly evaluation;
- skill resources, scripts, assets, registries, installers, or `allowed-tools`;
- a new authenticated persisted-prompt digest or a copy-prompt-to-skill-hash
  runtime validator;
- production-hardening backlog refinement;
- rewriting or recompiling an existing run.

## Current Contract

The active catalog currently binds:

| Agent | Skill | Permission | Output contract |
|---|---|---|---|
| `security_checker` | `security_checker_core` inline | `RO_VERIFY` | `security_report_v1` |
| `prepush_code_reviewer` | `prepush_review_core` inline | `RO_PREPUSH_VERIFY` | `prepush_review_v1` |

`security_report_v1` settles to `pass`, `block`, `invalid`, or `unknown`.
`prepush_review_v1` settles to the same canonical status set. A controlled
implementation closeout is green only when both contracts settle to `pass`.

In `full-mvp-live.yaml`, security review runs in the parallel portion of
`state_9_implementation_reviewed`; pre-push review runs later in the same state
after audit evidence exists. This provides a natural end-to-end observation
without a special workflow.

The compiled production task, rather than the broader catalog agent entry,
defines materialized evidence. In this workflow the security task receives
`approved_proposal` and `changed_files_manifest`. The pre-push task receives
`approved_proposal`, `changed_files_manifest`, `audit_report`, and
`security_report`. It does not directly receive `tests_result`; test truth is
available only through the implementation audit, whose immediately preceding
task does receive `tests_result`. This slice does not change that topology.

## Decision

Use the existing strict external-skill compiler and prompt finalizer unchanged.
This is catalog, procedure, fixture, and gate work. No new runtime abstraction
is introduced.

The two definitions become:

```yaml
security_checker_core:
  type: external_skill
  path: skills/security-review

prepush_review_core:
  type: external_skill
  path: skills/prepush-review
```

Each directory contains exactly one regular `SKILL.md`. Existing bundle rules
remain authoritative: descriptor-relative no-follow reads, 65,536-byte maximum,
500-line body maximum, validated frontmatter, no auxiliary entries, and no
`allowed-tools`.

## Security Review Procedure

Path: `examples/agents/skills/security-review/SKILL.md`.

Required frontmatter:

```yaml
name: security-review
description: Use when reviewing a Chainworks implementation for security and privacy release blockers.
compatibility: Chainworks Forge security review stages with frozen mission, evidence, permission, and output contracts.
```

The Markdown body must instruct the reviewer to:

1. stay within the frozen proposal, mission, and only the evidence declared by
   the compiled task; inspect test evidence directly only when `tests_result`
   is declared, and otherwise never invent or fetch that missing input;
2. inspect authentication, authorization, secrets, unsafe defaults, injection,
   serialization, filesystem and symlink boundaries, network boundaries, data
   leakage, and dependency risk when implicated by the change;
3. use read-only scanner results as evidence rather than as a substitute for
   reasoning;
4. accept only the declared control-plane-generated `changed_files_manifest` as
   canonical Git evidence and never invoke `git status`, `git diff`,
   `git rev-parse`, read `.git`, or substitute another manifest;
5. keep discovery bounded to changed and implicated paths;
6. return `pass` only when no blocking security issue remains and required
   evidence is sufficient;
7. publish only the logical output `security_report` under
   `security_report_v1` and perform no mutation beyond that declared report;
   source, proposal, release, and external effects are forbidden.

The catalog prompt retains only role specialization: apply the frozen procedure
to the current implementation and output the declared security contract. It
must not repeat the procedure body.

## Pre-Push Review Procedure

Path: `examples/agents/skills/prepush-review/SKILL.md`.

Required frontmatter:

```yaml
name: prepush-review
description: Use for the final Chainworks code-quality review before an approved implementation may proceed to release Git actions.
compatibility: Chainworks Forge pre-push review stages with frozen audit, security, test, and output contracts.
```

The Markdown body must instruct the reviewer to:

1. treat the approved proposal and frozen mission as scope;
2. evaluate correctness, maintainability, regression risk, surprising side
   effects, and missing tests without adding unrelated improvements;
3. consume exactly the canonical changed-files, implementation-audit, and
   security evidence declared by the compiled task; assess test truth directly
   only when `tests_result` is declared by that task, and otherwise use the
   implementation audit without inventing a missing direct input;
4. accept only the declared control-plane-generated `changed_files_manifest` as
   canonical Git evidence and never invoke `git status`, `git diff`,
   `git rev-parse`, read `.git`, or substitute another manifest;
5. keep discovery bounded to changed and implicated paths;
6. return `block` when required evidence is missing, invalid, red, or contains
   an unresolved blocking finding; never reinterpret a blocking security or
   audit result as pass;
7. publish only the logical output `prepush_review_report` under
   `prepush_review_v1` and perform no mutation beyond that declared report;
   source edits, commits, pushes, approvals, releases, and external effects are
   forbidden.

The catalog prompt retains only role specialization: perform the final review
using the frozen procedure and declared evidence, then output the contract. It
must not repeat the procedure body.

## Preserved Authority

The migration changes procedure source only. Tests must prove exact preservation
of these bindings:

| Field | `security_checker` | `prepush_code_reviewer` |
|---|---|---|
| backend profile | `claude_security_high` | `claude_prepush_medium` |
| permission profile | `RO_VERIFY` | `RO_PREPUSH_VERIFY` |
| output | `security_report` | `prepush_review_report` |
| output contract | `security_report_v1` | `prepush_review_v1` |
| human approval | `false` | `false` |
| worktree write | disabled/absent | disabled/absent |

The skill body grants no tools, permissions, filesystem roots, output ownership,
or transition authority. Existing `required_tools`, inputs, and workflow task
bindings remain byte-for-byte unchanged.

## Prompt and Snapshot Contract

For each affected agent in a newly compiled run:

1. the validated Markdown body is frozen in catalog snapshot V2;
2. the final procedure participates in the existing `skill_snapshot_hash`;
3. prompt order remains system instructions, mission context, skill procedure,
   then task body and materialized evidence;
4. the procedure appears exactly once;
5. frontmatter never enters the provider prompt;
6. retry and resume reuse the frozen prompt bytes;
7. changed or removed live bundle files do not alter a frozen run;
8. corrupted stored catalog bytes, embedded bundle bytes, or their existing
   catalog/bundle hashes fail closed before provider work.

Existing runs are not drift-upgraded. Retrying an existing run cannot pick up
these bundles; the end-to-end observation therefore requires a new run.

This slice does not claim that the current copy validator authenticates an
arbitrary persisted prompt against `skill_snapshot_hash`. It continues to
validate the V1 mission-block shape and reuse stored bytes under the existing
runtime contract. Adding a new persisted-prompt digest is separate runtime work.

A checked-in pre-migration V2 snapshot fixture must preserve the current inline
definitions for these two skills. After live catalog migration and removal of
the new source directories, compilation from that fixture must reproduce exact
golden security and pre-push prompt bytes without consulting live files.

## Deterministic Cases

### `CTX-007`: Security Evidence and Authority

The fixture represents a security review of an implementation that changes an
authorization or filesystem boundary.

The test compiles the real `check_implementation_security` task from the active
`full-mvp-live.yaml` and `agents.yaml`; it does not construct a synthetic task
with additional inputs. Positive assertions prove that the finalized prompt
contains:

- the frozen operator objective and security assignment;
- `RO_VERIFY` and `security_report_v1` truth;
- exactly `approved_proposal` and `changed_files_manifest` as production inputs,
  with no direct `tests_result`;
- control-plane-generated provenance for canonical `changed_files_manifest`;
- scanner-as-evidence guidance;
- bounded discovery and no direct Git access;
- no source-write, approval, release, or external-effect authority;
- the next-execution-phase consumer
  `audit_implementation_against_proposal/proposal_implementation_auditor`.

Mutation negatives independently remove or alter mission, permission, output,
canonical evidence, no-mutation, and consumer truth. Every mutation must fail
the deterministic scorer.

### `CTX-008`: Pre-Push Fail-Closed Settlement

The fixture represents final review with the exact compiled production inputs:
`approved_proposal`, `changed_files_manifest`, `audit_report`, and
`security_report`. Its deterministic task body contains one blocking or invalid
upstream audit/security condition. It does not inject a direct `tests_result`.
The procedure must not invent direct test evidence; any test assessment is
transitive through `audit_report` in this workflow.

The test compiles the real `prepush_review` task from active
`full-mvp-live.yaml` and `agents.yaml`. Positive assertions prove that the
finalized prompt contains:

- the frozen operator objective and pre-push assignment;
- `RO_PREPUSH_VERIFY` and `prepush_review_v1` truth;
- exactly the four production evidence inputs and no direct `tests_result`;
- explicit fail-closed handling of missing, invalid, red, or blocking evidence;
- control-plane-generated provenance for canonical changed-files evidence and
  bounded discovery;
- no edit, commit, push, approval, release, or external-effect authority;
- the next-execution-phase consumer
  `aggregate_implementation_reviews/lead_orchestrator`.

Mutation negatives independently weaken permission, output, exact evidence set,
fail-closed policy, no-release authority, and next-phase consumer truth. Every
mutation must fail the deterministic scorer.

The cases score prompt contract truth, not model intelligence. They make no
claim that a provider will always obey the procedure.

### Mutation Harness V2

`CTX-001..006` retain their current JSON-pointer mutations. `CTX-007/008` use a
closed mutation union that regenerates the final prompt for every case:

- `mission_json_replace`: replace one mission-context JSON pointer;
- `system_prompt_remove`: remove one named system-prompt clause before finalization;
- `procedure_remove`: remove one named clause from the resolved frozen procedure;
- `task_input_remove`: remove one declared compiled-task input and regenerate
  the deterministic materialized task body;
- `task_input_add`: inject one undeclared input and regenerate the task body;
- `task_body_remove`: remove one named evidence or policy clause from the task body.

Each expected claim has a stable `claim_id` and exactly one targeted negative
mutation. The scorer returns the complete satisfied-claim set. The baseline
must equal the fixture's exact claim set; each mutation must remove its named
claim and no unrelated claim. Deleting or weakening a scorer rule therefore
also breaks the positive exact-set assertion rather than silently making the
mutation pass.

`CTX-007` exact-sets `approved_proposal` and `changed_files_manifest`, proves
that direct `tests_result` is absent, and includes `task_input_add` for an
undeclared `tests_result`. The corresponding `no_undeclared_test_evidence`
claim must fail. The parity fixture separately preserves direct `tests_result`
for the security task in `workflow.yaml`, where it is genuinely declared.

Both cases have an independent `control_plane_manifest_provenance` claim.
Removing `control-plane-generated` or changing the procedure to permit a
caller/provider-authored alternative manifest must fail that claim.

### Conditional Evidence Compatibility Matrix

The named test
`active_review_tasks_cover_conditional_test_evidence_branches` compiles both
checked-in workflows with the active catalog and finalizes all four real tasks:

| Workflow | Task | Exact inputs | Logical output | Contract | Direct test branch |
|---|---|---|---|---|---|
| `full-mvp-live.yaml` | `check_implementation_security` | `approved_proposal`, `changed_files_manifest` | `security_report` | `security_report_v1` | absent; adding `tests_result` is rejected |
| `full-mvp-live.yaml` | `prepush_review` | `approved_proposal`, `changed_files_manifest`, `audit_report`, `security_report` | `prepush_review_report` | `prepush_review_v1` | absent; adding `tests_result` is rejected |
| `workflow.yaml` | `review_security` | `approved_proposal`, `implementation_progress`, `changed_files_manifest`, `tests_result` | `security_report` | `security_report_v1` | present and accepted |
| `workflow.yaml` | `review_before_push` | `approved_proposal`, `implementation_progress`, `changed_files_manifest`, `tests_result`, `audit_report`, `security_report` | `prepush_review_report` | `prepush_review_v1` | present and accepted |

For the two direct-test tasks, `task_input_remove(tests_result)` regenerates the
prompt, removes exactly the `declared_test_evidence_available` claim, and must
fail the scorer. For the two no-direct-test tasks, the existing
`task_input_add(tests_result)` mutation removes exactly
`no_undeclared_test_evidence` and must fail. Every finalized prompt separately
asserts the logical output name and output contract listed above.

Consumer assertions use the current `task_consumers` meaning: the task or tasks
in the next execution phase, or state transitions when no later phase exists.
They do not claim to enumerate every artifact reader.

## Required Tests and Gate

Extend `./scripts/test-gate.sh agent-context-skills` without adding providers,
daemon startup, Xcode, network access, or remote execution.

The gate must execute:

1. catalog parity for all five migrated external bindings;
2. strict bundle validation for both new directories;
3. complete before-state parity for the two affected catalog agent entries,
   their full referenced backend and permission profiles, and every task object
   using those agents in `full-mvp-live.yaml` and `workflow.yaml`;
4. prompt ordering and exact-once procedure injection for both agents;
5. frontmatter exclusion and no duplicated catalog procedure prose;
6. post-migration frozen-snapshot reuse after both live bundle directories are
   changed or removed;
7. exact pre-migration V2 inline snapshot and golden-prompt reuse after the live
   catalog and new bundle directories are unavailable;
8. corrupted catalog/bundle bytes or existing hashes fail closed with zero
   provider work;
9. exact `CTX-001..008` corpus membership;
10. active-workflow compilation plus positive and independent V2 mutation
    scoring for `CTX-007/008`;
11. unchanged skill definitions and procedure bytes for every skill outside
    the two newly migrated bindings, including byte-pinned SHA-256 values for
    `proposal-review-router/SKILL.md`, `code-implementation/SKILL.md`, and
    `implementation-audit/SKILL.md`;
12. `active_review_tasks_cover_conditional_test_evidence_branches` finalizes
    the four-task compatibility matrix, proves both conditional branches, and
    independently mutates direct-test presence for every task.

The before-state fixture stores canonical JSON for the two inline skill
definitions, both complete agent entries, both referenced backend profiles,
both referenced permission profiles, and the four workflow task objects that
use the agents. One deterministic transform may change only the two skill
definitions and replace each long prompt with its specified concise
specialization. Exact comparison rejects all other drift.
Mutation tests cover backend profile, model/effort/MCP, permission rules,
required tools, task inputs, outputs, output contract, approval requirement,
worktree policy, phase/parallel placement, and workflow task identity.

The fixture also stores exact full-file SHA-256 values for the three existing
external bundles. A one-byte mutation in each file is an independent negative
case and must fail before catalog compilation is accepted. The only permitted
bundle-byte additions or changes are the two new directories named by this
slice.

The proof manifest maps each requirement above to named executable tests. Gate
preflight fails when a named test is absent or renamed, and the tests themselves
must execute rather than satisfy string-only presence checks.
The compatibility-matrix test is a required proof entry and cannot be replaced
by YAML source comparison alone.

The existing gate name remains unchanged because this extends the same contract
surface. Documentation must update its case count and active-bundle inventory.

## End-to-End Observation

After the implementation is reviewed, merged into local `main`, and the local
app/daemon is refreshed, create one new Chainworks run from the normal
`full-mvp-live.yaml` workflow. Do not retry an older run and do not introduce a
run-specific flag or catalog.

Readback must prove:

1. the run snapshot contains external `security-review` and `prepush-review`
   bundles with stable hashes;
2. the state-9 security and pre-push executions use those frozen procedures;
3. both outputs settle through their existing canonical contracts;
4. a blocking finding, if any, follows the existing refinement transition
   rather than being bypassed;
5. absence of a blocking finding allows the existing workflow to continue.

The run is observational evidence, not a prerequisite for claiming the code
slice implemented. Any defect found in it is fixed forward; the feature is not
disabled.

## Failure Policy

- Missing, malformed, oversized, escaping, or multi-entry bundle fails new-run
  compilation.
- Missing skill resolution never degrades to an inline or empty procedure.
- Prompt-finalization or existing catalog/bundle hash failure creates zero
  provider work.
- Security and pre-push evidence failure uses existing `block`, `invalid`, or
  `unknown` settlement; this slice creates no waiver.
- Existing frozen runs remain readable and retry only with their stored bytes.
- There is no fallback to the old inline definitions for a newly compiled run.

## Acceptance Criteria

1. Exactly the two named bindings become external bundles.
2. Exactly two new `SKILL.md` files are added, each single-file and free of
   `allowed-tools`.
3. Existing permission, model, tool, input, output, approval, workflow, and
   settlement contracts remain unchanged.
4. Reusable procedure is removed from both catalog prompts and injected exactly
   once from frozen bundle bytes.
5. Post-migration frozen runs reuse embedded bundle bytes; the pinned
   pre-migration V2 fixture preserves exact inline prompt bytes.
6. New runs fail closed on invalid source or stored bundle state.
7. `CTX-007/008` compile the real active state-9 tasks with their exact inputs,
   next-phase consumers, and no synthetic direct test evidence.
8. The V2 mutation harness regenerates prompts and makes every named claim
   independently falsifiable; `CTX-001..008` pass.
9. Complete before-state parity rejects drift in every affected agent, profile,
   permission, tool, task, output, approval, and write-policy field.
10. Both procedures require the declared control-plane-generated manifest and
    reject alternate provenance; `CTX-007` rejects undeclared direct test input.
11. All three previously migrated bundle files are byte-pinned and independently
    mutation-tested.
12. All four real review tasks are compiled and finalized; both direct-test
    conditional branches, exact input sets, logical outputs, and contracts are
    mutation-tested.
13. The provider-free focused gate passes locally without remote execution.
14. No unrelated skill, runtime, API, persistence, UI, or workflow surface is
   changed.
15. After implementation merge, one new normal run exercises both procedures
    with no disable flag or special workflow.

## Review R1 Resolution

| Finding | Resolution |
|---|---|
| P1-01 production evidence mismatch | `CTX-008` compiles active `prepush_review`, uses its exact four inputs, and treats test truth only as transitive through `audit_report` |
| P1-02 unsupported prompt integrity | authenticated persisted-prompt digest is explicitly out of scope; claims are limited to existing catalog and bundle digest enforcement |
| P1-03 non-falsifiable prompt mutations | closed Mutation Harness V2 regenerates final prompts and exact-sets stable claim IDs |
| P1-04 incomplete parity | canonical before-state covers complete agents, profiles, permissions, and workflow task objects with authority-field mutations |
| P2-01 consumer ambiguity | consumer means next execution phase; exact task/agent tuples are specified |
| P2-02 pre-migration proof | pinned V2 inline snapshot and exact golden prompts are required |
| P2-03 proof manifest | every new requirement maps to an executable named test and gate preflight checks the mapping |
| R2 P1-01 security input fidelity | security evidence is task-conditional; `CTX-007` rejects injected undeclared `tests_result`, while parity preserves the workflow that declares it |
| R2 P1-02 manifest provenance | both procedures and cases require the control-plane-generated manifest and reject alternate provenance |
| R2 P1-03 existing bundle drift | full SHA-256 values and one-byte mutations pin all three previously migrated bundles |
| R3 P1-01 positive conditional branch | named compatibility test compiles/finalizes all four real tasks; direct-test removal and undeclared-test addition independently fail their claims |

## Review Guidance

Reviewers should block only for buildability, authority drift, incomplete
fail-closed behavior, frozen-run incompatibility, procedure duplication, or
insufficient deterministic proof. The lack of statistical provider evaluation
is an accepted limitation of this slice.

Implementation may begin only after proposal-readiness review returns `Ready`
with no blocking finding for this scope.
