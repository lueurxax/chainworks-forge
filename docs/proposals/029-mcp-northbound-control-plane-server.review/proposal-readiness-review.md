# Consolidated Proposal Review

## 0. Review Mode and Proposal Evidence Summary
- Mode used: `proposal-readiness`
- Evidence completeness: `Complete`
- Proposal / docs reviewed:
  - `docs/proposals/029-mcp-northbound-control-plane-server.md`
  - `docs/proposals/029-mcp-northbound-control-plane-server.review/evidence-pack.md`
  - `.review-baselines/current-system-baseline.md`
  - `docs/reference/rust-control-plane.md`
  - `docs/reference/test-gates.md`
- Reusable baseline used:
  - `.review-baselines/current-system-baseline.md`
  - `docs/reference/rust-control-plane.md`
- Baseline reused:
  - review posture from `.review-baselines/current-system-baseline.md`
  - current MCP/GraphQL surface from `docs/reference/rust-control-plane.md`
- Baseline refreshed:
  - GraphQL mount and resolver seams
  - MCP stdio and dispatch seams
  - `CommandHandler` and `command_journal` ownership
  - test-gate slug occupancy
- Baseline freshness: `Partially refreshed`
- Proposal-specific integration context: `Missing`
- Targeted context refresh performed: `Yes`
- External research used: `None`
- Research pack: `None`
- Sources reused: `None`
- Sources refreshed: `None`
- Time-sensitive external guidance: `None`
- Code areas inspected:
  - `control-plane/crates/graphql-server/src/{server,schema}.rs`
  - `control-plane/crates/mcp-server/src/{server,http,protocol}.rs`
  - `control-plane/crates/engine/src/command_handler.rs`
  - `control-plane/crates/db/src/repos/command_journal.rs`
  - `scripts/test-gate.sh`
- Current repo contradictions found:
  - proposal section 4.3 versus risk 11.4 disagree on the audit redaction owner,
  - proposal section 4.1 versus sections 4.2 and 6 disagree on the auth/capability helper owner,
  - AC-11 promises client-visible `journal_id` without a northbound result contract.
- Runtime evidence used: `None`
- Provenance of key evidence:
  - proposal text,
  - stable baseline docs,
  - targeted code inspection,
  - existing local review artifacts for freshness control only.
- Remaining assumptions:
  - Stage B and Stage C are future slices and not readiness blockers for P029 Stage A.
  - `CommandHandler` remains the canonical mutating command and journaling owner.
- Remaining blockers:
  - No `Critical` blocker remains.
  - One `High` architecture finding and one `Medium` architecture finding should still be fixed before implementation handoff.

## 1. Executive Summary
- Overall readiness: `Amber`
- Confidence: `High`
- Proposal completeness signal: `Mixed`
- Top risks:
  1. The audit contract is still internally inconsistent: section 4.3 correctly moves redaction into the engine, but risk 11.4 points back to `mcp-server/src/audit.rs`, and AC-11 still lacks a concrete MCP or GraphQL result contract for `journal_id`.
  2. The shared auth boundary is still underspecified because section 4.1 creates a shared `control-plane/crates/auth` crate while sections 4.2 and 6 still name `mcp-server/src/auth.rs`.
  3. The stale prior review artifacts still say P029 is missing GraphQL auth, stdio bootstrap, and `command_journal` alignment, which is no longer true on current `HEAD`.
- Top opportunities:
  1. R3 now aligns the proposal to current HEAD on GraphQL auth seam, stdio bootstrap, proof-lane slug, and `command_journal` ownership.
  2. First-wave versus deferred scope is now much clearer, which lowers scope-creep risk during implementation.
  3. The proof lane is now close to implementation-ready once the remaining audit and auth-owner contradictions are cleaned up.

## 2. Proposal Scope and Completeness
- In scope:
  - principal resolution for MCP HTTP and stdio,
  - caller-scoped capability filtering,
  - caller attribution in `command_journal`,
  - GraphQL coexistence and cutover rules,
  - alignment to the current MCP surface.
- Out of scope:
  - new MCP tool families and dropped resource URIs,
  - southbound runtime policy,
  - token rotation and revocation policy,
  - UI changes.
- Deferred intentionally:
  - GraphQL Stage B and Stage C cutover work,
  - future tool/resource expansion listed in proposal section 3.2.
- Most important baseline refreshes performed:
  - confirmed current GraphQL mount and mutation seams,
  - confirmed current stdio transport shape,
  - confirmed current `CommandHandler` and `command_journal` owner path,
  - confirmed `proposal-029` slug is already occupied elsewhere.
- Most important contradictions with current repo:
  - none of the stale prior blockers survive against current R3.
- Most important missing or partial states:
  - audit privacy ownership is still contradictory inside the proposal,
  - client-visible `journal_id` exposure is promised without a response contract,
  - auth helper ownership is still split between shared-crate and MCP-local wording.

## 4. Discipline Scorecard
| Discipline | Readiness | Confidence | Evidence Completeness | Critical | High | Medium | Low |
|---|---|---|---|---:|---:|---:|---:|
| UI | Green | High | Complete | 0 | 0 | 0 | 0 |
| UX | Green | High | Complete | 0 | 0 | 0 | 0 |
| Architecture | Amber | High | Complete | 0 | 1 | 1 | 0 |

## 5. Findings by Discipline

### 5.1 UI Findings
- None.
  This proposal is backend and transport focused. No visual or layout readiness issue surfaced in this round.

### 5.2 UX Findings
- None.
  The current live gaps are architecture and contract-definition issues, not operator task-flow or recovery UX issues.

