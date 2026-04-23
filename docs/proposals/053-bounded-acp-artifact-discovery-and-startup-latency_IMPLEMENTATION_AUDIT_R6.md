# Proposal 053 Implementation Audit R6

## Metadata

| Field | Value |
| --- | --- |
| Proposal | `docs/proposals/053-bounded-acp-artifact-discovery-and-startup-latency.md` |
| Proposal revision | `p053-r12-ui-deferred-to-p069-2026-04-23` |
| Audit timestamp | `2026-04-23T22:22:32+0300 EEST` |
| Report path | `docs/proposals/053-bounded-acp-artifact-discovery-and-startup-latency_IMPLEMENTATION_AUDIT_R6.md` |
| Repo root | `/Users/user/Documents/Chainworks Forge` |
| Branch | `main` |
| Audited HEAD | `c9a59787ee75c5363e484656a42e651616da56ab` |
| Implementation target | Current worktree, implicit compare target |
| Compare base | Implicit current tree; no PR branch or diff range supplied |
| Working tree status before final report update | Clean except this new untracked R6 audit report |
| Overall conformance | **Partial** |
| Overall implementation readiness | **Not Ready** |
| Reviewer-selection reuse | **Not reused** |
| Audit confidence | Medium-high |

Line references to the proposal use the rendered `document_markdown` body from the JSON-wrapped proposal file.

## Implementation Target / Compare Base

The user supplied only the proposal path, so this audit targets the current worktree at HEAD `c9a59787ee75c5363e484656a42e651616da56ab` plus this new untracked report. The current HEAD is `chore: update agent catalog and p017 audit`; its parent `14ad5e55327c7b8ef9738defc413d97b8e152ed0` is the P053 closeout commit `fix: close p053 audit r5 blockers`.

The current HEAD includes non-P053 agent catalog/config/P017 audit changes after the P053 closeout commit. The P053 gate passed on this HEAD, but those adjacent changes are not part of the P053 proof surface.

## Prior Proposal-Review Reuse Summary

The proposal-review discovery helper returned no prior proposal-review artifacts for P053. The adjacent `docs/proposals/053.review/` directory contains implementation evidence sidecars, not a prior reviewer-selection report.

Reuse state: **Not reused**.

The prior implementation audit R5 is present in history and was not reused for reviewer routing, per the audit skill. It is relevant context because HEAD is explicitly a closeout commit for R5 findings.

## Selected Reviewers

| Reviewer | Why selected |
| --- | --- |
| `rust_arch_reviewer` | Rust crate/API ownership, `domain::discovery`, ACP transport, engine settlement, filesystem seam, and testability boundaries. |
| `rust_reliability_reviewer` | Fresh/reused ACP session sequencing, bounded metadata, timeouts, cancellation-aware manifest behavior, stale output handling, and legacy override controls. |
| `api_contract_reviewer` | ACP request/result shape, workflow YAML/Swift schema mirrors, diagnostics payloads, GraphQL/MCP readback, and stale/missing/rejected contract semantics. |
| `observability_rollout_reviewer` | Cap-validation evidence, production exposure decision, security checklist, metrics, gate registration, rollout/readiness sidecars, and branch-scope handoff risk. |
| `chainworks_execution_truth_reviewer` | Durable AgentExecution discovery diagnostics, runtime facts, accepted artifact truth, reconciliation, MCP truth, and ACP runtime truth. |

## Rejected Close Alternatives

| Reviewer | Reason not selected |
| --- | --- |
| `macos_ui_reviewer` | P053 explicitly defers macOS operator UI to P069 and says missing UI must not block P053 readiness. |
| `apple_arch_reviewer` | Swift scope is limited to DSL/schema mirror compatibility; primary implementation risk is Rust/control-plane. |
| `rust_security_reviewer` | Security-sensitive path/root/cap behavior is covered by source tests and the security checklist; no auth, unsafe, secrets, or public network boundary change was found beyond selected API/rollout lenses. |
| `rust_performance_reviewer` | Startup latency is validated as a sequencing and timing-attribution contract through the P053 gate, not a separate benchmarked hot path. |
| `product_reviewer` | Product-owner decision evidence is relevant, but the concrete issue is rollout/signoff consistency and is covered by `observability_rollout_reviewer`. |
| `ios_ui_reviewer` | No iOS target evidence. |

## Proposal State And Contract Summary

