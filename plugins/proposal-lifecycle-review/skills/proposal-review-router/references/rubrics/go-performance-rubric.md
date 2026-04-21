# Go Performance Review Rubric

Use only when evidence shows Go latency, throughput, allocation, GC, serialization, batching, or hot-path risk.

## Focus areas

- Allocation and GC pressure: avoid avoidable heap churn in request/worker hot paths.
- Serialization/network overhead: JSON/protobuf conversion, compression, and round trips are bounded.
- Contention: mutexes, pools, channels, fan-out, and shared caches are scoped.
- Batching/pooling: batch sizes, pool lifetime, and fairness are defined.
- Measurement: benchmark, pprof, trace, or production metric fits the risk.

## Sharp heuristics

- Do not request performance work for non-hot operator paths without evidence.
- Treat `sync.Pool` as an optimization needing ownership and correctness proof.
- Treat unbounded response bodies or request buffering as reliability and performance risk.

## Finding requirements

Each finding must cite evidence IDs, hot path, expected cost, required fix, acceptance criteria, and confidence.
