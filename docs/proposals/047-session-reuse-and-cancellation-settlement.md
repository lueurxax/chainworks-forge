# Proposal 047: Session Lineage, Context Budget, and Cancellation Settlement

| Field | Value |
|---|---|
| Date | 2026-04-14 |
| Status | Draft |
| Author | Claude |
| Depends on | None |
| Scope | (A) Durable session lineage with immutable generations, invocation owner keys, binding fingerprints, and append-only events — matching Swift's `AgentSessionLineage` / `AgentSessionGeneration` / `SessionReusePolicy` owner chain. (B) Generation-scoped context budget evaluation driven by economic signals, not prompt-size heuristics. (C) Two-phase cancellation settlement with structured evidence, `Cancelled` status variants, and per-session close outcomes. |
| Goal | The Rust daemon persists session lineage truth, reuses sessions safely across loop iterations, evaluates budget based on generation economics, and settles cancellations through a two-phase `cancelling → cancelled` contract with durable evidence — matching the stable Swift owner chain. |

---

## 1. Context and Motivation

### 1a. Session Reuse and Lineage

The agent catalog defines reuse policies:

```yaml
proposal_writer:
  session_reuse_scope: same_agent_family_within_run
  session_family_id: proposal_authoring_loop
```

The Rust daemon creates a fresh ACP session for every `InvokeAgent` work item. The stable Swift model is **not** a transient in-memory session map — it is a durable lineage chain:

- **`AgentSessionLineage`**: persisted record linking `(run_id, agent_id, family_id)` to an ordered list of generations and append-only events.
- **`AgentSessionGeneration`**: immutable record per session lifecycle — captures `invocationOwnerKey` (who started it), `bindingFingerprint` (SHA-256 of the full agent binding: provider, model, prompt, skill, worktree mode, MCP inventory, etc.), turn count, cumulative tokens, cumulative cost, and close reason.
- **`SessionReusePolicy.evaluate()`**: reads the last generation's status and end reason to produce a `SessionReuseDisposition` (10 cases: `reused`, `fresh_after_reset`, `fresh_after_budget`, `fresh_session_required`, etc.).
- **Binding fingerprint mismatch** (e.g. prompt changed between iterations) → `fresh_session_required`, regardless of family scope.
- **Family scope** (`same_agent_family_within_run`) relaxes `invocationOwnerKey` matching but keeps fingerprint verification mandatory.

### 1b. Context Budget

Swift's `ContextBudgetGuard` is **generation-scoped and economics-driven**, not a prompt-size heuristic. Decision signals:

| Signal | Type | Source |
|--------|------|--------|
| Turn count | Hard guardrail | Generation `turnCount` |
| Estimated input tokens | Hard guardrail | Generation `estimatedInputTokens` |
| Cumulative prompt tokens | Hard guardrail | Generation `cumulativePromptTokens` |
| Cumulative cost (cents) | Hard guardrail | Generation `cumulativeCostCents` |
| Idle age (seconds) | Hard guardrail | `now - lastActivityAt` |
| Transcript growth ratio | Economic | Current input / fresh-session baseline |
| Cached token share | Economic | Provider cache metadata |
| Normalized savings vs. fresh | Economic | Net cost difference |
| Effective prompt size fraction | Economic | Fraction of context window used |
| Compaction churn count | Economic | Prior compaction events on this generation |

Decision: `continueReuse` | `compact(reason)` | `invalidate(reason)`. Prompt trimming is one remediation tool inside `compact`, not the decision owner.

### 1c. Cancellation Settlement

The Rust daemon's `CancelRun` marks the run as `Cancelling`, cleans up the worktree, and returns. But:
- ACP subprocesses continue running
- In-flight work items stay `Running` forever (no `Cancelled` variant in `WorkItemStatus`)
- No `Cancelled` variant in `StageStatus`
- No session close outcomes persisted
- No cancellation settlement log
- `cancellation_settled_at` is never written (only `cancellation_requested_at`)

Swift uses a **two-phase** contract:
1. **Phase 1 (`beginSettlement`)**: All in-flight agent executions → `.cancelled`. Settlement entries created with `sessionCloseSucceeded: nil`. Run stays `cancelling`.
2. **Async session close**: Per-session 10s timeout. Returns `SessionCloseOutcome { sessionID, attempted, succeeded }`.
3. **Phase 2 (`finalizeSettlement`)**: Entries updated with actual close outcomes. `cancellation_settled_at` written. Run → `.cancelled`.

