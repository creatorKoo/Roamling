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

use std::collections::{BTreeMap, HashMap};
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
pub const PET_PACKAGE_PATH: &str = "roamling.petPackagePath";
pub const AUTO_UPDATE: &str = "roamling.autoUpdate";
pub const WORK_APPS: &str = "roamling.workApps";
pub const DEFAULT_WORK_APPS: [&str; 6] = [
    "Hwp.exe",
    "HShow.exe",
    "HCell.exe",
    "WINWORD.EXE",
    "POWERPNT.EXE",
    "EXCEL.EXE",
];
pub const WORK_APP_LABEL_PREFIX: &str = "roamling.workAppLabel.";
/// macOS keeps the whole tuning as one JSON blob under `roamling.runtimeTuning`,
/// because `UserDefaults` can hold data. This file is flat text, so the eleven
/// values get eleven sub-keys under the same name -- which also means the file
/// stays something a person can read and edit.
pub const TUNING_PREFIX: &str = "roamling.runtimeTuning.";

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
        Self::load_from(path)
    }

    pub(crate) fn load_from(path: Option<PathBuf>) -> Self {
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

    /// Missing means the authored defaults; a present empty value is the
    /// user's explicit choice to watch nothing.
    pub fn work_apps(&self) -> Vec<String> {
        match self.values.get(WORK_APPS) {
            Some(stored) => stored
                .split(',')
                .map(str::trim)
                .filter(|app| !app.is_empty())
                .map(str::to_owned)
                .collect(),
            None => DEFAULT_WORK_APPS
                .iter()
                .map(|app| (*app).to_owned())
                .collect(),
        }
    }

    pub fn work_app_labels(&self) -> HashMap<String, String> {
        self.values
            .iter()
            .filter_map(|(key, label)| {
                let exe = key.strip_prefix(WORK_APP_LABEL_PREFIX)?;
                if exe.is_empty() || label.is_empty() || label.eq_ignore_ascii_case(exe) {
                    return None;
                }
                Some((exe.to_ascii_lowercase(), label.clone()))
            })
            .collect()
    }

    pub fn clear_work_app_labels_except(&mut self, work_apps: &[String]) {
        let stale_keys: Vec<String> = self
            .values
            .keys()
            .filter_map(|key| {
                let exe = key.strip_prefix(WORK_APP_LABEL_PREFIX)?;
                let selected = work_apps
                    .iter()
                    .any(|work_app| work_app.eq_ignore_ascii_case(exe));
                (!selected).then(|| key.clone())
            })
            .collect();
        for key in stale_keys {
            self.clear(&key);
        }
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

    /// Forgets a key, so whatever supplies the fallback gets to answer again.
    ///
    /// This is what keeps a settings file from freezing the defaults that
    /// happened to be current when it was written -- see `remember_tuning`.
    pub fn clear(&mut self, key: &str) {
        if self.values.remove(key).is_some() {
            self.write();
        }
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

    /// A cleared key has to actually go, or whatever supplies the fallback --
    /// for the tuning values, the authored default -- never gets to answer.
    #[test]
    fn clearing_a_key_removes_it() {
        let mut settings = Settings {
            values: parse(
                "a=1
b=2
",
            ),
            path: None,
        };
        assert_eq!(settings.number("a"), Some(1.0));
        settings.clear("a");
        assert_eq!(settings.number("a"), None);
        assert_eq!(settings.number("b"), Some(2.0), "it took the wrong one");
        // Clearing something that was never there is not an error.
        settings.clear("nothing");
    }

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

    #[test]
    fn work_apps_distinguish_missing_from_empty() {
        let missing = Settings {
            values: BTreeMap::new(),
            path: None,
        };
        assert_eq!(
            missing.work_apps(),
            DEFAULT_WORK_APPS.map(str::to_owned).to_vec()
        );

        let empty = Settings {
            values: parse("roamling.workApps=\n"),
            path: None,
        };
        assert!(empty.work_apps().is_empty());
    }

    #[test]
    fn work_app_labels_survive_a_restart_one_key_per_app() {
        let directory = std::env::temp_dir().join(format!(
            "roamling-settings-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("the test clock is before Unix time")
                .as_nanos()
        ));
        let path = directory.join("settings.txt");
        let mut first = Settings::load_from(Some(path.clone()));
        first.set(WORK_APPS, "Hwp.exe");
        first.set(
            &format!("{WORK_APP_LABEL_PREFIX}hwp.exe"),
            "HWP 2024, Hancom Office",
        );
        drop(first);

        let restarted = Settings::load_from(Some(path.clone()));
        assert_eq!(
            restarted.work_app_labels(),
            HashMap::from([("hwp.exe".into(), "HWP 2024, Hancom Office".into())])
        );
        assert_eq!(
            restarted.text(&format!("{WORK_APP_LABEL_PREFIX}hwp.exe")),
            Some("HWP 2024, Hancom Office".into()),
            "the comma was treated as another app or setting"
        );

        std::fs::remove_file(path).expect("the throwaway settings file was not removed");
        std::fs::remove_dir(directory).expect("the throwaway settings directory was not removed");
    }

    #[test]
    fn unconfigured_work_app_labels_are_cleared_from_disk() {
        let directory = std::env::temp_dir().join(format!(
            "roamling-settings-cleanup-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("the test clock is before Unix time")
                .as_nanos()
        ));
        let path = directory.join("settings.txt");
        let selected_key = format!("{WORK_APP_LABEL_PREFIX}hwp.exe");
        let stale_key = format!("{WORK_APP_LABEL_PREFIX}windowsterminal.exe");
        let mut settings = Settings::load_from(Some(path.clone()));
        settings.set(WORK_APPS, "Hwp.exe");
        settings.set(&selected_key, "HWP 2024");
        settings.set(&stale_key, "Windows Terminal Host");

        let work_apps = settings.work_apps();
        settings.clear_work_app_labels_except(&work_apps);
        drop(settings);

        let restarted = Settings::load_from(Some(path.clone()));
        assert_eq!(restarted.text(&selected_key), Some("HWP 2024".into()));
        assert_eq!(restarted.text(&stale_key), None);

        std::fs::remove_file(path).expect("the throwaway settings file was not removed");
        std::fs::remove_dir(directory).expect("the throwaway settings directory was not removed");
    }

    #[test]
    fn executable_names_are_not_loaded_as_labels() {
        let settings = Settings {
            values: parse(
                "roamling.workAppLabel.WINWORD.EXE=winword.exe\n\
                 roamling.workAppLabel.hwp.exe=HWP 2024\n",
            ),
            path: None,
        };
        assert_eq!(
            settings.work_app_labels(),
            HashMap::from([("hwp.exe".into(), "HWP 2024".into())])
        );
    }
}
