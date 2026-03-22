
# Proposal 004: Live Provider Execution Slice — Goose Adapter, Session Bridge, and Real Proposal Loop

| Field | Value |
|---|---|
| Date | 2026-03-22 |
| Status | Draft |
| Author | Engineer (single-engineer project) |
| Depends on | Proposal 001 (Foundation — Domain Model + YAML DSL Parser), Proposal 002 (Workflow Execution Engine — RunPlan Compiler, Orchestrator, Approval Flow) |
| Adjacent work | Proposal 003 (Forge Steward) is valuable, but **not** on the critical path for first live execution |
| Goal | Get the first **real** end-to-end run in the app with a provider-backed executor |

---

## 1. Context

Proposal 001 establishes the foundation: SwiftData models for `Idea -> Run -> StageExecution -> AgentExecution -> Approval -> Artifact`, immutable run provenance, and the YAML DSL parser/validator.

Proposal 002 establishes the execution engine: `RunPlanCompiler`, `ExecutionService`, `WorkflowOrchestrator`, `TransitionEvaluator`, `ArtifactManager`, approval flow, resume handling, and a **simulated** `AgentExecutor`.

That means the product already has:

- a place to store runs and stage progress,
- a typed workflow/runtime model,
- a way to pause at approval gates,
- artifact persistence,
- run resume semantics,
- and a testing executor that proves the engine shape.

What is still missing is the part that makes the app feel real in the hands:

> a provider-backed executor that can take a real idea, call a real model through Goose, produce real artifacts, and walk a real workflow until the engineer approves the result.

This proposal is intentionally narrow.
It does **not** try to make the whole 12-state `proposal-to-release` pipeline live in one step.
It builds the first useful vertical slice:

**Idea -> Proposal draft -> Review fan-out -> Proposal refinement loop -> Human approval -> Completed run**

That is the shortest path from architecture to felt product value.

---

## 2. Why this proposal now

Proposal 002 is intentionally broad and infrastructural.
It proves that the app can compile and execute workflows with a simulated executor.
That is the correct foundation, but simulated execution still leaves one practical question unanswered:

> Can the app run a real workflow against a real provider, stream progress, persist outputs, pause for approval, and recover safely?

Until that question is answered, the product is still mostly an elegant control plane sketch.

This proposal answers that question with the smallest possible live slice.

### Why not jump directly to implementation/release

Because the fastest way to lose a week is to mix several unstable concerns at once:

- live provider integration,
- writable worktrees,
- git side effects,
- release side effects,
- permission enforcement,
- multi-provider routing,
- and human gates around destructive operations.

If all of that lands at once, the first broken run tells us almost nothing.
We will not know whether the issue is:

- the executor,
- Goose session isolation,
- artifact binding,
- run resume,
- the worktree layer,
- or the release path.

This proposal keeps the surface small enough that failures stay interpretable.

---

## 3. Product question this proposal must answer

The proposal is successful if one engineer can do the following inside the app:

1. Create an idea.
2. Start a **live** proposal-loop run.
3. Watch a real proposal be drafted.
4. Watch real reviewer agents fan out and return artifacts.
5. See a real review summary and proposal refinement.
6. Hit a real approval gate.
7. Approve the proposal.
8. See the run finish with real stored artifacts and inspectable execution traces.

If that works, the product stops being theoretical.

---

## 4. What we build

Two layers:

### Layer E: Live Provider Runtime

The first real provider-backed executor path.

| Component | Responsibility |
|---|---|
| **GooseTransport** | Low-level client for Goose backend over HTTP/SSE |
| **GooseSessionBridge** | Creates an isolated session for one `AgentExecution`, binds workspace and prompt packet |
| **GooseAgentExecutor** | Concrete implementation of `AgentExecutor` using Goose |
| **ExecutionEventBridge** | Converts provider events into app-friendly progress events for UI and logs |
| **ExecutionReceiptBuilder** | Produces structured receipt/transcript artifacts from a live execution |

### Layer F: Live Proposal Slice

