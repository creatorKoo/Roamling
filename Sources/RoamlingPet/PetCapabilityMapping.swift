// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

import Foundation
import RoamlingCore

/// Maps a behavior state onto the animation capability that should play for it.
///
/// Deliberately exhaustive: adding a `BehaviorState` case must break the build
/// here rather than silently fall through to `.idle`. That silent fallthrough is
/// how `travelToInterest` ended up sliding across the desktop on idle frames.
public enum PetCapabilityMapping {
    public static func capability(
        for state: BehaviorState,
        velocityDX: Double,
        isCaughtTransitionActive: Bool
    ) -> PetCapability {
        switch state {
        case .idle:
            return .idle
        case .wander, .evadePointer, .findSleepSpot, .travelToInterest:
            return velocityDX < 0 ? .moveLeft : .moveRight
        case .sit:
            return .sit
        // Watching the cursor and reviewing a file are different pictures. They
        // shared `observe` -- and so shared Petdex's `review` row, a one-second
        // "about to read a file" beat -- which is how a cursor drifting past the
        // pet started a loop that ran until something else interrupted it.
        case .lookAtPointer:
            return .gaze
        case .observe:
            return .observe
        case .spark:
            return .spark
        case .caught:
            return .caught
        case .dragged:
            return isCaughtTransitionActive ? .caught : .dragged
        case .dropped:
            return .landing
        case .work:
            return .work
        case .waitingForUser:
            return .paw
        case .celebrate:
            return .celebrate
        case .sad:
            return .fail
        case .sleep:
            return .sleep
        case .stretch, .wake:
            return .stretch
        }
    }

    /// States that walk the pet across the desktop. Every one of them must
    /// resolve to a directional move track.
    public static let movingStates: Set<BehaviorState> = [
        .wander, .evadePointer, .findSleepSpot, .travelToInterest
    ]
}
