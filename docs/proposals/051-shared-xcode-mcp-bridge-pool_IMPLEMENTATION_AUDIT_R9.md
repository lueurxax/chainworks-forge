# P051 Implementation Audit R9

Proposal: `docs/proposals/051-shared-xcode-mcp-bridge-pool.md`
Audit timestamp: 2026-04-26T00:18:17+0300
Audit target: current worktree
Current git HEAD: `490e79343953903a0680253d771cf4785306258e` (`490e7934 Use current macOS runner for CI`)
Report path: `docs/proposals/051-shared-xcode-mcp-bridge-pool_IMPLEMENTATION_AUDIT_R9.md`

## Decision

Decision: **NOT READY FOR CLOSEOUT**.

Overall proposal conformance: **Partially Implemented**.
Overall implementation readiness: **Not Ready**.

The fixture/readback implementation is now materially present: the shared backend model is implemented in Rust, the current reference doc describes that model, dogfood evidence records successful R9 shared-backend behavior, and the canonical `proposal-051` gate passed in this audit. The remaining blockers are not broad implementation absence; they are closeout blockers:

- stale top-level proposal text still contradicts the accepted shared-backend architecture;
- live dogfood is still explicitly `HOLD` pending release-owner `GO` and production SMAppService validation or scope-out;
- the P051-relevant implementation/docs/evidence files are still dirty or untracked in the audited worktree.

## Reviewer Routing

Prior proposal-review discovery found only proposal-local evidence/research artifacts:

- `docs/proposals/051-shared-xcode-mcp-bridge-pool.review/dependency-audit.md`
- `docs/proposals/051-shared-xcode-mcp-bridge-pool.review/http-streaming-feasibility.md`

No durable reviewer-selection artifact with detected reviewer IDs was found. Prior `IMPLEMENTATION_AUDIT` reports were ignored for reviewer selection per audit rules. Reuse classification: **Not reused**; routing was reconstructed from the current proposal contract, repo-local routing expectations, and the implementation surface.

Selected implementation reviewers/lenses:

- `rust_arch_reviewer`: ACP broker, shared process backend, runtime manager, daemon ownership boundaries.
- `rust_reliability_reviewer`: lease lifecycle, backend failure cleanup, host interruption, close-session cleanup.
- `rust_security_reviewer`: fail-closed capability, bearer/shim token boundary, policy before forwarding, token redaction evidence.
- `api_contract_reviewer`: ACP HTTP MCP contract, GraphQL/MCP/report readback, observation envelope.
- `observability_rollout_reviewer`: gates, health, dogfood evidence, rollout stop signs, follow-up triggers.

Not selected:

- `macos_ui_reviewer`: Swift scope is a narrow read-only status/timeline readback surface and was covered by the targeted Swift gate; no broad UI implementation change was introduced in this R9 delta.
- Go/iOS/performance reviewers: no matching implementation surface or benchmark-level acceptance target was in scope for this audit.

## Evidence Reviewed

- Proposal stale top-level goals: `docs/proposals/051-shared-xcode-mcp-bridge-pool.md:61-72`.
- Proposal stale resolved-feedback text: `docs/proposals/051-shared-xcode-mcp-bridge-pool.md:266-270`.
- Proposal current shared-backend model: `docs/proposals/051-shared-xcode-mcp-bridge-pool.md:1199-1203`.
- Proposal acceptance criteria: `docs/proposals/051-shared-xcode-mcp-bridge-pool.md:1829-1840`.
- Proposal follow-up blockers: `docs/proposals/051-shared-xcode-mcp-bridge-pool.md:1956-1970`.
- Current reference truth: `docs/reference/xcode-mcp-bridge-pool.md:69-75`.
- Dogfood stop sign: `docs/evidence/051-shared-xcode-mcp-bridge-pool/dogfood-signoff.md:5-8`, `:36-46`, `:248-258`.
- Dependency audit posture: `docs/proposals/051-shared-xcode-mcp-bridge-pool.review/dependency-audit.md:14-26`; `docs/evidence/051-shared-xcode-mcp-bridge-pool/dependency-audit.md:9-29`.
- Shared process backend implementation: `control-plane/crates/acp/src/xcode_broker.rs:124-144`, `:214-239`, `:382-455`, `:821-852`, `:860-1005`, `:1901-1912`.
- Policy enforcement before shared backend forwarding: `control-plane/crates/acp/src/xcode_broker.rs:770-818`.
- Shared-backend regression test: `control-plane/crates/acp/tests/integration.rs:2501-2667`.
- Canonical gate registration: `scripts/test-gate.sh:2447-2536`.

## Review Finding Updates

### Finding 1: P051 remains dependency-blocked

Status: **Resolved for fixture/readback scheduling; still not a release closeout sign-off**.

The current dependency audit no longer treats missing historical P025/P026 proposal lineage files as current P051 fixture blockers. It records implemented reference artifacts and registered gates for P025/P026/P029, with P037/P049 kept as rollout compatibility dependencies rather than scaffold blockers. Broad release remains gated by dogfood/sign-off, not dependency artifact recovery.

