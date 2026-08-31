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

    /// A second sheet holding the frames Petdex has no room for.
    ///
    /// The package sheet is exactly the nine-row contract and stays that way;
    /// sleeping, being carried and watching the cursor live here instead. Frames
    /// on this sheet are addressed by continuing the index past the end of the
    /// package's own grid, so a track's frame list needs no notion of which
    /// sheet it is reading from.
    public let extensionAtlas: CGImage?
    public let extensionColumns: Int
    public let extensionRows: Int

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
        extensionAtlas: CGImage? = nil,
        extensionColumns: Int = 0,
        extensionRows: Int = 0,
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
        self.extensionAtlas = extensionAtlas
        self.extensionColumns = extensionColumns
        self.extensionRows = extensionRows
        self.warnings = warnings
    }

    public var resolver: AnimationResolver {
        AnimationResolver(tracks: tracks, explicitBehaviors: behaviorMappings)
    }

    /// Cells on the package sheet. Extension frames start at this index.
    public var frameCount: Int { columns * rows }

    /// Cells across both sheets, which is what a frame index is checked against.
    public var addressableFrameCount: Int {
        frameCount + (extensionAtlas == nil ? 0 : extensionColumns * extensionRows)
    }

    public var supportsDirectionalLook: Bool { rows >= 11 && columns >= 8 }

    public func frameImage(at index: Int) -> CGImage? {
        guard index >= 0, index < addressableFrameCount else { return nil }
        if let cached = frameCache[index] { return cached }
        let source: CGImage
        let offset: Int
        let stride: Int
        if index < frameCount {
            source = atlas
            offset = index
            stride = columns
        } else {
            guard let extensionAtlas else { return nil }
            source = extensionAtlas
            offset = index - frameCount
            stride = extensionColumns
        }
        let rect = CGRect(
            x: (offset % stride) * frameWidth,
            y: (offset / stride) * frameHeight,
            width: frameWidth,
            height: frameHeight
        )
        guard let image = source.cropping(to: rect) else { return nil }
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
