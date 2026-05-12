# Proposal 089: Junie Structured-Output Capability Proof and ACP Canary

| Field | Value |
|---|---|
| Date | 2026-05-12 |
| Status | Draft |
| Author | Codex |
| Depends on | P037 ACP supervision and idle-hang watchdog, P079 contract-aware output repair and provider fallback, `docs/reference/output-contracts-failure-evidence-and-recovery.md`, `docs/reference/acp-runtime-transport.md`, `docs/reference/rust-control-plane.md`, retained `proposal-088` gate |
| Related | P036 UX consolidation, `docs/reference/test-gates.md`, `examples/agents/agents.yaml` |
| Scope | De-risk Junie-specific rollout and P036-family diagnosis by proving Junie can emit strict structured outputs at all, then proving the same behavior on the real ACP code-writer path with a tiny canary. |
| Non-goal | No broad completion-boundary implementation here, no provider swap, no weakening of output contracts, and no claim that a passing canary alone fully fixes P036-class failures. |

---

## 1. Problem

P088 already established and landed the general code-writer completion receipt, freshness, and repair-diagnostic boundary. P089 is narrower: before relying on Junie for future P036-family diagnosis or broader Junie rollout, the team needs durable proof that Junie can actually be driven to produce structured outputs instead of narrative prose.

Without that proof, Junie-specific rollout and diagnosis work would be high risk for two different reasons:

1. Junie might fundamentally resist returning strict JSON or `CHAINWORKS_OUTPUT`, making prompt and settlement hardening a dead end.
2. Junie might succeed in isolated prompts but still fail on the real ACP code-writer path, meaning the missing piece is in orchestration, not basic model capability.

The current evidence from P036 only proves the negative path:

- Junie can produce large prose completions;
- a long completion can be truncated before output extraction;
- completion repair can also return prose instead of a structured payload;
- the run then fails as `missing_required_outputs`.

That is enough to justify investigation, but not enough to trust Junie on future structured-output runs with confidence.

### 1.1 Current baseline and sequencing

The relevant current-system baseline is ACP-based, not Goose-based. Current references and code already describe ACP runtime profiles, the Junie ACP adapter, P088 completion receipts, and the retained `proposal-088|p088` gate. Any older Goose-era provider baseline is investigation history only and must not drive P089 implementation decisions.

P089 therefore does not sequence ahead of P088. It sequences ahead of future Junie-specific rollout decisions and P036-family diagnosis that would otherwise rely on unproven Junie structured-output behavior.

## 2. Decision

Before future Junie rollout or P036-family diagnosis depends on Junie structured-output behavior, the project must complete two explicit proof steps:

1. **Capability proof:** prove on native Junie CLI that the model can return strict JSON and strict `CHAINWORKS_OUTPUT` without prose.
2. **ACP canary:** prove on the real ACP-backed `code_writer` path that the same provider can complete a deliberately tiny structured-output task and settle valid outputs through the normal orchestration boundary.

Junie-specific rollout work may proceed only after both steps are captured as durable evidence and validated by `./scripts/test-gate.sh proposal-089`. This does not retroactively block or redefine the already-landed P088 completion-boundary contract; it adds a provider-specific proof layer on top of that contract.

## 3. Step 1: Capability Proof

### 3.1 Goal

Prove that Junie is not inherently limited to narrative responses and can, when instructed narrowly enough, return exact structured payloads.

### 3.2 Required experiments

Run at least these isolated experiments against Junie outside the workflow engine:

1. exact JSON only:
   - prompt requires one exact JSON object and nothing else;
2. exact `CHAINWORKS_OUTPUT` only:
   - prompt requires one exact top-level `CHAINWORKS_OUTPUT` object and nothing else;
3. repair-style minimal synthesis:
   - prompt mirrors the intended completion-repair shape and requires only corrected output payloads.

### 3.3 Evidence paths

Native capability proof is not accepted from memory or console scrollback. Each experiment must write the following repo evidence:

| Experiment | Directory |
|---|---|
| exact JSON only | `docs/evidence/089-junie-structured-output-canary/native/exact-json/` |
| exact `CHAINWORKS_OUTPUT` only | `docs/evidence/089-junie-structured-output-canary/native/exact-chainworks-output/` |
| repair-style minimal synthesis | `docs/evidence/089-junie-structured-output-canary/native/repair-style-minimal/` |

Each directory must contain:

- `prompt.txt`: exact prompt sent to Junie;
- `command.json`: executable name/path, arguments, working directory, input mode, output mode, timeout, environment names used, and redacted sensitive values;
- `environment.json`: timestamp, host class, Junie version, detected binary path, auth/license preflight result, and model/toolchain identifier when available;
- `final-output.raw.txt`: raw final text returned by Junie;
- `parser-result.json`: parser verdict, top-level shape, extracted fields, parse error if any, and `sha256` of `final-output.raw.txt`;
- `conclusion.json`: one of the closed status values from Section 6 plus a short reviewer-readable conclusion.

### 3.4 Native invocation contract

Native experiments must use Junie standard CLI mode, not ACP mode. The command schema is part of the proof and default-mode validation must reject evidence produced by any other invocation shape.

Required command shape:

```json
{
  "schema_version": "p089_native_command_v1",
  "binary_resolution": {
    "env_var": "CHAINWORKS_JUNIE_ACP_BINARY",
    "fallback_binary": "junie",
    "resolved_path": "/absolute/path/or/path-lookup-result"
  },
  "working_directory": "/absolute/repo/or-disposable-project-root",
  "project_path": "/absolute/repo/or-disposable-project-root",
  "input_mode": "task_arg",
  "output_mode": "stdout_text",
  "args": [
    "--project",
    "<project_path>",
    "--task",
    "<prompt.txt contents>",
    "--output-format",
    "text",
    "--timeout",
    "<milliseconds>",
    "--skip-update-check"
  ],
  "forbidden_args": ["--acp", "--json-output-file"],
  "parser_input_source": "stdout_after_process_exit",
  "stderr_role": "diagnostics_only",
  "timeout_ms": 120000,
  "exit_code_policy": "exit_code_zero_required_for_pass"
}
```

The exact timeout may be raised by implementation if the live Junie CLI requires it, but the value must be recorded in `command.json` and `live-gate-run.json`. `prompt.txt` is the source of truth for the task body; `command.json.args` may record a redacted or hash-only task argument if needed, but it must include `prompt_sha256` and the gate must verify it against `prompt.txt`.

Exit-code handling is closed:

- preflight failures before invocation classify as `environment_unavailable`;
- non-zero process exit caused by auth, license, binary, unsupported version, missing model, or local toolchain setup classifies as `environment_unavailable`;
- timeout or non-zero exit after a successful preflight but before a parseable final response classifies as `native_capability_failed` unless the failure is conclusively one of the environment cases above;
- `final-output.raw.txt` is populated only from stdout after process exit, not from stderr or terminal scrollback.

Official Junie documentation distinguishes standard/headless CLI from ACP mode and documents `--task`, `--output-format`, `--project`, `--timeout`, `--skip-update-check`, and `--acp`. P089 intentionally uses standard CLI for native capability proof and ACP mode only for the canary.

### 3.5 Acceptance criteria

Capability proof passes only if:

- each experiment returns a parseable final JSON object;
- no Markdown, prose summary, or explanatory prelude appears in the final response;
- the top-level shape exactly matches the requested contract;
- `command.json` matches `p089_native_command_v1`;
- `parser-result.json` records success and the output hash;
- `./scripts/test-gate.sh proposal-089` can validate the evidence without relying on operator memory.

### 3.6 Current status

Local notes say Junie already returned exact JSON, exact `CHAINWORKS_OUTPUT`, and repair-style `CHAINWORKS_OUTPUT`. Those notes are useful investigation context, but they are not accepted proof until the evidence paths in Section 3.3 exist and pass the P089 gate.

