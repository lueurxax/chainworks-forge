# Proposal 069: Bounded Discovery Diagnostics Operator UI

| Field | Value |
|---|---|
| Date | 2026-04-23 |
| Status | Draft |
| Author | Andrey Khasanov |
| Depends on | [query-projections-and-client-consumption-contract.md](../reference/query-projections-and-client-consumption-contract.md), [artifact-discovery-and-settlement-optimization.md](../reference/artifact-discovery-and-settlement-optimization.md) |
| Blocked by | Affected surfaces must consume the canonical thin UI read contract; no dependency on historical proposal files |
| Scope | Implement macOS operator UI surfaces for bounded discovery diagnostics on top of the thin UI ownership model. |
| Goal | Let operators inspect missing/stale/rejected output diagnostics, startup timing, discovery mode, caps, and source-change evidence from the macOS UI without making control-plane readiness depend on legacy Swift UI work. |

**Gate naming note:** implementation owns the future canonical gate alias `proposal-069|p069`. It must be added to `scripts/test-gate.sh` and `docs/reference/test-gates.md` when work starts.

---

## 1. Context

The Rust control plane implements the bounded ACP artifact discovery and settlement model. It removes broad pre-`initialize` filesystem discovery, adds typed expected-output specs, persists discovery diagnostics, and exposes durable readback for operators and support tooling.

The bounded discovery design also described a macOS operator UI. That UI work is valuable, but it does not block control-plane sign-off. Governed workflow screens now use the canonical thin UI boundary: GraphQL-only reads over server projections. Adding discovery-specific Swift surfaces must extend that boundary, not recreate the legacy UI ownership problem.

P069 owns rendering bounded discovery truth in the macOS operator app after the thin UI boundary. Durable discovery truth and server readback remain owned by the control-plane reference contract.

---

## 2. Non-Negotiable Boundary

### 2.1 P069 builds on the thin UI boundary

P069 implementation must start from [query-projections-and-client-consumption-contract.md](../reference/query-projections-and-client-consumption-contract.md). For any affected surface, the implementation must either:

- add discovery diagnostics to an existing governed thin UI read surface, or
- first create a server-owned GraphQL read slice for that surface without using legacy Swift workflow truth.

Until then, missing macOS UI surfaces are not a control-plane readiness blocker.

### 2.2 UI reads through GraphQL only

The macOS UI must consume discovery diagnostics through GraphQL read projections or subscriptions owned by the server/control-plane layer.

The UI must not:

- call MCP tools or MCP resources;
- read SQLite directly;
- scan run artifact directories as workflow truth;
- infer missing/stale/rejected output state from local filesystem checks;
- define or invoke GraphQL mutations for discovery diagnostics.

MCP readback may exist for agents, CLI/operator-debug tooling, automation, and audit preparation. It is not the UI transport.

### 2.3 Control-plane discovery truth remains authoritative

P069 may request additional GraphQL projection fields, but it must not create a second discovery decision model in Swift. The bounded discovery and implemented output-settlement truth in [output contracts, failure evidence, and recovery](../reference/output-contracts-failure-evidence-and-recovery.md) remains authoritative.

---

## 3. In Scope

- Add read-only discovery diagnostics to governed thin UI Run Detail, Stage Detail, report, artifact, and failed-stage evidence surfaces.
- Render missing, stale, rejected, unauthorized-root, oversized, capped, metadata-timeout, symlink-escape, and contract-invalid output states from server-provided fields.
- Show discovery mode: exact path, bounded meta-root, provider envelope, `CHAINWORKS_OUTPUT`, legacy fallback, resume warning, missing metadata, and override-active.
- Show startup timing attribution that separates Forge local overhead from provider latency when those metrics are present.
- Show source-change evidence from the changed-files manifest with friendly states for timeout, not-a-git-repository, and command-failed cases.
- Provide Copy Path and Open Location affordances based on server-provided expected paths and availability metadata.
- Preserve operator readability at narrow sidebar widths and large Dynamic Type sizes.
- Add accessibility labels for status icons and diagnostic controls.
- Add design tokens or approved semantic color mappings for Forge overhead and provider latency.
- Add UI tests/static guards that prove governed UI code uses GraphQL reads only for discovery diagnostics and does not call MCP, SQLite, local artifact scanning, or local workflow mutation paths.

---

## 4. Out of Scope

- Changing discovery, settlement, cap, or runtime-fact semantics.
- Making control-plane implementation sign-off wait for macOS UI work.
- Reintroducing legacy Swift workflow truth.
- Adding UI write paths for retry, approve, reject, cancel, declare-as-output, or recovery.
- Adding GraphQL mutations for discovery diagnostics.
- Using MCP from the macOS UI.
- Replacing MCP agent/debug readback with GraphQL.
- Implementing broad discovery fallback or contract-specific output-size maxima.

