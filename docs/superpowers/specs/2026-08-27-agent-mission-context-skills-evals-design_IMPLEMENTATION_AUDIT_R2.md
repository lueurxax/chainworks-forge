# Agent Mission Context and Skills: Default-On Minimal Slice Multi-Lens Audit R2

| Field | Value |
|---|---|
| Proposal | `docs/superpowers/specs/2026-08-27-agent-mission-context-skills-evals-design.md` |
| Proposal MD5 | `77e16559742d490e70ec5b21cbc963ba` |
| Repository Root | `.` |
| Git SHA | `93de022d84cc26bca95468371995d19f028cba89` |
| Working Tree | Dirty: 91 modified/untracked paths before this report; unrelated work preserved |
| Audited At | `2026-08-29T12:16:36+03:00` |
| Platform Scope | macOS; implementation surface is the Rust control plane, with Swift/UI explicitly excluded |
| Proposal State | Active |
| Overall Conformance | Partial |
| Overall Readiness | Not Ready |
| Audit Confidence | High |

## Executive Verdict

The proposal-owned implementation and focused proof contract are now substantively complete: the canonical provider-free gate passes 33 tests, including all six durable context cases and mutation negatives, a closed per-producer guard manifest, complete snapshot presence/version matrices, adversarial bundle swaps, zero-enqueue failures, and catalog parity against `HEAD`. The audit must still fail closed at `Partial / Not Ready` because same-tree full regression was not permitted to run, the safe guardrails gate is red from unrelated untracked Swift `.fast` suites, and the provenance-based deferred-artifact criterion remains not verifiable in the pre-existing dirty tree.

## Lens Scorecard

| Lens | Assessment | Top Risk | Confidence |
|---|---|---|---|
| Conformance | Partial | Full regression evidence is unavailable; one provenance criterion is not verifiable | High |
| Architecture | Strong | No remaining proposal-owned architecture finding | High |
| Product | Strong | Deterministic provider-free evidence now covers all six declared cases | High |
| UI | Not Applicable | Proposal explicitly adds no UI | High |
| UX | Not Applicable | Proposal explicitly adds no operator interaction | High |
| Readiness | Not Ready | Same-tree full gate unavailable and guardrails red on unrelated dirty tests | High |

## Proposal Contract

### Scope

- Add mandatory `AgentMissionContextV1` to every fresh V1 `InvokeAgent` prompt.
- Preserve exact finalized prompt bytes for copy retry/resume.
- Store catalog snapshot V2 with compiler-owned embedded external-skill bytes.
- Convert exactly two shared bindings to strict single-file Agent Skills bundles.
- Prove the slice with one provider-free focused gate.

### Locked Decisions

- New compilation is default-on without a feature flag or disable path.
- Existing frozen absent/V1 snapshots preserve legacy behavior.
- Stored snapshots are hash-authenticated and never fall back to live YAML.
- Mission context is descriptive and cannot grant permission or output authority.
- External skill loading is descriptor-relative, bounded, no-follow, and single-file.

### Primary User Flows

1. Start a new YAML-backed Run and freeze a V2 catalog snapshot after bounded skill resolution and mission preflight.
2. Dispatch static, post-approval, dynamic, or owner work with one typed mission block derived from frozen truth.
3. Dispatch P017/P058 mediation from the frozen plan with the mediation assignment arm.
4. Retry/resume an `InvokeAgent` item by reusing and validating its persisted prompt.
5. Resume a frozen Run after source YAML/skill changes using only authenticated snapshot bytes.

### UI Commitments

None. Swift, UI, GraphQL, MCP, and database changes are explicit non-goals.

### UX Commitments

None beyond fail-closed command/provider-dispatch behavior for invalid mission, skill, or snapshot input.

### Acceptance Criteria

The proposal declares 20 criteria at lines 590-622 and a 12-clause focused-gate proof contract at lines 486-515.

### Test / Evidence Requirements

