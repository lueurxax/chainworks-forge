ALTER TABLE session_generations
  ADD COLUMN estimated_input_tokens INTEGER NOT NULL DEFAULT 0;

ALTER TABLE session_generations
  ADD COLUMN last_activity_at TEXT;
