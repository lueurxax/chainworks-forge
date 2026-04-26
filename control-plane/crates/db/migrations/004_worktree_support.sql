-- Worktree support (Proposal 007): track provisioned worktree path and git
-- branch metadata on runs for implementation stages.
ALTER TABLE runs ADD COLUMN worktree_root TEXT;
ALTER TABLE runs ADD COLUMN base_branch TEXT;
ALTER TABLE runs ADD COLUMN base_revision TEXT;
ALTER TABLE runs ADD COLUMN target_branch TEXT;
-- Frozen delivery configuration (Proposal 007): JSON blob frozen at run start.
ALTER TABLE runs ADD COLUMN delivery_configuration_json TEXT;
