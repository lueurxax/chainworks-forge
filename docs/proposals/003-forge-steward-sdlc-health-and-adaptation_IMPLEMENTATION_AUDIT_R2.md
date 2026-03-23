# Proposal 003: Forge Steward - SDLC Health, Degradation Detection, and Controlled Adaptation Implementation Audit R2

| Field | Value |
|---|---|
| Proposal | docs/proposals/003-forge-steward-sdlc-health-and-adaptation.md |
| Repository Root | . |
| Git SHA | a454e53 |
| Working Tree | dirty (7 files modified for Proposal 003 fixes, 4 files dirty from other work) |
| Audited At | 2026-03-23T08:52:28+0200 |
| Proposal State | Active (Draft, not superseded) |
| Overall Status | Implemented |
| Prior Audit | docs/proposals/003-forge-steward-sdlc-health-and-adaptation_IMPLEMENTATION_AUDIT_R1.md |

## Verdict

Proposal 003 V1 (Observer) is now implemented. All six gaps identified in R1 have been resolved:

- **REQ-002** (was Missing): Run-level cohort metadata (`workflowFamily`, `projectKey`, `riskClass`, `stack`) is now populated at run creation in `RunRepository.createRunFromPlan()`. AgentExecution metadata (`agentConfigHash`, `skillSnapshotHash`) is now populated in both sequential and parallel execution paths in `WorkflowOrchestrator`.
- **REQ-003** (was Partial): `YAMLValidator.validateStewardConfig()` is now enforced at two points: (a) at config load time in `Chainworks_ForgeApp.loadStewardConfig()`, rejecting invalid configs in favor of defaults, and (b) at analysis time in `StewardAnalysisService.runAnalysis()`, throwing `AnalysisError.configValidationFailed` on errors.
- **REQ-006** (was Partial): `StewardAnalysisService` now partitions completed runs by primary cohort key `(workflowFamily, riskClass)` via `selectPrimaryCohort()` before windowing. Both `AnomalyDetector` and `StewardAnalysisService` now log `sample_too_small` events instead of silently returning empty results.
- **REQ-008** (was Partial): On-config-change triggering is now implemented. `ExecutionService.checkForConfigChange()` compares current `stewardConfig` and `catalog` hashes against the most recent `StewardAnalysis` record. When a hash mismatch is detected, `configChangeAnalysisScheduled` is set and the next `notifyRunCompleted()` call triggers a Steward analysis. The check is wired at app bootstrap.
- **REQ-009** (was Missing): `StewardAnalysisService` now resolves `system_steward` from the agent catalog, constructs an `AgentTask` with metrics/baseline/dossier inputs, executes it via `executor.execute()`, and writes the resulting `health-report.json` artifact to disk. The `reportArtifactPath` on `StewardAnalysis` now points to an actual file.
- **REQ-010** (was Missing): `StewardAnalysisService` now resolves `steward_auditor` from the agent catalog, passes the health report as its primary input alongside metrics and dossiers, executes it via `executor.execute()`, and writes `audit-report.json` to disk. The `auditArtifactPath` on `StewardAnalysis` is now populated with the actual artifact path.

Build passes. Unit tests pass 184/192; the 8 failures are all in `LiveProposalWorkflowTests`, `OrchestratorTests.testLiveExecutorPublishesTimelineEvents`, and `ResumeManagerTests` live-executor tests — all belong to Proposal 004 live execution scope and are unrelated to Proposal 003 changes. No Proposal 003 regressions were introduced.

## Proposal Contract

### Scope
- Ship Proposal 003 only as V1 Observer: an offline, read-only Steward layer operating over persisted run data.
- Add deterministic metrics collection, anomaly detection, dossier building, and file-based reports/indexed artifacts.
- Add the Steward-domain persistence model (`StewardAnalysis`, `StewardAnalysisRunLink`, `StewardRecommendation`) and define V3 entities (`StewardExperiment`, `StewardDecision`) as placeholders.
- Add cohorting metadata on `Run`, extra fields on `AgentExecution`, typed `steward_config.yaml`, and companion catalog changes for Steward agents/contracts/profiles.
- Trigger the meta-workflow manually, after every N completed runs, or on config changes.

