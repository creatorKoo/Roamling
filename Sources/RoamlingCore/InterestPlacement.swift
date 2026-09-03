// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

import Foundation

public struct InterestDestination: Equatable, Sendable {
    public let point: WorldPoint
    public let displayID: String
    public let score: Double

    public init(point: WorldPoint, displayID: String, score: Double) {
        self.point = point
        self.displayID = displayID
        self.score = score
    }
}

/// How a seat looks right now, so a caller can decide to stay put.
///
/// Re-planning a seat on every agent event makes the pet twitch across the
/// screen while nothing about the seat got worse. `isHoldable` is the question
/// that deserves to be asked first: is the pet covering the user's work, and is
/// it still watching the window it was sent to.
public struct SeatEvaluation: Equatable, Sendable {
    public let point: WorldPoint
    public let displayID: String
    public let score: Double
    /// `nil` when no capture was available or the seat fell outside it, which
    /// reads as "cannot tell" rather than "busy".
    public let emptiness: Double?
    public let coversCaret: Bool
    /// False once the seat no longer belongs to the window it was planned for,
    /// which is how a focus change unsticks a held seat.
    public let watchesRegion: Bool

    public init(
        point: WorldPoint,
        displayID: String,
        score: Double,
        emptiness: Double?,
        coversCaret: Bool,
        watchesRegion: Bool
    ) {
        self.point = point
        self.displayID = displayID
        self.score = score
        self.emptiness = emptiness
        self.coversCaret = coversCaret
        self.watchesRegion = watchesRegion
    }

    public var isHoldable: Bool {
        watchesRegion
            && !coversCaret
            && (emptiness ?? 1) >= BasicInterestPositionPlanner.holdEmptiness
    }
}

/// Where the pet should sit to watch a window, and how good a given seat is.
///
/// A protocol so the decision can be answered by the Rust core while the port
/// is under way -- the director asks, and does not care which side answers.
public protocol InterestPlacing: Sendable {
    func destination(
        for hint: LocationHint,
        in world: DesktopWorldSnapshot,
        currentPosition: WorldPoint,
        pointerPosition: WorldPoint?,
        objectSize: WorldSize
    ) -> InterestDestination?

    func evaluateSeat(
        at point: WorldPoint,
        for hint: LocationHint,
        in world: DesktopWorldSnapshot,
        currentPosition: WorldPoint,
        pointerPosition: WorldPoint?,
        objectSize: WorldSize
    ) -> SeatEvaluation?
}

/// The Swift implementation, kept as the control the port is measured against.
public struct SwiftInterestPlanner: InterestPlacing {
    public init() {}

    public func destination(
        for hint: LocationHint,
        in world: DesktopWorldSnapshot,
        currentPosition: WorldPoint,
        pointerPosition: WorldPoint?,
        objectSize: WorldSize
    ) -> InterestDestination? {
        BasicInterestPositionPlanner.destination(
            for: hint, in: world, currentPosition: currentPosition,
            pointerPosition: pointerPosition, objectSize: objectSize
        )
    }

    public func evaluateSeat(
        at point: WorldPoint,
        for hint: LocationHint,
        in world: DesktopWorldSnapshot,
        currentPosition: WorldPoint,
        pointerPosition: WorldPoint?,
        objectSize: WorldSize
    ) -> SeatEvaluation? {
        BasicInterestPositionPlanner.evaluateSeat(
            at: point, for: hint, in: world, currentPosition: currentPosition,
            pointerPosition: pointerPosition, objectSize: objectSize
        )
    }
}

/// Placement favors the bottom edge and sits just outside the window when
/// space exists. Without Accessibility it sees only a coarse window region.
/// With `DesktopWorldSnapshot.focus` it prefers the focused window's frame,
/// leans toward the caret, and refuses to sit on top of it. With a capture it
/// also sweeps the lower window for a gap that is actually empty.
public enum BasicInterestPositionPlanner {
    /// Extra margin kept between the pet and the insertion point.
    private static let caretClearance = 12.0
    /// A held seat may not look busier than this. The luminance field is
    /// downsampled, so this answers "is the pet parked on content", never "is a
    /// glyph under a paw".
    public static let holdEmptiness = 0.55
    /// A seat only counts as verified empty above this. Overlapping the focused
    /// control is forgiven here and nowhere else, so it sits well clear of the
    /// hold threshold rather than next to it.
    private static let clearEmptiness = 0.85
    /// How far a seat may sit outside its window and still count as watching it.
    ///
    /// It has to reach at least as far as the seats this planner places beside
    /// the window, or a seat it just chose reads as no longer watching and the
    /// caller moves the pet again on the very next review, forever. Those
    /// candidates sit `halfWidth + 14` outside the edge, so the margin is
    /// derived from the pet rather than fixed. A full-screen window hid this:
    /// its outside seats get clamped back onto the display.
    private static let holdRegionMargin = 48.0
    /// The caret advances to the right as the user types, so a seat on its line
    /// and ahead of it will be written into within seconds. It outweighs the
    /// proximity bonus on purpose — otherwise wanting to sit near the caret is
    /// exactly what walks the pet into the next sentence — and stays below the
    /// penalty for covering the caret outright.
    private static let caretAdvancePenalty = 60.0

