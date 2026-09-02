// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

import Foundation

public enum BuiltInPetKind: String, CaseIterable, Codable, Sendable {
    case mochi
    case fatMochi = "fat-mochi"

    public var displayName: String {
        switch self {
        case .mochi: "Mochi"
        case .fatMochi: "FatMochi"
        }
    }

    fileprivate var manifestID: String { "roamling-\(rawValue)" }

    fileprivate var resourceName: String {
        switch self {
        case .mochi: "mochi-poses"
        case .fatMochi: "fat-mochi-poses"
        }
    }
}

/// Loads an authored built-in runtime atlas, or derives a lightweight evaluation atlas
/// from a mascot's four approved key poses when no authored atlas is available.
public enum MascotPetFactory {
    private static let cellWidth = 192
    private static let cellHeight = 208
    private static let columns = 8
    private static let poseDerivedRows = 4
    private static let authoredRows = 7
    private static let standardRows = 9
    private static let extensionSheetRows = 3

    private enum Pose: Int {
        case idle
        case walkRight
        case sleep
        case caught
    }

    private struct PoseCrop {
        let x: Int
        let y: Int
        let width: Int
        let height: Int
    }

    private struct FrameRecipe {
        let pose: Pose
        let mirrored: Bool
        let yOffset: Double
        let scale: Double
    }

    public static func make(
        _ kind: BuiltInPetKind = .fatMochi,
        images: any PetImageSourcing
    ) -> PetAsset {
        switch kind {
        case .mochi:
            if let atlas = loadSheet(named: "mochi-standard-atlas", images: images),
               atlas.width == cellWidth * columns,
               atlas.height == cellHeight * standardRows {
                return makeStandardMochi(atlas: atlas, images: images)
            }
        case .fatMochi:
            if let atlas = loadSheet(named: "fat-mochi-runtime-atlas", images: images),
               atlas.width == cellWidth * columns,
               atlas.height == cellHeight * authoredRows {
                return makeAuthoredFatMochi(atlas: atlas)
            }
        }

        return makePoseDerivedPet(kind, images: images)
    }

