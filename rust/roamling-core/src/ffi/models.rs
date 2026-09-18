// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

use crate::activity::CompanionReaction;
use crate::behavior::BEHAVIOR_STATES;
use crate::behavior::BehaviorController;
use crate::behavior::BehaviorInput;
use crate::geometry::WorldPoint;
use crate::geometry::WorldVector;
use crate::movement::MovementConfiguration;
use crate::movement::MovementController;
use crate::pointer::PointerInteractionConfiguration;
use crate::pointer::PointerInteractionModel;
use crate::pointer::PointerProximity;
use std::sync::Mutex;

// ------------------------------------------------------- the per-tick models

const PROXIMITIES: [PointerProximity; 5] = [
    PointerProximity::Far,
    PointerProximity::Watching,
    PointerProximity::SlowEvade,
    PointerProximity::FastEvade,
    PointerProximity::Catchable,
];

pub(super) const REACTIONS: [CompanionReaction; 9] = [
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
        FfiPoint {
            x: point.x,
            y: point.y,
        }
    }

    pub fn velocity(&self) -> FfiPoint {
        let velocity = self.inner.lock().unwrap().velocity();
        FfiPoint {
            x: velocity.dx,
            y: velocity.dy,
        }
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
            .map(|point| FfiPoint {
                x: point.x,
                y: point.y,
            })
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
        self.inner
            .lock()
            .unwrap()
            .teleport(WorldPoint::new(x, y), stop);
    }

    pub fn set_velocity(&self, dx: f64, dy: f64) {
        self.inner
            .lock()
            .unwrap()
            .set_velocity(WorldVector::new(dx, dy));
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
