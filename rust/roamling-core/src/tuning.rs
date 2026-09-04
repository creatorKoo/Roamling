// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

//! Ported from `Sources/RoamlingCore/RuntimeTuning.swift`, which moved to
//! `RoamlingEngine` in the same change -- Core cannot call the Rust seam, and
//! this type's clamping lives in its initialiser rather than at its callers.
//!
//! Every bound has one owner, and it is this file. A second table drifts: the
//! panel once offered `catchArmDistance` up to 140 while the model accepted 360.

use crate::geometry::{clamped, swift_max};
use crate::pointer::PointerInteractionConfiguration;

/// Declaration order is the wire order, and it is also the order the panel
/// lists the sliders in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeTuningKey {
    WalkingSpeed,
    WanderPause,
    CrossDisplayWanderChance,
    IdleBeforeRest,
    PointerAwarenessDistance,
    EvadeSpeedScale,
    CatchArmDistance,
    CatchApproachSpeed,
    CatchWindow,
    HitRegionScale,
    GaitCadence,
}

pub const TUNING_KEYS: [RuntimeTuningKey; 11] = [
    RuntimeTuningKey::WalkingSpeed,
    RuntimeTuningKey::WanderPause,
    RuntimeTuningKey::CrossDisplayWanderChance,
    RuntimeTuningKey::IdleBeforeRest,
    RuntimeTuningKey::PointerAwarenessDistance,
    RuntimeTuningKey::EvadeSpeedScale,
    RuntimeTuningKey::CatchArmDistance,
    RuntimeTuningKey::CatchApproachSpeed,
    RuntimeTuningKey::CatchWindow,
    RuntimeTuningKey::HitRegionScale,
    RuntimeTuningKey::GaitCadence,
];

/// The default `idleBeforeRest`, which Swift reads from
/// `RestConfiguration.standard`. Repeated here because Core owns that type and
/// this file no longer can; the Swift logic tests pin the two together.
pub const STANDARD_IDLE_BEFORE_REST: f64 = 75.0;

/// The intentionally small set of live-tunable values.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RuntimeTuning {
    pub walking_speed: f64,
    pub wander_pause: f64,
    pub cross_display_wander_chance: f64,
    pub pointer_awareness_distance: f64,
    pub catch_arm_distance: f64,
    pub catch_approach_speed: f64,
    pub catch_window: f64,
    pub hit_region_scale: f64,
    pub gait_cadence: f64,
    pub evade_speed_scale: f64,
    pub idle_before_rest: f64,
}

impl RuntimeTuning {
    /// What `new` will clamp a value to, given the rest of this tuning.
    ///
    /// Takes the pointer awareness rather than reading a field, because one
    /// bound moves with it: arming a catch further away than the pet can notice
    /// is meaningless, so `CatchArmDistance` ends where awareness does.
    pub fn bounds(key: RuntimeTuningKey, pointer_awareness: f64) -> (f64, f64) {
        match key {
            RuntimeTuningKey::WalkingSpeed => (20.0, 320.0),
            RuntimeTuningKey::WanderPause => (2.0, 40.0),
            RuntimeTuningKey::CrossDisplayWanderChance => (0.0, 1.0),
            RuntimeTuningKey::IdleBeforeRest => (15.0, 600.0),
            RuntimeTuningKey::PointerAwarenessDistance => (140.0, 360.0),
            RuntimeTuningKey::EvadeSpeedScale => (0.8, 3.0),
            RuntimeTuningKey::CatchArmDistance => (40.0, pointer_awareness),
            RuntimeTuningKey::CatchApproachSpeed => (150.0, 900.0),
            RuntimeTuningKey::CatchWindow => (0.15, 1.2),
            RuntimeTuningKey::HitRegionScale => (0.75, 1.3),
            RuntimeTuningKey::GaitCadence => (0.5, 3.2),
        }
    }