A deliberately narrow workflow and UI path that uses the runtime above.

| Component | Responsibility |
|---|---|
| **Live Proposal Workflow** | Dedicated workflow slice: draft -> review -> refine -> approve |
| **Provider-backed Start Run Flow** | Launches a real run instead of a simulated one |
| **Live Run Progress UI** | Shows streaming stage/agent state and provider-backed updates |
| **Live Artifact Inspector** | Opens actual proposal/review artifacts and raw receipts |
| **Approval Gate on Real Run** | Human approval before run completion |

---

## 5. Scope

### In scope

1. A **real** `GooseAgentExecutor` implementing Proposal 002's `AgentExecutor` protocol.
2. A dedicated live workflow for the **proposal loop only**.
3. Provider-backed execution for this agent subset:
   - `lead_orchestrator`
   - `proposal_writer`
   - `proposal_reviewer_product_owner`
   - `proposal_reviewer_ux`
   - `proposal_reviewer_ui`
   - `proposal_reviewer_architect`
4. Real artifact persistence for:
   - proposal drafts,
   - proposal review artifacts,
   - proposal review summary,
   - proposal refinement summary,
   - execution receipts / transcripts.
5. Real approval flow at the end of the proposal loop.
6. Resume support for interrupted **safe** live runs.
7. Runtime safety boundaries:
   - explicit `RunWorkspace`,
   - no implicit cwd,
   - no git/release side effects,
   - no writable repo worktree.

### Out of scope

1. Code-writing stages.
2. Security/audit/pre-push/docs stages.
3. Git commit/push.
4. Connect distribution.
5. Writable implementation worktrees.
6. Full multi-provider routing.
7. Provider selection per-agent in the first live pass.
8. ACP-based integration.
9. Automatic workflow/catalog mutation by Forge Steward.

---

## 6. Narrowing decision: build a dedicated live workflow

To move fast, this proposal introduces a dedicated workflow file for live experimentation:

`examples/workflows/proposal-loop-live.yaml`

This avoids overloading the full release workflow before the provider path is trusted.

### Proposed states

| State | Purpose |
|---|---|
| `state_1_idea_received` | Normalize input and prepare proposal brief |
| `state_2_proposal_drafted` | Proposal Writer creates initial draft |
| `state_3_proposal_reviewed` | PO / UX / UI / Architect reviewers run in parallel, Lead aggregates |
| `state_4_proposal_refined` | Proposal Writer refines from review summary |
| `state_5_proposal_approval` | Human gate: engineer inspects proposal and approves/rejects |
| `state_6_workflow_complete` | Run completes after approval |

### Loop rule

`state_3 -> state_4 -> state_3`

until one of the following is true:

- review summary says proposal passes target score,
- engineer manually rejects and cancels,
- loop budget is exhausted and the run goes blocked.

This is enough to test:

- run compilation,
- real provider execution,
- fan-out/fan-in,
- artifact binding,
- stage transitions,
- approval flow,
- and resume.

---

## 7. Architecture

## 7.1 Main shape

```text
SwiftUI App
  -> ExecutionService
    -> WorkflowOrchestrator
      -> GooseAgentExecutor
        -> GooseSessionBridge
          -> GooseTransport (HTTP/SSE)
      -> ArtifactManager / ArtifactStorage
      -> Approval flow
```

The app remains the control plane.
Goose is still a runtime substrate, not the source of truth.

### Control plane responsibilities stay in app code

- run lifecycle,
- workflow state transitions,
- artifact indexing,
- approval gates,
- resume decisions,
- workspace isolation,
- and failure policy.

### Goose responsibilities

- model invocation,
- session lifecycle for one execution,
- tool use,
- streaming events,
- and final model output.

---

## 7.2 One `AgentExecution` = one Goose session

This proposal locks a very important decision early:

> **Every live `AgentExecution` gets its own isolated Goose session.**

That means:

- no session reuse across different agents,
- no session reuse across iterations,
- no invisible conversational carry-over,
- no dependence on provider memory to reconstruct workflow state.

