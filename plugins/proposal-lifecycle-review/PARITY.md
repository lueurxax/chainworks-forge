# Parity Map

This map records how the source packages were carried into `proposal-lifecycle-review`.

Status meanings:

- Preserved as-is: copied into the plugin with no semantic change.
- Moved without semantic change: path changed only because the package is now inside a plugin.
- Refactored without semantic change: structure added around the source behavior, with no intended behavior reduction.
- Intentionally improved: packaging or validation improved without reducing capability.
- Intentionally removed: not carried over, with reason.

## Skills

| Source Component | Destination | Status | Notes |
|---|---|---|---|
| `proposal-review-router/SKILL.md` | `skills/proposal-review-router/SKILL.md` | Moved without semantic change | Primary proposal-review skill remains separate. |
| `proposal-implementation-audit/SKILL.md` | `skills/proposal-implementation-audit/SKILL.md` | Moved without semantic change | Primary implementation-audit skill remains separate. |
| No source lifecycle wrapper | `skills/proposal-lifecycle-review/SKILL.md` | Intentionally improved | Thin dispatcher only; no review logic duplicated. |

## Modes

| Mode Set | Status | Notes |
|---|---|---|
| Proposal review: `auto`, `proposal-readiness`, `research`, `ui-only`, `ux-only`, `architecture-only`, `reliability-only`, `performance-only`, `security-only`, `api-contract-only`, `observability-rollout-only`, `product-only`, deprecated `full-review` alias | Preserved as-is | Defined in `skills/proposal-review-router/SKILL.md`. |
| Implementation audit: `auto`, `implementation-audit`, `implementation-readiness`, `conformance-only`, `reuse-proposal-review-selection`, `reroute`, `diff-only`, specialist modes | Preserved as-is | Defined in `skills/proposal-implementation-audit/SKILL.md`. |
| Lifecycle dispatch | Intentionally improved | New wrapper chooses review vs audit by task phase and delegates. |

## Reviewer Registries And IDs

| Source Component | Destination | Status | Notes |
|---|---|---|---|
| Proposal reviewer registry | `skills/proposal-review-router/assets/reviewer-registry.yaml` | Moved without semantic change | Built-in proposal routing registry preserved. |
| Implementation reviewer registry | `skills/proposal-implementation-audit/assets/implementation-reviewer-registry.yaml` | Moved without semantic change | Audit registry preserves compatible ids. |
| Shared reviewer-id list | `shared/reviewer-id-contract.md` | Intentionally improved | Documents continuity contract across both skills. |

Required shared reviewer ids preserved:

- `ios_ui_reviewer`
- `macos_ui_reviewer`
- `apple_ux_reviewer`
- `apple_arch_reviewer`
- `rust_arch_reviewer`
- `rust_reliability_reviewer`
- `rust_performance_reviewer`
- `rust_security_reviewer`
- `go_service_arch_reviewer`
- `go_reliability_reviewer`
- `go_performance_reviewer`
- `go_security_reviewer`
- `api_contract_reviewer`
- `observability_rollout_reviewer`
- `product_reviewer`

## Rubrics

| Source Rubric Family | Destination | Status |
|---|---|---|
| Proposal Apple UI/UX/Architecture rubrics | `skills/proposal-review-router/references/rubrics/` | Moved without semantic change |
| Proposal Rust rubrics | `skills/proposal-review-router/references/rubrics/` | Moved without semantic change |
| Proposal Go rubrics | `skills/proposal-review-router/references/rubrics/` | Moved without semantic change |
| Proposal API/OPS/Product rubrics | `skills/proposal-review-router/references/rubrics/` | Moved without semantic change |
| Implementation Apple UI/UX/Architecture rubrics | `skills/proposal-implementation-audit/references/rubrics/` | Moved without semantic change |
| Implementation Rust rubrics | `skills/proposal-implementation-audit/references/rubrics/` | Moved without semantic change |
| Implementation Go rubrics | `skills/proposal-implementation-audit/references/rubrics/` | Moved without semantic change |
| Implementation API/OPS/Product rubrics | `skills/proposal-implementation-audit/references/rubrics/` | Moved without semantic change |
| Root rubric pointers | `references/rubrics/proposal/`, `references/rubrics/implementation/` | Intentionally improved |

