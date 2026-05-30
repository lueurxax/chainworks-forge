# Proposal 076 Implementation Audit R1

## Metadata

| Field | Value |
|---|---|
| Proposal | `docs/proposals/076-auto-retry-observation-ledger-and-recovery-policy.md` |
| Proposal source used | `daccc1fc^:docs/proposals/076-auto-retry-observation-ledger-and-recovery-policy.md` (`910a7300`) |
| Proposal state | Replaced/retired on `main` by closeout commit `daccc1fc`; implemented truth now lives in `docs/reference/auto-retry-observation-ledger.md` |
| Audit timestamp | 2026-05-24 |
| Repo root | `/Users/user/Documents/Chainworks Forge` |
| Implementation target | `main` |
| Audited HEAD | `81be5b80b8e52447a66d77507bc5a1f1656c39d1` |
| Working tree status before report | Clean; `main` ahead of `origin/main` by 7 commits |
| Compare base | `origin/main` for implementation inventory; current `main` for conformance |
| Report path helper | Failed because the proposal file is retired on `main`; report path was assigned manually using the standard R1 sidecar stem |
| Report path | `docs/proposals/076-auto-retry-observation-ledger-and-recovery-policy_IMPLEMENTATION_AUDIT_R1.md` |

## Prior Proposal-Review Reuse

| Item | Result |
|---|---|
| Prior review artifacts found | None |
| Reviewer selection reuse | Not reused |
| Reason | The proposal file is retired on `main`, no sibling proposal-review artifacts were found, and the helper could not search a missing proposal path. |

## Selected Reviewers

| Reviewer | Why selected |
|---|---|
| `rust_reliability_reviewer` | P076 owns append-only JSONL writes, lock behavior, retry/cooldown fail-closed policy, and degraded readback. |
| `api_contract_reviewer` | P076 exposes `automation.auto_retry.latest`, closed readback shapes, version negotiation, and JSON fixture contracts. |
| `observability_rollout_reviewer` | P076 is an automation/ledger rollout with external prompt deployment, evidence fixtures, and operator readback. |

Rejected close alternatives:

- `rust_arch_reviewer`: ownership boundaries are small and covered by reliability/API review.
- `rust_security_reviewer`: no new public auth/authz or secret-handling surface was introduced beyond existing capability mapping.
- `product_reviewer`: no product metric, experiment, or user-facing decision gate is central to this slice.

## Proposal Contract Summary

P076 proposed turning the free-form auto-retry monitor into a structured observation and recovery-policy subsystem:

- append one structured observation record per monitor poll;
- maintain a deduplicated known-issue catalog by `blocker_signature_id`;
- classify blocked runs before any retry decision;
- prevent blind retry loops, especially human gates and repeated unresolved signatures;
- produce proposal-ready rollup evidence from normalized data;
- keep the surface MCP-first and outside SwiftUI/GraphQL ownership;
- validate the schema, rollup, and no-human-gate retry behavior with a retained gate.

Platform/product scope:

- Apple: Not applicable.
- Backend/service: Rust MCP readback, Python automation writer/rollup scripts, local JSONL/JSON/Markdown evidence files, rollout prompt/config, and test-gate fixtures.

Primary service/operator flows:

1. A poll writer records one normalized `auto-retry-observation.v1` JSONL event under `.chainworks/automation/`.
2. The rollup groups valid ledger records by `blocker_signature_id` into JSON and generated Markdown views.
3. Operators and integrations read the latest state through `automation.auto_retry.latest`.
4. Partial/corrupt ledger data returns diagnostics or degraded readback instead of fabricated state.
5. The recurring automation prompt uses the repo writer and remains observe-only.

## Proposal Fidelity Inventory

### Matches

- `scripts/chainworks/auto_retry_observe.py` writes newline-terminated, fsynced, hash-stamped `auto-retry-observation.v1` records and refreshes budget/catalog/rollup files.
- `scripts/chainworks/auto_retry_rollup.py` reads the JSONL ledger, ignores only a true unterminated trailing fragment, deduplicates by `blocker_signature_id`, and emits JSON/Markdown summaries.
- `control-plane/crates/mcp-server/src/tools/automation.rs` exposes `automation.auto_retry.latest`, echoes the six path fields, supports latest-by-run rows, handles no-history rows, and degrades complete malformed records.
- `control-plane/crates/mcp-server/src/server.rs` rejects unsupported auto-retry readback versions with JSON-RPC error `-32076`.
- `scripts/test-gate.sh proposal-076` validates schema fixtures, negative fixtures, observe-only writer behavior, rollup dedupe, MCP tool presence, version/readback shape, and focused Rust tests.
- `/Users/user/.codex/automations/chainworks-guard/automation.toml` now uses the repo writer, `auto-retry-observation.v1`, and explicit observe-only no-mutation policy.

