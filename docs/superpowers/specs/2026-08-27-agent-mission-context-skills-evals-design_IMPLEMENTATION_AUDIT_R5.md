# Agent Mission Context and Skills: Default-On Minimal Slice Implementation Audit R5

| Field | Value |
|---|---|
| Proposal | `docs/superpowers/specs/2026-08-27-agent-mission-context-skills-evals-design.md` |
| Proposal MD5 | `77e16559742d490e70ec5b21cbc963ba` |
| Implementation Target | `main` commit `b56dcc1ec76a58fa90876d189f9f20dc357f18e5` |
| Compare Base | R4 target `dbd66bbdd0fb25ec7e84aea7b25439eabe0b0755` |
| Audit Timestamp | `2026-08-30T07:46:36Z` |
| Audit Tree | Clean detached worktree at the exact implementation target; dirty main-worktree files excluded from evidence |
| Mode | `implementation-readiness` |
| Proposal State | Active |
| Platform / Product Scope | Rust control-plane compiler, prompt assembly, work-item lifecycle, and provider-free proof gate |
| Overall Conformance | **Partial** |
| Overall Implementation Readiness | **Not Ready** |
| Reviewer Selection Reuse | **Reused with R4 delta**: architecture, reliability, security, API/frozen-contract, and Chainworks execution-truth lenses |
| Audit Confidence | **High** |

## Executive Verdict

R5 remains fail-closed at `Partial / Not Ready`. The exact-tree canonical gate is green: 57 tests passed, one explicit fixture-regeneration test was ignored, and no test failed. The R4 producer-inventory blocker is closed. The R4 dynamic finalizer/settlement blocker is closed for the minimal V1 contract: the complete prepared work set is committed transactionally, late finalizer failure leaves zero work, blocked evidence/stage/Run writes are atomic, production missing-Idea is covered through `advance_run`, and write-failure replay is exercised. The 24 KiB pre-parse bound and Run/Idea/task-consumer validation are also implemented.

One Major in-scope blocker remains. Persisted mediation copy validation proves only that the substituted lead is some agent in the frozen plan and that the substituted contract and conflict/escalation IDs are non-empty and self-consistent between prompt and payload. It does not prove the exact frozen lead, frozen lead-resolution contract, or durable P017/P058 identity that originally authorized the mediation assignment. This directly violates the proposal's frozen mediation-assignment and exact copy-validation contract.

The separately observed P060 selection-plan binding mismatch/unknown-binding behavior is not used as an R5 blocker. It belongs to P060 routing-artifact admission and does not cause an actually enqueued minimal-V1 item to omit or bypass mission finalization. Task inputs also remain outside the canonical mission object by the proposal's explicit JSON and assignment grammar; they stay in the task body.

## Prior Review Reuse

- R4 architecture, reliability, security, API/frozen-contract, and execution-truth selections remain the smallest sufficient set for the same Rust surfaces.
- Product, rollout/observability, performance, and Apple UI/UX remain excluded because the proposal adds no experiment, rollout, telemetry, API, UI, or live-provider behavior.
- R4 findings are context only. Every disposition below was rechecked against current `b56dcc1e` code and exact-tree test output.

## Reviewer Routing

### Selected Reviewers

| Reviewer | Trigger | Result | Confidence |
|---|---|---|---|
| Rust architecture | Producer ownership and recursive source-tree closure | Pass | High |
| Rust reliability | Dynamic fan-out, transactional settlement, replay, and P017/P058 lifecycle | Pass for minimal V1 scope | High |
| Rust security | Parser bounds and frozen authority projection on copied work | Fail: `SEC-001` | High |
| API / frozen contract | Absent/V1/V2 compatibility and exact persisted assignment validation | Fail: `SEC-001` | High |
| Chainworks execution truth | Run/Idea/frozen-plan/payload/work-item parity | Fail: `SEC-001` | High |

### Rejected Close Alternatives

| Reviewer | Reason rejected |
|---|---|
| Product | Live A/B, metrics, causal model proof, and rollout decisions are explicit non-goals |
| Observability / rollout | No migration, flag, telemetry, alert, release receipt, or production rollout belongs to this proposal |
| Performance | No latency/throughput claim; the explicit parser bounds were covered by security/reliability |
| Apple UI / UX | No Swift, screen, navigation, accessibility, or operator-interaction change is in scope |

## Proposal Contract

### Scope

