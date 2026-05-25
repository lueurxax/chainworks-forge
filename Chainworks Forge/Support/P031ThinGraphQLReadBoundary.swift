import Foundation

enum P031GraphQLReadBoundaryError: Error, Equatable, CustomStringConvertible, LocalizedError {
  case emptyDocument
  case unsupportedOperation(String)
  case queryOperationRequired(String)
  case subscriptionOperationRequired(String)
  case mutationOperationForbidden(String)
  case forbiddenOperationName(String)
  case operationNameNotFound(String)
  case httpFailure(status: Int, body: String)
  case graphqlErrors([String])
  case missingData(String)
  case decodingFailed(String)
  case transportFailed(String)

  var description: String {
    switch self {
    case .emptyDocument:
      return "P031 GraphQL read request is empty"
    case .unsupportedOperation(let keyword):
      return "P031 GraphQL request must be a query or subscription, got \(keyword)"
    case .queryOperationRequired(let operationName):
      return "P031 GraphQL HTTP read client requires a query operation, got \(operationName)"
    case .subscriptionOperationRequired(let operationName):
      return
        "P031 GraphQL subscription client requires a subscription operation, got \(operationName)"
    case .mutationOperationForbidden(let operationName):
      return "P031/P072 UI must not execute forbidden GraphQL mutation operation \(operationName)"
    case .forbiddenOperationName(let operationName):
      return
        "P031 UI read operation name looks like removed write/control plumbing: \(operationName)"
    case .operationNameNotFound(let operationName):
      return "P031 GraphQL document does not contain named operation \(operationName)"
    case .httpFailure(let status, let body):
      return "P031 GraphQL read HTTP \(status): \(body)"
    case .graphqlErrors(let errors):
      return "P031 GraphQL read errors: \(errors.joined(separator: "; "))"
    case .missingData(let operationName):
      return "P031 GraphQL read response for \(operationName) did not include data"
    case .decodingFailed(let message):
      return "P031 GraphQL read response decode failed: \(message)"
    case .transportFailed(let message):
      return "P031 GraphQL read transport failed: \(message)"
    }
  }

  var errorDescription: String? {
    description
  }
}

enum P031GraphQLOperationKind: String, Codable, Equatable, Sendable {
  case query
  case subscription
  case mutation
}

struct P031GraphQLReadRequest: Equatable, Sendable {
  let operationName: String
  let document: String
  let variables: [String: P031GraphQLVariableValue]
  let operationKind: P031GraphQLOperationKind

  nonisolated init(
    operationName: String,
    document: String,
    variables: [String: P031GraphQLVariableValue] = [:]
  ) throws {
    let normalizedName = operationName.trimmingCharacters(in: .whitespacesAndNewlines)
    let normalizedDocument = document.trimmingCharacters(in: .whitespacesAndNewlines)
    guard !normalizedName.isEmpty, !normalizedDocument.isEmpty else {
      throw P031GraphQLReadBoundaryError.emptyDocument
    }
    if Self.isForbiddenOperationName(normalizedName) {
      throw P031GraphQLReadBoundaryError.forbiddenOperationName(normalizedName)
    }

    let operations = try Self.operations(in: normalizedDocument)
    if let forbiddenDocumentOperationName = operations.compactMap(\.name).first(where: {
      Self.isForbiddenOperationName($0)
    }) {
      throw P031GraphQLReadBoundaryError.forbiddenOperationName(forbiddenDocumentOperationName)
    }
    guard let requestedOperation = operations.first(where: { $0.name == normalizedName }) else {
      throw P031GraphQLReadBoundaryError.operationNameNotFound(normalizedName)
    }
    switch requestedOperation.keyword {
    case "query":
      operationKind = .query
    case "subscription":
      operationKind = .subscription
    case "mutation":
      // Reject documents with more than one mutation to prevent multi-operation allowlist bypass.
      guard operations.filter({ $0.keyword == "mutation" }).count == 1 else {
        throw P031GraphQLReadBoundaryError.mutationOperationForbidden(normalizedName)
      }
      guard Self.isAllowedApprovalMutationDocument(normalizedDocument) else {
        throw P031GraphQLReadBoundaryError.mutationOperationForbidden(normalizedName)
      }
      operationKind = .mutation
    default:
      throw P031GraphQLReadBoundaryError.unsupportedOperation(requestedOperation.keyword)
    }

    self.operationName = normalizedName
    self.document = normalizedDocument
    self.variables = variables
  }

  nonisolated private static func operations(in document: String) throws
    -> [(keyword: String, name: String?)]
  {
    let scanDocument = document.maskingGraphQLIgnoredTextForP031OperationScan()
    var operations: [(keyword: String, name: String?)] = []
    var index = scanDocument.startIndex
    var braceDepth = 0
    var acceptingDefinitionKeyword = true

    while index < scanDocument.endIndex {
      let character = scanDocument[index]

      if character == "{" {
        braceDepth += 1
        acceptingDefinitionKeyword = false
        index = scanDocument.index(after: index)
        continue
      }

      if character == "}" {
        braceDepth = max(0, braceDepth - 1)
        acceptingDefinitionKeyword = braceDepth == 0
        index = scanDocument.index(after: index)
        continue
      }

      guard Self.isGraphQLNameStart(character) else {
        index = scanDocument.index(after: index)
        continue
      }

      let tokenStart = index
      index = scanDocument.index(after: index)
      while index < scanDocument.endIndex, Self.isGraphQLNameContinue(scanDocument[index]) {
        index = scanDocument.index(after: index)
      }

      guard braceDepth == 0, acceptingDefinitionKeyword else {
        continue
      }

      let token = String(scanDocument[tokenStart..<index]).lowercased()
      switch token {
      case "query", "subscription", "mutation":
        let operationName = Self.operationName(after: index, in: scanDocument)
        operations.append((token, operationName))
        acceptingDefinitionKeyword = false
      case "fragment":
        acceptingDefinitionKeyword = false
      default:
        continue
      }
    }

    guard !operations.isEmpty else {
      throw P031GraphQLReadBoundaryError.unsupportedOperation("missing operation keyword")
    }
    return operations
  }

  nonisolated private static func operationName(
    after tokenEnd: String.Index,
    in document: String
  ) -> String? {
    var index = tokenEnd
    while index < document.endIndex, document[index].isWhitespace {
      index = document.index(after: index)
    }
    guard index < document.endIndex, Self.isGraphQLNameStart(document[index]) else {
      return nil
    }
    let nameStart = index
    index = document.index(after: index)
    while index < document.endIndex, Self.isGraphQLNameContinue(document[index]) {
      index = document.index(after: index)
    }
    return String(document[nameStart..<index])
  }

  nonisolated fileprivate static func isGraphQLNameStart(_ character: Character) -> Bool {
    guard let scalar = character.unicodeScalars.first, character.unicodeScalars.count == 1 else {
      return false
    }
    return scalar.value == 95
      || (65...90).contains(scalar.value)
      || (97...122).contains(scalar.value)
  }

  nonisolated fileprivate static func isGraphQLNameContinue(_ character: Character) -> Bool {
    guard let scalar = character.unicodeScalars.first, character.unicodeScalars.count == 1 else {
      return false
    }
    return isGraphQLNameStart(character) || (48...57).contains(scalar.value)
  }

  nonisolated private static func isForbiddenOperationName(_ operationName: String) -> Bool {
    let lowercase = operationName.lowercased()
    return [
      "mutation",
      "createidea",
      "startrun",
      "cancelrun",
      "retrystage",
      "resolveapproval",
      "runsteward",
      "resetsession",
      "resumerun",
      "clonerun",
      "comparerun",
      "launchexperiment",
      "runtimehealth",
      "agentreset",
      "resetagent",
      "commandreceipt",
      "clientcommandid",
    ].contains { lowercase.contains($0) }
  }

  nonisolated private static func isAllowedApprovalMutationDocument(_ document: String) -> Bool {
    let scanDocument = document.maskingGraphQLIgnoredTextForP031OperationScan()
    let allowedFields: Set<String> = ["approveApproval", "rejectApproval"]
    guard let rootFields = mutationRootSelectionFields(in: scanDocument),
          !rootFields.isEmpty
    else { return false }
    return rootFields.count == 1 && rootFields.allSatisfy { allowedFields.contains($0) }
  }

  // Extracts the root-level selection field names from the single mutation operation body.
  // Returns nil if the body cannot be parsed, contains aliases, or contains fragment spreads.
  // Caller must ensure the document has exactly one mutation before invoking this function.
  nonisolated private static func mutationRootSelectionFields(in scanDocument: String) -> [String]? {
    var index = scanDocument.startIndex
    var globalDepth = 0

    while index < scanDocument.endIndex {
      let c = scanDocument[index]

      if c == "{" {
        globalDepth += 1
        index = scanDocument.index(after: index)
        continue
      }
      if c == "}" {
        globalDepth = max(0, globalDepth - 1)
        index = scanDocument.index(after: index)
        continue
      }

      guard isGraphQLNameStart(c) else {
        index = scanDocument.index(after: index)
        continue
      }

      let tokenStart = index
      index = scanDocument.index(after: index)
      while index < scanDocument.endIndex, isGraphQLNameContinue(scanDocument[index]) {
        index = scanDocument.index(after: index)
      }

      guard globalDepth == 0 else { continue }

      let token = String(scanDocument[tokenStart..<index]).lowercased()
      guard token == "mutation" else { continue }

      // Skip optional variable definitions (inside parens) and directives to reach the body '{'.
      var parenDepth = 0
      var foundBody = false
      while index < scanDocument.endIndex {
        let bc = scanDocument[index]
        if bc == "(" { parenDepth += 1 }
        else if bc == ")" { parenDepth = max(0, parenDepth - 1) }
        else if bc == "{", parenDepth == 0 {
          index = scanDocument.index(after: index)
          foundBody = true
          globalDepth = 1
          break
        }
        index = scanDocument.index(after: index)
      }
      guard foundBody else { continue }

      // Extract root-level fields at globalDepth == 1 (inside mutation body).
      var fields: [String] = []
      var bodyParenDepth = 0

      while index < scanDocument.endIndex, globalDepth > 0 {
        let bc = scanDocument[index]
        if bc == "(" {
          bodyParenDepth += 1
          index = scanDocument.index(after: index)
          continue
        }
        if bc == ")" {
          bodyParenDepth = max(0, bodyParenDepth - 1)
          index = scanDocument.index(after: index)
          continue
        }
        if bc == "{" {
          if bodyParenDepth == 0 { globalDepth += 1 }
          index = scanDocument.index(after: index)
          continue
        }
        if bc == "}" {
          if bodyParenDepth == 0 {
            globalDepth -= 1
            if globalDepth == 0 { break }
          }
          index = scanDocument.index(after: index)
          continue
        }
        // Fragment spreads ("...") at root level are rejected.
        if globalDepth == 1, bodyParenDepth == 0, bc == "." {
          return nil
        }
        guard globalDepth == 1, bodyParenDepth == 0, isGraphQLNameStart(bc) else {
          index = scanDocument.index(after: index)
          continue
        }
        let fieldStart = index
        index = scanDocument.index(after: index)
        while index < scanDocument.endIndex, isGraphQLNameContinue(scanDocument[index]) {
          index = scanDocument.index(after: index)
        }
        let fieldToken = String(scanDocument[fieldStart..<index])
        // Reject aliases: "aliasName: fieldName".
        var peekIdx = index
        while peekIdx < scanDocument.endIndex, scanDocument[peekIdx].isWhitespace {
          peekIdx = scanDocument.index(after: peekIdx)
        }
        if peekIdx < scanDocument.endIndex, scanDocument[peekIdx] == ":" {
          return nil
        }
        // "on" is a keyword in inline fragments, not a selection field.
        if fieldToken != "on" {
          fields.append(fieldToken)
        }
      }
      return fields.isEmpty ? nil : fields
    }
    return nil
  }
}

extension String {
  nonisolated fileprivate func containsGraphQLFieldNamed(_ fieldName: String) -> Bool {
    guard !fieldName.isEmpty else { return false }
    var index = startIndex
    while index < endIndex {
      guard let range = self[index...].range(of: fieldName) else { return false }
      let before = range.lowerBound > startIndex ? self[self.index(before: range.lowerBound)] : " "
      let after = range.upperBound < endIndex ? self[range.upperBound] : " "
      let beforeOK = !P031GraphQLReadRequest.isGraphQLNameContinue(before)
      let afterOK = after == "(" || after == "{" || after.isWhitespace
      if beforeOK && afterOK {
        return true
      }
      index = range.upperBound
    }
    return false
  }

  nonisolated fileprivate func maskingGraphQLIgnoredTextForP031OperationScan() -> String {
    var masked = ""
    masked.reserveCapacity(count)
    var index = startIndex

    func appendMask(for character: Character) {
      masked.append(character == "\n" ? "\n" : " ")
    }

    func isTripleQuote(at position: String.Index) -> Bool {
      guard position < endIndex, self[position] == "\"" else {
        return false
      }
      let second = self.index(after: position)
      guard second < endIndex, self[second] == "\"" else {
        return false
      }
      let third = self.index(after: second)
      return third < endIndex && self[third] == "\""
    }

    while index < endIndex {
      let character = self[index]

      if character == "#" {
        while index < endIndex, self[index] != "\n" {
          appendMask(for: self[index])
          index = self.index(after: index)
        }
        continue
      }

      if character == "\"" {
        if isTripleQuote(at: index) {
          for _ in 0..<3 {
            appendMask(for: self[index])
            index = self.index(after: index)
          }
          while index < endIndex {
            if isTripleQuote(at: index) {
              for _ in 0..<3 {
                appendMask(for: self[index])
                index = self.index(after: index)
              }
              break
            }
            appendMask(for: self[index])
            index = self.index(after: index)
          }
          continue
        }

        appendMask(for: character)
        index = self.index(after: index)
        while index < endIndex {
          let stringCharacter = self[index]
          appendMask(for: stringCharacter)
          index = self.index(after: index)
          if stringCharacter == "\\" {
            guard index < endIndex else {
              break
            }
            appendMask(for: self[index])
            index = self.index(after: index)
          } else if stringCharacter == "\"" {
            break
          }
        }
        continue
      }

      masked.append(character)
      index = self.index(after: index)
    }

    return masked
  }
}

indirect enum P031GraphQLVariableValue: Equatable, Sendable {
  case string(String)
  case int(Int)
  case double(Double)
  case bool(Bool)
  case null
  case array([P031GraphQLVariableValue])
  case object([String: P031GraphQLVariableValue])

  var jsonValue: Any {
    switch self {
    case .string(let value):
      return value
    case .int(let value):
      return value
    case .double(let value):
      return value
    case .bool(let value):
      return value
    case .null:
      return NSNull()
    case .array(let values):
      return values.map(\.jsonValue)
    case .object(let values):
      return values.mapValues(\.jsonValue)
    }
  }
}

extension P031GraphQLVariableValue: ExpressibleByStringLiteral {
  init(stringLiteral value: String) {
    self = .string(value)
  }
}

extension P031GraphQLVariableValue: ExpressibleByIntegerLiteral {
  init(integerLiteral value: Int) {
    self = .int(value)
  }
}

extension P031GraphQLVariableValue: ExpressibleByFloatLiteral {
  init(floatLiteral value: Double) {
    self = .double(value)
  }
}

extension P031GraphQLVariableValue: ExpressibleByBooleanLiteral {
  init(booleanLiteral value: Bool) {
    self = .bool(value)
  }
}

extension P031GraphQLVariableValue: ExpressibleByNilLiteral {
  init(nilLiteral: ()) {
    self = .null
  }
}

extension P031GraphQLVariableValue: ExpressibleByArrayLiteral {
  init(arrayLiteral elements: P031GraphQLVariableValue...) {
    self = .array(elements)
  }
}

extension P031GraphQLVariableValue: ExpressibleByDictionaryLiteral {
  init(dictionaryLiteral elements: (String, P031GraphQLVariableValue)...) {
    self = .object(Dictionary(uniqueKeysWithValues: elements))
  }
}

extension Dictionary where Key == String, Value == P031GraphQLVariableValue {
  var p031JSONObject: [String: Any] {
    mapValues(\.jsonValue)
  }
}

protocol P031GraphQLReadTransport: Sendable {
  func send(_ request: P031GraphQLReadRequest) async throws -> Data
}

protocol P031GraphQLSubscriptionTransport: Sendable {
  nonisolated func subscribe(_ request: P031GraphQLReadRequest) -> AsyncThrowingStream<Data, Error>
}

enum P031GraphQLWebSocketFrameAction: Equatable, Sendable {
  case acknowledge
  case next(Data)
  case complete
  case ignore
}

struct P031GraphQLReadClient<Transport: P031GraphQLReadTransport>: Sendable {
  let transport: Transport

  func execute(
    operationName: String,
    document: String,
    variables: [String: P031GraphQLVariableValue] = [:]
  ) async throws -> Data {
    let request = try P031GraphQLReadRequest(
      operationName: operationName,
      document: document,
      variables: variables
    )
    guard request.operationKind == .query else {
      throw P031GraphQLReadBoundaryError.queryOperationRequired(operationName)
    }
    return try await transport.send(request)
  }

  func execute<Payload: Decodable>(
    _ payloadType: Payload.Type,
    operationName: String,
    document: String,
    variables: [String: P031GraphQLVariableValue] = [:]
  ) async throws -> Payload {
    let data = try await execute(
      operationName: operationName,
      document: document,
      variables: variables
    )
    return try P031GraphQLResponseDecoder.decode(
      payloadType,
      from: data,
      operationName: operationName
    )
  }
}

enum P072ApprovalDecisionAction: Equatable, Sendable {
  case approve
  case reject(reason: String)
}

struct P072ApprovalMutationResult: Decodable, Equatable, Sendable {
  let approval: P031ApprovalReadModel
  let journalID: String
  /// Typed conflict/idempotency result code when the server supports it; nil for success.
  let conflictResultCode: P085MutationConflictResultCode?

  enum CodingKeys: String, CodingKey {
    case approval
    case journalID = "journalId"
    case conflictResultCodeRaw = "conflictResultCode"
  }

  init(from decoder: Decoder) throws {
    let container = try decoder.container(keyedBy: CodingKeys.self)
    self.approval = try container.decode(P031ApprovalReadModel.self, forKey: .approval)
    self.journalID = try container.decode(String.self, forKey: .journalID)
    if let rawCode = try container.decodeIfPresent(String.self, forKey: .conflictResultCodeRaw) {
      self.conflictResultCode = P085MutationConflictResultCode.fromRaw(rawCode)
    } else {
      self.conflictResultCode = nil
    }
  }
}

struct P072ApprovalMutationClient<Transport: P031GraphQLReadTransport>: Sendable {
  let transport: Transport

  func approve(approvalID: String, idempotencyKey: String) async throws -> P072ApprovalMutationResult {
    try await execute(
      P072ApproveApprovalPayload.self,
      operationName: "P072ApproveApproval",
      document: P031GraphQLDocuments.approveApproval,
      variables: [
        "approvalId": .string(approvalID),
        "idempotencyKey": .string(idempotencyKey),
      ]
    ).approveApproval
  }

  func reject(approvalID: String, reason: String, idempotencyKey: String) async throws -> P072ApprovalMutationResult {
    try await execute(
      P072RejectApprovalPayload.self,
      operationName: "P072RejectApproval",
      document: P031GraphQLDocuments.rejectApproval,
      variables: [
        "approvalId": .string(approvalID),
        "reason": .string(reason),
        "idempotencyKey": .string(idempotencyKey),
      ]
    ).rejectApproval
  }

  private func execute<Payload: Decodable>(
    _ payloadType: Payload.Type,
    operationName: String,
    document: String,
    variables: [String: P031GraphQLVariableValue]
  ) async throws -> Payload {
    let request = try P031GraphQLReadRequest(
      operationName: operationName,
      document: document,
      variables: variables
    )
    guard request.operationKind == .mutation else {
      throw P031GraphQLReadBoundaryError.mutationOperationForbidden(operationName)
    }
    let data = try await transport.send(request)
    return try P031GraphQLResponseDecoder.decode(
      payloadType,
      from: data,
      operationName: operationName
    )
  }

  private struct P072ApproveApprovalPayload: Decodable {
    let approveApproval: P072ApprovalMutationResult
  }

  private struct P072RejectApprovalPayload: Decodable {
    let rejectApproval: P072ApprovalMutationResult
  }
}

struct P031GraphQLSubscriptionClient<Transport: P031GraphQLSubscriptionTransport>: Sendable {
  let transport: Transport

  nonisolated func subscribe(
    operationName: String,
    document: String,
    variables: [String: P031GraphQLVariableValue] = [:]
  ) throws -> AsyncThrowingStream<Data, Error> {
    let request = try P031GraphQLReadRequest(
      operationName: operationName,
      document: document,
      variables: variables
    )
    guard request.operationKind == .subscription else {
      throw P031GraphQLReadBoundaryError.subscriptionOperationRequired(operationName)
    }
    return transport.subscribe(request)
  }

  nonisolated func subscribe<Payload: Decodable>(
    _ payloadType: Payload.Type,
    operationName: String,
    document: String,
    variables: [String: P031GraphQLVariableValue] = [:]
  ) throws -> AsyncThrowingStream<Payload, Error> {
    let stream = try subscribe(
      operationName: operationName,
      document: document,
      variables: variables
    )
    return AsyncThrowingStream { continuation in
      let task = Task {
        do {
          for try await data in stream {
            let payload = try P031GraphQLResponseDecoder.decode(
              payloadType,
              from: data,
              operationName: operationName
            )
            continuation.yield(payload)
          }
          continuation.finish()
        } catch {
          continuation.finish(throwing: error)
        }
      }
      continuation.onTermination = { _ in task.cancel() }
    }
  }
}

struct P031URLSessionGraphQLReadTransport: P031GraphQLReadTransport, @unchecked Sendable {
  let endpoint: DaemonClientEndpoint
  let urlSession: URLSession

  init(endpoint: DaemonClientEndpoint, urlSession: URLSession = .shared) {
    self.endpoint = endpoint
    self.urlSession = urlSession
  }

  func send(_ request: P031GraphQLReadRequest) async throws -> Data {
    let body: [String: Any] = [
      "operationName": request.operationName,
      "query": request.document,
      "variables": request.variables.p031JSONObject,
    ]
    let encodedBody: Data
    do {
      encodedBody = try JSONSerialization.data(withJSONObject: body)
    } catch {
      throw P031GraphQLReadBoundaryError.decodingFailed(error.localizedDescription)
    }

    var urlRequest = URLRequest(url: endpoint.graphqlURL)
    urlRequest.httpMethod = "POST"
    urlRequest.httpBody = encodedBody
    urlRequest.setValue("application/json", forHTTPHeaderField: "Content-Type")
    urlRequest.setValue("Bearer \(endpoint.bearerToken)", forHTTPHeaderField: "Authorization")

    let response: URLResponse
    let data: Data
    do {
      (data, response) = try await urlSession.data(for: urlRequest)
    } catch {
      throw P031GraphQLReadBoundaryError.transportFailed(error.localizedDescription)
    }
    guard let http = response as? HTTPURLResponse else {
      throw P031GraphQLReadBoundaryError.httpFailure(status: -1, body: "no HTTP response")
    }
    guard (200..<300).contains(http.statusCode) else {
      let body = String(data: data, encoding: .utf8) ?? "<binary>"
      throw P031GraphQLReadBoundaryError.httpFailure(status: http.statusCode, body: body)
    }
    return data
  }
}

struct P031URLSessionGraphQLSubscriptionTransport: P031GraphQLSubscriptionTransport,
  @unchecked Sendable
{
  let endpoint: DaemonClientEndpoint
  let urlSession: URLSession

  init(endpoint: DaemonClientEndpoint, urlSession: URLSession = .shared) {
    self.endpoint = endpoint
    self.urlSession = urlSession
  }

  nonisolated func subscribe(_ request: P031GraphQLReadRequest) -> AsyncThrowingStream<Data, Error> {
    let socket = urlSession.webSocketTask(with: Self.subscribeRequest(for: endpoint))
    return AsyncThrowingStream { continuation in
      let task = Task {
        await Self.drive(
          socket: socket,
          bearerToken: endpoint.bearerToken,
          request: request,
          continuation: continuation
        )
      }
      continuation.onTermination = { _ in
        task.cancel()
        socket.cancel(with: .goingAway, reason: nil)
      }
    }
  }

  nonisolated static func subscribeRequest(for endpoint: DaemonClientEndpoint) -> URLRequest {
    var request = URLRequest(url: endpoint.graphqlWSURL)
    request.setValue("graphql-transport-ws", forHTTPHeaderField: "Sec-WebSocket-Protocol")
    return request
  }

  static func connectionInitFrame(bearerToken: String) throws -> String {
    try encodeJSON([
      "type": "connection_init",
      "payload": [
        "Authorization": "Bearer \(bearerToken)"
      ],
    ])
  }

  static func subscribeFrame(for request: P031GraphQLReadRequest) throws -> String {
    try encodeJSON([
      "id": request.operationName,
      "type": "subscribe",
      "payload": [
        "operationName": request.operationName,
        "query": request.document,
        "variables": request.variables.p031JSONObject,
      ],
    ])
  }

  static func decodeFrame(_ text: String) throws -> P031GraphQLWebSocketFrameAction {
    guard let data = text.data(using: .utf8),
      let object = try JSONSerialization.jsonObject(with: data) as? [String: Any],
      let type = object["type"] as? String
    else {
      return .ignore
    }

    switch type {
    case "connection_ack":
      return .acknowledge
    case "next":
      guard let payload = object["payload"] as? [String: Any] else {
        return .ignore
      }
      return .next(try JSONSerialization.data(withJSONObject: payload))
    case "complete":
      return .complete
    case "error":
      throw P031GraphQLReadBoundaryError.graphqlErrors(graphQLErrorMessages(from: object))
    case "ping":
      return .ignore
    default:
      return .ignore
    }
  }

  private static func drive(
    socket: URLSessionWebSocketTask,
    bearerToken: String,
    request: P031GraphQLReadRequest,
    continuation: AsyncThrowingStream<Data, Error>.Continuation
  ) async {
    socket.resume()
    do {
      try await socket.send(.string(try connectionInitFrame(bearerToken: bearerToken)))
    } catch {
      continuation.finish(
        throwing: P031GraphQLReadBoundaryError.transportFailed(error.localizedDescription))
      return
    }

    while !Task.isCancelled {
      let message: URLSessionWebSocketTask.Message
      do {
        message = try await socket.receive()
      } catch {
        continuation.finish(
          throwing: P031GraphQLReadBoundaryError.transportFailed(error.localizedDescription))
        return
      }

      guard let text = message.stringValue else {
        continue
      }
      let frame: P031GraphQLWebSocketFrameAction
      do {
        frame = try decodeFrame(text)
      } catch {
        continuation.finish(throwing: error)
        return
      }

      switch frame {
      case .acknowledge:
        do {
          try await socket.send(.string(try subscribeFrame(for: request)))
        } catch {
          continuation.finish(
            throwing: P031GraphQLReadBoundaryError.transportFailed(error.localizedDescription)
          )
          return
        }
      case .next(let payload):
        continuation.yield(payload)
      case .complete:
        continuation.finish()
        return
      case .ignore:
        _ = try? await socket.send(.string(try encodeJSON(["type": "pong"])))
      }
    }
  }

  private static func graphQLErrorMessages(from object: [String: Any]) -> [String] {
    let messages =
      (object["payload"] as? [[String: Any]])?
      .compactMap { $0["message"] as? String } ?? []
    return messages.isEmpty ? ["subscription error"] : messages
  }

  private static func encodeJSON(_ object: [String: Any]) throws -> String {
    let data = try JSONSerialization.data(withJSONObject: object)
    return String(data: data, encoding: .utf8) ?? "{}"
  }
}

extension URLSessionWebSocketTask.Message {
  fileprivate var stringValue: String? {
    switch self {
    case .string(let text):
      return text
    case .data(let data):
      return String(data: data, encoding: .utf8)
    @unknown default:
      return nil
    }
  }
}

nonisolated private struct P031GraphQLResponseEnvelope<Payload: Decodable>: Decodable {
  let data: Payload?
  let errors: [P031GraphQLResponseError]?
  let extensions: P081GraphQLResponseExtensions?
}

nonisolated private struct P031GraphQLResponseError: Decodable {
  let message: String
}

struct P081GraphQLRedaction: Decodable, Equatable, Sendable {
  let path: [String]
  let reasonCode: String
  let rowId: String?
  let redactionMode: String
  let callerClass: String?
  let redactionId: String
}

struct P081GraphQLResponseExtensions: Decodable, Equatable, Sendable {
  let redactions: [P081GraphQLRedaction]

  init(redactions: [P081GraphQLRedaction] = []) {
    self.redactions = redactions
  }

  init(from decoder: Decoder) throws {
    let container = try decoder.container(keyedBy: CodingKeys.self)
    self.redactions = try container.decodeIfPresent([P081GraphQLRedaction].self, forKey: .redactions) ?? []
  }

  private enum CodingKeys: String, CodingKey {
    case redactions
  }
}

struct P081OperatorAlert: Decodable, Equatable, Sendable {
  let id: String
  let dedupeKey: String
  let severity: String
  let title: String
  let message: String
  let active: Bool
  let silenceable: Bool
  let acknowledgedAtMs: Int?
  let silencedUntilMs: Int?
  let nativeDelivery: P081OperatorAlertNativeDelivery?
  let lifecycle: P081OperatorAlertLifecycle?

  var accessibilityLabel: String {
    "\(title), \(severity)"
  }

  var accessibilityValue: String {
    if let lifecycle {
      return lifecycle.state
    }
    return active ? "active" : "cleared"
  }

  var accessibilityHint: String {
    "Boundary alert. Copy diagnostics for \(dedupeKey)."
  }
}

struct P081OperatorAlertNativeDelivery: Decodable, Equatable, Sendable {
  let deliveryKey: String
  let dockBadgeContribution: Int
  let requestUserAttention: String
  let notificationCategory: String
  let dedupePolicy: String
}

struct P081OperatorAlertLifecycle: Decodable, Equatable, Sendable {
  let state: String
  let dedupeKey: String
  let ackRequired: Bool
  let clearCondition: String
}

enum P081RedactionState: Equatable, Sendable {
  case ordinaryNil(fieldDisplayName: String)
  case redacted(fieldDisplayName: String, redaction: P081GraphQLRedaction)
  case dropResource(fieldDisplayName: String, denialCopy: String, redaction: P081GraphQLRedaction)

  var accessibilityLabel: String {
    switch self {
    case .ordinaryNil(let fieldDisplayName),
      .redacted(let fieldDisplayName, _):
      return fieldDisplayName
    case .dropResource:
      return "Restricted view"
    }
  }

  var accessibilityValue: String {
    switch self {
    case .ordinaryNil:
      return "No value"
    case .redacted:
      return "Restricted value"
    case .dropResource(_, let denialCopy, _):
      return denialCopy
    }
  }

  var accessibilityHint: String? {
    switch self {
    case .ordinaryNil:
      return nil
    case .redacted:
      return "Permissions hide this value. Copy diagnostics for the access rule."
    case .dropResource:
      return "Permissions hide this resource. Copy diagnostics for the access rule."
    }
  }
}

struct P081AccessibilityModePolicy: Equatable, Sendable {
  let fullKeyboardAccessEnabled: Bool
  let increaseContrastEnabled: Bool
  let reduceMotionEnabled: Bool

  func presentation(for state: P081RedactionState) -> P081RedactionPresentation {
    let restricted: Bool
    switch state {
    case .ordinaryNil:
      restricted = false
    case .redacted, .dropResource:
      restricted = true
    }
    return P081RedactionPresentation(
      accessibilityLabel: state.accessibilityLabel,
      accessibilityValue: state.accessibilityValue,
      accessibilityHint: state.accessibilityHint,
      isKeyboardFocusable: fullKeyboardAccessEnabled,
      visualTreatment: restricted && increaseContrastEnabled ? .highContrastRestricted : .ordinary
    )
  }

  func disabledApprovalPresentation(reason: String) -> P081DisabledApprovalPresentation {
    P081DisabledApprovalPresentation(
      isActionEnabled: false,
      isKeyboardFocusable: fullKeyboardAccessEnabled,
      accessibilityHint: "Approval action disabled. \(reason)"
    )
  }

  func alertPresentation(for severity: String) -> P081AlertPresentation {
    if reduceMotionEnabled {
      return P081AlertPresentation(allowsMotion: false, attentionStyle: .staticCritical)
    }
    return P081AlertPresentation(
      allowsMotion: true,
      attentionStyle: severity.lowercased() == "critical" ? .animatedCritical : .staticInformational
    )
  }
}

