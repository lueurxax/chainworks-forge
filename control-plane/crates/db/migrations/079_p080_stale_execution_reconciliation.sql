-- P080 Continuous Stale Execution Reconciliation
-- Nine additive tables: helper leases, reconciliation events, epoch tracking,
-- dedup, deferral, iteration cursor, readback heartbeats, watchdog, and
-- rollout control matrix.  All timestamps are RFC3339 UTC, all columns use
-- explicit SQLite type affinity (TEXT / INTEGER), and every CHECK constraint
-- is named so migration tooling can identify it.

-- p080_001: Active helper-process lease registry.
-- Stores the durable binding between a work-item tuple and its helper
-- process group so P080 can verify liveness and reap orphans safely.
CREATE TABLE IF NOT EXISTS p080_helper_leases_v1 (
    id                              TEXT    NOT NULL,
    run_id                          TEXT    NOT NULL,
    stage_id                        TEXT    NOT NULL,
    work_item_id                    TEXT    NOT NULL,
    helper_kind                     TEXT    NOT NULL,
    process_group_id                INTEGER NOT NULL,
    owner_process_id                INTEGER NOT NULL,
    process_start_boottime_nanos    INTEGER NOT NULL,
    acquired_at                     TEXT    NOT NULL,
    created_at                      TEXT    NOT NULL,
    updated_at                      TEXT    NOT NULL,
    agent_execution_id              TEXT    NULL,
    session_generation_id           TEXT    NULL,
    last_heartbeat_at               TEXT    NULL,
    expires_at                      TEXT    NULL,
    terminated_at                   TEXT    NULL,
    termination_reason              TEXT    NULL,
    CONSTRAINT pk_p080_helper_leases PRIMARY KEY (id),
    CONSTRAINT ck_p080_helper_leases_pid_positive
        CHECK (owner_process_id > 0),
    CONSTRAINT ck_p080_helper_leases_pgid_positive
        CHECK (process_group_id > 0),
    CONSTRAINT ck_p080_helper_leases_start_nonnegative
        CHECK (process_start_boottime_nanos >= 0),
    CONSTRAINT ck_p080_helper_leases_termination_reason
        CHECK (
            termination_reason IS NULL OR termination_reason IN (
                'verified_terminated',
                'signal_verification_failed',
                'operator_clear',
                'startup_repair_reclaim',
                'helper_idle_expiry'
            )
        )
);

-- Unique active lease per (helper_kind, process_group_id) — prevents duplicate
-- lease creation from racing writers or replayed inserts.
CREATE UNIQUE INDEX IF NOT EXISTS uq_p080_helper_leases_active_pgid
    ON p080_helper_leases_v1 (helper_kind, process_group_id)
    WHERE terminated_at IS NULL;

CREATE INDEX IF NOT EXISTS ix_p080_helper_leases_tuple
    ON p080_helper_leases_v1 (run_id, stage_id, work_item_id);

CREATE INDEX IF NOT EXISTS ix_p080_helper_leases_expiry
    ON p080_helper_leases_v1 (expires_at)
    WHERE terminated_at IS NULL;

-- p080_002: Members of a helper lease (child processes under a lease).
CREATE TABLE IF NOT EXISTS p080_helper_lease_members_v1 (
    lease_id                            TEXT    NOT NULL,
    member_pid                          INTEGER NOT NULL,
    member_process_start_boottime_nanos INTEGER NOT NULL,
    recorded_parent_pid                 INTEGER NOT NULL,
    acquired_at                         TEXT    NOT NULL,
    created_at                          TEXT    NOT NULL,
    updated_at                          TEXT    NOT NULL,
    terminated_at                       TEXT    NULL,
    CONSTRAINT pk_p080_helper_lease_members
        PRIMARY KEY (lease_id, member_pid, member_process_start_boottime_nanos),
    CONSTRAINT fk_p080_helper_lease_members_lease_id
        FOREIGN KEY (lease_id) REFERENCES p080_helper_leases_v1(id),
    CONSTRAINT ck_p080_helper_lease_members_pid_positive
        CHECK (member_pid > 0),
    CONSTRAINT ck_p080_helper_lease_members_parent_positive
        CHECK (recorded_parent_pid > 0),
    CONSTRAINT ck_p080_helper_lease_members_start_nonnegative
        CHECK (member_process_start_boottime_nanos >= 0)
);

