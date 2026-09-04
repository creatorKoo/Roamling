// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

//! Sprite sheets as bytes, and the built-in mascot built out of them.
//!
//! Ported from Swift's `RoamlingPet`. Only the shipped `mochi-v3` path is here
//! -- the older authored sheets and the pose-derived emergency fallback stay in
//! Swift until something needs them. `docs/windows.md`, W4.
//!
//! The decoder is the Rust `image` crate, which is what W2b was waiting on: it
//! gives WebP and PNG together, on every platform, with no C vendored in.

pub mod package;

use roamling_core::{standard_tracks, PetAnimationFrame, PetAnimationTrack};
use std::collections::BTreeMap;

/// A decoded sheet, as bytes rather than a platform image.
///
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
            // Integer, and rounding down, to land on the same bytes CoreGraphics
            // produced before W2 replaced it.
            pixel[0] = (pixel[0] as u32 * alpha / 255) as u8;
            pixel[1] = (pixel[1] as u32 * alpha / 255) as u8;
            pixel[2] = (pixel[2] as u32 * alpha / 255) as u8;
        }
        Some(Self {
            width,
            height,
            pixels,
        })
    }
}

/// Which sheet a frame index lands on.
///
/// Petdex's grid is a nine-row contract with no room for sleeping, being
/// carried or watching the cursor. Those live on a second sheet, addressed by
/// continuing the index past the end of the first -- so a track's frame list
/// never has to say which sheet it means.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sheet {
    Package,
    Extension,
}

/// Where one frame is: which sheet, and the cell's top-left corner.
#[derive(Debug, Clone, Copy)]
pub struct FrameRect {
    pub sheet: Sheet,
    pub x: usize,
    pub y: usize,
    pub width: usize,
    pub height: usize,
}

pub struct PetAsset {
    /// What the menu calls it. `BuiltInPetKind.mochi.displayName` on the Swift
    /// side; a package's manifest supplies it once the catalogue exists.
    pub display_name: String,
    pub atlas: PetImage,
    pub extension_atlas: Option<PetImage>,
    pub frame_width: usize,
    pub frame_height: usize,
    pub columns: usize,
    pub rows: usize,
    pub extension_columns: usize,
    pub extension_rows: usize,
    pub tracks: BTreeMap<String, PetAnimationTrack>,
    pub behavior_mappings: BTreeMap<String, String>,
}

impl PetAsset {
    fn frame_count(&self) -> usize {
        self.columns * self.rows
    }

    /// Ported from `PetAsset.frameImage(at:)`, including its bounds check: a
    /// track that names a cell the sheet does not have draws nothing rather
    /// than reading past the end.
    pub fn frame_rect(&self, index: usize) -> Option<FrameRect> {
        let (sheet, offset, stride, image) = if index < self.frame_count() {
            (Sheet::Package, index, self.columns, &self.atlas)
        } else {
            let extension = self.extension_atlas.as_ref()?;
            (
                Sheet::Extension,
                index - self.frame_count(),
                self.extension_columns,
                extension,
            )
        };
        if stride == 0 {
            return None;
        }
        let x = (offset % stride) * self.frame_width;
        let y = (offset / stride) * self.frame_height;
        if x + self.frame_width > image.width || y + self.frame_height > image.height {
            return None;
        }
        Some(FrameRect {
            sheet,
            x,
            y,
            width: self.frame_width,
            height: self.frame_height,
        })
    }

    pub fn sheet(&self, which: Sheet) -> Option<&PetImage> {
        match which {
            Sheet::Package => Some(&self.atlas),
            Sheet::Extension => self.extension_atlas.as_ref(),
        }
    }
}

const CELL_WIDTH: usize = 192;
const CELL_HEIGHT: usize = 208;
const COLUMNS: usize = 8;
const STANDARD_ROWS: usize = 9;
const EXTENSION_ROWS: usize = 3;

// The shipped `mochi-v3` package, byte for byte the same files as
// `~/.codex/pets/mochi-v3`. Compiled in rather than read from disk: the
// built-in mascot has to exist even when nothing else does.
pub(crate) const STANDARD: &[u8] =
    include_bytes!("../../../Sources/RoamlingPet/Resources/BuiltInPets/mochi-standard-atlas.webp");
pub(crate) const EXTENSION: &[u8] =
    include_bytes!("../../../Sources/RoamlingPet/Resources/BuiltInPets/mochi-extension-atlas.webp");

fn track(name: &str, frames: &[(usize, f64)], loops: bool) -> PetAnimationTrack {
    let mut built = PetAnimationTrack::new(
        name,
        frames
            .iter()
            .map(|(index, duration)| PetAnimationFrame::new(*index, *duration))
            .collect(),
        loops,
    );
    built.fallback = Some("idle".to_string());
    built
}

