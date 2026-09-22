// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

//! Owned, non-activating scenery; never part of the pet's hit target.
use crate::sprite::Surface;
use roamling_core::effects::EffectFrame;
use windows::core::{w, Result};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::*;

pub struct EffectOverlay {
    hwnd: HWND,
    surface: Option<Surface>,
    visible: bool,
}

impl EffectOverlay {
    pub fn new(owner: HWND) -> Result<Self> {
        unsafe {
            let instance = GetModuleHandleW(None)?;
            let class = w!("RoamlingEffects");
            let wc = WNDCLASSW {
                lpfnWndProc: Some(wndproc), hInstance: instance.into(),
                lpszClassName: class, ..Default::default()
            };
            RegisterClassW(&wc);
            let hwnd = CreateWindowExW(
                WS_EX_LAYERED | WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_TRANSPARENT,
                class, w!("Roamling effects"), WS_POPUP, 0, 0, 1, 1,
                owner, None, instance, None,
            )?;
            if std::env::var_os("ROAMLING_ALLOW_CAPTURE").is_none() {
                let _ = SetWindowDisplayAffinity(hwnd, WDA_EXCLUDEFROMCAPTURE);
            }
            Ok(Self { hwnd, surface: None, visible: false })
        }
    }

    pub fn hide(&mut self) {
        if self.visible {
            unsafe { let _ = ShowWindow(self.hwnd, SW_HIDE); }
            self.visible = false;
        }
    }

    /// Centre is already in physical screen pixels, as in the pet renderer.
    pub fn draw(&mut self, frames: &[EffectFrame], centre: (f64, f64), body: (i32, i32)) {
        if frames.is_empty() { self.hide(); return; }
        let (width, height) = (body.0 * 2, body.1 * 2);
        if self.surface.as_ref().map_or(true, |s| s.width != width || s.height != height) {
            self.surface = Surface::new(width, height);
        }
        let Some(surface) = self.surface.as_mut() else { self.hide(); return; };
        surface.draw_effects(frames);
        surface.present(self.hwnd, POINT {
            x: (centre.0 - body.0 as f64).round() as i32,
            y: (centre.1 - body.1 as f64 * 1.5).round() as i32,
        });
        if !self.visible {
            unsafe { let _ = ShowWindow(self.hwnd, SW_SHOWNOACTIVATE); }
            self.visible = true;
        }
    }
}

impl Drop for EffectOverlay {
    fn drop(&mut self) {
        unsafe { let _ = DestroyWindow(self.hwnd); }
    }
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_NCHITTEST => LRESULT(HTTRANSPARENT as isize),
        WM_MOUSEACTIVATE => LRESULT(MA_NOACTIVATE as isize),
        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effect_window_is_nonactivating_click_through_and_owned() {
        unsafe {
            let owner = CreateWindowExW(
                WS_EX_TOOLWINDOW, w!("STATIC"), w!("effect owner"), WS_POPUP,
                0, 0, 100, 100, None, None, None, None,
            ).unwrap();
            let mut overlay = EffectOverlay::new(owner).unwrap();
            let style = GetWindowLongPtrW(overlay.hwnd, GWL_EXSTYLE) as u32;
            for flag in [WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TRANSPARENT, WS_EX_TOOLWINDOW] {
                assert_ne!(style & flag.0, 0);
            }
            assert_eq!(GetWindow(overlay.hwnd, GW_OWNER).unwrap(), owner);
            assert_eq!(SendMessageW(overlay.hwnd, WM_NCHITTEST, WPARAM(0), LPARAM(0)).0, HTTRANSPARENT as isize);
            let mut effects = roamling_core::effects::EffectSystem::default();
            for _ in 0..20 { effects.update(1.0 / 30.0, true, 1.0); }
            overlay.draw(&effects.frames(), (200.0, 200.0), (96, 104));
            assert!(IsWindowVisible(overlay.hwnd).as_bool());
            overlay.draw(&effects.frames(), (400.0, 300.0), (144, 156));
            assert_eq!(overlay.surface.as_ref().unwrap().width, 288);
            overlay.hide();
            assert!(!IsWindowVisible(overlay.hwnd).as_bool());
            overlay.draw(&[], (200.0, 200.0), (96, 104));
            assert!(!IsWindowVisible(overlay.hwnd).as_bool());
            let child = overlay.hwnd;
            drop(overlay);
            assert!(!IsWindow(child).as_bool());
            let _ = DestroyWindow(owner);
        }
    }
}
