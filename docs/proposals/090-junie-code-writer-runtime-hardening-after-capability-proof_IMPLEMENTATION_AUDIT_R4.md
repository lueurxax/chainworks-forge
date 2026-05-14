# Proposal 090 Implementation Audit R4

## Metadata

Audit timestamp: `2026-05-14T19:30:17Z`  
Proposal: `docs/proposals/090-junie-code-writer-runtime-hardening-after-capability-proof.md`  
Proposal md5: `f98170c78ca39398e9aaed497180c057`  
Proposal state: `Active` / `Draft`  
Repo root: `/Users/user/Documents/Chainworks Forge`  
Implementation target: current worktree on branch `main`  
Current HEAD: `225bac4e47b135d92c4fe2de243dd13c4647be19`  
Compare base: implicit current worktree, including uncommitted P090/P091 changes and prior audit artifacts  
Report path: `docs/proposals/090-junie-code-writer-runtime-hardening-after-capability-proof_IMPLEMENTATION_AUDIT_R4.md`

Overall Conformance: **Partially Implemented**  
Overall Implementation Readiness: **Not Ready**

## Implementation Target

Audited the Rust control-plane implementation, persistence migration, GraphQL/MCP readbacks, Junie ACP adapter, P090 evidence inventory, and `proposal-090`/`proposal-089` gates in the current dirty worktree.

The implementation has advanced materially since the earlier P090 shape: it now includes P090 receipt fields, durable settlement rows, provider-envelope spoof/mismatch detection, active-pointer publication for the normal staged-repair path, startup settlement-row recovery, concrete negative fixtures, and a checked-in Junie refine-like canary.

## Prior-Review Reuse

Prior proposal-review artifact discovered:

- `docs/proposals/090-junie-code-writer-runtime-hardening-after-capability-proof.review/evidence-pack.md`

Reuse status: **Reused exactly**.

The evidence pack reviewed an earlier proposal md5 (`e6f4a176751fffe415aeed362041a0bb`), but its selected implementation disciplines still match the current implementation surface: execution truth, Rust reliability, API contracts, and rollout/evidence.

Selected reviewers:

- `chainworks_execution_truth_reviewer`
- `rust_reliability_reviewer`
- `api_contract_reviewer`
- `observability_rollout_reviewer`

Rejected close alternatives:

- `rust_security_reviewer`: provider-spoofing is security-sensitive, but the current audit surface is already covered by execution-truth and API-contract fail-closed review.
- `rust_arch_reviewer`: covered by execution-truth plus reliability for this runtime boundary.
- `apple_arch_reviewer`: Swift is not the implementation owner for P090.
- `product_reviewer`: no product metric or prioritization decision is central to implementation readiness.

## Contract Summary

P090 requires Junie `code_writer` runtime hardening after P089 capability proof:

- keep P089 structured-output capability passing;
- require structured final completion shape or engine-owned typed failure truth;
- reject provider-authored failure envelopes as authority, including identity mismatch and unknown schemas;
- decouple final completion payload capture from transcript persistence;
- expose provider-neutral completion boundary subtypes across DB, GraphQL, MCP, and reports;
- stage repair materialization per output before canonical writes;
- derive receipt decisions and active artifact pointers from accepted durable settlement rows;
- recover staged/committed/failed settlement rows after crash without promoting unaccepted staged files;
- add Junie preflight/remediation before launch with persisted runtime phases and redacted facts;
- gate all of the above through `./scripts/test-gate.sh proposal-090`.

## Platform And Product Scope

Apple platform scope: macOS app is a read-side consumer only for this proposal.  
Backend/service scope: Rust control-plane engine, DB, ACP adapter/runtime, GraphQL, MCP, report readback, evidence gates.  
Product scope: operator-facing diagnosis and recovery trust, not a new user workflow.

## Critical Flows Audited

1. Junie happy-path completion through `BackgroundExecutor.process_next_item`.
   Status: **Implemented enough for canary**. Evidence exists under `docs/evidence/090/junie-runtime-hardening/refine-like-canary/` and `scripts/test-gate.sh:7070-7125` validates the canary.

2. Provider-authored failure envelope spoof/mismatch.
   Status: **Implemented for classification/readback**. `control-plane/crates/engine/src/executor.rs:12117-12219` parses provider-authored failure-shaped JSON as a claim, and `:11839-11845` maps identity mismatch/unknown schema to fail-closed subtypes.

3. Partial repair materialization with malformed sibling.
   Status: **Partially Implemented**. Normal staged repair writes rows before canonical mutation and publishes accepted active generations, but crash recovery does not publish active pointers after recovering a committed row.

