// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

import Foundation
import RoamlingCore

final class LoopbackHookReceiver: @unchecked Sendable {
    typealias Normalizer = @Sendable (Data, TimeInterval) throws -> CompanionEvent?

    var state: ActivityReceiverState {
        lock.withLock { receiverState }
    }

    /// Bounds how much an unauthenticated local sender can make Roamling
    /// buffer, since the token is only checked once the body is complete.
    /// 1 MiB is where curl starts sending `Expect: 100-continue`, which this
    /// receiver never answers, so a larger cap could not accept more anyway.
    private static let maximumRequestBytes = 1_024 * 1_024
    private let token: String
    private let tokenHeader: String
    private let path: String
    private let port: UInt16
    private let clock: @Sendable () -> TimeInterval
    private let normalizer: Normalizer

    private let lock = NSLock()
    private let stream: AsyncStream<CompanionEvent>
    private let continuation: AsyncStream<CompanionEvent>.Continuation
    private var receiverState = ActivityReceiverState.stopped
    private var listener: LoopbackSocket?

    private let label: String

    init(
        label: String,
        token: String,
        tokenHeader: String,
        path: String,
        port: UInt16,
        clock: @escaping @Sendable () -> TimeInterval,
        normalizer: @escaping Normalizer
    ) {
        self.label = label
        self.token = token
        self.tokenHeader = tokenHeader.lowercased()
        self.path = path
        self.port = port
        self.clock = clock
        self.normalizer = normalizer
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
        let socket = LoopbackSocket(port: port)
        lock.withLock { receiverState = .starting }
        do {
            // Bind and listen happen here rather than in a callback, so a port
            // already in use is an error the caller sees at start().
            try socket.start(label: label) { [weak self] connection in
                self?.serve(connection)
            }
        } catch {
            lock.withLock { receiverState = .failed(String(describing: error)) }
            throw error
        }
        lock.withLock {
            self.listener = socket
            receiverState = .ready
        }
    }

    func stop() {
        let listener = lock.withLock { () -> LoopbackSocket? in
            let current = self.listener
            self.listener = nil
            receiverState = .stopped
            return current
        }
        listener?.stop()
    }

    /// One request, read to completion and answered. Connections are served one
    /// at a time, which is what the serial queue behind `NWListener` did.
    private func serve(_ connection: LoopbackSocket.Handle) {
        var request = Data()
        while true {
            guard request.count <= Self.maximumRequestBytes else {
                respond(status: 413, phrase: "Payload Too Large", on: connection)
                return
            }
            switch parse(request) {
            case let .complete(parsed):
                handle(parsed, on: connection)
                return
            case .incomplete:
                guard let chunk = LoopbackSocket.read(connection, upTo: 64 * 1_024) else {
                    // The peer stopped talking mid-request.
                    respond(status: 400, phrase: "Bad Request", on: connection)
                    return
                }
                request.append(chunk)
            case .invalid:
                respond(status: 400, phrase: "Bad Request", on: connection)
                return
            }
        }
    }

    private func handle(_ request: HTTPRequest, on connection: LoopbackSocket.Handle) {
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

    private func respond(status: Int, phrase: String, on connection: LoopbackSocket.Handle) {
        let response = "HTTP/1.1 \(status) \(phrase)\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
        if let data = response.data(using: .utf8) {
            LoopbackSocket.write(connection, data)
        }
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
