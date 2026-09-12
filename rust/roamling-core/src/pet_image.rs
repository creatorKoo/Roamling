// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

//! Sheet bytes in, pixels out.
//!
//! This is here rather than in `roamling-pet` because both shells need it and
//! `roamling-pet` depends on this crate, not the other way round. It decides
//! nothing -- it is a byte transform, which is what makes it portable.

use std::collections::HashMap;

pub type PaletteColour = [u8; 3];

/// The three places a caller wants a pet's colour families moved to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PaletteTargets {
    pub dark: PaletteColour,
    pub orange: PaletteColour,
    pub cream: PaletteColour,
}

impl PaletteTargets {
    pub const fn new(dark: PaletteColour, orange: PaletteColour, cream: PaletteColour) -> Self {
        Self {
            dark,
            orange,
            cream,
        }
    }

    fn colour(self, family: Family) -> PaletteColour {
        match family {
            Family::Dark => self.dark,
            Family::Orange => self.orange,
            Family::Cream => self.cream,
        }
    }
}

/// What the three colour clouds in one or more straight-alpha sheets measure.
///
/// A family can be absent in somebody else's sheet, so the general-purpose
/// measurement keeps that fact rather than inventing an anchor for it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PaletteAnchors {
    pub dark: Option<PaletteColour>,
    pub orange: Option<PaletteColour>,
    pub cream: Option<PaletteColour>,
}

impl PaletteAnchors {
    /// Turn a complete measurement into slider targets at the identity point.
    pub fn targets(self) -> Option<PaletteTargets> {
        Some(PaletteTargets::new(self.dark?, self.orange?, self.cream?))
    }

    fn colour(self, family: Family) -> Option<PaletteColour> {
        match family {
            Family::Dark => self.dark,
            Family::Orange => self.orange,
            Family::Cream => self.cream,
        }
    }
}

/// An encoded sheet decoded to the straight alpha the palette algorithm needs.
///
/// This type cannot be rendered by either shell. Turning it into `PetImage`
/// always performs the premultiplication that their drawing contracts require.
#[derive(Debug, Clone)]
pub struct PetImageSource {
    width: usize,
    height: usize,
    pixels: Vec<u8>,
}

impl PetImageSource {
    pub fn decode(bytes: &[u8]) -> Option<Self> {
        let decoded = image::load_from_memory(bytes).ok()?.to_rgba8();
        let (width, height) = (decoded.width() as usize, decoded.height() as usize);
        Some(Self {
            width,
            height,
            pixels: decoded.into_raw(),
        })
    }

    pub fn width(&self) -> usize {
        self.width
    }

    pub fn height(&self) -> usize {
        self.height
    }

    /// Measure one set of anchors over every supplied sheet.
    ///
    /// Standard and extension atlases must be passed together. Measuring them
    /// separately would let the same pet change colour when animation crosses
    /// from one sheet to the other.
    pub fn palette_anchors(sources: &[&Self]) -> PaletteAnchors {
        let mut census: [FamilyCensus; 3] = std::array::from_fn(|_| FamilyCensus::default());
        for source in sources {
            for pixel in source.pixels.chunks_exact(4) {
                // `unify_palette.py` excludes translucent antialiasing from
                // the census. The pixels still get translated below.
                if pixel[3] <= 200 {
                    continue;
                }
                let colour = [pixel[0], pixel[1], pixel[2]];
                if let Some(family) = family_of(colour) {
                    census[family.index()].add(colour);
                }
            }
        }
        PaletteAnchors {
            dark: census[Family::Dark.index()].anchor(),
            orange: census[Family::Orange.index()].anchor(),
            cream: census[Family::Cream.index()].anchor(),
        }
    }

    /// Assign every visible pixel to the nearest RGB anchor once.
    ///
    /// Targets move while the debug sliders are dragged, but anchors and these
    /// assignments do not. Keeping the distance pass out of that hot path is
    /// what makes the pet change on screen as the slider moves.
    pub fn palette_map(&self, anchors: PaletteAnchors) -> PaletteMap {
        let mut candidates = vec![([0, 0, 0], FAMILY_BLACK)];
        for family in Family::ALL {
            if let Some(anchor) = anchors.colour(family) {
                candidates.push((anchor, family.code()));
            }
        }

        let mut assignments = Vec::with_capacity(self.width * self.height);
        for pixel in self.pixels.chunks_exact(4) {
            if pixel[3] == 0 {
                assignments.push(FAMILY_BLACK);
                continue;
            }
            let colour = [pixel[0], pixel[1], pixel[2]];
            let mut nearest = candidates[0];
            let mut nearest_distance = distance_squared(colour, nearest.0);
            // Strictly less preserves Python's `min` tie behaviour: black is
            // first, followed by dark, orange and cream.
            for candidate in candidates.iter().copied().skip(1) {
                let distance = distance_squared(colour, candidate.0);
                if distance < nearest_distance {
                    nearest = candidate;
                    nearest_distance = distance;
                }
            }
            assignments.push(nearest.1);
        }
        PaletteMap { assignments }
    }

