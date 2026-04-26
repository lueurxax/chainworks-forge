# Chainworks Forge — Design System

**Apple-HIG-aligned redesign** of the Chainworks Forge visual lane. Forge is a
local-first, macOS SwiftUI control plane for agent-driven engineering work; this
design system unifies the existing `Forge*` token primitives (`ForgeColor`,
`ForgeTypography`, `ForgeSpacing`, `ForgeRadius`, `ForgeStatusColor`,
`ForgePanel`, `StatusCapsule`, …) with the Apple Human Interface Guidelines
type ramp, color tokens, materials, and corner-radius continuum.

> The product is opinionated: **the primary object is not a chat thread — it is
> a Run.** A run captures one idea, freezes a workflow snapshot, dispatches it
> through specialised agents, pauses at approval gates, and leaves behind
> durable artifacts. The visual system has to make Run, Stage, Approval, and
> Artifact truth legible at a glance — that is what this redesign optimises for.

---

## Sources

This system was distilled from the upstream codebase. None of the links assume
the reader has access; they are for traceability if you do.

| Source | Path |
| --- | --- |
| Repository | `lueurxax/chainworks-forge` (default branch `main`) |
| README + product positioning | `README.md` |
| Token authority (Swift) | `Chainworks Forge/Support/Design/Forge*.swift` |
| Compatibility facade | `Chainworks Forge/Support/DesignTokens.swift` |
| Status capsule | `Chainworks Forge/Support/StatusCapsule.swift` |
| Operator surfaces | `Chainworks Forge/Views/*.swift` |
| Brand reference doc | `docs/reference/design-system-and-brand-application.md` |
| Brand renders | `docs/brand/render/*` and `docs/brand/from-sheet/*` |
| App icon set | `Chainworks Forge/Assets.xcassets/AppIcon.appiconset/` |

The brand SVG primitives (orbit gradient, three-bird flock, wordmark) are
captured in `docs/brand/render/chainworks-forge-logo-horizontal.html` and
`chainworks-forge-readme-hero.html`. We re-render them as React components in
`ui_kits/brand/` to keep the system editable.

---

## Index

| File | What's in it |
| --- | --- |
| `README.md` | This document — context, content fundamentals, visual foundations, iconography. |
| `SKILL.md` | Agent-Skills entrypoint: load this first. |
| `colors_and_type.css` | All design tokens (semantic colors + Apple HIG type ramp + Forge legacy facade). |
| `assets/` | Brand mark, horizontal logo (light + dark), brand hero, app icon master. |
| `preview/` | Per-card design-system specimens that render in the Design System tab. |
| `ui_kits/macos/` | UI kit recreating the Forge SwiftUI app (Runs Home, Run Timeline, Approvals, etc.) as React. |
| `ui_kits/brand/` | Brand wordmark, three-geese flock, app icon as inline SVG components. |

---

## Content Fundamentals

Forge writes like a precise, local-first developer tool. There is no marketing
fluff and no second-person hype. Voice is **operator-to-operator**.

- **Person.** Mostly _it_/passive (“workflows pause at approval gates”). _You_
  appears in setup and remediation copy (“you must explicitly continue”).
  Avoid _we_ unless writing project-positioning prose.
- **Casing.** **Sentence case** for buttons, headers, banners and inline copy
  (“Awaiting approval”, “Resume run”, “Open run report”). **Title Case** is
  reserved for proper nouns and the brand wordmark (`CHAINWORKS FORGE`).
  Code identifiers stay verbatim (`RunPlanCompiler`, `goosed`, `ACP`).
- **Tone.** Plain, declarative, slightly engineering-stoic. Things _happen_
  rather than _delight_. Examples lifted from the codebase:
  - “Workflows instead of ad hoc prompt chains.”
  - “Durable artifacts and reports instead of ephemeral chat history.”
  - “Approval gates instead of invisible autonomous continuation.”
