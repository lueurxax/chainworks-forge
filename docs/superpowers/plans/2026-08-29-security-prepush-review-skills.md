# Security And Pre-Push Review Skills Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Convert the security and pre-push reviewers to frozen local Agent Skills bundles while preserving every existing authority and proving both workflow input shapes in the provider-free gate.

**Architecture:** Reuse the existing strict single-file bundle compiler and `AgentMissionContextV1` finalizer without runtime changes. Extend the focused workflow/engine fixtures with complete before-state parity, active-workflow prompt cases, mutation-aware claim scoring, and pre-migration V2 compatibility. Migrate only two catalog skill definitions and remove only their duplicated procedure prose.

**Tech Stack:** Rust, serde YAML/JSON, shell gate checks, local Agent Skills `SKILL.md` files.

**Spec:** `docs/superpowers/specs/2026-08-29-security-prepush-review-skills-design.md` at reviewed commit `465fa72a880333347fbc0988f788f0f82d8b2523` / MD5 `6d4468ad59b4987babc316dadfa96cb3`.

---

## Global Constraints

- Work only in `codex/security-prepush-skills-spec` until verification completes.
- Do not modify database, GraphQL, MCP, ACP, SwiftUI, workflow topology, profiles, permission rules, models, tools, inputs, outputs, or settlement code.
- Do not modify the three existing external bundle files.
- No remote, Xcode, daemon, or live-provider verification before the final new run.
- Follow RED -> GREEN for every behavior change.

## File Map

- Create `examples/agents/skills/security-review/SKILL.md`: reusable security procedure.
- Create `examples/agents/skills/prepush-review/SKILL.md`: reusable final review procedure.
- Modify `examples/agents/agents.yaml`: two external bindings and concise role prompts.
- Modify `control-plane/crates/workflow/tests/agent_context_skills.rs`: complete before-state, bundle-byte, and pre-migration snapshot proofs.
- Create `control-plane/crates/workflow/tests/fixtures/agent_context/security_prepush_before_state.json`: canonical pre-migration affected surface.
- Create `control-plane/crates/workflow/tests/fixtures/agent_context/security_prepush_catalog_v2.json`: frozen pre-migration V2 snapshot.
- Create `control-plane/crates/workflow/tests/fixtures/agent_context/security_prepush_golden_prompts.json`: exact inline prompt compatibility.
- Modify `control-plane/crates/workflow/tests/fixtures/agent_context/catalog_parity.json`: five migrated agents and new unrelated hash.
- Modify `control-plane/crates/engine/tests/agent_context_skills.rs`: V2 claim scorer and four-task compatibility test.
- Create `control-plane/crates/engine/tests/fixtures/agent_context/CTX-007.json`: active security case.
- Create `control-plane/crates/engine/tests/fixtures/agent_context/CTX-008.json`: active pre-push case.
- Modify `control-plane/crates/engine/tests/fixtures/agent_context/proof_manifest.json`: executable proof mapping.
- Modify `scripts/test-gate.sh`: exact eight-case and five-bundle preflight.
- Modify `docs/reference/test-gates.md`: current focused gate contract.

### Task 1: Complete Before-State And Existing Bundle Pins

**Files:**
- Modify: `control-plane/crates/workflow/tests/agent_context_skills.rs`
- Create: `control-plane/crates/workflow/tests/fixtures/agent_context/security_prepush_before_state.json`
- Modify: `control-plane/crates/workflow/tests/fixtures/agent_context/catalog_parity.json`

- [ ] **Step 1: Write the failing complete-parity test**

Add `security_and_prepush_migration_preserves_complete_before_state` that loads the current catalog and both workflow YAML files. The fixture must contain:

```rust
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReviewSkillBeforeState {
    inline_skill_definitions: BTreeMap<String, Value>,
    agents: BTreeMap<String, Value>,
    backend_profiles: BTreeMap<String, Value>,
    permission_profiles: BTreeMap<String, Value>,
    workflow_tasks: BTreeMap<String, Value>,
    existing_bundle_sha256: BTreeMap<String, String>,
}
```

Compare the current pre-migration values to the fixture. Define one pure `expected_after_migration` transform that changes only the two skill definitions and two prompt strings. Assert the transformed fixture equals the post-migration source.

- [ ] **Step 2: Add authority-field mutations**

