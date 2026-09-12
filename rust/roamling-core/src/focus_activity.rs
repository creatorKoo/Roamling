// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

//! The frontmost working app translated into state declarations.
//!
//! This module knows only the sampled app, whether it is watched, keyboard
//! recency, and time. Placement, rest, agents, and delivery all belong to the
//! director and runtime that consume its declarations.

use std::collections::VecDeque;

use crate::activity::ActivitySourceType;
use crate::source_state::{Milestone, SourceLevel, StateDeclaration};

/// A key remains evidence of typing for this long.
pub const TYPING_WINDOW: f64 = 10.0;
/// How long the paused question is shown before the seat is released.
pub const WAITING_BEFORE_RELEASE: f64 = 10.0;
/// A brief focus switch does not end the sitting.
pub const FOCUS_GRACE: f64 = 3.0;
/// Returning sooner than this continues the same sitting.
pub const BREAK: f64 = 120.0;
/// A sitting this long earns a goodbye wave when it ends.
pub const WAVE_AFTER_TYPING: f64 = 180.0;

const MAX_RECENT: usize = 6;
const SOURCE_PREFIX: &str = "focus:";

fn source_id(app: &str) -> String {
    format!("{SOURCE_PREFIX}{app}")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    Away,
    Present,
    Typing,
    Waiting,
    Released,
}

#[derive(Debug)]
pub struct FocusActivity {
    phase: Phase,
    /// The watched app whose sitting is still in progress. It remains set
    /// after release while that app stays in front.
    app: Option<String>,
    last_app: Option<String>,
    left_at: Option<f64>,
    away_since: Option<f64>,
    /// First sample of the current uninterrupted spell in front. A Cmd-Tab
    /// key is at or before this point, so it never counts as work in the app.
    in_front_since: Option<f64>,
    /// Whether this sitting has already declared its first-typing milestone.
    session_typed: bool,
    last_counted_key_at: Option<f64>,
    waiting_since: Option<f64>,
    typed_seconds: f64,
    last_observed_at: Option<f64>,
    /// `None` input means unknown, so it refreshes this declaration without a
    /// milestone instead of ending or pausing the sitting.
    last_declaration: Option<StateDeclaration>,
    recent: VecDeque<String>,
}

impl Default for FocusActivity {
    fn default() -> Self {
        Self {
            phase: Phase::Away,
            app: None,
            last_app: None,
            left_at: None,
            away_since: None,
            in_front_since: None,
            session_typed: false,
            last_counted_key_at: None,
            waiting_since: None,
            typed_seconds: 0.0,
            last_observed_at: None,
            last_declaration: None,
            recent: VecDeque::new(),
        }
    }
}

impl FocusActivity {
    pub fn new() -> Self {
        Self::default()
    }

    /// Translates one shell sample into declarations. Usually there is one;
    /// switching directly between watched apps yields the old app's `Away`
    /// followed by the new app's `Beside` in the same sample.
    pub fn observe(
        &mut self,
        app: Option<&str>,
        watched: bool,
        seconds_since_key: f64,
        now: f64,
    ) -> Vec<StateDeclaration> {
        let elapsed = self
            .last_observed_at
            .map_or(0.0, |last| (now - last).max(0.0));
        self.last_observed_at = Some(now);

        let on_seat = watched && app.is_some() && self.app.as_deref() == app;
        if !on_seat {
            self.in_front_since = None;
        }

        // The pet's own windows and an unidentifiable frontmost app are
        // unknown, not departures. Refresh the latest fact so expiry only
        // detects a shell that stopped sampling. Wall-clock phase timers keep
        // running and are evaluated when a named app returns.
        let Some(app) = app else {
            return self
                .last_declaration
                .clone()
                .map(|mut declaration| {
                    declaration.milestone = None;
                    vec![declaration]
                })
                .unwrap_or_default();
        };
        self.remember(app);

        if on_seat && self.phase == Phase::Typing {
            self.typed_seconds += elapsed;
        }

        if !watched {
            self.leave(now)
        } else if on_seat {
            self.away_since = None;
            let since = *self.in_front_since.get_or_insert(now);
            let typed = seconds_since_key < TYPING_WINDOW && now - seconds_since_key > since;
            if typed {
                self.last_counted_key_at = Some(now - seconds_since_key);
            }
            vec![self.sustain(typed, now)]
        } else {
            self.arrive(app, now)
        }
    }

