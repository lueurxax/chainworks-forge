import SwiftUI

#if os(macOS)
import AppKit

final class AutomationFallbackAppDelegate: AppTerminationCoordinator {
    private var fallbackWindow: NSWindow?

    func applicationDidFinishLaunching(_ notification: Notification) {
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
        window.center()
        window.makeKeyAndOrderFront(nil)
        window.orderFrontRegardless()
        NSApp.setActivationPolicy(.regular)
        NSApp.activate(ignoringOtherApps: true)
        fallbackWindow = window
    }
}
#endif
