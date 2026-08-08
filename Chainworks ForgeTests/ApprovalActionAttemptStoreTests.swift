import Foundation
import Testing
@testable import Chainworks_Forge

@Suite("Proposal 081 approval action attempt store", .tags(.fast))
struct Proposal081ApprovalActionAttemptStoreTests {
    @Test
    func approvalActionAttemptStoreUsesStableLowercaseUUIDv4RequestID() {
        let suiteName = "approval-action-attempt-store-tests-\(UUID().uuidString)"
        let defaults = UserDefaults(suiteName: suiteName)!
        defer { defaults.removePersistentDomain(forName: suiteName) }

        let ids = IDSource([
            "11111111-1111-4111-8111-111111111111",
            "22222222-2222-4222-8222-222222222222",
        ])
        let store = P081ApprovalActionAttemptStore(
            defaults: defaults,
            storageKey: "approval-action-attempt-store-tests",
            makeID: { ids.next() }
        )

        let first = store.idempotencyKey(for: "approval-1", action: .approve)
        let replay = store.idempotencyKey(for: "approval-1", action: .approve)

        #expect(first == "11111111-1111-4111-8111-111111111111")
        #expect(replay == first)
        #expect(isLowercaseUUIDv4(first))

        store.clear(approvalID: "approval-1", action: .approve)
        let next = store.idempotencyKey(for: "approval-1", action: .approve)

        #expect(next == "22222222-2222-4222-8222-222222222222")
        #expect(isLowercaseUUIDv4(next))
    }

    @Test
    func approvalActionAttemptStoreRegeneratesPersistedNonUUIDv4RequestID() {
        let suiteName = "approval-action-attempt-store-tests-\(UUID().uuidString)"
        let defaults = UserDefaults(suiteName: suiteName)!
        defer { defaults.removePersistentDomain(forName: suiteName) }

        let storageKey = "approval-action-attempt-store-tests"
        defaults.set(
            [
                "approval:approval-1|action:approve": "019789aa-0000-7000-8000-000000000000",
            ],
            forKey: storageKey
        )

        let store = P081ApprovalActionAttemptStore(
            defaults: defaults,
            storageKey: storageKey,
            makeID: { "33333333-3333-4333-8333-333333333333" }
        )

        let regenerated = store.idempotencyKey(for: "approval-1", action: .approve)

        #expect(regenerated == "33333333-3333-4333-8333-333333333333")
        #expect(isLowercaseUUIDv4(regenerated))
    }

    private func isLowercaseUUIDv4(_ value: String) -> Bool {
        let chars = Array(value)
        guard chars.count == 36 else { return false }
        for index in [8, 13, 18, 23] where chars[index] != "-" {
            return false
        }
        guard chars[14] == "4" else { return false }
        guard ["8", "9", "a", "b"].contains(chars[19]) else { return false }
        return value.unicodeScalars.allSatisfy { scalar in
            switch scalar.value {
            case 45, 48...57, 97...102:
                return true
            default:
                return false
            }
        }
    }

    private final class IDSource: @unchecked Sendable {
        private let lock = NSLock()
        private var ids: [String]

        init(_ ids: [String]) {
            self.ids = ids
        }

        func next() -> String {
            lock.lock()
            defer { lock.unlock() }
            return ids.removeFirst()
        }
    }
}
