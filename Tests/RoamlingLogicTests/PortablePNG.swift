// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

import Foundation

/// Writes an RGBA8 buffer out as a PNG, so a fixture can be made anywhere.
///
/// The harness used ImageIO for this, which was the last thing in the test
/// target that only existed on macOS -- W2b took the decoder but the fixtures
/// still had to be encoded. Writing one is smaller than it sounds: PNG's
/// compression is deflate, deflate has a "stored" block that compresses
/// nothing, and a fixture does not care how large it is.
///
/// Uncompressed also means the bytes are predictable, which is worth something
/// in a file whose whole job is to be read back and compared.
enum PortablePNG {
    /// `pixels` is RGBA8, top row first, `width * 4` bytes per row.
    static func data(width: Int, height: Int, pixels: [UInt8]) -> Data {
        precondition(pixels.count == width * height * 4, "not an RGBA8 buffer of that size")

        var raw: [UInt8] = []
        raw.reserveCapacity(height * (1 + width * 4))
        for row in 0..<height {
            // Filter type 0, "None". Filters exist to help compression, and
            // there is no compression here to help.
            raw.append(0)
            raw.append(contentsOf: pixels[(row * width * 4)..<((row + 1) * width * 4)])
        }

        var png = Data([0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A])
        var header = Data()
        header.append(be32(UInt32(width)))
        header.append(be32(UInt32(height)))
        header.append(contentsOf: [8, 6, 0, 0, 0])  // 8-bit, truecolour with alpha
        png.append(chunk("IHDR", header))
        png.append(chunk("IDAT", zlibStored(raw)))
        png.append(chunk("IEND", Data()))
        return png
    }

    private static func be32(_ value: UInt32) -> Data {
        Data([UInt8(value >> 24 & 0xFF), UInt8(value >> 16 & 0xFF),
              UInt8(value >> 8 & 0xFF), UInt8(value & 0xFF)])
    }

    private static func chunk(_ type: String, _ payload: Data) -> Data {
        var body = Data(type.utf8)
        body.append(payload)
        var out = be32(UInt32(payload.count))
        out.append(body)
        out.append(be32(crc32(body)))
        return out
    }

    /// A zlib stream of stored blocks: deflate is allowed to give up, and 65535
    /// bytes at a time is as much as one such block can carry.
    private static func zlibStored(_ bytes: [UInt8]) -> Data {
        var out = Data([0x78, 0x01])  // deflate, 32K window, no preset dictionary
        var offset = 0
        repeat {
            let count = min(65535, bytes.count - offset)
            let isLast = offset + count >= bytes.count
            out.append(isLast ? 1 : 0)
            out.append(UInt8(count & 0xFF))
            out.append(UInt8(count >> 8 & 0xFF))
            out.append(UInt8(~count & 0xFF))
            out.append(UInt8(~count >> 8 & 0xFF))
            out.append(contentsOf: bytes[offset..<(offset + count)])
            offset += count
        } while offset < bytes.count
        out.append(be32(adler32(bytes)))
        return out
    }

    private static func adler32(_ bytes: [UInt8]) -> UInt32 {
        var a: UInt32 = 1, b: UInt32 = 0
        for byte in bytes {
            a = (a + UInt32(byte)) % 65521
            b = (b + a) % 65521
        }
        return b << 16 | a
    }

    private static let crcTable: [UInt32] = (0..<256).map { index -> UInt32 in
        var value = UInt32(index)
        for _ in 0..<8 {
            value = value & 1 == 1 ? 0xEDB8_8320 ^ (value >> 1) : value >> 1
        }
        return value
    }

    private static func crc32(_ bytes: Data) -> UInt32 {
        var value: UInt32 = 0xFFFF_FFFF
        for byte in bytes {
            value = crcTable[Int((value ^ UInt32(byte)) & 0xFF)] ^ (value >> 8)
        }
        return value ^ 0xFFFF_FFFF
    }
}
