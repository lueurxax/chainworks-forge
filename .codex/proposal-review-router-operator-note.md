# Proposal Review Router Plugin Note

This repo-local layer adapts the universal `proposal-review-router` skill to Chainworks Forge.

## What changed

- `.codex/review-router.yaml` maps Chainworks paths to real stack/surface/risk tags.
- Built-in reviewers remain the default. The plugin adds only one repo-specific reviewer: `chainworks_execution_truth_reviewer`.
- Exact repo-local agents were added for macOS UI, Apple UX/architecture, Rust architecture/reliability/security, API contract, rollout/observability, Go/Temporal architecture/reliability, and Chainworks execution truth.
- Root and proposal-area `AGENTS.md` files document routing, artifact layout, validation commands, and safety rules.

## Supported stacks in this repo

- macOS SwiftUI app: active, canonical operator shell.
- Rust control-plane workspace: active parity/control-plane infrastructure.
- Shared workflow/catalog/API contracts: active across Swift, Rust, examples, GraphQL, MCP, ACP.
- Go/Temporal service: proposal-level only until `go.mod` or Go code appears.
- iOS: not active in current repo; use built-in iOS reviewer only with explicit iOS target evidence.

## Example routes

- `Chainworks Forge/Views/BlockedRunRecoveryView.swift` proposal: `macos_ui_reviewer`, `apple_ux_reviewer`, maybe `chainworks_execution_truth_reviewer`.
- Rust work queue retry proposal: `rust_arch_reviewer`, `rust_reliability_reviewer`, `chainworks_execution_truth_reviewer`.
- GraphQL/MCP payload proposal: `rust_arch_reviewer`, `api_contract_reviewer`, maybe `rust_security_reviewer`.
- DB migration plus gate proposal: `rust_arch_reviewer`, `observability_rollout_reviewer`, maybe `chainworks_execution_truth_reviewer`.
- Go/Temporal extraction proposal: `go_service_arch_reviewer`, `go_reliability_reviewer`, `api_contract_reviewer`, `observability_rollout_reviewer`.

## Customize next

- Add a real Go path override after a `go.mod` lands.
- Add a repo-specific release reviewer only if release/delivery proposals keep outgrowing `observability_rollout_reviewer` plus `chainworks_execution_truth_reviewer`.
- Add an iOS agent only if Chainworks Forge gains an iOS target.
