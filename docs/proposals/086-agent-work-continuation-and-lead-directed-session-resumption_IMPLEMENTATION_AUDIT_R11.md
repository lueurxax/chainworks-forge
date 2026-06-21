# Proposal 086 Implementation Audit R11

## Metadata

- Proposal: `docs/proposals/086-agent-work-continuation-and-lead-directed-session-resumption.md`
- Proposal title: Proposal 086: Provider Session Resurrection Completion
- Proposal status audited: Draft
- Audit report: `docs/proposals/086-agent-work-continuation-and-lead-directed-session-resumption_IMPLEMENTATION_AUDIT_R11.md`
- Audit date: 2026-06-20
- Implementation target: current worktree at `/Users/user/Documents/Chainworks Forge`
- Compare base: repository `HEAD` `0e6482c8` plus uncommitted worktree changes
- Final verdict: Not Ready
- Overall conformance: Partial implementation. The Claude adapter capability, basic attach-before-prompt path, durable resurrection phase fields, raw receipt storage, and readback scaffolding exist, but same-tree proof fails and several required safety/recovery cases remain incomplete.

## Prior Review Reuse

Mandatory prior-review discovery returned no reusable review artifacts:

```json
{
  "artifacts": [],
  "proposal_path": "/Users/user/Documents/Chainworks Forge/docs/proposals/086-agent-work-continuation-and-lead-directed-session-resumption.md",
  "repo_root": "/Users/user/Documents/Chainworks Forge"
}
```

Reviewer-selection reuse status: not reused. This audit performs a fresh requirement pass and ignores prior implementation audit files for reviewer selection.

## Proposal Contract Summary

P086 is no longer the historical live-handle continuation baseline. The active scope is the remaining `provider_session_resurrection` path: start a new Chainworks-managed ACP subprocess, attach/resume a recorded provider session id for a supported adapter, prove requested-vs-actual provider session identity before prompt send, preserve fail-closed behavior for unsupported adapters, write durable attach and correlation evidence, support output-only repair after live handle loss, and prove crash/replay behavior without duplicate prompt send or fresh retry fallback.

Platform/product scope: Rust control-plane daemon, ACP adapter/runtime boundary, SQLite continuation state, MCP admission/readback, GraphQL/operator readback, JSON Schemas, continuation metrics, and passive Swift/macOS readback. Swift mutation UI remains out of scope.

Primary flows audited:

1. Operator requests `agents.continue_work` with `continuation_mode=provider_session_resurrection`.
2. MCP admission validates target run/stage/agent/session, frozen catalog opt-in, adapter capability, and safety blockers before enqueue.
3. Worker starts a fresh ACP child, passes the recorded provider session id, verifies actual session id, and persists attach receipt before prompt send.
4. Worker sends the P086 continuation prompt through the attached session and settles via continuation artifacts/readback.
5. Negative/recovery paths reject unsupported/mismatched/unsafe targets, output-only repair, lost terminal response recovery, and crash/replay boundaries.

## Reviewer Routing

Mandatory surface fingerprinting selected these required lenses: `api-contract`, `apple-ui-ux`, `architecture`, `observability-rollout`, `performance`, `reliability`, and `security`. With the hard cap of five routed lenses, this audit selected:

- `rust_architecture_reviewer`: ACP adapter/runtime, worker state machine, DB phase model.
- `rust_reliability_reviewer`: replay, duplicate prompt, timeout, crash, and daemon gate behavior.
- `api_contract_reviewer`: MCP/GraphQL/schema/readback conformance.
- `observability_rollout_reviewer`: metrics, operator report/readback, evidence fixtures.
- `rust_security_reviewer`: required because `security_sensitive_diff.py` triggered.

Rejected/scoped reviewers:

- `apple-ui-ux`: scoped to passive readback only; no mutation UI is in scope and the gate already includes Swift readback tests.
- `performance`: scoped inside reliability/security because the active failures are correctness, proof, and safety blockers rather than throughput regressions.

