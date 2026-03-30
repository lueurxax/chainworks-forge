import Foundation

enum PathTemplateResolver {
    static func resolvePath(_ template: String, projectRoot: URL?) -> URL {
        let expanded = expandVariables(in: (template as NSString).expandingTildeInPath)
        if expanded.hasPrefix("/") {
            return URL(fileURLWithPath: expanded, isDirectory: false).standardizedFileURL
        }
        guard let projectRoot else {
            return URL(fileURLWithPath: expanded, isDirectory: false).standardizedFileURL
        }
        return projectRoot.appendingPathComponent(expanded, isDirectory: false).standardizedFileURL
    }

    private static func expandVariables(in template: String) -> String {
        let pattern = #"\$\{([A-Za-z_][A-Za-z0-9_]*)(:-([^}]*))?\}"#
        guard let regex = try? NSRegularExpression(pattern: pattern) else {
            return template
        }

        let nsrange = NSRange(template.startIndex..<template.endIndex, in: template)
        let matches = regex.matches(in: template, range: nsrange)
        guard !matches.isEmpty else { return template }

        var resolved = template
        for match in matches.reversed() {
            guard let whole = Range(match.range(at: 0), in: resolved),
                  let keyRange = Range(match.range(at: 1), in: resolved) else {
                continue
            }

            let key = String(resolved[keyRange])
            let fallback: String
            if match.range(at: 3).location != NSNotFound,
               let fallbackRange = Range(match.range(at: 3), in: resolved) {
                fallback = String(resolved[fallbackRange])
            } else {
                fallback = ""
            }

            let replacement = ProcessInfo.processInfo.environment[key]
                .flatMap { $0.isEmpty ? nil : $0 } ?? fallback
            resolved.replaceSubrange(whole, with: replacement)
        }

        return resolved
    }
}