    /// Premultiply without moving any colour.
    pub fn image(&self) -> PetImage {
        let mut pixels = self.pixels.clone();
        premultiply(&mut pixels);
        PetImage {
            width: self.width,
            height: self.height,
            pixels,
        }
    }

    /// Translate the three families in straight alpha, then premultiply.
    pub fn recolored(
        &self,
        map: &PaletteMap,
        anchors: PaletteAnchors,
        targets: PaletteTargets,
    ) -> Option<PetImage> {
        if map.assignments.len() != self.width * self.height {
            return None;
        }
        let deltas = palette_deltas(anchors, targets);
        if deltas.iter().flatten().all(|delta| *delta == 0) {
            return Some(self.image());
        }
        // Translate and premultiply in the same pass. Apart from being cheaper
        // while a slider is moving, this ordering makes it impossible to apply
        // a straight-alpha delta to an already-premultiplied edge by accident.
        let mut pixels = self.pixels.clone();
        for (pixel, family) in pixels
            .chunks_exact_mut(4)
            .zip(map.assignments.iter().copied())
        {
            let alpha = u32::from(pixel[3]);
            for channel in 0..3 {
                let straight = if pixel[3] == 0 {
                    pixel[channel]
                } else {
                    (i16::from(pixel[channel]) + deltas[family as usize][channel]).clamp(0, 255)
                        as u8
                };
                pixel[channel] = premultiplied_channel(straight, alpha);
            }
        }
        Some(PetImage {
            width: self.width,
            height: self.height,
            pixels,
        })
    }
}

/// The nearest-family decision for one straight-alpha sheet.
#[derive(Debug, Clone)]
pub struct PaletteMap {
    assignments: Vec<u8>,
}

/// RGBA8, **premultiplied** alpha, row-major with the top row first and no
/// padding between rows -- byte for byte the contract Swift's `PetImage`
/// states. `image` decodes to straight alpha, so the multiply below is not
/// cosmetic: skip it and every soft edge on the sheet renders as a halo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PetImage {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<u8>,
}

impl PetImage {
    pub fn decode(bytes: &[u8]) -> Option<Self> {
        Some(PetImageSource::decode(bytes)?.image())
    }
}

const FAMILY_BLACK: u8 = 0;

#[derive(Debug, Clone, Copy)]
enum Family {
    Dark,
    Orange,
    Cream,
}

impl Family {
    const ALL: [Self; 3] = [Self::Dark, Self::Orange, Self::Cream];

    const fn index(self) -> usize {
        match self {
            Self::Dark => 0,
            Self::Orange => 1,
            Self::Cream => 2,
        }
    }

    const fn code(self) -> u8 {
        self.index() as u8 + 1
    }
}

/// HLS lightness is `(max + min) / 2`; hue and saturation are irrelevant.
fn family_of(colour: PaletteColour) -> Option<Family> {
    let low = *colour.iter().min()? as f64;
    let high = *colour.iter().max()? as f64;
    let lightness = (low + high) * 100.0 / 510.0;
    match lightness {
        value if (10.0..40.0).contains(&value) => Some(Family::Dark),
        value if (42.0..58.0).contains(&value) => Some(Family::Orange),
        value if (82.0..100.1).contains(&value) => Some(Family::Cream),
        _ => None,
    }
}

fn distance_squared(left: PaletteColour, right: PaletteColour) -> u32 {
    (0..3)
        .map(|channel| {
            let difference = i32::from(left[channel]) - i32::from(right[channel]);
            (difference * difference) as u32
        })
        .sum()
}

#[derive(Default)]
struct FamilyCensus {
    colours: HashMap<PaletteColour, u64>,
    buckets: HashMap<PaletteColour, u64>,
    bucket_order: Vec<PaletteColour>,
}

