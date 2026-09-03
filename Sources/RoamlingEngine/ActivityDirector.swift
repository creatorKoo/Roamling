// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

import Foundation
import RoamlingCore

/// One thing the runtime must do, in the order the decision made them.
///
/// The director cannot move the pet: movement, behaviour and placement belong
/// to the runtime. So it answers in effects. When the rest of the runtime comes
/// over these become direct calls and the list goes away.
public enum ActivityEffect: Equatable, Sendable {
    /// Wake the pet, because what arrived is worth getting up for.
    case cancelRest
    /// The trip is off but the window is still the one being watched.
    case settleInPlace(sourceID: String)
    case cancelRoute
    case setNextWanderAt(TimeInterval)
    case applyReaction(CompanionReaction)
    /// Ask the platform for a fresh capture near the window being watched.
    case requestLuminance(WorldRect)
}

/// Which agent the pet is watching, what it wears, and what it still owes the
/// user when it arrives.
@MainActor
public protocol ActivityDirecting: AnyObject {
    var isWatchingWindow: Bool { get }
    var activeSourceID: String? { get }
    var hint: LocationHint? { get }
    var hasArrivalReaction: Bool { get }
    /// What the pet wears while it simply sits beside a working agent.
    var sustainedReaction: CompanionReaction? { get }

    func handle(
        _ event: CompanionEvent,
        isHeldByPointer: Bool,
        isResting: Bool,
        randomUnit: Double,
        at timestamp: TimeInterval
    ) -> [ActivityEffect]

    func expireSilent(isResting: Bool, at timestamp: TimeInterval) -> [ActivityEffect]

    func resumePendingIfReady(
        isIdle: Bool,
        isHeldByPointer: Bool,
        isResting: Bool,
        randomUnit: Double,
        at timestamp: TimeInterval
    ) -> [ActivityEffect]

    func deliverArrivalReaction(isResting: Bool, at timestamp: TimeInterval) -> [ActivityEffect]

    func sustainOnSeat(isResting: Bool, at timestamp: TimeInterval) -> [ActivityEffect]
}

public extension ActivityDirecting {
    /// Whether an event without a window of its own is worth asking the
    /// platform where its window is. The query costs a synchronous round trip,
    /// so the rule lives with the decision and the call stays with the caller.
    static func wantsWindowHint(_ kind: CompanionEventKind) -> Bool {
        kind == .activityStarted || kind == .highIntensity || kind == .attentionRequired
    }
}

/// The Swift original, lifted out of `RoamlingRuntime` unchanged so that the
/// Rust port has something to be compared against. Nothing in the app calls it:
/// it is the control for `activity.txt` and for the switch-over test, and it
/// goes when the Rust one has earned it.
@MainActor
public final class SwiftActivityDirector: ActivityDirecting {
    /// Routine tool completions are useful as adapter-level evidence but do not
    /// deserve attention changes or visible reactions on their own.
    private static let routinePositiveIntensity = 0.15

    private var attention = AttentionModel()
    private var reactions = ReactionPolicy()
    private var recent: [String: CompanionEvent] = [:]
    private var pending: CompanionEvent?
    private var lastDispatchedID: String?
    private var active: String?
    private var window: LocationHint?
    private var heardAt: TimeInterval = 0
    private var sustained: CompanionReaction?
    private var arrival: CompanionReaction?

    public init() {}

    /// Sorted, because this was a dictionary and Swift randomizes dictionary
    /// order per process -- so on a tie between two agents the pet watched
    /// whichever one the launch happened to pick.
    private var live: [CompanionEvent] {
        recent.keys.sorted().compactMap { recent[$0] }
    }

    public var isWatchingWindow: Bool { active != nil && window != nil }
    public var activeSourceID: String? { active }
    public var hint: LocationHint? { window }
    public var hasArrivalReaction: Bool { arrival != nil }
    public var sustainedReaction: CompanionReaction? { sustained }

    public func handle(
        _ event: CompanionEvent,
        isHeldByPointer: Bool,
        isResting: Bool,
        randomUnit: Double,
        at timestamp: TimeInterval
    ) -> [ActivityEffect] {
        var effects: [ActivityEffect] = []
        if event.kind == .positive, event.intensity < Self.routinePositiveIntensity {
            return effects
        }

        if event.kind == .activityEnded || event.kind == .idle {
            recent.removeValue(forKey: event.sourceID)
            if attention.currentSourceID == event.sourceID {
                attention.clear(at: timestamp)
                lastDispatchedID = nil
            }
            if active == event.sourceID {
                clearActive(at: timestamp, into: &effects)
                applyReaction(.calm, isResting: isResting, at: timestamp, into: &effects)
            }
            queueNextCandidate(at: timestamp)
            return effects
        }

        recent[event.sourceID] = event
        guard let selected = attention.select(from: live, at: timestamp),
              selected.id != lastDispatchedID else { return effects }

        if isHeldByPointer {
            pending = selected
            return effects
        }
        if isResting {
            guard selected.kind.wakesRestingPet else { return effects }
            effects.append(.cancelRest)
            pending = selected
            return effects
        }
        dispatch(
            selected, isHeldByPointer: isHeldByPointer, isResting: isResting,
            randomUnit: randomUnit, at: timestamp, into: &effects
        )
        return effects
    }

    public func expireSilent(isResting: Bool, at timestamp: TimeInterval) -> [ActivityEffect] {
        var effects: [ActivityEffect] = []
        guard active != nil,
              ActivityLifetime.hasFallenSilent(lastEventAt: heardAt, now: timestamp)
        else { return effects }
        clearActive(at: timestamp, into: &effects)
        applyReaction(.calm, isResting: isResting, at: timestamp, into: &effects)
        return effects
    }

