import CryptoKit
import Foundation

nonisolated struct CodexModelVariantPolicy: Equatable, Sendable {
    struct Variant: Equatable, Sendable {
        let modelID: String
        let displayName: String
        let allowedEfforts: Set<String>
    }

    let variantsByModelID: [String: Variant]
    let knownEfforts: Set<String>
}

nonisolated enum CodexModelVariantPolicyAvailability: Equatable, Sendable {
    case available(CodexModelVariantPolicy)
    case unavailable(CodexModelVariantPolicyUnavailableReason)
}

nonisolated enum CodexModelVariantPolicyUnavailableReason: String, Equatable, Sendable {
    case missingResource = "missing_resource"
    case unreadableResource = "unreadable_resource"
    case policyBytesMismatch = "policy_bytes_mismatch"
    case policySchemaInvalid = "policy_schema_invalid"
}

nonisolated enum CodexModelVariantPolicyLoader {
    static let resourceName = "codex-model-variant-matrix.v1"
    static let resourceExtension = "json"
    static let expectedByteCount = 1_479
    static let expectedSHA256 = "b6ad3f2047466a34da42241eae6b790f60bb835d9e6826cb77b51eb3fc558911"

    nonisolated static func loadBundled(
        bundle: Bundle = .main
    ) -> CodexModelVariantPolicyAvailability {
        loadResource(
            resourceURL: bundle.url(forResource: resourceName, withExtension: resourceExtension),
            readData: { try Data(contentsOf: $0, options: [.mappedIfSafe]) }
        )
    }

    nonisolated static func loadResource(
        resourceURL: URL?,
        readData: (URL) throws -> Data
    ) -> CodexModelVariantPolicyAvailability {
        guard let resourceURL else {
            return .unavailable(.missingResource)
        }
        guard let data = try? readData(resourceURL) else {
            return .unavailable(.unreadableResource)
        }
        return load(data: data)
    }

    nonisolated static func load(data: Data?) -> CodexModelVariantPolicyAvailability {
        guard let data else {
            return .unavailable(.missingResource)
        }
        guard data.count == expectedByteCount,
              data.last == 0x0A,
              sha256(data) == expectedSHA256
        else {
            return .unavailable(.policyBytesMismatch)
        }
        guard let decoded = try? JSONDecoder().decode(PolicyDocument.self, from: data),
              decoded.schemaVersion == 1,
              decoded.policyID == "codex_model_variant_matrix_v1",
              decoded.provider == "codex_acp",
              decoded.canonicalProvider == "codex"
        else {
            return .unavailable(.policySchemaInvalid)
        }

        var variants: [String: CodexModelVariantPolicy.Variant] = [:]
        var knownEfforts = Set<String>()
        for row in decoded.variants {
            guard !row.modelID.isEmpty,
                  row.modelID == row.modelID.trimmingCharacters(in: .whitespacesAndNewlines),
                  !row.displayName.isEmpty,
                  row.displayName == row.displayName.trimmingCharacters(in: .whitespacesAndNewlines),
                  variants[row.modelID] == nil,
                  !row.allowedEfforts.isEmpty,
                  Set(row.allowedEfforts).count == row.allowedEfforts.count
            else {
                return .unavailable(.policySchemaInvalid)
            }
            let efforts = Set(row.allowedEfforts)
            knownEfforts.formUnion(efforts)
            variants[row.modelID] = CodexModelVariantPolicy.Variant(
                modelID: row.modelID,
                displayName: row.displayName,
                allowedEfforts: efforts
            )
        }
        guard !variants.isEmpty else { return .unavailable(.policySchemaInvalid) }
        return .available(
            CodexModelVariantPolicy(
                variantsByModelID: variants,
                knownEfforts: knownEfforts
            )
        )
    }

    private nonisolated static func sha256(_ data: Data) -> String {
        SHA256.hash(data: data).map { String(format: "%02x", $0) }.joined()
    }

    private struct PolicyDocument: Decodable {
        let schemaVersion: Int
        let policyID: String
        let provider: String
        let canonicalProvider: String
        let variants: [VariantDocument]

        enum CodingKeys: String, CodingKey {
            case schemaVersion = "schema_version"
            case policyID = "policy_id"
            case provider
            case canonicalProvider = "canonical_provider"
            case variants
        }
    }

    private struct VariantDocument: Decodable {
        let modelID: String
        let displayName: String
        let allowedEfforts: [String]

        enum CodingKeys: String, CodingKey {
            case modelID = "model_id"
            case displayName = "display_name"
            case allowedEfforts = "allowed_efforts"
        }
    }
}

nonisolated enum CodexPlannedAssignmentPresentation: Equatable, Sendable {
    case planned(
        variantToken: String,
        visualSuffix: String,
        fullAccessibilityValue: String
    )
    case unavailable
    case nonCodex(existingValue: String)

    nonisolated var variantToken: String? {
        guard case let .planned(variantToken, _, _) = self else { return nil }
        return variantToken
    }

    nonisolated var visualSuffix: String {
        switch self {
        case let .planned(_, visualSuffix, _):
            return visualSuffix
        case .unavailable:
            return "Planned assignment unavailable"
        case let .nonCodex(existingValue):
            return existingValue
        }
    }

    nonisolated var fullAccessibilityValue: String {
        switch self {
        case let .planned(_, _, fullAccessibilityValue):
            return fullAccessibilityValue
        case .unavailable:
            return "Planned assignment unavailable"
        case let .nonCodex(existingValue):
            return existingValue
        }
    }

    nonisolated var isCodexPlannedLine: Bool {
        switch self {
        case .planned, .unavailable: return true
        case .nonCodex: return false
        }
    }
}

