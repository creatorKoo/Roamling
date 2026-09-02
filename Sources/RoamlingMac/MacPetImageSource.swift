// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

import CoreGraphics
import Foundation
import ImageIO
import RoamlingPet

/// macOS's answer to `PetImageSourcing`. ImageIO reads WebP and PNG without
/// anything being vendored, which is why the pet layer asks the platform for
/// pixels instead of decoding them itself -- Windows has no such gift for
/// WebP and will need a real decoder here.
public struct MacPetImageSource: PetImageSourcing {
    public init() {}

    public func decode(contentsOf url: URL) -> PetImage? {
        guard let source = CGImageSourceCreateWithURL(url as CFURL, nil),
              let image = CGImageSourceCreateImageAtIndex(source, 0, nil) else { return nil }
        return Self.pixels(of: image)
    }

    public func placeholderAtlas(
        columns: Int,
        rows: Int,
        cellWidth: Int,
        cellHeight: Int
    ) -> PetImage? {
        MacPlaceholderArt.drawAtlas(
            columns: columns,
            rows: rows,
            cellWidth: cellWidth,
            cellHeight: cellHeight
        ).flatMap(Self.pixels(of:))
    }

    /// Redraws into a known layout rather than trusting whatever the file had:
    /// RGBA8 premultiplied, top row first, no row padding. This is the same
    /// normalization the pet tests already asked for, so the bytes crossing the
    /// boundary are the bytes that used to reach the screen.
    static func pixels(of image: CGImage) -> PetImage? {
        let width = image.width
        let height = image.height
        guard width > 0, height > 0 else { return nil }
        var bytes = [UInt8](repeating: 0, count: width * height * 4)
        let drawn = bytes.withUnsafeMutableBytes { raw -> Bool in
            guard let context = CGContext(
                data: raw.baseAddress,
                width: width,
                height: height,
                bitsPerComponent: 8,
                bytesPerRow: width * 4,
                space: CGColorSpaceCreateDeviceRGB(),
                bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue
            ) else { return false }
            context.draw(image, in: CGRect(x: 0, y: 0, width: width, height: height))
            return true
        }
        guard drawn else { return nil }
        return PetImage(width: width, height: height, pixels: bytes)
    }

    /// The other direction, for handing a frame to AppKit.
    static func cgImage(of image: PetImage) -> CGImage? {
        guard !image.isEmpty else { return nil }
        guard let provider = CGDataProvider(data: Data(image.pixels) as CFData) else { return nil }
        return CGImage(
            width: image.width,
            height: image.height,
            bitsPerComponent: 8,
            bitsPerPixel: 32,
            bytesPerRow: image.width * 4,
            space: CGColorSpaceCreateDeviceRGB(),
            bitmapInfo: CGBitmapInfo(rawValue: CGImageAlphaInfo.premultipliedLast.rawValue),
            provider: provider,
            decode: nil,
            shouldInterpolate: false,
            intent: .defaultIntent
        )
    }
}
