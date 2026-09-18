// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

use super::*;

// ------------------------------------------------------------------ palette
//
// Windows links this crate's sibling directly and needs none of this; macOS
// reaches everything through uniffi, so the palette has to cross here. What
// crosses is deliberately small: the preset table, the two conversions a
// picker needs, and "these sheets, recoloured".
//
// The recoloured sheets come back as images rather than as a finished pet.
// macOS assembles the built-in mascot in Swift (`MascotPetFactory`), and the
// Rust assembler is a port that is not finished; handing the whole asset over
// would drag that across the boundary for nothing. This is the seam W2b
// already established for decoding.

#[derive(uniffi::Record, Clone, Copy)]
pub struct FfiPaletteTargets {
    pub hue: f32,
    pub hue_end: f32,
    pub light_low: f32,
    pub light_high: f32,
    pub chroma: f32,
}

impl From<crate::pet_image::PaletteTargets> for FfiPaletteTargets {
    fn from(value: crate::pet_image::PaletteTargets) -> Self {
        Self {
            hue: value.hue,
            hue_end: value.hue_end,
            light_low: value.light_low,
            light_high: value.light_high,
            chroma: value.chroma,
        }
    }
}

impl From<FfiPaletteTargets> for crate::pet_image::PaletteTargets {
    fn from(value: FfiPaletteTargets) -> Self {
        Self {
            hue: value.hue,
            hue_end: value.hue_end,
            light_low: value.light_low,
            light_high: value.light_high,
            chroma: value.chroma,
        }
    }
}

#[derive(uniffi::Record, Clone, Copy)]
pub struct FfiPalette {
    pub marking: FfiPaletteTargets,
    pub body: FfiPaletteTargets,
    pub eye: FfiPaletteTargets,
}

impl From<crate::pet_image::Palette> for FfiPalette {
    fn from(value: crate::pet_image::Palette) -> Self {
        Self {
            marking: value.marking.into(),
            body: value.body.into(),
            eye: value.eye.into(),
        }
    }
}

impl From<FfiPalette> for crate::pet_image::Palette {
    fn from(value: FfiPalette) -> Self {
        Self {
            marking: value.marking.into(),
            body: value.body.into(),
            eye: value.eye.into(),
        }
    }
}

/// A menu row: the string key to show and the colours it means.
#[derive(uniffi::Record)]
pub struct FfiPalettePreset {
    pub key: String,
    pub palette: FfiPalette,
}

#[derive(uniffi::Record, Clone, Copy)]
pub struct FfiColour {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
}

/// The colours the menu offers, in menu order. Read from the one table.
#[uniffi::export]
pub fn palette_presets() -> Vec<FfiPalettePreset> {
    crate::pet_image::palette_presets()
        .iter()
        .map(|(key, palette)| FfiPalettePreset {
            key: (*key).to_string(),
            palette: (*palette).into(),
        })
        .collect()
}

/// Where the shipped sheet already is. Selecting it restores the exact bytes.
#[uniffi::export]
pub fn built_in_palette() -> FfiPalette {
    crate::pet_image::BUILT_IN_PALETTE.into()
}

/// Point one region's ramp at a picked colour, keeping its shading width.
///
/// The shell must not do this arithmetic itself. A colour picker gives one
/// colour and a region is five numbers; which of the five it becomes is a
/// product decision that is already made and measured here.
#[uniffi::export]
pub fn palette_aimed_at(targets: FfiPaletteTargets, colour: FfiColour) -> FfiPaletteTargets {
    let targets: crate::pet_image::PaletteTargets = targets.into();
    targets
        .aimed_at([colour.red, colour.green, colour.blue])
        .into()
}

/// The one colour a ramp reads as, for the swatch beside its menu row.
///
/// Built from the same arithmetic the recolour uses, or the swatch shows a
/// colour the cat is not wearing.
#[uniffi::export]
pub fn palette_middle(targets: FfiPaletteTargets) -> FfiColour {
    let targets: crate::pet_image::PaletteTargets = targets.into();
    let [red, green, blue] = targets.middle();
    FfiColour { red, green, blue }
}

#[derive(uniffi::Record)]
pub struct FfiRecoloredSheets {
    pub standard: FfiPetImage,
    pub extension: FfiPetImage,
}

/// The decoded sheets and their region maps, held so a live colour session
/// does not pay for them again on every change.
///
/// Deciding which pixels are fur and which are eye is the expensive half and it
/// does not depend on the palette, so it happens once here.
#[derive(uniffi::Object)]
pub struct PaletteSheets {
    standard: crate::pet_image::PetImageSource,
    extension: crate::pet_image::PetImageSource,
    standard_map: crate::pet_image::PaletteMap,
    extension_map: crate::pet_image::PaletteMap,
}

/// Decode both built-in sheets and map their regions.
///
/// The bytes come from the caller because the shell already has them: on macOS
/// they are resources in the app bundle, and compiling a second copy into this
/// library to avoid one argument would add two megabytes for nothing.
#[uniffi::export]
pub fn decode_palette_sheets(
    standard: Vec<u8>,
    extension: Vec<u8>,
    cell_width: u32,
    cell_height: u32,
) -> Option<std::sync::Arc<PaletteSheets>> {
    let cell = (cell_width as usize, cell_height as usize);
    if cell.0 == 0 || cell.1 == 0 {
        return None;
    }
    let standard = crate::pet_image::PetImageSource::decode(&standard)?;
    let extension = crate::pet_image::PetImageSource::decode(&extension)?;
    let standard_map = standard.region_map(cell);
    let extension_map = extension.region_map(cell);
    Some(std::sync::Arc::new(PaletteSheets {
        standard,
        extension,
        standard_map,
        extension_map,
    }))
}

#[uniffi::export]
impl PaletteSheets {
    /// Both sheets at the given colours, premultiplied and ready to draw.
    ///
    /// `None` only when a map does not match its sheet, which cannot happen
    /// for sheets this object decoded itself.
    pub fn recolored(&self, palette: FfiPalette) -> Option<FfiRecoloredSheets> {
        let identity = crate::pet_image::BUILT_IN_PALETTE;
        let targets: crate::pet_image::Palette = palette.into();
        let standard = self
            .standard
            .recolored(&self.standard_map, identity, targets)?;
        let extension = self
            .extension
            .recolored(&self.extension_map, identity, targets)?;
        Some(FfiRecoloredSheets {
            standard: FfiPetImage {
                width: standard.width as u32,
                height: standard.height as u32,
                pixels: standard.pixels,
            },
            extension: FfiPetImage {
                width: extension.width as u32,
                height: extension.height as u32,
                pixels: extension.pixels,
            },
        })
    }
}

/// The settings line for a palette, written by whichever shell is running.
#[uniffi::export]
pub fn palette_to_text(palette: FfiPalette) -> String {
    crate::pet_image::palette_to_text(palette.into())
}

/// Reads back what `palette_to_text` wrote. `None` for anything else.
#[uniffi::export]
pub fn palette_from_text(text: String) -> Option<FfiPalette> {
    crate::pet_image::palette_from_text(&text).map(Into::into)
}
