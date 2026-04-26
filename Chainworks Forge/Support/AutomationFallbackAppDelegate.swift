import SwiftUI

#if os(macOS)
import AppKit

final class AutomationFallbackAppDelegate: AppTerminationCoordinator {
    private var fallbackWindow: NSWindow?
    private var windowPolicyObservers: [NSObjectProtocol] = []

    func applicationDidFinishLaunching(_ notification: Notification) {
        installWindowRestorationPolicyIfNeeded()
        guard Chainworks_ForgeApp.isUIAutomationHost else { return }
        guard NSApp.windows.contains(where: { $0.isVisible && !$0.isMiniaturized }) == false else {
            return
        }

        let hostingController = NSHostingController(rootView: ContentView())
        let window = NSWindow(contentViewController: hostingController)
        window.title = "Chainworks Forge"
        window.identifier = NSUserInterfaceItemIdentifier("chainworks-fallback-window")
        window.setContentSize(NSSize(width: 1200, height: 800))
        window.styleMask = [.titled, .closable, .miniaturizable, .resizable]
        Chainworks_ForgeApp.applyWindowRestorationPolicy(to: window)
        window.center()
        window.makeKeyAndOrderFront(nil)
        window.orderFrontRegardless()
        NSApp.setActivationPolicy(.regular)
        NSApp.activate(ignoringOtherApps: true)
        fallbackWindow = window
    }

    private func installWindowRestorationPolicyIfNeeded() {
        guard Chainworks_ForgeApp.shouldDisableWindowRestoration else { return }
        Chainworks_ForgeApp.applyWindowRestorationPolicyToOpenWindows()
        DispatchQueue.main.async {
            Chainworks_ForgeApp.applyWindowRestorationPolicyToOpenWindows()
        }

        let notifications: [NSNotification.Name] = [
            NSWindow.didBecomeKeyNotification,
            NSWindow.didBecomeMainNotification,
        ]
        for name in notifications {
            let observer = NotificationCenter.default.addObserver(
                forName: name,
                object: nil,
                queue: .main
            ) { notification in
                guard let window = notification.object as? NSWindow else { return }
                Chainworks_ForgeApp.applyWindowRestorationPolicy(to: window)
            }
            windowPolicyObservers.append(observer)
        }
    }
}
#endif
