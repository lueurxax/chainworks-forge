import Foundation
import Security

enum SecretStoreError: Error, LocalizedError {
    case unexpectedStatus(OSStatus)
    case invalidPayload

    var errorDescription: String? {
        switch self {
        case .unexpectedStatus(let status):
            return "Keychain operation failed with status \(status)"
        case .invalidPayload:
            return "Secret payload could not be decoded"
        }
    }
}

struct KeychainSecretStore: Sendable {
    private let serviceName: String
    private let usesInMemoryStore: Bool
    private static let inMemorySecrets = InMemorySecretBox()

    init(
        serviceName: String = "com.chainworks.forge.secrets",
        useInMemoryStore: Bool? = nil
    ) {
        self.serviceName = serviceName
        self.usesInMemoryStore = useInMemoryStore
            ?? (ProcessInfo.processInfo.environment["CHAINWORKS_IN_MEMORY_STORE"] == "1")
    }

    func setSecret(_ value: String, for key: String) throws {
        if usesInMemoryStore {
            Self.inMemorySecrets.set(value, for: scopedKey(key))
            return
        }

        let encoded = Data(value.utf8)
        let query = baseQuery(for: key)
        SecItemDelete(query as CFDictionary)

        var attributes = query
        attributes[kSecValueData as String] = encoded

        let status = SecItemAdd(attributes as CFDictionary, nil)
        guard status == errSecSuccess else {
            throw SecretStoreError.unexpectedStatus(status)
        }
    }

    func secret(for key: String) throws -> String? {
        if usesInMemoryStore {
            return Self.inMemorySecrets.secret(for: scopedKey(key))
        }

        var query = baseQuery(for: key)
        query[kSecReturnData as String] = true
        query[kSecMatchLimit as String] = kSecMatchLimitOne

        var item: CFTypeRef?
        let status = SecItemCopyMatching(query as CFDictionary, &item)
        if status == errSecItemNotFound {
            return nil
        }
        guard status == errSecSuccess else {
            throw SecretStoreError.unexpectedStatus(status)
        }
        guard let data = item as? Data,
              let string = String(data: data, encoding: .utf8) else {
            throw SecretStoreError.invalidPayload
        }
        return string
    }

    func deleteSecret(for key: String) throws {
        if usesInMemoryStore {
            Self.inMemorySecrets.deleteSecret(for: scopedKey(key))
            return
        }

        let status = SecItemDelete(baseQuery(for: key) as CFDictionary)
        guard status == errSecSuccess || status == errSecItemNotFound else {
            throw SecretStoreError.unexpectedStatus(status)
        }
    }

    func isAccessible() -> Bool {
        if usesInMemoryStore {
            return true
        }

        let query = baseQuery(for: "__probe__")
        let status = SecItemCopyMatching(query as CFDictionary, nil)
        return status == errSecSuccess || status == errSecItemNotFound
    }

    private func baseQuery(for key: String) -> [String: Any] {
        [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: serviceName,
            kSecAttrAccount as String: key
        ]
    }

    private func scopedKey(_ key: String) -> String {
        "\(serviceName)::\(key)"
    }
}

private final class InMemorySecretBox: @unchecked Sendable {
    private let lock = NSLock()
    private var secrets: [String: String] = [:]

    func set(_ value: String, for key: String) {
        lock.lock()
        secrets[key] = value
        lock.unlock()
    }

    func secret(for key: String) -> String? {
        lock.lock()
        defer { lock.unlock() }
        return secrets[key]
    }

    func deleteSecret(for key: String) {
        lock.lock()
        secrets.removeValue(forKey: key)
        lock.unlock()
    }
}
