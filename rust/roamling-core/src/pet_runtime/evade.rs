// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

//! Getting out of the cursor's way, and easing back out of it.

use super::*;

impl PetRuntime {
    pub(super) fn apply_evade(&mut self, desired_velocity: WorldVector, delta_time: f64, now: f64) {
        let topology = DisplayTopology::new(self.displays.clone());
        if let Some(mut transition) = topology.evade_transition(
            self.movement.position(),
            desired_velocity,
            self.object_size,
            320.0,
            1.0,
        ) {
            if let Some(target) = transition.display_ids.last()
                .and_then(|id| self.displays.iter().find(|display| &display.id == id))
            {
                if let Some(destination) = transition.waypoints.last_mut() {
                    *destination = placement_frame(target, &self.displays)
                        .clamped_center(*destination, self.object_size);
                }
            }
            self.is_evade_transitioning = true;
            self.movement.set_maximum_speed(swift_max(
                self.tuning.walking_speed,
                desired_velocity.length(),
            ));
            self.movement.set_route(transition.waypoints);
            self.movement.update_route(delta_time);
            self.next_wander_at = now + 1.5;
            return;
        }

        self.movement.cancel_route(false);
        let mut velocity = desired_velocity;
        let position = self.movement.position();
        let current = self
            .world
            .display_containing(position)
            .or_else(|| self.world.nearest_display(position))
            .map(|display| display.visible_frame);
        if let Some(frame) = current {
            let safe = frame.inset_by(self.object_size.width / 2.0, self.object_size.height / 2.0);
            let proposed = position.offset(velocity.scaled(delta_time));
            if proposed.x < safe.min_x() || proposed.x > safe.max_x() {
                velocity.dx = 0.0;
            }
            if proposed.y < safe.min_y() || proposed.y > safe.max_y() {
                velocity.dy = 0.0;
            }
            if velocity.length() < 1.0 {
                // Pinned against an edge: go along it, away from where the
                // cursor is heading rather than into it.
                let pointer_y = self
                    .last_pointer_decision
                    .map_or(0.0, |decision| decision.kinematics.velocity.dy);
                velocity = WorldVector::new(
                    0.0,
                    if pointer_y >= 0.0 {
                        -desired_velocity.length()
                    } else {
                        desired_velocity.length()
                    },
                );
            }
            let constrained = safe.closest_point(position.offset(velocity.scaled(delta_time)));
            self.movement.teleport(constrained, false);
        } else {
            self.movement
                .teleport(position.offset(velocity.scaled(delta_time)), false);
        }
        self.movement.set_maximum_speed(swift_max(
            self.tuning.walking_speed,
            desired_velocity.length(),
        ));
        self.movement.set_velocity(velocity);
        self.next_wander_at = now + 1.0;
    }

    pub(super) fn update_evade_transition(&mut self, now: f64, delta_time: f64) {
        if !self.movement.has_route() {
            self.is_evade_transitioning = false;
            self.behavior
                .handle(BehaviorInput::Pointer(PointerProximity::Far), now);
            return;
        }
        self.movement.set_maximum_speed(swift_max(
            self.tuning.walking_speed,
            self.tuning.pointer_configuration().fast_evade_speed,
        ));
        if !self.movement.update_route(delta_time).reached_destination {
            return;
        }
        self.is_evade_transitioning = false;
        self.behavior
            .handle(BehaviorInput::Pointer(PointerProximity::Far), now);
        let roll = self.rng.unit();
        self.next_wander_at = now + swift_max(1.5, self.tuning.wander_delay(roll) * 0.35);
        self.persist_position = true;
    }
}
