import Foundation

// MARK: - Proposal 013 Layer M: Output Contract Resolver V2

/// Canonical runtime reader that resolves typed contract schemas from AgentCatalog.contracts.
/// Replaces the hardcoded fallback branches in OutputContractResolver.
///
/// Rules (§4.2):
/// 1. OutputContractSchemaV2 is derived from AgentCatalog.contracts.
/// 2. This is the ONLY runtime reader used by orchestrator, artifact manager, report builder, and recovery.
/// 3. No hardcoded outputName -> contractID branches.
/// 4. Contract ID resolution is fully catalog-driven.
enum OutputContractResolverV2 {

    // MARK: - Schema Resolution

    /// Resolve the typed contract schema for an output.
    /// Returns nil if no contract is declared for this output.
    static func resolveSchema(
        for outputName: String,
        agent: ResolvedAgent,
        catalog: AgentCatalog?
    ) -> OutputContractSchemaV2? {
        guard let contractID = resolveContractID(for: outputName, agent: agent, catalog: catalog),
              let catalog,
              let contract = catalog.contracts[contractID] else {
            return nil
        }
        let mode = inferValidationMode(contractID: contractID, contract: contract)
        return OutputContractSchemaV2.from(
            contractID: contractID,
            contract: contract,
            validationMode: mode,
            outputName: outputName
        )
    }

    // MARK: - Contract ID Resolution (catalog-driven, no hardcoded fallbacks)

    /// Resolve contract ID purely from catalog declarations.
    /// Priority:
    /// 1. Agent's explicit outputContract field matches the output
    /// 2. Catalog contains a contract named exactly as the output
    /// 3. Catalog contains a versioned contract (outputName + "_v1")
    /// 4. Agent's explicit outputContract (fallback for all outputs of this agent)
    static func resolveContractID(
        for outputName: String,
        agent: ResolvedAgent,
        catalog: AgentCatalog?
    ) -> String? {
        guard let catalog else { return agent.outputContract }

        // 1. Exact match: catalog has a contract named exactly as the output
        if catalog.contracts[outputName] != nil {
            return outputName
        }

        // 2. Versioned match: outputName_v1
        let versioned = "\(outputName)_v1"
        if catalog.contracts[versioned] != nil {
            return versioned
        }

        // 3. Agent-level explicit contract
        if let explicit = agent.outputContract {
            // Check the explicit contract exists in catalog
            if catalog.contracts[explicit] != nil {
                return explicit
            }
        }

        // 4. Infer from output name patterns using catalog contract stems
        // e.g., "proposal_review_po" matches "proposal_review_v1" via stem "proposal_review"
        for (contractID, _) in catalog.contracts {
            if contractStemMatches(contractID: contractID, outputName: outputName) {
                return contractID
            }
        }

        return nil
    }

    // MARK: - Expected Outputs

    /// Returns the expected output names for a task/agent.
    static func expectedOutputs(for task: AgentTask, agent: ResolvedAgent) -> [String] {
        task.outputs ?? agent.outputs
    }

    // MARK: - Validation

    /// Validate structured outputs against their resolved contracts.
    /// Returns validation results per output name.
    static func validateOutputs(
        _ outputs: [String: Data],
        agent: ResolvedAgent,
        catalog: AgentCatalog?
    ) -> [String: OutputValidationResult] {
        var results: [String: OutputValidationResult] = [:]

        for (name, data) in outputs {
            guard let schema = resolveSchema(for: name, agent: agent, catalog: catalog) else {
                // No contract declared — validation passes by default
                results[name] = OutputValidationResult(
                    outputName: name,
                    contractID: nil,
                    status: .noContractDeclared,
                    missingFields: [],
                    validationError: nil,
                    rawPayloadSize: data.count
                )
                continue
            }

            results[name] = validateSingleOutput(
                name: name,
                data: data,
                schema: schema
            )
        }

        return results
    }

    // MARK: - Single Output Validation

    private static func validateSingleOutput(
        name: String,
        data: Data,
        schema: OutputContractSchemaV2
    ) -> OutputValidationResult {
        switch schema.validationMode {
        case .humanOnly:
            // Human-only: no machine validation
            return OutputValidationResult(
                outputName: name,
                contractID: schema.contractID,
                status: .passed,
                missingFields: [],
                validationError: nil,
                rawPayloadSize: data.count
            )

        case .strictStructured:
            return validateStructured(
                name: name,
                data: data,
                schema: schema,
                requireMachineOnly: true
            )

        case .structuredWithHumanCompanion:
            return validateStructured(
                name: name,
                data: data,
                schema: schema,
                requireMachineOnly: false
            )
        }
    }

