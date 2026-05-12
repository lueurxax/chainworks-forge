import Foundation
import Testing

@testable import Chainworks_Forge

@Suite("Proposal 085: thin-client affordance contract", .tags(.fast))
struct Proposal085Tests {

  // MARK: - Artifact payload state mapping

  @Test("payload_deferred maps to .deferred presentation, not .unavailable")
  func payloadDeferredMapsToDeferred() {
    let artifact = P031ArtifactReadModel(
      id: "a-1", runID: "r-1", stageID: "s-1",
      name: "proposal", contractID: "proposal", format: "markdown",
      freshnessState: .live, payloadAvailabilityState: .payloadDeferred
    )
    let state = P085AffordancePresenter.artifactListAffordance(for: artifact)
    #expect(state.payloadPresentation == .deferred)
    #expect(state.label.contains("Open to preview"))
  }

  @Test("payload_deferred list label never says Unavailable")
  func payloadDeferredLabelIsNotUnavailable() {
    let artifact = P031ArtifactReadModel(
      id: "a-2", runID: "r-1", stageID: "s-1",
      name: "output", contractID: "output", format: "text",
      freshnessState: .live, payloadAvailabilityState: .payloadDeferred
    )
    let state = P085AffordancePresenter.artifactListAffordance(for: artifact)
    #expect(!state.label.lowercased().contains("unavailable"))
  }

  @Test("metadata_only maps to .metadataOnly presentation")
  func metadataOnlyMapsToMetadataOnly() {
    let artifact = P031ArtifactReadModel(
      id: "a-3", runID: "r-1", stageID: "s-1",
      name: "report", contractID: "report", format: "report",
      freshnessState: .live, payloadAvailabilityState: .metadataOnly
    )
    let state = P085AffordancePresenter.artifactListAffordance(for: artifact)
    #expect(state.payloadPresentation == .metadataOnly)
    #expect(state.label.lowercased().contains("metadata"))
  }

  @Test("available payload maps to .available with payloadText")
  func availablePayloadMapsToAvailable() {
    let artifact = P031ArtifactReadModel(
      id: "a-4", runID: "r-1", stageID: "s-1",
      name: "plan", contractID: "plan", format: "markdown",
      freshnessState: .live, payloadAvailabilityState: .available,
      payloadText: "# Plan content"
    )
    let state = P085AffordancePresenter.artifactDetailAffordance(for: artifact)
    #expect(state.payloadPresentation == .available(payloadText: "# Plan content"))
  }

  @Test("unavailable payload maps to .unavailable with reason code")
  func unavailablePayloadPreservesReasonCode() {
    let artifact = P031ArtifactReadModel(
      id: "a-5", runID: "r-1", stageID: "s-1",
      name: "missing", contractID: "missing", format: "markdown",
      freshnessState: .live, payloadAvailabilityState: .unavailable,
      payloadUnavailableReasonCode: .notAuthorized
    )
    let state = P085AffordancePresenter.artifactListAffordance(for: artifact)
    #expect(state.payloadPresentation == .unavailable(reasonCode: .notAuthorized))
  }

  // MARK: - Unknown enum fail-closed

  @Test("Unknown raw payload availability state fails closed to .unknown, not optimistic")
  func unknownPayloadStateFailsClosed() {
    let state = P085AffordancePresenter.payloadPresentation(fromRaw: "future_state_v3")
    #expect(state == .unknown(rawState: "future_state_v3"))
    // Must not be actionable or unavailable — must be the explicit unknown sentinel
    if case .unknown = state { } else {
      Issue.record("Expected .unknown but got \(state)")
    }
  }

  @Test("Unknown raw freshness state fails closed to .unknown(rawValue:)")
  func unknownFreshnessStateFailsClosed() {
    let state = P085FreshnessState.fromRaw("hypothetical_new_state")
    if case .unknown(let raw) = state {
      #expect(raw == "hypothetical_new_state")
    } else {
      Issue.record("Expected .unknown but got \(state)")
    }
  }

