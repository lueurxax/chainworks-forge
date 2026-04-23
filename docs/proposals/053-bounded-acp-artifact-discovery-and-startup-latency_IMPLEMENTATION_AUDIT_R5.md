# Proposal 053 Implementation Audit R5

## Metadata

| Field | Value |
| --- | --- |
| Proposal | `docs/proposals/053-bounded-acp-artifact-discovery-and-startup-latency.md` |
| Proposal revision | `p053-r12-ui-deferred-to-p069-2026-04-23` |
| Audit timestamp | `2026-04-23T21:52:12+0300 EEST` |
| Report path | `docs/proposals/053-bounded-acp-artifact-discovery-and-startup-latency_IMPLEMENTATION_AUDIT_R5.md` |
| Repo root | `/Users/user/Documents/Chainworks Forge` |
| Branch | `main` |
| Audited HEAD | `d17a447b5ae8e5ee1609bea906f08a89b3e8db36` |
| Implementation target | Current worktree, implicit compare target |
| Working tree status before report | Dirty: `.codex/config.toml` modified; unrelated untracked `docs/proposals/017-lead-mediated-workflow-conflict-resolution-and-mandatory-lead-validation_IMPLEMENTATION_AUDIT_R1.md` |
| Audit mode | `auto` implementation audit |
| Overall conformance | **Partial** |
| Overall implementation readiness | **Not Ready** for production exposure; gate-only/internal control-plane validation passes |
| Reviewer-selection reuse | **Not reused** |
| Audit confidence | Medium-high |

Line references to the proposal use the rendered `document_markdown` body from the JSON-wrapped proposal file.

## Implementation Target / Compare Base

The user supplied only the proposal path, so this audit targets the current worktree at HEAD `d17a447b5ae8e5ee1609bea906f08a89b3e8db36` plus local uncommitted state. No PR branch, diff range, or implementation directory was supplied.

This report is the only file written by the audit workflow.

## Prior Proposal-Review Reuse Summary

The proposal-review discovery helper found no reusable proposal-review artifacts for P053. The adjacent `docs/proposals/053.review/` directory contains implementation/evidence sidecars, not a prior reviewer-selection report:

- `cap-validation.json`
- `manual-latency-spot-check.md`
- `operator-clarity-evidence.md`
- `phase-1-retrospective.md`
- `proposal-053-gate-2026-04-23.md`
- `security-checklist.md`

Reuse state: **Not reused**. Reviewer routing was derived from the current proposal contract, `.codex/review-router.yaml`, `.codex/reviewers/chainworks-execution-truth.yaml`, and implementation evidence.

## Selected Reviewers

| Reviewer | Why selected |
| --- | --- |
| `rust_arch_reviewer` | P053 changes Rust workspace crate boundaries, domain discovery APIs, ACP transport, engine settlement, DB, workflow compiler, and tests. |
| `rust_reliability_reviewer` | P053 is sensitive to prompt/session sequencing, timeouts, cancellation, retry-bound legacy overrides, changed-files manifest execution, and stale/reused-session behavior. |
| `api_contract_reviewer` | P053 changes ACP request/result semantics, workflow YAML/Swift schema, GraphQL readback, MCP reports, diagnostic payloads, and artifact-settlement contracts. |
| `observability_rollout_reviewer` | P053 includes migrations, diagnostics, metrics, cap-validation artifacts, security checklist, gate evidence, and production exposure gates. |
| `chainworks_execution_truth_reviewer` | P053 changes durable AgentExecution discovery diagnostics, output settlement, artifact truth, runtime facts reconciliation, MCP truth, and ACP runtime truth. |

## Rejected Close Alternatives

| Reviewer | Reason not selected |
| --- | --- |
| `macos_ui_reviewer` | P053 explicitly defers macOS operator UI work to P069 and excludes UI implementation from sign-off. |
| `apple_arch_reviewer` | Swift work is limited to DSL/schema mirror compatibility; the primary implementation and risks are Rust/control-plane. |
| `rust_security_reviewer` | Path/root/symlink/cap validation is security-sensitive, but no unsafe/auth/secrets/public endpoint surface was introduced; these checks are covered through API, reliability, rollout evidence, and the P053 security checklist. |
| `rust_performance_reviewer` | Startup latency is central, but the implemented performance claim is mostly a sequencing/gate metric, not a separate benchmarked hot-path implementation. Covered under reliability and rollout. |
| `product_reviewer` | Product review remains opt-in by repo policy; metric and decision-gate concerns are covered by observability/rollout. |
| `ios_ui_reviewer` | No iOS target evidence. |

## Proposal State And Contract Summary

