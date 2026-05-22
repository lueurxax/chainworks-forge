CREATE TABLE IF NOT EXISTS timeline_raw_details (
    handle TEXT PRIMARY KEY,
    run_id TEXT NOT NULL,
    agent_execution_id TEXT,
    session_generation_id TEXT,
    timeline_event_id TEXT NOT NULL,
    raw_detail TEXT NOT NULL,
    raw_detail_bytes INTEGER NOT NULL,
    raw_detail_digest TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'available',
    expires_at TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE INDEX IF NOT EXISTS idx_timeline_raw_details_run_agent
    ON timeline_raw_details(run_id, agent_execution_id);

CREATE INDEX IF NOT EXISTS idx_timeline_raw_details_event_scope
    ON timeline_raw_details(run_id, agent_execution_id, session_generation_id, timeline_event_id);
