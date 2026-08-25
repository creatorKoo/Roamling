// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

import AppKit
import RoamlingCore

@MainActor
public final class MacPointerProvider: PointerProviding {
    private let coordinateSpace: () -> DesktopCoordinateSpace

    public init(coordinateSpace: @escaping () -> DesktopCoordinateSpace) {
        self.coordinateSpace = coordinateSpace
    }

    public func currentPointer(at timestamp: TimeInterval) -> PointerSnapshot {
        let point = NSEvent.mouseLocation
        let appKitPoint = WorldPoint(x: Double(point.x), y: Double(point.y))
        let primaryDown = (NSEvent.pressedMouseButtons & 1) != 0
        return PointerSnapshot(
            position: coordinateSpace().pointFromAppKit(appKitPoint),
            timestamp: timestamp,
            primaryButtonDown: primaryDown
        )
    }
}