### 5.3 Architecture Findings
- Finding ID: `ARCH-029-01`
  Severity: `High`
  Evidence IDs: `DATA-03`, `DATA-04`, `INT-03`, `REAL-04`, `REAL-06`, `MAP-06`, `MAP-07`
  Why it matters:
  R3 fixed the major audit-owner issue by extending `command_journal` and moving redaction into an engine-owned `command_journal_redact.rs` path. But the proposal still contradicts itself in two places. First, risk 11.4 says `mcp-server/src/audit.rs::redact(tool_name, args)` performs redaction before insert, which reintroduces the same pre-engine redaction model that section 4.3 explicitly removed. Second, AC-11 says MCP and GraphQL callers can request `journal_id`, but the proposal only defines the internal `Commanded { result, journal_id }` wrapper and never defines the MCP result payload shape or any GraphQL schema change that would expose it. That leaves the audit contract not fully closed even though the main storage owner is now correct.
  Recommended fix:
  1. Rewrite risk 11.4 so it references the same engine-owned redaction path described in section 4.3.
  2. Either define the exact northbound response contract for `journal_id` on both MCP and GraphQL, or remove and defer AC-11 from P029.
  Acceptance criteria:
  - No section of the proposal points redaction back to `mcp-server/src/audit.rs`.
  - The proposal either defines concrete MCP and GraphQL audit-pointer response shapes or explicitly removes the promise from P029 scope.
  - The audit story stays single-owner: `CommandHandler` plus `command_journal`.
  Confidence: `High`

- Finding ID: `ARCH-029-02`
  Severity: `Medium`
  Evidence IDs: `DATA-05`, `INT-01`, `REAL-05`, `DOC-01`
  Why it matters:
  Section 4.1 creates a shared `control-plane/crates/auth` crate and even sketches `resolve_bearer`, `filter_tools`, and `filter_resources` inside it so both MCP and GraphQL consume the same principal and capability tables. But section 4.2 then says the owner is `mcp-server/src/auth.rs::filter_tools(...)`, and section 6 points to `auth.rs::filter_resources`. That is an internal owner-path mismatch on a core security boundary. Implementation could still recover by choosing the shared crate, but the proposal should not require that inference.
  Recommended fix:
  Pick one canonical owner chain. The cleanest answer is:
  1. `control-plane/crates/auth` owns principal resolution and capability tables,
  2. MCP and GraphQL call into that crate,
  3. any transport-local helper is explicitly described as a thin adapter, not the policy owner.
  Acceptance criteria:
  - Sections 4.1, 4.2, and 6 all point to the same owner chain.
  - Capability filtering is clearly shared across MCP and GraphQL without duplicate policy tables.
  Confidence: `High`

## 6. Cross-Discipline Conflicts and Decisions
- Conflict:
  Audit privacy mitigation in risk 11.4 conflicts with the corrected engine-owned redaction model in section 4.3.
  Tradeoff:
  A transport-local redaction helper looks simpler, but it breaks the single-owner audit model and drifts away from the typed `Command` schema.
  Decision:
  Keep redaction engine-owned and update the risk section to match.
  Owner:
  Proposal author.

- Conflict:
  Shared auth policy is described both as a cross-crate dependency and as an MCP-local helper.
  Tradeoff:
  MCP-local wording is shorter, but it weakens the claim that GraphQL and MCP share one capability table.
  Decision:
  Make the shared `auth` crate the canonical owner and describe any transport-local code as adapters only.
  Owner:
  Proposal author.

## 7. Prioritized Action Backlog
| Priority | Item | Discipline | Owner | Horizon | Dependencies | Success Metric | Source Findings |
|---|---|---|---|---|---|---|---|
| P0 | Align the audit contract end to end: fix risk 11.4 and either specify or defer client-visible `journal_id` | Architecture | Proposal author | Before implementation handoff | none | Proposal contains one consistent audit-owner story and no implicit response-contract work | `ARCH-029-01` |
| P1 | Unify auth and capability helper ownership on the shared `control-plane/crates/auth` boundary | Architecture | Proposal author | Before implementation handoff | none | Sections 4.1, 4.2, and 6 point to the same policy owner | `ARCH-029-02` |
| P2 | Keep future review rounds anchored to refreshed artifacts, not the stale pre-R3 review | Architecture | Proposal author and reviewer | Next review round | refreshed review artifacts | No future review repeats already-closed blockers | freshness evidence in `DOC-02` |

## 8. Validation and Measurement Plan
| Area | What Will Be Measured | Leading Indicators | Guardrails | Review Checkpoint | Rollback / Hold Criteria |
|---|---|---|---|---|---|
| Audit contract | one consistent owner path for redaction and audit-pointer behavior | section 4.3, section 8, and section 11 describe the same flow | no transport-local pre-engine redaction returns | proposal update review | hold handoff if the proposal still contains conflicting audit-owner text |
| Auth boundary | one shared capability-policy owner across MCP and GraphQL | sections 4.1, 4.2, and 6 converge on the same crate or adapter chain | no duplicate policy tables or ambiguous owners | proposal update review | hold handoff if ownership still requires inference |

## 9. Evidence Gaps and Open Questions

### Evidence Gaps
- None blocking.
  Proposal text, stable references, and current code were sufficient for a defensible proposal-readiness call.

### Open Questions
- QUESTION-01: Should P029 expose `journal_id` to northbound clients now, or should that remain an internal persistence pointer until a later slice?
- QUESTION-02: Does the repo want any transport-local `auth.rs` file at all, or should all policy helpers live in `control-plane/crates/auth` and be called directly?