Proposal state: **Active**, with the proposal text stating "Ready for Phase 0" while "Phase 1 coding remains gated" and "macOS operator UI work is deferred to P069" (proposal rendered lines 3-6, 90-95, 708-714).

P053 commits to:

- Send ACP `initialize` promptly before any local repository, worktree, generated-state, exact-output, or broad discovery scan.
- Replace pre-init broad diffing with typed, bounded, engine-owned artifact discovery.
- Bind expected outputs to declared `ExpectedOutputSpec` contracts, authorized roots, reuse policy, byte caps, and run/stage/agent/prompt identity.
- Capture bounded pre-prompt metadata every execution and prompt turn, including reused sessions.
- Accept output bytes only through `OutputDiscoveryDecision` and downstream P057/P058 settlement, never from stale/rejected/escaped/unauthorized/oversized/wrong-root/wrong-run raw disk reads.
- Support bounded provider output envelopes, bounded meta-root supplemental discovery, changed-files manifests, and a disabled-by-default legacy broad discovery fallback.
- Persist durable diagnostics and expose GraphQL/MCP readback, operator clarity, metrics, and reconciliation signals.
- Validate caps and evidence before production exposure.
- Keep P053 macOS UI work out of scope and defer operator UI to P069/P031.

## Platform / Product Scope

| Scope | Classification |
| --- | --- |
| Apple | macOS app exists in repo, but P053 macOS UI is explicitly deferred. Swift DSL/schema mirror is in scope only for workflow compatibility. |
| Backend/service | Rust control-plane, ACP transport, workflow compiler, engine executor, DB persistence, GraphQL, MCP reports, diagnostics, rollout evidence. |
| Cross-stack | ACP/runtime truth -> engine settlement -> DB/runtime facts -> GraphQL/MCP readback -> future macOS UI via P069. |
| Product | Metric and decision gates matter, but product reviewer was not selected because the audit is implementation-readiness focused and UI/product journey work is deferred. |

Leading metric: `acp_pre_initialize_local_latency_ms`.

Guardrail metrics: missing/rejected/stale required-output counts, legacy broad discovery usage, meta-root truncation, provider envelope cap rejections, and reconciliation pending warnings.

Decision checkpoint: production exposure requires production sampling/signoff, minimal readback validation, and security/rollout approval beyond gate-only/internal evidence.

## Primary Service Flows

1. Fresh ACP session startup spawns the provider, opens logs, records pre-initialize local latency, and sends `initialize` before any local artifact discovery or broad scan.
2. Every agent prompt turn builds typed expected-output specs, captures bounded pre-prompt metadata, runs the prompt, then settles output decisions through provider envelopes, exact paths, generated outputs, reuse policy, and P057/P058 artifact truth.
3. Provider-supplied `CHAINWORKS_OUTPUT` envelopes and exact target reads are capped per output and in aggregate before artifact references become accepted settlement input.
4. Supplemental meta-root discovery and legacy broad discovery stay bounded, post-prompt, non-authoritative for required outputs, and auditable.
5. Discovery decisions and metrics persist to DB diagnostics and project through GraphQL/MCP readback with runtime-facts reconciliation.

## Implementation Fingerprint

Stack tags: `rust-backend`, `shared-api`, `macos` schema mirror, `cross-stack`.

Surface tags: ACP transport, engine executor, workflow schema/compiler, DB migration/repository, GraphQL, MCP reports, diagnostics payloads, test-gate, sidecar rollout evidence.

Risk tags: latency-sensitive, availability-sensitive, security-sensitive path validation, backward compatibility, durable artifact truth, stale/reused-session semantics, rollout evidence, operator support/debuggability.

Primary audited files and evidence:

- `control-plane/crates/domain/src/discovery.rs`
- `control-plane/crates/acp/src/transport.rs`
- `control-plane/crates/acp/src/lib.rs`
- `control-plane/crates/acp/src/manager.rs`
- `control-plane/crates/acp/src/session.rs`
- `control-plane/crates/engine/src/contracts.rs`
- `control-plane/crates/engine/src/executor.rs`
- `control-plane/crates/engine/src/git_manifest.rs`
- `control-plane/crates/workflow/src/definition.rs`
- `control-plane/crates/workflow/src/compiler.rs`
- `control-plane/crates/workflow/src/plan.rs`
- `control-plane/crates/db/migrations/026_p053_discovery_diagnostics.sql`
- `control-plane/crates/db/src/repos/agent_execution_discovery_diagnostics.rs`
- `control-plane/crates/graphql-server/src/types/stage.rs`
- `control-plane/crates/mcp-server/src/tools/reports.rs`
- `Chainworks Forge/DSL/WorkflowDefinition.swift`
- `scripts/test-gate.sh`
- `docs/proposals/053.review/*`

