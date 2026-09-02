// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

import CoreGraphics
import Foundation
import ImageIO
import RoamlingPet

/// The harness's own decoder. `RoamlingPet` stopped decoding in W2, so the
/// tests supply what the platform normally would.
///
/// This is the last ImageIO tie in the test target: W2b replaces it with a
/// portable decoder, and until then the harness only builds where ImageIO does.
struct TestPetImageSource: PetImageSourcing {
    func decode(contentsOf url: URL) -> PetImage? {
        guard let source = CGImageSourceCreateWithURL(url as CFURL, nil),
              let image = CGImageSourceCreateImageAtIndex(source, 0, nil) else { return nil }
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

    /// Nil, deliberately: the placeholder's art is a platform drawing and the
    /// harness has no window system. `PlaceholderPetFactory` falls back to a
    /// transparent sheet of the right shape, which is all these tests assert.
    func placeholderAtlas(columns: Int, rows: Int, cellWidth: Int, cellHeight: Int) -> PetImage? {
        nil
    }
}

let testImages = TestPetImageSource()