- Add one mandatory typed `AgentMissionContextV1` to every fresh V1 `InvokeAgent` prompt.
- Preserve exact persisted prompt bytes for copy retry/resume after bounded, typed validation against durable and frozen truth.
- Derive mission, assignment, consumers, completion, permission, and output ownership only from the Run/Idea and frozen plan/catalog.
- Compile catalog snapshot V2 after bounded external-skill resolution and preserve authenticated absent/V1/V2 behavior.
- Convert the two minimal bindings to descriptor-relative single-file Agent Skills bundles.
- Prove all commitments through one provider-free 12-clause canonical gate.

### Primary Service Flows

1. Preflight Idea/context bounds, compile V2, and insert a Run only after frozen inputs are valid.
2. Finalize one mission block for every static, post-approval, dynamic, owner-only, P017, and P058 provider enqueue.
3. Copy retry/resume exact persisted V1 prompt bytes only after validating their complete durable and frozen contract.
4. Resume absent/V1/V2 frozen Runs without live workflow/catalog/skill fallback.
5. Fail dynamic/P017 finalization with zero provider work and coherent durable state.

### Explicit Exclusions

The production-hardening backlog, rollout sidecar, and full-surface fixtures are deferred and non-normative. The merged security/pre-push proposal is a separately owned follow-up. P060 selection-plan artifact admission beyond the mission-finalization boundary remains owned by the existing P060 routing contract. None of these scopes is used to excuse the remaining mediation copy-validation gap.

## Evidence Pack

### Identity and Provenance

- `b56dcc1e` resolved to `b56dcc1ec76a58fa90876d189f9f20dc357f18e5`, is contained by `main`, and has parent `dbd66bbdd0fb25ec7e84aea7b25439eabe0b0755`.
- Proposal MD5 is `77e16559742d490e70ec5b21cbc963ba`; proposal length is 645 lines.
- All code and test evidence came from a clean detached worktree at the exact commit.
- The dirty main worktree was not used as implementation evidence and was not modified except for this single R5 report.

### Requested Validation

| Command | Result | Evidence meaning |
|---|---|---|
| `./scripts/test-gate.sh agent-context-skills` | **PASS**: 57 passed, 1 explicitly ignored fixture-regeneration test, 0 failed | Same-tree canonical gate is green; P058 deadline/resume passed 8/8 and all seven `agent_context_` engine tests passed |
| `bash -n scripts/test-gate.sh` | **PASS** | Gate script syntax is valid |
| `git diff --check dbd66bbd..b56dcc1e` | **PASS** | R5 delta has no whitespace-error evidence |
| `rustfmt --edition 2021 --check <eight changed Rust files>` | **PASS** | Every Rust file changed by R5 is formatted |

The initial sandboxed gate attempt failed only because the shared managed Cargo cache was inaccessible. The exact command was rerun with approved cache access and passed. A whole-workspace `cargo fmt --all --check` also reports pre-existing formatting differences in unrelated P089 domain/GraphQL/MCP files that are absent from the R5 diff; the requested scoped Rust check is green, so this is recorded as a baseline caveat rather than an R5 blocker.

### Implementation Mapping

- Typed mission parsing and copied-payload validation: `control-plane/crates/engine/src/agent_mission_context.rs`.
- Dynamic, P017, and P058 orchestration: `control-plane/crates/engine/src/orchestrator.rs`.
- P058 deadline/chain resume: `control-plane/crates/engine/src/p058_deadline_resume.rs`.
- Transactional materialization/stage writes: `control-plane/crates/db/src/repos/dynamic_materialization.rs`, `control-plane/crates/db/src/repos/stages.rs`.
- Frozen catalog contract and descriptor-first skill loading: `control-plane/crates/workflow/src/compiler.rs`, `control-plane/crates/workflow/src/skill_bundle.rs`.
- Producer/proof inventory and canonical gate: `control-plane/crates/engine/tests/agent_context_skills.rs`, its fixtures, and `scripts/test-gate.sh`.

## Proposal Fidelity / Divergence

### Matches