Proposal state: **Active**. The proposal says Phase 1 coding remains gated on dependency readiness, contract freeze, cap validation, production-exposure decision, and security review artifact, while macOS UI is deferred to P069 and not part of P053 sign-off.

The implementation contract requires:

- Fresh ACP startup sends `initialize` before repository, workspace, worktree, generated-state, broad Git, or exact output traversal.
- Typed expected outputs drive bounded pre-prompt metadata after session selection and before every prompt turn, including reused sessions.
- Output acceptance flows through `OutputDiscoveryDecision`, `CapturedOutput`, P057/P058 settlement, runtime facts, and diagnostics.
- Stale, absent, rejected, unauthorized, escaped, oversized, timeout, wrong-root, and wrong-run states are durable and visible.
- Provider envelopes and `CHAINWORKS_OUTPUT` payloads share declared-output caps.
- Supplemental discovery is bounded to current-run meta-root.
- Legacy broad discovery is disabled by default, capped, audited, post-prompt only, and temporary.
- `DiscoveryFilesystem` is the injectable filesystem boundary in `domain::discovery` and is used by transport and engine discovery paths for traversal, metadata, canonicalization, operation recording, and reads.
- Production exposure requires durable readback, cap-validation evidence, and security/architecture signoff.

## Platform / Product Scope

| Scope | Classification |
| --- | --- |
| Apple | macOS app exists in repo, but P053 UI is explicitly out of scope. Swift DSL/schema mirror compatibility is in scope. |
| Backend/service | Rust control-plane, ACP transport, workflow compiler, engine executor, DB persistence, GraphQL, MCP reports, diagnostics, rollout evidence. |
| Cross-stack | ACP/runtime truth -> engine settlement -> DB/runtime facts -> GraphQL/MCP readback -> future P069 UI through GraphQL. |
| Product | Production exposure and guardrail metrics are central; user-facing UI value remains deferred to P069. |

Leading metric: `acp_pre_initialize_local_latency_ms`.

Guardrail metrics: missing/rejected/stale output counts, legacy broad discovery usage, meta-root truncation, provider envelope cap rejections, aggregate cap hits, and reconciliation pending warnings.

Decision checkpoint: production exposure requires consistent cap-validation/security/retrospective evidence and a proposal-compliant fallback approval path when production execution IDs are unavailable.

## Primary Service Flows

1. Fresh ACP startup opens the provider and sends `initialize` before local discovery work, with Forge overhead separated from provider latency.
2. Every prompt turn builds typed expected output specs, captures bounded pre-prompt metadata, runs the provider prompt, and settles output decisions.
3. Output bytes from provider envelopes, exact paths, generated manifests, and allowed reuse policy are capped and converted into accepted or non-accepted decisions.
4. Stale must-produce exact outputs remain missing/stale rather than accepted unless current-invocation provenance or `allow_unchanged_existing` applies.
5. Durable diagnostics persist to DB and project through GraphQL/MCP readback for support, reports, and future P069 UI.

## Implementation Fingerprint

Stack tags: `rust-backend`, `shared-api`, `macos` schema mirror, `cross-stack`.

Surface tags: ACP transport, engine settlement, domain discovery types, workflow schema/compiler, DB migration/repository, GraphQL, MCP reports, diagnostics payloads, test-gate, sidecar rollout evidence.

Risk tags: latency-sensitive, durable artifact truth, stale/reused-session correctness, path/cap validation, production exposure, test seam coverage, branch-scope handoff, backward compatibility.

Primary audited files and evidence:

- `control-plane/crates/domain/src/discovery.rs`
- `control-plane/crates/acp/src/transport.rs`
- `control-plane/crates/engine/src/contracts.rs`
- `control-plane/crates/engine/src/executor.rs`
- `control-plane/crates/engine/src/git_manifest.rs`
- `control-plane/crates/workflow/src/definition.rs`
- `control-plane/crates/workflow/src/compiler.rs`
- `control-plane/crates/db/migrations/026_p053_discovery_diagnostics.sql`
- `control-plane/crates/db/src/repos/agent_execution_discovery_diagnostics.rs`
- `control-plane/crates/graphql-server/src/types/stage.rs`
- `control-plane/crates/graphql-server/tests/proposal_058_runtime_facts.rs`
- `control-plane/crates/mcp-server/src/tools/reports.rs`
- `control-plane/crates/mcp-server/tests/proposal_058_runtime_facts.rs`
- `Chainworks Forge/DSL/WorkflowDefinition.swift`
- `scripts/test-gate.sh`
- `docs/reference/test-gates.md`
- `docs/proposals/053.review/*`

