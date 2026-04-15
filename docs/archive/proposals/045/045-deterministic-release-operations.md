# Proposal 045: Deterministic Release Operations (Git + Sandbox Publish)

| Field | Value |
|---|---|
| Date | 2026-04-14 |
| Status | Draft |
| Author | Claude |
| Depends on | [044-post-approval-task-execution-and-release-gate-completion.md](044-post-approval-task-execution-and-release-gate-completion.md) |
| Scope | (A) Port Swift `GitReleaseService`, `ConnectPublishService`, `ReleaseOpsCoordinator`, and `DeliveryReceiptBuilder` to the Rust daemon. (B) Add `delivery_configuration_json` input path through `StartRunCmd`, command_handler, and northbound surfaces so the frozen config reaches the run. |
| Goal | Release agents execute through native Rust services (not ACP), a frozen `DeliveryConfiguration` flows from run creation to release execution, and a structured `delivery_receipt` artifact is persisted on happy paths plus release-attempt failure paths, with terminal backfill only when the stable Swift owner chain has enough release lineage to compute `currentReleaseResultSummary()`. |

---

## 1. Context and Motivation

The Swift app routes release agents through `ReleaseOpsCoordinator` → `GitReleaseService` / `ConnectPublishService` instead of the generic ACP executor. This is a deliberate safety decision (ARCH-069): release side effects must be **deterministic** — no LLM decides how to `git push` or what to archive.

**Current stable baseline:**
- Release is limited to `sandbox` and `staging` modes (ARCH-072). Production upload is intentionally excluded.
- `ConnectPublishService` attempts `xcodebuild build` for compilability verification, computes an archive checksum from worktree state, and **records a safe-mode receipt without real App Store Connect communication**.
- Build failure in sandbox is not fatal — it records `status: "build_warning"` and continues.
- `DeliveryReceiptBuilder` persists a structured `delivery_receipt` artifact on run completion when the stable owner chain has enough data to build it: `delivery_configuration_json`, `worktree_root`, and a non-nil release-result summary derived from prior release-agent lineage. This receipt is consumed by the operator UI, delivery proof gates, and report readers.

The Rust daemon currently sends all agents through ACP, including release agents, and has **no owner** that supplies a frozen `DeliveryConfiguration` at run start.

---

## 2. Design

### 2a. DeliveryConfiguration Input Path (New)

**Problem:** `delivery_configuration_json` is defined on the `Run` model (added in P007) but no northbound surface or command can ever set it. `StartRunCmd` has no delivery-config field; command_handler persists `None`; MCP `runs.start` and GraphQL `startRun` have no delivery-config parameter.

**Fix — end-to-end input chain:**

**2a-i. `domain/src/commands.rs` — Add to `StartRunCmd`:**

```rust
pub struct StartRunCmd {
    // ... existing fields ...
    /// Frozen delivery configuration JSON. Required for repo-backed runs.
    /// Serialized `DeliveryConfiguration`. None for non-repo runs.
    pub delivery_configuration_json: Option<String>,
}
```

**2a-ii. `engine/src/command_handler.rs` — Persist at run creation:**

```rust
let run = Run {
    // ... existing fields ...
    delivery_configuration_json: c.delivery_configuration_json.clone(),
};
```

**2a-iii. `mcp-server/src/tools/runs.rs` — Accept at northbound MCP surface:**

```rust
let delivery_configuration_json = params["delivery_configuration_json"]
    .as_str()
    .map(String::from);

let cmd = Command::StartRun(StartRunCmd {
    // ... existing fields ...
    delivery_configuration_json,
});
```

**2a-iv. `graphql-server/src/schema.rs` — Accept at northbound GraphQL surface:**

Add `deliveryConfigurationJson: Option<String>` to the `startRun` mutation input.

**Validation:** `execute_release_agent` deserializes the JSON string into `DeliveryConfiguration`. If deserialization fails or the field is None → fail closed. The frozen config provides: `repo_identifier`, `repo_root`, `base_branch`, `target_branch`, `release_target_id`, `release_mode`.

### 2b. ReleaseOpsCoordinator (Rust)

A new top-level coordinator that mirrors Swift's `ReleaseOpsCoordinator.executeRelease()`:

```rust
// engine/src/release/coordinator.rs

pub struct ReleaseResult {
    pub git_manifest: Option<ReleaseManifest>,
    pub git_receipt: Option<GitPushReceipt>,
    pub bundle_manifest: Option<ReleaseBundleManifest>,
    pub upload_receipt: Option<ConnectUploadReceipt>,
    pub succeeded: bool,
    pub failure_stage: Option<String>,
    pub failure_reason: Option<String>,
}

pub async fn execute_release(
    delivery_config: &DeliveryConfiguration,
    worktree_root: &str,
    commit_message: &str,
) -> ReleaseResult;
```

**Sequence (matching Swift §9):**
1. Call `git::commit_and_push()` → produces `ReleaseManifest` + `GitPushReceipt`
2. If step 1 fails → return `ReleaseResult { succeeded: false, failure_stage: "commit_and_push", git_manifest: None, ... }`
3. Call `connect::build_and_distribute()` with `git_receipt` + `release_manifest` as inputs → produces `ReleaseBundleManifest` + `ConnectUploadReceipt`
4. If step 3 fails → **partial failure**: return `ReleaseResult { succeeded: false, failure_stage: "build_archive_and_push", git_manifest: Some, git_receipt: Some, bundle_manifest: None, ... }`
5. If both succeed → return full `ReleaseResult { succeeded: true, ... }`

### 2c. GitReleaseService (Rust)

```rust
// engine/src/release/git.rs

pub struct ReleaseManifest {
    pub commit_sha: String,
    pub branch: String,
    pub files_changed: usize,
    pub insertions: usize,
    pub deletions: usize,
    pub timestamp: DateTime<Utc>,
}

pub struct GitPushReceipt {
    pub status: String,       // "success" | "failed"
    pub branch: String,
    pub commit_sha: String,
    pub remote: String,
    pub failure_reason: Option<String>,
    pub timestamp: DateTime<Utc>,
}

pub async fn commit_and_push(
    worktree_root: &str,
    target_branch: &str,
    commit_message: &str,
) -> Result<(ReleaseManifest, GitPushReceipt)>;
```

**Algorithm (matching Swift `GitReleaseService.commitAndPush`):**
1. `git -C {worktree} status --porcelain` — enumerate changes
2. `git -C {worktree} diff --stat HEAD` — count files_changed / insertions / deletions
3. `git -C {worktree} add -A` — stage all
4. `git -C {worktree} commit -m {message}` — commit
5. `git -C {worktree} rev-parse HEAD` — record commit SHA
6. `git -C {worktree} push origin {target_branch}` — push
7. Write `release_manifest.json` and `git_push_receipt.json` to artifact paths

### 2d. ConnectPublishService (Rust) — Sandbox/Staging Safe Mode

```rust
// engine/src/release/connect.rs

pub struct ReleaseBundleManifest { /* ... same as before ... */ }
pub struct ConnectUploadReceipt { /* ... same as before ... */ }

pub async fn build_and_distribute(
    worktree_root: &str,
    git_push_receipt: &GitPushReceipt,
    release_manifest: &ReleaseManifest,
    delivery_config: &DeliveryConfiguration,
) -> Result<(ReleaseBundleManifest, ConnectUploadReceipt)>;
```

**Critical: Input owner chain.** The function explicitly consumes `git_push_receipt` and `release_manifest` from the prior git step.

**Algorithm (matching Swift `ConnectPublishService.buildArchiveAndUpload` sandbox mode):**
1. Validate `git_push_receipt.status == "success"` — fail with `missingGitPushReceipt` otherwise
2. Attempt `xcodebuild build` (macOS only) — compilability check, not real archive
3. Build failure is **not fatal** for sandbox — record `status: "build_warning"`
4. Compute deterministic checksum from `release_manifest.commit_sha + files_changed + insertions + deletions`
5. Measure worktree directory size
6. Produce `ReleaseBundleManifest` + `ConnectUploadReceipt` with `destination: "{release_mode}://{release_target_id}"`
7. Write `release_bundle_manifest.json` and `connect_upload_receipt.json` to artifact paths

**No real App Store Connect communication in v1.**

### 2e. DeliveryReceiptBuilder (Rust) — Structured Receipt Persistence