    /// Mochi ships the standard 8x9 Codex/Petdex row set rather than the
    /// seven-row layout the other built-in uses, plus the two-row extension
    /// sheet that carries what Petdex has no word for.
    ///
    /// The two sheets are the shipped `mochi-v3` package, and the timings below
    /// are its manifests transcribed. They are written out rather than parsed
    /// because a built-in has no package directory to read. Keep them in step
    /// with the shipped `mochi-v3` package when it changes -- the nine-row test
    /// pins each track's length against what those manifests declare, and
    /// checks every extension frame lands on a cell that has art rather than on
    /// one of the sheet's five spare cells.
    ///
    /// `idle` overrides the standard 1.10s. Its six frames are one long hold
    /// and a blink, so the standard timing blinks the cat continuously; the
    /// package holds frame zero for 1.2s and spends 0.5s on the blink.
    private static func makeStandardMochi(atlas: PetImage, images: any PetImageSourcing) -> PetAsset {
        let manifest = PetManifest(
            id: BuiltInPetKind.mochi.manifestID,
            displayName: BuiltInPetKind.mochi.displayName,
            description: "Roamling's built-in Mochi mascot.",
            spritesheetPath: "builtin://mochi",
            frame: PetFrameManifest(
                width: cellWidth,
                height: cellHeight,
                columns: columns,
                rows: standardRows
            )
        )
        var tracks = StandardPetAnimations.tracks(columns: columns)
        let jumpRow = 4 * columns

        // A finished turn waves, and this sheet authors that row, so nothing
        // needs writing out here: `.celebrate` resolves straight to `waving` at
        // the Petdex length. `jumping` is left alone -- it opens a turn.

        tracks["idle"] = track("idle", frames: [
            (0, 1.20), (1, 0.10), (2, 0.10), (3, 0.10), (4, 0.10), (5, 0.10)
        ])

        // Without this, `.landing` falls through to jumping and the pet throws a
        // full celebration every time it is dropped.
        tracks["landing"] = track("landing", frames: [
            (jumpRow + 4, 0.10), (jumpRow + 3, 0.12),
            (jumpRow + 2, 0.10), (jumpRow, 0.18)
        ], loops: false)

        var extensionAtlas: PetImage?
        var extensionColumns = 0
        var extensionRows = 0
        if let sheet = loadSheet(named: "mochi-extension-atlas", images: images),
           sheet.width == cellWidth * columns,
           sheet.height == cellHeight * extensionSheetRows {
            extensionAtlas = sheet
            extensionColumns = columns
            extensionRows = extensionSheetRows

            // Frame indices continue past the package grid, so the first cell of
            // the extension sheet is 72. `gaze` is the exception: it points back
            // into the package's own review row and is played faster the closer
            // the pointer gets, so the tail flick doubles as watching.
            let base = columns * standardRows
            tracks["gaze"] = track("gaze", frames:
                (0..<6).map { (8 * columns + $0, 0.172) })
            tracks["sleeping"] = track("sleeping", frames:
                (0..<3).map { (base + $0, 0.667) })
            tracks["caught"] = track("caught", frames:
                (3..<7).map { (base + $0, 0.150) })
            tracks["sitting"] = track("sitting", frames:
                (0..<4).map { (base + columns + $0, 0.600) }, loops: false)
            // `wake` and `stretch` are one capability, and the player only
            // restarts a track when the capability changes, so these eight run
            // straight through both states rather than replaying the first half.
            tracks["stretching"] = track("stretching", frames:
                (0..<8).map { (base + columns * 2 + $0, 0.212) }, loops: false)
        }

        return PetAsset(
            manifest: manifest,
            packageURL: nil,
            atlas: atlas,
            frameWidth: cellWidth,
            frameHeight: cellHeight,
            columns: columns,
            rows: standardRows,
            tracks: tracks,
            behaviorMappings: extensionAtlas == nil ? [:] : [
                "gaze": "gaze", "sleep": "sleeping", "caught": "caught",
                "sit": "sitting", "stretch": "stretching"
            ],
            extensionAtlas: extensionAtlas,
            extensionColumns: extensionColumns,
            extensionRows: extensionRows
        )
    }

    private static func makeAuthoredMochi(atlas: PetImage) -> PetAsset {
        let manifest = PetManifest(
            id: BuiltInPetKind.mochi.manifestID,
            displayName: BuiltInPetKind.mochi.displayName,
            description: "Roamling's built-in Mochi mascot.",
            spritesheetPath: "builtin://mochi",
            frame: PetFrameManifest(
                width: cellWidth,
                height: cellHeight,
                columns: columns,
                rows: authoredRows
            )
        )

        let idleFrames: [(Int, TimeInterval)] = [
            (0, 1.45), (1, 0.08), (2, 0.07), (3, 0.10),
            (4, 0.07), (5, 0.08), (6, 0.12), (7, 0.20)
        ]
        let walkRightFrames = (8...15).map { ($0, 0.09) }
        let walkLeftFrames = (16...23).map { ($0, 0.09) }
        let hop: [(Int, TimeInterval)] = [
            (48, 0.09), (49, 0.08), (50, 0.10),
            (51, 0.14), (52, 0.10), (53, 0.17)
        ]

        let tracks = [
            "idle": track("idle", frames: idleFrames),
            "running-right": track("running-right", frames: walkRightFrames),
            "running-left": track("running-left", frames: walkLeftFrames),
            "sleeping": track(
                "sleeping",
                frames: [(24, 0.55), (25, 0.55), (26, 0.55), (27, 0.55)]
            ),
            "caught": track(
                "caught",
                frames: [(32, 0.04), (33, 0.08), (34, 0.09), (35, 0.11)],
                loops: false
            ),
            "dragged": track(
                "dragged",
                frames: [(36, 0.10), (37, 0.10), (38, 0.10), (39, 0.10)]
            ),
            "sitting": track("sitting", frames: [(0, 0.8)]),
            "landing": track(
                "landing",
                frames: [(48, 0.08), (49, 0.08), (50, 0.10),
                         (51, 0.14), (52, 0.10), (53, 0.22)],
                loops: false
            ),
            "watching": track("watching", frames: idleFrames),
            "failed": track("failed", frames: [(24, 0.8)]),
            "celebrate": track(
                "celebrate",
                frames: hop + hop + hop + [(53, 0.16)],
                loops: false
            ),
            // The opening hop is one bound, not three: Petdex plays `jumping`
            // when a turn starts and hands back inside a second.
            "jumping": track("jumping", frames: hop, loops: false),
            "stretching": track(
                "stretching",
                frames: [(40, 0.14), (41, 0.15), (42, 0.18),
                         (43, 0.20), (44, 0.15), (45, 0.18)],
                loops: false
            ),
            "waiting": track(
                "waiting",
                frames: [(36, 0.13), (37, 0.13), (38, 0.13), (39, 0.13)]
            ),
            "working": track("working", frames: walkRightFrames),
            "running": track("running", frames: walkRightFrames),
            "waving": track(
                "waving",
                frames: [(36, 0.13), (37, 0.13), (38, 0.13), (39, 0.13)]
            ),
            "review": track("review", frames: idleFrames)
        ]

        return PetAsset(
            manifest: manifest,
            packageURL: nil,
            atlas: atlas,
            frameWidth: cellWidth,
            frameHeight: cellHeight,
            columns: columns,
            rows: authoredRows,
            tracks: tracks
        )
    }

