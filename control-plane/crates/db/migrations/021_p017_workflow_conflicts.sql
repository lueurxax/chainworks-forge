CREATE TABLE IF NOT EXISTS workflow_conflicts (
  conflict_id TEXT PRIMARY KEY,
  conflict_fingerprint TEXT NOT NULL,
  run_id TEXT NOT NULL REFERENCES runs(id),
  stage_execution_id TEXT REFERENCES stage_executions(id),
  lineage_id TEXT,
  current_state_id TEXT NOT NULL,
  reason TEXT NOT NULL,
  operator_label TEXT NOT NULL,
  status TEXT NOT NULL,
  candidate_transitions_json TEXT NOT NULL,
  candidate_transition_hash TEXT NOT NULL,
  advisory_evidence_refs_json TEXT NOT NULL DEFAULT '[]',
  lead_agent_id TEXT,
  mediation_record_id TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  resolved_at TEXT,
  superseded_by_conflict_id TEXT,
  resolution_record_json TEXT,
  terminal_failure_reason TEXT,
  diagnostic_redaction_tier TEXT NOT NULL,
  record_json TEXT NOT NULL,
  UNIQUE(run_id, current_state_id, conflict_fingerprint)
);

CREATE INDEX IF NOT EXISTS idx_workflow_conflicts_run_id ON workflow_conflicts(run_id);
CREATE INDEX IF NOT EXISTS idx_workflow_conflicts_status ON workflow_conflicts(status);
CREATE INDEX IF NOT EXISTS idx_workflow_conflicts_current_state ON workflow_conflicts(current_state_id);
CREATE INDEX IF NOT EXISTS idx_workflow_conflicts_fingerprint ON workflow_conflicts(conflict_fingerprint);

CREATE TABLE IF NOT EXISTS workflow_advisory_rejections (
  rejection_id TEXT PRIMARY KEY,
  run_id TEXT NOT NULL REFERENCES runs(id),
  stage_execution_id TEXT REFERENCES stage_executions(id),
  lineage_id TEXT,
  current_state_id TEXT NOT NULL,
  selected_transition_id TEXT NOT NULL,
  selected_next_state_id TEXT NOT NULL,
  advisory_next_stage_hint TEXT,
  advisory_next_action TEXT,
  advisory_hint_hash TEXT NOT NULL,
  advisory_hint_provenance_json TEXT NOT NULL,
  graph_membership_result TEXT NOT NULL,
  created_at TEXT NOT NULL,
  record_json TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_workflow_advisory_rejections_run_id ON workflow_advisory_rejections(run_id);
CREATE INDEX IF NOT EXISTS idx_workflow_advisory_rejections_stage_execution_id ON workflow_advisory_rejections(stage_execution_id);
CREATE INDEX IF NOT EXISTS idx_workflow_advisory_rejections_hint_hash ON workflow_advisory_rejections(advisory_hint_hash);
CREATE INDEX IF NOT EXISTS idx_workflow_advisory_rejections_selected_transition ON workflow_advisory_rejections(selected_transition_id);

CREATE TABLE IF NOT EXISTS implementation_handoff_statuses (
  run_id TEXT PRIMARY KEY REFERENCES runs(id),
  current_state_id TEXT NOT NULL,
  task_name TEXT NOT NULL,
  code_writer_start_status TEXT NOT NULL,
  status TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  record_json TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_implementation_handoff_statuses_status ON implementation_handoff_statuses(status);
CREATE INDEX IF NOT EXISTS idx_implementation_handoff_statuses_current_state ON implementation_handoff_statuses(current_state_id);

CREATE TABLE IF NOT EXISTS workflow_transition_cursors (
  run_id TEXT PRIMARY KEY REFERENCES runs(id),
  current_state_id TEXT NOT NULL,
  cursor_status TEXT NOT NULL,
  resume_policy TEXT NOT NULL,
  selected_transition_id TEXT,
  selected_next_state_id TEXT,
  conflict_id TEXT,
  conflict_fingerprint TEXT,
  candidate_transition_hash TEXT,
  terminal_failure_reason TEXT,
  updated_at TEXT NOT NULL,
  record_json TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_workflow_transition_cursors_current_state ON workflow_transition_cursors(current_state_id);
CREATE INDEX IF NOT EXISTS idx_workflow_transition_cursors_status ON workflow_transition_cursors(cursor_status);
CREATE INDEX IF NOT EXISTS idx_workflow_transition_cursors_conflict_fingerprint ON workflow_transition_cursors(conflict_fingerprint);
