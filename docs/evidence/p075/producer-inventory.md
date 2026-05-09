# P075 High-Volume Evidence Producer Inventory

**Status:** gate-backed inventory captured.

This document records the current P075 high-volume evidence producer truth so
future audits do not have to infer it from scattered call sites. The invariant is
one compact SQLite metadata row per logical evidence object; bytes live under the
canonical `evidence/runs/{run_id}/stages/{stage_id}/agents/{agent_id}/{kind}/...`
spool layout.

| Producer class | Current runtime producer | Spool kind | SQLite truth | Gate evidence |
|---|---|---|---|---|
| Failed-stage diagnostic packet | `engine::evidence::build_and_persist_failed_stage_evidence` | `runtime_event` | `stages.evidence_packet_json` stores a v2 pointer; full packet is a spooled file with one `evidence_spool_refs` row | `cargo test -p engine failed_stage_evidence_packet_tests -- --nocapture` |
| ACP transcript capture | `BackgroundExecutor::build_transcript_artifact_if_present` behind `CHAINWORKS_PERSIST_ACP_TRANSCRIPTS=1` | `transcript` | artifact row stores the user-facing artifact; `evidence_spool_refs` stores compact path/checksum/size metadata through `insert_idempotent_via_dbwriter` | `./scripts/test-gate.sh proposal-075` raw-evidence scan plus engine compile coverage |
| Tool trace files | no primary runtime producer currently writes tool trace bytes into SQLite | `tool_trace` reserved and validated by spool primitives | `evidence_spool_refs` accepts the kind; orphan sweep can recover canonical files | `cargo test -p db evidence_spool_refs -- --nocapture` and `cargo test -p db evidence_spool -- --nocapture` |
| stdout snippets | no primary runtime producer currently writes stdout bytes into SQLite | `stdout` reserved and validated by spool primitives | `evidence_spool_refs` accepts the kind; orphan sweep can recover canonical files | same DB spool/ref coverage |
| stderr snippets | no primary runtime producer currently writes stderr bytes into SQLite | `stderr` reserved and validated by spool primitives | `evidence_spool_refs` accepts the kind; orphan sweep can recover canonical files | same DB spool/ref coverage |
| Model delta stream | no primary runtime producer currently writes model delta bytes into SQLite | `model_delta` reserved and validated by spool primitives | `evidence_spool_refs` accepts the kind; orphan sweep can recover canonical files | same DB spool/ref coverage |
| Delivery/readback receipts | stored as bounded artifacts or structured summaries, not row-per-chunk runtime streams | `receipt` / `delivery_readback` reserved and validated by spool primitives | compact metadata path is available when a producer needs large receipt bytes | same DB spool/ref coverage |

The proposal-075 gate also scans `control-plane/crates/engine/src` for legacy
raw failed-stage evidence writes such as `update_evidence_packet_json(...,
&encoded)` and fails if that pattern returns.

Residual scope: this is an implementation inventory, not a claim that every
future evidence class is already producing bytes. New high-volume producer
classes must either write through `db::evidence_spool::write_spool_file` plus
`evidence_spool_refs::*_via_dbwriter`, or extend this inventory and the gate in
the same change.
