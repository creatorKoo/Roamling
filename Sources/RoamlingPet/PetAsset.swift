// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

import CoreGraphics
import Foundation

public final class PetAsset {
    public let manifest: PetManifest
    public let packageURL: URL?
    public let atlas: CGImage
    public let frameWidth: Int
    public let frameHeight: Int
    public let columns: Int
    public let rows: Int
    public let tracks: [String: PetAnimationTrack]
    public let behaviorMappings: [String: String]
    public let warnings: [String]

    private var frameCache: [Int: CGImage] = [:]

    public init(
        manifest: PetManifest,
        packageURL: URL?,
        atlas: CGImage,
        frameWidth: Int,
        frameHeight: Int,
        columns: Int,
        rows: Int,
        tracks: [String: PetAnimationTrack],
        behaviorMappings: [String: String] = [:],
        warnings: [String] = []
    ) {
        self.manifest = manifest
        self.packageURL = packageURL
        self.atlas = atlas
        self.frameWidth = frameWidth
        self.frameHeight = frameHeight
        self.columns = columns
        self.rows = rows
        self.tracks = tracks
        self.behaviorMappings = behaviorMappings
        self.warnings = warnings
    }

    public var resolver: AnimationResolver {
        AnimationResolver(tracks: tracks, explicitBehaviors: behaviorMappings)
    }

    public var frameCount: Int { columns * rows }
    public var supportsDirectionalLook: Bool { rows >= 11 && columns >= 8 }

    public func frameImage(at index: Int) -> CGImage? {
        guard index >= 0, index < frameCount else { return nil }
        if let cached = frameCache[index] { return cached }
        let column = index % columns
        let row = index / columns
        let rect = CGRect(
            x: column * frameWidth,
            y: row * frameHeight,
            width: frameWidth,
            height: frameHeight
        )
        guard let image = atlas.cropping(to: rect) else { return nil }
        frameCache[index] = image
        return image
    }

    public func lookFrameIndex(degrees: Double) -> Int? {
        guard supportsDirectionalLook else { return nil }
        let normalized = degrees.truncatingRemainder(dividingBy: 360) + (degrees < 0 ? 360 : 0)
        let direction = Int((normalized / 22.5).rounded()) % 16
        if direction < 8 { return 9 * columns + direction }
        return 10 * columns + (direction - 8)
    }
}
