// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

//! The notification-area icon and its menu.
//!
//! macOS puts this in the menu bar and builds it from `ShellMenu`, which holds
//! the tree as data. That module is Swift, so this one carries its own small
//! tree for now, in the same order and with the same words.
//!
//! The strings still come from the shared `.strings` files. See `strings.rs`.

use crate::strings::{localized, localized_format};
use roamling_agent::{installer, Agent};

use windows::core::PCWSTR;
use windows::Win32::Foundation::{HWND, LPARAM, POINT, WPARAM};

use windows::Win32::UI::Shell::{
    Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW,
};
use windows::Win32::UI::WindowsAndMessaging::*;

/// The tray's callback lands here. `WM_APP` upwards is reserved for the app.
pub const WM_TRAY: u32 = WM_APP + 1;

pub const CMD_ROAMING: usize = 1;
pub const CMD_AVOID_POINTER: usize = 2;
pub const CMD_INTERACTIONS: usize = 3;
pub const CMD_VISUAL: usize = 4;
pub const CMD_CURSOR_AWARE: usize = 5;
pub const CMD_QUIT: usize = 6;
pub const CMD_OPEN_PET_FOLDER: usize = 7;
pub const CMD_COPY_DIAGNOSTICS: usize = 8;
pub const CMD_ABOUT: usize = 9;
pub const CMD_TUNING: usize = 11;
pub const CMD_RELOAD_PETS: usize = 12;
pub const CMD_UPDATE_CHECK: usize = 13;
pub const CMD_UPDATE_AUTO: usize = 14;
pub const CMD_LAUNCH_AT_LOGIN: usize = 15;
pub const CMD_HIDE: usize = 16;
pub const CMD_USAGE_GUIDE: usize = 18;
/// One id per entry in `built_in_mochi_presets`, in menu order.
pub const CMD_PALETTE_BASE: usize = 300;
pub const CMD_SSAL_PALETTE_BASE: usize = 400;
/// The built-in mascot, then one id per discovered package.
pub const CMD_PET_BUILT_IN: usize = 1_000;
pub const CMD_PET_BASE: usize = 1_001;
/// One id per entry in `SCALE_CHOICES`, in that order.
pub const CMD_SCALE_BASE: usize = 20;
/// One block of ids per agent, so the handler can tell which one was picked
/// without a second lookup table.
pub const CMD_AGENT_BASE: usize = 100;
pub const CMD_AGENT_STRIDE: usize = 10;
pub const CMD_AGENT_INSTALL: usize = 0;
pub const CMD_AGENT_REMOVE: usize = 1;
pub const CMD_AGENT_TEST: usize = 2;
/// One id per app in `MenuState.work_apps`, in menu order.
pub const CMD_WORK_APP_BASE: usize = 200;

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Use the same size-specific resource as Explorer and the application icon.
fn app_icon() -> Option<HICON> {
    unsafe {
        let module = windows::Win32::System::LibraryLoader::GetModuleHandleW(None).ok()?;
        let instance: windows::Win32::Foundation::HINSTANCE = module.into();
        let side = GetSystemMetrics(SM_CXSMICON).max(16);
        let icon = LoadImageW(Some(&instance), PCWSTR(1usize as *const u16), IMAGE_ICON,
            side, side, LR_SHARED).ok()?;
        Some(HICON(icon.0))
    }
}

fn data(hwnd: HWND) -> NOTIFYICONDATAW {
    NOTIFYICONDATAW {
        cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: hwnd,
        uID: 1,
        ..Default::default()
    }
}

/// Returns whether the shell accepted it. Windows 11 files new icons into the
/// overflow flyout by default, so "registered" and "visible" are not the same
/// thing -- the user pins it, or it lives behind the chevron.
pub fn add(hwnd: HWND) -> bool {
    let mut entry = data(hwnd);
    entry.uFlags = NIF_MESSAGE | NIF_TIP | NIF_ICON;
    entry.uCallbackMessage = WM_TRAY;
    if let Some(icon) = app_icon() {
        entry.hIcon = icon;
    }
    let tip = wide(localized("app.name"));
    entry.szTip[..tip.len()].copy_from_slice(&tip);
    unsafe { Shell_NotifyIconW(NIM_ADD, &entry).as_bool() }
}

