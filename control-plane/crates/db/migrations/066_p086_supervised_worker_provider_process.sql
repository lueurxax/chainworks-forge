-- P086: durable provider process binding for continuation worker recovery.
--
-- Existing development databases may already have migration 065 applied before
-- provider process binding was added. Keep this additive migration separate so
-- startup recovery can verify and reap the exact ACP process group after a
-- daemon restart instead of relying on in-memory session handles.

ALTER TABLE supervised_workers_continuation
  ADD COLUMN provider_child_pid INTEGER;

ALTER TABLE supervised_workers_continuation
  ADD COLUMN provider_process_group_id INTEGER;

ALTER TABLE supervised_workers_continuation
  ADD COLUMN provider_process_uid INTEGER;
