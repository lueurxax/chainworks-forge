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
- Baseline freshness: `Partially refreshed`
- External research used: `None`
- Runtime evidence used: `None`
- Current repo tensions found:
  - the previous stale findings about missing docs coverage, missing `SettingsTransferService` proof, operator-facing Goose wording, and `gooseSessionID` ownership are now closed in the proposal text
  - deleting old Codex rows still breaks credential continuity unless the proposal owns UUID/keychain remapping
  - the provider-vocabulary migration still stops short of some canonical boundary owners (`runtime-contract`, `mvp-sign-off`, `MVPBoundaryPolicy`)
  - `P030` remains red, so implementation is still operationally blocked behind the proposal's own prerequisite gate

## 1. Executive Summary
- Overall readiness: `Red`
- Confidence: `High`
- Proposal completeness signal: `Substantially improved, but still not implementation-safe`
- What improved:
  1. The proposal now explicitly owns `SettingsTransferService` proof, neutral legacy operator wording, and persistent-model renaming for `runtimeSessionID`.
  2. The earlier findings about docs-table gaps, proof-lane gaps, operator-string contradiction, and missing `gooseSessionID` ownership are now stale and should not be reused.
- What still blocks `Green`:
  1. Codex-row deletion still has no secret/keychain continuity contract even though current secrets are keyed by provider UUID.
  2. The provider-vocabulary migration to `*_acp` still does not classify all canonical MVP boundary owners.

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
| iOS Architecture | Red | High | Complete | 1 | 1 | 0 | 0 |

## 5. Findings by Discipline

