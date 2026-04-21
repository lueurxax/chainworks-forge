---
name: proposal-lifecycle-review
description: Thin dispatcher for the Proposal Lifecycle Review plugin. Use when a user asks for proposal lifecycle review but has not clearly chosen between pre-implementation proposal review and post/during-implementation audit. Delegates to proposal-review-router for proposal readiness/research/routing before implementation, or proposal-implementation-audit for proposal-vs-implementation conformance/readiness audits. Do not use as an independent review engine.
---

# Proposal Lifecycle Review

Use this skill only as a dispatcher. Do not duplicate the core workflows here.

## Dispatch

Choose exactly one primary skill unless the user explicitly asks for a multi-phase lifecycle run.

Use `proposal-review-router` when the task asks to:

- review a proposal before implementation
- assess proposal readiness
- select specialist reviewers
- build proposal evidence/research packs
- evaluate proposal completeness, risks, routing, or local evidence
- run `auto`, `proposal-readiness`, `research`, or specialist proposal-review modes

Use `proposal-implementation-audit` when the task asks to:

- audit current implementation, branch, diff, PR, or commit against a proposal
- decide whether implementation satisfies proposal requirements
- reuse prior proposal-review reviewer selection
- produce `REQ-*` conformance statuses
- write a versioned implementation audit report
- run `implementation-audit`, `implementation-readiness`, `conformance-only`, `diff-only`, `reroute`, or audit specialist modes

If the user asks for both phases:

1. Run `proposal-review-router` first.
2. Preserve its reviewer selection artifact and review outputs.
3. Run `proposal-implementation-audit` only after there is an implementation target.
4. Instruct the audit to reuse the proposal-review reviewer selection when valid and add evidence-backed delta reviewers only when implementation evidence introduces a new surface or risk.

## Guardrails

- Do not flatten proposal review and implementation audit into one report unless the user explicitly asks for a combined lifecycle summary.
- Do not weaken either skill's read-only boundary or output contract.
- Do not invent reviewer ids. Use the shared reviewer-id contract in `../../shared/reviewer-id-contract.md`.
- If phase intent is ambiguous, infer from available inputs:
  - proposal path only: proposal review
  - proposal path plus implementation branch/diff/current worktree audit wording: implementation audit
  - request mentions `REQ-*`, conformance, readiness against implementation, or audit report: implementation audit
  - request mentions reviewer routing, proposal readiness, research, evidence pack, or final review: proposal review
