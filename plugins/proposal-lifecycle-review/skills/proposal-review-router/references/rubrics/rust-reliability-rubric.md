# Rust Reliability Review Rubric

Use for Rust proposals involving failure handling, async work, queues, retries, cancellation, recovery, or state machines.

## Focus areas

- Failure taxonomy: validation, dependency, timeout, cancellation, shutdown, retryable, and terminal errors differ.
- Idempotency: retries, dedupe keys, replay, and duplicate enqueue paths are durable and testable.
- Atomicity: DB mutations, work queue scheduling, journal writes, and event publication have repair semantics.
- Backpressure: queue depth, admission control, overload behavior, and bounded concurrency are defined.
- Shutdown/recovery: in-flight work settles without silent loss, double execution, or false status truth.
- Diagnostics: errors are actionable and tied to persisted evidence.

## Sharp heuristics

- Any proposal that says "retry", "resume", or "replay" needs duplicate-work and stale-claim semantics.
- Any pre-created runtime row needs a truthful lifecycle state before provider work starts.
- Any background task spawned without ownership must define cancellation and shutdown behavior.

## Finding requirements

Each finding must include evidence IDs, failed state or race, consequence, required fix, acceptance criteria, and confidence.
