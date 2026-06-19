import Foundation

// Note: ToolchainCacheScope and ToolchainCachePolicy are defined in AgentCatalog.swift.

// MARK: - P066: Mapping State

/// P066: Semantic state of toolchain mapping for a given agent execution.
enum ToolchainMappingState: String, Codable, Sendable {
    case active
    case disabledByPolicy = "disabled_by_policy"
    case policyAbsent = "policy_absent"
    case unsupportedFamily = "unsupported_family"
    case setupFailed = "setup_failed"
    case queueTimeout = "queue_timeout"
    case legacyRowUnavailable = "legacy_row_unavailable"
}

/// P066: Inactive reason when mapping is not active.
enum ToolchainMappingInactiveReason: String, Codable, Sendable {
    case policyDisabled = "policy_disabled"
    case policyAbsent = "policy_absent"
    case unsupportedFamily = "unsupported_family"
    case legacyRow = "legacy_row"
}

/// P066: Policy provenance for diagnostics.
enum ToolchainMappingPolicySource: String, Codable, Sendable {
    case runplanSnapshot = "runplan_snapshot"
    case synthesizedLegacy = "synthesized_legacy"
}

/// P066: Decoded toolchain mapping diagnostics document.
/// Consumed by SwiftUI and report surfaces — never raw JSON.
struct ToolchainMappingDiagnostics: Codable, Sendable {
    let version: Int
    let mappingState: ToolchainMappingState
    let mappingEnabled: Bool
    let inactiveReason: ToolchainMappingInactiveReason?
    let policySource: ToolchainMappingPolicySource
    let policyVersion: Int?
    let providerFamily: String

    enum CodingKeys: String, CodingKey {
        case version
        case mappingState = "mapping_state"
        case mappingEnabled = "mapping_enabled"
        case inactiveReason = "inactive_reason"
        case policySource = "policy_source"
        case policyVersion = "policy_version"
        case providerFamily = "provider_family"
    }
}

// MARK: - P066: ToolchainMappingReadAdapter

/// P066: The only sanctioned entry point for decoding toolchain cache policy
/// and mapping diagnostics in the Swift operator-facing layer.
///
/// Rules:
/// - Frozen-snapshot compatibility is validated before any decode.
/// - Legacy snapshots (no format version, no policy) decode as policy_absent.
/// - Mixed-version snapshots (policy present but version absent/unsupported)
///   fail deterministically as frozen_snapshot_contract_incompatible.
/// - Unknown enum values in policy or diagnostics fail decode rather than
///   silently coercing to defaults.
///
/// Consumers: RunPlanCompiler, ExecutionService, RunReportBuilder,
/// RunComparisonService. None of these may decode AgentCatalog for
/// toolchain-cache-policy-aware behavior via try? fallback.
enum ToolchainMappingReadAdapter {
    private static let policyJSONKeys: Set<String> = [
        "version",
        "enabled",
        "xcode_scope",
        "go_scope",
    ]

    // MARK: - Frozen-snapshot compatibility gate

    /// P066 format version value.
    nonisolated static let p066FormatVersion: Int = 1

    /// Validate a frozen catalog snapshot format version.
    ///
    /// - Returns: `true` for P066-aware snapshots (version = 1),
    ///            `false` for legacy_v0 (version absent, no policy).
    /// - Throws: `ToolchainSnapshotCompatibilityError` for incompatible snapshots.
    static func validateCatalogSnapshotFormatVersion(
        version: Int?,
        hasToolchainCachePolicy: Bool
    ) throws -> Bool {
        switch (version, hasToolchainCachePolicy) {
        case (nil, false):
            // Legacy v0: no version, no policy → policy_absent, safe to read.
            return false
        case (nil, true):
            throw ToolchainSnapshotCompatibilityError.missingVersionWithPolicy
        case (let v?, _) where v > p066FormatVersion:
            throw ToolchainSnapshotCompatibilityError.unsupportedVersion(v)
        case (1, _):
            return true
        case (let v?, _):
            throw ToolchainSnapshotCompatibilityError.unsupportedVersion(v)
        }
    }

    // MARK: - Policy decoding

