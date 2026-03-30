import Testing
import Foundation
@testable import Chainworks_Forge

@Suite("Delivery Services")
struct DeliveryServicesTests {

    // MARK: - DeliveryConfiguration

    @Test("DeliveryConfiguration encodes and decodes correctly")
    func deliveryConfigCodable() throws {
        let config = DeliveryConfiguration(
            profileID: "sample-1",
            profileLabel: "Sample Repo",
            sampleProfileID: "dogfood-1",
            repoIdentifier: "chainworks-forge",
            repoRoot: "/tmp/test-repo",
            baseBranch: "main",
            worktreeBasePath: "/tmp/worktrees",
            targetBranch: "dogfood/test",
            releaseTargetID: "sandbox-1",
            releaseTargetLabel: "Sandbox",
            releaseMode: .sandbox
        )

        let data = try JSONEncoder().encode(config)
        let decoded = try JSONDecoder().decode(DeliveryConfiguration.self, from: data)

        #expect(decoded.profileID == "sample-1")
        #expect(decoded.repoIdentifier == "chainworks-forge")
        #expect(decoded.baseBranch == "main")
        #expect(decoded.releaseMode == .sandbox)
        #expect(decoded.worktreeBasePath == "/tmp/worktrees")
    }

    @Test("RepositoryProfile produces valid DeliveryConfiguration")
    func repoProfileToDeliveryConfig() {
        let profile = RepositoryProfile(
            id: "profile-1",
            label: "Test Repo",
            repoIdentifier: "test-repo",
            repoRoot: "/tmp/repo",
            defaultBaseBranch: "main",
            defaultWorktreeBasePath: "/tmp/worktrees",
            defaultTargetBranch: "release/v1",
            defaultReleaseTargetID: "staging-1",
            sampleProfileID: nil
        )

        let config = profile.toDeliveryConfiguration(releaseMode: .staging, releaseTargetLabel: "Staging")

        #expect(config.profileID == "profile-1")
        #expect(config.repoRoot == "/tmp/repo")
        #expect(config.releaseMode == .staging)
        #expect(config.targetBranch == "release/v1")
    }

