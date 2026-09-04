// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

//! Where the user is actually working: the focused window, and the text caret
//! inside it.
//!
//! `docs/windows.md` section 5 said to try `GetGUIThreadInfo` before reaching
//! for UI Automation, because COM interop is the most painful part of this port
//! and a great many apps report their caret through the older path for free.
//! This is that attempt. Where it comes back empty the pet simply keeps the
//! placement it had, which is the same thing that happens on macOS when
//! Accessibility is not granted.
//!
//! The same file answers the other window question -- where the work the agent
//! is doing probably is -- because both are "measure the foreground window" and
//! macOS only splits them because one needs a permission and the other does not.
//! Windows needs neither.
//!
//! It reads rectangles. Not text, not window titles, not what is typed.

use roamling_core::{FocusSnapshot, LocationHint, WorldRect};
use windows::Win32::Foundation::{HWND, POINT, RECT};
use windows::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_EXTENDED_FRAME_BOUNDS};
use windows::Win32::Graphics::Gdi::ClientToScreen;
use windows::Win32::UI::WindowsAndMessaging::{
    GetForegroundWindow, GetGUIThreadInfo, GetWindowRect,
    GetWindowThreadProcessId, GUITHREADINFO,
};

/// Roughly where the user is working, for an event that arrived without a place.
///
/// Ported from `MacWindowProvider.currentActivityLocationHint`, down to the
/// 0.55 confidence and the size floor: a window smaller than this is a palette
/// or a notification, and sitting next to one says nothing about where the work
/// is. It never reads a window title, and it never reads any window but the
/// foreground one.
pub fn activity_location_hint() -> Option<LocationHint> {
    let foreground = unsafe { GetForegroundWindow() };
    if foreground.is_invalid() {
        return None;
    }
    // Roamling's own overlay must not be mistaken for the user's work.
    let mut owner = 0u32;
    unsafe { GetWindowThreadProcessId(foreground, Some(&mut owner)) };
    if owner == std::process::id() {
        return None;
    }
    let frame = frame_of(foreground)?;
    if frame.size.width < 120.0 || frame.size.height < 100.0 {
        return None;
    }
    Some(LocationHint::new(Some(frame), 0.55))
}

/// Rectangles arrive from Win32 in physical pixels and the core works in the
/// world plane, so everything this file produces goes through here. One funnel,
/// so the caret and the window frame cannot end up in different units.
fn to_world(rect: RECT) -> Option<WorldRect> {
    let scale = crate::platform::world_scale();
    let width = (rect.right - rect.left) as f64 / scale;
    let height = (rect.bottom - rect.top) as f64 / scale;
    // Empty rects are dropped on the way in, so a `Some` means something real
    // was measured. `FocusSnapshot`'s own documentation asks for that.
    if width <= 0.0 || height <= 0.0 {
        return None;
    }
    Some(WorldRect::new(
        rect.left as f64 / scale,
        rect.top as f64 / scale,
        width,
        height,
    ))
}

/// `GetWindowRect` includes the invisible resize border that Windows 10 added,
/// so a window measured that way is wider than it looks and the pet would sit
/// inside it thinking it was clear. DWM knows the drawn bounds.
fn frame_of(window: HWND) -> Option<WorldRect> {
    let mut rect = RECT::default();
    let dwm = unsafe {
        DwmGetWindowAttribute(
            window,
            DWMWA_EXTENDED_FRAME_BOUNDS,
            &mut rect as *mut RECT as *mut std::ffi::c_void,
            std::mem::size_of::<RECT>() as u32,
        )
    };
    if dwm.is_ok() {
        return to_world(rect);
    }
    // A window DWM has nothing to say about is better measured badly than not
    // at all: the frame only steers placement, it does not gate it.
    let mut fallback = RECT::default();
    unsafe { GetWindowRect(window, &mut fallback).ok()? };
    to_world(fallback)
}

pub fn focus() -> Option<FocusSnapshot> {
    unsafe {
        let foreground = GetForegroundWindow();
        if foreground.is_invalid() {
            return None;
        }
        let window_frame = frame_of(foreground);

        let thread = GetWindowThreadProcessId(foreground, None);
        let mut info = GUITHREADINFO {
            cbSize: std::mem::size_of::<GUITHREADINFO>() as u32,
            ..Default::default()
        };
        let queried = GetGUIThreadInfo(thread, &mut info).is_ok();

        // The caret rect is in the client coordinates of whichever window owns
        // it, which is often a child of the foreground window rather than the
        // window itself.
        let caret_frame = if queried && !info.hwndCaret.is_invalid() {
            let mut origin = POINT {
                x: info.rcCaret.left,
                y: info.rcCaret.top,
            };
            if ClientToScreen(info.hwndCaret, &mut origin).as_bool() {
                let width = info.rcCaret.right - info.rcCaret.left;
                let height = info.rcCaret.bottom - info.rcCaret.top;
                to_world(RECT {
                    left: origin.x,
                    top: origin.y,
                    right: origin.x + width,
                    bottom: origin.y + height,
                })
            } else {
                None
            }
        } else {
            None
        };

        let focused_element_frame = if queried && !info.hwndFocus.is_invalid() {
            frame_of(info.hwndFocus)
        } else {
            None
        };

        if window_frame.is_none() && caret_frame.is_none() && focused_element_frame.is_none() {
            return None;
        }

        // A caret is a real measurement of where the work is. A window frame
        // alone only says which rectangle to stay out of, so it is worth less.
        let confidence = if caret_frame.is_some() {
            1.0
        } else if focused_element_frame.is_some() {
            0.6
        } else {
            0.3
        };

        Some(FocusSnapshot {
            window_frame,
            focused_element_frame,
            caret_frame,
            confidence,
        })
    }
}
