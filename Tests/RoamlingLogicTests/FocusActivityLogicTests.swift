// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

import Foundation
import RoamlingCore
import RoamlingEngine
import RoamlingPet
import RoamlingShell

/// The activity source that is not an agent: the app the user is working in.
///
/// The rules are pinned in `focus_activity.rs`, where a day of desk work costs
/// no wall time. What is here is the wiring -- that the shell samples the right
/// facts, that the events reach the director on the ordinary path, and that
/// the pet ends up wearing the pictures the user was promised.
func focusActivityLogicTests() -> [LogicTest] {
    [
        LogicTest(name: "a named work app brings the pet over to sit, and nothing more") {
            try MainActor.assumeIsolated {
                let scene = try WorkAppScene()
                defer { scene.tearDown() }

                // Nobody is typing: the app is merely in front.
                scene.platform.window.frontmost = WorkAppScene.editor
                let walk = try scene.walkAndSettle(within: 40)

                // And it stays sitting, for longer than the typing window and
                // the release after it together. Reading is not a reason for
                // the pet to ask anything.
                let sat = scene.run(seconds: 20)
                let worn = walk.worn + sat.worn
                for unwanted in [PetCapability.spark, .paw, .work] {
                    try expect(
                        !worn.contains(unwanted),
                        "the app alone dressed the pet in \(unwanted): \(worn)"
                    )
                }
                try expect(!sat.walked, "the seated pet got up again: \(sat.worn)")
                try expect(
                    scene.runtime.behaviorState == .idle,
                    "a seated pet with nothing to show is \(scene.runtime.behaviorState)"
                )
                try expect(
                    scene.distanceToEditor < scene.distanceToAgentWindow,
                    "the pet sat down away from the editor, at \(scene.runtime.position)"
                )
                try expect(
                    scene.lastDiagnostic("agent")?.hasPrefix("focus:\(WorkAppScene.editor)") == true,
                    "the seat is not the work app's: \(String(describing: scene.lastDiagnostic("agent")))"
                )
            }
        },

        LogicTest(name: "the first keystroke hops, ten quiet seconds ask, and five more give the seat back") {
            try MainActor.assumeIsolated {
                let scene = try WorkAppScene(roaming: true)
                defer { scene.tearDown() }
                scene.platform.window.frontmost = WorkAppScene.editor
                try scene.walkAndSettle(within: 40)

                // Typing. Same app, same window -- only the keyboard changed.
                scene.platform.userIdle.keyboardDuration = 0.2
                let typing = scene.run(seconds: 5) { runtime, _ in runtime.behaviorState == .work }
                let spark = try require(
                    typing.worn.firstIndex(of: .spark),
                    "no hop for the session's first keystroke; it wore \(typing.worn)"
                )
                let work = try require(
                    typing.worn.firstIndex(of: .work),
                    "the pet never went to work; it wore \(typing.worn)"
                )
                try expect(spark < work, "the hop came after the work picture: \(typing.worn)")
                try expect(!typing.walked, "the seated pet walked off to hop")
                try expect(
                    scene.diagnostics("pet").contains("spark"),
                    "the hop is missing from the diagnostics the user can copy"
                )

                // Hands off. Nine seconds is still working, because a pause for
                // thought is not the end of anything.
                scene.platform.userIdle.keyboardDuration = 9.0
                scene.run(seconds: 3)
                try expect(
                    scene.runtime.behaviorState == .work,
                    "a nine-second pause ended the work picture"
                )

                scene.platform.userIdle.keyboardDuration = 11.0
                scene.run(seconds: 2) { runtime, _ in runtime.behaviorState == .waitingForUser }
                try expect(
                    scene.runtime.behaviorState == .waitingForUser,
                    "ten quiet seconds did not ask; the pet is \(scene.runtime.behaviorState)"
                )
                try expect(scene.runtime.currentCapability == .paw)

                // Asking is a question, not a vigil.
                scene.run(seconds: 4)
                try expect(
                    scene.runtime.behaviorState == .waitingForUser,
                    "the pet stopped asking early; it is \(scene.runtime.behaviorState)"
                )
                scene.run(seconds: 3) { runtime, _ in runtime.behaviorState != .waitingForUser }
                try expect(
                    scene.runtime.behaviorState != .waitingForUser,
                    "the pet kept asking past the release"
                )
                // One more tick: the log is written inside the tick, and the
                // release may have landed after this one wrote it.
                scene.run(seconds: 1.0 / 30)
                try expect(
                    scene.lastDiagnostic("agent") == "none",
                    "the seat was not given back: \(String(describing: scene.lastDiagnostic("agent")))"
                )

                // Its own day again.
                scene.run(seconds: 10) { runtime, _ in runtime.behaviorState == .wander }
                try expect(
                    scene.runtime.behaviorState == .wander,
                    "the released pet never went back to roaming; it is \(scene.runtime.behaviorState)"
                )

                // And typing brings it back to work, without a second hop.
                scene.platform.userIdle.keyboardDuration = 0.2
                let back = scene.run(seconds: 30) { runtime, _ in
                    runtime.behaviorState == .work && !runtime.isPlacementTravelling
                }
                try expect(
                    scene.runtime.behaviorState == .work,
                    "typing did not bring the pet back; it is \(scene.runtime.behaviorState), wore \(back.worn)"
                )
                try expect(!back.worn.contains(.spark), "the same session hopped twice: \(back.worn)")
            }
        },

        LogicTest(name: "beside a working agent the work app neither hops, runs nor asks") {
            try MainActor.assumeIsolated {
                // The ordinary developer desk: an agent already has the seat,
                // at a window that is not the editor's.
                let scene = try WorkAppScene(withSeatedAgent: true)
                defer { scene.tearDown() }
                try expect(
                    scene.distanceToAgentWindow < scene.distanceToEditor,
                    "the agent's seat is not by the agent's window: \(scene.runtime.position)"
                )

                // The editor comes forward and the user types into it, for
                // longer than attention counts the agent's last word. As a
                // score, the typing took the seat once that word aged out.
                scene.platform.window.frontmost = WorkAppScene.editor
                scene.platform.userIdle.keyboardDuration = 0.2
                let typing = scene.run(seconds: 45)
                try expect(!typing.walked, "typing took the pet from the agent: \(typing.worn)")
                try expect(
                    !typing.worn.contains(.spark),
                    "the pet hopped for the editor beside a working agent: \(typing.worn)"
                )
                try expect(
                    scene.distanceToAgentWindow < scene.distanceToEditor,
                    "the pet left the agent's window, for \(scene.runtime.position)"
                )
                try expect(
                    scene.lastDiagnostic("agent")?.hasPrefix(WorkAppScene.agentSource) == true,
                    "the agent lost its seat: \(String(describing: scene.lastDiagnostic("agent")))"
                )

                // A tool call, and then the user stops typing. The question is
                // urgent to attention, and as a score it took a working agent's
                // seat on the spot.
                scene.emitAgent(.highIntensity, intensity: 0.8)
                drainActivityEvents()
                scene.platform.userIdle.keyboardDuration = 11
                let quiet = scene.run(seconds: 20)
                try expect(!quiet.walked, "the question took the pet from the agent: \(quiet.worn)")
                try expect(
                    !quiet.worn.contains(.paw),
                    "the pet asked for the editor beside a working agent: \(quiet.worn)"
                )
                try expect(
                    scene.lastDiagnostic("agent")?.hasPrefix(WorkAppScene.agentSource) == true,
                    "the agent lost its seat: \(String(describing: scene.lastDiagnostic("agent")))"
                )
            }
        },

        LogicTest(name: "when the agent finishes, the work app takes the seat: walk, hop, then work") {
            try MainActor.assumeIsolated {
                let scene = try WorkAppScene(withSeatedAgent: true)
                defer { scene.tearDown() }

                // Typing into the editor while the agent works, for longer
                // than attention would remember any single thing said.
                scene.platform.window.frontmost = WorkAppScene.editor
                scene.platform.userIdle.keyboardDuration = 0.2
                scene.run(seconds: 40)
                try expect(
                    scene.distanceToAgentWindow < scene.distanceToEditor,
                    "the pet left a working agent, for \(scene.runtime.position)"
                )

                // The turn ends. What the desk said while it waited is the
                // next candidate, and the hop the pet never got is still owed.
                scene.emitAgent(.achievement, intensity: 0.55)
                drainActivityEvents()
                let handover = scene.run(seconds: 30) { runtime, recording in
                    recording.walked && recording.worn.contains(.spark)
                        && runtime.behaviorState == .work && !runtime.isPlacementTravelling
                }
                try expect(handover.walked, "the pet never walked to the editor: \(handover.worn)")
                let spark = try require(
                    handover.worn.firstIndex(of: .spark),
                    "no hop once the seat was free; it wore \(handover.worn)"
                )
                try expect(
                    handover.worn[spark...].contains(.work),
                    "the work picture never followed the hop: \(handover.worn)"
                )
                try expect(
                    scene.distanceToEditor < scene.distanceToAgentWindow,
                    "the pet ended by the agent's window, at \(scene.runtime.position)"
                )
                try expect(
                    scene.lastDiagnostic("agent")?.hasPrefix("focus:\(WorkAppScene.editor)") == true,
                    "the seat is not the work app's: \(String(describing: scene.lastDiagnostic("agent")))"
                )
            }
        },

        LogicTest(name: "a pet asleep at the seat still hops for the first keystroke") {
            try MainActor.assumeIsolated {
                let scene = try WorkAppScene()
                defer { scene.tearDown() }
                scene.platform.window.frontmost = WorkAppScene.editor
                try scene.walkAndSettle(within: 40)

                // Reading, long enough to doze off in the seat.
                scene.platform.userIdle.duration = 100
                let dozing = scene.run(seconds: 30) { runtime, _ in runtime.behaviorState == .sleep }
                try expect(
                    scene.runtime.behaviorState == .sleep,
                    "the pet never fell asleep at the seat; it is \(scene.runtime.behaviorState)"
                )
                try expect(!dozing.walked, "the pet left the seat to sleep")

                // The first keystroke. A sleeping pet ticks twice a second, the
                // same cadence the desk is sampled at, so the tick that sees the
                // key is the one that samples it.
                scene.platform.userIdle.duration = 0.2
                scene.platform.userIdle.keyboardDuration = 0.2
                let waking = scene.run(seconds: 6, firstStep: 0.5) { runtime, recording in
                    recording.worn.contains(.spark) && runtime.behaviorState == .work
                }
                let spark = try require(
                    waking.worn.firstIndex(of: .spark),
                    "the hop was lost to the nap; it wore \(waking.worn)"
                )
                try expect(
                    waking.worn[spark...].contains(.work),
                    "the work picture never followed the hop: \(waking.worn)"
                )
            }
        },

        LogicTest(name: "a pet asleep at the seat still waves when a long sitting ends") {
            try MainActor.assumeIsolated {
                let scene = try WorkAppScene()
                defer { scene.tearDown() }
                scene.platform.window.frontmost = WorkAppScene.editor
                try scene.walkAndSettle(within: 40)

                scene.platform.userIdle.duration = 100
                scene.run(seconds: 30) { runtime, _ in runtime.behaviorState == .sleep }
                try expect(
                    scene.runtime.behaviorState == .sleep,
                    "the pet never fell asleep at the seat; it is \(scene.runtime.behaviorState)"
                )

                // Three minutes of typing it sleeps through: only the keyboard
                // clock moves, so nothing wakes it, and it is still at this
                // seat when the user leaves.
                scene.platform.userIdle.keyboardDuration = 0.2
                let sitting = scene.run(seconds: 190, step: 0.5)
                try expect(
                    scene.runtime.behaviorState == .sleep,
                    "the pet woke during the sitting; it is \(scene.runtime.behaviorState), wore \(sitting.worn)"
                )

                // Off to an app nobody called work. The wave wakes the pet, and
                // it is handed the wave only once it stands idle -- the end of
                // the sitting used to arrive first and take the wave back.
                scene.platform.window.frontmost = "com.example.browser"
                let leaving = scene.run(seconds: 12) { _, recording in
                    recording.worn.contains(.celebrate)
                }
                try expect(
                    leaving.worn.contains(.celebrate),
                    "the wave was lost to the nap; it wore \(leaving.worn)"
                )
                scene.run(seconds: 2)
                try expect(
                    scene.lastDiagnostic("agent") == "none",
                    "the seat was kept after the wave: \(String(describing: scene.lastDiagnostic("agent")))"
                )
            }
        },

        LogicTest(name: "a long sitting waves on the way out, even after the seat went back") {
            try MainActor.assumeIsolated {
                let scene = try WorkAppScene(roaming: true)
                defer { scene.tearDown() }
                scene.platform.window.frontmost = WorkAppScene.editor
                try scene.walkAndSettle(within: 40)

                // Three minutes of typing: the hop at the display rate, then
                // the pet sits at work and the rest goes by at the sample rate.
                scene.platform.userIdle.keyboardDuration = 0.2
                scene.run(seconds: 5) { runtime, _ in runtime.behaviorState == .work }
                try expect(
                    scene.runtime.behaviorState == .work,
                    "the pet never went to work; it is \(scene.runtime.behaviorState)"
                )
                scene.run(seconds: 185, step: 0.5)

                // Hands off: the question, then the seat goes back and the pet
                // goes about its own day.
                scene.platform.userIdle.keyboardDuration = 11
                scene.run(seconds: 12) { runtime, _ in runtime.behaviorState == .waitingForUser }
                try expect(
                    scene.runtime.behaviorState == .waitingForUser,
                    "the keys stopped and nothing asked; the pet is \(scene.runtime.behaviorState)"
                )
                scene.run(seconds: 7) { runtime, _ in runtime.behaviorState != .waitingForUser }
                scene.run(seconds: 10) { runtime, _ in runtime.behaviorState == .wander }
                try expect(
                    scene.runtime.behaviorState == .wander,
                    "the released pet never went roaming; it is \(scene.runtime.behaviorState)"
                )
                try expect(
                    scene.lastDiagnostic("agent") == "none",
                    "the seat was not given back: \(String(describing: scene.lastDiagnostic("agent")))"
                )

                // Off to an app nobody called work. The stretch was typed, so
                // the pet waves -- where it is, not after a walk back.
                scene.platform.window.frontmost = "com.example.browser"
                let leaving = scene.run(seconds: 12) { _, recording in
                    recording.worn.contains(.celebrate)
                }
                try expect(
                    leaving.worn.contains(.celebrate),
                    "no wave on the way out of a seat given back; it wore \(leaving.worn)"
                )
                try expect(!leaving.walked, "the pet walked back to the editor to wave")
            }
        },

        LogicTest(name: "beside a working agent a long sitting ends without a wave") {
            try MainActor.assumeIsolated {
                let scene = try WorkAppScene(withSeatedAgent: true)
                defer { scene.tearDown() }

                // Three minutes typed into the editor while the agent works:
                // a stretch that earns a wave, none of it the pet's.
                scene.platform.window.frontmost = WorkAppScene.editor
                scene.platform.userIdle.keyboardDuration = 0.2
                scene.run(seconds: 190, step: 0.5)
                try expect(
                    scene.distanceToAgentWindow < scene.distanceToEditor,
                    "the pet left a working agent, for \(scene.runtime.position)"
                )

                // A tool call, and the user leaves the editor. Beside an agent
                // on duty no wave is said at all: the sitting just ends.
                scene.emitAgent(.highIntensity, intensity: 0.8)
                drainActivityEvents()
                scene.platform.window.frontmost = "com.example.browser"
                let leaving = scene.run(seconds: 12)
                try expect(
                    !leaving.worn.contains(.celebrate),
                    "the pet waved for the editor beside a working agent: \(leaving.worn)"
                )
                try expect(!leaving.walked, "the wave took the pet from the agent: \(leaving.worn)")
                try expect(
                    scene.lastDiagnostic("agent")?.hasPrefix(WorkAppScene.agentSource) == true,
                    "the agent lost its seat: \(String(describing: scene.lastDiagnostic("agent")))"
                )
            }
        },

        LogicTest(name: "an agent finishing just after a long sitting ends plays nothing of it late") {
            try MainActor.assumeIsolated {
                let scene = try WorkAppScene(withSeatedAgent: true)
                defer { scene.tearDown() }

                // Three minutes typed into the editor beside a working agent.
                scene.platform.window.frontmost = WorkAppScene.editor
                scene.platform.userIdle.keyboardDuration = 0.2
                scene.run(seconds: 190, step: 0.5)

                // A tool call, and the user leaves. The source samples twice a
                // second, so the grace is over by three and a half seconds.
                scene.emitAgent(.highIntensity, intensity: 0.8)
                drainActivityEvents()
                scene.platform.window.frontmost = "com.example.browser"
                scene.run(seconds: 4.5)

                // The agent finishes about a second later (plan §9.7a). A wave
                // said beside it was kept from the pet but waited in the
                // director for the end behind it, up to five seconds -- and the
                // agent letting go handed it over, for the pet to wear straight
                // after its own celebration.
                scene.emitAgent(.achievement, intensity: 0.55)
                drainActivityEvents()
                var states: [BehaviorState] = []
                let after = scene.run(seconds: 12) { runtime, _ in
                    if states.last != runtime.behaviorState { states.append(runtime.behaviorState) }
                    return false
                }
                try expect(
                    states.filter { $0 == .celebrate }.count == 1,
                    "the pet did not celebrate the agent exactly once: \(states)"
                )
                try expect(
                    !states.contains(.observe),
                    "the pet reacted to the editor's sitting after the agent finished: \(states)"
                )
                try expect(!after.walked, "the pet walked off after the agent finished: \(after.worn)")
                try expect(
                    scene.lastDiagnostic("agent") == "none",
                    "the seat was taken after the agent finished: "
                        + "\(String(describing: scene.lastDiagnostic("agent")))"
                )
            }
        },

        LogicTest(name: "the work app list is a menu of what has been in front") {
            try MainActor.assumeIsolated {
                let harness = try RuntimeHarness()
                defer { harness.tearDown() }

                @MainActor func rows() throws -> [MenuItem] {
                    let row = try require(
                        ShellMenu.items(for: harness.runtime)
                            .first { $0.title == localized("menu.workApps") },
                        "no work-app submenu in the menu"
                    )
                    guard case let .submenu(children) = row.content else {
                        throw LogicTestFailure(
                            message: "the work-app row is not a submenu: \(row.content)",
                            file: #filePath, line: #line
                        )
                    }
                    return children
                }

                // Nothing seen yet: a sentence, not an empty menu that reads
                // as a broken one.
                let empty = try rows()
                try expect(empty.count == 1)
                try expect(empty[0].title == localized("menu.workApps.none"))
                guard case .caption = empty[0].content else {
                    throw LogicTestFailure(
                        message: "the empty list is clickable", file: #filePath, line: #line
                    )
                }

                harness.platform.window.frontmost = "com.example.editor"
                harness.platform.window.displayNames = ["com.example.editor": "Editor"]
                harness.runtime.tick()

                let listed = try require(try rows().first)
                try expect(listed.title == "Editor", "the app is not named the way the OS names it")
                guard case let .check(action, isOn) = listed.content else {
                    throw LogicTestFailure(
                        message: "the app row is not a checkbox", file: #filePath, line: #line
                    )
                }
                try expect(!isOn, "an app was work before the user said so")
                try expect(action == .toggleWorkApp(id: "com.example.editor"))

                _ = ShellController.perform(action, runtime: harness.runtime, version: "1.2.3")
                try expect(harness.runtime.workApps == ["com.example.editor"])
                guard case let .check(_, checkedNow) = try require(try rows().first).content else {
                    throw LogicTestFailure(
                        message: "the app row stopped being a checkbox",
                        file: #filePath, line: #line
                    )
                }
                try expect(checkedNow, "the checkmark did not follow the choice")

                _ = ShellController.perform(action, runtime: harness.runtime, version: "1.2.3")
                try expect(harness.runtime.workApps.isEmpty, "toggling twice did not undo it")
            }
        },

        LogicTest(name: "the chosen work apps come back after a restart") {
            try MainActor.assumeIsolated {
                let suite = try makeTestDefaults()
                defer { suite.discard() }
                let display = DisplaySnapshot(
                    id: "1",
                    name: "test",
                    frame: WorldRect(x: 0, y: 0, width: 1440, height: 900),
                    visibleFrame: WorldRect(x: 0, y: 25, width: 1440, height: 850),
                    scale: 2
                )

                @MainActor func launch() -> RoamlingRuntime {
                    RoamlingRuntime(
                        services: FakePlatform(display: display, worldTop: 900).services,
                        defaults: suite.defaults,
                        catalog: PetCatalog(roots: []),
                        clock: { 0 }
                    )
                }

                let first = launch()
                try expect(first.workApps.isEmpty, "a fresh install watches something")
                first.toggleWorkApp("com.example.editor")
                first.toggleWorkApp("com.example.slides")

                try expect(launch().workApps == ["com.example.editor", "com.example.slides"])

                // Emptying the list takes the key with it, the way "Reset
                // Defaults" does for tuning: a stored empty would be a choice
                // this app has to keep honouring forever.
                first.toggleWorkApp("com.example.editor")
                first.toggleWorkApp("com.example.slides")
                try expect(suite.defaults.string(forKey: "roamling.workApps") == nil)
                try expect(launch().workApps.isEmpty)
            }
        }
    ]
}