## Security-Sensitive Diff Summary

`security_sensitive_diff.py --root . --json` triggered `true` with categories:

- `auth`
- `dos_resource_limits`
- `filesystem_subprocess_boundary`
- `parser_boundary`
- `public_ingress`
- `secrets_redaction_privacy`
- `unsafe_crypto_dependency`

Independent security pass status: required and performed as an in-audit pass. Positive evidence exists for redacted provider-session mismatch messages and DB-backed raw receipt storage with audited access. Security sign-off is still blocked by the incomplete forbidden-lane gate and by transcript recovery that can select Claude session-store output by provider session id and `CHAINWORKS_OUTPUT` presence without proving target prompt marker/request fingerprint/stage/agent ownership.

## Fidelity Buckets

Implemented or substantially present:

- Adapter-owned capability struct and failure-class list exist in `control-plane/crates/acp/src/adapters/mod.rs`.
- Claude declares `resumeSessionId`, `session/new.result.sessionId`, and write-enabled resurrection capability in `control-plane/crates/acp/src/adapters/claude.rs`.
- `AcpRuntimeManager::attach_provider_session_for_resurrection` starts a fresh session and compares actual provider session id before inserting it into live sessions.
- SQLite migrations add `resurrection_phase`, deadlines, heartbeat, terminal ledger, receipt access audit, and DB-backed raw receipt storage.
- MCP admission checks frozen catalog JSON and runtime adapter capability instead of only static provider names.
- Worker has a provider-session resurrection branch that writes a raw DB receipt and redacted artifact before sending the continuation prompt.
- MCP/GraphQL attach-receipt readback surfaces and receipt access audit repositories exist.

Partial or divergent:

- Canonical `proposal-086` gate fails on the same tree.
- Supported Claude resurrection requests in daemon tests are rejected at admission instead of accepted.
- Existing live-handle daemon regression times out after the continuation prompt.
- Stage-safety filtering covers only part of the proposal's forbidden lane list.
- The resurrection prompt text still describes "an existing live ACP session" even in the fresh-process resurrection path.
- Receipt `adapter_capability_version` is numeric in schema/worker while adapter capability is a string (`provider_session_resurrection_v1`).

Missing or not proven:

- Output-only repair through provider-session resurrection after live handle loss.
- Machine-checkable `changed_source_files == 0` proof for resurrection output-only repair.
- Claude session-store recovery bound to prompt-turn marker, request fingerprint, stage execution id, and agent execution id.
- Ambiguous same-provider-session transcript rejection for a different target execution.
- Crash/replay tests for `launching`, `launched`, `attaching`, `attached_unprompted`, and `prompting`.
- Replacement of historical negative fixtures that still describe resurrection as pre-Phase-4 unsupported.

## Findings

### F-01 - Blocker - Canonical P086 gate fails on the same tree

Evidence: `./scripts/test-gate.sh proposal-086` exited with code `101`. The preflight/schema and earlier Rust unit slices passed, but `cargo test -p daemon --test proposal_086_mcp_continuation_live_reuse` failed all 3 tests:

- `p086_mcp_continue_work_resurrects_provider_session_and_records_v2_receipt`: expected `accepted`, got `rejected`.
- `p086_mcp_continue_work_rejects_resurrection_identity_mismatch_before_prompt`: expected `accepted`, got `rejected`.
- `p086_mcp_continue_work_reuses_live_acp_session_and_materializes_terminal_artifacts`: timed out after the fixture saw the original prompt and continuation prompt.

This directly fails acceptance criterion 14 and blocks any Ready/Ready-with-risks verdict.

### F-02 - Blocker - Supported Claude provider-session resurrection is not proven to admit or execute

The daemon test fixture seeds a completed Claude `code_writer`, `provider_session_id=fixture-session-reuse`, `session_generation_id=p086-generation`, and frozen catalog opt-in, then calls `agents.continue_work` with `provider_session_resurrection`. The current result is `rejected`, not `accepted` (`control-plane/crates/daemon/tests/proposal_086_mcp_continuation_live_reuse.rs:724` and `:846`). This leaves acceptance criteria 1 and 2 unproven despite adapter/runtime code existing.

