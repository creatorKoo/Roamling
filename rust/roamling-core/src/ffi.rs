// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

//! What the Swift shell calls while the port is under way.
//!
//! Only macOS crosses this boundary; the Windows shell links the crate and
//! calls the same functions directly. Kept coarse on purpose -- one call with a
//! whole world rather than a call per rectangle, which is the shape that
//! measured 0.03% of a frame in `docs/windows.md` section 12.

use crate::emptiness::LuminanceField;
use crate::interest::BasicInterestPositionPlanner;
use crate::geometry::{WorldPoint, WorldRect, WorldSize};
use crate::safe_zone::BasicSafeZonePlanner;
use crate::world::{
    DesktopWorldSnapshot, DisplaySnapshot, FocusSnapshot, LocationHint, SafeZone,
};

#[derive(uniffi::Record)]
pub struct FfiRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl From<&FfiRect> for WorldRect {
    fn from(value: &FfiRect) -> Self {
        WorldRect::new(value.x, value.y, value.width, value.height)
    }
}

impl From<WorldRect> for FfiRect {
    fn from(value: WorldRect) -> Self {
        FfiRect {
            x: value.min_x(),
            y: value.min_y(),
            width: value.size.width,
            height: value.size.height,
        }
    }
}

#[derive(uniffi::Record)]
pub struct FfiDisplay {
    pub id: String,
    pub frame: FfiRect,
    pub visible_frame: FfiRect,
}

#[derive(uniffi::Record)]
pub struct FfiSafeZone {
    pub frame: FfiRect,
    pub score: f64,
    pub confidence: f64,
    pub reason: String,
}

impl From<SafeZone> for FfiSafeZone {
    fn from(value: SafeZone) -> Self {
        FfiSafeZone {
            frame: value.frame.into(),
            score: value.score,
            confidence: value.confidence,
            reason: value.reason,
        }
    }
}

fn snapshot(displays: &[FfiDisplay]) -> DesktopWorldSnapshot {
    DesktopWorldSnapshot::new(
        displays
            .iter()
            .map(|display| DisplaySnapshot {
                id: display.id.clone(),
                name: String::new(),
                frame: (&display.frame).into(),
                visible_frame: (&display.visible_frame).into(),
                scale: 1.0,
            })
            .collect(),
        Vec::new(),
    )
}

#[uniffi::export]
pub fn safe_zones(displays: Vec<FfiDisplay>) -> Vec<FfiSafeZone> {
    BasicSafeZonePlanner::safe_zones(&snapshot(&displays))
        .into_iter()
        .map(FfiSafeZone::from)
        .collect()
}

#[derive(uniffi::Record)]
pub struct FfiRestDestination {
    pub x: f64,
    pub y: f64,
    pub display_id: String,
    pub reason: String,
    pub score: f64,
}

#[uniffi::export]
pub fn rest_destination(
    displays: Vec<FfiDisplay>,
    zones: Vec<FfiSafeZone>,
    current_x: f64,
    current_y: f64,
    pointer: Option<Vec<f64>>,
    object_width: f64,
    object_height: f64,
) -> Option<FfiRestDestination> {
    let mut world = snapshot(&displays);
    world.safe_zones = zones
        .into_iter()
        .map(|zone| SafeZone::new((&zone.frame).into(), zone.score, zone.confidence, zone.reason))
        .collect();
    let pointer = pointer.and_then(|values| {
        (values.len() == 2).then(|| WorldPoint::new(values[0], values[1]))
    });
    BasicSafeZonePlanner::destination(
        &world,
        WorldPoint::new(current_x, current_y),
        pointer,
        WorldSize::new(object_width, object_height),
    )
    .map(|destination| FfiRestDestination {
        x: destination.point.x,
        y: destination.point.y,
        display_id: destination.display_id,
        reason: destination.reason,
        score: destination.score,
    })
}

/// The luminance grid, flattened. It is already the small downsampled thing --
/// 64 columns at most -- so passing it whole costs less than holding a handle
/// that has to be invalidated every time the capture refreshes.
#[derive(uniffi::Record)]
pub struct FfiLuminanceField {
    pub bounds: FfiRect,
    pub columns: u32,
    pub rows: u32,
    pub samples: Vec<f64>,
}

#[uniffi::export]
pub fn naps_in_place(
    x: f64,
    y: f64,
    object_width: f64,
    object_height: f64,
    field: Option<FfiLuminanceField>,
    threshold: f64,
) -> bool {
    let field = field.and_then(|value| {
        LuminanceField::new(
            (&value.bounds).into(),
            value.columns as usize,
            value.rows as usize,
            value.samples,
        )
    });
    BasicSafeZonePlanner::naps_in_place(
        WorldPoint::new(x, y),
        WorldSize::new(object_width, object_height),
        field.as_ref(),
        threshold,
    )
}

/// The scene interest placement reads: displays, the window being watched, and
/// whatever focus and capture the user granted.
///
/// One record rather than several calls -- the crossing is free and the
/// serialization is not, so the shape that costs least is the one that carries
/// everything once.
#[derive(uniffi::Record)]
pub struct FfiInterestScene {
    pub displays: Vec<FfiDisplay>,
    pub region: FfiRect,
    pub hint_confidence: f64,
    pub focus: Option<FfiFocus>,
    pub field: Option<FfiLuminanceField>,
}

#[derive(uniffi::Record)]
pub struct FfiFocus {
    pub window_frame: Option<FfiRect>,
    pub focused_element_frame: Option<FfiRect>,
    pub caret_frame: Option<FfiRect>,
    pub confidence: f64,
}

#[derive(uniffi::Record)]
pub struct FfiInterestDestination {
    pub x: f64,
    pub y: f64,
    pub display_id: String,
    pub score: f64,
}

#[derive(uniffi::Record)]
pub struct FfiSeatEvaluation {
    pub x: f64,
    pub y: f64,
    pub display_id: String,
    pub score: f64,
    pub emptiness: Option<f64>,
    pub covers_caret: bool,
    pub watches_region: bool,
    pub is_holdable: bool,
}

