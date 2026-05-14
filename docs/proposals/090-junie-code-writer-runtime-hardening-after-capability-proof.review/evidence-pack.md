# Proposal Evidence Pack

Proposal: `docs/proposals/090-junie-code-writer-runtime-hardening-after-capability-proof.md`  
Mode: `auto`  
Reviewed on: `2026-05-14`  
Reviewer router version: `proposal-review-router`  
Reviewed proposal md5: `e6f4a176751fffe415aeed362041a0bb`  
Working tree note: `dirty before review; unrelated Swift support change and other proposal artifacts were not used as P090 evidence`

## A. Repo-local proposal and document inventory

| Evidence ID | Source / path / artifact | Verified on | Confidence | Key fact | Risk if wrong | Relevance |
|---|---|---:|---|---|---|---|
| DOC-01 | `docs/proposals/090-junie-code-writer-runtime-hardening-after-capability-proof.md:57` | 2026-05-14 | High | P090 still treats P036 shapes as design inputs requiring durable historical or synthetic evidence per subtype. | Readiness could overclaim evidence. | Evidence inventory. |
| DOC-02 | `docs/proposals/090-junie-code-writer-runtime-hardening-after-capability-proof.md:237` | 2026-05-14 | High | Failure envelopes are now engine-synthesized receipt/readback shapes; provider-authored envelope-shaped JSON is untrusted and can only contribute after validation. | Provider spoofing could become authoritative. | Trust boundary. |
| DOC-03 | `docs/proposals/090-junie-code-writer-runtime-hardening-after-capability-proof.md:354` | 2026-05-14 | High | `completion_boundary_subtype` is now a provider-neutral public wrapper with Junie-prefixed initial known values. | Public API could overfit Junie. | API contract. |
| DOC-04 | `docs/proposals/090-junie-code-writer-runtime-hardening-after-capability-proof.md:411` | 2026-05-14 | High | P090 adds staged per-output repair settlement with a proposed `code_writer_output_settlement_rows` table and validate-before-materialize flow. | Repair implementation could remain all-or-nothing or write before validation. | Settlement truth. |
| DOC-05 | `docs/proposals/090-junie-code-writer-runtime-hardening-after-capability-proof.md:551` | 2026-05-14 | High | Junie preflight lifecycle is now specified, including preflight states, provider capacity timing, one remediation attempt, and no-launch terminal failure. | Launch lifecycle could be ambiguous. | Runtime reliability. |
| DOC-06 | `docs/proposals/090-junie-code-writer-runtime-hardening-after-capability-proof.md:612` | 2026-05-14 | High | P090 adds receipt/readback fields for subtype, final payload, transcript status, repair materialization, and runtime preflight. | Migration/readback work could be incomplete. | Data/API. |
| DOC-07 | `docs/proposals/090-junie-code-writer-runtime-hardening-after-capability-proof.md:830` | 2026-05-14 | High | Acceptance now includes evidence index, canonical `proposal-090` gate, spoof/mismatch fixtures, preflight fixtures, and per-output settlement transaction fixtures. | Gate may not enforce the full acceptance scope. | Proof gate. |
| DOC-08 | `docs/proposals/090-junie-code-writer-runtime-hardening-after-capability-proof.md:955` | 2026-05-14 | Medium | Rollout has four steps and says partial repair materialization shares the P090 rollout flag, with a separate emergency disable only for staged repair materialization. | Flag/rollback behavior could be ambiguous. | Rollout/rollback. |

## B. Reusable baseline inputs

