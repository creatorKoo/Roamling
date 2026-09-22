// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

use super::*;
use crate::activity::CompanionEventKind;

fn display(id: &str, x: f64, y: f64) -> DisplaySnapshot {
    DisplaySnapshot {
        id: id.into(),
        name: id.into(),
        frame: WorldRect::new(x, y, 1000.0, 800.0),
        visible_frame: WorldRect::new(x, y, 1000.0, 800.0),
        scale: 1.0,
    }
}

fn input(now: f64, pointer: WorldPoint) -> TickInput {
    TickInput {
        now,
        pointer,
        primary_button_down: false,
        user_idle_duration: 0.0,
        capture_authorized: false,
        focus_authorized: false,
        did_query_focus: false,
        queried_focus: None,
        pointer_is_over_pet: false,
        affection_held: false,
    }
}

#[test]
fn a_pointer_kept_on_the_pet_prevents_rest_despite_idle_desktop() {
    let mut pet = PetRuntime::new(WorldPoint::new(500.0, 400.0), RuntimeTuning::default(), 7);
    pet.set_displays(vec![display("main", 0.0, 0.0)]);
    for tick in 1..=600 {
        let now = 1000.0 + f64::from(tick) / 30.0;
        pet.begin_tick(now);
        let mut sample = input(now, pet.position());
        sample.pointer_is_over_pet = true;
        sample.user_idle_duration = 600.0;
        let output = pet.finish_tick(&sample);
        assert!(!output.state.is_resting(), "resting with a pointer on the pet: {:?}", output.state);
    }
}

#[test]
fn body_click_catches_walk_and_work_without_an_approach_then_drags_and_drops() {
    for working in [false, true] {
        let mut pet = PetRuntime::new(WorldPoint::new(500.0, 400.0), RuntimeTuning::default(), 7);
        pet.set_displays(vec![display("main", 0.0, 0.0)]);
        // Avoidance is independently configurable; direct clicks must not
        // depend on either avoidance or the fast-approach reaction.
        pet.set_flags(true, false, true);
        if working {
            pet.handle_activity_event(CompanionEvent::new(
                "work", "codex:turn", 10.0, CompanionEventKind::HighIntensity, 1.0, None,
            ), 10.0);
        } else {
            pet.begin_stroll(WorldPoint::new(750.0, 400.0), 10.0, 0.0);
        }
        let contact = pet.position();
        let mut sample = input(10.1, contact);
        sample.pointer_is_over_pet = true;
        let out = pet.finish_tick(&sample);
        assert!(out.interaction_enabled, "the shell must receive the click without a fast approach");
        assert_eq!(pet.approach_hold_until, 0.0);
        assert_eq!(out.state, if working { BehaviorState::Work } else { BehaviorState::Wander });
        let origin = pet.position();
        pet.pointer_down(contact, 10.11);
        assert_eq!(pet.state(), BehaviorState::Caught);
        assert!(!pet.movement.has_route());
        let dragged_pointer = contact.offset(WorldVector::new(100.0, 50.0));
        pet.pointer_dragged(dragged_pointer, 112.0, 10.2);
        assert_eq!(pet.state(), BehaviorState::Dragged);
        assert_eq!(pet.position(), origin.offset(WorldVector::new(100.0, 50.0)));
        let dropped = pet.pointer_up(dragged_pointer, true, 10.3);
        assert!(dropped.persist_position);
        assert!(!pet.state().is_held());
    }
}

#[test]
fn a_stationary_cursor_can_click_an_evading_pet() {
    let mut pet = PetRuntime::new(WorldPoint::new(500.0, 400.0), RuntimeTuning::default(), 7);
    pet.set_displays(vec![display("main", 0.0, 0.0)]);
    let contact = pet.position();
    let mut sample = input(10.0, contact);
    sample.pointer_is_over_pet = true;
    let out = pet.finish_tick(&sample);
    assert_eq!(pet.approach_hold_until, 0.0);
    assert_eq!(out.state, BehaviorState::EvadePointer);
    assert!(out.interaction_enabled);
    pet.pointer_down(contact, 10.01);
    assert_eq!(pet.state(), BehaviorState::Caught);
}

