// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

import Foundation
import RoamlingCore

public typealias CodexReceiverState = ActivityReceiverState

public final class CodexSource: ActivitySource, @unchecked Sendable {
    public let id = "codex"
    public let sourceType = ActivitySourceType.agent

    public var state: CodexReceiverState { receiver.state }

    private let receiver: LoopbackHookReceiver

    public init(
        token: String,
        port: UInt16 = CodexHookInstaller.defaultPort,
        clock: @escaping @Sendable () -> TimeInterval = {
            ProcessInfo.processInfo.systemUptime
        }
    ) {
        receiver = LoopbackHookReceiver(
            label: "dev.roamling.codex-source",
            token: token,
            tokenHeader: CodexHookInstaller.tokenHeader,
            path: CodexHookInstaller.path,
            port: port,
            clock: clock,
            normalizer: CodexEventNormalizer.event
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
