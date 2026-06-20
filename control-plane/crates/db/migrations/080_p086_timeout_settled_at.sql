-- P086: Add timeout_settled_at readback field to agent_work_continuations.
--
-- Records the wall-clock time at which the watchdog atomically settled a
-- provider_session_resurrection row into failed_closed via a timeout.
-- NULL for all non-timeout terminal states and all live_handle_continuation rows.
-- Required by the rollout_contract readback surface (p086_timeout_settled_at field).

ALTER TABLE agent_work_continuations
  ADD COLUMN timeout_settled_at TEXT NULL
    CHECK (
      timeout_settled_at IS NULL
      OR mode = 'provider_session_resurrection'
    );
