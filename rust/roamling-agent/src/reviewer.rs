// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

//! Who answers a Codex approval request: the user, or Codex's own reviewer.
//!
//! The hook does not say. Codex fires `PermissionRequest` *before* it routes
//! the request, and under auto-review almost none of them ever reach the user
//! -- so a pet that asks on every one of them spends the whole run asking for
//! an approval nobody is waiting on (openai/codex #23465 and #28833 ask for
//! the missing field; `docs/requests.md` B8 is what it looked like here).
//!
//! The answer is in the session's own record. Every turn opens with a
//! `turn_context` line, and that line carries `approvals_reviewer`.
//!
//! **This is the one place the crate opens a transcript, and it reads one
//! value from it.** Only lines that announce themselves as `turn_context` are
//! parsed at all; of those, the turn id and the reviewer are read and the rest
//! is dropped with the line. Nothing is stored, and no path outside Codex's
//! own home directory is opened -- the path arrives in a request, and a
//! request is not a reason to read an arbitrary file.
//!
//! What this cannot know: a reviewer that gives up hands the question to the
//! user, and nothing announces that either. A session under auto-review is
//! therefore never asked about, including the rare time it should be. That
//! trade was the user's to make and they made it.

use serde_json::Value;
use std::fs::File;
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// A session that has run for days. Past this the file is not read and the
/// request is treated as the user's, which is the safe way to be wrong.
const LARGEST_RECORD: u64 = 256 * 1024 * 1024;

/// The last answer, so the second approval request of a turn does not read the
/// file again. One slot: an agent asks in bursts within a turn.
static LAST: Mutex<Option<(PathBuf, String, bool)>> = Mutex::new(None);

/// Whether Codex will answer this request itself. `false` whenever that
/// cannot be established.
pub fn codex_answers_itself(payload: &Value) -> bool {
    let Some(path) = payload.get("transcript_path").and_then(Value::as_str) else {
        return false;
    };
    let Some(home) = codex_home() else { return false };
    let turn = payload.get("turn_id").and_then(Value::as_str);
    answers_itself(Path::new(path), turn, &home)
}

fn answers_itself(path: &Path, turn: Option<&str>, home: &Path) -> bool {
    let Some(path) = inside(path, home) else { return false };
    if let (Some(turn), Ok(last)) = (turn, LAST.lock()) {
        if let Some((known, known_turn, answer)) = last.as_ref() {
            if *known == path && known_turn == turn {
                return *answer;
            }
        }
    }
    let answer = reviewer(&path, turn).is_some_and(|reviewer| {
        matches!(reviewer.as_str(), "auto_review" | "guardian_subagent")
    });
    if let (Some(turn), Ok(mut last)) = (turn, LAST.lock()) {
        *last = Some((path, turn.to_owned(), answer));
    }
    answer
}

/// `CODEX_HOME` when set, as Codex itself resolves it, else `~/.codex`.
fn codex_home() -> Option<PathBuf> {
    if let Some(home) = std::env::var_os("CODEX_HOME").filter(|value| !value.is_empty()) {
        return Some(PathBuf::from(home));
    }
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(|home| PathBuf::from(home).join(".codex"))
}

/// The record, resolved, if it is a `.jsonl` under `home`. Resolving first is
/// what makes `..` and links answer for where they lead.
fn inside(path: &Path, home: &Path) -> Option<PathBuf> {
    if path.extension().and_then(|extension| extension.to_str()) != Some("jsonl") {
        return None;
    }
    let path = path.canonicalize().ok()?;
    let home = home.canonicalize().ok()?;
    path.starts_with(&home).then_some(path)
}

/// The reviewer of `turn`, or of the latest turn when the id is unknown or not
/// found -- a session does not change reviewers often, and the latest turn is
/// the one asking.
///
/// The turn that asks is the newest one, so its opening line is near the end
/// of a record that may be tens of megabytes of everything said before it.
/// The end is read first, in widening windows, and the whole file only if the
/// turn opened that long ago.
fn reviewer(path: &Path, turn: Option<&str>) -> Option<String> {
    let mut file = File::open(path).ok()?;
    let length = file.metadata().ok()?.len();
    if length > LARGEST_RECORD {
        return None;
    }
    for window in [1 << 20, 16 << 20, u64::MAX] {
        let start = length.saturating_sub(window);
        file.seek(SeekFrom::Start(start)).ok()?;
        let mut lines = BufReader::new(&file).split(b'\n');
        if start > 0 {
            // The window opens mid-line. That line belongs to the wider one.
            lines.next();
        }
        // A window that holds some other turn's opening has not answered for
        // this one. Only the whole file may fall back to the latest turn.
        match latest_reviewer(lines, turn) {
            (Some(exact), _) => return Some(exact),
            (None, latest) if start == 0 || turn.is_none() => {
                if latest.is_some() || start == 0 {
                    return latest;
                }
            }
            _ => {}
        }
    }
    None
}