#[test]
fn direct_mouse_click_respects_body_visibility_and_interaction_gates() {
    let mut pet = PetRuntime::new(WorldPoint::new(500.0, 400.0), RuntimeTuning::default(), 7);
    pet.set_displays(vec![display("main", 0.0, 0.0)]);
    let contact = pet.position();
    assert!(!pet.finish_tick(&input(10.0, WorldPoint::new(800.0, 700.0))).interaction_enabled);
    pet.pointer_down(WorldPoint::new(800.0, 700.0), 10.01);
    assert!(!pet.state().is_held());
    let mut sample = input(10.1, contact);
    sample.pointer_is_over_pet = true;
    pet.set_hidden(true);
    assert!(!pet.finish_tick(&sample).interaction_enabled);
    pet.pointer_down(contact, 10.11);
    assert!(!pet.state().is_held());
    pet.set_hidden(false);
    pet.set_flags(true, false, false);
    sample.now = 10.2;
    assert!(!pet.finish_tick(&sample).interaction_enabled);
    pet.pointer_down(contact, 10.21);
    assert!(!pet.state().is_held());
    pet.set_flags(true, false, true);
    pet.pointer_down(pet.position(), 10.3);
    assert_eq!(pet.state(), BehaviorState::Caught);
    let origin = pet.position();
    pet.pointer_down(origin.offset(WorldVector::new(20.0, 0.0)), 10.31);
    pet.pointer_dragged(origin.offset(WorldVector::new(100.0, 0.0)), 100.0, 10.4);
    assert_eq!(pet.position(), origin.offset(WorldVector::new(100.0, 0.0)),
        "duplicate down must not reset the grab offset");
}


#[test]
fn hearts_belong_to_pet_contact_and_clear_on_hide_or_pet_change() {
    let origin = WorldPoint::new(500.0, 400.0);
    let mut pet = PetRuntime::new(origin, RuntimeTuning::default(), 7);
    pet.set_displays(vec![display("main", 0.0, 0.0)]);
    let mut saw_hearts = false;
    for tick in 0..60 {
        let now = 10.0 + f64::from(tick) / 30.0;
        pet.begin_tick(now);
        let mut sample = input(now, origin);
        sample.affection_held = true;
        sample.pointer_is_over_pet = true;
        pet.finish_tick(&sample);
        saw_hearts |= !pet.effect_frames().is_empty();
    }
    assert!(saw_hearts);
    assert_eq!(pet.position(), origin);
    pet.set_hidden(true);
    assert!(pet.effect_frames().is_empty());
    pet.set_hidden(false);
    for tick in 0..60 {
        let now = 12.0 + f64::from(tick) / 30.0;
        pet.begin_tick(now);
        let mut sample = input(now, origin);
        sample.affection_held = true;
        sample.pointer_is_over_pet = true;
        pet.finish_tick(&sample);
    }
    assert!(!pet.effect_frames().is_empty());
    pet.clear_click_reaction(true);
    assert!(pet.effect_frames().is_empty());
    let mut beside = input(14.0, origin.offset(WorldVector::new(130.0, 0.0)));
    beside.affection_held = true;
    for tick in 0..60 {
        beside.now = 14.0 + f64::from(tick) / 30.0;
        pet.begin_tick(beside.now);
        pet.finish_tick(&beside);
        assert!(pet.effect_frames().is_empty());
    }
}

