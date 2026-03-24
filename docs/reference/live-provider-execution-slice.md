# Live Provider Execution Slice

This document is the permanent reference for the implemented live proposal-loop slice that was originally introduced as Proposal 004.

It describes the current fixture-backed live runtime contract, the app surfaces that make the slice usable, the artifact and safety guarantees, and the boundary between the implemented slice and later follow-on work.

## Status

- State: implemented and proven against real Goose.app (2026-03-24)
- Scope owner: current app runtime and UI shell
- Backing workflow: `examples/workflows/proposal-loop-live.yaml`
- Live transport: GooseServerTransport (real goosed API) or FixtureGooseTransport (deterministic tests)

## Related Docs

- [runtime-contract.md](runtime-contract.md)
- [workspace-isolation-risk.md](workspace-isolation-risk.md)
- [architecture-decisions.md](architecture-decisions.md)
- [workflow-execution-engine.md](workflow-execution-engine.md)
- [goose-server-transport.md](goose-server-transport.md)

## 1. Purpose

The live provider execution slice is the first app-facing non-simulated execution path in Chainworks Forge.

Its job is narrow on purpose:

- run the proposal loop through the same execution engine the rest of the product uses
- enforce explicit workspace and safety rules at runtime
- persist durable proposal, review, summary, and receipt artifacts
- pause at approval with enough context for a human decision
- resume safely after interruption

This slice proves the control-plane model without introducing writable repo side effects.

## 2. Scope Boundary

### In scope

- live-mode execution for the proposal loop only
- `GooseAgentExecutor`, `GooseSessionBridge`, `GooseTransport`, and `ExecutionEventBridge`
- fixture-backed live transport used by the app and tests
- app-launched Start Run, Run Progress, approval, and artifact-inspection surfaces
- durable transcript, receipt, proposal, review, and summary artifacts
- fail-closed read-only launch policy
- structured artifact validation before success and transition evaluation
- safe resume for waiting-approval and other non-destructive states

### Out of scope

- writable implementation worktrees
- git, release, or publish side effects
- code-writing stages
- per-agent provider routing
- ACP as the primary transport

## 3. Runtime Shape

```text
SwiftUI App
  -> ExecutionService
    -> WorkflowOrchestrator
      -> GooseAgentExecutor
        -> GooseSessionBridge
          -> GooseServerTransport, GooseTransport, or FixtureGooseTransport
      -> ArtifactManager / ArtifactStorage
      -> Approval flow
```

The app remains the control plane. Goose is the execution substrate, not the source of truth.

Control-plane responsibilities stay in app code:

- run lifecycle
- workflow transitions
- approval state
- artifact indexing
- resume decisions
- workspace isolation
- failure policy

Provider-facing responsibilities stay in the live executor path:

- session lifecycle
- prompt packet construction
- streaming events
- tool execution
- final model output

## 4. Runtime Contract

### 4.1 One `AgentExecution` equals one live session

Every live `AgentExecution` gets its own isolated session.

That means:

- no session reuse across agents
- no session reuse across iterations
- no hidden provider memory dependency
- no cross-run conversational carry-over

The only durable explanation for an output is the stored inputs, packet, artifacts, and metadata.

### 4.2 Explicit workspace, never implicit cwd

Every live execution is bound to an explicit `RunWorkspace`.

For this slice:

- `workspaceRoot` is run-scoped
- `artifactRoot` is inside that workspace
- `worktreeRoot` remains `nil`
- repo writes are forbidden
- no execution may rely on implicit cwd

This contract is enforced both in request construction and in downstream path guards.

### 4.3 Fail-closed read-only policy

Live launch is allowed only when the backend acknowledges the required read-only execution policy.

The launch packet carries:

- run and stage identity
- workspace roots
- resolved permission policy
- explicit no-git / no-release / no-repo-write rules
- the acknowledgement field required for launch

If acknowledgement is absent, launch fails before the first live stage starts.

### 4.4 Structured artifact validation before transitions

File existence alone is not enough for success.

The runtime requires:

- declared required outputs to exist
- artifacts to be persisted before success is recorded
- reviewer and aggregate-review JSON to pass structural validation
- malformed structured artifacts to fail the stage before transition evaluation

The proposal loop therefore consumes validated structured review artifacts, not opportunistic JSON reads.

## 5. Live Proposal Workflow

The live slice is intentionally narrow and runs only the proposal loop workflow:

- `lead_orchestrator`
- `proposal_writer`
- `proposal_reviewer_product_owner`
- `proposal_reviewer_ux`
- `proposal_reviewer_ui`
- `proposal_reviewer_architect`

Primary workflow file:

- `examples/workflows/proposal-loop-live.yaml`

