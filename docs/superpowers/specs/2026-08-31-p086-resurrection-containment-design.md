# P086 Resurrection Containment

Date: 2026-08-31
Status: Deferred roadmap source; not implementation-approved
Source checkpoint: `acf85de1`
Inherited finding: P1-03
Reserved focused gate: `p086-resurrection-containment`

## Purpose

Own provider-session resurrection and output-only recovery without coupling it
to fresh Codex model/effort selection.

## Owned scope

- Exact provider attach capability manifests and executable/adapter digests.
- The currently supported Claude
  `session/new.params.resumeSessionId` protocol only.
- A private raw-session-ID resolver bound to the exact Claude protocol tag,
  request ID, serializer digest, owner, target generation, and nonce.
- Zero raw-ID access for generic `session/resume`, `session/load`, untagged
  `session/new`, logging, artifacts, and northbound readback.
- Immutable source and effective MCP inventories.
- Symlink-safe source-root authority and daemon-private materialized roots.
- Attach correlation, readiness settlement, replay phases, and bounded cleanup.
- Output-only recovery correlated by request fingerprint, prompt marker, stage,
  agent, generation, and provider response.

## Required proof when scheduled

- Supported Claude success and every unsupported-provider zero-launch case.
- Wrong session, request, serializer, protocol tag, root, MCP inventory, and
  terminal response negatives.
- Crash/restart fixtures for launch, attach, prompt, terminal settlement, and
  receipt persistence.
- Pre/post source proof that detects edits to already-dirty files.

## Activation rule

Provider egress and diagnostics containment is a separate dependency. This
document cannot broaden the active model-label slice or enable Codex
resurrection implicitly.
