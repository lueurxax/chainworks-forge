# Observability and Rollout Review Rubric

Use for proposals involving feature flags, migrations, rollout sequencing, rollback, telemetry, SLOs, alerts, dashboards, or operational support.

## Focus areas

- Telemetry: logs, metrics, traces, events, dimensions, and sampling answer the operational question.
- Feature flags: default state, targeting, stale config, kill switch, and ownership are explicit.
- Rollout: sequencing, canary/dark launch, hold criteria, blast radius, and cross-stack ordering are credible.
- Rollback: reversible migrations, downgrade behavior, cleanup, and data recovery are defined.
- Supportability: dashboards, alerts, runbooks, user/operator messages, and debug surfaces exist.

## Sharp heuristics

- Treat migrations without rollback or forward-fix plan as incomplete.
- Treat telemetry that cannot distinguish old vs new path as inadequate for rollout decisions.
- Treat client/server staggered rollout as an API compatibility issue too.

## Finding requirements

Each finding must cite evidence IDs, rollout or operability gap, consequence, required fix, acceptance criteria, and confidence.
