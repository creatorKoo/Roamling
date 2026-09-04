// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

//! The notification-area icon and its menu.
//!
//! macOS puts this in the menu bar and builds it from `ShellMenu`, which holds
//! the tree as data. That module is Swift, so this one carries its own small
//! tree for now, in the same order and with the same words.
//!
//! The strings still come from the shared `.strings` files. See `strings.rs`.

use crate::strings::{localized, localized_format};
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
pub const CMD_OPEN_PET_FOLDER: usize = 7;
pub const CMD_COPY_DIAGNOSTICS: usize = 8;
pub const CMD_ABOUT: usize = 9;
pub const CMD_VIEW_SOURCE: usize = 10;
pub const CMD_TUNING: usize = 11;
pub const CMD_RELOAD_PETS: usize = 12;
/// The built-in mascot, then one id per discovered package.
pub const CMD_PET_BUILT_IN: usize = 1_000;
pub const CMD_PET_BASE: usize = 1_001;
/// One id per entry in `SCALE_CHOICES`, in that order.
pub const CMD_SCALE_BASE: usize = 20;
/// One block of ids per agent, so the handler can tell which one was picked
/// without a second lookup table.
pub const CMD_AGENT_BASE: usize = 100;
pub const CMD_AGENT_STRIDE: usize = 10;
pub const CMD_AGENT_INSTALL: usize = 0;
pub const CMD_AGENT_REMOVE: usize = 1;
pub const CMD_AGENT_TEST: usize = 2;

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

/// What the menu shows. A struct rather than a row of bools, which is the shape
/// that eventually gets one of them silently swapped.
#[derive(Clone)]
pub struct MenuState {
    /// The pet's own name, for the caption line.
    pub pet_name: String,
    /// How many of the sixteen capabilities the sheet answers for, and which
    /// ones are borrowed or missing. A package that declares one animation
    /// renders it for every state, and from outside that looks like a pet whose
    /// behaviour is broken rather than one whose sheet is thin. Say which.
    pub covered: usize,
    pub total: usize,
    pub substituted: Vec<String>,
    pub placeholder: Vec<String>,
    /// The user's own size multiplier, on top of the display's scale.
    pub scale: f64,
    /// Discovered packages and whether each is the one in use. Empty when
    /// nothing is installed, which is the ordinary case.
    pub pets: Vec<(String, bool)>,
    /// Whether the built-in mascot is the one showing.
    pub built_in: bool,
    /// Each agent, whether its hook is installed, stale or absent, and whether
    /// its endpoint came up.
    pub agents: [(Agent, installer::Status, bool); 2],
    pub roaming: bool,
    pub avoiding: bool,
    pub interactive: bool,
    pub visual: bool,
    pub cursor_aware: bool,
}

/// The sizes the menu offers, matching `ShellMenu.scaleChoices`.
pub const SCALE_CHOICES: [(&str, f64); 4] =
    [("0.75x", 0.75), ("1.0x", 1.0), ("1.25x", 1.25), ("1.5x", 1.5)];

/// A line that reports rather than commands. AppKit draws these disabled and so
/// does Win32, which is the whole reason a caption cannot be clicked by mistake.
unsafe fn caption(menu: HMENU, text: &str) {
    let label = wide(text);
    let _ = AppendMenuW(
        menu,
        MF_STRING | MF_DISABLED | MF_GRAYED,
        0,
        PCWSTR(label.as_ptr()),
    );
}

unsafe fn command(menu: HMENU, id: usize, text: &str) {
    let label = wide(text);
    let _ = AppendMenuW(menu, MF_STRING, id, PCWSTR(label.as_ptr()));
}

unsafe fn attach(parent: HMENU, submenu: HMENU, title: &str) {
    let label = wide(title);
    let _ = AppendMenuW(
        parent,
        MF_STRING | MF_POPUP,
        submenu.0 as usize,
        PCWSTR(label.as_ptr()),
    );
}

