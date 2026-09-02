// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

import Foundation

/// The last-resort pet, used only when no authored sheet can be read -- a
/// broken build rather than a state a user reaches.
///
/// The manifest and the track table are here because they are data. The sheet
/// itself is antialiased vector art, which is a drawing job and therefore a
/// platform one: `PetImageSourcing.placeholderAtlas` supplies the pixels.
public enum PlaceholderPetFactory {
    public static let cellWidth = 192
    public static let cellHeight = 208
    public static let columns = 8
    public static let rows = 11

    /// Which cells of each row the drawing fills, and therefore what a platform
    /// implementation has to cover. Follows the v2 grid so every runtime path
    /// stays exercised even on the fallback.
    public static let usedFrames = [6, 8, 8, 4, 5, 8, 6, 6, 6, 8, 8]

    public static func make(images: any PetImageSourcing) -> PetAsset {
        let atlas = images.placeholderAtlas(
            columns: columns,
            rows: rows,
            cellWidth: cellWidth,
            cellHeight: cellHeight
        ) ?? PetImage(
            width: cellWidth * columns,
            height: cellHeight * rows,
            pixels: [UInt8](repeating: 0, count: cellWidth * columns * cellHeight * rows * 4)
        )

        let manifest = PetManifest(
            id: "roamling-placeholder-cat",
            displayName: "Mochi",
            description: "Roamling's license-safe, code-drawn placeholder cat.",
            spriteVersionNumber: 2,
            spritesheetPath: "procedural://mochi"
        )
        var tracks = StandardPetAnimations.tracks(columns: columns)
        tracks["sleeping"] = PetAnimationTrack(
            name: "sleeping",
            frames: (0..<4).map { column in
                PetAnimationFrame(index: 5 * columns + column, duration: 0.8)
            },
            loops: true,
            fallback: "idle"
        )
        return PetAsset(
            manifest: manifest,
            packageURL: nil,
            atlas: atlas,
            frameWidth: cellWidth,
            frameHeight: cellHeight,
            columns: columns,
            rows: rows,
            tracks: tracks
        )
    }
}
