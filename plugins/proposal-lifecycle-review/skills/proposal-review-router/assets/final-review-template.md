# Proposal Review

Proposal: `<path>`  
Mode: `<auto | proposal-readiness | research | specialist-mode>`  
Verdict: `<Ready | Ready with conditions | Not ready | Evidence gap>`  
Confidence: `<High | Medium | Low>`

Selected reviewers:

| Reviewer ID | Why selected | Evidence IDs |
|---|---|---|
|  |  |  |

Rejected close alternatives:

| Reviewer ID | Why not selected | Evidence IDs |
|---|---|---|
|  |  |  |

Fingerprint:

| Tag type | Tags | Evidence IDs |
|---|---|---|
| Stack |  |  |
| Surface |  |  |
| Risk |  |  |

Baseline status: `<Fresh | Partially refreshed | Missing | Stale>`  
Evidence pack: `<path or not written>`  
Research pack: `<path or not used>`

Leading metric: `<required when product_reviewer selected; otherwise N/A>`  
Guardrail metric: `<required when product_reviewer selected; otherwise N/A>`  
Decision checkpoint: `<required when product_reviewer selected or rollout is central; otherwise N/A>`

## Findings

Order findings by severity. Use tight file and line references where possible.

### `<SEV-ID>` - `<title>`

Reviewer: `<reviewer_id>`  
Severity: `<P0 | P1 | P2 | P3>`  
Evidence IDs: `<DOC-01, MAP-02, ...>`  
File / lines: `<path:line>`  
Confidence: `<0.0-1.0>`

Problem:

`<one paragraph explaining the issue and why it matters>`

Required fix:

`<specific change to proposal or implementation plan>`

Acceptance criteria:

- `<observable proof or contract>`
- `<test/proof gate only if appropriate for proposal mode>`

## Evidence gaps

| Gap | Why it matters | Next artifact |
|---|---|---|
|  |  |  |

## Proposal completeness

| Dimension | Status | Notes |
|---|---|---|
| Problem and target user |  |  |
| Scope and non-goals |  |  |
| Current-system fit |  |  |
| Data / state model |  |  |
| API / compatibility |  |  |
| Runtime / failure handling |  |  |
| Security / privacy / auth |  |  |
| Migration / rollout / rollback |  |  |
| Observability / diagnostics |  |  |
| Test / proof gate |  |  |
| Product metrics |  |  |

## Notes on validation

Proposal-readiness review does not require build/run, simulator runs, service startup, benchmarks, load tests, or fuzzing. List those only as optional later validation when the proposal itself requires them.
