# Proposal 044 Readiness Evidence Pack

Review mode: `proposal-readiness`  
Proposal: `docs/proposals/044-idea-crud-completeness-and-lifecycle-mcp-tools.md`  
Reviewed on: 2026-04-17 (Asia/Nicosia)  
Repository HEAD: `bf06b30f4a6c439dc046410756b9d18a972b25b2`  
Runtime evidence: none; this mode uses proposal/docs/code/baseline evidence.  
Worktree note: the repository was dirty before this review. User changes were treated as current repo reality and were not reverted.

## A. Repo-Local Proposal / Document Inventory
| Evidence ID | Source / Path / Artifact | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---:|---|---|---|---|
| DOC-01 | `docs/proposals/044-idea-crud-completeness-and-lifecycle-mcp-tools.md:1` | 2026-04-17 | High | P044 R2 proposes MCP/GraphQL idea get, update, archive, unarchive, duplicate, enhanced list, capability IDs, command journaling, and `proposal-044-ideas` gate. | Review would judge the wrong scope. | Primary proposal. |
| DOC-02 | `docs/proposals/044-idea-crud-completeness-and-lifecycle-mcp-tools.md:38` | 2026-04-17 | High | Product questions require active-run archive rejection, unarchive to Active, duplicate as Draft, list filters, GraphQL parity, and non-empty title/body for Draft-to-Active. | Missing a product invariant would hide a blocker. | Product behavior. |
| DOC-03 | `docs/proposals/044-idea-crud-completeness-and-lifecycle-mcp-tools.md:475` | 2026-04-17 | High | Status matrix says all transitions are governed by P044 and lists Draft->Active only via `ideas.update`, not `runs.start`. | Existing run-start activation path may bypass the proposal's guards. | Lifecycle correctness. |
| DOC-04 | `docs/proposals/044-idea-crud-completeness-and-lifecycle-mcp-tools.md:520` | 2026-04-17 | High | P044 requires journaled lifecycle writes through `CommandHandler::handle`; read-only get/list do not journal. | Tool/resolver writes could fabricate `journalId` or bypass audit. | Command ownership. |
| DOC-05 | `docs/proposals/044-idea-crud-completeness-and-lifecycle-mcp-tools.md:592` | 2026-04-17 | High | P044 says no DB schema migration is required and `update_status` should change from `COALESCE` to unconditional `archived_at = ?2`. | A current caller may clear `archived_at` unexpectedly. | Persistence migration. |
| DOC-06 | `docs/reference/current-system-baseline.md:43` | 2026-04-17 | High | Current baseline lists `domain-model.md`, `project-workspace-contract.md`, `idea-lifecycle.md`, and test gate docs as canonical references. | A proposal depending only on a stale doc can miss current contract truth. | Baseline source chain. |
| DOC-07 | `.review-baselines/current-system-baseline.md:26` | 2026-04-17 | Medium | Reusable review baseline points reviewers to stable `docs/reference/` docs, including workspace and idea lifecycle. | Review may waste time on old proposal lineage. | Review intake. |
| DOC-08 | `docs/reference/idea-lifecycle.md:25` | 2026-04-17 | High | Archive is reversible visibility state, must preserve run history, and may not happen while a run is active, waiting approval, or live in-flight. | Archive implementation could corrupt operator truth. | Lifecycle invariant. |
| DOC-09 | `docs/reference/project-workspace-contract.md:31` | 2026-04-17 | High | Ideas own explicit workspace roots; run creation freezes workspace truth; later idea workspace edits do not mutate existing runs. | P044's update surface could violate run workspace provenance. | Workspace contract. |
| DOC-10 | `docs/reference/domain-model.md:26` | 2026-04-17 | Medium | Domain-model doc still describes the SwiftData `IdeaStatus` as `draft`, `active`, `completed`, `failed`, which diverges from current Rust `IdeaStatus` and `idea-lifecycle.md`. | Using this as the sole dependency can mislead implementation. | Dependency freshness. |
| DOC-11 | `docs/reference/test-gates.md:501` | 2026-04-17 | High | Existing `proposal-044` gate is already owned by post-approval task execution and release gate completion. | P044 idea gate must not collide with existing gate. | Gate ownership. |
| DOC-12 | `docs/proposals/029-mcp-northbound-control-plane-server.md:154` | 2026-04-17 | High | P029 defines `domain`-owned `CapabilityToolId`, server-side converters, and the closed-enum capability drift contract. | New idea tools may bypass northbound auth policy. | Capability model dependency. |