| Evidence ID | Artifact / slice | Status | Covered surfaces | Verified on | Confidence | Freshness notes | Relevance |
|---|---|---|---|---:|---|---|---|
| BASE-01 | `.review-baselines/current-system-baseline.md` | Stale | Review setup | 2026-05-14 | High | Local reusable baseline still contains outdated Goose/provider wording. | Consumed but not treated as current truth. |
| BASE-02 | `docs/reference/current-system-baseline.md` | Reused | Current system map | 2026-05-14 | High | Current reference baseline includes live ACP execution, GraphQL thin UI, recovery, SQLite write serialization, and current provider families. | Current review context. |
| REF-01 | `docs/reference/execution-truth-and-recovery.md:67` | Reused | Execution truth | 2026-05-14 | High | Persisted execution-truth columns outrank envelopes and receipts. | Engine-owned failure envelope authority. |
| REF-02 | `docs/reference/output-contracts-failure-evidence-and-recovery.md:50` | Reused | Output contracts | 2026-05-14 | High | Required outputs flow through one materialization path; exact paths/legacy envelopes are compatibility evidence only. | Per-output settlement fit. |
| REF-03 | `docs/reference/acp-runtime-transport.md:138` | Reused | ACP runtime | 2026-05-14 | High | P089 proves bounded Junie structured-output and ACP canary, not long-running P036-class reliability. | P090 problem scope. |

## C. Prior proposal artifacts consumed

| Evidence ID | Artifact | Status | Key fact | Relevance |
|---|---|---|---|---|
| ART-01 | prior P090 review answer | Superseded | Prior reviewed md5 was `965444e7b85b805d2f8e9393ac78f192`; current md5 is `e6f4a176751fffe415aeed362041a0bb`. | Fresh review required by md5 guard. |
| ART-02 | `docs/evidence/090/junie-runtime-hardening/evidence-index.json` | Reused | Evidence index exists, lists all seven subtypes, records fixture paths/SHA-256, provider-neutral subtype contract, and required negative fixture classes. | Evidence inventory. |
| ART-03 | `docs/evidence/090/junie-runtime-hardening/fixtures/*.json` | Reused | All seven subtype fixture files exist and SHA-256 values match the evidence index. | Subtype coverage. |
| ART-04 | `./scripts/test-gate.sh proposal-090` | Verified | Gate passed locally on 2026-05-14: evidence inventory validation passed. | Readiness gate status. |

## D. Current repo / code-path map

| Evidence ID | Surface / entry point | File / module / manifest | Layer | Key fact | Risk if wrong | Relevance |
|---|---|---|---|---|---|---|
| MAP-01 | Current receipt domain model | `control-plane/crates/domain/src/code_writer_completion.rs:6` | Domain/API | Current P088 receipt has no P090 fields yet; `session_generation_id` is optional. | P090 migration may conflict with existing nullable session generation. | Data model delta. |
| MAP-02 | Current implementation completion summary | `control-plane/crates/domain/src/code_writer_completion.rs:159` | Domain/API | Public completion summary currently exposes P088 fields and generic enum wrappers only. | P090 readback fields need additive compatibility. | API contract. |
| MAP-03 | Current GraphQL readback | `control-plane/crates/graphql-server/src/types/run.rs:220` and `:365` | GraphQL API | GraphQL exposes P088 receipt/summary fields only, not P090 subtype/preflight/materialization fields. | Public readback work remains implementation scope. | API contract. |
| MAP-04 | Current repair merge behavior | `control-plane/crates/engine/src/executor.rs:5594` | Engine | Repair settlement is merged only when overall validation succeeds; otherwise repair is recorded as failed missing outputs. | Confirms why P090 needs per-output settlement. | Reliability. |
| MAP-05 | Current P088 receipt table shape | `control-plane/crates/db/migrations/051_p088_code_writer_completion_receipts.sql:65` and `:132` | DB migration | Current completion receipts are unique per `agent_execution_id`; output decisions are keyed by `(receipt_id, output_name)`. | New settlement rows need a stable link to receipt/readback and idempotency keys. | Persistence. |
| MAP-06 | Current receipt upsert conflict behavior | `control-plane/crates/db/src/repos/code_writer_completion_receipts.rs:44` | DB repo | Receipt upsert rejects conflicts when receipt/captures/output decisions differ for an existing execution. | Additional settlement rows must not diverge from receipt output decisions. | Idempotency. |
| MAP-07 | Current AgentStatus vocabulary | `control-plane/crates/domain/src/agent.rs:8` | Domain/API | AgentStatus only supports running/completed/failed/cancelled; P090 text names `preflight_running`, `preflight_remediating`, and `launching` as AgentExecution row states. | New raw statuses would break parsing/readback unless modeled separately or migrated everywhere. | Runtime lifecycle. |
| MAP-08 | Current ACP completion capture | `control-plane/crates/acp/src/transport.rs:1049` and `:1137` | ACP transport | Existing capture selection couples selected extraction input and truncation metadata. | P090 final-payload/transcript split is new implementation work. | Completion boundary. |
| MAP-09 | Current Junie adapter | `control-plane/crates/acp/src/adapters/junie.rs:48` | ACP adapter | Junie adapter launches `junie --acp true` and has no tool-path/runtime-home preflight today. | P090 preflight is new implementation work. | Runtime reliability. |
| MAP-10 | P090 gate implementation | `scripts/test-gate.sh:6981` | Test gate | `proposal-090|p090` validates index, subtype fixtures, required negative classes, proposal required terms, and test-gates doc. | Gate exists but negative classes are not checked as concrete fixture files. | Proof gap. |