    /// The apps the user has had in front lately, most recent first.
    pub fn recent_apps(&self) -> Vec<String> {
        self.recent.iter().cloned().collect()
    }

    /// Which watched app still owns the sitting, or `None`. Read by tests.
    pub fn seated_app(&self) -> Option<&str> {
        self.app.as_deref()
    }

    fn arrive(&mut self, app: &str, now: f64) -> Vec<StateDeclaration> {
        let mut declarations = Vec::new();

        // A direct app-to-app switch closes the old source before opening the
        // new one. Both facts belong to this one sample.
        if let Some(previous) = self.app.take() {
            let milestone = self.end_milestone();
            declarations.push(self.declaration(&previous, SourceLevel::Away, false, milestone));
        }

        let rested = self.left_at.is_none_or(|left| now - left >= BREAK);
        if rested || self.last_app.as_deref() != Some(app) {
            self.session_typed = false;
            self.typed_seconds = 0.0;
        }
        self.app = Some(app.to_owned());
        self.last_app = Some(app.to_owned());
        self.left_at = None;
        self.away_since = None;
        self.in_front_since = Some(now);
        self.last_counted_key_at = None;
        self.waiting_since = None;
        self.phase = Phase::Present;
        declarations.push(self.declaration(app, SourceLevel::Beside, true, None));
        declarations
    }

    fn sustain(&mut self, typed: bool, now: f64) -> StateDeclaration {
        let (level, milestone) = match self.phase {
            Phase::Present | Phase::Waiting | Phase::Released if typed => {
                let milestone = (!self.session_typed).then_some(Milestone::SittingStarted);
                self.session_typed = true;
                self.waiting_since = None;
                self.phase = Phase::Typing;
                (SourceLevel::Active, milestone)
            }
            Phase::Typing if self.keys_stopped(now) => {
                self.waiting_since = Some(now);
                self.phase = Phase::Waiting;
                (SourceLevel::Paused, None)
            }
            Phase::Waiting
                if self
                    .waiting_since
                    .is_some_and(|since| now - since >= WAITING_BEFORE_RELEASE) =>
            {
                self.waiting_since = None;
                self.phase = Phase::Released;
                (SourceLevel::Away, None)
            }
            Phase::Present => (SourceLevel::Beside, None),
            Phase::Typing => (SourceLevel::Active, None),
            Phase::Waiting => (SourceLevel::Paused, None),
            Phase::Released | Phase::Away => (SourceLevel::Away, None),
        };
        let app = self
            .app
            .clone()
            .expect("a sustained sitting always has an app");
        self.declaration(&app, level, true, milestone)
    }

    fn leave(&mut self, now: f64) -> Vec<StateDeclaration> {
        let Some(app) = self.app.clone() else {
            return Vec::new();
        };
        let since = *self.away_since.get_or_insert(now);
        if now - since < FOCUS_GRACE {
            let declaration = self.declaration(&app, self.current_level(), false, None);
            return vec![declaration];
        }

        let milestone = self.end_milestone();
        let declaration = self.declaration(&app, SourceLevel::Away, false, milestone);
        self.app = None;
        self.phase = Phase::Away;
        self.away_since = None;
        self.in_front_since = None;
        self.last_counted_key_at = None;
        self.waiting_since = None;
        // Record when focus actually left, not when the grace was observed.
        self.left_at = Some(now - FOCUS_GRACE);
        vec![declaration]
    }

    fn end_milestone(&mut self) -> Option<Milestone> {
        if self.typed_seconds < WAVE_AFTER_TYPING {
            return None;
        }
        self.typed_seconds = 0.0;
        Some(Milestone::SittingEnded)
    }

    fn current_level(&self) -> SourceLevel {
        match self.phase {
            Phase::Away | Phase::Released => SourceLevel::Away,
            Phase::Present => SourceLevel::Beside,
            Phase::Typing => SourceLevel::Active,
            Phase::Waiting => SourceLevel::Paused,
        }
    }

