// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

//! Where the user is actually working: the focused window, and the text caret
//! inside it.
//!
//! `docs/history/windows.md` section 5 said to try `GetGUIThreadInfo` before reaching
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
use std::collections::HashMap;
use std::ffi::OsString;
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::path::{Path, PathBuf};
use windows::core::{PCWSTR, PWSTR};
use windows::Win32::Foundation::{CloseHandle, HWND, POINT, RECT};
use windows::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_EXTENDED_FRAME_BOUNDS};
use windows::Win32::Graphics::Gdi::ClientToScreen;
use windows::Win32::Storage::FileSystem::{
    GetFileVersionInfoSizeW, GetFileVersionInfoW, VerQueryValueW,
};
use windows::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetForegroundWindow, GetGUIThreadInfo, GetWindowRect, GetWindowThreadProcessId, GUITHREADINFO,
};

pub(crate) fn cache_key(exe: &str) -> String {
    exe.to_ascii_lowercase()
}

/// The display label remembered for an executable, or its file name until one
/// can be read from the executable's version resource.
pub fn application_label<'a>(labels: &'a HashMap<String, String>, exe: &'a str) -> &'a str {
    labels
        .get(&cache_key(exe))
        .map(String::as_str)
        .unwrap_or(exe)
}

fn file_description(path: &Path) -> Option<String> {
    let mut path: Vec<u16> = path.as_os_str().encode_wide().collect();
    path.push(0);

    let size = unsafe { GetFileVersionInfoSizeW(PCWSTR(path.as_ptr()), None) };
    if size == 0 {
        return None;
    }
    // DWORD-aligned storage matches the version-resource contract while the
    // API still receives the exact byte count it requested.
    let words = (size as usize + std::mem::size_of::<u32>() - 1) / std::mem::size_of::<u32>();
    let mut version = vec![0u32; words];
    unsafe {
        GetFileVersionInfoW(PCWSTR(path.as_ptr()), 0, size, version.as_mut_ptr().cast()).ok()?;
    }

    let translations = wide("\\VarFileInfo\\Translation");
    let mut translation_ptr = std::ptr::null_mut();
    let mut translation_bytes = 0u32;
    if !unsafe {
        VerQueryValueW(
            version.as_ptr().cast(),
            PCWSTR(translations.as_ptr()),
            &mut translation_ptr,
            &mut translation_bytes,
        )
    }
    .as_bool()
        || translation_ptr.is_null()
    {
        return None;
    }

    // Each translation is a language/code-page pair. The installed Office and
    // Hancom binaries declare a neutral (0000) language, so use the resource's
    // own table instead of guessing a Korean or English block.
    let translation_words = translation_bytes as usize / std::mem::size_of::<u16>();
    for pair in 0..translation_words / 2 {
        let language = unsafe { (translation_ptr.cast::<u16>().add(pair * 2)).read_unaligned() };
        let code_page =
            unsafe { (translation_ptr.cast::<u16>().add(pair * 2 + 1)).read_unaligned() };
        let query = wide(&format!(
            "\\StringFileInfo\\{language:04x}{code_page:04x}\\FileDescription"
        ));
        let mut value_ptr = std::ptr::null_mut();
        let mut value_chars = 0u32;
        if unsafe {
            VerQueryValueW(
                version.as_ptr().cast(),
                PCWSTR(query.as_ptr()),
                &mut value_ptr,
                &mut value_chars,
            )
        }
        .as_bool()
            && !value_ptr.is_null()
            && value_chars > 0
        {
            let value = unsafe {
                std::slice::from_raw_parts(value_ptr.cast::<u16>(), value_chars as usize)
            };
            let description = String::from_utf16_lossy(value)
                .trim_end_matches('\0')
                .trim()
                .to_owned();
            if !description.is_empty() {
                return Some(description);
            }
        }
    }
    None
}

fn remember_application_label(labels: &mut HashMap<String, String>, exe: &str, path: &Path) {
    let key = cache_key(exe);
    if labels.contains_key(&key) {
        return;
    }
    if let Some(label) = file_description(path).filter(|label| !label.eq_ignore_ascii_case(exe)) {
        labels.insert(key, label);
    }
}

/// The executable file name owning the foreground window.
///
/// Roamling's own tray and overlay are unknown rather than a departure, so
/// opening the menu cannot end a work-app session.
pub fn foreground_application(labels: &mut HashMap<String, String>) -> Option<String> {
    let foreground = unsafe { GetForegroundWindow() };
    if foreground.is_invalid() {
        return None;
    }

    let mut process_id = 0u32;
    unsafe { GetWindowThreadProcessId(foreground, Some(&mut process_id)) };
    if process_id == 0 || process_id == std::process::id() {
        return None;
    }

    let process =
        unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, process_id).ok()? };
    let result = (|| {
        let mut path = vec![0u16; 32_768];
        let mut length = path.len() as u32;
        unsafe {
            QueryFullProcessImageNameW(
                process,
                PROCESS_NAME_WIN32,
                PWSTR(path.as_mut_ptr()),
                &mut length,
            )
            .ok()?;
        }
        let path = PathBuf::from(OsString::from_wide(&path[..length as usize]));
        let exe = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())?;
        remember_application_label(labels, &exe, &path);
        Some(exe)
    })();
    unsafe {
        let _ = CloseHandle(process);
    }
    result
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_fall_back_to_the_executable_name() {
        let labels = HashMap::new();
        assert_eq!(application_label(&labels, "WINWORD.EXE"), "WINWORD.EXE");
    }

    #[test]
    fn cached_labels_are_case_insensitive_and_not_replaced() {
        let mut labels = HashMap::from([("winword.exe".into(), "Microsoft Word".into())]);
        remember_application_label(
            &mut labels,
            "WINWORD.EXE",
            Path::new(r"Z:\a-path-that-does-not-exist\WINWORD.EXE"),
        );
        assert_eq!(application_label(&labels, "WINWORD.EXE"), "Microsoft Word");
        assert_eq!(labels.len(), 1);
    }

    #[test]
    fn an_unresolved_executable_name_is_not_cached_as_a_label() {
        let mut labels = HashMap::new();
        remember_application_label(
            &mut labels,
            "WINWORD.EXE",
            Path::new(r"Z:\a-path-that-does-not-exist\WINWORD.EXE"),
        );
        assert!(labels.is_empty());
    }
}