## E. Fingerprint summary

| Tag type | Tag | Evidence IDs | Reason |
|---|---|---|---|
| Stack | `rust-backend` | DOC-04, DOC-05, MAP-04, MAP-09 | P090 changes Rust engine, ACP transport/adapter, DB, and runtime lifecycle behavior. |
| Stack | `shared-api` | DOC-03, DOC-06, MAP-02, MAP-03 | Receipt, GraphQL, MCP, and report readback fields change. |
| Stack | `cross-stack` | DOC-06, REF-03 | Operator-facing diagnostics depend on server readback and runtime evidence. |
| Surface | `api-contract` | DOC-03, DOC-06, MAP-02, MAP-03 | Public enum/readback additions are required. |
| Surface | `persistence` | DOC-04, DOC-06, MAP-05 | New settlement rows and receipt fields are persisted truth. |
| Surface | `migration` | DOC-06, MAP-01, MAP-05 | Additive schema changes need legacy compatibility. |
| Surface | `background-work` | DOC-05, MAP-09 | Preflight changes work-item/provider launch lifecycle. |
| Surface | `security-boundary` | DOC-02, MAP-08 | Provider-authored completion text is untrusted input. |
| Surface | `rollout` | DOC-08, ART-04, MAP-10 | P090 now has a readiness gate and rollout flag language. |
| Surface | `telemetry` | DOC-06, DOC-07 | New subtype/readback fields drive operator diagnosis. |
| Risk | `backward-compatibility` | DOC-03, DOC-06, MAP-02 | Old receipts and unknown enum values must remain readable. |
| Risk | `idempotency` | DOC-04, MAP-05, MAP-06 | Per-output settlement and repair retry need stable keys and receipt consistency. |
| Risk | `data-loss` | DOC-04, MAP-04 | Validate-before-materialize protects canonical outputs from malformed repair siblings. |
| Risk | `security-sensitive` | DOC-02, ART-02, MAP-08 | Spoofed provider failure envelopes must not become authoritative. |
| Risk | `availability-sensitive` | DOC-05, MAP-09 | Preflight decides whether provider launch is blocked. |
| Risk | `operability-sensitive` | DOC-07, ART-04, MAP-10 | Operators need precise subtype and gate-backed evidence. |

## F. Routing decision

Selected reviewers:

