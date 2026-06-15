import json
import hashlib
import os

run_state_path = "/Users/user/Documents/Chainworks Forge/.chainworks/runs/dc8a088d-3f95-434d-be41-13b55e100a44/state/run-state.json"
plan_path = "/Users/user/Documents/Chainworks Forge/.chainworks/runs/dc8a088d-3f95-434d-be41-13b55e100a44/implementation/plan.md"
backlog_path = "/Users/user/Documents/Chainworks Forge/.chainworks/runs/dc8a088d-3f95-434d-be41-13b55e100a44/implementation/backlog.yaml"

os.makedirs(os.path.dirname(plan_path), exist_ok=True)

# 1. Update run state
run_state = {
  "agent_execution_id": "d06b74a2-b9b3-42fe-9c2a-88332f204645",
  "decision": "approved_for_implementation",
  "gate": None,
  "loop_counters": {
    "proposal_review": 0,
    "implementation_cycles": 0
  },
  "outputs": {
    "approved_proposal": "/Users/user/Documents/Chainworks Forge/.chainworks/runs/dc8a088d-3f95-434d-be41-13b55e100a44/proposals/approved/proposal.md",
    "implementation_plan": "/Users/user/Documents/Chainworks Forge/.chainworks/runs/dc8a088d-3f95-434d-be41-13b55e100a44/implementation/plan.md",
    "implementation_backlog": "/Users/user/Documents/Chainworks Forge/.chainworks/runs/dc8a088d-3f95-434d-be41-13b55e100a44/implementation/backlog.yaml",
    "run_state": "/Users/user/Documents/Chainworks Forge/.chainworks/runs/dc8a088d-3f95-434d-be41-13b55e100a44/state/run-state.json",
    "orchestrator_summary": "/Users/user/Documents/Chainworks Forge/.chainworks/runs/dc8a088d-3f95-434d-be41-13b55e100a44/summaries/orchestrator.md"
  },
  "proposal_revision_id": "P083-r68-refined-r66-score-lift",
  "run_id": "dc8a088d-3f95-434d-be41-13b55e100a44",
  "schema_version": "run_state_v1",
  "session_generation_id": "5e8f3191-2c77-4599-8f8f-a940a485e4f4",
  "stage_execution_id": "fbf35522-39df-4f46-8d8d-d80324a5e3fc",
  "stage_id": "state_7_implementation_started",
  "state": "implementation_started",
  "work_item_id": "p058-invoke:fbf35522-39df-4f46-8d8d-d80324a5e3fc:0"
}

with open(run_state_path, "w") as f:
    json.dump(run_state, f, indent=2)

# 2. Write implementation plan
plan_content = """# Implementation Plan: Execution-Truth Ownership and Invariant Model (P083)

## Objective
Implement Proposal 083 (Execution-Truth Ownership and Invariant Model) to define a single authoritative durable record for execution-truth identifiers across GraphQL, MCP, SQLite, and SwiftUI.

## Strategy
1. **Schema & Migrations**: Implement the 7 additive migrations ensuring `schema_version` constraints, idempotency tables, shutdown receipts, bounded metrics, and cancellation intent fields. Add a durable monotonic clock migration.
2. **Backend Domain & Persistence**: Build rust models for bounded metric labels, idempotency records, and shutdown signals.
3. **GraphQL & MCP API**: Align MCP tool schemas and GraphQL SDL mutations. Integrate a shared typed denial vocabulary. Return byte-equal rollback disposition readbacks.
4. **AppKit & SwiftUI UI**: Implement `ManualProcessIdentityCheckBanner` in SwiftUI. Ensure native command validations defer to the focused lifecycle window, gracefully routing through `focusedSceneValue` or AppKit responders, rendering disabled-but-visible menus appropriately.
5. **Fixtures & Tests**: Create all mandated fixtures covering idempotency TTls, crash recovery rules, SwiftData model leakage guards, and late-output caps.

## Phases
1. **Phase 1: Migrations and SQLite Models**
   - Apply additive DB changes (migrations p083_001 to p083_007).
   - Backfill/verify `artifact_lineage.report_kind`.
2. **Phase 2: GraphQL & MCP Surface**
   - Author exact schemas for the 8 lifecycle commands.
   - Return standard rollback dispositions.
3. **Phase 3: Daemon & Command Idempotency**
   - Introduce lease acquire, replay logic, and late-output bounds.
   - Plumb atomic counter checks and the durable monotonic clock.
4. **Phase 4: macOS UI & SwiftData**
   - Ensure AppKit grace bounds are handled accurately.
   - Isolate SwiftData into projection-only scopes via `@ModelActor` and Sendables.
   - Wire UI banners for manual identity resolution with VoiceOver bindings and NSPasteboard hygiene.
5. **Phase 5: Fixtures & Contract Rollout**
   - Publish all required `.fixture.json` files and test using `scripts/test-gate.sh p083`.
"""
with open(plan_path, "w") as f:
    f.write(plan_content)

