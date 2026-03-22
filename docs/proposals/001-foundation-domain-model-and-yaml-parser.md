# Proposal 001: Foundation — Domain Model + YAML DSL Parser

**Date:** 2026-03-22
**Status:** Draft
**Priority:** P0 — prerequisite for all subsequent work
**Estimated effort:** 5–6 дней

---

## 1. Контекст

Chainworks Forge — macOS SwiftUI приложение, local control plane для агентных инженерных воркфлоу. Продуктовая модель, YAML DSL, архитектура и каталог агентов полностью специфицированы:

- 13 специализированных агентов с промптами, правами, backend-профилями
- 12-stage workflow state machine с approval gates и loops
- 11 artifact-контрактов (JSON-схемы)
- 8 permission profiles
- 11 backend profiles

Всё это определено в YAML-файлах и документации. **Код приложения = шаблон Xcode.** Ни одной строки продуктовой логики.

Этот proposal определяет **первый шаг реализации**: фундамент, без которого невозможно построить ни UI, ни runtime, ни оркестрацию.

---

## 2. Что строим

Два слоя, которые являются prerequisites для всего остального:

### Слой A: SwiftData Domain Model

Persistent модели, которые описывают всё, что приложение хранит и показывает.

### Слой B: YAML DSL Parser

Парсер, который читает `agents.yaml` и `workflow.yaml` и превращает их в типизированные Swift-структуры, пригодные для компиляции в runtime-объекты.

---

## 3. Domain Model (SwiftData)

### 3.1 Модели

```
┌──────────┐     1    ┌──────────┐     *    ┌──────────────┐
│   Idea   │────────▶│   Run    │────────▶│    Stage     │
│          │         │          │         │  Execution   │
└──────────┘         └────┬─────┘         └──────┬───────┘
                          │                      │
                          │ *                    │ *
                     ┌────▼─────┐          ┌─────▼───────┐
                     │ Approval │          │   Agent     │
                     │          │          │  Execution  │
                     └──────────┘          └─────┬───────┘
                                                 │ *
                                           ┌─────▼───────┐
                                           │  Artifact   │
                                           └─────────────┘
```

#### `Idea`
```swift
@Model final class Idea {
    @Attribute(.unique) var id: UUID
    var title: String              // краткое описание от пользователя
    var body: String               // полный текст идеи
    var attachmentPath: String?    // опциональный путь к файлу
    var createdAt: Date
    var status: IdeaStatus         // draft | active | completed | failed

    @Relationship(deleteRule: .cascade)
    var runs: [Run]
}

enum IdeaStatus: String, Codable {
    case draft, active, completed, failed
}
```

> **Design note (review finding P1):** `Idea` intentionally does NOT carry `workflowID`.
> Workflow identity lives on `Run` because the schema allows many runs per idea,
> and each run must capture the exact workflow+catalog revision it was started with.
> This makes reruns unambiguous and resume safe even when YAML changes between runs.

#### `Run`
```swift
@Model final class Run {
    @Attribute(.unique) var id: UUID
    var startedAt: Date
    var completedAt: Date?
    var status: RunStatus
    var currentStageID: String     // id текущего state из workflow
    var loopCounters: [String: Int]  // proposal_revision_cycles: 2, etc.

    // ARCH-005: cost stored as integer minor units (cents), not Double
    var totalCostCents: Int64?     // $12.34 → 1234; avoids floating-point drift

    // --- Workflow provenance (immutable after run creation) ---
    var workflowID: String         // id workflow из каталога
    var workflowTitle: String      // human-readable title на момент создания run
    var workflowSnapshotHash: String  // SHA-256 of serialized WorkflowDefinition
    var catalogSnapshotHash: String   // SHA-256 of serialized AgentCatalog
    var workflowSourcePath: String    // путь к workflow.yaml, использованному при создании
    var catalogSourcePath: String     // путь к agents.yaml, использованному при создании

    // ARCH-004: full serialized snapshots for safe resume after YAML drift
    var workflowSnapshotJSON: Data    // полный WorkflowDefinition, сериализованный в JSON
    var catalogSnapshotJSON: Data     // полный AgentCatalog, сериализованный в JSON

    // ARCH-003: drift detection metadata
    var driftDetectedAt: Date?        // когда обнаружен drift (nil = no drift)
    var driftDetails: String?         // human-readable описание что изменилось
    var driftDecision: DriftDecision? // решение инженера по drift

    @Relationship(inverse: \Idea.runs)
    var idea: Idea?

    @Relationship(deleteRule: .cascade)
    var stageExecutions: [StageExecution]

    @Relationship(deleteRule: .cascade)
    var approvals: [Approval]
}

enum RunStatus: String, Codable {
    case running              // workflow actively executing
    case pausedAtGate         // waiting for human approval
    case driftDetected        // ARCH-003: YAML changed since run started; needs decision
    case completed
    case failed
}

/// ARCH-003: engineer's explicit decision when workflow drift is detected
enum DriftDecision: String, Codable {
    case continueWithOriginal   // resume using snapshotted workflow/catalog
    case restartWithCurrent     // abandon this run, start new run with current YAML
    case cancelled              // stop the run entirely
}
```

> **Provenance contract (updated per ARCH-003 + ARCH-004):**
>
> При создании Run:
> 1. Orchestrator сериализует `WorkflowDefinition` и `AgentCatalog` в **canonical JSON**
>    using `DefinitionHasher.canonicalEncoder` (see below)
> 2. Записывает полные snapshots в `workflowSnapshotJSON` / `catalogSnapshotJSON`
> 3. Вычисляет SHA-256 хеш каждого snapshot → `workflowSnapshotHash` / `catalogSnapshotHash`
>
> **Canonical serialization rule (addresses false-drift risk):**
>
> ```swift
> struct DefinitionHasher {
>     /// The ONLY encoder used for provenance snapshots.
>     /// Settings guarantee deterministic output for dictionary-heavy types
>     /// (states, variables, backendProfiles, permissionProfiles, etc.).
>     static let canonicalEncoder: JSONEncoder = {
>         let encoder = JSONEncoder()
>         encoder.outputFormatting = [.sortedKeys, .withoutEscapingSlashes]
>         encoder.dateEncodingStrategy = .iso8601
>         encoder.dataEncodingStrategy = .base64
>         return encoder
>     }()
>
>     /// Serialize + SHA-256 in one call.
>     static func hash<T: Encodable>(_ value: T) throws -> (data: Data, sha256: String) {
>         let data = try canonicalEncoder.encode(value)
>         let digest = SHA256.hash(data: data)
>         let hex = digest.map { String(format: "%02x", $0) }.joined()
>         return (data, hex)
>     }
> }
> ```
>
> Why `.sortedKeys` is mandatory:
> - `WorkflowDefinition.states` is `[String: WorkflowState]` — without sorted keys,
>   dictionary iteration order varies between runs, producing different JSON and false drift.
> - Same applies to `variables`, `backendProfiles`, `permissionProfiles`, `paths`, `artifacts`, etc.
> - `.sortedKeys` makes `JSONEncoder` emit keys in lexicographic order, deterministically.
>
> **Test:** `testDefinitionHashDeterministic` — encode same object 100 times → all hashes identical.
> **Test:** `testDefinitionHashStableAcrossAppLaunches` — encode, write to disk, relaunch, re-encode → same hash.
>
> При resume после перезапуска:
> 1. Orchestrator загружает текущие YAML-файлы и вычисляет их хеши
> 2. Сравнивает с сохранёнными хешами
> 3. Если хеши совпадают → resume нормально
> 4. Если хеши расходятся → `status = .driftDetected`, заполняется `driftDetails`
> 5. Инженер видит drift-review UI (UX-01) и выбирает `DriftDecision`
> 6. При `continueWithOriginal` — orchestrator десериализует workflow/catalog из snapshot
> 7. При `restartWithCurrent` — создаётся новый Run с текущими YAML
>
> Почему snapshot, а не только hash:
> - Hash обнаруживает drift, но не позволяет продолжить с оригиналом (ARCH-004)
> - Snapshot хранится как `Data` (JSON blob), не усложняя SwiftData schema
> - При десериализации snapshot'а используется `JSONDecoder` (snapshot хранится как JSON, не YAML)

