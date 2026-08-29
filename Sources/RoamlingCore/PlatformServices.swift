// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

import Foundation

@MainActor
public protocol DisplayProviding: AnyObject {
    func currentDisplays() -> [DisplaySnapshot]
}

@MainActor
public protocol WindowProviding: AnyObject {
    func currentWindows() -> [WindowSnapshot]
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
    func currentFocus() -> FocusSnapshot?
}

@MainActor
public protocol SafeZoneProviding: AnyObject {
    func safeZones(in world: DesktopWorldSnapshot) async -> [SafeZone]
}

@MainActor
public protocol CaptureProviding: AnyObject {
    var isAuthorized: Bool { get }

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