### F-03 - High - Output-only repair after live-handle loss is not implemented as resurrection

P086 requires malformed/incomplete `CHAINWORKS_OUTPUT` repair through a resurrected provider session when Chainworks no longer owns the live ACP handle. Current admission treats `output_only_recovery` as a separate mode that requires a live session (`control-plane/crates/mcp-server/src/tools/agents.rs:1256`-`1304`). The provider-session resurrection receipt path hardcodes `output_only=false`, `source_edit_allowance=true`, and `changed_source_files_count=0` (`control-plane/crates/engine/src/executor.rs:7632`-`7640`). That is not machine-checkable no-source-change proof for output-only resurrection and fails acceptance criterion 9.

### F-04 - High - Claude session-store recovery is not target-bound and is not recorded in resurrection receipts

The Claude session-store helper scans for `<provider_session_id>.jsonl` and takes the latest line containing `CHAINWORKS_OUTPUT` (`control-plane/crates/acp/src/session.rs:807`-`904`). It does not require a prompt-turn marker, request fingerprint, stage execution id, or agent execution id. The resurrection worker writes `session_store_recovery_result="not_attempted"` plus null transcript path/digest/ownership fields and does not update them later (`control-plane/crates/engine/src/executor.rs:7667`-`7678`). Existing tests cover different provider session ids, not same-session/different-target ambiguity. This fails acceptance criteria 10 and 11.

### F-05 - High - Forbidden target lanes are incomplete

The proposal requires fail-closed rejection for release, publish, upload, distribution, commit, push, security, prepush-review, and lead-orchestration lanes. `forbidden_stage_kind` currently checks release, publish, `git_push`/`git-push`, upload, distribution/distribute, and connect only (`control-plane/crates/mcp-server/src/tools/agents.rs:1089`-`1113`). It does not cover commit, generic push, security, prepush-review, or lead-orchestration. This is a safety blocker for section 3.4 and section 5.

### F-06 - Medium - Capability/readback contract has shape drift

Claude declares the adapter capability version as the string `provider_session_resurrection_v1` (`control-plane/crates/acp/src/adapters/claude.rs:57`-`69`), but the v2 receipt schema defines `adapter_capability_version` as an integer and the worker writes `1` (`docs/reference/p086/schemas/artifacts/provider_session_attach_receipt_v2.schema.json:77`-`79`, `control-plane/crates/engine/src/executor.rs:7582`-`7585`). The proposal asks for a capability schema/version string and audit-readable adapter proof. This should be aligned before closeout even if the integer is intentionally a receipt schema version.

## Requirement Coverage Matrix

| Requirement | Status | Evidence |
|---|---:|---|
| REQ-01 explicit modes and no silent retry/reuse collapse | Partial | Mode enum exists, but output-only/resurrection composition and selected-mode rationale are incomplete. |
| REQ-02 adapter capability contract and typed failures | Partial | Capability struct/failures exist; gate fails and receipt version shape drifts. |
| REQ-03 frozen catalog capability gate | Partial | Admission parses `catalog_snapshot_json`; daemon resurrection admission currently rejects the seeded opt-in fixture. |
| REQ-04 Claude provider resurrection success | Not met | Canonical daemon success test gets `rejected`; gate failed. |
| REQ-05 attach identity proof before prompt | Partial | Runtime compares actual vs requested id before prompt; mismatch test cannot reach worker because admission rejects. |
| REQ-06 generic resurrection flow and durable attach receipt | Partial | Worker writes raw DB/redacted artifact receipt before prompt; provider request/turn id and recovery fields remain null/not attempted. |
| REQ-07 output-repair use case | Not met | Output-only recovery requires live session and resurrection receipt hardcodes output-only false. |
| REQ-08 durable resurrection phase and replay contract | Partial | DB phases/deadlines exist; crash/replay phase tests are missing and gate's live reuse test times out. |
| REQ-09 readback and metrics | Partial | Metrics/readback scaffolding exists; same-tree gate fails and some receipt fields are only raw/null. |
| REQ-10 safety fail-closed rules | Partial | Unsupported/mismatch scaffolding exists; forbidden lane set is incomplete. |
| REQ-11 test and evidence suite | Not met | Required proposal gate fails; output-only, transcript ambiguity, and crash/replay evidence missing. |