### Finding 2: Follow-up blockers need draft proposals

Status: **Resolved inside P051 scope**.

P051 now includes concrete follow-up IDs with trigger, owner, scope, and acceptance criteria:

- `P051-FU-01`: sandbox/libc-audit command-boundary hardening.
- `P051-FU-02`: normalized observation event-table path and migration guardrails.

There are still no separate draft proposal files for those follow-ups, but the prior review allowed either linked drafts or explicit fallback work inside P051 scope. The current P051 text now provides the latter.

## Requirement Conformance

| ID | Requirement | Status | Evidence |
|---|---|---|---|
| `REQ-001` | Use Chainworks-owned HTTP streaming endpoints for Xcode MCP instead of provider-owned `xcrun mcpbridge` stdio. | Implemented | ACP broker lease attachment and HTTP MCP forwarding are implemented and covered by `xcode_mcp_bridge_pool_` tests; gate passed. |
| `REQ-002` | Preserve provider fake-home isolation while routing Xcode-bound work through host-session boundary. | Implemented | Proposal/reference preserve fake-home boundary; backend launch records host `HOME`, `TMPDIR`, `DEVELOPER_DIR`; no direct host-home provider grant was found. |
| `REQ-003` | Fail closed before lease/token/backend/session allocation when provider lacks HTTP MCP capability. | Implemented | `brokered_xcode_probe_fails_closed_when_provider_lacks_http_mcp` and `unsupported_brokered_xcode_provider_fails_before_probe_spawn` are in the gate-covered ACP test slice. |
| `REQ-004` | Guard direct Xcode shell commands through PATH shims and catalog lint; keep `mcpbridge` broker-only. | Implemented | Proposal acceptance and test-gate stale checks cover the boundary; security-review evidence exists. |
| `REQ-005` | Preserve per-agent MCP policy, session reuse semantics, and recoverability. | Implemented | Per-lease `BrokerMcpPolicy` is stored on the lease and applied before tool forwarding/filtering; close-session cleanup and failure cleanup focused tests passed. |
| `REQ-006` | Add durable typed Xcode runtime observations and expose them through GraphQL/MCP/UI readback. | Implemented | DB/engine/GraphQL/MCP compile and targeted Swift readback tests passed in `proposal-051`; dogfood health reports no observation persistence failures. |
| `REQ-007` | Shared backend model: one initialized backend per `run_id + Xcode pid + developer_dir`, isolated HTTP leases/policies, one real initialize, ordered pump. | Partially Implemented | Code/reference/main proposal sections implement the shared model, and the shared-backend test proves one spawn/one initialize with sibling leases. However stale proposal lines still say one backend per provider HTTP lease and independent leases/backends. |
| `REQ-008` | Broker capacity, backpressure, disabled state, and health are observable. | Implemented | Gate-covered ACP tests exercise capacity/queueing/health; runtime health returned `state=ready` and broker `state=healthy`. |
| `REQ-009` | Register and pass staged gates `p051-scaffold` and `proposal-051|p051`. | Implemented | `scripts/test-gate.sh` registers both gates; `./scripts/test-gate.sh proposal-051` passed during this audit. |
| `REQ-010` | Live dogfood/sign-off gates broad completion. | Partially Implemented | Dogfood evidence records successful fixture/live checks, but the artifact is still `HOLD`, not release-owner `GO`, and production SMAppService path is not validated or scoped out. |
| `REQ-011` | Dependency and rollout follow-up blockers are explicit enough to avoid unnamed-future stalls. | Implemented | Dependency audit is reconciled; P051 now carries `P051-FU-01` and `P051-FU-02` with trigger/owner/scope/acceptance. |
| `REQ-012` | Implementation state is durable repository truth at audit time. | Partially Implemented | Current worktree contains P051-relevant dirty tracked files and untracked dogfood YAML evidence; this is not yet committed durable truth. |

## Routed Findings

### ARCH-001 - Stale proposal goals still contradict the implemented shared-backend architecture

Severity: **Major**
Reviewer: `rust_arch_reviewer`

The current implementation and reference now use one initialized `xcrun mcpbridge` backend per `run_id + Xcode pid + developer_dir`, with sibling HTTP leases mapped to that backend and policy enforced at the broker facade. That is consistent with `docs/reference/xcode-mcp-bridge-pool.md:69-75`, `docs/proposals/051-shared-xcode-mcp-bridge-pool.md:1199-1203`, and the regression test at `control-plane/crates/acp/tests/integration.rs:2501-2667`.

However the top-level proposal goals still say "Spawn one backend `xcrun mcpbridge` subprocess per provider HTTP lease" and allow tools parallelism across "independent leases and backends" at `docs/proposals/051-shared-xcode-mcp-bridge-pool.md:61-64`. The resolved-feedback section repeats "Each lease has one stdio backend" at `docs/proposals/051-shared-xcode-mcp-bridge-pool.md:266-270`.

Impact: future implementers can still follow the stale top-level goals and reintroduce the earlier modal/consent duplication model. This is a documentation-contract blocker for closeout, even though the Rust implementation now follows the accepted shared-backend design.