### Divergences

- The historical draft included wording for retrying safe blocked runs through `stages.retry`. Current `main` intentionally resolves P076 as an observe-only evidence/readback contract; retry dispatch is excluded by code, gate, reference docs, and automation prompt.
- The proposal file itself is retired on `main`, so this audit used the historical proposal from `910a7300` plus the closeout reference as current repository truth.
- The external `chainworks-guard` automation is configured with an aligned prompt but is currently `PAUSED`; the repo implementation is ready, but live recurring execution is not proven active.

### Ambiguities / Evidence Gaps

- No live daemon MCP call was executed; validation is by code inspection, fixtures, and the retained proposal gate.
- The prompt lives in `/Users/user/.codex/automations/chainworks-guard/automation.toml`, while the historical proposal named `/Users/user/.codex/automations/auto-retry/`. The active prompt text is aligned, but the directory name differs.

## Requirement Summary

| Requirement | Status |
|---|---|
| REQ-001 Append-only structured observation ledger | Implemented |
| REQ-002 One parseable observation per successful writer poll | Implemented |
| REQ-003 Deduplicated known-issue catalog by blocker signature | Implemented |
| REQ-004 Blocker classification fields and owner lanes | Implemented |
| REQ-005 Human gates and side-effect actions remain fail-closed | Implemented |
| REQ-006 Retry/cooldown policy prevents blind repeated retry | Implemented |
| REQ-007 Proposal-ready rollup without scraping old automation memory | Implemented |
| REQ-008 Validator/gate and negative-fixture coverage | Implemented |
| REQ-009 MCP readback and versioned contract surface | Implemented |
| REQ-010 Updated recurring automation prompt | Implemented |
| REQ-011 Generated `.chainworks/automation` outputs excluded from git | Implemented |
| REQ-012 No GraphQL/SwiftUI ownership of operational control | Implemented |

## Detailed Requirement Audit

### REQ-001 Append-only structured observation ledger

- Proposal source: lines 42, 70-115, 197-200, 227-228.
- Status: Implemented.
- Evidence: code, tests-run.
- Evidence references: `scripts/chainworks/auto_retry_observe.py:46-87`, `scripts/chainworks/auto_retry_observe.py:327-357`, `scripts/test-gate.sh:6745-6810`.
- Implementation mapping: The writer resolves `.chainworks/automation/auto-retry-observations.jsonl`, appends one canonical JSON object plus newline, fsyncs the file and parent directory, and includes `canonical_record_hash`.
- Note: Lock-held polls return `skipped_lock_held`; the one-record guarantee applies to successful writer polls.

### REQ-002 One parseable observation per successful writer poll

- Proposal source: lines 78-115, 197-200, 227-228.
- Status: Implemented.
- Evidence: code, tests-run.
- Evidence references: `scripts/chainworks/auto_retry_observe.py:308-371`, `scripts/test-gate.sh:6776-6798`.
- Implementation mapping: The retained gate executes the writer against a temporary input and proves exactly one JSONL record, correct schema, newline termination, and hash presence.

### REQ-003 Deduplicated known-issue catalog by blocker signature

- Proposal source: lines 117-137, 202-205, 229-230.
- Status: Implemented.
- Evidence: code, tests-run.
- Evidence references: `scripts/chainworks/auto_retry_rollup.py:90-147`, `scripts/chainworks/auto_retry_observe.py:264-305`, `scripts/test-gate.sh:6732-6744`.
- Implementation mapping: Rollup groups rows by `blocker_signature_id`; the writer refreshes `auto-retry-known-issues.json` and generated Markdown after appending.

### REQ-004 Blocker classification fields and owner lanes

- Proposal source: lines 139-150, 200.
- Status: Implemented.
- Evidence: code, fixtures, tests-run.
- Evidence references: `scripts/chainworks/auto_retry_observe.py:144-186`, `scripts/chainworks/auto_retry_rollup.py:150-160`, `docs/evidence/rollout-contract/operator-readback/p076-full-surface.fixture.json`.
- Implementation mapping: Blocked runs carry `blocker_class`, `blocker_signature_id`, `failure_class`, `failure_summary`, `policy_decision`, and `next_systemic_action`; rollup maps classes to owner lanes.

### REQ-005 Human gates and side-effect actions remain fail-closed

- Proposal source: lines 47, 52-56, 145, 160, 231.
- Status: Implemented.
- Evidence: code, config, tests-run.
- Evidence references: `scripts/chainworks/auto_retry_observe.py:152-155`, `scripts/test-gate.sh:6728-6730`, `scripts/test-gate.sh:6799-6803`, `/Users/user/.codex/automations/chainworks-guard/automation.toml`.
- Implementation mapping: The writer rejects side-effect retry results and human-gate retry actions; the gate scans for retry/recovery/approval dispatch hooks and validates human-gate behavior.

