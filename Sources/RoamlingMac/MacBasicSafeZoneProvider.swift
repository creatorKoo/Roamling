// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

import Foundation
import RoamlingCore

/// MVP 0.7's permission-free provider. AppKit-specific menu bar and Dock
/// exclusion has already been normalized into each display's `visibleFrame`.
@MainActor
public final class MacBasicSafeZoneProvider: SafeZoneProviding {
    public init() {}

    public func safeZones(in world: DesktopWorldSnapshot) -> [SafeZone] {
        BasicSafeZonePlanner.safeZones(in: world)
    }
}
