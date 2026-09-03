// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

import Foundation
import RoamlingCore
import RoamlingCoreRs

/// The seam the port crosses on macOS.
///
/// Swift's own types stay on this side while the units come over one at a time,
/// so each function converts, calls, and converts back. That conversion is the
/// cost -- `docs/windows.md` section 12 measured the crossing itself at 27 ns
/// and the payload at everything else -- which is why these take a whole world
/// rather than being called per rectangle.
///
/// Windows will not have this file. Its shell links the crate and calls the
/// same functions with no boundary at all.
enum RustCore {
    private static func displays(_ world: DesktopWorldSnapshot) -> [FfiDisplay] {
        world.displays.map { display in
            FfiDisplay(
                id: display.id,
                frame: rect(display.frame),
                visibleFrame: rect(display.visibleFrame)
            )
        }
    }

    /// Shared with `RustPlacement`, which builds the same records.
    static func ffiRect(_ value: WorldRect) -> FfiRect { rect(value) }
    static func ffiLuminance(_ value: LuminanceField?) -> FfiLuminanceField? { luminance(value) }

    private static func rect(_ value: WorldRect) -> FfiRect {
        FfiRect(x: value.minX, y: value.minY, width: value.size.width, height: value.size.height)
    }

    private static func zone(_ value: FfiSafeZone) -> SafeZone {
        SafeZone(
            frame: WorldRect(
                x: value.frame.x,
                y: value.frame.y,
                width: value.frame.width,
                height: value.frame.height
            ),
            score: value.score,
            confidence: value.confidence,
            reason: value.reason
        )
    }

    static func safeZones(in world: DesktopWorldSnapshot) -> [SafeZone] {
        RoamlingCoreRs.safeZones(displays: displays(world)).map(zone)
    }

    private static func luminance(_ value: LuminanceField?) -> FfiLuminanceField? {
        value.map {
            FfiLuminanceField(
                bounds: rect($0.bounds),
                columns: UInt32($0.columns),
                rows: UInt32($0.rows),
                samples: $0.samples
            )
        }
    }

    /// Everything interest placement reads, in one value. The crossing is free
    /// and the serialization is not, so it is carried once rather than asked
    /// for piece by piece.
    private static func scene(
        for hint: LocationHint,
        in world: DesktopWorldSnapshot
    ) -> FfiInterestScene? {
        guard let region = hint.approximateRegion else { return nil }
        return FfiInterestScene(
            displays: displays(world),
            region: rect(region),
            hintConfidence: hint.confidence,
            focus: world.focus.map { focus in
                FfiFocus(
                    windowFrame: focus.windowFrame.map(rect),
                    focusedElementFrame: focus.focusedElementFrame.map(rect),
                    caretFrame: focus.caretFrame.map(rect),
                    confidence: focus.confidence
                )
            },
            field: luminance(world.luminance)
        )
    }

    static func interestDestination(
        for hint: LocationHint,
        in world: DesktopWorldSnapshot,
        currentPosition: WorldPoint,
        pointerPosition: WorldPoint?,
        objectSize: WorldSize
    ) -> InterestDestination? {
        // A hint with no region cannot be placed against, and the Swift planner
        // returned nil for it too.
        guard let scene = scene(for: hint, in: world) else { return nil }
        return RoamlingCoreRs.interestDestination(
            scene: scene,
            currentX: currentPosition.x,
            currentY: currentPosition.y,
            pointer: pointerPosition.map { [$0.x, $0.y] },
            objectWidth: objectSize.width,
            objectHeight: objectSize.height
        ).map {
            InterestDestination(
                point: WorldPoint(x: $0.x, y: $0.y),
                displayID: $0.displayId,
                score: $0.score
            )
        }
    }

