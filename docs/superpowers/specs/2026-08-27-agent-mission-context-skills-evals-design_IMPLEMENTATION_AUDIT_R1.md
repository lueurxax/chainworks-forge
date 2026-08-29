# Agent Mission Context and Skills: Default-On Minimal Slice Multi-Lens Audit R1

| Field | Value |
|---|---|
| Proposal | `docs/superpowers/specs/2026-08-27-agent-mission-context-skills-evals-design.md` |
| Proposal MD5 | `77e16559742d490e70ec5b21cbc963ba` |
| Repository Root | `.` |
| Git SHA | `93de022d84cc26bca95468371995d19f028cba89` |
| Working Tree | Dirty: 88 modified/untracked paths; unrelated work preserved |
| Audited At | `2026-08-29T11:26:15+03:00` |
| Platform Scope | macOS; implementation surface is the Rust control plane, with Swift/UI explicitly excluded |
| Proposal State | Active |
| Overall Conformance | Not Implemented |
| Overall Readiness | Not Ready |
| Audit Confidence | High |

## Executive Verdict

The compiler, frozen-snapshot, prompt-finalization, StartRun-preflight, and strict single-file skill-loader slice is substantially implemented, and the focused gate passes 19 tests. The proposal is nevertheless not implemented as written because the required six-case deterministic corpus and mutation negatives do not exist, and the producer-inventory check does not prove that each production `InvokeAgent` producer is fresh-finalized, copy-validated, or an explicit legacy exclusion. The passing focused gate therefore does not execute the proof contract declared by the proposal.

## Lens Scorecard

| Lens | Assessment | Top Risk | Confidence |
|---|---|---|---|
| Conformance | Not Implemented | Mandatory deterministic corpus and source-inventory proof are absent | High |
| Architecture | At Risk | Producer inventory is count/name based rather than route-semantic | High |
| Product | At Risk | The claimed provider-free decision evidence is not executable | High |
| UI | Not Applicable | Proposal explicitly adds no UI | High |
| UX | Not Applicable | Proposal explicitly adds no operator interaction | High |
| Readiness | Not Ready | Green focused gate proves less than its normative contract | High |

## Proposal Contract

### Scope

- Add mandatory `AgentMissionContextV1` to every fresh V1 `InvokeAgent` prompt.
- Preserve exact finalized prompt bytes for copy-based retry/resume.
- Store compiler-owned catalog snapshot V2 with embedded validated external-skill bytes.
- Convert exactly two shared bindings to strict single-file Agent Skills bundles.
- Prove the slice with one provider-free focused gate.

### Locked Decisions

- New compilation is default-on, without a feature flag or disable path.
- Existing frozen absent/V1 snapshots preserve legacy behavior.
- Stored snapshots are hash-authenticated and never fall back to live YAML.
- Mission context is descriptive and cannot grant permission or output authority.
- External skill loading is descriptor-relative, bounded, no-follow, and single-file.

### Primary User Flows

1. Start a new YAML-backed Run and freeze a V2 catalog snapshot after bounded skill resolution and mission preflight.
2. Dispatch static, post-approval, dynamic, or owner work with one typed mission block derived from frozen truth.
3. Dispatch P017/P058 mediation from the frozen plan with the mediation assignment arm.
4. Retry/resume an `InvokeAgent` item by reusing and validating its exact persisted prompt.
5. Resume a frozen Run after source YAML/skill changes and use only authenticated snapshot bytes.

### UI Commitments

None. New Swift, UI, GraphQL, MCP, and database surfaces are explicit non-goals.

### UX Commitments

None beyond existing command failure behavior: invalid mission/skill/snapshot input must fail before provider dispatch.

### Acceptance Criteria

The proposal declares 20 numbered acceptance criteria at lines 590-622. The principal proof contract is the 12-part provider-free gate at lines 486-515.

### Test / Evidence Requirements

- `./scripts/test-gate.sh agent-context-skills` must execute all 12 proof categories.
- Six durable `CTX-001` through `CTX-006` cases must have exact positive and mutation-negative assertions.
- Source inventory must cover and classify every production `InvokeAgent` producer.
- No daemon, provider, Xcode build, or network call is required.

### Explicit Exclusions

