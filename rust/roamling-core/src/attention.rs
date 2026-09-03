// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

//! Ported from `Sources/RoamlingCore/AttentionModel.swift` and
//! `ReactionPolicy.swift`. Both carry state between calls, so they stay objects
//! rather than becoming functions -- the Swift side holds one of each.

use std::collections::HashMap;

use crate::activity::{
    CompanionEvent, CompanionEventKind, CompanionReaction, ReactingBehavior, UserContext,
};
use crate::geometry::{clamped, swift_max};
use crate::world::last_maximum;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AttentionConfiguration {
    pub minimum_dwell_time: f64,
    pub hysteresis_margin: f64,
    pub revisit_cooldown: f64,
    pub maximum_event_age: f64,
}

impl AttentionConfiguration {
    pub fn new(
        minimum_dwell_time: f64,
        hysteresis_margin: f64,
        revisit_cooldown: f64,
        maximum_event_age: f64,
    ) -> Self {
        Self {
            minimum_dwell_time: swift_max(0.0, minimum_dwell_time),
            hysteresis_margin: swift_max(0.0, hysteresis_margin),
            revisit_cooldown: swift_max(0.0, revisit_cooldown),
            maximum_event_age: swift_max(0.1, maximum_event_age),
        }
    }
}

impl Default for AttentionConfiguration {
    fn default() -> Self {
        Self::new(3.0, 12.0, 2.0, 30.0)
    }
}

/// Which source the pet is watching, and what it takes to change its mind.
#[derive(Debug, Clone, Default)]
pub struct AttentionModel {
    pub configuration: AttentionConfiguration,
    current_source_id: Option<String>,
    current_event_id: Option<String>,
    acquired_at: Option<f64>,
    last_left_at: HashMap<String, f64>,
}

impl AttentionModel {
    pub fn new(configuration: AttentionConfiguration) -> Self {
        Self { configuration, ..Default::default() }
    }

    pub fn current_source_id(&self) -> Option<&str> {
        self.current_source_id.as_deref()
    }

    pub fn current_event_id(&self) -> Option<&str> {
        self.current_event_id.as_deref()
    }

    pub fn acquired_at(&self) -> Option<f64> {
        self.acquired_at
    }

    pub fn select(&mut self, events: &[CompanionEvent], timestamp: f64) -> Option<CompanionEvent> {
        let live: Vec<&CompanionEvent> = events
            .iter()
            .filter(|event| {
                timestamp - event.timestamp <= self.configuration.maximum_event_age
                    && timestamp >= event.timestamp
                    && event.kind != CompanionEventKind::ActivityEnded
                    && event.kind != CompanionEventKind::Idle
            })
            .collect();

        // Swift's `max(by:)` keeps the last of equal elements, which decides
        // which of two equally urgent agents the pet goes to.
        let best = last_maximum(&live, |lhs, rhs| {
            self.score(lhs, timestamp) < self.score(rhs, timestamp)
        })
        .map(|event| (*event).clone());
        let Some(best) = best else {
            self.clear(timestamp);
            return None;
        };

        let Some(current_source_id) = self.current_source_id.clone() else {
            self.acquire(&best, timestamp);
            return Some(best);
        };

        if best.source_id == current_source_id {
            self.current_event_id = Some(best.id.clone());
            return Some(best);
        }

        let dwell_age = timestamp - self.acquired_at.unwrap_or(timestamp);
        let urgent = matches!(
            best.kind,
            CompanionEventKind::AttentionRequired
                | CompanionEventKind::Negative
                | CompanionEventKind::Setback
        );
        let held: Vec<&CompanionEvent> = live
            .iter()
            .copied()
            .filter(|event| event.source_id == current_source_id)
            .collect();
        let current = last_maximum(&held, |lhs, rhs| {
            self.score(lhs, timestamp) < self.score(rhs, timestamp)
        })
        .map(|event| (*event).clone());
        let current_score = current.as_ref().map_or(0.0, |event| self.score(event, timestamp));
        let candidate_score = self.score(&best, timestamp);
        let in_cooldown = self
            .last_left_at
            .get(&best.source_id)
            .is_some_and(|left| timestamp - left < self.configuration.revisit_cooldown);

        if (!urgent && dwell_age < self.configuration.minimum_dwell_time)
            || in_cooldown
            || candidate_score < current_score + self.configuration.hysteresis_margin
        {
            return current;
        }

        self.last_left_at.insert(current_source_id, timestamp);
        self.acquire(&best, timestamp);
        Some(best)
    }

    pub fn clear(&mut self, timestamp: f64) {
        if let Some(current) = self.current_source_id.take() {
            self.last_left_at.insert(current, timestamp);
        }
        self.current_event_id = None;
        self.acquired_at = None;
    }