#### Single Active Run Invariant

> **ARCH-002 (P0):** MVP требует `single_active_run_per_idea: true`, но модель
> выставляет unconstrained `Idea.runs` коллекцию. Без архитектурной защиты
> возможны concurrent runs для одной идеи.

```swift
/// Atomic run creation — check + insert in one serialized operation.
///
/// SwiftData ModelContext is not thread-safe, but it is @MainActor-isolated.
/// By combining the active-run check and the insert into a single @MainActor
/// method, we eliminate the TOCTOU window: no second caller can interleave
/// between the check and the insert because both run on the same serial
/// executor (MainActor).
@MainActor
struct RunGuard {

    /// Atomically checks that no active run exists for the idea, then inserts
    /// the new run into the context. Returns the inserted Run.
    ///
    /// Active run = status in [.running, .pausedAtGate, .driftDetected]
    ///
    /// This is the ONLY approved way to create a Run. Direct ModelContext.insert
    /// for Run objects is a contract violation.
    ///
    /// Throws RunGuardError.activeRunExists if an active run already exists.
    static func createRun(
        for idea: Idea,
        workflow: WorkflowDefinition,
        catalog: AgentCatalog,
        workflowSourcePath: String,
        catalogSourcePath: String,
        in context: ModelContext
    ) throws -> Run

    /// Returns the current active run for the idea, or nil.
    static func activeRun(for idea: Idea) -> Run?
}

enum RunGuardError: Error, LocalizedError {
    case activeRunExists(runID: UUID, status: RunStatus)
}
```

**Why this is safe:**

1. `ModelContext` is `@MainActor`-isolated in SwiftData.
2. `RunGuard` is `@MainActor`, so `createRun` executes on the main serial executor.
3. Check + insert happen in the same synchronous block — no suspension point between them.
4. No other code path may insert a `Run` directly; `RunGuard.createRun` is the single entry point (enforced by code review + test coverage, not by access control).

This is strictly stronger than a separate `ensureNoActiveRun` + `insert` pattern, because there is no gap between check and mutation where a concurrent caller could interleave.

**Тесты:**

```swift
// Sequential: second call to createRun with same idea → RunGuardError
func testSequentialRunCreationBlocked()

// Parallel: two Tasks both calling createRun for the same idea.
// Because both are @MainActor, they serialize. Exactly one succeeds,
// the other throws activeRunExists.
func testParallelRunCreationSerializes() async {
    let idea = makeIdea()
    async let run1 = RunGuard.createRun(for: idea, ...)
    async let run2 = RunGuard.createRun(for: idea, ...)
    let results = await [Result { try await run1 }, Result { try await run2 }]
    XCTAssertEqual(results.filter(\.isSuccess).count, 1)
    XCTAssertEqual(results.filter(\.isFailure).count, 1)
}

// After completion: new run allowed
func testRunGuardAllowsAfterCompletion()
```

#### `StageExecution`
```swift
@Model final class StageExecution {
    @Attribute(.unique) var id: UUID
    var stageID: String            // id state из workflow YAML
    var label: String              // human-readable label
    var startedAt: Date
    var completedAt: Date?
    var status: StageStatus        // pending | running | completed | failed | skipped
    var iteration: Int             // номер итерации в loop (1-based)

    @Relationship(inverse: \Run.stageExecutions)
    var run: Run?

    @Relationship(deleteRule: .cascade)
    var agentExecutions: [AgentExecution]
}

enum StageStatus: String, Codable {
    case pending, running, completed, failed, skipped
}
```

#### `AgentExecution`
```swift
@Model final class AgentExecution {
    @Attribute(.unique) var id: UUID
    var agentID: String            // id агента из каталога
    var agentTitle: String
    var taskName: String           // task из workflow YAML
    var startedAt: Date
    var completedAt: Date?
    var status: AgentStatus        // queued | running | completed | failed
    var provider: String           // claude_code | codex | gemini
    var effort: String             // low | medium | high | critical
    var costCents: Int64?          // ARCH-005: integer minor units; $0.73 → 73
    var logSnippet: String?        // последние N строк лога для быстрого просмотра

    // Goose session tracking
    var gooseSessionID: String?

    @Relationship(inverse: \StageExecution.agentExecutions)
    var stageExecution: StageExecution?

    @Relationship(deleteRule: .cascade)
    var artifacts: [Artifact]
}

enum AgentStatus: String, Codable {
    case queued, running, completed, failed
}
```

#### `Approval`
```swift
@Model final class Approval {
    @Attribute(.unique) var id: UUID
    var stageID: String            // stage, на котором запрошен approval
    var requestedAt: Date
    var decidedAt: Date?
    var decision: ApprovalDecision // pending | approved | rejected
    var comment: String?           // опциональный комментарий инженера

    @Relationship(inverse: \Run.approvals)
    var run: Run?
}

enum ApprovalDecision: String, Codable {
    case pending, approved, rejected
}
```

#### `Artifact`
```swift
@Model final class Artifact {
    @Attribute(.unique) var id: UUID
    var name: String               // proposal_review_po, audit_report, etc.
    var contractID: String         // proposal_review_v1, audit_report_v1
    var format: ArtifactFormat     // json | markdown
    var filePath: String           // путь к файлу на диске
    var createdAt: Date
    var sizeBytes: Int64?

    @Relationship(inverse: \AgentExecution.artifacts)
    var agentExecution: AgentExecution?
}

enum ArtifactFormat: String, Codable {
    case json, markdown
}
```

### 3.2 Почему именно эти модели

Каждая модель отражает ключевую сущность из MVP PS:

| PS Requirement | Model |
|---|---|
| "describe an idea in text, optionally attach a file" | `Idea` |
| "one execution instance of a workflow for one idea" | `Run` |
| "which workflow definition was used for this run" | `Run.workflowID` + `Run.workflowSnapshotHash` + `Run.catalogSnapshotHash` |
| "show the workflow chain and current stage" | `StageExecution` |
| "show active agents and let the engineer inspect" | `AgentExecution` |
| "pause at workflow-defined approval gates" | `Approval` |
| "readable report with summary, time, and cost" | `Run.totalCostCents` + `Artifact` (final_feature_report) |
| "durable artifacts" | `Artifact` |
| "resume interrupted runs on app launch" | `Run.status` + `Run.currentStageID` + provenance hash comparison |

---

## 4. YAML DSL Parser

### 4.1 Что парсим

Два типа YAML-файлов с тремя форматами, уже определённых в `examples/`:

1. **Agent Catalog** (`agents.yaml`) — определения агентов, backend profiles, permission profiles, artifact contracts, paths, skills
2. **Full Workflow** (`workflow.yaml`) — state machine со states, transitions, run blocks, approval gates, loops, variables, scoring
3. **Compact Workflow** (`proposal-to-release.yaml`) — упрощённый формат со stages, needs, gate — **другая схема**, которая требует нормализации в `WorkflowDefinition`

> **Design note (review finding P1):** Full и compact workflow — это два разных YAML-формата.
> Compact не является подмножеством full. Compact описывает линейный pipeline со stages/needs/gate,
> а full описывает explicit state machine со states/transitions/run blocks.
> Парсер десериализует каждый в свой тип, затем `WorkflowNormalizer` преобразует compact → full.

### 4.2 Parsed structures (Codable, не SwiftData)

Эти структуры — **in-memory представление YAML DSL**. Они не персистятся в SwiftData. Они загружаются при старте и при выборе workflow.

