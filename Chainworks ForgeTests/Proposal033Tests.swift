import Testing
import Foundation
@testable import Chainworks_Forge

@Suite("Proposal033", .serialized)
struct Proposal033Tests {

    @Test("Example agent catalogs use backend-owned MCP and contain no legacy MCP blocks")
    func exampleCatalogsUseBackendOwnedMCP() throws {
        let repoRoot = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent()
            .deletingLastPathComponent()
        let catalogURLs = [
            repoRoot.appendingPathComponent("examples/agents/agents.yaml"),
            repoRoot.appendingPathComponent("examples/agents/agents_mcp_profiles_v2.yaml")
        ]

        for url in catalogURLs {
            let raw = try String(contentsOf: url, encoding: .utf8)
            #expect(!raw.contains("mcp_server_registry:"))
            #expect(!raw.contains("mcp_profiles:"))
            #expect(!raw.contains("mcp_profile:"))

            let catalog = try YAMLParser.loadAgentCatalog(from: url)
            #expect(catalog.backendProfiles["codex_builder_high"]?.mcp == ["context7", "xcode"])
            #expect(catalog.backendProfiles["codex_writer_high"]?.mcp == ["context7", "xcode"])
            #expect(catalog.backendProfiles["gemini_review_pro"]?.mcp == ["xcode"])
            #expect(catalog.backendProfiles["claude_security_high"]?.mcp == ["context7"])
            #expect(ProviderFamily.from(runtimeIdentifier: catalog.backendProfiles["claude_orchestrator_high"]?.provider ?? "") == .claudeACP)
            #expect(ProviderFamily.from(runtimeIdentifier: catalog.backendProfiles["codex_builder_high"]?.provider ?? "") == .codexACP)
            #expect(ProviderFamily.from(runtimeIdentifier: catalog.backendProfiles["gemini_review_pro"]?.provider ?? "") == .geminiACP)

            let agentsWithLegacyMCP = catalog.agents.compactMap { agent -> String? in
                guard let legacy = agent.mcpProfile?.trimmingCharacters(in: .whitespacesAndNewlines),
                      !legacy.isEmpty else { return nil }
                return agent.id
            }
            #expect(agentsWithLegacyMCP.isEmpty)
            #expect(!raw.contains("provider: claude_code"))
            #expect(!raw.contains("provider: codex\n"))
            #expect(!raw.contains("provider: gemini\n"))
            #expect(!raw.contains("provider: codexACP"))
            #expect(!raw.contains("provider: claudeACP"))
            #expect(!raw.contains("provider: geminiACP"))
            #expect(raw.contains("provider: codex_acp"))
        }
    }

    @Test("Runtime extension registry reader uses ACP config path and no Goose fallback")
    func runtimeExtensionRegistryReaderUsesACPConfigPathOnly() {
        let reader = CodexExtensionRegistryReader()
        let path = reader.configURL.path
        let usesCanonicalPath = path.hasSuffix("/.config/mcp/config.yaml")
        let usesTestFixturePath = path.hasSuffix("/examples/mcp/mcp-config-fixture.yaml")
        #expect(usesCanonicalPath || usesTestFixturePath)
        #expect(!reader.configURL.path.contains("/.config/goose/"))
    }

    @Test("Runtime extension registry reader migrates legacy shared config into ACP path")
    func runtimeExtensionRegistryReaderMigratesLegacySharedConfigIntoACPPath() throws {
        let tempDirectory = FileManager.default.temporaryDirectory
            .appendingPathComponent(UUID().uuidString, isDirectory: true)
        try FileManager.default.createDirectory(at: tempDirectory, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: tempDirectory) }

        let canonicalURL = tempDirectory.appendingPathComponent("mcp/config.yaml")
        let legacyURL = tempDirectory.appendingPathComponent("goose/config.yaml")
        try FileManager.default.createDirectory(at: legacyURL.deletingLastPathComponent(), withIntermediateDirectories: true)
        try """
        extensions:
          xcode:
            enabled: true
            type: stdio
            name: Xcode
            cmd: xcode-mcp
        """.write(to: legacyURL, atomically: true, encoding: .utf8)

        let reader = CodexExtensionRegistryReader(configURL: canonicalURL, legacyConfigURL: legacyURL)
        let snapshot = try reader.registrySnapshot()