    fn keys_stopped(&self, now: f64) -> bool {
        self.last_counted_key_at
            .is_none_or(|at| now - at >= TYPING_WINDOW)
    }

    fn declaration(
        &mut self,
        app: &str,
        level: SourceLevel,
        focused: bool,
        milestone: Option<Milestone>,
    ) -> StateDeclaration {
        let declaration = StateDeclaration {
            source_id: source_id(app),
            source_type: ActivitySourceType::System,
            level,
            focused,
            hint: None,
            milestone,
        };
        let mut remembered = declaration.clone();
        remembered.milestone = None;
        self.last_declaration = Some(remembered);
        declaration
    }

    fn remember(&mut self, app: &str) {
        if self.recent.front().map(String::as_str) == Some(app) {
            return;
        }
        if let Some(index) = self.recent.iter().position(|seen| seen == app) {
            self.recent.remove(index);
        }
        self.recent.push_front(app.to_owned());
        while self.recent.len() > MAX_RECENT {
            self.recent.pop_back();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HWP: &str = "com.hancom.hwp";
    const SHOW: &str = "com.hancom.show";
    const QUIET: f64 = 10_000.0;

    fn one(
        source: &mut FocusActivity,
        app: Option<&str>,
        watched: bool,
        key: f64,
        now: f64,
    ) -> StateDeclaration {
        let declarations = source.observe(app, watched, key, now);
        assert_eq!(declarations.len(), 1);
        declarations.into_iter().next().unwrap()
    }

    #[test]
    fn a_watched_app_is_beside_until_typing_starts() {
        let mut source = FocusActivity::new();
        let arrived = one(&mut source, Some(HWP), true, QUIET, 100.0);
        assert_eq!(arrived.level, SourceLevel::Beside);
        assert_eq!(arrived.milestone, None);
        assert!(arrived.focused);

        let present = one(&mut source, Some(HWP), true, QUIET, 100.5);
        assert_eq!(present.level, SourceLevel::Beside);
        assert_eq!(present.milestone, None);
    }

    #[test]
    fn first_typing_marks_the_sitting_then_pauses_and_releases() {
        let mut source = FocusActivity::new();
        one(&mut source, Some(HWP), true, QUIET, 100.0);
        let started = one(&mut source, Some(HWP), true, 0.2, 100.5);
        assert_eq!(started.level, SourceLevel::Active);
        assert_eq!(started.milestone, Some(Milestone::SittingStarted));

        let active = one(&mut source, Some(HWP), true, QUIET, 110.2);
        assert_eq!(active.level, SourceLevel::Active);
        assert_eq!(active.milestone, None);
        let paused = one(&mut source, Some(HWP), true, QUIET, 110.3);
        assert_eq!(paused.level, SourceLevel::Paused);
        let released = one(&mut source, Some(HWP), true, QUIET, 120.3);
        assert_eq!(released.level, SourceLevel::Away);
        assert_eq!(source.seated_app(), Some(HWP));
    }

    #[test]
    fn typing_after_release_is_active_without_a_second_start() {
        let mut source = FocusActivity::new();
        one(&mut source, Some(HWP), true, QUIET, 0.0);
        one(&mut source, Some(HWP), true, 0.1, 0.5);
        one(&mut source, Some(HWP), true, QUIET, 10.4);
        one(&mut source, Some(HWP), true, QUIET, 10.5);
        one(&mut source, Some(HWP), true, QUIET, 20.5);

        let active = one(&mut source, Some(HWP), true, 0.1, 21.0);
        assert_eq!(active.level, SourceLevel::Active);
        assert_eq!(active.milestone, None);
    }

    #[test]
    fn focus_grace_preserves_the_sitting_and_ignores_cmd_tab() {
        let mut source = FocusActivity::new();
        one(&mut source, Some(HWP), true, QUIET, 100.0);
        let first = one(&mut source, Some(HWP), false, 0.0, 101.0);
        assert_eq!(first.level, SourceLevel::Beside);
        assert!(!first.focused);

        let returned = one(&mut source, Some(HWP), true, 0.0, 102.0);
        assert_eq!(returned.level, SourceLevel::Beside);
        assert_eq!(returned.milestone, None);
        let typed = one(&mut source, Some(HWP), true, 0.1, 102.5);
        assert_eq!(typed.milestone, Some(Milestone::SittingStarted));
    }

    #[test]
    fn short_break_keeps_the_session_but_long_break_starts_another() {
        let mut source = FocusActivity::new();
        one(&mut source, Some(HWP), true, QUIET, 0.0);
        assert_eq!(
            one(&mut source, Some(HWP), true, 0.1, 0.5).milestone,
            Some(Milestone::SittingStarted)
        );
        one(&mut source, Some(HWP), false, QUIET, 1.0);
        one(&mut source, Some(HWP), false, QUIET, 4.0);
        one(&mut source, Some(HWP), true, QUIET, 100.0);
        assert_eq!(
            one(&mut source, Some(HWP), true, 0.1, 100.5).milestone,
            None
        );

        one(&mut source, Some(HWP), false, QUIET, 101.0);
        one(&mut source, Some(HWP), false, QUIET, 104.0);
        one(&mut source, Some(HWP), true, QUIET, 224.0);
        assert_eq!(
            one(&mut source, Some(HWP), true, 0.1, 224.5).milestone,
            Some(Milestone::SittingStarted)
        );
    }

    #[test]
    fn direct_switch_declares_old_away_before_new_beside() {
        let mut source = FocusActivity::new();
        one(&mut source, Some(HWP), true, QUIET, 0.0);
        let declarations = source.observe(Some(SHOW), true, QUIET, 1.0);
        assert_eq!(declarations.len(), 2);
        assert_eq!(declarations[0].source_id, source_id(HWP));
        assert_eq!(declarations[0].level, SourceLevel::Away);
        assert_eq!(declarations[1].source_id, source_id(SHOW));
        assert_eq!(declarations[1].level, SourceLevel::Beside);
    }

    #[test]
    fn nil_refreshes_the_previous_level_while_timers_keep_running() {
        let mut source = FocusActivity::new();
        one(&mut source, Some(HWP), true, QUIET, 100.0);
        one(&mut source, Some(HWP), true, 0.1, 100.5);

        let unknown = one(&mut source, None, false, QUIET, 111.0);
        assert_eq!(unknown.level, SourceLevel::Active);
        assert_eq!(unknown.milestone, None);
        let paused = one(&mut source, Some(HWP), true, QUIET, 111.5);
        assert_eq!(paused.level, SourceLevel::Paused);

        let unknown = one(&mut source, None, false, QUIET, 122.0);
        assert_eq!(unknown.level, SourceLevel::Paused);
        let released = one(&mut source, Some(HWP), true, QUIET, 122.5);
        assert_eq!(released.level, SourceLevel::Away);
    }

    #[test]
    fn three_minutes_of_typing_marks_the_end_once() {
        let mut source = FocusActivity::new();
        one(&mut source, Some(HWP), true, QUIET, 0.0);
        one(&mut source, Some(HWP), true, 0.1, 0.5);
        for second in 1..=180 {
            one(&mut source, Some(HWP), true, 0.1, 0.5 + second as f64);
        }
        one(&mut source, Some(HWP), false, QUIET, 181.0);
        let ended = one(&mut source, Some(HWP), false, QUIET, 184.0);
        assert_eq!(ended.level, SourceLevel::Away);
        assert_eq!(ended.milestone, Some(Milestone::SittingEnded));
    }

    #[test]
    fn an_unwatched_app_only_updates_the_recent_list() {
        let mut source = FocusActivity::new();
        assert!(source.observe(Some(HWP), false, QUIET, 0.0).is_empty());
        assert_eq!(source.recent_apps(), vec![HWP]);
    }

    #[test]
    fn recent_apps_are_unique_most_recent_and_bounded() {
        let mut source = FocusActivity::new();
        for (index, app) in ["a", "b", "c", "d", "e", "f", "g"].into_iter().enumerate() {
            source.observe(Some(app), false, QUIET, index as f64);
        }
        source.observe(Some("d"), false, QUIET, 8.0);
        assert_eq!(source.recent_apps(), vec!["d", "g", "f", "e", "c", "b"]);
    }
}