## B. Reusable Baseline Inputs
| Evidence ID | Artifact / Slice | Status | Covered Surfaces | Verified On | Confidence | Freshness Notes | Relevance |
|---|---|---|---|---:|---|---|---|
| BASE-01 | `.review-baselines/current-system-baseline.md` | Reused | Review posture, stable reference preference, idea/workspace docs | 2026-04-17 | Medium | Baseline still has some broad provider wording, but the P044 idea/workspace reference slice is usable. | Review setup. |
| BASE-02 | `docs/reference/current-system-baseline.md` | Reused | Canonical docs, implemented idea archive/restore flow, workspace boundary, test gates | 2026-04-17 | High | Current reference baseline is the better source for P044 than archived proposals. | Source-of-truth chain. |
| BASE-03 | Targeted code refresh | Partially refreshed | Rust idea CRUD, run start, command journal, GraphQL, MCP, auth, test gates | 2026-04-17 | High | Targeted refresh was required because P044 expands Rust northbound surfaces not fully described by the baseline. | Feasibility and contradictions. |

## C. Scope, Out-of-Scope, and Intentional Deferrals
- In scope: MCP `ideas.get/update/archive/unarchive/duplicate`, enhanced `ideas.list`, GraphQL parity mutations/query filters, `CommandHandler` ownership for lifecycle writes, command-journal IDs, capability IDs, auth class policy, list filters, and distinct `proposal-044-ideas` gate.
- Out of scope: Swift UI changes, deletion, bulk operations, external auth mechanisms, runtime execution changes beyond lifecycle guard interaction with `runs.start`.
- Deferred intentionally: keyset pagination and YAML-driven per-principal capability policy.
- Assumptions: existing user changes in the dirty tree are current repo reality; no runtime build/run is required for proposal-readiness mode.
- Open questions: should Draft->Active be allowed through `runs.start`, or must run start require prior activation by `ideas.update`?
- Blockers: P044 must specify how `runs.start` participates in idea lifecycle invariants before implementation starts.

## D. Affected Runtime / Entry-Point / Protocol Slice
| Evidence ID | Surface / Entry Point / Runtime Boundary | Source | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---:|---|---|---|---|
| SURF-01 | MCP `ideas.*` tools | Current repo | 2026-04-17 | High | Current `ideas.rs` registers only `ideas.list` and `ideas.create`; `ideas.create` writes directly to DB. | New tools could be incompletely registered or bypass journal. | MCP scope. |
| SURF-02 | MCP `runs.start` | Current repo | 2026-04-17 | High | `runs.start` builds `StartRunCmd` and routes through `CommandHandler::handle`. | Idea lifecycle may be changed through run start, not just idea tools. | Lifecycle seam. |
| SURF-03 | GraphQL `idea/ideas` and mutations | Current repo | 2026-04-17 | High | GraphQL has `idea` and `ideas` reads and command-backed run/stage mutations; no idea lifecycle mutations yet. | Parity work must extend mutation auth and payloads. | GraphQL scope. |
| SURF-04 | `CommandHandler::handle` | Current repo | 2026-04-17 | High | Commands are journaled before execution and completed/failed afterward. | Journaled writes must be added here, not direct DB writes. | Audit truth. |
| SURF-05 | SQLite repositories | Current repo | 2026-04-17 | High | `ideas` repo has insert/find/list/update_status only; `runs` repo has `list_by_idea` and insert/update functions. | Guard and write atomicity depend on repo shape. | Persistence. |
| SURF-06 | Auth capability converters | Current repo/P029 | 2026-04-17 | High | Tool authorization uses `CapabilityToolId` variants and server-owned converters. | Missing variants can hide or deny tools incorrectly. | Security boundary. |

