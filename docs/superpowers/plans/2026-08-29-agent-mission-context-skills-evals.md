# Agent Mission Context And Skills Eval Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add mandatory frozen mission context and two local Agent Skills bundles, proved by a provider-free regression gate.

**Architecture:** The workflow compiler owns catalog snapshot V2 and embeds validated external skill bytes. The engine uses one fallible prompt finalizer for every fresh provider dispatch and preflights static contexts before `StartRun` persists a run. Existing authorization, output ownership, and retry persistence remain authoritative.

**Tech Stack:** Rust, SQLite, serde YAML/JSON, shell test gates.

**Spec:** `docs/superpowers/specs/2026-08-27-agent-mission-context-skills-evals-design.md`

## Global Constraints

- New runs always use catalog snapshot format V2; there is no feature flag or disable path.
- Legacy frozen snapshots retain their exact prompt behavior.
- Stored snapshot corruption fails closed and never falls back to live YAML.
- Skill loading is local, single-file, descriptor-relative, bounded, and no-follow.
- The acceptance gate starts no daemon, provider, Xcode build, or network request.
- Existing dirty-worktree changes are preserved.

---

### Task 1: Frozen Catalog And Skill Bundles

**Files:**
- Modify: `control-plane/crates/workflow/src/catalog.rs`
- Modify: `control-plane/crates/workflow/src/compiler.rs`
- Modify: `control-plane/crates/workflow/src/plan.rs`
- Modify: `control-plane/crates/workflow/Cargo.toml`
- Test: `control-plane/crates/workflow/tests/agent_context_skills.rs`

**Interfaces:**
- Produces: catalog snapshot V2, `chainworks_compiled`, embedded bundle lookup, and total procedure identity.

- [x] Add tests for author-field rejection, absent/1/2 compatibility, exact bundle cardinality, source mutation/deletion, strict bundle validation, and all procedure arms.
- [x] Run the focused workflow tests and confirm they fail for the missing V2 behavior.
- [x] Implement bounded descriptor-relative bundle loading and V2 snapshot enrichment.
- [x] Make unknown or failed `skill_ref` resolution a compile error and preserve the existing final procedure hash format.
- [x] Re-run the focused workflow tests to green.

### Task 2: Mission Context And Prompt Finalization

**Files:**
- Modify: `control-plane/crates/engine/src/orchestrator.rs`
- Modify: `control-plane/crates/engine/src/contracts.rs`
- Test: `control-plane/crates/engine/tests/agent_context_skills.rs`

**Interfaces:**
- Consumes: frozen V2 catalog extension and resolved procedure identity.
- Produces: `AgentMissionContextV1`, assignment unions, consumer projection, one ordered prompt finalizer, and exact copy-prompt validation.

- [x] Add tests for static, post-approval, dynamic, owner-only, P017 mediation, P058 mediation, retry-copy, outputs, consumers, and size bounds.
- [x] Run the focused engine tests and confirm the missing finalizer assertions fail.
- [x] Implement the shared fallible finalizer and route every fresh `InvokeAgent` producer through it.
- [x] Reuse persisted prompt bytes for copy retries and reject malformed V1 copies.
- [x] Re-run the focused engine tests to green.

### Task 3: StartRun And Frozen Snapshot Boundary

**Files:**
- Modify: `control-plane/crates/engine/src/command_handler.rs`
- Modify: `control-plane/crates/engine/src/orchestrator.rs`
- Test: `control-plane/crates/engine/tests/agent_context_skills.rs`

**Interfaces:**
- Consumes: `RunPlan`, Idea title/body, and mission preflight.
- Produces: zero-write StartRun rejection and hash-authenticated frozen-plan loading.

- [x] Add tests for exact/+1 Idea bounds, missing/read-failed Idea, one-sided/hash-mismatched snapshots, and no live fallback.
- [x] Confirm the tests fail for current mutation/fallback behavior.
- [x] Preflight before Run/work insertion and propagate later Idea-read failures.
- [x] Authenticate both stored JSON strings before compile and fail closed on every invalid state.
- [x] Re-run the focused engine tests to green.

### Task 4: Active Bundles And Provider-Free Gate

**Files:**
- Create: `examples/agents/skills/proposal-review-router/SKILL.md`
- Create: `examples/agents/skills/code-implementation/SKILL.md`
- Modify: `examples/agents/agents.yaml`
- Modify: `scripts/test-gate.sh`

**Interfaces:**
- Produces: two strict active bundles and `./scripts/test-gate.sh agent-context-skills`.

- [x] Convert only the two named catalog skills and remove duplicated procedure prose.
- [x] Add the focused gate with the six deterministic context fixtures and source-inventory checks.
- [x] Run focused managed Cargo tests.
- [x] Run `./scripts/test-gate.sh agent-context-skills`, `bash -n scripts/test-gate.sh`, and scoped `git diff --check`.
