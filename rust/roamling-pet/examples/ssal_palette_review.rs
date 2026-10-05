// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

//! Render the actual live palette path, and verify the selected source package.
use roamling_core::pet_image::ssal::{BLACK, WHITE};
use roamling_core::{Palette, PaletteTargets};
use roamling_pet::package::{load, SsalPalette};
use std::path::PathBuf;

fn main() {
    let mut args = std::env::args().skip(1);
    let package = PathBuf::from(args.next().expect("package path"));
    let out = PathBuf::from(args.next().expect("output path"));
    std::fs::create_dir_all(&out).unwrap();
    let mut loaded = load(&package).unwrap();
    assert!(loaded.warnings.is_empty());
    let source = SsalPalette::load(&package).unwrap();
    let original = loaded.asset.atlas.pixels.clone();
    let extra = loaded
        .asset
        .extension_atlas
        .as_ref()
        .unwrap()
        .pixels
        .clone();
    let tracks = loaded.asset.tracks.clone();
    for (label, palette) in [("black", BLACK), ("white", WHITE), ("black-again", BLACK)] {
        let start = std::time::Instant::now();
        assert!(source.apply(&mut loaded.asset, palette));
        println!("{label}: {:.1}ms", start.elapsed().as_secs_f64() * 1000.0);
        for (name, im, before) in [
            ("standard", &loaded.asset.atlas, &original),
            (
                "extension",
                loaded.asset.extension_atlas.as_ref().unwrap(),
                &extra,
            ),
        ] {
            for (old, new) in before.chunks_exact(4).zip(im.pixels.chunks_exact(4)) {
                assert_eq!(old[3], new[3]);
                if i16::from(old[0]) - i16::from(old[1]) > 15 && old[0] > old[2] {
                    assert_eq!(old, new, "pink accent changed");
                }
            }
            if label == "white" {
                assert_eq!(&im.pixels, before);
            }
            if label == "black-again" {
                let first = image::open(out.join(format!("black-{name}.png")))
                    .unwrap()
                    .to_rgba8();
                assert_eq!(&im.pixels, first.as_raw());
            } else {
                image::save_buffer(
                    out.join(format!("{label}-{name}.png")),
                    &im.pixels,
                    im.width as u32,
                    im.height as u32,
                    image::ColorType::Rgba8,
                )
                .unwrap();
            }
        }
        assert_eq!(
            format!("{:?}", loaded.asset.tracks),
            format!("{:?}", tracks)
        );
    }
    println!(
        "PASS: live Rust path; alpha, pink, timing, exact white reset, repeated black identical"
    );
    for (label, base) in [("white", WHITE), ("black", BLACK)] {
        source.apply(&mut loaded.asset, base);
        let standard = loaded.asset.atlas.pixels.clone();
        let extension = loaded
            .asset
            .extension_atlas
            .as_ref()
            .unwrap()
            .pixels
            .clone();
        for (eye_label, eye) in [
            ("amber", PaletteTargets::new(30.0, 15.0, 45.0, 80.0)),
            ("blue", PaletteTargets::new(205.0, 15.0, 45.0, 80.0)),
        ] {
            source.apply(&mut loaded.asset, Palette { eye, ..base });
            for (name, image, before) in [
                ("standard", &loaded.asset.atlas, &standard),
                (
                    "extension",
                    loaded.asset.extension_atlas.as_ref().unwrap(),
                    &extension,
                ),
            ] {
                let mut changed = 0;
                for (a, b) in before.chunks_exact(4).zip(image.pixels.chunks_exact(4)) {
                    assert_eq!(a[3], b[3]);
                    if a != b {
                        changed += 1;
                    }
                    if i16::from(a[0]) - i16::from(a[1]) > 15 && a[0] > a[2] {
                        assert_eq!(a, b, "pink changed with eyes");
                    }
                    if a[..3].iter().all(|c| *c >= 240) {
                        assert_eq!(a, b, "white glint changed");
                    }
                }
                assert!(changed > 0);
                println!("{label}-{eye_label}-{name}: {changed} iris pixels changed");
                image::save_buffer(
                    out.join(format!("{label}-{eye_label}-{name}.png")),
                    &image.pixels,
                    image.width as u32,
                    image.height as u32,
                    image::ColorType::Rgba8,
                )
                .unwrap();
            }
        }
    }
    source.apply(&mut loaded.asset, WHITE);
    assert_eq!(loaded.asset.atlas.pixels, original);
    assert_eq!(loaded.asset.extension_atlas.as_ref().unwrap().pixels, extra);
    println!(
        "PASS: independent eye controls, preserved alpha/accents/glints, full white restoration"
    );
}
