// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

import Foundation

/// Why the director is sending the pet somewhere.
///
/// The reason is not decoration. A caller has to know whether it may interrupt
/// a nap to obey the move, and every placement bug so far was easier to read as
/// "it travelled for the wrong reason" than as a wrong coordinate.
public enum PlacementTravelReason: String, Hashable, Sendable {
    /// A source the pet was not already watching started working.
    case newActivity
    case coveringCaret
    case coveringWork
    /// The seat was chosen before any capture existed and one has since
    /// arrived, so the decision gets re-made rather than defended.
    case plannedBlind
    /// The seat no longer belongs to the window being watched.
    case followedFocus
}

/// The single answer to "where should the pet be right now".
public enum PlacementIntent: Equatable, Sendable {
    /// Something else owns the pet — a catch, a drag, a pointer reaction, or an
    /// evade. The seat verdict behind this was still computed; only the move
    /// waits, so the tick the pointer lets go acts on a current answer.
    case none
    case hold
    case travel(InterestDestination, reason: PlacementTravelReason)
    case sleepInPlace
    case stroll(WorldPoint)

    public var travelReason: PlacementTravelReason? {
        guard case let .travel(_, reason) = self else { return nil }
        return reason
    }
}

/// Everything the placement decision is allowed to look at, gathered once per
/// tick by the platform adapter.
///
/// It is a value type on purpose. The decision used to read seventeen mutable
/// fields on the runtime that four separate code paths wrote to, and every
/// placement defect in MVP 4 was one path failing to set what another read.
public struct PetSituation: Sendable {
    public var timestamp: TimeInterval
    /// Displays, plus focus and luminance when the user granted them.
    public var world: DesktopWorldSnapshot
    public var position: WorldPoint
    public var objectSize: WorldSize
    public var pointerPosition: WorldPoint?
    public var walkingSpeed: Double
    /// The pointer owns the pet: caught, dragged, or reacting to a near cursor.
    public var isPointerOwned: Bool
    public var isEvading: Bool
    /// A walk is already under way, so nothing here should start another one.
    public var isWalking: Bool
    /// The pet is sitting, seeking a sleep spot, or asleep. Rest owns movement
    /// while that lasts, so planning a stroll it cannot take is wasted work.
    public var isResting: Bool
    public var activitySourceID: String?
    public var activityHint: LocationHint?
    public var userIdleDuration: TimeInterval
    public var idleBeforeRest: TimeInterval
    public var isRoamingEnabled: Bool
    /// The roaming pause has run out. Pacing belongs to the caller because a
    /// catch, a drop and a display change all extend it for reasons that have
    /// nothing to do with placement.
    public var isStrollDue: Bool
    /// Aimless destinations for the director to filter. Keeping the sampling
    /// outside means roaming stays random without the decision being random.
    public var strollCandidates: [WorldPoint]

    public init(
        timestamp: TimeInterval,
        world: DesktopWorldSnapshot,
        position: WorldPoint,
        objectSize: WorldSize,
        pointerPosition: WorldPoint? = nil,
        walkingSpeed: Double = 160,
        isPointerOwned: Bool = false,
        isEvading: Bool = false,
        isWalking: Bool = false,
        isResting: Bool = false,
        activitySourceID: String? = nil,
        activityHint: LocationHint? = nil,
        userIdleDuration: TimeInterval = 0,
        idleBeforeRest: TimeInterval = .infinity,
        isRoamingEnabled: Bool = true,
        isStrollDue: Bool = false,
        strollCandidates: [WorldPoint] = []
    ) {
        self.timestamp = timestamp
        self.world = world
        self.position = position
        self.objectSize = objectSize
        self.pointerPosition = pointerPosition
        self.walkingSpeed = walkingSpeed
        self.isPointerOwned = isPointerOwned
        self.isEvading = isEvading
        self.isWalking = isWalking
        self.isResting = isResting
        self.activitySourceID = activitySourceID
        self.activityHint = activityHint
        self.userIdleDuration = userIdleDuration
        self.idleBeforeRest = idleBeforeRest
        self.isRoamingEnabled = isRoamingEnabled
        self.isStrollDue = isStrollDue
        self.strollCandidates = strollCandidates
    }
}

