// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

import Foundation

/// Names one tunable value, so a panel can be built from the model rather
/// than from a second list that drifts away from it.
public enum RuntimeTuningKey: String, CaseIterable, Codable, Sendable {
    case walkingSpeed
    case wanderPause
    case crossDisplayWanderChance
    case idleBeforeRest
    case pointerAwarenessDistance
    case evadeSpeedScale
    case catchArmDistance
    case catchApproachSpeed
    case catchWindow
    case hitRegionScale
    case gaitCadence
}

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
    public var evadeSpeedScale: Double
    public var idleBeforeRest: Double

    public init(
        walkingSpeed: Double = 160,
        wanderPause: Double = 12,
        crossDisplayWanderChance: Double = 0.46,
        pointerAwarenessDistance: Double = 170,
        catchArmDistance: Double = 74,
        catchApproachSpeed: Double = 380,
        catchWindow: Double = 0.35,
        hitRegionScale: Double = 1.12,
        gaitCadence: Double = 1,
        evadeSpeedScale: Double = 1.4,
        idleBeforeRest: Double = RestConfiguration.standard.idleBeforeRest
    ) {
        // Every bound comes from `bounds(_:pointerAwareness:)` so the panel can
        // ask for the same numbers instead of restating them.
        func bound(_ key: RuntimeTuningKey, _ awareness: Double = 0) -> ClosedRange<Double> {
            RuntimeTuning.bounds(key, pointerAwareness: awareness)
        }
        self.walkingSpeed = walkingSpeed.clamped(to: bound(.walkingSpeed))
        self.wanderPause = wanderPause.clamped(to: bound(.wanderPause))
        self.crossDisplayWanderChance = crossDisplayWanderChance
            .clamped(to: bound(.crossDisplayWanderChance))
        self.pointerAwarenessDistance = pointerAwarenessDistance
            .clamped(to: bound(.pointerAwarenessDistance))
        // Clamped after pointer awareness, because its ceiling is that value:
        // arming a catch further away than the pet can notice is meaningless.
        self.catchArmDistance = catchArmDistance
            .clamped(to: bound(.catchArmDistance, self.pointerAwarenessDistance))
        self.catchApproachSpeed = catchApproachSpeed.clamped(to: bound(.catchApproachSpeed))
        self.catchWindow = catchWindow.clamped(to: bound(.catchWindow))
        self.hitRegionScale = hitRegionScale.clamped(to: bound(.hitRegionScale))
        self.gaitCadence = gaitCadence.clamped(to: bound(.gaitCadence))
        self.evadeSpeedScale = evadeSpeedScale.clamped(to: bound(.evadeSpeedScale))
        self.idleBeforeRest = idleBeforeRest.clamped(to: bound(.idleBeforeRest))
    }

    /// Decoding tolerates a saved blob written before a field existed.
    ///
    /// The synthesized decoder requires every key, so adding one value silently
    /// threw away everything the user had tuned and reset the panel. Missing
    /// values now take their default instead.
    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        let fallback = RuntimeTuning()
        func value(_ key: CodingKeys, _ standard: Double) throws -> Double {
            try container.decodeIfPresent(Double.self, forKey: key) ?? standard
        }
        self.init(
            walkingSpeed: try value(.walkingSpeed, fallback.walkingSpeed),
            wanderPause: try value(.wanderPause, fallback.wanderPause),
            crossDisplayWanderChance: try value(
                .crossDisplayWanderChance,
                fallback.crossDisplayWanderChance
            ),
            pointerAwarenessDistance: try value(
                .pointerAwarenessDistance,
                fallback.pointerAwarenessDistance
            ),
            catchArmDistance: try value(.catchArmDistance, fallback.catchArmDistance),
            catchApproachSpeed: try value(.catchApproachSpeed, fallback.catchApproachSpeed),
            catchWindow: try value(.catchWindow, fallback.catchWindow),
            hitRegionScale: try value(.hitRegionScale, fallback.hitRegionScale),
            gaitCadence: try value(.gaitCadence, fallback.gaitCadence),
            evadeSpeedScale: try value(.evadeSpeedScale, fallback.evadeSpeedScale),
            idleBeforeRest: try value(.idleBeforeRest, fallback.idleBeforeRest)
        )
    }

    public static let standard = RuntimeTuning()

    private static func bounds(
        _ key: RuntimeTuningKey,
        pointerAwareness: Double
    ) -> ClosedRange<Double> {
        switch key {
        case .walkingSpeed: 20...320
        case .wanderPause: 2...40
        case .crossDisplayWanderChance: 0...1
        case .idleBeforeRest: 15...600
        case .pointerAwarenessDistance: 140...360
        case .evadeSpeedScale: 0.8...3
        case .catchArmDistance: 40...pointerAwareness
        case .catchApproachSpeed: 150...900
        case .catchWindow: 0.15...1.2
        case .hitRegionScale: 0.75...1.3
        case .gaitCadence: 0.5...3.2
        }
    }

    /// What `init` will clamp this value to, given the rest of this tuning.
    ///
    /// An instance method because one bound moves: `catchArmDistance` cannot
    /// exceed `pointerAwarenessDistance`. A fixed table has to guess a ceiling,
    /// and the panel guessed 140 while the model allowed up to 360.
    public func limits(for key: RuntimeTuningKey) -> ClosedRange<Double> {
        Self.bounds(key, pointerAwareness: pointerAwarenessDistance)
    }

    public subscript(key: RuntimeTuningKey) -> Double {
        get {
            switch key {
            case .walkingSpeed: walkingSpeed
            case .wanderPause: wanderPause
            case .crossDisplayWanderChance: crossDisplayWanderChance
            case .idleBeforeRest: idleBeforeRest
            case .pointerAwarenessDistance: pointerAwarenessDistance
            case .evadeSpeedScale: evadeSpeedScale
            case .catchArmDistance: catchArmDistance
            case .catchApproachSpeed: catchApproachSpeed
            case .catchWindow: catchWindow
            case .hitRegionScale: hitRegionScale
            case .gaitCadence: gaitCadence
            }
        }
        set {
            switch key {
            case .walkingSpeed: walkingSpeed = newValue
            case .wanderPause: wanderPause = newValue
            case .crossDisplayWanderChance: crossDisplayWanderChance = newValue
            case .idleBeforeRest: idleBeforeRest = newValue
            case .pointerAwarenessDistance: pointerAwarenessDistance = newValue
            case .evadeSpeedScale: evadeSpeedScale = newValue
            case .catchArmDistance: catchArmDistance = newValue
            case .catchApproachSpeed: catchApproachSpeed = newValue
            case .catchWindow: catchWindow = newValue
            case .hitRegionScale: hitRegionScale = newValue
            case .gaitCadence: gaitCadence = newValue
            }
        }
    }

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
            gaitCadence: gaitCadence,
            evadeSpeedScale: evadeSpeedScale,
            idleBeforeRest: idleBeforeRest
        )
    }

    /// Rest timing, so the one value a user has a reason to change is the one
    /// exposed. Sitting and waking stay authored: they are animation pacing,
    /// not a preference.
    public var restConfiguration: RestConfiguration {
        RestConfiguration(idleBeforeRest: idleBeforeRest)
    }

    /// How fast the authored walk cycle plays while travelling.
    ///
    /// Deriving this from `walkingSpeed` was tried and rejected: the authored
    /// gait reads correctly at its own cadence even when the pet moves faster,
    /// and retiming it made the walk look busy. It stays a deliberate opt-in at
    /// 1.0 so the default motion is untouched no matter how the speed is tuned.
    ///
    /// Re-examined when Mochi's walk was retimed from 0.727s to the Petdex
    /// standard 1.06s, which is a 46% slower cycle over unchanged ground speed
    /// and stretched the stride from 1.89 body lengths to 2.77. Left at 1.0
    /// anyway: the row is a bounding gait with three of its eight frames
    /// airborne and a different paw touching down in each of the rest, so no paw
    /// is planted long enough to slip against the ground, and there is no
    /// mechanical error for a cadence multiplier to correct. Whether the longer
    /// stride reads well is a judgment, and this is the knob for it --
    /// `scripts/preview_gait.py` renders the walk over scrolling ground at the
    /// tuned speed so the two can be compared without launching the app.
    public var locomotionAnimationRate: Double { gaitCadence }

    /// Evading has to outrun strolling. Fixed evade speeds meant a fast walking
    /// speed made the pet look slower when it was trying to get out of the way,
    /// so both evade speeds scale with `walkingSpeed`.
    ///
    /// The floor keeps evasion usable at the slowest walking speeds; a pet that
    /// cannot step aside is worse than one that steps aside briskly. It only
    /// takes over below roughly a 43 pt/s walk, so it never overrides a tuned
    /// value that is already fast enough.
    public var fastEvadeSpeed: Double { max(60, walkingSpeed * evadeSpeedScale) }

    /// The gentle sidestep keeps the authored 74:138 relationship to the urgent
    /// one, so the two still read as different reactions.
    public var slowEvadeSpeed: Double { fastEvadeSpeed * 0.55 }

    public var pointerConfiguration: PointerInteractionConfiguration {
        PointerInteractionConfiguration(
            awarenessDistance: pointerAwarenessDistance,
            slowEvadeDistance: 100,
            fastEvadeDistance: 50,
            catchDistance: catchArmDistance,
            slowEvadeSpeed: slowEvadeSpeed,
            fastEvadeSpeed: fastEvadeSpeed,
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