---

## 5. Required Server Read Model

P069 needs a server-owned read projection for each visible field. If a field is missing, the implementation must add it to the control-plane read model or explicitly hide/defer the UI element. Swift must not reconstruct it locally.

Minimum fields:

| UI need | Server-owned field family |
|---|---|
| Output status | `output_name`, `output_role`, `display_label`, `status`, `reason`, `settlement_kind` |
| Expected path | `target_path`, `canonical_path`, `authorized_roots`, `path_availability` |
| Provenance | `source_generation_owner`, `provenance`, `provider_envelope`, `chainworks_output`, `meta_root`, `legacy_fallback` |
| Caps | `size_bytes`, `max_bytes`, `aggregate_cap_bytes`, `cap_status`, `truncation_flags` |
| Pre-prompt metadata | `metadata_status`, `metadata_captured_at`, `metadata_timeout`, `baseline_digest_status` |
| Startup timing | `forge_overhead_ms`, `provider_initialize_latency_ms`, `provider_ready_latency_ms` |
| Source changes | `changed_files_manifest_status`, `changed_files_count`, `changed_files_error_kind`, `changed_files_artifact_id` |
| Freshness | `projection_updated_at`, `freshness_state`, `server_debug_detail` gated as operator-only |

---

## 6. UI Surfaces

### Run Detail

- Show discovery/startup summary near the run status area.
- Show warning promotion only for states that affect operator action or interpretation: missing required output, unauthorized root, cap rejection, metadata timeout, legacy fallback, override active, or source-change failure.
- Keep normal bounded discovery mode in diagnostics/runtime metadata, not as a noisy primary warning.

### Stage Detail and Failed-Stage Evidence

- Show the output-level diagnostic table/list for the selected execution.
- Default compact view shows the first five issue rows and a Show Issues Only toggle when many expected outputs exist.
- Each row exposes full path, reason, status, and authorized-root detail through tooltip or disclosure.

### Artifact Inspector

- Show provenance chips: Exact Path, Provider Envelope, `CHAINWORKS_OUTPUT`, Meta Root, Control Plane, Legacy Fallback.
- Show missing/stale/rejected/unauthorized evidence only from server fields.

### Reports

- Render source-change manifest state with user-readable messages.
- Preserve raw JSON fallback only as an explicit technical detail, not as the primary operator message.

---

## 7. UX and Accessibility Requirements

- Missing output icon: `square.dotted`.
- Stale output icon: `clock.badge.exclamationmark.fill`.
- Terminal rejected output uses existing error treatment. Warning-grade rejection uses `exclamationmark.triangle.fill`.
- Status icons include complete accessibility labels such as `Stale artifact: Modified before current run`.
- Unauthorized-root details include the task-specific authorized roots and rejected canonical path when available.
- Expected absolute paths use middle truncation in compact rows.
- Diagnostic reasons wrap cleanly and remain available in full through tooltip/disclosure.
- At large Dynamic Type sizes, rows may grow vertically; critical status and reason text must not be permanently obscured.
- Labels and output ids remain readable at 280-340 pt sidebar widths.
- The startup timing visualization must pass light/dark contrast checks before shipping.

---

## 8. Acceptance Criteria

- Control-plane readiness checks do not require macOS UI surfaces; they verify only the control-plane/API/readback contract.
- P069 implementation starts from the canonical thin UI boundary for the affected surfaces.
- Governed UI code reads discovery diagnostics through GraphQL only.
- Static guards fail if governed UI code imports/calls MCP, reads SQLite, scans local artifacts for truth, or defines GraphQL mutations for discovery diagnostics.
- Operators can distinguish missing, stale, rejected, unauthorized, oversized, capped, metadata-timeout, and source-change failure states without reading raw JSON.
- UI behavior at narrow widths and large Dynamic Type sizes is validated.
- The canonical `proposal-069|p069` gate passes.

---

## 9. Relationship to Control-Plane Discovery Truth

The control plane must continue to expose enough durable discovery truth for P069:

- persisted `DiscoveryDiagnosticsV1`;
- typed `OutputDiscoveryDecision` records;
- `AgentOutputSettlement` integration;
- minimal durable readback for production-exposed runs;
- GraphQL-readable projection fields or a documented server read-model gap.

Control-plane discovery sign-off does not require P069 to be implemented. If P069 discovers a missing server field, that is either a P069 server-read-model task or a control-plane follow-up only when the missing field contradicts the durable discovery truth contract.
