# Proposal Evidence Pack

## A. Repo-Local Proposal / Document Inventory
| Evidence ID | Source / Path / Artifact | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|
| DOC-01 | `docs/proposals/048-evidence-packs-delivery-preflight-and-mcp-resolution.md` | 2026-04-15 | High | Current P048 is materially newer than the stale local review artifacts and already closes the earlier blockers around `run://{run_id}` parity and the exact GraphQL blocked-start schema shape. | A stale review would keep resolved blockers open. | Primary review target. |
| DOC-02 | `docs/proposals/048-evidence-packs-delivery-preflight-and-mcp-resolution.review/evidence-pack.md` and `.../proposal-readiness-review.md` from the previous round | 2026-04-15 | High | Existing local review artifacts are stale against the current draft. | Review output would misstate current readiness. | Freshness control. |
| DOC-03 | `.review-baselines/current-system-baseline.md` | 2026-04-15 | High | Proposal review should stay grounded in current repo reality and stable control-plane references. | Proposal could be judged against stale lineage. | Required intake. |
| DOC-04 | `docs/reference/rust-control-plane.md` | 2026-04-15 | High | Current canonical single-run MCP resource is `run://{run_id}`; P048 now matches that contract. | A stale blocker would survive after the proposal already fixed it. | MCP resource baseline. |
| DOC-05 | `docs/reference/test-gates.md` | 2026-04-15 | High | Proposal-specific gates are expected to be reproducible proof slices whose commands directly match the claimed scope. | A proposal can look more proven than its gate really is. | Proof-lane baseline. |

## B. Reusable Baseline Inputs
| Evidence ID | Artifact / Slice | Status (`Reused | Partially refreshed | Missing`) | Covered Surfaces | Verified On | Confidence | Freshness Notes | Relevance |
|---|---|---|---|---|---|---|---|
| BASE-01 | `.review-baselines/current-system-baseline.md` | Reused | overall review posture | 2026-04-15 | High | Fresh enough for intake; targeted refresh was still required for P048's MCP / GraphQL / test-gate deltas. | Review setup. |
| BASE-02 | `docs/reference/rust-control-plane.md` | Reused | MCP resource and northbound ownership | 2026-04-15 | High | Fresh for current `run://{run_id}` behavior and reader ownership. | Host-system slice. |
| BASE-03 | `docs/reference/test-gates.md` | Reused | proof-lane conventions | 2026-04-15 | High | Fresh for proposal-specific proof-slice expectations. | Test-gate baseline. |
| BASE-04 | Prior P048 review artifacts | Partially refreshed | stale findings only | 2026-04-15 | High | Reused only to separate closed blockers from still-live gaps. | Freshness boundary. |
| BASE-05 | `<proposal>.review/integration-context.md` | Missing | proposal-local reusable context | 2026-04-15 | High | No integration-context artifact exists. This did not block the round. | Not a blocker. |

## C. Scope, Out-of-Scope, and Intentional Deferrals
- In scope:
  - stage-owned failed-stage evidence and recovery truth,
  - delivery-preflight blocking, persistence, and northbound exposure,
  - execution-time MCP resolution and northbound execution truth,
  - proof-lane ownership for the slice.
- Out of scope:
  - runtime build/run validation,
  - export-pack redesign,
  - broad workflow `PreflightService`,
  - UI polish or frontend interaction design.
- Assumptions for this round:
  - current stable `docs/reference/` artifacts are the active contract,
  - current proposal text supersedes the earlier local review artifacts.
- Closed stale blockers from the prior round:
  - wrong MCP single-run resource family,
  - missing exact GraphQL blocked-start schema shape,
  - missing stage-owned `validation_failure_json` parity,
  - missing blocked-start engine/MCP transport truth.
- Live blockers discovered in this round:
  - the canonical `proposal-048|p048` proof lane is still stale and does not prove several P048 acceptance criteria,
  - the migration note hard-codes an outdated concrete slot (`008_*`) against current `HEAD`.

## D. Affected Screens / Navigation / Entry-Point Slice
| Evidence ID | Screen / Surface / Entry Point | Source (`Baseline | Targeted refresh | Proposal`) | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|---|
| NAV-01 | GraphQL `startRun` mutation | Targeted refresh | 2026-04-15 | High | Current repo still returns `Result<GqlRun>` today, but P048 now explicitly specifies `StartRunResult` for the proposed delta. | A stale review would incorrectly keep the old GraphQL-shape blocker open. | Delivery-preflight mutation surface. |
| NAV-02 | MCP single-run resource | Stable ref + targeted refresh | 2026-04-15 | High | Current canonical single-run MCP resource is `run://{run_id}`, and P048 now uses that contract consistently. | A stale blocker would misread the current draft. | Run-owned readback surface. |
| NAV-03 | Proposal-specific proof gate | Targeted refresh | 2026-04-15 | High | P048 names `proposal-048|p048` as the canonical proof lane, so the command list must match the acceptance surface it claims to prove. | A weak or stale gate can leave core claims unproven. | Review-critical handoff surface. |