No live A/B, dedicated validation Run, statistical model proof, provider sandbox change, additional skill conversion, telemetry, UI/API/DB change, rollout machinery, or deferred production-hardening work.

## Proposal Fidelity / Divergence

### Matches

- Initial compilation writes catalog snapshot V2 only after loading embedded external-skill bytes (`control-plane/crates/workflow/src/compiler.rs:47`, `control-plane/crates/workflow/src/compiler.rs:159`).
- Frozen V2 validation enforces extension version, exact external-skill cardinality, and bundle digest/content validation (`control-plane/crates/workflow/src/compiler.rs:214`).
- The skill loader uses descriptor-relative no-follow opens, stable-handle checks, bounded reads, exact entry enumeration, and strict frontmatter (`control-plane/crates/workflow/src/skill_bundle.rs:41`).
- The engine exposes one internal finalization core through task, owner, and mediation assignment wrappers and validates copied V1 prompts (`control-plane/crates/engine/src/agent_mission_context.rs:109`, `control-plane/crates/engine/src/agent_mission_context.rs:189`).
- StartRun reads the Idea and runs mission preflight before the Run transaction/insertion (`control-plane/crates/engine/src/command_handler.rs:2991`, `control-plane/crates/engine/src/command_handler.rs:3025`, `control-plane/crates/engine/src/command_handler.rs:3047`).
- Stored snapshot JSON/hash quartet is verified before deserialization, with live compilation only for the all-absent legacy state (`control-plane/crates/engine/src/command_handler.rs:1735`, `control-plane/crates/engine/src/command_handler.rs:1777`).
- Exactly the two named skill definitions are converted in `examples/agents/agents.yaml:102` and `examples/agents/agents.yaml:122`; both bundle directories contain only `SKILL.md`.
- The focused gate passes 10 workflow tests, 8 engine tests, and 1 P058 regression test.

### Divergences

- No `CTX-001` through `CTX-006` fixture/case identifiers, durable expected contexts, or complete mutation-negative scorer are present in either focused test target or the gate.
- Source inventory checks only producer counts and global helper-name presence. It does not bind each producer site to a fresh-finalizer, copy-validator, or explicit legacy classification (`scripts/test-gate.sh:12591`).
- Several proof categories promised by the gate are not exercised: full snapshot-state permutation coverage; owner/dynamic enqueue failure with zero work; terminal/multi-transition/parallel consumer shapes; oversized/malformed/rename-swap and intermediate symlink-swap bundle cases; and before/after parity for affected and unrelated catalog contracts.

### Ambiguities / Evidence Gaps

- The dirty tree contains unrelated proposal and implementation work, including deferred artifacts. This audit did not attribute their provenance and did not modify them.
- No full regression gate was run because the audit already has a `Missing` requirement and cannot reach a successful roll-up.

## Requirement Summary

| Status | Count |
|---|---:|
| Implemented | 18 |
| Partially Implemented | 0 |
| Missing | 1 |
| Not Verifiable | 1 |

## Requirement Audit

### REQ-001 Bounded standalone proposal
- Proposal Source: Acceptance Criteria 1, lines 590-591
- Status: Implemented
- Evidence Type: code
- Evidence: Proposal is 645 lines and marks deferred hardening as excluded at lines 570-586.
- Gap / Note: None.

### REQ-002 Frozen snapshot compatibility matrix
- Proposal Source: Acceptance Criteria 2, lines 592-593
- Status: Implemented
- Evidence Type: code, tests-found, tests-run
- Evidence: `control-plane/crates/workflow/src/compiler.rs:214`; `control-plane/crates/engine/src/command_handler.rs:1777`; focused workflow compatibility tests and engine quartet tests passed.
- Gap / Note: Runtime code is total; exhaustive permutation tests remain a readiness-evidence gap recorded below.

### REQ-003 Mission block on every V1 enqueue route
- Proposal Source: Acceptance Criteria 3, lines 594-595
- Status: Implemented
- Evidence Type: code, tests-found, tests-run
- Evidence: `control-plane/crates/engine/src/agent_mission_context.rs:109`; task enqueue validates at `control-plane/crates/engine/src/orchestrator.rs:2769`; dynamic finalization occurs before insertion at `control-plane/crates/engine/src/orchestrator.rs:5332`; copy retries validate persisted prompts in orchestrator and command handler.
- Gap / Note: Direct code inspection supports the behavior, but the source-inventory regression check is not route-semantic.