    private static func makeAuthoredFatMochi(atlas: PetImage) -> PetAsset {
        let manifest = PetManifest(
            id: BuiltInPetKind.fatMochi.manifestID,
            displayName: BuiltInPetKind.fatMochi.displayName,
            description: "Roamling's built-in FatMochi mascot.",
            spritesheetPath: "builtin://fat-mochi",
            frame: PetFrameManifest(
                width: cellWidth,
                height: cellHeight,
                columns: columns,
                rows: authoredRows
            )
        )

        let idleFrames: [(Int, TimeInterval)] = [
            (0, 1.25), (1, 0.10), (2, 0.12), (3, 0.16), (4, 0.12), (5, 0.10)
        ]
        let walkRightFrames = (8...15).map { ($0, 0.09) }
        let walkLeftFrames = (16...23).map { ($0, 0.09) }

        let tracks = [
            "idle": track("idle", frames: idleFrames),
            "running-right": track("running-right", frames: walkRightFrames),
            "running-left": track("running-left", frames: walkLeftFrames),
            "sleeping": track(
                "sleeping",
                frames: [(24, 0.55), (25, 0.55), (26, 0.55), (27, 0.55)]
            ),
            "caught": track(
                "caught",
                frames: [(32, 0.04), (33, 0.08), (34, 0.09), (35, 0.11)],
                loops: false
            ),
            "dragged": track(
                "dragged",
                frames: [(36, 0.09), (37, 0.09), (38, 0.09), (39, 0.09)]
            ),
            "sitting": track("sitting", frames: [(0, 0.8)]),
            "landing": track(
                "landing",
                frames: [(48, 0.055), (49, 0.055), (50, 0.07), (51, 0.06), (52, 0.11)],
                loops: false
            ),
            "watching": track("watching", frames: idleFrames),
            "failed": track("failed", frames: [(24, 0.8)]),
            // One bound: Petdex plays `jumping` when a turn opens and hands back
            // inside a second.
            "jumping": track(
                "jumping",
                frames: [(48, 0.10), (49, 0.09), (50, 0.10),
                         (52, 0.18), (51, 0.08), (48, 0.29)],
                loops: false
            ),
            // Roamling's own name for a finished turn. This sheet has no wave,
            // so the completion is a hop -- but it runs for `waving`'s 0.70s,
            // not the jump's, because that is how long the state holds.
            "celebrate": track(
                "celebrate",
                frames: [(48, 0.09), (49, 0.08), (50, 0.09),
                         (52, 0.16), (51, 0.07), (48, 0.21)],
                loops: false
            ),
            "stretching": track(
                "stretching",
                frames: [(40, 0.16), (41, 0.13), (42, 0.13),
                         (43, 0.18), (44, 0.15), (45, 0.20)],
                loops: false
            ),
            "waiting": track(
                "waiting",
                frames: [(36, 0.12), (37, 0.12), (38, 0.12), (39, 0.12)]
            ),
            "working": track("working", frames: walkRightFrames),
            "running": track("running", frames: walkRightFrames),
            "waving": track(
                "waving",
                frames: [(36, 0.12), (37, 0.12), (38, 0.12), (39, 0.12)]
            ),
            "review": track("review", frames: idleFrames)
        ]

        return PetAsset(
            manifest: manifest,
            packageURL: nil,
            atlas: atlas,
            frameWidth: cellWidth,
            frameHeight: cellHeight,
            columns: columns,
            rows: authoredRows,
            tracks: tracks
        )
    }