Only after Phase 2 does the run appear as `cancelled` to operator/report readers. The settlement log (JSON array of `CancellationSettlementEntry`) is persisted on the Run.

---

## 2. Design

### 2a. Session Lineage Data Model

**Legacy schema reality:** `session_lineages` is **not** greenfield on Rust installs. `control-plane/crates/db/migrations/002_projections.sql` already creates a table named `session_lineages` with the old projection-era shape:

- `stage_id`
- `lineage_kind`
- `previous_session_id`
- no immutable generation rows
- no active-generation pointer
- no invocation owner key / binding fingerprint / close reason contract

That schema is incompatible with the durable lineage owner proposed here. P047 therefore requires an explicit migration path instead of treating `session_lineages` as new.

**Migration contract for existing installs:**

1. In `006_session_lineage.sql`, rename the legacy table to `session_lineages_legacy`.
2. Create the new canonical `session_lineages`, `session_generations`, and `session_events` tables.
3. Do **not** synthesize new-generation history from legacy rows. The old table does not carry enough truth to reconstruct immutable generations, binding fingerprints, or invocation ownership safely.
4. Existing reads/writes switch exclusively to the new canonical tables after migration.
5. `session_lineages_legacy` is retained temporarily for audit/provenance only and is not read by policy evaluation, budget evaluation, or northbound readers.
6. A later cleanup proposal may drop `session_lineages_legacy` once migration coverage and operator confidence are complete.

**Canonical DB tables after migration:**

```sql
CREATE TABLE session_lineages (
    id TEXT PRIMARY KEY,
    run_id TEXT NOT NULL REFERENCES runs(id),
    agent_id TEXT NOT NULL,
    lineage_id TEXT NOT NULL,
    session_reuse_scope TEXT NOT NULL,  -- "none" | "same_invocation_owner" | "same_agent_family_within_run"
    session_family_id TEXT,
    active_generation_id TEXT,
    created_at TEXT NOT NULL,
    closed_at TEXT
);

CREATE TABLE session_generations (
    id TEXT PRIMARY KEY,
    lineage_id TEXT NOT NULL REFERENCES session_lineages(id),
    generation INTEGER NOT NULL,
    invocation_owner_key TEXT NOT NULL,
    provider_session_id TEXT,
    binding_fingerprint TEXT NOT NULL,
    rehydrated_from_checkpoint_artifact_id TEXT,
    working_directory TEXT NOT NULL,
    workspace_mode TEXT NOT NULL,       -- "read_only" | "read_write"
    runtime_provider TEXT NOT NULL,
    runtime_model TEXT NOT NULL,
    status TEXT NOT NULL,               -- "active" | "invalidated" | "closed" | "reset"
    turn_count INTEGER NOT NULL DEFAULT 0,
    cumulative_prompt_tokens INTEGER NOT NULL DEFAULT 0,
    cumulative_cost_cents INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    ended_at TEXT,
    end_reason TEXT
);

CREATE TABLE session_events (
    id TEXT PRIMARY KEY,
    lineage_id TEXT NOT NULL REFERENCES session_lineages(id),
    generation_id TEXT NOT NULL,
    event_type TEXT NOT NULL,           -- "created" | "reused" | "invalidated" | "closed" | "operator_reset" | "budget_exceeded" | "compacted"
    recorded_at TEXT NOT NULL,
    details_json TEXT
);
```

### 2b. Invocation Owner Key and Binding Fingerprint

**InvocationOwnerKey (matching Swift `InvocationOwnerKeyBuilder`):**

```
{run_id}:{agent_id}:{stage_lineage_id}:{task_name}:{owner_execution_lineage_id}
```

