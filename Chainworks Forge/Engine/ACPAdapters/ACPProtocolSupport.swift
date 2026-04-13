import Foundation

enum ACPProtocolSupport {
    static func formatJSONRPCError(
        _ error: [String: Any],
        fallback: String
    ) -> String {
        let message = (error["message"] as? String)?.trimmingCharacters(in: .whitespacesAndNewlines)
        let code = error["code"] as? Int
        let dataDescription: String? = {
            if let string = error["data"] as? String, !string.isEmpty {
                return string
            }
            if let payload = error["data"],
               JSONSerialization.isValidJSONObject(payload),
               let data = try? JSONSerialization.data(withJSONObject: payload, options: [.sortedKeys]),
               let text = String(data: data, encoding: .utf8),
               !text.isEmpty {
                return text
            }
            return nil
        }()

        var parts: [String] = []
        if let message, !message.isEmpty {
            parts.append(message)
        } else {
            parts.append(fallback)
        }
        if let code {
            parts.append("(code \(code))")
        }
        if let dataDescription {
            parts.append(dataDescription)
        }
        return parts.joined(separator: " ")
    }

    static func permissionSelectionResponse(
        requestID: Any?,
        params: [String: Any]?
    ) -> [String: Any]? {
        guard let requestID else { return nil }
        guard let optionID = selectedPermissionOptionID(from: params) else { return nil }

        return [
            "jsonrpc": "2.0",
            "id": requestID,
            "result": [
                "outcome": [
                    "outcome": "selected",
                    "optionId": optionID
                ]
            ]
        ]
    }

    private static func selectedPermissionOptionID(from params: [String: Any]?) -> String? {
        let options = (params?["options"] as? [[String: Any]])
            ?? ((params?["toolCall"] as? [String: Any])?["options"] as? [[String: Any]])
            ?? []

        if let preferred = options.first(where: { ($0["kind"] as? String) == "allow_once" }),
           let optionID = preferred["optionId"] as? String,
           !optionID.isEmpty {
            return optionID
        }

        if let approved = options.first(where: { ($0["optionId"] as? String) == "approved" }),
           let optionID = approved["optionId"] as? String,
           !optionID.isEmpty {
            return optionID
        }

        return nil
    }

    static func unsupportedRequestResponse(
        requestID: Any?,
        method: String
    ) -> [String: Any]? {
        guard let requestID else { return nil }
        return [
            "jsonrpc": "2.0",
            "id": requestID,
            "error": [
                "code": -32601,
                "message": "Unsupported ACP client request: \(method)"
            ]
        ]
    }

    static func stripANSIEscapeCodes(from text: String) -> String {
        let pattern = #"\u001B\[[0-9;]*[ -/]*[@-~]"#
        guard let regex = try? NSRegularExpression(pattern: pattern) else {
            return text
        }
        let range = NSRange(text.startIndex..., in: text)
        return regex.stringByReplacingMatches(in: text, options: [], range: range, withTemplate: "")
    }

    static func codexProviderDiagnostic(fromStderrLine line: String) -> RuntimeProviderDiagnostic? {
        let sanitized = stripANSIEscapeCodes(from: line).trimmingCharacters(in: .whitespacesAndNewlines)
        guard !sanitized.isEmpty else { return nil }

        let message: String = {
            if let range = sanitized.range(of: "error=") {
                return sanitized[range.upperBound...].trimmingCharacters(in: .whitespacesAndNewlines)
            }
            return sanitized
        }()

        let lowercased = message.lowercased()
        let severity: RuntimeProviderDiagnosticSeverity =
            if lowercased.contains("error") || lowercased.contains("failed") || lowercased.contains("panic") {
                .error
            } else if lowercased.contains("warning") {
                .warning
            } else {
                .info
            }

        let normalizedReason: String? =
            if lowercased.contains("stdin is closed for this session") {
                "stdin_closed_for_session"
            } else if lowercased.contains("apply_patch verification failed") {
                "apply_patch_verification_failed"
            } else if lowercased.contains("exec_command failed")
                && (lowercased.contains("createprocess") || lowercased.contains("failed to create unified exec process")) {
                "exec_command_create_process_failed"
            } else if lowercased.contains("exec_command failed") {
                "exec_command_failed"
            } else if lowercased.contains("write_stdin failed") {
                "write_stdin_failed"
            } else {
                nil
            }

        return RuntimeProviderDiagnostic(
            source: "codex_stderr",
            severity: severity,
            message: message,
            normalizedReason: normalizedReason
        )
    }

    static func geminiProviderDiagnostic(fromStderrLine line: String) -> RuntimeProviderDiagnostic? {
        let sanitized = stripANSIEscapeCodes(from: line).trimmingCharacters(in: .whitespacesAndNewlines)
        guard !sanitized.isEmpty else { return nil }

        let lowercased = sanitized.lowercased()
        let normalizedReason: String? =
            if lowercased.contains("model_capacity_exhausted")
                || lowercased.contains("\"reason\": \"model_capacity_exhausted\"")
                || lowercased.contains("\"reason\":\"model_capacity_exhausted\"")
                || lowercased.contains("no capacity available for model") {
                "model_capacity_exhausted"
            } else if lowercased.contains("\"method not found\": session/close")
                || (lowercased.contains("method not found") && lowercased.contains("session/close")) {
                "session_close_unsupported"
            } else if lowercased.contains("resource_exhausted") {
                "resource_exhausted"
            } else {
                nil
            }

        let severity: RuntimeProviderDiagnosticSeverity =
            if normalizedReason == "session_close_unsupported" {
                .warning
            } else if lowercased.contains("error") || lowercased.contains("failed") {
                .error
            } else if lowercased.contains("warning") {
                .warning
            } else {
                .info
            }

        guard normalizedReason != nil || severity != .info else { return nil }

        return RuntimeProviderDiagnostic(
            source: "gemini_stderr",
            severity: severity,
            message: sanitized,
            normalizedReason: normalizedReason
        )
    }

    static func shouldPersistProviderDiagnostic(_ diagnostic: RuntimeProviderDiagnostic) -> Bool {
        diagnostic.normalizedReason != "session_close_unsupported"
    }
}
