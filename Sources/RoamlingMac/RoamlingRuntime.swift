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
    public var petCoverage: AnimationResolver.Coverage { asset.resolver.coverage }
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
    /// The window the pet is watching. It outlives any one walk: a parked pet
    /// still has to know which window to judge its seat against.
    private var activityHint: LocationHint?
    private var placement = PlacementDirector()
    private var cachedFocus: FocusSnapshot?
    private var focusQueriedAt: TimeInterval = -.infinity
    private var cachedLuminance: LuminanceField?
    private var luminanceCapturedAt: TimeInterval = -.infinity
    private var luminanceTask: Task<Void, Never>?
    /// A screen capture is far heavier than an accessibility query and the
    /// desktop rarely changes shape between seats, so it refreshes slowly.
    private static let luminanceRefreshInterval: TimeInterval = 3
    /// A sleeping pet still has to notice text arriving underneath it, because
    /// the screen keeps changing while the user is away. It just does not need
    /// to spend a capture as often to find out.
    private static let restingLuminanceRefreshInterval: TimeInterval = 6
    /// A parked pet between walks has to notice the user scrolling text under
    /// it, which is most of its life. Half the rate of an agent seat: nothing
    /// here is urgent, and one capture measures about 60 ms.
    private static let roamingLuminanceRefreshInterval: TimeInterval = 6
    /// The accessibility query is synchronous, so it runs on a slow beat rather
    /// than per frame. `PlacementDirector` reviews a seat on a beat of its own,
    /// and asking more often than it looks buys nothing.
    private static let focusRefreshInterval: TimeInterval = 0.5
    /// States where the pointer, not placement, decides where the pet goes.
    private static let pointerOwnedStates: Set<BehaviorState> = [
        .caught, .dragged, .evadePointer, .lookAtPointer
    ]
    /// Destinations offered to the emptiness score before a stroll. Enough to
    /// usually find a clear one, few enough that roaming stays aimless.
    private static let wanderCandidateCount = 6
    private var pendingActivityEvent: CompanionEvent?
    private var recentActivityEvents: [String: CompanionEvent] = [:]
    private var attentionModel = AttentionModel()
    private var reactionPolicy = ReactionPolicy()
    private var lastDispatchedActivityEventID: String?
    private var activeActivitySourceID: String?
    /// When the active source last said anything, so a watch that will never
    /// be ended by a hook can end on its own.
    private var activityHeardAt: TimeInterval = 0
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
        luminanceTask?.cancel()
        luminanceTask = nil
        cachedLuminance = nil
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
        expireSilentActivity(at: now)
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
            // Gather, decide, apply. The decision runs every tick even when
            // something else owns the pet, so the seat verdict is never stale by
            // the time placement is allowed to act on it. Gating the judging
            // along with the moving is what froze placement next to the cursor.
            let intent = placement.decide(makeSituation(
                at: now,
                pointer: pointer.position,
                proximity: decision.proximity,
                catchIsArmed: catchIsArmed,
                userIdleDuration: userIdleDuration
            ))

            // Whether a seat was chosen with a capture in hand is the first
            // question when the pet parks somewhere odd, and it is not
            // answerable from outside the app.
            record(
                "capture",
                captureProvider.isAuthorized
                    ? (cachedLuminance == nil ? "authorised, none yet" : "available")
                    : "not authorised",
                at: now
            )
            record("pet", behavior.state.rawValue, at: now)
            record("place", Self.describe(intent), at: now)
            record(
                "agent",
                activeActivitySourceID.map {
                    "\($0) window=\(activityHint == nil ? "none" : "found")"
                } ?? "none",
                at: now
            )

            if catchIsArmed {
                isEvadeTransitioning = false
                movement.cancelRoute(stop: false)
                behavior.handle(.pointer(.catchable), at: now)
                movement.configuration.maximumSpeed = tuning.walkingSpeed
                _ = movement.update(deltaTime: deltaTime)
                nextWanderAt = max(nextWanderAt, now + 1.0)
            } else if isEvadeTransitioning {
                updateEvadeTransition(at: now, deltaTime: deltaTime)
            } else if intent.travelReason != nil, behavior.state.isResting {
                // Stepping out from under the user's text is the one thing that
                // outranks a nap, and the only reason placement may end one.
                cancelRestForActivity(at: now)
                apply(intent, at: now, deltaTime: deltaTime)
            } else if updateRestLifecycle(
                userIdleDuration: userIdleDuration,
                pointerProximity: decision.proximity,
                pointerPosition: pointer.position,
                mayNapOnSeat: intent == .sleepInPlace,
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
                    apply(intent, at: now, deltaTime: deltaTime)
                }
            } else {
                apply(intent, at: now, deltaTime: deltaTime)
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
    ///
    /// Watching the pointer scales too, but on distance rather than on tuning:
    /// the closer the pointer, the faster the pet's tail goes.
    private var locomotionAnimationRate: Double {
        switch behavior.state {
        case .wander, .findSleepSpot, .travelToInterest:
            tuning.locomotionAnimationRate
        case .lookAtPointer:
            lastPointerDecision?.attentionRate ?? 1
        default:
            1
        }
    }

    /// Rest timing follows the tuning panel, so changing the idle threshold
    /// takes effect on the next tick rather than at the next launch.
    private var restConfiguration: RestConfiguration { tuning.restConfiguration }

    /// Whether the pet is on duty, which takes a window and not merely a source.
    ///
    /// Both roaming and rest stand down while this is true, so keying it on the
    /// source alone froze the pet outright whenever an event arrived without a
    /// window to watch: nothing to sit beside, nowhere to stroll, and no seat
    /// for the decision table to call worth sleeping on. An agent it cannot
    /// locate is not a reason for the pet to stand still.
    private var isWatchingWindow: Bool {
        activeActivitySourceID != nil && activityHint != nil
    }

    /// Why the pet is doing what it is doing, kept in memory and copyable from
    /// the menu. Standing and sitting look identical from outside the app, so
    /// without this the only way to tell them apart was to add a log and ship a
    /// build.
    private var diagnostics = DiagnosticsLog()
    /// Mirrors the same entries to a file when a path is set, for a session too
    /// long to hold in the buffer.
    ///
    ///     defaults write dev.roamling.app roamling.diagnosticsLog /tmp/pet.log
    private static let diagnosticsPath: String? =
        ProcessInfo.processInfo.environment["ROAMLING_REST_LOG"]
            ?? UserDefaults.standard.string(forKey: "roamling.diagnosticsLog")

    public var diagnosticsText: String {
        diagnostics.text(now: ProcessInfo.processInfo.systemUptime)
    }

    private func record(_ category: String, _ message: String, at timestamp: TimeInterval) {
        guard diagnostics.record(category, message, at: timestamp) else { return }
        guard let path = Self.diagnosticsPath,
              let entry = diagnostics.entries.last,
              let data = String(
                format: "%.1f %@ %@\n", entry.timestamp, entry.category, entry.message
              ).data(using: .utf8) else { return }
        if let handle = FileHandle(forWritingAtPath: path) {
            handle.seekToEndOfFile()
            try? handle.write(contentsOf: data)
            try? handle.close()
        } else {
            try? data.write(to: URL(fileURLWithPath: path))
        }
    }

    private func recordRestGate(
        userIdleDuration: TimeInterval,
        pointerProximity: PointerProximity,
        mayNapOnSeat: Bool,
        at timestamp: TimeInterval
    ) {
        let blocked =
            userIdleDuration < restConfiguration.idleBeforeRest ? "waiting for user idle"
            : placement.isTravelling ? "travelling"
            : (isWatchingWindow && !mayNapOnSeat) ? "on duty, seat not nappable"
            : pointerProximity != .far ? "pointer \(pointerProximity)"
            : !BehaviorController.restEntryStates.contains(behavior.state)
                ? "state \(behavior.state.rawValue)"
            : "clear to rest"
        record("rest", blocked, at: timestamp)
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

    private static func describe(_ intent: PlacementIntent) -> String {
        switch intent {
        case .none: "none, something else owns the pet"
        case .hold: "hold"
        case .sleepInPlace: "sleep in place"
        case let .stroll(point): String(format: "stroll to %.0f,%.0f", point.x, point.y)
        case let .travel(destination, reason):
            String(
                format: "travel %@ to %.0f,%.0f",
                reason.rawValue, destination.point.x, destination.point.y
            )
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
            // An agent emits an event per tool call. Waking for each of them
            // would mean the pet can doze for one beat and never longer, so
            // only events the user would want to be shown get it up.
            guard Self.deservesWakingFromRest(selected) else { return }
            cancelRestForActivity(at: now)
            pendingActivityEvent = selected
            return
        }
        dispatchActivityEvent(selected, at: now)
    }

    /// Routine progress is what the pet is already sitting next to. Only a
    /// result or a request for the user is worth interrupting a nap for.
    private static func deservesWakingFromRest(_ event: CompanionEvent) -> Bool {
        switch event.kind {
        case .attentionRequired, .achievement, .negative, .setback:
            true
        case .activityStarted, .inspecting, .highIntensity, .positive, .calm, .idle, .activityEnded:
            false
        }
    }

    /// A Stop hook cannot run for a session that was interrupted or killed, and
    /// driving agents from a GUI is exactly how that happens. Without this the
    /// pet stays on duty forever: never roaming, and able to sleep only while
    /// its seat keeps scoring clear.
    private func expireSilentActivity(at timestamp: TimeInterval) {
        guard activeActivitySourceID != nil,
              ActivityLifetime.hasFallenSilent(
                lastEventAt: activityHeardAt,
                now: timestamp
              ) else { return }
        clearActiveActivity(at: timestamp)
        applyActivityReaction(.calm, at: timestamp)
    }

    private func resumePendingActivityIfReady(at timestamp: TimeInterval) {
        guard behavior.state == .idle, let event = pendingActivityEvent else { return }
        pendingActivityEvent = nil
        dispatchActivityEvent(event, at: timestamp)
    }

    private func dispatchActivityEvent(_ event: CompanionEvent, at timestamp: TimeInterval) {
        lastDispatchedActivityEventID = event.id
        if activeActivitySourceID == nil || activeActivitySourceID == event.sourceID {
            activityHeardAt = timestamp
        }
        let reaction = reactionPolicy.reaction(
            for: event,
            context: event.context ?? .idle,
            currentBehavior: behavior.state,
            randomUnit: Double.random(in: 0..<1),
            at: timestamp
        )

        switch event.kind {
        case .activityStarted:
            // The hop is the whole reaction. What the pet wears afterwards is
            // stillness, because Petdex's `jumping` is a duration state and the
            // next hook event -- a tool starting -- is a beat away.
            beginWatching(event, sustained: .observe, reaction: reaction ?? .spark, at: timestamp)

        case .inspecting:
            beginWatching(event, sustained: .observe, reaction: reaction ?? .observe, at: timestamp)

        case .highIntensity:
            beginWatching(event, sustained: .work, reaction: reaction ?? .work, at: timestamp)

        case .attentionRequired:
            // The paw is sustained too: the agent stays blocked until the user
            // answers, so the pet has to keep asking rather than drift off it.
            beginWatching(event, sustained: .paw, reaction: reaction ?? .paw, at: timestamp)

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
            activityHeardAt = timestamp
            activeActivityReaction = .observe
            // The trip is off but the window is still the one being watched, so
            // the hint stays and the seat keeps being judged where the pet is.
            placement.settleInPlace(sourceID: event.sourceID, at: timestamp)
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

    /// The capture the pet is allowed to judge with right now.
    ///
    /// Revoking screen recording has to take effect on the next decision rather
    /// than leave the pet trusting an old capture, and both callers -- seat
    /// planning and the nap check -- have to read that the same way.
    private var judgeableLuminance: LuminanceField? {
        captureProvider.isAuthorized ? cachedLuminance : nil
    }

    private func planningWorld(focus: FocusSnapshot?) -> DesktopWorldSnapshot {
        let luminance = judgeableLuminance
        guard focus != nil || luminance != nil else { return world }
        return DesktopWorldSnapshot(
            displays: world.displays,
            focus: focus,
            luminance: luminance
        )
    }

    /// Captures off the main thread and lets the travelling reseat check pick the
    /// result up. Placement never waits on a snapshot, so a slow or failed
    /// capture costs nothing but the visual term.
    private func requestLuminanceRefresh(
        at timestamp: TimeInterval,
        near region: WorldRect,
        every requested: TimeInterval
    ) {
        let interval = behavior.state.isResting
            ? max(requested, Self.restingLuminanceRefreshInterval)
            : requested
        guard captureProvider.isAuthorized,
              luminanceTask == nil,
              timestamp - luminanceCapturedAt >= interval,
              let display = world.display(containing: region.center)
                ?? world.nearestDisplay(to: region.center) else { return }
        luminanceCapturedAt = timestamp
        luminanceTask = Task { [weak self] in
            guard let provider = self?.captureProvider else { return }
            let field = await provider.captureLuminanceField(for: display)
            self?.cachedLuminance = field
            self?.luminanceTask = nil
        }
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

    /// Records the window to watch and the reaction the user is owed. Where the
    /// pet stands to watch it is the director's answer, on the next tick.
    private func beginWatching(
        _ event: CompanionEvent,
        sustained: CompanionReaction,
        reaction: CompanionReaction,
        at timestamp: TimeInterval
    ) {
        if let hint = event.locationHint {
            activityHint = hint
            if let region = hint.approximateRegion {
                requestLuminanceRefresh(
                    at: timestamp,
                    near: region,
                    every: Self.luminanceRefreshInterval
                )
            }
        } else if activeActivitySourceID != event.sourceID {
            // A different agent arriving without a window to point at: the last
            // one's window is not evidence about this one, and leaving it in
            // place would walk the pet to the wrong screen.
            activityHint = nil
        }
        activeActivitySourceID = event.sourceID
        activityHeardAt = timestamp
        activeActivityReaction = sustained
        guard activityHint != nil else {
            // Nothing to walk to, so the reaction plays where the pet is.
            activityArrivalReaction = nil
            applyActivityReaction(reaction, at: timestamp)
            return
        }
        activityArrivalReaction = reaction
    }

    /// Collects everything the placement decision is allowed to look at.
    ///
    /// Nothing here decides anything. Gathering is the adapter's whole job on
    /// this path, which is why the fields it fills are inputs to one function
    /// rather than state four code paths write to and read from each other.
    private func makeSituation(
        at timestamp: TimeInterval,
        pointer: WorldPoint,
        proximity: PointerProximity,
        catchIsArmed: Bool,
        userIdleDuration: TimeInterval
    ) -> PetSituation {
        let isWatching = isWatchingWindow
        let isRoaming = isRoamingEnabled && !isWatching
        let isStrollDue = isRoaming && !movement.hasRoute && timestamp >= nextWanderAt

        if isWatching, let region = activityHint?.approximateRegion {
            requestLuminanceRefresh(
                at: timestamp,
                near: region,
                every: Self.luminanceRefreshInterval
            )
        } else if !isWatching, !movement.hasRoute {
            requestLuminanceRefreshForRoaming(at: timestamp)
        }

        // The accessibility query costs a synchronous round trip, so it only
        // runs while there is a window whose caret the answer would move the pet
        // away from.
        let focus = isWatching ? refreshedFocus(at: timestamp, force: false) : nil
        return PetSituation(
            timestamp: timestamp,
            world: planningWorld(focus: focus),
            position: movement.position,
            objectSize: overlay.objectSize,
            pointerPosition: pointer,
            walkingSpeed: tuning.walkingSpeed,
            isPointerOwned: catchIsArmed
                || Self.pointerOwnedStates.contains(behavior.state)
                || (isPointerAvoidanceEnabled && proximity != .far),
            isEvading: isEvadeTransitioning,
            isWalking: movement.hasRoute,
            isResting: behavior.state.isResting,
            activitySourceID: activeActivitySourceID,
            activityHint: activityHint,
            userIdleDuration: userIdleDuration,
            idleBeforeRest: restConfiguration.idleBeforeRest,
            isRoamingEnabled: isRoamingEnabled,
            isStrollDue: isStrollDue,
            strollCandidates: isRoaming && !movement.hasRoute ? strollCandidates() : []
        )
    }

    /// Carries out the director's decision. Nothing here re-decides.
    private func apply(
        _ intent: PlacementIntent,
        at timestamp: TimeInterval,
        deltaTime: TimeInterval
    ) {
        switch intent {
        case let .travel(destination, _):
            travelToSeat(destination, at: timestamp, deltaTime: deltaTime)
        case let .stroll(point):
            beginStroll(to: point, at: timestamp, deltaTime: deltaTime)
        case .hold, .sleepInPlace, .none:
            // `.sleepInPlace` lands here when rest declined to start — the
            // pointer came close, or the state machine was mid-transition. The
            // seat is kept either way.
            if isWatchingWindow {
                holdSeat(at: timestamp, deltaTime: deltaTime)
            } else {
                updateRoaming(at: timestamp, deltaTime: deltaTime)
            }
        }
    }

    private func travelToSeat(
        _ destination: InterestDestination,
        at timestamp: TimeInterval,
        deltaTime: TimeInterval
    ) {
        if !movement.hasRoute || movement.destination != destination.point {
            let route = DisplayTopology(displays: displays).route(
                from: movement.position,
                to: destination.point
            )
            guard !route.waypoints.isEmpty else {
                // Nowhere to walk is not a reason to keep trying. The pet
                // watches from where it stands and the seat is judged there.
                placement.settleInPlace(
                    sourceID: activeActivitySourceID,
                    at: timestamp
                )
                holdSeat(at: timestamp, deltaTime: deltaTime)
                return
            }
            movement.setRoute(route.waypoints)
        }
        isEvadeTransitioning = false
        restDestination = nil
        behavior.handle(.beginInterestTravel, at: timestamp)
        movement.configuration.maximumSpeed = tuning.walkingSpeed
        nextWanderAt = .infinity
        if movement.update(deltaTime: deltaTime).reachedDestination {
            deliverArrivalReaction(at: timestamp)
            persistPosition()
        }
    }

    /// A parked pet keeps its seat. All that is left is wearing the reaction the
    /// current event asked for.
    private func holdSeat(at timestamp: TimeInterval, deltaTime: TimeInterval) {
        movement.cancelRoute(stop: false)
        movement.configuration.maximumSpeed = tuning.walkingSpeed
        _ = movement.update(deltaTime: deltaTime)
        if activityArrivalReaction != nil {
            deliverArrivalReaction(at: timestamp)
            return
        }
        switch behavior.state {
        case .observe, .work, .waitingForUser, .celebrate, .sad, .spark:
            break
        case .wake, .stretch, .caught, .dragged:
            break
        default:
            // Only a lasting condition is worn continuously. A moment -- the
            // start hop, a glance at a file being read -- is delivered once and
            // then the pet is simply present, which is what a companion beside a
            // busy agent should look like.
            guard let sustained = activeActivityReaction, sustained.isOngoing else { break }
            applyActivityReaction(sustained, at: timestamp)
        }
    }

    /// The reaction an event asked for is owed to the user until the pet
    /// settles, whether it walked to a new seat or kept the one it had.
    private func deliverArrivalReaction(at timestamp: TimeInterval) {
        let reaction = activityArrivalReaction ?? activeActivityReaction ?? .observe
        activityArrivalReaction = nil
        applyActivityReaction(reaction, at: timestamp)
    }

    private func applyActivityReaction(
        _ reaction: CompanionReaction,
        at timestamp: TimeInterval
    ) {
        // Reactions never wake the creature by themselves. Callers that mean to
        // interrupt rest call `cancelRestForActivity` first, so a session that
        // simply ends leaves a sleeping pet asleep.
        guard !behavior.state.isResting else { return }
        behavior.handle(.reaction(reaction), at: timestamp)
        movement.cancelRoute(stop: false)
        nextWanderAt = activeActivitySourceID == nil ? timestamp + 2.0 : .infinity
    }

    /// The director releases the seat on its own once there is no source to
    /// watch, so nothing here has to remember to clear a placement flag.
    private func clearActiveActivity(at timestamp: TimeInterval) {
        activeActivitySourceID = nil
        activeActivityReaction = nil
        activityArrivalReaction = nil
        activityHint = nil
        movement.cancelRoute(stop: false)
        nextWanderAt = timestamp + 2.0
    }

    private func updateRestLifecycle(
        userIdleDuration: TimeInterval,
        pointerProximity: PointerProximity,
        pointerPosition: WorldPoint,
        mayNapOnSeat: Bool,
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
                beginRestTravel(
                    pointerPosition: pointerPosition,
                    napInPlace: mayNapOnSeat,
                    at: timestamp
                )
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

        // Watching an agent used to block rest outright, which meant the pet
        // could never sleep during the long unattended run that is exactly when
        // nobody is looking at it. Priority 7 of the decision table answers
        // whether the seat it is parked on is worth dozing on.
        recordRestGate(
            userIdleDuration: userIdleDuration,
            pointerProximity: pointerProximity,
            mayNapOnSeat: mayNapOnSeat,
            at: timestamp
        )
        guard userIdleDuration >= restConfiguration.idleBeforeRest,
              !placement.isTravelling,
              !isWatchingWindow || mayNapOnSeat,
              pointerProximity == .far,
              BehaviorController.restEntryStates.contains(behavior.state) else {
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

    private func beginRestTravel(
        pointerPosition: WorldPoint,
        napInPlace: Bool,
        at timestamp: TimeInterval
    ) {
        // A pet that dozed off beside a working agent is already on a vetted
        // seat. Walking it to a display corner to sleep would throw that away
        // and put the trip back that this gate exists to remove.
        if napInPlace {
            record("rest", "sleeping in place, on a vetted seat", at: timestamp)
            enterSleep(at: timestamp)
            return
        }

        // Away from an agent, the spot has to answer for itself. Standing on a
        // clear patch of desktop is the ordinary case, and getting up to walk
        // to a corner from it is the trip the user actually sees: a pet that
        // was nodding off suddenly striding across the screen.
        if BasicSafeZonePlanner.napsInPlace(
            at: movement.position,
            objectSize: overlay.objectSize,
            in: judgeableLuminance
        ) {
            record("rest", "sleeping in place, spot reads clear", at: timestamp)
            enterSleep(at: timestamp)
            return
        }
        record("rest", "tucking into a safe zone, spot unvetted", at: timestamp)

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

    /// Walks whatever route roaming already has and paces the next pause.
    /// Choosing where to stroll is priority 9 of the decision table, not this.
    private func updateRoaming(at timestamp: TimeInterval, deltaTime: TimeInterval) {
        movement.configuration.maximumSpeed = tuning.walkingSpeed
        guard isRoamingEnabled else {
            movement.cancelRoute(stop: false)
            _ = movement.update(deltaTime: deltaTime)
            return
        }
        guard movement.hasRoute else {
            _ = movement.update(deltaTime: deltaTime)
            return
        }
        if movement.update(deltaTime: deltaTime).reachedDestination {
            behavior.handle(.arrived, at: timestamp)
            nextWanderAt = timestamp + tuning.wanderDelay(
                randomUnit: Double.random(in: 0...1)
            )
            persistPosition()
        }
    }

    private func requestLuminanceRefreshForRoaming(at timestamp: TimeInterval) {
        guard let display = world.display(containing: movement.position)
            ?? world.nearestDisplay(to: movement.position) else { return }
        requestLuminanceRefresh(
            at: timestamp,
            near: display.visibleFrame,
            every: Self.roamingLuminanceRefreshInterval
        )
    }

    private func beginStroll(
        to point: WorldPoint,
        at timestamp: TimeInterval,
        deltaTime: TimeInterval
    ) {
        isEvadeTransitioning = false
        // Claim the walking state before laying a route. Setting the route
        // first left it in place when the state machine refused the
        // transition, so the pet walked the whole leg animated as whatever it
        // had been doing -- observe frames, which are the idle frames.
        guard behavior.handle(.beginWander, at: timestamp).to == .wander else {
            nextWanderAt = timestamp + 2
            return
        }
        let route = DisplayTopology(displays: displays).route(
            from: movement.position,
            to: point
        )
        movement.configuration.maximumSpeed = tuning.walkingSpeed
        movement.setRoute(route.waypoints)
        if !movement.hasRoute { nextWanderAt = timestamp + 2 }
        _ = movement.update(deltaTime: deltaTime)
    }

    /// Wandering is where the pet spends most of its life: an agent turn ends,
    /// the activity clears, and two seconds later it strolls off. That walk
    /// never looked at the screen, so it parked on the user's text far more
    /// often than any interest seat ever did. Offering the director a handful
    /// of destinations to reject is the cheapest way to fix that without making
    /// roaming look calculated.
    private func strollCandidates() -> [WorldPoint] {
        (0..<Self.wanderCandidateCount).compactMap { _ in randomWanderPoint() }
    }

    private func randomWanderPoint() -> WorldPoint? {
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