# 3. Write implementation backlog
backlog_content = """schema_version: backlog_v1
proposal_id: P083
proposal_revision_id: P083-r68-refined-r66-score-lift
items:
  - id: P083-HARDEN-001
    title: GraphQL SDL inventory for lifecycle mutations
    status: pending
  - id: P083-HARDEN-002
    title: MCP tool inventory and shared denial vocabulary
    status: pending
  - id: P083-HARDEN-003
    title: artifact_lineage.report_kind backfill posture
    status: pending
  - id: P083-HARDEN-004
    title: Schema-version evolution policy
    status: pending
  - id: P083-HARDEN-005
    title: SwiftData isolation boundary
    status: pending
  - id: P083-HARDEN-006
    title: Command idempotency intent hashing
    status: pending
  - id: P083-HARDEN-007
    title: Failed-terminal retry policy per lifecycle command
    status: pending
  - id: P083-HARDEN-008
    title: Atomic late-output counter increments
    status: pending
  - id: P083-HARDEN-009
    title: External side-effect composition for idempotent commands
    status: pending
  - id: P083-HARDEN-010
    title: Durable monotonic clock contract
    status: pending
  - id: P083-HARDEN-011
    title: Minimum command lease TTL policy
    status: pending
  - id: API-P083-R68-NB-001
    title: GraphQL SDL coverage for lifecycle mutations
    status: pending
  - id: API-P083-R68-NB-002
    title: MCP tool inventory and shared denial vocabulary
    status: pending
  - id: API-P083-R68-NB-003
    title: artifact_lineage_report_kind backfill posture
    status: pending
  - id: API-P083-R68-NB-004
    title: Schema-version evolution policy
    status: pending
  - id: APPLE-P083-R68-NB-001
    title: swift_concurrency_isolation
    status: pending
  - id: APPLE-P083-R68-NB-002
    title: windowgroup_container_binding
    status: pending
  - id: APPLE-P083-R68-NB-003
    title: terminate_later_budget_accounting
    status: pending
  - id: MACOS-P083-R68-NB-001
    title: keyboard workflows
    status: pending
  - id: MACOS-P083-R68-NB-002
    title: accessibility copy
    status: pending
  - id: MACOS-P083-R68-NB-003
    title: off-focus surfacing
    status: pending
  - id: MACOS-P083-R68-NB-004
    title: progress feedback
    status: pending
  - id: MACOS-P083-R68-NB-005
    title: pasteboard hygiene
    status: pending
  - id: MACOS-P083-R68-NB-006
    title: command routing API
    status: pending
  - id: REL-P083-R68-NB-001
    title: command idempotency intent hashing
    status: pending
  - id: REL-P083-R68-NB-002
    title: failed_terminal retry policy
    status: pending
  - id: REL-P083-R68-NB-003
    title: stuck-state risk for held identity_ambiguous intents
    status: pending
  - id: REL-P083-R68-NB-004
    title: background re-evidencing of held identity_ambiguous intents
    status: pending
  - id: REL-P083-R68-NB-005
    title: concurrent late-output counter integrity
    status: pending
  - id: REL-P083-R68-NB-006
    title: external side-effect composition across commands
    status: pending
  - id: REL-P083-R68-NB-007
    title: minimum command lease TTL floor
    status: pending
  - id: REL-P083-R68-NB-008
    title: durable deadline clocks
    status: pending
  - id: ui-designer-NB-001
    title: proposal polish (ManualProcessIdentityCheckBanner spacing/SF Symbols)
    status: pending
  - id: ui-designer-NB-002
    title: proposal polish (unknown denial codes visually distinct)
    status: pending
"""
with open(backlog_path, "w") as f:
    f.write(backlog_content)

# 4. Generate manifest
def compute_manifest(path, name):
    with open(path, "rb") as f:
        data = f.read()
    digest = hashlib.sha256(data).hexdigest()
    size = len(data)
    return {
        "mode": "direct_file",
        "output_name": name,
        "path": path,
        "digest": f"sha256:{digest}",
        "size_bytes": size
    }

manifests = {
    plan_path: compute_manifest(plan_path, "implementation_plan"),
    backlog_path: compute_manifest(backlog_path, "implementation_backlog"),
    run_state_path: compute_manifest(run_state_path, "run_state")
}

output = {
    "CHAINWORKS_OUTPUT": manifests
}
with open("output.json", "w") as f:
    json.dump(output, f)

print("SUCCESS")
