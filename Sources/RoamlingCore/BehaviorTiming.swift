// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

import Foundation

/// How long a transient behavior holds before it hands back.
///
/// Most of these numbers are not ours. Petdex classes `waving`, `jumping`,
/// `failed` and `review` as duration states that play once and return to idle,
/// and publishes a standard length for each.
///
/// **Roamling matches those lengths exactly**, because a row drawn to the Petdex
/// contract is paced for them. Hold one longer and it loops -- a 0.7s greeting
/// replayed three times reads as a pet that will not stop waving. Hold it shorter
/// and it is cut mid-gesture. Any conforming pet dropped into Roamling has to
/// look the way its author intended, and that is only true when the clock agrees.
///
/// Steady states are different: `idle`, `running`, `waiting` and the walk have no
/// timer on either side, so a package is free to pace those loops however it
/// likes and Roamling simply plays them.
///
/// Core cannot import `RoamlingPet`, so the borrowed values are repeated here
/// and pinned by `BehaviorTiming matches the Petdex contract` in the logic tests.
/// If that test fails, the upstream numbers moved; port them, do not loosen it.
public enum BehaviorTiming {
    /// Petdex `jumping`: the turn just started.
    public static let spark: TimeInterval = 0.840

    /// Petdex `review`: reading or searching. Plays once, then stillness.
    public static let observe: TimeInterval = 1.030

    /// Petdex `waving`: the turn finished.
    public static let celebrate: TimeInterval = 0.700

    /// Petdex `failed`: a tool call failed.
    public static let sad: TimeInterval = 1.220

    /// Petdex `jumping` again, because a drop borrows the hop.
    ///
    /// A pet that does not author `landing` falls back to the jump row, and any
    /// length other than the jump's own cuts that row mid-air.
    public static let dropped: TimeInterval = 0.840

    /// Roamling-only. Petdex has no word for waking from a nap or stretching, so
    /// these are the only two lengths here that are ours to choose.
    public static let wake: TimeInterval = 0.7
    public static let stretch: TimeInterval = 1.0
}
