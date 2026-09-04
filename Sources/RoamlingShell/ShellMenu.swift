// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

import Foundation
import RoamlingCore
import RoamlingEngine
import RoamlingPet

/// Everything the menu can ask for. A closed set, so a second platform renders
/// the same choices instead of inventing its own.
public enum MenuAction: Equatable, Sendable {
    case selectBuiltInPet(BuiltInPetKind)
    case selectInstalledPet(path: String)
    case setScale(Double)
    case toggleRoaming
    case togglePointerAvoidance
    case toggleInteractions
    case showTuning
    case installAgent(id: String)
    case removeAgent(id: String)
    case testAgentReaction(id: String)
    case enableAccessibility
    case enableVisualPlacement
    case openPetFolder
    case copyDiagnostics
    case reloadPets
    case checkForUpdates
    case toggleAutomaticUpdates
    case showAbout
    case quit
}

public struct MenuItem: Sendable {
    public enum Content: Sendable {
        /// A line that only reports something. Drawn disabled.
        case caption
        case command(MenuAction)
        /// Drawn with a checkmark when on. The pet and size lists are pick-one
        /// but AppKit draws them the same way, so they are not split here.
        case check(MenuAction, isOn: Bool)
        case submenu([MenuItem])
        case separator
    }

    public let title: String
    public let content: Content
    /// A single character, or empty. macOS reads it as a Command shortcut;
    /// other shells may ignore it.
    public let shortcut: String

    public init(_ title: String, _ content: Content, shortcut: String = "") {
        self.title = title
        self.content = content
        self.shortcut = shortcut
    }

    public static let separator = MenuItem("", .separator)
}

/// The menu as a value: a pure function of what the runtime currently says.
///
/// It lives here rather than in the AppKit delegate because a Windows tray
/// shows the same tree. Being a value also means the tree can be tested without
/// a screen -- which the AppKit version could not be.
@MainActor
public enum ShellMenu {
    /// What the update row says, and whether the timer is on. Both are set by
    /// the platform, because fetching and remembering are the platform's and
    /// this module only says how they read.
    public static var updateStatus: UpdateStatus = .idle
    public static var automaticUpdates = true

    public enum UpdateStatus: Equatable, Sendable {
        case idle
        case checking
        case staged(version: String)
    }

    private static var updateItem: MenuItem {
        switch updateStatus {
        case .idle:
            MenuItem(localized("menu.update.check"), .command(.checkForUpdates))
        case .checking:
            MenuItem(localized("status.update.checking"), .caption)
        case let .staged(version):
            MenuItem(localizedFormat("status.update.ready", version), .caption)
        }
    }

    public static func items(for runtime: RoamlingRuntime) -> [MenuItem] {
        var items: [MenuItem] = [
            MenuItem(localizedFormat("menu.title", runtime.petDisplayName), .caption),
            .separator,
            MenuItem(localized("menu.pet"), .submenu(petItems(for: runtime))),
            MenuItem(localized("menu.size"), .submenu(sizeItems(for: runtime))),
            .separator,
            MenuItem(
                localized("menu.roaming"),
                .check(.toggleRoaming, isOn: runtime.isRoamingEnabled)
            ),
            MenuItem(
                localized("menu.avoidPointer"),
                .check(.togglePointerAvoidance, isOn: runtime.isPointerAvoidanceEnabled)
            ),
            MenuItem(
                localized("menu.catchDrag"),
                .check(.toggleInteractions, isOn: runtime.areInteractionsEnabled)
            ),
            MenuItem(localized("menu.tuning"), .command(.showTuning), shortcut: ","),
        ]
        // One submenu per agent, in the order the app handed them over. An app
        // built with no agents simply has none, which is what a platform that
        // cannot install hooks yet gets.
        items += runtime.agentIntegrations.map { agent in
            MenuItem(agent.displayName, .submenu(agentItems(for: agent)))
        }
        items += [
            MenuItem(localized("menu.accessibility"), .submenu(accessibilityItems(for: runtime))),
            MenuItem(localized("menu.visualPlacement"), .submenu(visualPlacementItems(for: runtime))),
            .separator,
            MenuItem(localized("menu.openPetFolder"), .command(.openPetFolder)),
            MenuItem(localized("menu.copyDiagnostics"), .command(.copyDiagnostics)),
            MenuItem(localized("menu.reloadPets"), .command(.reloadPets), shortcut: "r"),
            .separator,
            // A staged update replaces the offer to look for one: there is
            // nothing more to do, and saying so is more useful than a button
            // that would find the same answer again.
            updateItem,
            MenuItem(
                localized("menu.update.auto"),
                .check(.toggleAutomaticUpdates, isOn: automaticUpdates)
            ),
            MenuItem(localized("menu.about"), .command(.showAbout)),
            MenuItem(localized("menu.quit"), .command(.quit), shortcut: "q")
        ]
        items.reserveCapacity(items.count)
        return items
    }

