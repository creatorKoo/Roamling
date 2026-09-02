// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

import CoreGraphics
import Foundation
import RoamlingCore

/// What the runtime wants to hear about a pointer touching the pet.
///
/// Stated in world points, not the platform's screen points: the overlay owns
/// the conversion because it is the thing that already has to know where it
/// drew itself. A mouse, a trackpad and a touch screen all arrive here the
/// same way.
@MainActor
public protocol PetOverlayInputHandling: AnyObject {
    func petOverlayPointerDown(at point: WorldPoint)
    func petOverlayPointerDragged(to point: WorldPoint, distance: Double)
    func petOverlayPointerUp(at point: WorldPoint, wasDragged: Bool)
}

/// The overlay the runtime actually drives, as opposed to the three-method
/// `OverlayProviding` a passive display would satisfy. Everything here was
/// already being called through the concrete AppKit class.
@MainActor
public protocol PetOverlayProviding: OverlayProviding {
    /// Live scale, which is not always the scale that was last requested --
    /// implementations clamp, and the menu reads back what was accepted.
    var scale: Double { get }

    /// The pet's on-screen footprint at the current scale. Placement clamps
    /// against this, so it must reflect the scale already applied.
    var objectSize: WorldSize { get }

    /// Held weakly by implementations: the handler is the runtime, which owns
    /// the services that own the overlay.
    var inputHandler: (any PetOverlayInputHandling)? { get set }

    func setScale(_ scale: Double)
    func setHitRegionScale(_ scale: Double)
    func setFrameImage(_ image: CGImage?)

    /// Whether a world point lands on the pet rather than the transparent
    /// padding around it. Used to decide whether a click belongs to the pet
    /// or to the app underneath.
    func containsPet(atWorldPoint worldPoint: WorldPoint) -> Bool
}

/// Everything the runtime needs from the machine it is running on.
///
/// The runtime used to build these itself, which meant a Windows adapter had
/// nowhere to be handed in. Taking them as one value also lets a test supply
/// fakes and step the runtime without a screen.
@MainActor
public struct PlatformServices {
    public let display: any DisplayProviding
    public let displayChanges: any DisplayChangeObserving
    public let safeZone: any SafeZoneProviding
    public let userIdle: any UserIdleProviding
    public let capture: any CaptureProviding
    public let pointer: any PointerProviding
    public let window: any WindowProviding
    public let focus: any FocusProviding
    public let overlay: any PetOverlayProviding

    /// Shared with the providers above, which read it to convert platform
    /// coordinates. The runtime is the only writer.
    public let coordinateSpace: CoordinateSpaceSource

    public init(
        display: any DisplayProviding,
        displayChanges: any DisplayChangeObserving,
        safeZone: any SafeZoneProviding,
        userIdle: any UserIdleProviding,
        capture: any CaptureProviding,
        pointer: any PointerProviding,
        window: any WindowProviding,
        focus: any FocusProviding,
        overlay: any PetOverlayProviding,
        coordinateSpace: CoordinateSpaceSource
    ) {
        self.display = display
        self.displayChanges = displayChanges
        self.safeZone = safeZone
        self.userIdle = userIdle
        self.capture = capture
        self.pointer = pointer
        self.window = window
        self.focus = focus
        self.overlay = overlay
        self.coordinateSpace = coordinateSpace
    }
}
