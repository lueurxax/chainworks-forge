import Foundation
import CryptoKit

/// Builds a stable SHA-256 compatibility fingerprint for session reuse decisions (§6.1).
///
/// A session may be reused only when this fingerprint still matches. The fingerprint
/// covers the full "reusable static prefix" — everything that was true when the session
/// was created and that must still be true for reuse to be safe.
///
/// Components (per §6.1):
/// - `agentID`
/// - resolved provider family
/// - resolved model
/// - resolved effort
/// - static task/instruction scaffold (via system prompt)
/// - system prompt framing (via system prompt)
/// - tool inventory and tool configuration
/// - permission profile
/// - workspace mode (`read_only` vs `read_write`)
/// - effective working directory
/// - skill snapshot hash / runtime injected skill content hash
/// - relevant system prompt framing version
struct BindingFingerprintBuilder {
    static func build(
        agent: ResolvedAgent,
        provider: String,
        model: String,
        effort: String,
        systemPrompt: String,
        workingDirectory: String,
        workspaceMode: String,
        strategyFingerprintMaterial: String? = nil
    ) -> String {
        var components: [String] = []

        // Core identity
        components.append(agent.id)
        components.append(provider)
        components.append(model)
        components.append(effort)

        // Static task/instruction scaffold and system prompt framing (§6.1)
        // The full system prompt includes framing, scaffold, and injected skill content.
        components.append(systemPrompt)

        // Permission profile (§6.1)
        components.append(agent.permissionProfile)

        // Workspace isolation (§6.1)
        components.append(workingDirectory)
        components.append(workspaceMode)

        // Skill snapshot hash / runtime-injected skill content hash (§6.1)
        // Hash the actual resolved prompt content (which contains the skill-injected body),
        // not just the skill reference name. This ensures that if the skill body changes
        // while the skillRef name stays the same, the fingerprint detects the drift.
        // agent.prompt is the runtime-resolved skill content from catalog resolution.
        components.append(DefinitionHasher.hashString(agent.prompt))
        // Also include skillRef and skillRole as structural identity markers.
        components.append(agent.skillRef)
        components.append(agent.skillRole ?? "")

        // Tool inventory and tool configuration (§6.1)
        // The agent's mode, output contract, max turns, and temperature collectively
        // define the tool/runtime configuration surface. Changes to any of these
        // alter the effective tool contract the session was created under.
        components.append(agent.mode)
        components.append(agent.outputContract ?? "")
        components.append(String(agent.maxTurns))
        components.append(String(agent.temperature))

        // Input/output contract surface (§6.1 — tool inventory)
        // Sorted for deterministic hashing regardless of declaration order.
        components.append(agent.inputs.sorted().joined(separator: ","))
        components.append(agent.outputs.sorted().joined(separator: ","))

        // Worktree write policy (affects tool permissions)
        components.append(agent.worktreeWriteEnabled ? "worktree_write" : "worktree_readonly")

        // System prompt framing version (§6.1)
        // Implicit in the system prompt content hash above, but we also include
        // the backend profile ID as a proxy for provider-specific framing version.
        components.append(agent.backendProfileID ?? "default")

        // Proposal 019: strategy handoff fingerprint material (when strategy mode is active).
        if let strategyFingerprintMaterial, !strategyFingerprintMaterial.isEmpty {
            components.append("strategy:")
            components.append(strategyFingerprintMaterial)
        }

        let combined = components.joined(separator: "|")
        let data = Data(combined.utf8)
        let hash = SHA256.hash(data: data)
        return hash.map { String(format: "%02hhx", $0) }.joined()
    }
}
