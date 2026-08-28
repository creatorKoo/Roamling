// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

import CoreGraphics
import Foundation
import ImageIO

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

    private enum Pose: Int {
        case idle
        case walkRight
        case sleep
        case caught
    }

    private struct FrameRecipe {
        let pose: Pose
        let mirrored: Bool
        let yOffset: CGFloat
        let scale: CGFloat
    }

    public static func make(_ kind: BuiltInPetKind = .fatMochi) -> PetAsset {
        switch kind {
        case .mochi:
            if let atlas = loadSheet(named: "mochi-standard-atlas"),
               atlas.width == cellWidth * columns,
               atlas.height == cellHeight * standardRows {
                return makeStandardMochi(atlas: atlas)
            }
        case .fatMochi:
            if let atlas = loadSheet(named: "fat-mochi-runtime-atlas"),
               atlas.width == cellWidth * columns,
               atlas.height == cellHeight * authoredRows {
                return makeAuthoredFatMochi(atlas: atlas)
            }
        }

        return makePoseDerivedPet(kind)
    }

    /// Mochi ships the standard 8x9 Codex/Petdex row set rather than the
    /// seven-row layout the other built-in uses. Every row is authored art, and
    /// `AnimationResolver` already maps this taxonomy: work resolves to running,
    /// observe to review, paw to waving, celebrate and landing to jumping, fail
    /// to failed, caught and dragged to waiting. Only sit, sleep, and stretch
    /// have no row here and fall back to idle.
    private static func makeStandardMochi(atlas: CGImage) -> PetAsset {
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

        // MVP 2 asks a completion to hold real motion for about 2.2 seconds and
        // end at rest. The standard jumping track is a short loop, so the
        // celebration plays the arc out and back and settles on the neutral pose.
        tracks["jumping"] = track("jumping", frames: [
            (jumpRow, 0.16), (jumpRow + 1, 0.14), (jumpRow + 2, 0.18),
            (jumpRow + 3, 0.16), (jumpRow + 4, 0.20), (jumpRow + 3, 0.16),
            (jumpRow + 2, 0.18), (jumpRow + 1, 0.14), (jumpRow, 0.88)
        ], loops: false)

        // Without this, `.landing` falls through to jumping and the pet throws a
        // full celebration every time it is dropped.
        tracks["landing"] = track("landing", frames: [
            (jumpRow + 4, 0.10), (jumpRow + 3, 0.12),
            (jumpRow + 2, 0.10), (jumpRow, 0.18)
        ], loops: false)

        return PetAsset(
            manifest: manifest,
            packageURL: nil,
            atlas: atlas,
            frameWidth: cellWidth,
            frameHeight: cellHeight,
            columns: columns,
            rows: standardRows,
            tracks: tracks
        )
    }

    private static func makeAuthoredMochi(atlas: CGImage) -> PetAsset {
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
            "jumping": track(
                "jumping",
                frames: hop + hop + hop + [(53, 0.16)],
                loops: false
            ),
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

    private static func makeAuthoredFatMochi(atlas: CGImage) -> PetAsset {
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
            "jumping": track(
                "jumping",
                frames: [
                    // Two complete, reversible hops keep moving for the full
                    // completion reaction instead of freezing on the apex.
                    (48, 0.10), (49, 0.09), (50, 0.10), (52, 0.18),
                    (51, 0.08), (50, 0.08), (49, 0.10), (48, 0.22),
                    (49, 0.09), (50, 0.10), (52, 0.18), (51, 0.08),
                    (50, 0.08), (49, 0.10), (48, 0.62)
                ],
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

    private static func makePoseDerivedPet(_ kind: BuiltInPetKind) -> PetAsset {
        guard let sheet = loadSheet(named: kind.resourceName),
              let atlas = makeAtlas(from: sheet, kind: kind) else {
            return PlaceholderPetFactory.make()
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
            "jumping": track(
                "jumping",
                frames: [
                    // Mochi uses the approved caught/idle poses for three
                    // small hops, returning to the exact resting silhouette.
                    (24, 0.14), (21, 0.12), (3, 0.10), (0, 0.22),
                    (24, 0.14), (21, 0.12), (3, 0.10), (0, 0.22),
                    (24, 0.14), (21, 0.12), (3, 0.10), (0, 0.68)
                ],
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

    private static func loadSheet(named name: String) -> CGImage? {
        // WebP keeps the nine-row Mochi atlas under a megabyte and half the
        // size of the equivalent PNG, and ImageIO decodes both.
        let resourceURL = ["png", "webp"].lazy.compactMap { ext in
            Bundle.module.url(forResource: name, withExtension: ext, subdirectory: "BuiltInPets")
                ?? Bundle.module.url(forResource: name, withExtension: ext)
        }.first
        guard let resourceURL,
              let source = CGImageSourceCreateWithURL(resourceURL as CFURL, nil) else { return nil }
        return CGImageSourceCreateImageAtIndex(source, 0, nil)
    }

    private static func makeAtlas(from sheet: CGImage, kind: BuiltInPetKind) -> CGImage? {
        let crops = cropRects(for: kind)
        guard crops.count == 4,
              crops.allSatisfy({ rect in
                  rect.minX >= 0 && rect.minY >= 0
                      && rect.maxX <= CGFloat(sheet.width)
                      && rect.maxY <= CGFloat(sheet.height)
              }) else { return nil }

        let poses = crops.compactMap { sheet.cropping(to: $0) }
        guard poses.count == crops.count else { return nil }

        let width = cellWidth * columns
        let height = cellHeight * poseDerivedRows
        guard let context = CGContext(
            data: nil,
            width: width,
            height: height,
            bitsPerComponent: 8,
            bytesPerRow: 0,
            space: CGColorSpaceCreateDeviceRGB(),
            bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue
        ) else { return nil }

        context.clear(CGRect(x: 0, y: 0, width: width, height: height))
        context.interpolationQuality = .none
        context.setShouldAntialias(false)

        let recipes = frameRecipes
        for (index, recipe) in recipes.enumerated() {
            let row = index / columns
            let column = index % columns
            let cell = CGRect(
                x: column * cellWidth,
                y: height - (row + 1) * cellHeight,
                width: cellWidth,
                height: cellHeight
            )
            draw(
                poses[recipe.pose.rawValue],
                in: cell,
                mirrored: recipe.mirrored,
                yOffset: recipe.yOffset,
                scale: recipe.scale,
                context: context
            )
        }
        return context.makeImage()
    }

    private static func draw(
        _ image: CGImage,
        in cell: CGRect,
        mirrored: Bool,
        yOffset: CGFloat,
        scale: CGFloat,
        context: CGContext
    ) {
        let maximumWidth = cell.width - 10
        let maximumHeight = cell.height - 10
        let fit = min(
            maximumWidth / CGFloat(image.width),
            maximumHeight / CGFloat(image.height)
        ) * scale
        let size = CGSize(
            width: CGFloat(image.width) * fit,
            height: CGFloat(image.height) * fit
        )
        let destination = CGRect(
            x: cell.midX - size.width / 2,
            y: cell.minY + 5 + yOffset,
            width: size.width,
            height: size.height
        )

        context.saveGState()
        if mirrored {
            context.translateBy(x: destination.midX * 2, y: 0)
            context.scaleBy(x: -1, y: 1)
        }
        context.draw(image, in: destination)
        context.restoreGState()
    }

    private static func cropRects(for kind: BuiltInPetKind) -> [CGRect] {
        switch kind {
        case .mochi:
            [
                CGRect(x: 42, y: 332, width: 310, height: 354),
                CGRect(x: 408, y: 344, width: 354, height: 344),
                CGRect(x: 768, y: 448, width: 356, height: 242),
                CGRect(x: 1192, y: 330, width: 306, height: 360)
            ]
        case .fatMochi:
            [
                CGRect(x: 32, y: 334, width: 384, height: 380),
                CGRect(x: 432, y: 348, width: 346, height: 368),
                CGRect(x: 808, y: 438, width: 330, height: 280),
                CGRect(x: 1168, y: 378, width: 338, height: 342)
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
