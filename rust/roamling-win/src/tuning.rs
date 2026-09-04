// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

//! The behaviour tuning panel.
//!
//! Ported from `RuntimeTuningWindowController`, which is SwiftUI. There is no
//! SwiftUI here, so this is the same panel in plain Win32: a static label, a
//! trackbar and a value readout per row, in the three sections the macOS panel
//! groups them into, with Reset and Done underneath.
//!
//! **The bounds are not this file's.** `RuntimeTuning::bounds` owns them, and
//! the comment on it says why: a second table drifts, and once did -- the panel
//! offered a catch arm up to 140 while the model accepted 360. Every range here
//! is read from the core, and re-read after every change, because one of them
//! moves with another value.
//!
//! Changes leave through `take_pending`, which the tick drains. Reaching into
//! the app from here would mean touching the runtime from inside a nested
//! message loop, and `wndproc` in `main.rs` explains why that is not safe.

use crate::strings::{localized, localized_format};
use roamling_core::{RuntimeTuning, RuntimeTuningKey};
use std::cell::RefCell;

use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    CreateFontW, CreateSolidBrush, DeleteObject, FillRect, InvalidateRect, SetBkMode,
    SetTextColor, UpdateWindow, CLEARTYPE_QUALITY, CLIP_DEFAULT_PRECIS, DEFAULT_CHARSET,
    DEFAULT_PITCH, FF_DONTCARE, FW_NORMAL, FW_SEMIBOLD, HBRUSH, HDC, HFONT,
    OUT_DEFAULT_PRECIS, TRANSPARENT,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Controls::{
    InitCommonControlsEx, ICC_BAR_CLASSES, INITCOMMONCONTROLSEX, TBM_SETPOS, TBM_SETRANGE,
    TBS_HORZ, TBS_NOTICKS,
};
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::WindowsAndMessaging::*;

/// Every row the panel shows, in the order `RuntimeTuningView` lists them.
///
/// The step is the granularity the slider offers, which is also what decides
/// how many notches the trackbar has -- Win32 trackbars are integer-positioned,
/// so the step is what turns a range of doubles into positions.
struct Row {
    title: &'static str,
    key: RuntimeTuningKey,
    step: f64,
    unit: Unit,
}

#[derive(Clone, Copy)]
enum Unit {
    PointsPerSecond,
    Seconds,
    SecondsDecimal,
    Percent,
    Points,
    Multiplier,
}

impl Unit {
    fn text(self, value: f64) -> String {
        match self {
            Self::PointsPerSecond => {
                localized_format("unit.speed", &[&format!("{}", value.round() as i64)])
            }
            Self::Seconds => {
                localized_format("unit.seconds", &[&format!("{}", value.round() as i64)])
            }
            Self::SecondsDecimal => localized_format("unit.secondsPrecise", &[&format!("{value:.2}")]),
            // The only readout the macOS panel writes out rather than reading
            // from the table, so it stays written out here too.
            Self::Percent => format!("{}%", (value * 100.0).round() as i64),
            Self::Points => {
                localized_format("unit.points", &[&format!("{}", value.round() as i64)])
            }
            Self::Multiplier => localized_format("unit.multiplier", &[&format!("{value:.2}")]),
        }
    }
}

