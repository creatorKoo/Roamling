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
//! has 58, and turned the blush blue. The whole argument is in `docs/history/palette-making.md`
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
    /// Where the hue arrives by the light end of the ramp. Equal to `hue` for
    /// every colour the answer cats drew -- all thirteen held one hue across
    /// the whole marking -- so this exists for the one thing they could not
    /// show us. A rainbow needs the colour to travel, and travelling along the
    /// shading is free: `rank` already says where each pixel sits.
    pub hue_end: f32,
    pub light_low: f32,
    pub light_high: f32,
    pub chroma: f32,
}

impl PaletteTargets {
    /// One hue all the way through, which is what a measured colour is.
    pub const fn new(hue: f32, light_low: f32, light_high: f32, chroma: f32) -> Self {
        Self {
            hue,
            hue_end: hue,
            light_low,
            light_high,
            chroma,
        }
    }

    /// A hue that travels as the shading climbs. `hue_end` is not wrapped to
    /// the shorter way round: 0 to 359 is meant to be the long way, or there
    /// would be no rainbow.
    pub const fn sweeping(
        hue: f32,
        hue_end: f32,
        light_low: f32,
        light_high: f32,
        chroma: f32,
    ) -> Self {
        Self {
            hue,
            hue_end,
            light_low,
            light_high,
            chroma,
        }
    }

    /// The one colour this ramp reads as: its middle, in hue as well as in
    /// lightness. A swatch showing it has to be built from the same arithmetic
    /// the recolour uses, or it tells the person a colour the cat is not
    /// wearing -- which is the whole reason a swatch exists.
    pub fn middle(self) -> PaletteColour {
        from_hls(
            (self.hue + self.hue_end) / 2.0,
            (self.light_low + self.light_high) / 2.0,
            self.chroma,
        )
    }

