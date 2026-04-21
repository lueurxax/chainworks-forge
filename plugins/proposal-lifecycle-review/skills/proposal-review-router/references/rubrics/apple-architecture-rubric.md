# Apple Architecture Review Rubric

Use for iOS/macOS proposals touching state, navigation, lifecycle, persistence, networking, or module boundaries.

## Focus areas

- Ownership: view, model, service, persistence, networking, and routing boundaries are explicit.
- State flow: source of truth, derived state, cache invalidation, navigation state, and restoration are coherent.
- Concurrency: actor isolation, cancellation, task lifetime, background work, and main-thread updates are safe.
- Persistence: migrations, offline behavior, sync semantics, and data loss paths are specified.
- Integration: API errors, auth expiry, feature flags, telemetry, and rollback are represented in client state.
- Testability: deterministic units, preview/test seams, and failure injection are credible.

## Sharp heuristics

- Treat `@MainActor` as an ownership claim that must match actual work, not a blanket fix.
- Treat navigation as state; proposals must define deep-link, back-stack, and restoration behavior when routes change.
- Treat client feature flags as data dependencies with rollout and stale-config behavior.

## Finding requirements

Each finding must cite evidence IDs, affected modules or proposal lines, architectural consequence, required fix, acceptance criteria, and confidence.
