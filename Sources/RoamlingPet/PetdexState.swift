// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

import Foundation

/// The nine Petdex sprite rows, with what each one *means*.
///
/// This is the vocabulary Roamling borrows, not one it defines. Row order and
/// standard durations come from petdex `src/lib/pet-states.ts`; the agent
/// meanings come from `petdex-desktop-native/src/hook_runner.zig:235`
/// (`stateForEvent`); the transient/steady split comes from
/// `petdex-desktop-native/src/main.zig:1955` (`isDurationState`).
///
/// Nothing here is ours to choose. When upstream changes, re-read those three
/// files and port the change here rather than adjusting a caller to match.
///
/// The reason this type exists at all: the capability table used to name rows
/// by resemblance, and `jumping` looks like a celebration. It is not -- it is
/// the *start* of a turn, and `waving` is the finish. Roamling therefore played
/// the "starting" picture every time an agent finished, on every pet drawn to
/// this contract.
public enum PetdexState: String, CaseIterable, Sendable {
    case idle
    case runningRight = "running-right"
    case runningLeft = "running-left"
    case waving
    case jumping
    case failed
    case waiting
    case running
    case review

    /// Row index in the 8-column atlas.
    public var row: Int {
        switch self {
        case .idle: 0
        case .runningRight: 1
        case .runningLeft: 2
        case .waving: 3
        case .jumping: 4
        case .failed: 5
        case .waiting: 6
        case .running: 7
        case .review: 8
        }
    }

    /// What the row means when an agent drives it (`stateForEvent`).
    public var meaning: String {
        switch self {
        case .idle: "Between events, or a tool call just finished"
        case .runningRight, .runningLeft: "Directional locomotion"
        case .waving: "The turn finished -- \"Done.\""
        case .jumping: "The turn started -- \"Thinking…\""
        case .failed: "A tool call failed"
        case .waiting: "Blocked on the user: approval or input"
        case .running: "A tool other than a read or search is running"
        case .review: "About to read or search files"
        }
    }

    /// `pet-states.ts` durationMs, in seconds.
    public var standardDuration: TimeInterval {
        switch self {
        case .idle: 1.100
        case .runningRight, .runningLeft: 1.060
        case .waving: 0.700
        case .jumping: 0.840
        case .failed: 1.220
        case .waiting: 1.010
        case .running: 0.820
        case .review: 1.030
        }
    }

    /// True when the row plays once and hands back to `idle`; false when it
    /// holds until the next event arrives.
    ///
    /// Roamling has to honour this or it stretches a one-second picture across
    /// a whole agent session. `review` is the case that bit us.
    public var isTransient: Bool {
        switch self {
        case .waving, .failed, .review, .jumping: true
        case .idle, .running, .waiting, .runningRight, .runningLeft: false
        }
    }

    /// Alternative track names Codex's `model.rs` accepts for the same row.
    public var aliases: [String] {
        switch self {
        case .runningRight: ["move-right", "move_right"]
        case .runningLeft: ["move-left", "move_left"]
        case .waving: ["wave"]
        case .jumping: ["jump", "bounce"]
        case .failed: ["failure", "sad"]
        default: []
        }
    }

    /// Every name that answers for this row, most specific first.
    public var trackNames: [String] { [rawValue] + aliases }
}
