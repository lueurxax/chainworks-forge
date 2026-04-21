# macOS UI Review Rubric

Use for macOS UI proposals. Do not require app launch or screenshots in proposal-readiness mode.

## Focus areas

- Window ergonomics: resizing, minimum size, split views, sidebars, inspectors, and dense layouts remain coherent.
- Desktop interaction: toolbar, menu commands, context menus, keyboard shortcuts, focus, selection, drag-and-drop, and multiwindow behavior fit macOS.
- State presentation: loading, empty, error, disabled, recovery, and long-running task states are visible in multi-pane layouts.
- Platform fidelity: avoid phone-style full-screen flows, oversized cards, hidden controls, and touch-first patterns on desktop.
- Accessibility: keyboard navigation, VoiceOver order, contrast, focus rings, reduced motion, and non-color cues are covered.

## Sharp heuristics

- Treat missing keyboard/focus behavior as a real macOS defect, not polish.
- Treat unclear toolbar/menu ownership as architecture-adjacent; route `apple_arch_reviewer` when command state is shared.
- Treat modal-only recovery for operational tools as a workflow risk if users need side-by-side context.

## Finding requirements

Each finding must include severity, evidence IDs, proposal/code location, desktop-specific impact, required fix, acceptance criteria, and confidence.
