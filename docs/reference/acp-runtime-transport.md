# ACP Runtime Transport

Stable reference for the ACP-only runtime transport layer in Chainworks Forge, including runtime selection, adapter families, and persisted runtime truth.

## Purpose

Runtime execution should be transport-neutral at the control-plane level.

The app must be able to:

- compile runs without binding orchestration to provider-specific endpoint semantics,
- select a runtime through catalog/runtime-profile truth,
- persist transport-neutral execution truth,
- support multiple ACP-capable runtimes,
- and keep the control plane independent from any one adapter family.

## Scope

This reference covers:

- `RuntimeTransportProtocol`,
- runtime transport factory selection,
- catalog-owned runtime profiles,
- backend-profile to runtime-profile binding,
- ACP adapter families,
- runtime-profile and backend-profile interaction,
- persisted transport-neutral run truth,
- and operator-facing consequences of runtime selection.

It does not define:

- provider credentials and setup UX,
- MCP policy semantics,
- or future second-wave runtime expansion beyond the currently implemented adapters.

## Related docs

- [workflow-execution-engine.md](workflow-execution-engine.md)
- [runtime-contract.md](runtime-contract.md)
- [provider-platform.md](provider-platform.md)
- [per-agent-mcp-policy-and-runtime-validation.md](per-agent-mcp-policy-and-runtime-validation.md)
- [live-provider-execution-slice.md](live-provider-execution-slice.md)

## Canonical transport contract

Core execution depends on `RuntimeTransportProtocol` as the stable ACP runtime boundary.

The transport contract owns:

- session creation,
- prompt submission,
- stream events,
- session close,
- runtime namespace/capability hints,
- and transport-level diagnostics.

The control plane still owns:

- run lifecycle,
- stage transitions,
- approvals,
- artifact persistence,
- recovery,
- reports,
- and frozen run truth.

## Runtime selection model

Runtime selection is catalog-driven.

The current chain is:

```text
AgentCatalog backend_profile/runtime_profile
  -> BackendProfileResolverV2
  -> ResolvedProviderBinding
  -> RuntimeTransportFactory
  -> selected adapter family
```

No screen or executor should invent a second runtime owner path.

## Catalog-owned runtime profiles

Runtime profile intent is stored in the agent catalog, not invented in view code or machine-local settings.

Current owner fields live in `AgentCatalog.RuntimeProfile`:

- `capability_class`
- `adapter_family`
- `requires`
- `transport_kind`
- `mcp_realization_path`

Current repo-backed runtime profiles are:

| Runtime profile | Adapter family | Capability class | Transport kind | MCP realization |
|---|---|---|---|---|
| `claude_agent_acp` | `claude_agent_acp` | `operator_grade` | `acp_stdio` | `acp_native` |
| `gemini_cli_acp` | `gemini_cli_acp` | `control_capable` | `acp_stdio` | `acp_native` |
| `codex_acp` | `codex_acp` | `operator_grade` | `acp_stdio` | `acp_native` |
| `auggie_cli_acp` | `auggie_cli_acp` | `control_capable` | `acp_stdio` | `acp_native` |
| `junie_cli_acp` | `junie_cli_acp` | `control_capable` | `acp_stdio` | `acp_native` |

`RuntimeProfile.requires` is a normative capability map — it gates launch, startup preflight, and MCP-policy reconciliation. Profiles that are known but missing required capabilities block run execution via the deterministic readiness contract. Disabled, configured, and unavailable states remain distinct in operator-facing surfaces (not collapsed into a single message path).

## Backend-profile ownership

Agents continue to select only `backend_profile`.

`backend_profile` remains the single repo-owned bundle for:

- provider family,
- model,
- effort,
- structured-output intent,
- and backend-owned required MCP.

This keeps runtime selection attached to the same binding lane that already owns provider/model intent.

In the current catalog:

- Claude-backed operator and writer profiles bind to `claude_agent_acp`
- Gemini review profiles bind to `gemini_cli_acp`
- Codex-backed implementation and authoring profiles bind to `codex_acp`
- Auggie and Junie remain ACP-only families where configured

The current resolver path is:

```text
backend_profile
  -> optional runtime_profile
  -> BackendProfileResolverV2
  -> ResolvedProviderBinding(runtimeProfileID, adapterFamily, capabilityClass)
  -> RuntimeTransportFactory
```

## Implemented transport families

### ACP-native adapters

The Rust control plane owns live ACP subprocess execution. Provider adapters
are normalized into canonical provider families (`claude`, `gemini`, `codex`,
`auggie`, `junie`) for consistent capacity management and backpressure:

- `ClaudeAgentAdapter` (canonical: `claude`)
- `GeminiCliAdapter` (canonical: `gemini`)
- `CodexAdapter` (canonical: `codex`)
- `AuggieAdapter` (canonical: `auggie`)
- `JunieAdapter` (canonical: `junie`)

Shared ACP plumbing lives in `control-plane/crates/acp/src/transport.rs`.
Junie must be launched in explicit ACP mode with `--acp true`; plain `junie`
does not enter the JSON-RPC ACP handshake.

Junie structured-output capability is covered by the retained
`proposal-089|p089` gate alias. That gate preserves native Junie CLI proof for
strict JSON and strict `CHAINWORKS_OUTPUT`, then validates a tiny ACP
`code_writer` canary through the production `junie_code_editor_acp` backend
profile, `JunieAdapter`, the full production code-writer output set, and the
engine-owned settlement/materialization path. The proof is intentionally
bounded: it establishes Junie viability for structured output and the small ACP
handoff boundary, but it is not a guarantee that long-running implementation
attempts cannot regress or that a broader P036-class failure is fixed.

Junie `code_writer` execution has an adapter preflight before provider launch.
The preflight validates the execution root, project readability, write access to
required output parents, runtime cache, and temporary directory. Diagnostic mode
records the same lifecycle facts without blocking launch; enforced mode fails
closed before the subprocess is spawned when a non-remediable path problem is
found. Wrong-cwd and runtime-cache failures get one remediation attempt, and
the durable lifecycle records `preflight_running`, `preflight_remediating`,
`passed`, or `failed_no_launch` in the runtime receipt.

