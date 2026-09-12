// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

#![cfg(debug_assertions)]

//! A disposable palette laboratory for debug builds.
//!
//! It deliberately follows `tuning.rs`: one Win32 class, common-controls
//! trackbars, thread-local ownership, a pending value drained by the main tick,
//! and a `create` seam that lets tests inspect a real hidden window.

use roamling_core::PaletteTargets;
use std::cell::RefCell;

use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    CreateFontW, CreateSolidBrush, DeleteObject, FillRect, InvalidateRect, SetBkMode, SetTextColor,
    UpdateWindow, CLEARTYPE_QUALITY, CLIP_DEFAULT_PRECIS, DEFAULT_CHARSET, DEFAULT_PITCH,
    FF_DONTCARE, FW_NORMAL, FW_SEMIBOLD, HBRUSH, HDC, HFONT, OUT_DEFAULT_PRECIS, TRANSPARENT,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Controls::{
    InitCommonControlsEx, ICC_BAR_CLASSES, INITCOMMONCONTROLSEX, TBM_SETPOS, TBM_SETRANGE,
    TBS_HORZ, TBS_NOTICKS,
};
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::WindowsAndMessaging::*;

/// Posted to the overlay when a slider changes, so a resting pet does not wait
/// for its next low-frequency timer tick before changing colour.
pub const WM_PALETTE_CHANGED: u32 = WM_APP + 2;

const TBM_GETPOS: u32 = WM_USER;
const SS_RIGHT: u32 = 0x0000_0002;
const ID_RESET: usize = 1;
const ID_DONE: usize = 2;
const ID_SLIDER: usize = 100;
const ID_VALUE: usize = 200;

#[derive(Clone, Copy)]
enum Family {
    Dark,
    Orange,
    Cream,
}

impl Family {
    fn colour(self, palette: PaletteTargets) -> [u8; 3] {
        match self {
            Self::Dark => palette.dark,
            Self::Orange => palette.orange,
            Self::Cream => palette.cream,
        }
    }

    fn with_colour(self, palette: PaletteTargets, colour: [u8; 3]) -> PaletteTargets {
        match self {
            Self::Dark => PaletteTargets {
                dark: colour,
                ..palette
            },
            Self::Orange => PaletteTargets {
                orange: colour,
                ..palette
            },
            Self::Cream => PaletteTargets {
                cream: colour,
                ..palette
            },
        }
    }
}

#[derive(Clone, Copy)]
struct Row {
    family: Family,
    channel: usize,
    label: &'static str,
}

const ROWS: [Row; 9] = [
    Row {
        family: Family::Dark,
        channel: 0,
        label: "R",
    },
    Row {
        family: Family::Dark,
        channel: 1,
        label: "G",
    },
    Row {
        family: Family::Dark,
        channel: 2,
        label: "B",
    },
    Row {
        family: Family::Orange,
        channel: 0,
        label: "R",
    },
    Row {
        family: Family::Orange,
        channel: 1,
        label: "G",
    },
    Row {
        family: Family::Orange,
        channel: 2,
        label: "B",
    },
    Row {
        family: Family::Cream,
        channel: 0,
        label: "R",
    },
    Row {
        family: Family::Cream,
        channel: 1,
        label: "G",
    },
    Row {
        family: Family::Cream,
        channel: 2,
        label: "B",
    },
];

struct Panel {
    window: HWND,
    app_window: HWND,
    palette: PaletteTargets,
    original: PaletteTargets,
    sliders: Vec<(HWND, HWND, Row)>,
    body: HFHandle,
    heading: HFHandle,
    background: HBHandle,
}

struct HFHandle(HFONT);
struct HBHandle(HBRUSH);

thread_local! {
    static PANEL: RefCell<Option<Panel>> = const { RefCell::new(None) };
    static PENDING: RefCell<Option<PaletteTargets>> = const { RefCell::new(None) };
}

