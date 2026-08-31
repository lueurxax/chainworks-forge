# Frozen Run Replacement and Input Repair

Date: 2026-08-31
Status: Deferred roadmap source; not implementation-approved
Source checkpoint: `acf85de1`
Inherited findings: P1-08, P2-02
Reserved focused gate: `frozen-run-input-repair`

## Purpose

Own the operator flow for inspecting invalid frozen input and creating a safe
replacement run without changing the original snapshot.

## Owned scope

- An exact no-oracle result shared by unauthorized, missing, and ineligible
  frozen-run lookups.
- Operator-only read and mutation boundaries with live principal reload.
- Encoded HTTP body and decoded workflow limits with typed oversized responses.
- Atomic ARCH-002-compliant settlement/supersession of the original before a
  replacement for the same Idea becomes active.
- Immutable linkage, idempotency, crash replay, and preservation of original
  snapshots, artifacts, attempts, approvals, and ledger history.
- A same-window workflow repair workspace with immutable source, editable
  draft, validation, submission, failure, and submitted states.
- Accurate commands: `Open workflow repair` and `Create replacement run`.
- Deterministic focus, cancellation, accessibility, and stale-response rules.

## Required proof when scheduled

- Agent/Observer no-oracle byte equality for missing, unauthorized, and
  ineligible inputs.
- Request body exact-limit and plus-one fixtures before expensive decoding.
- Duplicate command, concurrent replacement, crash, stale validation, and
  original-active negatives.
- Hosted macOS state/focus/accessibility tests plus mutation-free original
  snapshot verification.

## Activation rule

This is a standalone run-lifecycle proposal. It cannot be introduced as error
recovery for the active model-label slice.
