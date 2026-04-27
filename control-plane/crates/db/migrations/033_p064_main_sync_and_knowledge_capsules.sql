-- P064: Run Worktree Main Sync and Cross-Run Knowledge Transfer
-- Phase 0 contract freeze — schema-only, no data writes until Phase 1.
--
-- Tables:
--   main_sync_attempts          — durable sync attempt lifecycle
--   main_sync_conflict_files    — conflict file list per attempt
--   worktree_mutation_barriers  — exclusive barrier leases
--   run_knowledge_capsules      — emitted capsule records
--   run_knowledge_capsule_match_keys — capsule relevance tags
--   run_knowledge_capsule_attachments — capsule-to-run attachments
--
-- Column additions:
--   work_items.worktree_access_mode    — none|read|write
--   work_items.worktree_resource_key   — run worktree identity for barrier coordination
--   background_leases.*                — worktree mutation ownership readback

-- ── Main sync attempts ────────────────────────────────────────────────

CREATE TABLE IF NOT EXISTS main_sync_attempts (
    id                      TEXT    PRIMARY KEY,
    run_id                  TEXT    NOT NULL,
    idempotency_key         TEXT    NOT NULL,
    trigger_reason          TEXT    NOT NULL,
    status                  TEXT    NOT NULL DEFAULT 'pending',
    barrier_id              TEXT,
    target_main_ref         TEXT,
    pre_sync_branch         TEXT,
    pre_sync_commit         TEXT,
    post_sync_commit        TEXT,
    preservation_commit     TEXT,
    conflict_count          INTEGER DEFAULT 0,
    resolver_work_item_id   TEXT,
    error_message           TEXT,
    recovery_note           TEXT,
    requested_by_stage_id   TEXT,
    requested_by_work_item_id TEXT,
    created_at              TEXT    NOT NULL,
    started_at              TEXT,
    completed_at            TEXT
);

CREATE INDEX IF NOT EXISTS idx_main_sync_attempts_run_id
    ON main_sync_attempts(run_id);

CREATE UNIQUE INDEX IF NOT EXISTS idx_main_sync_attempts_idempotency
    ON main_sync_attempts(run_id, idempotency_key);

CREATE INDEX IF NOT EXISTS idx_main_sync_attempts_status
    ON main_sync_attempts(status)
    WHERE status IN ('pending', 'waiting_for_barrier', 'inspecting', 'preserving', 'merging');

-- ── Main sync conflict files ──────────────────────────────────────────