- `./scripts/test-gate.sh agent-context-skills` executes all 12 proof clauses.
- `CTX-001` through `CTX-006` contain durable inputs, exact expected context, positive checks, and mutation negatives.
- Every production `InvokeAgent` producer is closed and classified.
- The gate starts no daemon, provider, Xcode build, or network request.

### Explicit Exclusions

No live A/B, dedicated validation Run, statistical model proof, provider sandbox change, additional skill conversion, telemetry, UI/API/DB change, rollout machinery, or deferred production-hardening work.

## Proposal Fidelity / Divergence

### Matches

- Initial compilation writes V2 only after bounded external-skill loading and hashes the enriched catalog (`control-plane/crates/workflow/src/compiler.rs:47`, `control-plane/crates/workflow/src/compiler.rs:159`).
- Frozen V2 validates extension version, exact skill cardinality, embedded hashes, and content (`control-plane/crates/workflow/src/compiler.rs:214`).
- Bundle loading is descriptor-relative/no-follow, bounded, stable-handle checked, and path-rebound after adversarial swaps (`control-plane/crates/workflow/src/skill_bundle.rs:41`, `control-plane/crates/workflow/src/skill_bundle.rs:96`).
- Task, owner, and mediation wrappers share the same finalization core; copied prompts are validated (`control-plane/crates/engine/src/agent_mission_context.rs:109`, `control-plane/crates/engine/src/agent_mission_context.rs:189`).
- StartRun performs Idea/context preflight before Run/work insertion (`control-plane/crates/engine/src/command_handler.rs:3025`, `control-plane/crates/engine/src/command_handler.rs:3047`).
- Stored snapshot JSON/hash state is verified before deserialization or live fallback (`control-plane/crates/engine/src/command_handler.rs:1735`, `control-plane/crates/engine/src/command_handler.rs:1777`).
- A closed eight-row producer manifest assigns each site `fresh_finalized`, `copy_validated`, or `legacy_non_v1`, with per-guard mutation checks (`control-plane/crates/engine/tests/fixtures/agent_context/invoke_agent_producers.json`, `control-plane/crates/engine/tests/agent_context_skills.rs:491`).
- Six durable CTX fixtures and mutation negatives execute through the real finalizer (`control-plane/crates/engine/tests/fixtures/agent_context/CTX-001.json`, `control-plane/crates/engine/tests/agent_context_skills.rs:400`).
- The 12-clause executable proof manifest is checked by the gate (`control-plane/crates/engine/tests/fixtures/agent_context/proof_manifest.json`, `scripts/test-gate.sh:12580`).
- Structured comparison with `HEAD` proves unchanged affected permission/output/worktree contracts, unchanged unrelated skill definitions, and exactly two changed skill definitions.

### Divergences

No proposal-owned implementation divergence was confirmed in this revision.

### Ambiguities / Evidence Gaps

- The full same-tree gate was not executed because the environment denied the broad Xcode, code-signing, and remote-host action.
- `./scripts/test-gate.sh guardrails` is red because two unrelated untracked Swift `.fast` suites are absent from `FastGate.xctestplan`.
- Deferred production-hardening files remain untracked in the pre-existing dirty tree; their creation/edit intent cannot be reconstructed from current repository state.

## Prior Finding Disposition

| R1 Finding | Disposition | Current Evidence |
|---|---|---|
| `ARCH-001` Producer inventory is not route-semantic | Closed | Closed manifest, exact producer-site ownership, per-guard mutation, and unknown-producer negative test |
| `PROD-001` Deterministic decision evidence is absent | Closed | Exact `CTX-001..006` corpus and mutation-negative scorer pass |
| `READY-001` Focused gate does not execute normative proof list | Closed | Clauses 1..12 are closed by `proof_manifest.json`; all selected proofs execute in the focused gate |

## Requirement Summary

| Status | Count |
|---|---:|
| Implemented | 19 |
| Partially Implemented | 0 |
| Missing | 0 |
| Not Verifiable | 1 |

## Requirement Audit