nonisolated enum CodexPlannedAssignmentFormatter {
    nonisolated static func presentation(
        provider: String,
        model: String?,
        effort: String?,
        policy: CodexModelVariantPolicyAvailability = CodexModelVariantPolicyLoader.loadBundled()
    ) -> CodexPlannedAssignmentPresentation {
        guard provider == "codex" else {
            let existingValue = [provider, model, effort]
                .compactMap { value -> String? in
                    guard let value else { return nil }
                    let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines)
                    return trimmed.isEmpty ? nil : trimmed
                }
                .joined(separator: " · ")
            return .nonCodex(existingValue: existingValue)
        }
        guard case let .available(policy) = policy,
              let model,
              isSafeToken(model),
              effort.map(isSafeToken) ?? true
        else {
            return .unavailable
        }

        if let variant = policy.variantsByModelID[model] {
            guard let effort else {
                return .planned(
                    variantToken: variant.displayName,
                    visualSuffix: "\(model) · Planned effort not recorded",
                    fullAccessibilityValue: "Codex · \(variant.displayName) · \(model) · Planned effort not recorded"
                )
            }
            guard variant.allowedEfforts.contains(effort) else { return .unavailable }
            return .planned(
                variantToken: variant.displayName,
                visualSuffix: "\(model) · \(effort) · planned",
                fullAccessibilityValue: "Codex · \(variant.displayName) · \(model) · \(effort) · planned"
            )
        }

        guard model == "gpt-5.6" else { return .unavailable }
        guard let effort else {
            return .planned(
                variantToken: "Codex",
                visualSuffix: "gpt-5.6 · Planned effort not recorded",
                fullAccessibilityValue: "Codex · gpt-5.6 · Planned effort not recorded"
            )
        }
        guard policy.knownEfforts.contains(effort) else { return .unavailable }
        return .planned(
            variantToken: "Codex",
            visualSuffix: "gpt-5.6 · \(effort) · planned",
            fullAccessibilityValue: "Codex · gpt-5.6 · \(effort) · planned"
        )
    }

    private nonisolated static func isSafeToken(_ value: String) -> Bool {
        guard !value.isEmpty,
              value.utf8.count <= 256,
              value == value.trimmingCharacters(in: .whitespacesAndNewlines),
              !value.contains("·"),
              !["\\n", "\\r", "\\t", "\\u", "\\x"].contains(where: { value.contains($0) })
        else {
            return false
        }
        return value.unicodeScalars.allSatisfy { scalar in
            switch scalar.properties.generalCategory {
            case .control, .format, .lineSeparator, .paragraphSeparator:
                return false
            default:
                return true
            }
        }
    }
}

nonisolated struct P031PlannedAssignmentCandidate: Equatable, Sendable {
    let agentID: String
    let provider: String
    let model: String?
    let effort: String?
}

nonisolated enum P031PlannedAssignmentMatcher {
    nonisolated static func presentation(
        agentID: String,
        provider: String,
        model: String?,
        candidates: [P031PlannedAssignmentCandidate],
        policy: CodexModelVariantPolicyAvailability = CodexModelVariantPolicyLoader.loadBundled()
    ) -> CodexPlannedAssignmentPresentation {
        guard provider == "codex" else {
            return CodexPlannedAssignmentFormatter.presentation(
                provider: provider,
                model: model,
                effort: nil,
                policy: policy
            )
        }
        let matches = candidates.filter {
            $0.agentID == agentID && $0.provider == provider && $0.model == model
        }
        guard matches.count == 1, let match = matches.first else { return .unavailable }
        return CodexPlannedAssignmentFormatter.presentation(
            provider: match.provider,
            model: match.model,
            effort: match.effort,
            policy: policy
        )
    }
}

nonisolated enum P036PlannedAssignmentAccessibility {
    static func overviewLabel(
        agentTitle: String,
        status: String,
        presentation: CodexPlannedAssignmentPresentation,
        stage: String?,
        task: String?,
        session: String?,
        eventCount: Int
    ) -> String {
        joinedNonempty([
            agentTitle,
            status,
            presentation.fullAccessibilityValue,
            stage,
            task,
            session.map { "session \($0)" },
            "\(eventCount) events",
        ])
    }

    static func stageOccurrenceLabel(
        agentTitle: String,
        task: String,
        status: String,
        presentation: CodexPlannedAssignmentPresentation,
        executionCount: String?
    ) -> String {
        joinedNonempty([
            agentTitle,
            task,
            status,
            presentation.fullAccessibilityValue,
            executionCount,
        ])
    }

    private static func joinedNonempty(_ values: [String?]) -> String {
        values
            .compactMap { value -> String? in
                let trimmed = value?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
                return trimmed.isEmpty ? nil : trimmed
            }
            .joined(separator: ", ")
    }
}
