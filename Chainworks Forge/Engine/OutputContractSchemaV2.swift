import Foundation

// MARK: - Proposal 013 Layer M: Output Contract Schema V2

/// Typed schema derived from the existing catalog-backed contract truth.
/// Includes machine format, human-readable companion format, and validation mode.
/// This is the canonical contract representation used by all runtime components.
struct OutputContractSchemaV2: Codable, Sendable, Equatable {
    /// Unique contract identifier (e.g., "proposal_review_v1").
    let contractID: String
    /// Machine-readable output format.
    let machineFormat: ContractFormat
    /// Optional human-readable companion format.
    let humanFormat: ContractFormat?
    /// Validation mode governing how outputs are checked.
    let validationMode: ValidationMode
    /// Required fields in the machine-readable output.
    let requiredFields: [String]
    /// Name of the raw artifact before normalization.
    let rawArtifactName: String?
    /// Name of the normalized artifact after validation.
    let normalizedArtifactName: String?

    /// How validation failures should be treated.
    var failureDisposition: FailureDisposition { .blockStage }
}

// MARK: - Contract Format

enum ContractFormat: String, Codable, Sendable, Equatable {
    case json
    case markdown
    case diff
    case yaml
    case plaintext
}

// MARK: - Validation Mode (§4.3)

/// Validation modes per Proposal 013 §4.3.
enum ValidationMode: String, Codable, Sendable, Equatable {
    /// Machine payload must be strictly valid. No prose fallback accepted.
    case strictStructured = "strict_structured"
    /// Machine payload required, plus a human-readable companion artifact.
    case structuredWithHumanCompanion = "structured_with_human_companion"
    /// Human-readable output only. No machine validation.
    case humanOnly = "human_only"
}

// MARK: - Failure Disposition

enum FailureDisposition: String, Codable, Sendable, Equatable {
    /// Block the stage and require operator recovery.
    case blockStage = "block_stage"
    /// Warn but allow the stage to proceed.
    case warnAndContinue = "warn_and_continue"
}

// MARK: - Schema Migration from ArtifactContract

extension OutputContractSchemaV2 {
    /// Derive a V2 schema from the existing catalog `ArtifactContract`.
    /// This is the canonical migration path — no second contract authority.
    static func from(
        contractID: String,
        contract: ArtifactContract,
        validationMode: ValidationMode = .strictStructured,
        outputName: String? = nil
    ) -> OutputContractSchemaV2 {
        let machineFormat = ContractFormat(rawValue: contract.format) ?? .json
        // Proposal 013 §4.3: For structured_with_human_companion, declare both formats
        let humanFormat: ContractFormat? = (validationMode == .structuredWithHumanCompanion) ? .markdown : nil
        let rawName = outputName.map { "\($0)_raw" }
        let normalizedName = outputName

        return OutputContractSchemaV2(
            contractID: contractID,
            machineFormat: machineFormat,
            humanFormat: humanFormat,
            validationMode: validationMode,
            requiredFields: contract.requiredFields,
            rawArtifactName: rawName,
            normalizedArtifactName: normalizedName
        )
    }
}
