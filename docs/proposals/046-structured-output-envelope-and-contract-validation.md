# Proposal 046: Structured Output Envelope Parsing and Contract Validation

| Field | Value |
|---|---|
| Date | 2026-04-14 |
| Status | Draft |
| Author | Claude |
| Depends on | None (standalone improvement) |
| Scope | (A) Parse `<<<CHAINWORKS_OUTPUT:name>>>` envelope blocks from ACP agent responses. (B) Validate extracted artifacts against output contracts using the `OutputContractResolverV2` owner model. (C) Persist validation-failure evidence as durable stage-owned artifacts. |
| Goal | Agent outputs are identified by name (not just filesystem diff), validated against the single contract authority (`AgentCatalog.contracts`), and failures produce durable `ValidationFailureRecord` artifacts that report/recovery readers consume — matching the stable Swift owner chain. |

---

## 1. Context and Motivation

### 1a. Envelope Parsing

The Swift `RuntimeAgentExecutor` extracts structured output blocks from agent responses using delimiters:

```
<<<CHAINWORKS_OUTPUT:proposal_review_summary>>>
{"pass": true, "average_score": 9.55, ...}
<<<END_CHAINWORKS_OUTPUT>>>
```

The Rust daemon relies exclusively on workspace filesystem diff (`pre_files` vs `post_files` in `transport.rs`). This works for agents that write files, but:

- **False positives:** temp files, log files, `.DS_Store` appear as artifacts
- **No naming:** discovered files aren't mapped to declared output names
- **Lost outputs:** if an agent returns structured output in stdout without writing a file, the daemon loses it entirely

### 1b. Contract Validation

The Rust compiler already resolves `OutputSchema` (contract_id, format, required_fields) and injects them into prompts. But after the agent responds, **no validation occurs**. If an agent returns malformed JSON or missing fields, the daemon accepts it silently and the next agent reads garbage.

**Stable Swift owner chain (not `StructuredOutputSchemaGate`):**
- `StructuredOutputSchemaGate` is a **preflight-only** gate — it validates transport capability declarations *before* execution, not returned payloads.
- **Post-generation validation** is owned by `OutputContractResolverV2` which resolves schemas and validates returned payloads against `AgentCatalog.contracts`.
- `ArtifactPersistenceOrderingPolicy` orchestrates the ordering: raw outputs persist first, validation runs second, failure evidence persists third, stage settles last.
- On failure, a `ValidationFailureRecord` is persisted as a durable system artifact consumed by `RunReportBuilder` and recovery surfaces.

P046 ports this exact owner chain to Rust. It does **not** introduce a second contract-validation authority.

---

## 2. Design

### 2a. Envelope Extraction (transport layer)

In `acp/src/transport.rs`, after the ACP session completes, scan the accumulated agent response text for envelope blocks:

```rust
fn extract_output_envelopes(response_text: &str) -> Vec<(String, String)> {
    // Parse <<<CHAINWORKS_OUTPUT:name>>>...<<<END_CHAINWORKS_OUTPUT>>> pairs
    // Returns Vec<(artifact_name, content)>
}
```

Apply after filesystem diff, merge results: envelope outputs take priority over filesystem-discovered files with the same name.

### 2b. Artifact Discovery with Canonical Metadata Binding

Discovered artifacts (envelope and filesystem) must be rebound to compiled plan metadata **before persistence**. No heuristic basename-derived names; no fallback to provider-scoped stub `contract_id` when a declared output exists.

```rust
struct DiscoveredArtifact {
    /// Output name from the compiled task's declared outputs (canonical).
    name: String,
    /// Raw content bytes.
    content: Vec<u8>,
    /// How the artifact was discovered.
    source: ArtifactSource,  // Envelope | Filesystem
    /// Filesystem path (if discovered via diff).
    path: Option<String>,
    /// Canonical path from `RunPlan.artifact_paths` (resolved).
    canonical_path: String,
    /// Contract metadata from `CompiledTask.output_schemas` (if declared).
    contract_id: Option<String>,
    /// Format from output schema (e.g. "json", "markdown").
    format: Option<String>,
    /// Required fields from output schema (for validation).
    required_fields: Vec<String>,
}
```

