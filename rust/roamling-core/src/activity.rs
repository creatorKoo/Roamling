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
    /// The user is at their own work and nothing calls for a reaction: an app
    /// they named as work came to the front. The pet walks over and sits.
    ///
    /// Only the working-app source says this; agent normalization never does.
    /// No other kind walks the pet over without dressing it in something, and
    /// this one is re-sent every minute to keep the seat, so what it wears has
    /// to be the one reaction whose replay cannot be seen -- `Calm`.
    ///
    /// Last in the list because kinds cross the FFI as indices, and every
    /// fixture recorded before it existed names the others by position.
    Present,
}

impl CompanionEventKind {
    /// Whether this event is worth getting a sleeping pet up for.
    ///
    /// An agent emits an event per tool call. If each of them woke the pet it
    /// could doze for one beat and never longer, so routine progress -- the
    /// thing the pet is already sitting next to -- lets it sleep, and only a
    /// result or a request for the user gets it up. A work app merely coming
    /// to the front (`Present`) is not either.
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

/// What kind of thing an event came from. Ported from Swift's
/// `ActivitySourceType`, without the name `custom` carries there: no rule reads
/// it, so it stops at the boundary.
///
/// One rule reads the rest. While an agent holds the pet's attention, what the
/// desk says about the user's own app is not a candidate at all
/// (`ActivityDirector`): the pet is beside the agent the user started, and
/// typing next to it is not a reason to walk away.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ActivitySourceType {
    Agent,
    Game,
    Media,
    /// The desk itself: the app in front, from `focus_activity.rs`.
    System,
    Custom,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CompanionEvent {
    pub id: String,
    pub source_id: String,
    /// `Agent` unless said otherwise. Every source there was before the desk
    /// was an agent's hook, and every fixture recorded then is one.
    pub source_type: ActivitySourceType,
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
            source_type: ActivitySourceType::Agent,
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

    pub fn with_source_type(mut self, source_type: ActivitySourceType) -> Self {
        self.source_type = source_type;
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
