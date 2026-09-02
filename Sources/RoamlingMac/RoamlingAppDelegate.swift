// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

import AppKit
import RoamlingEngine
import RoamlingPet
import RoamlingSources

@MainActor
public final class RoamlingAppDelegate: NSObject, NSApplicationDelegate, NSMenuDelegate {
    private var runtime: RoamlingRuntime?
    private var statusItem: NSStatusItem?
    private var tuningWindowController: RuntimeTuningWindowController?

    public override init() {
        super.init()
    }

    public func applicationDidFinishLaunching(_ notification: Notification) {
        NSApp.setActivationPolicy(.accessory)
        let runtime = RoamlingRuntime(services: MacPlatform.makeServices())
        self.runtime = runtime
        runtime.start()
        runtime.repairClaudeCodeIntegrationIfNeeded()
        setupMenuBar()

        // Starts the complete AppKit lifecycle for automated packaging checks,
        // then exits cleanly without needing a synthetic user interaction.
        if ProcessInfo.processInfo.environment["ROAMLING_SMOKE_TEST"] == "1" {
            // A missing CFBundleLocalizations pins the whole process to English
            // no matter what ships in the module bundle, and nothing crashes to
            // say so. Print what actually resolved.
            let resolved = Bundle.module.preferredLocalizations.first ?? "none"
            print("smoke.localization=\(resolved) sample=\(localized("menu.quit"))")
            perform(#selector(finishSmokeTest), with: nil, afterDelay: 0.4)
        }
    }

    public func applicationWillTerminate(_ notification: Notification) {
        runtime?.stop()
    }

    public func menuNeedsUpdate(_ menu: NSMenu) {
        guard menu === statusItem?.menu else { return }
        runtime?.reloadCatalog()
        rebuildMenu()
    }

    private func setupMenuBar() {
        let item = NSStatusBar.system.statusItem(withLength: NSStatusItem.squareLength)
        item.button?.title = "🐾"
        item.button?.toolTip = "Roamling"
        let menu = NSMenu(title: "Roamling")
        menu.delegate = self
        item.menu = menu
        statusItem = item
        rebuildMenu()
    }

    private func rebuildMenu() {
        guard let runtime, let menu = statusItem?.menu else { return }
        menu.removeAllItems()

        let title = NSMenuItem(title: localizedFormat("menu.title", runtime.petDisplayName), action: nil, keyEquivalent: "")
        title.isEnabled = false
        menu.addItem(title)
        menu.addItem(.separator())

        let petItem = NSMenuItem(title: localized("menu.pet"), action: nil, keyEquivalent: "")
        let petMenu = NSMenu(title: localized("menu.pet"))
        for kind in BuiltInPetKind.allCases {
            let builtIn = NSMenuItem(
                title: localizedFormat("menu.pet.builtin", kind.displayName),
                action: #selector(selectBuiltInPet(_:)),
                keyEquivalent: ""
            )
            builtIn.target = self
            builtIn.representedObject = kind.rawValue as NSString
            builtIn.state = runtime.selectedBuiltInPet == kind ? .on : .off
            petMenu.addItem(builtIn)
        }
        if !runtime.installedPets.isEmpty { petMenu.addItem(.separator()) }
        for descriptor in runtime.installedPets {
            let item = NSMenuItem(
                title: descriptor.displayName,
                action: #selector(selectInstalledPet(_:)),
                keyEquivalent: ""
            )
            item.target = self
            item.representedObject = descriptor.packageURL.path as NSString
            item.state = runtime.currentPetPackagePath == descriptor.packageURL.standardizedFileURL.path ? .on : .off
            petMenu.addItem(item)
        }
        // A package that declares one animation renders it for every state, and
        // from outside that looks like a pet whose behaviour is broken rather
        // than one whose sprite sheet is thin. Say which it is.
        petMenu.addItem(.separator())
        let coverage = runtime.petCoverage
        let summary = NSMenuItem(
            title: localizedFormat(
                "menu.pet.coverage",
                coverage.covered,
                coverage.total
            ),
            action: nil,
            keyEquivalent: ""
        )
        summary.isEnabled = false
        petMenu.addItem(summary)
        if !coverage.substituted.isEmpty {
            let borrowed = NSMenuItem(
                title: localizedFormat(
                    "menu.pet.substituted",
                    coverage.substituted.map(\.rawValue).sorted().joined(separator: ", ")
                ),
                action: nil,
                keyEquivalent: ""
            )
            borrowed.isEnabled = false
            petMenu.addItem(borrowed)
        }
        if !coverage.placeholder.isEmpty {
            let missing = NSMenuItem(
                title: localizedFormat(
                    "menu.pet.placeholder",
                    coverage.placeholder.map(\.rawValue).sorted().joined(separator: ", ")
                ),
                action: nil,
                keyEquivalent: ""
            )
            missing.isEnabled = false
            petMenu.addItem(missing)
        }

        petItem.submenu = petMenu
        menu.addItem(petItem)

        let sizeItem = NSMenuItem(title: localized("menu.size"), action: nil, keyEquivalent: "")
        let sizeMenu = NSMenu(title: localized("menu.size"))
        for (label, value) in [("0.75×", 0.75), ("1.0×", 1.0), ("1.25×", 1.25), ("1.5×", 1.5)] {
            let item = NSMenuItem(title: label, action: #selector(selectSize(_:)), keyEquivalent: "")
            item.target = self
            item.representedObject = NSNumber(value: value)
            item.state = abs(runtime.scale - value) < 0.01 ? .on : .off
            sizeMenu.addItem(item)
        }
        sizeItem.submenu = sizeMenu
        menu.addItem(sizeItem)
        menu.addItem(.separator())

        menu.addItem(toggleItem(
            title: localized("menu.roaming"),
            checked: runtime.isRoamingEnabled,
            action: #selector(toggleRoaming(_:))
        ))
        menu.addItem(toggleItem(
            title: localized("menu.avoidPointer"),
            checked: runtime.isPointerAvoidanceEnabled,
            action: #selector(togglePointerAvoidance(_:))
        ))
        menu.addItem(toggleItem(
            title: localized("menu.catchDrag"),
            checked: runtime.areInteractionsEnabled,
            action: #selector(toggleInteractions(_:))
        ))

        let tuning = NSMenuItem(
            title: localized("menu.tuning"),
            action: #selector(showBehaviorTuning),
            keyEquivalent: ","
        )
        tuning.target = self
        menu.addItem(tuning)

        let claude = NSMenuItem(title: "Claude Code", action: nil, keyEquivalent: "")
        claude.submenu = makeClaudeCodeMenu(runtime: runtime)
        menu.addItem(claude)

        let codex = NSMenuItem(title: "Codex", action: nil, keyEquivalent: "")
        codex.submenu = makeCodexMenu(runtime: runtime)
        menu.addItem(codex)

        let accessibility = NSMenuItem(title: localized("menu.accessibility"), action: nil, keyEquivalent: "")
        accessibility.submenu = makeAccessibilityMenu(runtime: runtime)
        menu.addItem(accessibility)

        let visual = NSMenuItem(title: localized("menu.visualPlacement"), action: nil, keyEquivalent: "")
        visual.submenu = makeVisualPlacementMenu(runtime: runtime)
        menu.addItem(visual)

        menu.addItem(.separator())
        let openFolder = NSMenuItem(
            title: localized("menu.openPetFolder"),
            action: #selector(openPetFolder),
            keyEquivalent: ""
        )
        openFolder.target = self
        menu.addItem(openFolder)

        let diagnostics = NSMenuItem(
            title: localized("menu.copyDiagnostics"),
            action: #selector(copyDiagnostics),
            keyEquivalent: ""
        )
        diagnostics.target = self
        menu.addItem(diagnostics)

        let reload = NSMenuItem(title: localized("menu.reloadPets"), action: #selector(reloadPets), keyEquivalent: "r")
        reload.target = self
        menu.addItem(reload)
        menu.addItem(.separator())

        let about = NSMenuItem(
            title: localized("menu.about"),
            action: #selector(showAbout),
            keyEquivalent: ""
        )
        about.target = self
        menu.addItem(about)

        let quit = NSMenuItem(title: localized("menu.quit"), action: #selector(quit), keyEquivalent: "q")
        quit.target = self
        menu.addItem(quit)
    }

    private func toggleItem(title: String, checked: Bool, action: Selector) -> NSMenuItem {
        let item = NSMenuItem(title: title, action: action, keyEquivalent: "")
        item.target = self
        item.state = checked ? .on : .off
        return item
    }

    private func makeClaudeCodeMenu(runtime: RoamlingRuntime) -> NSMenu {
        let menu = NSMenu(title: "Claude Code")
        let integrationText = switch runtime.claudeCodeIntegrationStatus {
        case .installed: localized("status.hooks.installed")
        case .needsRepair: localized("status.hooks.needsRepair")
        case .notInstalled: localized("status.hooks.notInstalled")
        }
        let receiverText = switch runtime.claudeCodeReceiverState {
        case .ready: localized("status.receiver.ready")
        case .starting: localized("status.receiver.starting")
        case .stopped: localized("status.receiver.stopped")
        case .failed: localized("status.receiver.unavailable")
        }
        for text in [integrationText, receiverText] {
            let item = NSMenuItem(title: text, action: nil, keyEquivalent: "")
            item.isEnabled = false
            menu.addItem(item)
        }
        menu.addItem(.separator())

        let installTitle = runtime.claudeCodeIntegrationStatus == .notInstalled
            ? localized("action.install")
            : localized("action.repair")
        let install = NSMenuItem(
            title: installTitle,
            action: #selector(installClaudeCodeIntegration),
            keyEquivalent: ""
        )
        install.target = self
        menu.addItem(install)

        if runtime.claudeCodeIntegrationStatus != .notInstalled {
            let remove = NSMenuItem(
                title: localized("action.remove"),
                action: #selector(removeClaudeCodeIntegration),
                keyEquivalent: ""
            )
            remove.target = self
            menu.addItem(remove)
        }

        let test = NSMenuItem(
            title: localized("action.testReaction"),
            action: #selector(testClaudeCodeReaction),
            keyEquivalent: ""
        )
        test.target = self
        menu.addItem(test)
        return menu
    }

    private func makeAccessibilityMenu(runtime: RoamlingRuntime) -> NSMenu {
        let menu = NSMenu(title: localized("menu.accessibility"))
        let authorized = runtime.isAccessibilityAuthorized
        let status = NSMenuItem(
            title: authorized ? localized("accessibility.status.on") : localized("accessibility.status.off"),
            action: nil,
            keyEquivalent: ""
        )
        status.isEnabled = false
        menu.addItem(status)
        menu.addItem(.separator())

        if authorized {
            // macOS owns revocation; pointing at it beats a button that cannot
            // actually take the permission back.
            let hint = NSMenuItem(
                title: localized("accessibility.revoke.hint"),
                action: nil,
                keyEquivalent: ""
            )
            hint.isEnabled = false
            menu.addItem(hint)
        } else {
            let enable = NSMenuItem(
                title: localized("accessibility.enable"),
                action: #selector(enableAccessibility),
                keyEquivalent: ""
            )
            enable.target = self
            menu.addItem(enable)
        }
        return menu
    }

    private func makeVisualPlacementMenu(runtime: RoamlingRuntime) -> NSMenu {
        let menu = NSMenu(title: localized("menu.visualPlacement"))
        let authorized = runtime.isScreenCaptureAuthorized
        let status = NSMenuItem(
            title: authorized ? localized("visual.status.on") : localized("visual.status.off"),
            action: nil,
            keyEquivalent: ""
        )
        status.isEnabled = false
        menu.addItem(status)
        menu.addItem(.separator())

        if authorized {
            let hint = NSMenuItem(
                title: localized("visual.revoke.hint"),
                action: nil,
                keyEquivalent: ""
            )
            hint.isEnabled = false
            menu.addItem(hint)
        } else {
            let enable = NSMenuItem(
                title: localized("visual.enable"),
                action: #selector(enableVisualPlacement),
                keyEquivalent: ""
            )
            enable.target = self
            menu.addItem(enable)
        }
        return menu
    }

    private func makeCodexMenu(runtime: RoamlingRuntime) -> NSMenu {
        let menu = NSMenu(title: "Codex")
        let integrationText = switch runtime.codexIntegrationStatus {
        case .installed: localized("status.hooks.installed")
        case .needsRepair: localized("status.hooks.needsRepair")
        case .notInstalled: localized("status.hooks.notInstalled")
        }
        let receiverText = switch runtime.codexReceiverState {
        case .ready: localized("status.receiver.ready")
        case .starting: localized("status.receiver.starting")
        case .stopped: localized("status.receiver.stopped")
        case .failed: localized("status.receiver.unavailable")
        }
        for text in [integrationText, receiverText] {
            let item = NSMenuItem(title: text, action: nil, keyEquivalent: "")
            item.isEnabled = false
            menu.addItem(item)
        }
        menu.addItem(.separator())

        let installTitle = runtime.codexIntegrationStatus == .notInstalled
            ? localized("action.install")
            : localized("action.repair")
        let install = NSMenuItem(
            title: installTitle,
            action: #selector(installCodexIntegration),
            keyEquivalent: ""
        )
        install.target = self
        menu.addItem(install)

        if runtime.codexIntegrationStatus != .notInstalled {
            let remove = NSMenuItem(
                title: localized("action.remove"),
                action: #selector(removeCodexIntegration),
                keyEquivalent: ""
            )
            remove.target = self
            menu.addItem(remove)
        }

        let test = NSMenuItem(
            title: localized("action.testReaction"),
            action: #selector(testCodexReaction),
            keyEquivalent: ""
        )
        test.target = self
        menu.addItem(test)
        return menu
    }

    @objc private func toggleRoaming(_ sender: NSMenuItem) {
        runtime?.isRoamingEnabled.toggle()
        rebuildMenu()
    }

    @objc private func togglePointerAvoidance(_ sender: NSMenuItem) {
        runtime?.isPointerAvoidanceEnabled.toggle()
        rebuildMenu()
    }

    @objc private func toggleInteractions(_ sender: NSMenuItem) {
        runtime?.areInteractionsEnabled.toggle()
        rebuildMenu()
    }

    @objc private func selectBuiltInPet(_ sender: NSMenuItem) {
        guard let rawValue = sender.representedObject as? String,
              let kind = BuiltInPetKind(rawValue: rawValue) else { return }
        runtime?.useBuiltInPet(kind)
        rebuildMenu()
    }

    @objc private func showBehaviorTuning() {
        guard let runtime else { return }
        let controller: RuntimeTuningWindowController
        if let tuningWindowController {
            controller = tuningWindowController
        } else {
            let created = RuntimeTuningWindowController(tuning: runtime.tuning) { [weak runtime] tuning in
                runtime?.applyTuning(tuning)
            }
            tuningWindowController = created
            controller = created
        }
        controller.present(tuning: runtime.tuning)
    }

    @objc private func installClaudeCodeIntegration() {
        guard let runtime else { return }
        let alert = NSAlert()
        alert.messageText = localized("alert.claude.install.title")
        alert.informativeText = localized("alert.claude.install.body")
        alert.addButton(withTitle: localized("button.install"))
        alert.addButton(withTitle: localized("button.cancel"))
        guard alert.runModal() == .alertFirstButtonReturn else { return }
        presentIntegrationResult(runtime.installClaudeCodeIntegration(), success: localized("result.claude.installed"))
        rebuildMenu()
    }

    @objc private func removeClaudeCodeIntegration() {
        guard let runtime else { return }
        let alert = NSAlert()
        alert.messageText = localized("alert.claude.remove.title")
        alert.informativeText = localized("alert.claude.remove.body")
        alert.addButton(withTitle: localized("button.remove"))
        alert.addButton(withTitle: localized("button.cancel"))
        guard alert.runModal() == .alertFirstButtonReturn else { return }
        presentIntegrationResult(runtime.removeClaudeCodeIntegration(), success: localized("result.claude.removed"))
        rebuildMenu()
    }

    @objc private func testClaudeCodeReaction() {
        runtime?.testClaudeCodeReaction()
    }

    @objc private func installCodexIntegration() {
        guard let runtime else { return }
        let alert = NSAlert()
        alert.messageText = localized("alert.codex.install.title")
        alert.informativeText = localized("alert.codex.install.body")
        alert.addButton(withTitle: localized("button.install"))
        alert.addButton(withTitle: localized("button.cancel"))
        guard alert.runModal() == .alertFirstButtonReturn else { return }
        presentIntegrationResult(
            runtime.installCodexIntegration(),
            success: localized("result.codex.installed"),
            detail: localized("result.detail.codex.installed")
        )
        rebuildMenu()
    }

    @objc private func removeCodexIntegration() {
        guard let runtime else { return }
        let alert = NSAlert()
        alert.messageText = localized("alert.codex.remove.title")
        alert.informativeText = localized("alert.codex.remove.body")
        alert.addButton(withTitle: localized("button.remove"))
        alert.addButton(withTitle: localized("button.cancel"))
        guard alert.runModal() == .alertFirstButtonReturn else { return }
        presentIntegrationResult(
            runtime.removeCodexIntegration(),
            success: localized("result.codex.removed"),
            detail: localized("result.detail.codex.removed")
        )
        rebuildMenu()
    }

    @objc private func enableAccessibility() {
        guard let runtime else { return }
        let alert = NSAlert()
        alert.messageText = localized("accessibility.alert.title")
        alert.informativeText = localized("accessibility.alert.body")
        alert.addButton(withTitle: localized("button.openSystemSettings"))
        alert.addButton(withTitle: localized("button.cancel"))
        guard alert.runModal() == .alertFirstButtonReturn else { return }
        runtime.requestAccessibilityAuthorization()
        rebuildMenu()
    }

    @objc private func enableVisualPlacement() {
        guard let runtime else { return }
        let alert = NSAlert()
        alert.messageText = localized("visual.alert.title")
        alert.informativeText = localized("visual.alert.body")
        alert.addButton(withTitle: localized("button.openSystemSettings"))
        alert.addButton(withTitle: localized("button.cancel"))
        guard alert.runModal() == .alertFirstButtonReturn else { return }
        runtime.requestScreenCaptureAuthorization()
        rebuildMenu()
    }

    @objc private func testCodexReaction() {
        runtime?.testCodexReaction()
    }

    private func presentIntegrationResult(
        _ result: Result<Void, Error>,
        success: String,
        detail: String = localized("result.detail.claude")
    ) {
        let alert = NSAlert()
        switch result {
        case .success:
            alert.messageText = success
            alert.informativeText = detail
        case let .failure(error):
            alert.alertStyle = .warning
            alert.messageText = localized("error.claude.settings")
            alert.informativeText = error.localizedDescription
        }
        alert.runModal()
    }

    @objc private func selectInstalledPet(_ sender: NSMenuItem) {
        guard let path = sender.representedObject as? String, let runtime else { return }
        switch runtime.loadPet(at: URL(fileURLWithPath: path, isDirectory: true)) {
        case .success:
            rebuildMenu()
        case let .failure(error):
            let alert = NSAlert()
            alert.alertStyle = .warning
            alert.messageText = localized("error.pet.load")
            alert.informativeText = error.localizedDescription
            alert.runModal()
        }
    }

    @objc private func selectSize(_ sender: NSMenuItem) {
        guard let value = sender.representedObject as? NSNumber else { return }
        runtime?.setScale(value.doubleValue)
        rebuildMenu()
    }

    @objc private func openPetFolder() {
        let folder = FileManager.default.homeDirectoryForCurrentUser
            .appendingPathComponent("Library/Application Support/Roamling/Pets", isDirectory: true)
        do {
            try FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
            NSWorkspace.shared.open(folder)
        } catch {
            NSSound.beep()
        }
    }

    /// The pet cannot say why it is standing still, and from outside the app
    /// standing and sitting look the same. This is how that gets asked.
    @objc private func copyDiagnostics() {
        guard let runtime else { return }
        let pasteboard = NSPasteboard.general
        pasteboard.clearContents()
        pasteboard.setString(runtime.diagnosticsText, forType: .string)
    }

    @objc private func reloadPets() {
        runtime?.reloadCatalog()
        rebuildMenu()
    }

    @objc private func showAbout() {
        let alert = NSAlert()
        alert.messageText = "Roamling"
        alert.informativeText = localized("alert.about.body")
        alert.addButton(withTitle: localized("button.ok"))
        alert.addButton(withTitle: localized("menu.viewSource"))
        if alert.runModal() == .alertSecondButtonReturn,
           let sourceURL = URL(string: "https://github.com/creatorKoo/Roamling") {
            NSWorkspace.shared.open(sourceURL)
        }
    }

    @objc private func quit() {
        NSApp.terminate(nil)
    }

    @objc private func finishSmokeTest() {
        NSApp.terminate(nil)
    }
}