impl FamilyCensus {
    fn add(&mut self, colour: PaletteColour) {
        *self.colours.entry(colour).or_default() += 1;
        let bucket = colour.map(|channel| channel / 8);
        if !self.buckets.contains_key(&bucket) {
            self.bucket_order.push(bucket);
        }
        *self.buckets.entry(bucket).or_default() += 1;
    }

    fn anchor(&self) -> Option<PaletteColour> {
        let mut winner = None;
        let mut population = 0;
        for bucket in &self.bucket_order {
            let count = self.buckets[bucket];
            // Python's Counter keeps the first-seen bucket on a tie.
            if count > population {
                winner = Some(*bucket);
                population = count;
            }
        }
        let winner = winner?;
        let mut total = 0_u64;
        let mut sums = [0_u64; 3];
        for (colour, count) in &self.colours {
            if colour.map(|channel| channel / 8) != winner {
                continue;
            }
            total += count;
            for channel in 0..3 {
                sums[channel] += u64::from(colour[channel]) * count;
            }
        }
        Some(sums.map(|sum| divide_round_ties_even(sum, total) as u8))
    }
}

/// Python's `round`, including its ties-to-even rule.
fn divide_round_ties_even(numerator: u64, denominator: u64) -> u64 {
    let whole = numerator / denominator;
    let remainder = numerator % denominator;
    match remainder.saturating_mul(2).cmp(&denominator) {
        std::cmp::Ordering::Less => whole,
        std::cmp::Ordering::Greater => whole + 1,
        std::cmp::Ordering::Equal if whole % 2 == 0 => whole,
        std::cmp::Ordering::Equal => whole + 1,
    }
}

#[cfg(test)]
fn translated_pixels(
    source: &[u8],
    assignments: &[u8],
    anchors: PaletteAnchors,
    targets: PaletteTargets,
) -> Vec<u8> {
    let deltas = palette_deltas(anchors, targets);
    // This explicit identity path is a contract. Even a mathematically neutral
    // colour conversion here could leak a rounding difference into frame bytes.
    if deltas.iter().flatten().all(|delta| *delta == 0) {
        return source.to_vec();
    }

    let mut output = source.to_vec();
    for (pixel, family) in output.chunks_exact_mut(4).zip(assignments.iter().copied()) {
        // Preserve a fully transparent straight-alpha pixel through the
        // translation. Premultiplication later will canonicalise its hidden RGB.
        if pixel[3] == 0 {
            continue;
        }
        let delta = deltas[family as usize];
        for channel in 0..3 {
            pixel[channel] = (i16::from(pixel[channel]) + delta[channel]).clamp(0, 255) as u8;
        }
    }
    output
}

fn palette_deltas(anchors: PaletteAnchors, targets: PaletteTargets) -> [[i16; 3]; 4] {
    let mut deltas = [[0_i16; 3]; 4];
    for family in Family::ALL {
        let Some(anchor) = anchors.colour(family) else {
            continue;
        };
        let target = targets.colour(family);
        for channel in 0..3 {
            deltas[family.code() as usize][channel] =
                i16::from(target[channel]) - i16::from(anchor[channel]);
        }
    }
    deltas
}

fn premultiply(pixels: &mut [u8]) {
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
        pixel[0] = premultiplied_channel(pixel[0], alpha);
        pixel[1] = premultiplied_channel(pixel[1], alpha);
        pixel[2] = premultiplied_channel(pixel[2], alpha);
    }
}

