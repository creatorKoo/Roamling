// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

import Foundation
import RoamlingCore
import RoamlingEngine
import RoamlingPet

func runtimeLogicTests() -> [LogicTest] {
    [
        LogicTest(name: "an idle minute walks the pet to a safe zone and puts it to sleep") {
            try MainActor.assumeIsolated {
                let clock = TestClock(startingAt: 1_000)
                let platform = FakePlatform(
                    display: DisplaySnapshot(
                        id: "test-display",
                        name: "Test",
                        frame: WorldRect(x: 0, y: 0, width: 1440, height: 900),
                        visibleFrame: WorldRect(x: 0, y: 25, width: 1440, height: 875),
                        scale: 2
                    ),
                    worldTop: 900
                )
                // Far from the pet, which starts in the opposite corner: the
                // cursor outranks rest, so a near pointer would never let the
                // pet settle.
                platform.pointer.position = WorldPoint(x: 20, y: 60)
                platform.userIdle.duration = 80
                platform.capture.isAuthorized = false

                let suite = try makeTestDefaults()
                defer { suite.discard() }
                let defaults = suite.defaults

                let runtime = RoamlingRuntime(
                    services: platform.services,
                    defaults: defaults,
                    catalog: PetCatalog(roots: []),
                    clock: clock.read
                )

                var states: [BehaviorState] = []
                for _ in 0..<3_000 {
                    clock.advance(1.0 / 30)
                    runtime.tick()
                    if states.last != runtime.behaviorState { states.append(runtime.behaviorState) }
                    if runtime.behaviorState == .sleep { break }
                }

                // The whole rest path, driven through the real tick loop: an
                // idle desktop seats the pet, sitting turns into looking for a
                // spot, and the walk ends in sleep.
                try expect(
                    states == [.sit, .findSleepSpot, .sleep],
                    "expected sit -> findSleepSpot -> sleep, got \(states)"
                )

                // Without screen capture the pet cannot vouch for where it is
                // standing, so it tucks into a permission-free safe zone
                // rather than dozing on top of whatever is there.
                try expect(
                    runtime.diagnosticsText.contains("tucking into a safe zone, spot unvetted"),
                    "expected the unvetted-spot reason in diagnostics"
                )

                let world = DesktopWorldSnapshot(displays: [platform.displaySet.displays[0]])
                let zones = BasicSafeZonePlanner.safeZones(in: world)
                try expect(
                    zones.contains { $0.frame.contains(runtime.position) },
                    "expected the pet to end inside a safe zone, ended at \(runtime.position)"
                )
                try expect(
                    platform.overlay.position == runtime.position,
                    "expected the overlay to have been moved to where the pet ended"
                )
            }
        },

        LogicTest(name: "the affection key turns an approach into a sit") {
            try MainActor.assumeIsolated {
                let clock = TestClock(startingAt: 1_000)
                let platform = FakePlatform(
                    display: DisplaySnapshot(
                        id: "test-display",
                        name: "Test",
                        frame: WorldRect(x: 0, y: 0, width: 1440, height: 900),
                        visibleFrame: WorldRect(x: 0, y: 25, width: 1440, height: 875),
                        scale: 2
                    ),
                    worldTop: 900
                )
                let suite = try makeTestDefaults()
                defer { suite.discard() }
                let runtime = RoamlingRuntime(
                    services: platform.services,
                    defaults: suite.defaults,
                    catalog: PetCatalog(roots: []),
                    clock: clock.read
                )
                platform.pointer.position = runtime.position + WorldVector(dx: 600, dy: 0)
                for _ in 0..<30 {
                    clock.advance(1.0 / 30)
                    runtime.tick()
                }

                // Inside the evade radius, with the key held: no stepping
                // away, but no petting either -- the hand is beside the pet,
                // so it watches as it would any cursor.
                platform.pointer.affectionHeld = true
                platform.pointer.position = runtime.position + WorldVector(dx: 80, dy: 0)
                let start = runtime.position
                var watched = false
                for _ in 0..<60 {
                    clock.advance(1.0 / 30)
                    runtime.tick()
                    try expect(
                        runtime.behaviorState != .evadePointer,
                        "the pet stepped away from a hand"
                    )
                    try expect(runtime.currentCapability != .paw, "petted from 80 points away")
                    if runtime.behaviorState == .lookAtPointer, runtime.currentCapability == .gaze {
                        watched = true
                    }
                }
                try expect(watched, "the pet never watched the hand, state \(runtime.behaviorState)")

                // The hand on the pet: it sits up for it.
                platform.pointer.position = runtime.position
                var sat = false
                for _ in 0..<60 {
                    clock.advance(1.0 / 30)
                    runtime.tick()
                    try expect(runtime.behaviorState != .evadePointer, "stepped away from a hand")
                    if runtime.behaviorState == .lookAtPointer, runtime.currentCapability == .paw {
                        sat = true
                    }
                }
                try expect(sat, "the pet never sat for the hand, state \(runtime.behaviorState)")
                try expect(
                    runtime.position.distance(to: start) < 1,
                    "the pet moved \(runtime.position.distance(to: start)) points while being petted"
                )

                // Key released with the cursor still close: back to its old self.
                platform.pointer.affectionHeld = false
                platform.pointer.position = runtime.position + WorldVector(dx: 80, dy: 0)
                var stepped = false
                for _ in 0..<30 {
                    clock.advance(1.0 / 30)
                    runtime.tick()
                    if runtime.behaviorState == .evadePointer { stepped = true }
                }
                try expect(stepped, "the pet kept sitting after the hand was gone")
            }
        },
        LogicTest(name: "a pointer beside the pet keeps it awake however long the desk is idle") {
            try MainActor.assumeIsolated {
                let clock = TestClock(startingAt: 1_000)
                let platform = FakePlatform(
                    display: DisplaySnapshot(
                        id: "test-display",
                        name: "Test",
                        frame: WorldRect(x: 0, y: 0, width: 1440, height: 900),
                        visibleFrame: WorldRect(x: 0, y: 25, width: 1440, height: 875),
                        scale: 2
                    ),
                    worldTop: 900
                )
                platform.userIdle.duration = 600

                let suite = try makeTestDefaults()
                defer { suite.discard() }
                let defaults = suite.defaults

                let runtime = RoamlingRuntime(
                    services: platform.services,
                    defaults: defaults,
                    catalog: PetCatalog(roots: []),
                    clock: clock.read
                )
                // On the pet, not merely near it. Idle time is about the desk,
                // not the cursor, so a parked cursor on top of the pet has to
                // keep it up on its own.
                platform.pointer.position = runtime.position

                for _ in 0..<600 {
                    clock.advance(1.0 / 30)
                    runtime.tick()
                    try expect(
                        !runtime.behaviorState.isResting,
                        "the pet slept with the cursor on it, state \(runtime.behaviorState)"
                    )
                }
            }
        }
    ]
}

