# Proposal Implementation Audit Report

## 0. Metadata
| Field | Value |
|---|---|
| Proposal | `docs/proposals/RUST_QUEUE_RETRY.md` |
| Proposal state | Active |
| Implementation target | current PR diff |
| Compare base | `origin/main` |
| Repository root | `/repo` |
| Git SHA | `abc1234` |
| Working tree status | modified files present |
| Audit timestamp | `2026-04-19T09:12:00+03:00` |
| Report path | `docs/proposals/RUST_QUEUE_RETRY_IMPLEMENTATION_AUDIT_R2.md` |
| Platform/product scope | worker/service |

## 1. Verdict
- Overall Conformance: Partial
- Overall Implementation Readiness: Not Ready
- Reviewer Selection Reuse: Reused with delta
- Audit Confidence: Medium
- Same-tree full regression / canonical gate: Not Run because visible conformance and rollout gaps already block a successful verdict
- Highest-risk blockers:
  1. Retry behavior is implemented, but idempotency is not proven for replay after worker restart.
  2. The proposal-review rollout concern was not addressed: the new worker path has metrics but no rollback/disable control.
  3. The protobuf contract was regenerated for the Rust service but not for the Go gateway client.

## 2. Prior Proposal-Review Reuse
- Prior artifacts found: `docs/proposals/RUST_QUEUE_RETRY.review/final-review.md`, `docs/proposals/RUST_QUEUE_RETRY.review/evidence-pack.md`
- Prior selected reviewers: `rust_arch_reviewer`, `rust_reliability_reviewer`, `api_contract_reviewer`
- Prior rejected close alternatives: `rust_performance_reviewer`, `product_reviewer`
- Prior stacks / surfaces / risks: Rust backend, worker, retry, idempotency, protobuf contract, rollout risk
- Prior required changes before implementation: specify idempotency key ownership; keep protobuf compatibility; add rollout guardrail
- Reuse decision: reused prior reviewers and added `observability_rollout_reviewer`
- Delta from prior selection: implementation touched `config/flags.yaml` and metrics paths, making rollout review concrete rather than optional
- Reasoning: current diff still matches the proposal-review fingerprint, but implementation introduced operational controls that require audit.

## 3. Current Reviewer Routing
| Reviewer | Discipline / Stack | Why Selected | Evidence IDs | Reused From Proposal Review? | Notes |
|---|---|---|---|---|---|
| `rust_arch_reviewer` | Rust architecture | Worker module and trait boundary changed | MAP-01, MAP-02 | Yes | Checks crate/module and async boundaries |
| `rust_reliability_reviewer` | Rust reliability | Retry/idempotency/deadline behavior is central | REQ-002, MAP-03, TEST-02 | Yes | Highest-risk reviewer |
| `api_contract_reviewer` | API contract | Protobuf message changed | API-01, API-02 | Yes | Go gateway regeneration gap found |
| `observability_rollout_reviewer` | Rollout/ops | Metrics and flag config touched | OPS-01, OPS-02 | Delta | Added from implementation evidence |

### Rejected Close Alternatives
| Reviewer | Why Rejected | Evidence IDs |
|---|---|---|
| `rust_performance_reviewer` | No proposal latency/throughput target and no hot-path optimization in diff | MAP-01 |
| `product_reviewer` | No new metric decision gate beyond operational rollout; covered by rollout reviewer | PRV-01, OPS-01 |

## 4. Proposal Contract Summary
- In scope: retry worker redesign for failed queue jobs, idempotency, protobuf retry reason, rollout metrics
- Out of scope: changing the public HTTP API, changing queue storage engine
- Platform/product scope: worker/service
- Locked decisions: idempotency key is owned by the job envelope; retries must be deadline-bound
- Primary implementation flows: enqueue failed job → retry worker picks it up → idempotency check → downstream call → success/failure metric
- API / schema commitments: add optional `retry_reason` field without breaking older clients
- Reliability commitments: bounded retry, no duplicate side effects after restart
- Rollout / telemetry commitments: retry attempts and terminal failures are observable