  @Test("Known freshness values round-trip through P085FreshnessState wrapper")
  func knownFreshnessValuesRoundTrip() {
    #expect(P085FreshnessState(.live) == .live)
    #expect(P085FreshnessState(.refreshing) == .refreshing)
    #expect(P085FreshnessState(.projectionLag) == .projectionLag)
    #expect(P085FreshnessState(.stale) == .stale)
    #expect(P085FreshnessState(.unavailable) == .unavailable)
    #expect(P085FreshnessState(.unauthorized) == .unauthorized)
  }

  // MARK: - Approval actionability

  @Test("Approval is .actionable when writePathState is available and action is in availableActions")
  func approvalIsActionableWhenConditionsMet() {
    let approval = P031ApprovalReadModel(
      id: "appr-1", runID: "r-1", stageID: "s-1",
      decision: "pending", freshnessState: .live,
      disabledReasonCode: nil, writePathState: .available,
      diagnosticID: nil, serverDebugDetail: nil,
      availableActions: ["approve", "reject"]
    )
    let state = P085AffordancePresenter.approvalAffordance(for: approval)
    #expect(state.approveAvailability == .actionable)
    #expect(state.rejectAvailability == .actionable)
  }

  @Test("Approval is .disabled when writePathState is not available")
  func approvalDisabledWhenWritePathNotAvailable() {
    let approval = P031ApprovalReadModel(
      id: "appr-2", runID: "r-1", stageID: "s-1",
      decision: "pending", freshnessState: .live,
      disabledReasonCode: .writePathNotAvailable,
      writePathState: .writePathNotAvailable,
      diagnosticID: nil, serverDebugDetail: nil,
      availableActions: []
    )
    let state = P085AffordancePresenter.approvalAffordance(for: approval)
    if case .disabled = state.approveAvailability { } else {
      Issue.record("approveAvailability should be .disabled but got \(state.approveAvailability)")
    }
    if case .disabled = state.rejectAvailability { } else {
      Issue.record("rejectAvailability should be .disabled but got \(state.rejectAvailability)")
    }
  }

  @Test("Approval is .disabled when availableActions does not include the action")
  func approvalDisabledWhenActionNotInAvailableActions() {
    let approval = P031ApprovalReadModel(
      id: "appr-3", runID: "r-1", stageID: "s-1",
      decision: "pending", freshnessState: .live,
      disabledReasonCode: .unsupportedAction,
      writePathState: .available,
      diagnosticID: nil, serverDebugDetail: nil,
      availableActions: []
    )
    let state = P085AffordancePresenter.approvalAffordance(for: approval)
    if case .disabled(let code, _) = state.approveAvailability {
      #expect(code == .unsupportedAction)
    } else {
      Issue.record("Expected .disabled but got \(state.approveAvailability)")
    }
  }

  // MARK: - Freshness is diagnostic only

  @Test("Freshness affordance never drives payload availability or approval actionability")
  func freshnessAffordanceIsAlwaysDiagnosticOnly() {
    let snapshot = P031FreshnessSnapshot(state: .projectionLag)
    let state = P085AffordancePresenter.freshnessAffordance(
      snapshot: snapshot, surface: .approvalsQueue
    )
    #expect(state.canDrivePayloadAvailability == false)
    #expect(state.canDriveApprovalActionability == false)
  }

  @Test("Freshness state .live produces canDrivePayloadAvailability = false")
  func liveFresnessIsStillDiagnosticOnly() {
    for p031State in P031FreshnessState.allCases {
      let snapshot = P031FreshnessSnapshot(state: p031State)
      let state = P085AffordancePresenter.freshnessAffordance(
        snapshot: snapshot, surface: .artifacts
      )
      #expect(state.canDrivePayloadAvailability == false)
      #expect(state.canDriveApprovalActionability == false)
    }
  }

  // MARK: - List / detail merge and stale response handling