-- At most one live entry per (lease_id, member_pid).
CREATE UNIQUE INDEX IF NOT EXISTS uq_p080_helper_lease_members_live
    ON p080_helper_lease_members_v1 (lease_id, member_pid)
    WHERE terminated_at IS NULL;

CREATE INDEX IF NOT EXISTS ix_p080_helper_lease_members_pid
    ON p080_helper_lease_members_v1 (member_pid, member_process_start_boottime_nanos);

-- p080_003: Immutable append-only reconciliation event log.
-- Each row records one classifier/repair decision for a (run, stage, work_item, stale_class) tuple.
CREATE TABLE IF NOT EXISTS p080_reconciliation_events_v1 (
    id                          TEXT    NOT NULL,
    run_id                      TEXT    NOT NULL,
    stage_id                    TEXT    NOT NULL,
    work_item_id                TEXT    NOT NULL,
    stale_class                 TEXT    NOT NULL,
    repair_action               TEXT    NOT NULL,
    hold_reason                 TEXT    NOT NULL,
    predicate_hash              TEXT    NOT NULL,
    recurrence_epoch            INTEGER NOT NULL,
    decision                    TEXT    NOT NULL,
    event_source                TEXT    NOT NULL,
    created_at                  TEXT    NOT NULL,
    details_json                TEXT    NOT NULL DEFAULT '{}',
    agent_execution_id          TEXT    NULL,
    session_generation_id       TEXT    NULL,
    initiating_principal_id     TEXT    NULL,
    repair_idempotency_key      TEXT    NULL,
    cooldown_until              TEXT    NULL,
    CONSTRAINT pk_p080_reconciliation_events PRIMARY KEY (id),
    CONSTRAINT ck_p080_recon_events_stale_class
        CHECK (stale_class IN (
            'useful', 'warmup_pending', 'acp_startup_stale', 'acp_prompt_stale',
            'scheduler_ownership_drift', 'helper_orphan_drift',
            'release_side_effect_drift', 'ambiguous_owner', 'unknown'
        )),
    CONSTRAINT ck_p080_recon_events_repair_action
        CHECK (repair_action IN (
            'none', 'diagnose_only', 'acp_session_reset', 'scheduler_lease_reclaim',
            'helper_reap', 'delegated_to_p037', 'delegated_to_p076',
            'hold', 'clear_permanent_hold'
        )),
    CONSTRAINT ck_p080_recon_events_hold_reason
        CHECK (hold_reason IN (
            'none', 'cooldown_active', 'permanent_hold_active', 'ambiguous_owner',
            'side_effect_drift_unsafe', 'dependency_read_failure', 'gateway_saturated',
            'live_disable', 'warmup_pending', 'rollout_disabled', 'unknown'
        )),
    CONSTRAINT ck_p080_recon_events_decision
        CHECK (decision IN (
            'diagnosed', 'repaired', 'held', 'race_aborted',
            'delegated', 'cleared', 'already_cleared', 'rejected'
        )),
    CONSTRAINT ck_p080_recon_events_event_source
        CHECK (event_source IN ('live_loop', 'startup_repair', 'operator_request')),
    CONSTRAINT ck_p080_recon_events_details_size
        CHECK (length(details_json) <= 65536),
    CONSTRAINT ck_p080_recon_events_details_json
        CHECK (json_valid(details_json))
);

CREATE INDEX IF NOT EXISTS ix_p080_recon_events_tuple_time
    ON p080_reconciliation_events_v1 (run_id, stage_id, work_item_id, stale_class, created_at);

-- Idempotency: only one repaired event per repair_idempotency_key.
CREATE UNIQUE INDEX IF NOT EXISTS uq_p080_recon_events_repaired_idem
    ON p080_reconciliation_events_v1 (repair_idempotency_key)
    WHERE decision = 'repaired';

CREATE INDEX IF NOT EXISTS ix_p080_recon_events_cooldown
    ON p080_reconciliation_events_v1 (cooldown_until)
    WHERE cooldown_until IS NOT NULL;

-- p080_004: Recurrence epoch counter per (run, stage, work_item, stale_class).
-- Monotonically advances on each repair cycle or projection rebuild.
CREATE TABLE IF NOT EXISTS p080_recurrence_epoch_v1 (
    run_id                  TEXT    NOT NULL,
    stage_id                TEXT    NOT NULL,
    work_item_id            TEXT    NOT NULL,
    stale_class             TEXT    NOT NULL,
    epoch                   INTEGER NOT NULL DEFAULT 0,
    last_advance_reason     TEXT    NOT NULL,
    updated_at              TEXT    NOT NULL,
    CONSTRAINT pk_p080_recurrence_epoch
        PRIMARY KEY (run_id, stage_id, work_item_id, stale_class),
    CONSTRAINT ck_p080_recurrence_epoch_nonnegative
        CHECK (epoch >= 0),
    CONSTRAINT ck_p080_recurrence_epoch_reason
        CHECK (last_advance_reason IN (
            'noop_race_aborted', 'repaired', 'held', 'cleared', 'projection_rebuilt'
        ))
);

