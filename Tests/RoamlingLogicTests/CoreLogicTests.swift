// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

import Foundation
import RoamlingCore
import RoamlingEngine
import RoamlingPet

func coreLogicTests() -> [LogicTest] {
    [
        LogicTest(name: "wander entry never strands the pet in another state") {
            for state in BehaviorState.allCases {
                var behavior = BehaviorController(state: state, enteredAt: 0)
                let transition = behavior.handle(.beginWander, at: 1)
                let expected = BehaviorController.wanderEntryStates.contains(state)
                    ? BehaviorState.wander
                    : state
                try expect(
                    transition.to == expected,
                    "\(state) + beginWander produced \(transition.to)"
                )
            }
        },
        LogicTest(name: "wander entry set covers the sustained reaction states") {
            // Stop hooks park the pet in .observe (via .glance) or .work. Both
            // must be able to start roaming again, or the next route is walked
            // wearing observe frames.
            try expect(BehaviorController.wanderEntryStates.contains(.observe))
            try expect(BehaviorController.wanderEntryStates.contains(.work))
            try expect(BehaviorController.wanderEntryStates.contains(.wander))
            for blocked in [BehaviorState.caught, .dragged, .sleep, .sit, .travelToInterest] {
                try expect(
                    !BehaviorController.wanderEntryStates.contains(blocked),
                    "\(blocked) must not start a wander"
                )
            }
        },
        LogicTest(name: "an agent with no window found does not own the pet") {
            // An event can arrive with no location, and the window query can
            // come back empty. The source is still active, but there is nothing
            // to sit beside. Treating that as being on duty froze the pet
            // outright -- no stroll, and no seat the table would call worth
            // sleeping on -- until the source ended, which for an interrupted
            // session is never.
            let fixture = DirectorFixture()
            var director = PlacementDirector()
            let clear = WorldPoint(x: 900, y: 300)
            let situation = fixture.situation(
                at: 0,
                position: fixture.corner,
                sourceID: "claude",
                hintless: true,
                isStrollDue: true,
                strollCandidates: [clear]
            )
            try expect(situation.activityHint == nil, "this case needs a source with no window")
            try expect(situation.activitySourceID != nil)
            try expect(director.decide(situation) == .stroll(clear), "the pet was pinned in place")
        },
        LogicTest(name: "diagnostics keep transitions, not samples") {
            // Callers record every tick. A sampled log of a pet standing still
            // is a wall of identical lines that answers nothing, and the one
            // time it mattered the answer was in the gap between lines.
            var log = DiagnosticsLog(capacity: 4)
            try expect(log.record("rest", "waiting for user idle", at: 10))
            try expect(!log.record("rest", "waiting for user idle", at: 11))
            try expect(!log.record("rest", "waiting for user idle", at: 12))
            try expect(log.record("rest", "clear to rest", at: 13))
            try expect(log.entries.count == 2)

            // Categories do not mask each other.
            try expect(log.record("pet", "idle", at: 13))
            try expect(log.entries.count == 3)

            // A repeat after a change is a change again.
            try expect(log.record("rest", "waiting for user idle", at: 14))

            // The buffer is bounded, and the oldest entry is the one that goes.
            try expect(log.record("pet", "wander", at: 15))
            try expect(log.entries.count == 4)
            try expect(log.entries.first?.timestamp == 13)

            // Elapsed time is relative, because uptime alone says nothing.
            try expect(DiagnosticsLog().text() == "(no entries)")
            try expect(log.text().contains("0.0"))
        },
        LogicTest(name: "a watch ends on its own when the agent goes quiet") {
            // Nothing else ends one. `activityEnded` comes from a Stop hook,
            // and a hook cannot run for a session that was interrupted or
            // killed -- the ordinary way an agent ends when it is driven from a
            // GUI. A watch that never ends stops the pet roaming and lets it
            // sleep only while its seat keeps scoring clear.
            let heard = 1_000.0
            try expect(!ActivityLifetime.hasFallenSilent(lastEventAt: heard, now: heard))
            // A slow tool call is silence too, and must not end the watch.
            try expect(!ActivityLifetime.hasFallenSilent(lastEventAt: heard, now: heard + 120))
            try expect(
                ActivityLifetime.silenceBeforeExpiry > 120,
                "expiring inside a slow tool call would walk the pet off mid-build"
            )
            // A session that will never speak again has to stop owning the pet.
            try expect(ActivityLifetime.hasFallenSilent(lastEventAt: heard, now: heard + 600))
        },
        LogicTest(name: "the Rust safe zones agree with the Swift ones the app replaced") {
            // The switch-over check. The Swift planner is still here as the
            // control; when it goes, so does this test.
            var seed: UInt64 = 0x2545_F491_4F6C_DD1D
            func next(_ scale: Double = 1) -> Double {
                seed ^= seed << 13; seed ^= seed >> 7; seed ^= seed << 17
                return (Double(seed % 400_001) / 100.0 - 2000.0) * scale
            }
            var compared = 0
            for iteration in 0..<200 {
                let displays = (0...(iteration % 3)).map { index -> DisplaySnapshot in
                    let x = next(), y = next()
                    let w = abs(next()), h = abs(next())
                    let frame = WorldRect(x: x, y: y, width: w, height: h)
                    let inset = abs(next(0.02))
                    return DisplaySnapshot(
                        id: "d\(index)",
                        name: "d\(index)",
                        frame: frame,
                        visibleFrame: WorldRect(
                            x: frame.minX + inset,
                            y: frame.minY + inset,
                            width: max(0, frame.size.width - inset * 2),
                            height: max(0, frame.size.height - inset * 2)
                        ),
                        scale: 1
                    )
                }
                let world = DesktopWorldSnapshot(displays: displays)
                let swiftZones = BasicSafeZonePlanner.safeZones(in: world)
                let rustZones = RustCoreTestBridge.safeZones(in: world)
                try expect(
                    swiftZones.count == rustZones.count,
                    "zone count differs at \(iteration): \(swiftZones.count) vs \(rustZones.count)"
                )
                for (lhs, rhs) in zip(swiftZones, rustZones) {
                    try expect(lhs.frame == rhs.frame, "frame differs: \(lhs.frame) vs \(rhs.frame)")
                    try expect(lhs.score == rhs.score, "score differs: \(lhs.score) vs \(rhs.score)")
                    try expect(lhs.confidence == rhs.confidence)
                    try expect(lhs.reason == rhs.reason, "reason differs: \(lhs.reason) vs \(rhs.reason)")
                    compared += 1
                }

                let position = WorldPoint(x: next(), y: next())
                let pointer = iteration % 3 == 0 ? nil : WorldPoint(x: next(), y: next())
                let size = WorldSize(width: abs(next(0.05)), height: abs(next(0.05)))
                let swiftRest = BasicSafeZonePlanner.destination(
                    in: world, currentPosition: position,
                    pointerPosition: pointer, objectSize: size
                )
                let rustRest = RustCoreTestBridge.restDestination(
                    in: world, currentPosition: position,
                    pointerPosition: pointer, objectSize: size
                )
                try expect(
                    swiftRest == rustRest,
                    "rest destination differs at \(iteration): \(String(describing: swiftRest)) vs \(String(describing: rustRest))"
                )
            }
            try expect(compared > 400, "only \(compared) zones compared")
        },
        LogicTest(name: "the Rust attention and reactions agree with the Swift ones") {
            // Both carry state, so this is a script rather than a set of
            // independent comparisons: the dwell, the hysteresis, the revisit
            // cooldown and the reaction interval only exist across calls.
            var seed: UInt64 = 0x5D2C_913F_08A4_E7B1
            func step() -> UInt64 {
                seed ^= seed << 13; seed ^= seed >> 7; seed ^= seed << 17
                return seed
            }
            func unit() -> Double { Double(step() % 1_000_001) / 1_000_000 }
            // `Int(someUInt64)` traps when it does not fit, so reduce first.
            func pick(_ count: Int) -> Int { Int(step() % UInt64(count)) }

            let kinds: [CompanionEventKind] = [
                .activityStarted, .activityEnded, .positive, .negative, .achievement,
                .setback, .attentionRequired, .inspecting, .highIntensity, .calm, .idle
            ]
            let contexts: [UserContext] = [.working, .gaming, .watchingMedia, .browsing, .idle]

            var compared = 0
            for run in 0..<60 {
                var swiftAttention = AttentionModel()
                var swiftPolicy = ReactionPolicy()
                let rustAttention = RustCoreTestBridge.makeAttention()
                let rustReaction = RustCoreTestBridge.makeReactions()
                var now = 100.0

                for call in 0..<12 {
                    now += unit() * 6
                    let sources = 1 + pick(3)
                    let events = (0..<(1 + pick(4))).map { index in
                        CompanionEvent(
                            id: "r\(run)c\(call)e\(index)",
                            sourceID: "s\(pick(sources))",
                            sourceType: .agent,
                            timestamp: now - (unit() * 40 - 2),
                            kind: kinds[pick(kinds.count)],
                            intensity: unit(),
                            locationHint: step() % 3 != 0
                                ? LocationHint(confidence: unit()) : nil
                        )
                    }

                    let swiftPick = swiftAttention.select(from: events, at: now)
                    let rustPickID = rustAttention.select(events, now)
                    try expect(
                        swiftPick?.id == rustPickID,
                        "attention differs: \(String(describing: swiftPick?.id)) vs \(String(describing: rustPickID))"
                    )
                    try expect(
                        swiftAttention.currentSourceID == rustAttention.currentSourceID(),
                        "watched source differs at run \(run) call \(call)"
                    )

                    if step() % 5 == 0 {
                        swiftAttention.clear(at: now)
                        rustAttention.clear(now)
                        try expect(
                            swiftAttention.currentSourceID == rustAttention.currentSourceID()
                        )
                    }

                    if let chosen = swiftPick {
                        let context = contexts[pick(contexts.count)]
                        let roll = unit()
                        let held = step() % 7 == 0
                        let swiftReaction = swiftPolicy.reaction(
                            for: chosen,
                            context: context,
                            currentBehavior: held ? .caught : .idle,
                            randomUnit: roll,
                            at: now
                        )
                        let rustAnswer = rustReaction(chosen, context, held, roll, now)
                        try expect(
                            swiftReaction == rustAnswer,
                            "reaction differs: \(String(describing: swiftReaction)) vs \(String(describing: rustAnswer))"
                        )
                        compared += 1
                    }
                }
            }
            try expect(compared > 300, "only \(compared) reactions compared")
        },
        LogicTest(name: "the Rust interest planner agrees with the Swift one the app replaced") {
            var seed: UInt64 = 0x77C1_5A0E_2B93_D486
            func next(_ scale: Double = 1) -> Double {
                seed ^= seed << 13; seed ^= seed >> 7; seed ^= seed << 17
                return (Double(seed % 300_001) / 100.0 - 1500.0) * scale
            }
            func unit() -> Double {
                seed ^= seed << 13; seed ^= seed >> 7; seed ^= seed << 17
                return seed % 7 == 0 ? Double(seed % 1001) / 1000.0 : Double(seed % 61) / 1000.0
            }
            func flag() -> Bool {
                seed ^= seed << 13; seed ^= seed >> 7; seed ^= seed << 17
                return seed % 3 == 0
            }

            let swiftPlanner = SwiftInterestPlanner()
            let rustPlanner = RustCoreTestBridge.interestPlanner
            var compared = 0
            for _ in 0..<200 {
                let frame = WorldRect(
                    x: next(0.2), y: next(0.2),
                    width: 900 + abs(next(0.4)), height: 700 + abs(next(0.3))
                )
                let inset = abs(next(0.01))
                let visible = WorldRect(
                    x: frame.minX + inset, y: frame.minY + inset,
                    width: max(0, frame.size.width - inset * 2),
                    height: max(0, frame.size.height - inset * 2)
                )
                let display = DisplaySnapshot(
                    id: "d0", name: "d0", frame: frame, visibleFrame: visible, scale: 1
                )
                let region = WorldRect(
                    x: visible.minX + abs(next(0.1)), y: visible.minY + abs(next(0.1)),
                    width: 200 + abs(next(0.3)), height: 150 + abs(next(0.3))
                )
                // Every combination of what the user granted: no permissions,
                // accessibility only, capture only, both.
                let focus: FocusSnapshot? = flag() ? FocusSnapshot(
                    windowFrame: flag() ? region : nil,
                    focusedElementFrame: flag() ? WorldRect(
                        x: region.minX + abs(next(0.05)), y: region.minY + abs(next(0.05)),
                        width: abs(next(0.1)), height: abs(next(0.1))
                    ) : nil,
                    caretFrame: flag() ? WorldRect(
                        x: region.minX + abs(next(0.08)), y: region.minY + abs(next(0.08)),
                        width: abs(next(0.005)), height: 16
                    ) : nil,
                    confidence: unit()
                ) : nil
                let field: LuminanceField? = flag()
                    ? LuminanceField(
                        bounds: visible, columns: 16, rows: 12,
                        samples: (0..<192).map { _ in unit() }
                    )
                    : nil
                let world = DesktopWorldSnapshot(
                    displays: [display], focus: focus, luminance: field
                )
                let hint = LocationHint(approximateRegion: region, confidence: unit())
                let position = WorldPoint(x: next(0.3), y: next(0.3))
                let pointer: WorldPoint? = flag()
                    ? WorldPoint(x: next(0.3), y: next(0.3)) : nil
                let size = WorldSize(
                    width: 60 + abs(next(0.05)), height: 60 + abs(next(0.05))
                )

                let swiftSeat = swiftPlanner.destination(
                    for: hint, in: world, currentPosition: position,
                    pointerPosition: pointer, objectSize: size
                )
                let rustSeat = rustPlanner.destination(
                    for: hint, in: world, currentPosition: position,
                    pointerPosition: pointer, objectSize: size
                )
                try expect(
                    swiftSeat == rustSeat,
                    "destination differs: \(String(describing: swiftSeat)) vs \(String(describing: rustSeat))"
                )

                let seat = WorldPoint(x: next(0.3), y: next(0.3))
                let swiftEvaluation = swiftPlanner.evaluateSeat(
                    at: seat, for: hint, in: world, currentPosition: position,
                    pointerPosition: pointer, objectSize: size
                )
                let rustEvaluation = rustPlanner.evaluateSeat(
                    at: seat, for: hint, in: world, currentPosition: position,
                    pointerPosition: pointer, objectSize: size
                )
                try expect(
                    swiftEvaluation == rustEvaluation,
                    "seat evaluation differs: \(String(describing: swiftEvaluation)) vs \(String(describing: rustEvaluation))"
                )
                compared += 1
            }
            try expect(compared == 200, "only \(compared) scenes compared")
        },
        LogicTest(name: "the Rust nap check agrees with the Swift one the app replaced") {
            var seed: UInt64 = 0x1D87_2B4C_66E9_A031
            func next(_ scale: Double = 1) -> Double {
                seed ^= seed << 13; seed ^= seed >> 7; seed ^= seed << 17
                return (Double(seed % 200_001) / 100.0 - 1000.0) * scale
            }
            func unit() -> Double {
                seed ^= seed << 13; seed ^= seed >> 7; seed ^= seed << 17
                return seed % 7 == 0 ? Double(seed % 1001) / 1000.0 : Double(seed % 61) / 1000.0
            }
            var judged = 0
            for iteration in 0..<150 {
                let columns = 2 + (iteration % 9)
                let rows = 2 + ((iteration / 3) % 8)
                let bounds = WorldRect(
                    x: next(), y: next(),
                    width: abs(next()) + 1, height: abs(next()) + 1
                )
                let samples = (0..<(columns * rows)).map { _ in unit() }
                guard let field = LuminanceField(
                    bounds: bounds, columns: columns, rows: rows, samples: samples
                ) else { continue }
                let position = WorldPoint(x: next(), y: next())
                let size = WorldSize(width: abs(next(0.2)) + 1, height: abs(next(0.2)) + 1)
                let threshold = abs(next(0.001))
                let swiftAnswer = BasicSafeZonePlanner.napsInPlace(
                    at: position, objectSize: size, in: field, atLeast: threshold
                )
                let rustAnswer = RustCoreTestBridge.napsInPlace(
                    at: position, objectSize: size, in: field, atLeast: threshold
                )
                try expect(
                    swiftAnswer == rustAnswer,
                    "nap check differs at \(iteration): swift \(swiftAnswer) vs rust \(rustAnswer)"
                )
                // Absent capture answers no on both sides, which is the rule
                // that keeps an unvetted roaming spot from becoming a bed.
                try expect(!RustCoreTestBridge.napsInPlace(
                    at: position, objectSize: size, in: nil, atLeast: threshold
                ))
                judged += 1
            }
            try expect(judged > 100, "only \(judged) fields judged")
        },
        LogicTest(name: "the Rust activity director agrees with the Swift one") {
            // The director is main-actor isolated, like everything the runtime
            // touches. The harness's main is nonisolated and on the main thread.
            try MainActor.assumeIsolated {
                // Seven fields that only exist across events, so this is a script.
                // What it adds over the Rust fixture is the crossing: effects come
                // back as an ordered list and the order is the answer -- a setback
                // settles the seat, cancels the route and *then* reacts.
                var seed: UInt64 = 0x18B7_6C2F_D093_4AE5
                func step() -> UInt64 {
                    seed ^= seed << 13; seed ^= seed >> 7; seed ^= seed << 17
                    return seed
                }
                func unit() -> Double { Double(step() % 1_000_001) / 1_000_000 }
                func pick(_ count: Int) -> Int { Int(step() % UInt64(count)) }

                let kinds: [CompanionEventKind] = [
                    .activityStarted, .activityEnded, .positive, .negative, .achievement,
                    .setback, .attentionRequired, .inspecting, .highIntensity, .calm, .idle
                ]
                let contexts: [UserContext] = [.working, .gaming, .watchingMedia, .browsing, .idle]

                for kind in kinds {
                    try expect(
                        SwiftActivityDirector.wantsWindowHint(kind) == RustCoreTestBridge.wantsWindowHint(kind),
                        "the window-hint rule differs for \(kind)"
                    )
                }

                var compared = 0
                var counter = 0
                for run in 0..<40 {
                    let swiftDirector = SwiftActivityDirector()
                    let rustDirector = RustActivityDirector()
                    var now = 100.0 + unit() * 20
                    let sources = 1 + pick(3)

                    for call in 0..<20 {
                        now += step() % 11 == 0 ? unit() * 400 : unit() * 6
                        let isResting = step() % 6 == 0
                        let isHeld = step() % 9 == 0
                        let roll = unit()

                        let swiftEffects: [ActivityEffect]
                        let rustEffects: [ActivityEffect]
                        switch step() % 12 {
                        case 8:
                            swiftEffects = swiftDirector.expireSilent(isResting: isResting, at: now)
                            rustEffects = rustDirector.expireSilent(isResting: isResting, at: now)
                        case 9:
                            let isIdle = step() % 3 != 0
                            swiftEffects = swiftDirector.resumePendingIfReady(
                                isIdle: isIdle, isHeldByPointer: isHeld,
                                isResting: isResting, randomUnit: roll, at: now
                            )
                            rustEffects = rustDirector.resumePendingIfReady(
                                isIdle: isIdle, isHeldByPointer: isHeld,
                                isResting: isResting, randomUnit: roll, at: now
                            )
                        case 10:
                            swiftEffects = swiftDirector.deliverArrivalReaction(
                                isResting: isResting, at: now
                            )
                            rustEffects = rustDirector.deliverArrivalReaction(
                                isResting: isResting, at: now
                            )
                        case 11:
                            swiftEffects = swiftDirector.sustainOnSeat(isResting: isResting, at: now)
                            rustEffects = rustDirector.sustainOnSeat(isResting: isResting, at: now)
                        default:
                            counter += 1
                            let hasHint = step() % 3 != 0
                            let event = CompanionEvent(
                                id: "e\(counter)",
                                sourceID: "s\(pick(sources))",
                                sourceType: .agent,
                                timestamp: now - unit() * 3,
                                kind: kinds[pick(kinds.count)],
                                intensity: step() % 5 == 0 ? unit() * 0.2 : unit(),
                                context: step() % 4 != 0 ? contexts[pick(contexts.count)] : nil,
                                locationHint: hasHint
                                    ? LocationHint(
                                        approximateRegion: WorldRect(
                                            x: unit() * 1_400, y: unit() * 800,
                                            width: 200 + unit() * 400, height: 150 + unit() * 300
                                        ),
                                        confidence: unit()
                                      )
                                    : nil
                            )
                            swiftEffects = swiftDirector.handle(
                                event, isHeldByPointer: isHeld, isResting: isResting,
                                randomUnit: roll, at: now
                            )
                            rustEffects = rustDirector.handle(
                                event, isHeldByPointer: isHeld, isResting: isResting,
                                randomUnit: roll, at: now
                            )
                        }

                        try expect(
                            swiftEffects == rustEffects,
                            "effects differ at run \(run) call \(call): "
                                + "\(swiftEffects) vs \(rustEffects)"
                        )
                        try expect(
                            swiftDirector.isWatchingWindow == rustDirector.isWatchingWindow
                                && swiftDirector.activeSourceID == rustDirector.activeSourceID
                                && swiftDirector.hint == rustDirector.hint
                                && swiftDirector.hasArrivalReaction == rustDirector.hasArrivalReaction
                                && swiftDirector.sustainedReaction == rustDirector.sustainedReaction,
                            "director state differs at run \(run) call \(call)"
                        )
                        compared += 1
                    }
                }
                try expect(compared == 800, "only \(compared) calls compared")
            }
        },
        LogicTest(name: "the capability wire order matches the capability list") {
            // The tick answers with an index, so this list and
            // `PET_CAPABILITIES` in `capability.rs` are one contract. Adding a
            // capability in the middle would draw the wrong picture rather
            // than fail to compile.
            try expect(
                RustCoreTestBridge.capabilityWireOrder == PetCapability.allCases,
                "the wire order and the capability list have drifted apart"
            )
        },
        LogicTest(name: "the tuning wire order matches the key list") {
            // Swift sends the index, so this list and `TUNING_KEYS` in
            // `tuning.rs` are one contract. Adding a knob in the middle would
            // silently hand the panel another knob's range.
            try expect(
                RustCoreTestBridge.tuningWireOrder == RuntimeTuningKey.allCases,
                "the wire order and the key list have drifted apart"
            )
        },
        LogicTest(name: "the behaviour wire order matches the state list") {
            // Swift sends the index, so this list and `BEHAVIOR_STATES` in
            // `behavior.rs` are one contract. Adding a state in the middle
            // renames every state above it, and the pet would start sleeping
            // when it meant to stretch. Fail here instead.
            try expect(
                behaviorStateOrder == BehaviorState.allCases,
                "the wire order and the state list have drifted apart"
            )
        },
        LogicTest(name: "the Rust tick models agree with the Swift ones they replaced") {
            // All three carry state, so this is a script rather than a set of
            // independent comparisons -- a route is walked over hundreds of
            // ticks, closing speed only exists between two samples, and a
            // transient state only ends relative to when it began.
            //
            // The Rust differential fixture already proves the algorithms match
            // bit for bit. What this adds is the crossing: the index mappings,
            // the argument order, and the configuration that only reaches Rust
            // through a `didSet`.
            var seed: UInt64 = 0x2F94_C0B7_51DA_836E
            func step() -> UInt64 {
                seed ^= seed << 13; seed ^= seed >> 7; seed ^= seed << 17
                return seed
            }
            func unit() -> Double { Double(step() % 1_000_001) / 1_000_000 }
            func pick(_ count: Int) -> Int { Int(step() % UInt64(count)) }
            func span(_ low: Double, _ high: Double) -> Double { low + unit() * (high - low) }

            let proximities: [PointerProximity] = [
                .far, .watching, .slowEvade, .fastEvade, .catchable
            ]
            let reactions: [CompanionReaction] = [
                .glance, .observe, .spark, .work, .paw,
                .smallCelebrate, .largeCelebrate, .sad, .calm
            ]
            var compared = 0

            for run in 0..<40 {
                let start = WorldPoint(x: span(0, 1_400), y: span(0, 900))
                let configuration = MovementConfiguration(
                    maximumSpeed: span(20, 320), acceleration: 90, deceleration: 115
                )
                var swiftMovement = MovementController(
                    position: start, velocity: .zero, configuration: configuration
                )
                let rustMovement = RustMovement(
                    position: start, velocity: .zero, configuration: configuration
                )

                let startState = BehaviorState.allCases[pick(BehaviorState.allCases.count)]
                let enteredAt = span(0, 100)
                var swiftBehavior = BehaviorController(state: startState, enteredAt: enteredAt)
                let rustBehavior = RustBehavior(state: startState, enteredAt: enteredAt)

                let tuning = RuntimeTuning(
                    walkingSpeed: span(20, 320),
                    pointerAwarenessDistance: span(140, 360),
                    catchArmDistance: span(40, 140),
                    catchApproachSpeed: span(150, 900)
                )
                var swiftPointer = PointerInteractionModel(
                    configuration: tuning.pointerConfiguration
                )
                let rustPointer = RustPointerModel(configuration: tuning.pointerConfiguration)

                var now = enteredAt
                var pet = start
                var cursor = WorldPoint(x: pet.x + span(-400, 400), y: pet.y + span(-400, 400))

                for tick in 0..<60 {
                    now += 1.0 / 60.0

                    if tick % 17 == 0 {
                        let waypoints = (0..<(1 + pick(3))).map { _ in
                            WorldPoint(x: span(0, 1_400), y: span(0, 900))
                        }
                        swiftMovement.setRoute(waypoints)
                        rustMovement.setRoute(waypoints)
                    }
                    if tick % 23 == 0 {
                        let speed = span(20, 320)
                        swiftMovement.configuration.maximumSpeed = speed
                        rustMovement.maximumSpeed = speed
                    }
                    let swiftUpdate = swiftMovement.update(deltaTime: 1.0 / 60.0)
                    let rustUpdate = rustMovement.update(deltaTime: 1.0 / 60.0)
                    try expect(
                        swiftUpdate == rustUpdate,
                        "movement differs at run \(run) tick \(tick): "
                            + "\(swiftUpdate) vs \(rustUpdate)"
                    )
                    try expect(
                        swiftMovement.hasRoute == rustMovement.hasRoute
                            && swiftMovement.destination == rustMovement.destination,
                        "route state differs at run \(run) tick \(tick)"
                    )
                    pet = swiftUpdate.position

                    // The cursor closes on the pet, so the run walks through
                    // the proximity bands instead of sampling them at random.
                    cursor = WorldPoint(
                        x: cursor.x + (pet.x - cursor.x) * unit() * 0.6 + span(-8, 8),
                        y: cursor.y + (pet.y - cursor.y) * unit() * 0.6 + span(-8, 8)
                    )
                    if step() % 29 == 0 {
                        swiftPointer.reset()
                        rustPointer.reset()
                    }
                    if step() % 31 == 0 {
                        let replacement = RuntimeTuning(
                            walkingSpeed: span(20, 320),
                            pointerAwarenessDistance: span(140, 360),
                            catchArmDistance: span(40, 140)
                        ).pointerConfiguration
                        swiftPointer.configuration = replacement
                        rustPointer.configuration = replacement
                    }
                    let swiftDecision = swiftPointer.evaluate(
                        pointer: cursor, pet: pet, timestamp: now
                    )
                    let rustDecision = rustPointer.evaluate(
                        pointer: cursor, pet: pet, timestamp: now
                    )
                    try expect(
                        swiftDecision == rustDecision,
                        "pointer differs at run \(run) tick \(tick): "
                            + "\(swiftDecision) vs \(rustDecision)"
                    )

                    let code = pick(14)
                    let input: BehaviorInput = switch code {
                    case 0: .beginWander
                    case 1: .arrived
                    case 2: .beginRest
                    case 3: .seekSleepSpot
                    case 4: .sleepSpotReached
                    case 5: .beginStretch
                    case 6: .beginInterestTravel
                    case 7: .pointer(proximities[pick(proximities.count)])
                    case 8: .catchBegan
                    case 9: .dragMoved
                    case 10: .mouseReleased
                    case 11: .reaction(reactions[pick(reactions.count)])
                    case 12: .meaningfulActivity
                    default: .tick
                    }
                    let swiftTransition = swiftBehavior.handle(input, at: now)
                    let rustTransition = rustBehavior.handle(input, at: now)
                    try expect(
                        swiftTransition == rustTransition,
                        "behaviour differs at run \(run) tick \(tick) on \(input): "
                            + "\(swiftTransition) vs \(rustTransition)"
                    )
                    try expect(
                        swiftBehavior.state == rustBehavior.state
                            && swiftBehavior.enteredAt == rustBehavior.enteredAt,
                        "behaviour state differs at run \(run) tick \(tick)"
                    )
                    compared += 1
                }
            }
            try expect(compared == 2_400, "only \(compared) ticks compared")
        },
        LogicTest(name: "the Rust director agrees with the Swift one it replaced") {
            // The director carries a seat, a trip and the time of its last
            // review, so this is a trajectory: the pet walks toward what it was
            // told, which is the only way arrival and the review beat happen.
            //
            // What this adds over the Rust fixture is the crossing. The grid
            // and the display list are pushed only when they change, and a
            // missed push would hand the director a world the pet is no longer
            // standing in.
            var seed: UInt64 = 0x4B62_D19E_37F0_A85C
            func step() -> UInt64 {
                seed ^= seed << 13; seed ^= seed >> 7; seed ^= seed << 17
                return seed
            }
            func unit() -> Double { Double(step() % 1_000_001) / 1_000_000 }
            func pick(_ count: Int) -> Int { Int(step() % UInt64(count)) }
            func span(_ low: Double, _ high: Double) -> Double { low + unit() * (high - low) }

            let display = DisplaySnapshot(
                id: "d0", name: "d0",
                frame: WorldRect(x: 0, y: 0, width: 1_600, height: 1_000),
                visibleFrame: WorldRect(x: 0, y: 0, width: 1_600, height: 1_000),
                scale: 1
            )
            let region = WorldRect(x: 900, y: 500, width: 400, height: 300)
            let hint = LocationHint(approximateRegion: region, confidence: 0.8)
            let size = WorldSize(width: 60, height: 60)

            /// Flat background with one busy rectangle, because emptiness
            /// scores flatness: uniform noise reads as busy everywhere and the
            /// director would never find anywhere worth sitting.
            func makeField(busyColumn: Int, busyRow: Int) -> LuminanceField? {
                let columns = 16, rows = 12
                let samples = (0..<(columns * rows)).map { index -> Double in
                    let column = index % columns
                    let row = index / columns
                    let busy = abs(column - busyColumn) < 3 && abs(row - busyRow) < 3
                    return busy ? ((column + row) % 2 == 0 ? 0.05 : 0.95) : 0.5
                }
                return LuminanceField(
                    bounds: display.visibleFrame, columns: columns, rows: rows, samples: samples
                )
            }

            var compared = 0
            for run in 0..<30 {
                var swiftDirector = PlacementDirector(planner: RustCoreTestBridge.interestPlanner)
                let rustDirector = RustPlacement()
                var pet = WorldPoint(x: span(60, 1_500), y: span(60, 900))
                var now = 1_000.0 + unit() * 20
                var field = makeField(busyColumn: pick(16), busyRow: pick(12))

                for tick in 0..<40 {
                    now += tick % 5 == 0 ? span(0.4, 1.4) : 1.0 / 30.0
                    // The capture refreshing is exactly the event the push
                    // cache has to notice.
                    if step() % 9 == 0 {
                        field = step() % 5 == 0
                            ? nil
                            : makeField(busyColumn: pick(16), busyRow: pick(12))
                    }
                    let hasSource = step() % 6 != 0
                    let world = DesktopWorldSnapshot(
                        displays: [display],
                        focus: step() % 3 == 0
                            ? FocusSnapshot(
                                windowFrame: region,
                                focusedElementFrame: nil,
                                caretFrame: WorldRect(
                                    x: pet.x - 1, y: pet.y - 8, width: 2, height: 16
                                ),
                                confidence: 0.9
                              )
                            : nil,
                        luminance: field
                    )
                    let situation = PetSituation(
                        timestamp: now,
                        world: world,
                        position: pet,
                        objectSize: size,
                        pointerPosition: step() % 3 == 0
                            ? WorldPoint(x: span(0, 1_600), y: span(0, 1_000)) : nil,
                        walkingSpeed: 160,
                        isPointerOwned: step() % 17 == 0,
                        isPointerWatching: step() % 11 == 0,
                        isEvading: step() % 19 == 0,
                        isWalking: step() % 7 == 0,
                        isResting: step() % 13 == 0,
                        activitySourceID: hasSource ? "s0" : nil,
                        activityHint: hasSource ? hint : nil,
                        userIdleDuration: unit() * 200,
                        idleBeforeRest: step() % 3 == 0 ? 75 : .infinity,
                        isRoamingEnabled: step() % 5 != 0,
                        isStrollDue: step() % 12 == 0,
                        strollCandidates: (0..<pick(4)).map { _ in
                            WorldPoint(x: span(0, 1_600), y: span(0, 1_000))
                        }
                    )

                    let swiftIntent = swiftDirector.decide(situation)
                    let rustIntent = rustDirector.decide(situation)
                    try expect(
                        swiftIntent == rustIntent,
                        "intent differs at run \(run) tick \(tick): "
                            + "\(swiftIntent) vs \(rustIntent)"
                    )
                    try expect(
                        swiftDirector.isSeated == rustDirector.isSeated
                            && swiftDirector.isTravelling == rustDirector.isTravelling,
                        "director state differs at run \(run) tick \(tick)"
                    )

                    if case let .travel(destination, _) = swiftIntent {
                        pet = destination.point
                    } else if case let .stroll(point) = swiftIntent {
                        pet = point
                    } else if case let .escape(point) = swiftIntent {
                        pet = point
                    }
                    if step() % 21 == 0 {
                        let owner: String? = step() % 2 == 0 ? "s0" : nil
                        swiftDirector.settleInPlace(sourceID: owner, at: now)
                        rustDirector.settleInPlace(sourceID: owner, at: now)
                    }
                    compared += 1
                }
            }
            try expect(compared == 1_200, "only \(compared) ticks compared")
        },
        LogicTest(name: "two seats of equal score pick the same one every launch") {
            // Mirrored seats either side of a centred window score identically
            // and sit the same distance from the pet. The candidates used to be
            // a dictionary, whose order Swift randomizes per process, so the pet
            // sat left 24 launches out of 40 and right the other 16.
            let display = DisplaySnapshot(
                id: "d0", name: "d0",
                frame: WorldRect(x: 0, y: 0, width: 1600, height: 1000),
                visibleFrame: WorldRect(x: 0, y: 0, width: 1600, height: 1000),
                scale: 1
            )
            let world = DesktopWorldSnapshot(displays: [display])
            let hint = LocationHint(
                approximateRegion: WorldRect(x: 600, y: 300, width: 400, height: 300),
                confidence: 0.8
            )
            let first = try require(BasicInterestPositionPlanner.destination(
                for: hint, in: world,
                currentPosition: WorldPoint(x: 800, y: 900),
                pointerPosition: nil,
                objectSize: WorldSize(width: 96, height: 104)
            ))
            // The first candidate proposed is the seat outside the left edge,
            // and with nothing to separate them that is the one that wins.
            try expect(first.point.x < 600, "expected the left seat, got \(first.point)")
            for _ in 0..<50 {
                let again = try require(BasicInterestPositionPlanner.destination(
                    for: hint, in: world,
                    currentPosition: WorldPoint(x: 800, y: 900),
                    pointerPosition: nil,
                    objectSize: WorldSize(width: 96, height: 104)
                ))
                try expect(again == first)
            }
        },
        LogicTest(name: "two routes of equal cost pick the same one every launch") {
            // This failed about half the time before the fix, differently each
            // run: Dijkstra walked a Set, and Swift randomizes Set order per
            // process, so a symmetric desk sent the pet left or right by coin
            // flip on the same journey.
            func square(_ id: String, _ x: Double, _ y: Double) -> DisplaySnapshot {
                DisplaySnapshot(
                    id: id,
                    name: id,
                    frame: WorldRect(x: x, y: y, width: 100, height: 100),
                    visibleFrame: WorldRect(x: x, y: y, width: 100, height: 100),
                    scale: 1
                )
            }
            // S and T face each other across a gap of 300; A and B are mirrored
            // detours costing 283.84 each, so the two best routes tie exactly.
            let topology = DisplayTopology(displays: [
                square("S", 0, 0), square("T", 0, -400),
                square("A", -200, -200), square("B", 200, -200)
            ])
            let route = topology.route(
                from: WorldPoint(x: 50, y: 50),
                to: WorldPoint(x: 50, y: -350)
            )
            try expect(
                route.displayIDs == ["S", "A", "T"],
                "expected the lower display index to win, got \(route.displayIDs)"
            )
            // Same answer however many times it is asked, within a run too.
            for _ in 0..<50 {
                let again = topology.route(
                    from: WorldPoint(x: 50, y: 50),
                    to: WorldPoint(x: 50, y: -350)
                )
                try expect(again.displayIDs == route.displayIDs)
                try expect(again.waypoints == route.waypoints)
            }
        },
        LogicTest(name: "the tuning panel cannot offer a value the model refuses") {
            // The panel used to restate these bounds, and one of them drifted:
            // it capped catchArmDistance at 140 while the model allowed up to
            // whatever pointer awareness was, which reaches 360.
            var tuning = RuntimeTuning.standard
            for key in RuntimeTuningKey.allCases {
                let limits = tuning.limits(for: key)
                try expect(limits.lowerBound < limits.upperBound, "\(key) has an empty range")

                var atCeiling = tuning
                atCeiling[key] = limits.upperBound
                try expect(
                    atCeiling.normalized[key] == limits.upperBound,
                    "\(key): the model clamped its own upper bound away"
                )

                var atFloor = tuning
                atFloor[key] = limits.lowerBound
                try expect(
                    atFloor.normalized[key] == limits.lowerBound,
                    "\(key): the model clamped its own lower bound away"
                )

                var beyond = tuning
                beyond[key] = limits.upperBound + 1000
                try expect(
                    beyond.normalized[key] <= limits.upperBound,
                    "\(key): the model accepted a value past the offered range"
                )
            }

            // And the moving one actually moves.
            tuning.pointerAwarenessDistance = 360
            tuning = tuning.normalized
            try expect(tuning.limits(for: .catchArmDistance).upperBound == 360)
            tuning.pointerAwarenessDistance = 140
            tuning = tuning.normalized
            try expect(tuning.limits(for: .catchArmDistance).upperBound == 140)
        },
        LogicTest(name: "a sleeping pet is woken only for what the user would want to see") {
            // An agent emits an event per tool call. If each of them woke the
            // pet it could doze for one beat and never longer.
            for routine in [CompanionEventKind.activityStarted, .inspecting, .highIntensity, .positive, .calm] {
                try expect(!routine.wakesRestingPet, "\(routine) is the work the pet is sitting next to")
            }
            // The end of a watch is handled by the watch ending, not by a wake.
            try expect(!CompanionEventKind.activityEnded.wakesRestingPet)
            try expect(!CompanionEventKind.idle.wakesRestingPet)
            // A result, or a request for the user, is what a nap is worth losing for.
            for outcome in [CompanionEventKind.attentionRequired, .achievement, .negative, .setback] {
                try expect(outcome.wakesRestingPet, "\(outcome) is what the user wants shown")
            }
        },
        LogicTest(name: "a walking pet can still fall asleep") {
            // Rest is gated on how long the *user* has been idle, never on the
            // pet holding still, and the tick chain checks rest before roaming.
            // So a pet mid-stroll has to be able to stop and sit, or raising the
            // pause between walks would quietly mean it never sleeps again.
            try expect(
                BehaviorController.restEntryStates.contains(.wander),
                "a stroll would block rest for as long as the pause is set to"
            )
            var behavior = BehaviorController(state: .wander, enteredAt: 0)
            behavior.handle(.beginRest, at: 80)
            try expect(behavior.state == .sit, "walking pet refused to sit")
            behavior.handle(.seekSleepSpot, at: 82.4)
            behavior.handle(.sleepSpotReached, at: 84)
            try expect(behavior.state == .sleep)
        },
        LogicTest(name: "the nap threshold is tunable and stays bounded") {
            try expectNear(
                RuntimeTuning.standard.restConfiguration.idleBeforeRest,
                RestConfiguration.standard.idleBeforeRest
            )
            let patient = RuntimeTuning(idleBeforeRest: 300)
            try expectNear(patient.restConfiguration.idleBeforeRest, 300)
            // The panel's own range is the outer bound; nothing downstream may
            // widen it into a pet that naps after two seconds.
            try expectNear(RuntimeTuning(idleBeforeRest: 1).idleBeforeRest, 15)
            try expectNear(RuntimeTuning(idleBeforeRest: 9_000).idleBeforeRest, 600)
        },
        LogicTest(name: "observing pet roams and settles back to idle") {
            var behavior = BehaviorController(state: .idle, enteredAt: 0)
            behavior.handle(.reaction(.glance), at: 1)
            try expect(behavior.state == .observe)

            behavior.handle(.beginWander, at: 2)
            try expect(behavior.state == .wander)

            behavior.handle(.arrived, at: 3)
            try expect(behavior.state == .idle)
        },
        LogicTest(name: "coordinate transform round-trips complex layouts") {
            let frames = [
                WorldRect(x: 0, y: 0, width: 1920, height: 1080),
                WorldRect(x: 1920, y: -200, width: 1280, height: 1024),
                WorldRect(x: -800, y: 1080, width: 800, height: 900)
            ]
            let space = DesktopCoordinateSpace.fromAppKitFrames(frames)
            try expect(space.worldTop == 1980)
            let point = WorldPoint(x: 2030, y: -80)
            try expect(space.pointToAppKit(space.pointFromAppKit(point)) == point)
            let rect = WorldRect(x: -750, y: 1200, width: 500, height: 400)
            try expect(space.rectToAppKit(space.rectFromAppKit(rect)) == rect)
        },
        LogicTest(name: "CoreGraphics rects fold in from the primary display anchor") {
            // Secondary display sits above the primary, so the world plane's top
            // is not the primary display's top. CGWindowList and accessibility
            // both measure down from the primary, and the gap between the two
            // anchors is exactly what this conversion has to absorb.
            let space = DesktopCoordinateSpace.fromAppKitFrames([
                WorldRect(x: 0, y: 0, width: 1_200, height: 800),
                WorldRect(x: 0, y: 800, width: 1_200, height: 600)
            ])
            try expect(space.worldTop == 1_400)

            let onPrimary = space.rectFromCoreGraphics(
                WorldRect(x: 100, y: 50, width: 400, height: 300),
                primaryTop: 800
            )
            try expect(onPrimary == WorldRect(x: 100, y: 650, width: 400, height: 300))

            // A window on the upper display reports negative CoreGraphics y and
            // has to land near the top of the world plane, not off it.
            let abovePrimary = space.rectFromCoreGraphics(
                WorldRect(x: 0, y: -500, width: 200, height: 100),
                primaryTop: 800
            )
            try expect(abovePrimary == WorldRect(x: 0, y: 100, width: 200, height: 100))
        },
        LogicTest(name: "world clamp accounts for pet size") {
            let display = testDisplay("A", WorldRect(x: 0, y: 0, width: 1000, height: 800))
            let world = DesktopWorldSnapshot(displays: [display])
            let clamped = world.clamp(
                WorldPoint(x: -300, y: 900),
                objectSize: WorldSize(width: 100, height: 120)
            )
            try expect(clamped == WorldPoint(x: 50, y: 740))
        },
        LogicTest(name: "partial horizontal seam uses overlap midpoint") {
            let left = testDisplay("left", WorldRect(x: 0, y: 0, width: 1000, height: 800))
            let right = testDisplay("right", WorldRect(x: 1000, y: 300, width: 700, height: 900))
            let portal = DisplayTopology(displays: [left, right]).portal(from: left, to: right)
            try expect(portal.exit == WorldPoint(x: 1000, y: 550))
            try expect(portal.entry == portal.exit)
            try expect(portal.gap == 0)
        },
        LogicTest(name: "stacked displays use vertical seam") {
            let top = testDisplay("top", WorldRect(x: -300, y: -900, width: 1200, height: 900))
            let bottom = testDisplay("bottom", WorldRect(x: 0, y: 0, width: 1000, height: 800))
            let portal = DisplayTopology(displays: [top, bottom]).portal(from: top, to: bottom)
            try expect(portal.exit == WorldPoint(x: 450, y: 0))
            try expect(portal.entry == portal.exit)
        },
        LogicTest(name: "route prefers connected intermediate display") {
            let a = testDisplay("A", WorldRect(x: 0, y: 0, width: 100, height: 100))
            let b = testDisplay("B", WorldRect(x: 100, y: 0, width: 100, height: 100))
            let c = testDisplay("C", WorldRect(x: 200, y: 0, width: 100, height: 100))
            let route = DisplayTopology(displays: [a, b, c]).route(
                from: WorldPoint(x: 30, y: 50),
                to: WorldPoint(x: 270, y: 60)
            )
            try expect(route.displayIDs == ["A", "B", "C"])
            try expect(route.waypoints.last == WorldPoint(x: 270, y: 60))
            try expect(route.waypoints.contains(WorldPoint(x: 100, y: 50)))
            try expect(route.waypoints.contains(WorldPoint(x: 200, y: 50)))
        },
        LogicTest(name: "disconnected route contains distinct exit and entry") {
            let a = testDisplay("A", WorldRect(x: 0, y: 0, width: 200, height: 200))
            let b = testDisplay("B", WorldRect(x: 500, y: 400, width: 200, height: 200))
            let route = DisplayTopology(displays: [a, b]).route(
                from: WorldPoint(x: 100, y: 100),
                to: WorldPoint(x: 600, y: 500)
            )
            try expect(route.displayIDs == ["A", "B"])
            try expect(route.waypoints.count >= 3)
            try expect(route.waypoints[0] != route.waypoints[1])
        },
        LogicTest(name: "edge pressure chooses the connected display in its direction") {
            let center = testDisplay("center", WorldRect(x: 0, y: 0, width: 1920, height: 1080))
            let left = testDisplay("left", WorldRect(x: -1920, y: 180, width: 1920, height: 1080))
            let upper = testDisplay("upper", WorldRect(x: 520, y: -900, width: 1400, height: 900))
            let topology = DisplayTopology(displays: [center, left, upper])
            let objectSize = WorldSize(width: 96, height: 104)

            let leftRoute = try require(topology.evadeTransition(
                from: WorldPoint(x: 48, y: 540),
                direction: WorldVector(dx: -1, dy: 0),
                objectSize: objectSize
            ))
            try expect(leftRoute.displayIDs == ["center", "left"])
            let leftTarget = try require(leftRoute.waypoints.last)
            try expect(leftTarget.x < 0)
            try expect(left.visibleFrame.insetBy(dx: 48, dy: 52).contains(leftTarget))

            let upperRoute = try require(topology.evadeTransition(
                from: WorldPoint(x: 900, y: 52),
                direction: WorldVector(dx: 0, dy: -1),
                objectSize: objectSize
            ))
            try expect(upperRoute.displayIDs == ["center", "upper"])
            let upperTarget = try require(upperRoute.waypoints.last)
            try expect(upperTarget.y < 0)
            try expect(upper.visibleFrame.insetBy(dx: 48, dy: 52).contains(upperTarget))
        },
        LogicTest(name: "edge pressure never teleports across a display gap") {
            let center = testDisplay("center", WorldRect(x: 0, y: 0, width: 1000, height: 800))
            let gapped = testDisplay("gapped", WorldRect(x: 1040, y: 0, width: 1000, height: 800))
            let route = DisplayTopology(displays: [center, gapped]).evadeTransition(
                from: WorldPoint(x: 952, y: 400),
                direction: WorldVector(dx: 1, dy: 0),
                objectSize: WorldSize(width: 96, height: 104)
            )
            try expect(route == nil)
        },
        LogicTest(name: "movement caps speed and arrives without overshoot") {
            var controller = MovementController(
                position: .zero,
                configuration: MovementConfiguration(
                    maximumSpeed: 40,
                    acceleration: 80,
                    deceleration: 100,
                    arrivalRadius: 0.5
                )
            )
            controller.setRoute([WorldPoint(x: 100, y: 0)])
            var maximumSpeed = 0.0
            for _ in 0..<500 {
                let update = controller.update(deltaTime: 1 / 60)
                maximumSpeed = max(maximumSpeed, update.velocity.length)
                if update.reachedDestination { break }
            }
            try expect(maximumSpeed <= 40.0001)
            try expectNear(controller.position.x, 100)
            try expectNear(controller.position.y, 0)
            try expect(!controller.hasRoute)
            try expect(controller.velocity == .zero)
        },
        LogicTest(name: "movement consumes multiple waypoints") {
            var controller = MovementController(position: .zero)
            controller.setRoute([
                WorldPoint(x: 10, y: 0),
                WorldPoint(x: 10, y: 10),
                WorldPoint(x: 20, y: 10)
            ])
            for _ in 0..<600 where controller.hasRoute {
                _ = controller.update(deltaTime: 1 / 60)
            }
            try expect(controller.position == WorldPoint(x: 20, y: 10))
        },
        LogicTest(name: "pointer thresholds yield capped escape") {
            var model = PointerInteractionModel()
            let pet = WorldPoint.zero
            try expect(model.evaluate(pointer: WorldPoint(x: 250, y: 0), pet: pet, timestamp: 0).proximity == .far)
            try expect(model.evaluate(pointer: WorldPoint(x: 150, y: 0), pet: pet, timestamp: 1).proximity == .watching)
            let slow = model.evaluate(pointer: WorldPoint(x: 80, y: 0), pet: pet, timestamp: 2)
            try expect(slow.proximity == .slowEvade)
            try expect(slow.escapeVelocity.dx < 0)
            try expectNear(slow.escapeVelocity.length, 74)
            let fast = model.evaluate(pointer: WorldPoint(x: 48, y: 0), pet: pet, timestamp: 3)
            try expect(fast.proximity == .fastEvade)
            try expectNear(fast.escapeVelocity.length, 138)
        },
        LogicTest(name: "fast closing approach arms catch") {
            var model = PointerInteractionModel()
            _ = model.evaluate(pointer: WorldPoint(x: 500, y: 0), pet: .zero, timestamp: 0)
            let decision = model.evaluate(pointer: WorldPoint(x: 40, y: 0), pet: .zero, timestamp: 0.1)
            try expect(decision.proximity == .catchable)
            try expect(decision.shouldArmCatch)
            try expect(decision.kinematics.closingSpeed > 320)
        },
        LogicTest(name: "fast lateral motion does not arm catch") {
            var model = PointerInteractionModel()
            _ = model.evaluate(pointer: WorldPoint(x: -40, y: 0), pet: .zero, timestamp: 0)
            let decision = model.evaluate(pointer: WorldPoint(x: 40, y: 0), pet: .zero, timestamp: 0.1)
            try expect(decision.proximity == .fastEvade)
            try expectNear(decision.kinematics.closingSpeed, 0)
        },
        LogicTest(name: "standard tuning makes trackpad catch forgiving but intentional") {
            let tuning = RuntimeTuning.standard
            var fastApproach = PointerInteractionModel(configuration: tuning.pointerConfiguration)
            _ = fastApproach.evaluate(pointer: WorldPoint(x: 160, y: 0), pet: .zero, timestamp: 0)
            let catchable = fastApproach.evaluate(
                pointer: WorldPoint(x: 70, y: 0),
                pet: .zero,
                timestamp: 0.15
            )
            try expect(catchable.proximity == .catchable)

            var slowApproach = PointerInteractionModel(configuration: tuning.pointerConfiguration)
            _ = slowApproach.evaluate(pointer: WorldPoint(x: 110, y: 0), pet: .zero, timestamp: 0)
            let evade = slowApproach.evaluate(
                pointer: WorldPoint(x: 70, y: 0),
                pet: .zero,
                timestamp: 0.3
            )
            try expect(evade.proximity == .slowEvade)
            try expect(!evade.shouldArmCatch)
        },
        LogicTest(name: "MVP tuning clamps unsafe values and varies idle pause") {
            let tuning = RuntimeTuning(
                walkingSpeed: 500,
                wanderPause: -1,
                crossDisplayWanderChance: 2,
                pointerAwarenessDistance: 80,
                catchArmDistance: 900,
                catchApproachSpeed: 10,
                catchWindow: 3,
                hitRegionScale: 4
            )
            try expect(tuning.walkingSpeed == 320)
            try expect(tuning.wanderPause == 2)
            try expect(tuning.crossDisplayWanderChance == 1)
            try expect(tuning.pointerAwarenessDistance == 140)
            try expect(tuning.catchArmDistance == 140)
            try expect(tuning.catchApproachSpeed == 150)
            try expect(tuning.catchWindow == 1.2)
            try expect(tuning.hitRegionScale == 1.3)
            // The pause was promoted from 12 s to 40 after living with the pet,
            // so the spread it maps to moved with it.
            try expectNear(RuntimeTuning.standard.wanderDelay(randomUnit: 0), 28)
            try expectNear(RuntimeTuning.standard.wanderDelay(randomUnit: 1), 58)
        },
        LogicTest(name: "saved tuning survives a field being added later") {
            // A blob written before gaitCadence existed. The synthesized decoder
            // rejected it and threw away everything the user had tuned.
            let legacy = Data(#"""
            {
              "walkingSpeed": 80,
              "wanderPause": 9,
              "crossDisplayWanderChance": 0.3,
              "pointerAwarenessDistance": 200,
              "catchArmDistance": 60,
              "catchApproachSpeed": 400,
              "catchWindow": 0.4,
              "hitRegionScale": 1.1
            }
            """#.utf8)
            let decoded = try JSONDecoder().decode(RuntimeTuning.self, from: legacy)
            try expectNear(decoded.walkingSpeed, 80)
            try expectNear(decoded.wanderPause, 9)
            try expectNear(decoded.hitRegionScale, 1.1)
            try expectNear(decoded.gaitCadence, RuntimeTuning.standard.gaitCadence)
            try expectNear(decoded.idleBeforeRest, RuntimeTuning.standard.idleBeforeRest)

            // Bounds still apply to whatever was on disk.
            let outOfRange = Data(#"{"walkingSpeed": 5000}"#.utf8)
            let clamped = try JSONDecoder().decode(RuntimeTuning.self, from: outOfRange)
            try expectNear(clamped.walkingSpeed, 320)
            try expectNear(clamped.wanderPause, RuntimeTuning.standard.wanderPause)
        },
        LogicTest(name: "visual emptiness separates flat regions from busy ones") {
            let bounds = WorldRect(x: 0, y: 0, width: 800, height: 400)
            let columns = 40
            let rows = 20

            func field(_ make: (Int, Int) -> Double) throws -> LuminanceField {
                var samples: [Double] = []
                for row in 0..<rows {
                    for column in 0..<columns { samples.append(make(column, row)) }
                }
                return try require(LuminanceField(
                    bounds: bounds, columns: columns, rows: rows, samples: samples
                ))
            }

            let seat = WorldRect(x: 100, y: 100, width: 200, height: 120)

            // Flat wallpaper is the ideal seat.
            let flat = try field { _, _ in 0.62 }
            try expectNear(try require(VisualEmptiness.score(of: seat, in: flat)), 1)

            // A desktop gradient wallpaper drifts across the whole screen, so
            // one cell differs from its neighbour by very little. It has to
            // stay comfortable enough to sit on.
            let gradient = try field { column, row in
                0.3 + 0.2 * Double(column) / Double(columns) + 0.15 * Double(row) / Double(rows)
            }
            let gradientScore = try require(VisualEmptiness.score(of: seat, in: gradient))
            try expect(gradientScore > 0.6, "smooth gradient scored \(gradientScore)")

            // Body text survives downsampling as a low-contrast wash, not as
            // alternating black and white. This case is the one that matters:
            // scoring it as empty is what parks the pet on the user's work.
            let text = try field { column, row in
                (column + row).isMultiple(of: 3) ? 0.78 : 0.86
            }
            let textScore = try require(VisualEmptiness.score(of: seat, in: text))
            try expect(
                textScore < BasicInterestPositionPlanner.holdEmptiness,
                "downsampled text scored \(textScore) and would read as an empty seat"
            )

            // Alternating high-contrast cells stand in for dense UI.
            let busy = try field { column, row in (column + row).isMultiple(of: 2) ? 0.05 : 0.95 }
            let busyScore = try require(VisualEmptiness.score(of: seat, in: busy))
            try expect(busyScore < 0.1, "dense region scored \(busyScore)")
            try expect(busyScore < textScore)
            try expect(textScore < gradientScore)
        },
        LogicTest(name: "a stroll rejects destinations that sit on content") {
            // The pet spends most of its life roaming, not watching an agent,
            // and that walk used to ignore the screen entirely.
            let bounds = WorldRect(x: 0, y: 0, width: 800, height: 400)
            let columns = 40
            let rows = 20
            var samples: [Double] = []
            for row in 0..<rows {
                for column in 0..<columns {
                    let text = column < columns / 2
                    samples.append(text && (column + row).isMultiple(of: 3) ? 0.78 : 0.86)
                }
            }
            let field = try require(LuminanceField(
                bounds: bounds, columns: columns, rows: rows, samples: samples
            ))
            let pet = WorldSize(width: 96, height: 104)
            let onText = WorldPoint(x: 150, y: 200)
            let clear = WorldPoint(x: 620, y: 200)
            let alsoClear = WorldPoint(x: 700, y: 120)

            func pick(_ points: [WorldPoint]) -> WorldPoint? {
                VisualEmptiness.firstComfortable(
                    among: points, objectSize: pet, in: field, atLeast: 0.55
                )
            }

            // Offered the text first, it walks past it.
            try expect(pick([onText, clear]) == clear)
            // Offered a clear spot first, it takes that one instead of shopping
            // around, which is what keeps roaming from looking calculated.
            try expect(pick([alsoClear, clear]) == alsoClear)
            // When every option is bad the least bad one still wins, because
            // refusing to move is not an answer.
            try expect(pick([onText]) == onText)
            // Nothing judgeable means no opinion, so the caller keeps its pick.
            try expect(pick([WorldPoint(x: 4_000, y: 4_000)]) == nil)
        },
        LogicTest(name: "visual emptiness refuses to guess from too little overlap") {
            let bounds = WorldRect(x: 0, y: 0, width: 400, height: 400)
            let field = try require(LuminanceField(
                bounds: bounds,
                columns: 20,
                rows: 20,
                samples: Array(repeating: 0.5, count: 400)
            ))

            // Entirely outside the captured region.
            try expect(VisualEmptiness.score(
                of: WorldRect(x: 900, y: 900, width: 100, height: 100), in: field
            ) == nil)

            // Clipping a single cell is not enough to judge.
            try expect(VisualEmptiness.score(
                of: WorldRect(x: -50, y: -50, width: 55, height: 55), in: field
            ) == nil)

            // A partial overlap that covers real cells still scores.
            try expect(VisualEmptiness.score(
                of: WorldRect(x: -50, y: -50, width: 200, height: 200), in: field
            ) != nil)

            // A sample count that disagrees with the grid is not a field.
            try expect(LuminanceField(
                bounds: bounds, columns: 4, rows: 4, samples: [0.1, 0.2]
            ) == nil)
        },
        LogicTest(name: "evade speed scales with the walking speed") {
            // Getting out of the way must never look slower than strolling.
            let standard = RuntimeTuning.standard
            try expect(standard.fastEvadeSpeed > standard.walkingSpeed)
            try expectNear(standard.fastEvadeSpeed, standard.walkingSpeed * 1.4)
            try expectNear(standard.slowEvadeSpeed, standard.fastEvadeSpeed * 0.55)

            var slower = RuntimeTuning.standard
            slower.walkingSpeed = 60
            let halved = slower.normalized
            try expectNear(halved.fastEvadeSpeed, 84)
            try expect(halved.fastEvadeSpeed > halved.walkingSpeed)

            // The floor keeps evasion usable at the slowest walking speeds.
            var crawling = RuntimeTuning.standard
            crawling.walkingSpeed = 20
            crawling.evadeSpeedScale = 0.8
            let floored = crawling.normalized
            try expectNear(floored.fastEvadeSpeed, 60)
            try expectNear(floored.slowEvadeSpeed, 33)

            // The panel value reaches the pointer model, not just the tuning.
            var brisk = RuntimeTuning.standard
            brisk.evadeSpeedScale = 3
            let configuration = brisk.normalized.pointerConfiguration
            try expectNear(configuration.fastEvadeSpeed, 480)
            try expectNear(configuration.slowEvadeSpeed, 264)
        },
        LogicTest(name: "gait cadence stays opt-in and independent of walking speed") {
            // The authored motion is the default. Raising the walking speed must
            // not retime the walk cycle on its own.
            try expectNear(RuntimeTuning.standard.locomotionAnimationRate, 1)

            var fast = RuntimeTuning.standard
            fast.walkingSpeed = 160
            try expectNear(fast.normalized.locomotionAnimationRate, 1)

            var doubled = RuntimeTuning.standard
            doubled.gaitCadence = 2
            try expectNear(doubled.normalized.locomotionAnimationRate, 2)

            var beyond = RuntimeTuning.standard
            beyond.gaitCadence = 9
            try expectNear(beyond.normalized.locomotionAnimationRate, 3.2)

            var below = RuntimeTuning.standard
            below.gaitCadence = 0
            try expectNear(below.normalized.locomotionAnimationRate, 0.5)
        },
        LogicTest(name: "look angles match Codex v2 clock convention") {
            let pet = WorldPoint.zero
            try expectNear(try require(PointerInteractionModel.lookDirectionDegrees(from: pet, to: WorldPoint(x: 0, y: -10))), 0)
            try expectNear(try require(PointerInteractionModel.lookDirectionDegrees(from: pet, to: WorldPoint(x: 10, y: 0))), 90)
            try expectNear(try require(PointerInteractionModel.lookDirectionDegrees(from: pet, to: WorldPoint(x: 0, y: 10))), 180)
            try expectNear(try require(PointerInteractionModel.lookDirectionDegrees(from: pet, to: WorldPoint(x: -10, y: 0))), 270)
        },
        LogicTest(name: "behavior follows catch drag drop lifecycle") {
            var behavior = BehaviorController(state: .idle, enteredAt: 0)
            behavior.handle(.pointer(.watching), at: 1)
            try expect(behavior.state == .lookAtPointer)
            behavior.handle(.pointer(.fastEvade), at: 2)
            try expect(behavior.state == .evadePointer)
            behavior.handle(.catchBegan, at: 3)
            behavior.handle(.dragMoved, at: 3.1)
            try expect(behavior.state == .dragged)
            behavior.handle(.pointer(.far), at: 3.2)
            try expect(behavior.state == .dragged)
            behavior.handle(.mouseReleased, at: 4)
            try expect(behavior.state == .dropped)
            // A drop borrows the jump row when a pet has no landing art, so it
            // runs for the jump's length rather than being cut mid-air.
            behavior.handle(.tick, at: 4 + BehaviorTiming.dropped - 0.01)
            try expect(behavior.state == .dropped)
            behavior.handle(.tick, at: 4 + BehaviorTiming.dropped + 0.01)
            try expect(behavior.state == .idle)
        },
        LogicTest(name: "behavior follows sit seek sleep wake lifecycle") {
            var behavior = BehaviorController(state: .idle, enteredAt: 0)
            behavior.handle(.beginRest, at: 10)
            try expect(behavior.state == .sit)
            try expect(behavior.state.isResting)
            behavior.handle(.seekSleepSpot, at: 12.4)
            try expect(behavior.state == .findSleepSpot)
            behavior.handle(.sleepSpotReached, at: 15)
            try expect(behavior.state == .sleep)
            behavior.handle(.meaningfulActivity, at: 20)
            try expect(behavior.state == .wake)
            behavior.handle(.tick, at: 20.71)
            try expect(behavior.state == .stretch)
            behavior.handle(.tick, at: 21.72)
            try expect(behavior.state == .idle)
        },
        LogicTest(name: "a pet watching the cursor can still walk off the text") {
            // Placement only ever hands this state a route that is taking the
            // pet off the user's work, so refusing the transition here is what
            // made that route land on a pet the state machine would not move.
            var behavior = BehaviorController(state: .idle, enteredAt: 0)
            behavior.handle(.pointer(.watching), at: 1)
            try expect(behavior.state == .lookAtPointer)
            try expect(behavior.handle(.beginWander, at: 2).to == .wander)

            // Being carried is still not a state you walk out of.
            var held = BehaviorController(state: .idle, enteredAt: 0)
            held.handle(.catchBegan, at: 1)
            try expect(held.handle(.beginWander, at: 2).to == .caught)
        },
        LogicTest(name: "manual stretch preview settles without breaking drag") {
            var behavior = BehaviorController(state: .idle, enteredAt: 0)
            behavior.handle(.beginStretch, at: 1)
            try expect(behavior.state == .stretch)
            behavior.handle(.tick, at: 2.01)
            try expect(behavior.state == .idle)

            behavior.handle(.catchBegan, at: 3)
            behavior.handle(.dragMoved, at: 3.1)
            behavior.handle(.beginStretch, at: 3.2)
            try expect(behavior.state == .dragged)
        },
        LogicTest(name: "completion celebration runs for exactly one wave") {
            // Petdex's `waving` is 0.70s and a conforming pet paces its four
            // frames for that. Holding longer replays the greeting; holding less
            // cuts it. Roamling therefore uses the same clock.
            var behavior = BehaviorController(state: .idle, enteredAt: 0)
            behavior.handle(.reaction(.smallCelebrate), at: 1)
            try expect(behavior.state == .celebrate)
            behavior.handle(.tick, at: 1 + BehaviorTiming.celebrate - 0.01)
            try expect(behavior.state == .celebrate)
            behavior.handle(.tick, at: 1 + BehaviorTiming.celebrate + 0.01)
            try expect(behavior.state == .idle)
        },
        LogicTest(name: "basic safe zones honor visible frame and Dock inset") {
            let display = DisplaySnapshot(
                id: "main",
                name: "main",
                frame: WorldRect(x: 0, y: 0, width: 1_000, height: 800),
                visibleFrame: WorldRect(x: 0, y: 24, width: 1_000, height: 700),
                scale: 2
            )
            let zones = BasicSafeZonePlanner.safeZones(
                in: DesktopWorldSnapshot(displays: [display])
            )
            try expect(zones.count == 4)
            for zone in zones {
                try expect(display.visibleFrame.contains(zone.frame.origin))
                try expect(display.visibleFrame.contains(
                    WorldPoint(x: zone.frame.maxX, y: zone.frame.maxY)
                ))
            }
            try expect(zones.contains(where: { $0.reason == "dock-adjacent-bottom-left" }))
            try expect(zones.contains(where: { $0.reason == "dock-adjacent-bottom-right" }))
        },
        LogicTest(name: "sleep placement stays on current display and avoids pointer") {
            let left = testDisplay("left", WorldRect(x: 0, y: 0, width: 1_000, height: 800))
            let right = testDisplay("right", WorldRect(x: 1_000, y: 0, width: 1_000, height: 800))
            let baseWorld = DesktopWorldSnapshot(displays: [left, right])
            let world = DesktopWorldSnapshot(
                displays: [left, right],
                safeZones: BasicSafeZonePlanner.safeZones(in: baseWorld)
            )
            let pointer = WorldPoint(x: 870, y: 690)
            let destination = try require(BasicSafeZonePlanner.destination(
                in: world,
                currentPosition: WorldPoint(x: 500, y: 400),
                pointerPosition: pointer,
                objectSize: WorldSize(width: 96, height: 104)
            ))
            try expect(destination.displayID == "left")
            try expect(left.visibleFrame.insetBy(dx: 48, dy: 52).contains(destination.point))
            try expect(destination.point.distance(to: pointer) > 400)
        },
        LogicTest(name: "a nap stays in place only on a spot the capture vetted") {
            // Getting up from a doze to walk to a corner is the trip the user
            // sees, so it now has to be earned: the pet sleeps where it dozed
            // off unless the screen says it is sitting on something.
            let bounds = WorldRect(x: 0, y: 0, width: 800, height: 400)
            let columns = 40
            let rows = 20
            var samples: [Double] = []
            for row in 0..<rows {
                for column in 0..<columns {
                    let text = column < columns / 2
                    samples.append(text && (column + row).isMultiple(of: 3) ? 0.78 : 0.86)
                }
            }
            let field = try require(LuminanceField(
                bounds: bounds, columns: columns, rows: rows, samples: samples
            ))
            let pet = WorldSize(width: 96, height: 104)

            func naps(at point: WorldPoint, in field: LuminanceField?) -> Bool {
                BasicSafeZonePlanner.napsInPlace(at: point, objectSize: pet, in: field)
            }

            try expect(naps(at: WorldPoint(x: 620, y: 200), in: field))
            try expect(!naps(at: WorldPoint(x: 150, y: 200), in: field))

            // No capture is not permission to sleep anywhere. Without one the
            // stroll that put the pet here never scored its destination either,
            // so the safe zone is the only vetted spot on offer.
            try expect(!naps(at: WorldPoint(x: 620, y: 200), in: nil))
            // Same answer when the pet stands outside the captured display.
            try expect(!naps(at: WorldPoint(x: 4_000, y: 4_000), in: field))
        },
        LogicTest(name: "rest timing is bounded") {
            let rest = RestConfiguration(
                idleBeforeRest: 1,
                sittingDuration: 100,
                wakeWanderDelay: 0
            )
            try expect(rest.idleBeforeRest == 10)
            try expect(rest.sittingDuration == 10)
            try expect(rest.wakeWanderDelay == 0.5)
        },
        LogicTest(name: "attention dwell yields to urgent event") {
            var model = AttentionModel(configuration: AttentionConfiguration(
                minimumDwellTime: 4,
                hysteresisMargin: 10,
                revisitCooldown: 2,
                maximumEventAge: 60
            ))
            let codex = testEvent("codex-work", "codex", .activityStarted, 0.5, 0)
            try expect(model.select(from: [codex], at: 0)?.sourceID == "codex")
            let claudeWork = testEvent("claude-work", "claude", .achievement, 1, 1)
            try expect(model.select(from: [codex, claudeWork], at: 1)?.sourceID == "codex")
            let permission = testEvent("claude-permission", "claude", .attentionRequired, 1, 1.5)
            try expect(model.select(from: [codex, permission], at: 1.5)?.sourceID == "claude")
        },
        LogicTest(name: "attention applies hysteresis after dwell") {
            var model = AttentionModel(configuration: AttentionConfiguration(
                minimumDwellTime: 1,
                hysteresisMargin: 20,
                revisitCooldown: 0,
                maximumEventAge: 60
            ))
            let current = testEvent("a", "a", .positive, 0.8, 0)
            _ = model.select(from: [current], at: 0)
            let barelyBetter = testEvent("b", "b", .achievement, 0.8, 2)
            try expect(model.select(from: [current, barelyBetter], at: 2)?.sourceID == "a")
        },
        LogicTest(name: "reaction policy reduces media celebration") {
            let achievement = testEvent("win", "source", .achievement, 1, 0)
            var activePolicy = ReactionPolicy(configuration: ReactionConfiguration(minimumInterval: 0))
            let active = activePolicy.reaction(
                for: achievement,
                context: .idle,
                currentBehavior: .idle,
                randomUnit: 0.3,
                at: 1
            )
            try expect(active == .largeCelebrate)
            var mediaPolicy = ReactionPolicy(configuration: ReactionConfiguration(minimumInterval: 0))
            let media = mediaPolicy.reaction(
                for: achievement,
                context: .watchingMedia,
                currentBehavior: .idle,
                randomUnit: 0.3,
                at: 1
            )
            try expect(media == .smallCelebrate)
        },
        LogicTest(name: "reaction policy ignores routine tool completions") {
            var policy = ReactionPolicy(configuration: ReactionConfiguration(minimumInterval: 0))
            let routine = testEvent("tool", "codex", .positive, 0.08, 0)
            let reaction = policy.reaction(
                for: routine,
                context: .working,
                currentBehavior: .work,
                randomUnit: 0,
                at: 1
            )
            try expect(reaction == nil)
        },
        LogicTest(name: "reaction policy always acknowledges achievements") {
            let achievement = testEvent("done", "codex", .achievement, 0.2, 0)
            for roll in [0.0, 0.5, 0.999_999] {
                var policy = ReactionPolicy(configuration: ReactionConfiguration(minimumInterval: 0))
                let reaction = policy.reaction(
                    for: achievement,
                    context: .working,
                    currentBehavior: .work,
                    randomUnit: roll,
                    at: 1
                )
                try expect(reaction == .smallCelebrate)
            }
        },
        LogicTest(name: "candidate scoring prefers stability on a tie") {
            let unstable = PositionCandidate(point: WorldPoint(x: 10, y: 0), visualEmptyScore: 5, stabilityScore: 1)
            let stable = PositionCandidate(point: WorldPoint(x: 20, y: 0), visualEmptyScore: 3, stabilityScore: 3)
            try expect(CandidatePositionScorer.best(from: [unstable, stable])?.point == stable.point)
        },
        LogicTest(name: "placement yields to the pointer without going blind") {
            // Defect 5: gating the judging along with the moving is what froze
            // the seat verdict whenever the cursor came near.
            let fixture = DirectorFixture()
            var director = PlacementDirector()
            let owned = fixture.situation(at: 0, position: fixture.corner, isPointerOwned: true)
            try expect(director.decide(owned) == PlacementIntent.none)

            // The verdict behind that `.none` was still worked out, so the tick
            // the pointer lets go acts on it instead of starting over.
            let released = fixture.situation(at: 0.05, position: fixture.corner)
            try expect(director.decide(released).travelReason == .newActivity)
        },
        LogicTest(name: "a new agent moves the pet across displays, not across the room") {
            // Walking over is the point of this priority, but only when there
            // is somewhere better to be. Without a caret the strongest pull is
            // the window's bottom edge, and on a full-screen window that is the
            // corner of the display -- not worth leaving a clear seat for.
            // Standing on another display is worth it, and the score does not
            // say so: measured here the corner seat beats a clear one on the
            // wrong screen by less than the margin.
            let left = DisplaySnapshot(
                id: "1", name: "1",
                frame: WorldRect(x: 0, y: 0, width: 1_728, height: 1_117),
                visibleFrame: WorldRect(x: 0, y: 25, width: 1_728, height: 1_092),
                scale: 2
            )
            let right = DisplaySnapshot(
                id: "2", name: "2",
                frame: WorldRect(x: 1_728, y: 0, width: 1_920, height: 1_080),
                visibleFrame: WorldRect(x: 1_728, y: 25, width: 1_920, height: 1_055),
                scale: 1
            )
            let window = WorldRect(x: 1_728, y: 25, width: 1_920, height: 1_055)
            func situation(_ position: WorldPoint) -> PetSituation {
                PetSituation(
                    timestamp: 0,
                    world: DesktopWorldSnapshot(displays: [left, right]),
                    position: position,
                    objectSize: WorldSize(width: 96, height: 104),
                    activitySourceID: "claude",
                    activityHint: LocationHint(approximateRegion: window, confidence: 0.55)
                )
            }

            var away = PlacementDirector()
            let crossing = away.decide(situation(WorldPoint(x: 800, y: 500)))
            try expect(crossing.travelReason == .newActivity, "got \(crossing)")
            let landing = try require(crossing.destination).point
            try expect(
                landing.x >= right.frame.minX,
                "the pet has to end up on the display the work is on, got \(landing)"
            )

            // Already watching that window: a corner on the same screen is not
            // a reason to get up.
            var present = PlacementDirector()
            try expect(present.decide(situation(WorldPoint(x: 2_600, y: 700))) == .hold)
            // Just outside the frame still counts as being there.
            var beside = PlacementDirector()
            try expect(beside.decide(situation(WorldPoint(x: 1_760, y: 60))) == .hold)
        },
        LogicTest(name: "a seat the planner picks is one it agrees is watching") {
            // Otherwise placement chooses a seat beside the window and then
            // decides on the next review that the seat is not watching the
            // window, forever. A full-screen window hides it, because the seats
            // beside it get clamped back onto the display.
            let display = DisplaySnapshot(
                id: "main", name: "main",
                frame: WorldRect(x: 0, y: 0, width: 1_600, height: 1_000),
                visibleFrame: WorldRect(x: 0, y: 25, width: 1_600, height: 975),
                scale: 2
            )
            let objectSize = WorldSize(width: 96, height: 104)
            for width in [320.0, 640.0, 900.0, 1_400.0] {
                for height in [240.0, 500.0, 800.0] {
                    let window = WorldRect(x: 120, y: 90, width: width, height: height)
                    let world = DesktopWorldSnapshot(displays: [display])
                    let hint = LocationHint(approximateRegion: window, confidence: 0.55)
                    let pet = WorldPoint(x: 1_500, y: 900)
                    guard let destination = BasicInterestPositionPlanner.destination(
                        for: hint, in: world, currentPosition: pet,
                        pointerPosition: nil, objectSize: objectSize
                    ) else { continue }
                    let seat = try require(BasicInterestPositionPlanner.evaluateSeat(
                        at: destination.point, for: hint, in: world,
                        currentPosition: destination.point,
                        pointerPosition: nil, objectSize: objectSize
                    ))
                    try expect(
                        seat.watchesRegion,
                        "window \(width)x\(height) got a seat at \(destination.point) "
                            + "that it does not consider to be watching it"
                    )
                }
            }
        },
        LogicTest(name: "a new agent seats the pet and its next event keeps it there") {
            let fixture = DirectorFixture()
            var director = PlacementDirector()
            let travel = director.decide(fixture.situation(at: 0, position: fixture.corner))
            try expect(travel.travelReason == .newActivity)
            let seat = try require(travel.destination).point

            // Still walking. The destination must not change under the pet.
            let midWalk = director.decide(fixture.situation(at: 0.6, position: fixture.corner))
            try expect(midWalk.destination?.point == seat)

            try expect(director.decide(fixture.situation(at: 1, position: seat)) == .hold)
            try expect(director.isSeated)

            // An agent fires an event per tool call. Re-planning on each is
            // what made the pet pace the window instead of watching it.
            for step in 0..<12 {
                let now = 1.5 + Double(step) * 0.5
                try expect(
                    director.decide(fixture.situation(at: now, position: seat)) == .hold,
                    "tick \(now) moved a seat that never got worse"
                )
            }
        },
        LogicTest(name: "a covered seat is left once, for somewhere actually clear") {
            // The reported twitch: leaving a marginal seat for another marginal
            // seat meant the replacement flickered across the same line the old
            // one did, so the pet paced instead of settling. Measured on a real
            // desktop a clear seat scores about 0.97 and text lands anywhere in
            // 0.35...0.55, so the rule is about where it lands, not how far it
            // fell.
            let fixture = DirectorFixture()
            let configuration = PlacementDirector.Configuration.standard
            var director = PlacementDirector()
            let flat = try require(fixture.flatField())
            let seat = try require(director.decide(
                fixture.situation(at: 0, position: fixture.corner, luminance: flat)
            ).destination).point
            try expect(director.decide(
                fixture.situation(at: 1, position: seat, luminance: flat)
            ) == .hold)

            // Sparse text under the pet: not dense, and still the user's work.
            let covered = try require(fixture.field(busyAround: seat, delta: 0.014))
            let score = try require(
                VisualEmptiness.score(of: fixture.petFrame(at: seat), in: covered)
            )
            try expect(
                score < configuration.holdEmptiness && score > 0.35,
                "this case needs a seat in the band real text lands in, got \(score)"
            )
            let moved = director.decide(fixture.situation(
                at: 4, position: seat, luminance: covered
            ))
            try expect(moved.travelReason == .coveringWork, "got \(moved)")

            // It moved somewhere genuinely empty, so nothing brings it back.
            let better = try require(moved.destination).point
            let betterScore = try require(
                VisualEmptiness.score(of: fixture.petFrame(at: better), in: covered)
            )
            try expect(
                betterScore >= configuration.holdEmptiness,
                "the replacement was as marginal as the seat it left: \(betterScore)"
            )
            for step in 0..<20 {
                let now = 5 + Double(step) * 0.5
                try expect(
                    director.decide(fixture.situation(
                        at: now, position: better, luminance: covered
                    )) == .hold,
                    "the pet kept pacing at \(now)"
                )
            }
        },
        LogicTest(name: "a marginal seat is kept when every alternative is as bad") {
            // Walking off the user's text is only progress if there is somewhere
            // better to stand. Trading one covered seat for another is the pacing
            // this is here to prevent.
            let fixture = DirectorFixture()
            var director = PlacementDirector()
            let flat = try require(fixture.flatField())
            let seat = try require(director.decide(
                fixture.situation(at: 0, position: fixture.corner, luminance: flat)
            ).destination).point
            try expect(director.decide(
                fixture.situation(at: 1, position: seat, luminance: flat)
            ) == .hold)

            let everywhere = try require(fixture.uniformField(delta: 0.014))
            for step in 0..<20 {
                let now = 4 + Double(step) * 0.5
                try expect(
                    director.decide(fixture.situation(
                        at: now, position: seat, luminance: everywhere
                    )) == .hold,
                    "the pet walked to an equally covered seat at \(now)"
                )
            }
        },
        LogicTest(name: "a fresh seat survives the first bad frame under it") {
            let fixture = DirectorFixture()
            var director = PlacementDirector()
            let flat = try require(fixture.flatField())
            let seat = try require(director.decide(
                fixture.situation(at: 0, position: fixture.corner, luminance: flat)
            ).destination).point
            try expect(director.decide(
                fixture.situation(at: 1, position: seat, luminance: flat)
            ) == .hold)

            let covered = try require(fixture.field(busyAround: seat, delta: 0.06))
            // Inside the dwell window the seat is defended...
            try expect(director.decide(
                fixture.situation(at: 2, position: seat, luminance: covered)
            ) == .hold)
            // ...and once it is over the pet steps off the user's work.
            try expect(director.decide(
                fixture.situation(at: 4, position: seat, luminance: covered)
            ).travelReason == .coveringWork)
        },
        LogicTest(name: "a seat chosen without a capture is re-decided once one arrives") {
            // Defect 3: the first event of a session always plans blind, and
            // hysteresis then defended that guess for the whole session because
            // wallpaper in a corner always scores as empty.
            let fixture = DirectorFixture()
            var director = PlacementDirector()
            let blind = try require(director.decide(
                fixture.situation(at: 0, position: fixture.corner)
            ).destination).point
            try expect(director.decide(fixture.situation(at: 1, position: blind)) == .hold)

            let field = try require(fixture.field(busyAround: blind, delta: 0.06))
            let replan = director.decide(
                fixture.situation(at: 1.6, position: blind, luminance: field)
            )
            try expect(replan.travelReason == .plannedBlind, "got \(replan)")

            // And only once. The replacement was chosen with the capture in
            // hand, so nothing about it is owed a second look.
            let better = try require(replan.destination).point
            let betterScore = try require(
                VisualEmptiness.score(of: fixture.petFrame(at: better), in: field)
            )
            try expect(
                betterScore >= PlacementDirector.Configuration.standard.holdEmptiness,
                "the replacement seat scored \(betterScore)"
            )
            try expect(director.decide(
                fixture.situation(at: 2.2, position: better, luminance: field)
            ) == .hold)
            try expect(director.decide(
                fixture.situation(at: 3, position: better, luminance: field)
            ) == .hold)
        },
        LogicTest(name: "a capture landing mid-walk still leaves the seat blind") {
            // The capture is requested when the seat is planned and lands while
            // the pet walks, so this is the ordinary case rather than the corner
            // one. Counting the arrival's capture as though it had informed the
            // choice turned `plannedBlind` off everywhere it mattered.
            let fixture = DirectorFixture()
            var director = PlacementDirector()
            let blind = try require(director.decide(
                fixture.situation(at: 0, position: fixture.corner)
            ).destination).point

            // It arrives with a capture in hand that it did not have when it set off.
            let field = try require(fixture.field(busyAround: blind, delta: 0.06))
            try expect(director.decide(
                fixture.situation(at: 1, position: blind, luminance: field)
            ) == .hold)

            let replan = director.decide(
                fixture.situation(at: 1.6, position: blind, luminance: field)
            )
            try expect(replan.travelReason == .plannedBlind, "got \(replan)")
        },
        LogicTest(name: "the pet steps off the caret without waiting out the dwell") {
            let fixture = DirectorFixture()
            var director = PlacementDirector()
            let seat = try require(director.decide(
                fixture.situation(at: 0, position: fixture.corner)
            ).destination).point
            try expect(director.decide(fixture.situation(at: 1, position: seat)) == .hold)

            // The user clicks into the line the pet is sitting on.
            let focus = FocusSnapshot(
                windowFrame: fixture.window,
                caretFrame: WorldRect(x: seat.x - 1, y: seat.y - 8, width: 2, height: 18),
                confidence: 0.9
            )
            let moved = director.decide(
                fixture.situation(at: 1.6, position: seat, focus: focus)
            )
            try expect(moved.travelReason == .coveringCaret, "got \(moved)")
        },
        LogicTest(name: "the seat follows the window the agent moved to") {
            let fixture = DirectorFixture()
            var director = PlacementDirector()
            let seat = try require(director.decide(
                fixture.situation(at: 0, position: fixture.corner)
            ).destination).point
            try expect(director.decide(fixture.situation(at: 1, position: seat)) == .hold)

            // Same agent, different window: the seat it holds is no longer
            // watching anything.
            let moved = LocationHint(
                approximateRegion: WorldRect(x: 700, y: 120, width: 420, height: 300),
                confidence: 0.55
            )
            let follow = director.decide(
                fixture.situation(at: 1.6, position: seat, hint: moved)
            )
            try expect(follow.travelReason == .followedFocus, "got \(follow)")
        },
        LogicTest(name: "an idle pet sleeps on the seat it is keeping") {
            let fixture = DirectorFixture()
            var director = PlacementDirector()
            let seat = try require(director.decide(
                fixture.situation(at: 0, position: fixture.corner)
            ).destination).point
            try expect(director.decide(fixture.situation(at: 1, position: seat)) == .hold)
            try expect(director.decide(fixture.situation(
                at: 90, position: seat, userIdleDuration: 80, idleBeforeRest: 75
            )) == .sleepInPlace)

            // A seat that went bad outranks the nap.
            let field = try require(fixture.field(busyAround: seat, delta: 0.06))
            let woken = director.decide(fixture.situation(
                at: 91,
                position: seat,
                luminance: field,
                userIdleDuration: 81,
                idleBeforeRest: 75
            ))
            try expect(woken.travelReason != nil, "got \(woken)")
        },
        LogicTest(name: "the seat is released when there is no agent left to watch") {
            let fixture = DirectorFixture()
            var director = PlacementDirector()
            let seat = try require(director.decide(
                fixture.situation(at: 0, position: fixture.corner)
            ).destination).point
            try expect(director.decide(fixture.situation(at: 1, position: seat)) == .hold)
            try expect(director.isSeated)

            try expect(director.decide(
                fixture.situation(at: 2, position: seat, sourceID: nil)
            ) == .hold)
            try expect(!director.isSeated)
        },
        LogicTest(name: "a parked pet leaves text without waiting out the pause") {
            // Between agent turns the pet is just roaming, and the pause between
            // walks is long enough for the user to scroll a paragraph under it.
            // Nothing else watches during that window.
            let fixture = DirectorFixture()
            var director = PlacementDirector()
            let parked = WorldPoint(x: 300, y: 400)
            let clear = WorldPoint(x: 900, y: 300)
            let covered = try require(fixture.field(busyAround: parked, delta: 0.06))

            func roaming(at timestamp: TimeInterval, position: WorldPoint) -> PetSituation {
                fixture.situation(
                    at: timestamp,
                    position: position,
                    sourceID: nil,
                    luminance: covered,
                    strollCandidates: [clear]
                )
            }

            // It has only just stopped here, so it is given a moment.
            try expect(director.decide(roaming(at: 0, position: parked)) == .hold)
            try expect(director.decide(roaming(at: 1, position: parked)) == .hold)

            let escaped = director.decide(roaming(at: 6, position: parked))
            try expect(escaped == .escape(clear), "got \(escaped)")

            // Once it is standing somewhere clear, nothing sends it off again.
            for step in 0..<10 {
                try expect(
                    director.decide(roaming(
                        at: 10 + Double(step) * 0.5, position: clear
                    )) == .hold,
                    "the pet left a clear spot"
                )
            }
        },
        LogicTest(name: "a parked pet on text stays put when nowhere is better") {
            let fixture = DirectorFixture()
            var director = PlacementDirector()
            let parked = WorldPoint(x: 300, y: 400)
            let alsoCovered = WorldPoint(x: 340, y: 430)
            let everywhere = try require(fixture.uniformField(delta: 0.06))
            for step in 0..<20 {
                let intent = director.decide(fixture.situation(
                    at: Double(step) * 0.5,
                    position: parked,
                    sourceID: nil,
                    luminance: everywhere,
                    strollCandidates: [alsoCovered]
                ))
                try expect(intent == .hold, "walked to an equally covered spot: \(intent)")
            }
        },
        LogicTest(name: "a walking or sleeping pet is not re-routed by its own spot") {
            let fixture = DirectorFixture()
            let parked = WorldPoint(x: 300, y: 400)
            let clear = WorldPoint(x: 900, y: 300)
            let covered = try require(fixture.field(busyAround: parked, delta: 0.06))

            for (label, walking, resting) in [("walking", true, false), ("resting", false, true)] {
                var director = PlacementDirector()
                for step in 0..<20 {
                    let intent = director.decide(fixture.situation(
                        at: Double(step) * 0.5,
                        position: parked,
                        sourceID: nil,
                        luminance: covered,
                        isWalking: walking,
                        isResting: resting,
                        strollCandidates: [clear]
                    ))
                    try expect(intent == .hold, "\(label) pet was re-routed: \(intent)")
                }
            }
        },
        LogicTest(name: "text under the pet outlives the cursor that interrupted it") {
            let fixture = DirectorFixture()
            var director = PlacementDirector()
            let parked = WorldPoint(x: 300, y: 400)
            let clear = WorldPoint(x: 900, y: 300)
            let covered = try require(fixture.field(busyAround: parked, delta: 0.06))

            func decide(at timestamp: TimeInterval, pointerOwned: Bool) -> PlacementIntent {
                director.decide(fixture.situation(
                    at: timestamp,
                    position: parked,
                    sourceID: nil,
                    luminance: covered,
                    isPointerOwned: pointerOwned,
                    strollCandidates: [clear]
                ))
            }

            // The cursor arrives while the pet is still serving its dwell, and
            // stays. Every one of these ticks decides the escape and throws it
            // away, which is exactly when the dwell used to be reset -- so the
            // wait restarted faster than it could ever mature, and the pet kept
            // the paragraph for as long as the cursor sat next to it.
            _ = decide(at: 0, pointerOwned: false)
            for step in 0..<20 {
                try expect(decide(at: 3 + Double(step) * 0.5, pointerOwned: true) == .none)
            }
            // The cursor leaves. The text is still there, so the walk is still
            // owed, and it is owed now rather than one dwell from now.
            try expect(decide(at: 13, pointerOwned: false) == .escape(clear))
        },
        LogicTest(name: "a glance stops an aimless walk but not one off the text") {
            // The cursor parked beside a pet that is standing on a paragraph is
            // the ordinary case, not a corner one: the pet is on the text
            // because that is where the user is working, so that is where their
            // cursor is. Holding the glance above the walk meant the pet kept
            // the paragraph for as long as the user read it.
            let fixture = DirectorFixture()
            let parked = WorldPoint(x: 300, y: 400)
            let clear = WorldPoint(x: 900, y: 300)
            let covered = try require(fixture.field(busyAround: parked, delta: 0.06))

            var owed = PlacementDirector()
            func watched(at timestamp: TimeInterval) -> PlacementIntent {
                owed.decide(fixture.situation(
                    at: timestamp,
                    position: parked,
                    sourceID: nil,
                    luminance: covered,
                    isPointerWatching: true,
                    strollCandidates: [clear]
                ))
            }
            // Still serving its dwell, so there is nothing yet that outranks
            // the glance.
            try expect(watched(at: 0) == PlacementIntent.none)
            try expect(watched(at: 3) == .escape(clear))

            // A walk the pet merely fancied still yields. Nothing is under it,
            // so there is nothing for the glance to be in the way of.
            var whim = PlacementDirector()
            try expect(whim.decide(fixture.situation(
                at: 0,
                position: fixture.corner,
                sourceID: nil,
                luminance: fixture.flatField(),
                isPointerWatching: true,
                isStrollDue: true,
                strollCandidates: [clear]
            )) == PlacementIntent.none)

            // And a cursor close enough to reach for the pet takes it back.
            var reached = PlacementDirector()
            func owned(at timestamp: TimeInterval) -> PlacementIntent {
                reached.decide(fixture.situation(
                    at: timestamp,
                    position: parked,
                    sourceID: nil,
                    luminance: covered,
                    isPointerOwned: true,
                    strollCandidates: [clear]
                ))
            }
            try expect(owned(at: 0) == PlacementIntent.none)
            try expect(owned(at: 3) == PlacementIntent.none)
        },
        LogicTest(name: "a stroll destination passes the same emptiness bar as a seat") {
            // Defect 4: the rule about empty space lived only on the agent-seat
            // path, and roaming is where the pet spends most of its life.
            let fixture = DirectorFixture()
            var director = PlacementDirector()
            let busy = WorldPoint(x: 300, y: 400)
            let clear = WorldPoint(x: 900, y: 300)
            let field = try require(fixture.field(busyAround: busy, delta: 0.06))

            let strolling = director.decide(fixture.situation(
                at: 0,
                position: fixture.corner,
                sourceID: nil,
                luminance: field,
                isStrollDue: true,
                strollCandidates: [busy, clear]
            ))
            try expect(strolling == .stroll(clear), "got \(strolling)")

            // Nothing is due, so nothing is chosen.
            try expect(director.decide(
                fixture.situation(at: 1, position: fixture.corner, sourceID: nil)
            ) == .hold)
        },
        LogicTest(name: "an approval request always shows the paw") {
            // It used to roll a third of these into a stare. Petdex holds
            // `waiting` until the user answers, and a pet that asks only
            // two times in three is a pet you learn not to trust.
            var policy = ReactionPolicy(configuration: .init(minimumInterval: 0))
            for step in 0..<20 {
                let reaction = policy.reaction(
                    for: CompanionEvent(
                        sourceID: "agent",
                        sourceType: .agent,
                        timestamp: Double(step),
                        kind: .attentionRequired,
                        intensity: 0.95,
                        context: .working
                    ),
                    context: .working,
                    currentBehavior: .idle,
                    randomUnit: Double(step) / 20,
                    at: Double(step)
                )
                try expect(reaction == .paw, "roll \(step) gave \(String(describing: reaction))")
            }
        },
        LogicTest(name: "a turn opening sparks and a read observes") {
            var policy = ReactionPolicy(configuration: .init(minimumInterval: 0))
            func reaction(_ kind: CompanionEventKind, _ intensity: Double, at time: Double) -> CompanionReaction? {
                policy.reaction(
                    for: CompanionEvent(
                        sourceID: "agent",
                        sourceType: .agent,
                        timestamp: time,
                        kind: kind,
                        intensity: intensity,
                        context: .working
                    ),
                    context: .working,
                    currentBehavior: .idle,
                    randomUnit: 0.5,
                    at: time
                )
            }
            // Petdex: user-prompt -> jumping, pre+Read -> review, pre -> running.
            try expect(reaction(.activityStarted, 0.55, at: 0) == .spark)
            try expect(reaction(.inspecting, 0.45, at: 1) == .observe)
            try expect(reaction(.highIntensity, 0.72, at: 2) == .work)
        },
        LogicTest(name: "waiting for the user does not time out") {
            // Petdex classes `waiting` as steady. The old 1.2s hand-off meant the
            // picture on screen during an approval prompt was `review`, not the
            // paw the user was supposed to notice.
            var behavior = BehaviorController(state: .idle, enteredAt: 0)
            behavior.handle(.reaction(.paw), at: 0)
            try expect(behavior.state == .waitingForUser)
            for step in 1...60 {
                behavior.handle(.tick, at: Double(step))
            }
            try expect(behavior.state == .waitingForUser, "drifted to \(behavior.state)")
        },
        LogicTest(name: "transient agent states hand back on the Petdex clock") {
            var behavior = BehaviorController(state: .idle, enteredAt: 0)
            behavior.handle(.reaction(.spark), at: 0)
            try expect(behavior.state == .spark)
            behavior.handle(.tick, at: BehaviorTiming.spark - 0.01)
            try expect(behavior.state == .spark, "spark ended early")
            behavior.handle(.tick, at: BehaviorTiming.spark + 0.01)
            try expect(behavior.state == .idle)

            behavior.handle(.reaction(.observe), at: 10)
            try expect(behavior.state == .observe)
            behavior.handle(.tick, at: 10 + BehaviorTiming.observe - 0.01)
            try expect(behavior.state == .observe, "observe ended early")
            behavior.handle(.tick, at: 10 + BehaviorTiming.observe + 0.01)
            try expect(behavior.state == .idle, "observe never handed back")
        },
        LogicTest(name: "only lasting conditions are worn continuously") {
            // What holdSeat is allowed to re-apply every tick. Re-applying a
            // moment is what turned a one-second beat into a session-long loop.
            try expect(CompanionReaction.work.isOngoing)
            try expect(CompanionReaction.paw.isOngoing)
            for moment in [CompanionReaction.observe, .glance, .spark, .smallCelebrate, .largeCelebrate, .sad, .calm] {
                try expect(!moment.isOngoing, "\(moment) would be re-applied forever")
            }
        },
        LogicTest(name: "watching quickens as the pointer closes") {
            // Proximity is three bands; the distance behind it is continuous, and
            // so is a tail. The rate spends that resolution rather than snapping
            // between two speeds at a threshold.
            let config = PointerInteractionConfiguration()
            try expectNear(config.attentionRate(atDistance: 400), 1.0)
            try expectNear(config.attentionRate(atDistance: config.awarenessDistance), 1.0)
            try expectNear(config.attentionRate(atDistance: config.catchDistance), 2.0)
            try expectNear(config.attentionRate(atDistance: 0), 2.0)

            let mid = (config.awarenessDistance + config.catchDistance) / 2
            try expectNear(config.attentionRate(atDistance: mid), 1.5)

            // Monotonic: never slower for being nearer.
            var previous = 0.0
            for step in stride(from: 400.0, through: 0, by: -10) {
                let rate = config.attentionRate(atDistance: step)
                try expect(rate >= previous, "rate dropped at \(step)")
                previous = rate
            }
        },
        LogicTest(name: "the watching rate follows the tuned radii") {
            // One idea of "close". Widening awareness in the tuning panel has to
            // stretch this ramp with it, not leave a second hidden threshold.
            let wide = PointerInteractionConfiguration(awarenessDistance: 320, catchDistance: 80)
            try expectNear(wide.attentionRate(atDistance: 320), 1.0)
            try expectNear(wide.attentionRate(atDistance: 200), 1.5)
            try expectNear(wide.attentionRate(atDistance: 80), 2.0)
        },
    ]
}

private extension PlacementIntent {
    var destination: InterestDestination? {
        guard case let .travel(destination, _) = self else { return nil }
        return destination
    }
}

/// One display, one window, and whatever the decision table needs to read.
///
/// Every argument here used to be a mutable field on the runtime that four
/// code paths wrote to, which is why none of these cases could be reproduced
/// without launching the app.
private struct DirectorFixture {
    let display = DisplaySnapshot(
        id: "main",
        name: "main",
        frame: WorldRect(x: 0, y: 0, width: 1_200, height: 900),
        visibleFrame: WorldRect(x: 0, y: 24, width: 1_200, height: 830),
        scale: 2
    )
    /// Deliberately not filling the display: the pet has to be able to stand
    /// somewhere that is not already watching this window.
    let window = WorldRect(x: 300, y: 86, width: 842, height: 600)
    let objectSize = WorldSize(width: 96, height: 104)
    /// Outside the watched region, so a first plan is a real walk rather than
    /// the pet deciding it is already where it needs to be.
    let corner = WorldPoint(x: 80, y: 800)

    func petFrame(at point: WorldPoint) -> WorldRect {
        WorldRect(
            x: point.x - objectSize.width / 2,
            y: point.y - objectSize.height / 2,
            width: objectSize.width,
            height: objectSize.height
        )
    }

    func situation(
        at timestamp: TimeInterval,
        position: WorldPoint,
        sourceID: String? = "claude",
        hint: LocationHint? = nil,
        /// A source that arrived without a window, which is a state the runtime
        /// can really be in and used to freeze the pet.
        hintless: Bool = false,
        focus: FocusSnapshot? = nil,
        luminance: LuminanceField? = nil,
        isPointerOwned: Bool = false,
        isPointerWatching: Bool = false,
        isEvading: Bool = false,
        isWalking: Bool = false,
        isResting: Bool = false,
        userIdleDuration: TimeInterval = 0,
        idleBeforeRest: TimeInterval = .infinity,
        isStrollDue: Bool = false,
        strollCandidates: [WorldPoint] = []
    ) -> PetSituation {
        PetSituation(
            timestamp: timestamp,
            world: DesktopWorldSnapshot(
                displays: [display],
                windows: [WindowSnapshot(id: "w1", frame: window, isFocused: true)],
                focus: focus,
                luminance: luminance
            ),
            position: position,
            objectSize: objectSize,
            pointerPosition: WorldPoint(x: 600, y: 100),
            isPointerOwned: isPointerOwned,
            isPointerWatching: isPointerWatching,
            isEvading: isEvading,
            isWalking: isWalking,
            isResting: isResting,
            activitySourceID: sourceID,
            activityHint: hintless || sourceID == nil
                ? nil
                : (hint ?? LocationHint(approximateRegion: window, confidence: 0.55)),
            userIdleDuration: userIdleDuration,
            idleBeforeRest: idleBeforeRest,
            isStrollDue: isStrollDue,
            strollCandidates: strollCandidates
        )
    }

    /// Wallpaper: nothing anywhere for the score to object to.
    func flatField() -> LuminanceField? { field(busyAround: nil, delta: 0) }

    /// The same middling texture over the whole display, so no seat anywhere is
    /// better than the one the pet already has.
    func uniformField(delta: Double) -> LuminanceField? {
        field(busyAround: display.frame.center, delta: delta, radius: display.frame)
    }

    /// Flat except around `point`, where neighbouring cells differ by `delta`.
    /// Emptiness is driven almost entirely by that difference, so it is the one
    /// knob these cases turn.
    func field(
        busyAround point: WorldPoint?,
        delta: Double,
        radius: WorldRect? = nil
    ) -> LuminanceField? {
        let columns = 80
        let rows = 60
        let cell = WorldSize(
            width: display.frame.size.width / Double(columns),
            height: display.frame.size.height / Double(rows)
        )
        let busy = radius ?? point.map { seat -> WorldRect in
            let frame = petFrame(at: seat)
            return WorldRect(
                x: frame.minX - 30,
                y: frame.minY - 30,
                width: frame.size.width + 60,
                height: frame.size.height + 60
            )
        }
        var samples: [Double] = []
        for row in 0..<rows {
            for column in 0..<columns {
                let sample = WorldPoint(
                    x: (Double(column) + 0.5) * cell.width,
                    y: (Double(row) + 0.5) * cell.height
                )
                guard let busy, busy.contains(sample) else {
                    samples.append(0.62)
                    continue
                }
                samples.append(
                    (column + row).isMultiple(of: 2) ? 0.5 - delta / 2 : 0.5 + delta / 2
                )
            }
        }
        return LuminanceField(
            bounds: display.frame,
            columns: columns,
            rows: rows,
            samples: samples
        )
    }
}


private func testDisplay(_ id: String, _ frame: WorldRect) -> DisplaySnapshot {
    DisplaySnapshot(id: id, name: id, frame: frame, visibleFrame: frame, scale: 1)
}

private func testEvent(
    _ id: String,
    _ source: String,
    _ kind: CompanionEventKind,
    _ intensity: Double,
    _ timestamp: TimeInterval
) -> CompanionEvent {
    CompanionEvent(
        id: id,
        sourceID: source,
        sourceType: .agent,
        timestamp: timestamp,
        kind: kind,
        intensity: intensity
    )
}