## E. Impacted Crates / Modules / Code-Path Map
| Evidence ID | File Path / Crate / Module / Symbol | Layer | Role in Flow | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---:|---|---|---|---|
| MAP-01 | `control-plane/crates/domain/src/idea.rs:6` | domain | Idea model | 2026-04-17 | High | Rust `IdeaStatus` is `Draft`, `Active`, `Archived`; `Idea` carries workspace root, project key, status, and `archived_at`. | Proposal may follow stale Swift doc instead. | Model truth. |
| MAP-02 | `control-plane/crates/domain/src/commands.rs:25` | domain | Command enum | 2026-04-17 | High | Current `Command` has no idea lifecycle variants. | P044 must add them and update all command matches. | Command ownership. |
| MAP-03 | `control-plane/crates/db/src/repos/ideas.rs:8` | db | Idea persistence | 2026-04-17 | High | Current functions are `insert`, `find_by_id`, `list`, and `update_status`; `update_status` preserves `archived_at` through `COALESCE`. | P044 needs new update/list/duplicate functions and safe status transitions. | Persistence. |
| MAP-04 | `control-plane/crates/db/src/repos/runs.rs:90` and `control-plane/crates/domain/src/run.rs:77` | db/domain | Active-run guard input | 2026-04-17 | High | `list_by_idea` can enumerate runs; terminal statuses are completed, failed, and cancelled. | Archive guard must use correct terminal definition. | Lifecycle guard. |
| MAP-05 | `control-plane/crates/engine/src/command_handler.rs:152` | engine | Start run | 2026-04-17 | High | `StartRun` fetches the idea, inserts a run, then unconditionally sets idea status to `Active`. | This bypasses P044's Draft/Archived transition matrix. | Critical contradiction. |
| MAP-06 | `control-plane/crates/mcp-server/src/tools/ideas.rs:43` | MCP | Idea tool dispatch | 2026-04-17 | High | `ideas.execute` currently receives a `CommandHandler` but ignores it for idea tools. | Lifecycle writes must stop writing direct DB rows. | MCP implementation. |
| MAP-07 | `control-plane/crates/graphql-server/src/schema.rs:203` | GraphQL | Mutation auth | 2026-04-17 | High | `MutationName` and `capability_id_for` cover start/approval/retry/cancel only. | P044 must add idea mutation capability mapping. | GraphQL auth. |
| MAP-08 | `control-plane/crates/auth/src/lib.rs:217` | auth | Class policy | 2026-04-17 | High | Current all-tool inventory has 13 variants and only `IdeasCreate`/`IdeasList` for ideas. | New tools require explicit class policy. | Security. |
| MAP-09 | `control-plane/crates/mcp-server/src/tools/mod.rs:11` | MCP | Capability registration | 2026-04-17 | High | MCP registers 13 capability IDs and maps `ideas.list` to `IdeasList`. | Tool discovery/call auth must be updated. | Northbound protocol. |
| MAP-10 | `scripts/test-gate.sh:1511` | scripts | Proof gate | 2026-04-17 | High | `proposal-044|p044` currently runs the post-approval/release control-plane gate. | P044 must add a distinct gate alias. | Verification. |
| MAP-11 | `control-plane/crates/engine/src/command_journal_redact.rs:8` | engine | Redaction | 2026-04-17 | High | Redaction currently covers only approval/rejection comments. | New free-text/path idea commands require redaction. | Security/audit. |
| MAP-12 | `control-plane/crates/auth/src/lib.rs:50` | auth | Principal table | 2026-04-17 | Medium | Principal table persists token/id/class only; capability sets are derived when `Principal` is constructed. | Capability rename is mostly code/config compatibility, not DB migration. | Migration nuance. |

