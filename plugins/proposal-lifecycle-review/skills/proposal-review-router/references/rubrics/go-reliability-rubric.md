# Go Reliability Review Rubric

Use for Go proposals involving deadlines, goroutines, queues, retries, workers, shutdown, or failure handling.

## Focus areas

- Context and deadlines: every external call and worker path respects cancellation.
- Goroutine lifecycle: ownership, error propagation, leak prevention, and shutdown are explicit.
- Retry/idempotency: dedupe keys, replay behavior, and duplicate side effects are handled.
- Backpressure: bounded queues, worker pools, admission control, and overload behavior exist.
- Recovery/diagnostics: failures are observable and actionable.

## Sharp heuristics

- Any goroutine without a cancellation owner is suspect.
- Any retry without idempotency semantics is incomplete.
- Any channel or queue without bounds or close semantics needs review.

## Finding requirements

Each finding must cite evidence IDs, failure path, operational consequence, required fix, acceptance criteria, and confidence.
