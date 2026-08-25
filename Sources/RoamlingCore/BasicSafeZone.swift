// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

import Foundation

public struct RestDestination: Equatable, Sendable {
    public let point: WorldPoint
    public let displayID: String
    public let reason: String
    public let score: Double

    public init(point: WorldPoint, displayID: String, reason: String, score: Double) {
        self.point = point
        self.displayID = displayID
        self.reason = reason
        self.score = score
    }
}

/// Permission-free sleep placement based only on platform-neutral display
/// snapshots. `visibleFrame` already excludes the macOS menu bar and Dock.
public enum BasicSafeZonePlanner {
    public static func safeZones(in world: DesktopWorldSnapshot) -> [SafeZone] {
        world.displays.flatMap { safeZones(on: $0) }
    }

    public static func destination(
        in world: DesktopWorldSnapshot,
        currentPosition: WorldPoint,
        pointerPosition: WorldPoint?,
        objectSize: WorldSize
    ) -> RestDestination? {
        let zones = world.safeZones.isEmpty ? safeZones(in: world) : world.safeZones
        let currentDisplay = world.display(containing: currentPosition)
            ?? world.nearestDisplay(to: currentPosition)

        let candidates: [RestDestination] = zones.compactMap { zone in
            guard let display = world.display(containing: zone.frame.center)
                ?? world.nearestDisplay(to: zone.frame.center) else { return nil }
            let point = display.visibleFrame.clampedCenter(zone.frame.center, objectSize: objectSize)
            let sameDisplayBonus = display.id == currentDisplay?.id ? 38.0 : 0
            let travelPenalty = min(16, currentPosition.distance(to: point) / 180)
            let pointerPenalty: Double
            if let pointerPosition {
                pointerPenalty = max(0, 260 - pointerPosition.distance(to: point)) / 11
            } else {
                pointerPenalty = 0
            }
            return RestDestination(
                point: point,
                displayID: display.id,
                reason: zone.reason,
                score: zone.score + sameDisplayBonus - travelPenalty - pointerPenalty
            )
        }

        if let best = candidates.max(by: { lhs, rhs in
            if lhs.score == rhs.score {
                return currentPosition.distance(to: lhs.point)
                    > currentPosition.distance(to: rhs.point)
            }
            return lhs.score < rhs.score
        }) {
            return best
        }

        guard let display = currentDisplay else { return nil }
        let safe = display.visibleFrame.insetBy(
            dx: objectSize.width / 2 + 18,
            dy: objectSize.height / 2 + 14
        )
        return RestDestination(
            point: WorldPoint(x: safe.maxX, y: safe.maxY),
            displayID: display.id,
            reason: "display-corner-fallback",
            score: 0
        )
    }

    private static func safeZones(on display: DisplaySnapshot) -> [SafeZone] {
        let visible = display.visibleFrame
        guard !visible.isEmpty else { return [] }
        let bounds = visible.insetBy(dx: min(18, visible.size.width / 4), dy: min(16, visible.size.height / 4))
        let width = min(220, bounds.size.width)
        let height = min(160, bounds.size.height)
        guard width > 0, height > 0 else { return [] }

        let leftInset = max(0, visible.minX - display.frame.minX)
        let rightInset = max(0, display.frame.maxX - visible.maxX)
        let bottomInset = max(0, display.frame.maxY - visible.maxY)
        let dockEdge: DockEdge?
        let sideInsets: [(DockEdge, Double)] = [
            (.left, leftInset),
            (.right, rightInset),
            (.bottom, bottomInset)
        ]
        if let largest = sideInsets.max(by: { $0.1 < $1.1 }), largest.1 > 4 {
            dockEdge = largest.0
        } else {
            dockEdge = nil
        }

        let corners: [(String, DockEdge?, WorldRect)] = [
            (
                "top-left",
                .left,
                WorldRect(x: bounds.minX, y: bounds.minY, width: width, height: height)
            ),
            (
                "top-right",
                .right,
                WorldRect(x: bounds.maxX - width, y: bounds.minY, width: width, height: height)
            ),
            (
                "bottom-left",
                dockEdge == .bottom ? .bottom : .left,
                WorldRect(x: bounds.minX, y: bounds.maxY - height, width: width, height: height)
            ),
            (
                "bottom-right",
                dockEdge == .bottom ? .bottom : .right,
                WorldRect(x: bounds.maxX - width, y: bounds.maxY - height, width: width, height: height)
            )
        ]

        return corners.map { name, adjacentEdge, frame in
            let isDockAdjacent = dockEdge != nil && adjacentEdge == dockEdge
            return SafeZone(
                frame: frame,
                score: (name.hasPrefix("bottom") ? 38 : 34) + (isDockAdjacent ? 4 : 0),
                confidence: 0.72,
                reason: isDockAdjacent ? "dock-adjacent-\(name)" : "display-corner-\(name)"
            )
        }
    }

    private enum DockEdge {
        case left
        case right
        case bottom
    }
}
