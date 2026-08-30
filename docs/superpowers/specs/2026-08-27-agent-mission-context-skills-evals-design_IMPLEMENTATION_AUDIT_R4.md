# Agent Mission Context and Skills: Default-On Minimal Slice Implementation Audit R4

| Field | Value |
|---|---|
| Proposal | `docs/superpowers/specs/2026-08-27-agent-mission-context-skills-evals-design.md` |
| Proposal MD5 | `77e16559742d490e70ec5b21cbc963ba` |
| Implementation Target | `main` commit `dbd66bbdd0fb25ec7e84aea7b25439eabe0b0755` |
| Compare Base | R3 target `3e125db06474b2d49e01898d02098eba70256d2d` |
| Audit Tree | Clean detached worktree at the exact implementation target; dirty main-worktree files excluded from evidence |
| Mode | `implementation-readiness` |
| Proposal State | Active |
| Platform / Product Scope | Rust control-plane compiler, prompt assembly, work-item lifecycle, and provider-free proof gate |
| Overall Conformance | **Partial** |
| Overall Implementation Readiness | **Not Ready** |
| Reviewer Selection Reuse | **Reused with R3 delta**: architecture, reliability, security, API/frozen-contract, and Chainworks execution-truth lenses |
| Audit Confidence | **High** |

## Executive Verdict

R4 remains fail-closed at `Partial / Not Ready`. The exact-tree canonical gate is green and materially stronger than R3: 53 tests passed, one explicit fixture-regeneration test was ignored, P058 deadline/resume passed 8/8, and the P058 lead-authority replacement proof passed. Most R3 runtime findings are closed.

Three in-scope blocker groups remain:

1. the producer inventory is not source-tree closed because it scans only three hard-coded Rust files;
2. dynamic fan-out can leave earlier reviewer work/materialization committed when a later reviewer fails, and its typed evidence/stage/Run blocking is not one atomic settlement; the production missing-Idea path also bypasses the new settlement helper;
3. persisted V1 validation is typed but still unbounded before parse and not bound to complete durable mission/consumer truth.

These gaps directly contradict the minimal proposal's closed-producer, zero-provider-work-on-finalizer-failure, bounded-validation, and frozen-truth requirements. A green focused gate cannot override that its own clause 11 source closure and dynamic failure proof are incomplete.

## Prior Review Reuse

- R3 architecture, reliability, security, API-contract, and execution-truth selections remain the smallest sufficient set for the same Rust surfaces.
- Product, rollout/observability, performance, and Apple UI/UX remain excluded because the proposal explicitly adds no experiment, rollout, telemetry, API, UI, or live-provider behavior.
- R3 is context only; every closure below was rechecked against current `dbd66bbd` bytes and exact-tree test output.

## Reviewer Routing

### Selected Reviewers

| Reviewer | Trigger | Result | Confidence |
|---|---|---|---|
| Rust architecture | Producer ownership and source-tree closure | Fail | High |
| Rust reliability | Dynamic fan-out, failure settlement, retry/resume | Fail | High |
| Rust security | Bounded persisted parsing and permission/filesystem boundaries | Fail | High |
| API / frozen contract | Typed V1 and absent/V1/V2 compatibility | Fail | High |
| Chainworks execution truth | Mission, payload, stage, work-item, and frozen-plan parity | Completed locally; fail | High |

### Rejected Close Alternatives

| Reviewer | Reason rejected |
|---|---|
| Product | Live A/B, metrics, causal model proof, and rollout decisions are non-goals |
| Observability / rollout | No migration, flag, telemetry, alert, release receipt, or production rollout belongs to this proposal |
| Performance | No latency/throughput claim; resource bounds are covered by security/reliability |
| Apple UI / UX | No Swift, screen, navigation, accessibility, or operator-interaction change is in scope |

## Proposal Contract

### Scope

- Add one mandatory typed `AgentMissionContextV1` to every fresh V1 `InvokeAgent` prompt.
- Preserve exact persisted prompt bytes for copy retry/resume after bounded, typed validation.
- Derive mission, assignment, consumers, completion, permission, and output ownership only from durable Run/Idea and frozen-plan truth.
- Compile catalog snapshot V2 after bounded external-skill resolution and preserve authenticated absent/V1/V2 behavior.
- Convert the two minimal bindings to descriptor-relative single-file Agent Skills bundles.
- Prove all commitments through one provider-free 12-clause canonical gate.

### Primary Service Flows