## F. Data / Protocol / Persistence / Auth Touchpoints
| Evidence ID | Touchpoint | File / Module / Doc | Direction | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---:|---|---|---|---|
| DATA-01 | MCP JSON schemas | P044 sections 5.1-5.6 | client -> server | 2026-04-17 | High | P044 defines JSON inputs for get/update/archive/unarchive/duplicate/list filters. | Agent clients could have inconsistent contracts. | Protocol. |
| DATA-02 | GraphQL schema | P044 sections 5.2-5.6 and `graphql-server/src/schema.rs` | GraphQL client -> server | 2026-04-17 | High | P044 correctly calls out `MaybeUndefined` for patch semantics; current schema lacks idea mutations. | `Option<String>` would lose null-vs-omitted truth. | Protocol. |
| DATA-03 | Ideas table writes | `db/src/repos/ideas.rs` | server -> SQLite | 2026-04-17 | High | No schema change needed, but `update_status` behavior changes. | Existing `StartRun` caller can clear archived state if unchanged. | Persistence. |
| DATA-04 | Command journal | `engine/src/command_handler.rs` | command -> audit DB | 2026-04-17 | High | Journal record is inserted before execution and failed rows are marked on errors. | Lifecycle failures should leave audit evidence. | Audit/recovery. |
| DATA-05 | Auth capabilities | `domain/src/capabilities.rs`, `auth/src/lib.rs`, `mcp-server/src/tools/mod.rs`, P029 | principal -> tool/mutation | 2026-04-17 | High | New tools need enum variants, class policy, MCP converters, and GraphQL mutation converters. | Unauthorized access or hidden tools. | Security. |
| DATA-06 | Workspace root edits | `project-workspace-contract.md` | idea -> run freeze | 2026-04-17 | High | Later idea workspace edits must not mutate existing runs. | P044 update tests must protect run provenance. | Workspace safety. |

## G. Current Host-System Integration Surfaces
| Evidence ID | Surface / Seam / Owner | Source | Verified On | Confidence | Key Fact | Conflict / Proposal Risk | Relevance |
|---|---|---|---:|---|---|---|---|
| INT-01 | `runs.start` owns a status transition | Current repo | 2026-04-17 | High | StartRun currently activates the idea after inserting a run. | P044 transition matrix omits this path. | Critical blocker. |
| INT-02 | Archive guard vs run creation | Proposal + current repo | 2026-04-17 | High | Proposal checks existing runs before archive, while StartRun inserts runs independently. | Separate check/write can race. | Reliability. |
| INT-03 | Idea workspace root vs frozen run workspace | Baseline + proposal | 2026-04-17 | High | P044 updates `workspace_root_path`; baseline says existing runs keep frozen workspace. | Missing tests can regress run provenance. | Data integrity. |
| INT-04 | P029 capability model | Baseline + current repo | 2026-04-17 | High | New idea tools cross existing MCP/GraphQL auth boundary. | Capability inventory drift can hide or overexpose tools. | Security. |
| INT-05 | Gate alias ownership | Current repo + proposal | 2026-04-17 | High | Existing `proposal-044` gate is occupied; P044 proposes `proposal-044-ideas`. | Collision would break prior proof lane. | Verification. |

## H. State and Failure Coverage Matrix
| State | Proposal Status | Evidence IDs | Repo Touchpoints | Notes / Risks |
|---|---|---|---|---|
| Entry | Specified | DOC-01, SURF-01, SURF-03 | MCP/GraphQL tool entry points | Tool names and payload shapes are specified. |
| Happy path | Specified | DOC-01, DOC-04 | command handler, repos | Get/update/archive/unarchive/duplicate/list paths are covered. |
| Loading / inflight | Partial | DOC-08, INT-02 | runs repo, command handler | Active-run archive guard exists conceptually, but atomicity is not specified. |
| Timeout | Deferred intentionally | DOC-01 | none | No new remote runtime calls; not proposal-critical. |
| Validation error | Contradicted by repo | DOC-02, DOC-03, MAP-05 | StartRun | Empty Draft and Archived start paths are not covered by P044's matrix. |
| Dependency error | Partial | DOC-09, DATA-06 | workflow compile, workspace preflight | Workspace edit/run-freeze interaction needs explicit test coverage. |
| Retry / replay | Deferred intentionally | DOC-01 | none | Idea CRUD has no retry model beyond idempotent archive/unarchive. |
| Cancellation / shutdown | Partial | DOC-08 | runs.cancel, archive guard | Proposal says cancel active runs first but does not bind archive guard to cancellation settlement races. |
| Overload / backpressure | Partial | DOC-01 | list filters | Limit cap is specified; no rate limiting beyond existing auth. |
| Degraded / offline | Deferred intentionally | DOC-01 | none | Local SQLite/control-plane only. |
| Auth / permission failure | Specified | DOC-12, DATA-05 | auth, MCP, GraphQL | Capability IDs and class policy are specified. |
| Rollback / migration failure | Partial | DOC-05, MAP-05 | `update_status` | No DB migration, but behavior change misses current StartRun caller. |

