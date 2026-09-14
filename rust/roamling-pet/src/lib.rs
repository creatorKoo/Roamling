// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

//! Sprite sheets as bytes, and the built-in mascot built out of them.
//!
//! Ported from Swift's `RoamlingPet`. Only the shipped `mochi-v3` path is here
//! -- the older authored sheets and the pose-derived emergency fallback stay in
//! Swift until something needs them. `docs/history/windows.md`, W4.
//!
//! The decoder is the Rust `image` crate, which is what W2b was waiting on: it
//! gives WebP and PNG together, on every platform, with no C vendored in.

pub mod package;

use roamling_core::pet_image::BUILT_IN_PALETTE;
use roamling_core::{
    standard_tracks, Palette, PaletteMap, PetAnimationFrame, PetAnimationTrack,
    PetImageSource,
};
use std::collections::BTreeMap;
use std::sync::OnceLock;

/// A decoded sheet, as bytes rather than a platform image.
///
/// The decoder lives in the core because the macOS shell needs it too, and the
/// core is the crate that shell already links. It is the same type either way.
pub use roamling_core::PetImage;

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
/// Where Mochi already is, measured over every drawn cell of the shipped

// The shipped `mochi-v3` package, byte for byte the same files as
// `~/.codex/pets/mochi-v3`. Compiled in rather than read from disk: the
// built-in mascot has to exist even when nothing else does.
pub(crate) const STANDARD: &[u8] =
    include_bytes!("../../../Sources/RoamlingPet/Resources/BuiltInPets/mochi-standard-atlas.webp");
pub(crate) const EXTENSION: &[u8] =
    include_bytes!("../../../Sources/RoamlingPet/Resources/BuiltInPets/mochi-extension-atlas.webp");

struct BuiltInSource {
    standard: PetImageSource,
    extension: PetImageSource,
    standard_map: PaletteMap,
    extension_map: PaletteMap,
}

impl BuiltInSource {
    fn decode() -> Option<Self> {
        let standard = PetImageSource::decode(STANDARD)?;
        let extension = PetImageSource::decode(EXTENSION)?;
        if standard.width() != CELL_WIDTH * COLUMNS
            || standard.height() != CELL_HEIGHT * STANDARD_ROWS
            || extension.width() != CELL_WIDTH * COLUMNS
            || extension.height() != CELL_HEIGHT * EXTENSION_ROWS
        {
            return None;
        }
        // Which pixels are fur and which are eye, decided once. The eye pass
        // walks cell by cell because it looks for rings of outline, and a ring
        // detector run over the whole atlas would have to separate 57 cats'
        // worth of outline at the same time.
        let cell = (CELL_WIDTH, CELL_HEIGHT);
        let standard_map = standard.region_map(cell);
        let extension_map = extension.region_map(cell);
        Some(Self {
            standard,
            extension,
            standard_map,
            extension_map,
        })
    }

    fn images(&self, targets: Palette) -> Option<(PetImage, PetImage)> {
        std::thread::scope(|scope| {
            let extension = scope.spawn(|| {
                self.extension
                    .recolored(&self.extension_map, BUILT_IN_PALETTE, targets)
            });
            let standard =
                self.standard
                    .recolored(&self.standard_map, BUILT_IN_PALETTE, targets)?;
            Some((standard, extension.join().ok()??))
        })
    }
}

fn built_in_source() -> Option<&'static BuiltInSource> {
    static SOURCE: OnceLock<Option<BuiltInSource>> = OnceLock::new();
    SOURCE.get_or_init(BuiltInSource::decode).as_ref()
}

/// Decode the straight-alpha sheets and cache their nearest-family maps before
/// a live palette session begins.
pub fn prepare_built_in_mochi_recolor() -> bool {
    built_in_source().is_some()
}


/// Name key and palette for each preset, in menu order.
pub fn built_in_mochi_presets() -> &'static [(&'static str, Palette)] {
    roamling_core::pet_image::palette_presets()
}