1. Preflight Idea/context bounds, compile V2, and insert a Run only after frozen inputs are valid.
2. Finalize one mission block for every static, post-approval, dynamic, owner-only, P017, and P058 provider enqueue.
3. Copy retry/resume exact persisted V1 prompt bytes only after validating their complete durable contract.
4. Resume absent/V1/V2 frozen Runs without live workflow/catalog/skill fallback.
5. Fail dynamic/P017 finalization with zero provider work and coherent durable state.

### Explicit Exclusions

The production-hardening backlog, rollout sidecar, and full-surface fixtures are deferred and non-normative. The merged security/pre-push proposal is a separately owned follow-up. None blocks this audit, and none is used to excuse an in-scope minimal-flow gap.

## Evidence Pack

### Identity and Provenance

- `dbd66bbd` resolved to `dbd66bbdd0fb25ec7e84aea7b25439eabe0b0755`, is contained by `main`, and has parent `3e125db06474b2d49e01898d02098eba70256d2d`.
- Proposal MD5 is `77e16559742d490e70ec5b21cbc963ba`; proposal length is 645 lines.
- All code and test evidence came from a clean detached worktree at the exact commit.
- The dirty main worktree was not used as implementation evidence and was not modified except for this single R4 report.

### Requested Validation

| Command | Result | Evidence meaning |
|---|---|---|
| `./scripts/test-gate.sh agent-context-skills` | **PASS**: 53 passed, 1 explicitly ignored fixture-regeneration test, 0 failed | Same-tree canonical gate is green; P058 deadline/resume is included and passed 8/8 |
| `bash -n scripts/test-gate.sh` | **PASS** | Gate script syntax is valid |
| `git diff --check 3e125db0..dbd66bbd` | **PASS** | R4 delta has no whitespace-error evidence |

The initial sandboxed gate attempt failed only because the shared managed Cargo cache was not accessible. The exact command was immediately rerun with approved cache access and completed successfully; the sandbox failure is not a product/test failure.

### Implementation Mapping

- Typed mission parsing and copied-payload validation: `control-plane/crates/engine/src/agent_mission_context.rs`.
- Dynamic, P017, and P058 orchestration: `control-plane/crates/engine/src/orchestrator.rs`.
- P058 deadline/chain resume: `control-plane/crates/engine/src/p058_deadline_resume.rs`.
- Frozen catalog contract: `control-plane/crates/workflow/src/compiler.rs`.
- Descriptor-first skill loading: `control-plane/crates/workflow/src/skill_bundle.rs`.
- Producer/proof inventory and canonical gate: `control-plane/crates/engine/tests/agent_context_skills.rs`, its fixtures, and `scripts/test-gate.sh`.

## Proposal Fidelity / Divergence

### Matches

- P058 deadline/resume now validates copied V1 payloads before mutation or write.
- P058 lead mediation replaces provider/model/permission/skill/MCP/worktree/session authority from the frozen lead and validates it before transaction/work insertion.
- P017 selects the unique system lead from the authenticated frozen catalog/plan; the live resolver path is gone.
- P017 builds/finalizes the complete work item before atomically inserting mediation, conflict pointer, and provider work.
- Absent/V1 frozen catalogs reject external skills without authenticated embedded bytes.
- The loader opens the original catalog path descriptor-first with `O_NOFOLLOW`; parent canonicalization was removed.
- P058 snapshot fixtures use a valid quartet, the suite is in the canonical gate, and 8/8 cases pass.

### Divergences

- Producer closure is limited to `orchestrator.rs`, `command_handler.rs`, and `p058_deadline_resume.rs`, not all production `engine/src/**/*.rs` files.
- Dynamic materialization inserts a durable marker and queues each reviewer inside the loop; a later finalizer failure cannot undo earlier iterations.
- Dynamic failure evidence, stage `Blocked`, and Run `Blocked` are three separately committed writes.
- Production resolves a dynamic Idea before entering the helper that performs typed durable settlement, so a missing row still escapes with the stage already Running.
- Persisted mission JSON is parsed without the declared 24 KiB pre-parse bound.
- Persisted validation ignores Idea ID/title/body and does not compare complete consumers, transition conditions, or dynamic assignment fields with durable/frozen truth.

### Ambiguities / Evidence Gaps

- No multi-reviewer fixture makes reviewer 1 valid and reviewer 2 invalid, then proves zero materialization/work across the whole fan-out.
- No failure-injection test interrupts each write in dynamic blocked settlement and verifies atomic replay.
- No production-path missing-Idea test enters through `advance_run` and checks typed stage/Run settlement.
- No exact-limit/plus-one persisted mission parser test runs before deserialization.
- No coordinated prompt-plus-payload mutation test changes Run/Idea/consumer truth and proves rejection.
- No mutation adds an unregistered producer to an arbitrary different or newly added engine source file.

