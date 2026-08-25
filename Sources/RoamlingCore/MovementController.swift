// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

import Foundation

public struct MovementConfiguration: Equatable, Sendable {
    public var maximumSpeed: Double
    public var acceleration: Double
    public var deceleration: Double
    public var arrivalRadius: Double

    public init(
        maximumSpeed: Double = 48,
        acceleration: Double = 110,
        deceleration: Double = 130,
        arrivalRadius: Double = 1.5
    ) {
        self.maximumSpeed = max(0, maximumSpeed)
        self.acceleration = max(0, acceleration)
        self.deceleration = max(0, deceleration)
        self.arrivalRadius = max(0.1, arrivalRadius)
    }
}

public struct MovementUpdate: Equatable, Sendable {
    public let position: WorldPoint
    public let velocity: WorldVector
    public let reachedDestination: Bool
}

public struct MovementController: Sendable {
    public private(set) var position: WorldPoint
    public private(set) var velocity: WorldVector
    public var configuration: MovementConfiguration

    private var waypoints: [WorldPoint] = []
    private var waypointIndex = 0

    public init(
        position: WorldPoint,
        velocity: WorldVector = .zero,
        configuration: MovementConfiguration = MovementConfiguration()
    ) {
        self.position = position
        self.velocity = velocity
        self.configuration = configuration
    }

    public var hasRoute: Bool { waypointIndex < waypoints.count }
    public var destination: WorldPoint? { waypoints.last }
    public var currentWaypoint: WorldPoint? {
        guard hasRoute else { return nil }
        return waypoints[waypointIndex]
    }

    public mutating func setRoute(_ newWaypoints: [WorldPoint]) {
        waypoints = newWaypoints
        waypointIndex = 0
        skipReachedWaypoints()
    }

    public mutating func cancelRoute(stop: Bool = false) {
        waypoints.removeAll(keepingCapacity: true)
        waypointIndex = 0
        if stop { velocity = .zero }
    }

    public mutating func teleport(to point: WorldPoint, stop: Bool = true) {
        position = point
        if stop { velocity = .zero }
    }

    public mutating func setVelocity(_ newVelocity: WorldVector) {
        velocity = newVelocity.limited(to: configuration.maximumSpeed)
    }

    @discardableResult
    public mutating func update(deltaTime rawDeltaTime: TimeInterval) -> MovementUpdate {
        let deltaTime = rawDeltaTime.clamped(to: 0...0.1)
        guard deltaTime > 0, let target = currentWaypoint else {
            velocity = velocity.moved(toward: .zero, maximumDelta: configuration.deceleration * deltaTime)
            return MovementUpdate(position: position, velocity: velocity, reachedDestination: !hasRoute)
        }

        let offset = target - position
        let distance = offset.length
        if distance <= configuration.arrivalRadius {
            position = target
            waypointIndex += 1
            skipReachedWaypoints()
            if !hasRoute { velocity = .zero }
            return MovementUpdate(position: position, velocity: velocity, reachedDestination: !hasRoute)
        }

        let stoppingSpeed = sqrt(max(0, 2 * configuration.deceleration * distance))
        let desiredSpeed = min(configuration.maximumSpeed, stoppingSpeed)
        let desiredVelocity = offset.normalized * desiredSpeed
        velocity = velocity.moved(
            toward: desiredVelocity,
            maximumDelta: configuration.acceleration * deltaTime
        )

        let previousOffset = target - position
        let proposed = position + velocity * deltaTime
        let remainingOffset = target - proposed
        if previousOffset.dot(remainingOffset) <= 0 {
            position = target
            waypointIndex += 1
            skipReachedWaypoints()
            if !hasRoute { velocity = .zero }
        } else {
            position = proposed
        }

        return MovementUpdate(position: position, velocity: velocity, reachedDestination: !hasRoute)
    }

    private mutating func skipReachedWaypoints() {
        while waypointIndex < waypoints.count,
              position.distance(to: waypoints[waypointIndex]) <= configuration.arrivalRadius {
            position = waypoints[waypointIndex]
            waypointIndex += 1
        }
    }
}
