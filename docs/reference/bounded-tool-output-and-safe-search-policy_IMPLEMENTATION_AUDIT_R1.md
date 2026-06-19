# P096 Implementation Audit R1: Bounded Tool Output and Safe Search Policy

## Metadata

| Field | Value |
| --- | --- |
| Audit timestamp | 2026-06-11T09:01:49Z |
| Audit mode | auto / implementation-audit |
| Requested proposal | P096 |
| Spec anchor | `docs/reference/bounded-tool-output-and-safe-search-policy.md` |
| Spec md5 | `b7265e0c415f216f3fd6671e3ecb357b` |
| Proposal state | Replaced / retired into reference documentation |
| Repository root | `/Users/user/Documents/Chainworks Forge` |
| Implementation target | Current worktree |
| Git HEAD | `d2a25b64e29d0dff4cb708b6d72ce8ea2c1ff14e` |
| Branch | `main` |
| Compare base | Implicit current worktree audit; no PR base supplied |
| Report path | `docs/reference/bounded-tool-output-and-safe-search-policy_IMPLEMENTATION_AUDIT_R1.md` |

## Verdict

| Track | Verdict |
| --- | --- |
| Overall Conformance | Implemented |
| Overall Implementation Readiness | Ready with Risks |
| Reviewer Selection Reuse | Not reused |
| Audit Confidence | High for P096-owned implementation slice; medium for branch-wide release attribution because the worktree contains many unrelated changes |

P096 is implemented against the retired reference spec. The canonical same-tree gate `./scripts/test-gate.sh proposal-096` passed during this audit. The only readiness risk is release hygiene: the current worktree contains broad unrelated Swift/UI/docs changes, so branch-level handoff should isolate P096-owned files before merging or shipping this slice.

## Prior Proposal-Review Reuse Summary

No persisted prior P096 proposal-review artifact was found beside the retired proposal/spec or in the searched docs paths. Reviewer selection was reconstructed from the current spec and implementation evidence.

Selected reviewers:

- `rust_arch_reviewer`: runtime-owned policy module, ACP transport integration, provider wrapper boundary.
- `rust_reliability_reviewer`: budget enforcement, local-activity polling, failure classification, session invalidation semantics.
- `rust_security_reviewer`: shell/tool execution, filesystem boundary, generated-root excludes, DoS/resource-exhaustion guard.
- `api_contract_reviewer`: JSON-RPC error shape, `AgentFailureKind` vocabulary, `runtime.health.toolOutputGuard` readback.
- `observability_rollout_reviewer`: runtime health readback, focused gate, reference-doc closeout.

Rejected close alternatives:

- Apple UI/UX reviewers: no user-visible macOS/iOS UI flow is in scope.
- Product reviewer: no product metric, experiment, or adoption decision is central to P096.
- Go reviewers: no Go implementation surface exists.
- Rust performance reviewer: no latency, throughput, benchmark, or hot-path performance claim is made; resource-exhaustion behavior was covered by reliability and security reviewers.

## Proposal State and Contract Summary

The original proposal file is retired. The implemented-system contract now lives in `docs/reference/bounded-tool-output-and-safe-search-policy.md`, which states that P096 is implemented, owned by the control-plane runtime, and proven by `./scripts/test-gate.sh proposal-096`.

Backend/service scope:

- ACP/provider runtime tool boundary.
- Codex provider runtime wrappers for `rg` and `find`.
- Runtime failure classification and session reuse policy.
- MCP `runtime.health` readback.
- Prompt guidance for reviewer/auditor agents.
- Reference documentation and focused regression gate.

Apple platform scope: not applicable.

Explicit non-scope observed from the spec: prompt guidance is advisory only; the runtime/tool boundary remains authoritative.

## Primary Service Flows