#[test]
fn petting_follows_strokes_then_settles_without_moving_the_pet() {
    for hz in [30, 60] {
        let origin = WorldPoint::new(500.0, 400.0);
        let mut pet = PetRuntime::new(origin, RuntimeTuning::default(), 7);
        pet.set_displays(vec![display("main", 0.0, 0.0)]);
        let mut rate = 0.0;
        for tick in 0..=hz * 4 {
            let now = 10.0 + f64::from(tick) / f64::from(hz);
            pet.begin_tick(now);
            // One second still, a second stroking back and forth across the
            // body, then two seconds still. Coordinates stay on the body.
            let x = if tick > hz && tick <= hz * 2 {
                20.0 * (std::f64::consts::TAU * 5.0 * (now - 11.0)).sin()
            } else { 0.0 };
            let mut sample = input(now, origin.offset(WorldVector::new(x, 0.0)));
            sample.affection_held = true;
            sample.pointer_is_over_pet = true;
            let out = pet.finish_tick(&sample);
            assert_eq!(out.state, BehaviorState::LookAtPointer);
            assert_eq!(out.capability, PetCapability::Paw);
            assert_eq!(pet.position(), origin);
            assert!((0.5..=2.5).contains(&out.locomotion_rate));
            if tick <= hz { assert_eq!(out.locomotion_rate, 0.5); }
            if tick == hz * 2 { assert!(out.locomotion_rate > 1.7); }
            if tick > hz * 2 { assert!(out.locomotion_rate <= rate); }
            rate = out.locomotion_rate;
        }
        assert!((rate - 0.5).abs() < 0.004);
        // Leaving and returning starts a new, calm touch even after a jump.
        pet.finish_tick(&input(14.1, WorldPoint::new(900.0, 400.0)));
        let mut returning = input(14.2, origin);
        returning.affection_held = true;
        returning.pointer_is_over_pet = true;
        assert_eq!(pet.finish_tick(&returning).locomotion_rate, 0.5);
    }
}

#[test]
fn approval_wait_uses_the_slow_tilt_even_when_the_pointer_moves() {
    let mut pet = PetRuntime::new(WorldPoint::new(500.0, 400.0), RuntimeTuning::default(), 7);
    pet.set_displays(vec![display("main", 0.0, 0.0)]);
    pet.handle_activity_event(CompanionEvent::new(
        "approval", "codex:turn", 10.0, CompanionEventKind::AttentionRequired, 1.0, None,
    ), 10.0);
    for tick in 0..60 {
        let now = 10.0 + f64::from(tick) / 30.0;
        pet.begin_tick(now);
        let pointer = WorldPoint::new(if tick % 2 == 0 { 800.0 } else { 950.0 }, 400.0);
        let out = pet.finish_tick(&input(now, pointer));
        assert_eq!(out.state, BehaviorState::WaitingForUser);
        assert!(pet.effect_frames().is_empty());
        assert_eq!(out.capability, PetCapability::Paw);
        assert_eq!(out.locomotion_rate, 0.5);
    }
}