| Reviewer ID | Mode | Evidence IDs | Why selected | Repo-local agent used? |
|---|---|---|---|---|
| `chainworks_execution_truth_reviewer` | architecture-only | DOC-02, DOC-04, REF-01, MAP-04, MAP-07 | Proposal changes durable execution, repair, preflight, and receipt truth. | No, rubric used in main thread. |
| `rust_reliability_reviewer` | reliability-only | DOC-04, DOC-05, MAP-04, MAP-06, MAP-09 | Proposal changes repair idempotency, provider launch preflight, and failure lifecycle. | No, rubric used in main thread. |
| `api_contract_reviewer` | api-contract-only | DOC-03, DOC-06, MAP-01, MAP-02, MAP-03 | Proposal changes public receipt/GraphQL/MCP/report contracts and enum wrappers. | No, rubric used in main thread. |
| `observability_rollout_reviewer` | observability-rollout-only | DOC-07, DOC-08, ART-02, ART-04, MAP-10 | Proposal readiness depends on evidence inventory, gate coverage, and rollout controls. | No, rubric used in main thread. |

Rejected close alternatives:

| Reviewer ID | Evidence IDs | Why not selected |
|---|---|---|
| `rust_security_reviewer` | DOC-02, ART-02 | Security-sensitive spoofing is real, but it is now specified as an API/execution-truth fail-closed contract and covered by selected reviewers. |
| `rust_arch_reviewer` | DOC-04, DOC-05 | Covered by execution-truth and reliability for this targeted runtime boundary. |
| `apple_arch_reviewer` | MAP-03 | Swift remains read-side consumer; changes are server-owned readback/runtime work. |
| `product_reviewer` | DOC-08 | No product metric, adoption experiment, or prioritization decision is central. |

Routing cap status: `4 selected; target 2-4; hard cap 5.`

## G. State and failure coverage matrix

| State / failure class | Proposal coverage | Evidence IDs | Gap / risk | Reviewer owner |
|---|---|---|---|---|
| Entry / setup | Partial | DOC-05, MAP-07, MAP-09 | Preflight lifecycle names AgentExecution states not in current enum. | api_contract_reviewer |
| Happy path | Ready | DOC-03, REF-03 | P089 capability baseline remains out of scope and preserved. | chainworks_execution_truth_reviewer |
| Loading / in-flight | Partial | DOC-05, MAP-07 | Need model preflight phase without breaking AgentStatus parsing. | rust_reliability_reviewer |
| Timeout / cancellation | Not directly covered | DOC-05 | P090 does not target cancellation semantics. | n/a |
| Retry / replay / idempotency | Partial | DOC-04, MAP-05, MAP-06 | Settlement row idempotency and receipt linkage need tightening. | rust_reliability_reviewer |
| Persistence / migration | Partial | DOC-04, DOC-06, MAP-05 | Settlement rows need FK/unique strategy relative to P088 receipts/output decisions. | api_contract_reviewer |
| Auth / permission failure | Ready with conditions | DOC-05, DOC-07 | Preflight permission-denied no-launch path is specified; concrete fixture coverage still needs implementation-mode tests. | observability_rollout_reviewer |
| Dependency failure | Ready with conditions | DOC-05 | Wrong cwd/runtime-home remediation is scoped. | rust_reliability_reviewer |
| Rollback / recovery | Partial | DOC-08 | Rollout flags and emergency disable behavior need names/defaults and downgrade behavior. | observability_rollout_reviewer |
| Observability / support | Partial | DOC-07, ART-04, MAP-10 | Readiness gate passes, but negative spoof/mismatch fixtures are listed as classes, not concrete files. | observability_rollout_reviewer |

## H. Proposal completeness matrix

| Dimension | Status | Evidence IDs | Notes |
|---|---|---|---|
| Problem and target user | Ready | DOC-01, REF-03 | Clearly separates P089 capability from long-running reliability/handoff failures. |
| Scope and non-goals | Ready | DOC-01, DOC-02 | Scope is runtime/completion boundary hardening. |
| Current-system fit | Ready with conditions | MAP-04, MAP-08, MAP-09 | Proposal targets real current gaps; implementation details below need tightening. |
| Data / state model | Partial | DOC-04, DOC-06, MAP-01, MAP-05 | Settlement rows and preflight lifecycle need exact DB/API shape. |
| API / contract compatibility | Partial | DOC-03, DOC-06, MAP-02, MAP-03 | Provider-neutral wrapper is now settled; AgentStatus/preflight phase needs clarity. |
| Runtime / concurrency semantics | Partial | DOC-05, MAP-07 | Provider capacity timing is specified; phase modeling needs compatibility. |
| Failure handling | Ready with conditions | DOC-02, DOC-05, DOC-07 | Fail-closed semantics are strong; negative fixtures need concrete artifacts or implementation tests. |
| Security / privacy / auth | Ready with conditions | DOC-02, ART-02 | Spoofing boundary is specified, but readiness gate checks class names only. |
| Migration / rollout / rollback | Partial | DOC-08 | Feature flags/rollback controls need exact names/defaults. |
| Observability / diagnostics | Ready with conditions | DOC-06, ART-04 | Readback target is clear. |
| Test / proof gate | Ready with conditions | DOC-07, ART-04, MAP-10 | Readiness gate passes; implementation-mode gate must add Rust/API tests. |
| Product metrics / decision checkpoint | Not applicable | DOC-08 | Product reviewer not selected. |

## I. Evidence gaps and fallback decisions

| Gap ID | Missing evidence | Blocks routing or finding? | Next local artifact or file to inspect | Integration-context refresh needed? |
|---|---|---|---|---|
| GAP-01 | Concrete negative fixture files for provider spoof, identity mismatch, and unknown schema. | Does not block routing; supports P1 gate finding. | Add `negative_fixtures` entries with paths/SHA or implementation-mode tests before enforcement. | No. |
| GAP-02 | Exact preflight phase storage model. | Does not block routing; supports P1 API/lifecycle finding. | Proposal should decide AgentStatus expansion vs runtime-facts/preflight-phase field. | No. |
| GAP-03 | Exact rollout flag names/defaults. | Does not block routing; supports P2 rollout finding. | Add named flags and disable semantics to rollout section. | No. |

## J. Research triggers

| Trigger ID | Local evidence IDs | Question local evidence cannot settle | Required source type | Why it matters |
|---|---|---|---|---|
| RES-01 | None | None. | N/A | Local evidence is sufficient for this proposal-readiness review. |

## K. Findings ledger

| Finding ID | Reviewer | Severity | Evidence IDs | File / lines | Summary | Confidence |
|---|---|---|---|---|---|---|
| F-01 | observability_rollout_reviewer, api_contract_reviewer | P1 | DOC-02, DOC-07, ART-02, ART-04, MAP-10 | `scripts/test-gate.sh:6981`, `docs/proposals/090-junie-code-writer-runtime-hardening-after-capability-proof.md:918` | Readiness gate validates negative fixture class names but not concrete spoof/mismatch fixture artifacts. | 0.9 |
| F-02 | api_contract_reviewer, chainworks_execution_truth_reviewer | P1 | DOC-05, DOC-06, MAP-01, MAP-07 | `docs/proposals/090-junie-code-writer-runtime-hardening-after-capability-proof.md:562`, `control-plane/crates/domain/src/agent.rs:8` | Preflight lifecycle uses new AgentExecution row states that do not exist in current AgentStatus. | 0.88 |
| F-03 | rust_reliability_reviewer, api_contract_reviewer | P2 | DOC-04, MAP-05, MAP-06 | `docs/proposals/090-junie-code-writer-runtime-hardening-after-capability-proof.md:430`, `control-plane/crates/db/migrations/051_p088_code_writer_completion_receipts.sql:132` | New per-output settlement rows lack explicit receipt linkage/unique idempotency constraints relative to P088 receipt output decisions. | 0.82 |
| F-04 | observability_rollout_reviewer | P2 | DOC-08 | `docs/proposals/090-junie-code-writer-runtime-hardening-after-capability-proof.md:955` | Rollout/rollback controls are conceptually named but not operationally specified as concrete flags/defaults. | 0.78 |
