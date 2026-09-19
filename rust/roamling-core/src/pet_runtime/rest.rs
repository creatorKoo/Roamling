// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

//! Going to sleep and staying asleep: when to sit, how long, and what wakes
//! the pet. Where it sleeps is the placement director's answer -- this file
//! chooses no coordinates. It used to, and the two answers to "where" walked
//! the pet back and forth between them (`docs/placement.md` 3.5).

use super::*;

impl PetRuntime {
    pub(super) fn update_rest_lifecycle(
        &mut self,
        user_idle_duration: f64,
        proximity: PointerProximity,
        situation: &PetSituation,
        intent: &PlacementIntent,
        now: f64,
        delta_time: f64,
    ) -> bool {
        if self.behavior.state().is_resting() && proximity != PointerProximity::Far {
            self.cancel_rest_for_activity(now);
            return false;
        }

        match self.behavior.state() {
            BehaviorState::Sit => {
                self.movement.cancel_route(false);
                self.movement.update_route(delta_time);
                if now - self.behavior.entered_at() >= SITTING_DURATION {
                    self.follow_rest_answer(situation, intent, now);
                }
                return true;
            }
            BehaviorState::FindSleepSpot => {
                self.movement
                    .set_maximum_speed(swift_max(24.0, self.tuning.walking_speed * 0.75));
                if self.movement.has_route() {
                    if self.movement.update_route(delta_time).reached_destination {
                        self.arrive_at_rest(situation, now);
                    }
                } else {
                    self.arrive_at_rest(situation, now);
                }
                return true;
            }
            BehaviorState::Sleep => {
                // Asleep beside an agent the answer is `SleepInPlace` for as
                // long as the seat is good; `Hold` is the director taking that
                // back. `None` is the pointer owning the pet, which says
                // nothing about the spot.
                let spot_is_gone = match intent {
                    PlacementIntent::NoRestSpot => true,
                    PlacementIntent::Hold => self.activity.is_watching_window(),
                    _ => false,
                };
                if spot_is_gone {
                    self.defer_rest(now);
                    return true;
                }
                self.movement.cancel_route(false);
                self.movement.update_route(delta_time);
                return true;
            }
            _ => {}
        }

        let may_nap_on_seat = *intent == PlacementIntent::SleepInPlace;
        // Watching an agent used to block rest outright, which meant the pet
        // could never sleep during the long unattended run that is exactly when
        // nobody is looking at it.
        let blocked = if user_idle_duration < self.tuning.idle_before_rest {
            "waiting for user idle".to_string()
        } else if self.placement.is_travelling() {
            "travelling".to_string()
        } else if self.activity.is_watching_window() && !may_nap_on_seat {
            "on duty, seat not nappable".to_string()
        } else if proximity != PointerProximity::Far {
            format!("pointer {}", proximity_name(proximity))
        } else if !self.behavior.state().allows_rest_entry() {
            format!("state {}", state_name(self.behavior.state()))
        } else {
            "clear to rest".to_string()
        };
        self.record("rest", &blocked);

        if !(user_idle_duration >= self.tuning.idle_before_rest
            && now >= self.rest_retry_at
            && !self.placement.is_travelling()
            && (!self.activity.is_watching_window() || may_nap_on_seat)
            && proximity == PointerProximity::Far
            && self.behavior.state().allows_rest_entry())
        {
            return false;
        }
        self.is_evade_transitioning = false;
        self.movement.cancel_route(false);
        self.behavior.handle(BehaviorInput::BeginRest, now);
        self.next_wander_at = f64::INFINITY;
        self.movement.update_route(delta_time);
        true
    }

    /// The sit is over and the director has said where to sleep.
    fn follow_rest_answer(&mut self, situation: &PetSituation, intent: &PlacementIntent, now: f64) {
        let beside_agent = self.activity.is_watching_window();
        match intent {
            PlacementIntent::SleepInPlace => {
                self.record_unvetted_rest(beside_agent);
                self.behavior.handle(BehaviorInput::SeekSleepSpot, now);
                self.enter_sleep(now);
            }
            PlacementIntent::RestAt(point) => {
                self.record_unvetted_rest(false);
                self.behavior.handle(BehaviorInput::SeekSleepSpot, now);
                let route = DisplayTopology::new(self.displays.clone())
                    .route(self.movement.position(), *point);
                self.movement
                    .set_maximum_speed(swift_max(24.0, self.tuning.walking_speed * 0.75));
                self.movement.set_route(route.waypoints);
                if !self.movement.has_route() {
                    self.arrive_at_rest(situation, now);
                }
            }
            PlacementIntent::NoRestSpot => {
                self.record_unvetted_rest(false);
                self.behavior.handle(BehaviorInput::SeekSleepSpot, now);
                self.defer_rest(now);
            }
            // The seat stopped being one to sleep on while the pet was sitting
            // down. It gets up and keeps watch rather than sitting for good.
            PlacementIntent::Hold if beside_agent => {
                self.behavior.handle(BehaviorInput::SeekSleepSpot, now);
                self.defer_rest(now);
            }
            // No answer this tick -- something else owns the pet. Keep sitting
            // and ask again.
            _ => {}
        }
    }

    /// Without a capture nothing was judged, and the log says which kind of
    /// unjudged it was. The wording is recorded in `RuntimeTrace.txt`.
    fn record_unvetted_rest(&mut self, on_agent_seat: bool) {
        if self.luminance.is_some() {
            return;
        }
        if on_agent_seat {
            self.record("rest", "sleeping in place, on a vetted seat");
        } else {
            self.record("rest", "tucking into a safe zone, spot unvetted");
        }
    }

    /// The route to bed has run out. The director judges the spot now rather
    /// than on the next tick's `decide`, so arriving and lying down stay one tick.
    fn arrive_at_rest(&mut self, situation: &PetSituation, now: f64) {
        let position = self.movement.position();
        match self.placement.rest_arrival(situation, position) {
            PlacementIntent::NoRestSpot => self.defer_rest(now),
            _ => self.enter_sleep(now),
        }
    }

    fn enter_sleep(&mut self, now: f64) {
        self.movement.cancel_route(true);
        self.behavior.handle(BehaviorInput::SleepSpotReached, now);
        self.next_wander_at = f64::INFINITY;
        self.persist_position = true;
    }

    fn defer_rest(&mut self, now: f64) {
        self.movement.cancel_route(true);
        self.behavior.handle(BehaviorInput::Reaction(CompanionReaction::Calm), now);
        self.rest_retry_at = now + 30.0;
        self.next_wander_at = now + WAKE_WANDER_DELAY;
        self.record("rest", "no clear sleep spot, staying awake");
    }

    pub(super) fn cancel_rest_for_activity(&mut self, now: f64) {
        if !self.behavior.state().is_resting() {
            return;
        }
        self.movement.cancel_route(false);
        self.behavior.handle(BehaviorInput::MeaningfulActivity, now);
        self.next_wander_at = now + WAKE_WANDER_DELAY;
    }
}
