-- Proposal 050: Per-run workspace isolation.
-- Each run resolves YAML artifact paths through its own meta root
-- instead of the shared .chainworks/ directory.
ALTER TABLE runs ADD COLUMN chainworks_meta_root TEXT;
