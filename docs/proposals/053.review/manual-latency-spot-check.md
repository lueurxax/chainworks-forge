# P053 Manual Latency Spot-Check

Date: `2026-04-23`
Tree: `main`
Scope: production-exposure replacement-sample validation for P053 control-plane/API/readback behavior

## Goal

Confirm that P053 keeps Forge-owned discovery work out of the pre-`initialize` path and that the remaining startup work is attributable to explicit ACP lifecycle phases rather than broad workspace traversal.

## Evidence Slice

- `control-plane/crates/acp/src/transport.rs` now records and returns:
  - `acp_pre_initialize_local_latency_ms`
  - `acp_initialize_latency_ms`
  - `acp_session_new_latency_ms`
  - `acp_prompt_duration_ms`
  - `acp_pre_prompt_metadata_latency_ms`
- `control-plane/crates/domain/src/discovery.rs` keeps broad traversal behind bounded/explicit discovery surfaces.
- `proposal-053` gate covers the no-pre-`initialize` traversal boundary and operation-recorder ordering:
  - `generated_state_denylist_matches_p053_roots`
  - `proposal_053_operation_recorder_observes_bounded_discovery_without_generated_state_reads`
  - `proposal_053_operation_recorder_orders_metadata_before_file_read`
  - `test_claude_adapter_executes_subprocess_and_returns_artifacts`

## Observed Metric

### ACP Fixture Gate

- Command:
  - `cargo test -p acp test_claude_adapter_executes_subprocess_and_returns_artifacts --test integration -- --nocapture`
- Observed `acp_pre_initialize_local_latency_ms`:
  - `0`
- Observation date:
  - `2026-04-23`

### Reference Workspace Measurement

- Proposal reference profile:
  - approximately `8.9 GB`
  - `126,643` files
- Measurement root:
  - `/Users/user/Documents/Chainworks Forge`
- Measurement tree:
  - `main` after merging `codex/p053-manual-merge-1833dd16`
- Current workspace profile observed before the manual run:
  - total root disk usage: `159G`
  - bounded non-generated file inventory: `637,617` files
  - inventory command excluded `.git`, `.chainworks/worktrees`, `control-plane/target`, `.forge-codex-acp`, and `.claude/worktrees`
- Manual command:
  - `CARGO_TARGET_DIR=target/proposal-053-gate CHAINWORKS_P053_REFERENCE_WORKSPACE_ROOT='/Users/user/Documents/Chainworks Forge' cargo test -p acp p053_manual_reference_workspace_pre_initialize_latency --test integration -- --ignored --exact --nocapture`
- Observed output:
  - `p053_manual_reference_workspace=/Users/user/Documents/Chainworks Forge acp_pre_initialize_local_latency_ms=0`
- Result:
  - `passed`
- Observation date:
  - `2026-04-23`

## Manual Conclusion

Result: `pass`

The same-tree command `./scripts/test-gate.sh proposal-053` passed on `2026-04-23`; see `docs/proposals/053.review/proposal-053-gate-2026-04-23.md`.

- Fresh ACP startup no longer depends on repository/workspace/worktree-wide discovery before `initialize`.
- Pre-prompt exact-path metadata is separately timed and bounded after session selection.
- Post-prompt settlement and supplemental discovery are now measured as explicit downstream phases instead of being hidden inside provider startup.
- The manual reference-workspace check ran against a workspace larger than the proposal's motivating `8.9 GB / 126,643-file` sample and still reported `acp_pre_initialize_local_latency_ms=0`.

## Remaining Boundary

- This spot-check is part of the approved replacement sample recorded in `cap-validation.json`.
- Direct production execution IDs were unavailable in the local closeout environment; the replacement sample is accepted for limited P053 control-plane/API/readback production exposure.
- Post-rollout production telemetry remains required for cap retuning, but it is not a blocker for P053 sign-off.
