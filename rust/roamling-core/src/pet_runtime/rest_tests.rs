// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

use super::*;
use crate::activity::CompanionEventKind;

fn sample(now: f64) -> TickInput {
    TickInput {
        now, pointer: WorldPoint::new(-1000.0, -1000.0),
        primary_button_down: false, user_idle_duration: 1000.0,
        capture_authorized: false, focus_authorized: false,
        did_query_focus: false, queried_focus: None,
        pointer_is_over_pet: false, affection_held: false,
    }
}

fn sleeping() -> PetRuntime {
    sleeping_with_agent(false)
}

fn sleeping_with_agent(watching: bool) -> PetRuntime {
    let mut pet = PetRuntime::new(WorldPoint::new(500.0, 400.0), RuntimeTuning::default(), 7);
    let frame = WorldRect::new(0.0, 0.0, 1280.0, 800.0);
    pet.set_displays(vec![DisplaySnapshot {
        id: "main".into(), name: "main".into(), frame, visible_frame: frame, scale: 1.0,
    }]);
    pet.set_flags(false, true, true);
    if watching {
        pet.handle_activity_event(CompanionEvent::new("work", "codex:turn", 99.0,
            CompanionEventKind::HighIntensity, 1.0, Some(LocationHint::new(Some(frame), 1.0))), 99.0);
    }
    for i in 0..100 {
        let now = 100.0 + i as f64 / 30.0;
        pet.begin_tick(now);
        pet.finish_tick(&sample(now));
    }
    assert_eq!(pet.state(), BehaviorState::Sleep);
    pet
}

fn event(pet: &mut PetRuntime, kind: CompanionEventKind, now: f64) {
    pet.handle_activity_event(CompanionEvent::new(
        format!("event-{now}"), "codex:turn", now, kind, 1.0, None,
    ), now);
}

fn tick(pet: &mut PetRuntime, now: f64) -> BehaviorState {
    pet.begin_tick(now);
    pet.finish_tick(&sample(now)).state
}

#[test]
fn waking_waits_for_stretch_before_followup_agent_event() {
    let mut pet = sleeping();
    event(&mut pet, CompanionEventKind::AttentionRequired, 104.0);
    assert_eq!(pet.state(), BehaviorState::Wake);
    event(&mut pet, CompanionEventKind::AttentionRequired, 104.1);
    assert_eq!(pet.state(), BehaviorState::Wake, "same-agent followup must remain pending");
    assert_eq!(tick(&mut pet, 104.71), BehaviorState::Stretch);
    assert_eq!(tick(&mut pet, 106.60), BehaviorState::Stretch);
    assert_eq!(tick(&mut pet, 106.62), BehaviorState::WaitingForUser);
}

#[test]
fn waking_cursor_waits_until_the_full_stretch_finishes() {
    for affection in [true, false] {
        let mut pet = sleeping();
        let mut input = sample(104.0);
        input.pointer = pet.position().offset(WorldVector::new(135.0, 0.0));
        input.affection_held = affection;
        pet.begin_tick(input.now);
        assert_eq!(pet.finish_tick(&input).state, BehaviorState::Wake);
        for (now, expected) in [(104.1, BehaviorState::Wake), (104.71, BehaviorState::Stretch),
            (106.60, BehaviorState::Stretch), (106.62, BehaviorState::LookAtPointer)] {
            input.now = now;
            pet.begin_tick(now);
            assert_eq!(pet.finish_tick(&input).state, expected, "at {now}, affection={affection}");
        }
    }
}

#[test]
fn waking_fast_approach_under_the_affection_key_still_stretches() {
    let mut pet = sleeping();
    let mut input = sample(104.0);
    // From far away to 50 pt in one tick: a fast approach, which opens the
    // approach hold, and the affection key lets that branch run from any state.
    input.pointer = pet.position().offset(WorldVector::new(50.0, 0.0));
    input.affection_held = true;
    pet.begin_tick(input.now);
    assert_eq!(pet.finish_tick(&input).state, BehaviorState::Wake);
    for (now, expected) in [(104.1, BehaviorState::Wake), (104.71, BehaviorState::Stretch),
        (106.60, BehaviorState::Stretch)] {
        input.now = now;
        pet.begin_tick(now);
        assert_eq!(pet.finish_tick(&input).state, expected, "at {now}");
    }
}

