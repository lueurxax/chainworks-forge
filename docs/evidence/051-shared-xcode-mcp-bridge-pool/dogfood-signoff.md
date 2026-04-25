# P051 Dogfood And Sign-Off

Date: 2026-04-26

Status: `GO` for P051 closeout.

This artifact records live dogfood evidence separately from fixture evidence. It
records release-owner sign-off for the P051 broker/readback and live dev-daemon
dogfood scope.

## Release-Owner Sign-Off

| Field | Value |
|---|---|
| Decision | `GO` for P051 closeout |
| Signer | Operator / release owner, confirmed in Codex thread |
| Timestamp | 2026-04-26 00:42 +0300 |
| Scope | P051 broker/readback implementation plus live dogfood through `com.chainworks.forge.daemon.manual.p051` |
| Production rollout boundary | Production `com.chainworks.forge.daemon` SMAppService validation remains owned by P042 `proposal-042-packaging` before release or broad `shim_enforced` rollout |

## Completed Evidence

| Evidence | Status | Location |
|---|---|---|
| HTTP streaming feasibility research | Complete | `docs/proposals/051-shared-xcode-mcp-bridge-pool.review/http-streaming-feasibility.md` |
| Dependency audit | Complete | `docs/evidence/051-shared-xcode-mcp-bridge-pool/dependency-audit.md` |
| Targeted fixture security review | Complete | `docs/evidence/051-shared-xcode-mcp-bridge-pool/security-review.md` |
| Stable behavior reference | Complete | `docs/reference/xcode-mcp-bridge-pool.md` |
| P051 fixture gate registration | Complete | `scripts/test-gate.sh`, `docs/reference/test-gates.md` |
| Temporary live dogfood daemon config | Complete | `docs/evidence/051-shared-xcode-mcp-bridge-pool/dogfood-mcp-config.yaml` |
| Live dogfood workflow/catalog | Complete | `docs/evidence/051-shared-xcode-mcp-bridge-pool/dogfood-workflow.yaml`, `docs/evidence/051-shared-xcode-mcp-bridge-pool/dogfood-agents.yaml` |

## Fixture Gate Evidence

Fresh local fixture/readback validation was recorded on 2026-04-25:

- `./scripts/test-gate.sh proposal-051` passed.
- Latest Swift result bundle: `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/chainworks-test-gates/proposal-051-swift-20260425-234747.xcresult`.

The fixture gate proves local broker/readback behavior and Swift readback
contracts. It is not live dogfood sign-off.

## Live Runtime Substrate

| Field | Observed Value |
|---|---|
| Daemon label | `com.chainworks.forge.daemon.manual.p051` |
| Latest daemon PID | `46760` |
| Latest build SHA | `490e7934-p051-sleep1` |
| Health endpoint | `http://127.0.0.1:4000/health` returned `state=ready` |
| Broker health | `state=healthy`, `backend_available=true`, `can_acquire_new_xcode_leases=true`, `active_lease_count=0`, `observation_persistence_failures=0` |
| Xcode selector | `pid:36971` |
| Workspace | `/Users/user/Documents/Chainworks Forge` |
| Xcode snapshot | `Chainworks Forge - Chainworks_ForgeApp.swift`, App Shortcuts Preview, Organizer |

The live evidence uses the temporary dev daemon, not the production
`com.chainworks.forge.daemon` SMAppService path.

## Live Dogfood R3

R3 is the first completed direct P051 parallel Gemini Xcode dogfood after the
SQLite observation append serialization fix.

