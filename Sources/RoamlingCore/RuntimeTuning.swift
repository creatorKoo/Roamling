// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

import Foundation

/// The intentionally small set of live-tunable values used to validate
/// MVP 0/0.5 feel. Later milestone settings should not be added here until
/// their behavior is implemented.
public struct RuntimeTuning: Codable, Equatable, Sendable {
    public var walkingSpeed: Double
    public var wanderPause: Double
    public var crossDisplayWanderChance: Double
    public var pointerAwarenessDistance: Double
    public var catchArmDistance: Double
    public var catchApproachSpeed: Double
    public var catchWindow: Double
    public var hitRegionScale: Double
    public var gaitCadence: Double

    public init(
        walkingSpeed: Double = 40,
        wanderPause: Double = 12,
        crossDisplayWanderChance: Double = 0.46,
        pointerAwarenessDistance: Double = 170,
        catchArmDistance: Double = 74,
        catchApproachSpeed: Double = 380,
        catchWindow: Double = 0.35,
        hitRegionScale: Double = 1.12,
        gaitCadence: Double = 1
    ) {
        self.walkingSpeed = walkingSpeed.clamped(to: 20...160)
        self.wanderPause = wanderPause.clamped(to: 2...40)
        self.crossDisplayWanderChance = crossDisplayWanderChance.clamped(to: 0...1)
        self.pointerAwarenessDistance = pointerAwarenessDistance.clamped(to: 140...360)
        self.catchArmDistance = catchArmDistance.clamped(
            to: 40...self.pointerAwarenessDistance
        )
        self.catchApproachSpeed = catchApproachSpeed.clamped(to: 150...900)
        self.catchWindow = catchWindow.clamped(to: 0.15...1.2)
        self.hitRegionScale = hitRegionScale.clamped(to: 0.75...1.3)
        self.gaitCadence = gaitCadence.clamped(to: 0.5...3.2)
    }

    public static let standard = RuntimeTuning()

    /// Re-applies all safety bounds after UI mutation or Codable decoding.
    public var normalized: RuntimeTuning {
        RuntimeTuning(
            walkingSpeed: walkingSpeed,
            wanderPause: wanderPause,
            crossDisplayWanderChance: crossDisplayWanderChance,
            pointerAwarenessDistance: pointerAwarenessDistance,
            catchArmDistance: catchArmDistance,
            catchApproachSpeed: catchApproachSpeed,
            catchWindow: catchWindow,
            hitRegionScale: hitRegionScale,
            gaitCadence: gaitCadence
        )
    }

    /// How fast the authored walk cycle plays while travelling.
    ///
    /// Deriving this from `walkingSpeed` was tried and rejected: the authored
    /// gait reads correctly at its own cadence even when the pet moves faster,
    /// and retiming it made the walk look busy. It stays a deliberate opt-in at
    /// 1.0 so the default motion is untouched.
    public var locomotionAnimationRate: Double { gaitCadence }

    public var pointerConfiguration: PointerInteractionConfiguration {
        PointerInteractionConfiguration(
            awarenessDistance: pointerAwarenessDistance,
            slowEvadeDistance: 100,
            fastEvadeDistance: 50,
            catchDistance: catchArmDistance,
            slowEvadeSpeed: 74,
            fastEvadeSpeed: 138,
            catchPointerSpeed: catchApproachSpeed,
            catchClosingSpeed: max(120, catchApproachSpeed * 0.48)
        )
    }

    /// A deterministic random mapping keeps the pacing testable while avoiding
    /// a metronomic pause. Standard tuning yields roughly 8.4...17.4 seconds.
    public func wanderDelay(randomUnit: Double) -> TimeInterval {
        wanderPause * (0.7 + randomUnit.clamped(to: 0...1) * 0.75)
    }
}
