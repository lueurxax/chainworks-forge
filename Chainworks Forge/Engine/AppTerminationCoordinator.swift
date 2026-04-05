import Foundation

#if os(macOS)
import AppKit
#endif

@MainActor
protocol ManagedGooseServerControlling: AnyObject {
    func stopManagedServer()
    func prepareForSystemSleep()
    func reconcileAfterSystemWake() async
}

extension ManagedGooseServerControlling {
    func prepareForSystemSleep() {}
    func reconcileAfterSystemWake() async {}
}

@MainActor
class AppTerminationCoordinator: NSObject {
    #if os(macOS)
    private let workspaceNotificationCenter: NotificationCenter
    private var sleepObserver: NSObjectProtocol?
    private var wakeObserver: NSObjectProtocol?
    #endif

    var gooseServerManager: ManagedGooseServerControlling?

    #if os(macOS)
    override init() {
        self.workspaceNotificationCenter = NSWorkspace.shared.notificationCenter
        super.init()
        registerLifecycleObservers()
    }

    init(workspaceNotificationCenter: NotificationCenter) {
        self.workspaceNotificationCenter = workspaceNotificationCenter
        super.init()
        registerLifecycleObservers()
    }

    deinit {
        if let sleepObserver {
            workspaceNotificationCenter.removeObserver(sleepObserver)
        }
        if let wakeObserver {
            workspaceNotificationCenter.removeObserver(wakeObserver)
        }
    }
    #else
    override init() {
        super.init()
    }
    #endif

    func prepareForTermination() {
        gooseServerManager?.stopManagedServer()
    }

    #if os(macOS)
    private func registerLifecycleObservers() {
        sleepObserver = workspaceNotificationCenter.addObserver(
            forName: NSWorkspace.willSleepNotification,
            object: nil,
            queue: nil
        ) { [weak self] _ in
            MainActor.assumeIsolated { [weak self] in
                self?.gooseServerManager?.prepareForSystemSleep()
            }
        }

        wakeObserver = workspaceNotificationCenter.addObserver(
            forName: NSWorkspace.didWakeNotification,
            object: nil,
            queue: nil
        ) { [weak self] _ in
            Task { @MainActor [weak self] in
                await self?.gooseServerManager?.reconcileAfterSystemWake()
            }
        }
    }
    #endif
}

#if os(macOS)
extension AppTerminationCoordinator: NSApplicationDelegate {
    func applicationWillTerminate(_ notification: Notification) {
        prepareForTermination()
    }
}
#endif
