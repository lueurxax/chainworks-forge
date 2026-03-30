import Foundation

nonisolated struct ProviderExecutionReceipt: Sendable, Codable, Equatable {
    let providerFamily: String
    let configuredProviderID: UUID?
    let model: String
    let effort: String?
    let transport: String
    let inputTokens: Int?
    let outputTokens: Int?
    let billedUnits: Int?
    let costCents: Int64?
    let wallClockSeconds: Double
    let rawReceiptJSON: Data?
}