### REQ-006 Retry/cooldown policy prevents blind repeated retry

- Proposal source: lines 152-164, 207, 232, 234.
- Status: Implemented.
- Evidence: code, config, tests-run, reference.
- Evidence references: `scripts/chainworks/auto_retry_observe.py:170-180`, `scripts/chainworks/auto_retry_observe.py:239-261`, `docs/evidence/rollout-contract/negative/p076-budget-failure-retried.json`, `docs/reference/auto-retry-observation-ledger.md:5-33`, `/Users/user/.codex/automations/chainworks-guard/automation.toml`.
- Implementation mapping: Current `main` resolves the retry/cooldown slice as observe-only: budget records use `max_attempts=0`, `remaining_attempts=0`, no dispatch is allowed, and the prompt records recommendations rather than performing `stages.retry`.
- Note: This is narrower than the draft's safe-retry wording, but it is the accepted mainline contract after closeout and prevents shotgun retry loops by construction.

### REQ-007 Proposal-ready rollup without scraping old automation memory

- Proposal source: lines 165-186, 233.
- Status: Implemented.
- Evidence: code, tests-run.
- Evidence references: `scripts/chainworks/auto_retry_rollup.py:28-36`, `scripts/chainworks/auto_retry_rollup.py:90-147`, `scripts/chainworks/auto_retry_rollup.py:200-239`, `scripts/test-gate.sh:6732-6744`.
- Implementation mapping: Rollup consumes the JSONL ledger and emits JSON catalog/rollup plus optional Markdown; it does not read old automation memory.

### REQ-008 Validator/gate and negative-fixture coverage

- Proposal source: lines 214-219, 225-234.
- Status: Implemented.
- Evidence: code, fixtures, tests-run.
- Evidence references: `scripts/test-gate.sh:6065-6815`, `docs/evidence/rollout-contract/negative/p076-*`, `docs/evidence/rollout-contract/operator-readback/p076-full-surface.fixture.json`.
- Implementation mapping: The retained gate checks twenty negative fixtures, closed enum domains, missing fields, no side-effect retry, no human-gate retry, rollup dedupe, writer behavior, and Rust readback tests.

### REQ-009 MCP readback and versioned contract surface

- Proposal source: lines 48, 195, 217-218.
- Status: Implemented.
- Evidence: code, tests-run.
- Evidence references: `control-plane/crates/mcp-server/src/tools/automation.rs:11-40`, `control-plane/crates/mcp-server/src/tools/automation.rs:49-125`, `control-plane/crates/mcp-server/src/server.rs:445-470`, `control-plane/crates/mcp-server/src/tools/automation.rs:335-425`.
- Implementation mapping: `automation.auto_retry.latest` is registered, capability-mapped, versioned as `auto_retry_readback.v1`, rejects unsupported versions, and has tests for normal history, true partial trailing fragments, and malformed complete final lines.

### REQ-010 Updated recurring automation prompt

- Proposal source: lines 188-210, 216.
- Status: Implemented.
- Evidence: external config.
- Evidence references: `/Users/user/.codex/automations/chainworks-guard/automation.toml`, `/Users/user/.codex/automations/auto-retry/memory.md`.
- Implementation mapping: The current automation prompt instructs P076 observe-only operation, uses the repo writer, writes `auto-retry-observation.v1`, keeps retry budget observe-only, updates compact memory, and treats JSONL/JSON/readback as authority.
- Note: The configured automation is `PAUSED`; see OPS-001.

### REQ-011 Generated `.chainworks/automation` outputs excluded from git

- Proposal source: line 214, risk line 240.
- Status: Implemented.
- Evidence: config.
- Evidence references: `.gitignore:36-39`.
- Implementation mapping: `.chainworks/` is ignored wholesale.

### REQ-012 No GraphQL/SwiftUI ownership of operational control

- Proposal source: lines 52-56.
- Status: Implemented.
- Evidence: code search, reference.
- Evidence references: `control-plane/crates/mcp-server/src/tools/automation.rs`, `scripts/chainworks/auto_retry_observe.py`, `docs/reference/auto-retry-observation-ledger.md:35-50`.
- Implementation mapping: The implemented surface is Python writer/rollup plus MCP readback. SwiftUI is passive for this slice, and no GraphQL mutation ownership was found.

## Routed Specialist Findings

### OPS-001 Recurring automation prompt is aligned but currently paused

- Reviewer: `observability_rollout_reviewer`
- Severity: Minor
- Confidence: High
- Related requirements: REQ-010
- Evidence: external config.
- Evidence references: `/Users/user/.codex/automations/chainworks-guard/automation.toml`.
- Why it matters: The prompt now matches the P076 observe-only v1 contract, but `status = "PAUSED"` means the recurring job will not produce new observations until re-enabled. That is acceptable for code readiness, but it is a handoff/operations risk if the operator expects live recurring evidence.
- Recommended action: Before relying on live unattended monitoring, confirm whether `chainworks-guard` should be active and, if so, unpause it through the automation owner flow.
- Acceptance criteria: The intended automation is either explicitly left paused with operator acknowledgement or set active with the same v1 observe-only prompt.