    pub fn score(&self, event: &CompanionEvent, timestamp: f64) -> f64 {
        let base = match event.kind {
            CompanionEventKind::AttentionRequired => 100.0,
            CompanionEventKind::Negative | CompanionEventKind::Setback => 90.0,
            CompanionEventKind::Achievement => {
                if event.intensity >= 0.75 { 80.0 } else { 65.0 }
            }
            CompanionEventKind::Positive => 65.0,
            CompanionEventKind::HighIntensity => 75.0,
            // Reading and searching is work worth standing beside, but it is
            // the quietest kind, so it sits below a tool that changes something.
            CompanionEventKind::Inspecting => 60.0,
            CompanionEventKind::ActivityStarted => 50.0,
            CompanionEventKind::Calm => 30.0,
            CompanionEventKind::ActivityEnded | CompanionEventKind::Idle => 0.0,
        };
        let age = swift_max(0.0, timestamp - event.timestamp);
        let recency = swift_max(0.0, 1.0 - age / self.configuration.maximum_event_age) * 5.0;
        let location = event.location_hint.as_ref().map_or(0.0, |hint| hint.confidence) * 3.0;
        base + event.intensity * 10.0 + recency + location
    }

    fn acquire(&mut self, event: &CompanionEvent, timestamp: f64) {
        self.current_source_id = Some(event.source_id.clone());
        self.current_event_id = Some(event.id.clone());
        self.acquired_at = Some(timestamp);
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ReactionConfiguration {
    pub minimum_interval: f64,
    pub focused_context_scale: f64,
    pub media_context_scale: f64,
    pub gaming_context_scale: f64,
}

impl ReactionConfiguration {
    pub fn new(
        minimum_interval: f64,
        focused_context_scale: f64,
        media_context_scale: f64,
        gaming_context_scale: f64,
    ) -> Self {
        Self {
            minimum_interval: swift_max(0.0, minimum_interval),
            focused_context_scale: clamped(focused_context_scale, 0.0, 1.0),
            media_context_scale: clamped(media_context_scale, 0.0, 1.0),
            gaming_context_scale: clamped(gaming_context_scale, 0.0, 1.0),
        }
    }
}

impl Default for ReactionConfiguration {
    fn default() -> Self {
        Self::new(1.5, 0.8, 0.4, 0.5)
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ReactionPolicy {
    pub configuration: ReactionConfiguration,
    last_reaction_at: Option<f64>,
}

impl ReactionPolicy {
    pub fn new(configuration: ReactionConfiguration) -> Self {
        Self { configuration, last_reaction_at: None }
    }

    pub fn reaction(
        &mut self,
        event: &CompanionEvent,
        context: UserContext,
        current_behavior: ReactingBehavior,
        random_unit: f64,
        timestamp: f64,
    ) -> Option<CompanionReaction> {
        if matches!(current_behavior, ReactingBehavior::Caught | ReactingBehavior::Dragged) {
            return None;
        }
        let urgent = matches!(
            event.kind,
            CompanionEventKind::AttentionRequired
                | CompanionEventKind::Negative
                | CompanionEventKind::Setback
        );
        if let Some(last) = self.last_reaction_at {
            if timestamp - last < self.configuration.minimum_interval && !urgent {
                return None;
            }
        }

        let context_scale = match context {
            UserContext::Working => self.configuration.focused_context_scale,
            UserContext::Gaming => self.configuration.gaming_context_scale,
            UserContext::WatchingMedia => self.configuration.media_context_scale,
            UserContext::Browsing | UserContext::Idle => 1.0,
        };
        let effective_intensity = clamped(event.intensity * context_scale, 0.0, 1.0);
        let roll = clamped(random_unit, 0.0, 0.999_999);

        let result = match event.kind {
            // Always the paw. Petdex answers an approval prompt with `waiting`
            // and holds it until the user replies; rolling a third of these
            // into `observe` meant the pet sometimes never asked at all.
            CompanionEventKind::AttentionRequired => Some(CompanionReaction::Paw),
            CompanionEventKind::Negative | CompanionEventKind::Setback => {
                Some(if effective_intensity >= 0.2 {
                    CompanionReaction::Sad
                } else {
                    CompanionReaction::Glance
                })
            }
            CompanionEventKind::Achievement => {
                if effective_intensity >= 0.75 && roll < effective_intensity * 0.7 {
                    Some(CompanionReaction::LargeCelebrate)
                } else {
                    // An achievement is a meaningful completion. Keep the size
                    // probabilistic, but always make it visible.
                    Some(CompanionReaction::SmallCelebrate)
                }
            }
            CompanionEventKind::Positive => {
                if effective_intensity < 0.15 {
                    None
                } else if effective_intensity >= 0.75 && roll < effective_intensity * 0.7 {
                    Some(CompanionReaction::LargeCelebrate)
                } else if roll < 0.15 + effective_intensity * 0.55 {
                    Some(CompanionReaction::SmallCelebrate)
                } else if roll < 0.75 {
                    Some(CompanionReaction::Glance)
                } else {
                    None
                }
            }
            CompanionEventKind::ActivityStarted => Some(CompanionReaction::Spark),
            CompanionEventKind::Inspecting => Some(CompanionReaction::Observe),
            CompanionEventKind::HighIntensity => Some(if effective_intensity >= 0.5 {
                CompanionReaction::Work
            } else {
                CompanionReaction::Observe
            }),
            CompanionEventKind::Calm => Some(CompanionReaction::Calm),
            CompanionEventKind::ActivityEnded | CompanionEventKind::Idle => None,
        };

        if result.is_some() {
            self.last_reaction_at = Some(timestamp);
        }
        result
    }
}
