// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

import Foundation
import RoamlingCore
import RoamlingEngine
import RoamlingSources

/// Assembles the macOS half of the app. Everything AppKit-specific about how
/// the runtime reaches the machine is decided here and nowhere else, so the
/// Windows port's job is to write the same function against its own providers.
@MainActor
public enum MacPlatform {
    public static func makeServices() -> PlatformServices {
        let coordinateSpace = CoordinateSpaceSource()
        // The providers read the space rather than holding a copy, so the
        // runtime can move the origin -- on a display change, say -- without
        // anyone being left converting against yesterday's desktop.
        let read = { coordinateSpace.current }

        let display = MacDisplayProvider()
        return PlatformServices(
            display: display,
            displayChanges: display,
            safeZone: BasicSafeZoneProvider(),
            userIdle: MacUserIdleProvider(),
            capture: MacCaptureProvider(),
            pointer: MacPointerProvider(coordinateSpace: read),
            window: MacWindowProvider(coordinateSpace: read),
            focus: MacFocusProvider(coordinateSpace: read),
            overlay: MacOverlayProvider(coordinateSpace: read),
            images: MacPetImageSource(),
            coordinateSpace: coordinateSpace
        )
    }

    /// The agents this build watches, in the order they are shown. Assembled
    /// here rather than inside the runtime so a platform with no hook transport
    /// yet can hand over an empty list and still get a pet that roams.
    public static func makeAgentIntegrations(
        defaults: UserDefaults = .standard
    ) -> [any AgentIntegration] {
        [
            ClaudeCodeIntegration(token: HookToken.loadOrCreate(
                key: HookToken.claudeCodeDefaultsKey,
                defaults: defaults
            )),
            CodexIntegration(token: HookToken.loadOrCreate(
                key: HookToken.codexDefaultsKey,
                defaults: defaults
            ))
        ]
    }
}
