// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

import Foundation

/// Where this module's resources are once the app is assembled.
///
/// `Bundle.module` cannot answer that. SwiftPM generates it to look in exactly
/// two places -- next to `Bundle.main.bundleURL`, and at the absolute `.build`
/// path the binary was compiled at -- and for a hand-assembled `.app` neither
/// one is right. `bundleURL` is the `.app` itself, and a resource bundle at the
/// root of an app bundle is not something `codesign` will seal: it refuses with
/// "unsealed contents present in the bundle root". Resources belong under
/// `Contents/Resources`, which is `Bundle.main.resourceURL`, and that is the one
/// place the generated accessor never looks.
///
/// It went unnoticed because the second candidate is an absolute path into the
/// build directory of whichever machine compiled the binary. On that machine it
/// exists, so every build anyone tested here worked. The first machine to run a
/// binary it had not built was a user installing the first macOS release, and
/// it trapped before the app finished launching.
///
/// `Bundle.module` stays as the last resort: it is what makes `swift run` and
/// the test harness work, where the resources really are in the build directory
/// and there is no `.app` around them.
let shellResourceBundle: Bundle = resourceBundle(named: "Roamling_RoamlingShell", fallback: .module)

/// Looks where an assembled app keeps it, then defers to SwiftPM.
private func resourceBundle(named name: String, fallback: @autoclosure () -> Bundle) -> Bundle {
    if let resources = Bundle.main.resourceURL,
       let found = Bundle(url: resources.appendingPathComponent("\(name).bundle")) {
        return found
    }
    return fallback()
}