4. Junie tool-path preflight and remediation.
   Status: **Partially Implemented**. Adapter checks cwd/read/output/temp paths and remediates wrong cwd once, but persisted `preflight_running`/`preflight_remediating`, attempt counts, remediation facts, and runtime-home remediation are missing.

5. Evidence and rollout gate.
   Status: **Partially Ready**. `proposal-090` passes, but the live canary does not exercise staged repair mode despite the gate setting the staged flag.

## Fidelity Buckets

Implemented:

- P090 DB migration and domain/readback fields for receipts and settlement rows.
- Provider-neutral subtype readback and unknown-value wrapping.
- Provider-authored failure-envelope spoof/mismatch rejection.
- Concrete negative fixture files and SHA validation in `proposal-090`.
- Focused DB/ACP/engine/GraphQL/MCP P090 gate tests.
- Checked-in long-running Junie ACP canary evidence.

Partially implemented:

- Staged repair settlement and active-pointer publication, because the normal path publishes accepted rows but startup recovery does not republish active artifact generations.
- Junie preflight lifecycle, because launch-time checks exist but persisted phase/attempt/remediation lifecycle is not implemented as specified.
- Live canary proof, because it proves fresh output settlement through Junie ACP but not staged repair materialization.
- Truncation/progress-without-handoff proof, because helper classification and synthetic fixtures exist, but true runtime fixtures are still limited.

Missing or not verifiable:

- Runtime-home/cache remediation once before launch.
- `preflight_attempt_count` and `preflight_remediation_applied` public readback fields.
- Full regression sign-off.

## REQ Audit

| Req | Proposal commitment | Status | Evidence |
|---|---|---|---|
| REQ-001 | P089 capability proof still passes unchanged. | Implemented | `./scripts/test-gate.sh proposal-089` passed; `docs/evidence/089/junie-structured-output-canary/live-gate-run.json:2-5` records passed live evidence for current HEAD. |
| REQ-002 | Strict final completion envelope; free-form prose is not ordinary success. | Partially Implemented | Subtype logic exists in `control-plane/crates/engine/src/executor.rs:11824-11933`; focused large-narrative runtime fixture remains synthetic-only in `docs/evidence/090/.../junie-final-response-truncated.fixture.json`. |
| REQ-003 | Provider-authored failure envelopes are untrusted and identity/unknown schema fail closed. | Implemented | Proposal `:237-315`; implementation `executor.rs:12117-12219`; tests `executor.rs:14457-14549`; P090 negative fixture validation `scripts/test-gate.sh:7161-7196`. |
| REQ-004 | Separate final payload capture from forensic transcript. | Implemented | Receipt fields in `control-plane/crates/domain/src/code_writer_completion.rs:30-40`; canary has captured final payload while transcript is unavailable at `docs/evidence/090/.../harness-result.json:124-158`. |
| REQ-005 | Provider-neutral `completion_boundary_subtype` across public readback. | Implemented | Domain wrapper `code_writer_completion.rs:345-353`; GraphQL fields `graphql-server/src/types/run.rs:218-234`; gate runs GraphQL/MCP readback tests at `scripts/test-gate.sh:7228-7229`. |
| REQ-006 | Durable per-output settlement rows with receipt linkage and idempotency. | Implemented | Migration `control-plane/crates/db/migrations/053_p090_code_writer_runtime_hardening.sql:34-69`; repo validation `code_writer_completion_receipts.rs:322-369`; DB tests `proposal_088_persistence.rs:882-1060`. |
| REQ-007 | Staged per-output repair materialization and active pointers from accepted rows. | Partially Implemented | Normal path stages/commits/publishes at `executor.rs:1372-1570` and `:11287-11302`; recovery gap in REL-001 below. |
| REQ-008 | Crash recovery for staged/committed/failed settlement rows. | Partially Implemented | Startup recovery updates settlement rows at `control-plane/crates/engine/src/recovery.rs:597-665`, but does not publish active artifact pointers for recovered committed rows. |
| REQ-009 | Distinct progress-without-terminal-handoff diagnosis. | Partially Implemented | Classification helper returns `junie_progress_without_terminal_handoff` at `executor.rs:11916-11932` and progress state at `:12048-12063`; runtime no-terminal fixture remains synthetic. |
| REQ-010 | Transcript persistence failure must not erase completion truth. | Implemented | Receipt persists final capture and transcript absence separately at `executor.rs:11079-11242`; canary evidence shows `completion_status=complete` with `transcript_status=unavailable`. |
| REQ-011 | Junie preflight/remediation lifecycle before provider launch. | Partially Implemented | Adapter preflight exists at `control-plane/crates/acp/src/adapters/junie.rs:81-190`; persisted lifecycle fields and attempt/remediation readback are missing per REL-002/API-001. |
| REQ-012 | GraphQL/MCP/report compatibility for P090 fields. | Implemented | GraphQL exposes summary/receipt/settlement rows at `graphql-server/src/types/run.rs:218-234` and `:389-514`; focused GraphQL/MCP tests pass in `proposal-090`. |
| REQ-013 | Rollout controls and downgrade behavior. | Partially Implemented | Flags parse at `executor.rs:11793-11816`; requested staged-without-strict is silently disabled rather than failing closed or warning per proposal `:990-994`. |
| REQ-014 | Canonical `proposal-090` gate verifies evidence, negative fixtures, lifecycle fixtures, settlement tests, and readback tests. | Partially Implemented | Gate exists and passes at `scripts/test-gate.sh:6981-7231`; it does not assert live canary staged mode or persisted preflight remediation facts. |

