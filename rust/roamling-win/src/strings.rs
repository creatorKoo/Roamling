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
fn parse(text: &str) -> HashMap<String, String> {
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
        table.insert(key.to_string(), unescape(value));
    }
    table
}

/// The escapes a `.strings` file may carry. `Bundle` resolves these before the
/// text ever reaches a label; a table read as raw bytes has to do it here, or
/// the About box shows a literal backslash-n where the paragraph break belongs.
///
/// The newline becomes CRLF because these strings end up in Win32 controls,
/// which do not break on a bare LF.
fn unescape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut characters = value.chars();
    while let Some(character) = characters.next() {
        if character != '\\' {
            out.push(character);
            continue;
        }
        match characters.next() {
            Some('n') => out.push_str("\r\n"),
            Some('t') => out.push('\t'),
            Some('"') => out.push('"'),
            Some('\\') => out.push('\\'),
            // Anything else was not an escape; keep both characters so the
            // text is never silently shortened.
            Some(other) => {
                out.push('\\');
                out.push(other);
            }
            None => out.push('\\'),
        }
    }
    out
}

fn table() -> &'static HashMap<String, String> {
    static TABLE: OnceLock<HashMap<String, String>> = OnceLock::new();
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
    table().get(key).map_or(key, |value| value.as_str())
}

/// `localizedFormat(_:_:)`, for the handful of strings that take values.
///
/// The tables are shared with Swift, so the placeholders are `NSString`'s:
/// `%@` for a value in order and `%1$d` when the two languages want the numbers
/// in a different order -- which is exactly why `menu.pet.coverage` is written
/// positionally. Anything not understood is copied through, so a placeholder we
/// do not handle shows up in the menu rather than eating the rest of the line.
pub fn localized_format(key: &'static str, arguments: &[&str]) -> String {
    fill(localized(key), arguments)
}

