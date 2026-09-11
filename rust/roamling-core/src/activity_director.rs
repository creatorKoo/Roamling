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
    ActivityLifetime, ActivitySourceType, CompanionEvent, CompanionEventKind, CompanionReaction,
    ReactingBehavior, UserContext,
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
            | CompanionEventKind::Present
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
    /// What kind of source `active_source_id` is. Set and cleared with it.
    active_source_type: Option<ActivitySourceType>,
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

    /// The event the director last acted on. The working-app source reads it
    /// to tell whether its greeting has reached the pet at all: an event that
    /// is queued for a pet in the user's hand, passed over for a sleeping one,
    /// or outranked by an agent has not been dispatched, and nothing about the
    /// seat says so -- the seat is often that source's already.
    pub fn last_dispatched_id(&self) -> Option<&str> {
        self.last_dispatched_id.as_deref()
    }

    /// The event queued for the pet once it is free, if any. Read only by the
    /// switch-over test, which counts the queued desk words it sees dropped
    /// for an agent on duty rather than trusting a random script to reach
    /// them.
    pub fn pending_event_id(&self) -> Option<&str> {
        self.pending.as_ref().map(|event| event.id.as_str())
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
        let live = self.candidates(now);
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
        // Queued before an agent came back on duty: the agent finished, the
        // desk's word was next, and the agent started again while the pet was
        // still busy. Nothing re-reads the queue against `candidates`, so the
        // word took the agent's seat the moment the pet was free. Dropped, not
        // kept: it is still in `recent` for when the agent lets go, and the
        // desk says it again meanwhile (plan §9.6b).
        if event.source_type == ActivitySourceType::System && self.agent_on_duty(now) {
            return effects;
        }
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
            // Walk over and sit, wearing nothing. Every dispatch re-arms the
            // arrival reaction and a seated pet puts it on, so a kind that is
            // re-sent each minute can only wear what replays invisibly: `Calm`,
            // which is plain `idle`.
            CompanionEventKind::Present => self.begin_watching(
                &event,
                CompanionReaction::Calm,
                reaction.unwrap_or(CompanionReaction::Calm),
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
                self.active_source_type = Some(event.source_type);
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
        self.active_source_type = Some(event.source_type);
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
        let live = self.candidates(now);
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

    /// What attention may choose from: every source heard lately, except that
    /// the desk is not a candidate at all while an agent is on duty.
    ///
    /// Not a score. The desk's first keystroke scores within the hysteresis
    /// margin of an agent's fresh turn, so as a score it lost the hop to the
    /// turn and then won the seat back twenty seconds later with `running` --
    /// and its question outranked a working agent outright. The user started
    /// the agent; typing beside it is not a reason to walk away. The desk's
    /// events stay in `recent`, so the moment the agent lets go the next
    /// candidate is already there.
    ///
    /// Attention itself -- score, margin, dwell, cooldown -- is untouched, and
    /// with no desk event in `recent` this is exactly the list it always got.
    fn candidates(&self, now: f64) -> Vec<CompanionEvent> {
        let agent_on_duty = self.agent_on_duty(now);
        self.recent
            .values()
            .filter(|event| !(agent_on_duty && event.source_type == ActivitySourceType::System))
            .cloned()
            .collect()
    }

    /// An agent has been heard within the age attention still counts, or
    /// holds the seat and has not gone silent for good. The second half is
    /// what keeps a long tool call's seat: attention stops counting the agent
    /// after thirty seconds, the watch lasts five minutes.
    ///
    /// Public for the switch-over test, which holds both directors to the rule
    /// by what they act on.
    pub fn agent_on_duty(&self, now: f64) -> bool {
        let maximum_age = self.attention.configuration.maximum_event_age;
        let heard_lately = self.recent.values().any(|event| {
            event.source_type == ActivitySourceType::Agent
                && now - event.timestamp <= maximum_age
                && now >= event.timestamp
        });
        heard_lately
            || (self.active_source_type == Some(ActivitySourceType::Agent)
                && !ActivityLifetime::has_fallen_silent(self.heard_at, now))
    }

    /// The director releases the seat on its own once there is no source to
    /// watch, so nothing here has to remember to clear a placement flag.
    fn clear_active(&mut self, now: f64, effects: &mut Vec<ActivityEffect>) {
        self.active_source_id = None;
        self.active_source_type = None;
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

#[cfg(test)]
mod tests {
    use super::*;

    const HWP: &str = "focus:Hwp.exe";
    const AGENT: &str = "claude-code:session-1";

    fn editor() -> WorldRect {
        WorldRect::new(200.0, 200.0, 700.0, 500.0)
    }

    fn terminal() -> WorldRect {
        WorldRect::new(1_000.0, 450.0, 300.0, 300.0)
    }

    fn event(
        id: &str,
        source: &str,
        kind: CompanionEventKind,
        intensity: f64,
        at: f64,
        window: WorldRect,
        confidence: f64,
    ) -> CompanionEvent {
        CompanionEvent::new(id, source, at, kind, intensity, Some(LocationHint::new(Some(window), confidence)))
            .with_context(Some(UserContext::Working))
    }

    fn reactions(effects: &[ActivityEffect]) -> Vec<CompanionReaction> {
        effects
            .iter()
            .filter_map(|effect| match effect {
                ActivityEffect::ApplyReaction { reaction } => Some(*reaction),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn present_is_worth_asking_where_the_window_is() {
        assert!(wants_window_hint(CompanionEventKind::Present));
    }

    /// Walks over, sits in `Calm`, and keeps the seat. The minute's re-send
    /// re-arms the arrival reaction, which is why that reaction is `Calm`: a
    /// seated pet puts it on again and nobody can see the difference.
    #[test]
    fn present_walks_the_pet_over_and_sits_it_down_wearing_nothing() {
        let mut director = ActivityDirector::default();
        let effects = director.handle_event(
            event("f1", HWP, CompanionEventKind::Present, 0.5, 100.0, editor(), 0.8),
            false,
            false,
            0.5,
            100.0,
        );
        assert!(reactions(&effects).is_empty(), "reacted before walking: {effects:?}");
        assert!(director.is_watching_window());
        assert!(director.has_arrival_reaction());
        assert_eq!(director.last_dispatched_id(), Some("f1"));

        let arrived = director.deliver_arrival_reaction(false, 102.0);
        assert_eq!(reactions(&arrived), [CompanionReaction::Calm]);
        assert_eq!(director.sustained_reaction(), Some(CompanionReaction::Calm));
        // Not a lasting condition, so sitting there re-applies nothing.
        assert!(director.sustain_on_seat(false, 103.0).is_empty());

        let effects = director.handle_event(
            event("f2", HWP, CompanionEventKind::Present, 0.5, 160.0, editor(), 0.8),
            false,
            false,
            0.5,
            160.0,
        );
        assert!(reactions(&effects).is_empty(), "the re-send reacted: {effects:?}");
        assert_eq!(director.last_dispatched_id(), Some("f2"));
        assert_eq!(director.active_source_id(), Some(HWP));
        let again = director.deliver_arrival_reaction(false, 160.1);
        assert_eq!(reactions(&again), [CompanionReaction::Calm]);
    }

    /// The user's app never draws the pet off an agent's seat. Stacked against
    /// it as far as attention allows: the quietest agent event at the edge of
    /// the age attention still counts, and the loudest `Present` there can be.
    #[test]
    fn present_never_takes_a_seat_from_an_agent() {
        for agent_kind in [
            CompanionEventKind::ActivityStarted,
            CompanionEventKind::Inspecting,
            CompanionEventKind::HighIntensity,
            CompanionEventKind::AttentionRequired,
            CompanionEventKind::Setback,
        ] {
            let mut director = ActivityDirector::default();
            director.handle_event(
                event("a1", AGENT, agent_kind, 0.0, 100.0, terminal(), 0.0),
                false,
                false,
                0.5,
                100.0,
            );
            director.deliver_arrival_reaction(false, 101.0);

            let mut now = 100.0;
            let mut sent = 0;
            while now < 130.0 {
                now += 0.5;
                sent += 1;
                let effects = director.handle_event(
                    event(&format!("f{sent}"), HWP, CompanionEventKind::Present, 1.0, now, editor(), 1.0),
                    false,
                    false,
                    0.5,
                    now,
                );
                assert!(effects.is_empty(), "{agent_kind:?} lost the pet at {now}: {effects:?}");
                assert_eq!(director.active_source_id(), Some(AGENT), "{agent_kind:?} at {now}");
                assert_eq!(director.last_dispatched_id(), Some("a1"));
            }
        }
    }

    /// Bringing an app to the front is not a reason to get up. The first
    /// keystroke is: typing reaches the pet as user input, not as an event.
    #[test]
    fn present_lets_a_sleeping_pet_sleep() {
        let mut director = ActivityDirector::default();
        let effects = director.handle_event(
            event("f1", HWP, CompanionEventKind::Present, 0.5, 100.0, editor(), 0.8),
            false,
            true,
            0.5,
            100.0,
        );
        assert!(effects.is_empty(), "woke the pet: {effects:?}");
        assert_eq!(director.active_source_id(), None);
        assert_eq!(director.last_dispatched_id(), None);
        assert!(!CompanionEventKind::Present.wakes_resting_pet());
    }

    // ------------------------------------------- plan §9 B2: an agent comes first

    /// What the desk says, as `focus_activity.rs` says it: loud and certain,
    /// the best it can score.
    fn desk(id: &str, kind: CompanionEventKind, at: f64) -> CompanionEvent {
        event(id, HWP, kind, 1.0, at, editor(), 1.0).with_source_type(ActivitySourceType::System)
    }

    /// An agent at work beside its own window, the pet already dressed for it.
    fn agent_at_work(at: f64) -> ActivityDirector {
        let mut director = ActivityDirector::default();
        director.handle_event(
            event("a1", AGENT, CompanionEventKind::HighIntensity, 0.8, at, terminal(), 0.8),
            false,
            false,
            0.5,
            at,
        );
        director.deliver_arrival_reaction(false, at + 1.0);
        assert_eq!(director.active_source_id(), Some(AGENT));
        director
    }

    /// Sitting, the first keystroke and typing, for a minute: through the
    /// thirty seconds attention counts the agent's last word and past them,
    /// where as a score the desk would have won.
    #[test]
    fn the_desk_never_draws_the_pet_off_a_working_agent() {
        use CompanionEventKind::{ActivityStarted, HighIntensity, Present};
        for kind in [Present, ActivityStarted, HighIntensity] {
            let mut director = agent_at_work(100.0);
            let mut now = 101.0;
            let mut sent = 0;
            while now < 160.0 {
                now += 0.5;
                sent += 1;
                let effects =
                    director.handle_event(desk(&format!("f{sent}"), kind, now), false, false, 0.5, now);
                assert!(effects.is_empty(), "{kind:?} took the pet at {now}: {effects:?}");
                assert_eq!(director.active_source_id(), Some(AGENT), "{kind:?} at {now}");
                assert_eq!(director.last_dispatched_id(), Some("a1"));
            }
        }
    }

    /// The desk's question is urgent to attention, so as a score it skipped
    /// the dwell and took a working agent's seat on the spot.
    #[test]
    fn the_desks_question_does_not_interrupt_a_working_agent() {
        let mut director = agent_at_work(100.0);
        let effects = director.handle_event(
            desk("f1", CompanionEventKind::AttentionRequired, 105.0),
            false,
            false,
            0.5,
            105.0,
        );
        assert!(effects.is_empty(), "the question took the seat: {effects:?}");
        assert_eq!(director.active_source_id(), Some(AGENT));
        assert_eq!(director.sustained_reaction(), Some(CompanionReaction::Work));
    }

    /// A long tool call is still the agent's turn: the watch outlives
    /// attention's thirty seconds by design, and the desk waits for the whole
    /// of it. When the watch expires the desk's next word has the seat.
    #[test]
    fn a_quiet_agent_keeps_the_seat_until_its_watch_expires() {
        let mut director = agent_at_work(100.0);
        for at in [160.0, 250.0, 399.5] {
            let effects = director.handle_event(
                desk(&format!("f{at}"), CompanionEventKind::Present, at),
                false,
                false,
                0.5,
                at,
            );
            assert!(effects.is_empty(), "the desk took a quiet agent's seat at {at}: {effects:?}");
            assert_eq!(director.active_source_id(), Some(AGENT));
        }
        assert!(!director.expire_silent(false, 400.0).is_empty(), "the watch did not expire");

        director.handle_event(desk("f2", CompanionEventKind::Present, 401.0), false, false, 0.5, 401.0);
        assert_eq!(director.active_source_id(), Some(HWP));
        assert_eq!(director.last_dispatched_id(), Some("f2"));
    }

    /// The agent finishes, or ends, and what the desk said while it waited is
    /// already the next candidate: queued, and dispatched once the pet is free.
    #[test]
    fn the_desk_takes_the_seat_the_moment_the_agent_lets_go() {
        use CompanionEventKind::{Achievement, ActivityEnded, HighIntensity};
        for last_word in [Achievement, ActivityEnded] {
            let mut director = agent_at_work(100.0);
            director.handle_event(desk("f1", HighIntensity, 110.0), false, false, 0.5, 110.0);
            assert_eq!(director.active_source_id(), Some(AGENT));

            director.handle_event(
                event("a2", AGENT, last_word, 0.55, 112.0, terminal(), 0.8),
                false,
                false,
                0.5,
                112.0,
            );
            assert_eq!(director.active_source_id(), None, "{last_word:?}");
            assert!(
                director.resume_pending_if_ready(false, false, false, 0.5, 112.5).is_empty(),
                "{last_word:?}: the desk went ahead of the agent's goodbye"
            );
            director.resume_pending_if_ready(true, false, false, 0.5, 113.0);
            assert_eq!(director.active_source_id(), Some(HWP), "{last_word:?}");
            assert_eq!(director.last_dispatched_id(), Some("f1"));
            assert_eq!(director.sustained_reaction(), Some(CompanionReaction::Work));
        }
    }

    /// The other way round. As a score an agent's new turn lost to the typing
    /// it interrupted; as a candidate list it is the only thing there. Past the
    /// desk's dwell, because attention's dwell is unchanged.
    #[test]
    fn an_agent_starting_a_turn_takes_the_seat_from_the_desk() {
        use CompanionEventKind::{ActivityStarted, HighIntensity, Present};
        let mut director = ActivityDirector::default();
        director.handle_event(desk("f1", Present, 100.0), false, false, 0.5, 100.0);
        director.deliver_arrival_reaction(false, 101.0);
        director.handle_event(desk("f2", HighIntensity, 102.0), false, false, 0.5, 102.0);
        assert_eq!(director.last_dispatched_id(), Some("f2"));

        let effects = director.handle_event(
            event("a1", AGENT, ActivityStarted, 0.35, 110.0, terminal(), 0.8),
            false,
            false,
            0.5,
            110.0,
        );
        assert_eq!(director.active_source_id(), Some(AGENT), "{effects:?}");
        assert_eq!(director.last_dispatched_id(), Some("a1"));

        director.handle_event(desk("f3", HighIntensity, 111.0), false, false, 0.5, 111.0);
        assert_eq!(director.active_source_id(), Some(AGENT), "the desk took it straight back");
    }

    /// Plan §9.6b. The agent finishes and the desk's word is queued. The pet
    /// is not free to take it up -- looking at the cursor, say -- for longer
    /// than attention keeps the agent out (its cooldown, and the dwell the
    /// queued word acquired), and in that time the agent starts its next turn
    /// and takes the seat directly. When the pet is free, the queued word is
    /// from before the agent came back: dropped, not dispatched. Dropping it
    /// forgets nothing -- the word is still in `recent`, so the next time the
    /// agent lets go the desk takes the seat.
    #[test]
    fn a_desk_word_queued_before_the_agent_came_back_does_not_take_its_seat() {
        use CompanionEventKind::{Achievement, ActivityStarted, HighIntensity};
        let mut director = agent_at_work(100.0);
        director.handle_event(desk("f1", HighIntensity, 110.0), false, false, 0.5, 110.0);

        director.handle_event(
            event("a2", AGENT, Achievement, 0.55, 112.0, terminal(), 0.8),
            false,
            false,
            0.5,
            112.0,
        );
        assert_eq!(director.active_source_id(), None);
        let mut now = 112.0;
        while now < 115.5 {
            now += 0.5;
            assert!(director.resume_pending_if_ready(false, false, false, 0.5, now).is_empty());
        }

        director.handle_event(
            event("a3", AGENT, ActivityStarted, 0.35, 116.0, terminal(), 0.8),
            false,
            false,
            0.5,
            116.0,
        );
        assert_eq!(director.active_source_id(), Some(AGENT), "the agent's next turn did not take the seat");
        assert_eq!(director.last_dispatched_id(), Some("a3"));

        let effects = director.resume_pending_if_ready(true, false, false, 0.5, 116.5);
        assert!(effects.is_empty(), "the queued desk word acted: {effects:?}");
        assert_eq!(director.active_source_id(), Some(AGENT), "the queued desk word took the agent's seat");
        assert_eq!(director.last_dispatched_id(), Some("a3"));
        assert!(
            director.resume_pending_if_ready(true, false, false, 0.5, 117.0).is_empty(),
            "the dropped word came back"
        );

        director.handle_event(
            event("a4", AGENT, Achievement, 0.55, 120.0, terminal(), 0.8),
            false,
            false,
            0.5,
            120.0,
        );
        director.resume_pending_if_ready(true, false, false, 0.5, 121.0);
        assert_eq!(director.active_source_id(), Some(HWP), "dropping the queued word forgot the desk");
        assert_eq!(director.last_dispatched_id(), Some("f1"));
    }
}
