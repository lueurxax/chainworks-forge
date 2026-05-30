import Foundation

// MARK: - P085 typed mutation conflict result

/// Typed result codes for approveApproval/rejectApproval mutations.
/// Fail-closed: unknown server values decode to .unknown(rawValue:) rather than crashing.
enum P085MutationConflictResultCode: Equatable, Sendable {
  case alreadyResolved
  case unknown(rawValue: String)

  nonisolated static func fromRaw(_ rawValue: String) -> P085MutationConflictResultCode {
    switch rawValue {
    case "already_resolved": return .alreadyResolved
    default: return .unknown(rawValue: rawValue)
    }
  }
}

// MARK: - P085 fail-closed freshness wrapper

enum P085FreshnessState: Equatable, Sendable {
  case live
  case refreshing
  case projectionLag
  case stale
  case unavailable
  case unauthorized
  case unknown(rawValue: String)

  nonisolated static func == (lhs: P085FreshnessState, rhs: P085FreshnessState) -> Bool {
    switch (lhs, rhs) {
    case (.live, .live), (.refreshing, .refreshing), (.projectionLag, .projectionLag),
         (.stale, .stale), (.unavailable, .unavailable), (.unauthorized, .unauthorized): return true
    case (.unknown(let l), .unknown(let r)): return l == r
    default: return false
    }
  }

  nonisolated init(_ p031State: P031FreshnessState) {
    switch p031State {
    case .live: self = .live
    case .refreshing: self = .refreshing
    case .projectionLag: self = .projectionLag
    case .stale: self = .stale
    case .unavailable: self = .unavailable
    case .unauthorized: self = .unauthorized
    }
  }

  nonisolated static func fromRaw(_ rawValue: String) -> P085FreshnessState {
    switch rawValue {
    case "live": return .live
    case "refreshing": return .refreshing
    case "projection_lag": return .projectionLag
    case "stale": return .stale
    case "unavailable": return .unavailable
    case "unauthorized": return .unauthorized
    default: return .unknown(rawValue: rawValue)
    }
  }
}

// MARK: - Immutable presenter DTOs

struct P085DiagnosticAffordanceState: Equatable, Sendable {
  let diagnosticID: String?
  let serverDebugDetail: String?
  let isAvailable: Bool

  nonisolated var copyLabel: String {
    isAvailable ? "Copy Diagnostic ID" : "No Diagnostic Available"
  }
}

enum P085SupportedInteraction: String, Equatable, Sendable, CaseIterable {
  case listRow = "list_row"
  case detailPane = "detail_pane"
  case contextMenu = "context_menu"
  case keyboardDefaultOpen = "keyboard_default_open"
  case quickLookWhenDetailAuthorized = "quick_look_when_detail_authorized"
}

struct P085ArtifactAffordanceState: Equatable, Sendable {
  enum PayloadPresentation: Equatable, Sendable {
    case available(payloadText: String?)
    case metadataOnly
    case deferred
    case generating
    case unavailable(reasonCode: P031PayloadUnavailableReasonCode?)
    case unknown(rawState: String)
  }

  let artifactID: String
  let name: String
  let payloadPresentation: PayloadPresentation
  let freshnessState: P085FreshnessState
  let supportedInteractions: [P085SupportedInteraction]
  let diagnostic: P085DiagnosticAffordanceState
  let label: String
  let helpText: String?
}

enum P085ApprovalActionAvailability: Equatable, Sendable {
  case actionable
  case disabled(reasonCode: P031DisabledReasonCode?, helpText: String)
  case hidden
}

struct P085ApprovalAffordanceState: Equatable, Sendable {
  let approvalID: String
  let approveAvailability: P085ApprovalActionAvailability
  let rejectAvailability: P085ApprovalActionAvailability
  let freshnessState: P085FreshnessState
  let diagnostic: P085DiagnosticAffordanceState
  // Freshness projection lag is diagnostic context only; never the sole actionability gate.
  let projectionLagIsOnlyConstraint: Bool
}

