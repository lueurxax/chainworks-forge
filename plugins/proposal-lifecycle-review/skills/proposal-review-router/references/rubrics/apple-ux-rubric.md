# Apple UX Review Rubric

Use for iOS/macOS proposal flows where trust, clarity, accessibility, or multi-step user outcomes matter.

## Focus areas

- User goal: the main task is recognizable and efficient for repeated use.
- Mental model: labels, hierarchy, and state transitions explain what is happening.
- Error prevention: destructive, expensive, privacy-sensitive, or irreversible actions are guarded.
- Recovery: users can understand failure causes and next actions without losing context.
- Accessibility: Dynamic Type, VoiceOver, keyboard, reduced motion, contrast, and localization length are viable.
- Trust: permissions, data handling, sync status, money, identity, and operational risk are explicit.

## Sharp heuristics

- If a user cannot tell whether work is saved, submitted, queued, or failed, treat it as a P1/P2 UX risk depending on consequence.
- If recovery advice requires internal vocabulary, require operator/user-facing translation.
- If the proposal introduces metrics but no user decision point, consider `product_reviewer` only when metrics drive launch decisions.

## Finding requirements

Each finding must include evidence IDs, affected journey/state, user harm, required proposal change, acceptance criteria, and confidence.
