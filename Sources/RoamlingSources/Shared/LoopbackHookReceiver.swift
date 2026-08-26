// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

import Foundation
import Network
import RoamlingCore

public enum ActivityReceiverState: Equatable, Sendable {
    case stopped
    case starting
    case ready
    case failed(String)
}

final class LoopbackHookReceiver: @unchecked Sendable {
    typealias Normalizer = @Sendable (Data, TimeInterval) throws -> CompanionEvent?

    var state: ActivityReceiverState {
        lock.withLock { receiverState }
    }

    private static let maximumRequestBytes = 256 * 1_024
    private let token: String
    private let tokenHeader: String
    private let path: String
    private let port: NWEndpoint.Port
    private let clock: @Sendable () -> TimeInterval
    private let normalizer: Normalizer
    private let queue: DispatchQueue
    private let lock = NSLock()
    private let stream: AsyncStream<CompanionEvent>
    private let continuation: AsyncStream<CompanionEvent>.Continuation
    private var receiverState = ActivityReceiverState.stopped
    private var listener: NWListener?

    init(
        label: String,
        token: String,
        tokenHeader: String,
        path: String,
        port: UInt16,
        clock: @escaping @Sendable () -> TimeInterval,
        normalizer: @escaping Normalizer
    ) {
        self.token = token
        self.tokenHeader = tokenHeader.lowercased()
        self.path = path
        self.port = NWEndpoint.Port(rawValue: port)!
        self.clock = clock
        self.normalizer = normalizer
        queue = DispatchQueue(label: label, qos: .utility)
        let pair = AsyncStream<CompanionEvent>.makeStream()
        stream = pair.stream
        continuation = pair.continuation
    }

    deinit {
        stop()
        continuation.finish()
    }

    func makeEventStream() -> AsyncStream<CompanionEvent> { stream }

    func start() throws {
        guard lock.withLock({ self.listener == nil }) else { return }
        let parameters = NWParameters.tcp
        parameters.allowLocalEndpointReuse = true
        parameters.requiredLocalEndpoint = .hostPort(host: "127.0.0.1", port: port)
        let listener = try NWListener(using: parameters)
        lock.withLock {
            self.listener = listener
            receiverState = .starting
        }
        listener.stateUpdateHandler = { [weak self] state in
            self?.handleListenerState(state)
        }
        listener.newConnectionHandler = { [weak self] connection in
            self?.accept(connection)
        }
        listener.start(queue: queue)
    }

    func stop() {
        let listener = lock.withLock { () -> NWListener? in
            let current = self.listener
            self.listener = nil
            receiverState = .stopped
            return current
        }
        listener?.cancel()
    }

    private func handleListenerState(_ state: NWListener.State) {
        switch state {
        case .ready:
            lock.withLock { receiverState = .ready }
        case let .failed(error):
            lock.withLock {
                receiverState = .failed(error.localizedDescription)
                listener = nil
            }
        case .cancelled:
            lock.withLock {
                if listener == nil { receiverState = .stopped }
            }
        default:
            break
        }
    }

    private func accept(_ connection: NWConnection) {
        connection.stateUpdateHandler = { [weak self, weak connection] state in
            guard let self, let connection else { return }
            switch state {
            case .ready:
                receive(on: connection, buffer: Data())
            case .failed, .cancelled:
                connection.cancel()
            default:
                break
            }
        }
        connection.start(queue: queue)
    }

    private func receive(on connection: NWConnection, buffer: Data) {
        connection.receive(
            minimumIncompleteLength: 1,
            maximumLength: 64 * 1_024
        ) { [weak self] data, _, complete, error in
            guard let self else {
                connection.cancel()
                return
            }
            var requestData = buffer
            if let data { requestData.append(data) }
            guard requestData.count <= Self.maximumRequestBytes else {
                respond(status: 413, phrase: "Payload Too Large", on: connection)
                return
            }
            switch parse(requestData) {
            case let .complete(request):
                handle(request, on: connection)
            case .incomplete where error == nil && !complete:
                receive(on: connection, buffer: requestData)
            case .incomplete, .invalid:
                respond(status: 400, phrase: "Bad Request", on: connection)
            }
        }
    }

    private func handle(_ request: HTTPRequest, on connection: NWConnection) {
        guard request.method == "POST", request.path == path else {
            respond(status: 404, phrase: "Not Found", on: connection)
            return
        }
        guard request.headers[tokenHeader] == token else {
            respond(status: 401, phrase: "Unauthorized", on: connection)
            return
        }
        do {
            if let event = try normalizer(request.body, clock()) {
                continuation.yield(event)
            }
            respond(status: 204, phrase: "No Content", on: connection)
        } catch {
            respond(status: 400, phrase: "Bad Request", on: connection)
        }
    }

    private func respond(status: Int, phrase: String, on connection: NWConnection) {
        let response = "HTTP/1.1 \(status) \(phrase)\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
        connection.send(
            content: response.data(using: .utf8),
            completion: .contentProcessed { _ in connection.cancel() }
        )
    }

    private enum ParseResult {
        case incomplete
        case invalid
        case complete(HTTPRequest)
    }

    private struct HTTPRequest {
        let method: String
        let path: String
        let headers: [String: String]
        let body: Data
    }

    private func parse(_ data: Data) -> ParseResult {
        let separator = Data([13, 10, 13, 10])
        guard let headerRange = data.range(of: separator) else { return .incomplete }
        guard let headerText = String(data: data[..<headerRange.lowerBound], encoding: .utf8) else {
            return .invalid
        }
        let lines = headerText.components(separatedBy: "\r\n")
        guard let requestLine = lines.first else { return .invalid }
        let requestParts = requestLine.split(separator: " ")
        guard requestParts.count == 3 else { return .invalid }

        var headers: [String: String] = [:]
        for line in lines.dropFirst() {
            guard let colon = line.firstIndex(of: ":") else { return .invalid }
            let name = line[..<colon].trimmingCharacters(in: .whitespaces).lowercased()
            let value = line[line.index(after: colon)...].trimmingCharacters(in: .whitespaces)
            headers[name] = value
        }
        guard let lengthText = headers["content-length"],
              let length = Int(lengthText),
              length >= 0,
              length <= Self.maximumRequestBytes else { return .invalid }
        let bodyStart = headerRange.upperBound
        guard data.count >= bodyStart + length else { return .incomplete }
        let body = data.subdata(in: bodyStart..<(bodyStart + length))
        return .complete(HTTPRequest(
            method: String(requestParts[0]),
            path: String(requestParts[1]),
            headers: headers,
            body: body
        ))
    }
}

private extension NSLock {
    func withLock<T>(_ body: () throws -> T) rethrows -> T {
        lock()
        defer { unlock() }
        return try body()
    }
}
