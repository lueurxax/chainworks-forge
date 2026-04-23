# Proposal 053: Bounded ACP Artifact Discovery and Startup Latency Multi-Lens Audit R1

| Field | Value |
|---|---|
| Proposal | `docs/proposals/053-bounded-acp-artifact-discovery-and-startup-latency.md` |
| Repository Root | `.` |
| Worktree | `.chainworks/worktrees/codex-p053-manual-merge-1833dd16` |
| Branch | `codex/p053-manual-merge-1833dd16` |
| Git SHA | `190bc8f186bb788cc2efe884d9cff0f271adde15` |
| Working Tree | clean before audit report creation |
| Audited At | `2026-04-23T13:29:34+03:00` |
| Platform Scope | macOS operator app plus Rust control-plane |
| Proposal State | Active |
| Overall Conformance | Partial |
| Overall Readiness | Not Ready |
| Audit Confidence | High for source conformance, Medium for runtime/readiness because no tests were run |

## Executive Verdict

P053 is substantially implemented on the Rust control-plane path, especially Phase 1 and much of Phase 2: fresh ACP startup sends `initialize` before discovery work, typed expected-output specs exist, per-turn metadata is captured, output settlement is decision-based, diagnostics persistence/readback exists, changed-file manifests exist, and the `proposal-053` gate is registered. It is not ready for proposal closeout because required Phase 0/Phase 1 evidence artifacts are missing, the promised `DiscoveryFilesystem` injectable trait and operation-recorder boundary is not implemented, the required metrics/observability fields are not emitted as named, Phase 3 operator UI is not implemented, and this audit did not run the canonical gate.

## Lens Scorecard

| Lens | Assessment | Top Risk | Confidence |
|---|---|---|---|
| Conformance | Partial | Evidence artifacts, metrics, trait boundary, and UI phase are incomplete | High |
| Architecture | At Risk | `DiscoveryFilesystem` is concrete static logic without the promised injectable operation recorder | High |
| Product | At Risk | Core control-plane behavior exists, but production exposure evidence is absent | Medium |
| UI | Weak | P053-specific operator UI is missing and `ArtifactInspectorView` is still a placeholder | High |
| UX | At Risk | Diagnostics are queryable, but the promised user-facing recovery/readability states are not implemented in macOS UI | Medium |
| Readiness | Not Ready | Canonical gate not run by this audit; required Phase 0/1 evidence files absent | High |

## Proposal Contract

Note: the proposal file is a JSON object. The reviewable proposal body is stored in `document_markdown` at physical `docs/proposals/053-bounded-acp-artifact-discovery-and-startup-latency.md:9`. Logical line references below refer to the rendered `document_markdown` content.

### Scope

- Rust control-plane ACP startup and artifact discovery.
- Engine-owned discovery settlement for declared outputs.
- Bounded pre-prompt exact-path metadata for fresh and reused ACP sessions.
- Bounded current-run meta-root supplemental discovery.
- Generated-state denylist for fallback/support traversals.
- Legacy broad discovery disabled by default, with opt-in/override controls.
- Changed-files manifest generation.
- Durable diagnostics, GraphQL/MCP/report readback, restart recovery, and operator support workflows.
- macOS operator UI surfaces for missing/rejected/stale output diagnostics and startup performance.

### Locked Decisions

- ACP transport remains stdio.
- ACP transport must not decide required-output truth.
- Required-output truth remains in P057/P058 settlement.
- Broad discovery is disabled by default.
- `ExpectedOutputSpec`, `PrePromptExpectedOutputMetadata`, and `OutputDiscoveryDecision` are the typed P053 contracts.
- Accepted discovery decisions, not raw target-path reads, feed validation and persistence.
- Provider envelopes and `CHAINWORKS_OUTPUT` payloads use the same cap model as exact-path outputs.
- Phase 1 production exposure requires minimal durable discovery-decision readback.
- Phase 3 UI may not start without UI design sign-off on the specified P053 visual behavior.

### Primary User Flows

1. Operator starts or retries an ACP-backed run in a large repository and sees provider startup begin promptly instead of waiting on pre-initialize workspace discovery.
2. Agent execution produces declared outputs through provider envelope, `CHAINWORKS_OUTPUT`, exact-path write, or control-plane manifest generation, and the engine settles those outputs through accepted discovery decisions.
3. Required output is missing, stale, oversized, unauthorized, escaped, metadata-timed-out, or over aggregate cap, and the run records `missing_required_outputs` with durable diagnostics.
4. Operator inspects failure/readback surfaces and can tell what output failed, why, where it was expected, whether stale/absent/capped/unauthorized/legacy fallback was involved, and what timing belonged to Forge versus provider startup.
5. Maintainer enables or audits legacy broad discovery only through frozen workflow policy or retry-bound override, with caps, generated-state exclusions, and sunset telemetry.

### UI Commitments

