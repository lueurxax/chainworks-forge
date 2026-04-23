import SwiftUI

// MARK: - Proposal 013 Layer P: Failed Stage Evidence Panel

/// Shell-owned evidence panel showing raw output presence, validation
/// failure cause, receipt/transcript availability, and recommended next action.
struct FailedStageEvidencePanel: View {
    let evidencePacket: FailedStageEvidencePacket
    @State private var showFullValidation = false
    @State private var showOutputEnvelopes = false

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            // Header
            Label("Failure Evidence", systemImage: "exclamationmark.triangle.fill")
                .font(.headline)
                .foregroundStyle(.red)

            // Stage identification
            GroupBox {
                LabeledContent("Stage", value: evidencePacket.stageLabel)
                LabeledContent("Attempt", value: "#\(evidencePacket.stageAttemptNumber)")
                if let agentTitle = evidencePacket.failedAgentTitle {
                    LabeledContent("Failed Agent", value: agentTitle)
                }
                LabeledContent("Failure Class", value: evidencePacket.failureClass.rawValue.replacingOccurrences(of: "_", with: " ").capitalized)
            }

            // Evidence availability
            GroupBox("Evidence Availability") {
                evidenceRow("Raw Output", exists: evidencePacket.rawOutputsExist)
                evidenceRow("Receipt", exists: evidencePacket.receiptExists)
                evidenceRow("Transcript", exists: evidencePacket.transcriptExists)
            }

            // Failure summary
            GroupBox("Failure Summary") {
                Text(evidencePacket.failureSummary)
                    .font(.callout)
                    .foregroundStyle(.secondary)
                    .textSelection(.enabled)
            }

            if let friendlyFailure = XcodeRuntimeFriendlyFailure.first(in: evidencePacket.failureSummary) {
                GroupBox("Xcode Recovery") {
                    VStack(alignment: .leading, spacing: 6) {
                        Label(friendlyFailure.title, systemImage: "exclamationmark.triangle.fill")
                            .font(.callout.weight(.semibold))
                            .foregroundStyle(.orange)
                        Text(friendlyFailure.suggestedAction)
                            .font(.caption)
                            .foregroundStyle(.secondary)
                            .textSelection(.enabled)
                    }
                    .frame(maxWidth: .infinity, alignment: .leading)
                }
                .accessibilityIdentifier("xcode-friendly-failure")
            }

            // Validation failure details (if present)
            if let validationFailure = evidencePacket.validationFailure {
                DisclosureGroup("Validation Details", isExpanded: $showFullValidation) {
                    validationDetailView(validationFailure)
                }
            }

            // Output envelopes
            if !evidencePacket.outputEnvelopes.isEmpty {
                DisclosureGroup("Output Envelopes (\(evidencePacket.outputEnvelopes.count))", isExpanded: $showOutputEnvelopes) {
                    ForEach(evidencePacket.outputEnvelopes, id: \.id) { envelope in
                        outputEnvelopeRow(envelope)
                    }
                }
            }

            // Recovery recommendation
            if let recovery = evidencePacket.recoverySnapshot {
                recoveryRecommendationView(recovery)
            }

            // Timing
            GroupBox("Timing") {
                LabeledContent("Stage Started", value: evidencePacket.timing.stageStartedAt.formatted())
                if let completed = evidencePacket.timing.stageCompletedAt {
                    LabeledContent("Stage Completed", value: completed.formatted())
                }
                if let duration = evidencePacket.timing.agentDurationSeconds {
                    LabeledContent("Agent Duration", value: String(format: "%.1fs", duration))
                }
            }
        }
        .padding()
    }

    // MARK: - Evidence Row

    @ViewBuilder
    private func evidenceRow(_ label: String, exists: Bool) -> some View {
        HStack {
            Image(systemName: exists ? "checkmark.circle.fill" : "xmark.circle")
                .foregroundStyle(exists ? .green : .red)
            Text(label)
            Spacer()
            Text(exists ? "Present" : "Missing")
                .font(.caption)
                .foregroundStyle(.secondary)
        }
    }

    // MARK: - Validation Detail

    @ViewBuilder
    private func validationDetailView(_ failure: ValidationFailureRecord) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            ForEach(failure.outputResults, id: \.outputName) { result in
                HStack {
                    Image(systemName: result.status == .passed ? "checkmark.circle.fill" : "xmark.circle.fill")
                        .foregroundStyle(result.status == .passed ? .green : .red)
                    VStack(alignment: .leading) {
                        Text(result.outputName)
                            .font(.callout.monospaced())
                        if let error = result.validationError {
                            Text(error)
                                .font(.caption)
                                .foregroundStyle(.secondary)
                        }
                        if !result.missingFields.isEmpty {
                            Text("Missing: \(result.missingFields.joined(separator: ", "))")
                                .font(.caption)
                                .foregroundStyle(.orange)
                        }
                    }
                }
            }

            if !failure.contractMetadata.isEmpty {
                Divider()
                Text("Contract Metadata")
                    .font(.caption.bold())
                ForEach(failure.contractMetadata, id: \.outputName) { meta in
                    HStack {
                        Text(meta.contractID)
                            .font(.caption.monospaced())
                        Spacer()
                        Text("\(meta.machineFormat) / \(meta.validationMode)")
                            .font(.caption2)
                            .foregroundStyle(.secondary)
                    }
                }
            }
        }
        .padding(.vertical, 4)
    }

    // MARK: - Output Envelope Row

    @ViewBuilder
    private func outputEnvelopeRow(_ envelope: StructuredOutputEnvelope) -> some View {
        GroupBox {
            LabeledContent("Output", value: envelope.outputName)
            LabeledContent("Size", value: ByteCountFormatter.string(fromByteCount: Int64(envelope.rawPayloadSize), countStyle: .file))
            LabeledContent("Persisted", value: envelope.rawPayloadPersisted ? "Yes" : "No")
            if let contractID = envelope.contractID {
                LabeledContent("Contract", value: contractID)
            }
            if let result = envelope.validationResult {
                LabeledContent("Validation") {
                    Text(result.status.rawValue.capitalized)
                        .foregroundStyle(result.status == .passed ? .green : .red)
                }
            }
        }
    }

    // MARK: - Recovery Recommendation

    @ViewBuilder
    private func recoveryRecommendationView(_ snapshot: RecoveryActionSnapshot) -> some View {
        GroupBox("Recovery Recommendation") {
            if let recommended = snapshot.recommendedAction {
                VStack(alignment: .leading, spacing: 4) {
                    Text(recommended.action.rawValue.replacingOccurrences(of: "_", with: " ").capitalized)
                        .font(.callout.bold())
                    Text(recommended.explanation)
                        .font(.caption)
                        .foregroundStyle(.secondary)

                    HStack(spacing: 16) {
                        Label(recommended.staysInSameRun ? "Same Run" : "New Run",
                              systemImage: recommended.staysInSameRun ? "arrow.uturn.backward" : "doc.on.doc")
                        .font(.caption2)

                        if recommended.reusesSiblingOutputs {
                            Label("Reuses Siblings", systemImage: "link")
                                .font(.caption2)
                        }
                    }
                    .foregroundStyle(.secondary)
                }
            }

            if snapshot.availableActions.count > 1 {
                Divider()
                Text("\(snapshot.availableActions.count) actions available")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }

            Text("Source: \(snapshot.source.rawValue)")
                .font(.caption2)
                .foregroundStyle(.tertiary)
        }
    }
}