// MARK: - Support

/// A defaults suite of its own, so a test neither reads the settings of the
/// app running on this machine nor leaves anything behind on disk.
struct TestDefaults {
    let defaults: UserDefaults
    let name: String

    func discard() {
        defaults.removePersistentDomain(forName: name)
        // Removing the domain empties the file; it does not delete it. Left
        // alone, every run adds one more 42-byte plist to ~/Library/Preferences.
        let plist = FileManager.default.homeDirectoryForCurrentUser
            .appendingPathComponent("Library/Preferences/\(name).plist")
        try? FileManager.default.removeItem(at: plist)
    }
}

func makeTestDefaults() throws -> TestDefaults {
    let name = "dev.roamling.tests.\(UUID().uuidString)"
    let defaults = try require(
        UserDefaults(suiteName: name),
        "could not open a throwaway defaults suite"
    )
    return TestDefaults(defaults: defaults, name: name)
}

// MARK: - Fakes

/// A desktop with no window server behind it. Every provider answers from a
/// value the test sets, so a run is the same on any machine and costs no
/// wall-clock time.
@MainActor
final class FakePlatform {
    let displayProvider: FakeDisplayProvider
    let safeZone = BasicSafeZoneProvider()
    let userIdle = FakeUserIdleProvider()
    let capture = FakeCaptureProvider()
    let pointer = FakePointerProvider()
    let window = FakeWindowProvider()
    let focus = FakeFocusProvider()
    let overlay = FakeOverlay()
    let coordinateSpace = CoordinateSpaceSource()

