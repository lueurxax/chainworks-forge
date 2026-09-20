# Headless I1 Planning Review

Date: 2026-09-19. Source: the user's supplied repeat proposal review.
Mode: `proposal-readiness`. This records that review, not a newly dispatched
independent review or an implementation verification performed by this task.

## Verdict And Identity

**Ready for I1 planning only.** No confirmed blocker to the first experiment
was reported. I1 is separated from production, uses two disposable projects,
and stops after uncertain outcomes without automatically repeating an open.
I2-I4 and the full migration/release are not Ready under this verdict.

Reviewed source HEAD: `a096b8831f0db7f17d1ffe426851c49a65b8b000`.
Reviewed parent MD5: `e9d6a8dad4bbc2d9ab38cbc4dc88142b`.
Both were checked again when receiving the review. The five specification
artifacts remain byte-for-byte unchanged; their SHA-256 pins are in the
[R3 handoff](xcode-headless-review-response-2026-09-19.md).

Reviewer confidence: High for I1. Evidence completeness: Complete for I1
planning, Partial for future production integration. H1 remains `not_run`:
metadata discovery and equal schema captures do not establish successful
open/read/reconnect behavior.

## Nonblocking Future Findings

IDs below are scoped to this repeat review (`R3-review/P2-*`). In particular,
R3-review/P2-01 is not the earlier R1/P2-01 release-receipt finding.

| Finding | Verified basis | Acceptance before production / owner | State |
| --- | --- | --- | --- |
| R3-review/P2-01: operator authority | The wire table grants detailed diagnostics to an authenticated Operator; `Principal::from_entry` can restrict an Operator's MCP/resource/GraphQL surfaces | Define separate diagnostic and reconciliation capabilities/scopes. An authenticated but insufficiently authorized Operator gets no detailed data and cannot change a hold. Auth/wire owner, before the corresponding production surface | Open, nonblocking I1 |
| R3-review/P2-02: legacy daemon cutover | The existing singleton is per SQLite database; a legacy binary using another DB does not participate in the proposed common lock | Assign Ops/runtime ownership for retiring the legacy path, checking unfinished work and proving incompatible daemons absent. Unverified state means hold; a new lock is not proof an old client stopped | Open, nonblocking I1 |
| R3-review/P2-03: historical result schemas | Attempt reads retain past results while the wire union is generated from the currently admitted manifest | Pin historical result-schema versions independently from new-call admission. Read a persisted result after its mutator is removed or its schema changes, without re-enabling dispatch. Domain/wire/journal owner, before durable attempt readback | Open, nonblocking I1 |

Source anchors verified on receipt:

- [Restricted Operator construction](../../control-plane/crates/auth/src/lib.rs): `Principal::from_entry` applies explicit surface policies and can leave zero MCP/resource capabilities.
- [Existing database lock](../../control-plane/crates/daemon/src/supervisor.rs): `sqlite_database_lock_path` derives the lock from the selected database file.
- [Wire target](../superpowers/specs/2026-09-19-xcode-headless-wire-contract.md): Effectful Calls and Privilege And Mixed Versions retain the identified gaps.

These entries accept and preserve the findings; they do not claim a fix or
silently amend the reviewed normative contracts. Resolve them in the applicable
later contract/plan before production admission. I1 creates no operator API,
production singleton, DB journal or historical provider-result surface.

## Reported Review Coverage

The user reports five independent checks routed through
`.codex/review-router.yaml`: `rust_arch_reviewer`,
`rust_reliability_reviewer`, `rust_security_reviewer`,
`api_contract_reviewer`, and `observability_rollout_reviewer`.
This task did not rerun those reviewers or independently inspect their execution.

The report rates I1 purpose, scope, code fit, state, internal API, execution,
failure handling, permissions, evidence, I1-01 through I1-08, and outcome
classification as Ready. Production migration/rollback is not applicable to
I1. Separate Apple architecture and execution-truth reviewers were excluded by
the routing cap because I1 changes neither Swift state nor durable Run authority;
those checks remain relevant to later stages. No UI/performance/product review
was added. The report states no files, builds, tests, IDE or services changed.

## Planning Handoff

The [I1 implementation plan](../superpowers/plans/2026-09-19-xcode-headless-i1.md)
is prepared under this limited approval. It still needs the user's review and
choice of execution method. Live fixture opening additionally needs explicit
authorization for the actual fixture paths and launch identity; OS consent is
separate. No deployment, existing-run retry, commit or push follows from this
review. The original specifications retain their reviewed hashes; this dated
record supersedes their pre-review status text for I1 planning only.
