// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

import Foundation

/// MVP 0.7 timing only. It intentionally does not include Accessibility,
/// visual placement, agent activity, or context-specific behavior.
public struct RestConfiguration: Equatable, Sendable {
    public var idleBeforeRest: TimeInterval
    public var sittingDuration: TimeInterval
    public var wakeWanderDelay: TimeInterval

    public init(
        idleBeforeRest: TimeInterval = 75,
        sittingDuration: TimeInterval = 2.4,
        wakeWanderDelay: TimeInterval = 2.5
    ) {
        self.idleBeforeRest = idleBeforeRest.clamped(to: 10...3_600)
        self.sittingDuration = sittingDuration.clamped(to: 0.5...10)
        self.wakeWanderDelay = wakeWanderDelay.clamped(to: 0.5...20)
    }

    public static let standard = RestConfiguration()
}

public extension BehaviorState {
    var isResting: Bool {
        switch self {
        case .sit, .findSleepSpot, .sleep:
            true
        default:
            false
        }
    }
}
