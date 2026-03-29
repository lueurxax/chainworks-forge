# Current System Baseline

Review-intake baseline for the current Chainworks Forge repository.

Primary stable source:

- [../docs/reference/current-system-baseline.md](../docs/reference/current-system-baseline.md)

## Use this artifact for

- proposal-readiness review setup,
- implementation-audit orientation,
- dependency-chain validation after proposal files are promoted into `docs/reference/`,
- avoiding repeated direct code/doc remapping for the same current-head system shape.

## Current repository baseline

- Product type: macOS operator tool for multi-agent idea-to-delivery workflows
- Execution model: YAML workflow + agent catalog compiled into a persisted run/state machine
- Runtime: simulated and live Goose-backed execution paths
- Operator shell: implemented and stable
- Provider/settings platform: implemented and stable
- Repo-backed full delivery: implemented and stable
- MVP sign-off layer: implemented and stable

## Canonical stable references

- `docs/reference/workflow-execution-engine.md`
- `docs/reference/runtime-contract.md`
- `docs/reference/operator-experience.md`
- `docs/reference/provider-platform.md`
- `docs/reference/provider-binding-truth.md`
- `docs/reference/project-workspace-contract.md`
- `docs/reference/idea-lifecycle.md`
- `docs/reference/live-workflow-map.md`
- `docs/reference/full-mvp-delivery.md`
- `docs/reference/mvp-sign-off.md`
- `docs/reference/current-system-baseline.md`

## Review assumptions that should now be reusable

1. Active dependencies should prefer stable `docs/reference/` documents over removed or superseded proposal files.
2. The current MVP provider families are `codex`, `claude_code`, and `gemini`.
3. The current repo-backed delivery path is already baseline behavior, not future-state scope.
4. The current sign-off layer is already a stable reference and should not depend on removed Proposal 008 artifacts.
5. Review rounds should only fall back to direct code/doc mapping when the reviewed area lacks a stable reference.

## When to refresh this baseline

Refresh this artifact when one of the following changes materially:

- top-level provider boundary,
- operator shell ownership,
- repo-backed delivery topology,
- sign-off contract,
- reference-doc layout or promotion of major proposals into `docs/reference/`.
