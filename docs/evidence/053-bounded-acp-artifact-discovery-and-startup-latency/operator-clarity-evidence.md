# P053 Operator Clarity Evidence

Date: `2026-04-23`
Tree: `main`
Scope: production-exposure replacement-sample validation for P053 control-plane/API/readback behavior

## Goal

Confirm that operator-facing readback can distinguish Forge overhead from provider latency, instead of collapsing the whole startup interval into an opaque “provider is slow” impression.

## Evidence

The durable discovery payload now carries separate structured fields for:

- `acp_pre_initialize_local_latency_ms`
- `acp_initialize_latency_ms`
- `acp_session_new_latency_ms`
- `acp_prompt_duration_ms`
- `acp_pre_prompt_metadata_latency_ms`
- `acp_control_plane_manifest_latency_ms`
- `acp_exact_output_acceptance_latency_ms`
- `acp_meta_root_discovery_latency_ms`
- `acp_git_changed_files_latency_ms`

The same payload also exposes decision-state clarity signals:

- `acp_git_manifest_status`
- `acp_resume_discovery_warning`
- `acp_exact_output_acceptance_timeout`
- `acp_exact_output_aggregate_cap_hit`
- `acp_meta_discovery_truncated`
- `acp_meta_discovery_truncation_reason`
- `acp_missing_required_output_count`
- `acp_rejected_output_count`
- `acp_stale_output_count`

Readback coverage remains available through:

- DB-backed `agent_execution_discovery_diagnostics`
- GraphQL execution surfaces
- MCP reports/resource readback

## Manual Conclusion

Result: `pass`

The same-tree command `./scripts/test-gate.sh proposal-053` passed on `2026-04-23`; see `docs/evidence/053-bounded-acp-artifact-discovery-and-startup-latency/proposal-053-gate-2026-04-23.md`.

- Forge-owned overhead is now named and structured instead of being hidden inside provider startup.
- Missing/stale/rejected output outcomes remain separate from transport timing.
- The data shape needed by future P069 UI is already durable without requiring that UI to exist in P053.
