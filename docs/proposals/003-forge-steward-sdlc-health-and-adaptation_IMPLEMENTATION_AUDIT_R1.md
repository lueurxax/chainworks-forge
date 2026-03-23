# Proposal 003: Forge Steward - SDLC Health, Degradation Detection, and Controlled Adaptation Implementation Audit R1

| Field | Value |
|---|---|
| Proposal | docs/proposals/003-forge-steward-sdlc-health-and-adaptation.md |
| Repository Root | . |
| Git SHA | a454e53 |
| Working Tree | dirty (`Chainworks Forge/Chainworks_ForgeApp.swift`, `Chainworks Forge/Views/IdeaListView.swift`, `Chainworks ForgeTests/ResumeManagerTests.swift`, `Chainworks ForgeUITests/Chainworks_ForgeUITests.swift`, `docs/proposals/002-workflow-execution-engine_IMPLEMENTATION_AUDIT_R2.md`) |
| Audited At | 2026-03-23T07:47:56+02:00 |
| Proposal State | Active (Draft, not superseded) |
| Overall Status | Not Implemented |

## Verdict

Proposal 003 is only partially implemented. The repository now contains a real Steward slice: SwiftData entities, typed `steward_config`, catalog additions, deterministic metrics/anomaly/dossier components, and an `ExecutionService` trigger path. The proposal is still not implemented overall because several mandatory V1 contracts are missing: run-level cohort metadata is never populated, `steward_config` validation is defined but not enforced, config-change triggering is absent, and the core agentic half of V1 does not run. `StewardAnalysisService` never executes Forge Steward or Steward Auditor, never writes the promised health/audit report artifacts, and leaves `auditArtifactPath` unused.

Fresh repo-health evidence is also not clean in this audit: `xcodebuild build` passed, but `xcodebuild test` failed due a UI-test runner automation timeout and a failing `GooseAgentExecutorTests.testGooseExecutorPersistsReceiptArtifact()`. Those failures are secondary to the proposal-contract gaps, but they reduce confidence further.

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
| Implemented | 5 |
| Partially Implemented | 3 |
| Missing | 3 |
| Not Verifiable | 0 |

## Requirement Audit

