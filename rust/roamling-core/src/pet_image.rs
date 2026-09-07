// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

//! Sheet bytes in, pixels out.
//!
//! This is here rather than in `roamling-pet` because both shells need it and
//! `roamling-pet` depends on this crate, not the other way round. It decides
//! nothing -- it is a byte transform, which is what makes it portable.

/// RGBA8, **premultiplied** alpha, row-major with the top row first and no
/// padding between rows -- byte for byte the contract Swift's `PetImage`
/// states. `image` decodes to straight alpha, so the multiply below is not
/// cosmetic: skip it and every soft edge on the sheet renders as a halo.
pub struct PetImage {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<u8>,
}

impl PetImage {
    pub fn decode(bytes: &[u8]) -> Option<Self> {
        let decoded = image::load_from_memory(bytes).ok()?.to_rgba8();
        let (width, height) = (decoded.width() as usize, decoded.height() as usize);
        let mut pixels = decoded.into_raw();
        for pixel in pixels.chunks_exact_mut(4) {
            let alpha = pixel[3] as u32;
            // `+ 127` before the divide, which is round-to-nearest in integers:
            // a tie cannot occur, because `c * a / 255` is only ever a half when
            // `2 * c * a` is an odd multiple of 255, and 255 is odd.
            //
            // Rounding down was here first, and it was measured against nothing.
            // CoreGraphics rounds, so 47,678 of the standard sheet's pixels came
            // out a channel darker than the same sheet decoded on macOS -- 1.66%
            // of it, every one of them partly transparent, none of them opaque.
            // One count off 255 is not visible, but it made the two platforms
            // disagree about bytes that the frame hashes compare exactly.
            pixel[0] = ((pixel[0] as u32 * alpha + 127) / 255) as u8;
            pixel[1] = ((pixel[1] as u32 * alpha + 127) / 255) as u8;
            pixel[2] = ((pixel[2] as u32 * alpha + 127) / 255) as u8;
        }
        Some(Self {
            width,
            height,
            pixels,
        })
    }
}