#[test]
fn codex_work_ignores_glance_but_not_affection_or_close_evasion() {
    let mut pet = PetRuntime::new(WorldPoint::new(500.0, 400.0), RuntimeTuning::default(), 7);
    pet.set_displays(vec![display("main", 0.0, 0.0)]);
    pet.handle_activity_event(
        CompanionEvent::new(
            "work",
            "codex:turn",
            10.0,
            CompanionEventKind::HighIntensity,
            1.0,
            None,
        ),
        10.0,
    );
    assert_eq!(pet.state(), BehaviorState::Work);
    for i in 0..90 {
        let now = 10.0 + f64::from(i) / 30.0;
        pet.begin_tick(now);
        let out = pet.finish_tick(&input(now, WorldPoint::new(635.0, 400.0)));
        assert_eq!(out.state, BehaviorState::Work);
        assert_eq!(out.capability, PetCapability::Work);
    }
    let mut affection = input(13.1, WorldPoint::new(635.0, 400.0));
    affection.affection_held = true;
    // Beside the pet, affection still uses the distance-paced gaze. On its
    // body, the head tilt must slow down instead of inheriting that haste.
    let gaze = pet.finish_tick(&affection);
    assert_eq!(gaze.state, BehaviorState::LookAtPointer);
    assert_eq!(gaze.capability, PetCapability::Gaze);
    assert!(gaze.locomotion_rate > 1.0);
    affection.now = 13.12;
    affection.pointer = pet.position();
    affection.pointer_is_over_pet = true;
    let petted = pet.finish_tick(&affection);
    assert_eq!(petted.capability, PetCapability::Paw);
    assert_eq!(petted.locomotion_rate, 0.5);
    affection.now = 13.15;
    affection.pointer = WorldPoint::new(635.0, 400.0);
    affection.pointer_is_over_pet = false;
    let beside = pet.finish_tick(&affection);
    assert_eq!(beside.capability, PetCapability::Gaze);
    assert_eq!(beside.locomotion_rate, gaze.locomotion_rate);
    // Work without a window hint must also recover after explicit affection.
    assert_eq!(
        pet.finish_tick(&input(13.2, WorldPoint::new(635.0, 400.0)))
            .state,
        BehaviorState::Work
    );
    // A held approach reaction exposes the hit target without starting a tail wag.
    pet.approach_hold_until = 14.0;
    assert_eq!(
        pet.finish_tick(&input(13.3, WorldPoint::new(635.0, 400.0)))
            .state,
        BehaviorState::Work
    );
    pet.pointer_down(pet.position(), 13.4);
    assert_eq!(pet.state(), BehaviorState::Caught);

    let mut evading =
        PetRuntime::new(WorldPoint::new(500.0, 400.0), RuntimeTuning::default(), 7);
    evading.set_displays(vec![display("main", 0.0, 0.0)]);
    evading.handle_activity_event(
        CompanionEvent::new(
            "work",
            "codex:turn",
            10.0,
            CompanionEventKind::HighIntensity,
            1.0,
            None,
        ),
        10.0,
    );
    assert_eq!(
        evading
            .finish_tick(&input(10.0, WorldPoint::new(580.0, 400.0)))
            .state,
        BehaviorState::EvadePointer
    );
}

#[test]
fn a_wandering_pet_only_wags_after_arriving_and_sitting_still() {
    let mut pet = PetRuntime::new(WorldPoint::new(500.0, 400.0), RuntimeTuning::default(), 7);
    pet.set_displays(vec![display("main", 0.0, 0.0)]);
    pet.begin_stroll(WorldPoint::new(750.0, 400.0), 10.0, 0.0);
    pet.approach_hold_until = 100.0;
    let mut arrived = false;
    for i in 1..600 {
        let now = 10.0 + f64::from(i) / 30.0;
        pet.begin_tick(now);
        let pointer = pet.position().offset(WorldVector::new(135.0, 0.0));
        let out = pet.finish_tick(&input(now, pointer));
        assert_ne!(out.state, BehaviorState::LookAtPointer);
        if !pet.movement.has_route() {
            assert_eq!(out.state, BehaviorState::Idle);
            assert_eq!(pet.position(), WorldPoint::new(750.0, 400.0));
            let seated = pet.finish_tick(&input(now + 0.1, pointer));
            assert_eq!(seated.state, BehaviorState::LookAtPointer);
            arrived = true;
            break;
        }
    }
    assert!(arrived);
}