    var displaySet: DisplaySnapshotSet { displayProvider.displaySet }

    init(display: DisplaySnapshot, worldTop: Double) {
        displayProvider = FakeDisplayProvider(
            displaySet: DisplaySnapshotSet(
                displays: [display],
                coordinateSpace: DesktopCoordinateSpace(worldTop: worldTop)
            )
        )
    }

    var services: PlatformServices {
        PlatformServices(
            display: displayProvider,
            displayChanges: displayProvider,
            safeZone: safeZone,
            userIdle: userIdle,
            capture: capture,
            pointer: pointer,
            window: window,
            focus: focus,
            overlay: overlay,
            images: testImages,
            coordinateSpace: coordinateSpace
        )
    }
}

@MainActor
final class FakeDisplayProvider: DisplayProviding, DisplayChangeObserving {
    var displaySet: DisplaySnapshotSet
    private(set) var isObserving = false

    init(displaySet: DisplaySnapshotSet) {
        self.displaySet = displaySet
    }

    func currentDisplaySet() -> DisplaySnapshotSet { displaySet }

    private var handlers: [@MainActor () -> Void] = []

    func observeDisplayChanges(
        _ handler: @escaping @MainActor () -> Void
    ) -> DisplayChangeSubscription {
        isObserving = true
        handlers.append(handler)
        return DisplayChangeSubscription { self.isObserving = false }
    }

    /// Stands in for the screen-parameters notification, so a test can plug in
    /// a second display and watch what the runtime does about it.
    func notifyChange() {
        for handler in handlers { handler() }
    }
}

@MainActor
final class FakePointerProvider: PointerProviding {
    var position: WorldPoint = .zero
    /// The drop path only runs when the button comes back up, so a trace that
    /// catches the pet has to be able to put it down.
    var primaryButtonDown = false
    var affectionHeld = false

    func currentPointer(at timestamp: TimeInterval) -> PointerSnapshot {
        PointerSnapshot(
            position: position,
            timestamp: timestamp,
            primaryButtonDown: primaryButtonDown,
            affectionHeld: affectionHeld
        )
    }
}

@MainActor
final class FakeUserIdleProvider: UserIdleProviding {
    var duration: TimeInterval = 0
    /// Since the last keystroke. Held apart from `duration` so a test can put
    /// the user's hands on the keyboard without also waking the idle rules.
    var keyboardDuration: TimeInterval = 3_600
    /// When the hands came off, in clock time. A frozen `keyboardDuration`
    /// answers "the last key was N seconds ago" at every sample, which walks
    /// that key forward with the clock: the pause never grows and the machine
    /// contradicts what it said a moment earlier. Set this instead whenever
    /// the test is about how long the keyboard has been still.
    var lastKeyAt: TimeInterval?

    func idleDuration(at timestamp: TimeInterval) -> TimeInterval { duration }

    func keyboardIdleDuration(at timestamp: TimeInterval) -> TimeInterval {
        guard let lastKeyAt else { return keyboardDuration }
        return max(timestamp - lastKeyAt, 0)
    }
}

@MainActor
final class FakeCaptureProvider: CaptureProviding {
    var isAuthorized = false
    var field: LuminanceField?

    @discardableResult
    func requestAuthorization() -> Bool { isAuthorized }

    func captureLuminanceField(for display: DisplaySnapshot) async -> LuminanceField? { field }
}

@MainActor
final class FakeWindowProvider: WindowProviding {
    var windows: [WindowSnapshot] = []
    var hint: LocationHint?
    /// Nil is "this app is in front", which is what the recorded session sees
    /// for its whole forty seconds.
    var frontmost: String?
    var displayNames: [String: String] = [:]

