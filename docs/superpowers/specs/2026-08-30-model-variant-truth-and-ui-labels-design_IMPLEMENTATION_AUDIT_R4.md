# Implementation Audit R4: Codex Planned Variant Matrix and UI Labels

## 0. Metadata

| Field | Value |
|---|---|
| Proposal | `docs/superpowers/specs/2026-08-30-model-variant-truth-and-ui-labels-design.md` |
| Proposal identity | MD5 `6f6191fb7a910acd8580e16df8e528c9`; 654 lines; unchanged from R3 |
| Proposal state | `Active` (`Draft`) |
| Implementation target | Clean detached worktree at merge commit `965bdd3503681d4e14b30836024091925edd8332` |
| Target tree | `4e598d6872354ea5394d88eef5e4bfad39d07c10` |
| R4 delta base | R3 target `98a7766853471b704a99b26869c075f456ce04f3` |
| Scoped fix commit | `69065a229d37b61e19f33bf68e8e7593a97bb0dd` (`Close model variant R3 proof gap`) |
| Main worktree | Dirty with unrelated concurrent work; no dirty-main implementation byte or test result was used as evidence |
| Audit timestamp | `2026-09-01T17:16:05+03:00` |
| Report path | `docs/superpowers/specs/2026-08-30-model-variant-truth-and-ui-labels-design_IMPLEMENTATION_AUDIT_R4.md` |

## 1. Verdict

- Overall Conformance: `Implemented`
- Overall Implementation Readiness: `Ready`
- Reviewer Selection Reuse: `Partially reused with delta`
- Audit Confidence: `High`
- Same-tree canonical gate: `Passed` on exact clean commit `965bdd35`.
- Gate inventory: `17` Swift tests and `37` Rust tests passed; every selected filter was nonzero.
- Open in-scope Critical/Major findings: `None`.

The only R3 Major, `UI-002`, is closed. The production `RunsHomeView` refresh test now proves that the post-refresh segmented control is the same object, remains attached to the same window, and is the actual first responder. The R4 Rust test helpers also resolve fixtures from the current checkout at test runtime, so binaries reused from the shared Cargo target do not retain a temporary reviewer-worktree source path.

## 2. Scope and Review Routing

R3 remains context, not current proof. R4 independently inspected the complete four-file delta `98a77668..965bdd35`, its adjacent gate/loader boundaries, and reran the full proposal gate.

Selected reviewers:

| Reviewer | R4 scope | Result |
|---|---|---|
| `macos_ui_reviewer` | Visible post-refresh control identity, window attachment, selection, focus, status, and geometry | `Pass` |
| `rust_arch_reviewer` | Test-fixture source provenance under a shared Cargo target | `Pass` |
| `rust_security_reviewer` | Test-only path lookup confinement and fail-closed fixture integrity | `Pass` |

Rejected close alternatives:

- `api_contract_reviewer`: no API or resolver delta; the full canonical gate rechecked GraphQL behavior.
- `rust_performance_reviewer`: no production parser, allocation, or hot-path delta.
- `observability_rollout_reviewer`: no migration, flag, telemetry, deployment, or rollback surface changed.
- `product_reviewer`: no product contract or decision rule changed.

## 3. Evidence Pack

### Evidence IDs

- `E-R4-001`: exact target commit `965bdd3503681d4e14b30836024091925edd8332`, tree `4e598d6872354ea5394d88eef5e4bfad39d07c10`, in a clean detached worktree.
- `E-R4-002`: proposal MD5 `6f6191fb7a910acd8580e16df8e528c9`, 654 lines, unchanged from R3.
- `E-R4-003`: immutable delta `98a77668..965bdd35`: 4 files, 48 insertions, 9 deletions.
- `E-R4-004`: `CodexModelVariantTruthTests.swift:671-729` hosts the real `RunsHomeView` and proves post-refresh identity, attachment, first-responder ownership, selection, status update, occurrence identity, and frame stability.
- `E-R4-005`: three Rust test helpers resolve the nearest current-directory ancestor containing both the expected fixture and `control-plane/Cargo.toml`; no `CARGO_MANIFEST_DIR` source path remains in those helpers.
- `E-R4-006`: `scripts/test-gate.sh:12632-12663` enters the exact checkout's `control-plane` directory before every affected Rust test and rejects zero-test filters.
- `E-R4-007`: `./scripts/test-gate.sh codex-planned-variant-slice`, exit 0 on `E-R4-001`.
- `E-R4-008`: Swift result: 17 tests passed in `CodexModelVariantTruthTests`.
- `E-R4-009`: Rust result: 37 selected tests passed; every gate filter selected at least one test.
- `E-R4-010`: `git diff --check 98a77668..965bdd35`, `bash -n scripts/test-gate.sh`, and scoped `rustfmt --check` all exited 0.
- `E-R4-011`: three independent specialist passes, all with no confirmed finding.

