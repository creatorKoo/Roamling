// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

import AppKit
import RoamlingCore
import RoamlingPet

@MainActor
public final class RoamlingRuntime: NSObject, PetOverlayViewDelegate {
    private enum DefaultsKey {
        static let roaming = "roamling.roaming"
        static let avoidPointer = "roamling.avoidPointer"
        static let interactions = "roamling.interactions"
        static let scale = "roamling.scale"
        static let runtimeTuning = "roamling.runtimeTuning"
        static let petPackagePath = "roamling.petPackagePath"
        static let builtInPet = "roamling.builtInPet"
        static let positionX = "roamling.position.x"
        static let positionY = "roamling.position.y"
        static let hasPosition = "roamling.position.exists"
    }

    public var isRoamingEnabled: Bool {
        didSet {
            UserDefaults.standard.set(isRoamingEnabled, forKey: DefaultsKey.roaming)
            if !isRoamingEnabled {
                movement.cancelRoute(stop: false)
                nextWanderAt = .infinity
            } else if oldValue != isRoamingEnabled {
                nextWanderAt = ProcessInfo.processInfo.systemUptime + 0.8
            }
        }
    }

    public var isPointerAvoidanceEnabled: Bool {
        didSet {
            UserDefaults.standard.set(isPointerAvoidanceEnabled, forKey: DefaultsKey.avoidPointer)
            if !isPointerAvoidanceEnabled, isEvadeTransitioning {
                isEvadeTransitioning = false
                movement.cancelRoute(stop: false)
            }
        }
    }

    public var areInteractionsEnabled: Bool {
        didSet {
            UserDefaults.standard.set(areInteractionsEnabled, forKey: DefaultsKey.interactions)
            if !areInteractionsEnabled {
                catchArmedUntil = 0
                overlay.setInteractionEnabled(false)
            }
        }
    }

    public private(set) var asset: PetAsset
    public private(set) var installedPets: [PetDescriptor]
    public private(set) var selectedBuiltInPet: BuiltInPetKind?
    public private(set) var tuning: RuntimeTuning

    public var currentPetPackagePath: String? { asset.packageURL?.standardizedFileURL.path }
    public var petDisplayName: String { asset.manifest.displayName }
    public var scale: Double { overlay.scale }

    private let displayProvider = MacDisplayProvider()
    private let safeZoneProvider = MacBasicSafeZoneProvider()
    private let userIdleProvider = MacUserIdleProvider()
    private let restConfiguration = RestConfiguration.standard
    private var pointerProvider: MacPointerProvider!
    private let catalog: PetCatalog
    private let loader = PetLoader()
    private let overlay: MacOverlayProvider

    private var displays: [DisplaySnapshot]
    private var coordinateSpace: DesktopCoordinateSpace
    private var world: DesktopWorldSnapshot
    private var movement: MovementController
    private var behavior: BehaviorController
    private var pointerModel: PointerInteractionModel
    private var animationPlayer: PetAnimationPlayer

    private var tickTimer: Timer?
    private var screenObserver: NSObjectProtocol?
    private var lastTickAt: TimeInterval?
    private var nextWanderAt: TimeInterval
    private var catchArmedUntil: TimeInterval = 0
    private var isDragging = false
    private var dragOffset = WorldVector.zero
    private var lastPointerDecision: PointerDecision?
    private var isEvadeTransitioning = false
    private var restDestination: RestDestination?
    private var running = false

