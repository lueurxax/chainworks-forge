# Proposal 044: Idea CRUD Completeness and Lifecycle MCP Tools

| Field | Value |
|---|---|
| Date | 2026-04-17 (revised R3) |
| Status | Draft (R3 - readiness blockers addressed: StartRun lifecycle ownership, atomic archive/start invariant, command/journal ownership, explicit capabilities, GraphQL patch semantics, distinct gate alias) |
| Author | Andrey Khasanov |
| Depends on | [../reference/idea-lifecycle.md](../reference/idea-lifecycle.md), [../reference/project-workspace-contract.md](../reference/project-workspace-contract.md), [../reference/current-system-baseline.md](../reference/current-system-baseline.md), [../reference/test-gates.md](../reference/test-gates.md), [../reference/mcp-northbound-control-plane-server.md](../reference/mcp-northbound-control-plane-server.md), [../reference/domain-model.md](../reference/domain-model.md) |
| Scope | Add get, update, archive, unarchive, and duplicate MCP tools for ideas, plus enhanced list filtering, with matching GraphQL mutations and server-side guard rails. |
| Goal | Give MCP-connected agents and operators full idea lifecycle management with patch-semantic updates and safety guards that exceed the Swift app's current capabilities. |

**Gate naming note:** the repository already owns `proposal-044|p044` for the post-approval task execution and release gate completion proof lane. This proposal must use the distinct canonical gate alias `proposal-044-ideas|p044-ideas`; it must not replace or repurpose the existing `proposal-044|p044` gate.

---

## 1. Context and Motivation

The control-plane exposes only two idea-related MCP tools today:

| Tool | Capability |
|------|-----------|
| `ideas.create` | Create a new idea (title, body, workspace_root_path, project_key) |
| `ideas.list` | List ideas with an optional `include_archived` flag |

The Swift macOS app has a richer idea surface: editing title/body/workspace_root_path, archiving/unarchiving, and a detail view for individual ideas. But these operations go through SwiftUI views and local persistence, not through the control-plane's MCP server. An MCP-connected agent that wants to retrieve a specific idea, update its body, or archive it after a run completes has no tool to do so.

The DB repo layer already has `find_by_id`, `insert`, `list`, and `update_status`, and the GraphQL schema already exposes an `idea(id)` query. But there is no `update` repo function, no archive/unarchive MCP tool, and no GraphQL mutation for idea writes beyond `startRun`.

This gap matters because:

1. **Agents cannot close the loop.** A steward agent that analyzes run quality and wants to archive a stale idea must ask the operator to do it manually.
2. **Partial updates are impossible.** The Swift app always writes the full `Idea` model. An agent that only wants to change the `project_key` must know and resend every field.
3. **No server-side safety.** The Swift app shows a warning when archiving an idea with active runs, but does not enforce it. An operator can archive an idea while a run is in `running` or `waiting_approval` state, causing confusion in the Runs tab.
4. **No duplication.** When an operator wants to run a similar workflow with slightly different parameters, they must manually create a new idea and copy fields from the original.

---

## 2. Product Questions This Proposal Must Answer

After implementation, the system must be able to answer:

1. Can an MCP-connected agent retrieve a single idea by ID without listing all ideas?
2. Can an agent update one field on an idea (e.g., only `project_key`) without resending the entire model?
3. Can an agent archive an idea, and does the server reject the archive if any run is in a non-terminal state?
4. Can an agent unarchive a previously archived idea, restoring it to Active status?
5. Can an agent duplicate an idea, producing a new Draft idea with copied fields and a fresh ID?
6. Can an agent list ideas filtered by status, by project_key, and with pagination?
7. Does the Draft-to-Active transition require non-empty title and body?
8. Are the same operations available via GraphQL for the Swift app and other clients?

---

## 3. Scope

This proposal includes:

- Five new MCP tools: `ideas.get`, `ideas.update`, `ideas.archive`, `ideas.unarchive`, `ideas.duplicate`.
- Enhanced `ideas.list` with status filtering, project_key filtering, and offset/limit pagination.
- Server-side guard: archive rejects ideas with non-terminal runs.
- Server-side guard: status transition Draft-to-Active requires non-empty title and body.
- New DB repo functions: `update_fields`, `duplicate`, `list_filtered`.
- New domain command variants and `CommandHandler` execution paths for journaled idea lifecycle writes.
- `StartRun` lifecycle guard integration so runs can start only from eligible ideas and cannot accidentally clear archive metadata.
- Atomic archive/start invariant: archiving an idea and starting a run for that idea cannot race into "archived with active run" truth.
- Explicit northbound capability IDs and principal-class policy for idea read/write/archive/duplicate operations.
- New GraphQL mutations: `updateIdea`, `archiveIdea`, `unarchiveIdea`, `duplicateIdea`.
- Updated GraphQL `ideas` query with filter/pagination arguments.
- Distinct proof gate `proposal-044-ideas|p044-ideas` because `proposal-044|p044` is already owned by another proof lane.