struct P085FreshnessAffordanceState: Equatable, Sendable {
  let state: P085FreshnessState
  let label: String
  // Freshness communicates recency only; never drives payload or approval actionability.
  let canDrivePayloadAvailability: Bool
  let canDriveApprovalActionability: Bool
}

// MARK: - Canonical P085 presenter

enum P085AffordancePresenter {

  nonisolated static func artifactListAffordance(
    for artifact: P031ArtifactReadModel
  ) -> P085ArtifactAffordanceState {
    let presentation = listPayloadPresentation(
      payloadAvailabilityState: artifact.payloadAvailabilityState,
      payloadUnavailableReasonCode: artifact.payloadUnavailableReasonCode,
      payloadText: artifact.payloadText
    )
    let freshness = P085FreshnessState(artifact.freshnessState)
    return P085ArtifactAffordanceState(
      artifactID: artifact.id,
      name: artifact.name,
      payloadPresentation: presentation,
      freshnessState: freshness,
      supportedInteractions: listInteractions(for: presentation),
      diagnostic: diagnosticAffordance(
        diagnosticID: artifact.diagnosticID,
        serverDebugDetail: artifact.serverDebugDetail,
        freshnessState: freshness
      ),
      label: listLabel(for: presentation, name: artifact.name),
      helpText: nil
    )
  }

  nonisolated static func artifactDetailAffordance(
    for artifact: P031ArtifactReadModel
  ) -> P085ArtifactAffordanceState {
    let presentation = detailPayloadPresentation(
      payloadAvailabilityState: artifact.payloadAvailabilityState,
      payloadUnavailableReasonCode: artifact.payloadUnavailableReasonCode,
      payloadText: artifact.payloadText
    )
    let freshness = P085FreshnessState(artifact.freshnessState)
    return P085ArtifactAffordanceState(
      artifactID: artifact.id,
      name: artifact.name,
      payloadPresentation: presentation,
      freshnessState: freshness,
      supportedInteractions: detailInteractions(for: presentation),
      diagnostic: diagnosticAffordance(
        diagnosticID: artifact.diagnosticID,
        serverDebugDetail: artifact.serverDebugDetail,
        freshnessState: freshness
      ),
      label: detailLabel(for: presentation, name: artifact.name),
      helpText: detailHelpText(for: presentation)
    )
  }

  /// Merges list affordance with an optional detail affordance for the currently selected artifact.
  /// Stale async detail responses (detail.artifactID != selectedArtifactID) do not overwrite
  /// the list affordance — selection change invalidates in-flight detail state.
  nonisolated static func mergedAffordance(
    list: P085ArtifactAffordanceState,
    detail: P085ArtifactAffordanceState?,
    selectedArtifactID: String?
  ) -> P085ArtifactAffordanceState {
    guard let detail,
      let selectedID = selectedArtifactID,
      selectedID == list.artifactID,
      selectedID == detail.artifactID
    else {
      return list
    }
    return detail
  }

  nonisolated static func approvalAffordance(
    for approval: P031ApprovalReadModel
  ) -> P085ApprovalAffordanceState {
    let freshness = P085FreshnessState(approval.freshnessState)
    let approveAvailability = actionAvailability(action: "approve", approval: approval)
    let rejectAvailability = actionAvailability(action: "reject", approval: approval)
    let projectionLagIsOnlyConstraint =
      approval.freshnessState == .projectionLag
      && approval.writePathState == .available
      && approval.disabledReasonCode == .projectionLag
    return P085ApprovalAffordanceState(
      approvalID: approval.id,
      approveAvailability: approveAvailability,
      rejectAvailability: rejectAvailability,
      freshnessState: freshness,
      diagnostic: diagnosticAffordance(
        diagnosticID: approval.diagnosticID,
        serverDebugDetail: approval.serverDebugDetail,
        freshnessState: freshness
      ),
      projectionLagIsOnlyConstraint: projectionLagIsOnlyConstraint
    )
  }

