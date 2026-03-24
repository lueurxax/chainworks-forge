import Foundation

// MARK: - P005-OPS §9: Artifact Diff Service

/// Supports pinned artifact content diffing for comparison view.
/// Structural / line-level diff for text-based artifacts.
struct ArtifactDiffService {

    /// Compare two artifact contents and produce a line-level diff.
    static func diff(artifactA: Artifact, artifactB: Artifact) -> ArtifactDiff? {
        guard let contentA = try? String(contentsOfFile: artifactA.filePath, encoding: .utf8),
              let contentB = try? String(contentsOfFile: artifactB.filePath, encoding: .utf8) else {
            return nil
        }

        return diff(linesA: contentA.components(separatedBy: .newlines),
                     linesB: contentB.components(separatedBy: .newlines),
                     nameA: artifactA.name,
                     nameB: artifactB.name)
    }

    /// Line-level diff between two string arrays.
    static func diff(linesA: [String], linesB: [String], nameA: String, nameB: String) -> ArtifactDiff {
        var hunks: [ArtifactDiff.Hunk] = []
        var i = 0, j = 0

        while i < linesA.count || j < linesB.count {
            if i < linesA.count && j < linesB.count && linesA[i] == linesB[j] {
                // Context line
                i += 1
                j += 1
            } else {
                // Divergence — collect changed lines
                var removed: [String] = []
                var added: [String] = []
                let startI = i
                let startJ = j

                // Simple: consume differing lines until we find a match again
                while i < linesA.count && (j >= linesB.count || linesA[i] != linesB[j]) {
                    removed.append(linesA[i])
                    i += 1
                }
                while j < linesB.count && (i >= linesA.count || linesB[j] != linesA[i]) {
                    added.append(linesB[j])
                    j += 1
                }

                if !removed.isEmpty || !added.isEmpty {
                    hunks.append(ArtifactDiff.Hunk(
                        lineA: startI + 1,
                        lineB: startJ + 1,
                        removed: removed,
                        added: added
                    ))
                }
            }
        }

        return ArtifactDiff(
            nameA: nameA,
            nameB: nameB,
            hunks: hunks,
            identical: hunks.isEmpty
        )
    }
}

// MARK: - Diff Types

struct ArtifactDiff: Identifiable {
    let id = UUID()
    let nameA: String
    let nameB: String
    let hunks: [Hunk]
    let identical: Bool

    struct Hunk: Identifiable {
        let id = UUID()
        let lineA: Int
        let lineB: Int
        let removed: [String]
        let added: [String]
    }
}
