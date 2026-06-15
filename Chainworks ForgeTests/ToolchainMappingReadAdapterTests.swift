import Foundation
import Testing
@testable import Chainworks_Forge

@Suite("ToolchainMappingReadAdapter")
@MainActor
struct ToolchainMappingReadAdapterTests {
    @Test("Toolchain cache policy decode accepts known keys")
    func policyDecodeAcceptsKnownKeys() throws {
        let json = """
        {
          "version": 1,
          "enabled": true,
          "xcode_scope": "run",
          "go_scope": "session"
        }
        """

        let policy = try ToolchainMappingReadAdapter.decodePolicyFromCatalogJSON(json)

        #expect(policy?.version == 1)
        #expect(policy?.enabled == true)
        #expect(policy?.xcodeScope == .run)
        #expect(policy?.goScope == .session)
    }

    @Test("Toolchain cache policy decode rejects unknown keys")
    func policyDecodeRejectsUnknownKeys() throws {
        let json = """
        {
          "version": 1,
          "enabled": true,
          "xcode_scope": "run",
          "go_scope": "session",
          "unexpected": "value"
        }
        """

        do {
            _ = try ToolchainMappingReadAdapter.decodePolicyFromCatalogJSON(json)
            Issue.record("unknown toolchain_cache_policy key should fail decode")
        } catch ToolchainMappingDecodeError.unknownKeys(let keys) {
            #expect(keys == ["unexpected"])
        } catch {
            Issue.record("expected unknownKeys error, got \(error)")
        }
    }
}