pub fn take_pending() -> Option<PaletteTargets> {
    PENDING.with(|slot| slot.borrow_mut().take())
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

fn set_icon(window: HWND, instance: windows::Win32::Foundation::HMODULE) {
    const ICON: PCWSTR = PCWSTR(1 as *const u16);
    unsafe {
        for (which, metric) in [(ICON_SMALL, SM_CXSMICON), (ICON_BIG, SM_CXICON)] {
            let side = GetSystemMetrics(metric);
            let Ok(icon) = LoadImageW(instance, ICON, IMAGE_ICON, side, side, LR_DEFAULTCOLOR)
            else {
                continue;
            };
            SendMessageW(
                window,
                WM_SETICON,
                WPARAM(which as usize),
                LPARAM(icon.0 as isize),
            );
        }
    }
}

/// Opens the lab, or brings the existing hidden instance back.
pub fn show(app_window: HWND, current: PaletteTargets, original: PaletteTargets) {
    let existing = PANEL.with(|slot| slot.borrow().as_ref().map(|panel| panel.window));
    if let Some(window) = existing {
        PANEL.with(|slot| {
            if let Some(panel) = slot.borrow_mut().as_mut() {
                panel.app_window = app_window;
                panel.original = original;
            }
        });
        replace(current, false);
        unsafe {
            let _ = ShowWindow(window, SW_SHOW);
            let _ = SetForegroundWindow(window);
        }
        return;
    }
    match create(app_window, current, original) {
        Ok(window) => unsafe {
            let _ = ShowWindow(window, SW_SHOW);
            let _ = UpdateWindow(window);
            let _ = SetForegroundWindow(window);
        },
        Err(error) => println!("could not open the palette lab: {error:?}"),
    }
}

/// Builds the window without showing it, so tests can inspect real controls
/// without flashing a panel on the desktop.
fn create(
    app_window: HWND,
    current: PaletteTargets,
    original: PaletteTargets,
) -> windows::core::Result<HWND> {
    unsafe {
        // Trackbars are registered by comctl32 only after this call.
        let controls = INITCOMMONCONTROLSEX {
            dwSize: std::mem::size_of::<INITCOMMONCONTROLSEX>() as u32,
            dwICC: ICC_BAR_CLASSES,
        };
        let _ = InitCommonControlsEx(&controls);

        let instance = GetModuleHandleW(None)?;
        let class = w!("RoamlingPaletteDebug");
        let registered = WNDCLASSW {
            lpfnWndProc: Some(wndproc),
            hInstance: instance.into(),
            lpszClassName: class,
            hCursor: LoadCursorW(None, IDC_ARROW)?,
            hbrBackground: HBRUSH(std::ptr::null_mut()),
            ..Default::default()
        };
        RegisterClassW(&registered);

        let title = wide("Mochi Palette Lab (Debug)");
        let window = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            class,
            PCWSTR(title.as_ptr()),
            WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            430,
            560,
            None,
            None,
            instance,
            None,
        )?;
        set_icon(window, instance);
        build(window, app_window, current, original);
        Ok(window)
    }
}

