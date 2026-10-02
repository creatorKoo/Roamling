// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

//! A short history of what changed, so the pet can be asked why it is doing
//! something.
//!
//! Ported from `Sources/RoamlingCore/DiagnosticsLog.swift`. It sits in the
//! shell rather than the core for the same reason it does on macOS: the core
//! hands out `TickOutput.diagnostics` on every tick and keeps nothing, and
//! whoever wants a history keeps one.
//!
//! **Only transitions survive.** A sampled trace of a pet standing still is a
//! wall of identical lines that says nothing, and the one time it mattered the
//! answer was in a gap between lines rather than in the lines. Callers record
//! every tick; this keeps the changes.
//!
//! It holds coordinates, scores, state names and timings. Nothing derived from
//! the screen beyond an emptiness score belongs here, and neither does anything
//! naming what the user is working on -- the capture path never keeps an image,
//! and a diagnostics buffer must not undo that from behind.

use std::collections::HashMap;

const CAPACITY: usize = 2_000;

/// How far back the copied text reaches. On a quiet day `CAPACITY` holds two
/// days of naps, and someone asking why the pet just did something pasted all
/// of it (`docs/requests.md` R28). The buffer itself keeps what it kept.
const WINDOW: f64 = 30.0 * 60.0;

struct Entry {
    timestamp: f64,
    category: String,
    message: String,
}

pub struct DiagnosticsLog {
    entries: Vec<Entry>,
    /// The last message seen per category, which is what turns a per-tick
    /// caller into a transition log rather than a sampled one.
    latest: HashMap<String, String>,
}

impl DiagnosticsLog {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            latest: HashMap::new(),
        }
    }

    /// Records only when `message` differs from this category's previous one.
    pub fn record(&mut self, category: &str, message: &str, timestamp: f64) {
        if self.latest.get(category).is_some_and(|seen| seen == message) {
            return;
        }
        self.latest.insert(category.to_string(), message.to_string());
        self.entries.push(Entry {
            timestamp,
            category: category.to_string(),
            message: message.to_string(),
        });
        if self.entries.len() > CAPACITY {
            let excess = self.entries.len() - CAPACITY;
            self.entries.drain(..excess);
        }
    }

    /// The last `WINDOW` seconds, under what each category last said before
    /// them -- without those lines a pet that has slept for hours reads as an
    /// empty log. Elapsed seconds count from the start of the window, so the
    /// lines carried in from before it are negative. A log younger than the
    /// window counts from its first entry, as it always has: absolute uptime is
    /// a large number that says nothing, and the question is always how long
    /// something lasted.
    pub fn text(&self, now: f64) -> String {
        let cutoff = now - WINDOW;
        let mut carried: Vec<&Entry> = Vec::new();
        for entry in self.entries.iter().rev().filter(|entry| entry.timestamp < cutoff) {
            if !carried.iter().any(|kept| kept.category == entry.category) {
                carried.push(entry);
            }
        }
        carried.reverse();
        let recent = self.entries.iter().filter(|entry| entry.timestamp >= cutoff);
        let shown: Vec<&Entry> = carried.iter().copied().chain(recent).collect();
        let Some(first) = shown.first() else {
            return "(no entries)".to_string();
        };
        let origin = if carried.is_empty() { first.timestamp } else { cutoff };
        let mut lines: Vec<String> = shown
            .iter()
            .map(|entry| {
                format!(
                    "{:8.1}  {:<9} {}",
                    entry.timestamp - origin,
                    entry.category,
                    entry.message
                )
            })
            .collect();
        if let Some(last) = shown.last() {
            if now > last.timestamp {
                lines.push(format!("{:8.1}  {:<9} -", now - origin, "now"));
            }
        }
        lines.join("\r\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The whole point of the type: a caller that records every tick must not
    /// produce one line per tick.
    #[test]
    fn only_changes_are_kept() {
        let mut log = DiagnosticsLog::new();
        for tick in 0..10 {
            log.record("state", "Sleep", tick as f64);
        }
        log.record("state", "Wake", 10.0);
        assert_eq!(log.entries.len(), 2);
        assert_eq!(log.entries[0].timestamp, 0.0);
        assert_eq!(log.entries[1].message, "Wake");
    }

    /// Two categories do not shadow each other, or a busy one would hide a
    /// quiet one's transitions.
    #[test]
    fn categories_are_tracked_apart() {
        let mut log = DiagnosticsLog::new();
        log.record("state", "Sit", 0.0);
        log.record("seat", "score 0.4", 0.0);
        log.record("state", "Sit", 1.0);
        log.record("seat", "score 0.9", 1.0);
        assert_eq!(log.entries.len(), 3);
    }

    /// Times are relative to the first entry, and the trailing "now" line marks
    /// how long the last state has held.
    #[test]
    fn the_text_is_relative_and_ends_at_now() {
        let mut log = DiagnosticsLog::new();
        assert_eq!(log.text(0.0), "(no entries)");
        log.record("state", "Sit", 100.0);
        log.record("state", "Sleep", 103.5);
        let text = log.text(110.0);
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 3);
        assert!(lines[0].starts_with("     0.0  state"), "{}", lines[0]);
        assert!(lines[1].starts_with("     3.5  state"), "{}", lines[1]);
        assert!(lines[2].starts_with("    10.0  now"), "{}", lines[2]);
    }

    /// The buffer is bounded, and it is the oldest lines that go.
    #[test]
    fn the_oldest_lines_are_dropped_first() {
        let mut log = DiagnosticsLog::new();
        for tick in 0..(CAPACITY + 50) {
            log.record("state", &format!("step {tick}"), tick as f64);
        }
        assert_eq!(log.entries.len(), CAPACITY);
        assert_eq!(log.entries[0].message, "step 50");
    }

    /// R28. Only the last half hour is handed out, under each category's last
    /// line from before it; the transitions in between are left out.
    #[test]
    fn the_text_is_the_last_half_hour() {
        let mut log = DiagnosticsLog::new();
        log.record("place", "sleep in place", 100.0);
        log.record("pet", "wander", 100.0);
        log.record("pet", "sleep", 300.0);
        log.record("pet", "wake", 5000.0);
        log.record("pet", "idle", 5001.0);
        let text = log.text(5100.0);
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(
            lines,
            [
                " -3200.0  place     sleep in place",
                " -3000.0  pet       sleep",
                "  1700.0  pet       wake",
                "  1701.0  pet       idle",
                "  1800.0  now       -",
            ]
        );
    }

    /// Hours asleep is still a pet asleep, not an empty log.
    #[test]
    fn a_long_sleep_still_says_so() {
        let mut log = DiagnosticsLog::new();
        log.record("pet", "sleep", 100.0);
        let text = log.text(100_000.0);
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines, ["-98100.0  pet       sleep", "  1800.0  now       -"]);
    }
}
