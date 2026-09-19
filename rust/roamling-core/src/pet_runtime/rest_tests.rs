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