### REQ-004 Frozen source and assignment derivation
- Proposal Source: Acceptance Criteria 4, lines 596-597
- Status: Implemented
- Evidence Type: code, tests-found, tests-run
- Evidence: Typed DTO/assignment construction in `control-plane/crates/engine/src/agent_mission_context.rs:15-103` and frozen procedure validation in `control-plane/crates/engine/src/agent_mission_context.rs:312`.
- Gap / Note: None found.

### REQ-005 Exact mission bounds and zero provider work
- Proposal Source: Acceptance Criteria 5, lines 598-599
- Status: Implemented
- Evidence Type: code, tests-found, tests-run
- Evidence: `control-plane/crates/engine/src/agent_mission_context.rs:165`; `control-plane/crates/engine/src/command_handler.rs:3025`; `production_start_run_rejects_missing_or_oversized_idea_before_run_and_work_insert` passed.
- Gap / Note: Dynamic/owner finalization failure is correctly ordered before enqueue in code but lacks the separately promised integration proof.

### REQ-006 Descriptive-only authority projection
- Proposal Source: Acceptance Criteria 6, lines 600-601
- Status: Implemented
- Evidence Type: code, tests-found, tests-run
- Evidence: The same control-plane-owned-output predicate is reused at `control-plane/crates/engine/src/agent_mission_context.rs:105` and `control-plane/crates/engine/src/orchestrator.rs:8524`; permission fields come from frozen `ResolvedAgent`.
- Gap / Note: None found.

### REQ-007 Closed assignment/consumer grammar and prompt order
- Proposal Source: Acceptance Criteria 7, lines 602-603
- Status: Implemented
- Evidence Type: code, tests-found, tests-run
- Evidence: Closed serde-tagged unions and finalizer ordering in `control-plane/crates/engine/src/agent_mission_context.rs:47-103` and `control-plane/crates/engine/src/agent_mission_context.rs:235`; focused assignment/order tests passed.
- Gap / Note: Some declared shape variants lack focused test cases; see READY-001.

### REQ-008 Mandatory default-on activation
- Proposal Source: Acceptance Criteria 8, line 604
- Status: Implemented
- Evidence Type: code
- Evidence: Initial compile always supplies `agent_mission_context_v1` at `control-plane/crates/workflow/src/compiler.rs:47-60`; no flag, environment toggle, or optional configuration path was found.
- Gap / Note: None.

### REQ-009 Exactly two active skill conversions
- Proposal Source: Acceptance Criteria 9, lines 605-606
- Status: Implemented
- Evidence Type: code, tests-found, tests-run
- Evidence: `examples/agents/agents.yaml:102-104`, `examples/agents/agents.yaml:122-124`; focused gate active-binding scan passed.
- Gap / Note: None.

### REQ-010 Strict descriptor-relative bundle loading
- Proposal Source: Acceptance Criteria 10, lines 607-608
- Status: Implemented
- Evidence Type: code, tests-found, tests-run
- Evidence: `control-plane/crates/workflow/src/skill_bundle.rs:41-103`; auxiliary-entry, final symlink, parent escape, and allowed-tools tests passed.
- Gap / Note: The implementation is present; complete adversarial fixture coverage required by the gate is incomplete.

### REQ-011 Frontmatter metadata only and no allowed-tools
- Proposal Source: Acceptance Criteria 11, line 609
- Status: Implemented
- Evidence Type: code, tests-found, tests-run
- Evidence: Strict frontmatter at `control-plane/crates/workflow/src/skill_bundle.rs:30-39`; body-only return at `control-plane/crates/workflow/src/skill_bundle.rs:127-182`; focused tests passed.
- Gap / Note: None.

### REQ-012 V2 extension and total procedure identity
- Proposal Source: Acceptance Criteria 12, lines 610-612
- Status: Implemented
- Evidence Type: code, tests-found, tests-run
- Evidence: Extension construction precedes canonical serialization/hash at `control-plane/crates/workflow/src/compiler.rs:159-211`; procedure union/hash validation at `control-plane/crates/engine/src/agent_mission_context.rs:94-103` and `control-plane/crates/engine/src/agent_mission_context.rs:312`; focused tests passed.
- Gap / Note: None.

