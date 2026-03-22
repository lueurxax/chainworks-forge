# Consolidated Review

## 0. Review Mode and Evidence Summary
- Review date: `2026-03-22`
- Mode used: `full-review`
- Output type: `Evidence Gap Review`
- Evidence completeness: `Partial`
- Product overlay: not triggered
- Documents / repo inputs reviewed: Proposal 001, README, MVP/problem framing docs, canonical YAML examples, current app/test files, extracted launch screenshots, and the proposal's own response log.
- Build/run attempts:
  - `RUN-01`: `xcodebuild -project "Chainworks Forge.xcodeproj" -scheme "Chainworks Forge" -destination "platform=macOS" build` succeeded on `2026-03-22`.
  - `RUN-02`: `xcodebuild -project "Chainworks Forge.xcodeproj" -scheme "Chainworks Forge" -destination "platform=macOS" test` succeeded on `2026-03-22`; result bundle at `~/Library/Developer/Xcode/DerivedData/Chainworks_Forge-ayratnusrqmfbievclfarmszyhkv/Logs/Test/Test-Chainworks Forge-2026.03.22_10-47-58-+0200.xcresult`.
- Screenshots available:
  - `SCR-01`: light-mode launch baseline
  - `SCR-02`: dark-mode launch baseline
- Code areas inspected:
  - `Chainworks Forge/Chainworks_ForgeApp.swift`
  - `Chainworks Forge/ContentView.swift`
  - `Chainworks Forge/Item.swift`
  - `Chainworks ForgeTests/*`
  - `Chainworks ForgeUITests/*`
  - `examples/agents/agents.yaml`
  - `examples/workflows/workflow.yaml`
  - `examples/workflows/proposal-to-release.yaml`
- Remaining blockers:
  - Proposal 001 is still not implemented in the app.
  - Current screenshots still prove only the stock template baseline, not the proposed foundation UI.

## Evidence Gap Review
- What was attempted:
  - Re-read the current Proposal 001 draft.
  - Re-checked the parser, normalizer, and validator contracts against the real `examples/*.yaml` fixtures.
  - Re-ran `xcodebuild build` and `xcodebuild test` on the current repo baseline.
  - Re-used the existing launch screenshot evidence because the app UI is still the stock template.
- What is missing:
  - An implemented foundation slice matching the proposal.
  - Runtime proof that full parsing, compact parsing, normalization, validation, provenance snapshotting, and the verification scaffold all work together.
  - Screenshots for the actual Ideas, Agent Catalog, and Workflow Inspector states promised by the proposal.
- Blockers:
  - The repo is still the template app, so the target flow cannot be exercised.
  - The compact-workflow contract remains partially under-specified against the repo fixtures.
- Confidence: `Medium`
- What can still be said with partial confidence:
  - The earlier findings around workflow provenance, deterministic hashing, `RunGuard`, validator breadth, and scaffold scope are materially improved in the current draft.
  - No new UI or UX blocking issues surfaced in this pass.
  - The remaining blockers are concentrated in the compact-workflow parser/normalizer contract.
- What evidence is required to finish the full review:
  - Implement Proposal 001.
  - Exercise both full and compact YAML paths in code.
  - Capture the real scaffold in success and failure states.
  - Show validator output against the normalized compact workflow and the canonical catalog.

## 1. Findings Supported With Partial Confidence

### 1.1 Architecture Findings
- Finding ID: `ARCH-009`
  Severity: `High`
  File/lines: `docs/proposals/001-foundation-domain-model-and-yaml-parser.md:732-738`
  Why it matters:
  The proposal now claims every YAML-facing snake_case type has explicit `CodingKeys`, but `CompactWorkflowMeta` still omits them even though the real compact fixture uses `required_providers`. As written, `proposal-to-release.yaml` does not satisfy the proposal's own "decode without preprocessing" contract.
  Recommended fix:
  Add explicit `CodingKeys` for compact workflow types, at minimum `requiredProviders = "required_providers"`, and bring the compact path under the same documented decoding contract as the full path.
  Acceptance criteria:
  `testParseCompactWorkflow()` passes against `examples/workflows/proposal-to-release.yaml` using the documented structs and plain `YAMLDecoder().decode(...)`.
  Confidence: `High`