fn build(window: HWND, app_window: HWND, palette: PaletteTargets, original: PaletteTargets) {
    unsafe {
        let dpi = GetDpiForWindow(window).max(96);
        let scaled = |value: i32| value * dpi as i32 / 96;
        let font = |size: i32, weight: i32| {
            let face = wide("Segoe UI");
            CreateFontW(
                -scaled(size),
                0,
                0,
                0,
                weight,
                0,
                0,
                0,
                DEFAULT_CHARSET.0 as u32,
                OUT_DEFAULT_PRECIS.0 as u32,
                CLIP_DEFAULT_PRECIS.0 as u32,
                CLEARTYPE_QUALITY.0 as u32,
                (DEFAULT_PITCH.0 | FF_DONTCARE.0) as u32,
                PCWSTR(face.as_ptr()),
            )
        };
        let body = font(12, FW_NORMAL.0 as i32);
        let heading = font(13, FW_SEMIBOLD.0 as i32);
        let margin = scaled(20);
        let channel_width = scaled(22);
        let track_width = scaled(270);
        let value_width = scaled(44);
        let gap = scaled(8);
        let row_height = scaled(28);
        let width = margin * 2 + channel_width + track_width + value_width + gap * 2;
        let instance = GetModuleHandleW(None).unwrap_or_default();
        let mut y = margin;
        let mut sliders = Vec::with_capacity(ROWS.len());

        let label = |text: &str, x: i32, y: i32, w: i32, h: i32, style: u32, which: HFONT| {
            let content = wide(text);
            let control = CreateWindowExW(
                WINDOW_EX_STYLE(0),
                w!("STATIC"),
                PCWSTR(content.as_ptr()),
                WINDOW_STYLE(WS_CHILD.0 | WS_VISIBLE.0 | style),
                x,
                y,
                w,
                h,
                window,
                None,
                instance,
                None,
            )
            .unwrap_or_default();
            SendMessageW(control, WM_SETFONT, WPARAM(which.0 as usize), LPARAM(1));
            control
        };

        label(
            "Disposable debug controls. Changes apply to the on-screen Mochi immediately.",
            margin,
            y,
            width - margin * 2,
            scaled(42),
            0,
            body,
        );
        y += scaled(48);

        for (family_index, (family, title)) in [
            (Family::Dark, "Dark family"),
            (Family::Orange, "Orange family"),
            (Family::Cream, "Cream family"),
        ]
        .into_iter()
        .enumerate()
        {
            label(title, margin, y, width - margin * 2, row_height, 0, heading);
            y += row_height;
            for row in ROWS.iter().copied().skip(family_index * 3).take(3) {
                debug_assert!(matches!(
                    (family, row.family),
                    (Family::Dark, Family::Dark)
                        | (Family::Orange, Family::Orange)
                        | (Family::Cream, Family::Cream)
                ));
                let index = sliders.len();
                label(
                    row.label,
                    margin,
                    y + scaled(4),
                    channel_width,
                    row_height,
                    0,
                    body,
                );
                let track = CreateWindowExW(
                    WINDOW_EX_STYLE(0),
                    w!("msctls_trackbar32"),
                    PCWSTR::null(),
                    WINDOW_STYLE(WS_CHILD.0 | WS_VISIBLE.0 | WS_TABSTOP.0 | TBS_HORZ | TBS_NOTICKS),
                    margin + channel_width + gap,
                    y,
                    track_width,
                    row_height,
                    window,
                    HMENU((ID_SLIDER + index) as *mut std::ffi::c_void),
                    instance,
                    None,
                )
                .unwrap_or_default();
                let readout = label(
                    "",
                    margin + channel_width + track_width + gap * 2,
                    y + scaled(4),
                    value_width,
                    row_height,
                    SS_RIGHT,
                    body,
                );
                let _ = SetWindowLongPtrW(readout, GWLP_ID, (ID_VALUE + index) as isize);
                sliders.push((track, readout, row));
                y += row_height + scaled(3);
            }
            y += gap;
        }

        let button_width = scaled(120);
        let button_height = scaled(28);
        let button = |text: &str, x: i32, id: usize| {
            let content = wide(text);
            let control = CreateWindowExW(
                WINDOW_EX_STYLE(0),
                w!("BUTTON"),
                PCWSTR(content.as_ptr()),
                WINDOW_STYLE(WS_CHILD.0 | WS_VISIBLE.0 | WS_TABSTOP.0 | BS_PUSHBUTTON as u32),
                x,
                y,
                button_width,
                button_height,
                window,
                HMENU(id as *mut std::ffi::c_void),
                instance,
                None,
            )
            .unwrap_or_default();
            SendMessageW(control, WM_SETFONT, WPARAM(body.0 as usize), LPARAM(1));
        };
        button("Reset anchors", margin, ID_RESET);
        button("Done", width - margin - button_width, ID_DONE);
        y += button_height + margin;

        let mut frame = RECT {
            left: 0,
            top: 0,
            right: width,
            bottom: y,
        };
        let style = WINDOW_STYLE(GetWindowLongPtrW(window, GWL_STYLE) as u32);
        let _ = AdjustWindowRect(&mut frame, style, false);
        let _ = SetWindowPos(
            window,
            None,
            0,
            0,
            frame.right - frame.left,
            frame.bottom - frame.top,
            SWP_NOMOVE | SWP_NOZORDER,
        );
        centre(window);

        PANEL.with(|slot| {
            *slot.borrow_mut() = Some(Panel {
                window,
                app_window,
                palette,
                original,
                sliders,
                body: HFHandle(body),
                heading: HFHandle(heading),
                background: HBHandle(CreateSolidBrush(COLORREF(0x00F0_F0F0))),
            });
        });
        refresh();
    }
}