    public override init() {
        let defaults = UserDefaults.standard
        defaults.register(defaults: [
            DefaultsKey.roaming: true,
            DefaultsKey.avoidPointer: true,
            DefaultsKey.interactions: true,
            DefaultsKey.scale: 1.0,
            DefaultsKey.builtInPet: BuiltInPetKind.mochi.rawValue
        ])
        let runtimeTuning = Self.loadRuntimeTuning(defaults: defaults)

        let catalog = PetCatalog()
        let descriptors = catalog.discover()
        let selectedPath = defaults.string(forKey: DefaultsKey.petPackagePath)
        let selectedBuiltInPet = defaults.string(forKey: DefaultsKey.builtInPet)
            .flatMap(BuiltInPetKind.init(rawValue:)) ?? .mochi
        let initialAsset = Self.loadInitialAsset(
            descriptors: descriptors,
            selectedPath: selectedPath,
            builtInPet: selectedBuiltInPet,
            loader: PetLoader()
        )

        let displayProvider = MacDisplayProvider()
        let displaySet = displayProvider.snapshotSet()
        let initialWorld = DesktopWorldSnapshot(displays: displaySet.displays)
        let objectScale = defaults.double(forKey: DefaultsKey.scale).clamped(to: 0.6...1.8)
        let objectSize = WorldSize(
            width: MacOverlayProvider.baseSize.width * objectScale,
            height: MacOverlayProvider.baseSize.height * objectScale
        )
        let initialPosition = Self.initialPosition(
            defaults: defaults,
            world: initialWorld,
            objectSize: objectSize
        )

        self.catalog = catalog
        installedPets = descriptors
        asset = initialAsset
        self.selectedBuiltInPet = initialAsset.packageURL == nil ? selectedBuiltInPet : nil
        displays = displaySet.displays
        coordinateSpace = displaySet.coordinateSpace
        world = initialWorld
        tuning = runtimeTuning
        overlay = MacOverlayProvider(
            coordinateSpace: displaySet.coordinateSpace,
            scale: objectScale,
            hitRegionScale: runtimeTuning.hitRegionScale
        )
        movement = MovementController(
            position: initialPosition,
            configuration: MovementConfiguration(
                maximumSpeed: runtimeTuning.walkingSpeed,
                acceleration: 90,
                deceleration: 115
            )
        )
        behavior = BehaviorController(enteredAt: ProcessInfo.processInfo.systemUptime)
        pointerModel = PointerInteractionModel(configuration: runtimeTuning.pointerConfiguration)
        animationPlayer = PetAnimationPlayer(asset: initialAsset)
        isRoamingEnabled = defaults.bool(forKey: DefaultsKey.roaming)
        isPointerAvoidanceEnabled = defaults.bool(forKey: DefaultsKey.avoidPointer)
        areInteractionsEnabled = defaults.bool(forKey: DefaultsKey.interactions)
        nextWanderAt = ProcessInfo.processInfo.systemUptime + 2.2

        super.init()

        // Use the actual owned provider after initialization; the temporary one
        // above only produced immutable startup snapshots.
        let ownedSet = self.displayProvider.snapshotSet()
        if !ownedSet.displays.isEmpty {
            displays = ownedSet.displays
            coordinateSpace = ownedSet.coordinateSpace
            world = DesktopWorldSnapshot(displays: ownedSet.displays)
            overlay.coordinateSpace = ownedSet.coordinateSpace
            let clamped = world.clamp(movement.position, objectSize: overlay.objectSize)
            movement.teleport(to: clamped)
        }
        pointerProvider = MacPointerProvider { [weak self] in
            self?.coordinateSpace ?? displaySet.coordinateSpace
        }
    }

    public func start() {
        guard !running else { return }
        running = true
        overlay.view.delegate = self
        overlay.setPosition(movement.position)
        renderCurrentFrame()
        overlay.setVisible(true)

        screenObserver = NotificationCenter.default.addObserver(
            forName: NSApplication.didChangeScreenParametersNotification,
            object: nil,
            queue: .main
        ) { [weak self] _ in
            MainActor.assumeIsolated { self?.handleDisplayChange() }
        }
        scheduleNextTick(after: 0.02)
    }

    public func stop() {
        guard running else { return }
        running = false
        tickTimer?.invalidate()
        tickTimer = nil
        if let screenObserver {
            NotificationCenter.default.removeObserver(screenObserver)
            self.screenObserver = nil
        }
        overlay.setInteractionEnabled(false)
        persistPosition()
        overlay.setVisible(false)
    }

    public func reloadCatalog() {
        installedPets = catalog.discover()
    }

    @discardableResult
    public func loadPet(at packageURL: URL) -> Result<Void, Error> {
        do {
            let newAsset = try loader.load(packageAt: packageURL)
            install(asset: newAsset)
            selectedBuiltInPet = nil
            UserDefaults.standard.set(packageURL.standardizedFileURL.path, forKey: DefaultsKey.petPackagePath)
            return .success(())
        } catch {
            return .failure(error)
        }
    }

    public func useBuiltInPet(_ kind: BuiltInPetKind) {
        install(asset: MascotPetFactory.make(kind))
        selectedBuiltInPet = kind
        UserDefaults.standard.set(kind.rawValue, forKey: DefaultsKey.builtInPet)
        UserDefaults.standard.removeObject(forKey: DefaultsKey.petPackagePath)
    }