- Producer discovery recursively reads production `control-plane/crates/engine/src/**/*.rs`; removing any registered guard fails, and unknown producers in another existing module and a new nested module fail.
- The manifest still contains all nine current producers, including P058 deadline/resume.
- Dynamic reviewers are fully finalized into an in-memory prepared set before any materialization/work write.
- Materialization records and provider work are committed in one transaction; a second-work failure rolls the whole set back and replay succeeds.
- Prompt-finalization failure writes typed evidence, Stage `Blocked`, and Run `Blocked` in one transaction; a last-write failpoint rolls all three back and replay succeeds.
- Production `advance_run` missing-Idea uses the same typed atomic blocked settlement.
- Persisted mission size is checked at 24 KiB before JSON deserialization; exact-limit passes and plus-one fails.
- Persisted task/owner assignment, phase/parallel, output partition, consumers, transition conditions, Run/Idea identity, and title/body are compared with frozen/durable truth without rewriting valid prompt bytes.
- Earlier P058 complete lead-authority replacement, P017 frozen lead selection/finalizer ordering, absent/V1 external-skill rejection, snapshot quartet, and descriptor-first skill loading remain green.

### Divergences

- Mediation copy validation accepts any `lead_agent_id` that `frozen_agent` can find anywhere in the plan, rather than the exact frozen system lead for the P017/P058 assignment.
- `lead_resolution` and `conflict_or_escalation_id` need only be non-empty and match self-reported payload fields; they are not compared with the frozen lead contract or durable mediation/escalation identity.
- The canonical mutation suite exercises task Run/Idea/consumer substitutions but has no coordinated P017/P058 mediation authority substitution.

### Ambiguities / Evidence Gaps

- None for the remaining finding: the validator branches and omitted mediation mutations are directly observable in source.
- P060 selected-set binding admission is intentionally not assessed as minimal-V1 conformance under the user's final scope instruction.

## R4 Finding Disposition

| R4 finding | R5 disposition |
|---|---|
| `ARCH-001` source-tree producer closure | **Closed**: recursive source discovery, exact manifest, other-existing-module and new-module mutations pass in the canonical gate |
| `REL-001` dynamic all-or-nothing settlement | **Closed for the minimal V1 contract**: late finalizer, success transaction, blocked transaction, production missing-Idea, failure injection, and replay proofs pass |
| `REL-002` persisted V1 bound and durable truth | **Partially closed**: pre-parse bound and task/owner durable truth are closed; exact mediation assignment truth remains `SEC-001` below |

All earlier R3 P058/P017/frozen-skill findings remain closed on the exact target.

## Residual Scope / Follow-up Ownership

| Residual | Owner | In scope? | Blocks? |
|---|---|---|---|
| Production-hardening backlog | Deferred proposal document | No | No |
| Rollout sidecar and full-surface fixtures | Deferred production-hardening scope | No | No |
| Security/pre-push skill conversion | Separate merged security/pre-push proposal | No | No |
| P060 selection-plan binding admission beyond actual V1 enqueue/finalization | Existing P060 routing contract/reference | No | No |
| Exact persisted P017/P058 mediation assignment validation | This minimal proposal, lines 247-273 and acceptance 3/4/6/7 | Yes | **Yes** |

## Specialist Coverage Matrix

| Surface | Required lens | Completed | Coverage blocker |
|---|---|---|---|
| Producer/finalizer ownership | Rust architecture | Yes | No |
| Dynamic/retry/failure lifecycle | Rust reliability | Yes | No |
| Permission projection and parser bounds | Rust security | Yes | No |
| Absent/V1/V2 and persisted V1 contract | API/frozen contract | Yes | No |
| Mission/payload/work-item/frozen-plan parity | Chainworks execution truth | Yes | No |

## Requirement Summary

| Status | Count |
|---|---:|
| Implemented | 15 |
| Partially Implemented | 5 |
| Missing | 0 |
| Not Verifiable | 0 |
| Out of Scope | 0 |

## Requirement Audit

