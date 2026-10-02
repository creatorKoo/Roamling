// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

use super::*;

fn display(height: f64) -> DisplaySnapshot {
    DisplaySnapshot {
        id: "display".into(),
        name: "Test".into(),
        frame: WorldRect::new(0.0, 0.0, 1_000.0, height),
        visible_frame: WorldRect::new(0.0, 0.0, 1_000.0, height),
        scale: 1.0,
    }
}

fn strolling() -> PetRuntime {
    let mut pet = PetRuntime::new(WorldPoint::new(200.0, 600.0), RuntimeTuning::default(), 7);
    pet.set_object_size(WorldSize::new(96.0, 104.0));
    pet.set_displays(vec![display(800.0)]);
    pet.begin_stroll(WorldPoint::new(800.0, 650.0), 10.0, 1.0 / 60.0);
    assert_eq!(pet.state(), BehaviorState::Wander);
    assert!(pet.movement.has_route());
    pet
}

/// What a phone shell does when its keyboard comes up mid-stroll: the world
/// shrinks to a band at the top and roaming stops. The pet is put in the band,
/// so the walk it was on is over.
#[test]
fn a_world_that_shrinks_mid_stroll_ends_the_walk() {
    let mut pet = strolling();
    let here = pet.movement.position();
    let seat = pet.handle_display_change(vec![display(224.0)], here, 10.5);
    assert!(seat.y < here.y, "the pet was put inside the smaller world");
    assert_eq!(
        pet.state(),
        BehaviorState::Idle,
        "no walk frames without a walk"
    );
    assert!(!pet.movement.has_route());
}

#[test]
fn roaming_turned_off_mid_stroll_ends_the_walk() {
    let mut pet = strolling();
    pet.set_roaming_enabled(false, 10.5);
    assert_eq!(pet.state(), BehaviorState::Idle);
    assert!(!pet.movement.has_route());
}

/// Only a walk is ended. A pet asleep when the desk changes stays asleep.
#[test]
fn a_display_change_leaves_a_pet_that_was_not_walking_alone() {
    let mut pet = PetRuntime::new(WorldPoint::new(200.0, 600.0), RuntimeTuning::default(), 7);
    pet.set_object_size(WorldSize::new(96.0, 104.0));
    pet.set_displays(vec![display(800.0)]);
    pet.behavior.handle(BehaviorInput::BeginRest, 10.0);
    let before = pet.state();
    assert_ne!(before, BehaviorState::Wander);
    pet.handle_display_change(vec![display(224.0)], pet.movement.position(), 10.5);
    assert_eq!(pet.state(), before);
}
