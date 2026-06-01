# Proposal 096: P058 Release Evidence and macOS Runtime Proof

## Metadata

- **Proposal ID:** P096
- **Title:** P058 Release Evidence and macOS Runtime Proof
- **Status:** Draft for implementation
- **Date:** 2026-05-29
- **Owner:** macOS operator shell / control-plane release evidence
- **Depends on:** [escalation policy contract](../reference/escalation-policies.md) and retained escalation proof-gate evidence in [test-gates.md](../reference/test-gates.md#proposal-058p058)
- **Proposed future gate aliases:** `proposal-096`, `p096`

## Problem

The configurable escalation-policy contract is implemented and documented in [escalation-policies.md](../reference/escalation-policies.md). Its local proof path is the retained escalation proof gate: durable escalation policy state, scheduler-owned tier advancement, GraphQL/MCP/report readback, governed SwiftUI read surfaces, shared adapter ownership, MenuBarExtra overflow routing, and focused macOS component contracts.

Several remaining proof items require a live approved macOS UI host, long-running runtime observation, or operator drill artifacts. Keeping those items inside implementation closeout makes audits conflate completed implementation with release evidence that cannot be produced by same-tree unit and control-plane gates alone.

## Goal

Produce the release evidence needed to default-enable or broadly rely on configurable escalation chains in operator workflows without changing the implemented runtime model.

This proposal owns only evidence and runtime proof:

- remote visual/runtime proof for the escalation read surface;
- Full Keyboard Access traversal proof;
- contrast and reduced-motion proof for escalation components;
- scene restoration and multi-window runtime proof on the approved macOS UI host;
- long-run metric-threshold trend capture;
- live operational drills for force-detach/restart and escalation attention behavior.

## Non-Goals

- Do not add new escalation tier kinds, triggers, pause reasons, or scheduler behavior.
- Do not add SwiftUI mutations or make SwiftUI an escalation lifecycle authority.
- Do not weaken escalation GraphQL/MCP/report redaction or caller-class boundaries.
- Do not include release, publish, git-push, upload, or distribution stages in escalation behavior.
- Do not change durable side-effect settlement or retry safety.

## Scope

### Remote macOS UI Proof

Run on the approved remote host (`test@SMacBook.local`) using the repository remote-UI protocol.

Evidence must include:

- escalation read-surface screen state for at least one paused escalation run;
- MenuBarExtra aggregate count, five-row cap, overflow route, and empty state;
- run detail / inspector status capsule, lineage, pause card, trace section, command row, and drift review sheet;
- screenshot or structured UI assertion evidence stored under `docs/evidence/p058/`.

### Accessibility And Motion Proof

Evidence must include:

- Full Keyboard Access traversal order for banner stack, status capsule/menu, lineage rows, pause-card actions, trace disclosure, and drift controls;
- VoiceOver/accessibility labels containing state, tier, trigger, and raw IDs where required;
- contrast proof for Light, Dark, and Increase Contrast;
- reduced-motion proof showing crossfade/no movement for tier transitions.

### Scene And Multi-Window Runtime Proof

Evidence must include:

- restored run-detail scene shows a loading escalation state until the shared adapter publisher emits;
- two windows/inspectors for the same `run_id` receive the same shared adapter update;
- Runs Home visible-run refresh does not evict a retained inspector adapter.

The last invariant is covered by the retained focused escalation tests; this proposal adds live runtime proof.

### Operational Drill Proof

Evidence must include:

- operator restart / daemon restart drill for a running escalation execution;
- provider force-detach replay evidence;
- late-frame-after-detach metric/event evidence;
- escalation attention request/cancel evidence when the app is backgrounded and then activated;
- long-run metric-threshold trending for escalation retry, pause, force-detach, drift, and attention counters.

## Evidence Storage

Evidence follows the existing storage baseline:

- screenshots, logs, and raw UI/run artifacts are file-backed under `docs/evidence/p058/`;
- compact receipts and manifests summarize file paths, hashes, host, command, timestamp, and result;
- no row-per-stream-chunk persistence is introduced.

## Acceptance Criteria

- `./scripts/test-gate.sh proposal-058` remains green on `main`.
- Remote macOS proof runs on `test@SMacBook.local` and writes an escalation evidence receipt.
- Evidence receipt includes command, host, git SHA, timestamps, result, and artifact hashes.
- Full Keyboard Access traversal covers all escalation interactive elements listed in this proposal.
- Contrast and reduced-motion proof artifacts are present and referenced from the receipt.
- Scene restoration and multi-window shared-adapter proof artifacts are present and referenced from the receipt.
- Operational drill receipt includes force-detach/restart, late-frame, attention request/cancel, and long-run metric trend artifacts.
- [escalation-policies.md](../reference/escalation-policies.md) links to the final P096 evidence receipt.

## Relationship To The Escalation Contract

[escalation-policies.md](../reference/escalation-policies.md) is the implementation contract for configurable escalation chains. P096 is the release-proof envelope around that implementation. A missing P096 artifact should block broad release/default-enable decisions, but should not reclassify the escalation implementation code as incomplete when the retained escalation gate and focused component tests pass.
