// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

use crate::clearance::ClearanceMap;
use crate::*;

const SIZE: WorldSize = WorldSize {
    width: 96.0,
    height: 104.0,
};

fn field(busy: impl Fn(usize, usize) -> bool) -> LuminanceField {
    let samples = (0..40)
        .flat_map(|row| {
            let busy = &busy;
            (0..64).map(move |col| {
                if busy(col, row) {
                    if (col + row) % 2 == 0 {
                        0.1
                    } else {
                        0.9
                    }
                } else {
                    1.0
                }
            })
        })
        .collect();
    LuminanceField::new(WorldRect::new(0.0, 0.0, 1280.0, 800.0), 64, 40, samples).unwrap()
}

fn world(field: Option<LuminanceField>) -> DesktopWorldSnapshot {
    let frame = WorldRect::new(0.0, 0.0, 1280.0, 800.0);
    let mut world = DesktopWorldSnapshot::new(
        vec![DisplaySnapshot {
            id: "test".into(),
            name: "synthetic".into(),
            frame,
            visible_frame: frame,
            scale: 1.0,
        }],
        vec![],
    );
    world.luminance = field;
    world
}

fn input(now: f64) -> TickInput {
    TickInput {
        now,
        pointer: WorldPoint::new(-1000.0, -1000.0),
        primary_button_down: false,
        user_idle_duration: 1000.0,
        capture_authorized: true,
        focus_authorized: false,
        did_query_focus: false,
        queried_focus: None,
        pointer_is_over_pet: false,
        affection_held: false,
    }
}

fn pet(world: &DesktopWorldSnapshot, position: WorldPoint) -> PetRuntime {
    let mut pet = PetRuntime::new(position, RuntimeTuning::default(), 7);
    pet.set_displays(world.displays.clone());
    pet.set_luminance(world.luminance.clone());
    pet
}

#[test]
fn clearance_distinguishes_the_old_top_tier_and_does_not_reward_unseen_space() {
    let field = field(|col, _| col < 20);
    let map = ClearanceMap::new(&field);
    let points = [
        WorldPoint::new(600.0, 400.0),
        WorldPoint::new(1100.0, 400.0),
        WorldPoint::new(2000.0, 400.0),
        WorldPoint::new(200.0, 400.0),
    ];
    assert_eq!(map.best(&points, SIZE), vec![1]);
    assert!(map.distance(points[1], SIZE).unwrap() > 500.0);
    assert!(!map.improves(points[1], WorldPoint::new(1110.0, 400.0), SIZE));
    assert!(
        map.best(&points[2..], SIZE).is_empty(),
        "unseen is not a safe escape from known content"
    );
    assert_eq!(
        map.best(&points[2..3], SIZE),
        vec![0],
        "fully unobserved preserves fallback"
    );
}

fn situation(world: DesktopWorldSnapshot, position: WorldPoint) -> PetSituation {
    PetSituation {
        timestamp: 100.0,
        world,
        position,
        object_size: SIZE,
        pointer_position: None,
        pointer_clearance: 0.0,
        walking_speed: 120.0,
        is_pointer_owned: false,
        is_pointer_watching: false,
        is_evading: false,
        is_walking: false,
        is_resting: false,
        activity_source_id: None,
        activity_hint: None,
        user_idle_duration: 0.0,
        idle_before_rest: 75.0,
        is_roaming_enabled: true,
        is_stroll_due: true,
        stroll_candidates: vec![
            WorldPoint::new(600.0, 400.0),
            WorldPoint::new(1100.0, 400.0),
        ],
    }
}

#[test]
fn runtime_roaming_uses_the_shared_clearance_and_rejects_all_busy_candidates() {
    let world = world(Some(field(|col, _| col < 20)));
    let mut scene = situation(world, WorldPoint::new(600.0, 400.0));
    let mut director = PlacementDirector::for_runtime();
    let PlacementIntent::Stroll(point) = director.decide(&scene) else {
        panic!("expected a stroll");
    };
    assert!(point.x >= 1100.0);
    scene.world.luminance = Some(field(|_, _| true));
    scene.timestamp += 1.0;
    assert_eq!(director.decide(&scene), PlacementIntent::Hold);
}

#[test]
fn an_improved_agent_seat_is_kept_after_arrival_and_dwell() {
    let world = world(Some(field(|col, row| col < 57 && row >= 34)));
    let hint = LocationHint::new(Some(world.displays[0].frame), 1.0);
    let mut scene = situation(world, WorldPoint::new(1222.0, 734.0));
    scene.activity_source_id = Some("codex:turn".into());
    scene.activity_hint = Some(hint);
    let mut director = PlacementDirector::for_runtime();
    let PlacementIntent::Travel(destination, _) = director.decide(&scene) else {
        panic!("expected a safer seat");
    };
    scene.position = destination.point;
    for second in 1..20 {
        scene.timestamp = 100.0 + f64::from(second);
        assert_eq!(director.decide(&scene), PlacementIntent::Hold);
    }
}

