# Tolerant Chainworks Output Extraction Design

Date: 2026-05-05

## Problem

P084 remains blocked after the ACP `result.output` extraction fix because required outputs are still settling as `missing_required_outputs`. The observed failure is not malformed output validation. The raw payload size is `0` for `implementation_progress`, `implementation_self_assessment`, and `tests_result`, while the control-plane generated `changed_files_manifest` passes. That points to the ingestion boundary: either the final answer text is not fully collected, or the output envelope shape returned by the agent is not being recognized.

The fix should not make durable output-contract repair the normal path. Repair can remain an emergency fallback, but the primary path must reliably ingest required outputs from the current ACP final result.

## Goal

Keep one canonical prompt contract while making the extractor tolerant to common, unambiguous variations in the final answer.

Canonical preferred format:

```json
{"CHAINWORKS_OUTPUT":{"<canonical path from Required Outputs>":{"status":"complete"}}}
```

The engine must accept only outputs declared for the current invocation and must continue to reject stale or undeclared artifacts.

## Non-Goals

- Do not reintroduce broad filesystem discovery.
- Do not treat output-contract repair as the primary materialization path.
- Do not accept arbitrary JSON from agent prose unless it contains a top-level `CHAINWORKS_OUTPUT`.
- Do not accept stale exact-path files unless the declared reuse policy allows it.

## Architecture

The normal flow is:

```text
prompt contract -> ACP final text collection -> tolerant CHAINWORKS_OUTPUT extraction -> declared-output settlement -> validation/import
```

Responsibilities:

- `control-plane/crates/engine/src/orchestrator.rs` builds one preferred prompt contract.
- `control-plane/crates/acp/src/transport.rs` collects final response text from valid ACP result sources and extracts output envelopes.
- `control-plane/crates/engine/src/executor.rs` and `control-plane/crates/engine/src/contracts.rs` remain the strict settlement and validation gate.
- Existing output-contract repair remains a fallback for genuinely missing or invalid outputs.

The extractor may be tolerant about syntax. Settlement remains strict about output identity and current-invocation ownership.

## Accepted Input Shapes

Extraction precedence:

1. Canonical JSON with canonical-path keys:
   `{"CHAINWORKS_OUTPUT":{"<canonical_path>":{"status":"complete"}}}`
2. JSON with output-name keys:
   `{"CHAINWORKS_OUTPUT":{"implementation_self_assessment":{"status":"needs_code_fixes"}}}`
3. Legacy marker envelope:
   `<<<CHAINWORKS_OUTPUT:implementation_self_assessment>>>{"status":"needs_code_fixes"}<<<END_CHAINWORKS_OUTPUT>>>`
4. Markdown/code-fenced JSON only when the parsed JSON contains top-level `CHAINWORKS_OUTPUT`.
5. Nested or stringified JSON when the provider wrapped final answer text in a field such as `result.output`, `result.text`, or `content[].text`.

If multiple payloads are found for the same declared output, the extractor uses the first payload by precedence and records a duplicate warning in diagnostics.

## Safety Rules

- Accept only keys matching a declared canonical target path or declared output name.
- If a key matches both a target path and an output name, canonical path wins.
- Do not read paths mentioned in prose.
- Do not parse prompt/history examples as output candidates; only parse final response text from the current ACP result.
- Enforce existing payload byte caps.
- Preserve existing stale-file rejection behavior.

## Prompt Contract Changes

The prompt should recommend exactly one format:

```json
{"CHAINWORKS_OUTPUT":{"<canonical path from Required Outputs>":{"status":"complete"}}}
```

Prompt text should explicitly say:

- Canonical path keys are preferred.
- Output-name keys may be accepted as fallback but should not be used when canonical paths are available.
- No markdown, prose, or code fences around the final JSON.
- Required output JSON must be valid JSON; examples must not contain comments or non-JSON placeholders.

The output-contract repair prompt should use the same preferred JSON shape. Marker envelopes remain supported as ingestion fallback, not as the primary prompt example.

Docs no-op examples should also use the preferred JSON object shape.

## Diagnostics

Discovery diagnostics should distinguish:

- final text absent;
- final text present but no `CHAINWORKS_OUTPUT` candidate found;
- candidate found but rejected because the key was undeclared;
- candidate found but rejected because payload exceeded caps;
- candidate accepted through canonical path, output-name fallback, marker envelope, fenced JSON, or stringified JSON.

Failed-stage evidence should include enough readback to tell whether the problem was text collection, envelope parsing, or settlement rejection.

## Tests

Required regression tests:

- ACP unit: `result.output` canonical JSON with canonical-path keys discovers every declared output.
- ACP unit: `result.output` JSON with output-name keys discovers outputs with fallback source diagnostics.
- ACP unit: fenced JSON containing top-level `CHAINWORKS_OUTPUT` is extracted.
- ACP unit: legacy marker envelope is still extracted.
- ACP unit: prose without valid `CHAINWORKS_OUTPUT` extracts nothing.
- Engine/executor fixture: P084-like required outputs `implementation_progress`, `implementation_self_assessment`, and `tests_result` settle as valid from final ACP text; `changed_files_manifest` remains control-plane generated.
- Regression: stale exact-path files are not accepted unless declared reuse policy allows them.

## Success Criteria

- A P084-like fixture fails before the extraction/prompt fix and passes after it.
- Existing ACP extraction tests continue to pass.
- No broad filesystem discovery is reintroduced.
- Diagnostics identify whether a failure came from missing final text, missing envelope, rejected key, rejected payload, or schema validation.
- The normal successful path does not depend on an output-contract repair turn.