## I. Feature Flags / Rollout / Migration / Rollback
| Evidence ID | Mechanism / Flag | Scope | Rollout Plan | Rollback Path | Verified On | Confidence | Notes |
|---|---|---|---|---|---:|---|---|
| FLAG-01 | No feature flag | New MCP/GraphQL API | Additive API plus capability inventory changes | Revert proposal slice | 2026-04-17 | Medium | Acceptable if lifecycle blockers are fixed before implementation. |
| FLAG-02 | No DB migration | Existing `ideas` columns | Repo function additions only | Revert code | 2026-04-17 | High | `update_status` behavior change needs caller-specific safety. |
| FLAG-03 | Distinct test gate | `proposal-044-ideas|p044-ideas` | Add new alias and preserve existing P044 gate | Remove new alias | 2026-04-17 | High | Proposal correctly avoids gate collision. |

## J. Telemetry / Instrumentation
| Evidence ID | Event / Signal | Purpose | Trigger Point | Verified On | Confidence | Gap / Risk |
|---|---|---|---|---:|---|---|
| METRIC-01 | `command_journal` rows | Audit lifecycle writes and failures | `CommandHandler::handle` | 2026-04-17 | High | Good audit signal; no separate metrics proposed. |
| METRIC-02 | Error messages for archive/start rejection | Operator/agent guidance | archive and start-run guards | 2026-04-17 | Medium | P044 specifies archive error text, but not start-run lifecycle rejection text. |
| METRIC-03 | Tracing/logging | Operational diagnosis | command handler/repo writes | 2026-04-17 | Low | Proposal does not specify lifecycle-specific tracing; not blocking but useful. |

## K. Testing Strategy
| Evidence ID | Layer | Covered Surface | Current Coverage | Proposed Additions | Verified On | Confidence | Gap / Risk |
|---|---|---|---|---|---:|---|---|
| TEST-01 | Unit/integration | MCP idea CRUD | Current MCP tests cover existing tools only | P044 adds tool behavior, journal ID, list filtering | 2026-04-17 | High | Add tests in `mcp-server`. |
| TEST-02 | Unit/integration | GraphQL idea mutations | Current mutation tests cover run/stage commands | P044 adds idea mutations and patch semantics | 2026-04-17 | High | Add `MaybeUndefined` tests. |
| TEST-03 | Unit/integration | Command journal and redaction | Redaction only covers approval/rejection comments | P044 adds idea command journal and redaction tests | 2026-04-17 | High | Required for audit truth. |
| TEST-04 | Unit/integration | Lifecycle invariants | Proposal covers archive, unarchive, update | Missing StartRun archived/Draft-empty/duplicate-start cases | 2026-04-17 | High | Critical missing tests. |
| TEST-05 | Integration/race | Archive vs start-run | None specified | Need atomic guard proof around archive and run creation | 2026-04-17 | Medium | SQLite transaction behavior should be tested. |
| TEST-06 | Gate | Canonical proof lane | Existing `proposal-044` occupied | Add `proposal-044-ideas|p044-ideas` | 2026-04-17 | High | Proposal correctly names distinct gate. |