| ID | Requirement | Proposal source | Status | Current evidence / gap |
|---|---|---|---|---|
| REQ-001 | Bounded standalone proposal without deferred normative dependency | Lines 590-591 | Implemented | 645 lines; deferred files remain explicit non-goals |
| REQ-002 | Complete absent/V1/V2 frozen snapshot matrix and legacy behavior | Lines 290-306, 592-593 | Implemented | Quartet/version and unauthenticated external-skill rejection tests pass |
| REQ-003 | One deterministic V1 mission on every fresh route; validated exact copy retry/resume | Lines 259-273, 594-595 | Partially Implemented | Every current producer is guarded, but mediation copies can preserve bytes that no longer match the exact frozen/durable assignment |
| REQ-004 | Mission/assignment/consumer/completion derive only from durable Run/Idea and frozen plan | Lines 236-273, 596-597 | Partially Implemented | Task/owner truth is exact; mediation lead/contract/identity are only self-consistent, not exact |
| REQ-005 | Exact bounds and zero Run/provider work; dynamic failures durably block | Lines 275-288, 598-599 | Implemented | Exact/plus-one parser proof, late-reviewer zero-work, atomic success/failure, missing-Idea, failpoint, and replay tests pass |
| REQ-006 | Descriptive mission mirrors existing permission/output authority | Lines 215-234, 600-601 | Partially Implemented | Fresh and task-copy paths mirror authority; a mediation copy can substitute another frozen agent/contract |
| REQ-007 | Closed assignment/consumer grammar and exact prompt order | Lines 236-273, 602-603 | Partially Implemented | Grammar/order and consumers are exact, but mediation assignment identity is not exact on copy validation |
| REQ-008 | Mandatory default-on activation with no disable path | Lines 202-205, 532-547, 604 | Implemented | No runtime disable or legacy fallback for fresh V2 Runs found |
| REQ-009 | Convert exactly the two minimal-slice bindings | Lines 446-464, 605-606 | Implemented | Minimal two conversions remain; later security/pre-push conversions are separately owned |
| REQ-010 | Descriptor-relative no-follow bounded bundle loading | Lines 397-442, 607-608 | Implemented | Runtime and canonical loader tests pass |
| REQ-011 | Frontmatter metadata only and reject `allowed-tools` | Lines 397-412, 609 | Implemented | Strict loader tests pass |
| REQ-012 | Exact V2 extension and total procedure identity | Lines 151-205, 610-612 | Implemented | Compiler/frozen tests pass |
| REQ-013 | Authenticate stored snapshots and never read changed live bytes | Lines 290-306, 435-437, 613-614 | Implemented | Both hashes checked before parse; absent/V1/V2 drift cases fail closed |
| REQ-014 | Preserve affected permission profiles/output contracts | Lines 455-461, 615-616 | Implemented | Catalog parity and fresh P058 lead authority tests pass |
| REQ-015 | Preserve unrelated skill procedure bytes | Lines 463-464, 617 | Implemented | Focused parity tests pass; separate follow-up is excluded |
| REQ-016 | Remove affected prompt duplication | Lines 455-464, 618 | Implemented | Focused procedure/prompt tests pass |
| REQ-017 | Execute deterministic cases and mutation negatives | Lines 466-482, 619 | Implemented | Canonical CTX-001 through CTX-008 corpus tests pass |
| REQ-018 | Provider-free gate executes all 12 proof clauses | Lines 484-515, 620 | Partially Implemented | Gate is green/provider-free, but clauses 3/11 lack an exact coordinated mediation-copy authority mutation |
| REQ-019 | No dedicated validation Run | Lines 517-528, 621 | Implemented | Exact-tree proof used no provider or validation Run |
| REQ-020 | Do not edit deferred artifacts for this slice | Lines 570-586, 622 | Implemented | Deferred artifacts remain non-normative and unchanged by the R5 delta |

## Reviewer Scorecard

| Lens | Assessment | Top risk | Confidence |
|---|---|---|---|
| Conformance | Partial | Five requirements remain partial around one mediation copy-validation defect | High |
| Rust architecture | Pass | Recursive producer ownership is closed | High |
| Rust reliability | Pass for minimal scope | Required dynamic transaction/replay paths are closed | High |
| Rust security | Fail | Copied mediation authority can be coherently substituted | High |
| API / frozen contract | Fail | Mediation assignment validation proves membership, not exact identity | High |
| Chainworks execution truth | Fail | Durable P017/P058 identity is not bound into copied validation | High |
| Readiness | Not Ready | One unresolved Major in the minimal contract | High |

## Security-Sensitive Diff Scan

The helper triggered parser, resource-limit, filesystem, auth/permission, public-ingress, and redaction categories. Manual review narrowed the actual changed security surfaces to persisted JSON parsing, the 24 KiB boundary, frozen permission/procedure projection, and provider work creation. The pre-parse bound and task/owner projection are closed. `SEC-001` remains because mediation copy validation treats frozen-plan membership and self-consistency as authority instead of proving the exact frozen and durable mediation assignment.

## Findings

### SEC-001 Major: Mediation copy validation accepts coordinated substitution of frozen authority