## Reviewer Scorecard

| Reviewer | Score | Readiness stance | Notes |
|---|---:|---|---|
| `chainworks_execution_truth_reviewer` | 3/5 | Not Ready | Execution truth is much closer, but recovered committed rows do not restore active artifact truth. |
| `rust_reliability_reviewer` | 2/5 | Not Ready | Crash recovery and preflight lifecycle still have failure windows not covered by the passing gate. |
| `api_contract_reviewer` | 4/5 | Mostly Ready | Receipt, GraphQL, MCP, and enum wrapper surfaces are largely implemented; preflight attempt/remediation fields are absent. |
| `observability_rollout_reviewer` | 3/5 | Not Ready | `proposal-090` passes, but its live-canary assertion overstates staged settlement coverage. |

## Routed Findings

### REL-001 / OPS-001: Startup recovery can mark staged rows committed without publishing active artifact pointers

Severity: P1  
Reviewers: `rust_reliability_reviewer`, `chainworks_execution_truth_reviewer`, `observability_rollout_reviewer`

Proposal `docs/proposals/090-junie-code-writer-runtime-hardening-after-capability-proof.md:482-492` requires accepted repair rows to update canonical outputs and publish active artifact pointers from accepted rows, and `:501-506` requires committed rows to be idempotently recovered from `canonical_after_sha256`.

The normal path persists staged rows, commits canonical files, publishes active generations, then upserts committed rows in `control-plane/crates/engine/src/executor.rs:11287-11302`; active generation publication itself is in `executor.rs:1527-1570`. However startup recovery only updates settlement row state/hash/rejection facts in `control-plane/crates/engine/src/recovery.rs:597-665`. It never calls the P090 active-pointer publisher or equivalent logic.

Crash window: process dies after `commit_p090_staged_repair_materialization` writes canonical bytes but before `publish_p090_committed_repair_artifact_generations` and the second DB upsert complete. On restart, `recover_p090_output_settlement_rows` can mark the row `committed`, but the active artifact index can remain stale. That leaves durable settlement truth and active artifact truth disagreeing.

### REL-002 / API-001: Junie preflight/remediation lifecycle is not durably represented as specified

Severity: P1  
Reviewers: `rust_reliability_reviewer`, `api_contract_reviewer`, `chainworks_execution_truth_reviewer`

Proposal `docs/proposals/090-junie-code-writer-runtime-hardening-after-capability-proof.md:563-617` requires persisted `runtime_preflight_phase` states including `preflight_running`, `preflight_remediating`, `passed`, and `failed_no_launch`; receipt fields for `preflight_attempt_count`, `preflight_remediation_applied`, `provider_launched`, redacted failure facts; one remediation for wrong cwd/runtime-home; and provider capacity only after preflight passes.

The Junie adapter performs a pre-spawn check and can remediate a missing worktree cwd once by changing `launch_spec.current_dir_override` in `control-plane/crates/acp/src/adapters/junie.rs:81-105`. The actual checks are filesystem probes in `junie.rs:125-190`.

But the persisted receipt data is synthesized later by the engine: success always uses `p090_runtime_tool_path_preflight_json_for_success` with `attempt_count=1` and `remediation_applied=null` in `control-plane/crates/engine/src/executor.rs:11959-11975`; failure uses a single error-derived JSON at `executor.rs:11977-12031`. The search surface contains no `preflight_attempt_count`, `preflight_remediation_applied`, `preflight_running`, or `preflight_remediating` persistence fields. Work item/AgentExecution claim and capacity filtering happen before ACP adapter preflight through `executor.rs:329-453` and `:522-705`.