## Proposal Fidelity / Divergence Inventory

### Matches

- ACP startup sends `initialize` before session creation and before bounded discovery logic runs (`control-plane/crates/acp/src/transport.rs:1030-1209`).
- Pre-initialize latency is recorded and the P053 gate observed `acp_pre_initialize_local_latency_ms=0`.
- Generated-state denylist, exact-byte caps, metadata caps, meta-root caps, and legacy broad caps are encoded in domain constants and tests (`control-plane/crates/domain/src/discovery.rs:12-22`, `1581-2155`).
- `ExpectedOutputSpec`, `PrePromptExpectedOutputMetadata`, `OutputDiscoveryDecision`, provenance, reasons, and diagnostic payload structs are implemented in the domain crate (`control-plane/crates/domain/src/discovery.rs:117-630`).
- Provider envelope and JSON `CHAINWORKS_OUTPUT` extraction are capped before settlement (`control-plane/crates/acp/src/transport.rs:490-704`).
- Engine settlement uses accepted decisions and artifact references, with tests ensuring rejected/missing target paths are not reread (`control-plane/crates/engine/src/contracts.rs:517-567`, `730-830`; `control-plane/crates/engine/src/executor.rs:905-1095`, `4275-4340`).
- Bounded meta-root discovery is supplemental and excludes generated state while allowing logs (`control-plane/crates/domain/src/discovery.rs:1148-1280`; `control-plane/crates/engine/src/executor.rs:1413-1466`, `3472-3501`).
- Changed-files manifest generation is shell-free, timeout-bound, post-prompt, and preserves agent-authored manifests (`control-plane/crates/engine/src/git_manifest.rs:41-438`; `control-plane/crates/engine/src/executor.rs:3263-3316`).
- Legacy broad discovery is default disabled, capped, post-prompt, and controlled by workflow policy or audited retry-bound override (`control-plane/crates/workflow/src/definition.rs:40-50`; `control-plane/crates/acp/src/transport.rs:1263-1688`; DB override tests).
- Diagnostics readback projects through DB, GraphQL, and MCP (`control-plane/crates/db/src/repos/agent_execution_discovery_diagnostics.rs:43-272`; `control-plane/crates/graphql-server/src/types/stage.rs:237-292`; `control-plane/crates/mcp-server/src/tools/reports.rs:151-218`).
- Canonical same-tree P053 gate passed on the audited worktree.

### Divergences

- The proposal calls for `DiscoveryFilesystem` to be a trait in `domain::discovery` with a shared fake for the P053 gate (proposal rendered lines 421-446). The implementation has a concrete static `pub struct DiscoveryFilesystem` plus an operation recorder trait, not an injectable filesystem trait (`control-plane/crates/domain/src/discovery.rs:80`, `717-719`).
- The proposal requires stale output handling to be distinguishable from absent output in diagnostics and readback (proposal rendered lines 817-829, 833-845). The domain enum and metrics support `StaleExpectedOutput`, but engine production settlement for must-produce unchanged output emits `MissingAfterPrompt`, and tests assert that behavior (`control-plane/crates/engine/src/executor.rs:1324`, `5940`).
- Production cap/security evidence is explicitly gate-only/internal, not production-shippable. The cap-validation artifact says `phase_1_exposure_mode=gate_only_internal`, `production_shippable=false`, and no representative production execution IDs or production sizing distributions are available (`docs/proposals/053.review/cap-validation.json:7-38`, `86-98`).

### Ambiguities / Evidence Gaps

- No live daemon or end-to-end GraphQL/MCP runtime was started during this audit; readback was verified by code inspection and focused tests, not live operator use.
- The sidecar evidence is adequate for gate-only/internal control-plane validation, but not for production exposure.
- UI claims are intentionally not audited beyond confirming P053 excludes UI implementation and points to P069.
- Full repository regression was not run; the canonical proposal gate was run and passed, which is the relevant repo policy gate for this proposal.

## Requirement Summary

| Requirement | Status |
| --- | --- |
| REQ-001 Phase 0 evidence, cap validation, and exposure gating | Partially Implemented |
| REQ-002 Fresh ACP startup initializes before scans | Implemented |
| REQ-003 Pre-init scan prohibition and generated-state denylist | Implemented |
| REQ-004 ExpectedOutputSpec schema and authorized roots | Implemented |
| REQ-005 Per-turn bounded pre-prompt metadata | Implemented |
| REQ-006 OutputDiscoveryDecision-only settlement handoff | Implemented |
| REQ-007 Provider envelope caps and aggregate caps | Implemented |
| REQ-008 Exact target settlement validation and P057/P058 truth | Implemented |
| REQ-009 Bounded meta-root supplemental discovery | Implemented |
| REQ-010 Changed-files manifest contract | Implemented |
| REQ-011 Legacy broad discovery fallback and override controls | Implemented |
| REQ-012 Durable diagnostics and GraphQL/MCP readback | Implemented |
| REQ-013 Stale vs absent required-output truth | Partially Implemented |
| REQ-014 Workflow YAML and Swift DSL compatibility | Implemented |
| REQ-015 DiscoveryFilesystem trait/fake test seam | Partially Implemented |
| REQ-016 macOS UI deferral to P069 | Out of Scope |
| REQ-017 Canonical P053 gate evidence | Implemented |

