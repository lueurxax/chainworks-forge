# Reviewer Selection Playbook

Use this after the evidence pack has a fingerprint.

## Selection principles

- Prefer specialists over generic review.
- Select the smallest reviewer set that covers the evidence-backed risks.
- Use one primary architecture reviewer per active implementation stack.
- Add reliability, performance, security, API, rollout, or product reviewers only when evidence warrants them.
- Record why close alternatives were not selected.

## Hard triggers

Select these reviewers when evidence proves the trigger:

| Trigger | Reviewer |
|---|---|
| iOS UI, navigation, or visual state | `ios_ui_reviewer` |
| macOS UI, window, command, toolbar, sidebar, or desktop interaction | `macos_ui_reviewer` |
| Apple client state, data flow, lifecycle, concurrency, navigation ownership, persistence | `apple_arch_reviewer` |
| Apple trust, recovery, accessibility, destructive flow, or complex user journey | `apple_ux_reviewer` |
| Rust crate/service/runtime/persistence/API implementation | `rust_arch_reviewer` |
| Rust retry, timeout, queue, worker, cancellation, idempotency, shutdown, backpressure | `rust_reliability_reviewer` |
| Rust latency, throughput, allocation, lock contention, streaming, serialization hot path | `rust_performance_reviewer` |
| Rust auth, secrets, unsafe, FFI, parsing, public boundary, permissions | `rust_security_reviewer` |
| Go service, handler, worker, package, persistence, transport, microservice | `go_service_arch_reviewer` |
| Go context, goroutine, retry, deadline, queue, shutdown, backpressure | `go_reliability_reviewer` |
| Go GC, allocation, serialization, batching, pooling, throughput, latency | `go_performance_reviewer` |
| Go auth, secrets, validation, webhook, public endpoint, SSRF-like egress, permissions | `go_security_reviewer` |
| Public API, protobuf, OpenAPI, GraphQL, DTO, event, schema compatibility | `api_contract_reviewer` |
| Feature flag, migration, telemetry, rollout, rollback, alerting, SLO, support diagnostics | `observability_rollout_reviewer` |
| Explicit product request or central KPI, experiment, prioritization, decision checkpoint | `product_reviewer` |

## Cross-stack patterns

Apple client plus backend contract:

- select the relevant Apple reviewer or `apple_arch_reviewer`
- select the relevant backend architecture reviewer
- select `api_contract_reviewer`
- add `observability_rollout_reviewer` only if rollout, telemetry, migration, or rollback changes

Rust worker plus protobuf plus rollout:

- select `rust_arch_reviewer`
- select `rust_reliability_reviewer` when queue/retry/idempotency exists
- select `api_contract_reviewer`
- select `observability_rollout_reviewer`

Go public endpoint plus auth plus migration:

- select `go_service_arch_reviewer`
- select `go_security_reviewer`
- select `api_contract_reviewer`
- select `observability_rollout_reviewer`

macOS UI-only cleanup:

- select `macos_ui_reviewer`
- add `apple_ux_reviewer` only if trust, accessibility, or journey clarity is central
- do not add backend reviewers

## Scoring optional reviewers

Start with registry weights, then adjust:

- Add explicit user request points only when the user asked for that discipline.
- Add stack points only for proven stack tags.
- Add surface points only for proven surface tags.
- Add risk points only for proven risk tags.
- Add repo-signal points when current files or symbols match.
- Add cross-stack points when the reviewer covers a seam between selected stacks.
- Apply overlap penalty when another selected reviewer already owns the same risk with better evidence.

## Cap management

If more than 5 reviewers score above threshold:

1. Keep primary architecture reviewers for active implementation stacks.
2. Keep cross-cutting reviewers for explicit API or rollout seams.
3. Keep security over performance when both are weakly evidenced but auth/public boundary exists.
4. Keep reliability over performance when retry/idempotency/background work exists.
5. Drop product unless explicitly requested or metrics are central.
6. Record rejected close alternatives.

## Anti-patterns

- Selecting all reviewers from a detected stack.
- Adding product review because every product has users.
- Adding performance review without hot-path, latency, throughput, or resource evidence.
- Adding security review without a trust boundary, auth, secrets, parsing, public endpoint, or permission evidence.
- Treating missing build/run proof as a proposal-readiness finding.
- Starting research before local evidence can produce narrow questions.