/// What a stretch of ticks showed: every change of picture, in order, and
/// whether the pet walked to a seat at any point.
private struct Recording {
    var worn: [PetCapability] = []
    var walked = false
}

/// A desktop with an editor the user calls work and, when asked for, an agent
/// already sitting at a window of its own. The two windows are in different
/// places, so a walk from one to the other is a walk.
@MainActor
private struct WorkAppScene {
    static let editor = "com.example.editor"
    static let agentSource = "fake-agent:session"
    /// The left of the screen. The pet starts at the right edge.
    static let editorWindow = WorldRect(x: 120, y: 200, width: 700, height: 500)
    /// Near where the pet starts, and nowhere near the editor.
    static let agentWindow = WorldRect(x: 1_100, y: 500, width: 300, height: 300)

    let runtime: RoamlingRuntime
    let platform: FakePlatform
    let clock: TestClock
    let agent: FakeAgent?
    private let defaults: TestDefaults

    init(withSeatedAgent: Bool = false, roaming: Bool = false) throws {
        clock = TestClock(startingAt: 1_000)
        platform = FakePlatform(
            display: DisplaySnapshot(
                id: "1",
                name: "test",
                frame: WorldRect(x: 0, y: 0, width: 1440, height: 900),
                visibleFrame: WorldRect(x: 0, y: 25, width: 1440, height: 875),
                scale: 2
            ),
            worldTop: 900
        )
        // Out of the way: a near cursor outranks a seat, and this test is
        // about the seat.
        platform.pointer.position = WorldPoint(x: 20, y: 60)
        platform.userIdle.duration = 0
        platform.capture.isAuthorized = false

        defaults = try makeTestDefaults()
        defaults.defaults.set(Self.editor, forKey: "roamling.workApps")
        if roaming { defaults.defaults.set(true, forKey: "roamling.roaming") }
        agent = withSeatedAgent ? FakeAgent() : nil
        runtime = RoamlingRuntime(
            services: platform.services,
            agents: agent.map { [$0] } ?? [],
            defaults: defaults.defaults,
            catalog: PetCatalog(roots: []),
            clock: clock.read
        )
        runtime.start(drivingTicks: false)

        if agent != nil {
            emitAgent(.highIntensity, intensity: 0.8)
            drainActivityEvents()
            run(seconds: 20) { runtime, _ in
                runtime.behaviorState == .work && !runtime.isPlacementTravelling
            }
            guard runtime.behaviorState == .work else {
                throw LogicTestFailure(
                    message: "the agent never got its seat; the pet is \(runtime.behaviorState)",
                    file: #filePath, line: #line
                )
            }
        }

        // The window the shell hands over for any event that wants one and
        // did not bring its own: the editor, set only once the agent sits.
        platform.window.hint = LocationHint(
            applicationIdentifier: Self.editor,
            approximateRegion: Self.editorWindow,
            confidence: 0.8
        )
    }

