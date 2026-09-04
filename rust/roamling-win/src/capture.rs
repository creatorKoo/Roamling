// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

//! A downsampled luminance view of one display, so the pet can tell a busy
//! part of the screen from an empty one.
//!
//! It does not read the screen. Sixty-four columns across a whole monitor is
//! roughly forty pixels per sample -- enough to see that a region is dense,
//! nowhere near enough to see what it says. No OCR, no image ever written
//! anywhere. `docs/mvp.md` MVP 4.
//!
//! The frame comes from Desktop Duplication rather than `BitBlt`. See
//! `duplication.rs` for why, and `docs/windows.md` section 5 for the numbers
//! that made the original plan wrong.

use crate::duplication::Duplication;
use roamling_core::{DisplaySnapshot, LuminanceField};
use windows::Win32::System::StationsAndDesktops::{
    CloseDesktop, OpenInputDesktop, DESKTOP_CONTROL_FLAGS, DESKTOP_READOBJECTS,
};

/// The macOS provider's number, so both platforms hand the scorer the same
/// shape of field. `MacCaptureProvider.sampleColumns`.
const COLUMNS: usize = 64;

pub struct Capture {
    pub field: Option<LuminanceField>,
    pub read_ms: f64,
    pub shrink_ms: f64,
    /// The screen had not been drawn to since the last look, so the previous
    /// answer still stands. Most calls end here and cost almost nothing.
    pub unchanged: bool,
}

/// Holds the duplication open between captures, because opening it is the
/// expensive part and `AcquireNextFrame` only answers for one already held.
#[derive(Default)]
pub struct Capturer {
    duplication: Option<Duplication>,
    opened_for: String,
}

impl Capturer {
    pub fn luminance(&mut self, display: &DisplaySnapshot) -> Capture {
        let mut result = Capture {
            field: None,
            read_ms: 0.0,
            shrink_ms: 0.0,
            unchanged: false,
        };
        if !screen_is_readable() {
            return result;
        }
        if self.opened_for != display.id || self.duplication.is_none() {
            self.duplication = Duplication::open(&display.id);
            self.opened_for = display.id.clone();
        }
        let Some(duplication) = self.duplication.as_mut() else {
            return result;
        };

        let rows = ((COLUMNS as f64 * (display.frame.size.height / display.frame.size.width))
            .round() as usize)
            .max(2);
        let mut samples: Option<Vec<f64>> = None;
        let mut shrink_ms = 0.0;

        let at = std::time::Instant::now();
        let outcome = duplication.next_frame(|pixels, frame| {
            let started = std::time::Instant::now();
            samples = Some(average(pixels, frame.width, frame.height, frame.stride, rows));
            shrink_ms = started.elapsed().as_secs_f64() * 1000.0;
        });
        result.read_ms = (at.elapsed().as_secs_f64() * 1000.0 - shrink_ms).max(0.0);
        result.shrink_ms = shrink_ms;

        match outcome {
            Ok(true) => {
                result.field = samples.map(|samples| LuminanceField {
                    bounds: display.frame,
                    columns: COLUMNS,
                    rows,
                    samples,
                });
            }
            Ok(false) => result.unchanged = true,
            Err(()) => {
                // Gone for good rather than merely lost; drop it so the next
                // call opens a fresh one.
                self.duplication = None;
            }
        }
        result
    }
}