1. Provider requests permission to run a broad repo-root `rg`/`find`; runtime preflight rejects it before provider context is polluted.
2. Provider runs a narrow search or a broad search with every generated-root exclude; runtime allows it and the Codex wrapper caps output.
3. Provider emits excessive function/tool output or session-store growth; local activity monitoring classifies it as `tool_output_budget_exceeded` before generic provider fallback.
4. Runtime stores failure facts and applies session reuse policy: preflight denial does not quarantine the session, but budget-exceeded/unbounded-output does.
5. Operator/test calls `runtime.health`; the response includes `toolOutputGuard` policy, denylist, budgets, and enforcement readback.

## Proposal Fidelity / Divergence Inventory

### Matches

- Runtime policy is centralized in `domain::tool_policy` with policy/guard versions, output budgets, and generated-root denylist.
- Safe-search preflight rejects broad `rg` and `find` unless every generated/build-root exclude is present.
- ACP permission denial returns typed JSON-RPC error data with `classification`, `preflightCode`, policy/guard versions, and denylist.
- Codex runtime wrappers enforce equivalent `rg`/`find` behavior and cap line/byte output.
- Local activity monitoring records `tool_output_budget_exceeded` for oversized output, cumulative output, session-store growth, and wrapper truncation markers.
- Failure classification orders P096 failures before generic provider/internal fallback.
- Session policy distinguishes preflight denial from poisoned-session output budget failures.
- `runtime.health.toolOutputGuard` reports policy and enforcement readback.
- Reference docs and `proposal-096|p096` gate are present.

### Divergences

- None found for the P096-owned implementation slice.

### Ambiguities / Evidence Gaps

- The whole worktree is dirty with many unrelated changes. This does not affect P096 conformance because the canonical P096 gate passed, but it lowers confidence for branch-wide release attribution until P096 files are isolated.

## Residual Scope / Follow-up Ownership

| Item | Status | Owner / Follow-up | Blocks Conformance | Blocks Readiness |
| --- | --- | --- | --- | --- |
| P096 runtime policy, wrappers, classification, health readback, prompt guidance, reference closeout | Complete | P096 implementation | No | No |
| Branch-level isolation from unrelated dirty worktree changes | Residual release hygiene | No proposal owner required; handoff/staging concern | No | No, but recorded as readiness risk |

No promised P096 behavior remains missing, partially implemented, or unverified.

## Specialist Coverage Matrix

| Triggered Surface | Evidence | Required Lens | Selected Reviewer | Completed | Missing Coverage Blocker |
| --- | --- | --- | --- | --- | --- |
| Rust runtime/tool-boundary architecture | `domain::tool_policy`, ACP transport, Codex adapter | Architecture | `rust_arch_reviewer` | Yes | None |
| Retry/session lifecycle and poisoned-session handling | failure classifier and session policy | Reliability | `rust_reliability_reviewer` | Yes | None |
| Shell/tool execution, filesystem boundary, DoS/resource exhaustion | preflight, wrappers, output caps | Security | `rust_security_reviewer` | Yes | None |
| Typed errors/readback vocabulary | JSON-RPC denial data, `AgentFailureKind`, `runtime.health` | API contract | `api_contract_reviewer` | Yes | None |
| Operator proof/readback/closeout | focused test gate, reference docs, health readback | Observability/rollout | `observability_rollout_reviewer` | Yes | None |

The bundled whole-worktree fingerprint helper over-triggered Apple UI/UX and performance lenses because the repository contains many unrelated dirty files. Manual P096 scoping found no P096 UI/UX surface and no separate performance benchmark/hot-path claim.

## Requirement Summary

| Requirement | Status |
| --- | --- |
| REQ-001 Runtime policy constants, budgets, and generated-root denylist | Implemented |
| REQ-002 Fail-safe safe-search preflight for broad `rg`/`find` | Implemented |
| REQ-003 Typed ACP permission denial contract | Implemented |
| REQ-004 Codex wrapper enforcement and output caps | Implemented |
| REQ-005 Runtime output-budget monitoring and classification | Implemented |
| REQ-006 Failure classification and session quarantine semantics | Implemented |
| REQ-007 `runtime.health.toolOutputGuard` readback | Implemented |
| REQ-008 Prompt guidance, reference closeout, and regression gate | Implemented |