    /// The agent says something, naming its own window -- so the shell's
    /// answer for the front window, which is the editor's, is not consulted.
    func emitAgent(_ kind: CompanionEventKind, intensity: Double) {
        agent?.emit(CompanionEvent(
            sourceID: Self.agentSource,
            sourceType: .agent,
            timestamp: clock.read(),
            kind: kind,
            intensity: intensity,
            context: .working,
            locationHint: LocationHint(
                applicationIdentifier: "com.example.terminal",
                approximateRegion: Self.agentWindow,
                confidence: 0.8
            )
        ))
    }

    /// Ticks at the display rate for up to `seconds`, stopping early once
    /// `until` holds, and records what the pet wore and whether it walked.
    /// `step` is for a sleeping pet, which ticks twice a second.
    @discardableResult
    func run(
        seconds: Double,
        firstStep: Double = 1.0 / 30,
        step: Double = 1.0 / 30,
        until: ((RoamlingRuntime, Recording) -> Bool)? = nil
    ) -> Recording {
        var recording = Recording()
        var elapsed = 0.0
        var next = firstStep
        while elapsed < seconds {
            clock.advance(next)
            elapsed += next
            next = step
            runtime.tick()
            if let picture = runtime.currentCapability, recording.worn.last != picture {
                recording.worn.append(picture)
            }
            if runtime.behaviorState == .travelToInterest { recording.walked = true }
            if let until, until(runtime, recording) { break }
        }
        return recording
    }