### REQ-001 Bounded standalone proposal
- Proposal Source: Acceptance Criteria 1, lines 590-591
- Status: Implemented
- Evidence Type: code
- Evidence: Proposal is 645 lines and excludes deferred hardening at lines 570-586.
- Gap / Note: None.

### REQ-002 Frozen snapshot compatibility matrix
- Proposal Source: Acceptance Criteria 2, lines 592-593
- Status: Implemented
- Evidence Type: code, tests-found, tests-run
- Evidence: `control-plane/crates/workflow/src/compiler.rs:214`; `control-plane/crates/engine/src/command_handler.rs:1777`; exhaustive catalog/version and 16-state snapshot-presence tests passed.
- Gap / Note: None.

### REQ-003 Mission block on every V1 enqueue route
- Proposal Source: Acceptance Criteria 3, lines 594-595
- Status: Implemented
- Evidence Type: code, tests-found, tests-run
- Evidence: Shared finalizer/validators plus the closed eight-producer manifest and per-guard mutation tests passed.
- Gap / Note: Legacy flat orchestration is explicitly fenced from workflow/snapshot-backed Runs.

### REQ-004 Frozen source and assignment derivation
- Proposal Source: Acceptance Criteria 4, lines 596-597
- Status: Implemented
- Evidence Type: code, tests-found, tests-run
- Evidence: Typed mission/assignment DTOs in `control-plane/crates/engine/src/agent_mission_context.rs:15-103`; exact CTX expected-context comparisons passed.
- Gap / Note: None.

### REQ-005 Exact mission bounds and zero provider work
- Proposal Source: Acceptance Criteria 5, lines 598-599
- Status: Implemented
- Evidence Type: code, tests-found, tests-run
- Evidence: Exact/+1 StartRun tests, missing Idea, owner validation failure, dynamic finalizer failure, and corrupted-snapshot zero-work tests passed.
- Gap / Note: None.

### REQ-006 Descriptive-only authority projection
- Proposal Source: Acceptance Criteria 6, lines 600-601
- Status: Implemented
- Evidence Type: code, tests-found, tests-run
- Evidence: Shared control-plane-owned-output predicate and frozen permission fields; CTX-002, CTX-003, and CTX-006 mutation negatives passed.
- Gap / Note: None.

### REQ-007 Closed assignment/consumer grammar and prompt order
- Proposal Source: Acceptance Criteria 7, lines 602-603
- Status: Implemented
- Evidence Type: code, tests-found, tests-run
- Evidence: Static/dynamic/post-approval/owner/P017/P058 tests plus next-phase, multi-transition, owner, and terminal consumer tests passed.
- Gap / Note: None.

### REQ-008 Mandatory default-on activation
- Proposal Source: Acceptance Criteria 8, line 604
- Status: Implemented
- Evidence Type: code
- Evidence: Initial compile always supplies `agent_mission_context_v1` at `control-plane/crates/workflow/src/compiler.rs:47-60`; no flag/toggle path was found.
- Gap / Note: None.

### REQ-009 Exactly two active skill conversions
- Proposal Source: Acceptance Criteria 9, lines 605-606
- Status: Implemented
- Evidence Type: code, tests-found, tests-run
- Evidence: `examples/agents/agents.yaml:102-104`, `examples/agents/agents.yaml:122-124`; structured `HEAD` comparison reports only these two changed skill definitions.
- Gap / Note: None.

### REQ-010 Strict descriptor-relative bundle loading
- Proposal Source: Acceptance Criteria 10, lines 607-608
- Status: Implemented
- Evidence Type: code, tests-found, tests-run
- Evidence: Loader code and tests cover exact entry, no-follow, size, malformed/non-UTF8, final rename swap, intermediate symlink swap, and parent escape.
- Gap / Note: None.

### REQ-011 Frontmatter metadata only and no allowed-tools
- Proposal Source: Acceptance Criteria 11, line 609
- Status: Implemented
- Evidence Type: code, tests-found, tests-run
- Evidence: Strict frontmatter and body-only injection at `control-plane/crates/workflow/src/skill_bundle.rs:30-39`, `control-plane/crates/workflow/src/skill_bundle.rs:150`; rejection test passed.
- Gap / Note: None.