## Proposal Fidelity / Divergence Inventory

### Matches

- Fresh ACP startup sends `initialize` before session/new and before P053 metadata/discovery work (`control-plane/crates/acp/src/transport.rs:1030-1209`).
- The P053 gate passed and observed `acp_pre_initialize_local_latency_ms=0`.
- Domain discovery constants and tests cover generated-state denylist, metadata bounds, exact caps, meta-root caps, legacy broad caps, and operation ordering.
- `ExpectedOutputSpec`, `PrePromptExpectedOutputMetadata`, `OutputDiscoveryDecision`, discovery diagnostics, and reason/provenance enums are implemented.
- Provider envelope and JSON `CHAINWORKS_OUTPUT` extraction are capped before settlement.
- Engine settlement accepts provider/exact/control-plane/reuse outputs only through decisions and accepted payload refs.
- Stale must-produce unchanged outputs now produce `OutputDiscoveryReason::StaleExpectedOutput` and remain non-accepted (`control-plane/crates/engine/src/executor.rs:956-959`, `1344-1388`, `5983-5998`).
- GraphQL and MCP tests assert stale output count readback (`control-plane/crates/graphql-server/tests/proposal_058_runtime_facts.rs:497-570`; `control-plane/crates/mcp-server/tests/proposal_058_runtime_facts.rs:494-547`).
- `DiscoveryFilesystem` now exists as a trait with `StdDiscoveryFilesystem` and `FakeDiscoveryFilesystem` implementations (`control-plane/crates/domain/src/discovery.rs:750-896`).
- The P053 gate now includes a trait-backed fake test and stale-vs-absent readback tests.
- Cap-validation and security artifacts now claim production exposure from an approved replacement sample.

### Divergences

- Production-exposure evidence is internally inconsistent: `cap-validation.json` and `security-checklist.md` approve production exposure, while `manual-latency-spot-check.md` and `phase-1-retrospective.md` still state production exposure requires refreshed production sampling/signoff.
- The production fallback approval is recorded as a control-plane owner decision, not the proposal's "control-plane tech lead plus product owner" fallback decision.
- The `DiscoveryFilesystem` trait exists, but the trait does not cover generic exact reads/canonicalization and is not injected through ACP/engine discovery paths. Engine settlement still performs direct filesystem reads/canonicalization for control-plane generated outputs, declared reuse policy, stale detection, and canonical path projection (`control-plane/crates/engine/src/executor.rs:1244-1248`, `1281-1287`, `1360-1364`, `1442-1446`).
- The fake filesystem gate test proves the fake itself works, but it is a domain-level test; it does not drive the fresh-session and reused-session transport/engine paths through an injected fake as the proposal specifies.

### Ambiguities / Evidence Gaps

- No live daemon or operator UI runtime was started during this audit.
- No full repo regression was run; the canonical P053 gate was run and passed.
- Current HEAD includes non-P053 `examples/agents`, `.codex/config.toml`, and P017 audit changes after the P053 closeout commit; they are outside the P053 gate proof.
- Production data was unavailable; the current evidence uses an approved replacement sample, but adjacent sidecars disagree on whether that approval is sufficient.

## Requirement Summary

| Requirement | Status |
| --- | --- |
| REQ-001 Phase 0 evidence, cap validation, and production-exposure decision | Partially Implemented |
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
| REQ-013 Stale vs absent required-output truth | Implemented |
| REQ-014 Workflow YAML and Swift DSL compatibility | Implemented |
| REQ-015 DiscoveryFilesystem injectable boundary and fake gate coverage | Partially Implemented |
| REQ-016 macOS UI deferral to P069 | Out of Scope |
| REQ-017 Canonical P053 gate evidence | Implemented |

Counts: 14 Implemented, 2 Partially Implemented, 1 Out of Scope, 0 Missing, 0 Not Verifiable.

## Detailed REQ Audit

### REQ-001 Phase 0 Evidence, Cap Validation, And Production-Exposure Decision

