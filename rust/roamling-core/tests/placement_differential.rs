// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

//! Unit 5b's gate: the placement director.
//!
//! It holds a seat, a trip and the time of its last review across ticks, so the
//! fixture is a trajectory rather than a set of cases -- the pet actually walks
//! toward what it was told, which is the only way arrival, the seat dwell and
//! the review beat ever happen.
//!
//! The tail of the fixture is scripted rather than random. Escaping a covered
//! spot needs 2.5 uninterrupted seconds of parked dwell with somewhere clear to
//! run to, and abandoning a seat needs the screen to change under a pet that is
//! already sitting on it. Random search found one escape in 8,699 lines.

use roamling_core::{
    DesktopWorldSnapshot, DisplaySnapshot, FocusSnapshot, LocationHint, LuminanceField,
    PetSituation, PlacementConfiguration, PlacementDirector, PlacementIntent,
    PlacementTravelReason, WorldPoint, WorldRect, WorldSize,
};

const NIL: f64 = -999.0;

const REASONS: [PlacementTravelReason; 5] = [
    PlacementTravelReason::NewActivity,
    PlacementTravelReason::CoveringCaret,
    PlacementTravelReason::CoveringWork,
    PlacementTravelReason::PlannedBlind,
    PlacementTravelReason::FollowedFocus,
];

fn parse(fields: std::str::SplitWhitespace<'_>) -> Vec<f64> {
    fields
        .map(|field| f64::from_bits(u64::from_str_radix(field, 16).unwrap()))
        .collect()
}

/// Optional rects travel as a present flag followed by four numbers.
fn optional_rect(values: &[f64]) -> Option<WorldRect> {
    (values[0] == 1.0).then(|| WorldRect::new(values[1], values[2], values[3], values[4]))
}

struct Scene {
    world: DesktopWorldSnapshot,
    hint: LocationHint,
}

fn build_scene(values: &[f64]) -> Scene {
    let count = values[0] as usize;
    let mut displays = Vec::with_capacity(count);
    for index in 0..count {
        let base = 1 + index * 8;
        displays.push(DisplaySnapshot {
            id: format!("d{index}"),
            name: format!("d{index}"),
            frame: WorldRect::new(
                values[base],
                values[base + 1],
                values[base + 2],
                values[base + 3],
            ),
            visible_frame: WorldRect::new(
                values[base + 4],
                values[base + 5],
                values[base + 6],
                values[base + 7],
            ),
            scale: 1.0,
        });
    }

    let mut cursor = 1 + count * 8;
    let region = WorldRect::new(
        values[cursor],
        values[cursor + 1],
        values[cursor + 2],
        values[cursor + 3],
    );
    let hint = LocationHint::new(Some(region), values[cursor + 4]);
    let has_focus = values[cursor + 5] == 1.0;
    cursor += 6;

    let focus = has_focus.then(|| {
        FocusSnapshot::new(
            optional_rect(&values[cursor..cursor + 5]),
            optional_rect(&values[cursor + 5..cursor + 10]),
            optional_rect(&values[cursor + 10..cursor + 15]),
            values[cursor + 15],
        )
    });
    cursor += 16;

    let has_field = values[cursor] == 1.0;
    let bounds = WorldRect::new(
        values[cursor + 1],
        values[cursor + 2],
        values[cursor + 3],
        values[cursor + 4],
    );
    let columns = values[cursor + 5] as usize;
    let rows = values[cursor + 6] as usize;
    cursor += 7;
    let field = has_field
        .then(|| LuminanceField::new(bounds, columns, rows, values[cursor..].to_vec()))
        .flatten();

    let mut world = DesktopWorldSnapshot::new(displays, Vec::new());
    world.focus = focus;
    world.luminance = field;
    Scene { world, hint }
}

/// The intent, flattened to a fixed-width row so the comparison never has to
/// guess how many numbers a case produced.
fn encode(intent: &PlacementIntent) -> Vec<f64> {
    match intent {
        PlacementIntent::None => vec![0.0, NIL, NIL, NIL, NIL],
        PlacementIntent::Hold => vec![1.0, NIL, NIL, NIL, NIL],
        PlacementIntent::Travel(destination, reason) => vec![
            2.0,
            destination.point.x,
            destination.point.y,
            destination.score,
            REASONS.iter().position(|value| value == reason).unwrap() as f64,
        ],
        PlacementIntent::SleepInPlace => vec![3.0, NIL, NIL, NIL, NIL],
        PlacementIntent::Stroll(point) => vec![4.0, point.x, point.y, NIL, NIL],
        PlacementIntent::Escape(point) => vec![5.0, point.x, point.y, NIL, NIL],
    }
}

