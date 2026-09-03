// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

//! User-visible copy, read from the same files the macOS shell reads.
//!
//! `CLAUDE.md` puts every menu and alert string in
//! `Sources/RoamlingShell/Resources/{en,ko}.lproj/Localizable.strings` and says
//! not to branch on language in code. macOS gets that for free from `Bundle`;
//! Windows has no equivalent, so the two files are compiled in and the same
//! rule is kept by hand: pick the table once, then only look keys up.
//!
//! English is the base. Anything that is not Korean lands there, which is what
//! the bundle would have done.

use std::collections::HashMap;
use std::sync::OnceLock;
use windows::Win32::Globalization::GetUserDefaultUILanguage;

const EN: &str =
    include_str!("../../../Sources/RoamlingShell/Resources/en.lproj/Localizable.strings");
const KO: &str =
    include_str!("../../../Sources/RoamlingShell/Resources/ko.lproj/Localizable.strings");

/// `"key" = "value";`, skipping comments and blank lines. Deliberately not a
/// full plist parser: these files are one flat form and have to stay that way
/// for the Swift side too.
fn parse(text: &str) -> HashMap<&str, &str> {
    let mut table = HashMap::new();
    for line in text.lines() {
        let line = line.trim();
        if !line.starts_with('"') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim().trim_matches('"');
        let value = value.trim().trim_end_matches(';').trim().trim_matches('"');
        table.insert(key, value);
    }
    table
}

fn table() -> &'static HashMap<&'static str, &'static str> {
    static TABLE: OnceLock<HashMap<&'static str, &'static str>> = OnceLock::new();
    TABLE.get_or_init(|| {
        // 0x12 is LANG_KOREAN. Only the primary language matters; the sublang
        // says which region, and there is one Korean table.
        let korean = (unsafe { GetUserDefaultUILanguage() } & 0x3ff) == 0x12;
        let mut chosen = parse(EN);
        if korean {
            // Korean over English, so a key missing from ko still resolves
            // rather than disappearing -- the same fallback the bundle does.
            chosen.extend(parse(KO));
        }
        chosen
    })
}

/// The key itself when nothing has that key, which is what `NSLocalizedString`
/// does and is loud enough to notice in a menu.
pub fn localized(key: &'static str) -> &'static str {
    table().get(key).copied().unwrap_or(key)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The two files have to carry the same keys. English is the base, so a key
    /// only in Korean is a typo, and one only in English shows up untranslated.
    #[test]
    fn the_two_tables_have_the_same_keys() {
        let en = parse(EN);
        let ko = parse(KO);
        let mut missing: Vec<&str> = en.keys().filter(|k| !ko.contains_key(*k)).copied().collect();
        missing.sort_unstable();
        assert!(missing.is_empty(), "keys missing from ko.lproj: {missing:?}");
        let mut extra: Vec<&str> = ko.keys().filter(|k| !en.contains_key(*k)).copied().collect();
        extra.sort_unstable();
        assert!(extra.is_empty(), "keys in ko.lproj with no English base: {extra:?}");
    }

    /// The menu this shell actually shows, so a rename on the Swift side is
    /// caught here rather than by a menu that reads "menu.quit".
    #[test]
    fn the_keys_the_tray_needs_exist() {
        let en = parse(EN);
        for key in ["menu.roaming", "menu.avoidPointer", "menu.catchDrag", "menu.quit"] {
            assert!(en.contains_key(key), "{key} is gone from en.lproj");
        }
    }
}
