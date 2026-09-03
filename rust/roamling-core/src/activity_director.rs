// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

//! The activity half of `RoamlingRuntime`, ported from
//! `Sources/RoamlingEngine/RoamlingRuntime.swift`.
//!
//! Which agent the pet is watching, what it has been told to wear, and what it
//! still owes the user when it arrives. All of that survived between ticks as
//! seven fields on the runtime, and every one of them was written by more than
//! one code path.
//!
//! What it cannot do is move the pet: the movement, behaviour and placement
//! handles belong to the caller for now. So it answers in effects, in the order
//! the Swift original performed them. When the rest of the runtime comes over,
//! these become direct calls and the list goes away.

use std::collections::BTreeMap;

use crate::activity::{
    ActivityLifetime, CompanionEvent, CompanionEventKind, CompanionReaction, ReactingBehavior,
    UserContext,
};
use crate::attention::{AttentionModel, ReactionPolicy};
use crate::geometry::WorldRect;
use crate::world::LocationHint;

/// One thing the caller must do, in the order the Swift original did it.
#[derive(Debug, Clone, PartialEq)]
pub enum ActivityEffect {
    /// Wake the pet, because what arrived is worth getting up for.
    CancelRest,
    /// The trip is off but the window is still the one being watched.
    SettleInPlace { source_id: String },
    /// `movement.cancelRoute(stop: false)`.
    CancelRoute,
    SetNextWanderAt { timestamp: f64 },
    /// `behavior.handle(.reaction(_))`.
    ApplyReaction { reaction: CompanionReaction },
    /// Ask the platform for a fresh capture near the window being watched.
    RequestLuminance { region: WorldRect },
}

/// Routine tool completions are useful as adapter-level evidence but do not
/// deserve attention changes or visible reactions on their own.
const ROUTINE_POSITIVE_INTENSITY: f64 = 0.15;

/// Whether an event without a window of its own is worth asking the platform
/// where its window is. The query costs a synchronous round trip, so the rule
/// lives here and the call stays with the caller.
pub fn wants_window_hint(kind: CompanionEventKind) -> bool {
    matches!(
        kind,
        CompanionEventKind::ActivityStarted
            | CompanionEventKind::HighIntensity
            | CompanionEventKind::AttentionRequired
    )
}

#[derive(Debug, Default)]
pub struct ActivityDirector {
    attention: AttentionModel,
    reactions: ReactionPolicy,
    /// Keyed by source and iterated in key order. It was a dictionary, and
    /// Swift randomizes dictionary order per process, so on a tie between two
    /// agents the pet watched whichever one the launch happened to pick.
    recent: BTreeMap<String, CompanionEvent>,
    pending: Option<CompanionEvent>,
    last_dispatched_id: Option<String>,
    active_source_id: Option<String>,
    hint: Option<LocationHint>,
    heard_at: f64,
    active_reaction: Option<CompanionReaction>,
    arrival_reaction: Option<CompanionReaction>,
}

impl ActivityDirector {
    pub fn is_watching_window(&self) -> bool {
        self.active_source_id.is_some() && self.hint.is_some()
    }

    pub fn active_source_id(&self) -> Option<&str> {
        self.active_source_id.as_deref()
    }

    pub fn hint(&self) -> Option<&LocationHint> {
        self.hint.as_ref()
    }

    pub fn has_arrival_reaction(&self) -> bool {
        self.arrival_reaction.is_some()
    }

    /// What the pet wears while it simply sits beside a working agent. Only a
    /// lasting condition qualifies; the caller checks `is_ongoing`.
    pub fn sustained_reaction(&self) -> Option<CompanionReaction> {
        self.active_reaction
    }

    /// The event this arrived as, already located: the caller resolves the
    /// window, because that is a platform query.
    pub fn handle_event(
        &mut self,
        event: CompanionEvent,
        is_held_by_pointer: bool,
        is_resting: bool,
        random_unit: f64,
        now: f64,
    ) -> Vec<ActivityEffect> {
        let mut effects = Vec::new();
        if event.kind == CompanionEventKind::Positive
            && event.intensity < ROUTINE_POSITIVE_INTENSITY
        {
            return effects;
        }

        if matches!(
            event.kind,
            CompanionEventKind::ActivityEnded | CompanionEventKind::Idle
        ) {
            self.recent.remove(&event.source_id);
            if self.attention.current_source_id() == Some(event.source_id.as_str()) {
                self.attention.clear(now);
                self.last_dispatched_id = None;
            }
            if self.active_source_id.as_deref() == Some(event.source_id.as_str()) {
                self.clear_active(now, &mut effects);
                self.apply_reaction(CompanionReaction::Calm, is_resting, now, &mut effects);
            }
            self.queue_next_candidate(now);
            return effects;
        }

        self.recent.insert(event.source_id.clone(), event);
        let live: Vec<CompanionEvent> = self.recent.values().cloned().collect();
        let Some(selected_id) = self.attention.select(&live, now).map(|event| event.id) else {
            return effects;
        };
        let Some(selected) = live.iter().find(|event| event.id == selected_id).cloned() else {
            return effects;
        };
        if Some(selected.id.as_str()) == self.last_dispatched_id.as_deref() {
            return effects;
        }

        if is_held_by_pointer {
            self.pending = Some(selected);
            return effects;
        }
        if is_resting {
            if !selected.kind.wakes_resting_pet() {
                return effects;
            }
            effects.push(ActivityEffect::CancelRest);
            self.pending = Some(selected);
            return effects;
        }
        self.dispatch(selected, is_held_by_pointer, is_resting, random_unit, now, &mut effects);
        effects
    }