## 4. Step 2: ACP Canary

### 4.1 Goal

Prove that the real ACP-backed `code_writer` path can carry a tiny structured-output interaction end to end.

This step is intentionally narrower than fixing P036. It is not a product feature run. It is a proof that:

- provider launch,
- ACP transport,
- final-text capture,
- extraction,
- settlement,
- and required-output materialization

can work together on a deliberately small task.

### 4.2 Canary identity

The default canary must use the existing production `code_writer` catalog binding unchanged:

- `agent_id = code_writer`;
- `backend_profile = junie_code_editor_acp`;
- `provider = junie`;
- `runtime_profile = junie_cli_acp`;
- adapter family `JunieAdapter`;
- launch mode `--acp true`, as documented in `docs/reference/acp-runtime-transport.md`.

If a dedicated synthetic canary agent is introduced instead, the proposal must be amended before implementation. That amendment must explain why the production `code_writer` identity is unsafe for the canary, and the receipt must still prove that the same Junie ACP adapter, backend profile, output contract settlement, and completion capture path were used.

### 4.3 Canary shape

The canary must use:

- the same Junie provider family as the failing production path;
- the real ACP adapter and session transport;
- a tiny `code_writer` task with minimal or no repository mutation;
- declared outputs under a disposable canary artifact root;
- a required output set small enough that any failure is attributable to the boundary, not to implementation complexity.

Recommended canary shape:

- one synthetic canary run using the production `code_writer` agent identity;
- one minimal required output, or a minimal trio of `progress`, `self-assessment`, and `tests` with trivial values;
- explicit instruction that the final response must be only `CHAINWORKS_OUTPUT`;
- bounded prompt context so the completion cannot naturally balloon into a multi-hundred-kilobyte prose summary;
- mutation guard proving no non-canary repository files changed.

### 4.4 ACP canary evidence

ACP canary evidence must live under:

`docs/evidence/089-junie-structured-output-canary/acp-canary/`

The directory must contain:

- `preflight.json`: closed preflight status from Section 6, including binary path, version, auth/license result, model/toolchain availability, and project path;
- `receipt.json`: provider/runtime identity, run id, stage id, agent execution id, session generation, declared output names and paths, completion capture metadata, extraction metadata, settlement metadata, and repair metadata;
- `terminal-completion.raw.txt`: raw final completion text used for extraction, or a typed absence reason if unavailable;
- `extraction-result.json`: extraction source, truncation flags, extracted payload hash, parser result, and failure reason when applicable;
- `settled-outputs.json`: declared outputs, materialized paths, freshness/current-attempt proof, and per-output settlement decisions;
- `run-report.json`: concise operator-facing result and next action;
- `worktree-fingerprint-pre.json`: `worktree_fingerprint_v1` before canary execution;
- `worktree-fingerprint-post.json`: `worktree_fingerprint_v1` after canary execution and evidence write;
- `mutation-guard-result.json`: path-level comparison, allowed roots, preexisting dirty-work classification, and final mutation verdict;
- `conclusion.json`: one of the closed status values from Section 6 plus a reviewer-readable conclusion.

### 4.5 Acceptance criteria

The ACP canary passes only if all of the following are true:

- preflight succeeds and does not report `environment_unavailable`;
- the Junie `code_writer` prompt reaches terminal completion;
- the receipt proves `provider=junie`, `runtime_profile=junie_cli_acp`, `agent_id=code_writer`, `backend_profile=junie_code_editor_acp`, and `adapter_family=JunieAdapter`;
- the final completion text is captured without truncation at the extraction boundary;
- a valid `CHAINWORKS_OUTPUT` payload is extracted from the completion path;
- declared outputs settle as fresh current-attempt outputs;
- no completion repair turn is needed;
- `completion_turn_attempted = false`;
- `completion_repair_turn_count = 0`;
- `generic_repair_turn_count = 0`;
- no completion repair runtime receipt is present;
- mutation guard proves no non-canary repository files changed;
- pre/post fingerprints use the existing `worktree_fingerprint_v1` schema from `docs/reference/output-contracts-failure-evidence-and-recovery.md`;
- `./scripts/test-gate.sh proposal-089` validates the receipt, settled outputs, and conclusion.

