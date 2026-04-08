# Per-Agent MCP Policy and Runtime Validation

Stable reference for how Chainworks Forge resolves per-agent MCP intent, validates that intent against the selected runtime, persists requested/predicted/actual truth, and exposes the results in operator-facing surfaces.

## Purpose

MCP access is runtime policy, not an untracked side effect.

For any execution, the system must be able to explain:

- which per-agent MCP profile was requested,
- which runtime servers/extensions were predicted,
- which ones actually became effective,
- which ones were denied or unavailable,
- and where that truth appears in reports and comparison.

## Scope

This reference covers:

- catalog-owned `mcp_profile`,
- `mcp_policy`, `mcp_server_registry`, and `mcp_profiles`,
- preflight and runtime validation,
- runtime-specific MCP realization,
- requested / predicted / actual / denied truth,
- and MCP telemetry in reports and comparisons.

It does not define:

- provider credential setup,
- generic runtime transport selection,
- or external MCP server authoring outside the catalog/runtime contract.

## Related docs

- [provider-platform.md](provider-platform.md)
- [runtime-contract.md](runtime-contract.md)
- [acp-runtime-transport.md](acp-runtime-transport.md)
- [operator-experience.md](operator-experience.md)
- [skill-resolution-and-runtime-integration.md](skill-resolution-and-runtime-integration.md)

## Canonical ownership

The canonical MCP ownership model is:

- `AgentCatalog.mcpPolicy` defines system-wide policy defaults,
- `mcp_server_registry` defines named runtime-relevant MCP servers,
- `mcp_profiles` define reusable policy bundles,
- `agent.mcp_profile` selects the effective runtime intent for an agent.

Permission profiles do not own modern MCP truth.
They may still provide legacy ceiling context, but runtime authority lives on the catalog MCP model above.

## Resolution model

The system tracks four distinct truth layers:

1. `requested`
2. `predicted`
3. `actual`
4. `denied`

These layers must not be collapsed.

### Requested

What the selected `mcp_profile` asks for.

### Predicted

What the app expects to become active after applying catalog policy plus runtime mapping.

### Actual

What the runtime actually settled on for the execution.

### Denied

What policy or runtime constraints blocked from the requested set.

## Resolution pipeline

The implemented path is:

```text
AgentCatalog
  -> YAML validation
  -> RunPlanCompiler / ResolvedAgent
  -> MCPPolicyResolver
  -> GooseSessionBridge or ACP-native runtime path
  -> AgentExecution settlement
  -> RunReportBuilder / RunComparisonService / operator shell
```

### Validation

Before execution, the catalog validator checks:

- missing MCP profile references,
- unknown server registry references,
- unsupported fallback policy values,
- conflicting required/optional declarations,
- legacy permission-profile MCP drift.

### Runtime realization

Runtime realization depends on adapter family:

- Goose-backed runtimes reconcile against Goose extension IDs,
- ACP-native runtimes keep Forge truth but realize MCP through ACP-native semantics,
- portability-sensitive paths must not hardcode workstation-specific absolute paths.

The runtime layer may vary, but the persisted Forge truth model remains the same.

## Persistence and reporting

The MCP slice is durable only if post-run truth stays inspectable.

Current persisted lanes include:

- frozen MCP policy data in the run snapshot,
- `AgentExecution` MCP fields,
- report payload MCP sections,
- comparison payload MCP sections,
- aggregate telemetry.

### Reported telemetry

Current report telemetry includes:

- executions with MCP profile,
- zero-MCP executions,
- requested extension count,
- predicted extension count,
- actual extension count,
- denied extension count,
- prompt/context delta attributable to MCP,
- blocked runs caused by MCP preflight,
- per-server usage summary when available.

## Operator-visible surfaces

MCP truth is part of the existing shell-owned explanation path.

### Run reports

Reports expose:

- requested MCP set,
- predicted set,
- actual set,
- denied set,
- MCP telemetry and drift summaries.

### Comparison

Comparison can explain why two runs differ at the MCP/runtime layer rather than reducing the difference to generic provider drift.

### Artifact inspection and diagnostics

Execution receipts and diagnostics preserve enough MCP truth to explain runtime variance after the fact.

### Preflight and readiness

Start-time preflight remains the early warning surface when the runtime cannot honor the requested MCP profile safely.

## Integration with the app

This capability is integrated into four existing system layers:

1. catalog validation and preflight,
2. runtime packet/session preparation,
3. execution settlement and telemetry persistence,
4. operator-facing reports, comparison, and diagnostics.

That means MCP policy is not a background adapter detail. It is part of the product's execution truth.

## Current implementation owners

- `Chainworks Forge/DSL/AgentCatalog.swift`
- `Chainworks Forge/DSL/YAMLValidator.swift`
- `Chainworks Forge/Engine/MCPPolicyRuntime.swift`
- `Chainworks Forge/Engine/GooseSessionBridge.swift`
- `Chainworks Forge/Engine/GooseAdapter/GooseServerTransport.swift`
- `Chainworks Forge/Engine/RunReportBuilder.swift`
- `Chainworks Forge/Engine/RunComparisonService.swift`
- `Chainworks Forge/Models/AgentExecution.swift`
- `Chainworks Forge/Views/RunReportView.swift`
- `Chainworks Forge/Views/RunComparisonView.swift`
- `Chainworks Forge/Views/ProviderTroubleshootingPanel.swift`

## Verification baseline

Current stable verification for this slice is:

- dedicated MCP capability regression coverage on the current tree
- current focused verification summary `36/36` passed
- capability verification still re-proves the portability-sensitive MCP assertions
- same-tree approved-host `full` green basis:
  - `full-20260408-101540.xcresult`
