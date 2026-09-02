// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

//! Unit 2's gate: world queries, the safe-zone planner and display topology,
//! against what the Swift original answered. Bit-exact, for the reasons in
//! `differential.rs`.
//!
//! A `world` line sets the display set that every case after it reads, which
//! is what keeps this fixture a megabyte instead of three.

use roamling_core::{
    BasicSafeZonePlanner, DesktopWorldSnapshot, DisplaySnapshot, DisplayTopology, WorldPoint,
    WorldRect, WorldSize, WorldVector,
};

fn hash_reason(reason: &str) -> f64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in reason.bytes() {
        hash = (hash ^ u64::from(byte)).wrapping_mul(0x100_0000_01b3);
    }
    (hash % 1_000_000) as f64
}

/// Display ids are `d0`, `d1`, ... so the fixture can carry them as numbers.
fn id_number(id: &str) -> f64 {
    id[1..].parse().expect("display ids are d<number>")
}

fn build_displays(flat: &[f64]) -> Vec<DisplaySnapshot> {
    flat.chunks_exact(8)
        .enumerate()
        .map(|(index, values)| DisplaySnapshot {
            id: format!("d{index}"),
            name: format!("d{index}"),
            frame: WorldRect::new(values[0], values[1], values[2], values[3]),
            visible_frame: WorldRect::new(values[4], values[5], values[6], values[7]),
            scale: 1.0,
        })
        .collect()
}

fn route_output(ids: &[String], waypoints: &[WorldPoint]) -> Vec<f64> {
    let mut out = vec![ids.len() as f64, waypoints.len() as f64];
    out.extend(ids.iter().map(|id| id_number(id)));
    for point in waypoints {
        out.push(point.x);
        out.push(point.y);
    }
    out
}

fn answer(
    op: &str,
    input: &[f64],
    world: &DesktopWorldSnapshot,
    topology: &DisplayTopology,
) -> Vec<f64> {
    match op {
        "world.displayContaining" => {
            let found = world.display_containing(WorldPoint::new(input[0], input[1]));
            vec![found.map(|d| id_number(&d.id)).unwrap_or(-1.0)]
        }
        "world.nearestDisplay" => {
            let found = world.nearest_display(WorldPoint::new(input[0], input[1]));
            vec![found.map(|d| id_number(&d.id)).unwrap_or(-1.0)]
        }
        "world.clamp" => {
            let point = world.clamp(
                WorldPoint::new(input[0], input[1]),
                WorldSize::new(input[2], input[3]),
            );
            vec![point.x, point.y]
        }
        "safeZone.count" => vec![BasicSafeZonePlanner::safe_zones(world).len() as f64],
        "safeZone.zone" => {
            let zones = BasicSafeZonePlanner::safe_zones(world);
            let zone = &zones[input[0] as usize];
            vec![
                zone.frame.min_x(),
                zone.frame.min_y(),
                zone.frame.size.width,
                zone.frame.size.height,
                zone.score,
                zone.confidence,
                hash_reason(&zone.reason),
            ]
        }
        "safeZone.destination" => {
            let pointer = if input[2] == 1.0 {
                Some(WorldPoint::new(input[3], input[4]))
            } else {
                None
            };
            let found = BasicSafeZonePlanner::destination(
                world,
                WorldPoint::new(input[0], input[1]),
                pointer,
                WorldSize::new(input[5], input[6]),
            );
            match found {
                Some(destination) => vec![
                    destination.point.x,
                    destination.point.y,
                    id_number(&destination.display_id),
                    destination.score,
                    hash_reason(&destination.reason),
                ],
                None => vec![-1.0],
            }
        }
        "topology.route" => {
            let route = topology.route(
                WorldPoint::new(input[0], input[1]),
                WorldPoint::new(input[2], input[3]),
            );
            route_output(&route.display_ids, &route.waypoints)
        }
        "topology.portal" => {
            let portal = topology.portal(&topology.displays[0], &topology.displays[1], None);
            vec![portal.exit.x, portal.exit.y, portal.entry.x, portal.entry.y]
        }
        "topology.portalNear" => {
            let portal = topology.portal(
                &topology.displays[0],
                &topology.displays[1],
                Some(WorldPoint::new(input[0], input[1])),
            );
            vec![portal.exit.x, portal.exit.y, portal.entry.x, portal.entry.y]
        }
        "topology.evade" => {
            let found = topology.evade_transition(
                WorldPoint::new(input[0], input[1]),
                WorldVector::new(input[2], input[3]),
                WorldSize::new(input[4], input[5]),
                320.0,
                1.0,
            );
            match found {
                Some(route) => route_output(&route.display_ids, &route.waypoints),
                None => vec![-1.0],
            }
        }
        other => panic!("fixture names an operation this port does not have: {other}"),
    }
}

fn parse(fields: std::str::SplitWhitespace<'_>) -> Vec<f64> {
    fields
        .map(|field| f64::from_bits(u64::from_str_radix(field, 16).unwrap()))
        .collect()
}

#[test]
fn matches_the_swift_original_bit_for_bit() {
    let fixture = include_str!("fixtures/world.txt");
    let mut world = DesktopWorldSnapshot::default();
    let mut topology = DisplayTopology::new(Vec::new());
    let mut checked = 0usize;
    let mut operations = std::collections::BTreeSet::new();

    for (index, line) in fixture.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        if let Some(rest) = line.strip_prefix("world ") {
            let displays = build_displays(&parse(rest.split_whitespace()));
            world = DesktopWorldSnapshot::new(displays.clone(), Vec::new());
            topology = DisplayTopology::new(displays);
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

        let produced: Vec<u64> = answer(op, &input, &world, &topology)
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

    assert!(checked > 5_000, "only {checked} cases -- the fixture shrank");
    assert_eq!(operations.len(), 10, "an operation lost its coverage");
}
