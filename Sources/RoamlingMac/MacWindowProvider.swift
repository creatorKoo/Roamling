// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

import AppKit
import RoamlingCore

/// Permission-free, coarse window bounds for MVP 1 placement. It never reads
/// window titles or contents. Accessibility-based focus/caret detail is MVP 3.
@MainActor
public final class MacWindowProvider: WindowProviding {
    private let coordinateSpace: () -> DesktopCoordinateSpace

    public init(coordinateSpace: @escaping () -> DesktopCoordinateSpace) {
        self.coordinateSpace = coordinateSpace
    }

    public func currentWindows() -> [WindowSnapshot] {
        guard let rawWindows = CGWindowListCopyWindowInfo(
            [.optionOnScreenOnly, .excludeDesktopElements],
            kCGNullWindowID
        ) as? [[String: Any]] else { return [] }

        let ownPID = ProcessInfo.processInfo.processIdentifier
        let frontmostPID = NSWorkspace.shared.frontmostApplication?.processIdentifier
        let primaryTop = NSScreen.screens.first?.frame.maxY ?? 0
        let space = coordinateSpace()

        return rawWindows.compactMap { info in
            guard (info[kCGWindowLayer as String] as? NSNumber)?.intValue == 0,
                  let pid = (info[kCGWindowOwnerPID as String] as? NSNumber)?.int32Value,
                  pid != ownPID,
                  let number = (info[kCGWindowNumber as String] as? NSNumber)?.uint32Value,
                  let bounds = info[kCGWindowBounds as String] as? [String: Any],
                  let cgRect = CGRect(dictionaryRepresentation: bounds as CFDictionary),
                  cgRect.width >= 120,
                  cgRect.height >= 100 else { return nil }

            let app = NSRunningApplication(processIdentifier: pid)
            return WindowSnapshot(
                id: String(number),
                applicationIdentifier: app?.bundleIdentifier,
                title: nil,
                frame: space.rectFromCoreGraphics(
                    WorldRect(
                        x: Double(cgRect.minX),
                        y: Double(cgRect.minY),
                        width: Double(cgRect.width),
                        height: Double(cgRect.height)
                    ),
                    primaryTop: Double(primaryTop)
                ),
                isFocused: pid == frontmostPID
            )
        }
    }

    public func currentActivityLocationHint() -> LocationHint? {
        let windows = currentWindows()
        guard let focused = windows
            .filter(\.isFocused)
            .max(by: { $0.frame.size.width * $0.frame.size.height
                < $1.frame.size.width * $1.frame.size.height }) else { return nil }
        return LocationHint(
            applicationIdentifier: focused.applicationIdentifier,
            approximateRegion: focused.frame,
            confidence: 0.55
        )
    }
}