    public func resumePendingIfReady(
        isIdle: Bool,
        isHeldByPointer: Bool,
        isResting: Bool,
        randomUnit: Double,
        at timestamp: TimeInterval
    ) -> [ActivityEffect] {
        var effects: [ActivityEffect] = []
        guard isIdle, let event = pending else { return effects }
        pending = nil
        dispatch(
            event, isHeldByPointer: isHeldByPointer, isResting: isResting,
            randomUnit: randomUnit, at: timestamp, into: &effects
        )
        return effects
    }

    public func deliverArrivalReaction(
        isResting: Bool,
        at timestamp: TimeInterval
    ) -> [ActivityEffect] {
        var effects: [ActivityEffect] = []
        let reaction = arrival ?? sustained ?? .observe
        arrival = nil
        applyReaction(reaction, isResting: isResting, at: timestamp, into: &effects)
        return effects
    }

    public func sustainOnSeat(isResting: Bool, at timestamp: TimeInterval) -> [ActivityEffect] {
        var effects: [ActivityEffect] = []
        guard let sustained, sustained.isOngoing else { return effects }
        applyReaction(sustained, isResting: isResting, at: timestamp, into: &effects)
        return effects
    }

    private func dispatch(
        _ event: CompanionEvent,
        isHeldByPointer: Bool,
        isResting: Bool,
        randomUnit: Double,
        at timestamp: TimeInterval,
        into effects: inout [ActivityEffect]
    ) {
        lastDispatchedID = event.id
        if active == nil || active == event.sourceID { heardAt = timestamp }
        let reaction = reactions.reaction(
            for: event,
            context: event.context ?? .idle,
            currentBehavior: isHeldByPointer ? .caught : .idle,
            randomUnit: randomUnit,
            at: timestamp
        )

        switch event.kind {
        case .activityStarted:
            beginWatching(
                event, sustained: .observe, reaction: reaction ?? .spark,
                isResting: isResting, at: timestamp, into: &effects
            )
        case .inspecting:
            beginWatching(
                event, sustained: .observe, reaction: reaction ?? .observe,
                isResting: isResting, at: timestamp, into: &effects
            )
        case .highIntensity:
            beginWatching(
                event, sustained: .work, reaction: reaction ?? .work,
                isResting: isResting, at: timestamp, into: &effects
            )
        case .attentionRequired:
            beginWatching(
                event, sustained: .paw, reaction: reaction ?? .paw,
                isResting: isResting, at: timestamp, into: &effects
            )
        case .positive:
            if let reaction {
                applyReaction(reaction, isResting: isResting, at: timestamp, into: &effects)
            }
        case .achievement:
            clearActive(at: timestamp, into: &effects)
            applyReaction(reaction ?? .glance, isResting: isResting, at: timestamp, into: &effects)
            finishTransient(event, at: timestamp)
        case .negative:
            clearActive(at: timestamp, into: &effects)
            applyReaction(reaction ?? .sad, isResting: isResting, at: timestamp, into: &effects)
            finishTransient(event, at: timestamp)
        case .setback:
            active = event.sourceID
            heardAt = timestamp
            sustained = .observe
            effects.append(.settleInPlace(sourceID: event.sourceID))
            effects.append(.cancelRoute)
            applyReaction(reaction ?? .sad, isResting: isResting, at: timestamp, into: &effects)
        case .activityEnded, .calm, .idle:
            if event.kind == .calm, active == nil || active == event.sourceID {
                clearActive(at: timestamp, into: &effects)
                applyReaction(
                    reaction ?? .calm, isResting: isResting, at: timestamp, into: &effects
                )
            }
        }
    }

    private func beginWatching(
        _ event: CompanionEvent,
        sustained newSustained: CompanionReaction,
        reaction: CompanionReaction,
        isResting: Bool,
        at timestamp: TimeInterval,
        into effects: inout [ActivityEffect]
    ) {
        if let hint = event.locationHint {
            window = hint
            if let region = hint.approximateRegion {
                effects.append(.requestLuminance(region))
            }
        } else if active != event.sourceID {
            window = nil
        }
        active = event.sourceID
        heardAt = timestamp
        sustained = newSustained
        guard window != nil else {
            arrival = nil
            applyReaction(reaction, isResting: isResting, at: timestamp, into: &effects)
            return
        }
        arrival = reaction
    }

    private func finishTransient(_ event: CompanionEvent, at timestamp: TimeInterval) {
        recent.removeValue(forKey: event.sourceID)
        attention.clear(at: timestamp)
        queueNextCandidate(at: timestamp)
    }

    private func queueNextCandidate(at timestamp: TimeInterval) {
        guard let next = attention.select(from: live, at: timestamp) else {
            pending = nil
            return
        }
        pending = next.id == lastDispatchedID ? nil : next
    }

    private func clearActive(at timestamp: TimeInterval, into effects: inout [ActivityEffect]) {
        active = nil
        sustained = nil
        arrival = nil
        window = nil
        effects.append(.cancelRoute)
        effects.append(.setNextWanderAt(timestamp + 2.0))
    }

    private func applyReaction(
        _ reaction: CompanionReaction,
        isResting: Bool,
        at timestamp: TimeInterval,
        into effects: inout [ActivityEffect]
    ) {
        guard !isResting else { return }
        effects.append(.applyReaction(reaction))
        effects.append(.cancelRoute)
        effects.append(.setNextWanderAt(active == nil ? timestamp + 2.0 : .infinity))
    }
}