### REL-001 Retry dispatch remains intentionally observe-only

- Reviewer: `rust_reliability_reviewer`
- Severity: Note
- Confidence: High
- Related requirements: REQ-006
- Evidence: code, config, reference.
- Evidence references: `scripts/chainworks/auto_retry_observe.py:152-155`, `scripts/chainworks/auto_retry_observe.py:170-180`, `docs/reference/auto-retry-observation-ledger.md:5-33`, `/Users/user/.codex/automations/chainworks-guard/automation.toml`.
- Why it matters: The original draft allowed safe `stages.retry` within budget, but current mainline P076 deliberately records observations and recommendations only. Dependent proposals must not assume P076 owns active retry execution.
- Recommended action: Keep dependent docs/proposals phrased around observation/readback unless a future proposal implements side-effecting retry budget ownership.
- Acceptance criteria: Future active retry work adds its own state-machine tests for attempt counting, advancement reset, cooldown exhaustion, and stage-id target selection.

## Reviewer / Lens Scorecard

| Lens | Result | Top risk | Confidence |
|---|---|---|---|
| Objective conformance | Implemented | Historical safe-retry wording was narrowed by closeout to observe-only | High |
| Rust reliability | Ready | No active retry side effects; final-line degraded readback fixed and tested | High |
| API contract | Ready | No known readback/versioning blocker after final-line fix | High |
| Observability / rollout | Ready with risks | External automation is paused | High |
| Overall readiness | Ready with Risks | Operational activation state needs operator decision | High |

## Readiness Checklist

| Check | Result |
|---|---|
| Canonical focused gate | Passed: `./scripts/test-gate.sh proposal-076` on audited `main` HEAD `81be5b80` |
| Core writer flow | Passed through gate temporary ledger: one JSONL record, newline, hash, budget/catalog/rollup created |
| Core rollup flow | Passed through gate: repeated `blocker_signature_id` dedupes to one issue with count 2 |
| MCP readback flow | Passed focused Rust tests: normal history, true partial trailing fragment, malformed complete final line degraded |
| Negative fixture coverage | Present: 20 retained `p076-*` negative fixtures |
| Runtime daemon call | Not run |
| Full regression suite | Not run; focused retained proposal gate is the canonical gate for this implemented slice |
| UI/accessibility/localization/privacy | Not applicable; no UI surface in proposal scope |
| External automation deployment | Prompt aligned; `status = "PAUSED"` requires operator decision before live recurrence |

## Verification Log

| Command / Check | Result |
|---|---|
| `git status --short --branch` | Clean before report; `main...origin/main [ahead 7]` |
| `git show daccc1fc^:docs/proposals/076-auto-retry-observation-ledger-and-recovery-policy.md` | Historical proposal source available; current `main` deletes the proposal as part of closeout |
| `python3 .../report_path.py .../docs/proposals/076-auto-retry-observation-ledger-and-recovery-policy.md` | Failed because proposal is retired; manual standard R1 sidecar path used |
| `python3 .../discover_prior_review.py .../docs/proposals/076-auto-retry-observation-ledger-and-recovery-policy.md` | Failed because proposal is retired; no prior artifacts found by repo search |
| `find docs/evidence/rollout-contract/negative -name 'p076-*'` | 20 negative fixtures found |
| `rg` implementation surface search | Found writer, rollup, MCP readback, capability mapping, reference docs, gate, fixtures, and dependent proposal references |
| Read-only external automation prompt search | Found aligned v1 observe-only prompt in `chainworks-guard`; no v2/broad-retry prompt found |
| `./scripts/test-gate.sh proposal-076` | Passed; includes fixture checks, rollup/writer temporary ledger checks, and 3 focused Rust MCP readback tests |

## Final Verdict

Overall conformance: Implemented.

The mainline P076 implementation satisfies the accepted observe-only contract now documented in the reference tree. The historical draft's safe-retry wording is no longer the active mainline behavior; closeout resolved P076 as an observation/readback layer, not an owner of side-effecting retry dispatch.

Overall implementation readiness: Ready with Risks.

The repo-local implementation and canonical proposal gate are green. The only remaining handoff risk is operational: the aligned recurring automation is currently paused, so live recurring observations depend on the operator's activation decision.

Recommended next actions:

1. Decide whether `chainworks-guard` should remain paused or be reactivated with the aligned P076 prompt.
2. Keep future active retry/cooldown execution in a separate proposal with explicit state-machine tests.
