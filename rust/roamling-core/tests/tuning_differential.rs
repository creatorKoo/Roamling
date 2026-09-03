// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

//! Unit 6a's gate: the tuning value and every bound in it.
//!
//! No state, so these are independent cases -- but the clamps are order
//! dependent, because `CatchArmDistance` is bounded by the *already clamped*
//! pointer awareness. Most of the draws are therefore out of range, which is
//! the only place that ordering is visible.

use roamling_core::{RuntimeTuning, RuntimeTuningKey, TUNING_KEYS};

fn parse(fields: std::str::SplitWhitespace<'_>) -> Vec<f64> {
    fields
        .map(|field| f64::from_bits(u64::from_str_radix(field, 16).unwrap()))
        .collect()
}

fn build(input: &[f64]) -> RuntimeTuning {
    RuntimeTuning::new(
        input[0], input[1], input[2], input[3], input[4], input[5], input[6], input[7],
        input[8], input[9], input[10],
    )
}

fn fields(tuning: &RuntimeTuning) -> Vec<f64> {
    vec![
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
    ]
}

#[test]
fn matches_the_swift_original_bit_for_bit() {
    let fixture = include_str!("fixtures/tuning.txt");
    let mut current = RuntimeTuning::default();
    let mut checked = 0usize;
    let mut operations = std::collections::BTreeSet::new();
    let mut keys = std::collections::BTreeSet::new();

    for (index, line) in fixture.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let (lhs, rhs) = line.split_once('|').expect("every case has a | separator");
        let mut names = lhs.split_whitespace();
        let op = names.next().expect("every case names an operation");
        let input = parse(names);
        let expected: Vec<u64> = rhs
            .split_whitespace()
            .map(|field| u64::from_str_radix(field, 16).unwrap())
            .collect();

        let produced: Vec<f64> = match op {
            "tuning" => {
                current = build(&input);
                let pointer = current.pointer_configuration();
                let mut out = fields(&current);
                out.extend([
                    current.fast_evade_speed(),
                    current.slow_evade_speed(),
                    pointer.awareness_distance,
                    pointer.slow_evade_distance,
                    pointer.fast_evade_distance,
                    pointer.catch_distance,
                    pointer.slow_evade_speed,
                    pointer.fast_evade_speed,
                    pointer.catch_pointer_speed,
                    pointer.catch_closing_speed,
                    current.locomotion_animation_rate(),
                    // `restConfiguration` re-clamps to 10...3600 on the Swift
                    // side, which is a no-op over the 15...600 this already
                    // enforces. Carried so that stops being true loudly.
                    current.idle_before_rest,
                ]);
                out
            }
            "delay" => vec![current.wander_delay(input[0])],
            "limits" => {
                let key = TUNING_KEYS[input[0] as usize];
                keys.insert(input[0] as i64);
                let (lower, upper) = current.limits(key);
                vec![lower, upper]
            }
            // Re-applying every bound to a value that has already been through
            // them has to be a no-op, or the panel drifts on every edit.
            "renorm" => fields(&build(&input)),
            "standard" => fields(&RuntimeTuning::default()),
            other => panic!("fixture names an operation this port does not have: {other}"),
        };

        let produced: Vec<u64> = produced.into_iter().map(f64::to_bits).collect();
        assert_eq!(
            produced,
            expected,
            "line {}: {op} disagrees with Swift\n  swift {:?}\n  rust  {:?}",
            index + 1,
            expected.iter().map(|b| f64::from_bits(*b)).collect::<Vec<_>>(),
            produced.iter().map(|b| f64::from_bits(*b)).collect::<Vec<_>>(),
        );
        operations.insert(op.to_string());
        checked += 1;
    }

    assert!(checked > 13_000, "only {checked} cases -- the fixture shrank");
    assert_eq!(operations.len(), 5, "an operation lost its coverage");
    assert_eq!(keys.len(), 11, "a tuning key lost its coverage");
}

/// One bound moves with another field, and that is the whole reason `limits`
/// takes an instance. The panel guessed a fixed ceiling of 140 while the model
/// accepted up to 360, and the slider could not reach values the pet obeyed.
#[test]
fn the_catch_radius_ends_where_awareness_does() {
    for awareness in [140.0, 220.0, 360.0] {
        let tuning = RuntimeTuning::new(
            160.0, 12.0, 0.46, awareness, 1_000.0, 380.0, 0.35, 1.12, 1.0, 1.4, 75.0,
        );
        assert_eq!(tuning.catch_arm_distance, awareness);
        assert_eq!(
            tuning.limits(RuntimeTuningKey::CatchArmDistance),
            (40.0, awareness)
        );
    }
}
