# Chainworks Forge — Design Kit v1

## 1. Why This Exists

This document defines the baseline visual system for Chainworks Forge: the mark, colors, typography, iconography, interface rules, and ready-made prompts for further generation and refinement. The goal is simple: the product should look like a tool with character, not like just another AI chat app with a polished cover.

The core brand metaphor is:

> **Coordinated motion by several specialized agents under the guidance of a leader.**

That leads directly to the mark:

- **a wedge of three geese** for workflow and coordinated movement;
- **the lead goose** for the orchestrator / run;
- **a soft trajectory line** for flow, chain, and the execution path;
- **no literal chain** to reduce noise and keep the product message clear.

---

## 2. Brand Core

### Name

**Chainworks Forge**

### Character

- engineered;
- deliberate;
- alive, but not noisy;
- disciplined without corporate sterility;
- not "magical AI," but a clear control plane.

### What The Brand Should Convey

- orchestration;
- forward motion;
- coordinated roles working together;
- control over a complex process;
- transformation of a raw idea into a result.

### What The Brand Should Not Convey

- generic AI marketing chatter;
- chatbot vibes;
- fantasy for its own sake;
- heavy-handed "metal" literalism;
- startup gloss built on meaningless gradients.

---

## 3. Logo

## 3.1 Core Idea

The primary mark is **three geese in a wedge**, flying upward along a rising trajectory from lower left to upper right.

### Semantics

- the first goose is the leader, orchestrator, and run control;
- the following two geese are specialized agents;
- the overall form communicates workflow / chain of execution;
- the line beneath the birds expresses direction and execution trajectory.

## 3.2 Core Rules

### Required Properties Of The Mark

- exactly 3 geese, not 4 or 5;
- the wedge form must remain legible even at small sizes;
- the leader should sit slightly ahead;
- the motion line should stay soft and secondary, never overpowering the birds;
- silhouettes must stay clean, without tiny illustrative details.

### What Not To Do

- do not add literal chain links;
- do not turn the mark into a complex illustration;
- do not render the birds as realistic ornithological drawings;
- do not overload the wings with tiny feathers;
- do not use multiple accent colors at the same time.

## 3.3 Logo Variants

### Primary Horizontal Version

Usage:
- README;
- website;
- splash / launch screen;
- documentation;
- presentations.

Composition:
- mark on the left;
- wordmark on the right;
- spacing between mark and wordmark should feel open and breathable.

### App icon version

Usage:
- `Assets.xcassets`;
- macOS app icon;
- internal build previews.

Composition:
- square composition;
- centered mark;
- dark background;
- no text.

### Monochrome version

Usage:
- toolbar;
- sidebar;
- small-size UI;
- print / single-color use cases.

Composition:
- single color;
- no internal highlights;
- the cleanest possible silhouette.

## 3.4 Minimum Sizes

- **up to 24 px** use a simplified silhouette without the trajectory line;
- **24–64 px** use the three-geese mark without small internal details;
- **64 px and above** the full version with the motion line is acceptable.

## 3.5 Safe area

Keep a clear margin around the mark of at least the height of the middle goose's head. Nothing should crowd the leader's beak or the outer arc of the trajectory line.

---

## 4. Color System

## 4.1 Primary Palette

### Primary

| Token | Hex | Purpose |
|---|---|---|
| `ForgeBlue` | `#0B1F2A` | primary dark background, key interface accents |
| `ForgeBlueSoft` | `#132F3F` | secondary dark tone, hover / panels / depth |

### Accent

| Token | Hex | Purpose |
|---|---|---|
| `ForgeAccent` | `#FF8A00` | beaks in the logo, CTAs, approval / attention highlights |
| `ForgeAccentSoft` | `#FFB347` | softer accents, highlights, secondary emphasis |

### Neutrals

| Token | Hex | Purpose |
|---|---|---|
| `ForgeBackgroundLight` | `#F5F7FA` | light background |
| `ForgeBackgroundDark` | `#0A0F14` | dark background |
| `ForgeSurfaceLight` | `#FFFFFF` | cards and surfaces on light backgrounds |
| `ForgeSurfaceDark` | `#111821` | cards and surfaces on dark backgrounds |
| `ForgeTextPrimary` | `#1A1A1A` | primary text on light backgrounds |
| `ForgeTextSecondary` | `#6B7280` | secondary text |
| `ForgeTextOnDark` | `#E8EDF2` | primary text on dark backgrounds |

## 4.2 Statuses

| Status | Token | Hex |
|---|---|---|
| running | `RunBlue` | `#2563EB` |
| waiting approval | `RunAmber` | `#F59E0B` |
| blocked | `RunRed` | `#DC2626` |
| failed | `RunCrimson` | `#B91C1C` |
| completed | `RunGreen` | `#16A34A` |
| pending / idle | `RunGray` | `#9CA3AF` |

