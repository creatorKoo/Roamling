// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

import Foundation

public enum BehaviorState: String, CaseIterable, Codable, Hashable, Sendable {
    case idle
    case wander
    case lookAtPointer
    case evadePointer
    case caught
    case dragged
    case dropped
    case sit
    case findSleepSpot
    case sleep
    case wake
    case stretch
    case travelToInterest
    case observe
    case spark
    case work
    case waitingForUser
    case celebrate
    case sad
}

public enum CompanionReaction: String, Codable, Hashable, Sendable {
    case glance
    case observe
    /// The agent just started a turn. Petdex plays `jumping` here and calls it
    /// "Thinking…", so Roamling does the same rather than staring.
    case spark
    case work
    case paw
    case smallCelebrate
    case largeCelebrate
    case sad
    case calm
}

public extension CompanionReaction {
    /// True when the reaction describes a condition that lasts, rather than a
    /// moment that passes.
    ///
    /// Only these may be re-applied while the pet holds a seat beside a working
    /// agent. Re-applying a moment every tick is how a one-second "reading a
    /// file" beat turned into a loop that ran for as long as the session did.
    var isOngoing: Bool {
        switch self {
        case .work, .paw: true
        case .glance, .observe, .spark, .smallCelebrate, .largeCelebrate, .sad, .calm: false
        }
    }
}

public enum BehaviorInput: Equatable, Sendable {
    case beginWander
    case arrived
    case beginRest
    case seekSleepSpot
    case sleepSpotReached
    case beginStretch
    case beginInterestTravel
    case pointer(PointerProximity)
    case catchBegan
    case dragMoved
    case mouseReleased
    case reaction(CompanionReaction)
    case meaningfulActivity
    case tick
}

public struct BehaviorTransition: Equatable, Sendable {
    public let from: BehaviorState
    public let to: BehaviorState
    public let changed: Bool

    /// Public so the Rust core can report one.
    public init(from: BehaviorState, to: BehaviorState, changed: Bool) {
        self.from = from
        self.to = to
        self.changed = changed
    }
}

public struct BehaviorController: Sendable {
    /// States that `.beginWander` may start roaming from.
    ///
    /// `.observe` and `.work` belong here because the activity that parked the
    /// pet in them is already cleared before roaming runs, and `.wander` because
    /// a route can be cancelled without ever arriving. Omitting any of them
    /// strands the pet: the caller lays a route the state machine refuses, and
    /// the pet walks it wearing the wrong animation.
    ///
    /// `.lookAtPointer` is here for one caller only. Placement refuses to start
    /// an aimless walk while the cursor is being watched, so the sole route
    /// that arrives in this state is the one that takes the pet off the user's
    /// work -- and refusing that left the pet holding the paragraph for as long
    /// as the cursor sat beside it.
    public static let wanderEntryStates: Set<BehaviorState> = [
        .idle, .dropped, .wander, .observe, .work, .lookAtPointer
    ]

    /// States that `.beginRest` may start sitting from.
    ///
    /// `.observe` and `.work` belong here because a pet parked beside a working
    /// agent is standing on a seat it already vetted. Refusing rest there means
    /// it can only ever doze between sessions, which is exactly when the user is
    /// least likely to be watching.
    public static let restEntryStates: Set<BehaviorState> = [
        .idle, .wander, .dropped, .observe, .work
    ]

    public private(set) var state: BehaviorState
    public private(set) var enteredAt: TimeInterval

    public init(state: BehaviorState = .idle, enteredAt: TimeInterval = 0) {
        self.state = state
        self.enteredAt = enteredAt
    }