| Field | Observed Value |
|---|---|
| Run id | `52b3f96b-e43e-44d1-b645-5064fd94ffcf` |
| Idea id | `0a2f5c52-d971-46fe-9dd8-df457919bd62` |
| Workflow | `docs/evidence/051-shared-xcode-mcp-bridge-pool/dogfood-workflow.yaml` |
| Stage | `state_2_parallel_gemini_xcode_review` |
| Result | Run `completed`; both Gemini executions `completed` |
| UX execution | `e3cbd2e0-8534-490b-bdb6-1fa28c743674`, lease `lease-c187da99-93f1-4e7a-953a-f9d748fcde07` |
| UI execution | `2d1e501d-358f-4dbb-a510-55c317ade425`, lease `lease-fc68d6d5-796c-4a01-a66c-28ca8a448e05` |
| Parallelism | Both executions started at `2026-04-25T17:55:00Z`; sibling lease counts observed as `0` and `1` |
| Initialize wait | UX `0 ms`; UI `62 ms` |
| Initialize backend latency | UX `65 ms`; UI `47 ms` |
| Notification latency | Both `notifications/initialized` completed in `0 ms` |
| Terminal backend result | Both `tools/list` calls timed out after about `35 s` with `xcode_mcp_initialize_timeout` |
| Lease lifecycle | Both leases reached `lease_released` |
| Storage pressure | `truncated=false`, `total_events_dropped=0`, `mcp_broker_observations_dropped=0`, corrupt recovery count `0` |
| Broker health after run | `healthy`, `observation_persistence_failures=0` |

R3 proves live parallel lease allocation, initialize serialization, observation
persistence, readback completeness for both executions, and lease release. It
does not prove successful parallel `tools/list`.

## Live Dogfood R4

R4 was run after fixing broker notification forwarding so JSON-RPC notifications
are no longer rewritten into requests with synthetic backend ids.

| Field | Observed Value |
|---|---|
| Run id | `332aadd2-807f-44d8-9699-0286732db178` |
| Idea id | `8bfdf5c9-8962-4931-a672-3e51badddcb2` |
| Workflow | `docs/evidence/051-shared-xcode-mcp-bridge-pool/dogfood-workflow.yaml` |
| Stage | `state_2_parallel_gemini_xcode_review` |
| Result | Run `blocked`; stage `blocked` after one Gemini lane failed by provider idle timeout |
| UX execution | `40eff926-cade-40a5-b02f-7ff276250b7d`, completed, lease `lease-b6544ced-ebe8-4c4b-824c-5060532ca3d4` |
| UI execution | `911652ae-2172-4c20-8abd-4511ffc9c257`, failed, lease `lease-625397a0-952e-4633-9d40-53394954d6f1` |
| Initialize wait | UX `93 ms`; UI `0 ms` |
| Initialize backend latency | UX `51 ms`; UI `156 ms` |
| Notification latency | Both `notifications/initialized` completed in `0 ms` |
| Terminal backend result | Both `tools/list` calls timed out after about `35 s` with `xcode_mcp_initialize_timeout` |
| Lease lifecycle | Both leases reached `lease_released` after cleanup fix and daemon restart |
| Storage pressure | `truncated=false`, `total_events_dropped=0`, `mcp_broker_observations_dropped=0`, corrupt recovery count `0` |
| Broker health after cleanup | `healthy`, `active_lease_count=0`, `observation_persistence_failures=0` |

R4 proves the failed-session cleanup path no longer leaves live broker leases
behind. It still does not satisfy successful parallel `tools/list`.

## Live Dogfood R7

R7 was run after fixing the Gemini ACP capability-probe timeout path and
tightening the temporary dogfood output prompts so Gemini emits declared JSON
outputs through `CHAINWORKS_OUTPUT`.

