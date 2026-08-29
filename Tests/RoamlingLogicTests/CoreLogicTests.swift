// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

import Foundation
import RoamlingCore

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
            try expectNear(RuntimeTuning.standard.wanderDelay(randomUnit: 0), 8.4)
            try expectNear(RuntimeTuning.standard.wanderDelay(randomUnit: 1), 17.4)
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

            // A smooth gradient is still comfortable: no local edges, and the
            // spread alone must not disqualify it.
            let gradient = try field { column, _ in Double(column) / Double(columns) }
            let gradientScore = try require(VisualEmptiness.score(of: seat, in: gradient))
            try expect(gradientScore > 0.6, "smooth gradient scored \(gradientScore)")

            // Alternating high-contrast cells stand in for text and dense UI.
            let busy = try field { column, row in (column + row).isMultiple(of: 2) ? 0.05 : 0.95 }
            let busyScore = try require(VisualEmptiness.score(of: seat, in: busy))
            try expect(busyScore < 0.1, "dense region scored \(busyScore)")
            try expect(busyScore < gradientScore)
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
            behavior.handle(.tick, at: 4.36)
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
        LogicTest(name: "completion celebration remains visible for its animation") {
            var behavior = BehaviorController(state: .idle, enteredAt: 0)
            behavior.handle(.reaction(.smallCelebrate), at: 1)
            try expect(behavior.state == .celebrate)
            behavior.handle(.tick, at: 3.19)
            try expect(behavior.state == .celebrate)
            behavior.handle(.tick, at: 3.21)
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
        }
    ]
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
