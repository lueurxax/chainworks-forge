-- P079: Contract-Aware Output Repair and Provider Fallback
-- Tables: output_contract_repair_events, output_contract_repair_leases,
--         output_contract_repair_fallback_parent_links.
-- Schema is non-destructive; rollback leaves rows readable via GraphQL/MCP/run-report and Swift DTO.
-- PK is repair_attempt_id (approved appendix). See sqlite_migration_appendix in proposal JSON.

-- One repair evidence row per agent execution that enters the P079 recovery lane.
-- This is the scheduling and readback authority for repair/fallback state.
CREATE TABLE IF NOT EXISTS output_contract_repair_events (
    repair_attempt_id            TEXT NOT NULL PRIMARY KEY,
    schema_version               TEXT NOT NULL DEFAULT 'output_contract_repair.v1',
    -- Run/stage/agent/session identifiers (transport-allocated; not provider-emitted).
    run_id                       TEXT NOT NULL REFERENCES runs(id),
    stage_execution_id           TEXT NOT NULL,
    agent_execution_id           TEXT NOT NULL,
    session_generation_id        TEXT NOT NULL,
    role                         TEXT NOT NULL,
    provider_family              TEXT NOT NULL CHECK(provider_family IN ('codex','claude','gemini','junie','auggie','fixture')),
    adapter_family               TEXT NOT NULL CHECK(adapter_family IN ('codex','claude','gemini','junie','auggie','fixture')),
    required_output_mode         TEXT NOT NULL,
    -- Failure classification.
    initial_failure_class        TEXT NOT NULL CHECK(initial_failure_class IN (
        'no_output_produced','empty_output','missing_required_outputs',
        'invalid_required_outputs','output_contract_mismatch','provider_mode_mismatch'
    )),
    initial_failure_subtype      TEXT,
    -- Top-level status (derived from subobject results + final_output_settlement).
    -- Closed enum: not_attempted | in_progress | recovered | blocked | skipped | cancelled | failed
    status                       TEXT NOT NULL DEFAULT 'in_progress' CHECK(status IN (
        'not_attempted','in_progress','recovered','blocked','skipped','cancelled','failed'
    )),
    -- Presentation category (derived from status + final_output_settlement).
    -- NOT NULL: initial value informational; updated as status changes.
    presentation_category        TEXT NOT NULL DEFAULT 'informational' CHECK(presentation_category IN (
        'informational','recovered','blocked','skipped','failed','cancelled'
    )),
    -- Recommended next action. NOT NULL; closed vocabulary from approved proposal.
    recommended_next_action      TEXT NOT NULL DEFAULT 'continue' CHECK(recommended_next_action IN (
        'continue','inspect_repair_evidence','configure_fallback_policy',
        'operator_resolve_approval','operator_resolve_workflow_conflict',
        'retry_after_transport_restored','cancel_acknowledged','manual_investigation'
    )),
    -- Final output settlement result (approved closed enum — api-contract-r3-001).
    -- NOTE: blocked_unsafe_continuation is NOT an approved value. Unsafe continuation
    -- events settle as blocked_missing_required_outputs with initial_failure_subtype=unsafe_continuation.
    final_output_settlement      TEXT CHECK(final_output_settlement IN (
        'valid_outputs_from_completed_execution',
        'valid_outputs_from_repair',
        'valid_outputs_from_transcript_recovery',
        'valid_outputs_from_provider_envelope',
        'valid_outputs_from_fallback',
        'blocked_missing_required_outputs',
        'blocked_invalid_required_outputs',
        'blocked_provider_mode_mismatch',
        'ignored_late_outputs',
        'cancelled',
        'failed_transport',
        'deadline_exceeded'
    )),
    -- Repair attempt (JSON payload, embedded schema_version).
    same_session_repair_json     TEXT,
    -- Transcript recovery (JSON payload).
    transcript_recovery_json     TEXT,
    -- Provider fallback (JSON payload).
    provider_fallback_json       TEXT,
    -- Provider plan evidence (JSON payload).
    provider_plan_evidence_json  TEXT,
    -- Required output bindings (JSON array). NOT NULL; empty array when no outputs declared.
    required_outputs_json        TEXT NOT NULL DEFAULT '[]',
    -- Permission decisions (JSON array of {method, resource_kind, decision, reason}).
    permission_decisions_json    TEXT NOT NULL DEFAULT '[]',
    -- Budget accounting.
    repair_budget_consumed       INTEGER NOT NULL DEFAULT 0 CHECK(repair_budget_consumed IN (0,1)),
    fallback_budget_consumed     INTEGER NOT NULL DEFAULT 0 CHECK(fallback_budget_consumed IN (0,1)),
    -- Template/parser version pins.
    repair_prompt_template_version TEXT,
    recovery_parser_version        TEXT,
    -- Policy feature flags (JSON array of string flag names). NOT NULL; empty array when no policy.
    policy_feature_flags_json    TEXT NOT NULL DEFAULT '[]',
    -- Evidence artifact path (run-meta-root-relative).
    evidence_artifact_path       TEXT,
    -- Active lease key (nullable; set when a lease is created for this event).
    lease_id                     TEXT,
    -- Monotonic content version; drives Equatable comparison in Swift DTO.
    evidence_version             INTEGER NOT NULL DEFAULT 1,
    -- Projection integrity for materialized evidence artifact.
    projection_integrity         TEXT NOT NULL DEFAULT 'fresh' CHECK(projection_integrity IN ('fresh','stale','permanently_stale')),
    projection_stale_since       TEXT,
    projection_schema_version    TEXT NOT NULL DEFAULT 'output_contract_repair_events_v1',
    -- Consecutive failed rebuild attempts (REL-r2-9 abandonment ceiling: 12).
    projection_rebuild_attempts  INTEGER NOT NULL DEFAULT 0,
    -- Timestamps.
    recorded_at                  TEXT NOT NULL DEFAULT '',
    created_at                   TEXT NOT NULL,
    updated_at                   TEXT NOT NULL,
    -- At most one P079 evidence row per agent execution.
    UNIQUE(agent_execution_id)
);