## 5. Implementation Evidence Summary
- Changed files inspected: `crates/worker/src/retry.rs`, `crates/worker/src/job.rs`, `proto/jobs.proto`, `config/flags.yaml`, `gateway/gen/jobs.pb.go`
- Adjacent files inspected: `crates/worker/src/shutdown.rs`, `crates/worker/tests/retry_worker.rs`
- Tests found: worker retry unit tests, protobuf generation smoke test
- Tests run: `cargo test -p worker retry_worker`
- API/schema/migration checks: protobuf Rust generated code updated; Go generated gateway file stale
- Evidence gaps: no restart/replay test; no flag rollback test

## 6. Proposal Fidelity / Divergence
### Matches
- Retry loop is deadline-bound.
- `retry_reason` exists in `proto/jobs.proto` as an optional-compatible field.
- Retry attempt and terminal failure metrics exist.

### Divergences
- Idempotency is checked after deserializing the job body, not before side-effect reconstruction as the proposal required.
- Rollout control is declared in config but not wired into worker startup.

### Ambiguities / Evidence Gaps
- No test proves duplicate suppression after process restart.
- No evidence that Go gateway generated code was refreshed from the new proto.

## 7. Requirement Summary
| Status | Count |
|---|---:|
| Implemented | 2 |
| Partially Implemented | 2 |
| Missing | 1 |
| Not Verifiable | 1 |
| Out of Scope | 0 |

## 8. Requirement Audit

### REQ-001 Retry loop is bounded by job deadline
- Proposal Source: `Reliability Requirements` (`docs/proposals/RUST_QUEUE_RETRY.md:31`)
- Status: Implemented
- Implementation Mapping: `crates/worker/src/retry.rs:71-114`
- Evidence Type: code, tests-run
- Evidence:
  - `crates/worker/src/retry.rs:71-114`
  - `cargo test -p worker retry_worker::deadline_bounds_retry` passed
- Gap / Note: None.

### REQ-002 Duplicate side effects are prevented after worker restart
- Proposal Source: `Locked Decisions` (`docs/proposals/RUST_QUEUE_RETRY.md:42`)
- Status: Not Verifiable
- Implementation Mapping: `crates/worker/src/retry.rs:116-148`, `crates/worker/tests/retry_worker.rs`
- Evidence Type: code, tests-found
- Evidence:
  - `crates/worker/src/retry.rs:116-148`
  - no restart/replay test found
- Gap / Note: Code appears to check idempotency, but the audit found no proof for restart/replay behavior.

### REQ-003 Protobuf retry reason remains backward-compatible
- Proposal Source: `API Compatibility` (`docs/proposals/RUST_QUEUE_RETRY.md:55`)
- Status: Partially Implemented
- Implementation Mapping: `proto/jobs.proto`, generated Rust and Go clients
- Evidence Type: schema, code
- Evidence:
  - `proto/jobs.proto:18`
  - `crates/proto/src/jobs.rs:203`
  - `gateway/gen/jobs.pb.go` lacks `RetryReason`
- Gap / Note: Schema and Rust generated code were updated, but Go gateway generated code is stale.

### REQ-004 Retry rollout can be disabled without redeploying the worker
- Proposal Source: `Rollout Plan` (`docs/proposals/RUST_QUEUE_RETRY.md:63`)
- Status: Missing
- Implementation Mapping: `config/flags.yaml`, `crates/worker/src/main.rs`
- Evidence Type: config, code
- Evidence:
  - `config/flags.yaml:22`
  - `crates/worker/src/main.rs:49-80`
- Gap / Note: Flag exists in config but is not read by the worker startup path.

## 9. Prior Review Finding Follow-Through
| Prior Finding / Required Change | Status | Evidence | Notes |
|---|---|---|---|
| Specify idempotency key ownership | Addressed | `crates/worker/src/job.rs:28` | Job envelope owns the key |
| Keep protobuf compatibility across Rust and Go consumers | Partially Addressed | `proto/jobs.proto`, `gateway/gen/jobs.pb.go` | Go generated file stale |
| Add rollout guardrail | Not Addressed | `config/flags.yaml`, `crates/worker/src/main.rs` | Flag not wired |

