// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

import Foundation

public enum BehaviorState: String, Codable, Hashable, Sendable {
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
    case work
    case waitingForUser
    case celebrate
    case sad
}

public enum CompanionReaction: String, Codable, Hashable, Sendable {
    case glance
    case observe
    case work
    case paw
    case smallCelebrate
    case largeCelebrate
    case sad
    case calm
}

public enum BehaviorInput: Equatable, Sendable {
    case beginWander
    case arrived
    case beginRest
    case seekSleepSpot
    case sleepSpotReached
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
}

public struct BehaviorController: Sendable {
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
            if state == .idle || state == .dropped { transition(to: .wander, at: timestamp) }
        case .arrived:
            if state == .wander || state == .travelToInterest { transition(to: .idle, at: timestamp) }
        case .beginRest:
            if state == .idle || state == .wander || state == .dropped {
                transition(to: .sit, at: timestamp)
            }
        case .seekSleepSpot:
            if state == .sit { transition(to: .findSleepSpot, at: timestamp) }
        case .sleepSpotReached:
            if state == .findSleepSpot { transition(to: .sleep, at: timestamp) }
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
        case .dropped where age >= 0.35:
            transition(to: .idle, at: timestamp)
        case .wake where age >= 0.7:
            transition(to: .stretch, at: timestamp)
        case .stretch where age >= 1.0:
            transition(to: .idle, at: timestamp)
        case .celebrate where age >= 1.4:
            transition(to: .idle, at: timestamp)
        case .sad where age >= 1.5:
            transition(to: .idle, at: timestamp)
        case .waitingForUser where age >= 1.2:
            transition(to: .observe, at: timestamp)
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
