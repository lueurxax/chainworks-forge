-- Ideas
CREATE TABLE IF NOT EXISTS ideas (
  id TEXT PRIMARY KEY,
  title TEXT NOT NULL,
  body TEXT NOT NULL,
  workspace_root_path TEXT,
  status TEXT NOT NULL DEFAULT 'draft',
  created_at TEXT NOT NULL,
  archived_at TEXT
);

-- Runs
CREATE TABLE IF NOT EXISTS runs (
  id TEXT PRIMARY KEY,
  idea_id TEXT NOT NULL REFERENCES ideas(id),
  status TEXT NOT NULL DEFAULT 'pending',
  workflow_id TEXT NOT NULL,
  workflow_title TEXT NOT NULL,
  workspace_root TEXT NOT NULL,
  artifact_root TEXT NOT NULL,
  started_at TEXT NOT NULL,
  completed_at TEXT,
  cancellation_requested_at TEXT,
  cancellation_settled_at TEXT
);

-- Stage executions
CREATE TABLE IF NOT EXISTS stage_executions (
  id TEXT PRIMARY KEY,
  run_id TEXT NOT NULL REFERENCES runs(id),
  stage_id TEXT NOT NULL,
  label TEXT NOT NULL,
  status TEXT NOT NULL DEFAULT 'pending',
  iteration INTEGER NOT NULL DEFAULT 1,
  attempt_number INTEGER NOT NULL DEFAULT 1,
  settlement_kind TEXT,
  started_at TEXT NOT NULL,
  completed_at TEXT
);

-- Agent executions
CREATE TABLE IF NOT EXISTS agent_executions (
  id TEXT PRIMARY KEY,
  stage_execution_id TEXT NOT NULL REFERENCES stage_executions(id),
  agent_id TEXT NOT NULL,
  provider TEXT NOT NULL,
  model TEXT,
  status TEXT NOT NULL DEFAULT 'running',
  started_at TEXT NOT NULL,
  completed_at TEXT
);

-- Approvals
CREATE TABLE IF NOT EXISTS approvals (
  id TEXT PRIMARY KEY,
  run_id TEXT NOT NULL REFERENCES runs(id),
  stage_id TEXT NOT NULL,
  decision TEXT NOT NULL DEFAULT 'pending',
  requested_at TEXT NOT NULL,
  decided_at TEXT,
  comment TEXT,
  expires_at TEXT,
  repaired_at TEXT
);

-- Artifacts (metadata only; contents on local FS)
CREATE TABLE IF NOT EXISTS artifacts (
  id TEXT PRIMARY KEY,
  run_id TEXT NOT NULL REFERENCES runs(id),
  stage_id TEXT NOT NULL,
  agent_id TEXT NOT NULL,
  name TEXT NOT NULL,
  contract_id TEXT NOT NULL,
  format TEXT NOT NULL,
  file_path TEXT NOT NULL,
  checksum_sha256 TEXT,
  size_bytes INTEGER,
  provider TEXT NOT NULL,
  model TEXT,
  created_at TEXT NOT NULL,
  is_pinned INTEGER NOT NULL DEFAULT 0,
  report_kind TEXT,
  report_version INTEGER
);

-- Work queue (internal background jobs)
CREATE TABLE IF NOT EXISTS work_items (
  id TEXT PRIMARY KEY,
  kind TEXT NOT NULL,
  payload_json TEXT NOT NULL,
  status TEXT NOT NULL DEFAULT 'pending',
  run_id TEXT,
  stage_id TEXT,
  created_at TEXT NOT NULL,
  scheduled_at TEXT NOT NULL,
  started_at TEXT,
  completed_at TEXT,
  failed_at TEXT,
  attempt_count INTEGER NOT NULL DEFAULT 0,
  last_error TEXT
);

-- Command journal (audit + repair)
CREATE TABLE IF NOT EXISTS command_journal (
  id TEXT PRIMARY KEY,
  command_type TEXT NOT NULL,
  payload_json TEXT NOT NULL,
  result_status TEXT NOT NULL DEFAULT 'pending',
  run_id TEXT,
  created_at TEXT NOT NULL,
  completed_at TEXT,
  error TEXT
);

-- Indexes
CREATE INDEX IF NOT EXISTS idx_runs_idea_id ON runs(idea_id);
CREATE INDEX IF NOT EXISTS idx_runs_status ON runs(status);
CREATE INDEX IF NOT EXISTS idx_stage_executions_run_id ON stage_executions(run_id);
CREATE INDEX IF NOT EXISTS idx_approvals_run_id ON approvals(run_id);
CREATE INDEX IF NOT EXISTS idx_approvals_decision ON approvals(decision);
CREATE INDEX IF NOT EXISTS idx_artifacts_run_id ON artifacts(run_id);
CREATE INDEX IF NOT EXISTS idx_work_items_status ON work_items(status);
CREATE INDEX IF NOT EXISTS idx_work_items_run_id ON work_items(run_id);