    public func setScale(_ newScale: Double) {
        overlay.setScale(newScale)
        UserDefaults.standard.set(overlay.scale, forKey: DefaultsKey.scale)
        let clamped = world.clamp(movement.position, objectSize: overlay.objectSize)
        movement.teleport(to: clamped, stop: false)
        overlay.setPosition(clamped)
    }

    /// Applies only the values currently exposed for MVP 0/0.5 validation.
    /// Rest and future activity behavior use separate milestone-specific
    /// configuration so this validated interaction preset remains stable.
    public func applyTuning(_ proposed: RuntimeTuning) {
        let normalized = proposed.normalized
        let pauseChanged = abs(normalized.wanderPause - tuning.wanderPause) > 0.001
        tuning = normalized
        pointerModel.configuration = normalized.pointerConfiguration
        pointerModel.reset()
        movement.configuration.maximumSpeed = normalized.walkingSpeed
        overlay.setHitRegionScale(normalized.hitRegionScale)
        if pauseChanged, !movement.hasRoute,
           behavior.state != .caught, behavior.state != .dragged {
            nextWanderAt = ProcessInfo.processInfo.systemUptime
                + normalized.wanderDelay(randomUnit: 0.5)
        }
        persistRuntimeTuning()
    }

    public func resetTuning() {
        applyTuning(.standard)
    }

    public func petOverlayMouseDown(screenPoint: NSPoint) {
        let now = ProcessInfo.processInfo.systemUptime
        guard areInteractionsEnabled, now <= catchArmedUntil else {
            overlay.setInteractionEnabled(false)
            return
        }
        let pointer = corePoint(fromAppKitScreenPoint: screenPoint)
        dragOffset = movement.position - pointer
        isDragging = false
        isEvadeTransitioning = false
        movement.cancelRoute(stop: true)
        behavior.handle(.catchBegan, at: now)
        updateAnimation(pointerDegrees: lastPointerDecision?.lookDirectionDegrees)
    }

    public func petOverlayDragged(screenPoint: NSPoint, distance: CGFloat) {
        guard behavior.state == .caught || behavior.state == .dragged else { return }
        let now = ProcessInfo.processInfo.systemUptime
        if distance > 4 {
            isDragging = true
            behavior.handle(.dragMoved, at: now)
        }
        let pointer = corePoint(fromAppKitScreenPoint: screenPoint)
        movement.teleport(to: pointer + dragOffset)
        overlay.setPosition(movement.position)
        updateAnimation(pointerDegrees: nil)
        renderCurrentFrame()
    }

    public func petOverlayMouseUp(screenPoint: NSPoint, wasDragged: Bool) {
        guard behavior.state == .caught || behavior.state == .dragged else {
            overlay.setInteractionEnabled(false)
            return
        }
        let pointer = corePoint(fromAppKitScreenPoint: screenPoint)
        if wasDragged || isDragging {
            movement.teleport(to: pointer + dragOffset)
        }
        finishDrop(at: ProcessInfo.processInfo.systemUptime)
    }

    @objc private func tickTimerFired() {
        guard running else { return }
        autoreleasepool { tick() }
    }

