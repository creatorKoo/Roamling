// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

import Foundation
import RoamlingEngine
import RoamlingPet

/// What a dialog says. How it is put on screen is the platform's business;
/// what it says is not, or the two shells would say different things.
public struct AlertModel: Sendable, Equatable {
    public let title: String
    public let body: String
    /// First is the default. Two buttons means the first confirms.
    public let buttons: [String]
    public let isWarning: Bool

    public init(title: String, body: String, buttons: [String], isWarning: Bool = false) {
        self.title = title
        self.body = body
        self.buttons = buttons
        self.isWarning = isWarning
    }
}

/// What the shell does after an action runs. The platform carries these out;
/// deciding which one belongs to an action does not vary by platform.
public enum ShellEffect: Sendable {
    /// Nothing visible, and the menu need not change.
    case none
    case rebuildMenu
    case present(AlertModel)
    case presentThenRebuild(AlertModel)
    case openTuningPanel
    case reveal(URL)
    case openLink(URL)
    case copyToClipboard(String)
    /// Go and look for a new version. The platform owns the fetching and the
    /// swapping; what this module owns is when to offer it and how it reads.
    case checkForUpdates
    /// Remember the choice and stop or start the timer.
    case setAutomaticUpdates(Bool)
    case quit
}

@MainActor
public enum ShellPrompt {
    /// What to ask before running this, or nil to run it straight away.
    public static func confirmation(for action: MenuAction) -> AlertModel? {
        switch action {
        case let .installAgent(id):
            AgentCopy.forAgent(id).map {
                AlertModel(
                    title: localized($0.installTitle),
                    body: localized($0.installBody),
                    buttons: [localized("button.install"), localized("button.cancel")]
                )
            }
        case let .removeAgent(id):
            AgentCopy.forAgent(id).map {
                AlertModel(
                    title: localized($0.removeTitle),
                    body: localized($0.removeBody),
                    buttons: [localized("button.remove"), localized("button.cancel")]
                )
            }
        case .enableAccessibility:
            AlertModel(
                title: localized("accessibility.alert.title"),
                body: localized("accessibility.alert.body"),
                buttons: [localized("button.openSystemSettings"), localized("button.cancel")]
            )
        case .enableVisualPlacement:
            AlertModel(
                title: localized("visual.alert.title"),
                body: localized("visual.alert.body"),
                buttons: [localized("button.openSystemSettings"), localized("button.cancel")]
            )
        default:
            nil
        }
    }

    /// Which strings belong to which agent, written out rather than built from
    /// the id: a constructed key that is missing shows up as the key itself on
    /// screen, and nothing greps for a name that was never typed.
    struct AgentCopy {
        let installTitle: String
        let installBody: String
        let removeTitle: String
        let removeBody: String
        let installed: String
        let removed: String
        let installedDetail: String
        let removedDetail: String
        /// What went wrong is named by the agent, not by whichever one was
        /// written first: a failed Codex install used to say "Couldn't update
        /// Claude Code settings", which points at the wrong file.
        let failure: String

        static func forAgent(_ id: String) -> AgentCopy? {
            switch id {
            case "claude-code":
                AgentCopy(
                    installTitle: "alert.claude.install.title",
                    installBody: "alert.claude.install.body",
                    removeTitle: "alert.claude.remove.title",
                    removeBody: "alert.claude.remove.body",
                    installed: "result.claude.installed",
                    removed: "result.claude.removed",
                    installedDetail: "result.detail.claude",
                    removedDetail: "result.detail.claude",
                    failure: "error.claude.settings"
                )
            case "codex":
                AgentCopy(
                    installTitle: "alert.codex.install.title",
                    installBody: "alert.codex.install.body",
                    removeTitle: "alert.codex.remove.title",
                    removeBody: "alert.codex.remove.body",
                    installed: "result.codex.installed",
                    removed: "result.codex.removed",
                    installedDetail: "result.detail.codex.installed",
                    removedDetail: "result.detail.codex.removed",
                    failure: "error.codex.hooks"
                )
            default:
                nil
            }
        }
    }

    /// The version is passed in rather than read here: on macOS it lives in
    /// `Support/Info.plist` and on Windows in the crate, and neither is
    /// something this module should know how to open.
    public static func about(version: String) -> AlertModel {
        AlertModel(
            title: "Roamling",
            body: localizedFormat("about.version", version)
                + "\n\n" + localized("alert.about.body"),
            buttons: [localized("button.ok"), localized("menu.viewSource")]
        )
    }

    public static let sourceURL = URL(string: "https://github.com/creatorKoo/Roamling")!