  nonisolated static func freshnessAffordance(
    snapshot: P031FreshnessSnapshot,
    surface: P031ReadRefreshSurface
  ) -> P085FreshnessAffordanceState {
    let state = P085FreshnessState(snapshot.state)
    return P085FreshnessAffordanceState(
      state: state,
      label: freshnessLabel(for: state),
      canDrivePayloadAvailability: false,
      canDriveApprovalActionability: false
    )
  }

  nonisolated static func diagnosticAffordance(
    diagnosticID: String?,
    serverDebugDetail: String?,
    freshnessState: P085FreshnessState
  ) -> P085DiagnosticAffordanceState {
    let isAvailable = diagnosticID != nil && freshnessState != .unauthorized
    return P085DiagnosticAffordanceState(
      diagnosticID: isAvailable ? diagnosticID : nil,
      serverDebugDetail: isAvailable ? serverDebugDetail : nil,
      isAvailable: isAvailable
    )
  }

  /// Fail-closed mapping from a raw string payload availability state.
  /// Unknown values produce `.unknown` rather than optimistic actionability.
  nonisolated static func payloadPresentation(fromRaw rawState: String, payloadText: String? = nil, reasonCode: P031PayloadUnavailableReasonCode? = nil) -> P085ArtifactAffordanceState.PayloadPresentation {
    switch rawState {
    case "available": return .available(payloadText: payloadText)
    case "metadata_only": return .metadataOnly
    case "payload_deferred": return .deferred
    case "generating": return .generating
    case "unavailable": return .unavailable(reasonCode: reasonCode)
    default: return .unknown(rawState: rawState)
    }
  }

  // MARK: Private

  private nonisolated static func listPayloadPresentation(
    payloadAvailabilityState: P031PayloadAvailabilityState,
    payloadUnavailableReasonCode: P031PayloadUnavailableReasonCode?,
    payloadText: String?
  ) -> P085ArtifactAffordanceState.PayloadPresentation {
    switch payloadAvailabilityState {
    case .available: return .available(payloadText: payloadText)
    case .metadataOnly: return .metadataOnly
    case .payloadDeferred: return .deferred
    case .generating: return .generating
    case .unavailable: return .unavailable(reasonCode: payloadUnavailableReasonCode)
    }
  }

  private nonisolated static func detailPayloadPresentation(
    payloadAvailabilityState: P031PayloadAvailabilityState,
    payloadUnavailableReasonCode: P031PayloadUnavailableReasonCode?,
    payloadText: String?
  ) -> P085ArtifactAffordanceState.PayloadPresentation {
    switch payloadAvailabilityState {
    case .available: return .available(payloadText: payloadText)
    case .metadataOnly: return .metadataOnly
    case .payloadDeferred: return .deferred
    case .generating: return .generating
    case .unavailable: return .unavailable(reasonCode: payloadUnavailableReasonCode)
    }
  }

  private nonisolated static func listInteractions(
    for presentation: P085ArtifactAffordanceState.PayloadPresentation
  ) -> [P085SupportedInteraction] {
    var interactions: [P085SupportedInteraction] = [.listRow, .contextMenu]
    switch presentation {
    case .available:
      interactions.append(contentsOf: [.keyboardDefaultOpen, .quickLookWhenDetailAuthorized])
    case .deferred:
      interactions.append(.keyboardDefaultOpen)
    default:
      break
    }
    return interactions
  }

  private nonisolated static func detailInteractions(
    for presentation: P085ArtifactAffordanceState.PayloadPresentation
  ) -> [P085SupportedInteraction] {
    var interactions: [P085SupportedInteraction] = [.detailPane, .contextMenu]
    if case .available = presentation {
      interactions.append(.quickLookWhenDetailAuthorized)
    }
    return interactions
  }

  private nonisolated static func listLabel(
    for presentation: P085ArtifactAffordanceState.PayloadPresentation,
    name: String
  ) -> String {
    switch presentation {
    case .available: return name
    case .metadataOnly: return "\(name) — metadata only"
    case .deferred: return "\(name) — Open to preview"
    case .generating: return "\(name) — Generating\u{2026}"
    case .unavailable: return "\(name) — Unavailable"
    case .unknown: return "\(name) — Status Unknown"
    }
  }