fn fill(template: &str, arguments: &[&str]) -> String {
    let mut out = String::with_capacity(template.len());
    let mut characters = template.chars().peekable();
    let mut next = 0usize;

    while let Some(character) = characters.next() {
        if character != '%' {
            out.push(character);
            continue;
        }
        // Everything between the `%` and the conversion, kept verbatim so an
        // unrecognised specifier can be put back exactly as it was written.
        let mut spec = String::new();
        while characters.peek().is_some_and(char::is_ascii_digit) {
            spec.push(characters.next().unwrap_or('0'));
        }
        // An explicit position, as in `%2$d`.
        let indexed = !spec.is_empty() && characters.peek() == Some(&'$');
        let position = if indexed {
            let digits = std::mem::take(&mut spec);
            spec.push_str(&digits);
            spec.push(characters.next().unwrap_or('$'));
            digits.parse::<usize>().unwrap_or(1)
        } else {
            0
        };
        // A precision, as in `%.2f`. Arguments arrive already rendered, so this
        // is recognised rather than applied -- the caller formats the number.
        if characters.peek() == Some(&'.') {
            spec.push(characters.next().unwrap_or('.'));
            while characters.peek().is_some_and(char::is_ascii_digit) {
                spec.push(characters.next().unwrap_or('0'));
            }
        }
        match characters.next() {
            Some('%') => out.push('%'),
            Some('@' | 'd' | 'f' | 's') => {
                let at = if indexed {
                    position.saturating_sub(1)
                } else {
                    let at = next;
                    next += 1;
                    at
                };
                out.push_str(arguments.get(at).copied().unwrap_or(""));
            }
            Some(other) => {
                out.push('%');
                out.push_str(&spec);
                out.push(other);
            }
            None => {
                out.push('%');
                out.push_str(&spec);
            }
        }
    }
    out
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
        let mut missing: Vec<&String> = en.keys().filter(|k| !ko.contains_key(*k)).collect();
        missing.sort_unstable();
        assert!(missing.is_empty(), "keys missing from ko.lproj: {missing:?}");
        let mut extra: Vec<&String> = ko.keys().filter(|k| !en.contains_key(*k)).collect();
        extra.sort_unstable();
        assert!(extra.is_empty(), "keys in ko.lproj with no English base: {extra:?}");
    }

    /// The menu this shell actually shows, so a rename on the Swift side is
    /// caught here rather than by a menu that reads "menu.quit".
    #[test]
    fn the_keys_the_tray_needs_exist() {
        let en = parse(EN);
        for key in [
            "menu.title",
            "menu.pet",
            "menu.pet.builtin",
            "menu.pet.coverage",
            "menu.size",
            "menu.roaming",
            "menu.avoidPointer",
            "menu.catchDrag",
            "menu.accessibility",
            "menu.visualPlacement",
            "menu.openPetFolder",
            "menu.copyDiagnostics",
            "menu.about",
            "menu.viewSource",
            "menu.quit",
            "alert.about.body",
            "about.version",
            "tuning.offDefault",
            "status.hooks.installed",
            "status.receiver.ready",
            "action.install",
            "action.testReaction",
            "alert.claude.install.body",
            "alert.codex.install.body",
            "result.claude.installed",
            "result.detail.codex.removed",
            "error.pet.load",
            "error.claude.settings",
            "error.codex.hooks",
        ] {
            assert!(en.contains_key(key), "{key} is gone from en.lproj");
        }
    }

    /// The About box is the one place a shared string carries paragraph
    /// breaks. A raw read would put a literal backslash-n in the dialog.
    #[test]
    fn escapes_are_resolved_rather_than_shown() {
        let en = parse(EN);
        let about = &en["alert.about.body"];
        assert!(about.contains("\r\n"), "no line break survived: {about}");
        assert!(
            !about.contains(r"\n"),
            "a literal escape is still in there: {about}"
        );
        assert_eq!(unescape(r#"a\"b\\c"#), "a\"b\\c");
        // Not an escape sequence, so both characters stay.
        assert_eq!(unescape(r"C:\Roamling"), r"C:\Roamling");
    }

    /// The two placeholder forms the shared tables actually use. `%@` runs in
    /// order; `menu.pet.coverage` is positional because the numbers read in a
    /// different order in Korean. Tested against literal templates rather than
    /// through the table, so the result does not depend on the machine's UI
    /// language.
    #[test]
    fn both_placeholder_forms_are_filled() {
        assert_eq!(fill("%@ (Built-in)", &["Mochi"]), "Mochi (Built-in)");
        assert_eq!(fill("Animations: %1$d of %2$d", &["14", "16"]), "Animations: 14 of 16");
        assert_eq!(fill("%2$@ then %1$@", &["a", "b"]), "b then a");
        // A literal percent, and a conversion nobody passed an argument for.
        assert_eq!(fill("100%% sure about %@", &[]), "100% sure about ");
        // A precision is recognised, not applied: the caller renders the
        // number, because arguments arrive as text.
        assert_eq!(fill("%.2f x", &["1.25"]), "1.25 x");
        // Anything unrecognised survives instead of eating the line.
        assert_eq!(fill("50%x off", &[]), "50%x off");
        assert_eq!(fill("%2$xy", &[]), "%2$xy");
    }

    /// Every placeholder in the shared tables has to be one this understands,
    /// in both languages -- a `%lu` or a `%.1f` would come out empty.
    #[test]
    fn the_tables_use_no_placeholder_this_cannot_fill() {
        for (language, table) in [("en", parse(EN)), ("ko", parse(KO))] {
            for (key, template) in &table {
                let mut characters = template.chars().peekable();
                while let Some(character) = characters.next() {
                    if character != '%' {
                        continue;
                    }
                    while characters.peek().is_some_and(char::is_ascii_digit) {
                        characters.next();
                    }
                    if characters.peek() == Some(&'$') {
                        characters.next();
                    }
                    if characters.peek() == Some(&'.') {
                        characters.next();
                        while characters.peek().is_some_and(char::is_ascii_digit) {
                            characters.next();
                        }
                    }
                    let conversion = characters.next();
                    assert!(
                        matches!(conversion, Some('@' | 'd' | 'f' | 's' | '%')),
                        "{language}.lproj {key} has %{conversion:?}, which localized_format drops"
                    );
                }
            }
        }
    }
}