    private static func makePoseDerivedPet(_ kind: BuiltInPetKind, images: any PetImageSourcing) -> PetAsset {
        guard let sheet = loadSheet(named: kind.resourceName, images: images),
              let atlas = makeAtlas(from: sheet, kind: kind) else {
            return PlaceholderPetFactory.make(images: images)
        }

        let manifest = PetManifest(
            id: kind.manifestID,
            displayName: kind.displayName,
            description: "Roamling's built-in \(kind.displayName) mascot.",
            spritesheetPath: "builtin://\(kind.rawValue)",
            frame: PetFrameManifest(
                width: cellWidth,
                height: cellHeight,
                columns: columns,
                rows: poseDerivedRows
            )
        )

        let tracks = [
            // The quick reversible settle reads as a tiny blink/breath without a
            // hard jump back to the resting pose.
            "idle": track(
                "idle",
                frames: [(0, 1.55), (1, 0.09), (2, 0.08), (3, 0.10), (2, 0.08), (1, 0.09)]
            ),
            "running-right": track(
                "running-right",
                frames: [(4, 0.075), (5, 0.075), (6, 0.075), (7, 0.075),
                         (8, 0.075), (7, 0.075), (6, 0.075), (5, 0.075)]
            ),
            "running-left": track(
                "running-left",
                frames: [(9, 0.075), (10, 0.075), (11, 0.075), (12, 0.075),
                         (13, 0.075), (12, 0.075), (11, 0.075), (10, 0.075)]
            ),
            "sleeping": track(
                "sleeping",
                frames: [(14, 0.52), (15, 0.52), (16, 0.52), (15, 0.52)]
            ),
            "caught": track(
                "caught",
                frames: [(17, 0.14), (18, 0.14), (19, 0.14), (18, 0.14)]
            ),
            "dragged": track("dragged", frames: [(17, 0.5)]),
            "sitting": track("sitting", frames: [(20, 0.8)]),
            "landing": track(
                "landing",
                frames: [(21, 0.10), (3, 0.08), (2, 0.08), (1, 0.08), (0, 0.3)],
                loops: false
            ),
            "watching": track("watching", frames: [(22, 0.8)]),
            "failed": track("failed", frames: [(23, 0.8)]),
            // One hop, returning to the exact resting silhouette, held for
            // `waving`'s 0.70s.
            "celebrate": track(
                "celebrate",
                frames: [(24, 0.16), (21, 0.14), (3, 0.12), (0, 0.28)],
                loops: false
            ),
            // A turn opening is one of those hops, not all three.
            "jumping": track(
                "jumping",
                frames: [(24, 0.14), (21, 0.12), (3, 0.10), (0, 0.48)],
                loops: false
            ),
            "stretching": track(
                "stretching",
                frames: [(25, 0.22), (3, 0.08), (2, 0.08), (1, 0.08), (0, 0.3)],
                loops: false
            ),
            "waiting": track(
                "waiting",
                frames: [(17, 0.17), (18, 0.17), (19, 0.17), (18, 0.17)]
            ),
            "working": track(
                "working",
                frames: [(4, 0.09), (5, 0.09), (6, 0.09), (7, 0.09),
                         (8, 0.09), (7, 0.09), (6, 0.09), (5, 0.09)]
            ),
            "running": track(
                "running",
                frames: [(4, 0.09), (5, 0.09), (6, 0.09), (7, 0.09),
                         (8, 0.09), (7, 0.09), (6, 0.09), (5, 0.09)]
            ),
            "waving": track(
                "waving",
                frames: [(17, 0.16), (18, 0.16), (19, 0.16), (18, 0.16)]
            ),
            "review": track("review", frames: [(22, 0.8)])
        ]

        return PetAsset(
            manifest: manifest,
            packageURL: nil,
            atlas: atlas,
            frameWidth: cellWidth,
            frameHeight: cellHeight,
            columns: columns,
            rows: poseDerivedRows,
            tracks: tracks
        )
    }