        #expect(reader.configURL == canonicalURL)
        #expect(FileManager.default.isReadableFile(atPath: canonicalURL.path))
        #expect(snapshot.configURL == canonicalURL)
        #expect(snapshot.installedExtensionIDs == ["xcode"])
    }

    @Test("Provider settings raw migration removes Goose transport values before decode")
    func providerSettingsRawMigrationRemovesGooseTransportValuesBeforeDecode() throws {
        let payload = """
        {
          "configuredProviders": [
            {
              "id": "\(UUID().uuidString)",
              "family": "claude",
              "displayName": "Claude Goose",
              "transport": "goose_server",
              "authMode": "apiKey",
              "defaultModel": "opus",
              "endpoint": "https://127.0.0.1:51200",
              "isEnabled": true
            },
            {
              "id": "\(UUID().uuidString)",
              "family": "codex",
              "displayName": "Codex Goose",
              "transport": "goose_server",
              "authMode": "apiKey",
              "defaultModel": "gpt-5",
              "endpoint": "https://127.0.0.1:51200",
              "isEnabled": true
            }
          ],
          "preferredProviderIDsByFamily": {
            "claude": "preserve-me",
            "codex": "drop-me"
          }
        }
        """

        let migrated = try ProviderSettingsStore.migrateRawProviderSettings(Data(payload.utf8))
        let json = try JSONSerialization.jsonObject(with: migrated) as? [String: Any]
        let providers = try #require(json?["configuredProviders"] as? [[String: Any]])
        #expect(providers.count == 1)
        #expect(providers.first?["family"] as? String == "claudeACP")
        #expect(providers.first?["transport"] as? String == "cli")
        #expect((providers.first?["endpoint"] as? NSNull) != nil)

        let preferred = try #require(json?["preferredProviderIDsByFamily"] as? [String: Any])
        #expect(preferred["claudeACP"] as? String == "preserve-me")
        #expect(preferred["codex"] == nil)
    }

    @Test("Active P033 proof surfaces carry ACP-era fixture and naming residue only")
    func activeProofSurfacesDropGooseEraResidue() throws {
        let repoRoot = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent()
            .deletingLastPathComponent()

        let gateScript = try String(
            contentsOf: repoRoot.appendingPathComponent("scripts/test-gate.sh"),
            encoding: .utf8
        )
        #expect(!gateScript.contains("CHAINWORKS_GOOSE_FIXTURE_MODE"))
        #expect(!gateScript.contains("CHAINWORKS_GOOSE_CONFIG_PATH"))
        #expect(gateScript.contains("CHAINWORKS_FIXTURE_MODE"))

        let uiDoc = try String(
            contentsOf: repoRoot.appendingPathComponent("docs/reference/agent-ui-test-execution.md"),
            encoding: .utf8
        )
        #expect(!uiDoc.contains("CHAINWORKS_GOOSE_FIXTURE_MODE"))
        #expect(uiDoc.contains("CHAINWORKS_FIXTURE_MODE"))

        let legacyStubURL = repoRoot.appendingPathComponent("Chainworks ForgeTests/LiveGooseConnectionProofTests.swift")
        let acpStubURL = repoRoot.appendingPathComponent("Chainworks ForgeTests/LiveACPConnectionProofTests.swift")
        #expect(FileManager.default.fileExists(atPath: legacyStubURL.path) == false)
        #expect(FileManager.default.fileExists(atPath: acpStubURL.path))
    }

    @Test("Provider settings raw migration rewrites surviving Goose rows to ACP semantics")
    func providerSettingsRawMigrationRewritesSurvivingRowsToACPSemantics() throws {
        let claudeID = UUID()
        let geminiID = UUID()
        let payload = """
        {
          "configuredProviders": [
            {
              "id": "\(claudeID.uuidString)",
              "family": "claude",
              "displayName": "Claude Goose",
              "transport": "goose_server",
              "authMode": "apiKey",
              "defaultModel": "opus",
              "capabilities": {
                "supportsStreaming": false,
                "supportsTools": false,
                "supportsStructuredOutput": false,
                "supportsEffortControl": false,
                "supportsSessionResume": false,
                "supportsFileEditing": false,
                "supportsSandboxHints": false,
                "supportsMCPReconciliation": false
              },
              "adapterVersion": "v1",
              "endpoint": "https://127.0.0.1:51200",
              "isEnabled": false
            },
            {
              "id": "\(geminiID.uuidString)",
              "family": "gemini",
              "displayName": "Custom Gemini",
              "transport": "goose_server",
              "authMode": "sessionToken",
              "defaultModel": "gemini-2.5-pro",
              "capabilities": {
                "supportsStreaming": false,
                "supportsTools": false,
                "supportsStructuredOutput": false,
                "supportsEffortControl": false,
                "supportsSessionResume": false,
                "supportsFileEditing": false,
                "supportsSandboxHints": false,
                "supportsMCPReconciliation": false
              },
              "adapterVersion": "v1",
              "endpoint": "https://127.0.0.1:51200",
              "isEnabled": false
            }
          ],
          "preferredProviderIDsByFamily": {
            "claude": "\(claudeID.uuidString)",
            "gemini": "\(geminiID.uuidString)"
          }
        }
        """

        let migrated = try ProviderSettingsStore.migrateRawProviderSettings(Data(payload.utf8))
        let decoded = try JSONDecoder().decode(ProviderSettings.self, from: migrated)
        #expect(decoded.configuredProviders.count == 2)

        let claude = try #require(decoded.configuredProviders.first(where: { $0.id == claudeID }))
        #expect(claude.family == .claudeACP)
        #expect(claude.transport == .cli)
        #expect(claude.endpoint == nil)
        #expect(claude.authMode == .apiKey)
        #expect(claude.displayName == "Claude ACP")
        #expect(claude.capabilities == .default(for: .claudeACP))
        #expect(claude.adapterVersion == "acp-v1")
        #expect(claude.isEnabled)

        let gemini = try #require(decoded.configuredProviders.first(where: { $0.id == geminiID }))
        #expect(gemini.family == .geminiACP)
        #expect(gemini.transport == .cli)
        #expect(gemini.endpoint == nil)
        #expect(gemini.authMode == .sessionToken)
        #expect(gemini.displayName == "Custom Gemini")
        #expect(gemini.capabilities == .default(for: .geminiACP))
        #expect(gemini.adapterVersion == "acp-v1")
        #expect(gemini.isEnabled)

        #expect(decoded.preferredProviderIDsByFamily[ProviderFamily.claudeACP.rawValue] == claudeID)
        #expect(decoded.preferredProviderIDsByFamily[ProviderFamily.geminiACP.rawValue] == geminiID)
    }

    @Test("Transfer package migration drops deleted Codex placeholders and keeps surviving provider UUIDs")
    func transferPackageMigrationDropsDeletedCodexPlaceholders() throws {
        let codexID = UUID()
        let claudeID = UUID()
        let payload = """
        {
          "transferSchemaVersion": 1,
          "appConfiguration": {
            "runStorageBasePath": "/tmp/runs",
            "workflowSourcePath": "/tmp/workflow.yaml",
            "agentCatalogSourcePath": "/tmp/agents.yaml",
            "activeConfigurationSource": "persisted_settings"
          },
          "providerSettings": {
            "configuredProviders": [
              {
                "id": "\(codexID.uuidString)",
                "family": "codex",
                "displayName": "Codex Goose",
                "transport": "goose_server",
                "authMode": "apiKey",
                "defaultModel": "gpt-5",
                "adapterVersion": "v1",
                "isEnabled": true
              },
              {
                "id": "\(claudeID.uuidString)",
                "family": "claude",
                "displayName": "Claude Goose",
                "transport": "goose_server",
                "authMode": "apiKey",
                "defaultModel": "opus",
                "adapterVersion": "v1",
                "isEnabled": true
              }
            ],
            "preferredProviderIDsByFamily": {
              "codex": "\(codexID.uuidString)",
              "claude": "\(claudeID.uuidString)"
            }
          },
          "exportedAt": "2026-04-10T20:00:00Z",
          "appVersion": "dev",
          "secretPlaceholders": [
            "provider.\(codexID.uuidString)",
            "provider.\(claudeID.uuidString)"
          ]
        }
        """

        let migrated = try ProviderSettingsStore.migrateRawTransferPackage(Data(payload.utf8))
        let json = try JSONSerialization.jsonObject(with: migrated) as? [String: Any]
        let placeholders = try #require(json?["secretPlaceholders"] as? [String])
        #expect(placeholders == ["provider.\(claudeID.uuidString)"])

        let providerSettingsJSON = try #require(json?["providerSettings"] as? [String: Any])
        let providers = try #require(providerSettingsJSON["configuredProviders"] as? [[String: Any]])
        #expect(providers.count == 1)
        #expect(providers.first?["id"] as? String == claudeID.uuidString)
        #expect(providers.first?["family"] as? String == "claudeACP")
    }

    @Test("Runtime provenance badge recognizes ACP trust vocabulary")
    func runtimeProvenanceBadgeRecognizesACPTrustVocabulary() {
        let verified = RuntimeTrustPresentation(trustLevel: "runtime_verified")
        #expect(verified.badgeLabel == "Verified")
        #expect(verified.badgeIcon == "checkmark.shield.fill")

        let unverified = RuntimeTrustPresentation(trustLevel: "runtime_unverified")
        #expect(unverified.badgeLabel == "Unverified")
        #expect(unverified.badgeIcon == "shield.lefthalf.filled")

        let legacy = RuntimeTrustPresentation(trustLevel: "server_verified")
        #expect(legacy.badgeLabel == "Legacy (verified)")
    }
}
