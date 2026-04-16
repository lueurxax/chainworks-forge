# Proposal 049: Steward Analysis System Multi-Lens Audit R5

| Field | Value |
|---|---|
| Proposal | `docs/proposals/049-steward-analysis-system.md` |
| Repository Root | `/Users/user/Documents/Chainworks Forge` |
| Git SHA | `af3054c73064b05e42cb816a81a3c5fb0c2e29d9` |
| Working Tree | Dirty; audited current worktree, not a clean HEAD baseline |
| Audited At | `2026-04-16T15:01:23+03:00` |
| Platform Scope | macOS-hosted Rust control-plane / daemon / GraphQL / MCP surfaces; no dedicated UI dashboard scope |
| Proposal State | Active |
| Overall Conformance | Partial |
| Overall Readiness | Not Ready |
| Audit Confidence | High |

## Executive Verdict

The current implementation satisfies every explicit Proposal 049 requirement that this audit decomposed, and the focused `./scripts/test-gate.sh proposal-049` gate passed on this tree. The roll-up is still fail-closed as `Partial / Not Ready` because the required same-tree canonical full gate could not run on this host: `./scripts/test-gate.sh full` exited 3 before executing tests due the repository's remote-UI host policy. This is a delivery/readiness blocker, not a found P049 functional gap.

## Lens Scorecard

| Lens | Assessment | Top Risk | Confidence |
|---|---|---|---|
| Conformance | Focused implemented; roll-up partial | Same-tree full regression unavailable on this host | High |
| Architecture | Strong | Broad dirty worktree prevents clean release-baseline attribution | High |
| Product | Acceptable | Operator value is proven through northbound/readback surfaces, but release sign-off is blocked by full gate policy | High |
| UI | Acceptable / No dedicated UI scope | Proposal explicitly excludes a Steward dashboard | High |
| UX | Acceptable | CLI/API readback is covered; no human-facing dashboard UX is in scope | High |
| Readiness | Not Ready | Canonical full gate blocked; working tree is dirty | High |

## Proposal Contract

### Scope

Proposal 049 ports the deterministic Steward V1 observer pipeline to the Rust daemon. The scope includes cohort selection, metric collection, anomaly detection, dossiers, persisted analysis records, active-catalog steward LLM lanes, daemon-owned current-input hashing, queue-based triggers, and northbound analysis readback. The proposal later narrows the report/resource lane to dedicated GraphQL queries plus MCP `steward.get_analysis` and `steward-analysis://{analysis_id}` readback, not unrelated run reports. Source: `docs/proposals/049-steward-analysis-system.md:9-10`, `docs/proposals/049-steward-analysis-system.md:768-775`.

### Locked Decisions

- The deterministic pipeline must use persisted DB truth, daemon-owned parsed current inputs, and canonical hashing/serialization for deterministic steps. Source: `docs/proposals/049-steward-analysis-system.md:77-85`.
- The primary cohort key is exactly `(workflow_family, risk_class)`; `project_key` and `stack` are quality/diagnostic facets only. Source: `docs/proposals/049-steward-analysis-system.md:66-75`.
- Legacy pre-P049 runs missing frozen cohort/snapshot truth are excluded from deterministic cohorting. Source: `docs/proposals/049-steward-analysis-system.md:48-51`.
- The optional steward-agent lanes use active-catalog parity and `CHAINWORKS_META_ROOT`, not a second path vocabulary. Source: `docs/proposals/049-steward-analysis-system.md:87-108`.
- Northbound readback must surface analyses through GraphQL, MCP tools/resources, and report-style reads. Source: `docs/proposals/049-steward-analysis-system.md:62-63`.
- The dedicated report/resource read lane must not hide Steward analyses inside unrelated run reports. Source: `docs/proposals/049-steward-analysis-system.md:768-775`.

### Primary User Flows

- Operator starts a run with workflow/catalog inputs and the control-plane freezes cohort and snapshot provenance at run creation.
- Daemon bootstraps current Steward inputs, validates config, hashes effective config/catalog state, and records pending config-change analysis without executing immediately.
- Steward analysis work item loads completed persisted runs, selects a deterministic cohort, emits metrics/dossiers/signals/artifacts, and persists the analysis record.
- Optional active-catalog steward agents consume materialized analysis-owned inputs and write active-catalog outputs without blocking deterministic persistence.
- Operator reads analyses, recommendations, run links, status, and artifacts through GraphQL, MCP tools, and `steward-analysis://{analysis_id}`.