-- p080_005: Operator request deduplication store.
-- Principal-scoped; prevents replaying mutating requests with changed context.
CREATE TABLE IF NOT EXISTS p080_operator_request_dedup_v1 (
    principal_id                TEXT    NOT NULL,
    principal_class             TEXT    NOT NULL,
    tool_name                   TEXT    NOT NULL,
    operator_request_dedup_key  TEXT    NOT NULL,
    requested_action            TEXT    NOT NULL,
    auth_policy_generation      INTEGER NOT NULL,
    secret_generation_id        TEXT    NOT NULL,
    rollout_phase               TEXT    NOT NULL,
    repair_class_enabled_hash   TEXT    NOT NULL,
    live_disable_generation     INTEGER NOT NULL,
    request_fingerprint         TEXT    NOT NULL,
    response_json               TEXT    NOT NULL,
    created_at                  TEXT    NOT NULL,
    expires_at                  TEXT    NOT NULL,
    CONSTRAINT pk_p080_operator_request_dedup
        PRIMARY KEY (principal_id, tool_name, operator_request_dedup_key),
    CONSTRAINT ck_p080_dedup_key_size
        CHECK (length(operator_request_dedup_key) <= 128),
    CONSTRAINT ck_p080_dedup_response_json
        CHECK (json_valid(response_json)),
    CONSTRAINT ck_p080_dedup_principal_class
        CHECK (principal_class IN ('operator', 'read_only_operator'))
);

CREATE INDEX IF NOT EXISTS ix_p080_dedup_expires
    ON p080_operator_request_dedup_v1 (expires_at);

-- p080_006: Per-candidate deferral table.
-- Used by the reconciliation loop to back off candidates that hit budgets or
-- gateway saturation.
CREATE TABLE IF NOT EXISTS p080_candidate_deferral_v1 (
    run_id                  TEXT    NOT NULL,
    stage_id                TEXT    NOT NULL,
    work_item_id            TEXT    NOT NULL,
    stale_class             TEXT    NOT NULL,
    deferred_until          TEXT    NOT NULL,
    last_deferral_reason    TEXT    NOT NULL,
    updated_at              TEXT    NOT NULL,
    CONSTRAINT pk_p080_candidate_deferral
        PRIMARY KEY (run_id, stage_id, work_item_id, stale_class),
    CONSTRAINT ck_p080_candidate_deferral_reason
        CHECK (last_deferral_reason IN (
            'per_dependency_read_timeout', 'per_candidate_budget',
            'gateway_saturated', 'live_disable'
        ))
);

CREATE INDEX IF NOT EXISTS ix_p080_candidate_deferral_due
    ON p080_candidate_deferral_v1 (deferred_until);

-- p080_007: Reconciliation iteration cursor.
-- Tracks the current position of the live reconciliation loop across restarts.
CREATE TABLE IF NOT EXISTS p080_iteration_cursor_v1 (
    id              TEXT    NOT NULL,
    cursor_version  INTEGER NOT NULL,
    cursor_json     TEXT    NOT NULL,
    started_at      TEXT    NOT NULL,
    updated_at      TEXT    NOT NULL,
    completed_at    TEXT    NULL,
    CONSTRAINT pk_p080_iteration_cursor PRIMARY KEY (id),
    CONSTRAINT ck_p080_iteration_cursor_json
        CHECK (json_valid(cursor_json)),
    CONSTRAINT ck_p080_iteration_cursor_version_current
        CHECK (cursor_version >= 1)
);

CREATE INDEX IF NOT EXISTS ix_p080_iteration_cursor_latest
    ON p080_iteration_cursor_v1 (updated_at)
    WHERE completed_at IS NULL;