- Finding ID: `ARCH-010`
  Severity: `High`
  File/lines: `docs/proposals/001-foundation-domain-model-and-yaml-parser.md:754-773`
  Why it matters:
  The normalizer rules never define how compact agent IDs become canonical catalog IDs. The real compact fixture uses names like `proposal-writer`, `proposal-po-reviewer`, `security-checker`, `github-commit-push`, and `connect-publisher`, while the canonical catalog exposes `proposal_writer`, `proposal_reviewer_product_owner`, `security_checker`, `commit_and_push_to_github`, and `build_archive_and_push_connect`. Without an explicit alias table or mapping rule, the normalized compact workflow cannot pass the proposal's own `validateAgentReferences()` contract against the real catalog.
  Recommended fix:
  Define a deterministic compact-to-canonical agent ID mapping rule, or explicitly narrow compact workflow support to non-executable inspection only.
  Acceptance criteria:
  Normalizing `proposal-to-release.yaml` and then running `validateAgentReferences(normalized.definition, catalog)` against `examples/agents/agents.yaml` yields zero agent-reference errors.
  Confidence: `High`

- Finding ID: `ARCH-011`
  Severity: `Medium`
  File/lines: `docs/proposals/001-foundation-domain-model-and-yaml-parser.md:754-779`
  Why it matters:
  `NormalizedWorkflow.definition` is described as "usable by engine", but compact stages do not carry `AgentTask.task`, `inputs`, `outputs`, or a concrete `Transition.when` expression language. The proposal documents missing `inputs`/`outputs`, but it still does not specify how required `task` and `when` values are derived. That leaves the compact normalization contract incomplete and makes "correct normalization" hard to test consistently.
  Recommended fix:
  Either narrow compact normalization to inspector-only output, or define deterministic derivation rules for `task`, IO bindings, `when` expressions, and every defaulted execution field.
  Acceptance criteria:
  The proposal can explain, field by field, how each normalized `AgentTask` and `Transition` value is sourced or defaulted from compact YAML, and the corresponding tests assert those exact rules.
  Confidence: `Medium`

## 2. Resolved Since Prior Pass
- The previous scaffold-scope issue appears fixed at `docs/proposals/001-foundation-domain-model-and-yaml-parser.md:960-963`; the draft now explicitly keeps the scaffold to CRUD, parsing, and validation, without `Start Run` or workflow-selection affordances.
- The prior provenance-hash determinism issue appears addressed by the canonical encoder contract at `docs/proposals/001-foundation-domain-model-and-yaml-parser.md:149-181`.
- The prior single-active-run TOCTOU issue appears materially improved by the `@MainActor` `RunGuard.createRun(...)` contract at `docs/proposals/001-foundation-domain-model-and-yaml-parser.md:203-270`.

## 3. Union Readiness
- Union readiness: `Red`
- Rationale:
  - UI/UX specification for the minimal scaffold is now mostly coherent for this slice.
  - The full-workflow path is much closer to implementation-ready than before.
  - The compact-workflow path is still not contract-complete against the real fixtures, and compact support is part of the proposal's stated acceptance criteria.
  - Because this is a foundation proposal, unresolved parser/normalizer contracts keep the overall readiness in `Red` despite the draft's improvement.

## 4. Final Judgment
- The draft is materially better than the previous pass and closes several earlier blockers.
- It is still not implementation-ready as written because the compact parser/normalizer path does not yet survive a repo-reality check against `proposal-to-release.yaml` plus `agents.yaml`.
- If compact support is either fully specified or intentionally narrowed, the proposal is close to moving out of `Red`.