| Field | Observed Value |
|---|---|
| Run id | `9c588c93-573f-4391-9f7c-fb69b15b906e` |
| Idea id | `7ca002cd-2001-4a62-b597-0a945ec78707` |
| Workflow | `docs/evidence/051-shared-xcode-mcp-bridge-pool/dogfood-workflow.yaml` |
| Stage | `state_2_parallel_gemini_xcode_review` |
| Result | Run `completed`; both Gemini executions `completed`; final state `state_3_workflow_complete` |
| UX execution | `p051_gemini_ux_xcode`, lease `lease-00e8260b-a1ad-4d8e-900a-d5d5d7b3d5ec` |
| UI execution | `p051_gemini_ui_xcode`, lease `lease-bb903517-d84e-45af-b93b-aa41657dcf66` |
| Parallelism | UX and UI executions started at `2026-04-25T19:58:59Z`; sibling lease counts observed as `0` and `1` |
| Initialize wait | UX `54 ms`; UI `0 ms` |
| Initialize backend latency | UX `42 ms`; UI `62 ms` |
| Notification latency | Both `notifications/initialized` completed in `0 ms` |
| Tools/list result | UX `tools/list` completed in `5200 ms`; UI `tools/list` completed in `3878 ms` |
| Follow-up tools | UX recorded five successful `tools/call` completions; UI recorded six successful `tools/call` completions |
| Lease lifecycle | Both leases reached `lease_closing` and `lease_released` |
| Broker health after run | `healthy`, `active_lease_count=0`, `observation_persistence_failures=0` |
| Output settlement | `proposal_review_ux` and `proposal_review_ui` normalized artifacts were produced |

R7 satisfies the live parallel Xcode dogfood path: both Gemini agents acquired
brokered Xcode MCP leases, initialized through the broker, successfully listed
tools, used Xcode tools, emitted declared outputs, and released leases.

## Live Dogfood R8

R8 was run after adding a shared initialized process backend per
`run_id + Xcode pid + developer_dir`. It was intentionally cancelled after one
Gemini lane failed to emit the required structured output and the other held a
lease for about 9 minutes. The failed run exposed a cancellation cleanup gap
that has since been fixed and covered by a regression test.

| Field | Observed Value |
|---|---|
| Run id | `d8feca2a-6f63-4b70-b022-c1d8af492e6e` |
| Idea id | `8a581d9a-23fa-45fe-b203-b32c6f2061fb` |
| Workflow | `docs/evidence/051-shared-xcode-mcp-bridge-pool/dogfood-workflow.yaml` |
| Stage | `state_2_parallel_gemini_xcode_review` |
| Result | Cancelled after structured-output failure/stale provider lane; not release evidence |
| UX execution | `1a7aef22-2bb0-4088-8f16-4a76fb42d762`, lease `lease-9af56443-678b-4c8e-a5dc-c29e28cfc4c5` |
| UI execution | `58f09015-ad96-4d16-b1c0-6c4cd00c4e71`, lease `lease-80288e70-e20f-4cc4-8c08-ee0dd8b40a5e` |
| Shared backend evidence | Both leases used backend process `29020`; only one `mcpbridge` process was visible for two Gemini ACP processes |
| Initialize behavior | First initialize completed in `186 ms`; second initialize completed in `0 ms` through the cached initialized backend |
| Tools/list result | Both leases completed `tools/list` in about `1.7 s` |
| Cancellation cleanup finding | Cancellation settlement reported `session_close_succeeded=false` and left one active broker lease until daemon restart |
| Fix added after R8 | `AcpRuntimeManager::close_session()` now releases orphaned Xcode lease cleanup even when the live ACP session is already missing |
| Regression evidence | `cargo test -p acp close_session_releases_orphaned_xcode_lease_cleanup_when_live_session_is_missing -- --nocapture` passed |

R8 is negative evidence for the old cancellation cleanup behavior and positive
evidence that modal-deduplicated backend sharing was active before the final
successful dogfood.

## Live Dogfood R9

R9 was run after the orphaned lease cleanup fix, daemon restart, and a tighter
temporary dogfood catalog prompt that requires explicit
`CHAINWORKS_OUTPUT` markers.

