# iOS UI Review Rubric

Use for iPhone/iPad UI proposals. Do not require simulator proof in proposal-readiness mode.

## Focus areas

- Visual hierarchy: primary action, scan order, density, and content grouping are clear.
- State coverage: loading, empty, error, disabled, success, permission, and offline states are specified.
- Navigation: back stack, sheets, full-screen covers, tabs, deep links, and restoration behave predictably.
- Platform fidelity: safe areas, touch targets, Dynamic Type, keyboard, pointer on iPad, and native gestures fit iOS.
- Accessibility: VoiceOver labels, focus order, contrast, non-color status cues, reduced motion, and hit targets are covered.
- Integration seams: UI state reflects backend/API failures without lying or trapping the user.

## Sharp heuristics

- Treat hidden destructive actions, ambiguous progress, and irreversible confirmations as trust risks.
- Treat custom navigation shells as architecture-adjacent; route `apple_arch_reviewer` if state ownership is unclear.
- Treat dense admin-style screens on iPhone as platform-fit risks unless the proposal defines responsive behavior.

## Finding requirements

Each finding must include severity, evidence IDs, file/line or proposal section, user-visible impact, required fix, acceptance criteria, and confidence.
