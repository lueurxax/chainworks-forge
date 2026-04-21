# Shared Reviewer ID Contract

This plugin preserves reviewer-id continuity between proposal review and implementation audit.

The proposal-review phase selects reviewers from `skills/proposal-review-router/assets/reviewer-registry.yaml`.
The implementation-audit phase reuses compatible selections through `<proposal>.review/reviewer-selection.yaml` or fallback markdown artifacts, then adds evidence-backed delta reviewers from `skills/proposal-implementation-audit/assets/implementation-reviewer-registry.yaml`.

## Required Shared IDs

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

## Compatibility Rules

- Do not rename reviewer ids in repo-local overrides.
- Repo-local reviewers may extend or specialize these ids only when the same lifecycle meaning is preserved.
- Proposal-review output must record selected reviewers, rejected close alternatives, fingerprint tags, and evidence ids.
- Implementation-audit reuse must treat prior reviewer selection as routing context, not implementation proof.
- Delta reviewers must be justified by implementation evidence such as changed files, API/schema changes, migration/rollout surfaces, security boundaries, retry/recovery behavior, performance hot paths, or UI/runtime surfaces.
- If a prior reviewer id is unknown to the implementation-audit registry, record an evidence gap or reroute explicitly instead of silently dropping it.
