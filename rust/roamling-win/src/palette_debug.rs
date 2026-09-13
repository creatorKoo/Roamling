// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

//! Mixing a colour by hand.
//!
//! It deliberately follows `tuning.rs`: one Win32 class, common-controls
//! trackbars, thread-local ownership, a pending value drained by the main tick,
//! and a `create` seam that lets tests inspect a real hidden window.
//!
//! Reached from the tray menu only while Alt is held. The presets beside it
//! are what most people want; this is for everyone else, so it can afford to
//! be long as long as every row says what it does.

use crate::strings::localized;
use roamling_core::{Palette, PaletteTargets};
use std::cell::RefCell;

use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, CreateFontW, CreateSolidBrush, DeleteObject, EndPaint, FillRect, FrameRect,
    InvalidateRect, SetBkMode, SetTextColor, UpdateWindow, CLEARTYPE_QUALITY, CLIP_DEFAULT_PRECIS,
    DEFAULT_CHARSET, DEFAULT_PITCH, FF_DONTCARE, FW_NORMAL, FW_SEMIBOLD, HBRUSH, HDC, HFONT,
    OUT_DEFAULT_PRECIS, PAINTSTRUCT, TRANSPARENT,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Controls::Dialogs::{
    ChooseColorW, CC_ANYCOLOR, CC_FULLOPEN, CC_RGBINIT, CHOOSECOLORW,
};
use windows::Win32::UI::Controls::{
    InitCommonControlsEx, SetScrollInfo, ICC_BAR_CLASSES, INITCOMMONCONTROLSEX, TBM_SETPOS,
    TBM_SETRANGE, TBS_HORZ, TBS_NOTICKS,
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
/// One per group, so the button knows which of the three it speaks for.
const ID_PICK: usize = 300;

/// Which of the four numbers a slider drives.
///
/// This used to be nine: three colour families times R, G and B. The answer
/// cats say a palette is not three colours -- it is one hue, a lightness ramp
/// and a chroma, applied to the marking alone (`docs/palette.md` §0). Fewer
/// sliders and each one means something you can see.
#[derive(Clone, Copy, PartialEq)]
enum Axis {
    Hue,
    /// Only the marking gets one. The answer cats all held a single hue, so
    /// there is no evidence for a gradient on the body or the eyes, and the
    /// window was asked to get simpler rather than longer.
    HueEnd,
    LightLow,
    LightHigh,
    Chroma,
}

impl Axis {
    fn value(self, palette: PaletteTargets) -> f32 {
        match self {
            Self::Hue => palette.hue,
            Self::HueEnd => palette.hue_end,
            Self::LightLow => palette.light_low,
            Self::LightHigh => palette.light_high,
            Self::Chroma => palette.chroma,
        }
    }

    fn with_value(self, palette: PaletteTargets, value: f32) -> PaletteTargets {
        match self {
            // Moving the start alone would leave a gradient nobody asked for,
            // so a part without its own end slider keeps the two together.
            Self::Hue => PaletteTargets {
                hue: value,
                hue_end: if palette.hue_end == palette.hue {
                    value
                } else {
                    palette.hue_end
                },
                ..palette
            },
            Self::HueEnd => PaletteTargets {
                hue_end: value,
                ..palette
            },
            // The ramp cannot cross itself: dragging one end past the other
            // would invert the shading, which reads as a different cat rather
            // than a differently coloured one.
            Self::LightLow => PaletteTargets {
                light_low: value.min(palette.light_high),
                ..palette
            },
            Self::LightHigh => PaletteTargets {
                light_high: value.max(palette.light_low),
                ..palette
            },
            Self::Chroma => PaletteTargets {
                chroma: value,
                ..palette
            },
        }
    }
}

/// Which of the three things a slider steers.
///
/// Separate because the answer cats treat them separately: `blue` tinted the
/// body with the fur and `sky` left it cream, and twelve of thirteen kept brown
/// eyes while `sky` alone went blue.
#[derive(Clone, Copy, PartialEq)]
enum Part {
    Marking,
    Body,
    Eye,
}

impl Part {
    fn targets(self, palette: Palette) -> PaletteTargets {
        match self {
            Self::Marking => palette.marking,
            Self::Body => palette.body,
            Self::Eye => palette.eye,
        }
    }

    fn with_targets(self, palette: Palette, aim: PaletteTargets) -> Palette {
        match self {
            Self::Marking => Palette {
                marking: aim,
                ..palette
            },
            Self::Body => Palette { body: aim, ..palette },
            Self::Eye => Palette { eye: aim, ..palette },
        }
    }
}

#[derive(Clone, Copy)]
struct Row {
    part: Part,
    axis: Axis,
    label: &'static str,
    /// Each axis has its own units, so the trackbar range is per row rather
    /// than the 0-255 every channel shared.
    maximum: i32,
}

/// Title key, hint key, and which part the rows steer.
const GROUPS: [(Part, &str, &str); 3] = [
    (
        Part::Marking,
        "palette.section.marking",
        "palette.hint.marking",
    ),
    (Part::Body, "palette.section.body", "palette.hint.body"),
    (Part::Eye, "palette.section.eye", "palette.hint.eye"),
];

fn rows_for(part: Part) -> Vec<Row> {
    let mut rows = vec![Row {
        part,
        axis: Axis::Hue,
        label: "palette.hue",
        maximum: 359,
    }];
    if part == Part::Marking {
        rows.push(Row {
            part,
            axis: Axis::HueEnd,
            label: "palette.hueEnd",
            maximum: 359,
        });
    }
    rows.push(Row {
        part,
        axis: Axis::LightLow,
        label: "palette.dark",
        maximum: 100,
    });
    rows.push(Row {
        part,
        axis: Axis::LightHigh,
        label: "palette.light",
        maximum: 100,
    });
    rows.push(Row {
        part,
        axis: Axis::Chroma,
        label: "palette.chroma",
        maximum: 255,
    });
    rows
}

/// Every slider the window builds, across all three groups.
fn total_rows() -> usize {
    GROUPS.iter().map(|(part, _, _)| rows_for(*part).len()).sum()
}

struct Panel {
    window: HWND,
    app_window: HWND,
    palette: Palette,
    original: Palette,
    sliders: Vec<(HWND, HWND, Row)>,
    /// Where to paint each group's colour, and whose colour it is. Painted by
    /// the window rather than made of child controls: the colour changes on
    /// every slider drag, and a static control would mean a brush to rebuild
    /// and free each time. Held in content coordinates, which is what the
    /// scroll offset is measured against.
    swatches: Vec<(RECT, Part)>,
    /// How far down the content has been scrolled, and how tall the content is.
    /// Thirteen sliders with their explanations is a long window, and it is
    /// taller than a laptop screen -- so it opens as tall as the screen allows
    /// and the rest is scrolled to.
    scroll: i32,
    content: i32,
    body: HFHandle,
    heading: HFHandle,
    background: HBHandle,
}

struct HFHandle(HFONT);
struct HBHandle(HBRUSH);

thread_local! {
    static PANEL: RefCell<Option<Panel>> = const { RefCell::new(None) };
    static PENDING: RefCell<Option<Palette>> = const { RefCell::new(None) };
    /// The sixteen slots down the left of the system dialog. Kept for the life
    /// of the process so a colour mixed for the fur is still there when the
    /// eyes are picked.
    static CUSTOM: RefCell<[COLORREF; 16]> = const { RefCell::new([COLORREF(0x00FF_FFFF); 16]) };
}

pub fn take_pending() -> Option<Palette> {
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
pub fn show(app_window: HWND, current: Palette, original: Palette) {
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
    current: Palette,
    original: Palette,
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

        let title = wide(localized("palette.window.title"));
        let window = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            class,
            PCWSTR(title.as_ptr()),
            WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_VSCROLL,
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

fn build(window: HWND, app_window: HWND, palette: Palette, original: Palette) {
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
        let channel_width = scaled(66);
        let track_width = scaled(270);
        let value_width = scaled(44);
        let gap = scaled(8);
        let row_height = scaled(28);
        let width = margin * 2 + channel_width + track_width + value_width + gap * 2;
        let instance = GetModuleHandleW(None).unwrap_or_default();
        let mut y = margin;
        let mut sliders = Vec::with_capacity(total_rows());
        let mut swatches = Vec::with_capacity(GROUPS.len());
        let pick_width = scaled(96);
        let swatch_width = scaled(40);

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
            localized("palette.header"),
            margin,
            y,
            width - margin * 2,
            row_height,
            0,
            heading,
        );
        y += row_height;
        label(
            localized("palette.footer"),
            margin,
            y,
            width - margin * 2,
            scaled(82),
            0,
            body,
        );
        y += scaled(88);

        for (index, (part, title, hint)) in GROUPS.into_iter().enumerate() {
            // Title on the left, then the colour it is wearing, then the way to
            // change it. The swatch sits beside the button so that what you are
            // about to change and what it looks like now are the same glance.
            let pick_x = width - margin - pick_width;
            let swatch_x = pick_x - swatch_width - gap;
            label(
                localized(title),
                margin,
                y,
                swatch_x - margin - gap,
                row_height,
                0,
                heading,
            );
            swatches.push((
                RECT {
                    left: swatch_x,
                    top: y + scaled(2),
                    right: swatch_x + swatch_width,
                    bottom: y + row_height - scaled(2),
                },
                part,
            ));
            let content = wide(localized("palette.pick"));
            let pick = CreateWindowExW(
                WINDOW_EX_STYLE(0),
                w!("BUTTON"),
                PCWSTR(content.as_ptr()),
                WINDOW_STYLE(WS_CHILD.0 | WS_VISIBLE.0 | WS_TABSTOP.0 | BS_PUSHBUTTON as u32),
                pick_x,
                y,
                pick_width,
                row_height,
                window,
                HMENU((ID_PICK + index) as *mut std::ffi::c_void),
                instance,
                None,
            )
            .unwrap_or_default();
            SendMessageW(pick, WM_SETFONT, WPARAM(body.0 as usize), LPARAM(1));
            y += row_height;
            label(
                localized(hint),
                margin,
                y,
                width - margin * 2,
                scaled(36),
                0,
                body,
            );
            y += scaled(40);
            for row in rows_for(part) {
                let index = sliders.len();
                label(
                    localized(row.label),
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

        // What each kind of row does, once, rather than beside all thirteen.
        for key in [
            "palette.hint.pick",
            "palette.hint.hueEnd",
            "palette.hint.range",
            "palette.hint.chroma",
        ] {
            label(
                localized(key),
                margin,
                y,
                width - margin * 2,
                scaled(36),
                0,
                body,
            );
            y += scaled(38);
        }
        y += gap;

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
        button(localized("palette.reset"), margin, ID_RESET);
        button(
            localized("palette.done"),
            width - margin - button_width,
            ID_DONE,
        );
        y += button_height + margin;

        // As tall as the content, unless the screen says otherwise. The scroll
        // bar takes its own width, so the content keeps all of it.
        let content = y;
        let mut work = RECT::default();
        let room = if SystemParametersInfoW(
            SPI_GETWORKAREA,
            0,
            Some(&mut work as *mut RECT as *mut std::ffi::c_void),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        )
        .is_ok()
        {
            work.bottom - work.top
        } else {
            GetSystemMetrics(SM_CYSCREEN)
        };
        let mut frame = RECT {
            left: 0,
            top: 0,
            right: width + GetSystemMetrics(SM_CXVSCROLL),
            bottom: content.min(room * 85 / 100),
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
                swatches,
                scroll: 0,
                content,
                body: HFHandle(body),
                heading: HFHandle(heading),
                background: HBHandle(CreateSolidBrush(COLORREF(0x00F0_F0F0))),
            });
        });
        update_scrollbar(window);
        refresh();
    }
}

/// Tell the bar how much there is and how much of it shows.
fn update_scrollbar(window: HWND) {
    let Some((content, scroll)) = PANEL.with(|slot| {
        slot.borrow()
            .as_ref()
            .map(|panel| (panel.content, panel.scroll))
    }) else {
        return;
    };
    unsafe {
        let mut client = RECT::default();
        let _ = GetClientRect(window, &mut client);
        let info = SCROLLINFO {
            cbSize: std::mem::size_of::<SCROLLINFO>() as u32,
            fMask: SCROLLINFO_MASK(SIF_RANGE.0 | SIF_PAGE.0 | SIF_POS.0),
            nMin: 0,
            nMax: (content - 1).max(0),
            nPage: (client.bottom - client.top).max(1) as u32,
            nPos: scroll,
            nTrackPos: 0,
        };
        SetScrollInfo(window, SB_VERT, &info, true);
    }
}

/// Move the content to an offset, taking the children and the swatches with it.
fn scroll_to(window: HWND, wanted: i32) {
    let moved = PANEL.with(|slot| {
        let mut panel = slot.borrow_mut();
        let Some(panel) = panel.as_mut() else { return 0 };
        let mut client = RECT::default();
        unsafe {
            let _ = GetClientRect(window, &mut client);
        }
        let furthest = (panel.content - (client.bottom - client.top)).max(0);
        let wanted = wanted.clamp(0, furthest);
        let moved = panel.scroll - wanted;
        panel.scroll = wanted;
        moved
    });
    if moved == 0 {
        return;
    }
    unsafe {
        // The swatches are painted by the window, so scrolling the pixels is
        // not enough on its own -- `paint_swatches` subtracts the offset too.
        ScrollWindowEx(
            window,
            0,
            moved,
            None,
            None,
            None,
            None,
            SW_SCROLLCHILDREN | SW_INVALIDATE,
        );
    }
    update_scrollbar(window);
}

fn scrolled_by(window: HWND, steps: i32) {
    let at = PANEL.with(|slot| slot.borrow().as_ref().map_or(0, |panel| panel.scroll));
    scroll_to(window, at + steps);
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
            let value = row.axis.value(row.part.targets(panel.palette)).round() as i32;
            unsafe {
                SendMessageW(
                    *track,
                    TBM_SETRANGE,
                    WPARAM(1),
                    LPARAM((row.maximum as isize) << 16),
                );
                SendMessageW(*track, TBM_SETPOS, WPARAM(1), LPARAM(value as isize));
                let text = wide(&value.to_string());
                let _ = SetWindowTextW(*readout, PCWSTR(text.as_ptr()));
                let _ = InvalidateRect(*readout, None, true);
            }
        }
        // The swatches follow the sliders, so dragging one shows its colour
        // without the pet having to redraw first.
        for (frame, _) in &panel.swatches {
            let shown = RECT {
                top: frame.top - panel.scroll,
                bottom: frame.bottom - panel.scroll,
                ..*frame
            };
            unsafe {
                let _ = InvalidateRect(panel.window, Some(&shown), false);
            }
        }
    });
}

