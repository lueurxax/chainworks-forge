# Proposal Lifecycle Review

Proposal Lifecycle Review packages two proposal-driven Codex workflows as one installable plugin:

- `proposal-review-router`: proposal-first review, evidence intake, fingerprinting, selective reviewer routing, proposal-readiness, research, evidence packs, and final reviews.
- `proposal-implementation-audit`: proposal-anchored implementation audit, prior reviewer-selection reuse, `REQ-*` conformance tracking, routed implementation findings, readiness roll-up, versioned audit reports, scripts, tests, and evals.

The plugin intentionally keeps these workflows separate. The optional `proposal-lifecycle-review` skill is only a thin dispatcher.

## Skills

| Skill | Purpose |
|---|---|
| `proposal-review-router` | Review a proposal before implementation, choose reviewers, and produce proposal-readiness/research artifacts. |
| `proposal-implementation-audit` | Audit an implementation, diff, branch, PR, or current worktree against a proposal. |
| `proposal-lifecycle-review` | Thin convenience entrypoint that delegates to one of the two primary skills. |

## Lifecycle Flow

1. Use `proposal-review-router` before implementation.
2. Preserve `<proposal>.review/reviewer-selection.yaml` plus evidence/final/research artifacts.
3. Use `proposal-implementation-audit` during or after implementation.
4. Reuse prior reviewer selection when valid.
5. Add delta reviewers only when implementation evidence introduces new surfaces or risks.

## Reviewer ID Continuity

The shared reviewer-id contract is in `shared/reviewer-id-contract.md`.

The following ids are preserved across proposal review and implementation audit:

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

## Package Layout

```text
proposal-lifecycle-review/
├── .codex-plugin/plugin.json
├── skills/
│   ├── proposal-review-router/
│   ├── proposal-implementation-audit/
│   └── proposal-lifecycle-review/
├── shared/
├── assets/templates/
├── scripts/
├── tests/
├── evals/
├── references/
├── README.md
├── INSTALL.md
├── MIGRATION.md
└── PARITY.md
```

## Validation

From the plugin root:

```bash
python3 -m json.tool .codex-plugin/plugin.json >/dev/null
python3 /Users/user/.codex/skills/.system/skill-creator/scripts/quick_validate.py skills/proposal-review-router
python3 /Users/user/.codex/skills/.system/skill-creator/scripts/quick_validate.py skills/proposal-implementation-audit
python3 /Users/user/.codex/skills/.system/skill-creator/scripts/quick_validate.py skills/proposal-lifecycle-review
python3 -m unittest discover -s skills/proposal-implementation-audit/tests
```

The source eval files are preserved in each skill. A plugin-level union eval suite is available at `evals/scenarios.yaml`.
