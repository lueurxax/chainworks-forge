# Chainworks Forge Agent Conventions

## Repository shape

Chainworks Forge is a macOS SwiftUI operator app plus a Rust control-plane parity workspace.

Primary surfaces:

- `Chainworks Forge/`: macOS SwiftUI app, operator shell, workflow engine, providers, recovery, artifacts, release/sign-off UI.
- `Chainworks ForgeTests/` and `Chainworks ForgeUITests/`: Swift unit/integration/UI tests.
- `control-plane/`: Rust workspace for daemon, GraphQL, MCP, engine, workflow compiler, DB, auth, ACP transport.
- `examples/workflows/` and `examples/agents/`: shared workflow/catalog contracts.
- `docs/reference/`: current implemented-system truth.
- `docs/proposals/`: active proposals and review/audit artifacts.

Current Go support is proposal-level unless a real `go.mod` appears.

## Proposal review defaults

Use `proposal-review-router` for proposal-readiness and research routing.

Router inputs:

- proposal file
- adjacent docs and reference docs
- `.review-baselines/current-system-baseline.md`
- `<proposal>.review/integration-context.md` when present
- prior evidence/research packs when relevant
- narrow current code-path slices only

Do not require build/run, Xcode, simulator, daemon startup, cargo tests, benchmarks, load tests, or fuzzing in proposal-readiness mode.

## Repo-local routing expectations

Use `.codex/review-router.yaml` as the local router plugin.

Preferred reviewer behavior:

- macOS UI proposals route to `macos_ui_reviewer`.
- Swift app state/provider/workflow proposals route to `apple_arch_reviewer`.
- Rust control-plane proposals route to `rust_arch_reviewer` and, when retry/resume/work queues/cancellation are involved, `rust_reliability_reviewer`.
- GraphQL, MCP, ACP, workflow YAML, agent catalog, report payload, or future protobuf/OpenAPI changes route to `api_contract_reviewer`.
- Migrations, test gates, release receipts, rollout, rollback, telemetry, or support/debuggability route to `observability_rollout_reviewer`.
- Durable Run/Stage/Agent/Approval/artifact/recovery truth changes route to `chainworks_execution_truth_reviewer`.
- Product review remains opt-in unless metrics or decision checkpoints are central.
- iOS review is not selected unless a proposal explicitly introduces iOS target evidence.

## Validation commands

Use `scripts/test-gate.sh` as the canonical gate wrapper when validation is requested.

Common commands:

```bash
./scripts/test-gate.sh list
./scripts/test-gate.sh build
./scripts/test-gate.sh fast
./scripts/test-gate.sh proposal-XXX
```

Rust-only commands, when explicitly requested:

```bash
cd control-plane
cargo test
cargo test -p engine <test_name>
```

UI tests are remote-only by repository policy. Do not run local UI smoke tests unless the user explicitly asks and the remote-host workflow is available.

## Safety rules

- Do not run destructive git commands such as `git reset --hard` or `git checkout --` unless explicitly requested.
- Do not delete `.chainworks/*.db*`, `.chainworks/`, worktrees, build outputs, or artifacts unless explicitly requested.
- Do not mutate proposal files during review unless the user asks for proposal editing.
- Keep proposal review read-only by default.
- Prefer current `docs/reference/` truth over old proposal lineage.

## Run lifecycle safety

Do not normalize manual closeout of unfinished implementation runs. The target operating model is to diagnose and fix orchestration/retry/transition issues so runs reach their intended workflow states through the orchestrator.

Cancelling, archiving, retry-superseding, cleaning up, or otherwise triggering lifecycle actions that may remove or replace a dirty run-owned worktree is an emergency-only path, not the standard way to finish implementation work. Run-owned worktrees are ephemeral; before any such emergency lifecycle action, agents must first prove that implementation work is durable outside the run lifecycle.

Durability proof must be one of:

- a git commit or branch that actually contains the implementation work;
- a `git diff --binary` patch bundle stored outside the run-owned tree;
- a tar/archive snapshot stored outside the run-owned tree;
- an explicit operator decision to discard the dirty work after seeing `git status --short`, untracked-file inventory, and the preservation risk.

High-context operator phrases such as "like the previous one", "clean it up", "kill the run", "archive it", or "do the same" are not sufficient authorization for destructive lifecycle actions when dirty implementation work may exist. Treat ambiguity as a stop condition: first explain the orchestration fix path, summarize any emergency preservation risk, show the preservation plan, and wait for explicit confirmation.

Incident reference: `docs/incidents/2026-04-21-p053-worktree-loss-retro.md`.