## Playbooks And References

| Source Component | Destination | Status |
|---|---|---|
| Proposal pre-review evidence playbook | `skills/proposal-review-router/references/pre-review-evidence-playbook.md` | Moved without semantic change |
| Proposal research-mode playbook | `skills/proposal-review-router/references/research-mode-playbook.md` | Moved without semantic change |
| Proposal reviewer-selection playbook | `skills/proposal-review-router/references/reviewer-selection-playbook.md` | Moved without semantic change |
| Audit conformance model | `skills/proposal-implementation-audit/references/conformance-model.md` | Moved without semantic change |
| Audit implementation evidence playbook | `skills/proposal-implementation-audit/references/implementation-evidence-playbook.md` | Moved without semantic change |
| Audit reviewer reuse and routing playbook | `skills/proposal-implementation-audit/references/reviewer-reuse-and-routing-playbook.md` | Moved without semantic change |
| Audit example report | `skills/proposal-implementation-audit/references/example-implementation-audit-report.md` | Moved without semantic change |
| Lifecycle artifact contract | `shared/lifecycle-artifact-contract.md` | Intentionally improved |

## Evidence And Report Templates

| Source Component | Destination | Status |
|---|---|---|
| Proposal evidence pack template | `skills/proposal-review-router/assets/evidence-pack-template.md` | Moved without semantic change |
| Proposal final review template | `skills/proposal-review-router/assets/final-review-template.md` | Moved without semantic change |
| Proposal research pack template | `skills/proposal-review-router/assets/research-pack-template.md` | Moved without semantic change |
| Audit report template | `skills/proposal-implementation-audit/assets/implementation-audit-report-template.md` | Moved without semantic change |
| Audit evidence pack template | `skills/proposal-implementation-audit/assets/implementation-evidence-pack-template.md` | Moved without semantic change |
| Reviewer-selection reuse template | `skills/proposal-implementation-audit/assets/reviewer-selection-reuse-template.md` | Moved without semantic change |
| Shared reviewer-selection state template | `shared/reviewer-selection-state-template.yaml` | Intentionally improved |

## Helper Scripts

| Source Component | Destination | Status | Notes |
|---|---|---|---|
| `discover_prior_review.py` | `skills/proposal-implementation-audit/scripts/discover_prior_review.py` | Moved without semantic change | Skill-local script preserved for existing tests and instructions. |
| `report_path.py` | `skills/proposal-implementation-audit/scripts/report_path.py` | Moved without semantic change | Skill-local script preserved for existing tests and instructions. |
| Shared script copies | `scripts/discover_prior_review.py`, `scripts/report_path.py` | Refactored without semantic change | Root copies expose the lifecycle helpers at plugin level. |

## Tests

| Source Component | Destination | Status |
|---|---|---|
| `test_discover_prior_review.py` | `skills/proposal-implementation-audit/tests/test_discover_prior_review.py` | Moved without semantic change |
| `test_report_path.py` | `skills/proposal-implementation-audit/tests/test_report_path.py` | Moved without semantic change |
| Root tests pointer | `tests/README.md` | Intentionally improved |

## Eval Scenarios

| Source Suite | Source Count | Destination | Status |
|---|---:|---|---|
| Proposal review evals | 13 | `skills/proposal-review-router/evals/scenarios.yaml` | Moved without semantic change |
| Implementation audit evals | 10 | `skills/proposal-implementation-audit/evals/scenarios.yaml` | Moved without semantic change |
| Plugin-level union evals | 23 | `evals/scenarios.yaml` | Intentionally improved |

