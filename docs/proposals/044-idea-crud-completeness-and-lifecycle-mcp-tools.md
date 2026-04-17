# Proposal 044: Idea CRUD Completeness and Lifecycle MCP Tools

| Field | Value |
|---|---|
| Date | 2026-04-17 |
| Status | Draft |
| Author | Andrey Khasanov |
| Depends on | [../reference/domain-model.md](../reference/domain-model.md) |
| Scope | Add get, update, archive, unarchive, and duplicate MCP tools for ideas, plus enhanced list filtering, with matching GraphQL mutations and server-side guard rails. |
| Goal | Give MCP-connected agents and operators full idea lifecycle management with patch-semantic updates and safety guards that exceed the Swift app's current capabilities. |

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
- New GraphQL mutations: `updateIdea`, `archiveIdea`, `unarchiveIdea`, `duplicateIdea`.
- Updated GraphQL `ideas` query with filter/pagination arguments.

This proposal does **not** include:

- Changes to the Swift app UI (the app can adopt the new GraphQL mutations separately).
- Idea deletion (ideas are archived, not deleted; deletion is out of scope).
- Bulk operations (archive-all, update-many).
- Changes to `Run`, `Artifact`, or any domain model other than `Idea`.
- Changes to MCP tool authorization / principal-based access control (existing `auth` layer applies unchanged).

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
5. Persist via new repo function `ideas::update_fields`.
6. Return the updated `Idea` as JSON.

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

Note: GraphQL nullable fields use the `String` scalar (nullable by default). To distinguish "omit" from "set to null", the resolver checks whether the field key was present in the input object.

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
3. **Active-run guard:** Call `runs::list_by_idea(pool, id)` and check whether any run has a non-terminal status (i.e., `!run.status.is_terminal()`). If any non-terminal run exists, return error:
   ```
   "Cannot archive idea {id}: {n} run(s) are still active
   (statuses: {comma-separated statuses}). Cancel or wait for
   them to complete before archiving."
   ```
4. Set status to `Archived` and `archived_at` to `Utc::now()` via `ideas::update_status`.
5. Return the updated `Idea`.

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
3. Set status to `Active` and clear `archived_at` to `None`.
4. Return the updated `Idea`.

**DB change:** `update_status` already sets `archived_at` conditionally when the new status is `Archived`. Extend it to clear `archived_at` when the new status is not `Archived`:

```rust
// In update_status, replace COALESCE logic:
let archived_at: Option<String> = if matches!(status, IdeaStatus::Archived) {
    Some(Utc::now().to_rfc3339())
} else {
    None  // This clears archived_at for unarchive
};

// SQL: UPDATE ideas SET status = ?1, archived_at = ?2 WHERE id = ?3
// (unconditional set, not COALESCE)
```

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
4. Return the new `Idea`.

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

Transitions not listed are rejected. In particular:

- Active-to-Draft is not allowed (ideas do not regress).
- Direct status field writes via `ideas.update` are not allowed (use archive/unarchive tools).
- Archived ideas cannot be updated (must unarchive first).

### 5.8 MCP tool registration

Add the five new tools to `tool_specs()` in `mcp-server/src/tools/ideas.rs` and extend the `execute` match arms. The capability tool ID for all idea tools maps to a new `CapabilityToolId::IdeasManage` variant (or reuse the existing tool-registration pattern if no capability gating is needed for ideas).

---

## 6. Migration

### 6.1 DB schema

No schema migration required. The `ideas` table already has all columns: `id`, `title`, `body`, `workspace_root_path`, `project_key`, `status`, `created_at`, `archived_at`. The new repo functions operate on existing columns.

### 6.2 `update_status` behavior change

The existing `update_status` function uses `COALESCE(?2, archived_at)` which preserves `archived_at` when the new value is NULL. For unarchive to work correctly, this must change to an unconditional set: `archived_at = ?2`. This is a backward-compatible change because the only callers today set `archived_at` when archiving and pass NULL otherwise (which COALESCE also handles as no-op on non-archived ideas).

### 6.3 Existing `ideas.list` callers