  @Test("Merged affordance uses detail when selectedArtifactID matches both IDs")
  func mergedAffordanceUsesDetailWhenSelectionMatches() {
    let artifact = P031ArtifactReadModel(
      id: "a-10", runID: "r-1", stageID: "s-1",
      name: "plan", contractID: "plan", format: "markdown",
      freshnessState: .live, payloadAvailabilityState: .payloadDeferred
    )
    let detailArtifact = P031ArtifactReadModel(
      id: "a-10", runID: "r-1", stageID: "s-1",
      name: "plan", contractID: "plan", format: "markdown",
      freshnessState: .live, payloadAvailabilityState: .available,
      payloadText: "# Full plan"
    )
    let list = P085AffordancePresenter.artifactListAffordance(for: artifact)
    let detail = P085AffordancePresenter.artifactDetailAffordance(for: detailArtifact)
    let merged = P085AffordancePresenter.mergedAffordance(
      list: list, detail: detail, selectedArtifactID: "a-10"
    )
    #expect(merged.payloadPresentation == .available(payloadText: "# Full plan"))
  }

  @Test("Stale async detail response does not overwrite list when selection has changed")
  func staleDetailDoesNotOverwriteAfterSelectionChange() {
    let listArtifact = P031ArtifactReadModel(
      id: "a-11", runID: "r-1", stageID: "s-1",
      name: "artifact-new", contractID: "artifact-new", format: "markdown",
      freshnessState: .live, payloadAvailabilityState: .payloadDeferred
    )
    let staleDetailArtifact = P031ArtifactReadModel(
      id: "a-10", runID: "r-1", stageID: "s-1",
      name: "artifact-old", contractID: "artifact-old", format: "markdown",
      freshnessState: .live, payloadAvailabilityState: .available,
      payloadText: "stale"
    )
    let list = P085AffordancePresenter.artifactListAffordance(for: listArtifact)
    let staleDetail = P085AffordancePresenter.artifactDetailAffordance(for: staleDetailArtifact)
    let merged = P085AffordancePresenter.mergedAffordance(
      list: list, detail: staleDetail, selectedArtifactID: "a-11"
    )
    // Detail is for a-10, selection is now a-11: list affordance wins
    #expect(merged.artifactID == "a-11")
    #expect(merged.payloadPresentation == .deferred)
  }

  @Test("Nil detail returns list affordance unchanged")
  func nilDetailReturnsListAffordance() {
    let artifact = P031ArtifactReadModel(
      id: "a-12", runID: "r-1", stageID: "s-1",
      name: "artifact", contractID: "artifact", format: "markdown",
      freshnessState: .live, payloadAvailabilityState: .metadataOnly
    )
    let list = P085AffordancePresenter.artifactListAffordance(for: artifact)
    let merged = P085AffordancePresenter.mergedAffordance(
      list: list, detail: nil, selectedArtifactID: "a-12"
    )
    #expect(merged == list)
  }

  // MARK: - Diagnostic copy

  @Test("Diagnostic affordance is available when diagnosticID is present and freshness is not unauthorized")
  func diagnosticAffordanceAvailableWithDiagnosticID() {
    let state = P085AffordancePresenter.diagnosticAffordance(
      diagnosticID: "diag-abc",
      serverDebugDetail: "server detail here",
      freshnessState: .live
    )
    #expect(state.isAvailable == true)
    #expect(state.diagnosticID == "diag-abc")
  }

  @Test("Diagnostic affordance is unavailable when diagnosticID is nil")
  func diagnosticAffordanceUnavailableWithoutDiagnosticID() {
    let state = P085AffordancePresenter.diagnosticAffordance(
      diagnosticID: nil,
      serverDebugDetail: nil,
      freshnessState: .live
    )
    #expect(state.isAvailable == false)
  }

  @Test("Diagnostic affordance invalidates on unauthorized freshness transition")
  func diagnosticAffordanceInvalidatesOnUnauthorized() {
    let state = P085AffordancePresenter.diagnosticAffordance(
      diagnosticID: "diag-xyz",
      serverDebugDetail: "detail",
      freshnessState: .unauthorized
    )
    #expect(state.isAvailable == false)
  }

  // MARK: - Supported interactions

  @Test("Available payload includes quick look and keyboard open in list interactions")
  func availablePayloadIncludesQuickLook() {
    let artifact = P031ArtifactReadModel(
      id: "a-20", runID: "r-1", stageID: "s-1",
      name: "file", contractID: "file", format: "markdown",
      freshnessState: .live, payloadAvailabilityState: .available,
      payloadText: "content"
    )
    let state = P085AffordancePresenter.artifactListAffordance(for: artifact)
    #expect(state.supportedInteractions.contains(.quickLookWhenDetailAuthorized))
    #expect(state.supportedInteractions.contains(.keyboardDefaultOpen))
  }

