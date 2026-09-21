# Proposal Review

Proposal: [P039](../039-blocked-run-fork-and-canonical-carry-forward.md)

- Date / revision: 2026-09-20 / `p039-r2`.
- Reviewed proposal md5: `84f0dfc8a23baf1af665c48990208e44`.
- Mode: `proposal-readiness`.
- Verdict: **Ready with conditions for staged implementation planning**.
- Confidence: **High** in the resolved document findings.
- Evidence completeness: **Partial** overall because the cap omitted one
  independent specialist; sufficient for the isolated I1 planning decision.
- Code baseline HEAD: `8c40f71a03cfebff413fbad584131e0bcbe9859b`.

The April draft is now aligned with current Rust ownership, frozen permissions,
MCP-only mutation authority and headless Xcode policy. Five independent reviewers
completed first passes and targeted resolution rechecks; none of their findings
remains open at specification level. This is not a full independent second pass
over every final byte, implementation acceptance or authorization to transfer P095.

Proceed with the I1 provider-free task plan. I2/I3 consume that experiment's
results, and I4 requires offline implementation proof, a separate execution-truth
review, compatible deployment and confirmed live scope. These are staged delivery
conditions, not claims that any production gate has already passed.

## Routing

Selected reviewers:

| Reviewer ID | Why selected | Evidence IDs |
| --- | --- | --- |
| rust_arch_reviewer | Component ownership, inputs, source/successor authority | E02/E03/E07/E10/E13 |
| rust_reliability_reviewer | Queue, crash, races, uncertain effects and replay | E03/E07/E11/E13 |
| rust_security_reviewer | Exact capability/scope, private files, unsafe paths, permission transfer | E05/E06/E07/E12 |
| api_contract_reviewer | Strict shapes, input origin, compiler/approval boundaries and compatibility | E01/E04/E09/E11/E12 |
| observability_rollout_reviewer | Additive migration, downgrade/disable, evidence and rollout readback | E10/E14/E15 |

Rejected close alternatives:

| Reviewer ID | Why not selected | Evidence IDs |
| --- | --- | --- |
| apple_arch_reviewer | No new Swift execution owner; additive reader only | E01/E02 |
| macos_ui_reviewer / apple_ux_reviewer | No new visual/navigation/control surface | E01 |
| rust_performance_reviewer | Bounded worker covered by reliability; no throughput improvement claim | E19 |
| product_reviewer | Not requested; engineering correctness proposal, not product experiment | E16 |

Mandatory reviewer not dispatched:

| Reviewer ID | Why it fired | Reason | Surface left unreviewed |
| --- | --- | --- | --- |
| chainworks_execution_truth_reviewer | Durable run, approval and imported-input authority | cap overflow, 5 selected | Separate domain-specialist pass; overlapping boundaries were examined by architecture, reliability and API reviewers |

No substitute sixth reviewer or user-owned task was silently created. Independent
reviewers received their own rubric and facts-only inputs, not peer findings.

Fingerprint:

| Tag type | Tags | Evidence IDs |
| --- | --- | --- |
| Stack | rust-backend, shared-api | E01/E02/E03/E04/E11/E12 |
| Surface | architecture, state-management, background-work, api-contract, auth, security-boundary, concurrency, persistence, migration, rollout, rollback | E02/E03/E05/E07/E10/E12/E14/E15 |
| Risk | backward-compatibility, privacy-sensitive, data-loss, idempotency, availability-sensitive, operability-sensitive | E04/E06/E07/E10/E11/E13/E14/E15 |

Baseline status: **Partially refreshed**, limited to affected current source owners.
Evidence pack: [evidence-pack.md](evidence-pack.md), facts E01..E24.
Research pack: [research-pack.md](research-pack.md), no external claims required.
Config layer: `.codex/review-router.yaml`, native custom reviewer plugin; no
`.claude` fallback. Unrelated dirty/staged work remains outside this revision.

Leading metric: N/A, product reviewer not selected.
Guardrail metric: N/A, product reviewer not selected.
Decision checkpoint: parent H1 and verification slices I1 -> I2/I3 -> I4;
future `proposal-039|p039` gate, followed by separately authorized P095 acceptance.

## Findings

**Open: none from the five selected reviewers.** The ledger below retains the
original severities and fixes; closed means specified and rechecked, not built.
The repeated imported-input finding is deduplicated into P2-01.

| ID | Reviewer | Original issue / evidence | Resolution | Required implementation proof |
| --- | --- | --- | --- | --- |
| P1-01 | rust_arch_reviewer | Old granted approval at a logical stage can satisfy a new entry, E20 | Durable exact-stage evidence binding, supersession, resolution CAS and first-dispatch consumption | CF-16 |
| P1-02 | rust_arch_reviewer | Read-only review defaults to repository/main, E21 | Frozen successor-root strategy for review, refinement and dynamic reviewers | CF-18 |
| P1-03 | rust_reliability_reviewer | Abort can release source while preparation worker continues, E19 | Aborting phase, queue invalidation, generation revoke, retained fence until settled | CF-20 |
| P1-04 | observability_rollout_reviewer | Main-file backup alone cannot promise committed WAL data, E17 | SQLite-consistent backup and isolated restore prerequisite before migration | CF-21 |
| P2-01 | rust_arch_reviewer, rust_reliability_reviewer | Imported-only successor has no provider artifact to select, E12 | Tagged artifact/carried_input selection and bounded immediate-source ancestry | CF-07 |
| P2-02 | rust_reliability_reviewer | Inline preparation can block unrelated coordination, E19 | One active/three queued tracked preparation lane; overload before reservation | CF-26 |
| P2-03 | rust_security_reviewer | Preview inherits a write probe, E22 | Pure observational preview; probe only in authorized journalled preparation | CF-27 |
| P2-04 | rust_security_reviewer | Empty scoped principal could be mistaken for global, E06/E23 | Require run_scope.is_none(), including replay | CF-12 |
| P2-05 | api_contract_reviewer | Result/denial/unknown-outcome envelope unspecified, E11/E24 | Dedicated strict P039 result DTO, transport mapping and replay policy | CF-28 |
| P2-06 | api_contract_reviewer | Preview inventory pagination requires a mutation-created operation | Read-only plan-digest-bound paging and typed stale response | CF-27 |
| P2-07 | api_contract_reviewer | Singular continuation readback loses one direction in a chain | Dedicated incoming/outgoing aggregate; aborted attempt history separate | CF-29 |
| P2-08 | observability_rollout_reviewer | Admission flag changes shared enforcement semantics, E18 | Separate P039 admission DTO plus four-lane design fixture | CF-22/23 |