### Canonical Gate Breakdown

| Filter | Passed |
|---|---:|
| Swift `CodexModelVariantTruthTests` | 17 |
| Domain policy | 4 |
| Workflow bounded source reader | 6 |
| Workflow admission/compatibility | 6 |
| Snapshot integrity | 3 |
| Engine StartRun preflight | 2 |
| Engine production bridge | 1 |
| Engine persisted quartet | 4 |
| ACP adapter lane classifier | 1 |
| ACP exact/rejection transport | 2 |
| ACP closed wire table | 1 |
| GraphQL tampered quartet | 2 |
| GraphQL hash-valid compile failure | 1 |
| GraphQL provider normalization | 2 |
| GraphQL topology | 2 |
| **Rust total** | **37** |

### Evidence Completeness

- Exact implementation provenance: `Complete`.
- R3 blocker closure: `Complete`.
- Test-fixture/current-checkout provenance: `Complete` for the canonical gate.
- Same-tree canonical gate: `Complete`.
- Backend, GraphQL, ACP, Swift presentation, and fail-closed regression evidence: `Complete` for proposal acceptance.
- Live provider, network, remote UI host, and dedicated product Run: proposal-excluded and not required.

## 4. R3 Finding Closure

### UI-002: Closed

R3 required the visible post-refresh control to be bound to all focus assertions. The exact R4 test does so:

1. `pickerAfter.selectedSegment == 1` preserves selection.
2. `pickerAfter === picker` proves the refreshed view still exposes the original control.
3. `pickerAfter.window === window` proves the control remains attached to the hosted production window.
4. `window.firstResponder === pickerAfter` proves that visible control owns focus.
5. The same test retains the `Running` to `Paused` update, stable occurrence identifier, and unchanged frame.

The canonical Swift suite executed this test successfully as one of 17 passing cases. The former stale-control false-positive path is no longer possible.

## 5. Shared Cargo Cache and Fixture Provenance

The R4 Rust delta replaces compile-time manifest-directory fixture lookup in all three affected test sites:

- `control-plane/crates/domain/tests/codex_model_variant_policy.rs:9-24`
- `control-plane/crates/workflow/src/compiler.rs:2171-2189` inside `#[cfg(test)]`
- `control-plane/crates/workflow/tests/codex_planned_variant.rs:92-107`

Each helper starts from the test process current directory, chooses the nearest ancestor with the expected fixture and `control-plane/Cargo.toml`, and fails explicitly when no such checkout exists. The gate changes directory to `$ROOT_DIR/control-plane` before Cargo execution. Consequently, test source selection follows the exact audited checkout at runtime; the shared `CARGO_TARGET_DIR` contains build products but no longer determines fixture provenance through an embedded temporary worktree path.

This change is test-only. It does not broaden production filesystem authority, runtime source selection, public input handling, or fallback behavior. Existing byte-length and pinned SHA-256 validation still gates the policy bytes before parsing.

## 6. Requirement Audit

| ID | Requirement | R4 status | Evidence |
|---|---|---|---|
| `REQ-001` | Byte-pinned policy and strict parser | `Implemented` | Domain policy and bounded loader filters pass. |
| `REQ-002` | Exact seven production profile pairs | `Implemented` | Admission and production bridge filters pass. |
| `REQ-003` | Single-read admission before writes | `Implemented` | Source-reader and StartRun zero-write filters pass. |
| `REQ-004` | Verified historical and snapshot-less compatibility | `Implemented` | Persisted quartet replay and all-absent fallback filters pass. |
| `REQ-005` | Shared quartet verifier gates engine and GraphQL | `Implemented` | Engine and both GraphQL fail-closed filters pass. |
| `REQ-006` | Every effort class enters one wire lane | `Implemented` | ACP classifier and closed-table filters pass. |
| `REQ-007` | Best-effort effort rejection still sends prompt | `Implemented` | Both ACP rejection cases pass. |
| `REQ-008` | Resolver-local Codex normalization only | `Implemented` | GraphQL normalization inventory passes. |
| `REQ-009` | Overview current-stage filtering | `Implemented` | GraphQL topology and Swift presentation tests pass. |
| `REQ-010` | Shared fail-closed Swift loader/formatter | `Implemented` | Same-tree Swift suite passes. |
| `REQ-011` | Non-Codex copy/order compatibility | `Implemented` | Hosted non-Codex regressions pass. |
| `REQ-012` | Geometry, Dynamic Type, help, focus, and selection | `Implemented` | Hosted UI tests pass, including conclusive visible-control focus proof. |
| `REQ-013` | Topology/order/selection and lifecycle unchanged | `Implemented` | No production lifecycle delta; selected regressions pass. |
| `REQ-014` | No accepted/actual claim or feature flag | `Implemented` | Static gate checks pass. |
| `REQ-015` | Focused gate executes every specified proof | `Implemented` | Canonical gate passes 17 Swift and 37 Rust tests with nonzero filters. |