struct P081RedactionPresentation: Equatable, Sendable {
  let accessibilityLabel: String
  let accessibilityValue: String
  let accessibilityHint: String?
  let isKeyboardFocusable: Bool
  let visualTreatment: P081RedactionVisualTreatment
}

enum P081RedactionVisualTreatment: Equatable, Sendable {
  case ordinary
  case highContrastRestricted
}

struct P081DisabledApprovalPresentation: Equatable, Sendable {
  let isActionEnabled: Bool
  let isKeyboardFocusable: Bool
  let accessibilityHint: String
}

struct P081AlertPresentation: Equatable, Sendable {
  let allowsMotion: Bool
  let attentionStyle: P081AlertAttentionStyle
}

enum P081AlertAttentionStyle: Equatable, Sendable {
  case animatedCritical
  case staticCritical
  case staticInformational
}

enum P031GraphQLResponseDecoder {
  nonisolated static func decodeExtensions(from data: Data) throws -> P081GraphQLResponseExtensions {
    let envelope: P031GraphQLResponseEnvelope<P031EmptyGraphQLPayload>
    do {
      envelope = try JSONDecoder().decode(P031GraphQLResponseEnvelope<P031EmptyGraphQLPayload>.self, from: data)
    } catch {
      throw P031GraphQLReadBoundaryError.decodingFailed(error.localizedDescription)
    }
    return envelope.extensions ?? P081GraphQLResponseExtensions()
  }

  nonisolated static func decode<Payload: Decodable>(
    _ payloadType: Payload.Type,
    from data: Data,
    operationName: String
  ) throws -> Payload {
    let envelope: P031GraphQLResponseEnvelope<Payload>
    do {
      envelope = try JSONDecoder().decode(P031GraphQLResponseEnvelope<Payload>.self, from: data)
    } catch {
      throw P031GraphQLReadBoundaryError.decodingFailed(error.localizedDescription)
    }
    if let errors = envelope.errors, !errors.isEmpty {
      throw P031GraphQLReadBoundaryError.graphqlErrors(errors.map(\.message))
    }
    guard let payload = envelope.data else {
      throw P031GraphQLReadBoundaryError.missingData(operationName)
    }
    return payload
  }
}

nonisolated private struct P031EmptyGraphQLPayload: Decodable {}

enum P031ReadErrorPresenter {
  nonisolated static let schemaMismatchTitle = "Daemon schema mismatch"

  nonisolated static func description(for error: Error) -> String {
    if isSchemaMismatch(error) {
      return "\(schemaMismatchTitle): restart daemon to load the bundled GraphQL schema. \(rawDescription(for: error))"
    }
    return error.localizedDescription
  }

  nonisolated static func isSchemaMismatchDescription(_ description: String?) -> Bool {
    guard let description else { return false }
    return description.contains(schemaMismatchTitle)
      || isSchemaMismatchMessage(description)
  }

  nonisolated private static func isSchemaMismatch(_ error: Error) -> Bool {
    guard case .graphqlErrors(let messages) = error as? P031GraphQLReadBoundaryError else {
      return false
    }
    return messages.contains(where: isSchemaMismatchMessage)
  }

  nonisolated private static func isSchemaMismatchMessage(_ message: String) -> Bool {
    let lowercased = message.lowercased()
    return (lowercased.contains("unknown field") || lowercased.contains("cannot query field"))
      && (lowercased.contains("gql") || lowercased.contains("type"))
  }

  nonisolated private static func rawDescription(for error: Error) -> String {
    (error as? LocalizedError)?.errorDescription ?? error.localizedDescription
  }
}

enum P031FreshnessState: String, Codable, CaseIterable, Equatable, Sendable {
  case live
  case refreshing
  case projectionLag = "projection_lag"
  case stale
  case unavailable
  case unauthorized

  // Fail-closed: unknown server values decode to .unavailable, never to optimistic states.
  init(from decoder: Decoder) throws {
    let raw = try decoder.singleValueContainer().decode(String.self)
    self = Self(rawValue: raw) ?? .unavailable
  }
}

enum P031DisabledReasonCode: String, Codable, CaseIterable, Equatable, Sendable {
  case writePathNotAvailable = "WRITE_PATH_NOT_AVAILABLE"
  case managedOutsideUI = "MANAGED_OUTSIDE_UI"
  case ambiguousApprovalIdentity = "AMBIGUOUS_APPROVAL_IDENTITY"
  case staleRead = "STALE_READ"
  case projectionLag = "PROJECTION_LAG"
  case unauthorized = "UNAUTHORIZED"
  case unsupportedAction = "UNSUPPORTED_ACTION"
  case redacted = "REDACTED"
  case conflict = "CONFLICT"
  case duplicate = "DUPLICATE"
  case alreadyResolved = "ALREADY_RESOLVED"
  case approvalNotActionable = "APPROVAL_NOT_ACTIONABLE"
  case observerScope = "OBSERVER_SCOPE"
  case nonApprovalMutation = "NON_APPROVAL_MUTATION"
  case capabilityOutOfScope = "CAPABILITY_OUT_OF_SCOPE"

  // Fail-closed: unknown server values decode to .writePathNotAvailable (most restrictive).
  init(from decoder: Decoder) throws {
    let raw = try decoder.singleValueContainer().decode(String.self)
    self = Self(rawValue: raw) ?? .writePathNotAvailable
  }
}

enum P031WritePathState: String, Codable, CaseIterable, Equatable, Sendable {
  case available
  case readOnlyDiagnostic = "read_only_diagnostic"
  case writePathNotAvailable = "write_path_not_available"
  case externalTransportRequired = "external_transport_required"
  case hidden

  // Fail-closed: unknown server values decode to .writePathNotAvailable (disables mutations).
  init(from decoder: Decoder) throws {
    let raw = try decoder.singleValueContainer().decode(String.self)
    self = Self(rawValue: raw) ?? .writePathNotAvailable
  }
}

enum P031PayloadAvailabilityState: String, Codable, CaseIterable, Equatable, Sendable {
  case available
  case metadataOnly = "metadata_only"
  case payloadDeferred = "payload_deferred"
  case generating
  case unavailable

  // Fail-closed: unknown server values decode to .unavailable, not to actionable states.
  init(from decoder: Decoder) throws {
    let raw = try decoder.singleValueContainer().decode(String.self)
    self = Self(rawValue: raw) ?? .unavailable
  }
}

enum P031PayloadUnavailableReasonCode: String, Codable, CaseIterable, Equatable, Sendable {
  case payloadDeferredByP031 = "PAYLOAD_DEFERRED_BY_P031"
  case generating = "GENERATING"
  case notIndexed = "NOT_INDEXED"
  case notAuthorized = "NOT_AUTHORIZED"
  case notAvailable = "NOT_AVAILABLE"
  case unknown = "UNKNOWN"

  // Fail-closed: unknown server values decode to .unknown (already defined sentinel).
  init(from decoder: Decoder) throws {
    let raw = try decoder.singleValueContainer().decode(String.self)
    self = Self(rawValue: raw) ?? .unknown
  }
}

struct P031FreshnessSnapshot: Equatable, Sendable {
  let state: P031FreshnessState
  let lastCheckedAt: Date?
  let reason: String?

  nonisolated init(state: P031FreshnessState, lastCheckedAt: Date? = nil, reason: String? = nil) {
    self.state = state
    self.lastCheckedAt = lastCheckedAt
    self.reason = reason
  }
}

enum P031FreshnessEvent: Equatable, Sendable {
  case refreshStarted(at: Date)
  case serverStateReceived(P031FreshnessState, checkedAt: Date, reason: String?)
  case refreshCompletedWithoutNewProjection(checkedAt: Date, reason: String?)
  case refreshFailed(checkedAt: Date, reason: String)
}

enum WorkflowFreshnessReducer {
  nonisolated static func reduce(
    _ snapshot: P031FreshnessSnapshot,
    event: P031FreshnessEvent
  ) -> P031FreshnessSnapshot {
    switch event {
    case .refreshStarted(let at):
      return P031FreshnessSnapshot(state: .refreshing, lastCheckedAt: at, reason: snapshot.reason)
    case .serverStateReceived(let state, let checkedAt, let reason):
      return P031FreshnessSnapshot(state: state, lastCheckedAt: checkedAt, reason: reason)
    case .refreshCompletedWithoutNewProjection(let checkedAt, let reason):
      return P031FreshnessSnapshot(state: snapshot.state, lastCheckedAt: checkedAt, reason: reason)
    case .refreshFailed(let checkedAt, let reason):
      return P031FreshnessSnapshot(state: .unavailable, lastCheckedAt: checkedAt, reason: reason)
    }
  }
}

enum P031ReadRefreshSurface: Equatable, Sendable {
  case runsHome
  case runDetail
  case stageDetail
  case stages
  case approvalsQueue
  case artifacts
  case reportMetadata
  case daemonLifecycle
  case visibleSurface
}

enum P031ReadRefreshPresenter {
  nonisolated static func feedbackText(for surface: P031ReadRefreshSurface) -> String {
    switch surface {
    case .runsHome, .runDetail, .visibleSurface:
      return "Checking latest data"
    case .stageDetail:
      return "Updating stage"
    case .stages:
      return "Updating stages"
    case .approvalsQueue:
      return "Updating approvals"
    case .artifacts:
      return "Refreshing artifacts"
    case .reportMetadata:
      return "Refreshing reports"
    case .daemonLifecycle:
      return "Checking daemon status"
    }
  }
}

struct P031ReadRefreshOutcome<Value: Sendable>: Sendable {
  let surface: P031ReadRefreshSurface
  let feedbackText: String
  let value: Value?
  let freshness: P031FreshnessSnapshot
  let errorDescription: String?

  var succeeded: Bool {
    value != nil && errorDescription == nil
  }
}

struct P031TargetedReadRefreshCoordinator<Store: P031WorkflowReadStore>: Sendable {
  let store: Store

  func refreshRuns(
    currentFreshness: P031FreshnessSnapshot,
    checkedAt: Date = Date()
  ) async -> P031ReadRefreshOutcome<[P031RunRowReadModel]> {
    await refresh(
      surface: .runsHome,
      currentFreshness: currentFreshness,
      checkedAt: checkedAt,
      read: { try await store.fetchRuns() },
      serverStates: { $0.map(\.freshnessState) }
    )
  }

  func refreshRunDetail(
    runID: String,
    currentFreshness: P031FreshnessSnapshot,
    checkedAt: Date = Date()
  ) async -> P031ReadRefreshOutcome<P031RunDetailReadModel> {
    await refresh(
      surface: .runDetail,
      currentFreshness: currentFreshness,
      checkedAt: checkedAt,
      read: { try await store.fetchRunDetail(runID: runID) },
      serverStates: { $0.freshnessStates }
    )
  }

  func refreshStages(
    runID: String,
    currentFreshness: P031FreshnessSnapshot,
    checkedAt: Date = Date()
  ) async -> P031ReadRefreshOutcome<[P031StageReadModel]> {
    await refresh(
      surface: .stages,
      currentFreshness: currentFreshness,
      checkedAt: checkedAt,
      read: { try await store.fetchStages(runID: runID) },
      serverStates: { $0.map(\.freshnessState) }
    )
  }

  func refreshStageDetail(
    stageExecutionID: String,
    currentFreshness: P031FreshnessSnapshot,
    checkedAt: Date = Date()
  ) async -> P031ReadRefreshOutcome<P031StageDetailReadModel> {
    await refresh(
      surface: .stageDetail,
      currentFreshness: currentFreshness,
      checkedAt: checkedAt,
      read: { try await store.fetchStageDetail(stageExecutionID: stageExecutionID) },
      serverStates: { $0.freshnessStates }
    )
  }

  func refreshApprovalInbox(
    currentFreshness: P031FreshnessSnapshot,
    checkedAt: Date = Date()
  ) async -> P031ReadRefreshOutcome<[P031ApprovalReadModel]> {
    await refresh(
      surface: .approvalsQueue,
      currentFreshness: currentFreshness,
      checkedAt: checkedAt,
      read: { try await store.fetchApprovalInbox() },
      serverStates: { $0.map(\.freshnessState) }
    )
  }

  func refreshArtifacts(
    runID: String,
    currentFreshness: P031FreshnessSnapshot,
    checkedAt: Date = Date()
  ) async -> P031ReadRefreshOutcome<[P031ArtifactReadModel]> {
    await refresh(
      surface: .artifacts,
      currentFreshness: currentFreshness,
      checkedAt: checkedAt,
      read: { try await store.fetchArtifacts(runID: runID) },
      serverStates: { $0.map(\.freshnessState) }
    )
  }

  func refreshReportMetadata(
    runID: String,
    currentFreshness: P031FreshnessSnapshot,
    checkedAt: Date = Date()
  ) async -> P031ReadRefreshOutcome<[P031ReportMetadataReadModel]> {
    await refresh(
      surface: .reportMetadata,
      currentFreshness: currentFreshness,
      checkedAt: checkedAt,
      read: { try await store.fetchReportMetadata(runID: runID) },
      serverStates: { $0.map(\.freshnessState) }
    )
  }

  func refreshDaemonStatus(
    currentFreshness: P031FreshnessSnapshot,
    checkedAt: Date = Date()
  ) async -> P031ReadRefreshOutcome<P031DaemonStatusReadModel> {
    await refresh(
      surface: .daemonLifecycle,
      currentFreshness: currentFreshness,
      checkedAt: checkedAt,
      read: { try await store.fetchDaemonStatus() },
      serverStates: { _ in [.live] }
    )
  }

  private func refresh<Value: Sendable>(
    surface: P031ReadRefreshSurface,
    currentFreshness: P031FreshnessSnapshot,
    checkedAt: Date,
    read: @Sendable () async throws -> Value,
    serverStates: @Sendable (Value) -> [P031FreshnessState]
  ) async -> P031ReadRefreshOutcome<Value> {
    let feedbackText = P031ReadRefreshPresenter.feedbackText(for: surface)

    do {
      let value = try await read()
      let states = serverStates(value)
      let freshness: P031FreshnessSnapshot
      if let serverState = P031FreshnessAggregator.mostConservativeState(from: states) {
        freshness = WorkflowFreshnessReducer.reduce(
          currentFreshness,
          event: .serverStateReceived(serverState, checkedAt: checkedAt, reason: nil)
        )
      } else {
        freshness = WorkflowFreshnessReducer.reduce(
          currentFreshness,
          event: .refreshCompletedWithoutNewProjection(
            checkedAt: checkedAt,
            reason: "No newer projection returned"
          )
        )
      }

      return P031ReadRefreshOutcome(
        surface: surface,
        feedbackText: feedbackText,
        value: value,
        freshness: freshness,
        errorDescription: nil
      )
    } catch {
      return P031ReadRefreshOutcome(
        surface: surface,
        feedbackText: feedbackText,
        value: nil,
        freshness: WorkflowFreshnessReducer.reduce(
          currentFreshness,
          event: .refreshFailed(checkedAt: checkedAt, reason: P031ReadErrorPresenter.description(for: error))
        ),
        errorDescription: P031ReadErrorPresenter.description(for: error)
      )
    }
  }
}

enum P031FreshnessAggregator {
  nonisolated static func mostConservativeState(from states: [P031FreshnessState])
    -> P031FreshnessState?
  {
    let priority: [P031FreshnessState] = [
      .unauthorized, .unavailable, .stale, .projectionLag, .refreshing, .live,
    ]
    return priority.first { states.contains($0) }
  }
}

struct P031ApprovalReadModel: Decodable, Equatable, Sendable {
  let id: String
  let runID: String
  let stageID: String
  let decision: String?
  let freshnessState: P031FreshnessState
  let disabledReasonCode: P031DisabledReasonCode?
  let writePathState: P031WritePathState
  let diagnosticID: String?
  let serverDebugDetail: String?
  let availableActions: [String]
  let disabledReason: String?

  enum CodingKeys: String, CodingKey {
    case id
    case runID = "runId"
    case stageID = "stageId"
    case decision
    case freshnessState
    case disabledReasonCode
    case writePathState
    case diagnosticID = "diagnosticId"
    case serverDebugDetail
    case availableActions
    case disabledReason
  }

  init(
    id: String,
    runID: String,
    stageID: String,
    decision: String?,
    freshnessState: P031FreshnessState,
    disabledReasonCode: P031DisabledReasonCode?,
    writePathState: P031WritePathState,
    diagnosticID: String?,
    serverDebugDetail: String?,
    availableActions: [String] = [],
    disabledReason: String? = nil
  ) {
    self.id = id
    self.runID = runID
    self.stageID = stageID
    self.decision = decision
    self.freshnessState = freshnessState
    self.disabledReasonCode = disabledReasonCode
    self.writePathState = writePathState
    self.diagnosticID = diagnosticID
    self.serverDebugDetail = serverDebugDetail
    self.availableActions = availableActions
    self.disabledReason = disabledReason
  }

  init(from decoder: Decoder) throws {
    let container = try decoder.container(keyedBy: CodingKeys.self)
    id = try container.decode(String.self, forKey: .id)
    runID = try container.decode(String.self, forKey: .runID)
    stageID = try container.decode(String.self, forKey: .stageID)
    decision = try container.decodeIfPresent(String.self, forKey: .decision)
    freshnessState = try container.decode(P031FreshnessState.self, forKey: .freshnessState)
    disabledReasonCode = try container.decodeIfPresent(
      P031DisabledReasonCode.self,
      forKey: .disabledReasonCode
    )
    writePathState = try container.decode(P031WritePathState.self, forKey: .writePathState)
    diagnosticID = try container.decodeIfPresent(String.self, forKey: .diagnosticID)
    serverDebugDetail = try container.decodeIfPresent(String.self, forKey: .serverDebugDetail)
    availableActions =
      try container.decodeIfPresent([String].self, forKey: .availableActions) ?? []
    disabledReason = try container.decodeIfPresent(String.self, forKey: .disabledReason)
  }

  nonisolated var canApprove: Bool {
    isActionableDecision && availableActions.contains("approve") && writePathState == .available
  }

  nonisolated var canReject: Bool {
    isActionableDecision && availableActions.contains("reject") && writePathState == .available
  }

  private nonisolated var isActionableDecision: Bool {
    decision == nil || decision == "pending" || decision == "requested"
  }
}

struct P031ReportMetadataReadModel: Decodable, Equatable, Sendable {
  let id: String
  let name: String?
  let format: String?
  let reportKind: String?
  let reportVersion: Int?
  let freshnessState: P031FreshnessState
  let payloadAvailabilityState: P031PayloadAvailabilityState
  let payloadUnavailableReasonCode: P031PayloadUnavailableReasonCode?
  let diagnosticID: String?
  let serverDebugDetail: String?

  nonisolated var isReportMetadata: Bool {
    let normalizedFormat = format?.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
    let normalizedKind = reportKind?.trimmingCharacters(in: .whitespacesAndNewlines)
    return normalizedFormat == "report" || normalizedKind?.isEmpty == false
  }

  enum CodingKeys: String, CodingKey {
    case id
    case name
    case format
    case reportKind
    case reportVersion
    case freshnessState
    case payloadAvailabilityState
    case payloadUnavailableReasonCode
    case diagnosticID = "diagnosticId"
    case serverDebugDetail
  }
}

struct P088PublicEnumReadback: Decodable, Equatable, Sendable {
  let value: String
  let raw: String?
  let known: Bool

  nonisolated init(value: String, raw: String? = nil, known: Bool) {
    self.value = value
    self.raw = raw
    self.known = known
  }

  nonisolated static func known(value: String) -> P088PublicEnumReadback {
    P088PublicEnumReadback(value: value, raw: value, known: true)
  }
}

struct P088ImplementationCompletionTextCaptureReadModel: Decodable, Equatable, Sendable {
  let promptKind: String
  let turnIndex: Int
  let terminalResponseStatus: String?
  let completionTextStatus: String
  let completionTextCaptureSource: String?
  let completionTextRawByteLimit: Int?
  let completionTextCapturedByteCount: Int?
  let completionTextTruncated: Bool
  let extractionInputTruncated: Bool
  let extractionInputSha256: String?
  let redactedTextArtifactPath: String?
  let textAbsenceReason: String?
  let createdAt: String?
}

struct P088ImplementationCompletionReadModel: Decodable, Equatable, Sendable {
  let status: P088PublicEnumReadback
  let failureClass: String?
  let workChangeKind: String?
  let activationSource: String?
  let ingestionBoundaryFailure: P088PublicEnumReadback
  let completionTurnAttempted: Bool
  let completionTurnResult: P088PublicEnumReadback
  let terminalResponseStatus: String?
  let completionTextCaptures: [P088ImplementationCompletionTextCaptureReadModel]
  let freshRequiredOutputCount: Int
  let staleRequiredOutputCount: Int
  let missingRequiredOutputCount: Int
  let controlPlaneOutputCount: Int
  let receiptArtifactPath: String?
  let failedStageEvidencePath: String?
  let nextOperatorAction: P088PublicEnumReadback

  nonisolated init(
    status: P088PublicEnumReadback,
    failureClass: String?,
    workChangeKind: String?,
    activationSource: String?,
    ingestionBoundaryFailure: P088PublicEnumReadback,
    completionTurnAttempted: Bool,
    completionTurnResult: P088PublicEnumReadback,
    terminalResponseStatus: String?,
    completionTextCaptures: [P088ImplementationCompletionTextCaptureReadModel],
    freshRequiredOutputCount: Int,
    staleRequiredOutputCount: Int,
    missingRequiredOutputCount: Int,
    controlPlaneOutputCount: Int,
    receiptArtifactPath: String?,
    failedStageEvidencePath: String?,
    nextOperatorAction: P088PublicEnumReadback
  ) {
    self.status = status
    self.failureClass = failureClass
    self.workChangeKind = workChangeKind
    self.activationSource = activationSource
    self.ingestionBoundaryFailure = ingestionBoundaryFailure
    self.completionTurnAttempted = completionTurnAttempted
    self.completionTurnResult = completionTurnResult
    self.terminalResponseStatus = terminalResponseStatus
    self.completionTextCaptures = completionTextCaptures
    self.freshRequiredOutputCount = freshRequiredOutputCount
    self.staleRequiredOutputCount = staleRequiredOutputCount
    self.missingRequiredOutputCount = missingRequiredOutputCount
    self.controlPlaneOutputCount = controlPlaneOutputCount
    self.receiptArtifactPath = receiptArtifactPath
    self.failedStageEvidencePath = failedStageEvidencePath
    self.nextOperatorAction = nextOperatorAction
  }
}

enum P088ImplementationCompletionVisualState: Equatable, Sendable {
  case neutral
  case positive
  case warning
  case blocking
}

struct P088ImplementationCompletionPresentation: Equatable, Sendable {
  let compactSignalLabel: String
  let statusLabel: String
  let failureClassLabel: String?
  let workChangeKindLabel: String?
  let outputFreshnessLabel: String
  let primaryEvidencePath: String?
  let evidencePathLabel: String?
  let nextOperatorActionLabel: String
  let diagnosticRows: [String]
  let copyItems: [P031DiagnosticCopyItem]
  let accessibilityLabel: String
  let visualState: P088ImplementationCompletionVisualState
}

enum P088ImplementationCompletionPresenter {
  nonisolated static func presentationIfPresent(
    for readback: P088ImplementationCompletionReadModel?
  ) -> P088ImplementationCompletionPresentation? {
    guard let readback, readback.status.value != "not_attempted" else {
      return nil
    }
    return presentation(for: readback)
  }

  nonisolated static func presentation(
    for readback: P088ImplementationCompletionReadModel
  ) -> P088ImplementationCompletionPresentation {
    let statusLabel = P031ThinPresentationFormatting.titleCase(readback.status.value)
    let compactSignalLabel = "Implementation Completion: \(statusLabel)"
    let failureClassLabel = normalizedText(readback.failureClass).map { "Failure class: \($0)" }
    let workChangeKindLabel = normalizedText(readback.workChangeKind).map { "Work change: \($0)" }
    let outputFreshnessLabel = [
      "\(readback.freshRequiredOutputCount) fresh",
      "\(readback.staleRequiredOutputCount) stale",
      "\(readback.missingRequiredOutputCount) missing",
      "\(readback.controlPlaneOutputCount) control-plane",
    ].joined(separator: ", ")
    let evidencePath = normalizedText(readback.receiptArtifactPath)
      ?? normalizedText(readback.failedStageEvidencePath)
    let evidencePathLabel = evidencePath.map { "Evidence: \($0)" }
    let nextOperatorActionLabel =
      "Next: \(P031ThinPresentationFormatting.titleCase(readback.nextOperatorAction.value))"
    let capturedCount = readback.completionTextCaptures.filter {
      $0.completionTextStatus == "captured"
    }.count
    let absentCount = readback.completionTextCaptures.filter {
      $0.completionTextStatus != "captured"
    }.count

    let diagnosticRows = [
      failureClassLabel,
      workChangeKindLabel,
      normalizedText(readback.activationSource).map { "Activation: \($0)" },
      "Ingestion boundary: \(readback.ingestionBoundaryFailure.value)",
      "Completion turn: \(readback.completionTurnAttempted ? "attempted" : "not attempted") / \(readback.completionTurnResult.value)",
      normalizedText(readback.terminalResponseStatus).map { "Terminal response: \($0)" },
      "Outputs: \(readback.freshRequiredOutputCount) fresh, \(readback.staleRequiredOutputCount) stale, \(readback.missingRequiredOutputCount) missing, \(readback.controlPlaneOutputCount) control-plane",
      "Capture: \(capturedCount) captured, \(absentCount) absent",
      evidencePathLabel,
      nextOperatorActionLabel,
    ].compactMap { $0 }

    let copyItems = [
      readback.receiptArtifactPath.map {
        P031DiagnosticCopyItem(label: "Receipt path", value: $0)
      },
      readback.failedStageEvidencePath.map {
        P031DiagnosticCopyItem(label: "Failed-stage evidence path", value: $0)
      },
    ].compactMap { $0 }

    return P088ImplementationCompletionPresentation(
      compactSignalLabel: compactSignalLabel,
      statusLabel: statusLabel,
      failureClassLabel: failureClassLabel,
      workChangeKindLabel: workChangeKindLabel,
      outputFreshnessLabel: "Outputs: \(outputFreshnessLabel)",
      primaryEvidencePath: evidencePath,
      evidencePathLabel: evidencePathLabel,
      nextOperatorActionLabel: nextOperatorActionLabel,
      diagnosticRows: diagnosticRows,
      copyItems: copyItems,
      accessibilityLabel: ([compactSignalLabel, failureClassLabel, workChangeKindLabel,
                            "Outputs: \(outputFreshnessLabel)", evidencePathLabel,
                            nextOperatorActionLabel] as [String?]).compactMap { $0 }.joined(separator: ", "),
      visualState: visualState(for: readback.status.value)
    )
  }

  private nonisolated static func visualState(for status: String) -> P088ImplementationCompletionVisualState {
    switch status {
    case "succeeded", "not_attempted":
      return .positive
    case "partial_evidence", "repair_succeeded":
      return .warning
    case "failed", "repair_failed":
      return .blocking
    default:
      return .neutral
    }
  }

  private nonisolated static func normalizedText(_ value: String?) -> String? {
    let trimmed = value?.trimmingCharacters(in: .whitespacesAndNewlines)
    return trimmed?.isEmpty == false ? trimmed : nil
  }
}

enum P078SideEffectReadbackVisualState: Equatable, Sendable {
  case neutral
  case blocking
}

struct P078SideEffectReadbackPresentation: Equatable, Sendable {
  let compactSignalLabel: String
  let statusLabel: String
  let nextOperatorActionLabel: String
  let diagnosticRows: [String]
  let copyItems: [P031DiagnosticCopyItem]
  let accessibilityLabel: String
  let visualState: P078SideEffectReadbackVisualState
}

enum P078SideEffectReadbackPresenter {
  nonisolated static func presentationIfPresent(
    for readback: SideEffectReadbackSummary?
  ) -> P078SideEffectReadbackPresentation? {
    guard let readback, readback.blocked || readback.unresolvedCount > 0 else {
      return nil
    }
    return presentation(for: readback)
  }

  nonisolated static func presentation(
    for readback: SideEffectReadbackSummary
  ) -> P078SideEffectReadbackPresentation {
    let first = readback.effects.first
    let action = first?.operatorNextAction ?? "effects.inspect"
    let compact = "Release Side Effects: \(readback.unresolvedCount) unresolved"
    let rows = readback.effects.flatMap { effect in
      [
        "\(effect.effectKind): \(effect.status)",
        "Blocked: \(effect.blockedReason)",
        "Next: \(effect.operatorNextAction)",
        effect.reportPath.map { "Evidence: \($0)" },
      ].compactMap { $0 }
    }
    let copyItems = readback.effects.flatMap { effect in
      [
        P031DiagnosticCopyItem(label: "Effect ID", value: effect.id),
        effect.reportPath.map { P031DiagnosticCopyItem(label: "Evidence path", value: $0) },
      ].compactMap { $0 }
    }

    return P078SideEffectReadbackPresentation(
      compactSignalLabel: compact,
      statusLabel: readback.blocked ? "Release blocked" : "Release side effects clear",
      nextOperatorActionLabel: "Next: \(action)",
      diagnosticRows: rows,
      copyItems: copyItems,
      accessibilityLabel: ([compact, first?.blockedReason, "Next: \(action)"] as [String?])
        .compactMap { $0 }
        .joined(separator: ", "),
      visualState: readback.blocked ? .blocking : .neutral
    )
  }
}

struct P031RunRowReadModel: Decodable, Equatable, Sendable {
  let id: String
  let status: String
  let ideaID: String?
  let ideaTitle: String?
  let projectKey: String?
  let workflowTitle: String
  let workflowID: String?
  let workflowSnapshotHash: String?
  let catalogSnapshotHash: String?
  let freshnessState: P031FreshnessState
  let totalStages: Int?
  let completedStages: Int?
  let failedStages: Int?
  let pendingApprovals: Int?
  let closeoutReadinessSummary: P077CloseoutReadinessSummaryReadModel?
  let rolloutDecisionSummary: RolloutDecisionSummary?
  let implementationCompletion: P088ImplementationCompletionReadModel?
  let sideEffectReadback: SideEffectReadbackSummary?

  nonisolated init(
    id: String,
    status: String,
    ideaID: String? = nil,
    ideaTitle: String? = nil,
    projectKey: String? = nil,
    workflowTitle: String,
    workflowID: String? = nil,
    workflowSnapshotHash: String? = nil,
    catalogSnapshotHash: String? = nil,
    freshnessState: P031FreshnessState,
    totalStages: Int?,
    completedStages: Int?,
    failedStages: Int?,
    pendingApprovals: Int?,
    closeoutReadinessSummary: P077CloseoutReadinessSummaryReadModel? = nil,
    rolloutDecisionSummary: RolloutDecisionSummary? = nil,
    implementationCompletion: P088ImplementationCompletionReadModel? = nil,
    sideEffectReadback: SideEffectReadbackSummary? = nil
  ) {
    self.id = id
    self.status = status
    self.ideaID = ideaID
    self.ideaTitle = ideaTitle
    self.projectKey = projectKey
    self.workflowTitle = workflowTitle
    self.workflowID = workflowID
    self.workflowSnapshotHash = workflowSnapshotHash
    self.catalogSnapshotHash = catalogSnapshotHash
    self.freshnessState = freshnessState
    self.totalStages = totalStages
    self.completedStages = completedStages
    self.failedStages = failedStages
    self.pendingApprovals = pendingApprovals
    self.closeoutReadinessSummary = closeoutReadinessSummary
    self.rolloutDecisionSummary = rolloutDecisionSummary
    self.implementationCompletion = implementationCompletion
    self.sideEffectReadback = sideEffectReadback
  }

  enum CodingKeys: String, CodingKey {
    case id
    case status
    case ideaID = "ideaId"
    case ideaTitle
    case projectKey
    case workflowTitle
    case workflowID = "workflowId"
    case workflowSnapshotHash
    case catalogSnapshotHash
    case freshnessState
    case totalStages
    case completedStages
    case failedStages
    case pendingApprovals
    case implementationCompletion
    case implementationCloseoutReadinessSummary
    case closeoutReadinessSummaryJson
    case rolloutDecisionSummary = "rolloutContractReadbackJson"
    case sideEffectReadback = "sideEffectReadbackJson"
  }