fn centre(window: HWND) {
    unsafe {
        let mut frame = RECT::default();
        if GetWindowRect(window, &mut frame).is_err() {
            return;
        }
        let width = frame.right - frame.left;
        let height = frame.bottom - frame.top;
        let _ = SetWindowPos(
            window,
            None,
            (GetSystemMetrics(SM_CXSCREEN) - width) / 2,
            (GetSystemMetrics(SM_CYSCREEN) - height) / 2,
            0,
            0,
            SWP_NOSIZE | SWP_NOZORDER,
        );
    }
}

fn refresh() {
    PANEL.with(|slot| {
        let panel = slot.borrow();
        let Some(panel) = panel.as_ref() else { return };
        for (track, readout, row) in &panel.sliders {
            let value = row.family.colour(panel.palette)[row.channel];
            unsafe {
                SendMessageW(*track, TBM_SETRANGE, WPARAM(1), LPARAM((255_isize) << 16));
                SendMessageW(*track, TBM_SETPOS, WPARAM(1), LPARAM(value as isize));
                let text = wide(&value.to_string());
                let _ = SetWindowTextW(*readout, PCWSTR(text.as_ptr()));
                let _ = InvalidateRect(*readout, None, true);
            }
        }
    });
}

fn replace(palette: PaletteTargets, notify: bool) {
    let changed = PANEL.with(|slot| {
        let mut panel = slot.borrow_mut();
        let Some(panel) = panel.as_mut() else {
            return false;
        };
        if panel.palette == palette {
            return false;
        }
        panel.palette = palette;
        true
    });
    if !changed {
        return;
    }
    refresh();
    if notify {
        // Only the first unconsumed change needs to wake the overlay. Later
        // slider messages replace the value it will pick up.
        let should_wake = PENDING.with(|slot| {
            let mut pending = slot.borrow_mut();
            let should_wake = pending.is_none();
            *pending = Some(palette);
            should_wake
        });
        if should_wake {
            let app_window = PANEL.with(|slot| {
                slot.borrow()
                    .as_ref()
                    .map_or(HWND(std::ptr::null_mut()), |panel| panel.app_window)
            });
            if !app_window.0.is_null() {
                unsafe {
                    let _ = PostMessageW(app_window, WM_PALETTE_CHANGED, WPARAM(0), LPARAM(0));
                }
            }
        }
    }
}