Instead, each execution is fully defined by:

- the resolved agent definition,
- its prompt,
- input artifacts,
- execution context,
- and explicit workspace binding.

### Why this is the right tradeoff for the first live slice

It is slightly less efficient than long-lived sessions,
but much safer for:

- reproducibility,
- isolation,
- debugging,
- and future migration to a Rust backend / Temporal.

The system should be able to reconstruct why an agent produced an output **from stored inputs and artifacts**, not from hidden session drift.

---

## 7.3 Workspace contract

This proposal inherits and reinforces the workspace isolation rule:

> There is no implicit working directory in Chainworks.

All provider-backed executions receive:

```swift
RunWorkspace {
  runID
  workspaceRoot
  artifactRoot
  worktreeRoot == nil   // for Proposal 004
}
```

### Proposal 004 workspace policy

For the live proposal slice:

- `workspaceRoot` is run-scoped and isolated,
- `artifactRoot` is inside `workspaceRoot`,
- `worktreeRoot` remains `nil`,
- repo writes are forbidden,
- and provider tools are restricted to read-only inputs + artifact output path.

This is intentionally conservative.
The point is to make the first live runs boring in the best possible way.

---

## 7.4 Transport choice

**Locked decision:** Proposal 004 uses a Goose backend interface exposed over HTTP/SSE.

It does **not** use ACP as the primary transport for the first live slice.

### Why

Because the app already has an execution engine and persistent run model.
What it needs from Goose is:

- request/response execution,
- event streaming,
- session lifecycle,
- and a stable bridge for one execution at a time.

ACP is valuable later, especially for richer editor-style integration.
For the first provider-backed slice, it is extra protocol surface with no immediate product benefit.

---

## 7.5 Event flow

```text
ExecutionService.startRun()
  -> WorkflowOrchestrator.start()
    -> stage enters running
    -> GooseAgentExecutor.execute()
      -> GooseSessionBridge.createSession()
      -> GooseTransport.startRun()
      -> SSE stream emits events
      -> ExecutionEventBridge maps stream -> app events
      -> final outputs persisted as artifacts
    -> Orchestrator records AgentExecution result
    -> transitions evaluated
    -> approval gate or completion
```

### Event types we care about in Proposal 004

- session started,
- prompt submitted,
- tool call started,
- tool call finished,
- partial text chunk,
- final output received,
- execution failed,
- session closed.

The UI does not need to display every raw transport event.
It needs a clear, stable subset.

---

## 8. Agent execution contract for the live slice

## 8.1 `GooseAgentExecutor`

```swift
final class GooseAgentExecutor: AgentExecutor {
    let transport: GooseTransport
    let receiptBuilder: ExecutionReceiptBuilder
    let artifactStorage: ArtifactStorage

    func execute(
        agent: ResolvedAgent,
        task: AgentTask,
        inputs: [String: URL],
        outputDir: URL,
        context: ExecutionContext
    ) async throws -> AgentResult
}
```

### Responsibilities

1. Build the execution packet.
2. Create an isolated Goose session.
3. Bind prompt + task + input artifact references.
4. Stream execution events.
5. Persist raw transcript/receipt artifacts.
6. Extract declared output artifacts.
7. Return a structured `AgentResult`.

---

## 8.2 Execution packet shape

The executor should not give Goose a vague conversational prompt.
It should give it a structured packet.

### Packet sections

1. **System prompt**
   - agent role,
   - task boundaries,
   - output contract expectation,
   - forbidden behaviors.

2. **Run context**
   - run ID,
   - stage ID,
   - attempt number,
   - iteration,
   - workflow ID.

3. **Workspace context**
   - absolute `workspaceRoot`,
   - absolute `artifactRoot`,
   - explicit statement that no implicit cwd is allowed.

4. **Input artifacts**
   - named artifact list,
   - absolute paths,
   - short summaries if available.

5. **Task directive**
   - exact `task` from workflow,
   - expected outputs,
   - stop condition.

