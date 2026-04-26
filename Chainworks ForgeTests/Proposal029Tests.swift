import Testing
import Foundation
@testable import Chainworks_Forge

@MainActor
@Suite("Proposal029", .serialized)
struct Proposal029Tests {
    // MARK: - Test 1: Fail-closed factory throws for unknown adapter family

    @Test("Transport factory throws unknownAdapterFamily for unregistered family")
    func failClosedFactoryThrowsForUnknownFamily() throws {
        let factory = DefaultRuntimeTransportFactory(fixtureTransport: nil)
        let agent = ResolvedAgent(
            id: "test_agent",
            title: "Test Agent",
            mode: "tool_use",
            provider: "unknown_provider",
            model: "test",
            effort: "medium",
            maxTurns: 5,
            temperature: 0.0,
            permissionProfile: "ORCH",
            skillRef: "sk1",
            skillRole: nil,
            prompt: "Test prompt",
            outputContract: nil,
            requiresHumanApproval: false,
            inputs: [],
            outputs: ["output"]
        )
        let binding = ResolvedProviderBinding(
            agentID: "test_agent",
            backendProfileID: nil,
            configuredProviderID: UUID(),
            providerFamily: "unknown",
            providerIdentifier: "unknown",
            model: "test",
            effort: "medium",
            transport: "acp_stdio",
            adapterVersion: "v1",
            runtimeProfileID: "unknown_profile",
            adapterFamily: "unknown_family",
            capabilityClass: .controlCapable
        )
        #expect(throws: RuntimeTransportError.self) {
            _ = try factory.transport(for: agent, binding: binding)
        }
    }

    // MARK: - Test 2: Factory still works for registered families

    @Test("Transport factory resolves registered ACP families without throwing")
    func factoryResolvesRegisteredFamilies() throws {
        let factory = DefaultRuntimeTransportFactory(fixtureTransport: nil)
        let agent = ResolvedAgent(
            id: "test_agent",
            title: "Test Agent",
            mode: "tool_use",
            provider: "acp_provider",
            model: "test",
            effort: "medium",
            maxTurns: 5,
            temperature: 0.0,
            permissionProfile: "ORCH",
            skillRef: "sk1",
            skillRole: nil,
            prompt: "Test prompt",
            outputContract: nil,
            requiresHumanApproval: false,
            inputs: [],
            outputs: ["output"]
        )
        for family in ["claude_agent_acp", "gemini_cli_acp", "codex_acp", "auggie_cli_acp", "junie_cli_acp"] {
            let binding = ResolvedProviderBinding(
                agentID: "test_agent", backendProfileID: nil, configuredProviderID: UUID(),
                providerFamily: family, providerIdentifier: family, model: "test", effort: "medium",
                transport: "acp_stdio", adapterVersion: "v1",
                adapterFamily: family, capabilityClass: .controlCapable
            )
            let transport = try factory.transport(for: agent, binding: binding)
            #expect(transport.mcpRuntimeNamespace != nil)
        }
    }

    // MARK: - Test 3: New provider families exist

    @Test("ProviderFamily includes second-wave ACP families")
    func providerFamilyIncludesSecondWave() {
        let families = ProviderFamily.allCases.map(\.rawValue)
        #expect(families.contains("codexACP"))
        #expect(families.contains("auggie"))
        #expect(families.contains("junie"))
    }

    // MARK: - Test 4: New ACP transports have correct namespaces

    @Test("Second-wave ACP transports declare correct runtime namespaces")
    func secondWaveNamespaces() {
        #expect(CodexACPTransport().mcpRuntimeNamespace == "codex")
        #expect(AuggieCLIACPTransport().mcpRuntimeNamespace == "auggie")
        #expect(JunieCLIACPTransport().mcpRuntimeNamespace == "junie")
    }

    // MARK: - Test 5: effectiveRuntimeNamespace includes second-wave families

    @Test("ResolvedProviderBinding namespace resolves for second-wave adapters")
    func namespaceResolvesForSecondWave() {
        for (family, expected) in [("codex_acp", "codex"), ("auggie_cli_acp", "auggie"), ("junie_cli_acp", "junie")] {
            let binding = ResolvedProviderBinding(
                agentID: "test", backendProfileID: nil, configuredProviderID: UUID(),
                providerFamily: family, providerIdentifier: family, model: "test", effort: "medium",
                transport: "acp_stdio", adapterVersion: "v1",
                adapterFamily: family
            )
            #expect(binding.effectiveRuntimeNamespace == expected)
        }
    }

    // MARK: - Test 6: ProviderCapabilities.satisfies maps tokens correctly

    @Test("ProviderCapabilities.satisfies maps requires tokens to capability fields")
    func capabilitiesSatisfiesMapping() {
        let caps = ProviderCapabilities.default(for: .codexACP)
        #expect(caps.satisfies("streaming") == true)
        #expect(caps.satisfies("tools") == true)
        #expect(caps.satisfies("session_resume") == true)  // codexACP supports this
        #expect(caps.satisfies("structured_output") == false)
        #expect(caps.satisfies("nonexistent_token") == false)

        let auggieCaps = ProviderCapabilities.default(for: .auggie)
        #expect(auggieCaps.satisfies("session_resume") == false)
    }

    // MARK: - Test 7: preferredProvider filters disabled providers

    @Test("Disabled providers are not returned by preferredProvider")
    func disabledProviderFiltering() throws {
        // Create a provider settings store with a disabled provider
        let tempDir = FileManager.default.temporaryDirectory
            .appendingPathComponent("p029-test-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: tempDir, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: tempDir) }

        let disabledProvider = ConfiguredProvider(
            family: .codexACP,
            displayName: "Codex ACP",
            transport: .cli,
            authMode: .apiKey,
            defaultModel: "gpt-5",
            isEnabled: false
        )
        let store = ProviderSettingsStore(
            fileURL: tempDir.appendingPathComponent("settings.json"),
            initialSettings: ProviderSettings(
                configuredProviders: [disabledProvider],
                preferredProviderIDsByFamily: [:],
                notificationOnProviderFailure: false,
                runStartRequiresCleanPreflight: false
            )
        )
        let registry = ProviderRegistry(
            settingsStore: store,
            secretStore: KeychainSecretStore(serviceName: "com.chainworks.tests.p029-disabled", useInMemoryStore: true)
        )

        let resolved = registry.preferredProvider(for: .codexACP)
        #expect(resolved == nil)
    }

    @Test("ProviderSettingsStore migrates empty persisted stores to seeded defaults")
    func emptyPersistedStoreReseedsDefaults() throws {
        let tempDir = FileManager.default.temporaryDirectory
            .appendingPathComponent("p029-empty-store-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: tempDir, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: tempDir) }

        let fileURL = tempDir.appendingPathComponent("settings.json")
        try JSONEncoder().encode(ProviderSettings.empty).write(to: fileURL)

        let store = ProviderSettingsStore(fileURL: fileURL)
        let families = Set(store.settings.configuredProviders.map(\.family))

        #expect(families.contains(.codexACP))
        #expect(families.contains(.claudeACP))
        #expect(families.contains(.geminiACP))
        #expect(!store.settings.configuredProviders.isEmpty)
    }

    // MARK: - Test 9: CodexACPTransport session creation fails with subprocess error, not stub error

    @Test("CodexACPTransport session creation fails with subprocess error, not stub error")
    func codexTransportIsNotStubbed() async {
        let transport = CodexACPTransport(executablePath: "/nonexistent/codex-acp")
        do {
            _ = try await transport.createSession(request: RuntimeSessionRequest(
                systemPrompt: "test",
                workingDirectory: nil,
                model: "gpt-5",
                provider: nil,
                executionPolicy: nil,
                metadata: nil,
                requestedExtensions: nil
            ))
            Issue.record("Expected error")
        } catch {
            let message = error.localizedDescription
            // Should NOT contain "not yet implemented"
            #expect(!message.contains("not yet implemented"), "Transport is still stubbed: \(message)")
        }
    }

    @Test("CodexACPTransport prepares isolated CODEX_HOME with auth and sanitized runtime config only")
    func codexTransportPreparesIsolatedRuntimeHome() throws {
        let fileManager = FileManager.default
        let tempRoot = fileManager.temporaryDirectory
            .appendingPathComponent("p029-codex-home-\(UUID().uuidString)", isDirectory: true)
        try fileManager.createDirectory(at: tempRoot, withIntermediateDirectories: true)
        defer { try? fileManager.removeItem(at: tempRoot) }

        let sourceHome = tempRoot.appendingPathComponent("source-home", isDirectory: true)
        try fileManager.createDirectory(at: sourceHome, withIntermediateDirectories: true)
        let authURL = sourceHome.appendingPathComponent("auth.json", isDirectory: false)
        let configURL = sourceHome.appendingPathComponent("config.toml", isDirectory: false)
        let stateURL = sourceHome.appendingPathComponent("state_5.sqlite", isDirectory: false)
        try Data("{\"token\":\"abc\"}".utf8).write(to: authURL)
        try Data(
            """
            model = "gpt-5.3-codex-spark"
            model_reasoning_effort = "xhigh"
            personality = "pragmatic"

            [profiles]
            """.utf8
        ).write(to: configURL)
        try Data("sqlite".utf8).write(to: stateURL)

        let runtimeHome = try CodexACPTransport.prepareRuntimeHome(
            workingDirectory: "/tmp/work",
            fileManager: fileManager,
            environment: ["CODEX_HOME": sourceHome.path],
            tempRootURL: tempRoot
        )
        defer { CodexACPTransport.cleanupRuntimeHomeIfPresent(runtimeHome, fileManager: fileManager) }

        #expect(fileManager.fileExists(atPath: runtimeHome.appendingPathComponent("auth.json").path))
        #expect(fileManager.fileExists(atPath: runtimeHome.appendingPathComponent("config.toml").path))
        #expect(!fileManager.fileExists(atPath: runtimeHome.appendingPathComponent("state_5.sqlite").path))
        let runtimeConfig = try String(
            contentsOf: runtimeHome.appendingPathComponent("config.toml"),
            encoding: .utf8
        )
        #expect(!runtimeConfig.contains(#"model = "gpt-5.3-codex-spark""#))
        #expect(!runtimeConfig.contains(#"model_reasoning_effort = "xhigh""#))
        #expect(runtimeConfig.contains(#"personality = "pragmatic""#))
    }

    @Test("CodexACPTransport prepares writable cache roots for sandboxed exec tools")
    func codexTransportPreparesWritableExecCaches() throws {
        let fileManager = FileManager.default
        let tempRoot = fileManager.temporaryDirectory
            .appendingPathComponent("p029-codex-caches-\(UUID().uuidString)", isDirectory: true)
        try fileManager.createDirectory(at: tempRoot, withIntermediateDirectories: true)
        defer { try? fileManager.removeItem(at: tempRoot) }

        let sourceHome = tempRoot.appendingPathComponent("source-home", isDirectory: true)
        try fileManager.createDirectory(at: sourceHome, withIntermediateDirectories: true)
        try Data("{\"token\":\"abc\"}".utf8).write(to: sourceHome.appendingPathComponent("auth.json"))

        let runtimeHome = try CodexACPTransport.prepareRuntimeHome(
            workingDirectory: "/tmp/work",
            fileManager: fileManager,
            environment: ["CODEX_HOME": sourceHome.path],
            tempRootURL: tempRoot
        )
        defer { CodexACPTransport.cleanupRuntimeHomeIfPresent(runtimeHome, fileManager: fileManager) }

        let expectedPaths = [
            runtimeHome.appendingPathComponent("Library/org.swift.swiftpm/configuration", isDirectory: true),
            runtimeHome.appendingPathComponent("Library/org.swift.swiftpm/security", isDirectory: true),
            runtimeHome.appendingPathComponent("Library/Caches/org.swift.swiftpm", isDirectory: true),
            runtimeHome.appendingPathComponent(".cache/clang/ModuleCache", isDirectory: true),
            runtimeHome.appendingPathComponent("tmp", isDirectory: true)
        ]

        for path in expectedPaths {
            #expect(fileManager.fileExists(atPath: path.path), "Expected writable exec cache path at \(path.path)")
        }
    }

    @Test("CodexACPTransport injects a swift shim that disables nested SwiftPM sandboxing")
    func codexTransportInjectsSwiftSandboxShim() async throws {
        let fileManager = FileManager.default
        let tempRoot = fileManager.temporaryDirectory
            .appendingPathComponent("p029-codex-swift-shim-\(UUID().uuidString)", isDirectory: true)
        try fileManager.createDirectory(at: tempRoot, withIntermediateDirectories: true)
        defer { try? fileManager.removeItem(at: tempRoot) }

        let sourceHome = tempRoot.appendingPathComponent("source-home", isDirectory: true)
        try fileManager.createDirectory(at: sourceHome, withIntermediateDirectories: true)
        try Data("{\"token\":\"abc\"}".utf8).write(to: sourceHome.appendingPathComponent("auth.json"))

        let envDumpURL = tempRoot.appendingPathComponent("env.json", isDirectory: false)
        let fakeCodexURL = tempRoot.appendingPathComponent("fake-codex-acp.py", isDirectory: false)
        let script = """
#!/usr/bin/env python3
import json
import os
import sys

with open(\(String(reflecting: envDumpURL.path)), "w", encoding="utf-8") as fh:
    json.dump({"PATH": os.environ.get("PATH", ""), "HOME": os.environ.get("HOME", "")}, fh)

for line in sys.stdin:
    req = json.loads(line)
    mid = req.get("method")
    if mid == "initialize":
        print(json.dumps({"jsonrpc":"2.0","id":req["id"],"result":{"protocolVersion":1}}), flush=True)
    elif mid == "session/new":
        print(json.dumps({"jsonrpc":"2.0","id":req["id"],"result":{"sessionId":"fake-session"}}), flush=True)
    elif mid == "session/close":
        print(json.dumps({"jsonrpc":"2.0","id":req["id"],"result":{}}), flush=True)
        sys.exit(0)
"""
        try script.write(to: fakeCodexURL, atomically: true, encoding: .utf8)
        try fileManager.setAttributes([.posixPermissions: 0o755], ofItemAtPath: fakeCodexURL.path)

        let transport = CodexACPTransport(executablePath: fakeCodexURL.path)
        let session = try await transport.createSession(request: RuntimeSessionRequest(
            systemPrompt: "test",
            workingDirectory: tempRoot.path,
            model: "gpt-5",
            provider: nil,
            executionPolicy: nil,
            metadata: nil,
            requestedExtensions: nil
        ))
        defer {
            Task {
                try? await transport.closeSession(sessionID: session.sessionId)
            }
        }

        let envDumpData = try Data(contentsOf: envDumpURL)
        let envDump = try JSONSerialization.jsonObject(with: envDumpData) as? [String: String]
        let pathValue = try #require(envDump?["PATH"])
        let homeValue = try #require(envDump?["HOME"])

        let firstPathComponent = pathValue.split(separator: ":").first.map(String.init)
        let shimDirectory = try #require(firstPathComponent)
        let shimPath = URL(fileURLWithPath: shimDirectory, isDirectory: true).appendingPathComponent("swift", isDirectory: false)
        let shimContents = try String(contentsOf: shimPath, encoding: .utf8)

        #expect(homeValue.contains(".forge-codex-acp"))
        #expect(fileManager.fileExists(atPath: shimPath.path))
        #expect(shimContents.contains("--disable-sandbox"))
    }

    @Test("CodexACPTransport places isolated runtime home inside working directory when available")
    func codexTransportPlacesRuntimeHomeInsideWorkingDirectory() throws {
        let fileManager = FileManager.default
        let tempRoot = fileManager.temporaryDirectory
            .appendingPathComponent("p029-codex-workingdir-\(UUID().uuidString)", isDirectory: true)
        try fileManager.createDirectory(at: tempRoot, withIntermediateDirectories: true)
        defer { try? fileManager.removeItem(at: tempRoot) }

        let sourceHome = tempRoot.appendingPathComponent("source-home", isDirectory: true)
        let workingDirectory = tempRoot.appendingPathComponent("workspace-root", isDirectory: true)
        try fileManager.createDirectory(at: sourceHome, withIntermediateDirectories: true)
        try fileManager.createDirectory(at: workingDirectory, withIntermediateDirectories: true)
        try Data("{\"token\":\"abc\"}".utf8).write(to: sourceHome.appendingPathComponent("auth.json"))

        let runtimeHome = try CodexACPTransport.prepareRuntimeHome(
            workingDirectory: workingDirectory.path,
            fileManager: fileManager,
            environment: ["CODEX_HOME": sourceHome.path],
            tempRootURL: tempRoot
        )
        defer { CodexACPTransport.cleanupRuntimeHomeIfPresent(runtimeHome, fileManager: fileManager) }

        #expect(runtimeHome.path.hasPrefix(workingDirectory.path + "/.forge-codex-acp/"))
        #expect(fileManager.fileExists(atPath: runtimeHome.appendingPathComponent(".cache/clang/ModuleCache").path))
        #expect(fileManager.fileExists(atPath: runtimeHome.appendingPathComponent("tmp").path))
    }



    @Test("CodexACPTransport surfaces silent EOF before prompt result as streaming failure")
    func codexTransportTreatsSilentEOFAfterPromptAsError() async throws {
        let fileManager = FileManager.default
        let tempRoot = fileManager.temporaryDirectory
            .appendingPathComponent("p029-codex-eof-\(UUID().uuidString)", isDirectory: true)
        try fileManager.createDirectory(at: tempRoot, withIntermediateDirectories: true)
        defer { try? fileManager.removeItem(at: tempRoot) }

        let scriptURL = tempRoot.appendingPathComponent("fake-codex-acp.py")
        let script = """
#!/usr/bin/env python3
import json, sys
for line in sys.stdin:
    req = json.loads(line)
    mid = req.get('method')
    if mid == 'initialize':
        print(json.dumps({'jsonrpc':'2.0','id':req['id'],'result':{'protocolVersion':1}}), flush=True)
    elif mid == 'session/new':
        print(json.dumps({'jsonrpc':'2.0','id':req['id'],'result':{'sessionId':'fake-session'}}), flush=True)
    elif mid == 'session/prompt':
        sys.exit(0)
"""
        try script.write(to: scriptURL, atomically: true, encoding: .utf8)
        try fileManager.setAttributes([.posixPermissions: 0o755], ofItemAtPath: scriptURL.path)

        let transport = CodexACPTransport(executablePath: scriptURL.path)
        let session = try await transport.createSession(request: RuntimeSessionRequest(
            systemPrompt: "test",
            workingDirectory: tempRoot.path,
            model: "gpt-5",
            provider: nil,
            executionPolicy: nil,
            metadata: nil,
            requestedExtensions: nil
        ))

        let stream = transport.submitPrompt(
            sessionID: session.sessionId,
            prompt: RuntimePromptRequest(content: "hello", context: nil)
        )

        do {
            for try await _ in stream {}
            Issue.record("Expected silent EOF to surface as a streaming failure")
        } catch {
            #expect(error.localizedDescription.contains("ended before final result"))
        }
    }

    @Test("CodexACPTransport auto-grants ACP permission requests during prompt execution")
    func codexTransportAutoGrantsPermissionRequests() async throws {
        let fileManager = FileManager.default
        let tempRoot = fileManager.temporaryDirectory
            .appendingPathComponent("p029-codex-permission-\(UUID().uuidString)", isDirectory: true)
        try fileManager.createDirectory(at: tempRoot, withIntermediateDirectories: true)
        defer { try? fileManager.removeItem(at: tempRoot) }

        let scriptURL = tempRoot.appendingPathComponent("fake-codex-acp.py")
        let script = """
#!/usr/bin/env python3
import json, sys
prompt_request_id = None
for line in sys.stdin:
    req = json.loads(line)
    method = req.get("method")
    if method == "initialize":
        print(json.dumps({"jsonrpc": "2.0", "id": req["id"], "result": {"protocolVersion": 1}}), flush=True)
    elif method == "session/new":
        print(json.dumps({"jsonrpc": "2.0", "id": req["id"], "result": {"sessionId": "permission-session"}}), flush=True)
    elif method == "session/prompt":
        prompt_request_id = req["id"]
        print(json.dumps({
            "jsonrpc": "2.0",
            "id": 9001,
            "method": "session/request_permission",
            "params": {
                "toolCall": {"name": "execute"},
                "options": [
                    {"optionId": "allow_once", "kind": "allow_once"},
                    {"optionId": "reject_once", "kind": "reject_once"}
                ]
            }
        }), flush=True)
    elif req.get("id") == 9001:
        outcome = (req.get("result") or {}).get("outcome") or {}
        if outcome.get("outcome") == "selected" and outcome.get("optionId") == "allow_once":
            print(json.dumps({
                "jsonrpc": "2.0",
                "id": prompt_request_id,
                "result": {"stopReason": "end_turn"}
            }), flush=True)
            sys.exit(0)
        sys.exit(2)
"""
        try script.write(to: scriptURL, atomically: true, encoding: .utf8)
        try fileManager.setAttributes([.posixPermissions: 0o755], ofItemAtPath: scriptURL.path)

        let transport = CodexACPTransport(executablePath: scriptURL.path)
        let session = try await transport.createSession(request: RuntimeSessionRequest(
            systemPrompt: "test",
            workingDirectory: tempRoot.path,
            model: "gpt-5",
            provider: nil,
            executionPolicy: nil,
            metadata: nil,
            requestedExtensions: nil
        ))

        let stream = transport.submitPrompt(
            sessionID: session.sessionId,
            prompt: RuntimePromptRequest(content: "hello", context: nil)
        )

        var sawPermission = false
        var sawFinish = false
        for try await event in stream {
            switch event {
            case .toolCallStarted(let toolName, _):
                if toolName == "permission:execute" {
                    sawPermission = true
                }
            case .finish:
                sawFinish = true
            default:
                break
            }
        }

        #expect(sawPermission)
        #expect(sawFinish)
    }

    @Test("CodexACPTransport fails fast on unsupported provider requests instead of hanging until watchdog timeout")
    func codexTransportFailsFastOnUnsupportedProviderRequests() async throws {
        let fileManager = FileManager.default
        let tempRoot = fileManager.temporaryDirectory
            .appendingPathComponent("p029-codex-unsupported-request-\(UUID().uuidString)", isDirectory: true)
        try fileManager.createDirectory(at: tempRoot, withIntermediateDirectories: true)
        defer { try? fileManager.removeItem(at: tempRoot) }

        let scriptURL = tempRoot.appendingPathComponent("fake-codex-acp.py")
        let script = """
#!/usr/bin/env python3
import json, sys
for line in sys.stdin:
    req = json.loads(line)
    method = req.get("method")
    if method == "initialize":
        print(json.dumps({"jsonrpc": "2.0", "id": req["id"], "result": {"protocolVersion": 1}}), flush=True)
    elif method == "session/new":
        print(json.dumps({"jsonrpc": "2.0", "id": req["id"], "result": {"sessionId": "unsupported-request-session"}}), flush=True)
    elif method == "session/prompt":
        print(json.dumps({
            "jsonrpc": "2.0",
            "id": 777,
            "method": "session/request_input",
            "params": {"prompt": "need more input"}
        }), flush=True)
    elif req.get("id") == 777:
        sys.exit(0)
"""
        try script.write(to: scriptURL, atomically: true, encoding: .utf8)
        try fileManager.setAttributes([.posixPermissions: 0o755], ofItemAtPath: scriptURL.path)

        let transport = CodexACPTransport(executablePath: scriptURL.path)
        let session = try await transport.createSession(request: RuntimeSessionRequest(
            systemPrompt: "test",
            workingDirectory: tempRoot.path,
            model: "gpt-5",
            provider: nil,
            executionPolicy: nil,
            metadata: nil,
            requestedExtensions: nil
        ))

        let stream = transport.submitPrompt(
            sessionID: session.sessionId,
            prompt: RuntimePromptRequest(content: "hello", context: nil)
        )

        do {
            for try await _ in stream {}
            Issue.record("Expected unsupported provider request to fail fast")
        } catch {
            #expect(error.localizedDescription.contains("Unsupported Codex ACP client request"))
        }
    }

    @Test("ACPSubprocessManager sendJSON throws instead of crashing when runtime closes stdin")
    func acpSubprocessManagerHandlesClosedStdinGracefully() throws {
        // Launch a process that closes stdin and exits immediately.
        // Once the process exits, sendJSON must throw .notRunning or .brokenPipe.
        let manager = ACPSubprocessManager(
            executablePath: "/usr/bin/python3",
            arguments: ["-c", "import os; os.close(0)"]
        )
        try manager.launch()
        defer { manager.terminate() }

        // Wait for the process to actually exit (up to 2 seconds).
        for _ in 0..<20 {
            if !manager.isRunning { break }
            Thread.sleep(forTimeInterval: 0.1)
        }

        do {
            try manager.sendJSON([
                "jsonrpc": "2.0",
                "id": 1,
                "method": "ping"
            ])
            Issue.record("Expected sendJSON to fail once runtime has exited")
        } catch let error as ACPSubprocessError {
            switch error {
            case .brokenPipe, .notRunning:
                break
            default:
                Issue.record("Unexpected ACPSubprocessError: \(error.localizedDescription)")
            }
        } catch {
            Issue.record("Unexpected error type: \(error)")
        }
    }

    // MARK: - Test 10: AuggieCLIACPTransport session creation fails with subprocess error, not stub error

    @Test("AuggieCLIACPTransport session creation fails with subprocess error, not stub error")
    func auggieTransportIsNotStubbed() async {
        let transport = AuggieCLIACPTransport(executablePath: "/nonexistent/auggie")
        do {
            _ = try await transport.createSession(request: RuntimeSessionRequest(
                systemPrompt: "test",
                workingDirectory: nil,
                model: "default",
                provider: nil,
                executionPolicy: nil,
                metadata: nil,
                requestedExtensions: nil
            ))
            Issue.record("Expected error")
        } catch {
            let message = error.localizedDescription
            // Should NOT contain "not yet implemented"
            #expect(!message.contains("not yet implemented"), "Transport is still stubbed: \(message)")
        }
    }

    // MARK: - Test 11: JunieCLIACPTransport session creation fails with subprocess error, not stub error

    @Test("JunieCLIACPTransport session creation fails with subprocess error, not stub error")
    func junieTransportIsNotStubbed() async {
        let transport = JunieCLIACPTransport(executablePath: "/nonexistent/junie")
        do {
            _ = try await transport.createSession(request: RuntimeSessionRequest(
                systemPrompt: "test",
                workingDirectory: nil,
                model: "default",
                provider: nil,
                executionPolicy: nil,
                metadata: nil,
                requestedExtensions: nil
            ))
            Issue.record("Expected error")
        } catch {
            let message = error.localizedDescription
            // Should NOT contain "not yet implemented"
            #expect(!message.contains("not yet implemented"), "Transport is still stubbed: \(message)")
        }
    }

    @Test("Example catalog keeps only realizable Codex MCP requirements inline on backend profiles")
    func exampleCatalogKeepsOnlyRealizableCodexMCPRequirementsInline() throws {
        let repoRoot = URL(fileURLWithPath: "/Users/user/Documents/Chainworks Forge", isDirectory: true)
        let catalogURL = repoRoot.appendingPathComponent("examples/agents/agents.yaml")
        let catalog = try YAMLParser.loadAgentCatalog(from: catalogURL)

        #expect(catalog.backendProfiles["codex_builder_high"]?.mcp == ["context7", "xcode"])
        #expect(catalog.backendProfiles["codex_audit_high"]?.mcp == ["xcode"])
    }
}