- Proposal source: Phase 0 readiness and cap-validation exit criteria, rendered lines 590-655; Phase 1 production exposure, rendered lines 661-683.
- Status: **Partially Implemented**.
- Evidence types: proposal, code, tests-run, telemetry sidecar, rollout sidecar.
- Evidence references: `docs/proposals/053.review/cap-validation.json:7-46`, `69-75`, `115-120`; `docs/proposals/053.review/security-checklist.md:8`, `26`, `46`; `docs/proposals/053.review/manual-latency-spot-check.md:73`; `docs/proposals/053.review/phase-1-retrospective.md:27`; `./scripts/test-gate.sh proposal-053`.
- Implementation mapping: cap-validation now records `phase_1_exposure_mode=production_exposed`, non-null p50/p90/p99 values, approved replacement sample status, dependency readiness, interface freeze, and reviewer signoff fields. Security checklist approves P053 control-plane/API/readback production exposure.
- Gap / note: the required evidence set is contradictory because manual latency and retrospective sidecars still say production exposure requires refreshed production sampling/signoff. The fallback decision also does not record the proposal-required control-plane tech lead plus product owner approval.

### REQ-002 Fresh ACP Startup Initializes Before Scans

- Proposal source: ACP execution sequence and disallowed pre-initialize work, rendered lines 98-122.
- Status: **Implemented**.
- Evidence types: code, tests-run, telemetry sidecar.
- Evidence references: `control-plane/crates/acp/src/transport.rs:1030-1209`; `docs/proposals/053.review/manual-latency-spot-check.md`; `./scripts/test-gate.sh proposal-053`.
- Implementation mapping: fresh startup sends `initialize` before session/new and before P053 pre-prompt metadata/discovery; gate fixture observed `acp_pre_initialize_local_latency_ms=0`.
- Gap / note: no P053 gap found.

### REQ-003 Pre-Init Scan Prohibition And Generated-State Denylist

- Proposal source: generated-state exclusion and housekeeping policy, rendered lines 124-150.
- Status: **Implemented**.
- Evidence types: code, tests-run.
- Evidence references: `control-plane/crates/domain/src/discovery.rs`; `./scripts/test-gate.sh proposal-053`.
- Implementation mapping: generated-state denylist and tests cover `.chainworks/worktrees`, `.chainworks/backups`, `.forge-codex-acp`, `.claude/worktrees`, `.git/objects`, target directories, `DerivedData`, `.build`, `node_modules`, DB backups, and SQLite files.
- Gap / note: no P053 gap found.

### REQ-004 ExpectedOutputSpec Schema And Authorized Roots

- Proposal source: Expected Output Specs, rendered lines 152-178.
- Status: **Implemented**.
- Evidence types: code, tests-run.
- Evidence references: `control-plane/crates/domain/src/discovery.rs`; `control-plane/crates/engine/src/contracts.rs`; `./scripts/test-gate.sh proposal-053`.
- Implementation mapping: specs carry output identity, role, target, companion, label, contract, required flag, reuse policy, caps, authorized roots, and source-generation owner; engine derives roots from worktree/meta/workspace/control-plane context.
- Gap / note: no P053 gap found.

### REQ-005 Per-Turn Bounded Pre-Prompt Metadata

- Proposal source: Pre-Prompt Metadata Bounds, rendered lines 180-231.
- Status: **Implemented**.
- Evidence types: code, tests-run.
- Evidence references: `control-plane/crates/domain/src/discovery.rs`; `control-plane/crates/acp/src/transport.rs:1280-1369`; `./scripts/test-gate.sh proposal-053`.
- Implementation mapping: ACP captures bounded metadata after session selection and before prompt, with execution/stage/attempt/session/prompt/discovery identity, spec-count limit, byte budget, and timeout behavior.
- Gap / note: no P053 gap found.

### REQ-006 OutputDiscoveryDecision-Only Settlement Handoff

- Proposal source: OutputDiscoveryDecision and settlement boundary, rendered lines 237-285, 479-494, 831-845.
- Status: **Implemented**.
- Evidence types: code, tests-run.
- Evidence references: `control-plane/crates/domain/src/discovery.rs`; `control-plane/crates/engine/src/contracts.rs`; `control-plane/crates/engine/src/executor.rs:930-1047`, `1061-1095`; `./scripts/test-gate.sh proposal-053`.
- Implementation mapping: accepted decisions carry accepted payload refs and digests; rejected/missing/stale decisions do not expose accepted bytes; persistence gates on accepted decisions.
- Gap / note: no P053 gap found.

### REQ-007 Provider Envelope Caps And Aggregate Caps