    /// Point the ramp at a colour, keeping the width of its shading.
    ///
    /// A region is five numbers and a colour picker gives one colour, so
    /// something has to say which of the five it is. It is the middle (user
    /// decision 2026-09-13): the span between the ends is what makes a cat's
    /// dark line work read as line work, and it was measured, so picking a
    /// colour slides that span rather than collapsing it.
    ///
    /// A picked colour is one colour, so it ends any sweep. The hue-end slider
    /// is still there to start another.
    pub fn aimed_at(self, colour: PaletteColour) -> Self {
        let span = (self.light_high - self.light_low).clamp(0.0, 100.0);
        let hue = hue_of(colour);
        let light_low = (lightness(colour) - span / 2.0).clamp(0.0, 100.0 - span);
        Self {
            hue,
            hue_end: hue,
            light_low,
            light_high: light_low + span,
            chroma: chroma_of(colour),
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

/// sheets rather than chosen (`docs/history/palette-making.md` §0). Handing these back as
/// targets has to return the original bytes -- the frame hashes compare
/// exactly, so these have to be integers a slider can land on.
///
/// The eye's chroma looks low because half that region is the near-black pupil
/// and the white catchlight. Only the brown iris carries colour, and rank
/// keeps all three in their places when the eye is moved.
pub const BUILT_IN_PALETTE: Palette = Palette::new(
    PaletteTargets::new(22.0, 17.0, 53.0, 90.0),
    PaletteTargets::new(38.0, 86.0, 95.0, 33.0),
    PaletteTargets::new(21.0, 0.0, 78.0, 17.0),
);

/// The colours the menu offers, in the order it shows them.
///
/// The hues come off the hand-drawn answer cats (`docs/history/palette-making.md` §0). The
/// **lightness ends do not**, and that is the one place these were tuned by eye
/// rather than measured: the answers' marking is bunched at one lightness while
/// this ramp spreads evenly by rank, so reusing their p5 as the dark end left
/// yellow reading as olive and red as washed salmon. Raising the dark end fixes
/// both. The first entry is the sheet as drawn, so picking it restores the
/// original bytes exactly.
///
/// **The body moves only for black and white** -- the answer key's black cat
/// dropped the cream body from lightness 93 to 33, and no other colour touched
/// it. When it does move the blush and the boundary shading follow it, or the
/// cat ends up outlined in glowing cream.
///
/// **Eyes are paired to the fur** the way a cat's are: gold on black, blue on
/// white, green on ginger. The dark end stays at 0 so the pupil is still black
/// -- chroma cannot survive there and `from_hls` clamps it away -- and the
/// light end keeps the catchlight bright, so the colour lands on the iris
/// between them and nowhere else.
const PRESETS: [(&str, Palette); 9] = [
    ("palette.default", BUILT_IN_PALETTE),
    (
        // A cat that is black rather than Mochi seen at night: marking and
        // body land on nearly the same darkness, so the calico patches stop
        // showing through and only the outline and the eyes read.
        "palette.black",
        Palette::new(
            PaletteTargets::new(40.0, 3.0, 18.0, 8.0),
            PaletteTargets::new(40.0, 8.0, 20.0, 8.0),
            PaletteTargets::new(48.0, 10.0, 96.0, 205.0),
        ),
    ),
    (
        "palette.white",
        Palette::new(
            PaletteTargets::new(26.0, 42.0, 92.0, 16.0),
            PaletteTargets::new(38.0, 90.0, 98.0, 14.0),
            PaletteTargets::new(210.0, 0.0, 84.0, 190.0),
        ),
    ),
    (
        "palette.red",
        Palette::new(
            PaletteTargets::new(8.0, 28.0, 80.0, 200.0),
            BUILT_IN_PALETTE.body,
            PaletteTargets::new(112.0, 0.0, 82.0, 180.0),
        ),
    ),
    (
        "palette.hotpink",
        Palette::new(
            PaletteTargets::new(332.0, 33.0, 84.0, 205.0),
            BUILT_IN_PALETTE.body,
            PaletteTargets::new(185.0, 0.0, 82.0, 190.0),
        ),
    ),
    (
        "palette.yellow",
        Palette::new(
            PaletteTargets::new(48.0, 30.0, 90.0, 200.0),
            BUILT_IN_PALETTE.body,
            PaletteTargets::new(100.0, 0.0, 82.0, 180.0),
        ),
    ),
    (
        "palette.blue",
        Palette::new(
            PaletteTargets::new(214.0, 22.0, 78.0, 195.0),
            BUILT_IN_PALETTE.body,
            PaletteTargets::new(35.0, 0.0, 82.0, 200.0),
        ),
    ),
    (
        "palette.purple",
        Palette::new(
            PaletteTargets::new(290.0, 20.0, 76.0, 185.0),
            BUILT_IN_PALETTE.body,
            PaletteTargets::new(45.0, 0.0, 82.0, 195.0),
        ),
    ),
    (
        // The one colour no answer cat could show: a hue that travels instead
        // of holding still. It sweeps the long way round on purpose.
        "palette.rainbow",
        Palette::new(
            PaletteTargets::sweeping(0.0, 359.0, 36.0, 86.0, 175.0),
            BUILT_IN_PALETTE.body,
            PaletteTargets::new(330.0, 0.0, 82.0, 195.0),
        ),
    ),
];

/// A palette as one settings line: five numbers per region, regions by `;`.
///
/// Here rather than in either shell because both write it and the file has to
/// mean the same thing on a machine that changed platforms. Reading it back is
/// `palette_from_text`.
pub fn palette_to_text(palette: Palette) -> String {
    let part = |aim: PaletteTargets| {
        format!(
            "{},{},{},{},{}",
            aim.hue, aim.hue_end, aim.light_low, aim.light_high, aim.chroma
        )
    };
    format!(
        "{};{};{}",
        part(palette.marking),
        part(palette.body),
        part(palette.eye)
    )
}

/// `None` for anything this did not write, so a hand-edited or older line
/// falls back to the shipped colours rather than to a half-read palette.
pub fn palette_from_text(text: &str) -> Option<Palette> {
    let mut parts = text.split(';');
    let mut next = || -> Option<PaletteTargets> {
        let numbers: Option<Vec<f32>> = parts
            .next()?
            .split(',')
            .map(|number| number.trim().parse::<f32>().ok())
            .collect();
        let [hue, hue_end, low, high, chroma] = numbers?[..] else {
            return None;
        };
        Some(PaletteTargets::sweeping(hue, hue_end, low, high, chroma))
    };
    let marking = next()?;
    let body = next()?;
    let eye = next()?;
    Some(Palette::new(marking, body, eye))
}

/// Name key and palette for each preset, in menu order.
///
/// Here rather than in `roamling-pet` for the reason the decoder is here: the
/// macOS shell links this crate and not that one, and a second copy of these
/// numbers is a second answer to "what colour is the black cat".
pub fn palette_presets() -> &'static [(&'static str, Palette)] {
    &PRESETS
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
        let mut ink = vec![false; self.width * self.height];
        for (index, pixel) in self.pixels.chunks_exact(4).enumerate() {
            if pixel[3] <= OPAQUE {
                continue;
            }
            let colour = [pixel[0], pixel[1], pixel[2]];
            if is_marking(colour) {
                regions[index] = REGION_MARKING;
            } else if is_body(colour) {
                regions[index] = REGION_BODY;
            } else if (MARKING_HIGH..CREAM_LOW).contains(&lightness(colour)) {
                regions[index] = REGION_ACCENT;
            } else if lightness(colour) < INK_HIGH {
                ink[index] = true;
            }
        }
        let body: Vec<bool> = regions.iter().map(|region| *region == REGION_BODY).collect();
        // The outline is drawn antialiased, so the marking pixels touching it
        // are part ink and part fur, and they are what gives the line its
        // weight. Which ones they are cannot be read off lightness: measured
        // over the sheet they run 11/19/26/40/54 at the five percentiles while
        // fur away from the outline runs 23/26/36/52/54. Adjacency separates
        // them exactly, and it is known here once rather than per repaint.
        let edges: Vec<bool> = (0..regions.len())
            .map(|index| regions[index] == REGION_MARKING && self.beside(&ink, index))
            .collect();

        // The accent band holds two different things. The blush, the nose and
        // the inner ear are colours, and the answer cats leave all three alone.
        // But the same lightness range also holds the gradient where the fur
        // meets the cream body, and that is not a colour -- it is a blend of
        // the two. Leaving it alone is what drew a ragged orange line between a
        // yellow cat's fur and its face: the fur beside it had gone yellow and
        // the blend had not.
        //
        // A blend can be reached from both of the things it blends; a blush can
        // be reached from the body alone. The marking it is reached from has to
        // be real fur, because every ink stroke on this cat carries a skirt of
        // antialiasing that lands in the marking band, and without that clause
        // the mouth counts as a seam and ends up wearing a halo.
        let mut clear = ink.clone();
        for _ in 0..SEAM_INK_CLEAR {
            clear = (0..clear.len())
                .map(|index| clear[index] || self.beside(&clear, index))
                .collect();
        }
        for index in 0..clear.len() {
            clear[index] = regions[index] == REGION_MARKING && !clear[index];
        }
        for index in 0..regions.len() {
            if regions[index] == REGION_ACCENT
                && self.beside(&clear, index)
                && self.beside(&body, index)
            {
                regions[index] = REGION_SEAM;
            }
        }

        // Eyes last: they sit inside the marking band by lightness and have to
        // win, which is the whole reason the ring pass exists.
        for (origin_x, origin_y) in self.cells(cell) {
            for spot in self.eyes_in_cell(origin_x, origin_y, cell) {
                regions[spot] = REGION_EYE;
            }
        }
        PaletteMap { regions, edges }
    }

