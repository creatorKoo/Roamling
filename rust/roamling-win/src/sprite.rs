// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

//! The layered window's pixels: one atlas cell, sampled to the size the
//! monitor wants, pushed with the window position in a single call.
//!
//! The sheet is a 2x asset -- a 192x208 cell drawn into a 96x104 footprint --
//! and it is pixel art, so it is sampled **nearest neighbour**, matching
//! `NSImageInterpolation.none` on the macOS panel. Anything smoother turns the
//! authored edges to mush, and `docs/art/mochi-animation-handoff.md` treats
//! those edges as the character's identity.

use roamling_pet::{FrameRect, PetImage};
use windows::Win32::Foundation::{COLORREF, HWND, POINT, SIZE};
use windows::Win32::Graphics::Gdi::{
    CreateCompatibleDC, CreateDIBSection, DeleteDC, SelectObject, AC_SRC_ALPHA, AC_SRC_OVER,
    BITMAPINFO, BITMAPINFOHEADER, BLENDFUNCTION, DIB_RGB_COLORS, HDC,
};
use windows::Win32::UI::WindowsAndMessaging::{UpdateLayeredWindow, ULW_ALPHA};

pub struct Surface {
    dc: HDC,
    bits: *mut u8,
    pub width: i32,
    pub height: i32,
}

impl Surface {
    /// A 32bpp top-down DIB, reused across frames. Only the pixels change when
    /// the pet does; the surface is rebuilt only when the size does.
    pub fn new(width: i32, height: i32) -> Option<Self> {
        if width <= 0 || height <= 0 {
            return None;
        }
        unsafe {
            let dc = CreateCompatibleDC(None);
            let info = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: width,
                    // Negative is top-down, which is the order the sheet is in.
                    biHeight: -height,
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: 0,
                    ..Default::default()
                },
                ..Default::default()
            };
            let mut bits: *mut std::ffi::c_void = std::ptr::null_mut();
            let bitmap = CreateDIBSection(dc, &info, DIB_RGB_COLORS, &mut bits, None, 0).ok()?;
            SelectObject(dc, bitmap);
            Some(Self {
                dc,
                bits: bits as *mut u8,
                width,
                height,
            })
        }
    }

    /// Copy one atlas cell in, sampling nearest neighbour.
    ///
    /// The sheet is already premultiplied RGBA; the DIB wants premultiplied
    /// BGRA, so this is a channel swap and nothing more -- no blending, no
    /// gamma, nothing that could shift an authored pixel.
    pub fn draw_frame(&mut self, sheet: &PetImage, rect: FrameRect) {
        let destination = unsafe {
            std::slice::from_raw_parts_mut(self.bits, (self.width * self.height * 4) as usize)
        };
        let stride = sheet.width * 4;
        for y in 0..self.height {
            // Integer mapping, so the same destination row always takes the
            // same source row: a rounded one would shimmer as the pet walks.
            let source_y = rect.y + (y as usize * rect.height) / self.height as usize;
            for x in 0..self.width {
                let source_x = rect.x + (x as usize * rect.width) / self.width as usize;
                let from = source_y * stride + source_x * 4;
                let to = ((y * self.width + x) * 4) as usize;
                destination[to] = sheet.pixels[from + 2]; // B
                destination[to + 1] = sheet.pixels[from + 1]; // G
                destination[to + 2] = sheet.pixels[from]; // R
                destination[to + 3] = sheet.pixels[from + 3]; // A
            }
        }
    }

    /// Move the window and blend the bitmap in one call, so the pet never tears
    /// between where it is and what it looks like.
    pub fn present(&self, hwnd: HWND, corner: POINT) {
        let size = SIZE {
            cx: self.width,
            cy: self.height,
        };
        let source = POINT { x: 0, y: 0 };
        let blend = BLENDFUNCTION {
            BlendOp: AC_SRC_OVER as u8,
            BlendFlags: 0,
            SourceConstantAlpha: 255,
            AlphaFormat: AC_SRC_ALPHA as u8,
        };
        unsafe {
            let _ = UpdateLayeredWindow(
                hwnd,
                None,
                Some(&corner),
                Some(&size),
                self.dc,
                Some(&source),
                COLORREF(0),
                Some(&blend),
                ULW_ALPHA,
            );
        }
    }
}

impl Drop for Surface {
    fn drop(&mut self) {
        unsafe {
            let _ = DeleteDC(self.dc);
        }
    }
}