This proposal does **not** include:

- Changes to the Swift app UI (the app can adopt the new GraphQL mutations separately).
- Idea deletion (ideas are archived, not deleted; deletion is out of scope).
- Bulk operations (archive-all, update-many).
- Changes to `Run`, `Artifact`, or their persistence schema. `StartRun` behavior changes only at the command guard boundary.
- New principal classes or external auth mechanisms. This proposal does change the existing exhaustive capability map by adding explicit idea lifecycle capability IDs for the new tools.

---

## 4. Problem Statement

### 4.1 No way to retrieve a single idea by ID via MCP

The GraphQL schema has `idea(id: ID!): Idea`, but the MCP server has no equivalent. An agent that received an idea ID from a `startRun` response or from a steward analysis must call `ideas.list` and scan the result array client-side to find the matching idea.

### 4.2 No update capability at all

Neither MCP nor GraphQL exposes an idea update operation. The Swift app writes directly via its local persistence layer. The DB repo has `update_status` but no general field update. An agent or external tool cannot change an idea's title, body, workspace_root_path, or project_key through the control-plane.

### 4.3 Archive/unarchive is not exposed via MCP

The DB repo has `update_status` which can set `IdeaStatus::Archived` (and records `archived_at`), but this is not wired to any MCP tool. The Swift app calls the repo directly from the view layer. There is no server-side validation preventing archival of ideas with active runs.

### 4.4 No duplication

Creating a variation of an existing idea requires manual field-by-field copying. For operators iterating on workflow parameters (different workspace_root_path, adjusted body), this is tedious.

### 4.5 List filtering is limited

`ideas.list` accepts only `include_archived: bool`. There is no way to filter by status (e.g., only Draft ideas), by project_key, or to paginate results. For workspaces with dozens of ideas, the agent must download all of them and filter client-side.

---

## 5. Core Product Behavior

### 5.1 `ideas.get` -- retrieve a single idea by ID

**MCP tool spec:**

```json
{
  "name": "ideas.get",
  "description": "Get a single idea by its ID",
  "input_schema": {
    "type": "object",
    "required": ["id"],
    "properties": {
      "id": {
        "type": "string",
        "format": "uuid",
        "description": "The idea ID"
      }
    }
  }
}
```

**Behavior:**

1. Parse `id` as UUID. Return error if malformed.
2. Call `ideas::find_by_id(pool, id)`.
3. If `None`, return MCP error: `"Idea not found: {id}"`.
4. Return the `Idea` as JSON.

**GraphQL:** Already exists as `idea(id: ID!): Idea`. No change needed.

### 5.2 `ideas.update` -- partial update (patch semantics)

**MCP tool spec:**

```json
{
  "name": "ideas.update",
  "description": "Update an idea. Only provided fields are changed (patch semantics). Cannot update status directly -- use ideas.archive / ideas.unarchive for status transitions.",
  "input_schema": {
    "type": "object",
    "required": ["id"],
    "properties": {
      "id": {
        "type": "string",
        "format": "uuid",
        "description": "The idea ID to update"
      },
      "title": {
        "type": "string",
        "description": "New title (omit to keep current)"
      },
      "body": {
        "type": "string",
        "description": "New body (omit to keep current)"
      },
      "workspace_root_path": {
        "type": ["string", "null"],
        "description": "New workspace root path. Pass null to clear, omit to keep current."
      },
      "project_key": {
        "type": ["string", "null"],
        "description": "New project cohort key. Pass null to clear, omit to keep current."
      }
    }
  }
}
```

**Behavior:**

1. Fetch the existing idea by `id`. Return error if not found.
2. Reject if idea status is `Archived`: `"Cannot update an archived idea. Unarchive it first."`.
3. Apply patch: for each optional field present in the request, overwrite the existing value. Fields absent from the request are left unchanged. Fields explicitly set to `null` clear the value (for nullable fields only: `workspace_root_path`, `project_key`).
4. **Draft-to-Active validation:** If the idea is in `Draft` status, check whether the resulting title and body are both non-empty. If so, automatically transition the idea to `Active` status. This prevents ideas with empty fields from reaching Active and failing at run-start preflight.
5. Updating `workspace_root_path` or `project_key` changes the idea row for future use only. It must not rewrite existing run records, run workspace roots, release artifact paths, or per-run metadata covered by `project-workspace-contract.md`.
6. Persist via new repo function `ideas::update_fields`.
7. Return `{ "idea": updated_idea, "journal_id": commanded.journal_id }` for MCP and `UpdateIdeaPayload { idea, journalId }` for GraphQL.

**New DB repo function:**