#[test]
fn agent_seats_put_clearance_before_bottom_and_caret_preferences() {
    let mut world = world(Some(field(|col, row| col < 57 && row >= 34)));
    let hint = LocationHint::new(Some(world.displays[0].frame), 1.0);
    let current = WorldPoint::new(1222.0, 734.0);
    world.focus = Some(FocusSnapshot::new(
        Some(world.displays[0].frame),
        None,
        Some(WorldRect::new(1120.0, 720.0, 2.0, 20.0)),
        1.0,
    ));
    let old =
        BasicInterestPositionPlanner::destination(&hint, &world, current, None, 0.0, SIZE).unwrap();
    let chosen =
        BasicInterestPositionPlanner::clear_destination(&hint, &world, current, None, 0.0, SIZE)
            .unwrap();
    let map = ClearanceMap::new(world.luminance.as_ref().unwrap());
    assert!(map.improves(old.point, chosen.point, SIZE));
    assert!(map.distance(chosen.point, SIZE).unwrap() > 200.0);
}

#[test]
fn no_capture_keeps_permission_free_destinations() {
    let world = world(None);
    let hint = LocationHint::new(Some(world.displays[0].frame), 1.0);
    let current = WorldPoint::new(1152.0, 704.0);
    assert_eq!(
        BasicInterestPositionPlanner::clear_destination(&hint, &world, current, None, 0.0, SIZE),
        BasicInterestPositionPlanner::destination(&hint, &world, current, None, 0.0, SIZE)
    );
    assert_eq!(
        BasicSafeZonePlanner::clear_destination(&world, current, None, SIZE),
        BasicSafeZonePlanner::destination(&world, current, None, SIZE)
    );
}

#[test]
fn runtime_leaves_the_busy_sleep_corner_for_clear_space() {
    let world = world(Some(field(|col, row| col >= 50 && row >= 30)));
    let original = WorldPoint::new(1152.0, 704.0);
    let mut pet = pet(&world, original);
    for i in 0..1800 {
        let now = 100.0 + f64::from(i) / 30.0;
        pet.begin_tick(now);
        pet.finish_tick(&input(now));
        if pet.state() == BehaviorState::Sleep {
            break;
        }
    }
    assert_eq!(pet.state(), BehaviorState::Sleep);
    assert!(pet.position().distance(original) > 100.0);
    assert!(
        ClearanceMap::new(world.luminance.as_ref().unwrap())
            .distance(pet.position(), SIZE)
            .unwrap()
            > 0.0
    );
}

#[test]
fn all_busy_stays_awake_without_retrying_rest_every_tick() {
    let world = world(Some(field(|_, _| true)));
    let mut pet = pet(&world, WorldPoint::new(1152.0, 704.0));
    let mut deferred = 0;
    for i in 0..300 {
        let now = 100.0 + f64::from(i) / 30.0;
        pet.begin_tick(now);
        let out = pet.finish_tick(&input(now));
        assert_ne!(out.state, BehaviorState::Sleep);
        deferred += out
            .diagnostics
            .iter()
            .filter(|(_, message)| message == "no clear sleep spot, staying awake")
            .count();
    }
    assert_eq!(deferred, 1);
    assert_eq!(pet.state(), BehaviorState::Idle);
}

#[test]
fn new_content_at_arrival_and_under_a_sleeping_pet_is_rechecked() {
    let clear = world(Some(field(|_, _| false)));
    let mut sleeping = pet(&clear, WorldPoint::new(600.0, 400.0));
    for i in 0..100 {
        let now = 100.0 + f64::from(i) / 30.0;
        sleeping.begin_tick(now);
        sleeping.finish_tick(&input(now));
    }
    assert_eq!(sleeping.state(), BehaviorState::Sleep);
    sleeping.set_luminance(Some(field(|_, _| true)));
    sleeping.begin_tick(104.0);
    assert_ne!(
        sleeping.finish_tick(&input(104.0)).state,
        BehaviorState::Sleep
    );

    let busy_corner = world(Some(field(|col, row| col >= 50 && row >= 30)));
    let mut walking = pet(&busy_corner, WorldPoint::new(1152.0, 704.0));
    for i in 0..100 {
        let now = 100.0 + f64::from(i) / 30.0;
        walking.begin_tick(now);
        walking.finish_tick(&input(now));
    }
    assert_eq!(walking.state(), BehaviorState::FindSleepSpot);
    walking.set_luminance(Some(field(|_, _| true)));
    for i in 100..1800 {
        let now = 100.0 + f64::from(i) / 30.0;
        walking.begin_tick(now);
        assert_ne!(walking.finish_tick(&input(now)).state, BehaviorState::Sleep);
    }
}
