// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

//! What the shell reads off the machine, as core domain values.
//!
//! The macOS counterpart is `MacPlatform.makeServices()`. Nothing Win32 leaves
//! this file: `HMONITOR`, `POINT` and `HWND` do not cross into the runtime.

use roamling_core::{DisplaySnapshot, WorldPoint, WorldRect};
use windows::Win32::Foundation::{LPARAM, POINT, RECT};
use windows::Win32::Graphics::Gdi::{
    EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFOEXW,
};
use windows::Win32::System::SystemInformation::GetTickCount;
use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, GetLastInputInfo, LASTINPUTINFO, VK_LBUTTON,
};
use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;

/// The world plane is the Windows virtual screen, in physical pixels.
///
/// It is already top-left/y-down with negative coordinates allowed, which is
/// exactly what `WorldPoint` wants -- so unlike AppKit there is no flip here.
///
/// UNSETTLED, and W4 has to settle it: macOS hands the core *logical* points
/// and reports the backing factor separately, while a per-monitor-DPI-aware
/// process on Windows gets physical pixels. Converting per monitor would tear
/// the plane apart, because the desktop is laid out in physical pixels and two
/// monitors at different scales would then overlap or gap. Taking physical as
/// the world keeps the plane whole, but it means the tuned constants -- walk
/// 40/s, notice 170, catch radius 74 -- are read as physical pixels here and as
/// points on macOS. On a 150% display the pet therefore walks two thirds as far
/// per second as it does on a Mac. Whether that is corrected by scaling the
/// constants or by scaling the plane is a question for the user's real-use
/// check, not something to guess at now.
fn rect_to_world(rect: RECT) -> WorldRect {
    WorldRect::new(
        rect.left as f64,
        rect.top as f64,
        (rect.right - rect.left) as f64,
        (rect.bottom - rect.top) as f64,
    )
}

/// `EnumDisplayMonitors` in the order Windows reports it, which puts the
/// primary first often enough not to rely on it -- the core does not care.
pub fn displays() -> Vec<DisplaySnapshot> {
    let mut found: Vec<DisplaySnapshot> = Vec::new();
    unsafe {
        let _ = EnumDisplayMonitors(
            None,
            None,
            Some(collect),
            LPARAM(&mut found as *mut Vec<DisplaySnapshot> as isize),
        );
    }
    found
}

unsafe extern "system" fn collect(
    monitor: HMONITOR,
    _: HDC,
    _: *mut RECT,
    out: LPARAM,
) -> windows::Win32::Foundation::BOOL {
    let list = &mut *(out.0 as *mut Vec<DisplaySnapshot>);
    let mut info = MONITORINFOEXW::default();
    info.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;
    if !GetMonitorInfoW(monitor, &mut info.monitorInfo as *mut _).as_bool() {
        return true.into();
    }

    let mut dpi_x = 96u32;
    let mut dpi_y = 96u32;
    let _ = GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y);

    let name = String::from_utf16_lossy(&info.szDevice)
        .trim_end_matches('\0')
        .to_string();
    list.push(DisplaySnapshot {
        // Stable within a session, which is all the core asks of it.
        id: name.clone(),
        name,
        frame: rect_to_world(info.monitorInfo.rcMonitor),
        // The taskbar's exclusion, and macOS's `visibleFrame` equivalent.
        visible_frame: rect_to_world(info.monitorInfo.rcWork),
        scale: dpi_x as f64 / 96.0,
    });
    true.into()
}

pub fn pointer() -> WorldPoint {
    let mut point = POINT::default();
    unsafe {
        let _ = GetCursorPos(&mut point);
    }
    WorldPoint::new(point.x as f64, point.y as f64)
}

pub fn primary_button_down() -> bool {
    // The high bit is "currently down"; the low bit is "pressed since last
    // call" and would make the runtime see a click the user already released.
    unsafe { (GetAsyncKeyState(VK_LBUTTON.0 as i32) as u16 & 0x8000) != 0 }
}

/// Seconds since the last keyboard or mouse input, machine-wide.
///
/// `GetLastInputInfo` needs no permission at all, unlike the macOS side which
/// had to avoid an event tap to stay permission-free.
pub fn user_idle_duration() -> f64 {
    let mut info = LASTINPUTINFO {
        cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32,
        dwTime: 0,
    };
    unsafe {
        if !GetLastInputInfo(&mut info).as_bool() {
            return 0.0;
        }
        // Both wrap at 49.7 days; the subtraction wraps with them.
        GetTickCount().wrapping_sub(info.dwTime) as f64 / 1000.0
    }
}
