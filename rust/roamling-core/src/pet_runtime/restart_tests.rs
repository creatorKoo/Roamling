// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

use super::*;
use crate::activity::CompanionEventKind;

fn pet() -> PetRuntime {
    let mut pet = PetRuntime::new(WorldPoint::new(100.0, 100.0), RuntimeTuning::default(), 7);
    pet.set_object_size(WorldSize::new(96.0, 104.0));
    pet.set_displays(vec![DisplaySnapshot {
        id: "display".into(),
        name: "Test".into(),
        frame: WorldRect::new(0.0, 0.0, 1_000.0, 800.0),
        visible_frame: WorldRect::new(0.0, 0.0, 1_000.0, 760.0),
        scale: 1.0,
    }]);
    pet
}

/// An update restarts the process, and a restart is only a blink when nothing
/// was going on. Each of these would be something the user saw break.
#[test]
fn a_restart_waits_for_a_pet_that_is_standing_alone_and_in_view() {
    let mut pet = pet();
    assert_eq!(pet.state(), BehaviorState::Idle);
    assert!(pet.is_quiet_for_restart(10.0));

    pet.approach_hold_until = 12.0;
    assert!(!pet.is_quiet_for_restart(11.0), "a hand is on its way to the pet");
    assert!(pet.is_quiet_for_restart(12.5));

    pet.set_hidden(true);
    assert!(
        !pet.is_quiet_for_restart(13.0),
        "a fresh launch would bring a hidden pet back"
    );
    pet.set_hidden(false);
    assert!(pet.is_quiet_for_restart(13.0));

    pet.set_flags(true, true, true);
    pet.touch_down(WorldPoint::new(140.0, 150.0), 14.0);
    assert_eq!(pet.state(), BehaviorState::Caught);
    assert!(!pet.is_quiet_for_restart(14.1), "the pet is in the user's hand");
}

#[test]
fn a_restart_waits_while_a_source_holds_the_pet() {
    let mut pet = pet();
    pet.handle_activity_event(
        CompanionEvent::new(
            "event",
            "agent:turn",
            10.0,
            CompanionEventKind::ActivityStarted,
            0.5,
            None,
        ),
        10.0,
    );
    assert!(pet.active_source_id().is_some());
    assert!(!pet.is_quiet_for_restart(10.1), "an agent is working next to the pet");
}
