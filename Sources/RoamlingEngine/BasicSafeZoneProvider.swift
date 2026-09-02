// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

import Foundation
import RoamlingCore

/// MVP 0.7's permission-free provider. Platform-specific menu bar, Dock and
/// taskbar exclusions have already been normalized into each display's
/// `visibleFrame`, so what is left is arithmetic every platform shares.
@MainActor
public final class BasicSafeZoneProvider: SafeZoneProviding {
    public init() {}

    public func safeZones(in world: DesktopWorldSnapshot) -> [SafeZone] {
        BasicSafeZonePlanner.safeZones(in: world)
    }
}