- Missing artifacts, discovery mode, startup performance, cap warnings, source changes, Copy Path, Open Location, accessibility labels, Dynamic Type behavior, and friendly Source Changes failures render without sidebar overflow.
- P053 diagnostics map into `RunDetailPanel`, `FailedStageEvidencePanel`, Stage Detail, Run Report, and `ArtifactInspectorView`.
- `DesignTokens.Charts.forgeOverhead` and `DesignTokens.Charts.providerLatency` or approved equivalents exist with contrast verification.
- Startup Performance segmented bar appears in `RunDetailPanel.headerBlock` below `StatusCapsule`.
- Pre-flight metadata warnings render as a persistent Banner pinned to the top of `RunDetailPanel`.
- Stale, missing, and rejected states have distinct icons and labels.

### UX Commitments

- Operators can distinguish Forge overhead from provider latency without prior explanation.
- Stale outputs are visually distinct from never-produced outputs.
- Unauthorized-root tooltips include the task-specific authorized roots and rejected canonical path when available.
- Compact rows handle narrow sidebars, truncation, disclosure/scroll, Show Issues Only filtering, and Dynamic Type expansion.
- Source Changes failures distinguish timeout, not-git-repository, command-failed, and empty states.

### Acceptance Criteria

- Fresh ACP startup sends `initialize` without recursive repository, workspace-root, worktree-root, or generated-state scanning.
- `proposal-053|p053` gate proves no traversal under `workspace_root` or effective `worktree_root` before `initialize`.
- Provider handshake timing is reported separately from Forge local overhead.
- Missing, stale, escaped, unauthorized, oversized, over aggregate cap, metadata-timed-out, or contract-invalid required outputs settle as `missing_required_outputs`.
- Stale pre-existing required outputs do not pass unless accepted through provider/control-plane current-invocation provenance or explicit `allow_unchanged_existing`.
- Bounded pre-prompt metadata respects spec-count, byte-budget, and timeout limits for fresh and reused ACP sessions.
- Bounded supplemental discovery scans only the current run meta-root.
- Legacy broad discovery is disabled by default and can run only post-prompt with audited, capped, temporary policy.
- Changed-files manifests are generated after prompt completion and before exact-path acceptance when declared.
- Operators can see missing/rejected output reason and location details.
- Phase 1 production exposure requires minimal durable discovery-decision readback.
- P053 gate includes generated-state denylist, reused-session metadata, oversized-envelope, workflow policy compatibility, and raw target-path bypass fixtures.
- Phase 3 UI renders the specified P053 diagnostic and source-change states without sidebar overflow.

### Test / Evidence Requirements

- `docs/proposals/053.review/cap-validation.json`.
- Optional run-local mirror `.chainworks/runs/<run_id>/proposals/current/cap-validation.json`.
- Phase 0 evidence fields including dependency readiness, sample coverage, p50/p90/p99 sizes, chosen caps, phase exposure mode, reviewer signoff, and generated timestamp.
- Lightweight `proposal-053-phase0` or equivalent check for the Phase 0 evidence artifact.
- Phase 1 manual latency spot-check on the reference large workspace.
- Phase 1 qualitative operator-clarity check.
- Phase 1 retrospective decision before Phase 2/3 investment.
- Phase 1 security checklist or security risk-acceptance artifact.
- `./scripts/test-gate.sh proposal-053`.

### Explicit Exclusions

- No ACP transport switch from stdio to HTTP.
- No P051 behavior changes.
- No tracked `.chainworks`.
- No historical artifact byte migration.
- No broad legacy discovery by default.
- No local UI smoke tests as the primary proof path for Rust control-plane change.
- No second output-validation system parallel to P057/P058.
- No transport-owned required-output truth.
- No contract-specific artifact size maxima in P053.
- No manual reconstruction of implementation work from the prior cancelled P053 run.

## Proposal Fidelity / Divergence

### Matches

- ACP transport sends `initialize` and `session/new` before per-prompt metadata capture or discovery work in fresh sessions.
- `ExpectedOutputSpec`, `PrePromptExpectedOutputMetadata`, `OutputDiscoveryDecision`, and `DiscoveryDiagnosticsV1` exist under `domain::discovery`.
- Provider envelope and `CHAINWORKS_OUTPUT` payloads are bounded before settlement.
- Engine builds typed expected-output specs and settles discovered artifacts through `OutputDiscoveryDecision`.
- Accepted decisions feed `CapturedOutput`; rejected decisions do not expose payloads to validation.
- Bounded meta-root discovery and generated-state denylist exist.
- `changed_files_manifest` generation is shell-free, timeout-aware, and preserves agent-authored manifests.
- `agent_execution_discovery_diagnostics` persistence and GraphQL/MCP readback exist.
- `proposal-053|p053` is registered in `scripts/test-gate.sh` and documented in `docs/reference/test-gates.md`.

### Divergences

- Required Phase 0 cap-validation and dependency-readiness evidence artifact is absent.
- Phase 1 security checklist or risk-acceptance artifact is absent.
- `DiscoveryFilesystem` is a concrete static helper, not the promised injectable trait with operation-recorder types.
- Required structured metric names are not implemented as searchable source fields.
- P053-specific macOS UI surfaces are absent or placeholders.
- The registered gate does not include a Phase 0 evidence-artifact lane.

### Ambiguities / Evidence Gaps

- This audit did not execute `./scripts/test-gate.sh proposal-053`; tests are recorded as found, not run.
- The proposal's Phase 1 exposure mode cannot be established because `cap-validation.json` is absent.
- The proposal is active but explicitly phased; this audit treats the full proposal contract as the target because the user asked for P053 implementation audit, not Phase 1-only audit.