This is slower to author once, but it pays off the first night you need to understand why an agent went sideways.

---

## 8.3 Output rule

Proposal 004 keeps output handling strict.

An execution is only considered successful if:

1. the provider returns normally,
2. declared required outputs are present,
3. output artifacts are persisted,
4. artifact formats can be identified,
5. and the final `AgentResult.status` is `completed`.

### Required output artifacts for the slice

| Agent | Expected primary output |
|---|---|
| `proposal_writer` | `proposal_current.md` |
| `proposal_reviewer_product_owner` | `proposal_review_po.json` |
| `proposal_reviewer_ux` | `proposal_review_ux.json` |
| `proposal_reviewer_ui` | `proposal_review_ui.json` |
| `proposal_reviewer_architect` | `proposal_review_arch.json` |
| `lead_orchestrator` (aggregate) | `proposal_review_summary.json` |
| `proposal_writer` (refine pass) | `proposal_revision_summary.json` |

If the provider produced text but not the required outputs, the stage should fail loudly.
Silent success is worse than a visible crash here.

---

## 9. Minimal provider strategy

The long-term architecture supports per-agent backends and effort levels.
But that is not the fastest way to get a live run working.

### Proposal 004 strategy

Introduce an app-scoped **live execution override**:

```swift
struct LiveExecutionOverride {
    let enabled: Bool
    let provider: String
    let model: String
    let effort: String
}
```

If enabled, all agents in the proposal-loop live workflow use the same provider/model/effort during Proposal 004.

### Why this is worth it

It reduces the first live slice from:

- multi-provider routing,
- provider-specific event mapping,
- provider-specific structured output quirks,
- and provider-specific failure handling

to one problem:

- can the app run a real workflow end-to-end?

Once that answer is yes, the system can broaden.

---

## 10. UI surface

Proposal 004 should add only the UI necessary to make the live slice usable.

## 10.1 Start Run Sheet

New controls:

- workflow selector,
- execution mode:
  - simulated,
  - live,
- provider override selector (dev-only if needed),
- summary of resolved live agents.

### Example

```text
Start Run
  Workflow: Proposal Loop (Live)
  Mode: Live
  Provider override: Claude / high
  Agents: 6 resolved
  Safety: read-only workspace, no git/release side effects
```

---

## 10.2 Run Progress View

New live-specific behavior:

- streaming status updates,
- current agent activity,
- recent tool activity,
- visible session ID / execution receipt link,
- live cost accumulation if available.

The view must still read from `Run`, `StageExecution`, `AgentExecution`, and `Artifact`.
It must not become a second runtime.

---

## 10.3 Artifact Inspector

Proposal 004 adds special handling for these artifacts:

- rendered markdown proposal draft,
- pretty-printed reviewer JSON,
- summary JSON,
- raw transcript / receipt artifact,
- provider error payload if execution failed.

That last one matters more than it looks.
When things break, people go straight to the raw receipt.

---

## 11. Suggested model additions

Proposal 004 can ship with minimal schema changes, but these additions are highly recommended.

## 11.1 `AgentExecution`

Add:

```swift
var providerSessionID: String?
var providerRequestID: String?
var transcriptArtifactPath: String?
var resolvedBackendProfileID: String?
```

### Why

- `gooseSessionID` is useful but too specific as the only durable link,
- request/session IDs are invaluable when debugging transport failures,
- transcript path makes execution inspection easier,
- and backend profile provenance helps explain behavior later.

## 11.2 Persist exact consumed inputs

Recommended shape:

```swift
var consumedInputArtifactNamesJSON: Data?
```

or a dedicated relation later.

### Why

If the proposal loop iterates several times, it becomes very useful to know exactly which review artifacts and proposal draft the writer consumed on that attempt.

This is not strictly required to feel the first live run.
It becomes required surprisingly quickly once you try to understand bad runs.

---

## 12. Testing strategy

## 12.1 Unit tests

### Goose executor

