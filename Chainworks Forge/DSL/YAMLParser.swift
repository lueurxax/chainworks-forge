import Foundation
import Yams

nonisolated enum YAMLParserError: Error, LocalizedError {
    case fileNotFound(String)
    case fileReadFailed(String, Error)
    case decodingFailed(String, Error)

    var errorDescription: String? {
        switch self {
        case .fileNotFound(let path):
            return "YAML file not found: \(path)"
        case .fileReadFailed(let path, let error):
            return "Failed to read YAML at \(path): \(error.localizedDescription)"
        case .decodingFailed(let path, let error):
            return "Failed to decode YAML at \(path): \(error.localizedDescription)"
        }
    }
}

struct YAMLParser: Sendable {
    nonisolated static func loadAgentCatalog(from url: URL) throws -> AgentCatalog {
        let yamlString = try readFile(at: url)
        do {
            return try YAMLDecoder().decode(AgentCatalog.self, from: yamlString)
        } catch {
            throw YAMLParserError.decodingFailed(url.path, error)
        }
    }

    nonisolated static func loadWorkflow(from url: URL) throws -> WorkflowDefinition {
        let yamlString = try readFile(at: url)
        do {
            return try YAMLDecoder().decode(WorkflowDefinition.self, from: yamlString)
        } catch {
            throw YAMLParserError.decodingFailed(url.path, error)
        }
    }

    nonisolated static func loadCompactWorkflow(from url: URL) throws -> CompactWorkflowDefinition {
        let yamlString = try readFile(at: url)
        do {
            return try YAMLDecoder().decode(CompactWorkflowDefinition.self, from: yamlString)
        } catch {
            throw YAMLParserError.decodingFailed(url.path, error)
        }
    }

    nonisolated static func loadStewardConfig(from url: URL) throws -> StewardConfig {
        let yamlString = try readFile(at: url)
        do {
            return try YAMLDecoder().decode(StewardConfig.self, from: yamlString)
        } catch {
            throw YAMLParserError.decodingFailed(url.path, error)
        }
    }

    private nonisolated static func readFile(at url: URL) throws -> String {
        guard SecurityScopedAccess.fileExists(at: url) else {
            throw YAMLParserError.fileNotFound(url.path)
        }
        do {
            return try SecurityScopedAccess.loadString(from: url)
        } catch {
            let message = "Failed to read YAML at \(url.path): \(error.localizedDescription)"
            Task { @MainActor in
                ForgeLogger.app.error(message)
            }
            throw YAMLParserError.fileReadFailed(url.path, error)
        }
    }
}
