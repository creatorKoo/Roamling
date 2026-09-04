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

use windows::core::PCWSTR;
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    CreateCompatibleDC, CreateFontW, DeleteDC, DrawTextW, GetDC, ReleaseDC, SelectObject,
    SetBkMode, SetTextColor, CLEARTYPE_QUALITY,
    CLIP_DEFAULT_PRECIS, DEFAULT_CHARSET, DEFAULT_PITCH, DT_CENTER, DT_NOCLIP, DT_SINGLELINE,
    DT_VCENTER, FF_DONTCARE, FW_NORMAL, OUT_DEFAULT_PRECIS, TRANSPARENT,
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
/// Reserved. See the note in `show_menu` for why nothing offers it yet.
pub const CMD_CURSOR_AWARE: usize = 5;
pub const CMD_QUIT: usize = 6;

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}


/// The same mark the macOS menu bar shows.
///
/// `RoamlingAppDelegate` sets its status item's title to "🐾" and nothing else,
/// so the tray gets the same glyph rather than a second, different idea of what
/// Roamling looks like. It is drawn rather than shipped as an .ico so it lands
/// on whatever size the shell asks for -- a tray icon is 16px at 100% and 24 at
/// 150%, and a bitmap picked for one is wrong on the other.
///
/// GDI has no colour-emoji path, so the glyph is drawn white on black and the
/// coverage becomes the alpha. That is what makes the antialiased edge survive:
/// the shape is the alpha, the colour is chosen here.
fn paw_icon() -> Option<HICON> {
    let side = unsafe { GetSystemMetrics(SM_CXSMICON) }.max(16);
    unsafe {
        let screen = GetDC(None);
        let dc = CreateCompatibleDC(screen);
        ReleaseDC(None, screen);

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
        let Ok(bitmap) = CreateDIBSection(dc, &info, DIB_RGB_COLORS, &mut bits, None, 0) else {
            let _ = DeleteDC(dc);
            return None;
        };
        SelectObject(dc, bitmap);
        let pixels = std::slice::from_raw_parts_mut(bits as *mut u8, (side * side * 4) as usize);
        pixels.fill(0);

        // Slightly under the box: the glyph has its own side bearings and a
        // paw drawn edge to edge reads as a smudge at sixteen pixels.
        let face = wide("Segoe UI Emoji");
        let font = CreateFontW(
            -(side * 5 / 8),
            0,
            0,
            0,
            FW_NORMAL.0 as i32,
            0,
            0,
            0,
            DEFAULT_CHARSET.0 as u32,
            OUT_DEFAULT_PRECIS.0 as u32,
            CLIP_DEFAULT_PRECIS.0 as u32,
            CLEARTYPE_QUALITY.0 as u32,
            (DEFAULT_PITCH.0 | FF_DONTCARE.0) as u32,
            PCWSTR(face.as_ptr()),
        );
        let previous = SelectObject(dc, font);
        SetBkMode(dc, TRANSPARENT);
        SetTextColor(dc, COLORREF(0x00FF_FFFF));
        // `wide` appends the terminator Win32 strings want; `DrawTextW` takes a
        // length instead, so the glyph goes in without one.
        let mut glyph: Vec<u16> = "\u{1F43E}".encode_utf16().collect();
        let mut box_ = RECT {
            left: 0,
            top: 0,
            right: side,
            bottom: side,
        };
        DrawTextW(
            dc,
            &mut glyph,
            &mut box_,
            DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_NOCLIP,
        );
        SelectObject(dc, previous);
        let _ = DeleteObject(font);

        // Coverage becomes alpha, premultiplied against the colour. Windows
        // taskbars are light as often as dark, so this is the emoji's own warm
        // brown rather than a flat black that vanishes on one of them.
        let (r, g, b) = (0x6Du32, 0x4Cu32, 0x41u32);
        for pixel in pixels.chunks_exact_mut(4) {
            let coverage = pixel[0].max(pixel[1]).max(pixel[2]) as u32;
            pixel[0] = (b * coverage / 255) as u8;
            pixel[1] = (g * coverage / 255) as u8;
            pixel[2] = (r * coverage / 255) as u8;
            pixel[3] = coverage as u8;
        }

        // A development affordance: the tray hides new icons behind the Windows
        // 11 chevron, so there is no way to look at this one on screen.
        if let Some(path) = std::env::var_os("ROAMLING_DUMP_ICON") {
            let mut raw = (side as u32).to_le_bytes().to_vec();
            raw.extend_from_slice(pixels);
            let _ = std::fs::write(path, raw);
        }

        // A 32bpp icon carries its own alpha; the mask exists because the
        // structure demands one, and all-zero means "let the colour through".
        let mask: HBITMAP = CreateBitmap(side, side, 1, 1, None);
        let mut icon_info = ICONINFO {
            fIcon: true.into(),
            xHotspot: 0,
            yHotspot: 0,
            hbmMask: mask,
            hbmColor: bitmap,
        };
        let icon = CreateIconIndirect(&mut icon_info).ok();
        let _ = DeleteObject(mask);
        let _ = DeleteObject(bitmap);
        let _ = DeleteDC(dc);
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
pub fn add(hwnd: HWND) -> bool {
    let mut entry = data(hwnd);
    entry.uFlags = NIF_MESSAGE | NIF_TIP | NIF_ICON;
    entry.uCallbackMessage = WM_TRAY;
    if let Some(icon) = paw_icon() {
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
            // "커서 인식"은 여기 없다. 코드는 focus.rs 에 있지만 코어가 캐럿을
            // 묻는 것은 펫이 어떤 창을 보고 있을 때뿐이고, 그 상태는 agent
            // 이벤트로만 시작된다. RoamlingSources 가 이식되기 전까지 이 토글은
            // 아무것도 하지 않으므로, 있는 척하지 않는다.
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
