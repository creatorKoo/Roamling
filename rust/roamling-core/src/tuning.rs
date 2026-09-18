// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

//! Ported from `Sources/RoamlingCore/RuntimeTuning.swift`, which moved to
//! `RoamlingEngine` in the same change -- Core cannot call the Rust seam, and
//! this type's clamping lives in its initialiser rather than at its callers.
//!
//! Every bound has one owner, and it is this file. A second table drifts: the
//! panel once offered the approach distance up to 140 while the model accepted 360.
//!
//! Since 2026-09-18 the three pointer values are named for what they do -- the
//! seated pet noticing a fast approach -- and no longer for the catch they once
//! gated. The names they are *stored* under did not move; see `storage_name`.

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
    ApproachDistance,
    ApproachSpeed,
    ApproachHold,
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
    RuntimeTuningKey::ApproachDistance,
    RuntimeTuningKey::ApproachSpeed,
    RuntimeTuningKey::ApproachHold,
    RuntimeTuningKey::HitRegionScale,
    RuntimeTuningKey::GaitCadence,
];

impl RuntimeTuningKey {
    /// The name each value is persisted under, on both platforms: the key
    /// inside macOS's `roamling.runtimeTuning` JSON blob and the suffix of
    /// Windows' `roamling.runtimeTuning.<name>` line.
    ///
    /// Tabulated rather than derived from the variant, deliberately. Windows
    /// used to build this from `{key:?}`, which meant the code name *was* the
    /// stored name -- and renaming a variant would have silently reset every
    /// value a user had tuned. Since 2026-09-18 the three pointer values are
    /// named for the approach reaction they drive while their stored names
    /// keep saying `catch`, so the two lists differ on purpose and a test pins
    /// this one.
    pub fn storage_name(self) -> &'static str {
        match self {
            RuntimeTuningKey::WalkingSpeed => "walkingSpeed",
            RuntimeTuningKey::WanderPause => "wanderPause",
            RuntimeTuningKey::CrossDisplayWanderChance => "crossDisplayWanderChance",
            RuntimeTuningKey::IdleBeforeRest => "idleBeforeRest",
            RuntimeTuningKey::PointerAwarenessDistance => "pointerAwarenessDistance",
            RuntimeTuningKey::EvadeSpeedScale => "evadeSpeedScale",
            RuntimeTuningKey::ApproachDistance => "catchArmDistance",
            RuntimeTuningKey::ApproachSpeed => "catchApproachSpeed",
            RuntimeTuningKey::ApproachHold => "catchWindow",
            RuntimeTuningKey::HitRegionScale => "hitRegionScale",
            RuntimeTuningKey::GaitCadence => "gaitCadence",
        }
    }
}

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
    pub approach_distance: f64,
    pub approach_speed: f64,
    pub approach_hold: f64,
    pub hit_region_scale: f64,
    pub gait_cadence: f64,
    pub evade_speed_scale: f64,
    pub idle_before_rest: f64,
}

impl RuntimeTuning {
    /// What `new` will clamp a value to, given the rest of this tuning.
    ///
    /// Takes the pointer awareness rather than reading a field, because one
    /// bound moves with it: reacting to an approach further away than the pet
    /// can notice is meaningless, so `ApproachDistance` ends where awareness does.
    pub fn bounds(key: RuntimeTuningKey, pointer_awareness: f64) -> (f64, f64) {
        match key {
            RuntimeTuningKey::WalkingSpeed => (20.0, 320.0),
            // The ceiling moved with the default: 40 used to be the top of
            // the track, so promoting it would have pinned the slider to the
            // right-hand end. 78 leaves as much room above the default as the
            // floor leaves below it, which is what the panel's centred-default
            // rule needs.
            RuntimeTuningKey::WanderPause => (2.0, 78.0),
            RuntimeTuningKey::CrossDisplayWanderChance => (0.0, 1.0),
            RuntimeTuningKey::IdleBeforeRest => (15.0, 600.0),
            RuntimeTuningKey::PointerAwarenessDistance => (140.0, 360.0),
            RuntimeTuningKey::EvadeSpeedScale => (0.8, 3.0),
            RuntimeTuningKey::ApproachDistance => (40.0, pointer_awareness),
            RuntimeTuningKey::ApproachSpeed => (150.0, 900.0),
            RuntimeTuningKey::ApproachHold => (0.15, 1.2),
            RuntimeTuningKey::HitRegionScale => (0.75, 1.3),
            RuntimeTuningKey::GaitCadence => (0.5, 3.2),
        }
    }

