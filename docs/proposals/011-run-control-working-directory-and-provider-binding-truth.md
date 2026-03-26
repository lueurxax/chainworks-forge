# Proposal 011: Run Control, Working Directory Ownership, and Provider Binding Truth

| Field | Value |
|---|---|
| Date | 2026-03-26 |
| Status | Draft |
| Author | Engineer (single-engineer project) |
| Depends on | [reference/runtime-contract.md](../reference/runtime-contract.md), [reference/operator-experience.md](../reference/operator-experience.md), [reference/provider-platform.md](../reference/provider-platform.md), [reference/idea-lifecycle.md](../reference/idea-lifecycle.md), [reference/goose-provider-remediation.md](../reference/goose-provider-remediation.md), [reference/live-workflow-map.md](../reference/live-workflow-map.md) |
| Goal | Make run control and agent execution truth operationally trustworthy by separating stop from archive, attaching one explicit working directory contract to each idea, and making provider/model resolution truthful from catalog to live run surfaces. |

---

## 1. Context

The current stable operator-clarity baseline already covers:

- archive lifecycle,
- Goose-first provider troubleshooting,
- workflow map and agent activity.

Current `HEAD` exposed three additional contracts that should not keep expanding the operator-clarity baseline in place:

1. stopping active work is not the same thing as archiving finished work,
2. project-backed ideas still need one explicit working directory / repo-root contract,
3. provider/model labels shown in run surfaces can diverge from catalog intent in ways that are not obviously safe.

These are not cosmetic details.
They affect whether the operator can trust what the app is doing to a real repository and whether the displayed agent quality is the one the runtime is actually attempting to use.

---

## 2. Product questions this proposal must answer

After Proposal 011, the engineer should be able to:

1. stop an active idea and trust that all in-flight agent work for its run is being cancelled,
2. archive only after work has genuinely reached a terminal state,
3. see one explicit working directory / project root on every project-backed idea,
4. trust that run compilation and live execution use that explicit directory rather than ambient app cwd,
5. trust that provider/model labels shown in run surfaces reflect the frozen resolved binding actually sent to Goose,
6. understand when a model came from backend-profile intent, configured provider default, or explicit run override.

### Definition of done

Proposal 011 is done only when all of the following are true:

1. stop and archive are separate lifecycle actions in both policy and UI,
2. stopping a run cancels orchestrator progress and propagates cooperative cancellation to in-flight agent work,
3. a project-backed idea cannot start live work without one explicit valid working directory contract,
4. run compilation freezes that directory into the run/workspace context,
5. provider/model resolution is validated for family coherence before live start,
6. run surfaces show resolved binding truth plus provenance rather than ambiguous mixed labels.

---

## 3. What we build

Three tightly scoped slices.

### Layer P: Run Control Truth

| Component | Responsibility |
|---|---|
| `IdeaStopService` | Stops the active run for an idea and coordinates run-wide cancellation |
| `RunCancellationCoordinator` | Propagates cancellation to orchestrator, active agent executions, and Goose session handles |
| `StopRunConfirmationSurface` | Explains exactly what will stop and what historical data remains intact |
| `RunTerminationBadge` | Distinguishes cancelled terminal work from failed terminal work |

### Layer Q: Idea Working Directory Ownership

| Component | Responsibility |
|---|---|
| `IdeaWorkspaceContract` | Durable project-root / working-directory truth owned by the idea |
| `IdeaWorkspaceEditor` | Canonical owner-path UI for selecting, validating, and editing the idea directory |
| `WorkspaceReadinessProbe` | Validates path existence, accessibility, and policy fit before run start |
| `RunWorkspaceFreezer` | Copies the idea-owned directory into the frozen run/workspace snapshot |

### Layer R: Provider Binding Truth

| Component | Responsibility |
|---|---|
| `ResolvedBindingExplainer` | Explains how provider/model/effort were resolved for each agent |
| `ProviderModelCoherencePolicy` | Blocks or warns on cross-family provider/model mismatches |
| `RunBindingProvenancePanel` | Shows backend profile intent, configured provider choice, resolved runtime binding, and override provenance |
| `BindingTruthProbe` | Verifies the frozen binding sent to Goose matches what operator surfaces show |

