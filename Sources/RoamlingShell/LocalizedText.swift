// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

import Foundation

/// Menu, dialog, and panel copy.
///
/// English is the base localization and Korean ships beside it. Every other
/// system language resolves to English through the bundle's own fallback, so no
/// language check happens here.
///
/// It lives in the shell rather than in the AppKit module because the words are
/// not AppKit's: a Windows tray shows the same menu and has to say the same
/// things. W0 confirmed corelibs-foundation reads `.lproj` unchanged.
public func localized(_ key: String, _ comment: String = "") -> String {
    NSLocalizedString(key, bundle: .module, comment: comment)
}

/// Localized copy that carries a value, such as a pet name or a slider unit.
/// Kept separate from `localized(_:_:)` so a `String` argument cannot be read
/// as the comment parameter instead.
public func localizedFormat(_ key: String, _ arguments: any CVarArg...) -> String {
    String(format: NSLocalizedString(key, bundle: .module, comment: ""), arguments: arguments)
}

/// Which localization the bundle actually resolved to. A missing
/// `CFBundleLocalizations` pins the process to English however many `.lproj`
/// directories ship, and nothing crashes to say so -- the packaging smoke test
/// reads this to catch that.
public var resolvedLocalization: String {
    Bundle.module.preferredLocalizations.first ?? "none"
}
