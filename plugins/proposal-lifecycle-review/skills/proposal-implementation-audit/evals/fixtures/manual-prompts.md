# Manual Prompt Fixtures

## Default implementation audit

```text
Use $proposal-implementation-audit in auto mode for docs/proposals/<proposal>.md.
Reuse prior proposal-review reviewer selection when valid, audit the current implementation against the proposal, and write the versioned report beside the proposal.
```

## Audit current PR diff

```text
Use $proposal-implementation-audit in auto mode for docs/proposals/<proposal>.md against the current PR diff.
Show whether prior proposal-review routing was reused, changed, or rejected.
```

## Readiness gate

```text
Use $proposal-implementation-audit in implementation-readiness mode for docs/proposals/<proposal>.md.
Focus on conformance, prior review follow-through, tests, rollout, security, and release blockers.
```

## Force reroute

```text
Use $proposal-implementation-audit in reroute mode for docs/proposals/<proposal>.md.
Ignore prior selected reviewers except as context and route from current proposal plus implementation evidence.
```