This means wrong-cwd remediation can succeed operationally while public readback still claims one passed attempt with no remediation. Runtime-home remediation is also not implemented as a distinct retry path.

### READY-001: The passing live canary does not prove staged repair settlement despite running with the staged flag

Severity: P2  
Reviewers: `observability_rollout_reviewer`, `rust_reliability_reviewer`

`scripts/test-gate.sh:6991-6997` runs the P090 live canary with `CHAINWORKS_P090_STAGED_REPAIR_SETTLEMENT=1`, and `docs/evidence/090/junie-runtime-hardening/evidence-index.json:103-113` records that flag. The gate validates the canary status and fresh settled outputs at `scripts/test-gate.sh:7070-7125`.

However the canary receipt itself reports `repair_materialization_mode = "legacy_all_or_nothing"`, `staged_repair_settlement_enabled = false`, and settlement rows with `staging_path = null` / `active_pointer_generation_id = null` in `docs/evidence/090/junie-runtime-hardening/refine-like-canary/harness-result.json:144-152` and `:160-260`. That is expected for a direct successful final `CHAINWORKS_OUTPUT` with no repair turn, but it means the live canary does not prove staged repair behavior. Staged repair is currently covered by unit/focused tests, not by the live Junie canary.

### OPS-002: Staged repair requested without strict final payload is silently disabled

Severity: P2  
Reviewers: `observability_rollout_reviewer`, `api_contract_reviewer`

Proposal `docs/proposals/090-junie-code-writer-runtime-hardening-after-capability-proof.md:990-994` says staged repair requested without strict final-payload capture must fail closed or be explicitly coerced to disabled with a warning.

Implementation requires strict mode before staged mode can be true in `control-plane/crates/engine/src/executor.rs:11793-11801`, but I found no startup/config validation or warning readback for the coerced-disabled case. This makes rollout behavior less observable than specified.

## Readiness Checklist

- [x] P089 default capability gate passed.
- [x] P090 focused gate passed.
- [x] DB migration and settlement rows exist.
- [x] GraphQL/MCP readback fields exist and focused tests pass.
- [x] Provider spoof/mismatch negative fixtures exist and are gate-validated.
- [x] Long-running Junie ACP canary evidence exists and is gate-validated.
- [ ] Crash recovery republishes active artifact pointers from recovered committed accepted rows.
- [ ] Preflight lifecycle persists `preflight_running` and `preflight_remediating`.
- [ ] Preflight readback exposes attempt count and remediation applied.
- [ ] Runtime-home/cache remediation fixture is implemented, not only cwd remediation.
- [ ] Live canary or separate runtime fixture proves staged repair under live Junie if readiness language continues to imply that.
- [ ] Full regression sign-off is run after the P1 issues are fixed.

## Verification Log

Commands run:

- `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/report_path.py /Users/user/Documents/Chainworks Forge/docs/proposals/090-junie-code-writer-runtime-hardening-after-capability-proof.md`
  - Returned this R4 report path.
- `md5 -q docs/proposals/090-junie-code-writer-runtime-hardening-after-capability-proof.md`
  - `f98170c78ca39398e9aaed497180c057`
- `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/discover_prior_review.py ...`
  - Found `docs/proposals/090-junie-code-writer-runtime-hardening-after-capability-proof.review/evidence-pack.md`.
- `./scripts/test-gate.sh proposal-090`
  - Passed.
  - Covered evidence index validation, 2 DB tests, 4 ACP tests, 9 engine tests, 2 GraphQL readback tests, and 2 MCP readback tests.
- `./scripts/test-gate.sh proposal-089`
  - Passed default evidence validation.

Not run:

- `./scripts/test-gate.sh full`
  - Not run because the audit found P1 readiness blockers before full sign-off would be meaningful.
- `CHAINWORKS_PROPOSAL_090_LIVE=1 ./scripts/test-gate.sh proposal-090`
  - Not rerun; audited checked-in live evidence and default gate validation.

## Final Recommendation

Do not close P090 yet.

The implementation is no longer "not implemented"; it is a serious partial implementation with passing focused gates. The remaining issues are still contract-level, not polish:

1. make P090 startup recovery restore active artifact pointer truth for recovered committed accepted rows;
2. persist the Junie preflight lifecycle and remediation facts before/around launch instead of reconstructing only a final success/failure JSON;
3. tighten the gate/evidence language so the live canary does not imply staged repair coverage unless it actually exercises a repair turn;
4. add explicit rollout warning/fail-closed behavior when staged repair is requested without strict final payload.

Recommended next verdict after fixes: rerun `proposal-090`, `proposal-089`, and then the repo's canonical full sign-off gate before marking Implemented/Ready.