Counts: 13 Implemented, 3 Partially Implemented, 1 Out of Scope, 0 Missing, 0 Not Verifiable.

## Detailed REQ Audit

### REQ-001 Phase 0 Evidence, Cap Validation, And Exposure Gating

- Proposal source: Phase 0 dependency/readiness and cap-validation artifact requirements, rendered lines 74-88, 590-655, 769-777.
- Status: **Partially Implemented**.
- Evidence types: proposal, code, tests-run, telemetry sidecar, rollout sidecar.
- Evidence references: `docs/proposals/053.review/cap-validation.json:7-38`, `86-98`; `docs/proposals/053.review/security-checklist.md`; `docs/proposals/053.review/manual-latency-spot-check.md`.
- Implementation mapping: evidence sidecars exist, name the exposure mode, record dependency owner readiness, capture gate/manual fixture latency, and declare production gaps.
- Gap / note: the artifact explicitly says gate-only/internal and `production_shippable=false`; it does not provide representative production execution IDs, production p50/p90/p99 sizing, or production signoff.

### REQ-002 Fresh ACP Startup Initializes Before Scans

- Proposal source: ACP startup sequence and disallowed pre-initialize operations, rendered lines 100-122.
- Status: **Implemented**.
- Evidence types: code, tests-run, telemetry sidecar.
- Evidence references: `control-plane/crates/acp/src/transport.rs:1030-1209`; `docs/proposals/053.review/manual-latency-spot-check.md`; `./scripts/test-gate.sh proposal-053`.
- Implementation mapping: transport spawns/logs, records pre-initialize local latency, sends `initialize`, then proceeds to session/prompt/discovery logic. Gate output observed `acp_pre_initialize_local_latency_ms=0`.
- Gap / note: no gap for gate-only/internal validation.

### REQ-003 Pre-Init Scan Prohibition And Generated-State Denylist

- Proposal source: disallowed pre-initialize operations and generated-state denylist, rendered lines 118-150.
- Status: **Implemented**.
- Evidence types: code, tests-run.
- Evidence references: `control-plane/crates/domain/src/discovery.rs:716-901`, `1034-1084`, `1282-1431`, `1581-1655`; `./scripts/test-gate.sh proposal-053`.
- Implementation mapping: domain filesystem helpers skip `.chainworks/worktrees`, `.chainworks/backups`, `.forge-codex-acp`, `.claude/worktrees`, `.git/objects`, target/build/cache directories, DB backups, and SQLite files; operation recorder tests verify no generated-state reads for bounded discovery.
- Gap / note: no gap for the audited paths.

### REQ-004 ExpectedOutputSpec Schema And Authorized Roots

- Proposal source: ExpectedOutputSpec contract, rendered lines 152-178, 396-446.
- Status: **Implemented**.
- Evidence types: code, tests-found, tests-run.
- Evidence references: `control-plane/crates/domain/src/discovery.rs:117-230`; `control-plane/crates/engine/src/contracts.rs:371-501`, `730-830`; `./scripts/test-gate.sh proposal-053`.
- Implementation mapping: domain spec fields cover IDs, target path, authorized root class/path, required/reuse policy, caps, settlement owner, and manifest generation. Engine derives specs from declared outputs and worktree/meta/workspace roots.
- Gap / note: the filesystem trait seam is tracked separately under REQ-015.

### REQ-005 Per-Turn Bounded Pre-Prompt Metadata

- Proposal source: pre-prompt metadata requirements, rendered lines 180-231.
- Status: **Implemented**.
- Evidence types: code, tests-run.
- Evidence references: `control-plane/crates/domain/src/discovery.rs:255-276`, `903-1134`, `1581-2155`; `control-plane/crates/acp/src/transport.rs:1263-1688`; `control-plane/crates/acp/src/manager.rs`; `control-plane/crates/acp/src/session.rs`; `./scripts/test-gate.sh proposal-053`.
- Implementation mapping: metadata includes agent/stage/attempt/session/prompt/discovery identity, records status, caps specs/bytes/time, uses symlink metadata and canonical roots, and is captured in the prompt path used by fresh and reused sessions.
- Gap / note: no gap found.