    static func integrationResult(
        _ result: Result<Void, Error>,
        success: String,
        detail: String,
        failure: String
    ) -> AlertModel {
        switch result {
        case .success:
            AlertModel(title: success, body: detail, buttons: [])
        case let .failure(error):
            AlertModel(
                title: localized(failure),
                body: error.localizedDescription,
                buttons: [],
                isWarning: true
            )
        }
    }

    /// What a check the user asked for comes back with. A background check
    /// that finds nothing says nothing, so this is only ever shown on request
    /// or when there is something to report.
    public static func updateResult(
        upToDate current: String
    ) -> AlertModel {
        AlertModel(
            title: localized("result.update.upToDate"),
            body: localizedFormat("result.update.upToDate.detail", current),
            buttons: []
        )
    }

    public static func updateStaged(version: String) -> AlertModel {
        AlertModel(
            title: localizedFormat("result.update.ready", version),
            body: localized("result.update.ready.detail"),
            buttons: []
        )
    }

    public static func updateFailure(_ detail: String) -> AlertModel {
        AlertModel(
            title: localized("error.update.failed"),
            body: detail,
            buttons: [],
            isWarning: true
        )
    }

    static func petLoadFailure(_ error: Error) -> AlertModel {
        AlertModel(
            title: localized("error.pet.load"),
            body: error.localizedDescription,
            buttons: [],
            isWarning: true
        )
    }
}

/// Runs a menu action against the runtime and says what should happen next.
///
/// Confirmation is not here: presenting a modal and waiting is inherently the
/// platform's, and the platform asks `ShellPrompt.confirmation(for:)` first.
@MainActor
public enum ShellController {
    public static func perform(
        _ action: MenuAction,
        runtime: RoamlingRuntime,
        version: String
    ) -> ShellEffect {
        switch action {
        case let .selectBuiltInPet(kind):
            runtime.useBuiltInPet(kind)
            return .rebuildMenu
        case let .selectInstalledPet(path):
            switch runtime.loadPet(at: URL(fileURLWithPath: path, isDirectory: true)) {
            case .success: return .rebuildMenu
            case let .failure(error): return .present(ShellPrompt.petLoadFailure(error))
            }
        case let .setScale(value):
            runtime.setScale(value)
            return .rebuildMenu
        case .toggleRoaming:
            runtime.isRoamingEnabled.toggle()
            return .rebuildMenu
        case .togglePointerAvoidance:
            runtime.isPointerAvoidanceEnabled.toggle()
            return .rebuildMenu
        case .toggleInteractions:
            runtime.areInteractionsEnabled.toggle()
            return .rebuildMenu
        case .showTuning:
            return .openTuningPanel
        case let .installAgent(id):
            guard let agent = runtime.agentIntegration(id: id),
                  let copy = ShellPrompt.AgentCopy.forAgent(id) else { return .none }
            return .presentThenRebuild(ShellPrompt.integrationResult(
                agent.install(),
                success: localized(copy.installed),
                detail: localized(copy.installedDetail),
                failure: copy.failure
            ))
        case let .removeAgent(id):
            guard let agent = runtime.agentIntegration(id: id),
                  let copy = ShellPrompt.AgentCopy.forAgent(id) else { return .none }
            return .presentThenRebuild(ShellPrompt.integrationResult(
                agent.remove(),
                success: localized(copy.removed),
                detail: localized(copy.removedDetail),
                failure: copy.failure
            ))
        case let .testAgentReaction(id):
            runtime.testAgentReaction(id: id)
            return .none
        case .enableAccessibility:
            runtime.requestAccessibilityAuthorization()
            return .rebuildMenu
        case .enableVisualPlacement:
            runtime.requestScreenCaptureAuthorization()
            return .rebuildMenu
        case .openPetFolder:
            return .reveal(petFolder)
        case .copyDiagnostics:
            return .copyToClipboard(runtime.diagnosticsText)
        case .reloadPets:
            runtime.reloadCatalog()
            return .rebuildMenu
        case .checkForUpdates:
            return .checkForUpdates
        case .toggleAutomaticUpdates:
            return .setAutomaticUpdates(!ShellMenu.automaticUpdates)
        case .showAbout:
            return .present(ShellPrompt.about(version: version))
        case .quit:
            return .quit
        }
    }

    /// Where a user drops a pet package. The same folder `PetCatalog` searches
    /// first, asked from the one place that knows it, so the menu cannot open a
    /// directory nothing reads.
    public static var petFolder: URL { PetCatalog.userPetFolder }
}
