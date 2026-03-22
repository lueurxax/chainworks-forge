import Foundation
import CryptoKit

struct DefinitionHasher: Sendable {

    static let canonicalEncoder: JSONEncoder = {
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.sortedKeys, .withoutEscapingSlashes]
        encoder.dateEncodingStrategy = .iso8601
        encoder.dataEncodingStrategy = .base64
        return encoder
    }()

    nonisolated static func hash<T: Encodable>(_ value: T) throws -> (data: Data, sha256: String) {
        let data = try canonicalEncoder.encode(value)
        let digest = SHA256.hash(data: data)
        let hex = digest.map { String(format: "%02x", $0) }.joined()
        return (data, hex)
    }
}
