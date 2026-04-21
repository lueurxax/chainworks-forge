# Rust Performance Review Rubric

Use only when evidence shows latency, throughput, allocation, contention, streaming, serialization, or hot-path risk.

## Focus areas

- Allocation and copy behavior: avoid accidental clones, string churn, and unbounded buffering.
- Contention: locks, channels, task scheduling, shared maps, and blocking calls are scoped.
- Serialization: JSON/protobuf encoding, validation, compression, and schema conversion costs are bounded.
- Streaming/batching: backpressure and chunk sizes match consumers.
- Measurement: benchmark, trace, profile, or production signal fits the actual risk.

## Sharp heuristics

- Do not demand benchmarks for cold operator paths unless the proposal claims performance goals.
- Treat unbounded fan-out, unbounded buffers, and synchronous filesystem/network work inside async paths as performance and reliability risks.
- Treat cache proposals without invalidation and memory bounds as incomplete.

## Finding requirements

Each finding must cite evidence IDs, hot path, expected cost or failure mode, required fix, acceptance criteria, and confidence.