/// What the system dialog and the swatch both mean by a group's colour.
fn swatch_colour(part: Part, palette: Palette) -> COLORREF {
    let [red, green, blue] = part.targets(palette).middle();
    COLORREF(u32::from(red) | (u32::from(green) << 8) | (u32::from(blue) << 16))
}

fn paint_swatches(window: HWND) {
    unsafe {
        let mut paint = PAINTSTRUCT::default();
        let dc = BeginPaint(window, &mut paint);
        PANEL.with(|slot| {
            let panel = slot.borrow();
            let Some(panel) = panel.as_ref() else { return };
            let edge = CreateSolidBrush(COLORREF(0x0090_9090));
            for (frame, part) in &panel.swatches {
                let shown = RECT {
                    top: frame.top - panel.scroll,
                    bottom: frame.bottom - panel.scroll,
                    ..*frame
                };
                let fill = CreateSolidBrush(swatch_colour(*part, panel.palette));
                FillRect(dc, &shown, fill);
                FrameRect(dc, &shown, edge);
                let _ = DeleteObject(fill);
            }
            let _ = DeleteObject(edge);
        });
        let _ = EndPaint(window, &paint);
    }
}

/// The system's colour dialog, aimed at one group.
///
/// A group is five numbers and this gives back one colour, so `aimed_at` says
/// which of the five it is -- the middle of the shading, keeping the width of
/// the ramp (user decision 2026-09-13). The sliders are left in place and move
/// to match, because the ramp's width and the rainbow's sweep can only be made
/// there, and they are what the presets were built from.
fn pick(window: HWND, part: Part) {
    let Some(palette) = PANEL.with(|slot| slot.borrow().as_ref().map(|panel| panel.palette)) else {
        return;
    };
    let chosen = CUSTOM.with(|slot| {
        let mut custom = slot.borrow_mut();
        let mut chooser = CHOOSECOLORW {
            lStructSize: std::mem::size_of::<CHOOSECOLORW>() as u32,
            hwndOwner: window,
            rgbResult: swatch_colour(part, palette),
            lpCustColors: custom.as_mut_ptr(),
            // Opened on the mixing side rather than the sixteen basic colours:
            // a cat's colour is hardly ever one of those.
            Flags: CC_RGBINIT | CC_FULLOPEN | CC_ANYCOLOR,
            ..Default::default()
        };
        let picked = unsafe { ChooseColorW(&mut chooser) }.as_bool();
        picked.then_some(chooser.rgbResult)
    });
    let Some(COLORREF(packed)) = chosen else {
        return;
    };
    let colour = [
        (packed & 0xFF) as u8,
        ((packed >> 8) & 0xFF) as u8,
        ((packed >> 16) & 0xFF) as u8,
    ];
    let aim = part.targets(palette).aimed_at(colour);
    replace(part.with_targets(palette, aim), true);
}

