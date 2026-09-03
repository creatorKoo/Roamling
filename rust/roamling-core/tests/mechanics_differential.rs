// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

//! Unit 5a's gate: the three models the tick loop steps every frame.
//!
//! All three carry state, so the fixture is a script rather than a set of
//! independent cases -- `mreset`, `preset` and `breset` start a fresh model and
//! every line after one of them depends on all the lines before it. A route
//! walked over two hundred ticks is the thing under test; a single `update`
//! proves almost nothing.

use roamling_core::{
    look_direction_degrees, BehaviorController, BehaviorInput, BehaviorState, CompanionReaction,
    MovementConfiguration, MovementController, PointerInteractionConfiguration,
    PointerInteractionModel, PointerProximity, WorldPoint, WorldVector, BEHAVIOR_STATES,
};

const NIL: f64 = -1.0;

const REACTIONS: [CompanionReaction; 9] = [
    CompanionReaction::Glance,
    CompanionReaction::Observe,
    CompanionReaction::Spark,
    CompanionReaction::Work,
    CompanionReaction::Paw,
    CompanionReaction::SmallCelebrate,
    CompanionReaction::LargeCelebrate,
    CompanionReaction::Sad,
    CompanionReaction::Calm,
];

const PROXIMITIES: [PointerProximity; 5] = [
    PointerProximity::Far,
    PointerProximity::Watching,
    PointerProximity::SlowEvade,
    PointerProximity::FastEvade,
    PointerProximity::Catchable,
];

fn parse(fields: std::str::SplitWhitespace<'_>) -> Vec<f64> {
    fields
        .map(|field| f64::from_bits(u64::from_str_radix(field, 16).unwrap()))
        .collect()
}

/// The one field in this fixture that is allowed to differ, and only by the
/// smallest amount a `f64` can differ by.
///
/// `look_direction_degrees` comes from `atan2`, which IEEE does not require to
/// be correctly rounded and which platforms therefore disagree on. Measured on
/// Windows 11 / x86_64 / MSVC against this fixture: 4 of 3,057 answers differ,
/// every one of them by exactly one unit in the last place.
///
/// It is exempted rather than replaced because there is no deterministic
/// stand-in worth having -- writing our own `atan2` is a worse risk than the
/// difference, and the `libm` crate measured *further* from macOS than the
/// platform's own. And the difference is unobservable: the angle is quantised
/// into sixteen directions before anything is drawn with it, and 0 of the 4
/// disagreements land in a different one.
///
/// Distances used to need this too. They no longer do -- `hypot` was dropped
/// for `(dx*dx + dy*dy).sqrt()`, which IEEE specifies exactly. `docs/windows.md`, W4.
fn within_one_ulp(lhs: f64, rhs: f64) -> bool {
    if lhs == rhs {
        return true;
    }
    if lhs.is_nan() || rhs.is_nan() || lhs.is_sign_negative() != rhs.is_sign_negative() {
        return false;
    }
    (lhs.to_bits() as i64 - rhs.to_bits() as i64).abs() <= 1
}

fn state_index(state: BehaviorState) -> f64 {
    BEHAVIOR_STATES
        .iter()
        .position(|candidate| *candidate == state)
        .expect("every state is in the wire order") as f64
}

