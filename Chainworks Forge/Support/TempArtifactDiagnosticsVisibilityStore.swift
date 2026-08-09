import Foundation

/// P089: CFPreferences/UserDefaults-backed local operator preference for the temp
/// artifact diagnostics surface. Domain: com.chainworks.forge, key:
/// TempArtifactDiagnosticsVisible. An absent key defaults to false (hidden).
///
/// This store is only one half of the visibility decision: it is app-local and
/// cannot itself reflect the daemon's `CHAINWORKS_TEMP_ARTIFACT_INVENTORY_MODE`.
/// `TempArtifactInventoryView` composes this preference with the backend `mode`
/// reported on `TempArtifactInventoryResponse` (via
/// `TempArtifactInventoryViewModel.isBackendAuthorizedForVisibleSurface`) so a
/// stale/true local preference can never keep the surface visible once the
/// backend is confirmed to be in `hidden_readback` or `disabled` mode.
final class TempArtifactDiagnosticsVisibilityStore: @unchecked Sendable {
    static let domain = "com.chainworks.forge"
    static let visibilityKey = "TempArtifactDiagnosticsVisible"

    private let defaults: UserDefaults

    /// Production: uses CFPreferences domain com.chainworks.forge. Inject a suiteName-matched
    /// UserDefaults in tests to isolate state from the host environment.
    init(defaults: UserDefaults = UserDefaults(suiteName: domain) ?? .standard) {
        self.defaults = defaults
    }

    var isVisible: Bool {
        defaults.bool(forKey: Self.visibilityKey)
    }

    func setVisible(_ visible: Bool) {
        defaults.set(visible, forKey: Self.visibilityKey)
    }
}
