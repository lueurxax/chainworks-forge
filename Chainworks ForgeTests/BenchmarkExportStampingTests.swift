import Testing
import Foundation
@testable import Chainworks_Forge

@Suite("Benchmark Export Stamping", .tags(.fast))
struct BenchmarkExportStampingTests {
    @Test("Marks matching app-driven benchmark record as exported")
    func marksMatchingAppDrivenRecord() throws {
        let runID = UUID()
        let cohort = BenchmarkCohort(label: "Cohort")
        let pair = BenchmarkPair(ideaIdentifier: "idea", repositoryID: "repo")
        pair.cohort = cohort

        let appRecord = BenchmarkExecutionRecord(
            executionMode: .appDriven,
            linkedRunID: runID,
            terminalOutcome: .happyPathCompleted
        )
        pair.appDrivenRecord = appRecord

        let exportedAt = Date(timeIntervalSince1970: 1234)
        var saveCalled = false

        let marked = try BenchmarkExportStamping.markRunEvidencePackExported(
            runID: runID,
            cohortID: cohort.id,
            fetchPairs: { [pair] },
            save: {
                saveCalled = true
            },
            exportedAt: exportedAt
        )

        #expect(marked)
        #expect(saveCalled)
        #expect(appRecord.evidencePackExportedAt == exportedAt)
    }

    @Test("Throws when benchmark export stamp save fails")
    func throwsWhenSaveFails() {
        enum TestError: LocalizedError {
            case saveFailed

            var errorDescription: String? {
                "fixture save failed"
            }
        }

        let runID = UUID()
        let cohort = BenchmarkCohort(label: "Cohort")
        let pair = BenchmarkPair(ideaIdentifier: "idea", repositoryID: "repo")
        pair.cohort = cohort

        let appRecord = BenchmarkExecutionRecord(
            executionMode: .appDriven,
            linkedRunID: runID,
            terminalOutcome: .happyPathCompleted
        )
        pair.appDrivenRecord = appRecord

        #expect(throws: TestError.self) {
            try BenchmarkExportStamping.markRunEvidencePackExported(
                runID: runID,
                cohortID: cohort.id,
                fetchPairs: { [pair] },
                save: {
                    throw TestError.saveFailed
                }
            )
        }
    }
}
