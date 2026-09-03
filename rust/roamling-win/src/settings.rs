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
            .map(|text| {
                text.lines()
                    .filter_map(|line| line.split_once('='))
                    .map(|(key, value)| (key.trim().to_string(), value.trim().to_string()))
                    .collect()
            })
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
