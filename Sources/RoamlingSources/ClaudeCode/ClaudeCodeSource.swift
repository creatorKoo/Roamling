// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

import Foundation
import RoamlingCore

public typealias ClaudeCodeReceiverState = ActivityReceiverState

public final class ClaudeCodeSource: ActivitySource, @unchecked Sendable {
    public let id = "claude-code"
    public let sourceType = ActivitySourceType.agent

    public var state: ClaudeCodeReceiverState {
        receiver.state
    }

    private let receiver: LoopbackHookReceiver

    public init(
        token: String,
        port: UInt16 = ClaudeCodeHookInstaller.defaultPort,
        clock: @escaping @Sendable () -> TimeInterval = {
            ProcessInfo.processInfo.systemUptime
        }
    ) {
        receiver = LoopbackHookReceiver(
            label: "dev.roamling.claude-code-source",
            token: token,
            tokenHeader: ClaudeCodeHookInstaller.tokenHeader,
            path: ClaudeCodeHookInstaller.path,
            port: port,
            clock: clock,
            normalizer: ClaudeCodeEventNormalizer.event
        )
    }

    public func makeEventStream() -> AsyncStream<CompanionEvent> {
        receiver.makeEventStream()
    }

    public func start() throws {
        try receiver.start()
    }

    public func stop() {
        receiver.stop()
    }
}
