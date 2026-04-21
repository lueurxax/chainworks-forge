# API Contract Review Rubric

Use for public or shared request/response, protobuf, OpenAPI, GraphQL, event, webhook, schema, DTO, or generated-client changes.

## Focus areas

- Compatibility: additive vs breaking changes, required fields, enum expansion, unknown fields, default behavior, and versioning.
- Error model: status codes, error bodies, retryability, localization, and client-visible semantics are stable.
- Consumer impact: Apple clients, Rust services, Go services, integrations, jobs, and dashboards are identified.
- Migration: dual-read/write, backfill, deprecation, rollout, and rollback are credible.
- Idempotency/pagination: request identity, ordering, cursors, limits, and duplicate handling are defined.

## Sharp heuristics

- Treat enum changes as compatibility risks for clients with exhaustive switches.
- Treat making optional fields required as breaking unless every caller migration is owned.
- Treat server-only schema claims as suspect when generated clients or stored events exist.

## Finding requirements

Each finding must cite evidence IDs, affected contract, incompatible consumer behavior, required fix, acceptance criteria, and confidence.