## Detailed Requirement Audit

### REQ-001: Runtime Policy Constants, Budgets, and Generated-Root Denylist

- Proposal source: `docs/reference/bounded-tool-output-and-safe-search-policy.md` lines 17-32.
- Status: Implemented.
- Evidence types: proposal, code, tests-found, tests-run.
- Implementation mapping: `control-plane/crates/domain/src/tool_policy.rs` lines 7-30 defines `TOOL_POLICY_VERSION`, `TOOL_GUARD_VERSION`, default byte/line/cumulative budgets, and `GENERATED_ROOT_DENYLIST`; `control-plane/crates/domain/src/lib.rs` line 33 exports `tool_policy`.
- Gap / note: None.

### REQ-002: Fail-Safe Safe-Search Preflight for Broad `rg`/`find`

- Proposal source: `docs/reference/bounded-tool-output-and-safe-search-policy.md` lines 34-49.
- Status: Implemented.
- Evidence types: proposal, code, tests-found, tests-run.
- Implementation mapping: `control-plane/crates/domain/src/tool_policy.rs` lines 54-73 performs command preflight; lines 111-129 classify broad `rg`/`find`; lines 228-258 require complete generated-root exclude coverage; lines 328-367 test broad denial, partial-exclude denial, narrow allowance, and full-exclude allowance.
- Gap / note: None.

### REQ-003: Typed ACP Permission Denial Contract

- Proposal source: `docs/reference/bounded-tool-output-and-safe-search-policy.md` lines 42-46.
- Status: Implemented.
- Evidence types: proposal, code, tests-found, tests-run.
- Implementation mapping: `control-plane/crates/acp/src/transport.rs` lines 3039-3057 returns a JSON-RPC error with `classification`, `preflightCode`, matched tool, command, policy/guard versions, and generated-root denylist; lines 6042-6074 test the typed broad-`rg` denial.
- Gap / note: None.

### REQ-004: Codex Wrapper Enforcement and Output Caps

- Proposal source: `docs/reference/bounded-tool-output-and-safe-search-policy.md` lines 51-61 and 98-107.
- Status: Implemented.
- Evidence types: proposal, code, tests-found, tests-run.
- Implementation mapping: `control-plane/crates/acp/src/adapters/codex.rs` lines 201-322 generates wrappers that deny broad searches fail-closed, require all generated excludes, locate the real tool outside the wrapper directory, apply a line cap with a truncation marker, and apply a byte cap; lines 591-825 test wrapper installation, denial parity for `rg`/`find`, narrow/full-exclude allowance, and line/byte caps.
- Gap / note: None.

### REQ-005: Runtime Output-Budget Monitoring and Classification

- Proposal source: `docs/reference/bounded-tool-output-and-safe-search-policy.md` lines 51-61 and 63-71.
- Status: Implemented.
- Evidence types: proposal, code, tests-found, tests-run.
- Implementation mapping: `control-plane/crates/acp/src/transport.rs` lines 560-592 detect excessive session-store growth; lines 740-746 detect per-output/cumulative function output excess; lines 776-782 detect wrapper truncation markers; lines 625-650 emits `tool_output_budget_exceeded` provider failure events. `control-plane/crates/engine/src/failure_classifier.rs` lines 207-214 maps `tool_output_budget_preflight_denied`, `tool_output_budget_exceeded`, and `codex_unbounded_tool_output` before generic fallback; lines 667-705 test this ordering.
- Gap / note: None.

### REQ-006: Failure Classification and Session Quarantine Semantics

