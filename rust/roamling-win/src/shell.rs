// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

//! The menu items that talk to the rest of the desktop rather than to the pet.
//!
//! `ShellPrompt` on the macOS side returns these as effects -- `.reveal`,
//! `.copyToClipboard`, `.present` -- and the app delegate performs them. There
//! is no second consumer here, so they are performed directly.

use crate::strings::localized;
use roamling_agent::Agent;
use windows::core::PCWSTR;
use windows::Win32::Foundation::{HANDLE, HWND};
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData,
};
use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::{
    MessageBoxW, IDOK, MB_ICONINFORMATION, MB_ICONWARNING, MB_OKCANCEL, MB_OK, SW_SHOWNORMAL,
};

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Where a user drops a pet package.
///
/// `PetCatalog.userPetFolder` is `~/Library/Application Support/Roamling/Pets`;
/// `%APPDATA%\Roamling\Pets` is the same idea in the same rank -- first in the
/// search order, ahead of `~/.codex/pets`. Asked from one place, so the menu
/// cannot open a directory nothing reads.
pub fn user_pet_folder() -> Option<std::path::PathBuf> {
    let base = std::env::var_os("APPDATA").map(std::path::PathBuf::from)?;
    Some(base.join("Roamling").join("Pets"))
}

/// Opens it in Explorer, creating it first. A folder that does not exist yet is
/// the normal case on a fresh install, and "nothing happened" is a worse answer
/// than an empty window.
pub fn open_pet_folder() {
    let Some(folder) = user_pet_folder() else {
        return;
    };
    if std::fs::create_dir_all(&folder).is_err() {
        return;
    }
    let path = wide(&folder.to_string_lossy());
    let verb = wide("explore");
    unsafe {
        ShellExecuteW(
            None,
            PCWSTR(verb.as_ptr()),
            PCWSTR(path.as_ptr()),
            PCWSTR::null(),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        );
    }
}

/// Puts `text` on the clipboard as Unicode, returning whether it landed.
///
/// The buffer has to be movable global memory that the clipboard then owns, so
/// it is deliberately not freed on the success path -- freeing it is what would
/// be the bug.
pub fn copy_to_clipboard(hwnd: HWND, text: &str) -> bool {
    // CF_UNICODETEXT. The constant is a `u32` in the crate and `SetClipboardData`
    // takes one, so it is written out rather than fetched from three modules.
    const CF_UNICODETEXT: u32 = 13;

    let encoded = wide(text);
    let bytes = std::mem::size_of_val(&encoded[..]);
    unsafe {
        if OpenClipboard(hwnd).is_err() {
            return false;
        }
        let mut placed = false;
        if EmptyClipboard().is_ok() {
            if let Ok(handle) = GlobalAlloc(GMEM_MOVEABLE, bytes) {
                let target = GlobalLock(handle) as *mut u16;
                if !target.is_null() {
                    std::ptr::copy_nonoverlapping(encoded.as_ptr(), target, encoded.len());
                    let _ = GlobalUnlock(handle);
                    // The clipboard owns the handle from here, so a failure is
                    // the only path that may free it.
                    placed = SetClipboardData(CF_UNICODETEXT, HANDLE(handle.0)).is_ok();
                }
            }
        }
        let _ = CloseClipboard();
        placed
    }
}

/// Which strings belong to which agent.
///
/// Written out rather than built from the id, for the reason `ShellPrompt`
/// gives: a constructed key that is missing shows up as the key itself on
/// screen, and nothing greps for a name that was never typed.
pub struct AgentCopy {
    pub install_title: &'static str,
    pub install_body: &'static str,
    pub remove_title: &'static str,
    pub remove_body: &'static str,
    pub installed: &'static str,
    pub removed: &'static str,
    pub installed_detail: &'static str,
    pub removed_detail: &'static str,
    pub failure: &'static str,
}

