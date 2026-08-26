// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

import Foundation

public struct ReactionConfiguration: Equatable, Sendable {
    public var minimumInterval: TimeInterval
    public var focusedContextScale: Double
    public var mediaContextScale: Double
    public var gamingContextScale: Double

    public init(
        minimumInterval: TimeInterval = 1.5,
        focusedContextScale: Double = 0.8,
        mediaContextScale: Double = 0.4,
        gamingContextScale: Double = 0.5
    ) {
        self.minimumInterval = max(0, minimumInterval)
        self.focusedContextScale = focusedContextScale.clamped(to: 0...1)
        self.mediaContextScale = mediaContextScale.clamped(to: 0...1)
        self.gamingContextScale = gamingContextScale.clamped(to: 0...1)
    }
}

public struct ReactionPolicy: Sendable {
    public var configuration: ReactionConfiguration
    private var lastReactionAt: TimeInterval?

    public init(configuration: ReactionConfiguration = .init()) {
        self.configuration = configuration
    }

    public mutating func reaction(
        for event: CompanionEvent,
        context: UserContext,
        currentBehavior: BehaviorState,
        randomUnit: Double,
        at timestamp: TimeInterval
    ) -> CompanionReaction? {
        guard currentBehavior != .caught && currentBehavior != .dragged else { return nil }
        if let lastReactionAt,
           timestamp - lastReactionAt < configuration.minimumInterval,
           event.kind != .attentionRequired,
           event.kind != .negative,
           event.kind != .setback {
            return nil
        }

        let contextScale: Double
        switch context {
        case .working: contextScale = configuration.focusedContextScale
        case .gaming: contextScale = configuration.gamingContextScale
        case .watchingMedia: contextScale = configuration.mediaContextScale
        case .browsing, .idle: contextScale = 1
        }
        let effectiveIntensity = (event.intensity * contextScale).clamped(to: 0...1)
        let roll = randomUnit.clamped(to: 0...0.999_999)

        let result: CompanionReaction?
        switch event.kind {
        case .attentionRequired:
            result = roll < 0.65 ? .paw : .observe
        case .negative, .setback:
            result = effectiveIntensity >= 0.2 ? .sad : .glance
        case .achievement:
            if effectiveIntensity >= 0.75, roll < effectiveIntensity * 0.7 {
                result = .largeCelebrate
            } else {
                // An achievement is a meaningful completion. Keep the size
                // probabilistic, but always make the acknowledgement visible.
                result = .smallCelebrate
            }
        case .positive:
            if effectiveIntensity < 0.15 {
                result = nil
            } else if effectiveIntensity >= 0.75, roll < effectiveIntensity * 0.7 {
                result = .largeCelebrate
            } else if roll < 0.15 + effectiveIntensity * 0.55 {
                result = .smallCelebrate
            } else if roll < 0.75 {
                result = .glance
            } else {
                result = nil
            }
        case .activityStarted, .highIntensity:
            result = effectiveIntensity >= 0.5 ? .work : .observe
        case .calm:
            result = .calm
        case .activityEnded, .idle:
            result = nil
        }

        if result != nil { lastReactionAt = timestamp }
        return result
    }
}
