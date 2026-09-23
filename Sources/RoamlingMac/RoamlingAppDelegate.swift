// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

import AppKit
import RoamlingEngine
import RoamlingPet
import RoamlingShell
import RoamlingSources
import ServiceManagement

@MainActor
public final class RoamlingAppDelegate: NSObject, NSApplicationDelegate, NSMenuDelegate {
    private var runtime: RoamlingRuntime?
    private var paletteWindowController: PaletteWindowController?
    private var statusItem: NSStatusItem?
    private var tuningWindowController: RuntimeTuningWindowController?
    private var usageGuideWindowController: UsageGuideWindowController?

    public override init() {
        super.init()
    }

    public func applicationDidFinishLaunching(_ notification: Notification) {
        // A copy started by an update waits here until the one it replaces has
        // gone, so the two never hold the agent ports or the pet at once.
        MacUpdater.waitForPreviousInstance()
        MacUpdater.discardLeftovers()
        NSApp.setActivationPolicy(.accessory)
        let runtime = RoamlingRuntime(
            services: MacPlatform.makeServices(),
            agents: MacPlatform.makeAgentIntegrations()
        )
        self.runtime = runtime
        runtime.start()
        runtime.repairAgentIntegrationsIfNeeded()
        UserDefaults.standard.register(defaults: [Self.automaticUpdatesKey: true])
        ShellMenu.automaticUpdates = UserDefaults.standard.bool(forKey: Self.automaticUpdatesKey)
        offerLaunchAtLoginOnce()
        setupMenuBar()
        scheduleUpdateChecks()
        if ProcessInfo.processInfo.environment["ROAMLING_SMOKE_TEST"] != "1" {
            showUsageGuide(manual: false)
        }

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
        usageGuideWindowController?.prepareForTermination()
        // A version still waiting for a quiet moment goes in on the way out:
        // no process is left behind to lose the screen, and the next launch
        // is the new one.
        if let version = updater.staged {
            do {
                try updater.install()
                runtime?.recordUpdate("installed \(version) at quit")
            } catch {
                runtime?.recordUpdate("install at quit failed: \(error.localizedDescription)")
            }
        }
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
        // Asked every time rather than stored: System Settings can change it
        // behind our back, and a stale checkmark is a lie.
        ShellMenu.launchAtLogin = SMAppService.mainApp.status == .enabled
        menu.removeAllItems()
        // Read once, here, because the menu is rebuilt on every open. The
        // alternative -- `NSMenuItem.isAlternate` -- needs a row to stand in
        // front of, and the row it hides is the last one in its submenu.
        let alternateHeld = NSEvent.modifierFlags.contains(.option)
        render(ShellMenu.items(for: runtime, alternateHeld: alternateHeld), into: menu)
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
            case let .submenu(children, isOn):
                let widget = NSMenuItem(title: item.title, action: nil, keyEquivalent: "")
                let submenu = NSMenu(title: item.title)
                render(children, into: submenu)
                widget.submenu = submenu
                // A row can open a submenu and still say it is the one in use.
                widget.state = isOn ? .on : .off
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

    /// The updater lives here rather than in the runtime: fetching bytes and
    /// replacing a bundle are this platform's, and the decisions inside them
    /// are already shared with Windows in `roamling-update`.
    private let updater = MacUpdater()
    private var updateTimer: Timer?
    /// Runs only while a staged version waits for the pet to be idle.
    private var restartTimer: Timer?

    private static let automaticUpdatesKey = "roamling.automaticUpdates"
    private static let launchAtLoginOfferedKey = "roamling.launchAtLoginOffered"

    /// A companion that only shows up when you remember to open it is not much
    /// of a companion, so the first bundled launch registers the login item.
    /// Once: the key says the offer was made, and turning it off afterwards
    /// (menu or System Settings) is the user's answer, never re-asked. Nothing
    /// is presented here -- a first launch is not the moment for a modal, and
    /// the menu already shows what the OS decided.
    private func offerLaunchAtLoginOnce() {
        let defaults = UserDefaults.standard
        guard !defaults.bool(forKey: Self.launchAtLoginOfferedKey) else { return }
        // Not a bundle (`swift run`): nothing to register, and the offer must
        // wait for the first run that is one. Judged by shape, not by
        // `status` -- a signed, never-registered .app also reads `.notFound`.
        guard Bundle.main.bundleURL.pathExtension == "app" else { return }
        // The release and rehearsal workflows really launch the packaged app;
        // a runner must not end up with a login item. Leave the key unset so
        // a real first launch still gets the offer.
        guard ProcessInfo.processInfo.environment["ROAMLING_SMOKE_TEST"] != "1" else { return }
        let service = SMAppService.mainApp
        switch service.status {
        case .enabled, .requiresApproval:
            // Already answered, by the user or by System Settings.
            break
        default:
            try? service.register()
        }
        defaults.set(true, forKey: Self.launchAtLoginOfferedKey)
    }

    /// One `SMAppService` call either way. Nothing is remembered locally: the
    /// OS keeps the login item, and the menu reads it back on every build. A
    /// binary that is not a bundle (`swift run`) fails here by design.
    private func setLaunchAtLogin(_ enabled: Bool) {
        let service = SMAppService.mainApp
        do {
            if enabled {
                try service.register()
                // The user turned it off in System Settings earlier; the OS
                // will not silently override that, so take them there.
                if service.status == .requiresApproval {
                    SMAppService.openSystemSettingsLoginItems()
                }
            } else {
                try service.unregister()
            }
        } catch {
            // Turned off in System Settings earlier: register() throws
            // "Operation not permitted" rather than overriding that choice.
            // The fix is there, not in an alert.
            if service.status == .requiresApproval {
                SMAppService.openSystemSettingsLoginItems()
            } else {
                present(ShellPrompt.launchAtLoginFailure(detail: error.localizedDescription))
            }
        }
    }

    /// A day. A desktop pet checking more often than that is spending the
    /// user's battery to find out nothing, which `docs/battery.md` would have
    /// something to say about.
    private static let updateInterval: TimeInterval = 24 * 60 * 60

    /// Runs the first check a little after launch rather than during it: the
    /// pet appearing is what the user is waiting for, and a network round trip
    /// is not part of that.
    private func scheduleUpdateChecks() {
        updateTimer?.invalidate()
        updateTimer = nil
        guard ShellMenu.automaticUpdates else { return }
        let timer = Timer(timeInterval: Self.updateInterval, repeats: true) { [weak self] _ in
            MainActor.assumeIsolated { self?.checkForUpdates(asked: false) }
        }
        updateTimer = timer
        RunLoop.main.add(timer, forMode: .common)
        DispatchQueue.main.asyncAfter(deadline: .now() + 10) { [weak self] in
            guard let self, ShellMenu.automaticUpdates else { return }
            self.checkForUpdates(asked: false)
        }
    }

    /// - Parameter asked: whether the user asked. A background check that finds
    ///   nothing is silent, which is what `Never annoying` requires of
    ///   something that runs on a timer all day.
    private func checkForUpdates(asked: Bool) {
        // One is already unpacked and waiting; fetching it again finds nothing new.
        guard updater.staged == nil else { return }
        ShellMenu.updateStatus = .checking
        rebuildMenu()
        updater.check { [weak self] outcome in
            guard let self else { return }
            // The menu is put right whatever happened. Only whether to
            // interrupt the user depends on who asked.
            switch outcome {
            case .upToDate, .failed:
                ShellMenu.updateStatus = .idle
            case let .staged(version):
                ShellMenu.updateStatus = .staged(version: version)
            }
            self.rebuildMenu()

            switch outcome {
            case let .upToDate(current):
                if asked { self.apply(.present(ShellPrompt.updateResult(upToDate: current))) }
            case let .staged(version):
                // No alert: the pet blinking back as the new version is the news.
                // Someone who asked does not wait for a quiet moment.
                self.runtime?.recordUpdate("staged \(version)")
                if asked { self.restartIntoUpdate() } else { self.waitForQuietMoment() }
            case let .failed(detail):
                self.runtime?.recordUpdate("check failed: \(detail)")
                if asked { self.apply(.present(ShellPrompt.updateFailure(detail))) }
            }
        }
    }

    /// Looks every two seconds, which is plenty: the pet stands idle for tens
    /// of seconds between strolls. `.default` rather than `.common`, so it
    /// holds off while a menu is open or an alert is up.
    private func waitForQuietMoment() {
        restartTimer?.invalidate()
        let timer = Timer(timeInterval: 2, repeats: true) { [weak self] _ in
            MainActor.assumeIsolated {
                guard let self, self.isQuietForRestart else { return }
                self.restartIntoUpdate()
            }
        }
        restartTimer = timer
        RunLoop.main.add(timer, forMode: .default)
    }

    /// The pet's side is the core's to judge; a window of ours the user has
    /// open is this side's -- restarting would close it under them.
    private var isQuietForRestart: Bool {
        guard runtime?.isQuietForRestart == true else { return false }
        let windows = [
            tuningWindowController?.window,
            paletteWindowController?.window,
            usageGuideWindowController?.window,
        ]
        return !windows.contains { $0?.isVisible == true }
    }

    /// Swaps the new version in and starts it, together. Only on the way out:
    /// the process left behind by a swap can no longer capture the screen
    /// (`docs/capture.md` section 2).
    private func restartIntoUpdate() {
        restartTimer?.invalidate()
        restartTimer = nil
        guard let version = updater.staged else { return }
        do {
            try updater.install()
        } catch {
            runtime?.recordUpdate("install failed: \(error.localizedDescription)")
            ShellMenu.updateStatus = .idle
            rebuildMenu()
            return
        }
        runtime?.recordUpdate("restarting into \(version)")
        updater.relaunch { [weak self] error in
            guard let error else {
                NSApp.terminate(nil)
                return
            }
            // The new bundle is in place and nothing started it. A pet that
            // stays is better than one that vanishes unexplained; the next
            // launch, by hand or at login, is the new version.
            self?.runtime?.recordUpdate("relaunch failed: \(error.localizedDescription)")
        }
    }

    private func showPaletteMixer() {
        guard let runtime else { return }
        let controller = paletteWindowController ?? PaletteWindowController(runtime: runtime)
        paletteWindowController = controller
        controller.present()
    }

    private func showUsageGuide(manual: Bool) {
        let defaults = UserDefaults.standard
        guard let guide = UsageGuide.bundled(),
              let page = guide.page(seen: defaults.integer(forKey: UsageGuide.seenKey), manual: manual)
        else { return }
        let controller = usageGuideWindowController ?? UsageGuideWindowController()
        usageGuideWindowController = controller
        controller.present(page, manual: manual) {
            guard ProcessInfo.processInfo.environment["ROAMLING_SMOKE_TEST"] != "1" else { return }
            defaults.set(max(defaults.integer(forKey: UsageGuide.seenKey), page.revision),
                         forKey: UsageGuide.seenKey)
        }
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
        case .openUsageGuide:
            showUsageGuide(manual: true)
        case .openPaletteMixer:
            showPaletteMixer()
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
        case .checkForUpdates:
            checkForUpdates(asked: true)
        case let .setAutomaticUpdates(enabled):
            ShellMenu.automaticUpdates = enabled
            UserDefaults.standard.set(enabled, forKey: Self.automaticUpdatesKey)
            scheduleUpdateChecks()
            rebuildMenu()
        case let .setLaunchAtLogin(enabled):
            setLaunchAtLogin(enabled)
            rebuildMenu()
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
