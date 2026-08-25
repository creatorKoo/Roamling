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

/// Builds a small runtime atlas from the selected mascot's four approved key poses.
///
/// The source sheets deliberately remain pose auditions rather than pretending to be
/// finished animation. Tiny vertical offsets provide enough life for in-app evaluation
/// while the final hand-cleaned sprite sheets are still being authored.
public enum MascotPetFactory {
    private static let cellWidth = 192
    private static let cellHeight = 208
    private static let columns = 8
    private static let rows = 2

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

    public static func make(_ kind: BuiltInPetKind = .mochi) -> PetAsset {
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
                rows: rows
            )
        )

        let tracks = [
            "idle": track("idle", frames: [(0, 0.72), (1, 0.28)]),
            "running-right": track("running-right", frames: [(2, 0.17), (3, 0.17)]),
            "running-left": track("running-left", frames: [(4, 0.17), (5, 0.17)]),
            "sleeping": track("sleeping", frames: [(6, 0.9), (7, 0.9)]),
            "caught": track("caught", frames: [(8, 0.22), (9, 0.22)]),
            "dragged": track("dragged", frames: [(8, 0.5)]),
            "sitting": track("sitting", frames: [(10, 0.8)]),
            "landing": track("landing", frames: [(11, 0.14), (0, 0.3)], loops: false),
            "watching": track("watching", frames: [(12, 0.8)]),
            "failed": track("failed", frames: [(13, 0.8)]),
            "jumping": track("jumping", frames: [(14, 0.16), (11, 0.16), (0, 0.28)], loops: false),
            "stretching": track("stretching", frames: [(15, 0.35), (0, 0.3)], loops: false),
            "waiting": track("waiting", frames: [(8, 0.34), (9, 0.34)]),
            "working": track("working", frames: [(2, 0.2), (3, 0.2)]),
            "running": track("running", frames: [(2, 0.2), (3, 0.2)]),
            "waving": track("waving", frames: [(8, 0.25), (9, 0.25)]),
            "review": track("review", frames: [(12, 0.8)])
        ]

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
        let height = cellHeight * rows
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
            FrameRecipe(pose: .idle, mirrored: false, yOffset: 0, scale: 1),
            FrameRecipe(pose: .idle, mirrored: false, yOffset: 1.5, scale: 0.995),
            FrameRecipe(pose: .walkRight, mirrored: false, yOffset: 0, scale: 1),
            FrameRecipe(pose: .walkRight, mirrored: false, yOffset: 3, scale: 1),
            FrameRecipe(pose: .walkRight, mirrored: true, yOffset: 0, scale: 1),
            FrameRecipe(pose: .walkRight, mirrored: true, yOffset: 3, scale: 1),
            FrameRecipe(pose: .sleep, mirrored: false, yOffset: 0, scale: 1),
            FrameRecipe(pose: .sleep, mirrored: false, yOffset: 0, scale: 1.01),
            FrameRecipe(pose: .caught, mirrored: false, yOffset: 0, scale: 1),
            FrameRecipe(pose: .caught, mirrored: false, yOffset: 2, scale: 1),
            FrameRecipe(pose: .idle, mirrored: false, yOffset: -1, scale: 1),
            FrameRecipe(pose: .idle, mirrored: false, yOffset: 10, scale: 0.98),
            FrameRecipe(pose: .idle, mirrored: false, yOffset: 0, scale: 1),
            FrameRecipe(pose: .sleep, mirrored: false, yOffset: 0, scale: 1),
            FrameRecipe(pose: .caught, mirrored: false, yOffset: 12, scale: 0.98),
            FrameRecipe(pose: .idle, mirrored: false, yOffset: 0, scale: 1.015)
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
