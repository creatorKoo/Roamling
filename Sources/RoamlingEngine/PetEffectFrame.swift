// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

import RoamlingCore

/// Filled polygon in pet-width units relative to its centre, with y down.
/// The core supplies geometry so every platform draws the same shape.
public struct PetEffectFrame: Equatable {
    public let points: [WorldPoint]
    public let red: UInt8
    public let green: UInt8
    public let blue: UInt8
    public let opacity: Double
}