    @discardableResult
    public mutating func handle(_ input: BehaviorInput, at timestamp: TimeInterval) -> BehaviorTransition {
        let old = state

        switch input {
        case .beginWander:
            if BehaviorController.wanderEntryStates.contains(state) {
                transition(to: .wander, at: timestamp)
            }
        case .arrived:
            if state == .wander || state == .travelToInterest { transition(to: .idle, at: timestamp) }
        case .beginRest:
            if BehaviorController.restEntryStates.contains(state) {
                transition(to: .sit, at: timestamp)
            }
        case .seekSleepSpot:
            if state == .sit { transition(to: .findSleepSpot, at: timestamp) }
        case .sleepSpotReached:
            if state == .findSleepSpot { transition(to: .sleep, at: timestamp) }
        case .beginStretch:
            if state != .caught && state != .dragged { transition(to: .stretch, at: timestamp) }
        case .beginInterestTravel:
            if state != .caught && state != .dragged {
                transition(to: .travelToInterest, at: timestamp)
            }
        case let .pointer(proximity):
            handlePointer(proximity, at: timestamp)
        case .catchBegan:
            if state != .dragged { transition(to: .caught, at: timestamp) }
        case .dragMoved:
            if state == .caught || state == .dragged { transition(to: .dragged, at: timestamp) }
        case .mouseReleased:
            if state == .caught || state == .dragged { transition(to: .dropped, at: timestamp) }
        case let .reaction(reaction):
            guard state != .caught && state != .dragged else { break }
            switch reaction {
            case .glance: transition(to: .observe, at: timestamp)
            case .observe: transition(to: .observe, at: timestamp)
            case .spark: transition(to: .spark, at: timestamp)
            case .work: transition(to: .work, at: timestamp)
            case .paw: transition(to: .waitingForUser, at: timestamp)
            case .smallCelebrate, .largeCelebrate: transition(to: .celebrate, at: timestamp)
            case .sad: transition(to: .sad, at: timestamp)
            case .calm: transition(to: .idle, at: timestamp)
            }
        case .meaningfulActivity:
            if state.isResting { transition(to: .wake, at: timestamp) }
        case .tick:
            settleTransientState(at: timestamp)
        }

        return BehaviorTransition(from: old, to: state, changed: old != state)
    }

    private mutating func handlePointer(_ proximity: PointerProximity, at timestamp: TimeInterval) {
        guard state != .caught && state != .dragged else { return }
        if state.isResting, proximity != .far {
            transition(to: .wake, at: timestamp)
        }
        switch proximity {
        case .far:
            if state == .lookAtPointer || state == .evadePointer {
                transition(to: .idle, at: timestamp)
            }
        case .watching, .catchable:
            transition(to: .lookAtPointer, at: timestamp)
        case .slowEvade, .fastEvade:
            transition(to: .evadePointer, at: timestamp)
        }
    }

    private mutating func settleTransientState(at timestamp: TimeInterval) {
        let age = timestamp - enteredAt
        switch state {
        case .dropped where age >= BehaviorTiming.dropped:
            transition(to: .idle, at: timestamp)
        case .wake where age >= BehaviorTiming.wake:
            transition(to: .stretch, at: timestamp)
        case .stretch where age >= BehaviorTiming.stretch:
            transition(to: .idle, at: timestamp)
        case .celebrate where age >= BehaviorTiming.celebrate:
            transition(to: .idle, at: timestamp)
        case .sad where age >= BehaviorTiming.sad:
            transition(to: .idle, at: timestamp)
        // `.waitingForUser` deliberately has no timer. Petdex classes `waiting`
        // as a steady state -- the agent is blocked on the user, so the pet keeps
        // asking until the user answers and the next hook event arrives. The 1.2s
        // hand-off to `.observe` used to mean the picture the user actually stared
        // at during an approval prompt was `review`, not the paw.
        case .spark where age >= BehaviorTiming.spark:
            transition(to: .idle, at: timestamp)
        case .observe where age >= BehaviorTiming.observe:
            // `review` is a Petdex duration state: it plays once and hands back.
            transition(to: .idle, at: timestamp)
        default:
            break
        }
    }

    private mutating func transition(to newState: BehaviorState, at timestamp: TimeInterval) {
        guard newState != state else { return }
        state = newState
        enteredAt = timestamp
    }
}
