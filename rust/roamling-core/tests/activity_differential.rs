// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

//! Unit 6b's gate: the activity director.
//!
//! It carries seven fields between calls -- which agent is being watched, what
//! the pet wears, what it still owes -- so the fixture is a script. It also
//! answers in an ordered list of effects, and the order is part of the answer:
//! a setback settles the seat, cancels the route and *then* reacts, and doing
//! those in another order moves the pet.

use roamling_core::{
    wants_window_hint, ActivityDirector, ActivityEffect, CompanionEvent, CompanionEventKind,
    CompanionReaction, LocationHint, UserContext, WorldRect,
};

const NIL: f64 = -1.0;

const KINDS: [CompanionEventKind; 11] = [
    CompanionEventKind::ActivityStarted,
    CompanionEventKind::ActivityEnded,
    CompanionEventKind::Positive,
    CompanionEventKind::Negative,
    CompanionEventKind::Achievement,
    CompanionEventKind::Setback,
    CompanionEventKind::AttentionRequired,
    CompanionEventKind::Inspecting,
    CompanionEventKind::HighIntensity,
    CompanionEventKind::Calm,
    CompanionEventKind::Idle,
];

const CONTEXTS: [UserContext; 5] = [
    UserContext::Working,
    UserContext::Gaming,
    UserContext::WatchingMedia,
    UserContext::Browsing,
    UserContext::Idle,
];

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

fn parse(fields: std::str::SplitWhitespace<'_>) -> Vec<f64> {
    fields
        .map(|field| f64::from_bits(u64::from_str_radix(field, 16).unwrap()))
        .collect()
}

fn reaction_index(reaction: CompanionReaction) -> f64 {
    REACTIONS
        .iter()
        .position(|candidate| *candidate == reaction)
        .expect("closed vocabulary") as f64
}

/// Effects travel as a count followed by five numbers each, so the comparison
/// never has to guess how many a case produced.
fn encode(effects: &[ActivityEffect]) -> Vec<f64> {
    let mut out = vec![effects.len() as f64];
    for effect in effects {
        match effect {
            ActivityEffect::CancelRest => out.extend([0.0, NIL, NIL, NIL, NIL]),
            ActivityEffect::SettleInPlace { source_id } => out.extend([
                1.0,
                source_id[1..].parse::<f64>().expect("s<n>"),
                NIL,
                NIL,
                NIL,
            ]),
            ActivityEffect::CancelRoute => out.extend([2.0, NIL, NIL, NIL, NIL]),
            ActivityEffect::SetNextWanderAt { timestamp } => {
                out.extend([3.0, *timestamp, NIL, NIL, NIL])
            }
            ActivityEffect::ApplyReaction { reaction } => {
                out.extend([4.0, reaction_index(*reaction), NIL, NIL, NIL])
            }
            ActivityEffect::RequestLuminance { region } => out.extend([
                5.0,
                region.min_x(),
                region.min_y(),
                region.size.width,
                region.size.height,
            ]),
        }
    }
    out
}

fn state(director: &ActivityDirector) -> Vec<f64> {
    vec![
        director.is_watching_window() as u8 as f64,
        director
            .active_source_id()
            .map_or(NIL, |id| id[1..].parse().expect("s<n>")),
        director.hint().is_some() as u8 as f64,
        director.hint().map_or(NIL, |hint| hint.confidence),
        director.has_arrival_reaction() as u8 as f64,
        director.sustained_reaction().map_or(NIL, reaction_index),
    ]
}

#[test]
fn matches_the_swift_original_bit_for_bit() {
    let fixture = include_str!("fixtures/activity.txt");
    let mut director = ActivityDirector::default();
    let mut checked = 0usize;
    let mut operations = std::collections::BTreeSet::new();
    let mut effects_seen = std::collections::BTreeSet::new();
    // Discriminants are not orderable, so the effect's own code stands in.

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
            "reset" => {
                director = ActivityDirector::default();
                state(&director)
            }
            "wants" => vec![wants_window_hint(KINDS[input[0] as usize]) as u8 as f64],
            _ => {
                let effects = match op {
                    "event" => {
                        let hint = (input[5] == 1.0).then(|| {
                            LocationHint::new(
                                Some(WorldRect::new(input[6], input[7], input[8], input[9])),
                                input[10],
                            )
                        });
                        let context =
                            (input[11] == 1.0).then(|| CONTEXTS[input[12] as usize]);
                        let event = CompanionEvent::new(
                            format!("e{}", input[2] as i64),
                            format!("s{}", input[1] as i64),
                            input[3],
                            KINDS[input[0] as usize],
                            input[4],
                            hint,
                        )
                        .with_context(context);
                        director.handle_event(
                            event,
                            input[13] == 1.0,
                            input[14] == 1.0,
                            input[15],
                            input[16],
                        )
                    }
                    "expire" => director.expire_silent(input[0] == 1.0, input[1]),
                    "resume" => director.resume_pending_if_ready(
                        input[0] == 1.0,
                        input[1] == 1.0,
                        input[2] == 1.0,
                        input[3],
                        input[4],
                    ),
                    "arrive" => director.deliver_arrival_reaction(input[0] == 1.0, input[1]),
                    "sustain" => director.sustain_on_seat(input[0] == 1.0, input[1]),
                    other => {
                        panic!("fixture names an operation this port does not have: {other}")
                    }
                };
                let mut out = encode(&effects);
                for chunk in out[1..].chunks_exact(5) {
                    effects_seen.insert(chunk[0] as i64);
                }
                out.extend(state(&director));
                out
            }
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
    assert_eq!(effects_seen.len(), 6, "an effect lost its coverage");
}