/// Where marking, body and eyes already are. The debug controls open here, and
/// coming back to it has to restore the original bytes.
pub const fn built_in_mochi_palette() -> Palette {
    BUILT_IN_PALETTE
}

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
    let extension = PetImage::decode(EXTENSION)?;
    if atlas.width != CELL_WIDTH * COLUMNS
        || atlas.height != CELL_HEIGHT * STANDARD_ROWS
        || extension.width != CELL_WIDTH * COLUMNS
        || extension.height != CELL_HEIGHT * EXTENSION_ROWS
    {
        return None;
    }
    Some(built_in_mochi_from_images(atlas, extension))
}

/// Build Mochi at caller-selected family targets. The cached inputs are still
/// straight alpha here; premultiplication only happens inside `recolored`.
pub fn built_in_mochi_recolored(targets: Palette) -> Option<PetAsset> {
    let source = built_in_source()?;
    let (atlas, extension_sheet) = source.images(targets)?;
    Some(built_in_mochi_from_images(atlas, extension_sheet))
}

fn built_in_mochi_from_images(atlas: PetImage, extension_sheet: PetImage) -> PetAsset {
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

    if let Some(sheet) = Some(extension_sheet).filter(|sheet| {
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

    PetAsset {
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
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use roamling_core::PaletteTargets;

    /// The sheets are a contract, not just data: `docs/history/windows.md` and
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

    // There used to be a test here asserting the pinned palette equalled a
    // runtime census of the two sheets. Both the census and the anchors it
    // measured are gone: anchors were the wrong description of what the answer
    // cats do, and measuring one on an answer cat returned lightness 11 for a
    // marking that actually sits at 58 (`docs/history/palette-making.md` §0). The pinned
    // values are now a marking measurement, taken in `output/palette-answers/`
    // with the region masks, and `the_default_palette_is_a_byte_identity`
    // below is what keeps them honest.

    /// The controls open at the measured anchors. Going through the palette
    /// path there must be byte-for-byte the same as the ordinary decoder.
    #[test]
    fn the_default_palette_is_a_byte_identity() {
        let asset = built_in_mochi().expect("the built-in mascot has to decode");
        assert_eq!(asset.atlas, PetImage::decode(STANDARD).expect("standard"));
        assert_eq!(
            asset.extension_atlas.expect("extension"),
            PetImage::decode(EXTENSION).expect("extension")
        );
    }

    /// The region split is the whole argument, so it gets a number to defend.
    ///
    /// Marking and body were measured in `output/palette-answers/region_identity.py`
    /// over the drawn cells of the standard sheet: 242,663 and 228,117.
    ///
    /// The eye figure is 21,051 -- 51 of 57 cells finding a pair at about 400px
    /// each. It is worth a band in both directions. Far below and ring detection
    /// has broken. Far above and the false blobs are back: taking every ring at
    /// face value instead of the best pair put 514px of tail into the landing
    /// animation's last frame and 931px into the jump's, which is how this was
    /// found. Neither failure shows up in a render test.
    ///
    /// It was 30,529 until the growth was stopped from leaving the eye, in two
    /// steps. First the eyelid and the socket shading round the eye, brown like
    /// the iris, which is why lightness had let them in. Then the fur beside the
    /// eye, which three steps of growth could reach by walking round the outside
    /// of the rim. Nobody could see either while eyes stayed brown; a black cat
    /// with gold eyes drew them on the cheek.
    #[test]
    fn the_shipped_sheet_splits_into_the_regions_that_were_measured() {
        let standard = PetImageSource::decode(STANDARD).expect("standard");
        let map = standard.region_map((CELL_WIDTH, CELL_HEIGHT));
        let (marking, body, eye) = (map.marking_pixels(), map.body_pixels(), map.eye_pixels());
        assert!(
            (230_000..250_000).contains(&marking),
            "marking pixels moved a long way from the measurement: {marking}"
        );
        assert!(
            (215_000..240_000).contains(&body),
            "body pixels moved a long way from the measurement: {body}"
        );
        assert!(
            (20_000..22_500).contains(&eye),
            "eye pixels left the measured band: {eye}"
        );
    }

    /// Writes each preset's idle row as raw RGBA so it can be looked at.
    ///
    /// Numbers measured off an answer cat are not a promise that the preset
    /// looks right: the operator sits between them and the screen. The same
    /// reason `ROAMLING_WRITE_TRACE` exists -- some things are only true if you
    /// go and check.
    #[test]
    fn write_presets_when_asked() {
        let Some(directory) = std::env::var_os("ROAMLING_WRITE_PRESETS") else {
            return;
        };
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).expect("preset directory");
        assert!(prepare_built_in_mochi_recolor());
        // Three sizes, because three different questions get asked. The idle
        // row is what the close-ups crop. The whole standard sheet is the only
        // way to see a false eye out on a tail in row four, or to check that a
        // catchlight survives in every frame and not just the one being looked
        // at. The extension sheet carries sleeping, caught, sitting and
        // stretching, and "the outline is thin when the mouse picks him up" is
        // a claim about a row that lives only there.
        for (name, palette) in built_in_mochi_presets() {
            let asset = built_in_mochi_recolored(*palette).expect("recoloured");
            let row = CELL_WIDTH * COLUMNS * 4 * CELL_HEIGHT;
            std::fs::write(
                directory.join(format!("{name}.rgba")),
                &asset.atlas.pixels[..row],
            )
            .expect("write");
            std::fs::write(
                directory.join(format!("full.{name}.rgba")),
                &asset.atlas.pixels,
            )
            .expect("write");
            let extension = asset.extension_atlas.as_ref().expect("extension");
            std::fs::write(directory.join(format!("ext.{name}.rgba")), &extension.pixels)
                .expect("write");
        }
        let source = built_in_source().expect("source");
        let regions = source.standard_map.regions();
        let row = CELL_WIDTH * COLUMNS * CELL_HEIGHT;
        std::fs::write(directory.join("regions.bin"), &regions[..row]).expect("write");
        std::fs::write(directory.join("full.regions.bin"), regions).expect("write");
        std::fs::write(
            directory.join("ext.regions.bin"),
            source.extension_map.regions(),
        )
        .expect("write");
        let shape = |width: usize, pixels: usize| format!("{width} {}", pixels / width);
        std::fs::write(
            directory.join("size.txt"),
            format!("{} {}", CELL_WIDTH * COLUMNS, CELL_HEIGHT),
        )
        .expect("write");
        std::fs::write(
            directory.join("full.size.txt"),
            shape(CELL_WIDTH * COLUMNS, regions.len()),
        )
        .expect("write");
        std::fs::write(
            directory.join("ext.size.txt"),
            shape(CELL_WIDTH * COLUMNS, source.extension_map.regions().len()),
        )
        .expect("write");
    }

    #[test]
    fn recoloring_the_shipped_sheets_preserves_every_alpha_byte() {
        let original = built_in_mochi().expect("original");
        assert!(prepare_built_in_mochi_recolor());
        // Every region moved at once: the alpha plane has to survive all three.
        let recolored = built_in_mochi_recolored(Palette::new(
            PaletteTargets::new(214.0, 30.0, 95.0, 120.0),
            PaletteTargets::new(210.0, 80.0, 96.0, 40.0),
            PaletteTargets::new(140.0, 5.0, 90.0, 120.0),
        ))
        .expect("recolored");
        for (before, after) in [
            (&original.atlas, &recolored.atlas),
            (
                original
                    .extension_atlas
                    .as_ref()
                    .expect("original extension"),
                recolored.extension_atlas.as_ref().expect("new extension"),
            ),
        ] {
            assert_eq!(before.pixels.len(), after.pixels.len());
            for (before, after) in before
                .pixels
                .chunks_exact(4)
                .zip(after.pixels.chunks_exact(4))
            {
                assert_eq!(before[3], after[3]);
            }
        }
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