  init(from decoder: Decoder) throws {
    let container = try decoder.container(keyedBy: CodingKeys.self)
    self.init(
      id: try container.decode(String.self, forKey: .id),
      status: try container.decode(String.self, forKey: .status),
      ideaID: try container.decodeIfPresent(String.self, forKey: .ideaID),
      ideaTitle: try container.decodeIfPresent(String.self, forKey: .ideaTitle),
      projectKey: try container.decodeIfPresent(String.self, forKey: .projectKey),
      workflowTitle: try container.decode(String.self, forKey: .workflowTitle),
      workflowID: try container.decodeIfPresent(String.self, forKey: .workflowID),
      workflowSnapshotHash: try container.decodeIfPresent(String.self, forKey: .workflowSnapshotHash),
      catalogSnapshotHash: try container.decodeIfPresent(String.self, forKey: .catalogSnapshotHash),
      freshnessState: try container.decode(P031FreshnessState.self, forKey: .freshnessState),
      totalStages: try container.decodeIfPresent(Int.self, forKey: .totalStages),
      completedStages: try container.decodeIfPresent(Int.self, forKey: .completedStages),
      failedStages: try container.decodeIfPresent(Int.self, forKey: .failedStages),
      pendingApprovals: try container.decodeIfPresent(Int.self, forKey: .pendingApprovals),
      closeoutReadinessSummary: try container.decodeIfPresent(
        P077CloseoutReadinessSummaryReadModel.self,
        forKey: .implementationCloseoutReadinessSummary
      )
        ?? container.decodeIfPresent(
          P077CloseoutReadinessSummaryReadModel.self,
          forKey: .closeoutReadinessSummaryJson
        ),
      rolloutDecisionSummary: try container.decodeIfPresent(
        RolloutDecisionSummary.self,
        forKey: .rolloutDecisionSummary
      ),
      implementationCompletion: try container.decodeIfPresent(
        P088ImplementationCompletionReadModel.self,
        forKey: .implementationCompletion
      ),
      sideEffectReadback: try container.decodeIfPresent(
        SideEffectReadbackSummary.self,
        forKey: .sideEffectReadback
      )
    )
  }

  nonisolated func withIdeaTitle(_ title: String?) -> P031RunRowReadModel {
    P031RunRowReadModel(
      id: id,
      status: status,
      ideaID: ideaID,
      ideaTitle: title ?? ideaTitle,
      projectKey: projectKey,
      workflowTitle: workflowTitle,
      workflowID: workflowID,
      workflowSnapshotHash: workflowSnapshotHash,
      catalogSnapshotHash: catalogSnapshotHash,
      freshnessState: freshnessState,
      totalStages: totalStages,
      completedStages: completedStages,
      failedStages: failedStages,
      pendingApprovals: pendingApprovals,
      closeoutReadinessSummary: closeoutReadinessSummary,
      rolloutDecisionSummary: rolloutDecisionSummary,
      implementationCompletion: implementationCompletion,
      sideEffectReadback: sideEffectReadback
    )
  }

  /// P036: canonical lane classification. Fails to .deferred for any status not in the
  /// typed RunStatus vocabulary — never guesses terminality or blockage from string heuristics.
  nonisolated var lane: P036RunLane {
    if (pendingApprovals ?? 0) > 0 { return .waiting }
    guard let bucket = RunStatus.from(serverValue: status) else { return .deferred }
    switch bucket {
    case .failed, .blocked: return .blocked
    case .running, .cancelling, .pending, .ready, .waitingApproval: return .running
    case .completed, .cancelled: return .completed
    }
  }
}

struct P031IdeaReadModel: Decodable, Equatable, Sendable {
  let id: String
  let title: String
  let body: String?
  let workspaceRootPath: String?
  let projectKey: String?
  let status: String?
  let createdAt: String?
  let archivedAt: String?

  nonisolated init(
    id: String,
    title: String,
    body: String? = nil,
    workspaceRootPath: String? = nil,
    projectKey: String? = nil,
    status: String? = nil,
    createdAt: String? = nil,
    archivedAt: String? = nil
  ) {
    self.id = id
    self.title = title
    self.body = body
    self.workspaceRootPath = workspaceRootPath
    self.projectKey = projectKey
    self.status = status
    self.createdAt = createdAt
    self.archivedAt = archivedAt
  }
}

struct P031RunStatusChangedReadModel: Decodable, Equatable, Sendable {
  let id: String
  let status: String
  let freshnessState: P031FreshnessState
  let projectionUpdatedAt: String?
  let projectionLag: Bool?
}

struct P031RuntimeTimelineEventReadModel: Decodable, Equatable, Sendable {
  let id: String?
  let runID: String
  let stageID: String
  let agentID: String
  let provider: String
  let eventKind: String
  let title: String?
  let detail: String?
  let surfaceLabel: String?
  let sessionGenerationID: String?
  let timestamp: String
  let rawDetail: String?
  let rawDetailBytes: Int?
  let rawDetailTruncated: Bool?
  let rawDetailHandle: String?
  let rawDetailDigest: String?
  let fullRawAvailable: Bool?
  let detailDigest: String?
  let detailCharCount: Int?
  let chunkCount: Int?
  let isStreaming: Bool?
  let isTerminal: Bool?
  let stateLabel: String?

  enum CodingKeys: String, CodingKey {
    case id
    case runID = "runId"
    case stageID = "stageId"
    case agentID = "agentId"
    case provider
    case eventKind
    case title
    case detail
    case surfaceLabel
    case sessionGenerationID = "sessionGenerationId"
    case timestamp
    case rawDetail
    case rawDetailBytes
    case rawDetailTruncated
    case rawDetailHandle
    case rawDetailDigest
    case fullRawAvailable
    case detailDigest
    case detailCharCount
    case chunkCount
    case isStreaming
    case isTerminal
    case stateLabel
  }

  nonisolated init(
    id: String? = nil,
    runID: String,
    stageID: String,
    agentID: String,
    provider: String,
    eventKind: String,
    title: String? = nil,
    detail: String? = nil,
    surfaceLabel: String? = nil,
    sessionGenerationID: String? = nil,
    timestamp: String,
    rawDetail: String? = nil,
    rawDetailBytes: Int? = nil,
    rawDetailTruncated: Bool? = nil,
    rawDetailHandle: String? = nil,
    rawDetailDigest: String? = nil,
    fullRawAvailable: Bool? = nil,
    detailDigest: String? = nil,
    detailCharCount: Int? = nil,
    chunkCount: Int? = nil,
    isStreaming: Bool? = nil,
    isTerminal: Bool? = nil,
    stateLabel: String? = nil
  ) {
    self.id = id
    self.runID = runID
    self.stageID = stageID
    self.agentID = agentID
    self.provider = provider
    self.eventKind = eventKind
    self.title = title
    self.detail = detail
    self.surfaceLabel = surfaceLabel
    self.sessionGenerationID = sessionGenerationID
    self.timestamp = timestamp
    self.rawDetail = rawDetail
    self.rawDetailBytes = rawDetailBytes
    self.rawDetailTruncated = rawDetailTruncated
    self.rawDetailHandle = rawDetailHandle
    self.rawDetailDigest = rawDetailDigest
    self.fullRawAvailable = fullRawAvailable
    self.detailDigest = detailDigest
    self.detailCharCount = detailCharCount
    self.chunkCount = chunkCount
    self.isStreaming = isStreaming
    self.isTerminal = isTerminal
    self.stateLabel = stateLabel
  }

  nonisolated init(from decoder: Decoder) throws {
    let container = try decoder.container(keyedBy: CodingKeys.self)
    self.id = try container.decodeIfPresent(String.self, forKey: .id)
    self.runID = try container.decode(String.self, forKey: .runID)
    self.stageID = try container.decode(String.self, forKey: .stageID)
    self.agentID = try container.decode(String.self, forKey: .agentID)
    self.provider = try container.decode(String.self, forKey: .provider)
    self.eventKind = try container.decode(String.self, forKey: .eventKind)
    self.title = try container.decodeIfPresent(String.self, forKey: .title)
    self.detail = try container.decodeIfPresent(String.self, forKey: .detail)
    self.surfaceLabel = try container.decodeIfPresent(String.self, forKey: .surfaceLabel)
    self.sessionGenerationID = try container.decodeIfPresent(String.self, forKey: .sessionGenerationID)
    self.timestamp = try container.decode(String.self, forKey: .timestamp)
    self.rawDetail = try container.decodeIfPresent(String.self, forKey: .rawDetail)
    self.rawDetailBytes = try container.decodeIfPresent(Int.self, forKey: .rawDetailBytes)
    self.rawDetailTruncated = try container.decodeIfPresent(Bool.self, forKey: .rawDetailTruncated)
    self.rawDetailHandle = try container.decodeIfPresent(String.self, forKey: .rawDetailHandle)
    self.rawDetailDigest = try container.decodeIfPresent(String.self, forKey: .rawDetailDigest)
    self.fullRawAvailable = try container.decodeIfPresent(Bool.self, forKey: .fullRawAvailable)
    self.detailDigest = try container.decodeIfPresent(String.self, forKey: .detailDigest)
    self.detailCharCount = try container.decodeIfPresent(Int.self, forKey: .detailCharCount)
    self.chunkCount = try container.decodeIfPresent(Int.self, forKey: .chunkCount)
    self.isStreaming = try container.decodeIfPresent(Bool.self, forKey: .isStreaming)
    self.isTerminal = try container.decodeIfPresent(Bool.self, forKey: .isTerminal)
    self.stateLabel = try container.decodeIfPresent(String.self, forKey: .stateLabel)
  }
}

enum P031TimelineRawDetailStatus: String, Decodable, Equatable, Sendable {
  case available
  case missing
  case stale
  case unauthorized
  case unavailable
  case digestMismatch = "digest_mismatch"
}

enum P031TimelineRawDetailErrorReason: String, Decodable, Equatable, Sendable {
  case handleNotFound = "handle_not_found"
  case handleExpired = "handle_expired"
  case runNotAuthorized = "run_not_authorized"
  case eventNotAuthorized = "event_not_authorized"
  case storageUnavailable = "storage_unavailable"
  case digestValidationFailed = "digest_validation_failed"
}

struct P031TimelineRawDetailReadModel: Decodable, Equatable, Sendable {
  let status: P031TimelineRawDetailStatus
  let rawDetail: String?
  let rawDetailBytes: Int?
  let rawDetailDigest: String?
  let errorReason: P031TimelineRawDetailErrorReason?
}

enum P031DaemonLifecycleState: String, CaseIterable, Equatable, Sendable {
  case notStarted = "not_started"
  case starting
  case ready
  case degraded
  case restarting
  case failed
  case shutdown
}

struct P031DaemonStatusReadModel: Equatable, Sendable {
  let state: P031DaemonLifecycleState
  let schemaVersion: Int?
  let binarySchemaVersion: Int?
  let buildSHA: String?
  let startedAt: String?
  let lastStateChangeAt: String?
  let restartCountSinceBoot: Int?
  let pid: Int?
  let rawJSON: String
}

enum P077CloseoutReadinessStatus: String, Equatable, Sendable, Decodable {
  case ready
  case readyWithRisks = "ready_with_risks"
  case handoffRequired = "handoff_required"
  case notReady = "not_ready"
  case blocked
  case invalid
  case unknown
}

struct P077CloseoutReadinessSummaryReadModel: Decodable, Equatable, Sendable {
  let runID: String
  let stageID: String
  let readinessStatus: P077CloseoutReadinessStatus
  let readinessDecision: String
  let readinessGenerationID: String
  let readinessMode: String
  let gateStatus: String
  let gateGenerationID: String
  let auditStatus: String?
  let diagnosticReason: String?
  let primaryUnblock: String?
  let codeBlockerCount: Int
  let handoffCount: Int
  let handoffOwner: String?
  let riskSettlementRequired: Bool
  let acceptedRiskCount: Int
  let fingerprintHash: String?
  let summary: String?
  let synthesizedAt: String?
  let isApplicable: Bool

  nonisolated var generationDisplayID: String {
    guard !readinessGenerationID.isEmpty else { return "none" }
    return String(readinessGenerationID.prefix(8))
  }

  nonisolated var hasGenerationID: Bool {
    !readinessGenerationID.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
  }

  init(from decoder: Decoder) throws {
    let container = try decoder.container(keyedBy: P031FlexibleCodingKey.self)
    runID = try container.decodeFlexible(String.self, keys: ["run_id", "runId"])
    stageID = try container.decodeFlexible(String.self, keys: ["stage_id", "stageId"])
    readinessStatus = try container.decodeFlexible(
      P077CloseoutReadinessStatus.self,
      keys: ["readiness_status", "readinessStatus"]
    )
    readinessDecision = try container.decodeFlexible(
      String.self,
      keys: ["readiness_decision", "readinessDecision"]
    )
    readinessGenerationID = try container.decodeFlexibleIfPresent(
      String.self,
      keys: ["readiness_generation_id", "readinessGenerationId", "readinessGenerationID"]
    ) ?? ""
    readinessMode = try container.decodeFlexibleIfPresent(
      String.self,
      keys: ["readiness_mode", "readinessMode"]
    ) ?? "advisory"
    gateStatus = try container.decodeFlexibleIfPresent(
      String.self,
      keys: ["gate_status", "gateStatus"]
    ) ?? "missing_definition"
    gateGenerationID = try container.decodeFlexibleIfPresent(
      String.self,
      keys: ["gate_generation_id", "gateGenerationId", "gateGenerationID"]
    ) ?? ""
    auditStatus = try container.decodeFlexibleIfPresent(
      String.self,
      keys: ["audit_status", "auditStatus"]
    )
    diagnosticReason = try container.decodeFlexibleIfPresent(
      String.self,
      keys: ["diagnostic_reason", "diagnosticReason"]
    )
    primaryUnblock = try container.decodeFlexibleIfPresent(
      String.self,
      keys: ["primary_unblock", "primaryUnblock"]
    )
    codeBlockerCount = try container.decodeFlexibleIfPresent(
      Int.self,
      keys: ["code_blocker_count", "codeBlockerCount"]
    ) ?? 0
    handoffCount = try container.decodeFlexibleIfPresent(
      Int.self,
      keys: ["handoff_count", "handoffCount"]
    ) ?? 0
    handoffOwner = try container.decodeFlexibleIfPresent(
      String.self,
      keys: ["handoff_owner", "handoffOwner"]
    )
    riskSettlementRequired = try container.decodeFlexibleIfPresent(
      Bool.self,
      keys: ["risk_settlement_required", "riskSettlementRequired"]
    ) ?? false
    acceptedRiskCount = try container.decodeFlexibleIfPresent(
      Int.self,
      keys: ["accepted_risk_count", "acceptedRiskCount"]
    ) ?? 0
    fingerprintHash = try container.decodeFlexibleIfPresent(
      String.self,
      keys: ["fingerprint_hash", "fingerprintHash"]
    )
    summary = try container.decodeFlexibleIfPresent(String.self, keys: ["summary"])
    synthesizedAt = try container.decodeFlexibleIfPresent(
      String.self,
      keys: ["synthesized_at", "synthesizedAt"]
    )
    isApplicable = try container.decodeFlexibleIfPresent(
      Bool.self,
      keys: ["is_applicable", "isApplicable"]
    ) ?? true
  }
}

struct P031FlexibleCodingKey: CodingKey {
  let stringValue: String
  let intValue: Int?

  init?(stringValue: String) {
    self.stringValue = stringValue
    intValue = nil
  }

  init?(intValue: Int) {
    stringValue = String(intValue)
    self.intValue = intValue
  }
}

extension KeyedDecodingContainer where Key == P031FlexibleCodingKey {
  func decodeFlexible<Value: Decodable>(_ type: Value.Type, keys: [String]) throws -> Value {
    for key in keys {
      guard let codingKey = P031FlexibleCodingKey(stringValue: key), contains(codingKey) else {
        continue
      }
      return try decode(type, forKey: codingKey)
    }
    throw DecodingError.keyNotFound(
      P031FlexibleCodingKey(stringValue: keys.first ?? "<empty>")!,
      DecodingError.Context(
        codingPath: codingPath,
        debugDescription: "Missing any of keys: \(keys.joined(separator: ", "))"
      )
    )
  }

  func decodeFlexibleIfPresent<Value: Decodable>(_ type: Value.Type, keys: [String]) throws -> Value? {
    for key in keys {
      guard let codingKey = P031FlexibleCodingKey(stringValue: key), contains(codingKey) else {
        continue
      }
      return try decodeIfPresent(type, forKey: codingKey)
    }
    return nil
  }
}

struct P031StageReadModel: Decodable, Equatable, Sendable {
  let id: String
  let runID: String
  let stageID: String
  let label: String
  let status: String
  let iteration: Int?
  let attemptNumber: Int?
  let startedAt: String?
  let completedAt: String?
  let settlementKind: String?
  let hasArtifacts: Bool?
  let hasPendingApproval: Bool?
  let hasValidationFailure: Bool?
  let projectionPresent: Bool
  let projectionUpdatedAt: String?
  let projectionLag: Bool
  let freshnessState: P031FreshnessState

  nonisolated init(
    id: String,
    runID: String,
    stageID: String,
    label: String,
    status: String,
    iteration: Int?,
    attemptNumber: Int?,
    startedAt: String? = nil,
    completedAt: String? = nil,
    settlementKind: String?,
    hasArtifacts: Bool?,
    hasPendingApproval: Bool?,
    hasValidationFailure: Bool?,
    projectionPresent: Bool,
    projectionUpdatedAt: String?,
    projectionLag: Bool,
    freshnessState: P031FreshnessState
  ) {
    self.id = id
    self.runID = runID
    self.stageID = stageID
    self.label = label
    self.status = status
    self.iteration = iteration
    self.attemptNumber = attemptNumber
    self.startedAt = startedAt
    self.completedAt = completedAt
    self.settlementKind = settlementKind
    self.hasArtifacts = hasArtifacts
    self.hasPendingApproval = hasPendingApproval
    self.hasValidationFailure = hasValidationFailure
    self.projectionPresent = projectionPresent
    self.projectionUpdatedAt = projectionUpdatedAt
    self.projectionLag = projectionLag
    self.freshnessState = freshnessState
  }

  enum CodingKeys: String, CodingKey {
    case id
    case runID = "runId"
    case stageID = "stageId"
    case label
    case status
    case iteration
    case attemptNumber
    case startedAt
    case completedAt
    case settlementKind
    case hasArtifacts
    case hasPendingApproval
    case hasValidationFailure
    case projectionPresent
    case projectionUpdatedAt
    case projectionLag
    case freshnessState
  }
}

struct P031ArtifactReadModel: Decodable, Equatable, Sendable {
  let id: String
  let runID: String
  let stageID: String
  let sourceStageExecutionID: String?
  let createdAt: String?
  let agentID: String?
  let name: String
  let contractID: String
  let format: String
  let isPinned: Bool?
  let reportKind: String?
  let reportVersion: Int?
  let outputSettlement: String?
  let sourceGenerationVerified: Bool?
  let freshnessState: P031FreshnessState
  let payloadAvailabilityState: P031PayloadAvailabilityState
  let payloadUnavailableReasonCode: P031PayloadUnavailableReasonCode?
  let payloadText: String?
  let diagnosticID: String?
  let serverDebugDetail: String?

  nonisolated init(
    id: String,
    runID: String,
    stageID: String,
    sourceStageExecutionID: String? = nil,
    createdAt: String? = nil,
    agentID: String? = nil,
    name: String,
    contractID: String,
    format: String,
    isPinned: Bool? = nil,
    reportKind: String? = nil,
    reportVersion: Int? = nil,
    outputSettlement: String? = nil,
    sourceGenerationVerified: Bool? = nil,
    freshnessState: P031FreshnessState,
    payloadAvailabilityState: P031PayloadAvailabilityState,
    payloadUnavailableReasonCode: P031PayloadUnavailableReasonCode? = nil,
    payloadText: String? = nil,
    diagnosticID: String? = nil,
    serverDebugDetail: String? = nil
  ) {
    self.id = id
    self.runID = runID
    self.stageID = stageID
    self.sourceStageExecutionID = sourceStageExecutionID
    self.createdAt = createdAt
    self.agentID = agentID
    self.name = name
    self.contractID = contractID
    self.format = format
    self.isPinned = isPinned
    self.reportKind = reportKind
    self.reportVersion = reportVersion
    self.outputSettlement = outputSettlement
    self.sourceGenerationVerified = sourceGenerationVerified
    self.freshnessState = freshnessState
    self.payloadAvailabilityState = payloadAvailabilityState
    self.payloadUnavailableReasonCode = payloadUnavailableReasonCode
    self.payloadText = payloadText
    self.diagnosticID = diagnosticID
    self.serverDebugDetail = serverDebugDetail
  }

  nonisolated var isReportMetadata: Bool {
    let normalizedFormat = format.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
    let normalizedKind = reportKind?.trimmingCharacters(in: .whitespacesAndNewlines)
    return normalizedFormat == "report" || normalizedKind?.isEmpty == false
  }

  nonisolated var reportMetadataReadModel: P031ReportMetadataReadModel {
    P031ReportMetadataReadModel(
      id: id,
      name: name,
      format: format,
      reportKind: reportKind,
      reportVersion: reportVersion,
      freshnessState: freshnessState,
      payloadAvailabilityState: payloadAvailabilityState,
      payloadUnavailableReasonCode: payloadUnavailableReasonCode,
      diagnosticID: diagnosticID,
      serverDebugDetail: serverDebugDetail
    )
  }

  enum CodingKeys: String, CodingKey {
    case id
    case runID = "runId"
    case stageID = "stageId"
    case sourceStageExecutionID = "sourceStageExecutionId"
    case createdAt
    case agentID = "agentId"
    case name
    case contractID = "contractId"
    case format
    case isPinned
    case reportKind
    case reportVersion
    case outputSettlement
    case sourceGenerationVerified
    case freshnessState
    case payloadAvailabilityState
    case payloadUnavailableReasonCode
    case payloadText
    case diagnosticID = "diagnosticId"
    case serverDebugDetail
  }
}

struct P031ActiveAgentExecutionReadModel: Decodable, Equatable, Sendable {
  let id: String
  let stageExecutionID: String
  let agentID: String
  let agentTitle: String?
  let provider: String
  let model: String?
  let status: String
  let startedAt: String?
  let completedAt: String?
  let stageLabel: String?
  let taskLabel: String?
  let lastEventAt: String?
  let eventCount: Int?
  let selectionOrder: Int?
  let selectionUnavailableReason: String?
  let sessionLineageID: String?
  let sessionGenerationID: String?

  nonisolated init(
    id: String,
    stageExecutionID: String,
    agentID: String,
    agentTitle: String? = nil,
    provider: String,
    model: String? = nil,
    status: String,
    startedAt: String? = nil,
    completedAt: String? = nil,
    stageLabel: String? = nil,
    taskLabel: String? = nil,
    lastEventAt: String? = nil,
    eventCount: Int? = nil,
    selectionOrder: Int? = nil,
    selectionUnavailableReason: String? = nil,
    sessionLineageID: String? = nil,
    sessionGenerationID: String? = nil
  ) {
    self.id = id
    self.stageExecutionID = stageExecutionID
    self.agentID = agentID
    self.agentTitle = agentTitle
    self.provider = provider
    self.model = model
    self.status = status
    self.startedAt = startedAt
    self.completedAt = completedAt
    self.stageLabel = stageLabel
    self.taskLabel = taskLabel
    self.lastEventAt = lastEventAt
    self.eventCount = eventCount
    self.selectionOrder = selectionOrder
    self.selectionUnavailableReason = selectionUnavailableReason
    self.sessionLineageID = sessionLineageID
    self.sessionGenerationID = sessionGenerationID
  }

  enum CodingKeys: String, CodingKey {
    case id
    case stageExecutionID = "stageExecutionId"
    case agentID = "agentId"
    case agentTitle
    case provider
    case model
    case status
    case startedAt
    case completedAt
    case stageLabel
    case taskLabel
    case lastEventAt
    case eventCount
    case selectionOrder
    case selectionUnavailableReason
    case sessionLineageID = "sessionLineageId"
    case sessionGenerationID = "sessionGenerationId"
  }
}

struct P031RunStageTopologyOccurrenceReadModel: Decodable, Equatable, Sendable {
  let agentID: String
  let agentTitle: String
  let taskName: String
  let status: String
  let provider: String
  let model: String?
  let effort: String?
  let executionCount: Int

  enum CodingKeys: String, CodingKey {
    case agentID = "agentId"
    case agentTitle
    case taskName
    case status
    case provider
    case model
    case effort
    case executionCount
  }
}

struct P031RunStageTopologyTransitionReadModel: Decodable, Equatable, Sendable {
  let toStageID: String
  let toLabel: String?
  let detail: String?

  enum CodingKeys: String, CodingKey {
    case toStageID = "toStageId"
    case toLabel
    case detail
  }
}

struct P031RunStageTopologyReadModel: Decodable, Equatable, Sendable {
  let stageID: String
  let label: String
  let order: Int
  let ownerAgentID: String
  let ownerAgentTitle: String
  let status: String
  let isCurrent: Bool
  let iteration: Int?
  let attemptNumber: Int?
  let approvalRequired: Bool
  let artifactCount: Int
  let communicationCount: Int
  let occurrences: [P031RunStageTopologyOccurrenceReadModel]
  let transitions: [P031RunStageTopologyTransitionReadModel]

  enum CodingKeys: String, CodingKey {
    case stageID = "stageId"
    case label
    case order
    case ownerAgentID = "ownerAgentId"
    case ownerAgentTitle
    case status
    case isCurrent
    case iteration
    case attemptNumber
    case approvalRequired
    case artifactCount
    case communicationCount
    case occurrences
    case transitions
  }
}

struct P031RunDetailReadModel: Decodable, Equatable, Sendable {
  let run: P031RunRowReadModel?
  let idea: P031IdeaReadModel?
  let stages: [P031StageReadModel]
  let artifacts: [P031ArtifactReadModel]
  let approvalInbox: [P031ApprovalReadModel]
  let activeAgentExecutions: [P031ActiveAgentExecutionReadModel]
  let runStageTopology: [P031RunStageTopologyReadModel]
  let continuations: [P086ContinuationRecordReadModel]
  let continuationMetricsSummary: P086ContinuationMetricsSummaryReadModel?

  nonisolated init(
    run: P031RunRowReadModel?,
    idea: P031IdeaReadModel? = nil,
    stages: [P031StageReadModel],
    artifacts: [P031ArtifactReadModel],
    approvalInbox: [P031ApprovalReadModel] = [],
    activeAgentExecutions: [P031ActiveAgentExecutionReadModel] = [],
    runStageTopology: [P031RunStageTopologyReadModel] = [],
    continuations: [P086ContinuationRecordReadModel] = [],
    continuationMetricsSummary: P086ContinuationMetricsSummaryReadModel? = nil
  ) {
    self.run = run
    self.idea = idea
    self.stages = stages
    self.artifacts = artifacts
    self.approvalInbox = approvalInbox
    self.activeAgentExecutions = activeAgentExecutions
    self.runStageTopology = runStageTopology
    self.continuations = continuations
    self.continuationMetricsSummary = continuationMetricsSummary
  }

  nonisolated var freshnessStates: [P031FreshnessState] {
    [run?.freshnessState].compactMap { $0 }
      + stages.map(\.freshnessState)
      + artifacts.map(\.freshnessState)
      + approvalInbox.map(\.freshnessState)
      + continuations.map(\.freshnessState)
  }

  nonisolated var ordinaryArtifacts: [P031ArtifactReadModel] {
    artifacts.filter { !$0.isReportMetadata }
  }

  nonisolated var reportMetadata: [P031ReportMetadataReadModel] {
    artifacts.filter(\.isReportMetadata).map(\.reportMetadataReadModel)
  }

  nonisolated var approvalsForRun: [P031ApprovalReadModel] {
    guard let runID = run?.id else {
      return []
    }
    return approvalInbox.filter { $0.runID == runID }
  }

  enum CodingKeys: String, CodingKey {
    case run
    case idea
    case stages
    case artifacts
    case approvalInbox
    case activeAgentExecutions
    case runStageTopology
    case continuations
    case continuationMetricsSummary
  }

  init(from decoder: Decoder) throws {
    let container = try decoder.container(keyedBy: CodingKeys.self)
    self.run = try container.decodeIfPresent(P031RunRowReadModel.self, forKey: .run)
    self.idea = try container.decodeIfPresent(P031IdeaReadModel.self, forKey: .idea)
    self.stages =
      try container.decodeIfPresent([P031StageReadModel].self, forKey: .stages) ?? []
    self.artifacts =
      try container.decodeIfPresent([P031ArtifactReadModel].self, forKey: .artifacts) ?? []
    self.approvalInbox =
      try container.decodeIfPresent([P031ApprovalReadModel].self, forKey: .approvalInbox) ?? []
    self.activeAgentExecutions =
      try container.decodeIfPresent(
        [P031ActiveAgentExecutionReadModel].self,
        forKey: .activeAgentExecutions
      ) ?? []
    self.runStageTopology =
      try container.decodeIfPresent(
        [P031RunStageTopologyReadModel].self,
        forKey: .runStageTopology
      ) ?? []
    self.continuations =
      try container.decodeIfPresent([P086ContinuationRecordReadModel].self, forKey: .continuations)
      ?? []
    self.continuationMetricsSummary =
      try container.decodeIfPresent(
        P086ContinuationMetricsSummaryReadModel.self,
        forKey: .continuationMetricsSummary
      )
  }
}

struct P086ContinuationRecordReadModel: Decodable, Equatable, Sendable {
  let id: String
  let runID: String
  let stageExecutionID: String
  let agentExecutionID: String
  let modeRaw: String
  let modeDisplay: String
  let triggerKindRaw: String
  let triggerKindDisplay: String
  let statusRaw: String
  let statusDisplay: String
  let isTerminal: Bool
  let failureReason: String?
  let reconciliationStatus: String?
  let requestFingerprintSHA256: String
  let canonicalRequestArtifactID: String?
  let attachReceiptArtifactID: String?
  let evidenceBundleArtifactID: String?
  let worktreeReadbackArtifactID: String?
  let continuationReportArtifactID: String?
  let responseFingerprintSHA256: String?
  let responseArtifactID: String?
  let resultOrNoProgressArtifactID: String?
  let conflictCount: Int
  let createdAt: String
  let updatedAt: String
  let freshnessState: P031FreshnessState
  let projectionLagMS: Int?

  enum CodingKeys: String, CodingKey {
    case id
    case runID = "runId"
    case stageExecutionID = "stageExecutionId"
    case agentExecutionID = "agentExecutionId"
    case modeRaw
    case modeDisplay
    case triggerKindRaw
    case triggerKindDisplay
    case statusRaw
    case statusDisplay
    case isTerminal
    case failureReason
    case reconciliationStatus
    case requestFingerprintSHA256 = "requestFingerprintSha256"
    case canonicalRequestArtifactID = "canonicalRequestArtifactId"
    case attachReceiptArtifactID = "attachReceiptArtifactId"
    case evidenceBundleArtifactID = "evidenceBundleArtifactId"
    case worktreeReadbackArtifactID = "worktreeReadbackArtifactId"
    case continuationReportArtifactID = "continuationReportArtifactId"
    case responseFingerprintSHA256 = "responseFingerprintSha256"
    case responseArtifactID = "responseArtifactId"
    case resultOrNoProgressArtifactID = "resultOrNoProgressArtifactId"
    case conflictCount
    case createdAt
    case updatedAt
    case freshnessState
    case projectionLagMS = "projectionLagMs"
  }
}

struct P086ContinuationMetricsSummaryReadModel: Decodable, Equatable, Sendable {
  let runID: String
  let admissionTotal: Int
  let acceptedTotal: Int
  let rejectedTotal: Int
  let replayTotal: Int
  let successTotal: Int
  let noProgressTotal: Int
  let failedTotal: Int
  let cancelledTotal: Int
  let freshSessionAvoidedTotal: Int
  let leadAutoTotal: Int
  let operatorMCPTotal: Int
  let changedFilesTotal: Int
  let testsOrGatesTotal: Int
  let terminalTotal: Int
  let usefulProgressTotal: Int
  let usefulProgressRate: Double
  let noProgressRate: Double
  let testsPassedAfterContinuationTotal: Int
  let followupValidationTotal: Int
  let followupValidationSuccessTotal: Int
  let followupValidationSuccessRate: Double
  let leadAutoSuccessTotal: Int
  let leadAutoSuccessRate: Double
  let operatorMCPSuccessTotal: Int
  let operatorMCPSuccessRate: Double
  let timeSavedSecondsTotal: Int
  let timeSavedSampleCount: Int
  let averageTimeSavedSeconds: Double
  let providerSessionBudgetInputTokensTotal: Int
  let providerSessionBudgetOutputTokensTotal: Int
  let providerSessionBudgetCachedInputTokensTotal: Int
  let providerSessionBudgetCostCentsTotal: Int
  let providerSessionResurrectionAttachSuccessTotal: Int
  let providerSessionResurrectionAttachFailureTotal: Int
  let orphanReapAttemptedTotal: Int
  let orphanReapVerifiedTotal: Int
  let resurrectionUnsupportedTotal: Int