## Requirement Summary

| Status | Count |
|---|---:|
| Implemented | 11 |
| Partially Implemented | 4 |
| Missing | 5 |
| Not Verifiable | 1 |

## Requirement Audit

### REQ-001 Phase 0 dependency and cap-validation evidence

- Proposal Source: Dependencies and Readiness; Rollout Phase 0 logical lines 67-85 and 579-644.
- Status: Missing.
- Evidence Type: code, tests-found.
- Evidence:
  - `docs/proposals/053-bounded-acp-artifact-discovery-and-startup-latency.md:37-45` lists Phase 1 blockers including Phase 0 dependency readiness, cap validation, exposure decision, semantics freeze, interface freeze, and security review plan.
  - `find docs -path '*053.review*' -maxdepth 5 -type f -print` returned no files.
  - `rg -n "proposal-053-phase0|cap-validation" scripts docs/reference control-plane` returned no P053 Phase 0 gate or cap-validation implementation beyond proposal text.
- Gap / Note: `docs/proposals/053.review/cap-validation.json` and an equivalent Phase 0 evidence gate are absent.

### REQ-002 Fresh ACP startup sends initialize before repository/workspace/worktree traversal

- Proposal Source: ACP Execution Sequence; Disallowed Before Initialize; Behavioral Acceptance Criteria logical lines 89-113 and 815-817.
- Status: Implemented.
- Evidence Type: code, tests-found.
- Evidence:
  - `control-plane/crates/acp/src/transport.rs:1061-1082` sends `initialize` and awaits the response.
  - `control-plane/crates/acp/src/transport.rs:1084-1103` sends `session/new` only after `initialize`.
  - `control-plane/crates/acp/src/transport.rs:1219-1267` captures pre-prompt metadata only in `prompt`, after session startup.
  - `scripts/test-gate.sh:2391-2416` registers a P053 gate with discovery-focused tests.
- Gap / Note: The audit did not run the gate, so implementation is proven by source inspection and tests-found only.

### REQ-003 Generated-state denylist and bounded fallback traversal

- Proposal Source: Generated-State Exclusion and Housekeeping Policy logical lines 115-141.
- Status: Partially Implemented.
- Evidence Type: code, tests-found.
- Evidence:
  - `control-plane/crates/domain/src/discovery.rs:531-563` implements generated-state directory/file skip checks.
  - `control-plane/crates/domain/src/discovery.rs:884-995` bounds legacy broad discovery by timeout, file count, file size, total bytes, symlink skip, and generated-state skip.
  - `control-plane/crates/domain/src/discovery.rs:1119-1165` has tests for the denylist roots and DB backup files.
- Gap / Note: Discovery correctness is implemented. The housekeeping cleanup command requirements, including safe cleanup categories and bytes-reclaimed reporting, are not implemented in P053 and are documented as long-term P061/follow-up ownership.

### REQ-004 Typed `ExpectedOutputSpec` contract

- Proposal Source: Expected Output Specs; Implementation Contracts logical lines 143-169 and 831.
- Status: Implemented.
- Evidence Type: code, tests-found.
- Evidence:
  - `control-plane/crates/domain/src/discovery.rs:121-136` defines `ExpectedOutputSpec` with output role, target path, display label, contract id, required, reuse policy, caps, authorized roots, and source generation owner.
  - `control-plane/crates/acp/src/lib.rs:52-61` keeps `expected_output_paths` as compatibility projection and adds typed `expected_outputs`.
  - `control-plane/crates/engine/src/contracts.rs:371-457` builds specs from declared outputs and policies.
  - `control-plane/crates/domain/src/discovery.rs:1260-1287` tests serialization of P053 policy fields.
- Gap / Note: None for the typed contract.

### REQ-005 Bounded per-prompt metadata for fresh and reused ACP sessions

- Proposal Source: Pre-Prompt Metadata Bounds; Implementation Contracts logical lines 171-222 and 832.
- Status: Implemented.
- Evidence Type: code, tests-found.
- Evidence:
  - `control-plane/crates/domain/src/discovery.rs:161-182` defines metadata identity including agent execution, stage execution, attempt, session generation, prompt turn, and discovery generation.
  - `control-plane/crates/domain/src/discovery.rs:194-209` defines default metadata bounds.
  - `control-plane/crates/domain/src/discovery.rs:686-722` enforces spec-count, digest budget, and timeout.
  - `control-plane/crates/acp/src/transport.rs:1244-1267` creates a new metadata context and captures bounded metadata before every `session/prompt`.
  - `control-plane/crates/domain/src/discovery.rs:1344-1519` tests spec-count, aggregate budget, and timeout behavior.
- Gap / Note: Source supports reused sessions because `prompt` is called per turn; runtime reuse behavior was not executed by this audit.

### REQ-006 Engine-owned `OutputDiscoveryDecision` handoff and accepted-decision validation

