// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

//! The layered window's pixels: one atlas cell, sampled to the size the
//! monitor wants, pushed with the window position in a single call.
//!
//! The sheet is a 2x asset -- a 192x208 cell drawn into a 96x104 footprint --
//! and it is pixel art, so at the authored size and above it is sampled
//! **nearest neighbour**, matching `NSImageInterpolation.none` on the macOS
//! panel. Anything smoother turns the authored edges to mush, and
//! `docs/art/mochi-animation-handoff.md` treats those edges as the character's
//! identity.
//!
//! Below the authored size that reasoning runs out: nearest neighbour keeps one
//! source pixel in several and drops the rest, so outlines break up. The sizes
//! under 1.0x average the pixels each destination pixel covers instead (user
//! decision 2026-09-19, R19); macOS does the same with `.high`.

use roamling_pet::{FrameRect, PetImage};
use windows::Win32::Foundation::{COLORREF, HWND, POINT, SIZE};
use windows::Win32::Graphics::Gdi::{
    CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, SelectObject, HBITMAP, HGDIOBJ, AC_SRC_ALPHA, AC_SRC_OVER,
    BITMAPINFO, BITMAPINFOHEADER, BLENDFUNCTION, DIB_RGB_COLORS, HDC,
};
use windows::Win32::UI::WindowsAndMessaging::{UpdateLayeredWindow, ULW_ALPHA};

pub struct Surface {
    dc: HDC,
    bitmap: HBITMAP,
    previous_bitmap: HGDIOBJ,
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
            let bitmap = match CreateDIBSection(dc, &info, DIB_RGB_COLORS, &mut bits, None, 0) {
                Ok(bitmap) => bitmap,
                Err(_) => { let _ = DeleteDC(dc); return None; }
            };
            let previous_bitmap = SelectObject(dc, bitmap);
            Some(Self {
                dc,
                bitmap,
                previous_bitmap,
                bits: bits as *mut u8,
                width,
                height,
            })
        }
    }

    /// Copy one atlas cell in: nearest neighbour, or an area average when the
    /// user has shrunk the pet.
    pub fn draw_frame(&mut self, sheet: &PetImage, rect: FrameRect, smooth: bool) {
        let destination = unsafe {
            std::slice::from_raw_parts_mut(self.bits, (self.width * self.height * 4) as usize)
        };
        sample(sheet, rect, self.width, self.height, smooth, destination);
    }
    pub fn draw_effects(&mut self, frames: &[roamling_core::effects::EffectFrame]) {
        let pixels = unsafe {
            std::slice::from_raw_parts_mut(self.bits, (self.width * self.height * 4) as usize)
        };
        paint_effects(frames, self.width as usize, self.height as usize, pixels);
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
            SelectObject(self.dc, self.previous_bitmap);
            let _ = DeleteObject(self.bitmap);
            let _ = DeleteDC(self.dc);
        }
    }
}


/// Fill common-core polygons into premultiplied BGRA. Four coverage samples
/// keep small hearts smooth without touching the character's pixel sampling.
fn paint_effects(frames: &[roamling_core::effects::EffectFrame], width: usize, height: usize, pixels: &mut [u8]) {
    pixels.fill(0);
    let body_width = width as f64 / 2.0;
    for frame in frames {
        if frame.points.len() < 3 || frame.opacity <= 0.0 { continue; }
        let points: Vec<(f64, f64)> = frame.points.iter().map(|p|
            (body_width + p.x * body_width, height as f64 * 0.75 + p.y * body_width)
        ).collect();
        let left = points.iter().map(|p| p.0).fold(f64::INFINITY, f64::min).floor().max(0.0) as usize;
        let top = points.iter().map(|p| p.1).fold(f64::INFINITY, f64::min).floor().max(0.0) as usize;
        let right = points.iter().map(|p| p.0).fold(0.0, f64::max).ceil().min(width as f64) as usize;
        let bottom = points.iter().map(|p| p.1).fold(0.0, f64::max).ceil().min(height as f64) as usize;
        for y in top..bottom {
            for x in left..right {
                let mut coverage = 0;
                for dy in [0.25, 0.75] {
                    for dx in [0.25, 0.75] {
                        if polygon_contains(&points, x as f64 + dx, y as f64 + dy) { coverage += 1; }
                    }
                }
                if coverage == 0 { continue; }
                let alpha = (frame.opacity.clamp(0.0, 1.0) * coverage as f64 * 255.0 / 4.0).round() as u32;
                let offset = (y * width + x) * 4;
                for (channel, colour) in [frame.blue, frame.green, frame.red, 255].into_iter().enumerate() {
                    pixels[offset + channel] = ((colour as u32 * alpha
                        + pixels[offset + channel] as u32 * (255 - alpha) + 127) / 255) as u8;
                }
            }
        }
    }
}

