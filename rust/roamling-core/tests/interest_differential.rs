// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

//! Unit 3b's gate: interest placement, across every combination of focus and
//! capture. A `scene` line sets the desktop the cases after it read.

use roamling_core::{
    BasicInterestPositionPlanner, DesktopWorldSnapshot, DisplaySnapshot, FocusSnapshot,
    LocationHint, LuminanceField, WorldPoint, WorldRect, WorldSize,
};

const NIL: f64 = -999.0;

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
    let frame = WorldRect::new(values[0], values[1], values[2], values[3]);
    let visible = WorldRect::new(values[4], values[5], values[6], values[7]);
    let region = WorldRect::new(values[8], values[9], values[10], values[11]);
    let hint = LocationHint::new(Some(region), values[12]);

    let focus = (values[13] == 1.0).then(|| {
        FocusSnapshot::new(
            optional_rect(&values[14..19]),
            optional_rect(&values[19..24]),
            optional_rect(&values[24..29]),
            values[29],
        )
    });

    let has_field = values[30] == 1.0;
    let columns = values[31] as usize;
    let rows = values[32] as usize;
    let field = has_field
        .then(|| LuminanceField::new(visible, columns, rows, values[33..].to_vec()))
        .flatten();

    let mut world = DesktopWorldSnapshot::new(
        vec![DisplaySnapshot {
            id: "d0".to_string(),
            name: "d0".to_string(),
            frame,
            visible_frame: visible,
            scale: 1.0,
        }],
        Vec::new(),
    );
    world.focus = focus;
    world.luminance = field;
    Scene { world, hint }
}

fn answer(op: &str, input: &[f64], scene: &Scene) -> Vec<f64> {
    match op {
        "interest.destination" => {
            let pointer = (input[2] == 1.0).then(|| WorldPoint::new(input[3], input[4]));
            match BasicInterestPositionPlanner::destination(
                &scene.hint,
                &scene.world,
                WorldPoint::new(input[0], input[1]),
                pointer,
                WorldSize::new(input[5], input[6]),
            ) {
                Some(destination) => {
                    vec![destination.point.x, destination.point.y, destination.score]
                }
                None => vec![NIL],
            }
        }
        "interest.evaluateSeat" => {
            let pointer = (input[4] == 1.0).then(|| WorldPoint::new(input[5], input[6]));
            match BasicInterestPositionPlanner::evaluate_seat(
                WorldPoint::new(input[0], input[1]),
                &scene.hint,
                &scene.world,
                WorldPoint::new(input[2], input[3]),
                pointer,
                WorldSize::new(input[7], input[8]),
            ) {
                Some(evaluation) => vec![
                    evaluation.score,
                    evaluation.emptiness.unwrap_or(NIL),
                    if evaluation.covers_caret { 1.0 } else { 0.0 },
                    if evaluation.watches_region { 1.0 } else { 0.0 },
                    if evaluation.is_holdable() { 1.0 } else { 0.0 },
                ],
                None => vec![NIL],
            }
        }
        other => panic!("fixture names an operation this port does not have: {other}"),
    }
}

#[test]
fn matches_the_swift_original_bit_for_bit() {
    let fixture = include_str!("fixtures/interest.txt");
    let mut scene: Option<Scene> = None;
    let mut checked = 0usize;
    let mut operations = std::collections::BTreeSet::new();

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

        let scene = scene.as_ref().expect("a scene precedes every case");
        let produced: Vec<u64> = answer(op, &input, scene)
            .into_iter()
            .map(f64::to_bits)
            .collect();
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

    assert!(checked > 1_000, "only {checked} cases -- the fixture shrank");
    assert_eq!(operations.len(), 2, "an operation lost its coverage");
}
