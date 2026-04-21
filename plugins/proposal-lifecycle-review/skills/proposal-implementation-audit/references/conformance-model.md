# Proposal Conformance Model

## Requirement Extraction

A `REQ-*` item is valid only when the proposal explicitly commits to something. Valid sources include:

- acceptance criteria
- locked decisions
- rollout or migration plan
- API/schema compatibility commitments
- test/evidence requirements
- UI/UX state commitments
- reliability/performance/security constraints
- explicit non-goals and exclusions, when implementation violates them

Do not create `REQ-*` items from reviewer preference.

## Status Definitions

### Implemented
Direct implementation evidence proves the commitment is satisfied. Inference alone is not enough.

### Partially Implemented
Some meaningful implementation exists, but a committed part is missing, incomplete, untested when tests were required, or only works for one in-scope platform/service/path.

### Missing
The committed behavior, constraint, migration, compatibility path, evidence, or exclusion is absent or contradicted by implementation.

### Not Verifiable
The implementation may satisfy the commitment, but the audit could not prove it with available evidence.

### Out of Scope
The proposal explicitly excluded the item or the current implementation target intentionally does not include that slice.

## Roll-Up

- All in-scope `REQ-*` implemented and passing same-tree full regression or canonical full/proposal gate evidence exists -> `Overall Conformance = Implemented`
- Any in-scope `REQ-*` missing -> `Overall Conformance = Not Implemented`
- Any in-scope `REQ-*` partial or not verifiable and none missing -> `Overall Conformance = Partial`
- Most critical requirements not verifiable -> `Overall Conformance = Not Verifiable`

Do not report a successful audit verdict from stale or different-tree regression evidence. `Overall Conformance = Implemented`, `Overall Implementation Readiness = Ready`, and `Overall Implementation Readiness = Ready with Risks` all require passing same-tree full regression or canonical full/proposal gate evidence recorded in the verification log.

## Readiness Roll-Up

- `Ready`: no blocking `REQ-*` gaps, critical/major reviewer findings, or missing critical evidence; same-tree full regression/canonical gate passed
- `Ready with Risks`: no unresolved critical blocker, but bounded major/minor risks remain; same-tree full regression/canonical gate passed
- `Not Ready`: missing critical evidence, blocked primary flow, failed/missing gate evidence, or unresolved major/critical findings make ship/handoff unsafe
- `Blocked`: the audit cannot establish readiness because the proposal, implementation target, or required evidence is inaccessible or contradictory

## Prior Review Follow-Through

Prior proposal-review findings are not requirements by themselves. They become implementation-audit obligations only when:

- the proposal explicitly required them, or
- the proposal review marked them as required before implementation and the user expects that review to gate readiness.

Even then, verify them against current implementation evidence.
