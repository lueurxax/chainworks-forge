import AppKit
import CryptoKit
import Foundation
import SwiftUI
import Testing
@testable import Chainworks_Forge

@Suite("Codex planned model variant truth", .tags(.fast))
@MainActor
struct CodexModelVariantTruthTests {
    @Test("Pinned policy is present in the built app bundle")
    func bundledPolicyIsBytePinned() throws {
        let url = try #require(
            Bundle.main.url(
                forResource: "codex-model-variant-matrix.v1",
                withExtension: "json"
            )
        )
        let data = try Data(contentsOf: url)

        #expect(data.count == 1_479)
        #expect(
            SHA256.hash(data: data).map { String(format: "%02x", $0) }.joined()
                == "b6ad3f2047466a34da42241eae6b790f60bb835d9e6826cb77b51eb3fc558911"
        )
        guard case .available = CodexModelVariantPolicyLoader.load(data: data) else {
            Issue.record("pinned bundled policy must load")
            return
        }
    }

    @Test("Every resource failure is unavailable without a fallback matrix")
    func policyResourceFailuresFailClosed() throws {
        let sourceURL = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .appendingPathComponent("examples/agents/codex-model-variant-matrix.v1.json")
        let pinned = try Data(contentsOf: sourceURL)
        var mutated = pinned
        mutated[100] ^= 1

        let failures: [(Data?, CodexModelVariantPolicyUnavailableReason)] = [
            (nil, .missingResource),
            (Data(), .policyBytesMismatch),
            (pinned.dropLast().data, .policyBytesMismatch),
            (Data("not-json\n".utf8), .policyBytesMismatch),
            (pinned + Data([0x0a]), .policyBytesMismatch),
            (mutated, .policyBytesMismatch),
        ]
        for (data, reason) in failures {
            #expect(CodexModelVariantPolicyLoader.load(data: data) == .unavailable(reason))
        }

        struct UnreadableResource: Error {}
        #expect(
            CodexModelVariantPolicyLoader.loadResource(
                resourceURL: URL(fileURLWithPath: "/does/not/matter"),
                readData: { _ in throw UnreadableResource() }
            ) == .unavailable(.unreadableResource)
        )
        #expect(
            CodexModelVariantPolicyLoader.loadResource(
                resourceURL: nil,
                readData: { _ in pinned }
            ) == .unavailable(.missingResource)
        )
    }

    @Test("Formatter uses one policy authority for known and legacy tuples")
    func formatterCoversKnownLegacyMissingAndUnavailableStates() throws {
        let policy = try pinnedPolicy()

        #expect(
            CodexPlannedAssignmentFormatter.presentation(
                provider: "codex",
                model: "gpt-5.6-sol",
                effort: "max",
                policy: policy
            ) == .planned(
                variantToken: "Sol",
                visualSuffix: "gpt-5.6-sol · max · planned",
                fullAccessibilityValue: "Codex · Sol · gpt-5.6-sol · max · planned"
            )
        )
        #expect(
            CodexPlannedAssignmentFormatter.presentation(
                provider: "codex",
                model: "gpt-5.6-terra",
                effort: "high",
                policy: policy
            ).variantToken == "Terra"
        )
        #expect(
            CodexPlannedAssignmentFormatter.presentation(
                provider: "codex",
                model: "gpt-5.6-luna",
                effort: "high",
                policy: policy
            ).variantToken == "Luna"
        )
        #expect(
            CodexPlannedAssignmentFormatter.presentation(
                provider: "codex",
                model: "gpt-5.6",
                effort: "high",
                policy: policy
            ) == .planned(
                variantToken: "Codex",
                visualSuffix: "gpt-5.6 · high · planned",
                fullAccessibilityValue: "Codex · gpt-5.6 · high · planned"
            )
        )
        #expect(
            CodexPlannedAssignmentFormatter.presentation(
                provider: "codex",
                model: "gpt-5.6-sol",
                effort: nil,
                policy: policy
            ) == .planned(
                variantToken: "Sol",
                visualSuffix: "gpt-5.6-sol · Planned effort not recorded",
                fullAccessibilityValue: "Codex · Sol · gpt-5.6-sol · Planned effort not recorded"
            )
        )
        #expect(
            CodexPlannedAssignmentFormatter.presentation(
                provider: "codex",
                model: "custom-model",
                effort: "high",
                policy: policy
            ) == .unavailable
        )
        #expect(
            CodexPlannedAssignmentFormatter.presentation(
                provider: "claude_acp",
                model: "opus",
                effort: "high",
                policy: policy
            ) == .nonCodex(existingValue: "claude_acp · opus · high")
        )
    }

    @Test("Every formatter state stays identical across Overview and Stages copy")
    func formatterSurfaceTableUsesOneAccessibilityTruth() throws {
        let policy = try pinnedPolicy()
        let inputs: [(String, String?, String?)] = [
            ("codex", "gpt-5.6-sol", "max"),
            ("codex", "gpt-5.6-terra", "high"),
            ("codex", "gpt-5.6-luna", "high"),
            ("codex", "gpt-5.6", "high"),
            ("codex", "gpt-5.6-sol", nil),
            ("codex", "gpt-5.6-sol", "unknown"),
            ("codex", "unknown-model", "high"),
            ("claude_acp", "opus", "high"),
        ]

        for (provider, model, effort) in inputs {
            let presentation = CodexPlannedAssignmentFormatter.presentation(
                provider: provider,
                model: model,
                effort: effort,
                policy: policy
            )
            let overview = P036PlannedAssignmentAccessibility.overviewLabel(
                agentTitle: "Agent",
                status: "running",
                presentation: presentation,
                stage: "Stage",
                task: "Task",
                session: nil,
                eventCount: 1
            )
            let stage = P036PlannedAssignmentAccessibility.stageOccurrenceLabel(
                agentTitle: "Agent",
                task: "Task",
                status: "Running",
                presentation: presentation,
                executionCount: "1 attempt"
            )

            #expect(overview.components(separatedBy: presentation.fullAccessibilityValue).count == 2)
            #expect(stage.components(separatedBy: presentation.fullAccessibilityValue).count == 2)
        }
    }

    @Test("Unsafe or unknown Codex values are never interpolated")
    func unsafeInputsUseFixedUnavailableCopy() throws {
        let policy = try pinnedPolicy()
        let unsafeValues = [
            "gpt-5.6-sol\nspoof",
            "gpt-5.6-sol\u{202E}",
            "gpt-5.6-sol\u{200B}",
            "gpt-5.6-sol\u{2028}",
            "gpt-5.6-sol\\nspoof",
            String(repeating: "x", count: 300),
        ]

        for value in unsafeValues {
            let presentation = CodexPlannedAssignmentFormatter.presentation(
                provider: "codex",
                model: value,
                effort: "high",
                policy: policy
            )
            #expect(presentation == .unavailable)
            #expect(presentation.visualSuffix == "Planned assignment unavailable")
            #expect(!presentation.fullAccessibilityValue.contains(value))
        }
    }

    @Test("Candidate matching requires one exact frozen assignment")
    func candidateMatchingFailsClosed() throws {
        let policy = try pinnedPolicy()
        let candidate = P031PlannedAssignmentCandidate(
            agentID: "proposal_writer",
            provider: "codex",
            model: "gpt-5.6-terra",
            effort: "high"
        )

        #expect(
            P031PlannedAssignmentMatcher.presentation(
                agentID: "proposal_writer",
                provider: "codex",
                model: "gpt-5.6-terra",
                candidates: [candidate],
                policy: policy
            ).variantToken == "Terra"
        )
        #expect(
            P031PlannedAssignmentMatcher.presentation(
                agentID: "proposal_writer",
                provider: "codex",
                model: "gpt-5.6-terra",
                candidates: [],
                policy: policy
            ) == .unavailable
        )
        let missingEffort = P031PlannedAssignmentCandidate(
            agentID: "proposal_writer",
            provider: "codex",
            model: "gpt-5.6-terra",
            effort: nil
        )
        #expect(
            P031PlannedAssignmentMatcher.presentation(
                agentID: "proposal_writer",
                provider: "codex",
                model: "gpt-5.6-terra",
                candidates: [missingEffort],
                policy: policy
            ).fullAccessibilityValue
                == "Codex · Terra · gpt-5.6-terra · Planned effort not recorded"
        )
        #expect(
            P031PlannedAssignmentMatcher.presentation(
                agentID: "proposal_writer",
                provider: "codex",
                model: "gpt-5.6-terra",
                candidates: [candidate, candidate],
                policy: policy
            ) == .unavailable
        )
        #expect(
            P031PlannedAssignmentMatcher.presentation(
                agentID: "proposal_writer",
                provider: "codex",
                model: "gpt-5.6-sol",
                candidates: [candidate],
                policy: policy
            ) == .unavailable
        )
    }

    @Test("Stage topology uses the shared planned-assignment formatter")
    func stageTopologyUsesSharedFormatter() {
        let presentation = P031StageTopologyPresenter.presentation(
            for: P031RunStageTopologyReadModel(
                stageID: "state_current",
                label: "Current stage",
                order: 1,
                ownerAgentID: "lead_orchestrator",
                ownerAgentTitle: "Lead Orchestrator",
                status: "running",
                isCurrent: true,
                iteration: nil,
                attemptNumber: 1,
                startedAt: nil,
                completedAt: nil,
                approvalRequired: false,
                artifactCount: 0,
                communicationCount: 0,
                occurrences: [
                    P031RunStageTopologyOccurrenceReadModel(
                        agentID: "proposal_writer",
                        agentTitle: "Proposal Writer",
                        taskName: "Draft proposal",
                        status: "running",
                        provider: "codex",
                        model: "gpt-5.6-terra",
                        effort: "high",
                        executionCount: 1
                    )
                ],
                transitions: []
            )
        )

        #expect(presentation.occurrences.count == 1)
        #expect(presentation.occurrences.first?.plannedAssignment.variantToken == "Terra")
        #expect(
            presentation.occurrences.first?.plannedAssignment.fullAccessibilityValue
                == "Codex · Terra · gpt-5.6-terra · high · planned"
        )
    }

    @Test("Overview exposes only active agents mapped to the current stage")
    func currentStageFilteringFailsClosed() {
        let current = activeAgent(id: "current", stageID: "stage-current", selectionOrder: 2)
        let currentFirst = activeAgent(id: "current-first", stageID: "stage-current", selectionOrder: 1)
        let noncurrent = activeAgent(id: "noncurrent", stageID: "stage-other", selectionOrder: 0)
        let unresolved = activeAgent(id: "unresolved", stageID: nil, selectionOrder: 0)
        let completed = activeAgent(
            id: "completed",
            stageID: "stage-current",
            selectionOrder: 0,
            status: "completed"
        )
        let candidates = [current, noncurrent, unresolved, completed, currentFirst]

        #expect(
            RunsWorkbenchPresentationModel.activeAgents(
                candidates,
                forCurrentStageID: nil
            ).isEmpty
        )
        #expect(
            RunsWorkbenchPresentationModel.activeAgents(
                candidates,
                forCurrentStageID: "stage-current"
            ).map(\.id) == ["current-first", "current"]
        )
    }

    @Test("Accessibility labels use the normative component order")
    func accessibilityLabelsUseNormativeOrder() {
        let planned = plannedTerra
        #expect(
            P036PlannedAssignmentAccessibility.overviewLabel(
                agentTitle: "Proposal Writer",
                status: "running",
                presentation: planned,
                stage: "Proposal drafted",
                task: "Draft proposal",
                session: "session-1",
                eventCount: 4
            )
                == "Proposal Writer, running, Codex · Terra · gpt-5.6-terra · high · planned, Proposal drafted, Draft proposal, session session-1, 4 events"
        )
        #expect(
            P036PlannedAssignmentAccessibility.stageOccurrenceLabel(
                agentTitle: "Proposal Writer",
                task: "Draft proposal",
                status: "Running",
                presentation: planned,
                executionCount: "1 attempt"
            )
                == "Proposal Writer, Draft proposal, Running, Codex · Terra · gpt-5.6-terra · high · planned, 1 attempt"
        )

        for provider in ["claude_acp", "gemini_acp", "auggie_acp", "junie_acp", "unknown_provider"] {
            let existingValue = "\(provider) · existing-model · high"
            #expect(
                P036PlannedAssignmentAccessibility.stageOccurrenceLabel(
                    agentTitle: "Agent",
                    task: "Task",
                    status: "Running",
                    presentation: .nonCodex(existingValue: existingValue),
                    executionCount: "1 attempt"
                ) == "Agent, Task, Running, 1 attempt, \(existingValue)"
            )
        }
    }

    @Test("Non-Codex Stage visual metadata preserves the legacy component order")
    func nonCodexStageVisualOrderIsUnchanged() throws {
        for provider in ["claude_acp", "gemini_acp", "auggie_acp", "junie_acp", "unknown_provider"] {
            let existingValue = "\(provider) · existing-model · high"
            let occurrence = RunsWorkbenchPresentationModel.StageOccurrence(
                id: "\(provider)-occurrence",
                agentTitle: "Agent",
                taskName: "Task",
                statusText: "Running",
                providerLabel: existingValue,
                plannedAssignment: .nonCodex(existingValue: existingValue),
                executionCountLabel: "1 attempt"
            )
            let elements = hostedAccessibilityElements(
                P036StageOccurrenceDetailLine(occurrence: occurrence),
                width: 292
            )
            let detail = try #require(
                elements.first { $0.identifier == "p036-stage-legacy-detail" }
            )
            #expect(detail.text == "Task · Running · 1 attempt · \(existingValue)")
        }
    }

    @Test("Planned variant survives every supported Dynamic Type size")
    func plannedVariantSurvivesDynamicTypeRange() throws {
        let regularSizes: [DynamicTypeSize] = [.xSmall, .small, .medium, .large]
        for size in regularSizes {
            let plannedOnly = hostedAccessibilityElements(
                P036PlannedAssignmentLine(
                    presentation: plannedTerra,
                    trailingComponents: []
                )
                .environment(\.dynamicTypeSize, size),
                width: 1_000
            )
            let plannedSuffix = try #require(
                plannedOnly.first { $0.text == plannedTerra.visualSuffix }
            )
            let surfaces: [(String, CGFloat, AnyView)] = [
                (
                    "p036-stage-occurrence-content-occurrence-0",
                    292,
                    AnyView(
                        P036StageOccurrenceRowContent(
                            occurrence: stageOccurrence(
                                status: "Running",
                                taskName: "Draft a deliberately long proposal metadata suffix"
                            )
                        )
                    )
                ),
                (
                    "p036-active-agent-content-proposal-writer",
                    496,
                    AnyView(
                        P036ActiveAgentReadbackRowContent(
                            agent: activeAgent(
                                id: "proposal-writer",
                                stageID: "stage-current",
                                selectionOrder: 1,
                                taskLabel: "Draft a deliberately long proposal metadata suffix",
                                sessionID: "session-with-a-long-readback-identifier"
                            )
                        )
                    )
                ),
            ]
            for (containerID, width, surface) in surfaces {
                let elements = hostedAccessibilityElements(
                    surface.environment(\.dynamicTypeSize, size),
                    width: width,
                    height: 96
                )
                let container = try #require(elements.first { $0.identifier == containerID })
                let token = try #require(elements.first { $0.text == "Terra" })
                let suffix = try #require(
                    elements.first { $0.text?.hasPrefix(plannedTerra.visualSuffix) == true }
                )
                #expect((suffix.frame?.width ?? 0) + 0.5 >= (plannedSuffix.frame?.width ?? .infinity))
                expectContained(token, in: container)
                expectContained(suffix, in: container)
            }
        }

        let accessibilitySizes: [DynamicTypeSize] = [
            .xLarge,
            .xxLarge,
            .xxxLarge,
            .accessibility1,
            .accessibility2,
            .accessibility3,
            .accessibility4,
            .accessibility5,
        ]
        for size in accessibilitySizes {
            let stage = P036StageOccurrenceRowContent(
                occurrence: stageOccurrence(
                    status: "Running",
                    taskName: "Draft a deliberately long proposal metadata suffix"
                )
            )
            .environment(\.dynamicTypeSize, size)
            let overview = P036ActiveAgentReadbackRowContent(
                agent: activeAgent(
                    id: "proposal-writer",
                    stageID: "stage-current",
                    selectionOrder: 1,
                    taskLabel: "Draft a deliberately long proposal metadata suffix",
                    sessionID: "session-with-a-long-readback-identifier"
                )
            )
            .environment(\.dynamicTypeSize, size)
            for (containerID, width, surface) in [
                ("p036-stage-occurrence-content-occurrence-0", CGFloat(292), AnyView(stage)),
                ("p036-active-agent-content-proposal-writer", CGFloat(496), AnyView(overview)),
            ] {
                let natural = hostedAccessibilityElements(surface, width: 1_000, height: 120)
                let constrained = hostedAccessibilityElements(surface, width: width, height: 120)
                let naturalToken = try #require(natural.first { $0.text == "Terra" })
                let naturalSuffix = try #require(
                    natural.first { $0.text?.hasPrefix(plannedTerra.visualSuffix) == true }
                )
                let container = try #require(
                    constrained.first { $0.identifier == containerID }
                )
                let token = try #require(constrained.first { $0.text == "Terra" })
                let suffix = try #require(
                    constrained.first { $0.text?.hasPrefix(plannedTerra.visualSuffix) == true }
                )
                #expect(abs((token.frame?.width ?? 0) - (naturalToken.frame?.width ?? 0)) < 0.5)
                #expect((suffix.frame?.width ?? .infinity) < (naturalSuffix.frame?.width ?? 0))
                expectContained(token, in: container)
                expectContained(suffix, in: container)
            }

            let rowStrings = hostedAccessibilityStrings(
                P036StageOccurrenceRow(occurrence: stageOccurrence(status: "Running"))
                    .environment(\.dynamicTypeSize, size),
                width: 292
            )
            #expect(rowStrings.filter { $0.contains(plannedTerra.fullAccessibilityValue) }.count == 1)
        }
    }

    @Test("Hosted rows expose exact help and remain within planned bounds")
    func hostedRowsExposeHelpAndBounds() throws {
        let active = activeAgent(id: "proposal-writer", stageID: "stage-current", selectionOrder: 1)
        let overviewElements = hostedAccessibilityElements(
            P036ActiveAgentReadbackRow(agent: active),
            width: 496
        )
        let overview = try #require(
            overviewElements.first { $0.identifier == "p036-active-agent-proposal-writer" }
        )
        #expect(overview.help == plannedTerra.fullAccessibilityValue)
        #expect((overview.frame?.width ?? 0) > 0)
        #expect((overview.frame?.width ?? .infinity) <= 496)

        let stageElements = hostedAccessibilityElements(
            P036StageTopologyCard(stage: stageCard(occurrenceCount: 1)),
            width: 316,
            height: 234
        )
        let stage = try #require(
            stageElements.first { $0.identifier == "p036-stage-occurrence-occurrence-0" }
        )
        #expect(stage.help == plannedTerra.fullAccessibilityValue)
        #expect((stage.frame?.width ?? 0) > 0)
        #expect((stage.frame?.width ?? .infinity) <= 292)
        #expect((stage.frame?.height ?? .infinity) <= 210)
    }

    @Test("Hosted Overview row exposes the combined planned label exactly once")
    func hostedOverviewAccessibilityUsesCombinedLabel() {
        let agent = activeAgent(id: "proposal-writer", stageID: "stage-current", selectionOrder: 1)
        let strings = hostedAccessibilityStrings(
            P036ActiveAgentReadbackRow(agent: agent),
            width: 520
        )
        let expected = "proposal-writer, running, Codex · Terra · gpt-5.6-terra · high · planned, stage-current, task, 1 events"

        #expect(strings.contains(expected))
        #expect(strings.filter { $0.contains(plannedTerra.fullAccessibilityValue) }.count == 1)
    }

    @Test("Hosted topology geometry remains fixed for one two and five occurrences")
    func hostedTopologyGeometryIsStable() {
        for count in [1, 2, 5] {
            let size = hostedSize(
                P036StageTopologyCard(stage: stageCard(occurrenceCount: count))
            )
            #expect(abs(size.width - 316) < 0.5)
            #expect(abs(size.height - 234) < 0.5)
        }

        for units in [1, 2, 5] {
            let expectedInner = CGFloat(units) * 210 + CGFloat(units - 1) * 12
            let size = hostedSize(
                P036StageTopologyCard(
                    stage: stageCard(occurrenceCount: 1),
                    heightUnits: units
                )
            )
            #expect(abs(size.width - 316) < 0.5)
            #expect(abs(size.height - (expectedInner + 24)) < 0.5)
        }

        let connector = hostedSize(
            P036StageTopologyConnectorView(style: .primary)
                .frame(width: 34, height: 210)
        )
        #expect(abs(connector.width - 34) < 0.5)
        #expect(abs(connector.height - 210) < 0.5)
    }

    @Test("Hosted Stage card exposes each planned occurrence exactly once")
    func hostedStageAccessibilityHasOneLabelPerOccurrence() {
        let host = NSHostingView(
            rootView: P036StageTopologyCard(stage: stageCard(occurrenceCount: 2))
        )
        let frame = NSRect(x: 0, y: 0, width: 316, height: 234)
        let window = NSWindow(
            contentRect: frame,
            styleMask: [.titled],
            backing: .buffered,
            defer: false
        )
        let windowTitle = "Codex planned variant accessibility proof"
        window.title = windowTitle
        window.isReleasedWhenClosed = false
        window.contentView = host
        window.makeKeyAndOrderFront(nil)
        defer {
            window.orderOut(nil)
            window.contentView = nil
            window.close()
        }
        RunLoop.current.run(until: Date().addingTimeInterval(0.05))
        window.displayIfNeeded()

        enableSwiftUIAccessibilityTree()
        let labels = accessibilityStrings(from: host)
        let plannedValue = plannedTerra.fullAccessibilityValue
        #expect(labels.filter { $0.contains(plannedValue) }.count == 2)
    }

    @Test("Status-only refresh preserves occurrence identity and planned labels")
    func statusOnlyRefreshPreservesPlannedAccessibility() {
        let running = stageCard(occurrenceCount: 2, occurrenceStatus: "Running")
        let paused = stageCard(occurrenceCount: 2, occurrenceStatus: "Paused")
        #expect(running.occurrences.map(\.id) == paused.occurrences.map(\.id))
        #expect(hostedSize(P036StageTopologyCard(stage: running)) == hostedSize(P036StageTopologyCard(stage: paused)))

        let host = NSHostingView(rootView: P036StageTopologyCard(stage: running))
        let frame = NSRect(x: 0, y: 0, width: 316, height: 234)
        let window = NSWindow(
            contentRect: frame,
            styleMask: [.titled],
            backing: .buffered,
            defer: false
        )
        window.title = "Codex planned variant status refresh proof"
        window.isReleasedWhenClosed = false
        window.contentView = host
        window.makeKeyAndOrderFront(nil)
        defer {
            window.orderOut(nil)
            window.contentView = nil
            window.close()
        }

        enableSwiftUIAccessibilityTree()
        RunLoop.current.run(until: Date().addingTimeInterval(0.05))
        let before = accessibilityStrings(from: host)

        host.rootView = P036StageTopologyCard(stage: paused)
        RunLoop.current.run(until: Date().addingTimeInterval(0.05))
        let after = accessibilityStrings(from: host)
        let plannedValue = plannedTerra.fullAccessibilityValue

        #expect(before.filter { $0.contains(plannedValue) }.count == 2)
        #expect(after.filter { $0.contains(plannedValue) }.count == 2)
        #expect(after.filter { $0.contains(", Paused, " + plannedValue + ",") }.count == 2)
    }

    @Test("Production run-detail selection and focus survive status-only refresh")
    func statusRefreshPreservesProductionInteractionState() throws {
        let running = productionRunDetail(status: "Running")
        let paused = productionRunDetail(status: "Paused")
        let model = P031ThinReadDashboardModel.testLoaded(runDetail: running)
        let workbench = RunsWorkbenchPresentationModel()
        workbench.populate(from: running)
        let host = NSHostingView(
            rootView: RunsHomeView(
                model: model,
                workbench: workbench,
                initialTab: .stages
            )
        )
        let window = NSWindow(
            contentRect: NSRect(x: 0, y: 0, width: 1_200, height: 800),
            styleMask: [.titled, .resizable],
            backing: .buffered,
            defer: false
        )
        window.isReleasedWhenClosed = false
        window.contentView = host
        window.makeKeyAndOrderFront(nil)
        defer {
            window.orderOut(nil)
            window.contentView = nil
            window.close()
        }
        enableSwiftUIAccessibilityTree()
        RunLoop.current.run(until: Date().addingTimeInterval(0.2))

        let picker = try #require(
            descendants(of: NSSegmentedControl.self, in: host)
                .first { $0.segmentCount == P031RunDetailTab.allCases.count }
        )
        #expect(picker.selectedSegment == 1)
        #expect(window.makeFirstResponder(picker))
        let occurrenceID = "p036-stage-occurrence-stage-current-proposal_writer-Draft proposal"
        let before = try #require(
            accessibilityElements(from: host).first { $0.identifier == occurrenceID }
        )
        #expect(before.label?.contains(", Running, ") == true)

        workbench.populate(from: paused)
        RunLoop.current.run(until: Date().addingTimeInterval(0.2))

        let pickerAfter = try #require(
            descendants(of: NSSegmentedControl.self, in: host)
                .first { $0.segmentCount == P031RunDetailTab.allCases.count }
        )
        let after = try #require(
            accessibilityElements(from: host).first { $0.identifier == occurrenceID }
        )
        #expect(pickerAfter.selectedSegment == 1)
        #expect(pickerAfter === picker)
        #expect(pickerAfter.window === window)
        #expect(window.firstResponder === pickerAfter)
        #expect(after.label?.contains(", Paused, ") == true)
        #expect(after.frame == before.frame)
    }

    private func pinnedPolicy() throws -> CodexModelVariantPolicyAvailability {
        let sourceURL = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .appendingPathComponent("examples/agents/codex-model-variant-matrix.v1.json")
        let policy = CodexModelVariantPolicyLoader.load(data: try Data(contentsOf: sourceURL))
        guard case .available = policy else {
            Issue.record("source policy must load")
            return .unavailable(.policySchemaInvalid)
        }
        return policy
    }

    private func activeAgent(
        id: String,
        stageID: String?,
        selectionOrder: Int?,
        status: String = "running",
        taskLabel: String = "task",
        sessionID: String? = nil
    ) -> RunsWorkbenchPresentationModel.ActiveTimelineAgent {
        RunsWorkbenchPresentationModel.ActiveTimelineAgent(
            id: id,
            title: id,
            providerID: "codex",
            modelID: "gpt-5.6-terra",
            plannedAssignment: .planned(
                variantToken: "Terra",
                visualSuffix: "gpt-5.6-terra · high · planned",
                fullAccessibilityValue: "Codex · Terra · gpt-5.6-terra · high · planned"
            ),
            stageID: stageID,
            stageLabel: stageID,
            taskLabel: taskLabel,
            status: status,
            sessionID: sessionID,
            latestAt: Date(timeIntervalSince1970: 1),
            eventCount: 1,
            selectionOrder: selectionOrder,
            selectionUnavailableReason: nil
        )
    }

    private var plannedTerra: CodexPlannedAssignmentPresentation {
        .planned(
            variantToken: "Terra",
            visualSuffix: "gpt-5.6-terra · high · planned",
            fullAccessibilityValue: "Codex · Terra · gpt-5.6-terra · high · planned"
        )
    }

    private func stageCard(
        occurrenceCount: Int,
        occurrenceStatus: String = "Running"
    ) -> RunsWorkbenchPresentationModel.StageCard {
        RunsWorkbenchPresentationModel.StageCard(
            id: "state_current",
            ordinal: 4,
            title: "Proposal reviewed",
            ownerAgentTitle: "Lead Orchestrator",
            status: "active",
            statusText: "Running",
            isCurrent: true,
            iterationText: nil,
            attemptText: "Attempt 1",
            startedLabel: nil,
            completedLabel: nil,
            durationLabel: nil,
            evidenceLabels: [],
            artifactCount: 0,
            communicationCount: 0,
            approvalRequired: false,
            occurrences: (0..<occurrenceCount).map { index in
                stageOccurrence(index: index, status: occurrenceStatus)
            },
            hiddenOccurrenceCount: 0,
            transitions: []
        )
    }

    private func stageOccurrence(
        index: Int = 0,
        status: String,
        taskName: String = "Draft proposal"
    ) -> RunsWorkbenchPresentationModel.StageOccurrence {
        RunsWorkbenchPresentationModel.StageOccurrence(
            id: "occurrence-\(index)",
            agentTitle: "Proposal Writer \(index + 1)",
            taskName: taskName,
            statusText: status,
            providerLabel: plannedTerra.visualSuffix,
            plannedAssignment: plannedTerra,
            executionCountLabel: "1 attempt"
        )
    }

    private func productionRunDetail(status: String) -> P031RunDetailPresentation {
        let freshness = P031FreshnessSnapshot(state: .live, lastCheckedAt: Date())
        return P031RunDetailPresentation(
            title: "Model variant proof",
            runID: "run-model-variant-proof",
            workflowLabel: "model_variant_proof",
            statusLabel: "Running",
            progressLabel: nil,
            pendingApprovalsLabel: nil,
            ideaContext: nil,
            stageTransitions: [],
            stageTopology: [
                P031StageTopologyPresentation(
                    stageID: "stage-current",
                    ordinal: 4,
                    title: "Proposal reviewed",
                    ownerAgentID: "lead_orchestrator",
                    ownerAgentTitle: "Lead Orchestrator",
                    status: "running",
                    statusText: "Running",
                    isCurrent: true,
                    iterationText: nil,
                    attemptText: "Attempt 1",
                    startedLabel: nil,
                    completedLabel: nil,
                    durationLabel: nil,
                    approvalRequired: false,
                    artifactCount: 0,
                    communicationCount: 1,
                    occurrences: [
                        P031StageTopologyOccurrencePresentation(
                            agentID: "proposal_writer",
                            agentTitle: "Proposal Writer",
                            taskName: "Draft proposal",
                            statusText: status,
                            providerLabel: plannedTerra.visualSuffix,
                            plannedAssignment: plannedTerra,
                            executionCountLabel: "1 attempt"
                        )
                    ],
                    transitions: []
                )
            ],
            approvalRows: [],
            artifactRows: [],
            artifactViewerRows: [],
            reportRows: [],
            activeAgentTimelineEntries: [
                P031ActiveAgentTimelinePresentation(
                    id: "proposal-writer",
                    title: "Proposal Writer",
                    detail: "Draft proposal",
                    timestamp: Date(timeIntervalSince1970: 1),
                    stageID: "stage-current",
                    providerID: "codex",
                    modelID: "gpt-5.6-terra",
                    plannedAssignment: plannedTerra,
                    stageLabel: "Proposal reviewed",
                    taskLabel: "Draft proposal",
                    status: status.lowercased(),
                    eventCount: 1,
                    selectionOrder: 1,
                    agentID: "proposal_writer",
                    sessionID: nil
                )
            ],
            catalogContext: nil,
            freshness: freshness,
            refreshFeedbackText: "Live projection",
            emptyStateTitle: nil,
            errorDescription: nil,
            rawStatus: "running",
            failedStages: 0
        )
    }

    private func hostedSize<Content: View>(_ content: Content) -> CGSize {
        let host = NSHostingView(rootView: content)
        host.layoutSubtreeIfNeeded()
        return host.fittingSize
    }

    private func enableSwiftUIAccessibilityTree() {
        let application = NSApplication.shared
        let selector = NSSelectorFromString("setAccessibilityEnhancedUserInterface:")
        guard application.responds(to: selector) else {
            Issue.record("AppKit test host cannot enable the SwiftUI accessibility tree")
            return
        }
        application.setValue(true, forKey: "accessibilityEnhancedUserInterface")
    }

    private func hostedAccessibilityStrings<Content: View>(
        _ content: Content,
        width: CGFloat
    ) -> [String] {
        let frame = NSRect(x: 0, y: 0, width: width, height: 234)
        let host = NSHostingView(rootView: content)
        let window = NSWindow(
            contentRect: frame,
            styleMask: [.titled],
            backing: .buffered,
            defer: false
        )
        window.title = "Codex planned variant hosted proof"
        window.isReleasedWhenClosed = false
        window.contentView = host
        window.makeKeyAndOrderFront(nil)
        defer {
            window.orderOut(nil)
            window.contentView = nil
            window.close()
        }

        enableSwiftUIAccessibilityTree()
        RunLoop.current.run(until: Date().addingTimeInterval(0.05))
        window.displayIfNeeded()
        return accessibilityStrings(from: host)
    }

    private func hostedAccessibilityElements<Content: View>(
        _ content: Content,
        width: CGFloat,
        height: CGFloat = 234
    ) -> [P036AccessibilityElementSnapshot] {
        let frame = NSRect(x: 0, y: 0, width: width, height: height)
        let host = NSHostingView(rootView: content)
        let window = NSWindow(
            contentRect: frame,
            styleMask: [.titled],
            backing: .buffered,
            defer: false
        )
        window.title = "Codex planned variant visual proof"
        window.isReleasedWhenClosed = false
        window.contentView = host
        window.makeKeyAndOrderFront(nil)
        defer {
            window.orderOut(nil)
            window.contentView = nil
            window.close()
        }

        enableSwiftUIAccessibilityTree()
        RunLoop.current.run(until: Date().addingTimeInterval(0.05))
        window.displayIfNeeded()
        return accessibilityElements(from: host)
    }

    private func accessibilityStrings(from root: NSView) -> [String] {
        var result: [String] = []
        var visited = Set<ObjectIdentifier>()

        func visit(_ element: Any) {
            guard let object = element as? NSObject else { return }
            guard visited.insert(ObjectIdentifier(object)).inserted else { return }

            for key in ["accessibilityLabel", "accessibilityValue"] {
                guard object.responds(to: NSSelectorFromString(key)) else { continue }
                if let value = object.value(forKey: key) as? String, !value.isEmpty {
                    result.append(value)
                }
            }
            if object.responds(to: NSSelectorFromString("accessibilityChildren")),
               let children = object.value(forKey: "accessibilityChildren") as? [Any] {
                for child in children {
                    visit(child)
                }
            }
        }

        visit(root)
        return result
    }

    private func accessibilityElements(from root: NSView) -> [P036AccessibilityElementSnapshot] {
        var result: [P036AccessibilityElementSnapshot] = []
        var visited = Set<ObjectIdentifier>()

        func stringValue(_ object: NSObject, _ key: String) -> String? {
            guard object.responds(to: NSSelectorFromString(key)) else { return nil }
            return object.value(forKey: key) as? String
        }

        func visit(_ element: Any) {
            guard let object = element as? NSObject else { return }
            guard visited.insert(ObjectIdentifier(object)).inserted else { return }
            let frame = object.responds(to: NSSelectorFromString("accessibilityFrame"))
                ? (object.value(forKey: "accessibilityFrame") as? NSValue)?.rectValue
                : nil
            result.append(
                P036AccessibilityElementSnapshot(
                    identifier: stringValue(object, "accessibilityIdentifier"),
                    label: stringValue(object, "accessibilityLabel"),
                    value: stringValue(object, "accessibilityValue"),
                    help: stringValue(object, "accessibilityHelp"),
                    frame: frame
                )
            )
            if object.responds(to: NSSelectorFromString("accessibilityChildren")),
               let children = object.value(forKey: "accessibilityChildren") as? [Any] {
                for child in children {
                    visit(child)
                }
            }
        }

        visit(root)
        return result
    }

    private func expectContained(
        _ child: P036AccessibilityElementSnapshot,
        in container: P036AccessibilityElementSnapshot
    ) {
        guard let childFrame = child.frame, let containerFrame = container.frame else {
            Issue.record("hosted layout proof requires concrete accessibility frames")
            return
        }
        #expect(containerFrame.insetBy(dx: -0.5, dy: -0.5).contains(childFrame))
    }

    private func descendants<T: NSView>(of type: T.Type, in root: NSView) -> [T] {
        var matches: [T] = []
        if let match = root as? T {
            matches.append(match)
        }
        for child in root.subviews {
            matches.append(contentsOf: descendants(of: type, in: child))
        }
        return matches
    }
}

private struct P036AccessibilityElementSnapshot {
    let identifier: String?
    let label: String?
    let value: String?
    let help: String?
    let frame: NSRect?

    var text: String? { value ?? label }
}

private extension Data.SubSequence {
    var data: Data { Data(self) }
}