---

## 4. Stop vs archive lifecycle

### 4.1 Core rule

Stopping and archiving are related but not interchangeable.

- `Stop` is an execution-control action.
- `Archive` is an attention-management action.

Archive never implies stop.
An active idea must be stopped first, and its active run must settle into a terminal state before archive becomes eligible.

### 4.2 Stop semantics

Stopping an idea means:

- the active run stops accepting new stage progress,
- the orchestrator enters cancellation flow,
- active agent executions receive cooperative cancellation,
- managed Goose sessions for that run are explicitly closed where possible,
- the run reaches terminal `cancelled` truth only after cancellation propagation is recorded.

### 4.2.1 Cancellation settlement criteria

A run's cancellation is **settled** only when all of the following are true:

1. the orchestrator has exited its state-machine loop and will not advance to any new stage,
2. every `AgentExecution` that was `.running` at cancellation-request time has transitioned to a terminal status (`.cancelled`, `.failed`, or `.completed`),
3. every managed Goose session that was open at cancellation-request time has received a `closeSession` call (success or best-effort timeout),
4. the `Run` record carries both `cancellationRequestedAt` (when the operator pressed stop) and `cancellationSettledAt` (when criteria 1–3 were confirmed).

`RunCancellationCoordinator` is responsible for checking criteria 1–3 and writing `cancellationSettledAt` only after all are satisfied.

A run whose `cancellationRequestedAt` is set but `cancellationSettledAt` is still nil must display as **"cancelling…"**, not as settled `cancelled` truth.

The minimum persisted evidence for settlement is:

```swift
// On Run
var cancellationRequestedAt: Date?       // when operator pressed stop
var cancellationSettledAt: Date?         // when coordinator confirmed propagation
var cancellationSettlementLog: Data?     // JSON array of per-agent settlement entries

// Each entry in the settlement log
struct CancellationSettlementEntry: Codable {
    let agentExecutionID: UUID
    let agentID: String
    let priorStatus: String              // status at cancellation-request time
    let terminalStatus: String           // status after propagation
    let sessionCloseAttempted: Bool
    let sessionCloseSucceeded: Bool?     // nil if no session was open
    let settledAt: Date
}
```

This log is the authoritative proof that cancellation was propagated, not just requested.

### 4.3 Operator surface rules

- `Ideas` is the owner path for stopping an active idea,
- stop must not be hidden behind archive affordances,
- the confirmation surface must say that run history, artifacts, and receipts remain intact,
- cancelled runs remain visible in run-centric surfaces with truthful status.

---

## 5. Idea working directory contract

### 5.1 Core rule

Every project-backed idea owns one explicit working directory / project root.

This path belongs to the idea itself, not only to a transient run-start sheet.

### 5.2 Project access requirement selector

The question "does this workflow require project access?" must have **one typed authoritative answer** shared by Start Run, preflight, compiler, and resume.

That answer lives in the workflow definition:

```yaml
workflow:
  execution:
    requires_project_access: true   # or false
```

Parsed into the compiled `RunPlan` as:

```swift
// RunPlan or WorkflowMeta
let requiresProjectAccess: Bool
```

Rules that depend on this selector:

| Consumer | Behavior when `true` | Behavior when `false` |
|---|---|---|
| **Start Run** | Blocks start if `idea.workspaceRootPath` is nil or invalid | Allows start without a directory |
| **Preflight** | Includes workspace readiness check | Skips workspace readiness check |
| **Compiler / `createRun()`** | Freezes idea directory into `RunWorkspace`; fails if missing | Creates workspace without idea directory |
| **Resume** | Restores frozen workspace directory; fails if directory no longer exists | Resumes without directory dependency |

If `requires_project_access` is absent from the YAML, it defaults to `false` for backward compatibility.

### 5.3 Rules