fn premultiplied_channel(straight: u8, alpha: u32) -> u8 {
    ((u32::from(straight) * alpha + 127) / 255) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source(pixels: &[[u8; 4]]) -> PetImageSource {
        PetImageSource {
            width: pixels.len(),
            height: 1,
            pixels: pixels.iter().flatten().copied().collect(),
        }
    }

    fn complete_source(extra: &[[u8; 4]]) -> PetImageSource {
        let mut pixels = vec![[80, 40, 20, 255], [220, 120, 40, 255], [250, 235, 220, 255]];
        pixels.extend_from_slice(extra);
        source(&pixels)
    }

    #[test]
    fn anchors_find_the_largest_bucket_then_average_inside_it() {
        let mut census = FamilyCensus::default();
        for colour in [[80, 40, 16], [80, 40, 16], [84, 44, 20], [120, 64, 32]] {
            census.add(colour);
        }
        assert_eq!(census.anchor(), Some([81, 41, 17]));
    }

    #[test]
    fn anchor_rounding_matches_python_ties_to_even() {
        assert_eq!(divide_round_ties_even(20, 8), 2);
        assert_eq!(divide_round_ties_even(28, 8), 4);
        assert_eq!(divide_round_ties_even(21, 8), 3);
    }

    #[test]
    fn translucent_pixels_do_not_vote_in_the_census() {
        let image = complete_source(&[[87, 47, 23, 200], [87, 47, 23, 200], [87, 47, 23, 200]]);
        let anchors = PetImageSource::palette_anchors(&[&image]);
        assert_eq!(anchors.dark, Some([80, 40, 20]));
    }

    #[test]
    fn the_hls_lightness_bands_keep_their_deliberate_gaps() {
        assert!(family_of([50, 0, 0]).is_none()); // below 10%
        assert!(matches!(family_of([51, 0, 0]), Some(Family::Dark)));
        assert!(family_of([204, 0, 0]).is_none()); // 40%, before orange
        assert!(matches!(family_of([215, 0, 0]), Some(Family::Orange)));
        assert!(family_of([255, 41, 41]).is_none()); // above 58%, before cream
        assert!(matches!(family_of([255, 164, 164]), Some(Family::Cream)));
        assert!(matches!(family_of([255, 255, 255]), Some(Family::Cream)));
    }

    #[test]
    fn a_zero_delta_is_byte_identical() {
        let image = complete_source(&[[72, 36, 18, 128], [20, 10, 5, 255], [7, 9, 11, 0]]);
        let anchors = PetImageSource::palette_anchors(&[&image]);
        let targets = anchors.targets().expect("all families");
        let map = image.palette_map(anchors);
        let translated = translated_pixels(&image.pixels, &map.assignments, anchors, targets);
        assert_eq!(translated, image.pixels);
        assert_eq!(image.recolored(&map, anchors, targets), Some(image.image()));
    }

    #[test]
    fn recoloring_never_changes_alpha() {
        let image = complete_source(&[
            [70, 35, 15, 201],
            [210, 115, 35, 128],
            [245, 230, 215, 1],
            [7, 9, 11, 0],
        ]);
        let anchors = PetImageSource::palette_anchors(&[&image]);
        let map = image.palette_map(anchors);
        let recolored = image
            .recolored(
                &map,
                anchors,
                PaletteTargets::new([160, 80, 40], [40, 180, 120], [210, 230, 255]),
            )
            .expect("matching map");
        let before: Vec<u8> = image.pixels.chunks_exact(4).map(|pixel| pixel[3]).collect();
        let after: Vec<u8> = recolored
            .pixels
            .chunks_exact(4)
            .map(|pixel| pixel[3])
            .collect();
        assert_eq!(after, before);
    }

    #[test]
    fn a_fully_transparent_pixel_passes_through_translation() {
        let image = complete_source(&[[7, 9, 11, 0]]);
        let anchors = PetImageSource::palette_anchors(&[&image]);
        let map = image.palette_map(anchors);
        let translated = translated_pixels(
            &image.pixels,
            &map.assignments,
            anchors,
            PaletteTargets::new([255, 0, 255], [0, 255, 0], [0, 0, 255]),
        );
        assert_eq!(&translated[12..16], &[7, 9, 11, 0]);
    }

    #[test]
    fn translation_happens_before_premultiplication() {
        let image = complete_source(&[[70, 35, 15, 128]]);
        let anchors = PetImageSource::palette_anchors(&[&image]);
        let unchanged = anchors.targets().expect("all families");
        let map = image.palette_map(anchors);
        let recolored = image
            .recolored(
                &map,
                anchors,
                PaletteTargets::new([160, 80, 40], unchanged.orange, unchanged.cream),
            )
            .expect("matching map");
        assert_eq!(&recolored.pixels[12..16], &[75, 38, 18, 128]);
    }

    #[test]
    fn black_is_a_zero_delta_distance_candidate() {
        let image = complete_source(&[[20, 10, 5, 255]]);
        let anchors = PetImageSource::palette_anchors(&[&image]);
        let map = image.palette_map(anchors);
        let translated = translated_pixels(
            &image.pixels,
            &map.assignments,
            anchors,
            PaletteTargets::new([255, 0, 255], [0, 255, 0], [0, 0, 255]),
        );
        assert_eq!(&translated[12..16], &[20, 10, 5, 255]);
    }
}
