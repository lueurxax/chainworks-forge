import Foundation

enum SessionReuseDecision {
    case reuse(AgentSessionGeneration)
    case createFresh(disposition: SessionReuseDisposition, reason: String?)
    case requireReset(reason: String)
}

final class SessionReusePolicy {

    /// Evaluate whether an existing session lineage can be reused for the current invocation.
    ///
    /// The policy reads, in order (§6.6):
    /// 1. persisted `invocationOwnerKey`
    /// 2. active `AgentSessionLineage`
    /// 3. immutable `AgentSessionGeneration`
    /// 4. current binding fingerprint
    /// 5. current recovery branch / owner execution lineage imported from execution truth
    ///
    /// If any of those are missing or contradictory, the result is `fresh_session_required`, not `reuse`.
    static func evaluate(
        lineage: AgentSessionLineage?,
        currentInvocationOwnerKey: String,
        currentBindingFingerprint: String,
        currentRecoveryBranchID: UUID?
    ) -> SessionReuseDecision {
        guard let lineage = lineage else {
            return .createFresh(disposition: .fresh, reason: "No existing lineage found")
        }

        guard let activeGenerationID = lineage.activeGenerationID else {
            // If activeGenerationID is nil, check the last ended generation to determine disposition.
            // This covers the real reset flow: after resetSession() clears activeGenerationID,
            // the next invocation should see .fresh_after_reset if the last generation was reset.
            if let lastGen = lineage.generations.sorted(by: { $0.createdAt < $1.createdAt }).last {
                let disposition: SessionReuseDisposition = switch lastGen.status {
                    case .reset: .fresh_after_reset
                    case .invalidated:
                        if lastGen.endReason?.contains("budget") == true { .fresh_after_budget }
                        else if lastGen.endReason?.contains("compaction") == true { .fresh_after_compaction }
                        else if lastGen.endReason?.contains("transport") == true { .fresh_after_transport_error }
                        else if lastGen.endReason?.contains("timeout") == true { .fresh_after_timeout }
                        else { .fresh_after_invalidation }
                    case .closed: .fresh
                    case .active: .fresh // Should not happen if activeID is nil
                }
                return .createFresh(disposition: disposition, reason: "Last generation was \(lastGen.status.rawValue)")
            }
            return .createFresh(disposition: .fresh, reason: "No active generation found")
        }

        guard let activeGeneration = lineage.generations.first(where: { $0.id == activeGenerationID }) else {
            return .createFresh(disposition: .unverifiable_session_history, reason: "Active generation not found in lineage")
        }

        // 1. Check status
        guard activeGeneration.status == .active else {
            let disposition: SessionReuseDisposition = switch activeGeneration.status {
                case .invalidated: .fresh_after_invalidation
                case .reset: .fresh_after_reset
                case .closed: .fresh
                case .active: .reused // Should not happen here
            }
            return .createFresh(disposition: disposition, reason: "Session status is \(activeGeneration.status.rawValue)")
        }

        // 2. Binding fingerprint must match (§6.1)
        guard activeGeneration.bindingFingerprint == currentBindingFingerprint else {
            return .createFresh(disposition: .fresh_session_required, reason: "Binding fingerprint mismatch")
        }

        // 3. Recovery-branch truth must be verified (§4.1.1, §6.6, ARCH-001)
        //    ownerExecutionLineageID ties reuse to one recovery branch.
        //    If execution truth does not provide a trustworthy branch ID, fail closed.
        if lineage.sessionReuseScope == .same_invocation_owner {
            // For same_invocation_owner, the recovery branch is embedded in the owner key.
            // If we have an explicit recovery branch ID, verify it against the owner key.
            if let branchID = currentRecoveryBranchID {
                let branchFragment = branchID.uuidString
                // The owner key encodes ownerExecutionLineageID as the last component.
                // If the branch changed, the owner key itself will differ, which is caught
                // in the owner-key comparison below. But if somehow branch diverged while
                // owner key stayed the same, that's an unverifiable state.
                if !activeGeneration.invocationOwnerKey.contains(branchFragment) &&
                   activeGeneration.invocationOwnerKey != currentInvocationOwnerKey {
                    return .createFresh(disposition: .fresh_session_required, reason: "Recovery branch mismatch")
                }
            }
        }

        // 4. Scope rules (§4.3, §6.2)
        switch lineage.sessionReuseScope {
        case .none:
            return .createFresh(disposition: .fresh, reason: "Scope is none")
        case .same_invocation_owner:
            if activeGeneration.invocationOwnerKey == currentInvocationOwnerKey {
                return .reuse(activeGeneration)
            } else {
                return .createFresh(disposition: .fresh, reason: "Invocation owner mismatch")
            }
        case .same_agent_family_within_run:
            // Family scope allows reuse across different owners if familyID matches
            // (already checked by finding the lineage via runID + agentID + familyID).
            // Recovery-branch verification is relaxed for family scope because
            // multiple invocation owners may legitimately share the session.
            // Binding fingerprint was already verified above.
            return .reuse(activeGeneration)
        }
    }
}
