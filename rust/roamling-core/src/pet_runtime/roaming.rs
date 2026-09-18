// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

//! Strolls: when one is due, where it could go, and the candidates offered to
//! the placement decision.

use super::*;

impl PetRuntime {
    /// Walks whatever route roaming already has and paces the next pause.
    /// Choosing where to stroll is the director's job, not this one.
    pub(super) fn update_roaming(&mut self, now: f64, delta_time: f64) {
        self.movement.set_maximum_speed(self.tuning.walking_speed);
        if !self.is_roaming_enabled {
            self.movement.cancel_route(false);
            self.movement.update_route(delta_time);
            return;
        }
        if !self.movement.has_route() {
            self.movement.update_route(delta_time);
            return;
        }
        if self.movement.update_route(delta_time).reached_destination {
            self.behavior.handle(BehaviorInput::Arrived, now);
            let roll = self.rng.unit();
            self.next_wander_at = now + self.tuning.wander_delay(roll);
            self.persist_position = true;
        }
    }

    pub(super) fn begin_stroll(&mut self, point: WorldPoint, now: f64, delta_time: f64) {
        self.is_evade_transitioning = false;
        let route =
            DisplayTopology::new(self.displays.clone()).route(self.movement.position(), point);
        self.movement.set_maximum_speed(self.tuning.walking_speed);
        self.movement.set_route(route.waypoints);

        // A destination the pet is already standing on leaves nothing to walk:
        // `set_route` hands the single waypoint straight to its arrival check
        // and it is consumed. Claiming the walking state before finding that
        // out is what put the pet in the walk frames going nowhere, once every
        // two seconds until a different point came up -- roughly six seconds of
        // running on the spot in the session this was found in.
        if !self.movement.has_route() {
            self.next_wander_at = now + 2.0;
            self.movement.update_route(delta_time);
            return;
        }

        // Only now is the state claimed. The route is laid first because the
        // question "is there anywhere to go" cannot be answered without it, and
        // it is cancelled again if the state machine refuses -- otherwise the
        // pet walks the whole leg wearing the idle frames, which is the fault
        // this ordering was reversed to fix in the first place.
        if self.behavior.handle(BehaviorInput::BeginWander, now).to != BehaviorState::Wander {
            self.movement.cancel_route(false);
            self.next_wander_at = now + 2.0;
            self.movement.update_route(delta_time);
            return;
        }
        self.movement.update_route(delta_time);
    }

    /// Offering the director a handful of destinations to reject is the
    /// cheapest way to keep an aimless walk off the user's text without making
    /// roaming look calculated.
    pub(super) fn stroll_candidates(&mut self) -> Vec<WorldPoint> {
        let position = self.movement.position();
        let radius = self.movement.configuration().arrival_radius;
        // Somewhere the pet already stands is not a destination. It comes up
        // because `random_wander_point` ends in a clamp: every target past the
        // safe area folds onto the same edge, so a pet parked on that edge is
        // handed its own position back. Filtered after the draw, not instead of
        // it -- the sampler's random draws stay in the same order.
        (0..WANDER_CANDIDATE_COUNT)
            .filter_map(|_| self.random_wander_point())
            .filter(|candidate| position.distance(*candidate) > radius)
            .collect()
    }

    pub(super) fn random_wander_point(&mut self) -> Option<WorldPoint> {
        if self.displays.is_empty() {
            return None;
        }
        let position = self.movement.position();
        let current_id = self
            .world
            .display_containing(position)
            .or_else(|| self.world.nearest_display(position))
            .map(|display| display.id.clone());
        // Short-circuits exactly as Swift's `&&` does: a single display draws
        // nothing at all, and a draw taken here would shift every draw after it.
        let should_explore =
            self.displays.len() > 1 && self.rng.unit() < self.tuning.cross_display_wander_chance;
        let target = if should_explore {
            let alternatives: Vec<DisplaySnapshot> = self
                .displays
                .iter()
                .filter(|display| Some(&display.id) != current_id.as_ref())
                .cloned()
                .collect();
            if alternatives.is_empty() {
                self.displays[0].clone()
            } else {
                let index = self.rng.index(alternatives.len());
                alternatives[index].clone()
            }
        } else {
            match current_id
                .as_ref()
                .and_then(|id| self.displays.iter().find(|display| &display.id == id))
            {
                Some(display) => display.clone(),
                None => {
                    let index = self.rng.index(self.displays.len());
                    self.displays[index].clone()
                }
            }
        };

        let safe = placement_frame(&target, &self.displays).inset_by(
            self.object_size.width / 2.0 + 18.0,
            self.object_size.height / 2.0 + 12.0,
        );
        if safe.is_empty() {
            return Some(target.visible_frame.center());
        }

        // A cross-display trip ends shortly inside the destination display.
        // Crossing the seam reads clearly, while avoiding another full-screen
        // trek before Roamling finally pauses.
        if Some(&target.id) != current_id.as_ref() {
            let boundary = target.visible_frame.closest_point(position);
            let inward = target
                .visible_frame
                .center()
                .vector_from(boundary)
                .normalized();
            let depth = 140.0 + self.rng.unit() * 220.0;
            return Some(safe.closest_point(boundary.offset(inward.scaled(depth))));
        }

        let x = safe.min_x() + self.rng.unit() * safe.size.width;
        let y = if self.rng.unit() < 0.72 {
            let upper = swift_max(
                safe.min_y(),
                safe.max_y() - swift_min(170.0, safe.size.height * 0.32),
            );
            upper + self.rng.unit() * (safe.max_y() - upper)
        } else {
            safe.min_y() + self.rng.unit() * safe.size.height
        };
        let sampled = WorldPoint::new(x, y);
        let offset = sampled.vector_from(position);
        if !(offset.length() > 520.0) {
            return Some(sampled);
        }
        let leg = 280.0 + self.rng.unit() * 240.0;
        Some(safe.closest_point(position.offset(offset.normalized().scaled(leg))))
    }
}
