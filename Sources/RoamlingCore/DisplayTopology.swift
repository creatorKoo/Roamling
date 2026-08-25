// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

import Foundation

public struct DisplayPortal: Equatable, Sendable {
    public let fromDisplayID: String
    public let toDisplayID: String
    public let exit: WorldPoint
    public let entry: WorldPoint

    public var gap: Double { exit.distance(to: entry) }
}

public struct DisplayRoute: Equatable, Sendable {
    public let displayIDs: [String]
    public let waypoints: [WorldPoint]

    public init(displayIDs: [String], waypoints: [WorldPoint]) {
        self.displayIDs = displayIDs
        self.waypoints = waypoints
    }
}

public struct DisplayTopology: Sendable {
    public let displays: [DisplaySnapshot]

    public init(displays: [DisplaySnapshot]) {
        self.displays = displays
    }

    public func route(from start: WorldPoint, to destination: WorldPoint) -> DisplayRoute {
        guard !displays.isEmpty else {
            return DisplayRoute(displayIDs: [], waypoints: [destination])
        }

        let startIndex = displayIndex(containingOrNearestTo: start)
        let endIndex = displayIndex(containingOrNearestTo: destination)
        guard let startIndex, let endIndex else {
            return DisplayRoute(displayIDs: [], waypoints: [destination])
        }
        guard startIndex != endIndex else {
            return DisplayRoute(displayIDs: [displays[startIndex].id], waypoints: [destination])
        }

        let indices = shortestDisplayPath(from: startIndex, to: endIndex)
        var waypoints: [WorldPoint] = []

        for pair in zip(indices, indices.dropFirst()) {
            let portal = portal(from: displays[pair.0], to: displays[pair.1])
            appendIfDistinct(portal.exit, to: &waypoints)
            appendIfDistinct(portal.entry, to: &waypoints)
        }
        appendIfDistinct(destination, to: &waypoints)

        return DisplayRoute(
            displayIDs: indices.map { displays[$0].id },
            waypoints: waypoints
        )
    }

    public func portal(from: DisplaySnapshot, to: DisplaySnapshot) -> DisplayPortal {
        let a = from.frame
        let b = to.frame
        let overlapXMin = max(a.minX, b.minX)
        let overlapXMax = min(a.maxX, b.maxX)
        let overlapYMin = max(a.minY, b.minY)
        let overlapYMax = min(a.maxY, b.maxY)
        let hasXOverlap = overlapXMin <= overlapXMax
        let hasYOverlap = overlapYMin <= overlapYMax

        let exit: WorldPoint
        let entry: WorldPoint

        if hasYOverlap, a.maxX <= b.minX {
            let y = (overlapYMin + overlapYMax) / 2
            exit = WorldPoint(x: a.maxX, y: y)
            entry = WorldPoint(x: b.minX, y: y)
        } else if hasYOverlap, b.maxX <= a.minX {
            let y = (overlapYMin + overlapYMax) / 2
            exit = WorldPoint(x: a.minX, y: y)
            entry = WorldPoint(x: b.maxX, y: y)
        } else if hasXOverlap, a.maxY <= b.minY {
            let x = (overlapXMin + overlapXMax) / 2
            exit = WorldPoint(x: x, y: a.maxY)
            entry = WorldPoint(x: x, y: b.minY)
        } else if hasXOverlap, b.maxY <= a.minY {
            let x = (overlapXMin + overlapXMax) / 2
            exit = WorldPoint(x: x, y: a.minY)
            entry = WorldPoint(x: x, y: b.maxY)
        } else if hasXOverlap, hasYOverlap {
            let common = WorldPoint(
                x: (overlapXMin + overlapXMax) / 2,
                y: (overlapYMin + overlapYMax) / 2
            )
            exit = common
            entry = common
        } else {
            exit = a.closestPoint(to: b.center)
            entry = b.closestPoint(to: a.center)
        }

        return DisplayPortal(
            fromDisplayID: from.id,
            toDisplayID: to.id,
            exit: exit,
            entry: entry
        )
    }

    private func displayIndex(containingOrNearestTo point: WorldPoint) -> Int? {
        if let containing = displays.firstIndex(where: { $0.frame.contains(point) }) {
            return containing
        }
        return displays.indices.min {
            displays[$0].frame.distance(to: point) < displays[$1].frame.distance(to: point)
        }
    }

    /// Complete graph + Dijkstra. Touching displays have a tiny edge cost, so
    /// a chain of real seams wins over a direct jump across an intervening gap.
    private func shortestDisplayPath(from source: Int, to target: Int) -> [Int] {
        let count = displays.count
        var distance = Array(repeating: Double.infinity, count: count)
        var previous = Array<Int?>(repeating: nil, count: count)
        var unvisited = Set(displays.indices)
        distance[source] = 0

        while !unvisited.isEmpty {
            guard let current = unvisited.min(by: { distance[$0] < distance[$1] }) else { break }
            unvisited.remove(current)
            if current == target { break }
            if !distance[current].isFinite { break }

            for neighbor in unvisited where neighbor != current {
                let crossing = portal(from: displays[current], to: displays[neighbor])
                let weight = max(1, crossing.gap)
                let candidate = distance[current] + weight
                if candidate < distance[neighbor] {
                    distance[neighbor] = candidate
                    previous[neighbor] = current
                }
            }
        }

        var result = [target]
        var cursor = target
        while cursor != source, let parent = previous[cursor] {
            result.append(parent)
            cursor = parent
        }
        if result.last != source { return [source, target] }
        return result.reversed()
    }

    private func appendIfDistinct(_ point: WorldPoint, to points: inout [WorldPoint]) {
        if let last = points.last, last.distance(to: point) < 0.5 { return }
        points.append(point)
    }
}
