// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

//! Modeless native guide. Content/revisions are shared with the macOS shell.
use crate::strings::{localized, localized_format};
use std::cell::RefCell;
use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::WindowsAndMessaging::*;

pub const SEEN_KEY: &str = "roamling.guideSeenRevision";
const CONTENT: &str = include_str!("../../../Sources/RoamlingShell/Resources/UsageGuide.txt");

struct Guide {
    revision: u32,
    basics: Vec<String>,
    changes: Vec<(u32, String)>,
}
#[derive(Clone, Debug, PartialEq)]
struct Page {
    revision: u32,
    basics: bool,
    keys: Vec<String>,
}
impl Guide {
    fn parse(text: &str) -> Option<Self> {
        let mut guide = Self {
            revision: 0,
            basics: vec![],
            changes: vec![],
        };
        for line in text
            .lines()
            .map(str::trim)
            .filter(|s| !s.is_empty() && !s.starts_with('#'))
        {
            let (kind, value) = line.split_once('|')?;
            if value.is_empty() || value.contains('|') {
                return None;
            }
            match kind {
                "revision" => guide.revision = value.parse().ok()?,
                "basic" => guide.basics.push(value.into()),
                _ => {
                    let revision = kind.parse::<u32>().ok()?;
                    if revision == 0 {
                        return None;
                    }
                    guide.changes.push((revision, value.into()));
                }
            }
        }
        (guide.revision > 0
            && !guide.basics.is_empty()
            && guide.changes.iter().all(|(v, _)| *v <= guide.revision))
        .then_some(guide)
    }
    fn page(&self, seen: u32, manual: bool) -> Option<Page> {
        if manual || seen == 0 {
            return Some(Page {
                revision: self.revision,
                basics: true,
                keys: self.basics.clone(),
            });
        }
        if seen >= self.revision {
            return None;
        }
        let keys: Vec<_> = self
            .changes
            .iter()
            .filter(|(v, _)| *v > seen)
            .map(|(_, k)| k.clone())
            .collect();
        (!keys.is_empty()).then_some(Page {
            revision: self.revision,
            basics: false,
            keys,
        })
    }
}
impl Page {
    fn title(&self) -> &str {
        localized(if self.basics {
            "guide.title"
        } else {
            "guide.updates.title"
        })
    }
    fn body(&self) -> String {
        let mut sections = vec![localized("guide.intro").to_owned()];
        for key in &self.keys {
            sections.push(format!(
                "{}\r\n{}",
                localized(&format!("{key}.title")),
                localized_format(
                    &format!("{key}.body"),
                    &[localized("guide.modifier.windows")]
                )
            ));
        }
        sections.push(localized("guide.footer").into());
        sections.join("\r\n\r\n")
    }
}

struct Panel {
    window: HWND,
    heading: HWND,
    body: HWND,
    done: HWND,
    font: HFONT,
    revision: u32,
}
thread_local! {
    static PANEL: RefCell<Option<Panel>> = const { RefCell::new(None) };
    static ACKNOWLEDGED: RefCell<Option<u32>> = const { RefCell::new(None) };
}
pub fn take_acknowledged() -> Option<u32> {
    ACKNOWLEDGED.with(|v| v.borrow_mut().take())
}

/// Whether the panel is on screen. Closing only hides it, so being built is
/// not the same thing. An update waits rather than close it under the user.
pub fn is_visible() -> bool {
    PANEL.with(|slot| {
        slot.borrow()
            .as_ref()
            .is_some_and(|panel| unsafe { IsWindowVisible(panel.window).as_bool() })
    })
}
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}

pub fn show(seen: u32, manual: bool) {
    let Some(page) = Guide::parse(CONTENT).and_then(|guide| guide.page(seen, manual)) else {
        return;
    };
    let existing = PANEL.with(|p| p.borrow().as_ref().map(|p| (p.window, p.heading, p.body)));
    let window = if let Some((window, heading, body)) = existing {
        unsafe {
            let _ = SetWindowTextW(window, PCWSTR(wide(page.title()).as_ptr()));
            let _ = SetWindowTextW(heading, PCWSTR(wide(page.title()).as_ptr()));
            let _ = SetWindowTextW(body, PCWSTR(wide(&page.body()).as_ptr()));
        }
        window
    } else {
        let Ok(window) = create(&page) else {
            return;
        };
        window
    };
    unsafe {
        let _ = ShowWindow(
            window,
            if manual {
                SW_SHOWNORMAL
            } else {
                SW_SHOWNOACTIVATE
            },
        );
        if manual {
            let _ = SetForegroundWindow(window);
        }
    }
}

pub fn handle_message(message: &MSG) -> bool {
    let window = PANEL.with(|p| p.borrow().as_ref().map(|p| p.window));
    window.is_some_and(|window| unsafe { IsDialogMessageW(window, message).as_bool() })
}

