// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

import Foundation
import RoamlingEngine
import RoamlingPet

/// The harness's own decoder. `RoamlingPet` stopped decoding in W2, so the
/// tests supply what the platform normally would.
///
/// It used to be ImageIO, and was the last thing in this target that only
/// built on macOS. W2b made the decoder portable, so this is the same one the
/// app and the Windows shell use.
struct TestPetImageSource: PetImageSourcing {
    func decode(contentsOf url: URL) -> PetImage? {
        RustImageDecoder.decode(contentsOf: url)
    }

    /// Nil, deliberately: the placeholder's art is a platform drawing and the
    /// harness has no window system. `PlaceholderPetFactory` falls back to a
    /// transparent sheet of the right shape, which is all these tests assert.
    func placeholderAtlas(columns: Int, rows: Int, cellWidth: Int, cellHeight: Int) -> PetImage? {
        nil
    }
}

let testImages = TestPetImageSource()