pub fn remove(hwnd: HWND) {
    unsafe {
        let _ = Shell_NotifyIconW(NIM_DELETE, &data(hwnd));
    }
}

/// What the menu shows. A struct rather than a row of bools, which is the shape
/// that eventually gets one of them silently swapped.
#[derive(Clone)]
pub struct MenuState {
    /// The pet's own name, for the caption line.
    pub pet_name: String,
    /// How many of the sixteen capabilities the sheet answers for, and which
    /// ones are borrowed or missing. A package that declares one animation
    /// renders it for every state, and from outside that looks like a pet whose
    /// behaviour is broken rather than one whose sheet is thin. Say which.
    pub covered: usize,
    pub total: usize,
    pub substituted: Vec<String>,
    pub placeholder: Vec<String>,
    /// The user's own size multiplier, on top of the display's scale.
    pub scale: f64,
    /// Discovered packages and whether each is the one in use. Empty when
    /// nothing is installed, which is the ordinary case.
    pub pets: Vec<(String, bool)>,
    /// Whether the built-in mascot is the one showing.
    pub built_in: bool,
    /// Which colour preset is ticked, by index into `built_in_mochi_presets`.
    /// `None` for a legacy saved custom colour that matches no current preset.
    pub palette: Option<usize>,
    pub ssal_package: Option<usize>,
    pub ssal_palette: Option<usize>,
    pub ssal_selected: bool,
    pub auto_update: bool,
    /// Whether the OS starts the app at sign-in, read from the registry.
    pub launch_at_login: bool,
    /// The version waiting for a restart, if a swap has already happened.
    pub staged: Option<String>,
    pub checking: bool,
    /// Each agent, whether its hook is installed, stale or absent, and whether
    /// its endpoint came up.
    pub agents: [(Agent, installer::Status, bool); 2],
    /// Recently seen apps first, then selected apps that are not running.
    pub work_apps: Vec<(String, String, bool)>,
    pub hidden: bool,
    pub roaming: bool,
    pub avoiding: bool,
    pub interactive: bool,
    pub visual: bool,
    pub cursor_aware: bool,
}

/// The sizes the menu offers, matching `ShellMenu.scaleChoices`.
pub const SCALE_CHOICES: [(&str, f64); 5] = [
    ("0.5x", 0.5),
    ("0.75x", 0.75),
    ("1.0x", 1.0),
    ("1.25x", 1.25),
    ("1.5x", 1.5),
];

/// A line that reports rather than commands. AppKit draws these disabled and so
/// does Win32, which is the whole reason a caption cannot be clicked by mistake.
unsafe fn caption(menu: HMENU, text: &str) {
    let label = wide(text);
    let _ = AppendMenuW(
        menu,
        MF_STRING | MF_DISABLED | MF_GRAYED,
        0,
        PCWSTR(label.as_ptr()),
    );
}

unsafe fn command(menu: HMENU, id: usize, text: &str) {
    let label = wide(text);
    let _ = AppendMenuW(menu, MF_STRING, id, PCWSTR(label.as_ptr()));
}

unsafe fn attach(parent: HMENU, submenu: HMENU, title: &str) {
    let label = wide(title);
    let _ = AppendMenuW(
        parent,
        MF_STRING | MF_POPUP,
        submenu.0 as usize,
        PCWSTR(label.as_ptr()),
    );
}