CREATE INDEX IF NOT EXISTS idx_ocre_run_id
    ON output_contract_repair_events(run_id);

CREATE INDEX IF NOT EXISTS idx_ocre_agent_execution_id
    ON output_contract_repair_events(agent_execution_id);

CREATE INDEX IF NOT EXISTS idx_ocre_status_updated
    ON output_contract_repair_events(status, updated_at);

CREATE INDEX IF NOT EXISTS idx_ocre_projection_stale
    ON output_contract_repair_events(projection_integrity, projection_stale_since)
    WHERE projection_integrity != 'fresh';

-- One lease row per scheduled repair or fallback attempt.
-- Lease key is a deterministic composite hash of
-- (run_id, stage_execution_id, parent_agent_execution_id, schema_version, frozen_fallback_policy_hash).
-- The PK on lease_key provides the single-flight guarantee.
CREATE TABLE IF NOT EXISTS output_contract_repair_leases (
    lease_key                    TEXT NOT NULL PRIMARY KEY,
    schema_version               TEXT NOT NULL DEFAULT 'output_contract_repair_leases_v1',
    repair_event_id              TEXT NOT NULL REFERENCES output_contract_repair_events(repair_attempt_id),
    run_id                       TEXT NOT NULL REFERENCES runs(id),
    stage_execution_id           TEXT NOT NULL,
    parent_agent_execution_id    TEXT NOT NULL,
    -- Lease kind: repair | fallback
    lease_kind                   TEXT NOT NULL CHECK(lease_kind IN ('repair','fallback')),
    -- Lease state machine: reserved -> prompt_sent -> settled
    lease_state                  TEXT NOT NULL DEFAULT 'reserved' CHECK(lease_state IN ('reserved','prompt_sent','settled')),
    -- Result when settled. NULL while lease is active.
    settled_result               TEXT CHECK(settled_result IN (
        'accepted','rejected_invalid','skipped_ineligible','unavailable',
        'failed_transport','deadline_exceeded','cancelled','superseded_ignored',
        'lease_contended','budget_exhausted'
    )),
    -- Reclamation reason when swept by the reconciliation loop.
    -- Full approved proposal vocabulary per LeaseReclamationReason domain enum.
    reclamation_reason           TEXT CHECK(reclamation_reason IN (
        'ttl_expired_reserved','ttl_expired_prompt_sent',
        'cancellation','supersession','principal_revoked'
    )),
    -- Frozen fallback policy hash (for single-flight lease dedup).
    frozen_fallback_policy_hash  TEXT,
    -- Idempotency token minted at lease commit (v4 UUID).
    idempotency_token            TEXT NOT NULL,
    -- Principal binding.
    lease_owner_principal_id     TEXT NOT NULL,
    -- TTL fields (ISO8601 UTC).
    lease_acquired_at            TEXT NOT NULL,
    lease_expires_at             TEXT NOT NULL,
    lease_seconds                INTEGER NOT NULL,
    -- Timestamp when reserved->prompt_sent transition committed (durable-commit invariant REL-r2-1).
    -- NULL while lease is in 'reserved' state.
    dispatch_committed_at        TEXT,
    -- Optimistic-locking version for CAS reclamation.
    version                      INTEGER NOT NULL DEFAULT 0,
    -- Infrastructure retry counter.
    infra_retry_count            INTEGER NOT NULL DEFAULT 0,
    created_at                   TEXT NOT NULL,
    updated_at                   TEXT NOT NULL,
    -- CHECK: dispatch-commit invariant.
    -- prompt_sent lease MUST have idempotency_token set.
    -- (enforced in repo layer before INSERT/UPDATE as well)
    CONSTRAINT prompt_sent_requires_idempotency
        CHECK(lease_state != 'prompt_sent' OR idempotency_token != ''),
    -- CHECK: dispatch_committed_at must be set when state is prompt_sent or settled
    CONSTRAINT dispatch_committed_at_set_on_send
        CHECK(lease_state = 'reserved' OR dispatch_committed_at IS NOT NULL),
    -- Single-flight guarantee: at most one lease per (run, stage, parent, schema, policy_hash, kind).
    -- The lease_key PK is a hash of these components; this explicit UNIQUE enforces the semantic contract
    -- from the approved sqlite_migration_appendix even if the hash scheme changes.
    UNIQUE(run_id, stage_execution_id, parent_agent_execution_id, schema_version, frozen_fallback_policy_hash, lease_kind)
);