```swift
// MARK: - Agent Catalog
//
// CodingKeys contract: EVERY type that appears in YAML with snake_case keys
// has explicit CodingKeys. Types with only single-word keys (id, title, mode,
// prompt, agent, task, type, path, etc.) omit CodingKeys because no mapping
// is needed. This contract is verified by parser tests that decode the
// canonical examples/agents/agents.yaml and examples/workflows/workflow.yaml.

struct AgentCatalog: Codable {
    let schemaVersion: Int
    let app: AppConfig
    let paths: [String: String]
    let artifacts: [String: String]
    let skills: [String: SkillRef]
    let contracts: [String: ArtifactContract]
    let backendProfiles: [String: BackendProfile]
    let permissionProfiles: [String: PermissionProfile]
    let agents: [AgentDefinition]

    enum CodingKeys: String, CodingKey {
        case schemaVersion = "schema_version"
        case app, paths, artifacts, skills, contracts, agents
        case backendProfiles = "backend_profiles"
        case permissionProfiles = "permission_profiles"
    }
}

struct AppConfig: Codable {
    let name: String
    let runtime: String
    let transport: String
    let description: String
    let ideaInputMode: String
    let singleActiveRunPerIdea: Bool
    let runResumePolicy: String
    let requiredProviders: [String]

    enum CodingKeys: String, CodingKey {
        case name, runtime, transport, description
        case ideaInputMode = "idea_input_mode"
        case singleActiveRunPerIdea = "single_active_run_per_idea"
        case runResumePolicy = "run_resume_policy"
        case requiredProviders = "required_providers"
    }
}

struct AgentDefinition: Codable, Identifiable {
    let id: String
    let title: String
    let mode: String
    let backendProfile: String
    let permissionProfile: String
    let skillRef: String
    let skillRole: String?
    let worktreePolicy: WorktreePolicy?
    let requiredTools: [String]?
    let inputs: [String]
    let outputs: [String]
    let outputContract: String?
    let requiresHumanApproval: Bool
    let prompt: String
    let notes: String?

    enum CodingKeys: String, CodingKey {
        case id, title, mode, prompt, notes, inputs, outputs
        case backendProfile = "backend_profile"
        case permissionProfile = "permission_profile"
        case skillRef = "skill_ref"
        case skillRole = "skill_role"
        case worktreePolicy = "worktree_policy"
        case requiredTools = "required_tools"
        case outputContract = "output_contract"
        case requiresHumanApproval = "requires_human_approval"
    }
}

struct BackendProfile: Codable {
    let provider: String
    let model: String
    let effort: String
    let temperature: Double
    let maxTurns: Int
    let structuredOutput: String

    enum CodingKeys: String, CodingKey {
        case provider, model, effort, temperature
        case maxTurns = "max_turns"
        case structuredOutput = "structured_output"
    }
}

struct PermissionProfile: Codable {
    let filesystem: FilesystemPermissions
    let git: GitPermissions
    let shell: ShellPermissions
    let network: NetworkPermissions
    let mcp: MCPPermissions
    // all single-word keys — no CodingKeys needed
}

struct ArtifactContract: Codable {
    let format: String
    let requiredFields: [String]

    enum CodingKeys: String, CodingKey {
        case format
        case requiredFields = "required_fields"
    }
}

struct SkillRef: Codable {
    let type: String
    let path: String?
    let name: String?
    let description: String?
    // all single-word keys — no CodingKeys needed
}

struct WorktreePolicy: Codable {
    let strategy: String     // dedicated | meta_only | shared_implementation_worktree
    let path: String
    let baseBranch: String?
    let writeEnabled: Bool

    enum CodingKeys: String, CodingKey {
        case strategy, path
        case baseBranch = "base_branch"
        case writeEnabled = "write_enabled"
    }
}

// MARK: - Workflow

struct WorkflowDefinition: Codable {
    let schemaVersion: Int
    let workflow: WorkflowMeta
    let variables: [String: AnyCodableValue]
    let failurePolicy: FailurePolicy
    let scoring: ScoringConfig
    let initialState: String
    let states: [String: WorkflowState]

    enum CodingKeys: String, CodingKey {
        case workflow, variables, scoring, states
        case schemaVersion = "schema_version"
        case failurePolicy = "failure_policy"
        case initialState = "initial_state"
    }
}

struct WorkflowMeta: Codable {
    let id: String
    let name: String
    let usesAgentCatalog: String?
    let description: String
    let ideaInput: IdeaInputConfig?
    let execution: ExecutionConfig
    let requiredProviders: [String]

    enum CodingKeys: String, CodingKey {
        case id, name, description, execution
        case usesAgentCatalog = "uses_agent_catalog"
        case ideaInput = "idea_input"
        case requiredProviders = "required_providers"
    }
}

struct WorkflowState: Codable {
    let label: String
    let type: String?          // start | end | manual_gate | nil (normal)
    let owner: String
    let approval: String?      // "required" | nil
    let run: RunBlock?
    let runAfterApproval: RunBlock?
    let loop: LoopConfig?
    let transitions: [Transition]

    enum CodingKeys: String, CodingKey {
        case label, type, owner, approval, run, loop, transitions
        case runAfterApproval = "run_after_approval"
    }
}

struct RunBlock: Codable {
    let sequence: [AgentTask]?
    let parallel: [AgentTask]?
    let then: [AgentTask]?     // sequential tasks after parallel fan-out
    // all single-word keys — no CodingKeys needed
}

struct AgentTask: Codable {
    let agent: String
    let task: String
    let inputs: [String]
    let outputs: [String]
    // all single-word keys — no CodingKeys needed
}

struct Transition: Codable {
    let to: String
    let when: String           // expression string — evaluated by engine
    // all single-word keys — no CodingKeys needed
}

struct LoopConfig: Codable {
    let counter: String
    let max: String            // "vars.max_proposal_revision_cycles"
    // all single-word keys — no CodingKeys needed
}

struct FailurePolicy: Codable {
    let onError: String
    let onLoopBudgetExhausted: String
    let preserveArtifacts: Bool

    enum CodingKeys: String, CodingKey {
        case onError = "on_error"
        case onLoopBudgetExhausted = "on_loop_budget_exhausted"
        case preserveArtifacts = "preserve_artifacts"
    }
}

// MARK: - Supporting types (referenced by main structs)

struct ExecutionConfig: Codable {
    let singleActiveRunPerIdea: Bool
    let resumePolicy: String

    enum CodingKeys: String, CodingKey {
        case singleActiveRunPerIdea = "single_active_run_per_idea"
        case resumePolicy = "resume_policy"
    }
}

struct IdeaInputConfig: Codable {
    let mode: String
}

struct ScoringConfig: Codable {
    let proposal: ProposalScoring?
    let implementation: ImplementationScoring?
}

struct ProposalScoring: Codable {
    let aggregateFormula: String?
    let passWhen: [String]?

    enum CodingKeys: String, CodingKey {
        case aggregateFormula = "aggregate_formula"
        case passWhen = "pass_when"
    }
}

struct ImplementationScoring: Codable {
    let implementedWhen: [String]?

    enum CodingKeys: String, CodingKey {
        case implementedWhen = "implemented_when"
    }
}

struct FilesystemPermissions: Codable {
    let read: [String]?
    let write: [String]?
    let deny: [String]?
}

struct GitPermissions: Codable {
    let status: Bool?
    let diff: Bool?
    let checkout: Bool?
    let commit: Bool?
    let push: Bool?
}

struct ShellPermissions: Codable {
    let allow: [String]?
    let deny: [String]?
}

struct NetworkPermissions: Codable {
    let allow: [String]?
}

struct MCPPermissions: Codable {
    let allow: [String]?
}

/// Type-erased Codable value for heterogeneous YAML maps (e.g. workflow variables).
/// Supports String, Int, Double, Bool, [AnyCodableValue], [String: AnyCodableValue].
/// Implementation: custom init(from:)/encode(to:) that probes each JSON type.
enum AnyCodableValue: Codable {
    case string(String)
    case int(Int)
    case double(Double)
    case bool(Bool)
    case array([AnyCodableValue])
    case dictionary([String: AnyCodableValue])
    case null
}
```