- Proposal Source: Output Discovery Decisions; Architecture; Implementation Contracts logical lines 224-275, 398-410, and 833-836.
- Status: Implemented.
- Evidence Type: code, tests-found.
- Evidence:
  - `control-plane/crates/domain/src/discovery.rs:258-291` defines `OutputDiscoveryDecision`.
  - `control-plane/crates/engine/src/executor.rs:903-1039` builds decision settlements and payload refs.
  - `control-plane/crates/engine/src/executor.rs:1052-1088` implements `settle_agent_outputs_from_discovery_decisions`.
  - `control-plane/crates/engine/src/contracts.rs:517-567` builds `CapturedOutput` only from accepted decisions and payload refs.
  - `control-plane/crates/engine/src/executor.rs:4002-4073` persists declared artifacts only when an accepted discovery decision exists on the P053 path.
  - `control-plane/crates/engine/src/contracts.rs:729-775` tests that rejected target paths are not re-read into validation.
- Gap / Note: `declared_output_has_accepted_discovery_decision(None, ...)` intentionally returns true for legacy/no-decision callers at `control-plane/crates/engine/src/executor.rs:1183-1195`; the P053 path passes decisions. Keep that compatibility branch constrained so it cannot become a P053 bypass.

### REQ-007 Provider envelope and `CHAINWORKS_OUTPUT` cap parity

- Proposal Source: Provider Envelope and CHAINWORKS_OUTPUT Caps; Byte Caps; Behavioral Acceptance Criteria logical lines 277-303 and 818-820.
- Status: Implemented.
- Evidence Type: code, tests-found.
- Evidence:
  - `control-plane/crates/acp/src/transport.rs:622-649` truncates provider/`CHAINWORKS_OUTPUT` payloads to `max_bytes + 1` for bounded oversize signaling.
  - `control-plane/crates/acp/src/transport.rs:651-659` caps NDJSON line parsing.
  - `control-plane/crates/engine/src/executor.rs:978-990` rejects payloads over per-output cap.
  - `control-plane/crates/engine/src/executor.rs:992-1005` rejects aggregate cap overflow.
  - `control-plane/crates/engine/src/executor.rs:5410-5499` tests oversized provider and `CHAINWORKS_OUTPUT` rejections.
  - `control-plane/crates/engine/src/executor.rs:5501-5565` tests aggregate cap rejection.
- Gap / Note: Production cap validation evidence is missing under REQ-001.

### REQ-008 Bounded current-run meta-root supplemental discovery

- Proposal Source: Bounded Meta-Root Discovery logical lines 305-325.
- Status: Implemented.
- Evidence Type: code, tests-found.
- Evidence:
  - `control-plane/crates/domain/src/discovery.rs:786-882` implements bounded meta-root discovery.
  - `control-plane/crates/engine/src/executor.rs:1407-1425` invokes bounded meta-root discovery only from `chainworks_meta_root`.
  - `control-plane/crates/domain/src/discovery.rs:1207-1257` tests small regular files and symlink exclusion.
  - `control-plane/crates/engine/src/executor.rs:5124-5136` tests supplemental-only meta-root paths.
- Gap / Note: None for control-plane behavior.

### REQ-009 Changed-files manifest generation

- Proposal Source: Changed-Files Manifest logical lines 327-350 and 823.
- Status: Implemented.
- Evidence Type: code, tests-found.
- Evidence:
  - `control-plane/crates/engine/src/git_manifest.rs:41-79` generates the manifest when declared and worktree-backed.
  - `control-plane/crates/engine/src/git_manifest.rs:81-110` defines `GitManifestRunner` with a worktree root and timeout.
  - `control-plane/crates/engine/src/git_manifest.rs:331-385` parses porcelain status into staged, unstaged, deleted, renamed, conflicted, and untracked groups.
  - `control-plane/crates/engine/src/git_manifest.rs:409-423` preserves an agent-authored manifest as `changed_files_manifest.agent.json`.
  - `control-plane/crates/engine/src/git_manifest.rs:446-523` tests manifest generation, preservation, and not-git-repository status.
- Gap / Note: Runtime timing metric for git manifest latency is not implemented under REQ-016.

### REQ-010 Legacy broad discovery disabled by default and opt-in/override bounded

- Proposal Source: Legacy Broad Discovery logical lines 352-381 and 822.
- Status: Implemented.
- Evidence Type: code, tests-found.
- Evidence:
  - `control-plane/crates/workflow/src/definition.rs:40-51` models `legacy_broad_discovery_policy`.
  - `control-plane/crates/workflow/src/compiler.rs:45-57` defaults the compiled policy when absent.
  - `control-plane/crates/workflow/tests/integration.rs:833-887` tests default disabled, workflow opt-in, and unknown-value rejection.
  - `control-plane/crates/db/migrations/026_p053_discovery_diagnostics.sql:28-52` defines retry-bound `legacy_discovery_overrides`.
  - `control-plane/crates/acp/src/transport.rs:1268-1270` checks whether legacy broad discovery is enabled before prompt-time broad discovery.
  - `control-plane/crates/acp/src/transport.rs:1414-1460` runs legacy broad discovery only after the prompt response.
- Gap / Note: Sunset telemetry is incomplete under REQ-016.

### REQ-011 Durable discovery diagnostics and readback