fn polygon_contains(points: &[(f64, f64)], x: f64, y: f64) -> bool {
    let mut inside = false;
    let mut previous = points[points.len() - 1];
    for &next in points {
        if (next.1 > y) != (previous.1 > y)
            && x < (previous.0 - next.0) * (y - next.1) / (previous.1 - next.1) + next.0
        {
            inside = !inside;
        }
        previous = next;
    }
    inside
}

#[cfg(test)]
mod effect_tests {
    use super::*;

    /// Opt-in artifact made by the production sprite and effect compositors.
    #[test]
    fn preview_pet_with_hearts() {
        let Ok(path) = std::env::var("ROAMLING_EFFECTS_PREVIEW") else { return; };
        let asset = roamling_pet::built_in_mochi().unwrap();
        let rect = asset.frame_rect(50).unwrap();
        let mut pet = vec![0; 96 * 104 * 4];
        sample(asset.sheet(rect.sheet).unwrap(), rect, 96, 104, false, &mut pet);
        let mut engine = roamling_core::effects::EffectSystem::default();
        for _ in 0..48 { engine.update(1.0 / 30.0, true, 1.0); }
        let mut overlay = vec![0; 192 * 208 * 4];
        paint_effects(&engine.frames(), 192, 208, &mut overlay);
        let mut canvas = vec![0u8; 192 * 208 * 4];
        for y in 0..208 {
            for x in 0..192 {
                let i = (y * 192 + x) * 4;
                canvas[i..i + 4].copy_from_slice(&[240, 243, 246, 255]);
                if (48..144).contains(&x) && y >= 104 {
                    let src = ((y - 104) * 96 + x - 48) * 4;
                    let alpha = pet[src + 3] as u32;
                    for c in 0..3 {
                        canvas[i + c] = (pet[src + c] as u32
                            + (canvas[i + c] as u32 * (255 - alpha) + 127) / 255) as u8;
                    }
                }
                let alpha = overlay[i + 3] as u32;
                for c in 0..3 {
                    canvas[i + c] = (overlay[i + c] as u32
                        + (canvas[i + c] as u32 * (255 - alpha) + 127) / 255) as u8;
                }
            }
        }
        // Top-down 32-bit BMP, written without any image-library dependency.
        let mut bmp = Vec::new();
        bmp.extend_from_slice(b"BM");
        bmp.extend_from_slice(&(54u32 + canvas.len() as u32).to_le_bytes());
        bmp.extend_from_slice(&[0; 4]);
        bmp.extend_from_slice(&54u32.to_le_bytes());
        bmp.extend_from_slice(&40u32.to_le_bytes());
        bmp.extend_from_slice(&192i32.to_le_bytes());
        bmp.extend_from_slice(&(-208i32).to_le_bytes());
        bmp.extend_from_slice(&1u16.to_le_bytes());
        bmp.extend_from_slice(&32u16.to_le_bytes());
        bmp.extend_from_slice(&[0; 24]);
        bmp.extend_from_slice(&canvas);
        std::fs::write(path, bmp).unwrap();
    }