    static func evaluateSeat(
        at point: WorldPoint,
        for hint: LocationHint,
        in world: DesktopWorldSnapshot,
        currentPosition: WorldPoint,
        pointerPosition: WorldPoint?,
        objectSize: WorldSize
    ) -> SeatEvaluation? {
        guard let scene = scene(for: hint, in: world) else { return nil }
        return RoamlingCoreRs.evaluateSeat(
            scene: scene,
            seatX: point.x,
            seatY: point.y,
            currentX: currentPosition.x,
            currentY: currentPosition.y,
            pointer: pointerPosition.map { [$0.x, $0.y] },
            objectWidth: objectSize.width,
            objectHeight: objectSize.height
        ).map {
            SeatEvaluation(
                point: WorldPoint(x: $0.x, y: $0.y),
                displayID: $0.displayId,
                score: $0.score,
                emptiness: $0.emptiness,
                coversCaret: $0.coversCaret,
                watchesRegion: $0.watchesRegion
            )
        }
    }

    /// Kinds and reactions cross as indices. Two spellings kept in step is
    /// cheaper than three, and the vocabulary is closed on both sides.
    private static let kindOrder: [CompanionEventKind] = [
        .activityStarted, .activityEnded, .positive, .negative, .achievement,
        .setback, .attentionRequired, .inspecting, .highIntensity, .calm, .idle
    ]
    private static let contextOrder: [UserContext] = [
        .working, .gaming, .watchingMedia, .browsing, .idle
    ]
    private static let reactionOrder: [CompanionReaction] = [
        .glance, .observe, .spark, .work, .paw,
        .smallCelebrate, .largeCelebrate, .sad, .calm
    ]

    static func activityEvent(_ event: CompanionEvent) -> FfiActivityEvent {
        FfiActivityEvent(
            id: event.id,
            sourceId: event.sourceID,
            timestamp: event.timestamp,
            kind: UInt8(kindOrder.firstIndex(of: event.kind) ?? 0),
            intensity: event.intensity,
            hintConfidence: event.locationHint?.confidence
        )
    }

    static func reaction(at index: UInt8) -> CompanionReaction? {
        let position = Int(index)
        return position < reactionOrder.count ? reactionOrder[position] : nil
    }

    static func contextIndex(_ context: UserContext) -> UInt8 {
        UInt8(contextOrder.firstIndex(of: context) ?? 4)
    }

    static func reactionIndex(_ reaction: CompanionReaction) -> UInt8 {
        UInt8(reactionOrder.firstIndex(of: reaction) ?? 0)
    }

    // MARK: - Tuning

    /// Spelled out rather than taken from `allCases`, because Swift sends the
    /// index and this list is a contract with `TUNING_KEYS` in `tuning.rs`.
    private static let tuningKeyOrder: [RuntimeTuningKey] = [
        .walkingSpeed, .wanderPause, .crossDisplayWanderChance, .idleBeforeRest,
        .pointerAwarenessDistance, .evadeSpeedScale, .catchArmDistance,
        .catchApproachSpeed, .catchWindow, .hitRegionScale, .gaitCadence
    ]

    static var tuningWireOrder: [RuntimeTuningKey] { tuningKeyOrder }

    @Sendable
    static func normalizeTuning(
        walkingSpeed: Double,
        wanderPause: Double,
        crossDisplayWanderChance: Double,
        pointerAwarenessDistance: Double,
        catchArmDistance: Double,
        catchApproachSpeed: Double,
        catchWindow: Double,
        hitRegionScale: Double,
        gaitCadence: Double,
        evadeSpeedScale: Double,
        idleBeforeRest: Double
    ) -> FfiTuning {
        RoamlingCoreRs.normalizeTuning(
            walkingSpeed: walkingSpeed,
            wanderPause: wanderPause,
            crossDisplayWanderChance: crossDisplayWanderChance,
            pointerAwarenessDistance: pointerAwarenessDistance,
            catchArmDistance: catchArmDistance,
            catchApproachSpeed: catchApproachSpeed,
            catchWindow: catchWindow,
            hitRegionScale: hitRegionScale,
            gaitCadence: gaitCadence,
            evadeSpeedScale: evadeSpeedScale,
            idleBeforeRest: idleBeforeRest
        )
    }

    static func tuningLimits(
        for key: RuntimeTuningKey,
        pointerAwareness: Double
    ) -> ClosedRange<Double> {
        let range = RoamlingCoreRs.tuningLimits(
            key: UInt8(tuningKeyOrder.firstIndex(of: key) ?? 0),
            pointerAwareness: pointerAwareness
        )
        return range.lower...range.upper
    }

