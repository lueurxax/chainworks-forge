# Managed Temporary Artifact Inventory

This document is the implemented contract for the managed temporary artifact inventory in Chainworks Forge. It serves as the canonical source of truth for the read-only, dry-run-only advisory inventory slice.

| Field | Value |
|---|---|
| Status | Implemented (read-only, dry-run-only smoke slice; `disabled` by default — see [§1.1](#11-implementation-status-by-lane-current-slice)) |
| Implementation readiness | Implemented across the MCP, GraphQL, run report, release receipt, and packaged Swift Run Report lanes. `operator_visible` promotion is still held on redaction-key initialization reconciliation, contract-fixture reconciliation, and packaged remote UI/accessibility evidence; deletion, cleanup, durable persistence, and scheduled sweeps remain out of scope. |
| Implementation gate | `./scripts/test-gate.sh proposal-089-temp-inventory` / `p089-temp-inventory` |
| Author | Codex |
| Implementation evidence | Eight contract fixtures are checked in under [`docs/evidence/089/temp-inventory/contracts/`](../evidence/089/temp-inventory/contracts) (GraphQL SDL, MCP request/result schemas, run-report and release-receipt schemas, enum projection matrix, DateTime nullability parity, over-2GB `ByteCountString`), but the required `status-by-field-matrix.fixture.json` is missing and the existing fixtures are **not yet reconciled with the shipped schema; see [§4.4](#44-graphql-projection-specifics-implemented)**. Operator readback: [`p089-temp-inventory-full-surface.fixture.json`](../evidence/rollout-contract/operator-readback/p089-temp-inventory-full-surface.fixture.json) — no lane records a captured operator observation yet: `run_report`, `release_receipt`, and `graphql` are `evidence_not_captured`, `mcp` is `implemented_smoke_slice`, and packaged smoke is `not_run` on every lane that declares it (`graphql` omits the field entirely). Negative fixtures: `docs/evidence/rollout-contract/negative/p089-temp-inventory-*.json`. |
| Related | `docs/reference/test-gates.md`, `docs/reference/rust-control-plane.md`, `docs/reference/query-projections-and-client-consumption-contract.md` |

---

## 1. Overview and Scope

Chainworks Forge can create substantial temporary directories during gate runs, provider sessions, copied toolchains, caches, and general workspace operations (such as timestamped Xcode `DerivedData` or `.xcresult` structures). Left unchecked, these directories can cause significant disk pressure on developer machines.

The managed temporary artifact inventory provides a robust, **read-only and dry-run-only** capability to discover and categorize these temporary artifact roots. To guarantee maximum stability and prevent accidental data loss:
- No deletion, cleanup mutation, pruning, compaction, manifest migration, chmod, or file rewrite capability exists in this slice.
- No durable database table, SQLite inventory table, or SwiftData inventory model is utilized; scans are performed purely on demand and exposed as transient readback.
- No global diagnostics panel is used; the UI presentation is strictly scoped to the **Run Report Diagnostics** panel of the packaged macOS app.

### 1.1 Implementation Status by Lane (Current Slice)

This is a smoke slice: read-only inventory plus advisory dry-run readback, `disabled` by default. Every readback lane shares one implementation — the MCP-side `execute_inventory_preview` path in `control-plane/crates/mcp-server/src/tools/temp_artifacts.rs`, which owns mode checking, request validation, permit admission, scanning, redaction, and the mutation guard — so no lane can drift from the canonical DTO:

- **MCP** — the `temp_artifacts.inventory.preview` tool plus the read-only resource `chainworks://runs/{run_id}/temp-artifact-inventory`, which routes through the same request parser and scanner path. Both are restricted to `Operator` principals — `Observer`, `Agent`, and `ReadOnlyOperator` are denied capability access because the preview exposes run-scoped metadata with cross-run disclosure risk (SEC-P089-HIGH-002) — and dispatch directly without emitting a `command_journal` row.
- **GraphQL** — `tempArtifactInventory(input:)`, Operator-only via `require_operator_read`. The daemon installs the MCP-backed backend at startup (`graphql_server::types::temp_artifact_inventory::install_backend` with `mcp_server::tools::temp_artifacts::McpTempArtifactInventoryBackend`), so this lane runs the identical scan path and returns a lossless camelCase projection of the same canonical DTO. graphql-server cannot depend on mcp-server directly, so the trait plus process-static handle inverts the dependency. A process that builds a bare schema without installing the backend (unit tests) gets disabled readback with `disabled_reason_code = "backend_not_wired"`.
- **Run report** and **release receipt** — `reports.get` (Operator-only) embeds a `temp_artifact_inventory` section in both lanes via `p089_temp_artifact_inventory_run_report_section` and `p089_temp_artifact_inventory_release_receipt_section`, each delegating to the same preview path. The release receipt carries the same complete canonical DTO as the run report so parity checks stay lossless. If the preview path returns an error, both report lanes substitute an *error-shaped* section rather than failing the report — `status = error`, `enabled_state = unknown`, `disabled_reason_code = null`, and one `internal_error` entry with a redacted message. The GraphQL lane does the same (`build_integrity_error_inventory`). A scan failure is therefore never projected as a disabled kill switch: only a genuinely `disabled` mode, or a GraphQL process without the backend installed, produces disabled readback.
- **Swift app surface** — `TempArtifactInventoryView` renders inside the Run Report surface (`RunsHomeView` reports section), gated by both `TempArtifactDiagnosticsVisibilityStore` and the backend `mode == operator_visible`, with `TempArtifactInventoryCommands` registered in the app command tree and `TempArtifactRowPasteboardWriter` backing redacted copy. It reads through `TempArtifactInventoryGraphQLFetcher` against the Operator-only GraphQL projection and never touches the filesystem.

In `disabled` mode — the default — each lane still applies its normal Operator authorization boundary, then the shared preview path short-circuits to disabled disposition before override validation, caller-class derivation, or any filesystem access.

What remains is redaction-key initialization reconciliation, contract-fixture completion and reconciliation, and promotion evidence, not lane wiring: the key is initialized on first hash use rather than daemon startup (see [§6.1](#61-redaction-key)), the required `status-by-field-matrix.fixture.json` is missing, the checked-in SDL/JSON Schema fixtures still diverge from the shipped schema, packaged remote UI/accessibility smoke is outstanding, and the operator-readback fixture has not been refreshed with captured observations. `operator_visible` promotion therefore stays held. The focused gate unconditionally requires all nine contract fixture paths plus passing operator-readback lane status and packaged-smoke status; it remains red until that evidence is completed and refreshed (see [`test-gates.md`](test-gates.md#proposal-089-temp-inventoryp089-temp-inventory)).

---

## 2. System Architecture and Boundaries

A strict architectural boundary divides backend ownership from UI presentation to prevent local path leaks and unauthorized modifications:

### 2.1 Backend Ownership (Rust Control Plane)
The Rust control plane is the sole execution and validation authority, owning:
- File system scanning, classification, and size estimation;
- Absolute path redaction and short path hash derivation;
- Advisory dry-run recommendations;
- Full mutation guards ensuring no side-effects occur;
- API contracts (GraphQL query and MCP tool/resource endpoints);
- Projection of inventory reports in Run Reports and Release Receipts;
- Scan deadlines, cooperative cancellation, permit gating, and backpressure metrics.

### 2.2 Frontend Ownership (SwiftApp / SwiftUI Client)
The SwiftUI app is a thin client restricted to presentation and user command dispatch, owning:
- High-fidelity UI presentation within Run Report Diagnostics;
- Focused keyboard and toolbar commands (Refresh, Cancel);
- Accessibility announcements and status token presentation;
- NSPasteboard-backed copy clipboard adapter;
- View-level, generation-scoped state management.

The Swift client is **explicitly forbidden** from:
- Scanning the local filesystem directly;
- Running local ownership inference;
- Persisting temporary inventory rows;
- Executing or triggering file-system deletion/cleanup actions;
- Receiving or rendering raw absolute paths.

---

## 3. Configuration and Modes

The inventory capability is governed by daemon-level and client-level flags:

### 3.1 Daemon Execution Modes
Configured via the daemon process environment variable `CHAINWORKS_TEMP_ARTIFACT_INVENTORY_MODE`:
- `disabled`: Default fallback (also the value used for any unrecognized string). All lanes expose a disabled disposition and do not scan roots.
- `hidden_readback`: Backend APIs and reports compute inventory readback for automated tests and scripts. The packaged surface stays hidden even if the independent app preference is true because the client also requires the returned backend mode to be `operator_visible`.
- `operator_visible`: The scanning backend is active. The packaged app displays the diagnostics surface only when the independent client visibility key is also enabled.

### 3.2 App Visibility Store
The client controls one half of temporary artifact surface visibility via `TempArtifactDiagnosticsVisibilityStore`, which reads and writes the `TempArtifactDiagnosticsVisible` boolean preference in the `com.chainworks.forge` user defaults domain. This is LaunchServices-safe and test-injectable. `TempArtifactInventoryViewModel.resolveBackendVisibility` performs a bounded GraphQL visibility probe and requires the returned mode to be `operator_visible`; a stale true preference therefore cannot expose a `hidden_readback` or `disabled` backend. Changing the backend mode does not clear the local preference, so rollout and rollback should still set both switches explicitly.

### 3.3 Scan Root Discovery
In enabled modes, a `run_id`-scoped request scans that run's meta directory under `${CHAINWORKS_META_ROOT}/runs/<run_id>`, with both the `runs/` parent and the candidate run directory canonicalized and containment-checked before admission. When the meta root has the normal `<workspace>/.chainworks` shape, it also admits that workspace's `.chainworks/cargo-target`, `.forge-codex-acp`, and `.chainworks/tmp` descendants without broadening the run-meta portion to other runs.

Every enabled scan also admits the process-wide managed and legacy roots below. There is no separate global-root opt-in; the common request limit, total deadline, cancellation checks, and permit bounds constrain their cost:

| Environment variable | Effect |
|---|---|
| `CHAINWORKS_TEMP_ARTIFACT_CONTROL_PLANE_CACHE_ROOT` | Overrides the `control_plane_cache` root (default: the ACP adapter cache root). |
| `CHAINWORKS_TEMP_ARTIFACT_PROVIDER_HOME_FALLBACK_ROOT` | Overrides the `provider_home_copy` fallback root (default: `$TMPDIR/forge-codex-acp`). |
| `CHAINWORKS_TEMP_ARTIFACT_LEGACY_TMP_ROOT` | Overrides the `legacy_chainworks_tmp` root (default: `$HOME/.chainworks/tmp`). |
| `CHAINWORKS_TEMP_ARTIFACT_DIAGNOSTIC_TEST_ROOTS` | Colon-separated absolute-path allowlist for `test_root_override` (see [§7.2](#72-diagnostic-test-root-override)). Empty or unset disables the diagnostic test root entirely. |

A `workspace_context` request admits the same fixed set of known Chainworks-managed descendants of the canonicalized workspace root — `.chainworks/runs` (`run_meta_root`), `.chainworks/cargo-target` (`control_plane_cache`), `.forge-codex-acp` (`provider_home_copy`), and `.chainworks/tmp` (`legacy_chainworks_tmp`) — each canonicalized and re-checked for containment under the workspace root. The workspace root itself and unrelated project files are never enumerated. Process-wide roots from the table above are appended to both run and workspace scopes by default.

A run-scoped request reaches per-workspace provider runtime-home copies when `${CHAINWORKS_META_ROOT}` uses the normal `<workspace>/.chainworks` layout. With a nonstandard meta-root layout, the scanner cannot infer a workspace association; it still scans the requested run directory and process-wide roots, while an explicit `workspace_context` request reaches the workspace descendants.

---

## 4. Canonical DTO Schema (`temp_artifact_inventory_v1`)

The canonical inventory readback is serialized as JSON in MCP, Run Reports, and Release Receipts using `snake_case`. The GraphQL layer exposes a value-lossless `camelCase` projection: every canonical field is reachable and every canonical enum domain is a GraphQL enum with `SCREAMING_SNAKE_CASE` items (see [§4.4](#44-graphql-projection-specifics-implemented) for the type-name and shape differences that remain against the checked-in SDL fixture). Every lane produces this same DTO from the shared preview path (see [§1.1](#11-implementation-status-by-lane-current-slice)).

### 4.1 Top-Level Fields
- `schema_version`: Must be `"temp_artifact_inventory_v1"`.
- `status`: Lifecycle state of the scan (see status enum below).
- `enabled_state`: Indicates if the backend is enabled (`"enabled"`, `"disabled"`, `"unknown"`).
- `mode`: Daemon process-start mode (`"disabled"`, `"hidden_readback"`, `"operator_visible"`); the Swift surface composes this with its local visibility preference.
- `disabled_reason_code`: Reason for a disabled state, or null.
- `generated_at`: ISO-8601 UTC timestamp of the snapshot.
- `limits_applied`: Bounded constraints of the current run — `limit`, `timeout_ms`, `scan_deadline_at`, and `queue_wait_ms`.
- `summary`: High-level aggregated statistics (total artifact counts, total byte counts, grouped classifications), plus `truncated` and `queue_wait_ms`.
- `rows`: List of discovered temporary artifacts.
- `errors`: List of aggregated scan errors (capped at 100).
- `dry_run`: Advisory summary of potential cleanup operations.
- `mutation_guard`: Validation showing that no mutations were executed.

### 4.2 Numeric Representation (`ByteCountString`)
To handle filesystems exceeding 2 GB, all byte counts are serialized as `ByteCountString` decimal strings (regex: `^(0|[1-9][0-9]*)$`).
- Numeric values, negative strings, leading-zero strings (except `"0"`), and whitespace are strictly rejected at boundaries.
- Values greater than 2 GB are preserved as string-format decimal integers in all lanes.

### 4.3 Implemented DateTime Shape
Timestamp fields (`generatedAt`, `scanDeadlineAt`, `lastTouchedAt`, `dryRun.generatedAt`) use the strict GraphQL `DateTime` scalar; JSON lanes serialize RFC3339/ISO-8601 UTC strings. Top-level `generatedAt` and row `generatedAt` are non-null in the shipped GraphQL projection, while `scanDeadlineAt`, `lastTouchedAt`, and `dryRun.generatedAt` are nullable. The canonical preview currently supplies `dry_run.generated_at` whenever `dry_run` is present, but the GraphQL field permits null. The checked-in DateTime fixture still does not prove lane parity because it marks release-receipt top-level `generatedAt` nullable even though the shared canonical DTO always supplies it. This remains part of the contract-fixture promotion hold described in [§4.4](#44-graphql-projection-specifics-implemented).

### 4.4 GraphQL Projection Specifics (Implemented)

The shipped GraphQL projection in `control-plane/crates/graphql-server/src/types/temp_artifact_inventory.rs` is the authority for the GraphQL lane. Enum coverage and value casing now agree with the projection-matrix fixture; the remaining differences against the checked-in contract fixtures under [`docs/evidence/089/temp-inventory/contracts/`](../evidence/089/temp-inventory/contracts) are **not** reconciled:

- **Missing status matrix.** The rollout contract and focused gate require `status-by-field-matrix.fixture.json`, but that file is not checked in. The gate therefore cannot pass even after its Rust and Swift test lanes succeed.
- **Enum coverage (partially reconciled).** The seven proposal-era domains are GraphQL enums whose item values render in `SCREAMING_SNAKE_CASE` (`COMPLETE`, `RESOURCE_EXHAUSTED`, `WOULD_KEEP_ACTIVE`) via `rename_items = "SCREAMING_SNAKE_CASE"`: `InventoryStatus`, `InventoryEnabledState`, `MutationGuardStatus`, `RootKind`, `LifecycleClassification`, `DryRunRecommendation`, `InventoryErrorCode`. Their value mapping matches `enum-value-projection-matrix.fixture.json`; the GraphQL *type name* for `enabled_state` still diverges (shipped `InventoryEnabledState`, SDL fixture `EnabledState`). The implementation also adds the canonical `mode` field and typed `InventoryMode` enum, which both the SDL fixture and enum projection matrix currently omit.
- **Type names and fields.** The implemented objects are `TempArtifactInventoryLimitsApplied` and `TempArtifactInventorySummary` (the SDL fixture names them `TempArtifactLimitsApplied` / `TempArtifactSummary`). `TempArtifactInventorySummary` carries `queueWaitMs`, which the fixture omits. `TempArtifactWorkspaceContextInput.workspaceRoot` is non-null in both the shipped input and fixture.
- **Dry run.** `TempArtifactDryRun` is `{ schemaVersion: String!, generatedAt: DateTime, recommendationCounts: JSON, mutationGuard: TempArtifactDryRunMutationGuard! }`. The SDL fixture omits `schemaVersion`; `recommendationCounts` is an untyped JSON object keyed by `dry_run_recommendation` value rather than the fixture's typed `TempArtifactDryRunCounts` object.
- **Mutation guard.** `TempArtifactMutationGuard` is `{ status, checkedAt, noDelete, noPrune, noChmod, noPersist, noRetry }`, not the fixture's `{ status, evidence }`. `TempArtifactDryRun.mutationGuard` is the object `TempArtifactDryRunMutationGuard { status, checkedAt }`, not the bare `MutationGuardStatus!` enum the fixture declares.
- **MCP result schema fixture.** `mcp-result-schema.fixture.json` sets top-level `additionalProperties: false` while omitting the canonical `mode` field, and sets `additionalProperties: false` on `summary` while omitting `queue_wait_ms`. A real MCP result therefore does not validate against that fixture as checked in. The authoritative MCP output schema is the one the tool itself declares in `tools/temp_artifacts.rs`.

Each unreconciled fixture carries its own in-file warning — a leading docstring in `graphql-sdl.fixture.graphql` and a `$comment` in `mcp-result-schema.fixture.json`, `datetime-nullability-parity.fixture.json`, and `enum-value-projection-matrix.fixture.json` — so a reader who opens one directly is not misled into treating it as parity evidence.

Reconciling these fixtures with the implementation (or changing the implementation to match the approved contract) is required before `operator_visible` promotion: the rollout contract holds on "GraphQL, MCP, run report, or release receipt contracts fail the executable SDL or JSON Schema fixtures". The focused gate asserts fixture **presence** only, not payload conformance, so a passing gate is not parity evidence.

---

## 5. Enum Domains

These are the canonical domains used by the JSON lanes (MCP, run report, release receipt). The seven proposal-era domains support an `"unknown"` fallback. The refinement-added `inventory_mode` domain is currently closed to its three process-start values and has no `unknown` member. Each is projected as a GraphQL enum whose items are the `SCREAMING_SNAKE_CASE` form of the values below; see [§4.4](#44-graphql-projection-specifics-implemented) for the GraphQL type names and fixture gap.

| Enum Type | Allowed Values |
|---|---|
| `inventory_status` | `complete`, `partial`, `timeout`, `cancelled`, `error`, `disabled`, `resource_exhausted`, `unknown` |
| `enabled_state` | `enabled`, `disabled`, `unknown` |
| `inventory_mode` | `disabled`, `hidden_readback`, `operator_visible` |
| `lifecycle_classification` | `active_or_recent`, `terminal_candidate`, `orphan_candidate`, `legacy_unmanaged`, `scan_error`, `unknown` |
| `dry_run_recommendation` | `would_keep_active`, `would_keep_recent`, `would_preserve_failure_evidence`, `would_delete_after_future_approval`, `would_migrate_legacy_manifest_after_future_migration_enabled`, `needs_operator_review`, `no_recommendation`, `unknown` |
| `mutation_guard_status` | `pass`, `fail`, `skipped`, `unknown` |
| `root_kind` | `run_meta_root`, `control_plane_cache`, `provider_home_copy`, `legacy_chainworks_tmp`, `diagnostic_test_root`, `unknown` |
| `error_code` | `invalid_root_override`, `root_unreadable`, `manifest_parse_failed`, `size_estimation_failed`, `deadline_exceeded`, `cancelled`, `internal_error`, `mutation_guard_failed`, `resource_exhausted`, `unknown` |

---

## 6. Path Redaction and HMAC-SHA256 Hash Contract

The system must never leak raw absolute paths to the client, logs, or reports.

### 6.1 Redaction Key
- The implemented key is a 32-byte HMAC key assembled from two UUID v4 byte arrays. It is initialized lazily by the first `compute_path_hash` call and then retained for the daemon process lifetime.
- The key is retained purely in memory and is never serialized, logged, or shared.
- Lazy first-use initialization differs from the approved proposal's daemon-startup initialization contract. Reconcile the implementation and add startup-lifecycle proof before `operator_visible` promotion.

### 6.2 Path Hash Derivation
- **Input**: Native bytes of the normalized absolute path (`OsStrExt::as_bytes()` on Unix) + NUL separator (`\0`) + the UTF-8 `root_kind` string. For valid UTF-8 paths this is byte-for-byte the approved proposal input; preserving native bytes for non-UTF-8 path components is an implementation hardening that prevents distinct paths from collapsing through lossy conversion.
- **Algorithm**: HMAC-SHA256.
- **Output (`path_hash`)**: 64-character lowercase hexadecimal string.
- **Display Hash (`path_hash_short`)**: First 12 characters of `path_hash`. If two display hashes collide in the same payload, they are expanded to the minimum even length needed for uniqueness (up to 20 characters).
- **Stability**: Display hashes are stable only within a single daemon process session. They must not be treated as persistent surrogates.

---

## 7. Request Contract and Diagnostic Test Root Override

### 7.1 Scope and Limits

Request fields are `snake_case` on MCP and `camelCase` on GraphQL; both map onto one validated request contract. Type parsing is strict — a non-boolean `include_dry_run`, non-integer `limit`/`timeout_ms`, or non-string `test_root_override` is rejected rather than coerced:

| Field | Rule |
|---|---|
| `run_id` / `workspace_context` | Exactly one is required in enabled modes. Both present or neither present is rejected. `run_id` is validated for path safety (no separators, traversal, NUL bytes, or overlong values). |
| `workspace_context.workspace_root` | Must be an absolute, canonicalizable, existing directory. The caller-provided root is never scanned itself; only known Chainworks-managed descendants are admitted, so `workspace_context` cannot become an arbitrary filesystem inventory primitive. |
| `limit` | Integer `0`–`500`, default `500`. Out-of-range values are rejected, never clamped. |
| `timeout_ms` | Integer `1`–`5000`, default `5000`. Permit queue wait counts against this total deadline; the post-admission scan deadline is the remaining budget. The deadline is enforced both cooperatively inside the walker and as an outer wall-clock bound on scope resolution and scan dispatch (see [§8.2](#82-operational-constraints)). |
| `include_dry_run` | Default `true`. When `false`, `dry_run` is null and row-level `dry_run_recommendation` is null. |
| `test_root_override` | Diagnostic-only, see [§7.2](#72-diagnostic-test-root-override). |

### 7.2 Diagnostic Test Root Override

To support offline testing and sandboxed verification, the API accepts a `testRootOverride` parameter:
- **Availability**: Requires a non-empty `CHAINWORKS_TEMP_ARTIFACT_DIAGNOSTIC_TEST_ROOTS` allowlist and an `automation` or `developer_break_glass` caller class; any other caller receives `invalid_root_override` rather than a distinguishable authorization error. In `disabled` mode the request short-circuits before override validation, caller-class derivation, or any filesystem probe.
- **Input Rules**: Absolute path only, maximum 4096 bytes, no NUL bytes, no tilde or env expansion, no relative paths, and no traversal components (`..`) after lexical normalization.
- **Containment Check**: Containment is bound to a *single* allowlist entry. `resolve_contained_test_root` first finds the specific allowlisted root the raw path is lexically under, then requires the `realpath`-resolved target to be under **that same** canonicalized root. Containment against "any" allowlist entry is not sufficient, so a path lexically inside allowlist root A that resolves into a different allowlist root B is rejected as an escape.
- **Symlink Containment**: A symlink override is rejected if the symlink or its resolved target escapes the matched allowlist root, including escape into another allowlist entry. Violation results in `invalid_root_override`. When containment passes, the canonicalized path — never the raw input — is handed to the scanner.
- **Unreadable Overrides**: An allowlisted but unreadable root returns a success payload with a status of `partial` or `error` containing the `root_unreadable` code, suppressing raw path exposure.

---

## 8. Scanner Reliability and Resource Bounds

The scanner implements cooperative multitasking and strict resource safeguards. The bound constants named below (`SCAN_*_PERMIT_MAX`, `SCAN_CANCEL_CHECK_INTERVAL_*`, `SCAN_MAX_VISITED_DIRS`, `SCAN_MAX_DIR_DEPTH`, `SCAN_MAX_PATH_BYTES`, `SCAN_PARTIAL_ERRORS_MAX_PER_PAYLOAD`, `SCAN_PARTIAL_ERRORS_MAX_PER_ROW`) are declared in `control-plane/crates/domain/src/temp_artifact_inventory.rs` and enforced by the walker in `control-plane/crates/mcp-server/src/tools/scanner.rs`, which owns `SCAN_MAX_TOTAL_ENTRIES` locally:

### 8.1 ScanPermitGuard
- Restricts concurrent scans using a per-context and global ticket-based permit system: one permit per context key (`SCAN_CONTEXT_PERMIT_MAX = 1`) and four process-wide (`SCAN_GLOBAL_PERMIT_MAX = 4`).
- Acquisition is non-blocking and ordered context-then-global; exhaustion of either semaphore returns `resource_exhausted` immediately instead of queueing.
- The guard is RAII, so both permits release on every terminal path — completion, error, timeout, cancellation, unwind, or connection teardown. Idle context semaphores are evicted so distinct run ids cannot grow the map without bound.
- One release is asynchronous rather than immediate. The permit is moved into the blocking scan task, so when the outer request deadline elapses the caller receives a `timeout` payload while the detached task still holds the permit until its next cooperative cancellation check lets it return. Release is bounded, not instantaneous: a retry issued immediately after a timeout can legitimately observe `resource_exhausted`.

### 8.2 Operational Constraints
- **Multi-tasking**: The walker checks for cancellation and request deadlines at least once every 128 directory entries or 100 ms (`SCAN_CANCEL_CHECK_INTERVAL_ENTRIES` / `SCAN_CANCEL_CHECK_INTERVAL_MS`), and a scan visits at most 100,000 entries in total (`SCAN_MAX_TOTAL_ENTRIES`).
- **Total deadline**: Cooperative checks cannot fire while a single filesystem syscall is blocked — a stalled network mount, for example — so the request deadline is *also* enforced as an outer wall-clock `tokio::time::timeout` around both scope/override resolution and the blocking scan dispatch. Elapse returns `status = timeout` with a `deadline_exceeded` error and an otherwise zeroed payload. Root discovery (`canonicalize` / `is_dir` probes) runs inside the same blocking task as the scan it feeds, so it is covered by that same bound instead of occupying an async runtime worker thread.
- **Symlinks**: Traversal is descriptor-relative. Every directory in the walk stack is an fd opened with `openat(O_NOFOLLOW)` from its parent fd, and children are enumerated with `readdir` and stat'ed with `fstatat`, so mutable path ancestors are never re-resolved mid-walk (SEC-P089-HIGH-001). The scanner **never descends into symlinked directories** (reported as a bounded partial error); symlinked files are reported as link metadata only.
- **Mount Boundaries**: Scans are contained to the mount point of the root. Crossing device boundaries returns a bounded partial error unless explicitly allowlisted in diagnostic mode.
- **Visited Identities**: Visited directories are tracked using device ID + inode to prevent cycle loops. A repeated identity is skipped as a bounded error rather than descended, and the visited set itself is capped at 10,000 directories (`SCAN_MAX_VISITED_DIRS`); once that cap is reached no further directory is descended, so a pathological tree degrades to a bounded partial result instead of an unbounded walk.
- **Depth and Path Budgets**: Descent additionally stops at 128 stack levels (`SCAN_MAX_DIR_DEPTH`, one open directory descriptor per level) or a logical child path longer than 4096 bytes (`SCAN_MAX_PATH_BYTES`). Exceeding either budget emits a bounded `size_estimation_failed` partial error and closes the child descriptor instead of descending. This bounds a deep, singly-nested chain of always-novel directory identities — a shape that never trips the distinct-identity cap above yet would otherwise grow open descriptors and cloned paths without limit. Worst-case concurrent descriptor use is `SCAN_MAX_DIR_DEPTH * SCAN_GLOBAL_PERMIT_MAX`, which the `p089_temp_inventory_scanner_constants_are_sane` unit test pins at or below 4096 so it stays well under typical `RLIMIT_NOFILE`.
- **Error Caps**: Root-level scan errors are capped at 100, and row-level partial errors are capped at 10.
- **Graceful Shutdown**: Daemon shutdown calls `scanner::request_global_shutdown()`, which sets a process-wide flag that in-progress scans observe at their next cancellation check and then return `cancelled` (SEC-P089-007). Cancelled scans release all permits, do not persist any rows, and do not trigger recovery scans at startup.

---

## 9. Advisory Classification and Dry-Run Mapping

Classification is derived from root kind plus last-touched age. It is deliberately conservative: ambiguous evidence lands in a keep/review bucket, never in a delete bucket. No mutation is implied or reachable from any recommendation.

| Condition | `lifecycle_classification` | `dry_run_recommendation` |
|---|---|---|
| `root_kind = legacy_chainworks_tmp` (any age) | `legacy_unmanaged` | `would_migrate_legacy_manifest_after_future_migration_enabled` |
| Last-touched timestamp missing or unparsable | `unknown` | `needs_operator_review` |
| Touched under 1 hour ago | `active_or_recent` | `would_keep_active` |
| Touched under 24 hours ago | `active_or_recent` | `would_keep_recent` |
| Touched under 7 days ago | `terminal_candidate` | `needs_operator_review` |
| Touched 7 days ago or older | `orphan_candidate` | `would_delete_after_future_approval` |

Age thresholds are 1 hour (`AGE_ACTIVE_SECS`), 24 hours (`AGE_RECENT_SECS`), and 7 days (`AGE_TERMINAL_SECS`) in `control-plane/crates/mcp-server/src/tools/scanner.rs`.

A row that accumulated any row-level partial error overrides the age-derived pair after the fact: its `lifecycle_classification` becomes `scan_error` and its `dry_run_recommendation` becomes `needs_operator_review`, so incomplete scan evidence can never present as a delete candidate.

`would_delete_after_future_approval` names a future, not-yet-implemented capability. No deletion path exists in this slice. `would_preserve_failure_evidence` and `no_recommendation` are contract values that this slice's classifier does not yet emit.

---

## 10. Metrics

Metric emitters live in `control-plane/crates/db/src/metrics.rs` as `record_p089_*` helpers. `P089_REQUIRED_METRICS` pins 17 required families — the `p089_temp_inventory_smoke_gate_pass_total{status}` adoption metric plus these 16 operational families:

`temp_artifact_inventory_scan_total`, `temp_artifact_inventory_scan_duration_ms`, `temp_artifact_inventory_rows_total`, `temp_artifact_inventory_estimated_bytes`, `temp_artifact_inventory_scan_rejected_total`, `temp_artifact_inventory_cancel_total`, `temp_artifact_inventory_deadline_exceeded_total`, `temp_artifact_inventory_queue_wait_ms`, `temp_artifact_inventory_permit_reclaimed_total`, `temp_artifact_dry_run_recommendation_total`, `temp_artifact_inventory_scan_error_total`, `temp_artifact_inventory_mutation_guard_total`, `temp_artifact_inventory_readback_parity_total`, `temp_artifact_inventory_redaction_failure_total`, `temp_artifact_inventory_remote_ui_accessibility_total`, `temp_artifact_inventory_metric_health_total`.

Bounded-label policy: labels carry only closed vocabularies (`status`, `root_kind`, `mode`, `reason`, `manifest_state`, `lifecycle_classification`, `artifact_kind`, `recommendation`, `guard_status`, `lane`, `parity_status`, `phase`, `source`, `terminal_status`, `scope`). Raw paths, run/stage/work-item ids, user names, tokens, process ids, and free-form error text are forbidden as label values — per-path correlation belongs in redacted readback, not in metrics. An out-of-vocabulary value is coerced to `fail` for metric-health reporting rather than admitted as a new label.

Every readback lane records `temp_artifact_inventory_readback_parity_total{lane,parity_status}` with `lane` in `mcp`, `graphql`, `run_report`, `release_receipt`. `parity_status` is derived from the shared `dto_redaction_is_safe` boundary check in `tools/temp_artifacts.rs`. That check requires array-shaped `rows` and `errors`; validates every row's exact `<redacted:...>` display syntax, 64-character full hash, bounded short-hash prefix, and `correlation_key == path_hash`; and requires every error message to be the exact `<redacted>` literal. A prefix-only `path_display` check is explicitly insufficient and must not be reintroduced — it would admit `"<redacted:ab12> /Users/user/secret/path"`, leaking the real filesystem path behind a passing verdict.

That check is **fail-closed**, not detect-only: a payload containing any unredacted `path_display` is discarded and replaced with an `internal_error` payload (preserving the request's `include_dry_run` shape) before it leaves the daemon, in addition to recording `temp_artifact_inventory_redaction_failure_total{lane}`. No lane forwards a payload that failed the redaction check.

No lane hardcodes its verdict. The `run_report` and `release_receipt` sections derive theirs from the same `dto_redaction_is_safe` check through `record_and_enforce_lane_parity`, and record `parity_status = "fail"` when the preview path errors — a hardcoded `"pass"` would keep reporting success on a lane whose own projection had gone unsafe. The GraphQL resolver in `graphql-server/src/schema.rs` deliberately does *not* re-check or re-record: the installed backend already ran the payload through `enforce_lane_parity_and_redaction("graphql", ..)`, and the former duplicate check both double-counted the metric and never substituted a safe payload on an unsafe verdict.

Counter samples are projection events, not scan counts. `reports.get` computes the inventory DTO once through `inventory_preview_raw`, records the initial `run_report` verdict, and reuses that DTO across report artifacts; each embedded run-report or release-receipt section then records its own lane verdict without rescanning. A single `reports.get` call can therefore emit multiple `run_report` or `release_receipt` samples while performing only one inventory scan, but it does not emit a spurious `graphql` sample.

No lane compares a payload against the checked-in contract fixtures — see [§4.4](#44-graphql-projection-specifics-implemented).

---

## 11. Operator Enablement and Rollback

Enablement is two independent switches — a daemon-side mode and a client-side visibility key — so backend readback and the packaged UI surface can be turned on separately.

To enable backend readback only for automation and tests, set the daemon mode and explicitly keep the independent app visibility preference false:

```bash
CHAINWORKS_TEMP_ARTIFACT_INVENTORY_MODE=hidden_readback   # daemon process env
defaults write com.chainworks.forge TempArtifactDiagnosticsVisible -bool false
```

To additionally show the packaged app surface:

```bash
CHAINWORKS_TEMP_ARTIFACT_INVENTORY_MODE=operator_visible  # daemon process env, then:
defaults write com.chainworks.forge TempArtifactDiagnosticsVisible -bool true
```

Restart the daemon after changing the mode and relaunch the packaged app after changing the visibility key. LaunchServices launches do not inherit shell environment, so the visibility key — not an env var — is the client-side switch.

To roll back (`disable_temp_artifact_inventory_diagnostics`, no data-loss risk — existing temp files and historical release receipts are untouched):

1. Set `CHAINWORKS_TEMP_ARTIFACT_INVENTORY_MODE=disabled` for the daemon process and restart it.
2. `defaults write com.chainworks.forge TempArtifactDiagnosticsVisible -bool false`, then relaunch the packaged app.
3. Confirm the Swift, GraphQL, MCP, run report, and release receipt lanes all report a disabled disposition and that no root scan occurs.

`operator_visible` promotion is still held pending redaction-key initialization reconciliation, contract-fixture reconciliation, packaged remote UI/accessibility smoke, and a refreshed operator-readback fixture (see [§1.1](#11-implementation-status-by-lane-current-slice)).

---

## 12. Packaged App Surface Contract

The operator surface is `TempArtifactInventoryView(runID:)`, embedded in the Run Report (`.reports`) section of `RunsHomeView`. State is owned by the scene-scoped `@MainActor TempArtifactInventoryViewModel` (`acceptedGenerationID`, `inFlightGenerationID`, `selectedRowIdentity`, stale snapshot, refresh task handle, focused copy state); there is no global singleton and no row persistence.

### 12.1 Controls

| Control | Accessibility identifier |
|---|---|
| Refresh Preview | `temp-artifact-refresh-preview` |
| Cancel Refresh | `temp-artifact-cancel-refresh` |
| Copy Redacted Row | `temp-artifact-copy-redacted-row` |

No delete, clean, prune, apply, Reveal/Open in Finder, raw-path copy, root picker, or destructive context-menu action exists on this surface.

### 12.2 Deterministic States

Each state renders under a stable identifier so remote smoke and accessibility assertions are deterministic:

| State | Identifier |
|---|---|
| Visibility key false (surface hidden) | `temp-artifact-inventory-hidden` |
| Visible root / content | `temp-artifact-inventory-root`, `temp-artifact-inventory-content` |
| First load, no preview yet | `temp-artifact-first-load`, `temp-artifact-capability-status` |
| Loading without prior result | `temp-artifact-loading` |
| Loading over a retained accepted result | `temp-artifact-stale-badge` (icon plus text, never color alone) |
| Complete with rows | `temp-artifact-summary-counters`, `temp-artifact-table`, `temp-artifact-selected-row-inspector` |
| Complete and empty | `temp-artifact-empty-result` |
| Partial / timeout / cancelled / error | `temp-artifact-banner-stack` |
| Disabled mode | `temp-artifact-disabled` |
| `resource_exhausted` backpressure | `temp-artifact-busy` |

Layout adapts through `ViewThatFits`: at ≥900 pt the summary is a single divided metric strip and the selected-row inspector is a 320 pt side region; narrower widths fall back to a two-column and then one-column metric grid with the inspector stacked below the table. The banner stack keeps every error reachable in a scroll region whose visible height is capped at three 48 pt banners / 144 pt, so rows stay in the first viewport.

### 12.3 Copy and Command Routing

`TempArtifactInventoryCommands` inserts **Copy Redacted Row** before the standard pasteboard command group with Command-C. Enablement and invocation flow through the focused scene values `TempArtifactInventoryCopyCommandState.canCopy` and `TempArtifactInventoryCopyCommandActions.copyRedactedRow`, so a scene without a focused selection, a hidden surface, or a loading-without-prior-result state disables the command. A selected stale row remains copyable.

`TempArtifactContextMenuTargeting.targetID(contextSelection:keyboardSelection:)` prefers the right-clicked row and falls back to the keyboard selection; opening a context menu on an unselected row never mutates the keyboard table selection.

Every copy path routes through `TempArtifactRowPasteboardWriter`, which calls `NSPasteboard.general.clearContents()` and writes exactly one `.string` value containing `path_display`, `path_hash`, `lifecycle_classification`, `generated_at`, `stale`, plus `source_generated_at` when the row is stale and `dry_run_recommendation` when present. Raw absolute paths never reach the pasteboard because the client never receives them.

### 12.4 Accessibility Announcements

Terminal-state announcements post through `NSAccessibility.post(.announcementRequested)` behind the injectable `TempArtifactAccessibilityAnnouncing` protocol. They are emitted at most once per accepted generation (`lastTerminalAnnouncementGenerationID`) and only while the scene is both visible and focused (`setSceneActivity(isVisible:isFocused:)`), so hidden or unfocused scenes and superseded generations stay silent.
