// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

import Foundation
import RoamlingCore

/// Claude Code as one `AgentIntegration`: its hook installer and its loopback
/// receiver, behind the interface the runtime actually needs.
@MainActor
public final class ClaudeCodeIntegration: AgentIntegration {
    public let id = "claude-code"
    public let displayName = "Claude Code"

    private let source: ClaudeCodeSource
    private let installer: ClaudeCodeHookInstaller

    public init(
        token: String,
        clock: @escaping @Sendable () -> TimeInterval = { ProcessInfo.processInfo.systemUptime }
    ) {
        source = ClaudeCodeSource(token: token, clock: clock)
        installer = ClaudeCodeHookInstaller(
            settingsURL: FileManager.default.homeDirectoryForCurrentUser
                .appendingPathComponent(".claude/settings.json"),
            token: token
        )
    }

    public var installationStatus: AgentIntegrationStatus { installer.status() }
    public var receiverState: ActivityReceiverState { source.state }
    public func makeEventStream() -> AsyncStream<CompanionEvent> { source.makeEventStream() }
    public func startReceiving() throws { try source.start() }
    public func stopReceiving() { source.stop() }
    public func install() -> Result<Void, Error> { Result { try installer.install() } }
    public func remove() -> Result<Void, Error> { Result { try installer.remove() } }
}

@MainActor
public final class CodexIntegration: AgentIntegration {
    public let id = "codex"
    public let displayName = "Codex"

    private let source: CodexSource
    private let installer: CodexHookInstaller

    public init(
        token: String,
        clock: @escaping @Sendable () -> TimeInterval = { ProcessInfo.processInfo.systemUptime }
    ) {
        source = CodexSource(token: token, clock: clock)
        installer = CodexHookInstaller(
            hooksURL: FileManager.default.homeDirectoryForCurrentUser
                .appendingPathComponent(".codex/hooks.json"),
            token: token
        )
    }

    public var installationStatus: AgentIntegrationStatus { installer.status() }
    public var receiverState: ActivityReceiverState { source.state }
    public func makeEventStream() -> AsyncStream<CompanionEvent> { source.makeEventStream() }
    public func startReceiving() throws { try source.start() }
    public func stopReceiving() { source.stop() }
    public func install() -> Result<Void, Error> { Result { try installer.install() } }
    public func remove() -> Result<Void, Error> { Result { try installer.remove() } }
}
