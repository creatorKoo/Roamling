// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

//! Ported from `Sources/RoamlingCore/PointerInteraction.swift`.
//!
//! Stateful for one reason: closing speed. Where the cursor is says almost
//! nothing on its own -- a hand resting near the pet and a hand grabbing for it
//! are the same point -- so the model keeps the previous sample and reads the
//! difference.

use crate::geometry::{clamped, swift_max, swift_min, WorldPoint, WorldVector};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointerProximity {
    Far,
    Watching,
    SlowEvade,
    FastEvade,
    Catchable,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PointerInteractionConfiguration {
    pub awareness_distance: f64,
    pub slow_evade_distance: f64,
    pub fast_evade_distance: f64,
    pub catch_distance: f64,
    pub slow_evade_speed: f64,
    pub fast_evade_speed: f64,
    pub catch_pointer_speed: f64,
    pub catch_closing_speed: f64,
}

impl PointerInteractionConfiguration {
    /// Every bound reads the *arguments*, not the fields assigned just above
    /// it. Swift's initialiser parameters shadow the properties, so
    /// `min(fastEvadeDistance, slowEvadeDistance)` compares the two values that
    /// were passed in -- a detail worth spelling out, because ordering the
    /// assignments differently here would quietly change the result.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        awareness_distance: f64,
        slow_evade_distance: f64,
        fast_evade_distance: f64,
        catch_distance: f64,
        slow_evade_speed: f64,
        fast_evade_speed: f64,
        catch_pointer_speed: f64,
        catch_closing_speed: f64,
    ) -> Self {
        Self {
            awareness_distance,
            slow_evade_distance: swift_min(slow_evade_distance, awareness_distance),
            fast_evade_distance: swift_min(fast_evade_distance, slow_evade_distance),
            // A fast approach should arm interaction before the pointer reaches
            // the sprite. It may therefore be wider than the fast-evade radius.
            catch_distance: clamped(catch_distance, 0.0, awareness_distance),
            slow_evade_speed: swift_max(0.0, slow_evade_speed),
            fast_evade_speed: swift_max(slow_evade_speed, fast_evade_speed),
            catch_pointer_speed: swift_max(0.0, catch_pointer_speed),
            catch_closing_speed: swift_max(0.0, catch_closing_speed),
        }
    }

    /// Unhurried at the edge of awareness, twice as quick within catching
    /// range, and continuous in between.
    pub fn attention_rate(&self, distance: f64) -> f64 {
        let span = self.awareness_distance - self.catch_distance;
        if !(span > 0.0) {
            return 1.0;
        }
        1.0 + clamped((self.awareness_distance - distance) / span, 0.0, 1.0)
    }
}

impl Default for PointerInteractionConfiguration {
    fn default() -> Self {
        Self::new(170.0, 100.0, 50.0, 74.0, 74.0, 138.0, 380.0, 182.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PointerKinematics {
    pub velocity: WorldVector,
    pub speed: f64,
    pub distance_to_pet: f64,
    pub closing_speed: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PointerDecision {
    pub proximity: PointerProximity,
    pub kinematics: PointerKinematics,
    pub escape_velocity: WorldVector,
    pub look_direction_degrees: Option<f64>,
    pub attention_rate: f64,
}

impl PointerDecision {
    pub fn should_arm_catch(&self) -> bool {
        self.proximity == PointerProximity::Catchable
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct PointerInteractionModel {
    configuration: PointerInteractionConfiguration,
    previous_pointer: Option<WorldPoint>,
    previous_distance: Option<f64>,
    previous_timestamp: Option<f64>,
}

impl PointerInteractionModel {
    pub fn new(configuration: PointerInteractionConfiguration) -> Self {
        Self {
            configuration,
            previous_pointer: None,
            previous_distance: None,
            previous_timestamp: None,
        }
    }

    pub fn configuration(&self) -> PointerInteractionConfiguration {
        self.configuration
    }

    pub fn set_configuration(&mut self, configuration: PointerInteractionConfiguration) {
        self.configuration = configuration;
    }

    /// Forgets the previous sample, so the next reading reports no motion at
    /// all rather than a jump. The runtime calls this whenever tuning changes
    /// under it.
    pub fn reset(&mut self) {
        self.previous_pointer = None;
        self.previous_distance = None;
        self.previous_timestamp = None;
    }

    pub fn evaluate(
        &mut self,
        pointer: WorldPoint,
        pet: WorldPoint,
        timestamp: f64,
    ) -> PointerDecision {
        let distance = pointer.distance(pet);
        let raw_delta = self.previous_timestamp.map_or(0.0, |previous| timestamp - previous);
        let delta_time = if raw_delta > 0.0 {
            swift_min(raw_delta, 0.25)
        } else {
            0.0
        };
        let pointer_velocity = match self.previous_pointer {
            Some(previous) if delta_time > 0.0 => {
                pointer.vector_from(previous).divided_by(delta_time)
            }
            _ => WorldVector::ZERO,
        };
        let closing_speed = match self.previous_distance {
            Some(previous) if delta_time > 0.0 => {
                swift_max(0.0, (previous - distance) / delta_time)
            }
            _ => 0.0,
        };

        self.previous_pointer = Some(pointer);
        self.previous_distance = Some(distance);
        self.previous_timestamp = Some(timestamp);

        let kinematics = PointerKinematics {
            velocity: pointer_velocity,
            speed: pointer_velocity.length(),
            distance_to_pet: distance,
            closing_speed,
        };

        let proximity = if distance <= self.configuration.catch_distance
            && kinematics.speed >= self.configuration.catch_pointer_speed
            && closing_speed >= self.configuration.catch_closing_speed
        {
            PointerProximity::Catchable
        } else if distance <= self.configuration.fast_evade_distance {
            PointerProximity::FastEvade
        } else if distance <= self.configuration.slow_evade_distance {
            PointerProximity::SlowEvade
        } else if distance <= self.configuration.awareness_distance {
            PointerProximity::Watching
        } else {
            PointerProximity::Far
        };

        // Straight away from the cursor; failing that, away from where the
        // cursor is heading; failing that, rightward, because standing still
        // while being pushed is the one answer that reads as broken.
        let mut away = pet.vector_from(pointer).normalized();
        if away.length() < 0.001 {
            away = pointer_velocity.negated().normalized();
        }
        if away.length() < 0.001 {
            away = WorldVector::new(1.0, 0.0);
        }

        let escape_speed = match proximity {
            PointerProximity::SlowEvade => self.configuration.slow_evade_speed,
            PointerProximity::FastEvade => self.configuration.fast_evade_speed,
            _ => 0.0,
        };

        PointerDecision {
            proximity,
            kinematics,
            escape_velocity: away.scaled(escape_speed),
            look_direction_degrees: look_direction_degrees(pet, pointer),
            attention_rate: self.configuration.attention_rate(distance),
        }
    }
}

/// Codex v2 convention: 0 degrees up, 90 screen-right, 180 down.
pub fn look_direction_degrees(pet: WorldPoint, pointer: WorldPoint) -> Option<f64> {
    let vector = pointer.vector_from(pet);
    if !(vector.length() > 0.001) {
        return None;
    }
    let degrees = vector.dx.atan2(-vector.dy) * 180.0 / std::f64::consts::PI;
    Some(if degrees >= 0.0 { degrees } else { degrees + 360.0 })
}
