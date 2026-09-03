// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

import Foundation
import RoamlingCore
import RoamlingEngine
import RoamlingPet

/// A recorded session, replayed tick by tick.
///
/// Every other gate in this suite states a rule. This one states nothing: it is
/// forty seconds of the real runtime standing on fakes -- roaming, a cursor
/// closing in, a catch, a drag, an agent turn, and a nap -- recorded once and
/// required back byte for byte.
///
/// It exists for the port. `docs/windows.md` unit 6c moves the tick body to
/// Rust, and the only honest way to show that the Rust runtime behaves like
/// this one is to record what this one does and demand the same answer. A
/// transcription of the Swift original would only prove the transcription.
///
/// Regenerate deliberately, never to make it pass:
///
///     ROAMLING_WRITE_TRACE=Tests/RoamlingLogicTests/RuntimeTrace.txt \
///         swift run RoamlingLogicTests
///
/// Screen capture stays off for the whole session. The capture arrives on a
/// `Task` whose completion this loop cannot pin down, and a trace that depends
/// on when a task happened to land is not a trace. What capture changes is
/// already gated by the placement fixture.
func runtimeTraceTests() -> [LogicTest] {
    [
        LogicTest(name: "a recorded session replays tick for tick") {
            try MainActor.assumeIsolated {
                let recorded = try recordTraceSession()
                let path = ProcessInfo.processInfo.environment["ROAMLING_WRITE_TRACE"]
                if let path {
                    try recorded.write(
                        to: URL(fileURLWithPath: path), atomically: true, encoding: .utf8
                    )
                    return
                }
                let expected = try String(
                    contentsOf: traceFixtureURL(), encoding: .utf8
                )
                guard recorded != expected else { return }
                let mine = recorded.components(separatedBy: "\n")
                let theirs = expected.components(separatedBy: "\n")
                for (index, line) in mine.enumerated() where index < theirs.count {
                    if line != theirs[index] {
                        throw LogicTestFailure(
                            message: "the session diverged at line \(index + 1)\n"
                                + "  recorded \(line)\n  expected \(theirs[index])",
                            file: #filePath,
                            line: #line
                        )
                    }
                }
                throw LogicTestFailure(
                    message: "the session has \(mine.count) lines,"
                        + " the fixture has \(theirs.count)",
                    file: #filePath,
                    line: #line
                )
            }
        }
    ]
}