    func currentWindows() -> [WindowSnapshot] { windows }
    func currentActivityLocationHint() -> LocationHint? { hint }
    func frontmostApplicationIdentifier() -> String? { frontmost }
    func applicationDisplayName(for identifier: String) -> String? { displayNames[identifier] }
}

@MainActor
final class FakeFocusProvider: FocusProviding {
    var isAuthorized = false
    var focus: FocusSnapshot?

    @discardableResult
    func requestAuthorization() -> Bool { isAuthorized }

    func currentFocus() -> FocusSnapshot? { focus }
}

@MainActor
final class FakeOverlay: PetOverlayProviding {
    private(set) var scale: Double = 1
    private(set) var position: WorldPoint = .zero
    private(set) var isVisible = false
    private(set) var isInteractionEnabled = false
    private(set) var hitRegionScale: Double = 1
    private(set) var renderedFrames = 0
    weak var inputHandler: (any PetOverlayInputHandling)?

    var objectSize: WorldSize {
        WorldSize(width: 96 * scale, height: 104 * scale)
    }

    func setPosition(_ position: WorldPoint) { self.position = position }
    func setVisible(_ visible: Bool) { isVisible = visible }
    func setInteractionEnabled(_ enabled: Bool) { isInteractionEnabled = enabled }
    func setScale(_ scale: Double) { self.scale = scale.clamped(to: 0.6...1.8) }
    func setHitRegionScale(_ scale: Double) { hitRegionScale = scale }
    func setFrameImage(_ frame: PetFrame?) { renderedFrames += 1 }

    func containsPet(atWorldPoint worldPoint: WorldPoint) -> Bool {
        WorldRect(
            x: position.x - objectSize.width / 2,
            y: position.y - objectSize.height / 2,
            width: objectSize.width,
            height: objectSize.height
        ).contains(worldPoint)
    }
}

/// The runtime's clock, wound by hand. `@unchecked Sendable` because the
/// runtime takes a `@Sendable` closure; the lock is what makes that true.
final class TestClock: @unchecked Sendable {
    private let lock = NSLock()
    private var seconds: TimeInterval

    init(startingAt seconds: TimeInterval) {
        self.seconds = seconds
    }

    var read: @Sendable () -> TimeInterval {
        { [self] in
            lock.lock()
            defer { lock.unlock() }
            return seconds
        }
    }

    func advance(_ delta: TimeInterval) {
        lock.lock()
        seconds += delta
        lock.unlock()
    }
}

/// A runtime standing on fakes, with a throwaway defaults suite. Used by the
/// shell tests too: the menu is a function of runtime state, so asking what it
/// shows means having a runtime.
@MainActor
struct RuntimeHarness {
    let runtime: RoamlingRuntime
    /// Exposed so a test can say what is in front of the user, which is the
    /// only thing the menu's list of work apps is built from.
    let platform: FakePlatform
    private let defaults: TestDefaults

    init() throws {
        let display = DisplaySnapshot(
            id: "1",
            name: "test",
            frame: WorldRect(x: 0, y: 0, width: 1440, height: 900),
            visibleFrame: WorldRect(x: 0, y: 25, width: 1440, height: 850),
            scale: 2
        )
        platform = FakePlatform(display: display, worldTop: 900)
        defaults = try makeTestDefaults()
        runtime = RoamlingRuntime(
            services: platform.services,
            defaults: defaults.defaults,
            catalog: PetCatalog(roots: []),
            clock: { 0 }
        )
    }

    func tearDown() {
        defaults.discard()
    }
}