    @Test("DeliveryConfiguration canonicalizes repository identity across label and remote formats")
    func deliveryConfigurationCanonicalizesRepoIdentity() {
        let config = DeliveryConfiguration(
            profileID: "chainworks_forge_self",
            profileLabel: "Chainworks Forge (Self)",
            sampleProfileID: nil,
            repoIdentifier: "Chainworks Forge",
            repoRoot: "/Users/test/Chainworks Forge",
            baseBranch: "main",
            worktreeBasePath: "/tmp/worktrees",
            targetBranch: "dogfood/full-mvp",
            releaseTargetID: "sandbox_local",
            releaseTargetLabel: "Local Sandbox",
            releaseMode: .sandbox
        )

        #expect(config.repoIdentifier == "chainworks-forge")
        #expect(
            RepositoryIdentityNormalizer.canonicalIdentifier(from: "git@github.com:example/chainworks-forge.git")
                == config.repoIdentifier
        )
    }

    // MARK: - RepoSafetyGuard

    @Test("RepoSafetyGuard allows paths within workspace")
    func safetyGuardAllowsWorkspace() throws {
        try RepoSafetyGuard.validatePath(
            "/tmp/workspace/artifacts/test.json",
            workspaceRoot: "/tmp/workspace",
            worktreeRoot: nil
        )
    }

    @Test("RepoSafetyGuard allows paths within worktree")
    func safetyGuardAllowsWorktree() throws {
        try RepoSafetyGuard.validatePath(
            "/tmp/worktree/src/main.swift",
            workspaceRoot: "/tmp/workspace",
            worktreeRoot: "/tmp/worktree"
        )
    }

    @Test("RepoSafetyGuard rejects paths outside boundary")
    func safetyGuardRejectsOutside() {
        #expect(throws: RepoSafetyGuard.SafetyViolation.self) {
            try RepoSafetyGuard.validatePath(
                "/etc/passwd",
                workspaceRoot: "/tmp/workspace",
                worktreeRoot: "/tmp/worktree"
            )
        }
    }

    @Test("RepoSafetyGuard rejects path traversal attacks")
    func safetyGuardRejectsTraversal() {
        #expect(throws: RepoSafetyGuard.SafetyViolation.self) {
            try RepoSafetyGuard.validatePath(
                "/tmp/workspace/../../../etc/passwd",
                workspaceRoot: "/tmp/workspace",
                worktreeRoot: nil
            )
        }
    }

    @Test("RepoSafetyGuard validates repo identity match")
    func safetyGuardRepoIdentity() throws {
        try RepoSafetyGuard.validateRepoIdentity(
            expectedRepoRoot: "/tmp/repo",
            actualRepoRoot: "/tmp/repo"
        )
    }

    @Test("RepoSafetyGuard rejects repo identity mismatch")
    func safetyGuardRepoMismatch() {
        #expect(throws: RepoSafetyGuard.SafetyViolation.self) {
            try RepoSafetyGuard.validateRepoIdentity(
                expectedRepoRoot: "/tmp/repo-a",
                actualRepoRoot: "/tmp/repo-b"
            )
        }
    }

    @Test("RepoSafetyGuard rejects unprovisioned worktree")
    func safetyGuardWorktreeNotProvisioned() {
        #expect(throws: RepoSafetyGuard.SafetyViolation.self) {
            try RepoSafetyGuard.validateWorktreeReady(worktreeRoot: nil)
        }
    }

    // MARK: - DeliveryPreflightService

    @Test("DeliveryPreflightService validates missing repo root")
    func preflightMissingRepo() async {
        let config = DeliveryConfiguration(
            profileID: nil, profileLabel: nil, sampleProfileID: nil,
            repoIdentifier: "test",
            repoRoot: "/nonexistent/path/to/repo",
            baseBranch: "main",
            worktreeBasePath: "/tmp/worktrees",
            targetBranch: "release",
            releaseTargetID: "sandbox-1",
            releaseTargetLabel: "Sandbox",
            releaseMode: .sandbox
        )

        let service = DeliveryPreflightService()
        let result = await service.validate(config)

        #expect(!result.passed)
        #expect(result.failedChecks.contains { $0.id == "repo_root" })
    }

    @Test("DeliveryPreflightService validates empty release target")
    func preflightEmptyReleaseTarget() async {
        let config = DeliveryConfiguration(
            profileID: nil, profileLabel: nil, sampleProfileID: nil,
            repoIdentifier: "test",
            repoRoot: "/tmp",
            baseBranch: "main",
            worktreeBasePath: "/tmp/worktrees",
            targetBranch: "release",
            releaseTargetID: "",
            releaseTargetLabel: "",
            releaseMode: .sandbox
        )

        let service = DeliveryPreflightService()
        let result = await service.validate(config)

        #expect(result.failedChecks.contains { $0.id == "release_target" })
    }

    // MARK: - DeliveryReceiptBuilder

    @Test("DeliveryReceiptBuilder produces valid receipt")
    func receiptBuilder() {
        let config = DeliveryConfiguration(
            profileID: "p1", profileLabel: "Test", sampleProfileID: nil,
            repoIdentifier: "test-repo",
            repoRoot: "/tmp/repo",
            baseBranch: "main",
            worktreeBasePath: "/tmp/wt",
            targetBranch: "release",
            releaseTargetID: "sandbox-1",
            releaseTargetLabel: "Sandbox",
            releaseMode: .sandbox
        )

        let receipt = DeliveryReceiptBuilder.buildReceipt(
            runID: UUID(),
            workflowID: "full_mvp_live",
            ideaTitle: "Test Idea",
            deliveryConfig: config,
            worktreeRoot: "/tmp/wt/cw-test-abc123",
            baseRevision: "abc123def",
            releaseResult: nil,
            implementationReviewStatus: nil
        )

        #expect(receipt.workflowID == "full_mvp_live")
        #expect(receipt.ideaTitle == "Test Idea")
        #expect(receipt.deliveryConfig.releaseMode == .sandbox)
    }

    // MARK: - ReleaseMode

    @Test("ReleaseMode encodes and decodes", arguments: [ReleaseMode.sandbox, .staging])
    func releaseModeCodable(_ mode: ReleaseMode) throws {
        let data = try JSONEncoder().encode(mode)
        let decoded = try JSONDecoder().decode(ReleaseMode.self, from: data)
        #expect(decoded == mode)
    }
}