/// The one place that answers where the pet should be.
///
/// Placement used to be decided in four unrelated code paths that shared
/// mutable runtime state, so a rule added to one of them silently did not apply
/// to the other three, and none of it could be tested without running the app.
/// `docs/placement.md` records the decision table this implements and why the
/// thresholds are asymmetric.
public struct PlacementDirector: Sendable {
    public struct Configuration: Equatable, Sendable {
        /// A seat has to look at least this empty to be worth taking or keeping.
        public var holdEmptiness: Double
        /// The bar for walking away from a seat, deliberately the same as the
        /// bar for taking one.
        ///
        /// A lower bar was tried and measured: on a real 1728x1117 desktop it
        /// turned 15% of the screen into seats that are on text and yet not bad
        /// enough to leave, because real text scores 0.35...0.55 far more often
        /// than it scores 0. The seat twitching this was meant to stop is fixed
        /// where it actually comes from -- see `replacementMargin`.
        public var abandonEmptiness: Double
        /// How long a fresh seat is defended against `coveringWork` alone, so
        /// that a screen changing under the pet cannot move it at frame rate.
        public var seatDwell: TimeInterval
        /// Scoring a seat is cheap, planning a new one is not, and neither is
        /// worth doing at frame rate.
        public var reviewInterval: TimeInterval
        /// How much better a replacement has to be when it is not itself clear.
        ///
        /// This is the actual fix for the seat that would not settle. The pet
        /// used to leave a marginal seat for another marginal seat, and the new
        /// one flickered across the same line the old one did, so it paced. A
        /// replacement that is genuinely empty ends the walk in one move; a
        /// replacement that is not has to clear this margin to be worth taking.
        public var replacementMargin: Double
        /// Below this a "new" seat is the seat the pet already has.
        public var reseatDistance: Double
        /// Shorter than this is not a walk worth watching.
        public var minimumTravelDistance: Double
        public var arrivalTolerance: Double

        public init(
            holdEmptiness: Double = BasicInterestPositionPlanner.holdEmptiness,
            abandonEmptiness: Double = BasicInterestPositionPlanner.holdEmptiness,
            seatDwell: TimeInterval = 2.5,
            reviewInterval: TimeInterval = 0.5,
            replacementMargin: Double = 15,
            reseatDistance: Double = 24,
            minimumTravelDistance: Double = 18,
            arrivalTolerance: Double = 4
        ) {
            self.holdEmptiness = holdEmptiness.clamped(to: 0...1)
            self.abandonEmptiness = min(abandonEmptiness.clamped(to: 0...1), self.holdEmptiness)
            self.seatDwell = max(0, seatDwell)
            self.reviewInterval = max(0, reviewInterval)
            self.replacementMargin = max(0, replacementMargin)
            self.reseatDistance = max(0, reseatDistance)
            self.minimumTravelDistance = max(0, minimumTravelDistance)
            self.arrivalTolerance = max(0.5, arrivalTolerance)
        }

        public static let standard = Configuration()
    }

    /// A seat the pet is standing on. Its coordinate is deliberately absent:
    /// the pet is the seat, so a drag or a display change cannot leave the
    /// director judging a spot the pet no longer occupies.
    private struct Seat {
        var sourceID: String
        var takenAt: TimeInterval
        /// Whether a capture existed when this seat was chosen. Without one the
        /// planner never sweeps the window and only the seats beside it remain,
        /// which is how a pet ends up in a corner for a whole session.
        var sawCapture: Bool
    }

    private struct Travel {
        var destination: InterestDestination
        var reason: PlacementTravelReason
        var sourceID: String
        var startedAt: TimeInterval
        var sawCapture: Bool
    }

    public let configuration: Configuration
    private var seat: Seat?
    private var travel: Travel?
    private var parkedSince: TimeInterval?
    private var lastReviewAt: TimeInterval = -.infinity
    /// The verdict from the last review, repeated between beats so a walk in
    /// progress keeps its destination instead of restarting every frame.
    private var carried: PlacementIntent = .hold

    public init(configuration: Configuration = .standard) {
        self.configuration = configuration
    }

    /// True while the pet is parked on a seat it picked for the current source.
    public var isSeated: Bool { seat != nil && travel == nil }
    public var isTravelling: Bool { travel != nil }

    public mutating func decide(_ situation: PetSituation) -> PlacementIntent {
        let verdict = verdict(for: situation)
        // Priorities 1 and 2. The table above them was still read, so the
        // verdict is current when the pointer lets go. Gating the reading as
        // well as the moving is what froze the seat watch next to the cursor.
        if situation.isPointerOwned || situation.isEvading { return .none }
        return verdict
    }

    /// Drops the trip in progress and treats where the pet stands as its seat.
    /// A setback ends the walk without ending the watch, so the caller names
    /// the source the seat now belongs to rather than letting it keep the one
    /// the abandoned trip was for.
    public mutating func settleInPlace(
        sourceID owner: String?,
        at timestamp: TimeInterval
    ) {
        guard let sourceID = owner ?? travel?.sourceID ?? seat?.sourceID else { return }
        seat = Seat(
            sourceID: sourceID,
            takenAt: timestamp,
            sawCapture: travel?.sawCapture ?? seat?.sawCapture ?? false
        )
        travel = nil
        carried = .hold
    }