#[test]
fn waking_shows_what_the_agent_is_doing_now_not_what_woke_it() {
    let mut pet = sleeping();
    event(&mut pet, CompanionEventKind::AttentionRequired, 104.0);
    assert_eq!(pet.state(), BehaviorState::Wake);
    // Approved and working again before the pet is on its feet.
    event(&mut pet, CompanionEventKind::HighIntensity, 104.1);
    assert_eq!(pet.state(), BehaviorState::Wake);
    assert_eq!(tick(&mut pet, 104.71), BehaviorState::Stretch);
    assert_eq!(tick(&mut pet, 106.60), BehaviorState::Stretch);
    // Read between the two halves of the tick: with nobody at the keyboard
    // the second half sits the pet straight back down beside its working
    // agent, which is right. What it must not do is go back to asking.
    pet.begin_tick(106.62);
    assert_eq!(pet.state(), BehaviorState::Work);
    pet.finish_tick(&sample(106.62));
    assert_ne!(tick(&mut pet, 110.0), BehaviorState::WaitingForUser);
}

/// B8: the pet beside a Codex run sat in the asking pose for the whole run
/// and so never slept, because a waiting pet does not rest and nothing ended
/// the wait.
#[test]
fn an_answered_question_lets_the_pet_doze_beside_the_running_tool() {
    let mut pet = sleeping_with_agent(true);
    event(&mut pet, CompanionEventKind::AttentionRequired, 104.0);
    let mut now = 104.0;
    while now < 110.0 {
        now += 1.0 / 30.0;
        tick(&mut pet, now);
    }
    assert_eq!(pet.state(), BehaviorState::WaitingForUser);
    // The tool ran and finished, so someone said yes.
    pet.handle_activity_event(CompanionEvent::new(
        "post", "codex:turn", now, CompanionEventKind::Positive, 0.08, None,
    ), now);
    assert_eq!(pet.state(), BehaviorState::Work);
    while now < 120.0 {
        now += 1.0 / 30.0;
        tick(&mut pet, now);
    }
    assert_eq!(pet.state(), BehaviorState::Sleep);
}

#[test]
fn waking_caret_travel_waits_for_stretch() {
    let mut pet = sleeping_with_agent(true);
    let origin = pet.position();
    let hint = LocationHint::new(Some(pet.displays[0].frame), 1.0);
    pet.handle_activity_event(CompanionEvent::new(
        "work", "codex:turn", 104.0, CompanionEventKind::HighIntensity, 1.0, Some(hint),
    ), 104.0);
    // Routine progress does not wake a sleeping pet. A newly observed caret does.
    assert_eq!(pet.state(), BehaviorState::Sleep);
    let mut input = sample(104.0);
    input.focus_authorized = true;
    input.did_query_focus = true;
    input.queried_focus = Some(FocusSnapshot::new(Some(pet.displays[0].frame), None,
        Some(WorldRect::new(origin.x, origin.y, 2.0, 20.0)), 1.0));
    pet.begin_tick(input.now);
    assert_eq!(pet.finish_tick(&input).state, BehaviorState::Wake);
    for (now, expected) in [(104.1, BehaviorState::Wake), (104.71, BehaviorState::Stretch),
        (106.60, BehaviorState::Stretch), (106.62, BehaviorState::TravelToInterest)] {
        input.now = now;
        pet.begin_tick(now);
        assert_eq!(pet.finish_tick(&input).state, expected, "at {now}");
        if expected != BehaviorState::TravelToInterest { assert_eq!(pet.position(), origin); }
    }
}

#[test]
fn waking_achievement_is_delivered_after_stretch() {
    let mut pet = sleeping();
    event(&mut pet, CompanionEventKind::AttentionRequired, 104.0);
    assert_eq!(tick(&mut pet, 104.71), BehaviorState::Stretch);
    event(&mut pet, CompanionEventKind::Achievement, 104.81);
    assert_eq!(pet.state(), BehaviorState::Stretch);
    assert_eq!(tick(&mut pet, 106.60), BehaviorState::Stretch);
    assert_eq!(tick(&mut pet, 106.62), BehaviorState::Celebrate);
}

#[test]
fn waking_catch_can_interrupt_even_the_extended_stretch() {
    // Inside the old 1.7 s and inside the part that was added: a catch was
    // always allowed, and lengthening the stretch must not have closed it.
    for grab_at in [104.8, 106.01] {
        let mut pet = sleeping();
        event(&mut pet, CompanionEventKind::AttentionRequired, 104.0);
        assert_eq!(tick(&mut pet, 104.71), BehaviorState::Stretch);
        assert_eq!(tick(&mut pet, grab_at - 0.01), BehaviorState::Stretch);
        pet.pointer_down(pet.position(), grab_at);
        assert_eq!(pet.state(), BehaviorState::Caught, "grabbed at {grab_at}");
    }
}