### REQ-006 OutputDiscoveryDecision-Only Settlement Handoff

- Proposal source: OutputDiscoveryDecision model and engine-owned settlement, rendered lines 237-285, 817-845.
- Status: **Implemented**.
- Evidence types: code, tests-found, tests-run.
- Evidence references: `control-plane/crates/domain/src/discovery.rs:306-385`; `control-plane/crates/engine/src/contracts.rs:517-567`, `730-830`; `control-plane/crates/engine/src/executor.rs:905-1095`, `4275-4340`; `./scripts/test-gate.sh proposal-053`.
- Implementation mapping: accepted decisions carry artifact payload refs; rejected/missing decisions do not expose accepted bytes/artifact refs; artifact persistence gates on accepted discovery decisions.
- Gap / note: stale reason classification is tracked separately under REQ-013.

### REQ-007 Provider Envelope Caps And Aggregate Caps

- Proposal source: provider envelope and byte-cap requirements, rendered lines 287-312.
- Status: **Implemented**.
- Evidence types: code, tests-run.
- Evidence references: `control-plane/crates/acp/src/transport.rs:196-200`, `490-704`; `control-plane/crates/engine/src/executor.rs:905-1045`; `./scripts/test-gate.sh proposal-053`.
- Implementation mapping: ACP extracts `<<<CHAINWORKS_OUTPUT:name>>>` and JSON `CHAINWORKS_OUTPUT` envelopes using bounded readers; oversized payloads become cap-exceeded evidence before settlement; engine enforces per-output and aggregate caps.
- Gap / note: production sizing validation remains partial under REQ-001.

### REQ-008 Exact Target Settlement Validation And P057/P058 Truth

- Proposal source: exact output acceptance, P057/P058 settlement, security exit criteria, and implementation contracts, rendered lines 237-285, 679-683, 817-845.
- Status: **Implemented**.
- Evidence types: code, tests-found, tests-run.
- Evidence references: `control-plane/crates/engine/src/executor.rs:905-1156`, `1230-1297`, `3433-3560`, `4275-4340`; `control-plane/crates/engine/src/contracts.rs:517-567`; `./scripts/test-gate.sh proposal-053`.
- Implementation mapping: exact reads reject symlinks, unauthorized roots, wrong-run metadata, oversize payloads, and missing files; accepted outputs are materialized and persisted through artifact/runtime facts paths.
- Gap / note: no raw reread bypass was found for rejected/missing decisions.

### REQ-009 Bounded Meta-Root Supplemental Discovery

- Proposal source: meta-root requirements, rendered lines 314-334.
- Status: **Implemented**.
- Evidence types: code, tests-run.
- Evidence references: `control-plane/crates/domain/src/discovery.rs:1148-1280`; `control-plane/crates/engine/src/executor.rs:1413-1466`, `3472-3501`; `./scripts/test-gate.sh proposal-053`.
- Implementation mapping: meta-root discovery rejects symlink roots, canonicalizes roots, reads only regular files under caps, skips generated directories, preserves logs, and only appends supplemental artifact paths after required-output settlement.
- Gap / note: no gap found.

### REQ-010 Changed-Files Manifest Contract

- Proposal source: changed-files manifest contract, rendered lines 336-359.
- Status: **Implemented**.
- Evidence types: code, tests-run.
- Evidence references: `control-plane/crates/engine/src/git_manifest.rs:41-438`; `control-plane/crates/engine/src/executor.rs:3263-3316`; `./scripts/test-gate.sh proposal-053`.
- Implementation mapping: manifest generation is declared-output gated, post-prompt, before settlement, shell-free via `Command::new("git")`, cwd-bound to the worktree, timeout-bound, kill-on-drop, and preserves `.agent.json`.
- Gap / note: no gap found.

### REQ-011 Legacy Broad Discovery Fallback And Override Controls

- Proposal source: legacy broad discovery requirements, rendered lines 361-390.
- Status: **Implemented**.
- Evidence types: code, schema, migration, tests-run.
- Evidence references: `control-plane/crates/domain/src/discovery.rs:1282-1431`; `control-plane/crates/workflow/src/definition.rs:40-50`; `control-plane/crates/workflow/src/compiler.rs:45-57`; `control-plane/crates/db/migrations/026_p053_discovery_diagnostics.sql`; `./scripts/test-gate.sh proposal-053`.
- Implementation mapping: workflow policy defaults disabled, opt-in values are parsed/validated, legacy broad discovery runs only after prompt when allowed, caps are enforced, and retry-bound override records are validated/consumed through DB tests.
- Gap / note: no gap found for the audited policy and override path.