### UI Commitments

No dedicated Steward dashboard UI is in scope. Source: `docs/proposals/049-steward-analysis-system.md:992-998`.

### UX Commitments

Operators must be able to distinguish `completed`, `inconclusive`, and `failed` analyses from northbound reads, and readback must preserve the primary cohort identity while keeping project/stack data in diagnostic context. Source: `docs/proposals/049-steward-analysis-system.md:936-942`.

### Acceptance Criteria

The proposal acceptance criteria cover cohort owner contracts, frozen snapshot provenance, current-input bootstrap semantics, deterministic pipeline behavior, active-catalog steward parity, metric-source correctness, northbound readback, and trigger semantics. Source: `docs/proposals/049-steward-analysis-system.md:890-948`.

### Test / Evidence Requirements

The proposal requires a focused `proposal-049|p049` proof gate that covers workflow/idea metadata freezing, cohort grouping, legacy exclusion, snapshot production, bootstrap fallback, canonical serialization, config-change hashing, active-catalog IO, and GraphQL/MCP readback. Source: `docs/proposals/049-steward-analysis-system.md:951-988`.

### Explicit Exclusions

Dedicated Steward dashboard UI, schedule trigger wiring, V2 recommendation synthesis, V3 experiment execution, and live-session introspection outside persisted truth are out of scope. Source: `docs/proposals/049-steward-analysis-system.md:992-998`.

## Proposal Fidelity / Divergence

### Matches

- The focused gate exists as `proposal-049|p049` and covers the Rust control-plane Steward slice. Evidence: `scripts/test-gate.sh:1535-1547`, `docs/reference/test-gates.md:616-635`.
- Workflow compiler now produces workflow family, risk class, stack, canonical workflow/catalog snapshots, and hashes. Evidence: `control-plane/crates/workflow/src/compiler.rs:25-49`, `control-plane/crates/workflow/src/compiler.rs:87-99`.
- `StartRun` freezes plan-derived metadata and idea-derived `project_key` onto the persisted run. Evidence: `control-plane/crates/engine/src/command_handler.rs:143-212`.
- Steward analysis persistence, links, recommendations, runtime state, and widened run/stage fields exist in the P049 migration. Evidence: `control-plane/crates/db/migrations/012_steward_analysis.sql:3-83`.
- Deterministic service flow persists analysis records, deterministic artifacts, no-signal context dossiers, optional steward-agent artifact IDs, and failed analysis rows. Evidence: `control-plane/crates/engine/src/steward/service.rs:126-341`, `control-plane/crates/engine/src/steward/service.rs:401-447`.
- Active-catalog steward lanes load the current catalog, resolve backend/MCP, build `CHAINWORKS_META_ROOT` prompts, and execute through ACP. Evidence: `control-plane/crates/engine/src/executor.rs:49-140`, `control-plane/crates/engine/src/executor.rs:224-310`, `control-plane/crates/engine/src/executor.rs:1239-1268`.
- GraphQL, MCP tools, and the dedicated `steward-analysis://{analysis_id}` resource expose persisted analysis truth. Evidence: `control-plane/crates/graphql-server/src/schema.rs:128-196`, `control-plane/crates/mcp-server/src/tools/steward.rs:10-116`, `control-plane/crates/mcp-server/src/server.rs:276-280`, `control-plane/crates/mcp-server/src/server.rs:460-471`.

### Divergences

- No explicit P049 implementation divergence was found in the focused audit.
- Overall audit roll-up diverges from a successful conformance verdict only because the canonical full gate could not run on this host.

### Ambiguities / Evidence Gaps

- Same-tree full regression evidence is missing because `./scripts/test-gate.sh full` is restricted to approved remote UI hosts and exited before test execution on this host.
- The working tree is dirty and contains broad modified/untracked control-plane and docs files; this audit is therefore tied to the current worktree state, not clean committed HEAD.
- Schedule trigger wiring is intentionally out of scope, so only manual, config-change pending, and post-run interval convergence were audited.

