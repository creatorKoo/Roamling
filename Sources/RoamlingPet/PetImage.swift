// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

import Foundation

/// A decoded sprite sheet or frame, as bytes rather than a platform image.
///
/// RGBA8, premultiplied alpha, row-major with the top row first and no padding
/// between rows -- the same layout `CGContext` hands back, so the pixels that
/// crossed this boundary before and after W2 are the same pixels.
///
/// A class rather than a struct because identity is load-bearing: the overlay
/// skips a redraw when handed the frame it is already showing, and it decides
/// that by identity. A struct would make every tick look like a new frame.
public final class PetImage {
    public let width: Int
    public let height: Int
    public let pixels: [UInt8]

    public init(width: Int, height: Int, pixels: [UInt8]) {
        precondition(width >= 0 && height >= 0, "negative image size")
        precondition(pixels.count == width * height * 4, "pixel buffer does not match size")
        self.width = width
        self.height = height
        self.pixels = pixels
    }

    public var isEmpty: Bool { width == 0 || height == 0 }

    /// The rectangular copy `CGImage.cropping(to:)` did, with the same origin
    /// convention: y counts down from the top row.
    public func cropped(x: Int, y: Int, width cropWidth: Int, height cropHeight: Int) -> PetImage? {
        guard cropWidth > 0, cropHeight > 0,
              x >= 0, y >= 0,
              x + cropWidth <= width, y + cropHeight <= height else { return nil }
        var out = [UInt8](repeating: 0, count: cropWidth * cropHeight * 4)
        let sourceStride = width * 4
        let destinationStride = cropWidth * 4
        pixels.withUnsafeBufferPointer { source in
            out.withUnsafeMutableBufferPointer { destination in
                for row in 0..<cropHeight {
                    let from = (y + row) * sourceStride + x * 4
                    let to = row * destinationStride
                    destination.baseAddress!.advanced(by: to)
                        .update(from: source.baseAddress!.advanced(by: from), count: destinationStride)
                }
            }
        }
        return PetImage(width: cropWidth, height: cropHeight, pixels: out)
    }
}

/// Anything that can hand over a rectangle of RGBA8 bytes: a whole sheet, or
/// one cell of one. Lets the frame checks read either without caring which.
public protocol PetPixels {
    var width: Int { get }
    var height: Int { get }
    /// Contiguous RGBA8, top row first. A frame materializes this on demand, so
    /// hold the result rather than asking twice in a loop.
    var pixels: [UInt8] { get }
}

extension PetImage: PetPixels {}

/// One cell of a sheet, described rather than copied.
///
/// Frames used to be `CGImage.cropping(to:)`, which shares the sheet's backing
/// store: a hundred of them cost nothing. Copying each cell out instead would
/// add about 16 MB per pet for a pet that shows one frame at a time, so a frame
/// stays a rectangle and the bytes are cut only when someone asks.
public struct PetFrame: PetPixels {
    public let sheet: PetImage
    public let x: Int
    public let y: Int
    public let width: Int
    public let height: Int

    public init(sheet: PetImage, x: Int, y: Int, width: Int, height: Int) {
        self.sheet = sheet
        self.x = x
        self.y = y
        self.width = width
        self.height = height
    }

    public var pixels: [UInt8] {
        sheet.cropped(x: x, y: y, width: width, height: height)?.pixels
            ?? [UInt8](repeating: 0, count: max(0, width * height * 4))
    }

    /// A sub-rectangle of this frame, in the frame's own coordinates.
    public func cropped(x cropX: Int, y cropY: Int, width cropWidth: Int, height cropHeight: Int) -> PetImage? {
        guard cropX >= 0, cropY >= 0,
              cropX + cropWidth <= width, cropY + cropHeight <= height else { return nil }
        return sheet.cropped(x: x + cropX, y: y + cropY, width: cropWidth, height: cropHeight)
    }
}

/// A mutable RGBA8 surface for composing sheets. Starts fully transparent,
/// which is what `CGContext.clear` gave the composition path it replaces.
public struct PetImageCanvas {
    public let width: Int
    public let height: Int
    private var pixels: [UInt8]

    public init(width: Int, height: Int) {
        precondition(width > 0 && height > 0, "empty canvas")
        self.width = width
        self.height = height
        pixels = [UInt8](repeating: 0, count: width * height * 4)
    }

    /// Nearest-neighbour scaled copy, optionally mirrored horizontally --
    /// the `interpolationQuality = .none` draw this replaces. Destination
    /// coordinates count down from the top row.
    ///
    /// Source pixels are sampled at destination pixel centres, which is the
    /// rule a box filter degenerates to at zero support. It is not promised to
    /// match CoreGraphics bit for bit; the sheets that ship are copied at 1:1
    /// by `cropped`, and only the unreachable pose-derived fallback scales.
    public mutating func blit(
        _ source: PetImage,
        toX destinationX: Int,
        toY destinationY: Int,
        width destinationWidth: Int,
        height destinationHeight: Int,
        mirrored: Bool = false
    ) {
        guard destinationWidth > 0, destinationHeight > 0, !source.isEmpty else { return }
        let sourceStride = source.width * 4
        let canvasStride = width * 4
        source.pixels.withUnsafeBufferPointer { src in
            pixels.withUnsafeMutableBufferPointer { dst in
                for row in 0..<destinationHeight {
                    let canvasY = destinationY + row
                    guard canvasY >= 0, canvasY < height else { continue }
                    let sourceY = min(
                        source.height - 1,
                        Int((Double(row) + 0.5) * Double(source.height) / Double(destinationHeight))
                    )
                    for column in 0..<destinationWidth {
                        let canvasX = destinationX + column
                        guard canvasX >= 0, canvasX < width else { continue }
                        let sampled = mirrored ? destinationWidth - 1 - column : column
                        let sourceX = min(
                            source.width - 1,
                            Int((Double(sampled) + 0.5) * Double(source.width) / Double(destinationWidth))
                        )
                        let from = sourceY * sourceStride + sourceX * 4
                        let alpha = src[from + 3]
                        guard alpha != 0 else { continue }
                        let to = canvasY * canvasStride + canvasX * 4
                        if alpha == 255 {
                            dst[to] = src[from]
                            dst[to + 1] = src[from + 1]
                            dst[to + 2] = src[from + 2]
                            dst[to + 3] = 255
                        } else {
                            // Source-over on premultiplied bytes.
                            let inverse = 255 - Int(alpha)
                            for channel in 0..<4 {
                                let under = Int(dst[to + channel]) * inverse / 255
                                dst[to + channel] = UInt8(min(255, Int(src[from + channel]) + under))
                            }
                        }
                    }
                }
            }
        }
    }

    public func image() -> PetImage {
        PetImage(width: width, height: height, pixels: pixels)
    }
}