The rule is: provider capacity accounting starts only after preflight passes. The
post-preflight provider launch gate persists the launch lease before spawning
the ACP subprocess, so preflight-only rows with
`runtime_preflight_provider_launched=false` do not consume the Junie provider
cap, while launched rows do. Completion-boundary receipt fields, failure
envelopes, and per-output settlement rows are owned by
[output-contracts-failure-evidence-and-recovery.md](output-contracts-failure-evidence-and-recovery.md#junie-code-writer-completion-boundary).

When an invocation declares `requires_xcode_host_execution` or
`xcode_shim_injection_signal`, the engine treats that as a brokered `xcode` MCP
requirement before ACP startup. This forces Xcode MCP lease acquisition and
warm-up to happen before the provider subprocess receives the task; if the
broker or registry is unavailable, the invocation fails closed before the agent
is launched.

Runtime provider subprocesses start with cwd set to the active execution root:
the run worktree for write-enabled implementation work, otherwise
`workspace_root`. This keeps terminal commands, provider-local project context,
and MCP-backed tool resolution aligned with the same tree the orchestrator
expects the agent to operate on.

Permission auto-grant prefers provider-declared read-only allowlist options
before falling back to one-shot approval. This preserves autonomous operation
without repeatedly exercising fragile terminal approval round-trips for safe
read-only commands such as `cat`, `ls`, and `grep`.

If an ACP session fails by idle/progress timeout after provider progress and the
runtime receipt's final events include streamed text or a diff update, the
engine records the failure as a recoverable handoff gap instead of an ordinary
provider timeout. The persisted runtime facts use
`failure_kind = missing_required_outputs`,
`supervision_classification = recoverable_handoff_gap_after_provider_progress`,
and `transport_error_code = ACP_HANDOFF_IDLE_AFTER_DIFF`. Permission waits still
win first: an ungranted permission request remains classified as
`waiting_on_permission_roundtrip`.

The Swift app process does not spawn live ACP providers. It reads durable run
truth through GraphQL and disk-backed artifacts, and its local transport factory
rejects live ACP adapter families.

#### Toolchain Cache Mapping

The ACP layer manages the isolation of toolchain-specific build and cache roots, including Xcode DerivedData and Go caches. This section is the stable owner for provider-launched toolchain cache mapping behavior.

- **Environment Redirection**: ACP adapters derive the appropriate toolchain root based on the agent's `toolchain_cache_policy` and session/run scope. They publish `CHAINWORKS_TOOLCHAIN_HOME` and `TOOLCHAIN_HOME` and apply family-specific redirection (e.g., `-derivedDataPath` for Xcode, `GOCACHE` for Go).
- **Exclusive Serialization**: For run-scoped Xcode work, the host-executor path acquires an exclusive per-run lease to prevent concurrent mutation of the same DerivedData root.
- **Diagnostics**: Adapters capture setup and mapping metadata, stored as `actualToolchainMappingDiagnostics` on the execution record.
- **Apple Read Adapter**: Swift operator-facing consumers decode toolchain mapping truth through `ToolchainMappingReadAdapter` to ensure consistent handling of frozen-snapshot compatibility and legacy sentinels.

#### Bounded Discovery and DiscoveryFilesystem

Broad filesystem discovery is not part of the pre-`initialize` path. Instead of implicit inference from the entire repository or worktree, the system uses a bounded discovery model:

- **DiscoveryFilesystem Ownership**: Shared discovery value-types and filesystem logic live in `domain::discovery`, while policy construction remains engine-owned.
- **Bounded Discovery**: Discovery is restricted to the run meta-root and explicitly declared expected output paths.
- **Pre-Prompt Metadata**: Metadata capture is now a per-execution, per-prompt-turn step for both fresh and reused sessions.
- **Settlement Pipeline**: An engine-owned pipeline settles discovered artifacts based on typed expected outputs and discovery decisions.

## Current app factory behavior

`DefaultRuntimeTransportFactory` is app-local only. It accepts fixture transport
injection for tests and fails closed for non-empty live adapter families such as
`claude_agent_acp`, `gemini_cli_acp`, `codex_acp`, `auggie_cli_acp`, and
`junie_cli_acp`.

## Persisted runtime truth

The runtime slice is only useful if transport decisions become durable execution truth.

Current persisted lanes include:

- run-start provider/runtime binding snapshot,
- runtime-profile and adapter-family truth on execution records,
- effective runtime namespace used for MCP/runtime settlement,
- report/comparison visibility of actual runtime family used,
- recovery logic that reasons from frozen runtime truth instead of current disk defaults.

This is what keeps an ACP-backed run explainable after relaunch or resume.

Concrete persisted execution truth includes:

- `ResolvedProviderBinding.runtimeProfileID`
- `ResolvedProviderBinding.adapterFamily`
- `ResolvedProviderBinding.capabilityClass`
- `AgentExecution.runtimeProfileID`
- `AgentExecution.actualAdapterFamily`

## ACP operator impact

Operator surfaces continue to read persisted Forge truth rather than adapter internals.

That means:

- reports describe the effective runtime family used,
- comparison can explain runtime-family drift,
- recovery works from frozen transport/runtime truth,
- and runtime selection is visible without exposing raw adapter implementation details.

## Integration with the rest of the app

The transport layer plugs into the app in five stable places:

1. catalog/runtime-profile selection,
2. run-start binding freeze,
3. executor session lifecycle,
4. persisted execution and report truth,
5. operator-facing recovery and comparison.

This is how ACP support became part of the system without rewriting the operator shell.

## Current invariants

The implemented ACP baseline currently guarantees:

1. core execution code depends on `RuntimeTransportProtocol` as the canonical ACP transport boundary,
2. runtime profile choice is frozen into run-start binding truth,
3. operator/report/recovery surfaces read persisted Forge truth rather than adapter-local heuristics,
4. unknown adapter families fail closed,
5. repo-owned catalog data can target Claude, Gemini, Codex, Auggie, and Junie ACP families,
6. Junie has a retained structured-output canary gate that validates native structured-output capability and the ACP `code_writer` settlement/materialization handoff on current proof-critical files.
7. Junie `code_writer` preflight rows do not consume provider capacity until the post-preflight launch lease is persisted.

## Current implementation owners

- `Chainworks Forge/Engine/RuntimeTransport.swift`
- `Chainworks Forge/Providers/BackendProfileResolverV2.swift`
- `Chainworks Forge/Engine/ExecutionService.swift`
- `Chainworks Forge/Engine/RuntimeSessionBridge.swift`
- `Chainworks Forge/Engine/RuntimeAgentExecutor.swift`
- `control-plane/crates/acp/src/manager.rs`
- `control-plane/crates/acp/src/transport.rs`
- `control-plane/crates/acp/src/adapters/claude.rs`
- `control-plane/crates/acp/src/adapters/gemini.rs`
- `control-plane/crates/acp/src/adapters/codex.rs`
- `control-plane/crates/acp/src/adapters/auggie.rs`
- `control-plane/crates/acp/src/adapters/junie.rs`
- `Chainworks Forge/Models/AgentExecution.swift`
- `Chainworks Forge/Engine/RunReportBuilder.swift`
- `Chainworks Forge/Engine/RunComparisonService.swift`

## Verification baseline

Current stable verification for this slice is:

- dedicated Rust ACP adapter and transport regression coverage on the current tree
- retained `proposal-089|p089` evidence validation for Junie native structured-output capability and ACP `code_writer` canary proof
- retained `proposal-090|p090` evidence validation for Junie `code_writer` runtime preflight, launch gating, and completion-boundary hardening
- current focused verification summary `71/71` passed
- capability verification includes both canonical ACP-backed proof flows:
  - proposal loop
  - implementation path to manual release gate
- same-tree approved-host `full` green basis:
  - `full-20260408-101540.xcresult`