Requirement totals:

| Status | Count |
|---|---:|
| Implemented | 15 |
| Partially Implemented | 0 |
| Missing | 0 |
| Not Verifiable | 0 |
| Out of Scope | 0 |

## 7. Security-Sensitive Diff Scan

Manual range inspection triggered security review because the delta changes test fixture path resolution.

- Result: `Pass`.
- Open Critical/Major `SEC-*`: none.
- All lookup sites are test targets or `#[cfg(test)]` code.
- The nearest matching checkout is selected and absence fails closed.
- The canonical gate controls the current directory from its exact script root.
- Policy bytes remain subject to exact length and digest checks before parsing.
- No auth broadening, secret exposure, unsafe/FFI addition, production path acceptance, or mutable live fallback was introduced.

## 8. Findings

No confirmed in-scope Critical, Major, or Minor findings remain at commit `965bdd3503681d4e14b30836024091925edd8332`.

## 9. Residual Scope and Follow-up Ownership

The proposal's eight explicit follow-up designs remain non-blocking and unchanged:

| Residual scope | Owner | Blocks R4? |
|---|---|---|
| Provider accepted truth and prompt authority | `2026-08-31-provider-accepted-truth-and-prompt-authority-design.md` | No |
| Provider configuration migration/reconciliation | `2026-08-31-provider-configuration-migration-and-reconciliation-design.md` | No |
| P079 repair output materialization | `2026-08-31-p079-repair-output-materialization-design.md` | No |
| P086 resurrection containment | `2026-08-31-p086-resurrection-containment-design.md` | No |
| Provider egress and diagnostics containment | `2026-08-31-provider-egress-and-diagnostics-containment-design.md` | No |
| P031 bounded runtime readback | `2026-08-31-p031-bounded-runtime-readback-design.md` | No |
| Frozen Run replacement/input repair | `2026-08-31-frozen-run-replacement-and-input-repair-design.md` | No |
| Verified provider truth advanced UI | `2026-08-31-verified-provider-truth-ui-design.md` | No |

No residual item is used to defer an in-scope acceptance requirement.

## 10. Readiness Checklist

| Check | Status | Evidence / note |
|---|---|---|
| Exact implementation provenance | Passed | Clean detached `965bdd35`; dirty main excluded |
| Proposal identity | Passed | MD5 and line count unchanged from R3 |
| Complete R4 delta inspection | Passed | Four changed files plus adjacent gate/loader boundaries |
| R3 `UI-002` closure | Passed | Same visible control is attached and owns focus after refresh |
| Test-fixture source provenance | Passed | Runtime current-checkout lookup; no compile-time temporary path |
| Canonical proposal gate | Passed | 17 Swift + 37 Rust; every filter nonzero |
| Structural validation | Passed | Shell syntax, range diff check, scoped Rust formatting |
| Mandatory security pass | Passed | No Critical/Major findings |
| Migration/rollout/telemetry | Not Applicable | Explicit exclusions; no changed surface |
| Release/handoff readiness | Passed | No in-scope blocker remains |

## 11. Verification Log

- Created a clean detached worktree at exact merge commit `965bdd3503681d4e14b30836024091925edd8332` because main contains unrelated dirty files.
- Confirmed target tree `4e598d6872354ea5394d88eef5e4bfad39d07c10` and proposal identity MD5 `6f6191fb7a910acd8580e16df8e528c9`.
- Inspected the complete R4 delta and adjacent production/test boundaries.
- Completed independent macOS UI, Rust architecture/test-infrastructure, and Rust security passes; all passed.
- Ran `./scripts/test-gate.sh codex-planned-variant-slice`: exit 0; 17 Swift and 37 Rust tests passed; every selected filter was nonzero.
- Ran `git diff --check 98a77668..965bdd35`: exit 0.
- Ran `bash -n scripts/test-gate.sh`: exit 0.
- Ran scoped `rustfmt --edition 2021 --check` on all three changed Rust files: exit 0.
- Confirmed the detached worktree remained clean after validation.
- Did not run a live provider, network request, remote UI host, or dedicated product Run; the proposal explicitly excludes them.

## 12. Final Closeout Judgment

All 15 proposal requirements are implemented and all acceptance-relevant proof is present in the exact implementation tree. The R3 Major is closed, the same-tree canonical gate is green, and no in-scope Critical or Major finding remains.

- Overall Conformance: `Implemented`
- Overall Implementation Readiness: `Ready`
- Required code-owned action before implementation closeout: `None`