For each named JSON pointer below, clone the expected current value, mutate it, and assert the exact comparator rejects it:

```text
/backend_profile
/permission_profile
/required_tools/0
/inputs/0
/outputs/0
/output_contract
/requires_human_approval
/worktree_policy
/provider
/model
/effort
/mcp
/task
/phase
/parallel
```

Pin full-file SHA-256 values for:

```text
examples/agents/skills/proposal-review-router/SKILL.md
examples/agents/skills/code-implementation/SKILL.md
examples/agents/skills/implementation-audit/SKILL.md
```

Mutate one byte in each in-memory buffer and assert its digest no longer matches the fixture.

- [ ] **Step 3: Run the test and verify RED**

Run:

```bash
cd control-plane
../scripts/cargo-managed test -p workflow --test agent_context_skills security_and_prepush_migration_preserves_complete_before_state -- --nocapture
```

Expected: FAIL because the two catalog skills are still inline and the post-migration transform does not match current source.

- [ ] **Step 4: Keep the fixture pre-migration bytes frozen**

Do not update the before-state fixture after production migration. Any expected post-migration value belongs in the deterministic transform in test code.

### Task 2: Active Review Cases And Mutation Harness V2

**Files:**
- Modify: `control-plane/crates/engine/tests/agent_context_skills.rs`
- Create: `control-plane/crates/engine/tests/fixtures/agent_context/CTX-007.json`
- Create: `control-plane/crates/engine/tests/fixtures/agent_context/CTX-008.json`
- Modify: `control-plane/crates/engine/tests/fixtures/agent_context/proof_manifest.json`

- [ ] **Step 1: Define active-case claims and mutations**

Add an active-workflow fixture shape with stable claim IDs:

```rust
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ActiveReviewContextCase {
    case_id: String,
    workflow_path: String,
    state_id: String,
    task_name: String,
    task_body: String,
    expected_inputs: Vec<String>,
    expected_output: String,
    expected_contract: String,
    expected_consumer_task: String,
    expected_consumer_agent: String,
    expected_claim_ids: BTreeSet<String>,
    negative_mutations: Vec<PromptMutation>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum PromptMutation {
    MissionJsonReplace { claim_id: String, json_pointer: String, replacement: Value },
    SystemPromptRemove { claim_id: String, needle: String },
    ProcedureRemove { claim_id: String, needle: String },
    TaskInputRemove { claim_id: String, input: String },
    TaskInputAdd { claim_id: String, input: String },
    TaskBodyRemove { claim_id: String, needle: String },
}
```

Every mutation must rebuild the `CompiledTask`, deterministic task body, and final prompt before scoring.

- [ ] **Step 2: Implement exact-set scorer tests first**

Add `ctx_007_and_008_compile_active_tasks_and_reject_each_prompt_mutation`. The scorer returns `BTreeSet<String>`. Baseline equality is exact. Each mutation must remove only its declared claim. Required claims include mission, permission, logical output, contract, input set, manifest provenance, bounded discovery, no mutation/release authority, fail-closed behavior where applicable, and next-phase consumer.

- [ ] **Step 3: Add the four-task conditional compatibility test**

Add `active_review_tasks_cover_conditional_test_evidence_branches`. Compile `full-mvp-live.yaml` and `workflow.yaml` with the active catalog, then finalize:

```text
check_implementation_security: tests_result absent; add mutation fails
prepush_review: tests_result absent; add mutation fails
review_security: tests_result present; remove mutation fails
review_before_push: tests_result present; remove mutation fails
```

Assert exact inputs, logical output, output contract, and consumer for each task.

- [ ] **Step 4: Run both tests and verify RED**

Run:

```bash
cd control-plane
../scripts/cargo-managed test -p engine --test agent_context_skills ctx_007_and_008_compile_active_tasks_and_reject_each_prompt_mutation -- --nocapture
../scripts/cargo-managed test -p engine --test agent_context_skills active_review_tasks_cover_conditional_test_evidence_branches -- --nocapture
```

Expected: FAIL because the active procedures are still inline and lack the reviewed clauses.

### Task 3: Add Two Bundles And Migrate The Catalog

**Files:**
- Create: `examples/agents/skills/security-review/SKILL.md`
- Create: `examples/agents/skills/prepush-review/SKILL.md`
- Modify: `examples/agents/agents.yaml`