- Proposal source: provider envelope and byte caps, rendered lines 287-312.
- Status: **Implemented**.
- Evidence types: code, tests-run.
- Evidence references: `control-plane/crates/acp/src/transport.rs`; `control-plane/crates/engine/src/executor.rs`; `./scripts/test-gate.sh proposal-053`.
- Implementation mapping: ACP caps provider envelopes and `CHAINWORKS_OUTPUT`; engine applies per-output and aggregate caps before accepted decisions.
- Gap / note: production sizing evidence is covered under REQ-001.

### REQ-008 Exact Target Settlement Validation And P057/P058 Truth

- Proposal source: settlement boundary, security exit criterion, acceptance criteria, and implementation contracts, rendered lines 479-528, 679-683, 817-845.
- Status: **Implemented**.
- Evidence types: code, tests-run.
- Evidence references: `control-plane/crates/engine/src/executor.rs`; `control-plane/crates/engine/src/contracts.rs`; `./scripts/test-gate.sh proposal-053`.
- Implementation mapping: exact output settlement rejects symlinks, unauthorized roots, wrong-run paths, oversized payloads, aggregate overages, stale must-produce outputs, and missing files; accepted outputs feed P057/P058 runtime/artifact truth.
- Gap / note: no P053 settlement-truth gap found.

### REQ-009 Bounded Meta-Root Supplemental Discovery

- Proposal source: Bounded Current-Run Meta-Root Discovery, rendered lines 314-334.
- Status: **Implemented**.
- Evidence types: code, tests-run.
- Evidence references: `control-plane/crates/domain/src/discovery.rs`; `control-plane/crates/engine/src/executor.rs`; `./scripts/test-gate.sh proposal-053`.
- Implementation mapping: meta-root discovery rejects symlink roots, canonicalizes roots, reads only regular files under caps, skips generated directories, preserves logs, and remains supplemental-only.
- Gap / note: no P053 gap found.

### REQ-010 Changed-Files Manifest Contract

- Proposal source: Changed-Files Manifest, rendered lines 336-359.
- Status: **Implemented**.
- Evidence types: code, tests-run.
- Evidence references: `control-plane/crates/engine/src/git_manifest.rs`; `control-plane/crates/engine/src/executor.rs`; `./scripts/test-gate.sh proposal-053`.
- Implementation mapping: manifest generation is declared-output gated, post-prompt, before settlement, shell-free, cwd-bound to the worktree, timeout-bound, kill-on-drop, typed-status based, and preserves agent-authored manifests.
- Gap / note: no P053 gap found.

### REQ-011 Legacy Broad Discovery Fallback And Override Controls

- Proposal source: Legacy Broad Discovery, rendered lines 361-390.
- Status: **Implemented**.
- Evidence types: code, schema, migration, tests-run.
- Evidence references: `control-plane/crates/domain/src/discovery.rs`; `control-plane/crates/workflow/src/definition.rs`; `control-plane/crates/db/migrations/026_p053_discovery_diagnostics.sql`; `./scripts/test-gate.sh proposal-053`.
- Implementation mapping: policy defaults disabled, opt-in values are parsed/validated, legacy discovery is post-prompt and capped, and retry-bound overrides are validated/consumed through DB tests.
- Gap / note: no P053 gap found.

### REQ-012 Durable Diagnostics And GraphQL/MCP Readback

- Proposal source: Durable Diagnostics and Phase 2 readback, rendered lines 496-528, 689-699.
- Status: **Implemented**.
- Evidence types: code, migration, tests-run.
- Evidence references: `control-plane/crates/domain/src/discovery.rs`; `control-plane/crates/db/src/repos/agent_execution_discovery_diagnostics.rs`; `control-plane/crates/graphql-server/src/types/stage.rs`; `control-plane/crates/mcp-server/src/tools/reports.rs`; `./scripts/test-gate.sh proposal-053`.
- Implementation mapping: diagnostics persist schema version, legacy usage, missing/rejected/stale counts, meta truncation, manifest status, resume warnings, payload JSON, reconciliation warnings, and GraphQL/MCP projections.
- Gap / note: no P053 gap found.

### REQ-013 Stale Vs Absent Required-Output Truth

- Proposal source: operator behavior and acceptance criteria, rendered lines 538-548, 817-826.
- Status: **Implemented**.
- Evidence types: code, tests-run.
- Evidence references: `control-plane/crates/engine/src/executor.rs:956-959`, `1344-1388`, `5983-5998`; `control-plane/crates/graphql-server/tests/proposal_058_runtime_facts.rs:497-570`; `control-plane/crates/mcp-server/tests/proposal_058_runtime_facts.rs:494-547`; `./scripts/test-gate.sh proposal-053`.
- Implementation mapping: must-produce unchanged exact outputs now settle as missing with reason `StaleExpectedOutput`, and stale output counts project through GraphQL and MCP tests.
- Gap / note: R5's stale-vs-absent gap is closed for the audited code paths.

