# Idea Lifecycle

Stable reference for the implemented idea-lifecycle baseline that was previously carried by Proposal 010's archive slice.

## Purpose

The default idea list should represent live operator attention.
It must not become a mixed backlog of active work, abandoned drafts, and already-finished ideas.

This document defines the current archive/restore contract for ideas.

## Scope

This reference covers:

- active vs archived idea visibility,
- archive eligibility,
- restore semantics,
- cross-surface truth for archived ideas,
- operator entry points in the `Ideas` flow.

It does not define stopping active work before archive.
That boundary belongs to [../proposals/011-run-control-working-directory-and-provider-binding-truth.md](../proposals/011-run-control-working-directory-and-provider-binding-truth.md).

## Core rule

Archive is a visibility/lifecycle action, not a delete action.

Archiving an idea:

- removes it from the default active ideas list,
- preserves all runs, artifacts, reports, receipts, and approvals,
- does not mutate historical run state,
- remains reversible via `Restore`.

## Eligibility

An idea may be archived only when one of the following is true:

1. it is still `draft` and has no active run,
2. its latest run is terminal,
3. it has no run history at all.

An idea may not be archived while:

- a run is active,
- a run is still waiting on an active approval gate,
- the idea is the current focus of live in-flight work.

## Restore

Restore returns an archived idea to the active ideas list.

Restore does not:

- recreate runs,
- reopen terminal runs,
- rewrite reports,
- modify artifact provenance.

It only changes the idea's lifecycle visibility.

## Shell ownership

Archive belongs to the `Ideas` flow.

Canonical operator path:

1. `Ideas`
2. archive or restore action
3. archived-ideas lane or list
4. idea detail

Non-goals:

- no standalone archive tab,
- no archive action as a peer to run-level recovery actions,
- no archive-driven mutation of run history.

## Cross-surface truth

Archived ideas stay hidden from the default active ideas list, but their historical work remains visible in run-centric surfaces.

Rules:

- `RunsHomeView`, reports, artifact inspection, and run detail may still show completed work for archived ideas,
- those surfaces must truthfully indicate that the parent idea is archived,
- restore is initiated from the archive lane in `Ideas`, not from every run-centric surface,
- archived ideas do not silently reappear in the active list.

## Operator expectations

The operator should always be able to answer:

- is this idea active or archived,
- is archive currently allowed,
- why is archive blocked,
- can this archived item be restored safely.

Archive must never be presented as a substitute for stop/cancel.