    private static func petItems(for runtime: RoamlingRuntime) -> [MenuItem] {
        var items = BuiltInPetKind.allCases.map { kind in
            MenuItem(
                localizedFormat("menu.pet.builtin", kind.displayName),
                .check(.selectBuiltInPet(kind), isOn: runtime.selectedBuiltInPet == kind)
            )
        }
        if !runtime.installedPets.isEmpty { items.append(.separator) }
        for descriptor in runtime.installedPets {
            let path = descriptor.packageURL.standardizedFileURL.path
            items.append(MenuItem(
                descriptor.displayName,
                .check(
                    .selectInstalledPet(path: descriptor.packageURL.path),
                    isOn: runtime.currentPetPackagePath == path
                )
            ))
        }
        // A package that declares one animation renders it for every state, and
        // from outside that looks like a pet whose behaviour is broken rather
        // than one whose sprite sheet is thin. Say which it is.
        items.append(.separator)
        let coverage = runtime.petCoverage
        items.append(MenuItem(
            localizedFormat("menu.pet.coverage", coverage.covered, coverage.total),
            .caption
        ))
        if !coverage.substituted.isEmpty {
            items.append(MenuItem(
                localizedFormat(
                    "menu.pet.substituted",
                    coverage.substituted.map(\.rawValue).sorted().joined(separator: ", ")
                ),
                .caption
            ))
        }
        if !coverage.placeholder.isEmpty {
            items.append(MenuItem(
                localizedFormat(
                    "menu.pet.placeholder",
                    coverage.placeholder.map(\.rawValue).sorted().joined(separator: ", ")
                ),
                .caption
            ))
        }
        return items
    }

    public static let scaleChoices: [(label: String, value: Double)] = [
        ("0.75×", 0.75), ("1.0×", 1.0), ("1.25×", 1.25), ("1.5×", 1.5)
    ]

    private static func sizeItems(for runtime: RoamlingRuntime) -> [MenuItem] {
        scaleChoices.map { choice in
            MenuItem(
                choice.label,
                .check(.setScale(choice.value), isOn: abs(runtime.scale - choice.value) < 0.01)
            )
        }
    }

    private static func agentItems(for agent: any AgentIntegration) -> [MenuItem] {
        let status = agent.installationStatus
        let integrationText = switch status {
        case .installed: localized("status.hooks.installed")
        case .needsRepair: localized("status.hooks.needsRepair")
        case .notInstalled: localized("status.hooks.notInstalled")
        }
        let receiverText = switch agent.receiverState {
        case .ready: localized("status.receiver.ready")
        case .starting: localized("status.receiver.starting")
        case .stopped: localized("status.receiver.stopped")
        case .failed: localized("status.receiver.unavailable")
        }
        var items = [
            MenuItem(integrationText, .caption),
            MenuItem(receiverText, .caption),
            .separator,
            MenuItem(
                status == .notInstalled ? localized("action.install") : localized("action.repair"),
                .command(.installAgent(id: agent.id))
            )
        ]
        if status != .notInstalled {
            items.append(MenuItem(localized("action.remove"), .command(.removeAgent(id: agent.id))))
        }
        items.append(MenuItem(
            localized("action.testReaction"),
            .command(.testAgentReaction(id: agent.id))
        ))
        return items
    }

    private static func accessibilityItems(for runtime: RoamlingRuntime) -> [MenuItem] {
        let authorized = runtime.isAccessibilityAuthorized
        var items = [
            MenuItem(
                authorized ? localized("accessibility.status.on") : localized("accessibility.status.off"),
                .caption
            ),
            .separator
        ]
        if authorized {
            // The OS owns revocation; pointing at it beats a button that cannot
            // actually take the permission back.
            items.append(MenuItem(localized("accessibility.revoke.hint"), .caption))
        } else {
            items.append(MenuItem(localized("accessibility.enable"), .command(.enableAccessibility)))
        }
        return items
    }

    private static func visualPlacementItems(for runtime: RoamlingRuntime) -> [MenuItem] {
        let authorized = runtime.isScreenCaptureAuthorized
        var items = [
            MenuItem(
                authorized ? localized("visual.status.on") : localized("visual.status.off"),
                .caption
            ),
            .separator
        ]
        if authorized {
            items.append(MenuItem(localized("visual.revoke.hint"), .caption))
        } else {
            items.append(MenuItem(localized("visual.enable"), .command(.enableVisualPlacement)))
        }
        return items
    }
}