- `stage_lineage_id`: stable identifier for the logical stage across retries (= `stage_id` in current Rust; distinct from transient `stage_execution_id` which changes per attempt)
- `owner_execution_lineage_id`: identifier of the execution lineage that started or claimed this generation (enables recovery-branch compatibility checking — if a retry creates a new execution lineage, the old generation's owner is different)

Built once per enqueue. **Immutable** on the generation. Used for `same_invocation_owner` scope matching and recovery-branch fail-closed verification.

**BindingFingerprint (matching Swift `BindingFingerprintBuilder`):** SHA-256 of sorted canonical binding components:
- `agent_id`, `provider`, `model`, `effort`
- Full system prompt text (includes skill injection)
- Working directory, workspace mode
- `worktree_write_enabled`, `worktree_strategy`
- Sorted inputs/outputs inventory
- `backend_profile` identifier
- Permission profile, MCP server inventory
- Skill snapshot hash, skillRef, skillRole
- `output_contract`, `max_turns`, `temperature`

Built at prompt construction time. If fingerprint changes between iterations (e.g. prompt was modified, MCP inventory changed), reuse is rejected → `FreshSessionRequired`.

### 2c. Reuse Policy Evaluation

```rust
// engine/src/session/policy.rs

pub enum SessionReuseDisposition {
    Fresh,                          // cold start — no lineage exists
    Reused,                         // active generation matched all criteria
    ReusedAfterResume,              // resumed from checkpoint after prior close
    FreshAfterReset,                // operator reset (via ResetSession command)
    FreshAfterInvalidation,         // generic invalidation fallback
    FreshAfterBudget,               // budget guardrail triggered
    FreshAfterCompaction,           // compaction event
    FreshAfterTransportError,       // transport failure
    FreshAfterTimeout,              // timeout
    FreshSessionRequired,           // binding fingerprint mismatch
    UnverifiableSessionHistory,     // active generation not found in lineage; fail closed
}

pub fn evaluate(
    lineage: &SessionLineage,
    current_owner_key: &str,
    current_fingerprint: &str,
    current_recovery_branch_id: Option<&str>,
) -> SessionReuseDisposition;
```

**Logic (matching Swift `SessionReusePolicy.evaluate`):**
1. No lineage → `Fresh`
2. Lineage exists, no active generation:
   - Check last ended generation's `end_reason` → map to corresponding `FreshAfter*`
   - Generic invalidation with no specialized reason → `FreshAfterInvalidation`
   - If last generation has checkpoint → `ReusedAfterResume` (resume from checkpoint)
3. Active generation exists:
   - Active generation not in lineage's generations array → `UnverifiableSessionHistory` (fail closed)
   - Fingerprint mismatch → `FreshSessionRequired`
   - Scope is `none` → `FreshSessionRequired`
   - Scope is `same_invocation_owner`:
     - Owner key mismatch → `FreshSessionRequired`
     - Recovery branch ID mismatch (if both present) → `FreshSessionRequired` (fail closed on retry drift)
   - Scope is `same_agent_family_within_run`:
     - Owner key check **relaxed** (multiple legitimate owners may share)
     - Recovery branch check **relaxed** for family scope
     - Fingerprint check **mandatory**
   - All checks pass → `Reused`

### 2d. ACP Session Resume

Live ACP reuse needs a **transport-lifetime owner**, not just a stored `sessionId`.
Current Rust ACP is one-shot: adapters spawn a fresh subprocess per invoke and `transport.rs` always runs `session/close` plus shutdown after one prompt. Swift reuse works because the transport owns active subprocess/session handles in memory and `RuntimeSessionBridge` can submit another prompt into that already-live session.

**Rust owner for persistent live sessions:**

- `acp::manager::AcpRuntimeManager` becomes the process-lifetime owner of reusable live ACP sessions.
- It holds an in-memory `active_sessions` map keyed by `session_generation_id` (and validated against `provider_session_id`) to `ActiveAcpSessionHandle`.
- `ActiveAcpSessionHandle` owns the live subprocess manager / stdio pipes / initialized transport state / provider session ID / adapter family.
- The engine never owns raw ACP subprocess handles. It only asks `AcpRuntimeManager` to:
  - start a fresh session for a new generation,
  - submit a prompt into an existing active generation,
  - close a generation's live session,
  - invalidate and drop a stale handle.

**Required runtime invariant:**

- DB lineage truth is necessary but not sufficient for live reuse.
- `SessionReuseDisposition::Reused` is valid only when both:
  - the lineage's active generation matches policy checks, and
  - `AcpRuntimeManager` still has a matching live `ActiveAcpSessionHandle` for that generation / provider session.
- If DB says a generation is active but no live handle exists, reuse must fail closed. The generation is not silently treated as reusable just because a `provider_session_id` string exists.

That missing-live-handle case must fall through to:

- `ReusedAfterResume` when checkpoint-backed resume is available, or
- a fresh-generation path (`FreshAfterTransportError` / `FreshAfterInvalidation`) when no trustworthy resume path exists.

When disposition is `Reused`, the executor submits `session/prompt` through the existing `ActiveAcpSessionHandle` owned by `AcpRuntimeManager`, using the persisted `generation.provider_session_id` only as a validation field. Reuse is therefore transport-backed, not just string-backed.

When disposition is `ReusedAfterResume`, the executor rehydrates from the last checkpoint artifact, creates a new generation with `rehydrated_from_checkpoint_artifact_id` persisted on `session_generations`, starts a fresh live ACP session through `AcpRuntimeManager`, and sends `session/prompt` with checkpoint context. This field is part of the durable owner chain, not reconstructible from events alone.

When disposition is `FreshAfter*` or `FreshSessionRequired`, a new ACP session is created through `AcpRuntimeManager`, a new generation is appended to the lineage, and the old generation is marked with the corresponding end reason.

When disposition is `UnverifiableSessionHistory`, a new session is forced with a warning event logged — fail closed, never resume an unverifiable generation.

### 2d-ii. Execution-Side Session Provenance (AgentExecution Fields)

Session lineage/generation truth must also be persisted on the **agent execution record** so report/recovery readers can trust execution provenance without traversing lineage tables. Matching Swift `AgentExecution` (lines 80-88):

**New columns on `agent_executions` table:**

```sql
ALTER TABLE agent_executions ADD COLUMN session_lineage_id TEXT;
ALTER TABLE agent_executions ADD COLUMN session_generation_id TEXT;
ALTER TABLE agent_executions ADD COLUMN rehydrated_from_checkpoint_artifact_id TEXT;
ALTER TABLE agent_executions ADD COLUMN invocation_owner_key TEXT;
ALTER TABLE agent_executions ADD COLUMN session_reuse_scope TEXT;
ALTER TABLE agent_executions ADD COLUMN session_family_id TEXT;
ALTER TABLE agent_executions ADD COLUMN session_reuse_disposition TEXT;
ALTER TABLE agent_executions ADD COLUMN session_reset_reason TEXT;
```

**When populated:** The executor writes these fields after policy evaluation and before ACP session start:
- `session_lineage_id` → the lineage record this execution belongs to
- `session_generation_id` → the specific generation (new or reused)
- `invocation_owner_key` → the owner tuple used for reuse decisions
- `session_reuse_disposition` → the concrete disposition (`Reused`, `FreshAfterBudget`, etc.)
- `session_reset_reason` → if disposition is `FreshAfterReset`, the operator-provided reason

**Why execution-first:** Report builders, comparison readers, and recovery surfaces read `agent_executions` as the primary truth for what happened during an execution. If they had to join through `session_lineages → session_generations` to discover reuse disposition, the query cost and coupling would be higher. The execution record is the canonical "what happened" surface; lineage tables are the "why it happened" history.

### 2e. Context Budget Evaluation

```rust
// engine/src/session/budget.rs

pub struct BudgetSignals {
    // Hard guardrail inputs (from persisted generation)
    pub turn_count: u32,
    pub estimated_input_tokens: i64,           // current estimated prompt size
    pub cumulative_prompt_tokens: i64,
    pub cumulative_cost_cents: i64,
    pub idle_age_seconds: f64,
    // Economic signal inputs (from provider runtime metadata)
    pub transcript_growth_ratio: Option<f64>,
    pub cached_token_share: Option<f64>,       // 0.0–1.0; fraction of input tokens cached
    pub normalized_savings_versus_fresh: Option<f64>, // positive = reuse cheaper
    pub effective_prompt_size_fraction: Option<f64>,   // 0.0–1.0; fraction of context window
    pub compaction_churn_count: u32,
}

pub struct BudgetConfig {
    pub max_turns: u32,                        // default 20
    pub max_estimated_input_tokens: i64,       // default 128_000
    pub max_cumulative_prompt_tokens: i64,     // default 1_000_000
    pub max_cumulative_cost_cents: i64,        // default 500
    pub max_idle_age_seconds: f64,             // default 14_400 (4h)
    pub max_transcript_growth_ratio: f64,      // default 2.0
}

pub enum BudgetDecision {
    ContinueReuse,
    Compact { reason: String },
    Invalidate { reason: String },
}

pub fn evaluate(signals: &BudgetSignals, config: &BudgetConfig) -> BudgetDecision;
```

**Decision logic (matching Swift `ContextBudgetGuard`):**
1. **Hard guardrails first**:
   - `turn_count >= max_turns` → `Compact`
   - `estimated_input_tokens >= max_estimated_input_tokens` → `Compact`
   - `cumulative_prompt_tokens >= max_cumulative_prompt_tokens` → `Invalidate`
   - `cumulative_cost_cents >= max_cumulative_cost_cents` → `Invalidate`
   - `idle_age_seconds >= max_idle_age_seconds` → `Invalidate`
2. **Economic signals** (if available):
   - `effective_prompt_size_fraction > 0.5` → `Compact`
   - `cached_token_share < 0.2 AND estimated_input_tokens > 50_000` → `Compact`
   - `transcript_growth_ratio > max_transcript_growth_ratio` → `Compact`
   - `normalized_savings_versus_fresh < -0.05` (reuse is 5¢+ more expensive) → `Invalidate`
   - `compaction_churn_count >= 3` → `Invalidate`
3. Otherwise → `ContinueReuse`

Budget evaluation runs **before** sending `session/prompt` on a reused session. On `Compact`, the current generation is ended, a checkpoint event is recorded, and a new generation starts. On `Invalidate`, the generation is ended with `budget_exceeded` and `FreshAfterBudget` disposition triggers on the next invocation. This preserves continuity for the stable max-turn and max-estimated-input compaction cases while still failing closed on the harder token/cost/idle/economic cases.

### 2f. Cancellation Settlement — Two-Phase Contract

**Phase 1: `begin_settlement`** (in `command_handler.rs` CancelRun, synchronous):

1. `run.cancellation_requested_at` ← now (already implemented)
2. Find all active agent executions for the run (`Running` / `Pending` / `Ready`) and transition each to terminal `Cancelled`
3. Build `CancellationSettlementEntry` per agent execution with `session_close_succeeded: None`
4. Serialize entries as JSON → `run.cancellation_settlement_log`
5. Supporting queue cleanup: find all `Running` work items for the run and mark each as `Cancelled` (requires new `WorkItemStatus::Cancelled` variant)
6. Find all `Running` stages → mark as `Failed` once their agent executions are terminal (existing variant; stages have no `Cancelled`)
7. Run stays `Cancelling` — **not** `Cancelled` yet

**Async session close** (background task after Phase 1):

```rust
pub struct SessionCloseOutcome {
    pub session_id: String,
    pub attempted: bool,
    pub succeeded: bool,
}

pub async fn close_runtime_sessions(
    session_ids: Vec<String>,
    timeout: Duration,  // 10s
) -> Vec<SessionCloseOutcome>;
```

Per-session: send SIGTERM to ACP subprocess, wait up to timeout, SIGKILL if needed.

**Phase 2: `finalize_settlement`** (after async close completes):

1. Read preliminary settlement entries from `cancellation_settlement_log`
2. Update each with actual `session_close_succeeded` from outcomes
3. Re-serialize → `run.cancellation_settlement_log`
4. `run.cancellation_settled_at` ← now
5. `run.status` ← `Cancelled`

**Domain model changes:**

```rust
// db/src/work_item.rs
pub enum WorkItemStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,   // NEW
}

// domain/src/run.rs — already has cancellation fields, add:
pub cancellation_settlement_log: Option<String>,  // JSON
```

**CancellationSettlementEntry:**

```rust
pub struct CancellationSettlementEntry {
    pub agent_execution_id: String,
    pub agent_id: String,
    pub prior_status: String,
    pub terminal_status: String,    // "cancelled"
    pub session_close_attempted: bool,
    pub session_close_succeeded: Option<bool>,  // None in Phase 1, Some in Phase 2
    pub settled_at: DateTime<Utc>,
}
```

---

## 3. Files to Create/Modify

| File | Change |
|---|---|
| **Session lineage** | |
| `db/migrations/006_session_lineage.sql` | **NEW** — rename legacy `session_lineages` to `session_lineages_legacy`; create canonical `session_lineages`, `session_generations`, `session_events`; add session provenance columns to `agent_executions` |
| `db/src/repos/sessions.rs` | **NEW** — CRUD for lineage, generations, events, including `rehydrated_from_checkpoint_artifact_id` |
| `db/src/repos/agent_executions.rs` | Persist session provenance fields on agent execution records (see §2d-ii) |
| `domain/src/session.rs` | **NEW** — `SessionLineage`, `SessionGeneration`, `SessionReuseDisposition`; `SessionGeneration` carries durable checkpoint provenance |
| `domain/src/agent.rs` | Add session provenance fields to `AgentExecution` domain struct |
| `engine/src/session/mod.rs` | **NEW** — Session management module |
| `engine/src/session/policy.rs` | **NEW** — `SessionReusePolicy::evaluate()` |
| `engine/src/session/fingerprint.rs` | **NEW** — `InvocationOwnerKeyBuilder`, `BindingFingerprintBuilder` |
| `workflow/src/plan.rs` | Add `session_reuse_scope`, `session_family_id` to `ResolvedAgent` |
| `workflow/src/compiler.rs` | Extract reuse fields from catalog |
| `acp/src/manager.rs` | Become the process-lifetime owner of `ActiveAcpSessionHandle` values keyed by active generation / provider session |
| `acp/src/transport.rs` | Split one-shot session execution into reusable primitives: initialize/start session, prompt existing session, close session, shutdown |
| `acp/src/adapters/*.rs` | Stop treating ACP execution as spawn-once-and-close-once only; delegate reusable session lifetime to `AcpRuntimeManager` |
| `engine/src/executor.rs` | Lineage lookup → policy evaluation → reuse or fresh → persist generation → write provenance on agent execution |
| **Context budget** | |
| `engine/src/session/budget.rs` | **NEW** — `ContextBudgetGuard::evaluate()` with hard guardrails + economic signals |
| **Cancellation** | |
| `db/src/work_item.rs` | Add `Cancelled` variant to `WorkItemStatus` |
| `domain/src/run.rs` | Add `cancellation_settlement_log: Option<String>` |
| `db/migrations/006_session_lineage.sql` | Add `cancellation_settlement_log` column to runs; add `cancelled` to work_item status CHECK |
| `engine/src/command_handler.rs` | Phase 1: settle active agent executions, perform supporting work-item cleanup, update stages, build preliminary execution-first log |
| `engine/src/cancellation.rs` | **NEW** — `begin_settlement()`, `close_runtime_sessions()`, `finalize_settlement()` keyed to agent executions as canonical settlement truth |
| **Northbound reader wiring** | |
| `db/src/repos/projections.rs` | Add `cancellation_settlement_summary: Option<String>` to `RunProjectionRow`. During `rebuild_all_for_run`, derive from `cancellation_settlement_log`: `"{settled_count}/{total_count} agents settled, {close_ok} sessions closed"`. Do **not** project the full JSON log. |
| `graphql-server/src/schema.rs` | `QueryRoot.run(id)` reads canonical `Run` for single-run inspection (full log); `QueryRoot.runs` / list queries remain projection-backed (summary only) |
| `graphql-server/src/types/run.rs` | `GqlRun` gets both: `cancellation_settlement_log: Option<String>` (populated only from canonical Run path) and `cancellation_settlement_summary: Option<String>` (populated from projection). List queries hydrate summary only; single-run queries hydrate both. |
| `mcp-server/src/tools/runs.rs` | `runs.get` already serializes full `Run` via `serde_json::to_value` — new `cancellation_settlement_log` appears automatically. `runs.list` returns projection rows with `cancellation_settlement_summary`. |
| `engine/src/lib.rs` | Register `pub mod session`, `pub mod cancellation` |

---

## 4. Acceptance Criteria

### Session Lineage
1. `proposal_writer` in loop iteration 3 reuses the session from iteration 1 — same ACP `sessionId`, same live subprocess handle owned by `AcpRuntimeManager`, same lineage ID in DB, generation count increments only on close/invalidate.
2. Binding fingerprint change between iterations → `FreshSessionRequired`, new generation created.
3. Operator `ResetSession` → generation ends with "reset", next invocation gets `FreshAfterReset`.
4. `session_lineages`, `session_generations`, `session_events` tables populated with correct run/agent/family linkage.
5. `invocation_owner_key` and `binding_fingerprint` on each generation are immutable after creation.
6. Active generation not found in lineage array → `UnverifiableSessionHistory`, fail closed, new session forced.
7. `ReusedAfterResume`: checkpoint artifact ID recorded on new generation's `rehydrated_from_checkpoint_artifact_id`.
8. Recovery branch mismatch on `same_invocation_owner` scope → `FreshSessionRequired` (fail closed on retry drift).
9. Generic invalidation with no specialized budget/compaction/transport/timeout reason → `FreshAfterInvalidation`.
10. DB active generation with no matching live `ActiveAcpSessionHandle` in `AcpRuntimeManager` is **not** treated as `Reused`; the path falls through to checkpoint-backed resume or fresh-session fail-closed behavior.

### Legacy Migration
11. Existing installs with projection-era `session_lineages` migrate successfully: legacy table is renamed to `session_lineages_legacy`, new canonical tables are created, and policy/runtime readers ignore the legacy rows.
12. No synthetic generation backfill is performed from `session_lineages_legacy`; immutable generation truth starts only when the canonical schema begins recording it.

### Execution-Side Provenance
13. After policy evaluation, `agent_executions` row carries: `session_lineage_id`, `session_generation_id`, `invocation_owner_key`, `session_reuse_scope`, `session_family_id`, `session_reuse_disposition`, `session_reset_reason`.
14. Report/recovery readers can determine reuse disposition from `agent_executions` alone — no lineage table join required for the "what happened" question.

### Context Budget
15. After 20 turns on a reused session → budget evaluation returns `Compact`, checkpoint recorded, new generation starts.
16. `estimated_input_tokens >= 128_000` → `Compact`.
17. Cumulative cost exceeding 500¢ → `Invalidate`.
18. `normalized_savings_versus_fresh < -0.05` → `Invalidate` (reuse more expensive than fresh).
19. Transcript growth > 2.0x → `Compact`, new generation starts.
20. Budget signals (turn count, cost, tokens, estimated input) are read from the persisted generation, not reconstructed.

### Cancellation Settlement
21. `CancelRun` Phase 1: all active agent executions become terminal `Cancelled`, settlement entries keyed by `agent_execution_id` are persisted with `session_close_succeeded: None`, supporting `Running` work items are cleaned up to `Cancelled`, and the run stays `Cancelling`.
22. Phase 2: after async session close, entries updated with actual close outcomes, `cancellation_settled_at` written, run → `Cancelled`.
23. Between Phase 1 and Phase 2, `run.status` remains `Cancelling` (not `Cancelled`).
24. **Single-run reader:** `QueryRoot.run(id)` / MCP `runs.get` expose full `cancellation_settlement_log` (JSON) and `cancellation_settled_at` from canonical `Run`.
25. **List reader:** `QueryRoot.runs` / MCP `runs.list` expose `cancellation_settlement_summary: Option<String>` from `RunProjectionRow` — a human-readable one-line summary (e.g. `"3/3 agents settled, 2 sessions closed"`). Full JSON log is **not** projected into list rows.
26. No active agent executions remain after Phase 2.
27. No `Running` work items remain after Phase 2.

---

## 5. Test Gate

### test-gates.md Entry

```
### `proposal-047`

Session lineage, context budget, and cancellation settlement gate.

Command:

\`\`\`bash
./scripts/test-gate.sh proposal-047
\`\`\`
```

### test-gate.sh Entry

```bash
proposal-047|p047)
  log "Proposal 047 control-plane gate: session lineage + budget + cancellation"
  (
    cd "$ROOT_DIR/control-plane"
    cargo test --workspace 2>&1
  )
  log "Proposal 047 control-plane gate passed"
  ;;
```

---

## 6. Out of Scope

- **Session checkpoint serialization**: Checkpointing session state for cross-process resume is transport-specific. P047 tracks checkpoint artifact IDs but does not define the serialization format.
- **Provider-specific budget tuning**: Hard guardrail defaults match Swift. Per-provider overrides are a future config concern.
- **UI for session inspector**: Operator-facing session lineage browsing is a thin-client concern.
