# Implementation Evidence Playbook

Use this playbook to keep audits grounded and small.

## Start with the Contract

Read the proposal before reading implementation details. Extract the explicit commitments and make them atomic. A large paragraph can yield several `REQ-*` items, but only if each item is a real proposal commitment.

Do not create requirements from taste, platform preference, or reviewer habit.

## Then Find Implementation Evidence

Prefer evidence in this order when practical:

1. Direct implementation code on the relevant path.
2. Focused tests that assert the committed behavior.
3. Tests actually run in this audit.
4. Runtime evidence, screenshots, logs, traces, or benchmark output when the claim needs live proof.
5. Config, schema, migrations, generated code, telemetry, or rollout controls.
6. Inference only as a low-confidence supplement.

Never mark `Implemented` from inference alone.

## Full Regression Gate

If the audit is trending toward a successful roll-up, run the repository's full regression suite or canonical full/proposal gate on the same tree/HEAD before locking the verdict. Treat `Overall Conformance = Implemented`, `Overall Implementation Readiness = Ready`, and `Overall Implementation Readiness = Ready with Risks` as successful outcomes.

Fail closed when that gate is unavailable, red, stale, or from a different tree/HEAD. Record the exact command/gate and result in the verification log.

## Use Prior Proposal Review as Context

Prior proposal-review output can tell you what reviewers cared about and what risks were expected. It does not prove the implementation is correct. Verify every carried-forward risk against current code, diff, tests, or runtime evidence.

## Keep the Search Slice Narrow

Useful narrow searches:

- proposal terms, API names, feature flags, route names, proto messages, migration names
- prior review finding IDs and required-change phrases
- entry points and test names named in the proposal
- schema/generated code references
- telemetry event names, metric names, flag names

Avoid broad repo spelunking unless the proposal itself is broad.

## Evidence Gap Discipline

Use `Not Verifiable` when a behavior may exist but cannot be proven. This is especially important for:

- UI flows with only code-level evidence
- service behavior that depends on retry/shutdown/overload paths but has no tests or logs
- performance claims with benchmark files but no run result
- rollout claims with flag code but no rollback path
- API compatibility claims without generated client/server alignment evidence
