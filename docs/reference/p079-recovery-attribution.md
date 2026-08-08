# P079 Recovery Attribution

P079 transcript/provider-envelope recovery can only accept output when the
daemon can attribute the recovered payload to the current failed agent
execution through transport-owned evidence.

## Implemented Rule

The implemented recovery order is:

1. normal declared output settlement,
2. bounded transcript/provider-envelope recovery,
3. same-session repair when the provider has an enforced permission boundary,
4. final blocked/skipped evidence.

Provider fallback dispatch remains a future lane unless a frozen policy and
safe dispatch boundary are implemented.

Recovery is implemented by `p079_attempt_transcript_recovery` in
`control-plane/crates/engine/src/executor.rs`.

## Accepted Source

P079 accepts only output envelopes extracted as
`DiscoveredArtifactSourceKind::ProviderEnvelope` by the ACP transport. Raw
`CHAINWORKS_OUTPUT` JSON found in transcript text is not sufficient proof. It
is rejected with attribution evidence instead of being treated as active output
truth.

The provider body is not trusted for attribution. Provider-emitted
`agent_execution_id`, `session_generation_id`, or `recovery_source` fields are
ignored for authority decisions. Current execution identity comes from the
daemon/session ledger and transport-owned capture path.

## Bounds

Recovery is fail-closed and bounded:

- default byte cap: 262144 bytes,
- minimum configured cap: 1024 bytes,
- maximum configured cap: 1048576 bytes,
- configuration knob: `CHAINWORKS_P079_TRANSCRIPT_RECOVERY_MAX_BYTES`,
- maximum JSON depth recorded in evidence: 32,
- maximum chunks examined recorded in evidence: 64.

If the transcript exceeds the byte cap, recovery returns `unavailable` with
subtype `oversized_payload`. No partial payload is accepted.

## Activation

Transcript recovery is always enabled for transport-attributed provider
envelopes. There is no runtime enablement flag for the core recovery path. The
only remaining runtime knob is the bounded byte cap
`CHAINWORKS_P079_TRANSCRIPT_RECOVERY_MAX_BYTES`.

## Result Mapping

- No transcript: `unavailable`
- No declared outputs: `not_needed`
- Oversized transcript: `unavailable`, subtype `oversized_payload`
- Raw/unattributed output body: `unavailable`, subtype
  `unattributable_envelope` or `attribution_not_verified`
- Transport-attributed provider envelope: `accepted`

Accepted recovery still runs through declared-output validation and canonical
path binding before active truth is updated.

## Gate Coverage

`./scripts/test-gate.sh proposal-079` runs focused engine tests that prove:

- no transcript is unavailable,
- oversized transcripts fail closed,
- raw transcript output without transport attribution fails closed,
- disabled recovery flag fails closed,
- transport-attributed provider envelopes are accepted.