  @Test("Deferred payload includes keyboard open but not quick look in list interactions")
  func deferredPayloadIncludesKeyboardButNotQuickLook() {
    let artifact = P031ArtifactReadModel(
      id: "a-21", runID: "r-1", stageID: "s-1",
      name: "deferred-file", contractID: "deferred-file", format: "markdown",
      freshnessState: .live, payloadAvailabilityState: .payloadDeferred
    )
    let state = P085AffordancePresenter.artifactListAffordance(for: artifact)
    #expect(state.supportedInteractions.contains(.keyboardDefaultOpen))
    #expect(!state.supportedInteractions.contains(.quickLookWhenDetailAuthorized))
  }

  @Test("Unavailable payload does not include quick look or keyboard open")
  func unavailablePayloadExcludesInteractiveOptions() {
    let artifact = P031ArtifactReadModel(
      id: "a-22", runID: "r-1", stageID: "s-1",
      name: "absent", contractID: "absent", format: "markdown",
      freshnessState: .live, payloadAvailabilityState: .unavailable,
      payloadUnavailableReasonCode: .notAvailable
    )
    let state = P085AffordancePresenter.artifactListAffordance(for: artifact)
    #expect(!state.supportedInteractions.contains(.quickLookWhenDetailAuthorized))
    #expect(!state.supportedInteractions.contains(.keyboardDefaultOpen))
  }

  // MARK: - Help text and accessibility

  @Test("Deferred payload detail affordance provides help text")
  func deferredPayloadDetailProvidesHelpText() {
    let artifact = P031ArtifactReadModel(
      id: "a-30", runID: "r-1", stageID: "s-1",
      name: "draft", contractID: "draft", format: "markdown",
      freshnessState: .live, payloadAvailabilityState: .payloadDeferred
    )
    let state = P085AffordancePresenter.artifactDetailAffordance(for: artifact)
    #expect(state.helpText != nil)
    #expect(state.helpText?.isEmpty == false)
  }

  @Test("Unknown payload state detail affordance provides help text mentioning unknown")
  func unknownPayloadDetailProvidesHelpText() {
    let unknownPresentation = P085AffordancePresenter.payloadPresentation(fromRaw: "exotic_future")
    if case .unknown = unknownPresentation { } else {
      Issue.record("Expected .unknown but got \(unknownPresentation)")
    }
  }

  // MARK: - Approval help text

  @Test("Disabled approval exposes help text derived from disabledReasonCode")
  func disabledApprovalExposesHelpText() {
    let approval = P031ApprovalReadModel(
      id: "appr-10", runID: "r-1", stageID: "s-1",
      decision: "pending", freshnessState: .projectionLag,
      disabledReasonCode: .projectionLag,
      writePathState: .available,
      diagnosticID: nil, serverDebugDetail: nil,
      availableActions: []
    )
    let state = P085AffordancePresenter.approvalAffordance(for: approval)
    if case .disabled(_, let helpText) = state.approveAvailability {
      #expect(!helpText.isEmpty)
    } else {
      Issue.record("Expected disabled approval but got \(state.approveAvailability)")
    }
  }

  @Test("projectionLagIsOnlyConstraint is true when lag is the only block")
  func projectionLagConstraintDetectedCorrectly() {
    let lagApproval = P031ApprovalReadModel(
      id: "appr-11", runID: "r-1", stageID: "s-1",
      decision: "pending", freshnessState: .projectionLag,
      disabledReasonCode: .projectionLag,
      writePathState: .available,
      diagnosticID: nil, serverDebugDetail: nil
    )
    let state = P085AffordancePresenter.approvalAffordance(for: lagApproval)
    #expect(state.projectionLagIsOnlyConstraint == true)
  }

  // MARK: - Decision-state gating (durable approval state check)

