import Foundation

#if os(macOS)
import AppKit
#endif

protocol ManagedGooseServerControlling: AnyObject {
    func stopManagedServer()
}

@MainActor
class AppTerminationCoordinator: NSObject {
    var gooseServerManager: ManagedGooseServerControlling?

    func prepareForTermination() {
        gooseServerManager?.stopManagedServer()
    }
}

#if os(macOS)
extension AppTerminationCoordinator: NSApplicationDelegate {
    func applicationWillTerminate(_ notification: Notification) {
        prepareForTermination()
    }
}
#endif
