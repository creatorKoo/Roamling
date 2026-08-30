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

    /// How a capability was answered, so a caller can tell a package's own
    /// artwork from something borrowed to stand in for it.
    public enum Provenance: Equatable, Sendable {
        /// The package authors this capability, under one of its own names.
        case authored
        /// Answered by a related capability's artwork. Expected, not a defect:
        /// the Petdex contract has nine agent states and no notion of a pet
        /// that sleeps, sits, or gets picked up.
        case substituted(PetCapability)
        /// Nothing related existed, so the pet's resting pose is standing in.
        case placeholder
    }

    public func resolve(_ capability: PetCapability) -> PetAnimationTrack? {
        resolution(capability).track
    }

    /// Resolves along meaning rather than down one flat list of names.
    ///
    /// Each capability names the one it degrades into, so improving a
    /// substitution improves everything downstream of it: teaching `sit` about
    /// a seated pose also seats the pet when it sleeps. A flat list per
    /// capability could not do that, and every capability had to repeat the
    /// same fallbacks by hand.
    public func resolution(
        _ capability: PetCapability
    ) -> (track: PetAnimationTrack?, provenance: Provenance) {
        var seen: Set<PetCapability> = []
        var step = capability
        while !seen.contains(step) {
            seen.insert(step)
            var candidates: [String] = []
            if let explicit = explicitBehaviors[step.rawValue] {
                candidates.append(explicit)
            }
            candidates.append(contentsOf: Self.candidates[step] ?? [])
            for name in candidates {
                if let resolved = resolveTrack(named: name, visited: []) {
                    return (resolved, step == capability ? .authored : .substituted(step))
                }
            }
            guard let next = Self.degradesInto[step] else { break }
            step = next
        }
        // A package with nothing recognisable still has to render something, or
        // the pet disappears. Callers are told, because a silent stand-in for
        // every state is exactly the failure this reports.
        return (tracks["idle"] ?? tracks.values.first, .placeholder)
    }

    /// What a pet can and cannot show, for a caller that wants to say so.
    ///
    /// The counts exist because a package with one declared animation renders
    /// that animation for every state and looks, from outside, like a pet whose
    /// behaviour is broken. Nothing surfaced that, so a whole session went into
    /// chasing a sleep bug that was a missing sprite row.
    public struct Coverage: Equatable, Sendable {
        public var authored: [PetCapability]
        public var substituted: [PetCapability]
        /// Answered by the last-resort pose because nothing related existed.
        public var placeholder: [PetCapability]

        public var total: Int { PetCapability.allCases.count }
        /// Substituted counts as covered: the Petdex contract has no sleeping
        /// or sitting pet, so borrowing for those is the design, not a fault.
        public var covered: Int { authored.count + substituted.count }
        public var isComplete: Bool { placeholder.isEmpty }
    }

    public var coverage: Coverage {
        var result = Coverage(authored: [], substituted: [], placeholder: [])
        for capability in PetCapability.allCases {
            switch resolution(capability).provenance {
            case .authored: result.authored.append(capability)
            case .substituted: result.substituted.append(capability)
            case .placeholder: result.placeholder.append(capability)
            }
        }
        return result
    }

    private func resolveTrack(named name: String, visited: Set<String>) -> PetAnimationTrack? {
        guard !visited.contains(name), let track = tracks[name] else { return nil }
        if !track.frames.isEmpty { return track }
        guard let fallback = track.fallback else { return nil }
        var nextVisited = visited
        nextVisited.insert(name)
        return resolveTrack(named: fallback, visited: nextVisited)
    }

    /// What each capability falls back to when the package does not author it.
    ///
    /// The Petdex table is nine agent states -- idle, both running rows,
    /// waving, jumping, failed, waiting, running, review -- and a Codex pet
    /// never sleeps, sits still to rest, stretches, or gets picked up. Those
    /// belong to a desktop pet, so no package will ever supply them and the
    /// substitution has to be deliberate rather than accidental.
    private static let degradesInto: [PetCapability: PetCapability] = [
        .sleep: .sit,
        // Sitting down borrows the seated, expectant pose, which is as close as
        // an agent pet gets to settling.
        .sit: .paw,
        .stretch: .idle,
        .caught: .celebrate,
        .dragged: .caught,
        .landing: .celebrate,
        .paw: .idle,
        .work: .moveRight,
        .observe: .idle,
        .celebrate: .idle,
        .fail: .idle,
        .moveLeft: .idle,
        .moveRight: .idle
    ]

    private static let candidates: [PetCapability: [String]] = [
        .idle: ["idle"],
        .moveLeft: ["running-left", "move-left", "move_left"],
        .moveRight: ["running-right", "move-right", "move_right"],
        .sit: ["sitting", "sit"],
        .sleep: ["sleeping", "sleep", "napping"],
        .work: ["working", "typing", "running"],
        .observe: ["watching", "observe", "review"],
        // Codex's `waiting` means the agent needs approval or input, which is
        // this capability exactly. A wave is a greeting, so it comes second.
        .paw: ["pawing", "paw", "waiting", "waving", "wave"],
        .celebrate: ["celebrate", "jumping", "jump", "bounce", "waving"],
        .fail: ["failed", "failure", "sad"],
        .stretch: ["stretching", "stretch"],
        .caught: ["caught"],
        .dragged: ["dragged"],
        .landing: ["landing"]
    ]
}