| Field | Observed Value |
|---|---|
| Run id | `59392967-c209-4a07-a8b1-2a407a27a4c8` |
| Idea id | `8e20e04d-46aa-45b5-9625-493dbaca5d36` |
| Workflow | `docs/evidence/051-shared-xcode-mcp-bridge-pool/dogfood-workflow.yaml` |
| Stage | `state_2_parallel_gemini_xcode_review` |
| Result | Run `completed`; both Gemini executions `completed`; final state `state_3_workflow_complete` |
| UX execution | `4ac30287-f0bc-434c-ba30-c503a99accab`, lease `lease-23f0455f-5dea-4ac1-8acd-53abd6a6d83d` |
| UI execution | `a29c5abc-7e62-4673-be54-af31e3fb3e26`, lease `lease-267867d7-dc40-4e17-bfbc-02abe9f302b5` |
| Shared backend evidence | Both leases used backend process `38015`; process list showed one `mcpbridge` for two Gemini ACP processes |
| Initialize behavior | First initialize completed in `112 ms`; second initialize completed in `0 ms` on the same backend process |
| Notification behavior | Both `notifications/initialized` completed in `0 ms` |
| Tools/list result | UX `tools/list` completed in `9714 ms`; UI `tools/list` completed in `9725 ms` |
| Follow-up tools | UX recorded one successful `tools/call`; UI recorded two successful `tools/call` completions |
| Lease lifecycle | Both leases reached `lease_closing` and `lease_released` |
| Broker health after run | `healthy`, `active_lease_count=0`, `observation_persistence_failures=0` |
| Output settlement | `proposal_review_ux` and `proposal_review_ui` normalized artifacts were produced |
| Token leakage review | Scoped R9 artifact/evidence search found no raw bearer or shim token matches; matches were limited to evidence docs naming the search patterns |
| Operator modal observation | Operator reported one real Xcode consent modal when the two brokered Gemini sessions started. A later modal was also seen near the end, but the operator could not attribute it to the same run because daemon/backend restarts were in progress. |

R9 satisfies the live parallel Xcode dogfood path with modal-deduplicated backend
sharing: both Gemini agents acquired brokered Xcode MCP leases, shared one
initialized `mcpbridge` process for the run/Xcode target, successfully listed
tools, used Xcode tools, emitted declared outputs, and released leases.

## Sleep/Wake Recovery Hardening

Post-R9 operator review identified that the engine had host-interruption
recovery for system sleep, wall-clock gaps, and network migration, but the live
daemon composition root did not start those monitors. The daemon now wires
`HostInterruptionService` with ACP runtime cleanup, native macOS sleep/wake
notifications, network migration notifications, and the heartbeat gap detector.

Intended behavior after closing and reopening the laptop is recovery by fresh
retry, not reconnecting old in-flight provider/MCP streams. Running executions
that overlap the host interruption are closed through ACP runtime cleanup,
marked `cancelled`, recorded under a host-interruption epoch, and requeued with
jitter/capacity controls. Any Xcode MCP leases owned by those live sessions are
released before retry enqueue.

Verification:

- `cargo test -p daemon --no-run` passed.
- `cargo test -p engine host_interruption_records_epoch_cancels_execution_and_requeues_invoke_work -- --nocapture` passed.
- `cargo test -p engine host_interruption_requires_runtime_cleanup_before_retry_enqueue -- --nocapture` passed.
- `cargo test -p daemon native_event_bridge_maps_system_sleep_wake_event -- --nocapture` passed.
- `cargo test -p daemon native_event_bridge_maps_network_migration_event -- --nocapture` passed.

## Direct Xcode MCP Probe

After clearing stale `mcpbridge` helper processes and restarting the dev daemon,
a direct local probe against `MCP_XCODE_PID=36971 xcrun mcpbridge` completed
`initialize` in about `0.13 s` and `tools/list` in about `9.5 s`.

This confirmed the current Xcode MCP backend could answer `tools/list` before
R7 proved the same path through the HTTP broker.

## Token-Leakage Spot Check

Scoped search covered R7 run artifact directories, R7 dogfood artifact root, and
P051 evidence docs. No raw `MCP_XCODE_TOKEN`,
`CHAINWORKS_XCODE_SHIM_TOKEN`, `Authorization: Bearer ...`,
`access_token=...`, `bearer_token=...`, or long raw bearer token match was found.