impl FfiInterestScene {
    fn parts(&self) -> (DesktopWorldSnapshot, LocationHint) {
        let mut world = snapshot(&self.displays);
        world.focus = self.focus.as_ref().map(|focus| {
            FocusSnapshot::new(
                focus.window_frame.as_ref().map(WorldRect::from),
                focus.focused_element_frame.as_ref().map(WorldRect::from),
                focus.caret_frame.as_ref().map(WorldRect::from),
                focus.confidence,
            )
        });
        world.luminance = self.field.as_ref().and_then(|field| {
            LuminanceField::new(
                (&field.bounds).into(),
                field.columns as usize,
                field.rows as usize,
                field.samples.clone(),
            )
        });
        (world, LocationHint::new(Some((&self.region).into()), self.hint_confidence))
    }
}

#[uniffi::export]
pub fn interest_destination(
    scene: FfiInterestScene,
    current_x: f64,
    current_y: f64,
    pointer: Option<Vec<f64>>,
    object_width: f64,
    object_height: f64,
) -> Option<FfiInterestDestination> {
    let (world, hint) = scene.parts();
    let pointer = pointer
        .and_then(|values| (values.len() == 2).then(|| WorldPoint::new(values[0], values[1])));
    BasicInterestPositionPlanner::destination(
        &hint,
        &world,
        WorldPoint::new(current_x, current_y),
        pointer,
        WorldSize::new(object_width, object_height),
    )
    .map(|destination| FfiInterestDestination {
        x: destination.point.x,
        y: destination.point.y,
        display_id: destination.display_id,
        score: destination.score,
    })
}

#[uniffi::export]
pub fn evaluate_seat(
    scene: FfiInterestScene,
    seat_x: f64,
    seat_y: f64,
    current_x: f64,
    current_y: f64,
    pointer: Option<Vec<f64>>,
    object_width: f64,
    object_height: f64,
) -> Option<FfiSeatEvaluation> {
    let (world, hint) = scene.parts();
    let pointer = pointer
        .and_then(|values| (values.len() == 2).then(|| WorldPoint::new(values[0], values[1])));
    BasicInterestPositionPlanner::evaluate_seat(
        WorldPoint::new(seat_x, seat_y),
        &hint,
        &world,
        WorldPoint::new(current_x, current_y),
        pointer,
        WorldSize::new(object_width, object_height),
    )
    .map(|evaluation| FfiSeatEvaluation {
        x: evaluation.point.x,
        y: evaluation.point.y,
        display_id: evaluation.display_id.clone(),
        score: evaluation.score,
        emptiness: evaluation.emptiness,
        covers_caret: evaluation.covers_caret,
        watches_region: evaluation.watches_region,
        is_holdable: evaluation.is_holdable(),
    })
}

// ------------------------------------------------------- attention and reactions

use crate::activity::{
    CompanionEvent, CompanionEventKind, CompanionReaction, ReactingBehavior, UserContext,
};
use crate::attention::{AttentionModel, ReactionPolicy};
use std::sync::Mutex;

/// Enums cross as indices rather than as uniffi enums: the Swift side already
/// has its own spellings of these, and mapping two names is cheaper than
/// keeping three in step.
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

#[derive(uniffi::Record)]
pub struct FfiActivityEvent {
    pub id: String,
    pub source_id: String,
    pub timestamp: f64,
    pub kind: u8,
    pub intensity: f64,
    pub hint_confidence: Option<f64>,
    /// Where the window is, when the adapter knew. Attention never reads this
    /// -- only the confidence -- but the activity director walks the pet to it.
    pub hint_region: Option<FfiRect>,
    pub context: Option<u8>,
}

impl From<&FfiActivityEvent> for CompanionEvent {
    fn from(value: &FfiActivityEvent) -> Self {
        CompanionEvent::new(
            value.id.clone(),
            value.source_id.clone(),
            value.timestamp,
            KINDS[value.kind as usize],
            value.intensity,
            value.hint_confidence.map(|confidence| {
                LocationHint::new(value.hint_region.as_ref().map(WorldRect::from), confidence)
            }),
        )
        .with_context(value.context.map(|index| CONTEXTS[index as usize]))
    }
}

/// Which source the pet is watching. Held across calls, so Swift keeps a handle
/// rather than shipping the state back and forth.
#[derive(uniffi::Object)]
pub struct Attention {
    model: Mutex<AttentionModel>,
}

#[uniffi::export]
impl Attention {
    #[uniffi::constructor]
    pub fn new() -> std::sync::Arc<Self> {
        std::sync::Arc::new(Self { model: Mutex::new(AttentionModel::default()) })
    }

    /// The id of the event to act on, or nil. Swift looks the event back up in
    /// its own table rather than having it marshalled home.
    pub fn select(&self, events: Vec<FfiActivityEvent>, timestamp: f64) -> Option<String> {
        let events: Vec<CompanionEvent> = events.iter().map(CompanionEvent::from).collect();
        self.model
            .lock()
            .unwrap()
            .select(&events, timestamp)
            .map(|event| event.id)
    }

    pub fn clear(&self, timestamp: f64) {
        self.model.lock().unwrap().clear(timestamp);
    }

    pub fn current_source_id(&self) -> Option<String> {
        self.model.lock().unwrap().current_source_id().map(str::to_owned)
    }
}

/// How often the pet is allowed to react, and with what.
#[derive(uniffi::Object)]
pub struct Reactions {
    policy: Mutex<ReactionPolicy>,
}

#[uniffi::export]
impl Reactions {
    #[uniffi::constructor]
    pub fn new() -> std::sync::Arc<Self> {
        std::sync::Arc::new(Self { policy: Mutex::new(ReactionPolicy::default()) })
    }

    /// The reaction's index, or nil for none.
    pub fn reaction(
        &self,
        event: FfiActivityEvent,
        context: u8,
        is_held_by_pointer: bool,
        random_unit: f64,
        timestamp: f64,
    ) -> Option<u8> {
        let behavior = if is_held_by_pointer {
            ReactingBehavior::Caught
        } else {
            ReactingBehavior::Other
        };
        self.policy
            .lock()
            .unwrap()
            .reaction(
                &CompanionEvent::from(&event),
                CONTEXTS[context as usize],
                behavior,
                random_unit,
                timestamp,
            )
            .map(|reaction| match reaction {
                CompanionReaction::Glance => 0,
                CompanionReaction::Observe => 1,
                CompanionReaction::Spark => 2,
                CompanionReaction::Work => 3,
                CompanionReaction::Paw => 4,
                CompanionReaction::SmallCelebrate => 5,
                CompanionReaction::LargeCelebrate => 6,
                CompanionReaction::Sad => 7,
                CompanionReaction::Calm => 8,
            })
    }
}

