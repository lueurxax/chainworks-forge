# P079 Repair Output Materialization and Recovery

Date: 2026-08-31
Status: Deferred roadmap source; not implementation-approved
Source checkpoint: `acf85de1`
Inherited finding: P1-02
Reserved focused gate: `p079-repair-materialization`

## Purpose

Own P079 repair-output staging and crash-safe publication independently of
Codex model selection and UI labeling.

## Owned scope

- Runtime-owned operation staging and least-privilege provider write roots.
- Immutable repair operations, leases, candidates, artifact sets, members,
  provenance, validation, and no-candidate evidence.
- History publication and canonical activation without inode aliasing.
- Bounded startup reconciliation and idempotent replay.
- Normative chunk-journal DDL for resumable large artifacts.
- Separate source content SHA-256 from chunk-chain/state digests. A chunk-tree
  digest must never be asserted equal to the digest of file bytes.
- At most 1 MiB per committed chunk with resumable hash state, temp-file
  offset, predecessor digest, and exact final content verification.
- Symlink, hard-link, rename, `openat`, truncation, and cross-operation escape
  defenses.

## Required proof when scheduled

- Crash injection around every chunk, fsync, history rename, activation CAS,
  and terminal settlement.
- Exact 10 MiB resume without full rescan, plus boundary and budget fixtures.
- Source mutation, chunk substitution, stale lease, duplicate member, and
  activation-race negatives.
- GraphQL/MCP/report projection parity only after the storage contract passes.

## Activation rule

This inventory is not part of the model-variant gate. A future P079 proposal
must define one bounded publication cut and its own retained gate.