    private static func loadSheet(named name: String, images: any PetImageSourcing) -> PetImage? {
        // WebP keeps the nine-row Mochi atlas under a megabyte and half the
        // size of the equivalent PNG, and ImageIO decodes both.
        let resourceURL = ["png", "webp"].lazy.compactMap { ext in
            Bundle.module.url(forResource: name, withExtension: ext, subdirectory: "BuiltInPets")
                ?? Bundle.module.url(forResource: name, withExtension: ext)
        }.first
        guard let resourceURL,
              let decoded = images.decode(contentsOf: resourceURL) else { return nil }
        return decoded
    }

    /// Reached only when the authored sheet for a built-in is missing or the
    /// wrong shape, which is a broken build. Kept working, but unlike the
    /// shipped sheets its pixels are not pinned -- it scales, and a hand-written
    /// nearest-neighbour blit need not land on CoreGraphics' rounding.
    private static func makeAtlas(from sheet: PetImage, kind: BuiltInPetKind) -> PetImage? {
        let crops = cropRects(for: kind)
        guard crops.count == 4,
              crops.allSatisfy({ rect in
                  rect.x >= 0 && rect.y >= 0
                      && rect.x + rect.width <= sheet.width
                      && rect.y + rect.height <= sheet.height
              }) else { return nil }

        let poses = crops.compactMap {
            sheet.cropped(x: $0.x, y: $0.y, width: $0.width, height: $0.height)
        }
        guard poses.count == crops.count else { return nil }

        var canvas = PetImageCanvas(
            width: cellWidth * columns,
            height: cellHeight * poseDerivedRows
        )
        for (index, recipe) in frameRecipes.enumerated() {
            draw(
                poses[recipe.pose.rawValue],
                inCellRow: index / columns,
                column: index % columns,
                mirrored: recipe.mirrored,
                yOffset: recipe.yOffset,
                scale: recipe.scale,
                canvas: &canvas
            )
        }
        return canvas.image()
    }

    /// `yOffset` still lifts the pose off the cell's floor; the canvas counts
    /// rows from the top, so the 5pt footing is measured from the bottom edge.
    private static func draw(
        _ image: PetImage,
        inCellRow row: Int,
        column: Int,
        mirrored: Bool,
        yOffset: Double,
        scale: Double,
        canvas: inout PetImageCanvas
    ) {
        let fit = min(
            Double(cellWidth - 10) / Double(image.width),
            Double(cellHeight - 10) / Double(image.height)
        ) * scale
        let width = Int((Double(image.width) * fit).rounded())
        let height = Int((Double(image.height) * fit).rounded())
        let cellX = column * cellWidth
        let cellBottom = (row + 1) * cellHeight
        canvas.blit(
            image,
            toX: cellX + (cellWidth - width) / 2,
            toY: cellBottom - Int((5 + yOffset).rounded()) - height,
            width: width,
            height: height,
            mirrored: mirrored
        )
    }

    private static func cropRects(for kind: BuiltInPetKind) -> [PoseCrop] {
        switch kind {
        case .mochi:
            [
                PoseCrop(x: 42, y: 332, width: 310, height: 354),
                PoseCrop(x: 408, y: 344, width: 354, height: 344),
                PoseCrop(x: 768, y: 448, width: 356, height: 242),
                PoseCrop(x: 1192, y: 330, width: 306, height: 360)
            ]
        case .fatMochi:
            [
                PoseCrop(x: 32, y: 334, width: 384, height: 380),
                PoseCrop(x: 432, y: 348, width: 346, height: 368),
                PoseCrop(x: 808, y: 438, width: 330, height: 280),
                PoseCrop(x: 1168, y: 378, width: 338, height: 342)
            ]
        }
    }

