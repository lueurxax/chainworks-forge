import Foundation

// MARK: - Proposal 013 Layer M: Proposal Review Contract Adapter

/// Aligns proposal-review agents and runtime so the declared contract matches
/// the produced artifact format. This is the first mandatory adopter (§4.4).
///
/// Ensures proposal_review_po, proposal_review_ux, proposal_review_ui,
/// and proposal_review_architect all have one coherent contract across:
/// - agents.yaml
/// - runtime validation
/// - artifact persistence
/// - run reports
/// - blocked-run recovery UI
enum ProposalReviewContractAdapter {

    /// All proposal review output names that share the review contract.
    static let reviewOutputNames: Set<String> = [
        "proposal_review_po",
        "proposal_review_ux",
        "proposal_review_ui",
        "proposal_review_architect"
    ]

    /// The canonical contract ID for individual reviews.
    static let reviewContractID = "proposal_review_v1"

    /// The canonical contract ID for the aggregated review summary.
    static let summaryContractID = "proposal_review_summary_v1"

    /// Check if an output name is a proposal review output.
    static func isReviewOutput(_ outputName: String) -> Bool {
        reviewOutputNames.contains(outputName)
    }

    /// Check if an output name is the review summary.
    static func isReviewSummary(_ outputName: String) -> Bool {
        outputName == "proposal_review_summary"
    }

    /// Resolve the contract schema for a proposal review output.
    /// Proposal-review outputs are strict JSON machine artifacts.
    static func resolveReviewSchema(for outputName: String, catalog: AgentCatalog?) -> OutputContractSchemaV2? {
        guard isReviewOutput(outputName) || isReviewSummary(outputName) else { return nil }

        let contractID = isReviewSummary(outputName) ? summaryContractID : reviewContractID
        guard let catalog,
              let contract = catalog.contracts[contractID] else { return nil }

        return OutputContractSchemaV2.from(
            contractID: contractID,
            contract: contract,
            validationMode: .strictStructured
        )
    }

    /// Validate a proposal review output.
    /// Proposal-review outputs must be top-level JSON objects with all required fields.
    static func validateReviewOutput(
        outputName: String,
        data: Data,
        catalog: AgentCatalog?
    ) -> OutputValidationResult {
        guard let schema = resolveReviewSchema(for: outputName, catalog: catalog) else {
            return OutputValidationResult(
                outputName: outputName,
                contractID: nil,
                status: .noContractDeclared,
                missingFields: [],
                validationError: nil,
                rawPayloadSize: data.count
            )
        }

        // Try JSON first
        if let jsonObject = try? JSONSerialization.jsonObject(with: data),
           let dict = jsonObject as? [String: Any] {
            let missingFields = schema.requiredFields.filter { dict[$0] == nil }
            if missingFields.isEmpty {
                return OutputValidationResult(
                    outputName: outputName,
                    contractID: schema.contractID,
                    status: .passed,
                    missingFields: [],
                    validationError: nil,
                    rawPayloadSize: data.count
                )
            } else {
                return OutputValidationResult(
                    outputName: outputName,
                    contractID: schema.contractID,
                    status: .failed,
                    missingFields: missingFields,
                    validationError: "JSON output missing fields: \(missingFields.joined(separator: ", "))",
                    rawPayloadSize: data.count
                )
            }
        }

        return OutputValidationResult(
            outputName: outputName,
            contractID: schema.contractID,
            status: .failed,
            missingFields: schema.requiredFields,
            validationError: "Output is not valid JSON or not a JSON object",
            rawPayloadSize: data.count
        )
    }
}
