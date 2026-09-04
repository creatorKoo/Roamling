// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

//! The notification-area icon and its menu.
//!
//! macOS puts this in the menu bar and builds it from `ShellMenu`, which holds
//! the tree as data. That module is Swift, so this one carries its own small
//! tree for now -- only the items whose actions exist on Windows today. The
//! rest of `ShellMenu` (pets, tuning, diagnostics) arrives with the features it
//! drives.
//!
//! The strings still come from the shared `.strings` files. See `strings.rs`.

use crate::strings::localized;
use roamling_agent::{installer, Agent};

use windows::core::PCWSTR;
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    HDC,
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
pub const CMD_CURSOR_AWARE: usize = 5;
pub const CMD_QUIT: usize = 6;
/// One block of ids per agent, so the handler can tell which one was picked
/// without a second lookup table.
pub const CMD_AGENT_BASE: usize = 100;
pub const CMD_AGENT_STRIDE: usize = 10;
pub const CMD_AGENT_INSTALL: usize = 0;
pub const CMD_AGENT_REMOVE: usize = 1;

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}


/// The same mark the macOS menu bar shows.
///
/// `RoamlingAppDelegate` sets its status item's title to "🐾" and nothing else,
/// so the tray gets the same glyph rather than a second, different idea of what
/// Roamling looks like. It is drawn rather than shipped as an `.ico` so it lands
/// on whatever size the shell asks for -- a tray icon is 16px at 100% and 24 at
/// 150%, and a bitmap picked for one is wrong on the other.
///
/// GDI has no colour-emoji path, so the glyph is drawn white on black and the
/// coverage becomes the alpha. The shape is the alpha; the colour is chosen here.
///
/// It is drawn oversized, then the *ink* is measured and fitted to the box.
/// Picking a font size instead means guessing at the glyph's side bearings, and
/// guessing left the paws small with empty margin all round.
fn paw_icon() -> Option<HICON> {
    let side = unsafe { GetSystemMetrics(SM_CXSMICON) }.max(16);
    // Four times over, so the fitted result is downscaled rather than up.
    let work = side * 4;

    unsafe {
        let screen = GetDC(None);
        let dc = CreateCompatibleDC(screen);
        ReleaseDC(None, screen);

        let Some((scratch, scratch_bits)) = dib(dc, work, work) else {
            let _ = DeleteDC(dc);
            return None;
        };
        SelectObject(dc, scratch);
        let drawn = std::slice::from_raw_parts_mut(scratch_bits, (work * work * 4) as usize);
        drawn.fill(0);

        let face = wide("Segoe UI Emoji");
        let font = CreateFontW(
            -work,
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
            right: work,
            bottom: work,
        };
        DrawTextW(
            dc,
            &mut glyph,
            &mut box_,
            DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_NOCLIP,
        );
        SelectObject(dc, previous);
        let _ = DeleteObject(font);

        let coverage = |x: i32, y: i32| -> u8 {
            let at = ((y * work + x) * 4) as usize;
            drawn[at].max(drawn[at + 1]).max(drawn[at + 2])
        };

        // The ink's own bounds, which is what gets fitted -- not the em box.
        let (mut left, mut top, mut right, mut bottom) = (work, work, -1, -1);
        for y in 0..work {
            for x in 0..work {
                if coverage(x, y) > 8 {
                    left = left.min(x);
                    top = top.min(y);
                    right = right.max(x);
                    bottom = bottom.max(y);
                }
            }
        }
        let mut icon = None;
        if right >= left && bottom >= top {
            let ink_w = right - left + 1;
            let ink_h = bottom - top + 1;
            // Square the source so the paws keep their proportions, and leave a
            // single pixel of air so the shape does not touch the tray's edge.
            let span = ink_w.max(ink_h);
            let origin_x = left - (span - ink_w) / 2;
            let origin_y = top - (span - ink_h) / 2;
            let margin = 1;
            let target = (side - margin * 2).max(1);

            if let Some((bitmap, bits)) = dib(dc, side, side) {
                let out = std::slice::from_raw_parts_mut(bits, (side * side * 4) as usize);
                out.fill(0);
                // The emoji's own warm brown: Windows taskbars are light as
                // often as dark, and a flat black vanishes on one of them.
                let (r, g, b) = (0x6Du32, 0x4Cu32, 0x41u32);
                for y in 0..target {
                    for x in 0..target {
                        // Box-average the source cell, so shrinking by four
                        // keeps the antialiasing rather than dropping it.
                        let x0 = origin_x + x * span / target;
                        let x1 = (origin_x + (x + 1) * span / target).max(x0 + 1);
                        let y0 = origin_y + y * span / target;
                        let y1 = (origin_y + (y + 1) * span / target).max(y0 + 1);
                        let mut total = 0u32;
                        let mut count = 0u32;
                        for sy in y0..y1 {
                            for sx in x0..x1 {
                                if (0..work).contains(&sx) && (0..work).contains(&sy) {
                                    total += coverage(sx, sy) as u32;
                                }
                                count += 1;
                            }
                        }
                        let alpha = if count == 0 { 0 } else { total / count };
                        let at = (((y + margin) * side + (x + margin)) * 4) as usize;
                        out[at] = (b * alpha / 255) as u8;
                        out[at + 1] = (g * alpha / 255) as u8;
                        out[at + 2] = (r * alpha / 255) as u8;
                        out[at + 3] = alpha as u8;
                    }
                }

                // A development affordance: Windows 11 files new tray icons
                // behind the chevron, so there is no way to look at this one.
                if let Some(path) = std::env::var_os("ROAMLING_DUMP_ICON") {
                    let mut raw = (side as u32).to_le_bytes().to_vec();
                    raw.extend_from_slice(out);
                    let _ = std::fs::write(path, raw);
                }

                // A 32bpp icon carries its own alpha; the mask exists because
                // the structure demands one, and zero lets the colour through.
                let mask: HBITMAP = CreateBitmap(side, side, 1, 1, None);
                let mut icon_info = ICONINFO {
                    fIcon: true.into(),
                    xHotspot: 0,
                    yHotspot: 0,
                    hbmMask: mask,
                    hbmColor: bitmap,
                };
                icon = CreateIconIndirect(&mut icon_info).ok();
                let _ = DeleteObject(mask);
                let _ = DeleteObject(bitmap);
            }
        }
        let _ = DeleteObject(scratch);
        let _ = DeleteDC(dc);
        icon
    }
}

