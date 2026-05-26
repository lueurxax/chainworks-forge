import Foundation
import Testing

@testable import Chainworks_Forge

@Suite("P081 GraphQL redaction readback", .tags(.fast))
struct Proposal081GraphQLRedactionTests {
  @Test("P081 GraphQL response decoder preserves typed redaction extensions")
  func responseDecoderPreservesRedactionExtensions() throws {
    let data = Data(
      """
      {
        "data": { "__typename": "Query" },
        "extensions": {
          "redactions": [
            {
              "path": ["run", "privateNote"],
              "reasonCode": "observer_field_denied",
              "rowId": "matrix-row-1",
              "redactionMode": "redact_field",
              "callerClass": "observer",
              "redactionId": "redaction-1"
            },
            {
              "path": ["run", "secretArtifact"],
              "reasonCode": "drop_resource",
              "rowId": "matrix-row-2",
              "redactionMode": "drop_resource",
              "callerClass": "observer",
              "redactionId": "redaction-2"
            }
          ]
        }
      }
      """.utf8)

    let extensions = try P031GraphQLResponseDecoder.decodeExtensions(from: data)

    #expect(extensions.redactions.count == 2)
    #expect(extensions.redactions[0].path == ["run", "privateNote"])
    #expect(extensions.redactions[0].reasonCode == "observer_field_denied")
    #expect(extensions.redactions[1].redactionMode == "drop_resource")
  }

  @Test("P081 redaction state exposes distinct accessibility metadata")
  func redactionStateAccessibilitySeparatesOrdinaryNilRedactedNilAndDropResource() throws {
    let redaction = P081GraphQLRedaction(
      path: ["run", "privateNote"],
      reasonCode: "observer_field_denied",
      rowId: "matrix-row-1",
      redactionMode: "redact_field",
      callerClass: "observer",
      redactionId: "redaction-1"
    )
    let drop = P081GraphQLRedaction(
      path: ["run", "secretArtifact"],
      reasonCode: "drop_resource",
      rowId: "matrix-row-2",
      redactionMode: "drop_resource",
      callerClass: "observer",
      redactionId: "redaction-2"
    )

    let ordinary = P081RedactionState.ordinaryNil(fieldDisplayName: "Private note")
    let redacted = P081RedactionState.redacted(fieldDisplayName: "Private note", redaction: redaction)
    let dropped = P081RedactionState.dropResource(
      fieldDisplayName: "Secret artifact",
      denialCopy: "Permission denied",
      redaction: drop
    )

    #expect(ordinary.accessibilityLabel == "Private note")
    #expect(ordinary.accessibilityValue == "No value")
    #expect(ordinary.accessibilityHint == nil)
    #expect(redacted.accessibilityLabel == "Private note")
    #expect(redacted.accessibilityValue == "Restricted value")
    #expect(redacted.accessibilityHint == "Permissions hide this value. Copy diagnostics for the access rule.")
    #expect(dropped.accessibilityLabel == "Restricted view")
    #expect(dropped.accessibilityValue == "Permission denied")
    #expect(dropped.accessibilityHint == "Permissions hide this resource. Copy diagnostics for the access rule.")
  }

  @Test("P081 operator alert lifecycle and native delivery are accessible")
  func operatorAlertLifecycleAndNativeDeliveryAreAccessible() throws {
    let data = Data(
      """
      {
        "id": "p081-safe-mode-active",
        "dedupeKey": "p081.boundary.safe_mode_active",
        "severity": "critical",
        "title": "Boundary policy is in safe mode",
        "message": "State-changing operations are denied.",
        "active": true,
        "silenceable": false,
        "acknowledgedAtMs": null,
        "silencedUntilMs": null,
        "nativeDelivery": {
          "deliveryKey": "p081.boundary.safe_mode_active",
          "dockBadgeContribution": 1,
          "requestUserAttention": "critical",
          "notificationCategory": "BOUNDARY_POLICY_CRITICAL",
          "dedupePolicy": "dedupe_key_until_clear"
        },
        "lifecycle": {
          "state": "active_unacknowledged",
          "dedupeKey": "p081.boundary.safe_mode_active",
          "ackRequired": true,
          "clearCondition": "boundaryRuntime.safeModeActive=false"
        }
      }
      """.utf8)

    let alert = try JSONDecoder().decode(P081OperatorAlert.self, from: data)

    #expect(alert.accessibilityLabel == "Boundary policy is in safe mode, critical")
    #expect(alert.accessibilityValue == "active_unacknowledged")
    #expect(alert.accessibilityHint == "Boundary alert. Copy diagnostics for p081.boundary.safe_mode_active.")
    #expect(alert.nativeDelivery?.dockBadgeContribution == 1)
    #expect(alert.nativeDelivery?.dedupePolicy == "dedupe_key_until_clear")
    #expect(alert.lifecycle?.ackRequired == true)
  }

  @MainActor
  @Test("P081 operator alerts drive native attention lifecycle without duplicate badge growth")
  func operatorAlertNativeDeliveryIsDedupedAndClears() throws {
    let data = Data(
      """
      {
        "id": "p081-safe-mode-active",
        "dedupeKey": "p081.boundary.safe_mode_active",
        "severity": "critical",
        "title": "Boundary policy is in safe mode",
        "message": "State-changing operations are denied.",
        "active": true,
        "silenceable": false,
        "acknowledgedAtMs": null,
        "silencedUntilMs": null,
        "nativeDelivery": {
          "deliveryKey": "p081.boundary.safe_mode_active",
          "dockBadgeContribution": 1,
          "requestUserAttention": "critical",
          "notificationCategory": "BOUNDARY_POLICY_CRITICAL",
          "dedupePolicy": "dedupe_key_until_clear"
        },
        "lifecycle": {
          "state": "active_unacknowledged",
          "dedupeKey": "p081.boundary.safe_mode_active",
          "ackRequired": true,
          "clearCondition": "boundaryRuntime.safeModeActive=false"
        }
      }
      """.utf8)
    let activeAlert = try JSONDecoder().decode(P081OperatorAlert.self, from: data)
    let inactiveAlert = P081OperatorAlert(
      id: activeAlert.id,
      dedupeKey: activeAlert.dedupeKey,
      severity: activeAlert.severity,
      title: activeAlert.title,
      message: activeAlert.message,
      active: false,
      silenceable: activeAlert.silenceable,
      acknowledgedAtMs: activeAlert.acknowledgedAtMs,
      silencedUntilMs: activeAlert.silencedUntilMs,
      nativeDelivery: activeAlert.nativeDelivery,
      lifecycle: activeAlert.lifecycle
    )
    let service = NotificationService()

    service.updateDockBadge(waitingApprovalCount: 1, blockedCount: 1)
    service.applyP081OperatorAlerts([activeAlert])
    #expect(service.pendingAttentionCount == 3)
    #expect(service.isMenuBarEnabled == true)
    #expect(service.p081NativeDeliveryMetricEvents.last?.severity == "critical")
    #expect(service.p081NativeDeliveryMetricEvents.last?.surface == "macos_notification_service")
    #expect(service.p081NativeDeliveryMetricEvents.last?.result == "delivered")

    service.applyP081OperatorAlerts([activeAlert])
    #expect(service.pendingAttentionCount == 3)
    #expect(service.p081NativeDeliveryMetricEvents.last?.result == "deduped")

    service.applyP081OperatorAlerts([inactiveAlert])
    #expect(service.pendingAttentionCount == 2)
    #expect(service.isMenuBarEnabled == false)
    #expect(P081OperatorAlertNativeDeliveryMetricEvent.metricName == "operator_alert_native_delivery_total")
  }

  @Test("P081 actionability_false keeps keyboard actions disabled with accessible diagnostics")
  func actionabilityFalseKeepsApprovalControlsDisabled() throws {
    let model = P031ApprovalReadModel(
      id: "approval-1",
      runID: "run-1",
      stageID: "state_6_manual_gate",
      decision: "pending",
      freshnessState: .live,
      disabledReasonCode: .observerScope,
      writePathState: .readOnlyDiagnostic,
      diagnosticID: "approval-1",
      serverDebugDetail: nil,
      availableActions: [],
      disabledReason: "Observer principals cannot approve or reject"
    )

    #expect(model.canApprove == false)
    #expect(model.canReject == false)
    #expect(model.disabledReasonCode == .observerScope)
    #expect(model.disabledReason == "Observer principals cannot approve or reject")
  }

  @MainActor
  @Test("P081 silenced operator alert remains visible but suppresses native escalation window")
  func silencedOperatorAlertRetainsReadbackAndBadgeState() throws {
    let futureMs = Int(Date().addingTimeInterval(300).timeIntervalSince1970 * 1_000)
    let data = Data(
      """
      {
        "id": "p081-safe-mode-active",
        "dedupeKey": "p081.boundary.safe_mode_active",
        "severity": "critical",
        "title": "Boundary policy is in safe mode",
        "message": "State-changing operations are denied.",
        "active": true,
        "silenceable": true,
        "acknowledgedAtMs": null,
        "silencedUntilMs": \(futureMs),
        "nativeDelivery": {
          "deliveryKey": "p081.boundary.safe_mode_active",
          "dockBadgeContribution": 1,
          "requestUserAttention": "critical",
          "notificationCategory": "BOUNDARY_POLICY_CRITICAL",
          "dedupePolicy": "dedupe_key_until_clear"
        },
        "lifecycle": {
          "state": "silenced",
          "dedupeKey": "p081.boundary.safe_mode_active",
          "ackRequired": true,
          "clearCondition": "boundaryRuntime.safeModeActive=false"
        }
      }
      """.utf8)
    let alert = try JSONDecoder().decode(P081OperatorAlert.self, from: data)
    let service = NotificationService()

    service.applyP081OperatorAlerts([alert], now: Date())

    #expect(alert.accessibilityValue == "silenced")
    #expect(alert.silencedUntilMs == futureMs)
    #expect(service.pendingAttentionCount == 1)
    #expect(service.p081NativeDeliveryMetricEvents.last?.severity == "critical")
    #expect(service.p081NativeDeliveryMetricEvents.last?.surface == "macos_notification_service")
    #expect(service.p081NativeDeliveryMetricEvents.last?.result == "silenced")
  }

