# P079 Repair Prompt Template

This is the implemented P079 same-session output repair prompt contract.
Operational behavior is owned by the Rust executor in
`control-plane/crates/engine/src/executor.rs`.

## Version

- `repair_prompt_template_version`: `p079_repair_v1`
- Stored on each `output_contract_repair.v1` row as
  `repair_prompt_template_version`
- Primary implementation functions:
  - `output_contract_repair_prompt`
  - `code_writer_completion_repair_prompt`

The version changes only when the observable prompt contract changes. Cosmetic
Rust formatting changes do not require a version bump.

## General Repair Prompt

The general repair prompt starts with:

```text
### Output Contract Repair (p079_repair_v1)
run_id=<run_id> stage_execution_id=<stage_execution_id> agent_execution_id=<agent_execution_id>
```

It then instructs the provider to use only the immediately preceding turn,
perform minimal synthesis, and return only corrected `CHAINWORKS_OUTPUT`
payloads for the failed outputs. It explicitly forbids unrelated
implementation work, Markdown explanations, code fences, stdout-as-output, and
shell `echo`/`printf` publication.

The prompt includes one line per failed output with:

- output name,
- contract id when present,
- missing field names,
- sanitized validator error when present.

It ends with one JSON example:

```json
{"CHAINWORKS_OUTPUT":{"<canonical output path or output name>":"<contract-shaped value>"}}
```

When a large canonical file already exists, the provider is directed to return
a direct-file manifest instead of embedding the full content.

## Code Writer Completion Repair Prompt

The code-writer variant starts with:

```text
### Code Writer Completion Repair v1 (p079_repair_v1)
```

It is output-publication only. It forbids repository edits, tool calls, and
redoing implementation work. It accepts a single final JSON object containing
`CHAINWORKS_OUTPUT` entries for missing or invalid completion outputs.

## Reflected Error Sanitization

Validator text is untrusted. Each reflected fragment is:

- capped at 2048 bytes,
- included only while the aggregate reflected-fragment budget is below 8192
  bytes,
- wrapped in:

```text
<<<UNTRUSTED_VALIDATOR_ERROR>>>...<<<END_UNTRUSTED_VALIDATOR_ERROR>>>
```

Known prompt-injection markers are replaced with
`[redacted:injection_marker]`. The marker list covers common model role tags,
well-known provider control tokens, the untrusted-content fence tokens
themselves, and case-insensitive instruction-override phrases such as
`ignore previous instructions`, `you are now`, and `new system prompt`.

## Settlement Rules

The prompt is only a publication attempt. It does not authorize output truth.
Returned content still passes normal declared-output validation, exact canonical
path binding, source-generation settlement, and P079 repair posture checks
before it can update active artifact truth.

Production providers remain fail-closed for same-session repair unless their
runtime exposes an enforceable filesystem/tool/network permission boundary.
Fixture transport qualifies for deterministic tests because it cannot perform
arbitrary file I/O.

## Gate Coverage

`./scripts/test-gate.sh proposal-079` covers the prompt through focused Rust
tests named for `output_contract_repair_prompt`, repair posture tests, and the
deterministic fixture same-session repair path. The rollout fixture
`docs/evidence/rollout-contract/p079/negative/repair-prompt-template-pinned.json`
is retained as rollout-contract evidence, but the executable Rust tests are the
normative proof.
