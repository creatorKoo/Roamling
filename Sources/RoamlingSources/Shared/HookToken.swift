// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

import Foundation

/// The shared secret between an agent's hook and Roamling's loopback receiver.
///
/// Kept next to the integrations rather than in the runtime, because the
/// runtime no longer knows which agents exist -- and a token is meaningless
/// without one.
public enum HookToken {
    public static let claudeCodeDefaultsKey = "roamling.claudeCodeHookToken"
    public static let codexDefaultsKey = "roamling.codexHookToken"

    /// Long enough that guessing it is not the cheap way in, and stable once
    /// written: the hook command in the user's config carries a copy.
    public static func loadOrCreate(key: String, defaults: UserDefaults) -> String {
        if let existing = defaults.string(forKey: key), existing.count >= 24 {
            return existing
        }
        let token = UUID().uuidString.replacingOccurrences(of: "-", with: "")
            + UUID().uuidString.replacingOccurrences(of: "-", with: "")
        defaults.set(token, forKey: key)
        return token
    }
}
