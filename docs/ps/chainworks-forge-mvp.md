# Chainworks Forge MVP

## 1. Terms and definitions

- **Idea**: a user-initiated unit of work described as text in the app, optionally with a referenced file.
- **Workflow**: a YAML-defined execution graph that describes stages, agents, approvals, and transitions.
- **Stage**: one logical step in a workflow, such as proposal generation, review, implementation, or pre-push validation.
- **Run**: one execution instance of a workflow for one idea.
- **RunPlanSnapshot**: an immutable compiled snapshot of workflow, agent bindings, backend profiles, and artifact paths used by a run.
- **WorkflowVersion**: the exact workflow definition revision compiled into a run.
- **AgentCatalogVersion**: the exact agent catalog revision resolved by the workflow at run start.
- **Agent**: a specialized worker bound to a role, provider, permissions, and output contract.
- **Approval gate**: a workflow-defined pause where the engineer must review and explicitly continue.

## 2. Business context

This PS defines the first implementation slice. Product vision and positioning live separately in `/Users/user/Documents/Chainworks Forge/docs/research/chainworks_core_idea.md`.

Chainworks Forge is a macOS application for a single engineer who wants to orchestrate multi-agent engineering workflows from a local desktop control plane. The immediate goal is not to hardcode one scenario, but to build a **workflow constructor/executor** that can load a `workflow.yaml`, execute it, and visualize progress in a way that stays understandable and controllable.

The first product problem is operational clarity. A single engineer needs to see which ideas are currently being processed, which workflow chain each idea is moving through, what the current stage is, which agents are active, and what each agent has produced. The app must also surface completed work as a report rather than leaving the engineer to reconstruct what happened from raw logs.

There are also control requirements. The engineer wants to intervene at three explicit points: after the first proposal is produced, before implementation starts, and before push/distribution side effects happen. Those approval points must be represented in the workflow itself through `approval: required` rather than hardcoded in the UI.

### 2.1 Hypothesis

If Chainworks Forge provides a local SwiftUI control plane that loads YAML workflows, executes them across multiple providers, and exposes clear approval gates plus agent output inspection, then one engineer will be able to run agent workflows with higher confidence and lower coordination overhead than with ad hoc chat sessions and manual orchestration.

Success metric: 50% reduction in manual orchestration time per idea.
Baseline: ~45 minutes manual orchestration time per idea (measured 2026-03-22: one sample idea through proposal draft, review cycle, implementation initiation, and artifact collection using ad-hoc chat sessions and manual file management).
Measurement plan: compare a fixed set of ideas executed manually vs through Chainworks Forge MVP, measuring time to proposal approval, time to implementation approval, and time to final release decision.

Leading indicator (Proposal 001 scaffold): verified 2026-03-22 via automated UI test `testProductCheckpointScaffoldFlowUnder60Seconds` — launch app, visit Ideas tab (CRUD scaffold), Agent Catalog (13 agents parsed + validation summary), Workflow Inspector (12 states parsed + validation summary), create idea — total flow completes in < 60 seconds. Evidence preserved in xcresult bundle as PROD-PA-001 attachments (screenshots + timing record).

### 2.2 Definition Of Done

- The app can load and execute a workflow defined in YAML for one idea.
- The app supports exactly one active run per idea in MVP.
- The main UI shows a list of ideas currently being processed.
- Each idea view shows the workflow chain and clearly indicates the current stage.
- The app shows active agents and lets the engineer open an agent to inspect raw logs, markdown summaries, and structured outputs.
- The app stores job/run state in SwiftData.
- The workflow model supports explicit approval gates, including the three required checkpoints.
- Completed ideas expose a readable report with summary, time, and cost.
- Multi-provider execution is supported from the first version, with Codex, Claude Code, and Gemini as the MVP provider set.
- Interrupted runs are resumed automatically on app launch.

## 3. Requirements

### 3.1 Functional requirements

- The engineer must be able to describe an idea in text in the app, optionally attach or reference a file, select a workflow definition, and start a run so that one idea maps to one workflow execution.
- The system must parse `workflow.yaml` and execute its stages so that stage transitions are driven by the workflow definition rather than hardcoded flows.
- The system must compile each started run into an immutable `RunPlanSnapshot` so that resume and reporting use the same workflow, agent bindings, backend profiles, and artifact paths that were active at run start.
- The system must resolve workflow stage agent references through the agent catalog and backend profiles so that workflow topology and agent policy remain separate concerns.
- The system must enforce exactly one active run per idea in MVP so that workflow ownership and UI state stay unambiguous.
- The system must persist ideas, runs, stages, active agents, and approvals in SwiftData so that app state survives restart and can be inspected later.
- The system must persist only metadata, indexes, statuses, and artifact references in SwiftData so that large logs, diffs, markdown, and structured payloads remain in the artifact store on disk.
- The system must show a list of in-progress ideas so that the engineer can see all currently active work in one place.
- The system must show, for each idea, the workflow chain and current stage so that progress is observable without opening raw artifacts.
- The system must show a list of active agents and let the engineer open each agent so that raw logs, markdown summaries, and structured outputs can be inspected during execution.
- The system must pause at workflow-defined approval gates using `approval: required` so that the engineer can approve or stop execution after the initial proposal, before implementation, and before push/distribution.
- The system must show completed ideas with a generated work report containing summary, elapsed time, and cost so that the engineer can review the result of the run after completion.
- The system must support multiple providers from the first version so that agents can run against different model backends in one workflow, with Codex, Claude Code, and Gemini required in v1.
- The system must treat workflow actions such as push or distribution as workflow-defined capabilities so that examples like Connect remain optional and not product-defining.
- The system must automatically resume interrupted runs on app launch so that the engineer does not need to restore workflow progress manually.
- The system must never silently auto-resume stages with external side effects; those stages must return to `waiting_approval` or `blocked`.