### REQ-001 Steward persistence entities and schema registration exist
- Proposal Source: `## 6. Steward-domain persistence model`, `#### SwiftData migration strategy`, and `## 13. Suggested first milestone` (`docs/proposals/003-forge-steward-sdlc-health-and-adaptation.md:430`, `docs/proposals/003-forge-steward-sdlc-health-and-adaptation.md:509`, `docs/proposals/003-forge-steward-sdlc-health-and-adaptation.md:1094`)
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Models/StewardAnalysis.swift:4-69`
  - `Chainworks Forge/Models/StewardAnalysisRunLink.swift:4-30`
  - `Chainworks Forge/Models/StewardRecommendation.swift:4-68`
  - `Chainworks Forge/Models/StewardExperiment.swift:4-60`
  - `Chainworks Forge/Models/StewardDecision.swift:4-38`
  - `Chainworks Forge/Chainworks_ForgeApp.swift:7-19`
- Gap / Note: The entity layer itself is present and registered in the app schema.

### REQ-002 Mandatory run and agent metadata for cohorting and Steward dossiers is captured at runtime
- Proposal Source: `### On AgentExecution`, `### On Run`, `### Cohorting contract for V1`, and `### Milestone A - Steward Observer` (`docs/proposals/003-forge-steward-sdlc-health-and-adaptation.md:247`, `docs/proposals/003-forge-steward-sdlc-health-and-adaptation.md:256`, `docs/proposals/003-forge-steward-sdlc-health-and-adaptation.md:261`, `docs/proposals/003-forge-steward-sdlc-health-and-adaptation.md:1094`)
- Status: Missing
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Models/Run.swift:32-37`
  - `Chainworks Forge/Models/AgentExecution.swift:25-30`
  - `Chainworks Forge/Models/Idea.swift:4-22`
  - `rg -n "workflowFamily\\s*=|projectKey\\s*=|riskClass\\s*=|stack\\s*=|experimentCohortID\\s*=|agentConfigHash\\s*=|skillSnapshotHash\\s*=|transcriptPath\\s*=|toolTracePath\\s*=|retryReason\\s*=" 'Chainworks Forge' 'Chainworks ForgeTests'` (no assignments beyond declarations)
  - `rg -n "projectKey|riskClass|stack|workflowFamily|experimentCohortID" 'Chainworks Forge/Models/Idea.swift' 'Chainworks Forge/Views' 'Chainworks Forge/Engine/RunPlanCompiler.swift' 'Chainworks Forge/Models/RunRepository.swift'` (no capture path)
- Gap / Note: The fields exist, but the app never populates them. `Idea` has no source fields for `projectKey` / `riskClass` / `stack`, and there is no assignment path for the new `Run` or `AgentExecution` metadata. That breaks the proposal's mandatory cohorting contract.

### REQ-003 `steward_config.yaml` is a typed, validated, hashable first-class config surface
- Proposal Source: `### steward_config.yaml - typed configuration surface` and `### Milestone A - Steward Observer` (`docs/proposals/003-forge-steward-sdlc-health-and-adaptation.md:334`, `docs/proposals/003-forge-steward-sdlc-health-and-adaptation.md:1099`)
- Status: Partially Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/DSL/StewardConfig.swift:3-88`
  - `Chainworks Forge/DSL/YAMLParser.swift:46-53`
  - `Chainworks Forge/DSL/YAMLValidator.swift:221-273`
  - `Chainworks Forge/DSL/DefinitionHasher.swift:4-19`
  - `examples/steward/steward_config.yaml:1-34`
  - `Chainworks Forge/Chainworks_ForgeApp.swift:61-67`
- Gap / Note: Loader, model, validator, hasher, and example config all exist. The validator is not wired into load or execution paths, so invalid `steward_config.yaml` can still reach runtime unchecked.

### REQ-004 Companion Steward catalog entries are implemented and loadable
- Proposal Source: `## 11. Suggested agent definitions` and `### 11.4 Catalog implementation checklist` (`docs/proposals/003-forge-steward-sdlc-health-and-adaptation.md:641`, `docs/proposals/003-forge-steward-sdlc-health-and-adaptation.md:736`)
- Status: Implemented
- Evidence Type: code, tests-found
- Evidence:
  - `examples/agents/agents.yaml:48-65`
  - `examples/agents/agents.yaml:98-101`
  - `examples/agents/agents.yaml:195-233`
  - `examples/agents/agents.yaml:312-333`
  - `examples/agents/agents.yaml:1013-1082`
  - `Chainworks ForgeTests/Chainworks_ForgeTests.swift:528-540`
  - `Chainworks ForgeTests/Chainworks_ForgeTests.swift:621-624`
- Gap / Note: The repo fixture now includes the Steward artifact keys, skill ref, contracts, backend profiles, and agents described in section 11.4.