/// The user has stepped away and left the cursor parked. Returns how many
/// times the pet woke in the next minute, and the pet.
fn rest_beside_a_parked_cursor(
    start: WorldPoint,
    cursor: WorldPoint,
    field: Option<LuminanceField>,
    avoidance: bool,
) -> (usize, PetRuntime) {
    let mut pet = PetRuntime::new(start, RuntimeTuning::default(), 7);
    let frame = WorldRect::new(0.0, 0.0, 1280.0, 800.0);
    pet.set_displays(vec![DisplaySnapshot {
        id: "main".into(), name: "main".into(), frame, visible_frame: frame, scale: 1.0,
    }]);
    pet.set_flags(true, avoidance, true);
    let capture = field.is_some();
    pet.set_luminance(field);
    let mut wakes = 0;
    for i in 0..1800 {
        let now = 100.0 + f64::from(i) / 30.0;
        let mut input = sample(now);
        input.pointer = cursor;
        input.capture_authorized = capture;
        pet.begin_tick(now);
        let before = pet.state();
        if pet.finish_tick(&input).state == BehaviorState::Wake && before != BehaviorState::Wake {
            wakes += 1;
        }
    }
    (wakes, pet)
}

/// A column of text down the middle of the 1280×800 desk, x 560...880, in the
/// 64×40 grid the shells capture.
fn text_down_the_middle() -> LuminanceField {
    let samples = (0..40usize)
        .flat_map(|row| {
            (0..64usize).map(move |col| match (28..44).contains(&col) {
                true if (col + row) % 2 == 0 => 0.1,
                true => 0.9,
                false => 1.0,
            })
        })
        .collect();
    LuminanceField::new(WorldRect::new(0.0, 0.0, 1280.0, 800.0), 64, 40, samples).unwrap()
}

/// B10. The clearest bed on the desk lay past a cursor the user had left on
/// their text. The walk there woke the pet 170 pt short of the cursor, the
/// walk off the text was cut short a tick later by the next sit, and the next
/// sit chose the same bed -- every 5.2 s until the user came back.
#[test]
fn b10_the_walk_to_bed_does_not_pass_a_parked_cursor() {
    let cursor = WorldPoint::new(720.0, 400.0);
    let (wakes, pet) = rest_beside_a_parked_cursor(
        WorldPoint::new(1100.0, 400.0), cursor, Some(text_down_the_middle()), true,
    );
    assert_eq!(wakes, 0);
    assert_eq!(pet.state(), BehaviorState::Sleep);
    assert!(pet.position().distance(cursor) > 170.0, "asleep at {:?}", pet.position());
}

/// Without a capture the beds are the four corners, and the cursor marks
/// down only a corner within 260 pt of it -- not one whose walk crosses it.
/// With pointer avoidance off nothing else keeps a walk clear of the cursor,
/// and a resting pet wakes for the cursor all the same.
#[test]
fn b10_without_capture_or_avoidance_the_bed_is_reachable_too() {
    let cursor = WorldPoint::new(364.0, 502.0);
    let outcomes: Vec<_> = [true, false]
        .into_iter()
        .map(|avoidance| {
            let (wakes, pet) =
                rest_beside_a_parked_cursor(WorldPoint::new(600.0, 300.0), cursor, None, avoidance);
            (avoidance, wakes, pet.state())
        })
        .collect();
    assert!(
        outcomes.iter().all(|&(_, wakes, state)| wakes == 0 && state == BehaviorState::Sleep),
        "(avoidance, wakes, state): {outcomes:?}"
    );
}

/// B10 and B11 together: the user steps away with the cursor parked beside
/// an awake pet. It looks, tires of looking, walks off, and goes to sleep once,
/// out of the cursor's reach. Before B11 it looked for as long as the user was
/// gone, and a pet looking at a cursor never starts to rest.
#[test]
fn b11_an_absent_users_parked_cursor_ends_in_one_nap_away_from_it() {
    let cursor = WorldPoint::new(740.0, 400.0);
    let (wakes, pet) =
        rest_beside_a_parked_cursor(WorldPoint::new(600.0, 400.0), cursor, None, true);
    assert_eq!(wakes, 0);
    assert_eq!(pet.state(), BehaviorState::Sleep);
    assert!(pet.position().distance(cursor) > 170.0, "asleep at {:?}", pet.position());
}