    /// A Stop hook cannot run for a session that was interrupted or killed, and
    /// driving agents from a GUI is exactly how that happens. Without this the
    /// pet stays on duty forever: never roaming, and able to sleep only while
    /// its seat keeps scoring clear.
    pub fn expire_silent(&mut self, is_resting: bool, now: f64) -> Vec<ActivityEffect> {
        let mut effects = Vec::new();
        if self.active_source_id.is_none()
            || !ActivityLifetime::has_fallen_silent(self.heard_at, now)
        {
            return effects;
        }
        self.clear_active(now, &mut effects);
        self.apply_reaction(CompanionReaction::Calm, is_resting, now, &mut effects);
        effects
    }

    pub fn resume_pending_if_ready(
        &mut self,
        is_idle: bool,
        is_held_by_pointer: bool,
        is_resting: bool,
        random_unit: f64,
        now: f64,
    ) -> Vec<ActivityEffect> {
        let mut effects = Vec::new();
        if !is_idle {
            return effects;
        }
        let Some(event) = self.pending.take() else { return effects };
        self.dispatch(event, is_held_by_pointer, is_resting, random_unit, now, &mut effects);
        effects
    }

    /// The reaction an event asked for is owed to the user until the pet
    /// settles, whether it walked to a new seat or kept the one it had.
    pub fn deliver_arrival_reaction(
        &mut self,
        is_resting: bool,
        now: f64,
    ) -> Vec<ActivityEffect> {
        let mut effects = Vec::new();
        let reaction = self
            .arrival_reaction
            .or(self.active_reaction)
            .unwrap_or(CompanionReaction::Observe);
        self.arrival_reaction = None;
        self.apply_reaction(reaction, is_resting, now, &mut effects);
        effects
    }

    /// Re-applies the lasting condition the pet is wearing while it holds a
    /// seat. A moment is delivered once; only `is_ongoing` reactions repeat.
    pub fn sustain_on_seat(&mut self, is_resting: bool, now: f64) -> Vec<ActivityEffect> {
        let mut effects = Vec::new();
        let Some(sustained) = self.active_reaction else { return effects };
        if !sustained.is_ongoing() {
            return effects;
        }
        self.apply_reaction(sustained, is_resting, now, &mut effects);
        effects
    }

    fn dispatch(
        &mut self,
        event: CompanionEvent,
        is_held_by_pointer: bool,
        is_resting: bool,
        random_unit: f64,
        now: f64,
        effects: &mut Vec<ActivityEffect>,
    ) {
        self.last_dispatched_id = Some(event.id.clone());
        if self.active_source_id.is_none()
            || self.active_source_id.as_deref() == Some(event.source_id.as_str())
        {
            self.heard_at = now;
        }
        let behavior = if is_held_by_pointer {
            ReactingBehavior::Caught
        } else {
            ReactingBehavior::Other
        };
        let reaction = self.reactions.reaction(
            &event,
            event.context.unwrap_or(UserContext::Idle),
            behavior,
            random_unit,
            now,
        );

        match event.kind {
            // The hop is the whole reaction. What the pet wears afterwards is
            // stillness, because Petdex's `jumping` is a duration state and the
            // next hook event -- a tool starting -- is a beat away.
            CompanionEventKind::ActivityStarted => self.begin_watching(
                &event,
                CompanionReaction::Observe,
                reaction.unwrap_or(CompanionReaction::Spark),
                is_resting,
                now,
                effects,
            ),
            CompanionEventKind::Inspecting => self.begin_watching(
                &event,
                CompanionReaction::Observe,
                reaction.unwrap_or(CompanionReaction::Observe),
                is_resting,
                now,
                effects,
            ),
            CompanionEventKind::HighIntensity => self.begin_watching(
                &event,
                CompanionReaction::Work,
                reaction.unwrap_or(CompanionReaction::Work),
                is_resting,
                now,
                effects,
            ),
            // The paw is sustained too: the agent stays blocked until the user
            // answers, so the pet has to keep asking rather than drift off it.
            CompanionEventKind::AttentionRequired => self.begin_watching(
                &event,
                CompanionReaction::Paw,
                reaction.unwrap_or(CompanionReaction::Paw),
                is_resting,
                now,
                effects,
            ),
            CompanionEventKind::Positive => {
                if let Some(reaction) = reaction {
                    self.apply_reaction(reaction, is_resting, now, effects);
                }
            }
            CompanionEventKind::Achievement => {
                self.clear_active(now, effects);
                self.apply_reaction(
                    reaction.unwrap_or(CompanionReaction::Glance),
                    is_resting,
                    now,
                    effects,
                );
                self.finish_transient(&event, now);
            }
            CompanionEventKind::Negative => {
                self.clear_active(now, effects);
                self.apply_reaction(
                    reaction.unwrap_or(CompanionReaction::Sad),
                    is_resting,
                    now,
                    effects,
                );
                self.finish_transient(&event, now);
            }
            CompanionEventKind::Setback => {
                self.active_source_id = Some(event.source_id.clone());
                self.heard_at = now;
                self.active_reaction = Some(CompanionReaction::Observe);
                // The trip is off but the window is still the one being
                // watched, so the hint stays and the seat keeps being judged
                // where the pet is.
                effects.push(ActivityEffect::SettleInPlace {
                    source_id: event.source_id.clone(),
                });
                effects.push(ActivityEffect::CancelRoute);
                self.apply_reaction(
                    reaction.unwrap_or(CompanionReaction::Sad),
                    is_resting,
                    now,
                    effects,
                );
            }
            CompanionEventKind::ActivityEnded | CompanionEventKind::Calm
            | CompanionEventKind::Idle => {
                if event.kind == CompanionEventKind::Calm
                    && (self.active_source_id.is_none()
                        || self.active_source_id.as_deref() == Some(event.source_id.as_str()))
                {
                    self.clear_active(now, effects);
                    self.apply_reaction(
                        reaction.unwrap_or(CompanionReaction::Calm),
                        is_resting,
                        now,
                        effects,
                    );
                }
            }
        }
    }

