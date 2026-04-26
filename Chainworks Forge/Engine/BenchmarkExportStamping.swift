import Foundation
import SwiftData

struct BenchmarkExportStamping {
    static func markRunEvidencePackExported(
        runID: UUID,
        cohortID: UUID?,
        context: ModelContext,
        exportedAt: Date = Date()
    ) throws -> Bool {
        try markRunEvidencePackExported(
            runID: runID,
            cohortID: cohortID,
            fetchPairs: {
                try context.fetch(FetchDescriptor<BenchmarkPair>())
            },
            save: {
                try context.save()
            },
            exportedAt: exportedAt
        )
    }

    static func markRunEvidencePackExported(
        runID: UUID,
        cohortID: UUID?,
        fetchPairs: () throws -> [BenchmarkPair],
        save: () throws -> Void,
        exportedAt: Date = Date()
    ) throws -> Bool {
        guard let cohortID else { return false }

        let allPairs = try fetchPairs()
        guard let pair = allPairs.first(where: {
            $0.appDrivenRecord?.linkedRunID == runID && $0.cohort?.id == cohortID
        }),
        let appRecord = pair.appDrivenRecord else {
            return false
        }

        appRecord.evidencePackExportedAt = exportedAt
        try save()
        return true
    }
}
