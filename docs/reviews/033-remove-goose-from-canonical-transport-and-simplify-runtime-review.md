# Consolidated Proposal Review

## 0. Review Mode and Proposal Evidence Summary
- Mode used: `proposal-readiness`
- Evidence completeness: `Complete`
- Proposal / docs reviewed:
  - `/Users/user/Documents/Chainworks Forge/docs/proposals/033-remove-goose-from-canonical-transport-and-simplify-runtime.md`
  - `/Users/user/Documents/Chainworks Forge/.review-baselines/current-system-baseline.md`
  - `/Users/user/Documents/Chainworks Forge/docs/reference/current-system-baseline.md`
  - `/Users/user/Documents/Chainworks Forge/docs/reference/acp-runtime-transport.md`
  - `/Users/user/Documents/Chainworks Forge/docs/reference/provider-platform.md`
  - `/Users/user/Documents/Chainworks Forge/docs/reference/provider-binding-truth.md`
  - `/Users/user/Documents/Chainworks Forge/docs/reference/test-gates.md`
  - `/Users/user/Documents/Chainworks Forge/docs/reference/README.md`
  - `/Users/user/Documents/Chainworks Forge/docs/reference/chainworks_forge_design_kit_v1.md`
  - `/Users/user/Documents/Chainworks Forge/docs/proposals/030-acp-second-wave-runtime-profiles-codex-auggie-junie_IMPLEMENTATION_AUDIT_R4.md`
- Reusable baseline used:
  - `/Users/user/Documents/Chainworks Forge/.review-baselines/current-system-baseline.md`
  - `/Users/user/Documents/Chainworks Forge/docs/reference/current-system-baseline.md`
- Baseline refreshed:
  - targeted code refresh for durable provider settings and settings-transfer persistence
  - targeted code refresh for provider-family / transport raw-value ownership
  - targeted code refresh for runtime factory fallback and frozen binding readers
  - targeted verification refresh for current repository gate ownership
- Baseline freshness: `Partially refreshed`
- External research used: `None`
- Runtime evidence used: `None`
- Current repo tensions found:
  - `P033` now tells a coherent ACP-only end state instead of compatibility-only demotion
  - current durable provider configuration still persists Goose-era family / transport raw values in user settings and settings export/import
  - current stable references and review baseline still treat Goose as authoritative across far more docs than the proposal's docs layer owns
  - no repository-owned `proposal-033` gate exists yet
  - `P030` is still red, but the proposal now correctly hard-gates implementation behind it

## 1. Executive Summary
- Overall readiness: `Red`
- Confidence: `High`
- Proposal completeness signal: `Materially improved but still not implementation-ready`
- What improved:
  1. The proposal is now conceptually consistent: it is a full Goose-removal slice, not a compatibility-only cleanup.
  2. The hard `P030` prerequisite is explicit and fail-closed.
  3. Legacy Goose-run blocking and trust-value fallback are now explicitly acknowledged.
- What still blocks `Green`:
  1. The provider-platform rewrite still lacks a durable migration contract for persisted settings, provider identifiers, and settings-transfer payloads.
  2. The docs/reference migration inventory is far too narrow for a slice that rewrites baseline truth, and the "zero Goose references" goal is broader than the intended runtime scope.
  3. The proposal-owned `proposal-033` gate is still only named, not operationally specified.

## 2. Proposal Scope and Completeness
- In scope:
  - complete Goose runtime removal
  - ACP-only transport / session / executor / provider runtime architecture
  - deletion of Goose-specific configuration and operator setup paths
  - blocking historical Goose-bound runs from resume
  - legacy trust-label fallback for historical data
- Out of scope:
  - completing `P030`
  - converting old Goose runs into ACP runs
  - runtime-heavy proof during proposal review
- External hold:
  - `P030` is still `Not Implemented / Not Ready`, so `P033` cannot start implementation yet even after proposal fixes

