import Testing
import Foundation
@testable import Chainworks_Forge

@Suite("ProviderBindingTruth", .tags(.fast, .provider))
struct ProviderBindingTruthTests {
    @Test("Codex family accepts gpt models without cross-family warning")
    func codexFamilyAcceptsGPTModels() {
        let binding = ResolvedProviderBinding(
            agentID: "proposal_writer",
            backendProfileID: "writer_profile",
            configuredProviderID: UUID(),
            providerFamily: "codex",
            providerIdentifier: "codex",
            model: "gpt-5-codex",
            effort: "high",
            transport: "acp_http",
            adapterVersion: "v1"
        )

        #expect(binding.hasCrossFamilyMismatch == false)
    }

    @Test("Cross-family mismatch still flags obvious provider-model conflicts")
    func mismatchedFamilyStillFlags() {
        let binding = ResolvedProviderBinding(
            agentID: "proposal_writer",
            backendProfileID: "writer_profile",
            configuredProviderID: UUID(),
            providerFamily: "claude",
            providerIdentifier: "claude_code",
            model: "gpt-5-codex",
            effort: "high",
            transport: "acp_http",
            adapterVersion: "v1"
        )

        #expect(binding.hasCrossFamilyMismatch == true)
    }
}