// ------------------------------------------------------- the per-tick models

use crate::behavior::{BehaviorController, BehaviorInput, BEHAVIOR_STATES};
use crate::geometry::WorldVector;
use crate::movement::{MovementConfiguration, MovementController};
use crate::pointer::{
    PointerInteractionConfiguration, PointerInteractionModel, PointerProximity,
};

const PROXIMITIES: [PointerProximity; 5] = [
    PointerProximity::Far,
    PointerProximity::Watching,
    PointerProximity::SlowEvade,
    PointerProximity::FastEvade,
    PointerProximity::Catchable,
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

#[derive(uniffi::Record)]
pub struct FfiPoint {
    pub x: f64,
    pub y: f64,
}

#[derive(uniffi::Record)]
pub struct FfiMovementUpdate {
    pub x: f64,
    pub y: f64,
    pub dx: f64,
    pub dy: f64,
    pub reached_destination: bool,
}

/// Where the pet is and where it is going. Held across ticks, so Swift keeps a
/// handle: a route laid on one tick is walked over the two hundred after it,
/// and shipping the waypoints back and forth every frame would be the one
/// shape that makes the crossing cost something.
#[derive(uniffi::Object)]
pub struct Movement {
    inner: Mutex<MovementController>,
}

#[uniffi::export]
impl Movement {
    #[uniffi::constructor]
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        x: f64,
        y: f64,
        dx: f64,
        dy: f64,
        maximum_speed: f64,
        acceleration: f64,
        deceleration: f64,
        arrival_radius: f64,
    ) -> std::sync::Arc<Self> {
        std::sync::Arc::new(Self {
            inner: Mutex::new(MovementController::new(
                WorldPoint::new(x, y),
                WorldVector::new(dx, dy),
                MovementConfiguration::new(
                    maximum_speed,
                    acceleration,
                    deceleration,
                    arrival_radius,
                ),
            )),
        })
    }

    pub fn position(&self) -> FfiPoint {
        let point = self.inner.lock().unwrap().position();
        FfiPoint { x: point.x, y: point.y }
    }

    pub fn velocity(&self) -> FfiPoint {
        let velocity = self.inner.lock().unwrap().velocity();
        FfiPoint { x: velocity.dx, y: velocity.dy }
    }

    pub fn maximum_speed(&self) -> f64 {
        self.inner.lock().unwrap().configuration().maximum_speed
    }

    /// Field assignment, matching Swift's `configuration.maximumSpeed = ...`.
    /// It deliberately does not re-run the initialiser's clamp.
    pub fn set_maximum_speed(&self, value: f64) {
        self.inner.lock().unwrap().set_maximum_speed(value);
    }

    pub fn has_route(&self) -> bool {
        self.inner.lock().unwrap().has_route()
    }

    pub fn destination(&self) -> Option<FfiPoint> {
        self.inner
            .lock()
            .unwrap()
            .destination()
            .map(|point| FfiPoint { x: point.x, y: point.y })
    }

    pub fn set_route(&self, waypoints: Vec<FfiPoint>) {
        self.inner.lock().unwrap().set_route(
            waypoints
                .into_iter()
                .map(|point| WorldPoint::new(point.x, point.y))
                .collect(),
        );
    }

    pub fn cancel_route(&self, stop: bool) {
        self.inner.lock().unwrap().cancel_route(stop);
    }

    pub fn teleport(&self, x: f64, y: f64, stop: bool) {
        self.inner.lock().unwrap().teleport(WorldPoint::new(x, y), stop);
    }

    pub fn set_velocity(&self, dx: f64, dy: f64) {
        self.inner.lock().unwrap().set_velocity(WorldVector::new(dx, dy));
    }

    pub fn update(&self, delta_time: f64) -> FfiMovementUpdate {
        let update = self.inner.lock().unwrap().update(delta_time);
        FfiMovementUpdate {
            x: update.position.x,
            y: update.position.y,
            dx: update.velocity.dx,
            dy: update.velocity.dy,
            reached_destination: update.reached_destination,
        }
    }
}

#[derive(uniffi::Record)]
pub struct FfiPointerDecision {
    pub proximity: u8,
    pub velocity_dx: f64,
    pub velocity_dy: f64,
    pub speed: f64,
    pub distance_to_pet: f64,
    pub closing_speed: f64,
    pub escape_dx: f64,
    pub escape_dy: f64,
    /// Absent when the cursor is on top of the pet, which is the one case with
    /// no direction to look in.
    pub look_direction_degrees: Option<f64>,
    pub attention_rate: f64,
}

/// How near the cursor is, and how fast it is closing. Stateful because closing
/// speed only exists between two samples.
#[derive(uniffi::Object)]
pub struct Pointer {
    inner: Mutex<PointerInteractionModel>,
}