```rust
pub async fn update_fields(
    pool: &SqlitePool,
    id: IdeaId,
    title: Option<&str>,
    body: Option<&str>,
    workspace_root_path: Option<Option<&str>>,  // None = no change, Some(None) = clear, Some(Some(v)) = set
    project_key: Option<Option<&str>>,           // same semantics
    status: Option<IdeaStatus>,
) -> Result<()>
```

Uses a dynamic SQL builder: only columns with provided values appear in the `SET` clause.

**GraphQL mutation:**

```graphql
input UpdateIdeaInput {
  title: String
  body: String
  workspaceRootPath: String
  projectKey: String
}

type UpdateIdeaPayload {
  idea: Idea!
  journalId: ID!
}

type Mutation {
  updateIdea(id: ID!, input: UpdateIdeaInput!): UpdateIdeaPayload!
}
```

**GraphQL patch semantics:**

The Rust resolver must not model `UpdateIdeaInput` fields as plain `Option<String>`, because async-graphql collapses omitted fields and explicit `null` into `None`. Use `async_graphql::MaybeUndefined<String>` for each patch field:

```rust
#[derive(InputObject)]
struct UpdateIdeaInput {
    title: MaybeUndefined<String>,
    body: MaybeUndefined<String>,
    workspace_root_path: MaybeUndefined<String>,
    project_key: MaybeUndefined<String>,
}
```

Resolver behavior:

- `MaybeUndefined::Undefined` means "no change".
- `MaybeUndefined::Value(v)` means "set to `v`".
- `MaybeUndefined::Null` clears nullable fields (`workspaceRootPath`, `projectKey`).
- `MaybeUndefined::Null` is rejected for non-null logical fields (`title`, `body`) with a validation error.

This preserves JSON/MCP merge-patch semantics and makes GraphQL clearing executable.

### 5.3 `ideas.archive` -- archive with active-run guard

**MCP tool spec:**

```json
{
  "name": "ideas.archive",
  "description": "Archive an idea. Fails if the idea has any runs in a non-terminal state (pending, ready, running, waiting_approval, blocked, cancelling).",
  "input_schema": {
    "type": "object",
    "required": ["id"],
    "properties": {
      "id": {
        "type": "string",
        "format": "uuid",
        "description": "The idea ID to archive"
      }
    }
  }
}
```

**Behavior:**

1. Fetch the idea. Return error if not found.
2. If already `Archived`, return the idea as-is (idempotent).
3. **Active-run guard:** Check whether any run has a non-terminal status (i.e., `!run.status.is_terminal()`). If any non-terminal run exists, return error:
   ```
   "Cannot archive idea {id}: {n} run(s) are still active
   (statuses: {comma-separated statuses}). Cancel or wait for
   them to complete before archiving."
   ```
4. The guard and the archive write must be one atomic DB operation. Implement this as either a single conditional `UPDATE ... WHERE NOT EXISTS (...)` or a SQLite transaction that takes a write lock before checking active runs. A check-then-update sequence outside a transaction is not acceptable because it can race `StartRun`.
5. Set status to `Archived` and `archived_at` to `Utc::now()` via an archive-specific repo helper.
6. Return `{ "idea": updated_idea, "journal_id": commanded.journal_id }` for MCP and `ArchiveIdeaPayload { idea, journalId }` for GraphQL.

**GraphQL mutation:**

```graphql
type ArchiveIdeaPayload {
  idea: Idea!
  journalId: ID!
}

type Mutation {
  archiveIdea(id: ID!): ArchiveIdeaPayload!
}
```

### 5.4 `ideas.unarchive` -- restore to Active

**MCP tool spec:**

```json
{
  "name": "ideas.unarchive",
  "description": "Restore an archived idea to Active status. Clears archived_at.",
  "input_schema": {
    "type": "object",
    "required": ["id"],
    "properties": {
      "id": {
        "type": "string",
        "format": "uuid",
        "description": "The idea ID to unarchive"
      }
    }
  }
}
```

**Behavior:**

1. Fetch the idea. Return error if not found.
2. If not `Archived`, return the idea as-is (idempotent).
3. Set status to `Active` and clear `archived_at` to `None` through an unarchive-specific repo helper. This is the only P044 path allowed to clear `archived_at`.
4. Return `{ "idea": updated_idea, "journal_id": commanded.journal_id }` for MCP and `UnarchiveIdeaPayload { idea, journalId }` for GraphQL.

**DB change:** do not make generic `update_status(..., Active)` clear `archived_at`. Current `StartRun` already calls `update_status(..., Active)`, so changing the generic helper to unconditional `archived_at = NULL` would let run start accidentally erase archive history. Split status writes into lifecycle-specific helpers instead:

