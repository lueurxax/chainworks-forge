# Artifact Content Rendering Proof

Current implementation and proof status for the unified read-only Markdown and JSON rendering slice implemented from Proposal 027.

## Status

| Field | Value |
|---|---|
| Slice | Artifact Content Rendering |
| Source contract | [../reference/artifact-content-rendering.md](../reference/artifact-content-rendering.md) |
| Current implementation status | Implemented |
| Current readiness | Ready |
| Primary evidence owner | test-gate proposal-owned suite + focused renderer tests |
| Last consolidated audit | `proposal-027` gate evidence on `2026-04-07` |

## What is considered proven

The accepted proof story on the implemented head supports:

- one shared renderer entry point (`ArtifactContentRenderer`) for markdown and json artifacts,
- AppKit/TextKit-backed read-only markdown document rendering with structured markdown block handling,
- disclosure-based JSON tree rendering for structured payloads,
- markdown→json payload rescue when `.markdown` or `.report` declarations contain valid top-level JSON,
- preserved artifact format truth (`Artifact.format`) with non-persistent, presentation-only rendering,
- local-only markdown image policy with safe fallback for unsupported, non-local, or unsafe image sources,
- non-editable markdown document behavior (`NSTextView.isEditable = false`).

## Accepted evidence sources

- Implementation and implementation-audit trail:
  - `Chainworks Forge/Views/ArtifactContentRenderer.swift`
  - `Chainworks ForgeTests/Proposal027Tests.swift`
  - `Chainworks Forge/Views/ArtifactContentRenderer.swift` parser/renderers and policy helpers
- Runtime gate evidence:
  - `./scripts/test-gate.sh proposal-027`

The final audit reported `19/19` focused renderer tests green on the approved remote host and marked the slice ready.

## Canonical proving path

The canonical local/remote proving path is:

```bash
./scripts/test-gate.sh proposal-027
./scripts/test-gate.sh full
```

Canonical remote form (when full-host workflow is required):

```bash
ssh test@SMacBook.local "cd '/Users/test/chainworks-remote' && ./scripts/test-gate.sh proposal-027"
```

## Consolidation note

This is a consolidation artifact.
The original implementation-trail documents were consolidated into this proof package during documentation cleanup:

- `docs/proposals/027-unified-read-only-json-and-markdown-rendering.md`

For stable status and behavior, this proof document is now the canonical source.

## Remaining risk

- `full` still exercises broader suite breadth and remains an external sign-off check depending on current host policy.
- renderer behavior is implemented and passing, but this does not replace regular re-run expectations on future heads.
