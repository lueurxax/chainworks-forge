import Foundation
import OSLog

enum ForgeLogCategory: String {
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

struct ForgeLogger {
    private let logger: Logger
    private let category: ForgeLogCategory

    init(category: ForgeLogCategory) {
        self.category = category
        self.logger = Logger(subsystem: "xax.Chainworks-Forge", category: category.rawValue)
    }

    func debug(_ message: String) {
        logger.debug("\(message, privacy: .public)")
    }

    func info(_ message: String) {
        logger.info("\(message, privacy: .public)")
    }

    func error(_ message: String) {
        logger.error("\(message, privacy: .public)")
    }

    func fault(_ message: String) {
        logger.fault("\(message, privacy: .public)")
    }
}

// Global loggers for common categories
extension ForgeLogger {
    static let steward = ForgeLogger(category: .steward)
    static let recovery = ForgeLogger(category: .recovery)
    static let execution = ForgeLogger(category: .execution)
    static let session = ForgeLogger(category: .session)
    static let claudeACP = ForgeLogger(category: .claudeACP)
    static let acpSubprocess = ForgeLogger(category: .acpSubprocess)
    static let notification = ForgeLogger(category: .notification)
    static let compiler = ForgeLogger(category: .compiler)
    static let bridge = ForgeLogger(category: .bridge)
    static let ui = ForgeLogger(category: .ui)
    static let app = ForgeLogger(category: .app)
    static let test = ForgeLogger(category: .test)
    static let general = ForgeLogger(category: .general)
}