fn replace(palette: Palette, notify: bool) {
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
                    let value = position.clamp(0, row.maximum) as f32;
                    let aim = row.axis.with_value(row.part.targets(panel.palette), value);
                    Some(row.part.with_targets(panel.palette, aim))
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
                    command if command >= ID_PICK && command < ID_PICK + GROUPS.len() => {
                        pick(window, GROUPS[command - ID_PICK].0);
                    }
                    _ => {}
                }
                LRESULT(0)
            }
            WM_PAINT => {
                paint_swatches(window);
                LRESULT(0)
            }
            WM_VSCROLL => {
                let line = GetSystemMetrics(SM_CYVSCROLL) * 2;
                let mut client = RECT::default();
                let _ = GetClientRect(window, &mut client);
                let page = client.bottom - client.top;
                let at = PANEL.with(|slot| slot.borrow().as_ref().map_or(0, |panel| panel.scroll));
                let wanted = match SCROLLBAR_COMMAND((wp.0 & 0xFFFF) as i32) {
                    SB_LINEUP => at - line,
                    SB_LINEDOWN => at + line,
                    SB_PAGEUP => at - page,
                    SB_PAGEDOWN => at + page,
                    SB_TOP => 0,
                    SB_BOTTOM => i32::MAX,
                    // The thumb reports where it is being dragged to, and the
                    // sixteen bits of the message cannot carry it -- a window
                    // this tall overflows them. Ask the bar instead.
                    SB_THUMBTRACK | SB_THUMBPOSITION => {
                        let mut info = SCROLLINFO {
                            cbSize: std::mem::size_of::<SCROLLINFO>() as u32,
                            fMask: SIF_TRACKPOS,
                            ..Default::default()
                        };
                        if GetScrollInfo(window, SB_VERT, &mut info).is_ok() {
                            info.nTrackPos
                        } else {
                            at
                        }
                    }
                    _ => at,
                };
                scroll_to(window, wanted);
                LRESULT(0)
            }
            WM_MOUSEWHEEL => {
                let notches = (((wp.0 >> 16) & 0xFFFF) as i16) as i32 / WHEEL_DELTA as i32;
                scrolled_by(window, -notches * GetSystemMetrics(SM_CYVSCROLL) * 3);
                LRESULT(0)
            }
            WM_SIZE => {
                // Growing the window can leave the content scrolled past its
                // own end, which shows as a band of nothing under the buttons.
                let at = PANEL.with(|slot| slot.borrow().as_ref().map_or(0, |panel| panel.scroll));
                scroll_to(window, at);
                update_scrollbar(window);
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

    fn palette() -> Palette {
        Palette::new(
            PaletteTargets::new(22.0, 17.0, 53.0, 90.0),
            PaletteTargets::new(38.0, 86.0, 95.0, 33.0),
            PaletteTargets::new(21.0, 0.0, 78.0, 17.0),
        )
    }

    #[test]
    fn the_hidden_window_builds_a_trackbar_for_every_axis() {
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
        assert_eq!(count, total_rows());
        assert!(intact, "a trackbar or readout failed to create");

        // And a swatch per group, showing the same colour the recolour would
        // put on the cat. A swatch built from different arithmetic would be a
        // picture of a cat that does not exist.
        let swatches = PANEL.with(|slot| slot.borrow().as_ref().unwrap().swatches.clone());
        assert_eq!(swatches.len(), GROUPS.len());
        assert_eq!(
            swatch_colour(Part::Body, palette()),
            COLORREF(
                u32::from(palette().body.middle()[0])
                    | (u32::from(palette().body.middle()[1]) << 8)
                    | (u32::from(palette().body.middle()[2]) << 16)
            )
        );
        let mut client = RECT::default();
        let _ = unsafe { GetClientRect(window, &mut client) };
        for (index, (frame, _)) in swatches.iter().enumerate() {
            assert!(frame.right > frame.left && frame.bottom > frame.top, "{frame:?}");
            assert!(frame.right <= client.right, "{frame:?} vs {client:?}");
            // The button has to carry the id the command routing reads, or
            // pressing it does nothing and the window looks broken.
            let button = unsafe { GetDlgItem(window, (ID_PICK + index) as i32) };
            let button = button.expect("the pick button failed to create");
            let mut bounds = RECT::default();
            let _ = unsafe { GetWindowRect(button, &mut bounds) };
            assert!(bounds.right > bounds.left, "{bounds:?}");
        }

        let track = PANEL.with(|slot| slot.borrow().as_ref().unwrap().sliders[0].0);
        unsafe {
            SendMessageW(track, TBM_SETPOS, WPARAM(1), LPARAM(0));
            wndproc(window, WM_HSCROLL, WPARAM(0), LPARAM(track.0 as isize));
        }
        assert_eq!(
            take_pending()
                .expect("the slider reported nothing")
                .marking
                .hue,
            0.0
        );

        unsafe {
            wndproc(window, WM_COMMAND, WPARAM(ID_RESET), LPARAM(0));
            let _ = DestroyWindow(window);
        }
        assert_eq!(take_pending(), Some(palette()));
    }

    /// Thirteen sliders and their explanations are taller than a laptop screen,
    /// so the window stops at the screen and scrolls. The numbers here are
    /// whatever this machine's screen allows, so what is asserted is the
    /// relation between them rather than any one of them.
    #[test]
    fn the_window_scrolls_to_its_end_and_back() {
        let window = create(HWND(std::ptr::null_mut()), palette(), palette())
            .expect("the palette lab did not build");
        let content = PANEL.with(|slot| slot.borrow().as_ref().unwrap().content);
        assert!(content > 0, "the content has no height");
        let mut client = RECT::default();
        let _ = unsafe { GetClientRect(window, &mut client) };
        let page = client.bottom - client.top;
        assert!(page > 0 && page <= content, "{page} against {content}");

        let at = |_: ()| PANEL.with(|slot| slot.borrow().as_ref().unwrap().scroll);
        assert_eq!(at(()), 0, "it should open at the top");
        wndproc(window, WM_VSCROLL, WPARAM(SB_BOTTOM.0 as usize), LPARAM(0));
        assert_eq!(at(()), content - page, "the end is the last page, no further");
        wndproc(window, WM_VSCROLL, WPARAM(SB_TOP.0 as usize), LPARAM(0));
        assert_eq!(at(()), 0, "the top is the top");

        // And the swatches move with it, or they would be painted where the
        // heading no longer is.
        wndproc(window, WM_VSCROLL, WPARAM(SB_BOTTOM.0 as usize), LPARAM(0));
        let (frame, scroll) = PANEL.with(|slot| {
            let panel = slot.borrow();
            let panel = panel.as_ref().unwrap();
            (panel.swatches[0].0, panel.scroll)
        });
        assert_eq!(frame.top - scroll, frame.top - (content - page));

        unsafe {
            let _ = DestroyWindow(window);
        }
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
