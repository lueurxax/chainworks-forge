# P070 C output ownership and frozen retry settlement

## Scope and evidence

Implementation branch: `codex/p070-output-and-escalation-recovery`, based on
`366643e2244f4e8820f16a8a5279eb2b9de018a2`. This includes the existing project-trust
fix `da399247`; the implementation does not change trust admission or grant trust.

The originating live evidence is
`/Users/user/Documents/Chainworks Forge/docs/evidence/project-trust-live-retry-2026-09-21.md`.
It describes run `0d3908d4-b551-47f3-bbb4-eb881ffebac4`, retry stage
`94d8588e-8efe-428e-9c21-0e809da7409d` and automatic fallback stage
`da60c0fc-2b89-4b5f-b491-f9422ceae674`.

Read-only MCP, live SQLite diagnostics and archived provider output confirmed:

- Codex execution `8eb0995f-c97a-494e-900a-ef007e262dd0` and Claude execution
  `abc857a6-58a2-4b3c-ab9e-7091afee27e2` emitted `run_state` manifests with 3987
  bytes and digest `c93d5c95abe768ab66d050a44228ec76fc7328274ab48f016c7218a9511bde02`.
  Both values exactly match the pre-prompt baseline. The file identifies SQLite
  as its owner and uses `run-state-projection.v1`.
- Both attempts used fresh provider sessions. The archived frozen mission still
  lists `run_state` among provider outputs; its declaration has no schema.
- The accepted Codex-to-Claude profile change is stored under
  `targeted_retry.provider_fallback`, with reason `p058_backend_profile_tier`.
  Top-level `provider_health_fallback` is null. The frozen policy permits this
  route on `contract_output_failure`.
- Claude's execution metadata belongs to tier 1. Its failure advanced the mutable
  ledger to tier 2 before the next `advance_run` validated the historical Claude
  source payload. No next-tier invocation was created; the V1 validator rejected
  the different `backend_profile_id`, leaving the run/stage running without queued
  or running work.

## Root causes and correction

### Generated projection incorrectly required fresh agent bytes

The canonical run-state projection is DB owned and has no artificial generation
timestamp. Identical bytes across attempts are valid. Expected-output construction
nevertheless assigned `Agent/MustProduce`, and direct-file acceptance rejected the
unchanged digest. Its generic diagnostic hid the specific branch.

Runtime ownership now recognizes only the exact machine `run_state` target under
the current run meta root, with absent schema or `run_state_projection_v1`.
Frozen mission lists and hashes remain unchanged. The final invocation directive
explains that this output is generated and read-only. Before acceptance the
executor rebuilds current DB truth, verifies run/path/schema/owner identity,
exports through the protected writer and settles captured DB bytes. Provider
manifests and subsequent file modifications cannot replace those bytes.

Output and aggregate caps still apply. Missing, unsafe or invalid generated output
fails as an engine persistence failure; it cannot authorize provider repair of
the projection. Original, transcript and repair settlement preserve control-plane
ownership. Declared/supplemental artifact import and P090 staging exclude it from
agent-owned generation publication, including canonical path aliases and rejected
outputs. Directory replacement cannot enter the agent directory-output lane.
Generated-projection failures retain inspect-only runtime facts, and cannot
trigger provider health fallback or escalation through a historical ledger.

Agent-owned direct-file references still require fresh bytes under `MustProduce`.
Explicit reuse works after the same path/digest/size checks, with declared-reuse
provenance. Diagnostics distinguish identity, path confinement, file type/read,
size cap, digest/size mismatch, absent baseline and unchanged content.

### Historical escalation profile compared with primary authority

V1 validation recognized health fallback but not the targeted escalation form.
It now checks targeted profile changes against frozen profiles/tier policy and
durable retry/source execution/work-item lineage. Provider aliases are normalized.
The executed tier comes from immutable execution metadata, not the ledger's
mutable current tier. Nullable profile fields are cleared when omitted by the
new profile, preventing stale settings from the previous provider.

Failed-stage retry scheduling catches incompatible frozen-payload errors and
atomically persists `retry_payload_authority_invalid`, settles the stage failed,
blocks the run and terminalizes its active retry authority. Cancellation,
completed stages and newer retry authority are preserved; DB write failure rolls
back the whole settlement.

## Validation

All focused checks passed, using managed Cargo with automatic cache cleanup
disabled and serial test execution. Logs are under
`/private/tmp/cw-p070-recovery-tests`.

| Engine unit selection | Passed |
| --- | ---: |
| `run_state_output_tests` | 11 |
| `direct_file_ref_tests` | 8 |
| `expected_output_specs_` | 7 |
| `runtime_invocation_contract_` | 9 |
| `p058_v1_` | 10 |
| `p058_escalation_retry_` / `auto_contract_output_retry_` | 1 / 1 |
| `agent_context_` | 7 |
| `proposal_053_` | 17 |
| `p079_` | 64 |
| `proposal_058_` | 15 |
| `runtime_facts_` | 8 |
| `shadow_escalation::tests` | 14 |

Integration targets passed: `agent_context_skills` 26 (one explicitly ignored
historical-fixture regeneration helper), `proposal_058_claim_start` 22,
`proposal_058_deadline_resume` 10, `xcode_project_trust_admission` 2. The first
integration attempt exhausted Rust's default test-thread stack; the successful
run used `RUST_MIN_STACK=8388608`, consistent with the repository's large-suite
execution setting. The old P053 idempotency fixture was corrected to explicitly
validate and materialize before reading the output; discovery settlement was
already pure before this patch.

Production `cargo check -p engine`, formatting, whitespace, shell syntax and
the repository `guardrails` gate passed. The canonical `proposal-053`,
`proposal-058` and `agent-context-skills` gates now include the new relevant
regression filters; full gate runs are not claimed. No Swift source or schema
migration changed.

## Safe live verification order

1. Integrate the verified patch into `main`, push to `origin`, and deploy the
   corresponding daemon build. The operator explicitly authorized these actions
   after the implementation work. Preserve the existing dirty original checkout.
2. After deployment, verify `/ready` reports the intended commit, storage readback
   is fresh, and no active invocation would race the controlled test.
3. Ops alone performs one explicit `RetryStage` for the current failed P070 C
   stage, using fresh frozen primary authority. Do not manually rewrite old queue
   payloads, mission snapshots, ledger tiers or run/stage status.
4. Read back the new execution's discovery decisions: canonical `run_state` must
   be `control_plane_generated`; plan/backlog remain subject to their own contract
   and provenance requirements. No repair prompt may request a run-state mutation.
5. If a legitimate provider failure triggers escalation, verify the new execution
   uses its authorized frozen tier/profile and historical source validation works
   even after ledger advancement. A rejected payload must leave explicit failed/
   blocked state, not running with an exhausted queue.
6. Only a run that reaches `code_writer`/project-trust admission can verify
   `da399247` live. If trust remains absent, expect explicit no-launch trust
   admission failure. Any trust grant requires its separate operator decision.

These are local regression proofs. Provider behavior, the corrected live retry
and the project-trust boundary remain unverified by this implementation task.