    /// Clamps in the order Swift's initialiser does. `catch_arm_distance` reads
    /// the *assigned* awareness, not the argument, so reordering these two
    /// changes the answer whenever awareness arrives out of range.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        walking_speed: f64,
        wander_pause: f64,
        cross_display_wander_chance: f64,
        pointer_awareness_distance: f64,
        catch_arm_distance: f64,
        catch_approach_speed: f64,
        catch_window: f64,
        hit_region_scale: f64,
        gait_cadence: f64,
        evade_speed_scale: f64,
        idle_before_rest: f64,
    ) -> Self {
        fn bound(value: f64, key: RuntimeTuningKey, awareness: f64) -> f64 {
            let (lower, upper) = RuntimeTuning::bounds(key, awareness);
            clamped(value, lower, upper)
        }
        let pointer_awareness_distance = bound(
            pointer_awareness_distance,
            RuntimeTuningKey::PointerAwarenessDistance,
            0.0,
        );
        Self {
            walking_speed: bound(walking_speed, RuntimeTuningKey::WalkingSpeed, 0.0),
            wander_pause: bound(wander_pause, RuntimeTuningKey::WanderPause, 0.0),
            cross_display_wander_chance: bound(
                cross_display_wander_chance,
                RuntimeTuningKey::CrossDisplayWanderChance,
                0.0,
            ),
            pointer_awareness_distance,
            catch_arm_distance: bound(
                catch_arm_distance,
                RuntimeTuningKey::CatchArmDistance,
                pointer_awareness_distance,
            ),
            catch_approach_speed: bound(
                catch_approach_speed,
                RuntimeTuningKey::CatchApproachSpeed,
                0.0,
            ),
            catch_window: bound(catch_window, RuntimeTuningKey::CatchWindow, 0.0),
            hit_region_scale: bound(hit_region_scale, RuntimeTuningKey::HitRegionScale, 0.0),
            gait_cadence: bound(gait_cadence, RuntimeTuningKey::GaitCadence, 0.0),
            evade_speed_scale: bound(evade_speed_scale, RuntimeTuningKey::EvadeSpeedScale, 0.0),
            idle_before_rest: bound(idle_before_rest, RuntimeTuningKey::IdleBeforeRest, 0.0),
        }
    }

    pub fn limits(&self, key: RuntimeTuningKey) -> (f64, f64) {
        Self::bounds(key, self.pointer_awareness_distance)
    }

    /// Swift reaches these through a subscript, which is what lets the panel be
    /// a list of rows rather than eleven near-identical blocks. Same idea here.
    pub fn get(&self, key: RuntimeTuningKey) -> f64 {
        match key {
            RuntimeTuningKey::WalkingSpeed => self.walking_speed,
            RuntimeTuningKey::WanderPause => self.wander_pause,
            RuntimeTuningKey::CrossDisplayWanderChance => self.cross_display_wander_chance,
            RuntimeTuningKey::IdleBeforeRest => self.idle_before_rest,
            RuntimeTuningKey::PointerAwarenessDistance => self.pointer_awareness_distance,
            RuntimeTuningKey::EvadeSpeedScale => self.evade_speed_scale,
            RuntimeTuningKey::CatchArmDistance => self.catch_arm_distance,
            RuntimeTuningKey::CatchApproachSpeed => self.catch_approach_speed,
            RuntimeTuningKey::CatchWindow => self.catch_window,
            RuntimeTuningKey::HitRegionScale => self.hit_region_scale,
            RuntimeTuningKey::GaitCadence => self.gait_cadence,
        }
    }

    /// One value changed, and the whole thing re-clamped.
    ///
    /// It goes back through `new` rather than assigning the field, because the
    /// bounds are not independent: raising the catch arm past the notice
    /// distance has to be caught, and only `new` knows that.
    #[must_use]
    pub fn with(&self, key: RuntimeTuningKey, value: f64) -> Self {
        let mut fields = *self;
        match key {
            RuntimeTuningKey::WalkingSpeed => fields.walking_speed = value,
            RuntimeTuningKey::WanderPause => fields.wander_pause = value,
            RuntimeTuningKey::CrossDisplayWanderChance => {
                fields.cross_display_wander_chance = value
            }
            RuntimeTuningKey::IdleBeforeRest => fields.idle_before_rest = value,
            RuntimeTuningKey::PointerAwarenessDistance => {
                fields.pointer_awareness_distance = value
            }
            RuntimeTuningKey::EvadeSpeedScale => fields.evade_speed_scale = value,
            RuntimeTuningKey::CatchArmDistance => fields.catch_arm_distance = value,
            RuntimeTuningKey::CatchApproachSpeed => fields.catch_approach_speed = value,
            RuntimeTuningKey::CatchWindow => fields.catch_window = value,
            RuntimeTuningKey::HitRegionScale => fields.hit_region_scale = value,
            RuntimeTuningKey::GaitCadence => fields.gait_cadence = value,
        }
        Self::new(
            fields.walking_speed,
            fields.wander_pause,
            fields.cross_display_wander_chance,
            fields.pointer_awareness_distance,
            fields.catch_arm_distance,
            fields.catch_approach_speed,
            fields.catch_window,
            fields.hit_region_scale,
            fields.gait_cadence,
            fields.evade_speed_scale,
            fields.idle_before_rest,
        )
    }

    /// Evading has to outrun strolling, so both evade speeds scale with the
    /// walking speed. The floor keeps evasion usable at the slowest walk; it
    /// only takes over below roughly 43 pt/s.
    pub fn fast_evade_speed(&self) -> f64 {
        swift_max(60.0, self.walking_speed * self.evade_speed_scale)
    }

    /// The gentle sidestep keeps the authored 74:138 relationship to the urgent
    /// one, so the two still read as different reactions.
    pub fn slow_evade_speed(&self) -> f64 {
        self.fast_evade_speed() * 0.55
    }

    pub fn pointer_configuration(&self) -> PointerInteractionConfiguration {
        PointerInteractionConfiguration::new(
            self.pointer_awareness_distance,
            100.0,
            50.0,
            self.catch_arm_distance,
            self.slow_evade_speed(),
            self.fast_evade_speed(),
            self.catch_approach_speed,
            swift_max(120.0, self.catch_approach_speed * 0.48),
        )
    }

    /// How fast the authored walk cycle plays while travelling. Deliberately
    /// not derived from `walking_speed`: the authored gait reads correctly at
    /// its own cadence, and retiming it made the walk look busy.
    pub fn locomotion_animation_rate(&self) -> f64 {
        self.gait_cadence
    }

    /// A deterministic random mapping keeps the pacing testable while avoiding
    /// a metronomic pause. Standard tuning yields roughly 8.4...17.4 seconds.
    pub fn wander_delay(&self, random_unit: f64) -> f64 {
        self.wander_pause * (0.7 + clamped(random_unit, 0.0, 1.0) * 0.75)
    }
}