- Proposal Source: Goals; Phase 2; Implementation Contracts logical lines 49, 500-519, 678-688, and 839-840.
- Status: Implemented.
- Evidence Type: code, tests-found.
- Evidence:
  - `control-plane/crates/db/migrations/026_p053_discovery_diagnostics.sql:1-27` creates `agent_execution_discovery_diagnostics`.
  - `control-plane/crates/db/src/repos/agent_execution_discovery_diagnostics.rs:39-89` upserts diagnostics.
  - `control-plane/crates/db/src/repos/agent_execution_discovery_diagnostics.rs:110-152` provides readback by execution/run.
  - `control-plane/crates/db/src/repos/agent_execution_discovery_diagnostics.rs:155-235` marks reconciliation pending when runtime facts or active artifact generation truth is missing.
  - `control-plane/crates/graphql-server/src/types/stage.rs:255-285` exposes `discoveryDiagnostics`.
  - `control-plane/crates/mcp-server/src/tools/reports.rs:165-238` includes discovery diagnostics in MCP execution truth.
- Gap / Note: The macOS UI consumption of this data is incomplete under REQ-020.

### REQ-012 P037 watchdog boundary for post-prompt discovery

- Proposal Source: P037 Watchdog Boundary logical lines 521-523.
- Status: Not Verifiable.
- Evidence Type: code, inference.
- Evidence:
  - `control-plane/crates/acp/src/transport.rs:1293-1316` applies prompt stream idle timeout while reading ACP messages.
  - `control-plane/crates/acp/src/transport.rs:1414-1550` performs post-prompt discovery after prompt terminal response.
- Gap / Note: Source shape implies post-prompt discovery is outside ACP prompt idle reads, but this audit did not inspect P037 supervision end-to-end or run a watchdog fixture.

### REQ-013 `DiscoveryFilesystem` trait and operation recorder

- Proposal Source: Rust API Freeze; DiscoveryFilesystem; Implementation Contracts logical lines 412-430 and 837.
- Status: Partially Implemented.
- Evidence Type: code, tests-found.
- Evidence:
  - `control-plane/crates/domain/src/discovery.rs:447-568` implements `DiscoveryFilesystem` as a concrete static helper type.
  - `rg -n "OperationRecorder|operation recorder|DiscoveryOperation|trait DiscoveryFilesystem|pub trait"` found no P053 operation recorder or `DiscoveryFilesystem` trait implementation.
  - `scripts/test-gate.sh:2397-2414` runs behavior tests but not an operation-recorder assertion for traversal ordering.
- Gap / Note: The implementation places discovery value types under `domain::discovery`, but it does not implement the promised injectable trait/operation-recorder boundary. That weakens the deterministic proof that no pre-initialize traversal hook executed.

### REQ-014 Rust and Swift workflow schema mirrors

- Proposal Source: Implementation Contracts; Phase 2 logical lines 680 and 841.
- Status: Implemented.
- Evidence Type: code, tests-found.
- Evidence:
  - `control-plane/crates/workflow/src/definition.rs:40-51` models `legacy_broad_discovery_policy`.
  - `control-plane/crates/workflow/src/compiler.rs:45-57` compiles the policy into the run plan.
  - `control-plane/crates/workflow/tests/integration.rs:718-887` tests output policy and legacy discovery policy behavior.
  - `Chainworks Forge/DSL/WorkflowDefinition.swift:43-54` models `discovery.legacy_broad_discovery_policy`.
  - `Chainworks Forge/DSL/WorkflowDefinition.swift:119-157` models `output_policies.reuse_policy`.
  - `Chainworks Forge/DSL/YAMLValidator.swift:463-469` validates output policy keys against declared task outputs.
- Gap / Note: This audit did not find a same-tree Swift/Rust snapshot-hash fixture execution; tests were not run.

### REQ-015 Canonical `proposal-053|p053` gate registration and coverage

- Proposal Source: Behavioral Acceptance Criteria; Implementation Contracts logical lines 816, 826, and 843.
- Status: Partially Implemented.
- Evidence Type: code, tests-found.
- Evidence:
  - `scripts/test-gate.sh:2391-2416` registers `proposal-053|p053` and runs focused domain, DB, workflow, ACP, engine, GraphQL, and MCP tests.
  - `docs/reference/test-gates.md:911-946` documents the gate and accepted alias.
- Gap / Note: The gate was not run by this audit. The gate also does not include the Phase 0 evidence-artifact check required by logical lines 601-602.

### REQ-016 Metrics and observability fields

- Proposal Source: Metrics and Observability logical lines 724-775.
- Status: Missing.
- Evidence Type: code.
- Evidence:
  - `rg -n "acp_pre_initialize_local_latency_ms|acp_pre_prompt_metadata_latency_ms|acp_expected_output_spec_count|acp_control_plane_manifest_latency_ms|acp_exact_output_acceptance_latency_ms|acp_meta_root_discovery_latency_ms|acp_git_changed_files_latency_ms|acp_cap_validation|acp_reconciliation_pending"` across `control-plane`, `Chainworks Forge`, `docs/reference`, and `scripts` returned no implementation hits.
- Gap / Note: Some adjacent fields exist, such as MCP session startup latency and diagnostics counts, but the required structured metric names and production confirmation fields are absent.

### REQ-017 Phase 1 manual latency, operator clarity, retrospective, and production exposure evidence