## 10. Reviewer Scorecard
| Reviewer | Result | Confidence | Evidence Completeness | Critical | Major | Minor | Notes |
|---|---|---|---|---:|---:|---:|---|
| `rust_arch_reviewer` | Pass with Issues | Medium | Partial | 0 | 1 | 0 | Boundary is mostly coherent but replay seam is weak |
| `rust_reliability_reviewer` | Fail | Medium | Partial | 0 | 2 | 0 | Restart/replay proof missing |
| `api_contract_reviewer` | Fail | High | Strong | 0 | 1 | 0 | Go generated client stale |
| `observability_rollout_reviewer` | Fail | High | Strong | 0 | 1 | 0 | Disable flag not wired |

## 11. Routed Specialist Findings

### 11.1 Major

#### REL-001 Restart/replay idempotency is not proven
- Reviewer: `rust_reliability_reviewer`
- Severity: Major
- Confidence: Medium
- Related Proposal Items / REQs: REQ-002
- Evidence Type: code, tests-found
- Evidence:
  - `crates/worker/src/retry.rs:116-148`
  - `crates/worker/tests/retry_worker.rs`
- Why It Matters: The dangerous case is not normal retry in one process; it is a restart after a partially completed side effect.
- Recommended Action: Add a restart/replay test that proves duplicate suppression across persisted job state.
- Acceptance Criteria: A focused test fails before the fix and passes with persisted idempotency state after restart.

#### API-001 Go gateway generated client is stale after protobuf change
- Reviewer: `api_contract_reviewer`
- Severity: Major
- Confidence: High
- Related Proposal Items / REQs: REQ-003
- Evidence Type: schema, code
- Evidence:
  - `proto/jobs.proto:18`
  - `gateway/gen/jobs.pb.go`
- Why It Matters: The proposal is cross-service; updating only the Rust side creates a hidden contract drift.
- Recommended Action: Regenerate Go gateway bindings and run the gateway contract tests.
- Acceptance Criteria: `RetryReason` appears in Go generated code and gateway contract tests pass.

#### OPS-001 Retry rollout flag is declared but not wired
- Reviewer: `observability_rollout_reviewer`
- Severity: Major
- Confidence: High
- Related Proposal Items / REQs: REQ-004
- Evidence Type: config, code
- Evidence:
  - `config/flags.yaml:22`
  - `crates/worker/src/main.rs:49-80`
- Why It Matters: Operators cannot disable the new worker path without another deploy, which violates the rollout plan.
- Recommended Action: Read the flag during worker startup or retry dispatch and add a rollback-path test.
- Acceptance Criteria: Disabling the flag prevents the new retry path and emits a clear metric/log signal.

## 12. Readiness Checklist
| Check | Status | Evidence / Note |
|---|---|---|
| Proposal contract satisfied | Partial | Two requirements partial/missing/not verifiable |
| Prior review blockers addressed | Partial | Idempotency ownership addressed; rollout and cross-language generation not complete |
| Tests cover committed behavior | Partial | No restart/replay test |
| Critical tests executed | Partial | Rust worker tests run; Go gateway contract tests not run |
| API/schema compatibility acceptable | Fail | Go generated client stale |
| Migration/rollback path acceptable | Fail | Disable flag not wired |
| Telemetry/observability sufficient | Partial | Metrics exist, but rollback signal absent |
| Security/privacy risk acceptable | Not Checked | Not central to this proposal |
| Performance risk acceptable | Not Checked | No performance commitment |
| Full regression suite or canonical full/proposal gate passed on audited tree/HEAD | Not Run | Not required to prove failure; must pass before any successful verdict |
| Release/handoff evidence sufficient | Fail | Contract and rollout gaps remain |

## 14. Verification Log
- `cargo test -p worker retry_worker` passed
- inspected `proto/jobs.proto`, `crates/proto/src/jobs.rs`, `gateway/gen/jobs.pb.go`
- inspected `config/flags.yaml`, `crates/worker/src/main.rs`
- did not run Go gateway tests because generated code was visibly stale

## 15. Recommended Next Actions
- MUST-01: Wire the rollout flag into worker startup or retry dispatch.
- MUST-02: Regenerate Go gateway protobuf code and run gateway contract tests.
- MUST-03: Add restart/replay idempotency coverage.
