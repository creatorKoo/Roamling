// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

import Foundation
import RoamlingCore
import RoamlingCoreRs
import RoamlingPet

@MainActor
public final class RoamlingRuntime: PetOverlayInputHandling {
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
            defaults.set(isRoamingEnabled, forKey: DefaultsKey.roaming)
            core.setRoamingEnabled(isRoamingEnabled, at: now())
        }
    }

    public var isPointerAvoidanceEnabled: Bool {
        didSet {
            defaults.set(isPointerAvoidanceEnabled, forKey: DefaultsKey.avoidPointer)
            core.setPointerAvoidanceEnabled(isPointerAvoidanceEnabled)
        }
    }

    public var areInteractionsEnabled: Bool {
        didSet {
            defaults.set(areInteractionsEnabled, forKey: DefaultsKey.interactions)
            if core.setInteractionsEnabled(areInteractionsEnabled) {
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

    /// What the pet is doing and where it is standing. Read-only, and read by
    /// tests -- the app watches the pet through the overlay instead.
    public var behaviorState: BehaviorState { core.state }
    public var position: WorldPoint { core.position }
    /// How many random numbers the pet has spent. Read only by the recorded
    /// session, where it is the fastest way to see two runs part company.
    public var randomDraws: UInt64 { core.randomDraws }
    /// Whether the director still has a walk in progress. Read by the test
    /// that pins arrival, which has to know when settling happened.
    public var isPlacementTravelling: Bool { core.isPlacementTravelling }
    /// In the order they were handed over, which is the order they are shown.
    public var agentIntegrations: [any AgentIntegration] { agents }

    public func agentIntegration(id: String) -> (any AgentIntegration)? {
        agents.first { $0.id == id }
    }

    private let services: PlatformServices
    private let defaults: UserDefaults
    private let now: @Sendable () -> TimeInterval

    private var displayProvider: any DisplayProviding { services.display }
    private var displayChanges: any DisplayChangeObserving { services.displayChanges }
    private var safeZoneProvider: any SafeZoneProviding { services.safeZone }
    private var userIdleProvider: any UserIdleProviding { services.userIdle }
    private var captureProvider: any CaptureProviding { services.capture }
    private var pointerProvider: any PointerProviding { services.pointer }
    private var windowProvider: any WindowProviding { services.window }
    private var focusProvider: any FocusProviding { services.focus }
    private var overlay: any PetOverlayProviding { services.overlay }

    private let catalog: PetCatalog
    private let loader: PetLoader
    private let agents: [any AgentIntegration]

    private var displays: [DisplaySnapshot]
    /// Stored in the services so the providers see every move of the origin.
    private var coordinateSpace: DesktopCoordinateSpace {
        get { services.coordinateSpace.current }
        set { services.coordinateSpace.current = newValue }
    }
    private var world: DesktopWorldSnapshot
    private var animationPlayer: PetAnimationPlayer

    /// Everything the pet decides. What is left on this side is the timer, the
    /// defaults, the diagnostics file, the agent subscriptions and the sprite
    /// sheet -- none of which is a decision. `docs/windows.md` unit 6c.
    private let core: RustPetLoop

    private var tickTimer: Timer?
    private var activityTasks: [Task<Void, Never>] = []
    private var screenObserver: DisplayChangeSubscription?
    private var cachedLuminance: LuminanceField?
    private var luminanceCapturedAt: TimeInterval = -.infinity
    private var luminanceTask: Task<Void, Never>?
    private var running = false

    /// - Parameters:
    ///   - defaults: Where the user's settings live. A test passes a throwaway
    ///     suite so it neither reads the running app's preferences nor writes
    ///     to them.
    ///   - catalog: Pass `PetCatalog(roots: [])` to keep a test off whatever
    ///     pets happen to be installed on the machine.
    ///   - clock: The monotonic clock everything schedules against. A test
    ///     steps it by hand and calls `tick()`, so a minute of pet behaviour
    ///     costs no wall time and never flakes on a slow machine.
    public init(
        services: PlatformServices,
        agents: [any AgentIntegration] = [],
        defaults: UserDefaults = .standard,
        catalog: PetCatalog = PetCatalog(),
        clock: @escaping @Sendable () -> TimeInterval = { ProcessInfo.processInfo.systemUptime },
        randomSeed: UInt64 = UInt64.random(in: 1...UInt64.max)
    ) {
        self.defaults = defaults
        now = clock
        diagnosticsPath = ProcessInfo.processInfo.environment["ROAMLING_REST_LOG"]
            ?? defaults.string(forKey: "roamling.diagnosticsLog")
        defaults.register(defaults: [
            DefaultsKey.roaming: true,
            DefaultsKey.avoidPointer: true,
            DefaultsKey.interactions: true,
            DefaultsKey.scale: 1.0,
            DefaultsKey.builtInPet: BuiltInPetKind.fatMochi.rawValue
        ])
        let runtimeTuning = Self.loadRuntimeTuning(defaults: defaults)

        let descriptors = catalog.discover()
        let selectedPath = defaults.string(forKey: DefaultsKey.petPackagePath)
        let selectedBuiltInPet = defaults.string(forKey: DefaultsKey.builtInPet)
            .flatMap(BuiltInPetKind.init(rawValue:)) ?? .fatMochi
        let initialAsset = Self.loadInitialAsset(
            descriptors: descriptors,
            selectedPath: selectedPath,
            builtInPet: selectedBuiltInPet,
            images: services.images,
            loader: PetLoader(images: services.images)
        )

        let displaySet = services.display.currentDisplaySet()
        let initialWorld = DesktopWorldSnapshot(displays: displaySet.displays)
        // Before anything asks the overlay where it is: the providers convert
        // through this box, and the overlay is about to be positioned.
        services.coordinateSpace.current = displaySet.coordinateSpace
        services.overlay.setScale(defaults.double(forKey: DefaultsKey.scale))
        services.overlay.setHitRegionScale(runtimeTuning.hitRegionScale)
        let initialPosition = Self.initialPosition(
            defaults: defaults,
            world: initialWorld,
            objectSize: services.overlay.objectSize
        )

        self.services = services
        self.agents = agents
        self.catalog = catalog
        loader = PetLoader(images: services.images)
        installedPets = descriptors
        asset = initialAsset
        self.selectedBuiltInPet = initialAsset.packageURL == nil ? selectedBuiltInPet : nil
        displays = displaySet.displays
        world = initialWorld
        tuning = runtimeTuning
        animationPlayer = PetAnimationPlayer(asset: initialAsset)
        core = RustPetLoop(
            position: initialPosition,
            tuning: runtimeTuning,
            seed: randomSeed
        )
        isRoamingEnabled = defaults.bool(forKey: DefaultsKey.roaming)
        isPointerAvoidanceEnabled = defaults.bool(forKey: DefaultsKey.avoidPointer)
        areInteractionsEnabled = defaults.bool(forKey: DefaultsKey.interactions)

        core.setDisplays(displaySet.displays)
        core.setObjectSize(services.overlay.objectSize)
        core.setFlags(
            roaming: isRoamingEnabled,
            avoidance: isPointerAvoidanceEnabled,
            interactions: areInteractionsEnabled
        )
        core.setAnimationDurations(
            caught: Self.caughtTransitionDuration(of: initialAsset),
            dragged: Self.draggedCycleDuration(of: initialAsset)
        )
        core.setNextWanderAt(now() + 2.2)
    }

    /// Brings the pet up. `drivingTicks` is false for a caller that owns the
    /// clock itself: the timer is the only part of this the runtime does not
    /// have to own, and a caller stepping `tick()` by hand cannot share the run
    /// loop with a timer that steps it too. The recorded-session test needs
    /// that, and so will a shell whose platform already has a frame loop.
    public func start(drivingTicks: Bool = true) {
        guard !running else { return }
        running = true
        overlay.inputHandler = self
        overlay.setPosition(core.position)
        renderCurrentFrame()
        overlay.setVisible(true)

        screenObserver = displayChanges.observeDisplayChanges { [weak self] in
            self?.handleDisplayChange()
        }
        startActivitySources()
        if drivingTicks { scheduleNextTick(after: 0.02) }
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
        agents.forEach { $0.stopReceiving() }
        screenObserver?.cancel()
        screenObserver = nil
        core.clearClickReaction(clearCaughtTransition: false)
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
            defaults.set(packageURL.standardizedFileURL.path, forKey: DefaultsKey.petPackagePath)
            return .success(())
        } catch {
            return .failure(error)
        }
    }

    public func useBuiltInPet(_ kind: BuiltInPetKind) {
        install(asset: MascotPetFactory.make(kind, images: services.images))
        selectedBuiltInPet = kind
        defaults.set(kind.rawValue, forKey: DefaultsKey.builtInPet)
        defaults.removeObject(forKey: DefaultsKey.petPackagePath)
    }

    public func setScale(_ newScale: Double) {
        overlay.setScale(newScale)
        defaults.set(overlay.scale, forKey: DefaultsKey.scale)
        overlay.setPosition(core.setScale(objectSize: overlay.objectSize))
    }

    /// Applies only the values currently exposed for MVP 0/0.5 validation.
    /// Rest and future activity behavior use separate milestone-specific
    /// configuration so this validated interaction preset remains stable.
    public func applyTuning(_ proposed: RuntimeTuning) {
        let normalized = proposed.normalized
        tuning = normalized
        core.applyTuning(normalized, at: now())
        overlay.setHitRegionScale(normalized.hitRegionScale)
        persistRuntimeTuning()
    }

    public func resetTuning() {
        applyTuning(.standard)
    }

    /// Brings every agent that was installed before this build up to the
    /// current handler shape, without asking again. An agent that was never
    /// opted into keeps an untouched config.
    public func repairAgentIntegrationsIfNeeded() {
        for agent in agents { _ = agent.repairIfNeeded() }
    }

    public func testAgentReaction(id: String) {
        testAgentReaction(sourceID: "\(id):test")
    }

    private func testAgentReaction(sourceID: String) {
        handleActivityEvent(CompanionEvent(
            sourceID: sourceID,
            sourceType: .agent,
            timestamp: now(),
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
                timestamp: now(),
                kind: .achievement,
                intensity: 0.55,
                context: .working
            ))
        }
    }

    public func petOverlayPointerDown(at pointer: WorldPoint) {
        apply(core.pointerDown(at: pointer, now: now()))
    }

    public func petOverlayPointerDragged(to pointer: WorldPoint, distance: Double) {
        apply(core.pointerDragged(to: pointer, distance: distance, now: now()))
    }

    public func petOverlayPointerUp(at pointer: WorldPoint, wasDragged: Bool) {
        apply(core.pointerUp(at: pointer, wasDragged: wasDragged, now: now()))
    }

    /// Carries out what a click or a drag decided. Nothing here re-decides.
    private func apply(_ interaction: RustPetLoop.Interaction) {
        if let enabled = interaction.setInteractionEnabled {
            overlay.setInteractionEnabled(enabled)
        }
        updateAnimation(
            capability: interaction.capability,
            pointerDegrees: interaction.lookDirectionDegrees
        )
        overlay.setPosition(interaction.position)
        if interaction.render { renderCurrentFrame() }
        if interaction.persistPosition { persistPosition() }
        if let interval = interaction.rescheduleAfter, running {
            scheduleNextTick(after: interval)
        }
    }

    private func tickTimerFired() {
        guard running else { return }
        autoreleasepool { tick() }
        scheduleNextTick(after: core.preferredTickInterval(at: now()))
    }

    /// Advances the pet by one step and returns, without arming the next one.
    /// The app never calls this directly -- its timer does -- but it is what
    /// lets a test drive the whole loop against a clock it controls.
    ///
    /// Gather, decide, apply. Only the gathering and the applying are here now:
    /// what the pet does about any of it is `pet_runtime.rs`.
    public func tick() {
        let now = self.now()
        // Two questions have to go back out mid-tick. This is the first: an
        // accessibility query is a synchronous round trip, so it only runs
        // while there is a window whose caret the answer would move the pet
        // away from -- and only the core knows whether there is one.
        let wantsFocus = core.beginTick(at: now)
        let focusAuthorized = focusProvider.isAuthorized
        let didQueryFocus = wantsFocus && focusAuthorized
        let queriedFocus = didQueryFocus ? focusProvider.currentFocus() : nil

        let pointer = pointerProvider.currentPointer(at: now)
        core.setLuminance(judgeableLuminance)
        let output = core.finishTick(
            at: now,
            pointer: pointer.position,
            primaryButtonDown: pointer.primaryButtonDown,
            userIdleDuration: userIdleProvider.idleDuration(at: now),
            captureAuthorized: captureProvider.isAuthorized,
            focusAuthorized: focusAuthorized,
            didQueryFocus: didQueryFocus,
            queriedFocus: queriedFocus,
            pointerIsOverPet: overlay.containsPet(atWorldPoint: pointer.position)
        )

        for line in output.diagnostics {
            record(line.category, line.message, at: now)
        }
        // The second question: a capture is far heavier again, and the
        // permission, the task and the throttle are all this side's.
        for request in output.luminanceRequests {
            requestLuminanceRefresh(at: now, near: request.region, every: request.interval)
        }
        if output.persistPosition { persistPosition() }

        overlay.setInteractionEnabled(output.interactionEnabled)
        updateAnimation(
            capability: output.capability,
            pointerDegrees: output.lookDirectionDegrees
        )
        animationPlayer.update(deltaTime: output.deltaTime * output.locomotionRate)
        overlay.setPosition(output.position)
        renderCurrentFrame()
    }

    /// Whether the pet is on duty, which takes a window and not merely a source.
    ///
    /// Both roaming and rest stand down while this is true, so keying it on the
    /// source alone froze the pet outright whenever an event arrived without a
    /// window to watch: nothing to sit beside, nowhere to stroll, and no seat
    /// for the decision table to call worth sleeping on. An agent it cannot
    /// locate is not a reason for the pet to stand still.
    private var isWatchingWindow: Bool { core.isWatchingWindow }

    /// Why the pet is doing what it is doing, kept in memory and copyable from
    /// the menu. Standing and sitting look identical from outside the app, so
    /// without this the only way to tell them apart was to add a log and ship a
    /// build.
    private var diagnostics = DiagnosticsLog()
    /// Mirrors the same entries to a file when a path is set, for a session too
    /// long to hold in the buffer.
    ///
    ///     defaults write dev.roamling.app roamling.diagnosticsLog /tmp/pet.log
    private let diagnosticsPath: String?

    public var diagnosticsText: String {
        diagnostics.text(now: now())
    }

    private func record(_ category: String, _ message: String, at timestamp: TimeInterval) {
        guard diagnostics.record(category, message, at: timestamp) else { return }
        guard let path = diagnosticsPath,
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

    private func scheduleNextTick(after interval: TimeInterval) {
        tickTimer?.invalidate()
        // `.common` so the pet keeps moving through a menu tracking loop or a
        // window resize, which would otherwise starve the default mode.
        let timer = Timer(timeInterval: max(0.01, interval), repeats: false) { [weak self] _ in
            MainActor.assumeIsolated { self?.tickTimerFired() }
        }
        tickTimer = timer
        RunLoop.main.add(timer, forMode: .common)
    }

    private func startActivitySources() {
        for agent in agents {
            startActivitySource(
                start: agent.startReceiving,
                stream: agent.makeEventStream()
            )
        }
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
        let now = self.now()
        // The window query costs a synchronous round trip, so the rule for
        // whether it is worth making lives with the decision and the call
        // stays here.
        let hint = event.locationHint
            ?? (RustCore.wantsWindowHint(event.kind)
                ? windowProvider.currentActivityLocationHint()
                : nil)
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
        for request in core.handleActivityEvent(located, at: now) {
            requestLuminanceRefresh(at: now, near: request.region, every: request.interval)
        }
    }

    /// Accessibility stays off until the user turns it on from the menu, so the
    /// app never prompts merely because it launched.
    public var isAccessibilityAuthorized: Bool { focusProvider.isAuthorized }

    @discardableResult
    public func requestAccessibilityAuthorization() -> Bool {
        focusProvider.requestAuthorization()
    }

    /// Visual placement stays off until the user turns it on from the menu.
    public var isScreenCaptureAuthorized: Bool { captureProvider.isAuthorized }

    @discardableResult
    public func requestScreenCaptureAuthorization() -> Bool {
        captureProvider.requestAuthorization()
    }

    private var judgeableLuminance: LuminanceField? {
        captureProvider.isAuthorized ? cachedLuminance : nil
    }

    /// The interval already has the resting slow-down folded in: what is left
    /// here is the permission, the task and the throttle, none of which is a
    /// decision the pet makes.
    private func requestLuminanceRefresh(
        at timestamp: TimeInterval,
        near region: WorldRect,
        every interval: TimeInterval
    ) {
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

    private func updateAnimation(capability: PetCapability, pointerDegrees: Double?) {
        animationPlayer.setCapability(capability)
        animationPlayer.setLookDirection(degrees: pointerDegrees)
    }

    private func renderCurrentFrame() {
        overlay.setFrameImage(asset.frameImage(at: animationPlayer.currentFrameIndex))
    }

    private func handleDisplayChange() {
        // Converted through the old space before the new one replaces it: the
        // pet is where it looked, not where its coordinates happen to land.
        let oldAppKitPoint = coordinateSpace.pointToAppKit(core.position)
        let displaySet = displayProvider.currentDisplaySet()
        guard !displaySet.displays.isEmpty else { return }
        displays = displaySet.displays
        coordinateSpace = displaySet.coordinateSpace
        world = DesktopWorldSnapshot(displays: displays)
        let clamped = core.handleDisplayChange(
            displays: displays,
            carrying: coordinateSpace.pointFromAppKit(oldAppKitPoint),
            at: now()
        )
        overlay.setPosition(clamped)
        persistPosition()
    }

    private func install(asset newAsset: PetAsset) {
        asset = newAsset
        core.clearClickReaction(clearCaughtTransition: true)
        animationPlayer = PetAnimationPlayer(asset: newAsset)
        core.setAnimationDurations(
            caught: Self.caughtTransitionDuration(of: newAsset),
            dragged: Self.draggedCycleDuration(of: newAsset)
        )
        updateAnimation(
            capability: PetCapabilityMapping.capability(
                for: core.state,
                velocityDX: 0,
                isCaughtTransitionActive: false
            ),
            pointerDegrees: nil
        )
        renderCurrentFrame()
    }

    /// How long the pet stays in the caught pose before it starts scrabbling.
    /// Capped because a pet held still for a whole second reads as stuck.
    private static func caughtTransitionDuration(of asset: PetAsset) -> TimeInterval {
        guard let track = asset.resolver.resolve(.caught), !track.loops else { return 0 }
        return min(track.frames.reduce(0) { $0 + $1.duration }, 0.8)
    }

    private static func draggedCycleDuration(of asset: PetAsset) -> TimeInterval {
        guard let track = asset.resolver.resolve(.dragged) else { return 0.4 }
        let duration = track.frames.reduce(0) { $0 + $1.duration }
        return min(max(duration, 0.3), 0.8)
    }

    private func persistPosition() {
        defaults.set(core.position.x, forKey: DefaultsKey.positionX)
        defaults.set(core.position.y, forKey: DefaultsKey.positionY)
        defaults.set(true, forKey: DefaultsKey.hasPosition)
    }

    /// Writes down the values the user moved and forgets the rest.
    ///
    /// Writing all eleven meant that opening the panel once sealed a machine
    /// off from every later change to any of them -- a stored value wins, so
    /// whichever defaults were current that day became permanent. It was
    /// "Reset Defaults" that wrote the file, which put the most careful users
    /// furthest out of reach.
    ///
    /// The stored form is the same JSON object as before with the untouched
    /// keys left out, so nothing needs migrating in either direction: the
    /// decoder already fills a missing key from the default, which is what it
    /// was written to do when a field is added, and a full object from an older
    /// build still reads here.
    private func persistRuntimeTuning() {
        let changed = tuning.changesFromStandard
        guard !changed.isEmpty else {
            // Removed rather than stored empty. A key that is gone lets the
            // default answer, which is the whole point, and this is where
            // "Reset Defaults" lands.
            defaults.removeObject(forKey: DefaultsKey.runtimeTuning)
            return
        }
        var stored: [String: Double] = [:]
        for (key, value) in changed { stored[key.rawValue] = value }
        guard let data = try? JSONEncoder().encode(stored) else { return }
        defaults.set(data, forKey: DefaultsKey.runtimeTuning)
    }

    /// Reads whatever is there -- every key, some of them, or none.
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
        images: any PetImageSourcing,
        loader: PetLoader
    ) -> PetAsset {
        if let selectedPath,
           let selected = descriptors.first(where: {
               $0.packageURL.standardizedFileURL.path == URL(fileURLWithPath: selectedPath).standardizedFileURL.path
           }),
           let loaded = try? loader.load(packageAt: selected.packageURL) {
            return loaded
        }
        return MascotPetFactory.make(builtInPet, images: images)
    }
}
