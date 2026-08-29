# Proposal 105: Deterministic GitHub Pull-Request Publication

| Field | Value |
|---|---|
| Date | 2026-08-09 |
| Status | Draft |
| Author | Engineering (delivery control plane) |
| Depends on | Proposal 007 delivery configuration, Proposal 048 delivery preflight, frozen RunPlan snapshots, `git_push_receipt_v1` |
| Target state | An opt-in GitHub pull-request publication SystemTask that creates at most one PR for a verified pushed run branch, without invoking an agent or ACP provider. |
| Goal | Make the post-push PR creation that was performed manually for P089 a durable, deterministic, auditable control-plane step. |

---

## 1. Context

A repo-backed run can create and push its delivery branch, but today it stops at
`git_push_receipt`. Opening a pull request is an external manual operation.
For P089 the branch was pushed successfully and an operator later created the
PR on GitHub. That split weakens the delivery receipt: it proves a branch was
pushed, but it cannot prove whether a review surface was created, which branch
was proposed, or whether a later retry opened a duplicate PR.

This must not be solved by prompting an agent to run `gh pr create`. An agent
cannot be the authority for an irreversible, externally visible side effect;
its output could select the wrong repository, target branch, title, or body.
The side effect belongs to the control plane, with the same frozen inputs,
idempotency, cancellation, and readback discipline as other system-owned work.

## 2. Decision

Add an opt-in, GitHub-only SystemTask:

```yaml
system_task:
  task_type: github_pull_request_publisher
  executor_mode: system.delivery.github_pull_request
```

The task runs only after a valid `git_push_receipt_v1` has established the
remote, branch, and pushed commit. It uses a native GitHub REST client owned by
the daemon. It never starts an ACP subprocess, never creates an
`AgentExecution`, and never forwards GitHub credentials to an agent
environment.

The first implementation creates a PR only. It does not merge, approve,
request reviewers, modify labels, close an existing PR, or push additional
commits.

### 2.1 Opt-in frozen configuration

Extend `DeliveryConfiguration` with an optional `pull_request_publication`
object. Absent configuration means disabled, preserving all existing and
already-frozen workflows.

```json
{
  "provider": "github",
  "repository": "owner/repository",
  "base_branch": "main",
  "mode": "draft",
  "title_template": "${project_key}: ${idea_title}",
  "credential_ref": "keychain:chainworks.github.operator"
}
```

Required invariants:

- `provider` is exactly `github` in the first release;
- `repository` matches the canonical `repo_identifier`, not merely a local Git
  remote string;
- `base_branch` equals the frozen delivery `base_branch`;
- the head is the frozen `target_branch`, never a value supplied by an agent or
  a retry request;
- `mode` is explicitly `draft` or `ready`; there is no implicit default;
- `title_template` uses a bounded allow-list of placeholders
  (`project_key`, `idea_title`, `run_id_short`) and has a rendered length cap;
- the body is a versioned daemon template containing only the resolved
  repository, base/head, pushed commit, run ID, and receipt links; arbitrary
  model-generated Markdown is out of scope;
- `credential_ref` identifies a daemon-local secret. The token itself is never
  persisted in the run, command journal, artifact, or child-process environment.

Run-start preflight must validate this object before creating a run, freeze its
canonical JSON with the delivery configuration, and record a redacted result.

### 2.2 Workflow placement

For a workflow that opts in, the manual release gate remains the human decision
to authorize delivery. Once the existing release path has produced a successful
`git_push_receipt`, a new state runs the SystemTask and transitions only when a
valid `pull_request_receipt` exists:

```yaml
state_11_pull_request_published:
  label: Pull request published
  owner: lead_orchestrator
  run:
    system_task:
      task_type: github_pull_request_publisher
      executor_mode: system.delivery.github_pull_request
  transitions:
    - to: state_12_workflow_complete
      when: exists('pull_request_receipt')
```

The task is skipped only when publication is disabled in the frozen delivery
configuration. A new run plan is required to opt in; no startup repair or
retrofit may add publication to an existing frozen run.

## 3. Execution and Side-Effect Contract

Before contacting GitHub, the daemon must validate all of the following:

1. the run is non-terminal, not cancelling, and is in the configured
   publication state;
2. the manual release approval has been durably granted;
3. exactly one current `git_push_receipt_v1` exists, has `status=success`, and
   names the frozen target branch and expected remote;
4. the receipt's commit SHA is non-empty and the GitHub branch ref resolves to
   that SHA;
5. the frozen repository and base branch match the request that will be sent;
6. the configured credential can read the repository and create PRs; and
7. no terminal publication record already exists for the deterministic idempotency
   key.

The deterministic key is SHA-256 over the run ID, provider, repository, base
branch, head branch, pushed commit SHA, and publication configuration version.
It is persisted before the network request. A unique constraint on
`(provider, repository, base_branch, head_branch, head_commit_sha)` prevents
duplicate PR creation across crash recovery and concurrent workers.

The executor first lists existing open and closed PRs for the exact
repository/base/head tuple. If a matching PR already exists, it verifies the
head SHA and records `already_exists`; it does not create another. A `422`
create response follows the same lookup-and-verify path. A base/head or SHA
mismatch is fail-closed and leaves the stage blocked with a typed reason.