    /// Decode a toolchain cache policy from an agent catalog entry's JSON
    /// representation. Returns nil (policy_absent) when the field is absent.
    /// Throws on unknown keys or unsupported enum values.
    static func decodePolicyFromCatalogJSON(_ json: String?) throws -> ToolchainCachePolicy? {
        guard let json = json, !json.isEmpty else {
            return nil
        }
        guard let data = json.data(using: .utf8) else {
            throw ToolchainMappingDecodeError.invalidUTF8
        }
        try rejectUnknownPolicyKeys(in: data)
        let decoder = JSONDecoder()
        return try decoder.decode(ToolchainCachePolicy.self, from: data)
    }

    private static func rejectUnknownPolicyKeys(in data: Data) throws {
        let rawObject = try JSONSerialization.jsonObject(with: data)
        guard let object = rawObject as? [String: Any] else {
            return
        }
        let unknownKeys = object.keys
            .filter { !policyJSONKeys.contains($0) }
            .sorted()
        if !unknownKeys.isEmpty {
            throw ToolchainMappingDecodeError.unknownKeys(unknownKeys)
        }
    }

    // MARK: - Diagnostics synthesis

    /// Synthesize a legacy_row_unavailable diagnostics sentinel for pre-P066
    /// NULL rows. Used by report and readback surfaces.
    static func legacyRowSentinel() -> ToolchainMappingDiagnostics {
        ToolchainMappingDiagnostics(
            version: 1,
            mappingState: .legacyRowUnavailable,
            mappingEnabled: false,
            inactiveReason: .legacyRow,
            policySource: .synthesizedLegacy,
            policyVersion: nil,
            providerFamily: "unknown"
        )
    }

    /// Synthesize a policy_absent diagnostics document for agents without a
    /// toolchain_cache_policy block.
    static func policyAbsentSentinel(providerFamily: String) -> ToolchainMappingDiagnostics {
        ToolchainMappingDiagnostics(
            version: 1,
            mappingState: .policyAbsent,
            mappingEnabled: false,
            inactiveReason: .policyAbsent,
            policySource: .runplanSnapshot,
            policyVersion: nil,
            providerFamily: providerFamily
        )
    }

    /// Synthesize a disabled_by_policy diagnostics document for agents with
    /// toolchain_cache_policy.enabled = false.
    static func disabledByPolicySentinel(
        providerFamily: String,
        policyVersion: Int
    ) -> ToolchainMappingDiagnostics {
        ToolchainMappingDiagnostics(
            version: 1,
            mappingState: .disabledByPolicy,
            mappingEnabled: false,
            inactiveReason: .policyDisabled,
            policySource: .runplanSnapshot,
            policyVersion: policyVersion,
            providerFamily: providerFamily
        )
    }

    /// Decode stored diagnostics JSON or synthesize a legacy sentinel.
    /// Never returns nil — callers always receive a typed document.
    static func decodeDiagnosticsOrSynthesize(_ json: String?) -> ToolchainMappingDiagnostics {
        guard let json = json, !json.isEmpty else {
            return legacyRowSentinel()
        }
        guard let data = json.data(using: .utf8),
              let decoded = try? JSONDecoder().decode(ToolchainMappingDiagnostics.self, from: data)
        else {
            return legacyRowSentinel()
        }
        return decoded
    }
}

// MARK: - P066: Error Types

enum ToolchainSnapshotCompatibilityError: Error, LocalizedError {
    case missingVersionWithPolicy
    case unsupportedVersion(Int)

    var errorDescription: String? {
        switch self {
        case .missingVersionWithPolicy:
            return "frozen_snapshot_contract_incompatible: catalog snapshot contains " +
                   "toolchain_cache_policy but omits catalog_snapshot_format_version"
        case .unsupportedVersion(let v):
            return "frozen_snapshot_contract_incompatible: catalog snapshot requires " +
                   "format version \(v) but this reader only supports version " +
                   "\(ToolchainMappingReadAdapter.p066FormatVersion)"
        }
    }
}

enum ToolchainMappingDecodeError: Error {
    case invalidUTF8
    case unknownKeys([String])
}
