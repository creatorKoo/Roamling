// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

//! Going to sleep and staying asleep: choosing a spot, walking to it, checking
//! it is not on top of content, and giving it up when something else matters.

use super::*;

impl PetRuntime {
    pub(super) fn update_rest_lifecycle(
        &mut self,
        user_idle_duration: f64,
        proximity: PointerProximity,
        pointer: WorldPoint,
        may_nap_on_seat: bool,
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
                    self.behavior.handle(BehaviorInput::SeekSleepSpot, now);
                    self.begin_rest_travel(pointer, may_nap_on_seat, now);
                }
                return true;
            }
            BehaviorState::FindSleepSpot => {
                self.movement
                    .set_maximum_speed(swift_max(24.0, self.tuning.walking_speed * 0.75));
                if self.movement.has_route() {
                    if self.movement.update_route(delta_time).reached_destination {
                        self.enter_sleep(now);
                    }
                } else {
                    self.enter_sleep(now);
                }
                return true;
            }
            BehaviorState::Sleep => {
                if self.rest_spot_is_busy() {
                    self.defer_rest(now);
                    return true;
                }
                self.movement.cancel_route(false);
                self.movement.update_route(delta_time);
                return true;
            }
            _ => {}
        }

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
        self.rest_destination = None;
        self.movement.cancel_route(false);
        self.behavior.handle(BehaviorInput::BeginRest, now);
        self.next_wander_at = f64::INFINITY;
        self.movement.update_route(delta_time);
        true
    }

    pub(super) fn begin_rest_travel(&mut self, pointer: WorldPoint, nap_in_place: bool, now: f64) {
        let away_from_seam = self.world.display_containing(self.movement.position())
            .is_some_and(|display| placement_frame(display, &self.displays)
                .inset_by(self.object_size.width / 2.0, self.object_size.height / 2.0)
                .contains(self.movement.position()));
        // Without a field, retain the permission-free behaviour. A measured
        // field, however, judges every sleep spot including an agent's seat.
        if nap_in_place && away_from_seam && self.luminance.is_none() {
            self.record("rest", "sleeping in place, on a vetted seat");
            self.enter_sleep(now);
            return;
        }
        if self.luminance.is_none() {
            self.record("rest", "tucking into a safe zone, spot unvetted");
        }

        let mut rest_world = placement_world(&self.world);
        rest_world.safe_zones = BasicSafeZonePlanner::safe_zones(&rest_world);
        rest_world.focus = self.world.focus.clone();
        rest_world.luminance = self.luminance.clone();
        self.rest_destination = BasicSafeZonePlanner::clear_destination(
            &rest_world,
            self.movement.position(),
            Some(pointer),
            self.object_size,
        );

        if away_from_seam {
            if let Some(field) = self.luminance.as_ref() {
                let map = crate::clearance::ClearanceMap::new(field);
                let position = self.movement.position();
                if map.distance(position, self.object_size).is_some_and(|distance| distance >= 0.0)
                    && !self.rest_destination.as_ref().is_some_and(|destination| {
                        map.improves(position, destination.point, self.object_size)
                    })
                {
                    self.enter_sleep(now);
                    return;
                }
            }
        }

        let Some(destination) = self.rest_destination.clone() else {
            self.defer_rest(now);
            return;
        };
        if !self.is_roaming_enabled {
            self.enter_sleep(now);
            return;
        }
        let route = DisplayTopology::new(self.displays.clone())
            .route(self.movement.position(), destination.point);
        self.movement
            .set_maximum_speed(swift_max(24.0, self.tuning.walking_speed * 0.75));
        self.movement.set_route(route.waypoints);
        if !self.movement.has_route() {
            self.enter_sleep(now);
        }
    }

    fn enter_sleep(&mut self, now: f64) {
        if self.rest_spot_is_busy() {
            self.defer_rest(now);
            return;
        }
        self.movement.cancel_route(true);
        self.behavior.handle(BehaviorInput::SleepSpotReached, now);
        self.next_wander_at = f64::INFINITY;
        self.persist_position = true;
    }

    pub(super) fn rest_spot_is_busy(&mut self) -> bool {
        let position = self.movement.position();
        if let Some((point, size, busy)) = self.rest_content_check {
            if point == position && size == self.object_size {
                return busy;
            }
        }
        // Sleeping has no movement: only a new field, position or size needs
        // another content check, not every animation tick.
        let busy = self.luminance.as_ref().is_some_and(|field| {
            crate::clearance::ClearanceMap::new(field)
                .distance(position, self.object_size)
                .is_some_and(|distance| distance < 0.0)
        });
        self.rest_content_check = Some((position, self.object_size, busy));
        busy
    }

    fn defer_rest(&mut self, now: f64) {
        self.movement.cancel_route(true);
        self.rest_destination = None;
        self.behavior.handle(BehaviorInput::Reaction(CompanionReaction::Calm), now);
        self.rest_retry_at = now + 30.0;
        self.next_wander_at = now + WAKE_WANDER_DELAY;
        self.record("rest", "no clear sleep spot, staying awake");
    }

    pub(super) fn cancel_rest_for_activity(&mut self, now: f64) {
        if !self.behavior.state().is_resting() {
            return;
        }
        self.rest_destination = None;
        self.movement.cancel_route(false);
        self.behavior.handle(BehaviorInput::MeaningfulActivity, now);
        self.next_wander_at = now + WAKE_WANDER_DELAY;
    }
}