    static func tuningFastEvadeSpeed(walkingSpeed: Double, evadeSpeedScale: Double) -> Double {
        RoamlingCoreRs.tuningFastEvadeSpeed(
            walkingSpeed: walkingSpeed, evadeSpeedScale: evadeSpeedScale
        )
    }

    static func tuningSlowEvadeSpeed(walkingSpeed: Double, evadeSpeedScale: Double) -> Double {
        RoamlingCoreRs.tuningSlowEvadeSpeed(
            walkingSpeed: walkingSpeed, evadeSpeedScale: evadeSpeedScale
        )
    }

    static func tuningWanderDelay(wanderPause: Double, randomUnit: Double) -> TimeInterval {
        RoamlingCoreRs.tuningWanderDelay(wanderPause: wanderPause, randomUnit: randomUnit)
    }

    static func napsInPlace(
        at position: WorldPoint,
        objectSize: WorldSize,
        in field: LuminanceField?,
        atLeast threshold: Double
    ) -> Bool {
        RoamlingCoreRs.napsInPlace(
            x: position.x,
            y: position.y,
            objectWidth: objectSize.width,
            objectHeight: objectSize.height,
            field: luminance(field),
            threshold: threshold
        )
    }

    static func restDestination(
        in world: DesktopWorldSnapshot,
        currentPosition: WorldPoint,
        pointerPosition: WorldPoint?,
        objectSize: WorldSize
    ) -> RestDestination? {
        let answer = RoamlingCoreRs.restDestination(
            displays: displays(world),
            zones: world.safeZones.map {
                FfiSafeZone(
                    frame: rect($0.frame),
                    score: $0.score,
                    confidence: $0.confidence,
                    reason: $0.reason
                )
            },
            currentX: currentPosition.x,
            currentY: currentPosition.y,
            pointer: pointerPosition.map { [$0.x, $0.y] },
            objectWidth: objectSize.width,
            objectHeight: objectSize.height
        )
        return answer.map {
            RestDestination(
                point: WorldPoint(x: $0.x, y: $0.y),
                displayID: $0.displayId,
                reason: $0.reason,
                score: $0.score
            )
        }
    }
}

/// The switch-over test compares the Rust answer against the Swift one it
/// replaced. Public only for that, and it goes when the Swift planner does.
public enum RustCoreTestBridge {
    public static func safeZones(in world: DesktopWorldSnapshot) -> [SafeZone] {
        RustCore.safeZones(in: world)
    }

    public static func napsInPlace(
        at position: WorldPoint,
        objectSize: WorldSize,
        in field: LuminanceField?,
        atLeast threshold: Double
    ) -> Bool {
        RustCore.napsInPlace(
            at: position, objectSize: objectSize, in: field, atLeast: threshold
        )
    }

    public static func restDestination(
        in world: DesktopWorldSnapshot,
        currentPosition: WorldPoint,
        pointerPosition: WorldPoint?,
        objectSize: WorldSize
    ) -> RestDestination? {
        RustCore.restDestination(
            in: world,
            currentPosition: currentPosition,
            pointerPosition: pointerPosition,
            objectSize: objectSize
        )
    }
}

/// The Rust core answering the interest questions, so the director asks it
/// without knowing which side of the boundary the answer came from.
struct RustInterestPlanner: InterestPlacing {
    func destination(
        for hint: LocationHint,
        in world: DesktopWorldSnapshot,
        currentPosition: WorldPoint,
        pointerPosition: WorldPoint?,
        objectSize: WorldSize
    ) -> InterestDestination? {
        RustCore.interestDestination(
            for: hint, in: world, currentPosition: currentPosition,
            pointerPosition: pointerPosition, objectSize: objectSize
        )
    }

    func evaluateSeat(
        at point: WorldPoint,
        for hint: LocationHint,
        in world: DesktopWorldSnapshot,
        currentPosition: WorldPoint,
        pointerPosition: WorldPoint?,
        objectSize: WorldSize
    ) -> SeatEvaluation? {
        RustCore.evaluateSeat(
            at: point, for: hint, in: world, currentPosition: currentPosition,
            pointerPosition: pointerPosition, objectSize: objectSize
        )
    }
}