- **Status copy is canonical.** Never invent new state names. The vocabulary
  is fixed: `Pending`, `Ready`, `Running`, `Awaiting approval`, `Blocked`,
  `Completed`, `Failed`, `Cancelling`, `Cancelled`. These map 1:1 to
  `ForgeIconBridge.statusSymbol(for:)`.
- **Numbers, paths, identifiers.** Always monospaced (SF Mono). Absolute paths
  are fine; do not abbreviate `~/Library/...` to icons.
- **Emoji.** **Not used** in product surfaces. The system substitutes SF Symbols
  (`bolt.circle.fill`, `checkmark.seal.fill`, `xmark.circle.fill`, …). Emoji
  is acceptable only in informal docs, never in UI strings or commit copy.
- **Punctuation.** No exclamation marks. Em-dashes for asides. Code spans for
  any token a user can type or paste.
- **Headers and tables.** Markdown-flavoured even inside the app — section
  headers stack a primary line + a secondary descriptor (see `ForgeSectionHeader`).

### Examples lifted verbatim

- _“Frozen run truth and operator-visible recovery.”_
- _“Local daemon lifecycle, supervision, packaged-mode health/readiness.”_
- _“Layered test gates for fast runtime validation, remote UI smoke, and full sign-off.”_
- _“UI tests are remote-only by repo policy.”_

That is the voice. Compressed, structural, and low-affect.

---

## Visual Foundations

### Brand DNA

The Chainworks Forge mark is a flock of **three geese** climbing along a
**silver orbit arc** over a **near-black navy** field. The geese have white
bodies, dark navy wings, and a single **orange beak** (`#F59A2B`) that is the
only saturated colour in the entire identity. The wordmark is set in a
display-weight grotesque with wide tracking (~0.06em).

| Element | Fill |
| --- | --- |
| Goose body | `#FAFBFD` / `#EEF3FA` |
| Goose wing | `#243244` |
| Beak (signature accent) | `#F59A2B` |
| Orbit arc gradient | `#E7EDF4 → #A3AFC0 → #627083` |
| Hero backdrop radial | `#24344B → #0E1623 → #04080D` |
| Wordmark on dark | `#F8FAFC` (primary), `#E5EAF2` (FORGE), `#9FB0C4` (tagline) |

### Colour

Two palettes coexist:

1. **Brand palette** (`--cwf-ink-*`, `--cwf-orbit-*`, `--cwf-feather*`,
   `--cwf-beak*`) — used for marketing surfaces, splash, hero, and bounded
   accents. The orange beak is the **only** chromatic accent and stays scarce.
2. **Apple HIG semantic palette** (`--bg-*`, `--label-*`, `--separator`,
   `--status-*`, `--tint`) — used for everything operator-facing. This is the
   redesign’s contribution: the previous system mapped status to raw
   `Color.green/.red/.orange`; we now route through the **systemGreen /
   systemRed / systemOrange / systemBlue / systemYellow** swatches with proper
   light/dark variants.
3. **Status truth outranks decoration.** The brand orange is **never** a
   status colour. Approval gates use systemYellow; warnings use systemOrange.

### Typography

Apple’s **SF Pro Display + SF Pro Text + SF Mono** ramp, mapped to the existing
Forge tokens.

| Forge token | HIG style | Spec |
| --- | --- | --- |
| `screenTitle` | Title 2 | 22 / 28, semibold, tight tracking |
| `sectionHeader` | Headline | 15 / 20, semibold |
| `cardTitle` | Subheadline (semi) | 13 / 18, semibold |
| `body` | Body | 15 / 20, regular |
| `supporting` | Footnote | 12 / 16, regular, secondary label |
| `micro` / `statusCapsule` | Caption 1 / 2 | 11 / 13, medium |
| `t-mono` (artifacts, paths, IDs) | SF Mono | 12 / 16 |

The brand wordmark is its own face: display-weight, uppercase, 0.06em tracking,
**only** for splash / hero / loading states. It never appears as a UI label.