  enum CodingKeys: String, CodingKey {
    case runID = "runId"
    case admissionTotal
    case acceptedTotal
    case rejectedTotal
    case replayTotal
    case successTotal
    case noProgressTotal
    case failedTotal
    case cancelledTotal
    case freshSessionAvoidedTotal
    case leadAutoTotal
    case operatorMCPTotal = "operatorMcpTotal"
    case changedFilesTotal
    case testsOrGatesTotal
    case terminalTotal
    case usefulProgressTotal
    case usefulProgressRate
    case noProgressRate
    case testsPassedAfterContinuationTotal
    case followupValidationTotal
    case followupValidationSuccessTotal
    case followupValidationSuccessRate
    case leadAutoSuccessTotal
    case leadAutoSuccessRate
    case operatorMCPSuccessTotal = "operatorMcpSuccessTotal"
    case operatorMCPSuccessRate = "operatorMcpSuccessRate"
    case timeSavedSecondsTotal
    case timeSavedSampleCount
    case averageTimeSavedSeconds
    case providerSessionBudgetInputTokensTotal
    case providerSessionBudgetOutputTokensTotal
    case providerSessionBudgetCachedInputTokensTotal
    case providerSessionBudgetCostCentsTotal
    case providerSessionResurrectionAttachSuccessTotal
    case providerSessionResurrectionAttachFailureTotal
    case orphanReapAttemptedTotal
    case orphanReapVerifiedTotal
    case resurrectionUnsupportedTotal
  }
}

struct P031StageDetailReadModel: Decodable, Equatable, Sendable {
  let stage: P031StageReadModel?

  nonisolated var freshnessStates: [P031FreshnessState] {
    [stage?.freshnessState].compactMap { $0 }
  }
}

protocol P031WorkflowReadStore: Sendable {
  func fetchIdeas(includeArchived: Bool) async throws -> [P031IdeaReadModel]
  func fetchRuns() async throws -> [P031RunRowReadModel]
  func fetchRunDetail(runID: String) async throws -> P031RunDetailReadModel
  func fetchIdea(id: String) async throws -> P031IdeaReadModel?
  func fetchStageDetail(stageExecutionID: String) async throws -> P031StageDetailReadModel
  func fetchStages(runID: String) async throws -> [P031StageReadModel]
  func fetchApprovalInbox() async throws -> [P031ApprovalReadModel]
  func fetchArtifacts(runID: String) async throws -> [P031ArtifactReadModel]
  func fetchArtifactPayload(artifactID: String) async throws -> P031ArtifactReadModel
  func fetchTimelineRawDetail(handle: String) async throws -> P031TimelineRawDetailReadModel
  func fetchReportMetadata(runID: String) async throws -> [P031ReportMetadataReadModel]
  func fetchDaemonStatus() async throws -> P031DaemonStatusReadModel
  nonisolated func subscribeToRunStatus(runID: String) throws -> AsyncThrowingStream<
    P031RunStatusChangedReadModel, Error
  >
  nonisolated func subscribeToRuntimeTimeline(runID: String) throws -> AsyncThrowingStream<
    P031RuntimeTimelineEventReadModel, Error
  >
  nonisolated func subscribeToDaemonStatus() throws -> AsyncThrowingStream<P031DaemonStatusReadModel, Error>
}

struct P031GraphQLDocumentSet: Equatable, Sendable {
  let ideas: String
  let runsHome: String
  let runDetail: String
  let stageDetail: String
  let stages: String
  let approvalInbox: String
  let artifacts: String
  let artifactPayload: String
  let timelineRawDetail: String
  let reportMetadata: String
  let daemonStatus: String
  let ideaTitle: String
  let runStatusChanged: String
  let runtimeStatusChanged: String
  let daemonStatusChanged: String
}

enum P031GraphQLDocuments {
  static let ideas = """
    query P031Ideas($includeArchived: Boolean) {
      ideas(includeArchived: $includeArchived) {
        id
        title
        body
        workspaceRootPath
        projectKey
        status
        createdAt
        archivedAt
      }
    }
    """

  static let runsHome = """
    query P031RunsHome {
      runs {
        id
        status
        ideaId
        projectKey
        workflowId
        workflowTitle
        workflowSnapshotHash
        catalogSnapshotHash
        freshnessState
        totalStages
        completedStages
        failedStages
        pendingApprovals
        implementationCompletion {
          status { value raw known }
          failureClass
          workChangeKind
          activationSource
          ingestionBoundaryFailure { value raw known }
          completionTurnAttempted
          completionTurnResult { value raw known }
          terminalResponseStatus
          completionTextCaptures {
            promptKind
            turnIndex
            terminalResponseStatus
            completionTextStatus
            completionTextCaptureSource
            completionTextRawByteLimit
            completionTextCapturedByteCount
            completionTextTruncated
            extractionInputTruncated
            extractionInputSha256
            redactedTextArtifactPath
            textAbsenceReason
            createdAt
          }
          freshRequiredOutputCount
          staleRequiredOutputCount
          missingRequiredOutputCount
          controlPlaneOutputCount
          receiptArtifactPath
          failedStageEvidencePath
          nextOperatorAction { value raw known }
        }
        implementationCloseoutReadinessSummary: closeoutReadinessSummaryJson
        rolloutContractReadbackJson
        sideEffectReadbackJson
      }
    }
    """

  static let runDetail = """
    query P031RunDetail($runId: ID!) {
      run(id: $runId) {
        id
        status
        ideaId
        projectKey
        workflowId
        workflowTitle
        workflowSnapshotHash
        catalogSnapshotHash
        freshnessState
        totalStages
        completedStages
        failedStages
        pendingApprovals
        implementationCompletion {
          status { value raw known }
          failureClass
          workChangeKind
          activationSource
          ingestionBoundaryFailure { value raw known }
          completionTurnAttempted
          completionTurnResult { value raw known }
          terminalResponseStatus
          completionTextCaptures {
            promptKind
            turnIndex
            terminalResponseStatus
            completionTextStatus
            completionTextCaptureSource
            completionTextRawByteLimit
            completionTextCapturedByteCount
            completionTextTruncated
            extractionInputTruncated
            extractionInputSha256
            redactedTextArtifactPath
            textAbsenceReason
            createdAt
          }
          freshRequiredOutputCount
          staleRequiredOutputCount
          missingRequiredOutputCount
          controlPlaneOutputCount
          receiptArtifactPath
          failedStageEvidencePath
          nextOperatorAction { value raw known }
        }
        implementationCloseoutReadinessSummary: closeoutReadinessSummaryJson
        rolloutContractReadbackJson
        sideEffectReadbackJson
      }
      stages(runId: $runId) {
        id
        runId
        stageId
        label
        status
        iteration
        attemptNumber
        startedAt
        completedAt
        settlementKind
        hasArtifacts
        hasPendingApproval
        hasValidationFailure
        projectionPresent
        projectionUpdatedAt
        projectionLag
        freshnessState
      }
      runStageTopology(runId: $runId) {
        stageId
        label
        order
        ownerAgentId
        ownerAgentTitle
        status
        isCurrent
        iteration
        attemptNumber
        approvalRequired
        artifactCount
        communicationCount
        occurrences {
          agentId
          agentTitle
          taskName
          status
          provider
          model
          effort
          executionCount
        }
        transitions {
          toStageId
          toLabel
          detail
        }
      }
      activeAgentExecutions(runId: $runId) {
        id
        stageExecutionId
        agentId
        agentTitle
        provider
        model
        status
        startedAt
        completedAt
        stageLabel
        taskLabel
        lastEventAt
        eventCount
        selectionOrder
        selectionUnavailableReason
        sessionLineageId
        sessionGenerationId
      }
      continuations(runId: $runId) {
        id
        runId
        stageExecutionId
        agentExecutionId
        modeRaw
        modeDisplay
        triggerKindRaw
        triggerKindDisplay
        statusRaw
        statusDisplay
        isTerminal
        failureReason
        reconciliationStatus
        requestFingerprintSha256
        canonicalRequestArtifactId
        attachReceiptArtifactId
        evidenceBundleArtifactId
        worktreeReadbackArtifactId
        continuationReportArtifactId
        responseFingerprintSha256
        responseArtifactId
        resultOrNoProgressArtifactId
        conflictCount
        createdAt
        updatedAt
        freshnessState
        projectionLagMs
      }
      continuationMetricsSummary(runId: $runId) {
        runId
        admissionTotal
        acceptedTotal
        rejectedTotal
        replayTotal
        successTotal
        noProgressTotal
        failedTotal
        cancelledTotal
        freshSessionAvoidedTotal
        leadAutoTotal
        operatorMcpTotal
        changedFilesTotal
        testsOrGatesTotal
        terminalTotal
        usefulProgressTotal
        usefulProgressRate
        noProgressRate
        testsPassedAfterContinuationTotal
        followupValidationTotal
        followupValidationSuccessTotal
        followupValidationSuccessRate
        leadAutoSuccessTotal
        leadAutoSuccessRate
        operatorMcpSuccessTotal
        operatorMcpSuccessRate
        timeSavedSecondsTotal
        timeSavedSampleCount
        averageTimeSavedSeconds
        providerSessionBudgetInputTokensTotal
        providerSessionBudgetOutputTokensTotal
        providerSessionBudgetCachedInputTokensTotal
        providerSessionBudgetCostCentsTotal
        providerSessionResurrectionAttachSuccessTotal
        providerSessionResurrectionAttachFailureTotal
        orphanReapAttemptedTotal
        orphanReapVerifiedTotal
        resurrectionUnsupportedTotal
      }
      artifacts(runId: $runId) {
        id
        runId
        stageId
        sourceStageExecutionId
        createdAt
        agentId
        name
        contractId
        format
        isPinned
        reportKind
        reportVersion
        outputSettlement
        sourceGenerationVerified
        freshnessState
        payloadAvailabilityState
        payloadUnavailableReasonCode
        diagnosticId
        serverDebugDetail
      }
      approvalInbox(runId: $runId) {
        id
        runId
        stageId
        decision
        freshnessState
        disabledReasonCode
        writePathState
        availableActions
        disabledReason
        diagnosticId
        serverDebugDetail
      }
    }
    """

  static let stageDetail = """
    query P031StageDetail($stageExecutionId: ID!) {
      stage(id: $stageExecutionId) {
        id
        runId
        stageId
        label
        status
        iteration
        attemptNumber
        startedAt
        completedAt
        settlementKind
        hasArtifacts
        hasPendingApproval
        hasValidationFailure
        projectionPresent
        projectionUpdatedAt
        projectionLag
        freshnessState
      }
    }
    """

  static let stages = """
    query P031Stages($runId: ID!) {
      stages(runId: $runId) {
        id
        runId
        stageId
        label
        status
        iteration
        attemptNumber
        startedAt
        completedAt
        settlementKind
        hasArtifacts
        hasPendingApproval
        hasValidationFailure
        projectionPresent
        projectionUpdatedAt
        projectionLag
        freshnessState
      }
    }
    """

  static let approvalInbox = """
    query P031ApprovalInbox {
      approvalInbox {
        id
        runId
        stageId
        decision
        freshnessState
        disabledReasonCode
        writePathState
        availableActions
        disabledReason
        diagnosticId
        serverDebugDetail
      }
    }
    """

  static let approveApproval = """
    mutation P072ApproveApproval($approvalId: ID!, $idempotencyKey: String!) {
      approveApproval(approvalId: $approvalId, idempotencyKey: $idempotencyKey) {
        approval {
          id
          runId
          stageId
          decision
          freshnessState
          disabledReasonCode
          writePathState
          availableActions
          disabledReason
          diagnosticId
          serverDebugDetail
        }
        journalId
        conflictResultCode
      }
    }
    """

  static let rejectApproval = """
    mutation P072RejectApproval($approvalId: ID!, $reason: String!, $idempotencyKey: String!) {
      rejectApproval(approvalId: $approvalId, reason: $reason, idempotencyKey: $idempotencyKey) {
        approval {
          id
          runId
          stageId
          decision
          freshnessState
          disabledReasonCode
          writePathState
          availableActions
          disabledReason
          diagnosticId
          serverDebugDetail
        }
        journalId
        conflictResultCode
      }
    }
    """

  static let artifacts = """
    query P031Artifacts($runId: ID!) {
      artifacts(runId: $runId) {
        id
        runId
        stageId
        sourceStageExecutionId
        createdAt
        agentId
        name
        contractId
        format
        isPinned
        reportKind
        reportVersion
        outputSettlement
        sourceGenerationVerified
        freshnessState
        payloadAvailabilityState
        payloadUnavailableReasonCode
        diagnosticId
        serverDebugDetail
      }
    }
    """

  static let artifactPayload = """
    query P031ArtifactPayload($artifactId: ID!) {
      artifact(id: $artifactId) {
        id
        runId
        stageId
        sourceStageExecutionId
        createdAt
        agentId
        name
        contractId
        format
        isPinned
        reportKind
        reportVersion
        outputSettlement
        sourceGenerationVerified
        freshnessState
        payloadAvailabilityState
        payloadUnavailableReasonCode
        payloadText
        diagnosticId
        serverDebugDetail
      }
    }
    """

  static let timelineRawDetail = """
    query P031TimelineRawDetail($handle: ID!) {
      timelineRawDetail(handle: $handle) {
        status
        rawDetail
        rawDetailBytes
        rawDetailDigest
        errorReason
      }
    }
    """

  static let reportMetadata = """
    query P031ReportMetadata($runId: ID!) {
      artifacts(runId: $runId) {
        id
        name
        format
        reportKind
        reportVersion
        freshnessState
        payloadAvailabilityState
        payloadUnavailableReasonCode
        diagnosticId
        serverDebugDetail
      }
    }
    """

  static let runStatusChanged = """
    subscription P031RunStatusChanged($runId: ID!) {
      runStatusChanged(runId: $runId) {
        id
        status
        freshnessState
        projectionUpdatedAt
        projectionLag
      }
    }
    """

  static let runtimeStatusChanged = """
    subscription P031RuntimeStatusChanged($runId: ID!) {
      runtimeStatusChanged(runId: $runId) {
        id
        runId
        stageId
        agentId
        provider
        eventKind
        title
        detail
        surfaceLabel
        sessionGenerationId
        timestamp
        rawDetail
        rawDetailBytes
        rawDetailTruncated
        rawDetailHandle
        rawDetailDigest
        fullRawAvailable
        detailDigest
        detailCharCount
        chunkCount
        isStreaming
        isTerminal
        stateLabel
      }
    }
    """

  static let daemonStatus = """
    query P031DaemonStatus {
      daemonStatus {
        json
      }
    }
    """

  static let ideaTitle = """
    query P031IdeaTitle($ideaId: ID!) {
      idea(id: $ideaId) {
        id
        title
        body
        workspaceRootPath
        projectKey
        status
        createdAt
        archivedAt
      }
    }
    """

  static let daemonStatusChanged = """
    subscription P031DaemonStatusChanged {
      daemonStatusChanged {
        json
      }
    }
    """

  nonisolated static let defaultSet = P031GraphQLDocumentSet(
    ideas: ideas,
    runsHome: runsHome,
    runDetail: runDetail,
    stageDetail: stageDetail,
    stages: stages,
    approvalInbox: approvalInbox,
    artifacts: artifacts,
    artifactPayload: artifactPayload,
    timelineRawDetail: timelineRawDetail,
    reportMetadata: reportMetadata,
    daemonStatus: daemonStatus,
    ideaTitle: ideaTitle,
    runStatusChanged: runStatusChanged,
    runtimeStatusChanged: runtimeStatusChanged,
    daemonStatusChanged: daemonStatusChanged
  )
}

struct P031GraphQLWorkflowReadStore<
  ReadTransport: P031GraphQLReadTransport, SubscriptionTransport: P031GraphQLSubscriptionTransport
>: P031WorkflowReadStore {
  private let readClient: P031GraphQLReadClient<ReadTransport>
  private let subscriptionClient: P031GraphQLSubscriptionClient<SubscriptionTransport>
  private let documents: P031GraphQLDocumentSet

  nonisolated init(
    readTransport: ReadTransport,
    subscriptionTransport: SubscriptionTransport,
    documents: P031GraphQLDocumentSet = P031GraphQLDocuments.defaultSet
  ) {
    self.readClient = P031GraphQLReadClient(transport: readTransport)
    self.subscriptionClient = P031GraphQLSubscriptionClient(transport: subscriptionTransport)
    self.documents = documents
  }

  func fetchIdeas(includeArchived: Bool = false) async throws -> [P031IdeaReadModel] {
    let payload = try await readClient.execute(
      IdeasPayload.self,
      operationName: "P031Ideas",
      document: documents.ideas,
      variables: ["includeArchived": .bool(includeArchived)]
    )
    return payload.ideas
  }

  func fetchRuns() async throws -> [P031RunRowReadModel] {
    let payload = try await readClient.execute(
      RunsPayload.self,
      operationName: "P031RunsHome",
      document: documents.runsHome
    )
    return await enrichRunsWithIdeaTitles(payload.runs)
  }

  func fetchRunDetail(runID: String) async throws -> P031RunDetailReadModel {
    let detail = try await readClient.execute(
      P031RunDetailReadModel.self,
      operationName: "P031RunDetail",
      document: documents.runDetail,
      variables: ["runId": .string(runID)]
    )
    guard let run = detail.run, let ideaID = run.ideaID, run.ideaTitle == nil,
      let ideaTitle = await fetchIdeaTitle(ideaID: ideaID)
    else {
      return detail
    }
    return P031RunDetailReadModel(
      run: run.withIdeaTitle(ideaTitle),
      idea: detail.idea,
      stages: detail.stages,
      artifacts: detail.artifacts,
      approvalInbox: detail.approvalInbox,
      activeAgentExecutions: detail.activeAgentExecutions,
      runStageTopology: detail.runStageTopology,
      continuations: detail.continuations,
      continuationMetricsSummary: detail.continuationMetricsSummary
    )
  }

  func fetchIdea(id: String) async throws -> P031IdeaReadModel? {
    let payload = try await readClient.execute(
      IdeaPayload.self,
      operationName: "P031IdeaTitle",
      document: documents.ideaTitle,
      variables: ["ideaId": .string(id)]
    )
    return payload.idea
  }

  func fetchStageDetail(stageExecutionID: String) async throws -> P031StageDetailReadModel {
    try await readClient.execute(
      P031StageDetailReadModel.self,
      operationName: "P031StageDetail",
      document: documents.stageDetail,
      variables: ["stageExecutionId": .string(stageExecutionID)]
    )
  }

  func fetchStages(runID: String) async throws -> [P031StageReadModel] {
    let payload = try await readClient.execute(
      StagesPayload.self,
      operationName: "P031Stages",
      document: documents.stages,
      variables: ["runId": .string(runID)]
    )
    return payload.stages
  }

  func fetchApprovalInbox() async throws -> [P031ApprovalReadModel] {
    let payload = try await readClient.execute(
      ApprovalInboxPayload.self,
      operationName: "P031ApprovalInbox",
      document: documents.approvalInbox
    )
    return payload.approvalInbox
  }

  func fetchArtifacts(runID: String) async throws -> [P031ArtifactReadModel] {
    let payload = try await readClient.execute(
      ArtifactsPayload.self,
      operationName: "P031Artifacts",
      document: documents.artifacts,
      variables: ["runId": .string(runID)]
    )
    return payload.artifacts
  }

  func fetchArtifactPayload(artifactID: String) async throws -> P031ArtifactReadModel {
    ForgeLogger.ui.info(
      "P031ArtifactPayload request artifactID=\(artifactID)"
    )
    do {
      let payload = try await readClient.execute(
        ArtifactPayload.self,
        operationName: "P031ArtifactPayload",
        document: documents.artifactPayload,
        variables: ["artifactId": .string(artifactID)]
      )
      guard let artifact = payload.artifact else {
        ForgeLogger.ui.error(
          "P031ArtifactPayload missing artifact artifactID=\(artifactID)"
        )
        throw P031GraphQLReadBoundaryError.missingData("P031ArtifactPayload")
      }
      ForgeLogger.ui.info(
        "P031ArtifactPayload response artifactID=\(artifact.id) payloadState=\(artifact.payloadAvailabilityState.rawValue) hasPayload=\((artifact.payloadText?.isEmpty == false)) reason=\(artifact.payloadUnavailableReasonCode?.rawValue ?? "nil")"
      )
      return artifact
    } catch {
      ForgeLogger.ui.error(
        "P031ArtifactPayload failed artifactID=\(artifactID) error=\(String(describing: error))"
      )
      throw error
    }
  }

  func fetchTimelineRawDetail(handle: String) async throws -> P031TimelineRawDetailReadModel {
    let payload = try await readClient.execute(
      TimelineRawDetailPayload.self,
      operationName: "P031TimelineRawDetail",
      document: documents.timelineRawDetail,
      variables: ["handle": .string(handle)]
    )
    return payload.timelineRawDetail
  }

  func fetchReportMetadata(runID: String) async throws -> [P031ReportMetadataReadModel] {
    let payload = try await readClient.execute(
      ReportMetadataPayload.self,
      operationName: "P031ReportMetadata",
      document: documents.reportMetadata,
      variables: ["runId": .string(runID)]
    )
    return payload.artifacts.filter(\.isReportMetadata)
  }

  func fetchDaemonStatus() async throws -> P031DaemonStatusReadModel {
    let payload = try await readClient.execute(
      DaemonStatusPayload.self,
      operationName: "P031DaemonStatus",
      document: documents.daemonStatus
    )
    return try Self.decodeDaemonStatusJSON(payload.daemonStatus.json)
  }

  nonisolated func subscribeToRunStatus(runID: String) throws -> AsyncThrowingStream<
    P031RunStatusChangedReadModel, Error
  > {
    try subscriptionClient.subscribe(
      RunStatusChangedPayload.self,
      operationName: "P031RunStatusChanged",
      document: documents.runStatusChanged,
      variables: ["runId": .string(runID)]
    )
    .map { payload in payload.runStatusChanged }
  }

  nonisolated func subscribeToRuntimeTimeline(runID: String) throws -> AsyncThrowingStream<
    P031RuntimeTimelineEventReadModel, Error
  > {
    try subscriptionClient.subscribe(
      RuntimeStatusChangedPayload.self,
      operationName: "P031RuntimeStatusChanged",
      document: documents.runtimeStatusChanged,
      variables: ["runId": .string(runID)]
    )
    .map { payload in payload.runtimeStatusChanged }
  }

  nonisolated func subscribeToDaemonStatus() throws -> AsyncThrowingStream<P031DaemonStatusReadModel, Error> {
    try subscriptionClient.subscribe(
      DaemonStatusChangedPayload.self,
      operationName: "P031DaemonStatusChanged",
      document: documents.daemonStatusChanged
    )
    .map { payload in try Self.decodeDaemonStatusJSON(payload.daemonStatusChanged.json) }
  }

  nonisolated private static func decodeDaemonStatusJSON(_ json: String) throws
    -> P031DaemonStatusReadModel
  {
    guard let data = json.data(using: .utf8) else {
      throw P031GraphQLReadBoundaryError.decodingFailed("daemonStatus.json was not UTF-8")
    }
    do {
      guard let object = try JSONSerialization.jsonObject(with: data) as? [String: Any],
        let rawState = object["state"] as? String,
        let state = P031DaemonLifecycleState(rawValue: rawState)
      else {
        throw P031GraphQLReadBoundaryError.decodingFailed(
          "daemonStatus.json did not include a valid state")
      }
      return P031DaemonStatusReadModel(
        state: state,
        schemaVersion: object["schema_version"] as? Int,
        binarySchemaVersion: object["binary_schema_version"] as? Int,
        buildSHA: object["build_sha"] as? String,
        startedAt: object["started_at"] as? String,
        lastStateChangeAt: object["last_state_change_at"] as? String,
        restartCountSinceBoot: object["restart_count_since_boot"] as? Int,
        pid: object["pid"] as? Int,
        rawJSON: json
      )
    } catch {
      throw P031GraphQLReadBoundaryError.decodingFailed(error.localizedDescription)
    }
  }

  private func enrichRunsWithIdeaTitles(_ runs: [P031RunRowReadModel]) async
    -> [P031RunRowReadModel]
  {
    let ideaIDs = Array(Set(runs.compactMap(\.ideaID)))
    var ideaTitles: [String: String] = [:]
    for ideaID in ideaIDs {
      ideaTitles[ideaID] = await fetchIdeaTitle(ideaID: ideaID)
    }
    return runs.map { run in
      guard let ideaID = run.ideaID else {
        return run
      }
      return run.withIdeaTitle(ideaTitles[ideaID])
    }
  }

  private func fetchIdeaTitle(ideaID: String) async -> String? {
    (try? await fetchIdea(id: ideaID))?.title
  }

  private struct RunsPayload: Decodable {
    let runs: [P031RunRowReadModel]
  }

  private struct IdeasPayload: Decodable {
    let ideas: [P031IdeaReadModel]
  }

  private struct IdeaPayload: Decodable {
    let idea: P031IdeaReadModel?
  }

  private struct StagesPayload: Decodable {
    let stages: [P031StageReadModel]
  }

  private struct ApprovalInboxPayload: Decodable {
    let approvalInbox: [P031ApprovalReadModel]
  }

  private struct ArtifactsPayload: Decodable {
    let artifacts: [P031ArtifactReadModel]
  }

  private struct ArtifactPayload: Decodable {
    let artifact: P031ArtifactReadModel?
  }

  private struct TimelineRawDetailPayload: Decodable {
    let timelineRawDetail: P031TimelineRawDetailReadModel
  }

  private struct ReportMetadataPayload: Decodable {
    let artifacts: [P031ReportMetadataReadModel]
  }

  private struct DaemonStatusPayload: Decodable {
    let daemonStatus: DaemonStatusJSONPayload
  }

  nonisolated private struct RunStatusChangedPayload: Decodable {
    let runStatusChanged: P031RunStatusChangedReadModel
  }

  nonisolated private struct RuntimeStatusChangedPayload: Decodable {
    let runtimeStatusChanged: P031RuntimeTimelineEventReadModel
  }

  nonisolated private struct DaemonStatusChangedPayload: Decodable {
    let daemonStatusChanged: DaemonStatusJSONPayload
  }

  nonisolated private struct DaemonStatusJSONPayload: Decodable {
    let json: String
  }
}

struct P031InMemoryWorkflowReadStore: P031WorkflowReadStore {
  let runs: [P031RunRowReadModel]
  let ideasByID: [String: P031IdeaReadModel]
  let runDetailsByRunID: [String: P031RunDetailReadModel]
  let stageDetailsByStageExecutionID: [String: P031StageDetailReadModel]
  let stagesByRunID: [String: [P031StageReadModel]]
  let approvalInbox: [P031ApprovalReadModel]
  let artifactsByRunID: [String: [P031ArtifactReadModel]]
  let timelineRawDetailsByHandle: [String: P031TimelineRawDetailReadModel]
  let reportsByRunID: [String: [P031ReportMetadataReadModel]]
  let daemonStatus: P031DaemonStatusReadModel?
  let runStatusEvents: [String: [P031RunStatusChangedReadModel]]
  let runtimeTimelineEvents: [String: [P031RuntimeTimelineEventReadModel]]
  let daemonStatusEvents: [P031DaemonStatusReadModel]

  init(
    runs: [P031RunRowReadModel] = [],
    ideasByID: [String: P031IdeaReadModel] = [:],
    runDetailsByRunID: [String: P031RunDetailReadModel] = [:],
    stageDetailsByStageExecutionID: [String: P031StageDetailReadModel] = [:],
    stagesByRunID: [String: [P031StageReadModel]] = [:],
    approvalInbox: [P031ApprovalReadModel] = [],
    artifactsByRunID: [String: [P031ArtifactReadModel]] = [:],
    timelineRawDetailsByHandle: [String: P031TimelineRawDetailReadModel] = [:],
    reportsByRunID: [String: [P031ReportMetadataReadModel]] = [:],
    daemonStatus: P031DaemonStatusReadModel? = nil,
    runStatusEvents: [String: [P031RunStatusChangedReadModel]] = [:],
    runtimeTimelineEvents: [String: [P031RuntimeTimelineEventReadModel]] = [:],
    daemonStatusEvents: [P031DaemonStatusReadModel] = []
  ) {
    self.runs = runs
    self.ideasByID = ideasByID
    self.runDetailsByRunID = runDetailsByRunID
    self.stageDetailsByStageExecutionID = stageDetailsByStageExecutionID
    self.stagesByRunID = stagesByRunID
    self.approvalInbox = approvalInbox
    self.artifactsByRunID = artifactsByRunID
    self.timelineRawDetailsByHandle = timelineRawDetailsByHandle
    self.reportsByRunID = reportsByRunID
    self.daemonStatus = daemonStatus
    self.runStatusEvents = runStatusEvents
    self.runtimeTimelineEvents = runtimeTimelineEvents
    self.daemonStatusEvents = daemonStatusEvents
  }

  func fetchRuns() async throws -> [P031RunRowReadModel] {
    runs
  }

  func fetchIdeas(includeArchived: Bool = false) async throws -> [P031IdeaReadModel] {
    Array(ideasByID.values)
      .filter { includeArchived || $0.archivedAt == nil }
      .sorted { lhs, rhs in
        switch (lhs.createdAt, rhs.createdAt) {
        case let (lhs?, rhs?):
          return lhs > rhs
        case (_?, nil):
          return true
        case (nil, _?):
          return false
        case (nil, nil):
          return lhs.title.localizedStandardCompare(rhs.title) == .orderedAscending
        }
      }
  }

  func fetchRunDetail(runID: String) async throws -> P031RunDetailReadModel {
    runDetailsByRunID[runID] ?? P031RunDetailReadModel(run: nil, stages: [], artifacts: [])
  }

  func fetchIdea(id: String) async throws -> P031IdeaReadModel? {
    ideasByID[id]
  }

  func fetchStageDetail(stageExecutionID: String) async throws -> P031StageDetailReadModel {
    stageDetailsByStageExecutionID[stageExecutionID] ?? P031StageDetailReadModel(stage: nil)
  }

  func fetchStages(runID: String) async throws -> [P031StageReadModel] {
    stagesByRunID[runID, default: []]
  }

  func fetchApprovalInbox() async throws -> [P031ApprovalReadModel] {
    approvalInbox
  }

  func fetchArtifacts(runID: String) async throws -> [P031ArtifactReadModel] {
    artifactsByRunID[runID, default: []]
  }

  func fetchArtifactPayload(artifactID: String) async throws -> P031ArtifactReadModel {
    if let artifact = artifactsByRunID.values.lazy.flatMap({ $0 }).first(where: {
      $0.id == artifactID
    }) {
      return artifact
    }
    if let artifact = runDetailsByRunID.values.lazy.flatMap({ $0.artifacts }).first(where: {
      $0.id == artifactID
    }) {
      return artifact
    }
    throw P031GraphQLReadBoundaryError.missingData("P031ArtifactPayload")
  }

  func fetchTimelineRawDetail(handle: String) async throws -> P031TimelineRawDetailReadModel {
    timelineRawDetailsByHandle[handle] ?? P031TimelineRawDetailReadModel(
      status: .missing,
      rawDetail: nil,
      rawDetailBytes: nil,
      rawDetailDigest: nil,
      errorReason: .handleNotFound
    )
  }

  func fetchReportMetadata(runID: String) async throws -> [P031ReportMetadataReadModel] {
    reportsByRunID[runID, default: []]
  }

  func fetchDaemonStatus() async throws -> P031DaemonStatusReadModel {
    guard let daemonStatus else {
      throw P031GraphQLReadBoundaryError.missingData("P031DaemonStatus")
    }
    return daemonStatus
  }

  nonisolated func subscribeToRunStatus(runID: String) throws -> AsyncThrowingStream<
    P031RunStatusChangedReadModel, Error
  > {
    let events = runStatusEvents[runID, default: []]
    return AsyncThrowingStream { continuation in
      for event in events {
        continuation.yield(event)
      }
      continuation.finish()
    }
  }