**Binding algorithm (executor, after ACP returns):**

1. For each envelope output `(name, content)`:
   - Look up `name` in `compiled_task.outputs` → confirm it's a declared output
   - Look up `name` in `compiled_task.output_schemas` → bind `contract_id`, `format`, `required_fields`
   - Look up `name` in `plan.artifact_paths` → resolve canonical path via `resolve_path_template`
   - If `name` not in declared outputs → still persist but mark as `undeclared` (no contract validation)

2. For each filesystem-diff output:
   - Match against declared outputs by filename/path heuristics (existing behavior)
   - If matched → bind same metadata as above
   - If unmatched → persist with no contract metadata (existing behavior, no regression)

3. Envelope outputs override filesystem outputs with the same canonical name.

### 2c. Contract Validation (Full OutputContractResolverV2 Parity)

Validation is a **separate step** from discovery. It runs after raw artifacts are persisted (matching Swift's `ArtifactPersistenceOrderingPolicy` ordering).

**Validation mode enum** (matching Swift `OutputContractSchemaV2.ValidationMode`):

```rust
pub enum ValidationMode {
    HumanOnly,                    // no machine validation — always passes
    StrictStructured,             // machine_format must parse + all required fields present; failure is hard
    StructuredWithHumanCompanion, // same checks as strict, but allows human companion alongside
}

pub enum MachineFormat {
    Json,
    Other(String),  // non-JSON structured (e.g. yaml, csv) — check non-empty only
}
```

**Output schema extension:** The compiled `OutputSchema` (from `workflow/plan.rs`) must carry `validation_mode` and `machine_format` in addition to the existing `contract_id`, `format`, and `required_fields`:

```rust
pub struct OutputSchema {
    pub contract_id: String,
    pub format: String,
    pub required_fields: Vec<String>,
    pub validation_mode: String,    // "strict_structured" | "structured_with_human_companion" | "human_only"
    pub machine_format: String,     // "json" | other
    pub human_format: Option<String>, // companion format: "markdown" | "text" | None
    pub normalized_artifact_name: Option<String>,  // stable artifact identity
    pub raw_artifact_name: Option<String>,          // pre-normalization name
}
```

`human_format` must be parsed from catalog YAML in `workflow/src/catalog.rs` and then carried through `workflow/src/compiler.rs` into `workflow/src/plan.rs`; it is part of full schema parity, not a compiler-only default.

**Validation result:**

```rust
pub struct OutputValidationResult {
    pub output_name: String,
    pub contract_id: Option<String>,
    pub status: ValidationStatus,  // Passed | Failed | NoContractDeclared
    pub missing_fields: Vec<String>,
    pub validation_error: Option<String>,
    pub raw_payload_size: usize,
}
```

**Validation algorithm (matching Swift `OutputContractResolverV2.validateSingleOutput`):**

```rust
pub fn validate_output(
    output_name: &str,
    content: &[u8],
    schema: &OutputSchema,
) -> OutputValidationResult;
```

1. **No contract declared** → `NoContractDeclared` (pass-through, not an error)
2. **`human_only` mode** → `Passed` (no machine validation; matches Swift line 141-149)
3. **`strict_structured` or `structured_with_human_companion` mode:**
   - If `machine_format == "json"`:
     a. Parse JSON; if invalid → `Failed` with "not valid JSON or not a JSON object"
     b. Check all `required_fields` present as top-level keys
     c. All present → `Passed`
     d. Missing fields → `Failed` with "Missing required fields: {list}"
   - If `machine_format` is non-JSON: check content non-empty; empty → `Failed`
4. Return `OutputValidationResult`

**Contract binding chain restoration (Swift `OutputContractResolverV2` parity):**

Before mode validation, resolve each discovered output to a contract through the same ordered chain:

1. **Explicit `output_contract` preference:** if output metadata includes an explicit `output_contract`, resolve it directly.
2. **Exact normalized match:** resolve against catalog contracts where `normalized_artifact_name == output_name`.
3. **Exact raw match:** resolve against catalog contracts where `raw_artifact_name == output_name`.
4. **Versioned fallback:** if no exact match, strip common version suffixes in Swift-compatible order (`_vN`, `-vN`, etc.) and retry exact normalized/raw lookups.
5. **Stem inference fallback:** as a final compatibility path, match by stem against declared artifact stems.

If all of these miss, continue as `NoContractDeclared`.

This sequence must be implemented in Rust compiler/execution binding so explicit `output_contract` values like `proposal_review_v1` remain authoritative, not bypassed by normalized-name-only heuristics.

**Artifact name resolution:**

1. **Primary:** `normalized_artifact_name` from the resolved contract metadata.
2. **Fallback alias:** `raw_artifact_name` when no normalized match is resolved.
3. **Compatibility:** raw basename heuristics are discovery-only and must not own contract binding.

**Companion artifact handling for `structured_with_human_companion`** (matching Swift `OutputContractSchemaV2`):

When `validation_mode == "structured_with_human_companion"` and `human_format` is set (defaults to `"markdown"` if not specified — matching Swift line 72-73):
- The machine artifact is the primary output (validated against required_fields as above)
- The companion artifact uses `raw_artifact_name` as its name (e.g. `proposal_review_summary_raw`) while the machine artifact uses `normalized_artifact_name` (e.g. `proposal_review_summary`)
- During discovery, both are expected: the machine output (from envelope or filesystem) and the companion (from filesystem, typically a `.md` file)
- If machine output is present and valid and companion is present at `raw_artifact_name` → `Passed`
- If machine output is missing but companion exists → `Failed` (machine payload required)
- If machine output is present and valid but companion is missing → `Failed` (human companion required)
- The companion is persisted alongside the machine artifact at the canonical `raw_artifact_name` path

### 2d. Persistence Ordering (ArtifactPersistenceOrderingPolicy model)

The executor follows this strict order:

1. **Persist raw outputs** — write discovered artifacts to canonical paths
2. **Run validation** — `validate_task_outputs()` against compiled schemas
3. **On failure: persist ValidationFailureRecord** — durable artifact
4. **Settle work item** — Completed if all passed, Failed if any failed

Raw outputs are **always persisted first**, even if validation will fail. This ensures recovery/report readers can inspect what the agent actually produced.

### 2e. ValidationFailureRecord (Durable Failure Evidence)

```rust
// domain/src/validation.rs

pub struct ValidationFailureRecord {
    pub id: String,
    pub timestamp: DateTime<Utc>,
    pub agent_id: String,
    pub stage_id: String,
    pub stage_execution_id: String,
    pub agent_execution_id: String,
    pub run_id: RunId,
    pub output_results: Vec<OutputValidationResult>,
    pub failure_summary: String,
    pub failure_class: ValidationFailureClass,
    pub contract_metadata: Vec<ContractValidationMetadata>,
    pub raw_output_exists: bool,
    pub receipt_exists: bool,
    pub transcript_exists: bool,
    pub recovery_recommendation: RecoveryRecommendation,
}

pub enum ValidationFailureClass {
    OutputContractMismatch,
    NoOutputProduced,
    EmptyOutput,
    PersistenceFailure,
}

pub struct ContractValidationMetadata {
    pub output_name: String,
    pub contract_id: String,
    pub machine_format: String,
    pub validation_mode: String,
    pub required_field_count: usize,
    pub raw_artifact_name: Option<String>,
    pub normalized_artifact_name: Option<String>,
}

pub struct RecoveryRecommendation {
    pub action: String,  // "retry_failed_agent" | "retry_failed_stage" | "operator_inspection"
    pub explanation: String,
}
```

**Persistence:** Serialized as JSON, written to `{canonical_artifact_path}/validation_failure_{agent_id}.json` via `ArtifactManager` (not `work_items.payload_json`).

This record must preserve the full stable failure-evidence continuity that current Swift report and recovery readers already consume. In particular, Rust V1 parity includes:
- `receipt_exists`
- `transcript_exists`
- both `raw_artifact_name` and `normalized_artifact_name` in contract metadata

These fields are durable evidence, not best-effort re-derivations during report assembly.

**Readers:**
- Failed-stage evidence builder (P048) consumes `ValidationFailureRecord` for stage-owned evidence packets
- Recovery coordinator uses `recovery_recommendation` to suggest operator action
- Northbound surfaces (see 2g below)

### 2f. DB Schema Addition

```sql
-- In migration 005 or appended to 004
CREATE TABLE IF NOT EXISTS validation_failure_records (
    id TEXT PRIMARY KEY,
    run_id TEXT NOT NULL REFERENCES runs(id),
    stage_id TEXT NOT NULL,
    agent_id TEXT NOT NULL,
    stage_execution_id TEXT NOT NULL REFERENCES stage_executions(id),
    agent_execution_id TEXT NOT NULL REFERENCES agent_executions(id),
    timestamp TEXT NOT NULL,
    failure_class TEXT NOT NULL,
    failure_summary TEXT NOT NULL,
    record_json TEXT NOT NULL,  -- full ValidationFailureRecord as JSON
    recovery_action TEXT
);

CREATE INDEX idx_vfr_run_id ON validation_failure_records(run_id);
CREATE INDEX idx_vfr_run_stage_execution ON validation_failure_records(run_id, stage_execution_id);
CREATE INDEX idx_vfr_run_stage_agent_execution ON validation_failure_records(run_id, stage_execution_id, agent_execution_id);
```

### 2g. Northbound Reader Wiring — Projection and Current MCP Surface

Validation status must reach operator/report readers through the **existing** daemon reader chain. Current Rust read surfaces relevant to this:

- `stages.retry` (`mcp-server/src/tools/stages.rs`) — command tool, no read surface
- `reports.get` (`mcp-server/src/tools/reports.rs`) — reads artifacts with `report_kind.is_some()`
- `runs.get` / `runs.list` — reads run-level data
- MCP resources:
  - `chainworks://runs/{run_id}/stages` — current stage resource backed by `StageSummaryRow`
  - `report://{run_id}` — current report resource backed by canonical run/report artifacts
- GraphQL types: `GqlRun`, `GqlStageExecution`, `GqlArtifact` — projection-backed for stage/run summary and artifact-backed for report surfaces

**Current MCP does NOT have a `stages.list` or `stages.get` read tool.** Stage read access flows through projection-backed `stage_summaries` (exposed via GraphQL `GqlStageExecution`).

**Stage-level validation status:**

Add `has_validation_failure BOOLEAN DEFAULT FALSE` to `stage_summaries` (the projection table). During `projections::rebuild_all_for_run`, derive from `validation_failure_records` by exact `stage_execution_id`, not by logical `stage_id`:

```sql
UPDATE stage_summaries SET has_validation_failure = TRUE
WHERE stage_execution_id IN (
    SELECT DISTINCT vfr.stage_execution_id
    FROM validation_failure_records vfr
    WHERE vfr.run_id = stage_summaries.run_id
);
```

The GraphQL `GqlStageExecution` type (which reads from `stage_summaries`) gets `has_validation_failure: bool`. The existing MCP stage resource `chainworks://runs/{run_id}/stages` must read the same projection-backed field, not a second canonical copy.

**Artifact-level validation status:**

Validation failure records are persisted as artifacts with `report_kind = "validation_failure"` and a stored `record_json` payload.

`GqlArtifact` must decode the full payload for these artifacts into a concrete `validationFailureRecord` shape when `report_kind == "validation_failure"`, so that readers get fields that stable readers already expect (`failureSummary`, `missingFields`, `failureClass`, `recoveryRecommendation`, plus output-level metadata).

The existing `reports.get` MCP tool already filters artifacts by `report_kind.is_some()`. `report://{run_id}` must include these artifacts through the same current assembly path and preserve the decoded payload in its report artifact output.

Concrete northbound path for decoded failure data:

- `validation_failure_records.record_json` is the authoritative typed payload source
- `db/src/repos/validation.rs` provides lookup by `agent_execution_id`, `stage_execution_id`, and artifact identity as needed by current readers
- `graphql-server/src/types/artifact.rs` joins/loads `record_json` for `report_kind = "validation_failure"` and decodes it into the typed `validationFailureRecord`
- `mcp-server/src/tools/reports.rs` includes the same decoded payload in `reports.get`
- `mcp-server/src/server.rs` includes the same decoded payload in `report://{run_id}`
- `GqlStageExecution.has_validation_failure` from `stage_summaries` remains the lightweight status bit; the decoded record is delivered through the artifact/report surfaces, not reconstructed from booleans
- `validationFailureRecord` decoding includes stable reader fields (`failureSummary`, `missingFields`, `failureClass`, `recoveryRecommendation`, plus output-level metadata), not only `report_kind` presence.

No new MCP tool is introduced.

**No new `stages.list` MCP tool is introduced.** Stage validation status is surfaced through the projection-backed GraphQL types and the `reports.get` artifact lane.

**Projection rebuild:**

In `db/repos/projections.rs` `rebuild_all_for_run`, add a step that sets `has_validation_failure = true` on stage projections when a matching `validation_failure_records` row exists. This keeps the projection table authoritative for read-heavy queries.

**Files affected:**

| File | Change |
|---|---|
| `db/src/repos/projections.rs` | Derive `has_validation_failure` during projection rebuild |
| `db/src/repos/validation.rs` | Provide attempt-aware reads of `ValidationFailureRecord.record_json` for GraphQL / MCP readers |
| `graphql-server/src/types/artifact.rs` | Decode `validation_failure_records.record_json` into typed `validationFailureRecord` payload on `GqlArtifact` |
| `graphql-server/src/types/stage.rs` | Add `has_validation_failure: bool` to `GqlStageExecution` |
| `mcp-server/src/tools/reports.rs` | Include decoded `validationFailureRecord` payload in `reports.get` for `report_kind = "validation_failure"` |
| `mcp-server/src/server.rs` | Ensure `chainworks://runs/{run_id}/stages` reads `has_validation_failure` from `StageSummaryRow`, and `report://{run_id}` includes validation-failure artifacts plus decoded `validationFailureRecord` payload through the current report resource path |

---

## 3. Files to Create/Modify

| File | Change |
|---|---|
| `acp/src/transport.rs` | Add `extract_output_envelopes()`, merge with filesystem diff results |
| `acp/src/lib.rs` | Extend `ExecutionResult` with `discovered_artifacts: Vec<DiscoveredArtifact>` |
| `domain/src/validation.rs` | **NEW** — `ValidationFailureRecord`, `OutputValidationResult`, `RecoveryRecommendation` |
| `engine/src/contracts.rs` | **NEW** — `validate_output()`, `validate_task_outputs()` with full validation-mode parity (`human_only`, `strict_structured`, `structured_with_human_companion`) |
| `workflow/src/catalog.rs` | Extend `ContractDef` parsing with `human_format` so full schema parity is ingestible from YAML |
| `workflow/src/plan.rs` | Extend `OutputSchema` with `validation_mode`, `machine_format`, `human_format`, `normalized_artifact_name`, `raw_artifact_name` |
| `workflow/src/compiler.rs` | Extract additional contract fields from catalog into `OutputSchema`, including `human_format` |
| `engine/src/executor.rs` | Persistence ordering: persist raw → validate → persist failure record → settle. Metadata binding from compiled plan. |
| `db/migrations/005_validation_records.sql` | **NEW** — `validation_failure_records` table |
| `db/src/repos/validation.rs` | **NEW** — CRUD for validation failure records |
| `db/src/repos/projections.rs` | Derive `has_validation_failure` on `stage_summaries` during `rebuild_all_for_run` from `validation_failure_records` by exact `stage_execution_id` |
| `db/migrations/005_validation_records.sql` | Also add `has_validation_failure BOOLEAN DEFAULT FALSE` to `stage_summaries` |
| `graphql-server/src/types/stage.rs` | Add `has_validation_failure: bool` to `GqlStageExecution` (reads from projection `stage_summaries`, not raw `stage_executions`) |
| `graphql-server/src/types/artifact.rs` | Surface decoded `ValidationFailureRecord` payload from `validation_failure_records.record_json` on validation-failure artifacts |
| `mcp-server/src/tools/reports.rs` | Surface decoded `ValidationFailureRecord` payload in `reports.get` from `validation_failure_records.record_json` |
| `mcp-server/src/server.rs` | Bind validation status / validation-failure artifacts to the existing MCP resources `chainworks://runs/{run_id}/stages` and `report://{run_id}`, using `validation_failure_records.record_json` as the typed payload source |
| `engine/src/lib.rs` | Register `pub mod contracts` |

---

## 4. Acceptance Criteria

1. Agent response containing `<<<CHAINWORKS_OUTPUT:name>>>` blocks → artifacts extracted, bound to canonical metadata from compiled plan, and persisted at canonical paths.
2. Filesystem-diff artifacts still discovered (backward compat). Envelope outputs override filesystem outputs with the same name.
3. Discovered artifacts inherit `contract_id`, `format`, and `required_fields` from `CompiledTask.output_schemas` — not from provider-scoped stubs.
4. If an agent returns `proposal_review_summary` via envelope with missing `average_score` field → raw output is persisted first, then `ValidationFailureRecord` is persisted with `failure_class: OutputContractMismatch` and `missing_fields: ["average_score"]`, then work item settles as Failed.
5. `validation_failure_records` table has an attempt-aware row keyed for report/recovery queries, including `stage_execution_id` and `agent_execution_id`, so failed first attempts remain inspectable without smearing later retries.
6. Projection-backed `stage_summaries.has_validation_failure` is `true` only for the exact `stage_execution_id` that owns a `validation_failure_records` row. GraphQL `GqlStageExecution.hasValidationFailure` and MCP resource `chainworks://runs/{run_id}/stages` both read from that projection. A later successful retry in the same run does not inherit a stale flag from an earlier failed attempt.
7. `human_only` validation mode → `Passed` (no machine check). `strict_structured` with missing JSON fields → `Failed`. `structured_with_human_companion` with missing JSON fields → `Failed`.
8. Non-JSON `machine_format` with non-empty content → `Passed`. Empty content → `Failed`.
9. `OutputSchema` in compiled plan carries `validation_mode`, `machine_format`, and `human_format` from catalog contracts — not just `format` and `required_fields`.
10. Recovery recommendation in `ValidationFailureRecord` suggests `retry_failed_agent` for contract mismatch, `operator_inspection` for no output produced.
11. Persisted `ValidationFailureRecord` preserves `receipt_exists`, `transcript_exists`, and both raw/normalized artifact identity fields so failed-stage evidence, run reports, recovery, GraphQL, and MCP resources consume the same durable failure truth.
12. `reports.get`, `report://{run_id}`, and `GqlArtifact` expose the decoded `ValidationFailureRecord` payload from `validation_failure_records.record_json`, not just artifact metadata or `report_kind` presence.

---

## 5. Out of Scope

- **StructuredOutputSchemaGate preflight**: Transport-capability validation before execution. Not a P046 concern — it's a pre-run catalog check, not post-generation validation.
- **Broader thin-client northbound design**: How validation status flows through GraphQL/MCP is surfacing, not validation logic.
- **Retry policy for validation failures**: Whether retry requires re-approval depends on stage type (covered by P044 for release gates). P046 produces the failure record; retry policy is the orchestrator's concern.