```rust
archive_if_no_active_runs(pool, idea_id, now)        // sets status=Archived, archived_at=now atomically
unarchive(pool, idea_id)                             // sets status=Active, archived_at=NULL
mark_active_from_valid_update(pool, idea_id)         // sets status=Active and preserves archived_at
```

The existing generic `update_status` may be removed, made private to tests, or retained only with semantics that cannot clear `archived_at` except through `unarchive`.

**GraphQL mutation:**

```graphql
type UnarchiveIdeaPayload {
  idea: Idea!
  journalId: ID!
}

type Mutation {
  unarchiveIdea(id: ID!): UnarchiveIdeaPayload!
}
```

### 5.5 `ideas.duplicate` -- create a copy

**MCP tool spec:**

```json
{
  "name": "ideas.duplicate",
  "description": "Create a duplicate of an existing idea. The copy gets a new ID, Draft status, fresh created_at, and no archived_at. Title is prefixed with 'Copy of '.",
  "input_schema": {
    "type": "object",
    "required": ["id"],
    "properties": {
      "id": {
        "type": "string",
        "format": "uuid",
        "description": "The idea ID to duplicate"
      },
      "title_override": {
        "type": "string",
        "description": "Optional title for the copy. If omitted, uses 'Copy of {original title}'."
      }
    }
  }
}
```

**Behavior:**

1. Fetch the source idea. Return error if not found.
2. Create a new `Idea`:
   - `id`: `IdeaId::new()` (fresh UUID).
   - `title`: `title_override` if provided, otherwise `"Copy of {source.title}"`.
   - `body`: copied from source.
   - `workspace_root_path`: copied from source.
   - `project_key`: copied from source.
   - `status`: `IdeaStatus::Draft` (always starts as Draft).
   - `created_at`: `Utc::now()`.
   - `archived_at`: `None`.
3. Insert via `ideas::insert`.
4. Return `{ "idea": new_idea, "journal_id": commanded.journal_id }` for MCP and `DuplicateIdeaPayload { idea, journalId }` for GraphQL.

**GraphQL mutation:**

```graphql
type DuplicateIdeaPayload {
  idea: Idea!
  journalId: ID!
}

type Mutation {
  duplicateIdea(id: ID!, titleOverride: String): DuplicateIdeaPayload!
}
```

### 5.6 Enhanced `ideas.list` -- filtering and pagination

**Updated MCP tool spec** (replaces existing `ideas.list`):

```json
{
  "name": "ideas.list",
  "description": "List ideas with optional filtering by status, project_key, and pagination.",
  "input_schema": {
    "type": "object",
    "properties": {
      "include_archived": {
        "type": "boolean",
        "description": "Whether to include archived ideas (default: false). Ignored if 'status' filter is provided."
      },
      "status": {
        "type": "string",
        "enum": ["draft", "active", "archived"],
        "description": "Filter by idea status. Overrides include_archived when set."
      },
      "project_key": {
        "type": "string",
        "description": "Filter by project cohort key (exact match)."
      },
      "offset": {
        "type": "integer",
        "minimum": 0,
        "description": "Number of results to skip (default: 0)."
      },
      "limit": {
        "type": "integer",
        "minimum": 1,
        "maximum": 200,
        "description": "Maximum number of results to return (default: 50, max: 200)."
      }
    }
  }
}
```

**Behavior:**

1. If `status` is provided, filter by that exact status (ignoring `include_archived`).
2. If `status` is not provided, fall back to current behavior: exclude archived unless `include_archived` is true.
3. If `project_key` is provided, add `AND project_key = ?` to the WHERE clause.
4. Apply `ORDER BY created_at DESC`, then `LIMIT ? OFFSET ?`.
5. Return the list as JSON array.

**New DB repo function:**

```rust
pub struct IdeaListFilter<'a> {
    pub status: Option<IdeaStatus>,
    pub include_archived: bool,
    pub project_key: Option<&'a str>,
    pub offset: i64,
    pub limit: i64,
}

pub async fn list_filtered(pool: &SqlitePool, filter: &IdeaListFilter<'_>) -> Result<Vec<Idea>>
```

**GraphQL query update:**

```graphql
type Query {
  ideas(
    includeArchived: Boolean
    status: String
    projectKey: String
    offset: Int
    limit: Int
  ): [Idea!]!
}
```

### 5.7 Status transition rules (server-side validation)

All status transitions are governed by the following matrix:

| From | To | Allowed via | Guard |
|------|----|-------------|-------|
| Draft | Active | `ideas.update` (automatic when title + body become non-empty) | title.len() > 0 AND body.len() > 0 |
| Draft | Archived | `ideas.archive` | No non-terminal runs |
| Active | Archived | `ideas.archive` | No non-terminal runs |
| Archived | Active | `ideas.unarchive` | None |
| Active | Active | `runs.start` | Idea is already Active and not archived |

Transitions not listed are rejected. In particular:

- Active-to-Draft is not allowed (ideas do not regress).
- Direct status field writes via `ideas.update` are not allowed (use archive/unarchive tools).
- Archived ideas cannot be updated (must unarchive first).
- `runs.start` must not activate Draft ideas. Callers that want to start a run from a Draft must first use `ideas.update` to make title and body non-empty and observe the returned `Active` idea.
- `runs.start` must reject Archived ideas. Callers must use `ideas.unarchive` first, which clears `archived_at` through the explicit unarchive path.
- Duplicated ideas remain Draft and inert until explicitly activated by `ideas.update`.

### 5.8 `runs.start` lifecycle participation

`StartRun` is an existing journaled command and is already a lifecycle participant because current code promotes an idea to `Active` while creating a run. P044 makes this contract explicit and changes it from implicit promotion to eligibility validation:

1. `CommandHandler::execute_command` for `StartRun` must load the idea inside the same command execution path that creates the run.
2. It must reject missing ideas, Draft ideas, and Archived ideas before inserting a run.
3. It must not call a generic `update_status(..., Active)` after run insertion.
4. It must not clear or rewrite `archived_at`.
5. If the command fails eligibility validation, it returns a failed command journal row and inserts no run.
6. The run insert and the archive guard must use compatible locking/transaction semantics so an archive and a start cannot both succeed for the same idea when the resulting state would be an archived idea with a non-terminal run.

This is an intentional behavior change from the current baseline: run start is allowed only for ideas that are already `Active`. Draft-to-Active remains owned by `ideas.update`, where title/body validation and journal readback are explicit.

### 5.9 MCP tool registration

Add the five new tools to `tool_specs()` in `mcp-server/src/tools/ideas.rs` and extend the `execute` match arms. Tool registration must use explicit capability IDs; do not reuse `IdeasList` or `IdeasCreate` for lifecycle writes.

### 5.10 Capability and principal policy

The current northbound auth model is exhaustive: every tool must have a `CapabilityToolId`, a converter mapping, inclusion in all-capability arrays, and class policy. P044 adds the following exact policy:

| Surface | Operation | CapabilityToolId | Principal classes |
|---|---|---|---|
| MCP | `ideas.create` | `IdeasCreate` | Operator, Agent |
| MCP | `ideas.list` | `IdeasRead` | Operator, Agent, Observer |
| MCP | `ideas.get` | `IdeasRead` | Operator, Agent, Observer |
| MCP / GraphQL | `ideas.update` / `updateIdea` | `IdeasUpdate` | Operator, Agent |
| MCP / GraphQL | `ideas.archive` / `archiveIdea` | `IdeasArchive` | Operator, Agent |
| MCP / GraphQL | `ideas.unarchive` / `unarchiveIdea` | `IdeasArchive` | Operator, Agent |
| MCP / GraphQL | `ideas.duplicate` / `duplicateIdea` | `IdeasDuplicate` | Operator, Agent |

`IdeasRead` replaces the existing `IdeasList` capability name for idea read access. After P044, converters must map `ideas.list` and `ideas.get` to `IdeasRead`; no new lifecycle write may use the old `IdeasList` read capability.

Implementation requirements:

- Replace `IdeasList` with `IdeasRead`, and add `IdeasUpdate`, `IdeasArchive`, and `IdeasDuplicate` to `domain::CapabilityToolId`.
- Preserve serialized compatibility for the old read capability name. Current principal persistence stores token, id, and principal class rather than per-principal capability rows, but `CapabilityToolId` is serialized in northbound/debug surfaces and fixtures. Add a one-release `serde` alias from `"IdeasList"` to `IdeasRead`, or provide an explicit fixture migration; the gate must prove old serialized `IdeasList` payloads still read back as `IdeasRead`.
- Update `auth::all_tool_capabilities()` and `auth::tool_allowed_for_class()` with the policy above.
- Update `mcp-server/src/tools/mod.rs` `all_capability_tool_ids()`, `capability_id_for(tool_name)`, and `mcp_tool_for(id)`.
- Update GraphQL `MutationName` and `capability_id_for(mutation)` so `updateIdea`, `archiveIdea`, `unarchiveIdea`, and `duplicateIdea` are authorized through the same capability IDs.
- Add converter tests proving every new MCP tool and GraphQL mutation maps to the expected capability ID and class policy.

### 5.11 Command and journal ownership

Any GraphQL payload that includes `journalId` must be backed by a real `command_journal` row created by `CommandHandler::handle(Command, CallerContext)`. P044 therefore owns idea lifecycle writes in the command layer, not in direct resolver/tool DB writes.

Add these command variants in `domain/src/commands.rs`:

```rust
pub enum Command {
    // existing variants...
    UpdateIdea(UpdateIdeaCmd),
    ArchiveIdea(ArchiveIdeaCmd),
    UnarchiveIdea(UnarchiveIdeaCmd),
    DuplicateIdea(DuplicateIdeaCmd),
}

pub struct UpdateIdeaCmd {
    pub idea_id: IdeaId,
    pub title: PatchField<String>,
    pub body: PatchField<String>,
    pub workspace_root_path: PatchField<Option<String>>,
    pub project_key: PatchField<Option<String>>,
}

pub struct ArchiveIdeaCmd {
    pub idea_id: IdeaId,
}

pub struct UnarchiveIdeaCmd {
    pub idea_id: IdeaId,
}

pub struct DuplicateIdeaCmd {
    pub source_idea_id: IdeaId,
    pub title_override: Option<String>,
}
```

`PatchField<T>` is a small serializable domain helper with `Unchanged` and `Set(T)` variants. It is not exposed northbound; GraphQL `MaybeUndefined` and MCP JSON parameter presence are converted into it before command execution.

Add `CommandResult` variants that carry the resulting idea so GraphQL and MCP can return the exact persisted row:

```rust
pub enum CommandResult {
    // existing variants...
    IdeaUpdated { idea: Idea },
    IdeaArchived { idea: Idea },
    IdeaUnarchived { idea: Idea },
    IdeaDuplicated { idea: Idea },
}
```

`CommandHandler::execute_command` owns:

- archive active-run guard
- archived-idea update rejection
- Draft-to-Active validation and transition
- unarchive `archived_at` clearing
- duplicate creation
- `StartRun` idea eligibility validation (`Active` only, not Draft or Archived)
- archive/start atomicity so archive and start cannot race into invalid persisted truth
- DB repo calls for the final write

Journal behavior:

- GraphQL `updateIdea`, `archiveIdea`, `unarchiveIdea`, and `duplicateIdea` call `CommandHandler::handle` with `CallerContext::graphql(...)` and return `commanded.journal_id`.
- MCP `ideas.update`, `ideas.archive`, `ideas.unarchive`, and `ideas.duplicate` call `CommandHandler::handle` with `CallerContext::mcp(...)` and return `{ "idea": ..., "journal_id": ... }`.
- `ideas.get` and `ideas.list` remain read-only and do not create command journal rows.
- `StartRun` remains the existing run-scoped command. P044 does not add a new journal payload for it, but it does require the existing `StartRun` command journal to record validation failures when the idea is Draft or Archived.
- `run_id_for_journal` is `None` for idea lifecycle commands because they are idea-scoped, not run-scoped.
- `command_journal_redact::redact_for_journal` must cover the new command variants. Redact free-text and local-path patch values (`title`, `body`, `workspace_root_path`, `title_override`) while preserving field presence and IDs. `project_key` may remain visible because it is a cohort identifier.

Resolvers and tools must not fabricate `journalId` values and must not write idea lifecycle changes directly through `db::repos::ideas` when a journaled command exists.

---

## 6. Migration

### 6.1 DB schema

No schema migration required. The `ideas` table already has all columns: `id`, `title`, `body`, `workspace_root_path`, `project_key`, `status`, `created_at`, `archived_at`. The new repo functions operate on existing columns.

### 6.2 Status helper split

The existing `update_status` function uses `COALESCE(?2, archived_at)` and is currently called by `StartRun` with `IdeaStatus::Active`. P044 must not change that generic helper to unconditional `archived_at = ?2`; doing so would make `StartRun` capable of clearing archive metadata.

Instead, the DB repo owns explicit lifecycle helpers:

- `archive_if_no_active_runs`: atomically verifies no non-terminal runs and sets `status = Archived`, `archived_at = now`.
- `unarchive`: sets `status = Active`, `archived_at = NULL`.
- `update_fields`: applies patch fields and may promote Draft to Active while preserving `archived_at`.

After these helpers exist, production command paths should stop using generic `update_status` for idea lifecycle transitions.

### 6.3 Existing `ideas.list` callers

The enhanced `ideas.list` is backward-compatible: all new parameters are optional. Existing callers that send only `include_archived` continue to work identically. The existing DB function `list(pool, include_archived)` can be retained as a convenience wrapper that delegates to `list_filtered`.

### 6.4 GraphQL schema additions

New mutations are additive. Existing queries and mutations are unchanged. No client breakage.

---

## 7. Implementation Inventory and Gate

### 7.1 Files to modify

