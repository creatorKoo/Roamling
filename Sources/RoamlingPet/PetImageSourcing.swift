// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

import Foundation

/// Where a pet layer gets pixels it cannot make itself.
///
/// Decoding is here rather than in `PetLoader` because it is the one part of
/// the pipeline that is genuinely a platform capability: macOS answers WebP and
/// PNG through ImageIO for free, and every other platform answers differently
/// or not at all. Everything downstream -- cropping, composing, the frame grid
/// -- is arithmetic on bytes and stays portable.
public protocol PetImageSourcing: Sendable {
    /// Decoded to RGBA8 premultiplied, top row first. Returns nil when the file
    /// is missing or in a format this platform cannot read.
    func decode(contentsOf url: URL) -> PetImage?

    /// Draws the last-resort sheet, reached only when the bundled art is
    /// missing or the wrong shape -- a broken build rather than a state a user
    /// can get into. It stays platform-side because it is antialiased vector
    /// art, not a data transform, and each platform can draw it its own way.
    func placeholderAtlas(
        columns: Int,
        rows: Int,
        cellWidth: Int,
        cellHeight: Int
    ) -> PetImage?
}
