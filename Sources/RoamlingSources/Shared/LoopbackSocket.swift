// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

import Foundation
#if canImport(Darwin)
import Darwin
#elseif canImport(Glibc)
import Glibc
#elseif canImport(WinSDK)
import WinSDK
#endif

/// A loopback TCP listener written against the sockets every platform has.
///
/// It replaces `NWListener`, which was one of exactly two lines that stopped
/// the codebase compiling on Windows. Nothing above it changed: the same
/// requests arrive, on the same port, bound to the same address.
///
/// `127.0.0.1` is not a default here but a requirement -- an agent hook posts a
/// token over this socket, and a wildcard bind would put that on the network.
final class LoopbackSocket: @unchecked Sendable {
    #if os(Windows)
    typealias Handle = SOCKET
    static let invalidHandle = INVALID_SOCKET
    #else
    typealias Handle = Int32
    static let invalidHandle: Int32 = -1
    #endif

    enum SocketError: Error, CustomStringConvertible {
        case create(Int32)
        case bind(Int32)
        case listen(Int32)

        var description: String {
            switch self {
            case let .create(code): "could not open a socket (errno \(code))"
            case let .bind(code): "could not bind 127.0.0.1 (errno \(code))"
            case let .listen(code): "could not listen (errno \(code))"
            }
        }
    }

    private let port: UInt16
    private let lock = NSLock()
    private var handle = LoopbackSocket.invalidHandle
    private var isStopping = false
    private let finished = DispatchSemaphore(value: 0)

    init(port: UInt16) {
        self.port = port
        Self.startNetworkingOnce()
    }

    /// Binds and listens synchronously, so a caller learns about a taken port
    /// here rather than through a callback, then serves on its own thread.
    func start(label: String, handler: @escaping @Sendable (Handle) -> Void) throws {
        let fd = socket(AF_INET, sockStreamType, 0)
        guard fd != Self.invalidHandle else { throw SocketError.create(Self.lastErrno()) }

        var reuse: Int32 = 1
        withUnsafePointer(to: &reuse) { pointer in
            _ = pointer.withMemoryRebound(to: CChar.self, capacity: MemoryLayout<Int32>.size) {
                setsockopt(fd, SOL_SOCKET, SO_REUSEADDR, $0, socklen_t(MemoryLayout<Int32>.size))
            }
        }

        var address = sockaddr_in()
        address.sin_family = sa_family_t(AF_INET)
        address.sin_port = port.bigEndian
        // INADDR_LOOPBACK, spelled out because its type differs by platform.
        address.sin_addr.s_addr = UInt32(0x7F00_0001).bigEndian
        #if canImport(Darwin)
        address.sin_len = UInt8(MemoryLayout<sockaddr_in>.size)
        #endif

        let bound = withUnsafePointer(to: &address) { pointer in
            pointer.withMemoryRebound(to: sockaddr.self, capacity: 1) {
                bind(fd, $0, socklen_t(MemoryLayout<sockaddr_in>.size))
            }
        }
        guard bound == 0 else {
            let code = Self.lastErrno()
            Self.close(fd)
            throw SocketError.bind(code)
        }
        guard listen(fd, 16) == 0 else {
            let code = Self.lastErrno()
            Self.close(fd)
            throw SocketError.listen(code)
        }

        lock.withLock {
            handle = fd
            isStopping = false
        }

        let thread = Thread { [weak self] in
            self?.serve(fd, handler: handler)
        }
        thread.name = label
        thread.stackSize = 512 * 1_024
        thread.start()
    }

    /// Blocks briefly until the accept loop has actually left, so a restart on
    /// the same port does not race the socket that is still bound to it.
    func stop() {
        let fd: Handle? = lock.withLock {
            guard handle != Self.invalidHandle else { return nil }
            isStopping = true
            return handle
        }
        guard let fd else { return }
        // accept() is blocking. Knocking on our own door is what returns it.
        wakeAccept()
        Self.close(fd)
        lock.withLock { handle = Self.invalidHandle }
        _ = finished.wait(timeout: .now() + 1)
    }

    private func serve(_ fd: Handle, handler: @Sendable (Handle) -> Void) {
        defer { finished.signal() }
        while true {
            let connection = accept(fd, nil, nil)
            if lock.withLock({ isStopping }) {
                if connection != Self.invalidHandle { Self.close(connection) }
                return
            }
            guard connection != Self.invalidHandle else {
                #if !os(Windows)
                if Self.lastErrno() == EINTR { continue }
                #endif
                return
            }
            handler(connection)
            Self.finish(connection)
            Self.close(connection)
        }
    }

