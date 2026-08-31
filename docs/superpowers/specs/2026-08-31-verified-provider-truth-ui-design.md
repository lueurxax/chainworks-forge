# Verified Provider Truth UI

Date: 2026-08-31
Status: Deferred roadmap source; not implementation-approved
Source checkpoint: `acf85de1`
Reserved focused gate: `verified-provider-truth-ui`

## Purpose

Own UI states that become meaningful only after durable accepted provider truth
and prompt authority exist. The active slice displays planned identity only.

## Owned scope

- Planned, configuring, configured, prompt-sent, delivery-unknown,
  failed-before-prompt, invalidated, legacy-unverified, and unavailable states.
- Shared formatting of requested and accepted model/effort without promoting
  one into the other.
- Overview, Stages, Timeline, Inspector, Help, copy, and accessibility parity.
- Stable occurrence/event identity, duplicate-label disambiguation, selection,
  paging controls, per-window focus, deep links, and stale callback rejection.
- Bounded raw diagnostics and safe spoken mappings for unknown identifiers.
- Compact and full-width layouts across supported window and Dynamic Type
  sizes.

## Dependencies

- Durable provider accepted truth and prompt authority.
- Bounded P031 runtime readback.
- Stable task-occurrence and event identity from P083-compatible ownership.

## Required proof when scheduled

- Exhaustive legal state matrix and mutation-negative tuple tests.
- Byte-identical formatting across all surfaces.
- Hosted focus/accessibility and compact/full layout matrices.
- No accepted/configured claim when only planned, stale, legacy, or unavailable
  evidence exists.

## Activation rule

This inventory must be split by readback and interaction surface if a single
proposal approaches 2,000 lines. It does not block approval of planned labels.
