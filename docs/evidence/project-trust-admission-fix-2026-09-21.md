# Project trust admission fix: 2026-09-21

Status: **locally verified; 52 unique tests passed**. Changes remain unstaged
and uncommitted in the implementation worktree; no commit, push, or merge was
performed. This is not a deployed fix, live grant/retry, provider launch,
Xcode acceptance, or real run unblock.

## Source and Incident Boundary

Implementation worktree: `/Users/user/.codex/worktrees/cf23/Chainworks Forge`.
Base commit: `8c40f71a03cfebff413fbad584131e0bcbe9859b`.
The original checkout was read only for this task; unrelated work was preserved.

Incident inputs, read without modification under
`/Users/user/Documents/Chainworks Forge`:

- `docs/evidence/blocked-runs-root-causes-2026-09-21.md` (primary report).
- `docs/evidence/blocked-runs-triage-2026-09-20-evening.md`.
- `control-plane/crates/engine/tests/blocked_run_diagnostic.rs` (read only;
  not rerun against the live database during this task).

The primary report recorded these historical observations:

- P070 child runs A/B/C/D resolved their dedicated worktrees and Xcode projects.
- The packaged `xcode-project-trust` store was absent, while the separate
  `xcode-runtime/authority.json` and coordinator lock existed.
- Lookup returned the misleading `authority_missing`; the failed executions
  had no session events or runtime-facts row because failure preceded launch.

Those observations are provenance, not a fresh live-state check. This patch
does not backfill historical runtime facts or rewrite earlier error records.

## Bounded Change

The initial missing store-path check now returns `project_trust_required`.
An absent grant has the same admission meaning. Lookup remains read only:
it neither creates the directory nor grants trust. Unsafe paths, ownership or
permission failures, corrupt records, missing/linked lock files, contention,
revocation, and project/root identity replacement still fail closed.

`HeadlessRuntime::prepare` attaches `ProjectTrustAdmissionFailure` with the exact
resolved root, project key, and reason code. Missing and revoked trust retain
their explicit codes; other trust failures become `project_trust_unavailable`,
with the original error chain retained. This boundary precedes provider/session
launch, Xcode service startup, workspace opening, and tool dispatch.

The executor/recovery/readback change records failed preparation as
`failed_no_launch`, keeps `provider_launched: false`, and provides an operator
disposition. Required/revoked trust directs explicit review; unavailable trust
directs inspection without grant guidance. Exact root/project details are
operator-only. Admission failures do not automatically consume provider
retry/escalation. Atomic persistence preserves cancelled, superseded, and
cleanup-held attempts rather than overwriting their terminal truth.

## Local Verification

All fixtures use temporary directories and fixture databases/authorities.
No production database mutation, live grant/retry/approval, daemon launch or
deployment, or original-checkout edit was performed by this task.

Behavioral red observations before their respective fixes: trust lookup returned
`authority_missing` (11 passed, 2 failed); executor runtime facts were `None`;
recovery offered generic retry; scheduler compilation masked admission failure;
the classifier returned `stale_no_output`; API preflight fields were missing;
cancellation became `Failed` instead of remaining `Cancelled`; and unavailable
trust offered review/grant rather than inspection. SQLx literal-query, GraphQL
stage-ID, and qualified `AgentExecutionId` setup corrections were fixture
corrections, not behavioral red evidence or product failures.

| Check | Observed result |
| --- | --- |
| Canonical `xcode-headless-project-trust` gate | 22 passed: ACP store 13, engine admission 2, engine prelaunch unit tests 5, GraphQL 1, MCP 1. |
| Store coverage | Missing store/parent without mutation; explicit fixture grant; exact root/checkout isolation; revoked, corrupt, linked, permissive, replaced, and busy identities rejected. |
| Engine prelaunch unit coverage | Recovery disposition, classifier, both retry schedulers, unavailable-store inspection, and distinct journal failure. |
| Existing `xcode_headless_preparation` suite | 30 passed, 0 failed. |
| Latest admission tests after targeted retry extension | 2 passed again: denial/grant/retry and cancellation; included in the 22, not added to the unique count. |
| Final independent read-only review | No remaining actionable findings in the reviewed scope. |
| Static checks | `rustfmt --check` on all 10 changed/new Rust files, `git diff --check`, and `bash -n scripts/test-gate.sh` passed. |
| Guardrails | Passed with `CHAINWORKS_AUTO_CACHE_CLEANUP=0`; boundary-coverage subcheck skipped because its `origin/main` comparison excludes the unstaged diff. |
| Deployed/runtime acceptance and real run recovery | Not performed. |