## L. Current Repo Reality / Contradictions
| Evidence ID | Repo Surface | Proposal Claim | Current Repo Reality | Verified On | Confidence | Implication |
|---|---|---|---|---:|---|---|
| REAL-01 | `CommandHandler::StartRun` | All status transitions are governed by P044 matrix; Draft->Active only via `ideas.update`. | StartRun currently inserts a run and then sets idea status `Active` unconditionally. | 2026-04-17 | High | Archived ideas and empty Draft ideas can bypass lifecycle guards unless StartRun is included. |
| REAL-02 | `ideas::update_status` | Changing to unconditional `archived_at = ?2` is backward-compatible because only archive/unarchive callers matter. | Current caller list includes `StartRun`, which passes `Active`. | 2026-04-17 | High | StartRun on archived idea could clear `archived_at` after P044's update. |
| REAL-03 | Dependency row | P044 depends only on `domain-model.md`. | Current baseline points to `idea-lifecycle.md` and `project-workspace-contract.md`; `domain-model.md` is stale for `IdeaStatus`. | 2026-04-17 | High | Implementation could follow wrong status/workspace truth. |
| REAL-04 | Duplicate risk | Duplicate creates inert Draft ideas that cannot have runs started until Active. | Current StartRun can activate any idea it finds, including Draft. | 2026-04-17 | High | Duplicate "inert" mitigation is false unless StartRun blocks or validates Draft. |
| REAL-05 | Gate state | P044 must add `proposal-044-ideas`. | Current script/docs only have the occupied `proposal-044` gate. | 2026-04-17 | High | This is planned work, not a contradiction, but must be implemented exactly. |

## M. Proposal Completeness Matrix
| Dimension | Status | Evidence IDs | Notes |
|---|---|---|---|
| Problem / user intent | Specified | DOC-01, DOC-02 | Clear user and agent workflows. |
| Scope boundaries | Specified | DOC-01 | UI/delete/bulk/auth classes excluded. |
| Reusable baseline coverage | Partial | DOC-06, DOC-08, DOC-09, REAL-03 | Proposal cites stale/incomplete dependency chain. |
| Runtime / entry-point definition | Partial | SURF-01, SURF-02, SURF-03, REAL-01 | Idea endpoints are defined; StartRun lifecycle seam missing. |
| State and failure handling | Contradicted by repo | DOC-03, REAL-01, REAL-02, REAL-04 | Transition matrix omits existing run-start activation. |
| Data / protocol contract | Specified | DATA-01, DATA-02 | Patch semantics are well specified. |
| Persistence / caching | Partial | DOC-05, MAP-03, REAL-02 | No schema migration, but `update_status` caller interaction unresolved. |
| Async/runtime assumptions | Partial | INT-02 | Archive guard atomicity not specified. |
| Permissions / auth expiry | Specified | DATA-05, DOC-12 | Class policy and converters are specified; no token lifecycle change. |
| Feature flags / rollout / rollback | Partial | FLAG-01, FLAG-02, FLAG-03 | No flag; acceptable for additive API after blockers. |
| Telemetry / instrumentation | Partial | METRIC-01, METRIC-02 | Command journal covered; start-run lifecycle rejection signal missing. |
| Testing / perf validation strategy | Partial | TEST-01..TEST-06 | Strong tests, but missing StartRun and atomicity cases. |
| Dependencies / integration points | Partial | REAL-03 | Needs canonical lifecycle/workspace/northbound deps. |
| Security / trust boundaries | Specified | DATA-05, MAP-08, MAP-09 | Capability IDs and class policies are explicit. |

## N. Assumptions, Open Questions, and Blockers
- ASSUMP-01: The dirty tree is the intended current review target.
- ASSUMP-02: Runtime/build verification is not required for `proposal-readiness` mode.
- QUESTION-01: Should `runs.start` be allowed to perform Draft->Active, or must it reject Draft ideas until `ideas.update` has activated them?
- QUESTION-02: Should `update_status` remain a generic helper, or should P044 split it into explicit `archive_idea`, `unarchive_idea`, and `mark_active_on_start` helpers to prevent accidental `archived_at` clearing?
- QUESTION-03: Should `IdeasList -> IdeasRead` include serde alias/backward-compatibility tests for any serialized `Principal` or external config using the enum name?
- BLOCKER-01: P044 must cover the existing `runs.start` lifecycle transition path.
- BLOCKER-02: P044 must make archive eligibility and status update atomic with respect to concurrent run creation.
- BLOCKER-03: P044 must update its dependency row to include canonical lifecycle/workspace/northbound references, or explicitly state why the stale SwiftData domain-model status vocabulary is not controlling.

## O. Research Triggers / External Questions
No external research was requested or needed. Local proposal/docs/code evidence was sufficient for a defensible readiness review.
