// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

//! The neutral white Ssal coat has no Bori marking band. Cache its protected
//! accents and reflections once, then use the shared palette controls/colour math.
use super::{from_hls, lightness, premultiply, Palette, PaletteTargets, PetImage, PetImageSource};
use std::collections::VecDeque;
mod eyes;

pub const WHITE: Palette = Palette::new(
    PaletteTargets::new(30.0, 80.0, 100.0, 0.0),
    PaletteTargets::new(0.0, 0.0, 100.0, 0.0),
    PaletteTargets::new(0.0, 0.0, 100.0, 0.0),
);
/// Approved white coat and walnut eyes. WHITE remains the raw-source reset.
pub const DEFAULT: Palette = Palette {
    eye: PaletteTargets::new(28.0, 22.0, 48.0, 64.0),
    ..WHITE
};
pub const BLACK: Palette = Palette {
    marking: PaletteTargets::new(220.0, 2.0, 31.0, 6.0),
    eye: PaletteTargets::new(43.0, 42.0, 75.0, 85.0),
    ..WHITE
};
/// Approved coat/iris pairs, calculated live from the single neutral sheet.
pub const PRESETS: &[(&str, Palette)] = &[
    ("palette.ssal.white", DEFAULT),
    ("palette.ssal.black", BLACK),
    (
        "palette.ssal.cream",
        pair(40.0, 30.0, 92.0, 34.0, 20.0, 9.0, 27.0, 35.0),
    ),
    (
        "palette.ssal.brown",
        pair(28.0, 10.0, 54.0, 36.0, 72.0, 27.0, 54.0, 40.0),
    ),
    (
        "palette.ssal.silver",
        pair(215.0, 18.0, 68.0, 6.0, 210.0, 32.0, 66.0, 65.0),
    ),
    (
        "palette.ssal.gold",
        pair(36.0, 24.0, 74.0, 44.0, 30.0, 25.0, 52.0, 78.0),
    ),
    (
        "palette.ssal.blue",
        pair(215.0, 10.0, 45.0, 5.0, 195.0, 40.0, 70.0, 30.0),
    ),
    (
        "palette.ssal.purple",
        pair(25.0, 7.0, 38.0, 40.0, 50.0, 34.0, 63.0, 65.0),
    ),
    (
        "palette.ssal.hotpink",
        pair(15.0, 12.0, 48.0, 23.0, 16.0, 22.0, 48.0, 58.0),
    ),
];

const fn pair(
    h: f32,
    low: f32,
    high: f32,
    chroma: f32,
    eh: f32,
    el: f32,
    ehi: f32,
    ec: f32,
) -> Palette {
    Palette::new(
        PaletteTargets::new(h, low, high, chroma),
        WHITE.body,
        PaletteTargets::new(eh, el, ehi, ec),
    )
}

pub struct Source {
    source: PetImageSource,
    keep: Vec<bool>,
    iris: Vec<bool>,
}

impl Source {
    pub fn decode(bytes: &[u8]) -> Option<Self> {
        let source = PetImageSource::decode(bytes)?;
        Some(Self::new(source))
    }

    fn new(source: PetImageSource) -> Self {
        let n = source.width * source.height;
        let mut keep = vec![false; n];
        let mut bright = vec![false; n];
        for (i, p) in source.pixels.chunks_exact(4).enumerate() {
            // v9's neutral fur has R-G <=12. The approved pink ramp is separate.
            keep[i] = p[3] == 0 || (i16::from(p[0]) - i16::from(p[1]) > 15 && p[0] > p[2]);
            bright[i] = p[3] > 0 && p[..3].iter().all(|c| *c >= 170);
        }
        // Tiny white islands enclosed by dark eyes/nose are reflections.
        // The connected white coat and anything bordering transparency fail.
        for start in 0..n {
            if !bright[start] {
                continue;
            }
            bright[start] = false;
            let mut queue = VecDeque::from([start]);
            let mut blob = vec![start];
            let mut boundary = Vec::new();
            let mut outside = false;
            while let Some(i) = queue.pop_front() {
                let x = i % source.width;
                let y = i / source.width;
                for (dx, dy) in [(1isize, 0isize), (-1, 0), (0, 1), (0, -1)] {
                    let nx = x as isize + dx;
                    let ny = y as isize + dy;
                    if nx < 0
                        || ny < 0
                        || nx >= source.width as isize
                        || ny >= source.height as isize
                    {
                        outside = true;
                        continue;
                    }
                    let j = ny as usize * source.width + nx as usize;
                    let p = &source.pixels[j * 4..j * 4 + 4];
                    if bright[j] {
                        bright[j] = false;
                        blob.push(j);
                        queue.push_back(j);
                    } else if p[3] == 0 {
                        outside = true;
                    } else if p[..3].iter().any(|c| *c < 170) {
                        boundary.push(j);
                    }
                }
            }
            boundary.sort_unstable();
            boundary.dedup();
            let dark = boundary
                .iter()
                .filter(|j| source.pixels[**j * 4..**j * 4 + 3].iter().all(|c| *c < 155))
                .count();
            if !outside
                && blob.len() <= 45
                && !boundary.is_empty()
                && dark * 5 >= boundary.len() * 4
                && blob.iter().all(|i| in_head(&source, *i))
            {
                for i in blob {
                    keep[i] = true;
                }
            }
        }
        let mut iris = vec![false; n];
        if source.width == 1536 && [1872, 624].contains(&source.height) {
            let offset = if source.height == 624 { 72 } else { 0 };
            for frame in 0..(source.height / 208 * 8) {
                for &[left, top, right, bottom] in eyes::bounds(frame + offset) {
                    for y in top + 1..bottom - 1 {
                        for x in left + 1..right - 1 {
                            let i = (frame / 8 * 208 + y) * source.width + frame % 8 * 192 + x;
                            let p = &source.pixels[i * 4..i * 4 + 4];
                            let value = *p[..3].iter().max().unwrap();
                            // Leave pupils, the outer contour and white glints alone.
                            iris[i] = !keep[i]
                                && (50..170).contains(&value)
                                && [i - 1, i + 1, i - source.width, i + source.width]
                                    .iter()
                                    .all(|j| {
                                        let q = &source.pixels[j * 4..j * 4 + 4];
                                        q[3] > 0 && (keep[*j] || q[..3].iter().all(|c| *c < 170))
                                    });
                        }
                    }
                }
            }
        }
        Self { source, keep, iris }
    }