    private mutating func verdict(for situation: PetSituation) -> PlacementIntent {
        guard let sourceID = situation.activitySourceID,
              let hint = situation.activityHint else {
            seat = nil
            travel = nil
            return strollVerdict(situation)
        }
        parkedSince = nil

        // A different agent is a different window. The pet walks over to it
        // rather than claiming wherever it happens to be standing.
        if seat?.sourceID != sourceID { seat = nil }
        if travel?.sourceID != sourceID { travel = nil }

        // Arrival runs every tick, not on the review beat: a pet that reached
        // its seat has to react now, not up to half a second later.
        if let travel, situation.position.distance(to: travel.destination.point)
            <= configuration.arrivalTolerance {
            return settle(
                sourceID: sourceID,
                sawCapture: travel.sawCapture || situation.world.luminance != nil,
                at: situation.timestamp
            )
        }
        // A walk that cannot finish must not own the pet for the rest of the
        // session, however it got stuck.
        if let travel, situation.timestamp - travel.startedAt > timeout(for: travel, in: situation) {
            return settle(
                sourceID: sourceID,
                sawCapture: travel.sawCapture,
                at: situation.timestamp
            )
        }

        let isNew = seat == nil && travel == nil
        guard isNew || situation.timestamp - lastReviewAt >= configuration.reviewInterval else {
            return carried
        }
        lastReviewAt = situation.timestamp

        let judged = travel?.destination.point ?? situation.position
        let evaluation = BasicInterestPositionPlanner.evaluateSeat(
            at: judged,
            for: hint,
            in: situation.world,
            currentPosition: situation.position,
            pointerPosition: situation.pointerPosition,
            objectSize: situation.objectSize
        )

        if let reason = departureReason(
            evaluation: evaluation,
            sawCapture: travel?.sawCapture ?? seat?.sawCapture ?? false,
            isNew: isNew,
            in: situation
        ),
           let destination = BasicInterestPositionPlanner.destination(
            for: hint,
            in: situation.world,
            currentPosition: situation.position,
            pointerPosition: situation.pointerPosition,
            objectSize: situation.objectSize
           ),
           accepts(
            destination,
            over: evaluation,
            judged: judged,
            for: reason,
            hint: hint,
            in: situation
           ) {
            travel = Travel(
                destination: destination,
                reason: reason,
                sourceID: sourceID,
                startedAt: situation.timestamp,
                sawCapture: situation.world.luminance != nil
            )
            seat = nil
            carried = .travel(destination, reason: reason)
            return carried
        }

        // Nothing better exists, so the walk already under way continues rather
        // than being restarted from here.
        if let travel {
            carried = .travel(travel.destination, reason: travel.reason)
            return carried
        }

        if seat == nil {
            // No seat was worth walking to, which still means this window is
            // the one being watched. Leaving that unrecorded is what stranded
            // the old seat watch for a whole session.
            seat = Seat(
                sourceID: sourceID,
                takenAt: situation.timestamp,
                sawCapture: situation.world.luminance != nil
            )
        }

        // Priority 7. A pet dozing beside a working agent keeps the seat it
        // already vetted instead of walking to a display corner to sleep.
        // "Cannot tell" reads as fine here, the same way a missing capture does.
        if evaluation?.isHoldable ?? true,
           situation.userIdleDuration >= situation.idleBeforeRest {
            carried = .sleepInPlace
            return carried
        }
        carried = .hold
        return carried
    }

    /// Priorities 3 through 6, in order. Above them is only ownership, below
    /// them only staying put.
    private func departureReason(
        evaluation: SeatEvaluation?,
        sawCapture: Bool,
        isNew: Bool,
        in situation: PetSituation
    ) -> PlacementTravelReason? {
        if isNew { return .newActivity }
        // No answer is not a bad answer. Moving because the seat could not be
        // scored would walk the pet on exactly the screens it understands least.
        guard let evaluation else { return nil }
        if evaluation.coversCaret { return .coveringCaret }
        if let emptiness = evaluation.emptiness,
           emptiness < configuration.abandonEmptiness,
           dwellElapsed(in: situation) {
            return .coveringWork
        }
        if !sawCapture, situation.world.luminance != nil { return .plannedBlind }
        if !evaluation.watchesRegion { return .followedFocus }
        return nil
    }

    private func dwellElapsed(in situation: PetSituation) -> Bool {
        guard let seat else { return true }
        return situation.timestamp - seat.takenAt >= configuration.seatDwell
    }