    private static var frameRecipes: [FrameRecipe] {
        [
            // 0...3: idle settle / blink-like breathing.
            FrameRecipe(pose: .idle, mirrored: false, yOffset: 0, scale: 1),
            FrameRecipe(pose: .idle, mirrored: false, yOffset: 0.5, scale: 0.999),
            FrameRecipe(pose: .idle, mirrored: false, yOffset: 1, scale: 0.997),
            FrameRecipe(pose: .idle, mirrored: false, yOffset: 1.5, scale: 0.995),

            // 4...8: right-facing waddle, sampled on the way up and down.
            FrameRecipe(pose: .walkRight, mirrored: false, yOffset: 0, scale: 1),
            FrameRecipe(pose: .walkRight, mirrored: false, yOffset: 0.75, scale: 0.998),
            FrameRecipe(pose: .walkRight, mirrored: false, yOffset: 1.5, scale: 0.996),
            FrameRecipe(pose: .walkRight, mirrored: false, yOffset: 2.25, scale: 0.994),
            FrameRecipe(pose: .walkRight, mirrored: false, yOffset: 3, scale: 0.992),

            // 9...13: the same phase sequence, mirrored for left travel.
            FrameRecipe(pose: .walkRight, mirrored: true, yOffset: 0, scale: 1),
            FrameRecipe(pose: .walkRight, mirrored: true, yOffset: 0.75, scale: 0.998),
            FrameRecipe(pose: .walkRight, mirrored: true, yOffset: 1.5, scale: 0.996),
            FrameRecipe(pose: .walkRight, mirrored: true, yOffset: 2.25, scale: 0.994),
            FrameRecipe(pose: .walkRight, mirrored: true, yOffset: 3, scale: 0.992),

            // 14...16: slow reversible sleep breathing.
            FrameRecipe(pose: .sleep, mirrored: false, yOffset: 0, scale: 1),
            FrameRecipe(pose: .sleep, mirrored: false, yOffset: 0, scale: 1.005),
            FrameRecipe(pose: .sleep, mirrored: false, yOffset: 0, scale: 1.01),

            // 17...19: caught/waiting bounce.
            FrameRecipe(pose: .caught, mirrored: false, yOffset: 0, scale: 1),
            FrameRecipe(pose: .caught, mirrored: false, yOffset: 1, scale: 0.997),
            FrameRecipe(pose: .caught, mirrored: false, yOffset: 2, scale: 0.994),

            // 20...25: single-purpose transition anchors.
            FrameRecipe(pose: .idle, mirrored: false, yOffset: -1, scale: 1),
            FrameRecipe(pose: .idle, mirrored: false, yOffset: 10, scale: 0.98),
            FrameRecipe(pose: .idle, mirrored: false, yOffset: 0, scale: 1),
            FrameRecipe(pose: .sleep, mirrored: false, yOffset: 0, scale: 1),
            FrameRecipe(pose: .caught, mirrored: false, yOffset: 12, scale: 0.98),
            FrameRecipe(pose: .idle, mirrored: false, yOffset: 0, scale: 1.015),

            // 26...31: safe non-transparent padding for the rectangular atlas.
            FrameRecipe(pose: .idle, mirrored: false, yOffset: 0, scale: 1),
            FrameRecipe(pose: .walkRight, mirrored: false, yOffset: 0, scale: 1),
            FrameRecipe(pose: .walkRight, mirrored: true, yOffset: 0, scale: 1),
            FrameRecipe(pose: .sleep, mirrored: false, yOffset: 0, scale: 1),
            FrameRecipe(pose: .caught, mirrored: false, yOffset: 0, scale: 1),
            FrameRecipe(pose: .idle, mirrored: false, yOffset: 0, scale: 1)
        ]
    }

    private static func track(
        _ name: String,
        frames: [(Int, TimeInterval)],
        loops: Bool = true
    ) -> PetAnimationTrack {
        PetAnimationTrack(
            name: name,
            frames: frames.map { PetAnimationFrame(index: $0.0, duration: $0.1) },
            loops: loops,
            fallback: "idle"
        )
    }
}