    /// Everything the candidate scoring needs, resolved once per call.
    private struct Plan {
        let region: WorldRect
        let display: DisplaySnapshot
        let safe: WorldRect
        let focus: FocusSnapshot?
        let field: LuminanceField?
        let confidence: Double
        let bottomY: Double
        let objectSize: WorldSize
        let currentPosition: WorldPoint
        let pointerPosition: WorldPoint?

        var halfWidth: Double { objectSize.width / 2 }
        var halfHeight: Double { objectSize.height / 2 }

        func petFrame(at point: WorldPoint) -> WorldRect {
            WorldRect(
                x: point.x - halfWidth,
                y: point.y - halfHeight,
                width: objectSize.width,
                height: objectSize.height
            )
        }
    }

    public static func destination(
        for hint: LocationHint,
        in world: DesktopWorldSnapshot,
        currentPosition: WorldPoint,
        pointerPosition: WorldPoint?,
        objectSize: WorldSize
    ) -> InterestDestination? {
        guard let plan = makePlan(
            for: hint,
            in: world,
            currentPosition: currentPosition,
            pointerPosition: pointerPosition,
            objectSize: objectSize
        ) else { return nil }

        let best = candidates(in: plan)
            .map { evaluate($0.point, intendedOutside: $0.outside, in: plan) }
            .max { lhs, rhs in
                if lhs.score == rhs.score {
                    return currentPosition.distance(to: lhs.point)
                        > currentPosition.distance(to: rhs.point)
                }
                return lhs.score < rhs.score
            }
        guard let best else { return nil }
        return InterestDestination(
            point: best.point,
            displayID: best.displayID,
            score: best.score
        )
    }

    /// Scores an arbitrary point with the same formula the planner uses, so a
    /// seat the pet already occupies can be compared against a fresh candidate
    /// without either side drifting to its own scale.
    public static func evaluateSeat(
        at point: WorldPoint,
        for hint: LocationHint,
        in world: DesktopWorldSnapshot,
        currentPosition: WorldPoint,
        pointerPosition: WorldPoint?,
        objectSize: WorldSize
    ) -> SeatEvaluation? {
        guard let plan = makePlan(
            for: hint,
            in: world,
            currentPosition: currentPosition,
            pointerPosition: pointerPosition,
            objectSize: objectSize
        ) else { return nil }
        return evaluate(point, intendedOutside: false, in: plan)
    }

    private static func makePlan(
        for hint: LocationHint,
        in world: DesktopWorldSnapshot,
        currentPosition: WorldPoint,
        pointerPosition: WorldPoint?,
        objectSize: WorldSize
    ) -> Plan? {
        let focus = world.focus.flatMap { $0.confidence > 0 ? $0 : nil }
        // The focused window frame is exact where the coarse hint only knows
        // the frontmost process, so it wins when accessibility supplied one.
        let focusedWindowFrame = focus?.windowFrame
        let confidence = max(hint.confidence, focus?.confidence ?? 0)
        guard confidence > 0,
              let region = focusedWindowFrame ?? hint.approximateRegion,
              let display = world.display(containing: region.center)
                ?? world.nearestDisplay(to: region.center) else { return nil }

        let safe = display.visibleFrame.insetBy(
            dx: objectSize.width / 2 + 10,
            dy: objectSize.height / 2 + 10
        )
        guard !safe.isEmpty else { return nil }

        return Plan(
            region: region,
            display: display,
            safe: safe,
            focus: focus,
            field: world.luminance,
            confidence: confidence,
            bottomY: region.maxY - objectSize.height / 2 - 14,
            objectSize: objectSize,
            currentPosition: currentPosition,
            pointerPosition: pointerPosition
        )
    }