thread_local! {
    static MENU_OPEN: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Whether the tray menu is up. Ticks keep running inside its nested message
/// loop, so a tick can ask -- and an update must not restart the pet from
/// under an open menu.
pub fn is_menu_open() -> bool {
    MENU_OPEN.with(std::cell::Cell::get)
}

/// Show the menu and return the chosen command, or 0.
///
/// `TPM_RETURNCMD` keeps the answer here instead of routing a `WM_COMMAND`
/// back through the window procedure, which matters because that procedure is
/// re-entrancy-sensitive -- see the note on `wndproc`.
pub fn show_menu(hwnd: HWND, state: MenuState) -> usize {
    unsafe {
        let Some(menu) = build(&state) else {
            return 0;
        };

        let mut cursor = POINT::default();
        let _ = GetCursorPos(&mut cursor);
        // Without the foreground dance the menu will not dismiss when the user
        // clicks away from it. A tray menu has needed both halves since Win95.
        let _ = SetForegroundWindow(hwnd);
        MENU_OPEN.with(|open| open.set(true));
        let chosen = TrackPopupMenu(
            menu,
            TPM_RETURNCMD | TPM_RIGHTBUTTON,
            cursor.x,
            cursor.y,
            0,
            hwnd,
            None,
        );
        MENU_OPEN.with(|open| open.set(false));
        let _ = PostMessageW(hwnd, WM_NULL, WPARAM(0), LPARAM(0));
        let _ = DestroyMenu(menu);
        chosen.0 as usize
    }
}

/// The tree itself, separated from showing it so a test can build one.
///
/// `DestroyMenu` on the returned handle frees the submenus with it.
unsafe fn build(state: &MenuState) -> Option<HMENU> {
    {
        let menu = CreatePopupMenu().ok()?;
        let checked = |on: bool| if on { MF_CHECKED } else { MF_UNCHECKED };
        let separator = |menu| AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());

        let pet_name = if state.built_in {
            localized("pet.name.bori")
        } else {
            &state.pet_name
        };
        caption(menu, &localized_format("menu.title", &[pet_name]));
        let _ = separator(menu);
        let hide_label = wide(localized("menu.hide"));
        let _ = AppendMenuW(
            menu,
            MF_STRING | checked(state.hidden),
            CMD_HIDE,
            PCWSTR(hide_label.as_ptr()),
        );
        let _ = separator(menu);

        // Pet. Only the built-in for now -- installed packages arrive with the
        // catalogue -- but the coverage lines are real, read off the resolver.
        if let Ok(pets) = CreatePopupMenu() {
            let label = localized_format("menu.pet.builtin", &[localized("pet.name.bori")]);
            if let Ok(colours) = CreatePopupMenu() {
                for (index, (key, _)) in roamling_pet::built_in_mochi_presets().iter().enumerate() {
                    let label = wide(localized(key));
                    let _ = AppendMenuW(
                        colours,
                        MF_STRING | checked(state.built_in && state.palette == Some(index)),
                        CMD_PALETTE_BASE + index,
                        PCWSTR(label.as_ptr()),
                    );
                }
                let name = wide(&label);
                let _ = AppendMenuW(
                    pets,
                    MF_POPUP | MF_STRING | checked(state.built_in),
                    colours.0 as usize,
                    PCWSTR(name.as_ptr()),
                );
            }
            if let Ok(colours) = CreatePopupMenu() {
                for (preset, (key, _)) in roamling_core::pet_image::ssal::PRESETS.iter().enumerate() {
                    let label = wide(localized(key));
                    let _ = AppendMenuW(colours,
                        MF_STRING | checked(state.ssal_selected && state.ssal_palette == Some(preset)),
                        CMD_SSAL_PALETTE_BASE + preset, PCWSTR(label.as_ptr()));
                }
                let label = wide(&localized_format("menu.pet.builtin", &[localized("pet.name.ssal")]));
                let _ = AppendMenuW(pets, MF_POPUP | MF_STRING | checked(state.ssal_selected),
                    colours.0 as usize, PCWSTR(label.as_ptr()));
            }
            if !state.pets.is_empty() { let _ = separator(pets); }
            for (index, (name, selected)) in state.pets.iter().enumerate() {
                // The approved built-in replaces the earlier external Ssal trial entry.
                if state.ssal_package == Some(index) { continue; }
                let label = wide(name);
                let _ = AppendMenuW(
                    pets,
                    MF_STRING | checked(*selected),
                    CMD_PET_BASE + index,
                    PCWSTR(label.as_ptr()),
                );
            }
            let _ = separator(pets);
            caption(
                pets,
                &localized_format(
                    "menu.pet.coverage",
                    &[&state.covered.to_string(), &state.total.to_string()],
                ),
            );
            if !state.substituted.is_empty() {
                caption(
                    pets,
                    &localized_format("menu.pet.substituted", &[&state.substituted.join(", ")]),
                );
            }
            if !state.placeholder.is_empty() {
                caption(
                    pets,
                    &localized_format("menu.pet.placeholder", &[&state.placeholder.join(", ")]),
                );
            }
            attach(menu, pets, localized("menu.pet"));
        }

        // Size.
        if let Ok(sizes) = CreatePopupMenu() {
            for (index, (label, value)) in SCALE_CHOICES.iter().enumerate() {
                let text = wide(label);
                let _ = AppendMenuW(
                    sizes,
                    MF_STRING | checked((state.scale - value).abs() < 0.01),
                    CMD_SCALE_BASE + index,
                    PCWSTR(text.as_ptr()),
                );
            }
            attach(menu, sizes, localized("menu.size"));
        }
        let _ = separator(menu);

        if let Ok(movement) = CreatePopupMenu() {
            for (flag, id, key) in [
                (checked(state.roaming), CMD_ROAMING, "menu.roaming"),
                (
                    checked(state.avoiding),
                    CMD_AVOID_POINTER,
                    "menu.avoidPointer",
                ),
                (
                    checked(state.interactive),
                    CMD_INTERACTIONS,
                    "menu.catchDrag",
                ),
            ] {
                let label = wide(localized(key));
                let _ = AppendMenuW(movement, MF_STRING | flag, id, PCWSTR(label.as_ptr()));
            }
            command(movement, CMD_TUNING, localized("menu.tuning"));
            attach(menu, movement, localized("menu.movement"));
        }

        if let Ok(awareness) = CreatePopupMenu() {
            // On macOS these two are submenus reporting an OS permission the
            // app can only ask for. Windows grants both outright -- there is no
            // prompt to route the user to -- so consent is the checkmark itself.
            // `docs/windows.md`, the permission model.
            for (flag, id, key) in [
                (
                    checked(state.cursor_aware),
                    CMD_CURSOR_AWARE,
                    "menu.accessibility",
                ),
                (checked(state.visual), CMD_VISUAL, "menu.visualPlacement"),
            ] {
                let label = wide(localized(key));
                let _ = AppendMenuW(awareness, MF_STRING | flag, id, PCWSTR(label.as_ptr()));
            }

            if let Ok(work_apps) = CreatePopupMenu() {
                if state.work_apps.is_empty() {
                    caption(work_apps, localized("menu.workApps.none"));
                } else {
                    for (index, (label, _, selected)) in state.work_apps.iter().enumerate() {
                        let label = wide(label);
                        let _ = AppendMenuW(
                            work_apps,
                            MF_STRING | checked(*selected),
                            CMD_WORK_APP_BASE + index,
                            PCWSTR(label.as_ptr()),
                        );
                    }
                }
                attach(awareness, work_apps, localized("menu.workApps"));
            }

            // One submenu per agent, the same shape `ShellMenu.agentItems`
            // builds: two status lines that cannot be clicked, then
            // install-or-repair, remove when applicable, and a test reaction.
            for (index, (agent, status, listening)) in state.agents.iter().enumerate() {
                let Ok(submenu) = CreatePopupMenu() else {
                    continue;
                };
                let base = CMD_AGENT_BASE + index * CMD_AGENT_STRIDE;

                caption(
                    submenu,
                    localized(match status {
                        installer::Status::Installed => "status.hooks.installed",
                        installer::Status::NeedsRepair => "status.hooks.needsRepair",
                        installer::Status::NotInstalled => "status.hooks.notInstalled",
                    }),
                );
                // Two states, not four: the endpoint either bound at launch or
                // it did not. There is no async start to be "starting" during,
                // and it is only stopped when the app is going away.
                caption(
                    submenu,
                    localized(if *listening {
                        "status.receiver.ready"
                    } else {
                        "status.receiver.unavailable"
                    }),
                );
                let _ = separator(submenu);

                command(
                    submenu,
                    base + CMD_AGENT_INSTALL,
                    localized(if *status == installer::Status::NotInstalled {
                        "action.install"
                    } else {
                        "action.repair"
                    }),
                );
                if *status != installer::Status::NotInstalled {
                    command(submenu, base + CMD_AGENT_REMOVE, localized("action.remove"));
                }
                command(
                    submenu,
                    base + CMD_AGENT_TEST,
                    localized("action.testReaction"),
                );
                attach(awareness, submenu, agent.display_name());
            }
            attach(menu, awareness, localized("menu.awareness"));
        }

        let _ = separator(menu);
        if let Ok(advanced) = CreatePopupMenu() {
            command(
                advanced,
                CMD_OPEN_PET_FOLDER,
                localized("menu.openPetFolder"),
            );
            command(
                advanced,
                CMD_COPY_DIAGNOSTICS,
                localized("menu.copyDiagnostics"),
            );
            command(advanced, CMD_RELOAD_PETS, localized("menu.reloadPets"));
            if state.checking {
                caption(advanced, localized("status.update.checking"));
            } else if state.staged.is_none() {
                command(advanced, CMD_UPDATE_CHECK, localized("menu.update.check"));
            }
            let login_label = wide(localized("menu.launchAtLogin"));
            let _ = AppendMenuW(
                advanced,
                MF_STRING | checked(state.launch_at_login),
                CMD_LAUNCH_AT_LOGIN,
                PCWSTR(login_label.as_ptr()),
            );
            let update_label = wide(localized("menu.update.auto"));
            let _ = AppendMenuW(
                advanced,
                MF_STRING | checked(state.auto_update),
                CMD_UPDATE_AUTO,
                PCWSTR(update_label.as_ptr()),
            );
            attach(menu, advanced, localized("menu.advanced"));
        }

        // A staged update is an alert, not a setting. Keep its quiet caption
        // at the top level even though all update controls are under Advanced.
        if let Some(version) = &state.staged {
            caption(menu, &localized_format("status.update.ready", &[version]));
        }

        let _ = separator(menu);
        // "View Source" is not here: it is a button inside About, which is
        // where macOS puts it.
        command(menu, CMD_USAGE_GUIDE, localized("menu.usageGuide"));
        command(menu, CMD_ABOUT, localized("menu.about"));
        command(menu, CMD_QUIT, localized("menu.quit"));
        Some(menu)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state() -> MenuState {
        MenuState {
            pet_name: "Bori".into(),
            covered: 14,
            total: 16,
            substituted: vec!["sit".into()],
            placeholder: vec![],
            scale: 1.0,
            pets: vec![
                ("Installed One".into(), true),
                ("Installed Two".into(), false),
            ],
            built_in: false,
            palette: Some(0),
            ssal_package: None,
            ssal_palette: Some(0),
            ssal_selected: false,
            auto_update: true,
            launch_at_login: false,
            staged: Some("0.2.0".into()),
            checking: false,
            agents: [
                (Agent::ClaudeCode, installer::Status::Installed, true),
                (Agent::Codex, installer::Status::NotInstalled, false),
            ],
            work_apps: vec![
                ("HWP 2024".into(), "Hwp.exe".into(), true),
                ("Notepad".into(), "notepad.exe".into(), false),
            ],
            hidden: false,
            roaming: true,
            avoiding: true,
            interactive: true,
            visual: false,
            cursor_aware: false,
        }
    }

    /// Every command id the dispatcher answers, gathered off a real menu.
    fn ids(menu: HMENU) -> Vec<usize> {
        let mut found = Vec::new();
        unsafe {
            for index in 0..GetMenuItemCount(menu) {
                let id = GetMenuItemID(menu, index);
                if id != u32::MAX && id != 0 {
                    found.push(id as usize);
                }
                let submenu = GetSubMenu(menu, index);
                if !submenu.is_invalid() {
                    found.extend(ids(submenu));
                }
            }
        }
        found
    }

    /// The tree is Win32 objects rather than a value, so the only way to know
    /// it came out whole is to build one. This catches an id colliding with
    /// another -- the ranges are hand-assigned -- and a submenu that failed to
    /// attach, which would silently drop everything under it.
    #[test]
    fn the_tree_builds_with_every_command_reachable() {
        let mut state = state();
        state.staged = None;
        let menu = unsafe { build(&state) }.expect("the menu did not build");
        let found = ids(menu);
        unsafe {
            let _ = DestroyMenu(menu);
        }

        let agent_one = CMD_AGENT_BASE;
        let agent_two = CMD_AGENT_BASE + CMD_AGENT_STRIDE;
        let mut expected = vec![
            CMD_HIDE,
            CMD_ROAMING,
            CMD_AVOID_POINTER,
            CMD_INTERACTIONS,
            CMD_VISUAL,
            CMD_CURSOR_AWARE,
            CMD_OPEN_PET_FOLDER,
            CMD_COPY_DIAGNOSTICS,
            CMD_ABOUT,
            CMD_USAGE_GUIDE,
            CMD_QUIT,
            CMD_TUNING,
            CMD_RELOAD_PETS,
            CMD_UPDATE_CHECK,
            CMD_UPDATE_AUTO,
            CMD_LAUNCH_AT_LOGIN,
            CMD_PET_BASE,
            CMD_PET_BASE + 1,
            CMD_SCALE_BASE,
            CMD_SCALE_BASE + SCALE_CHOICES.len() - 1,
            // Installed, so it offers repair, remove and a test.
            agent_one + CMD_AGENT_INSTALL,
            agent_one + CMD_AGENT_REMOVE,
            agent_one + CMD_AGENT_TEST,
            // Not installed, so there is nothing to remove.
            agent_two + CMD_AGENT_INSTALL,
            agent_two + CMD_AGENT_TEST,
            CMD_WORK_APP_BASE,
            CMD_WORK_APP_BASE + 1,
        ];
        // Every approved colour preset is reachable.
        for index in 0..roamling_pet::built_in_mochi_presets().len() {
            expected.push(CMD_PALETTE_BASE + index);
        }
        for id in expected {
            assert!(found.contains(&id), "{id} is not in the menu: {found:?}");
        }
        assert!(
            !found.contains(&(agent_two + CMD_AGENT_REMOVE)),
            "an agent with no hooks offered Remove"
        );

        let mut seen = found.clone();
        seen.sort_unstable();
        let before = seen.len();
        seen.dedup();
        assert_eq!(
            before,
            seen.len(),
            "two items share a command id: {found:?}"
        );
    }

    #[test]
    fn ssal_has_its_own_colour_commands_without_hiding_bori() {
        let mut state = state();
        state.pets = vec![("White Ssal".into(), true)];
        state.built_in = false;
        state.ssal_package = Some(0);
        state.ssal_palette = Some(1);
        let menu = unsafe { build(&state) }.unwrap();
        let commands = ids(menu);
        for index in 0..roamling_core::pet_image::ssal::PRESETS.len() {
            assert!(commands.contains(&(CMD_SSAL_PALETTE_BASE + index)));
        }
        for command in [
            CMD_SSAL_PALETTE_BASE,
            CMD_SSAL_PALETTE_BASE + 1,
            CMD_PALETTE_BASE,
        ] {
            assert!(commands.contains(&command));
        }
        assert!(!commands.contains(&CMD_PET_BASE));
        let mut unique = commands.clone();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(unique.len(), commands.len());
        unsafe {
            let _ = DestroyMenu(menu);
        }
    }

    #[test]
    fn colours_are_directly_under_mochi_with_the_selection_checked() {
        for built_in in [false, true] {
            {
                let mut state = state();
                state.built_in = built_in;
                state.pet_name = "Guest Mochi".into(); // External package names must remain unchanged.
                state.palette = Some(1); // Black, as restored from settings.
                let menu = unsafe { build(&state) }.unwrap();
                unsafe {
                    let mut caption = [0u16; 128];
                    let length = GetMenuStringW(menu, 0, Some(&mut caption), MF_BYPOSITION);
                    let name = if built_in {
                        localized("pet.name.bori")
                    } else {
                        &state.pet_name
                    };
                    assert_eq!(String::from_utf16_lossy(&caption[..length as usize]),
                        localized_format("menu.title", &[name]));
                    let pets = GetSubMenu(menu, 4);
                    let mochi = GetSubMenu(pets, 0);
                    assert!(!mochi.is_invalid());
                    let mut title = [0u16; 128];
                    let length = GetMenuStringW(pets, 0, Some(&mut title), MF_BYPOSITION);
                    assert_eq!(String::from_utf16_lossy(&title[..length as usize]),
                        localized_format("menu.pet.builtin", &[localized("pet.name.bori")]));
                    let presets = roamling_pet::built_in_mochi_presets();
                    assert_eq!(GetMenuItemCount(mochi) as usize,
                        presets.len());
                    for index in 0..presets.len() {
                        assert_eq!(GetMenuItemID(mochi, index as i32) as usize, CMD_PALETTE_BASE + index);
                        assert!(GetSubMenu(mochi, index as i32).is_invalid());
                        let checked = GetMenuState(mochi, index as u32, MF_BYPOSITION) & MF_CHECKED.0 != 0;
                        assert_eq!(checked, built_in && index == 1);
                    }
                    assert_eq!(GetMenuState(pets, 0, MF_BYPOSITION) & MF_CHECKED.0 != 0, built_in);
                    // Former custom commands must never be exposed, even as hidden entries.
                    assert!(!ids(menu).contains(&17));
                    assert!(!ids(menu).contains(&450));
                    let ssal = GetSubMenu(pets, 1);
                    assert_eq!(GetMenuItemCount(ssal) as usize,
                        roamling_core::pet_image::ssal::PRESETS.len());
                    let _ = DestroyMenu(menu);
                }
            }
        }
    }

    #[test]
    fn the_three_folded_menus_keep_every_command_reachable_and_quit_last() {
        let mut state = state();
        state.staged = None;
        let menu = unsafe { build(&state) }.expect("the menu did not build");
        let count = unsafe { GetMenuItemCount(menu) };
        assert_eq!(count, 15, "the ordinary top-level menu changed length");
        assert_eq!(unsafe { GetMenuItemID(menu, 2) } as usize, CMD_HIDE);
        assert_eq!(unsafe { GetMenuItemID(menu, count - 3) } as usize, CMD_USAGE_GUIDE);
        assert_eq!(
            unsafe { GetMenuItemID(menu, count - 2) } as usize,
            CMD_ABOUT
        );
        assert_eq!(unsafe { GetMenuItemID(menu, count - 1) } as usize, CMD_QUIT);

        let submenus: Vec<HMENU> = (0..count)
            .map(|index| unsafe { GetSubMenu(menu, index) })
            .filter(|submenu| !submenu.is_invalid())
            .collect();
        let movement = *submenus
            .iter()
            .find(|submenu| ids(**submenu).contains(&CMD_ROAMING))
            .expect("Movement is missing");
        let awareness = *submenus
            .iter()
            .find(|submenu| ids(**submenu).contains(&CMD_CURSOR_AWARE))
            .expect("Awareness is missing");
        let advanced = *submenus
            .iter()
            .find(|submenu| ids(**submenu).contains(&CMD_OPEN_PET_FOLDER))
            .expect("Advanced is missing");

        assert_eq!(
            ids(movement),
            vec![CMD_ROAMING, CMD_AVOID_POINTER, CMD_INTERACTIONS, CMD_TUNING]
        );
        let agent_one = CMD_AGENT_BASE;
        let agent_two = CMD_AGENT_BASE + CMD_AGENT_STRIDE;
        assert_eq!(
            ids(awareness),
            vec![
                CMD_CURSOR_AWARE,
                CMD_VISUAL,
                CMD_WORK_APP_BASE,
                CMD_WORK_APP_BASE + 1,
                agent_one + CMD_AGENT_INSTALL,
                agent_one + CMD_AGENT_REMOVE,
                agent_one + CMD_AGENT_TEST,
                agent_two + CMD_AGENT_INSTALL,
                agent_two + CMD_AGENT_TEST,
            ]
        );
        assert_eq!(
            ids(advanced),
            vec![
                CMD_OPEN_PET_FOLDER,
                CMD_COPY_DIAGNOSTICS,
                CMD_RELOAD_PETS,
                CMD_UPDATE_CHECK,
                CMD_LAUNCH_AT_LOGIN,
                CMD_UPDATE_AUTO,
            ]
        );

        // Caption and separators are not choices. The agents live under
        // Awareness, so the top level has the same nine choices as the
        // agent-free Swift harness.
        let choice_positions = [2, 4, 5, 7, 8, 10, 12, 13, 14];
        assert_eq!(choice_positions.len(), 9);
        for index in choice_positions {
            let id = unsafe { GetMenuItemID(menu, index) };
            let submenu = unsafe { GetSubMenu(menu, index) };
            assert!(
                id != u32::MAX || !submenu.is_invalid(),
                "row {index} is not a choice"
            );
        }
        unsafe {
            let _ = DestroyMenu(menu);
        }
    }

    #[test]
    fn a_staged_update_adds_a_disabled_top_level_alert() {
        let state = state();
        let menu = unsafe { build(&state) }.expect("the menu did not build");
        let count = unsafe { GetMenuItemCount(menu) };
        assert_eq!(count, 16, "the staged alert did not add one top-level row");
        let alert = unsafe { GetMenuState(menu, 11, MF_BYPOSITION) };
        assert_ne!(alert, u32::MAX, "the staged alert is missing");
        assert!(
            alert & (MF_DISABLED.0 | MF_GRAYED.0) != 0,
            "the staged alert is clickable"
        );
        assert!(
            !ids(menu).contains(&CMD_UPDATE_CHECK),
            "a staged update still offered another check"
        );
        assert_eq!(unsafe { GetMenuItemID(menu, count - 1) } as usize, CMD_QUIT);
        unsafe {
            let _ = DestroyMenu(menu);
        }
    }

    /// A caption reports; it must not be pickable, or "Animations: 14 of 16"
    /// would return an id the dispatcher has never heard of.
    #[test]
    fn captions_cannot_be_chosen() {
        let state = state();
        let menu = unsafe { build(&state) }.expect("the menu did not build");
        let title = unsafe { GetMenuState(menu, 0, MF_BYPOSITION) };
        unsafe {
            let _ = DestroyMenu(menu);
        }
        assert_ne!(title, u32::MAX, "the title line is missing");
        assert!(
            title & (MF_DISABLED.0 | MF_GRAYED.0) != 0,
            "the title caption is clickable"
        );
    }
}