- `testGooseExecutorCreatesSession()`
- `testGooseExecutorStreamsEvents()`
- `testGooseExecutorPersistsReceiptArtifact()`
- `testGooseExecutorFailsWhenRequiredOutputsMissing()`
- `testGooseExecutorReturnsAgentResult()`

### Session bridge

- `testSessionBridgeBindsWorkspaceExplicitly()`
- `testSessionBridgeRejectsImplicitCWD()`
- `testSessionBridgeUsesOneSessionPerExecution()`

### Live workflow slice

- `testLiveProposalWorkflowCompiles()`
- `testLiveProposalWorkflowUsesExpectedAgents()`
- `testReviewFanoutParallelismIsRecordedCorrectly()`

---

## 12.2 Integration tests

### Real backend smoke tests

These are allowed to be opt-in or environment-gated.

- `testLiveProposalDraftSmoke()`
- `testLiveReviewFanoutSmoke()`
- `testLiveProposalRunToApprovalSmoke()`

### Resume tests

- kill app during safe live review stage -> resume allowed,
- kill app while waiting approval -> approval gate restored,
- partial provider execution with no final outputs -> run blocked or failed, not silently resumed.

---

## 12.3 Manual product script

One human should be able to run this in under 10 minutes:

1. Launch app.
2. Create idea.
3. Start live proposal loop run.
4. Watch proposal draft appear.
5. Watch reviewers complete.
6. Read `proposal_current.md`.
7. Approve.
8. Inspect receipts and final artifacts.

If this works twice in a row without strange state leakage, Proposal 004 did its job.

---

## 13. Acceptance criteria

### Runtime

- [ ] `GooseAgentExecutor` implements `AgentExecutor`
- [ ] Each live `AgentExecution` uses its own isolated Goose session
- [ ] Workspace is passed explicitly; no execution relies on implicit cwd
- [ ] Live executions persist transcript/receipt artifacts
- [ ] Required declared outputs are validated before success is recorded

### Workflow

- [ ] `proposal-loop-live.yaml` compiles into a valid `RunPlan`
- [ ] Real proposal draft is produced and stored
- [ ] Real review fan-out produces four reviewer artifacts
- [ ] Real review summary is produced
- [ ] Refinement loop can run at least one additional iteration
- [ ] Human approval gate pauses execution and requires explicit engineer action
- [ ] Approval continues run to completion

### Resume / safety

- [ ] Interrupted safe live run can resume
- [ ] Waiting approval is restored on relaunch
- [ ] No git/release side effects are possible in Proposal 004 mode
- [ ] Path guard blocks filesystem actions outside `workspaceRoot`

### UI

- [ ] Start Run sheet can launch a live run
- [ ] Run Progress view reflects live agent state
- [ ] Artifact Inspector can open proposal markdown and review JSON
- [ ] Transcript/receipt artifacts are accessible from the UI

### Product checkpoint (PROD-PA-004)

- [ ] One engineer can go from idea creation to approved proposal in a real provider-backed run inside the app
- [ ] Total live slice setup + run time is practical enough to repeat during development
- [ ] At least one live run survives an intentional interruption/resume test
- [ ] The product now feels like a workflow tool, not just a definition viewer

---

## 14. Locked decisions

| ID | Decision | Rationale |
|---|---|---|
| ARCH-026 | Proposal 004 is **proposal-loop only** | Fastest path to first real value without mixing destructive side effects |
| ARCH-027 | One `AgentExecution` = one Goose session | Isolation, reproducibility, easier debugging |
| ARCH-028 | Use Goose over HTTP/SSE, not ACP | Smaller integration surface for first live slice |
| ARCH-029 | Live Proposal 004 runs are read-only with respect to repo/worktree | Avoids mixing provider uncertainty with destructive file operations |
| ARCH-030 | No reliance on session memory; state is reconstructed from artifacts | Keeps runs explainable and future-proof |
| ARCH-031 | Temporary single-provider override is allowed for Proposal 004 | Reduces moving parts and gets to a live run faster |
| ARCH-032 | Transcript/receipt artifacts are first-class outputs of live execution | Debuggability is part of the product, not an afterthought |

