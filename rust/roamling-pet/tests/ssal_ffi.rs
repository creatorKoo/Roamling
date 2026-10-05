// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

use roamling_core::{ffi, pet_image::ssal};
use roamling_pet::package;

#[test]
fn swift_ssal_ffi_matches_windows_for_every_approved_colour() {
    let standard = include_bytes!("../../../Sources/RoamlingPet/Resources/BuiltInPets/ssal-standard-atlas.webp");
    let extension = include_bytes!("../../../Sources/RoamlingPet/Resources/BuiltInPets/ssal-extension-atlas.webp");
    let source = ffi::decode_ssal_palette_sheets(standard.to_vec(), extension.to_vec()).unwrap();
    let windows = package::SsalPalette::built_in().unwrap();
    let mut asset = package::built_in_ssal().unwrap().asset;
    assert_eq!(ffi::ssal_palette_presets().len(), ssal::PRESETS.len());
    assert_eq!(roamling_core::Palette::from(ffi::built_in_ssal_palette()), ssal::DEFAULT);
    for (preset, (key, palette)) in ffi::ssal_palette_presets().iter().zip(ssal::PRESETS) {
        assert_eq!(preset.key, *key);
        assert_eq!(roamling_core::Palette::from(preset.palette), *palette);
        assert!(windows.apply(&mut asset, *palette));
        let swift = source.recolored(preset.palette);
        assert!(swift.standard.pixels == asset.atlas.pixels, "{key} standard");
        assert!(swift.extension.pixels == asset.extension_atlas.as_ref().unwrap().pixels, "{key} extension");
        assert_eq!(swift.standard.width as usize, asset.atlas.width);
        assert_eq!(swift.extension.height as usize, asset.extension_atlas.as_ref().unwrap().height);
    }
    assert!(ffi::decode_ssal_palette_sheets(vec![], extension.to_vec()).is_none());
}
