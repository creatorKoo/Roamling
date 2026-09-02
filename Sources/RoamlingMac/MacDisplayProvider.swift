// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

import AppKit
import RoamlingCore

@MainActor
public final class MacDisplayProvider: DisplayProviding {
    public init() {}

    public func currentDisplaySet() -> DisplaySnapshotSet {
        let screens = NSScreen.screens
        let appKitFrames = screens.map { screen in
            WorldRect(
                x: Double(screen.frame.minX),
                y: Double(screen.frame.minY),
                width: Double(screen.frame.width),
                height: Double(screen.frame.height)
            )
        }
        let coordinateSpace = DesktopCoordinateSpace.fromAppKitFrames(appKitFrames)

        let displays = zip(screens, appKitFrames).map { screen, appKitFrame in
            let visible = WorldRect(
                x: Double(screen.visibleFrame.minX),
                y: Double(screen.visibleFrame.minY),
                width: Double(screen.visibleFrame.width),
                height: Double(screen.visibleFrame.height)
            )
            let screenNumberKey = NSDeviceDescriptionKey("NSScreenNumber")
            let number = (screen.deviceDescription[screenNumberKey] as? NSNumber)?.uint32Value
            return DisplaySnapshot(
                id: number.map(String.init) ?? "screen-\(screen.localizedName)-\(Int(screen.frame.minX))-\(Int(screen.frame.minY))",
                name: screen.localizedName,
                frame: coordinateSpace.rectFromAppKit(appKitFrame),
                visibleFrame: coordinateSpace.rectFromAppKit(visible),
                scale: Double(screen.backingScaleFactor)
            )
        }

        return DisplaySnapshotSet(displays: displays, coordinateSpace: coordinateSpace)
    }
}