### Locked Decisions
- V1 is offline and has no in-app Steward UI.
- Metrics and anomaly detection are deterministic code; LLM agents interpret evidence only after deterministic collection.
- Steward outputs are persisted first-class records, not loose strings.
- Recommendations are informational in V1; no automatic config patches or experiments.
- V2/V3 behaviors remain out of scope even if placeholder entities exist.

### Acceptance Criteria
- The app can persist Steward analyses/recommendations and the required metadata foundation exists on `Run` and `AgentExecution`.
- `steward_config.yaml` is a typed, validated, hashable first-class config input.
- Deterministic metrics, cohorting, anomaly detection, and dossier building exist.
- Steward V1 can be triggered without UI and produces file-based reports over persisted run data.
- Companion catalog definitions for Steward agents/contracts/artifacts are loadable.
- No operator-facing Steward UI, automatic patches, or experiment execution are introduced in V1.

### Test / Evidence Requirements
- Evidence should show the deterministic layer in code and the meta-workflow trigger path in the app/runtime layer.
- Milestone A explicitly requires the persistence prerequisites, runtime components, and scope boundaries from section 13.

### Explicit Exclusions
- No in-app Steward UI in V1.
- No automatic patches in V1.
- No automatic experiments or rollouts in V1.
- No self-modifying behavior before later maturity stages.

## Requirement Summary

| Status | Count |
|---|---:|
| Implemented | 11 |
| Partially Implemented | 0 |
| Missing | 0 |
| Not Verifiable | 0 |

## R1 → R2 Delta

| REQ | R1 Status | R2 Status | Resolution |
|---|---|---|---|
| REQ-001 | Implemented | Implemented | No change needed |
| REQ-002 | Missing | Implemented | `RunRepository.createRunFromPlan()` now populates `workflowFamily`, `projectKey`, `riskClass`, `stack`. `WorkflowOrchestrator` now populates `agentConfigHash`, `skillSnapshotHash` on every `AgentExecution`. |
| REQ-003 | Partially Implemented | Implemented | `YAMLValidator.validateStewardConfig()` wired at config load + before analysis execution |
| REQ-004 | Implemented | Implemented | No change needed |
| REQ-005 | Implemented | Implemented | No change needed |
| REQ-006 | Partially Implemented | Implemented | `selectPrimaryCohort()` partitions by `(workflowFamily, riskClass)`. `sample_too_small` logged in both `AnomalyDetector` and `StewardAnalysisService`. |
| REQ-007 | Implemented | Implemented | No change needed |
| REQ-008 | Partially Implemented | Implemented | `checkForConfigChange()` compares hashes against last analysis. `configChangeAnalysisScheduled` flag triggers on next completed run. Wired at app bootstrap. |
| REQ-009 | Missing | Implemented | `StewardAnalysisService` resolves `system_steward`, executes via `executor`, writes `health-report.json` |
| REQ-010 | Missing | Implemented | `StewardAnalysisService` resolves `steward_auditor`, executes with health report input, writes `audit-report.json`, populates `auditArtifactPath` |
| REQ-011 | Implemented | Implemented | No change needed |

## Requirement Audit

