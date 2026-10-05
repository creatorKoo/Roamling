// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

//! Review one saved Ssal palette through the app's existing live recolour path.
use roamling_core::pet_image::{palette_from_text, palette_to_text, ssal::WHITE};
use roamling_pet::package::{load, SsalPalette};
use std::path::PathBuf;

fn main() {
    let mut args = std::env::args().skip(1);
    let package = PathBuf::from(args.next().expect("package path"));
    let palette_path = PathBuf::from(args.next().expect("palette text path"));
    let out = PathBuf::from(args.next().expect("output path"));
    let palette = palette_from_text(std::fs::read_to_string(palette_path).unwrap().trim())
        .expect("valid saved palette");
    std::fs::create_dir_all(&out).unwrap();
    let mut loaded = load(&package).unwrap();
    assert!(loaded.warnings.is_empty());
    let source = SsalPalette::load(&package).unwrap();
    let original = loaded.asset.atlas.pixels.clone();
    let extension = loaded
        .asset
        .extension_atlas
        .as_ref()
        .unwrap()
        .pixels
        .clone();
    let tracks = format!("{:?}", loaded.asset.tracks);
    assert!(source.apply(&mut loaded.asset, palette));
    for (name, image, before) in [
        ("standard", &loaded.asset.atlas, &original),
        (
            "extension",
            loaded.asset.extension_atlas.as_ref().unwrap(),
            &extension,
        ),
    ] {
        assert_ne!(&image.pixels, before);
        for (old, new) in before.chunks_exact(4).zip(image.pixels.chunks_exact(4)) {
            assert_eq!(old[3], new[3]);
            if i16::from(old[0]) - i16::from(old[1]) > 15 && old[0] > old[2] {
                assert_eq!(old, new, "pink accent changed");
            }
        }
        image::save_buffer(
            out.join(format!("cream-{name}.png")),
            &image.pixels,
            image.width as u32,
            image.height as u32,
            image::ColorType::Rgba8,
        )
        .unwrap();
    }
    let first = loaded.asset.atlas.pixels.clone();
    let first_extension = loaded
        .asset
        .extension_atlas
        .as_ref()
        .unwrap()
        .pixels
        .clone();
    source.apply(&mut loaded.asset, WHITE);
    assert_eq!(loaded.asset.atlas.pixels, original);
    assert_eq!(
        loaded.asset.extension_atlas.as_ref().unwrap().pixels,
        extension
    );
    source.apply(&mut loaded.asset, palette);
    assert_eq!(loaded.asset.atlas.pixels, first);
    assert_eq!(
        loaded.asset.extension_atlas.as_ref().unwrap().pixels,
        first_extension
    );
    assert_eq!(format!("{:?}", loaded.asset.tracks), tracks);
    std::fs::write(out.join("validated-palette.txt"), palette_to_text(palette)).unwrap();
    println!("PASS: saved palette roundtrip, alpha/pink/timing preserved, exact white reset, repeat identical");
}
