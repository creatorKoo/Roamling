// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

//! Ported from `Sources/RoamlingCore/Activity.swift`.
//!
//! Only what attention and reactions read comes across. Metadata does not: it
//! is carried for diagnostics and never scored, and the point of this type is
//! that it cannot express what the user was doing.

use crate::geometry::clamped;
use crate::world::LocationHint;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CompanionEventKind {
    ActivityStarted,
    ActivityEnded,
    Positive,
    Negative,
    Achievement,
    Setback,
    AttentionRequired,
    /// The source is examining rather than changing things -- reading a file,
    /// running a search. Petdex draws this as `review`, distinct from the
    /// `running` it shows for every other tool, so the split is carried here
    /// rather than folded into `HighIntensity`.
    Inspecting,
    HighIntensity,
    Calm,
    Idle,
}

impl CompanionEventKind {
    /// Whether this event is worth getting a sleeping pet up for.
    ///
    /// An agent emits an event per tool call. If each of them woke the pet it
    /// could doze for one beat and never longer, so routine progress -- the
    /// thing the pet is already sitting next to -- lets it sleep, and only a
    /// result or a request for the user gets it up.
    pub fn wakes_resting_pet(self) -> bool {
        matches!(
            self,
            Self::AttentionRequired | Self::Achievement | Self::Negative | Self::Setback
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UserContext {
    Working,
    Gaming,
    WatchingMedia,
    Browsing,
    Idle,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CompanionEvent {
    pub id: String,
    pub source_id: String,
    pub timestamp: f64,
    pub kind: CompanionEventKind,
    pub intensity: f64,
    pub location_hint: Option<LocationHint>,
    /// What the user was doing when this arrived. Only the reaction policy
    /// reads it, and only some adapters supply it.
    pub context: Option<UserContext>,
}

impl CompanionEvent {
    pub fn new(
        id: impl Into<String>,
        source_id: impl Into<String>,
        timestamp: f64,
        kind: CompanionEventKind,
        intensity: f64,
        location_hint: Option<LocationHint>,
    ) -> Self {
        Self {
            id: id.into(),
            source_id: source_id.into(),
            timestamp,
            kind,
            intensity: clamped(intensity, 0.0, 1.0),
            location_hint,
            context: None,
        }
    }

    pub fn with_context(mut self, context: Option<UserContext>) -> Self {
        self.context = context;
        self
    }
}

/// When an agent that stopped talking should be treated as finished.
///
/// Nothing else ends a watch. `ActivityEnded` arrives from a Stop hook, and a
/// hook cannot run for a session that was interrupted, killed or disconnected
/// -- which is the ordinary way an agent ends when driven from a GUI. A watch
/// that never ends freezes the pet's whole idle life.
pub struct ActivityLifetime;

impl ActivityLifetime {
    /// Long enough to sit through a slow tool call without the pet wandering
    /// off mid-build, short enough that a missed Stop costs one stroll rather
    /// than the rest of the session.
    pub const SILENCE_BEFORE_EXPIRY: f64 = 300.0;

    pub fn has_fallen_silent(last_event_at: f64, now: f64) -> bool {
        now - last_event_at >= Self::SILENCE_BEFORE_EXPIRY
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CompanionReaction {
    Glance,
    Observe,
    /// The agent just started a turn. Petdex plays `jumping` here and calls it
    /// "Thinking…", so Roamling does the same rather than staring.
    Spark,
    Work,
    Paw,
    SmallCelebrate,
    LargeCelebrate,
    Sad,
    Calm,
}

impl CompanionReaction {
    /// True when the reaction describes a condition that lasts rather than a
    /// moment that passes. Only these may be re-applied while the pet holds a
    /// seat beside a working agent.
    pub fn is_ongoing(self) -> bool {
        matches!(self, Self::Work | Self::Paw)
    }
}

/// The behaviour states the reaction policy has to know about. The rest of the
/// state machine stays in Swift until unit 5.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ReactingBehavior {
    Caught,
    Dragged,
    Other,
}
