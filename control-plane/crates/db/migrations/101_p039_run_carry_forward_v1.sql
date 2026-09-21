-- Logical migration: p039_run_carry_forward_v1. No historical Run or approval rewrite.
CREATE TABLE run_continuations (
    operation_id TEXT PRIMARY KEY NOT NULL,
    source_run_id TEXT NOT NULL REFERENCES runs(id),
    idea_id TEXT NOT NULL REFERENCES ideas(id),
    reserved_successor_run_id TEXT NOT NULL UNIQUE,
    successor_run_id TEXT UNIQUE REFERENCES runs(id),
    caller_fingerprint TEXT NOT NULL,
    caller_request_id TEXT NOT NULL,
    intent_sha256 TEXT NOT NULL CHECK(length(intent_sha256)=64 AND intent_sha256 NOT GLOB '*[^0-9a-f]*'),
    profile TEXT NOT NULL CHECK(profile='implementation_restart_v1'),
    phase TEXT NOT NULL DEFAULT 'preparing' CHECK(phase IN ('preparing','prepared','aborting','needs_reconciliation','activated','aborted')),
    version INTEGER NOT NULL DEFAULT 1 CHECK(version>0),
    generation INTEGER NOT NULL DEFAULT 1 CHECK(generation>0),
    plan_ref TEXT NOT NULL CHECK(length(CAST(plan_ref AS BLOB)) BETWEEN 1 AND 4096),
    plan_sha256 TEXT NOT NULL CHECK(length(plan_sha256)=64 AND plan_sha256 NOT GLOB '*[^0-9a-f]*'),
    target_ref TEXT NOT NULL CHECK(length(CAST(target_ref AS BLOB)) BETWEEN 1 AND 4096),
    source_witness_sha256 TEXT NOT NULL CHECK(length(source_witness_sha256)=64 AND source_witness_sha256 NOT GLOB '*[^0-9a-f]*'),
    manifest_ref TEXT CHECK(length(CAST(manifest_ref AS BLOB)) BETWEEN 1 AND 4096),
    manifest_sha256 TEXT CHECK(length(manifest_sha256)=64 AND manifest_sha256 NOT GLOB '*[^0-9a-f]*'),
    journal_id TEXT NOT NULL REFERENCES command_journal(id),
    holds_json TEXT NOT NULL DEFAULT '[]' CHECK(json_valid(holds_json) AND json_type(holds_json)='array' AND length(CAST(holds_json AS BLOB))<=65536),
    worker_claimed INTEGER NOT NULL DEFAULT 0 CHECK(worker_claimed IN (0,1)),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    deadline_at TEXT NOT NULL,
    UNIQUE(caller_fingerprint,caller_request_id),
    CHECK(source_run_id<>reserved_successor_run_id),
    CHECK((phase='activated' AND successor_run_id=reserved_successor_run_id) OR (phase<>'activated' AND successor_run_id IS NULL)),
    CHECK(phase NOT IN ('prepared','activated') OR (manifest_ref IS NOT NULL AND manifest_sha256 IS NOT NULL))
);
CREATE UNIQUE INDEX run_continuation_source_unique ON run_continuations(source_run_id) WHERE phase<>'aborted';
CREATE UNIQUE INDEX run_continuation_idea_reservation ON run_continuations(idea_id) WHERE phase IN ('preparing','prepared','aborting','needs_reconciliation');
CREATE INDEX run_continuation_phase ON run_continuations(phase,created_at);

-- Original service command results survive source phase changes and replay TTLs.
CREATE TABLE run_continuation_commands (
    caller_fingerprint TEXT NOT NULL,
    caller_request_id TEXT NOT NULL,
    command TEXT NOT NULL CHECK(command IN ('runs.continue_blocked','runs.continuation_activate','runs.continuation_reconcile','runs.continuation_abort')),
    intent_sha256 TEXT NOT NULL CHECK(length(intent_sha256)=64 AND intent_sha256 NOT GLOB '*[^0-9a-f]*'),
    source_run_id TEXT NOT NULL REFERENCES runs(id),
    operation_id TEXT REFERENCES run_continuations(operation_id),
    result_json TEXT NOT NULL CHECK(json_valid(result_json) AND length(CAST(result_json AS BLOB))<=65536),
    created_at TEXT NOT NULL,
    PRIMARY KEY(caller_fingerprint,caller_request_id)
);
CREATE TRIGGER run_continuation_commands_no_update
BEFORE UPDATE ON run_continuation_commands
BEGIN SELECT RAISE(ABORT,'continuation_command_immutable'); END;
CREATE TRIGGER run_continuation_commands_no_delete
BEFORE DELETE ON run_continuation_commands
BEGIN SELECT RAISE(ABORT,'continuation_command_immutable'); END;

CREATE TABLE run_continuation_steps (
    operation_id TEXT NOT NULL REFERENCES run_continuations(operation_id),
    step_key TEXT NOT NULL CHECK(length(step_key) BETWEEN 1 AND 128),
    generation INTEGER NOT NULL CHECK(generation>0),
    status TEXT NOT NULL CHECK(status IN ('planned','dispatching','verified','unknown')),
    intent_json TEXT NOT NULL CHECK(json_valid(intent_json) AND length(CAST(intent_json AS BLOB))<=65536),
    destination_identity TEXT NOT NULL CHECK(length(CAST(destination_identity AS BLOB)) BETWEEN 1 AND 4096),
    receipt_sha256 TEXT CHECK(length(receipt_sha256)=64 AND receipt_sha256 NOT GLOB '*[^0-9a-f]*'),
    updated_at TEXT NOT NULL,
    PRIMARY KEY(operation_id,step_key),
    CHECK(status<>'verified' OR receipt_sha256 IS NOT NULL)
);

CREATE TABLE run_continuation_inputs (
    input_id TEXT PRIMARY KEY NOT NULL,
    operation_id TEXT NOT NULL REFERENCES run_continuations(operation_id),
    ordinal INTEGER NOT NULL CHECK(ordinal>=0 AND ordinal<=128),
    successor_run_id TEXT REFERENCES runs(id),
    role TEXT NOT NULL CHECK(role IN ('execution_seed','reference_only')),
    source_kind TEXT NOT NULL CHECK(source_kind IN ('artifact','carried_input')),
    source_id TEXT NOT NULL,
    source_run_id TEXT NOT NULL REFERENCES runs(id),
    original_artifact_id TEXT NOT NULL REFERENCES artifacts(id),
    source_schema TEXT NOT NULL CHECK(length(source_schema) BETWEEN 1 AND 256),
    content_sha256 TEXT NOT NULL CHECK(length(content_sha256)=64 AND content_sha256 NOT GLOB '*[^0-9a-f]*'),
    target_logical_name TEXT NOT NULL CHECK(length(target_logical_name) BETWEEN 1 AND 256),
    target_relative_path TEXT NOT NULL CHECK(length(CAST(target_relative_path AS BLOB)) BETWEEN 1 AND 4096),
    installed INTEGER NOT NULL DEFAULT 0 CHECK(installed IN (0,1)),
    historical_relation_json TEXT NOT NULL CHECK(json_valid(historical_relation_json) AND length(CAST(historical_relation_json AS BLOB))<=65536),
    UNIQUE(operation_id,ordinal),
    UNIQUE(operation_id,target_relative_path),
    CHECK((installed=0 AND successor_run_id IS NULL) OR (installed=1 AND successor_run_id IS NOT NULL))
);

CREATE UNIQUE INDEX run_continuation_input_seed ON run_continuation_inputs(operation_id)
    WHERE role='execution_seed';

CREATE TABLE run_execution_fences (
    source_run_id TEXT PRIMARY KEY NOT NULL REFERENCES runs(id),
    operation_id TEXT NOT NULL UNIQUE REFERENCES run_continuations(operation_id),
    generation INTEGER NOT NULL CHECK(generation>0),
    disposition TEXT NOT NULL CHECK(disposition IN ('reserved','historical')),
    created_at TEXT NOT NULL
);

CREATE TABLE run_continuation_approval_bindings (
    approval_id TEXT PRIMARY KEY NOT NULL REFERENCES approvals(id),
    successor_run_id TEXT NOT NULL REFERENCES runs(id),
    stage_execution_id TEXT NOT NULL REFERENCES stage_executions(id),
    logical_stage_id TEXT NOT NULL,
    evidence_json TEXT NOT NULL CHECK(json_valid(evidence_json) AND length(CAST(evidence_json AS BLOB))<=65536),
    evidence_sha256 TEXT NOT NULL CHECK(length(evidence_sha256)=64 AND evidence_sha256 NOT GLOB '*[^0-9a-f]*'),
    generation INTEGER NOT NULL CHECK(generation>0),
    status TEXT NOT NULL CHECK(status IN ('current','superseded')),
    consumed_transition_id TEXT,
    snapshot_artifact_id TEXT REFERENCES artifacts(id),
    first_execution_id TEXT REFERENCES agent_executions(id),
    first_execution_started_at TEXT,
    CHECK(snapshot_artifact_id IS NULL OR consumed_transition_id IS NOT NULL),
    CHECK(first_execution_id IS NULL OR snapshot_artifact_id IS NOT NULL),
    CHECK(first_execution_started_at IS NULL OR first_execution_id IS NOT NULL),
    UNIQUE(successor_run_id,logical_stage_id,generation)
);
CREATE UNIQUE INDEX run_continuation_binding_current ON run_continuation_approval_bindings(successor_run_id,logical_stage_id) WHERE status='current';

CREATE TRIGGER run_continuation_identity_immutable
BEFORE UPDATE OF operation_id,source_run_id,idea_id,reserved_successor_run_id,caller_fingerprint,caller_request_id,intent_sha256,profile,plan_ref,plan_sha256,target_ref,source_witness_sha256,journal_id,created_at,deadline_at ON run_continuations
BEGIN SELECT RAISE(ABORT,'continuation_identity_immutable'); END;

CREATE TRIGGER run_continuation_transition
BEFORE UPDATE ON run_continuations WHEN
    NEW.version<>OLD.version+1 OR NEW.generation<OLD.generation OR
    NOT (
        (OLD.phase='preparing' AND NEW.phase IN ('preparing','prepared','needs_reconciliation','aborting')) OR
        (OLD.phase='prepared' AND NEW.phase IN ('activated','needs_reconciliation','aborting')) OR
        (OLD.phase='needs_reconciliation' AND NEW.phase IN ('needs_reconciliation','prepared','aborting')) OR
        (OLD.phase='aborting' AND NEW.phase IN ('aborting','aborted'))
    ) OR
    (NEW.phase='aborting' AND OLD.phase<>'aborting' AND NEW.generation<>OLD.generation+1) OR
    (NEW.phase='aborted' AND (NEW.worker_claimed<>0 OR EXISTS (
        SELECT 1 FROM run_continuation_steps s WHERE s.operation_id=NEW.operation_id AND s.status IN ('dispatching','unknown'))))
BEGIN SELECT RAISE(ABORT,'continuation_invalid_transition'); END;

CREATE TRIGGER run_continuation_fence_owner
BEFORE INSERT ON run_execution_fences WHEN NOT EXISTS (
    SELECT 1 FROM run_continuations c JOIN runs r ON r.id=c.source_run_id
    WHERE c.operation_id=NEW.operation_id AND c.source_run_id=NEW.source_run_id
      AND c.generation=NEW.generation AND c.phase='preparing'
      AND r.idea_id=c.idea_id AND r.status='blocked' AND NEW.disposition='reserved')
BEGIN SELECT RAISE(ABORT,'continuation_invalid_fence'); END;

-- REPLACE otherwise deletes the conflicting row without running its DELETE
-- trigger when recursive_triggers is disabled on a connection.
CREATE TRIGGER run_continuation_fence_no_replace
BEFORE INSERT ON run_execution_fences WHEN EXISTS (
    SELECT 1 FROM run_execution_fences
    WHERE source_run_id=NEW.source_run_id OR operation_id=NEW.operation_id)
BEGIN SELECT RAISE(ABORT,'continuation_fence_unsettled'); END;

CREATE TRIGGER run_continuation_fence_release
BEFORE DELETE ON run_execution_fences WHEN NOT EXISTS (
    SELECT 1 FROM run_continuations c WHERE c.operation_id=OLD.operation_id AND c.phase='aborted')
BEGIN SELECT RAISE(ABORT,'continuation_fence_unsettled'); END;

CREATE TRIGGER run_continuation_fence_update
BEFORE UPDATE ON run_execution_fences WHEN
    NEW.source_run_id IS NOT OLD.source_run_id OR NEW.operation_id IS NOT OLD.operation_id OR
    NEW.created_at IS NOT OLD.created_at OR OLD.disposition<>'reserved' OR NEW.disposition<>'historical' OR
    NEW.generation<OLD.generation OR NOT EXISTS (
        SELECT 1 FROM run_continuations c WHERE c.operation_id=OLD.operation_id
          AND c.source_run_id=OLD.source_run_id AND c.phase='prepared'
          AND c.worker_claimed=0 AND c.generation=NEW.generation)
BEGIN SELECT RAISE(ABORT,'continuation_invalid_fence'); END;

CREATE TRIGGER run_continuation_binding_identity
BEFORE UPDATE OF approval_id,successor_run_id,stage_execution_id,logical_stage_id,evidence_json,evidence_sha256,generation ON run_continuation_approval_bindings
BEGIN SELECT RAISE(ABORT,'continuation_binding_immutable'); END;

CREATE TRIGGER run_continuation_binding_transition
BEFORE UPDATE ON run_continuation_approval_bindings WHEN
    (NEW.status IS NOT OLD.status AND NOT (OLD.status='current' AND NEW.status='superseded')) OR
    (NEW.consumed_transition_id IS NOT OLD.consumed_transition_id AND NOT (
        OLD.status='current' AND NEW.status='current' AND OLD.consumed_transition_id IS NULL
        AND NEW.consumed_transition_id IS NOT NULL AND length(NEW.consumed_transition_id)>0))
BEGIN SELECT RAISE(ABORT,'continuation_binding_invalid_transition'); END;

CREATE TRIGGER run_continuation_binding_retention
BEFORE DELETE ON run_continuation_approval_bindings
BEGIN SELECT RAISE(ABORT,'continuation_binding_immutable'); END;

CREATE TRIGGER run_continuation_binding_no_replace
BEFORE INSERT ON run_continuation_approval_bindings WHEN EXISTS (
    SELECT 1 FROM run_continuation_approval_bindings b WHERE b.approval_id=NEW.approval_id OR
        (b.successor_run_id=NEW.successor_run_id AND b.logical_stage_id=NEW.logical_stage_id
            AND (b.generation=NEW.generation OR (b.status='current' AND NEW.status='current'))))
BEGIN SELECT RAISE(ABORT,'continuation_binding_immutable'); END;

CREATE TRIGGER run_continuation_step_identity
BEFORE UPDATE OF operation_id,step_key,intent_json,destination_identity ON run_continuation_steps
BEGIN SELECT RAISE(ABORT,'continuation_step_identity_immutable'); END;

CREATE TRIGGER run_continuation_step_insert_guard
BEFORE INSERT ON run_continuation_steps WHEN NOT EXISTS (
    SELECT 1 FROM run_continuations c WHERE c.operation_id=NEW.operation_id
      AND c.generation=NEW.generation AND c.phase='preparing')
BEGIN SELECT RAISE(ABORT,'continuation_stale_worker'); END;

CREATE TRIGGER run_continuation_step_update_guard
BEFORE UPDATE ON run_continuation_steps WHEN
    OLD.status='verified' OR NOT (
        (OLD.status='planned' AND NEW.status='dispatching') OR
        (OLD.status='dispatching' AND NEW.status IN ('verified','unknown')) OR
        (OLD.status='unknown' AND NEW.status='verified')
    ) OR (NEW.status<>'unknown' AND NOT EXISTS (
        SELECT 1 FROM run_continuations c WHERE c.operation_id=NEW.operation_id
          AND c.generation=NEW.generation AND c.phase IN ('preparing','needs_reconciliation','aborting')))
BEGIN SELECT RAISE(ABORT,'continuation_stale_worker'); END;

-- The same SQLite write lock serializes StartRun and continuation reservation.
-- Reservation checks all competing runs before its first operation is inserted.
-- Ideas without P039 history retain their existing Run insertion semantics.
CREATE TRIGGER run_continuation_start_guard
BEFORE INSERT ON runs WHEN
EXISTS (SELECT 1 FROM run_execution_fences WHERE source_run_id=NEW.id) OR (
NEW.status NOT IN ('completed','failed','cancelled')
AND EXISTS (SELECT 1 FROM run_continuations WHERE idea_id=NEW.idea_id) AND (
    EXISTS (SELECT 1 FROM runs r WHERE r.idea_id=NEW.idea_id
        AND r.status NOT IN ('completed','failed','cancelled')
        AND NOT EXISTS (SELECT 1 FROM run_execution_fences f WHERE f.source_run_id=r.id)) OR
    EXISTS (SELECT 1 FROM run_continuations c WHERE c.idea_id=NEW.idea_id
        AND c.phase IN ('preparing','prepared','aborting','needs_reconciliation')
        AND NOT (c.phase='prepared' AND c.reserved_successor_run_id=NEW.id
            AND EXISTS (SELECT 1 FROM run_execution_fences f WHERE f.operation_id=c.operation_id AND f.disposition='historical')))
))
BEGIN SELECT CASE WHEN EXISTS (
    SELECT 1 FROM run_execution_fences WHERE source_run_id=NEW.id AND disposition='historical')
    THEN RAISE(ABORT,'source_continued') ELSE RAISE(ABORT,'continuation_in_progress') END; END;

-- CF-06 durable source guards. Readback SELECTs and operation-owned P039 journals
-- are not execution authority. Unknown active ownership holds while a fence exists.
-- Mediation owner_id is a mediation key, never a run key.
CREATE VIEW p039_agent_run_owners AS
SELECT a.id AS agent_execution_id,s.run_id FROM agent_executions a JOIN stage_executions s
  ON s.id=a.stage_execution_id OR s.id=a.origin_stage_execution_id
    OR (a.owner_kind='stage_execution' AND s.id=a.owner_id)
UNION SELECT a.id,m.run_id FROM agent_executions a JOIN lead_conflict_mediations m
  ON m.id=a.lead_mediation_record_id OR (a.owner_kind='lead_conflict_mediation' AND m.id=a.owner_id)
UNION SELECT a.id,l.run_id FROM agent_executions a JOIN session_lineages l ON l.id=a.session_lineage_id
UNION SELECT a.id,l.run_id FROM agent_executions a JOIN session_generations g ON g.id=a.session_generation_id
  JOIN session_lineages l ON l.id=g.lineage_id;

CREATE VIEW p039_run_resources AS
SELECT id AS run_id,COALESCE(NULLIF(worktree_root,''),workspace_root) AS resource_key FROM runs
UNION SELECT run_id,worktree_resource_key FROM work_items
UNION SELECT CASE WHEN json_valid(payload_json) THEN json_extract(payload_json,'$.run_id') END,worktree_resource_key FROM work_items
UNION SELECT run_id,worktree_resource_key FROM background_leases
UNION SELECT run_id,worktree_resource_key FROM worktree_mutation_barriers;

CREATE VIEW p039_run_resource_peers AS
SELECT id AS source_run_id,id AS run_id FROM runs
UNION SELECT q.run_id,r.id FROM p039_run_resources q JOIN runs r
  ON COALESCE(NULLIF(r.worktree_root,''),r.workspace_root)=q.resource_key
WHERE q.resource_key IS NOT NULL AND q.resource_key<>'';

-- INSERT also checks existing conflicting identities: SQLite REPLACE can skip
-- DELETE triggers with recursive_triggers=OFF. UPDATE checks both OLD and NEW.

CREATE TRIGGER p039_guard_runs_update
BEFORE UPDATE ON runs
WHEN EXISTS (SELECT 1 FROM run_execution_fences)
BEGIN
    SELECT CASE (SELECT f.disposition FROM run_execution_fences f
        WHERE ((f.source_run_id=OLD.id) OR (f.source_run_id IN (SELECT run_id FROM p039_run_resources WHERE resource_key=COALESCE(NULLIF(OLD.worktree_root,''),OLD.workspace_root) AND resource_key<>''))) OR ((f.source_run_id=NEW.id) OR (f.source_run_id IN (SELECT run_id FROM p039_run_resources WHERE resource_key=COALESCE(NULLIF(NEW.worktree_root,''),NEW.workspace_root) AND resource_key<>'')))
        ORDER BY f.disposition LIMIT 1)
      WHEN 'historical' THEN RAISE(ABORT,'source_continued')
      WHEN 'reserved' THEN RAISE(ABORT,'continuation_in_progress') END;
END;

CREATE TRIGGER p039_guard_runs_delete
BEFORE DELETE ON runs
WHEN EXISTS (SELECT 1 FROM run_execution_fences)
BEGIN
    SELECT CASE (SELECT f.disposition FROM run_execution_fences f
        WHERE (f.source_run_id=OLD.id) OR (f.source_run_id IN (SELECT run_id FROM p039_run_resources WHERE resource_key=COALESCE(NULLIF(OLD.worktree_root,''),OLD.workspace_root) AND resource_key<>''))
        ORDER BY f.disposition LIMIT 1)
      WHEN 'historical' THEN RAISE(ABORT,'source_continued')
      WHEN 'reserved' THEN RAISE(ABORT,'continuation_in_progress') END;
END;

CREATE TRIGGER p039_guard_approvals_insert
BEFORE INSERT ON approvals
WHEN EXISTS (SELECT 1 FROM run_execution_fences)
BEGIN
    SELECT CASE (SELECT f.disposition FROM run_execution_fences f
        WHERE (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=NEW.run_id)) OR (EXISTS (SELECT 1 FROM approvals stored WHERE ((stored.id=NEW.id)) AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=stored.run_id))))
        ORDER BY f.disposition LIMIT 1)
      WHEN 'historical' THEN RAISE(ABORT,'source_continued')
      WHEN 'reserved' THEN RAISE(ABORT,'continuation_in_progress') END;
END;

CREATE TRIGGER p039_guard_approvals_update
BEFORE UPDATE ON approvals
WHEN EXISTS (SELECT 1 FROM run_execution_fences)
BEGIN
    SELECT CASE (SELECT f.disposition FROM run_execution_fences f
        WHERE (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=OLD.run_id)) OR (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=NEW.run_id))
        ORDER BY f.disposition LIMIT 1)
      WHEN 'historical' THEN RAISE(ABORT,'source_continued')
      WHEN 'reserved' THEN RAISE(ABORT,'continuation_in_progress') END;
END;

CREATE TRIGGER p039_guard_approvals_delete
BEFORE DELETE ON approvals
WHEN EXISTS (SELECT 1 FROM run_execution_fences)
BEGIN
    SELECT CASE (SELECT f.disposition FROM run_execution_fences f
        WHERE f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=OLD.run_id)
        ORDER BY f.disposition LIMIT 1)
      WHEN 'historical' THEN RAISE(ABORT,'source_continued')
      WHEN 'reserved' THEN RAISE(ABORT,'continuation_in_progress') END;
END;

CREATE TRIGGER p039_guard_workflow_transition_cursors_insert
BEFORE INSERT ON workflow_transition_cursors
WHEN EXISTS (SELECT 1 FROM run_execution_fences)
BEGIN
    SELECT CASE (SELECT f.disposition FROM run_execution_fences f
        WHERE (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=NEW.run_id)) OR (EXISTS (SELECT 1 FROM workflow_transition_cursors stored WHERE ((stored.run_id=NEW.run_id)) AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=stored.run_id))))
        ORDER BY f.disposition LIMIT 1)
      WHEN 'historical' THEN RAISE(ABORT,'source_continued')
      WHEN 'reserved' THEN RAISE(ABORT,'continuation_in_progress') END;
END;

CREATE TRIGGER p039_guard_workflow_transition_cursors_update
BEFORE UPDATE ON workflow_transition_cursors
WHEN EXISTS (SELECT 1 FROM run_execution_fences)
BEGIN
    SELECT CASE (SELECT f.disposition FROM run_execution_fences f
        WHERE (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=OLD.run_id)) OR (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=NEW.run_id))
        ORDER BY f.disposition LIMIT 1)
      WHEN 'historical' THEN RAISE(ABORT,'source_continued')
      WHEN 'reserved' THEN RAISE(ABORT,'continuation_in_progress') END;
END;

CREATE TRIGGER p039_guard_workflow_transition_cursors_delete
BEFORE DELETE ON workflow_transition_cursors
WHEN EXISTS (SELECT 1 FROM run_execution_fences)
BEGIN
    SELECT CASE (SELECT f.disposition FROM run_execution_fences f
        WHERE f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=OLD.run_id)
        ORDER BY f.disposition LIMIT 1)
      WHEN 'historical' THEN RAISE(ABORT,'source_continued')
      WHEN 'reserved' THEN RAISE(ABORT,'continuation_in_progress') END;
END;

CREATE TRIGGER p039_guard_implementation_handoff_statuses_insert
BEFORE INSERT ON implementation_handoff_statuses
WHEN EXISTS (SELECT 1 FROM run_execution_fences)
BEGIN
    SELECT CASE (SELECT f.disposition FROM run_execution_fences f
        WHERE (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=NEW.run_id)) OR (EXISTS (SELECT 1 FROM implementation_handoff_statuses stored WHERE ((stored.run_id=NEW.run_id)) AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=stored.run_id))))
        ORDER BY f.disposition LIMIT 1)
      WHEN 'historical' THEN RAISE(ABORT,'source_continued')
      WHEN 'reserved' THEN RAISE(ABORT,'continuation_in_progress') END;
END;

CREATE TRIGGER p039_guard_implementation_handoff_statuses_update
BEFORE UPDATE ON implementation_handoff_statuses
WHEN EXISTS (SELECT 1 FROM run_execution_fences)
BEGIN
    SELECT CASE (SELECT f.disposition FROM run_execution_fences f
        WHERE (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=OLD.run_id)) OR (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=NEW.run_id))
        ORDER BY f.disposition LIMIT 1)
      WHEN 'historical' THEN RAISE(ABORT,'source_continued')
      WHEN 'reserved' THEN RAISE(ABORT,'continuation_in_progress') END;
END;