---

## 15. Risks

| Risk | Impact | Mitigation |
|---|---|---|
| Goose session/workspace leakage between runs | Cross-project contamination | Explicit `RunWorkspace`, one session per execution, path guard, no implicit cwd |
| Provider does not reliably produce declared outputs | False-success or unusable artifacts | Treat missing required outputs as failure |
| Streaming disconnect mid-execution | Orchestrator left in ambiguous state | Persist receipt incrementally, fail or block clearly, never mark completed without final outputs |
| Review fan-out triggers rate limits or provider flakiness | Slow or failed runs | Keep first slice narrow, optionally serialize for debugging, add retry later |
| Output JSON shape drifts from contract | Broken transition evaluation | Contract validation on persisted artifacts before transition evaluation |
| Live runs are too expensive to iterate with | Slows development | Provider override, small workflow, limited agent set |
| Resume semantics on partial live runs are unclear | User distrust | Safe-stage resume only; ambiguous partial executions go to blocked/failed |
| The slice works but still feels slow or awkward | Product disappointment | Keep UI thin, focus on clarity of progress/artifacts over perfect polish |

---

## 16. What this proposal deliberately postpones

After Proposal 004 succeeds, the next layers become much easier to place.

### Likely next proposals after 004

1. **Implementation Slice**
   - real code writer,
   - writable dedicated worktree,
   - implementation review path.

2. **Release Slice**
   - commit/push,
   - archive/distribute,
   - hard side-effect gates.

3. **Provider Matrix**
   - backend profile routing,
   - per-agent models/effort,
   - better cost accounting.

4. **Forge Steward activation**
   - once enough live runs exist to generate meaningful SDLC telemetry.

Proposal 004 should not try to smuggle these in early.

---

## 17. Execution plan

| Day | Deliverable |
|---|---|
| Day 1 | `proposal-loop-live.yaml`, live scope decisions, runtime contracts |
| Day 2 | `GooseTransport` + session bridge skeleton |
| Day 3 | `GooseAgentExecutor` + receipt persistence |
| Day 4 | Start Run sheet live mode + run progress wiring |
| Day 5 | Artifact inspection + approval gate on real run |
| Day 6 | resume/safety hardening + smoke tests |
| Day 7 | manual end-to-end pass + polish |

This schedule is intentionally short.
The point is not to finish the whole system.
The point is to get the first real run through your hands.

---

## 18. What Proposal 004 enables

After Proposal 004, Chainworks can do something it could not do before:

> Take a real idea, run a real agent workflow through a real provider, produce real artifacts, pause for real human judgment, and finish with a durable run record.

That is the moment the app stops being only architecture and starts becoming a tool.

---

## 19. Suggested file structure

```text
Chainworks Forge/
  Engine/
    GooseTransport.swift              <- NEW
    GooseSessionBridge.swift          <- NEW
    GooseAgentExecutor.swift          <- NEW
    ExecutionEventBridge.swift        <- NEW
    ExecutionReceiptBuilder.swift     <- NEW

  DSL/
    proposal-loop-live.yaml           <- NEW fixture/workflow

  Views/
    StartRunSheet.swift               <- UPGRADED: simulated/live mode
    RunProgressView.swift             <- UPGRADED: live stream status
    ArtifactInspectorView.swift       <- UPGRADED: transcript/receipt support

  Tests/
    GooseAgentExecutorTests.swift     <- NEW
    GooseSessionBridgeTests.swift     <- NEW
    LiveProposalWorkflowTests.swift   <- NEW
    LiveSmokeTests.swift              <- NEW (env-gated)
```

---

## 20. Final recommendation

Do not turn Proposal 004 into a grand integration phase.
Keep it narrow enough that, by the end of it, you can sit down, enter an idea, hit Start, and feel the system move.

That first live run is worth more than another week of elegant diagrams.