CREATE INDEX IF NOT EXISTS idx_ocrl_repair_event_id
    ON output_contract_repair_leases(repair_event_id);

CREATE INDEX IF NOT EXISTS idx_ocrl_run_id
    ON output_contract_repair_leases(run_id);

CREATE INDEX IF NOT EXISTS idx_ocrl_state_expires
    ON output_contract_repair_leases(lease_state, lease_expires_at)
    WHERE lease_state != 'settled';

-- Fallback child -> parent linkage.
-- Stores the parent-child relationship for controlled provider fallback executions.
-- PK is fallback_agent_execution_id (approved sqlite_migration_appendix naming).
-- At most one fallback child per parent agent execution (enforced by UNIQUE constraint).
CREATE TABLE IF NOT EXISTS output_contract_repair_fallback_parent_links (
    fallback_agent_execution_id           TEXT NOT NULL PRIMARY KEY,
    parent_failed_agent_execution_id      TEXT NOT NULL,
    repair_event_id                       TEXT NOT NULL REFERENCES output_contract_repair_events(repair_attempt_id),
    lease_key                             TEXT NOT NULL REFERENCES output_contract_repair_leases(lease_key),
    -- Content-addressed hash of the serialized fallback context packet.
    fallback_packet_hash                  TEXT NOT NULL,
    -- Principal id and capability hash at dispatch.
    fallback_principal_id                 TEXT NOT NULL,
    fallback_principal_capability_hash    TEXT NOT NULL,
    -- Settled result of the fallback child execution.
    fallback_result                       TEXT CHECK(fallback_result IN (
        'accepted','rejected_invalid','unavailable','failed_transport',
        'deadline_exceeded','cancelled','superseded_ignored'
    )),
    created_at                            TEXT NOT NULL,
    updated_at                            TEXT NOT NULL,
    -- At most one fallback child per repair event.
    UNIQUE(repair_event_id),
    -- Reverse lookup: parent -> child (at most one fallback child per parent).
    UNIQUE(parent_failed_agent_execution_id)
);

CREATE INDEX IF NOT EXISTS idx_ocrfpl_parent
    ON output_contract_repair_fallback_parent_links(parent_failed_agent_execution_id);

CREATE INDEX IF NOT EXISTS idx_ocrfpl_fallback
    ON output_contract_repair_fallback_parent_links(fallback_agent_execution_id);
