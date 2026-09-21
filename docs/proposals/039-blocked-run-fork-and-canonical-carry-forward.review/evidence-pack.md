# P039 Evidence Pack

Facts only; preparation date 2026-09-20. Code baseline HEAD
`8c40f71a03cfebff413fbad584131e0bcbe9859b`; working tree is dirty with unrelated
work. This is proposal-readiness evidence, not build/runtime acceptance.

| ID | Fact | Source |
| --- | --- | --- |
| E01 | Governed UI permits GraphQL reads and two approval mutations, not fork/start controls | `docs/reference/ui-action-boundary.md` |
| E02 | Command enum/StartRun have no source-run carry-forward operation or import args | `control-plane/crates/domain/src/commands.rs:34`, `:113` |
| E03 | StartRun enters plan.initial_state, provisions roots and enqueues AdvanceRun | `control-plane/crates/engine/src/command_handler.rs:3270-3455` |
| E04 | Catalog retrofit removes only escalation_policies/backend_profiles before drift comparison | `control-plane/crates/engine/src/command_handler.rs:1301-1343` |
| E05 | Current CODE_WRITE explicitly grants xcode_headless; trust/native consent remain independent | `examples/agents/agents.yaml:1019`, `docs/reference/xcode-headless-runtime.md` |
| E06 | Restricted Operator entries replace default tool capabilities with an explicit allowlist | `control-plane/crates/auth/src/lib.rs:190-225` |
| E07 | Ordinary WorktreeProvisioner resolves a moving base branch and accepts existing path data; cleanup uses forced removal | `control-plane/crates/engine/src/worktree.rs:55-178` |
| E08 | WorktreeFingerprintV1 starts from Git status entries and classifies changed files | `control-plane/crates/engine/src/worktree_fingerprint.rs:120-165` |
| E09 | Current workflow has review state_4, manual state_6, preparation/code_writer sequence at state_7 | `examples/workflows/workflow.yaml:95-197` |
| E10 | RunStatus blocked is nonterminal; source/root isolation is persisted and source fallback is disabled for new roots | `control-plane/crates/domain/src/run.rs:77`, `docs/reference/per-run-workspace-isolation.md` |
| E11 | Lifecycle MCP commands use caller_request_id UUIDv4, while StartRun uses a different UUIDv7 path | `control-plane/crates/mcp-server/src/tools/runs.rs:98-155`, `:912-918`; `domain/src/commands.rs:876` |
| E12 | DB artifacts require non-null agent_id/stage_id; approvals have no proposal-hash column | Canonical read-only table schema inspection in the P095 preservation; migrations and repos in `control-plane/crates/db` |
| E13 | P064 barrier table appears in migration/readback; existing source-fence enforcement was not established | `control-plane/crates/db/migrations/033_p064_main_sync_and_knowledge_capsules.sql`, `graphql-server/src/schema.rs:5002` |
| E14 | Migration runtime rejects schema newer than the binary | `control-plane/crates/db/src/migrate.rs:98`, `:982`; daemon startup migration path |
| E15 | Behavior-changing proposals need rollout declaration, migration/hold/rollback/readback proof | `docs/reference/executable-rollout-gate-template.md` |
| E16 | P095 source preserved and preflighted but no transfer performed | Adjacent `integration-context.md`; private preservation manifest/TRANSFER-STATUS, no raw private evidence published |
| E17 | Existing migration backup copies only the main file and checks byte size | `control-plane/crates/db/src/migrate.rs:671` |
| E18 | Shared rollout enabled_state is derived from enforcement mode, not feature admission | `control-plane/crates/db/src/repos/rollout_contract_checks.rs:377` |
| E19 | Non-InvokeAgent work items currently execute inline in the coordinator loop | `control-plane/crates/engine/src/executor.rs:11409` |
| E20 | Current approval predicates accept any granted row at a logical stage; Approval has no evidence-generation field | `control-plane/crates/engine/src/orchestrator.rs:7049`, `domain/src/approval.rs:44` |
| E21 | Read-only tasks without explicit strategy use the repository, not a worktree | `control-plane/crates/domain/src/execution_root.rs:17`, `engine/src/worktree.rs:16` |
| E22 | Delivery writable-directory preflight writes then deletes a probe | `control-plane/crates/engine/src/preflight.rs:179` |
| E23 | Existing explicit-global check distinguishes absent scope from an empty list | `control-plane/crates/auth/src/lib.rs:1292` |
| E24 | P083 lifecycle response is strict, with no operation ID; successful MCP values use text-content JSON | `control-plane/crates/mcp-server/src/tools/runs.rs:28`, `src/server.rs:1070` |

## Missing Implemented Capabilities

- GAP-01: typed successor command, storage, exactly-one source/idea ownership.
- GAP-02: complete source execution fence across command/dispatch/recovery/settlement.
- GAP-03: independent preservation + pinned-OID materialization + crash receipts.
- GAP-04: closed compiler continuation entry and input-origin resolution.
- GAP-05: manifest-bound fresh approval, lineage/readback and production proof gate.

These gaps are proposed implementation scope, not claims that the feature exists.
No external research is needed to establish these repository-local facts.