| File | Change |
|---|---|
| `domain/src/commands.rs` | Add idea lifecycle command structs, `PatchField`, and `Command` variants |
| `domain/src/capabilities.rs` | Replace `IdeasList` with `IdeasRead`; add `IdeasUpdate`, `IdeasArchive`, `IdeasDuplicate` |
| `db/src/repos/ideas.rs` | Add `update_fields`, `duplicate`, `list_filtered`, `archive_if_no_active_runs`, and `unarchive`; avoid generic `update_status` semantics that let `StartRun` clear `archived_at` |
| `engine/src/command_handler.rs` | Add `CommandResult` variants, execute idea lifecycle commands with guards, and update `StartRun` to require an already-Active idea without mutating idea status |
| `engine/src/command_journal_redact.rs` | Redact free-text/path fields for idea lifecycle commands |
| `auth/src/lib.rs` | Update all-capability inventory, principal-class policy, and tool-name converter tests |
| `mcp-server/src/tools/ideas.rs` | Add new tool specs, route lifecycle writes through `CommandHandler`, return real `journal_id` |
| `mcp-server/src/tools/mod.rs` | Update exhaustive capability registration and tool lookup |
| `graphql-server/src/schema.rs` | Add GraphQL mutations, `MaybeUndefined` patch input, mutation capability mapping, and `CommandHandler` routing |
| `scripts/test-gate.sh` | Add distinct `proposal-044-ideas|p044-ideas` gate without changing existing `proposal-044|p044` |
| `docs/reference/test-gates.md` | Document `proposal-044-ideas|p044-ideas` and preserve the existing Proposal 044 post-approval gate |
| `docs/reference/idea-lifecycle.md` | Update lifecycle reference if needed so `runs.start` eligibility and Draft activation ownership match P044 |

### 7.2 Canonical gate

The canonical proof gate for this proposal is:

```bash
./scripts/test-gate.sh proposal-044-ideas
```

The runner must also accept `p044-ideas`. `./scripts/test-gate.sh proposal-044` and `p044` remain owned by the existing post-approval task execution/release gate and must not be changed by this proposal.

The gate must include focused Rust/control-plane tests for:

- idea command execution and command journal rows
- GraphQL mutation `journalId` readback from real command journal IDs
- MCP lifecycle write `journal_id` readback from real command journal IDs
- capability converter and principal-class policy for all new idea tools and mutations
- async-graphql `MaybeUndefined` patch behavior
- lifecycle guards and list filtering
- `StartRun` lifecycle eligibility, including Draft/Archived rejection and preservation of `archived_at`
- archive/start race coverage proving the atomic guard cannot produce an archived idea with a non-terminal run
- `IdeasList` serialized compatibility or fixture migration for the `IdeasRead` rename

---

## 8. Verification

### 8.1 `ideas.get`

- Calling `ideas.get` with a valid ID returns the full idea JSON.
- Calling `ideas.get` with a non-existent UUID returns an error containing "not found".
- Calling `ideas.get` with a malformed ID returns a parse error.

### 8.2 `ideas.update`

- Sending only `{ "id": "...", "title": "New Title" }` updates the title and leaves body, workspace_root_path, and project_key unchanged.
- Sending `{ "id": "...", "project_key": null }` clears the project_key field.
- Updating a Draft idea to have non-empty title and body transitions it to Active automatically.
- Updating an Archived idea returns an error mentioning "unarchive".
- Updating `workspace_root_path` or `project_key` affects future idea readback only and does not mutate existing run workspace roots or release artifact metadata.

### 8.3 `ideas.archive`

- Archiving an Active idea with no runs succeeds. The returned idea has status "archived" and a non-null `archived_at`.
- Archiving an idea with a `running` run returns an error listing the active run statuses.
- Archiving an idea with only `completed` and `failed` runs succeeds.
- Archiving an already-archived idea returns the idea unchanged (idempotent).
- Archive/start interleaving cannot produce an archived idea with a newly inserted non-terminal run. At most one of the archive command and `StartRun` command succeeds when they race for the same idea.

### 8.4 `ideas.unarchive`

- Unarchiving an Archived idea returns it with status "active" and `archived_at` cleared to null.
- Unarchiving a non-archived idea returns it unchanged (idempotent).
- Unarchive is the only lifecycle path that clears `archived_at`.

### 8.5 `runs.start` lifecycle eligibility

- Starting a run from an Active idea succeeds and does not mutate idea status or `archived_at`.
- Starting a run from a Draft idea fails before run insertion and returns a failed `StartRun` command journal row.
- Starting a run from a duplicated Draft idea fails before run insertion; the duplicate remains Draft.
- Starting a run from an Archived idea fails before run insertion and preserves the archived idea's `archived_at`.
- A Draft idea can become eligible only after `ideas.update` promotes it to Active by setting non-empty title and body.

### 8.6 `ideas.duplicate`

- Duplicating an idea returns a new idea with a different ID, status "draft", fresh `created_at`, and title prefixed with "Copy of ".
- Providing `title_override` uses that title instead of the prefix.
- The source idea is not modified.

### 8.7 Enhanced `ideas.list`