### REQ-014 Workflow YAML And Swift DSL Compatibility

- Proposal source: Workflow Compiler and implementation contracts, rendered lines 448-477, 844-845.
- Status: **Implemented**.
- Evidence types: code, tests-run.
- Evidence references: `control-plane/crates/workflow/src/definition.rs`; `control-plane/crates/workflow/src/compiler.rs`; `control-plane/crates/workflow/src/plan.rs`; `Chainworks Forge/DSL/WorkflowDefinition.swift`; `./scripts/test-gate.sh proposal-053`.
- Implementation mapping: Rust workflow schema/compiler and Swift DSL mirror include output policies, reuse policy, and legacy broad policy while preserving list-only compatibility and rejecting invalid policy keys/values.
- Gap / note: no P053 gap found.

### REQ-015 DiscoveryFilesystem Injectable Boundary And Fake Gate Coverage

- Proposal source: Rust API Freeze, DiscoveryFilesystem, implementation contracts, rendered lines 421-446 and 839.
- Status: **Partially Implemented**.
- Evidence types: code, tests-run.
- Evidence references: `control-plane/crates/domain/src/discovery.rs:750-896`, `1832-1884`; `control-plane/crates/acp/src/transport.rs:1293-1319`; `control-plane/crates/engine/src/executor.rs:1244-1248`, `1281-1287`, `1360-1364`, `1442-1446`; `./scripts/test-gate.sh proposal-053`.
- Implementation mapping: a `DiscoveryFilesystem` trait, `StdDiscoveryFilesystem`, `FakeDiscoveryFilesystem`, and a fake/recorder gate test now exist.
- Gap / note: the trait is not the full injected filesystem boundary promised by the proposal. It lacks generic exact read/canonicalization methods, ACP/engine paths call concrete/static functions, engine settlement still performs direct `std::fs` reads/canonicalization, and the fake test is not exercised through fresh-session or reused-session runtime paths.

### REQ-016 macOS UI Deferral To P069

- Proposal source: UI Deferral to P069 and non-goals, rendered lines 90-95 and 58-68.
- Status: **Out of Scope**.
- Evidence types: proposal.
- Evidence references: proposal rendered lines 90-95, 708-714, 817-829.
- Implementation mapping: P053 exposes durable readback needed by P069; UI rendering itself is deferred.
- Gap / note: no UI implementation was audited for P053.

### REQ-017 Canonical P053 Gate Evidence

- Proposal source: Phase 1 and Phase 2 exit criteria plus test-gate contract, rendered lines 661-699 and 845.
- Status: **Implemented**.
- Evidence types: tests-run.
- Evidence references: `./scripts/test-gate.sh proposal-053` on HEAD `c9a59787ee75c5363e484656a42e651616da56ab`.
- Implementation mapping: the canonical same-tree P053 control-plane gate passed.
- Gap / note: this is not a full repo regression and does not cover the committed non-P053 `examples/agents` changes.

## Reviewer / Lens Scorecard

| Lens | Result | Top risk | Confidence |
| --- | --- | --- | --- |
| Objective proposal conformance | Partial | Production evidence contradictions and incomplete filesystem injection boundary. | Medium-high |
| Rust architecture | Partial | Trait/fake exists, but not wired as the promised engine/transport filesystem boundary. | High |
| Rust reliability | Pass with caveat | Core session/stale/cap behavior passes; filesystem seam limits deterministic failure injection. | Medium-high |
| API contract | Implemented | Stale/missing/rejected readback now projects through GraphQL/MCP. | High |
| Observability/rollout | Partial | Production exposure evidence is contradictory across required sidecars. | High |
| Chainworks execution truth | Implemented with caveat | Stale output truth is durable; release evidence still ambiguous. | High |
| Readiness | Not Ready | Major rollout/evidence and architecture-boundary findings remain. | High |

## Routed Specialist Findings

### READY-001 Production-Exposure Evidence Contradicts Itself