/// Box-average the frame down to the sample grid.
///
/// Averaging rather than picking one pixel per cell is the whole point: a page
/// of text point-sampled reads as blank, which is exactly the thing this is
/// here to notice.
///
/// **Every pixel counts, and that is a deliberate reversal.** This used to take
/// at most sixteen samples per axis per cell, on the reasoning that sixteen is
/// plenty to tell dense from empty. It is not. A cell on a 2560-wide screen is
/// forty pixels across, so the cap stepped by three and looked at one pixel in
/// nine -- and a one-pixel antialiased stroke has two chances in three of being
/// stepped straight over. Faint text is exactly the case made of thin strokes
/// and low contrast, and the pet sat on it. macOS never had this problem: it
/// asks ScreenCaptureKit for a 64-column image and the system scaler reads
/// every pixel.
///
/// The cost of reading all of them is paid back by doing integer work per
/// pixel. The 51 ms that justified the cap was floating-point per pixel with a
/// cast; weights scaled to 1024 make it a multiply-add on bytes.
fn average(pixels: &[u8], width: usize, height: usize, stride: usize, rows: usize) -> Vec<f64> {
    // Rec. 709, scaled so the three sum to exactly 1024 -- the screen is sRGB
    // and this is the same weighting macOS reaches through a DeviceGray
    // context. The two will not agree bit for bit and nothing requires them to:
    // the differential fixture takes a field as *input* and pins only the
    // scoring done with it.
    const BLUE: u64 = 74;
    const GREEN: u64 = 732;
    const RED: u64 = 218;
    const UNIT: u64 = 1_024;

    if width == 0 || height == 0 || rows == 0 {
        return vec![0.0; COLUMNS * rows];
    }

    // Which column each source x falls in, worked out once rather than as a
    // division inside the inner loop.
    let column_for: Vec<usize> = (0..width).map(|x| (x * COLUMNS / width).min(COLUMNS - 1)).collect();

    // A screen far beyond 4K would make reading every row cost more than the
    // answer is worth. Skipping rows keeps the horizontal resolution, which is
    // the axis that catches the vertical stems letters are mostly made of.
    let row_step = (height * width).div_ceil(4_000_000).max(1);

    let mut totals = vec![0u64; COLUMNS * rows];
    let mut counts = vec![0u64; COLUMNS * rows];

    for y in (0..height).step_by(row_step) {
        let target = (y * rows / height).min(rows - 1) * COLUMNS;
        let line = y * stride;
        if line + width * 4 > pixels.len() {
            continue;
        }
        let row = &pixels[line..line + width * 4];
        for (x, pixel) in row.chunks_exact(4).enumerate() {
            let at = target + column_for[x];
            totals[at] += BLUE * pixel[0] as u64
                + GREEN * pixel[1] as u64
                + RED * pixel[2] as u64;
            counts[at] += 1;
        }
    }

    totals
        .iter()
        .zip(counts.iter())
        .map(|(total, count)| {
            if *count == 0 {
                0.0
            } else {
                *total as f64 / (*count as f64 * UNIT as f64 * 255.0)
            }
        })
        .collect()
}

/// The statistic the seat scorer actually uses, over the whole field.
///
/// `VisualEmptiness` scores a region by the mean difference between
/// neighbouring cells, against a reference of 0.02 -- a figure calibrated on
/// macOS against rendered terminal output. Printing the same number here is the
/// only way to tell a field that is too flat to judge from a scorer that is
/// judging it wrongly.
pub fn mean_gradient(field: &roamling_core::LuminanceField) -> f64 {
    let mut total = 0.0;
    let mut count = 0usize;
    for row in 0..field.rows {
        for column in 0..field.columns {
            let Some(value) = field.sample(column as i64, row as i64) else { continue };
            if column + 1 < field.columns {
                if let Some(right) = field.sample(column as i64 + 1, row as i64) {
                    total += (right - value).abs();
                    count += 1;
                }
            }
            if row + 1 < field.rows {
                if let Some(below) = field.sample(column as i64, row as i64 + 1) {
                    total += (below - value).abs();
                    count += 1;
                }
            }
        }
    }
    if count == 0 {
        0.0
    } else {
        total / count as f64
    }
}

/// Whether there is anything to look at.
///
/// A locked workstation switches the input desktop to Winlogon's secure one.
/// Desktop Duplication reports that honestly by losing the duplication, but
/// this is checked first anyway: it costs nothing and it keeps the pet from
/// rebuilding a duplication it cannot use, over and over, while the machine
/// sits locked. Also true while a UAC prompt owns the desktop.
pub fn screen_is_readable() -> bool {
    unsafe {
        match OpenInputDesktop(DESKTOP_CONTROL_FLAGS(0), false, DESKTOP_READOBJECTS) {
            Ok(desktop) => {
                let _ = CloseDesktop(desktop);
                true
            }
            Err(_) => false,
        }
    }
}

