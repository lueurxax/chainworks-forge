# Architecture Review Rubric: iOS Head of Architecture Perspective

Evaluate technical architecture quality and implementation risk for a proposed iOS flow or a mapped current repo surface. Work from proposal text, adjacent docs, the reusable host-system baseline when present, code-path mapping, and current repo reality. Runtime evidence is optional and only for host-system ambiguity reduction in `proposal-readiness`.

## Focus Areas

1. System boundaries and modularity
- Assess separation of concerns across domain, application, and presentation layers.
- Check dependency direction and module ownership clarity.

2. State management and data flow
- Validate single-source-of-truth strategy and synchronization behavior.
- Check handling of async updates, stale state, and race conditions.

3. Concurrency and performance
- Evaluate actor isolation, threading model, and main-thread safety.
- Assess rendering cost, memory pressure, and startup/runtime performance risks.

4. Reliability and testability
- Verify test seams, deterministic logic boundaries, and failure injection points.
- Check unit, integration, and UI test strategy for proposal-critical paths.

5. Security and privacy
- Evaluate key handling, local storage, PII exposure, and network boundaries.
- Check logging and telemetry for sensitive data leaks.

6. Finance-sensitive correctness and operability
- Check monetary precision and rounding behavior.
- Check stale balances and stale market or account data handling.
- Check idempotency for money-moving or commitment-changing actions.
- Check reconciliation between local state and server truth.
- Check auditability, rollback, and event traceability.
- Check date and timezone correctness for financial commitments.
- Check feature-flag, rollout, and rollback safety for financially sensitive flows.

## Output Requirements

For each finding, include:

- Finding ID
- Severity
- Evidence IDs
- Rationale / Why it matters
- Recommended fix
- Acceptance criteria
- Confidence
- Evidence gaps when the proposal or code mapping is too weak to support a firm call