/// A 32bpp top-down DIB and a pointer to its pixels.
fn dib(dc: HDC, width: i32, height: i32) -> Option<(HBITMAP, *mut u8)> {
    let info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: width,
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
#[derive(Clone)]
pub struct MenuState {
    /// Each agent, and whether its hook is installed, stale, or absent.
    pub agents: [(Agent, installer::Status); 2],
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

        // One submenu per agent, the same shape `ShellMenu.agentItems` builds:
        // a status line that cannot be clicked, then install-or-repair, then
        // remove once there is something to remove.
        for (index, (agent, status)) in state.agents.iter().enumerate() {
            let Ok(submenu) = CreatePopupMenu() else { continue };
            let base = CMD_AGENT_BASE + index * CMD_AGENT_STRIDE;

            let status_label = wide(localized(match status {
                installer::Status::Installed => "status.hooks.installed",
                installer::Status::NeedsRepair => "status.hooks.needsRepair",
                installer::Status::NotInstalled => "status.hooks.notInstalled",
            }));
            let _ = AppendMenuW(
                submenu,
                MF_STRING | MF_DISABLED | MF_GRAYED,
                0,
                PCWSTR(status_label.as_ptr()),
            );
            let _ = AppendMenuW(submenu, MF_SEPARATOR, 0, PCWSTR::null());

            let action = wide(localized(if *status == installer::Status::NotInstalled {
                "action.install"
            } else {
                "action.repair"
            }));
            let _ = AppendMenuW(
                submenu,
                MF_STRING,
                base + CMD_AGENT_INSTALL,
                PCWSTR(action.as_ptr()),
            );
            if *status != installer::Status::NotInstalled {
                let remove = wide(localized("action.remove"));
                let _ = AppendMenuW(
                    submenu,
                    MF_STRING,
                    base + CMD_AGENT_REMOVE,
                    PCWSTR(remove.as_ptr()),
                );
            }

            let name = wide(agent.display_name());
            let _ = AppendMenuW(
                menu,
                MF_STRING | MF_POPUP,
                submenu.0 as usize,
                PCWSTR(name.as_ptr()),
            );
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
