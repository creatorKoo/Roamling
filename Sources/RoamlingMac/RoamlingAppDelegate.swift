// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

import AppKit
import RoamlingPet

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
