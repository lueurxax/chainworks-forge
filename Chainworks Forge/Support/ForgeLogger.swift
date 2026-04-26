import Foundation
import OSLog

nonisolated enum ForgeLogCategory: String, Sendable {
    case steward = "Steward"
    case recovery = "Recovery"
    case execution = "Execution"
    case session = "Session"
    case claudeACP = "ClaudeACP"
    case acpSubprocess = "ACPSubprocess"
    case notification = "Notification"
    case compiler = "Compiler"
    case bridge = "Bridge"
    case ui = "UI"
    case app = "App"
    case test = "Test"
    case general = "General"
}

nonisolated struct ForgeLogger: Sendable {
    private let logger: Logger
    private let category: ForgeLogCategory

    nonisolated init(category: ForgeLogCategory) {
        self.category = category
        self.logger = Logger(subsystem: "xax.Chainworks-Forge", category: category.rawValue)
    }

    nonisolated func debug(_ message: String) {
        logger.debug("\(message, privacy: .public)")
    }

    nonisolated func info(_ message: String) {
        logger.info("\(message, privacy: .public)")
    }

    nonisolated func error(_ message: String) {
        logger.error("\(message, privacy: .public)")
    }

    nonisolated func fault(_ message: String) {
        logger.fault("\(message, privacy: .public)")
    }
}

// Global loggers for common categories
extension ForgeLogger {
    nonisolated static let steward = ForgeLogger(category: .steward)
    nonisolated static let recovery = ForgeLogger(category: .recovery)
    nonisolated static let execution = ForgeLogger(category: .execution)
    nonisolated static let session = ForgeLogger(category: .session)
    nonisolated static let claudeACP = ForgeLogger(category: .claudeACP)
    nonisolated static let acpSubprocess = ForgeLogger(category: .acpSubprocess)
    nonisolated static let notification = ForgeLogger(category: .notification)
    nonisolated static let compiler = ForgeLogger(category: .compiler)
    nonisolated static let bridge = ForgeLogger(category: .bridge)
    nonisolated static let ui = ForgeLogger(category: .ui)
    nonisolated static let app = ForgeLogger(category: .app)
    nonisolated static let test = ForgeLogger(category: .test)
    nonisolated static let general = ForgeLogger(category: .general)
}