    /// One throwaway connection to ourselves, so the blocked `accept` returns
    /// and sees the stop flag. Cheaper than polling, which would wake the CPU
    /// forever to catch a shutdown that happens once.
    private func wakeAccept() {
        let fd = socket(AF_INET, sockStreamType, 0)
        guard fd != Self.invalidHandle else { return }
        defer { Self.close(fd) }
        var address = sockaddr_in()
        address.sin_family = sa_family_t(AF_INET)
        address.sin_port = port.bigEndian
        address.sin_addr.s_addr = UInt32(0x7F00_0001).bigEndian
        #if canImport(Darwin)
        address.sin_len = UInt8(MemoryLayout<sockaddr_in>.size)
        #endif
        _ = withUnsafePointer(to: &address) { pointer in
            pointer.withMemoryRebound(to: sockaddr.self, capacity: 1) {
                connect(fd, $0, socklen_t(MemoryLayout<sockaddr_in>.size))
            }
        }
    }

    // MARK: - Reading and writing

    /// Up to `limit` bytes, or nil once the peer stops talking.
    static func read(_ fd: Handle, upTo limit: Int) -> Data? {
        var buffer = [UInt8](repeating: 0, count: limit)
        let received: Int = buffer.withUnsafeMutableBytes { raw in
            #if os(Windows)
            Int(recv(fd, raw.baseAddress?.assumingMemoryBound(to: CChar.self), Int32(limit), 0))
            #else
            recv(fd, raw.baseAddress, limit, 0)
            #endif
        }
        guard received > 0 else { return nil }
        return Data(buffer[0..<received])
    }

    /// Says goodbye without throwing away what was just written.
    ///
    /// Closing a socket that still has unread bytes queued makes the kernel
    /// send RST, and the peer loses the reply along with them. That is how a
    /// rejected oversized request looked like no answer at all rather than the
    /// 400 it was given.
    static func finish(_ fd: Handle) {
        shutdownWrite(fd)
        var drained = 0
        // Bounded: a sender that keeps talking after being told no does not get
        // to hold this thread.
        while drained < 4 * 1_024 * 1_024, let chunk = read(fd, upTo: 64 * 1_024) {
            drained += chunk.count
        }
    }

    private static func shutdownWrite(_ fd: Handle) {
        #if os(Windows)
        _ = shutdown(fd, SD_SEND)
        #elseif canImport(Glibc)
        _ = shutdown(fd, Int32(SHUT_WR))
        #else
        _ = shutdown(fd, SHUT_WR)
        #endif
    }

    static func write(_ fd: Handle, _ data: Data) {
        var offset = 0
        data.withUnsafeBytes { raw in
            guard let base = raw.baseAddress else { return }
            while offset < data.count {
                #if os(Windows)
                let sent = Int(send(
                    fd,
                    base.advanced(by: offset).assumingMemoryBound(to: CChar.self),
                    Int32(data.count - offset),
                    0
                ))
                #else
                let sent = send(fd, base.advanced(by: offset), data.count - offset, 0)
                #endif
                guard sent > 0 else { return }
                offset += sent
            }
        }
    }

    // MARK: - Platform spelling

    private var sockStreamType: Int32 {
        #if canImport(Glibc)
        Int32(SOCK_STREAM.rawValue)
        #else
        Int32(SOCK_STREAM)
        #endif
    }

    static func close(_ fd: Handle) {
        #if os(Windows)
        closesocket(fd)
        #else
        _ = Darwin.close(fd)
        #endif
    }

    static func lastErrno() -> Int32 {
        #if os(Windows)
        WSAGetLastError()
        #else
        errno
        #endif
    }

    #if os(Windows)
    private static let networkingStarted: Bool = {
        var data = WSADATA()
        return WSAStartup(0x0202, &data) == 0
    }()
    #endif

    private static func startNetworkingOnce() {
        #if os(Windows)
        _ = networkingStarted
        #endif
    }
}

extension NSLock {
    func withLock<T>(_ body: () throws -> T) rethrows -> T {
        lock()
        defer { unlock() }
        return try body()
    }
}
