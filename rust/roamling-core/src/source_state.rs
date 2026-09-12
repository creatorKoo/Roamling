// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

//! State-shaped activity sources, before placement or pet behavior is applied.

use std::cmp::Ordering;
use std::collections::BTreeMap;

use crate::activity::{ActivitySourceType, CompanionReaction};
use crate::world::LocationHint;

/// Four shell samples at the planned 0.5-second cadence: long enough to bridge
/// one missed sample while still clearing a source promptly when updates stop.
pub const STATE_EXPIRY: f64 = 2.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceLevel {
    Away,
    Beside,
    Active,
    Paused,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Milestone {
    SittingStarted,
    SittingEnded,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StateDeclaration {
    pub source_id: String,
    pub source_type: ActivitySourceType,
    pub level: SourceLevel,
    pub focused: bool,
    pub hint: Option<LocationHint>,
    pub milestone: Option<Milestone>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Transition {
    pub source_id: String,
    pub source_type: ActivitySourceType,
    pub from: SourceLevel,
    pub to: SourceLevel,
    pub milestone: Option<Milestone>,
    pub hint: Option<LocationHint>,
    pub focused: bool,
}

#[derive(Debug, Clone)]
struct StoredDeclaration {
    declaration: StateDeclaration,
    refreshed_at: f64,
    transitioned_at: f64,
}

#[derive(Debug, Default)]
pub struct SourceStates {
    declarations: BTreeMap<String, StoredDeclaration>,
}

impl SourceStates {
    pub fn new() -> Self {
        Self::default()
    }

    /// Records the latest declaration. Focus, hints and milestones are current
    /// declaration metadata; a level change or a newly supplied milestone is
    /// a state transition.
    pub fn declare(&mut self, declaration: StateDeclaration, now: f64) -> Option<Transition> {
        let source_id = declaration.source_id.clone();
        let previous = self.declarations.get(&source_id);
        let from = previous.map_or(SourceLevel::Away, |stored| stored.declaration.level);
        let milestone_changed = declaration.milestone.is_some()
            && previous.is_none_or(|stored| stored.declaration.milestone != declaration.milestone);
        let changed = from != declaration.level || milestone_changed;
        let transitioned_at = if changed {
            now
        } else {
            self.declarations
                .get(&source_id)
                .map_or(now, |stored| stored.transitioned_at)
        };

        let transition = changed.then(|| transition(from, &declaration));
        self.declarations.insert(
            source_id,
            StoredDeclaration {
                declaration,
                refreshed_at: now,
                transitioned_at,
            },
        );
        transition
    }

    /// Turns declarations whose refresh stopped into `Away` transitions.
    pub fn expire(&mut self, now: f64) -> Vec<Transition> {
        let mut transitions = Vec::new();
        for stored in self.declarations.values_mut() {
            if stored.declaration.level == SourceLevel::Away
                || now - stored.refreshed_at < STATE_EXPIRY
            {
                continue;
            }

            let from = stored.declaration.level;
            stored.declaration.level = SourceLevel::Away;
            stored.declaration.milestone = None;
            stored.transitioned_at = now;
            transitions.push(transition(from, &stored.declaration));
        }
        transitions
    }

    /// Returns the live declaration that wins the state-shaped layer.
    pub fn winner(&self, now: f64) -> Option<&StateDeclaration> {
        self.declarations
            .iter()
            .filter(|(_, stored)| {
                stored.declaration.level != SourceLevel::Away
                    && now - stored.refreshed_at < STATE_EXPIRY
            })
            .max_by(|(left_id, left), (right_id, right)| {
                compare_winner(left, right).then_with(|| left_id.cmp(right_id))
            })
            .map(|(_, stored)| &stored.declaration)
    }
}

fn transition(from: SourceLevel, declaration: &StateDeclaration) -> Transition {
    Transition {
        source_id: declaration.source_id.clone(),
        source_type: declaration.source_type,
        from,
        to: declaration.level,
        milestone: declaration.milestone,
        hint: declaration.hint.clone(),
        focused: declaration.focused,
    }
}

fn compare_winner(left: &StoredDeclaration, right: &StoredDeclaration) -> Ordering {
    left.declaration
        .focused
        .cmp(&right.declaration.focused)
        .then_with(|| level_rank(left.declaration.level).cmp(&level_rank(right.declaration.level)))
        .then_with(|| left.transitioned_at.total_cmp(&right.transitioned_at))
}

fn level_rank(level: SourceLevel) -> u8 {
    match level {
        SourceLevel::Away => 0,
        SourceLevel::Beside => 1,
        SourceLevel::Paused => 2,
        SourceLevel::Active => 3,
    }
}

/// Maps a state transition to its picture without deciding where the pet sits,
/// how it travels, or whether it is resting.
pub fn reaction_for(
    source_type: ActivitySourceType,
    transition: &Transition,
) -> Option<CompanionReaction> {
    match source_type {
        ActivitySourceType::System => match transition.milestone {
            Some(Milestone::SittingStarted) => Some(CompanionReaction::Spark),
            Some(Milestone::SittingEnded) => Some(CompanionReaction::SmallCelebrate),
            None => match transition.to {
                SourceLevel::Active => Some(CompanionReaction::Work),
                SourceLevel::Paused => Some(CompanionReaction::Paw),
                SourceLevel::Away | SourceLevel::Beside => None,
            },
        },
        // Media and game reactions are deliberately left for the phase that
        // introduces those sources. Agent and custom are not state-shaped.
        ActivitySourceType::Media
        | ActivitySourceType::Game
        | ActivitySourceType::Agent
        | ActivitySourceType::Custom => None,
    }
}

/// The picture worn for as long as a state-shaped source keeps its level.
pub fn sustained_reaction(
    source_type: ActivitySourceType,
    level: SourceLevel,
) -> Option<CompanionReaction> {
    match source_type {
        ActivitySourceType::System => match level {
            SourceLevel::Active => Some(CompanionReaction::Work),
            SourceLevel::Paused => Some(CompanionReaction::Paw),
            SourceLevel::Away | SourceLevel::Beside => None,
        },
        ActivitySourceType::Media
        | ActivitySourceType::Game
        | ActivitySourceType::Agent
        | ActivitySourceType::Custom => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn declaration(
        source_id: &str,
        source_type: ActivitySourceType,
        level: SourceLevel,
        focused: bool,
    ) -> StateDeclaration {
        StateDeclaration {
            source_id: source_id.into(),
            source_type,
            level,
            focused,
            hint: None,
            milestone: None,
        }
    }

    fn system(source_id: &str, level: SourceLevel, focused: bool) -> StateDeclaration {
        declaration(source_id, ActivitySourceType::System, level, focused)
    }

    fn sample_transition(
        source_type: ActivitySourceType,
        to: SourceLevel,
        milestone: Option<Milestone>,
    ) -> Transition {
        Transition {
            source_id: "source".into(),
            source_type,
            from: SourceLevel::Beside,
            to,
            milestone,
            hint: None,
            focused: false,
        }
    }

    #[test]
    fn redeclaring_the_same_state_only_refreshes_it() {
        let mut states = SourceStates::new();
        assert!(states
            .declare(system("work", SourceLevel::Beside, false), 0.0)
            .is_some());
        assert_eq!(
            states.declare(system("work", SourceLevel::Beside, false), 1.5),
            None
        );

        assert!(states.expire(3.4).is_empty());
        assert_eq!(states.expire(3.5).len(), 1);
    }

    #[test]
    fn a_level_change_reports_one_transition() {
        let mut states = SourceStates::new();
        states.declare(system("work", SourceLevel::Beside, false), 0.0);

        let changed = states
            .declare(system("work", SourceLevel::Active, true), 0.5)
            .expect("level change should produce a transition");
        assert_eq!(changed.source_id, "work");
        assert_eq!(changed.source_type, ActivitySourceType::System);
        assert_eq!(changed.from, SourceLevel::Beside);
        assert_eq!(changed.to, SourceLevel::Active);
        assert!(changed.focused);
    }

    #[test]
    fn a_new_milestone_reports_a_transition_without_a_level_change() {
        let mut states = SourceStates::new();
        states.declare(system("work", SourceLevel::Active, true), 0.0);

        let mut milestone = system("work", SourceLevel::Active, true);
        milestone.milestone = Some(Milestone::SittingStarted);
        let changed = states
            .declare(milestone.clone(), 0.5)
            .expect("a newly carried milestone should not disappear");
        assert_eq!(changed.from, SourceLevel::Active);
        assert_eq!(changed.to, SourceLevel::Active);
        assert_eq!(changed.milestone, Some(Milestone::SittingStarted));
        assert_eq!(states.declare(milestone, 1.0), None);
    }

    #[test]
    fn a_declaration_expires_after_two_seconds_without_a_refresh() {
        let mut states = SourceStates::new();
        states.declare(system("work", SourceLevel::Active, true), 10.0);

        assert!(states.expire(11.9).is_empty());
        let expired = states.expire(12.0);
        assert_eq!(expired.len(), 1);
        assert_eq!(expired[0].from, SourceLevel::Active);
        assert_eq!(expired[0].to, SourceLevel::Away);
        assert_eq!(expired[0].milestone, None);
    }

    #[test]
    fn declaring_again_after_expiry_returns_from_away() {
        let mut states = SourceStates::new();
        states.declare(system("work", SourceLevel::Beside, false), 0.0);
        states.expire(2.0);

        let returned = states
            .declare(system("work", SourceLevel::Beside, true), 2.1)
            .expect("a source returning after expiry should transition");
        assert_eq!(returned.from, SourceLevel::Away);
        assert_eq!(returned.to, SourceLevel::Beside);
    }

    #[test]
    fn a_focused_source_wins_and_focus_can_move() {
        let mut states = SourceStates::new();
        states.declare(system("document", SourceLevel::Active, false), 0.0);
        states.declare(
            declaration(
                "video",
                ActivitySourceType::Media,
                SourceLevel::Active,
                true,
            ),
            0.1,
        );
        assert_eq!(states.winner(0.2).unwrap().source_id, "video");

        assert_eq!(
            states.declare(system("document", SourceLevel::Active, true), 0.3),
            None
        );
        assert_eq!(
            states.declare(
                declaration(
                    "video",
                    ActivitySourceType::Media,
                    SourceLevel::Active,
                    false
                ),
                0.4,
            ),
            None
        );
        assert_eq!(states.winner(0.5).unwrap().source_id, "document");
    }

    #[test]
    fn level_decides_when_no_source_is_focused() {
        let mut states = SourceStates::new();
        states.declare(system("beside", SourceLevel::Beside, false), 0.0);
        states.declare(system("paused", SourceLevel::Paused, false), 0.1);
        states.declare(system("active", SourceLevel::Active, false), 0.2);
        assert_eq!(states.winner(0.3).unwrap().source_id, "active");

        states.declare(system("active", SourceLevel::Beside, false), 0.4);
        assert_eq!(states.winner(0.5).unwrap().source_id, "paused");
    }

    #[test]
    fn the_most_recent_transition_breaks_an_equal_rank() {
        let mut states = SourceStates::new();
        states.declare(system("older", SourceLevel::Beside, false), 0.0);
        states.declare(system("newer", SourceLevel::Beside, false), 0.5);

        assert_eq!(states.winner(0.6).unwrap().source_id, "newer");
    }

    #[test]
    fn away_never_wins() {
        let mut states = SourceStates::new();
        assert_eq!(
            states.declare(system("one", SourceLevel::Away, true), 0.0),
            None
        );
        assert_eq!(
            states.declare(system("two", SourceLevel::Away, false), 0.1),
            None
        );
        assert!(states.winner(0.2).is_none());
    }

    #[test]
    fn system_milestones_override_level_reactions() {
        let started = sample_transition(
            ActivitySourceType::System,
            SourceLevel::Active,
            Some(Milestone::SittingStarted),
        );
        assert_eq!(
            reaction_for(ActivitySourceType::System, &started),
            Some(CompanionReaction::Spark)
        );

        let active = sample_transition(ActivitySourceType::System, SourceLevel::Active, None);
        assert_eq!(
            reaction_for(ActivitySourceType::System, &active),
            Some(CompanionReaction::Work)
        );

        let ended = sample_transition(
            ActivitySourceType::System,
            SourceLevel::Away,
            Some(Milestone::SittingEnded),
        );
        assert_eq!(
            reaction_for(ActivitySourceType::System, &ended),
            Some(CompanionReaction::SmallCelebrate)
        );

        let away = sample_transition(ActivitySourceType::System, SourceLevel::Away, None);
        assert_eq!(reaction_for(ActivitySourceType::System, &away), None);
    }

    #[test]
    fn media_and_game_have_no_reactions_yet() {
        for source_type in [ActivitySourceType::Media, ActivitySourceType::Game] {
            for to in [
                SourceLevel::Away,
                SourceLevel::Beside,
                SourceLevel::Active,
                SourceLevel::Paused,
            ] {
                for milestone in [
                    None,
                    Some(Milestone::SittingStarted),
                    Some(Milestone::SittingEnded),
                ] {
                    let transition = sample_transition(source_type, to, milestone);
                    assert_eq!(reaction_for(source_type, &transition), None);
                }
            }
        }
    }

    #[test]
    fn only_working_system_levels_have_sustained_reactions() {
        assert_eq!(
            sustained_reaction(ActivitySourceType::System, SourceLevel::Active),
            Some(CompanionReaction::Work)
        );
        assert_eq!(
            sustained_reaction(ActivitySourceType::System, SourceLevel::Paused),
            Some(CompanionReaction::Paw)
        );
        assert_eq!(
            sustained_reaction(ActivitySourceType::System, SourceLevel::Beside),
            None
        );
        assert_eq!(
            sustained_reaction(ActivitySourceType::System, SourceLevel::Away),
            None
        );

        for source_type in [ActivitySourceType::Media, ActivitySourceType::Game] {
            assert_eq!(sustained_reaction(source_type, SourceLevel::Active), None);
        }
    }
}
