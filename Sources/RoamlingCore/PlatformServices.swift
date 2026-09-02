// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

import Foundation

/// One reading of the desktop: the displays and the coordinate space they
/// imply. The two are produced together because a display's frame is only
/// meaningful in the space it was measured in -- a provider that returned
/// them separately would invite the caller to pair a fresh frame with a
/// stale origin, which reads as the pet teleporting.
public struct DisplaySnapshotSet: Sendable {
    public let displays: [DisplaySnapshot]
    public let coordinateSpace: DesktopCoordinateSpace

    public init(displays: [DisplaySnapshot], coordinateSpace: DesktopCoordinateSpace) {
        self.displays = displays
        self.coordinateSpace = coordinateSpace
    }
}

@MainActor
public protocol DisplayProviding: AnyObject {
    func currentDisplaySet() -> DisplaySnapshotSet
}

/// The one coordinate space every provider reads from and the runtime alone
/// writes to.
///
/// Providers need the current space to convert what the OS hands them, and
/// the space is only known after a display reading -- which the runtime does,
/// because it is the one that decides an empty reading means "keep the last
/// space" rather than "the desktop is gone". Sharing one box breaks that
/// circle without letting anyone else move the origin.
@MainActor
public final class CoordinateSpaceSource {
    public var current: DesktopCoordinateSpace

    public init(_ current: DesktopCoordinateSpace = DesktopCoordinateSpace(worldTop: 0)) {
        self.current = current
    }
}

/// Tells the caller that the displays have changed shape. Kept apart from
/// `DisplayProviding` because a platform can answer "what is on screen now"
/// without being able to say "and tell me when that changes" -- a fake in a
/// test implements the first and not the second.
@MainActor
public protocol DisplayChangeObserving: AnyObject {
    func observeDisplayChanges(
        _ handler: @escaping @MainActor () -> Void
    ) -> DisplayChangeSubscription
}

/// Ends an observation. Cancelling twice does nothing the second time, and
/// nothing happens on deinit: the owner says when the observation stops,
/// because the runtime observes only while it is running and outlives that.
@MainActor
public final class DisplayChangeSubscription {
    private var cancelHandler: (() -> Void)?

    public init(cancel: @escaping () -> Void) {
        cancelHandler = cancel
    }

    public func cancel() {
        cancelHandler?()
        cancelHandler = nil
    }
}

@MainActor
public protocol WindowProviding: AnyObject {
    func currentWindows() -> [WindowSnapshot]

    /// Where the frontmost window says the user's activity is, at whatever
    /// confidence the platform can offer. Coarser than a focus snapshot and
    /// available without any permission, so it is the fallback the pet uses
    /// when accessibility is refused.
    func currentActivityLocationHint() -> LocationHint?
}

@MainActor
public protocol PointerProviding: AnyObject {
    func currentPointer(at timestamp: TimeInterval) -> PointerSnapshot
}

@MainActor
public protocol UserIdleProviding: AnyObject {
    func idleDuration(at timestamp: TimeInterval) -> TimeInterval
}

@MainActor
public protocol FocusProviding: AnyObject {
    var isAuthorized: Bool { get }

    /// Asks the platform for permission. Returns whether it is now granted;
    /// a platform that needs no permission returns true without prompting.
    @discardableResult
    func requestAuthorization() -> Bool

    func currentFocus() -> FocusSnapshot?
}

@MainActor
public protocol SafeZoneProviding: AnyObject {
    func safeZones(in world: DesktopWorldSnapshot) -> [SafeZone]
}

@MainActor
public protocol CaptureProviding: AnyObject {
    var isAuthorized: Bool { get }

    @discardableResult
    func requestAuthorization() -> Bool

    /// A downsampled luminance view of one display, or nil when capture is
    /// unavailable. Implementations must not persist or log the capture.
    func captureLuminanceField(for display: DisplaySnapshot) async -> LuminanceField?
}

@MainActor
public protocol OverlayProviding: AnyObject {
    func setPosition(_ position: WorldPoint)
    func setVisible(_ visible: Bool)
    func setInteractionEnabled(_ enabled: Bool)
}
