// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

//! Sheet bytes in, pixels out.
//!
//! This is here rather than in `roamling-pet` because both shells need it and
//! `roamling-pet` depends on this crate, not the other way round. It decides
//! nothing -- it is a byte transform, which is what makes it portable.
//!
//! The recolour used to translate three colour families by an RGB delta each.
//! That was measured against thirteen hand-drawn answer cats and it was the
//! wrong operator, not merely badly tuned: given the blue answer's own anchors
//! as targets it still put the dark marking at lightness 11 where the answer
//! has 58, and turned the blush blue. The whole argument is in `docs/palette.md`
//! §0. What the answers actually do:
//!
//! * only the marking moves -- 2,662 of 12,292 opaque pixels in a cell. The
//!   outline, the blush/nose/inner ear and the cream body stay where they are
//!   in eleven of thirteen.
//! * the marking takes **one** hue. The original spreads 17-26 degrees across
//!   its ramp; every answer collapses that to a single value.
//! * lightness is re-laid into a new range rather than stretched, and chroma is
//!   an independent axis -- same hue family, snow reads 9 and lemon 166.
//!
//! So a palette is four numbers, not nine.

use std::collections::VecDeque;

pub type PaletteColour = [u8; 3];

/// Alpha at or below this does not vote and does not get repainted. Carried
/// over from `unify_palette.py`, which excluded translucent antialiasing from
/// its census for the same reason: a half-transparent edge pixel is a blend of
/// two regions and belongs to neither.
const OPAQUE: u8 = 200;

/// Lightness band that holds the fur marking. Everything outside it -- the ink
/// outline below, the blush and the cream body above -- is left alone.
const MARKING_LOW: f32 = 10.0;
const MARKING_HIGH: f32 = 58.0;
/// Below this is the outline. The eye rings are built out of it.
const INK_HIGH: f32 = 10.0;
/// Above this is the cream body, which the eye growth must not walk onto.
const CREAM_LOW: f32 = 82.0;
const CREAM_HIGH: f32 = 100.1;

/// Where a caller wants the marking put.
///
/// Four numbers instead of three colours. `hue` is degrees, `light_low` and
/// `light_high` are the ends of the lightness ramp in percent, `chroma` is
/// max-minus-min in 0-255. HLS saturation is not usable here: the cream body
/// reads 94 while the orange marking reads 85, which is backwards, because
/// saturation degenerates near white.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PaletteTargets {
    pub hue: f32,
    pub light_low: f32,
    pub light_high: f32,
    pub chroma: f32,
}

impl PaletteTargets {
    pub const fn new(hue: f32, light_low: f32, light_high: f32, chroma: f32) -> Self {
        Self {
            hue,
            light_low,
            light_high,
            chroma,
        }
    }
}

/// The three things a cat is made of, each steerable on its own.
///
/// The answer cats treat these as separate axes rather than one. `blue` tinted
/// the body along with the marking and `sky` left it cream; twelve of thirteen
/// kept brown eyes and `sky` alone went blue. So which of them moves is a
/// choice, not a consequence of the fur colour.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Palette {
    pub marking: PaletteTargets,
    pub body: PaletteTargets,
    pub eye: PaletteTargets,
}

impl Palette {
    pub const fn new(marking: PaletteTargets, body: PaletteTargets, eye: PaletteTargets) -> Self {
        Self {
            marking,
            body,
            eye,
        }
    }