  nonisolated func subscribeToRuntimeTimeline(runID: String) throws -> AsyncThrowingStream<
    P031RuntimeTimelineEventReadModel, Error
  > {
    let events = runtimeTimelineEvents[runID, default: []]
    return AsyncThrowingStream { continuation in
      for event in events {
        continuation.yield(event)
      }
      continuation.finish()
    }
  }

  nonisolated func subscribeToDaemonStatus() throws -> AsyncThrowingStream<P031DaemonStatusReadModel, Error> {
    let events = daemonStatusEvents
    return AsyncThrowingStream { continuation in
      for event in events {
        continuation.yield(event)
      }
      continuation.finish()
    }
  }
}

extension AsyncThrowingStream {
  nonisolated fileprivate func map<Mapped>(
    _ transform: @escaping @Sendable (Element) throws -> Mapped
  ) -> AsyncThrowingStream<Mapped, Error> {
    AsyncThrowingStream<Mapped, Error> { continuation in
      let task = Task {
        do {
          for try await element in self {
            continuation.yield(try transform(element))
          }
          continuation.finish()
        } catch {
          continuation.finish(throwing: error)
        }
      }
      continuation.onTermination = { _ in task.cancel() }
    }
  }

  nonisolated fileprivate func compactMap<Mapped>(
    _ transform: @escaping @Sendable (Element) throws -> Mapped?
  ) -> AsyncThrowingStream<Mapped, Error> {
    AsyncThrowingStream<Mapped, Error> { continuation in
      let task = Task {
        do {
          for try await element in self {
            if let mapped = try transform(element) {
              continuation.yield(mapped)
            }
          }
          continuation.finish()
        } catch {
          continuation.finish(throwing: error)
        }
      }
      continuation.onTermination = { _ in task.cancel() }
    }
  }
}

struct P031DiagnosticCopyItem: Equatable, Sendable {
  let label: String
  let value: String
}

struct P031ApprovalDiagnosticPresentation: Equatable, Sendable {
  let title: String
  let body: String
  let actionLabel: String?
  let followUpID: String?
  let copyItems: [P031DiagnosticCopyItem]
}

enum P031ExternalWriteWorkflow: String, Equatable, Sendable {
  case cli
  case mcpTerminal
  case automation
  case nonP031UI
}

enum P031ExternalWritePathGuideState: Equatable, Sendable {
  case unavailable
  case documented(P031ExternalWriteWorkflow)

  nonisolated var cliWorkflowDocumented: Bool {
    switch self {
    case .documented(.cli):
      return true
    case .documented, .unavailable:
      return false
    }
  }

  nonisolated var guideAvailable: Bool {
    switch self {
    case .unavailable:
      return false
    case .documented:
      return true
    }
  }
}

struct P031OperatorWritePathGuideSummaryPresentation: Equatable, Sendable {
  let rows: [P031OperatorWritePathGuideRowPresentation]
  let availableExternalWorkflowCount: Int
  let unavailableCount: Int
  let pendingOrInvalidCount: Int
  let emptyStateTitle: String?
}

struct P031OperatorWritePathGuideResolution: Equatable, Sendable {
  let guide: P031OperatorWritePathGuide?
  let approvalResolutionState: P031ExternalWritePathGuideState
  let summaryPresentation: P031OperatorWritePathGuideSummaryPresentation
  let errorDescription: String?

  nonisolated static var unavailable: P031OperatorWritePathGuideResolution {
    P031OperatorWritePathGuideResolution(
      guide: nil,
      approvalResolutionState: .unavailable,
      summaryPresentation: P031OperatorWritePathGuidePresenter.unavailablePresentation(),
      errorDescription: nil
    )
  }
}

struct P031OperatorWritePathGuideRowPresentation: Equatable, Sendable {
  let removedControlID: String
  let title: String
  let statusLabel: String
  let workflowLabel: String
  let toolLabel: String?
  let requiredIdentifierLabels: [String]
  let followUpID: String?
  let canExecuteFromUI: Bool
  let accessibilityLabel: String
}

struct P031OperatorWritePathGuide: Decodable, Equatable, Sendable {
  nonisolated static let requiredSchemaVersion = "p031-operator-write-path-guide-v1"
  nonisolated static let requiredRemovedControlIdentifierRequirements:
    [(
      controlID: String, requiredIdentifiers: Set<String>
    )] = [
      ("ideas.create", []),
      ("runs.start", ["idea_id"]),
      ("runs.cancel", ["run_id"]),
      ("stages.retry", ["run_id", "stage_id"]),
      ("approvals.resolve", ["approval_id", "run_id", "stage_id"]),
      ("steward.run_analysis", ["run_id"]),
      ("session.reset", ["run_id"]),
      ("session.resume", ["run_id"]),
      ("runs.clone", ["run_id"]),
      ("runs.compare", ["run_id"]),
      ("experiments.launch", ["run_id"]),
      ("runtime.health", ["run_id"]),
      ("agents.reset", ["run_id"]),
    ]

  let schemaVersion: String?
  let rows: [P031OperatorWritePathGuideRow]

  enum CodingKeys: String, CodingKey {
    case schemaVersion = "schema_version"
    case rows
  }

  nonisolated func approvalResolutionState() -> P031ExternalWritePathGuideState {
    state(
      forRemovedControlID: "approvals.resolve",
      requiringIdentifiers: ["approval_id", "run_id", "stage_id"]
    )
  }

  nonisolated func state(
    forRemovedControlID removedControlID: String,
    requiringIdentifiers requiredIdentifiers: Set<String>
  ) -> P031ExternalWritePathGuideState {
    guard
      schemaVersion == Self.requiredSchemaVersion,
      hasCompleteRemovedControlCoverage,
      let row = rows.first(where: {
        P031OperatorWritePathGuideRow.normalizedIdentifier($0.removedControlID)
          == P031OperatorWritePathGuideRow.normalizedIdentifier(removedControlID)
      }),
      row.isGateCompatible,
      row.supportsExternalWorkflow(requiringIdentifiers: requiredIdentifiers)
    else {
      return .unavailable
    }

    switch row.externalWorkflowKind {
    case .cli:
      return .documented(.cli)
    case .mcpTerminal:
      return .documented(.mcpTerminal)
    case .automation:
      return .documented(.automation)
    case .nonP031UI:
      return .documented(.nonP031UI)
    case .temporarilyUnavailable, .unknown(_):
      return .unavailable
    }
  }

  nonisolated var isCurrentSchema: Bool {
    schemaVersion == Self.requiredSchemaVersion
  }

  nonisolated var hasCompleteRemovedControlCoverage: Bool {
    missingRemovedControlCoverage.isEmpty && unknownRemovedControlIDs.isEmpty
  }

  nonisolated var missingRemovedControlCoverage: [String] {
    let rowsByControlID = Dictionary(grouping: rows) {
      P031OperatorWritePathGuideRow.normalizedIdentifier($0.removedControlID)
    }
    return Self.requiredRemovedControlIdentifierRequirements.compactMap { requirement in
      let normalizedControlID = P031OperatorWritePathGuideRow.normalizedIdentifier(
        requirement.controlID)
      guard let matchingRows = rowsByControlID[normalizedControlID], matchingRows.count == 1 else {
        return requirement.controlID
      }
      let identifiers = Set(matchingRows[0].normalizedRequiredIdentifiers)
      guard !identifiers.isEmpty, requirement.requiredIdentifiers.isSubset(of: identifiers) else {
        return requirement.controlID
      }
      return nil
    }
  }

  nonisolated var unknownRemovedControlIDs: [String] {
    let requiredControlIDs = Set(
      Self.requiredRemovedControlIdentifierRequirements.map {
        P031OperatorWritePathGuideRow.normalizedIdentifier($0.controlID)
      })
    let presentControlIDs = Set(
      rows.map { P031OperatorWritePathGuideRow.normalizedIdentifier($0.removedControlID) })
    return Array(presentControlIDs.subtracting(requiredControlIDs)).sorted()
  }
}

enum P031OperatorWritePathGuideResolver {
  static func resolve(from data: Data?) -> P031OperatorWritePathGuideResolution {
    guard let data else {
      return .unavailable
    }
    do {
      let guide = try JSONDecoder().decode(P031OperatorWritePathGuide.self, from: data)
      guard guide.isCurrentSchema else {
        return P031OperatorWritePathGuideResolution(
          guide: guide,
          approvalResolutionState: .unavailable,
          summaryPresentation: P031OperatorWritePathGuidePresenter.unavailablePresentation(),
          errorDescription: "External write-path guide schema is unavailable"
        )
      }
      guard guide.hasCompleteRemovedControlCoverage else {
        return P031OperatorWritePathGuideResolution(
          guide: guide,
          approvalResolutionState: .unavailable,
          summaryPresentation: P031OperatorWritePathGuidePresenter.unavailablePresentation(),
          errorDescription: "External write-path guide coverage is incomplete"
        )
      }
      guard guide.rows.allSatisfy(\.isGateCompatible) else {
        return P031OperatorWritePathGuideResolution(
          guide: guide,
          approvalResolutionState: .unavailable,
          summaryPresentation: P031OperatorWritePathGuidePresenter.unavailablePresentation(),
          errorDescription: "External write-path guide row contract is incomplete"
        )
      }
      return P031OperatorWritePathGuideResolution(
        guide: guide,
        approvalResolutionState: guide.approvalResolutionState(),
        summaryPresentation: P031OperatorWritePathGuidePresenter.presentation(for: guide),
        errorDescription: nil
      )
    } catch {
      return P031OperatorWritePathGuideResolution(
        guide: nil,
        approvalResolutionState: .unavailable,
        summaryPresentation: P031OperatorWritePathGuidePresenter.unavailablePresentation(),
        errorDescription: P031ReadErrorPresenter.description(for: error)
      )
    }
  }
}

struct P031OperatorWritePathGuideRow: Decodable, Equatable, Sendable {
  let removedControlID: String
  let removedControlLabel: String
  let externalWorkflowKind: P031OperatorWritePathExternalWorkflowKind
  let externalWorkflowNameOrTool: String?
  let requiredIdentifiers: [String]
  let minimumParameterShape: String?
  let unavailableReason: String?
  let expectedSuccessOutput: String?
  let followUpID: String?
  let operatorNotes: String?
  let validationStatus: P031OperatorWritePathValidationStatus
  private let presentContractKeys: Set<String>

  enum CodingKeys: String, CodingKey {
    case removedControlID = "removed_control_id"
    case removedControlLabel = "removed_control_label"
    case externalWorkflowKind = "external_workflow_kind"
    case externalWorkflowNameOrTool = "external_workflow_name_or_tool"
    case requiredIdentifiers = "required_identifiers"
    case minimumParameterShape = "minimum_parameter_shape"
    case unavailableReason = "unavailable_reason"
    case expectedSuccessOutput = "expected_success_output"
    case followUpID = "follow_up_id"
    case operatorNotes = "operator_notes"
    case validationStatus = "validation_status"
  }

  nonisolated static let requiredContractKeys: Set<String> = [
    CodingKeys.removedControlID.rawValue,
    CodingKeys.removedControlLabel.rawValue,
    CodingKeys.externalWorkflowKind.rawValue,
    CodingKeys.externalWorkflowNameOrTool.rawValue,
    CodingKeys.requiredIdentifiers.rawValue,
    CodingKeys.minimumParameterShape.rawValue,
    CodingKeys.unavailableReason.rawValue,
    CodingKeys.expectedSuccessOutput.rawValue,
    CodingKeys.followUpID.rawValue,
    CodingKeys.operatorNotes.rawValue,
    CodingKeys.validationStatus.rawValue,
  ]

  init(from decoder: Decoder) throws {
    let container = try decoder.container(keyedBy: CodingKeys.self)
    self.presentContractKeys = Set(container.allKeys.map(\.rawValue))
    self.removedControlID = try container.decode(String.self, forKey: .removedControlID)
    self.removedControlLabel = try container.decode(String.self, forKey: .removedControlLabel)
    self.externalWorkflowKind = try container.decode(
      P031OperatorWritePathExternalWorkflowKind.self,
      forKey: .externalWorkflowKind
    )
    self.externalWorkflowNameOrTool = try container.decodeIfPresent(
      String.self,
      forKey: .externalWorkflowNameOrTool
    )
    self.requiredIdentifiers = try container.decode([String].self, forKey: .requiredIdentifiers)
    self.minimumParameterShape = try container.decodeIfPresent(
      String.self,
      forKey: .minimumParameterShape
    )
    self.unavailableReason = try container.decodeIfPresent(String.self, forKey: .unavailableReason)
    self.expectedSuccessOutput = try container.decodeIfPresent(
      String.self,
      forKey: .expectedSuccessOutput
    )
    self.followUpID = try container.decodeIfPresent(String.self, forKey: .followUpID)
    self.operatorNotes = try container.decodeIfPresent(String.self, forKey: .operatorNotes)
    self.validationStatus = try container.decode(
      P031OperatorWritePathValidationStatus.self,
      forKey: .validationStatus
    )
  }

  nonisolated static func normalizedIdentifier(_ value: String) -> String {
    value.trimmingCharacters(in: .whitespacesAndNewlines)
      .lowercased()
      .replacingOccurrences(of: "-", with: "_")
      .replacingOccurrences(of: " ", with: "_")
      .replacingOccurrences(of: ".", with: "_")
  }

  nonisolated var isGateCompatible: Bool {
    guard missingContractKeys.isEmpty,
      trimmedRemovedControlID != nil,
      trimmedRemovedControlLabel != nil,
      externalWorkflowKind.isKnown,
      validationStatus.isKnown,
      !requiredIdentifiersFromGuide.isEmpty,
      requiredIdentifiers.allSatisfy({ Self.nonEmptyTrimmed($0) != nil }),
      trimmedMinimumParameterShape != nil || trimmedUnavailableReason != nil,
      trimmedExpectedSuccessOutput != nil || trimmedFollowUpID != nil
    else {
      return false
    }

    switch externalWorkflowKind {
    case .temporarilyUnavailable:
      return trimmedUnavailableReason != nil && trimmedFollowUpID != nil
    case .cli, .mcpTerminal, .automation, .nonP031UI:
      return trimmedExternalWorkflowNameOrTool != nil && trimmedExpectedSuccessOutput != nil
    case .unknown(_):
      return false
    }
  }

  nonisolated var missingContractKeys: [String] {
    Self.requiredContractKeys.subtracting(presentContractKeys).sorted()
  }

  nonisolated func supportsExternalWorkflow(requiringIdentifiers requiredIdentifiers: Set<String>)
    -> Bool
  {
    guard validationStatus.isValidated,
      trimmedExternalWorkflowNameOrTool != nil,
      trimmedExpectedSuccessOutput != nil,
      !requiredIdentifiersFromGuide.isEmpty,
      requiredIdentifiers.isSubset(of: Set(requiredIdentifiersFromGuide))
    else {
      return false
    }

    switch externalWorkflowKind {
    case .cli, .mcpTerminal, .automation, .nonP031UI:
      return true
    case .temporarilyUnavailable, .unknown(_):
      return false
    }
  }

  nonisolated var trimmedExternalWorkflowNameOrTool: String? {
    Self.nonEmptyTrimmed(externalWorkflowNameOrTool)
  }

  nonisolated var trimmedRemovedControlID: String? {
    Self.nonEmptyTrimmed(removedControlID)
  }

  nonisolated var trimmedRemovedControlLabel: String? {
    Self.nonEmptyTrimmed(removedControlLabel)
  }

  nonisolated var trimmedMinimumParameterShape: String? {
    Self.nonEmptyTrimmed(minimumParameterShape)
  }

  nonisolated var trimmedUnavailableReason: String? {
    Self.nonEmptyTrimmed(unavailableReason)
  }

  nonisolated var trimmedExpectedSuccessOutput: String? {
    Self.nonEmptyTrimmed(expectedSuccessOutput)
  }

  nonisolated var trimmedFollowUpID: String? {
    Self.nonEmptyTrimmed(followUpID)
  }

  nonisolated var normalizedRequiredIdentifiers: [String] {
    requiredIdentifiersFromGuide
  }

  nonisolated private var requiredIdentifiersFromGuide: [String] {
    requiredIdentifiers.map(Self.normalizedIdentifier)
  }

  nonisolated private static func nonEmptyTrimmed(_ value: String?) -> String? {
    guard let trimmed = value?.trimmingCharacters(in: .whitespacesAndNewlines),
      !trimmed.isEmpty
    else {
      return nil
    }
    return trimmed
  }
}

enum P031OperatorWritePathGuidePresenter {
  nonisolated static func unavailablePresentation()
    -> P031OperatorWritePathGuideSummaryPresentation
  {
    P031OperatorWritePathGuideSummaryPresentation(
      rows: [],
      availableExternalWorkflowCount: 0,
      unavailableCount: 0,
      pendingOrInvalidCount: 0,
      emptyStateTitle: "External write-path guide unavailable"
    )
  }

  nonisolated static func presentation(for guide: P031OperatorWritePathGuide)
    -> P031OperatorWritePathGuideSummaryPresentation
  {
    guard guide.isCurrentSchema else {
      return unavailablePresentation()
    }

    let rows = guide.rows.map(rowPresentation)
    return P031OperatorWritePathGuideSummaryPresentation(
      rows: rows,
      availableExternalWorkflowCount: rows.filter {
        $0.statusLabel == "External workflow documented"
      }
      .count,
      unavailableCount: rows.filter { $0.statusLabel == "Temporarily unavailable" }.count,
      pendingOrInvalidCount: rows.filter {
        $0.statusLabel != "External workflow documented"
          && $0.statusLabel != "Temporarily unavailable"
      }
      .count,
      emptyStateTitle: rows.isEmpty ? "No removed controls documented" : nil
    )
  }

  nonisolated private static func rowPresentation(for row: P031OperatorWritePathGuideRow)
    -> P031OperatorWritePathGuideRowPresentation
  {
    let statusLabel = statusLabel(for: row)
    let title = row.removedControlLabel.trimmingCharacters(in: .whitespacesAndNewlines)
    let displayTitle = title.isEmpty ? row.removedControlID : title
    let workflowLabel = row.externalWorkflowKind.presentationLabel
    let requiredIdentifierLabels = row.requiredIdentifiers.compactMap { identifier -> String? in
      let trimmed = identifier.trimmingCharacters(in: .whitespacesAndNewlines)
      return trimmed.isEmpty ? nil : trimmed
    }
    let accessibilityParts = [
      displayTitle,
      statusLabel,
      workflowLabel,
      row.trimmedExternalWorkflowNameOrTool,
    ].compactMap { $0 }

    return P031OperatorWritePathGuideRowPresentation(
      removedControlID: row.removedControlID,
      title: displayTitle,
      statusLabel: statusLabel,
      workflowLabel: workflowLabel,
      toolLabel: row.trimmedExternalWorkflowNameOrTool,
      requiredIdentifierLabels: requiredIdentifierLabels,
      followUpID: row.trimmedFollowUpID,
      canExecuteFromUI: false,
      accessibilityLabel: accessibilityParts.joined(separator: ", ")
    )
  }

  nonisolated private static func statusLabel(for row: P031OperatorWritePathGuideRow) -> String {
    switch row.externalWorkflowKind {
    case .temporarilyUnavailable:
      return "Temporarily unavailable"
    case .cli, .mcpTerminal, .automation, .nonP031UI, .unknown(_):
      break
    }
    if row.supportsExternalWorkflow(requiringIdentifiers: Set(row.normalizedRequiredIdentifiers)) {
      return "External workflow documented"
    }
    return row.validationStatus.incompleteGuideStatusLabel
  }
}

enum P031OperatorWritePathExternalWorkflowKind: Equatable, Sendable {
  case cli
  case mcpTerminal
  case automation
  case nonP031UI
  case temporarilyUnavailable
  case unknown(String)

  nonisolated var presentationLabel: String {
    switch self {
    case .cli:
      return "CLI"
    case .mcpTerminal:
      return "MCP terminal"
    case .automation:
      return "Automation"
    case .nonP031UI:
      return "Non-P031 UI"
    case .temporarilyUnavailable:
      return "Temporarily unavailable"
    case .unknown(let raw):
      let trimmed = raw.trimmingCharacters(in: .whitespacesAndNewlines)
      return trimmed.isEmpty ? "Unknown workflow" : trimmed
    }
  }

  nonisolated var isKnown: Bool {
    switch self {
    case .cli, .mcpTerminal, .automation, .nonP031UI, .temporarilyUnavailable:
      return true
    case .unknown(_):
      return false
    }
  }
}

extension P031OperatorWritePathExternalWorkflowKind: Decodable {
  init(from decoder: Decoder) throws {
    let raw = try decoder.singleValueContainer().decode(String.self)
    switch P031OperatorWritePathGuideRow.normalizedIdentifier(raw) {
    case "cli":
      self = .cli
    case "mcp_terminal":
      self = .mcpTerminal
    case "automation":
      self = .automation
    case "non_p031_ui":
      self = .nonP031UI
    case "temporarily_unavailable":
      self = .temporarilyUnavailable
    default:
      self = .unknown(raw)
    }
  }
}

enum P031OperatorWritePathValidationStatus: Equatable, Sendable {
  case validated
  case pending
  case failed
  case unknown(String)

  nonisolated var isValidated: Bool {
    switch self {
    case .validated:
      return true
    case .pending, .failed, .unknown:
      return false
    }
  }

  nonisolated var isKnown: Bool {
    switch self {
    case .validated, .pending, .failed:
      return true
    case .unknown:
      return false
    }
  }

  nonisolated var incompleteGuideStatusLabel: String {
    switch self {
    case .validated:
      return "Guide row incomplete"
    case .pending:
      return "Pending validation"
    case .failed:
      return "Validation failed"
    case .unknown:
      return "Unknown validation"
    }
  }
}

extension P031OperatorWritePathValidationStatus: Decodable {
  init(from decoder: Decoder) throws {
    let raw = try decoder.singleValueContainer().decode(String.self)
    switch P031OperatorWritePathGuideRow.normalizedIdentifier(raw) {
    case "validated":
      self = .validated
    case "pending", "not_validated", "unvalidated":
      self = .pending
    case "failed", "invalid":
      self = .failed
    default:
      self = .unknown(raw)
    }
  }
}

enum DisabledReasonPresenter {
  nonisolated static func title(for reason: P031DisabledReasonCode?) -> String {
    switch reason {
    case nil:
      return "Ready for approval"
    case .writePathNotAvailable:
      return "Write path unavailable"
    case .managedOutsideUI:
      return "Managed outside UI"
    case .ambiguousApprovalIdentity:
      return "Ambiguous approval identity"
    case .staleRead:
      return "Stale read"
    case .projectionLag:
      return "Projection lag"
    case .unauthorized:
      return "Unauthorized"
    case .unsupportedAction:
      return "Unsupported action"
    case .redacted:
      return "Redacted"
    case .conflict:
      return "Conflict"
    case .duplicate:
      return "Duplicate"
    case .alreadyResolved:
      return "Already resolved"
    case .approvalNotActionable:
      return "Approval Not Actionable"
    case .observerScope:
      return "Read-Only Access"
    case .nonApprovalMutation:
      return "GraphQL Action Blocked"
    case .capabilityOutOfScope:
      return "Action Not Available"
    }
  }
}

enum ApprovalDiagnosticPresenter {
  nonisolated static func presentation(
    for approval: P031ApprovalReadModel,
    externalWritePathGuideState: P031ExternalWritePathGuideState = .unavailable
  ) -> P031ApprovalDiagnosticPresentation {
    let actionLabel: String?
    if approval.writePathState == .available {
      actionLabel = nil
    } else {
      actionLabel =
        externalWritePathGuideState.cliWorkflowDocumented
          && approval.writePathState == .externalTransportRequired
        ? "Execute via CLI"
        : nil
    }
    let followUpID =
      approval.writePathState == .available || externalWritePathGuideState.guideAvailable
      ? nil
      : "P031-FOLLOWUP-APPROVAL-WRITE-PATH"
    let copyItems = [
      P031DiagnosticCopyItem(label: "approval_id", value: approval.id),
      P031DiagnosticCopyItem(label: "run_id", value: approval.runID),
      P031DiagnosticCopyItem(label: "stage_id", value: approval.stageID),
      approval.diagnosticID.map { P031DiagnosticCopyItem(label: "diagnostic_id", value: $0) },
    ].compactMap { $0 }

    return P031ApprovalDiagnosticPresentation(
      title: approval.writePathState == .available
        ? "Approval ready"
        : (
          externalWritePathGuideState.guideAvailable
            ? "Approval managed outside UI"
            : "Approval write path unavailable"
        ),
      body: approval.disabledReason
        ?? DisabledReasonPresenter.title(for: approval.disabledReasonCode),
      actionLabel: actionLabel,
      followUpID: followUpID,
      copyItems: copyItems
    )
  }
}

struct P031PayloadAvailabilityPresentation: Equatable, Sendable {
  let title: String
  let detail: String
  let symbolName: String
  let canOpenPayload: Bool
  let copyItems: [P031DiagnosticCopyItem]
}

struct P031ReportMetadataRowPresentation: Equatable, Sendable {
  let title: String
  let availabilityLabel: String
  let availabilitySymbolName: String
  let payloadIndicatorSlotWidth: Double
  let canOpenPayload: Bool
  let accessibilityLabel: String
  let copyItems: [P031DiagnosticCopyItem]
}

enum PayloadUnavailableReasonPresenter {
  nonisolated static func presentation(for report: P031ReportMetadataReadModel)
    -> P031PayloadAvailabilityPresentation
  {
    let title: String
    let detail: String
    let symbolName: String
    let canOpenPayload: Bool

    switch report.payloadAvailabilityState {
    case .available:
      title = "Payload"
      detail = "Report payload available"
      symbolName = "doc.text.fill"
      canOpenPayload = true
    case .metadataOnly:
      title = "Metadata"
      detail = detailForReason(report.payloadUnavailableReasonCode)
      symbolName = "doc.text"
      canOpenPayload = false
    case .payloadDeferred:
      title = "Deferred"
      detail = detailForReason(report.payloadUnavailableReasonCode)
      symbolName = "clock.badge.exclamationmark"
      canOpenPayload = false
    case .generating:
      title = "Generating"
      detail = "Report payload is still generating"
      symbolName = "arrow.triangle.2.circlepath"
      canOpenPayload = false
    case .unavailable:
      title = "Unavailable"
      detail = detailForReason(report.payloadUnavailableReasonCode)
      symbolName = "exclamationmark.triangle"
      canOpenPayload = false
    }

    let copyItems = [
      report.diagnosticID.map { P031DiagnosticCopyItem(label: "diagnostic_id", value: $0) }
    ].compactMap { $0 }
    return P031PayloadAvailabilityPresentation(
      title: title,
      detail: detail,
      symbolName: symbolName,
      canOpenPayload: canOpenPayload,
      copyItems: copyItems
    )
  }

  nonisolated private static func detailForReason(_ reason: P031PayloadUnavailableReasonCode?)
    -> String
  {
    switch reason {
    case .payloadDeferredByP031:
      return "Payload rendering is deferred by P031"
    case .generating:
      return "Report payload is still generating"
    case .notIndexed:
      return "Report payload is not indexed"
    case .notAuthorized:
      return "Report payload is not authorized"
    case .notAvailable:
      return "Report payload is not available"
    case .unknown, nil:
      return "Report payload availability is unknown"
    }
  }
}

enum ReportMetadataRowPresenter {
  nonisolated static let payloadIndicatorSlotWidth: Double = 96

  nonisolated static func presentation(for report: P031ReportMetadataReadModel)
    -> P031ReportMetadataRowPresentation
  {
    let availability = PayloadUnavailableReasonPresenter.presentation(for: report)
    let title = report.name?.trimmingCharacters(in: .whitespacesAndNewlines)
    let displayTitle = title?.isEmpty == false ? title! : "Untitled report"
    let symbolName: String

    switch report.payloadAvailabilityState {
    case .available:
      symbolName = "doc.text.fill"
    case .metadataOnly:
      symbolName = "doc.text"
    case .payloadDeferred:
      symbolName = "clock.badge.exclamationmark"
    case .generating:
      symbolName = "arrow.triangle.2.circlepath"
    case .unavailable:
      symbolName = "exclamationmark.triangle"
    }

    return P031ReportMetadataRowPresentation(
      title: displayTitle,
      availabilityLabel: availability.title,
      availabilitySymbolName: symbolName,
      payloadIndicatorSlotWidth: payloadIndicatorSlotWidth,
      canOpenPayload: availability.canOpenPayload,
      accessibilityLabel: "\(displayTitle), \(availability.title). \(availability.detail)",
      copyItems: availability.copyItems
    )
  }
}

enum DiagnosticDetailsPresenter {
  static func operatorDebugDetail(_ detail: String?, operatorAuthorized: Bool) -> String? {
    operatorAuthorized ? detail : nil
  }
}

struct P031FirstRunOrientationPresentation: Equatable, Sendable {
  let title: String
  let body: String
  let externalWritePathLabel: String
  let canDismiss: Bool
}

enum P031FirstRunOrientationPresenter {
  nonisolated static func presentation(
    writePathGuideState: P031ExternalWritePathGuideState
  ) -> P031FirstRunOrientationPresentation {
    P031FirstRunOrientationPresentation(
      title: "GraphQL-only read mode",
      body:
        "Workflow screens show server projections only. Writes are handled outside this UI until an approved write transport is available.",
      externalWritePathLabel: writePathGuideState.guideAvailable
        ? "Open external write-path guide"
        : "External write-path guide unavailable",
      canDismiss: true
    )
  }
}

struct P031RunsHomeRowPresentation: Equatable, Sendable {
  let runID: String
  let title: String
  let workflowLabel: String?
  let statusLabel: String
  let progressLabel: String?
  let pendingApprovalsLabel: String?
  let closeoutReadinessSignalLabel: String?
  let implementationCompletionSignalLabel: String?
  let sideEffectSignalLabel: String?
  let freshnessState: P031FreshnessState
  let accessibilityLabel: String
  // P036: Expose typed fields for workbench classification
  let rawStatus: String
  let failedStages: Int
  let pendingApprovals: Int
  let lane: P036RunLane

  nonisolated init(
    runID: String,
    title: String,
    workflowLabel: String?,
    statusLabel: String,
    progressLabel: String?,
    pendingApprovalsLabel: String?,
    closeoutReadinessSignalLabel: String? = nil,
    implementationCompletionSignalLabel: String? = nil,
    sideEffectSignalLabel: String? = nil,
    freshnessState: P031FreshnessState,
    accessibilityLabel: String,
    rawStatus: String,
    failedStages: Int,
    pendingApprovals: Int
  ) {
    self.runID = runID
    self.title = title
    self.workflowLabel = workflowLabel
    self.statusLabel = statusLabel
    self.progressLabel = progressLabel
    self.pendingApprovalsLabel = pendingApprovalsLabel
    self.closeoutReadinessSignalLabel = closeoutReadinessSignalLabel
    self.implementationCompletionSignalLabel = implementationCompletionSignalLabel
    self.sideEffectSignalLabel = sideEffectSignalLabel
    self.freshnessState = freshnessState
    self.accessibilityLabel = accessibilityLabel
    self.rawStatus = rawStatus
    self.failedStages = failedStages
    self.pendingApprovals = pendingApprovals

    // P036: Canonical lane mapping
    if pendingApprovals > 0 {
      self.lane = .waiting
    } else {
      if let status = RunStatus.from(serverValue: rawStatus) {
        switch status {
        case .failed, .blocked:
          self.lane = .blocked
        case .running, .cancelling, .pending, .ready, .waitingApproval:
          self.lane = .running
        case .completed, .cancelled:
          self.lane = .completed
        }
      } else {
        self.lane = .deferred
      }
    }
  }
}

enum P036RunLane: String, Codable, Sendable, CaseIterable {
  case waiting
  case blocked
  case running
  case completed
  // Server status was not in the known RunStatus vocabulary; renders projection-lag/deferred
  // state instead of guessing terminality from local heuristics.
  case deferred
}

struct P031RunsHomePresentation: Equatable, Sendable {
  let orientation: P031FirstRunOrientationPresentation?
  let rows: [P031RunsHomeRowPresentation]
  let freshness: P031FreshnessSnapshot
  let refreshFeedbackText: String
  let emptyStateTitle: String?
  let errorDescription: String?
}

struct P031RunStatusSubscriptionPresentation: Equatable, Sendable {
  let runID: String
  let statusLabel: String
  let badgeLabels: [String]
  let freshness: P031FreshnessSnapshot
  let accessibilityLabel: String
}

