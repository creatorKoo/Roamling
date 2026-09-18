// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

use super::*;
use crate::activity::CompanionEventKind;

#[test]
fn the_same_field_again_keeps_the_rest_spot_verdict_and_a_new_one_drops_it() {
    let field = |value: f64| {
        LuminanceField::new(WorldRect::new(0.0, 0.0, 640.0, 400.0), 8, 5, vec![value; 40]).unwrap()
    };
    let mut pet = PetRuntime::new(WorldPoint::new(320.0, 200.0), RuntimeTuning::default(), 7);
    pet.set_object_size(WorldSize::new(96.0, 104.0));
    pet.set_luminance(Some(field(1.0)));
    let verdict = pet.rest_spot_is_busy();
    assert!(pet.rest_content_check.is_some());

    // What macOS does on every tick.
    pet.set_luminance(Some(field(1.0)));
    assert!(pet.rest_content_check.is_some(), "the same field must not cost another check");
    assert_eq!(pet.rest_spot_is_busy(), verdict);

    pet.set_luminance(Some(field(0.5)));
    assert!(pet.rest_content_check.is_none(), "a new capture has to be looked at");
    pet.rest_spot_is_busy();
    pet.set_luminance(None);
    assert!(pet.rest_content_check.is_none(), "losing the field is a change too");
}

#[test]
fn direct_touch_uses_the_shared_catch_drag_and_drop_without_arming_a_mouse() {
    let mut pet = PetRuntime::new(WorldPoint::new(100.0, 100.0), RuntimeTuning::default(), 7);
    pet.set_object_size(WorldSize::new(96.0, 104.0));
    pet.set_displays(vec![DisplaySnapshot {
        id: "phone".into(), name: "Test".into(),
        frame: WorldRect::new(0.0, 20.0, 400.0, 750.0),
        visible_frame: WorldRect::new(0.0, 20.0, 400.0, 750.0), scale: 1.0,
    }]);
    let contact = WorldPoint::new(140.0, 150.0);
    pet.touch_down(WorldPoint::new(51.0, 150.0), 10.0);
    assert!(!pet.state().is_held());
    pet.set_hidden(true);
    pet.touch_down(contact, 10.0);
    assert!(!pet.state().is_held());
    pet.set_hidden(false);
    pet.set_flags(true, true, false);
    pet.touch_down(contact, 10.0);
    assert!(!pet.state().is_held());
    pet.set_flags(true, true, true);
    pet.touch_down(contact, 10.0);
    assert_eq!(pet.state(), BehaviorState::Caught);
    // A second contact must not replace the original grab offset.
    pet.touch_down(WorldPoint::new(120.0, 130.0), 10.01);
    pet.pointer_dragged(WorldPoint::new(240.0, 250.0), 140.0, 10.1);
    assert_eq!(pet.position(), WorldPoint::new(200.0, 200.0));
    assert_eq!(pet.state(), BehaviorState::Dragged);
    let dropped = pet.pointer_up(WorldPoint::new(500.0, 900.0), false, 10.2);
    assert!(dropped.persist_position);
    assert!(!pet.state().is_held());
    assert!(pet.position().x <= 352.0 && pet.position().y <= 718.0);
}

#[test]
fn hiding_releases_the_catch_and_suppresses_only_luminance() {
    let mut pet = PetRuntime::new(WorldPoint::new(300.0, 300.0), RuntimeTuning::default(), 7);
    let display = DisplaySnapshot {
        id: "display".into(),
        name: "Test".into(),
        frame: WorldRect::new(0.0, 0.0, 1_000.0, 800.0),
        visible_frame: WorldRect::new(0.0, 0.0, 1_000.0, 760.0),
        scale: 1.0,
    };
    pet.set_displays(vec![display]);
    let region = WorldRect::new(100.0, 100.0, 600.0, 400.0);

    pet.approach_hold_until = 42.0;
    pet.request_luminance(region, LUMINANCE_REFRESH_INTERVAL);
    assert_eq!(pet.luminance_requests.len(), 1);

    pet.set_hidden(true);
    assert_eq!(pet.approach_hold_until, 0.0);
    assert!(pet.luminance_requests.is_empty());

    let requests = pet.handle_activity_event(
        CompanionEvent::new(
            "event",
            "agent:hidden-turn",
            10.0,
            CompanionEventKind::ActivityStarted,
            0.5,
            Some(LocationHint::new(Some(region), 0.8)),
        ),
        10.0,
    );
    assert!(requests.is_empty(), "a hidden event requested luminance");
    assert_eq!(pet.active_source_id(), Some("agent:hidden-turn"));

    pet.set_hidden(false);
    assert_eq!(pet.active_source_id(), Some("agent:hidden-turn"));
    pet.request_luminance(region, LUMINANCE_REFRESH_INTERVAL);
    assert_eq!(pet.luminance_requests.len(), 1);
}
