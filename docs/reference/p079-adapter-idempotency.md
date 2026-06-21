# P079 Adapter Idempotency

P079 recovery work must not double-dispatch repair or fallback attempts after a
crash, lost acknowledgement, or stale lease. This document captures the
implemented idempotency contract and the current boundary for deferred fallback
dispatch.

## Durable Keys

The repair lane uses deterministic identifiers persisted before provider
dispatch:

- `repair_attempt_id`
- `lease_key`
- `idempotency_token`

For repair leases, `lease_key` is derived from the run, stage execution, parent
agent execution, P079 schema/version vocabulary, and repair lane. The lease is
stored in `output_contract_repair_leases` before any repair prompt is sent.

Fallback schema support exists through:

- `output_contract_repair_fallback_parent_links`,
- `fallback_agent_execution_id`,
- `parent_failed_agent_execution_id`,
- `fallback_packet_hash`,
- `fallback_principal_id`,
- `fallback_principal_capability_hash`,
- fallback lease kind.

Controlled provider fallback dispatch from frozen YAML policy is not wired in
the current implementation. Existing fallback tables and DTO fields are
readback/storage substrate, not proof that fallback dispatch has occurred.

## State Machine

Repair/fallback leases use:

- `reserved`
- `prompt_sent`
- `settled`

The durable commit invariant is: `reserved -> prompt_sent` commits before
provider dispatch. Restart recovery must not issue a second repair prompt after
`prompt_sent`; it should settle or reclaim the existing lease.

Expired leases are reclaimed by the repository sweep:

- `reserved` expires to `unavailable` without consuming repair/fallback budget,
- `prompt_sent` expires to `failed_transport` and consumes the corresponding
  budget,
- lease settlement and repair-event settlement commit in the same transaction.

## Adapter Boundary

An adapter can participate in production same-session repair only when it can
enforce a server-side filesystem/tool/network permission boundary for the
repair turn. Prompt instructions and voluntary provider permission messages are
not an enforceable sandbox.

Current status:

- `fixture`: enabled for deterministic tests,
- `codex`, `claude`, `gemini`, `junie`, `auggie`: fail-closed for production
  same-session repair until a real permission boundary is available.

The environment variable `CHAINWORKS_P079_ACCEPT_ADVISORY_REPAIR_POSTURE` is
not accepted as production authority. Tests assert that advisory posture cannot
enable production repair.

## Metrics

The DB metrics inventory declares and records bounded P079 counters for repair
attempts, terminal outcomes, transcript recovery, fallback lease/budget
outcomes, invalid repair rejection, provider-mode mismatch, plan evidence, and
recovery bound exceedance. Provider fallback dispatch metrics are declared for
the future fallback lane; they are not evidence of a live fallback dispatcher by
themselves.

## Gate Coverage

`./scripts/test-gate.sh proposal-079` covers:

- repair-event and lease persistence,
- single-flight lease constraints,
- lease reclamation and budget rules,
- fallback parent-link storage/readback,
- metric declaration/recordability,
- deterministic fixture same-session repair,
- fail-closed production provider posture.