CREATE TABLE IF NOT EXISTS main_sync_conflict_files (
    id              TEXT    PRIMARY KEY,
    attempt_id      TEXT    NOT NULL REFERENCES main_sync_attempts(id),
    file_path       TEXT    NOT NULL,
    conflict_type   TEXT,
    created_at      TEXT    NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_main_sync_conflict_files_attempt
    ON main_sync_conflict_files(attempt_id);

-- ── Worktree mutation barriers ────────────────────────────────────────

CREATE TABLE IF NOT EXISTS worktree_mutation_barriers (
    id                      TEXT    PRIMARY KEY,
    run_id                  TEXT    NOT NULL,
    worktree_resource_key   TEXT    NOT NULL,
    owner_id                TEXT    NOT NULL,
    owner_kind              TEXT    NOT NULL,
    status                  TEXT    NOT NULL DEFAULT 'pending',
    reason                  TEXT    NOT NULL,
    acquired_at             TEXT,
    released_at             TEXT,
    heartbeat_at            TEXT,
    expires_at              TEXT    NOT NULL,
    created_at              TEXT    NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_worktree_barriers_resource_key
    ON worktree_mutation_barriers(worktree_resource_key)
    WHERE status IN ('pending', 'active');

CREATE INDEX IF NOT EXISTS idx_worktree_barriers_run_id
    ON worktree_mutation_barriers(run_id);

-- ── Knowledge capsules ────────────────────────────────────────────────

CREATE TABLE IF NOT EXISTS run_knowledge_capsules (
    id                  TEXT    PRIMARY KEY,
    source_run_id       TEXT    NOT NULL,
    source_proposal_id  TEXT,
    source_status       TEXT    NOT NULL,
    schema_version      TEXT    NOT NULL,
    capsule_yaml        TEXT    NOT NULL,
    byte_count          INTEGER NOT NULL DEFAULT 0,
    token_count         INTEGER NOT NULL DEFAULT 0,
    status              TEXT    NOT NULL DEFAULT 'active',
    ignored_reason      TEXT,
    ignored_at          TEXT,
    expires_at          TEXT,
    created_at          TEXT    NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_knowledge_capsules_source_run
    ON run_knowledge_capsules(source_run_id);

CREATE INDEX IF NOT EXISTS idx_knowledge_capsules_status
    ON run_knowledge_capsules(status)
    WHERE status = 'active';

-- ── Knowledge capsule match keys ──────────────────────────────────────

CREATE TABLE IF NOT EXISTS run_knowledge_capsule_match_keys (
    id          TEXT    PRIMARY KEY,
    capsule_id  TEXT    NOT NULL REFERENCES run_knowledge_capsules(id),
    match_rule  TEXT    NOT NULL,
    match_key   TEXT    NOT NULL,
    created_at  TEXT    NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_capsule_match_keys_capsule
    ON run_knowledge_capsule_match_keys(capsule_id);

CREATE INDEX IF NOT EXISTS idx_capsule_match_keys_rule_key
    ON run_knowledge_capsule_match_keys(match_rule, match_key);

-- ── Knowledge capsule attachments ─────────────────────────────────────

CREATE TABLE IF NOT EXISTS run_knowledge_capsule_attachments (
    id              TEXT    PRIMARY KEY,
    capsule_id      TEXT    NOT NULL REFERENCES run_knowledge_capsules(id),
    target_run_id   TEXT    NOT NULL,
    match_rule      TEXT    NOT NULL,
    attachment_reason TEXT  NOT NULL,
    injected        INTEGER NOT NULL DEFAULT 0,
    injected_byte_count  INTEGER,
    injected_token_count INTEGER,
    truncated       INTEGER NOT NULL DEFAULT 0,
    stale_main      INTEGER NOT NULL DEFAULT 0,
    ignored         INTEGER NOT NULL DEFAULT 0,
    ignored_reason  TEXT,
    ignored_at      TEXT,
    created_at      TEXT    NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_capsule_attachments_target_run
    ON run_knowledge_capsule_attachments(target_run_id);

CREATE INDEX IF NOT EXISTS idx_capsule_attachments_capsule
    ON run_knowledge_capsule_attachments(capsule_id);

-- ── Work items: worktree access metadata ──────────────────────────────
-- Indexed columns for barrier coordination and scheduler claim exclusion.

ALTER TABLE work_items ADD COLUMN worktree_access_mode TEXT DEFAULT 'none';
ALTER TABLE work_items ADD COLUMN worktree_resource_key TEXT;

CREATE INDEX IF NOT EXISTS idx_work_items_worktree_resource
    ON work_items(worktree_resource_key, worktree_access_mode)
    WHERE worktree_access_mode IN ('read', 'write')
      AND status IN ('pending', 'running');

-- ── Background leases: worktree mutation ownership ────────────────────
-- Existing background_leases rows prevent double-dispatch. P064 extends the
-- same projection with worktree ownership metadata so waiting_for_barrier
-- readback can name the active consumer/owner without opaque JSON.

ALTER TABLE background_leases ADD COLUMN run_id TEXT;
ALTER TABLE background_leases ADD COLUMN worktree_resource_key TEXT;
ALTER TABLE background_leases ADD COLUMN worktree_access_mode TEXT DEFAULT 'none';
ALTER TABLE background_leases ADD COLUMN owner_kind TEXT;
ALTER TABLE background_leases ADD COLUMN reason TEXT;
ALTER TABLE background_leases ADD COLUMN heartbeat_at TEXT;
ALTER TABLE background_leases ADD COLUMN released_at TEXT;

CREATE INDEX IF NOT EXISTS idx_background_leases_worktree_owner
    ON background_leases(worktree_resource_key, worktree_access_mode, owner_kind)
    WHERE worktree_resource_key IS NOT NULL
      AND worktree_access_mode IN ('read', 'write')
      AND released_at IS NULL;