## Requirement Summary

| Status | Count |
|---|---:|
| Implemented | 12 |
| Partially Implemented | 0 |
| Missing | 0 |
| Not Verifiable | 0 |

Note: all explicit P049 requirements audited below are implemented. `Overall Conformance` is still `Partial` because this audit skill requires passing same-tree full regression evidence before reporting a successful roll-up, and that evidence was unavailable on this host.

## Requirement Audit

### REQ-001 Workflow metadata and snapshot freezing

- Proposal Source: Cohort owner contract and frozen snapshot provenance, `docs/proposals/049-steward-analysis-system.md:892-907`.
- Status: Implemented.
- Evidence Type: code, tests-found, tests-run.
- Evidence:
  - `control-plane/crates/workflow/src/compiler.rs:25-49`
  - `control-plane/crates/workflow/src/compiler.rs:87-99`
  - `./scripts/test-gate.sh proposal-049` passed workflow metadata/snapshot tests.
- Gap / Note: The compiler requires workflow/catalog paths, loads parsed YAML, derives owner fields, serializes canonical snapshots, hashes them, and returns them in `RunPlan`.

### REQ-002 Run creation persists cohort, project, and provenance ownership

- Proposal Source: Cohort owner contract and frozen snapshot provenance, `docs/proposals/049-steward-analysis-system.md:892-907`.
- Status: Implemented.
- Evidence Type: code, tests-found, tests-run.
- Evidence:
  - `control-plane/crates/db/migrations/012_steward_analysis.sql:3-16`
  - `control-plane/crates/engine/src/command_handler.rs:143-212`
  - `./scripts/test-gate.sh proposal-049` passed `test_start_run_persists_delivery_configuration_json` and P049 db steward tests.
- Gap / Note: `StartRun` compiles before persistence, applies deterministic `"untagged"` fallback for `Idea.project_key`, and persists run-owned workflow family, project key, risk class, stack, snapshot hashes, and snapshot JSON.

### REQ-003 Legacy exclusion and primary cohort grouping

- Proposal Source: Cohort owner contract, `docs/proposals/049-steward-analysis-system.md:894-901`.
- Status: Implemented.
- Evidence Type: code, tests-found, tests-run.
- Evidence:
  - `control-plane/crates/engine/src/steward/cohort.rs:6-36`
  - `control-plane/crates/engine/src/steward/cohort.rs:38-77`
  - `control-plane/crates/engine/src/steward/cohort.rs:79-100`
  - `./scripts/test-gate.sh proposal-049` passed `steward_cohort_classifier_tests` and engine steward tests.
- Gap / Note: Eligibility requires frozen owner/snapshot fields. The primary cohort key contains only `workflow_family` and `risk_class`; `project_key` and `stack` are quality facets.

### REQ-004 Steward persistence schema and repository truth

- Proposal Source: Deterministic pipeline and northbound readback, `docs/proposals/049-steward-analysis-system.md:916-941`.
- Status: Implemented.
- Evidence Type: code, tests-found, tests-run.
- Evidence:
  - `control-plane/crates/db/migrations/012_steward_analysis.sql:18-83`
  - `control-plane/crates/engine/src/steward/service.rs:307-341`
  - `control-plane/crates/engine/src/steward/service.rs:401-447`
  - `./scripts/test-gate.sh proposal-049` passed P049 db steward tests and failed-analysis persistence tests.
- Gap / Note: The schema persists analyses, run links, recommendations, runtime state, artifact IDs, status, trigger reason, and error summary. Failed analysis persistence exists and is tested.

### REQ-005 Daemon-owned current-input bootstrap semantics

- Proposal Source: Current-input bootstrap semantics, `docs/proposals/049-steward-analysis-system.md:908-913`.
- Status: Implemented.
- Evidence Type: code, tests-found, tests-run.
- Evidence:
  - `control-plane/crates/engine/src/steward/config.rs:9-18`
  - `control-plane/crates/engine/src/steward/config.rs:87-183`
  - `control-plane/crates/engine/src/steward/config.rs:204-250`
  - `control-plane/crates/daemon/src/steward_runtime.rs:10-65`
  - `./scripts/test-gate.sh proposal-049` passed `steward_runtime_bootstrap_tests`.
