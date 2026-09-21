# P039 Implementation Audit R2

Date: 2026-09-20; resumed 2026-09-21. Baseline: `8c40f71a03cfebff413fbad584131e0bcbe9859b`.
Scope: uncommitted isolated `p039-carry-forward` worktree; main and live services
are not part of this audit snapshot. R1 remains the historical I1 verdict.

## Verdict

**Offline implementation accepted; I4 and full closeout remain pending.**
The candidate-3 frozen-tree gate passed 348 selected Rust tests plus three checker
tests, covering all 29 offline cases through 164 required exact test names.
Broad regressions are fully classified: 80 failures reproduce on baseline; the
one introduced producer-inventory regression is corrected and independently
verified. Independent final synthesis accepts the candidate within this offline
scope, with no scoped P039 finding left open. The full workspace is not green. There is no
compatible deployment or live P095 migration receipt. Full closeout and proposal
retirement are not claimed.

The implementation now includes durable reservation, preparation, activation,
input/readback and approval integration. This supersedes R1's statement that
these components do not exist, but not its distinction between local evidence
and live execution. Tests are disposable/provider-free unless explicitly stated
otherwise; a protocol fixture is not an actual provider or Apple observation.

- [Current measured evidence](../evidence/p039-durable-integration.md)
- [Incremental implementation plan](../superpowers/plans/2026-09-20-p039-durable-integration.md)
- [Acceptance and rollout contract](039/verification-and-rollout.md)

## Review Reconciliation

| Area | Current disposition |
| --- | --- |
| Fresh output producer and reader | Checksum/size persistence, mixed-output settlement and exact authorized fallback are corrected. F01/F02 independently accepted by code/regression inspection; actual provider execution remains separate. |
| Successor index, admission capacity, abort settlement | R1/R2/R3 independently accepted after 43 focused tests, with all scoped hashes unchanged. |
| MCP resources, persisted report/receipt, legacy transport | API-039-01/02/03 independently accepted by code/regression inspection. No historical-schema compatibility verdict is inferred from API parity. |
| Consistent WAL backup and relative DB paths | Backup regression corrected and independently inspected. Real old-daemon rollout and all filesystem fault boundaries are not proved. |
| Historical selection and finding retention | E01/E02-E07 corrections include dynamic-review E03; independent unfiltered preview rerun passed 48/48 with unchanged scoped hashes. Missing authenticated older findings stay held, never silently omitted or rebound to current bytes. |
| First dispatch and later approval gates | R01/R02 and exact-profile snapshot suppression independently accepted. Full ordinary approval runtime target passed 14/14, zero filtered; actual persisted PromptSent, not queue admission, grants started authority. |
| ACP metadata-root composition | R5-META-01 fixed after a stale-directory-identity RED regression. All seven ACP boundary cases and the engine activated-manifest/owned-execution test passed in the owner's and independent reviewer's runs. Ordinary confinement is unchanged. |
| Source/indirect-owner SQL fences | Approved isolated SQL expansion passed 21 fence tests and 42 related checks. F1 admission-owner correction independently passed four checked-admission and 13 storage tests; F2 queue-head correction independently passed 12/12. Both accepted with unchanged scoped hashes. No production application is authorized. |
| Final API and policy proofs | ET01 real-service post-commit lost-ack/reopen replay, ET04 GraphQL post-abort readback, ET02 structured carried-reference/reopen/current-declaration removal and ET03 missing-headless denial independently accepted by source/assertion inspection. MCP24, GraphQL3 and policy3 tests passed in owner runs. General removed-tool compatibility is not claimed. |
| Combined gate and execution truth | Final candidate-3 gate passed: 348 selected Rust tests, zero failed/ignored; checker 3/3; 29 cases, 164 distinct mapped successes; rollout lint PASS. All 1003 frozen-file checksums matched after execution. Workspace/all-targets compilation, formatting and changed-document links pass. Independent final offline synthesis: ACCEPTED, no open scoped finding. |
| Broad regression accounting | Candidate 2: 4011 passed, 81 failed, three ignored, 131 selections. All 80 pre-existing failures have baseline reproduction; the introduced inventory failure has an independent candidate-3 GREEN and is now required by the gate. Separate daemon unit lane: 137/137. No green full-workspace claim; host-global daemon startup and guardrails remain excluded. |
| P095 live acceptance | Not executed. Historical data shapes are representable; exact live provenance, retained findings and eligibility remain unproved. |

The detailed evidence records broad-suite failures and baseline reproductions.
Those reproductions are not a blanket regression waiver or a green-workspace
claim. No failed production test was removed to produce this verdict.

## Closeout Inventory

The closeout inventory was executed read-only. A bounded
[stable carry-forward contract](../reference/blocked-run-carry-forward.md) now
owns implemented operator/runtime behavior, with pointers from the Rust,
execution-truth and MCP references, README and documentation indexes. P070's
active dependency is rewired. Its accuracy has independent consistency review;
changed-document links pass. Stable gate aliases, migrations, tests and runtime
files are retained.

The parent proposal, both normative children, review directory and audit history
remain active. The reference explicitly separates implemented behavior from
deployment/live acceptance. No proposal-side artifact has been deleted.

## Remaining Order

1. Obtain the separately requested I4 operational scope. The isolated SQL-expansion
   approval does not authorize merge, push, production migration or P095 transfer.
2. Establish compatible deployment and execute the approved live-canary scope;
   observe fresh review, approval and headless execution rather than infer them.
3. Only after live acceptance, finalize the prepared stable documentation and
   retire proposal-side artifacts with link and retained-gate verification.

Closeout disposition: **DOCS UPDATED; RETIREMENT BLOCKED by unapproved/unexecuted
I4**, not by an unresolved scoped offline implementation finding.

No merge, push, daemon restart, production migration, source cancellation or
cleanup is represented by this audit.
