// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

import Foundation

/// Whether an agent's config has Roamling's hook in it.
public enum AgentIntegrationStatus: String, Sendable {
    case notInstalled
    case needsRepair
    case installed
}

/// Whether the loopback endpoint that hook posts to is listening.
public enum ActivityReceiverState: Equatable, Sendable {
    case stopped
    case starting
    case ready
    case failed(String)
}

/// One agent Roamling can watch.
///
/// The runtime used to name Claude Code and Codex directly and build both in
/// its initializer, which meant the orchestration could not move anywhere the
/// hook transport could not follow. It takes a list of these instead, so the
/// list can be empty -- a platform that has no way to install hooks yet still
/// gets a pet that roams.
@MainActor
public protocol AgentIntegration: AnyObject {
    /// Stable, and used to build the event source id and the localization keys
    /// for this agent's menu and dialogs.
    var id: String { get }
    /// Not localized: these are product names.
    var displayName: String { get }

    var installationStatus: AgentIntegrationStatus { get }
    var receiverState: ActivityReceiverState { get }

    func makeEventStream() -> AsyncStream<CompanionEvent>
    func startReceiving() throws
    func stopReceiving()

    func install() -> Result<Void, Error>
    func remove() -> Result<Void, Error>
}

public extension AgentIntegration {
    /// Upgrades an existing install to the current handler shape without asking
    /// again. Nil when there is nothing to repair, so a machine that never
    /// opted in keeps an untouched config.
    @discardableResult
    func repairIfNeeded() -> Result<Void, Error>? {
        guard installationStatus == .needsRepair else { return nil }
        return install()
    }
}
