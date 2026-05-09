# P081 Proposal Blockers Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Rewrite Proposal 081 and make minimal adjacent reference clarifications so the current Rust proposal-review blockers are resolved without changing implementation code or altering the current UI action boundary.

**Architecture:** Treat `docs/proposals/081-boundary-first-api-auth-contract-matrix.md` as the primary contract artifact and keep `docs/reference/*` as implemented truth. Resolve the blockers by making `P081` concrete about `CallerClass`, `schema_version: 3`, `BoundaryPolicy` ownership, and `audit_log`, then rerun proposal review from the blocked run after the document changes are merged with current `main`.

**Tech Stack:** Markdown docs, git, ripgrep, jq, Chainworks control-plane MCP run operations

---

### Task 1: Reconfirm current blocker evidence and baseline boundaries

**Files:**
- Modify: `docs/proposals/081-boundary-first-api-auth-contract-matrix.md`
- Reference: `docs/reference/ui-action-boundary.md`
- Reference: `docs/reference/query-projections-and-client-consumption-contract.md`
- Reference: `docs/reference/current-system-baseline.md`

- [ ] **Step 1: Capture the current Rust blocker text**

Run:

```bash
jq '{blocking_issues,issues,summary,verdict}' '.chainworks/runs/4dd7c886-e7b4-4f6d-8afe-f76d63bf268d/reviews/proposal/rust-architect.json'
```

Expected: JSON showing `BLK-RUST-001`, `BLK-RUST-002`, and `BLK-RUST-003`.

- [ ] **Step 2: Capture the current UI boundary truth**

Run:

```bash
sed -n '1,220p' 'docs/reference/ui-action-boundary.md'
```

Expected: text confirming the governed UI is GraphQL read/subscription plus approval-only mutations.

- [ ] **Step 3: Capture the current GraphQL read-plane truth**

Run:

```bash
sed -n '1,260p' 'docs/reference/query-projections-and-client-consumption-contract.md'
```

Expected: text confirming GraphQL is the thin UI read plane and MCP remains command/control for non-approval operations.

- [ ] **Step 4: Capture the current proposal body before rewrite**

Run:

```bash
sed -n '1,260p' 'docs/proposals/081-boundary-first-api-auth-contract-matrix.md'
```

Expected: a short draft proposal that does not yet contain a concrete `schema_version: 3`, `BoundaryPolicy` ownership section, or a fully concrete `audit_log` contract.

- [ ] **Step 5: Commit the evidence-only checkpoint**

```bash
git status --short
git commit --allow-empty -m "chore: checkpoint P081 blocker evidence review"
```

Expected: empty checkpoint commit created so later doc edits can be reviewed separately.

### Task 2: Add current-baseline alignment and target-state framing to Proposal 081

**Files:**
- Modify: `docs/proposals/081-boundary-first-api-auth-contract-matrix.md`

- [ ] **Step 1: Write a failing content check for missing baseline-alignment framing**

Run:

```bash
rg -n "Current baseline alignment|implemented truth|future target state" 'docs/proposals/081-boundary-first-api-auth-contract-matrix.md'
```

Expected: no matches, proving the proposal does not yet distinguish current baseline from target contract.

- [ ] **Step 2: Insert the baseline-alignment framing near the top of the proposal**

Add this section after the proposal metadata and before the detailed contract sections:

```markdown
## Current Baseline Alignment

This proposal defines a target-state contract. It does not redefine the current implemented baseline recorded in `docs/reference/*`.

At proposal time:

- the governed macOS UI remains a GraphQL read/subscription surface with `approveApproval` and `rejectApproval` as the only governed mutations;
- MCP remains the operator and automation control plane for non-approval actions;
- existing principal table behavior in production is the current `schema_version: 1` / `schema_version: 2` behavior already implemented under prior proposals;
- the new `CallerClass`, `BoundaryPolicy`, `schema_version: 3`, and `audit_log` requirements below describe the contract to land, not behavior already claimed as implemented at HEAD.

Every section in this proposal must preserve that distinction: current baseline truth stays in reference docs, while this proposal defines the migration-safe contract for the next boundary layer.
```

- [ ] **Step 3: Add a coexistence note for mainline movement**

Add this paragraph near the rollout or migration section:

```markdown
This proposal is written against post-merge repository truth rather than branch-start wording. If adjacent durability work lands in `main` before this proposal is implemented, Proposal 081 adopts the merged shared naming and primitives so long as the boundary contract defined here remains intact.
```

- [ ] **Step 4: Verify the new framing is present**

Run:

```bash
rg -n "Current Baseline Alignment|post-merge repository truth|governed macOS UI remains a GraphQL read/subscription surface" 'docs/proposals/081-boundary-first-api-auth-contract-matrix.md'
```

Expected: matches for all inserted phrases.

- [ ] **Step 5: Commit**

```bash
git add 'docs/proposals/081-boundary-first-api-auth-contract-matrix.md'
git commit -m "docs: frame P081 against current baseline truth"
```

### Task 3: Resolve BLK-RUST-001 with explicit identity model and BoundaryPolicy ownership

**Files:**
- Modify: `docs/proposals/081-boundary-first-api-auth-contract-matrix.md`
- Reference: `control-plane/crates/auth/src/lib.rs`
- Reference: `control-plane/crates/domain/src/commands.rs`

- [ ] **Step 1: Write a failing content check for missing ownership language**

Run:

```bash
rg -n "CallerClass|PrincipalClass|BoundaryPolicy ownership|daemon injects|immutable in-memory policy" 'docs/proposals/081-boundary-first-api-auth-contract-matrix.md'
```

Expected: either no matches or only shallow mentions without an ownership and injection contract.

- [ ] **Step 2: Add the identity-model contract**

Insert or replace the relevant proposal section with:

```markdown
## Identity Model

`PrincipalClass` remains the canonical persisted identity class for principal records and command provenance compatibility.

Proposal 081 introduces `CallerClass` as a derived runtime classification used for boundary decisions. `CallerClass` does not replace `PrincipalClass`.

`CallerClass` is derived from:

- principal identity and principal class;
- transport (`graphql_http`, `graphql_ws`, `mcp_http`, `mcp_stdio`, or explicit debug transport);
- surface policy attached to the resolved principal entry;
- request-scoped context required by the selected matrix row.

Compatibility readback keeps existing `callerPrincipalClass` behavior and adds nullable `callerClass` only where the new contract requires it.
```

- [ ] **Step 3: Add the BoundaryPolicy ownership and injection contract**

Insert or replace the relevant proposal section with:

```markdown
## BoundaryPolicy Ownership

`BoundaryPolicy` is a shared control-plane service, not a GraphQL-local, MCP-local, or engine-local helper.

The contract requires:

- one shared Rust home for matrix fixture loading, validation, evaluator types, and typed decision outputs;
- daemon startup loads one immutable validated policy instance;
- the daemon injects that shared instance into GraphQL authorization paths, MCP capability and call authorization paths, approval actionability computation, and accepted-command audit/provenance paths;
- request paths do not read `docs/reference/*` or fixture files directly.

This avoids duplicated surface-specific policy logic and makes approval availability, mutation authorization, and MCP denials derive from the same decision source.
```

- [ ] **Step 4: Verify BLK-RUST-001 is concretely addressed in text**

Run:

```bash
rg -n "does not replace `PrincipalClass`|one shared Rust home|immutable validated policy instance|approval actionability computation" 'docs/proposals/081-boundary-first-api-auth-contract-matrix.md'
```

Expected: matches for all phrases.

- [ ] **Step 5: Commit**

```bash
git add 'docs/proposals/081-boundary-first-api-auth-contract-matrix.md'
git commit -m "docs: define P081 identity and policy ownership contract"
```

### Task 4: Resolve BLK-RUST-002 with schema_version 3 compatibility rules

**Files:**
- Modify: `docs/proposals/081-boundary-first-api-auth-contract-matrix.md`
- Reference: `control-plane/crates/auth/src/lib.rs`

- [ ] **Step 1: Write a failing content check for schema_version 3**

Run:

```bash
rg -n "schema_version: 3|v3|upgrade from v2|P072" 'docs/proposals/081-boundary-first-api-auth-contract-matrix.md'
```

Expected: missing or insufficient matches, proving the current proposal does not cleanly separate new and existing schema semantics.

- [ ] **Step 2: Add a dedicated principal-table compatibility section**

Insert or replace the relevant proposal section with:

```markdown
## Principal Table Compatibility

Proposal 081 does not reinterpret the existing `schema_version: 2` principal-table format.

- `schema_version: 1` remains legacy compatibility behavior.
- `schema_version: 2` remains the current `P072` surface-policy format already implemented at HEAD.
- `schema_version: 3` is the first format that may encode Proposal 081 boundary-aware caller derivation and matrix-linked transport policy.

`schema_version: 3` must load fail closed when:

- required policy fields are missing;
- transport-specific policy rows are ambiguous;
- caller derivation would produce more than one eligible `CallerClass`;
- unknown enum values or unknown top-level policy fields are present.

The migration path from `v2` to `v3` must preserve existing `v2` behavior until a principal file explicitly upgrades.
```

- [ ] **Step 3: Add an explicit non-goal against dual-meaning v2 semantics**

Add this line in the non-goals or compatibility section:

```markdown
Proposal 081 must not assign a second incompatible meaning to `schema_version: 2`.
```

- [ ] **Step 4: Verify BLK-RUST-002 is concretely addressed**

Run:

```bash
rg -n "schema_version: 3|does not reinterpret the existing `schema_version: 2`|must not assign a second incompatible meaning" 'docs/proposals/081-boundary-first-api-auth-contract-matrix.md'
```

Expected: matches for all phrases.

- [ ] **Step 5: Commit**

```bash
git add 'docs/proposals/081-boundary-first-api-auth-contract-matrix.md'
git commit -m "docs: define P081 principal table v3 compatibility"
```

### Task 5: Resolve BLK-RUST-003 with a concrete audit_log contract

**Files:**
- Modify: `docs/proposals/081-boundary-first-api-auth-contract-matrix.md`
- Reference: `docs/reference/output-contracts-failure-evidence-and-recovery.md`
- Reference: `docs/reference/query-projections-and-client-consumption-contract.md`

- [ ] **Step 1: Write a failing content check for concrete audit_log semantics**

Run:

```bash
rg -n "audit_log|migration boundary|fail closed|retention|readback|transaction" 'docs/proposals/081-boundary-first-api-auth-contract-matrix.md'
```

Expected: either missing or too vague to satisfy the Rust review.

- [ ] **Step 2: Add a concrete audit_log contract section**

Insert or replace the relevant proposal section with:

```markdown
## audit_log Contract

Proposal 081 requires a dedicated durable `audit_log` contract for boundary-sensitive allows and denials.

The proposal-level contract must define:

- migration boundary for introducing `audit_log`;
- required row identity and request identity fields;
- required caller provenance fields, including compatibility treatment for `callerPrincipalClass` and future nullable `callerClass`;
- required policy linkage fields such as matrix row identifier and denial reason code where applicable;
- transaction coupling rules for allowed mutating calls when audit is mandatory;
- deny-path rules for cases that must emit exactly one durable audit row before returning;
- fail-closed behavior when audit-required storage is unavailable;
- retention and readback expectations sufficient for operator diagnostics and post-incident review.

Proposal 081 does not redefine the full shared durability framework. Where adjacent durability work already owns common write, integrity, or evidence primitives, `audit_log` adopts those shared invariants and adds only the boundary-specific requirements above.
```

- [ ] **Step 3: Add one explicit break-glass denial rule**

Add this paragraph in the same section:

```markdown
Disabled or rejected developer break-glass attempts must not degrade to best-effort logging. If the contract requires a durable audit row for that denial path and audit-required storage is unavailable, the request fails closed without claiming successful policy evaluation.
```

- [ ] **Step 4: Verify BLK-RUST-003 is concretely addressed**

Run:

```bash
rg -n "dedicated durable `audit_log` contract|fail-closed behavior when audit-required storage is unavailable|does not redefine the full shared durability framework|developer break-glass attempts" 'docs/proposals/081-boundary-first-api-auth-contract-matrix.md'
```

Expected: matches for all phrases.

- [ ] **Step 5: Commit**

```bash
git add 'docs/proposals/081-boundary-first-api-auth-contract-matrix.md'
git commit -m "docs: define concrete P081 audit log contract"
```

### Task 6: Make minimal reference clarifications without claiming implementation

**Files:**
- Modify: `docs/reference/ui-action-boundary.md`
- Modify: `docs/reference/query-projections-and-client-consumption-contract.md`
- Modify: `docs/reference/current-system-baseline.md`

- [ ] **Step 1: Write failing checks for current wording ambiguity**

Run:

```bash
rg -n "future|target state|proposal 081|boundary matrix" 'docs/reference/ui-action-boundary.md' 'docs/reference/query-projections-and-client-consumption-contract.md' 'docs/reference/current-system-baseline.md'
```

Expected: either no mentions or wording that does not help a reviewer distinguish current implemented truth from future `P081` contract.

- [ ] **Step 2: Clarify `ui-action-boundary.md` without changing behavior**

Add a short note near the top:

```markdown
This reference records the current implemented UI action boundary at HEAD. Future boundary-contract proposals may tighten how the server derives authorization and actionability, but they do not change the baseline rule here unless and until that behavior is implemented and promoted into reference truth.
```

- [ ] **Step 3: Clarify `query-projections-and-client-consumption-contract.md` without changing behavior**

Add a short note near the Thin UI Boundary section:

```markdown
Any future caller-class or boundary-matrix contract must preserve this implemented thin-client rule unless a later approved and implemented write-boundary change explicitly replaces it.
```

- [ ] **Step 4: Clarify `current-system-baseline.md` only if needed**

If the baseline doc would otherwise read as conflicting with the revised `P081`, add this short note near the UI/boundary summary:

```markdown
Proposal-level boundary-contract work may define future caller-class, matrix, or audit requirements, but this baseline document records only currently implemented behavior.
```

- [ ] **Step 5: Verify the reference edits are clarifying rather than promotional**

Run:

```bash
rg -n "current implemented UI action boundary at HEAD|must preserve this implemented thin-client rule|records only currently implemented behavior" 'docs/reference/ui-action-boundary.md' 'docs/reference/query-projections-and-client-consumption-contract.md' 'docs/reference/current-system-baseline.md'
```

Expected: matches for inserted clarifying phrases and no wording that says `P081` is already implemented.

- [ ] **Step 6: Commit**

```bash
git add 'docs/reference/ui-action-boundary.md' 'docs/reference/query-projections-and-client-consumption-contract.md' 'docs/reference/current-system-baseline.md'
git commit -m "docs: clarify current boundary baseline around P081"
```

### Task 7: Sync with mainline movement before final validation

**Files:**
- Modify: `docs/proposals/081-boundary-first-api-auth-contract-matrix.md`
- Modify: `docs/reference/ui-action-boundary.md`
- Modify: `docs/reference/query-projections-and-client-consumption-contract.md`
- Modify: `docs/reference/current-system-baseline.md`

- [ ] **Step 1: Refresh from current `main` before final review rerun**

Run:

```bash
git fetch origin
git log --oneline --decorate -n 12 origin/main -- 'docs/reference' 'control-plane/crates/auth' 'control-plane/crates/engine' 'control-plane/crates/mcp-server'
```

Expected: visible recent changes, including any `P075`-related terminology or durability naming now present on `main`.

- [ ] **Step 2: Reconcile wording if `main` changed shared durability naming**

If needed, update proposal/reference wording so it uses the merged shared naming rather than branch-start terminology. Apply only wording changes like this:

```markdown
Where adjacent durability work defines the shared primitive name, Proposal 081 adopts the merged repository naming while preserving the boundary contract defined here.
```

- [ ] **Step 3: Verify the proposal still reads against post-merge truth**

Run:

```bash
rg -n "post-merge repository truth|shared durability work|schema_version: 3|BoundaryPolicy|audit_log Contract" 'docs/proposals/081-boundary-first-api-auth-contract-matrix.md'
```

Expected: matches showing the document still reads coherently after mainline changes.

- [ ] **Step 4: Commit**

```bash
git add 'docs/proposals/081-boundary-first-api-auth-contract-matrix.md' 'docs/reference/ui-action-boundary.md' 'docs/reference/query-projections-and-client-consumption-contract.md' 'docs/reference/current-system-baseline.md'
git commit -m "docs: align P081 wording with current mainline truth"
```

### Task 8: Re-run proposal review for the blocked P081 run and capture outcome

**Files:**
- Modify: `docs/proposals/081-boundary-first-api-auth-contract-matrix.md`
- Reference: `.chainworks/runs/4dd7c886-e7b4-4f6d-8afe-f76d63bf268d/state/run-state.json`

- [ ] **Step 1: Verify the run is still blocked in proposal review before retry**

Run:

```bash
sed -n '1,120p' '.chainworks/runs/4dd7c886-e7b4-4f6d-8afe-f76d63bf268d/state/run-state.json'
```

Expected: blocked proposal-review state with `BLK-RUST-*` context still present.

- [ ] **Step 2: Trigger a proposal-review retry for the blocked run**

Run:

```bash
echo 'Use MCP tool mcp__chainworks_control_plane__.stages_retry with run_id=4dd7c886-e7b4-4f6d-8afe-f76d63bf268d and stage_id=state_4_proposal_reviewed after the doc changes are committed.'
```

Expected: reminder that the retry is an MCP operation against `state_4_proposal_reviewed`, not a local DB edit.

- [ ] **Step 3: Re-read the run summary after the retry settles**

Run:

```bash
sed -n '1,160p' '.chainworks/runs/4dd7c886-e7b4-4f6d-8afe-f76d63bf268d/state/run-state.json'
```

Expected: either blocker count drops and the run advances, or a narrower/new blocker remains with updated wording.

- [ ] **Step 4: Capture the new proposal-review evidence**

Run:

```bash
jq '{blocking_issues,issues,summary,verdict}' '.chainworks/runs/4dd7c886-e7b4-4f6d-8afe-f76d63bf268d/reviews/proposal/rust-architect.json'
```

Expected: evidence that the old blockers were resolved, reduced, or replaced by a smaller follow-up set.

- [ ] **Step 5: Commit the final doc state after successful review rerun**

```bash
git add 'docs/proposals/081-boundary-first-api-auth-contract-matrix.md' 'docs/reference/ui-action-boundary.md' 'docs/reference/query-projections-and-client-consumption-contract.md' 'docs/reference/current-system-baseline.md'
git commit -m "docs: make P081 review-ready"
```

## Self-Review

- Spec coverage:
  - proposal rewrite: Tasks 2-5
  - reference clarifications: Task 6
  - mid-flight `P075`/`main` movement: Task 7
  - blocked-run proposal review rerun: Task 8
- Placeholder scan:
  - no `TODO`/`TBD` placeholders remain
  - each command step includes an exact command and expected outcome
- Type consistency:
  - `PrincipalClass`, `CallerClass`, `BoundaryPolicy`, `schema_version: 3`, and `audit_log` are used consistently across tasks