- Reviewer: `observability_rollout_reviewer`
- Severity: Major
- Confidence: High
- Related requirements: REQ-001
- Evidence types: proposal, rollout sidecar, tests-run
- Evidence references: proposal rendered lines 600-604 and 673-683; `docs/proposals/053.review/cap-validation.json:7-46`, `69-75`, `115-120`; `docs/proposals/053.review/security-checklist.md:8`, `26`, `46`; `docs/proposals/053.review/manual-latency-spot-check.md:73`; `docs/proposals/053.review/phase-1-retrospective.md:27`; `./scripts/test-gate.sh proposal-053`.
- Why it matters: P053's production exposure decision is a core release gate. The cap-validation and security artifacts now approve production exposure from a replacement sample, but two other required Phase 1 evidence files still say production exposure requires refreshed production sampling/signoff. That leaves operators and reviewers with contradictory durable truth.
- Recommended action: update the sidecar set to one coherent production-exposure decision, or switch `cap-validation.json` back to `gate_only_internal`.
- Acceptance criteria: cap-validation, security checklist, manual latency evidence, operator clarity evidence, and Phase 1 retrospective all state the same exposure mode, replacement-sample rationale, remaining risk, telemetry follow-up, and approvers; the fallback approval records the proposal-required control-plane tech lead plus product owner decision or an explicit approved substitute.

### ARCH-001 DiscoveryFilesystem Is Present But Not The Injected Boundary Promised By P053

- Reviewer: `rust_arch_reviewer`
- Severity: Major
- Confidence: High
- Related requirements: REQ-015
- Evidence types: proposal, code, tests-run
- Evidence references: proposal rendered lines 435-446 and 839; `control-plane/crates/domain/src/discovery.rs:750-896`, `1832-1884`; `control-plane/crates/engine/src/executor.rs:1244-1248`, `1281-1287`, `1360-1364`, `1442-1446`; `control-plane/crates/acp/src/transport.rs:1293-1319`; `./scripts/test-gate.sh proposal-053`.
- Why it matters: P053 explicitly made `DiscoveryFilesystem` the injectable boundary for traversal, metadata lookup, canonicalization, exact reads, operation recording, and cancellation-aware file work, and said the gate should use the fake for fresh-session and reused-session paths. The implementation added a trait and fake, but key transport/engine paths still call concrete/static filesystem functions or `std::fs` directly, so the fake does not prove those runtime paths.
- Recommended action: thread a `&dyn DiscoveryFilesystem` or equivalent adapter into ACP/engine discovery paths, expand the trait to cover the promised read/canonicalization operations, and make at least one fresh-session and reused-session P053 gate path run through the fake.
- Acceptance criteria: exact-path acceptance, declared reuse, stale detection, bounded meta-root discovery, and pre-prompt metadata can be exercised through `FakeDiscoveryFilesystem`; the gate fails if those paths bypass the trait or perform unrecorded workspace/worktree traversal before the allowed phase.

### READY-002 Current HEAD Includes Non-P053 Catalog Changes Outside The Passing Gate

- Reviewer: `observability_rollout_reviewer`
- Severity: Minor
- Confidence: Medium
- Related requirements: readiness only
- Evidence types: git history, diff, tests-run
- Evidence references: `git show --stat --name-status HEAD`; `examples/agents/agents.yaml`; `examples/agents/agents_mcp_profiles_v2.yaml`; `./scripts/test-gate.sh proposal-053`.
- Why it matters: the P053 gate passed on the current HEAD, but that HEAD also contains an agent catalog model update, deletion of the tracked `agents_mcp_profiles_v2.yaml`, `.codex/config.toml` change, and a P017 audit report. Those are outside P053's proof surface and should not be silently treated as P053 validation.
- Recommended action: keep P053 readiness claims scoped to P053, and validate the catalog/config/P017 changes under their own proposal or release gate.
- Acceptance criteria: release notes or branch handoff explicitly separate P053 from the catalog/config/P017 changes, or additional validation evidence covers those non-P053 changes.

## Readiness Checklist

