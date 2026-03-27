import Foundation
import SwiftData
import CryptoKit

// MARK: - SignOffEvidencePackBuilder (Proposal 008 — §5.7)

/// Exports the final review packet for a benchmark cohort or individual run.
/// The exported packet contains all inputs and outputs needed to replay
/// the GO/HOLD decision without access to the live database.
@MainActor
struct SignOffEvidencePackBuilder {

    let modelContext: ModelContext

    // MARK: - Build Cohort Packet

    /// Build a complete sign-off evidence packet from a cohort and its evaluation snapshot.
    /// - Parameters:
    ///   - cohort: The benchmark cohort being evaluated
    ///   - snapshot: The MVPSignOffDecisionSnapshot produced by the evaluator
    /// - Returns: A self-contained, Codable packet suitable for export
    func buildCohortPacket(
        cohort: BenchmarkCohort,
        snapshot: MVPSignOffDecisionSnapshot
    ) -> SignOffPacket {
        let pairs = cohort.pairs
        let pairRecords = pairs.map { pair in
            SignOffPacket.PairRecord(
                pairID: pair.id,
                ideaIdentifier: pair.ideaIdentifier,
                repositoryID: pair.repositoryID,
                createdAt: pair.createdAt,
                manualRecord: pair.manualRecord.map { buildExecutionSnapshot($0) },
                appDrivenRecord: pair.appDrivenRecord.map { buildExecutionSnapshot($0) }
            )
        }

        let cohortMembers = cohort.ideaMembers.map { member in
            SignOffPacket.CohortMember(
                ideaIdentifier: member.ideaIdentifier,
                title: member.title,
                repositoryID: member.repositoryID
            )
        }

        // Reconstruct median computation inputs for reproducibility
        let manualTimes = pairs.compactMap { $0.manualRecord?.totalOrchestrationTimeSeconds }
        let appTimes = pairs.compactMap { $0.appDrivenRecord?.totalOrchestrationTimeSeconds }

        let medianInputs = SignOffPacket.MedianInputs(
            manualOrchestrationTimes: manualTimes,
            appOrchestrationTimes: appTimes,
            proposalApprovalTimes: pairs.compactMap { $0.appDrivenRecord?.timeToProposalApprovalSeconds },
            implementationApprovalTimes: pairs.compactMap { $0.appDrivenRecord?.timeToImplementationApprovalSeconds },
            releaseDecisionTimes: pairs.compactMap { $0.appDrivenRecord?.timeToFinalReleaseDecisionSeconds }
        )

        let medianOutputs = SignOffPacket.MedianOutputs(
            medianManualOrchestrationSeconds: snapshot.medianManualOrchestrationSeconds,
            medianAppOrchestrationSeconds: snapshot.medianAppOrchestrationSeconds,
            medianImprovementPercent: snapshot.medianImprovementPercent,
            medianProposalApprovalSeconds: snapshot.medianProposalApprovalSeconds,
            medianImplementationApprovalSeconds: snapshot.medianImplementationApprovalSeconds,
            medianReleaseDecisionSeconds: snapshot.medianReleaseDecisionSeconds
        )

        let packet = SignOffPacket(
            packetVersion: "008-v1",
            exportedAt: Date(),
            evaluatorVersion: snapshot.evaluatorVersion,
            cohortID: cohort.id,
            cohortLabel: cohort.label,
            cohortStatus: cohort.status.rawValue,
            cohortMembers: cohortMembers,
            pairRecords: pairRecords,
            medianInputs: medianInputs,
            medianOutputs: medianOutputs,
            decision: snapshot.decision.rawValue,
            failingGateReasons: snapshot.failingGateReasons,
            pairCount: snapshot.pairCount,
            happyPathCount: snapshot.happyPathCount,
            recoveredCount: snapshot.recoveredCount,
            snapshotID: snapshot.id,
            payloadChecksum: computePacketChecksum(
                pairRecords: pairRecords,
                medianInputs: medianInputs,
                medianOutputs: medianOutputs,
                decision: snapshot.decision.rawValue,
                failingGateReasons: snapshot.failingGateReasons
            )
        )

        return packet
    }

    // MARK: - Export to File

    /// Export a sign-off packet to a JSON file at the specified destination.
    /// Creates intermediate directories if needed.
    /// - Parameters:
    ///   - packet: The sign-off evidence packet to export
    ///   - destinationURL: The file URL to write the packet to
    func exportToFile(packet: SignOffPacket, destinationURL: URL) throws {
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
        encoder.dateEncodingStrategy = .iso8601

        let data = try encoder.encode(packet)

        try FileManager.default.createDirectory(
            at: destinationURL.deletingLastPathComponent(),
            withIntermediateDirectories: true
        )

        try data.write(to: destinationURL, options: .atomic)
    }

    // MARK: - Helpers