    fn beside(&self, mask: &[bool], index: usize) -> bool {
        let (x, y) = ((index % self.width) as isize, (index / self.width) as isize);
        NEIGHBOURS.iter().any(|(step_x, step_y)| {
            let (near_x, near_y) = (x + step_x, y + step_y);
            near_x >= 0
                && near_y >= 0
                && (near_x as usize) < self.width
                && (near_y as usize) < self.height
                && mask[near_y as usize * self.width + near_x as usize]
        })
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
    /// are shut-eyed smiles with no ring to find. `docs/history/palette-making.md` §6.
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
        let lone = if rims.is_empty() {
            // Nothing ringed at all means every eye in this frame is shut or
            // lidded, and only then is it safe to go looking for strokes.
            seeds = self.lidded_pair(&plain, origin_x, origin_y, cell);
            false
        } else {
            match self.best_pair(&rims, &plain, origin_x, origin_y, cell) {
                Some((index, partner)) => {
                    seeds = vec![seeds.swap_remove(index), partner];
                    false
                }
                None => true,
            }
        };

        // Which cream blob each pixel belongs to, and how far each blob reaches.
        // The growth needs this to tell the face from a catchlight: both are
        // cream, and it refuses to step anywhere that can see cream.
        let mut cream = vec![false; cell_width * cell_height];
        for (spot, flag) in cream.iter_mut().enumerate() {
            let pixel = self.cell_pixel(spot, origin_x, origin_y, cell);
            *flag = pixel[3] > OPAQUE && lightness([pixel[0], pixel[1], pixel[2]]) >= CREAM_LOW;
        }
        let mut blob = vec![usize::MAX; cell_width * cell_height];
        let mut reach = Vec::new();
        for patch in components(&cream, cell_width, cell_height) {
            for spot in &patch {
                blob[*spot] = reach.len();
            }
            reach.push(bounds(&patch, cell_width));
        }

        let masks: Vec<Vec<usize>> = seeds
            .into_iter()
            .map(|seed| self.grown_eye(&seed, &blob, &reach, origin_x, origin_y, cell))
            // With no partner to vouch for it a ring is on its own, and the
            // sizes separate cleanly. Re-measured every time the growth changes,
            // by turning the pair rule and this filter off and growing every
            // ring on the sheet: real eyes run 51-291px and the false blobs out
            // on the jump row come in at 461 and 727. The cap sits in that gap,
            // where it already was -- measured there, not scaled into it.
            .filter(|mask| !lone || mask.len() <= EYE_MASK_MAX)
            // An eye is not tall. Area alone does not say that: the front paw
            // on the stretching frame is drawn as a ring round a cream pad,
            // which passes the hole test, and its rim is 304px, which passes
            // the area test -- but it is 68 pixels tall.
            .filter(|mask| {
                let (_, _, top, bottom) = bounds(mask, cell_width);
                bottom - top < EYE_SPAN_MAX
            })
            .collect();

        masks
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
    ///
    /// The bound is the marking band's ceiling, not the cream body's floor. The
    /// difference is the accent band, where the eyelid and the skin round the
    /// eye live: 7% of the mask, measured over the sheet, and all of it outside
    /// the drawn eye. That cost nothing while eyes stayed brown, and on a black
    /// cat with gold eyes it was a gold ring drawn on the cheek. The iris this
    /// growth exists to collect sits below the ceiling, with the rest of the
    /// marking.
    ///
    /// Lightness alone is still not enough, because the shading in the eye
    /// socket is the same brown as the iris. What separates them is which side
    /// of the rim they are on, and that is what `sees_out` asks. Shortening the
    /// growth instead does not work: at one step the crescent is left behind
    /// and comes out painted as fur, which on a blue cat is a blue eye bottom.
    #[allow(clippy::too_many_arguments)]
    fn grown_eye(
        &self,
        seed: &[usize],
        blob: &[usize],
        reach: &[(usize, usize, usize, usize)],
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
        // The rim draws the eye's extent, so growth stays inside the box the
        // rim spans. Three steps is enough to walk round the outside of the
        // rim and out into the fur beside it, and there the fur is the same
        // brown as the iris: measured on the caught frame, three pixels of the
        // head marking at lightness 44-52 had been taken into the mask and came
        // out bright green. Nothing the growth is for lies outside this box.
        let (left, right, top, bottom) = bounds(seed, cell_width);
        let mut frontier = mask.clone();
        for _ in 0..EYE_GROWTH {
            let mut next = Vec::new();
            for spot in &frontier {
                for neighbour in around(*spot, cell_width, cell_height) {
                    if inside[neighbour] {
                        continue;
                    }
                    let (x, y) = (neighbour % cell_width, neighbour / cell_width);
                    if x < left || x > right || y < top || y > bottom {
                        continue;
                    }
                    let pixel = self.cell_pixel(neighbour, origin_x, origin_y, cell);
                    if pixel[3] <= OPAQUE
                        || lightness([pixel[0], pixel[1], pixel[2]]) >= MARKING_HIGH
                    {
                        continue;
                    }
                    if sees_out(
                        neighbour,
                        blob,
                        reach,
                        (left, right, top, bottom),
                        cell_width,
                        cell_height,
                    ) {
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
        // Fill whatever the finished mask wraps around, however the seed
        // arrived. Only the ring of a pair was handed its holes; `best_pair`
        // returns its partner as the bare ink component, so one eye of every
        // pair came in without its interior. The growth put the iris back --
        // but the catchlight is cream, above where the growth stops, so nothing
        // could reach it. That is a white dot in one eye and a dark hole in the
        // other, swapping sides from frame to frame as the pair is chosen
        // differently: the dot moving left and right while the cat blinks.
        let holes = enclosed(&mask, cell_width, cell_height);
        mask.extend(holes);
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

    /// The two lid strokes of a half-lidded eye.
    ///
    /// A lidded eye is a stroke, not a ring: it encloses nothing, so the ring
    /// test never sees it and the iris showing under the lid follows the fur.
    /// Measured on the sitting frames the lids are 22-83px, 17-23 wide by 4-7
    /// tall, islands, and drawn as a pair whose rows match to within half a
    /// pixel. The mouth is just as flat but has no partner and sits about 12px
    /// lower; whiskers reach the silhouette so they are not islands.
    ///
    /// The mask itself needs nothing new -- growing off the lid picks the iris
    /// up, the same way it picks up the crescent beside a pupil.
    fn lidded_pair(
        &self,
        candidates: &[Vec<usize>],
        origin_x: usize,
        origin_y: usize,
        cell: (usize, usize),
    ) -> Vec<Vec<usize>> {
        let (cell_width, cell_height) = cell;
        let strokes: Vec<&Vec<usize>> = candidates
            .iter()
            .filter(|component| {
                if !(LID_SIZE_LOW..=LID_SIZE_HIGH).contains(&component.len()) {
                    return false;
                }
                let (x0, x1, y0, y1) = bounds(component, cell_width);
                let width = (x1 - x0 + 1) as f32;
                let height = (y1 - y0 + 1) as f32;
                if width / height < LID_FLATNESS {
                    return false;
                }
                if !enclosed(component, cell_width, cell_height).is_empty() {
                    return false;
                }
                !self.touches_air(component, origin_x, origin_y, cell, cell_width, cell_height)
            })
            .collect();

        let mut best: Option<(f32, usize, usize)> = None;
        for first in 0..strokes.len() {
            let (lx0, lx1, ly0, ly1) = bounds(strokes[first], cell_width);
            for second in (first + 1)..strokes.len() {
                let (rx0, rx1, ry0, ry1) = bounds(strokes[second], cell_width);
                if ((ly0 + ly1) as isize - (ry0 + ry1) as isize).abs() > 2 * LID_ROW_SLACK {
                    continue;
                }
                if ((lx1 - lx0) as isize - (rx1 - rx0) as isize).abs() > LID_WIDTH_SLACK {
                    continue;
                }
                let ratio = strokes[second].len() as f32 / strokes[first].len() as f32;
                if !(PARTNER_SIZE_LOW..=PARTNER_SIZE_HIGH).contains(&ratio) {
                    continue;
                }
                let beside = rx0 > lx1 + PARTNER_SIDE_GAP || rx1 + PARTNER_SIDE_GAP < lx0;
                if !beside {
                    continue;
                }
                let score = (ratio - 1.0).abs();
                if best.is_none() || score < best.as_ref().expect("checked").0 {
                    best = Some((score, first, second));
                }
            }
        }
        match best {
            Some((_, first, second)) => vec![strokes[first].clone(), strokes[second].clone()],
            None => Vec::new(),
        }
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
            REGION_BODY | REGION_ACCENT => targets.body != identity.body,
            REGION_EYE => targets.eye != identity.eye,
            // A seam lies between two regions, so either one moving moves it.
            REGION_SEAM => {
                targets.marking != identity.marking || targets.body != identity.body
            }
            _ => false,
        };
        // How far the body travelled, so the accent can travel with it.
        let middle = |aim: PaletteTargets| (aim.light_low + aim.light_high) / 2.0;
        let body_shift = middle(targets.body) - middle(identity.body);
        // And how much colour it kept. A blush that darkens with the body but
        // holds its own depth comes back as orange specks on a black cat --
        // there is still room for chroma at that lightness, and it shows. The
        // accent belongs to the body's colour family, so it fades with it.
        let body_depth = if identity.body.chroma > 0.0 {
            targets.body.chroma / identity.body.chroma
        } else {
            1.0
        };

        // Rank inside each region's own lightness order, not the raw value.
        // The marking is bunched low -- p5/p50/p95 of 16/30/53 -- so spreading
        // it by value leaves every colour darker than the answer cats. Rank
        // also carries an eye across intact: the pupil is the darkest thing in
        // it and the catchlight the lightest, whatever colour they end up.
        let mut ramps: [Vec<f32>; REGION_COUNT] = Default::default();
        for (index, pixel) in self.pixels.chunks_exact(4).enumerate() {
            let region = map.regions[index];
            let laid = region != REGION_ACCENT && region != REGION_SEAM;
            if pixel[3] > OPAQUE && laid && moved(region) {
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
            let colour = [pixel[0], pixel[1], pixel[2]];
            // The catchlight is not one of the eye's colours. It is a white dot
            // drawn on top of the iris, and every open eye on the sheet has one.
            // Running it through the eye's ramp gives it that eye's hue at the
            // top of its range -- a gold dot on a gold iris, which reads as no
            // dot at all. Measured: the black cat's white is inside the mask in
            // 54 of the 55 cells that have open eyes, and it is 2 to 16 pixels
            // depending on the frame, so the small ones vanished and the large
            // ones did not. That is the dot coming and going.
            let catchlight = region == REGION_EYE && lightness(colour) >= CREAM_LOW;
            if pixel[3] > OPAQUE && moved(region) && !catchlight {
                // How much of this pixel belongs to its band. The boundary
                // between marking and body is a gradient, so a hard threshold
                // repaints one pixel and skips the next and the seam comes out
                // as scattered dots -- the speckle along every edge. Fading the
                // last few lightness steps joins them back up.
                let weight = match region {
                    // A pixel touching the outline hands back to the outline
                    // over a much longer fade, because that is how wide the
                    // antialiasing was drawn. Applied to the whole band it
                    // would drag half the real fur back with it; gated on
                    // touching ink it costs none of it.
                    REGION_MARKING => band_weight(
                        lightness(colour),
                        MARKING_LOW,
                        MARKING_HIGH,
                        if map.edges[index] {
                            EDGE_FEATHER
                        } else {
                            BAND_FEATHER
                        },
                    ),
                    REGION_ACCENT => {
                        band_weight(lightness(colour), MARKING_HIGH, CREAM_LOW, BAND_FEATHER)
                    }
                    REGION_BODY => {
                        band_weight(lightness(colour), CREAM_LOW, CREAM_HIGH, BAND_FEATHER)
                    }
                    _ => 1.0,
                };
                // What a pixel becomes when it belongs to the body rather than
                // to a colour of its own: it keeps its hue, and follows the
                // body in lightness and depth. That is the accent, and it is
                // also what an outline blend wants -- staying put would leave a
                // pale speck sitting on a black cat.
                let as_body = || {
                    let light = (lightness(colour) + body_shift).clamp(0.0, 100.0);
                    from_hls(hue_of(colour), light, chroma_of(colour) * body_depth)
                };
                if region == REGION_ACCENT {
                    pixel[..3].copy_from_slice(&as_body());
                } else if region == REGION_SEAM {
                    // Rebuilt as the blend it is: the brightest fur at the end
                    // it leaves, the body at the end it arrives. Both ends join
                    // their neighbours exactly, because the brightest fur is
                    // where the marking's own ramp finishes.
                    let across = ((lightness(colour) - MARKING_HIGH)
                        / (CREAM_LOW - MARKING_HIGH))
                        .clamp(0.0, 1.0);
                    let brightest = from_hls(
                        targets.marking.hue_end,
                        targets.marking.light_high,
                        targets.marking.chroma,
                    );
                    pixel[..3].copy_from_slice(&mix(brightest, as_body(), across));
                } else {
                    let ramp = &ramps[region as usize];
                    if let (Some(aim), false) = (targets.targets(region), ramp.is_empty()) {
                        let position = place(ramp, lightness(colour));
                        let light = aim.light_low + position * (aim.light_high - aim.light_low);
                        let hue = aim.hue + position * (aim.hue_end - aim.hue);
                        let fresh = from_hls(hue, light, aim.chroma);
                        // A marking pixel that is part black outline and part
                        // cream body lands in the fur's lightness band by
                        // accident, and painting it fur colour scatters bright
                        // dots along every edge -- invisible on a brown cat,
                        // glaring on a yellow one. Such a blend is grey:
                        // measured over the sheet, pixels beside the ink sit at
                        // chroma 71 against fur's 95. Too close for a threshold,
                        // so it mixes: grey goes with the body, colour goes with
                        // the fur, and the two overlap smoothly.
                        let fresh = if region == REGION_MARKING {
                            mix(as_body(), fresh, colourfulness(chroma_of(colour)))
                        } else {
                            fresh
                        };
                        // The band's edge hands off to the neighbouring region,
                        // not back to the original colour. Fading towards what
                        // was there is only invisible when the new colour is
                        // near the old one -- on a black cat it left the
                        // original pale brown sitting in the seam.
                        pixel[..3].copy_from_slice(&mix(as_body(), fresh, weight));
                    }
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
    /// Marking pixels that touch the ink outline, one per pixel alongside
    /// `regions`. Not a sixth region: they are marking, and they take the
    /// marking's colour -- they just hand back to the outline over a longer
    /// fade than the rest of the band does.
    edges: Vec<bool>,
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

    pub fn accent_pixels(&self) -> usize {
        self.regions
            .iter()
            .filter(|region| **region == REGION_ACCENT)
            .count()
    }

    pub fn seam_pixels(&self) -> usize {
        self.regions
            .iter()
            .filter(|region| **region == REGION_SEAM)
            .count()
    }

    /// One byte per pixel, so a tool can paint the map and look at it. Counting
    /// the regions says a mask is the right size; only seeing it says the mask
    /// is in the right place, and the eye that leaked onto the eyelid counted
    /// perfectly normally.
    pub fn regions(&self) -> &[u8] {
        &self.regions
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
/// The blush, the nose, the inner ears -- and the shading that runs between
/// marking and body, which lives in the same lightness band and cannot be told
/// apart from them by colour (`docs/history/palette-making.md` §0).
///
/// It keeps its own hue and depth but follows the body's lightness, because
/// that is what it is: a highlight lying on the body. Leaving it still while
/// the body moves is what drew a glowing cream line around every dark edge of
/// the black cat.
const REGION_ACCENT: u8 = 4;
/// The gradient where the marking meets the body. Sits in the accent's
/// lightness band and is not an accent: it is a blend, and it gets rebuilt from
/// the two answers it lies between.
const REGION_SEAM: u8 = 5;
const REGION_COUNT: usize = 6;

/// How far a marking pixel has to be from any ink before an accent pixel beside
/// it counts as a seam. One step is not enough: the mouth and the whiskers are
/// ink strokes on the cream face, their antialiasing reaches two pixels out into
/// the marking band, and treating that as fur put a halo round the mouth.
const SEAM_INK_CLEAR: usize = 2;

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
/// And no eye is taller than this. A shape test, not a size one, and it catches
/// what the size test cannot.
///
/// Height rather than the longer side, because an eye is drawn wide and the
/// things that are not eyes are drawn tall. Measured over both sheets: every
/// real eye fits in 22, and where a whisker touches the rim and comes along
/// with it the pair reaches 27. Above that there is nothing real -- a whisker
/// running off on its own is 36, and the front paw on the stretching frame,
/// which is a ring round a cream pad and passes both the hole test and the area
/// test, is 68. Capping the longer side instead was tried first and took a real
/// eye with it: at r7c3 the far eye and a whisker are one ink blob 40 wide.
const EYE_SPAN_MAX: usize = 30;
/// A half-lidded eye's stroke: small, flat, and one of a matched pair.
const LID_SIZE_LOW: usize = 15;
const LID_SIZE_HIGH: usize = 150;
const LID_FLATNESS: f32 = 2.5;
const LID_ROW_SLACK: isize = 3;
const LID_WIDTH_SLACK: isize = 6;
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

fn chroma_of(colour: PaletteColour) -> f32 {
    let low = *colour.iter().min().expect("three channels") as f32;
    let high = *colour.iter().max().expect("three channels") as f32;
    high - low
}

/// Degrees, the ordinary hexagon. Grey has no hue and answers zero.
fn hue_of(colour: PaletteColour) -> f32 {
    let chroma = chroma_of(colour);
    if chroma == 0.0 {
        return 0.0;
    }
    let (red, green, blue) = (colour[0] as f32, colour[1] as f32, colour[2] as f32);
    let high = red.max(green).max(blue);
    let sixth = if high == red {
        ((green - blue) / chroma).rem_euclid(6.0)
    } else if high == green {
        (blue - red) / chroma + 2.0
    } else {
        (red - green) / chroma + 4.0
    };
    (sixth * 60.0).rem_euclid(360.0)
}

/// How much of a pixel's place on the ramp comes from its own lightness rather
/// than its rank among the others.
///
/// Rank alone flattens the histogram. Mochi's shading is gentle and bunched --
/// the marking's p5/p50/p95 are 17/29/53 -- so flattening it stretches those
/// small steps into visible bands, which is the graininess the recoloured cats
/// had. Value alone leaves everything bunched where it started and the colour
/// never arrives. Most of the way towards value, with enough rank left to keep
/// the median where the answer cats put it.
const RAMP_FROM_VALUE: f32 = 0.65;

/// Lightness units over which a band hands off to its neighbour.
const BAND_FEATHER: f32 = 5.0;

/// And over which the outline's antialiasing hands back to the outline.
///
/// Four times as wide, because the skirt is that wide -- 28% of it sits below
/// lightness 20 and 45% below 25, where the band's floor is 10. A yellow cat is
/// the worst case and not by tuning: saturated yellow cannot be dark, so its
/// marking has to start at lightness 50, and every one of those skirt pixels
/// was being lifted there. The line came out thin and broken because the weight
/// around it had been painted away.
const EDGE_FEATHER: f32 = 30.0;

/// Chroma at which a marking pixel is fully fur rather than an outline blend,
/// and where it is fully blend. Measured over the sheet: pixels beside the ink
/// run 17/48/71/98/173 at the five percentiles, fur 77/87/95/206/214.
const BLEND_CHROMA: f32 = 35.0;
const FUR_CHROMA: f32 = 80.0;

fn colourfulness(chroma: f32) -> f32 {
    ((chroma - BLEND_CHROMA) / (FUR_CHROMA - BLEND_CHROMA)).clamp(0.0, 1.0)
}

/// How much of a pixel belongs to its band, 0 at the edges and 1 inside.
/// `rising` is the fade at the dark end, which is the only one that ever needs
/// to be wider than the other.
fn band_weight(light: f32, low: f32, high: f32, rising: f32) -> f32 {
    let inside = ((light - low) / rising).min((high - light) / BAND_FEATHER);
    inside.clamp(0.0, 1.0)
}

/// Between two colours, `t` of the way to the second.
fn mix(from: PaletteColour, to: PaletteColour, t: f32) -> PaletteColour {
    let mut out = [0_u8; 3];
    for channel in 0..3 {
        let was = from[channel] as f32;
        out[channel] = (was + (to[channel] as f32 - was) * t).round() as u8;
    }
    out
}

/// Where `value` belongs on the ramp, as 0.0 to 1.0.
fn place(ramp: &[f32], value: f32) -> f32 {
    let low = ramp[0];
    let high = ramp[ramp.len() - 1];
    let spread = if high > low {
        ((value - low) / (high - low)).clamp(0.0, 1.0)
    } else {
        0.0
    };
    RAMP_FROM_VALUE * spread + (1.0 - RAMP_FROM_VALUE) * rank(ramp, value)
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

/// Can this pixel see out of the eye without crossing the rim?
///
/// Cream is the outside world -- the face and the belly are cream, and an eye
/// that grows into them has escaped. But **the catchlight is cream too**, and
/// that was the whole trouble: a gap in the rim is drawn in marking rather than
/// ink, and the marking pixel that would close the gap was refused for seeing
/// the very thing the eye is supposed to close around. So the rim stayed open,
/// the flood walked out through it, and the white was left on the face -- dark,
/// on a black cat.
///
/// Size does not separate the two: measured over both sheets, a cell's cream
/// runs to 2,221 for a face and 1,999 for the same cat's belly, and the biggest
/// blob that is not the largest in its cell is 2,889. **Reach does.** The face
/// and the belly run far past the eye; a catchlight is drawn inside it. So a
/// cream neighbour is the outside world only when its blob leaves the box the
/// rim draws.
fn sees_out(
    spot: usize,
    blob: &[usize],
    reach: &[(usize, usize, usize, usize)],
    eye: (usize, usize, usize, usize),
    width: usize,
    height: usize,
) -> bool {
    around(spot, width, height).into_iter().any(|neighbour| {
        let Some((left, right, top, bottom)) = blob.get(neighbour).and_then(|id| reach.get(*id))
        else {
            return false;
        };
        *left < eye.0 || *right > eye.1 || *top < eye.2 || *bottom > eye.3
    })
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
        for index in [0_usize, 1] {
            assert_eq!(
                recolored.pixels[index * 4..index * 4 + 3],
                plain.pixels[index * 4..index * 4 + 3],
                "pixel {index} should not have moved"
            );
        }
        assert_ne!(recolored.pixels[12..15], plain.pixels[12..15], "the body");
        // The blush is a highlight lying on the body, so it travels with it.
        // Holding it still is what drew a glowing cream line around every dark
        // edge of the black cat.
        assert_ne!(
            recolored.pixels[8..11],
            plain.pixels[8..11],
            "the blush should follow the body"
        );
    }

    #[test]
    fn a_recoloured_eye_keeps_its_pupil_dark_and_its_catchlight_bright() {
        // The 7x7 ring of the detection test, filled in the way a drawn eye is:
        // ink rim, a brown iris, and one white dot on top of it.
        let mut pixels = vec![[254, 242, 220, 255]; 49];
        for y in 1..6 {
            for x in 1..6 {
                let edge = x == 1 || x == 5 || y == 1 || y == 5;
                pixels[y * 7 + x] = if edge {
                    [5, 4, 4, 255]
                } else {
                    [120, 70, 40, 255]
                };
            }
        }
        pixels[3 * 7 + 3] = [250, 250, 250, 255];
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
        assert!(at(1, 1) < at(2, 2), "the rim should stay darker than the iris");
        assert!(at(1, 1) <= 12.0, "the rim should sit at the dark end");
        assert!(
            (20.0..=80.0).contains(&at(2, 2)),
            "the iris should take the eye's colour, inside the range asked for"
        );
        // And the white dot on top is not one of the eye's colours. Giving it
        // the eye's hue at the top of the ramp puts a gold dot on a gold iris,
        // which is the same as having no dot.
        let dot = (3 * 7 + 3) * 4;
        assert_eq!(
            recolored.pixels[dot..dot + 3],
            image.image().pixels[dot..dot + 3],
            "the catchlight should still be the white it was drawn"
        );
    }

    #[test]
    fn aiming_a_ramp_at_its_own_middle_leaves_it_where_it_was() {
        let was = IDENTITY.marking;
        let again = was.aimed_at(was.middle());
        assert!((again.hue - was.hue).abs() < 2.0, "{again:?}");
        assert!((again.light_low - was.light_low).abs() < 2.0, "{again:?}");
        assert!((again.light_high - was.light_high).abs() < 2.0, "{again:?}");
        assert!((again.chroma - was.chroma).abs() < 6.0, "{again:?}");
    }

    #[test]
    fn a_ramp_aimed_at_a_colour_keeps_its_width_even_at_the_ends() {
        let wide = PaletteTargets::new(22.0, 30.0, 90.0, 200.0);
        // White. The middle cannot sit at lightness 100 and still leave room
        // for the dark end, so the span slides rather than shrinking.
        let aimed = wide.aimed_at([255, 255, 255]);
        assert!((aimed.light_high - aimed.light_low - 60.0).abs() < 0.01, "{aimed:?}");
        assert!(aimed.light_high <= 100.0, "{aimed:?}");
        // And a sweep ends when one colour is asked for.
        let rainbow = PaletteTargets::sweeping(0.0, 359.0, 36.0, 86.0, 175.0);
        let aimed = rainbow.aimed_at([40, 80, 200]);
        assert_eq!(aimed.hue, aimed.hue_end, "{aimed:?}");
    }

    #[test]
    fn the_accent_darkens_with_the_body_but_keeps_its_own_colour() {
        // A blush at L 72.5, warm and saturated.
        let image = source(&[[251, 154, 119, 255], [254, 242, 220, 255]]);
        let map = image.region_map((0, 0));
        assert_eq!(map.accent_pixels(), 1);
        let night = Palette {
            // The black preset's body: from a mid of 90.5 down to 30.
            body: PaletteTargets::new(349.0, 20.0, 40.0, 7.0),
            ..IDENTITY
        };
        let recolored = image.recolored(&map, IDENTITY, night).expect("matching map");
        let blush = [recolored.pixels[0], recolored.pixels[1], recolored.pixels[2]];
        assert!(
            lightness(blush) < 20.0,
            "the blush should have gone dark with the body, not stayed a lamp: {blush:?}"
        );
        assert!(
            blush[0] > blush[2],
            "and it should still be warm rather than turned grey: {blush:?}"
        );
    }

    #[test]
    fn a_sweeping_hue_travels_with_the_shading() {
        // Three marking pixels, dark to light, so the ranks are 0, 0.5 and 1.
        let image = source(&[
            [60, 30, 15, 255],
            [115, 52, 27, 255],
            [200, 110, 50, 255],
        ]);
        let map = image.region_map((0, 0));
        assert_eq!(map.marking_pixels(), 3);
        let rainbow = Palette {
            marking: PaletteTargets::sweeping(0.0, 240.0, 40.0, 60.0, 120.0),
            ..IDENTITY
        };
        let recolored = image
            .recolored(&map, IDENTITY, rainbow)
            .expect("matching map");
        let at = |index: usize| {
            let start = index * 4;
            [
                recolored.pixels[start],
                recolored.pixels[start + 1],
                recolored.pixels[start + 2],
            ]
        };
        let (dark, light) = (at(0), at(2));
        assert!(dark[0] > dark[2], "the dark end should leave from red");
        assert!(light[2] > light[0], "the light end should arrive at blue");
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

    /// Two flat strokes side by side on the same row, the way a half-lidded
    /// eye is drawn. Neither encloses anything, so nothing but the pairing
    /// makes them eyes -- and without this the iris showing under the lid
    /// follows the fur, which is what the sitting animation was doing.
    fn lidded(strokes: &[(usize, usize)]) -> PetImageSource {
        let (width, height) = (40usize, 20usize);
        let mut pixels = vec![[254, 242, 220, 255]; width * height];
        for (from, to) in strokes {
            for x in *from..*to {
                pixels[9 * width + x] = [5, 4, 4, 255];
                pixels[10 * width + x] = [5, 4, 4, 255];
            }
        }
        PetImageSource {
            width,
            height,
            pixels: pixels.iter().flatten().copied().collect(),
        }
    }

    #[test]
    fn a_matched_pair_of_lid_strokes_is_a_shut_eye() {
        let image = lidded(&[(6, 18), (24, 36)]);
        let map = image.region_map((40, 20));
        assert_eq!(map.eye_pixels(), 48, "both lids and nothing else");
    }

    #[test]
    fn one_lid_stroke_on_its_own_is_not_an_eye() {
        // The mouth is drawn just as flat. What it does not have is a partner.
        let image = lidded(&[(6, 18)]);
        let map = image.region_map((40, 20));
        assert_eq!(map.eye_pixels(), 0);
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