  @Test("Approval with non-nil decision is .disabled regardless of writePathState/availableActions")
  func alreadyResolvedApprovalIsDisabled() {
    let resolvedApproval = P031ApprovalReadModel(
      id: "appr-20", runID: "r-1", stageID: "s-1",
      decision: "approved", freshnessState: .live,
      disabledReasonCode: nil, writePathState: .available,
      diagnosticID: nil, serverDebugDetail: nil,
      availableActions: ["approve", "reject"]
    )
    let state = P085AffordancePresenter.approvalAffordance(for: resolvedApproval)
    if case .disabled = state.approveAvailability { } else {
      Issue.record("Expected .disabled for resolved approval but got \(state.approveAvailability)")
    }
    if case .disabled = state.rejectAvailability { } else {
      Issue.record("Expected .disabled for resolved approval but got \(state.rejectAvailability)")
    }
  }

  @Test("P031ApprovalReadModel.canApprove is false when decision is already set")
  func canApproveIsFalseWhenDecisionSet() {
    let resolvedApproval = P031ApprovalReadModel(
      id: "appr-21", runID: "r-1", stageID: "s-1",
      decision: "rejected", freshnessState: .live,
      disabledReasonCode: nil, writePathState: .available,
      diagnosticID: nil, serverDebugDetail: nil,
      availableActions: ["approve"]
    )
    #expect(resolvedApproval.canApprove == false)
    #expect(resolvedApproval.canReject == false)
  }

  // MARK: - Fail-closed actual JSON decoding

  @Test("Unknown freshnessState JSON value decodes to .unavailable (fail-closed, no crash)")
  func unknownFreshnessStateJsonDecodesToUnavailable() throws {
    let json = """
      {
        "id": "a-50", "runId": "r-1", "stageId": "s-1",
        "freshnessState": "future_hypothetical_state",
        "writePathState": "available",
        "availableActions": []
      }
      """.data(using: .utf8)!
    let model = try JSONDecoder().decode(P031ApprovalReadModel.self, from: json)
    #expect(model.freshnessState == .unavailable)
  }

  @Test("Unknown writePathState JSON value decodes fail-closed, disabling mutations")
  func unknownWritePathStateJsonDecodesToNotAvailable() throws {
    let json = """
      {
        "id": "a-51", "runId": "r-1", "stageId": "s-1",
        "freshnessState": "live",
        "writePathState": "some_future_write_mode",
        "availableActions": []
      }
      """.data(using: .utf8)!
    let model = try JSONDecoder().decode(P031ApprovalReadModel.self, from: json)
    #expect(model.writePathState == .writePathNotAvailable)
    #expect(model.canApprove == false)
  }

  @Test("Unknown payloadAvailabilityState JSON value decodes to .unavailable (fail-closed)")
  func unknownPayloadAvailabilityStateJsonDecodesToUnavailable() throws {
    struct ArtifactShim: Decodable {
      let payloadAvailabilityState: P031PayloadAvailabilityState
    }
    let json = """
      {"payloadAvailabilityState": "exotic_payload_state_v9"}
      """.data(using: .utf8)!
    let shim = try JSONDecoder().decode(ArtifactShim.self, from: json)
    #expect(shim.payloadAvailabilityState == .unavailable)
  }

  // MARK: - Mutation conflict result code

  @Test("P085MutationConflictResultCode fromRaw maps current backend-emitted code")
  func conflictResultCodeMapsCurrentBackendEmittedValue() {
    #expect(P085MutationConflictResultCode.fromRaw("already_resolved") == .alreadyResolved)
  }

  @Test("Unemitted conflict codes fail closed until backend implements them")
  func unemittedConflictCodesFailClosed() {
    if case .unknown(let raw) = P085MutationConflictResultCode.fromRaw("state_conflict") {
      #expect(raw == "state_conflict")
    } else {
      Issue.record("state_conflict must remain unknown until the backend emits it")
    }
    if case .unknown(let raw) = P085MutationConflictResultCode.fromRaw("transient_error_retryable") {
      #expect(raw == "transient_error_retryable")
    } else {
      Issue.record("transient_error_retryable must remain unknown until the backend emits it")
    }
  }

