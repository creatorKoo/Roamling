// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

import AppKit
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
        let runtime = RoamlingRuntime()
        self.runtime = runtime
        runtime.start()
        runtime.repairClaudeCodeIntegrationIfNeeded()
        setupMenuBar()

        // Starts the complete AppKit lifecycle for automated packaging checks,
        // then exits cleanly without needing a synthetic user interaction.
        if ProcessInfo.processInfo.environment["ROAMLING_SMOKE_TEST"] == "1" {
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

        let title = NSMenuItem(title: "Roamling — \(runtime.petDisplayName)", action: nil, keyEquivalent: "")
        title.isEnabled = false
        menu.addItem(title)
        menu.addItem(.separator())

        let petItem = NSMenuItem(title: "Pet", action: nil, keyEquivalent: "")
        let petMenu = NSMenu(title: "Pet")
        for kind in BuiltInPetKind.allCases {
            let builtIn = NSMenuItem(
                title: "\(kind.displayName) (Built-in)",
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
        petItem.submenu = petMenu
        menu.addItem(petItem)

        let sizeItem = NSMenuItem(title: "Size", action: nil, keyEquivalent: "")
        let sizeMenu = NSMenu(title: "Size")
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
            title: "Roaming",
            checked: runtime.isRoamingEnabled,
            action: #selector(toggleRoaming(_:))
        ))
        menu.addItem(toggleItem(
            title: "Avoid Pointer",
            checked: runtime.isPointerAvoidanceEnabled,
            action: #selector(togglePointerAvoidance(_:))
        ))
        menu.addItem(toggleItem(
            title: "Catch & Drag",
            checked: runtime.areInteractionsEnabled,
            action: #selector(toggleInteractions(_:))
        ))

        let tuning = NSMenuItem(
            title: "Behavior Tuning…",
            action: #selector(showBehaviorTuning),
            keyEquivalent: ","
        )
        tuning.target = self
        menu.addItem(tuning)

        let stretch = NSMenuItem(
            title: "Stretch Now",
            action: #selector(stretchNow),
            keyEquivalent: ""
        )
        stretch.target = self
        menu.addItem(stretch)

        let claude = NSMenuItem(title: "Claude Code", action: nil, keyEquivalent: "")
        claude.submenu = makeClaudeCodeMenu(runtime: runtime)
        menu.addItem(claude)

        let codex = NSMenuItem(title: "Codex", action: nil, keyEquivalent: "")
        codex.submenu = makeCodexMenu(runtime: runtime)
        menu.addItem(codex)

        let accessibility = NSMenuItem(title: "Accessibility", action: nil, keyEquivalent: "")
        accessibility.submenu = makeAccessibilityMenu(runtime: runtime)
        menu.addItem(accessibility)

        menu.addItem(.separator())
        let openFolder = NSMenuItem(
            title: "Open Pet Folder…",
            action: #selector(openPetFolder),
            keyEquivalent: ""
        )
        openFolder.target = self
        menu.addItem(openFolder)

        let reload = NSMenuItem(title: "Reload Pets", action: #selector(reloadPets), keyEquivalent: "r")
        reload.target = self
        menu.addItem(reload)
        menu.addItem(.separator())

        let about = NSMenuItem(
            title: "About Roamling",
            action: #selector(showAbout),
            keyEquivalent: ""
        )
        about.target = self
        menu.addItem(about)

        let quit = NSMenuItem(title: "Quit Roamling", action: #selector(quit), keyEquivalent: "q")
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
        case .installed: "Hooks: Installed"
        case .needsRepair: "Hooks: Needs Repair"
        case .notInstalled: "Hooks: Not Installed"
        }
        let receiverText = switch runtime.claudeCodeReceiverState {
        case .ready: "Receiver: Ready"
        case .starting: "Receiver: Starting"
        case .stopped: "Receiver: Stopped"
        case .failed: "Receiver: Unavailable"
        }
        for text in [integrationText, receiverText] {
            let item = NSMenuItem(title: text, action: nil, keyEquivalent: "")
            item.isEnabled = false
            menu.addItem(item)
        }
        menu.addItem(.separator())

        let installTitle = runtime.claudeCodeIntegrationStatus == .notInstalled
            ? "Install Integration…"
            : "Repair Integration…"
        let install = NSMenuItem(
            title: installTitle,
            action: #selector(installClaudeCodeIntegration),
            keyEquivalent: ""
        )
        install.target = self
        menu.addItem(install)

        if runtime.claudeCodeIntegrationStatus != .notInstalled {
            let remove = NSMenuItem(
                title: "Remove Integration…",
                action: #selector(removeClaudeCodeIntegration),
                keyEquivalent: ""
            )
            remove.target = self
            menu.addItem(remove)
        }

        let test = NSMenuItem(
            title: "Test Reaction",
            action: #selector(testClaudeCodeReaction),
            keyEquivalent: ""
        )
        test.target = self
        menu.addItem(test)
        return menu
    }

    private func makeAccessibilityMenu(runtime: RoamlingRuntime) -> NSMenu {
        let menu = NSMenu(title: "Accessibility")
        let authorized = runtime.isAccessibilityAuthorized
        let status = NSMenuItem(
            title: authorized ? "Caret Awareness: On" : "Caret Awareness: Off",
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
                title: "Turn off in System Settings › Privacy & Security",
                action: nil,
                keyEquivalent: ""
            )
            hint.isEnabled = false
            menu.addItem(hint)
        } else {
            let enable = NSMenuItem(
                title: "Enable Caret Awareness…",
                action: #selector(enableAccessibility),
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
        case .installed: "Hooks: Installed"
        case .needsRepair: "Hooks: Needs Repair"
        case .notInstalled: "Hooks: Not Installed"
        }
        let receiverText = switch runtime.codexReceiverState {
        case .ready: "Receiver: Ready"
        case .starting: "Receiver: Starting"
        case .stopped: "Receiver: Stopped"
        case .failed: "Receiver: Unavailable"
        }
        for text in [integrationText, receiverText] {
            let item = NSMenuItem(title: text, action: nil, keyEquivalent: "")
            item.isEnabled = false
            menu.addItem(item)
        }
        menu.addItem(.separator())

        let installTitle = runtime.codexIntegrationStatus == .notInstalled
            ? "Install Integration…"
            : "Repair Integration…"
        let install = NSMenuItem(
            title: installTitle,
            action: #selector(installCodexIntegration),
            keyEquivalent: ""
        )
        install.target = self
        menu.addItem(install)

        if runtime.codexIntegrationStatus != .notInstalled {
            let remove = NSMenuItem(
                title: "Remove Integration…",
                action: #selector(removeCodexIntegration),
                keyEquivalent: ""
            )
            remove.target = self
            menu.addItem(remove)
        }

        let test = NSMenuItem(
            title: "Test Reaction",
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

    @objc private func stretchNow() {
        runtime?.stretchNow()
    }

    @objc private func installClaudeCodeIntegration() {
        guard let runtime else { return }
        let alert = NSAlert()
        alert.messageText = "Install Claude Code integration?"
        alert.informativeText = """
        Roamling will add local lifecycle command hooks to ~/.claude/settings.json.
        Existing settings and hooks are preserved, and a one-time backup is created.

        Prompt text, tool input/output, transcripts, and source code are not stored or logged.
        """
        alert.addButton(withTitle: "Install")
        alert.addButton(withTitle: "Cancel")
        guard alert.runModal() == .alertFirstButtonReturn else { return }
        presentIntegrationResult(runtime.installClaudeCodeIntegration(), success: "Claude Code integration installed.")
        rebuildMenu()
    }

    @objc private func removeClaudeCodeIntegration() {
        guard let runtime else { return }
        let alert = NSAlert()
        alert.messageText = "Remove Claude Code integration?"
        alert.informativeText = "Only Roamling's hook handlers will be removed. Other Claude Code settings and hooks stay unchanged."
        alert.addButton(withTitle: "Remove")
        alert.addButton(withTitle: "Cancel")
        guard alert.runModal() == .alertFirstButtonReturn else { return }
        presentIntegrationResult(runtime.removeClaudeCodeIntegration(), success: "Claude Code integration removed.")
        rebuildMenu()
    }

    @objc private func testClaudeCodeReaction() {
        runtime?.testClaudeCodeReaction()
    }

    @objc private func installCodexIntegration() {
        guard let runtime else { return }
        let alert = NSAlert()
        alert.messageText = "Install Codex integration?"
        alert.informativeText = """
        Roamling will add local lifecycle command hooks to ~/.codex/hooks.json.
        Existing hooks and config.toml (including notify) are preserved, and a one-time backup is created.

        Prompt text, tool input/output, transcripts, and source code are not stored or logged.
        Restart Codex after installation and approve the new hook trust prompt.
        """
        alert.addButton(withTitle: "Install")
        alert.addButton(withTitle: "Cancel")
        guard alert.runModal() == .alertFirstButtonReturn else { return }
        presentIntegrationResult(
            runtime.installCodexIntegration(),
            success: "Codex integration installed.",
            detail: "Restart Codex and approve its new hook trust prompt."
        )
        rebuildMenu()
    }

    @objc private func removeCodexIntegration() {
        guard let runtime else { return }
        let alert = NSAlert()
        alert.messageText = "Remove Codex integration?"
        alert.informativeText = "Only Roamling's hook handlers will be removed. Other Codex hooks, config, and notify stay unchanged."
        alert.addButton(withTitle: "Remove")
        alert.addButton(withTitle: "Cancel")
        guard alert.runModal() == .alertFirstButtonReturn else { return }
        presentIntegrationResult(
            runtime.removeCodexIntegration(),
            success: "Codex integration removed.",
            detail: "Restart Codex sessions to stop using the removed hooks."
        )
        rebuildMenu()
    }

    @objc private func enableAccessibility() {
        guard let runtime else { return }
        let alert = NSAlert()
        alert.messageText = "Enable caret awareness?"
        alert.informativeText = """
        macOS will ask you to allow Roamling under Accessibility. Roamling then reads \
        the focused control's position and the text cursor's position so the pet can sit \
        near your work without covering it.

        It never reads what you type, the text you select, window titles, or document contents.
        """
        alert.addButton(withTitle: "Open System Settings")
        alert.addButton(withTitle: "Cancel")
        guard alert.runModal() == .alertFirstButtonReturn else { return }
        runtime.requestAccessibilityAuthorization()
        rebuildMenu()
    }

    @objc private func testCodexReaction() {
        runtime?.testCodexReaction()
    }

    private func presentIntegrationResult(
        _ result: Result<Void, Error>,
        success: String,
        detail: String = "New or resumed Claude Code sessions will now notify Roamling."
    ) {
        let alert = NSAlert()
        switch result {
        case .success:
            alert.messageText = success
            alert.informativeText = detail
        case let .failure(error):
            alert.alertStyle = .warning
            alert.messageText = "Couldn’t update Claude Code settings"
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
            alert.messageText = "Couldn’t load this pet"
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

    @objc private func reloadPets() {
        runtime?.reloadCatalog()
        rebuildMenu()
    }

    @objc private func showAbout() {
        let alert = NSAlert()
        alert.messageText = "Roamling"
        alert.informativeText = """
        A tiny companion that actually lives on your desktop.

        Copyright © 2026 GooBeom Jeoung
        Licensed under GNU GPL v3.0 only.
        This program comes with absolutely no warranty.

        Installed pet packages remain subject to their authors’ licenses.
        """
        alert.addButton(withTitle: "OK")
        alert.addButton(withTitle: "View Source")
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