## 4.3 Color Rules

- use orange sparingly;
- orange should not become the default color for large surfaces;
- the logo should still work without orange in monochrome form;
- the interface should not feel like a "black terminal with neon buttons";
- status color matters more than decorative color.

---

## 5. Typography

## 5.1 Primary Typeface

Use Apple's system stack:

- **SF Pro Display** for headings;
- **SF Pro Text** for primary interface text and content.

## 5.2 Sizes

| Purpose | Size |
|---|---|
| App Title / hero | 24–28 |
| Section title | 18–20 |
| Standard label | 14–16 |
| Body text | 13–14 |
| Meta / helper | 11–12 |

## 5.3 Weights

| Purpose | Weight |
|---|---|
| Hero / big section titles | Semibold / Bold |
| Primary UI labels | Medium |
| Body copy | Regular |
| Secondary info | Regular / Medium |

## 5.4 Typography Rules

- do not use bold as a substitute for hierarchy;
- do not set the entire interface in uppercase;
- uppercase is acceptable only in small utility labels and status chips;
- spacing matters more than "pretty type effects."

---

## 6. Iconography

Icons should continue the language of the logo, not compete with it.

## 6.1 Base Atom

**The goose** is the atomic symbol of motion and execution.

## 6.2 System Icons

| Icon | Meaning | Idea |
|---|---|---|
| `run` | run execution | a single goose facing forward |
| `workflow` | workflow | three geese in a wedge |
| `stage` | stage | goose + forward point |
| `approval` | gate / approval | goose + check |
| `blocked` | blocked state | goose + broken trajectory |
| `failed` | failure | goose + cross |
| `completed` | success | goose + circle / finish mark |
| `artifact` | artifact | sheet / layer + soft trajectory |

## 6.3 Rules For Small Icons

- below 16 px, do not draw three birds; use only an abstracted silhouette;
- below 20 px, avoid thin trailing lines;
- small icons should be monochrome.

---

## 7. UI Principles

## 7.1 Product Hierarchy

The interface should always make this sequence obvious:

```text
Run → Stage → Agent → Artifact
```

### What This Means Visually

- **Run** is the primary object, large and most visually prominent;
- **Stage** is the primary context within a run;
- **Agent** is the executor within a stage;
- **Artifact** is the result available for inspection.

## 7.2 What Not To Do

- do not build the interface like a chat app;
- do not collapse progress, logs, approvals, and artifacts into one layer;
- do not hide important actions behind decorative card grids;
- do not make the interface look "technical" at the expense of clarity.

## 7.3 Main Screen Rule

On any key screen, the user should quickly understand:

1. where the run is now;
2. what needs attention;
3. what has already happened;
4. what can be opened and checked.

---

## 8. Motion

Motion should help with orientation, not entertainment.

## 8.1 Allowed Effects

- a soft fade / slide between stages;
- subtle pulse for a running agent;
- brief pop-in for an approval gate;
- soft status transitions for run chips.

## 8.2 Prohibitions

- no "flying logo" on every screen;
- no endless decorative animation;
- no heavy spring animation for utility UI.

---

## 9. Assets And Structure

Recommended structure:

```text
Design/
  Brand/
    chainworks_forge_logo_main.png
    chainworks_forge_logo_dark.png
    chainworks_forge_logo_light.png
    chainworks_forge_logo_monochrome.png
  Icons/
    run.svg
    workflow.svg
    stage.svg
    approval.svg
    blocked.svg
    failed.svg
    completed.svg
    artifact.svg
  AppIcon/
    appicon_1024.png
    appicon_dark.png
    appicon_light.png
  Tokens/
    Colors.swift
    Typography.swift
    Theme.swift
```

---

## 10. SwiftUI design tokens

Below is a basic starter structure for the code-side design system.