The enhanced `ideas.list` is backward-compatible: all new parameters are optional. Existing callers that send only `include_archived` continue to work identically. The existing DB function `list(pool, include_archived)` can be retained as a convenience wrapper that delegates to `list_filtered`.

### 6.4 GraphQL schema additions

New mutations are additive. Existing queries and mutations are unchanged. No client breakage.

---

## 7. Verification

### 7.1 `ideas.get`

- Calling `ideas.get` with a valid ID returns the full idea JSON.
- Calling `ideas.get` with a non-existent UUID returns an error containing "not found".
- Calling `ideas.get` with a malformed ID returns a parse error.

### 7.2 `ideas.update`

- Sending only `{ "id": "...", "title": "New Title" }` updates the title and leaves body, workspace_root_path, and project_key unchanged.
- Sending `{ "id": "...", "project_key": null }` clears the project_key field.
- Updating a Draft idea to have non-empty title and body transitions it to Active automatically.
- Updating an Archived idea returns an error mentioning "unarchive".

### 7.3 `ideas.archive`

- Archiving an Active idea with no runs succeeds. The returned idea has status "archived" and a non-null `archived_at`.
- Archiving an idea with a `running` run returns an error listing the active run statuses.
- Archiving an idea with only `completed` and `failed` runs succeeds.
- Archiving an already-archived idea returns the idea unchanged (idempotent).

### 7.4 `ideas.unarchive`

- Unarchiving an Archived idea returns it with status "active" and `archived_at` cleared to null.
- Unarchiving a non-archived idea returns it unchanged (idempotent).

### 7.5 `ideas.duplicate`

- Duplicating an idea returns a new idea with a different ID, status "draft", fresh `created_at`, and title prefixed with "Copy of ".
- Providing `title_override` uses that title instead of the prefix.
- The source idea is not modified.

### 7.6 Enhanced `ideas.list`

- `ideas.list` with no parameters returns non-archived ideas (backward-compatible).
- `ideas.list` with `{ "status": "draft" }` returns only Draft ideas.
- `ideas.list` with `{ "project_key": "my-project" }` returns only ideas in that project.
- `ideas.list` with `{ "offset": 10, "limit": 5 }` returns at most 5 ideas starting from the 11th.
- `ideas.list` with `{ "status": "archived" }` returns archived ideas regardless of `include_archived`.

### 7.7 GraphQL parity

- `updateIdea`, `archiveIdea`, `unarchiveIdea`, `duplicateIdea` mutations succeed and return `journalId`.
- The `ideas` query accepts `status`, `projectKey`, `offset`, `limit` arguments.
- All guard rails (active-run check, archived-idea update rejection, Draft-to-Active auto-transition) apply identically in both MCP and GraphQL paths.

---

## 8. Risks

| Risk | Severity | Mitigation |
|------|----------|------------|
| Active-run guard blocks legitimate archive when a run is stuck in `pending` | Low | The operator can cancel the stuck run first via `runs.cancel`, then archive. The error message lists the blocking statuses to guide the operator. |
| Auto-transition from Draft to Active on update surprises callers | Low | The behavior is documented in the tool description. Callers that want to keep an idea in Draft can leave title or body empty. The transition is also visible in the returned idea's `status` field. |
| Patch semantics for nullable fields (null vs. absent) are hard to express in JSON | Medium | The implementation distinguishes `"project_key": null` (clear) from key-absent (no change) by checking `params.get("project_key")` presence before reading the value. This is standard JSON Merge Patch (RFC 7396) behavior. |
| `ideas.duplicate` could be used to create many junk ideas | Low | Duplication creates Draft ideas which are inert (cannot have runs started until they reach Active). Standard rate limiting and principal-based access control apply. |
| Changing `update_status` COALESCE to unconditional set could clear `archived_at` unexpectedly | Low | The only callers of `update_status` are the archive/unarchive paths, which always provide the correct `archived_at` value. The new behavior is actually more correct: unarchive should clear the timestamp. |
| `ideas.list` with large offset on a big table is slow (SQLite OFFSET scans) | Low | The 200-row limit cap and the expected idea-set size (tens to low hundreds) make this a non-issue for the foreseeable future. Keyset pagination can be added later if needed. |
