// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

import CoreGraphics
import Foundation
import RoamlingCore

/// Reads only the elapsed time since local keyboard/pointer input. It does not
/// install an event tap, record event content, or require Accessibility.
@MainActor
public final class MacUserIdleProvider: UserIdleProviding {
    private static let anyInputEvent = CGEventType(rawValue: UInt32.max)!
    private let sampleInterval: TimeInterval
    private var sampledAt: TimeInterval?
    private var sampledIdleDuration: TimeInterval = 0

    public init(sampleInterval: TimeInterval = 0.5) {
        self.sampleInterval = max(0.1, sampleInterval)
    }

    public func idleDuration(at timestamp: TimeInterval) -> TimeInterval {
        if let sampledAt, timestamp - sampledAt < sampleInterval {
            return sampledIdleDuration + max(0, timestamp - sampledAt)
        }

        let duration = CGEventSource.secondsSinceLastEventType(
            .combinedSessionState,
            eventType: Self.anyInputEvent
        )
        sampledAt = timestamp
        sampledIdleDuration = duration.isFinite ? max(0, duration) : 0
        return sampledIdleDuration
    }
}