/// Show the menu and return the chosen command, or 0.
///
/// `TPM_RETURNCMD` keeps the answer here instead of routing a `WM_COMMAND`
/// back through the window procedure, which matters because that procedure is
/// re-entrancy-sensitive -- see the note on `wndproc`.
pub fn show_menu(hwnd: HWND, state: MenuState) -> usize {
    unsafe {
        let Some(menu) = build(&state) else {
            return 0;
        };

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

/// The tree itself, separated from showing it so a test can build one.
///
/// `DestroyMenu` on the returned handle frees the submenus with it.
unsafe fn build(state: &MenuState) -> Option<HMENU> {
    {
        let menu = CreatePopupMenu().ok()?;
        let checked = |on: bool| if on { MF_CHECKED } else { MF_UNCHECKED };
        let separator = |menu| AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());

        caption(menu, &localized_format("menu.title", &[&state.pet_name]));
        let _ = separator(menu);

        // Pet. Only the built-in for now -- installed packages arrive with the
        // catalogue -- but the coverage lines are real, read off the resolver.
        if let Ok(pets) = CreatePopupMenu() {
            let label = localized_format("menu.pet.builtin", &["Mochi"]);
            let name = wide(&label);
            let _ = AppendMenuW(
                pets,
                MF_STRING | checked(state.built_in),
                CMD_PET_BUILT_IN,
                PCWSTR(name.as_ptr()),
            );
            if !state.pets.is_empty() {
                let _ = separator(pets);
            }
            for (index, (name, selected)) in state.pets.iter().enumerate() {
                let label = wide(name);
                let _ = AppendMenuW(
                    pets,
                    MF_STRING | checked(*selected),
                    CMD_PET_BASE + index,
                    PCWSTR(label.as_ptr()),
                );
            }
            let _ = separator(pets);
            caption(
                pets,
                &localized_format(
                    "menu.pet.coverage",
                    &[&state.covered.to_string(), &state.total.to_string()],
                ),
            );
            if !state.substituted.is_empty() {
                caption(
                    pets,
                    &localized_format("menu.pet.substituted", &[&state.substituted.join(", ")]),
                );
            }
            if !state.placeholder.is_empty() {
                caption(
                    pets,
                    &localized_format("menu.pet.placeholder", &[&state.placeholder.join(", ")]),
                );
            }
            attach(menu, pets, localized("menu.pet"));
        }

        // Size.
        if let Ok(sizes) = CreatePopupMenu() {
            for (index, (label, value)) in SCALE_CHOICES.iter().enumerate() {
                let text = wide(label);
                let _ = AppendMenuW(
                    sizes,
                    MF_STRING | checked((state.scale - value).abs() < 0.01),
                    CMD_SCALE_BASE + index,
                    PCWSTR(text.as_ptr()),
                );
            }
            attach(menu, sizes, localized("menu.size"));
        }
        let _ = separator(menu);

        for (flag, id, key) in [
            (checked(state.roaming), CMD_ROAMING, "menu.roaming"),
            (checked(state.avoiding), CMD_AVOID_POINTER, "menu.avoidPointer"),
            (checked(state.interactive), CMD_INTERACTIONS, "menu.catchDrag"),
        ] {
            let label = wide(localized(key));
            let _ = AppendMenuW(menu, MF_STRING | flag, id, PCWSTR(label.as_ptr()));
        }
        command(menu, CMD_TUNING, localized("menu.tuning"));

        // One submenu per agent, the same shape `ShellMenu.agentItems` builds:
        // two status lines that cannot be clicked, then install-or-repair,
        // remove once there is something to remove, and a test reaction.
        for (index, (agent, status, listening)) in state.agents.iter().enumerate() {
            let Ok(submenu) = CreatePopupMenu() else { continue };
            let base = CMD_AGENT_BASE + index * CMD_AGENT_STRIDE;

            caption(
                submenu,
                localized(match status {
                    installer::Status::Installed => "status.hooks.installed",
                    installer::Status::NeedsRepair => "status.hooks.needsRepair",
                    installer::Status::NotInstalled => "status.hooks.notInstalled",
                }),
            );
            // Two states, not four: the endpoint either bound at launch or it
            // did not. There is no async start to be "starting" during, and it
            // is only stopped when the app is going away.
            caption(
                submenu,
                localized(if *listening {
                    "status.receiver.ready"
                } else {
                    "status.receiver.unavailable"
                }),
            );
            let _ = separator(submenu);

            command(
                submenu,
                base + CMD_AGENT_INSTALL,
                localized(if *status == installer::Status::NotInstalled {
                    "action.install"
                } else {
                    "action.repair"
                }),
            );
            if *status != installer::Status::NotInstalled {
                command(submenu, base + CMD_AGENT_REMOVE, localized("action.remove"));
            }
            command(submenu, base + CMD_AGENT_TEST, localized("action.testReaction"));
            attach(menu, submenu, agent.display_name());
        }

        // On macOS these two are submenus reporting an OS permission the app
        // can only ask for. Windows grants both outright -- there is no prompt
        // to route the user to -- so consent is the checkmark itself, and a
        // submenu wrapping a single toggle would say less, not more.
        // `docs/windows.md` section 6.
        for (flag, id, key) in [
            (checked(state.cursor_aware), CMD_CURSOR_AWARE, "menu.accessibility"),
            (checked(state.visual), CMD_VISUAL, "menu.visualPlacement"),
        ] {
            let label = wide(localized(key));
            let _ = AppendMenuW(menu, MF_STRING | flag, id, PCWSTR(label.as_ptr()));
        }

        let _ = separator(menu);
        command(menu, CMD_OPEN_PET_FOLDER, localized("menu.openPetFolder"));
        command(menu, CMD_COPY_DIAGNOSTICS, localized("menu.copyDiagnostics"));
        command(menu, CMD_RELOAD_PETS, localized("menu.reloadPets"));
        let _ = separator(menu);
        command(menu, CMD_ABOUT, localized("menu.about"));
        // The macOS About alert carries this as a second button. `MessageBoxW`
        // cannot relabel its buttons, so it is an item instead.
        command(menu, CMD_VIEW_SOURCE, localized("menu.viewSource"));
        command(menu, CMD_QUIT, localized("menu.quit"));
        Some(menu)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state() -> MenuState {
        MenuState {
            pet_name: "Mochi".into(),
            covered: 14,
            total: 16,
            substituted: vec!["sit".into()],
            placeholder: vec![],
            scale: 1.0,
            pets: vec![("Installed One".into(), true), ("Installed Two".into(), false)],
            built_in: false,
            agents: [
                (Agent::ClaudeCode, installer::Status::Installed, true),
                (Agent::Codex, installer::Status::NotInstalled, false),
            ],
            roaming: true,
            avoiding: true,
            interactive: true,
            visual: false,
            cursor_aware: false,
        }
    }

    /// Every command id the dispatcher answers, gathered off a real menu.
    fn ids(menu: HMENU) -> Vec<usize> {
        let mut found = Vec::new();
        unsafe {
            for index in 0..GetMenuItemCount(menu) {
                let id = GetMenuItemID(menu, index);
                if id != u32::MAX && id != 0 {
                    found.push(id as usize);
                }
                let submenu = GetSubMenu(menu, index);
                if !submenu.is_invalid() {
                    found.extend(ids(submenu));
                }
            }
        }
        found
    }

    /// The tree is Win32 objects rather than a value, so the only way to know
    /// it came out whole is to build one. This catches an id colliding with
    /// another -- the ranges are hand-assigned -- and a submenu that failed to
    /// attach, which would silently drop everything under it.
    #[test]
    fn the_tree_builds_with_every_command_reachable() {
        let state = state();
        let menu = unsafe { build(&state) }.expect("the menu did not build");
        let found = ids(menu);
        unsafe {
            let _ = DestroyMenu(menu);
        }

        let agent_one = CMD_AGENT_BASE;
        let agent_two = CMD_AGENT_BASE + CMD_AGENT_STRIDE;
        let expected = [
            CMD_ROAMING,
            CMD_AVOID_POINTER,
            CMD_INTERACTIONS,
            CMD_VISUAL,
            CMD_CURSOR_AWARE,
            CMD_OPEN_PET_FOLDER,
            CMD_COPY_DIAGNOSTICS,
            CMD_ABOUT,
            CMD_VIEW_SOURCE,
            CMD_QUIT,
            CMD_TUNING,
            CMD_RELOAD_PETS,
            CMD_PET_BUILT_IN,
            CMD_PET_BASE,
            CMD_PET_BASE + 1,
            CMD_SCALE_BASE,
            CMD_SCALE_BASE + SCALE_CHOICES.len() - 1,
            // Installed, so it offers repair, remove and a test.
            agent_one + CMD_AGENT_INSTALL,
            agent_one + CMD_AGENT_REMOVE,
            agent_one + CMD_AGENT_TEST,
            // Not installed, so there is nothing to remove.
            agent_two + CMD_AGENT_INSTALL,
            agent_two + CMD_AGENT_TEST,
        ];
        for id in expected {
            assert!(found.contains(&id), "{id} is not in the menu: {found:?}");
        }
        assert!(
            !found.contains(&(agent_two + CMD_AGENT_REMOVE)),
            "an agent with no hooks offered Remove"
        );

        let mut seen = found.clone();
        seen.sort_unstable();
        let before = seen.len();
        seen.dedup();
        assert_eq!(before, seen.len(), "two items share a command id: {found:?}");
    }

    /// A caption reports; it must not be pickable, or "Animations: 14 of 16"
    /// would return an id the dispatcher has never heard of.
    #[test]
    fn captions_cannot_be_chosen() {
        let state = state();
        let menu = unsafe { build(&state) }.expect("the menu did not build");
        let title = unsafe { GetMenuState(menu, 0, MF_BYPOSITION) };
        unsafe {
            let _ = DestroyMenu(menu);
        }
        assert_ne!(title, u32::MAX, "the title line is missing");
        assert!(
            title & (MF_DISABLED.0 | MF_GRAYED.0) != 0,
            "the title caption is clickable"
        );
    }
}
