# P031 Bounded Runtime Readback

Date: 2026-08-31
Status: Deferred roadmap source; not implementation-approved
Source checkpoint: `acf85de1`
Inherited findings: P1-06, P1-07, P2-03
Reserved focused gate: `p031-bounded-runtime-readback`

## Purpose

Own the complete bounded GraphQL/Swift runtime-readback redesign removed from
the planned model-label slice.

## Owned scope

- One canonical protocol-operation inventory for probes, run detail, topology,
  execution attempts, Timeline snapshots/subscriptions, raw detail, and frozen
  input repair operations.
- Exact HTTP and WebSocket envelopes, normalization order, rejection
  precedence, closed error codes, and Swift state reductions for every
  protocol-owned operation.
- Immutable snapshot cursors and bounded paging for topology occurrences,
  transitions, active executions, execution attempts, prompt turns, and
  Timeline history.
- Aggregate row and byte caps with typed over-limit states; no unbounded nested
  arrays.
- Canonical string/custom-scalar representation for monotonic signed-64-bit
  counters instead of unchecked GraphQL `Int`.
- Legacy client coexistence, cursor expiry, gap handling, resubscription, and
  bounded client memory.
- Authorization parity and no raw provider-session identifier exposure.

## Required proof when scheduled

- Every operation document executes against real middleware, resolver, and
  Swift decoder paths.
- Exact boundary/plus-one row and byte fixtures.
- Mixed-version, malformed variable/document, unauthorized, stale cursor,
  cross-run cursor, gap, expiry, and zero-event cases.
- Nonzero Swift suite proof and SDL/fixture parity.

## Activation rule

This inventory must be decomposed by operation family if its implementation
proposal approaches 2,000 lines. The active model-label slice uses existing
topology/execution readback and adds no GraphQL field, ID, or operation.