### Spacing & rhythm

Strict 4-pt grid (`--sp-1`…`--sp-10`), preserving Forge legacy aliases
(`compact 4 / small 8 / medium 12 / large 16 / section 20`). Operator surfaces
prefer `medium → large` for inner gutters; section dividers always at `section`.

### Corner radii

Apple’s continuous-corner curve, three tiers:

- **10 px** — small controls (capsules, inputs, segmented controls).
- **12–14 px** — cards (`forge-radius-card = 14`).
- **16–20 px** — panels, sheets (`forge-radius-panel = 16`).
- **999 px** — pure capsules (status pills).

Always use `border-radius` with continuous geometry; on SwiftUI the rule is
`RoundedRectangle(cornerRadius:..., style: .continuous)`.

### Elevation & materials

Forge is a desktop product, so shadows are very soft. Four steps:

- `shadow-1` — hairline. Inputs, segmented controls.
- `shadow-2` — card. Default surface lift.
- `shadow-3` — popover. Menus, hovered cards.
- `shadow-4` / `shadow-popover` — sheet, attention banner.

Every elevated surface also carries a **0.5 px hairline** (`--hairline-all`)
which mimics Apple’s vibrant border. On dark mode the hairline switches to a
1-pt 6 % white inner stroke.

### Backgrounds

- App canvas: `--bg-canvas` (`#F2F2F7` / `#000000`).
- Cards: `--bg-elevated` over the canvas, never edge-to-edge.
- Brand surfaces (splash, login, hero) use the **inkwell radial** —
  `radial-gradient(circle at 30% 40%, #24344B 0%, #0E1623 50%, #04080D 100%)`.
- No hand-drawn illustrations; no full-bleed photography. The geese SVG is the
  only illustration.

### Animation

- Easing: `cubic-bezier(.32, .72, 0, 1)` (Apple’s “snap”) or
  `cubic-bezier(.4, 0, .2, 1)` for tonal transitions. **No bounces.**
- Duration: 150 ms (micro), 250 ms (standard), 400 ms (sheet / overlay).
- Hover: opacity `0.85` on text controls; `bg-fill-quaternary` overlay on
  bordered controls. Press: 96 % scale + opacity `0.7`. Reduce-motion just
  cross-fades.
- The geese flock has a **subtle 6-second loop** (vertical drift ±2 px,
  staggered) on splash only — never on operator surfaces.

### Borders / cards / shadows

| Surface | Background | Border | Shadow | Radius |
| --- | --- | --- | --- | --- |
| Card | `--bg-elevated` | `0.5 px --separator` | `--shadow-2` | 14 px |
| Panel | `--bg-elevated` | `0.5 px --separator` | none | 16 px |
| Sheet | `--bg-elevated` | none | `--shadow-popover` | 20 px |
| Capsule | `color-mix currentColor 14%` | none (or 1 px in increase-contrast) | none | 999 px |
| Banner (attention) | `color-mix currentColor 12%` | `0.5 px currentColor 30%` | none | 12 px |

### Transparency & blur

Used sparingly. Nav chrome and toolbars adopt `backdrop-filter: blur(20px)
saturate(180%)` over a translucent `--bg-elevated`. Inside content areas,
**no blur** — clarity outranks vibrancy.

### Imagery vibe

Cool, near-monochrome, navy with a single orange tap. No grain. No gradients
with purple/pink. Photography is avoided in product surfaces; the only allowed
imagery is the brand mark, the horizontal logo, and the hero render.

---

## Iconography

- **Primary system: SF Symbols.** The codebase already binds icons via
  `ForgeIconBridge.symbol(_:)` and `statusSymbol(for:)`. We keep that contract.
  All status icons map to SF Symbol names (`bolt.circle.fill`,
  `checkmark.seal.fill`, `pause.circle.fill`, `xmark.circle.fill`,
  `slash.circle.fill`, `hourglass`, `clock`).