### 4.3 Архитектура парсера

```swift
// Services/YAMLParser.swift

enum YAMLParserError: Error, LocalizedError {
    case fileNotFound(String)
    case decodingFailed(String, Error)
    case validationFailed([String])
}

struct YAMLParser {
    /// Загрузить каталог агентов из файла
    static func loadAgentCatalog(from url: URL) throws -> AgentCatalog

    /// Загрузить full workflow (workflow.yaml — state machine format)
    static func loadWorkflow(from url: URL) throws -> WorkflowDefinition

    /// Загрузить compact workflow (proposal-to-release.yaml — stages/needs format)
    static func loadCompactWorkflow(from url: URL) throws -> CompactWorkflowDefinition
}

/// Compact workflow — отдельный тип, другая схема.
/// CodingKeys для snake_case ключей из proposal-to-release.yaml (ARCH-009).
struct CompactWorkflowDefinition: Codable {
    let version: Int
    let workflow: CompactWorkflowMeta
    // all single-word keys — no CodingKeys needed
}

struct CompactWorkflowMeta: Codable {
    let id: String
    let title: String
    let execution: ExecutionConfig
    let requiredProviders: [String]
    let stages: [CompactStage]

    enum CodingKeys: String, CodingKey {
        case id, title, execution, stages
        case requiredProviders = "required_providers"
    }
}

struct CompactStage: Codable {
    let id: String
    let type: String              // single | fanout | approval
    let agent: String?            // для single
    let agents: [String]?         // для fanout
    let approval: String?         // "required"
    let needs: [String]?
    let gate: CompactGate?
    // all single-word keys — no CodingKeys needed
}

struct CompactGate: Codable {
    let require: [String]
    // single-word key — no CodingKeys needed
}

/// Нормализатор: compact → full WorkflowDefinition
struct WorkflowNormalizer {
    /// Преобразует compact pipeline (stages + needs + gate)
    /// в полноценную state machine (states + transitions + run blocks).
    ///
    /// Правила нормализации:
    /// - каждый compact stage → один WorkflowState
    /// - needs → transitions (предыдущий stage переходит в текущий)
    /// - approval stages → type: manual_gate + approval: required
    /// - single → RunBlock с sequence из одного AgentTask
    /// - fanout → RunBlock с parallel из agents
    /// - gate.require → transition conditions
    /// - первый stage без needs → initial_state
    /// - добавляется терминальный state_complete
    ///
    /// Ограничения нормализации (v1):
    /// - loop counters из compact не выводятся (compact их не описывает)
    /// - scoring/variables/failurePolicy получают дефолтные значения
    /// - run blocks не содержат inputs/outputs (compact их не описывает)
    static func normalize(_ compact: CompactWorkflowDefinition) throws -> NormalizedWorkflow
}

/// Result of compact → full normalization, preserving annotation metadata.
struct NormalizedWorkflow {
    /// The full workflow definition (usable by engine)
    let definition: WorkflowDefinition

    /// Which fields were inferred/defaulted during normalization (for UI badges)
    let annotations: [NormalizationAnnotation]

    /// Was this produced by normalization (true) or loaded directly from full YAML (false)?
    let isNormalized: Bool
}

struct NormalizationAnnotation: Identifiable {
    let id: UUID
    let path: String        // e.g. "states.draft_initial_proposal.transitions[0].when"
    let kind: AnnotationKind
    let explanation: String  // e.g. "Inferred from needs: [draft_initial_proposal]"
}

enum AnnotationKind: String {
    case defaulted   // field absent in compact, filled with default value
    case inferred    // field derived from compact semantics (needs → transition)
    case synthesized // entire object created (terminal state, empty scoring)
}

struct YAMLValidator {

    /// Полная валидация: workflow + catalog cross-references.
    /// Вызывает все частные валидаторы и возвращает объединённый список issues.
    static func validateAll(
        workflow: WorkflowDefinition,
        catalog: AgentCatalog
    ) -> [ValidationIssue]

    // --- State graph integrity ---

    /// initial_state существует в states
    /// Все transitions.to ведут к реальным states
    /// Нет orphan states (unreachable из initial_state)
    /// Есть хотя бы один state с type: end
    static func validateStateGraph(_ workflow: WorkflowDefinition) -> [ValidationIssue]

    // --- Agent ↔ workflow cross-references ---

    /// Все agent ids в workflow run blocks ссылаются на агентов из catalog
    /// Все owner ids в states ссылаются на агентов из catalog
    static func validateAgentReferences(
        workflow: WorkflowDefinition,
        catalog: AgentCatalog
    ) -> [ValidationIssue]

    // --- Catalog internal consistency ---

    /// Каждый agent.backend_profile ссылается на существующий backend profile
    static func validateBackendProfileRefs(_ catalog: AgentCatalog) -> [ValidationIssue]

    /// Каждый agent.permission_profile ссылается на существующий permission profile
    static func validatePermissionProfileRefs(_ catalog: AgentCatalog) -> [ValidationIssue]

    /// Каждый agent.skill_ref ссылается на существующий skill в catalog.skills
    static func validateSkillRefs(_ catalog: AgentCatalog) -> [ValidationIssue]

    /// Каждый agent.output_contract ссылается на существующий contract в catalog.contracts
    static func validateOutputContractRefs(_ catalog: AgentCatalog) -> [ValidationIssue]

    /// Все artifact ids в agent inputs/outputs существуют в catalog.artifacts
    static func validateArtifactRefs(_ catalog: AgentCatalog) -> [ValidationIssue]

    // --- Provider coverage ---

    /// required_providers из workflow покрываются backend_profiles в catalog
    static func validateProviderCoverage(
        workflow: WorkflowDefinition,
        catalog: AgentCatalog
    ) -> [ValidationIssue]

    // --- Environment placeholders ---

    /// Все ${VAR:-default} в paths/artifact paths/worktree paths синтаксически корректны
    /// Warning (не error) если переменная не имеет дефолтного значения
    static func validateEnvPlaceholders(_ catalog: AgentCatalog) -> [ValidationIssue]

    // --- Run block semantics ---

    /// parallel + then: then agents не дублируют parallel agents
    /// sequence: нет пустых sequence блоков
    /// fanout stages: agents list не пустой
    static func validateRunBlockSemantics(_ workflow: WorkflowDefinition) -> [ValidationIssue]
}

struct ValidationIssue: Identifiable {
    let id: UUID
    let severity: Severity
    let message: String
    let location: String?      // e.g. "agents[2].backend_profile", "states.state_4.run.parallel[1]"

    enum Severity: String, Codable {
        case error              // блокирует загрузку workflow
        case warning            // не блокирует, но выводится в UI
    }
}
```

### 4.4 Decoding Strategy: snake_case YAML → camelCase Swift

> **ARCH-001 (P0):** Каноничные YAML-файлы используют `snake_case` ключи
> (`schema_version`, `backend_profile`, `required_providers`, `write_globs`, etc.),
> а Swift structs используют `camelCase` свойства. Без явной стратегии декодирования
> парсер **не сможет десериализовать реальные YAML-файлы**.

**Решение:** Yams `YAMLDecoder` не имеет встроенного `keyDecodingStrategy` как `JSONDecoder`.
Поэтому **все Codable-структуры используют explicit `CodingKeys`** для маппинга snake_case → camelCase.

```swift
import Yams

struct YAMLParser {
    static func loadAgentCatalog(from url: URL) throws -> AgentCatalog {
        let yamlString = try String(contentsOf: url, encoding: .utf8)
        return try YAMLDecoder().decode(AgentCatalog.self, from: yamlString)
    }

    static func loadWorkflow(from url: URL) throws -> WorkflowDefinition {
        let yamlString = try String(contentsOf: url, encoding: .utf8)
        return try YAMLDecoder().decode(WorkflowDefinition.self, from: yamlString)
    }

    static func loadCompactWorkflow(from url: URL) throws -> CompactWorkflowDefinition {
        let yamlString = try String(contentsOf: url, encoding: .utf8)
        return try YAMLDecoder().decode(CompactWorkflowDefinition.self, from: yamlString)
    }
}
```