/// Reproduces the diagnostics the user captured on 2026-09-03:
///
///     1699.1  agent claude-code:... window=found
///     1699.2  pet spark
///     1702.1  place travel coveringWork to 960,1064
///     1702.2  pet travelToInterest
///     1704.0  place hold          <- the director settled
///             ...nothing for 22 seconds...
///     1726.5  pet lookAtPointer   <- a passing cursor ended it
///
/// The pet arrived and kept playing the walk.
func stuckTravelLogicTests() -> [LogicTest] {
    [
        LogicTest(name: "arriving at a work seat ends the walk even with nothing to react with") {
            try MainActor.assumeIsolated {
                let clock = TestClock(startingAt: 1_000)
                let display = DisplaySnapshot(
                    id: "1",
                    name: "test",
                    frame: WorldRect(x: 0, y: 0, width: 1440, height: 900),
                    visibleFrame: WorldRect(x: 0, y: 25, width: 1440, height: 875),
                    scale: 2
                )
                let platform = FakePlatform(display: display, worldTop: 900)
                // Far away: the cursor outranks placement, and in the captured
                // log it was a passing cursor that finally unstuck the pet.
                platform.pointer.position = WorldPoint(x: 20, y: 60)
                platform.userIdle.duration = 0
                // Authorised, but nothing captured yet -- which is the state
                // the log was in when the event landed. Without a field the
                // director has no reason to prefer a new seat, so it seats the
                // pet where it stands and the arrival reaction is spent there.
                platform.capture.isAuthorized = true
                platform.capture.field = nil

                let suite = try makeTestDefaults()
                defer { suite.discard() }
                // Start inside the window being watched. That is what makes the
                // director keep the pet where it stands instead of walking it
                // over, which is the order the captured log shows.
                suite.defaults.set(true, forKey: "roamling.position.exists")
                // On the line the planner prefers -- the bottom of the window,
                // less half the pet and 14 -- so a fresh seat is no better than
                // the one the pet is already on and the walk is refused.
                suite.defaults.set(700.0, forKey: "roamling.position.x")
                suite.defaults.set(734.0, forKey: "roamling.position.y")
                let agent = FakeAgent()
                let runtime = RoamlingRuntime(
                    services: platform.services,
                    agents: [agent],
                    defaults: suite.defaults,
                    catalog: PetCatalog(roots: []),
                    clock: clock.read
                )
                runtime.start()
                defer { runtime.stop() }

                // A turn opening, which is what the log shows. Its reaction is
                // `spark`, and spark is not an ongoing one -- that is the half
                // of the bug that decides whether it bites.
                let region = WorldRect(x: 200, y: 200, width: 1000, height: 600)
                agent.emit(CompanionEvent(
                    sourceID: "fake-agent:session",
                    sourceType: .agent,
                    timestamp: clock.read(),
                    kind: .activityStarted,
                    locationHint: LocationHint(
                        applicationIdentifier: "test",
                        approximateRegion: region,
                        confidence: 0.8
                    )
                ))
                drainActivityEvents()

                // Seat taken in place, spark spent. Nothing is left to react
                // with by the time a walk happens.
                for tick in 0..<60 {
                    clock.advance(1.0 / 30)
                    runtime.tick()
                    // The capture is fetched on a Task, so the loop has to let
                    // it run or the field never reaches the director.
                    if tick % 10 == 0 { drainActivityEvents() }
                }
                try expect(
                    runtime.behaviorState != .travelToInterest,
                    "expected the pet to seat in place first, got \(runtime.behaviorState)"
                )

                // Now the capture lands and says the seat is on content, which
                // is what sends the pet walking in the captured log.
                platform.capture.field = fieldBusy(
                    inside: WorldRect(x: 520, y: 600, width: 380, height: 280),
                    bounds: WorldRect(x: 0, y: 0, width: 1440, height: 900)
                )

                var sawTravel = false
                var settledState: BehaviorState?
                for tick in 0..<2_400 {
                    clock.advance(1.0 / 30)
                    runtime.tick()
                    if tick % 10 == 0 { drainActivityEvents() }
                    if runtime.behaviorState == .travelToInterest { sawTravel = true }
                    if sawTravel, !runtime.isPlacementTravelling {
                        settledState = runtime.behaviorState
                        // Give it a moment to leave the walk after settling.
                        for _ in 0..<10 {
                            clock.advance(1.0 / 30)
                            runtime.tick()
                            settledState = runtime.behaviorState
                            if settledState != .travelToInterest { break }
                        }
                        break
                    }
                }

                if ProcessInfo.processInfo.environment["ROAMLING_TRACE"] == "1" {
                    print("--- runtime diagnostics ---")
                    print(runtime.diagnosticsText)
                    print("--- settled as \(String(describing: settledState)) ---")
                }
                try expect(sawTravel, "the pet never travelled, so the case was not reached")
                let state = try require(settledState, "placement never stopped travelling")
                try expect(
                    state != .travelToInterest,
                    "the pet arrived and kept walking on the spot -- state is \(state)"
                )
            }
        },
        LogicTest(name: "a cursor in the glance band does not stop the walk off covered work") {
            try MainActor.assumeIsolated {
                // Seen on 2026-09-09: dropped onto the terminal's text while
                // Claude Code worked, the pet set off for a clear seat, met the
                // cursor on the way and stopped to look at it -- standing on the
                // text for as long as the cursor stayed. Roaming's walk off the
                // text already ignored the glance; the seat watch's did not.
                let clock = TestClock(startingAt: 1_000)
                let display = DisplaySnapshot(
                    id: "1",
                    name: "test",
                    frame: WorldRect(x: 0, y: 0, width: 1440, height: 900),
                    visibleFrame: WorldRect(x: 0, y: 25, width: 1440, height: 875),
                    scale: 2
                )
                let platform = FakePlatform(display: display, worldTop: 900)
                platform.pointer.position = WorldPoint(x: 20, y: 60)
                platform.userIdle.duration = 0
                platform.capture.isAuthorized = true
                platform.capture.field = nil

                let suite = try makeTestDefaults()
                defer { suite.discard() }
                suite.defaults.set(true, forKey: "roamling.position.exists")
                suite.defaults.set(700.0, forKey: "roamling.position.x")
                suite.defaults.set(734.0, forKey: "roamling.position.y")
                let agent = FakeAgent()
                let runtime = RoamlingRuntime(
                    services: platform.services,
                    agents: [agent],
                    defaults: suite.defaults,
                    catalog: PetCatalog(roots: []),
                    clock: clock.read
                )
                runtime.start()
                defer { runtime.stop() }

                let region = WorldRect(x: 200, y: 200, width: 1000, height: 600)
                agent.emit(CompanionEvent(
                    sourceID: "fake-agent:session",
                    sourceType: .agent,
                    timestamp: clock.read(),
                    kind: .activityStarted,
                    locationHint: LocationHint(
                        applicationIdentifier: "test",
                        approximateRegion: region,
                        confidence: 0.8
                    )
                ))
                drainActivityEvents()
                for tick in 0..<60 {
                    clock.advance(1.0 / 30)
                    runtime.tick()
                    if tick % 10 == 0 { drainActivityEvents() }
                }

                // The capture says the seat is on content: the walk this test
                // is about.
                platform.capture.field = fieldBusy(
                    inside: WorldRect(x: 520, y: 600, width: 380, height: 280),
                    bounds: WorldRect(x: 0, y: 0, width: 1440, height: 900)
                )
                var departed = false
                for tick in 0..<2_400 where !departed {
                    clock.advance(1.0 / 30)
                    runtime.tick()
                    if tick % 10 == 0 { drainActivityEvents() }
                    departed = runtime.behaviorState == .travelToInterest
                }
                try expect(departed, "the pet never set off, so the case was not reached")
                try expect(
                    runtime.diagnosticsText.contains("travel coveringWork"),
                    "the walk was not the covering-work one this test is about"
                )

                // Two ticks give the direction of travel. The cursor is parked a
                // little ahead and 130 points to the side of the path: inside
                // the 170 glance band as the pet passes, outside the 100 evade
                // radius, so only the glance is in play.
                let before = runtime.position
                clock.advance(1.0 / 30)
                runtime.tick()
                clock.advance(1.0 / 30)
                runtime.tick()
                let start = runtime.position
                let heading = (start - before).normalized
                try expect(heading != .zero, "the pet is not moving after departing")
                let side = WorldVector(dx: -heading.dy, dy: heading.dx)
                platform.pointer.position = start + heading * 60 + side * 130

                var glanced = false
                var pushed = false
                for _ in 0..<90 {
                    clock.advance(1.0 / 30)
                    runtime.tick()
                    if runtime.behaviorState == .lookAtPointer { glanced = true }
                    if runtime.behaviorState == .evadePointer { pushed = true }
                }
                let travelled = runtime.position.distance(to: start)
                try expect(!pushed, "the cursor was placed inside the evade radius; the case is wrong")
                try expect(!glanced, "the pet stopped to look at the cursor on its way off the text")
                try expect(
                    travelled > 120,
                    "the pet moved only \(travelled) points in three seconds with the cursor beside its path"
                )
            }
        }
    ]
}