public extension RustCoreTestBridge {
    /// The Rust planner, for the test that runs it beside the Swift one.
    static var interestPlanner: any InterestPlacing { RustInterestPlanner() }

    /// The order tuning keys cross as indices, so a test can pin it against
    /// `RuntimeTuningKey.allCases`.
    static var tuningWireOrder: [RuntimeTuningKey] { RustCore.tuningWireOrder }

    /// A fresh pair of the stateful models, so the switch-over test can drive
    /// them through the same script it drives the Swift ones through.
    static func makeAttention() -> (
        select: ([CompanionEvent], TimeInterval) -> String?,
        clear: (TimeInterval) -> Void,
        currentSourceID: () -> String?
    ) {
        let handle = Attention()
        return (
            select: { events, timestamp in
                handle.select(
                    events: events.map(RustCore.activityEvent),
                    timestamp: timestamp
                )
            },
            clear: { handle.clear(timestamp: $0) },
            currentSourceID: { handle.currentSourceId() }
        )
    }

    static func makeReactions() -> (
        CompanionEvent, UserContext, Bool, Double, TimeInterval
    ) -> CompanionReaction? {
        let handle = Reactions()
        return { event, context, heldByPointer, roll, timestamp in
            handle.reaction(
                event: RustCore.activityEvent(event),
                context: RustCore.contextIndex(context),
                isHeldByPointer: heldByPointer,
                randomUnit: roll,
                timestamp: timestamp
            ).flatMap(RustCore.reaction(at:))
        }
    }
}

// MARK: - The per-tick models

/// The wire order for `BehaviorState`, spelled out rather than taken from
/// `allCases`. Swift sends the index, so this list and `BEHAVIOR_STATES` in
/// `behavior.rs` are one contract; `CoreLogicTests` pins it against `allCases`
/// so that adding a state in the middle fails a test instead of renaming eight
/// of them at runtime.
public let behaviorStateOrder: [BehaviorState] = [
    .idle, .wander, .lookAtPointer, .evadePointer, .caught, .dragged, .dropped,
    .sit, .findSleepSpot, .sleep, .wake, .stretch, .travelToInterest, .observe,
    .spark, .work, .waitingForUser, .celebrate, .sad
]

private let pointerProximityOrder: [PointerProximity] = [
    .far, .watching, .slowEvade, .fastEvade, .catchable
]

/// `MovementController` with its state in Rust.
///
/// Public only so the switch-over test can drive it beside the Swift
/// original it replaced. That goes when the original does.
///
/// A class, not a struct: the state lives behind the handle, so two copies of a
/// value type would share it and only look independent. The runtime holds
/// exactly one and mutates it in place, which is what this is.
public final class RustMovement {
    private let handle: Movement

    public init(position: WorldPoint, velocity: WorldVector, configuration: MovementConfiguration) {
        handle = Movement(
            x: position.x,
            y: position.y,
            dx: velocity.dx,
            dy: velocity.dy,
            maximumSpeed: configuration.maximumSpeed,
            acceleration: configuration.acceleration,
            deceleration: configuration.deceleration,
            arrivalRadius: configuration.arrivalRadius
        )
    }

    public var position: WorldPoint {
        let point = handle.position()
        return WorldPoint(x: point.x, y: point.y)
    }

    public var velocity: WorldVector {
        let value = handle.velocity()
        return WorldVector(dx: value.x, dy: value.y)
    }

    /// Assigned straight through, the way Swift's
    /// `movement.configuration.maximumSpeed = ...` was. Running the
    /// initialiser's clamp here would be a behaviour change dressed as tidying.
    public var maximumSpeed: Double {
        get { handle.maximumSpeed() }
        set { handle.setMaximumSpeed(value: newValue) }
    }

    public var hasRoute: Bool { handle.hasRoute() }

    public var destination: WorldPoint? {
        handle.destination().map { WorldPoint(x: $0.x, y: $0.y) }
    }

    public func setRoute(_ waypoints: [WorldPoint]) {
        handle.setRoute(waypoints: waypoints.map { FfiPoint(x: $0.x, y: $0.y) })
    }

