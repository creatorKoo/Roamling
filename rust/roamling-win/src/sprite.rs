// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

//! The layered window's pixels.
//!
//! A placeholder blob for now. `MascotPetFactory` (621 lines) and a WebP
//! decoder are what turn this into Mochi -- see `docs/windows.md`, W4, "그 다음
//! — 아직 Swift에 남은 것". What matters here is the surface: build once, and
//! push a premultiplied BGRA bitmap at a position.


use windows::Win32::Foundation::{COLORREF, HWND, POINT, SIZE};
use windows::Win32::Graphics::Gdi::{
    CreateCompatibleDC, CreateDIBSection, DeleteDC, SelectObject, AC_SRC_ALPHA, AC_SRC_OVER,
    BITMAPINFO, BITMAPINFOHEADER, BLENDFUNCTION, DIB_RGB_COLORS, HDC,
};
use windows::Win32::UI::WindowsAndMessaging::{UpdateLayeredWindow, ULW_ALPHA};

/// A 32bpp top-down DIB holding a premultiplied-alpha blob at `scale`.
pub fn build(cell: i32, rows: i32, scale: f64) -> Option<HDC> {
    let width = (cell as f64 * scale).round() as i32;
    let height = (rows as f64 * scale).round() as i32;
    unsafe {
        let dc = CreateCompatibleDC(None);
        let info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width,
                // Negative is top-down, which matches atlas row order.
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

        let pixels = std::slice::from_raw_parts_mut(bits as *mut u8, (width * height * 4) as usize);
        let (cx, cy) = (width as f32 / 2.0, height as f32 / 2.0);
        let radius = width.min(height) as f32 * 0.42;
        let ramp = 10.0 * scale as f32;
        for y in 0..height {
            for x in 0..width {
                let (dx, dy) = (x as f32 - cx, y as f32 - cy);
                let distance = (dx * dx + dy * dy).sqrt();
                let alpha = (1.0 - (distance - (radius - ramp)) / ramp).clamp(0.0, 1.0);
                let a = (alpha * 255.0) as u32;
                let (r, g, b) = (250u32, 170u32, 200u32);
                let i = ((y * width + x) * 4) as usize;
                // UpdateLayeredWindow wants premultiplied BGRA.
                pixels[i] = (b * a / 255) as u8;
                pixels[i + 1] = (g * a / 255) as u8;
                pixels[i + 2] = (r * a / 255) as u8;
                pixels[i + 3] = a as u8;
            }
        }
        Some(dc)
    }
}

/// Move the window to `corner` and blend the bitmap in one call. Position and
/// pixels travel together, so the pet never tears between the two.
pub fn present(hwnd: HWND, dc: HDC, corner: POINT, width: i32, height: i32) {
    let size = SIZE {
        cx: width,
        cy: height,
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
            dc,
            Some(&source),
            COLORREF(0),
            Some(&blend),
            ULW_ALPHA,
        );
    }
}

pub fn destroy(dc: HDC) {
    unsafe {
        let _ = DeleteDC(dc);
    }
}