Expected primary outputs:

| Agent | Primary artifact |
|---|---|
| `proposal_writer` | `proposal_current.md` |
| `proposal_reviewer_product_owner` | `proposal_review_po.json` |
| `proposal_reviewer_ux` | `proposal_review_ux.json` |
| `proposal_reviewer_ui` | `proposal_review_ui.json` |
| `proposal_reviewer_architect` | `proposal_review_arch.json` |
| aggregate orchestrator step | `proposal_review_summary.json` |
| refinement pass | `proposal_revision_summary.json` |

Approval pauses the run at a stable decision point and resumes toward a stable inspectable outcome.

## 6. Persisted Metadata and Artifacts

The live slice persists both user-facing outputs and provider-facing execution traces.

Key `AgentExecution` metadata includes:

- `providerSessionID`
- `providerRequestID`
- `transcriptArtifactPath`
- `resolvedBackendProfileID`
- `consumedInputArtifactNamesJSON`

Live artifact classes include:

- proposal drafts
- reviewer JSON
- aggregate review summary
- refinement summary
- raw execution transcript
- structured receipt
- provider error payload, when present

The point is to make a live run inspectable after the fact without relying on hidden session state.

## 7. App Surfaces

### 7.1 Start Run

The Start Run surface exposes:

- workflow selection
- simulated vs live mode
- resolved live-agent summary
- safety framing in product language
- explicit missing-runtime guidance when live mode cannot start

### 7.2 Run Progress

The live run surface keeps the important state above the fold:

- current phase
- live agent activity
- approval state
- decision context
- spend or explicit unavailable state
- shortcuts to the latest proposal, review summary, and receipt artifacts

### 7.3 Approval and artifact inspection

At approval time, the operator can inspect:

- current proposal draft
- latest review summary
- latest refinement summary
- transcript / receipt artifacts

The artifact inspector renders both user-facing markdown and raw structured/provider-facing outputs.

### 7.4 Resume

Waiting-approval state is restored on relaunch.

The live slice is allowed to resume only for safe interrupted states. Ambiguous or partial provider executions must block or fail clearly rather than auto-resume silently.

## 8. Source Anchors

Primary implementation files:

- `Chainworks Forge/Engine/GooseTransport.swift`
- `Chainworks Forge/Engine/FixtureGooseTransport.swift`
- `Chainworks Forge/Engine/GooseSessionBridge.swift`
- `Chainworks Forge/Engine/GooseAgentExecutor.swift`
- `Chainworks Forge/Engine/ExecutionEventBridge.swift`
- `Chainworks Forge/Engine/ExecutionReceiptBuilder.swift`
- `Chainworks Forge/Engine/ExecutionService.swift`
- `Chainworks Forge/Engine/WorkflowOrchestrator.swift`
- `Chainworks Forge/Engine/ResumeManager.swift`
- `Chainworks Forge/Views/IdeaListView.swift`
- `Chainworks Forge/Models/AgentExecution.swift`

Primary verification files:

- `Chainworks ForgeTests/GooseAgentExecutorTests.swift`
- `Chainworks ForgeTests/GooseSessionBridgeTests.swift`
- `Chainworks ForgeTests/LiveProposalWorkflowTests.swift`
- `Chainworks ForgeTests/EndToEndTests.swift`
- `Chainworks ForgeTests/OrchestratorTests.swift`
- `Chainworks ForgeTests/ResumeManagerTests.swift`
- `Chainworks ForgeTests/WorkspaceIsolationTests.swift`
- `Chainworks ForgeUITests/Chainworks_ForgeUITests.swift`

## 9. Verification Standard

The live slice is proven at two levels:

**Fixture-backed proof:**
- Start Run sheet launches the live slice from inside the app
- Run progress, approval, and stable outcome are reachable
- Artifact inspector opens proposal and receipt-style artifacts
- At least one non-happy path is proven
- Waiting-approval restore is proven on relaunch

**Real Goose proof (2026-03-24):**
- `GooseServerTransport` connected to real Goose.app on `https://127.0.0.1:51200`
- Session `20260324_28` created via `/agent/start` + `/agent/update_provider`
- Prompt submitted, SSE response received: `CHAINWORKS_PROOF_OK`
- Full `GooseAgentExecutor` pipeline executed with receipt generation
- Evidence: `docs/evidence/live_goose_connection_proof.json`

See [goose-server-transport.md](goose-server-transport.md) for transport details.

## 10. Follow-On Boundary

This document covers the implemented live runtime contract for the proposal loop.

The transport layer supports both fixture-backed and real Goose server execution. Later operator-shell, provider-settings, and delivery-slice work builds on top of this baseline rather than redefining it.