    /// Runs until the pet has walked to a seat and stopped there.
    @discardableResult
    func walkAndSettle(within seconds: Double) throws -> Recording {
        let recording = run(seconds: seconds) { runtime, recording in
            recording.walked && !runtime.isPlacementTravelling
                && runtime.behaviorState != .travelToInterest
        }
        guard recording.walked, !runtime.isPlacementTravelling else {
            throw LogicTestFailure(
                message: "the pet never walked to the window it was told about; "
                    + "it is \(runtime.behaviorState), wore \(recording.worn)",
                file: #filePath, line: #line
            )
        }
        return recording
    }

    var distanceToEditor: Double { Self.distance(from: runtime.position, to: Self.editorWindow) }
    var distanceToAgentWindow: Double { Self.distance(from: runtime.position, to: Self.agentWindow) }

    private static func distance(from point: WorldPoint, to rect: WorldRect) -> Double {
        let dx = max(rect.minX - point.x, 0, point.x - rect.maxX)
        let dy = max(rect.minY - point.y, 0, point.y - rect.maxY)
        return (dx * dx + dy * dy).squareRoot()
    }

    /// Every message logged under `category`, oldest first. Read by words, so
    /// the column padding of the copyable text does not matter.
    func diagnostics(_ category: String) -> [String] {
        runtime.diagnosticsText
            .split(separator: "\n")
            .compactMap { line -> String? in
                let words = line.split(separator: " ")
                guard words.count >= 3, words[1] == category else { return nil }
                return words[2...].joined(separator: " ")
            }
    }

    func lastDiagnostic(_ category: String) -> String? {
        diagnostics(category).last
    }

    func tearDown() {
        runtime.stop()
        defaults.discard()
    }
}