    private func tick() {
        let now = ProcessInfo.processInfo.systemUptime
        let deltaTime = min(max(now - (lastTickAt ?? now), 0), 0.1)
        lastTickAt = now
        behavior.handle(.tick, at: now)

        let pointer = pointerProvider.currentPointer(at: now)
        let userIdleDuration = userIdleProvider.idleDuration(at: now)
        if userIdleDuration < 0.8, behavior.state.isResting {
            cancelRestForActivity(at: now)
        }
        if (behavior.state == .caught || behavior.state == .dragged), !pointer.primaryButtonDown {
            finishDrop(at: now)
        }

        let decision = pointerModel.evaluate(
            pointer: pointer.position,
            pet: movement.position,
            timestamp: now
        )
        lastPointerDecision = decision

        if decision.shouldArmCatch, areInteractionsEnabled {
            catchArmedUntil = max(catchArmedUntil, now + tuning.catchWindow)
        }
        let catchIsArmed = areInteractionsEnabled && now <= catchArmedUntil

        if behavior.state != .caught && behavior.state != .dragged {
            if catchIsArmed {
                isEvadeTransitioning = false
                movement.cancelRoute(stop: false)
                behavior.handle(.pointer(.catchable), at: now)
                movement.configuration.maximumSpeed = tuning.walkingSpeed
                _ = movement.update(deltaTime: deltaTime)
                nextWanderAt = max(nextWanderAt, now + 1.0)
            } else if isEvadeTransitioning {
                updateEvadeTransition(at: now, deltaTime: deltaTime)
            } else if updateRestLifecycle(
                userIdleDuration: userIdleDuration,
                pointerProximity: decision.proximity,
                pointerPosition: pointer.position,
                at: now,
                deltaTime: deltaTime
            ) {
                // Rest owns movement until input wakes the creature.
            } else if isPointerAvoidanceEnabled {
                behavior.handle(.pointer(decision.proximity), at: now)
                switch decision.proximity {
                case .slowEvade, .fastEvade:
                    applyEvade(decision.escapeVelocity, deltaTime: deltaTime)
                case .watching, .catchable:
                    movement.cancelRoute(stop: false)
                    movement.configuration.maximumSpeed = tuning.walkingSpeed
                    _ = movement.update(deltaTime: deltaTime)
                    nextWanderAt = max(nextWanderAt, now + 0.8)
                case .far:
                    updateRoaming(at: now, deltaTime: deltaTime)
                }
            } else {
                updateRoaming(at: now, deltaTime: deltaTime)
            }
        }

        let catchIsLive = catchIsArmed && overlay.containsPet(atWorldPoint: pointer.position)
        overlay.setInteractionEnabled(
            behavior.state == .caught || behavior.state == .dragged || catchIsLive
        )

        updateAnimation(
            pointerDegrees: behavior.state == .lookAtPointer ? decision.lookDirectionDegrees : nil
        )
        animationPlayer.update(deltaTime: deltaTime)
        overlay.setPosition(movement.position)
        renderCurrentFrame()

        scheduleNextTick(after: preferredTickInterval)
    }

    private var preferredTickInterval: TimeInterval {
        if ProcessInfo.processInfo.systemUptime <= catchArmedUntil { return 1 / 60 }
        return switch behavior.state {
        case .wander, .evadePointer, .caught, .dragged, .dropped, .findSleepSpot:
            1 / 30
        case .lookAtPointer:
            1 / 16
        case .sleep:
            1 / 2
        default:
            1 / 12
        }
    }

    private func scheduleNextTick(after interval: TimeInterval) {
        tickTimer?.invalidate()
        let timer = Timer(
            timeInterval: max(0.01, interval),
            target: self,
            selector: #selector(tickTimerFired),
            userInfo: nil,
            repeats: false
        )
        tickTimer = timer
        RunLoop.main.add(timer, forMode: .common)
    }

    private func updateRestLifecycle(
        userIdleDuration: TimeInterval,
        pointerProximity: PointerProximity,
        pointerPosition: WorldPoint,
        at timestamp: TimeInterval,
        deltaTime: TimeInterval
    ) -> Bool {
        if behavior.state.isResting, pointerProximity != .far {
            cancelRestForActivity(at: timestamp)
            return false
        }

        switch behavior.state {
        case .sit:
            movement.cancelRoute(stop: false)
            _ = movement.update(deltaTime: deltaTime)
            if timestamp - behavior.enteredAt >= restConfiguration.sittingDuration {
                behavior.handle(.seekSleepSpot, at: timestamp)
                beginRestTravel(pointerPosition: pointerPosition, at: timestamp)
            }
            return true

        case .findSleepSpot:
            movement.configuration.maximumSpeed = max(24, tuning.walkingSpeed * 0.75)
            if movement.hasRoute {
                let update = movement.update(deltaTime: deltaTime)
                if update.reachedDestination { enterSleep(at: timestamp) }
            } else {
                enterSleep(at: timestamp)
            }
            return true

        case .sleep:
            movement.cancelRoute(stop: false)
            _ = movement.update(deltaTime: deltaTime)
            return true

        default:
            break
        }

        guard userIdleDuration >= restConfiguration.idleBeforeRest,
              pointerProximity == .far,
              behavior.state == .idle || behavior.state == .wander || behavior.state == .dropped else {
            return false
        }
        isEvadeTransitioning = false
        restDestination = nil
        movement.cancelRoute(stop: false)
        behavior.handle(.beginRest, at: timestamp)
        nextWanderAt = .infinity
        _ = movement.update(deltaTime: deltaTime)
        return true
    }

