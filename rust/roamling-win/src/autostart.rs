// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

//! Start at sign-in, the way the installer already offers it: one string value
//! named `Roamling` under the per-user `Run` key. The registry is the record --
//! nothing is copied into `settings.txt` -- so a box ticked in the installer
//! shows as ticked here, and a value removed here is gone for the installer
//! too. Win32 types stay inside this file.

use windows::core::PCWSTR;
use windows::Win32::Foundation::ERROR_SUCCESS;
use windows::Win32::System::Registry::{
    RegCloseKey, RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW, RegSetValueExW,
    HKEY, HKEY_CURRENT_USER, KEY_QUERY_VALUE, KEY_SET_VALUE, REG_SZ,
};

const RUN_KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";
/// The same value name `installer/roamling.iss` writes and its uninstaller
/// deletes.
const VALUE: &str = "Roamling";

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

fn open(access: windows::Win32::System::Registry::REG_SAM_FLAGS) -> Option<HKEY> {
    let path = wide(RUN_KEY);
    let mut key = HKEY::default();
    let status = unsafe {
        RegOpenKeyExW(HKEY_CURRENT_USER, PCWSTR(path.as_ptr()), 0, access, &mut key)
    };
    (status == ERROR_SUCCESS).then_some(key)
}

/// Whether the value is there. Whatever it holds counts: a path from an older
/// install location still means "the user asked for this".
pub fn is_enabled() -> bool {
    let Some(key) = open(KEY_QUERY_VALUE) else { return false };
    let name = wide(VALUE);
    let status = unsafe {
        RegQueryValueExW(key, PCWSTR(name.as_ptr()), None, None, None, None)
    };
    unsafe {
        let _ = RegCloseKey(key);
    }
    status == ERROR_SUCCESS
}

/// Write the running exe's path, quoted the way the installer quotes it, or
/// remove the value. Removing what is not there is not an error.
pub fn set(enabled: bool) -> Result<(), String> {
    // Resolved before the key opens, so no early return can skip the close.
    let exe = if enabled {
        Some(std::env::current_exe().map_err(|e| e.to_string())?)
    } else {
        None
    };
    let key = open(KEY_SET_VALUE).ok_or_else(|| "the Run key did not open".to_string())?;
    let name = wide(VALUE);
    let result = if let Some(exe) = exe {
        let data = wide(&format!("\"{}\"", exe.display()));
        let bytes: &[u8] = unsafe {
            std::slice::from_raw_parts(data.as_ptr() as *const u8, data.len() * 2)
        };
        let status = unsafe {
            RegSetValueExW(key, PCWSTR(name.as_ptr()), 0, REG_SZ, Some(bytes))
        };
        if status == ERROR_SUCCESS {
            Ok(())
        } else {
            Err(format!("RegSetValueExW: {}", status.0))
        }
    } else {
        let status = unsafe { RegDeleteValueW(key, PCWSTR(name.as_ptr())) };
        if status == ERROR_SUCCESS || status == windows::Win32::Foundation::ERROR_FILE_NOT_FOUND {
            Ok(())
        } else {
            Err(format!("RegDeleteValueW: {}", status.0))
        }
    };
    unsafe {
        let _ = RegCloseKey(key);
    }
    result
}
