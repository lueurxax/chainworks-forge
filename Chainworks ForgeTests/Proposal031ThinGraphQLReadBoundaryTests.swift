import Foundation
import Testing

@testable import Chainworks_Forge

@Suite("P031 thin GraphQL read boundary", .tags(.fast))
struct Proposal031ThinGraphQLReadBoundaryTests {
  @Test("GraphQL read request accepts queries and rejects mutations before transport")
  func readRequestRejectsMutationDocuments() async throws {
    let transport = CapturingP031ReadTransport()
    let client = P031GraphQLReadClient(transport: transport)

    await #expect(throws: P031GraphQLReadBoundaryError.mutationOperationForbidden("P031ReadProbe"))
    {
      _ = try await client.execute(
        operationName: "P031ReadProbe",
        document: "mutation StartRun { runsStart { id } }"
      )
    }
    #expect(transport.requests.isEmpty)

    _ = try await client.execute(
      operationName: "P031RunList",
      document: "# allowed read\nquery P031RunList { runs { id freshnessState } }",
      variables: ["limit": "20"]
    )
    #expect(transport.requests.map(\.operationName) == ["P031RunList"])
    #expect(transport.requests.first?.operationKind == .query)
  }

  @Test("GraphQL read request accepts subscription documents")
  func readRequestAcceptsSubscriptions() throws {
    let request = try P031GraphQLReadRequest(
      operationName: "P031RunStatusChanged",
      document:
        "subscription P031RunStatusChanged { runStatusChanged(runId: \"run-1\") { id freshnessState } }"
    )

    #expect(request.operationKind == .subscription)
  }

  @Test("GraphQL query and subscription clients reject the opposite operation kind")
  func graphQLClientsEnforceDedicatedOperationKinds() async throws {
    let readTransport = CapturingP031ReadTransport()
    let readClient = P031GraphQLReadClient(transport: readTransport)
    await #expect(
      throws: P031GraphQLReadBoundaryError.queryOperationRequired("P031RunStatusChanged")
    ) {
      _ = try await readClient.execute(
        operationName: "P031RunStatusChanged",
        document: P031GraphQLDocuments.runStatusChanged,
        variables: ["runId": "run-1"]
      )
    }
    #expect(readTransport.requests.isEmpty)

