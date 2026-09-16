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
    /// One of the shipped colour sets, by its index in the core's table.
    case selectPalettePreset(index: Int)
    /// Open the window with a slider per axis. Hidden behind a modifier: it is
    /// for someone who wants a colour the nine presets do not have.
    case openPaletteMixer
    case setScale(Double)
    case toggleHidden
    case toggleRoaming
    case togglePointerAvoidance
    case toggleInteractions
    case showTuning
    case toggleWorkApp(id: String)
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
    case toggleLaunchAtLogin
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
        /// `isOn` for a row that both opens a submenu and reports a choice.
        /// macOS draws the checkmark and the arrow together; a shell that
        /// cannot may ignore it, but then nothing says which pet is in use.
        case submenu([MenuItem], isOn: Bool = false)
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
    /// Whether the OS will start the app at login. Read from the OS every time
    /// the menu is built rather than remembered here, so the checkmark cannot
    /// disagree with System Settings or the installer.
    public static var launchAtLogin = false

    public enum UpdateStatus: Equatable, Sendable {
        case idle
        case checking
        case staged(version: String)
    }

    private static var advancedUpdateItem: MenuItem? {
        switch updateStatus {
        case .idle:
            MenuItem(localized("menu.update.check"), .command(.checkForUpdates))
        case .checking:
            MenuItem(localized("status.update.checking"), .caption)
        case .staged:
            nil
        }
    }

    private static var stagedUpdateItem: MenuItem? {
        guard case let .staged(version) = updateStatus else { return nil }
        return MenuItem(localizedFormat("status.update.ready", version), .caption)
    }

    /// `alternateHeld` is read once when the menu opens, the way the Windows
    /// tray reads Shift -- macOS's own `NSMenuItem.isAlternate` needs a row to
    /// stand in front of it, and the row this hides behind is the last one.
    /// So the modifier is the one held when the menu bar item is clicked, not
    /// when the submenu is reached.
    public static func items(
        for runtime: RoamlingRuntime,
        alternateHeld: Bool = false
    ) -> [MenuItem] {
        let petName = runtime.selectedBuiltInPet.map { localizedBuiltInPetName($0) }
            ?? runtime.petDisplayName
        var items: [MenuItem] = [
            MenuItem(localizedFormat("menu.title", petName), .caption),
            .separator,
            MenuItem(localized("menu.hide"), .check(.toggleHidden, isOn: runtime.isHidden)),
            .separator,
            MenuItem(localized("menu.pet"), .submenu(petItems(for: runtime, alternateHeld: alternateHeld))),
            MenuItem(localized("menu.size"), .submenu(sizeItems(for: runtime))),
            .separator,
            MenuItem(localized("menu.movement"), .submenu(movementItems(for: runtime))),
            MenuItem(localized("menu.awareness"), .submenu(awarenessItems(for: runtime))),
            .separator,
            MenuItem(localized("menu.advanced"), .submenu(advancedItems())),
        ]
        // A staged update is an alert, not a setting. Keep it where the user
        // can see it even though the update controls live under Advanced.
        if let stagedUpdateItem { items.append(stagedUpdateItem) }
        items += [
            .separator,
            MenuItem(localized("menu.about"), .command(.showAbout)),
            MenuItem(localized("menu.quit"), .command(.quit), shortcut: "q")
        ]
        items.reserveCapacity(items.count)
        return items
    }

    private static func movementItems(for runtime: RoamlingRuntime) -> [MenuItem] {
        [
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
    }

    private static func awarenessItems(for runtime: RoamlingRuntime) -> [MenuItem] {
        var items = [
            MenuItem(localized("menu.accessibility"), .submenu(accessibilityItems(for: runtime))),
            MenuItem(localized("menu.visualPlacement"), .submenu(visualPlacementItems(for: runtime))),
            MenuItem(localized("menu.workApps"), .submenu(workAppItems(for: runtime))),
        ]
        // One submenu per agent, in the order the app handed them over. An app
        // built with no agents simply has none, which is what a platform that
        // cannot install hooks yet gets. Keeping them inside Awareness makes
        // the top-level shape independent of how many agents exist.
        items += runtime.agentIntegrations.map { agent in
            MenuItem(agent.displayName, .submenu(agentItems(for: agent)))
        }
        return items
    }

    private static func advancedItems() -> [MenuItem] {
        var items = [
            MenuItem(localized("menu.openPetFolder"), .command(.openPetFolder)),
            MenuItem(localized("menu.copyDiagnostics"), .command(.copyDiagnostics)),
            MenuItem(localized("menu.reloadPets"), .command(.reloadPets), shortcut: "r"),
        ]
        if let advancedUpdateItem { items.append(advancedUpdateItem) }
        items += [
            MenuItem(
                localized("menu.launchAtLogin"),
                .check(.toggleLaunchAtLogin, isOn: launchAtLogin)
            ),
            MenuItem(
                localized("menu.update.auto"),
                .check(.toggleAutomaticUpdates, isOn: automaticUpdates)
            ),
        ]
        return items
    }

    /// The colour submenu: the shipped sets, then one picker per part.
    ///
    /// Only the built-in Mochi carries the region map the recolour needs, so
    /// the rows say so rather than doing nothing when another pet is showing.
    /// The colours, as the submenu of the pet that wears them.
    ///
    /// Only the built-in Mochi carries the region map the recolour needs, so
    /// hanging the colours off its row is also what says which pet they belong
    /// to -- and picking one selects that pet, since a row with a submenu has
    /// no click of its own on macOS.
    ///
    /// One extra row while the modifier is held: the window with a slider per
    /// axis. Nine presets and a colour picker cover what was asked for, and
    /// this is for going further than that.
    private static func paletteItems(
        for runtime: RoamlingRuntime,
        alternateHeld: Bool
    ) -> [MenuItem] {
        var items = runtime.paletteOptions.enumerated().map { index, option in
            MenuItem(
                localized(option.key),
                .check(.selectPalettePreset(index: index), isOn: option.isSelected)
            )
        }
        guard alternateHeld else { return items }
        items.append(.separator)
        items.append(MenuItem(localized("menu.palette.custom"), .command(.openPaletteMixer)))
        return items
    }

    private static func petItems(
        for runtime: RoamlingRuntime,
        alternateHeld: Bool
    ) -> [MenuItem] {
        var items = BuiltInPetKind.allCases.map { kind in
            // Mochi opens its colours rather than being clicked. macOS gives a
            // row with a submenu no action of its own, so choosing a colour is
            // what selects this pet -- which is the same gesture either way,
            // because the colours are only Mochi's.
            guard kind == .mochi else {
                return MenuItem(
                    localizedFormat("menu.pet.builtin", localizedBuiltInPetName(kind)),
                    .check(.selectBuiltInPet(kind), isOn: runtime.selectedBuiltInPet == kind)
                )
            }
            return MenuItem(
                localizedFormat("menu.pet.builtin", localizedBuiltInPetName(kind)),
                .submenu(
                    paletteItems(for: runtime, alternateHeld: alternateHeld),
                    isOn: runtime.selectedBuiltInPet == kind
                )
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

    /// The apps the pet treats as work, and the ones it could.
    ///
    /// Recently seen first, because the app someone just used is the one they
    /// are about to name. Anything already chosen but not seen lately follows,
    /// checked: a list you cannot uncheck from because the app happens to be
    /// closed is a trap.
    private static func workAppItems(for runtime: RoamlingRuntime) -> [MenuItem] {
        var identifiers = runtime.recentApplications
        identifiers += runtime.workApps.filter { !identifiers.contains($0) }
        guard !identifiers.isEmpty else {
            return [MenuItem(localized("menu.workApps.none"), .caption)]
        }
        return identifiers.map { identifier in
            MenuItem(
                runtime.applicationDisplayName(for: identifier) ?? identifier,
                .check(.toggleWorkApp(id: identifier), isOn: runtime.workApps.contains(identifier))
            )
        }
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