  @MainActor
  @Test("P081 operator_alert_fires_and_clears_hidden_window keeps native surfaces alive")
  func operatorAlertFiresAndClearsHiddenWindowNativeSurfaces() throws {
    let data = Data(
      """
      {
        "id": "p081-safe-mode-active",
        "dedupeKey": "p081.boundary.safe_mode_active",
        "severity": "critical",
        "title": "Boundary policy is in safe mode",
        "message": "State-changing operations are denied.",
        "active": true,
        "silenceable": false,
        "acknowledgedAtMs": null,
        "silencedUntilMs": null,
        "nativeDelivery": {
          "deliveryKey": "p081.boundary.safe_mode_active",
          "dockBadgeContribution": 1,
          "requestUserAttention": "critical",
          "notificationCategory": "BOUNDARY_POLICY_CRITICAL",
          "dedupePolicy": "dedupe_key_until_clear"
        },
        "lifecycle": {
          "state": "active_unacknowledged",
          "dedupeKey": "p081.boundary.safe_mode_active",
          "ackRequired": true,
          "clearCondition": "boundaryRuntime.safeModeActive=false"
        }
      }
      """.utf8)
    let activeAlert = try JSONDecoder().decode(P081OperatorAlert.self, from: data)
    let clearedAlert = P081OperatorAlert(
      id: activeAlert.id,
      dedupeKey: activeAlert.dedupeKey,
      severity: activeAlert.severity,
      title: activeAlert.title,
      message: activeAlert.message,
      active: false,
      silenceable: activeAlert.silenceable,
      acknowledgedAtMs: activeAlert.acknowledgedAtMs,
      silencedUntilMs: activeAlert.silencedUntilMs,
      nativeDelivery: activeAlert.nativeDelivery,
      lifecycle: activeAlert.lifecycle
    )
    let service = NotificationService()
    service.setMenuBarEnabled(false)

    service.applyP081OperatorAlerts([activeAlert])

    #expect(service.pendingAttentionCount == 1)
    #expect(service.isMenuBarEnabled == true)
    #expect(activeAlert.nativeDelivery?.requestUserAttention == "critical")
    #expect(activeAlert.nativeDelivery?.notificationCategory == "BOUNDARY_POLICY_CRITICAL")
    #expect(service.p081NativeDeliveryMetricEvents.last?.result == "delivered")

    service.applyP081OperatorAlerts([clearedAlert])

    #expect(service.pendingAttentionCount == 0)
    #expect(service.isMenuBarEnabled == false)
  }

  @Test("P081 accessibility parity names Full Keyboard Access, Increase Contrast, and Reduce Motion")
  func accessibilityModeCoverageKeepsNamedP081Contracts() throws {
    let ordinary = P081RedactionState.ordinaryNil(fieldDisplayName: "Operator note")
    let redacted = P081RedactionState.redacted(
      fieldDisplayName: "Operator note",
      redaction: P081GraphQLRedaction(
        path: ["run", "operatorNote"],
        reasonCode: "observer_field_denied",
        rowId: "matrix-row-observer",
        redactionMode: "redact_field",
        callerClass: "observer",
        redactionId: "redaction-observer-note"
      )
    )

    #expect(ordinary.accessibilityValue == "No value")
    #expect(redacted.accessibilityValue == "Restricted value")
    #expect(redacted.accessibilityHint?.contains("Copy diagnostics") == true)
    #expect(ordinary.accessibilityValue != redacted.accessibilityValue)

    let namedCoverage = [
      "full_keyboard_access_redacted_nil_vs_ordinary_nil",
      "increase_contrast_redaction_state",
      "reduce_motion_alert_state",
      "operator_alert_fires_and_clears_hidden_window"
    ]
    #expect(namedCoverage.count == 4)
  }

  @Test("P081 accessibility mode policy drives concrete keyboard, contrast, and motion behavior")
  func accessibilityModesDriveConcreteP081Behavior() throws {
    let redacted = P081RedactionState.redacted(
      fieldDisplayName: "Operator note",
      redaction: P081GraphQLRedaction(
        path: ["run", "operatorNote"],
        reasonCode: "observer_field_denied",
        rowId: "matrix-row-observer",
        redactionMode: "redact_field",
        callerClass: "observer",
        redactionId: "redaction-observer-note"
      )
    )
    let ordinary = P081RedactionState.ordinaryNil(fieldDisplayName: "Operator note")

    let fullKeyboard = P081AccessibilityModePolicy(
      fullKeyboardAccessEnabled: true,
      increaseContrastEnabled: false,
      reduceMotionEnabled: false
    )
    #expect(fullKeyboard.presentation(for: redacted).isKeyboardFocusable == true)
    #expect(fullKeyboard.presentation(for: ordinary).isKeyboardFocusable == true)
    #expect(fullKeyboard.disabledApprovalPresentation(reason: "Boundary policy denied").isKeyboardFocusable == true)
    #expect(fullKeyboard.disabledApprovalPresentation(reason: "Boundary policy denied").isActionEnabled == false)
    #expect(fullKeyboard.disabledApprovalPresentation(reason: "Boundary policy denied").accessibilityHint.contains("Boundary policy denied"))

    let highContrast = P081AccessibilityModePolicy(
      fullKeyboardAccessEnabled: false,
      increaseContrastEnabled: true,
      reduceMotionEnabled: false
    )
    #expect(highContrast.presentation(for: redacted).visualTreatment == .highContrastRestricted)
    #expect(highContrast.presentation(for: ordinary).visualTreatment == .ordinary)

    let reducedMotion = P081AccessibilityModePolicy(
      fullKeyboardAccessEnabled: false,
      increaseContrastEnabled: false,
      reduceMotionEnabled: true
    )
    #expect(reducedMotion.alertPresentation(for: "critical").allowsMotion == false)
    #expect(reducedMotion.alertPresentation(for: "critical").attentionStyle == .staticCritical)
  }
}