- Gap / Note: Invalid config falls back to default runtime semantics, the effective config is canonically hashed, agent catalog is parsed and hashed, and config changes set a pending flag.

### REQ-006 Deterministic metrics, anomaly detection, and canonical artifacts

- Proposal Source: Deterministic pipeline and metric-source correctness, `docs/proposals/049-steward-analysis-system.md:914-935`.
- Status: Implemented.
- Evidence Type: code, tests-found, tests-run.
- Evidence:
  - `control-plane/crates/engine/src/steward/metrics.rs:47-155`
  - `control-plane/crates/engine/src/steward/anomaly.rs:26-135`
  - `control-plane/crates/engine/src/steward/json.rs:7-31`
  - `control-plane/crates/engine/src/steward/service.rs:196-240`
  - `./scripts/test-gate.sh proposal-049` passed `steward_metrics_tests` and deterministic pipeline tests.
- Gap / Note: Metrics are collected from persisted run/stage/approval/session-derived truth, anomaly detection uses configured thresholds, and canonical JSON serialization/writes are centralized.

### REQ-007 Window semantics, inconclusive status, and no-signal dossiers

- Proposal Source: Deterministic pipeline, `docs/proposals/049-steward-analysis-system.md:916-920`.
- Status: Implemented.
- Evidence Type: code, tests-found, tests-run.
- Evidence:
  - `control-plane/crates/engine/src/steward/service.rs:170-194`
  - `control-plane/crates/engine/src/steward/service.rs:242-253`
  - `./scripts/test-gate.sh proposal-049` passed deterministic pipeline tests.
- Gap / Note: Windows below `minimum_window_size` become `Inconclusive`; no-signal analyses still write bounded context dossiers.

### REQ-008 Active-catalog steward agent parity

- Proposal Source: Active-catalog steward parity, `docs/proposals/049-steward-analysis-system.md:921-928`.
- Status: Implemented.
- Evidence Type: code, tests-found, tests-run.
- Evidence:
  - `control-plane/crates/engine/src/steward/service.rs:255-297`
  - `control-plane/crates/engine/src/executor.rs:49-140`
  - `control-plane/crates/engine/src/executor.rs:224-310`
  - `control-plane/crates/engine/src/executor.rs:1239-1268`
  - `./scripts/test-gate.sh proposal-049` passed active-catalog steward IO and ACP-backed steward executor tests.
- Gap / Note: `system_steward` and `steward_auditor` are resolved from the active catalog, expected output paths are derived from catalog artifact templates, and optional lane failures do not block deterministic persistence.

### REQ-009 Workflow snapshot index represents multi-snapshot truth

- Proposal Source: Active-catalog steward parity item 20, `docs/proposals/049-steward-analysis-system.md:923-927`.
- Status: Implemented.
- Evidence Type: code, tests-found, tests-run.
- Evidence:
  - `control-plane/crates/engine/src/steward/service.rs:232-236`
  - `control-plane/crates/engine/src/steward/service.rs:457-495`
  - `./scripts/test-gate.sh proposal-049` passed deterministic artifact tests.
- Gap / Note: The analysis writes a singular workflow snapshot input as an index containing all cohort snapshot hashes and run IDs rather than collapsing truth into one scalar.

### REQ-010 Trigger convergence on StewardAnalysis work items

- Proposal Source: Trigger semantics, `docs/proposals/049-steward-analysis-system.md:944-948`.
- Status: Implemented.
- Evidence Type: code, tests-found, tests-run.
- Evidence:
  - `control-plane/crates/engine/src/command_handler.rs:470-486`
  - `control-plane/crates/engine/src/executor.rs:1239-1268`
  - `control-plane/crates/daemon/src/steward_runtime.rs:32-40`
  - `./scripts/test-gate.sh proposal-049` passed `steward_trigger_tests` and MCP manual trigger tests.
- Gap / Note: Manual commands enqueue `WorkItemKind::StewardAnalysis`; executor consumes the same work-item kind using daemon runtime inputs. Config-change bootstrap sets pending state only.

### REQ-011 GraphQL northbound readback

