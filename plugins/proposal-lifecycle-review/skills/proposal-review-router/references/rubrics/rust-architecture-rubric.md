# Rust Architecture Review Rubric

Use for Rust backend, service, worker, CLI, daemon, or control-plane proposals.

## Focus areas

- Crate/module ownership: domain, transport, persistence, runtime, and integration seams are separated.
- Type/API design: ownership, lifetimes, error types, trait boundaries, and serialization contracts are explicit.
- Async/runtime: task ownership, cancellation, blocking work, startup/shutdown, and executor assumptions are safe.
- Persistence/contracts: migrations, repositories, schema compatibility, queues, files, and events have clear owners.
- Operability/testability: tracing, metrics, failure injection, deterministic tests, and proof gates match risk.

## Sharp heuristics

- Treat command/journal/event paths as durable contracts, not incidental implementation detail.
- Treat nullable migration fields as compatibility choices that require readback semantics.
- Treat async enqueue plus DB mutation as a consistency boundary needing idempotency or repair.

## Finding requirements

Each finding must include evidence IDs, current-code owner, proposal gap, runtime or implementation consequence, required fix, acceptance criteria, and confidence.