- Proposal Source: Phase 1 exit criteria logical lines 650-666 and Open Question OQ-06 logical line 811.
- Status: Missing.
- Evidence Type: code.
- Evidence:
  - `find docs -path '*053.review*' -maxdepth 5 -type f -print` returned no Phase 1 exit evidence files.
  - `rg -n "phase_1_exposure_mode|operator-clarity|manual latency|retrospective|acp_pre_initialize_local_latency_ms" docs control-plane "Chainworks Forge" scripts` found only proposal text or unrelated references.
- Gap / Note: Without `phase_1_exposure_mode` and exit evidence, the implementation cannot be treated as production-shippable under the proposal even though much of the Phase 1 code exists.

### REQ-018 Phase 1 security review artifact

- Proposal Source: Security review exit criterion logical lines 668-672.
- Status: Missing.
- Evidence Type: code.
- Evidence:
  - `find docs/proposals -maxdepth 3 -type f -iname '*p053*security*' -o -iname '*053*security*'` returned no security checklist or risk-acceptance artifact.
  - `rg -n "Security Reviewer|security review|security-checklist|security-risk-acceptance" docs control-plane "Chainworks Forge" scripts` found no P053 security artifact implementation.
- Gap / Note: The branch has path-boundary code and tests-found, but the proposal explicitly requires durable reviewer signoff or risk acceptance before Phase 1 PR landing.

### REQ-019 Phase 2 legacy override and diagnostics readback

- Proposal Source: Legacy Broad Discovery; Phase 2 exit criteria logical lines 367-376 and 678-688.
- Status: Partially Implemented.
- Evidence Type: code, tests-found.
- Evidence:
  - `control-plane/crates/db/migrations/026_p053_discovery_diagnostics.sql:28-52` defines `legacy_discovery_overrides`.
  - `control-plane/crates/graphql-server/src/schema.rs:74-90` parses legacy broad discovery policy for GraphQL.
  - `control-plane/crates/mcp-server/src/tools/stages.rs:87-171` parses legacy discovery override policy for MCP stages tooling.
  - `control-plane/crates/db/tests/proposal_053_discovery_diagnostics.rs:306-420` tests override binding and rejection cases.
- Gap / Note: Sunset telemetry named by the proposal is not implemented under REQ-016, and this audit did not fully inspect every stale/wrong-attempt/expired override branch.

### REQ-020 Phase 3 operator UI surfaces

- Proposal Source: UX and UI Notes; Phase 3 exit criteria; Behavioral Acceptance Criteria logical lines 525-575, 697-712, and 827.
- Status: Missing.
- Evidence Type: code.
- Evidence:
  - `Chainworks Forge/Views/ArtifactInspectorView.swift:3-6` is a `ControlPlaneOnlyPlaceholder`.
  - `Chainworks Forge/Views/FailedStageEvidencePanel.swift:12-77` renders generic failure evidence, not P053 discovery mode, cap warnings, startup performance, source changes, Copy Path, Open Location, or issue filtering.
  - `rg -n "Discovery Mode|Startup Performance|forgeOverhead|providerLatency|Show Issues Only|Metadata Timeout|Copy Path|Open Location|Source Changes" "Chainworks Forge" -g'*.swift'` found no P053-specific UI implementation.
- Gap / Note: Control-plane readback exists, but the promised macOS operator surfaces do not.

### REQ-021 Full regression / canonical gate evidence on audited tree

- Proposal Source: Output Contract; Verification Strategy in audit skill.
- Status: Missing.
- Evidence Type: tests-found.
- Evidence:
  - `./scripts/test-gate.sh proposal-053` is registered at `scripts/test-gate.sh:2391-2416`.
  - This audit did not run validation commands.
- Gap / Note: Because no same-tree gate or full regression was run by this audit, the report cannot claim `Overall Conformance = Implemented`, `Overall Readiness = Ready`, or `Ready with Risks`.

## Architecture Review

**Summary:** At Risk.

### ARCH-001 DiscoveryFilesystem lacks the promised trait and operation recorder

- Severity: Major.
- Confidence: High.
- Related Proposal Items / Requirements: REQ-013.
- Evidence Type: code.
- Evidence:
  - `control-plane/crates/domain/src/discovery.rs:447-568`
  - `scripts/test-gate.sh:2397-2414`
  - `rg -n "OperationRecorder|operation recorder|DiscoveryOperation|trait DiscoveryFilesystem|pub trait"` returned no P053 implementation hits.
- Why It Matters: The proposal's deterministic no-pre-initialize-traversal proof depends on a shared discovery boundary that can record operations. Static helper methods can implement the behavior, but they cannot provide the same injectable proof surface or catch future transport/engine traversal before `initialize` without code inspection.
- Recommended Action: Convert `DiscoveryFilesystem` into the promised trait or add an explicit operation-recorder wrapper used by ACP and engine discovery paths. Add a gate fixture that records operation order and asserts `initialize` precedes all workspace/worktree/generated-state traversal attempts.

### ARCH-002 Phase 0 cap validation was skipped while defaults were hard-coded

- Severity: Major.
- Confidence: High.
- Related Proposal Items / Requirements: REQ-001, REQ-007, REQ-017.
- Evidence Type: code.
- Evidence:
  - `control-plane/crates/domain/src/discovery.rs:201-209`
  - `control-plane/crates/engine/src/contracts.rs:443-444`
  - Missing `docs/proposals/053.review/cap-validation.json`