    private func buildExecutionSnapshot(
        _ record: BenchmarkExecutionRecord
    ) -> SignOffPacket.ExecutionRecordSnapshot {
        SignOffPacket.ExecutionRecordSnapshot(
            recordID: record.id,
            executionMode: record.executionMode.rawValue,
            linkedRunID: record.linkedRunID,
            startedAt: record.startedAt,
            completedAt: record.completedAt,
            totalOrchestrationTimeSeconds: record.totalOrchestrationTimeSeconds,
            timeToProposalApprovalSeconds: record.timeToProposalApprovalSeconds,
            timeToImplementationApprovalSeconds: record.timeToImplementationApprovalSeconds,
            timeToFinalReleaseDecisionSeconds: record.timeToFinalReleaseDecisionSeconds,
            terminalOutcome: record.terminalOutcome.rawValue,
            artifactLinks: record.artifactLinks.map { link in
                SignOffPacket.ArtifactLinkSnapshot(
                    artifactID: link.artifactID,
                    name: link.name,
                    role: link.role
                )
            },
            notes: record.notes
        )
    }

    private func computePacketChecksum(
        pairRecords: [SignOffPacket.PairRecord],
        medianInputs: SignOffPacket.MedianInputs,
        medianOutputs: SignOffPacket.MedianOutputs,
        decision: String,
        failingGateReasons: [String]
    ) -> String {
        let encoder = JSONEncoder()
        encoder.outputFormatting = .sortedKeys
        encoder.dateEncodingStrategy = .iso8601

        var hashInput = Data()

        if let pairData = try? encoder.encode(pairRecords) {
            hashInput.append(pairData)
        }
        if let medianInData = try? encoder.encode(medianInputs) {
            hashInput.append(medianInData)
        }
        if let medianOutData = try? encoder.encode(medianOutputs) {
            hashInput.append(medianOutData)
        }
        if let decisionData = decision.data(using: .utf8) {
            hashInput.append(decisionData)
        }
        if let reasonsData = try? encoder.encode(failingGateReasons) {
            hashInput.append(reasonsData)
        }

        let digest = SHA256.hash(data: hashInput)
        return digest.map { String(format: "%02x", $0) }.joined()
    }
}

// MARK: - SignOffPacket

/// Self-contained, Codable evidence packet that captures all inputs and outputs
/// needed to replay a GO/HOLD sign-off decision.
struct SignOffPacket: Codable, Sendable {

    // Packet metadata
    let packetVersion: String
    let exportedAt: Date
    let evaluatorVersion: String

    // Cohort identification
    let cohortID: UUID
    let cohortLabel: String
    let cohortStatus: String
    let cohortMembers: [CohortMember]

    // Per-pair benchmark records
    let pairRecords: [PairRecord]

    // Median computation inputs and outputs
    let medianInputs: MedianInputs
    let medianOutputs: MedianOutputs

    // Decision
    let decision: String
    let failingGateReasons: [String]

    // Counts
    let pairCount: Int
    let happyPathCount: Int
    let recoveredCount: Int

    // Traceability
    let snapshotID: UUID
    let payloadChecksum: String

    // MARK: - Nested Types

    struct CohortMember: Codable, Sendable {
        let ideaIdentifier: String
        let title: String
        let repositoryID: String
    }

    struct PairRecord: Codable, Sendable {
        let pairID: UUID
        let ideaIdentifier: String
        let repositoryID: String
        let createdAt: Date
        let manualRecord: ExecutionRecordSnapshot?
        let appDrivenRecord: ExecutionRecordSnapshot?
    }

    struct ExecutionRecordSnapshot: Codable, Sendable {
        let recordID: UUID
        let executionMode: String
        let linkedRunID: UUID?
        let startedAt: Date
        let completedAt: Date?
        let totalOrchestrationTimeSeconds: Double?
        let timeToProposalApprovalSeconds: Double?
        let timeToImplementationApprovalSeconds: Double?
        let timeToFinalReleaseDecisionSeconds: Double?
        let terminalOutcome: String
        let artifactLinks: [ArtifactLinkSnapshot]
        let notes: [String]?
    }

    struct ArtifactLinkSnapshot: Codable, Sendable {
        let artifactID: UUID
        let name: String
        let role: String
    }

    struct MedianInputs: Codable, Sendable {
        let manualOrchestrationTimes: [Double]
        let appOrchestrationTimes: [Double]
        let proposalApprovalTimes: [Double]
        let implementationApprovalTimes: [Double]
        let releaseDecisionTimes: [Double]
    }

    struct MedianOutputs: Codable, Sendable {
        let medianManualOrchestrationSeconds: Double?
        let medianAppOrchestrationSeconds: Double?
        let medianImprovementPercent: Double?
        let medianProposalApprovalSeconds: Double?
        let medianImplementationApprovalSeconds: Double?
        let medianReleaseDecisionSeconds: Double?
    }
}