Recommended action: update the stale top-level goal and resolved-feedback text to match the shared-backend model, and extend the `p051-scaffold` stale-guidance guard to catch "per provider HTTP lease" and "independent leases and backends" wording unless explicitly marked as rejected historical context.

### READY-001 - Release-owner stop sign remains HOLD

Severity: **Major**
Reviewer: `observability_rollout_reviewer`

The dogfood artifact explicitly says `Status: HOLD pending explicit release-owner GO`, does not claim release-owner sign-off, and records that live evidence uses the temporary dev daemon label rather than the production `com.chainworks.forge.daemon` SMAppService path. Its stop sign says not to mark P051 fully complete, release-ready, or operator-signed-off until the production path is validated or scoped out and an explicit human `GO` is attached.

Impact: the implementation can be considered fixture/readback-complete enough for further dogfood, but not closeout/release-ready.

Recommended action: either validate the production SMAppService path or explicitly scope that validation out of P051 closeout with owner/acceptance, then attach a signed human `GO` to the dogfood artifact.

### READY-002 - P051-relevant implementation and evidence are not durable clean repo truth

Severity: **Major**
Reviewer: `observability_rollout_reviewer`

The audited worktree is dirty. P051-relevant tracked files include `control-plane/crates/acp/src/xcode_broker.rs`, `control-plane/crates/acp/src/manager.rs`, `control-plane/crates/acp/src/transport.rs`, `control-plane/crates/daemon/src/main.rs`, `control-plane/crates/workflow/tests/integration.rs`, `docs/proposals/051-shared-xcode-mcp-bridge-pool.md`, `docs/reference/xcode-mcp-bridge-pool.md`, `docs/evidence/051-shared-xcode-mcp-bridge-pool/dogfood-signoff.md`, and `scripts/test-gate.sh`. P051 dogfood YAML files are also untracked.

Impact: the audit can validate the current worktree, but closeout should not treat this as durable repository truth until the P051 slice is committed or otherwise preserved in a durable branch/patch artifact.

Recommended action: commit or otherwise durably preserve the P051-relevant code, docs, reference, evidence, and gate changes before closeout. Keep unrelated dirty P031/P053/P058/P061/P068/P069/P070 work out of the P051 closeout decision.

## Validation Run

Canonical gate:

```bash
./scripts/test-gate.sh proposal-051
```

Result: **passed**.

Notable gate coverage observed:

- `p051-scaffold` passed.
- `cargo test -p workflow --test integration p051_` passed.
- `cargo test -p db --test integration proposal_051_xcode_runtime_observation` passed.
- `cargo test -p acp --test integration xcode_mcp_bridge_pool_` passed, including the shared initialized backend regression.
- `cargo test -p engine --test integration xcode_broker_fail_closed_observation_is_persisted_from_acp_sink -- --exact` passed.
- `cargo check -p graphql-server` passed.
- `cargo check -p mcp-server` passed.
- Swift targeted tests passed:
  - `Chainworks ForgeTests/RunTimelineInspectorViewTests`
  - `Chainworks ForgeTests/DaemonLifecycleClientTests`

Swift result bundle:

```text
/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/chainworks-test-gates/proposal-051-swift-20260426-001412.xcresult
```

Focused runtime checks run during this audit:

```bash
cd control-plane
cargo test -p acp close_session_releases_orphaned_xcode_lease_cleanup_when_live_session_is_missing -- --nocapture
cargo test -p engine host_interruption_records_epoch_cancels_execution_and_requeues_invoke_work -- --nocapture
cargo test -p engine host_interruption_requires_runtime_cleanup_before_retry_enqueue -- --nocapture
cargo test -p daemon native_event_bridge_maps_system_sleep_wake_event -- --nocapture
cargo test -p daemon native_event_bridge_maps_network_migration_event -- --nocapture
```

Result: **all passed**.

Runtime health check:

```text
GET http://127.0.0.1:4000/health
state=ready
build_sha=490e7934-p051-sleep1
pid=46760
xcode_broker_health.state=healthy
backend_available=true
can_acquire_new_xcode_leases=true
active_lease_count=0
observation_persistence_failures=0

GET http://127.0.0.1:4000/xcode-mcp/health
state=healthy
backend_available=true
can_acquire_new_xcode_leases=true
active_leases=0
queued_leases=0
```

## Closeout Checklist

To move P051 from this R9 state to closeout-ready:

1. Replace stale proposal goal/resolution text at `docs/proposals/051-shared-xcode-mcp-bridge-pool.md:63-64` and `:270` with the shared-backend model, or explicitly mark the old wording as rejected historical context.
2. Add a stale-guidance guard for the old backend-per-lease wording to `p051-scaffold`.
3. Commit or otherwise durably preserve the P051-relevant dirty/untracked implementation, reference, evidence, and gate files.
4. Validate the production SMAppService path or explicitly scope it out of P051 closeout.
5. Attach the release-owner `GO` to the dogfood artifact.

After those are done, rerun:

```bash
./scripts/test-gate.sh proposal-051
```

Expected next audit decision if the gate still passes and the stop sign is cleared: **READY FOR CLOSEOUT**.