CREATE TRIGGER p039_guard_implementation_handoff_statuses_delete
BEFORE DELETE ON implementation_handoff_statuses
WHEN EXISTS (SELECT 1 FROM run_execution_fences)
BEGIN
    SELECT CASE (SELECT f.disposition FROM run_execution_fences f
        WHERE f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=OLD.run_id)
        ORDER BY f.disposition LIMIT 1)
      WHEN 'historical' THEN RAISE(ABORT,'source_continued')
      WHEN 'reserved' THEN RAISE(ABORT,'continuation_in_progress') END;
END;

CREATE TRIGGER p039_guard_artifact_contract_overrides_insert
BEFORE INSERT ON artifact_contract_overrides
WHEN EXISTS (SELECT 1 FROM run_execution_fences)
BEGIN
    SELECT CASE (SELECT f.disposition FROM run_execution_fences f
        WHERE (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=NEW.run_id)) OR (EXISTS (SELECT 1 FROM artifact_contract_overrides stored WHERE ((stored.override_id=NEW.override_id)) AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=stored.run_id))))
        ORDER BY f.disposition LIMIT 1)
      WHEN 'historical' THEN RAISE(ABORT,'source_continued')
      WHEN 'reserved' THEN RAISE(ABORT,'continuation_in_progress') END;
END;

CREATE TRIGGER p039_guard_artifact_contract_overrides_update
BEFORE UPDATE ON artifact_contract_overrides
WHEN EXISTS (SELECT 1 FROM run_execution_fences)
BEGIN
    SELECT CASE (SELECT f.disposition FROM run_execution_fences f
        WHERE (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=OLD.run_id)) OR (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=NEW.run_id))
        ORDER BY f.disposition LIMIT 1)
      WHEN 'historical' THEN RAISE(ABORT,'source_continued')
      WHEN 'reserved' THEN RAISE(ABORT,'continuation_in_progress') END;
END;

CREATE TRIGGER p039_guard_artifact_contract_overrides_delete
BEFORE DELETE ON artifact_contract_overrides
WHEN EXISTS (SELECT 1 FROM run_execution_fences)
BEGIN
    SELECT CASE (SELECT f.disposition FROM run_execution_fences f
        WHERE f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=OLD.run_id)
        ORDER BY f.disposition LIMIT 1)
      WHEN 'historical' THEN RAISE(ABORT,'source_continued')
      WHEN 'reserved' THEN RAISE(ABORT,'continuation_in_progress') END;
END;

CREATE TRIGGER p039_guard_artifacts_insert
BEFORE INSERT ON artifacts
WHEN EXISTS (SELECT 1 FROM run_execution_fences)
BEGIN
    SELECT CASE (SELECT f.disposition FROM run_execution_fences f
        WHERE ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=NEW.run_id)) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o1 WHERE o1.agent_execution_id=NEW.agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o1.run_id))))) OR (EXISTS (SELECT 1 FROM artifacts stored WHERE ((stored.id=NEW.id)) AND ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=stored.run_id)) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o2 WHERE o2.agent_execution_id=stored.agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o2.run_id)))))))
        ORDER BY f.disposition LIMIT 1)
      WHEN 'historical' THEN RAISE(ABORT,'source_continued')
      WHEN 'reserved' THEN RAISE(ABORT,'continuation_in_progress') END;
END;

CREATE TRIGGER p039_guard_artifacts_update
BEFORE UPDATE ON artifacts
WHEN EXISTS (SELECT 1 FROM run_execution_fences)
BEGIN
    SELECT CASE (SELECT f.disposition FROM run_execution_fences f
        WHERE ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=OLD.run_id)) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o3 WHERE o3.agent_execution_id=OLD.agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o3.run_id))))) OR ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=NEW.run_id)) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o4 WHERE o4.agent_execution_id=NEW.agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o4.run_id)))))
        ORDER BY f.disposition LIMIT 1)
      WHEN 'historical' THEN RAISE(ABORT,'source_continued')
      WHEN 'reserved' THEN RAISE(ABORT,'continuation_in_progress') END;
END;

CREATE TRIGGER p039_guard_artifacts_delete
BEFORE DELETE ON artifacts
WHEN EXISTS (SELECT 1 FROM run_execution_fences)
BEGIN
    SELECT CASE (SELECT f.disposition FROM run_execution_fences f
        WHERE (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=OLD.run_id)) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o5 WHERE o5.agent_execution_id=OLD.agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o5.run_id))))
        ORDER BY f.disposition LIMIT 1)
      WHEN 'historical' THEN RAISE(ABORT,'source_continued')
      WHEN 'reserved' THEN RAISE(ABORT,'continuation_in_progress') END;
END;

CREATE TRIGGER p039_guard_stage_executions_insert
BEFORE INSERT ON stage_executions
WHEN EXISTS (SELECT 1 FROM run_execution_fences)
BEGIN
    SELECT CASE (SELECT f.disposition FROM run_execution_fences f
        WHERE (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=NEW.run_id)) OR (EXISTS (SELECT 1 FROM stage_executions stored WHERE ((stored.id=NEW.id)) AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=stored.run_id))))
        ORDER BY f.disposition LIMIT 1)
      WHEN 'historical' THEN RAISE(ABORT,'source_continued')
      WHEN 'reserved' THEN RAISE(ABORT,'continuation_in_progress') END;
END;

CREATE TRIGGER p039_guard_stage_executions_update
BEFORE UPDATE ON stage_executions
WHEN EXISTS (SELECT 1 FROM run_execution_fences)
BEGIN
    SELECT CASE (SELECT f.disposition FROM run_execution_fences f
        WHERE (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=OLD.run_id)) OR (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=NEW.run_id))
        ORDER BY f.disposition LIMIT 1)
      WHEN 'historical' THEN RAISE(ABORT,'source_continued')
      WHEN 'reserved' THEN RAISE(ABORT,'continuation_in_progress') END;
END;

CREATE TRIGGER p039_guard_stage_executions_delete
BEFORE DELETE ON stage_executions
WHEN EXISTS (SELECT 1 FROM run_execution_fences)
BEGIN
    SELECT CASE (SELECT f.disposition FROM run_execution_fences f
        WHERE f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=OLD.run_id)
        ORDER BY f.disposition LIMIT 1)
      WHEN 'historical' THEN RAISE(ABORT,'source_continued')
      WHEN 'reserved' THEN RAISE(ABORT,'continuation_in_progress') END;
END;

CREATE TRIGGER p039_guard_session_lineages_insert
BEFORE INSERT ON session_lineages
WHEN EXISTS (SELECT 1 FROM run_execution_fences)
BEGIN
    SELECT CASE (SELECT f.disposition FROM run_execution_fences f
        WHERE (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=NEW.run_id)) OR (EXISTS (SELECT 1 FROM session_lineages stored WHERE ((stored.id=NEW.id)) AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=stored.run_id))))
        ORDER BY f.disposition LIMIT 1)
      WHEN 'historical' THEN RAISE(ABORT,'source_continued')
      WHEN 'reserved' THEN RAISE(ABORT,'continuation_in_progress') END;
END;

CREATE TRIGGER p039_guard_session_lineages_update
BEFORE UPDATE ON session_lineages
WHEN EXISTS (SELECT 1 FROM run_execution_fences)
BEGIN
    SELECT CASE (SELECT f.disposition FROM run_execution_fences f
        WHERE (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=OLD.run_id)) OR (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=NEW.run_id))
        ORDER BY f.disposition LIMIT 1)
      WHEN 'historical' THEN RAISE(ABORT,'source_continued')
      WHEN 'reserved' THEN RAISE(ABORT,'continuation_in_progress') END;
END;

CREATE TRIGGER p039_guard_session_lineages_delete
BEFORE DELETE ON session_lineages
WHEN EXISTS (SELECT 1 FROM run_execution_fences)
BEGIN
    SELECT CASE (SELECT f.disposition FROM run_execution_fences f
        WHERE f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=OLD.run_id)
        ORDER BY f.disposition LIMIT 1)
      WHEN 'historical' THEN RAISE(ABORT,'source_continued')
      WHEN 'reserved' THEN RAISE(ABORT,'continuation_in_progress') END;
END;

CREATE TRIGGER p039_guard_lead_conflict_mediations_insert
BEFORE INSERT ON lead_conflict_mediations
WHEN EXISTS (SELECT 1 FROM run_execution_fences)
BEGIN
    SELECT CASE (SELECT f.disposition FROM run_execution_fences f
        WHERE ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=NEW.run_id)) OR (EXISTS (SELECT 1 FROM workflow_conflicts o6 WHERE o6.conflict_id=NEW.conflict_id AND ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o6.run_id)) OR (EXISTS (SELECT 1 FROM stage_executions o7 WHERE o7.id=o6.stage_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o7.run_id)))))))) OR (EXISTS (SELECT 1 FROM lead_conflict_mediations stored WHERE ((stored.id=NEW.id)) AND ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=stored.run_id)) OR (EXISTS (SELECT 1 FROM workflow_conflicts o8 WHERE o8.conflict_id=stored.conflict_id AND ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o8.run_id)) OR (EXISTS (SELECT 1 FROM stage_executions o9 WHERE o9.id=o8.stage_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o9.run_id))))))))))
        ORDER BY f.disposition LIMIT 1)
      WHEN 'historical' THEN RAISE(ABORT,'source_continued')
      WHEN 'reserved' THEN RAISE(ABORT,'continuation_in_progress') END;
END;

CREATE TRIGGER p039_guard_lead_conflict_mediations_update
BEFORE UPDATE ON lead_conflict_mediations
WHEN EXISTS (SELECT 1 FROM run_execution_fences)
BEGIN
    SELECT CASE (SELECT f.disposition FROM run_execution_fences f
        WHERE ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=OLD.run_id)) OR (EXISTS (SELECT 1 FROM workflow_conflicts o10 WHERE o10.conflict_id=OLD.conflict_id AND ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o10.run_id)) OR (EXISTS (SELECT 1 FROM stage_executions o11 WHERE o11.id=o10.stage_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o11.run_id)))))))) OR ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=NEW.run_id)) OR (EXISTS (SELECT 1 FROM workflow_conflicts o12 WHERE o12.conflict_id=NEW.conflict_id AND ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o12.run_id)) OR (EXISTS (SELECT 1 FROM stage_executions o13 WHERE o13.id=o12.stage_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o13.run_id))))))))
        ORDER BY f.disposition LIMIT 1)
      WHEN 'historical' THEN RAISE(ABORT,'source_continued')
      WHEN 'reserved' THEN RAISE(ABORT,'continuation_in_progress') END;
END;

CREATE TRIGGER p039_guard_lead_conflict_mediations_delete
BEFORE DELETE ON lead_conflict_mediations
WHEN EXISTS (SELECT 1 FROM run_execution_fences)
BEGIN
    SELECT CASE (SELECT f.disposition FROM run_execution_fences f
        WHERE (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=OLD.run_id)) OR (EXISTS (SELECT 1 FROM workflow_conflicts o14 WHERE o14.conflict_id=OLD.conflict_id AND ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o14.run_id)) OR (EXISTS (SELECT 1 FROM stage_executions o15 WHERE o15.id=o14.stage_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o15.run_id)))))))
        ORDER BY f.disposition LIMIT 1)
      WHEN 'historical' THEN RAISE(ABORT,'source_continued')
      WHEN 'reserved' THEN RAISE(ABORT,'continuation_in_progress') END;
END;

CREATE TRIGGER p039_guard_workflow_conflicts_insert
BEFORE INSERT ON workflow_conflicts
WHEN EXISTS (SELECT 1 FROM run_execution_fences)
BEGIN
    SELECT CASE (SELECT f.disposition FROM run_execution_fences f
        WHERE ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=NEW.run_id)) OR (EXISTS (SELECT 1 FROM stage_executions o16 WHERE o16.id=NEW.stage_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o16.run_id)))) OR (EXISTS (SELECT 1 FROM lead_conflict_mediations o17 WHERE o17.id=NEW.mediation_record_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o17.run_id))))) OR (EXISTS (SELECT 1 FROM workflow_conflicts stored WHERE ((stored.conflict_id=NEW.conflict_id) OR (stored.run_id=NEW.run_id AND stored.current_state_id=NEW.current_state_id AND stored.conflict_fingerprint=NEW.conflict_fingerprint)) AND ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=stored.run_id)) OR (EXISTS (SELECT 1 FROM stage_executions o18 WHERE o18.id=stored.stage_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o18.run_id)))) OR (EXISTS (SELECT 1 FROM lead_conflict_mediations o19 WHERE o19.id=stored.mediation_record_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o19.run_id)))))))
        ORDER BY f.disposition LIMIT 1)
      WHEN 'historical' THEN RAISE(ABORT,'source_continued')
      WHEN 'reserved' THEN RAISE(ABORT,'continuation_in_progress') END;
END;

CREATE TRIGGER p039_guard_workflow_conflicts_update
BEFORE UPDATE ON workflow_conflicts
WHEN EXISTS (SELECT 1 FROM run_execution_fences)
BEGIN
    SELECT CASE (SELECT f.disposition FROM run_execution_fences f
        WHERE ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=OLD.run_id)) OR (EXISTS (SELECT 1 FROM stage_executions o20 WHERE o20.id=OLD.stage_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o20.run_id)))) OR (EXISTS (SELECT 1 FROM lead_conflict_mediations o21 WHERE o21.id=OLD.mediation_record_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o21.run_id))))) OR ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=NEW.run_id)) OR (EXISTS (SELECT 1 FROM stage_executions o22 WHERE o22.id=NEW.stage_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o22.run_id)))) OR (EXISTS (SELECT 1 FROM lead_conflict_mediations o23 WHERE o23.id=NEW.mediation_record_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o23.run_id)))))
        ORDER BY f.disposition LIMIT 1)
      WHEN 'historical' THEN RAISE(ABORT,'source_continued')
      WHEN 'reserved' THEN RAISE(ABORT,'continuation_in_progress') END;
END;

CREATE TRIGGER p039_guard_workflow_conflicts_delete
BEFORE DELETE ON workflow_conflicts
WHEN EXISTS (SELECT 1 FROM run_execution_fences)
BEGIN
    SELECT CASE (SELECT f.disposition FROM run_execution_fences f
        WHERE (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=OLD.run_id)) OR (EXISTS (SELECT 1 FROM stage_executions o24 WHERE o24.id=OLD.stage_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o24.run_id)))) OR (EXISTS (SELECT 1 FROM lead_conflict_mediations o25 WHERE o25.id=OLD.mediation_record_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o25.run_id))))
        ORDER BY f.disposition LIMIT 1)
      WHEN 'historical' THEN RAISE(ABORT,'source_continued')
      WHEN 'reserved' THEN RAISE(ABORT,'continuation_in_progress') END;
END;

CREATE TRIGGER p039_guard_escalation_ledger_insert
BEFORE INSERT ON escalation_ledger
WHEN EXISTS (SELECT 1 FROM run_execution_fences)
BEGIN
    SELECT CASE (SELECT f.disposition FROM run_execution_fences f
        WHERE ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=NEW.run_id)) OR (EXISTS (SELECT 1 FROM stage_executions o26 WHERE o26.id=NEW.stage_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o26.run_id))))) OR (EXISTS (SELECT 1 FROM escalation_ledger stored WHERE ((stored.id=NEW.id)) AND ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=stored.run_id)) OR (EXISTS (SELECT 1 FROM stage_executions o27 WHERE o27.id=stored.stage_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o27.run_id)))))))
        ORDER BY f.disposition LIMIT 1)
      WHEN 'historical' THEN RAISE(ABORT,'source_continued')
      WHEN 'reserved' THEN RAISE(ABORT,'continuation_in_progress') END;
END;

CREATE TRIGGER p039_guard_escalation_ledger_update
BEFORE UPDATE ON escalation_ledger
WHEN EXISTS (SELECT 1 FROM run_execution_fences)
BEGIN
    SELECT CASE (SELECT f.disposition FROM run_execution_fences f
        WHERE ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=OLD.run_id)) OR (EXISTS (SELECT 1 FROM stage_executions o28 WHERE o28.id=OLD.stage_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o28.run_id))))) OR ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=NEW.run_id)) OR (EXISTS (SELECT 1 FROM stage_executions o29 WHERE o29.id=NEW.stage_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o29.run_id)))))
        ORDER BY f.disposition LIMIT 1)
      WHEN 'historical' THEN RAISE(ABORT,'source_continued')
      WHEN 'reserved' THEN RAISE(ABORT,'continuation_in_progress') END;
END;

CREATE TRIGGER p039_guard_escalation_ledger_delete
BEFORE DELETE ON escalation_ledger
WHEN EXISTS (SELECT 1 FROM run_execution_fences)
BEGIN
    SELECT CASE (SELECT f.disposition FROM run_execution_fences f
        WHERE (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=OLD.run_id)) OR (EXISTS (SELECT 1 FROM stage_executions o30 WHERE o30.id=OLD.stage_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o30.run_id))))
        ORDER BY f.disposition LIMIT 1)
      WHEN 'historical' THEN RAISE(ABORT,'source_continued')
      WHEN 'reserved' THEN RAISE(ABORT,'continuation_in_progress') END;
END;

CREATE TRIGGER p039_guard_agent_work_continuations_insert
BEFORE INSERT ON agent_work_continuations
WHEN EXISTS (SELECT 1 FROM run_execution_fences)
BEGIN
    SELECT CASE (SELECT f.disposition FROM run_execution_fences f
        WHERE ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=NEW.run_id)) OR (EXISTS (SELECT 1 FROM stage_executions o31 WHERE o31.id=NEW.stage_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o31.run_id)))) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o32 WHERE o32.agent_execution_id=NEW.agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o32.run_id)))) OR (EXISTS (SELECT 1 FROM session_generations o33 WHERE o33.id=NEW.session_generation_id AND ((EXISTS (SELECT 1 FROM session_lineages o34 WHERE o34.id=o33.lineage_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o34.run_id)))) OR (f.source_run_id IN (SELECT run_id FROM p039_run_resources WHERE resource_key=o33.working_directory AND resource_key<>''))))) OR (EXISTS (SELECT 1 FROM provider_sessions o35 WHERE o35.provider_session_id=NEW.provider_session_id AND ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o35.run_id)) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o36 WHERE o36.agent_execution_id=o35.agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o36.run_id)))))))) OR (EXISTS (SELECT 1 FROM agent_work_continuations stored WHERE ((stored.id=NEW.id)) AND ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=stored.run_id)) OR (EXISTS (SELECT 1 FROM stage_executions o37 WHERE o37.id=stored.stage_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o37.run_id)))) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o38 WHERE o38.agent_execution_id=stored.agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o38.run_id)))) OR (EXISTS (SELECT 1 FROM session_generations o39 WHERE o39.id=stored.session_generation_id AND ((EXISTS (SELECT 1 FROM session_lineages o40 WHERE o40.id=o39.lineage_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o40.run_id)))) OR (f.source_run_id IN (SELECT run_id FROM p039_run_resources WHERE resource_key=o39.working_directory AND resource_key<>''))))) OR (EXISTS (SELECT 1 FROM provider_sessions o41 WHERE o41.provider_session_id=stored.provider_session_id AND ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o41.run_id)) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o42 WHERE o42.agent_execution_id=o41.agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o42.run_id))))))))))
        ORDER BY f.disposition LIMIT 1)
      WHEN 'historical' THEN RAISE(ABORT,'source_continued')
      WHEN 'reserved' THEN RAISE(ABORT,'continuation_in_progress') END;
END;

CREATE TRIGGER p039_guard_agent_work_continuations_update
BEFORE UPDATE ON agent_work_continuations
WHEN EXISTS (SELECT 1 FROM run_execution_fences)
BEGIN
    SELECT CASE (SELECT f.disposition FROM run_execution_fences f
        WHERE ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=OLD.run_id)) OR (EXISTS (SELECT 1 FROM stage_executions o43 WHERE o43.id=OLD.stage_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o43.run_id)))) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o44 WHERE o44.agent_execution_id=OLD.agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o44.run_id)))) OR (EXISTS (SELECT 1 FROM session_generations o45 WHERE o45.id=OLD.session_generation_id AND ((EXISTS (SELECT 1 FROM session_lineages o46 WHERE o46.id=o45.lineage_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o46.run_id)))) OR (f.source_run_id IN (SELECT run_id FROM p039_run_resources WHERE resource_key=o45.working_directory AND resource_key<>''))))) OR (EXISTS (SELECT 1 FROM provider_sessions o47 WHERE o47.provider_session_id=OLD.provider_session_id AND ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o47.run_id)) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o48 WHERE o48.agent_execution_id=o47.agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o48.run_id)))))))) OR ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=NEW.run_id)) OR (EXISTS (SELECT 1 FROM stage_executions o49 WHERE o49.id=NEW.stage_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o49.run_id)))) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o50 WHERE o50.agent_execution_id=NEW.agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o50.run_id)))) OR (EXISTS (SELECT 1 FROM session_generations o51 WHERE o51.id=NEW.session_generation_id AND ((EXISTS (SELECT 1 FROM session_lineages o52 WHERE o52.id=o51.lineage_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o52.run_id)))) OR (f.source_run_id IN (SELECT run_id FROM p039_run_resources WHERE resource_key=o51.working_directory AND resource_key<>''))))) OR (EXISTS (SELECT 1 FROM provider_sessions o53 WHERE o53.provider_session_id=NEW.provider_session_id AND ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o53.run_id)) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o54 WHERE o54.agent_execution_id=o53.agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o54.run_id))))))))
        ORDER BY f.disposition LIMIT 1)
      WHEN 'historical' THEN RAISE(ABORT,'source_continued')
      WHEN 'reserved' THEN RAISE(ABORT,'continuation_in_progress') END;
END;

CREATE TRIGGER p039_guard_agent_work_continuations_delete
BEFORE DELETE ON agent_work_continuations
WHEN EXISTS (SELECT 1 FROM run_execution_fences)
BEGIN
    SELECT CASE (SELECT f.disposition FROM run_execution_fences f
        WHERE (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=OLD.run_id)) OR (EXISTS (SELECT 1 FROM stage_executions o55 WHERE o55.id=OLD.stage_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o55.run_id)))) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o56 WHERE o56.agent_execution_id=OLD.agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o56.run_id)))) OR (EXISTS (SELECT 1 FROM session_generations o57 WHERE o57.id=OLD.session_generation_id AND ((EXISTS (SELECT 1 FROM session_lineages o58 WHERE o58.id=o57.lineage_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o58.run_id)))) OR (f.source_run_id IN (SELECT run_id FROM p039_run_resources WHERE resource_key=o57.working_directory AND resource_key<>''))))) OR (EXISTS (SELECT 1 FROM provider_sessions o59 WHERE o59.provider_session_id=OLD.provider_session_id AND ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o59.run_id)) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o60 WHERE o60.agent_execution_id=o59.agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o60.run_id)))))))
        ORDER BY f.disposition LIMIT 1)
      WHEN 'historical' THEN RAISE(ABORT,'source_continued')
      WHEN 'reserved' THEN RAISE(ABORT,'continuation_in_progress') END;
END;

CREATE TRIGGER p039_guard_agent_retry_budget_ledger_insert
BEFORE INSERT ON agent_retry_budget_ledger
WHEN EXISTS (SELECT 1 FROM run_execution_fences)
BEGIN
    SELECT CASE (SELECT f.disposition FROM run_execution_fences f
        WHERE (((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=NEW.run_id)) OR (EXISTS (SELECT 1 FROM stage_executions o61 WHERE o61.id=NEW.stage_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o61.run_id)))) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o62 WHERE o62.agent_execution_id=NEW.agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o62.run_id))))) OR (((NEW.owner_kind='stage_execution' AND (EXISTS (SELECT 1 FROM stage_executions o63 WHERE o63.id=NEW.owner_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o63.run_id)))))) OR ((NEW.owner_kind='lead_conflict_mediation' AND (EXISTS (SELECT 1 FROM lead_conflict_mediations o64 WHERE o64.id=NEW.owner_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o64.run_id)))))))) OR (EXISTS (SELECT 1 FROM agent_retry_budget_ledger stored WHERE ((stored.id=NEW.id) OR (stored.idempotency_key=NEW.idempotency_key)) AND (((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=stored.run_id)) OR (EXISTS (SELECT 1 FROM stage_executions o65 WHERE o65.id=stored.stage_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o65.run_id)))) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o66 WHERE o66.agent_execution_id=stored.agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o66.run_id))))) OR (((stored.owner_kind='stage_execution' AND (EXISTS (SELECT 1 FROM stage_executions o67 WHERE o67.id=stored.owner_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o67.run_id)))))) OR ((stored.owner_kind='lead_conflict_mediation' AND (EXISTS (SELECT 1 FROM lead_conflict_mediations o68 WHERE o68.id=stored.owner_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o68.run_id))))))))))
        ORDER BY f.disposition LIMIT 1)
      WHEN 'historical' THEN RAISE(ABORT,'source_continued')
      WHEN 'reserved' THEN RAISE(ABORT,'continuation_in_progress') END;
END;

CREATE TRIGGER p039_guard_agent_retry_budget_ledger_update
BEFORE UPDATE ON agent_retry_budget_ledger
WHEN EXISTS (SELECT 1 FROM run_execution_fences)
BEGIN
    SELECT CASE (SELECT f.disposition FROM run_execution_fences f
        WHERE (((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=OLD.run_id)) OR (EXISTS (SELECT 1 FROM stage_executions o69 WHERE o69.id=OLD.stage_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o69.run_id)))) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o70 WHERE o70.agent_execution_id=OLD.agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o70.run_id))))) OR (((OLD.owner_kind='stage_execution' AND (EXISTS (SELECT 1 FROM stage_executions o71 WHERE o71.id=OLD.owner_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o71.run_id)))))) OR ((OLD.owner_kind='lead_conflict_mediation' AND (EXISTS (SELECT 1 FROM lead_conflict_mediations o72 WHERE o72.id=OLD.owner_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o72.run_id)))))))) OR (((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=NEW.run_id)) OR (EXISTS (SELECT 1 FROM stage_executions o73 WHERE o73.id=NEW.stage_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o73.run_id)))) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o74 WHERE o74.agent_execution_id=NEW.agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o74.run_id))))) OR (((NEW.owner_kind='stage_execution' AND (EXISTS (SELECT 1 FROM stage_executions o75 WHERE o75.id=NEW.owner_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o75.run_id)))))) OR ((NEW.owner_kind='lead_conflict_mediation' AND (EXISTS (SELECT 1 FROM lead_conflict_mediations o76 WHERE o76.id=NEW.owner_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o76.run_id))))))))
        ORDER BY f.disposition LIMIT 1)
      WHEN 'historical' THEN RAISE(ABORT,'source_continued')
      WHEN 'reserved' THEN RAISE(ABORT,'continuation_in_progress') END;
