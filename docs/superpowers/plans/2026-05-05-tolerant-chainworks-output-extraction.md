# Tolerant Chainworks Output Extraction Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make Chainworks ingest required outputs from final ACP responses even when the agent returns a close, non-canonical `CHAINWORKS_OUTPUT` shape.

**Architecture:** Keep one preferred prompt format, then make ACP transport extraction tolerant around that format. Settlement and validation remain strict: only declared current-invocation outputs are accepted.

**Tech Stack:** Rust, ACP transport, Chainworks engine prompt builder, `cargo test`.

---

### Task 1: Add ACP Regression Tests for Tolerant Final Output Extraction

**Files:**
- Modify: `control-plane/crates/acp/src/transport.rs`

- [ ] **Step 1: Add failing tests**

Add tests in the existing `#[cfg(test)] mod tests` near the current `CHAINWORKS_OUTPUT` extraction tests:

```rust
#[test]
fn json_chainworks_output_is_extracted_from_fenced_final_text_with_trailing_prose() {
    let expected_outputs = vec![ExpectedOutputSpec {
        output_name: "tests_result".to_string(),
        output_role: domain::discovery::ExpectedOutputRole::Machine,
        target_path: "/tmp/run/implementation/tests-result.json".to_string(),
        companion_of: None,
        display_label: "tests result".to_string(),
        contract_id: None,
        required: true,
        reuse_policy: domain::discovery::OutputReusePolicy::MustProduce,
        max_bytes: 1024,
        aggregate_acceptance_cap_bytes: 4096,
        authorized_roots: vec![],
        source_generation_owner: domain::discovery::SourceGenerationOwner::Agent,
    }];
    let stream = "done\n```json\n{\"CHAINWORKS_OUTPUT\":{\"tests_result\":{\"status\":\"passed\",\"commands\":[]}}}\n```\nthanks";

    let artifacts = extract_output_envelopes(stream, &expected_outputs);

    assert_eq!(artifacts.len(), 1);
    assert_eq!(artifacts[0].name, "tests_result");
    assert_eq!(
        artifacts[0].content,
        br#"{"commands":[],"status":"passed"}"#
    );
    assert_eq!(
        artifacts[0].source_kind,
        DiscoveredArtifactSourceKind::ChainworksOutput
    );
}

#[test]
fn json_chainworks_output_parser_ignores_prompt_examples_without_final_json() {
    let expected_outputs = vec![ExpectedOutputSpec {
        output_name: "implementation_progress".to_string(),
        output_role: domain::discovery::ExpectedOutputRole::Machine,
        target_path: "/tmp/run/implementation/progress.json".to_string(),
        companion_of: None,
        display_label: "implementation progress".to_string(),
        contract_id: None,
        required: true,
        reuse_policy: domain::discovery::OutputReusePolicy::MustProduce,
        max_bytes: 1024,
        aggregate_acceptance_cap_bytes: 4096,
        authorized_roots: vec![],
        source_generation_owner: domain::discovery::SourceGenerationOwner::Agent,
    }];
    let stream = "Please return {\"CHAINWORKS_OUTPUT\":{\"implementation_progress\":{\"status\":\"example\"}}} but no final answer was provided.";

    let artifacts = extract_output_envelopes(stream, &expected_outputs);

    assert!(artifacts.is_empty());
}
```

- [ ] **Step 2: Verify red**

Run: `cd control-plane && cargo test -p acp json_chainworks_output_is_extracted_from_fenced_final_text_with_trailing_prose json_chainworks_output_parser_ignores_prompt_examples_without_final_json`

Expected: the fenced-final-text test fails before implementation.

### Task 2: Implement Tolerant JSON Extraction in ACP Transport

**Files:**
- Modify: `control-plane/crates/acp/src/transport.rs`

- [ ] **Step 1: Replace suffix-only JSON parsing**

Update `extract_json_object_output_envelopes` so it scans for candidate JSON object boundaries around `CHAINWORKS_OUTPUT`, parses bounded objects with trailing prose allowed, and ignores inline prose examples unless they are in a final-answer-looking block or are the whole response.

- [ ] **Step 2: Add helpers**

Add helpers that:

- find a balanced JSON object containing the marker;
- accept objects wrapped in fenced code blocks;
- accept whole-response JSON with optional leading/trailing whitespace;
- keep existing byte caps through `ndjson_line_cap_bytes` and `bounded_envelope_payload_bytes`.

- [ ] **Step 3: Verify green**

Run: `cd control-plane && cargo test -p acp json_chainworks_output_is_extracted_from_fenced_final_text_with_trailing_prose json_chainworks_output_parser_ignores_prompt_examples_without_final_json`

Expected: both tests pass.

### Task 3: Align Prompt Contract With Canonical JSON

**Files:**
- Modify: `control-plane/crates/engine/src/orchestrator.rs`
- Modify: `control-plane/crates/engine/src/executor.rs`

- [ ] **Step 1: Update prompt examples**

Change code-writer and repair prompt text to prefer one JSON object:

```json
{"CHAINWORKS_OUTPUT":{"<canonical path from Required Outputs>":{"status":"complete"}}}
```

Remove marker-envelope examples from primary prompt guidance. Keep marker support in extraction only.

- [ ] **Step 2: Add/adjust prompt tests**

Update existing tests around `build_runtime_context_block`, `append_docs_noop_contract_guidance`, and `output_contract_repair_prompt` so they assert canonical JSON examples and no comment-placeholder JSON.

- [ ] **Step 3: Verify prompt tests**

Run: `cd control-plane && cargo test -p engine output_contract_repair_prompt_names_missing_outputs_and_exact_envelopes runtime_context_lists_declared_outputs_and_required_envelope`

Expected: prompt tests pass with canonical JSON expectations.

### Task 4: Add P084-Like Integration Coverage

**Files:**
- Modify: `control-plane/crates/acp/tests/integration.rs`

- [ ] **Step 1: Add final-result fixture**

Add an ACP fixture that returns final `result.output` containing `implementation_progress`, `implementation_self_assessment`, and `tests_result` through tolerant JSON extraction.

- [ ] **Step 2: Verify fixture**

Run: `cd control-plane && cargo test -p acp p084_like_final_output_materializes_required_outputs`

Expected: the fixture passes and discovers all three required outputs.

### Task 5: Run Focused Verification

**Files:**
- No new files.

- [ ] **Step 1: Run ACP focused tests**

Run: `cd control-plane && cargo test -p acp --lib`

Expected: all ACP library tests pass.

- [ ] **Step 2: Run ACP integration focused test**

Run: `cd control-plane && cargo test -p acp p084_like_final_output_materializes_required_outputs`

Expected: focused integration test passes.

- [ ] **Step 3: Check formatting**

Run: `cd control-plane && cargo fmt --check -p acp -p engine`

Expected: formatting is clean.