fn situation(input: &[f64], scene: &Scene) -> PetSituation {
    let count = input[21] as usize;
    let stroll_candidates = (0..count)
        .map(|slot| WorldPoint::new(input[22 + slot * 2], input[23 + slot * 2]))
        .collect();
    PetSituation {
        timestamp: input[0],
        world: scene.world.clone(),
        position: WorldPoint::new(input[1], input[2]),
        object_size: WorldSize::new(input[3], input[4]),
        pointer_position: (input[5] == 1.0).then(|| WorldPoint::new(input[6], input[7])),
        walking_speed: input[8],
        is_pointer_owned: input[9] == 1.0,
        is_pointer_watching: input[10] == 1.0,
        is_evading: input[11] == 1.0,
        is_walking: input[12] == 1.0,
        is_resting: input[13] == 1.0,
        activity_source_id: (input[14] == 1.0).then(|| format!("s{}", input[15] as i64)),
        activity_hint: (input[16] == 1.0).then(|| scene.hint.clone()),
        user_idle_duration: input[17],
        idle_before_rest: input[18],
        is_roaming_enabled: input[19] == 1.0,
        is_stroll_due: input[20] == 1.0,
        stroll_candidates,
    }
}

#[test]
fn matches_the_swift_original_bit_for_bit() {
    let fixture = include_str!("fixtures/placement.txt");
    let mut director = PlacementDirector::default();
    let mut scene: Option<Scene> = None;
    let mut checked = 0usize;
    let mut operations = std::collections::BTreeSet::new();
    let mut intents = std::collections::BTreeSet::new();
    let mut reasons = std::collections::BTreeSet::new();

    for (index, line) in fixture.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        if let Some(rest) = line.strip_prefix("scene ") {
            scene = Some(build_scene(&parse(rest.split_whitespace())));
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
            "dreset" => {
                let configuration = PlacementConfiguration::new(
                    input[0], input[1], input[2], input[3], input[4], input[5], input[6],
                    input[7],
                );
                director = PlacementDirector::new(configuration);
                vec![
                    configuration.hold_emptiness,
                    configuration.abandon_emptiness,
                    configuration.seat_dwell,
                    configuration.review_interval,
                    configuration.replacement_margin,
                    configuration.reseat_distance,
                    configuration.minimum_travel_distance,
                    configuration.arrival_tolerance,
                    director.is_seated() as u8 as f64,
                    director.is_travelling() as u8 as f64,
                ]
            }
            "decide" => {
                let scene = scene.as_ref().expect("a decide follows a scene");
                let intent = director.decide(&situation(&input, scene));
                intents.insert(encode(&intent)[0] as i64);
                if let Some(reason) = intent.travel_reason() {
                    reasons.insert(format!("{reason:?}"));
                }
                let mut out = encode(&intent);
                out.push(director.is_seated() as u8 as f64);
                out.push(director.is_travelling() as u8 as f64);
                out
            }
            "settle" => {
                let owner = (input[0] == 1.0).then(|| format!("s{}", input[1] as i64));
                director.settle_in_place(owner.as_deref(), input[2]);
                vec![
                    director.is_seated() as u8 as f64,
                    director.is_travelling() as u8 as f64,
                ]
            }
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

    assert!(checked > 2_800, "only {checked} cases -- the fixture shrank");
    assert_eq!(operations.len(), 3, "an operation lost its coverage");
    // Every answer the director can give, and every reason it can travel for.
    // Three of these only appear because the tail of the fixture puts them
    // there on purpose; losing that section would leave the rules that move a
    // pet off the user's work untested.
    assert_eq!(intents.len(), 6, "an intent lost its coverage: {intents:?}");
    assert_eq!(reasons.len(), 5, "a travel reason lost its coverage: {reasons:?}");
}