    fn targets(self, region: u8) -> Option<PaletteTargets> {
        match region {
            REGION_MARKING => Some(self.marking),
            REGION_BODY => Some(self.body),
            REGION_EYE => Some(self.eye),
            _ => None,
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

    /// Decide once which pixels are marking and which are eye.
    ///
    /// Targets move while a slider is dragged; this does not. Keeping the
    /// region pass out of that hot path is what lets the pet change colour as
    /// the slider moves.
    ///
    /// `cell` is the sprite cell size. Eyes are found per cell rather than per
    /// sheet because a ring detector run over the whole atlas would have to
    /// separate 57 cats' worth of outlines at once.
    pub fn region_map(&self, cell: (usize, usize)) -> PaletteMap {
        let mut regions = vec![REGION_KEEP; self.width * self.height];
        for (index, pixel) in self.pixels.chunks_exact(4).enumerate() {
            if pixel[3] <= OPAQUE {
                continue;
            }
            let colour = [pixel[0], pixel[1], pixel[2]];
            if is_marking(colour) {
                regions[index] = REGION_MARKING;
            } else if is_body(colour) {
                regions[index] = REGION_BODY;
            }
        }
        // Eyes last: they sit inside the marking band by lightness and have to
        // win, which is the whole reason the ring pass exists.
        for (origin_x, origin_y) in self.cells(cell) {
            for spot in self.eyes_in_cell(origin_x, origin_y, cell) {
                regions[spot] = REGION_EYE;
            }
        }
        PaletteMap { regions }
    }

    fn cells(&self, cell: (usize, usize)) -> Vec<(usize, usize)> {
        let (cell_width, cell_height) = cell;
        if cell_width == 0 || cell_height == 0 {
            return vec![(0, 0)];
        }
        let mut found = Vec::new();
        let mut y = 0;
        while y + cell_height <= self.height {
            let mut x = 0;
            while x + cell_width <= self.width {
                found.push((x, y));
                x += cell_width;
            }
            y += cell_height;
        }
        found
    }

    /// Find the eyes in one cell, as rings of ink wrapped round a catchlight.
    ///
    /// Five colour-and-shape rules failed before this one, all of them asking
    /// which piece of the *marking* was an eye. The eye is not made of marking:
    /// it is an ink rim around an iris, and the marking band only ever saw the
    /// filling. A ring has a property nothing else on this cat has -- it
    /// encloses something. Whiskers and the mouth are strokes. The body outline
    /// encloses the whole body, so hole size separates it.
    ///
    /// Measured over the standard sheet's 57 drawn cells: 51 give both eyes,
    /// 4 are frames cropped by the cell edge where only one eye is drawn, and 2
    /// are shut-eyed smiles with no ring to find. `docs/palette.md` §6.
    fn eyes_in_cell(&self, origin_x: usize, origin_y: usize, cell: (usize, usize)) -> Vec<usize> {
        let (cell_width, cell_height) = cell;
        let mut ink = vec![false; cell_width * cell_height];
        for spot in 0..ink.len() {
            let pixel = self.cell_pixel(spot, origin_x, origin_y, cell);
            if pixel[3] > OPAQUE && lightness([pixel[0], pixel[1], pixel[2]]) < INK_HIGH {
                ink[spot] = true;
            }
        }

        // A rim with a one-pixel gap leaks, so try the sealed copy as well as
        // the plain one and let the overlap check drop the duplicates.
        let plain = components(&ink, cell_width, cell_height);
        let sealed = components(&closing(&ink, cell_width, cell_height), cell_width, cell_height);

        let mut seeds: Vec<Vec<usize>> = Vec::new();
        let mut rims: Vec<Vec<usize>> = Vec::new();
        let mut claimed: Vec<Vec<usize>> = Vec::new();
        for component in plain.iter().chain(sealed.iter()) {
            let holes = enclosed(component, cell_width, cell_height);
            if !(HOLE_MIN..=HOLE_MAX).contains(&holes.len()) {
                continue;
            }
            // The sealed pass re-finds rings the plain pass already had, and
            // its copy sits a pixel wider, so keying on a coordinate lets every
            // duplicate through. Two rings sharing most of a hole are one eye.
            if claimed.iter().any(|other| {
                let shared = holes.iter().filter(|spot| other.contains(spot)).count();
                shared * 2 > holes.len().min(other.len())
            }) {
                continue;
            }
            claimed.push(holes.clone());
            seeds.push(component.iter().copied().chain(holes).collect());
            rims.push(component.clone());
        }

        // An eye is not "a blob with a ring" -- it is one of a PAIR. Rings are
        // how a pair gets found, not what makes something an eye.
        //
        // Taking every ring at face value put a 514px blob out on the tail into
        // the mask on the last frame of the landing animation, and the real left
        // eye -- whose rim is open, so it encloses nothing -- was never looked
        // for. Two more cells had the same fault with 931px and 1,091px blobs.
        //
        // Asking for the best pair instead fixes all three at once: a lone false
        // blob has nothing that matches it and drops out by itself, and an eye
        // with a broken rim is still caught by its twin. Falls back to every
        // ring when no pair exists, which is what the frames showing a single
        // eye need.
        let lone = match self.best_pair(&rims, &plain, origin_x, origin_y, cell) {
            Some((index, partner)) => {
                seeds = vec![seeds.swap_remove(index), partner];
                false
            }
            None => true,
        };

        seeds
            .into_iter()
            .map(|seed| self.grown_eye(&seed, origin_x, origin_y, cell))
            // With no partner to vouch for it a ring is on its own, and the
            // sizes separate cleanly: every eye found as half of a pair grows
            // to 289-357px, while the blobs that turned out to be tail,
            // shoulder and hind leg came in at 410, 715, 931 and 1,091. The
            // frames that genuinely show a single eye sit at 303-318 and stay.
            .filter(|mask| !lone || mask.len() <= EYE_MASK_MAX)
            .collect::<Vec<_>>()
            .into_iter()
            .flatten()
            .map(|spot| {
                let (x, y) = (spot % cell_width, spot / cell_width);
                (origin_y + y) * self.width + origin_x + x
            })
            .collect()
    }

    fn cell_pixel(&self, spot: usize, origin_x: usize, origin_y: usize, cell: (usize, usize)) -> &[u8] {
        let (x, y) = (spot % cell.0, spot / cell.0);
        let start = ((origin_y + y) * self.width + origin_x + x) * 4;
        &self.pixels[start..start + 4]
    }

    /// Grow off the ink to pick up the brown iris crescent, which lies outside
    /// the ink component and is exactly what would otherwise be painted as fur.
    /// Bounded, so it cannot run away into the marking -- that escape is how
    /// every earlier attempt died.
    fn grown_eye(
        &self,
        seed: &[usize],
        origin_x: usize,
        origin_y: usize,
        cell: (usize, usize),
    ) -> Vec<usize> {
        let (cell_width, cell_height) = cell;
        let mut mask = seed.to_vec();
        let mut inside = vec![false; cell_width * cell_height];
        for spot in seed {
            inside[*spot] = true;
        }
        let mut frontier = mask.clone();
        for _ in 0..EYE_GROWTH {
            let mut next = Vec::new();
            for spot in &frontier {
                for neighbour in around(*spot, cell_width, cell_height) {
                    if inside[neighbour] {
                        continue;
                    }
                    let pixel = self.cell_pixel(neighbour, origin_x, origin_y, cell);
                    if pixel[3] <= OPAQUE || lightness([pixel[0], pixel[1], pixel[2]]) >= CREAM_LOW {
                        continue;
                    }
                    inside[neighbour] = true;
                    next.push(neighbour);
                }
            }
            if next.is_empty() {
                break;
            }
            mask.extend(next.iter().copied());
            frontier = next;
        }
        mask
    }

    /// The best pair: a ring we trust, and the blob that looks like its twin --
    /// the same size, sitting on the same row, off to one side, and an island
    /// the way a real eye is. Returns which ring was chosen and its partner.
    fn best_pair(
        &self,
        rims: &[Vec<usize>],
        candidates: &[Vec<usize>],
        origin_x: usize,
        origin_y: usize,
        cell: (usize, usize),
    ) -> Option<(usize, Vec<usize>)> {
        let (cell_width, cell_height) = cell;
        let mut best: Option<(f32, usize, &Vec<usize>)> = None;
        for (index, known) in rims.iter().enumerate() {
            let (kx0, kx1, ky0, ky1) = bounds(known, cell_width);
            for candidate in candidates {
                if candidate.iter().any(|spot| known.contains(spot)) {
                    continue;
                }
                let ratio = candidate.len() as f32 / known.len() as f32;
                if !(PARTNER_SIZE_LOW..=PARTNER_SIZE_HIGH).contains(&ratio) {
                    continue;
                }
                let (cx0, cx1, cy0, cy1) = bounds(candidate, cell_width);
                let row_gap = (cy0 as isize + cy1 as isize) - (ky0 as isize + ky1 as isize);
                if row_gap.abs() > 2 * PARTNER_ROW_SLACK as isize {
                    continue;
                }
                let height_gap = (cy1 as isize - cy0 as isize) - (ky1 as isize - ky0 as isize);
                if height_gap.abs() > PARTNER_ROW_SLACK as isize {
                    continue;
                }
                // Two eyes never share a column.
                let beside = cx0 > kx1 + PARTNER_SIDE_GAP || cx1 + PARTNER_SIDE_GAP < kx0;
                if !beside {
                    continue;
                }
                if self.touches_air(candidate, origin_x, origin_y, cell, cell_width, cell_height) {
                    continue;
                }
                let closeness = (ratio - 1.0).abs();
                if best.is_none() || closeness < best.as_ref().expect("checked").0 {
                    best = Some((closeness, index, candidate));
                }
            }
        }
        best.map(|(_, index, component)| (index, component.clone()))
    }

    fn touches_air(
        &self,
        component: &[usize],
        origin_x: usize,
        origin_y: usize,
        cell: (usize, usize),
        cell_width: usize,
        cell_height: usize,
    ) -> bool {
        let mut inside = vec![false; cell_width * cell_height];
        for spot in component {
            inside[*spot] = true;
        }
        for spot in component {
            let (x, y) = (spot % cell_width, spot / cell_width);
            for (dx, dy) in NEIGHBOURS {
                let (nx, ny) = (x as isize + dx, y as isize + dy);
                if nx < 0 || ny < 0 || nx as usize >= cell_width || ny as usize >= cell_height {
                    return true;
                }
                let neighbour = ny as usize * cell_width + nx as usize;
                if inside[neighbour] {
                    continue;
                }
                if self.cell_pixel(neighbour, origin_x, origin_y, cell)[3] <= OPAQUE {
                    return true;
                }
            }
        }
        false
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

    /// Repaint the marking in straight alpha, then premultiply.
    ///
    /// `identity` is where this sheet's own marking already sits. Passing it
    /// back as `targets` has to produce the original bytes: the frame hashes in
    /// `PreW2FrameHashes.swift` compare exactly, and colour is not supposed to
    /// reach the pet's decisions at all.
    pub fn recolored(
        &self,
        map: &PaletteMap,
        identity: Palette,
        targets: Palette,
    ) -> Option<PetImage> {
        if map.regions.len() != self.width * self.height {
            return None;
        }
        if targets == identity {
            return Some(self.image());
        }

        // Asked region by region, not once for the whole palette. Repainting a
        // region replaces its varying chroma with a single value, so touching
        // the body would otherwise flatten the marking as a side effect. A
        // region nobody moved keeps its own bytes.
        let moved = |region: u8| match region {
            REGION_MARKING => targets.marking != identity.marking,
            REGION_BODY => targets.body != identity.body,
            REGION_EYE => targets.eye != identity.eye,
            _ => false,
        };

        // Rank inside each region's own lightness order, not the raw value.
        // The marking is bunched low -- p5/p50/p95 of 16/30/53 -- so spreading
        // it by value leaves every colour darker than the answer cats. Rank
        // also carries an eye across intact: the pupil is the darkest thing in
        // it and the catchlight the lightest, whatever colour they end up.
        let mut ramps: [Vec<f32>; 4] = Default::default();
        for (index, pixel) in self.pixels.chunks_exact(4).enumerate() {
            let region = map.regions[index];
            if pixel[3] > OPAQUE && moved(region) {
                ramps[region as usize].push(lightness([pixel[0], pixel[1], pixel[2]]));
            }
        }
        for ramp in &mut ramps {
            ramp.sort_by(|left, right| left.partial_cmp(right).expect("no NaN lightness"));
        }

        let mut pixels = self.pixels.clone();
        for (index, pixel) in pixels.chunks_exact_mut(4).enumerate() {
            let alpha = u32::from(pixel[3]);
            let region = map.regions[index];
            if pixel[3] > OPAQUE && moved(region) {
                let ramp = &ramps[region as usize];
                if let (Some(aim), false) = (targets.targets(region), ramp.is_empty()) {
                    let here = lightness([pixel[0], pixel[1], pixel[2]]);
                    let position = rank(ramp, here);
                    let light = aim.light_low + position * (aim.light_high - aim.light_low);
                    pixel[..3].copy_from_slice(&from_hls(aim.hue, light, aim.chroma));
                }
            }
            for channel in 0..3 {
                pixel[channel] = premultiplied_channel(pixel[channel], alpha);
            }
        }
        Some(PetImage {
            width: self.width,
            height: self.height,
            pixels,
        })
    }
}

/// Which pixels of one straight-alpha sheet the recolour may touch.
#[derive(Debug, Clone)]
pub struct PaletteMap {
    regions: Vec<u8>,
}

impl PaletteMap {
    /// How many pixels the recolour would repaint. The eye count is the
    /// interesting one: those are pixels that *would* have been repainted and
    /// are held back, and without them a blue cat gets blue eyes.
    pub fn marking_pixels(&self) -> usize {
        self.regions
            .iter()
            .filter(|region| **region == REGION_MARKING)
            .count()
    }

    pub fn eye_pixels(&self) -> usize {
        self.regions
            .iter()
            .filter(|region| **region == REGION_EYE)
            .count()
    }

    pub fn body_pixels(&self) -> usize {
        self.regions
            .iter()
            .filter(|region| **region == REGION_BODY)
            .count()
    }
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

const REGION_KEEP: u8 = 0;
const REGION_MARKING: u8 = 1;
/// An eye sits inside the marking band by lightness and cannot be separated
/// from it by colour: the iris is a dark warm brown near L 30 and the dark fur
/// is near L 26. Only the ring of outline around it tells them apart.
const REGION_EYE: u8 = 2;
const REGION_BODY: u8 = 3;

const HOLE_MIN: usize = 8;
const HOLE_MAX: usize = 80;
const EYE_GROWTH: usize = 3;
/// How far a partner may differ from the eye already found by its own ring.
const PARTNER_SIZE_LOW: f32 = 0.6;
const PARTNER_SIZE_HIGH: f32 = 1.7;
const PARTNER_ROW_SLACK: usize = 6;
const PARTNER_SIDE_GAP: usize = 4;
/// Largest grown mask a ring may produce when nothing vouches for it as a pair.
const EYE_MASK_MAX: usize = 400;
const NEIGHBOURS: [(isize, isize); 8] = [
    (-1, -1),
    (0, -1),
    (1, -1),
    (-1, 0),
    (1, 0),
    (-1, 1),
    (0, 1),
    (1, 1),
];
const ORTHOGONAL: [(isize, isize); 4] = [(0, -1), (-1, 0), (1, 0), (0, 1)];

/// HLS lightness in percent: `(max + min) / 2`.
fn lightness(colour: PaletteColour) -> f32 {
    let low = *colour.iter().min().expect("three channels") as f32;
    let high = *colour.iter().max().expect("three channels") as f32;
    (low + high) * 100.0 / 510.0
}

fn is_marking(colour: PaletteColour) -> bool {
    (MARKING_LOW..MARKING_HIGH).contains(&lightness(colour))
}

fn is_body(colour: PaletteColour) -> bool {
    (CREAM_LOW..CREAM_HIGH).contains(&lightness(colour))
}

/// Where `value` sits in a sorted ramp, as 0.0 to 1.0.
///
/// Ties take the lower edge of their run, so a value shared by many pixels
/// ranks where its run begins rather than in the middle of it. That keeps the
/// dark end exact -- the darkest colour always maps to `light_low` -- at the
/// cost of the bright end falling short when the lightest colour is repeated.
/// The scoring that chose this operator was done against the same definition
/// (`output/palette-answers/prototype.py`), so changing it would invalidate it.
fn rank(ramp: &[f32], value: f32) -> f32 {
    let index = ramp.partition_point(|entry| *entry < value);
    if ramp.len() <= 1 {
        return 0.0;
    }
    (index as f32 / (ramp.len() - 1) as f32).clamp(0.0, 1.0)
}

/// HLS back to bytes, with chroma given in 0-255 rather than as a saturation.
fn from_hls(hue: f32, light: f32, chroma: f32) -> PaletteColour {
    let light = (light / 100.0).clamp(0.0, 1.0);
    let span = 1.0 - (2.0 * light - 1.0).abs();
    let chroma = (chroma / 255.0).clamp(0.0, 1.0).min(span);
    let hue = hue.rem_euclid(360.0) / 60.0;
    let second = chroma * (1.0 - (hue % 2.0 - 1.0).abs());
    let (red, green, blue) = match hue as u32 {
        0 => (chroma, second, 0.0),
        1 => (second, chroma, 0.0),
        2 => (0.0, chroma, second),
        3 => (0.0, second, chroma),
        4 => (second, 0.0, chroma),
        _ => (chroma, 0.0, second),
    };
    let base = light - chroma / 2.0;
    [
        ((red + base) * 255.0).round().clamp(0.0, 255.0) as u8,
        ((green + base) * 255.0).round().clamp(0.0, 255.0) as u8,
        ((blue + base) * 255.0).round().clamp(0.0, 255.0) as u8,
    ]
}

fn around(spot: usize, width: usize, height: usize) -> Vec<usize> {
    let (x, y) = (spot % width, spot / width);
    let mut found = Vec::with_capacity(8);
    for (dx, dy) in NEIGHBOURS {
        let (nx, ny) = (x as isize + dx, y as isize + dy);
        if nx >= 0 && ny >= 0 && (nx as usize) < width && (ny as usize) < height {
            found.push(ny as usize * width + nx as usize);
        }
    }
    found
}

fn bounds(component: &[usize], width: usize) -> (usize, usize, usize, usize) {
    let mut x0 = usize::MAX;
    let mut x1 = 0;
    let mut y0 = usize::MAX;
    let mut y1 = 0;
    for spot in component {
        let (x, y) = (spot % width, spot / width);
        x0 = x0.min(x);
        x1 = x1.max(x);
        y0 = y0.min(y);
        y1 = y1.max(y);
    }
    (x0, x1, y0, y1)
}

fn components(mask: &[bool], width: usize, height: usize) -> Vec<Vec<usize>> {
    let mut seen = vec![false; mask.len()];
    let mut found = Vec::new();
    for start in 0..mask.len() {
        if !mask[start] || seen[start] {
            continue;
        }
        let mut queue = VecDeque::from([start]);
        seen[start] = true;
        let mut cells = Vec::new();
        while let Some(spot) = queue.pop_front() {
            cells.push(spot);
            let (x, y) = (spot % width, spot / width);
            for (dx, dy) in NEIGHBOURS {
                let (nx, ny) = (x as isize + dx, y as isize + dy);
                if nx < 0 || ny < 0 || nx as usize >= width || ny as usize >= height {
                    continue;
                }
                let neighbour = ny as usize * width + nx as usize;
                if mask[neighbour] && !seen[neighbour] {
                    seen[neighbour] = true;
                    queue.push_back(neighbour);
                }
            }
        }
        found.push(cells);
    }
    found
}

/// Dilate then erode, bridging a gap of a pixel or two.
///
/// Safe in the direction that matters. The opening I tried first -- erode then
/// dilate -- ate the iris down to a line. Closing cannot shrink the eye.
fn closing(mask: &[bool], width: usize, height: usize) -> Vec<bool> {
    let mut grown = vec![false; mask.len()];
    for spot in 0..mask.len() {
        if !mask[spot] {
            continue;
        }
        grown[spot] = true;
        let (x, y) = (spot % width, spot / width);
        for (dx, dy) in NEIGHBOURS {
            let (nx, ny) = (x as isize + dx, y as isize + dy);
            if nx >= 0 && ny >= 0 && (nx as usize) < width && (ny as usize) < height {
                grown[ny as usize * width + nx as usize] = true;
            }
        }
    }
    let mut shrunk = vec![false; mask.len()];
    for spot in 0..grown.len() {
        if !grown[spot] {
            continue;
        }
        let (x, y) = (spot % width, spot / width);
        let solid = NEIGHBOURS.iter().all(|(dx, dy)| {
            let (nx, ny) = (x as isize + dx, y as isize + dy);
            nx >= 0
                && ny >= 0
                && (nx as usize) < width
                && (ny as usize) < height
                && grown[ny as usize * width + nx as usize]
        });
        shrunk[spot] = solid;
    }
    shrunk
}

/// The pixels a component wraps around.
///
/// Flood the component's bounding box from its border, going round the
/// component. Anything the flood cannot reach is inside the ring. Orthogonal
/// neighbours only, so a diagonal gap in the rim does not count as a way out.
fn enclosed(component: &[usize], width: usize, height: usize) -> Vec<usize> {
    let mut inside = vec![false; width * height];
    for spot in component {
        inside[*spot] = true;
    }
    let xs = component.iter().map(|spot| spot % width);
    let ys = component.iter().map(|spot| spot / width);
    let x0 = xs.clone().min().unwrap_or(0).saturating_sub(1);
    let x1 = (xs.max().unwrap_or(0) + 1).min(width - 1);
    let y0 = ys.clone().min().unwrap_or(0).saturating_sub(1);
    let y1 = (ys.max().unwrap_or(0) + 1).min(height - 1);

    let mut reached = vec![false; width * height];
    let mut queue = VecDeque::new();
    let seed = |x: usize, y: usize, reached: &mut Vec<bool>, queue: &mut VecDeque<usize>| {
        let spot = y * width + x;
        if !inside[spot] && !reached[spot] {
            reached[spot] = true;
            queue.push_back(spot);
        }
    };
    for x in x0..=x1 {
        seed(x, y0, &mut reached, &mut queue);
        seed(x, y1, &mut reached, &mut queue);
    }
    for y in y0..=y1 {
        seed(x0, y, &mut reached, &mut queue);
        seed(x1, y, &mut reached, &mut queue);
    }
    while let Some(spot) = queue.pop_front() {
        let (x, y) = (spot % width, spot / width);
        for (dx, dy) in ORTHOGONAL {
            let (nx, ny) = (x as isize + dx, y as isize + dy);
            if nx < 0 || ny < 0 {
                continue;
            }
            let (nx, ny) = (nx as usize, ny as usize);
            if nx < x0 || nx > x1 || ny < y0 || ny > y1 {
                continue;
            }
            let neighbour = ny * width + nx;
            if inside[neighbour] || reached[neighbour] {
                continue;
            }
            reached[neighbour] = true;
            queue.push_back(neighbour);
        }
    }

    let mut holes = Vec::new();
    for y in y0..=y1 {
        for x in x0..=x1 {
            let spot = y * width + x;
            if !inside[spot] && !reached[spot] {
                holes.push(spot);
            }
        }
    }
    holes
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

    const IDENTITY: Palette = Palette::new(
        PaletteTargets::new(22.0, 16.0, 53.0, 90.0),
        PaletteTargets::new(38.0, 86.0, 95.0, 33.0),
        PaletteTargets::new(28.0, 3.0, 95.0, 70.0),
    );

    fn with_marking(aim: PaletteTargets) -> Palette {
        Palette {
            marking: aim,
            ..IDENTITY
        }
    }

    fn source(pixels: &[[u8; 4]]) -> PetImageSource {
        PetImageSource {
            width: pixels.len(),
            height: 1,
            pixels: pixels.iter().flatten().copied().collect(),
        }
    }

    #[test]
    fn the_marking_band_keeps_its_ends() {
        assert!(!is_marking([50, 0, 0])); // below 10%, the outline
        assert!(is_marking([51, 0, 0]));
        assert!(is_marking([215, 0, 0])); // the old 40-42 gap is marking now
        assert!(!is_marking([255, 41, 41])); // above 58%, the blush
        assert!(!is_marking([255, 255, 255]));
    }

    #[test]
    fn identity_targets_are_byte_identical() {
        let image = source(&[[80, 40, 20, 255], [220, 120, 40, 255], [250, 235, 220, 255]]);
        let map = image.region_map((0, 0));
        assert_eq!(
            image.recolored(&map, IDENTITY, IDENTITY),
            Some(image.image())
        );
    }

    #[test]
    fn recoloring_never_changes_alpha() {
        let image = source(&[
            [70, 35, 15, 201],
            [210, 115, 35, 128],
            [245, 230, 215, 1],
            [7, 9, 11, 0],
        ]);
        let map = image.region_map((0, 0));
        let recolored = image
            .recolored(&map, IDENTITY, with_marking(PaletteTargets::new(214.0, 30.0, 95.0, 120.0)))
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
    fn only_the_marking_moves() {
        // outline, marking, blush, cream
        let image = source(&[
            [8, 6, 5, 255],
            [115, 52, 27, 255],
            [251, 154, 119, 255],
            [254, 242, 220, 255],
        ]);
        let map = image.region_map((0, 0));
        assert_eq!(map.marking_pixels(), 1);
        let recolored = image
            .recolored(&map, IDENTITY, with_marking(PaletteTargets::new(214.0, 30.0, 95.0, 120.0)))
            .expect("matching map");
        let plain = image.image();
        for index in [0_usize, 2, 3] {
            assert_eq!(
                recolored.pixels[index * 4..index * 4 + 3],
                plain.pixels[index * 4..index * 4 + 3],
                "pixel {index} should not have moved"
            );
        }
        assert_ne!(
            recolored.pixels[4..7],
            plain.pixels[4..7],
            "the marking should have moved"
        );
    }

    #[test]
    fn moving_one_region_leaves_the_others_byte_identical() {
        // outline, marking, blush, cream
        let image = source(&[
            [8, 6, 5, 255],
            [115, 52, 27, 255],
            [251, 154, 119, 255],
            [254, 242, 220, 255],
        ]);
        let map = image.region_map((0, 0));
        assert_eq!(map.body_pixels(), 1);
        let moved = Palette {
            body: PaletteTargets::new(210.0, 80.0, 96.0, 40.0),
            ..IDENTITY
        };
        let recolored = image.recolored(&map, IDENTITY, moved).expect("matching map");
        let plain = image.image();
        // Repainting a region replaces its varying chroma with one value, so a
        // region nobody asked about has to be left alone completely.
        for index in [0_usize, 1, 2] {
            assert_eq!(
                recolored.pixels[index * 4..index * 4 + 3],
                plain.pixels[index * 4..index * 4 + 3],
                "pixel {index} should not have moved"
            );
        }
        assert_ne!(recolored.pixels[12..15], plain.pixels[12..15]);
    }

    #[test]
    fn a_recoloured_eye_keeps_its_pupil_dark_and_its_catchlight_bright() {
        // Same 7x7 ring as the detection test: ink rim, pale hole.
        let mut pixels = vec![[254, 242, 220, 255]; 49];
        for y in 1..6 {
            for x in 1..6 {
                let edge = x == 1 || x == 5 || y == 1 || y == 5;
                pixels[y * 7 + x] = if edge {
                    [5, 4, 4, 255]
                } else {
                    [250, 250, 250, 255]
                };
            }
        }
        let image = PetImageSource {
            width: 7,
            height: 7,
            pixels: pixels.iter().flatten().copied().collect(),
        };
        let map = image.region_map((7, 7));
        let blue_eyed = Palette {
            eye: PaletteTargets::new(214.0, 10.0, 90.0, 120.0),
            ..IDENTITY
        };
        let recolored = image
            .recolored(&map, IDENTITY, blue_eyed)
            .expect("matching map");
        let at = |x: usize, y: usize| {
            let start = (y * 7 + x) * 4;
            lightness([
                recolored.pixels[start],
                recolored.pixels[start + 1],
                recolored.pixels[start + 2],
            ])
        };
        // Lightness is placed by rank, so the darkest thing in the eye stays
        // the darkest and the brightest stays the brightest whatever hue they
        // are given. That is what carries an eye's structure across.
        //
        // The bright end does not land exactly on `light_high`: ties take the
        // lower edge of their run, and here nine of twenty-five pixels share
        // one lightness. On a real sheet lightness is near-continuous and the
        // gap is invisible. What has to hold is the order and the bounds.
        assert!(at(1, 1) < at(3, 3), "the rim should stay darker than the hole");
        assert!(at(1, 1) <= 12.0, "the rim should sit at the dark end");
        assert!(
            (50.0..=90.0).contains(&at(3, 3)),
            "the catchlight should be well up the ramp and inside it"
        );
    }

    #[test]
    fn translucent_pixels_are_left_alone() {
        // Alpha 200 is the census floor; a pixel at or below it is a blend of
        // two regions and repainting it would harden the seam.
        let image = source(&[[115, 52, 27, 200], [115, 52, 27, 255]]);
        let map = image.region_map((0, 0));
        assert_eq!(map.marking_pixels(), 1);
    }

    #[test]
    fn a_fully_transparent_pixel_survives_untouched() {
        let image = source(&[[115, 52, 27, 255], [7, 9, 11, 0]]);
        let map = image.region_map((0, 0));
        let recolored = image
            .recolored(&map, IDENTITY, with_marking(PaletteTargets::new(300.0, 10.0, 90.0, 200.0)))
            .expect("matching map");
        assert_eq!(&recolored.pixels[4..8], &[0, 0, 0, 0]);
    }

    #[test]
    fn hls_round_trips_the_measured_marking() {
        // hue 22, mid lightness, chroma 150 should land in warm orange-brown
        let colour = from_hls(22.0, 41.0, 150.0);
        assert_eq!(lightness(colour).round(), 41.0);
        assert_eq!((colour.iter().max().unwrap() - colour.iter().min().unwrap()) as i32, 150);
    }

    #[test]
    fn chroma_is_clamped_to_what_the_lightness_allows() {
        // Near white there is no room for chroma 255; asking for it must not
        // wrap or overflow, it must give the most saturated colour available.
        let colour = from_hls(22.0, 96.0, 255.0);
        assert!(lightness(colour) >= 95.0);
    }

    #[test]
    fn a_ring_of_ink_round_a_catchlight_is_an_eye() {
        // 7x7 cell: a closed ink rim with a 3x3 hole of lighter pixels.
        let mut pixels = vec![[254, 242, 220, 255]; 49];
        for y in 1..6 {
            for x in 1..6 {
                let edge = x == 1 || x == 5 || y == 1 || y == 5;
                pixels[y * 7 + x] = if edge {
                    [5, 4, 4, 255]
                } else {
                    [250, 250, 250, 255]
                };
            }
        }
        let image = PetImageSource {
            width: 7,
            height: 7,
            pixels: pixels.iter().flatten().copied().collect(),
        };
        let map = image.region_map((7, 7));
        assert_eq!(map.eye_pixels(), 25, "rim plus the hole it wraps");
    }

    #[test]
    fn a_stroke_encloses_nothing_and_is_not_an_eye() {
        let mut pixels = vec![[254, 242, 220, 255]; 49];
        for x in 0..7 {
            pixels[3 * 7 + x] = [5, 4, 4, 255];
        }
        let image = PetImageSource {
            width: 7,
            height: 7,
            pixels: pixels.iter().flatten().copied().collect(),
        };
        let map = image.region_map((7, 7));
        assert_eq!(map.eye_pixels(), 0);
    }
}