## 4. Discipline Scorecard
| Discipline | Readiness | Confidence | Evidence Completeness | Critical | High | Medium | Low |
|---|---|---|---|---:|---:|---:|---:|
| UI | Green | High | Complete | 0 | 0 | 0 | 0 |
| UX | Green | High | Complete | 0 | 0 | 0 | 0 |
| iOS Architecture | Red | High | Complete | 1 | 2 | 0 | 0 |

## 5. Findings by Discipline

### 5.1 iOS Architecture Findings
- Finding ID: `ARCH-033-001`
  Severity: `Critical`
  Evidence IDs: `DOC-01`, `DOC-03`, `DOC-04`, `DOC-05`, `MAP-01`, `MAP-02`, `MAP-03`, `DATA-01`, `REAL-01`
  Why it matters: The ACP-only provider rewrite still has no durable migration contract. `P033` deletes or renames Goose-era provider families and transports in [033-remove-goose-from-canonical-transport-and-simplify-runtime.md](/Users/user/Documents/Chainworks%20Forge/docs/proposals/033-remove-goose-from-canonical-transport-and-simplify-runtime.md#L105) and [033-remove-goose-from-canonical-transport-and-simplify-runtime.md](/Users/user/Documents/Chainworks%20Forge/docs/proposals/033-remove-goose-from-canonical-transport-and-simplify-runtime.md#L124), but current durable settings persist those exact values in [ConfiguredProvider.swift](/Users/user/Documents/Chainworks%20Forge/Chainworks%20Forge/Providers/ConfiguredProvider.swift#L3), [ConfiguredProvider.swift](/Users/user/Documents/Chainworks%20Forge/Chainworks%20Forge/Providers/ConfiguredProvider.swift#L118), [ConfiguredProvider.swift](/Users/user/Documents/Chainworks%20Forge/Chainworks%20Forge/Providers/ConfiguredProvider.swift#L185), [ProviderSettings.swift](/Users/user/Documents/Chainworks%20Forge/Chainworks%20Forge/Providers/ProviderSettings.swift#L3), [ProviderSettingsStore.swift](/Users/user/Documents/Chainworks%20Forge/Chainworks%20Forge/Providers/ProviderSettingsStore.swift#L129), and [SettingsTransferService.swift](/Users/user/Documents/Chainworks%20Forge/Chainworks%20Forge/Support/SettingsTransferService.swift#L3). The reusable baseline still says the current MVP provider families are `codex`, `claude_code`, and `gemini` in [current-system-baseline.md](/Users/user/Documents/Chainworks%20Forge/.review-baselines/current-system-baseline.md#L42) and [current-system-baseline.md](/Users/user/Documents/Chainworks%20Forge/docs/reference/current-system-baseline.md#L91). As written, the proposal leaves implementers guessing how `provider-settings.json`, `chainworks-settings.json`, seeded env defaults, `preferredProviderIDsByFamily`, and YAML/provider identifiers migrate without corrupting user state.
  Recommended fix: Add one migration table for the provider platform that locks the post-`P033` canonical provider identifier vocabulary, the `ProviderFamily` / `ProviderTransport` end state, `provider-settings.json` migration rules, settings-transfer schema/version handling, seeded-env remapping, and preferred-provider key migration. The proposal must say whether YAML `provider:` values stay `codex` / `claude_code` / `gemini` or change to ACP-specific identifiers, and how readers accept old persisted values.
  Acceptance criteria:
  - post-`P033` canonical provider identifiers are explicit for YAML, settings, and frozen bindings
  - `ProviderSettingsStore` and `SettingsTransferService` migration behavior is explicit and testable
  - `preferredProviderIDsByFamily` raw-key remapping is explicit
  - no implementer has to invent fallback behavior for old persisted provider families or transports
  Confidence: `High`

- Finding ID: `ARCH-033-002`
  Severity: `High`
  Evidence IDs: `DOC-01`, `DOC-03`, `DOC-04`, `DOC-05`, `DOC-07`, `DOC-08`, `DOC-09`, `MAP-04`, `REAL-02`
  Why it matters: The docs layer is still drastically under-scoped for a proposal that changes the baseline architecture. `P033` only owns four doc changes in [033-remove-goose-from-canonical-transport-and-simplify-runtime.md](/Users/user/Documents/Chainworks%20Forge/docs/proposals/033-remove-goose-from-canonical-transport-and-simplify-runtime.md#L178), but current stable refs and the review baseline still embed Goose as implemented truth across [current-system-baseline.md](/Users/user/Documents/Chainworks%20Forge/docs/reference/current-system-baseline.md#L22), [README.md](/Users/user/Documents/Chainworks%20Forge/docs/reference/README.md#L19), [acp-runtime-transport.md](/Users/user/Documents/Chainworks%20Forge/docs/reference/acp-runtime-transport.md#L120), [provider-platform.md](/Users/user/Documents/Chainworks%20Forge/docs/reference/provider-platform.md#L246), plus many adjacent refs and test-doc surfaces. Separately, the proposal goal says "Zero Goose references in the codebase" in [033-remove-goose-from-canonical-transport-and-simplify-runtime.md](/Users/user/Documents/Chainworks%20Forge/docs/proposals/033-remove-goose-from-canonical-transport-and-simplify-runtime.md#L9), but the design authority intentionally uses geese as the product metaphor in [chainworks_forge_design_kit_v1.md](/Users/user/Documents/Chainworks%20Forge/docs/reference/chainworks_forge_design_kit_v1.md#L7). Without an explicit keep/delete/rewrite matrix, the repository baseline can end up self-contradictory even if code compiles.
  Recommended fix: Add a stable-doc migration matrix covering the reusable baseline, reference index, transport/runtime docs, provider/remediation docs, test-gate docs, and any proposal-promoted references that currently name Goose. Also narrow the goal from "zero Goose references in the codebase" to the real intended boundary: zero Goose runtime/transport/operator-remediation references, while explicitly preserving brand-metaphor docs unless they are intentionally rebranded too.
  Acceptance criteria:
  - every authoritative Goose-bearing reference doc is marked `rewrite`, `delete`, or `retain intentionally`
  - `.review-baselines/current-system-baseline.md` and `docs/reference/current-system-baseline.md` are explicitly owned
  - `docs/reference/README.md` is included in the migration scope
  - the proposal's goal wording is narrowed to a testable runtime/docs scope
  Confidence: `High`

- Finding ID: `ARCH-033-003`
  Severity: `High`
  Evidence IDs: `DOC-01`, `DOC-08`, `MAP-05`, `REAL-03`
  Why it matters: `P033` now depends on a proposal-owned proof lane in [033-remove-goose-from-canonical-transport-and-simplify-runtime.md](/Users/user/Documents/Chainworks%20Forge/docs/proposals/033-remove-goose-from-canonical-transport-and-simplify-runtime.md#L29) and [033-remove-goose-from-canonical-transport-and-simplify-runtime.md](/Users/user/Documents/Chainworks%20Forge/docs/proposals/033-remove-goose-from-canonical-transport-and-simplify-runtime.md#L223), but there is still no repository `proposal-033` gate in [test-gates.md](/Users/user/Documents/Chainworks%20Forge/docs/reference/test-gates.md#L1) or `scripts/test-gate.sh`. For a slice that removes transport families, runtime configuration, provider setup flows, and stable-reference truth together, "gate passes with P030 prerequisite" is not enough: the proposal still does not say which suites prove settings migration, provider resolution, legacy Goose-run blocking, transport removal, or docs/reference fallout.
  Recommended fix: Define the exact `proposal-033` gate composition, including named suite groups, any remote/local host policy, and the evidence outputs required to prove provider-settings migration, ACP-only transport resolution, legacy-run block behavior, and stable-doc/gate update completion.
  Acceptance criteria:
  - `proposal-033` names concrete suite groups or test targets
  - the gate includes durable provider-settings migration proof, ACP-only runtime selection proof, and historical Goose-run blocking proof
  - the gate owns explicit evidence outputs, not only a passing command name
  Confidence: `High`

## 6. Cross-Discipline Conflicts and Decisions
- Conflict: the proposal wants a clean ACP-only provider vocabulary, but the current durable configuration model still serializes Goose-era family and transport identifiers.
  Tradeoff: skipping migration would simplify the proposal text, but it would make settings/import/export breakage an implementation-time surprise.
  Decision: lock the durable provider/settings migration contract inside `P033` instead of assuming code-level discovery later.
  Owner: proposal author

- Conflict: the proposal wants complete Goose removal, but the repository baseline currently treats many Goose-bearing reference docs as authoritative and the design kit uses geese as the brand metaphor.
  Tradeoff: a slogan like "zero Goose references" is rhetorically clean, but it is not operationally precise enough for this repo.
  Decision: narrow the removal target to runtime/transport/operator-remediation truth and add a doc migration matrix for all authoritative references.
  Owner: proposal author

- Conflict: the proposal now names a hard proof gate, but repository-owned verification has not caught up.
  Tradeoff: leaving the gate abstract keeps the proposal shorter, but it makes acceptance `12` non-auditable.
  Decision: make `proposal-033` a concrete repository lane before claiming the slice is implementation-ready.
  Owner: proposal author

## 7. Prioritized Action Backlog
| Priority | Item | Discipline | Owner | Horizon | Dependencies | Success Metric | Source Findings |
|---|---|---|---|---|---|---|---|
| P0 | Add the durable provider-platform migration contract: provider identifiers, persisted settings, settings transfer, preferred-family keys, and env/default seeding | iOS Architecture | Proposal author | Before implementation | current provider platform baseline | implementers can migrate settings without inventing schema behavior | `ARCH-033-001` |
| P0 | Expand the docs/reference migration into a full authoritative matrix and narrow the "zero Goose references" goal to runtime/transport truth | iOS Architecture | Proposal author | Before implementation | current stable refs + baseline | no baseline reference remains implicitly stale after `P033` lands | `ARCH-033-002` |
| P1 | Define the exact `proposal-033` repository gate and expected evidence outputs | iOS Architecture | Proposal author | Before implementation | `P030` prerequisite semantics | acceptance `12` becomes operationally auditable | `ARCH-033-003` |

## 8. Validation and Measurement Plan
| Area | What Will Be Measured | Leading Indicators | Guardrails | Review Checkpoint | Rollback / Hold Criteria |
|---|---|---|---|---|---|
| Provider/platform migration | settings and provider resolution survive Goose-family removal | explicit migration table and schema/version plan | no silent loss of imported or persisted provider settings | next rereview of `P033` | hold if provider identifiers or settings migration remain implicit |
| Stable reference migration | reference layer remains self-consistent after Goose removal | explicit keep/rewrite/delete matrix | no authoritative baseline doc remains silently stale | next rereview of `P033` | hold if docs layer still names only a narrow subset |
| Focused proof gate | proposal-specific verification becomes executable | named `proposal-033` suites and evidence outputs | no acceptance criterion depends on an abstract gate | next rereview of `P033` | hold if `proposal-033` is still only conceptual |
| External dependency | `P030` readiness | `P030` audit turns green | `P033` implementation does not start early | next rereview of `P033` | hold if `P030` remains red |

## 9. Evidence Gaps and Open Questions

### Evidence Gaps
- GAP-01: No blocking evidence gap remains. The review has enough local proposal/docs/code/baseline evidence; the remaining issues are proposal-text and dependency-state issues.

### Open Questions
- QUESTION-01: what are the canonical post-`P033` provider identifiers in YAML and settings: keep `codex` / `claude_code` / `gemini`, or rename them to ACP-specific identifiers?
- QUESTION-02: how exactly do `provider-settings.json` and `chainworks-settings.json` migrate when `ProviderFamily` and `ProviderTransport` change?
- QUESTION-03: which Goose-bearing reference docs are intentionally retained because they describe product branding or historical evidence rather than live runtime architecture?
- QUESTION-04: what exact suites and proof artifacts make up `proposal-033`?

## 10. Evidence Gap Review Fallback

Not used. Evidence completeness is `Complete`.