#[uniffi::export]
impl Pointer {
    #[uniffi::constructor]
    pub fn new() -> std::sync::Arc<Self> {
        std::sync::Arc::new(Self {
            inner: Mutex::new(PointerInteractionModel::default()),
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn set_configuration(
        &self,
        awareness_distance: f64,
        slow_evade_distance: f64,
        fast_evade_distance: f64,
        catch_distance: f64,
        slow_evade_speed: f64,
        fast_evade_speed: f64,
        catch_pointer_speed: f64,
        catch_closing_speed: f64,
    ) {
        self.inner
            .lock()
            .unwrap()
            .set_configuration(PointerInteractionConfiguration::new(
                awareness_distance,
                slow_evade_distance,
                fast_evade_distance,
                catch_distance,
                slow_evade_speed,
                fast_evade_speed,
                catch_pointer_speed,
                catch_closing_speed,
            ));
    }

    pub fn reset(&self) {
        self.inner.lock().unwrap().reset();
    }

    pub fn evaluate(
        &self,
        pointer_x: f64,
        pointer_y: f64,
        pet_x: f64,
        pet_y: f64,
        timestamp: f64,
    ) -> FfiPointerDecision {
        let decision = self.inner.lock().unwrap().evaluate(
            WorldPoint::new(pointer_x, pointer_y),
            WorldPoint::new(pet_x, pet_y),
            timestamp,
        );
        FfiPointerDecision {
            proximity: PROXIMITIES
                .iter()
                .position(|candidate| *candidate == decision.proximity)
                .unwrap_or(0) as u8,
            velocity_dx: decision.kinematics.velocity.dx,
            velocity_dy: decision.kinematics.velocity.dy,
            speed: decision.kinematics.speed,
            distance_to_pet: decision.kinematics.distance_to_pet,
            closing_speed: decision.kinematics.closing_speed,
            escape_dx: decision.escape_velocity.dx,
            escape_dy: decision.escape_velocity.dy,
            look_direction_degrees: decision.look_direction_degrees,
            attention_rate: decision.attention_rate,
        }
    }
}

#[derive(uniffi::Record)]
pub struct FfiBehaviorTransition {
    pub from: u8,
    pub to: u8,
    pub changed: bool,
    pub entered_at: f64,
}

/// The state machine, and the timestamp it entered its current state at. Both
/// have to survive the tick, which is why this is a handle and not a call.
#[derive(uniffi::Object)]
pub struct Behavior {
    inner: Mutex<BehaviorController>,
}

#[uniffi::export]
impl Behavior {
    #[uniffi::constructor]
    pub fn new(state: u8, entered_at: f64) -> std::sync::Arc<Self> {
        std::sync::Arc::new(Self {
            inner: Mutex::new(BehaviorController::new(
                BEHAVIOR_STATES[state as usize],
                entered_at,
            )),
        })
    }

    pub fn state(&self) -> u8 {
        let state = self.inner.lock().unwrap().state();
        BEHAVIOR_STATES
            .iter()
            .position(|candidate| *candidate == state)
            .unwrap_or(0) as u8
    }

    pub fn entered_at(&self) -> f64 {
        self.inner.lock().unwrap().entered_at()
    }

    /// `argument` names the proximity for a pointer input and the reaction for
    /// a reaction input, and is ignored by the other twelve. Two u8s cross more
    /// cheaply than an enum with payloads, and the vocabulary is closed.
    pub fn handle(&self, input: u8, argument: u8, timestamp: f64) -> FfiBehaviorTransition {
        let event = match input {
            0 => BehaviorInput::BeginWander,
            1 => BehaviorInput::Arrived,
            2 => BehaviorInput::BeginRest,
            3 => BehaviorInput::SeekSleepSpot,
            4 => BehaviorInput::SleepSpotReached,
            5 => BehaviorInput::BeginStretch,
            6 => BehaviorInput::BeginInterestTravel,
            7 => BehaviorInput::Pointer(PROXIMITIES[argument as usize]),
            8 => BehaviorInput::CatchBegan,
            9 => BehaviorInput::DragMoved,
            10 => BehaviorInput::MouseReleased,
            11 => BehaviorInput::Reaction(REACTIONS[argument as usize]),
            12 => BehaviorInput::MeaningfulActivity,
            _ => BehaviorInput::Tick,
        };
        let mut model = self.inner.lock().unwrap();
        let transition = model.handle(event, timestamp);
        let index = |state| {
            BEHAVIOR_STATES
                .iter()
                .position(|candidate| *candidate == state)
                .unwrap_or(0) as u8
        };
        FfiBehaviorTransition {
            from: index(transition.from),
            to: index(transition.to),
            changed: transition.changed,
            entered_at: model.entered_at(),
        }
    }
}

// -------------------------------------------------------------- the director

use crate::placement::{
    PetSituation, PlacementDirector, PlacementIntent, PlacementTravelReason,
};

const TRAVEL_REASONS: [PlacementTravelReason; 5] = [
    PlacementTravelReason::NewActivity,
    PlacementTravelReason::CoveringCaret,
    PlacementTravelReason::CoveringWork,
    PlacementTravelReason::PlannedBlind,
    PlacementTravelReason::FollowedFocus,
];

#[derive(uniffi::Record)]
pub struct FfiHint {
    /// Absent for a hint that names a window but not where it is. The planner
    /// refuses to place against one, and so does this.
    pub region: Option<FfiRect>,
    pub confidence: f64,
}

/// One tick's worth of what placement is allowed to look at.
///
/// The displays and the luminance grid are deliberately *not* here. They change
/// a few times a second at most and the grid is 64 columns wide, so pushing
/// them separately keeps the per-tick payload to a few dozen numbers instead of
/// a few thousand.
#[derive(uniffi::Record)]
pub struct FfiSituation {
    pub timestamp: f64,
    pub x: f64,
    pub y: f64,
    pub object_width: f64,
    pub object_height: f64,
    pub pointer: Option<Vec<f64>>,
    pub walking_speed: f64,
    pub is_pointer_owned: bool,
    pub is_pointer_watching: bool,
    pub is_evading: bool,
    pub is_walking: bool,
    pub is_resting: bool,
    pub activity_source_id: Option<String>,
    pub hint: Option<FfiHint>,
    pub focus: Option<FfiFocus>,
    pub user_idle_duration: f64,
    pub idle_before_rest: f64,
    pub is_roaming_enabled: bool,
    pub is_stroll_due: bool,
    pub stroll_candidates: Vec<FfiPoint>,
}

#[derive(uniffi::Record)]
pub struct FfiPlacementIntent {
    /// 0 none, 1 hold, 2 travel, 3 sleep in place, 4 stroll, 5 escape.
    pub kind: u8,
    pub x: f64,
    pub y: f64,
    pub score: f64,
    /// Meaningful only when `kind` is 2, along with `score` and `display_id`.
    pub reason: u8,
    pub display_id: String,
    pub is_seated: bool,
    pub is_travelling: bool,
}

/// The one place that answers where the pet should be, with its seat, its trip
/// and the time of its last review held on this side.
#[derive(uniffi::Object)]
pub struct Placement {
    director: Mutex<PlacementDirector>,
    displays: Mutex<Vec<DisplaySnapshot>>,
    field: Mutex<Option<LuminanceField>>,
}

#[uniffi::export]
impl Placement {
    #[uniffi::constructor]
    pub fn new() -> std::sync::Arc<Self> {
        std::sync::Arc::new(Self {
            director: Mutex::new(PlacementDirector::default()),
            displays: Mutex::new(Vec::new()),
            field: Mutex::new(None),
        })
    }

    /// Pushed when the desk changes, not every tick.
    pub fn set_displays(&self, displays: Vec<FfiDisplay>) {
        *self.displays.lock().unwrap() = displays
            .iter()
            .map(|display| DisplaySnapshot {
                id: display.id.clone(),
                name: String::new(),
                frame: (&display.frame).into(),
                visible_frame: (&display.visible_frame).into(),
                scale: 1.0,
            })
            .collect();
    }

    /// Pushed when a capture lands, not every tick. The grid is the expensive
    /// part of the situation and it refreshes every three seconds at most.
    pub fn set_field(&self, field: Option<FfiLuminanceField>) {
        *self.field.lock().unwrap() = field.and_then(|value| {
            LuminanceField::new(
                (&value.bounds).into(),
                value.columns as usize,
                value.rows as usize,
                value.samples,
            )
        });
    }

    pub fn is_seated(&self) -> bool {
        self.director.lock().unwrap().is_seated()
    }

    pub fn is_travelling(&self) -> bool {
        self.director.lock().unwrap().is_travelling()
    }

    pub fn settle_in_place(&self, source_id: Option<String>, timestamp: f64) {
        self.director
            .lock()
            .unwrap()
            .settle_in_place(source_id.as_deref(), timestamp);
    }

    pub fn decide(&self, situation: FfiSituation) -> FfiPlacementIntent {
        let mut world = DesktopWorldSnapshot::new(self.displays.lock().unwrap().clone(), Vec::new());
        world.focus = situation.focus.as_ref().map(|focus| {
            FocusSnapshot::new(
                focus.window_frame.as_ref().map(WorldRect::from),
                focus.focused_element_frame.as_ref().map(WorldRect::from),
                focus.caret_frame.as_ref().map(WorldRect::from),
                focus.confidence,
            )
        });
        world.luminance = self.field.lock().unwrap().clone();

        let situation = PetSituation {
            timestamp: situation.timestamp,
            world,
            position: WorldPoint::new(situation.x, situation.y),
            object_size: WorldSize::new(situation.object_width, situation.object_height),
            pointer_position: situation
                .pointer
                .and_then(|values| (values.len() == 2).then(|| WorldPoint::new(values[0], values[1]))),
            walking_speed: situation.walking_speed,
            is_pointer_owned: situation.is_pointer_owned,
            is_pointer_watching: situation.is_pointer_watching,
            is_evading: situation.is_evading,
            is_walking: situation.is_walking,
            is_resting: situation.is_resting,
            activity_source_id: situation.activity_source_id,
            activity_hint: situation.hint.as_ref().map(|hint| {
                LocationHint::new(hint.region.as_ref().map(WorldRect::from), hint.confidence)
            }),
            user_idle_duration: situation.user_idle_duration,
            idle_before_rest: situation.idle_before_rest,
            is_roaming_enabled: situation.is_roaming_enabled,
            is_stroll_due: situation.is_stroll_due,
            stroll_candidates: situation
                .stroll_candidates
                .iter()
                .map(|point| WorldPoint::new(point.x, point.y))
                .collect(),
        };

        let mut director = self.director.lock().unwrap();
        let intent = director.decide(&situation);
        let (kind, x, y, score, reason, display_id) = match &intent {
            PlacementIntent::None => (0u8, 0.0, 0.0, 0.0, 0u8, String::new()),
            PlacementIntent::Hold => (1, 0.0, 0.0, 0.0, 0, String::new()),
            PlacementIntent::Travel(destination, reason) => (
                2,
                destination.point.x,
                destination.point.y,
                destination.score,
                TRAVEL_REASONS
                    .iter()
                    .position(|value| value == reason)
                    .unwrap_or(0) as u8,
                destination.display_id.clone(),
            ),
            PlacementIntent::SleepInPlace => (3, 0.0, 0.0, 0.0, 0, String::new()),
            PlacementIntent::Stroll(point) => (4, point.x, point.y, 0.0, 0, String::new()),
            PlacementIntent::Escape(point) => (5, point.x, point.y, 0.0, 0, String::new()),
        };
        FfiPlacementIntent {
            kind,
            x,
            y,
            score,
            reason,
            display_id,
            is_seated: director.is_seated(),
            is_travelling: director.is_travelling(),
        }
    }
}

// ----------------------------------------------------------------- the knobs

use crate::tuning::{RuntimeTuning, TUNING_KEYS};

#[derive(uniffi::Record)]
pub struct FfiTuning {
    pub walking_speed: f64,
    pub wander_pause: f64,
    pub cross_display_wander_chance: f64,
    pub pointer_awareness_distance: f64,
    pub catch_arm_distance: f64,
    pub catch_approach_speed: f64,
    pub catch_window: f64,
    pub hit_region_scale: f64,
    pub gait_cadence: f64,
    pub evade_speed_scale: f64,
    pub idle_before_rest: f64,
}

#[derive(uniffi::Record)]
pub struct FfiRange {
    pub lower: f64,
    pub upper: f64,
}

/// Shape A: a value in, a value out. Called when the panel moves a slider and
/// when a saved blob is decoded, not on the tick.
#[uniffi::export]
#[allow(clippy::too_many_arguments)]
pub fn normalize_tuning(
    walking_speed: f64,
    wander_pause: f64,
    cross_display_wander_chance: f64,
    pointer_awareness_distance: f64,
    catch_arm_distance: f64,
    catch_approach_speed: f64,
    catch_window: f64,
    hit_region_scale: f64,
    gait_cadence: f64,
    evade_speed_scale: f64,
    idle_before_rest: f64,
) -> FfiTuning {
    let tuning = RuntimeTuning::new(
        walking_speed,
        wander_pause,
        cross_display_wander_chance,
        pointer_awareness_distance,
        catch_arm_distance,
        catch_approach_speed,
        catch_window,
        hit_region_scale,
        gait_cadence,
        evade_speed_scale,
        idle_before_rest,
    );
    FfiTuning {
        walking_speed: tuning.walking_speed,
        wander_pause: tuning.wander_pause,
        cross_display_wander_chance: tuning.cross_display_wander_chance,
        pointer_awareness_distance: tuning.pointer_awareness_distance,
        catch_arm_distance: tuning.catch_arm_distance,
        catch_approach_speed: tuning.catch_approach_speed,
        catch_window: tuning.catch_window,
        hit_region_scale: tuning.hit_region_scale,
        gait_cadence: tuning.gait_cadence,
        evade_speed_scale: tuning.evade_speed_scale,
        idle_before_rest: tuning.idle_before_rest,
    }
}

/// What the initialiser will clamp this key to, given the rest of the tuning.
/// One bound moves: arming a catch further away than the pet can notice is
/// meaningless, so it ends where awareness does.
#[uniffi::export]
pub fn tuning_limits(key: u8, pointer_awareness: f64) -> FfiRange {
    let (lower, upper) = RuntimeTuning::bounds(TUNING_KEYS[key as usize], pointer_awareness);
    FfiRange { lower, upper }
}

#[uniffi::export]
pub fn tuning_fast_evade_speed(walking_speed: f64, evade_speed_scale: f64) -> f64 {
    let mut tuning = RuntimeTuning::default();
    tuning.walking_speed = walking_speed;
    tuning.evade_speed_scale = evade_speed_scale;
    tuning.fast_evade_speed()
}

#[uniffi::export]
pub fn tuning_slow_evade_speed(walking_speed: f64, evade_speed_scale: f64) -> f64 {
    let mut tuning = RuntimeTuning::default();
    tuning.walking_speed = walking_speed;
    tuning.evade_speed_scale = evade_speed_scale;
    tuning.slow_evade_speed()
}

#[uniffi::export]
pub fn tuning_wander_delay(wander_pause: f64, random_unit: f64) -> f64 {
    let mut tuning = RuntimeTuning::default();
    tuning.wander_pause = wander_pause;
    tuning.wander_delay(random_unit)
}

// ----------------------------------------------------- the activity director

use crate::activity_director::{ActivityDirector, ActivityEffect};

/// One thing the runtime must do, in the order the decision made them. The
/// order is part of the answer: a setback settles the seat, cancels the route
/// and *then* reacts.
#[derive(uniffi::Record)]
pub struct FfiActivityEffect {
    /// 0 cancel rest, 1 settle in place, 2 cancel route, 3 next wander at,
    /// 4 apply reaction, 5 request luminance.
    pub kind: u8,
    pub source_id: Option<String>,
    pub timestamp: f64,
    pub reaction: u8,
    pub region: Option<FfiRect>,
}

fn effect(effect: &ActivityEffect) -> FfiActivityEffect {
    let mut answer = FfiActivityEffect {
        kind: 0,
        source_id: None,
        timestamp: 0.0,
        reaction: 0,
        region: None,
    };
    match effect {
        ActivityEffect::CancelRest => {}
        ActivityEffect::SettleInPlace { source_id } => {
            answer.kind = 1;
            answer.source_id = Some(source_id.clone());
        }
        ActivityEffect::CancelRoute => answer.kind = 2,
        ActivityEffect::SetNextWanderAt { timestamp } => {
            answer.kind = 3;
            answer.timestamp = *timestamp;
        }
        ActivityEffect::ApplyReaction { reaction } => {
            answer.kind = 4;
            answer.reaction = REACTIONS
                .iter()
                .position(|candidate| candidate == reaction)
                .unwrap_or(0) as u8;
        }
        ActivityEffect::RequestLuminance { region } => {
            answer.kind = 5;
            answer.region = Some((*region).into());
        }
    }
    answer
}

/// Which agent the pet is watching, what it wears, and what it still owes the
/// user when it arrives. Seven fields that survived between ticks on the
/// runtime, each written by more than one code path.
#[derive(uniffi::Object)]
pub struct ActivityWatch {
    inner: Mutex<ActivityDirector>,
}

#[uniffi::export]
impl ActivityWatch {
    #[uniffi::constructor]
    pub fn new() -> std::sync::Arc<Self> {
        std::sync::Arc::new(Self {
            inner: Mutex::new(ActivityDirector::default()),
        })
    }

    pub fn handle_event(
        &self,
        event: FfiActivityEvent,
        is_held_by_pointer: bool,
        is_resting: bool,
        random_unit: f64,
        timestamp: f64,
    ) -> Vec<FfiActivityEffect> {
        self.inner
            .lock()
            .unwrap()
            .handle_event(
                CompanionEvent::from(&event),
                is_held_by_pointer,
                is_resting,
                random_unit,
                timestamp,
            )
            .iter()
            .map(effect)
            .collect()
    }

    pub fn expire_silent(&self, is_resting: bool, timestamp: f64) -> Vec<FfiActivityEffect> {
        self.inner
            .lock()
            .unwrap()
            .expire_silent(is_resting, timestamp)
            .iter()
            .map(effect)
            .collect()
    }

    pub fn resume_pending_if_ready(
        &self,
        is_idle: bool,
        is_held_by_pointer: bool,
        is_resting: bool,
        random_unit: f64,
        timestamp: f64,
    ) -> Vec<FfiActivityEffect> {
        self.inner
            .lock()
            .unwrap()
            .resume_pending_if_ready(
                is_idle,
                is_held_by_pointer,
                is_resting,
                random_unit,
                timestamp,
            )
            .iter()
            .map(effect)
            .collect()
    }

    pub fn deliver_arrival_reaction(
        &self,
        is_resting: bool,
        timestamp: f64,
    ) -> Vec<FfiActivityEffect> {
        self.inner
            .lock()
            .unwrap()
            .deliver_arrival_reaction(is_resting, timestamp)
            .iter()
            .map(effect)
            .collect()
    }

    pub fn sustain_on_seat(&self, is_resting: bool, timestamp: f64) -> Vec<FfiActivityEffect> {
        self.inner
            .lock()
            .unwrap()
            .sustain_on_seat(is_resting, timestamp)
            .iter()
            .map(effect)
            .collect()
    }

    pub fn is_watching_window(&self) -> bool {
        self.inner.lock().unwrap().is_watching_window()
    }

    pub fn active_source_id(&self) -> Option<String> {
        self.inner.lock().unwrap().active_source_id().map(str::to_owned)
    }

    pub fn hint(&self) -> Option<FfiHint> {
        self.inner.lock().unwrap().hint().map(|hint| FfiHint {
            region: hint.approximate_region.map(FfiRect::from),
            confidence: hint.confidence,
        })
    }

    pub fn has_arrival_reaction(&self) -> bool {
        self.inner.lock().unwrap().has_arrival_reaction()
    }

    pub fn sustained_reaction(&self) -> Option<u8> {
        self.inner.lock().unwrap().sustained_reaction().map(|reaction| {
            REACTIONS
                .iter()
                .position(|candidate| *candidate == reaction)
                .unwrap_or(0) as u8
        })
    }
}

/// Whether an event without a window of its own is worth asking the platform
/// where its window is. The query costs a synchronous round trip, so the rule
/// lives here and the call stays with the caller.
#[uniffi::export]
pub fn activity_wants_window_hint(kind: u8) -> bool {
    crate::activity_director::wants_window_hint(KINDS[kind as usize])
}

// ------------------------------------------------------------- the tick loop

use crate::capability::PET_CAPABILITIES;
use crate::pet_runtime::{PetRuntime, TickInput};
use crate::tuning::RuntimeTuning as CoreTuning;

#[derive(uniffi::Record)]
pub struct FfiDiagnostic {
    pub category: String,
    pub message: String,
}

/// A capture the runtime would like, near a region it cares about. The caller
/// owns the permission, the task and the throttle.
#[derive(uniffi::Record)]
pub struct FfiLuminanceRequest {
    pub region: FfiRect,
    pub interval: f64,
}

#[derive(uniffi::Record)]
pub struct FfiTickInput {
    pub now: f64,
    pub pointer_x: f64,
    pub pointer_y: f64,
    pub primary_button_down: bool,
    pub user_idle_duration: f64,
    pub capture_authorized: bool,
    pub focus_authorized: bool,
    pub did_query_focus: bool,
    pub queried_focus: Option<FfiFocus>,
    pub pointer_is_over_pet: bool,
}

#[derive(uniffi::Record)]
pub struct FfiTickOutput {
    pub delta_time: f64,
    pub x: f64,
    pub y: f64,
    pub state: u8,
    pub capability: u8,
    pub look_direction_degrees: Option<f64>,
    pub locomotion_rate: f64,
    pub interaction_enabled: bool,
    pub luminance_requests: Vec<FfiLuminanceRequest>,
    pub diagnostics: Vec<FfiDiagnostic>,
    pub persist_position: bool,
}

#[derive(uniffi::Record)]
pub struct FfiInteractionOutput {
    pub x: f64,
    pub y: f64,
    pub capability: u8,
    pub look_direction_degrees: Option<f64>,
    pub set_interaction_enabled: Option<bool>,
    pub render: bool,
    pub reschedule_after: Option<f64>,
    pub persist_position: bool,
}

fn tuning_from(value: &FfiTuning) -> CoreTuning {
    CoreTuning::new(
        value.walking_speed,
        value.wander_pause,
        value.cross_display_wander_chance,
        value.pointer_awareness_distance,
        value.catch_arm_distance,
        value.catch_approach_speed,
        value.catch_window,
        value.hit_region_scale,
        value.gait_cadence,
        value.evade_speed_scale,
        value.idle_before_rest,
    )
}

fn displays_from(displays: &[FfiDisplay]) -> Vec<DisplaySnapshot> {
    displays
        .iter()
        .map(|display| DisplaySnapshot {
            id: display.id.clone(),
            name: String::new(),
            frame: (&display.frame).into(),
            visible_frame: (&display.visible_frame).into(),
            scale: 1.0,
        })
        .collect()
}

fn capability_index(capability: crate::capability::PetCapability) -> u8 {
    PET_CAPABILITIES
        .iter()
        .position(|candidate| *candidate == capability)
        .unwrap_or(0) as u8
}

fn state_index(state: crate::behavior::BehaviorState) -> u8 {
    BEHAVIOR_STATES
        .iter()
        .position(|candidate| *candidate == state)
        .unwrap_or(0) as u8
}

fn interaction_out(answer: crate::pet_runtime::InteractionOutput) -> FfiInteractionOutput {
    FfiInteractionOutput {
        x: answer.position.x,
        y: answer.position.y,
        capability: capability_index(answer.capability),
        look_direction_degrees: answer.look_direction_degrees,
        set_interaction_enabled: answer.set_interaction_enabled,
        render: answer.render,
        reschedule_after: answer.reschedule_after,
        persist_position: answer.persist_position,
    }
}

/// Everything the pet decides, in one object. The shell around it keeps only
/// the parts that are not decisions: the timer, the defaults, the diagnostics
/// file, the agent subscriptions and the sprite sheet.
#[derive(uniffi::Object)]
pub struct PetLoop {
    inner: Mutex<PetRuntime>,
}

#[uniffi::export]
impl PetLoop {
    #[uniffi::constructor]
    pub fn new(x: f64, y: f64, tuning: FfiTuning, seed: u64) -> std::sync::Arc<Self> {
        std::sync::Arc::new(Self {
            inner: Mutex::new(PetRuntime::new(
                WorldPoint::new(x, y),
                tuning_from(&tuning),
                seed,
            )),
        })
    }

    pub fn set_displays(&self, displays: Vec<FfiDisplay>) {
        self.inner.lock().unwrap().set_displays(displays_from(&displays));
    }

    pub fn set_luminance(&self, field: Option<FfiLuminanceField>) {
        let field = field.and_then(|value| {
            LuminanceField::new(
                (&value.bounds).into(),
                value.columns as usize,
                value.rows as usize,
                value.samples,
            )
        });
        self.inner.lock().unwrap().set_luminance(field);
    }

    pub fn set_object_size(&self, width: f64, height: f64) {
        self.inner
            .lock()
            .unwrap()
            .set_object_size(WorldSize::new(width, height));
    }

    pub fn set_flags(&self, roaming: bool, avoidance: bool, interactions: bool) {
        self.inner
            .lock()
            .unwrap()
            .set_flags(roaming, avoidance, interactions);
    }

    pub fn set_roaming_enabled(&self, enabled: bool, now: f64) {
        self.inner.lock().unwrap().set_roaming_enabled(enabled, now);
    }

    pub fn set_pointer_avoidance_enabled(&self, enabled: bool) {
        self.inner.lock().unwrap().set_pointer_avoidance_enabled(enabled);
    }

    pub fn set_interactions_enabled(&self, enabled: bool) -> bool {
        self.inner.lock().unwrap().set_interactions_enabled(enabled)
    }

    pub fn set_animation_durations(&self, caught: f64, dragged: f64) {
        self.inner
            .lock()
            .unwrap()
            .set_animation_durations(caught, dragged);
    }

    pub fn set_position(&self, x: f64, y: f64) {
        self.inner.lock().unwrap().set_position(WorldPoint::new(x, y));
    }

    pub fn clear_click_reaction(&self, clear_caught_transition: bool) {
        self.inner
            .lock()
            .unwrap()
            .clear_click_reaction(clear_caught_transition);
    }

    pub fn set_next_wander_at(&self, timestamp: f64) {
        self.inner.lock().unwrap().set_next_wander_at(timestamp);
    }

    pub fn apply_tuning(&self, tuning: FfiTuning, now: f64) {
        self.inner
            .lock()
            .unwrap()
            .apply_tuning(tuning_from(&tuning), now);
    }

    pub fn handle_display_change(
        &self,
        displays: Vec<FfiDisplay>,
        carried_x: f64,
        carried_y: f64,
        now: f64,
    ) -> FfiPoint {
        let point = self.inner.lock().unwrap().handle_display_change(
            displays_from(&displays),
            WorldPoint::new(carried_x, carried_y),
            now,
        );
        FfiPoint { x: point.x, y: point.y }
    }

    pub fn set_scale(&self, width: f64, height: f64) -> FfiPoint {
        let point = self
            .inner
            .lock()
            .unwrap()
            .set_scale(WorldSize::new(width, height));
        FfiPoint { x: point.x, y: point.y }
    }

    /// Everything before the platform is asked anything. True when an
    /// accessibility query is worth paying for this tick.
    pub fn begin_tick(&self, now: f64) -> bool {
        self.inner.lock().unwrap().begin_tick(now)
    }

    pub fn finish_tick(&self, input: FfiTickInput) -> FfiTickOutput {
        let focus = input.queried_focus.as_ref().map(|focus| {
            FocusSnapshot::new(
                focus.window_frame.as_ref().map(WorldRect::from),
                focus.focused_element_frame.as_ref().map(WorldRect::from),
                focus.caret_frame.as_ref().map(WorldRect::from),
                focus.confidence,
            )
        });
        let answer = self.inner.lock().unwrap().finish_tick(&TickInput {
            now: input.now,
            pointer: WorldPoint::new(input.pointer_x, input.pointer_y),
            primary_button_down: input.primary_button_down,
            user_idle_duration: input.user_idle_duration,
            capture_authorized: input.capture_authorized,
            focus_authorized: input.focus_authorized,
            did_query_focus: input.did_query_focus,
            queried_focus: focus,
            pointer_is_over_pet: input.pointer_is_over_pet,
        });
        FfiTickOutput {
            delta_time: answer.delta_time,
            x: answer.position.x,
            y: answer.position.y,
            state: state_index(answer.state),
            capability: capability_index(answer.capability),
            look_direction_degrees: answer.look_direction_degrees,
            locomotion_rate: answer.locomotion_rate,
            interaction_enabled: answer.interaction_enabled,
            luminance_requests: answer
                .luminance_requests
                .into_iter()
                .map(|request| FfiLuminanceRequest {
                    region: request.region.into(),
                    interval: request.interval,
                })
                .collect(),
            diagnostics: answer
                .diagnostics
                .into_iter()
                .map(|(category, message)| FfiDiagnostic { category, message })
                .collect(),
            persist_position: answer.persist_position,
        }
    }

    pub fn pointer_down(&self, x: f64, y: f64, now: f64) -> FfiInteractionOutput {
        interaction_out(
            self.inner
                .lock()
                .unwrap()
                .pointer_down(WorldPoint::new(x, y), now),
        )
    }

    pub fn pointer_dragged(&self, x: f64, y: f64, distance: f64, now: f64) -> FfiInteractionOutput {
        interaction_out(self.inner.lock().unwrap().pointer_dragged(
            WorldPoint::new(x, y),
            distance,
            now,
        ))
    }

    pub fn pointer_up(&self, x: f64, y: f64, was_dragged: bool, now: f64) -> FfiInteractionOutput {
        interaction_out(self.inner.lock().unwrap().pointer_up(
            WorldPoint::new(x, y),
            was_dragged,
            now,
        ))
    }

    pub fn handle_activity_event(
        &self,
        event: FfiActivityEvent,
        now: f64,
    ) -> Vec<FfiLuminanceRequest> {
        self.inner
            .lock()
            .unwrap()
            .handle_activity_event(CompanionEvent::from(&event), now)
            .into_iter()
            .map(|request| FfiLuminanceRequest {
                region: request.region.into(),
                interval: request.interval,
            })
            .collect()
    }

    pub fn position(&self) -> FfiPoint {
        let point = self.inner.lock().unwrap().position();
        FfiPoint { x: point.x, y: point.y }
    }

    pub fn state(&self) -> u8 {
        state_index(self.inner.lock().unwrap().state())
    }

    pub fn is_placement_travelling(&self) -> bool {
        self.inner.lock().unwrap().is_placement_travelling()
    }

    pub fn is_watching_window(&self) -> bool {
        self.inner.lock().unwrap().is_watching_window()
    }

    pub fn active_source_id(&self) -> Option<String> {
        self.inner.lock().unwrap().active_source_id().map(str::to_owned)
    }

    /// How many random numbers the pet has spent. Only the recorded-session
    /// test reads it, and it is the fastest way to see two runs part company.
    pub fn draws(&self) -> u64 {
        self.inner.lock().unwrap().draws()
    }

    pub fn preferred_tick_interval(&self, now: f64) -> f64 {
        self.inner.lock().unwrap().preferred_tick_interval(now)
    }
}
