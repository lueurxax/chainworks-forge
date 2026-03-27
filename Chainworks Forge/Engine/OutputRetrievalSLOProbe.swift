import Foundation

// MARK: - OutputRetrievalSLOProbe (Proposal 008 — §6.4, PERF-080)

/// Measures active output/report open latency from operator action to first rendered content.
/// Tracks p50, p95, p99 percentiles against the PERF-080 SLO target of p95 <= 2.0 seconds.
///
/// Thread-safe: all mutable state is serialised through an actor-isolated lock.
/// Does NOT touch SwiftData; designed to run on any thread.
final class OutputRetrievalSLOProbe: @unchecked Sendable {

    /// PERF-080 target: p95 must not exceed this value.
    static let p95TargetSeconds: Double = 2.0

    // MARK: - Measurement Storage

    private let lock = NSLock()
    private var measurements: [LatencyMeasurement] = []

    // MARK: - Record

    /// Record an individual output-retrieval latency measurement.
    /// - Parameters:
    ///   - operationID: Unique identifier for the retrieval operation
    ///   - artifactName: Name of the artifact/report being opened
    ///   - runID: The run ID the artifact belongs to (if applicable)
    ///   - latencySeconds: Elapsed time from operator action to first rendered content
    ///   - succeeded: Whether the content was rendered successfully
    func recordMeasurement(
        operationID: UUID = UUID(),
        artifactName: String,
        runID: UUID? = nil,
        latencySeconds: Double,
        succeeded: Bool = true
    ) {
        let measurement = LatencyMeasurement(
            operationID: operationID,
            artifactName: artifactName,
            runID: runID,
            latencySeconds: latencySeconds,
            succeeded: succeeded,
            recordedAt: Date()
        )
        lock.lock()
        measurements.append(measurement)
        lock.unlock()
    }

    /// Convenience: measure a closure and record the latency automatically.
    /// - Parameters:
    ///   - artifactName: Name of the artifact/report being opened
    ///   - runID: The run ID the artifact belongs to (if applicable)
    ///   - operation: The retrieval operation to measure
    /// - Returns: The result of the operation
    func measure<T>(
        artifactName: String,
        runID: UUID? = nil,
        operation: () throws -> T
    ) rethrows -> T {
        let start = CFAbsoluteTimeGetCurrent()
        do {
            let result = try operation()
            let elapsed = CFAbsoluteTimeGetCurrent() - start
            recordMeasurement(
                artifactName: artifactName,
                runID: runID,
                latencySeconds: elapsed,
                succeeded: true
            )
            return result
        } catch {
            let elapsed = CFAbsoluteTimeGetCurrent() - start
            recordMeasurement(
                artifactName: artifactName,
                runID: runID,
                latencySeconds: elapsed,
                succeeded: false
            )
            throw error
        }
    }

    /// Async variant of measure for async retrieval operations.
    func measureAsync<T>(
        artifactName: String,
        runID: UUID? = nil,
        operation: () async throws -> T
    ) async rethrows -> T {
        let start = CFAbsoluteTimeGetCurrent()
        do {
            let result = try await operation()
            let elapsed = CFAbsoluteTimeGetCurrent() - start
            recordMeasurement(
                artifactName: artifactName,
                runID: runID,
                latencySeconds: elapsed,
                succeeded: true
            )
            return result
        } catch {
            let elapsed = CFAbsoluteTimeGetCurrent() - start
            recordMeasurement(
                artifactName: artifactName,
                runID: runID,
                latencySeconds: elapsed,
                succeeded: false
            )
            throw error
        }
    }

    // MARK: - Percentile Computation

