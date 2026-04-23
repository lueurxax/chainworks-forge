# P053 Phase 1 Security Checklist

| Field | Value |
|---|---|
| Proposal | P053 bounded ACP artifact discovery and startup latency |
| Proposal revision | `p053-r12-ui-deferred-to-p069-2026-04-23` |
| Reviewed tree | `codex/p053-manual-merge-1833dd16` |
| Mode | Gate-only/internal closeout evidence |
| Reviewed at | 2026-04-23 |
| Security reviewer | Codex implementation agent, source-level review |
| Architecture reviewer | P053 implementation audit R1 plus follow-up source patches |
| Production exposure | Not approved by this checklist |

## Scope

This checklist covers the P053 Phase 1 security-sensitive changes:

- expected-output authorized root checks;
- symlink escape rejection;
- generated-state denylist for fallback/support traversal;
- byte and aggregate caps for exact-path outputs, provider envelopes, and `CHAINWORKS_OUTPUT`;
- raw target-path validation bypass prevention;
- bounded current-run meta-root discovery;
- operation-recorder evidence for filesystem discovery paths.

It does not approve production rollout. Production exposure still requires replacing or amending `cap-validation.json` with representative production sampling and named release/security signoff.

## Checklist

| Check | Status | Evidence |
|---|---|---|
| Fresh ACP startup does not perform repository/worktree/generated-state traversal before `initialize`. | Source-covered | `AcpTransportSession::start` sends `initialize` before P053 metadata capture; `proposal-053` gate exercises startup behavior. |
| Expected output metadata rejects paths outside authorized roots. | Source-covered | `authorized_root_class_for_canonical_path` and `pre_prompt_expected_output_metadata_rejects_unauthorized_root`. |
| Symlink escapes do not produce accepted output truth. | Source-covered | `proposal_053_bounded_meta_root_never_follows_symlinks` and engine symlink rejection fixtures. |
| Oversized exact-path/provider-envelope/`CHAINWORKS_OUTPUT` payloads cannot bypass caps. | Source-covered | Engine and ACP cap fixtures in `proposal-053` gate. |
| Validation consumes accepted discovery decisions rather than rereading raw target paths. | Source-covered | `build_captured_outputs_from_discovery_decisions` and `proposal_053_declared_artifact_persistence_requires_accepted_decision`. |
| Legacy broad discovery is disabled by default and generated-state aware when explicitly enabled. | Source-covered | `LegacyBroadDiscoveryPolicy::Disabled` default and generated-state denylist tests. |
| Current-run meta-root discovery is bounded and supplemental-only. | Source-covered | `discover_bounded_meta_root_artifacts` caps plus `proposal_053_bounded_meta_root_artifact_paths_are_supplemental_only`. |
| Filesystem discovery paths are inspectable through an operation-recorder boundary. | Source-covered | `RecordingDiscoveryOperationRecorder` and `proposal_053_operation_recorder_*` tests. |
| Sensitive local paths are not exposed to non-operator UI through this proposal. | Deferred to P069/P031 | P053 only persists server readback; macOS UI rendering is blocked by P031 and P069. |

## Decision

P053 remains approved only for gate-only/internal control-plane validation on this tree. This checklist is sufficient to remove the R1 blocker that no security artifact existed. It is not a production security signoff.