### REQ-012 Durable Diagnostics And GraphQL/MCP Readback

- Proposal source: diagnostics/readback, metrics, and implementation contracts, rendered lines 728-765, 817-845.
- Status: **Implemented**.
- Evidence types: code, migration, tests-run.
- Evidence references: `control-plane/crates/domain/src/discovery.rs:396-630`; `control-plane/crates/db/migrations/026_p053_discovery_diagnostics.sql`; `control-plane/crates/db/src/repos/agent_execution_discovery_diagnostics.rs:43-272`; `control-plane/crates/graphql-server/src/types/stage.rs:237-292`; `control-plane/crates/mcp-server/src/tools/reports.rs:151-218`; `./scripts/test-gate.sh proposal-053`.
- Implementation mapping: diagnostics persist schema version, missing/rejected/stale counts, legacy usage, meta truncation, manifest status, resume warning count, payload JSON, reconciliation warnings, and readback projections through GraphQL/MCP.
- Gap / note: the readback plumbing exists, but stale count production depends on REQ-013.

### REQ-013 Stale Vs Absent Required-Output Truth

- Proposal source: acceptance criteria and implementation contracts requiring stale/rejected/escaped/unauthorized/oversized/wrong-root/wrong-run handling, rendered lines 817-845.
- Status: **Partially Implemented**.
- Evidence types: code, tests-found, tests-run.
- Evidence references: `control-plane/crates/domain/src/discovery.rs:324-325`, `519`, `2132-2151`; `control-plane/crates/engine/src/executor.rs:1324`, `3331`, `5940`, `6052`; `./scripts/test-gate.sh proposal-053`.
- Implementation mapping: domain types and metrics support `stale_expected_output`; tests can serialize and count stale decisions.
- Gap / note: production engine settlement for a must-produce unchanged exact output currently emits `MissingAfterPrompt`, and the focused test asserts that behavior. Operators and API consumers cannot reliably distinguish "stale unchanged prior output" from "absent after prompt" from the decision reason/count alone.

### REQ-014 Workflow YAML And Swift DSL Compatibility

- Proposal source: workflow compatibility, output policies, legacy broad policy, and future UI contract, rendered lines 361-390, 448-528, 833-845.
- Status: **Implemented**.
- Evidence types: code, tests-run.
- Evidence references: `control-plane/crates/workflow/src/definition.rs:40-50`, `107-128`; `control-plane/crates/workflow/src/plan.rs`; `control-plane/crates/workflow/src/compiler.rs:45-57`; `Chainworks Forge/DSL/WorkflowDefinition.swift:43-54`, `119-157`; workflow integration tests in `./scripts/test-gate.sh proposal-053`.
- Implementation mapping: Rust workflow schema/compiler and Swift DSL mirror include discovery policy, output policies, and reuse policy values while rejecting unknown values in compatibility tests.
- Gap / note: no gap found.

### REQ-015 DiscoveryFilesystem Trait/Fake Test Seam

- Proposal source: architecture contract for `domain::discovery` filesystem boundary and shared fake, rendered lines 421-446.
- Status: **Partially Implemented**.
- Evidence types: code, tests-run.
- Evidence references: `control-plane/crates/domain/src/discovery.rs:80`, `717-719`; `./scripts/test-gate.sh proposal-053`.
- Implementation mapping: discovery logic is centralized in `domain::discovery`; `DiscoveryOperationRecorder` provides operation-level observability and the gate verifies bounded operations.
- Gap / note: `DiscoveryFilesystem` is not a trait and there is no shared fake filesystem implementation as specified. Tests use real temp filesystem behavior plus operation recording.

### REQ-016 macOS UI Deferral To P069

- Proposal source: UI deferral and non-goals, rendered lines 58-68, 90-95, 708-714.
- Status: **Out of Scope**.
- Evidence types: proposal.
- Evidence references: proposal rendered lines 90-95 and 708-714.
- Implementation mapping: P053 sign-off is Rust control-plane/runtime/API/readback. Future macOS UI is assigned to P069/P031 and must use GraphQL.
- Gap / note: no UI implementation was audited or required for this P053 report.

### REQ-017 Canonical P053 Gate Evidence

- Proposal source: Phase 1/2 exit criteria and acceptance criteria, rendered lines 661-699, 817-829.
- Status: **Implemented**.
- Evidence types: tests-run.
- Evidence references: `./scripts/test-gate.sh proposal-053` on audited worktree at HEAD `d17a447b5ae8e5ee1609bea906f08a89b3e8db36`.
- Implementation mapping: the repository canonical proposal gate ran same-tree and passed.
- Gap / note: this is proposal-gate evidence, not full repository regression or production runtime evidence.

