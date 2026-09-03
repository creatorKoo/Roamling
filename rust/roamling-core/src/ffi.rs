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
}

impl From<&FfiActivityEvent> for CompanionEvent {
    fn from(value: &FfiActivityEvent) -> Self {
        CompanionEvent::new(
            value.id.clone(),
            value.source_id.clone(),
            value.timestamp,
            KINDS[value.kind as usize],
            value.intensity,
            value.hint_confidence.map(|confidence| LocationHint::new(None, confidence)),
        )
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