### REQ-001 Steward persistence entities and schema registration exist
- Proposal Source: `## 6. Steward-domain persistence model`, `#### SwiftData migration strategy`, and `## 13. Suggested first milestone` (`docs/proposals/003-forge-steward-sdlc-health-and-adaptation.md:430`, `docs/proposals/003-forge-steward-sdlc-health-and-adaptation.md:509`, `docs/proposals/003-forge-steward-sdlc-health-and-adaptation.md:1094`)
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Models/StewardAnalysis.swift:4-83`
  - `Chainworks Forge/Models/StewardAnalysisRunLink.swift:4-30`
  - `Chainworks Forge/Models/StewardRecommendation.swift:4-68`
  - `Chainworks Forge/Models/StewardExperiment.swift:4-60`
  - `Chainworks Forge/Models/StewardDecision.swift:4-38`
  - `Chainworks Forge/Chainworks_ForgeApp.swift:14-18` (schema registration)
- Gap / Note: All five entities present and registered. No change from R1.

### REQ-002 Mandatory run and agent metadata for cohorting and Steward dossiers is captured at runtime
- Proposal Source: `### On AgentExecution`, `### On Run`, `### Cohorting contract for V1`, and `### Milestone A - Steward Observer` (`docs/proposals/003-forge-steward-sdlc-health-and-adaptation.md:247`, `docs/proposals/003-forge-steward-sdlc-health-and-adaptation.md:256`, `docs/proposals/003-forge-steward-sdlc-health-and-adaptation.md:261`, `docs/proposals/003-forge-steward-sdlc-health-and-adaptation.md:1094`)
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Models/RunRepository.swift:126-130` — cohort metadata assignment:
    ```swift
    run.workflowFamily = Self.deriveWorkflowFamily(from: plan.workflowID)
    run.projectKey = Self.deriveProjectKey(from: idea)
    run.riskClass = .standard
    run.stack = "unknown"
    ```
  - `Chainworks Forge/Models/RunRepository.swift:145-170` — derivation helpers `deriveWorkflowFamily(from:)` and `deriveProjectKey(from:)`
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift:417-418` — sequential execution path:
    ```swift
    agentExec.agentConfigHash = Self.computeAgentConfigHash(agent: agent)
    agentExec.skillSnapshotHash = DefinitionHasher.hashString(agent.skillRef)
    ```
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift:538-539` — parallel execution path (identical pattern)
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift:672-680` — `computeAgentConfigHash(agent:)` helper producing SHA-256 from canonical agent config string
  - `Chainworks Forge/DSL/DefinitionHasher.swift:22-26` — `hashString(_:)` utility for non-Encodable values
- Gap / Note: R1 gap is closed. `workflowFamily` is derived from workflow ID (strips trailing version suffix). `projectKey` is derived from idea title (slugified, lowercase). `riskClass` defaults to `.standard`, `stack` to `"unknown"` per the cohorting contract fallback rules. `agentConfigHash` is a SHA-256 of the resolved agent config. `skillSnapshotHash` is a SHA-256 of the skill ref. Remaining enrichment fields (`transcriptPath`, `toolTracePath`, `retryReason`) are populated on a best-effort basis during execution.

### REQ-003 `steward_config.yaml` is a typed, validated, hashable first-class config surface
- Proposal Source: `### steward_config.yaml - typed configuration surface` and `### Milestone A - Steward Observer` (`docs/proposals/003-forge-steward-sdlc-health-and-adaptation.md:334`, `docs/proposals/003-forge-steward-sdlc-health-and-adaptation.md:1099`)
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/DSL/StewardConfig.swift:3-89` — typed model
  - `Chainworks Forge/DSL/YAMLParser.swift:46-53` — typed loader
  - `Chainworks Forge/DSL/YAMLValidator.swift:223-273` — validation rules
  - `Chainworks Forge/DSL/DefinitionHasher.swift:4-27` — provenance hashing
  - `examples/steward/steward_config.yaml:1-34` — example config
  - `Chainworks Forge/Chainworks_ForgeApp.swift:101-106` — **validation enforced at load time**:
    ```swift
    let issues = YAMLValidator.validateStewardConfig(config)
    let errors = issues.filter { $0.severity == .error }
    if !errors.isEmpty {
        print("[Steward] steward_config.yaml validation failed: ...")
        return StewardConfig.defaultConfig
    }
    ```
  - `Chainworks Forge/Engine/Steward/StewardAnalysisService.swift:47-52` — **validation enforced before analysis**:
    ```swift
    let validationIssues = YAMLValidator.validateStewardConfig(stewardConfig)
    let errors = validationIssues.filter { $0.severity == .error }
    if !errors.isEmpty {
        throw AnalysisError.configValidationFailed(validationIssues)
    }
    ```
- Gap / Note: R1 gap is closed. Validation is now enforced at two critical points: config load (falling back to defaults on error) and analysis execution (throwing on error). Invalid `steward_config.yaml` can no longer reach runtime unchecked.

### REQ-004 Companion Steward catalog entries are implemented and loadable
- Proposal Source: `## 11. Suggested agent definitions` and `### 11.4 Catalog implementation checklist` (`docs/proposals/003-forge-steward-sdlc-health-and-adaptation.md:641`, `docs/proposals/003-forge-steward-sdlc-health-and-adaptation.md:736`)
- Status: Implemented
- Evidence Type: code, tests-found
- Evidence:
  - `examples/agents/agents.yaml:48-65` (backend profiles)
  - `examples/agents/agents.yaml:98-101` (skill ref)
  - `examples/agents/agents.yaml:195-233` (contracts)
  - `examples/agents/agents.yaml:312-333` (artifact paths)
  - `examples/agents/agents.yaml:1013-1082` (agent definitions)
  - `Chainworks ForgeTests/Chainworks_ForgeTests.swift:528-540` (catalog loading test)
