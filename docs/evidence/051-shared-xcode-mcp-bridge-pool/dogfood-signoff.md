# P051 Dogfood And Sign-Off

Date: 2026-04-25

Status: blocked by real live dogfood and explicit operator/release-owner sign-off.

This artifact separates fixture/research evidence that exists from live dogfood evidence that is still required. It intentionally does not fabricate an operator sign-off.

## Completed Evidence

| Evidence | Status | Location |
|---|---|---|
| HTTP streaming feasibility research | Complete | `docs/proposals/051-shared-xcode-mcp-bridge-pool.review/http-streaming-feasibility.md` |
| P051 scaffold gate registration | Complete | `scripts/test-gate.sh`, `docs/reference/test-gates.md` |
| P051 full fixture gate registration | Complete | `scripts/test-gate.sh`, `docs/reference/test-gates.md` |
| Stable behavior reference | Complete | `docs/reference/xcode-mcp-bridge-pool.md` |
| Dependency audit | Complete | `docs/evidence/051-shared-xcode-mcp-bridge-pool/dependency-audit.md` |

## Fixture/Static Gate Evidence

Required before claiming fixture readiness:

- `./scripts/test-gate.sh p051-scaffold`
- `./scripts/test-gate.sh proposal-051` or `./scripts/test-gate.sh p051`

Fresh local fixture/readback validation was recorded on 2026-04-25:

- `./scripts/test-gate.sh proposal-051` passed.
- Swift result bundle: `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/chainworks-test-gates/proposal-051-swift-20260425-102233.xcresult`.

## Live Dogfood Required For Full P051 Completion

P051 pre-ship acceptance requires a real parallel Xcode-capable dogfood run.

Required fields before this status can become signed off:

| Field | Required Evidence | Current Value |
|---|---|---|
| Dogfood run id | Real Chainworks run id for a parallel Xcode-capable stage | Not run |
| Workflow/stage | Workflow and stage names proving parallel Xcode-capable execution | Not run |
| Provider/runtime | Runtime family and version used for Xcode-capable agents | Not recorded |
| Xcode target | Workspace identity and Xcode PID/snapshot used by broker | Not recorded |
| Modal count | Evidence that at most one Xcode consent modal appeared per Xcode process | Not recorded |
| Fake-home boundary | Evidence of zero enforced-boundary fake-home failures | Not recorded |
| Observation completeness | Evidence that every Xcode-capable execution has complete `actual_xcode_runtime_observation_json` readback | Not recorded |
| Token leakage review | Evidence that logs/reports/UI redact broker lease tokens | Not recorded |
| Observation pressure | Retry exhaustion, truncation percentage, append backoff/latency spikes | Not recorded |
| Operator/release-owner decision | Explicit `GO`/`HOLD` with signer and timestamp | Not signed |

## Stop Sign

Do not mark P051 fully complete, release-ready, or operator-signed-off until the live dogfood table above is filled with real evidence and an explicit human decision.

The registered `proposal-051|p051` gate is repo-appropriate as a fixture/readback gate. It should not fail solely because live dogfood is absent, because dogfood requires a real operator environment and would make ordinary local validation non-reproducible. The readiness stop sign is this artifact plus the P051 reference docs.