    /// Clamps in the order Swift's initialiser does. `approach_distance` reads
    /// the *assigned* awareness, not the argument, so reordering these two
    /// changes the answer whenever awareness arrives out of range.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        walking_speed: f64,
        wander_pause: f64,
        cross_display_wander_chance: f64,
        pointer_awareness_distance: f64,
        approach_distance: f64,
        approach_speed: f64,
        approach_hold: f64,
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
            approach_distance: bound(
                approach_distance,
                RuntimeTuningKey::ApproachDistance,
                pointer_awareness_distance,
            ),
            approach_speed: bound(
                approach_speed,
                RuntimeTuningKey::ApproachSpeed,
                0.0,
            ),
            approach_hold: bound(approach_hold, RuntimeTuningKey::ApproachHold, 0.0),
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
            RuntimeTuningKey::ApproachDistance => self.approach_distance,
            RuntimeTuningKey::ApproachSpeed => self.approach_speed,
            RuntimeTuningKey::ApproachHold => self.approach_hold,
            RuntimeTuningKey::HitRegionScale => self.hit_region_scale,
            RuntimeTuningKey::GaitCadence => self.gait_cadence,
        }
    }

    /// One value changed, and the whole thing re-clamped.
    ///
    /// It goes back through `new` rather than assigning the field, because the
    /// bounds are not independent: raising the approach distance past the notice
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
            RuntimeTuningKey::ApproachDistance => fields.approach_distance = value,
            RuntimeTuningKey::ApproachSpeed => fields.approach_speed = value,
            RuntimeTuningKey::ApproachHold => fields.approach_hold = value,
            RuntimeTuningKey::HitRegionScale => fields.hit_region_scale = value,
            RuntimeTuningKey::GaitCadence => fields.gait_cadence = value,
        }
        Self::new(
            fields.walking_speed,
            fields.wander_pause,
            fields.cross_display_wander_chance,
            fields.pointer_awareness_distance,
            fields.approach_distance,
            fields.approach_speed,
            fields.approach_hold,
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

    /// The two evade radii are fractions of the notice distance rather than
    /// constants. As constants they made the slider a lie: raising it only
    /// widened a `min` that already capped the evade radius at 100. The
    /// fractions are the shipped values over the shipped default, so at 170
    /// they are exactly 100 and 50 and the default pet is unchanged.
    /// Multiplied before dividing, to round once and to match Swift.
    pub fn pointer_configuration(&self) -> PointerInteractionConfiguration {
        PointerInteractionConfiguration::new(
            self.pointer_awareness_distance,
            self.pointer_awareness_distance * 100.0 / 170.0,
            self.pointer_awareness_distance * 50.0 / 170.0,
            self.approach_distance,
            self.slow_evade_speed(),
            self.fast_evade_speed(),
            self.approach_speed,
            swift_max(120.0, self.approach_speed * 0.48),
        )
    }

    /// How fast the authored walk cycle plays while travelling. Deliberately
    /// not derived from `walking_speed`: the authored gait reads correctly at
    /// its own cadence, and retiming it made the walk look busy.
    pub fn locomotion_animation_rate(&self) -> f64 {
        self.gait_cadence
    }

    /// A deterministic random mapping keeps the pacing testable while avoiding
    /// a metronomic pause. Standard tuning yields roughly 28...58 seconds.
    pub fn wander_delay(&self, random_unit: f64) -> f64 {
        self.wander_pause * (0.7 + clamped(random_unit, 0.0, 1.0) * 0.75)
    }
}

impl Default for RuntimeTuning {
    fn default() -> Self {
        Self::new(
            160.0,
            40.0,
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
    /// The stored names are a contract with every settings file already on
    /// disk and with the macOS blob, whose `Codable` keys these are.
    #[test]
    fn stored_names_are_the_ones_every_settings_file_already_uses() {
        let expected = [
            "walkingSpeed",
            "wanderPause",
            "crossDisplayWanderChance",
            "idleBeforeRest",
            "pointerAwarenessDistance",
            "evadeSpeedScale",
            "catchArmDistance",
            "catchApproachSpeed",
            "catchWindow",
            "hitRegionScale",
            "gaitCadence",
        ];
        let actual: Vec<&str> = TUNING_KEYS.iter().map(|key| key.storage_name()).collect();
        assert_eq!(actual, expected);
    }

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

    /// The one bound that is not independent: reacting to an approach further away than
    /// the pet can notice is meaningless, so lowering the notice distance has
    /// to pull the approach distance down with it.
    #[test]
    fn narrowing_awareness_pulls_the_approach_distance_in() {
        let wide = RuntimeTuning::default()
            .with(RuntimeTuningKey::PointerAwarenessDistance, 360.0)
            .with(RuntimeTuningKey::ApproachDistance, 300.0);
        assert_eq!(wide.get(RuntimeTuningKey::ApproachDistance), 300.0);

        let narrow = wide.with(RuntimeTuningKey::PointerAwarenessDistance, 140.0);
        assert_eq!(narrow.get(RuntimeTuningKey::ApproachDistance), 140.0);
    }
}