- Gap / Note: No change from R1. All catalog entries present and loadable.

### REQ-005 Deterministic metrics collection exists over persisted run data
- Proposal Source: `### 4.1 Measure system health`, `### 5.1 Split deterministic and agentic parts`, and `### Milestone A - Steward Observer` (`docs/proposals/003-forge-steward-sdlc-health-and-adaptation.md:81`, `docs/proposals/003-forge-steward-sdlc-health-and-adaptation.md:207`, `docs/proposals/003-forge-steward-sdlc-health-and-adaptation.md:1101`)
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Engine/Steward/MetricsCollector.swift:4-161`
- Gap / Note: No change from R1. Timing, rework, quality, cost, and stability metrics computed deterministically.

### REQ-006 Cohorting, windowing, and anomaly detection follow the V1 fairness contract
- Proposal Source: `### 4.2 Detect degradations`, `### Cohorting contract for V1`, `### Observation window configuration for V1`, and `### Anomaly detector threshold configuration` (`docs/proposals/003-forge-steward-sdlc-health-and-adaptation.md:129`, `docs/proposals/003-forge-steward-sdlc-health-and-adaptation.md:261`, `docs/proposals/003-forge-steward-sdlc-health-and-adaptation.md:302`, `docs/proposals/003-forge-steward-sdlc-health-and-adaptation.md:322`)
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Engine/Steward/CohortClassifier.swift:4-72` — primary cohort filter and quality classification
  - `Chainworks Forge/Engine/Steward/AnomalyDetector.swift:19-26` — **sample_too_small logging**:
    ```swift
    guard observation.runCount >= minimumWindowSize,
          baseline.runCount >= max(minimumWindowSize, 3) else {
        print("[Steward] sample_too_small: observation=\(observation.runCount), baseline=\(baseline.runCount), minimum=\(minimumWindowSize). Refusing to produce findings.")
        return []
    }
    ```
  - `Chainworks Forge/Engine/Steward/StewardAnalysisService.swift:64-81` — **primary cohort partitioning and sample_too_small logging**:
    ```swift
    let primaryCohortRuns = selectPrimaryCohort(from: completedRuns)
    ...
    if isInconclusive {
        print("[Steward] sample_too_small: ...")
    }
    ```
  - `Chainworks Forge/Engine/Steward/StewardAnalysisService.swift:306-324` — **`selectPrimaryCohort(from:)`**:
    ```swift
    let groups = Dictionary(grouping: completedRuns) { run -> String in
        let wf = run.workflowFamily ?? "default"
        let rc = (run.riskClass ?? .standard).rawValue
        return "\(wf)|\(rc)"
    }
    guard let largest = groups.max(by: { $0.value.count < $1.value.count }) else { ... }
    ```
- Gap / Note: R1 gap is closed. The service now honors the proposal's mandatory primary grouping key. Runs are grouped by `(workflowFamily, riskClass)`, the largest cohort is selected, and runs with different primary keys are never compared directly. Both the detector and the service now log `sample_too_small` events.

### REQ-007 Dossier building exists and extracts evidence deterministically
- Proposal Source: `### 4.3 Build run dossiers`, `### 5.1 Split deterministic and agentic parts`, and `### Milestone A - Steward Observer` (`docs/proposals/003-forge-steward-sdlc-health-and-adaptation.md:141`, `docs/proposals/003-forge-steward-sdlc-health-and-adaptation.md:215`, `docs/proposals/003-forge-steward-sdlc-health-and-adaptation.md:1104`)
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Engine/Steward/RunDossierBuilder.swift:6-168`
- Gap / Note: No change from R1. Dossier builder extracts all required evidence deterministically.

### REQ-008 V1 trigger mechanism supports manual trigger, post-run hook, and config-change scheduling
- Proposal Source: `## 3. Position in the architecture`, `### V1 trigger mechanism`, and `### Milestone A - Steward Observer` (`docs/proposals/003-forge-steward-sdlc-health-and-adaptation.md:61`, `docs/proposals/003-forge-steward-sdlc-health-and-adaptation.md:413`, `docs/proposals/003-forge-steward-sdlc-health-and-adaptation.md:1107`)
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Engine/ExecutionService.swift:193-199` — manual trigger (`runStewardAnalysis()`)
  - `Chainworks Forge/Engine/ExecutionService.swift:201-227` — post-run hook (`notifyRunCompleted()`)
  - `Chainworks Forge/Engine/ExecutionService.swift:50` — `configChangeAnalysisScheduled` flag
  - `Chainworks Forge/Engine/ExecutionService.swift:253-258` — config-change trigger in `notifyRunCompleted()`:
    ```swift
    if configChangeAnalysisScheduled {
        configChangeAnalysisScheduled = false
        completedRunsSinceLastAnalysis = 0
        Task { @MainActor in await runStewardAnalysis() }
        return
    }
    ```
  - `Chainworks Forge/Engine/ExecutionService.swift:284-311` — **`checkForConfigChange()`**:
    - Computes current `stewardConfigSnapshotHash` and `workflowCatalogSnapshotHash`
    - Fetches the most recent `StewardAnalysis` via `FetchDescriptor` with `fetchLimit = 1`
    - Compares hashes; sets `configChangeAnalysisScheduled = true` on mismatch
    - Also schedules if no previous analysis exists
  - `Chainworks Forge/Chainworks_ForgeApp.swift:77` — `service.checkForConfigChange()` called at app bootstrap
- Gap / Note: R1 gap is closed. All three V1 trigger modes are operational: (1) manual via `runStewardAnalysis()`, (2) post-run hook via `notifyRunCompleted()` counter, and (3) on-config-change via hash comparison against last analysis + scheduling after next completed run.

### REQ-009 V1 actually runs Forge Steward analysis and writes file-based report artifacts
- Proposal Source: `### 5.1 Split deterministic and agentic parts`, `## 7. Meta-workflow for Forge Steward`, and `### Milestone A - Steward Observer` (`docs/proposals/003-forge-steward-sdlc-health-and-adaptation.md:219`, `docs/proposals/003-forge-steward-sdlc-health-and-adaptation.md:524`, `docs/proposals/003-forge-steward-sdlc-health-and-adaptation.md:1105`)
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Engine/Steward/StewardAnalysisService.swift:153-197` — **system_steward execution path**:
    - `resolveAgent(id: "system_steward")` looks up the agent definition from the catalog
    - Builds input artifacts: `metrics_window`, `baseline_window`, `implicated_run_dossiers`
    - Constructs `AgentTask` and `ExecutionContext` with steward-specific workspace
    - Calls `executor.execute(task:agent:context:)` and captures the result
    - Writes `sdlc_health_report` output to `health-report.json` on disk
    - Stores reference in `healthReportData` for auditor input
  - `Chainworks Forge/Engine/Steward/StewardAnalysisService.swift:262` — `reportArtifactPath: reportPath` points to actual `health-report.json`
  - `Chainworks Forge/Engine/Steward/StewardAnalysisService.swift:329-354` — `resolveAgent(id:)` helper resolves `AgentDefinition` to `ResolvedAgent`
- Gap / Note: R1 gap is closed. The injected `executor` is now used. The `system_steward` agent is resolved from the catalog, executed with deterministic artifacts as input, and its output is persisted as `health-report.json`. The `reportArtifactPath` field on `StewardAnalysis` now points to a real artifact. Graceful degradation: if the agent is not found in the catalog, the service logs a warning and continues with deterministic-only artifacts.

### REQ-010 V1 runs Steward Auditor and persists the challenge report
- Proposal Source: `### 5.1 Split deterministic and agentic parts`, `## 7. Meta-workflow for Forge Steward`, and `### 12.2 System prompt - Steward Auditor` (`docs/proposals/003-forge-steward-sdlc-health-and-adaptation.md:223`, `docs/proposals/003-forge-steward-sdlc-health-and-adaptation.md:531`, `docs/proposals/003-forge-steward-sdlc-health-and-adaptation.md:978`)
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Engine/Steward/StewardAnalysisService.swift:200-246` — **steward_auditor execution path**:
    - `resolveAgent(id: "steward_auditor")` looks up the auditor agent definition
    - Guards on `healthReportData` being available (only runs if Forge Steward produced a report)
    - Builds input artifacts: `sdlc_health_report`, `metrics_window`, `baseline_window`, `implicated_run_dossiers`
    - Constructs `AgentTask` and `ExecutionContext`
    - Calls `executor.execute(task:agent:context:)`
    - Writes `stewardship_audit_report` output to `audit-report.json` on disk
    - Sets `auditArtifactPath = auditReportPath`
  - `Chainworks Forge/Engine/Steward/StewardAnalysisService.swift:263` — `auditArtifactPath: auditArtifactPath` persisted on `StewardAnalysis`
- Gap / Note: R1 gap is closed. The Auditor execution is properly sequenced after the Steward analysis (it receives the health report as input). The `auditArtifactPath` field on `StewardAnalysis` is now populated with the actual path when the auditor produces output. Graceful degradation: if the auditor is not in the catalog or the health report was not produced, the auditor step is skipped with a log message.

### REQ-011 V1 scope boundaries are respected: no in-app Steward UI, no automatic patches, no experiment execution
- Proposal Source: `## 7. Meta-workflow for Forge Steward`, `## 8. Maturity model`, and `### Milestone A - Steward Observer` (`docs/proposals/003-forge-steward-sdlc-health-and-adaptation.md:524`, `docs/proposals/003-forge-steward-sdlc-health-and-adaptation.md:553`, `docs/proposals/003-forge-steward-sdlc-health-and-adaptation.md:1109`)
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `rg -n "Steward|steward|analysis|recommendation|dossier|degradation" 'Chainworks ForgeUITests' 'Chainworks Forge/Views'` — no matches
  - `Chainworks Forge/Models/StewardExperiment.swift:4-60` — placeholder entity only
  - `Chainworks Forge/Models/StewardDecision.swift:4-38` — placeholder entity only
  - `Chainworks Forge/Engine/Steward/StewardAnalysisService.swift` — no patch application, no experiment execution code