/// The reviewer of the turn asked about, and of the last turn seen.
fn latest_reviewer(
    lines: impl Iterator<Item = std::io::Result<Vec<u8>>>,
    turn: Option<&str>,
) -> (Option<String>, Option<String>) {
    let mut latest = None;
    for line in lines {
        let Ok(line) = line else { break };
        // Everything that is not a turn opening is skipped unparsed. Messages
        // and tool output are most of the file and none of this module's
        // business.
        if !contains(&line, b"\"turn_context\"") {
            continue;
        }
        let Ok(record) = serde_json::from_slice::<Value>(&line) else { continue };
        if record.get("type").and_then(Value::as_str) != Some("turn_context") {
            continue;
        }
        let Some(context) = record.get("payload") else { continue };
        let Some(found) = context.get("approvals_reviewer").and_then(Value::as_str) else {
            continue;
        };
        if turn.is_some() && context.get("turn_id").and_then(Value::as_str) == turn {
            return (Some(found.to_owned()), latest);
        }
        latest = Some(found.to_owned());
    }
    (None, latest)
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.windows(needle.len()).any(|window| window == needle)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// A Codex home with one session record in it, removed when dropped.
    struct Home(PathBuf);

    impl Home {
        fn new(name: &str) -> Self {
            let root = std::env::temp_dir().join(format!(
                "roamling-reviewer-{name}-{}",
                std::process::id()
            ));
            let _ = std::fs::remove_dir_all(&root);
            std::fs::create_dir_all(root.join("sessions")).unwrap();
            Self(root)
        }

        fn record(&self, name: &str, lines: &[&str]) -> PathBuf {
            let path = self.0.join("sessions").join(name);
            let mut file = File::create(&path).unwrap();
            for line in lines {
                writeln!(file, "{line}").unwrap();
            }
            path
        }
    }

    impl Drop for Home {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn turn(id: &str, reviewer: &str) -> String {
        // The shape Codex writes, cut down to the fields around the two read.
        format!(
            r#"{{"timestamp":"2026-09-19T00:22:39.850Z","ordinal":7,"type":"turn_context","payload":{{"turn_id":"{id}","cwd":"C:\\repo","approval_policy":"on-request","approvals_reviewer":"{reviewer}"}}}}"#
        )
    }

    #[test]
    fn the_turn_that_asks_is_the_turn_that_answers() {
        let home = Home::new("turns");
        let chatter = r#"{"type":"response_item","payload":{"text":"mentions \"turn_context\" and approvals_reviewer auto_review in passing"}}"#;
        let path = home.record(
            "rollout.jsonl",
            &[&turn("t1", "user"), chatter, &turn("t2", "auto_review"), chatter],
        );
        assert!(!answers_itself(&path, Some("t1"), &home.0));
        assert!(answers_itself(&path, Some("t2"), &home.0));
        // An id the record does not have falls back to the latest turn.
        assert!(answers_itself(&path, Some("t9"), &home.0));
        assert!(answers_itself(&path, None, &home.0));

        let guardian = home.record("guardian.jsonl", &[&turn("t1", "guardian_subagent")]);
        assert!(answers_itself(&guardian, Some("t1"), &home.0));
    }

    /// The end of the record is read first. An older turn's answer must not
    /// be taken from a newer turn that happens to be in that window.
    #[test]
    fn a_turn_that_opened_megabytes_ago_is_still_found() {
        let home = Home::new("long");
        let filler = format!(r#"{{"type":"response_item","payload":{{"text":"{}"}}}}"#, "x".repeat(4000));
        let mut lines = vec![turn("old", "user")];
        lines.extend(std::iter::repeat(filler).take(600)); // about 2.4 MB
        lines.push(turn("new", "auto_review"));
        let lines: Vec<&str> = lines.iter().map(String::as_str).collect();
        let path = home.record("long.jsonl", &lines);
        assert!(std::fs::metadata(&path).unwrap().len() > 2 << 20);
        assert!(answers_itself(&path, Some("new"), &home.0));
        assert!(!answers_itself(&path, Some("old"), &home.0));
        assert!(answers_itself(&path, None, &home.0));
    }

    /// Wrong in the direction of asking: a pet that asks when nobody is
    /// waiting is a nuisance, one that stays quiet when somebody is, is broken.
    #[test]
    fn anything_that_cannot_be_read_is_the_users_to_answer() {
        let home = Home::new("unknown");
        assert!(!answers_itself(&home.0.join("sessions").join("missing.jsonl"), Some("t1"), &home.0));

        let empty = home.record("empty.jsonl", &[r#"{"type":"session_meta","payload":{}}"#]);
        assert!(!answers_itself(&empty, Some("t1"), &home.0));

        // Still being written: the last line is half a line.
        let torn = home.record("torn.jsonl", &[r#"{"type":"turn_context","payload":{"turn_id":"t1","approvals_rev"#]);
        assert!(!answers_itself(&torn, Some("t1"), &home.0));

        let payload: Value = serde_json::from_str(r#"{"session_id":"s","turn_id":"t1"}"#).unwrap();
        assert!(!codex_answers_itself(&payload));
    }

    /// The path comes from a request. It opens nothing outside Codex's home,
    /// whatever the file says.
    #[test]
    fn a_record_outside_codex_home_is_not_opened() {
        let home = Home::new("inside");
        let elsewhere = Home::new("elsewhere");
        let outside = elsewhere.record("rollout.jsonl", &[&turn("t1", "auto_review")]);
        assert!(!answers_itself(&outside, Some("t1"), &home.0));

        let climbing = home
            .0
            .join("sessions")
            .join("..")
            .join("..")
            .join(elsewhere.0.file_name().unwrap())
            .join("sessions")
            .join("rollout.jsonl");
        assert!(!answers_itself(&climbing, Some("t1"), &home.0));

        let not_a_record = home.record("notes.txt", &[&turn("t1", "auto_review")]);
        assert!(!answers_itself(&not_a_record, Some("t1"), &home.0));
    }
}
