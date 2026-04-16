# Consolidated Proposal Review

## 0. Review Mode and Proposal Evidence Summary
- Mode used: `proposal-readiness`
- Evidence completeness: `Complete`
- Proposal / docs reviewed:
  - `docs/proposals/049-steward-analysis-system.md`
  - `docs/proposals/049-steward-analysis-system.review/evidence-pack.md`
  - `.review-baselines/current-system-baseline.md`
  - `docs/reference/current-system-baseline.md`
  - `docs/reference/forge-steward.md`
  - `docs/reference/rust-control-plane.md`
  - `docs/reference/context-strategy-and-experiment-framework.md`
  - `examples/steward/steward_config.yaml`
  - `examples/agents/agents.yaml`
- Reusable baseline used:
  - `.review-baselines/current-system-baseline.md`
  - `docs/reference/current-system-baseline.md`
  - `docs/reference/forge-steward.md`
  - `docs/reference/rust-control-plane.md`
  - `docs/reference/context-strategy-and-experiment-framework.md`
- Baseline reused:
  - current system boundary and review posture
  - stable Steward V1 semantics
  - current daemon northbound boundary
- Baseline refreshed:
  - active run-start ingress paths
  - current `StartRun` command contract
  - historical completed-run implications for the new frozen fields
- Baseline freshness: `Partially refreshed`
- Proposal-specific integration context: `Missing`
- Targeted context refresh performed: `Yes`
- External research used: `None`
- Research pack: `None`
- Sources reused: `None`
- Sources refreshed: `None`
- Time-sensitive external guidance: `None`
- Code areas inspected:
  - `control-plane/crates/domain/src/{commands,idea,run}.rs`
  - `control-plane/crates/workflow/src/definition.rs`
  - `control-plane/crates/engine/src/command_handler.rs`
  - `control-plane/crates/graphql-server/src/schema.rs`
  - `control-plane/crates/mcp-server/src/tools/{ideas,runs}.rs`
  - `control-plane/crates/db/src/repos/ideas.rs`
- Current repo contradictions found:
  - active GraphQL and MCP run-start ingress still do not guarantee the YAML/catalog inputs that the proposal’s frozen metadata contract relies on,
  - the proposal does not define historical-run eligibility once the new frozen fields are introduced.
- Runtime evidence used: `None`
- Provenance of key evidence:
  - proposal text,
  - stable references,
  - active catalog and steward config,
  - targeted Rust code inspection,
  - stale local review artifacts for freshness control only.
- Remaining assumptions:
  - active GraphQL `startRun` and MCP `runs.start` stay live unless the proposal narrows them explicitly,
  - Steward remains a historical observer over completed persisted runs.
- Remaining blockers:
  - one `Critical` architecture blocker on active run-start compatibility,
  - one `High` architecture blocker on historical-run semantics.

## 1. Executive Summary
- Overall readiness: `Red`
- Confidence: `High`
- Proposal completeness signal: `Mixed`
- What the current draft fixed:
  1. The old `project_key` blocker is closed: the proposal now names the idea-domain, repo, migration, and `ideas.create` ingress chain.
  2. The old active-catalog parity blocker is closed: `agent_catalog_snapshot`, `workflow_snapshot`, and `config_change_log` now have explicit materialization rules and artifact inventory.
  3. The old `context_strategy_profiles` hash-scope blocker is closed: the proposal now includes that slice explicitly in parsed-object hashing.
  4. The old northbound readback blocker is closed: GraphQL, MCP, and `steward-analysis://` are now explicit.
- What still blocks implementation readiness:
  1. The proposal’s frozen metadata and snapshot owner chain is not yet reconciled with current live run-start ingress paths.
  2. The proposal never defines what Steward does with pre-P049 completed runs that do not carry the new frozen fields.

## 2. Proposal Scope and Completeness
- In scope:
  - deterministic Rust Steward pipeline,
  - run-owned cohort/provenance freezing,
  - daemon-owned current inputs,
  - queue triggers,
  - optional steward LLM lanes,
  - northbound analysis readback.
- Out of scope:
  - Steward dashboard UI,
  - schedule trigger wiring,
  - V2 recommendation synthesis,
  - V3 experiment execution,
  - live-session introspection beyond persisted truth.
- Most important current-head contradictions:
  - `StartRun` freezing depends on compiler-produced workflow/catalog snapshots, but current active run-start surfaces do not guarantee those inputs.
  - historical completed runs are still part of the Steward dataset, but the proposal gives them no explicit eligibility or migration rule once new frozen fields are introduced.
- Most important stale findings that are now closed:
  - missing `project_key` owner chain,
  - incomplete steward input materialization,
  - ambiguous `context_strategy_profiles` hash scope,
  - missing readback surface.

## 4. Discipline Scorecard
| Discipline | Readiness | Confidence | Evidence Completeness | Critical | High | Medium | Low |
|---|---|---|---|---:|---:|---:|---:|
| UI | Green | High | Complete | 0 | 0 | 0 | 0 |
| UX | Green | High | Complete | 0 | 0 | 0 | 0 |
| Architecture | Red | High | Complete | 1 | 1 | 0 | 0 |

## 5. Findings by Discipline

### 5.1 UI Findings
- None.
  This proposal still keeps Steward UI out of scope. No visual or layout readiness issue surfaced in this round.

### 5.2 UX Findings
- None.
  The earlier operator-readback gap is already closed by the current draft’s named GraphQL/MCP/resource reads.