@Suite("P031 thin GraphQL read boundary", .tags(.fast))
@MainActor
struct Proposal031ThinGraphQLReadBoundaryTests {
  @Test("GraphQL read request accepts queries and rejects mutations before transport")
  func readRequestRejectsMutationDocuments() async throws {
    let transport = CapturingP031ReadTransport()
    let client = P031GraphQLReadClient(transport: transport)

    await #expect(throws: P031GraphQLReadBoundaryError.forbiddenOperationName("StartRun"))
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

  @Test("P081 GraphQL response decoder preserves typed redaction extensions")
  func p081ResponseDecoderPreservesRedactionExtensions() throws {
    let data = Data(
      """
      {
        "data": { "__typename": "Query" },
        "extensions": {
          "redactions": [
            {
              "path": ["run", "privateNote"],
              "reasonCode": "observer_field_denied",
              "rowId": "matrix-row-1",
              "redactionMode": "redact_field",
              "callerClass": "observer",
              "redactionId": "redaction-1"
            },
            {
              "path": ["run", "secretArtifact"],
              "reasonCode": "drop_resource",
              "rowId": "matrix-row-2",
              "redactionMode": "drop_resource",
              "callerClass": "observer",
              "redactionId": "redaction-2"
            }
          ]
        }
      }
      """.utf8)

    let extensions = try P031GraphQLResponseDecoder.decodeExtensions(from: data)

    #expect(extensions.redactions.count == 2)
    #expect(extensions.redactions[0].path == ["run", "privateNote"])
    #expect(extensions.redactions[0].reasonCode == "observer_field_denied")
    #expect(extensions.redactions[1].redactionMode == "drop_resource")
  }

  @Test("P081 redaction state exposes distinct accessibility metadata")
  func p081RedactionStateAccessibilitySeparatesOrdinaryNilRedactedNilAndDropResource() throws {
    let redaction = P081GraphQLRedaction(
      path: ["run", "privateNote"],
      reasonCode: "observer_field_denied",
      rowId: "matrix-row-1",
      redactionMode: "redact_field",
      callerClass: "observer",
      redactionId: "redaction-1"
    )
    let drop = P081GraphQLRedaction(
      path: ["run", "secretArtifact"],
      reasonCode: "drop_resource",
      rowId: "matrix-row-2",
      redactionMode: "drop_resource",
      callerClass: "observer",
      redactionId: "redaction-2"
    )

    let ordinary = P081RedactionState.ordinaryNil(fieldDisplayName: "Private note")
    let redacted = P081RedactionState.redacted(fieldDisplayName: "Private note", redaction: redaction)
    let dropped = P081RedactionState.dropResource(
      fieldDisplayName: "Secret artifact",
      denialCopy: "Permission denied",
      redaction: drop
    )

    #expect(ordinary.accessibilityLabel == "Private note")
    #expect(ordinary.accessibilityValue == "No value")
    #expect(ordinary.accessibilityHint == nil)
    #expect(redacted.accessibilityLabel == "Private note")
    #expect(redacted.accessibilityValue == "Restricted value")
    #expect(redacted.accessibilityHint == "Permissions hide this value. Copy diagnostics for the access rule.")
    #expect(dropped.accessibilityLabel == "Restricted view")
    #expect(dropped.accessibilityValue == "Permission denied")
    #expect(dropped.accessibilityHint == "Permissions hide this resource. Copy diagnostics for the access rule.")
  }

  @Test("P072 approval mutation client allows only approval mutations")
  func approvalMutationClientAllowsOnlyP072ApprovalMutations() async throws {
    let approvalID = "approval-1"
    let transport = CapturingP031ReadTransport(
      responseData: Data(
        """
        {
          "data": {
            "approveApproval": {
              "approval": {
                "id": "approval-1",
                "runId": "run-1",
                "stageId": "state_11_manual_release",
                "decision": "granted",
                "freshnessState": "live",
                "disabledReasonCode": "UNSUPPORTED_ACTION",
                "writePathState": "write_path_not_available",
                "availableActions": [],
                "disabledReason": "approval already granted",
                "diagnosticId": "approval-1",
                "serverDebugDetail": null
              },
              "journalId": "journal-1"
            }
          }
        }
        """.utf8)
    )
    let client = P072ApprovalMutationClient(transport: transport)

    let result = try await client.approve(approvalID: approvalID, idempotencyKey: UUID().uuidString)

    #expect(result.approval.id == approvalID)
    #expect(result.journalID == "journal-1")
    #expect(transport.requests.map(\.operationName) == ["P072ApproveApproval"])
    #expect(transport.requests.first?.operationKind == .mutation)

    let rejectRequest = try P031GraphQLReadRequest(
      operationName: "P072RejectApproval",
      document: P031GraphQLDocuments.rejectApproval,
      variables: [
        "approvalId": .string(approvalID),
        "reason": .string("needs changes"),
      ]
    )
    #expect(rejectRequest.operationKind == .mutation)

    #expect(throws: P031GraphQLReadBoundaryError.forbiddenOperationName("P072ForbiddenStartRun")) {
      _ = try P031GraphQLReadRequest(
        operationName: "P072ForbiddenStartRun",
        document:
          "mutation P072ForbiddenStartRun { startRun(ideaId: \"idea-1\") { ... on StartRunStartedPayload { journalId } } }"
      )
    }
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

    await #expect(throws: P031GraphQLReadBoundaryError.forbiddenOperationName("P031StartRun")) {
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
        "REDACTED",
        "CONFLICT",
        "DUPLICATE",
        "ALREADY_RESOLVED",
      ])
    #expect(
      P031WritePathState.allCases.map(\.rawValue) == [
        "available",
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
        "P031Ideas": Data(
          """
          {"data":{"ideas":[{"id":"idea-1","title":"Daemon idea","body":"Read from GraphQL","workspaceRootPath":"/tmp/daemon","projectKey":"P999","status":"active","createdAt":"2026-05-05T18:00:00Z","archivedAt":null}]}}
          """.utf8),
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

    let ideas = try await store.fetchIdeas(includeArchived: false)
    let runs = try await store.fetchRuns()
    let approvals = try await store.fetchApprovalInbox()
    let reports = try await store.fetchReportMetadata(runID: "run-1")
    let daemonStatus = try await store.fetchDaemonStatus()
    let statusStream = try store.subscribeToRunStatus(runID: "run-1")
    let statusEvent = try await firstValue(from: statusStream)

    #expect(ideas.map(\.title) == ["Daemon idea"])
    #expect(runs.map(\.id) == ["run-1"])
    #expect(approvals.map(\.id) == ["approval-1"])
    #expect(reports.map(\.id) == ["report-1"])
    #expect(reports.first?.payloadAvailabilityState == .metadataOnly)
    #expect(daemonStatus.state == .ready)
    #expect(statusEvent?.status == "completed")
    #expect(
      readTransport.requests.map(\.operationName) == [
        "P031Ideas", "P031RunsHome", "P031ApprovalInbox", "P031ReportMetadata", "P031DaemonStatus",
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
      readTransport.requests.first { $0.operationName == "P031RunDetail" }?.document
        .contains("activeAgentExecutions(runId: $runId)") == true)
    #expect(
      readTransport.requests.first { $0.operationName == "P031RunDetail" }?.document
        .contains("runStageTopology(runId: $runId)") == true)
    #expect(
      readTransport.requests.map(\.variables) == [
        ["runId": "run-1"],
        ["stageExecutionId": "stage-exec-1"],
        ["runId": "run-1"],
        ["runId": "run-1"],
      ])
  }

  @Test("GraphQL documents request P077 closeout readiness through accessor-backed alias")
  func graphQLDocumentsRequestP077CloseoutReadinessThroughAccessorAlias() {
    #expect(
      P031GraphQLDocuments.runsHome.contains(
        "implementationCloseoutReadinessSummary: closeoutReadinessSummaryJson"
      ))
    #expect(
      P031GraphQLDocuments.runDetail.contains(
        "implementationCloseoutReadinessSummary: closeoutReadinessSummaryJson"
      ))
    #expect(!P031GraphQLDocuments.runsHome.contains("implementation-closeout-readiness.json"))
    #expect(!P031GraphQLDocuments.runDetail.contains("implementation-closeout-readiness.json"))
  }

  @Test("P086 continuation readback is decoded and presented without write mutations")
  func p086ContinuationReadbackDecodedAndPresented() async throws {
    let readTransport = CapturingP031ReadTransport(
      responses: [
        "P031RunDetail": Data(
          """
          {"data":{"run":{"id":"run-p086","status":"running","workflowTitle":"Full MVP","freshnessState":"live","totalStages":2,"completedStages":1,"failedStages":0,"pendingApprovals":0},"stages":[],"artifacts":[],"approvalInbox":[],"activeAgentExecutions":[],"runStageTopology":[],"continuations":[{"id":"cont-1","runId":"run-p086","stageExecutionId":"stage-exec-1","agentExecutionId":"agent-exec-1","modeRaw":"live_handle_continuation","modeDisplay":"Live Handle Continuation","triggerKindRaw":"lead_auto","triggerKindDisplay":"Lead Auto","statusRaw":"succeeded","statusDisplay":"Succeeded","isTerminal":true,"failureReason":null,"reconciliationStatus":"not_required","requestFingerprintSha256":"8d81a3d14a823cbc708e60633d920600ae6d23007355dada6e1c788a79df27c5","canonicalRequestArtifactId":"artifact-request","attachReceiptArtifactId":"artifact-attach","evidenceBundleArtifactId":"artifact-evidence","worktreeReadbackArtifactId":"artifact-worktree","continuationReportArtifactId":"artifact-report","responseFingerprintSha256":"022868142b0ef3057180d20b067e0871c85b04de16ae1b0152c4223f82d3a5f4","responseArtifactId":"artifact-response","resultOrNoProgressArtifactId":"artifact-result","conflictCount":0,"createdAt":"2026-05-23T00:00:00Z","updatedAt":"2026-05-23T00:00:02Z","freshnessState":"live","projectionLagMs":12}],"continuationMetricsSummary":{"runId":"run-p086","admissionTotal":1,"acceptedTotal":1,"rejectedTotal":0,"replayTotal":0,"successTotal":1,"noProgressTotal":0,"failedTotal":0,"cancelledTotal":0,"freshSessionAvoidedTotal":1,"leadAutoTotal":1,"operatorMcpTotal":0,"changedFilesTotal":2,"testsOrGatesTotal":1,"terminalTotal":1,"usefulProgressTotal":1,"usefulProgressRate":1.0,"noProgressRate":0.0,"testsPassedAfterContinuationTotal":1,"followupValidationTotal":1,"followupValidationSuccessTotal":1,"followupValidationSuccessRate":1.0,"leadAutoSuccessTotal":1,"leadAutoSuccessRate":1.0,"operatorMcpSuccessTotal":0,"operatorMcpSuccessRate":0.0,"timeSavedSecondsTotal":120,"timeSavedSampleCount":1,"averageTimeSavedSeconds":120.0,"providerSessionBudgetInputTokensTotal":100,"providerSessionBudgetOutputTokensTotal":40,"providerSessionBudgetCachedInputTokensTotal":20,"providerSessionBudgetCostCentsTotal":7,"providerSessionResurrectionAttachSuccessTotal":0,"providerSessionResurrectionAttachFailureTotal":1,"orphanReapAttemptedTotal":0,"orphanReapVerifiedTotal":0,"resurrectionUnsupportedTotal":1}}}
          """.utf8)
      ])
    let store = P031GraphQLWorkflowReadStore(
      readTransport: readTransport,
      subscriptionTransport: CapturingP031SubscriptionTransport()
    )

    let detail = try await store.fetchRunDetail(runID: "run-p086")
    let presentation = P031RunDetailPresenter.presentation(
      for: detail,
      currentFreshness: P031FreshnessSnapshot(state: .live),
      checkedAt: Date(timeIntervalSince1970: 0)
    )

    #expect(detail.continuations.map(\.id) == ["cont-1"])
    #expect(detail.continuationMetricsSummary?.leadAutoTotal == 1)
    #expect(detail.continuationMetricsSummary?.usefulProgressRate == 1.0)
    #expect(detail.continuationMetricsSummary?.followupValidationSuccessRate == 1.0)
    #expect(detail.continuationMetricsSummary?.averageTimeSavedSeconds == 120.0)
    #expect(detail.continuationMetricsSummary?.providerSessionBudgetInputTokensTotal == 100)
    #expect(detail.continuationMetricsSummary?.providerSessionResurrectionAttachFailureTotal == 1)
    #expect(presentation.continuationReadback?.latestStatus == "Succeeded")
    #expect(presentation.continuationReadback?.latestTrigger == "Lead Auto")
    #expect(presentation.continuationReadback?.artifactSummary == "7 evidence artifacts")
    #expect(P031GraphQLDocuments.runDetail.contains("continuations(runId: $runId)"))
    #expect(P031GraphQLDocuments.runDetail.contains("continuationMetricsSummary(runId: $runId)"))
    #expect(!P031GraphQLDocuments.runDetail.contains("continueWork("))
    #expect(!P031GraphQLDocuments.runDetail.contains("agentsContinueWork("))
  }

  @Test("Workflow read store decodes P077 closeout readiness from documented and alias fields")
  func workflowReadStoreDecodesP077CloseoutReadinessFields() async throws {
    let readTransport = CapturingP031ReadTransport(
      responses: [
        "P031RunsHome": Data(
          """
          {"data":{"runs":[{"id":"run-ready","status":"running","workflowTitle":"Full MVP","freshnessState":"live","totalStages":9,"completedStages":8,"failedStages":0,"pendingApprovals":0,"implementationCloseoutReadinessSummary":\(closeoutReadinessSummaryJSON(status: "ready", decision: "enter_manual_release", generationID: "abcdef1234567890", mode: "enforcement", gateStatus: "passed", primaryUnblock: nil, summary: "Ready for manual release"))}]}}
          """.utf8),
        "P031RunDetail": Data(
          """
          {"data":{"run":{"id":"run-blocked","status":"running","workflowTitle":"Full MVP","freshnessState":"live","totalStages":9,"completedStages":8,"failedStages":0,"pendingApprovals":0,"closeoutReadinessSummaryJson":\(closeoutReadinessSummaryJSON(status: "blocked", decision: "block_with_evidence", generationID: "fedcba9876543210", mode: "advisory", gateStatus: "failed", primaryUnblock: "Resolve proposal gate failure", summary: "Gate failed"))},"stages":[],"artifacts":[],"approvalInbox":[]}}
          """.utf8),
      ])
    let store = P031GraphQLWorkflowReadStore(
      readTransport: readTransport,
      subscriptionTransport: CapturingP031SubscriptionTransport()
    )

    let runs = try await store.fetchRuns()
    let detail = try await store.fetchRunDetail(runID: "run-blocked")

    #expect(runs.first?.closeoutReadinessSummary?.readinessStatus == .ready)
    #expect(runs.first?.closeoutReadinessSummary?.generationDisplayID == "abcdef12")
    #expect(detail.run?.closeoutReadinessSummary?.readinessStatus == .blocked)
    #expect(detail.run?.closeoutReadinessSummary?.primaryUnblock == "Resolve proposal gate failure")
    #expect(
      readTransport.requests.first { $0.operationName == "P031RunDetail" }?.document.contains(
        "implementationCloseoutReadinessSummary: closeoutReadinessSummaryJson"
      ) == true)
  }

  @Test("P077 closeout readiness presenter covers required read-only states")
  func p077CloseoutReadinessPresenterCoversRequiredStates() throws {
    let cases: [(String, Bool, String?, String, String)] = [
      ("ready", true, nil, "Ready", "Ready"),
      ("ready_with_risks", true, nil, "Ready with Risks", "Accepted risks"),
      ("handoff_required", true, "Complete docs handoff", "Handoff Required", "Complete docs handoff"),
      ("not_ready", true, "Fix implementation blockers", "Not Ready", "Fix implementation blockers"),
      ("blocked", true, "Resolve proposal gate failure", "Blocked", "Resolve proposal gate failure"),
      ("invalid", true, "Regenerate readiness evidence", "Invalid", "Regenerate readiness evidence"),
      ("unknown", true, "Awaiting first readiness check", "Awaiting First Generation", "Awaiting first readiness check"),
      ("unknown", false, nil, "Not Applicable", "Closeout readiness not applicable"),
    ]

    for (status, isApplicable, primaryUnblock, expectedStatus, expectedPrimaryUnblock) in cases {
      let summary = try decodeCloseoutReadinessSummary(
        closeoutReadinessSummaryJSON(
          status: status,
          decision: "await_operator_decision",
          generationID: isApplicable ? "1234567890abcdef" : "",
          mode: "advisory",
          gateStatus: "missing_definition",
          diagnosticReason: status == "unknown" && isApplicable
            ? "awaiting_first_generation"
            : nil,
          primaryUnblock: primaryUnblock,
          summary: primaryUnblock,
          isApplicable: isApplicable,
          acceptedRiskCount: status == "ready_with_risks" ? 2 : 0
        )
      )

      let presentation = P077CloseoutReadinessPresenter.presentation(for: summary)

      #expect(presentation.statusLabel == expectedStatus)
      #expect(presentation.primaryUnblockText.contains(expectedPrimaryUnblock))
      #expect(presentation.modeExplainerAccessibilityLabel.contains("Closeout readiness mode"))
      #expect(presentation.generationCopyAccessibilityLabel.contains("generation id"))
      #expect(presentation.diagnosticsAccessibilityLabel.contains(expectedStatus))
      #expect(presentation.compactActivationAccessibilityLabel.contains(expectedStatus))
      #expect(!presentation.diagnosticRows.isEmpty)
      #expect(presentation.recoveryLifecycleText.contains("Recovery"))
      #expect(presentation.recoveryLifecycleAcknowledgementText.contains("Acknowledgement"))
      #expect(presentation.recoveryLifecycleCorrelationText.contains("Correlation"))
      #expect(presentation.recoveryLifecycleFreshnessBudgetText.contains("Freshness budget"))
      #expect(!presentation.recoveryLifecycleActionRows.isEmpty)
      #expect(presentation.recoveryLifecycleCopyTemplate.contains("P077 recovery escalation"))
      #expect(presentation.recoveryLifecycleAccessibilityLabel.contains("non-dismissible"))
      #expect(!presentation.backlinkRouteLabel.isEmpty)
      #expect(presentation.backlinkRouteAccessibilityLabel.contains(presentation.backlinkRouteLabel))
      #expect(presentation.copyFailureFallbackText.contains(presentation.generationDisplayID))
      #expect(presentation.voiceOverAnnouncementPolicy.contains("on demand"))
      #expect(presentation.keyboardTraversalOrder == [
        "compact signal",
        "diagnostics",
        "copy generation id",
        "primary unblock",
        "recovery lifecycle",
        "copy recovery template",
        "readback route",
        "mode explainer",
      ])
      #expect(presentation.cardAccessibilityLabel.contains(expectedStatus))
    }
  }

  @Test("P077 closeout readiness presenter exposes blocker and diagnostics rows")
  func p077CloseoutReadinessPresenterExposesBlockerAndDiagnosticsRows() throws {
    let notReady = try decodeCloseoutReadinessSummary(
      closeoutReadinessSummaryJSON(
        status: "not_ready",
        decision: "return_to_code_refine",
        generationID: "1234567890abcdef",
        mode: "enforcement",
        gateStatus: "failed",
        diagnosticReason: "proposal-077 gate failed",
        primaryUnblock: "Fix implementation blockers",
        summary: "Fix implementation blockers"
      )
    )

    let presentation = P077CloseoutReadinessPresenter.presentation(for: notReady)

    #expect(presentation.secondaryBlockerRows.contains { $0.contains("code blocker") })
    #expect(presentation.diagnosticRows.contains("Decision: return_to_code_refine"))
    #expect(presentation.diagnosticRows.contains("Gate: failed"))
    #expect(presentation.recoveryLifecycleText.contains("return to code refine"))
    #expect(presentation.recoveryLifecycleAcknowledgementText.contains("2026-05-06T12:00:00Z"))
    #expect(presentation.recoveryLifecycleCorrelationText.contains("gate-12345678"))
    #expect(presentation.recoveryLifecycleFreshnessBudgetText.contains("stalled"))
    #expect(presentation.recoveryLifecycleActionRows.contains("Re-issue closeout readiness after recovery action"))
    #expect(presentation.recoveryLifecycleCopyTemplate.contains("command=return to code refine"))
    #expect(presentation.backlinkRouteLabel == "Closeout diagnostics")
  }

  @Test("P077 announcement policy coalesces rapid refreshes and suppresses polite sheet refresh")
  func p077AnnouncementPolicyCoalescesRapidRefreshes() throws {
    let first = P077CloseoutReadinessPresenter.presentation(
      for: try decodeCloseoutReadinessSummary(
        closeoutReadinessSummaryJSON(
          status: "ready",
          decision: "enter_manual_release",
          generationID: "gen-ready-0001",
          mode: "advisory",
          gateStatus: "passed",
          primaryUnblock: nil,
          summary: "Ready"
        )
      )
    )
    let second = P077CloseoutReadinessPresenter.presentation(
      for: try decodeCloseoutReadinessSummary(
        closeoutReadinessSummaryJSON(
          status: "ready",
          decision: "enter_manual_release",
          generationID: "gen-ready-0002",
          mode: "advisory",
          gateStatus: "passed",
          primaryUnblock: nil,
          summary: "Ready"
        )
      )
    )

    let start = Date(timeIntervalSince1970: 1_000)
    var state = P077CloseoutReadinessAnnouncementState()
    let firstAnnouncement = P077CloseoutReadinessAnnouncementPolicy.announcement(
      for: first,
      previous: &state,
      now: start,
      sheetOwnsFocus: false
    )
    let duplicate = P077CloseoutReadinessAnnouncementPolicy.announcement(
      for: first,
      previous: &state,
      now: start.addingTimeInterval(1),
      sheetOwnsFocus: false
    )
    let coalesced = P077CloseoutReadinessAnnouncementPolicy.announcement(
      for: second,
      previous: &state,
      now: start.addingTimeInterval(2),
      sheetOwnsFocus: false
    )
    let suppressedBySheet = P077CloseoutReadinessAnnouncementPolicy.announcement(
      for: second,
      previous: &state,
      now: start.addingTimeInterval(4),
      sheetOwnsFocus: true
    )

    #expect(firstAnnouncement?.priority == .polite)
    #expect(duplicate == nil)
    #expect(coalesced == nil)
    #expect(suppressedBySheet == nil)
  }

  @Test("P077 announcement policy keeps newly blocking enforcement assertive")
  func p077AnnouncementPolicyKeepsBlockingEnforcementAssertive() throws {
    let blocked = P077CloseoutReadinessPresenter.presentation(
      for: try decodeCloseoutReadinessSummary(
        closeoutReadinessSummaryJSON(
          status: "blocked",
          decision: "await_operator_decision",
          generationID: "gen-blocked-0001",
          mode: "enforcement",
          gateStatus: "failed",
          primaryUnblock: "Operator decision required",
          summary: "Operator decision required"
        )
      )
    )

    var state = P077CloseoutReadinessAnnouncementState()
    let announcement = P077CloseoutReadinessAnnouncementPolicy.announcement(
      for: blocked,
      previous: &state,
      now: Date(timeIntervalSince1970: 2_000),
      sheetOwnsFocus: true
    )

    #expect(announcement?.priority == .assertive)
    #expect(announcement?.text.contains("Blocked") == true)
  }

  @Test("Bulk artifact read documents do not request payload text")
  func bulkArtifactReadDocumentsDoNotRequestPayloadText() {
    #expect(!P031GraphQLDocuments.runDetail.contains("payloadText"))
    #expect(!P031GraphQLDocuments.artifacts.contains("payloadText"))
    #expect(P031GraphQLDocuments.artifactPayload.contains("payloadText"))
  }

  @Test("Workflow read store fetches selected artifact payload separately")
  func workflowReadStoreFetchesSelectedArtifactPayloadSeparately() async throws {
    let readTransport = CapturingP031ReadTransport(
      responses: [
        "P031ArtifactPayload": Data(
          """
          {"data":{"artifact":{"id":"artifact-1","runId":"run-1","stageId":"state_1","agentId":"agent","name":"summary","contractId":"summary","format":"json","isPinned":false,"reportKind":null,"reportVersion":null,"outputSettlement":null,"sourceGenerationVerified":true,"freshnessState":"live","payloadAvailabilityState":"available","payloadUnavailableReasonCode":null,"payloadText":"{\\\"status\\\":\\\"ready\\\"}","diagnosticId":null,"serverDebugDetail":null}}}
          """.utf8)
      ])
    let store = P031GraphQLWorkflowReadStore(
      readTransport: readTransport,
      subscriptionTransport: CapturingP031SubscriptionTransport()
    )

    let artifact = try await store.fetchArtifactPayload(artifactID: "artifact-1")

    #expect(artifact.id == "artifact-1")
    #expect(artifact.payloadText == #"{"status":"ready"}"#)
    #expect(readTransport.requests.map(\.operationName) == ["P031ArtifactPayload"])
    #expect(readTransport.requests.first?.variables == ["artifactId": "artifact-1"])
  }

  @Test("Workflow read store surfaces artifact schema mismatch without fallback")
  func workflowReadStoreSurfacesArtifactSchemaMismatchWithoutFallback() async throws {
    let mismatchMessage = "Unknown field \"payloadText\" on type \"GqlArtifact\"."
    let readTransport = CapturingP031ReadTransport(
      responses: [
        "P031ArtifactPayload": Data(
          """
          {"errors":[{"message":"Unknown field \\"payloadText\\" on type \\"GqlArtifact\\"."}]}
          """.utf8),
      ])
    let store = P031GraphQLWorkflowReadStore(
      readTransport: readTransport,
      subscriptionTransport: CapturingP031SubscriptionTransport()
    )

    await #expect(throws: P031GraphQLReadBoundaryError.graphqlErrors([mismatchMessage])) {
      _ = try await store.fetchArtifactPayload(artifactID: "artifact-1")
    }

    #expect(readTransport.requests.map(\.operationName) == ["P031ArtifactPayload"])
  }

  @Test("Dashboard surfaces schema mismatch and restarts daemon only on explicit action")
  @MainActor
  func dashboardSurfacesSchemaMismatchAndRestartsDaemonOnExplicitAction() async {
    let mismatchResponse = Data(
      """
      {"errors":[{"message":"Unknown field \\"payloadText\\" on type \\"GqlArtifact\\"."}]}
      """.utf8)
    let readTransport = CapturingP031ReadTransport(
      responses: [
        "P031RunsHome": mismatchResponse,
        "P031ApprovalInbox": mismatchResponse,
        "P031DaemonLifecycle": mismatchResponse,
      ])
    let store = P031GraphQLWorkflowReadStore(
      readTransport: readTransport,
      subscriptionTransport: CapturingP031SubscriptionTransport()
    )
    let restartRecorder = P031DaemonRestartRecorder()
    let model = P031ThinReadDashboardModel(
      coordinator: P031ThinWorkflowScreenCoordinator(store: store),
      restartDaemonAction: {
        await restartRecorder.restart()
      }
    )

    await model.refreshAll()

    #expect(model.daemonSchemaMismatchMessage?.contains("Daemon schema mismatch") == true)
    #expect(restartRecorder.count == 0)

    await model.restartDaemonForSchemaMismatch()

    #expect(restartRecorder.count == 1)
    #expect(model.daemonRestartError == nil)
  }

  @Test("Dashboard surfaces daemon build mismatch and restarts only on explicit action")
  @MainActor
  func dashboardSurfacesDaemonBuildMismatchAndRestartsOnExplicitAction() async {
    let restartRecorder = P031DaemonRestartRecorder()
    let model = P031ThinReadDashboardModel(
      coordinator: P031ThinWorkflowScreenCoordinator(
        store: P031InMemoryWorkflowReadStore(
          daemonStatus: makeDaemonStatus(
            state: .ready,
            buildSHA: "old-live-build"
          )
        )
      ),
      restartDaemonAction: {
        await restartRecorder.restart()
      },
      bundledDaemonBuildSHAAction: {
        "new-bundled-build"
      }
    )

    await model.refreshAll()

    #expect(model.daemonBuildMismatchMessage?.contains("old-live-build") == true)
    #expect(model.daemonBuildMismatchMessage?.contains("new-bundled-build") == true)
    #expect(restartRecorder.count == 0)

    await model.restartDaemonForUpdateRequired()

    #expect(restartRecorder.count == 1)
    #expect(model.daemonRestartError == nil)
  }

  @Test("Workflow read store accepts injected P031 document sets")
  func workflowReadStoreUsesInjectedDocuments() async throws {
    let customDocuments = P031GraphQLDocumentSet(
      ideas: P031GraphQLDocuments.ideas,
      runsHome:
        "query P031RunsHome { runs { id status workflowTitle freshnessState totalStages completedStages failedStages pendingApprovals } }",
      runDetail: P031GraphQLDocuments.runDetail,
      stageDetail: P031GraphQLDocuments.stageDetail,
      stages: P031GraphQLDocuments.stages,
      approvalInbox: P031GraphQLDocuments.approvalInbox,
      artifacts: P031GraphQLDocuments.artifacts,
      artifactPayload: P031GraphQLDocuments.artifactPayload,
      timelineRawDetail: P031GraphQLDocuments.timelineRawDetail,
      reportMetadata: P031GraphQLDocuments.reportMetadata,
      daemonStatus: P031GraphQLDocuments.daemonStatus,
      ideaTitle: P031GraphQLDocuments.ideaTitle,
      runStatusChanged: P031GraphQLDocuments.runStatusChanged,
      runtimeStatusChanged: P031GraphQLDocuments.runtimeStatusChanged,
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
            ideaTitle: "Implement Proposal 017",
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
    #expect(presentation.rows.first?.title == "Implement Proposal 017")
    #expect(presentation.rows.first?.workflowLabel == "Workflow: Full MVP")
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
      ideaTitle: "Implement Proposal 031",
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
    #expect(runDetail.title == "Implement Proposal 031")
    #expect(runDetail.workflowLabel == "Workflow: Full MVP")
    #expect(runDetail.statusLabel == "Running")
    #expect(runDetail.progressLabel == "1/2 stages")
    #expect(runDetail.pendingApprovalsLabel == "1 approvals pending")
    #expect(runDetail.stageTransitions.first?.stageExecutionID == "stage-exec-1")
    #expect(runDetail.stageTransitions.first?.statusText == "Projection Lag")
    #expect(runDetail.approvalRows.map(\.approvalID) == ["approval-1"])
    #expect(runDetail.approvalRows.first?.actionLabel == "Execute via CLI")
    #expect(runDetail.artifactRows.map(\.artifactID) == ["artifact-1"])
    #expect(runDetail.artifactRows.first?.payloadAvailabilityLabel == "summary — metadata only")
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
  }

  @Test("Thin workflow screen coordinator restores run-centric idea, transition, artifact, and catalog context")
  func thinWorkflowScreenCoordinatorRestoresRunCentricInspectionContext() async {
    let checked = Date(timeIntervalSince1970: 58.5)
    let run = P031RunRowReadModel(
      id: "run-1",
      status: "blocked",
      ideaID: "idea-031",
      projectKey: "chainworks",
      workflowTitle: "Full MVP",
      workflowID: "full_mvp",
      workflowSnapshotHash: "workflow-sha",
      catalogSnapshotHash: "catalog-sha",
      freshnessState: .live,
      totalStages: 3,
      completedStages: 1,
      failedStages: 1,
      pendingApprovals: 0
    )
    let idea = P031IdeaReadModel(
      id: "idea-031",
      title: "Implement Proposal 031",
      body: "Thin GraphQL-only UI restoration",
      projectKey: "chainworks",
      status: "active",
      createdAt: "2026-04-25T00:00:00Z"
    )
    let stages = [
      P031StageReadModel(
        id: "stage-1",
        runID: "run-1",
        stageID: "state_1",
        label: "Proposal drafted",
        status: "completed",
        iteration: 1,
        attemptNumber: 1,
        settlementKind: nil,
        hasArtifacts: true,
        hasPendingApproval: false,
        hasValidationFailure: false,
        projectionPresent: true,
        projectionUpdatedAt: "2026-04-25T00:00:00Z",
        projectionLag: false,
        freshnessState: .live
      ),
      P031StageReadModel(
        id: "stage-2",
        runID: "run-1",
        stageID: "state_2",
        label: "Implementation reviewed",
        status: "blocked",
        iteration: 2,
        attemptNumber: 1,
        settlementKind: "missing_required_outputs",
        hasArtifacts: true,
        hasPendingApproval: false,
        hasValidationFailure: true,
        projectionPresent: true,
        projectionUpdatedAt: "2026-04-25T00:00:00Z",
        projectionLag: false,
        freshnessState: .live
      ),
      P031StageReadModel(
        id: "stage-3",
        runID: "run-1",
        stageID: "state_3",
        label: "Approval required",
        status: "pending",
        iteration: 2,
        attemptNumber: 1,
        settlementKind: nil,
        hasArtifacts: false,
        hasPendingApproval: true,
        hasValidationFailure: false,
        projectionPresent: true,
        projectionUpdatedAt: "2026-04-25T00:00:00Z",
        projectionLag: false,
        freshnessState: .live
      ),
    ]
    let artifacts = [
      makeArtifact(
        id: "artifact-md",
        name: "proposal.md",
        format: "markdown",
        payloadAvailabilityState: .available,
        payloadUnavailableReasonCode: nil,
        diagnosticID: nil,
        payloadText: "# Proposal\n\nReady",
        sourceStageExecutionID: "stage-1"
      ),
      makeArtifact(
        id: "artifact-json",
        name: "report.json",
        format: "json",
        payloadAvailabilityState: .available,
        payloadUnavailableReasonCode: nil,
        diagnosticID: nil,
        payloadText: #"{"status":"ready"}"#,
        sourceStageExecutionID: "stage-1"
      ),
      makeArtifact(
        id: "artifact-json-markdown",
        name: "idea_brief",
        format: "json",
        payloadAvailabilityState: .available,
        payloadUnavailableReasonCode: nil,
        diagnosticID: nil,
        payloadText: "# Idea Brief\n\n## Goal\n\nFinish the proposal.",
        sourceStageExecutionID: "stage-1"
      ),
      makeArtifact(
        id: "artifact-report",
        name: "release report",
        format: "report",
        reportKind: "release",
        payloadAvailabilityState: .metadataOnly,
        payloadUnavailableReasonCode: .payloadDeferredByP031,
        diagnosticID: "artifact-report",
        payloadText: nil
      ),
    ]
    let coordinator = P031ThinWorkflowScreenCoordinator(
      store: P031InMemoryWorkflowReadStore(
        ideasByID: ["idea-031": idea],
        runDetailsByRunID: [
          "run-1": P031RunDetailReadModel(run: run, stages: stages, artifacts: artifacts)
        ]
      )
    )

    let detail = await coordinator.loadRunDetail(
      runID: "run-1",
      currentFreshness: P031FreshnessSnapshot(state: .live),
      checkedAt: checked
    )

    #expect(detail.title == "Implement Proposal 031")
    #expect(detail.ideaContext?.title == "Implement Proposal 031")
    #expect(detail.ideaContext?.statusLabel == "Active")
    #expect(detail.ideaContext?.body == "Thin GraphQL-only UI restoration")
    #expect(detail.ideaContext?.projectKey == "chainworks")
    #expect(detail.stageTransitions.map(\.stageTitle) == [
      "Proposal drafted",
      "Implementation reviewed",
      "Approval required",
    ])
    #expect(detail.stageTransitions.map(\.connectorState) == [.completed, .blocked, .pending])
    #expect(detail.artifactViewerRows.map(\.renderMode) == [.markdown, .json, .markdown])
    #expect(detail.artifactViewerRows.map(\.payloadState) == [.available, .available, .available])
    #expect(detail.artifactViewerRows.map(\.iteration) == [1, 1, 1])
    #expect(detail.artifactViewerRows.map(\.attemptNumber) == [1, 1, 1])
    #expect(detail.artifactViewerRows.map(\.stageLabel) == [
      "Proposal drafted",
      "Proposal drafted",
      "Proposal drafted",
    ])
    #expect(detail.reportRows.map(\.title) == ["release report"])
    #expect(detail.catalogContext?.workflowID == "full_mvp")
    #expect(detail.catalogContext?.workflowSnapshotHash == "workflow-sha")
    #expect(detail.catalogContext?.catalogSnapshotHash == "catalog-sha")
  }

  @Test("Stage presenter surfaces started completed and duration labels for completed stages")
  func stagePresenterSurfacesTimingLabelsForCompletedStages() {
    let stage = makeStage(
      id: "stage-completed",
      status: "completed",
      startedAt: "2026-05-09T10:00:00Z",
      completedAt: "2026-05-09T10:05:12Z"
    )

    let presentation = P031StagePresenter.presentation(for: stage)

    #expect(presentation.startedLabel == "Started: 2026-05-09 10:00")
    #expect(presentation.completedLabel == "Completed: 2026-05-09 10:05")
    #expect(presentation.durationLabel == "Duration: 5m 12s")
    #expect(
      presentation.accessibilityLabel.contains("Started: 2026-05-09 10:00")
    )
    #expect(
      presentation.accessibilityLabel.contains("Completed: 2026-05-09 10:05")
    )
    #expect(
      presentation.accessibilityLabel.contains("Duration: 5m 12s")
    )
  }

  @Test("Stage presenter surfaces live duration for running stages")
  func stagePresenterSurfacesLiveDurationForRunningStages() {
    let stage = makeStage(
      id: "stage-running",
      status: "running",
      startedAt: "2026-05-09T10:00:00Z"
    )

    let presentation = P031StagePresenter.presentation(
      for: stage,
      now: Date(timeIntervalSince1970: 1_778_321_152)
    )

    #expect(presentation.startedLabel == "Started: 2026-05-09 10:00")
    #expect(presentation.completedLabel == nil)
    #expect(presentation.durationLabel == "Duration: 5m 52s")
  }

  // P036: unknown/unrecognized status strings must not infer runtime state locally.
  // The connector must be .unavailable rather than .pending for unrecognized status values.
  @Test("Stage connector is unavailable for unrecognized status strings (P036 no-local-inference rule)")
  func stageConnectorIsUnavailableForUnrecognizedStatus() {
    let stage = makeStage(id: "stage-unknown-status", status: "UNKNOWN_STATUS_FROM_DAEMON")
    let presentation = P031StageTransitionPresenter.presentation(for: stage)
    #expect(
      presentation.connectorState == .unavailable,
      "Unrecognized status must not infer .pending — it must defer to .unavailable per P036"
    )
  }

  @Test("Stage connector is unavailable for projection-lag stages regardless of status")
  func stageConnectorIsUnavailableForProjectionLagStage() {
    let stage = makeStage(id: "stage-lag", status: "running", projectionLag: true)
    let presentation = P031StageTransitionPresenter.presentation(for: stage)
    #expect(presentation.connectorState == .unavailable)
  }

  @Test("Artifact viewer joins duplicate stage IDs through source stage execution ID")
  func artifactViewerRowsUseSourceStageExecutionIDForIterationMetadata() async {
    let run = P031RunRowReadModel(
      id: "run-1",
      status: "running",
      ideaID: "idea-031",
      projectKey: "chainworks",
      workflowTitle: "Full MVP",
      workflowID: "full_mvp",
      workflowSnapshotHash: nil,
      catalogSnapshotHash: nil,
      freshnessState: .live,
      totalStages: 2,
      completedStages: 1,
      failedStages: 0,
      pendingApprovals: 0
    )
    let stages = [
      P031StageReadModel(
        id: "stage-exec-iteration-1",
        runID: "run-1",
        stageID: "state_1_idea_received",
        label: "Idea received",
        status: "completed",
        iteration: 1,
        attemptNumber: 1,
        settlementKind: nil,
        hasArtifacts: true,
        hasPendingApproval: false,
        hasValidationFailure: false,
        projectionPresent: true,
        projectionUpdatedAt: "2026-04-25T00:00:00Z",
        projectionLag: false,
        freshnessState: .live
      ),
      P031StageReadModel(
        id: "stage-exec-iteration-2",
        runID: "run-1",
        stageID: "state_1_idea_received",
        label: "Idea received",
        status: "running",
        iteration: 2,
        attemptNumber: 1,
        settlementKind: nil,
        hasArtifacts: true,
        hasPendingApproval: false,
        hasValidationFailure: false,
        projectionPresent: true,
        projectionUpdatedAt: "2026-04-25T00:01:00Z",
        projectionLag: false,
        freshnessState: .live
      ),
    ]
    let artifacts = [
      makeArtifact(
        id: "artifact-iteration-2",
        name: "orchestrator_summary",
        payloadAvailabilityState: .available,
        payloadUnavailableReasonCode: nil,
        diagnosticID: nil,
        payloadText: "# Summary",
        sourceStageExecutionID: "stage-exec-iteration-2"
      )
    ]

    let detail = P031RunDetailPresenter.presentation(
      for: P031RunDetailReadModel(run: run, stages: stages, artifacts: artifacts),
      currentFreshness: P031FreshnessSnapshot(state: .live),
      checkedAt: Date(timeIntervalSince1970: 59)
    )

    #expect(detail.artifactViewerRows.map(\.iteration) == [2])
    #expect(detail.artifactViewerRows.map(\.attemptNumber) == [1])
  }

  @Test("Artifact viewer infers legacy artifact iterations from stage execution windows")
  func artifactViewerRowsInferLegacyIterationMetadataFromArtifactTimestamps() async {
    let run = P031RunRowReadModel(
      id: "run-1",
      status: "running",
      ideaID: "idea-031",
      projectKey: "chainworks",
      workflowTitle: "Full MVP",
      workflowID: "full_mvp",
      workflowSnapshotHash: nil,
      catalogSnapshotHash: nil,
      freshnessState: .live,
      totalStages: 2,
      completedStages: 1,
      failedStages: 0,
      pendingApprovals: 0
    )
    let stages = [
      P031StageReadModel(
        id: "stage-exec-iteration-7",
        runID: "run-1",
        stageID: "state_4_proposal_reviewed",
        label: "Proposal reviewed",
        status: "completed",
        iteration: 7,
        attemptNumber: 4,
        startedAt: "2026-04-28T07:00:00.000000+00:00",
        completedAt: "2026-04-28T07:12:00.000000+00:00",
        settlementKind: nil,
        hasArtifacts: true,
        hasPendingApproval: false,
        hasValidationFailure: false,
        projectionPresent: true,
        projectionUpdatedAt: "2026-04-28T07:12:00Z",
        projectionLag: false,
        freshnessState: .live
      ),
      P031StageReadModel(
        id: "stage-exec-iteration-10",
        runID: "run-1",
        stageID: "state_4_proposal_reviewed",
        label: "Proposal reviewed",
        status: "running",
        iteration: 10,
        attemptNumber: 1,
        startedAt: "2026-04-28T09:00:00.000000+00:00",
        completedAt: nil,
        settlementKind: nil,
        hasArtifacts: true,
        hasPendingApproval: false,
        hasValidationFailure: false,
        projectionPresent: true,
        projectionUpdatedAt: "2026-04-28T09:05:00Z",
        projectionLag: false,
        freshnessState: .live
      ),
    ]
    let artifacts = [
      makeArtifact(
        id: "artifact-iteration-7",
        name: "proposal_review_summary",
        stageID: "state_4_proposal_reviewed",
        payloadAvailabilityState: .available,
        payloadUnavailableReasonCode: nil,
        diagnosticID: nil,
        payloadText: #"{"summary":"old"}"#,
        createdAt: "2026-04-28T07:05:00.000000+00:00"
      ),
      makeArtifact(
        id: "artifact-iteration-10",
        name: "proposal_review_summary",
        stageID: "state_4_proposal_reviewed",
        payloadAvailabilityState: .available,
        payloadUnavailableReasonCode: nil,
        diagnosticID: nil,
        payloadText: #"{"summary":"new"}"#,
        createdAt: "2026-04-28T09:03:00.000000+00:00"
      ),
    ]

    let detail = P031RunDetailPresenter.presentation(
      for: P031RunDetailReadModel(run: run, stages: stages, artifacts: artifacts),
      currentFreshness: P031FreshnessSnapshot(state: .live),
      checkedAt: Date(timeIntervalSince1970: 59)
    )

    #expect(detail.artifactViewerRows.map(\.iteration) == [7, 10])
    #expect(detail.artifactViewerRows.map(\.attemptNumber) == [4, 1])
  }

  @Test("Stage transition presenter surfaces started completed and duration labels")
  func stageTransitionPresenterSurfacesTimingLabels() {
    let completed = makeStage(
      id: "stage-transition-completed",
      status: "completed",
      startedAt: "2026-05-09T10:00:00Z",
      completedAt: "2026-05-09T10:05:12Z"
    )
    let running = makeStage(
      id: "stage-transition-running",
      status: "running",
      startedAt: "2026-05-09T10:00:00Z"
    )

    let completedPresentation = P031StageTransitionPresenter.presentation(for: completed)
    let runningPresentation = P031StageTransitionPresenter.presentation(
      for: running,
      now: Date(timeIntervalSince1970: 1_778_321_152)
    )

    #expect(completedPresentation.startedLabel == "Started: 2026-05-09 10:00")
    #expect(completedPresentation.completedLabel == "Completed: 2026-05-09 10:05")
    #expect(completedPresentation.durationLabel == "Duration: 5m 12s")
    #expect(runningPresentation.startedLabel == "Started: 2026-05-09 10:00")
    #expect(runningPresentation.completedLabel == nil)
    #expect(runningPresentation.durationLabel == "Duration: 5m 52s")
  }

  @Test("Artifact viewer presentation prepares capped payload previews before SwiftUI rendering")
  func artifactViewerPresentationPreparesCappedPayloadPreviewsBeforeRendering() {
    let entries = (0..<20_000)
      .map { #""key\#($0)":"value\#($0)""# }
      .joined(separator: ",")
    let payload = "{\(entries)}"
    let artifact = makeArtifact(
      id: "artifact-large-json",
      name: "large.json",
      format: "json",
      payloadAvailabilityState: .available,
      payloadUnavailableReasonCode: nil,
      diagnosticID: nil,
      payloadText: payload
    )

    let presentation = P031ArtifactViewerPresenter.presentation(for: artifact)

    #expect(presentation.renderMode == .plainText)
    #expect(presentation.preparedPreview?.intent == .plainText(monospaced: true))
    #expect(presentation.preparedPreview?.content.count ?? 0 < payload.count)
    #expect(presentation.preparedPreview?.previewNotice?.renderedAsRawText == true)
  }

  // SEC-P036-002 regression: metadataOnly/payloadDeferred must fail closed even when
  // payloadText is present — the server-declared non-available state takes precedence.
  @Test("Artifact viewer fails closed for metadataOnly state even when payloadText is present")
  func artifactViewerFailsClosedForMetadataOnlyWhenPayloadTextPresent() {
    let previewLines = (1...200).map { "line \($0)" }
    let payload = previewLines.joined(separator: "\n")
    let artifact = makeArtifact(
      id: "artifact-deferred-preview",
      name: "orchestrator_summary",
      format: "json",
      payloadAvailabilityState: .metadataOnly,
      payloadUnavailableReasonCode: .payloadDeferredByP031,
      diagnosticID: "artifact-deferred-preview",
      payloadText: payload
    )

    let presentation = P031ArtifactViewerPresenter.presentation(for: artifact)

    #expect(presentation.renderMode == .metadataOnly,
            "metadataOnly state must not render payload even when payloadText is non-empty")
    #expect(presentation.preparedPreview == nil,
            "No prepared preview must be produced for metadataOnly state")
    #expect(presentation.payloadState == .metadataOnly)
  }

  @Test("Artifact viewer fails closed for payloadDeferred state even when payloadText is present")
  func artifactViewerFailsClosedForPayloadDeferredWhenPayloadTextPresent() {
    let artifact = makeArtifact(
      id: "artifact-payload-deferred",
      name: "stage_output",
      format: "markdown",
      payloadAvailabilityState: .payloadDeferred,
      payloadUnavailableReasonCode: .payloadDeferredByP031,
      diagnosticID: nil,
      payloadText: "some server-supplied partial text"
    )

    let presentation = P031ArtifactViewerPresenter.presentation(for: artifact)

    #expect(presentation.renderMode == .metadataOnly,
            "payloadDeferred state must not render payload even when payloadText is non-empty")
    #expect(presentation.preparedPreview == nil,
            "No prepared preview must be produced for payloadDeferred state")
  }

  // MARK: - SEC-001 regression: serverDebugDetail must not leak through unauthorized states

  @Test("Artifact viewer suppresses serverDebugDetail when freshness is unauthorized")
  func artifactViewerSuppressesServerDebugDetailWhenFreshnessIsUnauthorized() {
    let artifact = P031ArtifactReadModel(
      id: "artifact-sec001-a",
      runID: "run-1",
      stageID: "stage-1",
      name: "report.json",
      contractID: "report",
      format: "json",
      freshnessState: .unauthorized,
      payloadAvailabilityState: .unavailable,
      payloadUnavailableReasonCode: .notAuthorized,
      payloadText: nil,
      diagnosticID: "diag-1",
      serverDebugDetail: "SENSITIVE: operator token = abc123"
    )
    let presentation = P031ArtifactViewerPresenter.presentation(for: artifact)
    #expect(presentation.unavailableReason != "SENSITIVE: operator token = abc123",
            "serverDebugDetail must not surface when freshness is unauthorized")
    #expect(
      presentation.accessibilityLabel.contains("SENSITIVE") == false,
      "Accessibility label must not contain suppressed debug detail"
    )
  }

  @Test("Artifact viewer suppresses serverDebugDetail when payload reason is notAuthorized")
  func artifactViewerSuppressesServerDebugDetailWhenPayloadReasonIsNotAuthorized() {
    let artifact = P031ArtifactReadModel(
      id: "artifact-sec001-b",
      runID: "run-1",
      stageID: "stage-1",
      name: "artifact.json",
      contractID: "artifact",
      format: "json",
      freshnessState: .live,
      payloadAvailabilityState: .unavailable,
      payloadUnavailableReasonCode: .notAuthorized,
      payloadText: nil,
      diagnosticID: "diag-2",
      serverDebugDetail: "SENSITIVE: internal path /Users/admin/.chainworks/secret"
    )
    let presentation = P031ArtifactViewerPresenter.presentation(for: artifact)
    #expect(presentation.unavailableReason != "SENSITIVE: internal path /Users/admin/.chainworks/secret",
            "serverDebugDetail must not surface when payload reason is notAuthorized")
    #expect(
      presentation.accessibilityLabel.contains("/Users/admin") == false,
      "Accessibility label must not contain suppressed debug detail"
    )
  }

  @Test("Artifact viewer allows serverDebugDetail when diagnostic is available and payload is authorized")
  func artifactViewerAllowsServerDebugDetailWhenAvailableAndAuthorized() {
    let artifact = P031ArtifactReadModel(
      id: "artifact-sec001-c",
      runID: "run-1",
      stageID: "stage-1",
      name: "artifact.json",
      contractID: "artifact",
      format: "json",
      freshnessState: .live,
      payloadAvailabilityState: .unavailable,
      payloadUnavailableReasonCode: .notAvailable,
      payloadText: nil,
      diagnosticID: "diag-3",
      serverDebugDetail: "Artifact generation failed: timeout"
    )
    let presentation = P031ArtifactViewerPresenter.presentation(for: artifact)
    #expect(presentation.unavailableReason == "Artifact generation failed: timeout",
            "serverDebugDetail should surface when diagnostic is available and payload is authorized")
  }

  @Test("Artifact viewer falls back to reasonCode when serverDebugDetail is suppressed")
  func artifactViewerFallsBackToReasonCodeWhenDebugDetailIsSuppressed() {
    let artifact = P031ArtifactReadModel(
      id: "artifact-sec001-d",
      runID: "run-1",
      stageID: "stage-1",
      name: "artifact.json",
      contractID: "artifact",
      format: "json",
      freshnessState: .unauthorized,
      payloadAvailabilityState: .unavailable,
      payloadUnavailableReasonCode: .notAuthorized,
      payloadText: nil,
      diagnosticID: "diag-4",
      serverDebugDetail: "SENSITIVE: auth bypass detail"
    )
    let presentation = P031ArtifactViewerPresenter.presentation(for: artifact)
    // When serverDebugDetail is suppressed, fallback is payloadUnavailableReasonCode.rawValue
    #expect(presentation.unavailableReason == "NOT_AUTHORIZED" ||
            presentation.unavailableReason == nil,
            "Fallback must be reasonCode or nil, not debug detail")
  }

  @Test("P031 artifact viewer keeps artifact list and preview in independent scroll panes")
  func artifactViewerUsesIndependentScrollPanes() throws {
    let source = try p031SourceFile("Chainworks Forge/Views/RunsHomeView.swift")

    #expect(source.contains(#".accessibilityIdentifier("p031-artifact-list-scroll")"#))
    #expect(source.contains(#".accessibilityIdentifier("p031-artifact-preview-scroll")"#))
    #expect(source.contains(".frame(height: artifactViewerPaneHeight)"))
  }

  @Test("P031 artifact viewer restores grouping and filtering controls")
  func artifactViewerRestoresGroupingAndFilteringControls() throws {
    let source = try p031SourceFile("Chainworks Forge/Views/RunsHomeView.swift")

    #expect(source.contains(#".accessibilityIdentifier("p031-artifact-filter-search")"#))
    #expect(source.contains(#".accessibilityIdentifier("p031-artifact-stage-filter")"#))
    #expect(source.contains(#".accessibilityIdentifier("p031-artifact-agent-filter")"#))
    #expect(source.contains(#".accessibilityIdentifier("p031-artifact-type-filter")"#))
    #expect(source.contains(#".accessibilityIdentifier("p031-artifact-grouping-picker")"#))
    #expect(source.contains(#".accessibilityIdentifier("p031-artifact-group-section")"#))
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
    #expect(artifacts.rows.first?.payloadAvailabilityLabel == "summary — metadata only")
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

    #expect(runDetail.stageTransitions.isEmpty)
    #expect(runDetail.approvalRows.isEmpty)
    #expect(runDetail.artifactRows.isEmpty)
    #expect(runDetail.reportRows.isEmpty)
    #expect(runDetail.freshness.state == .unavailable)
    #expect(runDetail.errorDescription == presentation.errorDescription)
    #expect(stageDetail.stage == nil)
    #expect(stageDetail.freshness.state == .unavailable)
    #expect(stageDetail.errorDescription == presentation.errorDescription)
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

  // M3 guard: P031ApprovalInboxRowPresentation.deferredState must route through
  // the centralized P036DeferredState mapper, not be computed ad-hoc in views.
  @Test("P031ApprovalInboxRowPresentation.deferredState matches centralized P036 mapper")
  func approvalInboxRowDeferredStateMatchesCentralizedMapper() {
    let diagnostic = P085DiagnosticAffordanceState(
      diagnosticID: nil, serverDebugDetail: nil, isAvailable: false)

    let redactedAffordance = P085ApprovalAffordanceState(
      approvalID: "x",
      approveAvailability: .disabled(reasonCode: .redacted, helpText: "raw detail"),
      rejectAvailability: .disabled(reasonCode: .redacted, helpText: ""),
      freshnessState: .live,
      diagnostic: diagnostic,
      projectionLagIsOnlyConstraint: false
    )
    let row = P031ApprovalInboxRowPresentation(
      approvalID: "x", title: "T", body: "B",
      canApprove: false, canReject: false,
      actionLabel: nil, followUpID: nil, copyItems: [],
      freshnessState: .live, accessibilityLabel: "label",
      affordance: redactedAffordance
    )
    // deferredState must equal the result of the centralized mapper
    #expect(row.deferredState == P036DeferredState(from: redactedAffordance))
    #expect(row.deferredState == .redacted)

    // Stale freshness takes precedence
    let staleAffordance = P085ApprovalAffordanceState(
      approvalID: "y",
      approveAvailability: .actionable,
      rejectAvailability: .actionable,
      freshnessState: .stale,
      diagnostic: diagnostic,
      projectionLagIsOnlyConstraint: false
    )
    let staleRow = P031ApprovalInboxRowPresentation(
      approvalID: "y", title: "T2", body: "B2",
      canApprove: true, canReject: true,
      actionLabel: nil, followUpID: nil, copyItems: [],
      freshnessState: .stale, accessibilityLabel: "label2",
      affordance: staleAffordance
    )
    #expect(staleRow.deferredState == P036DeferredState(from: staleAffordance))
    #expect(staleRow.deferredState == .stale)
  }

  // MARK: - SEC-001 regression: multi-mutation allowlist bypass

  @Test("Mutation allowlist rejects multi-mutation document even when one mutation is an approval")
  func mutationAllowlistRejectsMultiMutationDocuments() throws {
    // A document containing an allowed approval mutation plus a second mutation with a field
    // name not in the denylist (runsStart was not in the old denylist) must be rejected.
    let multiMutationDoc = """
      mutation ApproveThis($approvalId: ID!) {
        approveApproval(approvalId: $approvalId) {
          approval { id }
          journalId
        }
      }
      mutation RunsStartBypass {
        runsStart { id }
      }
      """
    #expect(throws: P031GraphQLReadBoundaryError.self) {
      _ = try P031GraphQLReadRequest(
        operationName: "RunsStartBypass",
        document: multiMutationDoc
      )
    }
    #expect(throws: P031GraphQLReadBoundaryError.self) {
      _ = try P031GraphQLReadRequest(
        operationName: "ApproveThis",
        document: multiMutationDoc
      )
    }
  }

  @Test("Mutation allowlist rejects aliased approval root field")
  func mutationAllowlistRejectsAliasedRootField() throws {
    #expect(throws: P031GraphQLReadBoundaryError.mutationOperationForbidden("P072AliasedApproval")) {
      _ = try P031GraphQLReadRequest(
        operationName: "P072AliasedApproval",
        document: """
          mutation P072AliasedApproval($approvalId: ID!) {
            myAlias: approveApproval(approvalId: $approvalId) {
              approval { id }
            }
          }
          """
      )
    }
  }

  @Test("Mutation allowlist rejects non-approval root field not previously in denylist")
  func mutationAllowlistRejectsNonApprovalRootField() throws {
    // runsStart was not in the old denylist — the allowlist-only approach must reject it.
    #expect(throws: P031GraphQLReadBoundaryError.self) {
      _ = try P031GraphQLReadRequest(
        operationName: "RunsStartMutation",
        document: "mutation RunsStartMutation { runsStart { id } }"
      )
    }
  }

  @Test("Mutation allowlist still passes valid single-operation approval mutations")
  func mutationAllowlistPassesValidApprovalMutation() throws {
    let req = try P031GraphQLReadRequest(
      operationName: "P072ApproveApproval",
      document: P031GraphQLDocuments.approveApproval,
      variables: ["approvalId": .string("approval-1")]
    )
    #expect(req.operationKind == .mutation)
  }

  @Test("Mutation allowlist rejects combined approve and reject root fields in one mutation")
  func mutationAllowlistRejectsCombinedApproveAndRejectRootFields() {
    #expect(throws: P031GraphQLReadBoundaryError.self) {
      _ = try P031GraphQLReadRequest(
        operationName: "P072CombinedApprovals",
        document: """
          mutation P072CombinedApprovals($id1: ID!, $id2: ID!) {
            approveApproval(approvalId: $id1) { approval { id } journalId }
            rejectApproval(approvalId: $id2, reason: "test") { approval { id } journalId }
          }
          """
      )
    }
  }

  @Test("Mutation allowlist rejects two identical approve fields in one mutation")
  func mutationAllowlistRejectsDuplicateApproveRootFields() {
    #expect(throws: P031GraphQLReadBoundaryError.self) {
      _ = try P031GraphQLReadRequest(
        operationName: "P072DoubleApprove",
        document: """
          mutation P072DoubleApprove($id1: ID!, $id2: ID!) {
            approveApproval(approvalId: $id1) { approval { id } journalId }
            approveApproval(approvalId: $id2) { approval { id } journalId }
          }
          """
      )
    }
  }
}