  private nonisolated static func detailLabel(
    for presentation: P085ArtifactAffordanceState.PayloadPresentation,
    name: String
  ) -> String {
    switch presentation {
    case .available: return name
    case .metadataOnly: return "\(name) — Metadata Only"
    case .deferred: return "\(name) — Preview Deferred"
    case .generating: return "\(name) — Generating"
    case .unavailable: return "\(name) — Preview Unavailable"
    case .unknown: return "\(name) — Status Unknown"
    }
  }

  private nonisolated static func detailHelpText(
    for presentation: P085ArtifactAffordanceState.PayloadPresentation
  ) -> String? {
    switch presentation {
    case .available: return nil
    case .metadataOnly: return "Only metadata is available for this artifact."
    case .deferred: return "Preview is available but deferred. Open to load."
    case .generating: return "This artifact is being generated."
    case .unavailable(let code):
      return code.map { "Preview unavailable: \($0.rawValue)" } ?? "Preview unavailable."
    case .unknown(let raw):
      return "Payload status unknown (\(raw)). Contact support if this persists."
    }
  }

  private nonisolated static func actionAvailability(
    action: String,
    approval: P031ApprovalReadModel
  ) -> P085ApprovalActionAvailability {
    // Fail closed: any non-nil disabledReasonCode from the server disables the action,
    // regardless of writePathState or availableActions. This prevents redacted, conflict,
    // duplicate, alreadyResolved, managedOutsideUI, and projectionLag codes from passing
    // through to .actionable when those conditions are the only active constraint.
    if let code = approval.disabledReasonCode {
      return .disabled(reasonCode: code, helpText: disabledHelpText(for: code))
    }
    // pending/requested are actionable; any other non-nil decision (granted/rejected/expired) is resolved.
    if let d = approval.decision, d != "pending", d != "requested" {
      return .disabled(reasonCode: .unsupportedAction, helpText: "Approval is already resolved.")
    }
    guard approval.writePathState == .available else {
      return .disabled(reasonCode: .writePathNotAvailable, helpText: disabledHelpText(for: .writePathNotAvailable))
    }
    guard approval.availableActions.contains(action) else {
      return .disabled(reasonCode: .unsupportedAction, helpText: disabledHelpText(for: .unsupportedAction))
    }
    return .actionable
  }

  private nonisolated static func disabledHelpText(for reason: P031DisabledReasonCode) -> String {
    switch reason {
    case .writePathNotAvailable: return "Write path is not available."
    case .managedOutsideUI: return "This action is managed outside the UI."
    case .ambiguousApprovalIdentity: return "Approval identity is ambiguous."
    case .staleRead: return "Read model is stale. Refresh and try again."
    case .projectionLag:
      return "Projection lag detected. The action may be available after the projection catches up."
    case .unauthorized: return "You are not authorized to perform this action."
    case .unsupportedAction: return "This action is not supported."
    case .redacted: return "This action has been redacted for security."
    case .conflict: return "A conflict was detected. Refresh to see latest state."
    case .duplicate: return "This action is a duplicate of an existing request."
    case .alreadyResolved: return "This action has already been resolved."
    case .approvalNotActionable: return "This approval is no longer actionable."
    case .observerScope: return "Read-only access. Approval actions are not available."
    case .nonApprovalMutation: return "GraphQL mutations are restricted to approval actions."
    case .capabilityOutOfScope: return "Action Not Available."
    }
  }

  private nonisolated static func freshnessLabel(for state: P085FreshnessState) -> String {
    switch state {
    case .live: return "Live"
    case .refreshing: return "Refreshing\u{2026}"
    case .projectionLag: return "Projection Lag"
    case .stale: return "Stale"
    case .unavailable: return "Unavailable"
    case .unauthorized: return "Unauthorized"
    case .unknown: return "Unknown"
    }
  }
}