- the operator can view and edit the idea directory from the idea owner path,
- the path must be persisted on the idea or on one directly idea-owned configuration object,
- live runs must fail closed if project-backed execution is requested without a valid idea directory,
- repo-agnostic ideas may explicitly remain directory-free, but only when `requires_project_access` is `false`,
- agent execution must never infer project context from ambient process cwd.

### 5.4 Frozen run contract

At run start:

- the idea directory is copied into the frozen run/workspace snapshot,
- the frozen path becomes the source of truth for that run,
- later edits to the idea directory do not mutate an already-started run.

---

## 6. Provider/model binding truth

### 6.1 Problem statement

The catalog screen may correctly show:

- backend profile `claude_orchestrator_high`
- provider `claude_code`
- model `default`

but the run surface can still end up showing something like:

- `claude_code · gpt-5-codex · high`

That is not acceptable as ambiguous UI.
It may indicate either:

- harmlessly confusing display logic,
- or a real runtime mismatch where the resolved model sent to Goose no longer matches operator intent.

### 6.2 Four facts the UI must distinguish

Operator surfaces must keep these four facts separate:

1. backend profile intent from the catalog,
2. configured provider record selected at run start,
3. resolved runtime provider identifier/family,
4. resolved runtime model actually sent to Goose.

### 6.3 Coherence policy

Rules:

- a configured provider default model must not silently create a cross-family mismatch,
- provider settings and preflight should validate family/model coherence where possible,
- if the runtime intentionally allows an unusual binding, the UI must present it as an explicit warning with provenance,
- the run screen must prefer frozen resolved binding truth over catalog shorthand.

### 6.4 Provenance

The operator should be able to see whether the resolved model came from:

- backend profile default,
- configured provider default,
- run-start override.

That provenance must be available from run-centric surfaces without opening raw logs.

### 6.5 Provenance must be reproducible from frozen data only

Provenance cannot rely on mutable current provider settings for historical runs.

