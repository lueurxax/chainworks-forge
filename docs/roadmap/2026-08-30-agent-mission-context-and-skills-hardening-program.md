# Agent Mission Context and Skills Hardening Program

Date: 2026-08-30
Status: Planned program; no direct implementation authority

## Purpose

This note turns the deferred agent mission-context, Agent Skills, and eval
inventory into an ordered set of bounded roadmap slices. It is not a proposal,
an acceptance contract, or permission to implement the preserved backlog as a
single change.

Each slice must first become a small proposal with its own reviewer-approved
contract, tests, rollout boundary, and closeout evidence. Proposal numbers are
assigned only when a slice is scheduled.

## Implemented baseline

The following behavior is already implemented and must not be rescheduled:

- mission context is mandatory for newly compiled Rust-owned runs;
- the compiler freezes the exact mission and strict external `SKILL.md` bytes;
- retry, copied invocation, P017 mediation, and P058 escalation paths validate
  persisted mission authority before mutation;
- `changed_files_manifest` is owned by the control plane, not provider agents;
- five production procedures are external bundles: proposal review,
  implementation audit, code implementation, security review, and pre-push
  review;
- the provider-free `agent-context-skills` gate proves the baseline.

Durable behavior is documented in
[Mission Context, Skill Resolution, and Runtime Integration](../reference/skill-resolution-and-runtime-integration.md).
The large
[production hardening backlog](../superpowers/specs/2026-08-27-agent-mission-context-skills-evals-production-hardening-backlog.md)
is retained only as a non-normative source inventory.

## Scheduling rules

- Keep P073 freeze constraints: do not add provider families or agent roles.
- Stabilize P083 ownership truth and P070 typed boundaries before introducing
  mutable authority overlays or side-effect-owning skill migrations.
- Keep deterministic, provider-free evals in required pull-request gates.
- Run repeated live Codex, Claude, and Gemini evals through Steward/nightly
  operation, not on every pull request.
- Do not spend provider budget on a dedicated validation run. Natural-run
  observation may supplement deterministic proof but cannot replace it.
- The target production behavior is default-on. A slice must not add a feature
  flag or disable switch unless a separate rollback contract is explicitly
  reviewed and approved.
- Every proposal must name its source-inventory rows and must not inherit
  undeclared requirements from the preserved backlog.

## Ordered slices

### Slice 1: Remaining low-authority procedure migration

Move bounded, non-side-effect procedures from inline or builtin definitions
into strict Agent Skills bundles. Start with `proposal_writer_core`,
`docs_quality_guardian`, and `steward_core` only after verifying that each
procedure's current owner and write boundary remain unchanged.

Required proof:

- one canonical bundle per migrated procedure;
- frozen snapshot and copy-validation parity with the existing five bundles;
- no duplicated procedure prose in provider prompts;
- missing, malformed, oversized, or changed bundles fail closed;
- deterministic catalog and prompt-composition tests.

Out of scope: `orchestrator_core`, `github_commit_push`, and
`connect_publisher`.

### Slice 2: Authority and assignment overlay

Add a typed, durable mechanism for accepted mid-run operator directives and
assignment supersession without mutating the frozen base mission.

Dependency: P083 ownership truth and the relevant P070 typed-boundary work must
be stable first.

Required proof:

- one authoritative append-only owner and monotonic head;
- authenticated admission using the live principal table;
- exact run, approval, stage, and invocation binding;
- copied invocation and retry validation against the selected overlay head;
- forged projections and stale heads create no provider work or side effect.

### Slice 3: Skill resource and script broker

Extend strict bundles beyond one `SKILL.md` through a control-plane-owned
resource/script broker. Provider agents receive only declared, frozen,
capability-checked resources and never discover arbitrary bundle files.

Dependency: typed authority and ownership boundaries from Slice 2.

Required proof:

- canonical manifest, digest, size, count, and path limits;
- symlink/race-safe loading and run-local snapshot provenance;
- per-resource and per-script permission admission;
- bounded output and cleanup behavior;
- provider sandbox parity tests without live-provider calls.

### Slice 4: Provider-free eval registry and candidate scoring

Turn known run failures into deterministic fixtures and compare candidate
context/skill changes against a frozen baseline before promotion.

Dependency: stable snapshot and broker contracts from the preceding slices.

Required proof:

- versioned case registry with retained positive and mutation-negative cases;
- deterministic scorer and baseline/candidate identity;
- separate correctness, authority, output-contract, and cost-budget metrics;
- fail-closed missing evidence and reproducible local/CI results;
- no live provider dependency in the required pull-request gate.

### Slice 5: Steward-owned live provider evals

Add scheduled repeated provider evals as an operational signal. Steward owns
collection and recommendations but cannot mutate or promote production
catalogs, skills, workflows, models, or budgets.

Dependency: Slice 4 scoring and provenance.

Required proof:

- nightly/on-demand cohorts for supported providers and models;
- bounded run, token, time, and storage budgets;
- retry/failure accounting that does not turn provider outages into candidate
  regressions;
- redacted readback and retained trend evidence;
- no required live-provider lane in ordinary pull requests.

### Slice 6: Promotion, readback, and rollout controls

Add operator-visible candidate comparison, explicit promotion authority,
rollback evidence, and long-run metrics only after deterministic and live eval
foundations exist.

Required proof:

- promotion is an authenticated, idempotent, auditable operation;
- frozen runs retain their original skill/context identity;
- rollback creates a new version rather than rewriting history;
- GraphQL/macOS remain read-only except already approved action boundaries;
- rollout metrics distinguish context, skill, provider, model, and budget
  changes.

## Separate high-authority proposals

The following procedures are not part of the low-authority migration slice:

- `orchestrator_core` owns workflow/control-plane commands;
- `github_commit_push` owns Git and remote delivery side effects;
- `connect_publisher` owns network publication side effects.

Each requires its own proposal after P083/P070 stabilization. Those proposals
must preserve durable effect identity, idempotency, permission admission,
ambiguous-result reconciliation, and no-blind-retry behavior. A generic skill
migration proposal cannot grant or broaden those authorities.

## Backlog mapping

| Preserved design area | Roadmap owner |
|---|---|
| Remaining catalog procedure migration | Slice 1 or a separate high-authority proposal |
| Mid-run authority, supersession, assignment context | Slice 2 |
| Bundle resources, scripts, broker, sandbox | Slice 3 |
| Known-case corpus, mutation cases, candidate scoring | Slice 4 |
| Repeated live provider measurements | Slice 5 |
| Promotion, operator readback, metrics, rollout | Slice 6 |

Anything in the preserved backlog that is not explicitly mapped here remains
unscheduled. It cannot be treated as an implicit requirement of a future
slice.

## Program completion

This program is complete only when every scheduled slice has either reached
`Implemented / Ready` and been promoted to reference truth, or has been
explicitly rejected and removed from the roadmap. Completion is not required
for the already implemented minimal mission-context flow to remain enabled.