### REQ-005 Deterministic metrics collection exists over persisted run data
- Proposal Source: `### 4.1 Measure system health`, `### 5.1 Split deterministic and agentic parts`, and `### Milestone A - Steward Observer` (`docs/proposals/003-forge-steward-sdlc-health-and-adaptation.md:81`, `docs/proposals/003-forge-steward-sdlc-health-and-adaptation.md:207`, `docs/proposals/003-forge-steward-sdlc-health-and-adaptation.md:1101`)
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Engine/Steward/MetricsCollector.swift:4-160`
- Gap / Note: The collector computes timing, rework, quality, cost, and stability metrics directly from persisted run data without any LLM dependency.

### REQ-006 Cohorting, windowing, and anomaly detection follow the V1 fairness contract
- Proposal Source: `### 4.2 Detect degradations`, `### Cohorting contract for V1`, `### Observation window configuration for V1`, and `### Anomaly detector threshold configuration` (`docs/proposals/003-forge-steward-sdlc-health-and-adaptation.md:129`, `docs/proposals/003-forge-steward-sdlc-health-and-adaptation.md:261`, `docs/proposals/003-forge-steward-sdlc-health-and-adaptation.md:302`, `docs/proposals/003-forge-steward-sdlc-health-and-adaptation.md:322`)
- Status: Partially Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Engine/Steward/CohortClassifier.swift:4-72`
  - `Chainworks Forge/Engine/Steward/AnomalyDetector.swift:3-155`
  - `Chainworks Forge/Engine/Steward/StewardAnalysisService.swift:30-64`
- Gap / Note: The classifier and detector exist, but the live service does not honor the proposal's mandatory primary grouping key. `StewardAnalysisService` splits windows across all completed runs without first partitioning by `(workflowFamily, riskClass)`, and the sample-too-small path returns `[]` without recording the proposal's `sample_too_small` event.

### REQ-007 Dossier building exists and extracts evidence deterministically
- Proposal Source: `### 4.3 Build run dossiers`, `### 5.1 Split deterministic and agentic parts`, and `### Milestone A - Steward Observer` (`docs/proposals/003-forge-steward-sdlc-health-and-adaptation.md:141`, `docs/proposals/003-forge-steward-sdlc-health-and-adaptation.md:215`, `docs/proposals/003-forge-steward-sdlc-health-and-adaptation.md:1104`)
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Engine/Steward/RunDossierBuilder.swift:6-167`
- Gap / Note: The dossier builder extracts run metadata, stage summaries, approvals, costs, failures, artifacts, and drift fields directly from persisted relationships.

### REQ-008 V1 trigger mechanism supports manual trigger, post-run hook, and config-change scheduling
- Proposal Source: `## 3. Position in the architecture`, `### V1 trigger mechanism`, and `### Milestone A - Steward Observer` (`docs/proposals/003-forge-steward-sdlc-health-and-adaptation.md:61`, `docs/proposals/003-forge-steward-sdlc-health-and-adaptation.md:413`, `docs/proposals/003-forge-steward-sdlc-health-and-adaptation.md:1107`)
- Status: Partially Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Engine/ExecutionService.swift:27-37`
  - `Chainworks Forge/Engine/ExecutionService.swift:69-72`
  - `Chainworks Forge/Engine/ExecutionService.swift:182-209`
  - `Chainworks Forge/DSL/StewardConfig.swift:34-63`
- Gap / Note: Manual trigger and post-run hook are implemented. The on-config-change mode is not: there is no code comparing current workflow/catalog/steward-config hashes against the last `StewardAnalysis` and scheduling a follow-up analysis after the next completed run.

### REQ-009 V1 actually runs Forge Steward analysis and writes file-based report artifacts
- Proposal Source: `### 5.1 Split deterministic and agentic parts`, `## 7. Meta-workflow for Forge Steward`, and `### Milestone A - Steward Observer` (`docs/proposals/003-forge-steward-sdlc-health-and-adaptation.md:219`, `docs/proposals/003-forge-steward-sdlc-health-and-adaptation.md:524`, `docs/proposals/003-forge-steward-sdlc-health-and-adaptation.md:1105`)
- Status: Missing
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Engine/Steward/StewardAnalysisService.swift:25-161`
  - `Chainworks Forge/Engine/Steward/StewardAnalysisService.swift:75-109`
  - `rg -n "reportArtifactPath|auditArtifactPath|health-report.json|degradation-alerts.json" 'Chainworks Forge/Engine/Steward/StewardAnalysisService.swift' 'Chainworks ForgeTests'`
- Gap / Note: The service writes metrics, baseline, dossier, and degradation-alert JSON, but it never executes a Forge Steward agent, never uses the injected `executor`, and never writes the promised `health-report.json` artifact even though it persists a `reportArtifactPath`.

### REQ-010 V1 runs Steward Auditor and persists the challenge report
- Proposal Source: `### 5.1 Split deterministic and agentic parts`, `## 7. Meta-workflow for Forge Steward`, and `### 12.2 System prompt - Steward Auditor` (`docs/proposals/003-forge-steward-sdlc-health-and-adaptation.md:223`, `docs/proposals/003-forge-steward-sdlc-health-and-adaptation.md:531`, `docs/proposals/003-forge-steward-sdlc-health-and-adaptation.md:978`)
- Status: Missing
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Engine/Steward/StewardAnalysisService.swift:109-161`
  - `Chainworks Forge/Models/StewardAnalysis.swift:15-19`
  - `rg -n "auditArtifactPath|stewardship_audit_report|executor" 'Chainworks Forge/Engine/Steward/StewardAnalysisService.swift' 'Chainworks Forge/Engine'`
- Gap / Note: `auditArtifactPath` exists only as stored metadata. There is no Auditor execution path, no audit artifact creation, and no persistence of an actual challenge report.

### REQ-011 V1 scope boundaries are respected: no in-app Steward UI, no automatic patches, no experiment execution
- Proposal Source: `## 7. Meta-workflow for Forge Steward`, `## 8. Maturity model`, and `### Milestone A - Steward Observer` (`docs/proposals/003-forge-steward-sdlc-health-and-adaptation.md:524`, `docs/proposals/003-forge-steward-sdlc-health-and-adaptation.md:553`, `docs/proposals/003-forge-steward-sdlc-health-and-adaptation.md:1109`)
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `rg -n "Steward|steward|analysis|recommendation|dossier|degradation" 'Chainworks ForgeUITests' 'Chainworks Forge/Views'` (no matches)
  - `Chainworks Forge/Models/StewardExperiment.swift:4-60`
  - `Chainworks Forge/Models/StewardDecision.swift:4-38`
  - `Chainworks Forge/Engine/Steward/StewardAnalysisService.swift:146-158`