END;

CREATE TRIGGER p039_guard_agent_retry_budget_ledger_delete
BEFORE DELETE ON agent_retry_budget_ledger
WHEN EXISTS (SELECT 1 FROM run_execution_fences)
BEGIN
    SELECT CASE (SELECT f.disposition FROM run_execution_fences f
        WHERE ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=OLD.run_id)) OR (EXISTS (SELECT 1 FROM stage_executions o77 WHERE o77.id=OLD.stage_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o77.run_id)))) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o78 WHERE o78.agent_execution_id=OLD.agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o78.run_id))))) OR (((OLD.owner_kind='stage_execution' AND (EXISTS (SELECT 1 FROM stage_executions o79 WHERE o79.id=OLD.owner_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o79.run_id)))))) OR ((OLD.owner_kind='lead_conflict_mediation' AND (EXISTS (SELECT 1 FROM lead_conflict_mediations o80 WHERE o80.id=OLD.owner_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o80.run_id)))))))
        ORDER BY f.disposition LIMIT 1)
      WHEN 'historical' THEN RAISE(ABORT,'source_continued')
      WHEN 'reserved' THEN RAISE(ABORT,'continuation_in_progress') END;
END;

CREATE TRIGGER p039_guard_artifact_source_generation_claims_insert
BEFORE INSERT ON artifact_source_generation_claims
WHEN EXISTS (SELECT 1 FROM run_execution_fences)
BEGIN
    SELECT CASE (SELECT f.disposition FROM run_execution_fences f
        WHERE (((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=NEW.run_id)) OR (EXISTS (SELECT 1 FROM stage_executions o81 WHERE o81.id=NEW.stage_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o81.run_id)))) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o82 WHERE o82.agent_execution_id=NEW.agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o82.run_id))))) OR (((NEW.owner_kind='stage_execution' AND (EXISTS (SELECT 1 FROM stage_executions o83 WHERE o83.id=NEW.owner_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o83.run_id)))))) OR ((NEW.owner_kind='lead_conflict_mediation' AND (EXISTS (SELECT 1 FROM lead_conflict_mediations o84 WHERE o84.id=NEW.owner_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o84.run_id))))))) OR (EXISTS (SELECT 1 FROM session_generations o85 WHERE o85.id=NEW.current_session_generation_id AND ((EXISTS (SELECT 1 FROM session_lineages o86 WHERE o86.id=o85.lineage_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o86.run_id)))) OR (f.source_run_id IN (SELECT run_id FROM p039_run_resources WHERE resource_key=o85.working_directory AND resource_key<>''))))) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o87 WHERE o87.agent_execution_id=NEW.superseded_by_agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o87.run_id))))) OR (EXISTS (SELECT 1 FROM artifact_source_generation_claims stored WHERE ((stored.run_id=NEW.run_id AND stored.owner_kind=NEW.owner_kind AND stored.owner_id=NEW.owner_id AND stored.agent_execution_id=NEW.agent_execution_id AND stored.source_work_item_id=NEW.source_work_item_id)) AND (((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=stored.run_id)) OR (EXISTS (SELECT 1 FROM stage_executions o88 WHERE o88.id=stored.stage_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o88.run_id)))) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o89 WHERE o89.agent_execution_id=stored.agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o89.run_id))))) OR (((stored.owner_kind='stage_execution' AND (EXISTS (SELECT 1 FROM stage_executions o90 WHERE o90.id=stored.owner_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o90.run_id)))))) OR ((stored.owner_kind='lead_conflict_mediation' AND (EXISTS (SELECT 1 FROM lead_conflict_mediations o91 WHERE o91.id=stored.owner_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o91.run_id))))))) OR (EXISTS (SELECT 1 FROM session_generations o92 WHERE o92.id=stored.current_session_generation_id AND ((EXISTS (SELECT 1 FROM session_lineages o93 WHERE o93.id=o92.lineage_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o93.run_id)))) OR (f.source_run_id IN (SELECT run_id FROM p039_run_resources WHERE resource_key=o92.working_directory AND resource_key<>''))))) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o94 WHERE o94.agent_execution_id=stored.superseded_by_agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o94.run_id)))))))
        ORDER BY f.disposition LIMIT 1)
      WHEN 'historical' THEN RAISE(ABORT,'source_continued')
      WHEN 'reserved' THEN RAISE(ABORT,'continuation_in_progress') END;
END;

CREATE TRIGGER p039_guard_artifact_source_generation_claims_update
BEFORE UPDATE ON artifact_source_generation_claims
WHEN EXISTS (SELECT 1 FROM run_execution_fences)
BEGIN
    SELECT CASE (SELECT f.disposition FROM run_execution_fences f
        WHERE (((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=OLD.run_id)) OR (EXISTS (SELECT 1 FROM stage_executions o95 WHERE o95.id=OLD.stage_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o95.run_id)))) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o96 WHERE o96.agent_execution_id=OLD.agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o96.run_id))))) OR (((OLD.owner_kind='stage_execution' AND (EXISTS (SELECT 1 FROM stage_executions o97 WHERE o97.id=OLD.owner_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o97.run_id)))))) OR ((OLD.owner_kind='lead_conflict_mediation' AND (EXISTS (SELECT 1 FROM lead_conflict_mediations o98 WHERE o98.id=OLD.owner_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o98.run_id))))))) OR (EXISTS (SELECT 1 FROM session_generations o99 WHERE o99.id=OLD.current_session_generation_id AND ((EXISTS (SELECT 1 FROM session_lineages o100 WHERE o100.id=o99.lineage_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o100.run_id)))) OR (f.source_run_id IN (SELECT run_id FROM p039_run_resources WHERE resource_key=o99.working_directory AND resource_key<>''))))) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o101 WHERE o101.agent_execution_id=OLD.superseded_by_agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o101.run_id))))) OR (((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=NEW.run_id)) OR (EXISTS (SELECT 1 FROM stage_executions o102 WHERE o102.id=NEW.stage_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o102.run_id)))) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o103 WHERE o103.agent_execution_id=NEW.agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o103.run_id))))) OR (((NEW.owner_kind='stage_execution' AND (EXISTS (SELECT 1 FROM stage_executions o104 WHERE o104.id=NEW.owner_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o104.run_id)))))) OR ((NEW.owner_kind='lead_conflict_mediation' AND (EXISTS (SELECT 1 FROM lead_conflict_mediations o105 WHERE o105.id=NEW.owner_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o105.run_id))))))) OR (EXISTS (SELECT 1 FROM session_generations o106 WHERE o106.id=NEW.current_session_generation_id AND ((EXISTS (SELECT 1 FROM session_lineages o107 WHERE o107.id=o106.lineage_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o107.run_id)))) OR (f.source_run_id IN (SELECT run_id FROM p039_run_resources WHERE resource_key=o106.working_directory AND resource_key<>''))))) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o108 WHERE o108.agent_execution_id=NEW.superseded_by_agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o108.run_id)))))
        ORDER BY f.disposition LIMIT 1)
      WHEN 'historical' THEN RAISE(ABORT,'source_continued')
      WHEN 'reserved' THEN RAISE(ABORT,'continuation_in_progress') END;
END;

CREATE TRIGGER p039_guard_artifact_source_generation_claims_delete
BEFORE DELETE ON artifact_source_generation_claims
WHEN EXISTS (SELECT 1 FROM run_execution_fences)
BEGIN
    SELECT CASE (SELECT f.disposition FROM run_execution_fences f
        WHERE ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=OLD.run_id)) OR (EXISTS (SELECT 1 FROM stage_executions o109 WHERE o109.id=OLD.stage_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o109.run_id)))) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o110 WHERE o110.agent_execution_id=OLD.agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o110.run_id))))) OR (((OLD.owner_kind='stage_execution' AND (EXISTS (SELECT 1 FROM stage_executions o111 WHERE o111.id=OLD.owner_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o111.run_id)))))) OR ((OLD.owner_kind='lead_conflict_mediation' AND (EXISTS (SELECT 1 FROM lead_conflict_mediations o112 WHERE o112.id=OLD.owner_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o112.run_id))))))) OR (EXISTS (SELECT 1 FROM session_generations o113 WHERE o113.id=OLD.current_session_generation_id AND ((EXISTS (SELECT 1 FROM session_lineages o114 WHERE o114.id=o113.lineage_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o114.run_id)))) OR (f.source_run_id IN (SELECT run_id FROM p039_run_resources WHERE resource_key=o113.working_directory AND resource_key<>''))))) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o115 WHERE o115.agent_execution_id=OLD.superseded_by_agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o115.run_id))))
        ORDER BY f.disposition LIMIT 1)
      WHEN 'historical' THEN RAISE(ABORT,'source_continued')
      WHEN 'reserved' THEN RAISE(ABORT,'continuation_in_progress') END;
END;

CREATE TRIGGER p039_guard_artifact_contract_generations_insert
BEFORE INSERT ON artifact_contract_generations
WHEN EXISTS (SELECT 1 FROM run_execution_fences)
BEGIN
    SELECT CASE (SELECT f.disposition FROM run_execution_fences f
        WHERE ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=NEW.run_id)) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o116 WHERE o116.agent_execution_id=NEW.source_agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o116.run_id)))) OR (EXISTS (SELECT 1 FROM stage_executions o117 WHERE o117.id=NEW.source_stage_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o117.run_id))))) OR (EXISTS (SELECT 1 FROM artifact_contract_generations stored WHERE ((stored.generation_id=NEW.generation_id)) AND ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=stored.run_id)) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o118 WHERE o118.agent_execution_id=stored.source_agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o118.run_id)))) OR (EXISTS (SELECT 1 FROM stage_executions o119 WHERE o119.id=stored.source_stage_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o119.run_id)))))))
        ORDER BY f.disposition LIMIT 1)
      WHEN 'historical' THEN RAISE(ABORT,'source_continued')
      WHEN 'reserved' THEN RAISE(ABORT,'continuation_in_progress') END;
END;

CREATE TRIGGER p039_guard_artifact_contract_generations_update
BEFORE UPDATE ON artifact_contract_generations
WHEN EXISTS (SELECT 1 FROM run_execution_fences)
BEGIN
    SELECT CASE (SELECT f.disposition FROM run_execution_fences f
        WHERE ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=OLD.run_id)) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o120 WHERE o120.agent_execution_id=OLD.source_agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o120.run_id)))) OR (EXISTS (SELECT 1 FROM stage_executions o121 WHERE o121.id=OLD.source_stage_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o121.run_id))))) OR ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=NEW.run_id)) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o122 WHERE o122.agent_execution_id=NEW.source_agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o122.run_id)))) OR (EXISTS (SELECT 1 FROM stage_executions o123 WHERE o123.id=NEW.source_stage_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o123.run_id)))))
        ORDER BY f.disposition LIMIT 1)
      WHEN 'historical' THEN RAISE(ABORT,'source_continued')
      WHEN 'reserved' THEN RAISE(ABORT,'continuation_in_progress') END;
END;

CREATE TRIGGER p039_guard_artifact_contract_generations_delete
BEFORE DELETE ON artifact_contract_generations
WHEN EXISTS (SELECT 1 FROM run_execution_fences)
BEGIN
    SELECT CASE (SELECT f.disposition FROM run_execution_fences f
        WHERE (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=OLD.run_id)) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o124 WHERE o124.agent_execution_id=OLD.source_agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o124.run_id)))) OR (EXISTS (SELECT 1 FROM stage_executions o125 WHERE o125.id=OLD.source_stage_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o125.run_id))))
        ORDER BY f.disposition LIMIT 1)
      WHEN 'historical' THEN RAISE(ABORT,'source_continued')
      WHEN 'reserved' THEN RAISE(ABORT,'continuation_in_progress') END;
END;

CREATE TRIGGER p039_guard_active_artifact_contracts_insert
BEFORE INSERT ON active_artifact_contracts
WHEN EXISTS (SELECT 1 FROM run_execution_fences)
BEGIN
    SELECT CASE (SELECT f.disposition FROM run_execution_fences f
        WHERE ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=NEW.run_id)) OR (EXISTS (SELECT 1 FROM artifact_contract_generations o126 WHERE o126.generation_id=NEW.generation_id AND ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o126.run_id)) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o127 WHERE o127.agent_execution_id=o126.source_agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o127.run_id)))) OR (EXISTS (SELECT 1 FROM stage_executions o128 WHERE o128.id=o126.source_stage_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o128.run_id)))))))) OR (EXISTS (SELECT 1 FROM active_artifact_contracts stored WHERE ((stored.run_id=NEW.run_id AND stored.contract_id=NEW.contract_id)) AND ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=stored.run_id)) OR (EXISTS (SELECT 1 FROM artifact_contract_generations o129 WHERE o129.generation_id=stored.generation_id AND ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o129.run_id)) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o130 WHERE o130.agent_execution_id=o129.source_agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o130.run_id)))) OR (EXISTS (SELECT 1 FROM stage_executions o131 WHERE o131.id=o129.source_stage_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o131.run_id))))))))))
        ORDER BY f.disposition LIMIT 1)
      WHEN 'historical' THEN RAISE(ABORT,'source_continued')
      WHEN 'reserved' THEN RAISE(ABORT,'continuation_in_progress') END;
END;

CREATE TRIGGER p039_guard_active_artifact_contracts_update
BEFORE UPDATE ON active_artifact_contracts
WHEN EXISTS (SELECT 1 FROM run_execution_fences)
BEGIN
    SELECT CASE (SELECT f.disposition FROM run_execution_fences f
        WHERE ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=OLD.run_id)) OR (EXISTS (SELECT 1 FROM artifact_contract_generations o132 WHERE o132.generation_id=OLD.generation_id AND ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o132.run_id)) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o133 WHERE o133.agent_execution_id=o132.source_agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o133.run_id)))) OR (EXISTS (SELECT 1 FROM stage_executions o134 WHERE o134.id=o132.source_stage_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o134.run_id)))))))) OR ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=NEW.run_id)) OR (EXISTS (SELECT 1 FROM artifact_contract_generations o135 WHERE o135.generation_id=NEW.generation_id AND ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o135.run_id)) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o136 WHERE o136.agent_execution_id=o135.source_agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o136.run_id)))) OR (EXISTS (SELECT 1 FROM stage_executions o137 WHERE o137.id=o135.source_stage_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o137.run_id))))))))
        ORDER BY f.disposition LIMIT 1)
      WHEN 'historical' THEN RAISE(ABORT,'source_continued')
      WHEN 'reserved' THEN RAISE(ABORT,'continuation_in_progress') END;
END;

CREATE TRIGGER p039_guard_active_artifact_contracts_delete
BEFORE DELETE ON active_artifact_contracts
WHEN EXISTS (SELECT 1 FROM run_execution_fences)
BEGIN
    SELECT CASE (SELECT f.disposition FROM run_execution_fences f
        WHERE (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=OLD.run_id)) OR (EXISTS (SELECT 1 FROM artifact_contract_generations o138 WHERE o138.generation_id=OLD.generation_id AND ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o138.run_id)) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o139 WHERE o139.agent_execution_id=o138.source_agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o139.run_id)))) OR (EXISTS (SELECT 1 FROM stage_executions o140 WHERE o140.id=o138.source_stage_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o140.run_id)))))))
        ORDER BY f.disposition LIMIT 1)
      WHEN 'historical' THEN RAISE(ABORT,'source_continued')
      WHEN 'reserved' THEN RAISE(ABORT,'continuation_in_progress') END;
END;

CREATE TRIGGER p039_guard_session_generations_insert
BEFORE INSERT ON session_generations
WHEN EXISTS (SELECT 1 FROM run_execution_fences)
BEGIN
    SELECT CASE (SELECT f.disposition FROM run_execution_fences f
        WHERE ((EXISTS (SELECT 1 FROM session_lineages o141 WHERE o141.id=NEW.lineage_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o141.run_id)))) OR (f.source_run_id IN (SELECT run_id FROM p039_run_resources WHERE resource_key=NEW.working_directory AND resource_key<>''))) OR (EXISTS (SELECT 1 FROM session_generations stored WHERE ((stored.id=NEW.id)) AND ((EXISTS (SELECT 1 FROM session_lineages o142 WHERE o142.id=stored.lineage_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o142.run_id)))) OR (f.source_run_id IN (SELECT run_id FROM p039_run_resources WHERE resource_key=stored.working_directory AND resource_key<>'')))))
        ORDER BY f.disposition LIMIT 1)
      WHEN 'historical' THEN RAISE(ABORT,'source_continued')
      WHEN 'reserved' THEN RAISE(ABORT,'continuation_in_progress') END;
END;

CREATE TRIGGER p039_guard_session_generations_update
BEFORE UPDATE ON session_generations
WHEN EXISTS (SELECT 1 FROM run_execution_fences)
BEGIN
    SELECT CASE (SELECT f.disposition FROM run_execution_fences f
        WHERE ((EXISTS (SELECT 1 FROM session_lineages o143 WHERE o143.id=OLD.lineage_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o143.run_id)))) OR (f.source_run_id IN (SELECT run_id FROM p039_run_resources WHERE resource_key=OLD.working_directory AND resource_key<>''))) OR ((EXISTS (SELECT 1 FROM session_lineages o144 WHERE o144.id=NEW.lineage_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o144.run_id)))) OR (f.source_run_id IN (SELECT run_id FROM p039_run_resources WHERE resource_key=NEW.working_directory AND resource_key<>'')))
        ORDER BY f.disposition LIMIT 1)
      WHEN 'historical' THEN RAISE(ABORT,'source_continued')
      WHEN 'reserved' THEN RAISE(ABORT,'continuation_in_progress') END;
END;

CREATE TRIGGER p039_guard_session_generations_delete
BEFORE DELETE ON session_generations
WHEN EXISTS (SELECT 1 FROM run_execution_fences)
BEGIN
    SELECT CASE (SELECT f.disposition FROM run_execution_fences f
        WHERE (EXISTS (SELECT 1 FROM session_lineages o145 WHERE o145.id=OLD.lineage_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o145.run_id)))) OR (f.source_run_id IN (SELECT run_id FROM p039_run_resources WHERE resource_key=OLD.working_directory AND resource_key<>''))
        ORDER BY f.disposition LIMIT 1)
      WHEN 'historical' THEN RAISE(ABORT,'source_continued')
      WHEN 'reserved' THEN RAISE(ABORT,'continuation_in_progress') END;
END;

CREATE TRIGGER p039_guard_agent_executions_insert
BEFORE INSERT ON agent_executions
WHEN EXISTS (SELECT 1 FROM run_execution_fences)
BEGIN
    SELECT CASE (SELECT f.disposition FROM run_execution_fences f
        WHERE ((EXISTS (SELECT 1 FROM stage_executions o146 WHERE o146.id=NEW.stage_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o146.run_id)))) OR (EXISTS (SELECT 1 FROM stage_executions o147 WHERE o147.id=NEW.origin_stage_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o147.run_id)))) OR (((NEW.owner_kind='stage_execution' AND (EXISTS (SELECT 1 FROM stage_executions o148 WHERE o148.id=NEW.owner_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o148.run_id)))))) OR ((NEW.owner_kind='lead_conflict_mediation' AND (EXISTS (SELECT 1 FROM lead_conflict_mediations o149 WHERE o149.id=NEW.owner_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o149.run_id))))))) OR (EXISTS (SELECT 1 FROM lead_conflict_mediations o150 WHERE o150.id=NEW.lead_mediation_record_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o150.run_id)))) OR (EXISTS (SELECT 1 FROM session_lineages o151 WHERE o151.id=NEW.session_lineage_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o151.run_id)))) OR (EXISTS (SELECT 1 FROM session_generations o152 WHERE o152.id=NEW.session_generation_id AND ((EXISTS (SELECT 1 FROM session_lineages o153 WHERE o153.id=o152.lineage_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o153.run_id)))) OR (f.source_run_id IN (SELECT run_id FROM p039_run_resources WHERE resource_key=o152.working_directory AND resource_key<>''))))) OR (NEW.status NOT IN ('completed','failed','cancelled') AND ((NEW.owner_kind='lead_conflict_mediation' AND NOT EXISTS (SELECT 1 FROM lead_conflict_mediations m JOIN runs q ON q.id=m.run_id WHERE m.id=NEW.owner_id)) OR (NEW.owner_kind='stage_execution' AND NOT EXISTS (SELECT 1 FROM stage_executions s JOIN runs q ON q.id=s.run_id WHERE s.id=NEW.stage_execution_id))))) OR (EXISTS (SELECT 1 FROM agent_executions stored WHERE ((stored.id=NEW.id)) AND ((EXISTS (SELECT 1 FROM stage_executions o154 WHERE o154.id=stored.stage_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o154.run_id)))) OR (EXISTS (SELECT 1 FROM stage_executions o155 WHERE o155.id=stored.origin_stage_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o155.run_id)))) OR (((stored.owner_kind='stage_execution' AND (EXISTS (SELECT 1 FROM stage_executions o156 WHERE o156.id=stored.owner_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o156.run_id)))))) OR ((stored.owner_kind='lead_conflict_mediation' AND (EXISTS (SELECT 1 FROM lead_conflict_mediations o157 WHERE o157.id=stored.owner_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o157.run_id))))))) OR (EXISTS (SELECT 1 FROM lead_conflict_mediations o158 WHERE o158.id=stored.lead_mediation_record_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o158.run_id)))) OR (EXISTS (SELECT 1 FROM session_lineages o159 WHERE o159.id=stored.session_lineage_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o159.run_id)))) OR (EXISTS (SELECT 1 FROM session_generations o160 WHERE o160.id=stored.session_generation_id AND ((EXISTS (SELECT 1 FROM session_lineages o161 WHERE o161.id=o160.lineage_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o161.run_id)))) OR (f.source_run_id IN (SELECT run_id FROM p039_run_resources WHERE resource_key=o160.working_directory AND resource_key<>''))))) OR (stored.status NOT IN ('completed','failed','cancelled') AND ((stored.owner_kind='lead_conflict_mediation' AND NOT EXISTS (SELECT 1 FROM lead_conflict_mediations m JOIN runs q ON q.id=m.run_id WHERE m.id=stored.owner_id)) OR (stored.owner_kind='stage_execution' AND NOT EXISTS (SELECT 1 FROM stage_executions s JOIN runs q ON q.id=s.run_id WHERE s.id=stored.stage_execution_id)))))))
        ORDER BY f.disposition LIMIT 1)
      WHEN 'historical' THEN RAISE(ABORT,'source_continued')
      WHEN 'reserved' THEN RAISE(ABORT,'continuation_in_progress') END;
END;

CREATE TRIGGER p039_guard_agent_executions_update
BEFORE UPDATE ON agent_executions
WHEN EXISTS (SELECT 1 FROM run_execution_fences)
BEGIN
    SELECT CASE (SELECT f.disposition FROM run_execution_fences f
        WHERE ((EXISTS (SELECT 1 FROM stage_executions o162 WHERE o162.id=OLD.stage_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o162.run_id)))) OR (EXISTS (SELECT 1 FROM stage_executions o163 WHERE o163.id=OLD.origin_stage_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o163.run_id)))) OR (((OLD.owner_kind='stage_execution' AND (EXISTS (SELECT 1 FROM stage_executions o164 WHERE o164.id=OLD.owner_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o164.run_id)))))) OR ((OLD.owner_kind='lead_conflict_mediation' AND (EXISTS (SELECT 1 FROM lead_conflict_mediations o165 WHERE o165.id=OLD.owner_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o165.run_id))))))) OR (EXISTS (SELECT 1 FROM lead_conflict_mediations o166 WHERE o166.id=OLD.lead_mediation_record_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o166.run_id)))) OR (EXISTS (SELECT 1 FROM session_lineages o167 WHERE o167.id=OLD.session_lineage_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o167.run_id)))) OR (EXISTS (SELECT 1 FROM session_generations o168 WHERE o168.id=OLD.session_generation_id AND ((EXISTS (SELECT 1 FROM session_lineages o169 WHERE o169.id=o168.lineage_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o169.run_id)))) OR (f.source_run_id IN (SELECT run_id FROM p039_run_resources WHERE resource_key=o168.working_directory AND resource_key<>''))))) OR (OLD.status NOT IN ('completed','failed','cancelled') AND ((OLD.owner_kind='lead_conflict_mediation' AND NOT EXISTS (SELECT 1 FROM lead_conflict_mediations m JOIN runs q ON q.id=m.run_id WHERE m.id=OLD.owner_id)) OR (OLD.owner_kind='stage_execution' AND NOT EXISTS (SELECT 1 FROM stage_executions s JOIN runs q ON q.id=s.run_id WHERE s.id=OLD.stage_execution_id))))) OR ((EXISTS (SELECT 1 FROM stage_executions o170 WHERE o170.id=NEW.stage_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o170.run_id)))) OR (EXISTS (SELECT 1 FROM stage_executions o171 WHERE o171.id=NEW.origin_stage_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o171.run_id)))) OR (((NEW.owner_kind='stage_execution' AND (EXISTS (SELECT 1 FROM stage_executions o172 WHERE o172.id=NEW.owner_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o172.run_id)))))) OR ((NEW.owner_kind='lead_conflict_mediation' AND (EXISTS (SELECT 1 FROM lead_conflict_mediations o173 WHERE o173.id=NEW.owner_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o173.run_id))))))) OR (EXISTS (SELECT 1 FROM lead_conflict_mediations o174 WHERE o174.id=NEW.lead_mediation_record_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o174.run_id)))) OR (EXISTS (SELECT 1 FROM session_lineages o175 WHERE o175.id=NEW.session_lineage_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o175.run_id)))) OR (EXISTS (SELECT 1 FROM session_generations o176 WHERE o176.id=NEW.session_generation_id AND ((EXISTS (SELECT 1 FROM session_lineages o177 WHERE o177.id=o176.lineage_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o177.run_id)))) OR (f.source_run_id IN (SELECT run_id FROM p039_run_resources WHERE resource_key=o176.working_directory AND resource_key<>''))))) OR (NEW.status NOT IN ('completed','failed','cancelled') AND ((NEW.owner_kind='lead_conflict_mediation' AND NOT EXISTS (SELECT 1 FROM lead_conflict_mediations m JOIN runs q ON q.id=m.run_id WHERE m.id=NEW.owner_id)) OR (NEW.owner_kind='stage_execution' AND NOT EXISTS (SELECT 1 FROM stage_executions s JOIN runs q ON q.id=s.run_id WHERE s.id=NEW.stage_execution_id)))))
        ORDER BY f.disposition LIMIT 1)
      WHEN 'historical' THEN RAISE(ABORT,'source_continued')
      WHEN 'reserved' THEN RAISE(ABORT,'continuation_in_progress') END;