- Proposal Source: Northbound readback, `docs/proposals/049-steward-analysis-system.md:936-942`.
- Status: Implemented.
- Evidence Type: code, tests-found, tests-run.
- Evidence:
  - `control-plane/crates/graphql-server/src/types/steward.rs:4-72`
  - `control-plane/crates/graphql-server/src/types/steward.rs:75-125`
  - `control-plane/crates/graphql-server/src/schema.rs:128-196`
  - `./scripts/test-gate.sh proposal-049` passed `steward_graphql_readback_tests`.
- Gap / Note: GraphQL exposes list/get queries, statuses, hashes, artifact IDs, linked runs, and recommendations.

### REQ-012 MCP tools and `steward-analysis://` resource readback

- Proposal Source: Northbound readback and report/resource lane, `docs/proposals/049-steward-analysis-system.md:749-775`, `docs/proposals/049-steward-analysis-system.md:936-942`.
- Status: Implemented.
- Evidence Type: code, tests-found, tests-run.
- Evidence:
  - `control-plane/crates/mcp-server/src/tools/steward.rs:10-116`
  - `control-plane/crates/mcp-server/src/server.rs:276-280`
  - `control-plane/crates/mcp-server/src/server.rs:460-471`
  - `./scripts/test-gate.sh proposal-049` passed `steward_mcp_tools_tests` and resource parity tests.
- Gap / Note: MCP exposes manual trigger, list/get tools, and a resource URI that returns the same persisted analysis, run links, and recommendations.

## Architecture Review

**Summary:** Strong.

### ARCH-001 Deterministic and non-deterministic boundaries are cleanly separated

- Severity: Note.
- Confidence: High.
- Related Proposal Items / Requirements: REQ-004, REQ-006, REQ-008.
- Evidence Type: code, tests-found, tests-run.
- Evidence:
  - `control-plane/crates/engine/src/steward/service.rs:126-341`
  - `control-plane/crates/engine/src/steward/service.rs:401-447`
  - `control-plane/crates/engine/src/executor.rs:49-140`
  - `./scripts/test-gate.sh proposal-049` passed.
- Why It Matters: P049 depends on deterministic analysis records surviving optional LLM lane absence or failure. The implementation keeps persistence in the deterministic service and invokes catalog agents only as optional artifact producers.
- Recommended Action: Keep this boundary intact when adding future schedule trigger wiring or V2/V3 Steward features.

### ARCH-002 Dirty worktree prevents clean release-baseline attribution

- Severity: Minor.
- Confidence: High.
- Related Proposal Items / Requirements: All P049 requirements.
- Evidence Type: code, tests-run.
- Evidence:
  - `git status --short` reported broad modified and untracked control-plane/docs files.
  - Git SHA: `af3054c73064b05e42cb816a81a3c5fb0c2e29d9`.
- Why It Matters: The audit can validate the current workspace, but it cannot prove that a clean committed baseline contains exactly this implementation.
- Recommended Action: Commit or otherwise freeze the intended integration tree, then rerun the focused gate and full gate from that clean baseline.

## Product Review

**Summary:** Acceptable.

### PROD-001 Operator job is implemented through durable readback, not a dashboard

- Severity: Note.
- Confidence: High.
- Related Proposal Items / Requirements: REQ-011, REQ-012.
- Evidence Type: code, tests-found, tests-run.
- Evidence:
  - `control-plane/crates/graphql-server/src/schema.rs:128-196`
  - `control-plane/crates/mcp-server/src/tools/steward.rs:10-116`
  - `control-plane/crates/mcp-server/src/server.rs:460-471`
  - `./scripts/test-gate.sh proposal-049` passed GraphQL/MCP readback tests.
- Why It Matters: The proposal's product value is durable observability over persisted run truth. The implementation exposes that through existing northbound surfaces, matching the proposal instead of adding out-of-scope UI.
- Recommended Action: For product sign-off, validate one real operator readback scenario against a seeded database after the full gate can run.

## UI Review

**Summary:** Acceptable / No dedicated UI scope.

### UI-001 No Steward dashboard UI was required or audited

- Severity: Note.
- Confidence: High.
- Related Proposal Items / Requirements: Explicit exclusions.
- Evidence Type: code.
- Evidence:
  - `docs/proposals/049-steward-analysis-system.md:992-998`