    pub fn image(&self, palette: Palette) -> PetImage {
        if palette.marking == WHITE.marking && palette.eye == WHITE.eye {
            return self.source.image();
        }
        let aim = palette.marking;
        let mut pixels = self.source.pixels.clone();
        // Always start from straight-alpha white, never from the last output.
        for (i, p) in pixels.chunks_exact_mut(4).enumerate() {
            if self.iris[i] && palette.eye != WHITE.eye {
                let eye = palette.eye;
                let t = ((lightness([p[0], p[1], p[2]]) - 12.0) / 40.0).clamp(0.0, 1.0);
                p[..3].copy_from_slice(&from_hls(
                    eye.hue + (eye.hue_end - eye.hue) * t,
                    eye.light_low + (eye.light_high - eye.light_low) * t,
                    eye.chroma,
                ));
                continue;
            }
            if self.keep[i] || palette.marking == WHITE.marking {
                continue;
            }
            let t = (lightness([p[0], p[1], p[2]]) / 100.0).clamp(0.0, 1.0);
            let rgb = from_hls(
                aim.hue + (aim.hue_end - aim.hue) * t,
                (aim.light_low + (aim.light_high - aim.light_low) * t) * (t / 0.75).min(1.0),
                aim.chroma * t,
            );
            p[..3].copy_from_slice(&rgb);
        }
        premultiply(&mut pixels);
        PetImage {
            width: self.source.width,
            height: self.source.height,
            pixels,
        }
    }
}