impl Default for RuntimeTuning {
    fn default() -> Self {
        Self::new(
            160.0,
            12.0,
            0.46,
            170.0,
            74.0,
            380.0,
            0.35,
            1.12,
            1.0,
            1.4,
            STANDARD_IDLE_BEFORE_REST,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `get` and `with` have to name the same field for every key, or the panel
    /// silently edits a different slider than the one under the cursor. Eleven
    /// keys, eleven fields, and no compiler check that they line up.
    #[test]
    fn every_key_reads_back_what_it_wrote() {
        let tuning = RuntimeTuning::default();
        for key in TUNING_KEYS {
            let (lower, upper) = tuning.limits(key);
            // A value inside the bounds, so clamping cannot mask a mismatch.
            let wanted = lower + (upper - lower) * 0.25;
            let updated = tuning.with(key, wanted);
            assert_eq!(updated.get(key), wanted, "{key:?} did not read back");

            // And nothing else moved.
            for other in TUNING_KEYS {
                if other != key {
                    assert_eq!(
                        updated.get(other),
                        tuning.get(other),
                        "{key:?} also changed {other:?}"
                    );
                }
            }
        }
    }

    /// The one bound that is not independent: arming a catch further away than
    /// the pet can notice is meaningless, so lowering the notice distance has
    /// to pull the catch arm down with it.
    #[test]
    fn narrowing_awareness_pulls_the_catch_arm_in() {
        let wide = RuntimeTuning::default()
            .with(RuntimeTuningKey::PointerAwarenessDistance, 360.0)
            .with(RuntimeTuningKey::CatchArmDistance, 300.0);
        assert_eq!(wide.get(RuntimeTuningKey::CatchArmDistance), 300.0);

        let narrow = wide.with(RuntimeTuningKey::PointerAwarenessDistance, 140.0);
        assert_eq!(narrow.get(RuntimeTuningKey::CatchArmDistance), 140.0);
    }
}
