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

/// Loads the authored FatMochi runtime atlas, or derives a lightweight evaluation atlas
/// from a mascot's four approved key poses when no authored atlas is available.
public enum MascotPetFactory {
    private static let cellWidth = 192
    private static let cellHeight = 208
    private static let columns = 8
    private static let poseDerivedRows = 4
    private static let authoredFatMochiRows = 7

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
        if kind == .fatMochi,
           let atlas = loadSheet(named: "fat-mochi-runtime-atlas"),
           atlas.width == cellWidth * columns,
           atlas.height == cellHeight * authoredFatMochiRows {
            return makeAuthoredFatMochi(atlas: atlas)
        }

        return makePoseDerivedPet(kind)
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
                rows: authoredFatMochiRows
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
                frames: [(32, 0.13), (33, 0.13), (34, 0.13), (35, 0.13)]
            ),
            "dragged": track(
                "dragged",
                frames: [(32, 0.11), (33, 0.11), (34, 0.11), (35, 0.11)]
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
                frames: [(48, 0.10), (49, 0.10), (50, 0.10), (52, 0.22)],
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
                frames: [(32, 0.17), (33, 0.17), (34, 0.17), (35, 0.17)]
            ),
            "working": track("working", frames: walkRightFrames),
            "running": track("running", frames: walkRightFrames),
            "waving": track(
                "waving",
                frames: [(32, 0.16), (33, 0.16), (34, 0.16), (35, 0.16)]
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
            rows: authoredFatMochiRows,
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
                frames: [(24, 0.12), (21, 0.10), (3, 0.08), (0, 0.28)],
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
        let resourceURL = Bundle.module.url(
            forResource: name,
            withExtension: "png",
            subdirectory: "BuiltInPets"
        ) ?? Bundle.module.url(forResource: name, withExtension: "png")
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