- [ ] **Step 1: Add the reviewed security procedure**

Create the exact frontmatter from the spec. The body must be task-conditional for `tests_result`, require the control-plane-generated manifest, bound discovery, preserve scanner-as-evidence semantics, publish logical `security_report` under `security_report_v1`, and forbid all mutations except that declared report.

- [ ] **Step 2: Add the reviewed pre-push procedure**

Create the exact frontmatter from the spec. The body must preserve proposal scope, conditional direct-test evidence, control-plane manifest provenance, fail-closed audit/security evidence, bounded discovery, logical `prepush_review_report` under `prepush_review_v1`, and no release actions.

- [ ] **Step 3: Change only the two catalog definitions and prompts**

Use:

```yaml
security_checker_core:
  type: external_skill
  path: skills/security-review
prepush_review_core:
  type: external_skill
  path: skills/prepush-review
```

Replace each long prompt with the concise role specialization specified by the proposal. Do not touch adjacent fields.

- [ ] **Step 4: Run Task 1 and Task 2 tests to GREEN**

Run all three named tests. Expected: PASS.

### Task 4: Frozen Pre-Migration And Post-Migration Compatibility

**Files:**
- Modify: `control-plane/crates/workflow/tests/agent_context_skills.rs`
- Create: `control-plane/crates/workflow/tests/fixtures/agent_context/security_prepush_catalog_v2.json`
- Create: `control-plane/crates/workflow/tests/fixtures/agent_context/security_prepush_golden_prompts.json`

- [ ] **Step 1: Capture fixture bytes from the pre-migration commit**

Use the compiler against catalog/workflow bytes from reviewed parent commit `465fa72a880333347fbc0988f788f0f82d8b2523`, then store the resulting V2 catalog snapshot and exact finalized inline prompts. The committed tests must consume only fixture files and never invoke Git.

- [ ] **Step 2: Write the failing compatibility test**

Add `pre_migration_v2_inline_snapshot_survives_external_bundle_migration`. It loads the checked-in V2 fixture through `compile_from_snapshot_json`, makes the live catalog and new bundle directories unavailable through a temp root, finalizes both review prompts, and byte-compares them to the golden fixture.

- [ ] **Step 3: Add post-migration source removal coverage**

Compile the migrated live catalog into V2, remove/change both new bundle directories in the temp root, recompile from stored snapshot, and assert embedded bytes and hashes remain stable. Corrupt each embedded bundle/hash independently and assert failure before any engine work fixture can be created.

- [ ] **Step 4: Run compatibility tests**

Run:

```bash
cd control-plane
../scripts/cargo-managed test -p workflow --test agent_context_skills pre_migration_v2_inline_snapshot_survives_external_bundle_migration -- --nocapture
../scripts/cargo-managed test -p workflow --test agent_context_skills security_and_prepush_frozen_bundles_ignore_live_source_drift -- --nocapture
```

Expected: PASS.

### Task 5: Proof Manifest, Gate, Documentation, And Verification

**Files:**
- Modify: `control-plane/crates/engine/tests/fixtures/agent_context/proof_manifest.json`
- Modify: `scripts/test-gate.sh`
- Modify: `docs/reference/test-gates.md`

- [ ] **Step 1: Extend executable proof ownership**

Map the new named tests to the existing clauses for bundle handling, catalog parity, deterministic context mutations, frozen compatibility, and zero-work failures. Gate preflight must fail if any named test is absent.

- [ ] **Step 2: Update static exact sets**

Require exactly `CTX-001..008` and these five external bundle paths:

```text
skills/proposal-review-router
skills/code-implementation
skills/implementation-audit
skills/security-review
skills/prepush-review
```

- [ ] **Step 3: Update reference documentation**

Document eight deterministic cases, five bundles, complete parity, conditional evidence branches, and provider-free/local-only policy.

- [ ] **Step 4: Run fresh verification**

Run:

```bash
./scripts/test-gate.sh agent-context-skills
bash -n scripts/test-gate.sh
git diff --check
```

Also run the complete workflow and engine integration test files with managed Cargo. No remote or live provider is allowed here.

- [ ] **Step 5: Review the final diff against the spec**

Confirm exactly two new production bundle directories, two catalog definition/prompt changes, test/fixture/gate/reference updates, and no runtime/API/workflow mutation.
