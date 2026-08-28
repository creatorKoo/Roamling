// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

import Foundation

/// Converts a macOS global screen plane (bottom-left, y-up) into the core world
/// plane (top-left, y-down). Values are logical points, never backing pixels.
public struct DesktopCoordinateSpace: Codable, Hashable, Sendable {
    public let worldTop: Double

    public init(worldTop: Double) {
        self.worldTop = worldTop
    }

    public static func fromAppKitFrames(_ frames: [WorldRect]) -> DesktopCoordinateSpace {
        DesktopCoordinateSpace(worldTop: frames.map(\.maxY).max() ?? 0)
    }

    public func pointFromAppKit(_ point: WorldPoint) -> WorldPoint {
        WorldPoint(x: point.x, y: worldTop - point.y)
    }

    public func pointToAppKit(_ point: WorldPoint) -> WorldPoint {
        WorldPoint(x: point.x, y: worldTop - point.y)
    }

    public func rectFromAppKit(_ rect: WorldRect) -> WorldRect {
        WorldRect(
            x: rect.minX,
            y: worldTop - rect.maxY,
            width: rect.size.width,
            height: rect.size.height
        )
    }

    /// `CGWindowListCopyWindowInfo` and the accessibility APIs both report
    /// top-left origin rects on a plane anchored to the primary display, which
    /// is not the same anchor as the world plane once a second display sits
    /// above it. Both adapters must fold them in through here.
    public func rectFromCoreGraphics(_ rect: WorldRect, primaryTop: Double) -> WorldRect {
        rectFromAppKit(WorldRect(
            x: rect.minX,
            y: primaryTop - rect.maxY,
            width: rect.size.width,
            height: rect.size.height
        ))
    }

    public func rectToAppKit(_ rect: WorldRect) -> WorldRect {
        WorldRect(
            x: rect.minX,
            y: worldTop - rect.maxY,
            width: rect.size.width,
            height: rect.size.height
        )
    }
}