-- p080_008: Durable readback heartbeat projections.
-- One row per (run, stage, work_item, stale_class); updated on each projection
-- tick and verified for integrity on read.
CREATE TABLE IF NOT EXISTS p080_readback_heartbeats_v1 (
    run_id                  TEXT    NOT NULL,
    stage_id                TEXT    NOT NULL,
    work_item_id            TEXT    NOT NULL,
    stale_class             TEXT    NOT NULL,
    projection_generation   INTEGER NOT NULL,
    projection_updated_at   TEXT    NOT NULL,
    projection_integrity    TEXT    NOT NULL,
    readback_json           TEXT    NOT NULL,
    updated_at              TEXT    NOT NULL,
    CONSTRAINT pk_p080_readback_heartbeats
        PRIMARY KEY (run_id, stage_id, work_item_id, stale_class),
    CONSTRAINT ck_p080_readback_json
        CHECK (json_valid(readback_json) AND length(readback_json) <= 65536),
    CONSTRAINT ck_p080_readback_integrity
        CHECK (projection_integrity IN ('valid', 'stale', 'tamper_detected'))
);

CREATE INDEX IF NOT EXISTS ix_p080_readback_order
    ON p080_readback_heartbeats_v1 (projection_updated_at DESC, run_id, stage_id, work_item_id);

-- p080_009: Executor registration watchdog.
-- Tracks whether expected agent executors registered within their startup window.
CREATE TABLE IF NOT EXISTS p080_executor_registration_watchdog_v1 (
    run_id                      TEXT    NOT NULL,
    stage_id                    TEXT    NOT NULL,
    work_item_id                TEXT    NOT NULL,
    agent_execution_id          TEXT    NOT NULL,
    expected_registration_at    TEXT    NOT NULL,
    last_seen_at                TEXT    NOT NULL,
    reregistration_state        TEXT    NOT NULL,
    updated_at                  TEXT    NOT NULL,
    session_generation_id       TEXT    NULL,
    CONSTRAINT pk_p080_executor_registration_watchdog
        PRIMARY KEY (run_id, stage_id, work_item_id, agent_execution_id),
    CONSTRAINT ck_p080_watchdog_state
        CHECK (reregistration_state IN (
            'expected', 'seen', 'missing', 'stale', 'recovered'
        ))
);

CREATE INDEX IF NOT EXISTS ix_p080_watchdog_state_time
    ON p080_executor_registration_watchdog_v1 (reregistration_state, expected_registration_at);

-- Rollout control matrix: one row per feature class.
-- Seeded at daemon start if absent; updated by operator via future mutation tools.
CREATE TABLE IF NOT EXISTS p080_rollout_control_v1 (
    class                       TEXT    NOT NULL,
    enabled                     INTEGER NOT NULL DEFAULT 0,
    phase                       TEXT    NOT NULL,
    generation                  INTEGER NOT NULL,
    updated_at                  TEXT    NOT NULL,
    updated_by_principal_id     TEXT    NOT NULL,
    reason                      TEXT    NOT NULL,
    CONSTRAINT pk_p080_rollout_control PRIMARY KEY (class),
    CONSTRAINT ck_p080_rollout_control_enabled
        CHECK (enabled IN (0, 1)),
    CONSTRAINT ck_p080_rollout_control_class
        CHECK (class IN (
            'acp_startup_stale', 'scheduler_ownership_drift', 'helper_orphan_drift',
            'release_side_effect_drift', 'detection_only', 'live_disable',
            'permanent_hold_clear'
        )),
    CONSTRAINT ck_p080_rollout_control_phase
        CHECK (phase IN (
            'phase_0', 'phase_1', 'phase_2', 'phase_3', 'phase_4', 'phase_5'
        )),
    CONSTRAINT ck_p080_rollout_control_reason
        CHECK (reason IN (
            'phase_promotion', 'phase_rollback', 'live_disable', 'live_enable',
            'startup_seed', 'operator_change'
        ))
);

-- Immutable audit trail for all rollout control changes.
CREATE TABLE IF NOT EXISTS p080_rollout_control_audit_v1 (
    id                  TEXT    NOT NULL,
    class               TEXT    NOT NULL,
    generation_before   INTEGER NOT NULL,
    generation_after    INTEGER NOT NULL,
    enabled_before      INTEGER NOT NULL,
    enabled_after       INTEGER NOT NULL,
    reason              TEXT    NOT NULL,
    updated_by_principal_id TEXT NOT NULL,
    updated_at          TEXT    NOT NULL,
    CONSTRAINT pk_p080_rollout_control_audit PRIMARY KEY (id)
);

CREATE INDEX IF NOT EXISTS ix_p080_rollout_control_audit_class_time
    ON p080_rollout_control_audit_v1 (class, updated_at);