    private func beginRestTravel(pointerPosition: WorldPoint, at timestamp: TimeInterval) {
        let zones = safeZoneProvider.currentSafeZones(in: world)
        let restWorld = DesktopWorldSnapshot(
            displays: world.displays,
            windows: world.windows,
            pointer: PointerSnapshot(
                position: pointerPosition,
                timestamp: timestamp,
                primaryButtonDown: false
            ),
            focus: world.focus,
            safeZones: zones
        )
        restDestination = BasicSafeZonePlanner.destination(
            in: restWorld,
            currentPosition: movement.position,
            pointerPosition: pointerPosition,
            objectSize: overlay.objectSize
        )

        guard isRoamingEnabled, let restDestination else {
            enterSleep(at: timestamp)
            return
        }
        let route = DisplayTopology(displays: displays).route(
            from: movement.position,
            to: restDestination.point
        )
        movement.configuration.maximumSpeed = max(24, tuning.walkingSpeed * 0.75)
        movement.setRoute(route.waypoints)
        if !movement.hasRoute { enterSleep(at: timestamp) }
    }

    private func enterSleep(at timestamp: TimeInterval) {
        movement.cancelRoute(stop: true)
        behavior.handle(.sleepSpotReached, at: timestamp)
        nextWanderAt = .infinity
        persistPosition()
    }

    private func cancelRestForActivity(at timestamp: TimeInterval) {
        guard behavior.state.isResting else { return }
        restDestination = nil
        movement.cancelRoute(stop: false)
        behavior.handle(.meaningfulActivity, at: timestamp)
        nextWanderAt = timestamp + restConfiguration.wakeWanderDelay
    }

    private func updateRoaming(at timestamp: TimeInterval, deltaTime: TimeInterval) {
        guard isRoamingEnabled else {
            movement.cancelRoute(stop: false)
            movement.configuration.maximumSpeed = tuning.walkingSpeed
            _ = movement.update(deltaTime: deltaTime)
            return
        }

        if movement.hasRoute {
            movement.configuration.maximumSpeed = tuning.walkingSpeed
            let update = movement.update(deltaTime: deltaTime)
            if update.reachedDestination {
                behavior.handle(.arrived, at: timestamp)
                nextWanderAt = timestamp + tuning.wanderDelay(
                    randomUnit: Double.random(in: 0...1)
                )
                persistPosition()
            }
            return
        }

        _ = movement.update(deltaTime: deltaTime)
        guard timestamp >= nextWanderAt else { return }
        beginWander(at: timestamp)
    }

    private func beginWander(at timestamp: TimeInterval) {
        isEvadeTransitioning = false
        guard let destination = randomDestination() else {
            nextWanderAt = timestamp + 2
            return
        }
        let topology = DisplayTopology(displays: displays)
        let route = topology.route(from: movement.position, to: destination)
        movement.configuration.maximumSpeed = tuning.walkingSpeed
        movement.setRoute(route.waypoints)
        behavior.handle(.beginWander, at: timestamp)
        if !movement.hasRoute { nextWanderAt = timestamp + 2 }
    }

    private func randomDestination() -> WorldPoint? {
        guard !displays.isEmpty else { return nil }
        let current = world.display(containing: movement.position) ?? world.nearestDisplay(to: movement.position)
        let target: DisplaySnapshot
        let shouldExploreAnotherDisplay = displays.count > 1
            && Double.random(in: 0...1) < tuning.crossDisplayWanderChance
        if shouldExploreAnotherDisplay {
            let alternatives = displays.filter { $0.id != current?.id }
            target = alternatives.randomElement() ?? displays[0]
        } else {
            target = current ?? displays.randomElement()!
        }

        let safe = target.visibleFrame.insetBy(
            dx: overlay.objectSize.width / 2 + 18,
            dy: overlay.objectSize.height / 2 + 12
        )
        guard !safe.isEmpty else { return target.visibleFrame.center }

        // A cross-display trip ends shortly inside the destination display.
        // Crossing the seam reads clearly, while avoiding another full-screen
        // trek before Roamling finally pauses.
        if target.id != current?.id {
            let boundary = target.visibleFrame.closestPoint(to: movement.position)
            let inward = (target.visibleFrame.center - boundary).normalized
            let depth = Double.random(in: 140...360)
            return safe.closestPoint(to: boundary + inward * depth)
        }

        let x = Double.random(in: safe.minX...safe.maxX)
        let y: Double
        if Double.random(in: 0...1) < 0.72 {
            let upper = max(safe.minY, safe.maxY - min(170, safe.size.height * 0.32))
            y = Double.random(in: upper...safe.maxY)
        } else {
            y = Double.random(in: safe.minY...safe.maxY)
        }
        let sampled = WorldPoint(x: x, y: y)
        let offset = sampled - movement.position
        guard offset.length > 520 else { return sampled }
        let legLength = Double.random(in: 280...520)
        return safe.closestPoint(to: movement.position + offset.normalized * legLength)
    }

