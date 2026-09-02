// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

import CoreGraphics
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
private struct TestDefaults {
    let defaults: UserDefaults
    let name: String

    func discard() {
        defaults.removePersistentDomain(forName: name)
    }
}

private func makeTestDefaults() throws -> TestDefaults {
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
private final class FakePlatform {
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
private final class FakeDisplayProvider: DisplayProviding, DisplayChangeObserving {
    var displaySet: DisplaySnapshotSet
    private(set) var isObserving = false

    init(displaySet: DisplaySnapshotSet) {
        self.displaySet = displaySet
    }

    func currentDisplaySet() -> DisplaySnapshotSet { displaySet }

    func observeDisplayChanges(
        _ handler: @escaping @MainActor () -> Void
    ) -> DisplayChangeSubscription {
        isObserving = true
        return DisplayChangeSubscription { self.isObserving = false }
    }
}

@MainActor
private final class FakePointerProvider: PointerProviding {
    var position: WorldPoint = .zero

    func currentPointer(at timestamp: TimeInterval) -> PointerSnapshot {
        PointerSnapshot(position: position, timestamp: timestamp, primaryButtonDown: false)
    }
}

@MainActor
private final class FakeUserIdleProvider: UserIdleProviding {
    var duration: TimeInterval = 0

    func idleDuration(at timestamp: TimeInterval) -> TimeInterval { duration }
}

@MainActor
private final class FakeCaptureProvider: CaptureProviding {
    var isAuthorized = false
    var field: LuminanceField?

    @discardableResult
    func requestAuthorization() -> Bool { isAuthorized }

    func captureLuminanceField(for display: DisplaySnapshot) async -> LuminanceField? { field }
}

@MainActor
private final class FakeWindowProvider: WindowProviding {
    var windows: [WindowSnapshot] = []
    var hint: LocationHint?

    func currentWindows() -> [WindowSnapshot] { windows }
    func currentActivityLocationHint() -> LocationHint? { hint }
}

@MainActor
private final class FakeFocusProvider: FocusProviding {
    var isAuthorized = false
    var focus: FocusSnapshot?

    @discardableResult
    func requestAuthorization() -> Bool { isAuthorized }

    func currentFocus() -> FocusSnapshot? { focus }
}

@MainActor
private final class FakeOverlay: PetOverlayProviding {
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
private final class TestClock: @unchecked Sendable {
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
