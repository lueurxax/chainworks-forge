# P036 Field-Level Projection Inventory

| Surface | Display Field | Source Query/Subscription | Authorized Detail Read | Owning Presenter | Payload Availability/Deferred State | Fixture/Smoke Coverage | Decision |
| --- | --- | --- | --- | --- | --- | --- | --- |
| Runs lanes | `runs` | `runs(ideaId:)`, `runs` | N/A | `RunsWorkbenchPresentationModel` | N/A | `testApprovalInboxReachable` | carry_forward |
| Run summary | `status`, `attention` | `run(id:)` | N/A | `RunsWorkbenchPresentationModel` | N/A | `testRunProgressViewSurface` | carry_forward |
| Stage map/cards | `stages` | `stages(runId:)` | N/A | `RunsWorkbenchPresentationModel` | `projectionLag` | `testWorkflowMapSurfaceShowsAfterRunStart` | replace_with_p036_design |
| Inline approvals | `approveApproval` | `approvalInbox` | N/A | `RunsWorkbenchPresentationModel` | `disabledReasonCode` | `testApprovalGateViewSurface` | replace_with_p036_design |
| Artifacts | `artifacts` | `artifacts(runId:)` | authorized detail read | `RunsWorkbenchPresentationModel` | `payloadAvailabilityState` | `testCompletedRunExportHubSurface` | carry_forward |
| Reports | `reports` | `artifacts(runId:)` | future payload query | `RunsWorkbenchPresentationModel` | `payloadAvailabilityState` | `testCompletedRunExportHubSurface` | defer_until_projection_exists |
| Recovery/Evidence | `startupRecovery` | `startupRecoverySummary` | N/A | `RunsWorkbenchPresentationModel` | N/A | `testLiveRuntimeUnavailableShowsRecoveryGuidance` | carry_forward |
| Freshness | `freshnessState` | run, stage, approval | N/A | `RunsWorkbenchPresentationModel` | `FreshnessState` | N/A | carry_forward |
| Daemon health | `daemonStatus` | `daemonStatus` | N/A | `RunsWorkbenchPresentationModel` | N/A | N/A | carry_forward |
| Timeline | `timeline` | `agentExecutions` | N/A | `TimelinePresentationModel` | N/A | `testRunTimelineInspectorViewTests` | replace_with_p036_design |
| Ideas strips | `counts` | `ideas` | N/A | `IdeasPresenter` | N/A | N/A | replace_with_p036_design |
| Definitions | `agents` | `agents` | N/A | `DefinitionsPresenter` | N/A | `testProposal036DefinitionsSegmentedWrapper` | replace_with_p036_design |
| Definitions | `workflows` | `workflows` | N/A | `DefinitionsPresenter` | N/A | `testProposal036DefinitionsSegmentedWrapper` | replace_with_p036_design |
| Settings | `readiness` | `schedulerHealthSummary` | N/A | `SettingsPresenter` | N/A | `testPilotReadinessRefreshSurface` | replace_with_p036_design |