- `ideas.list` with no parameters returns non-archived ideas (backward-compatible).
- `ideas.list` with `{ "status": "draft" }` returns only Draft ideas.
- `ideas.list` with `{ "project_key": "my-project" }` returns only ideas in that project.
- `ideas.list` with `{ "offset": 10, "limit": 5 }` returns at most 5 ideas starting from the 11th.
- `ideas.list` with `{ "status": "archived" }` returns archived ideas regardless of `include_archived`.

### 8.8 GraphQL parity

- `updateIdea`, `archiveIdea`, `unarchiveIdea`, `duplicateIdea` mutations succeed and return `journalId`.
- The `ideas` query accepts `status`, `projectKey`, `offset`, `limit` arguments.
- All guard rails (active-run check, archived-idea update rejection, Draft-to-Active auto-transition) apply identically in both MCP and GraphQL paths.
- GraphQL `updateIdea` uses `MaybeUndefined` patch input semantics: omitted nullable fields remain unchanged, explicit `null` clears `workspaceRootPath`/`projectKey`, and explicit `null` for `title`/`body` is rejected.

### 8.9 Command journal and auth proof

- Every successful `updateIdea`, `archiveIdea`, `unarchiveIdea`, and `duplicateIdea` response has a `journalId` that exists in `command_journal`.
- Failed idea lifecycle commands create a failed command journal row rather than returning a fabricated ID.
- MCP `ideas.update`, `ideas.archive`, `ideas.unarchive`, and `ideas.duplicate` return `journal_id` from `CommandHandler::handle`.
- `command_journal_redact` redacts configured idea free-text/path fields and preserves IDs/field presence.
- `CapabilityToolId` converter tests cover `ideas.create`, `ideas.list`, `ideas.get`, `ideas.update`, `ideas.archive`, `ideas.unarchive`, and `ideas.duplicate`, including `ideas.list`/`ideas.get` -> `IdeasRead`.
- Serialized capability compatibility tests cover old `"IdeasList"` payloads reading back as `IdeasRead` or the explicit fixture migration chosen by the implementation.
- GraphQL mutation capability tests cover `updateIdea`, `archiveIdea`, `unarchiveIdea`, and `duplicateIdea`.
- Principal class tests prove Observers can read ideas but cannot create, update, archive, unarchive, or duplicate; Operators and Agents can use the lifecycle tools.
- `./scripts/test-gate.sh proposal-044-ideas` passes and `./scripts/test-gate.sh proposal-044` still points to the existing post-approval/release gate.

---

## 9. Risks

| Risk | Severity | Mitigation |
|------|----------|------------|
| Active-run guard blocks legitimate archive when a run is stuck in `pending` | Low | The operator can cancel the stuck run first via `runs.cancel`, then archive. The error message lists the blocking statuses to guide the operator. |
| Auto-transition from Draft to Active on update surprises callers | Low | The behavior is documented in the tool description. Callers that want to keep an idea in Draft can leave title or body empty. The transition is also visible in the returned idea's `status` field. |
| Patch semantics for nullable fields (null vs. absent) are hard to express in GraphQL | Medium | MCP uses JSON key presence; GraphQL uses `async_graphql::MaybeUndefined<String>` and tests omitted vs explicit null behavior. |
| `ideas.duplicate` could be used to create many junk ideas | Low | Duplication creates Draft ideas which are inert (cannot have runs started until they reach Active). Standard rate limiting and principal-based access control apply. |
| A generic status helper could clear `archived_at` from an unrelated caller such as `StartRun` | High | Split lifecycle writes into archive/unarchive/update helpers. Only `unarchive` may clear `archived_at`; `StartRun` must stop mutating idea status. |
| Archive and `StartRun` could race if the active-run check is not atomic | High | Implement archive guard and run start eligibility with compatible DB transaction/locking semantics and add race-focused tests to the proposal gate. |
| Requiring Active before `StartRun` changes current implicit Draft promotion behavior | Medium | Make the behavior explicit in MCP/GraphQL errors and tests. Draft-to-Active remains available through `ideas.update`, where validation and journal ownership are clear. |
| `ideas.list` with large offset on a big table is slow (SQLite OFFSET scans) | Low | The 200-row limit cap and the expected idea-set size (tens to low hundreds) make this a non-issue for the foreseeable future. Keyset pagination can be added later if needed. |
| New lifecycle tools accidentally bypass command journaling | High | GraphQL and MCP write paths must route through `CommandHandler`; verification asserts returned journal IDs exist in `command_journal`. |
| New idea tools get the wrong principal-class semantics | High | P044 adds explicit capability IDs and converter/class-policy tests instead of reusing `IdeasList` or `IdeasCreate`. |
| Renaming `IdeasList` to `IdeasRead` breaks serialized fixtures/debug payloads | Medium | Keep a one-release serde alias or implement a fixture migration, and gate it with compatibility readback tests. |