| Check | Status | Evidence / note |
| --- | --- | --- |
| Canonical build/gate | Passed | `./scripts/test-gate.sh proposal-053` passed on audited HEAD `c9a59787ee75c5363e484656a42e651616da56ab`. |
| Full regression suite | Not run | The canonical proposal gate was run; no full-repo success is claimed. |
| Core service flow validation | Passed with caveat | Rust focused unit/integration gate covers ACP startup, metadata, caps, stale, DB, GraphQL, MCP, workflow policy, and legacy override paths. |
| Startup latency evidence | Passed | ACP fixture observed `acp_pre_initialize_local_latency_ms=0`; manual reference workspace sidecar also reports 0 ms. |
| Stale-vs-absent readback | Passed | Engine, GraphQL, and MCP stale-count tests are in the passing gate. |
| Security/path validation | Passed with evidence caveat | Security checklist approves production exposure, but adjacent sidecars disagree on production sampling/signoff. |
| DiscoveryFilesystem fake/injection | Partial | Trait and fake exist; runtime paths are not fully injected through it. |
| API/schema contract validation | Passed | Workflow/Swift mirrors and diagnostics readback are covered by gate/code evidence. |
| UI empty/loading/error/offline/permissions | Out of scope | P053 UI deferred to P069. |
| Accessibility/localization/entitlements | Out of scope | No P053 UI or entitlement change audited. |
| Production exposure readiness | Not ready | Evidence sidecars contradict each other and fallback approval is not recorded with the proposal-required decision owners. |
| Branch scope risk | Caveat | Current HEAD includes non-P053 catalog/config/P017 changes outside the P053 gate. |

## Verification Log

| Command / check | Result |
| --- | --- |
| `pwd` | `/Users/user/Documents/Chainworks Forge` |
| `git rev-parse --show-toplevel && git rev-parse HEAD && git branch --show-current` | Repo root resolved; branch `main`; current final HEAD `c9a59787ee75c5363e484656a42e651616da56ab`. |
| `git status --short` | Final status before completion: only this new R6 audit report is untracked. Earlier in the audit, before HEAD advanced, the agent catalog/config/P017 changes were local; they are now committed in HEAD `c9a59787`. |
| `python3 .../report_path.py .../053-bounded-acp-artifact-discovery-and-startup-latency.md` | Report path resolved to R6. |
| `python3 .../discover_prior_review.py .../053-bounded-acp-artifact-discovery-and-startup-latency.md` | No prior proposal-review artifacts found. |
| `jq -r '.document_markdown' ...` | Proposal contract extracted from rendered Markdown. |
| `git show --stat HEAD` | Confirmed HEAD is `chore: update agent catalog and p017 audit`; parent `14ad5e55` is the P053 R5 closeout commit. |
| Focused `rg`, `sed`, and `nl` inspections | Verified stale output fix, trait/fake addition, direct filesystem calls, sidecar production-exposure contradictions, GraphQL/MCP stale tests, and gate registration. |
| `./scripts/test-gate.sh proposal-053` | Passed on final HEAD `c9a59787`. Output included `proposal_053_gate_uses_discovery_filesystem_trait_fake`, stale output settlement/readback tests, ACP fixture `observed acp_pre_initialize_local_latency_ms=0`, and final `Proposal 053 control-plane gate passed`. |

Tests found:

- Domain discovery tests for generated-state denylist, operation recorder ordering, trait fake, metadata bounds, meta-root caps, legacy broad caps, and serialization.
- ACP tests for provider output caps, legacy broad default-disabled behavior, and startup latency fixture.
- Engine tests for expected-output specs, stale must-produce behavior, meta-root supplemental paths, changed-files manifest, legacy override validation, and artifact settlement.
- Workflow integration tests for output policies and legacy broad policy compatibility.
- DB diagnostics tests for roundtrip/readback and legacy override behavior.
- GraphQL/MCP tests for reconciliation pending and stale output count readback.

Tests run:

- `./scripts/test-gate.sh proposal-053`.

Runtime evidence:

- No live daemon/operator runtime was started.
- ACP adapter fixture in the P053 gate executed and observed `acp_pre_initialize_local_latency_ms=0`.

## Final Verdict

Overall conformance: **Partial**.

Overall implementation readiness: **Not Ready**.

The R5 stale-output API gap is closed, and the same-tree P053 control-plane gate passes. However, P053 still should not be treated as production-ready because the production-exposure evidence files disagree with each other and the fallback approval path does not record the proposal-required decision owners. The `DiscoveryFilesystem` closeout is also incomplete: the trait/fake exists, but it is not yet the injected boundary used by the engine and transport discovery paths described by the proposal.

Recommended next actions:

1. Make the P053 evidence sidecars agree on one production-exposure decision and record the required fallback approvers.
2. Thread `DiscoveryFilesystem` through ACP/engine discovery paths and extend it to cover exact reads and canonicalization, then make fresh/reused gate paths exercise the fake.
3. Keep branch handoff/release notes explicit that current HEAD also includes non-P053 catalog/config/P017 changes, or validate those changes under their own gate.
