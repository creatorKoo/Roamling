// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

//! What the shell reads off the machine, as core domain values.
//!
//! The macOS counterpart is `MacPlatform.makeServices()`. Nothing Win32 leaves
//! this file: `HMONITOR`, `POINT` and `HWND` do not cross into the runtime.

use roamling_core::{DisplaySnapshot, WorldPoint, WorldRect};
use windows::Win32::Foundation::{LPARAM, POINT, RECT};
use windows::Win32::Graphics::Gdi::{
    EnumDisplayMonitors, GetMonitorInfoW, MonitorFromPoint, HDC, HMONITOR, MONITORINFOEXW,
    MONITOR_DEFAULTTOPRIMARY,
};
use windows::Win32::System::SystemInformation::GetTickCount;
use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, GetLastInputInfo, LASTINPUTINFO, VK_LBUTTON,
};
use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;

/// The world plane is the Windows virtual screen divided by one scale factor.
///
/// It is already top-left/y-down with negative coordinates allowed, which is
/// exactly what `WorldPoint` wants -- so unlike AppKit there is no flip here.
///
/// **Why divide at all.** macOS hands the core *logical points*; a
/// per-monitor-DPI-aware Windows process gets *physical pixels*. Taking the
/// physical plane as the world -- which is what this did until 2026-09-04 --
/// keeps the desktop whole but makes every tuned constant mean something
/// different on the two platforms: walk 160, notice 170 and catch 74 are points
/// on a Mac and pixels here, so a 150% display gets two thirds of the distance.
/// The user reported exactly that: movement and pointer awareness felt short.
///
/// **Why one factor rather than per monitor.** The desktop is *laid out* in
/// physical pixels. Dividing each monitor by its own scale tears the plane --
/// two monitors at different scales would overlap or leave a gap where the
/// desktop has neither. A single scalar is a similarity transform of the whole
/// virtual screen, so the plane stays exactly as connected as Windows says it
/// is. The primary display's scale is the factor, which makes the primary
/// monitor agree with macOS exactly and leaves a secondary at a different scale
/// moving by the ratio between them -- visible only as a slightly different
/// walking speed there, never as a hole in the world.
///
/// **Why it is fixed for the session.** Everything the core holds is in these
/// units, including the resting place written to settings. Changing the factor
/// while the pet is standing somewhere would move it without anything having
/// happened. A display change mid-session is rare and a restart settles it.
pub fn world_scale() -> f64 {
    static SCALE: std::sync::OnceLock<f64> = std::sync::OnceLock::new();
    *SCALE.get_or_init(|| {
        let mut scale = 1.0;
        unsafe {
            // MONITOR_DEFAULTTOPRIMARY, from a point no monitor owns.
            let monitor = MonitorFromPoint(POINT { x: 0, y: 0 }, MONITOR_DEFAULTTOPRIMARY);
            let mut dpi_x = 96u32;
            let mut dpi_y = 96u32;
            if GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y).is_ok() {
                scale = dpi_x as f64 / 96.0;
            }
        }
        if scale > 0.0 {
            scale
        } else {
            1.0
        }
    })
}

fn rect_to_world(rect: RECT) -> WorldRect {
    let scale = world_scale();
    WorldRect::new(
        rect.left as f64 / scale,
        rect.top as f64 / scale,
        (rect.right - rect.left) as f64 / scale,
        (rect.bottom - rect.top) as f64 / scale,
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
    let scale = world_scale();
    WorldPoint::new(point.x as f64 / scale, point.y as f64 / scale)
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

#[cfg(test)]
mod tests {
    use super::*;

    /// A plausible factor, and the same one every time it is asked. Everything
    /// the core holds is in units derived from this, so a value that drifted
    /// mid-session would move the pet without anything having happened.
    #[test]
    fn the_world_scale_is_sane_and_stable() {
        let scale = world_scale();
        assert!(
            (0.5..=8.0).contains(&scale),
            "{scale} is not a display scale"
        );
        assert_eq!(scale, world_scale());
    }

    /// The invariant that catches the mistake this conversion invites: dividing
    /// the displays but not the pointer, or the other way round. Either way the
    /// cursor stops landing on any monitor, and the pet spends its life running
    /// from something that is nowhere.
    #[test]
    fn the_pointer_lands_on_a_display() {
        let displays = displays();
        if displays.is_empty() {
            return; // No desktop session; nothing to check against.
        }
        let pointer = pointer();
        assert!(
            displays.iter().any(|display| display.frame.contains(pointer)),
            "pointer {pointer:?} is on none of {:?}",
            displays.iter().map(|d| d.frame).collect::<Vec<_>>()
        );
    }

    /// Both rectangles go through the same conversion, so the work area has to
    /// stay inside the monitor it belongs to.
    #[test]
    fn the_work_area_stays_inside_its_display() {
        for display in displays() {
            assert!(display.frame.size.width > 0.0 && display.frame.size.height > 0.0);
            let frame = display.frame;
            let visible = display.visible_frame;
            assert!(
                visible.origin.x >= frame.origin.x - 0.5
                    && visible.origin.y >= frame.origin.y - 0.5
                    && visible.size.width <= frame.size.width + 0.5
                    && visible.size.height <= frame.size.height + 0.5,
                "{visible:?} is not inside {frame:?}"
            );
        }
    }
}