fn create(page: &Page) -> windows::core::Result<HWND> {
    unsafe {
        let instance = GetModuleHandleW(None)?;
        let class = w!("RoamlingUsageGuide");
        RegisterClassW(&WNDCLASSW {
            lpfnWndProc: Some(wndproc),
            hInstance: instance.into(),
            lpszClassName: class,
            hCursor: LoadCursorW(None, IDC_ARROW)?,
            hbrBackground: GetSysColorBrush(COLOR_WINDOW),
            ..Default::default()
        });
        let window = CreateWindowExW(
            WS_EX_CONTROLPARENT,
            class,
            PCWSTR(wide(page.title()).as_ptr()),
            WS_OVERLAPPEDWINDOW,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            560,
            550,
            None,
            None,
            instance,
            None,
        )?;
        let dpi = GetDpiForWindow(window).max(96) as i32;
        let font = make_font(dpi);
        let children = (|| -> windows::core::Result<(HWND, HWND, HWND)> {
            let heading = CreateWindowExW(
                WINDOW_EX_STYLE(0),
                w!("STATIC"),
                PCWSTR(wide(page.title()).as_ptr()),
                WS_CHILD | WS_VISIBLE,
                0,
                0,
                0,
                0,
                window,
                None,
                instance,
                None,
            )?;
            let body = CreateWindowExW(
                WINDOW_EX_STYLE(0),
                w!("EDIT"),
                PCWSTR(wide(&page.body()).as_ptr()),
                WS_CHILD
                    | WS_VISIBLE
                    | WS_TABSTOP
                    | WS_VSCROLL
                    | WINDOW_STYLE(
                        ES_MULTILINE as u32 | ES_READONLY as u32 | ES_AUTOVSCROLL as u32,
                    ),
                0,
                0,
                0,
                0,
                window,
                None,
                instance,
                None,
            )?;
            let done = CreateWindowExW(
                WINDOW_EX_STYLE(0),
                w!("BUTTON"),
                PCWSTR(wide(localized("guide.done")).as_ptr()),
                WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(BS_DEFPUSHBUTTON as u32),
                0,
                0,
                0,
                0,
                window,
                HMENU(1 as *mut _),
                instance,
                None,
            )?;
            Ok((heading, body, done))
        })();
        let (heading, body, done) = match children {
            Ok(children) => children,
            Err(error) => {
                let _ = DestroyWindow(window);
                let _ = DeleteObject(font);
                return Err(error);
            }
        };
        for control in [heading, body, done] {
            SendMessageW(control, WM_SETFONT, WPARAM(font.0 as usize), LPARAM(1));
        }
        if let Ok(icon) = LoadIconW(instance, PCWSTR(1 as *const u16)) {
            SendMessageW(
                window,
                WM_SETICON,
                WPARAM(ICON_BIG as usize),
                LPARAM(icon.0 as isize),
            );
            SendMessageW(
                window,
                WM_SETICON,
                WPARAM(ICON_SMALL as usize),
                LPARAM(icon.0 as isize),
            );
        }
        PANEL.with(|p| {
            *p.borrow_mut() = Some(Panel {
                window,
                heading,
                body,
                done,
                font,
                revision: page.revision,
            })
        });
        let _ = SetWindowPos(
            window,
            None,
            0,
            0,
            560 * dpi / 96,
            550 * dpi / 96,
            SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE,
        );
        layout(window);
        Ok(window)
    }
}