struct P031RuntimeTimelineEventPresentation: Identifiable, Equatable, Sendable {
  let id: String
  let runID: String
  let stageID: String?
  let agentID: String
  let provider: String
  let eventKind: String
  let title: String
  let detail: String
  let surfaceLabel: String
  let sessionGenerationID: String?
  let timestamp: Date
  let rawDetail: String?
  let rawDetailBytes: Int?
  let rawDetailTruncated: Bool
  let rawDetailHandle: String?
  let rawDetailDigest: String?
  let fullRawAvailable: Bool
  let detailDigest: String?
  let detailCharCount: Int?
  let chunkCount: Int?
  let isStreaming: Bool
  let isTerminal: Bool
  let stateLabel: String?

  nonisolated init(
    id: String,
    runID: String,
    stageID: String?,
    agentID: String,
    provider: String,
    eventKind: String,
    title: String,
    detail: String,
    surfaceLabel: String,
    sessionGenerationID: String?,
    timestamp: Date,
    rawDetail: String? = nil,
    rawDetailBytes: Int? = nil,
    rawDetailTruncated: Bool = false,
    rawDetailHandle: String? = nil,
    rawDetailDigest: String? = nil,
    fullRawAvailable: Bool = true,
    detailDigest: String? = nil,
    detailCharCount: Int? = nil,
    chunkCount: Int? = nil,
    isStreaming: Bool = false,
    isTerminal: Bool = false,
    stateLabel: String? = nil
  ) {
    self.id = id
    self.runID = runID
    self.stageID = stageID
    self.agentID = agentID
    self.provider = provider
    self.eventKind = eventKind
    self.title = title
    self.detail = detail
    self.surfaceLabel = surfaceLabel
    self.sessionGenerationID = sessionGenerationID
    self.timestamp = timestamp
    self.rawDetail = rawDetail
    self.rawDetailBytes = rawDetailBytes
    self.rawDetailTruncated = rawDetailTruncated
    self.rawDetailHandle = rawDetailHandle
    self.rawDetailDigest = rawDetailDigest
    self.fullRawAvailable = fullRawAvailable
    self.detailDigest = detailDigest
    self.detailCharCount = detailCharCount
    self.chunkCount = chunkCount
    self.isStreaming = isStreaming
    self.isTerminal = isTerminal
    self.stateLabel = stateLabel
  }
}

struct P031ApprovalInboxRowPresentation: Equatable, Sendable {
  let approvalID: String
  let title: String
  let body: String
  let canApprove: Bool
  let canReject: Bool
  let actionLabel: String?
  let followUpID: String?
  let copyItems: [P031DiagnosticCopyItem]
  let freshnessState: P031FreshnessState
  let accessibilityLabel: String
  // P036: Expose P085 affordance for workbench integration
  let affordance: P085ApprovalAffordanceState

  // M3: gated accessor — views must use this instead of reading `.affordance` directly
  var deferredState: P036DeferredState? { P036DeferredState(from: affordance) }
}

struct P031ApprovalInboxPresentation: Equatable, Sendable {
  let rows: [P031ApprovalInboxRowPresentation]
  let freshness: P031FreshnessSnapshot
  let refreshFeedbackText: String
  let emptyStateTitle: String?
  let errorDescription: String?
}

struct P031StageSummaryPresentation: Equatable, Sendable {
  let stageExecutionID: String
  let title: String
  let statusLabel: String
  let iterationLabel: String?
  let startedLabel: String?
  let completedLabel: String?
  let durationLabel: String?
  let badgeLabels: [String]
  let freshnessState: P031FreshnessState
  let accessibilityLabel: String
}

struct P031ArtifactSummaryPresentation: Equatable, Sendable {
  let artifactID: String
  let title: String
  let detailLabel: String
  let payloadAvailabilityLabel: String
  let payloadAvailabilitySymbolName: String
  let canOpenPayload: Bool
  let diagnosticCopyItems: [P031DiagnosticCopyItem]
  let freshnessState: P031FreshnessState
  let accessibilityLabel: String
  // P036: Expose P085 affordance for workbench integration
  let affordance: P085ArtifactAffordanceState
}

struct P031IdeaContextPresentation: Equatable, Sendable {
  let id: String
  let title: String
  let statusLabel: String?
  let projectKey: String?
  let body: String?
  let createdAt: String?
  let archivedAt: String?
  let accessibilityLabel: String
}

enum P031StageConnectorState: Equatable, Sendable {
  case completed
  case blocked
  case running
  case pending
  case unavailable
}

struct P031StageTransitionPresentation: Equatable, Sendable {
  let stageExecutionID: String
  let stageTitle: String
  let statusText: String
  let attemptText: String?
  let startedLabel: String?
  let completedLabel: String?
  let durationLabel: String?
  let connectorState: P031StageConnectorState
  let evidenceLabels: [String]
  let accessibilityLabel: String
}

enum P031ArtifactRenderMode: Equatable, Sendable {
  case markdown
  case json
  case diff
  case plainText
  case metadataOnly
  case unavailable
}

struct P031ArtifactViewerPresentation: Equatable, Sendable {
  let artifactID: String
  let stageID: String
  let stageExecutionID: String?
  let stageLabel: String?
  let iteration: Int?
  let attemptNumber: Int?
  let agentID: String?
  let contractID: String
  let format: String
  let title: String
  let subtitle: String
  let renderMode: P031ArtifactRenderMode
  let payloadState: P031PayloadAvailabilityState
  let preparedPreview: ArtifactPreparedPreview?
  let unavailableReason: String?
  let freshnessState: P031FreshnessState
  let accessibilityLabel: String
}

struct P031CatalogContextPresentation: Equatable, Sendable {
  let workflowID: String?
  let workflowTitle: String
  let workflowSnapshotHash: String?
  let catalogSnapshotHash: String?
  let statusText: String
  let accessibilityLabel: String
}

enum P077CloseoutReadinessVisualState: Equatable, Sendable {
  case positive
  case warning
  case blocking
  case neutral

  nonisolated static func == (lhs: P077CloseoutReadinessVisualState, rhs: P077CloseoutReadinessVisualState) -> Bool {
    switch (lhs, rhs) {
    case (.positive, .positive), (.warning, .warning), (.blocking, .blocking), (.neutral, .neutral): return true
    default: return false
    }
  }
}

struct P077CloseoutReadinessPresentation: Equatable, Sendable {
  let statusLabel: String
  let compactSignalLabel: String
  let detailText: String
  let primaryUnblockText: String
  let secondaryBlockerRows: [String]
  let modeLabel: String
  let modeExplainerText: String
  let diagnosticRows: [String]
  let recoveryLifecycleText: String
  let recoveryLifecycleAcknowledgementText: String
  let recoveryLifecycleCorrelationText: String
  let recoveryLifecycleFreshnessBudgetText: String
  let recoveryLifecycleActionRows: [String]
  let recoveryLifecycleCopyTemplate: String
  let recoveryLifecycleAccessibilityLabel: String
  let backlinkRouteLabel: String
  let backlinkRouteAccessibilityLabel: String
  let focusReturnLabel: String
  let copyFailureFallbackText: String
  let voiceOverAnnouncementPolicy: String
  let keyboardTraversalOrder: [String]
  let generationDisplayID: String
  let generationCopyValue: String?
  let generationCopyAccessibilityLabel: String
  let diagnosticsAccessibilityLabel: String
  let compactActivationAccessibilityLabel: String
  let cardAccessibilityLabel: String
  let modeExplainerAccessibilityLabel: String
  let visualState: P077CloseoutReadinessVisualState
}

enum P077CloseoutReadinessAnnouncementPriority: String, Equatable, Sendable {
  case polite
  case assertive

  nonisolated var accessibilityPriorityLabel: String {
    switch self {
    case .polite:
      return "polite"
    case .assertive:
      return "assertive"
    }
  }
}

struct P077CloseoutReadinessAnnouncement: Equatable, Sendable {
  let generationID: String
  let text: String
  let priority: P077CloseoutReadinessAnnouncementPriority
}

struct P077CloseoutReadinessAnnouncementState: Equatable, Sendable {
  var lastGenerationID: String?
  var lastFieldHash: String?
  var lastAnnouncementAt: Date?
}

enum P077CloseoutReadinessAnnouncementPolicy {
  nonisolated static let coalescingWindow: TimeInterval = 3

  nonisolated static func announcement(
    for presentation: P077CloseoutReadinessPresentation,
    previous state: inout P077CloseoutReadinessAnnouncementState,
    now: Date,
    sheetOwnsFocus: Bool
  ) -> P077CloseoutReadinessAnnouncement? {
    guard let generationID = presentation.generationCopyValue, !generationID.isEmpty else {
      return nil
    }
    guard state.lastGenerationID != generationID else {
      return nil
    }

    let fieldHash = [
      presentation.statusLabel,
      presentation.primaryUnblockText,
      presentation.modeLabel,
    ].joined(separator: "|")
    if state.lastFieldHash == fieldHash,
       let lastAnnouncementAt = state.lastAnnouncementAt,
       now.timeIntervalSince(lastAnnouncementAt) < coalescingWindow {
      return nil
    }

    let priority = announcementPriority(for: presentation)
    if sheetOwnsFocus && priority == .polite {
      return nil
    }

    state.lastGenerationID = generationID
    state.lastFieldHash = fieldHash
    state.lastAnnouncementAt = now
    return P077CloseoutReadinessAnnouncement(
      generationID: generationID,
      text: presentation.compactActivationAccessibilityLabel,
      priority: priority
    )
  }

  private nonisolated static func announcementPriority(
    for presentation: P077CloseoutReadinessPresentation
  ) -> P077CloseoutReadinessAnnouncementPriority {
    if presentation.visualState == .blocking,
       presentation.modeLabel.localizedCaseInsensitiveContains("enforcement") {
      return .assertive
    }
    if presentation.primaryUnblockText.localizedCaseInsensitiveContains("authority") {
      return .assertive
    }
    return .polite
  }
}

struct P031ActiveAgentTimelinePresentation: Equatable, Sendable {
  let id: String
  let title: String
  let detail: String
  let timestamp: Date
  let stageID: String?
  let providerID: String?
  let stageLabel: String?
  let taskLabel: String?
  let status: String
  let eventCount: Int?
  let selectionOrder: Int?
  let selectionUnavailableReason: String?
  let agentID: String
  let sessionID: String?

  nonisolated init(
    id: String,
    title: String,
    detail: String,
    timestamp: Date,
    stageID: String?,
    providerID: String? = nil,
    stageLabel: String? = nil,
    taskLabel: String? = nil,
    status: String = "running",
    eventCount: Int? = nil,
    selectionOrder: Int? = nil,
    selectionUnavailableReason: String? = nil,
    agentID: String,
    sessionID: String?
  ) {
    self.id = id
    self.title = title
    self.detail = detail
    self.timestamp = timestamp
    self.stageID = stageID
    self.providerID = providerID
    self.stageLabel = stageLabel
    self.taskLabel = taskLabel
    self.status = status
    self.eventCount = eventCount
    self.selectionOrder = selectionOrder
    self.selectionUnavailableReason = selectionUnavailableReason
    self.agentID = agentID
    self.sessionID = sessionID
  }
}

struct P031StageTopologyOccurrencePresentation: Equatable, Sendable {
  let agentID: String
  let agentTitle: String
  let taskName: String
  let statusText: String
  let providerLabel: String
  let executionCountLabel: String?
}

struct P031StageTopologyTransitionPresentation: Equatable, Sendable {
  let toStageID: String
  let toLabel: String
  let detail: String?
}

struct P031StageTopologyPresentation: Equatable, Sendable {
  let stageID: String
  let ordinal: Int
  let title: String
  let ownerAgentID: String
  let ownerAgentTitle: String
  let status: String
  let statusText: String
  let isCurrent: Bool
  let iterationText: String?
  let attemptText: String?
  let approvalRequired: Bool
  let artifactCount: Int
  let communicationCount: Int
  let occurrences: [P031StageTopologyOccurrencePresentation]
  let transitions: [P031StageTopologyTransitionPresentation]
}

struct P031RunDetailPresentation: Equatable, Sendable {
  let title: String
  let workflowLabel: String?
  let statusLabel: String
  let progressLabel: String?
  let pendingApprovalsLabel: String?
  let rolloutDecisionSummary: RolloutDecisionSummary?
  let ideaContext: P031IdeaContextPresentation?
  let stageTransitions: [P031StageTransitionPresentation]
  let stageTopology: [P031StageTopologyPresentation]
  let approvalRows: [P031ApprovalInboxRowPresentation]
  let artifactRows: [P031ArtifactSummaryPresentation]
  let artifactViewerRows: [P031ArtifactViewerPresentation]
  let reportRows: [P031ReportMetadataRowPresentation]
  let activeAgentTimelineEntries: [P031ActiveAgentTimelinePresentation]
  let catalogContext: P031CatalogContextPresentation?
  let closeoutReadiness: P077CloseoutReadinessPresentation?
  let implementationCompletion: P088ImplementationCompletionPresentation?
  let sideEffectReadback: P078SideEffectReadbackPresentation?
  let continuationReadback: P086ContinuationReadbackPresentation?
  let freshness: P031FreshnessSnapshot
  let refreshFeedbackText: String
  let emptyStateTitle: String?
  let errorDescription: String?
  // P036: Expose typed fields for workbench classification
  let rawStatus: String
  let failedStages: Int

  nonisolated init(
    title: String,
    workflowLabel: String?,
    statusLabel: String,
    progressLabel: String?,
    pendingApprovalsLabel: String?,
    rolloutDecisionSummary: RolloutDecisionSummary? = nil,
    ideaContext: P031IdeaContextPresentation?,
    stageTransitions: [P031StageTransitionPresentation],
    stageTopology: [P031StageTopologyPresentation] = [],
    approvalRows: [P031ApprovalInboxRowPresentation],
    artifactRows: [P031ArtifactSummaryPresentation],
    artifactViewerRows: [P031ArtifactViewerPresentation],
    reportRows: [P031ReportMetadataRowPresentation],
    activeAgentTimelineEntries: [P031ActiveAgentTimelinePresentation] = [],
    catalogContext: P031CatalogContextPresentation?,
    closeoutReadiness: P077CloseoutReadinessPresentation? = nil,
    implementationCompletion: P088ImplementationCompletionPresentation? = nil,
    sideEffectReadback: P078SideEffectReadbackPresentation? = nil,
    continuationReadback: P086ContinuationReadbackPresentation? = nil,
    freshness: P031FreshnessSnapshot,
    refreshFeedbackText: String,
    emptyStateTitle: String?,
    errorDescription: String?,
    rawStatus: String,
    failedStages: Int
  ) {
    self.title = title
    self.workflowLabel = workflowLabel
    self.statusLabel = statusLabel
    self.progressLabel = progressLabel
    self.pendingApprovalsLabel = pendingApprovalsLabel
    self.ideaContext = ideaContext
    self.stageTransitions = stageTransitions
    self.stageTopology = stageTopology
    self.approvalRows = approvalRows
    self.artifactRows = artifactRows
    self.artifactViewerRows = artifactViewerRows
    self.reportRows = reportRows
    self.activeAgentTimelineEntries = activeAgentTimelineEntries
    self.catalogContext = catalogContext
    self.rolloutDecisionSummary = rolloutDecisionSummary
    self.closeoutReadiness = closeoutReadiness
    self.implementationCompletion = implementationCompletion
    self.sideEffectReadback = sideEffectReadback
    self.continuationReadback = continuationReadback
    self.freshness = freshness
    self.refreshFeedbackText = refreshFeedbackText
    self.emptyStateTitle = emptyStateTitle
    self.errorDescription = errorDescription
    self.rawStatus = rawStatus
    self.failedStages = failedStages
  }
}

struct P086ContinuationReadbackPresentation: Equatable, Sendable {
  let title: String
  let summary: String
  let latestStatus: String
  let latestMode: String
  let latestTrigger: String
  let latestContinuationID: String?
  let latestAgentExecutionID: String?
  let latestStageExecutionID: String?
  let artifactSummary: String
  let metricSummary: String
  let accessibilityLabel: String
}

enum P086ContinuationReadbackPresenter {
  nonisolated static func presentationIfPresent(
    records: [P086ContinuationRecordReadModel],
    metrics: P086ContinuationMetricsSummaryReadModel?
  ) -> P086ContinuationReadbackPresentation? {
    guard !records.isEmpty || metrics.map({ $0.admissionTotal > 0 }) == true else {
      return nil
    }

    let latest = records.sorted { lhs, rhs in
      lhs.updatedAt.localizedStandardCompare(rhs.updatedAt) == .orderedDescending
    }.first
    let artifactCount = latest.map { record in
      [
        record.canonicalRequestArtifactID,
        record.attachReceiptArtifactID,
        record.evidenceBundleArtifactID,
        record.worktreeReadbackArtifactID,
        record.continuationReportArtifactID,
        record.responseArtifactID,
        record.resultOrNoProgressArtifactID,
      ].compactMap { $0 }.count
    } ?? 0

    let total = metrics?.admissionTotal ?? records.count
    let succeeded = metrics?.successTotal ?? records.filter { $0.statusRaw == "succeeded" }.count
    let noProgress = metrics?.noProgressTotal ?? records.filter { $0.statusRaw == "no_progress" }.count
    let failed = metrics?.failedTotal ?? records.filter { $0.statusRaw == "failed" }.count
    let cancelled = metrics?.cancelledTotal ?? records.filter { $0.statusRaw == "cancelled" }.count
    let freshAvoided = metrics?.freshSessionAvoidedTotal ?? 0
    let orphanReaped = metrics?.orphanReapVerifiedTotal ?? 0
    let leadAuto = metrics?.leadAutoTotal ?? records.filter { $0.triggerKindRaw == "lead_auto" }.count
    let usefulProgressPercent = metrics.map { Int(($0.usefulProgressRate * 100).rounded()) }
    let followupValidationPercent = metrics.map {
      Int(($0.followupValidationSuccessRate * 100).rounded())
    }
    let averageTimeSavedSeconds = metrics?.averageTimeSavedSeconds ?? 0
    let providerBudgetTokens =
      (metrics?.providerSessionBudgetInputTokensTotal ?? 0)
      + (metrics?.providerSessionBudgetOutputTokensTotal ?? 0)
    let resurrectionFailures = metrics?.providerSessionResurrectionAttachFailureTotal ?? 0

    let status = latest?.statusDisplay ?? "No continuation history"
    let mode = latest?.modeDisplay ?? "Unavailable"
    let trigger = latest?.triggerKindDisplay ?? "Unavailable"
    let summary = [
      "\(total) admission\(total == 1 ? "" : "s")",
      "\(succeeded) succeeded",
      noProgress > 0 ? "\(noProgress) no progress" : nil,
      failed > 0 ? "\(failed) failed" : nil,
      cancelled > 0 ? "\(cancelled) cancelled" : nil,
    ].compactMap { $0 }.joined(separator: " · ")
    let metricSummary = [
      freshAvoided > 0 ? "\(freshAvoided) fresh session avoided" : nil,
      leadAuto > 0 ? "\(leadAuto) lead-auto" : nil,
      usefulProgressPercent.map { "useful progress \($0)%" },
      followupValidationPercent.map { "follow-up validation \($0)%" },
      averageTimeSavedSeconds > 0
        ? "avg \(Int(averageTimeSavedSeconds.rounded()))s saved" : nil,
      providerBudgetTokens > 0 ? "\(providerBudgetTokens) provider tokens" : nil,
      resurrectionFailures > 0 ? "\(resurrectionFailures) resurrection failed" : nil,
      orphanReaped > 0 ? "\(orphanReaped) orphan reaped" : nil,
    ].compactMap { $0 }.joined(separator: " · ")
    let artifacts = artifactCount > 0 ? "\(artifactCount) evidence artifact\(artifactCount == 1 ? "" : "s")" : "No terminal evidence artifacts"
    return P086ContinuationReadbackPresentation(
      title: "Agent Continuation",
      summary: summary.isEmpty ? status : summary,
      latestStatus: status,
      latestMode: mode,
      latestTrigger: trigger,
      latestContinuationID: latest?.id,
      latestAgentExecutionID: latest?.agentExecutionID,
      latestStageExecutionID: latest?.stageExecutionID,
      artifactSummary: artifacts,
      metricSummary: metricSummary.isEmpty ? "No rollout metrics yet" : metricSummary,
      accessibilityLabel: "P086 continuation readback, latest status \(status), \(summary)"
    )
  }
}

enum P077CloseoutReadinessPresenter {
  nonisolated static func presentation(
    for summary: P077CloseoutReadinessSummaryReadModel
  ) -> P077CloseoutReadinessPresentation {
    let state = displayState(for: summary)
    let statusLabel = statusText(for: state, summary: summary)
    let primaryUnblockText = primaryUnblock(for: state, summary: summary)
    let modeLabel = "Mode: \(P031ThinPresentationFormatting.titleCase(summary.readinessMode))"
    let modeExplainerText =
      "Closeout readiness mode: \(P031ThinPresentationFormatting.titleCase(summary.readinessMode)). This view is read-only; closeout actions run through governed CLI, MCP, orchestrator, or approval paths."
    let generationCopyValue = summary.hasGenerationID ? summary.readinessGenerationID : nil
    let generationText = generationCopyValue == nil
      ? "No generation yet"
      : "Generation \(summary.generationDisplayID)"
    let detailText = detailText(for: state, summary: summary, generationText: generationText)
    let secondaryBlockerRows = secondaryBlockers(for: state, summary: summary)
    let diagnosticRows = diagnostics(for: state, summary: summary, generationText: generationText)
    let recoveryLifecycleText = recoveryLifecycle(for: state, summary: summary)
    let recoveryLifecycleAcknowledgementText = recoveryAcknowledgement(for: state, summary: summary)
    let recoveryLifecycleCorrelationText = recoveryCorrelation(for: summary)
    let recoveryLifecycleFreshnessBudgetText = recoveryFreshnessBudget(for: state, summary: summary)
    let recoveryLifecycleActionRows = recoveryActions(for: state, summary: summary)
    let recoveryLifecycleCopyTemplate = recoveryCopyTemplate(
      for: state,
      summary: summary,
      primaryUnblockText: primaryUnblockText
    )
    let recoveryLifecycleAccessibilityLabel = [
      "Recovery lifecycle, non-dismissible",
      recoveryLifecycleText,
      recoveryLifecycleAcknowledgementText,
      recoveryLifecycleCorrelationText,
      recoveryLifecycleFreshnessBudgetText,
      "Actions: \(recoveryLifecycleActionRows.joined(separator: ", "))",
    ].joined(separator: ". ")
    let backlinkRouteLabel = backlinkRoute(for: state, summary: summary)
    let backlinkRouteAccessibilityLabel =
      "Closeout readiness readback route: \(backlinkRouteLabel)"
    let focusReturnLabel = "Returns focus to Closeout Readiness after copy or diagnostics dismissal."
    let copyFailureFallbackText =
      "Copy failed. Generation \(summary.generationDisplayID) remains visible in diagnostics."
    let voiceOverAnnouncementPolicy =
      "No automatic repeating announcements; VoiceOver reads compact signal, card summary, and diagnostics on demand."
    let keyboardTraversalOrder = [
      "compact signal",
      "diagnostics",
      "copy generation id",
      "primary unblock",
      "recovery lifecycle",
      "copy recovery template",
      "readback route",
      "mode explainer",
    ]
    let compactSignalLabel = "Closeout: \(statusLabel)"
    let cardAccessibilityParts = [
      "Closeout readiness",
      statusLabel,
      primaryUnblockText,
      modeLabel,
      generationText,
      recoveryLifecycleText,
    ]

    return P077CloseoutReadinessPresentation(
      statusLabel: statusLabel,
      compactSignalLabel: compactSignalLabel,
      detailText: detailText,
      primaryUnblockText: primaryUnblockText,
      secondaryBlockerRows: secondaryBlockerRows,
      modeLabel: modeLabel,
      modeExplainerText: modeExplainerText,
      diagnosticRows: diagnosticRows,
      recoveryLifecycleText: recoveryLifecycleText,
      recoveryLifecycleAcknowledgementText: recoveryLifecycleAcknowledgementText,
      recoveryLifecycleCorrelationText: recoveryLifecycleCorrelationText,
      recoveryLifecycleFreshnessBudgetText: recoveryLifecycleFreshnessBudgetText,
      recoveryLifecycleActionRows: recoveryLifecycleActionRows,
      recoveryLifecycleCopyTemplate: recoveryLifecycleCopyTemplate,
      recoveryLifecycleAccessibilityLabel: recoveryLifecycleAccessibilityLabel,
      backlinkRouteLabel: backlinkRouteLabel,
      backlinkRouteAccessibilityLabel: backlinkRouteAccessibilityLabel,
      focusReturnLabel: focusReturnLabel,
      copyFailureFallbackText: copyFailureFallbackText,
      voiceOverAnnouncementPolicy: voiceOverAnnouncementPolicy,
      keyboardTraversalOrder: keyboardTraversalOrder,
      generationDisplayID: summary.generationDisplayID,
      generationCopyValue: generationCopyValue,
      generationCopyAccessibilityLabel: generationCopyValue == nil
        ? "No closeout readiness generation id to copy"
        : "Copy closeout readiness generation id \(summary.generationDisplayID)",
      diagnosticsAccessibilityLabel:
        "Show closeout readiness diagnostics for \(statusLabel)",
      compactActivationAccessibilityLabel:
        "Closeout readiness compact signal, \(statusLabel), \(primaryUnblockText)",
      cardAccessibilityLabel: cardAccessibilityParts.joined(separator: ", "),
      modeExplainerAccessibilityLabel: modeExplainerText,
      visualState: visualState(for: state, summary: summary)
    )
  }

  private enum DisplayState: Equatable {
    case applicable(P077CloseoutReadinessStatus)
    case awaitingFirstGeneration
    case notApplicable
  }

  private nonisolated static func displayState(
    for summary: P077CloseoutReadinessSummaryReadModel
  ) -> DisplayState {
    guard summary.isApplicable else {
      return .notApplicable
    }
    if summary.diagnosticReason == "awaiting_first_generation" {
      return .awaitingFirstGeneration
    }
    return .applicable(summary.readinessStatus)
  }

  private nonisolated static func statusText(
    for state: DisplayState,
    summary: P077CloseoutReadinessSummaryReadModel
  ) -> String {
    switch state {
    case .notApplicable:
      return "Not Applicable"
    case .awaitingFirstGeneration:
      return "Awaiting First Generation"
    case .applicable(.ready):
      return "Ready"
    case .applicable(.readyWithRisks):
      return "Ready with Risks"
    case .applicable(.handoffRequired):
      return "Handoff Required"
    case .applicable(.notReady):
      return "Not Ready"
    case .applicable(.blocked):
      return "Blocked"
    case .applicable(.invalid):
      return "Invalid"
    case .applicable(.unknown):
      return "Unknown"
    }
  }

  private nonisolated static func primaryUnblock(
    for state: DisplayState,
    summary: P077CloseoutReadinessSummaryReadModel
  ) -> String {
    let explicit = [summary.primaryUnblock, summary.summary, summary.diagnosticReason]
      .compactMap { normalizedText($0) }
      .first
    switch state {
    case .notApplicable:
      return "Closeout readiness not applicable for this run."
    case .awaitingFirstGeneration:
      return explicit ?? "Awaiting first readiness check."
    case .applicable(.ready):
      return explicit ?? "Ready for manual release."
    case .applicable(.readyWithRisks):
      if let explicit {
        return explicit
      }
      return summary.acceptedRiskCount > 0
        ? "Accepted risks: \(summary.acceptedRiskCount)"
        : "Accepted risks require release-owner review."
    case .applicable(.handoffRequired):
      return explicit ?? "Complete non-code handoff before release."
    case .applicable(.notReady):
      return explicit ?? "Resolve code blockers before closeout."
    case .applicable(.blocked):
      return explicit ?? "Resolve blocking closeout evidence."
    case .applicable(.invalid):
      return explicit ?? "Regenerate valid closeout readiness evidence."
    case .applicable(.unknown):
      return explicit ?? "Closeout readiness is unknown."
    }
  }

  private nonisolated static func detailText(
    for state: DisplayState,
    summary: P077CloseoutReadinessSummaryReadModel,
    generationText: String
  ) -> String {
    switch state {
    case .notApplicable:
      return "Closeout readiness not applicable for this run. \(generationText)."
    case .awaitingFirstGeneration:
      return "Awaiting first readiness check. \(generationText)."
    case .applicable:
      let blockerText = [
        summary.codeBlockerCount > 0 ? "\(summary.codeBlockerCount) code blockers" : nil,
        summary.handoffCount > 0 ? "\(summary.handoffCount) handoff items" : nil,
        summary.acceptedRiskCount > 0 ? "\(summary.acceptedRiskCount) accepted risks" : nil,
      ].compactMap { $0 }.joined(separator: ", ")
      let evidenceText = blockerText.isEmpty ? "No counted blockers" : blockerText
      return "\(summary.readinessDecision) via \(summary.gateStatus). \(evidenceText). \(generationText)."
    }
  }

  private nonisolated static func secondaryBlockers(
    for state: DisplayState,
    summary: P077CloseoutReadinessSummaryReadModel
  ) -> [String] {
    var rows: [String] = []
    if summary.codeBlockerCount > 0 {
      rows.append("\(summary.codeBlockerCount) code blocker\(summary.codeBlockerCount == 1 ? "" : "s") remain")
    }
    if summary.handoffCount > 0 {
      let owner = normalizedText(summary.handoffOwner).map { " for \($0)" } ?? ""
      rows.append("\(summary.handoffCount) handoff item\(summary.handoffCount == 1 ? "" : "s")\(owner)")
    }
    if summary.riskSettlementRequired {
      rows.append("Risk settlement is required before release")
    }
    if summary.acceptedRiskCount > 0 {
      rows.append("\(summary.acceptedRiskCount) accepted risk\(summary.acceptedRiskCount == 1 ? "" : "s") recorded")
    }
    if case .awaitingFirstGeneration = state {
      rows.append("No active readiness generation has been published yet")
    }
    if case .notApplicable = state {
      rows.append("This run is outside the P077 closeout-readiness scope")
    }
    return rows
  }

  private nonisolated static func diagnostics(
    for state: DisplayState,
    summary: P077CloseoutReadinessSummaryReadModel,
    generationText: String
  ) -> [String] {
    [
      "Decision: \(summary.readinessDecision)",
      "Gate: \(summary.gateStatus)",
      normalizedText(summary.auditStatus).map { "Audit: \($0)" },
      normalizedText(summary.diagnosticReason).map { "Diagnostic: \($0)" },
      normalizedText(summary.fingerprintHash).map { "Fingerprint: \($0)" },
      "Mode: \(summary.readinessMode)",
      generationText,
    ].compactMap { $0 }
  }

  private nonisolated static func recoveryLifecycle(
    for state: DisplayState,
    summary: P077CloseoutReadinessSummaryReadModel
  ) -> String {
    switch state {
    case .applicable(.ready), .applicable(.readyWithRisks):
      return "Recovery lifecycle: enter manual release through governed control surfaces."
    case .applicable(.notReady):
      return "Recovery lifecycle: return to code refine and rerun closeout readiness."
    case .applicable(.handoffRequired):
      return "Recovery lifecycle: complete handoff owner action, then rerun closeout readiness."
    case .applicable(.blocked), .applicable(.invalid), .applicable(.unknown),
         .awaitingFirstGeneration:
      return "Recovery lifecycle: inspect diagnostics, settle the gate, or rerun the readiness check."
    case .notApplicable:
      return "Recovery lifecycle: no P077 action is available for this run."
    }
  }

  private nonisolated static func recoveryAcknowledgement(
    for state: DisplayState,
    summary: P077CloseoutReadinessSummaryReadModel
  ) -> String {
    switch state {
    case .notApplicable:
      return "Acknowledgement: not applicable for this run."
    case .applicable(.ready), .applicable(.readyWithRisks):
      return "Acknowledgement: current generation is settled for release review."
    case .awaitingFirstGeneration:
      return "Acknowledgement: no governed recovery acknowledgement has been recorded yet."
    case .applicable:
      let timestamp = normalizedText(summary.synthesizedAt) ?? "unknown time"
      return "Acknowledgement: blocker observed at \(timestamp); wait for governed channel acknowledgement before release."
    }
  }

  private nonisolated static func recoveryCorrelation(
    for summary: P077CloseoutReadinessSummaryReadModel
  ) -> String {
    let gate = normalizedText(summary.gateGenerationID) ?? "missing gate generation"
    let fingerprint = normalizedText(summary.fingerprintHash) ?? "missing fingerprint"
    return "Correlation: run \(summary.runID), stage \(summary.stageID), readiness generation \(summary.generationDisplayID), gate \(gate), fingerprint \(fingerprint)."
  }