### 5.1 iOS Architecture Findings
- Finding ID: `ARCH-033-001`
  Severity: `Critical`
  Evidence IDs: `DOC-01`, `MAP-01`, `MAP-02`, `DATA-01`, `REAL-01`
  Why it matters: The updated `3.6a` now deletes every old `.codex` provider row and replaces it with a fresh seeded `.codexACP` row. But current secret storage and settings-transfer placeholders are keyed by `provider.id`, not by provider family. [ProviderAdapter.swift](/Users/user/Documents/Chainworks%20Forge/Chainworks%20Forge/Providers/ProviderAdapter.swift#L231) derives the secret key as `provider.<uuid>`, and [SettingsTransferService.swift](/Users/user/Documents/Chainworks%20Forge/Chainworks%20Forge/Support/SettingsTransferService.swift#L46) exports/imports placeholder lists using that same key. If migration deletes the old Codex row and generates a new UUID for seeded `.codexACP`, the existing local secret and imported placeholder mapping no longer belong to the surviving row. The proposal currently says cross-machine continuity is preserved, but it does not define any secret remap or operator remediation for this UUID break.
  Recommended fix: extend `3.6a` with an explicit Codex credential-continuity contract. Either:
  1. migrate the old Codex row into `.codexACP` while preserving `id` specifically so the secret key remains valid, or
  2. keep delete-and-reseed behavior, but explicitly state that Codex credentials are invalidated and the operator must re-enter them, with proof and UX copy updated accordingly.
  Acceptance criteria:
  - the proposal explicitly states what happens to the old Codex row UUID and its secret key
  - `SettingsTransferService` placeholder continuity is either preserved or intentionally remediated
  - Codex migration does not silently strand valid credentials behind an orphaned provider UUID
  Confidence: `High`

- Finding ID: `ARCH-033-002`
  Severity: `High`
  Evidence IDs: `DOC-01`, `DOC-02`, `DOC-03`, `MAP-03`, `REAL-02`
  Why it matters: The provider-vocabulary migration is still under-owned across canonical boundary docs and policy code. `3.6a` now changes YAML/provider identifiers from `codex / claude_code / gemini` to `codex_acp / claude_acp / gemini_acp`, but current stable boundary owners still freeze the old set in [runtime-contract.md](/Users/user/Documents/Chainworks%20Forge/docs/reference/runtime-contract.md#L105), [mvp-sign-off.md](/Users/user/Documents/Chainworks%20Forge/docs/reference/mvp-sign-off.md#L49), and [MVPBoundaryPolicy.swift](/Users/user/Documents/Chainworks%20Forge/Chainworks%20Forge/Support/MVPBoundaryPolicy.swift#L9). The docs layer currently updates `current-system-baseline.md`, but not these boundary owners. That leaves MVP sign-off, runtime-boundary documentation, and policy code inconsistent with the proposal's new provider vocabulary.
  Recommended fix: expand the docs/policy fallout to explicitly classify `runtime-contract.md`, `mvp-sign-off.md`, and `MVPBoundaryPolicy.swift` as part of the provider-vocabulary migration. If the proposal intentionally defers those owners, it must say so and narrow the vocabulary change instead of implying repo-wide canonical replacement.
  Acceptance criteria:
  - all canonical MVP provider-boundary owners are explicitly classified
  - sign-off, runtime-contract, and policy-code provider sets cannot remain on `codex / claude_code / gemini` after `P033`
  - the proposal is clear whether `claude_acp / gemini_acp` are the new canonical MVP provider identifiers or only an internal migration step
  Confidence: `High`

## 6. Cross-Discipline Conflicts and Decisions
- Conflict: deleting `.codex` rows gives a cleaner break than preserving family identity, but current secrets and transfer placeholders are UUID-bound.
  Tradeoff: delete-and-reseed simplifies family cleanup, but it silently breaks credential continuity unless the proposal owns remediation.
  Decision: the proposal must explicitly choose between preserving the old UUID or requiring explicit Codex credential re-entry.
  Owner: proposal author

- Conflict: the proposal changes canonical provider identifiers, but some stable boundary docs and policy code still freeze the old set.
  Tradeoff: limiting the fallout list keeps the proposal shorter, but it leaves canonical MVP boundary owners inconsistent.
  Decision: expand the owned fallout or narrow the vocabulary change.
  Owner: proposal author

## 7. Prioritized Action Backlog
| Priority | Item | Discipline | Owner | Horizon | Dependencies | Success Metric | Source Findings |
|---|---|---|---|---|---|---|---|
| P0 | Add explicit Codex UUID/secret continuity contract to `3.6a` | iOS Architecture | Proposal author | Before implementation | current keychain + transfer placeholder design | migration cannot silently orphan Codex credentials | `ARCH-033-001` |
| P1 | Expand provider-vocabulary fallout to `runtime-contract.md`, `mvp-sign-off.md`, and `MVPBoundaryPolicy.swift` | iOS Architecture | Proposal author | Before implementation | current canonical MVP boundary owners | canonical provider boundary is consistent after `P033` | `ARCH-033-002` |

## 8. Validation and Measurement Plan
| Area | What Will Be Measured | Leading Indicators | Guardrails | Review Checkpoint | Rollback / Hold Criteria |
|---|---|---|---|---|---|
| Codex migration continuity | old Codex credentials remain usable or are explicitly remediated | explicit UUID/secret migration rule | no silent orphaned keychain secrets or placeholder drift | next rereview of `P033` | hold if Codex UUID/secret behavior remains implicit |
| Provider boundary consistency | all canonical boundary owners converge on the same provider identifiers | fallout table includes docs + policy owners | no stale `codex / claude_code / gemini` boundary survives in canonical MVP surfaces | next rereview of `P033` | hold if provider-vocabulary ownership remains partial |
| External dependency | `P030` readiness | `P030` audit turns green | `P033` implementation does not start early | next rereview of `P033` | hold if `P030` remains red |

## 9. Evidence Gaps and Open Questions

### Evidence Gaps
- GAP-01: No blocking evidence gap remains. The remaining issues are proposal-text closure issues, not missing local evidence.

### Open Questions
- QUESTION-01: should Codex migration preserve the old provider UUID specifically to preserve keychain continuity?
- QUESTION-02: are `codex_acp / claude_acp / gemini_acp` intended to replace the canonical MVP provider boundary everywhere, including sign-off and policy code?

## 10. Evidence Gap Review Fallback

Not used. Evidence completeness is `Complete`.