- Why It Matters: It would be incorrect to fail P049 for missing visual surfaces when the proposal explicitly excludes a dedicated dashboard.
- Recommended Action: If a Steward dashboard becomes desired, create a separate proposal with UI-specific requirements and runtime validation criteria.

## UX Review

**Summary:** Acceptable.

### UX-001 Northbound status and evidence readback preserve operator clarity

- Severity: Note.
- Confidence: High.
- Related Proposal Items / Requirements: REQ-011, REQ-012.
- Evidence Type: code, tests-found, tests-run.
- Evidence:
  - `control-plane/crates/graphql-server/src/types/steward.rs:4-22`
  - `control-plane/crates/mcp-server/src/tools/steward.rs:104-116`
  - `control-plane/crates/mcp-server/src/server.rs:460-471`
  - `./scripts/test-gate.sh proposal-049` passed.
- Why It Matters: Operators can distinguish completed, inconclusive, and failed analyses through typed status fields plus error summaries, linked runs, recommendations, and artifact IDs.
- Recommended Action: Keep status/error fields mandatory in future report-style surfaces so readback remains diagnosable without inspecting artifact directories manually.

## Delivery / Readiness Review

**Summary:** Not Ready.

### READY-001 Canonical full gate is unavailable on this host

- Severity: Critical.
- Confidence: High.
- Related Proposal Items / Requirements: Audit roll-up rule; all P049 requirements.
- Evidence Type: tests-run.
- Evidence:
  - `./scripts/test-gate.sh proposal-049` passed.
  - `./scripts/test-gate.sh full` exited 3 before test execution.
  - Failure output: `error: UI tests are remote-only and may not run on this host.`
  - Failure output: `approved remote hosts: smacbook.local,smacbook`
  - Failure output: `observed host names: 0000659.localdomain,0000659`
- Why It Matters: The audit workflow requires same-tree full regression evidence before reporting `Implemented`, `Ready`, or `Ready with Risks`. Focused P049 evidence is strong but not sufficient for a successful roll-up.
- Recommended Action: Run `./scripts/test-gate.sh full` on an approved remote UI host or CI environment, then rerun this audit or append a new versioned audit report.

### READY-002 Current worktree is not a clean release baseline

- Severity: Minor.
- Confidence: High.
- Related Proposal Items / Requirements: All P049 requirements.
- Evidence Type: code.
- Evidence:
  - `git status --short` reported broad modified/untracked files, including new P049 implementation surfaces under `control-plane/crates/engine/src/steward/`, `control-plane/crates/daemon/src/steward_runtime.rs`, `control-plane/crates/db/migrations/012_steward_analysis.sql`, `control-plane/crates/graphql-server/src/types/steward.rs`, and `control-plane/crates/mcp-server/src/tools/steward.rs`.
- Why It Matters: Dirty-tree audits are useful for implementation truth, but release readiness needs a reproducible committed tree.
- Recommended Action: Commit the intended P049 implementation state and rerun focused plus full gates from the committed baseline.

## Readiness Checklist

| Check | Status | Evidence / Note |
|---|---|---|
| Build succeeds on targeted platform(s) | Partial | Focused Rust P049 gate passed; full gate did not run due host policy. |
| Core user flow runtime-validated | Pass | Control-plane focused tests covered start-run metadata freeze, analysis pipeline, triggers, GraphQL, MCP, and ACP-backed steward executor paths. |
| Empty/loading/error states covered | Partial | `failed` and `inconclusive` analysis states are persisted/readable; no UI empty/loading states are in scope. |
| Accessibility risk acceptable | Not Checked | No dedicated UI dashboard scope. |
| Localization risk acceptable | Not Checked | No user-facing UI copy scope. |
| Critical tests executed | Pass | `./scripts/test-gate.sh proposal-049` passed. |
| Full regression suite / canonical full gate passed on same tree/HEAD | Fail | `./scripts/test-gate.sh full` exited 3 due remote-UI host policy before executing. |
| Privacy/permissions/entitlements reviewed | Not Checked | P049 is a local control-plane/daemon/MCP/GraphQL slice; no Apple entitlement or permission change was audited. |