```rust
// engine/src/release/receipt.rs

pub struct DeliveryReceipt {
    pub run_id: String,
    pub workflow_id: String,
    pub idea_title: String,
    pub delivery_config: DeliveryConfiguration,
    pub worktree_root: String,
    pub base_revision: Option<String>,
    pub release_result: Option<ReleaseResultSummary>,
    pub implementation_review_status: Option<String>,
    pub timestamp: DateTime<Utc>,
}

pub struct ReleaseResultSummary {
    pub commit_sha: Option<String>,
    pub branch: Option<String>,
    pub remote: Option<String>,
    pub files_changed: Option<usize>,
    pub succeeded: bool,
    pub failure_stage: Option<String>,
    pub failure_reason: Option<String>,
}

pub fn build_receipt(
    run: &Run,
    delivery_config: &DeliveryConfiguration,
    release_result: &ReleaseResult,
    idea_title: &str,
    review_status: Option<&str>,
) -> DeliveryReceipt;
```

**Multiple write sites with preserve guard** (matching Swift `persistDeliveryReceiptIfNeeded` at line 4510/4539 + release failure sites at line 1524 and 1596):

Swift writes the delivery receipt at **four** sites: git failure handler (line 1524), publish failure handler (line 1596), publish success handler (via `persistDeliveryReceiptIfNeeded`), and terminal-state fallback (line 324/350/383). Each site guards with `producedArtifactNames.contains("delivery_receipt") == false` — the first writer wins, later sites skip.

The Rust daemon mirrors this with **three** explicit write sites plus the state_12 fallback:

| Write site | When | Content |
|---|---|---|
| `execute_git_release` failure handler | Git push fails | `ReleaseResultSummary { succeeded: false, failure_stage: "commit_and_push" }` — git fields unpopulated |
| `execute_publish_release` failure handler | Publish fails after push succeeded | `ReleaseResultSummary { succeeded: false, failure_stage: "build_archive_and_push" }` — git fields populated |
| `execute_publish_release` success handler | Both steps succeed | `ReleaseResultSummary { succeeded: true }` — all fields populated |
| State_12 `finalize_run_and_produce_receipts` | Run reaches end state | Backfill only — if receipt was never written and finalization still has the full Swift eligibility chain: `delivery_config` + `worktree_root` + a non-nil `currentReleaseResultSummary()` derived from prior release-agent lineage |

**Every write site applies the preserve guard:**

```rust
fn write_delivery_receipt_if_absent(
    receipt: &DeliveryReceipt,
    plan: &RunPlan,
    workspace_root: &str,
) -> Result<bool> {
    let path = resolve_path_template(
        plan.artifact_paths.get("delivery_receipt").unwrap_or(&String::new()),
        workspace_root,
    );
    if std::path::Path::new(&path).exists() {
        return Ok(false);  // already written by earlier site — preserve
    }
    let json = serde_json::to_string_pretty(receipt)?;
    std::fs::create_dir_all(std::path::Path::new(&path).parent().unwrap_or(std::path::Path::new(".")))?;
    std::fs::write(&path, json)?;
    Ok(true)
}
```

**Pre-release failures** (missing delivery config, missing worktree) fail the work item before any release service is called. No receipt is written by the executor — the pre-release failure is recorded in the work item's error. Under strict Swift parity, state_12 does **not** synthesize a metadata-only receipt for that path, because `persistDeliveryReceiptIfNeeded(...)` first requires `currentReleaseResultSummary()`, and that is nil when no release-agent lineage exists. If the run blocks without any release-agent lineage, no receipt exists — this is correct: the receipt records release truth, and no release was attempted.

### 2f. Executor Routing — Per-Agent, Not One Coordinator Call

P044's N-phase ordering guarantees `commit_and_push_to_github` (phase 0) completes before `build_archive_and_push_connect` (phase 1) is enqueued. Each agent is a separate `InvokeAgent` work item processed independently by the executor. The executor routes each **individually** — matching Swift `WorkflowOrchestrator.executeReleaseAgentTask` (line 1473: `if agent.id == "commit_and_push_to_github"`, line 1538: `else if agent.id == "build_archive_and_push_connect"`).

```rust
// executor.rs — per-agent routing, NOT one coordinator call
match agent_id.as_str() {
    "commit_and_push_to_github" => {
        return self.execute_git_release(run, stage_id, ...).await;
    }
    "build_archive_and_push_connect" => {
        return self.execute_publish_release(run, stage_id, ...).await;
    }
    _ => { /* normal ACP path */ }
}
```

**`execute_git_release`:**
1. Deserializes `run.delivery_configuration_json` → `DeliveryConfiguration`; fail closed if None
2. Calls `GitReleaseService::commit_and_push(worktree_root, delivery_config.target_branch, commit_message)`
3. Writes `release_manifest.json` and `git_push_receipt.json` to canonical paths
4. On success → work item Completed
5. On failure → calls `write_delivery_receipt_if_absent` with structured `ReleaseResultSummary { succeeded: false, failure_stage: "commit_and_push" }`, work item Failed

