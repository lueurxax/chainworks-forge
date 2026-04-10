import Foundation

#if os(macOS)
import AppKit
#endif

@MainActor
protocol ExecutionTerminationControlling: AnyObject {
    func prepareForTermination()
}

@MainActor
class AppTerminationCoordinator: NSObject {
    weak var executionTerminationController: ExecutionTerminationControlling?

    #if os(macOS)
    override init() {
        super.init()
    }
    #else
    override init() {
        super.init()
    }
    #endif

    func prepareForTermination() {
        executionTerminationController?.prepareForTermination()
    }
}

#if os(macOS)
extension AppTerminationCoordinator: NSApplicationDelegate {
    func applicationWillTerminate(_ notification: Notification) {
        prepareForTermination()
    }
}
#endif