/// A heading, or a row, or a paragraph of explanation. Laying the panel out as
/// a list keeps it in step with the macOS one, which is also a list.
enum Item {
    Section(&'static str),
    Slider(Row),
    Note(&'static str),
}

fn items() -> Vec<Item> {
    use RuntimeTuningKey as K;
    let slider = |title, key, step, unit| Item::Slider(Row { title, key, step, unit });
    vec![
        Item::Section("tuning.section.movement"),
        slider("tuning.walkSpeed", K::WalkingSpeed, 1.0, Unit::PointsPerSecond),
        slider("tuning.wanderPause", K::WanderPause, 1.0, Unit::Seconds),
        slider("tuning.crossDisplay", K::CrossDisplayWanderChance, 0.01, Unit::Percent),
        slider("tuning.idleBeforeRest", K::IdleBeforeRest, 5.0, Unit::Seconds),
        Item::Section("tuning.section.pointer"),
        slider("tuning.noticeDistance", K::PointerAwarenessDistance, 5.0, Unit::Points),
        slider("tuning.evadeSpeed", K::EvadeSpeedScale, 0.05, Unit::Multiplier),
        slider("tuning.catchArm", K::CatchArmDistance, 2.0, Unit::Points),
        slider("tuning.catchSpeed", K::CatchApproachSpeed, 10.0, Unit::PointsPerSecond),
        slider("tuning.catchWindow", K::CatchWindow, 0.05, Unit::SecondsDecimal),
        slider("tuning.hitRegion", K::HitRegionScale, 0.01, Unit::Multiplier),
        Item::Note("tuning.pointerNote"),
        Item::Section("tuning.section.advanced"),
        slider("tuning.gaitCadence", K::GaitCadence, 0.05, Unit::Multiplier),
        Item::Note("tuning.gaitCadenceNote"),
    ]
}

/// Two the `windows` crate does not generate. `TBM_GETPOS` is `WM_USER`, the
/// first of the trackbar messages; `SS_RIGHT` is the static style that right
/// aligns, which is what lines the readouts up into a column.
const TBM_GETPOS: u32 = WM_USER;
const SS_RIGHT: u32 = 0x0000_0002;

const ID_RESET: usize = 1;
const ID_DONE: usize = 2;
/// Trackbars and their readouts are numbered from here, one pair per slider.
const ID_SLIDER: usize = 100;
const ID_VALUE: usize = 200;

struct Panel {
    window: HWND,
    tuning: RuntimeTuning,
    /// The trackbar and the readout for each slider row, in `items()` order.
    sliders: Vec<(HWND, HWND, Row)>,
    body: HFHandle,
    heading: HFHandle,
    background: HBHandle,
}

/// `HFONT` and `HBRUSH` are not `Send`, and neither is anything here -- the
/// panel lives on the message-loop thread only. These wrappers exist so the
/// thread-local can own them and free them on close.
struct HFHandle(HFONT);
struct HBHandle(HBRUSH);

thread_local! {
    static PANEL: RefCell<Option<Panel>> = const { RefCell::new(None) };
    /// What the user has changed and the tick has not applied yet.
    static PENDING: RefCell<Option<RuntimeTuning>> = const { RefCell::new(None) };
}

/// Whatever the panel changed since the last tick, if anything.
pub fn take_pending() -> Option<RuntimeTuning> {
    PENDING.with(|slot| slot.borrow_mut().take())
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Puts the application icon on a window's title bar and in Alt-Tab.
///
/// Resource id 1 is the icon `build.rs` compiles in -- the same file the macOS
/// app uses. `LoadImageW` is asked for each size rather than letting the shell
/// scale one, because the icon carries an image drawn at each size and picking
/// the right one is the whole point of having them.
fn set_icon(window: HWND, instance: windows::Win32::Foundation::HMODULE) {
    const ICON: PCWSTR = PCWSTR(1 as *const u16);
    unsafe {
        for (which, metric) in [
            (ICON_SMALL, SM_CXSMICON),
            (ICON_BIG, SM_CXICON),
        ] {
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

/// The authored value for this row, which is what the middle of the track means.
fn anchor(row: &Row, tuning: &RuntimeTuning) -> f64 {
    let (lower, upper) = tuning.limits(row.key);
    let default = RuntimeTuning::default().get(row.key);
    default.clamp(lower.min(upper), upper.max(lower))
}

/// Half the track. The two halves have the same number of notches, so the
/// authored value lands exactly on the middle one.
///
/// The bounds are not symmetric around the defaults -- the notice distance
/// runs 140 to 360 around a default of 170, so a straight linear track puts
/// "normal" a seventh of the way from the left and every adjustment looks like
/// a large one. Splitting the track at the default costs nothing and makes the
/// centre mean "what the pet was designed to do", with slower and faster the
/// same distance away in each direction.
///
/// The side with the wider span keeps the authored step; the narrower side ends
/// up finer than it, which is only ever more precision than was asked for.
fn half_notches(row: &Row, tuning: &RuntimeTuning) -> i32 {
    let (lower, upper) = tuning.limits(row.key);
    let middle = anchor(row, tuning);
    let below = ((middle - lower) / row.step).abs();
    let above = ((upper - middle) / row.step).abs();
    (below.max(above).round() as i32).max(1)
}

/// How many notches this row's trackbar has, and where a value sits on it.
fn positions(row: &Row, tuning: &RuntimeTuning) -> (i32, i32) {
    let half = half_notches(row, tuning);
    let (lower, upper) = tuning.limits(row.key);
    let middle = anchor(row, tuning);
    let value = tuning.get(row.key);

    let at = if value <= middle {
        let span = middle - lower;
        if span > 0.0 {
            ((value - lower) / span * f64::from(half)).round() as i32
        } else {
            0
        }
    } else {
        let span = upper - middle;
        if span > 0.0 {
            half + ((value - middle) / span * f64::from(half)).round() as i32
        } else {
            half * 2
        }
    };
    (half * 2, at.clamp(0, half * 2))
}

/// What the readout says: the value, and the authored one beside it when the
/// slider has been moved off it.
///
/// The centre of the track means "what the pet was designed to do", and without
/// this there is no way to tell a value you chose from one that came with the
/// app -- which is the question a user actually asks when a slider is not in
/// the middle.
fn readout_text(row: &Row, tuning: &RuntimeTuning) -> String {
    let value = tuning.get(row.key);
    let default = RuntimeTuning::default().get(row.key);
    if (value - default).abs() < 1e-9 {
        row.unit.text(value)
    } else {
        localized_format(
            "tuning.offDefault",
            &[&row.unit.text(value), &row.unit.text(default)],
        )
    }
}

fn value_at(row: &Row, tuning: &RuntimeTuning, position: i32) -> f64 {
    let half = half_notches(row, tuning);
    let (lower, upper) = tuning.limits(row.key);
    let middle = anchor(row, tuning);
    let position = position.clamp(0, half * 2);
    if position <= half {
        lower + (middle - lower) * f64::from(position) / f64::from(half)
    } else {
        middle + (upper - middle) * f64::from(position - half) / f64::from(half)
    }
}

/// Opens the panel, or brings it back to the front if it is already up.
pub fn show(current: RuntimeTuning) {
    let existing = PANEL.with(|slot| slot.borrow().as_ref().map(|panel| panel.window));
    if let Some(window) = existing {
        // Reopening reflects whatever the runtime holds now, which is what
        // `present(tuning:)` does before it shows the macOS window.
        replace(current, false);
        unsafe {
            let _ = ShowWindow(window, SW_SHOW);
            let _ = SetForegroundWindow(window);
        }
        return;
    }
    match create(current) {
        Ok(window) => unsafe {
            let _ = ShowWindow(window, SW_SHOW);
            let _ = UpdateWindow(window);
            let _ = SetForegroundWindow(window);
        },
        Err(error) => println!("could not open the tuning panel: {error:?}"),
    }
}

/// Builds the window without showing it, so a test can lay one out and take it
/// straight back down rather than flashing it on the user's screen.
fn create(current: RuntimeTuning) -> windows::core::Result<HWND> {
    unsafe {
        // Trackbars live in comctl32 and the class is not registered until
        // this is called. Without it every `CreateWindowExW` below fails.
        let controls = INITCOMMONCONTROLSEX {
            dwSize: std::mem::size_of::<INITCOMMONCONTROLSEX>() as u32,
            dwICC: ICC_BAR_CLASSES,
        };
        let _ = InitCommonControlsEx(&controls);

        let instance = GetModuleHandleW(None)?;
        let class = w!("RoamlingTuning");
        let registered = WNDCLASSW {
            lpfnWndProc: Some(wndproc),
            hInstance: instance.into(),
            lpszClassName: class,
            hCursor: LoadCursorW(None, IDC_ARROW)?,
            // Painted in WM_ERASEBKGND instead, so the panel matches the
            // dialog grey rather than the window default.
            hbrBackground: HBRUSH(std::ptr::null_mut()),
            ..Default::default()
        };
        // Re-registering is an error, and reopening after a close would hit it.
        RegisterClassW(&registered);

        let title = wide(localized("tuning.window.title"));
        // Sized once the DPI is known; this is a placeholder the code below
        // replaces before the window is shown.
        let window = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            class,
            PCWSTR(title.as_ptr()),
            // No maximise: the layout is a fixed column, and stretching it
            // would leave the sliders swimming in space.
            WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            520,
            640,
            None,
            None,
            instance,
            None,
        )?;

        // A window does not pick up the executable's icon on its own: the
        // resource is there, and the title bar still shows the default until
        // something says otherwise. Both sizes, because the title bar takes the
        // small one and Alt-Tab takes the large.
        set_icon(window, instance);
        build(window, current);
        Ok(window)
    }
}

/// Lays the controls out and stores the panel. Called once, on creation.
fn build(window: HWND, tuning: RuntimeTuning) {
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
        let label_width = scaled(150);
        // Wide enough for "1.30x (default 1.12x)", which is the longest thing
        // a readout has to say.
        let value_width = scaled(150);
        let track_width = scaled(200);
        let row_height = scaled(26);
        let gap = scaled(6);
        let width = margin * 2 + label_width + track_width + value_width + gap * 2;

        let instance = GetModuleHandleW(None).unwrap_or_default();
        let mut y = margin;
        let mut sliders = Vec::new();

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

        // The header and the paragraph under it, same words as the macOS panel.
        label(
            localized("tuning.header"),
            margin,
            y,
            width - margin * 2,
            row_height,
            0,
            heading,
        );
        y += row_height;
        let footer_height = scaled(52);
        label(
            localized("tuning.footer"),
            margin,
            y,
            width - margin * 2,
            footer_height,
            0,
            body,
        );
        y += footer_height + gap;

        for (index, item) in items().into_iter().enumerate() {
            match item {
                Item::Section(key) => {
                    y += gap;
                    label(
                        localized(key),
                        margin,
                        y,
                        width - margin * 2,
                        row_height,
                        0,
                        heading,
                    );
                    y += row_height;
                }
                Item::Note(key) => {
                    let height = scaled(48);
                    label(localized(key), margin, y, width - margin * 2, height, 0, body);
                    y += height + gap;
                }
                Item::Slider(row) => {
                    label(
                        localized(row.title),
                        margin,
                        y + scaled(4),
                        label_width,
                        row_height,
                        0,
                        body,
                    );
                    let track = CreateWindowExW(
                        WINDOW_EX_STYLE(0),
                        w!("msctls_trackbar32"),
                        PCWSTR::null(),
                        WINDOW_STYLE(
                            WS_CHILD.0 | WS_VISIBLE.0 | WS_TABSTOP.0 | TBS_HORZ | TBS_NOTICKS,
                        ),
                        margin + label_width + gap,
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
                        margin + label_width + track_width + gap * 2,
                        y + scaled(4),
                        value_width,
                        row_height,
                        SS_RIGHT,
                        body,
                    );
                    let _ = SetWindowLongPtrW(readout, GWLP_ID, (ID_VALUE + index) as isize);
                    sliders.push((track, readout, row));
                    y += row_height + gap;
                }
            }
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
            control
        };
        button(localized("button.resetDefaults"), margin, ID_RESET);
        button(
            localized("button.done"),
            width - margin - button_width,
            ID_DONE,
        );
        y += button_height + margin;

        // The client area is now known, so size the window around it.
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
                tuning,
                sliders,
                body: HFHandle(body),
                heading: HFHandle(heading),
                // The dialog face, so the panel does not read as a blank
                // document window.
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
        let screen_width = GetSystemMetrics(SM_CXSCREEN);
        let screen_height = GetSystemMetrics(SM_CYSCREEN);
        let _ = SetWindowPos(
            window,
            None,
            (screen_width - width) / 2,
            (screen_height - height) / 2,
            0,
            0,
            SWP_NOSIZE | SWP_NOZORDER,
        );
    }
}

/// Puts every trackbar and readout back in step with the current tuning.
///
/// Ranges are re-applied, not just positions: `CatchArmDistance`'s ceiling is
/// the notice distance, so moving one slider changes another's scale.
fn refresh() {
    PANEL.with(|slot| {
        let panel = slot.borrow();
        let Some(panel) = panel.as_ref() else { return };
        for (track, readout, row) in &panel.sliders {
            let (notches, at) = positions(row, &panel.tuning);
            unsafe {
                SendMessageW(
                    *track,
                    TBM_SETRANGE,
                    WPARAM(1),
                    LPARAM(((notches as isize) << 16) | 0),
                );
                SendMessageW(*track, TBM_SETPOS, WPARAM(1), LPARAM(at as isize));
                let text = wide(&readout_text(row, &panel.tuning));
                let _ = SetWindowTextW(*readout, PCWSTR(text.as_ptr()));
                let _ = InvalidateRect(*readout, None, true);
            }
        }
    });
}

/// Adopts a tuning, redraws, and -- unless it came from the runtime -- leaves
/// it for the tick to apply.
fn replace(tuning: RuntimeTuning, notify: bool) {
    let changed = PANEL.with(|slot| {
        let mut panel = slot.borrow_mut();
        let Some(panel) = panel.as_mut() else {
            return false;
        };
        if panel.tuning == tuning {
            return false;
        }
        panel.tuning = tuning;
        true
    });
    if !changed {
        return;
    }
    refresh();
    if notify {
        PENDING.with(|slot| *slot.borrow_mut() = Some(tuning));
    }
}

extern "system" fn wndproc(window: HWND, message: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    unsafe {
        match message {
            // Any trackbar movement: find which one, read it back, re-clamp.
            WM_HSCROLL => {
                let track = HWND(lp.0 as *mut std::ffi::c_void);
                let updated = PANEL.with(|slot| {
                    let panel = slot.borrow();
                    let panel = panel.as_ref()?;
                    let (_, _, row) = panel.sliders.iter().find(|(handle, _, _)| *handle == track)?;
                    let position = SendMessageW(track, TBM_GETPOS, WPARAM(0), LPARAM(0)).0 as i32;
                    Some(panel.tuning.with(row.key, value_at(row, &panel.tuning, position)))
                });
                if let Some(updated) = updated {
                    replace(updated, true);
                }
                LRESULT(0)
            }
            WM_COMMAND => {
                match (wp.0 & 0xFFFF) as usize {
                    ID_RESET => replace(RuntimeTuning::default(), true),
                    ID_DONE => {
                        let _ = ShowWindow(window, SW_HIDE);
                    }
                    _ => {}
                }
                LRESULT(0)
            }
            // Static controls paint their own background, which would be white
            // squares on the dialog face without this.
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
            // Closing hides rather than destroys, so the panel keeps its place
            // and the next open is instant -- `isReleasedWhenClosed = false`.
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

    /// Trackbars are integer-positioned, so every row's range has to divide
    /// into a usable number of notches -- one notch means a slider with two
    /// settings, which is not a slider.
    #[test]
    fn every_row_has_a_usable_number_of_notches() {
        let tuning = RuntimeTuning::default();
        for item in items() {
            let Item::Slider(row) = item else { continue };
            let (notches, _) = positions(&row, &tuning);
            assert!(
                (8..=1000).contains(&notches),
                "{} has {notches} notches",
                row.title
            );
        }
    }

    /// A position read off a trackbar has to come back as the value that put it
    /// there, or dragging a slider one notch would move it two.
    #[test]
    fn a_position_round_trips_to_its_value() {
        let tuning = RuntimeTuning::default();
        for item in items() {
            let Item::Slider(row) = item else { continue };
            let (notches, at) = positions(&row, &tuning);
            let value = value_at(&row, &tuning, at);
            assert!(
                (value - tuning.get(row.key)).abs() <= row.step,
                "{} drifted: {value} vs {}",
                row.title,
                tuning.get(row.key)
            );
            // And the ends of the trackbar are the ends of the range.
            let (lower, upper) = tuning.limits(row.key);
            assert!((value_at(&row, &tuning, 0) - lower).abs() < 1e-9);
            assert!((value_at(&row, &tuning, notches) - upper).abs() < 1e-9);
        }
    }

    /// The reason the track is split: every slider starts in the middle, so
    /// "centre" reads as the authored behaviour and the two directions are
    /// symmetric even where the bounds are not.
    #[test]
    fn a_default_sits_exactly_in_the_middle() {
        let tuning = RuntimeTuning::default();
        for item in items() {
            let Item::Slider(row) = item else { continue };
            let (notches, at) = positions(&row, &tuning);
            assert_eq!(
                at * 2,
                notches,
                "{} starts at {at} of {notches}, not the middle",
                row.title
            );
            // And the middle notch gives the default back exactly, so opening
            // the panel and closing it cannot drift the pet off its defaults.
            assert_eq!(value_at(&row, &tuning, notches / 2), tuning.get(row.key));
        }
    }

    /// Moving one notch either side of centre has to actually change the value,
    /// or the middle would be a dead zone the user cannot get out of by dragging.
    #[test]
    fn the_notches_either_side_of_centre_are_distinct() {
        let tuning = RuntimeTuning::default();
        for item in items() {
            let Item::Slider(row) = item else { continue };
            let (notches, middle) = positions(&row, &tuning);
            let centre = value_at(&row, &tuning, middle);
            assert!(
                value_at(&row, &tuning, middle - 1) < centre,
                "{} does not go lower",
                row.title
            );
            assert!(
                value_at(&row, &tuning, middle + 1) > centre,
                "{} does not go higher",
                row.title
            );
            assert_eq!(notches, middle * 2);
        }
    }

    /// The window has to carry the application icon, or its title bar shows the
    /// shell's default -- which is what happened when the icon was compiled in
    /// but nothing put it on a window.
    #[test]
    fn the_panel_wears_the_app_icon() {
        let window = create(RuntimeTuning::default()).expect("the panel did not build");
        let small = unsafe { SendMessageW(window, WM_GETICON, WPARAM(ICON_SMALL as usize), LPARAM(0)) };
        let big = unsafe { SendMessageW(window, WM_GETICON, WPARAM(ICON_BIG as usize), LPARAM(0)) };
        unsafe {
            let _ = DestroyWindow(window);
        }
        assert_ne!(small.0, 0, "no small icon; the title bar would show the default");
        assert_ne!(big.0, 0, "no large icon; Alt-Tab would show the default");
    }

    /// The layout is Win32 objects, so the only way to know it came out whole
    /// is to build one. Created hidden and destroyed straight away -- a test
    /// must not flash a window at whoever is running it.
    #[test]
    fn the_panel_lays_out_and_answers_the_sliders() {
        let window = create(RuntimeTuning::default()).expect("the panel did not build");
        let expected = items()
            .iter()
            .filter(|item| matches!(item, Item::Slider(_)))
            .count();

        let (count, matched) = PANEL.with(|slot| {
            let panel = slot.borrow();
            let panel = panel.as_ref().expect("the panel was not stored");
            let matched = panel
                .sliders
                .iter()
                .all(|(track, readout, _)| !track.is_invalid() && !readout.is_invalid());
            (panel.sliders.len(), matched)
        });
        assert_eq!(count, expected, "not every slider was created");
        assert!(matched, "a control failed to create");

        // Moving one slider has to reach `PENDING`, which is the only way a
        // change leaves this window.
        let (track, key) = PANEL.with(|slot| {
            let panel = slot.borrow();
            let panel = panel.as_ref().expect("the panel was not stored");
            let (track, _, row) = &panel.sliders[0];
            (*track, row.key)
        });
        unsafe {
            SendMessageW(track, TBM_SETPOS, WPARAM(1), LPARAM(3));
            wndproc(window, WM_HSCROLL, WPARAM(0), LPARAM(track.0 as isize));
        }
        let changed = take_pending().expect("moving a slider changed nothing");
        assert_ne!(changed.get(key), RuntimeTuning::default().get(key));

        // Reset puts it back, and that is a change too.
        unsafe {
            wndproc(window, WM_COMMAND, WPARAM(ID_RESET), LPARAM(0));
            let _ = DestroyWindow(window);
        }
        assert_eq!(take_pending(), Some(RuntimeTuning::default()));
    }

    /// A slider sitting on its default says only the value; one that has been
    /// moved says what it was. Without that there is no way to tell a value you
    /// chose from one that shipped -- which is exactly the question the centre
    /// of the track invites.
    #[test]
    fn a_moved_slider_says_what_the_default_was() {
        let defaults = RuntimeTuning::default();
        for item in items() {
            let Item::Slider(row) = item else { continue };
            let plain = readout_text(&row, &defaults);
            assert_eq!(
                plain,
                row.unit.text(defaults.get(row.key)),
                "{} says something extra while sitting on its default",
                row.title
            );

            // Move it to the far end of its track and the default must appear.
            let (notches, _) = positions(&row, &defaults);
            let moved = defaults.with(row.key, value_at(&row, &defaults, notches));
            if (moved.get(row.key) - defaults.get(row.key)).abs() < 1e-9 {
                continue; // Nowhere to move; nothing to report.
            }
            let text = readout_text(&row, &moved);
            assert!(
                text.contains(&plain),
                "{} lost the authored value: {text}",
                row.title
            );
            assert_ne!(text, plain, "{} did not mark itself as moved", row.title);
        }
    }

    /// The panel is a list, and it has to be the same list the macOS panel is.
    #[test]
    fn every_tunable_value_has_a_row() {
        let mut covered: Vec<RuntimeTuningKey> = items()
            .into_iter()
            .filter_map(|item| match item {
                Item::Slider(row) => Some(row.key),
                _ => None,
            })
            .collect();
        for key in roamling_core::TUNING_KEYS {
            assert!(covered.contains(&key), "{key:?} has no slider");
        }
        let before = covered.len();
        covered.dedup();
        assert_eq!(before, covered.len(), "a key has two sliders");
        assert_eq!(before, roamling_core::TUNING_KEYS.len());
    }
}
