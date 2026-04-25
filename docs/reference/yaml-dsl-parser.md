# YAML DSL Parser and Validator

This document describes the YAML parsing, validation, and provenance hashing layers of Chainworks Forge. All types live in `Chainworks Forge/DSL/`.

## Overview

The DSL layer reads YAML definition files and converts them into typed Swift structures. It handles three YAML formats:

| Format | File | Parsed Into | Executable |
|---|---|---|---|
| Agent catalog | `agents.yaml` | `AgentCatalog` | Yes |
| Full workflow | `workflow.yaml` | `WorkflowDefinition` | Yes |
| Compact workflow | `proposal-to-release.yaml` | `CompactWorkflowDefinition` | No (inspector-only) |

## Dependencies

- **[Yams](https://github.com/jpsim/Yams)** (>= 5.0.0) — YAML 1.2 parser with `Codable` support, added via SPM.
- **CryptoKit** (Apple framework) — SHA-256 hashing for provenance.

## YAMLParser

**File:** `DSL/YAMLParser.swift`

Three static methods load YAML files from disk and decode them into typed structures:

```swift
struct YAMLParser {
    static func loadAgentCatalog(from url: URL) throws -> AgentCatalog
    static func loadWorkflow(from url: URL) throws -> WorkflowDefinition
    static func loadCompactWorkflow(from url: URL) throws -> CompactWorkflowDefinition
}
```

### Error handling

```swift
enum YAMLParserError: Error, LocalizedError {
    case fileNotFound(String)        // URL does not exist or is not readable
    case decodingFailed(String, Error)  // YAML is invalid or does not match the Codable schema
}
```

Parser errors cover file access and YAML decoding **only**. Semantic validation is a separate stage that returns `[ValidationIssue]`.

### snake_case to camelCase mapping

Canonical YAML files use `snake_case` keys (`schema_version`, `backend_profile`, `required_providers`). Swift structs use `camelCase` properties. Since Yams' `YAMLDecoder` does not have a built-in `keyDecodingStrategy`, **all Codable structures use explicit `CodingKeys`** for the mapping.

Types with only single-word keys (e.g., `RunBlock`, `AgentTask`, `Transition`, `LoopConfig`, `SkillRef`) omit `CodingKeys` because no mapping is needed.

## Parsed structures

These are in-memory `Codable` value types. They are **not** persisted in SwiftData. They are loaded from YAML at startup and when the user selects files in the UI.

### Agent Catalog

**File:** `DSL/AgentCatalog.swift`

Top-level structure parsed from `agents.yaml`.

```
AgentCatalog
├── schemaVersion: Int
├── app: AppConfig
├── paths: [String: String]
├── artifacts: [String: String]
├── skills: [String: SkillRef]
├── contracts: [String: ArtifactContract]
├── backendProfiles: [String: BackendProfile]
├── permissionProfiles: [String: PermissionProfile]
└── agents: [AgentDefinition]
```

#### `AgentDefinition`

Each agent in the catalog has:

| Field | YAML Key | Type |
|---|---|---|
| `id` | `id` | `String` |
| `title` | `title` | `String` |
| `mode` | `mode` | `String` |
| `backendProfile` | `backend_profile` | `String` |
| `permissionProfile` | `permission_profile` | `String` |
| `skillRef` | `skill_ref` | `String` |
| `skillRole` | `skill_role` | `String?` |
| `worktreePolicy` | `worktree_policy` | `WorktreePolicy?` |
| `requiredTools` | `required_tools` | `[String]?` |
| `inputs` | `inputs` | `[String]` |
| `outputs` | `outputs` | `[String]` |
| `outputContract` | `output_contract` | `String?` |
| `requiresHumanApproval` | `requires_human_approval` | `Bool` |
| `prompt` | `prompt` | `String` |
| `notes` | `notes` | `String?` |

#### `BackendProfile`

| Field | YAML Key | Type |
|---|---|---|
| `provider` | `provider` | `String` |
| `model` | `model` | `String` |
| `effort` | `effort` | `String` |
| `temperature` | `temperature` | `Double` |
| `maxTurns` | `max_turns` | `Int` |
| `structuredOutput` | `structured_output` | `String` |

#### `PermissionProfile`

Contains five permission sub-objects with single-word keys:

- `filesystem: FilesystemPermissions` — `read`, `write`, `deny` (all `[String]?`)
- `git: GitPermissions` — `status`, `diff`, `checkout`, `commit`, `push` (all `Bool?`)
- `shell: ShellPermissions` — `allow`, `deny` (both `[String]?`)
- `network: NetworkPermissions` — `allow: [String]?`
- `mcp: MCPPermissions` — `allow: [String]?`

#### Supporting types

- `AppConfig` — top-level app configuration (`name`, `runtime`, `transport`, `ideaInputMode`, `singleActiveRunPerIdea`, `runResumePolicy`, `requiredProviders`)
- `ArtifactContract` — `format: String`, `requiredFields: [String]`
- `SkillRef` — `type: String`, `path: String?`, `name: String?`, `description: String?`
- `WorktreePolicy` — `strategy: String`, `path: String`, `baseBranch: String?`, `writeEnabled: Bool`

### Full Workflow

**File:** `DSL/WorkflowDefinition.swift`

Top-level structure parsed from `workflow.yaml`. Describes an explicit state machine.

```
WorkflowDefinition
├── schemaVersion: Int
├── workflow: WorkflowMeta
├── discovery: DiscoveryConfig?
├── variables: [String: AnyCodableValue]?
├── failurePolicy: FailurePolicy?
├── scoring: ScoringConfig?
├── initialState: String
└── states: [String: WorkflowState]
```

#### `DiscoveryConfig`

| Field | YAML Key | Type | Notes |
|---|---|---|---|
| `legacyBroadDiscoveryPolicy` | `legacy_broad_discovery_policy` | `String?` | `disabled` (default) or `workflow_opt_in` |

#### `WorkflowState`

| Field | YAML Key | Type |
|---|---|---|
| `label` | `label` | `String` |
| `type` | `type` | `String?` (`start`, `end`, `manual_gate`, or `nil`) |
| `owner` | `owner` | `String` |
| `approval` | `approval` | `String?` (`"required"` or `nil`) |
| `run` | `run` | `RunBlock?` |
| `runAfterApproval` | `run_after_approval` | `RunBlock?` |
| `loop` | `loop` | `LoopConfig?` |
| `transitions` | `transitions` | `[Transition]?` |

#### `RunBlock`

Defines which agents execute within a state:

- `sequence: [AgentTask]?` — agents run one after another
- `parallel: [AgentTask]?` — agents run concurrently (fan-out)
- `then: [AgentTask]?` — sequential tasks after parallel fan-out completes

#### `AgentTask`

- `agent: String` — agent ID from the catalog
- `task: String` — task description
- `inputs: [String]?` — input artifact references
- `outputs: [String]?` — output artifact references
- `output_policies: [String: OutputPolicyDefinition]?` — per-output settlement and reuse policies (P057/P058)

#### `OutputPolicyDefinition`

- `reuse_policy: String?` — `must_produce` or `allow_unchanged_existing`

#### `Transition`

- `to: String` — target state ID
- `when: String` — expression string evaluated by the workflow engine

#### Supporting types

- `WorkflowMeta` — workflow identity (`id`, `name`, `description`, `usesAgentCatalog`, `ideaInput`, `execution`, `requiredProviders`)
- `ExecutionConfig` — `singleActiveRunPerIdea: Bool`, `resumePolicy: String`
- `IdeaInputConfig` — `mode: String`
- `LoopConfig` — `counter: String`, `max: String`
- `FailurePolicy` — `onError`, `onLoopBudgetExhausted`, `preserveArtifacts`
- `ScoringConfig` — optional `proposal: ProposalScoring?`, `implementation: ImplementationScoring?`
- `ProposalScoring` — `aggregateFormula: String?`, `passWhen: [String]?`
- `ImplementationScoring` — `implementedWhen: [String]?`

#### `AnyCodableValue`

Type-erased `Codable` enum for heterogeneous YAML maps (e.g. workflow `variables`). Supports: `string`, `int`, `double`, `bool`, `array`, `dictionary`, `null`. Custom `init(from:)` and `encode(to:)` probe each JSON type in sequence.

### Compact Workflow (Inspector-Only)

**File:** `DSL/CompactWorkflowDefinition.swift`

A simplified pipeline format using `stages` / `needs` / `gate` instead of explicit state machines. This format is **not executable** by the workflow engine; it is displayed in the Workflow Inspector as a structural preview.

```
CompactWorkflowDefinition
├── version: Int
└── workflow: CompactWorkflowMeta
    ├── id: String
    ├── title: String
    ├── execution: ExecutionConfig
    ├── requiredProviders: [String]
    └── stages: [CompactStage]
```

#### `CompactStage`

| Field | Type | Notes |
|---|---|---|
| `id` | `String` | Stage identifier |
| `type` | `String` | `single`, `fanout`, or `approval` |
| `agent` | `String?` | For `single` type |
| `agents` | `[String]?` | For `fanout` type |
| `approval` | `String?` | `"required"` for approval stages |
| `needs` | `[String]?` | Dependencies on other stages |
| `gate` | `CompactGate?` | Gate requirements |

> **Important:** Compact agent IDs are hyphenated aliases (e.g. `proposal-writer`, `security-checker`) that do **not** match canonical catalog IDs (e.g. `proposal_writer`, `security_checker`). Compact workflows are not validated against the agent catalog. Alias resolution is deferred to a future implementation.

## Validation

### Full validation: `YAMLValidator`

**File:** `DSL/YAMLValidator.swift`

Performs cross-reference validation between a `WorkflowDefinition` and an `AgentCatalog`. Entry point:

```swift
static func validateAll(workflow: WorkflowDefinition, catalog: AgentCatalog) -> [ValidationIssue]
```

This calls all ten validation categories:

| # | Check | Method | Severity |
|---|---|---|---|
| 1 | `initial_state` exists in `states` | `validateStateGraph` | Error |
| 2 | All transitions point to existing states | `validateStateGraph` | Error |
| 3 | At least one state with `type: end` | `validateStateGraph` | Warning |
| 4 | No orphan states (unreachable from `initial_state`) | `validateStateGraph` | Warning |
| 5 | All agents in run blocks exist in catalog | `validateAgentReferences` | Error |
| 6 | All state owners exist in catalog | `validateAgentReferences` | Error |
| 7 | Backend profile references are valid | `validateBackendProfileRefs` | Error |
| 8 | Permission profile references are valid | `validatePermissionProfileRefs` | Error |
| 9 | Skill references are valid | `validateSkillRefs` | Error |
| 10 | Output contract references are valid | `validateOutputContractRefs` | Error |
| 11 | Artifact input/output references exist | `validateArtifactRefs` | Warning |
| 12 | Required providers are covered by backend profiles | `validateProviderCoverage` | Error |
| 13 | Environment placeholders are well-formed | `validateEnvPlaceholders` | Warning/Error |
| 14 | Run block semantics are valid | `validateRunBlockSemantics` | Warning/Error |

#### Run block semantics checks

- Empty `sequence` blocks produce a warning.
- Empty `parallel` blocks produce an error.
- An agent appearing in both `parallel` and `then` produces a warning.

#### Environment placeholder checks

- `${VAR:-default}` syntax is valid. A placeholder without a default value produces a warning.
- An unclosed `${` produces an error.

### Compact validation: `CompactWorkflowValidator`

**File:** `DSL/CompactWorkflowValidator.swift`

Structural-only validation for compact workflows. No cross-catalog checks.

```swift
static func validate(_ compact: CompactWorkflowDefinition) -> [ValidationIssue]
```

| Check | Severity |
|---|---|
| Stage IDs are unique | Error |
| `needs` reference existing stage IDs | Error |
| Fanout stages have non-empty `agents` list | Error |
| Approval stages have `approval: required` | Warning |
| At least one stage has no `needs` (entry point) | Error |
| No circular `needs` chains | Error |

### `ValidationIssue`

```swift
struct ValidationIssue: Identifiable {
    let id: UUID
    let severity: Severity     // .error or .warning
    let message: String
    let location: String?      // e.g. "agents[2].backend_profile"
}
```

Errors block workflow loading. Warnings are displayed in the UI but do not block.

## Provenance hashing: `DefinitionHasher`

**File:** `DSL/DefinitionHasher.swift`

Provides deterministic JSON serialization and SHA-256 hashing for `Run` provenance snapshots.

```swift
struct DefinitionHasher {
    static let canonicalEncoder: JSONEncoder  // .sortedKeys, .withoutEscapingSlashes, .iso8601

    static func hash<T: Encodable>(_ value: T) throws -> (data: Data, sha256: String)
}
```

### Why `.sortedKeys` is mandatory

Types like `WorkflowDefinition.states`, `variables`, `backendProfiles`, and `permissionProfiles` are `[String: T]` dictionaries. Without `.sortedKeys`, dictionary iteration order varies between runs, producing different JSON bytes and triggering false drift alerts. `.sortedKeys` forces lexicographic key ordering, making serialization deterministic.

## Verification scaffold UI

The app replaces the Xcode template with a three-tab verification scaffold:

### Tab 1: Ideas

- SwiftData CRUD: create and delete ideas (title + body + optional attachment path)
- Empty state: "No ideas yet. Create your first idea."
- Summary strip with idea counts
- No workflow actions — persistence verification only

### Tab 2: Agent Catalog

- Loads `agents.yaml` and displays parsed `AgentCatalog`
- Agent list with drill-down: identity, backend profile, permissions, skill, inputs/outputs, prompt
- Summary strip: agent count, backend count, permission count, error/warning count
- Validation issues section when issues exist
- Read-only inspection

### Tab 3: Workflow Inspector

Two sub-views toggled by a segmented picker:

**Full Workflow** (default):
- Loads `workflow.yaml` and displays parsed `WorkflowDefinition`
- State list with drill-down: ID, label, type, owner, approval, run blocks, transitions, loops
- Summary strip: state count, gate count, loop count, error/warning count
- Full cross-catalog validation when catalog is available

**Compact Preview**:
- Loads `proposal-to-release.yaml` and displays parsed `CompactWorkflowDefinition`
- Stage list: ID, type, agent/agents, needs, gate
- Orange banner: "Compact format — preview only, not executable"
- Structural-only validation (no cross-catalog checks)

### `LoadState<T>`

All YAML-backed views use a shared state model:

```swift
enum LoadState<T: Sendable> {
    case loading                          // YAML file being read/decoded
    case loaded(T, [ValidationIssue])     // Parse OK; issues may be empty
    case fileNotFound(String)             // URL does not exist
    case decodeError(String, Error)       // YAML invalid or schema mismatch
}
```

Each state renders distinct UI with appropriate recovery actions (Reload, Open File...).

## File structure

```
Chainworks Forge/
  Models/
    Idea.swift
    Run.swift
    RunRepository.swift
    StageExecution.swift
    AgentExecution.swift
    Approval.swift
    Artifact.swift

  DSL/
    AgentCatalog.swift
    WorkflowDefinition.swift
    CompactWorkflowDefinition.swift
    CompactWorkflowValidator.swift
    YAMLParser.swift
    YAMLValidator.swift
    DefinitionHasher.swift

  Views/
    ContentView.swift
    IdeaListView.swift
    AgentCatalogView.swift
    WorkflowInspectorView.swift

  Chainworks_ForgeApp.swift
```