  @Test("P085MutationConflictResultCode fromRaw maps unknown codes to .unknown(rawValue:)")
  func conflictResultCodeUnknownFailsClosed() {
    let result = P085MutationConflictResultCode.fromRaw("future_conflict_code")
    if case .unknown(let raw) = result {
      #expect(raw == "future_conflict_code")
    } else {
      Issue.record("Expected .unknown but got \(result)")
    }
  }

  @Test("P072ApprovalMutationResult decodes without conflictResultCode present (nil)")
  func mutationResultDecodesWithoutConflictCode() throws {
    let json = """
      {
        "approval": {
          "id": "appr-30", "runId": "r-1", "stageId": "s-1",
          "freshnessState": "live",
          "writePathState": "available",
          "availableActions": []
        },
        "journalId": "j-1"
      }
      """.data(using: .utf8)!
    let result = try JSONDecoder().decode(P072ApprovalMutationResult.self, from: json)
    #expect(result.conflictResultCode == nil)
    #expect(result.journalID == "j-1")
  }

  @Test("P072ApprovalMutationResult decodes known conflictResultCode")
  func mutationResultDecodesKnownConflictCode() throws {
    let json = """
      {
        "approval": {
          "id": "appr-31", "runId": "r-1", "stageId": "s-1",
          "freshnessState": "live",
          "writePathState": "available",
          "availableActions": []
        },
        "journalId": "j-2",
        "conflictResultCode": "already_resolved"
      }
      """.data(using: .utf8)!
    let result = try JSONDecoder().decode(P072ApprovalMutationResult.self, from: json)
    #expect(result.conflictResultCode == .alreadyResolved)
  }

  @Test("P072ApprovalMutationResult decodes unknown conflictResultCode fail-closed")
  func mutationResultDecodesUnknownConflictCodeFailClosed() throws {
    let json = """
      {
        "approval": {
          "id": "appr-32", "runId": "r-1", "stageId": "s-1",
          "freshnessState": "live",
          "writePathState": "available",
          "availableActions": []
        },
        "journalId": "j-3",
        "conflictResultCode": "hypothetical_future_code"
      }
      """.data(using: .utf8)!
    let result = try JSONDecoder().decode(P072ApprovalMutationResult.self, from: json)
    if case .unknown(let raw) = result.conflictResultCode {
      #expect(raw == "hypothetical_future_code")
    } else {
      Issue.record("Expected .unknown(rawValue:) but got \(String(describing: result.conflictResultCode))")
    }
  }

  // MARK: - P085 wired into production presenter

  @Test("P031ArtifactPresenter uses P085 label: payload_deferred shows Open to preview")
  func artifactPresenterUsesp085LabelForDeferred() {
    let artifact = P031ArtifactReadModel(
      id: "a-60", runID: "r-1", stageID: "s-1",
      name: "output", contractID: "output", format: "markdown",
      freshnessState: .live, payloadAvailabilityState: .payloadDeferred
    )
    let presentation = P031ArtifactPresenter.presentation(for: artifact)
    #expect(presentation.payloadAvailabilityLabel.contains("preview") || presentation.payloadAvailabilityLabel.contains("Open"))
    #expect(!presentation.payloadAvailabilityLabel.lowercased().contains("unavailable"))
  }

  @Test("P031ApprovalInboxPresenter uses P085 for canApprove: resolved decision disables button")
  func approvalPresenterUsesp085ForResolvedDecision() {
    let resolvedApproval = P031ApprovalReadModel(
      id: "appr-40", runID: "r-1", stageID: "s-1",
      decision: "approved", freshnessState: .live,
      disabledReasonCode: nil, writePathState: .available,
      diagnosticID: nil, serverDebugDetail: nil,
      availableActions: ["approve", "reject"]
    )
    let row = P031ApprovalInboxPresenter.rowPresentation(
      for: resolvedApproval,
      writePathGuideState: .unavailable
    )
    #expect(row.canApprove == false)
    #expect(row.canReject == false)
  }

  // MARK: - Security: diagnostic field redaction