  private nonisolated static func recoveryFreshnessBudget(
    for state: DisplayState,
    summary: P077CloseoutReadinessSummaryReadModel
  ) -> String {
    switch state {
    case .applicable(.ready), .applicable(.readyWithRisks):
      return "Freshness budget: keep release evidence current before manual release."
    case .notApplicable:
      return "Freshness budget: no P077 freshness budget applies."
    case .awaitingFirstGeneration:
      return "Freshness budget: publish the first closeout readiness generation before release review."
    case .applicable:
      let synthesizedAt = normalizedText(summary.synthesizedAt) ?? "unknown generation time"
      return "Freshness budget: if no new generation replaces this blocker after \(synthesizedAt), treat recovery as stalled and re-issue through governed control."
    }
  }

  private nonisolated static func recoveryActions(
    for state: DisplayState,
    summary: P077CloseoutReadinessSummaryReadModel
  ) -> [String] {
    switch state {
    case .notApplicable:
      return ["Return to run detail"]
    case .applicable(.ready), .applicable(.readyWithRisks):
      return ["Re-copy generation id", "Open manual-release evidence route"]
    case .applicable(.handoffRequired):
      let owner = normalizedText(summary.handoffOwner) ?? "handoff owner"
      return [
        "Re-copy generation id",
        "Re-issue closeout readiness after handoff settlement",
        "Escalate to \(owner)",
      ]
    case .awaitingFirstGeneration:
      return [
        "Re-issue closeout readiness",
        "Escalate missing generation to release owner",
      ]
    case .applicable:
      return [
        "Re-copy generation id",
        "Re-issue closeout readiness after recovery action",
        "Escalate to release owner",
      ]
    }
  }

  private nonisolated static func recoveryCopyTemplate(
    for state: DisplayState,
    summary: P077CloseoutReadinessSummaryReadModel,
    primaryUnblockText: String
  ) -> String {
    let owner = normalizedText(summary.handoffOwner) ?? "release owner"
    let commandLabel: String
    switch state {
    case .applicable(.notReady):
      commandLabel = "return to code refine and rerun closeout readiness"
    case .applicable(.handoffRequired):
      commandLabel = "complete handoff settlement and rerun closeout readiness"
    case .applicable(.ready), .applicable(.readyWithRisks):
      commandLabel = "enter manual release through governed control"
    case .applicable(.blocked), .applicable(.invalid), .applicable(.unknown),
         .awaitingFirstGeneration:
      commandLabel = "settle proposal gate or rerun readiness check"
    case .notApplicable:
      commandLabel = "no P077 action"
    }

    return [
      "P077 recovery escalation",
      "run=\(summary.runID)",
      "stage=\(summary.stageID)",
      "generation=\(summary.generationDisplayID)",
      "decision=\(summary.readinessDecision)",
      "status=\(summary.readinessStatus.rawValue)",
      "command=\(commandLabel)",
      "owner=\(owner)",
      "primary_unblock=\(primaryUnblockText)",
      "diagnostic=\(normalizedText(summary.diagnosticReason) ?? "none")",
      "fingerprint=\(normalizedText(summary.fingerprintHash) ?? "missing")",
    ].joined(separator: "; ")
  }

  private nonisolated static func backlinkRoute(
    for state: DisplayState,
    summary: P077CloseoutReadinessSummaryReadModel
  ) -> String {
    switch state {
    case .applicable(.ready), .applicable(.readyWithRisks):
      return "Manual release evidence"
    case .applicable(.handoffRequired):
      return normalizedText(summary.handoffOwner).map { "Handoff owner: \($0)" }
        ?? "Handoff owner"
    case .applicable(.notReady), .applicable(.blocked), .applicable(.invalid),
         .applicable(.unknown), .awaitingFirstGeneration:
      return "Closeout diagnostics"
    case .notApplicable:
      return "Run detail"
    }
  }

  private nonisolated static func visualState(
    for state: DisplayState,
    summary: P077CloseoutReadinessSummaryReadModel
  ) -> P077CloseoutReadinessVisualState {
    switch state {
    case .notApplicable:
      return .neutral
    case .awaitingFirstGeneration:
      return .warning
    case .applicable(.ready):
      return .positive
    case .applicable(.readyWithRisks), .applicable(.handoffRequired), .applicable(.unknown):
      return .warning
    case .applicable(.notReady), .applicable(.blocked), .applicable(.invalid):
      return .blocking
    }
  }

  private nonisolated static func normalizedText(_ value: String?) -> String? {
    let trimmed = value?.trimmingCharacters(in: .whitespacesAndNewlines)
    return trimmed?.isEmpty == false ? trimmed : nil
  }
}

struct P031StageDetailPresentation: Equatable, Sendable {
  let stage: P031StageSummaryPresentation?
  let freshness: P031FreshnessSnapshot
  let refreshFeedbackText: String
  let emptyStateTitle: String?
  let errorDescription: String?
}

struct P031ArtifactListPresentation: Equatable, Sendable {
  let rows: [P031ArtifactSummaryPresentation]
  let freshness: P031FreshnessSnapshot
  let refreshFeedbackText: String
  let emptyStateTitle: String?
  let errorDescription: String?
}

struct P031ReportMetadataListPresentation: Equatable, Sendable {
  let rows: [P031ReportMetadataRowPresentation]
  let freshness: P031FreshnessSnapshot
  let refreshFeedbackText: String
  let emptyStateTitle: String?
  let errorDescription: String?
}

struct P031DaemonLifecyclePresentation: Equatable, Sendable {
  let state: P031DaemonLifecycleState?
  let buildSHA: String?
  let pid: Int?
  let uptimeSeconds: Int?
  let title: String
  let detailLabel: String?
  let badgeLabels: [String]
  let copyItems: [P031DiagnosticCopyItem]
  let freshness: P031FreshnessSnapshot
  let refreshFeedbackText: String
  let errorDescription: String?
}

enum P031RunsHomePresenter {
  nonisolated static func presentation(
    for runs: [P031RunRowReadModel],
    currentFreshness: P031FreshnessSnapshot,
    checkedAt: Date,
    writePathGuideState: P031ExternalWritePathGuideState,
    showFirstRunOrientation: Bool
  ) -> P031RunsHomePresentation {
    let rows = runs.map(rowPresentation)
    let freshness = P031ThinPresentationFormatting.freshnessSnapshot(
      currentFreshness: currentFreshness,
      checkedAt: checkedAt,
      states: runs.map(\.freshnessState)
    )
    return P031RunsHomePresentation(
      orientation: showFirstRunOrientation
        ? P031FirstRunOrientationPresenter.presentation(writePathGuideState: writePathGuideState)
        : nil,
      rows: rows,
      freshness: freshness,
      refreshFeedbackText: P031ReadRefreshPresenter.feedbackText(for: .runsHome),
      emptyStateTitle: rows.isEmpty ? "No runs" : nil,
      errorDescription: nil
    )
  }

  nonisolated static func errorPresentation(
    error: Error,
    currentFreshness: P031FreshnessSnapshot,
    checkedAt: Date,
    writePathGuideState: P031ExternalWritePathGuideState,
    showFirstRunOrientation: Bool
  ) -> P031RunsHomePresentation {
    P031RunsHomePresentation(
      orientation: showFirstRunOrientation
        ? P031FirstRunOrientationPresenter.presentation(writePathGuideState: writePathGuideState)
        : nil,
      rows: [],
      freshness: WorkflowFreshnessReducer.reduce(
        currentFreshness,
        event: .refreshFailed(checkedAt: checkedAt, reason: P031ReadErrorPresenter.description(for: error))
      ),
      refreshFeedbackText: P031ReadRefreshPresenter.feedbackText(for: .runsHome),
      emptyStateTitle: nil,
      errorDescription: P031ReadErrorPresenter.description(for: error)
    )
  }

  nonisolated static func rowPresentation(for run: P031RunRowReadModel)
    -> P031RunsHomeRowPresentation
  {
    let trimmedIdeaTitle = run.ideaTitle?.trimmingCharacters(in: .whitespacesAndNewlines)
    let workflowTitle = run.workflowTitle.trimmingCharacters(in: .whitespacesAndNewlines)
    let ideaTitle = trimmedIdeaTitle.flatMap { $0.isEmpty ? nil : $0 }
    let displayTitle =
      ideaTitle ?? (workflowTitle.isEmpty ? "Untitled workflow" : workflowTitle)
    let workflowLabel =
      ideaTitle != nil && !workflowTitle.isEmpty
      ? "Workflow: \(workflowTitle)"
      : nil
    let statusLabel = P031ThinPresentationFormatting.titleCase(run.status)
    let progressLabel: String?
    if let completedStages = run.completedStages, let totalStages = run.totalStages {
      progressLabel = "\(completedStages)/\(totalStages) stages"
    } else {
      progressLabel = nil
    }
    let pendingApprovals = run.pendingApprovals ?? 0
    let pendingApprovalsLabel =
      pendingApprovals > 0
      ? "\(pendingApprovals) approvals pending"
      : nil
    let closeoutReadiness = run.closeoutReadinessSummary.map {
      P077CloseoutReadinessPresenter.presentation(for: $0)
    }
    let implementationCompletion = P088ImplementationCompletionPresenter.presentationIfPresent(
      for: run.implementationCompletion
    )
    let sideEffectReadback = P078SideEffectReadbackPresenter.presentationIfPresent(
      for: run.sideEffectReadback
    )
    let accessibilityParts = [
      displayTitle,
      workflowLabel,
      statusLabel,
      progressLabel,
      pendingApprovalsLabel,
      closeoutReadiness?.compactSignalLabel,
      implementationCompletion?.compactSignalLabel,
      sideEffectReadback?.compactSignalLabel,
      P031ThinPresentationFormatting.freshnessAccessibilityLabel(run.freshnessState),
    ].compactMap { $0 }

    return P031RunsHomeRowPresentation(
      runID: run.id,
      title: displayTitle,
      workflowLabel: workflowLabel,
      statusLabel: statusLabel,
      progressLabel: progressLabel,
      pendingApprovalsLabel: pendingApprovalsLabel,
      closeoutReadinessSignalLabel: closeoutReadiness?.compactSignalLabel,
      implementationCompletionSignalLabel: implementationCompletion?.compactSignalLabel,
      sideEffectSignalLabel: sideEffectReadback?.compactSignalLabel,
      freshnessState: run.freshnessState,
      accessibilityLabel: accessibilityParts.joined(separator: ", "),
      rawStatus: run.status,
      failedStages: run.failedStages ?? 0,
      pendingApprovals: pendingApprovals
    )
  }
}

enum P031RunStatusSubscriptionPresenter {
  nonisolated static func presentation(
    for event: P031RunStatusChangedReadModel,
    currentFreshness: P031FreshnessSnapshot,
    checkedAt: Date
  ) -> P031RunStatusSubscriptionPresentation {
    let statusLabel = P031ThinPresentationFormatting.titleCase(event.status)
    let badgeLabels = P031ThinPresentationFormatting.uniqueLabels(
      [
        event.projectionLag == true ? "Projection lag" : nil,
        P031ThinPresentationFormatting.freshnessAccessibilityLabel(event.freshnessState),
      ].compactMap { $0 })
    let freshness = WorkflowFreshnessReducer.reduce(
      currentFreshness,
      event: .serverStateReceived(event.freshnessState, checkedAt: checkedAt, reason: nil)
    )
    let accessibilityParts =
      [
        event.id,
        statusLabel,
      ] + badgeLabels

    return P031RunStatusSubscriptionPresentation(
      runID: event.id,
      statusLabel: statusLabel,
      badgeLabels: badgeLabels,
      freshness: freshness,
      accessibilityLabel: accessibilityParts.joined(separator: ", ")
    )
  }
}

enum P031RuntimeTimelineEventPresenter {
  nonisolated static func presentation(
    for event: P031RuntimeTimelineEventReadModel
  ) -> P031RuntimeTimelineEventPresentation? {
    guard let timestamp = P031ReadBoundaryDateParser.date(from: event.timestamp) else {
      return nil
    }
    let title = normalized(event.title) ?? title(for: event.eventKind)
    let detail = normalized(event.detail) ?? event.eventKind
    let rawDetail = normalized(event.rawDetail) ?? detail
    let surfaceLabel = normalized(event.surfaceLabel) ?? event.eventKind
    return P031RuntimeTimelineEventPresentation(
      id: normalized(event.id) ?? [
        event.runID,
        event.stageID,
        event.agentID,
        event.sessionGenerationID ?? "session",
        event.eventKind,
        event.timestamp,
      ].joined(separator: ":"),
      runID: event.runID,
      stageID: normalized(event.stageID),
      agentID: event.agentID,
      provider: event.provider,
      eventKind: event.eventKind,
      title: title,
      detail: detail,
      surfaceLabel: surfaceLabel,
      sessionGenerationID: normalized(event.sessionGenerationID),
      timestamp: timestamp,
      rawDetail: rawDetail,
      rawDetailBytes: event.rawDetailBytes,
      rawDetailTruncated: event.rawDetailTruncated ?? false,
      rawDetailHandle: normalized(event.rawDetailHandle),
      rawDetailDigest: normalized(event.rawDetailDigest),
      fullRawAvailable: event.fullRawAvailable ?? false,
      detailDigest: normalized(event.detailDigest),
      detailCharCount: event.detailCharCount ?? detail.count,
      chunkCount: event.chunkCount,
      isStreaming: event.isStreaming ?? Self.isStreaming(eventKind: event.eventKind, surfaceLabel: surfaceLabel),
      isTerminal: event.isTerminal ?? Self.isTerminal(eventKind: event.eventKind, surfaceLabel: surfaceLabel),
      stateLabel: normalized(event.stateLabel)
    )
  }

  nonisolated private static func isStreaming(eventKind: String, surfaceLabel: String) -> Bool {
    surfaceLabel == "text_chunk"
      || surfaceLabel == "agent_message_chunk"
      || eventKind == "meaningful_progress"
  }

  nonisolated private static func isTerminal(eventKind: String, surfaceLabel: String) -> Bool {
    surfaceLabel == "final_response"
      || surfaceLabel == "agent_summary"
      || eventKind == "session_completed"
      || eventKind == "session_failed"
  }

  nonisolated private static func title(for eventKind: String) -> String {
    switch eventKind {
    case "prompt_sent": return "Prompt sent"
    case "meaningful_progress": return "Runtime update"
    case "session_completed": return "Session completed"
    case "session_failed": return "Session failed"
    default:
      return eventKind
        .replacingOccurrences(of: "_", with: " ")
        .capitalized
    }
  }

  nonisolated private static func normalized(_ value: String?) -> String? {
    let trimmed = value?.trimmingCharacters(in: .whitespacesAndNewlines)
    return trimmed?.isEmpty == false ? trimmed : nil
  }
}

enum P031ApprovalInboxPresenter {
  nonisolated static func presentation(
    for approvals: [P031ApprovalReadModel],
    currentFreshness: P031FreshnessSnapshot,
    checkedAt: Date,
    writePathGuideState: P031ExternalWritePathGuideState
  ) -> P031ApprovalInboxPresentation {
    let rows = approvals.map {
      rowPresentation(for: $0, writePathGuideState: writePathGuideState)
    }
    return P031ApprovalInboxPresentation(
      rows: rows,
      freshness: P031ThinPresentationFormatting.freshnessSnapshot(
        currentFreshness: currentFreshness,
        checkedAt: checkedAt,
        states: approvals.map(\.freshnessState)
      ),
      refreshFeedbackText: P031ReadRefreshPresenter.feedbackText(for: .approvalsQueue),
      emptyStateTitle: rows.isEmpty ? "No pending approvals" : nil,
      errorDescription: nil
    )
  }

  nonisolated static func errorPresentation(
    error: Error,
    currentFreshness: P031FreshnessSnapshot,
    checkedAt: Date
  ) -> P031ApprovalInboxPresentation {
    P031ApprovalInboxPresentation(
      rows: [],
      freshness: WorkflowFreshnessReducer.reduce(
        currentFreshness,
        event: .refreshFailed(checkedAt: checkedAt, reason: P031ReadErrorPresenter.description(for: error))
      ),
      refreshFeedbackText: P031ReadRefreshPresenter.feedbackText(for: .approvalsQueue),
      emptyStateTitle: nil,
      errorDescription: P031ReadErrorPresenter.description(for: error)
    )
  }

  nonisolated static func rowPresentation(
    for approval: P031ApprovalReadModel,
    writePathGuideState: P031ExternalWritePathGuideState
  ) -> P031ApprovalInboxRowPresentation {
    let diagnostic = ApprovalDiagnosticPresenter.presentation(
      for: approval,
      externalWritePathGuideState: writePathGuideState
    )
    let accessibilityParts = [
      diagnostic.title,
      diagnostic.body,
      approval.runID,
      approval.stageID,
    ]
    // P085: derive actionability through the canonical affordance presenter, which checks
    // durable decision state, writePathState, and availableActions before granting actionable.
    let p085Affordance = P085AffordancePresenter.approvalAffordance(for: approval)
    let canApprove: Bool
    if case .actionable = p085Affordance.approveAvailability { canApprove = true } else { canApprove = false }
    let canReject: Bool
    if case .actionable = p085Affordance.rejectAvailability { canReject = true } else { canReject = false }

    return P031ApprovalInboxRowPresentation(
      approvalID: approval.id,
      title: diagnostic.title,
      body: diagnostic.body,
      canApprove: canApprove,
      canReject: canReject,
      actionLabel: diagnostic.actionLabel,
      followUpID: diagnostic.followUpID,
      copyItems: diagnostic.copyItems,
      freshnessState: approval.freshnessState,
      accessibilityLabel: accessibilityParts.joined(separator: ", "),
      affordance: p085Affordance
    )
  }
}

enum P031RunDetailPresenter {
  nonisolated static func presentation(
    for detail: P031RunDetailReadModel,
    currentFreshness: P031FreshnessSnapshot,
    checkedAt: Date,
    writePathGuideState: P031ExternalWritePathGuideState = .unavailable
  ) -> P031RunDetailPresentation {
    let run = detail.run
    let runRow = run.map(P031RunsHomePresenter.rowPresentation)
    let title = runRow?.title ?? "Run unavailable"
    let workflowLabel = runRow?.workflowLabel
    let statusLabel =
      run.map { P031ThinPresentationFormatting.titleCase($0.status) } ?? "Unavailable"
    let stageTransitions = detail.stages.map { P031StageTransitionPresenter.presentation(for: $0) }
    let stageTopology = detail.runStageTopology.map(P031StageTopologyPresenter.presentation)
    let approvalRows = detail.approvalsForRun.map {
      P031ApprovalInboxPresenter.rowPresentation(
        for: $0,
        writePathGuideState: writePathGuideState
      )
    }
    let artifactRows = detail.ordinaryArtifacts.map(P031ArtifactPresenter.presentation)
    let stageByExecutionID = Dictionary(uniqueKeysWithValues: detail.stages.map { ($0.id, $0) })
    let stageWindowsByStageID = stageExecutionWindowsByStageID(detail.stages)
    let artifactViewerRows = detail.ordinaryArtifacts.map { artifact in
      let sourceStage = artifact.sourceStageExecutionID.flatMap { stageByExecutionID[$0] }
      return P031ArtifactViewerPresenter.presentation(
        for: artifact,
        stage: sourceStage ?? stageExecution(for: artifact, windowsByStageID: stageWindowsByStageID)
      )
    }
    let reportRows = detail.reportMetadata.map(ReportMetadataRowPresenter.presentation)
    let activeAgentTimelineEntries = activeAgentTimelineEntries(
      from: detail.activeAgentExecutions,
      stageByExecutionID: stageByExecutionID
    )
    let progressLabel: String?
    if let completedStages = run?.completedStages, let totalStages = run?.totalStages {
      progressLabel = "\(completedStages)/\(totalStages) stages"
    } else {
      progressLabel = nil
    }
    let pendingApprovals = run?.pendingApprovals ?? 0
    let pendingApprovalsLabel = pendingApprovals > 0 ? "\(pendingApprovals) approvals pending" : nil
    let closeoutReadiness = run?.closeoutReadinessSummary.map {
      P077CloseoutReadinessPresenter.presentation(for: $0)
    }
    let implementationCompletion = P088ImplementationCompletionPresenter.presentationIfPresent(
      for: run?.implementationCompletion
    )
    let sideEffectReadback = P078SideEffectReadbackPresenter.presentationIfPresent(
      for: run?.sideEffectReadback
    )
    let continuationReadback = P086ContinuationReadbackPresenter.presentationIfPresent(
      records: detail.continuations,
      metrics: detail.continuationMetricsSummary
    )
    let emptyStateTitle: String?
    switch run {
    case .some:
      emptyStateTitle = nil
    case .none:
      emptyStateTitle = "Run unavailable"
    }

    return P031RunDetailPresentation(
      title: title,
      workflowLabel: workflowLabel,
      statusLabel: statusLabel,
      progressLabel: progressLabel,
      pendingApprovalsLabel: pendingApprovalsLabel,
      rolloutDecisionSummary: run?.rolloutDecisionSummary,
      ideaContext: P031IdeaContextPresenter.presentation(for: detail.idea, fallbackRun: run),
      stageTransitions: stageTransitions,
      stageTopology: stageTopology,
      approvalRows: approvalRows,
      artifactRows: artifactRows,
      artifactViewerRows: artifactViewerRows,
      reportRows: reportRows,
      activeAgentTimelineEntries: activeAgentTimelineEntries,
      catalogContext: run.map(P031CatalogContextPresenter.presentation),
      closeoutReadiness: closeoutReadiness,
      implementationCompletion: implementationCompletion,
      sideEffectReadback: sideEffectReadback,
      continuationReadback: continuationReadback,
      freshness: P031ThinPresentationFormatting.freshnessSnapshot(
        currentFreshness: currentFreshness,
        checkedAt: checkedAt,
        states: detail.freshnessStates
      ),
      refreshFeedbackText: P031ReadRefreshPresenter.feedbackText(for: .runDetail),
      emptyStateTitle: emptyStateTitle,
      errorDescription: nil,
      rawStatus: run?.status ?? "unavailable",
      failedStages: run?.failedStages ?? 0
    )
  }

  nonisolated private static func activeAgentTimelineEntries(
    from executions: [P031ActiveAgentExecutionReadModel],
    stageByExecutionID: [String: P031StageReadModel]
  ) -> [P031ActiveAgentTimelinePresentation] {
    executions.compactMap { execution in
      guard execution.status.trimmingCharacters(in: .whitespacesAndNewlines).lowercased() == "running",
        execution.completedAt == nil,
        let startedAt = P031ReadBoundaryDateParser.date(from: execution.startedAt)
      else {
        return nil
      }

      let stage = stageByExecutionID[execution.stageExecutionID]
      let providerModel = [execution.provider, execution.model]
        .compactMap { value -> String? in
          let trimmed = value?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
          return trimmed.isEmpty ? nil : trimmed
        }
        .joined(separator: " ")
      let stageLabel = execution.stageLabel?.trimmingCharacters(in: .whitespacesAndNewlines)
        ?? stage?.label.trimmingCharacters(in: .whitespacesAndNewlines)
      let detail = [
        providerModel.isEmpty ? nil : "Running via \(providerModel)",
        stageLabel?.isEmpty == false ? "Active in \(stageLabel!)" : nil,
      ]
        .compactMap { $0 }
        .joined(separator: " · ")
      let derivedTitle = execution.agentID
        .replacingOccurrences(of: "_", with: " ")
        .split(separator: " ")
        .map { $0.prefix(1).uppercased() + $0.dropFirst() }
        .joined(separator: " ")
      let title = execution.agentTitle?.trimmingCharacters(in: .whitespacesAndNewlines)
      let timestamp = P031ReadBoundaryDateParser.date(from: execution.lastEventAt) ?? startedAt

      return P031ActiveAgentTimelinePresentation(
        id: "active-agent-\(execution.id)",
        title: title?.isEmpty == false ? title! : (derivedTitle.isEmpty ? execution.agentID : derivedTitle),
        detail: detail.isEmpty ? "Agent is running" : detail,
        timestamp: timestamp,
        stageID: stage?.stageID,
        providerID: execution.provider,
        stageLabel: stageLabel?.isEmpty == false ? stageLabel : nil,
        taskLabel: execution.taskLabel,
        status: execution.status,
        eventCount: execution.eventCount,
        selectionOrder: execution.selectionOrder,
        selectionUnavailableReason: execution.selectionUnavailableReason,
        agentID: execution.agentID,
        sessionID: execution.sessionGenerationID ?? execution.sessionLineageID
      )
    }
  }

  nonisolated static func errorPresentation(
    error: Error,
    currentFreshness: P031FreshnessSnapshot,
    checkedAt: Date
  ) -> P031RunDetailPresentation {
    P031RunDetailPresentation(
      title: "Run unavailable",
      workflowLabel: nil,
      statusLabel: "Unavailable",
      progressLabel: nil,
      pendingApprovalsLabel: nil,
      rolloutDecisionSummary: nil,
      ideaContext: nil,
      stageTransitions: [],
      approvalRows: [],
      artifactRows: [],
      artifactViewerRows: [],
      reportRows: [],
      catalogContext: nil,
      closeoutReadiness: nil,
      implementationCompletion: nil,
      sideEffectReadback: nil,
      continuationReadback: nil,
      freshness: WorkflowFreshnessReducer.reduce(
        currentFreshness,
        event: .refreshFailed(checkedAt: checkedAt, reason: P031ReadErrorPresenter.description(for: error))
      ),
      refreshFeedbackText: P031ReadRefreshPresenter.feedbackText(for: .runDetail),
      emptyStateTitle: nil,
      errorDescription: P031ReadErrorPresenter.description(for: error),
      rawStatus: "unavailable",
      failedStages: 0
    )
  }

  private nonisolated static func stageExecutionWindowsByStageID(
    _ stages: [P031StageReadModel]
  ) -> [String: [P031StageExecutionWindow]] {
    let stagesWithStart = stages.compactMap { stage -> (stage: P031StageReadModel, startedAt: Date)? in
      guard let startedAt = P031ReadBoundaryDateParser.date(from: stage.startedAt) else {
        return nil
      }
      return (stage, startedAt)
    }
    let grouped = Dictionary(grouping: stagesWithStart, by: { $0.stage.stageID })
    return grouped.mapValues { entries in
      let sorted = entries.sorted {
        if $0.startedAt != $1.startedAt {
          return $0.startedAt < $1.startedAt
        }
        return $0.stage.isEarlierExecution(than: $1.stage)
      }
      return sorted.enumerated().map { index, entry in
        P031StageExecutionWindow(
          stage: entry.stage,
          startedAt: entry.startedAt,
          nextStartedAt: sorted.indices.contains(index + 1) ? sorted[index + 1].startedAt : nil
        )
      }
    }
  }

  private nonisolated static func stageExecution(
    for artifact: P031ArtifactReadModel,
    windowsByStageID: [String: [P031StageExecutionWindow]]
  ) -> P031StageReadModel? {
    guard let createdAt = P031ReadBoundaryDateParser.date(from: artifact.createdAt),
      let windows = windowsByStageID[artifact.stageID]
    else {
      return nil
    }
    return windows.last(where: { $0.contains(createdAt) })?.stage
  }
}

private struct P031StageExecutionWindow: Equatable, Sendable {
  let stage: P031StageReadModel
  let startedAt: Date
  let nextStartedAt: Date?

  nonisolated func contains(_ date: Date) -> Bool {
    guard date >= startedAt else {
      return false
    }
    if let nextStartedAt {
      return date < nextStartedAt
    }
    return true
  }
}

private enum P031ReadBoundaryDateParser {
  nonisolated private static let fractionalFormatterKey = "chainworks.p031.readBoundaryDateParser.fractional"
  nonisolated private static let standardFormatterKey = "chainworks.p031.readBoundaryDateParser.standard"

  nonisolated static func date(from value: String?) -> Date? {
    guard let value, !value.isEmpty else {
      return nil
    }
    if let date = formatter(for: fractionalFormatterKey, options: [.withInternetDateTime, .withFractionalSeconds])
      .date(from: value)
    {
      return date
    }
    return formatter(for: standardFormatterKey, options: [.withInternetDateTime]).date(from: value)
  }

  nonisolated private static func formatter(
    for key: String,
    options: ISO8601DateFormatter.Options
  ) -> ISO8601DateFormatter {
    if let cached = Thread.current.threadDictionary[key] as? ISO8601DateFormatter {
      return cached
    }
    let formatter = ISO8601DateFormatter()
    formatter.formatOptions = options
    Thread.current.threadDictionary[key] = formatter
    return formatter
  }
}

private extension P031StageReadModel {
  nonisolated func isEarlierExecution(than other: P031StageReadModel) -> Bool {
    let lhsIteration = iteration ?? Int.min
    let rhsIteration = other.iteration ?? Int.min
    if lhsIteration != rhsIteration {
      return lhsIteration < rhsIteration
    }
    let lhsAttempt = attemptNumber ?? Int.min
    let rhsAttempt = other.attemptNumber ?? Int.min
    if lhsAttempt != rhsAttempt {
      return lhsAttempt < rhsAttempt
    }
    return id.localizedStandardCompare(other.id) == .orderedAscending
  }
}

enum P031StageDetailPresenter {
  nonisolated static func presentation(
    for detail: P031StageDetailReadModel,
    currentFreshness: P031FreshnessSnapshot,
    checkedAt: Date
  ) -> P031StageDetailPresentation {
    let emptyStateTitle: String?
    switch detail.stage {
    case .some:
      emptyStateTitle = nil
    case .none:
      emptyStateTitle = "Stage unavailable"
    }

    return P031StageDetailPresentation(
      stage: detail.stage.map { P031StagePresenter.presentation(for: $0) },
      freshness: P031ThinPresentationFormatting.freshnessSnapshot(
        currentFreshness: currentFreshness,
        checkedAt: checkedAt,
        states: detail.freshnessStates
      ),
      refreshFeedbackText: P031ReadRefreshPresenter.feedbackText(for: .stageDetail),
      emptyStateTitle: emptyStateTitle,
      errorDescription: nil
    )
  }

  nonisolated static func errorPresentation(
    error: Error,
    currentFreshness: P031FreshnessSnapshot,
    checkedAt: Date
  ) -> P031StageDetailPresentation {
    P031StageDetailPresentation(
      stage: nil,
      freshness: WorkflowFreshnessReducer.reduce(
        currentFreshness,
        event: .refreshFailed(checkedAt: checkedAt, reason: P031ReadErrorPresenter.description(for: error))
      ),
      refreshFeedbackText: P031ReadRefreshPresenter.feedbackText(for: .stageDetail),
      emptyStateTitle: nil,
      errorDescription: P031ReadErrorPresenter.description(for: error)
    )
  }
}

enum P031StagePresenter {
  nonisolated static func presentation(
    for stage: P031StageReadModel,
    now: Date = Date()
  )
    -> P031StageSummaryPresentation
  {
    let iterationLabel: String?
    if let iteration = stage.iteration, let attemptNumber = stage.attemptNumber {
      iterationLabel = "Iteration \(iteration), attempt \(attemptNumber)"
    } else if let iteration = stage.iteration {
      iterationLabel = "Iteration \(iteration)"
    } else {
      iterationLabel = nil
    }
    let badgeLabels = [
      stage.hasPendingApproval == true ? "Approval pending" : nil,
      stage.hasValidationFailure == true ? "Validation failure" : nil,
      stage.hasArtifacts == true ? "Artifacts" : nil,
      stage.projectionLag ? "Projection lag" : nil,
    ].compactMap { $0 }
    let statusLabel = P031ThinPresentationFormatting.titleCase(stage.status)
    let startedAt = P031ReadBoundaryDateParser.date(from: stage.startedAt)
    let completedAt = P031ReadBoundaryDateParser.date(from: stage.completedAt)
    let startedLabel = startedAt.map { "Started: \(P031ThinPresentationFormatting.timestamp($0))" }
    let completedLabel = completedAt.map {
      "Completed: \(P031ThinPresentationFormatting.timestamp($0))"
    }
    let durationLabel = P031ThinPresentationFormatting.durationLabel(
      startedAt: startedAt,
      completedAt: completedAt,
      now: now
    )
    var accessibilityParts = [stage.label, statusLabel]
    if let iterationLabel {
      accessibilityParts.append(iterationLabel)
    }
    accessibilityParts.append(contentsOf: [startedLabel, completedLabel, durationLabel].compactMap { $0 })
    accessibilityParts.append(contentsOf: badgeLabels)

    return P031StageSummaryPresentation(
      stageExecutionID: stage.id,
      title: stage.label,
      statusLabel: statusLabel,
      iterationLabel: iterationLabel,
      startedLabel: startedLabel,
      completedLabel: completedLabel,
      durationLabel: durationLabel,
      badgeLabels: badgeLabels,
      freshnessState: stage.freshnessState,
      accessibilityLabel: accessibilityParts.joined(separator: ", ")
    )
  }
}