- Why It Matters: P053 intentionally made byte and count caps candidate defaults until production sampling validated p90/p99 values. Hard-coding caps without the evidence artifact can reject valid workflows or under-bound risky workloads with no recorded owner decision.
- Recommended Action: Add the Phase 0 cap-validation artifact with dependency readiness, sampled execution IDs, size distributions, chosen caps, exposure mode, coverage gaps, and reviewer signoff. Wire a lightweight gate check for the artifact before closeout.

## Product Review

**Summary:** At Risk.

### PROD-001 Production exposure mode is unknown

- Severity: Major.
- Confidence: High.
- Related Proposal Items / Requirements: REQ-001, REQ-017.
- Evidence Type: code.
- Evidence:
  - Proposal logical line 621 requires `phase_1_exposure_mode`.
  - Missing `docs/proposals/053.review/cap-validation.json`.
- Why It Matters: The implementation has durable readback code, but the proposal blocks production-shippable Phase 1 unless the exposure mode is recorded and the matching evidence path exists. Without that decision, operators and release owners cannot tell whether this branch is production-exposed or gate-only/internal.
- Recommended Action: Record `phase_1_exposure_mode` in `docs/proposals/053.review/cap-validation.json`; if production-exposed, include the minimal readback evidence and support path; if gate-only/internal, state that closeout is intentionally not production-ready.

## UI Review

**Summary:** Weak.

### UI-001 P053 operator UI surfaces are missing

- Severity: Major.
- Confidence: High.
- Related Proposal Items / Requirements: REQ-020.
- Evidence Type: code.
- Evidence:
  - `Chainworks Forge/Views/ArtifactInspectorView.swift:3-6`
  - `Chainworks Forge/Views/FailedStageEvidencePanel.swift:12-77`
  - Targeted Swift search found no `Discovery Mode`, `Startup Performance`, chart tokens, Show Issues Only, Copy Path, Open Location, or Source Changes states.
- Why It Matters: The control-plane can produce diagnostics, but the proposal explicitly requires operator-facing UI for interpreting missing/stale/rejected outputs and startup latency. Without it, the product still exposes the original ambiguity to operators outside raw JSON/readback paths.
- Recommended Action: Implement Phase 3 UI or split Phase 3 into a follow-up proposal and narrow P053 closeout accordingly. At minimum, map discovery diagnostics into `FailedStageEvidencePanel`, implement a real `ArtifactInspectorView`, and add P053-specific source-change and startup performance states.

## UX Review

**Summary:** At Risk.

### UX-001 Operator clarity checks are absent

- Severity: Major.
- Confidence: Medium.
- Related Proposal Items / Requirements: REQ-017, REQ-020.
- Evidence Type: code.
- Evidence:
  - Proposal logical lines 663-665 require manual latency and operator-clarity evidence.
  - No evidence file exists under `docs/proposals/053.review`.
  - UI search found no startup performance or Forge-overhead/provider-latency presentation.
- Why It Matters: A central P053 user job is to stop misattributing Forge local overhead to provider slowness. Code-level timing separation alone is insufficient if no operator can see and understand the distinction.
- Recommended Action: Add a Phase 1 evidence note with a large-workspace spot-check and a qualitative operator-clarity check. Add a compact log/report/readback rendering if UI remains deferred.

## Delivery / Readiness Review

**Summary:** Not Ready.

### READY-001 Canonical gate was not run by this audit

- Severity: Major.
- Confidence: High.
- Related Proposal Items / Requirements: REQ-015, REQ-021.
- Evidence Type: tests-found.
- Evidence:
  - `scripts/test-gate.sh:2391-2416`
  - No validation commands were run by this audit.
- Why It Matters: The audit skill requires same-tree full regression evidence before reporting a successful verdict. Since this audit did not run the P053 gate, readiness must fail closed even though tests are present.
- Recommended Action: Run `./scripts/test-gate.sh proposal-053` on branch `codex/p053-manual-merge-1833dd16` and attach the exact terminal/log evidence in a follow-up audit or addendum.

### READY-002 Required Phase 1 security signoff is absent

- Severity: Major.
- Confidence: High.
- Related Proposal Items / Requirements: REQ-018.
- Evidence Type: code.
- Evidence:
  - No `docs/proposals/053.review/security-checklist.md`.
  - No `docs/proposals/053.review/security-risk-acceptance.json`.
- Why It Matters: P053 changes path authorization, symlink handling, byte caps, and raw-path validation bypass behavior. The proposal explicitly made security signoff or risk acceptance a Phase 1 PR landing criterion.
- Recommended Action: Add the security checklist or risk-acceptance artifact and include named Security Reviewer and Architecture Reviewer signoff.

## Readiness Checklist

