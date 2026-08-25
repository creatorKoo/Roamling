// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

import AppKit
import RoamlingMac

@main
struct RoamlingMain {
    @MainActor
    static func main() {
        let application = NSApplication.shared
        let delegate = RoamlingAppDelegate()
        application.delegate = delegate
        application.run()
        withExtendedLifetime(delegate) {}
    }
}
