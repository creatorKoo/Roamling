// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

//! Unit 4's gate. These two carry state, so the fixture is a script rather than
//! a set of independent cases: `reset` starts a fresh pair, and every call after
//! it depends on all the ones before -- the dwell, the hysteresis, the revisit
//! cooldown and the reaction interval only exist across calls.

use roamling_core::{
    AttentionModel, CompanionEvent, CompanionEventKind, CompanionReaction, LocationHint,
    ReactingBehavior, ReactionPolicy, UserContext,
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

fn reaction_code(reaction: CompanionReaction) -> f64 {
    match reaction {
        CompanionReaction::Glance => 0.0,
        CompanionReaction::Observe => 1.0,
        CompanionReaction::Spark => 2.0,
        CompanionReaction::Work => 3.0,
        CompanionReaction::Paw => 4.0,
        CompanionReaction::SmallCelebrate => 5.0,
        CompanionReaction::LargeCelebrate => 6.0,
        CompanionReaction::Sad => 7.0,
        CompanionReaction::Calm => 8.0,
    }
}

fn parse(fields: std::str::SplitWhitespace<'_>) -> Vec<f64> {
    fields
        .map(|field| f64::from_bits(u64::from_str_radix(field, 16).unwrap()))
        .collect()
}

/// Ids carry the source and the index within the batch, which is all the
/// fixture needs to say which event came back.
fn events_from(values: &[f64]) -> Vec<CompanionEvent> {
    values
        .chunks_exact(7)
        .map(|fields| {
            let hint = (fields[5] == 1.0).then(|| LocationHint::new(None, fields[6]));
            CompanionEvent::new(
                format!("e{}", fields[2] as i64),
                format!("s{}", fields[1] as i64),
                fields[3],
                KINDS[fields[0] as usize],
                fields[4],
                hint,
            )
        })
        .collect()
}

#[test]
fn matches_the_swift_original_bit_for_bit() {
    let fixture = include_str!("fixtures/attention.txt");
    let mut attention = AttentionModel::default();
    let mut policy = ReactionPolicy::default();
    let mut checked = 0usize;
    let mut operations = std::collections::BTreeSet::new();

    for (index, line) in fixture.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        if line == "reset" {
            attention = AttentionModel::default();
            policy = ReactionPolicy::default();
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
            "select" => {
                let now = input[0];
                let count = input[1] as usize;
                let events = events_from(&input[2..2 + count * 7]);
                let selected = attention.select(&events, now);
                let mut out = match selected {
                    Some(event) => vec![
                        event.source_id[1..].parse().unwrap(),
                        event.id[1..].parse().unwrap(),
                    ],
                    None => vec![NIL],
                };
                out.push(
                    attention
                        .current_source_id()
                        .map_or(NIL, |id| id[1..].parse().unwrap()),
                );
                out.push(attention.acquired_at().unwrap_or(NIL));
                out
            }
            "clear" => {
                attention.clear(input[0]);
                vec![attention
                    .current_source_id()
                    .map_or(NIL, |id| id[1..].parse().unwrap())]
            }
            "react" => {
                let event = CompanionEvent::new(
                    "x",
                    "s0",
                    input[0],
                    KINDS[input[1] as usize],
                    input[2],
                    None,
                );
                let behavior = if input[4] == 1.0 {
                    ReactingBehavior::Caught
                } else {
                    ReactingBehavior::Other
                };
                let reaction = policy.reaction(
                    &event,
                    CONTEXTS[input[3] as usize],
                    behavior,
                    input[5],
                    input[0],
                );
                vec![reaction.map_or(NIL, reaction_code)]
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

    assert!(checked > 3_000, "only {checked} cases -- the fixture shrank");
    assert_eq!(operations.len(), 3, "an operation lost its coverage");
}
