// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

import Foundation

/// What Roamling asks to be shown. A superset of `PetdexState`: nine of these
/// mean exactly one Petdex row, and the rest are desktop-pet ideas the Petdex
/// contract has no word for.
///
/// Every case declares which it is -- `petdexState` for the nine, `borrows` for
/// the rest -- and the resolver reads those declarations instead of a table of
/// track names. A bare name list cannot say that `jumping` opens a turn, which
/// is how celebrate came to play it.
public enum PetCapability: String, CaseIterable, Codable, Hashable, Sendable {
    case idle
    case moveLeft
    case moveRight
    case sit
    case sleep
    case work
    case observe
    case gaze
    case paw
    case spark
    case celebrate
    case fail
    case stretch
    case caught
    case dragged
    case landing
}

public extension PetCapability {
    /// How a capability finds artwork when it has none of its own.
    ///
    /// The two borrowing kinds read the same to the resolver and differently to
    /// a person, which is the point: `landing` wants the *hop*, `celebrate`
    /// wants the *sentiment*. Recording only "falls back to" is how landing
    /// ended up chained behind celebrate, so that fixing celebrate's meaning
    /// would have turned every landing into a farewell wave.
    enum Borrow: Equatable, Sendable {
        /// Borrow what it says (a finished turn is greeted, so celebrate waves).
        case meaning(PetCapability)
        /// Borrow what it does (a landing really is a hop).
        case motion(PetCapability)

        public var capability: PetCapability {
            switch self {
            case let .meaning(value), let .motion(value): value
            }
        }
    }

    /// The Petdex row this capability *is*, or nil when Petdex has no such idea.
    var petdexState: PetdexState? {
        switch self {
        case .idle: .idle
        case .moveLeft: .runningLeft
        case .moveRight: .runningRight
        case .work: .running
        case .observe: .review
        case .paw: .waiting
        case .spark: .jumping
        case .celebrate: .waving
        case .fail: .failed
        case .sit, .sleep, .gaze, .stretch, .caught, .dragged, .landing: nil
        }
    }

    /// Names a package may use for this capability under its own vocabulary.
    /// Tried before the Petdex row name, so a package that draws the real thing
    /// always wins over a borrowed row.
    var authoredNames: [String] {
        switch self {
        case .idle: ["idle"]
        case .moveLeft: []
        case .moveRight: []
        case .sit: ["sitting", "sit"]
        case .sleep: ["sleeping", "sleep", "napping"]
        case .work: ["working", "typing"]
        case .observe: ["observe"]
        // Watching the cursor is not reviewing a file. `watching` is the name a
        // Roamling-aware package uses for it; `review` deliberately is not.
        case .gaze: ["gaze", "watching", "looking"]
        case .paw: ["pawing", "paw"]
        case .spark: ["spark"]
        case .celebrate: ["celebrate"]
        case .fail: []
        case .stretch: ["stretching", "stretch"]
        case .caught: ["caught"]
        case .dragged: ["dragged"]
        case .landing: ["landing"]
        }
    }

    /// What to fall back to, and in which sense.
    var borrows: Borrow? {
        switch self {
        case .idle: nil
        case .moveLeft, .moveRight: .motion(.idle)
        // Settling down goes to `idle` -- "between events" -- rather than to
        // `waiting`, whose contract meaning is "blocked on the user: approval
        // or input". A drowsy pet has not asked for anything, and borrowing
        // `waiting` put the picture for needing approval on screen every time
        // one got sleepy, on every Petdex pet rather than just this one.
        //
        // The seated pose `waiting` happens to have is not a reason to keep it:
        // Petdex fixes what a row means and leaves the pose to whoever draws
        // the pet, so a pet whose `waiting` is drawn standing would be no worse
        // served by `idle`, and every pet is better served by the right signal.
        case .sit: .meaning(.idle)
        case .sleep: .meaning(.sit)
        case .work: .motion(.moveRight)
        case .observe: .meaning(.idle)
        // Petdex has no picture for watching the cursor for as long as it is
        // near, so this degrades to stillness rather than to `review`, which is
        // a one-second "reading a file" beat.
        case .gaze: .meaning(.idle)
        case .paw: .meaning(.idle)
        case .spark: .meaning(.celebrate)
        case .celebrate: .meaning(.idle)
        case .fail: .meaning(.idle)
        case .stretch: .motion(.idle)
        // A held pet looking up at the cursor reads as held. A jump does not:
        // it loops, so while the cursor carries the pet it looks like the pet
        // is bouncing under its own power, which is the opposite of caught.
        case .caught: .meaning(.paw)
        case .dragged: .meaning(.caught)
        // Landing is the one that really is a hop, so it takes the jump row
        // directly instead of routing through celebrate, which now waves.
        case .landing: .motion(.spark)
        }
    }
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
            for name in Self.candidates(for: step, explicit: explicitBehaviors) {
                if let resolved = resolveTrack(named: name, visited: []) {
                    return (resolved, step == capability ? .authored : .substituted(step))
                }
            }
            guard let next = step.borrows?.capability else { break }
            step = next
        }
        // A package with nothing recognisable still has to render something, or
        // the pet disappears. Callers are told, because a silent stand-in for
        // every state is exactly the failure this reports.
        //
        // Sorted, and that has to be said out loud: this used to read
        // `tracks.values.first`, whose order Swift randomizes per process. A
        // package that declares two rows and no `idle` drew a different one of
        // them on every launch -- the fourth time an unordered collection has
        // been found deciding something the user can see.
        let anyTrack = tracks.keys.sorted().first.flatMap { tracks[$0] }
        return (tracks["idle"] ?? anyTrack, .placeholder)
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

    /// Every track name that answers for one capability, best first.
    ///
    /// Generated from the capability's own declaration rather than written out
    /// per capability. The flat table this replaced listed `jumping` ahead of
    /// `waving` under `celebrate`, which is backwards -- in the Petdex contract
    /// `jumping` starts a turn and `waving` ends one -- and nothing in a list of
    /// bare strings could have said so.
    static func candidates(
        for capability: PetCapability,
        explicit: [String: String]
    ) -> [String] {
        var names: [String] = []
        if let mapped = explicit[capability.rawValue] { names.append(mapped) }
        names.append(contentsOf: capability.authoredNames)
        if let state = capability.petdexState {
            names.append(contentsOf: state.trackNames)
        }
        return names
    }
}