### REQ-013 Authenticated stored snapshots without live fallback
- Proposal Source: Acceptance Criteria 13, lines 613-614
- Status: Implemented
- Evidence Type: code, tests-found, tests-run
- Evidence: `control-plane/crates/engine/src/command_handler.rs:1735-1803`; removed-source and corrupted-bundle tests passed.
- Gap / Note: Full malformed/permutation proof coverage is incomplete.

### REQ-014 Preserve affected permission/output contracts
- Proposal Source: Acceptance Criteria 14, lines 615-616
- Status: Implemented
- Evidence Type: code, tests-found
- Evidence: Working-tree diff converts skill definitions but does not change permission/output fields on affected reviewer or code-writer agents; prompt projection test asserts frozen `CODE_WRITE` and output partition.
- Gap / Note: No generated before/after golden is executed by the focused gate.

### REQ-015 Preserve unrelated procedure bytes
- Proposal Source: Acceptance Criteria 15, line 617
- Status: Implemented
- Evidence Type: code, tests-found
- Evidence: Catalog diff changes only the two named skill definitions; inline and builtin resolver paths retain their procedure construction; focused tests exercise all source kinds.
- Gap / Note: No exhaustive approved-HEAD byte-parity fixture is executed.

### REQ-016 Remove affected prompt duplication
- Proposal Source: Acceptance Criteria 16, line 618
- Status: Implemented
- Evidence Type: code, tests-found, tests-run
- Evidence: Both procedures moved to `examples/agents/skills/*/SKILL.md`; `active_catalog_preserves_procedure_kinds_and_does_not_duplicate_bundle_body_in_prompts` passed.
- Gap / Note: None.

### REQ-017 Execute six deterministic cases and mutation negatives
- Proposal Source: Acceptance Criteria 17, line 619; Deterministic Eval Corpus, lines 466-482
- Status: Missing
- Evidence Type: tests-found, tests-run
- Evidence: No `CTX-001` through `CTX-006` identifiers or equivalent six durable fixture records exist in `control-plane/crates/workflow/tests/agent_context_skills.rs`, `control-plane/crates/engine/tests/agent_context_skills.rs`, or the gate. The passing test targets contain 18 focused tests but no complete positive/mutation-negative scorer.
- Gap / Note: Add six durable cases containing Run/Idea input, frozen plan input, exact expected mission context, and positive plus mutation-negative assertions; execute all six from the focused gate.

### REQ-018 Provider-free focused gate
- Proposal Source: Acceptance Criteria 18, line 620
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence: `scripts/test-gate.sh:12580-12650`; successful gate ran local static checks and Cargo tests only.
- Gap / Note: The gate is provider-free, but its proof breadth is incomplete under REQ-017 and READY-001.

### REQ-019 No dedicated validation Run
- Proposal Source: Acceptance Criteria 19, line 621
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence: Focused verification used local tests only; no daemon, provider, or Chainworks Run was started.
- Gap / Note: None.

### REQ-020 Do not edit deferred artifacts for this slice
- Proposal Source: Acceptance Criteria 20, line 622
- Status: Not Verifiable
- Evidence Type: code
- Evidence: The production-hardening backlog and old rollout sidecar/fixtures are untracked in the pre-existing dirty tree.
- Gap / Note: This audit preserved them and cannot establish why or when those unrelated files were created.

## Architecture Review

**Summary:** At Risk

### ARCH-001 Producer inventory is not route-semantic
- Severity: Major
- Confidence: High
- Related Proposal Items / Requirements: REQ-003, REQ-017
- Evidence Type: code, tests-run
- Evidence: `scripts/test-gate.sh:12591-12621` compares only two occurrence counts and checks whether helper names appear anywhere in each production file. The seven orchestrator sites include fresh, copy, mediation, dynamic, and legacy-flat producers, but the gate has no per-site classification or call-path assertion.
- Why It Matters: A producer can bypass finalization/validation while the count and unrelated helper references remain unchanged, leaving the gate green despite violating the default-on invariant.
- Recommended Action: Generate or maintain an exact producer manifest keyed by stable producer identity and require each entry to prove `fresh_finalized`, `copy_validated`, or `legacy_non_v1`; add a mutation test that removes/bypasses each required guard and makes the gate fail.

