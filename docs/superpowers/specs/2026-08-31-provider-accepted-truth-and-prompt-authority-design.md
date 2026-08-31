# Provider Accepted Truth and Prompt Authority

Date: 2026-08-31
Status: Deferred roadmap source; not implementation-approved
Source checkpoint: `acf85de1`
Reserved focused gate: `provider-accepted-truth`

## Purpose

Preserve the durable provider-acceptance scope removed from the active Codex
variant slice. This document is an inventory only. It must be refined and
reviewed independently before implementation.

## Owned scope

- Stable task-occurrence identity across loops, retries, fallback, and copied
  work.
- Generation-scoped requested versus provider-accepted model/effort truth.
- Immutable owner bindings and prompt-turn identity.
- Crash-consistent prompt permits and sent/delivery-unknown/terminal outcomes.
- Exact-pair live-session reuse without copying another owner's receipt.
- Configuration invalidation from ordered provider option updates.
- Provider-neutral readiness evidence that cannot fabricate accepted model or
  effort.
- Controlled fallback-route uniqueness and settlement. The inherited P2-01
  choice between compile-time rejection and runtime ambiguity must be resolved
  before implementation.
- Operator-safe GraphQL/MCP/report projections of accepted truth.

## Dependencies

- P083 execution ownership truth.
- Typed P070/P081 boundary authorization.
- The active exact-variant slice must already prevent prompts under an
  unverified fresh pair.
- Provider configuration migration/reconciliation must define durable storage
  and restart ownership.

## Required proof when scheduled

- Crash/replay at every reservation, receipt, permit, prompt-write, and
  settlement boundary.
- Same-agent and same-session concurrent owner negatives.
- Zero-send proof for stale, malformed, mismatched, or invalidated evidence.
- Byte-identical readback across authorized GraphQL, MCP, report, and Swift
  lanes with no raw provider-session identifier disclosure.

## Activation rule

Scheduling requires a new bounded proposal below 2,000 lines with one
implementation cut, exact schemas, a focused gate, and explicit exclusions.
Nothing in this inventory may be implemented under the active model-label
proposal.