    #[test]
    fn particles_have_transparent_padding_and_premultiplied_edges() {
        let mut engine = roamling_core::effects::EffectSystem::default();
        for _ in 0..20 { engine.update(1.0 / 30.0, true, 1.0); }
        let mut pixels = vec![0; 192 * 208 * 4];
        paint_effects(&engine.frames(), 192, 208, &mut pixels);
        assert!(pixels.chunks_exact(4).any(|p| p[3] > 0));
        assert!(pixels.chunks_exact(4).all(|p| p[0] <= p[3] && p[1] <= p[3] && p[2] <= p[3]));
        assert!(pixels[..192 * 4].iter().all(|p| *p == 0));
        paint_effects(&[], 192, 208, &mut pixels);
        assert!(pixels.iter().all(|p| *p == 0));
    }
}

/// One atlas cell into a `width` x `height` BGRA buffer.
///
/// The sheet is already premultiplied RGBA and the DIB wants premultiplied
/// BGRA. Unsmoothed this is a channel swap and nothing more -- no blending, no
/// gamma, nothing that could shift an authored pixel. Smoothed, each destination
/// pixel is the mean of the source pixels it covers, which is the correct
/// average because the channels are premultiplied.
fn sample(sheet: &PetImage, rect: FrameRect, width: i32, height: i32, smooth: bool, destination: &mut [u8]) {
    let stride = sheet.width * 4;
    let (width, height) = (width as usize, height as usize);
    for y in 0..height {
        // Integer mapping, so the same destination row always takes the
        // same source rows: a rounded one would shimmer as the pet walks.
        let top = (y * rect.height) / height;
        let bottom = if smooth { (((y + 1) * rect.height) / height).max(top + 1) } else { top + 1 };
        for x in 0..width {
            let left = (x * rect.width) / width;
            let right = if smooth { (((x + 1) * rect.width) / width).max(left + 1) } else { left + 1 };
            let mut sum = [0u32; 4];
            for source_y in top..bottom {
                for source_x in left..right {
                    let from = (rect.y + source_y) * stride + (rect.x + source_x) * 4;
                    for channel in 0..4 {
                        sum[channel] += sheet.pixels[from + channel] as u32;
                    }
                }
            }
            let count = ((bottom - top) * (right - left)) as u32;
            let to = (y * width + x) * 4;
            destination[to] = ((sum[2] + count / 2) / count) as u8; // B
            destination[to + 1] = ((sum[1] + count / 2) / count) as u8; // G
            destination[to + 2] = ((sum[0] + count / 2) / count) as u8; // R
            destination[to + 3] = ((sum[3] + count / 2) / count) as u8; // A
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sheet() -> PetImage {
        // 4x2, premultiplied RGBA: a bright left half and an empty right half.
        let mut pixels = Vec::new();
        for _row in 0..2 {
            for column in 0..4 {
                pixels.extend_from_slice(if column < 2 { &[200, 100, 40, 255] } else { &[0, 0, 0, 0] });
            }
        }
        PetImage { width: 4, height: 2, pixels }
    }

    #[test]
    fn unsmoothed_is_a_channel_swap_of_the_nearest_pixel() {
        let rect = FrameRect { sheet: roamling_pet::Sheet::Package, x: 0, y: 0, width: 4, height: 2 };
        let mut out = vec![0u8; 2 * 1 * 4];
        sample(&sheet(), rect, 2, 1, false, &mut out);
        assert_eq!(out, [40, 100, 200, 255, 0, 0, 0, 0]);
    }

    #[test]
    fn smoothed_averages_everything_a_destination_pixel_covers() {
        let rect = FrameRect { sheet: roamling_pet::Sheet::Package, x: 0, y: 0, width: 4, height: 2 };
        let mut out = vec![0u8; 4];
        sample(&sheet(), rect, 1, 1, true, &mut out);
        // Half the covered pixels are opaque and half are empty.
        assert_eq!(out, [20, 50, 100, 128]);
    }

    #[test]
    fn smoothing_changes_nothing_when_the_cell_is_not_being_shrunk() {
        let rect = FrameRect { sheet: roamling_pet::Sheet::Package, x: 0, y: 0, width: 4, height: 2 };
        let (mut plain, mut smooth) = (vec![0u8; 8 * 4 * 4], vec![0u8; 8 * 4 * 4]);
        sample(&sheet(), rect, 8, 4, false, &mut plain);
        sample(&sheet(), rect, 8, 4, true, &mut smooth);
        assert_eq!(plain, smooth);
    }
}