| Check | Status | Evidence / Note |
|---|---|---|
| Build succeeds on targeted platform(s) | Not Checked | No build or test command was run. |
| Core user flow runtime-validated | Not Checked | Source inspection only. |
| Empty/loading/error states covered | Partial | Control-plane rejected/missing reasons exist; macOS UI is missing. |
| Accessibility risk acceptable | Fail | Required accessibility labels and Dynamic Type behavior are not implemented in P053 UI. |
| Localization risk acceptable | Not Checked | UI not implemented. |
| Critical tests executed | Not Checked | Tests found but not run. |
| Full regression suite / canonical full gate passed on same tree/HEAD | Not Checked | `./scripts/test-gate.sh proposal-053` not run. |
| Privacy/permissions/entitlements reviewed | Partial | Path-boundary code exists; required security artifact absent. |
| Phase 0 evidence artifact present | Fail | `docs/proposals/053.review/cap-validation.json` missing. |
| Phase 1 security artifact present | Fail | `security-checklist.md` or `security-risk-acceptance.json` missing. |

## Verification Log

- `git status --short --branch && git rev-parse HEAD`
- `find docs/proposals -maxdepth 1 -type f -name '053*.md' -o -name '*p053*.md' -o -name '*proposal-053*.md'`
- `python3 /Users/user/.agents/skills/proposal-implementation-audit/scripts/report_path.py /Users/user/Documents/Chainworks\ Forge/.chainworks/worktrees/codex-p053-manual-merge-1833dd16/docs/proposals/053-bounded-acp-artifact-discovery-and-startup-latency.md`
- `python3 - <<'PY' ... render document_markdown with logical line numbers ... PY`
- `rg -n "p053|proposal-053|ExpectedOutputSpec|PrePromptExpectedOutputMetadata|OutputDiscoveryDecision|DiscoveryFilesystem|GitManifestRunner|settle_agent_outputs_from_discovery_decisions|load_declared_output_bytes|legacy_broad_discovery|agent_execution_discovery_diagnostics|proposal_053|output_discovery|changed_files_manifest|CHAINWORKS_OUTPUT|provider_envelope_oversized|chainworks_output_oversized|metadata_timeout|bounded_meta_root|snapshot_workspace" control-plane "Chainworks Forge" scripts docs/reference docs/proposals/053.review -g'!*target*'`
- `find docs -path '*053.review*' -maxdepth 5 -type f -print`
- `nl -ba scripts/test-gate.sh | sed -n '2388,2418p'`
- `nl -ba docs/reference/test-gates.md | sed -n '908,946p'`
- `nl -ba control-plane/crates/domain/src/discovery.rs | sed -n '110,330p;440,590p;680,790p;880,1010p;1110,1168p;1200,1268p;1290,1578p'`
- `nl -ba control-plane/crates/engine/src/executor.rs | sed -n '860,1090p;1180,1365p;1400,1425p;1568,1595p;2428,2465p;2828,2860p;3098,3290p;3996,4025p;4268,4285p;5290,5795p'`
- `nl -ba control-plane/crates/acp/src/transport.rs | sed -n '1020,1215p;1215,1395p;1395,1565p'`
- `nl -ba control-plane/crates/db/migrations/026_p053_discovery_diagnostics.sql`
- `nl -ba control-plane/crates/db/src/repos/agent_execution_discovery_diagnostics.rs | sed -n '1,160p;220,285p'`
- `nl -ba control-plane/crates/graphql-server/src/types/stage.rs | sed -n '220,285p'`
- `nl -ba control-plane/crates/mcp-server/src/tools/reports.rs | sed -n '160,240p'`
- `rg -n "OperationRecorder|operation recorder|DiscoveryOperation|trait DiscoveryFilesystem|pub trait|record.*operation|operation_log|filesystem.*record" control-plane/crates/domain/src/discovery.rs control-plane/crates/acp/src control-plane/crates/engine/src scripts/test-gate.sh`
- `rg -n "acp_pre_initialize_local_latency_ms|acp_initialize_latency_ms|acp_session_new_latency_ms|acp_prompt_duration_ms|acp_pre_prompt_metadata_latency_ms|acp_expected_output_spec_count|acp_control_plane_manifest_latency_ms|acp_exact_output_acceptance_latency_ms|acp_meta_root_discovery_latency_ms|acp_git_changed_files_latency_ms|acp_expected_outputs_found_count|acp_expected_outputs_missing_count|acp_expected_outputs_stale_count|acp_expected_outputs_rejected_count|acp_cap_validation|acp_reconciliation_pending|Provider latency|Forge overhead" control-plane "Chainworks Forge" docs/reference scripts -g'!*target*'`
- `rg -n "Discovery Mode|Startup Performance|forgeOverhead|providerLatency|Show Issues Only|Metadata Timeout|Copy Path|Open Location|Source Changes" "Chainworks Forge" -g'*.swift'`

## Recommended Next Actions

1. Add `docs/proposals/053.review/cap-validation.json` with the required Phase 0 fields and either add `proposal-053-phase0` or fold the artifact check into `proposal-053`.
2. Add the Phase 1 security checklist or risk-acceptance artifact.
3. Implement or explicitly revise the `DiscoveryFilesystem` trait/operation-recorder contract, then gate traversal-order evidence.
4. Add the required structured metrics or update the proposal/reference truth if metric names have intentionally changed.
5. Implement Phase 3 macOS UI surfaces or split/de-scope Phase 3 from P053 closeout.
6. Run `./scripts/test-gate.sh proposal-053` on the branch and capture same-tree evidence before any successful closeout claim.