    /// Candidate seats in the order they were proposed, each carrying whether
    /// it was meant to land outside the window.
    ///
    /// Ordered, not a dictionary. Two mirrored seats either side of a window
    /// score identically and sit the same distance away, and `max(by:)` keeps
    /// whichever it met first -- which for a dictionary is whatever the hash
    /// seed decided that launch. The pet sat left of the window 24 runs out of
    /// 40 and right the other 16, on the same desk with the same window.
    private static func candidates(in plan: Plan) -> [(point: WorldPoint, outside: Bool)] {
        let region = plan.region
        var raw: [(WorldPoint, Bool)] = [
            (WorldPoint(x: region.minX - plan.halfWidth - 14, y: plan.bottomY), true),
            (WorldPoint(x: region.maxX + plan.halfWidth + 14, y: plan.bottomY), true),
            (WorldPoint(x: region.minX + plan.halfWidth + 18, y: plan.bottomY), false),
            (WorldPoint(x: region.maxX - plan.halfWidth - 18, y: plan.bottomY), false)
        ]

        // Four seats at the two edges give an emptiness score nothing to choose
        // between, and a single bottom line only ever finds the busiest part of
        // a terminal — the prompt. When a capture exists, sweep the window so
        // the score can find the gap that is actually there, wherever it is.
        // `bottomEdgeScore` still leans low, so the middle of a document only
        // wins when the capture says the lower window has no room left.
        if plan.field != nil {
            let innerLeft = region.minX + plan.halfWidth + 18
            let innerRight = region.maxX - plan.halfWidth - 18
            let topLimit = region.minY + plan.halfHeight + 18
            if innerRight > innerLeft {
                let rowStep = plan.objectSize.height * 1.15
                for row in 0..<6 {
                    let y = plan.bottomY - rowStep * Double(row)
                    guard y >= topLimit else { break }
                    let columns = 6
                    for step in 1..<columns {
                        let ratio = Double(step) / Double(columns)
                        let x = innerLeft + (innerRight - innerLeft) * ratio
                        raw.append((WorldPoint(x: x, y: y), false))
                    }
                    if row > 0 {
                        raw.append((WorldPoint(x: innerLeft, y: y), false))
                        raw.append((WorldPoint(x: innerRight, y: y), false))
                    }
                }
            }
        }

        // Clamping collapses candidates onto each other near a screen edge, so
        // duplicates still merge -- first proposal keeps its place, and being
        // wanted outside by any of them wins.
        var order: [WorldPoint] = []
        var outsideByPoint: [WorldPoint: Bool] = [:]
        for (point, outside) in raw {
            let clamped = plan.safe.closestPoint(to: point)
            if outsideByPoint[clamped] == nil { order.append(clamped) }
            outsideByPoint[clamped] = (outsideByPoint[clamped] ?? false) || outside
        }
        return order.map { (point: $0, outside: outsideByPoint[$0] ?? false) }
    }

    private static func evaluate(
        _ point: WorldPoint,
        intendedOutside: Bool,
        in plan: Plan
    ) -> SeatEvaluation {
        let frame = plan.petFrame(at: point)
        let isOutside = !plan.region.contains(point)
        // Sitting beside the window is safe but it is also how the pet ends up
        // parked at a screen edge, far from the work, whenever the window is
        // large. It stays a tiebreaker, not an argument that outranks a seat
        // the capture confirmed is empty.
        let outsideBonus = intendedOutside && isOutside ? 12.0 : 0
        let pointerPenalty = plan.pointerPosition.map {
            max(0, 220 - $0.distance(to: point)) / 8
        } ?? 0
        let travelPenalty = min(18, plan.currentPosition.distance(to: point) / 220)
        let bottomDistance = abs(point.y - plan.bottomY)
        let bottomEdgeScore = max(0, 12 - bottomDistance / 24)

        let emptiness = plan.field.flatMap { VisualEmptiness.score(of: frame, in: $0) }

        // Sitting near the caret is the point of this gate. Sitting on top of
        // it is the one thing the gate must never do, so the occlusion penalty
        // outweighs every bonus a candidate can earn.
        var caretAffinity = 0.0
        var occlusionPenalty = 0.0
        var advancePenalty = 0.0
        var coversCaret = false
        if let focus = plan.focus {
            if let caret = focus.caretFrame {
                // Falling off over a hundred points rather than a thousand is
                // what makes this a reason to pick a seat. The old curve was so
                // flat that a wallpaper seat at the screen edge outscored an
                // empty spot right beside the work.
                caretAffinity = max(0, 40 - caret.distance(to: point) / 12)
                if frame.intersects(caret, tolerance: Self.caretClearance) {
                    coversCaret = true
                    occlusionPenalty += 120
                }
                // The caret marches right through everything on its line, so a
                // seat ahead of it is empty now and buried in a sentence.
                let sharesLine = frame.maxY >= caret.minY && caret.maxY >= frame.minY
                if sharesLine, frame.maxX > caret.minX {
                    advancePenalty = Self.caretAdvancePenalty
                }
            }
            // Overlapping the focused control is a guess about content, and a
            // capture answers it directly. The forgiveness is all-or-nothing on
            // purpose: a partial discount let the pet inch onto body text that
            // merely scored middling.
            if let element = focus.focusedElementFrame, frame.intersects(element) {
                occlusionPenalty += (emptiness ?? 0) >= Self.clearEmptiness ? 0 : 40
            }
        }

        // Emptiness only ranks seats that already passed the caret and pointer
        // checks, so it can move the pet along the sweep but never onto
        // something it must avoid.
        let visualEmptyScore = (emptiness ?? 0) * 34

        let watchMargin = max(Self.holdRegionMargin, plan.halfWidth + 24)
        let watched = WorldRect(
            x: plan.region.minX - watchMargin,
            y: plan.region.minY - watchMargin,
            width: plan.region.size.width + watchMargin * 2,
            height: plan.region.size.height + watchMargin * 2
        )

        return SeatEvaluation(
            point: point,
            displayID: plan.display.id,
            score: plan.confidence * 30
                + outsideBonus
                + bottomEdgeScore
                + caretAffinity
                + visualEmptyScore
                - pointerPenalty
                - travelPenalty
                - occlusionPenalty
                - advancePenalty,
            emptiness: emptiness,
            coversCaret: coversCaret,
            watchesRegion: watched.contains(point)
        )
    }
}