### 5.3 Architecture Findings
- Finding ID: `ARCH-049-01`
  Severity: `Critical`
  Evidence IDs: `NAV-01`, `NAV-02`, `MAP-01`, `MAP-02`, `MAP-03`, `MAP-05`, `DATA-01`, `DATA-02`, `INT-01`, `REAL-04`
  Why it matters:
  The proposal correctly moves frozen workflow metadata and frozen snapshot provenance onto `StartRun`, and it correctly says those values come from compiler output over workflow and agent-catalog YAML. But the current live ingress contract is not aligned with that owner chain. GraphQL `startRun` still hardcodes both YAML paths to `None`, MCP `runs.start` still leaves both optional, and the file inventory does not include `domain/src/commands.rs`, `mcp-server/src/tools/runs.rs`, or GraphQL run-start input changes. As written, one active northbound surface can continue creating runs that cannot satisfy the proposal’s own frozen metadata contract.
  Recommended fix:
  Pick one explicit ingress rule and carry it end to end.
  1. Either make workflow/catalog YAML inputs mandatory on all active run-start surfaces and update the command contract plus GraphQL/MCP ingress accordingly.
  2. Or explicitly declare that only YAML-backed runs are Steward-eligible, and define rejection or ineligibility behavior for GraphQL/non-YAML starts.
  3. Reflect that decision in the file inventory and proof gate.
  Acceptance criteria:
  - Every active run-start surface either supplies the inputs needed for frozen metadata/snapshot production or is explicitly rejected or excluded.
  - The proposal’s files-to-modify list includes the affected ingress files and command contract.
  - The proof gate asserts the chosen run-start rule.
  Confidence: `High`

- Finding ID: `ARCH-049-02`
  Severity: `High`
  Evidence IDs: `DOC-04`, `NAV-04`, `MAP-06`, `DATA-03`, `INT-02`, `REAL-05`
  Why it matters:
  Stable Steward remains a historical observer over completed persisted runs, and Proposal 049 still begins by querying completed runs from the database. The draft widens `Run` with new optional cohort/provenance fields and forbids recomputing those values from mutable files during analysis. But it never states what happens to existing completed runs that predate those fields. Without an explicit exclusion, backfill, or legacy-fallback rule, Steward analysis of the current database remains under-specified at the exact moment it starts relying on those frozen fields.
  Recommended fix:
  Add one explicit historical-run policy.
  1. Exclude pre-P049 runs from Steward cohorts until they carry the new fields, or
  2. backfill them once from already-persisted sources, or
  3. define a bounded legacy fallback set and its confidence impact.
  Acceptance criteria:
  - The proposal states how pre-P049 completed runs are classified for Steward eligibility.
  - That rule is compatible with the “no recomputation from mutable files during analysis” boundary.
  - The proof gate includes coverage for the chosen historical-run rule.
  Confidence: `High`

## 6. Cross-Discipline Conflicts and Decisions
- Conflict:
  The proposal wants run-owned frozen truth, but current active run-start ingress still allows missing compiler inputs.
  Tradeoff:
  Keeping ingress flexible preserves backward compatibility, but it weakens the deterministic owner chain the proposal is trying to establish.
  Decision:
  The proposal must choose between stricter run-start requirements and an explicit Steward-eligibility boundary for non-YAML starts.
  Owner:
  Proposal author.

- Conflict:
  Steward should analyze historical completed runs, but the proposal forbids reconstructing new frozen truth from mutable files during analysis.
  Tradeoff:
  That determinism boundary is correct, but it requires an explicit policy for older rows that predate the new fields.
  Decision:
  Add a historical-run eligibility or backfill rule instead of leaving implementation to infer one.
  Owner:
  Proposal author.

## 7. Prioritized Action Backlog
| Priority | Item | Discipline | Owner | Horizon | Dependencies | Success Metric | Source Findings |
|---|---|---|---|---|---|---|---|
| P0 | Reconcile the frozen metadata/snapshot owner chain with all active run-start ingress surfaces | Architecture | Proposal author | Before next review | None | No active run-start surface can create Steward-eligible runs without the required frozen inputs | `ARCH-049-01` |
| P1 | Add an explicit historical-run eligibility/backfill rule for runs that predate the new frozen fields | Architecture | Proposal author | Before next review | None | Existing completed runs have one deterministic eligibility policy | `ARCH-049-02` |
| P2 | Keep future review rounds anchored to refreshed artifacts, not the stale pre-rewrite review package | Architecture | Proposal author and reviewer | Next review round | refreshed review artifacts | No future review repeats already-closed blockers | freshness evidence in `DOC-09` |

## 8. Validation and Measurement Plan
| Area | What Will Be Measured | Leading Indicators | Guardrails | Review Checkpoint | Rollback / Hold Criteria |
|---|---|---|---|---|---|
| Run-start compatibility | whether every active run-start path can satisfy or explicitly reject the frozen metadata contract | GraphQL/MCP/command contract all reflect the same rule | no hidden non-YAML path remains silently accepted for Steward-eligible runs | next proposal review | hold if any active ingress still requires inference |
| Historical-run semantics | whether completed runs already in the DB have one deterministic eligibility rule | proposal names exclude/backfill/legacy path explicitly | no analysis-time recomputation from mutable files | next proposal review | hold if pre-P049 rows remain undefined |

## 9. Evidence Gaps and Open Questions

### Evidence Gaps
- None blocking.
  Local proposal, stable references, active catalog/config, and current control-plane code were sufficient for a defensible proposal-readiness call.

### Open Questions
- QUESTION-01: Should non-YAML-backed GraphQL or MCP run starts be rejected once Steward freezing lands, or simply remain outside Steward cohorts?
- QUESTION-02: For pre-P049 runs, does the repo want exclusion, one-time backfill, or a bounded legacy fallback set?
