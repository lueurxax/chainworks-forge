-- Session lineage metadata
CREATE TABLE IF NOT EXISTS session_lineages (
  id TEXT PRIMARY KEY,
  run_id TEXT NOT NULL REFERENCES runs(id),
  stage_id TEXT NOT NULL,
  agent_id TEXT NOT NULL,
  lineage_kind TEXT NOT NULL,   -- "fresh" | "reused" | "reset"
  previous_session_id TEXT,
  created_at TEXT NOT NULL
);

-- Aggregate settlements
CREATE TABLE IF NOT EXISTS aggregate_settlements (
  id TEXT PRIMARY KEY,
  run_id TEXT NOT NULL REFERENCES runs(id),
  settlement_kind TEXT NOT NULL,
  settled_at TEXT NOT NULL,
  notes TEXT
);

-- Background leases (prevents double-dispatch)
CREATE TABLE IF NOT EXISTS background_leases (
  id TEXT PRIMARY KEY,
  resource_key TEXT NOT NULL UNIQUE,
  owner_id TEXT NOT NULL,
  acquired_at TEXT NOT NULL,
  expires_at TEXT NOT NULL
);

-- Startup repair log
CREATE TABLE IF NOT EXISTS startup_repairs (
  id TEXT PRIMARY KEY,
  run_id TEXT NOT NULL,
  repair_kind TEXT NOT NULL,   -- "stage_blocked" | "work_item_requeued" | "approval_expired"
  repaired_at TEXT NOT NULL,
  notes TEXT
);

-- Runtime invocations (ACP adapter calls)
CREATE TABLE IF NOT EXISTS runtime_invocations (
  id TEXT PRIMARY KEY,
  agent_execution_id TEXT NOT NULL,
  provider TEXT NOT NULL,
  model TEXT,
  started_at TEXT NOT NULL,
  completed_at TEXT,
  status TEXT NOT NULL DEFAULT 'running',
  cost_cents INTEGER,
  error TEXT
);

-- Projection: run summaries (materialized, rebuilt on mutation)
CREATE TABLE IF NOT EXISTS run_summaries (
  run_id TEXT PRIMARY KEY REFERENCES runs(id),
  idea_id TEXT NOT NULL,
  workflow_title TEXT NOT NULL,
  status TEXT NOT NULL,
  total_stages INTEGER NOT NULL DEFAULT 0,
  completed_stages INTEGER NOT NULL DEFAULT 0,
  failed_stages INTEGER NOT NULL DEFAULT 0,
  pending_approvals INTEGER NOT NULL DEFAULT 0,
  started_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

-- Projection: stage summaries
CREATE TABLE IF NOT EXISTS stage_summaries (
  stage_execution_id TEXT PRIMARY KEY REFERENCES stage_executions(id),
  run_id TEXT NOT NULL,
  stage_id TEXT NOT NULL,
  label TEXT NOT NULL,
  status TEXT NOT NULL,
  attempt_number INTEGER NOT NULL DEFAULT 1,
  has_artifacts INTEGER NOT NULL DEFAULT 0,
  has_pending_approval INTEGER NOT NULL DEFAULT 0,
  updated_at TEXT NOT NULL
);

-- Projection: approval inbox (pending/requested approvals)
CREATE TABLE IF NOT EXISTS approval_inbox (
  approval_id TEXT PRIMARY KEY REFERENCES approvals(id),
  run_id TEXT NOT NULL,
  stage_id TEXT NOT NULL,
  workflow_title TEXT NOT NULL,
  requested_at TEXT NOT NULL,
  expires_at TEXT,
  updated_at TEXT NOT NULL
);

-- Projection: artifact index
CREATE TABLE IF NOT EXISTS artifact_index (
  artifact_id TEXT PRIMARY KEY REFERENCES artifacts(id),
  run_id TEXT NOT NULL,
  stage_id TEXT NOT NULL,
  name TEXT NOT NULL,
  format TEXT NOT NULL,
  file_path TEXT NOT NULL,
  is_pinned INTEGER NOT NULL DEFAULT 0,
  report_kind TEXT,
  created_at TEXT NOT NULL
);

-- Recovery recommendations
CREATE TABLE IF NOT EXISTS recovery_recommendations (
  id TEXT PRIMARY KEY,
  run_id TEXT NOT NULL REFERENCES runs(id),
  stage_id TEXT,
  recommendation_kind TEXT NOT NULL,  -- "retry_stage" | "cancel_run" | "manual_review"
  reason TEXT NOT NULL,
  created_at TEXT NOT NULL,
  resolved_at TEXT
);

-- Indexes on projections
CREATE INDEX IF NOT EXISTS idx_run_summaries_idea_id ON run_summaries(idea_id);
CREATE INDEX IF NOT EXISTS idx_run_summaries_status ON run_summaries(status);
CREATE INDEX IF NOT EXISTS idx_stage_summaries_run_id ON stage_summaries(run_id);
CREATE INDEX IF NOT EXISTS idx_approval_inbox_run_id ON approval_inbox(run_id);
CREATE INDEX IF NOT EXISTS idx_artifact_index_run_id ON artifact_index(run_id);
CREATE INDEX IF NOT EXISTS idx_runtime_invocations_agent_execution_id ON runtime_invocations(agent_execution_id);