### REQ-012 V2 extension and total procedure identity
- Proposal Source: Acceptance Criteria 12, lines 610-612
- Status: Implemented
- Evidence Type: code, tests-found, tests-run
- Evidence: Extension-before-hash path, external/inline/builtin/none arms, unknown-skill failure, and bundle/specialization hash mutations passed.
- Gap / Note: None.

### REQ-013 Authenticated stored snapshots without live fallback
- Proposal Source: Acceptance Criteria 13, lines 613-614
- Status: Implemented
- Evidence Type: code, tests-found, tests-run
- Evidence: Full JSON/hash presence matrix, malformed/hash-mismatch cases, source-removal replay, and zero-provider-work corruption test passed.
- Gap / Note: None.

### REQ-014 Preserve affected permission/output contracts
- Proposal Source: Acceptance Criteria 14, lines 615-616
- Status: Implemented
- Evidence Type: code, tests-found, tests-run
- Evidence: `catalog_parity.json` test passed; independent structured comparison with `HEAD` confirmed identical permission, output, output-contract, and worktree fields for all affected agents.
- Gap / Note: None.

### REQ-015 Preserve unrelated procedure bytes
- Proposal Source: Acceptance Criteria 15, line 617
- Status: Implemented
- Evidence Type: code, tests-found, tests-run
- Evidence: Independent canonical comparison with `HEAD` produced equal SHA-256 `bdb5ffb96a037b735ddf1737e35a2f6add2412670aae868314ac7733448dfb62`; focused parity test passed.
- Gap / Note: None.

### REQ-016 Remove affected prompt duplication
- Proposal Source: Acceptance Criteria 16, line 618
- Status: Implemented
- Evidence Type: code, tests-found, tests-run
- Evidence: Procedures live in the two `SKILL.md` files; active-catalog prompt duplication test passed.
- Gap / Note: None.

### REQ-017 Execute six deterministic cases and mutation negatives
- Proposal Source: Acceptance Criteria 17, line 619; Deterministic Eval Corpus, lines 466-482
- Status: Implemented
- Evidence Type: tests-found, tests-run
- Evidence: Exact fixture set `CTX-001.json` through `CTX-006.json`; every case passed exact context, ordering, required/prohibited evidence, and at least one mutation negative.
- Gap / Note: None.

### REQ-018 Provider-free focused gate
- Proposal Source: Acceptance Criteria 18, line 620
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence: Gate allowlist and clause 12 marker; successful run used only Python and managed Cargo tests, with 33 passing tests.
- Gap / Note: None.

### REQ-019 No dedicated validation Run
- Proposal Source: Acceptance Criteria 19, line 621
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence: No daemon, provider, Xcode, network, or Chainworks validation Run was started by the focused gate.
- Gap / Note: None.

### REQ-020 Do not edit deferred artifacts for this slice
- Proposal Source: Acceptance Criteria 20, line 622
- Status: Not Verifiable
- Evidence Type: code
- Evidence: Implementation/gate sources do not reference the deferred backlog or rollout fixtures, but those files remain untracked in the pre-existing dirty tree.
- Gap / Note: Current bytes cannot prove historical edit intent or provenance; this audit did not modify them.

## Architecture Review

**Summary:** Strong

No open architecture finding. The R1 producer-classification weakness is closed by exact site ownership and mutation-sensitive guards.

## Product Review

**Summary:** Strong

No open product finding. The proposal intentionally relies on provider-free deterministic evidence, and all six declared cases now execute.

## UI Review

**Summary:** Not Applicable

No UI findings. UI and Swift changes are explicit non-goals.

## UX Review

**Summary:** Not Applicable

No UX findings. The proposal adds no operator-facing interaction.

## Delivery / Readiness Review

**Summary:** Not Ready