unsafe fn make_font(dpi: i32) -> HFONT {
    CreateFontW(
        -15 * dpi / 96,
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
        DEFAULT_PITCH.0 as u32,
        w!("Segoe UI"),
    )
}
unsafe fn layout(window: HWND) {
    let controls = PANEL.with(|p| p.borrow().as_ref().map(|p| (p.heading, p.body, p.done)));
    let Some((heading, body, done)) = controls else {
        return;
    };
    let mut rect = RECT::default();
    let _ = GetClientRect(window, &mut rect);
    let dpi = GetDpiForWindow(window).max(96) as i32;
    let s = |v: i32| v * dpi / 96;
    let _ = MoveWindow(
        heading,
        s(24),
        s(18),
        (rect.right - s(48)).max(1),
        s(28),
        true,
    );
    let _ = MoveWindow(
        body,
        s(24),
        s(58),
        (rect.right - s(48)).max(1),
        (rect.bottom - s(128)).max(1),
        true,
    );
    let _ = MoveWindow(
        done,
        (rect.right - s(154)).max(0),
        (rect.bottom - s(52)).max(0),
        s(130),
        s(32),
        true,
    );
}
unsafe extern "system" fn wndproc(window: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_GETMINMAXINFO => {
            let minimum = &mut *(lp.0 as *mut MINMAXINFO);
            let dpi = GetDpiForWindow(window).max(96) as i32;
            minimum.ptMinTrackSize.x = 440 * dpi / 96;
            minimum.ptMinTrackSize.y = 360 * dpi / 96;
            LRESULT(0)
        }
        WM_SIZE => {
            layout(window);
            LRESULT(0)
        }
        WM_DPICHANGED => {
            let font = make_font((wp.0 & 0xffff) as i32);
            let old = PANEL.with(|p| {
                p.borrow_mut().as_mut().map(|p| {
                    let old = p.font;
                    p.font = font;
                    (old, [p.heading, p.body, p.done])
                })
            });
            if let Some((old, controls)) = old {
                for control in controls {
                    SendMessageW(control, WM_SETFONT, WPARAM(font.0 as usize), LPARAM(1));
                }
                let _ = DeleteObject(old);
            } else {
                let _ = DeleteObject(font);
            }
            let rect = &*(lp.0 as *const RECT);
            let _ = SetWindowPos(
                window,
                None,
                rect.left,
                rect.top,
                rect.right - rect.left,
                rect.bottom - rect.top,
                SWP_NOZORDER | SWP_NOACTIVATE,
            );
            LRESULT(0)
        }
        WM_COMMAND if matches!(wp.0 & 0xffff, 1 | 2) => {
            SendMessageW(window, WM_CLOSE, WPARAM(0), LPARAM(0));
            LRESULT(0)
        }
        WM_CLOSE => {
            if let Some(revision) = PANEL.with(|p| p.borrow().as_ref().map(|p| p.revision)) {
                ACKNOWLEDGED.with(|v| *v.borrow_mut() = Some(revision));
            }
            let _ = DestroyWindow(window);
            LRESULT(0)
        }
        WM_DESTROY => {
            if let Some(panel) = PANEL.with(|p| p.borrow_mut().take()) {
                let _ = DeleteObject(panel.font);
            }
            LRESULT(0)
        }
        _ => DefWindowProcW(window, msg, wp, lp),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn guide_selection_handles_first_run_updates_skips_manual_and_downgrade() {
        let guide = Guide::parse("revision|3\nbasic|current\n1|old\n2|second\n3|third").unwrap();
        assert_eq!(guide.page(0, false).unwrap().keys, ["current"]);
        assert_eq!(guide.page(1, false).unwrap().keys, ["second", "third"]);
        assert_eq!(guide.page(2, false).unwrap().keys, ["third"]);
        assert!(guide.page(3, false).is_none());
        assert!(guide.page(4, false).is_none());
        assert!(guide.page(4, true).unwrap().basics);
        assert!(Guide::parse("revision|1\nbasic|x\n2|future").is_none());
        let unchanged = Guide::parse("revision|3\nbasic|current\n1|old").unwrap();
        assert!(unchanged.page(1, false).is_none());
    }
    #[test]
    fn bundled_guide_is_localized_and_native_window_only_acknowledges_close() {
        let guide = Guide::parse(CONTENT).unwrap();
        for key in guide
            .basics
            .iter()
            .chain(guide.changes.iter().map(|(_, key)| key))
        {
            for suffix in ["title", "body"] {
                let key = format!("{key}.{suffix}");
                assert_ne!(localized(&key), key);
            }
        }
        let page = guide.page(0, false).unwrap();
        let window = create(&page).unwrap();
        assert!(take_acknowledged().is_none());
        // Resize while hidden: all controls must remain inside the client area.
        unsafe {
            SetWindowPos(
                window,
                None,
                0,
                0,
                440,
                360,
                SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE,
            )
            .unwrap();
            let (body, done) = PANEL.with(|p| {
                let p = p.borrow();
                let p = p.as_ref().unwrap();
                (p.body, p.done)
            });
            assert_eq!(
                GetWindowTextLengthW(body) as usize,
                page.body().encode_utf16().count()
            );
            let mut client = RECT::default();
            GetClientRect(window, &mut client).unwrap();
            for control in [body, done] {
                let mut bounds = RECT::default();
                GetWindowRect(control, &mut bounds).unwrap();
                MapWindowPoints(
                    None,
                    window,
                    std::slice::from_raw_parts_mut((&mut bounds as *mut RECT).cast(), 2),
                );
                assert!(bounds.left >= 0 && bounds.top >= 0);
                assert!(bounds.right <= client.right && bounds.bottom <= client.bottom);
            }
        }
        unsafe {
            SendMessageW(window, WM_CLOSE, WPARAM(0), LPARAM(0));
        }
        let directory =
            std::env::temp_dir().join(format!("roamling-guide-test-{}", std::process::id()));
        let path = directory.join("settings.txt");
        let mut settings = crate::settings::Settings::load_from(Some(path.clone()));
        assert_eq!(crate::guide_seen(&settings), 0);
        crate::remember_guide(&mut settings);
        assert_eq!(
            crate::guide_seen(&crate::settings::Settings::load_from(Some(path.clone()))),
            guide.revision
        );
        // A manual reopen after a downgrade must not lower the stored revision.
        settings.set(SEEN_KEY, guide.revision + 1);
        let window = create(&page).unwrap();
        unsafe {
            SendMessageW(window, WM_COMMAND, WPARAM(1), LPARAM(0));
        }
        crate::remember_guide(&mut settings);
        assert_eq!(crate::guide_seen(&settings), guide.revision + 1);
        std::fs::remove_file(path).unwrap();
        std::fs::remove_dir(directory).unwrap();
        // Destruction during shutdown is not a user's acknowledgement.
        let window = create(&page).unwrap();
        unsafe {
            DestroyWindow(window).unwrap();
        }
        assert!(take_acknowledged().is_none());
        assert!(PANEL.with(|p| p.borrow().is_none()));
    }
}