**`execute_publish_release`:**
1. Deserializes `run.delivery_configuration_json` → `DeliveryConfiguration`; fail closed if None
2. Reads `git_push_receipt.json` and `release_manifest.json` from canonical paths (produced by phase 0)
3. If either file missing → fail with "requires git_push_receipt and release_manifest inputs" (matching Swift line 1540-1547)
4. Calls `ConnectPublishService::build_and_distribute(worktree_root, &git_push_receipt, &release_manifest, &delivery_config)`
5. Writes `release_bundle_manifest.json` and `connect_upload_receipt.json` to canonical paths
6. Calls `write_delivery_receipt_if_absent` with full `ReleaseResult` — succeeds on happy path, records partial truth on failure (git succeeded, publish failed)
7. On success → work item Completed; on failure → work item Failed

**Why per-agent, not one coordinator call:** `ReleaseOpsCoordinator::execute_release()` runs both steps atomically inside one call. But P044 sequences them as separate InvokeAgent work items in separate phases. If the executor called the full coordinator for the first agent, it would execute publish before P044 has even enqueued the publish agent. The coordinator remains useful as a convenience for tests and the delivery receipt builder, but the executor must call each service separately.

### 2g. Failure Truth — Structured, Not None

**Git push failure** (execute_git_release step 5):
- `release_manifest.json` and `git_push_receipt.json` are **not produced** (git service threw)
- `delivery_receipt.json` **is produced** by `execute_git_release`'s failure handler via `write_delivery_receipt_if_absent` with structured `ReleaseResultSummary { succeeded: false, failure_stage: "commit_and_push", failure_reason: "..." }` — NOT `release_result: None`
- `release_result: None` is not the pre-release default in this proposal; strict Swift parity does not backfill a receipt when no release-agent lineage exists
- Work item marked Failed; P044 phase gating skips phase 1 (publish never enqueued)

**Publish failure after successful push** (step 2 fails):
- `release_manifest.json` and `git_push_receipt.json` are **already persisted** (from phase 0)
- `release_bundle_manifest.json` and `connect_upload_receipt.json` are **not produced**
- `delivery_receipt.json` **is produced** with `ReleaseResultSummary { succeeded: false, failure_stage: "build_archive_and_push", ... }` including populated git fields (`commit_sha`, `branch`, `files_changed`)
- Work item marked Failed; stage settles as Failed; run blocks
- **No hidden rollback** — the git push has already happened

**Pre-release failure** (config missing, worktree missing):
- No release service called
- No executor-side `delivery_receipt.json` is produced, because the receipt schema requires `delivery_config` and `worktree_root`
- Work item Failed immediately
- State_12 finalization does not backfill this path unless prior release-agent lineage exists and finalization can compute a non-nil release result summary; otherwise no receipt exists

---

## 3. Files to Create/Modify

| File | Change |
|---|---|
| `domain/src/commands.rs` | Add `delivery_configuration_json: Option<String>` to `StartRunCmd` |
| `engine/src/command_handler.rs` | Pass `delivery_configuration_json` through to `Run` on creation |
| `mcp-server/src/tools/runs.rs` | Accept `delivery_configuration_json` in `runs.start` params |
| `graphql-server/src/schema.rs` | Accept `deliveryConfigurationJson` in `startRun` mutation |
| `engine/src/release/mod.rs` | **NEW** — Release operations module |
| `engine/src/release/coordinator.rs` | **NEW** — `ReleaseOpsCoordinator::execute_release()` with partial-failure handling |
| `engine/src/release/git.rs` | **NEW** — `GitReleaseService::commit_and_push()` |
| `engine/src/release/connect.rs` | **NEW** — `ConnectPublishService::build_and_distribute()` — sandbox/staging safe mode |
| `engine/src/release/receipt.rs` | **NEW** — `DeliveryReceiptBuilder::build_receipt()` — persists delivery_receipt on happy paths, release-attempt failure paths, and terminal backfill paths that already have release-agent lineage |
| `engine/src/executor.rs` | Route release agents to native services; deserialize delivery config; persist delivery receipt |
| `engine/src/lib.rs` | Register `pub mod release` |

---

## 4. Safety Constraints