The daemon uses an in-process HTTPS client against GitHub's REST API. It must
not shell out to `gh`, run arbitrary Git commands, or rely on an interactive
browser session. Timeouts, retry classification, TLS failures, and rate-limit
`Retry-After` values are persisted in the publication attempt record. Only
transport-safe retry classes may be automatically retried; an ambiguous
post-request connection failure always performs lookup before another create.

## 4. Persistence and Receipts

Add a dedicated `pull_request_publications` record with:

- run and stage execution IDs;
- deterministic idempotency key and request fingerprint;
- provider, repository, base/head branches, and head commit SHA;
- state (`admitted`, `creating`, `created`, `already_exists`, `failed_closed`,
  `cancelled_before_dispatch`);
- GitHub PR number, URL, node ID, draft state, and remote state when known;
- redacted retry/error classification, timestamps, and credential reference
  fingerprint;
- request/response redaction version.

The SystemTask also persists a `system_executions` row and writes a canonical
`pull_request_receipt_v1` artifact. The receipt contains the PR URL/number,
the exact base/head/commit identity, creation disposition, and the linked
`git_push_receipt` checksum. It contains no authorization header, token,
cookie, raw HTTP body, or unbounded GitHub response.

`run_report`, `release_delivery_receipt`, GraphQL, MCP `reports.get`, and the
read-only macOS UI must expose the same projected publication status. A missing
receipt is not rendered as a successful PR; an unavailable projection is
explicitly `unknown`/`unavailable`.

## 5. Cancellation, Recovery, and Authorization

- The task checks cancellation before lease acquisition, before dispatch, and
  after an ambiguous response. Cancellation before dispatch records
  `cancelled_before_dispatch` and makes no remote call.
- Once GitHub may have received a create request, recovery performs lookup by
  the frozen base/head tuple and commits the observed result. It never deletes
  a remotely created PR as compensation.
- A restart resumes only the durable publication record with its original
  idempotency key. It cannot create a new record for the same identity.
- Missing, revoked, or insufficient credentials fail closed. There is no
  fallback to an agent, `gh`, a different account, or a generic shell command.
- Only the daemon's delivery authority can load `credential_ref`. Readbacks
  reveal the credential reference fingerprint at most; they never reveal a
  secret or the raw response.
- Automatic merge, approval, reviewer assignment, label changes, and branch
  deletion are prohibited by both API scope and authorization policy.

## 6. Non-Goals

- GitLab, Bitbucket, forks, cross-repository PRs, and arbitrary web hosts.
- Generating review prose with an LLM.
- Replacing the existing code-writing or git-push task.
- Creating PRs for existing terminal runs or changing a frozen run snapshot.
- Auto-merge or any action that changes the target branch.
- An operator-facing UI mutation. The macOS client remains read-only for this
  side effect.

## 7. Acceptance Criteria

1. A workflow with the explicit system task and frozen opt-in configuration
   creates exactly one GitHub PR after a verified successful push.
2. The execution creates zero ACP subprocesses and zero `AgentExecution`
   records; it records a SystemTask execution and publication receipt instead.
3. Missing configuration, invalid configuration, repository/base/head mismatch,
   missing receipt, failed push, stale SHA, cancellation, missing credential,
   insufficient permission, and unsupported provider all fail before an
   unintended create request.
4. Repeating the task, restarting during it, and receiving an ambiguous network
   failure converge on the same PR without duplication.
5. An already-existing PR is verified and reported as `already_exists`; a
   mismatching existing PR is blocked with evidence.
6. Tokens and raw authorization material are absent from SQLite projections,
   command journal, artifacts, MCP, GraphQL, logs, and agent environments.
7. GraphQL, MCP, run report, release receipt, and Swift read projection agree
   on PR number, URL, base/head, commit SHA, and disposition.
8. The system never merges, approves, edits, closes, or deletes a PR or branch.

## 8. Verification Matrix

- Deterministic unit tests for template rendering, configuration validation,
  frozen identity construction, idempotency-key stability, and redaction.
- Mock-GitHub integration tests for create, pre-existing PR, 422 lookup,
  retry-after, timeout-before-response, timeout-after-request, rate limit,
  permission denial, and malformed response.
- Engine tests proving a SystemTask is recorded without an ACP launch or agent
  execution, and that the transition cannot complete without the receipt.
- Migration/recovery tests for concurrent admission, restart at every durable
  state, cancellation before/after dispatch, and duplicate-worker contention.
- Cross-surface contract tests for GraphQL, MCP, run report, release receipt,
  and macOS DTO parity, including absence of secret fields.
- A real sandbox-repository canary creates a draft PR, verifies the receipt by
  readback, and leaves merge and cleanup to an operator.

## 9. Rollout and Rollback

The capability ships disabled. It is first enabled only for an explicit
sandbox workflow configuration with draft mode and a least-privilege GitHub
credential. Promotion requires two clean canaries with matching cross-surface
receipts and no duplicate-creation or secret-redaction findings.

Rollback disables new admissions through the delivery configuration feature
flag. It does not mutate, close, or delete any PR already created. Existing
publication records and receipts remain available for audit and recovery.
