// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

//! Unit 7's gate: animation resolution and playback.
//!
//! The resolver is a pure lookup, but it resolves along *meaning* -- each
//! capability names the one it degrades into -- so the interesting cases are
//! packages that declare two or three rows and have to answer for sixteen.
//!
//! The player half is a script: the cursor and the elapsed time only exist
//! across calls, and a directional look stops the clock rather than advancing
//! it.

use std::collections::BTreeMap;

use roamling_core::{
    look_frame_index, AnimationResolver, PetAnimationFrame, PetAnimationPlayer,
    PetAnimationTrack, PetCapability, Provenance, PET_CAPABILITIES,
};

const NIL: f64 = -1.0;

/// The whole vocabulary a package can use, so a track travels as an index
/// rather than as a string. Both sides build this table in the same order.
const NAMES: [&str; 39] = [
    "idle", "running-right", "running-left", "waving", "jumping", "failed",
    "waiting", "running", "review", "sitting", "sit", "sleeping", "sleep",
    "napping", "working", "typing", "observe", "gaze", "watching", "looking",
    "pawing", "paw", "spark", "celebrate", "stretching", "stretch", "caught",
    "dragged", "landing", "move-right", "move_right", "move-left", "move_left",
    "wave", "jump", "bounce", "failure", "sad", "nonsense",
];

fn parse(fields: std::str::SplitWhitespace<'_>) -> Vec<f64> {
    fields
        .map(|field| f64::from_bits(u64::from_str_radix(field, 16).unwrap()))
        .collect()
}

/// Frames are derived from the name's index, so the fixture does not have to
/// carry every duration.
fn frames(name_id: usize, count: usize) -> Vec<PetAnimationFrame> {
    (0..count)
        .map(|offset| PetAnimationFrame::new(name_id * 100 + offset, 0.05 + offset as f64 * 0.01))
        .collect()
}

fn build_resolver(input: &[f64]) -> AnimationResolver {
    let count = input[0] as usize;
    let mut tracks = BTreeMap::new();
    for slot in 0..count {
        let base = 1 + slot * 4;
        let name_id = input[base] as usize;
        let frame_count = input[base + 1] as usize;
        let loops = input[base + 2] == 1.0;
        let fallback = input[base + 3];
        let mut track = PetAnimationTrack::new(NAMES[name_id], frames(name_id, frame_count), loops);
        if fallback >= 0.0 {
            track.fallback = Some(NAMES[fallback as usize].to_string());
        }
        tracks.insert(NAMES[name_id].to_string(), track);
    }
    let mut cursor = 1 + count * 4;
    let explicit_count = input[cursor] as usize;
    cursor += 1;
    let mut explicit = BTreeMap::new();
    for slot in 0..explicit_count {
        let capability = PET_CAPABILITIES[input[cursor + slot * 2] as usize];
        let name_id = input[cursor + slot * 2 + 1] as usize;
        explicit.insert(
            roamling_core::capability_name(capability).to_string(),
            NAMES[name_id].to_string(),
        );
    }
    AnimationResolver::new(tracks, explicit)
}

fn name_index(name: &str) -> f64 {
    NAMES.iter().position(|candidate| *candidate == name).map_or(-2.0, |index| index as f64)
}

fn capability_index(capability: PetCapability) -> f64 {
    PET_CAPABILITIES
        .iter()
        .position(|candidate| *candidate == capability)
        .expect("closed vocabulary") as f64
}

#[test]
fn matches_the_swift_original_bit_for_bit() {
    let fixture = include_str!("fixtures/animation.txt");
    let mut resolver = AnimationResolver::default();
    let mut player = PetAnimationPlayer::new(&resolver);
    let mut checked = 0usize;
    let mut operations = std::collections::BTreeSet::new();
    let mut provenances = std::collections::BTreeSet::new();

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
            "resolver" => {
                resolver = build_resolver(&input);
                vec![resolver.tracks.len() as f64]
            }
            "resolve" => {
                let capability = PET_CAPABILITIES[input[0] as usize];
                let (track, provenance) = resolver.resolution(capability);
                provenances.insert(match provenance {
                    Provenance::Authored => 0,
                    Provenance::Substituted(_) => 1,
                    Provenance::Placeholder => 2,
                });
                let mut out = vec![
                    track.map_or(NIL, |track| name_index(&track.name)),
                    track.map_or(0.0, |track| track.frames.len() as f64),
                    track.map_or(NIL, |track| track.loops as u8 as f64),
                    track
                        .and_then(|track| track.frames.first())
                        .map_or(NIL, |frame| frame.index as f64),
                ];
                match provenance {
                    Provenance::Authored => out.extend([0.0, NIL]),
                    Provenance::Substituted(capability) => {
                        out.extend([1.0, capability_index(capability)])
                    }
                    Provenance::Placeholder => out.extend([2.0, NIL]),
                }
                out
            }
            "coverage" => {
                let coverage = resolver.coverage();
                vec![
                    coverage.authored.len() as f64,
                    coverage.substituted.len() as f64,
                    coverage.placeholder.len() as f64,
                    coverage.covered() as f64,
                    coverage.is_complete() as u8 as f64,
                ]
            }
            "player.reset" => {
                player = PetAnimationPlayer::new(&resolver);
                vec![player.current_frame_index() as f64]
            }
            "player.capability" => {
                player.set_capability(&resolver, PET_CAPABILITIES[input[0] as usize]);
                vec![player.current_frame_index() as f64]
            }
            "player.look" => {
                let look = look_frame_index(input[1], input[2] as usize, input[3] as usize);
                player.set_look_frame(if input[0] == 1.0 { look } else { None });
                vec![
                    player.current_frame_index() as f64,
                    look.map_or(NIL, |index| index as f64),
                ]
            }
            "player.update" => {
                player.update(input[0]);
                vec![player.current_frame_index() as f64]
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

    assert!(checked > 7_000, "only {checked} cases -- the fixture shrank");
    assert_eq!(operations.len(), 7, "an operation lost its coverage");
    assert_eq!(provenances.len(), 3, "a provenance lost its coverage");
}