### 4.6 Mutation guard contract

The ACP canary reuses the P088 `worktree_fingerprint_v1` semantics. It must not introduce a second ad hoc mutation detector.

Live mode captures:

1. `worktree-fingerprint-pre.json` before the ACP canary starts;
2. the canary run and evidence write;
3. `worktree-fingerprint-post.json` after evidence write;
4. `mutation-guard-result.json` comparing the two fingerprints.

Allowed changed roots are limited to:

- `docs/evidence/089-junie-structured-output-canary/**`;
- the disposable canary artifact root recorded in `receipt.json`;
- the canary run directory recorded in `receipt.json`, if the implementation uses a real run;
- deterministic temporary paths under `.chainworks/tmp/p089-*` only when those paths are recorded and cleaned or marked ephemeral.

Source files, proposal files outside the P089 evidence root, workflow/catalog YAML, Rust/Swift code, and non-canary `.chainworks/runs/**` paths are forbidden. Any such change classifies the ACP canary as `unexpected_repo_mutation`.

Default live mode must start from a clean worktree outside the allowed evidence roots. If the operator intentionally sets `CHAINWORKS_PROPOSAL_089_ALLOW_DIRTY=1`, the gate may produce diagnostic evidence, but `mutation-guard-result.json` must mark `preexisting_dirty_work_non_canary_safe=true` and `overall_status` must not be `passed`. A signoff-quality P089 live receipt requires no preexisting dirty work outside allowed canary/evidence roots.

## 5. Gate Contract

P089 requires a focused gate:

```bash
./scripts/test-gate.sh proposal-089
```

The gate must have a deterministic evidence-validation mode and a live execution mode:

- default mode validates checked-in evidence schemas, closed vocabularies, parser results, output hashes, receipt identity, and canary settlement artifacts;
- live mode is enabled by `CHAINWORKS_PROPOSAL_089_LIVE=1` and runs the native experiments plus the ACP canary against the configured Junie toolchain;
- live mode must require `CHAINWORKS_JUNIE_ACP_BINARY` or a documented default Junie binary discovered by the adapter;
- live mode must fail closed with `environment_unavailable` when the local toolchain cannot support the run.

Signoff requires a successful live-mode gate at least once, followed by checked-in evidence that passes default validation mode. A default-mode pass without live evidence is not sufficient for proposal acceptance.

### 5.1 Evidence index and live-gate receipt

The evidence root must contain these top-level files:

| File | Purpose |
|---|---|
| `evidence-index.json` | Canonical index of native experiments, ACP canary, phase statuses, overall status, and file hashes. |
| `live-gate-run.json` | Machine-verifiable receipt for the prior successful live-mode gate. |

`evidence-index.json` schema:

```json
{
  "schema_version": "p089_evidence_index_v1",
  "proposal": "089-junie-structured-output-capability-proof-and-acp-canary",
  "audited_git_sha": "40-hex",
  "proof_critical_files": [
    {
      "path": "scripts/test-gate.sh",
      "sha256": "...",
      "size_bytes": 0
    },
    {
      "path": "control-plane/crates/acp/src/adapters/junie.rs",
      "sha256": "...",
      "size_bytes": 0
    },
    {
      "path": "control-plane/crates/engine/src/executor.rs",
      "sha256": "...",
      "size_bytes": 0
    },
    {
      "path": "examples/agents/agents.yaml",
      "sha256": "...",
      "size_bytes": 0
    }
  ],
  "native_phase_status": "passed",
  "acp_canary_status": "passed",
  "overall_status": "passed",
  "native_experiments": [
    {
      "name": "exact-json",
      "status": "passed",
      "directory": "docs/evidence/089-junie-structured-output-canary/native/exact-json",
      "files": {
        "prompt.txt": {"sha256": "...", "size_bytes": 0},
        "command.json": {"sha256": "...", "size_bytes": 0},
        "environment.json": {"sha256": "...", "size_bytes": 0},
        "final-output.raw.txt": {"sha256": "...", "size_bytes": 0},
        "parser-result.json": {"sha256": "...", "size_bytes": 0},
        "conclusion.json": {"sha256": "...", "size_bytes": 0}
      }
    }
  ],
  "acp_canary": {
    "status": "passed",
    "directory": "docs/evidence/089-junie-structured-output-canary/acp-canary",
    "files": {
      "preflight.json": {"sha256": "...", "size_bytes": 0},
      "receipt.json": {"sha256": "...", "size_bytes": 0},
      "terminal-completion.raw.txt": {"sha256": "...", "size_bytes": 0},
      "extraction-result.json": {"sha256": "...", "size_bytes": 0},
      "settled-outputs.json": {"sha256": "...", "size_bytes": 0},
      "run-report.json": {"sha256": "...", "size_bytes": 0},
      "worktree-fingerprint-pre.json": {"sha256": "...", "size_bytes": 0},
      "worktree-fingerprint-post.json": {"sha256": "...", "size_bytes": 0},
      "mutation-guard-result.json": {"sha256": "...", "size_bytes": 0},
      "conclusion.json": {"sha256": "...", "size_bytes": 0}
    }
  },
  "live_gate_run": {
    "path": "docs/evidence/089-junie-structured-output-canary/live-gate-run.json",
    "sha256": "..."
  }
}
```

`live-gate-run.json` schema:

```json
{
  "schema_version": "p089_live_gate_run_v1",
  "command": "./scripts/test-gate.sh proposal-089",
  "environment": {
    "CHAINWORKS_PROPOSAL_089_LIVE": "1",
    "recorded_env_names": ["CHAINWORKS_JUNIE_ACP_BINARY"],
    "redacted_env": true
  },
  "working_directory": "/absolute/repo/root",
  "audited_git_sha": "40-hex",
  "proof_critical_files": [
    {
      "path": "scripts/test-gate.sh",
      "sha256": "...",
      "size_bytes": 0
    },
    {
      "path": "control-plane/crates/acp/src/adapters/junie.rs",
      "sha256": "...",
      "size_bytes": 0
    },
    {
      "path": "control-plane/crates/engine/src/executor.rs",
      "sha256": "...",
      "size_bytes": 0
    },
    {
      "path": "examples/agents/agents.yaml",
      "sha256": "...",
      "size_bytes": 0
    }
  ],
  "started_at": "RFC3339",
  "completed_at": "RFC3339",
  "exit_code": 0,
  "result": "passed",
  "log": {
    "path": "docs/evidence/089-junie-structured-output-canary/live-gate.log.redacted",
    "sha256": "...",
    "size_bytes": 0
  },
  "evidence_index_sha256": "...",
  "native_phase_status": "passed",
  "acp_canary_status": "passed",
  "overall_status": "passed"
}
```

Default mode must verify that:

- every file listed in `evidence-index.json` exists and matches `sha256` and `size_bytes`;
- `live-gate-run.json.exit_code == 0`;
- `live-gate-run.json.result == "passed"`;
- `live-gate-run.json.audited_git_sha` matches `evidence-index.json.audited_git_sha`;
- `proof_critical_files` exists in both `evidence-index.json` and `live-gate-run.json`, covers at minimum the P089 gate implementation, Junie ACP adapter, completion extraction/settlement code, and production `code_writer` catalog binding, and the two records match exactly;
- every `proof_critical_files` entry still matches the current checked-out file by `sha256` and `size_bytes`;
- `live-gate-run.json.evidence_index_sha256` matches the checked-in `evidence-index.json`;
- the command and environment prove live mode was used;
- the redacted log exists and matches its hash.