1. All git operations use explicit absolute paths — no reliance on cwd
2. Push target branch must match `delivery_config.target_branch`
3. Push to `main` / `master` is rejected (hard-coded guard)
4. If push fails → git step fails, no publish attempted, no release artifacts persisted except delivery_receipt with failure
5. If publish fails after push succeeds → git artifacts preserved, delivery_receipt persisted with failure, run blocks, no rollback
6. `delivery_configuration_json` deserialization failure → fail closed
7. Commit message includes run ID and idea title for traceability
8. **No real App Store Connect communication** — sandbox/staging receipt mode only (ARCH-072)

---

## 5. Acceptance Criteria

1. `delivery_configuration_json` accepted at MCP `runs.start` and GraphQL `startRun`, persisted on `Run`, deserialized at release time.
2. `commit_and_push_to_github` produces `release_manifest.json` and `git_push_receipt.json` without ACP.
3. `build_archive_and_push_connect` consumes `git_push_receipt` and `release_manifest` as inputs, produces `release_bundle_manifest.json` and `connect_upload_receipt.json`.
4. `delivery_receipt.json` persisted at canonical path on happy paths and on release-attempt failure paths, with correct `ReleaseResultSummary`.
5. `git log` on the target branch shows the commit from the daemon.
6. All artifacts written to canonical paths → transition `exists('git_push_receipt')` evaluates true.
7. If push fails → delivery_receipt has structured `ReleaseResultSummary { succeeded: false, failure_stage: "commit_and_push" }` — NOT `release_result: None`. No publish artifacts. Phase 1 never enqueued.
8. If push succeeds but publish fails → git artifacts preserved, delivery_receipt has `succeeded: false, failure_stage: "build_archive_and_push"` with populated git fields, run blocks.
9. If `delivery_configuration_json` is None → fail closed with clear error; no executor-side `delivery_receipt` is produced on that pre-release failure path.
10. **Preserve-vs-backfill:** If `delivery_receipt.json` already exists at canonical path when `execute_publish_release` runs, it is NOT overwritten. State_12's `finalize_run_and_produce_receipts` agent sees the existing receipt as authoritative.
11. **Backfill eligibility:** If delivery_receipt was never written, state_12 can backfill it only when finalization still has `delivery_config`, `worktree_root`, and a non-nil `currentReleaseResultSummary()` derived from prior release-agent lineage. Pre-release failures with no release-agent lineage do not get a metadata-only backfill receipt.
12. No LLM involved in the release path.
13. `connect_upload_receipt.release_mode` is `"sandbox"` or `"staging"` — never `"production"`.

---

## 6. Test Gate

P045's proof lane follows the P027 pattern (Rust daemon-only, `cargo test`).

### test-gates.md Entry

```
### `proposal-045`

Deterministic release operations gate (git + sandbox publish).

Scope:

- delivery_configuration_json input path through StartRunCmd → command_handler → MCP/GraphQL
- GitReleaseService commit/push with branch safety guard
- ConnectPublishService sandbox receipt generation
- ReleaseOpsCoordinator partial-failure semantics
- DeliveryReceiptBuilder persistence on happy paths, release-attempt failure paths, and terminal backfill paths with prior release-agent lineage
- executor routing bypass around ACP for release agents

Command:

\`\`\`bash
./scripts/test-gate.sh proposal-045
\`\`\`
```

### test-gate.sh Entry

```bash
PROPOSAL_045_TESTS=(
  "engine::tests::test_delivery_config_input_path"
  "engine::tests::test_git_release_commit_and_push"
  "engine::tests::test_connect_publish_sandbox_receipt"
  "engine::tests::test_release_partial_failure_preserves_git_artifacts"
  "engine::tests::test_delivery_receipt_persisted_on_release_paths_and_eligible_backfill"
  "engine::tests::test_executor_routes_release_agents_to_native_services"
  "engine::tests::test_main_branch_push_rejected"
)
```

Gate runner:

```bash
proposal-045|p045)
  log "Proposal 045 control-plane gate: deterministic release operations"
  (
    cd "$ROOT_DIR/control-plane"
    cargo test --workspace 2>&1
  )
  log "Proposal 045 control-plane gate passed"
  ;;
```

---

## 7. Out of Scope

- **Real App Store Connect upload** (altool, notarytool, Transporter): Not part of the current baseline. Deferred per ARCH-072.
- **Production release mode**: Current contract is sandbox/staging only.
- **Post-approval orchestration**: Covered by P044.
- **How the operator selects a repo profile**: UI/CLI concern. P045 provides the input path; population is the caller's responsibility.