    private func applyEvade(_ desiredVelocity: WorldVector, deltaTime: TimeInterval) {
        let topology = DisplayTopology(displays: displays)
        if let transition = topology.evadeTransition(
            from: movement.position,
            direction: desiredVelocity,
            objectSize: overlay.objectSize
        ) {
            isEvadeTransitioning = true
            movement.configuration.maximumSpeed = max(
                tuning.walkingSpeed,
                desiredVelocity.length
            )
            movement.setRoute(transition.waypoints)
            _ = movement.update(deltaTime: deltaTime)
            nextWanderAt = ProcessInfo.processInfo.systemUptime + 1.5
            return
        }

        movement.cancelRoute(stop: false)
        var velocity = desiredVelocity
        let currentDisplay = world.display(containing: movement.position)
            ?? world.nearestDisplay(to: movement.position)
        if let currentDisplay {
            let safe = currentDisplay.visibleFrame.insetBy(
                dx: overlay.objectSize.width / 2,
                dy: overlay.objectSize.height / 2
            )
            let proposed = movement.position + velocity * deltaTime
            if proposed.x < safe.minX || proposed.x > safe.maxX { velocity.dx = 0 }
            if proposed.y < safe.minY || proposed.y > safe.maxY { velocity.dy = 0 }
            if velocity.length < 1 {
                let pointerY = lastPointerDecision?.kinematics.velocity.dy ?? 0
                velocity = WorldVector(dx: 0, dy: pointerY >= 0 ? -desiredVelocity.length : desiredVelocity.length)
            }
            let constrained = safe.closestPoint(to: movement.position + velocity * deltaTime)
            movement.teleport(to: constrained, stop: false)
        } else {
            movement.teleport(to: movement.position + velocity * deltaTime, stop: false)
        }
        movement.configuration.maximumSpeed = max(tuning.walkingSpeed, desiredVelocity.length)
        movement.setVelocity(velocity)
        nextWanderAt = ProcessInfo.processInfo.systemUptime + 1.0
    }

    private func updateEvadeTransition(at timestamp: TimeInterval, deltaTime: TimeInterval) {
        guard movement.hasRoute else {
            isEvadeTransitioning = false
            behavior.handle(.pointer(.far), at: timestamp)
            return
        }
        movement.configuration.maximumSpeed = max(
            tuning.walkingSpeed,
            tuning.pointerConfiguration.fastEvadeSpeed
        )
        let update = movement.update(deltaTime: deltaTime)
        guard update.reachedDestination else { return }
        isEvadeTransitioning = false
        behavior.handle(.pointer(.far), at: timestamp)
        nextWanderAt = timestamp + max(
            1.5,
            tuning.wanderDelay(randomUnit: Double.random(in: 0...1)) * 0.35
        )
        persistPosition()
    }

    private func updateAnimation(pointerDegrees: Double?) {
        let capability: PetCapability
        switch behavior.state {
        case .wander, .evadePointer, .findSleepSpot:
            capability = movement.velocity.dx < 0 ? .moveLeft : .moveRight
        case .sit:
            capability = .sit
        case .lookAtPointer, .observe:
            capability = .observe
        case .caught:
            capability = .caught
        case .dragged:
            capability = .dragged
        case .dropped:
            capability = .landing
        case .work:
            capability = .work
        case .waitingForUser:
            capability = .paw
        case .celebrate:
            capability = .celebrate
        case .sad:
            capability = .fail
        case .sleep:
            capability = .sleep
        case .stretch, .wake:
            capability = .stretch
        default:
            capability = .idle
        }
        animationPlayer.setCapability(capability)
        animationPlayer.setLookDirection(degrees: pointerDegrees)
    }