/// An agent whose events the test pushes by hand, so a scenario can be written
/// as the sequence of things an agent actually said.
@MainActor
/// An integration that cannot be installed, so the failure copy can be read.
final class FailingAgent: AgentIntegration {
    let id: String
    let displayName = "Failing"
    var installationStatus: AgentIntegrationStatus = .notInstalled
    var receiverState: ActivityReceiverState = .stopped

    struct Refused: Error, LocalizedError {
        var errorDescription: String? { "refused" }
    }

    init(id: String) { self.id = id }

    func makeEventStream() -> AsyncStream<CompanionEvent> {
        AsyncStream { $0.finish() }
    }
    func startReceiving() throws {}
    func stopReceiving() {}
    func install() -> Result<Void, Error> { .failure(Refused()) }
    func remove() -> Result<Void, Error> { .failure(Refused()) }
}

final class FakeAgent: AgentIntegration {
    let id = "fake-agent"
    let displayName = "Fake"
    var installationStatus: AgentIntegrationStatus = .installed
    var receiverState: ActivityReceiverState = .ready

    private let stream: AsyncStream<CompanionEvent>
    private let continuation: AsyncStream<CompanionEvent>.Continuation

    init() {
        let pair = AsyncStream<CompanionEvent>.makeStream()
        stream = pair.stream
        continuation = pair.continuation
    }