## E. Impacted Modules / Code-Path Map
| Evidence ID | File Path / Module / Symbol | Layer | Role in Flow | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|---|---|
| MAP-01 | `control-plane/crates/graphql-server/src/schema.rs` | GraphQL | `startRun` mutation surface | 2026-04-15 | High | Current mutation returns `Result<GqlRun>` only; the proposal now explicitly defines the intended replacement contract. | The old GraphQL ambiguity finding is closed at proposal level. | Closed prior blocker. |
| MAP-02 | `control-plane/crates/mcp-server/src/server.rs` | MCP resources | canonical run reader | 2026-04-15 | High | Current single-run resource remains `run://{run_id}`, with `chainworks://runs/...` used for collection-family surfaces. | Confirms that the old resource-family blocker is stale. | Closed prior blocker. |
| MAP-03 | `control-plane/crates/db/migrations` | Persistence | migration-slot reality check | 2026-04-15 | High | Current `HEAD` already has `008_session_runtime_usage.sql` and `009_owner_execution_lineage.sql`. | Hard-coded `008_*` guidance in the proposal is stale. | Live handoff gap. |
| MAP-04 | `scripts/test-gate.sh` | Repo process | canonical proof-lane owner | 2026-04-15 | High | Existing gate commands show that `test_start_run_persists_delivery_configuration_json` is an older delivery-configuration proof, not a P048 delivery-preflight persistence proof. | P048 can inherit stale coverage into its canonical gate. | Live blocker. |

## F. Data / API / Persistence Contract Map
| Evidence ID | Contract / Schema Surface | Source Files | Verified On | Confidence | Current Fact | Proposal Implication | Relevance |
|---|---|---|---|---|---|---|---|
| DATA-01 | GraphQL blocked-start schema shape | P048 §2b, §2d, §3, §4 plus current GraphQL code | 2026-04-15 | High | P048 now defines an explicit `StartRunResult` union and payload shape. | The old GraphQL-shape blocker is closed. | Closed prior finding. |
| DATA-02 | MCP run-owned readback parity | P048 §2b, §2d, §3, §4 plus stable MCP reference | 2026-04-15 | High | P048 now binds persisted run-owned delivery-preflight truth to `run://{run_id}` instead of the stale `chainworks://runs/{id}` reading. | The old MCP resource-family blocker is closed. | Closed prior finding. |
| DATA-03 | Migration guidance | P048 §3 plus current migrations directory | 2026-04-15 | High | The proposal says the next free slot would be `008_*`, but current `HEAD` already contains `008_*` and `009_*`. | The migration note is a live repo-reality mismatch. | Live handoff gap. |
| DATA-04 | Proof-lane coverage | P048 §5 plus `scripts/test-gate.sh` and current engine tests | 2026-04-15 | High | The proposed gate snippet reuses `test_start_run_persists_delivery_configuration_json` and omits explicit proof for several new P048 surfaces. | The canonical proof path does not yet prove the proposal's own acceptance claims. | Live blocker. |

## G. Current Host-System Integration Surfaces
| Evidence ID | Surface / Seam / Owner | Source (`Baseline | Targeted refresh | Current repo`) | Verified On | Confidence | Key Fact | Conflict / Proposal Risk | Relevance |
|---|---|---|---|---|---|---|---|
| INT-01 | GraphQL mutation contract pattern | Current repo + proposal | 2026-04-15 | High | Current repo still uses simple mutation returns, but the proposal now explicitly defines the intended union-based delta. | No remaining blocker here at proposal level. | Closed prior gap. |
| INT-02 | MCP single-run resource contract | Stable refs + current repo + proposal | 2026-04-15 | High | `run://{run_id}` remains canonical and P048 now aligns with it. | No remaining blocker here at proposal level. | Closed prior gap. |
| INT-03 | Proposal proof-lane contract | Stable refs + current repo + proposal | 2026-04-15 | High | Proposal-specific gates are expected to be faithful proof slices, but P048's current snippet still carries stale coverage. | The proposal can hand off an invalid proving path. | Live blocker. |

## H. State Coverage Matrix
| State | Proposal Status (`Specified | Partial | Missing | Contradicted by repo | Deferred intentionally`) | Evidence IDs | Repo Touchpoints | Notes / Risks |
|---|---|---|---|---|
| Entry | Specified | DOC-01, DATA-01 | `StartRun`, `runs.start` | Entry behavior is well-specified. |
| Happy path | Specified | DOC-01, DATA-02 | run persistence + northbound readers | Core readback contract is now explicit. |
| Validation / blocked start | Specified | NAV-01, DATA-01 | GraphQL + MCP start surfaces | The old schema-shape blocker is closed. |
| Run-owned readback | Specified | NAV-02, DATA-02 | GraphQL run read, `runs.get`, `run://{run_id}` | The old MCP resource-family blocker is closed. |
| Proof / regression gate | Partial | NAV-03, MAP-04, DATA-04 | `scripts/test-gate.sh` | Canonical proof path is still stale and incomplete. |
| Migration / rollout handoff | Partial | MAP-03, DATA-03 | `db/migrations` | Concrete ordinal note is stale against current `HEAD`. |
| UI-only states | Deferred intentionally | DOC-01 | N/A | Backend proposal; no UI-state readiness gate required. |