END;

CREATE TRIGGER p039_guard_agent_executions_delete
BEFORE DELETE ON agent_executions
WHEN EXISTS (SELECT 1 FROM run_execution_fences)
BEGIN
    SELECT CASE (SELECT f.disposition FROM run_execution_fences f
        WHERE (EXISTS (SELECT 1 FROM stage_executions o178 WHERE o178.id=OLD.stage_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o178.run_id)))) OR (EXISTS (SELECT 1 FROM stage_executions o179 WHERE o179.id=OLD.origin_stage_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o179.run_id)))) OR (((OLD.owner_kind='stage_execution' AND (EXISTS (SELECT 1 FROM stage_executions o180 WHERE o180.id=OLD.owner_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o180.run_id)))))) OR ((OLD.owner_kind='lead_conflict_mediation' AND (EXISTS (SELECT 1 FROM lead_conflict_mediations o181 WHERE o181.id=OLD.owner_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o181.run_id))))))) OR (EXISTS (SELECT 1 FROM lead_conflict_mediations o182 WHERE o182.id=OLD.lead_mediation_record_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o182.run_id)))) OR (EXISTS (SELECT 1 FROM session_lineages o183 WHERE o183.id=OLD.session_lineage_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o183.run_id)))) OR (EXISTS (SELECT 1 FROM session_generations o184 WHERE o184.id=OLD.session_generation_id AND ((EXISTS (SELECT 1 FROM session_lineages o185 WHERE o185.id=o184.lineage_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o185.run_id)))) OR (f.source_run_id IN (SELECT run_id FROM p039_run_resources WHERE resource_key=o184.working_directory AND resource_key<>''))))) OR (OLD.status NOT IN ('completed','failed','cancelled') AND ((OLD.owner_kind='lead_conflict_mediation' AND NOT EXISTS (SELECT 1 FROM lead_conflict_mediations m JOIN runs q ON q.id=m.run_id WHERE m.id=OLD.owner_id)) OR (OLD.owner_kind='stage_execution' AND NOT EXISTS (SELECT 1 FROM stage_executions s JOIN runs q ON q.id=s.run_id WHERE s.id=OLD.stage_execution_id))))
        ORDER BY f.disposition LIMIT 1)
      WHEN 'historical' THEN RAISE(ABORT,'source_continued')
      WHEN 'reserved' THEN RAISE(ABORT,'continuation_in_progress') END;
END;

CREATE TRIGGER p039_guard_lead_mediation_confirmations_insert
BEFORE INSERT ON lead_mediation_confirmations
WHEN EXISTS (SELECT 1 FROM run_execution_fences)
BEGIN
    SELECT CASE (SELECT f.disposition FROM run_execution_fences f
        WHERE ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=NEW.run_id)) OR (EXISTS (SELECT 1 FROM lead_conflict_mediations o186 WHERE o186.id=NEW.mediation_record_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o186.run_id))))) OR (EXISTS (SELECT 1 FROM lead_mediation_confirmations stored WHERE ((stored.id=NEW.id)) AND ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=stored.run_id)) OR (EXISTS (SELECT 1 FROM lead_conflict_mediations o187 WHERE o187.id=stored.mediation_record_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o187.run_id)))))))
        ORDER BY f.disposition LIMIT 1)
      WHEN 'historical' THEN RAISE(ABORT,'source_continued')
      WHEN 'reserved' THEN RAISE(ABORT,'continuation_in_progress') END;
END;

CREATE TRIGGER p039_guard_lead_mediation_confirmations_update
BEFORE UPDATE ON lead_mediation_confirmations
WHEN EXISTS (SELECT 1 FROM run_execution_fences)
BEGIN
    SELECT CASE (SELECT f.disposition FROM run_execution_fences f
        WHERE ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=OLD.run_id)) OR (EXISTS (SELECT 1 FROM lead_conflict_mediations o188 WHERE o188.id=OLD.mediation_record_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o188.run_id))))) OR ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=NEW.run_id)) OR (EXISTS (SELECT 1 FROM lead_conflict_mediations o189 WHERE o189.id=NEW.mediation_record_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o189.run_id)))))
        ORDER BY f.disposition LIMIT 1)
      WHEN 'historical' THEN RAISE(ABORT,'source_continued')
      WHEN 'reserved' THEN RAISE(ABORT,'continuation_in_progress') END;
END;

CREATE TRIGGER p039_guard_lead_mediation_confirmations_delete
BEFORE DELETE ON lead_mediation_confirmations
WHEN EXISTS (SELECT 1 FROM run_execution_fences)
BEGIN
    SELECT CASE (SELECT f.disposition FROM run_execution_fences f
        WHERE (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=OLD.run_id)) OR (EXISTS (SELECT 1 FROM lead_conflict_mediations o190 WHERE o190.id=OLD.mediation_record_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o190.run_id))))
        ORDER BY f.disposition LIMIT 1)
      WHEN 'historical' THEN RAISE(ABORT,'source_continued')
      WHEN 'reserved' THEN RAISE(ABORT,'continuation_in_progress') END;
END;

CREATE TRIGGER p039_guard_work_items_insert
BEFORE INSERT ON work_items
WHEN EXISTS (SELECT 1 FROM run_execution_fences)
BEGIN
    SELECT CASE (SELECT f.disposition FROM run_execution_fences f
        WHERE ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=NEW.run_id)) OR (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=CASE WHEN json_valid(NEW.payload_json) THEN json_extract(NEW.payload_json,'$.run_id') END)) OR (EXISTS (SELECT 1 FROM stage_executions o191 WHERE o191.id=CASE WHEN json_valid(NEW.payload_json) THEN json_extract(NEW.payload_json,'$.stage_execution_id') END AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o191.run_id)))) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o192 WHERE o192.agent_execution_id=CASE WHEN json_valid(NEW.payload_json) THEN json_extract(NEW.payload_json,'$.agent_execution_id') END AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o192.run_id)))) OR (EXISTS (SELECT 1 FROM lead_conflict_mediations o193 WHERE o193.id=CASE WHEN json_valid(NEW.payload_json) THEN json_extract(NEW.payload_json,'$.mediation_record_id') END AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o193.run_id)))) OR (EXISTS (SELECT 1 FROM lead_conflict_mediations o194 WHERE o194.id=CASE WHEN json_valid(NEW.payload_json) THEN json_extract(NEW.payload_json,'$.lead_mediation_record_id') END AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o194.run_id)))) OR (EXISTS (SELECT 1 FROM agent_work_continuations o195 WHERE o195.id=CASE WHEN json_valid(NEW.payload_json) THEN json_extract(NEW.payload_json,'$.continuation_id') END AND ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o195.run_id)) OR (EXISTS (SELECT 1 FROM stage_executions o196 WHERE o196.id=o195.stage_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o196.run_id)))) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o197 WHERE o197.agent_execution_id=o195.agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o197.run_id)))) OR (EXISTS (SELECT 1 FROM session_generations o198 WHERE o198.id=o195.session_generation_id AND ((EXISTS (SELECT 1 FROM session_lineages o199 WHERE o199.id=o198.lineage_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o199.run_id)))) OR (f.source_run_id IN (SELECT run_id FROM p039_run_resources WHERE resource_key=o198.working_directory AND resource_key<>''))))) OR (EXISTS (SELECT 1 FROM provider_sessions o200 WHERE o200.provider_session_id=o195.provider_session_id AND ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o200.run_id)) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o201 WHERE o201.agent_execution_id=o200.agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o201.run_id)))))))))) OR (f.source_run_id IN (SELECT run_id FROM p039_run_resources WHERE resource_key=NEW.worktree_resource_key AND resource_key<>'')) OR (NEW.status='running' AND (NOT json_valid(NEW.payload_json) OR (CASE WHEN json_valid(NEW.payload_json) THEN json_extract(NEW.payload_json,'$.run_id') END IS NOT NULL AND NOT EXISTS (SELECT 1 FROM runs WHERE id=CASE WHEN json_valid(NEW.payload_json) THEN json_extract(NEW.payload_json,'$.run_id') END)) OR (CASE WHEN json_valid(NEW.payload_json) THEN json_extract(NEW.payload_json,'$.stage_execution_id') END IS NOT NULL AND NOT EXISTS (SELECT 1 FROM stage_executions WHERE id=CASE WHEN json_valid(NEW.payload_json) THEN json_extract(NEW.payload_json,'$.stage_execution_id') END)) OR (CASE WHEN json_valid(NEW.payload_json) THEN json_extract(NEW.payload_json,'$.agent_execution_id') END IS NOT NULL AND NOT EXISTS (SELECT 1 FROM p039_agent_run_owners WHERE agent_execution_id=CASE WHEN json_valid(NEW.payload_json) THEN json_extract(NEW.payload_json,'$.agent_execution_id') END)) OR (CASE WHEN json_valid(NEW.payload_json) THEN json_extract(NEW.payload_json,'$.mediation_record_id') END IS NOT NULL AND NOT EXISTS (SELECT 1 FROM lead_conflict_mediations WHERE id=CASE WHEN json_valid(NEW.payload_json) THEN json_extract(NEW.payload_json,'$.mediation_record_id') END)) OR (CASE WHEN json_valid(NEW.payload_json) THEN json_extract(NEW.payload_json,'$.lead_mediation_record_id') END IS NOT NULL AND NOT EXISTS (SELECT 1 FROM lead_conflict_mediations WHERE id=CASE WHEN json_valid(NEW.payload_json) THEN json_extract(NEW.payload_json,'$.lead_mediation_record_id') END)) OR (CASE WHEN json_valid(NEW.payload_json) THEN json_extract(NEW.payload_json,'$.continuation_id') END IS NOT NULL AND NOT EXISTS (SELECT 1 FROM agent_work_continuations WHERE id=CASE WHEN json_valid(NEW.payload_json) THEN json_extract(NEW.payload_json,'$.continuation_id') END)) OR (NEW.run_id IS NOT NULL AND NOT EXISTS (SELECT 1 FROM runs WHERE id=NEW.run_id)) OR (NEW.kind IN ('invoke_agent','settle_stage','advance_run','trigger_next_stage','process_continuation') AND NEW.run_id IS NULL AND CASE WHEN json_valid(NEW.payload_json) THEN json_extract(NEW.payload_json,'$.run_id') END IS NULL AND CASE WHEN json_valid(NEW.payload_json) THEN json_extract(NEW.payload_json,'$.stage_execution_id') END IS NULL AND CASE WHEN json_valid(NEW.payload_json) THEN json_extract(NEW.payload_json,'$.agent_execution_id') END IS NULL AND CASE WHEN json_valid(NEW.payload_json) THEN json_extract(NEW.payload_json,'$.continuation_id') END IS NULL)))) OR (EXISTS (SELECT 1 FROM work_items stored WHERE ((stored.id=NEW.id)) AND ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=stored.run_id)) OR (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=CASE WHEN json_valid(stored.payload_json) THEN json_extract(stored.payload_json,'$.run_id') END)) OR (EXISTS (SELECT 1 FROM stage_executions o202 WHERE o202.id=CASE WHEN json_valid(stored.payload_json) THEN json_extract(stored.payload_json,'$.stage_execution_id') END AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o202.run_id)))) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o203 WHERE o203.agent_execution_id=CASE WHEN json_valid(stored.payload_json) THEN json_extract(stored.payload_json,'$.agent_execution_id') END AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o203.run_id)))) OR (EXISTS (SELECT 1 FROM lead_conflict_mediations o204 WHERE o204.id=CASE WHEN json_valid(stored.payload_json) THEN json_extract(stored.payload_json,'$.mediation_record_id') END AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o204.run_id)))) OR (EXISTS (SELECT 1 FROM lead_conflict_mediations o205 WHERE o205.id=CASE WHEN json_valid(stored.payload_json) THEN json_extract(stored.payload_json,'$.lead_mediation_record_id') END AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o205.run_id)))) OR (EXISTS (SELECT 1 FROM agent_work_continuations o206 WHERE o206.id=CASE WHEN json_valid(stored.payload_json) THEN json_extract(stored.payload_json,'$.continuation_id') END AND ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o206.run_id)) OR (EXISTS (SELECT 1 FROM stage_executions o207 WHERE o207.id=o206.stage_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o207.run_id)))) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o208 WHERE o208.agent_execution_id=o206.agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o208.run_id)))) OR (EXISTS (SELECT 1 FROM session_generations o209 WHERE o209.id=o206.session_generation_id AND ((EXISTS (SELECT 1 FROM session_lineages o210 WHERE o210.id=o209.lineage_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o210.run_id)))) OR (f.source_run_id IN (SELECT run_id FROM p039_run_resources WHERE resource_key=o209.working_directory AND resource_key<>''))))) OR (EXISTS (SELECT 1 FROM provider_sessions o211 WHERE o211.provider_session_id=o206.provider_session_id AND ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o211.run_id)) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o212 WHERE o212.agent_execution_id=o211.agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o212.run_id)))))))))) OR (f.source_run_id IN (SELECT run_id FROM p039_run_resources WHERE resource_key=stored.worktree_resource_key AND resource_key<>'')) OR (stored.status='running' AND (NOT json_valid(stored.payload_json) OR (CASE WHEN json_valid(stored.payload_json) THEN json_extract(stored.payload_json,'$.run_id') END IS NOT NULL AND NOT EXISTS (SELECT 1 FROM runs WHERE id=CASE WHEN json_valid(stored.payload_json) THEN json_extract(stored.payload_json,'$.run_id') END)) OR (CASE WHEN json_valid(stored.payload_json) THEN json_extract(stored.payload_json,'$.stage_execution_id') END IS NOT NULL AND NOT EXISTS (SELECT 1 FROM stage_executions WHERE id=CASE WHEN json_valid(stored.payload_json) THEN json_extract(stored.payload_json,'$.stage_execution_id') END)) OR (CASE WHEN json_valid(stored.payload_json) THEN json_extract(stored.payload_json,'$.agent_execution_id') END IS NOT NULL AND NOT EXISTS (SELECT 1 FROM p039_agent_run_owners WHERE agent_execution_id=CASE WHEN json_valid(stored.payload_json) THEN json_extract(stored.payload_json,'$.agent_execution_id') END)) OR (CASE WHEN json_valid(stored.payload_json) THEN json_extract(stored.payload_json,'$.mediation_record_id') END IS NOT NULL AND NOT EXISTS (SELECT 1 FROM lead_conflict_mediations WHERE id=CASE WHEN json_valid(stored.payload_json) THEN json_extract(stored.payload_json,'$.mediation_record_id') END)) OR (CASE WHEN json_valid(stored.payload_json) THEN json_extract(stored.payload_json,'$.lead_mediation_record_id') END IS NOT NULL AND NOT EXISTS (SELECT 1 FROM lead_conflict_mediations WHERE id=CASE WHEN json_valid(stored.payload_json) THEN json_extract(stored.payload_json,'$.lead_mediation_record_id') END)) OR (CASE WHEN json_valid(stored.payload_json) THEN json_extract(stored.payload_json,'$.continuation_id') END IS NOT NULL AND NOT EXISTS (SELECT 1 FROM agent_work_continuations WHERE id=CASE WHEN json_valid(stored.payload_json) THEN json_extract(stored.payload_json,'$.continuation_id') END)) OR (stored.run_id IS NOT NULL AND NOT EXISTS (SELECT 1 FROM runs WHERE id=stored.run_id)) OR (stored.kind IN ('invoke_agent','settle_stage','advance_run','trigger_next_stage','process_continuation') AND stored.run_id IS NULL AND CASE WHEN json_valid(stored.payload_json) THEN json_extract(stored.payload_json,'$.run_id') END IS NULL AND CASE WHEN json_valid(stored.payload_json) THEN json_extract(stored.payload_json,'$.stage_execution_id') END IS NULL AND CASE WHEN json_valid(stored.payload_json) THEN json_extract(stored.payload_json,'$.agent_execution_id') END IS NULL AND CASE WHEN json_valid(stored.payload_json) THEN json_extract(stored.payload_json,'$.continuation_id') END IS NULL))))))
        ORDER BY f.disposition LIMIT 1)
      WHEN 'historical' THEN RAISE(ABORT,'source_continued')
      WHEN 'reserved' THEN RAISE(ABORT,'continuation_in_progress') END;
END;

CREATE TRIGGER p039_guard_work_items_update
BEFORE UPDATE ON work_items
WHEN EXISTS (SELECT 1 FROM run_execution_fences)
BEGIN
    SELECT CASE (SELECT f.disposition FROM run_execution_fences f
        WHERE ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=OLD.run_id)) OR (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=CASE WHEN json_valid(OLD.payload_json) THEN json_extract(OLD.payload_json,'$.run_id') END)) OR (EXISTS (SELECT 1 FROM stage_executions o213 WHERE o213.id=CASE WHEN json_valid(OLD.payload_json) THEN json_extract(OLD.payload_json,'$.stage_execution_id') END AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o213.run_id)))) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o214 WHERE o214.agent_execution_id=CASE WHEN json_valid(OLD.payload_json) THEN json_extract(OLD.payload_json,'$.agent_execution_id') END AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o214.run_id)))) OR (EXISTS (SELECT 1 FROM lead_conflict_mediations o215 WHERE o215.id=CASE WHEN json_valid(OLD.payload_json) THEN json_extract(OLD.payload_json,'$.mediation_record_id') END AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o215.run_id)))) OR (EXISTS (SELECT 1 FROM lead_conflict_mediations o216 WHERE o216.id=CASE WHEN json_valid(OLD.payload_json) THEN json_extract(OLD.payload_json,'$.lead_mediation_record_id') END AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o216.run_id)))) OR (EXISTS (SELECT 1 FROM agent_work_continuations o217 WHERE o217.id=CASE WHEN json_valid(OLD.payload_json) THEN json_extract(OLD.payload_json,'$.continuation_id') END AND ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o217.run_id)) OR (EXISTS (SELECT 1 FROM stage_executions o218 WHERE o218.id=o217.stage_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o218.run_id)))) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o219 WHERE o219.agent_execution_id=o217.agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o219.run_id)))) OR (EXISTS (SELECT 1 FROM session_generations o220 WHERE o220.id=o217.session_generation_id AND ((EXISTS (SELECT 1 FROM session_lineages o221 WHERE o221.id=o220.lineage_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o221.run_id)))) OR (f.source_run_id IN (SELECT run_id FROM p039_run_resources WHERE resource_key=o220.working_directory AND resource_key<>''))))) OR (EXISTS (SELECT 1 FROM provider_sessions o222 WHERE o222.provider_session_id=o217.provider_session_id AND ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o222.run_id)) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o223 WHERE o223.agent_execution_id=o222.agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o223.run_id)))))))))) OR (f.source_run_id IN (SELECT run_id FROM p039_run_resources WHERE resource_key=OLD.worktree_resource_key AND resource_key<>'')) OR (OLD.status='running' AND (NOT json_valid(OLD.payload_json) OR (CASE WHEN json_valid(OLD.payload_json) THEN json_extract(OLD.payload_json,'$.run_id') END IS NOT NULL AND NOT EXISTS (SELECT 1 FROM runs WHERE id=CASE WHEN json_valid(OLD.payload_json) THEN json_extract(OLD.payload_json,'$.run_id') END)) OR (CASE WHEN json_valid(OLD.payload_json) THEN json_extract(OLD.payload_json,'$.stage_execution_id') END IS NOT NULL AND NOT EXISTS (SELECT 1 FROM stage_executions WHERE id=CASE WHEN json_valid(OLD.payload_json) THEN json_extract(OLD.payload_json,'$.stage_execution_id') END)) OR (CASE WHEN json_valid(OLD.payload_json) THEN json_extract(OLD.payload_json,'$.agent_execution_id') END IS NOT NULL AND NOT EXISTS (SELECT 1 FROM p039_agent_run_owners WHERE agent_execution_id=CASE WHEN json_valid(OLD.payload_json) THEN json_extract(OLD.payload_json,'$.agent_execution_id') END)) OR (CASE WHEN json_valid(OLD.payload_json) THEN json_extract(OLD.payload_json,'$.mediation_record_id') END IS NOT NULL AND NOT EXISTS (SELECT 1 FROM lead_conflict_mediations WHERE id=CASE WHEN json_valid(OLD.payload_json) THEN json_extract(OLD.payload_json,'$.mediation_record_id') END)) OR (CASE WHEN json_valid(OLD.payload_json) THEN json_extract(OLD.payload_json,'$.lead_mediation_record_id') END IS NOT NULL AND NOT EXISTS (SELECT 1 FROM lead_conflict_mediations WHERE id=CASE WHEN json_valid(OLD.payload_json) THEN json_extract(OLD.payload_json,'$.lead_mediation_record_id') END)) OR (CASE WHEN json_valid(OLD.payload_json) THEN json_extract(OLD.payload_json,'$.continuation_id') END IS NOT NULL AND NOT EXISTS (SELECT 1 FROM agent_work_continuations WHERE id=CASE WHEN json_valid(OLD.payload_json) THEN json_extract(OLD.payload_json,'$.continuation_id') END)) OR (OLD.run_id IS NOT NULL AND NOT EXISTS (SELECT 1 FROM runs WHERE id=OLD.run_id)) OR (OLD.kind IN ('invoke_agent','settle_stage','advance_run','trigger_next_stage','process_continuation') AND OLD.run_id IS NULL AND CASE WHEN json_valid(OLD.payload_json) THEN json_extract(OLD.payload_json,'$.run_id') END IS NULL AND CASE WHEN json_valid(OLD.payload_json) THEN json_extract(OLD.payload_json,'$.stage_execution_id') END IS NULL AND CASE WHEN json_valid(OLD.payload_json) THEN json_extract(OLD.payload_json,'$.agent_execution_id') END IS NULL AND CASE WHEN json_valid(OLD.payload_json) THEN json_extract(OLD.payload_json,'$.continuation_id') END IS NULL)))) OR ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=NEW.run_id)) OR (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=CASE WHEN json_valid(NEW.payload_json) THEN json_extract(NEW.payload_json,'$.run_id') END)) OR (EXISTS (SELECT 1 FROM stage_executions o224 WHERE o224.id=CASE WHEN json_valid(NEW.payload_json) THEN json_extract(NEW.payload_json,'$.stage_execution_id') END AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o224.run_id)))) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o225 WHERE o225.agent_execution_id=CASE WHEN json_valid(NEW.payload_json) THEN json_extract(NEW.payload_json,'$.agent_execution_id') END AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o225.run_id)))) OR (EXISTS (SELECT 1 FROM lead_conflict_mediations o226 WHERE o226.id=CASE WHEN json_valid(NEW.payload_json) THEN json_extract(NEW.payload_json,'$.mediation_record_id') END AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o226.run_id)))) OR (EXISTS (SELECT 1 FROM lead_conflict_mediations o227 WHERE o227.id=CASE WHEN json_valid(NEW.payload_json) THEN json_extract(NEW.payload_json,'$.lead_mediation_record_id') END AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o227.run_id)))) OR (EXISTS (SELECT 1 FROM agent_work_continuations o228 WHERE o228.id=CASE WHEN json_valid(NEW.payload_json) THEN json_extract(NEW.payload_json,'$.continuation_id') END AND ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o228.run_id)) OR (EXISTS (SELECT 1 FROM stage_executions o229 WHERE o229.id=o228.stage_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o229.run_id)))) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o230 WHERE o230.agent_execution_id=o228.agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o230.run_id)))) OR (EXISTS (SELECT 1 FROM session_generations o231 WHERE o231.id=o228.session_generation_id AND ((EXISTS (SELECT 1 FROM session_lineages o232 WHERE o232.id=o231.lineage_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o232.run_id)))) OR (f.source_run_id IN (SELECT run_id FROM p039_run_resources WHERE resource_key=o231.working_directory AND resource_key<>''))))) OR (EXISTS (SELECT 1 FROM provider_sessions o233 WHERE o233.provider_session_id=o228.provider_session_id AND ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o233.run_id)) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o234 WHERE o234.agent_execution_id=o233.agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o234.run_id)))))))))) OR (f.source_run_id IN (SELECT run_id FROM p039_run_resources WHERE resource_key=NEW.worktree_resource_key AND resource_key<>'')) OR (NEW.status='running' AND (NOT json_valid(NEW.payload_json) OR (CASE WHEN json_valid(NEW.payload_json) THEN json_extract(NEW.payload_json,'$.run_id') END IS NOT NULL AND NOT EXISTS (SELECT 1 FROM runs WHERE id=CASE WHEN json_valid(NEW.payload_json) THEN json_extract(NEW.payload_json,'$.run_id') END)) OR (CASE WHEN json_valid(NEW.payload_json) THEN json_extract(NEW.payload_json,'$.stage_execution_id') END IS NOT NULL AND NOT EXISTS (SELECT 1 FROM stage_executions WHERE id=CASE WHEN json_valid(NEW.payload_json) THEN json_extract(NEW.payload_json,'$.stage_execution_id') END)) OR (CASE WHEN json_valid(NEW.payload_json) THEN json_extract(NEW.payload_json,'$.agent_execution_id') END IS NOT NULL AND NOT EXISTS (SELECT 1 FROM p039_agent_run_owners WHERE agent_execution_id=CASE WHEN json_valid(NEW.payload_json) THEN json_extract(NEW.payload_json,'$.agent_execution_id') END)) OR (CASE WHEN json_valid(NEW.payload_json) THEN json_extract(NEW.payload_json,'$.mediation_record_id') END IS NOT NULL AND NOT EXISTS (SELECT 1 FROM lead_conflict_mediations WHERE id=CASE WHEN json_valid(NEW.payload_json) THEN json_extract(NEW.payload_json,'$.mediation_record_id') END)) OR (CASE WHEN json_valid(NEW.payload_json) THEN json_extract(NEW.payload_json,'$.lead_mediation_record_id') END IS NOT NULL AND NOT EXISTS (SELECT 1 FROM lead_conflict_mediations WHERE id=CASE WHEN json_valid(NEW.payload_json) THEN json_extract(NEW.payload_json,'$.lead_mediation_record_id') END)) OR (CASE WHEN json_valid(NEW.payload_json) THEN json_extract(NEW.payload_json,'$.continuation_id') END IS NOT NULL AND NOT EXISTS (SELECT 1 FROM agent_work_continuations WHERE id=CASE WHEN json_valid(NEW.payload_json) THEN json_extract(NEW.payload_json,'$.continuation_id') END)) OR (NEW.run_id IS NOT NULL AND NOT EXISTS (SELECT 1 FROM runs WHERE id=NEW.run_id)) OR (NEW.kind IN ('invoke_agent','settle_stage','advance_run','trigger_next_stage','process_continuation') AND NEW.run_id IS NULL AND CASE WHEN json_valid(NEW.payload_json) THEN json_extract(NEW.payload_json,'$.run_id') END IS NULL AND CASE WHEN json_valid(NEW.payload_json) THEN json_extract(NEW.payload_json,'$.stage_execution_id') END IS NULL AND CASE WHEN json_valid(NEW.payload_json) THEN json_extract(NEW.payload_json,'$.agent_execution_id') END IS NULL AND CASE WHEN json_valid(NEW.payload_json) THEN json_extract(NEW.payload_json,'$.continuation_id') END IS NULL))))
        ORDER BY f.disposition LIMIT 1)
      WHEN 'historical' THEN RAISE(ABORT,'source_continued')
      WHEN 'reserved' THEN RAISE(ABORT,'continuation_in_progress') END;