    let subscriptionTransport = CapturingP031SubscriptionTransport()
    let subscriptionClient = P031GraphQLSubscriptionClient(transport: subscriptionTransport)
    #expect(throws: P031GraphQLReadBoundaryError.subscriptionOperationRequired("P031RunsHome")) {
      _ = try subscriptionClient.subscribe(
        operationName: "P031RunsHome",
        document: P031GraphQLDocuments.runsHome
      )
    }
    #expect(subscriptionTransport.requests.isEmpty)
  }

  @Test("GraphQL read request rejects mixed documents that include mutations")
  func readRequestRejectsMixedMutationDocuments() async throws {
    let transport = CapturingP031ReadTransport()
    let client = P031GraphQLReadClient(transport: transport)

    await #expect(throws: P031GraphQLReadBoundaryError.mutationOperationForbidden("P031RunList")) {
      _ = try await client.execute(
        operationName: "P031RunList",
        document:
          """
          query P031RunList { runs { id freshnessState } }
          mutation P031StartRun { runsStart { id } }
          """
      )
    }
    #expect(transport.requests.isEmpty)

    await #expect(throws: P031GraphQLReadBoundaryError.forbiddenOperationName("AgentResetReadback"))
    {
      _ = try await client.execute(
        operationName: "AgentResetReadback",
        document: "query AgentResetReadback { run(id: \"run-1\") { id } }"
      )
    }
    #expect(transport.requests.isEmpty)
  }

  @Test("GraphQL read request ignores operation-like tokens in comments and strings")
  func readRequestIgnoresOperationTokensInIgnoredGraphQLText() throws {
    let request = try P031GraphQLReadRequest(
      operationName: "P031ReadWithIgnoredText",
      document:
        #"""
        # mutation StartRun { ignored }
        query P031ReadWithIgnoredText {
          node(
            text: "mutation CancelRun { ignored } # not a GraphQL comment"
            block: """subscription P031Ignored { runStatusChanged { id } }"""
          ) {
            id
          }
        }
        """#
    )

    #expect(request.operationKind == .query)
  }

  @Test("GraphQL read request ignores operation-like field and fragment names")
  func readRequestIgnoresOperationTokensOutsideOperationDefinitions() throws {
    let request = try P031GraphQLReadRequest(
      operationName: "P031ReadWithFragments",
      document:
        """
        fragment mutation on Run {
          id
          subscription
          nested {
            query
            mutation {
              id
            }
          }
        }

        query P031ReadWithFragments {
          runs {
            ...mutation
          }
        }
        """
    )

    #expect(request.operationKind == .query)
  }

  @Test("URLSession subscription transport builds GraphQL WS frames without write plumbing")
  func urlSessionSubscriptionTransportBuildsGraphQLWSFrames() throws {
    let endpoint = DaemonClientEndpoint(
      baseURL: URL(string: "http://127.0.0.1:4000")!,
      bearerToken: "token-1"
    )
    let request = try P031GraphQLReadRequest(
      operationName: "P031RunStatusChanged",
      document: P031GraphQLDocuments.runStatusChanged,
      variables: ["runId": "run-1"]
    )

    let urlRequest = P031URLSessionGraphQLSubscriptionTransport.subscribeRequest(for: endpoint)
    #expect(urlRequest.url?.absoluteString == "ws://127.0.0.1:4000/graphql/ws")
    #expect(
      urlRequest.value(forHTTPHeaderField: "Sec-WebSocket-Protocol")
        == "graphql-transport-ws")

    let initFrame = try jsonObject(
      from: P031URLSessionGraphQLSubscriptionTransport.connectionInitFrame(
        bearerToken: endpoint.bearerToken
      ))
    #expect(initFrame["type"] as? String == "connection_init")
    #expect(
      (initFrame["payload"] as? [String: Any])?["Authorization"] as? String
        == "Bearer token-1")

    let subscribeFrame = try jsonObject(
      from: P031URLSessionGraphQLSubscriptionTransport.subscribeFrame(for: request))
    let payload = subscribeFrame["payload"] as? [String: Any]
    #expect(subscribeFrame["id"] as? String == "P031RunStatusChanged")
    #expect(subscribeFrame["type"] as? String == "subscribe")
    #expect(payload?["operationName"] as? String == "P031RunStatusChanged")
    #expect(payload?["query"] as? String == P031GraphQLDocuments.runStatusChanged)
    #expect((payload?["variables"] as? [String: String]) == ["runId": "run-1"])
  }

  @Test("GraphQL read variables support JSON typed values")
  func readRequestEncodesTypedJSONVariables() throws {
    let request = try P031GraphQLReadRequest(
      operationName: "P031FilteredRuns",
      document:
        "query P031FilteredRuns($limit: Int!, $includeStale: Boolean!, $filters: RunFilter) { runs { id } }",
      variables: [
        "limit": 25,
        "includeStale": true,
        "threshold": 0.75,
        "ids": ["run-1", "run-2"],
        "filters": [
          "owner": "operator",
          "includeReports": true,
          "cursor": nil,
        ],
      ]
    )

    let subscribeFrame = try jsonObject(
      from: P031URLSessionGraphQLSubscriptionTransport.subscribeFrame(for: request))
    let payload = try #require(subscribeFrame["payload"] as? [String: Any])
    let variables = try #require(payload["variables"] as? [String: Any])
    let filters = try #require(variables["filters"] as? [String: Any])

    #expect(variables["limit"] as? Int == 25)
    #expect(variables["includeStale"] as? Bool == true)
    #expect(variables["threshold"] as? Double == 0.75)
    #expect((variables["ids"] as? [String]) == ["run-1", "run-2"])
    #expect(filters["owner"] as? String == "operator")
    #expect(filters["includeReports"] as? Bool == true)
    #expect(filters["cursor"] is NSNull)
  }

  @Test("URLSession subscription transport unwraps GraphQL WS next and error frames")
  func urlSessionSubscriptionTransportDecodesGraphQLWSFrames() throws {
    let frame = try P031URLSessionGraphQLSubscriptionTransport.decodeFrame(
      """
      {"type":"next","payload":{"data":{"runStatusChanged":{"id":"run-1","status":"running","freshnessState":"live"}}}}
      """)

    guard case .next(let payloadData) = frame else {
      Issue.record("expected next frame")
      return
    }
    let payload = try JSONSerialization.jsonObject(with: payloadData) as? [String: Any]
    let data = payload?["data"] as? [String: Any]
    let status = data?["runStatusChanged"] as? [String: Any]
    #expect(status?["id"] as? String == "run-1")

    #expect(
      try P031URLSessionGraphQLSubscriptionTransport.decodeFrame(#"{"type":"complete"}"#)
        == .complete)
    #expect(
      throws: P031GraphQLReadBoundaryError.graphqlErrors(["forbidden"])
    ) {
      _ = try P031URLSessionGraphQLSubscriptionTransport.decodeFrame(
        #"{"type":"error","payload":[{"message":"forbidden"}]}"#)
    }
  }

  @Test("GraphQL read request rejects operation names that look like removed writes")
  func readRequestRejectsRemovedWriteOperationNames() async throws {
    let transport = CapturingP031ReadTransport()
    let client = P031GraphQLReadClient(transport: transport)

    await #expect(throws: P031GraphQLReadBoundaryError.forbiddenOperationName("CancelRunReadback"))
    {
      _ = try await client.execute(
        operationName: "CancelRunReadback",
        document: "query CancelRunReadback { run(id: \"run-1\") { id } }"
      )
    }
    #expect(transport.requests.isEmpty)

    await #expect(throws: P031GraphQLReadBoundaryError.forbiddenOperationName("ResetAgentReadback"))
    {
      _ = try await client.execute(
        operationName: "P031ReadProbe",
        document: "query ResetAgentReadback { runs { id } }"
      )
    }
    #expect(transport.requests.isEmpty)
  }

  @Test("GraphQL read request rejects document operation names that look like removed writes")
  func readRequestRejectsRemovedWriteDocumentOperationNames() async throws {
    let transport = CapturingP031ReadTransport()
    let client = P031GraphQLReadClient(transport: transport)

    await #expect(throws: P031GraphQLReadBoundaryError.forbiddenOperationName("StartRunReadback")) {
      _ = try await client.execute(
        operationName: "P031ReadProbe",
        document: "query StartRunReadback { runs { id } }"
      )
    }
    #expect(transport.requests.isEmpty)
  }

  @Test("GraphQL read request must target a named operation in the document")
  func readRequestRequiresMatchingDocumentOperationName() async throws {
    let transport = CapturingP031ReadTransport()
    let client = P031GraphQLReadClient(transport: transport)

    await #expect(throws: P031GraphQLReadBoundaryError.operationNameNotFound("P031RunsHome")) {
      _ = try await client.execute(
        operationName: "P031RunsHome",
        document: "query P031OtherRead { runs { id } }"
      )
    }
    #expect(transport.requests.isEmpty)

    let request = try P031GraphQLReadRequest(
      operationName: "P031RunStatusChanged",
      document:
        """
        query P031RunsHome { runs { id } }
        subscription P031RunStatusChanged { runStatusChanged(runId: "run-1") { id } }
        """
    )
    #expect(request.operationKind == .subscription)
  }

  @Test("GraphQL read client decodes data envelopes and surfaces GraphQL errors")
  func readClientDecodesTypedGraphQLEnvelopes() async throws {
    let transport = CapturingP031ReadTransport(
      responseData: Data(
        """
        {
          "data": {
            "runs": [{
              "id": "run-1",
              "status": "running",
              "workflowTitle": "Full MVP",
              "freshnessState": "projection_lag",
              "totalStages": 4,
              "completedStages": 1,
              "failedStages": 0,
              "pendingApprovals": 2
            }]
          }
        }
        """.utf8))
    let client = P031GraphQLReadClient(transport: transport)

    let payload = try await client.execute(
      RunsPayload.self,
      operationName: "P031RunsHome",
      document: P031GraphQLDocuments.runsHome
    )

    #expect(payload.runs.first?.id == "run-1")
    #expect(payload.runs.first?.freshnessState == .projectionLag)

    let errorTransport = CapturingP031ReadTransport(
      responseData: Data(#"{"errors":[{"message":"forbidden"}]}"#.utf8))
    let errorClient = P031GraphQLReadClient(transport: errorTransport)
    await #expect(throws: P031GraphQLReadBoundaryError.graphqlErrors(["forbidden"])) {
      _ = try await errorClient.execute(
        RunsPayload.self,
        operationName: "P031RunsHome",
        document: P031GraphQLDocuments.runsHome
      )
    }
  }

  @Test("Swift P031 enum raw values match approved GraphQL schema contract")
  func swiftEnumsMatchApprovedP031SchemaContract() {
    #expect(
      P031FreshnessState.allCases.map(\.rawValue) == [
        "live",
        "refreshing",
        "projection_lag",
        "stale",
        "unavailable",
        "unauthorized",
      ])
    #expect(
      P031DisabledReasonCode.allCases.map(\.rawValue) == [
        "WRITE_PATH_NOT_AVAILABLE",
        "MANAGED_OUTSIDE_UI",
        "AMBIGUOUS_APPROVAL_IDENTITY",
        "STALE_READ",
        "PROJECTION_LAG",
        "UNAUTHORIZED",
        "UNSUPPORTED_ACTION",
      ])
    #expect(
      P031WritePathState.allCases.map(\.rawValue) == [
        "read_only_diagnostic",
        "write_path_not_available",
        "external_transport_required",
        "hidden",
      ])
    #expect(
      P031PayloadAvailabilityState.allCases.map(\.rawValue) == [
        "available",
        "metadata_only",
        "payload_deferred",
        "generating",
        "unavailable",
      ])
    #expect(
      P031PayloadUnavailableReasonCode.allCases.map(\.rawValue) == [
        "PAYLOAD_DEFERRED_BY_P031",
        "GENERATING",
        "NOT_INDEXED",
        "NOT_AUTHORIZED",
        "NOT_AVAILABLE",
        "UNKNOWN",
      ])
  }

  @Test("Workflow read store issues P031 query documents and decodes subscription frames")
  func workflowReadStoreUsesGraphQLReadContracts() async throws {
    let readTransport = CapturingP031ReadTransport(
      responses: [
        "P031RunsHome": Data(
          """
          {"data":{"runs":[{"id":"run-1","status":"running","workflowTitle":"Full MVP","freshnessState":"live","totalStages":4,"completedStages":2,"failedStages":0,"pendingApprovals":1}]}}
          """.utf8),
        "P031ApprovalInbox": Data(
          """
          {"data":{"approvalInbox":[{"id":"approval-1","runId":"run-1","stageId":"stage-1","decision":"pending","freshnessState":"live","disabledReasonCode":"WRITE_PATH_NOT_AVAILABLE","writePathState":"read_only_diagnostic","diagnosticId":"approval-1","serverDebugDetail":null}]}}
          """.utf8),
        "P031ReportMetadata": Data(
          """
          {"data":{"artifacts":[{"id":"artifact-1","name":"summary","format":"json","reportKind":null,"reportVersion":null,"freshnessState":"live","payloadAvailabilityState":"available","payloadUnavailableReasonCode":null,"diagnosticId":null,"serverDebugDetail":null},{"id":"report-1","name":"report","format":"report","reportKind":"release","reportVersion":1,"freshnessState":"live","payloadAvailabilityState":"metadata_only","payloadUnavailableReasonCode":"PAYLOAD_DEFERRED_BY_P031","diagnosticId":"report-1","serverDebugDetail":null}]}}
          """.utf8),
        "P031DaemonStatus": try daemonStatusGraphQLResponse(
          fieldName: "daemonStatus",
          status: daemonStatusJSON(state: "ready")
        ),
      ])
    let subscriptionTransport = CapturingP031SubscriptionTransport(
      frames: [
        Data(
          """
          {"data":{"runStatusChanged":{"id":"run-1","status":"completed","freshnessState":"live","projectionUpdatedAt":"2026-04-22T00:00:00Z","projectionLag":false}}}
          """.utf8)
      ])
    let store = P031GraphQLWorkflowReadStore(
      readTransport: readTransport,
      subscriptionTransport: subscriptionTransport
    )

    let runs = try await store.fetchRuns()
    let approvals = try await store.fetchApprovalInbox()
    let reports = try await store.fetchReportMetadata(runID: "run-1")
    let daemonStatus = try await store.fetchDaemonStatus()
    let statusStream = try store.subscribeToRunStatus(runID: "run-1")
    let statusEvent = try await firstValue(from: statusStream)

    #expect(runs.map(\.id) == ["run-1"])
    #expect(approvals.map(\.id) == ["approval-1"])
    #expect(reports.map(\.id) == ["report-1"])
    #expect(reports.first?.payloadAvailabilityState == .metadataOnly)
    #expect(daemonStatus.state == .ready)
    #expect(statusEvent?.status == "completed")
    #expect(
      readTransport.requests.map(\.operationName) == [
        "P031RunsHome", "P031ApprovalInbox", "P031ReportMetadata", "P031DaemonStatus",
      ])
    #expect(
      readTransport.requests.first { $0.operationName == "P031ReportMetadata" }?.variables == [
        "runId": "run-1"
      ])
    #expect(subscriptionTransport.requests.first?.operationKind == .subscription)
  }

  @Test("Workflow read store subscribes to daemon lifecycle through GraphQL reads")
  func workflowReadStoreCoversDaemonLifecycleSubscription() async throws {
    let readTransport = CapturingP031ReadTransport()
    let subscriptionTransport = CapturingP031SubscriptionTransport(
      frames: [
        try daemonStatusGraphQLResponse(
          fieldName: "daemonStatusChanged",
          status: daemonStatusJSON(state: "degraded")
        )
      ])
    let store = P031GraphQLWorkflowReadStore(
      readTransport: readTransport,
      subscriptionTransport: subscriptionTransport
    )

    let stream = try store.subscribeToDaemonStatus()
    let event = try await firstValue(from: stream)

    #expect(event?.state == .degraded)
    #expect(subscriptionTransport.requests.map(\.operationName) == ["P031DaemonStatusChanged"])
    #expect(
      subscriptionTransport.requests.first?.document == P031GraphQLDocuments.daemonStatusChanged)
    #expect(subscriptionTransport.requests.first?.operationKind == .subscription)
  }

  @Test("Workflow read store covers run detail, stages, and artifacts through read queries")
  func workflowReadStoreCoversRunDetailStagesAndArtifacts() async throws {
    let readTransport = CapturingP031ReadTransport(
      responses: [
        "P031RunDetail": Data(
          """
          {"data":{"run":{"id":"run-1","status":"running","workflowTitle":"Full MVP","freshnessState":"live","totalStages":2,"completedStages":1,"failedStages":0,"pendingApprovals":1},"stages":[{"id":"stage-exec-1","runId":"run-1","stageId":"state_1","label":"Stage 1","status":"running","iteration":1,"attemptNumber":1,"settlementKind":null,"hasArtifacts":true,"hasPendingApproval":false,"hasValidationFailure":false,"projectionPresent":true,"projectionUpdatedAt":"2026-04-22T00:00:00Z","projectionLag":false,"freshnessState":"live"}],"artifacts":[{"id":"artifact-1","runId":"run-1","stageId":"state_1","agentId":"agent","name":"summary","contractId":"summary","format":"json","isPinned":false,"reportKind":null,"reportVersion":null,"outputSettlement":null,"sourceGenerationVerified":true,"freshnessState":"live","payloadAvailabilityState":"available","payloadUnavailableReasonCode":null,"diagnosticId":null,"serverDebugDetail":null},{"id":"report-1","runId":"run-1","stageId":"state_1","agentId":"agent","name":"release report","contractId":"release-report","format":"report","isPinned":true,"reportKind":"release","reportVersion":1,"outputSettlement":null,"sourceGenerationVerified":true,"freshnessState":"projection_lag","payloadAvailabilityState":"metadata_only","payloadUnavailableReasonCode":"PAYLOAD_DEFERRED_BY_P031","diagnosticId":"report-1","serverDebugDetail":null}],"approvalInbox":[{"id":"approval-1","runId":"run-1","stageId":"stage-1","decision":"pending","freshnessState":"live","disabledReasonCode":"MANAGED_OUTSIDE_UI","writePathState":"external_transport_required","diagnosticId":"approval-1","serverDebugDetail":null},{"id":"approval-other","runId":"run-other","stageId":"stage-other","decision":"pending","freshnessState":"live","disabledReasonCode":"MANAGED_OUTSIDE_UI","writePathState":"external_transport_required","diagnosticId":"approval-other","serverDebugDetail":null}]}}
          """.utf8),
        "P031StageDetail": Data(
          """
          {"data":{"stage":{"id":"stage-exec-1","runId":"run-1","stageId":"state_1","label":"Stage 1","status":"running","iteration":1,"attemptNumber":1,"settlementKind":null,"hasArtifacts":true,"hasPendingApproval":false,"hasValidationFailure":false,"projectionPresent":true,"projectionUpdatedAt":"2026-04-22T00:00:00Z","projectionLag":false,"freshnessState":"live"}}}
          """.utf8),
        "P031Stages": Data(
          """
          {"data":{"stages":[{"id":"stage-exec-1","runId":"run-1","stageId":"state_1","label":"Stage 1","status":"running","iteration":1,"attemptNumber":1,"settlementKind":null,"hasArtifacts":true,"hasPendingApproval":false,"hasValidationFailure":false,"projectionPresent":true,"projectionUpdatedAt":"2026-04-22T00:00:00Z","projectionLag":false,"freshnessState":"live"}]}}
          """.utf8),
        "P031Artifacts": Data(
          """
          {"data":{"artifacts":[{"id":"artifact-1","runId":"run-1","stageId":"state_1","agentId":"agent","name":"summary","contractId":"summary","format":"json","isPinned":false,"reportKind":null,"reportVersion":null,"outputSettlement":null,"sourceGenerationVerified":true,"freshnessState":"live","payloadAvailabilityState":"available","payloadUnavailableReasonCode":null,"diagnosticId":null,"serverDebugDetail":null}]}}
          """.utf8),
      ])
    let store = P031GraphQLWorkflowReadStore(
      readTransport: readTransport,
      subscriptionTransport: CapturingP031SubscriptionTransport()
    )

    let detail = try await store.fetchRunDetail(runID: "run-1")
    let stageDetail = try await store.fetchStageDetail(stageExecutionID: "stage-exec-1")
    let stages = try await store.fetchStages(runID: "run-1")
    let artifacts = try await store.fetchArtifacts(runID: "run-1")

    #expect(detail.run?.id == "run-1")
    #expect(stageDetail.stage?.id == "stage-exec-1")
    #expect(detail.stages.map(\.stageID) == ["state_1"])
    #expect(detail.approvalsForRun.map(\.id) == ["approval-1"])
    #expect(detail.ordinaryArtifacts.map(\.id) == ["artifact-1"])
    #expect(detail.reportMetadata.map(\.id) == ["report-1"])
    #expect(stages.first?.freshnessState == .live)
    #expect(artifacts.first?.payloadAvailabilityState == .available)
    #expect(
      readTransport.requests.map(\.operationName) == [
        "P031RunDetail", "P031StageDetail", "P031Stages", "P031Artifacts",
      ])
    #expect(readTransport.requests.allSatisfy { $0.operationKind == .query })
    #expect(
      readTransport.requests.first { $0.operationName == "P031RunDetail" }?.document
        .contains("approvalInbox(runId: $runId)") == true)
    #expect(
      readTransport.requests.map(\.variables) == [
        ["runId": "run-1"],
        ["stageExecutionId": "stage-exec-1"],
        ["runId": "run-1"],
        ["runId": "run-1"],
      ])
  }

  @Test("Workflow read store accepts injected P031 document sets")
  func workflowReadStoreUsesInjectedDocuments() async throws {
    let customDocuments = P031GraphQLDocumentSet(
      runsHome:
        "query P031RunsHome { runs { id status workflowTitle freshnessState totalStages completedStages failedStages pendingApprovals } }",
      runDetail: P031GraphQLDocuments.runDetail,
      stageDetail: P031GraphQLDocuments.stageDetail,
      stages: P031GraphQLDocuments.stages,
      approvalInbox: P031GraphQLDocuments.approvalInbox,
      artifacts: P031GraphQLDocuments.artifacts,
      reportMetadata: P031GraphQLDocuments.reportMetadata,
      daemonStatus: P031GraphQLDocuments.daemonStatus,
      runStatusChanged: P031GraphQLDocuments.runStatusChanged,
      daemonStatusChanged: P031GraphQLDocuments.daemonStatusChanged
    )
    let readTransport = CapturingP031ReadTransport(
      responses: [
        "P031RunsHome": Data(
          """
          {"data":{"runs":[{"id":"run-1","status":"running","workflowTitle":"Full MVP","freshnessState":"live","totalStages":4,"completedStages":2,"failedStages":0,"pendingApprovals":1}]}}
          """.utf8)
      ])
    let store = P031GraphQLWorkflowReadStore(
      readTransport: readTransport,
      subscriptionTransport: CapturingP031SubscriptionTransport(),
      documents: customDocuments
    )

    _ = try await store.fetchRuns()

    #expect(readTransport.requests.first?.document == customDocuments.runsHome)
  }

  @Test("In-memory workflow read store is a read-only test double")
  func inMemoryWorkflowReadStoreIsReadOnlyDouble() async throws {
    let store = P031InMemoryWorkflowReadStore(
      runs: [
        P031RunRowReadModel(
          id: "run-1",
          status: "running",
          workflowTitle: "Workflow",
          freshnessState: .stale,
          totalStages: 2,
          completedStages: 1,
          failedStages: 0,
          pendingApprovals: 0
        )
      ],
      runStatusEvents: [
        "run-1": [
          P031RunStatusChangedReadModel(
            id: "run-1",
            status: "running",
            freshnessState: .stale,
            projectionUpdatedAt: nil,
            projectionLag: true
          )
        ]
      ],
      daemonStatusEvents: [makeDaemonStatus(state: .ready)]
    )

    #expect((try await store.fetchRuns()).first?.freshnessState == .stale)
    let event = try await firstValue(from: try store.subscribeToRunStatus(runID: "run-1"))
    #expect(event?.projectionLag == true)
    let daemonEvent = try await firstValue(from: try store.subscribeToDaemonStatus())
    #expect(daemonEvent?.state == .ready)
  }

  @Test("Freshness reducer uses server states and preserves stale server truth on no-newer refresh")
  func freshnessReducerPreservesServerTruthWithoutLocalInference() {
    let start = Date(timeIntervalSince1970: 10)
    let checked = Date(timeIntervalSince1970: 20)
    let snapshot = P031FreshnessSnapshot(
      state: .projectionLag, lastCheckedAt: start, reason: "lagging")

    let refreshing = WorkflowFreshnessReducer.reduce(snapshot, event: .refreshStarted(at: checked))
    #expect(refreshing.state == .refreshing)
    #expect(refreshing.reason == "lagging")

    let noNewerProjection = WorkflowFreshnessReducer.reduce(
      snapshot,
      event: .refreshCompletedWithoutNewProjection(
        checkedAt: checked, reason: "no newer projection")
    )
    #expect(noNewerProjection.state == .projectionLag)
    #expect(noNewerProjection.lastCheckedAt == checked)
    #expect(noNewerProjection.reason == "no newer projection")

    let serverLive = WorkflowFreshnessReducer.reduce(
      snapshot,
      event: .serverStateReceived(.live, checkedAt: checked, reason: nil)
    )
    #expect(serverLive.state == .live)
  }

  @Test("Targeted read refresh uses stable feedback and server freshness")
  func targetedReadRefreshUsesReadStoreAndServerFreshness() async {
    let checked = Date(timeIntervalSince1970: 30)
    let coordinator = P031TargetedReadRefreshCoordinator(
      store: P031InMemoryWorkflowReadStore(
        approvalInbox: [
          P031ApprovalReadModel(
            id: "approval-1",
            runID: "run-1",
            stageID: "stage-1",
            decision: "pending",
            freshnessState: .projectionLag,
            disabledReasonCode: .writePathNotAvailable,
            writePathState: .readOnlyDiagnostic,
            diagnosticID: "approval-1",
            serverDebugDetail: nil
          )
        ]
      )
    )

    let outcome = await coordinator.refreshApprovalInbox(
      currentFreshness: P031FreshnessSnapshot(state: .live),
      checkedAt: checked
    )

    #expect(outcome.succeeded)
    #expect(outcome.feedbackText == "Updating approvals")
    #expect(outcome.value?.map(\.id) == ["approval-1"])
    #expect(outcome.freshness.state == .projectionLag)
    #expect(outcome.freshness.lastCheckedAt == checked)
  }

  @Test("Targeted read refresh aggregates run detail, stage, and artifact freshness")
  func targetedReadRefreshCoversDetailStagesAndArtifacts() async {
    let checked = Date(timeIntervalSince1970: 35)
    let run = P031RunRowReadModel(
      id: "run-1",
      status: "running",
      workflowTitle: "Workflow",
      freshnessState: .live,
      totalStages: 2,
      completedStages: 1,
      failedStages: 0,
      pendingApprovals: 0
    )
    let stage = P031StageReadModel(
      id: "stage-exec-1",
      runID: "run-1",
      stageID: "state_1",
      label: "Stage 1",
      status: "running",
      iteration: 1,
      attemptNumber: 1,
      settlementKind: nil,
      hasArtifacts: true,
      hasPendingApproval: false,
      hasValidationFailure: false,
      projectionPresent: true,
      projectionUpdatedAt: "2026-04-22T00:00:00Z",
      projectionLag: false,
      freshnessState: .stale
    )
    let artifact = P031ArtifactReadModel(
      id: "artifact-1",
      runID: "run-1",
      stageID: "state_1",
      agentID: "agent",
      name: "summary",
      contractID: "summary",
      format: "json",
      isPinned: false,
      reportKind: nil,
      reportVersion: nil,
      outputSettlement: nil,
      sourceGenerationVerified: true,
      freshnessState: .live,
      payloadAvailabilityState: .available,
      payloadUnavailableReasonCode: nil,
      diagnosticID: nil,
      serverDebugDetail: nil
    )
    let coordinator = P031TargetedReadRefreshCoordinator(
      store: P031InMemoryWorkflowReadStore(
        runDetailsByRunID: [
          "run-1": P031RunDetailReadModel(run: run, stages: [stage], artifacts: [artifact])
        ],
        stageDetailsByStageExecutionID: [
          "stage-exec-1": P031StageDetailReadModel(stage: stage)
        ],
        stagesByRunID: ["run-1": [stage]],
        artifactsByRunID: ["run-1": [artifact]],
        reportsByRunID: [
          "run-1": [
            P031ReportMetadataReadModel(
              id: "report-1",
              name: "release report",
              format: "report",
              reportKind: "release",
              reportVersion: 1,
              freshnessState: .projectionLag,
              payloadAvailabilityState: .metadataOnly,
              payloadUnavailableReasonCode: .payloadDeferredByP031,
              diagnosticID: "report-1",
              serverDebugDetail: nil
            )
          ]
        ],
        daemonStatus: makeDaemonStatus(state: .ready)
      )
    )

    let detailOutcome = await coordinator.refreshRunDetail(
      runID: "run-1",
      currentFreshness: P031FreshnessSnapshot(state: .live),
      checkedAt: checked
    )
    let stageDetailOutcome = await coordinator.refreshStageDetail(
      stageExecutionID: "stage-exec-1",
      currentFreshness: P031FreshnessSnapshot(state: .live),
      checkedAt: checked
    )
    let stagesOutcome = await coordinator.refreshStages(
      runID: "run-1",
      currentFreshness: P031FreshnessSnapshot(state: .live),
      checkedAt: checked
    )
    let artifactsOutcome = await coordinator.refreshArtifacts(
      runID: "run-1",
      currentFreshness: P031FreshnessSnapshot(state: .projectionLag),
      checkedAt: checked
    )
    let reportsOutcome = await coordinator.refreshReportMetadata(
      runID: "run-1",
      currentFreshness: P031FreshnessSnapshot(state: .live),
      checkedAt: checked
    )
    let daemonOutcome = await coordinator.refreshDaemonStatus(
      currentFreshness: P031FreshnessSnapshot(state: .stale),
      checkedAt: checked
    )

    #expect(detailOutcome.feedbackText == "Checking latest data")
    #expect(detailOutcome.freshness.state == .stale)
    #expect(stageDetailOutcome.feedbackText == "Updating stage")
    #expect(stageDetailOutcome.freshness.state == .stale)
    #expect(stagesOutcome.feedbackText == "Updating stages")
    #expect(stagesOutcome.freshness.state == .stale)
    #expect(artifactsOutcome.feedbackText == "Refreshing artifacts")
    #expect(artifactsOutcome.freshness.state == .live)
    #expect(reportsOutcome.feedbackText == "Refreshing reports")
    #expect(reportsOutcome.freshness.state == .projectionLag)
    #expect(daemonOutcome.feedbackText == "Checking daemon status")
    #expect(daemonOutcome.freshness.state == .live)
  }

  @Test("Targeted read refresh preserves current freshness when no newer projection returns")
  func targetedReadRefreshPreservesFreshnessWhenNoProjectionReturns() async {
    let checked = Date(timeIntervalSince1970: 40)
    let current = P031FreshnessSnapshot(
      state: .stale,
      lastCheckedAt: Date(timeIntervalSince1970: 20),
      reason: "server stale"
    )
    let coordinator = P031TargetedReadRefreshCoordinator(
      store: P031InMemoryWorkflowReadStore()
    )

    let outcome = await coordinator.refreshReportMetadata(
      runID: "run-1",
      currentFreshness: current,
      checkedAt: checked
    )

    #expect(outcome.succeeded)
    #expect(outcome.feedbackText == "Refreshing reports")
    #expect(outcome.value == [])
    #expect(outcome.freshness.state == .stale)
    #expect(outcome.freshness.lastCheckedAt == checked)
    #expect(outcome.freshness.reason == "No newer projection returned")
  }

  @Test("Read refresh presenter uses P031-approved stable wording")
  func readRefreshPresenterUsesStableWording() {
    #expect(P031ReadRefreshPresenter.feedbackText(for: .runsHome) == "Checking latest data")
    #expect(P031ReadRefreshPresenter.feedbackText(for: .stageDetail) == "Updating stage")
    #expect(P031ReadRefreshPresenter.feedbackText(for: .reportMetadata) == "Refreshing reports")
    #expect(P031ReadRefreshPresenter.feedbackText(for: .approvalsQueue) == "Updating approvals")
    #expect(P031ReadRefreshPresenter.feedbackText(for: .artifacts) == "Refreshing artifacts")
    #expect(
      P031ReadRefreshPresenter.feedbackText(for: .daemonLifecycle) == "Checking daemon status")
  }

  @Test("Approval diagnostics decode GraphQL enums and render copy-only unavailable state")
  func approvalDiagnosticsRenderReadOnlyGuidance() throws {
    let approval = try JSONDecoder().decode(
      P031ApprovalReadModel.self,
      from: Data(
        """
        {
          "id": "approval-1",
          "runId": "run-1",
          "stageId": "stage-1",
          "decision": "pending",
          "freshnessState": "live",
          "disabledReasonCode": "WRITE_PATH_NOT_AVAILABLE",
          "writePathState": "read_only_diagnostic",
          "diagnosticId": "approval-1",
          "serverDebugDetail": "P031 renders approval rows as diagnostic read-only"
        }
        """.utf8))

    let presentation = ApprovalDiagnosticPresenter.presentation(for: approval)
    #expect(presentation.title == "Approval write path unavailable")
    #expect(presentation.actionLabel == nil)
    #expect(presentation.followUpID == "P031-FOLLOWUP-APPROVAL-WRITE-PATH")
    #expect(
      presentation.copyItems.map(\.label) == ["approval_id", "run_id", "stage_id", "diagnostic_id"])
  }

  @Test("Approval diagnostics only expose CLI action when the guide explicitly documents CLI")
  func approvalDiagnosticsRequireDocumentedCLIWorkflow() throws {
    let approval = P031ApprovalReadModel(
      id: "approval-1",
      runID: "run-1",
      stageID: "stage-1",
      decision: "pending",
      freshnessState: .live,
      disabledReasonCode: .managedOutsideUI,
      writePathState: .externalTransportRequired,
      diagnosticID: "approval-1",
      serverDebugDetail: nil
    )

    #expect(ApprovalDiagnosticPresenter.presentation(for: approval).actionLabel == nil)
    #expect(
      ApprovalDiagnosticPresenter.presentation(
        for: approval,
        externalWritePathGuideState: .documented(.cli)
      ).actionLabel == "Execute via CLI"
    )
  }

  @Test("Operator write-path guide derives approval CLI availability from validated rows")
  func operatorWritePathGuideDerivesApprovalCLIAvailability() throws {
    let guide = try JSONDecoder().decode(
      P031OperatorWritePathGuide.self,
      from: approvalCLIWritePathGuideData()
    )
    let approval = P031ApprovalReadModel(
      id: "approval-1",
      runID: "run-1",
      stageID: "stage-1",
      decision: "pending",
      freshnessState: .live,
      disabledReasonCode: .managedOutsideUI,
      writePathState: .externalTransportRequired,
      diagnosticID: "approval-1",
      serverDebugDetail: nil
    )

    let state = guide.approvalResolutionState()
    let presentation = ApprovalDiagnosticPresenter.presentation(
      for: approval,
      externalWritePathGuideState: state
    )

    #expect(state == .documented(.cli))
    #expect(presentation.actionLabel == "Execute via CLI")
    #expect(presentation.followUpID == nil)
  }

  @Test("Operator write-path guide separates external guide availability from CLI execution")
  func operatorWritePathGuideDocumentsNonCLIApprovalWorkflowWithoutCLIAction() throws {
    let guide = try JSONDecoder().decode(
      P031OperatorWritePathGuide.self,
      from: mcpTerminalApprovalWritePathGuideData()
    )
    let approval = P031ApprovalReadModel(
      id: "approval-1",
      runID: "run-1",
      stageID: "stage-1",
      decision: "pending",
      freshnessState: .live,
      disabledReasonCode: .managedOutsideUI,
      writePathState: .externalTransportRequired,
      diagnosticID: "approval-1",
      serverDebugDetail: nil
    )

    let state = guide.approvalResolutionState()
    let presentation = ApprovalDiagnosticPresenter.presentation(
      for: approval,
      externalWritePathGuideState: state
    )
    let orientation = P031FirstRunOrientationPresenter.presentation(writePathGuideState: state)

    #expect(state == .documented(.mcpTerminal))
    #expect(state.guideAvailable)
    #expect(!state.cliWorkflowDocumented)
    #expect(presentation.title == "Approval managed outside UI")
    #expect(presentation.actionLabel == nil)
    #expect(presentation.followUpID == nil)
    #expect(orientation.externalWritePathLabel == "Open external write-path guide")
  }

  @Test("Operator write-path guide presenter summarizes rows without UI execution")
  func operatorWritePathGuidePresenterSummarizesExternalWorkflowsReadOnly() throws {
    let guide = try JSONDecoder().decode(
      P031OperatorWritePathGuide.self,
      from: Data(
        """
        {
          "schema_version": "p031-operator-write-path-guide-v1",
          "rows": [
            {
              "removed_control_id": "approvals.resolve",
              "removed_control_label": "Approve or reject approval",
              "external_workflow_kind": "CLI",
              "external_workflow_name_or_tool": "chainworks approvals resolve",
              "required_identifiers": ["approval_id", "run_id", "stage_id"],
              "minimum_parameter_shape": "approval_id plus binary decision",
              "unavailable_reason": null,
              "expected_success_output": "approval resolved",
              "follow_up_id": null,
              "operator_notes": "Use copied identifiers from the approval diagnostic row.",
              "validation_status": "validated"
            },
            {
              "removed_control_id": "runs.cancel",
              "removed_control_label": "Cancel run",
              "external_workflow_kind": "temporarily unavailable",
              "external_workflow_name_or_tool": null,
              "required_identifiers": ["run_id"],
              "minimum_parameter_shape": null,
              "unavailable_reason": "P031-FOLLOWUP-WRITE-PATH",
              "expected_success_output": null,
              "follow_up_id": "P031-FOLLOWUP-WRITE-PATH",
              "operator_notes": null,
              "validation_status": "pending"
            },
            {
              "removed_control_id": "stages.retry",
              "removed_control_label": "Retry stage",
              "external_workflow_kind": "CLI",
              "external_workflow_name_or_tool": "chainworks stages retry",
              "required_identifiers": ["run_id", "stage_id"],
              "minimum_parameter_shape": "run_id and stage_id",
              "unavailable_reason": null,
              "expected_success_output": "stage retry queued",
              "follow_up_id": null,
              "operator_notes": null,
              "validation_status": "pending"
            }
          ]
        }
        """.utf8))

    let presentation = P031OperatorWritePathGuidePresenter.presentation(for: guide)

    #expect(presentation.availableExternalWorkflowCount == 1)
    #expect(presentation.unavailableCount == 1)
    #expect(presentation.pendingOrInvalidCount == 1)
    #expect(presentation.emptyStateTitle == nil)
    #expect(
      presentation.rows.map(\.statusLabel) == [
        "External workflow documented",
        "Temporarily unavailable",
        "Pending validation",
      ])
    #expect(presentation.rows.first?.workflowLabel == "CLI")
    #expect(presentation.rows.first?.toolLabel == "chainworks approvals resolve")
    #expect(
      presentation.rows.first?.requiredIdentifierLabels == [
        "approval_id", "run_id", "stage_id",
      ])
    #expect(presentation.rows[1].followUpID == "P031-FOLLOWUP-WRITE-PATH")
    #expect(presentation.rows.allSatisfy { !$0.canExecuteFromUI })
  }

  @Test("Operator write-path guide resolver derives read-only state from supplied data")
  func operatorWritePathGuideResolverDerivesReadOnlyStateFromData() throws {
    let resolution = P031OperatorWritePathGuideResolver.resolve(
      from: approvalCLIWritePathGuideData()
    )

    #expect(resolution.approvalResolutionState == .documented(.cli))
    #expect(resolution.summaryPresentation.availableExternalWorkflowCount == 1)
    #expect(resolution.summaryPresentation.unavailableCount == 12)
    #expect(resolution.summaryPresentation.rows.allSatisfy { !$0.canExecuteFromUI })
    #expect(resolution.errorDescription == nil)
  }

  @Test("Thin workflow coordinator exposes read-only operator guide summary from supplied data")
  func thinWorkflowCoordinatorExposesOperatorGuideSummary() {
    let coordinator = P031ThinWorkflowScreenCoordinator(
      store: P031InMemoryWorkflowReadStore(),
      writePathGuideData: approvalCLIWritePathGuideData()
    )

    let summary = coordinator.loadOperatorWritePathGuideSummary()

    #expect(coordinator.writePathGuideState == .documented(.cli))
    #expect(coordinator.writePathGuideErrorDescription == nil)
    #expect(
      Array(summary.rows.map(\.removedControlID).prefix(2)) == ["approvals.resolve", "runs.cancel"])
    #expect(summary.availableExternalWorkflowCount == 1)
    #expect(summary.unavailableCount == 12)
    #expect(summary.rows.allSatisfy { !$0.canExecuteFromUI })
  }

  @Test("Thin workflow coordinator fails closed when operator guide data is absent")
  func thinWorkflowCoordinatorFailsClosedWithoutOperatorGuideData() {
    let coordinator = P031ThinWorkflowScreenCoordinator(
      store: P031InMemoryWorkflowReadStore(),
      writePathGuideData: nil
    )

    let summary = coordinator.loadOperatorWritePathGuideSummary()

    #expect(coordinator.writePathGuideState == .unavailable)
    #expect(coordinator.writePathGuideErrorDescription == nil)
    #expect(summary.emptyStateTitle == "External write-path guide unavailable")
    #expect(summary.rows.isEmpty)
  }

  @Test("Guide bootstrap loads the machine-readable guide from the repository root")
  func operatorWritePathGuideBootstrapLoadsRepositoryGuide() throws {
    let repoRoot = try temporaryRepositoryRoot()
    defer { try? FileManager.default.removeItem(at: repoRoot) }
    let guideURL = repoRoot
      .appendingPathComponent("docs/reference", isDirectory: true)
      .appendingPathComponent("p031-operator-write-path-guide.json")
    try FileManager.default.createDirectory(
      at: guideURL.deletingLastPathComponent(),
      withIntermediateDirectories: true
    )
    try approvalCLIWritePathGuideData().write(to: guideURL)

    let resource = P031OperatorWritePathGuideBootstrap.load(
      currentDirectoryPath: repoRoot.path,
      bundledURL: nil,
      sourceFilePath: repoRoot
        .appendingPathComponent("Chainworks Forge/Views/RunsHomeView.swift").path
    )

    #expect(resource.url == guideURL)
    let data = try #require(resource.data)
    let loadedGuide = try JSONDecoder().decode(P031OperatorWritePathGuide.self, from: data)
    let expectedGuide = try JSONDecoder().decode(
      P031OperatorWritePathGuide.self,
      from: approvalCLIWritePathGuideData()
    )
    #expect(loadedGuide == expectedGuide)
  }

  @Test("Operator write-path guide resolver fails closed for missing malformed or stale data")
  func operatorWritePathGuideResolverFailsClosedForUnavailableData() {
    let missing = P031OperatorWritePathGuideResolver.resolve(from: nil)
    let malformed = P031OperatorWritePathGuideResolver.resolve(from: Data("{".utf8))
    let stale = P031OperatorWritePathGuideResolver.resolve(
      from: Data(
        """
        {
          "schema_version": "p031-operator-write-path-guide-draft",
          "rows": []
        }
        """.utf8))
    let incomplete = P031OperatorWritePathGuideResolver.resolve(
      from: partialApprovalCLIWritePathGuideData()
    )

    #expect(missing.approvalResolutionState == .unavailable)
    #expect(missing.summaryPresentation.emptyStateTitle == "External write-path guide unavailable")
    #expect(missing.errorDescription == nil)
    #expect(malformed.approvalResolutionState == .unavailable)
    #expect(malformed.guide == nil)
    #expect(malformed.errorDescription != nil)
    #expect(stale.approvalResolutionState == .unavailable)
    #expect(stale.guide != nil)
    #expect(stale.errorDescription == "External write-path guide schema is unavailable")
    #expect(incomplete.approvalResolutionState == .unavailable)
    #expect(incomplete.guide?.missingRemovedControlCoverage.contains("agents.reset") == true)
    #expect(incomplete.errorDescription == "External write-path guide coverage is incomplete")
  }

  @Test("Operator write-path guide resolver mirrors gate row contract checks")
  func operatorWritePathGuideResolverFailsClosedForGateRejectedRows() {
    var missingKeyRows = completeWritePathGuideRows()
    var missingKeyApproval = missingKeyRows[0]
    missingKeyApproval.removeValue(forKey: "operator_notes")
    missingKeyRows[0] = missingKeyApproval

    var unknownWorkflowRows = completeWritePathGuideRows()
    unknownWorkflowRows[1]["external_workflow_kind"] = "shell"

    var unknownValidationRows = completeWritePathGuideRows()
    unknownValidationRows[1]["validation_status"] = "needs_review"

    var incompleteUnavailableRows = completeWritePathGuideRows()
    incompleteUnavailableRows[1]["unavailable_reason"] = " "

    let missingKey = P031OperatorWritePathGuideResolver.resolve(
      from: writePathGuideData(rows: missingKeyRows)
    )
    let unknownWorkflow = P031OperatorWritePathGuideResolver.resolve(
      from: writePathGuideData(rows: unknownWorkflowRows)
    )
    let unknownValidation = P031OperatorWritePathGuideResolver.resolve(
      from: writePathGuideData(rows: unknownValidationRows)
    )
    let incompleteUnavailable = P031OperatorWritePathGuideResolver.resolve(
      from: writePathGuideData(rows: incompleteUnavailableRows)
    )

    #expect(missingKey.approvalResolutionState == .unavailable)
    #expect(missingKey.guide?.rows.first?.missingContractKeys == ["operator_notes"])
    #expect(missingKey.errorDescription == "External write-path guide row contract is incomplete")
    #expect(unknownWorkflow.approvalResolutionState == .unavailable)
    #expect(unknownWorkflow.errorDescription == missingKey.errorDescription)
    #expect(unknownValidation.approvalResolutionState == .unavailable)
    #expect(unknownValidation.errorDescription == missingKey.errorDescription)
    #expect(incompleteUnavailable.approvalResolutionState == .unavailable)
    #expect(incompleteUnavailable.errorDescription == missingKey.errorDescription)
  }

  @Test("Operator write-path guide presenter fails closed for stale schema")
  func operatorWritePathGuidePresenterFailsClosedForStaleSchema() throws {
    let guide = try JSONDecoder().decode(
      P031OperatorWritePathGuide.self,
      from: Data(
        """
        {
          "schema_version": "p031-operator-write-path-guide-draft",
          "rows": [
            {
              "removed_control_id": "approvals.resolve",
              "removed_control_label": "Approve or reject approval",
              "external_workflow_kind": "CLI",
              "external_workflow_name_or_tool": "chainworks approvals resolve",
              "required_identifiers": ["approval_id", "run_id", "stage_id"],
              "minimum_parameter_shape": "approval_id plus binary decision",
              "unavailable_reason": null,
              "expected_success_output": "approval resolved",
              "follow_up_id": null,
              "operator_notes": null,
              "validation_status": "validated"
            }
          ]
        }
        """.utf8))

    let presentation = P031OperatorWritePathGuidePresenter.presentation(for: guide)

    #expect(presentation.rows.isEmpty)
    #expect(presentation.availableExternalWorkflowCount == 0)
    #expect(presentation.unavailableCount == 0)
    #expect(presentation.pendingOrInvalidCount == 0)
    #expect(presentation.emptyStateTitle == "External write-path guide unavailable")
  }

  @Test("Operator write-path guide does not expose CLI for incomplete or unavailable rows")
  func operatorWritePathGuideRequiresValidatedCLIIdentifiers() throws {
    let missingIdentifierGuide = try JSONDecoder().decode(
      P031OperatorWritePathGuide.self,
      from: Data(
        """
        {
          "schema_version": "p031-operator-write-path-guide-v1",
          "rows": [
            {
              "removed_control_id": "approvals.resolve",
              "removed_control_label": "Approve or reject approval",
              "external_workflow_kind": "CLI",
              "external_workflow_name_or_tool": "chainworks approvals resolve",
              "required_identifiers": ["approval_id", "run_id"],
              "minimum_parameter_shape": "approval_id plus binary decision",
              "unavailable_reason": null,
              "expected_success_output": "approval resolved",
              "follow_up_id": null,
              "operator_notes": null,
              "validation_status": "validated"
            }
          ]
        }
        """.utf8))
    let unavailableGuide = try JSONDecoder().decode(
      P031OperatorWritePathGuide.self,
      from: Data(
        """
        {
          "schema_version": "p031-operator-write-path-guide-v1",
          "rows": [
            {
              "removed_control_id": "approvals.resolve",
              "removed_control_label": "Approve or reject approval",
              "external_workflow_kind": "temporarily unavailable",
              "external_workflow_name_or_tool": null,
              "required_identifiers": ["approval_id", "run_id", "stage_id"],
              "minimum_parameter_shape": null,
              "unavailable_reason": "P031-FOLLOWUP-APPROVAL-WRITE-PATH",
              "expected_success_output": null,
              "follow_up_id": "P031-FOLLOWUP-APPROVAL-WRITE-PATH",
              "operator_notes": null,
              "validation_status": "validated"
            }
          ]
        }
        """.utf8))
    let pendingGuide = try JSONDecoder().decode(
      P031OperatorWritePathGuide.self,
      from: Data(
        """
        {
          "schema_version": "p031-operator-write-path-guide-v1",
          "rows": [
            {
              "removed_control_id": "approvals.resolve",
              "removed_control_label": "Approve or reject approval",
              "external_workflow_kind": "CLI",
              "external_workflow_name_or_tool": "chainworks approvals resolve",
              "required_identifiers": ["approval_id", "run_id", "stage_id"],
              "minimum_parameter_shape": "approval_id plus binary decision",
              "unavailable_reason": null,
              "expected_success_output": "approval resolved",
              "follow_up_id": null,
              "operator_notes": null,
              "validation_status": "pending"
            }
          ]
        }
        """.utf8))
    let missingToolGuide = try JSONDecoder().decode(
      P031OperatorWritePathGuide.self,
      from: Data(
        """
        {
          "schema_version": "p031-operator-write-path-guide-v1",
          "rows": [
            {
              "removed_control_id": "approvals.resolve",
              "removed_control_label": "Approve or reject approval",
              "external_workflow_kind": "CLI",
              "external_workflow_name_or_tool": " ",
              "required_identifiers": ["approval_id", "run_id", "stage_id"],
              "minimum_parameter_shape": "approval_id plus binary decision",
              "unavailable_reason": null,
              "expected_success_output": "approval resolved",
              "follow_up_id": null,
              "operator_notes": null,
              "validation_status": "validated"
            }
          ]
        }
        """.utf8))
    let missingSuccessOutputGuide = try JSONDecoder().decode(
      P031OperatorWritePathGuide.self,
      from: Data(
        """
        {
          "schema_version": "p031-operator-write-path-guide-v1",
          "rows": [
            {
              "removed_control_id": "approvals.resolve",
              "removed_control_label": "Approve or reject approval",
              "external_workflow_kind": "CLI",
              "external_workflow_name_or_tool": "chainworks approvals resolve",
              "required_identifiers": ["approval_id", "run_id", "stage_id"],
              "minimum_parameter_shape": "approval_id plus binary decision",
              "unavailable_reason": null,
              "expected_success_output": " ",
              "follow_up_id": null,
              "operator_notes": null,
              "validation_status": "validated"
            }
          ]
        }
        """.utf8))

    #expect(missingIdentifierGuide.approvalResolutionState() == .unavailable)
    #expect(unavailableGuide.approvalResolutionState() == .unavailable)
    #expect(pendingGuide.approvalResolutionState() == .unavailable)
    #expect(missingToolGuide.approvalResolutionState() == .unavailable)
    #expect(missingSuccessOutputGuide.approvalResolutionState() == .unavailable)
  }

  @Test("Operator write-path guide fails closed for stale or missing schema versions")
  func operatorWritePathGuideRequiresCurrentSchemaVersion() throws {
    let missingVersionGuide = try JSONDecoder().decode(
      P031OperatorWritePathGuide.self,
      from: Data(
        """
        {
          "rows": [
            {
              "removed_control_id": "approvals.resolve",
              "removed_control_label": "Approve or reject approval",
              "external_workflow_kind": "CLI",
              "external_workflow_name_or_tool": "chainworks approvals resolve",
              "required_identifiers": ["approval_id", "run_id", "stage_id"],
              "minimum_parameter_shape": "approval_id plus binary decision",
              "unavailable_reason": null,
              "expected_success_output": "approval resolved",
              "follow_up_id": null,
              "operator_notes": null,
              "validation_status": "validated"
            }
          ]
        }
        """.utf8))
    let staleVersionGuide = try JSONDecoder().decode(
      P031OperatorWritePathGuide.self,
      from: Data(
        """
        {
          "schema_version": "p031-operator-write-path-guide-draft",
          "rows": [
            {
              "removed_control_id": "approvals.resolve",
              "removed_control_label": "Approve or reject approval",
              "external_workflow_kind": "CLI",
              "external_workflow_name_or_tool": "chainworks approvals resolve",
              "required_identifiers": ["approval_id", "run_id", "stage_id"],
              "minimum_parameter_shape": "approval_id plus binary decision",
              "unavailable_reason": null,
              "expected_success_output": "approval resolved",
              "follow_up_id": null,
              "operator_notes": null,
              "validation_status": "validated"
            }
          ]
        }
        """.utf8))

    #expect(missingVersionGuide.approvalResolutionState() == .unavailable)
    #expect(staleVersionGuide.approvalResolutionState() == .unavailable)
  }

  @Test("Report metadata presenter blocks payload opening for P031 metadata-only rows")
  func reportMetadataPresentationBlocksDeferredPayload() throws {
    let report = try JSONDecoder().decode(
      P031ReportMetadataReadModel.self,
      from: Data(
        """
        {
          "id": "artifact-1",
          "name": "latest-report",
          "freshnessState": "live",
          "payloadAvailabilityState": "metadata_only",
          "payloadUnavailableReasonCode": "PAYLOAD_DEFERRED_BY_P031",
          "diagnosticId": "artifact-1",
          "serverDebugDetail": "P031 exposes report metadata only"
        }
        """.utf8))

    let presentation = PayloadUnavailableReasonPresenter.presentation(for: report)
    #expect(presentation.title == "Metadata")
    #expect(presentation.detail == "Payload rendering is deferred by P031")
    #expect(!presentation.canOpenPayload)
    #expect(
      presentation.copyItems == [
        P031DiagnosticCopyItem(label: "diagnostic_id", value: "artifact-1")
      ])
  }

  @Test("Report metadata row presenter exposes fixed payload indicators")
  func reportMetadataRowPresentationUsesFixedIndicators() {
    let metadataOnly = P031ReportMetadataReadModel(
      id: "artifact-1",
      name: "release summary",
      format: "report",
      reportKind: "release",
      reportVersion: 1,
      freshnessState: .live,
      payloadAvailabilityState: .metadataOnly,
      payloadUnavailableReasonCode: .payloadDeferredByP031,
      diagnosticID: "artifact-1",
      serverDebugDetail: nil
    )
    let available = P031ReportMetadataReadModel(
      id: "artifact-2",
      name: "signed report",
      format: "report",
      reportKind: "signed",
      reportVersion: 2,
      freshnessState: .live,
      payloadAvailabilityState: .available,
      payloadUnavailableReasonCode: nil,
      diagnosticID: nil,
      serverDebugDetail: nil
    )
    let untitled = P031ReportMetadataReadModel(
      id: "artifact-3",
      name: "   ",
      format: "report",
      reportKind: nil,
      reportVersion: nil,
      freshnessState: .live,
      payloadAvailabilityState: .unavailable,
      payloadUnavailableReasonCode: .unknown,
      diagnosticID: nil,
      serverDebugDetail: nil
    )

    let metadataOnlyRow = ReportMetadataRowPresenter.presentation(for: metadataOnly)
    let availableRow = ReportMetadataRowPresenter.presentation(for: available)
    let untitledRow = ReportMetadataRowPresenter.presentation(for: untitled)

    #expect(metadataOnlyRow.title == "release summary")
    #expect(metadataOnlyRow.availabilityLabel == "Metadata")
    #expect(metadataOnlyRow.availabilitySymbolName == "doc.text")
    #expect(metadataOnlyRow.payloadIndicatorSlotWidth == 96)
    #expect(!metadataOnlyRow.canOpenPayload)
    #expect(
      metadataOnlyRow.accessibilityLabel
        == "release summary, Metadata. Payload rendering is deferred by P031")
    #expect(availableRow.availabilityLabel == "Payload")
    #expect(availableRow.availabilitySymbolName == "doc.text.fill")
    #expect(availableRow.canOpenPayload)
    #expect(untitledRow.title == "Untitled report")
  }

  @Test("Diagnostic presenter redacts server debug details for non-operator readers")
  func diagnosticPresenterRedactsUnauthorizedDebugDetails() {
    #expect(
      DiagnosticDetailsPresenter.operatorDebugDetail("detail", operatorAuthorized: true) == "detail"
    )
    #expect(
      DiagnosticDetailsPresenter.operatorDebugDetail("detail", operatorAuthorized: false) == nil)
  }

  @Test("First-run orientation is local presentation state with write-path availability")
  func firstRunOrientationPresentationIsLocalOnly() {
    let available = P031FirstRunOrientationPresenter.presentation(
      writePathGuideState: .documented(.cli)
    )
    let unavailable = P031FirstRunOrientationPresenter.presentation(
      writePathGuideState: .unavailable)

    #expect(available.title == "GraphQL-only read mode")
    #expect(available.externalWritePathLabel == "Open external write-path guide")
    #expect(unavailable.externalWritePathLabel == "External write-path guide unavailable")
    #expect(available.canDismiss)
  }

  @Test("Thin workflow screen coordinator renders Runs Home from GraphQL read rows")
  func thinWorkflowScreenCoordinatorRendersRunsHomeReadState() async {
    let checked = Date(timeIntervalSince1970: 50)
    let coordinator = P031ThinWorkflowScreenCoordinator(
      store: P031InMemoryWorkflowReadStore(
        runs: [
          P031RunRowReadModel(
            id: "run-1",
            status: "projection_lag",
            workflowTitle: " Full MVP ",
            freshnessState: .projectionLag,
            totalStages: 4,
            completedStages: 2,
            failedStages: 0,
            pendingApprovals: 1
          )
        ]
      ),
      writePathGuideData: approvalCLIWritePathGuideData()
    )

    let presentation = await coordinator.loadRunsHome(
      currentFreshness: P031FreshnessSnapshot(state: .live),
      checkedAt: checked,
      showFirstRunOrientation: true
    )

    #expect(presentation.orientation?.externalWritePathLabel == "Open external write-path guide")
    #expect(presentation.rows.map(\.runID) == ["run-1"])
    #expect(presentation.rows.first?.title == "Full MVP")
    #expect(presentation.rows.first?.statusLabel == "Projection Lag")
    #expect(presentation.rows.first?.progressLabel == "2/4 stages")
    #expect(presentation.rows.first?.pendingApprovalsLabel == "1 approvals pending")
    #expect(presentation.freshness.state == .projectionLag)
    #expect(presentation.freshness.lastCheckedAt == checked)
    #expect(presentation.refreshFeedbackText == "Checking latest data")
    #expect(presentation.emptyStateTitle == nil)
    #expect(presentation.errorDescription == nil)
  }

  @Test("Thin workflow screen coordinator renders approval inbox diagnostic rows")
  func thinWorkflowScreenCoordinatorRendersApprovalDiagnostics() async {
    let checked = Date(timeIntervalSince1970: 55)
    let coordinator = P031ThinWorkflowScreenCoordinator(
      store: P031InMemoryWorkflowReadStore(
        approvalInbox: [
          P031ApprovalReadModel(
            id: "approval-1",
            runID: "run-1",
            stageID: "stage-1",
            decision: "pending",
            freshnessState: .stale,
            disabledReasonCode: .managedOutsideUI,
            writePathState: .externalTransportRequired,
            diagnosticID: "diag-1",
            serverDebugDetail: nil
          )
        ]
      ),
      writePathGuideState: .documented(.cli)
    )

    let presentation = await coordinator.loadApprovalInbox(
      currentFreshness: P031FreshnessSnapshot(state: .live),
      checkedAt: checked
    )

    #expect(presentation.rows.map(\.approvalID) == ["approval-1"])
    #expect(presentation.rows.first?.title == "Approval managed outside UI")
    #expect(presentation.rows.first?.body == "Managed outside UI")
    #expect(presentation.rows.first?.actionLabel == "Execute via CLI")
    #expect(presentation.rows.first?.followUpID == nil)
    #expect(
      presentation.rows.first?.accessibilityLabel
        == "Approval managed outside UI, Managed outside UI, run-1, stage-1")
    #expect(
      presentation.rows.first?.copyItems.map(\.label) == [
        "approval_id", "run_id", "stage_id", "diagnostic_id",
      ])
    #expect(presentation.freshness.state == .stale)
    #expect(presentation.refreshFeedbackText == "Updating approvals")
    #expect(presentation.emptyStateTitle == nil)
  }

  @Test("Thin workflow screen coordinator renders Run Detail and Stage Detail from GraphQL reads")
  func thinWorkflowScreenCoordinatorRendersDetailReadStates() async {
    let checked = Date(timeIntervalSince1970: 58)
    let run = P031RunRowReadModel(
      id: "run-1",
      status: "running",
      workflowTitle: "Full MVP",
      freshnessState: .live,
      totalStages: 2,
      completedStages: 1,
      failedStages: 0,
      pendingApprovals: 1
    )
    let stage = makeStage(
      id: "stage-exec-1",
      status: "projection_lag",
      hasPendingApproval: true,
      projectionLag: true,
      freshnessState: .stale
    )
    let artifact = makeArtifact(
      id: "artifact-1",
      name: "summary",
      payloadAvailabilityState: .metadataOnly,
      payloadUnavailableReasonCode: .payloadDeferredByP031,
      diagnosticID: "artifact-1"
    )
    let reportArtifact = P031ArtifactReadModel(
      id: "report-1",
      runID: "run-1",
      stageID: "state_1",
      agentID: "agent",
      name: "release report",
      contractID: "release-report",
      format: "report",
      isPinned: true,
      reportKind: "release",
      reportVersion: 1,
      outputSettlement: nil,
      sourceGenerationVerified: true,
      freshnessState: .projectionLag,
      payloadAvailabilityState: .metadataOnly,
      payloadUnavailableReasonCode: .payloadDeferredByP031,
      diagnosticID: "report-1",
      serverDebugDetail: nil
    )
    let approval = P031ApprovalReadModel(
      id: "approval-1",
      runID: "run-1",
      stageID: "stage-1",
      decision: "pending",
      freshnessState: .live,
      disabledReasonCode: .managedOutsideUI,
      writePathState: .externalTransportRequired,
      diagnosticID: "approval-1",
      serverDebugDetail: nil
    )
    let coordinator = P031ThinWorkflowScreenCoordinator(
      store: P031InMemoryWorkflowReadStore(
        runDetailsByRunID: [
          "run-1": P031RunDetailReadModel(
            run: run,
            stages: [stage],
            artifacts: [artifact, reportArtifact],
            approvalInbox: [approval]
          )
        ],
        stageDetailsByStageExecutionID: [
          "stage-exec-1": P031StageDetailReadModel(stage: stage)
        ],
        stagesByRunID: ["run-1": [stage]]
      ),
      writePathGuideState: .documented(.cli)
    )

    let runDetail = await coordinator.loadRunDetail(
      runID: "run-1",
      currentFreshness: P031FreshnessSnapshot(state: .live),
      checkedAt: checked
    )
    let stageDetail = await coordinator.loadStageDetail(
      stageExecutionID: "stage-exec-1",
      currentFreshness: P031FreshnessSnapshot(state: .live),
      checkedAt: checked
    )
    let stageList = await coordinator.loadStages(
      runID: "run-1",
      currentFreshness: P031FreshnessSnapshot(state: .live),
      checkedAt: checked
    )

    #expect(runDetail.title == "Full MVP")
    #expect(runDetail.statusLabel == "Running")
    #expect(runDetail.progressLabel == "1/2 stages")
    #expect(runDetail.pendingApprovalsLabel == "1 approvals pending")
    #expect(
      runDetail.stageRows.first?.badgeLabels == ["Approval pending", "Artifacts", "Projection lag"])
    #expect(runDetail.approvalRows.map(\.approvalID) == ["approval-1"])
    #expect(runDetail.approvalRows.first?.actionLabel == "Execute via CLI")
    #expect(runDetail.artifactRows.map(\.artifactID) == ["artifact-1"])
    #expect(runDetail.artifactRows.first?.payloadAvailabilityLabel == "Metadata")
    #expect(runDetail.artifactRows.first?.canOpenPayload == false)
    #expect(runDetail.artifactRows.first?.diagnosticCopyItems.map(\.label) == ["diagnostic_id"])
    #expect(runDetail.reportRows.map(\.title) == ["release report"])
    #expect(runDetail.reportRows.first?.availabilityLabel == "Metadata")
    #expect(runDetail.freshness.state == .stale)
    #expect(runDetail.refreshFeedbackText == "Checking latest data")
    #expect(stageDetail.stage?.stageExecutionID == "stage-exec-1")
    #expect(stageDetail.stage?.statusLabel == "Projection Lag")
    #expect(stageDetail.freshness.state == .stale)
    #expect(stageDetail.refreshFeedbackText == "Updating stage")
    #expect(stageList.rows.map { $0.stageExecutionID } == ["stage-exec-1"])
    #expect(
      stageList.rows.first?.badgeLabels == ["Approval pending", "Artifacts", "Projection lag"])
    #expect(stageList.freshness.state == P031FreshnessState.stale)
    #expect(stageList.refreshFeedbackText == "Updating stages")
  }

  @Test("Thin workflow screen coordinator renders artifacts, reports, and daemon lifecycle")
  func thinWorkflowScreenCoordinatorRendersAuxiliaryReadStates() async {
    let checked = Date(timeIntervalSince1970: 59)
    let artifact = makeArtifact(
      id: "artifact-1",
      name: "summary",
      payloadAvailabilityState: .metadataOnly,
      payloadUnavailableReasonCode: .payloadDeferredByP031,
      diagnosticID: "artifact-1"
    )
    let report = P031ReportMetadataReadModel(
      id: "report-1",
      name: " release summary ",
      format: "report",
      reportKind: "release",
      reportVersion: 1,
      freshnessState: .projectionLag,
      payloadAvailabilityState: .metadataOnly,
      payloadUnavailableReasonCode: .payloadDeferredByP031,
      diagnosticID: "report-1",
      serverDebugDetail: nil
    )
    let coordinator = P031ThinWorkflowScreenCoordinator(
      store: P031InMemoryWorkflowReadStore(
        artifactsByRunID: ["run-1": [artifact]],
        reportsByRunID: ["run-1": [report]],
        daemonStatus: makeDaemonStatus(state: .degraded)
      )
    )

    let artifacts = await coordinator.loadArtifacts(
      runID: "run-1",
      currentFreshness: P031FreshnessSnapshot(state: .live),
      checkedAt: checked
    )
    let reports = await coordinator.loadReportMetadata(
      runID: "run-1",
      currentFreshness: P031FreshnessSnapshot(state: .live),
      checkedAt: checked
    )
    let daemon = await coordinator.loadDaemonLifecycle(
      currentFreshness: P031FreshnessSnapshot(state: .stale),
      checkedAt: checked
    )

    #expect(artifacts.rows.map(\.artifactID) == ["artifact-1"])
    #expect(artifacts.rows.first?.payloadAvailabilityLabel == "Metadata")
    #expect(artifacts.refreshFeedbackText == "Refreshing artifacts")
    #expect(artifacts.freshness.state == .live)
    #expect(reports.rows.map(\.title) == ["release summary"])
    #expect(reports.rows.first?.availabilityLabel == "Metadata")
    #expect(reports.rows.first?.payloadIndicatorSlotWidth == 96)
    #expect(reports.freshness.state == .projectionLag)
    #expect(reports.refreshFeedbackText == "Refreshing reports")
    #expect(daemon.state == .degraded)
    #expect(daemon.title == "Daemon Degraded")
    #expect(daemon.badgeLabels == ["Degraded"])
    #expect(
      daemon.copyItems.map(\.label) == [
        "build_sha", "pid", "schema_version", "binary_schema_version",
      ])
    #expect(daemon.freshness.state == .live)
    #expect(daemon.refreshFeedbackText == "Checking daemon status")
  }

  @Test("Thin subscription coordinator presents server subscription freshness")
  func thinSubscriptionCoordinatorPresentsGraphQLSubscriptionEvents() async throws {
    let checked = Date(timeIntervalSince1970: 61)
    let coordinator = P031ThinWorkflowSubscriptionCoordinator(
      store: P031InMemoryWorkflowReadStore(
        runStatusEvents: [
          "run-1": [
            P031RunStatusChangedReadModel(
              id: "run-1",
              status: "projection_lag",
              freshnessState: .projectionLag,
              projectionUpdatedAt: "2026-04-22T00:00:00Z",
              projectionLag: true
            )
          ]
        ],
        daemonStatusEvents: [makeDaemonStatus(state: .ready)]
      )
    )

    let runStatus = try await firstValue(
      from: try coordinator.runStatusPresentations(
        runID: "run-1",
        currentFreshness: P031FreshnessSnapshot(state: .live),
        checkedAt: checked
      ))
    let daemon = try await firstValue(
      from: try coordinator.daemonLifecyclePresentations(
        currentFreshness: P031FreshnessSnapshot(state: .stale),
        checkedAt: checked
      ))

    #expect(runStatus?.runID == "run-1")
    #expect(runStatus?.statusLabel == "Projection Lag")
    #expect(runStatus?.badgeLabels == ["Projection lag"])
    #expect(runStatus?.freshness.state == .projectionLag)
    #expect(runStatus?.freshness.lastCheckedAt == checked)
    #expect(daemon?.state == .ready)
    #expect(daemon?.freshness.state == .live)
    #expect(daemon?.freshness.lastCheckedAt == checked)
  }

  @Test("Thin workflow screen coordinator renders fail-closed errors without local fallback")
  func thinWorkflowScreenCoordinatorRendersReadErrors() async {
    let checked = Date(timeIntervalSince1970: 60)
    let coordinator = P031ThinWorkflowScreenCoordinator(
      store: FailingP031WorkflowReadStore(),
      writePathGuideState: .unavailable
    )

    let presentation = await coordinator.loadRunsHome(
      currentFreshness: P031FreshnessSnapshot(state: .live),
      checkedAt: checked,
      showFirstRunOrientation: false
    )

    #expect(presentation.rows.isEmpty)
    #expect(presentation.orientation == nil)
    #expect(presentation.freshness.state == .unavailable)
    #expect(presentation.freshness.lastCheckedAt == checked)
    #expect(
      presentation.errorDescription == "P031 GraphQL read transport failed: fixture read failure")
    #expect(presentation.emptyStateTitle == nil)

    let runDetail = await coordinator.loadRunDetail(
      runID: "run-1",
      currentFreshness: P031FreshnessSnapshot(state: .live),
      checkedAt: checked
    )
    let stageDetail = await coordinator.loadStageDetail(
      stageExecutionID: "stage-exec-1",
      currentFreshness: P031FreshnessSnapshot(state: .live),
      checkedAt: checked
    )
    let stages = await coordinator.loadStages(
      runID: "run-1",
      currentFreshness: P031FreshnessSnapshot(state: .live),
      checkedAt: checked
    )
    let artifacts = await coordinator.loadArtifacts(
      runID: "run-1",
      currentFreshness: P031FreshnessSnapshot(state: .live),
      checkedAt: checked
    )
    let reports = await coordinator.loadReportMetadata(
      runID: "run-1",
      currentFreshness: P031FreshnessSnapshot(state: .live),
      checkedAt: checked
    )
    let daemon = await coordinator.loadDaemonLifecycle(
      currentFreshness: P031FreshnessSnapshot(state: .live),
      checkedAt: checked
    )

    #expect(runDetail.stageRows.isEmpty)
    #expect(runDetail.approvalRows.isEmpty)
    #expect(runDetail.artifactRows.isEmpty)
    #expect(runDetail.reportRows.isEmpty)
    #expect(runDetail.freshness.state == .unavailable)
    #expect(runDetail.errorDescription == presentation.errorDescription)
    #expect(stageDetail.stage == nil)
    #expect(stageDetail.freshness.state == .unavailable)
    #expect(stageDetail.errorDescription == presentation.errorDescription)
    #expect(stages.rows.isEmpty)
    #expect(stages.freshness.state == .unavailable)
    #expect(stages.errorDescription == presentation.errorDescription)
    #expect(artifacts.rows.isEmpty)
    #expect(artifacts.freshness.state == .unavailable)
    #expect(artifacts.errorDescription == presentation.errorDescription)
    #expect(reports.rows.isEmpty)
    #expect(reports.freshness.state == .unavailable)
    #expect(reports.errorDescription == presentation.errorDescription)
    #expect(daemon.state == nil)
    #expect(daemon.title == "Daemon unavailable")
    #expect(daemon.freshness.state == .unavailable)
    #expect(daemon.errorDescription == presentation.errorDescription)
  }
}

private struct RunsPayload: Decodable {
  let runs: [P031RunRowReadModel]
}

private func approvalCLIWritePathGuideData() -> Data {
  writePathGuideData(rows: completeWritePathGuideRows())
}

private func mcpTerminalApprovalWritePathGuideData() -> Data {
  writePathGuideData(
    rows: completeWritePathGuideRows(
      approvalRow: availableGuideRow(
        controlID: "approvals.resolve",
        label: "Approve or reject approval",
        workflowKind: "mcp_terminal",
        workflowName: "chainworks-control-plane approvals.resolve",
        identifiers: ["approval_id", "run_id", "stage_id"],
        parameterShape: "approval_id plus binary decision",
        successOutput: "approval resolved",
        notes: "Use copied identifiers from the approval diagnostic row."
      )))
}

private func partialApprovalCLIWritePathGuideData() -> Data {
  writePathGuideData(rows: Array(completeWritePathGuideRows().prefix(2)))
}

private func completeWritePathGuideRows(
  approvalRow: [String: Any] = availableGuideRow(
    controlID: "approvals.resolve",
    label: "Approve or reject approval",
    workflowKind: "CLI",
    workflowName: "chainworks approvals resolve",
    identifiers: ["approval_id", "run_id", "stage_id"],
    parameterShape: "approval_id plus binary decision",
    successOutput: "approval resolved",
    notes: "Use copied identifiers from the approval diagnostic row."
  )
) -> [[String: Any]] {
  [
    approvalRow,
    unavailableGuideRow(controlID: "runs.cancel", label: "Cancel run", identifiers: ["run_id"]),
    unavailableGuideRow(controlID: "ideas.create", label: "Create idea", identifiers: ["idea_id"]),
    unavailableGuideRow(controlID: "runs.start", label: "Start run", identifiers: ["idea_id"]),
    unavailableGuideRow(
      controlID: "stages.retry",
      label: "Retry stage",
      identifiers: ["run_id", "stage_id"]
    ),
    unavailableGuideRow(
      controlID: "steward.run_analysis",
      label: "Run steward analysis",
      identifiers: ["run_id"]
    ),
    unavailableGuideRow(
      controlID: "session.reset", label: "Reset session", identifiers: ["run_id"]),
    unavailableGuideRow(
      controlID: "session.resume", label: "Resume session", identifiers: ["run_id"]),
    unavailableGuideRow(controlID: "runs.clone", label: "Clone run", identifiers: ["run_id"]),
    unavailableGuideRow(controlID: "runs.compare", label: "Compare runs", identifiers: ["run_id"]),
    unavailableGuideRow(
      controlID: "experiments.launch",
      label: "Launch experiment",
      identifiers: ["run_id"]
    ),
    unavailableGuideRow(
      controlID: "runtime.health",
      label: "Runtime health",
      identifiers: ["run_id"]
    ),
    unavailableGuideRow(controlID: "agents.reset", label: "Reset agent", identifiers: ["run_id"]),
  ]
}

private func writePathGuideData(rows: [[String: Any]]) -> Data {
  try! JSONSerialization.data(withJSONObject: [
    "schema_version": "p031-operator-write-path-guide-v1",
    "rows": rows,
  ])
}

private func availableGuideRow(
  controlID: String,
  label: String,
  workflowKind: String,
  workflowName: String,
  identifiers: [String],
  parameterShape: String,
  successOutput: String,
  notes: String? = nil
) -> [String: Any] {
  [
    "removed_control_id": controlID,
    "removed_control_label": label,
    "external_workflow_kind": workflowKind,
    "external_workflow_name_or_tool": workflowName,
    "required_identifiers": identifiers,
    "minimum_parameter_shape": parameterShape,
    "unavailable_reason": NSNull(),
    "expected_success_output": successOutput,
    "follow_up_id": NSNull(),
    "operator_notes": notes ?? NSNull(),
    "validation_status": "validated",
  ]
}

private func unavailableGuideRow(
  controlID: String,
  label: String,
  identifiers: [String]
) -> [String: Any] {
  [
    "removed_control_id": controlID,
    "removed_control_label": label,
    "external_workflow_kind": "temporarily unavailable",
    "external_workflow_name_or_tool": NSNull(),
    "required_identifiers": identifiers,
    "minimum_parameter_shape": NSNull(),
    "unavailable_reason": "P031-FOLLOWUP-WRITE-PATH",
    "expected_success_output": NSNull(),
    "follow_up_id": "P031-FOLLOWUP-WRITE-PATH",
    "operator_notes": NSNull(),
    "validation_status": "pending",
  ]
}

private func temporaryRepositoryRoot() throws -> URL {
  let root = FileManager.default.temporaryDirectory
    .appendingPathComponent("p031-guide-bootstrap-\(UUID().uuidString)", isDirectory: true)
  try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
  return root
}

private func daemonStatusJSON(state: String) -> String {
  """
  {
    "state": "\(state)",
    "schema_version": 1,
    "binary_schema_version": 1,
    "build_sha": "test-sha",
    "started_at": "2026-04-22T00:00:00Z",
    "last_state_change_at": "2026-04-22T00:00:01Z",
    "degraded": [],
    "failure": null,
    "restart_count_since_boot": 0,
    "pid": 1234
  }
  """
}

private func daemonStatusGraphQLResponse(fieldName: String, status: String) throws -> Data {
  try JSONSerialization.data(withJSONObject: [
    "data": [
      fieldName: [
        "json": status
      ]
    ]
  ])
}

private func makeDaemonStatus(state: P031DaemonLifecycleState) -> P031DaemonStatusReadModel {
  P031DaemonStatusReadModel(
    state: state,
    schemaVersion: 1,
    binarySchemaVersion: 1,
    buildSHA: "test-sha",
    startedAt: "2026-04-22T00:00:00Z",
    lastStateChangeAt: "2026-04-22T00:00:01Z",
    restartCountSinceBoot: 0,
    pid: 1234,
    rawJSON: daemonStatusJSON(state: state.rawValue)
  )
}

private func makeStage(
  id: String,
  status: String = "running",
  hasPendingApproval: Bool = false,
  projectionLag: Bool = false,
  freshnessState: P031FreshnessState = .live
) -> P031StageReadModel {
  P031StageReadModel(
    id: id,
    runID: "run-1",
    stageID: "state_1",
    label: "Stage 1",
    status: status,
    iteration: 1,
    attemptNumber: 1,
    settlementKind: nil,
    hasArtifacts: true,
    hasPendingApproval: hasPendingApproval,
    hasValidationFailure: false,
    projectionPresent: true,
    projectionUpdatedAt: "2026-04-22T00:00:00Z",
    projectionLag: projectionLag,
    freshnessState: freshnessState
  )
}

private func makeArtifact(
  id: String,
  name: String,
  payloadAvailabilityState: P031PayloadAvailabilityState,
  payloadUnavailableReasonCode: P031PayloadUnavailableReasonCode?,
  diagnosticID: String?
) -> P031ArtifactReadModel {
  P031ArtifactReadModel(
    id: id,
    runID: "run-1",
    stageID: "state_1",
    agentID: "agent",
    name: name,
    contractID: "summary",
    format: "json",
    isPinned: false,
    reportKind: nil,
    reportVersion: nil,
    outputSettlement: nil,
    sourceGenerationVerified: true,
    freshnessState: .live,
    payloadAvailabilityState: payloadAvailabilityState,
    payloadUnavailableReasonCode: payloadUnavailableReasonCode,
    diagnosticID: diagnosticID,
    serverDebugDetail: nil
  )
}

private final class CapturingP031ReadTransport: P031GraphQLReadTransport, @unchecked Sendable {
  private let responseData: Data
  private let responses: [String: Data]
  private(set) var requests: [P031GraphQLReadRequest] = []

  init(
    responseData: Data = Data(#"{"data":{}}"#.utf8),
    responses: [String: Data] = [:]
  ) {
    self.responseData = responseData
    self.responses = responses
  }

  func send(_ request: P031GraphQLReadRequest) async throws -> Data {
    requests.append(request)
    return responses[request.operationName] ?? responseData
  }
}

private final class CapturingP031SubscriptionTransport: P031GraphQLSubscriptionTransport,
  @unchecked Sendable
{
  private let frames: [Data]
  private(set) var requests: [P031GraphQLReadRequest] = []

  init(frames: [Data] = []) {
    self.frames = frames
  }

  func subscribe(_ request: P031GraphQLReadRequest) -> AsyncThrowingStream<Data, Error> {
    requests.append(request)
    let frames = frames
    return AsyncThrowingStream { continuation in
      for frame in frames {
        continuation.yield(frame)
      }
      continuation.finish()
    }
  }
}

private struct FailingP031WorkflowReadStore: P031WorkflowReadStore {
  func fetchRuns() async throws -> [P031RunRowReadModel] {
    throw P031GraphQLReadBoundaryError.transportFailed("fixture read failure")
  }

  func fetchRunDetail(runID: String) async throws -> P031RunDetailReadModel {
    throw P031GraphQLReadBoundaryError.transportFailed("fixture read failure")
  }

  func fetchStageDetail(stageExecutionID: String) async throws -> P031StageDetailReadModel {
    throw P031GraphQLReadBoundaryError.transportFailed("fixture read failure")
  }

  func fetchStages(runID: String) async throws -> [P031StageReadModel] {
    throw P031GraphQLReadBoundaryError.transportFailed("fixture read failure")
  }

  func fetchApprovalInbox() async throws -> [P031ApprovalReadModel] {
    throw P031GraphQLReadBoundaryError.transportFailed("fixture read failure")
  }

  func fetchArtifacts(runID: String) async throws -> [P031ArtifactReadModel] {
    throw P031GraphQLReadBoundaryError.transportFailed("fixture read failure")
  }

  func fetchReportMetadata(runID: String) async throws -> [P031ReportMetadataReadModel] {
    throw P031GraphQLReadBoundaryError.transportFailed("fixture read failure")
  }

  func fetchDaemonStatus() async throws -> P031DaemonStatusReadModel {
    throw P031GraphQLReadBoundaryError.transportFailed("fixture read failure")
  }

  func subscribeToRunStatus(runID: String) throws -> AsyncThrowingStream<
    P031RunStatusChangedReadModel, Error
  > {
    throw P031GraphQLReadBoundaryError.transportFailed("fixture read failure")
  }

  func subscribeToDaemonStatus() throws -> AsyncThrowingStream<P031DaemonStatusReadModel, Error> {
    throw P031GraphQLReadBoundaryError.transportFailed("fixture read failure")
  }
}

private func firstValue<Element>(
  from stream: AsyncThrowingStream<Element, Error>
) async throws -> Element? {
  for try await element in stream {
    return element
  }
  return nil
}

private func jsonObject(from text: String) throws -> [String: Any] {
  let data = Data(text.utf8)
  return try #require(JSONSerialization.jsonObject(with: data) as? [String: Any])
}