    private func renderCurrentFrame() {
        overlay.setFrameImage(asset.frameImage(at: animationPlayer.currentFrameIndex))
    }

    private func finishDrop(at timestamp: TimeInterval) {
        isDragging = false
        isEvadeTransitioning = false
        behavior.handle(.mouseReleased, at: timestamp)
        let clamped = world.clamp(movement.position, objectSize: overlay.objectSize)
        movement.teleport(to: clamped)
        overlay.setPosition(clamped)
        overlay.setInteractionEnabled(false)
        catchArmedUntil = 0
        nextWanderAt = timestamp + 1.4
        persistPosition()
    }

    private func handleDisplayChange() {
        let oldAppKitPoint = coordinateSpace.pointToAppKit(movement.position)
        let displaySet = displayProvider.snapshotSet()
        guard !displaySet.displays.isEmpty else { return }
        displays = displaySet.displays
        coordinateSpace = displaySet.coordinateSpace
        world = DesktopWorldSnapshot(displays: displays)
        overlay.coordinateSpace = coordinateSpace
        let newWorldPoint = coordinateSpace.pointFromAppKit(oldAppKitPoint)
        let clamped = world.clamp(newWorldPoint, objectSize: overlay.objectSize)
        isEvadeTransitioning = false
        movement.cancelRoute(stop: true)
        movement.teleport(to: clamped)
        overlay.setPosition(clamped)
        nextWanderAt = ProcessInfo.processInfo.systemUptime + 1
        persistPosition()
    }

    private func install(asset newAsset: PetAsset) {
        asset = newAsset
        animationPlayer = PetAnimationPlayer(asset: newAsset)
        updateAnimation(pointerDegrees: nil)
        renderCurrentFrame()
    }

    private func corePoint(fromAppKitScreenPoint point: NSPoint) -> WorldPoint {
        coordinateSpace.pointFromAppKit(WorldPoint(x: Double(point.x), y: Double(point.y)))
    }

    private func persistPosition() {
        let defaults = UserDefaults.standard
        defaults.set(movement.position.x, forKey: DefaultsKey.positionX)
        defaults.set(movement.position.y, forKey: DefaultsKey.positionY)
        defaults.set(true, forKey: DefaultsKey.hasPosition)
    }

    private func persistRuntimeTuning() {
        guard let data = try? JSONEncoder().encode(tuning) else { return }
        UserDefaults.standard.set(data, forKey: DefaultsKey.runtimeTuning)
    }

    private static func loadRuntimeTuning(defaults: UserDefaults) -> RuntimeTuning {
        guard let data = defaults.data(forKey: DefaultsKey.runtimeTuning),
              let decoded = try? JSONDecoder().decode(RuntimeTuning.self, from: data) else {
            return .standard
        }
        return decoded.normalized
    }

    private static func initialPosition(
        defaults: UserDefaults,
        world: DesktopWorldSnapshot,
        objectSize: WorldSize
    ) -> WorldPoint {
        if defaults.bool(forKey: DefaultsKey.hasPosition) {
            let saved = WorldPoint(
                x: defaults.double(forKey: DefaultsKey.positionX),
                y: defaults.double(forKey: DefaultsKey.positionY)
            )
            return world.clamp(saved, objectSize: objectSize)
        }
        guard let display = world.displays.first else { return .zero }
        let safe = display.visibleFrame.insetBy(
            dx: objectSize.width / 2 + 24,
            dy: objectSize.height / 2 + 18
        )
        return WorldPoint(x: safe.maxX, y: safe.maxY)
    }

    private static func loadInitialAsset(
        descriptors: [PetDescriptor],
        selectedPath: String?,
        builtInPet: BuiltInPetKind,
        loader: PetLoader
    ) -> PetAsset {
        if let selectedPath,
           let selected = descriptors.first(where: {
               $0.packageURL.standardizedFileURL.path == URL(fileURLWithPath: selectedPath).standardizedFileURL.path
           }),
           let loaded = try? loader.load(packageAt: selected.packageURL) {
            return loaded
        }
        return MascotPetFactory.make(builtInPet)
    }
}
