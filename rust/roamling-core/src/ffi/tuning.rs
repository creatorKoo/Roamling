// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

use crate::tuning::RuntimeTuning;
use crate::tuning::TUNING_KEYS;

// ----------------------------------------------------------------- the knobs

#[derive(uniffi::Record)]
pub struct FfiTuning {
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

#[derive(uniffi::Record)]
pub struct FfiRange {
    pub lower: f64,
    pub upper: f64,
}

/// Shape A: a value in, a value out. Called when the panel moves a slider and
/// when a saved blob is decoded, not on the tick.
#[uniffi::export]
#[allow(clippy::too_many_arguments)]
pub fn normalize_tuning(
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
) -> FfiTuning {
    let tuning = RuntimeTuning::new(
        walking_speed,
        wander_pause,
        cross_display_wander_chance,
        pointer_awareness_distance,
        catch_arm_distance,
        catch_approach_speed,
        catch_window,
        hit_region_scale,
        gait_cadence,
        evade_speed_scale,
        idle_before_rest,
    );
    FfiTuning {
        walking_speed: tuning.walking_speed,
        wander_pause: tuning.wander_pause,
        cross_display_wander_chance: tuning.cross_display_wander_chance,
        pointer_awareness_distance: tuning.pointer_awareness_distance,
        catch_arm_distance: tuning.approach_distance,
        catch_approach_speed: tuning.approach_speed,
        catch_window: tuning.approach_hold,
        hit_region_scale: tuning.hit_region_scale,
        gait_cadence: tuning.gait_cadence,
        evade_speed_scale: tuning.evade_speed_scale,
        idle_before_rest: tuning.idle_before_rest,
    }
}

/// What the initialiser will clamp this key to, given the rest of the tuning.
/// One bound moves: arming a catch further away than the pet can notice is
/// meaningless, so it ends where awareness does.
#[uniffi::export]
pub fn tuning_limits(key: u8, pointer_awareness: f64) -> FfiRange {
    let (lower, upper) = RuntimeTuning::bounds(TUNING_KEYS[key as usize], pointer_awareness);
    FfiRange { lower, upper }
}

#[uniffi::export]
pub fn tuning_fast_evade_speed(walking_speed: f64, evade_speed_scale: f64) -> f64 {
    let mut tuning = RuntimeTuning::default();
    tuning.walking_speed = walking_speed;
    tuning.evade_speed_scale = evade_speed_scale;
    tuning.fast_evade_speed()
}

#[uniffi::export]
pub fn tuning_slow_evade_speed(walking_speed: f64, evade_speed_scale: f64) -> f64 {
    let mut tuning = RuntimeTuning::default();
    tuning.walking_speed = walking_speed;
    tuning.evade_speed_scale = evade_speed_scale;
    tuning.slow_evade_speed()
}

#[uniffi::export]
pub fn tuning_wander_delay(wander_pause: f64, random_unit: f64) -> f64 {
    let mut tuning = RuntimeTuning::default();
    tuning.wander_pause = wander_pause;
    tuning.wander_delay(random_unit)
}
