# Proposal 065: Operator Retry Instruction Contract

| Field | Value |
|---|---|
| Date | 2026-04-21 |
| Status | Draft |
| Author | Andrey Khasanov |
| Depends on | [045-run-recovery-and-granular-retry-mcp-tools.md](045-run-recovery-and-granular-retry-mcp-tools.md), [052-orchestrator-loop-budget-source-of-truth.md](052-orchestrator-loop-budget-source-of-truth.md), [output-contracts-failure-evidence-and-recovery.md](../reference/output-contracts-failure-evidence-and-recovery.md), [execution-truth-and-recovery.md](../reference/execution-truth-and-recovery.md), [061-sqlite-write-serialization-and-executor-backpressure.md](061-sqlite-write-serialization-and-executor-backpressure.md), [064-run-worktree-main-sync-and-cross-run-knowledge-transfer.md](064-run-worktree-main-sync-and-cross-run-knowledge-transfer.md) |
| Scope | Add a durable, operator-owned retry instruction channel so `stages.retry` can narrowly direct the next invocation without manual artifact edits or permanent prompt mutation. |
| Goal | Let operators retry a blocked stage with a short, auditable instruction such as "only implement the GraphQL scheduler readback slice" while preserving frozen run artifacts, retry lineage, and command accountability. |

**Gate naming note:** this proposal owns the future canonical gate alias `proposal-065|p065`. It must be added to `scripts/test-gate.sh` and `docs/reference/test-gates.md` when implementation starts.

---

## 1. Context and Motivation

During parallel run dogfooding on 2026-04-21, P051 and P061 reached implementation-loop exhaustion with useful but overly broad remaining task lists. The operator needed a narrow retry command:

```text
Retry this stage, but only do the GraphQL scheduler readback slice.
```

Current recovery tooling cannot pass that intent durably:

- `stages.retry` accepts only `run_id`, `stage_id`, and internal retry-budget flags;
- MCP tool schema does not expose an instruction field;
- GraphQL has no write path for retry and should not gain one for this proposal;
- editing run artifacts by hand would bypass the orchestrator's evidence model;
- adding the text to a proposal or artifact makes the instruction too durable and too broad;
- relying on chat memory does not survive daemon restart, provider restart, or agent handoff.

The missing primitive is a bounded operator instruction attached to one retry attempt and injected only into the next invocation(s) created by that retry attempt.

---

## 2. Problem Statement

### 2.1 Retry cannot narrow the next pass

When a stage blocks because the remaining work is too broad, the operator may want to spend the next provider invocation on one precise slice. Today retry repeats the same stage context and asks the agent to infer the narrowed task from previous artifacts or external chat.

### 2.2 Manual artifact edits are the wrong escape hatch

Manually editing run artifacts can make the next invocation see the desired instruction, but it breaks provenance:

- the command journal does not explain why behavior changed;
- active artifact truth can be mutated outside normal source-generation claims;
- later reports cannot distinguish original workflow context from operator retry guidance;
- multiple agents may inherit an instruction that was meant for one retry only.

### 2.3 Prompt mutation must be bounded

Operator retry instructions are operational guidance, not a proposal rewrite and not a permanent agent catalog change. They must not become sticky prompt state for future stages, unrelated retry attempts, or cloned runs unless explicitly carried forward by another proposal.

---

## 3. Scope

P065 includes:

- `stages.retry` input schema extension with optional `operator_instruction`.
- Domain command extension for `RetryStageCmd`.
- Durable command-journal persistence of the instruction.
- Retry-attempt metadata that binds the instruction to the new stage attempt and generated work items.
- Executor/orchestrator prompt assembly changes so the instruction is exposed as a separate immutable input for only the next invocation(s) created by that retry command.
- Readback/projection evidence that a retry instruction existed, with safe redaction rules.
- Focused tests and a `proposal-065|p065` validation gate.

P065 does not include:

- UI use of MCP tools. The macOS UI must not call MCP for retry.
- A GraphQL retry mutation or any new GraphQL write path.
- General free-form operator prompt overrides.
- Editing frozen workflow snapshots, agent catalogs, proposals, or run artifacts to smuggle instructions into context.
- Replacing P045's broader recovery tool plan.
- Operator include/exclude reviewer overrides.
- Cross-run knowledge transfer; durable lessons between runs remain P064 scope.

---

## 4. Proposed Behavior