## Gate Evidence

`./scripts/test-gate.sh proposal-051` passed on `2026-04-26 00:30 +0300`.
Swift result bundle:
`/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/chainworks-test-gates/proposal-051-swift-20260426-002906.xcresult`.

The gate pass required an XCTest-host bootstrap guard so unit tests do not launch
the full operator UI or packaged daemon while validating non-UI P051 surfaces.

## Production SMAppService Scope-Out

P051 closeout scope is the broker/readback implementation plus live dogfood
through the temporary dev daemon label `com.chainworks.forge.daemon.manual.p051`.
Production packaged daemon validation for `com.chainworks.forge.daemon` remains
owned by the P042 release-host packaging lane.

Owner: release engineering / P042 packaging.

Acceptance before release or broad `shim_enforced` rollout:

- run `./scripts/test-gate.sh proposal-042-packaging` on a configured release
  host with `scripts/packaging.env`;
- verify Developer ID signing, Team ID match, notarization staple,
  Gatekeeper assessment, and packaged app launch-to-Ready;
- attach the generated P042 evidence log before removing the P051 release hold.

Local developer evidence does not satisfy this production acceptance. The
current workstation has no registered `com.chainworks.forge.daemon` service;
the active proof remains the manual dogfood daemon.

## Acceptance Table

| Field | Required Evidence | Current Value |
|---|---|---|
| Dogfood run id | Real Chainworks run id for a parallel Xcode-capable stage | R7 `9c588c93-573f-4391-9f7c-fb69b15b906e` completed |
| Workflow/stage | Workflow and stage names proving parallel Xcode-capable execution | Satisfied for direct dogfood workflow and `state_2_parallel_gemini_xcode_review` |
| Provider/runtime | Runtime family and version used for Xcode-capable agents | Gemini ACP agents using backend `gemini_review_pro_xcode` with brokered `xcode` MCP |
| Xcode target | Workspace identity and Xcode PID/snapshot used by broker | `pid:36971`, workspace `/Users/user/Documents/Chainworks Forge` |
| Modal count | At most one Xcode consent modal per Xcode process | Satisfied for observed parallel R9 start: operator saw one real Xcode consent modal when two brokered Gemini sessions started; backend evidence shows one real `initialize` and one `mcpbridge` process for the run. A later modal was seen during restart/debug activity and is not attributed to the same run. |
| Fake-home boundary | Zero enforced-boundary fake-home failures | Satisfied for observed leases: `host_operator_home_available`, `darwin_tmpdir_available` |
| Observation completeness | Every Xcode-capable execution has complete readback | Satisfied for R9; both leases have reserve, active, initialize, `tools/list`, `tools/call`, close, and release observations |
| Token leakage review | No raw MCP bearer/shim token in logs/reports/UI/artifacts | Scoped artifact/evidence search found no raw token matches |
| Observation pressure | Retry exhaustion, truncation, append pressure | No broker persistence failures; health reports `observation_persistence_failures=0` after R9 |
| Parallel tools success | Cross-lease tools after initialize | Satisfied in R9: both parallel leases completed `tools/list` and follow-up `tools/call` through the broker |
| Production packaged daemon | Release-host SMAppService packaging validation | Scoped out of P051 closeout; remains owned by P042 `proposal-042-packaging` before release/broad rollout |
| Operator/release-owner decision | Explicit `GO`/`HOLD` with signer and timestamp | `GO`; operator/release owner confirmed P051 closeout scope at 2026-04-26 00:42 +0300 |

## Stop Sign

P051 broker/readback closeout is operator-signed-off for the scoped evidence
above.

Do not mark release/broad `shim_enforced` rollout ready until the production
`com.chainworks.forge.daemon` SMAppService path is validated by P042
`proposal-042-packaging`.