## R3 Finding Disposition

| R3 finding | R4 disposition |
|---|---|
| `SEC-001` P058 lead authority mismatch | **Closed** |
| `ARCH-001` P058 copy bypass and producer inventory | **Partially closed**: P058 guard/site fixed; source-tree closure remains `ARCH-001` below |
| `REL-001` weak persisted V1 validation | **Partially closed**: closed types and payload authority checks added; complete durable identity and pre-parse bound remain `REL-002` |
| `REL-002` dynamic failure settlement | **Partially closed**: typed writes added; production missing-Idea, fan-out atomicity, and crash consistency remain `REL-001` |
| `API-001` legacy frozen external-skill live read | **Closed** |
| `SEC-002` catalog-parent symlink traversal | **Closed for runtime path** |
| `ARCH-002` mutable P017 lead selection | **Closed** |
| `REL-003` inconsistent P017 finalizer failure | **Closed for the audited finalizer path** |
| `READY-001` omitted/failing P058 suite | **Closed**: suite is included and passed 8/8 |

## Residual Scope / Follow-up Ownership

| Residual | Owner | In scope? | Blocks? |
|---|---|---|---|
| Production-hardening backlog | Deferred proposal document | No | No |
| Rollout sidecar and full-surface fixtures | Deferred production-hardening scope | No | No |
| Security/pre-push skill conversion | Separate merged security/pre-push proposal | No | No |
| Source-tree producer closure | This minimal proposal, gate clause 11 | Yes | **Yes** |
| Dynamic zero-work and atomic blocked settlement | This minimal proposal, lines 284-288 and gate clause 3 | Yes | **Yes** |
| Bounded, durable-truth-complete persisted V1 validation | This minimal proposal, copy-validation and acceptance 3/4/7 | Yes | **Yes** |

## Specialist Coverage Matrix

| Surface | Required lens | Completed | Coverage blocker |
|---|---|---|---|
| Producer/finalizer ownership | Rust architecture | Yes | No |
| Dynamic/retry/failure lifecycle | Rust reliability | Yes | No |
| Permission projection, parser bounds, filesystem traversal | Rust security | Yes | No |
| Absent/V1/V2 and persisted V1 contract | API/frozen contract | Yes | No |
| Mission/payload/work-item/frozen-plan parity | Chainworks execution truth | Yes, primary audit trace | No |

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
| REQ-002 | Complete absent/V1/V2 frozen snapshot matrix and legacy behavior | Lines 290-306, 592-593 | Implemented | Quartet/version checks pass; unauthenticated absent/V1 external skill now fails closed |
| REQ-003 | One deterministic V1 mission on every fresh route; validated exact copy retry/resume | Lines 274-288, 594-595 | Partially Implemented | Current P058 copy site is fixed, but global producer proof and complete persisted validation remain open |
| REQ-004 | Mission/assignment/consumer/completion derive only from durable Run/Idea and frozen plan | Lines 250-288, 596-597 | Partially Implemented | Fresh finalization is frozen; copied validation ignores Idea fields and does not compare complete consumers/dynamic assignment truth |
| REQ-005 | Exact bounds and zero Run/provider work; dynamic failures durably block | Lines 279-288, 598-599 | Partially Implemented | StartRun is closed; dynamic late failure can leave earlier work, missing Idea bypasses settlement, and blocked writes are non-atomic |
| REQ-006 | Descriptive mission mirrors existing permission/output authority | Lines 231-248, 600-601 | Implemented | P058 frozen-lead authority replacement and validation pass |
| REQ-007 | Closed assignment/consumer grammar and exact prompt order | Lines 250-288, 602-603 | Partially Implemented | Fresh paths pass, but persisted consumer/dynamic assignment validation is not exact |
| REQ-008 | Mandatory default-on activation with no disable path | Lines 226-229, 604 | Implemented | No runtime disable path found |
| REQ-009 | Convert exactly the two minimal-slice bindings | Lines 446-464, 605-606 | Implemented | Original two conversions remain; security/pre-push conversions are separately owned |
| REQ-010 | Descriptor-relative no-follow bounded bundle loading | Lines 397-442, 607-608 | Implemented | Runtime opens catalog parent/components with descriptors and `O_NOFOLLOW`; canonical gate loader tests pass |
| REQ-011 | Frontmatter metadata only and reject `allowed-tools` | Lines 397-412, 609 | Implemented | Strict loader tests pass |
| REQ-012 | Exact V2 extension and total procedure identity | Lines 188-224, 610-612 | Implemented | Compiler/frozen tests pass |
| REQ-013 | Authenticate stored snapshots and never read changed live bytes | Lines 290-306, 435-437, 613-614 | Implemented | Both hashes checked before parse; absent/V1 external and V2 source-drift cases fail closed |
| REQ-014 | Preserve affected permission profiles/output contracts | Lines 455-461, 615-616 | Implemented | Catalog parity and P058 runtime authority tests pass |
| REQ-015 | Preserve unrelated skill procedure bytes | Lines 463-464, 617 | Implemented | Focused parity tests pass; separate follow-up is excluded |
| REQ-016 | Remove affected prompt duplication | Lines 455-464, 618 | Implemented | Focused procedure/prompt tests pass |
| REQ-017 | Execute deterministic cases and mutation negatives | Lines 466-482, 619 | Implemented | Canonical corpus tests pass |
| REQ-018 | Provider-free gate executes all 12 proof clauses | Lines 484-515, 620 | Partially Implemented | Gate is green/provider-free and includes P058, but clause 11 is not source-tree closed and dynamic failure proof misses late partial dispatch |
| REQ-019 | No dedicated validation Run | Lines 517-528, 621 | Implemented | Exact-tree proof used no provider or validation Run |
| REQ-020 | Do not edit deferred artifacts for this slice | Lines 570-586, 622 | Implemented | Deferred artifacts remain non-normative and outside this commit's acceptance |

