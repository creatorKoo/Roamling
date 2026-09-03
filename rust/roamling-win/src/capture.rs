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
fn average(pixels: &[u8], width: usize, height: usize, stride: usize, rows: usize) -> Vec<f64> {
    let mut samples = Vec::with_capacity(COLUMNS * rows);
    for row in 0..rows {
        let y0 = row * height / rows;
        let y1 = (((row + 1) * height) / rows).max(y0 + 1).min(height);
        for column in 0..COLUMNS {
            let x0 = column * width / COLUMNS;
            let x1 = (((column + 1) * width) / COLUMNS).max(x0 + 1).min(width);
            // Sixteen samples per axis is plenty to tell dense from empty, and
            // it stops the cost growing with the monitor: a cell on this screen
            // is about forty pixels across, on a 4K one it is eighty, and both
            // end up doing the same work. Averaging every pixel measured 51 ms.
            let step = (x1 - x0).max(y1 - y0).div_ceil(16).max(1);
            let mut total = 0u64;
            let mut count = 0u64;
            for y in (y0..y1).step_by(step) {
                let line = y * stride;
                for x in (x0..x1).step_by(step) {
                    let at = line + x * 4;
                    if at + 2 >= pixels.len() {
                        continue;
                    }
                    // BGRA, and Rec. 709 because the screen is sRGB. macOS
                    // reaches the same place through a DeviceGray context; the
                    // two will not agree bit for bit and nothing requires them
                    // to -- the differential fixture takes a field as *input*
                    // and only pins the scoring done with it.
                    total += (0.2126 * pixels[at + 2] as f64
                        + 0.7152 * pixels[at + 1] as f64
                        + 0.0722 * pixels[at] as f64) as u64;
                    count += 1;
                }
            }
            samples.push(if count == 0 {
                0.0
            } else {
                total as f64 / count as f64 / 255.0
            });
        }
    }
    samples
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
