// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

//! Unit 3a's gate: luminance scoring, candidate ranking, and whether the pet
//! naps where it stands. A `field` line sets the luminance grid the cases after
//! it read.

use roamling_core::{
    BasicSafeZonePlanner, CandidatePositionScorer, LuminanceField, PositionCandidate,
    VisualEmptiness, WorldPoint, WorldRect, WorldSize,
};

/// Swift returns nil where this fixture writes -999; scores are `0...1` and
/// coordinates are bounded, so the sentinel cannot collide with an answer.
const NIL: f64 = -999.0;

fn parse(fields: std::str::SplitWhitespace<'_>) -> Vec<f64> {
    fields
        .map(|field| f64::from_bits(u64::from_str_radix(field, 16).unwrap()))
        .collect()
}

fn candidate(values: &[f64]) -> PositionCandidate {
    PositionCandidate {
        point: WorldPoint::new(values[0], values[1]),
        visual_empty_score: values[2],
        distance_from_caret: values[3],
        distance_from_controls: values[4],
        edge_preference: values[5],
        stability_score: values[6],
        context_preference: values[7],
        pet_comfort: values[8],
        pointer_proximity: values[9],
        obstruction_penalty: values[10],
    }
}

fn answer(op: &str, input: &[f64], field: Option<&LuminanceField>) -> Vec<f64> {
    match op {
        "emptiness.score" => {
            let rect = WorldRect::new(input[0], input[1], input[2], input[3]);
            vec![VisualEmptiness::score(rect, field.unwrap()).unwrap_or(NIL)]
        }
        "emptiness.firstComfortable" => {
            let points: Vec<WorldPoint> = input[0..10]
                .chunks_exact(2)
                .map(|pair| WorldPoint::new(pair[0], pair[1]))
                .collect();
            let size = WorldSize::new(input[10], input[11]);
            match VisualEmptiness::first_comfortable(&points, size, field.unwrap(), input[12]) {
                Some(point) => vec![point.x, point.y],
                None => vec![NIL],
            }
        }
        "safeZone.napsInPlace" => {
            let naps = BasicSafeZonePlanner::naps_in_place(
                WorldPoint::new(input[0], input[1]),
                WorldSize::new(input[2], input[3]),
                field,
                input[4],
            );
            vec![if naps { 1.0 } else { 0.0 }]
        }
        "scorer.best" => {
            let candidates: Vec<PositionCandidate> =
                input.chunks_exact(11).map(candidate).collect();
            match CandidatePositionScorer::best(&candidates) {
                Some(best) => vec![best.point.x, best.point.y, best.score()],
                None => vec![NIL],
            }
        }
        other => panic!("fixture names an operation this port does not have: {other}"),
    }
}

#[test]
fn matches_the_swift_original_bit_for_bit() {
    let fixture = include_str!("fixtures/emptiness.txt");
    let mut field: Option<LuminanceField> = None;
    let mut checked = 0usize;
    let mut operations = std::collections::BTreeSet::new();

    for (index, line) in fixture.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        if let Some(rest) = line.strip_prefix("field ") {
            let values = parse(rest.split_whitespace());
            let columns = values[4] as usize;
            let rows = values[5] as usize;
            field = LuminanceField::new(
                WorldRect::new(values[0], values[1], values[2], values[3]),
                columns,
                rows,
                values[6..].to_vec(),
            );
            assert!(field.is_some(), "line {}: the fixture's field is unusable", index + 1);
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

        let produced: Vec<u64> = answer(op, &input, field.as_ref())
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

    assert!(checked > 1_500, "only {checked} cases -- the fixture shrank");
    assert_eq!(operations.len(), 4, "an operation lost its coverage");
}
