// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

import Foundation

public struct AttentionConfiguration: Equatable, Sendable {
    public var minimumDwellTime: TimeInterval
    public var hysteresisMargin: Double
    public var revisitCooldown: TimeInterval
    public var maximumEventAge: TimeInterval

    public init(
        minimumDwellTime: TimeInterval = 3,
        hysteresisMargin: Double = 12,
        revisitCooldown: TimeInterval = 2,
        maximumEventAge: TimeInterval = 30
    ) {
        self.minimumDwellTime = max(0, minimumDwellTime)
        self.hysteresisMargin = max(0, hysteresisMargin)
        self.revisitCooldown = max(0, revisitCooldown)
        self.maximumEventAge = max(0.1, maximumEventAge)
    }
}

public struct AttentionModel: Sendable {
    public var configuration: AttentionConfiguration
    public private(set) var currentSourceID: String?
    public private(set) var currentEventID: String?
    public private(set) var acquiredAt: TimeInterval?

    private var lastLeftAt: [String: TimeInterval] = [:]

    public init(configuration: AttentionConfiguration = .init()) {
        self.configuration = configuration
    }

    public mutating func select(
        from events: [CompanionEvent],
        at timestamp: TimeInterval
    ) -> CompanionEvent? {
        let live = events.filter {
            timestamp - $0.timestamp <= configuration.maximumEventAge
                && timestamp >= $0.timestamp
                && $0.kind != .activityEnded
                && $0.kind != .idle
        }
        guard let best = live.max(by: { score($0, at: timestamp) < score($1, at: timestamp) }) else {
            clear(at: timestamp)
            return nil
        }

        guard let currentSourceID else {
            acquire(best, at: timestamp)
            return best
        }

        if best.sourceID == currentSourceID {
            currentEventID = best.id
            return best
        }

        let dwellAge = timestamp - (acquiredAt ?? timestamp)
        let urgent = best.kind == .attentionRequired || best.kind == .negative || best.kind == .setback
        let current = live
            .filter { $0.sourceID == currentSourceID }
            .max(by: { score($0, at: timestamp) < score($1, at: timestamp) })
        let currentScore = current.map { score($0, at: timestamp) } ?? 0
        let candidateScore = score(best, at: timestamp)
        let inCooldown = lastLeftAt[best.sourceID].map {
            timestamp - $0 < configuration.revisitCooldown
        } ?? false

        if (!urgent && dwellAge < configuration.minimumDwellTime)
            || inCooldown
            || candidateScore < currentScore + configuration.hysteresisMargin {
            return current
        }

        lastLeftAt[currentSourceID] = timestamp
        acquire(best, at: timestamp)
        return best
    }

    public mutating func clear(at timestamp: TimeInterval) {
        if let currentSourceID { lastLeftAt[currentSourceID] = timestamp }
        currentSourceID = nil
        currentEventID = nil
        acquiredAt = nil
    }

    public func score(_ event: CompanionEvent, at timestamp: TimeInterval) -> Double {
        let base: Double
        switch event.kind {
        case .attentionRequired: base = 100
        case .negative, .setback: base = 90
        case .achievement: base = event.intensity >= 0.75 ? 80 : 65
        case .positive: base = 65
        case .highIntensity: base = 75
        // Reading and searching is work worth standing beside, but it is the
        // quietest kind, so it sits below a tool that changes something.
        case .inspecting: base = 60
        case .activityStarted: base = 50
        case .calm: base = 30
        case .activityEnded, .idle: base = 0
        }
        let age = max(0, timestamp - event.timestamp)
        let recency = max(0, 1 - age / configuration.maximumEventAge) * 5
        let location = (event.locationHint?.confidence ?? 0) * 3
        return base + event.intensity * 10 + recency + location
    }

    private mutating func acquire(_ event: CompanionEvent, at timestamp: TimeInterval) {
        currentSourceID = event.sourceID
        currentEventID = event.id
        acquiredAt = timestamp
    }
}