/// Authored v9 face bounds, not a generic eye heuristic. A tiny white patch
/// enclosed by a paw outline also passes the ring test, so it must be excluded.
fn in_head(source: &PetImageSource, pixel: usize) -> bool {
    if source.width != 1536 || ![1872, 624].contains(&source.height) {
        return true; // Small synthetic images used by the operator tests.
    }
    let x = pixel % source.width;
    let y = pixel / source.width;
    let frame = y / 208 * 8 + x / 192 + if source.height == 624 { 72 } else { 0 };
    let (x, y) = (x % 192, y % 208);
    let (left, top, right, bottom) = match frame {
        8..=15 => (90, 30, 182, 140),
        16..=23 => (10, 30, 102, 140),
        56..=61 => (20, 48, 120, 147),
        72..=74 | 88 => return false, // Closed sleeping eyes.
        89..=93 => (25, 65, 140, 166),
        _ => (32, 20, 145, 125),
    };
    (left..right).contains(&x) && (top..bottom).contains(&y)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn eye_colour_is_independent_and_preserves_pupils_glints_and_non_eye_pixels() {
        let mut pixels = vec![0; 1536 * 1872 * 4];
        for (left, top) in [(65, 76), (84, 96)] {
            // iris and a similarly coloured nose
            for y in top..top + 7 {
                for x in left..left + 7 {
                    pixels[(y * 1536 + x) * 4..(y * 1536 + x) * 4 + 4]
                        .copy_from_slice(&[85, 74, 68, 255]);
                }
            }
        }
        let pupil = 80 * 1536 + 69;
        let glint = 78 * 1536 + 69;
        pixels[pupil * 4..pupil * 4 + 4].copy_from_slice(&[30, 25, 22, 255]);
        pixels[glint * 4..glint * 4 + 4].copy_from_slice(&[255, 255, 255, 255]);
        let source = Source::new(PetImageSource {
            width: 1536,
            height: 1872,
            pixels,
        });
        assert!(source.iris.iter().any(|i| *i));
        let eye = PaletteTargets::new(205.0, 15.0, 45.0, 75.0);
        for base in [WHITE, BLACK] {
            let before = source.image(base);
            let after = source.image(Palette { eye, ..base });
            assert_ne!(before.pixels, after.pixels);
            for (i, (a, b)) in before
                .pixels
                .chunks_exact(4)
                .zip(after.pixels.chunks_exact(4))
                .enumerate()
            {
                assert_eq!(a[3], b[3]);
                if !source.iris[i] {
                    assert_eq!(a, b);
                }
            }
            assert_eq!(
                &before.pixels[pupil * 4..pupil * 4 + 4],
                &after.pixels[pupil * 4..pupil * 4 + 4]
            );
            assert_eq!(
                &after.pixels[glint * 4..glint * 4 + 4],
                &[255, 255, 255, 255]
            );
        }
        let white_blue = source.image(Palette { eye, ..WHITE });
        let black_blue = source.image(Palette { eye, ..BLACK });
        for (i, is_iris) in source.iris.iter().enumerate() {
            if *is_iris {
                assert_eq!(
                    &white_blue.pixels[i * 4..i * 4 + 4],
                    &black_blue.pixels[i * 4..i * 4 + 4]
                );
            }
        }
        assert_eq!(source.image(WHITE).pixels, source.source.image().pixels);
    }
    #[test]
    fn recolor_preserves_alpha_accents_and_restores_white_without_accumulation() {
        let source = Source::new(PetImageSource {
            width: 4,
            height: 1,
            pixels: vec![
                250, 248, 244, 255, 232, 180, 176, 255, 190, 185, 180, 128, 0, 255, 0, 0,
            ],
        });
        let white = source.image(WHITE);
        let black = source.image(BLACK);
        assert_ne!(black.pixels, white.pixels);
        assert_eq!(&black.pixels[4..8], &white.pixels[4..8]);
        for (a, b) in black
            .pixels
            .chunks_exact(4)
            .zip(white.pixels.chunks_exact(4))
        {
            assert_eq!(a[3], b[3]);
        }
        assert_eq!(white.pixels, source.image(WHITE).pixels);
        assert_eq!(black.pixels, source.image(BLACK).pixels);
    }
    #[test]
    fn enclosed_eye_reflection_stays_white_but_white_fur_changes() {
        let mut pixels = vec![0; 9 * 9 * 4];
        for y in 1..8 {
            for x in 1..8 {
                pixels[(y * 9 + x) * 4..(y * 9 + x) * 4 + 4].copy_from_slice(&[245, 242, 239, 255]);
            }
        }
        for y in 3..6 {
            for x in 3..6 {
                pixels[(y * 9 + x) * 4..(y * 9 + x) * 4 + 4].copy_from_slice(&[40, 33, 29, 255]);
            }
        }
        pixels[(4 * 9 + 4) * 4..(4 * 9 + 4) * 4 + 4].copy_from_slice(&[255, 255, 255, 255]);
        let source = Source::new(PetImageSource {
            width: 9,
            height: 9,
            pixels,
        });
        let black = source.image(BLACK);
        assert_eq!(
            &black.pixels[(4 * 9 + 4) * 4..(4 * 9 + 4) * 4 + 4],
            &[255, 255, 255, 255]
        );
        assert!(black.pixels[(2 * 9 + 2) * 4] < 100);
    }

    #[test]
    fn a_dark_colour_picker_selection_actually_darkens_the_white_coat() {
        let source = Source::new(PetImageSource {
            width: 2,
            height: 1,
            pixels: vec![252, 250, 247, 255, 40, 33, 29, 255],
        });
        let mut chosen = WHITE;
        chosen.marking = WHITE.marking.aimed_at([30, 30, 30]);
        let result = source.image(chosen);
        assert!(result.pixels[0] < 70);
        assert!(result.pixels[4] < result.pixels[0]);
    }

    #[test]
    fn a_white_island_at_the_jump_paw_is_not_an_eye_reflection() {
        let mut source = PetImageSource {
            width: 1536,
            height: 1872,
            pixels: vec![0; 1536 * 1872 * 4],
        };
        let jump_cell = 4 * 208 * 1536 + 192;
        let paw = jump_cell + 170 * 1536 + 60;
        let eye = jump_cell + 105 * 1536 + 82;
        for center in [paw, eye] {
            for dy in -1isize..=1 {
                for dx in -1isize..=1 {
                    let i = (center as isize + dy * 1536 + dx) as usize * 4;
                    source.pixels[i..i + 4].copy_from_slice(&[40, 33, 29, 255]);
                }
            }
            source.pixels[center * 4..center * 4 + 4].copy_from_slice(&[255, 255, 255, 255]);
        }
        let black = Source::new(source).image(BLACK);
        assert!(black.pixels[paw * 4] < 100);
        assert_eq!(&black.pixels[eye * 4..eye * 4 + 4], &[255, 255, 255, 255]);
    }
}