#[test]
fn shared_edge_crossings_finish_in_one_direction_despite_a_nearby_cursor() {
    for (dx, dy) in [(1000.0, 0.0), (-1000.0, 0.0), (0.0, 800.0), (0.0, -800.0)] {
        let centre = WorldPoint::new(500.0, 400.0);
        let direction = WorldVector::new(dx, dy).normalized();
        let start = centre.offset(direction.scaled(if dx != 0.0 { 400.0 } else { 300.0 }));
        let destination = centre.offset(WorldVector::new(dx, dy));
        let mut pet = PetRuntime::new(start, RuntimeTuning::default(), 7);
        pet.set_displays(vec![display("a", 0.0, 0.0), display("b", dx, dy)]);
        pet.begin_stroll(destination, 0.0, 0.0);
        let mut previous = start;
        let mut committed = false;
        let mut finished = false;
        for i in 1..900 {
            let now = f64::from(i) / 30.0;
            pet.begin_tick(now);
            let previous_speed = pet.movement.velocity().length();
            let out =
                pet.finish_tick(&input(now, pet.position().offset(direction.scaled(135.0))));
            committed |= pet.crossing_clear.is_some();
            assert!(out.position.vector_from(previous).dot(direction) >= -0.001);
            assert_ne!(out.state, BehaviorState::LookAtPointer);
            let seam = centre.offset(direction.scaled(if dx != 0.0 { 500.0 } else { 400.0 }));
            if out.position.distance(seam) < 20.0 {
                assert!(
                    pet.movement.velocity().length() + 0.001 >= previous_speed,
                    "slowed down on the display seam"
                );
            }
            if committed && pet.crossing_clear.is_none() {
                let target = &pet.displays[1];
                assert!(placement_frame(target, &pet.displays)
                    .inset_by(48.0, 52.0)
                    .contains(out.position));
                finished = true;
                break;
            }
            previous = out.position;
        }
        assert!(finished, "crossing did not finish: {dx}, {dy}");
    }
}

#[test]
fn wander_and_sleep_destinations_leave_shared_edges_clear() {
    let mut pet = PetRuntime::new(WorldPoint::new(990.0, 400.0), RuntimeTuning::default(), 7);
    pet.set_displays(vec![
        display("left", 0.0, 0.0),
        display("right", 1000.0, 0.0),
    ]);
    for _ in 0..500 {
        let point = pet.random_wander_point().unwrap();
        assert!((point.x - 1000.0).abs() >= 168.0);
    }
    // The sleep spot is the director's answer to a sit that has just ended.
    let input = TickInput {
        now: 10.0, pointer: WorldPoint::new(0.0, 0.0), primary_button_down: false,
        user_idle_duration: 1000.0, capture_authorized: false, focus_authorized: false,
        did_query_focus: false, queried_focus: None, pointer_is_over_pet: false,
        affection_held: false,
    };
    let mut situation = pet.make_situation(10.0, &input, PointerProximity::Far, false, false);
    situation.is_resting = true;
    situation.rest_phase = RestPhase::Seeking;
    let PlacementIntent::RestAt(bed) = pet.placement.decide(&situation) else {
        panic!("a pet on a shared edge has to walk somewhere to sleep");
    };
    assert_eq!(pet.placement.rest_walk(), Some(bed));
    assert!((bed.x - 1000.0).abs() >= 168.0);
    let world = placement_world(&pet.world);
    let seat = crate::interest::BasicInterestPositionPlanner::destination(
        &LocationHint::new(Some(WorldRect::new(650.0, 0.0, 350.0, 800.0)), 1.0),
        &world,
        pet.position(),
        None,
        0.0,
        pet.object_size,
    )
    .unwrap();
    assert!((seat.point.x - 1000.0).abs() >= 168.0);
}

#[test]
fn a_direct_catch_can_interrupt_a_committed_crossing() {
    let mut pet = PetRuntime::new(WorldPoint::new(900.0, 400.0), RuntimeTuning::default(), 7);
    pet.set_displays(vec![
        display("left", 0.0, 0.0),
        display("right", 1000.0, 0.0),
    ]);
    pet.begin_stroll(WorldPoint::new(1300.0, 400.0), 0.0, 0.0);
    pet.finish_tick(&input(0.1, WorldPoint::new(500.0, 400.0)));
    assert!(pet.crossing_clear.is_some());
    assert_eq!(pet.approach_hold_until, 0.0);
    pet.pointer_down(pet.position(), 0.2);
    assert_eq!(pet.state(), BehaviorState::Caught);
    assert!(pet.crossing_clear.is_none());
    assert!(!pet.movement.has_route());
}