pub fn agent_copy(agent: Agent) -> AgentCopy {
    match agent {
        Agent::ClaudeCode => AgentCopy {
            install_title: "alert.claude.install.title",
            install_body: "alert.claude.install.body",
            remove_title: "alert.claude.remove.title",
            remove_body: "alert.claude.remove.body",
            installed: "result.claude.installed",
            removed: "result.claude.removed",
            installed_detail: "result.detail.claude",
            removed_detail: "result.detail.claude",
            failure: "error.claude.settings",
        },
        Agent::Codex => AgentCopy {
            install_title: "alert.codex.install.title",
            install_body: "alert.codex.install.body",
            remove_title: "alert.codex.remove.title",
            remove_body: "alert.codex.remove.body",
            installed: "result.codex.installed",
            removed: "result.codex.removed",
            installed_detail: "result.detail.codex.installed",
            removed_detail: "result.detail.codex.removed",
            failure: "error.codex.hooks",
        },
    }
}

/// Asks before doing something to the user's own files, and returns whether to
/// go ahead.
///
/// `ShellPrompt.confirmation(for:)` labels the confirming button "Install" or
/// "Remove". `MessageBoxW` cannot relabel its buttons, so this is OK/Cancel --
/// which is fine, because the body says exactly what is about to be written.
pub fn confirm(hwnd: HWND, title: &'static str, body: &'static str) -> bool {
    let text = wide(localized(body));
    let caption = wide(localized(title));
    let answer = unsafe {
        MessageBoxW(
            hwnd,
            PCWSTR(text.as_ptr()),
            PCWSTR(caption.as_ptr()),
            MB_OKCANCEL | MB_ICONINFORMATION,
        )
    };
    answer == IDOK
}

/// Says what happened. The macOS alert has no buttons beyond dismissal either.
pub fn report(hwnd: HWND, title: &str, body: &str) {
    let text = wide(body);
    let caption = wide(title);
    unsafe {
        MessageBoxW(
            hwnd,
            PCWSTR(text.as_ptr()),
            PCWSTR(caption.as_ptr()),
            MB_OK | MB_ICONINFORMATION,
        );
    }
}

/// The same, for something that did not work.
pub fn warn(hwnd: HWND, title: &str, body: &str) {
    let text = wide(body);
    let caption = wide(title);
    unsafe {
        MessageBoxW(
            hwnd,
            PCWSTR(text.as_ptr()),
            PCWSTR(caption.as_ptr()),
            MB_OK | MB_ICONWARNING,
        );
    }
}

/// `ShellPrompt.sourceURL`. The licence notice in the About text is not
/// decoration -- this is GPL-3.0-only, and the offer of source belongs where a
/// user can act on it.
const SOURCE_URL: &str = "https://github.com/creatorKoo/Roamling";

/// `ShellPrompt.about`, as the dialog Windows has.
///
/// The macOS alert carries a second button for the source. `MessageBoxW` cannot
/// relabel its buttons, and a mislabelled one is worse than none, so the source
/// is its own menu item instead. Same two actions, one level up.
pub fn about(hwnd: HWND) {
    // Which build this is, first, because that is what someone opens this box
    // to find out -- "is the update I was told about actually running".
    let body = format!(
        "{}\r\n\r\n{}",
        crate::strings::localized_format("about.version", &[env!("CARGO_PKG_VERSION")]),
        localized("alert.about.body")
    );
    let text = wide(&body);
    let title = wide("Roamling");
    unsafe {
        MessageBoxW(
            hwnd,
            PCWSTR(text.as_ptr()),
            PCWSTR(title.as_ptr()),
            MB_OK | MB_ICONINFORMATION,
        );
    }
}

/// Opens the repository in whatever the user browses with.
pub fn view_source() {
    let url = wide(SOURCE_URL);
    let verb = wide("open");
    unsafe {
        ShellExecuteW(
            None,
            PCWSTR(verb.as_ptr()),
            PCWSTR(url.as_ptr()),
            PCWSTR::null(),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        );
    }
}
