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
    static let defaultHostTotalMilliseconds = 5_000

    weak var executionTerminationController: ExecutionTerminationControlling?
    var hostTotalMilliseconds = AppTerminationCoordinator.defaultHostTotalMilliseconds
    private var gracefulTerminationInProgress = false

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

    func beginGracefulTermination(reply: @escaping @MainActor (Bool) -> Void) {
        guard !gracefulTerminationInProgress else {
            return
        }

        gracefulTerminationInProgress = true
        prepareForTermination()
        let delayNanoseconds = UInt64(max(hostTotalMilliseconds, 0)) * 1_000_000
        Task { @MainActor [weak self] in
            if delayNanoseconds > 0 {
                try? await Task.sleep(nanoseconds: delayNanoseconds)
            }
            self?.finishGracefulTermination(reply: reply)
        }
    }

    private func finishGracefulTermination(reply: @escaping @MainActor (Bool) -> Void) {
        guard gracefulTerminationInProgress else {
            return
        }
        gracefulTerminationInProgress = false
        reply(true)
    }
}

#if os(macOS)
extension AppTerminationCoordinator: NSApplicationDelegate {
    func applicationShouldTerminate(_ sender: NSApplication) -> NSApplication.TerminateReply {
        beginGracefulTermination { shouldTerminate in
            sender.reply(toApplicationShouldTerminate: shouldTerminate)
        }
        return .terminateLater
    }

    func applicationWillTerminate(_ notification: Notification) {
        prepareForTermination()
    }
}
#endif
