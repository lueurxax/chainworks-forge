# Security and Pre-Push Review Skills: Default-On Slice

Date: 2026-08-29
Status: Draft for proposal-readiness review; implementation prohibited until Ready

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

1. stay within the frozen proposal, mission, changed-files evidence, and test
   evidence;
2. inspect authentication, authorization, secrets, unsafe defaults, injection,
   serialization, filesystem and symlink boundaries, network boundaries, data
   leakage, and dependency risk when implicated by the change;
3. use read-only scanner results as evidence rather than as a substitute for
   reasoning;
4. use `changed_files_manifest` as canonical Git evidence and never invoke
   `git status`, `git diff`, `git rev-parse`, or read `.git`;
5. keep discovery bounded to changed and implicated paths;
6. return `pass` only when no blocking security issue remains and required
   evidence is sufficient;
7. emit exactly one `security_report_v1` and perform no mutation beyond that
   declared report output; source, proposal, release, and external effects are
   forbidden.

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
3. consume the canonical changed-files, test, implementation-audit, and security
   evidence declared by the assignment;
4. use `changed_files_manifest` as canonical Git evidence and never invoke
   `git status`, `git diff`, `git rev-parse`, or read `.git`;
5. keep discovery bounded to changed and implicated paths;
6. return `block` when required evidence is missing, invalid, red, or contains
   an unresolved blocking finding; never reinterpret a blocking security or
   audit result as pass;
7. emit exactly one `prepush_review_v1` and perform no mutation beyond that
   declared report output; source edits, commits, pushes, approvals, releases,
   and external effects are forbidden.

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
8. a corrupted stored bundle or prompt hash fails closed before provider work.

Existing runs are not drift-upgraded. Retrying an existing run cannot pick up
these bundles; the end-to-end observation therefore requires a new run.

## Deterministic Cases

### `CTX-007`: Security Evidence and Authority

The fixture represents a security review of an implementation that changes an
authorization or filesystem boundary.

Positive assertions prove that the finalized prompt contains:

- the frozen operator objective and security assignment;
- `RO_VERIFY` and `security_report_v1` truth;
- canonical `changed_files_manifest` guidance;
- scanner-as-evidence guidance;
- bounded discovery and no direct Git access;
- no source-write, approval, release, or external-effect authority;
- the correct downstream consumer.

Mutation negatives independently remove or alter mission, permission, output,
canonical evidence, no-mutation, and consumer truth. Every mutation must fail
the deterministic scorer.

### `CTX-008`: Pre-Push Fail-Closed Settlement

The fixture represents final review with declared test, audit, and security
evidence, including one blocking or invalid upstream condition.

Positive assertions prove that the finalized prompt contains:

- the frozen operator objective and pre-push assignment;
- `RO_PREPUSH_VERIFY` and `prepush_review_v1` truth;
- all declared upstream evidence inputs;
- explicit fail-closed handling of missing, invalid, red, or blocking evidence;
- canonical changed-files guidance and bounded discovery;
- no edit, commit, push, approval, release, or external-effect authority;
- the correct downstream consumer.

Mutation negatives independently weaken permission, output, evidence set,
fail-closed policy, no-release authority, and consumer truth. Every mutation
must fail the deterministic scorer.

The cases score prompt contract truth, not model intelligence. They make no
claim that a provider will always obey the procedure.

## Required Tests and Gate

Extend `./scripts/test-gate.sh agent-context-skills` without adding providers,
daemon startup, Xcode, network access, or remote execution.

The gate must execute:

1. catalog parity for all five migrated external bindings;
2. strict bundle validation for both new directories;
3. exact permission, tool, input, output, approval, and write-policy parity;
4. prompt ordering and exact-once procedure injection for both agents;
5. frontmatter exclusion and no duplicated catalog procedure prose;
6. frozen-snapshot reuse after both live bundle directories are changed or
   removed;
7. corrupted bundle/hash fail-closed behavior with zero provider work;
8. exact `CTX-001..008` corpus membership;
9. positive and independent mutation-negative scoring for `CTX-007/008`;
10. unchanged procedure bytes for every skill outside the five migrated
    bindings.

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
- Prompt-finalization or frozen-hash failure creates zero provider work.
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
5. Existing frozen runs preserve exact stored behavior.
6. New runs fail closed on invalid source or stored bundle state.
7. `CTX-001..008` and all declared mutation negatives pass.
8. The provider-free focused gate passes locally without remote execution.
9. No unrelated skill, runtime, API, persistence, UI, or workflow surface is
   changed.
10. After implementation merge, one new normal run exercises both procedures
    with no disable flag or special workflow.

## Review Guidance

Reviewers should block only for buildability, authority drift, incomplete
fail-closed behavior, frozen-run incompatibility, procedure duplication, or
insufficient deterministic proof. The lack of statistical provider evaluation
is an accepted limitation of this slice.

Implementation may begin only after proposal-readiness review returns `Ready`
with no blocking finding for this scope.