    public func cancelRoute(stop: Bool = false) {
        handle.cancelRoute(stop: stop)
    }

    public func teleport(to point: WorldPoint, stop: Bool = true) {
        handle.teleport(x: point.x, y: point.y, stop: stop)
    }

    public func setVelocity(_ newVelocity: WorldVector) {
        handle.setVelocity(dx: newVelocity.dx, dy: newVelocity.dy)
    }

    @discardableResult
    public func update(deltaTime: TimeInterval) -> MovementUpdate {
        let update = handle.update(deltaTime: deltaTime)
        return MovementUpdate(
            position: WorldPoint(x: update.x, y: update.y),
            velocity: WorldVector(dx: update.dx, dy: update.dy),
            reachedDestination: update.reachedDestination
        )
    }
}

/// `BehaviorController` with its state in Rust.
public final class RustBehavior {
    private let handle: Behavior

    public init(state: BehaviorState = .idle, enteredAt: TimeInterval = 0) {
        handle = Behavior(
            state: UInt8(behaviorStateOrder.firstIndex(of: state) ?? 0),
            enteredAt: enteredAt
        )
    }

    public var state: BehaviorState { behaviorStateOrder[Int(handle.state())] }
    public var enteredAt: TimeInterval { handle.enteredAt() }

    @discardableResult
    public func handle(_ input: BehaviorInput, at timestamp: TimeInterval) -> BehaviorTransition {
        let code: UInt8
        var argument: UInt8 = 0
        switch input {
        case .beginWander: code = 0
        case .arrived: code = 1
        case .beginRest: code = 2
        case .seekSleepSpot: code = 3
        case .sleepSpotReached: code = 4
        case .beginStretch: code = 5
        case .beginInterestTravel: code = 6
        case let .pointer(proximity):
            code = 7
            argument = UInt8(pointerProximityOrder.firstIndex(of: proximity) ?? 0)
        case .catchBegan: code = 8
        case .dragMoved: code = 9
        case .mouseReleased: code = 10
        case let .reaction(reaction):
            code = 11
            argument = RustCore.reactionIndex(reaction)
        case .meaningfulActivity: code = 12
        case .tick: code = 13
        }
        let transition = handle.handle(input: code, argument: argument, timestamp: timestamp)
        return BehaviorTransition(
            from: behaviorStateOrder[Int(transition.from)],
            to: behaviorStateOrder[Int(transition.to)],
            changed: transition.changed
        )
    }
}

/// `PointerInteractionModel` with its state in Rust. The state is one previous
/// sample, and it is the whole point: a cursor resting beside the pet and a
/// cursor grabbing for it are the same coordinate.
public final class RustPointerModel {
    private let handle = Pointer()

    public init(configuration: PointerInteractionConfiguration) {
        self.configuration = configuration
        // Explicit, because `didSet` does not fire for an assignment made
        // inside `init`. Leaving it implicit left the handle on its own
        // defaults until the first tuning change, and the pet watched the
        // cursor at the wrong rate until then.
        push(configuration)
    }

    public var configuration: PointerInteractionConfiguration = .init() {
        didSet { push(configuration) }
    }

    private func push(_ configuration: PointerInteractionConfiguration) {
        handle.setConfiguration(
            awarenessDistance: configuration.awarenessDistance,
            slowEvadeDistance: configuration.slowEvadeDistance,
            fastEvadeDistance: configuration.fastEvadeDistance,
            catchDistance: configuration.catchDistance,
            slowEvadeSpeed: configuration.slowEvadeSpeed,
            fastEvadeSpeed: configuration.fastEvadeSpeed,
            catchPointerSpeed: configuration.catchPointerSpeed,
            catchClosingSpeed: configuration.catchClosingSpeed
        )
    }

    public func reset() { handle.reset() }