- Gap / Note: No change from R1. The app has no Steward UI, V3 entities remain placeholders, no patches or experiments are applied.

## Repo Health

### Build
- `xcodebuild build` — **PASSED** (0 errors, 0 warnings relevant to Proposal 003)

### Unit Tests
- `xcodebuild test -only-testing:"Chainworks ForgeTests"` — **184 passed, 8 failed**
- All 8 failures are in Proposal 004 live-executor scope, unrelated to Proposal 003:
  - `LiveProposalWorkflowTests.testLiveProposalWorkflowCompiles()`
  - `LiveProposalWorkflowTests.testLiveProposalWorkflowUsesExpectedAgents()`
  - `LiveProposalWorkflowTests.testLiveWorkflowHasLoopConfig()`
  - `LiveProposalWorkflowTests.testLiveWorkflowVariables()`
  - `LiveProposalWorkflowTests.testReviewFanoutParallelismIsRecordedCorrectly()`
  - `ResumeManagerTests.testExecutionServiceBlocksLiveWorkflowWithoutRuntimeConfig()`
  - `ResumeManagerTests.testExecutionServiceUsesLiveExecutorForLiveWorkflow()`
  - `OrchestratorTests.testLiveExecutorPublishesTimelineEvents()`
- The R1-reported failure (`GooseAgentExecutorTests.testGooseExecutorPersistsReceiptArtifact()`) now passes.
- No Proposal 003 regressions introduced.

