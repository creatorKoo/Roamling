// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

import Foundation
import RoamlingCore
import RoamlingEngine
import RoamlingPet
import RoamlingShell

/// The menu used to be built straight into AppKit widgets, so nothing could
/// look at it without a screen. Now it is a value, and these are the first
/// tests it has ever had.
func shellLogicTests() -> [LogicTest] {
    [
        LogicTest(name: "a failed install names the agent that failed") {
            // The failure title was hard-coded to Claude Code, so a Codex
            // install that went wrong said "Couldn't update Claude Code
            // settings" -- pointing the user at a file that was never touched.
            try MainActor.assumeIsolated {
                for (id, expected) in [
                    ("claude-code", "error.claude.settings"),
                    ("codex", "error.codex.hooks")
                ] {
                    let agent = FailingAgent(id: id)
                    let platform = FakePlatform(
                        display: DisplaySnapshot(
                            id: "1", name: "test",
                            frame: WorldRect(x: 0, y: 0, width: 1440, height: 900),
                            visibleFrame: WorldRect(x: 0, y: 25, width: 1440, height: 850),
                            scale: 2
                        ),
                        worldTop: 900
                    )
                    let suite = try makeTestDefaults()
                    defer { suite.discard() }
                    let runtime = RoamlingRuntime(
                        services: platform.services,
                        agents: [agent],
                        defaults: suite.defaults,
                        catalog: PetCatalog(roots: []),
                        clock: { 0 }
                    )
                    let effect = ShellController.perform(
                        .installAgent(id: id), runtime: runtime, version: "1.2.3"
                    )
                    guard case let .presentThenRebuild(alert) = effect else {
                        throw LogicTestFailure(
                            message: "expected an alert for \(id), got \(effect)",
                            file: #filePath, line: #line
                        )
                    }
                    try expect(
                        alert.title == localized(expected),
                        "\(id) reported \(alert.title), expected \(localized(expected))"
                    )
                    try expect(alert.isWarning, "a failure should read as one")
                }
            }
        },
        LogicTest(name: "the about box says which build it is") {
            try MainActor.assumeIsolated {
                let alert = ShellPrompt.about(version: "9.8.7")
                try expect(
                    alert.body.hasPrefix(localizedFormat("about.version", "9.8.7")),
                    "the version is not the first line: \(alert.body.prefix(40))"
                )
            }
        },
        LogicTest(name: "the menu offers every pet and marks the one in use") {
            try MainActor.assumeIsolated {
                let harness = try RuntimeHarness()
                defer { harness.tearDown() }
                let runtime = harness.runtime

                let pets = try require(submenu(named: localized("menu.pet"), in: ShellMenu.items(for: runtime)))
                let checked = pets.compactMap { item -> String? in
                    if case let .check(_, isOn) = item.content, isOn { return item.title }
                    return nil
                }
                try expect(checked.count == 1, "expected exactly one pet checked, got \(checked)")

                // Every built-in is offered, and picking one moves the check.
                let offered = pets.compactMap { item -> MenuAction? in
                    if case let .check(action, _) = item.content { return action }
                    return nil
                }
                for kind in BuiltInPetKind.allCases {
                    try expect(
                        offered.contains(.selectBuiltInPet(kind)),
                        "\(kind) is not in the menu"
                    )
                }
                let other = try require(BuiltInPetKind.allCases.first { $0 != runtime.selectedBuiltInPet })
                _ = ShellController.perform(.selectBuiltInPet(other), runtime: runtime, version: "1.2.3")
                let after = try require(submenu(named: localized("menu.pet"), in: ShellMenu.items(for: runtime)))
                let nowChecked = after.first { item in
                    if case let .check(action, isOn) = item.content {
                        return isOn && action == .selectBuiltInPet(other)
                    }
                    return false
                }
                try expect(nowChecked != nil, "picking a pet did not move the checkmark")
            }
        },
        LogicTest(name: "the size menu offers exactly the scales the runtime accepts") {
            try MainActor.assumeIsolated {
                let harness = try RuntimeHarness()
                defer { harness.tearDown() }
                let runtime = harness.runtime

                for choice in ShellMenu.scaleChoices {
                    _ = ShellController.perform(.setScale(choice.value), runtime: runtime, version: "1.2.3")
                    try expect(
                        abs(runtime.scale - choice.value) < 0.001,
                        "\(choice.label) was offered but the runtime settled on \(runtime.scale)"
                    )
                    let sizes = try require(submenu(named: localized("menu.size"), in: ShellMenu.items(for: runtime)))
                    let checked = sizes.filter { item in
                        if case let .check(_, isOn) = item.content { return isOn }
                        return false
                    }
                    try expect(checked.count == 1, "\(choice.label): \(checked.count) items checked")
                    try expect(checked[0].title == choice.label)
                }
            }
        },
        LogicTest(name: "the three switches report the state they toggle") {
            try MainActor.assumeIsolated {
                let harness = try RuntimeHarness()
                defer { harness.tearDown() }
                let runtime = harness.runtime

                let switches: [(MenuAction, String, () -> Bool)] = [
                    (.toggleRoaming, localized("menu.roaming"), { runtime.isRoamingEnabled }),
                    (.togglePointerAvoidance, localized("menu.avoidPointer"), { runtime.isPointerAvoidanceEnabled }),
                    (.toggleInteractions, localized("menu.catchDrag"), { runtime.areInteractionsEnabled })
                ]
                for (action, title, read) in switches {
                    for _ in 0..<2 {
                        let before = read()
                        _ = ShellController.perform(action, runtime: runtime, version: "1.2.3")
                        try expect(read() != before, "\(title) did not change")
                        let item = try require(
                            ShellMenu.items(for: runtime).first { $0.title == title },
                            "\(title) is missing from the menu"
                        )
                        var isOn = false
                        if case let .check(_, on) = item.content { isOn = on } else {
                            try expect(false, "\(title) is not a checkable item")
                        }
                        try expect(isOn == read(), "\(title) shows \(isOn) but reads \(read())")
                    }
                }
            }
        },
        LogicTest(name: "destructive actions ask first and harmless ones do not") {
            try MainActor.assumeIsolated {
                let asks: [MenuAction] = [
                    .installAgent(id: "claude-code"), .removeAgent(id: "claude-code"),
                    .installAgent(id: "codex"), .removeAgent(id: "codex"),
                    .enableAccessibility, .enableVisualPlacement
                ]
                for action in asks {
                    let alert = try require(
                        ShellPrompt.confirmation(for: action),
                        "\(action) touches the user's settings or permissions without asking"
                    )
                    try expect(alert.buttons.count == 2, "\(action): expected confirm and cancel")
                    try expect(!alert.title.isEmpty && !alert.body.isEmpty)
                }
                let doesNotAsk: [MenuAction] = [
                    .toggleRoaming, .togglePointerAvoidance, .toggleInteractions,
                    .showTuning, .reloadPets, .copyDiagnostics, .openPetFolder,
                    .testAgentReaction(id: "claude-code"), .testAgentReaction(id: "codex"),
                    .showAbout, .quit,
                    .setScale(1.0)
                ]
                for action in doesNotAsk {
                    try expect(
                        ShellPrompt.confirmation(for: action) == nil,
                        "\(action) stops to ask about something reversible"
                    )
                }
            }
        },
        LogicTest(name: "every menu command carries a title and a shortcut that is one key") {
            try MainActor.assumeIsolated {
                let harness = try RuntimeHarness()
                defer { harness.tearDown() }
                var checked = 0
                func walk(_ items: [MenuItem]) throws {
                    for item in items {
                        switch item.content {
                        case .separator:
                            try expect(item.title.isEmpty, "a separator carries a title")
                        case let .submenu(children):
                            try expect(!item.title.isEmpty)
                            try walk(children)
                        case .caption, .command, .check:
                            checked += 1
                            try expect(!item.title.isEmpty, "an item with no title")
                            // A missing key in the strings file comes back as
                            // the key itself, which is how that shows up.
                            try expect(
                                !item.title.hasPrefix("menu.") && !item.title.hasPrefix("action."),
                                "untranslated key on screen: \(item.title)"
                            )
                            try expect(item.shortcut.count <= 1, "\(item.title) has a multi-key shortcut")
                        }
                    }
                }
                try walk(ShellMenu.items(for: harness.runtime))
                try expect(checked > 20, "only \(checked) items -- the menu lost most of itself")
            }
        }
    ]
}

private func submenu(named title: String, in items: [MenuItem]) -> [MenuItem]? {
    for item in items {
        if item.title == title, case let .submenu(children) = item.content { return children }
    }
    return nil
}
