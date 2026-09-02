// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

//! The port's gate: every case here was answered by the Swift original, and
//! this asserts Rust answers it with the identical bits.
//!
//! Bit-exact rather than approximate on purpose. A port that is merely close
//! drifts -- W0m.2 only matched once it reproduced the fact that Swift's
//! `max(by:)` keeps the last of equal elements, and no tolerance would have
//! caught that.
//!
//! Regenerate with `output/w-unit1/gen` while the Swift original still exists.

use roamling_core::{
    clamped, swift_max, swift_min, DesktopCoordinateSpace, WorldPoint, WorldRect, WorldSize,
    WorldVector,
};

fn bits(value: f64) -> u64 {
    value.to_bits()
}

fn point(values: &[f64], at: usize) -> WorldPoint {
    WorldPoint::new(values[at], values[at + 1])
}

fn vector(values: &[f64], at: usize) -> WorldVector {
    WorldVector::new(values[at], values[at + 1])
}

fn rect(values: &[f64], at: usize) -> WorldRect {
    WorldRect::new(values[at], values[at + 1], values[at + 2], values[at + 3])
}

fn flag(condition: bool) -> Vec<f64> {
    vec![if condition { 1.0 } else { 0.0 }]
}

fn answer(op: &str, input: &[f64]) -> Vec<f64> {
    match op {
        "point.distance" => vec![point(input, 0).distance(point(input, 2))],
        "point.plusVector" => {
            let p = point(input, 0).offset(vector(input, 2));
            vec![p.x, p.y]
        }
        "point.minusPoint" => {
            let v = point(input, 0).vector_from(point(input, 2));
            vec![v.dx, v.dy]
        }
        "vector.length" => vec![vector(input, 0).length()],
        "vector.normalized" => {
            let v = vector(input, 0).normalized();
            vec![v.dx, v.dy]
        }
        "vector.dot" => vec![vector(input, 0).dot(vector(input, 2))],
        "vector.limited" => {
            let v = vector(input, 0).limited(input[2]);
            vec![v.dx, v.dy]
        }
        "vector.movedToward" => {
            let v = vector(input, 0).moved_toward(vector(input, 2), input[4]);
            vec![v.dx, v.dy]
        }
        "vector.dividedBy" => {
            let v = vector(input, 0).divided_by(input[2]);
            vec![v.dx, v.dy]
        }
        "rect.edges" => {
            let r = rect(input, 0);
            vec![r.min_x(), r.min_y(), r.max_x(), r.max_y(), r.mid_x(), r.mid_y()]
        }
        "rect.contains" => flag(rect(input, 0).contains(point(input, 4))),
        "rect.intersects" => flag(rect(input, 0).intersects(&rect(input, 4), input[8])),
        "rect.insetBy" => {
            let r = rect(input, 0).inset_by(input[4], input[5]);
            vec![r.origin.x, r.origin.y, r.size.width, r.size.height]
        }
        "rect.closestPoint" => {
            let p = rect(input, 0).closest_point(point(input, 4));
            vec![p.x, p.y]
        }
        "rect.distance" => vec![rect(input, 0).distance(point(input, 4))],
        "rect.clampedCenter" => {
            let p = rect(input, 0)
                .clamped_center(point(input, 4), WorldSize::new(input[6], input[7]));
            vec![p.x, p.y]
        }
        "rect.union" => {
            let r = rect(input, 0).union(&rect(input, 4));
            vec![r.origin.x, r.origin.y, r.size.width, r.size.height]
        }
        "double.clamped" => {
            // Built the way the generator built the range, which is Swift's
            // min and max -- not Rust's, whose ±0 result is unspecified.
            let (lower, upper) = (swift_min(input[1], input[2]), swift_max(input[1], input[2]));
            vec![clamped(input[0], lower, upper)]
        }
        "double.clampedCrossed" => vec![clamped(input[0], input[1], input[2])],
        "space.fromFrames" => {
            let frames = [rect(input, 0), rect(input, 4)];
            vec![DesktopCoordinateSpace::from_host_frames(&frames).world_top]
        }
        "space.fromFrames.empty" => {
            vec![DesktopCoordinateSpace::from_host_frames(&[]).world_top]
        }
        "space.pointFrom" => {
            let p = DesktopCoordinateSpace::new(input[0]).point_from_host(point(input, 1));
            vec![p.x, p.y]
        }
        "space.rectFrom" => {
            let r = DesktopCoordinateSpace::new(input[0]).rect_from_host(rect(input, 1));
            vec![r.origin.x, r.origin.y, r.size.width, r.size.height]
        }
        "space.rectFromPrimaryAnchored" => {
            let r = DesktopCoordinateSpace::new(input[0])
                .rect_from_primary_anchored(rect(input, 1), input[5]);
            vec![r.origin.x, r.origin.y, r.size.width, r.size.height]
        }
        other => panic!("fixture names an operation this port does not have: {other}"),
    }
}

#[test]
fn matches_the_swift_original_bit_for_bit() {
    let fixture = include_str!("fixtures/geometry.txt");
    let mut checked = 0usize;
    let mut operations = std::collections::BTreeSet::new();

    for (index, line) in fixture.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let (lhs, rhs) = line.split_once('|').expect("every case has a | separator");
        let mut fields = lhs.split_whitespace();
        let op = fields.next().expect("every case names an operation");
        let input: Vec<f64> = fields
            .map(|field| f64::from_bits(u64::from_str_radix(field, 16).unwrap()))
            .collect();
        let expected: Vec<u64> = rhs
            .split_whitespace()
            .map(|field| u64::from_str_radix(field, 16).unwrap())
            .collect();

        let produced: Vec<u64> = answer(op, &input).into_iter().map(bits).collect();
        assert_eq!(
            produced,
            expected,
            "line {}: {op} disagrees with Swift\n  input    {:?}\n  swift    {:?}\n  rust     {:?}",
            index + 1,
            input,
            expected.iter().map(|b| f64::from_bits(*b)).collect::<Vec<_>>(),
            produced.iter().map(|b| f64::from_bits(*b)).collect::<Vec<_>>(),
        );
        operations.insert(op.to_string());
        checked += 1;
    }

    assert!(checked > 5_000, "only {checked} cases -- the fixture shrank");
    assert_eq!(operations.len(), 24, "an operation lost its coverage");
}