Fixes are in the [runtime contract](../039/runtime-and-wire-contract.md),
parent section 6, and the [acceptance matrix](../039/verification-and-rollout.md).
No cross-discipline disagreement remained at synthesis.

### Recheck Provenance

| Reviewer | Independent task ID | Targeted recheck outcome |
| --- | --- | --- |
| rust_arch_reviewer | `01a0bed5-29a2-77b2-ac05-d94b1c3535a9` | All three findings closed at specification level |
| rust_reliability_reviewer | `01a0bed5-2b57-7b61-b376-3b51d94482d3` | All three findings closed at specification level |
| rust_security_reviewer | `01a0bed5-2cd4-7d53-880f-d9c357b0f455` | Both findings closed at specification level |
| api_contract_reviewer | `01a0bed5-2e81-7de2-a922-d7133d3b5e6b` | All three findings closed at specification level |
| observability_rollout_reviewer | `01a0bed5-3087-77a0-9ad0-4b68e9f27a45` | Both findings closed at specification level |

After rechecks, author synthesis updated readiness metadata and made the omitted
specialist pass an explicit I4 condition. Final hashes below bind that integrated
revision, not an assertion that every specialist reread unrelated corrections.

## Evidence Gaps

| Gap | Why it matters | Next artifact |
| --- | --- | --- |
| Independent execution-truth specialist omitted by cap | Domain-specific review coverage is incomplete | Dedicated pass before production admission |
| I1 hypothesis not executed | Written preservation and fresh-authority semantics are not observed behavior | I1 disposable-repo/DB experiment receipts |
| P039 production paths/gates do not exist | Source fence, recovery, migration and API contracts need implementation proof | I2/I3 implementation, CF-01..29 mapping and registered gate |
| P095 live continuation not performed | Historical preservation does not prove current eligibility or headless execution | I4 canonical preflight and staged live readbacks |

The last three are declared implementation/proof work, not missing decisions
that require speculative expansion of this specification.

## Proposal Completeness

| Dimension | Status | Notes |
| --- | --- | --- |
| Problem and target user | Ready | P095-shaped blocked-run recovery, operator-owned |
| Scope and non-goals | Ready | Same idea/repository, closed implementation restart profile |
| Current-system fit | Ready | Current code owners; missing capabilities are implementation scope |
| Data / state model | Ready | Durable operation, fences, inputs, exact-stage approval binding |
| API / contract compatibility | Ready | Strict result/page schemas, directional lineage, historical readers |
| Runtime / concurrency semantics | Ready | Bounded preparation lane, guards, activation linearization |
| Failure handling | Ready | Unknown outcomes hold; generation-safe abort; no automatic effect repeat |
| Security / privacy / auth | Ready | Explicit global scope, exact capability, no-follow owned destinations |
| Migration / rollout / rollback | Ready | Consistent backup prerequisite, compatible rollback, admission separate from fences |
| Observability / diagnostics | Ready | Four readback lanes, typed holds, bounded metrics |
| Test / proof gate | Ready | CF-01..29; future provider-free gate and separate live acceptance |
| Product metrics / decision checkpoint | Ready | Product metrics N/A; I1 hypothesis, I2/I3 proof, I4 live checkpoint |

## Final Fingerprints

| Document | Lines | MD5 | SHA-256 |
| --- | --- | --- | --- |
| Parent P039 | 250 | `84f0dfc8a23baf1af665c48990208e44` | `d9cdf03db26f97a9c53aaa6f26adac369af98b13b250cfa9b688302bea6d8982` |
| Runtime/wire child | 470 | `3de2ddfb6d20fcc285f8fd921c462ff2` | `5191061cba9aa82bbc7519cac4104a6bbcdc498d9c433e93c8070533612eb5f8` |
| Verification/rollout child | 167 | `c0465fe7626eb0cc007418e79c445637` | `5c72c77d5632f03a9cd8ed990574ae4e19e2329a55bf2392f6e83df67e073806` |

## Notes On Validation

Document checks: rollout-contract lint; local Markdown links and line budgets;
four JSON documents parsed; normalized parity of both four-lane fixture groups;
CF-01..29 unique/contiguous; scoped whitespace check; final fingerprints.
Fixtures are explicitly design-only HOLD shapes, not observed runtime results.

No builds, tests, daemon/service restart, provider dispatch, Xcode/Apple call,
production DB write, P095 transfer, commit or push was performed for this
specification update. Proposal-readiness does not require those actions.
Implementation-readiness would add actual build/runtime evidence later.
