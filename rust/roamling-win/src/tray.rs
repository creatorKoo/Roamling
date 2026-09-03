// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

//! The notification-area icon and its menu.
//!
//! macOS puts this in the menu bar and builds it from `ShellMenu`, which holds
//! the tree as data. That module is Swift, so this one carries its own small
//! tree for now -- only the items whose actions exist on Windows today. The
//! rest of `ShellMenu` (pets, agents, tuning, diagnostics) arrives with the
//! features it drives.
//!
//! The strings still come from the shared `.strings` files. See `strings.rs`.

use crate::strings::localized;
use roamling_pet::PetAsset;
use windows::core::PCWSTR;
use windows::Win32::Foundation::{HWND, LPARAM, POINT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    CreateBitmap, CreateDIBSection, DeleteObject, BITMAPINFO, BITMAPINFOHEADER, DIB_RGB_COLORS,
    HBITMAP,
};
use windows::Win32::UI::Shell::{
    Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW,
};
use windows::Win32::UI::WindowsAndMessaging::*;

/// The tray's callback lands here. `WM_APP` upwards is reserved for the app.
pub const WM_TRAY: u32 = WM_APP + 1;

pub const CMD_ROAMING: usize = 1;
pub const CMD_AVOID_POINTER: usize = 2;
pub const CMD_INTERACTIONS: usize = 3;
pub const CMD_VISUAL: usize = 4;
pub const CMD_CURSOR_AWARE: usize = 5;
pub const CMD_QUIT: usize = 6;

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

/// The pet's own face, rather than a stock application icon: the sheet is
/// already decoded and the first idle cell is the character's approved look.
fn icon_from_idle(asset: &PetAsset) -> Option<HICON> {
    let side = unsafe { GetSystemMetrics(SM_CXSMICON) }.max(16);
    let rect = asset.frame_rect(0)?;
    let sheet = asset.sheet(rect.sheet)?;

    // Fit the cell inside the square without stretching it; the cell is taller
    // than it is wide, so this leaves transparent columns either side.
    let fit = (side as f64 / rect.width as f64).min(side as f64 / rect.height as f64);
    let draw_w = (rect.width as f64 * fit).round() as i32;
    let draw_h = (rect.height as f64 * fit).round() as i32;
    let left = (side - draw_w) / 2;
    let top = (side - draw_h) / 2;

    unsafe {
        let info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: side,
                biHeight: -side,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: 0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut bits: *mut std::ffi::c_void = std::ptr::null_mut();
        let colour = CreateDIBSection(None, &info, DIB_RGB_COLORS, &mut bits, None, 0).ok()?;
        let pixels =
            std::slice::from_raw_parts_mut(bits as *mut u8, (side * side * 4) as usize);
        pixels.fill(0);

        let stride = sheet.width * 4;
        for y in 0..draw_h {
            let source_y = rect.y + (y as usize * rect.height) / draw_h as usize;
            for x in 0..draw_w {
                let source_x = rect.x + (x as usize * rect.width) / draw_w as usize;
                let from = source_y * stride + source_x * 4;
                let to = (((y + top) * side + (x + left)) * 4) as usize;
                pixels[to] = sheet.pixels[from + 2];
                pixels[to + 1] = sheet.pixels[from + 1];
                pixels[to + 2] = sheet.pixels[from];
                pixels[to + 3] = sheet.pixels[from + 3];
            }
        }

        // A 32bpp icon carries its own alpha, so the mask is only there because
        // the structure demands one. All zero means "let the colour through".
        let mask: HBITMAP = CreateBitmap(side, side, 1, 1, None);
        let mut icon_info = ICONINFO {
            fIcon: true.into(),
            xHotspot: 0,
            yHotspot: 0,
            hbmMask: mask,
            hbmColor: colour,
        };
        let icon = CreateIconIndirect(&mut icon_info).ok();
        let _ = DeleteObject(mask);
        let _ = DeleteObject(colour);
        icon
    }
}

fn data(hwnd: HWND) -> NOTIFYICONDATAW {
    NOTIFYICONDATAW {
        cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: hwnd,
        uID: 1,
        ..Default::default()
    }
}

/// Returns whether the shell accepted it. Windows 11 files new icons into the
/// overflow flyout by default, so "registered" and "visible" are not the same
/// thing -- the user pins it, or it lives behind the chevron.
pub fn add(hwnd: HWND, asset: &PetAsset) -> bool {
    let mut entry = data(hwnd);
    entry.uFlags = NIF_MESSAGE | NIF_TIP | NIF_ICON;
    entry.uCallbackMessage = WM_TRAY;
    if let Some(icon) = icon_from_idle(asset) {
        entry.hIcon = icon;
    }
    let tip = wide("Roamling");
    entry.szTip[..tip.len()].copy_from_slice(&tip);
    unsafe {
        Shell_NotifyIconW(NIM_ADD, &entry).as_bool()
    }
}

pub fn remove(hwnd: HWND) {
    unsafe {
        let _ = Shell_NotifyIconW(NIM_DELETE, &data(hwnd));
    }
}

/// What the checkmarks show. A struct rather than five bools in a row, which
/// is the shape that eventually gets one of them silently swapped.
#[derive(Clone, Copy)]
pub struct MenuState {
    pub roaming: bool,
    pub avoiding: bool,
    pub interactive: bool,
    pub visual: bool,
    pub cursor_aware: bool,
}

/// Show the menu and return the chosen command, or 0.
///
/// `TPM_RETURNCMD` keeps the answer here instead of routing a `WM_COMMAND`
/// back through the window procedure, which matters because that procedure is
/// re-entrancy-sensitive -- see the note on `wndproc`.
pub fn show_menu(hwnd: HWND, state: MenuState) -> usize {
    unsafe {
        let Ok(menu) = CreatePopupMenu() else {
            return 0;
        };
        let checked = |on: bool| if on { MF_CHECKED } else { MF_UNCHECKED };
        for (flag, id, key) in [
            (checked(state.roaming), CMD_ROAMING, "menu.roaming"),
            (checked(state.avoiding), CMD_AVOID_POINTER, "menu.avoidPointer"),
            (checked(state.interactive), CMD_INTERACTIONS, "menu.catchDrag"),
            (checked(state.visual), CMD_VISUAL, "menu.visualPlacement"),
            (checked(state.cursor_aware), CMD_CURSOR_AWARE, "menu.accessibility"),
        ] {
            let label = wide(localized(key));
            let _ = AppendMenuW(menu, MF_STRING | flag, id, PCWSTR(label.as_ptr()));
        }
        let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
        let quit = wide(localized("menu.quit"));
        let _ = AppendMenuW(menu, MF_STRING, CMD_QUIT, PCWSTR(quit.as_ptr()));

        let mut cursor = POINT::default();
        let _ = GetCursorPos(&mut cursor);
        // Without the foreground dance the menu will not dismiss when the user
        // clicks away from it. A tray menu has needed both halves since Win95.
        let _ = SetForegroundWindow(hwnd);
        let chosen = TrackPopupMenu(
            menu,
            TPM_RETURNCMD | TPM_RIGHTBUTTON,
            cursor.x,
            cursor.y,
            0,
            hwnd,
            None,
        );
        let _ = PostMessageW(hwnd, WM_NULL, WPARAM(0), LPARAM(0));
        let _ = DestroyMenu(menu);
        chosen.0 as usize
    }
}