## Reviewer / Lens Scorecard

| Lens | Result | Top risk | Confidence |
| --- | --- | --- | --- |
| Objective proposal conformance | Partial | Production cap/security evidence is gate-only, stale classification is partial, filesystem trait seam diverges. | Medium-high |
| Rust architecture | Partial | `DiscoveryFilesystem` is centralized but not implemented as the proposed trait/fake boundary. | Medium |
| Rust reliability | Pass with caveat | Timeouts, caps, post-prompt ordering, and retry override controls are covered; stale-vs-missing semantics weaken recovery diagnostics. | Medium-high |
| API contract | Partial | Diagnostic schema exposes stale counts, but production decisions do not emit stale for must-produce unchanged outputs. | High |
| Observability/rollout | Not ready for production | Sidecars explicitly approve gate-only/internal exposure, not production. | High |
| Chainworks execution truth | Partial | Accepted/rejected artifact truth is strong; stale truth is not distinct enough in production settlement. | High |
| Readiness | Not Ready | Major production exposure and stale-truth findings remain. | High |

## Routed Specialist Findings

### READY-001 Production Exposure Is Still Blocked By Gate-Only Evidence

- Reviewer: `observability_rollout_reviewer`
- Severity: Major
- Confidence: High
- Related requirements: REQ-001, REQ-017
- Evidence types: rollout sidecar, telemetry sidecar, tests-run
- Evidence references: `docs/proposals/053.review/cap-validation.json:7-38`, `86-98`; `docs/proposals/053.review/security-checklist.md`; `docs/proposals/053.review/manual-latency-spot-check.md`; `./scripts/test-gate.sh proposal-053`
- Why it matters: the implementation can pass the P053 gate and still be unsafe to call production-ready because the evidence artifact explicitly says production exposure is not approved and production sizing/signoff data is missing.
- Recommended action: keep P053 restricted to gate-only/internal validation until production sampling and readback signoff are refreshed.
- Acceptance criteria: cap-validation records representative recent production execution IDs or an explicitly approved replacement sample; p50/p90/p99 output sizing and cap-hit rates are populated; security/readback signoff changes `production_shippable` to true; rollout notes identify owners, rollback, and exposure mode.

### API-001 Stale Required Outputs Collapse Into Missing-After-Prompt

- Reviewer: `api_contract_reviewer`, `chainworks_execution_truth_reviewer`
- Severity: Major
- Confidence: High
- Related requirements: REQ-013, REQ-012
- Evidence types: code, tests-found, tests-run
- Evidence references: `control-plane/crates/domain/src/discovery.rs:324-325`, `519`; `control-plane/crates/engine/src/executor.rs:1324`, `3331`, `5940`; `./scripts/test-gate.sh proposal-053`
- Why it matters: the proposal calls out stale output safety and operator clarity. The API schema exposes `stale_output_count`, but the engine path that detects a must-produce unchanged prior output currently reports `MissingAfterPrompt`. That erases the distinction between "no output appeared" and "a prior output stayed unchanged" for readback, metrics, and support diagnostics.
- Recommended action: when a must-produce output had pre-prompt metadata and no current invocation provenance, emit an `OutputDiscoveryDecision` with reason `StaleExpectedOutput` or add an equivalent explicit stale state that increments `stale_output_count` and survives GraphQL/MCP readback.
- Acceptance criteria: production settlement code constructs stale decisions for unchanged must-produce outputs; tests that currently expect `MissingAfterPrompt` for this scenario are updated; DB/GraphQL/MCP readback shows nonzero stale counts for the fixture.

### ARCH-001 Filesystem Boundary Does Not Match The Proposed Trait/Fake Contract

- Reviewer: `rust_arch_reviewer`
- Severity: Minor
- Confidence: Medium
- Related requirements: REQ-015
- Evidence types: code, tests-run
- Evidence references: proposal rendered lines 421-446; `control-plane/crates/domain/src/discovery.rs:80`, `717-719`; `./scripts/test-gate.sh proposal-053`
- Why it matters: the current static filesystem helper and operation recorder validate behavior well, but the proposal promised an injectable `DiscoveryFilesystem` trait and shared fake. That matters for future deterministic test coverage around filesystem error cases, latency, symlink races, and generated-state traversal.
- Recommended action: either introduce the trait/fake seam as proposed or amend the implementation contract/reference docs to bless the operation-recorder approach and explain why it gives equivalent testability.
- Acceptance criteria: a trait-backed fake is used by P053 gate tests, or proposal/reference truth is updated to reflect the concrete helper plus recorder design with equivalent coverage for failure modes.

## Readiness Checklist

