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
    case quit
}

@MainActor
public enum ShellPrompt {
    /// What to ask before running this, or nil to run it straight away.
    public static func confirmation(for action: MenuAction) -> AlertModel? {
        switch action {
        case .installClaudeCode:
            AlertModel(
                title: localized("alert.claude.install.title"),
                body: localized("alert.claude.install.body"),
                buttons: [localized("button.install"), localized("button.cancel")]
            )
        case .removeClaudeCode:
            AlertModel(
                title: localized("alert.claude.remove.title"),
                body: localized("alert.claude.remove.body"),
                buttons: [localized("button.remove"), localized("button.cancel")]
            )
        case .installCodex:
            AlertModel(
                title: localized("alert.codex.install.title"),
                body: localized("alert.codex.install.body"),
                buttons: [localized("button.install"), localized("button.cancel")]
            )
        case .removeCodex:
            AlertModel(
                title: localized("alert.codex.remove.title"),
                body: localized("alert.codex.remove.body"),
                buttons: [localized("button.remove"), localized("button.cancel")]
            )
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

    public static var about: AlertModel {
        AlertModel(
            title: "Roamling",
            body: localized("alert.about.body"),
            buttons: [localized("button.ok"), localized("menu.viewSource")]
        )
    }

    public static let sourceURL = URL(string: "https://github.com/creatorKoo/Roamling")!

    static func integrationResult(
        _ result: Result<Void, Error>,
        success: String,
        detail: String
    ) -> AlertModel {
        switch result {
        case .success:
            AlertModel(title: success, body: detail, buttons: [])
        case let .failure(error):
            AlertModel(
                title: localized("error.claude.settings"),
                body: error.localizedDescription,
                buttons: [],
                isWarning: true
            )
        }
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
    public static func perform(_ action: MenuAction, runtime: RoamlingRuntime) -> ShellEffect {
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
        case .installClaudeCode:
            return .presentThenRebuild(ShellPrompt.integrationResult(
                runtime.installClaudeCodeIntegration(),
                success: localized("result.claude.installed"),
                detail: localized("result.detail.claude")
            ))
        case .removeClaudeCode:
            return .presentThenRebuild(ShellPrompt.integrationResult(
                runtime.removeClaudeCodeIntegration(),
                success: localized("result.claude.removed"),
                detail: localized("result.detail.claude")
            ))
        case .testClaudeCodeReaction:
            runtime.testClaudeCodeReaction()
            return .none
        case .installCodex:
            return .presentThenRebuild(ShellPrompt.integrationResult(
                runtime.installCodexIntegration(),
                success: localized("result.codex.installed"),
                detail: localized("result.detail.codex.installed")
            ))
        case .removeCodex:
            return .presentThenRebuild(ShellPrompt.integrationResult(
                runtime.removeCodexIntegration(),
                success: localized("result.codex.removed"),
                detail: localized("result.detail.codex.removed")
            ))
        case .testCodexReaction:
            runtime.testCodexReaction()
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
        case .showAbout:
            return .present(ShellPrompt.about)
        case .quit:
            return .quit
        }
    }

    /// Where a user drops a pet package. The same folder `PetCatalog` searches
    /// first, asked from the one place that knows it, so the menu cannot open a
    /// directory nothing reads.
    public static var petFolder: URL { PetCatalog.userPetFolder }
}