/// Ported from `MascotPetFactory.makeStandardMochi`.
pub fn built_in_mochi() -> Option<PetAsset> {
    let atlas = PetImage::decode(STANDARD)?;
    if atlas.width != CELL_WIDTH * COLUMNS || atlas.height != CELL_HEIGHT * STANDARD_ROWS {
        return None;
    }

    let mut tracks = standard_tracks(COLUMNS);
    let jump_row = 4 * COLUMNS;

    // A finished turn waves, and this sheet authors that row, so `.celebrate`
    // resolves straight to `waving` with nothing written out here. `jumping` is
    // left alone -- it opens a turn rather than closing one.
    tracks.insert(
        "idle".into(),
        track(
            "idle",
            &[
                (0, 1.20),
                (1, 0.10),
                (2, 0.10),
                (3, 0.10),
                (4, 0.10),
                (5, 0.10),
            ],
            true,
        ),
    );
    // Without this, `landing` falls through to jumping and the pet throws a
    // full celebration every time it is dropped.
    tracks.insert(
        "landing".into(),
        track(
            "landing",
            &[
                (jump_row + 4, 0.10),
                (jump_row + 3, 0.12),
                (jump_row + 2, 0.10),
                (jump_row, 0.18),
            ],
            false,
        ),
    );

    let mut extension_atlas = None;
    let mut extension_columns = 0;
    let mut extension_rows = 0;
    let mut behavior_mappings = BTreeMap::new();

    if let Some(sheet) = PetImage::decode(EXTENSION).filter(|sheet| {
        sheet.width == CELL_WIDTH * COLUMNS && sheet.height == CELL_HEIGHT * EXTENSION_ROWS
    }) {
        extension_atlas = Some(sheet);
        extension_columns = COLUMNS;
        extension_rows = EXTENSION_ROWS;

        // Indices continue past the package grid, so the extension sheet's
        // first cell is 72. `gaze` is the exception: it points back into the
        // package's own review row, and is played faster the closer the pointer
        // gets, so the tail flick doubles as watching.
        let base = COLUMNS * STANDARD_ROWS;
        let range = |start: usize, count: usize, step: f64| -> Vec<(usize, f64)> {
            (0..count).map(|i| (start + i, step)).collect()
        };
        tracks.insert(
            "gaze".into(),
            track("gaze", &range(8 * COLUMNS, 6, 0.172), true),
        );
        tracks.insert(
            "sleeping".into(),
            track("sleeping", &range(base, 3, 0.667), true),
        );
        tracks.insert(
            "caught".into(),
            track("caught", &range(base + 3, 4, 0.150), true),
        );
        tracks.insert(
            "sitting".into(),
            track("sitting", &range(base + COLUMNS, 4, 0.600), false),
        );
        // `wake` and `stretch` are one capability, and the player only restarts
        // a track when the capability changes, so these eight run straight
        // through both states rather than replaying the first half.
        tracks.insert(
            "stretching".into(),
            track("stretching", &range(base + COLUMNS * 2, 8, 0.212), false),
        );

        for (behavior, name) in [
            ("gaze", "gaze"),
            ("sleep", "sleeping"),
            ("caught", "caught"),
            ("sit", "sitting"),
            ("stretch", "stretching"),
        ] {
            behavior_mappings.insert(behavior.to_string(), name.to_string());
        }
    }

    Some(PetAsset {
        display_name: "Mochi".to_string(),
        atlas,
        extension_atlas,
        frame_width: CELL_WIDTH,
        frame_height: CELL_HEIGHT,
        columns: COLUMNS,
        rows: STANDARD_ROWS,
        extension_columns,
        extension_rows,
        tracks,
        behavior_mappings,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The sheets are a contract, not just data: `docs/windows.md` and
    /// `CLAUDE.md` both pin 8 columns by 9 and 3 rows at 192x208.
    #[test]
    fn the_shipped_sheets_are_the_shape_the_tracks_assume() {
        let asset = built_in_mochi().expect("the built-in mascot has to decode");
        assert_eq!(asset.atlas.width, 8 * 192);
        assert_eq!(asset.atlas.height, 9 * 208);
        let extension = asset
            .extension_atlas
            .as_ref()
            .expect("the extension sheet ships too");
        assert_eq!(extension.width, 8 * 192);
        assert_eq!(extension.height, 3 * 208);
    }

    /// Every frame every track names has to land on a cell that exists. A typo
    /// in an index is otherwise invisible until the pet blinks out mid-track.
    #[test]
    fn every_track_frame_lands_on_a_real_cell() {
        let asset = built_in_mochi().expect("decode");
        for (name, track) in &asset.tracks {
            for frame in &track.frames {
                assert!(
                    asset.frame_rect(frame.index).is_some(),
                    "track {name} names frame {} and no cell holds it",
                    frame.index
                );
            }
        }
    }

    /// The index continues past the package grid rather than restarting, which
    /// is the whole reason a track never has to say which sheet it means.
    #[test]
    fn the_extension_sheet_continues_the_index() {
        let asset = built_in_mochi().expect("decode");
        let last = asset.frame_rect(71).expect("last package cell");
        assert_eq!(last.sheet, Sheet::Package);
        let first = asset.frame_rect(72).expect("first extension cell");
        assert_eq!(first.sheet, Sheet::Extension);
        assert_eq!((first.x, first.y), (0, 0));
    }

    /// Premultiplication is easy to leave out and invisible until it is on a
    /// screen, so it gets a check: no channel may exceed its own alpha.
    #[test]
    fn the_decoded_sheet_is_premultiplied() {
        let asset = built_in_mochi().expect("decode");
        for pixel in asset.atlas.pixels.chunks_exact(4) {
            let alpha = pixel[3];
            assert!(
                pixel[0] <= alpha && pixel[1] <= alpha && pixel[2] <= alpha,
                "a channel is brighter than its alpha, so the sheet is straight"
            );
        }
    }
}
