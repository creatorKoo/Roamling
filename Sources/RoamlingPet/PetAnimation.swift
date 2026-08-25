// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

import Foundation

public enum PetCapability: String, CaseIterable, Codable, Hashable, Sendable {
    case idle
    case moveLeft
    case moveRight
    case sit
    case sleep
    case work
    case observe
    case paw
    case celebrate
    case fail
    case stretch
    case caught
    case dragged
    case landing
}

public struct PetAnimationFrame: Equatable, Sendable {
    public let index: Int
    public let duration: TimeInterval

    public init(index: Int, duration: TimeInterval) {
        self.index = index
        self.duration = max(0.001, duration)
    }
}

public struct PetAnimationTrack: Equatable, Sendable {
    public let name: String
    public let frames: [PetAnimationFrame]
    public let loops: Bool
    public let fallback: String?

    public init(name: String, frames: [PetAnimationFrame], loops: Bool = true, fallback: String? = nil) {
        self.name = name
        self.frames = frames
        self.loops = loops
        self.fallback = fallback
    }
}

public enum StandardPetAnimations {
    public static func tracks(columns: Int) -> [String: PetAnimationTrack] {
        var result: [String: PetAnimationTrack] = [:]
        result["idle"] = track("idle", row: 0, columns: columns, durationsMS: [280, 110, 110, 140, 140, 320])
        result["running-right"] = track("running-right", row: 1, columns: columns, durationsMS: [120, 120, 120, 120, 120, 120, 120, 220])
        result["running-left"] = track("running-left", row: 2, columns: columns, durationsMS: [120, 120, 120, 120, 120, 120, 120, 220])
        result["waving"] = track("waving", row: 3, columns: columns, durationsMS: [140, 140, 140, 280])
        result["jumping"] = track("jumping", row: 4, columns: columns, durationsMS: [140, 140, 140, 140, 280])
        result["failed"] = track("failed", row: 5, columns: columns, durationsMS: [140, 140, 140, 140, 140, 140, 140, 240])
        result["waiting"] = track("waiting", row: 6, columns: columns, durationsMS: [150, 150, 150, 150, 150, 260])
        result["running"] = track("running", row: 7, columns: columns, durationsMS: [120, 120, 120, 120, 120, 220])
        result["review"] = track("review", row: 8, columns: columns, durationsMS: [150, 150, 150, 150, 150, 280])
        return result
    }

    private static func track(
        _ name: String,
        row: Int,
        columns: Int,
        durationsMS: [Int]
    ) -> PetAnimationTrack {
        let frames = durationsMS.enumerated().map { offset, milliseconds in
            PetAnimationFrame(
                index: row * columns + offset,
                duration: Double(milliseconds) / 1_000
            )
        }
        return PetAnimationTrack(name: name, frames: frames, loops: true)
    }
}

public struct AnimationResolver: Sendable {
    public let tracks: [String: PetAnimationTrack]
    public let explicitBehaviors: [String: String]

    public init(tracks: [String: PetAnimationTrack], explicitBehaviors: [String: String] = [:]) {
        self.tracks = tracks
        self.explicitBehaviors = explicitBehaviors
    }

    public func resolve(_ capability: PetCapability) -> PetAnimationTrack? {
        var candidates: [String] = []
        if let explicit = explicitBehaviors[capability.rawValue] {
            candidates.append(explicit)
        }
        candidates.append(contentsOf: Self.candidates[capability] ?? [])

        for name in candidates {
            if let resolved = resolveTrack(named: name, visited: []) { return resolved }
        }
        return tracks["idle"] ?? tracks.values.first
    }

    private func resolveTrack(named name: String, visited: Set<String>) -> PetAnimationTrack? {
        guard !visited.contains(name), let track = tracks[name] else { return nil }
        if !track.frames.isEmpty { return track }
        guard let fallback = track.fallback else { return nil }
        var nextVisited = visited
        nextVisited.insert(name)
        return resolveTrack(named: fallback, visited: nextVisited)
    }

    private static let candidates: [PetCapability: [String]] = [
        .idle: ["idle"],
        .moveLeft: ["running-left", "move-left", "move_left", "running", "idle"],
        .moveRight: ["running-right", "move-right", "move_right", "running", "idle"],
        .sit: ["sitting", "sit", "idle"],
        .sleep: ["sleeping", "sleep", "idle"],
        .work: ["working", "typing", "running", "idle"],
        .observe: ["watching", "observe", "review", "waiting", "idle"],
        .paw: ["pawing", "paw", "waving", "wave", "waiting", "idle"],
        .celebrate: ["celebrate", "jumping", "jump", "bounce", "waving", "idle"],
        .fail: ["failed", "failure", "sad", "idle"],
        .stretch: ["stretching", "stretch", "idle"],
        .caught: ["caught", "waiting", "idle"],
        .dragged: ["dragged", "caught", "waiting", "idle"],
        .landing: ["landing", "jumping", "idle"]
    ]
}
