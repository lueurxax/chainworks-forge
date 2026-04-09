import Foundation

// MARK: - AuggieCLIACPTransport (Proposal 029)

/// ACP transport adapter for Auggie CLI (`auggie` subprocess).
/// Communicates via subprocess stdin/stdout using ACP JSON-RPC protocol over ndjson framing.
///
/// Stub implementation — session lifecycle methods throw "not yet implemented" errors.
/// Wire-up is complete so that runtime profile resolution and transport factory routing work
/// end-to-end once the adapter is fleshed out with real subprocess management.
final class AuggieCLIACPTransport: RuntimeTransportProtocol, @unchecked Sendable {

    // MARK: - Configuration

    let executablePath: String

    var mcpRuntimeNamespace: String? { "auggie" }

    // MARK: - Init

    init(executablePath: String = "/usr/local/bin/auggie") {
        self.executablePath = executablePath
    }

    // MARK: - RuntimeTransportProtocol

    func createSession(request: RuntimeSessionRequest) async throws -> RuntimeSessionResponse {
        throw RuntimeTransportError.sessionCreationFailed(reason: "AuggieCLIACPTransport is not yet implemented")
    }

    func submitPrompt(
        sessionID: String,
        prompt: RuntimePromptRequest
    ) -> AsyncThrowingStream<RuntimeStreamEvent, Error> {
        AsyncThrowingStream { continuation in
            continuation.finish(throwing: RuntimeTransportError.streamingFailed(reason: "AuggieCLIACPTransport is not yet implemented"))
        }
    }

    func closeSession(sessionID: String) async throws {
        throw RuntimeTransportError.sessionCloseFailed(reason: "AuggieCLIACPTransport is not yet implemented")
    }
}
