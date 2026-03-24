import Foundation

enum UsageReceiptNormalizer {
    static func makeReceipt(
        providerFamily: String,
        configuredProviderID: UUID?,
        model: String,
        effort: String?,
        transport: String,
        costCents: Int64?,
        durationSeconds: Double,
        rawReceiptJSON: Data? = nil
    ) -> ProviderExecutionReceipt {
        ProviderExecutionReceipt(
            providerFamily: providerFamily,
            configuredProviderID: configuredProviderID,
            model: model,
            effort: effort,
            transport: transport,
            inputTokens: nil,
            outputTokens: nil,
            billedUnits: nil,
            costCents: costCents,
            wallClockSeconds: durationSeconds,
            rawReceiptJSON: rawReceiptJSON
        )
    }
}
