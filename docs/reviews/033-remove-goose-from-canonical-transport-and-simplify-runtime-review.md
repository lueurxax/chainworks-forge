# Consolidated Proposal Review

## 0. Review Mode and Proposal Evidence Summary
- Mode used: `proposal-readiness`
- Evidence completeness: `Complete`
- Proposal / docs reviewed:
  - `/Users/user/Documents/Chainworks Forge/docs/proposals/033-remove-goose-from-canonical-transport-and-simplify-runtime.md`
  - `/Users/user/Documents/Chainworks Forge/.review-baselines/current-system-baseline.md`
  - `/Users/user/Documents/Chainworks Forge/docs/reference/current-system-baseline.md`
  - `/Users/user/Documents/Chainworks Forge/docs/reference/runtime-contract.md`
  - `/Users/user/Documents/Chainworks Forge/docs/reference/mvp-sign-off.md`
  - `/Users/user/Documents/Chainworks Forge/docs/reference/domain-model.md`
  - `/Users/user/Documents/Chainworks Forge/docs/reference/provider-platform.md`
  - `/Users/user/Documents/Chainworks Forge/docs/reference/README.md`
  - `/Users/user/Documents/Chainworks Forge/docs/proposals/030-acp-second-wave-runtime-profiles-codex-auggie-junie_IMPLEMENTATION_AUDIT_R4.md`
- Reusable baseline used:
  - `/Users/user/Documents/Chainworks Forge/.review-baselines/current-system-baseline.md`
  - `/Users/user/Documents/Chainworks Forge/docs/reference/current-system-baseline.md`
- Baseline refreshed:
  - targeted code refresh for provider UUID/secret storage coupling
  - targeted code refresh for frozen MVP provider boundary ownership
  - targeted doc refresh for runtime/sign-off boundary docs
  - targeted proof-lane refresh for proposal numbering and gate ownership
- Baseline freshness: `Partially refreshed`
- External research used: `None`
- Runtime evidence used: `None`
- Current repo tensions found:
  - the previous stale findings about missing docs coverage, missing `SettingsTransferService` proof, operator-facing Goose wording, `gooseSessionID` ownership, Codex UUID continuity, and provider-boundary fallout are now closed in the proposal text
  - the proof lane still says transfer-path “cross-machine continuity preserved” even though `3.6a` now explicitly requires Codex re-auth and drops Codex placeholders
  - the prerequisite gate still mixes `P030` dependency language with the historical `proposal-029` lane name without explaining whether that alias is intentional
  - `P030` remains red, so implementation is still operationally blocked behind the proposal's own prerequisite gate

## 1. Executive Summary
- Overall readiness: `Amber`
- Confidence: `High`
- Proposal completeness signal: `Substantially improved and close to handoff, but not yet internally consistent`
- What improved:
  1. The proposal now explicitly owns `SettingsTransferService` proof, neutral legacy operator wording, persistent-model renaming for `runtimeSessionID`, Codex re-auth semantics, and the missing provider-boundary fallout (`runtime-contract`, `mvp-sign-off`, `MVPBoundaryPolicy.swift`).
  2. The earlier findings about docs-table gaps, proof-lane gaps, operator-string contradiction, missing `gooseSessionID` ownership, missing Codex continuity semantics, and missing provider-boundary owners are now stale and should not be reused.
- What still blocks `Green`:
  1. The proof lane still over-claims transfer continuity after the proposal switched Codex to explicit re-auth semantics.
  2. The prerequisite gate contract still mixes `P030` and `proposal-029` naming without locking whether the alias is intentional.

## 2. Proposal Scope and Completeness
- In scope:
  - complete Goose runtime removal
  - ACP-only transport / session / executor / provider runtime architecture
  - durable settings migration for provider/platform state
  - historical Goose-run blocking and trust fallback
  - stable-reference migration and proof-gate ownership
- Out of scope:
  - completing `P030`
  - converting old Goose runs into ACP runs
  - runtime-heavy proof during proposal review
- External hold:
  - `P030` is still `Not Implemented / Not Ready`, so implementation cannot start yet; this is an operational hold, not the main proposal-text blocker for this pass

## 4. Discipline Scorecard
| Discipline | Readiness | Confidence | Evidence Completeness | Critical | High | Medium | Low |
|---|---|---|---|---:|---:|---:|---:|
| UI | Green | High | Complete | 0 | 0 | 0 | 0 |
| UX | Green | High | Complete | 0 | 0 | 0 | 0 |
| iOS Architecture | Amber | High | Complete | 0 | 1 | 1 | 0 |

## 5. Findings by Discipline

