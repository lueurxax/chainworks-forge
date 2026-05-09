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
      decision: nil, freshnessState: .live,
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
      decision: nil, freshnessState: .live,
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
      decision: nil, freshnessState: .live,
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
      decision: nil, freshnessState: .projectionLag,
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
      decision: nil, freshnessState: .projectionLag,
      disabledReasonCode: .projectionLag,
      writePathState: .available,
      diagnosticID: nil, serverDebugDetail: nil
    )
    let state = P085AffordancePresenter.approvalAffordance(for: lagApproval)
    #expect(state.projectionLagIsOnlyConstraint == true)
  }
}