END;

CREATE TRIGGER p039_guard_work_items_delete
BEFORE DELETE ON work_items
WHEN EXISTS (SELECT 1 FROM run_execution_fences)
BEGIN
    SELECT CASE (SELECT f.disposition FROM run_execution_fences f
        WHERE (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=OLD.run_id)) OR (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=CASE WHEN json_valid(OLD.payload_json) THEN json_extract(OLD.payload_json,'$.run_id') END)) OR (EXISTS (SELECT 1 FROM stage_executions o235 WHERE o235.id=CASE WHEN json_valid(OLD.payload_json) THEN json_extract(OLD.payload_json,'$.stage_execution_id') END AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o235.run_id)))) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o236 WHERE o236.agent_execution_id=CASE WHEN json_valid(OLD.payload_json) THEN json_extract(OLD.payload_json,'$.agent_execution_id') END AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o236.run_id)))) OR (EXISTS (SELECT 1 FROM lead_conflict_mediations o237 WHERE o237.id=CASE WHEN json_valid(OLD.payload_json) THEN json_extract(OLD.payload_json,'$.mediation_record_id') END AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o237.run_id)))) OR (EXISTS (SELECT 1 FROM lead_conflict_mediations o238 WHERE o238.id=CASE WHEN json_valid(OLD.payload_json) THEN json_extract(OLD.payload_json,'$.lead_mediation_record_id') END AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o238.run_id)))) OR (EXISTS (SELECT 1 FROM agent_work_continuations o239 WHERE o239.id=CASE WHEN json_valid(OLD.payload_json) THEN json_extract(OLD.payload_json,'$.continuation_id') END AND ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o239.run_id)) OR (EXISTS (SELECT 1 FROM stage_executions o240 WHERE o240.id=o239.stage_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o240.run_id)))) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o241 WHERE o241.agent_execution_id=o239.agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o241.run_id)))) OR (EXISTS (SELECT 1 FROM session_generations o242 WHERE o242.id=o239.session_generation_id AND ((EXISTS (SELECT 1 FROM session_lineages o243 WHERE o243.id=o242.lineage_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o243.run_id)))) OR (f.source_run_id IN (SELECT run_id FROM p039_run_resources WHERE resource_key=o242.working_directory AND resource_key<>''))))) OR (EXISTS (SELECT 1 FROM provider_sessions o244 WHERE o244.provider_session_id=o239.provider_session_id AND ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o244.run_id)) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o245 WHERE o245.agent_execution_id=o244.agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o245.run_id)))))))))) OR (f.source_run_id IN (SELECT run_id FROM p039_run_resources WHERE resource_key=OLD.worktree_resource_key AND resource_key<>'')) OR (OLD.status='running' AND (NOT json_valid(OLD.payload_json) OR (CASE WHEN json_valid(OLD.payload_json) THEN json_extract(OLD.payload_json,'$.run_id') END IS NOT NULL AND NOT EXISTS (SELECT 1 FROM runs WHERE id=CASE WHEN json_valid(OLD.payload_json) THEN json_extract(OLD.payload_json,'$.run_id') END)) OR (CASE WHEN json_valid(OLD.payload_json) THEN json_extract(OLD.payload_json,'$.stage_execution_id') END IS NOT NULL AND NOT EXISTS (SELECT 1 FROM stage_executions WHERE id=CASE WHEN json_valid(OLD.payload_json) THEN json_extract(OLD.payload_json,'$.stage_execution_id') END)) OR (CASE WHEN json_valid(OLD.payload_json) THEN json_extract(OLD.payload_json,'$.agent_execution_id') END IS NOT NULL AND NOT EXISTS (SELECT 1 FROM p039_agent_run_owners WHERE agent_execution_id=CASE WHEN json_valid(OLD.payload_json) THEN json_extract(OLD.payload_json,'$.agent_execution_id') END)) OR (CASE WHEN json_valid(OLD.payload_json) THEN json_extract(OLD.payload_json,'$.mediation_record_id') END IS NOT NULL AND NOT EXISTS (SELECT 1 FROM lead_conflict_mediations WHERE id=CASE WHEN json_valid(OLD.payload_json) THEN json_extract(OLD.payload_json,'$.mediation_record_id') END)) OR (CASE WHEN json_valid(OLD.payload_json) THEN json_extract(OLD.payload_json,'$.lead_mediation_record_id') END IS NOT NULL AND NOT EXISTS (SELECT 1 FROM lead_conflict_mediations WHERE id=CASE WHEN json_valid(OLD.payload_json) THEN json_extract(OLD.payload_json,'$.lead_mediation_record_id') END)) OR (CASE WHEN json_valid(OLD.payload_json) THEN json_extract(OLD.payload_json,'$.continuation_id') END IS NOT NULL AND NOT EXISTS (SELECT 1 FROM agent_work_continuations WHERE id=CASE WHEN json_valid(OLD.payload_json) THEN json_extract(OLD.payload_json,'$.continuation_id') END)) OR (OLD.run_id IS NOT NULL AND NOT EXISTS (SELECT 1 FROM runs WHERE id=OLD.run_id)) OR (OLD.kind IN ('invoke_agent','settle_stage','advance_run','trigger_next_stage','process_continuation') AND OLD.run_id IS NULL AND CASE WHEN json_valid(OLD.payload_json) THEN json_extract(OLD.payload_json,'$.run_id') END IS NULL AND CASE WHEN json_valid(OLD.payload_json) THEN json_extract(OLD.payload_json,'$.stage_execution_id') END IS NULL AND CASE WHEN json_valid(OLD.payload_json) THEN json_extract(OLD.payload_json,'$.agent_execution_id') END IS NULL AND CASE WHEN json_valid(OLD.payload_json) THEN json_extract(OLD.payload_json,'$.continuation_id') END IS NULL)))
        ORDER BY f.disposition LIMIT 1)
      WHEN 'historical' THEN RAISE(ABORT,'source_continued')
      WHEN 'reserved' THEN RAISE(ABORT,'continuation_in_progress') END;
END;

CREATE TRIGGER p039_guard_main_sync_attempts_insert
BEFORE INSERT ON main_sync_attempts
WHEN EXISTS (SELECT 1 FROM run_execution_fences)
BEGIN
    SELECT CASE (SELECT f.disposition FROM run_execution_fences f
        WHERE ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=NEW.run_id)) OR (EXISTS (SELECT 1 FROM worktree_mutation_barriers o246 WHERE o246.id=NEW.barrier_id AND ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o246.run_id)) OR (f.source_run_id IN (SELECT run_id FROM p039_run_resources WHERE resource_key=o246.worktree_resource_key AND resource_key<>'')))))) OR (EXISTS (SELECT 1 FROM main_sync_attempts stored WHERE ((stored.id=NEW.id) OR (stored.run_id=NEW.run_id AND stored.idempotency_key=NEW.idempotency_key)) AND ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=stored.run_id)) OR (EXISTS (SELECT 1 FROM worktree_mutation_barriers o247 WHERE o247.id=stored.barrier_id AND ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o247.run_id)) OR (f.source_run_id IN (SELECT run_id FROM p039_run_resources WHERE resource_key=o247.worktree_resource_key AND resource_key<>''))))))))
        ORDER BY f.disposition LIMIT 1)
      WHEN 'historical' THEN RAISE(ABORT,'source_continued')
      WHEN 'reserved' THEN RAISE(ABORT,'continuation_in_progress') END;
END;

CREATE TRIGGER p039_guard_main_sync_attempts_update
BEFORE UPDATE ON main_sync_attempts
WHEN EXISTS (SELECT 1 FROM run_execution_fences)
BEGIN
    SELECT CASE (SELECT f.disposition FROM run_execution_fences f
        WHERE ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=OLD.run_id)) OR (EXISTS (SELECT 1 FROM worktree_mutation_barriers o248 WHERE o248.id=OLD.barrier_id AND ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o248.run_id)) OR (f.source_run_id IN (SELECT run_id FROM p039_run_resources WHERE resource_key=o248.worktree_resource_key AND resource_key<>'')))))) OR ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=NEW.run_id)) OR (EXISTS (SELECT 1 FROM worktree_mutation_barriers o249 WHERE o249.id=NEW.barrier_id AND ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o249.run_id)) OR (f.source_run_id IN (SELECT run_id FROM p039_run_resources WHERE resource_key=o249.worktree_resource_key AND resource_key<>''))))))
        ORDER BY f.disposition LIMIT 1)
      WHEN 'historical' THEN RAISE(ABORT,'source_continued')
      WHEN 'reserved' THEN RAISE(ABORT,'continuation_in_progress') END;
END;

CREATE TRIGGER p039_guard_main_sync_attempts_delete
BEFORE DELETE ON main_sync_attempts
WHEN EXISTS (SELECT 1 FROM run_execution_fences)
BEGIN
    SELECT CASE (SELECT f.disposition FROM run_execution_fences f
        WHERE (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=OLD.run_id)) OR (EXISTS (SELECT 1 FROM worktree_mutation_barriers o250 WHERE o250.id=OLD.barrier_id AND ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o250.run_id)) OR (f.source_run_id IN (SELECT run_id FROM p039_run_resources WHERE resource_key=o250.worktree_resource_key AND resource_key<>'')))))
        ORDER BY f.disposition LIMIT 1)
      WHEN 'historical' THEN RAISE(ABORT,'source_continued')
      WHEN 'reserved' THEN RAISE(ABORT,'continuation_in_progress') END;
END;

CREATE TRIGGER p039_guard_worktree_mutation_barriers_insert
BEFORE INSERT ON worktree_mutation_barriers
WHEN EXISTS (SELECT 1 FROM run_execution_fences)
BEGIN
    SELECT CASE (SELECT f.disposition FROM run_execution_fences f
        WHERE ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=NEW.run_id)) OR (f.source_run_id IN (SELECT run_id FROM p039_run_resources WHERE resource_key=NEW.worktree_resource_key AND resource_key<>''))) OR (EXISTS (SELECT 1 FROM worktree_mutation_barriers stored WHERE ((stored.id=NEW.id)) AND ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=stored.run_id)) OR (f.source_run_id IN (SELECT run_id FROM p039_run_resources WHERE resource_key=stored.worktree_resource_key AND resource_key<>'')))))
        ORDER BY f.disposition LIMIT 1)
      WHEN 'historical' THEN RAISE(ABORT,'source_continued')
      WHEN 'reserved' THEN RAISE(ABORT,'continuation_in_progress') END;
END;

CREATE TRIGGER p039_guard_worktree_mutation_barriers_update
BEFORE UPDATE ON worktree_mutation_barriers
WHEN EXISTS (SELECT 1 FROM run_execution_fences)
BEGIN
    SELECT CASE (SELECT f.disposition FROM run_execution_fences f
        WHERE ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=OLD.run_id)) OR (f.source_run_id IN (SELECT run_id FROM p039_run_resources WHERE resource_key=OLD.worktree_resource_key AND resource_key<>''))) OR ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=NEW.run_id)) OR (f.source_run_id IN (SELECT run_id FROM p039_run_resources WHERE resource_key=NEW.worktree_resource_key AND resource_key<>'')))
        ORDER BY f.disposition LIMIT 1)
      WHEN 'historical' THEN RAISE(ABORT,'source_continued')
      WHEN 'reserved' THEN RAISE(ABORT,'continuation_in_progress') END;
END;

CREATE TRIGGER p039_guard_worktree_mutation_barriers_delete
BEFORE DELETE ON worktree_mutation_barriers
WHEN EXISTS (SELECT 1 FROM run_execution_fences)
BEGIN
    SELECT CASE (SELECT f.disposition FROM run_execution_fences f
        WHERE (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=OLD.run_id)) OR (f.source_run_id IN (SELECT run_id FROM p039_run_resources WHERE resource_key=OLD.worktree_resource_key AND resource_key<>''))
        ORDER BY f.disposition LIMIT 1)
      WHEN 'historical' THEN RAISE(ABORT,'source_continued')
      WHEN 'reserved' THEN RAISE(ABORT,'continuation_in_progress') END;
END;

CREATE TRIGGER p039_guard_background_leases_insert
BEFORE INSERT ON background_leases
WHEN EXISTS (SELECT 1 FROM run_execution_fences)
BEGIN
    SELECT CASE (SELECT f.disposition FROM run_execution_fences f
        WHERE ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=NEW.run_id)) OR (f.source_run_id IN (SELECT run_id FROM p039_run_resources WHERE resource_key=NEW.worktree_resource_key AND resource_key<>'')) OR (f.source_run_id IN (SELECT run_id FROM p039_run_resources WHERE resource_key=NEW.resource_key AND resource_key<>'')) OR (NEW.released_at IS NULL AND NOT EXISTS (SELECT 1 FROM runs WHERE id=NEW.run_id) AND NOT EXISTS (SELECT 1 FROM p039_run_resources WHERE resource_key IN (NEW.resource_key,NEW.worktree_resource_key) AND resource_key<>''))) OR (EXISTS (SELECT 1 FROM background_leases stored WHERE ((stored.id=NEW.id) OR (stored.resource_key=NEW.resource_key)) AND ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=stored.run_id)) OR (f.source_run_id IN (SELECT run_id FROM p039_run_resources WHERE resource_key=stored.worktree_resource_key AND resource_key<>'')) OR (f.source_run_id IN (SELECT run_id FROM p039_run_resources WHERE resource_key=stored.resource_key AND resource_key<>'')) OR (stored.released_at IS NULL AND NOT EXISTS (SELECT 1 FROM runs WHERE id=stored.run_id) AND NOT EXISTS (SELECT 1 FROM p039_run_resources WHERE resource_key IN (stored.resource_key,stored.worktree_resource_key) AND resource_key<>'')))))
        ORDER BY f.disposition LIMIT 1)
      WHEN 'historical' THEN RAISE(ABORT,'source_continued')
      WHEN 'reserved' THEN RAISE(ABORT,'continuation_in_progress') END;
END;

CREATE TRIGGER p039_guard_background_leases_update
BEFORE UPDATE ON background_leases
WHEN EXISTS (SELECT 1 FROM run_execution_fences)
BEGIN
    SELECT CASE (SELECT f.disposition FROM run_execution_fences f
        WHERE ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=OLD.run_id)) OR (f.source_run_id IN (SELECT run_id FROM p039_run_resources WHERE resource_key=OLD.worktree_resource_key AND resource_key<>'')) OR (f.source_run_id IN (SELECT run_id FROM p039_run_resources WHERE resource_key=OLD.resource_key AND resource_key<>'')) OR (OLD.released_at IS NULL AND NOT EXISTS (SELECT 1 FROM runs WHERE id=OLD.run_id) AND NOT EXISTS (SELECT 1 FROM p039_run_resources WHERE resource_key IN (OLD.resource_key,OLD.worktree_resource_key) AND resource_key<>''))) OR ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=NEW.run_id)) OR (f.source_run_id IN (SELECT run_id FROM p039_run_resources WHERE resource_key=NEW.worktree_resource_key AND resource_key<>'')) OR (f.source_run_id IN (SELECT run_id FROM p039_run_resources WHERE resource_key=NEW.resource_key AND resource_key<>'')) OR (NEW.released_at IS NULL AND NOT EXISTS (SELECT 1 FROM runs WHERE id=NEW.run_id) AND NOT EXISTS (SELECT 1 FROM p039_run_resources WHERE resource_key IN (NEW.resource_key,NEW.worktree_resource_key) AND resource_key<>'')))
        ORDER BY f.disposition LIMIT 1)
      WHEN 'historical' THEN RAISE(ABORT,'source_continued')
      WHEN 'reserved' THEN RAISE(ABORT,'continuation_in_progress') END;
END;

CREATE TRIGGER p039_guard_background_leases_delete
BEFORE DELETE ON background_leases
WHEN EXISTS (SELECT 1 FROM run_execution_fences)
BEGIN
    SELECT CASE (SELECT f.disposition FROM run_execution_fences f
        WHERE (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=OLD.run_id)) OR (f.source_run_id IN (SELECT run_id FROM p039_run_resources WHERE resource_key=OLD.worktree_resource_key AND resource_key<>'')) OR (f.source_run_id IN (SELECT run_id FROM p039_run_resources WHERE resource_key=OLD.resource_key AND resource_key<>'')) OR (OLD.released_at IS NULL AND NOT EXISTS (SELECT 1 FROM runs WHERE id=OLD.run_id) AND NOT EXISTS (SELECT 1 FROM p039_run_resources WHERE resource_key IN (OLD.resource_key,OLD.worktree_resource_key) AND resource_key<>''))
        ORDER BY f.disposition LIMIT 1)
      WHEN 'historical' THEN RAISE(ABORT,'source_continued')
      WHEN 'reserved' THEN RAISE(ABORT,'continuation_in_progress') END;
END;

-- Timestamp-only observation may proceed; process and identity fields remain fenced.

CREATE TRIGGER p039_guard_provider_sessions_insert
BEFORE INSERT ON provider_sessions
WHEN EXISTS (SELECT 1 FROM run_execution_fences)
BEGIN
    SELECT CASE (SELECT f.disposition FROM run_execution_fences f
        WHERE ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=NEW.run_id)) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o251 WHERE o251.agent_execution_id=NEW.agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o251.run_id))))) OR (EXISTS (SELECT 1 FROM provider_sessions stored WHERE ((stored.provider_session_id=NEW.provider_session_id)) AND ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=stored.run_id)) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o252 WHERE o252.agent_execution_id=stored.agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o252.run_id)))))))
        ORDER BY f.disposition LIMIT 1)
      WHEN 'historical' THEN RAISE(ABORT,'source_continued')
      WHEN 'reserved' THEN RAISE(ABORT,'continuation_in_progress') END;
END;

CREATE TRIGGER p039_guard_provider_sessions_update
BEFORE UPDATE OF provider_session_id,run_id,agent_execution_id,provider,lifecycle_state,process_id,process_start_identity,process_fate,process_fate_updated_at,created_at ON provider_sessions
WHEN EXISTS (SELECT 1 FROM run_execution_fences)
BEGIN
    SELECT CASE (SELECT f.disposition FROM run_execution_fences f
        WHERE ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=OLD.run_id)) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o253 WHERE o253.agent_execution_id=OLD.agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o253.run_id))))) OR ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=NEW.run_id)) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o254 WHERE o254.agent_execution_id=NEW.agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o254.run_id)))))
        ORDER BY f.disposition LIMIT 1)
      WHEN 'historical' THEN RAISE(ABORT,'source_continued')
      WHEN 'reserved' THEN RAISE(ABORT,'continuation_in_progress') END;
END;

CREATE TRIGGER p039_guard_provider_sessions_delete
BEFORE DELETE ON provider_sessions
WHEN EXISTS (SELECT 1 FROM run_execution_fences)
BEGIN
    SELECT CASE (SELECT f.disposition FROM run_execution_fences f
        WHERE (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=OLD.run_id)) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o255 WHERE o255.agent_execution_id=OLD.agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o255.run_id))))
        ORDER BY f.disposition LIMIT 1)
      WHEN 'historical' THEN RAISE(ABORT,'source_continued')
      WHEN 'reserved' THEN RAISE(ABORT,'continuation_in_progress') END;
END;

CREATE TRIGGER p039_guard_provider_cancellation_intents_insert
BEFORE INSERT ON provider_cancellation_intents
WHEN EXISTS (SELECT 1 FROM run_execution_fences)
BEGIN
    SELECT CASE (SELECT f.disposition FROM run_execution_fences f
        WHERE ((EXISTS (SELECT 1 FROM provider_sessions o256 WHERE o256.provider_session_id=NEW.provider_session_id AND ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o256.run_id)) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o257 WHERE o257.agent_execution_id=o256.agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o257.run_id))))))) OR (NEW.intent_state<>'settled' AND NOT EXISTS (SELECT 1 FROM provider_sessions WHERE provider_session_id=NEW.provider_session_id))) OR (EXISTS (SELECT 1 FROM provider_cancellation_intents stored WHERE ((stored.provider_session_id=NEW.provider_session_id AND stored.cancellation_epoch=NEW.cancellation_epoch)) AND ((EXISTS (SELECT 1 FROM provider_sessions o258 WHERE o258.provider_session_id=stored.provider_session_id AND ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o258.run_id)) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o259 WHERE o259.agent_execution_id=o258.agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o259.run_id))))))) OR (stored.intent_state<>'settled' AND NOT EXISTS (SELECT 1 FROM provider_sessions WHERE provider_session_id=stored.provider_session_id)))))
        ORDER BY f.disposition LIMIT 1)
      WHEN 'historical' THEN RAISE(ABORT,'source_continued')
      WHEN 'reserved' THEN RAISE(ABORT,'continuation_in_progress') END;
END;

CREATE TRIGGER p039_guard_provider_cancellation_intents_update
BEFORE UPDATE ON provider_cancellation_intents
WHEN EXISTS (SELECT 1 FROM run_execution_fences)
BEGIN
    SELECT CASE (SELECT f.disposition FROM run_execution_fences f
        WHERE ((EXISTS (SELECT 1 FROM provider_sessions o260 WHERE o260.provider_session_id=OLD.provider_session_id AND ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o260.run_id)) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o261 WHERE o261.agent_execution_id=o260.agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o261.run_id))))))) OR (OLD.intent_state<>'settled' AND NOT EXISTS (SELECT 1 FROM provider_sessions WHERE provider_session_id=OLD.provider_session_id))) OR ((EXISTS (SELECT 1 FROM provider_sessions o262 WHERE o262.provider_session_id=NEW.provider_session_id AND ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o262.run_id)) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o263 WHERE o263.agent_execution_id=o262.agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o263.run_id))))))) OR (NEW.intent_state<>'settled' AND NOT EXISTS (SELECT 1 FROM provider_sessions WHERE provider_session_id=NEW.provider_session_id)))
        ORDER BY f.disposition LIMIT 1)
      WHEN 'historical' THEN RAISE(ABORT,'source_continued')
      WHEN 'reserved' THEN RAISE(ABORT,'continuation_in_progress') END;
END;

CREATE TRIGGER p039_guard_provider_cancellation_intents_delete
BEFORE DELETE ON provider_cancellation_intents
WHEN EXISTS (SELECT 1 FROM run_execution_fences)
BEGIN
    SELECT CASE (SELECT f.disposition FROM run_execution_fences f
        WHERE (EXISTS (SELECT 1 FROM provider_sessions o264 WHERE o264.provider_session_id=OLD.provider_session_id AND ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o264.run_id)) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o265 WHERE o265.agent_execution_id=o264.agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o265.run_id))))))) OR (OLD.intent_state<>'settled' AND NOT EXISTS (SELECT 1 FROM provider_sessions WHERE provider_session_id=OLD.provider_session_id))
        ORDER BY f.disposition LIMIT 1)
      WHEN 'historical' THEN RAISE(ABORT,'source_continued')
      WHEN 'reserved' THEN RAISE(ABORT,'continuation_in_progress') END;
END;

CREATE TRIGGER p039_guard_shutdown_signal_side_effects_insert
BEFORE INSERT ON shutdown_signal_side_effects
WHEN EXISTS (SELECT 1 FROM run_execution_fences)
BEGIN
    SELECT CASE (SELECT f.disposition FROM run_execution_fences f
        WHERE ((EXISTS (SELECT 1 FROM provider_sessions o266 WHERE o266.provider_session_id=NEW.provider_session_id AND ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o266.run_id)) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o267 WHERE o267.agent_execution_id=o266.agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o267.run_id))))))) OR (NEW.intent_state NOT IN ('observed','suppressed') AND NOT EXISTS (SELECT 1 FROM provider_sessions WHERE provider_session_id=NEW.provider_session_id))) OR (EXISTS (SELECT 1 FROM shutdown_signal_side_effects stored WHERE ((stored.signal_effect_id=NEW.signal_effect_id) OR (stored.provider_session_id=NEW.provider_session_id AND stored.shutdown_epoch=NEW.shutdown_epoch AND stored.signal_kind=NEW.signal_kind AND stored.generation=NEW.generation)) AND ((EXISTS (SELECT 1 FROM provider_sessions o268 WHERE o268.provider_session_id=stored.provider_session_id AND ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o268.run_id)) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o269 WHERE o269.agent_execution_id=o268.agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o269.run_id))))))) OR (stored.intent_state NOT IN ('observed','suppressed') AND NOT EXISTS (SELECT 1 FROM provider_sessions WHERE provider_session_id=stored.provider_session_id)))))
        ORDER BY f.disposition LIMIT 1)
      WHEN 'historical' THEN RAISE(ABORT,'source_continued')
      WHEN 'reserved' THEN RAISE(ABORT,'continuation_in_progress') END;
END;

CREATE TRIGGER p039_guard_shutdown_signal_side_effects_update
BEFORE UPDATE ON shutdown_signal_side_effects
WHEN EXISTS (SELECT 1 FROM run_execution_fences)
BEGIN
    SELECT CASE (SELECT f.disposition FROM run_execution_fences f
        WHERE ((EXISTS (SELECT 1 FROM provider_sessions o270 WHERE o270.provider_session_id=OLD.provider_session_id AND ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o270.run_id)) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o271 WHERE o271.agent_execution_id=o270.agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o271.run_id))))))) OR (OLD.intent_state NOT IN ('observed','suppressed') AND NOT EXISTS (SELECT 1 FROM provider_sessions WHERE provider_session_id=OLD.provider_session_id))) OR ((EXISTS (SELECT 1 FROM provider_sessions o272 WHERE o272.provider_session_id=NEW.provider_session_id AND ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o272.run_id)) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o273 WHERE o273.agent_execution_id=o272.agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o273.run_id))))))) OR (NEW.intent_state NOT IN ('observed','suppressed') AND NOT EXISTS (SELECT 1 FROM provider_sessions WHERE provider_session_id=NEW.provider_session_id)))
        ORDER BY f.disposition LIMIT 1)
      WHEN 'historical' THEN RAISE(ABORT,'source_continued')
      WHEN 'reserved' THEN RAISE(ABORT,'continuation_in_progress') END;