    private static func validateStructured(
        name: String,
        data: Data,
        schema: OutputContractSchemaV2,
        requireMachineOnly: Bool
    ) -> OutputValidationResult {
        guard schema.machineFormat == .json else {
            // Non-JSON structured outputs: just check non-empty
            if data.isEmpty {
                return OutputValidationResult(
                    outputName: name,
                    contractID: schema.contractID,
                    status: .failed,
                    missingFields: [],
                    validationError: "Output is empty",
                    rawPayloadSize: 0
                )
            }
            return OutputValidationResult(
                outputName: name,
                contractID: schema.contractID,
                status: .passed,
                missingFields: [],
                validationError: nil,
                rawPayloadSize: data.count
            )
        }

        // JSON validation: parse and check required fields
        if let jsonObject = try? JSONSerialization.jsonObject(with: data),
           let dict = jsonObject as? [String: Any] {
            // Valid JSON — check required fields
            let missingFields = schema.requiredFields.filter { dict[$0] == nil }
            if missingFields.isEmpty {
                return OutputValidationResult(
                    outputName: name,
                    contractID: schema.contractID,
                    status: .passed,
                    missingFields: [],
                    validationError: nil,
                    rawPayloadSize: data.count
                )
            } else if requireMachineOnly {
                // strict_structured: missing fields is a hard failure
                return OutputValidationResult(
                    outputName: name,
                    contractID: schema.contractID,
                    status: .failed,
                    missingFields: missingFields,
                    validationError: "Missing required fields: \(missingFields.joined(separator: ", "))",
                    rawPayloadSize: data.count
                )
            } else {
                // structured_with_human_companion: JSON present but incomplete — still pass,
                // the companion human output covers the gap (§4.3 Rule 2)
                return OutputValidationResult(
                    outputName: name,
                    contractID: schema.contractID,
                    status: .passed,
                    missingFields: missingFields,
                    validationError: nil,
                    rawPayloadSize: data.count
                )
            }
        }

        // Not valid JSON
        if requireMachineOnly {
            // strict_structured: non-JSON is a hard failure
            return OutputValidationResult(
                outputName: name,
                contractID: schema.contractID,
                status: .failed,
                missingFields: schema.requiredFields,
                validationError: "Output is not valid JSON or not a JSON object",
                rawPayloadSize: data.count
            )
        }

        // structured_with_human_companion (§4.3 Rule 2):
        // If not JSON, accept non-empty human-readable text as the companion format.
        // Per proposal: "If the app wants proposal reviews as markdown, the contract must say markdown."
        // Since validation mode is structured_with_human_companion, markdown IS acceptable.
        if let text = String(data: data, encoding: .utf8),
           !text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
            return OutputValidationResult(
                outputName: name,
                contractID: schema.contractID,
                status: .passed,
                missingFields: [],
                validationError: nil,
                rawPayloadSize: data.count
            )
        }

        // Empty or unreadable output
        return OutputValidationResult(
            outputName: name,
            contractID: schema.contractID,
            status: .failed,
            missingFields: schema.requiredFields,
            validationError: "Output is neither valid JSON nor non-empty human-readable text",
            rawPayloadSize: data.count
        )
    }

    // MARK: - Stem Matching

    /// Match contract ID stem to output name.
    /// e.g., "proposal_review_v1" stem is "proposal_review", matches "proposal_review_po".
    private static func contractStemMatches(contractID: String, outputName: String) -> Bool {
        // Extract stem by removing version suffix (_v1, _v2, etc.)
        guard let stemRange = contractID.range(of: #"_v\d+$"#, options: .regularExpression) else {
            return false
        }
        let stem = String(contractID[..<stemRange.lowerBound])
        return outputName.hasPrefix(stem) && outputName != contractID
    }

    // MARK: - Validation Mode Inference

    private static func inferValidationMode(
        contractID: String,
        contract: ArtifactContract
    ) -> ValidationMode {
        // JSON contracts default to strict structured
        if contract.format == "json" {
            return .strictStructured
        }
        // Markdown/other: human only
        return .humanOnly
    }
}

// MARK: - Output Validation Result

/// Result of validating a single output against its contract.
nonisolated struct OutputValidationResult: Codable, Sendable, Equatable {
    let outputName: String
    let contractID: String?
    let status: OutputValidationStatus
    let missingFields: [String]
    let validationError: String?
    let rawPayloadSize: Int
}

nonisolated enum OutputValidationStatus: String, Codable, Sendable, Equatable {
    case passed
    case failed
    case noContractDeclared
}