extern "system" fn wndproc(window: HWND, message: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    unsafe {
        match message {
            WM_HSCROLL => {
                let track = HWND(lp.0 as *mut std::ffi::c_void);
                let updated = PANEL.with(|slot| {
                    let panel = slot.borrow();
                    let panel = panel.as_ref()?;
                    let (_, _, row) = panel
                        .sliders
                        .iter()
                        .find(|(handle, _, _)| *handle == track)?;
                    let position = SendMessageW(track, TBM_GETPOS, WPARAM(0), LPARAM(0)).0 as i32;
                    let mut colour = row.family.colour(panel.palette);
                    colour[row.channel] = position.clamp(0, 255) as u8;
                    Some(row.family.with_colour(panel.palette, colour))
                });
                if let Some(updated) = updated {
                    replace(updated, true);
                }
                LRESULT(0)
            }
            WM_COMMAND => {
                match (wp.0 & 0xFFFF) as usize {
                    ID_RESET => {
                        let original =
                            PANEL.with(|slot| slot.borrow().as_ref().map(|panel| panel.original));
                        if let Some(original) = original {
                            replace(original, true);
                        }
                    }
                    ID_DONE => {
                        let _ = ShowWindow(window, SW_HIDE);
                    }
                    _ => {}
                }
                LRESULT(0)
            }
            WM_CTLCOLORSTATIC => {
                let dc = HDC(wp.0 as *mut std::ffi::c_void);
                SetBkMode(dc, TRANSPARENT);
                SetTextColor(dc, COLORREF(0x0020_2020));
                PANEL.with(|slot| {
                    let panel = slot.borrow();
                    let brush = panel
                        .as_ref()
                        .map_or(HBRUSH(std::ptr::null_mut()), |panel| panel.background.0);
                    LRESULT(brush.0 as isize)
                })
            }
            WM_ERASEBKGND => {
                let dc = HDC(wp.0 as *mut std::ffi::c_void);
                let mut client = RECT::default();
                let _ = GetClientRect(window, &mut client);
                PANEL.with(|slot| {
                    let panel = slot.borrow();
                    let brush = panel
                        .as_ref()
                        .map_or(HBRUSH(std::ptr::null_mut()), |panel| panel.background.0);
                    if !brush.is_invalid() {
                        FillRect(dc, &client, brush);
                    }
                });
                LRESULT(1)
            }
            WM_CLOSE => {
                let _ = ShowWindow(window, SW_HIDE);
                LRESULT(0)
            }
            WM_DESTROY => {
                PANEL.with(|slot| {
                    if let Some(panel) = slot.borrow_mut().take() {
                        let _ = DeleteObject(panel.body.0);
                        let _ = DeleteObject(panel.heading.0);
                        let _ = DeleteObject(panel.background.0);
                    }
                });
                LRESULT(0)
            }
            _ => DefWindowProcW(window, message, wp, lp),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn palette() -> PaletteTargets {
        PaletteTargets::new([91, 45, 22], [223, 127, 42], [250, 238, 220])
    }

    #[test]
    fn the_hidden_window_builds_all_nine_trackbars() {
        PENDING.with(|slot| *slot.borrow_mut() = None);
        let window = create(HWND(std::ptr::null_mut()), palette(), palette())
            .expect("the palette lab did not build");
        assert!(!unsafe { IsWindowVisible(window) }.as_bool());
        let (count, intact) = PANEL.with(|slot| {
            let panel = slot.borrow();
            let panel = panel.as_ref().expect("the panel was not stored");
            (
                panel.sliders.len(),
                panel
                    .sliders
                    .iter()
                    .all(|(track, readout, _)| !track.is_invalid() && !readout.is_invalid()),
            )
        });
        assert_eq!(count, ROWS.len());
        assert!(intact, "a trackbar or readout failed to create");

        let track = PANEL.with(|slot| slot.borrow().as_ref().unwrap().sliders[0].0);
        unsafe {
            SendMessageW(track, TBM_SETPOS, WPARAM(1), LPARAM(0));
            wndproc(window, WM_HSCROLL, WPARAM(0), LPARAM(track.0 as isize));
        }
        assert_eq!(
            take_pending().expect("the slider reported nothing").dark[0],
            0
        );

        unsafe {
            wndproc(window, WM_COMMAND, WPARAM(ID_RESET), LPARAM(0));
            let _ = DestroyWindow(window);
        }
        assert_eq!(take_pending(), Some(palette()));
    }

    #[test]
    fn closing_hides_instead_of_destroying() {
        let window = create(HWND(std::ptr::null_mut()), palette(), palette())
            .expect("the palette lab did not build");
        unsafe {
            let _ = ShowWindow(window, SW_SHOW);
            wndproc(window, WM_CLOSE, WPARAM(0), LPARAM(0));
        }
        assert!(!unsafe { IsWindowVisible(window) }.as_bool());
        assert!(PANEL.with(|slot| slot.borrow().is_some()));
        unsafe {
            let _ = DestroyWindow(window);
        }
    }
}