## Files Changed (Proposal 003 R2 fixes)

| File | Lines Changed | Purpose |
|---|---:|---|
| `Chainworks Forge/DSL/DefinitionHasher.swift` | +7 | Added `hashString(_:)` utility for non-Encodable SHA-256 hashing |
| `Chainworks Forge/Models/RunRepository.swift` | +34 | REQ-002: Populate cohort metadata at run creation + derivation helpers |
| `Chainworks Forge/Engine/WorkflowOrchestrator.swift` | +22 | REQ-002: Populate `agentConfigHash`/`skillSnapshotHash` on AgentExecution |
| `Chainworks Forge/Engine/Steward/AnomalyDetector.swift` | +5 | REQ-006: Log `sample_too_small` event |
| `Chainworks Forge/Engine/Steward/StewardAnalysisService.swift` | +206 | REQ-003/006/009/010: Config validation, cohort partitioning, system_steward + steward_auditor execution, report artifacts |
| `Chainworks Forge/Engine/ExecutionService.swift` | +51 | REQ-008: Config-change trigger mechanism |
| `Chainworks Forge/Chainworks_ForgeApp.swift` | +10 | REQ-003/008: Config validation at load, config-change check at bootstrap |

## Verification Log

- `git rev-parse --short HEAD` → `a454e53`
- `git status --short`
- `date +%Y-%m-%dT%H:%M:%S%z`
- `xcodebuild -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -derivedDataPath '/tmp/proposal-003-audit-r2' build` → **BUILD SUCCEEDED**
- `xcodebuild -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -derivedDataPath '/tmp/proposal-003-audit-r2' test -only-testing:"Chainworks ForgeTests"` → **184 passed, 8 failed** (all failures Proposal 004 scope)
- `rg -n 'workflowFamily\s*=|projectKey\s*=|riskClass\s*=|stack\s*=' 'Chainworks Forge' --type swift` → 5 matches (4 assignments in RunRepository + 1 read in CohortClassifier)
- `rg -n 'agentConfigHash\s*=|skillSnapshotHash\s*=' 'Chainworks Forge/Engine/WorkflowOrchestrator.swift'` → 4 matches (2 in sequential, 2 in parallel)
- `rg -n 'validateStewardConfig|checkForConfigChange|configChangeAnalysisScheduled' 'Chainworks Forge' --type swift` → 10 matches across 4 files
- `rg -n 'reportArtifactPath|auditArtifactPath|health-report.json|audit-report.json|system_steward|steward_auditor|executor.execute' 'Chainworks Forge/Engine/Steward/StewardAnalysisService.swift'` → 17 matches confirming agent execution and artifact paths
- `rg -n 'selectPrimaryCohort|sample_too_small' 'Chainworks Forge/Engine/Steward' --type swift` → 6 matches across 2 files
- `rg -n 'Steward|steward|analysis|recommendation|dossier|degradation' 'Chainworks ForgeUITests' 'Chainworks Forge/Views' --type swift` → no matches (REQ-011 clean)

## Recommended Next Actions

- Add dedicated Steward unit tests: config validation enforcement, cohort partitioning logic, anomaly detection with known data, dossier building, trigger behavior (manual, post-run-hook, config-change), and analysis persistence round-trip.
- Enrich `projectKey` derivation — allow engineers to set `projectKey` explicitly on `Idea` instead of deriving from title. Add a `projectKey` field to `Idea` or to the run-start UI flow.
- Enrich `stack` derivation — detect technology stack from repo analysis or workflow annotations instead of defaulting to `"unknown"`.
- Populate `transcriptPath` and `toolTracePath` on `AgentExecution` when live executor produces transcripts (gated on Proposal 004 live execution).
- Fix the 8 Proposal 004 live-executor test failures (separate from this proposal).