    public func evaluate(
        pointer: WorldPoint,
        pet: WorldPoint,
        timestamp: TimeInterval
    ) -> PointerDecision {
        let decision = handle.evaluate(
            pointerX: pointer.x,
            pointerY: pointer.y,
            petX: pet.x,
            petY: pet.y,
            timestamp: timestamp
        )
        return PointerDecision(
            proximity: pointerProximityOrder[Int(decision.proximity)],
            kinematics: PointerKinematics(
                velocity: WorldVector(dx: decision.velocityDx, dy: decision.velocityDy),
                speed: decision.speed,
                distanceToPet: decision.distanceToPet,
                closingSpeed: decision.closingSpeed
            ),
            escapeVelocity: WorldVector(dx: decision.escapeDx, dy: decision.escapeDy),
            lookDirectionDegrees: decision.lookDirectionDegrees,
            attentionRate: decision.attentionRate
        )
    }
}

/// `PlacementDirector` with its seat, its trip and its last review held in Rust.
///
/// Public only so the switch-over test can drive it beside the Swift original
/// it replaced. That goes when the original does.
public final class RustPlacement {
    private let handle = Placement()
    /// Pushed only when they change. The luminance grid is 64 columns wide and
    /// refreshes every three seconds at most, so sending it with every tick
    /// would be the one shape that makes this crossing cost something -- and
    /// comparing the samples is far cheaper than marshalling them.
    private var pushedDisplays: [DisplaySnapshot]?
    private var pushedField: LuminanceField?

    private static let reasonOrder: [PlacementTravelReason] = [
        .newActivity, .coveringCaret, .coveringWork, .plannedBlind, .followedFocus
    ]

    public init() {}

    public var isSeated: Bool { handle.isSeated() }
    public var isTravelling: Bool { handle.isTravelling() }

    public func settleInPlace(sourceID: String?, at timestamp: TimeInterval) {
        handle.settleInPlace(sourceId: sourceID, timestamp: timestamp)
    }

    public func decide(_ situation: PetSituation) -> PlacementIntent {
        if pushedDisplays != situation.world.displays {
            pushedDisplays = situation.world.displays
            handle.setDisplays(displays: situation.world.displays.map { display in
                FfiDisplay(
                    id: display.id,
                    frame: RustCore.ffiRect(display.frame),
                    visibleFrame: RustCore.ffiRect(display.visibleFrame)
                )
            })
        }
        if pushedField != situation.world.luminance {
            pushedField = situation.world.luminance
            handle.setField(field: RustCore.ffiLuminance(situation.world.luminance))
        }

        let answer = handle.decide(situation: FfiSituation(
            timestamp: situation.timestamp,
            x: situation.position.x,
            y: situation.position.y,
            objectWidth: situation.objectSize.width,
            objectHeight: situation.objectSize.height,
            pointer: situation.pointerPosition.map { [$0.x, $0.y] },
            walkingSpeed: situation.walkingSpeed,
            isPointerOwned: situation.isPointerOwned,
            isPointerWatching: situation.isPointerWatching,
            isEvading: situation.isEvading,
            isWalking: situation.isWalking,
            isResting: situation.isResting,
            activitySourceId: situation.activitySourceID,
            hint: situation.activityHint.map {
                FfiHint(
                    region: $0.approximateRegion.map(RustCore.ffiRect),
                    confidence: $0.confidence
                )
            },
            focus: situation.world.focus.map { focus in
                FfiFocus(
                    windowFrame: focus.windowFrame.map(RustCore.ffiRect),
                    focusedElementFrame: focus.focusedElementFrame.map(RustCore.ffiRect),
                    caretFrame: focus.caretFrame.map(RustCore.ffiRect),
                    confidence: focus.confidence
                )
            },
            userIdleDuration: situation.userIdleDuration,
            idleBeforeRest: situation.idleBeforeRest,
            isRoamingEnabled: situation.isRoamingEnabled,
            isStrollDue: situation.isStrollDue,
            strollCandidates: situation.strollCandidates.map { FfiPoint(x: $0.x, y: $0.y) }
        ))

        return switch answer.kind {
        case 0: .none
        case 2: .travel(
            InterestDestination(
                point: WorldPoint(x: answer.x, y: answer.y),
                displayID: answer.displayId,
                score: answer.score
            ),
            reason: Self.reasonOrder[Int(answer.reason)]
        )
        case 3: .sleepInPlace
        case 4: .stroll(WorldPoint(x: answer.x, y: answer.y))
        case 5: .escape(WorldPoint(x: answer.x, y: answer.y))
        default: .hold
        }
    }
}