## Reviewer Scorecard

| Lens | Assessment | Top risk | Confidence |
|---|---|---|---|
| Conformance | Partial | Five requirements remain partial | High |
| Rust architecture | Fail | Hard-coded three-file producer scan is not closed | High |
| Rust reliability | Fail | Dynamic fan-out and blocked settlement are not all-or-nothing | High |
| Rust security | Fail | Persisted V1 parser is unbounded and incompletely bound | High |
| API / frozen contract | Fail | Copied mission truth is not fully canonical/durable-bound | High |
| Chainworks execution truth | Fail | Dynamic failure can coexist with queued provider work | High |
| Readiness | Not Ready | Three Major blocker groups despite green gate | High |

## Security-Sensitive Diff Scan

The helper reported no dirty-file trigger because the audit target was a clean commit. Manual diff review still triggered the mandatory security lens: JSON deserialization, a 24 KiB parser boundary, permission/MCP/worktree/session projection, filesystem descriptors, symlinks, and provider work-item creation all changed. The independent security review confirms that P058 authority and descriptor-first traversal are closed, but the unbounded/incomplete persisted validator remains an open Major finding.

## Findings

### ARCH-001 Major: Producer inventory is not source-tree closed

- **Reviewer:** Rust architecture / execution truth
- **Confidence:** High
- **Related requirements:** REQ-003, REQ-018; acceptance 3; gate clause 11
- **Evidence:** `agent_context_skills.rs:1411-1427` loads only `orchestrator.rs`, `command_handler.rs`, and `p058_deadline_resume.rs`. The unknown-producer mutation is injected only into `orchestrator.rs` at `agent_context_skills.rs:1448-1454`.
- **Why it matters:** The current P058 producer is registered, but an unregistered producer in any other existing or new production engine module leaves the canonical gate green. The claimed closed source inventory is therefore not structurally true.
- **Required action:** Recursively discover all production `control-plane/crates/engine/src/**/*.rs` files, or centralize all `InvokeAgent` construction behind one enforced registration point, and require an exact producer-to-manifest bijection.
- **Acceptance:** Adding an unregistered producer to a different existing module and to a newly added module fails clause 11; deleting any registered guard still fails; the exact current manifest continues to include P058 deadline/resume.

### REL-001 Major: Dynamic failure is not an all-or-nothing zero-provider-work settlement

- **Reviewer:** Rust reliability / execution truth
- **Confidence:** High
- **Related requirements:** REQ-005, REQ-018; proposal lines 284-288; gate clause 3
- **Evidence:** `orchestrator.rs:5269-5435` finalizes, inserts a materialization marker, and enqueues each selected reviewer sequentially. A later failure at `orchestrator.rs:5340-5345` returns after earlier iterations have committed. `orchestrator.rs:5466-5468` writes failure evidence, stage status, and Run status through three separate repository transactions. Production missing-Idea resolution at `orchestrator.rs:1651-1660` fails before the helper that performs this settlement.
- **Why it matters:** A stage can be marked Blocked while provider work from earlier reviewers remains pending, or a crash can leave only part of the blocked state. A missing Idea can still leave the already-Running stage without typed evidence.
- **Required action:** Pre-finalize the complete selected fan-out before the first durable materialization/work write; commit materialization/work atomically on success and evidence/stage/Run atomically on failure. Route production missing-Idea through the same settlement.
- **Acceptance:** A two-reviewer fixture with reviewer 1 valid and reviewer 2 invalid leaves zero materialization and zero provider work; production `advance_run` missing-Idea and exact/plus-one cases produce one atomic typed Blocked state; fault injection after every write plus replay cannot produce marker-without-work, queued-work-with-blocked-stage, or split evidence/status.

