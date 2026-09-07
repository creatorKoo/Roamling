// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

import Foundation
import RoamlingCoreRs
import RoamlingPet

/// Sheet bytes to pixels, through the same decoder Windows uses.
///
/// Decoding was a platform capability, and the reasoning was sound at the time:
/// macOS answers WebP and PNG through ImageIO for nothing, and no other
/// platform does. What it cost was two decoders, and two decoders disagreed --
/// the Rust one truncated where CoreGraphics rounds, so every soft edge came
/// out a channel darker there. One implementation cannot drift from itself.
///
/// `PetImageSourcing` stays, because `placeholderAtlas` is still a platform
/// drawing. This is only the half that was arithmetic all along.
public enum RustImageDecoder {
    public static func decode(contentsOf url: URL) -> PetImage? {
        guard let data = try? Data(contentsOf: url) else { return nil }
        return decode(data)
    }

    public static func decode(_ data: Data) -> PetImage? {
        guard let decoded = decodePetImage(bytes: data) else { return nil }
        return PetImage(
            width: Int(decoded.width),
            height: Int(decoded.height),
            pixels: [UInt8](decoded.pixels)
        )
    }
}