- **Reviewers:** Rust security, API/frozen contract, Chainworks execution truth
- **Confidence:** High
- **Related requirements:** REQ-003, REQ-004, REQ-006, REQ-007, REQ-018; proposal lines 247-273; acceptance 3, 4, 6, 7
- **Evidence:** `agent_mission_context.rs:487-506` accepts mediation when `origin` is recognized, IDs/contracts are non-empty, and `frozen_agent(plan, lead_agent_id)` finds any frozen agent. `agent_mission_context.rs:597-616` then requires the payload to repeat those self-asserted values. `agent_mission_context.rs:685-690` deliberately excludes mediation from the normal frozen `output_contract` comparison. `agent_mission_context.rs:522-538` binds only Run/Idea identity and text. The mutation suite at `agent_context_skills.rs:1833-1975` covers task authority, Run/Idea, and consumers but no P017/P058 mediation substitution.
- **Contradiction:** A copied mediation prompt and payload can replace `lead_agent_id` plus all agent authority fields with another valid frozen agent, choose an arbitrary non-empty `lead_resolution`, and replace `conflict_or_escalation_id`; the current validator accepts the coordinated values although they did not derive from the frozen system lead or durable mediation/escalation record. This is the explicit minimal V1 mediation and copy-validation contract, not deferred P058 hardening.
- **Required action:** Make copied mediation validation variant-aware and pass the already available durable expected identity into it. P017 must bind the unique frozen system lead, its frozen `lead_resolution_contract`, current stage, conflict ID, and mediation relation. P058 must bind the frozen lead/tier contract, stage, and ledger/escalation ID. Keep successful copy behavior byte-preserving.
- **Minimal acceptance:** Add one table-driven canonical-gate test using real P017 and P058 finalized payloads. For each arm, coherently substitute another existing frozen agent, its payload authority fields, a different non-empty resolution contract, and a different conflict/ledger ID; every mutation must fail before any retry/work/state write. The unchanged payload must validate and retain byte-identical prompt bytes.

## Readiness Checklist

| Check | Result |
|---|---|
| Exact commit/proposal identity | Pass |
| Clean detached audit tree | Pass |
| Canonical `agent-context-skills` gate | Pass: 57 passed, 1 intentional ignore, 0 failed |
| Requested scoped syntax/rustfmt/diff checks | Pass |
| Recursive source-tree producer closure | Pass |
| Dynamic complete preparation and transactional success/failure | Pass for minimal V1 scope |
| Production missing-Idea and failure replay | Pass |
| Persisted 24 KiB pre-parse bound | Pass |
| Run/Idea/task/owner durable truth validation | Pass |
| Exact P017/P058 mediation copy authority | **Fail** |
| Earlier P058/P017/frozen-skill regressions | None found; canonical proofs pass |
| Required specialist coverage | Pass |
| Unresolved Critical/Major findings | **Present: SEC-001** |

## Verification Log

1. Verified exact full commit, `main` containment, parent commit, proposal MD5, line count, and absent R5 path.
2. Created a clean detached worktree at `b56dcc1ec76a58fa90876d189f9f20dc357f18e5` and confirmed an empty status.
3. Ran the canonical gate successfully: 57 passed, 1 explicit fixture-regeneration ignore, 0 failed.
4. Confirmed the gate executes P058 deadline/resume 8/8, seven engine `agent_context_` tests, P058 complete lead-authority replacement, and frozen skill/snapshot compatibility suites.
5. Ran `bash -n scripts/test-gate.sh`, scoped `rustfmt --check`, and `git diff --check dbd66bbd..b56dcc1e` successfully.
6. Ran the implementation-surface and security-sensitive diff helpers, then manually narrowed their broad lexical triggers to the proposal's actual Rust boundaries.
7. Traced recursive producer discovery and its guard-removal, existing-module, and new-module mutations.
8. Traced dynamic preparation, atomic marker/work transaction, atomic blocked settlement, missing-Idea production route, failpoints, and replay.
9. Traced persisted parser ordering, Run/Idea/task/owner/consumer checks, all four copy producers, and the mediation branch's omitted exact authority bindings.
10. Rechecked earlier P058, P017, absent/V1/V2 frozen-skill, snapshot-hash, and descriptor-first findings against current code and executed tests.

## Final Verdict

- **Overall Conformance: Partial.** Fifteen requirements are implemented; five remain partially implemented around one confirmed Major mediation copy-validation defect.
- **Overall Implementation Readiness: Not Ready.** Same-tree canonical evidence is green, but `SEC-001` permits copied mediation authority to diverge from exact frozen/durable truth.

## Required Next Action

Bind copied P017/P058 mediation validation to the exact durable mediation/escalation identity and frozen lead/contract, add the single table-driven coordinated-substitution proof to the canonical gate, and request R6 against one exact commit.