END;

CREATE TRIGGER p039_guard_shutdown_signal_side_effects_delete
BEFORE DELETE ON shutdown_signal_side_effects
WHEN EXISTS (SELECT 1 FROM run_execution_fences)
BEGIN
    SELECT CASE (SELECT f.disposition FROM run_execution_fences f
        WHERE (EXISTS (SELECT 1 FROM provider_sessions o274 WHERE o274.provider_session_id=OLD.provider_session_id AND ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o274.run_id)) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o275 WHERE o275.agent_execution_id=o274.agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o275.run_id))))))) OR (OLD.intent_state NOT IN ('observed','suppressed') AND NOT EXISTS (SELECT 1 FROM provider_sessions WHERE provider_session_id=OLD.provider_session_id))
        ORDER BY f.disposition LIMIT 1)
      WHEN 'historical' THEN RAISE(ABORT,'source_continued')
      WHEN 'reserved' THEN RAISE(ABORT,'continuation_in_progress') END;
END;

CREATE TRIGGER p039_guard_agent_external_side_effect_ledger_insert
BEFORE INSERT ON agent_external_side_effect_ledger
WHEN EXISTS (SELECT 1 FROM run_execution_fences)
BEGIN
    SELECT CASE (SELECT f.disposition FROM run_execution_fences f
        WHERE (EXISTS (SELECT 1 FROM agent_work_continuations o276 WHERE o276.id=NEW.continuation_id AND ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o276.run_id)) OR (EXISTS (SELECT 1 FROM stage_executions o277 WHERE o277.id=o276.stage_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o277.run_id)))) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o278 WHERE o278.agent_execution_id=o276.agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o278.run_id)))) OR (EXISTS (SELECT 1 FROM session_generations o279 WHERE o279.id=o276.session_generation_id AND ((EXISTS (SELECT 1 FROM session_lineages o280 WHERE o280.id=o279.lineage_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o280.run_id)))) OR (f.source_run_id IN (SELECT run_id FROM p039_run_resources WHERE resource_key=o279.working_directory AND resource_key<>''))))) OR (EXISTS (SELECT 1 FROM provider_sessions o281 WHERE o281.provider_session_id=o276.provider_session_id AND ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o281.run_id)) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o282 WHERE o282.agent_execution_id=o281.agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o282.run_id)))))))))) OR (EXISTS (SELECT 1 FROM agent_external_side_effect_ledger stored WHERE ((stored.continuation_id=NEW.continuation_id AND stored.sequence_number=NEW.sequence_number)) AND (EXISTS (SELECT 1 FROM agent_work_continuations o283 WHERE o283.id=stored.continuation_id AND ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o283.run_id)) OR (EXISTS (SELECT 1 FROM stage_executions o284 WHERE o284.id=o283.stage_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o284.run_id)))) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o285 WHERE o285.agent_execution_id=o283.agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o285.run_id)))) OR (EXISTS (SELECT 1 FROM session_generations o286 WHERE o286.id=o283.session_generation_id AND ((EXISTS (SELECT 1 FROM session_lineages o287 WHERE o287.id=o286.lineage_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o287.run_id)))) OR (f.source_run_id IN (SELECT run_id FROM p039_run_resources WHERE resource_key=o286.working_directory AND resource_key<>''))))) OR (EXISTS (SELECT 1 FROM provider_sessions o288 WHERE o288.provider_session_id=o283.provider_session_id AND ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o288.run_id)) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o289 WHERE o289.agent_execution_id=o288.agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o289.run_id))))))))))))
        ORDER BY f.disposition LIMIT 1)
      WHEN 'historical' THEN RAISE(ABORT,'source_continued')
      WHEN 'reserved' THEN RAISE(ABORT,'continuation_in_progress') END;
END;

CREATE TRIGGER p039_guard_agent_external_side_effect_ledger_update
BEFORE UPDATE ON agent_external_side_effect_ledger
WHEN EXISTS (SELECT 1 FROM run_execution_fences)
BEGIN
    SELECT CASE (SELECT f.disposition FROM run_execution_fences f
        WHERE (EXISTS (SELECT 1 FROM agent_work_continuations o290 WHERE o290.id=OLD.continuation_id AND ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o290.run_id)) OR (EXISTS (SELECT 1 FROM stage_executions o291 WHERE o291.id=o290.stage_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o291.run_id)))) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o292 WHERE o292.agent_execution_id=o290.agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o292.run_id)))) OR (EXISTS (SELECT 1 FROM session_generations o293 WHERE o293.id=o290.session_generation_id AND ((EXISTS (SELECT 1 FROM session_lineages o294 WHERE o294.id=o293.lineage_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o294.run_id)))) OR (f.source_run_id IN (SELECT run_id FROM p039_run_resources WHERE resource_key=o293.working_directory AND resource_key<>''))))) OR (EXISTS (SELECT 1 FROM provider_sessions o295 WHERE o295.provider_session_id=o290.provider_session_id AND ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o295.run_id)) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o296 WHERE o296.agent_execution_id=o295.agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o296.run_id)))))))))) OR (EXISTS (SELECT 1 FROM agent_work_continuations o297 WHERE o297.id=NEW.continuation_id AND ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o297.run_id)) OR (EXISTS (SELECT 1 FROM stage_executions o298 WHERE o298.id=o297.stage_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o298.run_id)))) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o299 WHERE o299.agent_execution_id=o297.agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o299.run_id)))) OR (EXISTS (SELECT 1 FROM session_generations o300 WHERE o300.id=o297.session_generation_id AND ((EXISTS (SELECT 1 FROM session_lineages o301 WHERE o301.id=o300.lineage_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o301.run_id)))) OR (f.source_run_id IN (SELECT run_id FROM p039_run_resources WHERE resource_key=o300.working_directory AND resource_key<>''))))) OR (EXISTS (SELECT 1 FROM provider_sessions o302 WHERE o302.provider_session_id=o297.provider_session_id AND ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o302.run_id)) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o303 WHERE o303.agent_execution_id=o302.agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o303.run_id))))))))))
        ORDER BY f.disposition LIMIT 1)
      WHEN 'historical' THEN RAISE(ABORT,'source_continued')
      WHEN 'reserved' THEN RAISE(ABORT,'continuation_in_progress') END;
END;

CREATE TRIGGER p039_guard_agent_external_side_effect_ledger_delete
BEFORE DELETE ON agent_external_side_effect_ledger
WHEN EXISTS (SELECT 1 FROM run_execution_fences)
BEGIN
    SELECT CASE (SELECT f.disposition FROM run_execution_fences f
        WHERE EXISTS (SELECT 1 FROM agent_work_continuations o304 WHERE o304.id=OLD.continuation_id AND ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o304.run_id)) OR (EXISTS (SELECT 1 FROM stage_executions o305 WHERE o305.id=o304.stage_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o305.run_id)))) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o306 WHERE o306.agent_execution_id=o304.agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o306.run_id)))) OR (EXISTS (SELECT 1 FROM session_generations o307 WHERE o307.id=o304.session_generation_id AND ((EXISTS (SELECT 1 FROM session_lineages o308 WHERE o308.id=o307.lineage_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o308.run_id)))) OR (f.source_run_id IN (SELECT run_id FROM p039_run_resources WHERE resource_key=o307.working_directory AND resource_key<>''))))) OR (EXISTS (SELECT 1 FROM provider_sessions o309 WHERE o309.provider_session_id=o304.provider_session_id AND ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o309.run_id)) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o310 WHERE o310.agent_execution_id=o309.agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o310.run_id)))))))))
        ORDER BY f.disposition LIMIT 1)
      WHEN 'historical' THEN RAISE(ABORT,'source_continued')
      WHEN 'reserved' THEN RAISE(ABORT,'continuation_in_progress') END;
END;

CREATE TRIGGER p039_guard_supervised_workers_continuation_insert
BEFORE INSERT ON supervised_workers_continuation
WHEN EXISTS (SELECT 1 FROM run_execution_fences)
BEGIN
    SELECT CASE (SELECT f.disposition FROM run_execution_fences f
        WHERE ((EXISTS (SELECT 1 FROM agent_work_continuations o311 WHERE o311.id=NEW.continuation_id AND ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o311.run_id)) OR (EXISTS (SELECT 1 FROM stage_executions o312 WHERE o312.id=o311.stage_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o312.run_id)))) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o313 WHERE o313.agent_execution_id=o311.agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o313.run_id)))) OR (EXISTS (SELECT 1 FROM session_generations o314 WHERE o314.id=o311.session_generation_id AND ((EXISTS (SELECT 1 FROM session_lineages o315 WHERE o315.id=o314.lineage_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o315.run_id)))) OR (f.source_run_id IN (SELECT run_id FROM p039_run_resources WHERE resource_key=o314.working_directory AND resource_key<>''))))) OR (EXISTS (SELECT 1 FROM provider_sessions o316 WHERE o316.provider_session_id=o311.provider_session_id AND ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o316.run_id)) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o317 WHERE o317.agent_execution_id=o316.agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o317.run_id)))))))))) OR (EXISTS (SELECT 1 FROM session_generations o318 WHERE o318.id=NEW.session_generation_id AND ((EXISTS (SELECT 1 FROM session_lineages o319 WHERE o319.id=o318.lineage_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o319.run_id)))) OR (f.source_run_id IN (SELECT run_id FROM p039_run_resources WHERE resource_key=o318.working_directory AND resource_key<>'')))))) OR (EXISTS (SELECT 1 FROM supervised_workers_continuation stored WHERE ((stored.continuation_id=NEW.continuation_id AND stored.worker_pid=NEW.worker_pid AND stored.daemon_generation_id=NEW.daemon_generation_id)) AND ((EXISTS (SELECT 1 FROM agent_work_continuations o320 WHERE o320.id=stored.continuation_id AND ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o320.run_id)) OR (EXISTS (SELECT 1 FROM stage_executions o321 WHERE o321.id=o320.stage_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o321.run_id)))) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o322 WHERE o322.agent_execution_id=o320.agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o322.run_id)))) OR (EXISTS (SELECT 1 FROM session_generations o323 WHERE o323.id=o320.session_generation_id AND ((EXISTS (SELECT 1 FROM session_lineages o324 WHERE o324.id=o323.lineage_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o324.run_id)))) OR (f.source_run_id IN (SELECT run_id FROM p039_run_resources WHERE resource_key=o323.working_directory AND resource_key<>''))))) OR (EXISTS (SELECT 1 FROM provider_sessions o325 WHERE o325.provider_session_id=o320.provider_session_id AND ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o325.run_id)) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o326 WHERE o326.agent_execution_id=o325.agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o326.run_id)))))))))) OR (EXISTS (SELECT 1 FROM session_generations o327 WHERE o327.id=stored.session_generation_id AND ((EXISTS (SELECT 1 FROM session_lineages o328 WHERE o328.id=o327.lineage_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o328.run_id)))) OR (f.source_run_id IN (SELECT run_id FROM p039_run_resources WHERE resource_key=o327.working_directory AND resource_key<>''))))))))
        ORDER BY f.disposition LIMIT 1)
      WHEN 'historical' THEN RAISE(ABORT,'source_continued')
      WHEN 'reserved' THEN RAISE(ABORT,'continuation_in_progress') END;
END;

CREATE TRIGGER p039_guard_supervised_workers_continuation_update
BEFORE UPDATE ON supervised_workers_continuation
WHEN EXISTS (SELECT 1 FROM run_execution_fences)
BEGIN
    SELECT CASE (SELECT f.disposition FROM run_execution_fences f
        WHERE ((EXISTS (SELECT 1 FROM agent_work_continuations o329 WHERE o329.id=OLD.continuation_id AND ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o329.run_id)) OR (EXISTS (SELECT 1 FROM stage_executions o330 WHERE o330.id=o329.stage_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o330.run_id)))) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o331 WHERE o331.agent_execution_id=o329.agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o331.run_id)))) OR (EXISTS (SELECT 1 FROM session_generations o332 WHERE o332.id=o329.session_generation_id AND ((EXISTS (SELECT 1 FROM session_lineages o333 WHERE o333.id=o332.lineage_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o333.run_id)))) OR (f.source_run_id IN (SELECT run_id FROM p039_run_resources WHERE resource_key=o332.working_directory AND resource_key<>''))))) OR (EXISTS (SELECT 1 FROM provider_sessions o334 WHERE o334.provider_session_id=o329.provider_session_id AND ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o334.run_id)) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o335 WHERE o335.agent_execution_id=o334.agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o335.run_id)))))))))) OR (EXISTS (SELECT 1 FROM session_generations o336 WHERE o336.id=OLD.session_generation_id AND ((EXISTS (SELECT 1 FROM session_lineages o337 WHERE o337.id=o336.lineage_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o337.run_id)))) OR (f.source_run_id IN (SELECT run_id FROM p039_run_resources WHERE resource_key=o336.working_directory AND resource_key<>'')))))) OR ((EXISTS (SELECT 1 FROM agent_work_continuations o338 WHERE o338.id=NEW.continuation_id AND ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o338.run_id)) OR (EXISTS (SELECT 1 FROM stage_executions o339 WHERE o339.id=o338.stage_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o339.run_id)))) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o340 WHERE o340.agent_execution_id=o338.agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o340.run_id)))) OR (EXISTS (SELECT 1 FROM session_generations o341 WHERE o341.id=o338.session_generation_id AND ((EXISTS (SELECT 1 FROM session_lineages o342 WHERE o342.id=o341.lineage_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o342.run_id)))) OR (f.source_run_id IN (SELECT run_id FROM p039_run_resources WHERE resource_key=o341.working_directory AND resource_key<>''))))) OR (EXISTS (SELECT 1 FROM provider_sessions o343 WHERE o343.provider_session_id=o338.provider_session_id AND ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o343.run_id)) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o344 WHERE o344.agent_execution_id=o343.agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o344.run_id)))))))))) OR (EXISTS (SELECT 1 FROM session_generations o345 WHERE o345.id=NEW.session_generation_id AND ((EXISTS (SELECT 1 FROM session_lineages o346 WHERE o346.id=o345.lineage_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o346.run_id)))) OR (f.source_run_id IN (SELECT run_id FROM p039_run_resources WHERE resource_key=o345.working_directory AND resource_key<>''))))))
        ORDER BY f.disposition LIMIT 1)
      WHEN 'historical' THEN RAISE(ABORT,'source_continued')
      WHEN 'reserved' THEN RAISE(ABORT,'continuation_in_progress') END;
END;

CREATE TRIGGER p039_guard_supervised_workers_continuation_delete
BEFORE DELETE ON supervised_workers_continuation
WHEN EXISTS (SELECT 1 FROM run_execution_fences)
BEGIN
    SELECT CASE (SELECT f.disposition FROM run_execution_fences f
        WHERE (EXISTS (SELECT 1 FROM agent_work_continuations o347 WHERE o347.id=OLD.continuation_id AND ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o347.run_id)) OR (EXISTS (SELECT 1 FROM stage_executions o348 WHERE o348.id=o347.stage_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o348.run_id)))) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o349 WHERE o349.agent_execution_id=o347.agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o349.run_id)))) OR (EXISTS (SELECT 1 FROM session_generations o350 WHERE o350.id=o347.session_generation_id AND ((EXISTS (SELECT 1 FROM session_lineages o351 WHERE o351.id=o350.lineage_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o351.run_id)))) OR (f.source_run_id IN (SELECT run_id FROM p039_run_resources WHERE resource_key=o350.working_directory AND resource_key<>''))))) OR (EXISTS (SELECT 1 FROM provider_sessions o352 WHERE o352.provider_session_id=o347.provider_session_id AND ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o352.run_id)) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o353 WHERE o353.agent_execution_id=o352.agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o353.run_id)))))))))) OR (EXISTS (SELECT 1 FROM session_generations o354 WHERE o354.id=OLD.session_generation_id AND ((EXISTS (SELECT 1 FROM session_lineages o355 WHERE o355.id=o354.lineage_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o355.run_id)))) OR (f.source_run_id IN (SELECT run_id FROM p039_run_resources WHERE resource_key=o354.working_directory AND resource_key<>'')))))
        ORDER BY f.disposition LIMIT 1)
      WHEN 'historical' THEN RAISE(ABORT,'source_continued')
      WHEN 'reserved' THEN RAISE(ABORT,'continuation_in_progress') END;
END;

CREATE TRIGGER p039_guard_side_effects_insert
BEFORE INSERT ON side_effects
WHEN EXISTS (SELECT 1 FROM run_execution_fences)
BEGIN
    SELECT CASE (SELECT f.disposition FROM run_execution_fences f
        WHERE ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=NEW.run_id)) OR (EXISTS (SELECT 1 FROM stage_executions o356 WHERE o356.id=NEW.stage_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o356.run_id)))) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o357 WHERE o357.agent_execution_id=NEW.agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o357.run_id))))) OR (EXISTS (SELECT 1 FROM side_effects stored WHERE ((stored.id=NEW.id) OR (stored.idempotency_key=NEW.idempotency_key)) AND ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=stored.run_id)) OR (EXISTS (SELECT 1 FROM stage_executions o358 WHERE o358.id=stored.stage_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o358.run_id)))) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o359 WHERE o359.agent_execution_id=stored.agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o359.run_id)))))))
        ORDER BY f.disposition LIMIT 1)
      WHEN 'historical' THEN RAISE(ABORT,'source_continued')
      WHEN 'reserved' THEN RAISE(ABORT,'continuation_in_progress') END;
END;

CREATE TRIGGER p039_guard_side_effects_update
BEFORE UPDATE ON side_effects
WHEN EXISTS (SELECT 1 FROM run_execution_fences)
BEGIN
    SELECT CASE (SELECT f.disposition FROM run_execution_fences f
        WHERE ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=OLD.run_id)) OR (EXISTS (SELECT 1 FROM stage_executions o360 WHERE o360.id=OLD.stage_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o360.run_id)))) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o361 WHERE o361.agent_execution_id=OLD.agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o361.run_id))))) OR ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=NEW.run_id)) OR (EXISTS (SELECT 1 FROM stage_executions o362 WHERE o362.id=NEW.stage_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o362.run_id)))) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o363 WHERE o363.agent_execution_id=NEW.agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o363.run_id)))))
        ORDER BY f.disposition LIMIT 1)
      WHEN 'historical' THEN RAISE(ABORT,'source_continued')
      WHEN 'reserved' THEN RAISE(ABORT,'continuation_in_progress') END;
END;

CREATE TRIGGER p039_guard_side_effects_delete
BEFORE DELETE ON side_effects
WHEN EXISTS (SELECT 1 FROM run_execution_fences)
BEGIN
    SELECT CASE (SELECT f.disposition FROM run_execution_fences f
        WHERE (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=OLD.run_id)) OR (EXISTS (SELECT 1 FROM stage_executions o364 WHERE o364.id=OLD.stage_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o364.run_id)))) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o365 WHERE o365.agent_execution_id=OLD.agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o365.run_id))))
        ORDER BY f.disposition LIMIT 1)
      WHEN 'historical' THEN RAISE(ABORT,'source_continued')
      WHEN 'reserved' THEN RAISE(ABORT,'continuation_in_progress') END;
END;

-- Readback may append/complete observations, never change identity/kind or replace evidence.

CREATE TRIGGER p039_guard_side_effect_attempts_insert
BEFORE INSERT ON side_effect_attempts
WHEN EXISTS (SELECT 1 FROM run_execution_fences)
BEGIN
    SELECT CASE (SELECT f.disposition FROM run_execution_fences f
        WHERE ((NEW.attempt_kind<>'readback') AND (EXISTS (SELECT 1 FROM side_effects o366 WHERE o366.id=NEW.side_effect_id AND ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o366.run_id)) OR (EXISTS (SELECT 1 FROM stage_executions o367 WHERE o367.id=o366.stage_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o367.run_id)))) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o368 WHERE o368.agent_execution_id=o366.agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o368.run_id)))))))) OR (EXISTS (SELECT 1 FROM side_effect_attempts stored WHERE ((stored.id=NEW.id)) AND (EXISTS (SELECT 1 FROM side_effects o369 WHERE o369.id=stored.side_effect_id AND ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o369.run_id)) OR (EXISTS (SELECT 1 FROM stage_executions o370 WHERE o370.id=o369.stage_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o370.run_id)))) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o371 WHERE o371.agent_execution_id=o369.agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o371.run_id)))))))))
        ORDER BY f.disposition LIMIT 1)
      WHEN 'historical' THEN RAISE(ABORT,'source_continued')
      WHEN 'reserved' THEN RAISE(ABORT,'continuation_in_progress') END;
END;

CREATE TRIGGER p039_guard_side_effect_attempts_update
BEFORE UPDATE ON side_effect_attempts
WHEN EXISTS (SELECT 1 FROM run_execution_fences) AND (NOT (OLD.attempt_kind='readback' AND NEW.attempt_kind='readback' AND NEW.id IS OLD.id AND NEW.side_effect_id IS OLD.side_effect_id AND NEW.attempt_number IS OLD.attempt_number AND NEW.owner_instance_id IS OLD.owner_instance_id AND NEW.started_at IS OLD.started_at))
BEGIN
    SELECT CASE (SELECT f.disposition FROM run_execution_fences f
        WHERE (EXISTS (SELECT 1 FROM side_effects o372 WHERE o372.id=OLD.side_effect_id AND ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o372.run_id)) OR (EXISTS (SELECT 1 FROM stage_executions o373 WHERE o373.id=o372.stage_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o373.run_id)))) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o374 WHERE o374.agent_execution_id=o372.agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o374.run_id))))))) OR (EXISTS (SELECT 1 FROM side_effects o375 WHERE o375.id=NEW.side_effect_id AND ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o375.run_id)) OR (EXISTS (SELECT 1 FROM stage_executions o376 WHERE o376.id=o375.stage_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o376.run_id)))) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o377 WHERE o377.agent_execution_id=o375.agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o377.run_id)))))))
        ORDER BY f.disposition LIMIT 1)
      WHEN 'historical' THEN RAISE(ABORT,'source_continued')
      WHEN 'reserved' THEN RAISE(ABORT,'continuation_in_progress') END;
END;

CREATE TRIGGER p039_guard_side_effect_attempts_delete
BEFORE DELETE ON side_effect_attempts
WHEN EXISTS (SELECT 1 FROM run_execution_fences)
BEGIN
    SELECT CASE (SELECT f.disposition FROM run_execution_fences f
        WHERE EXISTS (SELECT 1 FROM side_effects o378 WHERE o378.id=OLD.side_effect_id AND ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o378.run_id)) OR (EXISTS (SELECT 1 FROM stage_executions o379 WHERE o379.id=o378.stage_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o379.run_id)))) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o380 WHERE o380.agent_execution_id=o378.agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o380.run_id))))))
        ORDER BY f.disposition LIMIT 1)
      WHEN 'historical' THEN RAISE(ABORT,'source_continued')
      WHEN 'reserved' THEN RAISE(ABORT,'continuation_in_progress') END;
END;

-- Projection integrity/rebuild metadata and updated_at are observational only.

CREATE TRIGGER p039_guard_output_contract_repair_events_insert
BEFORE INSERT ON output_contract_repair_events
WHEN EXISTS (SELECT 1 FROM run_execution_fences)
BEGIN
    SELECT CASE (SELECT f.disposition FROM run_execution_fences f
        WHERE (((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=NEW.run_id)) OR (EXISTS (SELECT 1 FROM stage_executions o381 WHERE o381.id=NEW.stage_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o381.run_id)))) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o382 WHERE o382.agent_execution_id=NEW.agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o382.run_id))))) OR (EXISTS (SELECT 1 FROM session_generations o383 WHERE o383.id=NEW.session_generation_id AND ((EXISTS (SELECT 1 FROM session_lineages o384 WHERE o384.id=o383.lineage_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o384.run_id)))) OR (f.source_run_id IN (SELECT run_id FROM p039_run_resources WHERE resource_key=o383.working_directory AND resource_key<>'')))))) OR (EXISTS (SELECT 1 FROM output_contract_repair_events stored WHERE ((stored.repair_attempt_id=NEW.repair_attempt_id) OR (stored.agent_execution_id=NEW.agent_execution_id)) AND (((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=stored.run_id)) OR (EXISTS (SELECT 1 FROM stage_executions o385 WHERE o385.id=stored.stage_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o385.run_id)))) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o386 WHERE o386.agent_execution_id=stored.agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o386.run_id))))) OR (EXISTS (SELECT 1 FROM session_generations o387 WHERE o387.id=stored.session_generation_id AND ((EXISTS (SELECT 1 FROM session_lineages o388 WHERE o388.id=o387.lineage_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o388.run_id)))) OR (f.source_run_id IN (SELECT run_id FROM p039_run_resources WHERE resource_key=o387.working_directory AND resource_key<>''))))))))
        ORDER BY f.disposition LIMIT 1)
      WHEN 'historical' THEN RAISE(ABORT,'source_continued')
      WHEN 'reserved' THEN RAISE(ABORT,'continuation_in_progress') END;
END;

