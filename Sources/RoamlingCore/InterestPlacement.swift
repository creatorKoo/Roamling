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

/// MVP 1 placement uses a coarse window region only. It deliberately favors
/// the bottom edge and positions just outside the window when space exists;
/// caret/control-aware placement remains an Accessibility milestone.
public enum BasicInterestPositionPlanner {
    public static func destination(
        for hint: LocationHint,
        in world: DesktopWorldSnapshot,
        currentPosition: WorldPoint,
        pointerPosition: WorldPoint?,
        objectSize: WorldSize
    ) -> InterestDestination? {
        guard hint.confidence > 0,
              let region = hint.approximateRegion,
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
            return InterestDestination(
                point: point,
                displayID: display.id,
                score: hint.confidence * 30
                    + outsideBonus
                    + bottomEdgeScore
                    - pointerPenalty
                    - travelPenalty
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