## I. Feature Flags / Rollout / Rollback
| Evidence ID | Mechanism / Flag | Scope | Rollout Plan | Rollback Path | Verified On | Confidence | Notes |
|---|---|---|---|---|---|---|---|
| FLAG-01 | None specified | DB + northbound contract changes | Partial | Partial | 2026-04-15 | Medium | No new rollout blocker surfaced beyond the stale migration note. |

## J. Analytics / Instrumentation
| Evidence ID | Event / Signal | Purpose | Trigger Point | Verified On | Confidence | Gap / Risk |
|---|---|---|---|---|---|---|
| METRIC-01 | None specified | N/A | N/A | 2026-04-15 | Medium | No proposal-blocking instrumentation issue surfaced in this round. |

## K. Testing Strategy
| Evidence ID | Layer | Covered Surface | Current Coverage | Proposed Additions | Verified On | Confidence | Gap / Risk |
|---|---|---|---|---|---|---|---|
| TEST-01 | Proposal proof lane | delivery preflight, failed-stage evidence, execution-level MCP truth, northbound parity | P048 now declares a focused gate, but its command list still reuses a P045-era persistence test and misses direct proof for several new P048 surfaces | replace stale delivery-configuration proof and add explicit coverage for delivery-preflight readback, failed-stage evidence report readback, GraphQL execution truth, and `run://` parity | 2026-04-15 | High | Canonical proof path is still not aligned with the proposal's own acceptance criteria. |

## L. Current Repo Reality / Contradictions
| Evidence ID | Repo Surface | Proposal Claim | Current Repo Reality | Verified On | Confidence | Implication |
|---|---|---|---|---|---|---|
| REAL-01 | GraphQL blocked-start contract | P048 defines an explicit `StartRunResult` union | Current code still uses `Result<GqlRun>`, but the proposal delta is now explicit | 2026-04-15 | High | The old GraphQL-shape blocker is closed. |
| REAL-02 | MCP single-run resource | P048 uses `run://{run_id}` as the canonical run resource | Current repo and stable refs use the same resource family | 2026-04-15 | High | The old MCP resource-family blocker is closed. |
| REAL-03 | Proof gate | P048 says `proposal-048|p048` is the canonical proof lane for delivery-preflight persistence, failed-stage evidence, and northbound parity | The current proposed command list still points at an older delivery-configuration persistence test and misses direct coverage for several new P048 claims | 2026-04-15 | High | This is the main remaining blocker. |
| REAL-04 | Migration ordinal note | P048 says current concrete slot would be `008_*` | Current `HEAD` already has `008_*` and `009_*`; the next free slot is `010_*` | 2026-04-15 | High | This is a live but narrower repo-reality mismatch. |

## M. Proposal Completeness Matrix
| Dimension | Status (`Specified | Partial | Missing | Contradicted by repo | Deferred intentionally`) | Evidence IDs | Notes |
|---|---|---|---|
| Problem / user intent | Specified | DOC-01 | Problem statement is coherent and current. |
| Scope boundaries | Specified | DOC-01 | In-scope vs out-of-scope boundaries are clear. |
| Reusable baseline coverage | Specified | DOC-03, DOC-04, DOC-05 | Proposal is grounded in current stable references. |
| Navigation / entry points | Specified | NAV-01, NAV-02 | Earlier route-shape blockers are closed. |
| State handling | Specified | H matrix | Main runtime and recovery states are now adequately described. |
| Data / API contract | Specified | DATA-01, DATA-02 | Earlier GraphQL/MCP contract blockers are closed. |
| Persistence / caching | Partial | DATA-03 | Core persistence shape is clear, but the migration note is stale. |
| Feature flags / rollout / rollback | Partial | FLAG-01 | Not proposal-blocking on its own. |
| Analytics / instrumentation | Partial | METRIC-01 | Secondary only. |
| Testing strategy | Partial | TEST-01, DATA-04 | Canonical proof lane still needs tightening. |
| Dependencies / integration points | Specified | INT-01, INT-02 | Major integration contracts are now explicit. |

## N. Assumptions, Open Questions, and Blockers
- ASSUMP-01: Current proposal text is authoritative over the stale local review artifacts.
- ASSUMP-02: Proposal-specific proof lanes must directly prove the acceptance slices they claim as canonical.
- QUESTION-01: Should the migration guidance keep any concrete ordinal example, or only the invariant “next free migration ordinal at implementation time”?
- BLOCKER-01: The `proposal-048|p048` proof lane is still stale and does not prove several P048 acceptance criteria.
- BLOCKER-02: The migration note hard-codes an outdated concrete slot (`008_*`) against current `HEAD`.

## O. Research Triggers / External Questions
Not used in this round. Local proposal/docs/code/baseline evidence were sufficient for a proposal-readiness judgment.
