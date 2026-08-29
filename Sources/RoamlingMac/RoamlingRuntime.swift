// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

import AppKit
import RoamlingCore
import RoamlingPet
import RoamlingSources

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
        static let claudeCodeHookToken = "roamling.claudeCodeHookToken"
        static let codexHookToken = "roamling.codexHookToken"
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
    public var claudeCodeIntegrationStatus: ClaudeCodeIntegrationStatus {
        claudeCodeInstaller.status()
    }
    public var claudeCodeReceiverState: ClaudeCodeReceiverState { claudeCodeSource.state }
    public var codexIntegrationStatus: CodexIntegrationStatus { codexInstaller.status() }
    public var codexReceiverState: CodexReceiverState { codexSource.state }

    private let displayProvider = MacDisplayProvider()
    private let safeZoneProvider = MacBasicSafeZoneProvider()
    private let userIdleProvider = MacUserIdleProvider()
    private let captureProvider = MacCaptureProvider()
    private let restConfiguration = RestConfiguration.standard
    private var pointerProvider: MacPointerProvider!
    private var windowProvider: MacWindowProvider!
    private var focusProvider: MacFocusProvider!
    private let catalog: PetCatalog
    private let loader = PetLoader()
    private let overlay: MacOverlayProvider
    private let claudeCodeSource: ClaudeCodeSource
    private let claudeCodeInstaller: ClaudeCodeHookInstaller
    private let codexSource: CodexSource
    private let codexInstaller: CodexHookInstaller

    private var displays: [DisplaySnapshot]
    private var coordinateSpace: DesktopCoordinateSpace
    private var world: DesktopWorldSnapshot
    private var movement: MovementController
    private var behavior: BehaviorController
    private var pointerModel: PointerInteractionModel
    private var animationPlayer: PetAnimationPlayer

    private var tickTimer: Timer?
    private var activityTasks: [Task<Void, Never>] = []
    private var screenObserver: NSObjectProtocol?
    private var lastTickAt: TimeInterval?
    private var nextWanderAt: TimeInterval
    private var catchArmedUntil: TimeInterval = 0
    private var caughtAnimationUntil: TimeInterval = 0
    private var clickReactionUntil: TimeInterval = 0
    private var isClickReactionPending = false
    private var isDragging = false
    private var dragOffset = WorldVector.zero
    private var lastPointerDecision: PointerDecision?
    private var isEvadeTransitioning = false
    private var restDestination: RestDestination?
    private var activityDestination: InterestDestination?
    private var activityHint: LocationHint?
    private var cachedFocus: FocusSnapshot?
    private var focusQueriedAt: TimeInterval = -.infinity
    /// The accessibility query is synchronous, so it runs only when placement
    /// can act on the answer: when an event picks a seat, and on a slow beat
    /// while walking there. A seated pet waits for the next event.
    private static let focusRefreshInterval: TimeInterval = 0.5
    /// Re-routing for anything smaller than this would read as jitter.
    private static let focusReseatThreshold = 24.0
    private var pendingActivityEvent: CompanionEvent?
    private var recentActivityEvents: [String: CompanionEvent] = [:]
    private var attentionModel = AttentionModel()
    private var reactionPolicy = ReactionPolicy()
    private var lastDispatchedActivityEventID: String?
    private var activeActivitySourceID: String?
    private var activeActivityReaction: CompanionReaction?
    private var activityArrivalReaction: CompanionReaction?
    private var running = false

    public override init() {
        let defaults = UserDefaults.standard
        defaults.register(defaults: [
            DefaultsKey.roaming: true,
            DefaultsKey.avoidPointer: true,
            DefaultsKey.interactions: true,
            DefaultsKey.scale: 1.0,
            DefaultsKey.builtInPet: BuiltInPetKind.fatMochi.rawValue
        ])
        let runtimeTuning = Self.loadRuntimeTuning(defaults: defaults)
        let claudeCodeHookToken = Self.loadOrCreateClaudeCodeHookToken(defaults: defaults)
        let codexHookToken = Self.loadOrCreateCodexHookToken(defaults: defaults)

        let catalog = PetCatalog()
        let descriptors = catalog.discover()
        let selectedPath = defaults.string(forKey: DefaultsKey.petPackagePath)
        let selectedBuiltInPet = defaults.string(forKey: DefaultsKey.builtInPet)
            .flatMap(BuiltInPetKind.init(rawValue:)) ?? .fatMochi
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
        claudeCodeSource = ClaudeCodeSource(token: claudeCodeHookToken)
        claudeCodeInstaller = ClaudeCodeHookInstaller(
            settingsURL: FileManager.default.homeDirectoryForCurrentUser
                .appendingPathComponent(".claude/settings.json"),
            token: claudeCodeHookToken
        )
        codexSource = CodexSource(token: codexHookToken)
        codexInstaller = CodexHookInstaller(
            hooksURL: FileManager.default.homeDirectoryForCurrentUser
                .appendingPathComponent(".codex/hooks.json"),
            token: codexHookToken
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
        windowProvider = MacWindowProvider { [weak self] in
            self?.coordinateSpace ?? displaySet.coordinateSpace
        }
        focusProvider = MacFocusProvider { [weak self] in
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
        startActivitySources()
        scheduleNextTick(after: 0.02)
    }

    public func stop() {
        guard running else { return }
        running = false
        tickTimer?.invalidate()
        tickTimer = nil
        activityTasks.forEach { $0.cancel() }
        activityTasks.removeAll()
        claudeCodeSource.stop()
        codexSource.stop()
        if let screenObserver {
            NotificationCenter.default.removeObserver(screenObserver)
            self.screenObserver = nil
        }
        clickReactionUntil = 0
        isClickReactionPending = false
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

    @discardableResult
    public func installClaudeCodeIntegration() -> Result<Void, Error> {
        do {
            try claudeCodeInstaller.install()
            return .success(())
        } catch {
            return .failure(error)
        }
    }

    /// Upgrades an existing install to the current handler shape without asking
    /// again. Returns nil when there is nothing to repair, so a machine that
    /// never opted in keeps an untouched `settings.json`.
    @discardableResult
    public func repairClaudeCodeIntegrationIfNeeded() -> Result<Void, Error>? {
        guard claudeCodeInstaller.status() == .needsRepair else { return nil }
        return installClaudeCodeIntegration()
    }

    @discardableResult
    public func removeClaudeCodeIntegration() -> Result<Void, Error> {
        do {
            try claudeCodeInstaller.remove()
            return .success(())
        } catch {
            return .failure(error)
        }
    }

    @discardableResult
    public func installCodexIntegration() -> Result<Void, Error> {
        do {
            try codexInstaller.install()
            return .success(())
        } catch {
            return .failure(error)
        }
    }

    @discardableResult
    public func removeCodexIntegration() -> Result<Void, Error> {
        do {
            try codexInstaller.remove()
            return .success(())
        } catch {
            return .failure(error)
        }
    }

    public func testClaudeCodeReaction() {
        testAgentReaction(sourceID: "claude-code:test")
    }

    public func testCodexReaction() {
        testAgentReaction(sourceID: "codex:test")
    }

    private func testAgentReaction(sourceID: String) {
        handleActivityEvent(CompanionEvent(
            sourceID: sourceID,
            sourceType: .agent,
            timestamp: ProcessInfo.processInfo.systemUptime,
            kind: .activityStarted,
            intensity: 0.55,
            context: .working,
            locationHint: windowProvider.currentActivityLocationHint()
        ))
        DispatchQueue.main.asyncAfter(deadline: .now() + 3) { [weak self] in
            guard let self else { return }
            self.handleActivityEvent(CompanionEvent(
                sourceID: sourceID,
                sourceType: .agent,
                timestamp: ProcessInfo.processInfo.systemUptime,
                kind: .achievement,
                intensity: 0.55,
                context: .working
            ))
        }
    }

    public func petOverlayMouseDown(screenPoint: NSPoint) {
        let now = ProcessInfo.processInfo.systemUptime
        guard areInteractionsEnabled, now <= catchArmedUntil else {
            overlay.setInteractionEnabled(false)
            return
        }
        let pointer = corePoint(fromAppKitScreenPoint: screenPoint)
        dragOffset = movement.position - pointer
        clickReactionUntil = 0
        isClickReactionPending = false
        isDragging = false
        isEvadeTransitioning = false
        movement.cancelRoute(stop: true)
        behavior.handle(.catchBegan, at: now)
        caughtAnimationUntil = now + caughtTransitionDuration
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
        let now = ProcessInfo.processInfo.systemUptime
        let pointer = corePoint(fromAppKitScreenPoint: screenPoint)
        if wasDragged || isDragging {
            movement.teleport(to: pointer + dragOffset)
            finishDrop(at: now)
            return
        }

        // A click has the same caught -> four-paw scramble response as a drag.
        // Release panel ownership immediately so the reaction never blocks the
        // underlying app, then finish with the normal landing after one loop.
        behavior.handle(.dragMoved, at: now)
        isClickReactionPending = true
        clickReactionUntil = max(now, caughtAnimationUntil) + draggedCycleDuration
        let clamped = world.clamp(movement.position, objectSize: overlay.objectSize)
        movement.teleport(to: clamped)
        overlay.setPosition(clamped)
        overlay.setInteractionEnabled(false)
        catchArmedUntil = 0
        updateAnimation(pointerDegrees: nil)
        renderCurrentFrame()
        if running { scheduleNextTick(after: 1 / 30) }
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
        resumePendingActivityIfReady(at: now)

        let pointer = pointerProvider.currentPointer(at: now)
        let userIdleDuration = userIdleProvider.idleDuration(at: now)
        if userIdleDuration < 0.8, behavior.state.isResting {
            cancelRestForActivity(at: now)
        }
        if (behavior.state == .caught || behavior.state == .dragged), !pointer.primaryButtonDown {
            if !isClickReactionPending || now >= clickReactionUntil {
                finishDrop(at: now)
            }
        }

        let decision = pointerModel.evaluate(
            pointer: pointer.position,
            pet: movement.position,
            timestamp: now
        )
        lastPointerDecision = decision

        if !isClickReactionPending, decision.shouldArmCatch, areInteractionsEnabled {
            catchArmedUntil = max(catchArmedUntil, now + tuning.catchWindow)
        }
        let catchIsArmed = !isClickReactionPending
            && areInteractionsEnabled
            && now <= catchArmedUntil

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
                    if !updateActivityLifecycle(at: now, deltaTime: deltaTime) {
                        updateRoaming(at: now, deltaTime: deltaTime)
                    }
                }
            } else {
                if !updateActivityLifecycle(at: now, deltaTime: deltaTime) {
                    updateRoaming(at: now, deltaTime: deltaTime)
                }
            }
        }

        let catchIsLive = catchIsArmed && overlay.containsPet(atWorldPoint: pointer.position)
        let ownsPointer = (behavior.state == .caught || behavior.state == .dragged)
            && !isClickReactionPending
        overlay.setInteractionEnabled(ownsPointer || catchIsLive)

        updateAnimation(
            pointerDegrees: behavior.state == .lookAtPointer ? decision.lookDirectionDegrees : nil
        )
        animationPlayer.update(deltaTime: deltaTime * locomotionAnimationRate)
        overlay.setPosition(movement.position)
        renderCurrentFrame()

        scheduleNextTick(after: preferredTickInterval)
    }

    /// Only the states that actually travel at the tuned walking speed scale
    /// their cadence. Evade and catch run on their own speeds and keep the
    /// authored timing.
    private var locomotionAnimationRate: Double {
        switch behavior.state {
        case .wander, .findSleepSpot, .travelToInterest:
            tuning.locomotionAnimationRate
        default:
            1
        }
    }

    private var preferredTickInterval: TimeInterval {
        if ProcessInfo.processInfo.systemUptime <= catchArmedUntil { return 1 / 60 }
        return switch behavior.state {
        case .wander, .evadePointer, .findSleepSpot, .travelToInterest:
            1 / 60
        case .caught, .dragged, .dropped, .wake, .stretch:
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

    private func startActivitySources() {
        startActivitySource(
            start: claudeCodeSource.start,
            stream: claudeCodeSource.makeEventStream()
        )
        startActivitySource(
            start: codexSource.start,
            stream: codexSource.makeEventStream()
        )
    }

    private func startActivitySource(
        start: () throws -> Void,
        stream: AsyncStream<CompanionEvent>
    ) {
        do {
            try start()
            activityTasks.append(Task { [weak self] in
                for await event in stream {
                    guard !Task.isCancelled else { break }
                    self?.handleActivityEvent(event)
                }
            })
        } catch {
            // Receiver state is shown in the menu. Both integrations swallow
            // loopback delivery failures and never block agent work.
        }
    }

    private func handleActivityEvent(_ event: CompanionEvent) {
        let now = ProcessInfo.processInfo.systemUptime
        let shouldLocate = event.kind == .activityStarted
            || event.kind == .highIntensity
            || event.kind == .attentionRequired
        let hint = event.locationHint
            ?? (shouldLocate ? windowProvider.currentActivityLocationHint() : nil)
        let located = CompanionEvent(
            id: event.id,
            sourceID: event.sourceID,
            sourceType: event.sourceType,
            timestamp: event.timestamp,
            kind: event.kind,
            intensity: event.intensity,
            context: event.context,
            locationHint: hint,
            metadata: event.metadata
        )

        // Routine tool completions are useful as adapter-level evidence but do
        // not deserve attention changes or visible reactions on their own.
        if located.kind == .positive, located.intensity < 0.15 { return }

        if located.kind == .activityEnded || located.kind == .idle {
            recentActivityEvents.removeValue(forKey: located.sourceID)
            if attentionModel.currentSourceID == located.sourceID {
                attentionModel.clear(at: now)
                lastDispatchedActivityEventID = nil
            }
            if activeActivitySourceID == located.sourceID {
                clearActiveActivity(at: now)
                applyActivityReaction(.calm, at: now)
            }
            queueNextAttentionCandidate(at: now)
            return
        }

        recentActivityEvents[located.sourceID] = located
        guard let selected = attentionModel.select(
            from: Array(recentActivityEvents.values),
            at: now
        ), selected.id != lastDispatchedActivityEventID else { return }

        if behavior.state == .caught || behavior.state == .dragged {
            pendingActivityEvent = selected
            return
        }
        if behavior.state.isResting {
            cancelRestForActivity(at: now)
            pendingActivityEvent = selected
            return
        }
        dispatchActivityEvent(selected, at: now)
    }

    private func resumePendingActivityIfReady(at timestamp: TimeInterval) {
        guard behavior.state == .idle, let event = pendingActivityEvent else { return }
        pendingActivityEvent = nil
        dispatchActivityEvent(event, at: timestamp)
    }

    private func dispatchActivityEvent(_ event: CompanionEvent, at timestamp: TimeInterval) {
        lastDispatchedActivityEventID = event.id
        let reaction = reactionPolicy.reaction(
            for: event,
            context: event.context ?? .idle,
            currentBehavior: behavior.state,
            randomUnit: Double.random(in: 0..<1),
            at: timestamp
        )

        switch event.kind {
        case .activityStarted, .highIntensity:
            activeActivitySourceID = event.sourceID
            let sustained = event.kind == .highIntensity || event.intensity >= 0.65
                ? CompanionReaction.work
                : .observe
            activeActivityReaction = sustained
            beginActivityTravelIfPossible(
                for: event,
                arrivalReaction: reaction ?? sustained,
                at: timestamp
            )

        case .attentionRequired:
            activeActivitySourceID = event.sourceID
            activeActivityReaction = .observe
            beginActivityTravelIfPossible(
                for: event,
                arrivalReaction: reaction ?? .paw,
                at: timestamp
            )

        case .positive:
            if let reaction { applyActivityReaction(reaction, at: timestamp) }

        case .achievement:
            clearActiveActivity(at: timestamp)
            applyActivityReaction(reaction ?? .glance, at: timestamp)
            finishTransientActivityEvent(event, at: timestamp)

        case .negative:
            clearActiveActivity(at: timestamp)
            applyActivityReaction(reaction ?? .sad, at: timestamp)
            finishTransientActivityEvent(event, at: timestamp)

        case .setback:
            activeActivitySourceID = event.sourceID
            activeActivityReaction = .observe
            activityDestination = nil
            activityHint = nil
            movement.cancelRoute(stop: false)
            applyActivityReaction(reaction ?? .sad, at: timestamp)

        case .activityEnded, .calm, .idle:
            if event.kind == .calm,
               activeActivitySourceID == nil || activeActivitySourceID == event.sourceID {
                clearActiveActivity(at: timestamp)
                applyActivityReaction(reaction ?? .calm, at: timestamp)
            }
        }
    }

    private func finishTransientActivityEvent(
        _ event: CompanionEvent,
        at timestamp: TimeInterval
    ) {
        recentActivityEvents.removeValue(forKey: event.sourceID)
        attentionModel.clear(at: timestamp)
        queueNextAttentionCandidate(at: timestamp)
    }

    /// Accessibility stays off until the user turns it on from the menu, so the
    /// app never prompts merely because it launched.
    public var isAccessibilityAuthorized: Bool { focusProvider?.isAuthorized ?? false }

    @discardableResult
    public func requestAccessibilityAuthorization() -> Bool {
        focusProvider?.requestAuthorization() ?? false
    }

    /// Visual placement stays off until the user turns it on from the menu.
    public var isScreenCaptureAuthorized: Bool { captureProvider.isAuthorized }

    @discardableResult
    public func requestScreenCaptureAuthorization() -> Bool {
        captureProvider.requestAuthorization()
    }

    private func refreshedFocus(at timestamp: TimeInterval, force: Bool) -> FocusSnapshot? {
        guard focusProvider?.isAuthorized == true else {
            // Revoking the permission has to take effect on the next event, not
            // leave a stale caret behind.
            cachedFocus = nil
            return nil
        }
        guard force || timestamp - focusQueriedAt >= Self.focusRefreshInterval else {
            return cachedFocus
        }
        focusQueriedAt = timestamp
        cachedFocus = focusProvider.currentFocus()
        return cachedFocus
    }

    private func planningWorld(focus: FocusSnapshot?) -> DesktopWorldSnapshot {
        guard let focus else { return world }
        return DesktopWorldSnapshot(displays: world.displays, focus: focus)
    }

    /// Walking to a seat takes a moment and the caret can move while it does.
    private func reseatForFocusIfNeeded(at timestamp: TimeInterval) {
        guard timestamp - focusQueriedAt >= Self.focusRefreshInterval,
              let hint = activityHint,
              let current = activityDestination,
              let focus = refreshedFocus(at: timestamp, force: true),
              let updated = BasicInterestPositionPlanner.destination(
                for: hint,
                in: planningWorld(focus: focus),
                currentPosition: movement.position,
                pointerPosition: pointerProvider.currentPointer(at: timestamp).position,
                objectSize: overlay.objectSize
              ),
              updated.point.distance(to: current.point) > Self.focusReseatThreshold else { return }

        activityDestination = updated
        movement.setRoute(DisplayTopology(displays: displays).route(
            from: movement.position,
            to: updated.point
        ).waypoints)
    }

    private func queueNextAttentionCandidate(at timestamp: TimeInterval) {
        guard let next = attentionModel.select(
            from: Array(recentActivityEvents.values),
            at: timestamp
        ) else {
            pendingActivityEvent = nil
            return
        }
        pendingActivityEvent = next.id == lastDispatchedActivityEventID ? nil : next
    }

    private func beginActivityTravelIfPossible(
        for event: CompanionEvent,
        arrivalReaction: CompanionReaction,
        at timestamp: TimeInterval
    ) {
        guard let hint = event.locationHint,
              let destination = BasicInterestPositionPlanner.destination(
                for: hint,
                in: planningWorld(focus: refreshedFocus(at: timestamp, force: true)),
                currentPosition: movement.position,
                pointerPosition: pointerProvider.currentPointer(at: timestamp).position,
                objectSize: overlay.objectSize
              ) else {
            applyActivityReaction(arrivalReaction, at: timestamp)
            return
        }

        let route = DisplayTopology(displays: displays).route(
            from: movement.position,
            to: destination.point
        )
        guard !route.waypoints.isEmpty,
              movement.position.distance(to: destination.point) > 18 else {
            applyActivityReaction(arrivalReaction, at: timestamp)
            return
        }
        isEvadeTransitioning = false
        restDestination = nil
        activityDestination = destination
        activityHint = hint
        activityArrivalReaction = arrivalReaction
        movement.configuration.maximumSpeed = tuning.walkingSpeed
        movement.setRoute(route.waypoints)
        behavior.handle(.beginInterestTravel, at: timestamp)
        nextWanderAt = .infinity
    }

    private func updateActivityLifecycle(
        at timestamp: TimeInterval,
        deltaTime: TimeInterval
    ) -> Bool {
        if activityDestination != nil {
            reseatForFocusIfNeeded(at: timestamp)
        }

        if let destination = activityDestination {
            if !movement.hasRoute {
                let route = DisplayTopology(displays: displays).route(
                    from: movement.position,
                    to: destination.point
                )
                movement.setRoute(route.waypoints)
            }
            behavior.handle(.beginInterestTravel, at: timestamp)
            movement.configuration.maximumSpeed = tuning.walkingSpeed
            let update = movement.update(deltaTime: deltaTime)
            if update.reachedDestination {
                activityDestination = nil
                activityHint = nil
                let reaction = activityArrivalReaction ?? activeActivityReaction ?? .observe
                activityArrivalReaction = nil
                applyActivityReaction(reaction, at: timestamp)
                persistPosition()
            }
            return true
        }

        guard activeActivitySourceID != nil else { return false }
        movement.cancelRoute(stop: false)
        movement.configuration.maximumSpeed = tuning.walkingSpeed
        _ = movement.update(deltaTime: deltaTime)
        switch behavior.state {
        case .observe, .work, .waitingForUser, .celebrate, .sad:
            break
        case .wake, .stretch, .caught, .dragged:
            break
        default:
            applyActivityReaction(activeActivityReaction ?? .observe, at: timestamp)
        }
        return true
    }

    private func applyActivityReaction(
        _ reaction: CompanionReaction,
        at timestamp: TimeInterval
    ) {
        behavior.handle(.reaction(reaction), at: timestamp)
        movement.cancelRoute(stop: false)
        nextWanderAt = activeActivitySourceID == nil ? timestamp + 2.0 : .infinity
    }

    private func clearActiveActivity(at timestamp: TimeInterval) {
        activeActivitySourceID = nil
        activeActivityReaction = nil
        activityArrivalReaction = nil
        activityDestination = nil
        activityHint = nil
        movement.cancelRoute(stop: false)
        nextWanderAt = timestamp + 2.0
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
              activeActivitySourceID == nil,
              activityDestination == nil,
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
        // Claim the walking state before laying a route. Setting the route
        // first left it in place when the state machine refused the
        // transition, so the pet walked the whole leg animated as whatever it
        // had been doing -- observe frames, which are the idle frames.
        guard behavior.handle(.beginWander, at: timestamp).to == .wander else {
            nextWanderAt = timestamp + 2
            return
        }
        let topology = DisplayTopology(displays: displays)
        let route = topology.route(from: movement.position, to: destination)
        movement.configuration.maximumSpeed = tuning.walkingSpeed
        movement.setRoute(route.waypoints)
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
        let capability = PetCapabilityMapping.capability(
            for: behavior.state,
            velocityDX: movement.velocity.dx,
            isCaughtTransitionActive: ProcessInfo.processInfo.systemUptime < caughtAnimationUntil
        )
        animationPlayer.setCapability(capability)
        animationPlayer.setLookDirection(degrees: pointerDegrees)
    }

    private func renderCurrentFrame() {
        overlay.setFrameImage(asset.frameImage(at: animationPlayer.currentFrameIndex))
    }

    private func finishDrop(at timestamp: TimeInterval) {
        isDragging = false
        isClickReactionPending = false
        isEvadeTransitioning = false
        caughtAnimationUntil = 0
        clickReactionUntil = 0
        behavior.handle(.mouseReleased, at: timestamp)
        let clamped = world.clamp(movement.position, objectSize: overlay.objectSize)
        movement.teleport(to: clamped)
        overlay.setPosition(clamped)
        overlay.setInteractionEnabled(false)
        catchArmedUntil = 0
        nextWanderAt = timestamp + 1.4
        persistPosition()
    }

    private var caughtTransitionDuration: TimeInterval {
        guard let track = asset.resolver.resolve(.caught), !track.loops else { return 0 }
        return min(track.frames.reduce(0) { $0 + $1.duration }, 0.8)
    }

    private var draggedCycleDuration: TimeInterval {
        guard let track = asset.resolver.resolve(.dragged) else { return 0.4 }
        let duration = track.frames.reduce(0) { $0 + $1.duration }
        return min(max(duration, 0.3), 0.8)
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
        caughtAnimationUntil = 0
        clickReactionUntil = 0
        isClickReactionPending = false
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

    private static func loadOrCreateClaudeCodeHookToken(defaults: UserDefaults) -> String {
        loadOrCreateHookToken(key: DefaultsKey.claudeCodeHookToken, defaults: defaults)
    }

    private static func loadOrCreateCodexHookToken(defaults: UserDefaults) -> String {
        loadOrCreateHookToken(key: DefaultsKey.codexHookToken, defaults: defaults)
    }

    private static func loadOrCreateHookToken(key: String, defaults: UserDefaults) -> String {
        if let existing = defaults.string(forKey: key),
           existing.count >= 24 {
            return existing
        }
        let token = UUID().uuidString.replacingOccurrences(of: "-", with: "")
            + UUID().uuidString.replacingOccurrences(of: "-", with: "")
        defaults.set(token, forKey: key)
        return token
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