### 3.2 Non-functional requirements

- The UI must make current run state visible within one navigation step from the main screen.
- Workflow execution state must be recoverable and automatically resumed after app restart from SwiftData with no silent loss of stage status.
- Approval gates must be explicit, blocking, and auditable in stored run history.
- Provider integration must support adding more backends without rewriting the workflow model.
- Agent output retrieval for an active run should open in `[TBD]` seconds or less on a typical local machine.
- Completed-run reporting must retain at least summary, elapsed time, and cost for each finished idea.
- Resumed runs must continue from the frozen run snapshot, not from the latest YAML files on disk.

## 4. HLD

Chainworks Forge MVP consists of a SwiftUI macOS client, a SwiftData persistence layer, a workflow loader/compiler for YAML definitions, a runtime adapter layer for multiple providers, and a run monitor UI for ideas, stages, agents, approvals, and final reports.

At a high level:

- **Ideas and runs layer**: stores idea records, workflow bindings, a single active run per idea, stage state, approval history, and completion reports in SwiftData.
- **Workflow execution layer**: reads `workflow.yaml`, resolves stages/agents/approval gates, and drives execution.
- **Provider layer**: routes agent calls to multiple backends behind a common internal contract, with Codex, Claude Code, and Gemini mandatory in v1.
- **UI layer**: shows in-progress ideas, workflow stage chains, active agents, raw/markdown/structured output views, approvals, and completed reports.
- **Artifact/report layer**: stores or references outputs needed for agent inspection, final reporting, and automatic run recovery.

### 4.1 Teams

- One engineer owns product definition, workflow modeling, app implementation, and operational use in the MVP phase.

### 4.2 Execution model

Execution in MVP follows this chain:

- idea -> run -> compiled `RunPlanSnapshot` -> stage executions -> agent executions -> artifacts -> approvals -> final report
- `RunPlanSnapshot` freezes `WorkflowVersion`, `AgentCatalogVersion`, backend-profile bindings, permissions, artifact paths, and runtime settings at run start
- workflow stages resolve agent references through the agent catalog rather than embedding provider/model policy inline
- resume always uses the stored snapshot, never the latest YAML files on disk

### 4.3 State machines

The implementation must track separate status machines for:

- **Run**: `pending`, `ready`, `running`, `waiting_approval`, `blocked`, `completed`, `failed`, `cancelled`
- **Stage**: `pending`, `ready`, `running`, `waiting_approval`, `blocked`, `completed`, `failed`, `skipped`
- **Agent execution**: `pending`, `ready`, `running`, `completed`, `failed`, `cancelled`, `skipped`
- **Approval**: `pending`, `requested`, `granted`, `rejected`, `expired`
- **Side effect**: `pending`, `armed`, `running`, `completed`, `failed`, `blocked`

### 4.4 Artifact model

Artifacts are first-class runtime objects, not just log by-products.

- canonical examples include `proposal.md`, `review.json`, `audit.md`, `patch.diff`, `run-report.json`
- every artifact must record provenance: run, stage, agent, provider, model, effort, and creation time
- artifacts are immutable once written for a stage attempt; new attempts create new artifacts rather than rewriting history
- SwiftData stores artifact metadata, paths, checksums, and aggregates; artifact content lives on disk

### 4.5 Resume / retry policy

- safe local stages may auto-resume on launch from the frozen run snapshot
- workflow-defined approval stages resume into `waiting_approval`, not into silent execution
- stages with external side effects such as push, publish, and distribution never auto-resume silently
- retries are bounded by workflow/stage policy and must create new stage-attempt records and new artifacts

### 4.6 Out of scope for MVP

- provider integrations beyond Codex, Claude Code, and Gemini
- parallel write-capable agents in the same worktree
- distributed workers or cloud execution
- cloud sync and multi-user orchestration
- silent auto-resume of external side-effect stages

## 5. Open questions

- What exact file types should be supported as optional idea attachments in v1?
- Should cost in the completed report be shown only as total run cost, or also as a per-agent / per-stage breakdown?
- When the app auto-resumes and the run is currently waiting at an approval gate, should it reopen directly into that approval screen or only mark the run as blocked?
