// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

//! One native library contains both UniFFI components. Android owns the OS;
//! the existing PetLoop continues to own all behavior decisions.

use roamling_core::ffi::{normalize_tuning, FfiTuning};
use roamling_core::RuntimeTuning;

uniffi::setup_scaffolding!();
roamling_core::uniffi_reexport_scaffolding!();

/// Fetch defaults from the core instead of maintaining a Kotlin copy.
#[uniffi::export]
pub fn default_tuning() -> FfiTuning {
    let tuning = RuntimeTuning::default();
    normalize_tuning(
        tuning.walking_speed,
        tuning.wander_pause,
        tuning.cross_display_wander_chance,
        tuning.pointer_awareness_distance,
        tuning.catch_arm_distance,
        tuning.catch_approach_speed,
        tuning.catch_window,
        tuning.hit_region_scale,
        tuning.gait_cadence,
        tuning.evade_speed_scale,
        tuning.idle_before_rest,
    )
}
