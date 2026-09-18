// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

//! What the runtime leaves in the settings file, and under which key.
//!
//! `settings.rs` knows what the file looks like. This knows what goes in it:
//! tuning, the usage guide's revision, the last seat, and the labels of the work
//! apps. These lived in the middle of `main.rs`, which is how the tuning keys
//! came to be derived from an enum's debug name without anyone seeing a contract
//! with every settings file already on disk.

use crate::focus;
use crate::settings::Settings;
use crate::usage_guide;
use roamling_core::{RuntimeTuning, RuntimeTuningKey, WorldPoint};

pub(crate) fn is_work_app(work_apps: &[String], application: &str) -> bool {
    work_apps
        .iter()
        .any(|wanted| wanted.eq_ignore_ascii_case(application))
}

pub(crate) fn persist_work_app_label(settings: &mut Settings, work_apps: &[String], exe: &str, label: &str) {
    if !is_work_app(work_apps, exe) || label.eq_ignore_ascii_case(exe) {
        return;
    }
    let cache_key = focus::cache_key(exe);
    settings.set(
        &format!("{}{cache_key}", crate::settings::WORK_APP_LABEL_PREFIX),
        label,
    );
}

/// The eleven tunable values, under one sub-key each.
///
/// `RuntimeTuningKey` declares the wire order and this walks it, so a key added
/// to the core is saved here without this function being touched.
///
/// The name is the core's `storage_name`, which the macOS blob also uses. It
/// used to be derived from the variant's debug name; that made a rename in the
/// core a silent reset of every value a user had tuned, so the core now owns
/// the stored names as a table with a test on it.
pub(crate) fn tuning_key(key: RuntimeTuningKey) -> String {
    format!("{}{}", crate::settings::TUNING_PREFIX, key.storage_name())
}

pub(crate) fn guide_seen(settings: &Settings) -> u32 {
    settings
        .text(usage_guide::SEEN_KEY)
        .and_then(|value| value.parse().ok())
        .unwrap_or(0)
}

pub(crate) fn remember_guide(settings: &mut Settings) {
    if let Some(revision) = usage_guide::take_acknowledged() {
        if std::env::var("ROAMLING_SMOKE_TEST").as_deref() != Ok("1") {
            settings.set(usage_guide::SEEN_KEY, guide_seen(settings).max(revision));
        }
    }
}

/// Saves only what the user moved away from the authored value.
///
/// Writing all eleven every time -- which this used to do -- freezes whichever
/// defaults happened to be current when the file was written. The wander pause
/// went from 12 to 40 and nobody who had ever opened the panel would have seen
/// it, because their file still said 12 and a stored value wins. Keys equal to
/// the default are removed instead, so the default gets to answer again, and
/// "Reset Defaults" leaves no tuning keys behind at all.
pub(crate) fn remember_tuning(settings: &mut Settings, tuning: RuntimeTuning) {
    let authored = RuntimeTuning::default();
    for key in roamling_core::TUNING_KEYS {
        let name = tuning_key(key);
        if tuning.get(key) == authored.get(key) {
            settings.clear(&name);
        } else {
            settings.set(&name, tuning.get(key));
        }
    }
}

/// Whatever was saved, re-clamped on the way in.
///
/// A missing value keeps the default rather than becoming zero: a settings file
/// written by an older build has fewer keys, and a walking speed of nothing is
/// a pet that never moves again.
pub(crate) fn stored_tuning(stored: &Settings) -> RuntimeTuning {
    let mut tuning = RuntimeTuning::default();
    for key in roamling_core::TUNING_KEYS {
        if let Some(value) = stored.number(&tuning_key(key)) {
            tuning = tuning.with(key, value);
        }
    }
    tuning
}

/// The pet came to rest somewhere worth keeping. Same three keys the macOS
/// runtime writes, so the two platforms describe a seat the same way.
pub(crate) fn remember(settings: &mut Settings, position: WorldPoint) {
    settings.set(crate::settings::POSITION_X, position.x);
    settings.set(crate::settings::POSITION_Y, position.y);
    settings.set(crate::settings::HAS_POSITION, true);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::WORK_APP_LABEL_PREFIX;

    /// A settings file written before the 2026-09-18 rename says
    /// `catchArmDistance`; it has to keep answering for the approach distance.
    #[test]
    fn renamed_tuning_keys_still_read_the_lines_older_files_wrote() {

        assert_eq!(
            tuning_key(RuntimeTuningKey::ApproachDistance),
            "roamling.runtimeTuning.catchArmDistance"
        );
        assert_eq!(
            tuning_key(RuntimeTuningKey::ApproachSpeed),
            "roamling.runtimeTuning.catchApproachSpeed"
        );
        assert_eq!(
            tuning_key(RuntimeTuningKey::ApproachHold),
            "roamling.runtimeTuning.catchWindow"
        );
        let mut stored = Settings::load_from(None);
        stored.set("roamling.runtimeTuning.catchWindow", 0.9);
        assert_eq!(stored_tuning(&stored).approach_hold, 0.9);
    }

    #[test]
    fn work_app_names_ignore_ascii_case() {
        let configured = vec!["HShow.exe".to_string(), "WINWORD.EXE".to_string()];
        assert!(is_work_app(&configured, "hshow.EXE"));
        assert!(is_work_app(&configured, "winword.exe"));
        assert!(!is_work_app(&configured, "Hwp.exe"));
    }

    #[test]
    fn only_resolved_labels_for_configured_work_apps_are_persisted() {
        let directory = std::env::temp_dir().join(format!(
            "roamling-label-policy-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("the test clock is before Unix time")
                .as_nanos()
        ));
        let path = directory.join("settings.txt");
        let configured = vec!["Hwp.exe".into(), "WINWORD.EXE".into()];
        let mut settings = Settings::load_from(Some(path.clone()));

        persist_work_app_label(
            &mut settings,
            &configured,
            "windowsterminal.exe",
            "Windows Terminal Host",
        );
        persist_work_app_label(&mut settings, &configured, "winword.exe", "WINWORD.EXE");
        persist_work_app_label(&mut settings, &configured, "HWP.EXE", "HWP 2024");
        drop(settings);

        let restarted = Settings::load_from(Some(path.clone()));
        assert_eq!(
            restarted.text(&format!("{WORK_APP_LABEL_PREFIX}hwp.exe")),
            Some("HWP 2024".into())
        );
        assert_eq!(
            restarted.text(&format!("{WORK_APP_LABEL_PREFIX}windowsterminal.exe")),
            None
        );
        assert_eq!(
            restarted.text(&format!("{WORK_APP_LABEL_PREFIX}winword.exe")),
            None
        );

        std::fs::remove_file(path).expect("the throwaway settings file was not removed");
        std::fs::remove_dir(directory).expect("the throwaway settings directory was not removed");
    }
}