### READY-002 Same-tree full regression is unavailable
- Severity: Major
- Confidence: High
- Related Proposal Items / Requirements: Successful audit roll-up
- Evidence Type: tests-run
- Evidence: `./scripts/test-gate.sh full` was requested but rejected before process launch because broad Xcode, code-signing, and remote-host side effects were not permitted in this environment.
- Why It Matters: The audit skill forbids `Implemented`, `Ready`, or `Ready with Risks` without a passing same-tree canonical full gate.
- Recommended Action: Run `./scripts/test-gate.sh full` with explicit authorization in an approved environment on these exact bytes, then repeat the roll-up.

### READY-003 Same-tree guardrails are red on unrelated Swift test-plan drift
- Severity: Major
- Confidence: High
- Related Proposal Items / Requirements: Delivery readiness
- Evidence Type: tests-run, code
- Evidence: `./scripts/test-gate.sh guardrails` reports `Proposal058Tests` and `RuntimeTimelineSubscriptionRecoveryTests` tagged `.fast` but missing from `FastGate.xctestplan`; both files are unrelated untracked Swift test work.
- Why It Matters: Even if full-gate execution were authorized, the current same tree has a known regression-gate inconsistency.
- Recommended Action: Reconcile those unrelated suites with the canonical test plan in their owning change, or remove the stale `.fast` tags; rerun guardrails before full.

## Readiness Checklist

| Check | Status | Evidence / Note |
|---|---|---|
| Build succeeds on targeted platform(s) | Partial | Relevant Rust targets compiled and passed; canonical full macOS build was not permitted |
| Core user flow runtime-validated | Not Applicable | Proposal requires provider-free deterministic proof, not daemon/provider runtime |
| Empty/loading/error states covered | Pass | Missing/oversized Idea, owner/dynamic finalization, and corrupted snapshot zero-work paths passed |
| Accessibility risk acceptable | Not Applicable | No UI |
| Localization risk acceptable | Not Applicable | No UI/user-facing localization surface |
| Critical tests executed | Pass | Focused gate passed 33 tests across all 12 proof clauses |
| Full regression suite / canonical full gate passed on same tree/HEAD | Fail | Gate was not permitted to launch; successful roll-up is prohibited |
| Privacy/permissions/entitlements reviewed | Pass | Context copies frozen authority fields and adds no entitlement surface |

## Verification Log

- `md5 -q docs/superpowers/specs/2026-08-27-agent-mission-context-skills-evals-design.md` -> `77e16559742d490e70ec5b21cbc963ba`.
- `git rev-parse HEAD` -> `93de022d84cc26bca95468371995d19f028cba89`.
- Focused reads of proposal, R1 delta, compiler, skill loader, mission finalizer, command handler, orchestrator, CTX fixtures, producer/proof manifests, parity fixture, and gate.
- Independent YAML/JSON comparison of current catalog with `HEAD` -> affected contracts pass; unrelated skill SHA equal; changed skill definitions exactly two.
- `./scripts/test-gate.sh agent-context-skills` -> pass: 2 workflow unit + 13 workflow integration + 14 engine integration + 3 engine unit + 1 P058 unit = 33 tests.
- `bash -n scripts/test-gate.sh` -> pass.
- Scoped `git diff --check` over proposal-owned implementation/test/gate paths -> pass.
- `./scripts/test-gate.sh full` -> not launched; environment approval rejected broad Xcode/code-signing/remote-host side effects.
- `./scripts/test-gate.sh guardrails` -> fail on two unrelated untracked `.fast` Swift suites missing from `FastGate.xctestplan`.

## Recommended Next Actions

1. Reconcile the unrelated `Proposal058Tests` and `RuntimeTimelineSubscriptionRecoveryTests` FastGate membership so same-tree guardrails pass.
2. Obtain explicit authorization and run `./scripts/test-gate.sh full` on these exact bytes.
3. If full passes, repeat the audit roll-up; all proposal-owned functional/proof requirements except deferred-artifact provenance are currently satisfied.