    /// Records the window to watch and the reaction the user is owed. Where the
    /// pet stands to watch it is the director's answer, on the next tick.
    fn begin_watching(
        &mut self,
        event: &CompanionEvent,
        sustained: CompanionReaction,
        reaction: CompanionReaction,
        is_resting: bool,
        now: f64,
        effects: &mut Vec<ActivityEffect>,
    ) {
        if let Some(hint) = &event.location_hint {
            self.hint = Some(hint.clone());
            if let Some(region) = hint.approximate_region {
                effects.push(ActivityEffect::RequestLuminance { region });
            }
        } else if self.active_source_id.as_deref() != Some(event.source_id.as_str()) {
            // A different agent arriving without a window to point at: the last
            // one's window is not evidence about this one, and leaving it in
            // place would walk the pet to the wrong screen.
            self.hint = None;
        }
        self.active_source_id = Some(event.source_id.clone());
        self.heard_at = now;
        self.active_reaction = Some(sustained);
        if self.hint.is_none() {
            // Nothing to walk to, so the reaction plays where the pet is.
            self.arrival_reaction = None;
            self.apply_reaction(reaction, is_resting, now, effects);
            return;
        }
        self.arrival_reaction = Some(reaction);
    }

    fn finish_transient(&mut self, event: &CompanionEvent, now: f64) {
        self.recent.remove(&event.source_id);
        self.attention.clear(now);
        self.queue_next_candidate(now);
    }

    fn queue_next_candidate(&mut self, now: f64) {
        let live: Vec<CompanionEvent> = self.recent.values().cloned().collect();
        let Some(next_id) = self.attention.select(&live, now).map(|event| event.id) else {
            self.pending = None;
            return;
        };
        let Some(next) = live.iter().find(|event| event.id == next_id) else {
            self.pending = None;
            return;
        };
        self.pending = if Some(next.id.as_str()) == self.last_dispatched_id.as_deref() {
            None
        } else {
            Some(next.clone())
        };
    }

    /// The director releases the seat on its own once there is no source to
    /// watch, so nothing here has to remember to clear a placement flag.
    fn clear_active(&mut self, now: f64, effects: &mut Vec<ActivityEffect>) {
        self.active_source_id = None;
        self.active_reaction = None;
        self.arrival_reaction = None;
        self.hint = None;
        effects.push(ActivityEffect::CancelRoute);
        effects.push(ActivityEffect::SetNextWanderAt { timestamp: now + 2.0 });
    }

    /// Reactions never wake the creature by themselves. Callers that mean to
    /// interrupt rest emit `CancelRest` first, so a session that simply ends
    /// leaves a sleeping pet asleep.
    fn apply_reaction(
        &mut self,
        reaction: CompanionReaction,
        is_resting: bool,
        now: f64,
        effects: &mut Vec<ActivityEffect>,
    ) {
        if is_resting {
            return;
        }
        effects.push(ActivityEffect::ApplyReaction { reaction });
        effects.push(ActivityEffect::CancelRoute);
        effects.push(ActivityEffect::SetNextWanderAt {
            timestamp: if self.active_source_id.is_none() {
                now + 2.0
            } else {
                f64::INFINITY
            },
        });
    }
}
