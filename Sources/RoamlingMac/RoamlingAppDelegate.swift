// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

import AppKit
import RoamlingEngine
import RoamlingPet
import RoamlingShell
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
        let runtime = RoamlingRuntime(
            services: MacPlatform.makeServices(),
            agents: MacPlatform.makeAgentIntegrations()
        )
        self.runtime = runtime
        runtime.start()
        runtime.repairAgentIntegrationsIfNeeded()
        setupMenuBar()

        // Starts the complete AppKit lifecycle for automated packaging checks,
        // then exits cleanly without needing a synthetic user interaction.
        if ProcessInfo.processInfo.environment["ROAMLING_SMOKE_TEST"] == "1" {
            // A missing CFBundleLocalizations pins the whole process to English
            // no matter what ships in the module bundle, and nothing crashes to
            // say so. Print what actually resolved.
            print("smoke.localization=\(resolvedLocalization) sample=\(localized("menu.quit"))")
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
        render(ShellMenu.items(for: runtime), into: menu)
    }

    /// Turns the shell's tree into AppKit widgets. This is the whole of what
    /// macOS contributes to the menu; the tree, the words and what each item
    /// does are in `RoamlingShell`, where a Windows tray reads the same ones.
    private func render(_ items: [MenuItem], into menu: NSMenu) {
        for item in items {
            switch item.content {
            case .separator:
                menu.addItem(.separator())
            case .caption:
                let widget = NSMenuItem(title: item.title, action: nil, keyEquivalent: "")
                widget.isEnabled = false
                menu.addItem(widget)
            case let .submenu(children):
                let widget = NSMenuItem(title: item.title, action: nil, keyEquivalent: "")
                let submenu = NSMenu(title: item.title)
                render(children, into: submenu)
                widget.submenu = submenu
                menu.addItem(widget)
            case let .command(action):
                menu.addItem(widget(item, action: action, isOn: false))
            case let .check(action, isOn):
                menu.addItem(widget(item, action: action, isOn: isOn))
            }
        }
    }

    private func widget(_ item: MenuItem, action: MenuAction, isOn: Bool) -> NSMenuItem {
        let widget = NSMenuItem(
            title: item.title,
            action: #selector(runMenuAction(_:)),
            keyEquivalent: item.shortcut
        )
        widget.target = self
        widget.representedObject = MenuActionBox(action)
        widget.state = isOn ? .on : .off
        return widget
    }

    @objc private func runMenuAction(_ sender: NSMenuItem) {
        guard let runtime, let box = sender.representedObject as? MenuActionBox else { return }
        let action = box.action
        if let confirmation = ShellPrompt.confirmation(for: action),
           present(confirmation) != 0 {
            return
        }
        apply(ShellController.perform(action, runtime: runtime, version: Self.version))
    }

    /// The shipped version, from `Support/Info.plist`. `build-app.sh` copies
    /// that file into the bundle, and `rust/Cargo.toml` is what it follows.
    private static var version: String {
        Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String
            ?? "0.0.0"
    }

    private func apply(_ effect: ShellEffect) {
        switch effect {
        case .none:
            break
        case .rebuildMenu:
            rebuildMenu()
        case let .present(alert):
            handle(alert, chosen: present(alert))
        case let .presentThenRebuild(alert):
            handle(alert, chosen: present(alert))
            rebuildMenu()
        case .openTuningPanel:
            showBehaviorTuning()
        case let .reveal(folder):
            do {
                try FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
                NSWorkspace.shared.open(folder)
            } catch {
                NSSound.beep()
            }
        case let .openLink(url):
            NSWorkspace.shared.open(url)
        case let .copyToClipboard(text):
            let pasteboard = NSPasteboard.general
            pasteboard.clearContents()
            pasteboard.setString(text, forType: .string)
        case .quit:
            NSApp.terminate(nil)
        }
    }

    /// Index of the button the user chose, so a caller can branch without
    /// knowing what `NSApplication.ModalResponse` is.
    @discardableResult
    private func present(_ alert: AlertModel) -> Int {
        let panel = NSAlert()
        panel.messageText = alert.title
        panel.informativeText = alert.body
        if alert.isWarning { panel.alertStyle = .warning }
        for button in alert.buttons { panel.addButton(withTitle: button) }
        return panel.runModal().rawValue - NSApplication.ModalResponse.alertFirstButtonReturn.rawValue
    }

    /// The one alert whose second button does something.
    private func handle(_ alert: AlertModel, chosen: Int) {
        if alert == ShellPrompt.about(version: Self.version), chosen == 1 {
            apply(.openLink(ShellPrompt.sourceURL))
        }
    }

    private func showBehaviorTuning() {
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

    @objc private func finishSmokeTest() {
        NSApp.terminate(nil)
    }
}

/// `representedObject` holds an `Any`, and a Swift enum with payloads is not
/// one AppKit can carry. This is the box.
private final class MenuActionBox: NSObject {
    let action: MenuAction
    init(_ action: MenuAction) { self.action = action }
}