If any of these checks fail, default mode classifies P089 as `evidence_incomplete` even if individual experiment files look successful. In particular, checked-in evidence from an older gate, ACP adapter, completion extraction, settlement, or catalog version must fail closed instead of being replayed as current proof.

## 6. Closed Failure Vocabulary

Every P089 attempt must classify its terminal outcome as exactly one of:

| Status | Meaning |
|---|---|
| `passed` | The native proof or ACP canary met all acceptance criteria. |
| `environment_unavailable` | Junie binary missing, empty, not executable, unsupported version, auth/license unavailable, model unavailable, project path invalid, or required local toolchain missing. |
| `native_capability_failed` | Native Junie final output was not strict parseable JSON or did not match the requested shape. |
| `acp_launch_failed` | Junie ACP subprocess could not launch or exited before session establishment. |
| `acp_handshake_failed` | ACP process launched but JSON-RPC/session initialization failed. |
| `completion_capture_failed` | Terminal/final completion text was absent or not captured from the source used for extraction. |
| `completion_capture_truncated` | Completion text or extraction input was truncated before the required payload could be proven absent or present. |
| `extraction_failed` | Completion text was captured but no valid `CHAINWORKS_OUTPUT` payload was extracted. |
| `settlement_failed` | Payload extracted but declared outputs did not settle as fresh current-attempt artifacts. |
| `unexpected_completion_repair` | The canary used completion repair even though acceptance requires normal settlement. |
| `unexpected_repo_mutation` | Non-canary repository files changed during the canary. |
| `evidence_incomplete` | The attempt may have run, but required evidence files, hashes, or schema fields are missing. |

This vocabulary is public readback truth for P089 evidence. Unknown future values must be preserved as raw strings in readback but treated as non-passing until the proposal or reference contract is updated.

## 6.1 Phase and overall aggregation

Aggregation is deterministic and lives in `evidence-index.json`.

Native phase rules:

- `native_phase_status = passed` only when all three required native experiments are `passed` and all native evidence hashes validate.
- `native_phase_status = evidence_incomplete` when any required native file, hash, command schema field, parser result, or conclusion is missing or invalid.
- `native_phase_status = environment_unavailable` when any native experiment is `environment_unavailable` and no evidence-integrity problem has already classified the phase as `evidence_incomplete`.
- `native_phase_status = native_capability_failed` when at least one native experiment is `native_capability_failed` and none are `environment_unavailable` or `evidence_incomplete`.
- any other future native status is preserved raw and aggregated as `evidence_incomplete` until the proposal or reference contract is updated.

ACP canary rules:

- `acp_canary_status = passed` only when all ACP canary acceptance criteria pass, all no-repair receipt counters agree, all evidence hashes validate, and `mutation-guard-result.json.verdict == "passed"`.
- `acp_canary_status = evidence_incomplete` when any required ACP file, hash, receipt identity, no-repair receipt counter, fingerprint, parser result, settlement record, live-gate linkage, or conclusion is missing or invalid.
- `acp_canary_status = environment_unavailable` when preflight reports an unavailable Junie/toolchain/project environment and evidence integrity is otherwise valid.
- otherwise `acp_canary_status` equals the first failing boundary in this order: `acp_launch_failed`, `acp_handshake_failed`, `completion_capture_failed`, `completion_capture_truncated`, `extraction_failed`, `settlement_failed`, `unexpected_completion_repair`, `unexpected_repo_mutation`.

Overall rules:

- `overall_status = passed` only when `native_phase_status == passed`, `acp_canary_status == passed`, and `live-gate-run.json` proves a successful live-mode gate.
- `overall_status = evidence_incomplete` when the live-gate receipt or evidence index is missing, hash-invalid, or internally inconsistent.
- `overall_status = environment_unavailable` when either phase is `environment_unavailable` and evidence integrity is otherwise valid.
- otherwise `overall_status` is the first non-passing phase status in phase order: native capability, then ACP canary.

