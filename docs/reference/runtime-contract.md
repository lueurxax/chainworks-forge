# Runtime Contract

This document captures the minimum runtime contracts that should stay stable while the app implementation grows.

## 1. Frozen run snapshot

When a run starts, the app must compile the current workflow and agent catalog into an immutable `RunPlanSnapshot`.

That snapshot freezes:

- `WorkflowVersion`
- `AgentCatalogVersion`
- `BackendProfileVersion`
- stage graph and agent bindings
- permission bindings
- artifact paths
- runtime settings such as resume/retry policy

Resume always uses the stored snapshot, not the latest YAML files on disk.

## 2. State machines

The runtime should track separate status machines instead of one overloaded enum.

Recommended status sets:

- **Run**: `pending`, `ready`, `running`, `waiting_approval`, `blocked`, `completed`, `failed`, `cancelled`
- **Stage**: `pending`, `ready`, `running`, `waiting_approval`, `blocked`, `completed`, `failed`, `skipped`
- **Agent execution**: `pending`, `ready`, `running`, `completed`, `failed`, `cancelled`, `skipped`
- **Approval**: `pending`, `requested`, `granted`, `rejected`, `expired`
- **Side effect**: `pending`, `armed`, `running`, `completed`, `failed`, `blocked`

These states should be visible in SwiftData metadata and in the UI.

## 3. Artifact model

Artifacts are first-class objects.

Examples:

- `idea.md`
- `proposal.md`
- `review.json`
- `audit.md`
- `patch.diff`
- `run-report.json`

Each artifact should carry provenance:

- `artifact_id`
- `run_id`
- `stage_id`
- `agent_id`
- `provider`
- `model`
- `effort`
- `created_at`
- `path`
- `checksum`

Artifacts should be immutable per stage attempt. New attempts create new artifacts instead of mutating old ones.

## 4. Storage boundary

SwiftData should store only:

- ids
- statuses
- indexes
- aggregates
- artifact references
- checksums
- lightweight previews

The artifact store on disk should keep:

- raw logs
- markdown summaries
- structured JSON payloads
- diffs
- large reports

## 5. Filesystem and worktree policy

Minimum MVP policy:

- one run = one workspace root
- code-writing happens in a dedicated writable repo worktree
- review agents read from snapshot or read-only workspace state
- no two write-capable agents may write to the same worktree concurrently in MVP
- release side effects stay outside general write-capable agents

## 6. Bounded Artifact Discovery (P053)

The system uses a bounded discovery model to minimize startup latency and ensure artifact integrity.

- **Meta-root Bounding**: Discovery is restricted to the run-owned meta-root.
- **Exact-path Reads**: Declared expected outputs are read only from their exact paths.
- **Pre-Prompt Metadata**: Metadata is captured per-execution to ensure freshness.
- **Engine-owned Settlement**: The engine discovery pipeline settles artifacts based on typed expected outputs and discovery decisions.

## 7. Resume and retry policy

- safe local stages may auto-resume
- approval stages return to `waiting_approval`
- external side-effect stages never auto-resume silently
- retries are bounded by stage/workflow policy
- each retry creates a new stage attempt and new artifacts

## 8. Provider boundary

MVP provider boundary:

- required now: `codex_acp`, `claude_acp`, `gemini_acp`, with optional `auggie` and `junie` families when configured
- post-MVP via provider adapter extension: additional backends beyond the MVP provider set