#[test]
fn matches_the_swift_original_bit_for_bit() {
    let fixture = include_str!("fixtures/mechanics.txt");
    let mut movement = MovementController::new(
        WorldPoint::ZERO,
        WorldVector::ZERO,
        MovementConfiguration::default(),
    );
    let mut pointer = PointerInteractionModel::new(PointerInteractionConfiguration::default());
    let mut behavior = BehaviorController::default();
    let mut checked = 0usize;
    let mut tolerated = 0usize;
    let mut operations = std::collections::BTreeSet::new();

    for (index, line) in fixture.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let (lhs, rhs) = line.split_once('|').expect("every case has a | separator");
        let mut fields = lhs.split_whitespace();
        let op = fields.next().expect("every case names an operation");
        let input = parse(fields);
        let expected: Vec<u64> = rhs
            .split_whitespace()
            .map(|field| u64::from_str_radix(field, 16).unwrap())
            .collect();

        let produced: Vec<f64> = match op {
            // ------------------------------------------------------ movement
            "mreset" => {
                let configuration =
                    MovementConfiguration::new(input[4], input[5], input[6], input[7]);
                movement = MovementController::new(
                    WorldPoint::new(input[0], input[1]),
                    WorldVector::new(input[2], input[3]),
                    configuration,
                );
                vec![
                    configuration.maximum_speed,
                    configuration.acceleration,
                    configuration.deceleration,
                    configuration.arrival_radius,
                    movement.position().x,
                    movement.position().y,
                ]
            }
            "route" => {
                let count = input[0] as usize;
                let waypoints: Vec<WorldPoint> = (0..count)
                    .map(|slot| WorldPoint::new(input[1 + slot * 2], input[2 + slot * 2]))
                    .collect();
                movement.set_route(waypoints);
                let waypoint = movement.current_waypoint();
                let destination = movement.destination();
                vec![
                    movement.position().x,
                    movement.position().y,
                    movement.has_route() as u8 as f64,
                    waypoint.map_or(NIL, |point| point.x),
                    waypoint.map_or(NIL, |point| point.y),
                    destination.map_or(NIL, |point| point.x),
                    destination.map_or(NIL, |point| point.y),
                ]
            }
            "cancel" => {
                movement.cancel_route(input[0] == 1.0);
                vec![
                    movement.position().x,
                    movement.position().y,
                    movement.velocity().dx,
                    movement.velocity().dy,
                    movement.has_route() as u8 as f64,
                ]
            }
            "teleport" => {
                movement.teleport(WorldPoint::new(input[0], input[1]), input[2] == 1.0);
                vec![
                    movement.position().x,
                    movement.position().y,
                    movement.velocity().dx,
                    movement.velocity().dy,
                ]
            }
            "setvel" => {
                movement.set_velocity(WorldVector::new(input[0], input[1]));
                vec![movement.velocity().dx, movement.velocity().dy]
            }
            "speed" => {
                movement.set_maximum_speed(input[0]);
                vec![movement.configuration().maximum_speed]
            }
            "update" => {
                let update = movement.update(input[0]);
                vec![
                    update.position.x,
                    update.position.y,
                    update.velocity.dx,
                    update.velocity.dy,
                    update.reached_destination as u8 as f64,
                    movement.has_route() as u8 as f64,
                ]
            }
            // ------------------------------------------------------- pointer
            "preset" => {
                let configuration = PointerInteractionConfiguration::new(
                    input[0], input[1], input[2], input[3], input[4], input[5], input[6],
                    input[7],
                );
                pointer = PointerInteractionModel::new(configuration);
                vec![
                    configuration.awareness_distance,
                    configuration.slow_evade_distance,
                    configuration.fast_evade_distance,
                    configuration.catch_distance,
                    configuration.slow_evade_speed,
                    configuration.fast_evade_speed,
                    configuration.catch_pointer_speed,
                    configuration.catch_closing_speed,
                ]
            }
            "pclear" => {
                pointer.reset();
                vec![0.0]
            }
            "pointer" => {
                let decision = pointer.evaluate(
                    WorldPoint::new(input[0], input[1]),
                    WorldPoint::new(input[2], input[3]),
                    input[4],
                );
                vec![
                    PROXIMITIES
                        .iter()
                        .position(|candidate| *candidate == decision.proximity)
                        .unwrap() as f64,
                    decision.kinematics.velocity.dx,
                    decision.kinematics.velocity.dy,
                    decision.kinematics.speed,
                    decision.kinematics.distance_to_pet,
                    decision.kinematics.closing_speed,
                    decision.escape_velocity.dx,
                    decision.escape_velocity.dy,
                    decision.look_direction_degrees.unwrap_or(NIL),
                    decision.attention_rate,
                    decision.should_arm_catch() as u8 as f64,
                ]
            }
            // ------------------------------------------------------ behavior
            "breset" => {
                behavior = BehaviorController::new(BEHAVIOR_STATES[input[0] as usize], input[1]);
                vec![state_index(behavior.state()), behavior.entered_at()]
            }
            "behave" => {
                let argument = input[1] as usize;
                let event = match input[0] as usize {
                    0 => BehaviorInput::BeginWander,
                    1 => BehaviorInput::Arrived,
                    2 => BehaviorInput::BeginRest,
                    3 => BehaviorInput::SeekSleepSpot,
                    4 => BehaviorInput::SleepSpotReached,
                    5 => BehaviorInput::BeginStretch,
                    6 => BehaviorInput::BeginInterestTravel,
                    7 => BehaviorInput::Pointer(PROXIMITIES[argument]),
                    8 => BehaviorInput::CatchBegan,
                    9 => BehaviorInput::DragMoved,
                    10 => BehaviorInput::MouseReleased,
                    11 => BehaviorInput::Reaction(REACTIONS[argument]),
                    12 => BehaviorInput::MeaningfulActivity,
                    _ => BehaviorInput::Tick,
                };
                let transition = behavior.handle(event, input[2]);
                vec![
                    state_index(transition.from),
                    state_index(transition.to),
                    transition.changed as u8 as f64,
                    state_index(behavior.state()),
                    behavior.entered_at(),
                    behavior.state().is_resting() as u8 as f64,
                ]
            }
            other => panic!("fixture names an operation this port does not have: {other}"),
        };

        assert_eq!(
            produced.len(),
            expected.len(),
            "line {}: {op} produced {} values, the fixture has {}",
            index + 1,
            produced.len(),
            expected.len()
        );
        // Index 8 of a `pointer` case is the look angle, and only that.
        let tolerant: &[usize] = if op == "pointer" { &[8] } else { &[] };
        for (slot, (mine, theirs)) in produced.iter().zip(&expected).enumerate() {
            let theirs = f64::from_bits(*theirs);
            let exact = mine.to_bits() == theirs.to_bits();
            let agrees = if tolerant.contains(&slot) {
                let near = within_one_ulp(*mine, theirs);
                // Count the cases that only pass because of the exemption, so
                // the bound below can hold it to the size it was measured at.
                if near && !exact {
                    tolerated += 1;
                }
                near
            } else {
                exact
            };
            assert!(
                agrees,
                "line {}: {op} disagrees with Swift at field {slot}\n  swift {theirs}\n  rust  {mine}",
                index + 1,
            );
        }
        operations.insert(op.to_string());
        checked += 1;
    }

    assert!(checked > 10_000, "only {checked} cases -- the fixture shrank");
    assert_eq!(operations.len(), 12, "an operation lost its coverage");

    // The exemption above is meant to absorb a handful of platform libm
    // disagreements, not a systematic one. Measured: 0 on macOS/arm64 (the
    // fixture's own machine) and 4 of 3,057 on Windows/x86_64/MSVC. A porting
    // mistake that happened to land inside one unit in the last place would
    // show up here as hundreds, and would otherwise pass silently -- which is
    // the standing risk of any tolerance.
    //
    // Raising this bound is not the fix if it trips. Find out which platform,
    // which inputs, and whether the angles still quantise to the same frame.
    assert!(
        tolerated <= 16,
        "the one-ULP exemption absorbed {tolerated} disagreements; it was measured at 4"
    );
}

/// The look angle is a free function on both sides, so it gets its own check
/// rather than only being exercised through the model.
#[test]
fn a_pointer_on_top_of_the_pet_has_no_direction() {
    assert_eq!(
        look_direction_degrees(WorldPoint::new(10.0, 10.0), WorldPoint::new(10.0, 10.0)),
        None
    );
    assert_eq!(
        look_direction_degrees(WorldPoint::new(0.0, 0.0), WorldPoint::new(0.0, -10.0)),
        Some(0.0)
    );
    assert_eq!(
        look_direction_degrees(WorldPoint::new(0.0, 0.0), WorldPoint::new(10.0, 0.0)),
        Some(90.0)
    );
}