```swift
import SwiftUI

enum ForgeColor {
    static let blue = Color(hex: 0x0B1F2A)
    static let blueSoft = Color(hex: 0x132F3F)
    static let accent = Color(hex: 0xFF8A00)
    static let accentSoft = Color(hex: 0xFFB347)

    static let backgroundLight = Color(hex: 0xF5F7FA)
    static let backgroundDark = Color(hex: 0x0A0F14)
    static let surfaceLight = Color.white
    static let surfaceDark = Color(hex: 0x111821)

    static let textPrimary = Color(hex: 0x1A1A1A)
    static let textSecondary = Color(hex: 0x6B7280)
    static let textOnDark = Color(hex: 0xE8EDF2)

    static let runBlue = Color(hex: 0x2563EB)
    static let runAmber = Color(hex: 0xF59E0B)
    static let runRed = Color(hex: 0xDC2626)
    static let runCrimson = Color(hex: 0xB91C1C)
    static let runGreen = Color(hex: 0x16A34A)
    static let runGray = Color(hex: 0x9CA3AF)
}

enum ForgeTypography {
    static let hero = Font.system(size: 26, weight: .semibold, design: .default)
    static let section = Font.system(size: 18, weight: .semibold, design: .default)
    static let label = Font.system(size: 14, weight: .medium, design: .default)
    static let body = Font.system(size: 13, weight: .regular, design: .default)
    static let meta = Font.system(size: 11, weight: .regular, design: .default)
}
```

---

## 11. App Icon Rules

## 11.1 Composition

- center the mark;
- the geese should not press against the edges;
- the trajectory line should not touch the icon's rounded corners;
- a dark background is preferable so the silhouette keeps its contrast.

## 11.2 What To Check Manually

- does the mark stay legible at 32 px;
- does the trajectory line remain distinct from the background;
- does the middle goose become visual noise;
- does the mark accidentally look like an airline instead of a control plane.

---

## 12. Prompt pack

Below is a set of ready-made prompts in case the logo needs to be generated or refined again with an image model, or handed off to a designer.

## 12.1 Primary Prompt: Production Logo Refinement

```text
Create a production-ready logo for a macOS developer tool called “Chainworks Forge”.

Core metaphor:
- three geese flying in a clean V-formation
- the lead goose is slightly ahead and represents orchestration
- the other two geese represent specialized agents
- a subtle curved trajectory line underneath suggests workflow / execution flow
- do NOT use literal chain links

Style:
- modern product logo, not an illustration
- minimal, crisp, vector-like shapes
- clean silhouette that works at small sizes
- elegant but restrained
- engineering product, not playful mascot branding
- not an AI cliché, not fantasy art, not corporate stock style

Visual rules:
- exactly 3 geese
- reduce feather detail
- strong silhouette readability
- one lead bird slightly emphasized
- thin secondary trajectory line
- balanced negative space
- suitable for app icon and horizontal product branding

Color palette:
- dark navy / graphite body tones
- soft light gray for contrast shapes
- small orange accent on beaks only
- optional monochrome version

Deliver:
1. primary horizontal logo
2. square app icon version
3. monochrome version
4. dark and light background variants

Avoid:
- chain links
- realism
- too many gradients
- mascot/cartoon look
- too much detail in wings
- generic AI / neural / brain imagery
```

## 12.2 Prompt: ultra-minimal icon version

```text
Design an ultra-minimal app icon for “Chainworks Forge” based on three geese flying in V-formation.

Requirements:
- square composition
- dark background
- simplified geometric bird shapes
- high contrast
- readable at 32px and 64px
- no text
- subtle curved line for motion if it survives small-size clarity
- elegant, premium, engineering-tool aesthetic

Avoid:
- realistic feathers
- decorative textures
- excessive highlights
- cartoon bird styling
```

## 12.3 Prompt: monochrome UI symbol system

```text
Create a monochrome icon family for a macOS workflow orchestration tool.

Base metaphor: stylized goose / flight formation.

Icons needed:
- run
- workflow
- stage
- approval
- blocked
- failed
- completed
- artifact

Style:
- monochrome
- minimal
- consistent stroke logic
- suitable for toolbar / sidebar / 16–20px sizes
- derived from the same visual language as a three-geese logo
```

## 12.4 Prompt: brand board / presentation sheet

```text
Create a clean brand presentation board for “Chainworks Forge”.

Show:
- primary logo
- app icon
- dark version
- light version
- monochrome version
- color palette chips
- logo usage examples on a macOS product context

Style:
- premium product design board
- calm grid layout
- minimal labels
- no fake file icons
- no overdesigned presentation elements
- modern, technical, elegant
```

---

## 13. Practical Next Step

If this turns into a real working package, the order should be:

1. lock the **primary mark**;
2. build the **1024×1024 app icon**;
3. produce the **monochrome SVG / vector version**;
4. assemble `Colors.swift`, `Typography.swift`, and `Theme.swift`;
5. roll the system into 2-3 key SwiftUI screens;
6. only then refine smaller decorative differences.

Otherwise it becomes too easy to produce lots of attractive details before the structural backbone exists.

---

## 14. Short Formula

> **Chainworks Forge should feel less like an AI toy and more like a disciplined working tool.**
>
> Visually, that comes from three things:
> - coordinated motion,
> - leadership within the system,
> - and a clean engineering form without unnecessary magic.
