# AGENTS.md Template for Repo Root

Fill this in for the repository. Keep it concrete and current.

## Project Defaults
- Primary app(s):
- Primary languages / frameworks:
- Main proposal directories:
- Canonical review artifact convention:
- Canonical reusable baseline artifact convention:
- When proposal review is allowed without runtime evidence:
- When targeted baseline refresh is preferred:

## Build / Test / Validation Commands
- Primary build command:
- Primary unit-test command:
- Primary UI-test command:
- Lint / format commands:
- Any required environment variables:
- Commands that are intentionally forbidden or unsafe:

## Apple Platform Defaults
- Scheme name(s):
- Target name(s):
- Default simulator device:
- Default simulator OS:
- App boot / deep-link conventions:
- Login / seed-data / test-account conventions:
- Screenshot capture expectations:
- When Xcode MCP should be used:

## Proposal Review Expectations
- Required local materials before review:
- Required reusable baseline inputs before review:
- Proposal evidence pack expectations:
- Proposal-specific integration-context expectations:
- Research-pack expectations:
- What counts as an evidence-gap review:
- What must never cause an evidence-gap review by itself:

## Architecture Invariants
- Shared module boundaries:
- State ownership rules:
- Persistence / sync rules:
- Concurrency rules:
- Testability / operability rules:
- Security / privacy / PII rules:

## Rollout / Flags / Rollback
- Feature-flag convention:
- Rollout expectation:
- Rollback expectation:
- Hold criteria:

## Telemetry / Testing Expectations
- Required analytics or instrumentation fields:
- Required happy-path coverage:
- Required non-happy-path coverage:
- Accessibility expectations:
- Minimum regression gates:

## Baseline Refresh Expectations
- When to refresh `.review-baselines/current-system-baseline.md`:
- What makes a baseline slice stale enough to refresh:
- What must be mapped from docs/code before any runtime observation:
- When `xcode_operator` is allowed for ambiguity reduction:
