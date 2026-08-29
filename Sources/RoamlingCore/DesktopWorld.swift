// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

import Foundation

public struct DisplaySnapshot: Codable, Hashable, Sendable, Identifiable {
    public let id: String
    public let name: String
    public let frame: WorldRect
    public let visibleFrame: WorldRect
    public let scale: Double

    public init(
        id: String,
        name: String,
        frame: WorldRect,
        visibleFrame: WorldRect,
        scale: Double
    ) {
        self.id = id
        self.name = name
        self.frame = frame
        self.visibleFrame = visibleFrame
        self.scale = scale
    }
}

public struct WindowSnapshot: Codable, Hashable, Sendable, Identifiable {
    public let id: String
    public let applicationIdentifier: String?
    public let title: String?
    public let frame: WorldRect
    public let isFocused: Bool

    public init(
        id: String,
        applicationIdentifier: String? = nil,
        title: String? = nil,
        frame: WorldRect,
        isFocused: Bool = false
    ) {
        self.id = id
        self.applicationIdentifier = applicationIdentifier
        self.title = title
        self.frame = frame
        self.isFocused = isFocused
    }
}

public struct PointerSnapshot: Codable, Hashable, Sendable {
    public let position: WorldPoint
    public let timestamp: TimeInterval
    public let primaryButtonDown: Bool

    public init(position: WorldPoint, timestamp: TimeInterval, primaryButtonDown: Bool) {
        self.position = position
        self.timestamp = timestamp
        self.primaryButtonDown = primaryButtonDown
    }
}

public struct FocusSnapshot: Codable, Hashable, Sendable {
    /// Exact frame of the focused window. `MacWindowProvider`'s permission-free
    /// path can only guess this from the frontmost process, so an accessibility
    /// answer supersedes it wherever both exist.
    public let windowFrame: WorldRect?
    public let focusedElementFrame: WorldRect?
    public let caretFrame: WorldRect?
    public let confidence: Double

    public init(
        windowFrame: WorldRect? = nil,
        focusedElementFrame: WorldRect? = nil,
        caretFrame: WorldRect? = nil,
        confidence: Double = 0
    ) {
        self.windowFrame = windowFrame.flatMap { $0.isEmpty ? nil : $0 }
        self.focusedElementFrame = focusedElementFrame.flatMap { $0.isEmpty ? nil : $0 }
        self.caretFrame = Self.usableCaret(caretFrame)
        self.confidence = confidence.clamped(to: 0...1)
    }

    /// An insertion point legitimately reports zero width, so `isEmpty` would
    /// discard exactly the rect placement cares about most. Widen it to a
    /// usable obstacle instead of dropping it.
    private static func usableCaret(_ rect: WorldRect?) -> WorldRect? {
        guard let rect, rect.size.width > 0 || rect.size.height > 0 else { return nil }
        return WorldRect(
            x: rect.minX,
            y: rect.minY,
            width: max(rect.size.width, 2),
            height: max(rect.size.height, 2)
        )
    }
}

public struct SafeZone: Codable, Hashable, Sendable {
    public let frame: WorldRect
    public let score: Double
    public let confidence: Double
    public let reason: String

    public init(frame: WorldRect, score: Double, confidence: Double, reason: String) {
        self.frame = frame
        self.score = score
        self.confidence = confidence.clamped(to: 0...1)
        self.reason = reason
    }
}

public struct DesktopWorldSnapshot: Codable, Hashable, Sendable {
    public let displays: [DisplaySnapshot]
    public let windows: [WindowSnapshot]
    public let pointer: PointerSnapshot?
    public let focus: FocusSnapshot?
    /// Downsampled luminance for the display being placed on, when the user
    /// turned visual placement on.
    public let luminance: LuminanceField?
    public let safeZones: [SafeZone]

    public init(
        displays: [DisplaySnapshot],
        windows: [WindowSnapshot] = [],
        pointer: PointerSnapshot? = nil,
        focus: FocusSnapshot? = nil,
        luminance: LuminanceField? = nil,
        safeZones: [SafeZone] = []
    ) {
        self.displays = displays
        self.windows = windows
        self.pointer = pointer
        self.focus = focus
        self.luminance = luminance
        self.safeZones = safeZones
    }

    /// Screen samples are deliberately absent from the coding keys, so encoding
    /// a world snapshot can never carry what was on the user's screen into a
    /// file, a log, or a request. A decoded snapshot always has no luminance.
    private enum CodingKeys: String, CodingKey {
        case displays
        case windows
        case pointer
        case focus
        case safeZones
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        self.init(
            displays: try container.decode([DisplaySnapshot].self, forKey: .displays),
            windows: try container.decodeIfPresent([WindowSnapshot].self, forKey: .windows) ?? [],
            pointer: try container.decodeIfPresent(PointerSnapshot.self, forKey: .pointer),
            focus: try container.decodeIfPresent(FocusSnapshot.self, forKey: .focus),
            luminance: nil,
            safeZones: try container.decodeIfPresent([SafeZone].self, forKey: .safeZones) ?? []
        )
    }

    public func display(containing point: WorldPoint) -> DisplaySnapshot? {
        displays.first(where: { $0.frame.contains(point) })
    }

    public func nearestDisplay(to point: WorldPoint) -> DisplaySnapshot? {
        displays.min { $0.frame.distance(to: point) < $1.frame.distance(to: point) }
    }

    public func clamp(_ point: WorldPoint, objectSize: WorldSize) -> WorldPoint {
        if let containing = displays.first(where: {
            $0.visibleFrame.insetBy(dx: objectSize.width / 2, dy: objectSize.height / 2).contains(point)
        }) {
            return containing.visibleFrame.clampedCenter(point, objectSize: objectSize)
        }
        guard let nearest = displays.min(by: {
            $0.visibleFrame.distance(to: point) < $1.visibleFrame.distance(to: point)
        }) else { return point }
        return nearest.visibleFrame.clampedCenter(point, objectSize: objectSize)
    }
}
