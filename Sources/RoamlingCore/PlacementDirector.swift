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
    /// The cursor has settled on the seat the pet is walking to. The glance
    /// band stops the pet short of it every time, so the walk is one it cannot
    /// finish. Only ever raised for a walk in progress: a cursor passing a
    /// seated pet is a glance, not a reason to get up.
    case seatUnderPointer
    case coveringWork
    /// The seat was chosen before any capture existed and one has since
    /// arrived, so the decision gets re-made rather than defended.
    case plannedBlind
    /// The seat no longer belongs to the window being watched.
    case followedFocus

    /// Whether a walk for this reason keeps going when the cursor is merely
    /// being looked at. Two of these walks exist because the pet is standing on
    /// the user's work, and a glance must not cancel the remedy for that. The
    /// third exists because the cursor sat on the seat: a walk the cursor
    /// started, stopping to look at the cursor, would be a contradiction. The
    /// remaining reasons are about a better seat, not a bad one, so they wait.
    public var keepsWalkingPastGlance: Bool {
        switch self {
        case .coveringCaret, .coveringWork, .seatUnderPointer: return true
        case .newActivity, .plannedBlind, .followedFocus: return false
        }
    }
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
    /// The spot the pet is standing on turned out to be covered, so this is a
    /// walk it owes the user rather than one it fancied. Kept apart from
    /// `.stroll` because callers rank it differently: an aimless walk yields to
    /// the cursor, and getting off someone's paragraph does not.
    case escape(WorldPoint)

    public var travelReason: PlacementTravelReason? {
        guard case let .travel(_, reason) = self else { return nil }
        return reason
    }

    /// The walks the glance may not stop. Stopping to look at the cursor is a
    /// moment; standing on someone's paragraph is a condition, and a moment
    /// must not cancel the remedy for a condition. Roaming's `.escape` and the
    /// seat-watch's covering travels are the same remedy under two names, and
    /// the walk away from a cursor-blocked seat is one the cursor itself
    /// started. Everything else waits for the glance to pass.
    public var outranksGlance: Bool {
        switch self {
        case .escape: return true
        case let .travel(_, reason): return reason.keepsWalkingPastGlance
        case .none, .hold, .sleepInPlace, .stroll: return false
        }
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
    /// No seat within this distance of the cursor can be reached: the glance
    /// band stops the pet where it stands. The notice distance while pointer
    /// avoidance is on, zero when it is off -- and at zero nothing is blocked.
    public var pointerClearance: Double
    public var walkingSpeed: Double
    /// The pointer owns the pet outright: caught, dragged, evading, or close
    /// enough to be reaching for it. Nothing placement decides survives this.
    public var isPointerOwned: Bool
    /// The weaker claim: the cursor is in the outer band and the pet has
    /// stopped to look at it. It is a moment, and it stops the pet exactly
    /// where it stands -- which may be the paragraph the user is reading.
    public var isPointerWatching: Bool
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
        pointerClearance: Double = 0,
        walkingSpeed: Double = 160,
        isPointerOwned: Bool = false,
        isPointerWatching: Bool = false,
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
        self.pointerClearance = pointerClearance
        self.walkingSpeed = walkingSpeed
        self.isPointerOwned = isPointerOwned
        self.isPointerWatching = isPointerWatching
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
    private let planner: any InterestPlacing
    private var seat: Seat?
    private var travel: Travel?
    private var parkedSince: TimeInterval?
    private var lastReviewAt: TimeInterval = -.infinity
    /// The verdict from the last review, repeated between beats so a walk in
    /// progress keeps its destination instead of restarting every frame.
    private var carried: PlacementIntent = .hold

    public init(
        configuration: Configuration = .standard,
        planner: any InterestPlacing = SwiftInterestPlanner()
    ) {
        self.configuration = configuration
        self.planner = planner
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
        // The glance is the one pointer claim that loses, and only to the
        // answers that remedy a condition. Standing on the user's work is a
        // condition; looking up at the cursor is a moment, and the moment
        // happens to stop the pet right where the condition is -- whether the
        // walk off it is roaming's escape or the seat watch's covering travel.
        // Everything else still waits.
        if situation.isPointerWatching, !verdict.outranksGlance { return .none }
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
            // Whether the seat was chosen blind is a fact about the moment it
            // was chosen, and the walk to it is exactly when the capture tends
            // to land. Folding the arrival's capture in here marked every blind
            // seat as informed and switched `plannedBlind` off in practice.
            return settle(
                sourceID: sourceID,
                sawCapture: travel.sawCapture,
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
        let evaluation = planner.evaluateSeat(
            at: judged,
            for: hint,
            in: situation.world,
            currentPosition: situation.position,
            pointerPosition: situation.pointerPosition,
            pointerClearance: situation.pointerClearance,
            objectSize: situation.objectSize
        )
        let sawCapture = travel?.sawCapture ?? seat?.sawCapture ?? false

        if let reason = departureReason(
            evaluation: evaluation,
            sawCapture: sawCapture,
            isNew: isNew,
            in: situation
        ) {
            if let destination = planner.destination(
                for: hint,
                in: situation.world,
                currentPosition: situation.position,
                pointerPosition: situation.pointerPosition,
                pointerClearance: situation.pointerClearance,
                objectSize: situation.objectSize
            ), accepts(
                destination,
                over: evaluation,
                judged: judged,
                for: reason,
                hint: hint,
                in: situation
            ) {
                return depart(to: destination, for: reason, sourceID: sourceID, in: situation)
            }
            if let intent = fallback(
                for: reason,
                evaluation: evaluation,
                judged: judged,
                sawCapture: sawCapture,
                hint: hint,
                sourceID: sourceID,
                in: situation
            ) {
                return intent
            }
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

    private mutating func depart(
        to destination: InterestDestination,
        for reason: PlacementTravelReason,
        sourceID: String,
        in situation: PetSituation
    ) -> PlacementIntent {
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

    /// What to do when a reason to leave found no seat to leave for, which
    /// happens once seats under the cursor stop counting. A reason that says
    /// the spot is bad -- the caret, the user's text -- still has to be acted
    /// on, so the pet steps aside from the cursor instead. A walk the cursor
    /// blocked ends where the pet stands if that spot is fine, and steps aside
    /// if it is not. Anything else falls through to holding, as before.
    private mutating func fallback(
        for reason: PlacementTravelReason,
        evaluation: SeatEvaluation?,
        judged: WorldPoint,
        sawCapture: Bool,
        hint: LocationHint,
        sourceID: String,
        in situation: PetSituation
    ) -> PlacementIntent? {
        let spotIsBad: Bool
        switch reason {
        case .coveringCaret, .coveringWork:
            spotIsBad = true
        case .seatUnderPointer:
            // The evaluation judged the seat being walked to. Whether the walk
            // may end here is a question about where the pet stands. Watching
            // the region is not asked: watching from a distance until the
            // cursor moves on is still watching.
            let standing = planner.evaluateSeat(
                at: situation.position,
                for: hint,
                in: situation.world,
                currentPosition: situation.position,
                pointerPosition: situation.pointerPosition,
                pointerClearance: situation.pointerClearance,
                objectSize: situation.objectSize
            )
            spotIsBad = standing.map {
                $0.coversCaret || ($0.emptiness ?? 1) < configuration.holdEmptiness
            } ?? false
        case .newActivity, .plannedBlind, .followedFocus:
            return nil
        }
        if spotIsBad,
           let destination = planner.stepAside(
            for: hint,
            in: situation.world,
            currentPosition: situation.position,
            pointerPosition: situation.pointerPosition,
            pointerClearance: situation.pointerClearance,
            objectSize: situation.objectSize
           ),
           accepts(destination, over: evaluation, judged: judged, for: reason, hint: hint, in: situation) {
            return depart(to: destination, for: reason, sourceID: sourceID, in: situation)
        }
        if reason == .seatUnderPointer {
            // The walk cannot finish and there is nowhere better: it ends here.
            return settle(sourceID: sourceID, sawCapture: sawCapture, at: situation.timestamp)
        }
        return nil
    }

    /// Priorities 3 through 7, in order. Above them is only ownership, below
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
        if travel != nil, evaluation.pointerBlocked { return .seatUnderPointer }
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
            let replacement = planner.evaluateSeat(
                at: destination.point,
                for: hint,
                in: situation.world,
                currentPosition: situation.position,
                pointerPosition: situation.pointerPosition,
                pointerClearance: situation.pointerClearance,
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
        case .coveringCaret, .seatUnderPointer, .plannedBlind, .followedFocus:
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

        // `parkedSince` deliberately survives this. The decision is only worth
        // making if it is still here when it can be acted on, and `decide`
        // throws away every answer while the pointer owns the pet -- so
        // clearing the dwell here meant a cursor drifting past at the wrong
        // moment restarted the wait, over and over, and the pet kept the
        // paragraph it was supposed to be leaving. Walking clears it instead,
        // at the top of this function, which is the point at which the answer
        // has actually been used.
        return .escape(escape)
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