CREATE TRIGGER p039_guard_output_contract_repair_events_update
BEFORE UPDATE OF repair_attempt_id,schema_version,run_id,stage_execution_id,agent_execution_id,session_generation_id,role,provider_family,adapter_family,required_output_mode,initial_failure_class,initial_failure_subtype,status,presentation_category,recommended_next_action,final_output_settlement,same_session_repair_json,transcript_recovery_json,provider_fallback_json,provider_plan_evidence_json,required_outputs_json,permission_decisions_json,repair_budget_consumed,fallback_budget_consumed,repair_prompt_template_version,recovery_parser_version,policy_feature_flags_json,evidence_artifact_path,lease_id,evidence_version,recorded_at,created_at ON output_contract_repair_events
WHEN EXISTS (SELECT 1 FROM run_execution_fences)
BEGIN
    SELECT CASE (SELECT f.disposition FROM run_execution_fences f
        WHERE (((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=OLD.run_id)) OR (EXISTS (SELECT 1 FROM stage_executions o389 WHERE o389.id=OLD.stage_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o389.run_id)))) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o390 WHERE o390.agent_execution_id=OLD.agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o390.run_id))))) OR (EXISTS (SELECT 1 FROM session_generations o391 WHERE o391.id=OLD.session_generation_id AND ((EXISTS (SELECT 1 FROM session_lineages o392 WHERE o392.id=o391.lineage_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o392.run_id)))) OR (f.source_run_id IN (SELECT run_id FROM p039_run_resources WHERE resource_key=o391.working_directory AND resource_key<>'')))))) OR (((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=NEW.run_id)) OR (EXISTS (SELECT 1 FROM stage_executions o393 WHERE o393.id=NEW.stage_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o393.run_id)))) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o394 WHERE o394.agent_execution_id=NEW.agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o394.run_id))))) OR (EXISTS (SELECT 1 FROM session_generations o395 WHERE o395.id=NEW.session_generation_id AND ((EXISTS (SELECT 1 FROM session_lineages o396 WHERE o396.id=o395.lineage_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o396.run_id)))) OR (f.source_run_id IN (SELECT run_id FROM p039_run_resources WHERE resource_key=o395.working_directory AND resource_key<>''))))))
        ORDER BY f.disposition LIMIT 1)
      WHEN 'historical' THEN RAISE(ABORT,'source_continued')
      WHEN 'reserved' THEN RAISE(ABORT,'continuation_in_progress') END;
END;

CREATE TRIGGER p039_guard_output_contract_repair_events_delete
BEFORE DELETE ON output_contract_repair_events
WHEN EXISTS (SELECT 1 FROM run_execution_fences)
BEGIN
    SELECT CASE (SELECT f.disposition FROM run_execution_fences f
        WHERE ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=OLD.run_id)) OR (EXISTS (SELECT 1 FROM stage_executions o397 WHERE o397.id=OLD.stage_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o397.run_id)))) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o398 WHERE o398.agent_execution_id=OLD.agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o398.run_id))))) OR (EXISTS (SELECT 1 FROM session_generations o399 WHERE o399.id=OLD.session_generation_id AND ((EXISTS (SELECT 1 FROM session_lineages o400 WHERE o400.id=o399.lineage_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o400.run_id)))) OR (f.source_run_id IN (SELECT run_id FROM p039_run_resources WHERE resource_key=o399.working_directory AND resource_key<>'')))))
        ORDER BY f.disposition LIMIT 1)
      WHEN 'historical' THEN RAISE(ABORT,'source_continued')
      WHEN 'reserved' THEN RAISE(ABORT,'continuation_in_progress') END;
END;

CREATE TRIGGER p039_guard_output_contract_repair_leases_insert
BEFORE INSERT ON output_contract_repair_leases
WHEN EXISTS (SELECT 1 FROM run_execution_fences)
BEGIN
    SELECT CASE (SELECT f.disposition FROM run_execution_fences f
        WHERE ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=NEW.run_id)) OR (EXISTS (SELECT 1 FROM stage_executions o401 WHERE o401.id=NEW.stage_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o401.run_id)))) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o402 WHERE o402.agent_execution_id=NEW.parent_agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o402.run_id)))) OR (EXISTS (SELECT 1 FROM output_contract_repair_events o403 WHERE o403.repair_attempt_id=NEW.repair_event_id AND (((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o403.run_id)) OR (EXISTS (SELECT 1 FROM stage_executions o404 WHERE o404.id=o403.stage_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o404.run_id)))) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o405 WHERE o405.agent_execution_id=o403.agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o405.run_id))))) OR (EXISTS (SELECT 1 FROM session_generations o406 WHERE o406.id=o403.session_generation_id AND ((EXISTS (SELECT 1 FROM session_lineages o407 WHERE o407.id=o406.lineage_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o407.run_id)))) OR (f.source_run_id IN (SELECT run_id FROM p039_run_resources WHERE resource_key=o406.working_directory AND resource_key<>''))))))))) OR (EXISTS (SELECT 1 FROM output_contract_repair_leases stored WHERE ((stored.lease_key=NEW.lease_key)) AND ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=stored.run_id)) OR (EXISTS (SELECT 1 FROM stage_executions o408 WHERE o408.id=stored.stage_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o408.run_id)))) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o409 WHERE o409.agent_execution_id=stored.parent_agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o409.run_id)))) OR (EXISTS (SELECT 1 FROM output_contract_repair_events o410 WHERE o410.repair_attempt_id=stored.repair_event_id AND (((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o410.run_id)) OR (EXISTS (SELECT 1 FROM stage_executions o411 WHERE o411.id=o410.stage_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o411.run_id)))) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o412 WHERE o412.agent_execution_id=o410.agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o412.run_id))))) OR (EXISTS (SELECT 1 FROM session_generations o413 WHERE o413.id=o410.session_generation_id AND ((EXISTS (SELECT 1 FROM session_lineages o414 WHERE o414.id=o413.lineage_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o414.run_id)))) OR (f.source_run_id IN (SELECT run_id FROM p039_run_resources WHERE resource_key=o413.working_directory AND resource_key<>'')))))))))))
        ORDER BY f.disposition LIMIT 1)
      WHEN 'historical' THEN RAISE(ABORT,'source_continued')
      WHEN 'reserved' THEN RAISE(ABORT,'continuation_in_progress') END;
END;

CREATE TRIGGER p039_guard_output_contract_repair_leases_update
BEFORE UPDATE ON output_contract_repair_leases
WHEN EXISTS (SELECT 1 FROM run_execution_fences)
BEGIN
    SELECT CASE (SELECT f.disposition FROM run_execution_fences f
        WHERE ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=OLD.run_id)) OR (EXISTS (SELECT 1 FROM stage_executions o415 WHERE o415.id=OLD.stage_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o415.run_id)))) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o416 WHERE o416.agent_execution_id=OLD.parent_agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o416.run_id)))) OR (EXISTS (SELECT 1 FROM output_contract_repair_events o417 WHERE o417.repair_attempt_id=OLD.repair_event_id AND (((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o417.run_id)) OR (EXISTS (SELECT 1 FROM stage_executions o418 WHERE o418.id=o417.stage_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o418.run_id)))) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o419 WHERE o419.agent_execution_id=o417.agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o419.run_id))))) OR (EXISTS (SELECT 1 FROM session_generations o420 WHERE o420.id=o417.session_generation_id AND ((EXISTS (SELECT 1 FROM session_lineages o421 WHERE o421.id=o420.lineage_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o421.run_id)))) OR (f.source_run_id IN (SELECT run_id FROM p039_run_resources WHERE resource_key=o420.working_directory AND resource_key<>''))))))))) OR ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=NEW.run_id)) OR (EXISTS (SELECT 1 FROM stage_executions o422 WHERE o422.id=NEW.stage_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o422.run_id)))) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o423 WHERE o423.agent_execution_id=NEW.parent_agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o423.run_id)))) OR (EXISTS (SELECT 1 FROM output_contract_repair_events o424 WHERE o424.repair_attempt_id=NEW.repair_event_id AND (((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o424.run_id)) OR (EXISTS (SELECT 1 FROM stage_executions o425 WHERE o425.id=o424.stage_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o425.run_id)))) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o426 WHERE o426.agent_execution_id=o424.agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o426.run_id))))) OR (EXISTS (SELECT 1 FROM session_generations o427 WHERE o427.id=o424.session_generation_id AND ((EXISTS (SELECT 1 FROM session_lineages o428 WHERE o428.id=o427.lineage_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o428.run_id)))) OR (f.source_run_id IN (SELECT run_id FROM p039_run_resources WHERE resource_key=o427.working_directory AND resource_key<>'')))))))))
        ORDER BY f.disposition LIMIT 1)
      WHEN 'historical' THEN RAISE(ABORT,'source_continued')
      WHEN 'reserved' THEN RAISE(ABORT,'continuation_in_progress') END;
END;

CREATE TRIGGER p039_guard_output_contract_repair_leases_delete
BEFORE DELETE ON output_contract_repair_leases
WHEN EXISTS (SELECT 1 FROM run_execution_fences)
BEGIN
    SELECT CASE (SELECT f.disposition FROM run_execution_fences f
        WHERE (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=OLD.run_id)) OR (EXISTS (SELECT 1 FROM stage_executions o429 WHERE o429.id=OLD.stage_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o429.run_id)))) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o430 WHERE o430.agent_execution_id=OLD.parent_agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o430.run_id)))) OR (EXISTS (SELECT 1 FROM output_contract_repair_events o431 WHERE o431.repair_attempt_id=OLD.repair_event_id AND (((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o431.run_id)) OR (EXISTS (SELECT 1 FROM stage_executions o432 WHERE o432.id=o431.stage_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o432.run_id)))) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o433 WHERE o433.agent_execution_id=o431.agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o433.run_id))))) OR (EXISTS (SELECT 1 FROM session_generations o434 WHERE o434.id=o431.session_generation_id AND ((EXISTS (SELECT 1 FROM session_lineages o435 WHERE o435.id=o434.lineage_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o435.run_id)))) OR (f.source_run_id IN (SELECT run_id FROM p039_run_resources WHERE resource_key=o434.working_directory AND resource_key<>''))))))))
        ORDER BY f.disposition LIMIT 1)
      WHEN 'historical' THEN RAISE(ABORT,'source_continued')
      WHEN 'reserved' THEN RAISE(ABORT,'continuation_in_progress') END;
END;

CREATE TRIGGER p039_guard_output_contract_repair_fallback_parent_links_insert
BEFORE INSERT ON output_contract_repair_fallback_parent_links
WHEN EXISTS (SELECT 1 FROM run_execution_fences)
BEGIN
    SELECT CASE (SELECT f.disposition FROM run_execution_fences f
        WHERE ((EXISTS (SELECT 1 FROM p039_agent_run_owners o436 WHERE o436.agent_execution_id=NEW.fallback_agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o436.run_id)))) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o437 WHERE o437.agent_execution_id=NEW.parent_failed_agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o437.run_id)))) OR (EXISTS (SELECT 1 FROM output_contract_repair_events o438 WHERE o438.repair_attempt_id=NEW.repair_event_id AND (((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o438.run_id)) OR (EXISTS (SELECT 1 FROM stage_executions o439 WHERE o439.id=o438.stage_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o439.run_id)))) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o440 WHERE o440.agent_execution_id=o438.agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o440.run_id))))) OR (EXISTS (SELECT 1 FROM session_generations o441 WHERE o441.id=o438.session_generation_id AND ((EXISTS (SELECT 1 FROM session_lineages o442 WHERE o442.id=o441.lineage_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o442.run_id)))) OR (f.source_run_id IN (SELECT run_id FROM p039_run_resources WHERE resource_key=o441.working_directory AND resource_key<>'')))))))) OR (EXISTS (SELECT 1 FROM output_contract_repair_leases o443 WHERE o443.lease_key=NEW.lease_key AND ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o443.run_id)) OR (EXISTS (SELECT 1 FROM stage_executions o444 WHERE o444.id=o443.stage_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o444.run_id)))) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o445 WHERE o445.agent_execution_id=o443.parent_agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o445.run_id)))) OR (EXISTS (SELECT 1 FROM output_contract_repair_events o446 WHERE o446.repair_attempt_id=o443.repair_event_id AND (((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o446.run_id)) OR (EXISTS (SELECT 1 FROM stage_executions o447 WHERE o447.id=o446.stage_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o447.run_id)))) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o448 WHERE o448.agent_execution_id=o446.agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o448.run_id))))) OR (EXISTS (SELECT 1 FROM session_generations o449 WHERE o449.id=o446.session_generation_id AND ((EXISTS (SELECT 1 FROM session_lineages o450 WHERE o450.id=o449.lineage_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o450.run_id)))) OR (f.source_run_id IN (SELECT run_id FROM p039_run_resources WHERE resource_key=o449.working_directory AND resource_key<>'')))))))))))) OR (EXISTS (SELECT 1 FROM output_contract_repair_fallback_parent_links stored WHERE ((stored.fallback_agent_execution_id=NEW.fallback_agent_execution_id) OR (stored.parent_failed_agent_execution_id=NEW.parent_failed_agent_execution_id) OR (stored.repair_event_id=NEW.repair_event_id)) AND ((EXISTS (SELECT 1 FROM p039_agent_run_owners o451 WHERE o451.agent_execution_id=stored.fallback_agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o451.run_id)))) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o452 WHERE o452.agent_execution_id=stored.parent_failed_agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o452.run_id)))) OR (EXISTS (SELECT 1 FROM output_contract_repair_events o453 WHERE o453.repair_attempt_id=stored.repair_event_id AND (((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o453.run_id)) OR (EXISTS (SELECT 1 FROM stage_executions o454 WHERE o454.id=o453.stage_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o454.run_id)))) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o455 WHERE o455.agent_execution_id=o453.agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o455.run_id))))) OR (EXISTS (SELECT 1 FROM session_generations o456 WHERE o456.id=o453.session_generation_id AND ((EXISTS (SELECT 1 FROM session_lineages o457 WHERE o457.id=o456.lineage_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o457.run_id)))) OR (f.source_run_id IN (SELECT run_id FROM p039_run_resources WHERE resource_key=o456.working_directory AND resource_key<>'')))))))) OR (EXISTS (SELECT 1 FROM output_contract_repair_leases o458 WHERE o458.lease_key=stored.lease_key AND ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o458.run_id)) OR (EXISTS (SELECT 1 FROM stage_executions o459 WHERE o459.id=o458.stage_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o459.run_id)))) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o460 WHERE o460.agent_execution_id=o458.parent_agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o460.run_id)))) OR (EXISTS (SELECT 1 FROM output_contract_repair_events o461 WHERE o461.repair_attempt_id=o458.repair_event_id AND (((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o461.run_id)) OR (EXISTS (SELECT 1 FROM stage_executions o462 WHERE o462.id=o461.stage_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o462.run_id)))) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o463 WHERE o463.agent_execution_id=o461.agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o463.run_id))))) OR (EXISTS (SELECT 1 FROM session_generations o464 WHERE o464.id=o461.session_generation_id AND ((EXISTS (SELECT 1 FROM session_lineages o465 WHERE o465.id=o464.lineage_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o465.run_id)))) OR (f.source_run_id IN (SELECT run_id FROM p039_run_resources WHERE resource_key=o464.working_directory AND resource_key<>''))))))))))))))
        ORDER BY f.disposition LIMIT 1)
      WHEN 'historical' THEN RAISE(ABORT,'source_continued')
      WHEN 'reserved' THEN RAISE(ABORT,'continuation_in_progress') END;
END;

CREATE TRIGGER p039_guard_output_contract_repair_fallback_parent_links_update
BEFORE UPDATE ON output_contract_repair_fallback_parent_links
WHEN EXISTS (SELECT 1 FROM run_execution_fences)
BEGIN
    SELECT CASE (SELECT f.disposition FROM run_execution_fences f
        WHERE ((EXISTS (SELECT 1 FROM p039_agent_run_owners o466 WHERE o466.agent_execution_id=OLD.fallback_agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o466.run_id)))) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o467 WHERE o467.agent_execution_id=OLD.parent_failed_agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o467.run_id)))) OR (EXISTS (SELECT 1 FROM output_contract_repair_events o468 WHERE o468.repair_attempt_id=OLD.repair_event_id AND (((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o468.run_id)) OR (EXISTS (SELECT 1 FROM stage_executions o469 WHERE o469.id=o468.stage_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o469.run_id)))) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o470 WHERE o470.agent_execution_id=o468.agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o470.run_id))))) OR (EXISTS (SELECT 1 FROM session_generations o471 WHERE o471.id=o468.session_generation_id AND ((EXISTS (SELECT 1 FROM session_lineages o472 WHERE o472.id=o471.lineage_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o472.run_id)))) OR (f.source_run_id IN (SELECT run_id FROM p039_run_resources WHERE resource_key=o471.working_directory AND resource_key<>'')))))))) OR (EXISTS (SELECT 1 FROM output_contract_repair_leases o473 WHERE o473.lease_key=OLD.lease_key AND ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o473.run_id)) OR (EXISTS (SELECT 1 FROM stage_executions o474 WHERE o474.id=o473.stage_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o474.run_id)))) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o475 WHERE o475.agent_execution_id=o473.parent_agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o475.run_id)))) OR (EXISTS (SELECT 1 FROM output_contract_repair_events o476 WHERE o476.repair_attempt_id=o473.repair_event_id AND (((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o476.run_id)) OR (EXISTS (SELECT 1 FROM stage_executions o477 WHERE o477.id=o476.stage_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o477.run_id)))) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o478 WHERE o478.agent_execution_id=o476.agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o478.run_id))))) OR (EXISTS (SELECT 1 FROM session_generations o479 WHERE o479.id=o476.session_generation_id AND ((EXISTS (SELECT 1 FROM session_lineages o480 WHERE o480.id=o479.lineage_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o480.run_id)))) OR (f.source_run_id IN (SELECT run_id FROM p039_run_resources WHERE resource_key=o479.working_directory AND resource_key<>'')))))))))))) OR ((EXISTS (SELECT 1 FROM p039_agent_run_owners o481 WHERE o481.agent_execution_id=NEW.fallback_agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o481.run_id)))) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o482 WHERE o482.agent_execution_id=NEW.parent_failed_agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o482.run_id)))) OR (EXISTS (SELECT 1 FROM output_contract_repair_events o483 WHERE o483.repair_attempt_id=NEW.repair_event_id AND (((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o483.run_id)) OR (EXISTS (SELECT 1 FROM stage_executions o484 WHERE o484.id=o483.stage_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o484.run_id)))) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o485 WHERE o485.agent_execution_id=o483.agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o485.run_id))))) OR (EXISTS (SELECT 1 FROM session_generations o486 WHERE o486.id=o483.session_generation_id AND ((EXISTS (SELECT 1 FROM session_lineages o487 WHERE o487.id=o486.lineage_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o487.run_id)))) OR (f.source_run_id IN (SELECT run_id FROM p039_run_resources WHERE resource_key=o486.working_directory AND resource_key<>'')))))))) OR (EXISTS (SELECT 1 FROM output_contract_repair_leases o488 WHERE o488.lease_key=NEW.lease_key AND ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o488.run_id)) OR (EXISTS (SELECT 1 FROM stage_executions o489 WHERE o489.id=o488.stage_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o489.run_id)))) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o490 WHERE o490.agent_execution_id=o488.parent_agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o490.run_id)))) OR (EXISTS (SELECT 1 FROM output_contract_repair_events o491 WHERE o491.repair_attempt_id=o488.repair_event_id AND (((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o491.run_id)) OR (EXISTS (SELECT 1 FROM stage_executions o492 WHERE o492.id=o491.stage_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o492.run_id)))) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o493 WHERE o493.agent_execution_id=o491.agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o493.run_id))))) OR (EXISTS (SELECT 1 FROM session_generations o494 WHERE o494.id=o491.session_generation_id AND ((EXISTS (SELECT 1 FROM session_lineages o495 WHERE o495.id=o494.lineage_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o495.run_id)))) OR (f.source_run_id IN (SELECT run_id FROM p039_run_resources WHERE resource_key=o494.working_directory AND resource_key<>''))))))))))))
        ORDER BY f.disposition LIMIT 1)
      WHEN 'historical' THEN RAISE(ABORT,'source_continued')
      WHEN 'reserved' THEN RAISE(ABORT,'continuation_in_progress') END;
END;

CREATE TRIGGER p039_guard_output_contract_repair_fallback_parent_links_delete
BEFORE DELETE ON output_contract_repair_fallback_parent_links
WHEN EXISTS (SELECT 1 FROM run_execution_fences)
BEGIN
    SELECT CASE (SELECT f.disposition FROM run_execution_fences f
        WHERE (EXISTS (SELECT 1 FROM p039_agent_run_owners o496 WHERE o496.agent_execution_id=OLD.fallback_agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o496.run_id)))) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o497 WHERE o497.agent_execution_id=OLD.parent_failed_agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o497.run_id)))) OR (EXISTS (SELECT 1 FROM output_contract_repair_events o498 WHERE o498.repair_attempt_id=OLD.repair_event_id AND (((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o498.run_id)) OR (EXISTS (SELECT 1 FROM stage_executions o499 WHERE o499.id=o498.stage_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o499.run_id)))) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o500 WHERE o500.agent_execution_id=o498.agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o500.run_id))))) OR (EXISTS (SELECT 1 FROM session_generations o501 WHERE o501.id=o498.session_generation_id AND ((EXISTS (SELECT 1 FROM session_lineages o502 WHERE o502.id=o501.lineage_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o502.run_id)))) OR (f.source_run_id IN (SELECT run_id FROM p039_run_resources WHERE resource_key=o501.working_directory AND resource_key<>'')))))))) OR (EXISTS (SELECT 1 FROM output_contract_repair_leases o503 WHERE o503.lease_key=OLD.lease_key AND ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o503.run_id)) OR (EXISTS (SELECT 1 FROM stage_executions o504 WHERE o504.id=o503.stage_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o504.run_id)))) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o505 WHERE o505.agent_execution_id=o503.parent_agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o505.run_id)))) OR (EXISTS (SELECT 1 FROM output_contract_repair_events o506 WHERE o506.repair_attempt_id=o503.repair_event_id AND (((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o506.run_id)) OR (EXISTS (SELECT 1 FROM stage_executions o507 WHERE o507.id=o506.stage_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o507.run_id)))) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o508 WHERE o508.agent_execution_id=o506.agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o508.run_id))))) OR (EXISTS (SELECT 1 FROM session_generations o509 WHERE o509.id=o506.session_generation_id AND ((EXISTS (SELECT 1 FROM session_lineages o510 WHERE o510.id=o509.lineage_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o510.run_id)))) OR (f.source_run_id IN (SELECT run_id FROM p039_run_resources WHERE resource_key=o509.working_directory AND resource_key<>'')))))))))))
        ORDER BY f.disposition LIMIT 1)
      WHEN 'historical' THEN RAISE(ABORT,'source_continued')
      WHEN 'reserved' THEN RAISE(ABORT,'continuation_in_progress') END;
END;

CREATE TRIGGER p039_guard_escalation_execution_metadata_insert
BEFORE INSERT ON escalation_execution_metadata
WHEN EXISTS (SELECT 1 FROM run_execution_fences)
BEGIN
    SELECT CASE (SELECT f.disposition FROM run_execution_fences f
        WHERE ((EXISTS (SELECT 1 FROM p039_agent_run_owners o511 WHERE o511.agent_execution_id=NEW.agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o511.run_id)))) OR (EXISTS (SELECT 1 FROM escalation_ledger o512 WHERE o512.id=NEW.escalation_ledger_id AND ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o512.run_id)) OR (EXISTS (SELECT 1 FROM stage_executions o513 WHERE o513.id=o512.stage_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o513.run_id)))))))) OR (EXISTS (SELECT 1 FROM escalation_execution_metadata stored WHERE ((stored.agent_execution_id=NEW.agent_execution_id)) AND ((EXISTS (SELECT 1 FROM p039_agent_run_owners o514 WHERE o514.agent_execution_id=stored.agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o514.run_id)))) OR (EXISTS (SELECT 1 FROM escalation_ledger o515 WHERE o515.id=stored.escalation_ledger_id AND ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o515.run_id)) OR (EXISTS (SELECT 1 FROM stage_executions o516 WHERE o516.id=o515.stage_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o516.run_id))))))))))
        ORDER BY f.disposition LIMIT 1)
      WHEN 'historical' THEN RAISE(ABORT,'source_continued')
      WHEN 'reserved' THEN RAISE(ABORT,'continuation_in_progress') END;
END;

CREATE TRIGGER p039_guard_escalation_execution_metadata_update
BEFORE UPDATE ON escalation_execution_metadata
WHEN EXISTS (SELECT 1 FROM run_execution_fences)
BEGIN
    SELECT CASE (SELECT f.disposition FROM run_execution_fences f
        WHERE ((EXISTS (SELECT 1 FROM p039_agent_run_owners o517 WHERE o517.agent_execution_id=OLD.agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o517.run_id)))) OR (EXISTS (SELECT 1 FROM escalation_ledger o518 WHERE o518.id=OLD.escalation_ledger_id AND ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o518.run_id)) OR (EXISTS (SELECT 1 FROM stage_executions o519 WHERE o519.id=o518.stage_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o519.run_id)))))))) OR ((EXISTS (SELECT 1 FROM p039_agent_run_owners o520 WHERE o520.agent_execution_id=NEW.agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o520.run_id)))) OR (EXISTS (SELECT 1 FROM escalation_ledger o521 WHERE o521.id=NEW.escalation_ledger_id AND ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o521.run_id)) OR (EXISTS (SELECT 1 FROM stage_executions o522 WHERE o522.id=o521.stage_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o522.run_id))))))))
        ORDER BY f.disposition LIMIT 1)
      WHEN 'historical' THEN RAISE(ABORT,'source_continued')
      WHEN 'reserved' THEN RAISE(ABORT,'continuation_in_progress') END;
END;

CREATE TRIGGER p039_guard_escalation_execution_metadata_delete
BEFORE DELETE ON escalation_execution_metadata
WHEN EXISTS (SELECT 1 FROM run_execution_fences)
BEGIN
    SELECT CASE (SELECT f.disposition FROM run_execution_fences f
        WHERE (EXISTS (SELECT 1 FROM p039_agent_run_owners o523 WHERE o523.agent_execution_id=OLD.agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o523.run_id)))) OR (EXISTS (SELECT 1 FROM escalation_ledger o524 WHERE o524.id=OLD.escalation_ledger_id AND ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o524.run_id)) OR (EXISTS (SELECT 1 FROM stage_executions o525 WHERE o525.id=o524.stage_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o525.run_id)))))))
        ORDER BY f.disposition LIMIT 1)
      WHEN 'historical' THEN RAISE(ABORT,'source_continued')
      WHEN 'reserved' THEN RAISE(ABORT,'continuation_in_progress') END;
END;

