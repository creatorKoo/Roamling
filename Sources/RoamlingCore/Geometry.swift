// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

import Foundation

public struct WorldPoint: Codable, Hashable, Sendable {
    public var x: Double
    public var y: Double

    public init(x: Double, y: Double) {
        self.x = x
        self.y = y
    }

    public static let zero = WorldPoint(x: 0, y: 0)

    public func distance(to other: WorldPoint) -> Double {
        hypot(other.x - x, other.y - y)
    }

    public static func + (lhs: WorldPoint, rhs: WorldVector) -> WorldPoint {
        WorldPoint(x: lhs.x + rhs.dx, y: lhs.y + rhs.dy)
    }

    public static func - (lhs: WorldPoint, rhs: WorldVector) -> WorldPoint {
        WorldPoint(x: lhs.x - rhs.dx, y: lhs.y - rhs.dy)
    }

    public static func - (lhs: WorldPoint, rhs: WorldPoint) -> WorldVector {
        WorldVector(dx: lhs.x - rhs.x, dy: lhs.y - rhs.y)
    }
}

public struct WorldVector: Codable, Hashable, Sendable {
    public var dx: Double
    public var dy: Double

    public init(dx: Double, dy: Double) {
        self.dx = dx
        self.dy = dy
    }

    public static let zero = WorldVector(dx: 0, dy: 0)

    public var length: Double { hypot(dx, dy) }

    public var normalized: WorldVector {
        let magnitude = length
        guard magnitude > 0.000_001 else { return .zero }
        return self / magnitude
    }

    public func dot(_ other: WorldVector) -> Double {
        dx * other.dx + dy * other.dy
    }

    public func limited(to maximum: Double) -> WorldVector {
        guard maximum >= 0, length > maximum, length > 0 else { return self }
        return normalized * maximum
    }

    public func moved(toward target: WorldVector, maximumDelta: Double) -> WorldVector {
        let delta = target - self
        guard delta.length > maximumDelta, maximumDelta >= 0 else { return target }
        return self + delta.normalized * maximumDelta
    }

    public static func + (lhs: WorldVector, rhs: WorldVector) -> WorldVector {
        WorldVector(dx: lhs.dx + rhs.dx, dy: lhs.dy + rhs.dy)
    }

    public static func - (lhs: WorldVector, rhs: WorldVector) -> WorldVector {
        WorldVector(dx: lhs.dx - rhs.dx, dy: lhs.dy - rhs.dy)
    }

    public static prefix func - (value: WorldVector) -> WorldVector {
        WorldVector(dx: -value.dx, dy: -value.dy)
    }

    public static func * (lhs: WorldVector, rhs: Double) -> WorldVector {
        WorldVector(dx: lhs.dx * rhs, dy: lhs.dy * rhs)
    }

    public static func / (lhs: WorldVector, rhs: Double) -> WorldVector {
        guard rhs != 0 else { return .zero }
        return WorldVector(dx: lhs.dx / rhs, dy: lhs.dy / rhs)
    }
}

public struct WorldSize: Codable, Hashable, Sendable {
    public var width: Double
    public var height: Double

    public init(width: Double, height: Double) {
        self.width = width
        self.height = height
    }

    public static let zero = WorldSize(width: 0, height: 0)
}

public struct WorldRect: Codable, Hashable, Sendable {
    public var origin: WorldPoint
    public var size: WorldSize

    public init(x: Double, y: Double, width: Double, height: Double) {
        origin = WorldPoint(x: x, y: y)
        size = WorldSize(width: max(0, width), height: max(0, height))
    }

    public init(origin: WorldPoint, size: WorldSize) {
        self.init(x: origin.x, y: origin.y, width: size.width, height: size.height)
    }

    public var minX: Double { origin.x }
    public var minY: Double { origin.y }
    public var maxX: Double { origin.x + size.width }
    public var maxY: Double { origin.y + size.height }
    public var midX: Double { origin.x + size.width / 2 }
    public var midY: Double { origin.y + size.height / 2 }
    public var center: WorldPoint { WorldPoint(x: midX, y: midY) }
    public var isEmpty: Bool { size.width <= 0 || size.height <= 0 }

    public func contains(_ point: WorldPoint) -> Bool {
        point.x >= minX && point.x <= maxX && point.y >= minY && point.y <= maxY
    }

    public func intersects(_ other: WorldRect, tolerance: Double = 0) -> Bool {
        maxX + tolerance >= other.minX && other.maxX + tolerance >= minX
            && maxY + tolerance >= other.minY && other.maxY + tolerance >= minY
    }

    public func insetBy(dx: Double, dy: Double) -> WorldRect {
        let insetX = min(max(0, dx), size.width / 2)
        let insetY = min(max(0, dy), size.height / 2)
        return WorldRect(
            x: minX + insetX,
            y: minY + insetY,
            width: size.width - insetX * 2,
            height: size.height - insetY * 2
        )
    }

    public func closestPoint(to point: WorldPoint) -> WorldPoint {
        WorldPoint(
            x: min(max(point.x, minX), maxX),
            y: min(max(point.y, minY), maxY)
        )
    }

    public func distance(to point: WorldPoint) -> Double {
        closestPoint(to: point).distance(to: point)
    }

    public func clampedCenter(_ point: WorldPoint, objectSize: WorldSize) -> WorldPoint {
        insetBy(dx: objectSize.width / 2, dy: objectSize.height / 2).closestPoint(to: point)
    }

    public func union(_ other: WorldRect) -> WorldRect {
        WorldRect(
            x: min(minX, other.minX),
            y: min(minY, other.minY),
            width: max(maxX, other.maxX) - min(minX, other.minX),
            height: max(maxY, other.maxY) - min(minY, other.minY)
        )
    }
}

public extension Double {
    func clamped(to range: ClosedRange<Double>) -> Double {
        min(max(self, range.lowerBound), range.upperBound)
    }
}