## Product Review

**Summary:** At Risk

### PROD-001 The declared deterministic decision evidence is absent
- Severity: Major
- Confidence: High
- Related Proposal Items / Requirements: REQ-017
- Evidence Type: tests-found, tests-run
- Evidence: Proposal lines 466-482 require six durable cases and mutation negatives; the focused targets contain no `CTX-001..006` corpus or equivalent fixture inventory.
- Why It Matters: The minimal slice intentionally substitutes provider-free deterministic evidence for live evaluation. Without that corpus, the proposal's stated completion rule does not test the six product mistakes it claims to prevent.
- Recommended Action: Add and execute the six exact cases with durable inputs/expected contexts and mutation negatives; keep them provider-free as specified.

## UI Review

**Summary:** Not Applicable

No UI findings. UI and Swift changes are explicit non-goals.

## UX Review

**Summary:** Not Applicable

No UX findings. The proposal adds no operator-facing interaction.

## Delivery / Readiness Review

**Summary:** Not Ready

### READY-001 The green focused gate does not execute its normative proof list
- Severity: Major
- Confidence: High
- Related Proposal Items / Requirements: REQ-002, REQ-005, REQ-007, REQ-010, REQ-013, REQ-014, REQ-015, REQ-017
- Evidence Type: tests-found, tests-run
- Evidence: The gate invokes only two focused integration targets and one P058 unit test at `scripts/test-gate.sh:12643-12648`. Missing proof cases include the six-case corpus, full snapshot matrix, dynamic/owner zero-enqueue failures, terminal/multi-transition/parallel consumers, oversized/malformed/rename-swap/intermediate-symlink bundle fixtures, and before/after catalog parity.
- Why It Matters: The gate's pass result can be mistaken for proposal closeout even though multiple explicitly numbered proof obligations never execute.
- Recommended Action: Expand the focused test targets until each of the 12 proposal gate clauses maps to at least one executed test or static invariant, and have the gate print/fail on a closed proof manifest.

## Readiness Checklist

| Check | Status | Evidence / Note |
|---|---|---|
| Build succeeds on targeted platform(s) | Partial | Relevant Rust workflow/engine test targets compiled and passed; no full macOS/Swift build was required or run |
| Core user flow runtime-validated | Not Applicable | Proposal intentionally requires provider-free deterministic proof, not daemon/provider runtime |
| Empty/loading/error states covered | Partial | StartRun missing/oversized failures pass; dynamic/owner enqueue error proofs are absent |
| Accessibility risk acceptable | Not Applicable | No UI |
| Localization risk acceptable | Not Applicable | No UI/user-facing localization surface |
| Critical tests executed | Partial | Focused gate passed, but does not execute the complete normative proof contract |
| Full regression suite / canonical full gate passed on same tree/HEAD | Not Checked | Not run because REQ-017 is Missing and a successful roll-up is already impossible |
| Privacy/permissions/entitlements reviewed | Pass | Context copies frozen permission identity and adds no authority/entitlement surface |

## Verification Log

- `md5 -q docs/superpowers/specs/2026-08-27-agent-mission-context-skills-evals-design.md` -> `77e16559742d490e70ec5b21cbc963ba`
- `git rev-parse HEAD` -> `93de022d84cc26bca95468371995d19f028cba89`
- Focused `rg` and line reads across compiler, bundle loader, mission finalizer, command handler, orchestrator, catalog, tests, and gate.
- `./scripts/test-gate.sh agent-context-skills` -> initial sandbox-only shared-cache denial; rerun with shared managed Cargo cache succeeded: 10 workflow tests, 8 engine tests, 1 P058 unit test.
- `bash -n scripts/test-gate.sh` -> pass.
- Scoped `git diff --check` over relevant implementation/test/gate paths -> pass.
- Full regression gate not run because this audit is non-successful independent of full-regression status.

## Recommended Next Actions

1. Implement and execute the six durable `CTX-001..006` positive/mutation-negative cases.
2. Replace count/name source inventory with a closed per-producer classification and bypass-negative checks.
3. Complete the remaining proof-list fixtures, then rerun `./scripts/test-gate.sh agent-context-skills`.
4. Once every `REQ-*` is directly proven, run the same-tree canonical full gate before claiming `Implemented` or `Ready`.