- Gap / Note: The app still has no Steward UI, and the V3 entities are present only as placeholders. The code does not apply patches or run experiments automatically.

## Verification Log

- `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/report_path.py /Users/user/Documents/Chainworks Forge/docs/proposals/003-forge-steward-sdlc-health-and-adaptation.md`
- `git rev-parse --short HEAD`
- `git status --short`
- `date +%Y-%m-%dT%H:%M:%S%z`
- `rg -n "superseded|deprecated|replaced by|obsolete" 'docs/proposals/003-forge-steward-sdlc-health-and-adaptation.md' docs docs/reviews docs/proposals`
- `nl -ba 'docs/proposals/003-forge-steward-sdlc-health-and-adaptation.md' | sed -n '1,260p'`
- `nl -ba 'docs/proposals/003-forge-steward-sdlc-health-and-adaptation.md' | sed -n '260,620p'`
- `nl -ba 'docs/proposals/003-forge-steward-sdlc-health-and-adaptation.md' | sed -n '620,980p'`
- `nl -ba 'docs/proposals/003-forge-steward-sdlc-health-and-adaptation.md' | sed -n '980,1320p'`
- `rg -n '^## ' 'docs/proposals/003-forge-steward-sdlc-health-and-adaptation.md'`
- `find 'Chainworks Forge' -maxdepth 3 -type f | rg 'Steward|steward|Metrics|Anomaly|Dossier|Analysis|Recommendation|Experiment|Decision'`
- `rg -n "StewardAnalysis|StewardRecommendation|StewardAnalysisRunLink|StewardExperiment|StewardDecision|StewardConfig|runStewardAnalysis|MetricsCollector|AnomalyDetector|DossierBuilder|Steward|DegradationSignal|steward_config|workflowFamily|projectKey|riskClass|stack|agentConfigHash|retryReason|transcriptPath|toolTracePath|skillSnapshotHash|metrics_window|baseline_window|sdlc_health_report" 'Chainworks Forge' 'Chainworks ForgeTests' 'examples'`
- `nl -ba 'Chainworks Forge/DSL/StewardConfig.swift' | sed -n '1,220p'`
- `nl -ba 'Chainworks Forge/DSL/YAMLParser.swift' | sed -n '1,120p'`
- `nl -ba 'Chainworks Forge/DSL/YAMLValidator.swift' | sed -n '210,300p'`
- `nl -ba 'Chainworks Forge/DSL/DefinitionHasher.swift' | sed -n '1,260p'`
- `nl -ba 'Chainworks Forge/Models/Run.swift' | sed -n '1,120p'`
- `nl -ba 'Chainworks Forge/Models/AgentExecution.swift' | sed -n '1,120p'`
- `nl -ba 'Chainworks Forge/Models/StewardAnalysis.swift' | sed -n '1,220p'`
- `nl -ba 'Chainworks Forge/Models/StewardAnalysisRunLink.swift' | sed -n '1,220p'`
- `nl -ba 'Chainworks Forge/Models/StewardRecommendation.swift' | sed -n '1,220p'`
- `nl -ba 'Chainworks Forge/Models/StewardExperiment.swift' | sed -n '1,220p'`
- `nl -ba 'Chainworks Forge/Models/StewardDecision.swift' | sed -n '1,220p'`
- `nl -ba 'Chainworks Forge/Engine/ExecutionService.swift' | sed -n '1,260p'`
- `nl -ba 'Chainworks Forge/Engine/Steward/MetricsCollector.swift' | sed -n '1,260p'`
- `nl -ba 'Chainworks Forge/Engine/Steward/AnomalyDetector.swift' | sed -n '1,260p'`
- `nl -ba 'Chainworks Forge/Engine/Steward/CohortClassifier.swift' | sed -n '1,220p'`
- `nl -ba 'Chainworks Forge/Engine/Steward/RunDossierBuilder.swift' | sed -n '1,260p'`
- `nl -ba 'Chainworks Forge/Engine/Steward/StewardAnalysisService.swift' | sed -n '1,320p'`
- `nl -ba 'examples/steward/steward_config.yaml' | sed -n '1,200p'`
- `nl -ba 'examples/agents/agents.yaml' | sed -n '1,380p'`
- `nl -ba 'examples/agents/agents.yaml' | sed -n '1000,1085p'`
- `rg -n "workflowFamily\\s*=|projectKey\\s*=|riskClass\\s*=|stack\\s*=|experimentCohortID\\s*=|agentConfigHash\\s*=|skillSnapshotHash\\s*=|transcriptPath\\s*=|toolTracePath\\s*=|retryReason\\s*=" 'Chainworks Forge' 'Chainworks ForgeTests'`
- `rg -n "projectKey|riskClass|stack|workflowFamily|experimentCohortID" 'Chainworks Forge/Models/Idea.swift' 'Chainworks Forge/Views' 'Chainworks Forge/Engine/RunPlanCompiler.swift' 'Chainworks Forge/Models/RunRepository.swift'`
- `rg -n "reportArtifactPath|auditArtifactPath|health-report.json|degradation-alerts.json|stewardship_audit_report|sdlc_health_report|executor" 'Chainworks Forge/Engine/Steward/StewardAnalysisService.swift' 'Chainworks Forge/Engine'`
- `rg -n "Steward|steward|analysis|recommendation|dossier|degradation" 'Chainworks ForgeUITests' 'Chainworks Forge/Views'`
- `xcodebuild -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -derivedDataPath '/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T//proposal-003-audit.5hFI4c' build`
- `xcodebuild -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -derivedDataPath '/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T//proposal-003-audit.5hFI4c' test`

## Recommended Next Actions

- Populate the mandatory Proposal 003 cohort fields on `Run` at run creation and record the Steward metadata fields on `AgentExecution` during execution.
- Enforce `YAMLValidator.validateStewardConfig()` before Steward runs and wire the proposal's on-config-change scheduling path.
- Make `StewardAnalysisService` actually execute the `system_steward` and `steward_auditor` agent paths, write `health-report.json` and audit-report artifacts, and persist the resulting artifact paths.
- Add dedicated Steward tests for config validation, cohort partitioning, anomaly detection, dossier building, trigger behavior, and analysis persistence.
