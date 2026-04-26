# Go Service Architecture Review Rubric

Use for Go backend, server, worker, or microservice proposals.

## Focus areas

- Package boundaries: transport, domain, persistence, config, and wiring are separated.
- Interfaces: introduced where they improve testing or substitution, not as ceremony.
- Context flow: request, worker, and shutdown lifecycles preserve `context.Context`.
- Persistence/contracts: migrations, repositories, events, protobuf/OpenAPI, and DTO ownership are explicit.
- Operability/testability: logs, metrics, traces, health, startup, failure injection, and integration tests match risk.

## Sharp heuristics

- Treat package cycles and global singletons as design smells unless explicitly justified.
- Treat dropped contexts as reliability bugs.
- Treat generated protobuf/OpenAPI changes as API contract changes requiring contract review.

## Finding requirements

Each finding must cite evidence IDs, package/API owner, design consequence, required fix, acceptance criteria, and confidence.