## Verification Log

- `sed -n '1,760p' /Users/user/.agents/skills/proposal-implementation-audit/SKILL.md`
- `pwd`
- `git rev-parse HEAD`
- `git status --short`
- `python3 /Users/user/.agents/skills/proposal-implementation-audit/scripts/report_path.py /Users/user/Documents/Chainworks\ Forge/docs/proposals/049-steward-analysis-system.md`
- `test -e docs/proposals/049-steward-analysis-system_IMPLEMENTATION_AUDIT_R5.md; printf '%s\n' $?`
- `rg -n "superseded|deprecated|replaced by|obsolete|Proposal 049|049-steward" docs/proposals docs/reference docs/reviews`
- `nl -ba docs/proposals/049-steward-analysis-system.md | sed -n '1,120p'`
- `nl -ba docs/proposals/049-steward-analysis-system.md | sed -n '870,1010p'`
- `nl -ba docs/proposals/049-steward-analysis-system.md | sed -n '706,776p'`
- `rg -n "report-style|report spine|steward-analysis://|RunReport" --glob '!control-plane/target/**' --glob '!control-plane/control-plane/target/**' docs/proposals/049-steward-analysis-system.md control-plane docs/reference/test-gates.md`
- `nl -ba scripts/test-gate.sh | sed -n '1528,1552p'`
- `nl -ba docs/reference/test-gates.md | sed -n '616,638p'`
- `nl -ba control-plane/crates/workflow/src/compiler.rs | sed -n '1,115p'`
- `nl -ba control-plane/crates/engine/src/command_handler.rs | sed -n '135,215p'`
- `nl -ba control-plane/crates/engine/src/command_handler.rs | sed -n '462,490p'`
- `nl -ba control-plane/crates/engine/src/steward/cohort.rs | sed -n '1,115p'`
- `nl -ba control-plane/crates/db/migrations/012_steward_analysis.sql | sed -n '1,95p'`
- `nl -ba control-plane/crates/engine/src/steward/service.rs | sed -n '120,350p'`
- `nl -ba control-plane/crates/engine/src/steward/service.rs | sed -n '397,530p'`
- `nl -ba control-plane/crates/engine/src/steward/config.rs | sed -n '1,255p'`
- `nl -ba control-plane/crates/daemon/src/steward_runtime.rs | sed -n '1,85p'`
- `nl -ba control-plane/crates/engine/src/executor.rs | sed -n '36,145p'`
- `nl -ba control-plane/crates/engine/src/executor.rs | sed -n '224,315p'`
- `nl -ba control-plane/crates/engine/src/executor.rs | sed -n '1236,1270p'`
- `nl -ba control-plane/crates/graphql-server/src/types/steward.rs | sed -n '1,130p'`
- `nl -ba control-plane/crates/graphql-server/src/schema.rs | sed -n '124,198p'`
- `nl -ba control-plane/crates/mcp-server/src/tools/steward.rs | sed -n '1,125p'`
- `nl -ba control-plane/crates/mcp-server/src/server.rs | sed -n '268,284p'`
- `nl -ba control-plane/crates/mcp-server/src/server.rs | sed -n '456,476p'`
- `nl -ba control-plane/crates/engine/src/steward/metrics.rs | sed -n '40,160p'`
- `nl -ba control-plane/crates/engine/src/steward/anomaly.rs | sed -n '20,145p'`
- `nl -ba control-plane/crates/engine/src/steward/json.rs | sed -n '1,55p'`
- `./scripts/test-gate.sh proposal-049` - passed; final output included `==> Proposal 049 control-plane gate passed`.
- `./scripts/test-gate.sh full` - failed before executing tests with exit 3 due remote-only UI host policy.

## Recommended Next Actions

1. Run `./scripts/test-gate.sh full` on an approved host or CI environment that satisfies the repository's UI-test host policy.
2. Commit or otherwise freeze the intended P049 implementation tree, then rerun `./scripts/test-gate.sh proposal-049` and `./scripts/test-gate.sh full` from that baseline.
3. Keep the `proposal-049|p049` focused gate intact; it is currently the strongest targeted proof that the Steward analysis contract remains covered.