### 5.1 iOS Architecture Findings
- Finding ID: `ARCH-033-001`
  Severity: `High`
  Evidence IDs: `DOC-01`, `DATA-01`, `REAL-01`
  Why it matters: The proposal now makes a clear decision in `3.6a`: Claude/Gemini preserve UUID and continuity, while Codex is deleted and requires explicit re-auth ([033-remove-goose-from-canonical-transport-and-simplify-runtime.md](/Users/user/Documents/Chainworks%20Forge/docs/proposals/033-remove-goose-from-canonical-transport-and-simplify-runtime.md#L256)). But the proof lane still says `SettingsTransferService` import proves “cross-machine continuity preserved” for the migration as a whole ([033-remove-goose-from-canonical-transport-and-simplify-runtime.md](/Users/user/Documents/Chainworks%20Forge/docs/proposals/033-remove-goose-from-canonical-transport-and-simplify-runtime.md#L61)). Those two statements no longer match: Codex continuity is intentionally not preserved. That leaves the gate ambiguous about what must be asserted and makes the current proof wording technically false if read literally.
  Recommended fix: rewrite proof item `2` so it matches the new migration semantics. It should say continuity is preserved for in-place Claude/Gemini migrations, while deleted Codex rows require explicit re-auth and dropped placeholders. Acceptance/proof wording should validate both halves of that contract.
  Acceptance criteria:
  - proof wording no longer claims universal cross-machine continuity after Codex was moved to explicit re-auth
  - `Proposal033Tests` explicitly proves Claude/Gemini continuity and Codex re-auth/remediation as separate outcomes
  - transfer-path behavior and operator expectations use the same vocabulary as `3.6a`
  Confidence: `High`

- Finding ID: `ARCH-033-002`
  Severity: `Medium`
  Evidence IDs: `DOC-01`, `MAP-02`, `REAL-02`
  Why it matters: The proposal now correctly depends on `P030`, but its proof-lane snippet still invokes `proposal-029-prereq` and `${PROPOSAL_029_TESTS[@]}` ([033-remove-goose-from-canonical-transport-and-simplify-runtime.md](/Users/user/Documents/Chainworks%20Forge/docs/proposals/033-remove-goose-from-canonical-transport-and-simplify-runtime.md#L50)). Current repo reality also still exposes only `proposal-029` in [scripts/test-gate.sh](/Users/user/Documents/Chainworks%20Forge/scripts/test-gate.sh#L1370), even though the proposal file and current implementation audits are all `030-*`. That leaves the prerequisite contract ambiguous: is `proposal-029` a historical alias for `P030`, or is the proposal pointing at the wrong gate name?
  Recommended fix: make the gate naming explicit. Either:
  1. keep the repo-owned gate alias as `proposal-029` and state that `P030` currently reuses that historical lane name, or
  2. rename the lane and snippet to `proposal-030` / `PROPOSAL_030_TESTS` everywhere.
  Acceptance criteria:
  - the prerequisite gate uses one stable identifier across proposal text, repo script, and audit references
  - rereviewers do not have to infer whether `proposal-029` is a historical alias for `P030`
  - the proof lane can be executed unambiguously from the proposal alone
  Confidence: `High`

## 6. Cross-Discipline Conflicts and Decisions
- Conflict: the proposal now explicitly chooses Codex re-auth, but the proof lane still uses continuity language from the older migration model.
  Tradeoff: retaining the old proof sentence makes the gate look simpler, but it no longer matches the actual migration semantics.
  Decision: update the proof lane to assert two different outcomes: preserved continuity for migrated rows and explicit re-auth for deleted Codex rows.
  Owner: proposal author

- Conflict: the proposal depends on `P030`, but the repo-owned gate lane still carries the historical `proposal-029` name.
  Tradeoff: keeping the old alias avoids a repo-wide gate rename, but it makes the proposal's prerequisite contract ambiguous unless that alias is spelled out.
  Decision: either lock the alias explicitly or rename the lane end-to-end.
  Owner: proposal author

## 7. Prioritized Action Backlog
| Priority | Item | Discipline | Owner | Horizon | Dependencies | Success Metric | Source Findings |
|---|---|---|---|---|---|---|---|
| P0 | Align proof item `2` with the new Codex re-auth semantics in `3.6a` | iOS Architecture | Proposal author | Before implementation | updated migration contract | transfer proof matches the actual Codex/Claude/Gemini outcomes | `ARCH-033-001` |
| P1 | Resolve `P030` vs `proposal-029` prerequisite-gate naming | iOS Architecture | Proposal author | Before implementation | current repo gate lane naming | prerequisite proof can be followed without alias guesswork | `ARCH-033-002` |

## 8. Validation and Measurement Plan
| Area | What Will Be Measured | Leading Indicators | Guardrails | Review Checkpoint | Rollback / Hold Criteria |
|---|---|---|---|---|---|
| Transfer migration truth | proof lane matches the actual row-by-row migration semantics | separate assertions for migrated rows and deleted Codex rows | no generic “continuity preserved” claim survives when Codex requires re-auth | next rereview of `P033` | hold if proof wording still over-claims continuity |
| Prerequisite gate clarity | proposal and repo use one stable name for the second-wave ACP prereq | gate snippet and repo script converge | no rereviewer has to infer whether `proposal-029` means `P030` | next rereview of `P033` | hold if gate naming remains mixed |
| External dependency | `P030` readiness | `P030` audit turns green | `P033` implementation does not start early | next rereview of `P033` | hold if `P030` remains red |

## 9. Evidence Gaps and Open Questions

### Evidence Gaps
- GAP-01: No blocking evidence gap remains. The remaining issues are proposal-text closure issues, not missing local evidence.

### Open Questions
- QUESTION-01: should the proof lane say “continuity preserved for migrated rows” instead of “cross-machine continuity preserved” to match the new Codex re-auth rule?
- QUESTION-02: is `proposal-029` an intentional long-lived repo alias for `P030`, or should the gate be renamed in the same slice?

## 10. Evidence Gap Review Fallback

Not used. Evidence completeness is `Complete`.