| Check | Status | Evidence / note |
| --- | --- | --- |
| Canonical build/gate | Passed | `./scripts/test-gate.sh proposal-053` passed on the audited worktree. |
| Full regression suite | Not run | Repo policy points P053 validation to `scripts/test-gate.sh proposal-053`; no full-suite success is claimed. |
| Core service flow integration validation | Partially satisfied | Focused Rust/unit/integration gate passed, including ACP adapter fixture and DB/GraphQL/MCP readback tests. No live daemon/operator runtime was started. |
| Startup latency evidence | Gate-only pass | Gate output observed `acp_pre_initialize_local_latency_ms=0`; sidecar manual spot-check also reports 0 ms. Production P99 evidence absent. |
| Security/path validation | Gate-only pass | Symlink/root/generated-state/cap tests and security checklist exist; checklist is not production signoff. |
| API/schema contract validation | Passed with stale caveat | Workflow/ACP/GraphQL/MCP diagnostics are implemented and tested; stale reason production is partial. |
| Migration/readback | Passed in focused tests | DB diagnostics migration/repository tests and GraphQL/MCP projection tests passed under the proposal gate. |
| UI empty/loading/error/offline/permissions | Out of scope | P053 UI deferred to P069. |
| Accessibility/localization/entitlements | Out of scope | No P053 UI or entitlement change audited. |
| Privacy/PII risk | Low/unchanged | Audit found path/digest/payload diagnostics, not new PII/secrets handling. Security checklist remains gate-only. |
| Production exposure readiness | Not ready | Cap-validation sidecar explicitly says `production_shippable=false`. |

## Verification Log

Commands and checks run during this audit:

| Command / check | Result |
| --- | --- |
| `pwd`, `git rev-parse --show-toplevel`, `git rev-parse HEAD`, `git branch --show-current` | Repo resolved to `/Users/user/Documents/Chainworks Forge`, branch `main`, HEAD `d17a447b5ae8e5ee1609bea906f08a89b3e8db36`. |
| `git status --short` | Dirty worktree before report: `.codex/config.toml` modified, unrelated P017 audit report untracked. |
| `python3 .../report_path.py .../053-bounded-acp-artifact-discovery-and-startup-latency.md` | Report path resolved to this R5 file; prior R1-R4 not overwritten. |
| `python3 .../discover_prior_review.py .../053-bounded-acp-artifact-discovery-and-startup-latency.md` | No prior proposal-review artifacts discovered. |
| `jq -r '.document_markdown' ... | nl -ba` | Proposal contract extracted from rendered Markdown. |
| Focused `rg`, `sed`, and `nl` inspections across domain, ACP, engine, workflow, DB, GraphQL, MCP, Swift DSL, router, and P053 sidecars | Implementation evidence mapped to REQ items and reviewer findings. |
| `./scripts/test-gate.sh list` | Confirmed `proposal-053` gate is available. |
| `./scripts/test-gate.sh proposal-053` | Passed. Output included ACP adapter fixture with `observed acp_pre_initialize_local_latency_ms=0` and ended with `Proposal 053 control-plane gate passed`. |

Tests found:

- Domain discovery tests for denylist, recorder ordering, metadata bounds, meta-root bounds, legacy broad caps, and serialization.
- ACP transport tests for provider output envelopes and legacy broad default-disabled behavior.
- Engine tests for expected output specs, meta-root supplemental paths, changed-files manifest, settlement, artifact persistence, and stale/missing behavior.
- Workflow integration tests for output policies and legacy broad policy compatibility.
- DB diagnostics tests for roundtrip/readback and legacy override behavior.
- GraphQL/MCP projection tests for discovery reconciliation pending.

Tests run:

- `./scripts/test-gate.sh proposal-053` on the audited worktree.

Runtime evidence:

- No live daemon/operator runtime was launched by this audit.
- ACP adapter fixture in the proposal gate executed and observed `acp_pre_initialize_local_latency_ms=0`.

## Final Verdict

Overall conformance: **Partial**.

Overall implementation readiness: **Not Ready** for production exposure. The implementation is substantially complete for gate-only/internal control-plane validation and passes the same-tree P053 gate, but it cannot be reported production-ready while production cap/security evidence remains explicitly unapproved and stale required-output truth is not distinctly emitted in production settlement.

Recommended next actions:

1. Close `API-001` by producing explicit stale output decisions/counts for must-produce unchanged prior outputs and verifying GraphQL/MCP readback.
2. Close `READY-001` with representative production cap sampling, production readback/security signoff, and updated exposure mode.
3. Resolve `ARCH-001` by implementing the proposed filesystem trait/fake seam or updating durable reference truth to bless the recorder-based design.