- Proposal source: `docs/reference/bounded-tool-output-and-safe-search-policy.md` lines 73-81.
- Status: Implemented.
- Evidence types: proposal, code, tests-found, tests-run.
- Implementation mapping: `control-plane/crates/domain/src/agent.rs` lines 161-164 and 191-194 persist the typed failure kinds. `control-plane/crates/engine/src/session/policy.rs` lines 689-701 require session invalidation for `ToolOutputBudgetExceeded` and `codex_unbounded_tool_output`, not for preflight denial; lines 1647-1663 test the distinction.
- Gap / note: None.

### REQ-007: `runtime.health.toolOutputGuard` Readback

- Proposal source: `docs/reference/bounded-tool-output-and-safe-search-policy.md` lines 83-90.
- Status: Implemented.
- Evidence types: proposal, code, tests-found, tests-run.
- Implementation mapping: `control-plane/crates/mcp-server/src/tools/runtime.rs` lines 252-272 emits `toolOutputGuard` status, policy readback, enforcement readback, versions, denylist, and budgets. `control-plane/crates/mcp-server/src/server.rs` lines 4117-4161 tests the runtime-health response.
- Gap / note: None.

### REQ-008: Prompt Guidance, Reference Closeout, and Regression Gate

- Proposal source: `docs/reference/bounded-tool-output-and-safe-search-policy.md` lines 92-107.
- Status: Implemented.
- Evidence types: proposal, code, config, tests-found, tests-run.
- Implementation mapping: `control-plane/crates/domain/src/tool_policy.rs` lines 76-80 builds reusable advisory guidance; `control-plane/crates/acp/src/transport.rs` lines 3114-3121 appends that guidance to prompts; `examples/agents/agents.yaml` lines 2115, 2205, and 2320 include bounded discovery guidance for audit/review agents. `docs/reference/test-gates.md` lines 2027-2048 documents `proposal-096|p096`; `scripts/test-gate.sh` lines 10394-10525 defines the canonical gate and fails closed when targeted filters select zero tests. Reference links exist in `docs/README.md`, `docs/reference/current-system-baseline.md`, `docs/reference/acp-runtime-transport.md`, `docs/reference/mcp-northbound-control-plane-server.md`, and `docs/reference/README.md`.
- Gap / note: None.

## Reviewer / Lens Scorecard

| Lens | Conformance | Readiness | Top Risk | Confidence |
| --- | --- | --- | --- | --- |
| Rust architecture | Pass | Ready | Central policy module is shared rather than copied; no ownership drift found | High |
| Rust reliability | Pass | Ready | Session invalidation semantics distinguish preflight from poisoned output | High |
| Rust security | Pass | Ready | Shell/search boundary is fail-closed and caps resource exposure | High |
| API contract | Pass | Ready | Typed failure/readback vocabulary is explicit and tested | High |
| Observability/rollout | Pass | Ready | Health readback and focused gate are present; full branch signoff still needs unrelated changes isolated | Medium-high |

## Security-Sensitive Diff Scan Summary

Security-sensitive hard gate: triggered.

Triggered P096 surfaces:

- Filesystem and subprocess boundary: `rg`/`find` wrappers and permission preflight.
- DoS/resource-exhaustion boundary: per-output, cumulative-output, and session-store byte/line budgets.
- Public/semi-public runtime contract: JSON-RPC permission denial data and MCP `runtime.health` readback.
- Parser/input boundary: shell command tokenization and permission request inspection.

Reviewed P096 files/surfaces:

- `control-plane/crates/domain/src/tool_policy.rs`
- `control-plane/crates/acp/src/adapters/codex.rs`
- `control-plane/crates/acp/src/transport.rs`
- `control-plane/crates/engine/src/failure_classifier.rs`
- `control-plane/crates/engine/src/session/policy.rs`
- `control-plane/crates/mcp-server/src/tools/runtime.rs`
- `control-plane/crates/mcp-server/src/server.rs`
- `scripts/test-gate.sh`

Security pass verdict: Pass. No Critical or Major security findings found in the P096 slice.

Notes:

- The helper scan over the whole dirty worktree triggered many unrelated files and categories. Manual review scoped the security pass to P096-owned code paths.
- The preflight and wrappers reject broad searches rather than rewriting commands silently.
- Partial generated-root excludes are denied; complete exclude coverage is required.
- Output caps are enforced after allowed tool execution.
- Health readback exposes policy/budget metadata, not secrets or credentials.

## Routed Specialist Findings

No open Critical, Major, or Minor routed specialist findings.

### READY-RISK-001: Dirty Worktree Confounds Branch-Level Release Attribution

- Reviewer: observability_rollout_reviewer.
- Severity: Note.
- Confidence: High.
- Related REQ IDs: none; release hygiene only.
- Evidence types: diff, verification.
- Evidence references: `git status --short` shows many unrelated Swift/UI/docs changes outside the P096-owned slice.
- Why it matters: P096 conformance is proven, but branch-level release signoff should avoid mixing unrelated changes into the same handoff.
- Recommended action: Stage or branch P096-owned files separately before final merge/release signoff.
- Acceptance criteria: P096 handoff contains only the P096-owned code/docs/gate files, or the release note explicitly scopes unrelated changes separately.

## Readiness Checklist

| Check | Status | Evidence |
| --- | --- | --- |
| Canonical same-tree proposal gate | Passed | `./scripts/test-gate.sh proposal-096` |
| Full repository regression suite | Not run | Not required for successful P096 audit because same-tree canonical proposal gate passed; branch-wide signoff still should run the repo's normal release gate after unrelated changes are isolated |
| Core service flows | Passed | Preflight, wrappers, local activity monitor, classifier, session policy, health readback tests |
| UI/UX states | N/A | No user-visible UI in P096 scope |
| Accessibility/localization/entitlements | N/A | No Apple UI or entitlement change in P096 scope |
| Privacy/secrets | Passed | Security pass found health readback does not expose secrets/credentials |
| Critical tests executed | Passed | Targeted `domain`, `acp`, `engine`, and `mcp-server` cargo tests via `proposal-096` gate |
| Reference closeout | Passed | Retired proposal behavior lives in `docs/reference/bounded-tool-output-and-safe-search-policy.md` and is linked from reference index/baseline docs |

## Verification Log

Commands/evidence used:

- `md5 -q docs/reference/bounded-tool-output-and-safe-search-policy.md` -> `b7265e0c415f216f3fd6671e3ecb357b`.
- `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/report_path.py /Users/user/Documents/Chainworks Forge/docs/reference/bounded-tool-output-and-safe-search-policy.md` -> this R1 report path.
- `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/security_sensitive_diff.py --root /Users/user/Documents/Chainworks Forge --json` -> triggered security-sensitive review; manual P096-scoped security pass completed.
- `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/implementation_surface_fingerprint.py --root /Users/user/Documents/Chainworks Forge --json` -> whole-worktree helper over-triggered due unrelated dirty files; manual P096-scoped reviewer coverage completed.
- `./scripts/test-gate.sh proposal-096` -> passed. Covered static policy/health/classifier/quarantine/prompt checks plus targeted Rust tests:
  - `domain tool_policy`
  - `acp permission_preflight_denies_broad_rg_with_typed_error`
  - `acp codex_local_activity_classifies_cumulative_tool_output_budget`
  - `acp codex_local_activity_classifies_wrapper_truncation_marker_as_budget_exceeded`
  - `acp safe_search_wrapper`
  - `engine bounded_tool_output_classifies_before_provider_internal_fallback`
  - `engine tool_output_budget_failure_requires_session_invalidation`
  - `mcp-server proposal_096_runtime_health_includes_tool_output_guard`

## Final Verdict and Recommended Next Actions

Final verdict: P096 is implemented.

Recommended next actions:

1. Treat P096 conformance as closed.
2. Before branch-level handoff, isolate or stage the P096-owned files separately from unrelated dirty worktree changes.
3. Run the repository's normal broader release gate only after the release candidate is scoped, because the current dirty worktree is much larger than P096.