    private func accepts(
        _ destination: InterestDestination,
        over evaluation: SeatEvaluation?,
        judged: WorldPoint,
        for reason: PlacementTravelReason,
        hint: LocationHint,
        in situation: PetSituation
    ) -> Bool {
        guard situation.position.distance(to: destination.point)
                > configuration.minimumTravelDistance,
              destination.point.distance(to: judged) > configuration.reseatDistance
        else { return false }

        switch reason {
        case .coveringWork:
            // A seat that is genuinely empty ends this in one move: measured on
            // a real desktop a clear seat scores around 0.97, nowhere near the
            // bar it would have to fall back under to move the pet again.
            let replacement = BasicInterestPositionPlanner.evaluateSeat(
                at: destination.point,
                for: hint,
                in: situation.world,
                currentPosition: situation.position,
                pointerPosition: situation.pointerPosition,
                objectSize: situation.objectSize
            )
            if replacement?.isHoldable == true { return true }
            // Otherwise the pet would be trading one marginal seat for another,
            // and that trade has to be worth watching.
            guard let evaluation else { return true }
            return destination.score > evaluation.score + configuration.replacementMargin
        case .newActivity:
            // Walking over is the point of this priority, but only when there
            // is somewhere better to be. Without a caret the strongest pull
            // left is the window's bottom edge, and on a full-screen window
            // that is the corner of the display -- not a reason to leave a
            // seat that is already clear.
            //
            // Being on another display is a reason, and the score does not say
            // so: measured across two displays the corner seat beat a clear one
            // on the wrong screen by 6.9, well under the margin. Watching the
            // region is the question, so it is asked directly.
            guard let evaluation, evaluation.watchesRegion else { return true }
            return destination.score > evaluation.score + configuration.replacementMargin
        case .coveringCaret, .plannedBlind, .followedFocus:
            return true
        }
    }

    private mutating func settle(
        sourceID: String,
        sawCapture: Bool,
        at timestamp: TimeInterval
    ) -> PlacementIntent {
        seat = Seat(sourceID: sourceID, takenAt: timestamp, sawCapture: sawCapture)
        travel = nil
        lastReviewAt = timestamp
        carried = .hold
        return carried
    }

    /// Long enough for the walk plus the slowdown at every waypoint, and never
    /// so short that a legitimate cross-display trip counts as stuck.
    private func timeout(for travel: Travel, in situation: PetSituation) -> TimeInterval {
        let distance = situation.position.distance(to: travel.destination.point)
        return 8 + distance / max(20, situation.walkingSpeed) * 2
    }

    /// Priorities 10 and 11. Wandering is where the pet spends most of its life,
    /// so it passes the same emptiness bar as an interest seat — a rule that
    /// only applied to agent seats left most of the day unruled.
    private mutating func strollVerdict(_ situation: PetSituation) -> PlacementIntent {
        carried = .hold
        guard situation.isRoamingEnabled,
              let first = situation.strollCandidates.first else {
            parkedSince = nil
            return carried
        }
        if situation.isWalking || situation.isResting {
            parkedSince = nil
            return carried
        }
        if parkedSince == nil { parkedSince = situation.timestamp }

        if situation.isStrollDue {
            parkedSince = nil
            return .stroll(comfortable(among: situation) ?? first)
        }

        // The pause between walks is the whole point of roaming, and it is also
        // long enough for the user to scroll a paragraph under a pet that is
        // just sitting there. Nothing else is watching during it, so this is.
        guard let field = situation.world.luminance,
              let parkedSince,
              situation.timestamp - parkedSince >= configuration.seatDwell,
              let score = VisualEmptiness.score(
                of: frame(at: situation.position, size: situation.objectSize),
                in: field
              ),
              score < configuration.holdEmptiness,
              let escape = comfortable(among: situation),
              // Only somewhere genuinely clear, for the same reason a seat is:
              // trading one covered spot for another just paces the pet.
              let escapeScore = VisualEmptiness.score(
                of: frame(at: escape, size: situation.objectSize),
                in: field
              ),
              escapeScore >= configuration.holdEmptiness,
              situation.position.distance(to: escape) > configuration.minimumTravelDistance
        else { return carried }

        self.parkedSince = nil
        return .stroll(escape)
    }

    private func comfortable(among situation: PetSituation) -> WorldPoint? {
        situation.world.luminance.flatMap {
            VisualEmptiness.firstComfortable(
                among: situation.strollCandidates,
                objectSize: situation.objectSize,
                in: $0,
                atLeast: configuration.holdEmptiness
            )
        }
    }

    private func frame(at point: WorldPoint, size: WorldSize) -> WorldRect {
        WorldRect(
            x: point.x - size.width / 2,
            y: point.y - size.height / 2,
            width: size.width,
            height: size.height
        )
    }
}
