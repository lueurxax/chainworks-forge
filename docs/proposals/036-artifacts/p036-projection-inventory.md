# P036 Field-Level Projection Inventory

| Surface | Display Field | Source Query/Subscription | Authorized Detail Read | Owning Presenter | Payload Availability/Deferred State | Fixture/Smoke Coverage | Decision |
| --- | --- | --- | --- | --- | --- | --- | --- |
| Runs lanes | `id`, `title`, `count` | `runs(ideaId:)`, `runs` | N/A | `RunsWorkbenchPresentationModel` | N/A | `testApprovalInboxReachable` | carry_forward |
| Run summary | `title`, `status`, `workflow`, `progress` | `run(id:)` | N/A | `RunsWorkbenchPresentationModel` | N/A | `testRunProgressViewSurface` | carry_forward |
| Stage map | `stageId`, `status`, `attempt`, `duration` | `stages(runId:)` | N/A | `RunsWorkbenchPresentationModel` | `projectionLag` | `testWorkflowMapSurfaceShowsAfterRunStart` | replace_with_p036_design |
| Inline approvals | `approvalId`, `title`, `actionable` | `approvalInbox` | N/A | `RunsWorkbenchPresentationModel` | `disabledReasonCode` | `testApprovalGateViewSurface` | replace_with_p036_design |
| Artifacts | `artifactId`, `title`, `stageId` | `artifacts(runId:)` | authorized detail read | `RunsWorkbenchPresentationModel` | `payloadAvailabilityState` | `testCompletedRunExportHubSurface` | carry_forward |
| Reports | `reportId`, `kind`, `metadata` | `artifacts(runId:)` | future payload query | `RunsWorkbenchPresentationModel` | `payloadAvailabilityState` | `testCompletedRunExportHubSurface` | defer_until_projection_exists |
| Recovery/Evidence | `startupRecovery`, `diagnosticRows` | `startupRecoverySummary` | N/A | `RunsWorkbenchPresentationModel` | N/A | `testLiveRuntimeUnavailableShowsRecoveryGuidance` | carry_forward |
| Freshness | `freshnessState`, `lastCheckedAt` | run, stage, approval | N/A | `RunsWorkbenchPresentationModel` | `FreshnessState` | N/A | carry_forward |
| Daemon health | `daemonStatus`, `buildSHA`, `uptime` | `daemonStatus` | N/A | `RunsWorkbenchPresentationModel` | N/A | N/A | carry_forward |
| Timeline | `entries`, `kind`, `agentId`, `collapsed` | `agentExecutions` | N/A | `RunsWorkbenchPresentationModel` | N/A | `testRunTimelineInspectorViewTests` | replace_with_p036_design |
| Ideas strips | `counts`, `latestStatus` | `ideas` | N/A | `IdeasPresenter` | N/A | N/A | replace_with_p036_design |
| Definitions | `agents`, `groups`, `backends` | `agents` | N/A | `DefinitionsPresenter` | N/A | `testProposal036DefinitionsSegmentedWrapper` | replace_with_p036_design |
| Definitions | `workflows`, `order`, `initialState` | `workflows` | N/A | `DefinitionsPresenter` | N/A | `testProposal036DefinitionsSegmentedWrapper` | replace_with_p036_design |
| Settings | `readiness`, `mcpHub`, `capabilities` | `schedulerHealthSummary` | N/A | `SettingsPresenter` | N/A | `testPilotReadinessRefreshSurface` | replace_with_p036_design |