Total: **52 unique passing tests** (22 canonical-gate tests plus 30 existing
preparation tests). The latest integration extension uses the real
`CommandHandler` targeted `stages.retry` after a temporary grant. It creates a
distinct stage attempt 2 and agent execution, then reaches the deliberate
`LaunchProbe.prepare_launch_spec` stop with zero provider sessions. This proves
the retry admission path, not a launched provider process or live Xcode service.

Reproduction commands:

```bash
export CHAINWORKS_AUTO_CACHE_CLEANUP=0
./scripts/test-gate.sh xcode-headless-project-trust
./scripts/cargo-managed test --manifest-path control-plane/Cargo.toml -p engine \
  --test xcode_headless_preparation --test xcode_project_trust_admission --no-fail-fast
```

Only repository-managed shared Cargo caches were used. An earlier guardrails
attempt failed during automatic cache cleanup (`DerivedData: Directory not
empty`); rerunning with cleanup disabled passed. Sandbox-denied `ps` was
nonfatal. The existing MCP `public_artifact_index_row` dead-code warning remains.

## Modified Files

Production, including colocated regression tests (paths under `control-plane/crates/`):

- `acp/src/xcode_project_trust.rs`, `acp/src/xcode_headless_runtime.rs`.
- `engine/src/executor.rs`, `engine/src/orchestrator.rs`.
- `engine/src/recovery.rs`, `engine/src/shadow_escalation.rs`.
- `graphql-server/src/types/stage.rs`, `mcp-server/src/tools/reports.rs`.

Integration tests: `control-plane/crates/acp/tests/xcode_project_trust.rs` and
new `control-plane/crates/engine/tests/xcode_project_trust_admission.rs`.
Docs: `docs/reference/xcode-headless-runtime.md`, `docs/reference/test-gates.md`,
and this new evidence note. Gate: `scripts/test-gate.sh`.

## Operator Procedure and Limits

The reviewable procedure and placeholder `TrustGrant` request are in
[Headless Xcode Runtime](../reference/xcode-headless-runtime.md#review-grant-then-retry).
For a dedicated worktree the request contains the persisted `run_id`, root with
canonical repository and exact effective worktree, `kind: "worktree"`,
`strategy: "dedicated"`, current device/inode strings, and a root-relative
project selector. No real run request was generated as part of this fix.

Use the same deployed binary, mode, database, host account, principal table,
and selected app-support directory as the daemon. Packaged mode uses
`~/Library/Application Support/Chainworks Forge/control-plane.db` and its
`xcode-project-trust` directory; it ignores `DATABASE_URL`. Development mode
uses its explicit daemon database URL and `~/.chainworks/dev-app-support` store.
Using a development command against the packaged database does not select the
packaged trust store.

An explicit global Operator decision for the exact project/root is required.
The recommended principal has `xcode.global_admin` and `xcode.project_trust`
without `run_scope`; no credential or principal provisioning is part of this
patch. Supply an existing token privately via `CHAINWORKS_MCP_TOKEN`; never
persist it in argv, request JSON, evidence, or shell history.

The existing CLI validates the target against the persisted run and returns
`{"granted":true,"trust_digest":"<64-character-digest>"}` only after writing,
rereading, and checking the record. There is no separate trust-inspection action
or UI/MCP trust-write path. The grant applies to all invocations of that exact
identity, not only the validating run. The subsequent supported stage retry is
a separate operator decision and must be followed by execution/session readback.

The incident's separate A/B/D quota-escalation/frozen-profile disagreement and
failure-settlement defect remain out of scope and may still block recovery.
Historical false-running state, frozen snapshots, provider overrides, C/D role
compatibility, Green-main review provenance, and projection poison were not
repaired. Trust admission evidence alone must not be reported as resolving them.
