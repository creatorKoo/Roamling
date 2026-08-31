// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

import Foundation

public enum PointerProximity: String, Codable, Hashable, Sendable {
    case far
    case watching
    case slowEvade
    case fastEvade
    case catchable
}

public struct PointerInteractionConfiguration: Equatable, Sendable {
    public var awarenessDistance: Double
    public var slowEvadeDistance: Double
    public var fastEvadeDistance: Double
    public var catchDistance: Double
    public var slowEvadeSpeed: Double
    public var fastEvadeSpeed: Double
    public var catchPointerSpeed: Double
    public var catchClosingSpeed: Double

    public init(
        awarenessDistance: Double = 170,
        slowEvadeDistance: Double = 100,
        fastEvadeDistance: Double = 50,
        catchDistance: Double = 74,
        slowEvadeSpeed: Double = 74,
        fastEvadeSpeed: Double = 138,
        catchPointerSpeed: Double = 380,
        catchClosingSpeed: Double = 182
    ) {
        self.awarenessDistance = awarenessDistance
        self.slowEvadeDistance = min(slowEvadeDistance, awarenessDistance)
        self.fastEvadeDistance = min(fastEvadeDistance, slowEvadeDistance)
        // A fast approach should arm interaction before the pointer reaches
        // the sprite. It may therefore be wider than the fast-evade radius.
        self.catchDistance = catchDistance.clamped(to: 0...awarenessDistance)
        self.slowEvadeSpeed = max(0, slowEvadeSpeed)
        self.fastEvadeSpeed = max(slowEvadeSpeed, fastEvadeSpeed)
        self.catchPointerSpeed = max(0, catchPointerSpeed)
        self.catchClosingSpeed = max(0, catchClosingSpeed)
    }
}

public extension PointerInteractionConfiguration {
    /// Unhurried at the edge of awareness, twice as quick within catching range.
    ///
    /// Tied to the same two radii the rest of pointer behaviour uses, so moving
    /// one of them in the tuning panel moves this with it rather than leaving a
    /// second, invisible idea of "close".
    func attentionRate(atDistance distance: Double) -> Double {
        let span = awarenessDistance - catchDistance
        guard span > 0 else { return 1 }
        let closeness = ((awarenessDistance - distance) / span).clamped(to: 0...1)
        return 1 + closeness
    }
}

public struct PointerKinematics: Equatable, Sendable {
    public let velocity: WorldVector
    public let speed: Double
    public let distanceToPet: Double
    public let closingSpeed: Double
}

public struct PointerDecision: Equatable, Sendable {
    public let proximity: PointerProximity
    public let kinematics: PointerKinematics
    public let escapeVelocity: WorldVector
    public let lookDirectionDegrees: Double?

    /// How fast to play the watching animation, as a multiple of its own speed.
    ///
    /// Proximity is three coarse bands, but the distance behind them is
    /// continuous, and a cat's tail is the part of it that tracks interest
    /// continuously too. Driving the flick's speed from the distance spends that
    /// resolution instead of throwing it away: the pet quickens as the pointer
    /// closes rather than snapping between two states at a threshold.
    public let attentionRate: Double

    public var shouldArmCatch: Bool { proximity == .catchable }
}

public struct PointerInteractionModel: Sendable {
    public var configuration: PointerInteractionConfiguration

    private var previousPointer: WorldPoint?
    private var previousDistance: Double?
    private var previousTimestamp: TimeInterval?

    public init(configuration: PointerInteractionConfiguration = .init()) {
        self.configuration = configuration
    }

    public mutating func reset() {
        previousPointer = nil
        previousDistance = nil
        previousTimestamp = nil
    }

    public mutating func evaluate(
        pointer: WorldPoint,
        pet: WorldPoint,
        timestamp: TimeInterval
    ) -> PointerDecision {
        let distance = pointer.distance(to: pet)
        let rawDelta = previousTimestamp.map { timestamp - $0 } ?? 0
        let deltaTime = rawDelta > 0 ? min(rawDelta, 0.25) : 0
        let pointerVelocity: WorldVector
        if let previousPointer, deltaTime > 0 {
            pointerVelocity = (pointer - previousPointer) / deltaTime
        } else {
            pointerVelocity = .zero
        }
        let closingSpeed: Double
        if let previousDistance, deltaTime > 0 {
            closingSpeed = max(0, (previousDistance - distance) / deltaTime)
        } else {
            closingSpeed = 0
        }

        previousPointer = pointer
        previousDistance = distance
        previousTimestamp = timestamp

        let kinematics = PointerKinematics(
            velocity: pointerVelocity,
            speed: pointerVelocity.length,
            distanceToPet: distance,
            closingSpeed: closingSpeed
        )

        let proximity: PointerProximity
        if distance <= configuration.catchDistance,
           kinematics.speed >= configuration.catchPointerSpeed,
           closingSpeed >= configuration.catchClosingSpeed {
            proximity = .catchable
        } else if distance <= configuration.fastEvadeDistance {
            proximity = .fastEvade
        } else if distance <= configuration.slowEvadeDistance {
            proximity = .slowEvade
        } else if distance <= configuration.awarenessDistance {
            proximity = .watching
        } else {
            proximity = .far
        }

        var away = (pet - pointer).normalized
        if away.length < 0.001 {
            away = (-pointerVelocity).normalized
        }
        if away.length < 0.001 { away = WorldVector(dx: 1, dy: 0) }

        let escapeSpeed: Double
        switch proximity {
        case .slowEvade:
            escapeSpeed = configuration.slowEvadeSpeed
        case .fastEvade:
            escapeSpeed = configuration.fastEvadeSpeed
        default:
            escapeSpeed = 0
        }

        return PointerDecision(
            proximity: proximity,
            kinematics: kinematics,
            escapeVelocity: away * escapeSpeed,
            lookDirectionDegrees: Self.lookDirectionDegrees(from: pet, to: pointer),
            attentionRate: configuration.attentionRate(atDistance: distance)
        )
    }

    /// Codex v2 convention: 0° up, 90° screen-right, 180° down.
    public static func lookDirectionDegrees(from pet: WorldPoint, to pointer: WorldPoint) -> Double? {
        let vector = pointer - pet
        guard vector.length > 0.001 else { return nil }
        let radians = atan2(vector.dx, -vector.dy)
        let degrees = radians * 180 / .pi
        return degrees >= 0 ? degrees : degrees + 360
    }
}