Eval parity note: the merged plugin preserves all 23 effective source scenarios. The plugin-level eval suite is a valid union view with `source_skill` and `source_id` links back to each source scenario. No source eval scenario was intentionally dropped.

## Repo-Local Scaffolding And Templates

| Source / Required Template | Destination | Status |
|---|---|---|
| Proposal `AGENTS-root-template.md` | `skills/proposal-review-router/assets/AGENTS-root-template.md` | Moved without semantic change |
| Proposal `AGENTS-proposals-template.md` | `skills/proposal-review-router/assets/AGENTS-proposals-template.md` | Moved without semantic change |
| Proposal repo routing config template | `skills/proposal-review-router/assets/repo-routing-config-template.yaml` | Moved without semantic change |
| Proposal reviewer plugin template | `skills/proposal-review-router/assets/reviewer-plugin-template.yaml` | Moved without semantic change |
| Audit `AGENTS-root-template.md` | `skills/proposal-implementation-audit/assets/AGENTS-root-template.md` | Moved without semantic change |
| Audit `AGENTS-proposals-template.md` | `skills/proposal-implementation-audit/assets/AGENTS-proposals-template.md` | Moved without semantic change |
| Audit repo implementation config template | `skills/proposal-implementation-audit/assets/repo-implementation-audit-config-template.yaml` | Moved without semantic change |
| Audit reviewer plugin template | `skills/proposal-implementation-audit/assets/implementation-reviewer-plugin-template.yaml` | Moved without semantic change |
| Combined root guidance templates | `assets/templates/AGENTS-root-template.md`, `assets/templates/AGENTS-proposals-template.md` | Intentionally improved |
| `.codex/proposal-lifecycle-router.yaml` template | `assets/templates/proposal-lifecycle-router.yaml` | Intentionally improved |
| `.codex/review-router.yaml` template | `assets/templates/review-router.yaml` | Intentionally improved |
| `.codex/implementation-audit-router.yaml` template | `assets/templates/implementation-audit-router.yaml` | Intentionally improved |
| `.codex/reviewers/*.yaml` example | `assets/templates/reviewer-example.yaml` | Intentionally improved |
| `.codex/implementation-reviewers/*.yaml` example | `assets/templates/implementation-reviewer-example.yaml` | Intentionally improved |

## Intentionally Removed

| Source Item | Reason |
|---|---|
| `.DS_Store` | Non-functional macOS metadata. |
| `__pycache__/` and `*.pyc` | Generated Python cache files. |

No functional skill content, registry, rubric, playbook, helper script, test, eval scenario, or template was intentionally removed.

## No Functionality Lost?

Fully preserved:

- Proposal-first local evidence intake.
- Baseline reuse and narrow refresh behavior.
- Fingerprint-before-routing.
- Selective proposal reviewer routing.
- Proposal research gating after local evidence completion.
- Proposal evidence pack, final review, and research pack structures.
- Implementation-audit prior-review discovery and reviewer-selection reuse.
- Explicit reroute mode.
- Delta reviewer addition from implementation evidence.
- Atomic `REQ-*` model and proof-oriented statuses.
- Versioned implementation report generation.
- Audit read-only boundary except one report output.
- Helper scripts and script tests.
- Reviewer ids and rubrics across Apple, Rust, Go, API, OPS, and Product families.

Behaviorally equivalent after refactor:

- The two primary workflows now live under plugin `skills/` paths, but their internal relative links and resources remain skill-local.
- Root-level shared docs/templates/scripts add lifecycle packaging without replacing primary skill behavior.
- Plugin-level eval suite provides a valid union index while preserving original skill-local eval files.

Intentionally not carried over:

- Only `.DS_Store`, `__pycache__/`, and `*.pyc` were omitted.
- No MCP servers, apps, or hooks were added because the source packages did not require them.