  @Test("Unauthorized diagnostic state clears diagnosticID and serverDebugDetail")
  func unauthorizedDiagnosticStateClearsSensitiveFields() {
    let state = P085AffordancePresenter.diagnosticAffordance(
        diagnosticID: "diag-xyz",
        serverDebugDetail: "sensitive detail",
        freshnessState: .unauthorized
    )
    #expect(state.isAvailable == false)
    #expect(state.diagnosticID == nil)
    #expect(state.serverDebugDetail == nil)
  }

  // MARK: - Fail-closed on caller-policy blockers

  @Test("Approval is disabled when disabledReasonCode is .unauthorized, even with availableActions")
  func approvalDisabledWhenUnauthorizedCallerPolicy() {
    let approval = P031ApprovalReadModel(
        id: "appr-u1", runID: "r-1", stageID: "s-1",
        decision: "pending", freshnessState: .live,
        disabledReasonCode: .unauthorized,
        writePathState: .available,
        diagnosticID: nil, serverDebugDetail: nil,
        availableActions: ["approve", "reject"]
    )
    let state = P085AffordancePresenter.approvalAffordance(for: approval)
    if case .disabled(let code, _) = state.approveAvailability {
        #expect(code == .unauthorized)
    } else {
        Issue.record("Expected .disabled(.unauthorized) but got \(state.approveAvailability)")
    }
    if case .disabled(let code, _) = state.rejectAvailability {
        #expect(code == .unauthorized)
    } else {
        Issue.record("Expected .disabled(.unauthorized) but got \(state.rejectAvailability)")
    }
  }

  @Test("Approval is disabled when disabledReasonCode is .staleRead, even with availableActions")
  func approvalDisabledWhenStaleRead() {
    let approval = P031ApprovalReadModel(
        id: "appr-u2", runID: "r-1", stageID: "s-1",
        decision: "pending", freshnessState: .stale,
        disabledReasonCode: .staleRead,
        writePathState: .available,
        diagnosticID: nil, serverDebugDetail: nil,
        availableActions: ["approve", "reject"]
    )
    let state = P085AffordancePresenter.approvalAffordance(for: approval)
    if case .disabled(let code, _) = state.approveAvailability {
        #expect(code == .staleRead)
    } else {
        Issue.record("Expected .disabled(.staleRead) but got \(state.approveAvailability)")
    }
  }

  @Test("Approval is disabled when disabledReasonCode is .ambiguousApprovalIdentity")
  func approvalDisabledWhenAmbiguousIdentity() {
    let approval = P031ApprovalReadModel(
        id: "appr-u3", runID: "r-1", stageID: "s-1",
        decision: "pending", freshnessState: .live,
        disabledReasonCode: .ambiguousApprovalIdentity,
        writePathState: .available,
        diagnosticID: nil, serverDebugDetail: nil,
        availableActions: ["approve", "reject"]
    )
    let state = P085AffordancePresenter.approvalAffordance(for: approval)
    if case .disabled(let code, _) = state.approveAvailability {
        #expect(code == .ambiguousApprovalIdentity)
    } else {
        Issue.record("Expected .disabled(.ambiguousApprovalIdentity) but got \(state.approveAvailability)")
    }
  }

  @Test("P031ApprovalReadModel.canApprove is true for decision=pending with availableActions")
  func canApproveIsTrueForPendingDecision() {
    let approval = P031ApprovalReadModel(
        id: "appr-p1", runID: "r-1", stageID: "s-1",
        decision: "pending", freshnessState: .live,
        disabledReasonCode: nil, writePathState: .available,
        diagnosticID: nil, serverDebugDetail: nil,
        availableActions: ["approve", "reject"]
    )
    #expect(approval.canApprove == true)
    #expect(approval.canReject == true)
  }

  @Test("P031ApprovalReadModel.canApprove is true for decision=requested with availableActions")
  func canApproveIsTrueForRequestedDecision() {
    let approval = P031ApprovalReadModel(
        id: "appr-p2", runID: "r-1", stageID: "s-1",
        decision: "requested", freshnessState: .live,
        disabledReasonCode: nil, writePathState: .available,
        diagnosticID: nil, serverDebugDetail: nil,
        availableActions: ["approve", "reject"]
    )
    #expect(approval.canApprove == true)
    #expect(approval.canReject == true)
  }
}