Mixed states such as two native experiments `passed` and one `environment_unavailable` aggregate to `environment_unavailable`, not `passed` and not product/model evidence. No product conclusion may be drawn from any non-`passed` overall status.

## 7. Failure Interpretation

If the capability proof passes but the ACP canary fails with `acp_launch_failed`, `acp_handshake_failed`, `completion_capture_failed`, `completion_capture_truncated`, `extraction_failed`, or `settlement_failed`, the conclusion must be:

- Junie capability is adequate;
- the remaining defect is in the ACP/orchestration/completion boundary.

If capability proof fails with `native_capability_failed`, the conclusion must be:

- Junie output steering is insufficient under the current prompting approach;
- deeper engine hardening alone is unlikely to close the problem.

If either proof fails with `environment_unavailable`, no product or model conclusion may be drawn. The correct next action is to repair the local Junie toolchain, auth/license state, or project setup and rerun the gate.

## 8. Why both steps are required

Either step alone is insufficient:

- capability proof without ACP canary only proves model ability in isolation;
- ACP canary without capability proof leaves open the possibility that Junie never reliably follows strict output instructions at all.

Together, they isolate the problem:

- if both pass, Junie-specific rollout and diagnosis have a justified target;
- if capability passes and ACP fails, the target is the runtime boundary;
- if capability fails, the target is prompt/provider suitability, not settlement mechanics.

## 9. Impact on Completion-Boundary Work

This proposal does not replace the stable output-contract and ACP runtime references.

Instead, it adds an explicit de-risking gate for Junie-specific rollout and P036-family diagnosis:

1. preserve native Junie capability proof as durable evidence;
2. implement and run the ACP canary;
3. only then use Junie structured-output behavior as evidence for future P036-family diagnosis or rollout decisions.

The relevant completion-boundary truth is now reference-owned by:

- `docs/reference/output-contracts-failure-evidence-and-recovery.md`;
- `docs/reference/acp-runtime-transport.md`;
- `docs/reference/rust-control-plane.md`;
- the retained `proposal-088|p088` gate alias in `docs/reference/test-gates.md`.

P089 establishes whether Junie is a viable structured-output provider on the existing ACP/P088 boundary and where the likely defect boundary actually sits.

## 10. Non-Goals and Guardrails

- Do not declare P036 fixed based on capability proof alone.
- Do not treat a passing canary as proof that long-running production attempts can never regress.
- Do not treat P089 as a prerequisite for already-landed P088 behavior; it is a Junie-specific proof for future rollout and diagnosis.
- Do not widen prompt budgets or relax output contracts just to make the canary pass.
- Do not add provider-specific folklore as canonical truth if the real issue is missing completion-boundary discipline.
- Do not use fallback providers, completion repair, or manual artifact writing to make the ACP canary pass.

## 11. Acceptance

P089 is satisfied only when:

1. all three native capability experiments are preserved under `docs/evidence/089-junie-structured-output-canary/native/`;
2. a real ACP canary has passed end to end under `docs/evidence/089-junie-structured-output-canary/acp-canary/`;
3. `./scripts/test-gate.sh proposal-089` passes in default evidence-validation mode;
4. the evidence contains a prior successful `CHAINWORKS_PROPOSAL_089_LIVE=1 ./scripts/test-gate.sh proposal-089` run result;
5. `evidence-index.json` and `live-gate-run.json` validate all evidence hashes, phase statuses, live command identity, audited git SHA, proof-critical source/gate/catalog hashes, exit code, and redacted log hash;
6. the ACP canary mutation guard validates pre/post `worktree_fingerprint_v1` artifacts and proves no non-canary repository mutation;
7. the team can state, with evidence, whether Junie is capable and whether the runtime boundary is the remaining defect surface.

Only after that should the project rely on Junie structured-output behavior for future rollout or P036-family diagnosis.