    /// Compute current percentile statistics from all stored measurements.
    /// Only considers successful measurements for SLO evaluation.
    func computePercentiles() -> PercentileReport {
        lock.lock()
        let snapshot = measurements
        lock.unlock()

        let successfulLatencies = snapshot
            .filter(\.succeeded)
            .map(\.latencySeconds)
            .sorted()

        let totalCount = snapshot.count
        let successCount = successfulLatencies.count
        let failureCount = totalCount - successCount

        guard !successfulLatencies.isEmpty else {
            return PercentileReport(
                p50: nil,
                p95: nil,
                p99: nil,
                min: nil,
                max: nil,
                mean: nil,
                sampleCount: totalCount,
                successCount: successCount,
                failureCount: failureCount,
                p95MeetsSLO: false,
                sloTargetSeconds: Self.p95TargetSeconds
            )
        }

        let p50 = percentile(successfulLatencies, at: 0.50)
        let p95 = percentile(successfulLatencies, at: 0.95)
        let p99 = percentile(successfulLatencies, at: 0.99)
        let minVal = successfulLatencies.first!
        let maxVal = successfulLatencies.last!
        let meanVal = successfulLatencies.reduce(0.0, +) / Double(successfulLatencies.count)

        return PercentileReport(
            p50: p50,
            p95: p95,
            p99: p99,
            min: minVal,
            max: maxVal,
            mean: meanVal,
            sampleCount: totalCount,
            successCount: successCount,
            failureCount: failureCount,
            p95MeetsSLO: p95 <= Self.p95TargetSeconds,
            sloTargetSeconds: Self.p95TargetSeconds
        )
    }

    // MARK: - Query

    /// Return all recorded measurements, optionally filtered by run ID.
    func allMeasurements(forRunID runID: UUID? = nil) -> [LatencyMeasurement] {
        lock.lock()
        let snapshot = measurements
        lock.unlock()

        if let runID {
            return snapshot.filter { $0.runID == runID }
        }
        return snapshot
    }

    /// Reset all stored measurements. Intended for test harnesses.
    func reset() {
        lock.lock()
        measurements.removeAll()
        lock.unlock()
    }

    // MARK: - Percentile Calculation

    /// Nearest-rank percentile computation.
    /// - Parameters:
    ///   - sortedValues: Pre-sorted array of latency values
    ///   - at: Percentile as a fraction (0.0 - 1.0)
    /// - Returns: The value at the requested percentile
    private func percentile(_ sortedValues: [Double], at fraction: Double) -> Double {
        guard !sortedValues.isEmpty else { return 0 }
        let count = sortedValues.count
        if count == 1 { return sortedValues[0] }

        // Nearest-rank method
        let rank = fraction * Double(count - 1)
        let lowerIndex = Int(rank.rounded(.down))
        let upperIndex = min(lowerIndex + 1, count - 1)
        let fractionalPart = rank - Double(lowerIndex)

        return sortedValues[lowerIndex] + fractionalPart * (sortedValues[upperIndex] - sortedValues[lowerIndex])
    }
}

// MARK: - LatencyMeasurement

struct LatencyMeasurement: Sendable {
    let operationID: UUID
    let artifactName: String
    let runID: UUID?
    let latencySeconds: Double
    let succeeded: Bool
    let recordedAt: Date
}

// MARK: - PercentileReport

struct PercentileReport: Sendable {
    let p50: Double?
    let p95: Double?
    let p99: Double?
    let min: Double?
    let max: Double?
    let mean: Double?
    let sampleCount: Int
    let successCount: Int
    let failureCount: Int
    let p95MeetsSLO: Bool
    let sloTargetSeconds: Double

    /// Human-readable summary suitable for logging or display.
    var summary: String {
        guard let p50, let p95, let p99 else {
            return "No successful measurements recorded (total: \(sampleCount), failures: \(failureCount))"
        }
        let sloStatus = p95MeetsSLO ? "PASS" : "FAIL"
        return String(format:
            "Output Retrieval SLO [%@] — p50: %.3fs, p95: %.3fs, p99: %.3fs " +
            "(target p95 <= %.1fs) — samples: %d success, %d failed",
            sloStatus, p50, p95, p99, sloTargetSeconds,
            successCount, failureCount
        )
    }
}