### 4.1 MCP tool contract

Extend `stages.retry`:

```json
{
  "name": "stages.retry",
  "description": "Retry a failed or blocked stage",
  "input_schema": {
    "type": "object",
    "required": ["run_id", "stage_id"],
    "properties": {
      "run_id": { "type": "string" },
      "stage_id": { "type": "string" },
      "operator_instruction": {
        "type": "string",
        "minLength": 1,
        "maxLength": 2000,
        "description": "Optional operator instruction for only the next invocation(s) created by this retry attempt."
      }
    }
  }
}
```

Validation:

- reject non-operator principals at the existing MCP auth boundary;
- trim surrounding whitespace;
- reject empty strings after trim;
- reject text over the configured limit;
- reject binary/control characters other than normal whitespace;
- preserve the exact accepted text in durable command evidence;
- do not parse the instruction as YAML, JSON, shell, or workflow code.

### 4.2 Domain command

Extend `RetryStageCmd`:

```rust
pub struct RetryStageCmd {
    pub run_id: RunId,
    pub stage_id: String,
    pub consume_quota_budget_now: bool,
    pub operator_instruction: Option<String>,
}
```

The command handler writes one command journal entry that includes:

- command id / journal id;
- operator principal id and class;
- run id;
- stage id;
- source failed/blocked stage execution id;
- new retry stage execution id;
- retry work item ids created by the command;
- accepted `operator_instruction`, when present.

The command journal remains the audit authority for who requested the instruction and when.

### 4.3 Retry-attempt metadata

The retry transaction must create metadata that binds the instruction to the new attempt:

```json
{
  "schema_version": "retry-instruction.v1",
  "journal_id": "uuid",
  "run_id": "uuid",
  "stage_id": "state_8_implementation_continued",
  "source_stage_execution_id": "uuid",
  "retry_stage_execution_id": "uuid",
  "operator_instruction": "Implement only the GraphQL scheduler readback slice.",
  "created_at": "timestamp",
  "created_by_principal_id": "operator",
  "delivery": {
    "scope": "next_retry_invocations",
    "status": "pending"
  }
}
```

Implementation may use a dedicated table, a structured column on retry stage execution, or an existing stage-attempt metadata table. The required invariant is:

- the retry stage attempt and instruction metadata are committed in the same transaction-equivalent write unit as the retry command's stage/work-item supersession;
- there is no durable state where a retry instruction exists without a retry attempt, or a retry attempt references missing instruction metadata.

### 4.4 Invocation input injection

For each `InvokeAgent` work item created by the retry command, prompt/input assembly adds a separate immutable input:

```yaml
operator_retry_instruction:
  schema_version: retry-instruction.v1
  journal_id: uuid
  stage_id: state_8_implementation_continued
  instruction: "Implement only the GraphQL scheduler readback slice."
  scope: next_retry_invocation_only
```

Rules:

- The instruction is not appended into the proposal text.
- The instruction is not written into `run_state` as generic context.
- The instruction is not merged into `implementation_plan`, `implementation_backlog`, or other agent-authored artifacts.
- The instruction is delivered only to `InvokeAgent` work items created by this retry command.
- If the retried stage fans out to multiple agents, every invocation created by that retry command receives the same instruction unless a future agent-level retry proposal narrows it further.
- Later stages, retries, startup repair catchups, and cloned runs do not receive the instruction unless a new operator command supplies one.

Agent prompt guidance must tell the recipient:

- treat `operator_retry_instruction` as operator-scoped retry guidance;
- obey it only within the approved proposal/workflow boundaries;
- do not treat it as permission to violate repo policy, proposal scope, auth policy, or safety gates;
- report conflicts between the instruction and approved proposal instead of silently overriding proposal truth.

### 4.5 Delivery settlement

The executor records delivery state:

- `pending`: retry instruction exists but no matching invocation has started;
- `delivered`: at least one matching invocation received the immutable input;
- `completed`: all matching retry-created invocations reached terminal status;
- `abandoned`: retry attempt was superseded/cancelled before delivery;
- `failed`: prompt assembly or invocation startup failed before delivery.

Delivery settlement is diagnostic. It must not be used as proof that the agent followed the instruction; the agent's output artifacts and review/audit stages remain the behavioral evidence.

### 4.6 Readback

MCP `runs.get` / `reports.get` and GraphQL read models may expose retry-instruction evidence as read-only provenance:

- instruction present: yes/no;
- journal id;
- created timestamp;
- delivery status;
- stage attempt id;
- optionally the instruction text for operator-class readers.

Non-operator readers must receive omitted or redacted instruction text. The readback surface must not create a retry write path.

### 4.7 Idempotency and supersession

Retry command idempotency must include instruction ownership:

- repeating the same `stages.retry` command because of transport retry returns the existing journal/retry attempt when the command idempotency key matches;
- issuing a new retry after the previous retry attempt is terminal creates a new instruction scope;
- issuing a new retry while an instructed retry attempt is active must follow existing retry guards and not create a second active instruction for the same stage attempt;
- when a retry attempt is superseded, its instruction metadata is marked `abandoned` or `completed` according to delivery state, not deleted.

---

## 5. Implementation Inventory

Likely touched areas:

- `control-plane/crates/domain/src/commands.rs`
- `control-plane/crates/engine/src/command_handler.rs`
- `control-plane/crates/engine/src/orchestrator.rs`
- `control-plane/crates/engine/src/work_queue.rs`
- `control-plane/crates/db/src/repos/command_journal.rs`
- `control-plane/crates/db/src/repos/stages.rs`
- `control-plane/crates/db/src/repos/work_items.rs`
- `control-plane/crates/mcp-server/src/tools/stages.rs`
- `control-plane/crates/mcp-server/src/server.rs`
- `control-plane/crates/graphql-server/src/schema.rs`
- `control-plane/crates/graphql-server/src/types/*`
- `docs/reference/output-contracts-failure-evidence-and-recovery.md`
- `docs/reference/query-projections-and-client-consumption-contract.md`
- `docs/reference/test-gates.md`
- `scripts/test-gate.sh`

Exact files may differ if the implementation introduces a dedicated retry-instruction repository.

---

## 6. Tests and Proof Gate

Add canonical gate aliases:

- `proposal-065`
- `p065`

Required proof:

- MCP schema test proves `stages.retry` exposes optional `operator_instruction`.
- MCP auth test proves non-operator principals cannot issue instructed retries.
- Validation tests prove empty, oversized, and control-character instructions are rejected.
- Command handler test proves instruction text is persisted in command journal.
- Transaction test proves retry stage attempt, supersession, work items, and retry-instruction metadata are committed atomically.
- Crash-step or failure-injection test proves no orphan instruction metadata exists without a retry attempt/work item.
- Executor/orchestrator test proves the next retry-created invocation receives `operator_retry_instruction` as a separate immutable input.
- Regression test proves later invocations and later retry attempts do not inherit the instruction.
- Fan-out test proves every invocation created by a stage retry receives the instruction exactly once.
- Readback test proves operator-class readers can inspect provenance and non-operator readers do not receive raw instruction text.
- Gate registry test proves `proposal-065|p065` is discoverable.

---

## 7. Rollout

1. Add data model and command journal serialization first, behind a compatible optional field.
2. Extend MCP `stages.retry` schema and validation.
3. Add retry-attempt metadata write path inside the existing retry transaction.
4. Add invocation input injection for retry-created work items only.
5. Add readback/projection fields.
6. Register and run `./scripts/test-gate.sh proposal-065`.
7. Use the feature on P051/P061-style blocked implementation loops before broadening to `agents.retry`.

---

## 8. Acceptance Criteria

- Operator can call `stages.retry` with a short instruction and the command succeeds without manual artifact edits.
- The instruction is durably auditable in command journal and retry-attempt metadata.
- Only the next invocation(s) created by that retry command receive the instruction.
- Frozen proposals, workflow snapshots, agent catalogs, and agent-authored artifacts are not mutated to carry the instruction.
- UI does not gain an MCP command path or a GraphQL retry mutation.
- Readback exposes provenance without leaking raw instruction text to non-operators.
- Retry supersession and crash recovery cannot leave orphan instruction metadata.
- `./scripts/test-gate.sh proposal-065` passes.

---

## 9. Open Questions

1. Should the instruction limit stay at 2000 characters, or should product cap it lower to force concise operator guidance?
2. Should `agents.retry` from P045 share the same instruction metadata model when implemented, with target-agent scoping?
3. Should a future command allow explicit "carry this instruction into the next refinement stage", or should cross-stage guidance always be represented as proposal/refinement artifacts instead?
