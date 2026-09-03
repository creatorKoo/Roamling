// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

//! Ported from `Sources/RoamlingCore/MovementController.swift`.
//!
//! The first unit whose state lives between ticks rather than being handed in
//! with every call: the route, the cursor into it, and the velocity that
//! carries the pet along. Swift holds a handle to this and steps it.

use crate::geometry::{clamped, swift_max, swift_min, WorldPoint, WorldVector};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MovementConfiguration {
    pub maximum_speed: f64,
    pub acceleration: f64,
    pub deceleration: f64,
    pub arrival_radius: f64,
}

impl MovementConfiguration {
    /// The bounds are the initialiser's, not the field's. The runtime assigns
    /// `maximum_speed` straight from the tuned walking speed every tick and
    /// Swift does not re-clamp on assignment either, so neither does this --
    /// see `set_maximum_speed`.
    pub fn new(
        maximum_speed: f64,
        acceleration: f64,
        deceleration: f64,
        arrival_radius: f64,
    ) -> Self {
        Self {
            maximum_speed: swift_max(0.0, maximum_speed),
            acceleration: swift_max(0.0, acceleration),
            deceleration: swift_max(0.0, deceleration),
            arrival_radius: swift_max(0.1, arrival_radius),
        }
    }
}

impl Default for MovementConfiguration {
    fn default() -> Self {
        Self::new(48.0, 110.0, 130.0, 1.5)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MovementUpdate {
    pub position: WorldPoint,
    pub velocity: WorldVector,
    pub reached_destination: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MovementController {
    position: WorldPoint,
    velocity: WorldVector,
    configuration: MovementConfiguration,
    waypoints: Vec<WorldPoint>,
    waypoint_index: usize,
}

impl MovementController {
    pub fn new(
        position: WorldPoint,
        velocity: WorldVector,
        configuration: MovementConfiguration,
    ) -> Self {
        Self {
            position,
            velocity,
            configuration,
            waypoints: Vec::new(),
            waypoint_index: 0,
        }
    }

    pub fn position(&self) -> WorldPoint {
        self.position
    }

    pub fn velocity(&self) -> WorldVector {
        self.velocity
    }

    pub fn configuration(&self) -> MovementConfiguration {
        self.configuration
    }

    /// Field assignment, so it does not go through `MovementConfiguration::new`.
    /// Swift's `movement.configuration.maximumSpeed = ...` does not clamp, and a
    /// clamp here would be a behaviour change dressed as tidying.
    pub fn set_maximum_speed(&mut self, value: f64) {
        self.configuration.maximum_speed = value;
    }

    pub fn has_route(&self) -> bool {
        self.waypoint_index < self.waypoints.len()
    }

    pub fn destination(&self) -> Option<WorldPoint> {
        self.waypoints.last().copied()
    }

    pub fn current_waypoint(&self) -> Option<WorldPoint> {
        self.has_route().then(|| self.waypoints[self.waypoint_index])
    }

    pub fn set_route(&mut self, waypoints: Vec<WorldPoint>) {
        self.waypoints = waypoints;
        self.waypoint_index = 0;
        self.skip_reached_waypoints();
    }

    pub fn cancel_route(&mut self, stop: bool) {
        self.waypoints.clear();
        self.waypoint_index = 0;
        if stop {
            self.velocity = WorldVector::ZERO;
        }
    }

    pub fn teleport(&mut self, point: WorldPoint, stop: bool) {
        self.position = point;
        if stop {
            self.velocity = WorldVector::ZERO;
        }
    }

    pub fn set_velocity(&mut self, velocity: WorldVector) {
        self.velocity = velocity.limited(self.configuration.maximum_speed);
    }

    pub fn update(&mut self, raw_delta_time: f64) -> MovementUpdate {
        let delta_time = clamped(raw_delta_time, 0.0, 0.1);
        // Written as `!(x > 0)` so a NaN delta takes the same branch Swift's
        // `guard deltaTime > 0` sends it down.
        let target = match (delta_time > 0.0, self.current_waypoint()) {
            (true, Some(target)) => target,
            _ => {
                self.velocity = self.velocity.moved_toward(
                    WorldVector::ZERO,
                    self.configuration.deceleration * delta_time,
                );
                return self.report();
            }
        };

        let offset = target.vector_from(self.position);
        let distance = offset.length();
        if distance <= self.configuration.arrival_radius {
            self.arrive(target);
            return self.report();
        }

        let stopping_speed =
            swift_max(0.0, 2.0 * self.configuration.deceleration * distance).sqrt();
        let desired_speed = swift_min(self.configuration.maximum_speed, stopping_speed);
        let desired_velocity = offset.normalized().scaled(desired_speed);
        self.velocity = self.velocity.moved_toward(
            desired_velocity,
            self.configuration.acceleration * delta_time,
        );

        // Overshoot is detected by the sign of the dot product rather than by
        // distance: a step long enough to pass the waypoint flips it, and
        // snapping to the target there is what keeps the pet off a wobble.
        let previous_offset = target.vector_from(self.position);
        let proposed = self.position.offset(self.velocity.scaled(delta_time));
        let remaining_offset = target.vector_from(proposed);
        if previous_offset.dot(remaining_offset) <= 0.0 {
            self.arrive(target);
        } else {
            self.position = proposed;
        }

        self.report()
    }

    fn arrive(&mut self, target: WorldPoint) {
        self.position = target;
        self.waypoint_index += 1;
        self.skip_reached_waypoints();
        if !self.has_route() {
            self.velocity = WorldVector::ZERO;
        }
    }

    fn report(&self) -> MovementUpdate {
        MovementUpdate {
            position: self.position,
            velocity: self.velocity,
            reached_destination: !self.has_route(),
        }
    }

    fn skip_reached_waypoints(&mut self) {
        while self.waypoint_index < self.waypoints.len()
            && self.position.distance(self.waypoints[self.waypoint_index])
                <= self.configuration.arrival_radius
        {
            self.position = self.waypoints[self.waypoint_index];
            self.waypoint_index += 1;
        }
    }
}