**Каждая Codable-структура содержит explicit `CodingKeys`** для snake_case → camelCase маппинга:

```swift
struct AgentDefinition: Codable, Identifiable {
    let id: String
    let title: String
    let mode: String
    let backendProfile: String
    // ...

    enum CodingKeys: String, CodingKey {
        case id, title, mode, prompt, notes, inputs, outputs
        case backendProfile = "backend_profile"
        case permissionProfile = "permission_profile"
        case skillRef = "skill_ref"
        case skillRole = "skill_role"
        case worktreePolicy = "worktree_policy"
        case requiredTools = "required_tools"
        case outputContract = "output_contract"
        case requiresHumanApproval = "requires_human_approval"
    }
}
```

**Acceptance criteria (из ARCH-001):**
`agents.yaml`, `workflow.yaml`, и `proposal-to-release.yaml` декодируются
без препроцессинга, и все тесты валидатора работают на результате десериализации.

**Тест-proof:** каждый парсер-тест загружает реальный YAML из `examples/`, не синтетический.

### 4.5 Зависимость: Yams

Для YAML-парсинга используем [Yams](https://github.com/jpsim/Yams) — зрелую Swift-библиотеку с поддержкой YAML 1.2 и Codable.

```swift
// Package dependency (через SPM в Xcode project):
// https://github.com/jpsim/Yams.git, from: "5.0.0"
```

---

## 5. Что НЕ входит в scope

| Исключение | Почему |
|---|---|
| Goose REST/SSE adapter | Отдельный слой, зависит от парсера |
| Workflow state machine engine | Зависит от модели и парсера |
| UI (кроме минимального) | Зависит от модели |
| Worktree management | Runtime concern, не foundation |
| Artifact bus | Runtime concern |
| Approval gate UI | UI concern, зависит от модели |

---

## 6. Минимальный UI для верификации

> **Design note (review finding P2):** Scaffold ограничен поведением, которое
> этот proposal реально реализует. Никаких "Start Run" или workflow selection —
> engine и approval UI вне scope. Scaffold доказывает только:
> SwiftData CRUD работает, YAML парсится, валидация выдаёт результаты.

Для проверки работоспособности фундамента — **заменить шаблонный ContentView** на экран с тремя табами:

**Tab 1: Ideas (SwiftData CRUD)**
- Показывает список идей из SwiftData
- Позволяет создать новую идею (title + body + optional attachment path)
- Позволяет удалить идею
- Идея сохраняется в SwiftData — это всё, что происходит на этом уровне
- Никаких workflow actions — только persistence verification

**Tab 2: Agent Catalog (YAML parse result)**
- Загружает `agents.yaml` → показывает parsed `AgentCatalog`
- Список агентов с: id, title, backend profile name, permission profile name
- Drill-down в агента: prompt, skill_ref, inputs, outputs, output_contract
- Показывает validation issues (если есть)
- Read-only — это инспекция parsed YAML, не редактирование

**Tab 3: Workflow Inspector (YAML parse result)**
- Загружает `workflow.yaml` → показывает parsed `WorkflowDefinition`
- Список states с: id, label, type, owner
- Для каждого state: transitions, run block agents, approval requirement
- Показывает validation issues (от `YAMLValidator.validateAll`)
- **UX-02: Source vs normalized view.** Если загружен compact workflow, показать два режима:
  - **"Source (compact)"** — raw compact stages/needs/gate как пришли из YAML
  - **"Normalized (full)"** — результат `WorkflowNormalizer.normalize()`
  - Каждое дефолтное/синтезированное поле в normalized view помечается badge `[default]` или `[inferred]`
  - Пользователь никогда не перепутает source truth с нормализованным результатом
- Read-only инспекция

### 6.1 States для каждого таба (UI-002 / UX-03)

> **UX-03 + UI-002 (P1):** Scaffold должен показывать distinct UI для каждого типа сбоя.
> Без этого verification scaffold не сможет подтвердить парсер поведение при ошибках.

Каждый таб Agent Catalog и Workflow Inspector имеет четыре exclusive состояния:

| State | Trigger | UI |
|---|---|---|
| **Loading** | YAML файл читается/декодируется | Spinner + "Loading agents.yaml..." |
| **Loaded** | Парсинг и валидация завершены | Нормальный контент + validation summary strip |
| **File Not Found** | URL не существует или недоступен | Icon + "File not found at {path}" + **[Open File…]** кнопка для повторного выбора |
| **Decode Error** | YAML синтаксически невалиден или структура не совпадает с Codable | Icon + error message + raw excerpt проблемного фрагмента + **[Reload]** |
| **Validation Errors** | Парсинг OK, но validator нашёл errors (не warnings) | Контент показывается, но summary strip красный: "3 errors, 1 warning" + drill-down |

Tab Ideas имеет два состояния:
| State | Trigger | UI |
|---|---|---|
| **Empty** | Нет идей в SwiftData | "No ideas yet. Create your first idea." + prominent [New Idea] |
| **Has Ideas** | >= 1 идея | Список + detail |

### 6.2 Summary strip (UI-001)

> **UI-001 (P2):** Каждый таб начинается с summary strip — одна строка над контентом.

```
Tab Agent Catalog:
┌─────────────────────────────────────────────────┐
│ ✅ 13 agents · 11 backends · 8 permissions · 0 errors │  ← summary strip
├─────────────────────────────────────────────────┤

Tab Workflow Inspector:
┌─────────────────────────────────────────────────┐
│ 📋 12 states · 3 gates · 2 loops · ⚠ 1 warning      │  ← summary strip
├─────────────────────────────────────────────────┤
```

### 6.3 Drift-review UI contract (UX-01, out of scaffold scope)

> **UX-01 (P1):** Drift-review UI — это runtime concern, вне scope этого proposal.
> Но контракт для drift определяется здесь, чтобы следующий proposal мог реализовать UI.

Когда orchestrator обнаруживает drift при resume:
1. Run.status → `.driftDetected`, `driftDetectedAt` заполняется
2. `driftDetails` содержит human-readable diff: "Workflow hash changed: agents.yaml modified (3 agents updated)"
3. UI показывает **blocking modal** (не toast, не badge):
   - Что изменилось (из `driftDetails`)
   - Три кнопки: **[Continue with original]** / **[Restart with current]** / **[Cancel run]**
   - Warning icon без цвето-зависимости (accessible)
4. До решения инженера run не может продвинуться

**Этот UI реализуется в Proposal 002/004.** Данный proposal только обеспечивает модель (`RunStatus.driftDetected`, `DriftDecision`, snapshot fields).

```
┌─────────────────────────────────────────────────┐
│  Chainworks Forge                               │
│  [Ideas]  [Agent Catalog]  [Workflow Inspector] │
├──────────────┬──────────────────────────────────┤
│ Ideas        │  New Idea                        │
│              │  ┌──────────────────────────┐    │
│ • My feature │  │ Title: ____________     │    │
│ • Auth flow  │  │ Body:  ____________     │    │
│              │  │ File:  [Browse]         │    │
│              │  │ [Save Idea]             │    │
│              │  └──────────────────────────┘    │
│              │                                  │
│              │  (Idea is saved to SwiftData.    │
│              │   No run actions in this slice.) │
└──────────────┴──────────────────────────────────┘

┌──────────────┬──────────────────────────────────┐
│ Agents (13)  │  Agent: Lead / Orchestrator      │
│              │  Backend: claude_orchestrator_high│
│ ✅ Lead      │  Permission: ORCH                │
│ ✅ PO Review │  Skill: orchestrator_core         │
│ ✅ UX Review │  Contract: —                      │
│ ✅ Architect │  Prompt: "You are the lead..."    │
│ ...          │                                   │
│──────────────│  Validation: ✅ 0 errors, 0 warns │
└──────────────┴───────────────────────────────────┘

┌──────────────┬──────────────────────────────────┐
│ States (12)  │  state_4_proposal_reviewed       │
│              │  Label: Proposal reviewed         │
│ • idea_recv  │  Type: (normal)                   │
│ • proposal   │  Owner: lead_orchestrator         │
│ • ✋ approve │  Approval: —                      │
│ ★ reviewed  │  Run: parallel [PO, UX, UI, Arch] │
│ • refined    │       then [lead: aggregate]      │
│ • ...        │  Transitions:                     │
│              │    → state_6 when score > 9.1     │
│              │    → state_5 when score <= 9.1    │
│──────────────│                                   │
│ Validation   │  ✅ 0 errors, 1 warning           │
│ ⚠ 1 warning │  ⚠ env CHAINWORKS_META_ROOT has   │
│              │    no runtime value (default used) │
└──────────────┴───────────────────────────────────┘
```

---

## 7. Структура файлов

```
Chainworks Forge/
  Models/
    Idea.swift
    Run.swift
    RunGuard.swift              // ARCH-002: single active run invariant
    StageExecution.swift
    AgentExecution.swift
    Approval.swift
    Artifact.swift

  DSL/
    AgentCatalog.swift              // Codable structs для agent YAML
    WorkflowDefinition.swift        // Codable structs для full workflow YAML
    CompactWorkflowDefinition.swift // Codable structs для compact workflow YAML
    WorkflowNormalizer.swift        // compact → full нормализация
    YAMLParser.swift                // загрузка + десериализация
    YAMLValidator.swift             // cross-validation catalog ↔ workflow (10 checks)
    DefinitionHasher.swift          // SHA-256 provenance hashing для Run
    AnyCodableValue.swift           // helper для heterogeneous YAML values

  Views/
    ContentView.swift           // заменяем шаблон на verification scaffold
    IdeaListView.swift
    IdeaDetailView.swift
    AgentCatalogView.swift
    WorkflowPreviewView.swift

  Chainworks_ForgeApp.swift     // обновить ModelContainer schema
```

---

## 8. План выполнения

### Day 1: SwiftData Models + invariants
1. Создать 6 моделей (`Idea`, `Run`, `StageExecution`, `AgentExecution`, `Approval`, `Artifact`) + enums (`RunStatus` с `.driftDetected`, `DriftDecision`, `IdeaStatus`, `StageStatus`, `AgentStatus`, `ApprovalDecision`, `ArtifactFormat`)
2. Реализовать `RunGuard` (ARCH-002: single active run invariant)
3. Настроить relationships, delete rules, snapshot/drift fields на Run
4. Обновить `ModelContainer` в `Chainworks_ForgeApp.swift` с новой schema
5. Написать unit-тесты: CRUD, cascade delete, RunGuard, drift state persistence, cost aggregation, snapshot round-trip

### Day 2: YAML Codable structs + CodingKeys
1. Добавить Yams через SPM (Xcode project → Package Dependencies)
2. Создать все Codable-структуры для agent catalog с explicit `CodingKeys` (13 агентов, 11 backend profiles, 8 permission profiles, 11 contracts, supporting types)
3. Создать все Codable-структуры для full workflow definition с `CodingKeys`
4. Создать Codable-структуры для compact workflow (`CompactWorkflowDefinition`)
5. Реализовать `AnyCodableValue` — type-erased Codable для heterogeneous YAML values
6. Реализовать `YAMLParser` — загрузка + десериализация трёх форматов
7. Написать unit-тесты: парсинг реальных `agents.yaml`, `workflow.yaml`, `proposal-to-release.yaml` из `examples/`

### Day 3: Normalizer + hasher + validator
1. Реализовать `WorkflowNormalizer` — compact → full нормализация с `NormalizationAnnotation` metadata
2. Реализовать `DefinitionHasher` — SHA-256 для workflow/catalog provenance + `JSONEncoder` для snapshot serialization
3. Реализовать `YAMLValidator` — все 10 валидационных проверок (§4.3)
4. Написать тесты: compact → normalized round-trip, normalization annotations, hash determinism, каждая validator check

### Day 4: Verification UI (scaffold)
1. Заменить ContentView на TabView scaffold (три таба: Ideas, Agent Catalog, Workflow Inspector)
2. Tab Ideas: SwiftData CRUD + empty state
3. Tab Agent Catalog: read-only parsed YAML + summary strip + error states (File Not Found / Decode Error / Validation Errors)
4. Tab Workflow Inspector: read-only state graph + source/normalized toggle + summary strip + error states
5. Scaffold использует отдельный enum `LoadState<T>` для Loading/Loaded/FileNotFound/DecodeError

### Day 5-6 (buffer): Polish + edge cases
1. Environment variable expansion в YAML paths (`${CHAINWORKS_META_ROOT:-default}`)
2. Normalization edge cases: compact workflow без needs, single-stage workflow
3. Все `CodingKeys` проверены на реальных YAML (не синтетических)
4. Документация кода

---

## 9. Тестирование

### Unit-тесты (в `Chainworks ForgeTests/`)

```swift
// MARK: - Models

func testIdeaCreation()
func testIdeaWithoutWorkflow()         // Idea не содержит workflowID
func testRunCreationWithProvenance()   // Run хранит workflow/catalog hashes + snapshots
func testRunProvenanceIsImmutable()    // hashes не меняются после создания
func testRunSnapshotDeserializable()   // ARCH-004: snapshot JSON → WorkflowDefinition round-trip
func testRunDriftDetectedState()       // ARCH-003: status = .driftDetected is distinct from .pausedAtGate
func testRunDriftDecisionPersists()    // DriftDecision сохраняется в SwiftData
func testSequentialRunCreationBlocked()   // ARCH-002: second createRun → RunGuardError
func testParallelRunCreationSerializes()  // R4-002: async let × 2 → exactly 1 succeeds
func testRunGuardAllowsAfterCompletion()  // ARCH-002: new run OK after previous completed
func testCostCentsAggregation()        // ARCH-005: sum agent costs → run total without precision loss
func testStageExecutionRelationships()
func testApprovalDecisionFlow()
func testArtifactAttachmentToAgentExecution()
func testCascadeDeleteFromIdea()

// MARK: - Parser (full format)

func testParseAgentCatalog()           // agents.yaml → AgentCatalog
func testParseFullWorkflow()           // workflow.yaml → WorkflowDefinition
func testParseAgentDefinition()        // все 13 агентов распарсились
func testParsePermissionProfiles()     // все 8 профилей
func testParseBackendProfiles()        // все 11 профилей
func testParseArtifactContracts()      // все 11 контрактов
func testParseWorkflowStates()         // все 12 states
func testParseTransitions()            // transitions + when expressions
func testParseLoopConfig()             // loop counters
func testParseFailurePolicy()
func testInvalidYAMLThrows()
func testMissingRequiredFieldsThrows()

// MARK: - Parser (compact format + normalization)

func testParseCompactWorkflow()        // proposal-to-release.yaml → CompactWorkflowDefinition
func testCompactStagesParsed()         // все 10 compact stages
func testNormalizeCompactToFull()      // CompactWorkflowDefinition → NormalizedWorkflow
func testNormalizedHasCorrectStates()  // каждый compact stage → один WorkflowState
func testNormalizedTransitions()       // needs → transitions правильно выведены
func testNormalizedApprovalGates()     // approval stages → manual_gate
func testNormalizedFanout()            // fanout stages → parallel run block
func testNormalizedHasTerminalState()  // добавлен end state
func testCompactWithoutNeeds()         // stage без needs → первый stage
func testNormalizationAnnotations()    // inferred/defaulted/synthesized annotations present
func testNormalizedIsNormalizedFlag()  // NormalizedWorkflow.isNormalized == true for compact

// MARK: - Provenance hashing

func testDefinitionHashDeterministic()        // 100 encodes of same object → identical hash (R4-003)
func testDefinitionHashChanges()              // изменение prompt → другой hash
func testDefinitionHashSortedKeysRequired()   // verify .sortedKeys produces stable output for [String:T]

// MARK: - Validator (full coverage)

// State graph
func testMissingInitialState()         // initial_state не существует в states
func testBrokenTransition()            // transition to non-existent state
func testOrphanState()                 // state unreachable from initial_state
func testNoEndState()                  // нет state с type: end

// Agent ↔ workflow
func testMissingAgentInWorkflow()      // workflow references agent not in catalog
func testMissingOwnerInWorkflow()      // state owner not in catalog

// Catalog internal consistency
func testBrokenBackendProfileRef()     // agent → несуществующий backend profile
func testBrokenPermissionProfileRef()  // agent → несуществующий permission profile
func testBrokenSkillRef()              // agent → несуществующий skill
func testBrokenOutputContractRef()     // agent → несуществующий contract
func testBrokenArtifactRef()           // agent input → несуществующий artifact

// Provider coverage
func testMissingRequiredProvider()     // required_providers не покрыты

// Env placeholders
func testValidEnvPlaceholders()        // ${VAR:-default} синтаксически ок
func testMalformedEnvPlaceholder()     // ${VAR без закрытия

// Run block semantics
func testEmptySequenceBlock()          // пустой sequence
func testEmptyFanoutAgents()           // fanout с пустым agents list
func testDuplicateAgentInThen()        // agent в parallel и в then одновременно

// Happy path
func testValidConfigPassesValidation() // каноничные YAML файлы → 0 errors
```

---

## 10. Risks

| Риск | Влияние | Митигация |
|---|---|---|
| YAML-структура изменится в процессе разработки | Средний | Codable structs легко обновить; optional fields для forward compatibility |
| Yams не поддерживает edge case из наших YAML | Низкий | YAML-файлы стандартные, Yams зрелая библиотека |
| SwiftData schema migration при изменении моделей | Средний | На стадии dev — просто сбрасывать store; migration policy до production |
| Xcode project structure conflicts | Низкий | Чёткая структура папок, определённая в §7 |

---

## 11. Criteria of Done

### Domain Model
- [ ] Все 6 SwiftData моделей + `RunGuard` компилируются и создают store
- [ ] `Run` хранит immutable provenance: hashes + full JSON snapshots (ARCH-004)
- [ ] `RunStatus` включает `.driftDetected` как отдельный state (ARCH-003)
- [ ] `DriftDecision` persists и восстанавливается (ARCH-003)
- [ ] `Idea` НЕ содержит workflowID (workflow identity на Run)
- [ ] `RunGuard.createRun` — atomic check+insert, `@MainActor`-serialized (ARCH-002 + R4-002)
- [ ] Parallel-start test proves serialization (R4-002)
- [ ] Cost хранится как `Int64` cents, не `Double` (ARCH-005)
- [ ] CRUD-операции на моделях работают (тесты проходят)

### YAML Parser
- [ ] Все Codable structs используют explicit `CodingKeys` для snake_case mapping (ARCH-001)
- [ ] `agents.yaml` из `examples/` парсится в `AgentCatalog` без ошибок (13 агентов, 11 backend profiles, 8 permission profiles, 11 contracts)
- [ ] `workflow.yaml` из `examples/` парсится в `WorkflowDefinition` без ошибок (12 states)
- [ ] `proposal-to-release.yaml` парсится в `CompactWorkflowDefinition` (отдельный тип)
- [ ] `WorkflowNormalizer` преобразует compact → full `WorkflowDefinition` корректно
- [ ] Normalized view аннотирует defaulted/inferred поля (UX-02)
- [ ] `DefinitionHasher.canonicalEncoder` uses `.sortedKeys` — deterministic for dictionary types (R4-003)
- [ ] `DefinitionHasher.hash()` даёт идентичный SHA-256 при 100 повторных вызовах
- [ ] Snapshot JSON round-trip: serialize → deserialize → identical hash

### Validator
- [ ] `YAMLValidator.validateAll` находит все 10 категорий ошибок (тесты на каждую)
- [ ] `YAMLValidator.validateAll` пропускает каноничные YAML без errors

### Verification Scaffold
- [ ] Tab Ideas: SwiftData CRUD (create/delete idea, no run actions)
- [ ] Tab Ideas: empty state shows "No ideas yet" (UI-002)
- [ ] Tab Agent Catalog: read-only parsed YAML + summary strip + validation issues
- [ ] Tab Agent Catalog: distinct File Not Found / Decode Error / Validation Error states (UX-03)
- [ ] Tab Workflow Inspector: read-only state graph + validation issues
- [ ] Tab Workflow Inspector: source (compact) vs normalized (full) toggle with [default]/[inferred] badges (UX-02)
- [ ] Tab Workflow Inspector: distinct error states (UX-03)
- [ ] Summary strips на каждом табе (UI-001)

### General
- [ ] Приложение компилируется и запускается на macOS
- [ ] Все unit-тесты проходят

---

## 12. Что делает этот proposal фундаментом

```
                    ┌─────────────────────────┐
                    │   Proposal 001          │
                    │   Domain Model          │
                    │   + YAML Parser         │
                    └────────┬────────────────┘
                             │
              ┌──────────────┼──────────────────┐
              │              │                  │
              ▼              ▼                  ▼
    ┌─────────────┐  ┌──────────────┐  ┌───────────────┐
    │ Proposal 002│  │ Proposal 003 │  │ Proposal 004  │
    │ Workflow    │  │ Goose        │  │ Main UI:      │
    │ State       │  │ REST/SSE     │  │ Ideas, Runs,  │
    │ Machine     │  │ Adapter      │  │ Stages, Gates │
    │ Engine      │  │              │  │               │
    └─────────────┘  └──────────────┘  └───────────────┘
```

Без domain model — негде хранить state.
Без YAML parser — нечего исполнять.
Всё остальное строится поверх.

---

## 13. Review Response Log

### Review Round 1 (2026-03-22)

**Review type:** Evidence Gap Review (partial confidence)

| # | Finding | Severity | Resolution |
|---|---------|----------|------------|
| 1 | Workflow provenance modeled on `Idea` instead of `Run`; reruns and resume unsafe when YAML changes | P1 / High | **Fixed.** `workflowID` removed from `Idea`. `Run` now carries immutable provenance: `workflowID`, `workflowTitle`, `workflowSnapshotHash` (SHA-256), `catalogSnapshotHash`, source paths. Drift detection contract documented. |
| 2 | `loadCompactWorkflow` treats compact YAML as same type as full, but schemas differ | P1 / High | **Fixed.** Introduced `CompactWorkflowDefinition` as separate Codable type. Added `WorkflowNormalizer.normalize()` with explicit rules for compact→full conversion. Limitations documented (no loops, no scoring in compact). |
| 3 | Validator only checks 3 things; broken profile/contract/skill refs escape to runtime | P2 / Medium | **Fixed.** Validator expanded to 10 check categories: state graph (4), agent↔workflow (2), catalog consistency (5: backend, permission, skill, contract, artifact refs), providers (1), env placeholders (1), run block semantics (1). Each has dedicated test. |
| 4 | Verification scaffold includes "Start Run" which is out of scope; dead affordance or scope creep | P2 / Medium | **Fixed.** Scaffold redesigned as three read-only tabs: Ideas (SwiftData CRUD only, no run actions), Agent Catalog (parsed YAML inspection), Workflow Inspector (state graph + validation). No workflow selection, no Start Run button. |

### Review Round 2 (2026-03-22)

**Review type:** Evidence Gap Review, full-review mode (partial confidence, medium)
**Source:** `docs/reviews/001-foundation-domain-model-and-yaml-parser-review.md`

| # | ID | Finding | Severity | Resolution |
|---|-----|---------|----------|------------|
| 5 | ARCH-001 | Parser structs use camelCase but canonical YAML uses snake_case; no CodingKeys or decoder strategy specified | P0 / High | **Fixed.** Added §4.4 specifying `snake_case → camelCase` decoder strategy via Yams + explicit `CodingKeys` enums for all Codable structs. Acceptance criteria: real YAML files from `examples/` decode without preprocessing. All parser tests use real files, not synthetic. |
| 6 | ARCH-002 | `single_active_run_per_idea` invariant unenforced; unconstrained `Idea.runs` allows concurrent runs | P0 / High | **Fixed.** Added `RunGuard` with `ensureNoActiveRun(for:in:)` — application-level check before `ModelContext.insert(run)`. Active = `.running` / `.pausedAtGate` / `.driftDetected`. Test: `testConcurrentRunCreationBlocked`. |
| 7 | ARCH-003 | `driftDetected` mentioned in provenance contract but absent from `RunStatus` enum | P0 / High | **Fixed.** `RunStatus` now includes `.driftDetected`. Added `DriftDecision` enum (`continueWithOriginal` / `restartWithCurrent` / `cancelled`). Added `driftDetectedAt`, `driftDetails`, `driftDecision` fields on `Run`. Tests added. |
| 8 | ARCH-004 | Hash-only provenance cannot support "continue with old definition" because old YAML no longer available after edit | P0 / High | **Fixed.** `Run` now stores full serialized snapshots: `workflowSnapshotJSON: Data` and `catalogSnapshotJSON: Data`. On drift + `continueWithOriginal`, orchestrator deserializes workflow/catalog from snapshot. Hash retained for quick comparison. Test: `testRunSnapshotDeserializable`. |
| 9 | ARCH-005 | Cost fields as `Double` cause cent-level precision drift in aggregation | P2 / High | **Fixed.** `Run.totalCostUSD` → `totalCostCents: Int64`. `AgentExecution.costUSD` → `costCents: Int64`. Rounding only at presentation. Test: `testCostCentsAggregation`. |
| 10 | UI-001 | Tabs lack visual hierarchy; validation problems have same weight as ordinary fields | P2 / High | **Fixed.** Added §6.2: summary strip at top of each tab with counts + health state. Validation issues visually primary. |
| 11 | UI-002 | No empty/loading/error states specified for verification tabs | P1 / Medium | **Fixed.** Added §6.1: five exclusive states per YAML tab (Loading, Loaded, File Not Found, Decode Error, Validation Errors) and two states for Ideas tab (Empty, Has Ideas). Each renders distinct recoverable UI. |
| 12 | UX-01 | Drift detection defined but no user-facing drift state, explanation, or recovery surface | P1 / High | **Fixed.** Added §6.3: drift-review UI contract (blocking modal, three actions, accessible warning). Marked as out-of-scope for scaffold implementation but contract defined for Proposal 002/004. |
| 13 | UX-02 | Compact normalization lossy but inspector doesn't separate source from normalized output | P1 / Medium | **Fixed.** Tab 3 now shows Source (compact) vs Normalized (full) toggle. Defaulted/inferred fields annotated with badges. |
| 14 | UX-03 | File-not-found, decode failure, and validation failure share one recovery path | P1 / Medium | **Fixed.** Three distinct error states with tailored copy and next steps (§6.1). |

### Review Round 3 — Self-Review (2026-03-22)

**Review type:** Internal consistency audit against canonical YAML sources

| # | ID | Finding | Severity | Resolution |
|---|-----|---------|----------|------------|
| 15 | SELF-01 | Agent count "12" wrong everywhere — `agents.yaml` contains 13 agents (12 specialized + lead) | P0 | **Fixed.** All occurrences updated to 13. |
| 16 | SELF-02 | §1 says "7 permission profiles" — `agents.yaml` has 8 (ORCH, RO_REVIEW, RO_VERIFY, PROPOSAL_WRITE, CODE_WRITE, DOC_WRITE, RELEASE_GIT, RELEASE_PUBLISH) | P0 | **Fixed.** Updated to 8. |
| 17 | SELF-03 | Mapping table (§3.2) still references stale `Run.totalCostUSD` after ARCH-005 rename | P1 | **Fixed.** → `Run.totalCostCents`. |
| 18 | SELF-04 | §4.4 decoder example uses non-existent Yams `userInfo` keyDecodingStrategy API | P1 | **Fixed.** Removed fake API. YAMLParser now uses plain `YAMLDecoder().decode()` + explicit `CodingKeys` on every struct. |
| 19 | SELF-05 | 8 types referenced in Codable structs but never defined: `ScoringConfig`, `ExecutionConfig`, `IdeaInputConfig`, `FilesystemPermissions`, `GitPermissions`, `ShellPermissions`, `NetworkPermissions`, `MCPPermissions`, `AnyCodableValue` | P1 | **Fixed.** All types now defined in §4.2 with fields and CodingKeys matching canonical YAML. |
| 20 | SELF-06 | Provenance contract says "snapshot deserializes via YAMLParser" but snapshot is JSON Data, not YAML | P1 | **Fixed.** → "uses `JSONDecoder`". |
| 21 | SELF-07 | Stray `` ``` `` on line 198 breaks markdown structure after RunGuard section | P2 | **Fixed.** Removed. |
| 22 | SELF-08 | Day 1 plan says "create all 6 models" but doesn't mention RunGuard, drift infrastructure, snapshot fields added by ARCH-002/003/004 | P2 | **Fixed.** Plan rewritten to cover all models, enums, RunGuard, drift fields, snapshots across 5-6 days. |
| 23 | SELF-09 | Estimated effort "3-4 days" unrealistic after scope growth (drift, snapshots, 10 validator checks, error states, normalization annotations) | P2 | **Fixed.** → 5-6 days. |
| 24 | SELF-10 | UX-02 requires [default]/[inferred] badges in normalized view but no annotation mechanism defined | P2 | **Fixed.** Added `NormalizedWorkflow` wrapper with `[NormalizationAnnotation]` and `AnnotationKind` enum (defaulted/inferred/synthesized). Normalizer returns `NormalizedWorkflow`, not bare `WorkflowDefinition`. |

### Review Round 4 (2026-03-22)

**Review type:** Evidence Gap Review (partial confidence, medium)
**Source:** reviewer feedback, three findings

| # | ID | Finding | Severity | Resolution |
|---|-----|---------|----------|------------|
| 25 | R4-001 | CodingKeys still missing on top-level types (`AgentCatalog`, `AppConfig`, `WorkflowDefinition`, `WorkflowMeta`, `WorkflowState`, `BackendProfile`, `WorktreePolicy`, `ArtifactContract`). Parser would fail on canonical YAML. | P1 / High | **Fixed.** Every YAML-facing struct in §4.2 now has explicit `CodingKeys` for all snake_case fields. Types with only single-word keys (`RunBlock`, `AgentTask`, `Transition`, `LoopConfig`, `PermissionProfile`, `SkillRef`) document why CodingKeys are omitted. Contract header added: "verified by parser tests that decode canonical `examples/` files." |
| 26 | R4-002 | `RunGuard.ensureNoActiveRun` is a TOCTOU-vulnerable read-then-insert pattern. Test is sequential, not concurrent. | P1 / High | **Fixed.** RunGuard redesigned as `@MainActor` with atomic `createRun(for:workflow:catalog:in:)` that combines check + insert in one synchronous block. TOCTOU eliminated because `@MainActor` serializes all callers. Added `testParallelRunCreationSerializes` using `async let` to prove concurrent starts serialize correctly. Direct `ModelContext.insert(Run)` is now a contract violation. |
| 27 | R4-003 | Provenance hashing depends on `JSONEncoder` but no canonical settings specified. Dictionary-heavy types (`states`, `variables`, `backendProfiles`) produce non-deterministic key order without `.sortedKeys`. | P2 / Medium | **Fixed.** Added `DefinitionHasher.canonicalEncoder` with explicit settings: `.sortedKeys` + `.withoutEscapingSlashes` + `.iso8601` date encoding. `hash<T>()` method combines encode + SHA-256 in one call. Explained why `.sortedKeys` is mandatory for `[String: T]` types. Added stability tests. |