    func makeEventStream() -> AsyncStream<CompanionEvent> { stream }
    func startReceiving() throws {}
    func stopReceiving() {}
    func install() -> Result<Void, Error> { .success(()) }
    func remove() -> Result<Void, Error> { .success(()) }

    func emit(_ event: CompanionEvent) { continuation.yield(event) }
}

/// The runtime reads agent events off an `AsyncStream` in a `Task`, so a test
/// that only calls `tick()` never sees them. Turning the main run loop briefly
/// is what lets that task deliver.
@MainActor
func drainActivityEvents() {
    // Deliberately not `before:` a future date. The runtime's own tick timer is
    // on this run loop, so pumping it for real time lets an unpredictable
    // number of ticks happen -- which is invisible to a test that only asserts
    // a state, and fatal to one that records a session tick by tick. A deadline
    // already past still drains the main queue, which is where the delivery is.
    for _ in 0..<400 {
        RunLoop.main.run(mode: .default, before: Date())
    }
}

/// A field that is busy inside `busy` and flat everywhere else.
///
/// A checkerboard has the highest local gradient there is, so it scores zero;
/// the flat remainder scores one. The director refuses to trade one marginal
/// seat for another, so a uniformly busy screen produces no walk at all -- the
/// contrast is what makes it move.
@MainActor
func fieldBusy(
    inside busy: WorldRect,
    bounds: WorldRect,
    columns: Int = 48,
    rows: Int = 32
) -> LuminanceField? {
    let cellWidth = bounds.size.width / Double(columns)
    let cellHeight = bounds.size.height / Double(rows)
    var samples: [Double] = []
    for row in 0..<rows {
        for column in 0..<columns {
            let centre = WorldPoint(
                x: bounds.minX + (Double(column) + 0.5) * cellWidth,
                y: bounds.minY + (Double(row) + 0.5) * cellHeight
            )
            samples.append(busy.contains(centre) ? ((row + column) % 2 == 0 ? 0.0 : 1.0) : 0.5)
        }
    }
    return LuminanceField(bounds: bounds, columns: columns, rows: rows, samples: samples)
}