impl Capturer {
    /// Let go of the duplication.
    ///
    /// Unlike `BitBlt`, this path holds a D3D11 device and an open duplication
    /// between captures -- that is what makes it fast. The cost is a GPU
    /// resource kept alive in a process that runs all day, which may be enough
    /// to stop the GPU idling all the way down. Whether it actually is has not
    /// been measured. Releasing it the moment the user turns capture off is
    /// cheap either way, and reopening costs only the next capture.
    pub fn release(&mut self) {
        self.duplication = None;
        self.opened_for.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A BGRA frame of white paper with thin, faint vertical strokes across the
    /// left half -- one pixel wide, every third pixel, at the contrast of grey
    /// text on white. The right half is bare.
    fn page(width: usize, height: usize) -> Vec<u8> {
        let mut pixels = vec![0xFFu8; width * height * 4];
        for y in 0..height {
            for x in (0..width / 2).step_by(3) {
                let at = (y * width + x) * 4;
                // 190 on 255: light grey, the kind of text this kept missing.
                pixels[at] = 190;
                pixels[at + 1] = 190;
                pixels[at + 2] = 190;
            }
        }
        pixels
    }

    /// The defect this replaced: stepping through a cell can land entirely
    /// between one-pixel strokes, so a page of faint text reads as bare paper
    /// and the pet sits on it. Reading every pixel cannot miss them.
    #[test]
    fn faint_thin_text_is_not_stepped_over() {
        let (width, height) = (COLUMNS * 40, 40 * 8);
        let pixels = page(width, height);
        let samples = average(&pixels, width, height, width * 4, 8);
        assert_eq!(samples.len(), COLUMNS * 8);

        // One stroke in three columns of pixels, at 190 against 255:
        // (190 + 255 + 255) / 3 = 233.3, so about 0.915 of full white.
        let inked = samples[0];
        assert!(
            (inked - 233.0 / 255.0).abs() < 0.01,
            "the written half averaged {inked}, not the ink it actually has"
        );

        // The bare half is white, and the two halves have to be far enough
        // apart that a seat chooser can tell them apart at all.
        let bare = samples[COLUMNS - 1];
        assert!((bare - 1.0).abs() < 1e-6, "the bare half averaged {bare}");
        assert!(
            bare - inked > 0.05,
            "written and bare paper differ by only {}",
            bare - inked
        );
    }

    /// The instrument that found the black-frame bug, so it gets its own guard:
    /// a flat field has no gradient at all, and the seat scorer reads exactly
    /// that as a perfectly empty screen.
    #[test]
    fn a_flat_field_has_no_gradient_and_a_checkerboard_has_all_of_it() {
        let flat = roamling_core::LuminanceField::new(
            roamling_core::WorldRect::new(0.0, 0.0, 100.0, 100.0),
            4,
            4,
            vec![0.0; 16],
        )
        .expect("field");
        assert_eq!(mean_gradient(&flat), 0.0);

        let checker: Vec<f64> = (0..16).map(|i| ((i / 4 + i % 4) % 2) as f64).collect();
        let busy = roamling_core::LuminanceField::new(
            roamling_core::WorldRect::new(0.0, 0.0, 100.0, 100.0),
            4,
            4,
            checker,
        )
        .expect("field");
        assert_eq!(mean_gradient(&busy), 1.0);
    }

    /// The old cap, reproduced, to show it was not a theoretical loss: stepping
    /// by three over strokes drawn every three pixels can see none of them.
    #[test]
    fn the_sixteen_sample_cap_could_miss_every_stroke() {
        let (width, height) = (COLUMNS * 40, 40 * 8);
        let pixels = page(width, height);
        let cell = width / COLUMNS;
        let step = cell.div_ceil(16).max(1);
        assert_eq!(step, 3, "the cap stepped by three on a screen this wide");

        // Start one pixel in, which is where a cell boundary lands for many
        // columns, and every sample falls between the strokes.
        let mut total = 0u64;
        let mut count = 0u64;
        for y in (0..40).step_by(step) {
            for x in (1..cell).step_by(step) {
                let at = (y * width + x) * 4;
                total += pixels[at] as u64;
                count += 1;
            }
        }
        let seen = total as f64 / count as f64 / 255.0;
        assert!(
            seen > 0.99,
            "the cap saw {seen}; if it saw the ink this test is no longer the point"
        );

        // And what is actually there is meaningfully darker.
        let truth = average(&pixels, width, height, width * 4, 8)[0];
        assert!(
            seen - truth > 0.05,
            "stepping lost only {}, which would not have mattered",
            seen - truth
        );
    }
}