## Reviewer Lens Scorecard

| Lens | Score | Notes |
|---|---:|---|
| Architecture | 2/5 | Good separation of adapter capability, runtime manager, worker branch, and DB state, but the successful flow is not green. |
| Reliability | 1/5 | Canonical gate fails; crash/replay and timeout evidence are incomplete. |
| API contract | 2/5 | MCP/GraphQL/schema surfaces exist, but admission rejects supported fixture and receipt version shape drifts. |
| Observability/rollout | 2/5 | Metrics/readback scaffolding exists; historical unsupported fixtures still need replacement with real Phase 4 evidence. |
| Security | 2/5 | Raw receipt access/redaction work is present, but forbidden-lane and transcript-ownership gaps remain security-sensitive. |

## Readiness Checklist

- [x] Proposal and implementation surface identified.
- [x] Mandatory helper scripts run.
- [x] Prior reviewer-selection reuse checked.
- [x] Security-sensitive diff evaluated.
- [x] Canonical proposal gate run on the same tree.
- [ ] Canonical proposal gate passes.
- [ ] Supported Claude resurrection admits and executes in daemon proof.
- [ ] Output-only resurrection repair is implemented and proven with no-source-change evidence.
- [ ] Claude transcript recovery is prompt/target-bound and ambiguity-safe.
- [ ] Crash/replay evidence covers required resurrection phases.
- [ ] Forbidden lane list matches the proposal.

## Verification Log

- `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/report_path.py ...` -> selected `docs/proposals/086-agent-work-continuation-and-lead-directed-session-resumption_IMPLEMENTATION_AUDIT_R11.md`.
- `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/discover_prior_review.py docs/proposals/086-agent-work-continuation-and-lead-directed-session-resumption.md` -> no prior review artifacts.
- `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/security_sensitive_diff.py --root . --json` -> triggered with auth/public ingress/parser/filesystem subprocess/redaction/resource-limit categories.
- `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/implementation_surface_fingerprint.py --root . --json` -> required lenses included API contract, Apple UI/UX, architecture, observability/rollout, performance, reliability, security.
- `./scripts/test-gate.sh proposal-086` -> failed with exit code 101; daemon continuation regression failed 0/3 tests.
- Focused source inspection: proposal, ACP adapters/manager/session, MCP agents tool, engine executor, DB migrations/repos, schemas, reference doc, daemon tests.

## Final Verdict And Next Actions

Verdict: Not Ready.

The implementation has meaningful scaffolding for provider-session resurrection, but the same-tree proof fails and core P086 requirements remain incomplete. The highest-risk blockers are the failing canonical gate, unsupported successful Claude admission/execution proof, output-only resurrection repair gap, unbound Claude transcript recovery, incomplete forbidden-lane safety gate, and missing phase-specific crash/replay evidence.

Recommended next actions:

1. Fix `agents.continue_work` admission for the seeded supported Claude resurrection fixture and restore the daemon live-reuse regression.
2. Implement output-only repair over provider-session resurrection with pre/post worktree proof and `changed_source_files == 0`.
3. Bind Claude session-store recovery to prompt-turn marker, request fingerprint, stage execution id, and agent execution id; reject same-session ambiguity.
4. Complete forbidden-lane matching for commit, generic push, security, prepush-review, and lead-orchestration lanes.
5. Add/green crash/replay tests for `launching`, `launched`, `attaching`, `attached_unprompted`, and `prompting`, then rerun `./scripts/test-gate.sh proposal-086`.
