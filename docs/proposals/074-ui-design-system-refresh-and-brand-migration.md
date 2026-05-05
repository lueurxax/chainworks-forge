# Proposal 074: UI Design System Refresh and Brand Migration

| Field | Value |
|---|---|
| Date | 2026-04-26 |
| Status | Draft |
| Author | Engineering + Design-led implementation |
| Depends on | Proposal 031, Proposal 068, [UI action boundary](../reference/ui-action-boundary.md) |
| Target State | Native macOS SwiftUI client over GraphQL read projections + MCP control |
| Goal | Migrate the Chainworks Forge operator app from the previous visual shell to the new `docs/brand` design system, with Apple HIG-aligned tokens, copy, and surfaces while preserving existing product behaviour and control boundaries. |

---

## 1. Why this proposal exists

`docs/brand/` now contains a refreshed identity system (`colors_and_type.css`, brand mark/logo assets, ui_kits, and status/iconography guidance). The application should consume this as canonical design truth so operators get one coherent visual language across:

- shell/background surfaces,
- run / stage / approval / artifact views,
- status/attention patterns,
- onboarding, banners, and fallback diagnostics.

Without a dedicated proposal, UI implementation work remains ad hoc and visually drifted from the new design authority.

---

## 2. Principles

1. **Brand-first shell, operation-first content**: visuals must support the run lifecycle, not override it.
2. **Apple HIG semantics dominate** for interaction affordances and system colors.
3. **Orange is accent-only** (`#F59A2B`); do not repurpose it as generic status color.
4. **Status truth remains canonical**:
   - Pending, Ready, Running, Awaiting approval, Blocked, Completed, Failed, Cancelling, Cancelled.
5. **SwiftUI GraphQL boundary unchanged**: no mutation model changes in this proposal; only visual and content presentation updates.

---

## 3. Scope of work

### In scope

1. Apply `docs/brand` visual primitives to the macOS app UI layer:
   - palette mapping to `bg/canvas/elevated/label/separator/status`,
   - typography ramp for headings/cards/body/mono,
   - spacing/radius/shadow/elevation tiers,
   - status capsule and banner style harmonization.
2. Update App surfaces to the brand tone:
   - Runs home/tile hierarchy,
   - run detail and timeline,
   - artifact and report lists,
   - approvals surfaces,
   - daemon/workspace/system banners.
3. Standardize iconography usage (SF Symbols / existing bridge layer) and remove emoji-like replacements.
4. Introduce brand assets in-app where appropriate (hero/logo mark on non-operational surfaces only).
5. Align operator copy to the brand voice: concise, deterministic, no hype, no emoji, no invented status phrasing.
6. Add/adjust motion tokens for reveal and attention transitions:
   - micro 150ms,
   - standard 250ms,
   - sheet/overlay 400ms,
   - reduced-motion fallback.

### Out of scope

1. New workflows, API contracts, orchestration paths.
2. Navigation rewiring outside style refactor.
3. New feature flags unrelated to design refresh.
4. Backend/daemon logic and proposal boundary routing changes (covered by other proposals).

---

4. Canonical brand artifact references

- `docs/brand/README.md`
- `docs/brand/SKILL.md`
- `docs/brand/colors_and_type.css`
- `docs/brand/assets/*`
- `docs/brand/preview/*`
- `docs/brand/ui_kits/macos/*`

---

## 5. Required implementation outcomes

### 5.1 Foundation pass

1. Centralize reusable design tokens (`Color`, `Font`, `Spacing`, `Radius`, `Shadow`, `Animation`, `Status`) in the app layer and align them with `docs/brand`.
2. Replace scattered legacy hardcoded literals with token references.
3. Document fallback behavior for non-supported environments.

### 5.2 Surface pass

1. Re-skin:
   - `RunsHomeView`,
   - `RunDetailView`/timeline views,
   - artifact/report readers,
   - approval queue surfaces,
   - daemon lifecycle and diagnostics surfaces.
2. Ensure stateful surfaces preserve contrast and emphasis:
   - status, warning, attention, disabled, and loading states.
3. Ensure layout spacing/typography reduces visual noise and supports fast scan of run lineage.

### 5.3 Content/interaction pass

1. Normalize punctuation and copy style:
   - no exclamation marks in product banners,
   - sentence case labels,
   - monospace for IDs/paths/codes.
2. Remove non-standard icon glyph substitutions in UI.
3. Keep user actions visually distinct from status/progress readback.

### 5.4 Verification pass

1. Manual visual smoke on light and dark surfaces.
2. Diff-based review comparing old/new screenshot references (pre-change saved snapshots optional).
3. Accessibility checks for hierarchy and contrast on the changed screens.

---

## 6. Acceptance criteria

1. Token usage is centralized and references `docs/brand` assets/standards.
2. The app UI on both themes demonstrates:
   - brand-consistent palette and typography,
   - readable run lifecycle surfaces,
   - canonical status truth copy.
3. No status/action regressions due to styling changes (functional behavior unchanged).
4. No UI string uses emoji for product state.
5. Motion timings and easing match the approved profile and reduce without abrupt pops.

---

## 7. Risks

- **Drift risk**: partial adoption can create hybrid visuals; mitigate by central token ownership.
- **Contrast risk**: low-contrast status surfaces on dark backgrounds; mitigate with explicit status tokens and verification passes.
- **Over-styling risk**: operator clarity lost behind visual ornamentation; mitigate by keeping semantic hierarchy first.

---

## 8. Completion signal

This proposal is complete when the app design system migration is merged behind a stable branch point and the run-oriented operator screens pass visual review against the brand guide.