### REL-002 Major: Persisted V1 validation is unbounded and not bound to complete durable truth

- **Reviewer:** Rust reliability / security / API contract
- **Confidence:** High
- **Related requirements:** REQ-003, REQ-004, REQ-007; acceptance 3, 4, 7
- **Evidence:** `agent_mission_context.rs:314-337` deserializes the extracted mission without enforcing `MAX_MISSION_CONTEXT_BYTES`. `agent_mission_context.rs:347-351` explicitly ignores Idea ID/title/body. Dynamic phase/parallel and exact consumers are not rederived; task consumers require only non-empty strings at `agent_mission_context.rs:463-470`, and transition conditions are not compared with the frozen expression. Existing mutation coverage at `agent_context_skills.rs:1739-1823` does not exercise those coordinated identity/consumer changes or plus-one parser input.
- **Why it matters:** A copied prompt can remain structurally typed while carrying altered mission or consumer truth, and an oversized persisted mission reaches JSON deserialization despite the proposal's 24 KiB boundary.
- **Required action:** Enforce the byte bound before deserialization and validate/canonically compare the complete mission, assignment, consumers, completion, and runtime projection against the actual Run/Idea and frozen plan without rewriting valid persisted bytes.
- **Acceptance:** Exact-limit passes and plus-one fails before parse/write; reordered/extra/missing/wrong-type fields, Idea/run identity changes, dynamic phase/parallel changes, consumer/order/condition changes, and coordinated prompt-plus-payload mutations all fail with zero state/work mutation; valid persisted bytes remain byte-identical.

## Readiness Checklist

| Check | Result |
|---|---|
| Exact commit/proposal identity | Pass |
| Clean detached audit tree | Pass |
| Canonical `agent-context-skills` gate | Pass: 53 passed, 1 intentional ignore, 0 failed |
| P058 deadline/resume suite in canonical gate | Pass: 8/8 |
| P058 complete lead authority replacement | Pass |
| Frozen-only P017 lead selection/finalizer ordering | Pass |
| Absent/V1 external-skill rejection | Pass |
| Descriptor-first catalog-parent traversal | Pass |
| Source-tree-wide producer closure | **Fail** |
| Dynamic zero-work and atomic blocked settlement | **Fail** |
| Bounded durable-truth-complete persisted validation | **Fail** |
| Required specialist coverage | Pass |
| Unresolved Critical/Major findings | **Present** |

## Verification Log

1. Verified exact full commit, `main` containment, parent commit, proposal MD5, line count, and absent R4 path.
2. Created a clean detached worktree at `dbd66bbdd0fb25ec7e84aea7b25439eabe0b0755`.
3. Ran the canonical gate successfully: 53 passed, 1 explicit fixture-regeneration ignore, 0 failed.
4. Confirmed the canonical gate executes `proposal_058_deadline_resume`; all 8 tests passed.
5. Ran `bash -n scripts/test-gate.sh` successfully.
6. Ran scoped `git diff --check 3e125db0..dbd66bbd` successfully.
7. Traced every current production `WorkItemKind::InvokeAgent` constructor and compared it with the manifest and scan scope.
8. Traced dynamic finalization, materialization, enqueue, blocked settlement, and production Idea lookup ordering.
9. Traced persisted mission parse bounds and field-by-field frozen/durable validation.
10. Rechecked all R3 findings with architecture, reliability, security, API/frozen-contract, and execution-truth lenses.

## Final Verdict

- **Overall Conformance: Partial.** Fifteen requirements are implemented; five remain partially implemented.
- **Overall Implementation Readiness: Not Ready.** Same-tree canonical evidence is green, but three confirmed Major code-owned blocker groups remain in the minimal proposal itself.

## Required Next Actions

1. Make producer discovery source-tree closed and mutation-sensitive outside the three known modules.
2. Make dynamic fan-out prevalidated and transactional, including production missing-Idea and crash/replay cases.
3. Complete bounded canonical persisted V1 validation against actual Run/Idea and frozen assignment/consumer truth.
4. Request R5 against one exact commit after these cases are part of, and pass in, the canonical gate.
