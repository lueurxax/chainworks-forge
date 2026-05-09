import json
import os

run_dir = "/Users/user/Documents/Chainworks Forge/.chainworks/runs/4dd7c886-e7b4-4f6d-8afe-f76d63bf268d"

with open(f"{run_dir}/proposals/current/proposal.md", "r") as f:
    proposal_content = f.read()

plan_content = """# Implementation Plan: Boundary-First API and Auth Contract Matrix (P081-v6)

## Overview
This plan covers the implementation of the Boundary-First API and Auth Contract Matrix proposal (P081). It incorporates feedback from the proposal review to ensure audit durability, operator alerting, reliability, security, and Apple platform architecture alignment.

## Core Phases

### Phase 1: Matrix and Audit Foundation
- Define matrix docs and JSON schema.
- Implement fixture validator and embedded last-known-good fixture.
- Add SQL migrations for `audit_log` and `audit_log_checkpoints`.
- Implement `db::repos::audit_log` contract with transactional and bounded standalone append paths.
- Address Review Followups: Define hash canonicalization, retention policies, and unavailable-storage behavior. Make local-only audit-chain threat framing explicit.

### Phase 2: Identity and Caller Classification
- Implement `CallerClass` enum and `CallerContext.caller_class`.
- Update principal-table reader for v1, existing schema_version 2, and new schema_version 3.
- Build schema_version 3 bootstrap writer.
- Add nullable `command_journal.caller_class`.
- Address Review Followups: Define `caller_fingerprint`, evaluate plaintext token risks, and clarify token-derived identifiers.

### Phase 3: Resolution and Ambiguous Mode
- Update `auth::resolve` to derive `caller_class`.
- Implement ambiguous caller warnings while legacy guards remain authoritative.

### Phase 4: Shared Surface Injection and Safe Mode
- Inject single `BoundaryPolicy` service across GraphQL, MCP, observer reconciliation, startup safe-mode, `boundaryRuntime`, `operatorAlerts`, and native macOS alert delivery.
- Implement MCP -32004 compatibility signal and deterministic GraphQL errors.
- Address Review Followups: Pin operator alerting, runbooks, telemetry cardinality, and shadow coverage evidence. Clarify client cancellation, policy reload generation bumps, and safe-mode observability.

### Phase 5: Approval Actionability and Native UX
- Implement `ActionabilityProjection::for_caller` and `approveApproval`/`rejectApproval` `idempotencyKey`.
- Create `approval_mutation_idempotency` table and `ApprovalActionAttemptStore` persistence.
- Add typed redaction envelope and accessibility contract (macOS alerts and UI parity).
- Address Review Followups: Ensure injected `ApprovalActionAttemptStore` ownership and AppKit/native notification surfaces remain typed-state consumers. Define idempotency retention and quota.

### Phase 6: Compatibility Retirement and Coverage
- Retire compatibility fixtures.
- Enable `scripts/check-boundary-coverage.sh` in test-gate guardrails.
- Implement resumable `command_journal` backfill.
- Address Review Followups: Pin cleanup/backfill stalls, poison projection handling, transport confidentiality, wildcard review controls, and readback non-disclosure.
"""

backlog_content = """version: 1
backlog:
  - id: "p081-phase1"
    title: "Phase 1: Matrix and Audit Foundation"
    status: "todo"
    tasks:
      - "Create boundary matrix docs and JSON schema."
      - "Implement fixture validator and embedded last-known-good fixture."
      - "Create next additive numbered SQL migrations for audit_log and audit_log_checkpoints."
      - "Implement db::repos::audit_log contract with transaction append paths."
      - "Address hash canonicalization, retention policies, and unavailable-storage behavior."
  - id: "p081-phase2"
    title: "Phase 2: Identity and Caller Classification"
    status: "todo"
    tasks:
      - "Add CallerClass enum and CallerContext.caller_class."
      - "Implement principal-table reader for schema versions 1, 2, and 3."
      - "Implement schema_version 3 bootstrap writer."
      - "Update command_journal with nullable caller_class."
      - "Address caller_fingerprint definition and plaintext token risk."
  - id: "p081-phase3"
    title: "Phase 3: Resolution and Ambiguous Mode"
    status: "todo"
    tasks:
      - "Update auth::resolve to derive caller_class."
      - "Implement ambiguous caller warnings while keeping legacy guards authoritative."
  - id: "p081-phase4"
    title: "Phase 4: Shared Surface Injection and Safe Mode"
    status: "todo"
    tasks:
      - "Inject shared BoundaryPolicy service across GraphQL, MCP, and operatorAlerts."
      - "Implement MCP -32004 compatibility signal and deterministic GraphQL error contract."
      - "Pin operator alerting, runbooks, and shadow coverage evidence."
      - "Address client cancellation and policy reload generation bumps."
  - id: "p081-phase5"
    title: "Phase 5: Approval Actionability and Native UX"
    status: "todo"
    tasks:
      - "Implement ActionabilityProjection::for_caller."
      - "Add approveApproval/rejectApproval idempotencyKey and approval_mutation_idempotency table."
      - "Implement ApprovalActionAttemptStore persistence and typed redaction envelope."
      - "Implement accessibility contract and macOS native alert delivery."
      - "Address ApprovalActionAttemptStore injected ownership and AppKit typed-state consumers."
  - id: "p081-phase6"
    title: "Phase 6: Compatibility Retirement and Coverage"
    status: "todo"
    tasks:
      - "Retire compatibility fixtures and enable check-boundary-coverage.sh."
      - "Implement resumable command_journal backfill."
      - "Address cleanup/backfill stalls, wildcard review controls, and readback non-disclosure."
"""

with open(f"{run_dir}/state/run-state.json", "r") as f:
    run_state = json.load(f)

run_state["current_state"] = "state_7_implementation_started"
run_state["next_state"] = "implementation_in_progress"
run_state["stage_id"] = "state_7_implementation_started"
run_state["stage_execution_id"] = "4ec255af-3ad5-419c-869a-2ea8bd70be0c"
run_state["agent_execution_id"] = "f70bfbdb-c4bd-4a6f-8551-e3a2d914588f"
run_state["work_item_id"] = "p058-invoke:4ec255af-3ad5-419c-869a-2ea8bd70be0c:0"
run_state["session_generation_id"] = "d50465a0-362e-4df1-b036-38bc57bfb02e"
run_state["gates"]["implementation_start"] = {
    "status": "passed",
    "reason": "Proposal review passed and worktree provisioned."
}

output = {
    "CHAINWORKS_OUTPUT": {
        f"{run_dir}/proposals/approved/proposal.md": proposal_content,
        f"{run_dir}/implementation/plan.md": plan_content,
        f"{run_dir}/implementation/backlog.yaml": backlog_content,
        f"{run_dir}/state/run-state.json": json.dumps(run_state, indent=2)
    }
}

with open("chainworks_output.json", "w") as f:
    json.dump(output, f, separators=(',', ':'))

print("Done")