enum P031IdeaContextPresenter {
  nonisolated static func presentation(
    for idea: P031IdeaReadModel?,
    fallbackRun run: P031RunRowReadModel?
  ) -> P031IdeaContextPresentation? {
    guard let ideaID = idea?.id ?? run?.ideaID else {
      return nil
    }
    let rawTitle = idea?.title ?? run?.ideaTitle ?? "Idea \(ideaID)"
    let title = rawTitle.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
      ? "Idea \(ideaID)"
      : rawTitle
    let statusLabel = idea?.status.map(P031ThinPresentationFormatting.titleCase)
    let projectKey = idea?.projectKey ?? run?.projectKey
    let accessibilityParts = [
      title,
      statusLabel,
      projectKey.map { "Project \($0)" },
    ].compactMap { $0 }
    return P031IdeaContextPresentation(
      id: ideaID,
      title: title,
      statusLabel: statusLabel,
      projectKey: projectKey,
      body: idea?.body,
      createdAt: idea?.createdAt,
      archivedAt: idea?.archivedAt,
      accessibilityLabel: accessibilityParts.joined(separator: ", ")
    )
  }
}

enum P031StageTransitionPresenter {
  nonisolated static func presentation(
    for stage: P031StageReadModel,
    now: Date = Date()
  )
    -> P031StageTransitionPresentation
  {
    let statusText = P031ThinPresentationFormatting.titleCase(stage.status)
    let attemptText: String?
    if let iteration = stage.iteration, let attempt = stage.attemptNumber {
      attemptText = "Iteration \(iteration), attempt \(attempt)"
    } else if let iteration = stage.iteration {
      attemptText = "Iteration \(iteration)"
    } else {
      attemptText = nil
    }
    let startedAt = P031ReadBoundaryDateParser.date(from: stage.startedAt)
    let completedAt = P031ReadBoundaryDateParser.date(from: stage.completedAt)
    let startedLabel = startedAt.map { "Started: \(P031ThinPresentationFormatting.timestamp($0))" }
    let completedLabel = completedAt.map {
      "Completed: \(P031ThinPresentationFormatting.timestamp($0))"
    }
    let durationLabel = P031ThinPresentationFormatting.durationLabel(
      startedAt: startedAt,
      completedAt: completedAt,
      now: now
    )
    let evidenceLabels = [
      stage.hasArtifacts == true ? "Artifacts" : nil,
      stage.hasPendingApproval == true ? "Approval" : nil,
      stage.hasValidationFailure == true ? "Validation" : nil,
      stage.settlementKind.map(P031ThinPresentationFormatting.titleCase),
    ].compactMap { $0 }
    let accessibilityParts = [
      stage.label,
      statusText,
      attemptText,
      startedLabel,
      completedLabel,
      durationLabel,
    ].compactMap { $0 }
      + evidenceLabels
    return P031StageTransitionPresentation(
      stageExecutionID: stage.id,
      stageTitle: stage.label,
      statusText: statusText,
      attemptText: attemptText,
      startedLabel: startedLabel,
      completedLabel: completedLabel,
      durationLabel: durationLabel,
      connectorState: connectorState(for: stage),
      evidenceLabels: P031ThinPresentationFormatting.uniqueLabels(evidenceLabels),
      accessibilityLabel: accessibilityParts.joined(separator: ", ")
    )
  }

  nonisolated private static func connectorState(for stage: P031StageReadModel)
    -> P031StageConnectorState
  {
    let status = stage.status.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
    if stage.projectionLag || !stage.projectionPresent {
      return .unavailable
    }
    if status.contains("complete") || status == "succeeded" || status == "approved" {
      return .completed
    }
    if status.contains("block") || status.contains("fail") || stage.hasValidationFailure == true {
      return .blocked
    }
    if status.contains("running") || status.contains("active") || status.contains("in_progress") {
      return .running
    }
    if status.contains("pending") || status.contains("waiting") || stage.hasPendingApproval == true {
      return .pending
    }
    // P036: unknown/unrecognized statuses must not infer runtime state locally.
    return .unavailable
  }
}

enum P031StageTopologyPresenter {
  nonisolated static func presentation(
    for node: P031RunStageTopologyReadModel
  ) -> P031StageTopologyPresentation {
    let iterationText: String? = node.iteration.map { "Iteration \($0)" }
    let attemptText: String? = node.attemptNumber.map { "Attempt \($0)" }
    return P031StageTopologyPresentation(
      stageID: node.stageID,
      ordinal: node.order,
      title: node.label,
      ownerAgentID: node.ownerAgentID,
      ownerAgentTitle: node.ownerAgentTitle,
      status: node.status,
      statusText: P031ThinPresentationFormatting.titleCase(node.status),
      isCurrent: node.isCurrent,
      iterationText: iterationText,
      attemptText: attemptText,
      approvalRequired: node.approvalRequired,
      artifactCount: node.artifactCount,
      communicationCount: node.communicationCount,
      occurrences: node.occurrences.map { occurrence in
        let providerParts = [
          occurrence.provider,
          occurrence.model,
          occurrence.effort,
        ].compactMap { $0?.trimmingCharacters(in: .whitespacesAndNewlines) }
          .filter { !$0.isEmpty }
        return P031StageTopologyOccurrencePresentation(
          agentID: occurrence.agentID,
          agentTitle: occurrence.agentTitle,
          taskName: occurrence.taskName,
          statusText: P031ThinPresentationFormatting.titleCase(occurrence.status),
          providerLabel: providerParts.joined(separator: " · "),
          executionCountLabel: occurrence.executionCount > 0
            ? "\(occurrence.executionCount) attempt\(occurrence.executionCount == 1 ? "" : "s")"
            : nil
        )
      },
      transitions: node.transitions.map { transition in
        P031StageTopologyTransitionPresentation(
          toStageID: transition.toStageID,
          toLabel: transition.toLabel ?? transition.toStageID,
          detail: transition.detail
        )
      }
    )
  }
}

enum P031ArtifactViewerPresenter {
  nonisolated static func presentation(
    for artifact: P031ArtifactReadModel,
    stage: P031StageReadModel? = nil
  )
    -> P031ArtifactViewerPresentation
  {
    let normalizedFormat = artifact.format.trimmingCharacters(in: .whitespacesAndNewlines)
      .lowercased()
    let payloadText = artifact.payloadText?.trimmingCharacters(in: .whitespacesAndNewlines)
    let hasPayload = payloadText?.isEmpty == false
    let renderMode: P031ArtifactRenderMode
    let preparedPreview: ArtifactPreparedPreview?
    switch artifact.payloadAvailabilityState {
    case .metadataOnly, .payloadDeferred:
      // Fail closed: the server declared these states as non-available payloads.
      // Rendering payloadText here would let an inconsistent server detail payload
      // silently override the declared non-available state. A distinct authorized
      // partial-preview state is required to render content.
      renderMode = .metadataOnly
      preparedPreview = nil
    case .generating, .unavailable:
      renderMode = .unavailable
      preparedPreview = nil
    case .available:
      if let payloadText, hasPayload {
        let preview = makePreparedPreview(
          forPayload: payloadText,
          normalizedFormat: normalizedFormat,
          artifactName: artifact.name
        )
        renderMode = Self.renderMode(forIntent: preview.intent)
        preparedPreview = preview
      } else {
        renderMode = .unavailable
        preparedPreview = nil
      }
    }
    let subtitle = [
      artifact.contractID,
      artifact.format,
      artifact.agentID,
    ].compactMap { $0?.trimmingCharacters(in: .whitespacesAndNewlines) }
      .filter { !$0.isEmpty }
      .joined(separator: " / ")
    // SEC-001: serverDebugDetail must only surface when the P085 diagnostic affordance
    // is available and the payload is not in an unauthorized state. Routing through
    // diagnosticAffordance ensures freshnessState == .unauthorized suppresses detail.
    // The additional notAuthorized reason-code check prevents payload auth failures
    // from leaking operator/server debug strings.
    let freshnessStateForDiag = P085FreshnessState(artifact.freshnessState)
    let diagnostic = P085AffordancePresenter.diagnosticAffordance(
      diagnosticID: artifact.diagnosticID,
      serverDebugDetail: artifact.serverDebugDetail,
      freshnessState: freshnessStateForDiag
    )
    let payloadIsAuthorized = artifact.payloadUnavailableReasonCode != .notAuthorized
    let guardedDebugDetail =
      (diagnostic.isAvailable && payloadIsAuthorized) ? diagnostic.serverDebugDetail : nil
    let payloadUnavailableReason =
      guardedDebugDetail
      ?? artifact.payloadUnavailableReasonCode?.rawValue
      ?? (hasPayload ? nil : "Payload content is not exposed through GraphQL")
    let accessibilityParts = [
      artifact.name,
      subtitle,
      P031ThinPresentationFormatting.titleCase(artifact.payloadAvailabilityState.rawValue),
      payloadUnavailableReason,
    ].compactMap { $0 }.filter { !$0.isEmpty }
    return P031ArtifactViewerPresentation(
      artifactID: artifact.id,
      stageID: artifact.stageID,
      stageExecutionID: stage?.id ?? artifact.sourceStageExecutionID,
      stageLabel: stage?.label,
      iteration: stage?.iteration,
      attemptNumber: stage?.attemptNumber,
      agentID: artifact.agentID,
      contractID: artifact.contractID,
      format: artifact.format,
      title: artifact.name,
      subtitle: subtitle,
      renderMode: renderMode,
      payloadState: artifact.payloadAvailabilityState,
      preparedPreview: preparedPreview,
      unavailableReason: unavailableReason(for: renderMode, reason: payloadUnavailableReason),
      freshnessState: artifact.freshnessState,
      accessibilityLabel: accessibilityParts.joined(separator: ", ")
    )
  }

  nonisolated private static func unavailableReason(
    for renderMode: P031ArtifactRenderMode,
    reason: String?
  ) -> String? {
    switch renderMode {
    case .metadataOnly, .unavailable:
      return reason
    case .markdown, .json, .diff, .plainText:
      return nil
    }
  }

  nonisolated private static func makePreparedPreview(
    forPayload payloadText: String,
    normalizedFormat: String,
    artifactName: String
  ) -> ArtifactPreparedPreview {
    let context = ArtifactRenderContext.explicitNamed(
      format: declaredArtifactFormat(normalizedFormat),
      artifactName: artifactName
    )
    let intent = ArtifactPresentationIntent.resolve(content: payloadText, context: context)
    return ArtifactPreviewPolicy.prepare(content: payloadText, intent: intent)
  }

  nonisolated private static func declaredArtifactFormat(_ normalizedFormat: String) -> ArtifactFormat {
    switch normalizedFormat {
    case "markdown", "md":
      return .markdown
    case "diff", "patch":
      return .diff
    case "report":
      return .report
    default:
      return .json
    }
  }

  nonisolated private static func renderMode(forIntent intent: ArtifactPresentationIntent)
    -> P031ArtifactRenderMode
  {
    switch intent {
    case .markdownDocument:
      return .markdown
    case .jsonTree:
      return .json
    case .diff:
      return .diff
    case .plainText:
      return .plainText
    }
  }
}

enum P031CatalogContextPresenter {
  nonisolated static func presentation(for run: P031RunRowReadModel)
    -> P031CatalogContextPresentation
  {
    let workflowTitle = run.workflowTitle.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
      ? "Workflow unavailable"
      : run.workflowTitle
    let statusText: String
    if run.workflowSnapshotHash == nil && run.catalogSnapshotHash == nil {
      statusText = "Catalog snapshot content unavailable through GraphQL"
    } else {
      statusText = "Snapshot metadata available"
    }
    let accessibilityParts = [
      run.workflowID,
      workflowTitle,
      run.workflowSnapshotHash.map { "Workflow snapshot \($0)" },
      run.catalogSnapshotHash.map { "Catalog snapshot \($0)" },
      statusText,
    ].compactMap { $0 }
    return P031CatalogContextPresentation(
      workflowID: run.workflowID,
      workflowTitle: workflowTitle,
      workflowSnapshotHash: run.workflowSnapshotHash,
      catalogSnapshotHash: run.catalogSnapshotHash,
      statusText: statusText,
      accessibilityLabel: accessibilityParts.joined(separator: ", ")
    )
  }
}

enum P031ArtifactPresenter {
  nonisolated static func presentation(for artifact: P031ArtifactReadModel)
    -> P031ArtifactSummaryPresentation
  {
    let payload = PayloadUnavailableReasonPresenter.presentation(
      for: P031ReportMetadataReadModel(
        id: artifact.id,
        name: artifact.name,
        format: artifact.format,
        reportKind: artifact.reportKind,
        reportVersion: artifact.reportVersion,
        freshnessState: artifact.freshnessState,
        payloadAvailabilityState: artifact.payloadAvailabilityState,
        payloadUnavailableReasonCode: artifact.payloadUnavailableReasonCode,
        diagnosticID: artifact.diagnosticID,
        serverDebugDetail: artifact.serverDebugDetail
      ))
    let detailParts = [
      artifact.contractID,
      artifact.format,
      artifact.agentID,
    ].compactMap { value in
      value?.trimmingCharacters(in: .whitespacesAndNewlines)
    }.filter { !$0.isEmpty }
    let detailLabel = detailParts.joined(separator: " / ")
    // P085: use the canonical affordance presenter for the list label so payload_deferred
    // surfaces "Open to preview" rather than an Unavailable/generic fallback.
    let p085Affordance = P085AffordancePresenter.artifactListAffordance(for: artifact)
    let p085Label = p085Affordance.label
    let accessibilityParts = [
      artifact.name,
      detailLabel,
      p085Label,
      P031ThinPresentationFormatting.freshnessAccessibilityLabel(artifact.freshnessState),
    ].compactMap { $0 }.filter { !$0.isEmpty }

    return P031ArtifactSummaryPresentation(
      artifactID: artifact.id,
      title: artifact.name,
      detailLabel: detailLabel,
      payloadAvailabilityLabel: p085Label,
      payloadAvailabilitySymbolName: payload.symbolName,
      canOpenPayload: payload.canOpenPayload,
      diagnosticCopyItems: payload.copyItems,
      freshnessState: artifact.freshnessState,
      accessibilityLabel: accessibilityParts.joined(separator: ", "),
      affordance: p085Affordance
    )
  }
}

enum P031ArtifactListPresenter {
  nonisolated static func presentation(
    for artifacts: [P031ArtifactReadModel],
    currentFreshness: P031FreshnessSnapshot,
    checkedAt: Date
  ) -> P031ArtifactListPresentation {
    let rows = artifacts.map(P031ArtifactPresenter.presentation)
    return P031ArtifactListPresentation(
      rows: rows,
      freshness: P031ThinPresentationFormatting.freshnessSnapshot(
        currentFreshness: currentFreshness,
        checkedAt: checkedAt,
        states: artifacts.map(\.freshnessState)
      ),
      refreshFeedbackText: P031ReadRefreshPresenter.feedbackText(for: .artifacts),
      emptyStateTitle: rows.isEmpty ? "No artifacts" : nil,
      errorDescription: nil
    )
  }

  nonisolated static func errorPresentation(
    error: Error,
    currentFreshness: P031FreshnessSnapshot,
    checkedAt: Date
  ) -> P031ArtifactListPresentation {
    P031ArtifactListPresentation(
      rows: [],
      freshness: WorkflowFreshnessReducer.reduce(
        currentFreshness,
        event: .refreshFailed(checkedAt: checkedAt, reason: P031ReadErrorPresenter.description(for: error))
      ),
      refreshFeedbackText: P031ReadRefreshPresenter.feedbackText(for: .artifacts),
      emptyStateTitle: nil,
      errorDescription: P031ReadErrorPresenter.description(for: error)
    )
  }
}

enum P031ReportMetadataListPresenter {
  nonisolated static func presentation(
    for reports: [P031ReportMetadataReadModel],
    currentFreshness: P031FreshnessSnapshot,
    checkedAt: Date
  ) -> P031ReportMetadataListPresentation {
    let rows = reports.map(ReportMetadataRowPresenter.presentation)
    return P031ReportMetadataListPresentation(
      rows: rows,
      freshness: P031ThinPresentationFormatting.freshnessSnapshot(
        currentFreshness: currentFreshness,
        checkedAt: checkedAt,
        states: reports.map(\.freshnessState)
      ),
      refreshFeedbackText: P031ReadRefreshPresenter.feedbackText(for: .reportMetadata),
      emptyStateTitle: rows.isEmpty ? "No report metadata" : nil,
      errorDescription: nil
    )
  }

  nonisolated static func errorPresentation(
    error: Error,
    currentFreshness: P031FreshnessSnapshot,
    checkedAt: Date
  ) -> P031ReportMetadataListPresentation {
    P031ReportMetadataListPresentation(
      rows: [],
      freshness: WorkflowFreshnessReducer.reduce(
        currentFreshness,
        event: .refreshFailed(checkedAt: checkedAt, reason: P031ReadErrorPresenter.description(for: error))
      ),
      refreshFeedbackText: P031ReadRefreshPresenter.feedbackText(for: .reportMetadata),
      emptyStateTitle: nil,
      errorDescription: P031ReadErrorPresenter.description(for: error)
    )
  }
}

enum P031DaemonLifecyclePresenter {
  nonisolated static func presentation(
    for status: P031DaemonStatusReadModel,
    currentFreshness: P031FreshnessSnapshot,
    checkedAt: Date
  ) -> P031DaemonLifecyclePresentation {
    let title = "Daemon \(P031ThinPresentationFormatting.titleCase(status.state.rawValue))"
    let detailParts = [
      status.pid.map { "PID \($0)" },
      status.restartCountSinceBoot.map { "\($0) restarts since boot" },
      status.buildSHA.map { "Build \($0)" },
    ].compactMap { $0 }
    let copyItems = [
      status.buildSHA.map { P031DiagnosticCopyItem(label: "build_sha", value: $0) },
      status.pid.map { P031DiagnosticCopyItem(label: "pid", value: "\($0)") },
      status.schemaVersion.map { P031DiagnosticCopyItem(label: "schema_version", value: "\($0)") },
      status.binarySchemaVersion.map {
        P031DiagnosticCopyItem(label: "binary_schema_version", value: "\($0)")
      },
    ].compactMap { $0 }

    return P031DaemonLifecyclePresentation(
      state: status.state,
      buildSHA: status.buildSHA,
      pid: status.pid,
      uptimeSeconds: nil, // Add uptime if available in status
      title: title,
      detailLabel: detailParts.isEmpty ? nil : detailParts.joined(separator: " / "),
      badgeLabels: badgeLabels(for: status.state),
      copyItems: copyItems,
      freshness: P031ThinPresentationFormatting.freshnessSnapshot(
        currentFreshness: currentFreshness,
        checkedAt: checkedAt,
        states: [.live]
      ),
      refreshFeedbackText: P031ReadRefreshPresenter.feedbackText(for: .daemonLifecycle),
      errorDescription: nil
    )
  }

  nonisolated static func errorPresentation(
    error: Error,
    currentFreshness: P031FreshnessSnapshot,
    checkedAt: Date
  ) -> P031DaemonLifecyclePresentation {
    P031DaemonLifecyclePresentation(
      state: nil,
      buildSHA: nil,
      pid: nil,
      uptimeSeconds: nil,
      title: "Daemon unavailable",
      detailLabel: nil,
      badgeLabels: ["Unavailable"],
      copyItems: [],
      freshness: WorkflowFreshnessReducer.reduce(
        currentFreshness,
        event: .refreshFailed(checkedAt: checkedAt, reason: P031ReadErrorPresenter.description(for: error))
      ),
      refreshFeedbackText: P031ReadRefreshPresenter.feedbackText(for: .daemonLifecycle),
      errorDescription: P031ReadErrorPresenter.description(for: error)
    )
  }

  nonisolated private static func badgeLabels(for state: P031DaemonLifecycleState) -> [String] {
    switch state {
    case .ready:
      return ["Ready"]
    case .degraded:
      return ["Degraded"]
    case .failed:
      return ["Failed"]
    case .restarting:
      return ["Restarting"]
    case .starting:
      return ["Starting"]
    case .shutdown:
      return ["Shutdown"]
    case .notStarted:
      return ["Not started"]
    }
  }
}

struct P031ThinWorkflowScreenCoordinator<Store: P031WorkflowReadStore>: Sendable {
  let store: Store
  let writePathGuideState: P031ExternalWritePathGuideState
  let writePathGuideSummary: P031OperatorWritePathGuideSummaryPresentation
  let writePathGuideErrorDescription: String?

  init(
    store: Store,
    writePathGuideState: P031ExternalWritePathGuideState = .unavailable
  ) {
    self.store = store
    self.writePathGuideState = writePathGuideState
    self.writePathGuideSummary = P031OperatorWritePathGuidePresenter.unavailablePresentation()
    self.writePathGuideErrorDescription = nil
  }

  init(
    store: Store,
    writePathGuideData: Data?
  ) {
    let resolution = P031OperatorWritePathGuideResolver.resolve(from: writePathGuideData)
    self.store = store
    self.writePathGuideState = resolution.approvalResolutionState
    self.writePathGuideSummary = resolution.summaryPresentation
    self.writePathGuideErrorDescription = resolution.errorDescription
  }

  nonisolated func loadOperatorWritePathGuideSummary()
    -> P031OperatorWritePathGuideSummaryPresentation
  {
    writePathGuideSummary
  }

  nonisolated func loadRunsHome(
    currentFreshness: P031FreshnessSnapshot,
    checkedAt: Date = Date(),
    showFirstRunOrientation: Bool = false
  ) async -> P031RunsHomePresentation {
    do {
      let runs = try await store.fetchRuns()
      return P031RunsHomePresenter.presentation(
        for: runs,
        currentFreshness: currentFreshness,
        checkedAt: checkedAt,
        writePathGuideState: writePathGuideState,
        showFirstRunOrientation: showFirstRunOrientation
      )
    } catch {
      return P031RunsHomePresenter.errorPresentation(
        error: error,
        currentFreshness: currentFreshness,
        checkedAt: checkedAt,
        writePathGuideState: writePathGuideState,
        showFirstRunOrientation: showFirstRunOrientation
      )
    }
  }

  nonisolated func loadRunDetail(
    runID: String,
    currentFreshness: P031FreshnessSnapshot,
    checkedAt: Date = Date()
  ) async -> P031RunDetailPresentation {
    do {
      let detail = try await store.fetchRunDetail(runID: runID)
      let hydratedDetail: P031RunDetailReadModel
      if case nil = detail.idea, let ideaID = detail.run?.ideaID {
        let idea = try? await store.fetchIdea(id: ideaID)
        let run = detail.run.map { $0.withIdeaTitle(idea?.title) }
        hydratedDetail = P031RunDetailReadModel(
          run: run,
          idea: idea,
          stages: detail.stages,
          artifacts: detail.artifacts,
          approvalInbox: detail.approvalInbox,
          activeAgentExecutions: detail.activeAgentExecutions,
          runStageTopology: detail.runStageTopology
        )
      } else {
        hydratedDetail = detail
      }
      return P031RunDetailPresenter.presentation(
        for: hydratedDetail,
        currentFreshness: currentFreshness,
        checkedAt: checkedAt,
        writePathGuideState: writePathGuideState
      )
    } catch {
      return P031RunDetailPresenter.errorPresentation(
        error: error,
        currentFreshness: currentFreshness,
        checkedAt: checkedAt
      )
    }
  }

  nonisolated func loadStageDetail(
    stageExecutionID: String,
    currentFreshness: P031FreshnessSnapshot,
    checkedAt: Date = Date()
  ) async -> P031StageDetailPresentation {
    do {
      let detail = try await store.fetchStageDetail(stageExecutionID: stageExecutionID)
      return P031StageDetailPresenter.presentation(
        for: detail,
        currentFreshness: currentFreshness,
        checkedAt: checkedAt
      )
    } catch {
      return P031StageDetailPresenter.errorPresentation(
        error: error,
        currentFreshness: currentFreshness,
        checkedAt: checkedAt
      )
    }
  }

  nonisolated func loadApprovalInbox(
    currentFreshness: P031FreshnessSnapshot,
    checkedAt: Date = Date()
  ) async -> P031ApprovalInboxPresentation {
    do {
      let approvals = try await store.fetchApprovalInbox()
      return P031ApprovalInboxPresenter.presentation(
        for: approvals,
        currentFreshness: currentFreshness,
        checkedAt: checkedAt,
        writePathGuideState: writePathGuideState
      )
    } catch {
      return P031ApprovalInboxPresenter.errorPresentation(
        error: error,
        currentFreshness: currentFreshness,
        checkedAt: checkedAt
      )
    }
  }

  nonisolated func loadArtifacts(
    runID: String,
    currentFreshness: P031FreshnessSnapshot,
    checkedAt: Date = Date()
  ) async -> P031ArtifactListPresentation {
    do {
      let artifacts = try await store.fetchArtifacts(runID: runID)
      return P031ArtifactListPresenter.presentation(
        for: artifacts,
        currentFreshness: currentFreshness,
        checkedAt: checkedAt
      )
    } catch {
      return P031ArtifactListPresenter.errorPresentation(
        error: error,
        currentFreshness: currentFreshness,
        checkedAt: checkedAt
      )
    }
  }

  nonisolated func loadArtifactPreview(artifactID: String) async -> P031ArtifactViewerPresentation? {
    ForgeLogger.ui.info("P031 artifact preview load start artifactID=\(artifactID)")
    do {
      let artifact = try await store.fetchArtifactPayload(artifactID: artifactID)
      let presentation = P031ArtifactViewerPresenter.presentation(for: artifact)
      ForgeLogger.ui.info(
        "P031 artifact preview load success artifactID=\(artifactID) payloadState=\(presentation.payloadState.rawValue) renderMode=\(String(describing: presentation.renderMode)) hasPreview=\((presentation.preparedPreview != nil)) reason=\(presentation.unavailableReason ?? "nil")"
      )
      return presentation
    } catch {
      ForgeLogger.ui.error(
        "P031 artifact preview load failed artifactID=\(artifactID) error=\(String(describing: error))"
      )
      return nil
    }
  }

  nonisolated func resolveTimelineRawDetail(handle: String) async -> P031TimelineRawDetailReadModel {
    do {
      return try await store.fetchTimelineRawDetail(handle: handle)
    } catch {
      return P031TimelineRawDetailReadModel(
        status: .unavailable,
        rawDetail: nil,
        rawDetailBytes: nil,
        rawDetailDigest: nil,
        errorReason: .storageUnavailable
      )
    }
  }

  nonisolated func loadReportMetadata(
    runID: String,
    currentFreshness: P031FreshnessSnapshot,
    checkedAt: Date = Date()
  ) async -> P031ReportMetadataListPresentation {
    do {
      let reports = try await store.fetchReportMetadata(runID: runID)
      return P031ReportMetadataListPresenter.presentation(
        for: reports,
        currentFreshness: currentFreshness,
        checkedAt: checkedAt
      )
    } catch {
      return P031ReportMetadataListPresenter.errorPresentation(
        error: error,
        currentFreshness: currentFreshness,
        checkedAt: checkedAt
      )
    }
  }

  nonisolated func loadDaemonLifecycle(
    currentFreshness: P031FreshnessSnapshot,
    checkedAt: Date = Date()
  ) async -> P031DaemonLifecyclePresentation {
    do {
      let status = try await store.fetchDaemonStatus()
      return P031DaemonLifecyclePresenter.presentation(
        for: status,
        currentFreshness: currentFreshness,
        checkedAt: checkedAt
      )
    } catch {
      return P031DaemonLifecyclePresenter.errorPresentation(
        error: error,
        currentFreshness: currentFreshness,
        checkedAt: checkedAt
      )
    }
  }
}

struct P031ThinWorkflowSubscriptionCoordinator<Store: P031WorkflowReadStore>: Sendable {
  let store: Store

  nonisolated func runStatusPresentations(
    runID: String,
    currentFreshness: P031FreshnessSnapshot,
    checkedAt: Date = Date()
  ) throws -> AsyncThrowingStream<P031RunStatusSubscriptionPresentation, Error> {
    let stream = try store.subscribeToRunStatus(runID: runID)
    return stream.map { event in
      P031RunStatusSubscriptionPresenter.presentation(
        for: event,
        currentFreshness: currentFreshness,
        checkedAt: checkedAt
      )
    }
  }

  nonisolated func runtimeTimelinePresentations(
    runID: String
  ) throws -> AsyncThrowingStream<P031RuntimeTimelineEventPresentation, Error> {
    let stream = try store.subscribeToRuntimeTimeline(runID: runID)
    return stream.compactMap { event in
      P031RuntimeTimelineEventPresenter.presentation(for: event)
    }
  }

  nonisolated func daemonLifecyclePresentations(
    currentFreshness: P031FreshnessSnapshot,
    checkedAt: Date = Date()
  ) throws -> AsyncThrowingStream<P031DaemonLifecyclePresentation, Error> {
    let stream = try store.subscribeToDaemonStatus()
    return stream.map { status in
      P031DaemonLifecyclePresenter.presentation(
        for: status,
        currentFreshness: currentFreshness,
        checkedAt: checkedAt
      )
    }
  }
}

private enum P031ThinPresentationFormatting {
  nonisolated private static let timestampFormatter: DateFormatter = {
    let formatter = DateFormatter()
    formatter.calendar = Calendar(identifier: .gregorian)
    formatter.locale = Locale(identifier: "en_US_POSIX")
    formatter.timeZone = TimeZone(secondsFromGMT: 0)
    formatter.dateFormat = "yyyy-MM-dd HH:mm"
    return formatter
  }()

  nonisolated static func freshnessSnapshot(
    currentFreshness: P031FreshnessSnapshot,
    checkedAt: Date,
    states: [P031FreshnessState]
  ) -> P031FreshnessSnapshot {
    guard let state = P031FreshnessAggregator.mostConservativeState(from: states) else {
      return WorkflowFreshnessReducer.reduce(
        currentFreshness,
        event: .refreshCompletedWithoutNewProjection(
          checkedAt: checkedAt,
          reason: "No newer projection returned"
        )
      )
    }
    return WorkflowFreshnessReducer.reduce(
      currentFreshness,
      event: .serverStateReceived(state, checkedAt: checkedAt, reason: nil)
    )
  }

  nonisolated static func titleCase(_ raw: String) -> String {
    raw
      .replacingOccurrences(of: "_", with: " ")
      .split(separator: " ")
      .map { word in
        word.prefix(1).uppercased() + word.dropFirst().lowercased()
      }
      .joined(separator: " ")
  }

  nonisolated static func freshnessAccessibilityLabel(_ state: P031FreshnessState) -> String? {
    switch state {
    case .live:
      return nil
    case .refreshing:
      return "Refreshing"
    case .stale:
      return "Stale read"
    case .projectionLag:
      return "Projection lag"
    case .unauthorized:
      return "Unauthorized"
    case .unavailable:
      return "Unavailable"
    }
  }

  nonisolated static func uniqueLabels(_ labels: [String]) -> [String] {
    var seen: Set<String> = []
    return labels.filter { label in
      seen.insert(label).inserted
    }
  }

  nonisolated static func timestamp(_ date: Date) -> String {
    timestampFormatter.string(from: date)
  }

  nonisolated static func durationLabel(
    startedAt: Date?,
    completedAt: Date?,
    now: Date
  ) -> String? {
    guard let startedAt else {
      return nil
    }
    let end = completedAt ?? now
    guard end >= startedAt else {
      return nil
    }
    return "Duration: \(duration(end.timeIntervalSince(startedAt)))"
  }

  nonisolated private static func duration(_ interval: TimeInterval) -> String {
    let totalSeconds = Int(interval.rounded(.down))
    let hours = totalSeconds / 3600
    let minutes = (totalSeconds % 3600) / 60
    let seconds = totalSeconds % 60

    if hours > 0 {
      return "\(hours)h \(minutes)m \(seconds)s"
    }
    if minutes > 0 {
      return "\(minutes)m \(seconds)s"
    }
    return "\(seconds)s"
  }
}
