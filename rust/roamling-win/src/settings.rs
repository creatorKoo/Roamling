// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

//! What survives a restart.
//!
//! macOS uses `UserDefaults`. Windows has no equivalent worth reaching for, so
//! this is a flat `key=value` file under `%APPDATA%\Roamling`.
//!
//! **The keys are the ones `RoamlingRuntime` already uses** -- `roamling.roaming`,
//! `roamling.position.x` and the rest. Same vocabulary on both platforms, so a
//! question like "is roaming off?" has one answer to look up rather than two.
//! The defaults match `register(defaults:)` there: roaming, pointer avoidance
//! and interactions all start on.

use std::collections::BTreeMap;
use std::path::PathBuf;

pub const ROAMING: &str = "roamling.roaming";
pub const AVOID_POINTER: &str = "roamling.avoidPointer";
pub const INTERACTIONS: &str = "roamling.interactions";
pub const POSITION_X: &str = "roamling.position.x";
pub const POSITION_Y: &str = "roamling.position.y";
pub const HAS_POSITION: &str = "roamling.position.exists";
/// Windows-only. macOS gates these on TCC permission; there is no equivalent
/// here, so the setting *is* the consent. Both default off.
pub const VISUAL_PLACEMENT: &str = "roamling.visualPlacement";
pub const CURSOR_AWARENESS: &str = "roamling.cursorAwareness";
pub const SCALE: &str = "roamling.scale";

pub struct Settings {
    values: BTreeMap<String, String>,
    path: Option<PathBuf>,
}

impl Settings {
    pub fn load() -> Self {
        let path = std::env::var_os("APPDATA").map(|base| {
            let mut path = PathBuf::from(base);
            path.push("Roamling");
            path.push("settings.txt");
            path
        });
        let values = path
            .as_ref()
            .and_then(|path| std::fs::read_to_string(path).ok())
            .map(|text| parse(&text))
            .unwrap_or_default();
        Self { values, path }
    }

    pub fn bool(&self, key: &str, default: bool) -> bool {
        match self.values.get(key).map(String::as_str) {
            Some("true") => true,
            Some("false") => false,
            _ => default,
        }
    }

    pub fn text(&self, key: &str) -> Option<String> {
        self.values.get(key).cloned()
    }

    pub fn number(&self, key: &str) -> Option<f64> {
        self.values.get(key)?.parse().ok()
    }

    /// Writes only when something actually changed, so the drop-and-persist
    /// path does not touch the disk once per tick while the pet sits still.
    pub fn set(&mut self, key: &str, value: impl ToString) {
        let value = value.to_string();
        if self.values.get(key) == Some(&value) {
            return;
        }
        self.values.insert(key.to_string(), value);
        self.write();
    }

    fn write(&self) {
        let Some(path) = self.path.as_ref() else {
            return;
        };
        let Some(parent) = path.parent() else { return };
        if std::fs::create_dir_all(parent).is_err() {
            return;
        }
        let mut text = String::from("# Roamling settings. Keys match the macOS UserDefaults.\n");
        for (key, value) in &self.values {
            text.push_str(key);
            text.push('=');
            text.push_str(value);
            text.push('\n');
        }
        // Through a temporary so a crash mid-write cannot leave a truncated
        // file that reads as "no saved position".
        let temporary = path.with_extension("tmp");
        if std::fs::write(&temporary, text).is_ok() {
            let _ = std::fs::rename(&temporary, path);
        }
    }
}

/// `key=value` a line at a time.
///
/// Notepad and PowerShell's `Set-Content -Encoding utf8` both put a byte-order
/// mark at the front of the file. Left on, it becomes part of the first key and
/// that key silently stops resolving -- the pet forgets one setting and nothing
/// says why. Found because a test run wrote the file that way.
fn parse(text: &str) -> BTreeMap<String, String> {
    text.trim_start_matches('\u{feff}')
        .lines()
        .filter_map(|line| line.split_once('='))
        .map(|(key, value)| {
            (
                key.trim().trim_start_matches('\u{feff}').to_string(),
                value.trim().to_string(),
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_byte_order_mark_does_not_eat_the_first_key() {
        let values = parse("\u{feff}roamling.roaming=false\nroamling.avoidPointer=true\n");
        assert_eq!(
            values.get(ROAMING).map(String::as_str),
            Some("false"),
            "the mark became part of the key: {values:?}"
        );
        assert_eq!(values.len(), 2, "a mangled key survived: {values:?}");
    }

    /// The comment the writer puts at the top has no `=`, and neither do blank
    /// lines. Neither should turn into an entry.
    #[test]
    fn comments_and_blanks_are_skipped() {
        let values = parse("# a comment\n\nroamling.roaming=true\n");
        assert_eq!(values.len(), 1);
    }
}
