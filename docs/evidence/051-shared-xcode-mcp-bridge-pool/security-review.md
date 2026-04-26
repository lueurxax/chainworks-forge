# P051 Targeted Security Review

Date: 2026-04-25

Status: fixture/readback security review complete; scoped broker/readback closeout is signed off. Broad release remains held by P042 release-host packaging proof.

Reviewer: Codex implementation audit, using the P051 `rust_security_reviewer`, `api_contract_reviewer`, and `observability_rollout_reviewer` lenses.

## Verdict

The implemented P051 fixture/readback path satisfies the targeted security controls required before merging the host-executor/shim-dispatch slice:

- `/xcode-mcp/{lease_id}` requires the lease-specific bearer token before marking a lease active.
- Broker backend process spawn clears inherited environment and restores only the host-user allowlist, including `MCP_XCODE_PID`; `CHAINWORKS_*` variables are excluded.
- Shim dispatch grants bind token id, token secret hash, lease id, peer uid, peer pid, provider process lineage, and process fingerprint.
- Shim response surfaces redact bearer tokens, lease tokens, `token=`, `access_token=`, `bearer_token=`, and `authorization=` values before returning socket output.
- Direct `mcpbridge` execution remains broker-only; diagnostic mode does not bypass `mcpbridge` containment.
- Host executor cwd policy fails closed outside the workspace, and Xcode command routing rejects unknown `xcrun` flags while allowing only the P051 non-consuming allowlist.
- Broker health remains subsystem-scoped: degraded Xcode lease acquisition does not collapse global daemon readiness for non-Xcode routes.

## Evidence

| Control | Evidence |
|---|---|
| Lease bearer authorization | `control-plane/crates/daemon/src/xcode_broker_http.rs`; test `xcode_mcp_route_requires_matching_lease_bearer_and_marks_active` |
| Backend environment allowlist | `control-plane/crates/acp/src/xcode_broker.rs`; test `xcode_mcp_bridge_pool_process_backend_spawns_with_target_env_and_rewrites_ids` |
| Shim token replay protection | `control-plane/crates/acp/src/xcode_shim.rs`; tests `rejects_stale_and_mismatched_shim_tokens`, `dispatch_rejects_bad_token_before_host_process` |
| Provider process binding | `control-plane/crates/acp/src/xcode_shim.rs`; peer pid/process tree/fingerprint mismatch tests |
| Token redaction | `control-plane/crates/acp/src/xcode_shim.rs`; test `socket_dispatch_response_redacts_token_bearing_surfaces` |
| Broker-only `mcpbridge` policy | `control-plane/crates/acp/src/xcode_shim.rs`; `control-plane/crates/workflow/src/direct_command.rs`; P051 workflow/shim parser tests |
| Health/readiness separation | `control-plane/crates/acp/src/xcode_broker.rs`; `control-plane/crates/daemon/src/xcode_broker_http.rs`; `control-plane/crates/graphql-server/src/schema.rs`; Swift `DaemonLifecycleClientTests` |
| Canonical gate | `./scripts/test-gate.sh proposal-051` passed on 2026-04-25 |

## Residual Rollout Holds

Live dogfood and scoped closeout sign-off are recorded in `docs/evidence/051-shared-xcode-mcp-bridge-pool/dogfood-signoff.md`. Before broad `shim_enforced` rollout, complete the production packaged-daemon release proof through P042 `proposal-042-packaging`.
