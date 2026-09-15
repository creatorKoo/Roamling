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

    /// Which app is in front, named the way the user's list names it -- a
    /// bundle identifier here, an executable name on Windows. Nil for this app
    /// itself, so the pet never counts being clicked as the user working.
    ///
    /// Asked every half second, so it must not be a synchronous round trip to
    /// the window server.
    func frontmostApplicationIdentifier() -> String?

    /// What to call that identifier in a menu, or nil when the platform cannot
    /// say. The engine only ever holds strings: naming an app is the one part
    /// of this that needs the platform's own application list.
    func applicationDisplayName(for identifier: String) -> String?
}

@MainActor
public protocol PointerProviding: AnyObject {
    func currentPointer(at timestamp: TimeInterval) -> PointerSnapshot
}

@MainActor
public protocol UserIdleProviding: AnyObject {
    func idleDuration(at timestamp: TimeInterval) -> TimeInterval

    /// Since the last keystroke alone, ignoring the mouse. Typing is the one
    /// signal that says the user is doing something rather than merely sitting
    /// at a lit screen, and reading it must not need an input hook: nothing
    /// here may see *which* key, only how long ago.
    func keyboardIdleDuration(at timestamp: TimeInterval) -> TimeInterval
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

/// Why a capture produced no field. Named, because the one thing a capture
/// path must not do is fold every failure into a single `nil`: that is how a
/// process ran blind for two hours and the log could not say which of four
/// different things had gone wrong (`docs/capture.md` §2).
public enum CaptureFailure: Sendable, Equatable {
    /// The platform cannot capture at all: no permission, or no API.
    case unavailable
    /// The list of capturable content could not be fetched.
    case noContent
    /// The content list came back, but the display asked for is not in it.
    case displayNotFound
    /// The screenshot itself failed.
    case noImage
    /// An image arrived and could not be reduced to a field.
    case unusableImage
    /// Cancellation was observed between stages.
    case cancelled
}

public enum CaptureOutcome: Sendable {
    case field(LuminanceField)
    case failed(CaptureFailure)

    public var field: LuminanceField? {
        if case let .field(field) = self { return field }
        return nil
    }
}

@MainActor
public protocol CaptureProviding: AnyObject {
    var isAuthorized: Bool { get }

    @discardableResult
    func requestAuthorization() -> Bool

    /// A downsampled luminance view of one display, or which stage refused.
    /// Implementations must not persist or log the capture.
    func captureLuminanceField(for display: DisplaySnapshot) async -> CaptureOutcome

    /// Which stage the capture in flight is waiting on, so a caller that gives
    /// up on it can say where it stuck. Nil when nothing is in flight or the
    /// implementation has no stages worth naming.
    var inFlightStage: String? { get }
}

public extension CaptureProviding {
    var inFlightStage: String? { nil }
}

@MainActor
public protocol OverlayProviding: AnyObject {
    func setPosition(_ position: WorldPoint)
    func setVisible(_ visible: Bool)
    func setInteractionEnabled(_ enabled: Bool)
}
