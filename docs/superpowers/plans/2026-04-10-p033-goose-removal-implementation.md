# Proposal 033 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Remove Goose from the canonical runtime path and make ACP the only live runtime architecture, while preserving explicit migration and blocking behavior for Goose-era persisted data.

**Architecture:** Keep Goose-era compatibility only at persistence boundaries: raw settings migration and blocked legacy runs. Remove Goose from live catalog, MCP resolution, runtime factory, fixtures, tests, and operator-facing runtime truth. Implement the cut in proof-first order so the canonical execution path is ACP-only before doc and naming cleanup.

**Tech Stack:** Swift, SwiftData, xcodebuild tests, YAML-backed agent catalog, ACP subprocess transports.

---

### Task 1: Lock the P033 seam inventory

**Files:**
- Modify: `/Users/user/Documents/Chainworks Forge/docs/proposals/033-remove-goose-from-canonical-transport-and-simplify-runtime.md`
- Create: `/Users/user/Documents/Chainworks Forge/docs/superpowers/plans/2026-04-10-p033-goose-removal-implementation.md`

- [ ] Confirm the approved cut remains proof-first: catalog/MCP/runtime/provider/test seams first, doc cleanup after code proof.
- [ ] Keep Goose-era migration compatibility only where proposal requires it: raw provider-settings migration and blocked Goose-bound run resume.

### Task 2: Remove Goose from canonical catalog and MCP model

**Files:**
- Modify: `/Users/user/Documents/Chainworks Forge/examples/agents/agents.yaml`
- Modify: `/Users/user/Documents/Chainworks Forge/Chainworks Forge/DSL/AgentCatalog.swift`
- Modify: `/Users/user/Documents/Chainworks Forge/Chainworks Forge/DSL/YAMLValidator.swift`
- Modify: `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Engine/MCPPolicyRuntime.swift`
- Test: `/Users/user/Documents/Chainworks Forge/Chainworks ForgeTests/Proposal033Tests.swift`

- [ ] Write a failing test proving P033 catalog/runtime truth rejects repo-owned `mcp_server_registry` / `mcp_profiles` as canonical owner.
- [ ] Replace catalog-owned `mcp_server_registry` / `mcp_profiles` reads with backend/runtime-owned MCP intent and machine-local runtime registry only.
- [ ] Remove any `~/.config/goose/config.yaml` fallback or Goose namespace handling from MCP resolution.

### Task 3: Remove Goose from runtime execution and provider platform

**Files:**
- Modify: `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Engine/ExecutionService.swift`
- Modify: `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Engine/RuntimeSessionBridge.swift`
- Modify: `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Providers/ProviderSettingsStore.swift`
- Modify: `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Support/SettingsTransferService.swift`
- Modify: `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Engine/ResumeManager.swift`
- Test: `/Users/user/Documents/Chainworks Forge/Chainworks ForgeTests/Proposal033Tests.swift`
- Test: `/Users/user/Documents/Chainworks Forge/Chainworks ForgeTests/RuntimeSessionBridgeTests.swift`

- [ ] Write failing tests for: no Goose family in transport factory, Goose-era settings import migration, Goose-bound run blocking.
- [ ] Remove any live Goose transport/bootstrap/runtime-manager path from execution services while keeping blocked legacy-run classification.
- [ ] Preserve raw Goose-era settings migration and transfer import migration exactly as the compatibility boundary.

### Task 4: Replace Goose fixtures/tests and operator-facing vocabulary

**Files:**
- Modify: `/Users/user/Documents/Chainworks Forge/Chainworks ForgeTests/RuntimeSessionBridgeTests.swift`
- Modify: `/Users/user/Documents/Chainworks Forge/Chainworks ForgeTests/RuntimeAgentExecutorTests.swift`
- Modify: `/Users/user/Documents/Chainworks Forge/Chainworks ForgeTests/ProviderPlatformTests.swift`
- Delete or replace Goose-only test files if still compiled
- Modify: `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Views/RunsHomeView.swift`
- Modify: `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Views/IdeaListView.swift`

- [ ] Write failing tests for operator-facing strings and proof fixtures still using Goose vocabulary.
- [ ] Replace remaining Goose-only fixtures with ACP fixtures and runtime-neutral names.
- [ ] Remove operator-facing Goose wording except for explicit legacy migration/blocking contexts required by P033.

### Task 5: Verify P033 proof lane

**Files:**
- Test: `/Users/user/Documents/Chainworks Forge/Chainworks ForgeTests/Proposal033Tests.swift`
- Test: `/Users/user/Documents/Chainworks Forge/Chainworks ForgeTests/RuntimeSessionBridgeTests.swift`
- Test: `/Users/user/Documents/Chainworks Forge/Chainworks ForgeTests/RuntimeAgentExecutorTests.swift`
- Test: `/Users/user/Documents/Chainworks Forge/Chainworks ForgeTests/ProviderPlatformTests.swift`

- [ ] Run app build.
- [ ] Run focused P033-targeted tests.
- [ ] Re-scan source for remaining Goose runtime references and summarize any residual intentional compatibility references only.
