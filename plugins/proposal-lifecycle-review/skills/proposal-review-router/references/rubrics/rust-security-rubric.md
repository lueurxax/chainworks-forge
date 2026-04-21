# Rust Security Review Rubric

Use for Rust proposals touching auth, secrets, public boundaries, parsing, permissions, unsafe, FFI, or sensitive data.

## Focus areas

- Authn/authz: caller identity, capability checks, tenant/project scope, and failure modes are explicit.
- Secrets/PII: storage, logs, traces, errors, journals, and artifacts do not leak sensitive values.
- Parsing/validation: untrusted input, JSON/protobuf/YAML parsing, path handling, and command execution are hardened.
- Unsafe/FFI: invariants, memory safety assumptions, and dependency boundaries are justified.
- Supply chain: new crates, build scripts, native dependencies, and feature flags have risk review.

## Sharp heuristics

- Treat audit logs and command journals as potential secret sinks.
- Treat path joins and external commands as trust boundaries.
- Treat "operator-only" claims as incomplete unless capability mapping and tests are named.

## Finding requirements

Each finding must cite evidence IDs, trust boundary, exploit or leakage path, required fix, acceptance criteria, and confidence.