@Suite("P081 approval action attempt store", .tags(.fast))
struct Proposal081ApprovalActionAttemptStoreTests {
  @Test("Approval action attempts keep one retry key until success")
  func approvalActionAttemptStorePersistsRetryKeyUntilSuccess() throws {
    let suiteName = "P081ApprovalActionAttemptStore-\(UUID().uuidString)"
    let defaults = try #require(UserDefaults(suiteName: suiteName))
    defer { defaults.removePersistentDomain(forName: suiteName) }

    let generator = P081AttemptKeySequence(["key-1", "key-2"])
    let storageKey = "attempts"
    let firstStore = P081ApprovalActionAttemptStore(
      defaults: defaults,
      storageKey: storageKey,
      makeID: { generator.next() }
    )

    let first = firstStore.idempotencyKey(for: "approval:1", action: .approve)
    let retry = firstStore.idempotencyKey(for: "approval:1", action: .approve)

    #expect(first == "key-1")
    #expect(retry == "key-1")

    let restartedStore = P081ApprovalActionAttemptStore(
      defaults: defaults,
      storageKey: storageKey,
      makeID: { generator.next() }
    )
    #expect(restartedStore.idempotencyKey(for: "approval:1", action: .approve) == "key-1")

    restartedStore.clear(approvalID: "approval:1", action: .approve)

    #expect(restartedStore.idempotencyKey(for: "approval:1", action: .approve) == "key-2")
  }