After provider settings drift (e.g., a configured provider's default model changes), run-centric surfaces, reports, and comparison views must still correctly explain the model origin for past runs.

This means provenance **must be frozen at run start**, not derived at display time.

Required frozen provenance per agent binding:

```swift
struct FrozenBindingProvenance: Codable, Sendable {
    /// The source that determined the resolved model.
    let source: BindingProvenanceSource
    /// The backend profile ID from the agent catalog.
    let backendProfileID: String
    /// The backend profile's declared model (may be "default" or explicit).
    let backendProfileModel: String
    /// The configured provider ID selected at run start (if any).
    let configuredProviderID: UUID?
    /// The configured provider's default model at the time of run start.
    let configuredProviderDefaultModel: String?
    /// The explicit run-start override model (if any).
    let runOverrideModel: String?
    /// The final resolved model actually sent to Goose.
    let resolvedModel: String
    /// The final resolved provider family.
    let resolvedProviderFamily: String
}

enum BindingProvenanceSource: String, Codable, Sendable {
    case backendProfileDefault = "backend_profile"
    case configuredProviderDefault = "configured_provider"
    case runOverride = "run_override"
    case unverifiable = "unverifiable"
}
```

This struct is persisted once per agent binding in the run's `providerBindingSnapshotJSON` (or in an adjacent `bindingProvenanceJSON` field on the `Run`).

At display time, surfaces read provenance from the frozen snapshot — never from live provider settings.

Rule: the resolver must always be able to determine provenance from the three frozen inputs (backend profile, configured provider, run override). The `createRun()` path must supply all three inputs to the resolver so that provenance is guaranteed to be computable.

If, despite this contract, provenance cannot be determined (e.g., a future code path bypasses the resolver), the run must record `source: .unverifiable` and log a diagnostic error. The UI must display the binding as "provenance unknown" rather than attributing it to any specific source. Recording a false source (e.g., `.backendProfileDefault` when the actual source was not the backend profile) is never permitted.

---

## 7. Data and runtime model additions

Required additions:

```swift
// Idea — §5
var workspaceRootPath: String?

// Run — §4.2.1 cancellation settlement
var cancellationRequestedAt: Date?
var cancellationSettledAt: Date?
var cancellationSettlementLog: Data?     // JSON-encoded [CancellationSettlementEntry]

// Run — §6.5 frozen binding provenance
var bindingProvenanceJSON: Data?         // JSON-encoded [String: FrozenBindingProvenance]
                                          // keyed by agent ID

// WorkflowMeta / ExecutionConfig — §5.2
let requiresProjectAccess: Bool          // from workflow YAML
```

Rules:

- new persistence is allowed only for facts that are not already recoverable from existing run history,
- run surfaces must read provider/model truth from the frozen binding snapshot first,
- binding provenance is **frozen at run start** and must not be derived from mutable current settings (§6.5),
- cancellation settlement is **recorded at propagation time** and must not be derived from local flag flips (§4.2.1),
- `requires_project_access` is the **single authoritative selector** consumed by Start Run, preflight, compiler, and resume (§5.2).

---

## 8. Acceptance criteria

### Stop / run control

- [ ] Active ideas expose a stop action from the idea owner path
- [ ] Stopping an idea cancels the active run and propagates cancellation to in-flight agent work
- [ ] `RunCancellationCoordinator` confirms all four settlement criteria (§4.2.1) before writing `cancellationSettledAt`
- [ ] A run with `cancellationRequestedAt` set but `cancellationSettledAt` nil displays as "cancelling…", not as settled `cancelled`
- [ ] `cancellationSettlementLog` records per-agent terminal status and session-close outcome
- [ ] Cancelled runs remain visible as truthful terminal history
- [ ] Stop and archive never collapse into one ambiguous action

### Working directory

- [ ] `requires_project_access` is declared in workflow YAML and parsed into `RunPlan` (§5.2)
- [ ] Start Run, preflight, compiler, and resume all read the same `requiresProjectAccess` selector
- [ ] Each project-backed idea has one explicit working directory / project root contract
- [ ] Live run start fails closed when `requiresProjectAccess` is true and the idea directory is missing or invalid
- [ ] Frozen run/workspace state stores the idea-owned directory used for that run
- [ ] Agent execution does not rely on ambient app cwd for project selection

### Provider/model truth

- [ ] Run surfaces show resolved provider/model truth from the frozen binding
- [ ] `FrozenBindingProvenance` is persisted per agent binding at run start (§6.5)
- [ ] The UI can explain whether the shown model came from backend profile default, configured provider default, or explicit run override — using only frozen provenance data
- [ ] Historical runs remain correctly explained after provider settings drift
- [ ] Cross-family provider/model mismatches are blocked or surfaced as explicit warnings
- [ ] Operator surfaces no longer present ambiguous combinations like `claude_code · gpt-5-codex` as if they were normal truth

---

## 9. Out of scope

| Exclusion | Reason |
|---|---|
| New provider families | This proposal is about binding truth, not provider expansion |
| New archive taxonomy | Archive lifecycle remains owned by [reference/idea-lifecycle.md](../reference/idea-lifecycle.md) |
| Workflow-map redesign | Map and activity remain owned by [reference/live-workflow-map.md](../reference/live-workflow-map.md) |
| Multi-user repo assignment | Still a single-engineer local-first product |

---

## 10. Locked decisions

| ID | Decision | Rationale |
|---|---|---|
| OPS-201 | Stop and archive are separate lifecycle actions | Prevents hiding active work behind archive semantics |
| OPS-202 | Every project-backed idea owns one explicit working directory contract | Prevents agents from guessing target repo from ambient cwd |
| OPS-203 | Run surfaces must show resolved provider/model truth and provenance, not ambiguous mixed labels | Prevents operator trust loss from misleading agent cards |
| OPS-204 | Frozen run binding truth outranks catalog shorthand in run-centric UI | Run surfaces must reflect execution truth, not static intent alone |

---

## 11. What this proposal enables

Proposal 011 closes three trust gaps that become visible only once the product starts feeling real:

- stopping work must actually stop work,
- every idea must know which project it belongs to,
- and an agent card must mean what it says.

That is not a new capability family.
It is the difference between a believable operator shell and one the engineer can trust around a real repository.