- **Web substitution.** Browsers can’t render SF Symbols, so the React UI kits
  use **[Lucide](https://lucide.dev)** via CDN as the closest stroke-weight
  match (1.5 px, rounded line caps). We **flag this substitution** explicitly:
  in production SwiftUI surfaces, ship SF Symbols; in HTML mocks, Lucide
  approximates them.
- **Artifact glyphs.** `ForgeIconBridge.artifactSymbol(for:)` maps formats to
  symbols (`curlybraces` for JSON, `doc.text` for Markdown,
  `arrow.left.arrow.right` for diff, `doc.text.fill` for report). Web
  substitution: `Braces`, `FileText`, `GitCompare`, `FileCheck`.
- **Brand mark / logo / hero.** Live in `assets/` as PNG **and** are recreated
  as inline SVG components in `ui_kits/brand/` for crisp scaling.
- **No emoji** in UI surfaces (per content rules).
- **No unicode glyphs** as icons (no `→`, `✓`, `★`). Always SF Symbol or Lucide.
- **No hand-drawn SVG.** The geese are the only illustration; everything else
  is iconography.

### Font availability flag

The app targets `SF Pro Display`, `SF Pro Text`, `SF Pro Rounded`, `SF Mono`.
These ship with macOS but **are not free for arbitrary distribution**. For
HTML mocks we let the system fallback resolve (`-apple-system` → SF on Apple
devices → falls through to Inter / Helvetica Neue elsewhere).

> **🚩 Substitution flag:** if you need a web-distributable equivalent, swap
> the stack to **Inter Display + Inter + JetBrains Mono** (Google Fonts). Ask
> the user if non-Apple platforms need official Apple-licensed font files.

---

## Status / state vocabulary

Defined in `ForgeIconBridge` and `StatusCapsule`. Do not invent new states.

| State | Color token | SF Symbol | Lucide (web) |
| --- | --- | --- | --- |
| `Pending` / `Ready` | `--status-pending` | `clock` | `Clock` |
| `Running` | `--status-running` | `bolt.circle.fill` | `Zap` (filled circle) |
| `Awaiting approval` | `--status-approval` | `checkmark.seal.fill` | `BadgeCheck` |
| `Blocked` | `--status-warning` | `pause.circle.fill` | `PauseCircle` |
| `Completed` | `--status-success` | `checkmark.circle.fill` | `CheckCircle2` |
| `Failed` | `--status-error` | `xmark.circle.fill` | `XCircle` |
| `Cancelling` | `--status-pending` | `hourglass` | `Hourglass` |
| `Cancelled` | `--status-cancelled` | `slash.circle.fill` | `Ban` |

---

## Apple-HIG redesign — what changed vs. legacy `Forge*`

This is the brief, explicit changelog so reviewers can sanity-check the lift:

1. **Status colors** moved from raw `Color.green/.red/.orange/.blue` to
   the **systemGreen / systemRed / systemOrange / systemBlue / systemYellow**
   token set with light + dark variants.
2. **Type ramp** replaced ad-hoc `.title2 / .headline / .body` with the full
   HIG ramp (Large Title → Caption 2) — keeping `ForgeTypography` token names
   as a backwards-compatible facade.
3. **Corner radii** rationalised onto the HIG continuum (10 / 12 / 14 / 16 / 20 /
   pill). Legacy `ForgeRadius.card = 14`, `panel = 16`, `capsule = 999`
   preserved as facade values.
4. **Elevation + hairline** introduced (`--shadow-1…4`, `--hairline-all`).
   Previously cards had border-only treatment; now they carry HIG-style
   inset hairline + soft outer shadow.
5. **Brand subordination** rule made explicit: orange beak / wordmark
   never appears in operator status surfaces.
6. **Materials** added: blurred toolbars (`backdrop-filter: blur(20)
   saturate(180%)`).

---

## Caveats

See the bottom of this turn — the agent listed open questions and asks for the
human to validate before iterating.
