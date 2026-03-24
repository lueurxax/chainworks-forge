import Foundation

struct ProviderSettings: Codable, Equatable, Sendable {
    var configuredProviders: [ConfiguredProvider]
    var preferredProviderIDsByFamily: [String: UUID]
    var notificationOnProviderFailure: Bool
    var runStartRequiresCleanPreflight: Bool

    static let empty = ProviderSettings(
        configuredProviders: [],
        preferredProviderIDsByFamily: [:],
        notificationOnProviderFailure: true,
        runStartRequiresCleanPreflight: true
    )
}