private func traceFixtureURL() -> URL {
    URL(fileURLWithPath: #filePath)
        .deletingLastPathComponent()
        .appendingPathComponent("RuntimeTrace.txt")
}

@MainActor
private func recordTraceSession() throws -> String {
    let display = DisplaySnapshot(
        id: "1",
        name: "test",
        frame: WorldRect(x: 0, y: 0, width: 1440, height: 900),
        visibleFrame: WorldRect(x: 0, y: 25, width: 1440, height: 850),
        scale: 2
    )
    let platform = FakePlatform(display: display, worldTop: 900)
    platform.capture.isAuthorized = false
    platform.focus.isAuthorized = false
    platform.pointer.position = WorldPoint(x: 40, y: 60)
    platform.userIdle.duration = 0

    let suite = try makeTestDefaults()
    defer { suite.discard() }
    suite.defaults.set(true, forKey: "roamling.position.exists")
    suite.defaults.set(700.0, forKey: "roamling.position.x")
    suite.defaults.set(500.0, forKey: "roamling.position.y")

    let clock = TestClock(startingAt: 1_000)
    let agent = FakeAgent()
    let runtime = RoamlingRuntime(
        services: platform.services,
        agents: [agent],
        defaults: suite.defaults,
        catalog: PetCatalog(roots: []),
        clock: clock.read,
        // Deliberately a fixed seed. A session that cannot be replayed cannot
        // be a fixture, and the runtime takes its aimlessness as an argument
        // for exactly this reason.
        randomSeed: 0x51D3_7A0C_B84E_2F69
    )
    runtime.start(drivingTicks: false)
    defer { runtime.stop() }

    let states = BehaviorState.allCases
    var lines: [String] = []
    func record(_ label: String) {
        lines.append(
            String(
                format: "%@ %.4f %.4f %d %d %d",
                label,
                platform.overlay.position.x,
                platform.overlay.position.y,
                states.firstIndex(of: runtime.behaviorState) ?? -1,
                runtime.isPlacementTravelling ? 1 : 0,
                platform.overlay.isInteractionEnabled ? 1 : 0
            // The draw counter rides along on purpose. Two runs that diverge
            // almost always diverge because one of them consumed a random
            // number the other did not, and without this the first visible
            // symptom is a coordinate three decimals out, forty ticks later.
            ) + String(format: " d%d", runtime.randomDraws)
        )
    }

    let step = 1.0 / 30.0
    func tick(_ label: String) {
        clock.advance(step)
        runtime.tick()
        record(label)
    }
    func run(_ seconds: Double, _ label: String, each: (Int) -> Void = { _ in }) {
        for index in 0..<Int(seconds / step) {
            each(index)
            tick(label)
        }
    }

    // 1. Alone on the desktop: the roaming pause runs out and it strolls.
    run(10, "roam")

    // 2. A cursor closes in from the left. Watching, then evading.
    run(6, "cursor") { index in
        platform.pointer.position = WorldPoint(
            x: 40 + Double(index) * 4,
            y: platform.overlay.position.y
        )
    }

    // 3. A hand shoots across the desk for it. Only a fast, closing approach
    // arms the catch -- an ambling cursor never does, which is the point.
    // Back off first. The cursor ended the last phase alongside the pet, and a
    // hand already touching it has nowhere to accelerate from.
    platform.pointer.position = platform.overlay.position - WorldVector(dx: 520, dy: 0)
    // Closes until it is on the pet, then grabs on that tick rather than after
    // a fixed number of them: the catch window is 0.35 s wide, so arming it and
    // then ticking on is the same as never arming it at all.
    for _ in 0..<90 {
        let pet = platform.overlay.position
        let gap = pet - platform.pointer.position
        let stride = min(gap.length, 26)
        platform.pointer.position = gap.length < 0.001
            ? pet
            : platform.pointer.position + gap.normalized * stride
        tick("lunge")
        if platform.overlay.isInteractionEnabled { break }
    }
    platform.pointer.primaryButtonDown = true
    runtime.petOverlayPointerDown(at: platform.pointer.position)
    record("grab")
    run(0.5, "held")
    for index in 0..<40 {
        clock.advance(step)
        runtime.petOverlayPointerDragged(
            to: WorldPoint(x: 300 + Double(index) * 12, y: 400),
            distance: Double(index) * 12
        )
        record("drag")
    }
    runtime.petOverlayPointerUp(at: WorldPoint(x: 780, y: 400), wasDragged: true)
    platform.pointer.primaryButtonDown = false
    record("drop")
    run(3, "land")

    // 4. An agent starts a turn beside a window on the far side of the desk,
    // so the pet has somewhere to walk to rather than claiming where it stands.
    platform.pointer.position = WorldPoint(x: 30, y: 60)
    platform.window.hint = LocationHint(
        approximateRegion: WorldRect(x: 60, y: 520, width: 380, height: 260),
        confidence: 0.8
    )
    func emit(_ kind: CompanionEventKind, _ intensity: Double) {
        agent.emit(CompanionEvent(
            sourceID: "fake-agent",
            sourceType: .agent,
            timestamp: clock.read(),
            kind: kind,
            intensity: intensity,
            context: .working
        ))
        drainActivityEvents()
    }
    emit(.activityStarted, 0.6)
    run(10, "travel")

    // 5. The turn's shape, one event at a time: a tool running, a file being
    // read, a prompt the user has to answer, a failure, and the finish.
    emit(.highIntensity, 0.8)
    run(4, "busy")
    emit(.inspecting, 0.5)
    run(3, "review")
    emit(.attentionRequired, 0.9)
    run(4, "ask")
    emit(.setback, 0.7)
    run(3, "setback")
    emit(.achievement, 0.9)
    run(5, "done")

    // 6. Nobody has touched the machine for four minutes.
    platform.userIdle.duration = 240
    run(14, "rest")

    // 7. A second display appears, then the pet keeps roaming on it.
    platform.displayProvider.displaySet = DisplaySnapshotSet(
        displays: [
            display,
            DisplaySnapshot(
                id: "2",
                name: "second",
                frame: WorldRect(x: 1440, y: 0, width: 1280, height: 800),
                visibleFrame: WorldRect(x: 1440, y: 25, width: 1280, height: 750),
                scale: 2
            )
        ],
        coordinateSpace: DesktopCoordinateSpace(worldTop: 900)
    )
    platform.displayProvider.notifyChange()
    platform.userIdle.duration = 0
    record("displays")
    run(12, "wander")

    lines.append("--- diagnostics ---")
    lines.append(contentsOf: runtime.diagnosticsText.components(separatedBy: "\n"))
    return lines.joined(separator: "\n") + "\n"
}
