// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

import Foundation

public enum ActivitySourceType: Codable, Hashable, Sendable {
    case agent
    case game
    case media
    case system
    case custom(String)
}

public enum CompanionEventKind: String, Codable, Hashable, Sendable {
    case activityStarted
    case activityEnded
    case positive
    case negative
    case achievement
    case setback
    case attentionRequired
    case highIntensity
    case calm
    case idle
}

public enum UserContext: String, Codable, Hashable, Sendable {
    case working
    case gaming
    case watchingMedia
    case browsing
    case idle
}

public enum CompanionMetadata: Codable, Hashable, Sendable {
    case string(String)
    case number(Double)
    case boolean(Bool)
}

public struct LocationHint: Codable, Hashable, Sendable {
    public let applicationIdentifier: String?
    public let displayID: String?
    public let approximateRegion: WorldRect?
    public let confidence: Double

    public init(
        applicationIdentifier: String? = nil,
        displayID: String? = nil,
        approximateRegion: WorldRect? = nil,
        confidence: Double = 0
    ) {
        self.applicationIdentifier = applicationIdentifier
        self.displayID = displayID
        self.approximateRegion = approximateRegion
        self.confidence = confidence.clamped(to: 0...1)
    }
}

public struct CompanionEvent: Codable, Hashable, Sendable, Identifiable {
    public let id: String
    public let sourceID: String
    public let sourceType: ActivitySourceType
    public let timestamp: TimeInterval
    public let kind: CompanionEventKind
    public let intensity: Double
    public let context: UserContext?
    public let locationHint: LocationHint?
    public let metadata: [String: CompanionMetadata]

    public init(
        id: String = UUID().uuidString,
        sourceID: String,
        sourceType: ActivitySourceType,
        timestamp: TimeInterval,
        kind: CompanionEventKind,
        intensity: Double = 0.5,
        context: UserContext? = nil,
        locationHint: LocationHint? = nil,
        metadata: [String: CompanionMetadata] = [:]
    ) {
        self.id = id
        self.sourceID = sourceID
        self.sourceType = sourceType
        self.timestamp = timestamp
        self.kind = kind
        self.intensity = intensity.clamped(to: 0...1)
        self.context = context
        self.locationHint = locationHint
        self.metadata = metadata
    }
}

public protocol ActivitySource: Sendable {
    var id: String { get }
    var sourceType: ActivitySourceType { get }
    func makeEventStream() -> AsyncStream<CompanionEvent>
}
