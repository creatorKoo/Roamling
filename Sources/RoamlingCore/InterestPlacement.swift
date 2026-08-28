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

/// Placement favors the bottom edge and sits just outside the window when
/// space exists. Without Accessibility it sees only a coarse window region.
/// With `DesktopWorldSnapshot.focus` it prefers the focused window's frame,
/// leans toward the caret, and refuses to sit on top of it.
public enum BasicInterestPositionPlanner {
    /// Extra margin kept between the pet and the insertion point.
    private static let caretClearance = 12.0
    public static func destination(
        for hint: LocationHint,
        in world: DesktopWorldSnapshot,
        currentPosition: WorldPoint,
        pointerPosition: WorldPoint?,
        objectSize: WorldSize
    ) -> InterestDestination? {
        let focus = world.focus.flatMap { $0.confidence > 0 ? $0 : nil }
        // The focused window frame is exact where the coarse hint only knows
        // the frontmost process, so it wins when accessibility supplied one.
        let focusedWindowFrame = focus?.windowID.flatMap { id in
            world.windows.first { $0.id == id }?.frame
        }
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

        let halfWidth = objectSize.width / 2
        let halfHeight = objectSize.height / 2
        let bottomY = region.maxY - halfHeight - 14
        let rawCandidates: [(WorldPoint, Bool)] = [
            (WorldPoint(x: region.minX - halfWidth - 14, y: bottomY), true),
            (WorldPoint(x: region.maxX + halfWidth + 14, y: bottomY), true),
            (WorldPoint(x: region.minX + halfWidth + 18, y: bottomY), false),
            (WorldPoint(x: region.maxX - halfWidth - 18, y: bottomY), false)
        ]

        var unique: [WorldPoint: Bool] = [:]
        for (point, outside) in rawCandidates {
            let clamped = safe.closestPoint(to: point)
            unique[clamped] = (unique[clamped] ?? false) || outside
        }

        let candidates = unique.map { point, intendedOutside -> InterestDestination in
            let isOutside = !region.contains(point)
            let outsideBonus = intendedOutside && isOutside ? 28.0 : 0
            let pointerPenalty = pointerPosition.map {
                max(0, 220 - $0.distance(to: point)) / 8
            } ?? 0
            let travelPenalty = min(18, currentPosition.distance(to: point) / 220)
            let bottomDistance = abs(point.y - bottomY)
            let bottomEdgeScore = max(0, 12 - bottomDistance / 24)

            // Sitting near the caret is the point of this gate. Sitting on top
            // of it is the one thing the gate must never do, so the occlusion
            // penalty outweighs every bonus a candidate can earn.
            var caretAffinity = 0.0
            var occlusionPenalty = 0.0
            if let focus {
                let petFrame = WorldRect(
                    x: point.x - halfWidth,
                    y: point.y - halfHeight,
                    width: objectSize.width,
                    height: objectSize.height
                )
                if let caret = focus.caretFrame {
                    caretAffinity = max(0, 24 - caret.distance(to: point) / 40)
                    if petFrame.intersects(caret, tolerance: Self.caretClearance) {
                        occlusionPenalty += 120
                    }
                }
                if let element = focus.focusedElementFrame, petFrame.intersects(element) {
                    occlusionPenalty += 40
                }
            }

            return InterestDestination(
                point: point,
                displayID: display.id,
                score: confidence * 30
                    + outsideBonus
                    + bottomEdgeScore
                    + caretAffinity
                    - pointerPenalty
                    - travelPenalty
                    - occlusionPenalty
            )
        }

        return candidates.max { lhs, rhs in
            if lhs.score == rhs.score {
                return currentPosition.distance(to: lhs.point)
                    > currentPosition.distance(to: rhs.point)
            }
            return lhs.score < rhs.score
        }
    }
}
