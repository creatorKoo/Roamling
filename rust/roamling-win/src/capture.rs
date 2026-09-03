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
//! `docs/windows.md` section 5 picked `BitBlt` over Windows Graphics Capture:
//! WinRT is painful from Rust and this is one small readback every few seconds.
//! The pet is excluded from its own capture by `WDA_EXCLUDEFROMCAPTURE`, which
//! W0 verified by control experiment -- it must not make its own seat look busy.

use roamling_core::{DisplaySnapshot, LuminanceField};
use windows::Win32::System::StationsAndDesktops::{
    CloseDesktop, OpenInputDesktop, DESKTOP_READOBJECTS, DESKTOP_CONTROL_FLAGS,
};
use windows::Win32::Graphics::Gdi::{
    BitBlt, CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, GetDC, ReleaseDC,
    SelectObject, SetStretchBltMode, StretchBlt, BITMAPINFO, BITMAPINFOHEADER, DIB_RGB_COLORS,
    HALFTONE, HBITMAP, HDC, SRCCOPY,
};

/// The macOS provider's number, so both platforms hand the scorer the same
/// shape of field. `MacCaptureProvider.sampleColumns`.
const COLUMNS: usize = 64;

pub struct Capture {
    pub field: Option<LuminanceField>,
    /// Reading the framebuffer, which goes through the display driver.
    pub read_ms: f64,
    /// Averaging it down, which is local memory.
    pub shrink_ms: f64,
}

fn dib(dc: HDC, width: i32, height: i32) -> Option<(HBITMAP, *mut u8)> {
    let info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: width,
            // Negative is top-down, which is the order the field wants.
            biHeight: -height,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: 0,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut bits: *mut std::ffi::c_void = std::ptr::null_mut();
    let bitmap = unsafe { CreateDIBSection(dc, &info, DIB_RGB_COLORS, &mut bits, None, 0) }.ok()?;
    Some((bitmap, bits as *mut u8))
}

pub fn luminance(display: &DisplaySnapshot) -> Capture {
    let mut result = Capture {
        field: None,
        read_ms: 0.0,
        shrink_ms: 0.0,
    };
    let width = display.frame.size.width as i32;
    let height = display.frame.size.height as i32;
    if width < 1 || height < 1 {
        return result;
    }
    let rows = ((COLUMNS as f64 * (height as f64 / width as f64)).round() as usize).max(2) as i32;

    unsafe {
        let screen = GetDC(None);
        if screen.is_invalid() {
            return result;
        }

        // Two steps on purpose. Reading the framebuffer and averaging it down
        // are different costs, and asking one `StretchBlt` from the screen to
        // do both makes GDI average driver-side: that measured 300-800 ms,
        // which is a visible stall because this runs on the message loop.
        // Split, the average happens on local memory instead.
        let full_dc = CreateCompatibleDC(screen);
        let small_dc = CreateCompatibleDC(screen);
        let full = dib(full_dc, width, height);
        let small = dib(small_dc, COLUMNS as i32, rows);

        if let (Some((full_bitmap, _)), Some((small_bitmap, small_bits))) = (full, small) {
            SelectObject(full_dc, full_bitmap);
            SelectObject(small_dc, small_bitmap);

            let at = std::time::Instant::now();
            let read = BitBlt(
                full_dc,
                0,
                0,
                width,
                height,
                screen,
                display.frame.origin.x as i32,
                display.frame.origin.y as i32,
                SRCCOPY,
            )
            .is_ok();
            result.read_ms = at.elapsed().as_secs_f64() * 1000.0;

            if read {
                let at = std::time::Instant::now();
                // HALFTONE averages the pixels it is skipping. The default mode
                // picks one pixel per destination cell, and a page of text
                // sampled that way reads as blank -- the very thing this is
                // here to notice.
                SetStretchBltMode(small_dc, HALFTONE);
                let shrunk = StretchBlt(
                    small_dc, 0, 0, COLUMNS as i32, rows, full_dc, 0, 0, width, height, SRCCOPY,
                )
                .as_bool();
                result.shrink_ms = at.elapsed().as_secs_f64() * 1000.0;

                if shrunk {
                    let pixels =
                        std::slice::from_raw_parts(small_bits, COLUMNS * rows as usize * 4);
                    result.field = Some(LuminanceField {
                        bounds: display.frame,
                        columns: COLUMNS,
                        rows: rows as usize,
                        samples: pixels.chunks_exact(4).map(sample).collect(),
                    });
                }
            }
            let _ = DeleteObject(full_bitmap);
            let _ = DeleteObject(small_bitmap);
        }

        let _ = DeleteDC(full_dc);
        let _ = DeleteDC(small_dc);
        ReleaseDC(None, screen);
    }
    result
}

/// Rec. 709, because the screen is sRGB and that is its luminance. macOS
/// reaches the same place through a `DeviceGray` context; the two will not
/// agree bit for bit, and nothing requires them to -- the differential fixture
/// takes a field as *input* and only pins the scoring done with it.
fn sample(pixel: &[u8]) -> f64 {
    let b = pixel[0] as f64;
    let g = pixel[1] as f64;
    let r = pixel[2] as f64;
    (0.2126 * r + 0.7152 * g + 0.0722 * b) / 255.0
}

/// Whether there is anything to look at.
///
/// A locked workstation switches the input desktop to Winlogon's secure one,
/// which this process cannot open. `BitBlt` against the screen there does not
/// fail -- it succeeds and returns black. That is the dangerous shape: the pet
/// would read a whole empty screen, take a seat chosen on nothing, and keep
/// that field until the next capture. The user locks the screen often enough
/// for this to be the normal case rather than an edge one.
///
/// Also true while the UAC prompt owns the desktop, which is the same story.
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