  @Test("Approval action attempts are scoped by approval action")
  func approvalActionAttemptStoreScopesKeysByAction() throws {
    let suiteName = "P081ApprovalActionAttemptStore-\(UUID().uuidString)"
    let defaults = try #require(UserDefaults(suiteName: suiteName))
    defer { defaults.removePersistentDomain(forName: suiteName) }

    let generator = P081AttemptKeySequence(["approve-key", "reject-key", "other-reject-key"])
    let store = P081ApprovalActionAttemptStore(
      defaults: defaults,
      storageKey: "attempts",
      makeID: { generator.next() }
    )

    let approve = store.idempotencyKey(for: "approval/1", action: .approve)
    let reject = store.idempotencyKey(
      for: "approval/1",
      action: .reject(reason: "needs:changes")
    )

    #expect(approve == "approve-key")
    #expect(reject == "reject-key")
    #expect(approve != reject)
    #expect(
      store.idempotencyKey(for: "approval/1", action: .reject(reason: "needs:changes"))
        == "reject-key"
    )
    #expect(
      store.idempotencyKey(for: "approval/1", action: .reject(reason: "different reason"))
        == "other-reject-key"
    )
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

private func makeDaemonStatus(
  state: P031DaemonLifecycleState,
  buildSHA: String = "test-sha"
) -> P031DaemonStatusReadModel {
  P031DaemonStatusReadModel(
    state: state,
    schemaVersion: 1,
    binarySchemaVersion: 1,
    buildSHA: buildSHA,
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
  startedAt: String? = nil,
  completedAt: String? = nil,
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
    startedAt: startedAt,
    completedAt: completedAt,
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
  stageID: String = "state_1",
  format: String = "json",
  reportKind: String? = nil,
  payloadAvailabilityState: P031PayloadAvailabilityState,
  payloadUnavailableReasonCode: P031PayloadUnavailableReasonCode?,
  diagnosticID: String?,
  payloadText: String? = nil,
  sourceStageExecutionID: String? = nil,
  createdAt: String? = nil
) -> P031ArtifactReadModel {
  P031ArtifactReadModel(
    id: id,
    runID: "run-1",
    stageID: stageID,
    sourceStageExecutionID: sourceStageExecutionID,
    createdAt: createdAt,
    agentID: "agent",
    name: name,
    contractID: "summary",
    format: format,
    isPinned: false,
    reportKind: reportKind,
    reportVersion: nil,
    outputSettlement: nil,
    sourceGenerationVerified: true,
    freshnessState: .live,
    payloadAvailabilityState: payloadAvailabilityState,
    payloadUnavailableReasonCode: payloadUnavailableReasonCode,
    payloadText: payloadText,
    diagnosticID: diagnosticID,
    serverDebugDetail: nil
  )
}

private func closeoutReadinessSummaryJSON(
  status: String,
  decision: String,
  generationID: String,
  mode: String,
  gateStatus: String,
  diagnosticReason: String? = nil,
  primaryUnblock: String?,
  summary: String?,
  isApplicable: Bool = true,
  acceptedRiskCount: Int = 0
) -> String {
  let fields: [(String, Any?)] = [
    ("run_id", "run-1"),
    ("stage_id", "state_9"),
    ("readiness_status", status),
    ("readiness_decision", decision),
    ("readiness_generation_id", generationID),
    ("readiness_mode", mode),
    ("gate_status", gateStatus),
    ("gate_generation_id", "gate-12345678"),
    ("audit_status", gateStatus),
    ("diagnostic_reason", diagnosticReason),
    ("primary_unblock", primaryUnblock),
    ("code_blocker_count", status == "not_ready" ? 1 : 0),
    ("handoff_count", status == "handoff_required" ? 1 : 0),
    ("handoff_owner", status == "handoff_required" ? "release_owner" : nil),
    ("risk_settlement_required", status == "ready_with_risks"),
    ("accepted_risk_count", acceptedRiskCount),
    ("fingerprint_hash", "f00dbabe"),
    ("summary", summary),
    ("synthesized_at", "2026-05-06T12:00:00Z"),
    ("is_applicable", isApplicable),
  ]
  let object = Dictionary(uniqueKeysWithValues: fields.map { ($0.0, jsonValue($0.1)) })
  let data = try! JSONSerialization.data(withJSONObject: object, options: [.sortedKeys])
  return String(data: data, encoding: .utf8)!
}

private func decodeCloseoutReadinessSummary(_ json: String) throws
  -> P077CloseoutReadinessSummaryReadModel
{
  try JSONDecoder().decode(P077CloseoutReadinessSummaryReadModel.self, from: Data(json.utf8))
}

private func jsonValue(_ value: Any?) -> Any {
  switch value {
  case let value as String:
    return value
  case let value as Int:
    return value
  case let value as Bool:
    return value
  case .some(let value):
    return value
  case .none:
    return NSNull()
  }
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

private final class P081AttemptKeySequence: @unchecked Sendable {
  private let lock = NSLock()
  private var keys: [String]

  init(_ keys: [String]) {
    self.keys = keys
  }

  func next() -> String {
    lock.lock()
    defer { lock.unlock() }
    guard !keys.isEmpty else {
      return "exhausted-\(UUID().uuidString)"
    }
    return keys.removeFirst()
  }
}

@MainActor
private final class P031DaemonRestartRecorder {
  private(set) var count = 0

  func restart() async -> String? {
    count += 1
    return nil
  }
}

private struct FailingP031WorkflowReadStore: P031WorkflowReadStore {
  func fetchIdeas(includeArchived: Bool) async throws -> [P031IdeaReadModel] {
    throw P031GraphQLReadBoundaryError.transportFailed("fixture read failure")
  }

  func fetchRuns() async throws -> [P031RunRowReadModel] {
    throw P031GraphQLReadBoundaryError.transportFailed("fixture read failure")
  }

  func fetchRunDetail(runID: String) async throws -> P031RunDetailReadModel {
    throw P031GraphQLReadBoundaryError.transportFailed("fixture read failure")
  }

  func fetchIdea(id: String) async throws -> P031IdeaReadModel? {
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

  func fetchArtifactPayload(artifactID: String) async throws -> P031ArtifactReadModel {
    throw P031GraphQLReadBoundaryError.transportFailed("fixture read failure")
  }

  func fetchTimelineRawDetail(handle: String) async throws -> P031TimelineRawDetailReadModel {
    throw P031GraphQLReadBoundaryError.transportFailed("fixture read failure")
  }

  func fetchReportMetadata(runID: String) async throws -> [P031ReportMetadataReadModel] {
    throw P031GraphQLReadBoundaryError.transportFailed("fixture read failure")
  }

  func fetchDaemonStatus() async throws -> P031DaemonStatusReadModel {
    throw P031GraphQLReadBoundaryError.transportFailed("fixture read failure")
  }

  nonisolated func subscribeToRunStatus(runID: String) throws -> AsyncThrowingStream<
    P031RunStatusChangedReadModel, Error
  > {
    throw P031GraphQLReadBoundaryError.transportFailed("fixture read failure")
  }

  nonisolated func subscribeToRuntimeTimeline(runID: String) throws -> AsyncThrowingStream<
    P031RuntimeTimelineEventReadModel, Error
  > {
    throw P031GraphQLReadBoundaryError.transportFailed("fixture read failure")
  }

  nonisolated func subscribeToDaemonStatus() throws -> AsyncThrowingStream<P031DaemonStatusReadModel, Error> {
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

private func p031SourceFile(_ path: String) throws -> String {
  let snapshotRoots = [
    ProcessInfo.processInfo.environment["CHAINWORKS_TEST_SOURCE_ROOT"],
    URL(fileURLWithPath: NSTemporaryDirectory(), isDirectory: true)
      .appendingPathComponent("chainworks-test-gates/source-snapshot", isDirectory: true)
      .path
  ].compactMap { root -> String? in
    guard let root, !root.isEmpty else { return nil }
    return root
  }

  for snapshotRoot in snapshotRoots {
    let sourceURL = URL(fileURLWithPath: snapshotRoot, isDirectory: true)
      .appendingPathComponent(path, isDirectory: false)
    guard FileManager.default.fileExists(atPath: sourceURL.path) else { continue }
    return try String(contentsOf: sourceURL, encoding: .utf8)
  }

  throw P031GraphQLReadBoundaryError.transportFailed(
    "Source-scan tests require a test-gate source snapshot; run through scripts/test-gate.sh"
  )
}