CREATE TRIGGER p039_guard_escalation_deadline_windows_insert
BEFORE INSERT ON escalation_deadline_windows
WHEN EXISTS (SELECT 1 FROM run_execution_fences)
BEGIN
    SELECT CASE (SELECT f.disposition FROM run_execution_fences f
        WHERE ((EXISTS (SELECT 1 FROM escalation_ledger o526 WHERE o526.id=NEW.escalation_ledger_id AND ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o526.run_id)) OR (EXISTS (SELECT 1 FROM stage_executions o527 WHERE o527.id=o526.stage_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o527.run_id))))))) OR (EXISTS (SELECT 1 FROM stage_executions o528 WHERE o528.id=NEW.source_stage_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o528.run_id)))) OR (EXISTS (SELECT 1 FROM stage_executions o529 WHERE o529.id=NEW.retry_stage_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o529.run_id)))) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o530 WHERE o530.agent_execution_id=NEW.source_agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o530.run_id))))) OR (EXISTS (SELECT 1 FROM escalation_deadline_windows stored WHERE ((stored.id=NEW.id) OR (stored.command_journal_id=NEW.command_journal_id) OR (stored.resume_idempotency_key=NEW.resume_idempotency_key) OR (stored.retry_stage_execution_id=NEW.retry_stage_execution_id) OR (stored.work_item_id=NEW.work_item_id)) AND ((EXISTS (SELECT 1 FROM escalation_ledger o531 WHERE o531.id=stored.escalation_ledger_id AND ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o531.run_id)) OR (EXISTS (SELECT 1 FROM stage_executions o532 WHERE o532.id=o531.stage_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o532.run_id))))))) OR (EXISTS (SELECT 1 FROM stage_executions o533 WHERE o533.id=stored.source_stage_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o533.run_id)))) OR (EXISTS (SELECT 1 FROM stage_executions o534 WHERE o534.id=stored.retry_stage_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o534.run_id)))) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o535 WHERE o535.agent_execution_id=stored.source_agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o535.run_id)))))))
        ORDER BY f.disposition LIMIT 1)
      WHEN 'historical' THEN RAISE(ABORT,'source_continued')
      WHEN 'reserved' THEN RAISE(ABORT,'continuation_in_progress') END;
END;

CREATE TRIGGER p039_guard_escalation_deadline_windows_update
BEFORE UPDATE ON escalation_deadline_windows
WHEN EXISTS (SELECT 1 FROM run_execution_fences)
BEGIN
    SELECT CASE (SELECT f.disposition FROM run_execution_fences f
        WHERE ((EXISTS (SELECT 1 FROM escalation_ledger o536 WHERE o536.id=OLD.escalation_ledger_id AND ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o536.run_id)) OR (EXISTS (SELECT 1 FROM stage_executions o537 WHERE o537.id=o536.stage_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o537.run_id))))))) OR (EXISTS (SELECT 1 FROM stage_executions o538 WHERE o538.id=OLD.source_stage_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o538.run_id)))) OR (EXISTS (SELECT 1 FROM stage_executions o539 WHERE o539.id=OLD.retry_stage_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o539.run_id)))) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o540 WHERE o540.agent_execution_id=OLD.source_agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o540.run_id))))) OR ((EXISTS (SELECT 1 FROM escalation_ledger o541 WHERE o541.id=NEW.escalation_ledger_id AND ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o541.run_id)) OR (EXISTS (SELECT 1 FROM stage_executions o542 WHERE o542.id=o541.stage_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o542.run_id))))))) OR (EXISTS (SELECT 1 FROM stage_executions o543 WHERE o543.id=NEW.source_stage_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o543.run_id)))) OR (EXISTS (SELECT 1 FROM stage_executions o544 WHERE o544.id=NEW.retry_stage_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o544.run_id)))) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o545 WHERE o545.agent_execution_id=NEW.source_agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o545.run_id)))))
        ORDER BY f.disposition LIMIT 1)
      WHEN 'historical' THEN RAISE(ABORT,'source_continued')
      WHEN 'reserved' THEN RAISE(ABORT,'continuation_in_progress') END;
END;

CREATE TRIGGER p039_guard_escalation_deadline_windows_delete
BEFORE DELETE ON escalation_deadline_windows
WHEN EXISTS (SELECT 1 FROM run_execution_fences)
BEGIN
    SELECT CASE (SELECT f.disposition FROM run_execution_fences f
        WHERE (EXISTS (SELECT 1 FROM escalation_ledger o546 WHERE o546.id=OLD.escalation_ledger_id AND ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o546.run_id)) OR (EXISTS (SELECT 1 FROM stage_executions o547 WHERE o547.id=o546.stage_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o547.run_id))))))) OR (EXISTS (SELECT 1 FROM stage_executions o548 WHERE o548.id=OLD.source_stage_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o548.run_id)))) OR (EXISTS (SELECT 1 FROM stage_executions o549 WHERE o549.id=OLD.retry_stage_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o549.run_id)))) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o550 WHERE o550.agent_execution_id=OLD.source_agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o550.run_id))))
        ORDER BY f.disposition LIMIT 1)
      WHEN 'historical' THEN RAISE(ABORT,'source_continued')
      WHEN 'reserved' THEN RAISE(ABORT,'continuation_in_progress') END;
END;

CREATE TRIGGER p039_guard_p080_helper_leases_v1_insert
BEFORE INSERT ON p080_helper_leases_v1
WHEN EXISTS (SELECT 1 FROM run_execution_fences)
BEGIN
    SELECT CASE (SELECT f.disposition FROM run_execution_fences f
        WHERE ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=NEW.run_id)) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o551 WHERE o551.agent_execution_id=NEW.agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o551.run_id)))) OR (EXISTS (SELECT 1 FROM session_generations o552 WHERE o552.id=NEW.session_generation_id AND ((EXISTS (SELECT 1 FROM session_lineages o553 WHERE o553.id=o552.lineage_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o553.run_id)))) OR (f.source_run_id IN (SELECT run_id FROM p039_run_resources WHERE resource_key=o552.working_directory AND resource_key<>'')))))) OR (EXISTS (SELECT 1 FROM p080_helper_leases_v1 stored WHERE ((stored.id=NEW.id)) AND ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=stored.run_id)) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o554 WHERE o554.agent_execution_id=stored.agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o554.run_id)))) OR (EXISTS (SELECT 1 FROM session_generations o555 WHERE o555.id=stored.session_generation_id AND ((EXISTS (SELECT 1 FROM session_lineages o556 WHERE o556.id=o555.lineage_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o556.run_id)))) OR (f.source_run_id IN (SELECT run_id FROM p039_run_resources WHERE resource_key=o555.working_directory AND resource_key<>''))))))))
        ORDER BY f.disposition LIMIT 1)
      WHEN 'historical' THEN RAISE(ABORT,'source_continued')
      WHEN 'reserved' THEN RAISE(ABORT,'continuation_in_progress') END;
END;

CREATE TRIGGER p039_guard_p080_helper_leases_v1_update
BEFORE UPDATE ON p080_helper_leases_v1
WHEN EXISTS (SELECT 1 FROM run_execution_fences)
BEGIN
    SELECT CASE (SELECT f.disposition FROM run_execution_fences f
        WHERE ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=OLD.run_id)) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o557 WHERE o557.agent_execution_id=OLD.agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o557.run_id)))) OR (EXISTS (SELECT 1 FROM session_generations o558 WHERE o558.id=OLD.session_generation_id AND ((EXISTS (SELECT 1 FROM session_lineages o559 WHERE o559.id=o558.lineage_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o559.run_id)))) OR (f.source_run_id IN (SELECT run_id FROM p039_run_resources WHERE resource_key=o558.working_directory AND resource_key<>'')))))) OR ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=NEW.run_id)) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o560 WHERE o560.agent_execution_id=NEW.agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o560.run_id)))) OR (EXISTS (SELECT 1 FROM session_generations o561 WHERE o561.id=NEW.session_generation_id AND ((EXISTS (SELECT 1 FROM session_lineages o562 WHERE o562.id=o561.lineage_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o562.run_id)))) OR (f.source_run_id IN (SELECT run_id FROM p039_run_resources WHERE resource_key=o561.working_directory AND resource_key<>''))))))
        ORDER BY f.disposition LIMIT 1)
      WHEN 'historical' THEN RAISE(ABORT,'source_continued')
      WHEN 'reserved' THEN RAISE(ABORT,'continuation_in_progress') END;
END;

CREATE TRIGGER p039_guard_p080_helper_leases_v1_delete
BEFORE DELETE ON p080_helper_leases_v1
WHEN EXISTS (SELECT 1 FROM run_execution_fences)
BEGIN
    SELECT CASE (SELECT f.disposition FROM run_execution_fences f
        WHERE (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=OLD.run_id)) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o563 WHERE o563.agent_execution_id=OLD.agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o563.run_id)))) OR (EXISTS (SELECT 1 FROM session_generations o564 WHERE o564.id=OLD.session_generation_id AND ((EXISTS (SELECT 1 FROM session_lineages o565 WHERE o565.id=o564.lineage_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o565.run_id)))) OR (f.source_run_id IN (SELECT run_id FROM p039_run_resources WHERE resource_key=o564.working_directory AND resource_key<>'')))))
        ORDER BY f.disposition LIMIT 1)
      WHEN 'historical' THEN RAISE(ABORT,'source_continued')
      WHEN 'reserved' THEN RAISE(ABORT,'continuation_in_progress') END;
END;

CREATE TRIGGER p039_guard_p080_helper_lease_members_v1_insert
BEFORE INSERT ON p080_helper_lease_members_v1
WHEN EXISTS (SELECT 1 FROM run_execution_fences)
BEGIN
    SELECT CASE (SELECT f.disposition FROM run_execution_fences f
        WHERE (EXISTS (SELECT 1 FROM p080_helper_leases_v1 o566 WHERE o566.id=NEW.lease_id AND ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o566.run_id)) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o567 WHERE o567.agent_execution_id=o566.agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o567.run_id)))) OR (EXISTS (SELECT 1 FROM session_generations o568 WHERE o568.id=o566.session_generation_id AND ((EXISTS (SELECT 1 FROM session_lineages o569 WHERE o569.id=o568.lineage_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o569.run_id)))) OR (f.source_run_id IN (SELECT run_id FROM p039_run_resources WHERE resource_key=o568.working_directory AND resource_key<>'')))))))) OR (EXISTS (SELECT 1 FROM p080_helper_lease_members_v1 stored WHERE ((stored.lease_id=NEW.lease_id AND stored.member_pid=NEW.member_pid AND stored.member_process_start_boottime_nanos=NEW.member_process_start_boottime_nanos)) AND (EXISTS (SELECT 1 FROM p080_helper_leases_v1 o570 WHERE o570.id=stored.lease_id AND ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o570.run_id)) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o571 WHERE o571.agent_execution_id=o570.agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o571.run_id)))) OR (EXISTS (SELECT 1 FROM session_generations o572 WHERE o572.id=o570.session_generation_id AND ((EXISTS (SELECT 1 FROM session_lineages o573 WHERE o573.id=o572.lineage_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o573.run_id)))) OR (f.source_run_id IN (SELECT run_id FROM p039_run_resources WHERE resource_key=o572.working_directory AND resource_key<>''))))))))))
        ORDER BY f.disposition LIMIT 1)
      WHEN 'historical' THEN RAISE(ABORT,'source_continued')
      WHEN 'reserved' THEN RAISE(ABORT,'continuation_in_progress') END;
END;

CREATE TRIGGER p039_guard_p080_helper_lease_members_v1_update
BEFORE UPDATE ON p080_helper_lease_members_v1
WHEN EXISTS (SELECT 1 FROM run_execution_fences)
BEGIN
    SELECT CASE (SELECT f.disposition FROM run_execution_fences f
        WHERE (EXISTS (SELECT 1 FROM p080_helper_leases_v1 o574 WHERE o574.id=OLD.lease_id AND ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o574.run_id)) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o575 WHERE o575.agent_execution_id=o574.agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o575.run_id)))) OR (EXISTS (SELECT 1 FROM session_generations o576 WHERE o576.id=o574.session_generation_id AND ((EXISTS (SELECT 1 FROM session_lineages o577 WHERE o577.id=o576.lineage_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o577.run_id)))) OR (f.source_run_id IN (SELECT run_id FROM p039_run_resources WHERE resource_key=o576.working_directory AND resource_key<>'')))))))) OR (EXISTS (SELECT 1 FROM p080_helper_leases_v1 o578 WHERE o578.id=NEW.lease_id AND ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o578.run_id)) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o579 WHERE o579.agent_execution_id=o578.agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o579.run_id)))) OR (EXISTS (SELECT 1 FROM session_generations o580 WHERE o580.id=o578.session_generation_id AND ((EXISTS (SELECT 1 FROM session_lineages o581 WHERE o581.id=o580.lineage_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o581.run_id)))) OR (f.source_run_id IN (SELECT run_id FROM p039_run_resources WHERE resource_key=o580.working_directory AND resource_key<>''))))))))
        ORDER BY f.disposition LIMIT 1)
      WHEN 'historical' THEN RAISE(ABORT,'source_continued')
      WHEN 'reserved' THEN RAISE(ABORT,'continuation_in_progress') END;
END;

CREATE TRIGGER p039_guard_p080_helper_lease_members_v1_delete
BEFORE DELETE ON p080_helper_lease_members_v1
WHEN EXISTS (SELECT 1 FROM run_execution_fences)
BEGIN
    SELECT CASE (SELECT f.disposition FROM run_execution_fences f
        WHERE EXISTS (SELECT 1 FROM p080_helper_leases_v1 o582 WHERE o582.id=OLD.lease_id AND ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o582.run_id)) OR (EXISTS (SELECT 1 FROM p039_agent_run_owners o583 WHERE o583.agent_execution_id=o582.agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o583.run_id)))) OR (EXISTS (SELECT 1 FROM session_generations o584 WHERE o584.id=o582.session_generation_id AND ((EXISTS (SELECT 1 FROM session_lineages o585 WHERE o585.id=o584.lineage_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o585.run_id)))) OR (f.source_run_id IN (SELECT run_id FROM p039_run_resources WHERE resource_key=o584.working_directory AND resource_key<>'')))))))
        ORDER BY f.disposition LIMIT 1)
      WHEN 'historical' THEN RAISE(ABORT,'source_continued')
      WHEN 'reserved' THEN RAISE(ABORT,'continuation_in_progress') END;
END;

CREATE TRIGGER p039_guard_runtime_invocations_insert
BEFORE INSERT ON runtime_invocations
WHEN EXISTS (SELECT 1 FROM run_execution_fences)
BEGIN
    SELECT CASE (SELECT f.disposition FROM run_execution_fences f
        WHERE ((EXISTS (SELECT 1 FROM p039_agent_run_owners o586 WHERE o586.agent_execution_id=NEW.agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o586.run_id)))) OR (NEW.completed_at IS NULL AND NOT EXISTS (SELECT 1 FROM p039_agent_run_owners WHERE agent_execution_id=NEW.agent_execution_id))) OR (EXISTS (SELECT 1 FROM runtime_invocations stored WHERE ((stored.id=NEW.id)) AND ((EXISTS (SELECT 1 FROM p039_agent_run_owners o587 WHERE o587.agent_execution_id=stored.agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o587.run_id)))) OR (stored.completed_at IS NULL AND NOT EXISTS (SELECT 1 FROM p039_agent_run_owners WHERE agent_execution_id=stored.agent_execution_id)))))
        ORDER BY f.disposition LIMIT 1)
      WHEN 'historical' THEN RAISE(ABORT,'source_continued')
      WHEN 'reserved' THEN RAISE(ABORT,'continuation_in_progress') END;
END;

CREATE TRIGGER p039_guard_runtime_invocations_update
BEFORE UPDATE ON runtime_invocations
WHEN EXISTS (SELECT 1 FROM run_execution_fences)
BEGIN
    SELECT CASE (SELECT f.disposition FROM run_execution_fences f
        WHERE ((EXISTS (SELECT 1 FROM p039_agent_run_owners o588 WHERE o588.agent_execution_id=OLD.agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o588.run_id)))) OR (OLD.completed_at IS NULL AND NOT EXISTS (SELECT 1 FROM p039_agent_run_owners WHERE agent_execution_id=OLD.agent_execution_id))) OR ((EXISTS (SELECT 1 FROM p039_agent_run_owners o589 WHERE o589.agent_execution_id=NEW.agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o589.run_id)))) OR (NEW.completed_at IS NULL AND NOT EXISTS (SELECT 1 FROM p039_agent_run_owners WHERE agent_execution_id=NEW.agent_execution_id)))
        ORDER BY f.disposition LIMIT 1)
      WHEN 'historical' THEN RAISE(ABORT,'source_continued')
      WHEN 'reserved' THEN RAISE(ABORT,'continuation_in_progress') END;
END;

CREATE TRIGGER p039_guard_runtime_invocations_delete
BEFORE DELETE ON runtime_invocations
WHEN EXISTS (SELECT 1 FROM run_execution_fences)
BEGIN
    SELECT CASE (SELECT f.disposition FROM run_execution_fences f
        WHERE (EXISTS (SELECT 1 FROM p039_agent_run_owners o590 WHERE o590.agent_execution_id=OLD.agent_execution_id AND (f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=o590.run_id)))) OR (OLD.completed_at IS NULL AND NOT EXISTS (SELECT 1 FROM p039_agent_run_owners WHERE agent_execution_id=OLD.agent_execution_id))
        ORDER BY f.disposition LIMIT 1)
      WHEN 'historical' THEN RAISE(ABORT,'source_continued')
      WHEN 'reserved' THEN RAISE(ABORT,'continuation_in_progress') END;
END;

-- Existing headless identity/transition/no-delete guards remain authoritative for settlement; never dispatch after a source fence.

CREATE TRIGGER p039_guard_xcode_effect_attempts_insert
BEFORE INSERT ON xcode_effect_attempts
WHEN EXISTS (SELECT 1 FROM run_execution_fences)
BEGIN
    SELECT CASE (SELECT f.disposition FROM run_execution_fences f
        WHERE ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=CASE WHEN json_valid(NEW.intent_json) THEN json_extract(NEW.intent_json,'$.run_id') END)) OR (NOT EXISTS (SELECT 1 FROM runs WHERE id=CASE WHEN json_valid(NEW.intent_json) THEN json_extract(NEW.intent_json,'$.run_id') END)) OR (EXISTS (SELECT 1 FROM p039_run_resources q WHERE q.run_id=f.source_run_id AND q.resource_key<>'' AND (CASE WHEN json_valid(NEW.project_key) THEN json_extract(NEW.project_key,'$.canonical_path') END=rtrim(q.resource_key,'/') OR substr(CASE WHEN json_valid(NEW.project_key) THEN json_extract(NEW.project_key,'$.canonical_path') END,1,length(rtrim(q.resource_key,'/'))+1)=rtrim(q.resource_key,'/')||'/'))) OR (EXISTS (SELECT 1 FROM xcode_effect_attempts prior WHERE f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=CASE WHEN json_valid(prior.intent_json) THEN json_extract(prior.intent_json,'$.run_id') END) AND CASE WHEN json_valid(prior.project_key) THEN json_extract(prior.project_key,'$.uid') END=CASE WHEN json_valid(NEW.project_key) THEN json_extract(NEW.project_key,'$.uid') END AND (CASE WHEN json_valid(prior.project_key) THEN json_extract(prior.project_key,'$.canonical_path') END=CASE WHEN json_valid(NEW.project_key) THEN json_extract(NEW.project_key,'$.canonical_path') END OR (CASE WHEN json_valid(prior.project_key) THEN json_extract(prior.project_key,'$.device') END=CASE WHEN json_valid(NEW.project_key) THEN json_extract(NEW.project_key,'$.device') END AND CASE WHEN json_valid(prior.project_key) THEN json_extract(prior.project_key,'$.inode') END=CASE WHEN json_valid(NEW.project_key) THEN json_extract(NEW.project_key,'$.inode') END))))) OR (EXISTS (SELECT 1 FROM xcode_effect_attempts stored WHERE ((stored.attempt_id=NEW.attempt_id) OR (stored.nonce=NEW.nonce) OR (stored.owner_lineage=NEW.owner_lineage AND stored.project_key=NEW.project_key AND stored.operation_key=NEW.operation_key)) AND ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=CASE WHEN json_valid(stored.intent_json) THEN json_extract(stored.intent_json,'$.run_id') END)) OR (NOT EXISTS (SELECT 1 FROM runs WHERE id=CASE WHEN json_valid(stored.intent_json) THEN json_extract(stored.intent_json,'$.run_id') END)) OR (EXISTS (SELECT 1 FROM p039_run_resources q WHERE q.run_id=f.source_run_id AND q.resource_key<>'' AND (CASE WHEN json_valid(stored.project_key) THEN json_extract(stored.project_key,'$.canonical_path') END=rtrim(q.resource_key,'/') OR substr(CASE WHEN json_valid(stored.project_key) THEN json_extract(stored.project_key,'$.canonical_path') END,1,length(rtrim(q.resource_key,'/'))+1)=rtrim(q.resource_key,'/')||'/'))) OR (EXISTS (SELECT 1 FROM xcode_effect_attempts prior WHERE f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=CASE WHEN json_valid(prior.intent_json) THEN json_extract(prior.intent_json,'$.run_id') END) AND CASE WHEN json_valid(prior.project_key) THEN json_extract(prior.project_key,'$.uid') END=CASE WHEN json_valid(stored.project_key) THEN json_extract(stored.project_key,'$.uid') END AND (CASE WHEN json_valid(prior.project_key) THEN json_extract(prior.project_key,'$.canonical_path') END=CASE WHEN json_valid(stored.project_key) THEN json_extract(stored.project_key,'$.canonical_path') END OR (CASE WHEN json_valid(prior.project_key) THEN json_extract(prior.project_key,'$.device') END=CASE WHEN json_valid(stored.project_key) THEN json_extract(stored.project_key,'$.device') END AND CASE WHEN json_valid(prior.project_key) THEN json_extract(prior.project_key,'$.inode') END=CASE WHEN json_valid(stored.project_key) THEN json_extract(stored.project_key,'$.inode') END)))))))
        ORDER BY f.disposition LIMIT 1)
      WHEN 'historical' THEN RAISE(ABORT,'source_continued')
      WHEN 'reserved' THEN RAISE(ABORT,'continuation_in_progress') END;
END;

CREATE TRIGGER p039_guard_xcode_effect_attempts_update
BEFORE UPDATE ON xcode_effect_attempts
WHEN EXISTS (SELECT 1 FROM run_execution_fences) AND (NEW.state='dispatched')
BEGIN
    SELECT CASE (SELECT f.disposition FROM run_execution_fences f
        WHERE ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=CASE WHEN json_valid(OLD.intent_json) THEN json_extract(OLD.intent_json,'$.run_id') END)) OR (NOT EXISTS (SELECT 1 FROM runs WHERE id=CASE WHEN json_valid(OLD.intent_json) THEN json_extract(OLD.intent_json,'$.run_id') END)) OR (EXISTS (SELECT 1 FROM p039_run_resources q WHERE q.run_id=f.source_run_id AND q.resource_key<>'' AND (CASE WHEN json_valid(OLD.project_key) THEN json_extract(OLD.project_key,'$.canonical_path') END=rtrim(q.resource_key,'/') OR substr(CASE WHEN json_valid(OLD.project_key) THEN json_extract(OLD.project_key,'$.canonical_path') END,1,length(rtrim(q.resource_key,'/'))+1)=rtrim(q.resource_key,'/')||'/'))) OR (EXISTS (SELECT 1 FROM xcode_effect_attempts prior WHERE f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=CASE WHEN json_valid(prior.intent_json) THEN json_extract(prior.intent_json,'$.run_id') END) AND CASE WHEN json_valid(prior.project_key) THEN json_extract(prior.project_key,'$.uid') END=CASE WHEN json_valid(OLD.project_key) THEN json_extract(OLD.project_key,'$.uid') END AND (CASE WHEN json_valid(prior.project_key) THEN json_extract(prior.project_key,'$.canonical_path') END=CASE WHEN json_valid(OLD.project_key) THEN json_extract(OLD.project_key,'$.canonical_path') END OR (CASE WHEN json_valid(prior.project_key) THEN json_extract(prior.project_key,'$.device') END=CASE WHEN json_valid(OLD.project_key) THEN json_extract(OLD.project_key,'$.device') END AND CASE WHEN json_valid(prior.project_key) THEN json_extract(prior.project_key,'$.inode') END=CASE WHEN json_valid(OLD.project_key) THEN json_extract(OLD.project_key,'$.inode') END))))) OR ((f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=CASE WHEN json_valid(NEW.intent_json) THEN json_extract(NEW.intent_json,'$.run_id') END)) OR (NOT EXISTS (SELECT 1 FROM runs WHERE id=CASE WHEN json_valid(NEW.intent_json) THEN json_extract(NEW.intent_json,'$.run_id') END)) OR (EXISTS (SELECT 1 FROM p039_run_resources q WHERE q.run_id=f.source_run_id AND q.resource_key<>'' AND (CASE WHEN json_valid(NEW.project_key) THEN json_extract(NEW.project_key,'$.canonical_path') END=rtrim(q.resource_key,'/') OR substr(CASE WHEN json_valid(NEW.project_key) THEN json_extract(NEW.project_key,'$.canonical_path') END,1,length(rtrim(q.resource_key,'/'))+1)=rtrim(q.resource_key,'/')||'/'))) OR (EXISTS (SELECT 1 FROM xcode_effect_attempts prior WHERE f.source_run_id IN (SELECT source_run_id FROM p039_run_resource_peers WHERE run_id=CASE WHEN json_valid(prior.intent_json) THEN json_extract(prior.intent_json,'$.run_id') END) AND CASE WHEN json_valid(prior.project_key) THEN json_extract(prior.project_key,'$.uid') END=CASE WHEN json_valid(NEW.project_key) THEN json_extract(NEW.project_key,'$.uid') END AND (CASE WHEN json_valid(prior.project_key) THEN json_extract(prior.project_key,'$.canonical_path') END=CASE WHEN json_valid(NEW.project_key) THEN json_extract(NEW.project_key,'$.canonical_path') END OR (CASE WHEN json_valid(prior.project_key) THEN json_extract(prior.project_key,'$.device') END=CASE WHEN json_valid(NEW.project_key) THEN json_extract(NEW.project_key,'$.device') END AND CASE WHEN json_valid(prior.project_key) THEN json_extract(prior.project_key,'$.inode') END=CASE WHEN json_valid(NEW.project_key) THEN json_extract(NEW.project_key,'$.inode') END)))))
        ORDER BY f.disposition LIMIT 1)
      WHEN 'historical' THEN RAISE(ABORT,'source_continued')
      WHEN 'reserved' THEN RAISE(ABORT,'continuation_in_progress') END;
END;
